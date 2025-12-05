use crate::analysis::DependencyGraph;
use crate::model::{DefinitionKind, Issue, IssueKind, Module, Visibility};
use crate::output::relative_path;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tiktoken_rs::cl100k_base;

/// Shared context and helper methods for AI output formatters
pub struct AiContext {
    pub project_root: Option<PathBuf>,
    pub topo_order: bool,
    pub signatures_only: bool,
    pub token_budget: Option<usize>,
    pub sources: HashMap<PathBuf, String>,
}

impl AiContext {
    pub fn relative_path(&self, path: &Path) -> String {
        relative_path(path, self.project_root.as_ref())
    }

    pub fn order_modules<'a>(
        &self,
        modules: &'a [Module],
        graph: &DependencyGraph,
    ) -> Vec<&'a Module> {
        if self.topo_order {
            let order = graph.topological_order_with_cycles();
            order
                .iter()
                .filter_map(|path| modules.iter().find(|m| &m.path == path))
                .collect()
        } else {
            modules.iter().collect()
        }
    }

    pub fn prioritize_modules<'a>(
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

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        match cl100k_base() {
            Ok(bpe) => bpe.encode_with_special_tokens(text).len(),
            Err(_) => text.len() / 4,
        }
    }

    /// Generate a safe refactoring order (leaf modules first, working up to core modules).
    pub fn refactoring_order<'a>(
        &self,
        modules: &'a [Module],
        graph: &DependencyGraph,
    ) -> Vec<&'a Module> {
        let topo = graph.topological_order_with_cycles();
        let reversed: Vec<_> = topo.into_iter().rev().collect();

        reversed
            .iter()
            .filter_map(|path| modules.iter().find(|m| &m.path == path))
            .collect()
    }

    /// Generate file-level recommendations based on issues
    pub fn file_recommendations(
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

    pub fn format_module_signature(&self, module: &Module) -> String {
        let mut output = String::new();

        let public_defs: Vec<_> = module
            .definitions
            .iter()
            .filter(|d| d.visibility == Visibility::Public)
            .collect();

        if public_defs.is_empty() && module.imports.is_empty() {
            return output;
        }

        for import in &module.imports {
            output.push_str(&format!("use {};\n", import));
        }
        if !module.imports.is_empty() {
            output.push('\n');
        }

        for def in public_defs {
            if let Some(ref sig) = def.signature {
                if def.kind == DefinitionKind::Function {
                    output.push_str(sig);
                    output.push_str(" { ... }\n\n");
                } else {
                    output.push_str(sig);
                    output.push_str("\n\n");
                }
            }
        }

        output
    }

    pub fn format_module_full(&self, module: &Module) -> String {
        if let Some(source) = self.sources.get(&module.path) {
            source.clone()
        } else {
            self.format_module_signature(module)
        }
    }
}
