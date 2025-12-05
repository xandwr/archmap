use crate::analysis::DependencyGraph;
use crate::cli::{AiOutputFormat, PriorityStrategy};
use crate::model::{AnalysisResult, DefinitionKind, Issue, IssueKind, Module, Visibility};
use crate::output::OutputFormatter;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use tiktoken_rs::cl100k_base;

/// AI-optimized output formatter
pub struct AiOutput {
    pub project_root: Option<PathBuf>,
    pub topo_order: bool,
    pub signatures_only: bool,
    pub token_budget: Option<usize>,
    pub priority_strategy: PriorityStrategy,
    pub format: AiOutputFormat,
    /// Source files for signature extraction
    pub sources: HashMap<PathBuf, String>,
}

impl AiOutput {
    pub fn new(project_root: Option<PathBuf>) -> Self {
        Self {
            project_root,
            topo_order: true,
            signatures_only: false,
            token_budget: None,
            priority_strategy: PriorityStrategy::FanIn,
            format: AiOutputFormat::Markdown,
            sources: HashMap::new(),
        }
    }

    pub fn with_topo_order(mut self, enabled: bool) -> Self {
        self.topo_order = enabled;
        self
    }

    pub fn with_signatures_only(mut self, enabled: bool) -> Self {
        self.signatures_only = enabled;
        self
    }

    pub fn with_token_budget(mut self, tokens: usize) -> Self {
        self.token_budget = Some(tokens);
        self
    }

    pub fn with_priority(mut self, strategy: PriorityStrategy) -> Self {
        self.priority_strategy = strategy;
        self
    }

    pub fn with_format(mut self, format: AiOutputFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_sources(mut self, sources: HashMap<PathBuf, String>) -> Self {
        self.sources = sources;
        self
    }

    fn relative_path(&self, path: &std::path::Path) -> String {
        if let Some(ref root) = self.project_root {
            path.strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string()
        } else {
            path.display().to_string()
        }
    }

    fn order_modules<'a>(&self, modules: &'a [Module], graph: &DependencyGraph) -> Vec<&'a Module> {
        if self.topo_order {
            let order = graph.topological_order_with_cycles();

            // Map ordered paths back to modules
            order
                .iter()
                .filter_map(|path| modules.iter().find(|m| &m.path == path))
                .collect()
        } else {
            modules.iter().collect()
        }
    }

    fn prioritize_modules<'a>(
        &self,
        modules: &'a [Module],
        graph: &DependencyGraph,
    ) -> Vec<(&'a Module, f64)> {
        let mut scored: Vec<_> = modules
            .iter()
            .map(|m| {
                let score = graph.importance_score(&m.path, modules);
                (m, score)
            })
            .collect();

        // Sort by priority (highest first)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    fn count_tokens(&self, text: &str) -> usize {
        match cl100k_base() {
            Ok(bpe) => bpe.encode_with_special_tokens(text).len(),
            Err(_) => text.len() / 4, // Fallback: ~4 chars per token
        }
    }

    /// Generate a safe refactoring order (leaf modules first, working up to core modules).
    /// This allows an agent to refactor bottom-up without breaking dependents.
    fn refactoring_order<'a>(
        &self,
        modules: &'a [Module],
        graph: &DependencyGraph,
    ) -> Vec<&'a Module> {
        // Get reverse topological order (dependents before dependencies = leaves first)
        let topo = graph.topological_order_with_cycles();

        // Reverse it: we want leaves first (modules with no dependents)
        let reversed: Vec<_> = topo.into_iter().rev().collect();

        reversed
            .iter()
            .filter_map(|path| modules.iter().find(|m| &m.path == path))
            .collect()
    }

    /// Generate file-level recommendations based on issues
    fn file_recommendations(
        &self,
        module: &Module,
        issues: &[Issue],
        graph: &DependencyGraph,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();
        let path = &module.path;

        for issue in issues {
            let affects_this_module = issue.locations.iter().any(|loc| &loc.path == path);
            if !affects_this_module {
                continue;
            }

            match &issue.kind {
                IssueKind::GodObject => {
                    // Analyze what could be extracted
                    let struct_count = module
                        .definitions
                        .iter()
                        .filter(|d| d.kind == DefinitionKind::Struct)
                        .count();
                    let fn_count = module
                        .definitions
                        .iter()
                        .filter(|d| d.kind == DefinitionKind::Function)
                        .count();

                    if struct_count > 3 {
                        recommendations.push(format!(
                            "EXTRACT: This file has {} structs. Consider extracting related structs into separate modules (e.g., `{}_types.rs`).",
                            struct_count,
                            module.name
                        ));
                    }
                    if fn_count > 10 {
                        recommendations.push(format!(
                            "EXTRACT: This file has {} functions. Group related functions into separate modules by domain.",
                            fn_count
                        ));
                    }
                }
                IssueKind::HighCoupling => {
                    let fan_in = graph.fan_in(path);
                    recommendations.push(format!(
                        "INTERFACE: {} modules depend on this. Consider defining a trait/interface to reduce direct coupling.",
                        fan_in
                    ));
                }
                IssueKind::LowCohesion { score } => {
                    // Identify what external dependencies are used
                    let external: Vec<_> = module
                        .imports
                        .iter()
                        .filter(|i| {
                            !i.starts_with("crate::")
                                && !i.starts_with("super::")
                                && !i.starts_with("self::")
                        })
                        .take(3)
                        .collect();

                    if !external.is_empty() {
                        recommendations.push(format!(
                            "FOCUS: Cohesion score {:.2}. This module mixes concerns. Primary external deps: {}. Consider splitting by responsibility.",
                            score,
                            external.iter().map(|s| s.split("::").next().unwrap_or(s)).collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                IssueKind::BoundaryViolation { boundary_name } => {
                    let violation_count = issue
                        .locations
                        .iter()
                        .filter(|loc| &loc.path == path)
                        .count();

                    if violation_count > 0 {
                        recommendations.push(format!(
                            "CENTRALIZE: {} {} boundary crossings. Extract to a dedicated service/repository module.",
                            violation_count,
                            boundary_name
                        ));
                    }
                }
                IssueKind::CircularDependency => {
                    recommendations.push(
                        "DECOUPLE: Part of a circular dependency. Extract shared types to a separate module, or use dependency injection.".to_string()
                    );
                }
                IssueKind::DeepDependencyChain { depth } => {
                    recommendations.push(format!(
                        "FLATTEN: Part of a {}-deep dependency chain. Consider introducing a facade or flattening the hierarchy.",
                        depth
                    ));
                }
                IssueKind::FatModule {
                    private_functions,
                    public_functions,
                } => {
                    recommendations.push(format!(
                        "EXTRACT: {} private functions vs {} public. This module has hidden complexity. \
                        Group related private functions into submodules.",
                        private_functions, public_functions
                    ));
                }
            }
        }

        recommendations
    }

    fn format_module_signature(&self, module: &Module) -> String {
        let mut output = String::new();

        // Filter to public definitions only
        let public_defs: Vec<_> = module
            .definitions
            .iter()
            .filter(|d| d.visibility == Visibility::Public)
            .collect();

        if public_defs.is_empty() && module.imports.is_empty() {
            return output;
        }

        // Add imports
        for import in &module.imports {
            output.push_str(&format!("use {};\n", import));
        }
        if !module.imports.is_empty() {
            output.push('\n');
        }

        // Add public signatures
        for def in public_defs {
            if let Some(ref sig) = def.signature {
                // For functions, show just the signature
                if def.kind == DefinitionKind::Function {
                    output.push_str(sig);
                    output.push_str(" { ... }\n\n");
                } else {
                    // For structs/enums/traits, show full definition
                    output.push_str(sig);
                    output.push_str("\n\n");
                }
            }
        }

        output
    }

    fn format_module_full(&self, module: &Module) -> String {
        if let Some(source) = self.sources.get(&module.path) {
            source.clone()
        } else {
            self.format_module_signature(module)
        }
    }
}

impl OutputFormatter for AiOutput {
    fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        match self.format {
            AiOutputFormat::Markdown => self.format_markdown(result, writer),
            AiOutputFormat::Json => self.format_json(result, writer),
            AiOutputFormat::Xml => self.format_xml(result, writer),
        }
    }
}

impl AiOutput {
    fn format_markdown<W: Write>(
        &self,
        result: &AnalysisResult,
        writer: &mut W,
    ) -> std::io::Result<()> {
        let graph = DependencyGraph::build(&result.modules);

        writeln!(writer, "# Architectural Context: {}\n", result.project_name)?;

        if let Some(budget) = self.token_budget {
            self.format_with_budget(result, writer, &graph, budget)?;
        } else {
            // Order modules
            let ordered = self.order_modules(&result.modules, &graph);

            writeln!(writer, "## Modules ({})\n", ordered.len())?;

            // Build output content and track tokens
            let mut content = String::new();

            for module in &ordered {
                let rel_path = self.relative_path(&module.path);
                content.push_str(&format!("### `{}`\n\n", rel_path));

                if self.signatures_only {
                    let sig = self.format_module_signature(module);
                    if !sig.is_empty() {
                        content.push_str(&format!("```rust\n{}```\n\n", sig));
                    } else {
                        content.push_str("*No public API*\n\n");
                    }
                } else {
                    content.push_str(&format!("- Lines: {}\n", module.lines));
                    content.push_str(&format!("- Imports: {}\n", module.imports.len()));
                    if !module.exports.is_empty() {
                        content.push_str(&format!("- Exports: {}\n", module.exports.join(", ")));
                    }
                    content.push('\n');
                }
            }

            // Write the content
            write!(writer, "{}", content)?;

            // Calculate and display token count
            let total_tokens = self.count_tokens(&format!(
                "# Architectural Context: {}\n\n## Modules ({})\n\n{}",
                result.project_name,
                ordered.len(),
                content
            ));
            writeln!(writer, "---\n*Context size: ~{} tokens*", total_tokens)?;
        }

        Ok(())
    }

    fn format_with_budget<W: Write>(
        &self,
        result: &AnalysisResult,
        writer: &mut W,
        graph: &DependencyGraph,
        budget: usize,
    ) -> std::io::Result<()> {
        let prioritized = self.prioritize_modules(&result.modules, graph);

        // Reserve tokens for structure and metadata sections
        let structure_reserve = 800;
        let available = budget.saturating_sub(structure_reserve);

        let mut used_tokens = 0;
        let mut included = Vec::new();
        let mut truncated = Vec::new();
        let mut omitted = Vec::new();

        for (module, score) in prioritized {
            // Get content based on mode
            let content = if self.signatures_only {
                self.format_module_signature(module)
            } else {
                self.format_module_full(module)
            };

            let tokens = self.count_tokens(&content);

            if used_tokens + tokens <= available {
                included.push((module, score, content, tokens));
                used_tokens += tokens;
            } else if !content.is_empty() {
                // Try minimal (imports only)
                let minimal = format!(
                    "// {}\n{}",
                    module.name,
                    module
                        .imports
                        .iter()
                        .map(|i| format!("use {};", i))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                let minimal_tokens = self.count_tokens(&minimal);

                if used_tokens + minimal_tokens <= available {
                    truncated.push((module, score, minimal, minimal_tokens));
                    used_tokens += minimal_tokens;
                } else {
                    omitted.push(module);
                }
            }
        }

        // Output with budget info
        writeln!(
            writer,
            "## Token Budget: {}/{}\n",
            used_tokens + structure_reserve,
            budget
        )?;

        // Add refactoring order section
        let refactor_order = self.refactoring_order(&result.modules, graph);
        writeln!(writer, "## Suggested Refactoring Order\n")?;
        writeln!(
            writer,
            "Modules listed leaf-first (safest to modify first, fewest dependents):\n"
        )?;
        for (i, module) in refactor_order.iter().take(15).enumerate() {
            let fan_in = graph.fan_in(&module.path);
            let rel_path = self.relative_path(&module.path);
            writeln!(writer, "{}. `{}` ({} dependents)", i + 1, rel_path, fan_in)?;
        }
        if refactor_order.len() > 15 {
            writeln!(
                writer,
                "... and {} more modules\n",
                refactor_order.len() - 15
            )?;
        }
        writeln!(writer)?;

        // Add actionable recommendations section
        let modules_with_issues: Vec<_> = result
            .modules
            .iter()
            .filter_map(|m| {
                let recs = self.file_recommendations(m, &result.issues, graph);
                if recs.is_empty() {
                    None
                } else {
                    Some((m, recs))
                }
            })
            .collect();

        if !modules_with_issues.is_empty() {
            writeln!(writer, "## Actionable Recommendations\n")?;
            for (module, recs) in modules_with_issues.iter().take(10) {
                let rel_path = self.relative_path(&module.path);
                writeln!(writer, "### `{}`\n", rel_path)?;
                for rec in recs {
                    writeln!(writer, "- {}", rec)?;
                }
                writeln!(writer)?;
            }
        }

        writeln!(writer, "## Included Modules ({})\n", included.len())?;

        for (module, score, content, _tokens) in &included {
            let rel_path = self.relative_path(&module.path);
            writeln!(writer, "### `{}` (priority: {:.1})\n", rel_path, score)?;
            writeln!(writer, "```rust\n{}\n```\n", content.trim())?;
        }

        if !truncated.is_empty() {
            writeln!(writer, "## Truncated Modules ({})\n", truncated.len())?;
            for (module, _score, content, _tokens) in &truncated {
                let rel_path = self.relative_path(&module.path);
                writeln!(writer, "### `{}` (imports only)\n", rel_path)?;
                writeln!(writer, "```rust\n{}\n```\n", content.trim())?;
            }
        }

        if !omitted.is_empty() {
            writeln!(writer, "## Omitted Modules ({})\n", omitted.len())?;
            for module in omitted {
                writeln!(writer, "- `{}`", self.relative_path(&module.path))?;
            }
        }

        Ok(())
    }

    fn format_json<W: Write>(
        &self,
        result: &AnalysisResult,
        writer: &mut W,
    ) -> std::io::Result<()> {
        use serde_json::json;

        let graph = DependencyGraph::build(&result.modules);
        let ordered = self.order_modules(&result.modules, &graph);

        // Build refactoring order
        let refactor_order: Vec<_> = self
            .refactoring_order(&result.modules, &graph)
            .iter()
            .map(|m| {
                json!({
                    "path": self.relative_path(&m.path),
                    "dependents": graph.fan_in(&m.path)
                })
            })
            .collect();

        // Build recommendations
        let recommendations: Vec<_> = result
            .modules
            .iter()
            .filter_map(|m| {
                let recs = self.file_recommendations(m, &result.issues, &graph);
                if recs.is_empty() {
                    None
                } else {
                    Some(json!({
                        "path": self.relative_path(&m.path),
                        "actions": recs
                    }))
                }
            })
            .collect();

        let modules_json: Vec<_> = ordered
            .iter()
            .map(|m| {
                let sig = self.format_module_signature(m);
                let public_defs: Vec<_> = m
                    .definitions
                    .iter()
                    .filter(|d| d.visibility == Visibility::Public)
                    .map(|d| {
                        json!({
                            "name": d.name,
                            "kind": format!("{:?}", d.kind),
                            "line": d.line,
                            "signature": d.signature
                        })
                    })
                    .collect();

                json!({
                    "path": self.relative_path(&m.path),
                    "name": m.name,
                    "lines": m.lines,
                    "imports": m.imports,
                    "exports": m.exports,
                    "definitions": public_defs,
                    "signature": sig
                })
            })
            .collect();

        let output = json!({
            "project": result.project_name,
            "ordering": if self.topo_order { "topological" } else { "filesystem" },
            "refactoring_order": refactor_order,
            "recommendations": recommendations,
            "modules": modules_json
        });

        let json_str = serde_json::to_string_pretty(&output)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writeln!(writer, "{}", json_str)
    }

    fn format_xml<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        let graph = DependencyGraph::build(&result.modules);
        let ordered = self.order_modules(&result.modules, &graph);

        writeln!(
            writer,
            "<architectural_context project=\"{}\">",
            escape_xml(&result.project_name)
        )?;

        // Refactoring order section - critical for agents
        writeln!(
            writer,
            "  <refactoring_order description=\"Modules listed leaf-first, safest to modify first\">"
        )?;
        for (i, module) in self
            .refactoring_order(&result.modules, &graph)
            .iter()
            .enumerate()
        {
            let rel_path = self.relative_path(&module.path);
            let fan_in = graph.fan_in(&module.path);
            writeln!(
                writer,
                "    <step order=\"{}\" path=\"{}\" dependents=\"{}\"/>",
                i + 1,
                escape_xml(&rel_path),
                fan_in
            )?;
        }
        writeln!(writer, "  </refactoring_order>")?;

        // Actionable recommendations section
        let modules_with_issues: Vec<_> = result
            .modules
            .iter()
            .filter_map(|m| {
                let recs = self.file_recommendations(m, &result.issues, &graph);
                if recs.is_empty() {
                    None
                } else {
                    Some((m, recs))
                }
            })
            .collect();

        if !modules_with_issues.is_empty() {
            writeln!(writer, "  <recommendations>")?;
            for (module, recs) in &modules_with_issues {
                let rel_path = self.relative_path(&module.path);
                writeln!(writer, "    <file path=\"{}\">", escape_xml(&rel_path))?;
                for rec in recs {
                    // Parse the action type from the recommendation
                    let (action_type, description) = if let Some(idx) = rec.find(':') {
                        (&rec[..idx], rec[idx + 1..].trim())
                    } else {
                        ("REFACTOR", rec.as_str())
                    };
                    writeln!(
                        writer,
                        "      <action type=\"{}\">{}</action>",
                        action_type,
                        escape_xml(description)
                    )?;
                }
                writeln!(writer, "    </file>")?;
            }
            writeln!(writer, "  </recommendations>")?;
        }

        // Modules section
        writeln!(writer, "  <modules count=\"{}\">", ordered.len())?;
        for module in ordered {
            let rel_path = self.relative_path(&module.path);
            let fan_in = graph.fan_in(&module.path);
            let fan_out = graph.fan_out(&module.path);

            writeln!(
                writer,
                "    <module path=\"{}\" name=\"{}\" lines=\"{}\" fan_in=\"{}\" fan_out=\"{}\">",
                escape_xml(&rel_path),
                escape_xml(&module.name),
                module.lines,
                fan_in,
                fan_out
            )?;

            if !module.imports.is_empty() {
                writeln!(writer, "      <imports>")?;
                for import in &module.imports {
                    writeln!(writer, "        <import>{}</import>", escape_xml(import))?;
                }
                writeln!(writer, "      </imports>")?;
            }

            if !module.exports.is_empty() {
                writeln!(writer, "      <exports>")?;
                for export in &module.exports {
                    writeln!(writer, "        <export>{}</export>", escape_xml(export))?;
                }
                writeln!(writer, "      </exports>")?;
            }

            // Public definitions
            let public_defs: Vec<_> = module
                .definitions
                .iter()
                .filter(|d| d.visibility == Visibility::Public)
                .collect();

            if !public_defs.is_empty() {
                writeln!(writer, "      <definitions>")?;
                for def in public_defs {
                    let kind = format!("{:?}", def.kind).to_lowercase();
                    writeln!(
                        writer,
                        "        <{} name=\"{}\" line=\"{}\">",
                        kind,
                        escape_xml(&def.name),
                        def.line
                    )?;
                    if let Some(ref sig) = def.signature {
                        writeln!(writer, "<![CDATA[{}]]>", sig)?;
                    }
                    writeln!(writer, "        </{}>", kind)?;
                }
                writeln!(writer, "      </definitions>")?;
            }

            writeln!(writer, "    </module>")?;
        }
        writeln!(writer, "  </modules>")?;

        writeln!(writer, "</architectural_context>")
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
