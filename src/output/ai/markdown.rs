use super::AiContext;
use crate::analysis::DependencyGraph;
use crate::model::AnalysisResult;
use std::io::Write;

pub struct MarkdownFormatter {
    ctx: AiContext,
}

impl MarkdownFormatter {
    pub fn new(ctx: AiContext) -> Self {
        Self { ctx }
    }

    pub fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        let graph = DependencyGraph::build(&result.modules);

        writeln!(writer, "# Architectural Context: {}\n", result.project_name)?;

        if let Some(budget) = self.ctx.token_budget {
            self.format_with_budget(result, writer, &graph, budget)?;
        } else {
            let ordered = self.ctx.order_modules(&result.modules, &graph);

            writeln!(writer, "## Modules ({})\n", ordered.len())?;

            let mut content = String::new();

            for module in &ordered {
                let rel_path = self.ctx.relative_path(&module.path);
                content.push_str(&format!("### `{}`\n\n", rel_path));

                if self.ctx.signatures_only {
                    let sig = self.ctx.format_module_signature(module);
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

            write!(writer, "{}", content)?;

            let total_tokens = self.ctx.count_tokens(&format!(
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
        let prioritized = self.ctx.prioritize_modules(&result.modules, graph);

        let structure_reserve = 800;
        let available = budget.saturating_sub(structure_reserve);

        let mut used_tokens = 0;
        let mut included = Vec::new();
        let mut truncated = Vec::new();
        let mut omitted = Vec::new();

        for (module, score) in prioritized {
            let content = if self.ctx.signatures_only {
                self.ctx.format_module_signature(module)
            } else {
                self.ctx.format_module_full(module)
            };

            let tokens = self.ctx.count_tokens(&content);

            if used_tokens + tokens <= available {
                included.push((module, score, content, tokens));
                used_tokens += tokens;
            } else if !content.is_empty() {
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
                let minimal_tokens = self.ctx.count_tokens(&minimal);

                if used_tokens + minimal_tokens <= available {
                    truncated.push((module, score, minimal, minimal_tokens));
                    used_tokens += minimal_tokens;
                } else {
                    omitted.push(module);
                }
            }
        }

        writeln!(
            writer,
            "## Token Budget: {}/{}\n",
            used_tokens + structure_reserve,
            budget
        )?;

        // Refactoring order section
        let refactor_order = self.ctx.refactoring_order(&result.modules, graph);
        writeln!(writer, "## Suggested Refactoring Order\n")?;
        writeln!(
            writer,
            "Modules listed leaf-first (safest to modify first, fewest dependents):\n"
        )?;
        for (i, module) in refactor_order.iter().take(15).enumerate() {
            let fan_in = graph.fan_in(&module.path);
            let rel_path = self.ctx.relative_path(&module.path);
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

        // Actionable recommendations section
        let modules_with_issues: Vec<_> = result
            .modules
            .iter()
            .filter_map(|m| {
                let recs = self.ctx.file_recommendations(m, &result.issues, graph);
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
                let rel_path = self.ctx.relative_path(&module.path);
                writeln!(writer, "### `{}`\n", rel_path)?;
                for rec in recs {
                    writeln!(writer, "- {}", rec)?;
                }
                writeln!(writer)?;
            }
        }

        writeln!(writer, "## Included Modules ({})\n", included.len())?;

        for (module, score, content, _tokens) in &included {
            let rel_path = self.ctx.relative_path(&module.path);
            writeln!(writer, "### `{}` (priority: {:.1})\n", rel_path, score)?;
            writeln!(writer, "```rust\n{}\n```\n", content.trim())?;
        }

        if !truncated.is_empty() {
            writeln!(writer, "## Truncated Modules ({})\n", truncated.len())?;
            for (module, _score, content, _tokens) in &truncated {
                let rel_path = self.ctx.relative_path(&module.path);
                writeln!(writer, "### `{}` (imports only)\n", rel_path)?;
                writeln!(writer, "```rust\n{}\n```\n", content.trim())?;
            }
        }

        if !omitted.is_empty() {
            writeln!(writer, "## Omitted Modules ({})\n", omitted.len())?;
            for module in omitted {
                writeln!(writer, "- `{}`", self.ctx.relative_path(&module.path))?;
            }
        }

        Ok(())
    }
}
