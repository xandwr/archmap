use super::AiContext;
use crate::analysis::DependencyGraph;
use crate::model::{AnalysisResult, Visibility};
use serde_json::json;
use std::io::Write;

pub struct JsonFormatter {
    ctx: AiContext,
}

impl JsonFormatter {
    pub fn new(ctx: AiContext) -> Self {
        Self { ctx }
    }

    pub fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        let graph = DependencyGraph::build(&result.modules);
        let ordered = self.ctx.order_modules(&result.modules, &graph);

        // Build refactoring order
        let refactor_order: Vec<_> = self
            .ctx
            .refactoring_order(&result.modules, &graph)
            .iter()
            .map(|m| {
                json!({
                    "path": self.ctx.relative_path(&m.path),
                    "dependents": graph.fan_in(&m.path)
                })
            })
            .collect();

        // Build recommendations
        let recommendations: Vec<_> = result
            .modules
            .iter()
            .filter_map(|m| {
                let recs = self.ctx.file_recommendations(m, &result.issues, &graph);
                if recs.is_empty() {
                    None
                } else {
                    Some(json!({
                        "path": self.ctx.relative_path(&m.path),
                        "actions": recs
                    }))
                }
            })
            .collect();

        let modules_json: Vec<_> = ordered
            .iter()
            .map(|m| {
                let sig = self.ctx.format_module_signature(m);
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
                    "path": self.ctx.relative_path(&m.path),
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
            "ordering": if self.ctx.topo_order { "topological" } else { "filesystem" },
            "refactoring_order": refactor_order,
            "recommendations": recommendations,
            "modules": modules_json
        });

        let json_str = serde_json::to_string_pretty(&output)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writeln!(writer, "{}", json_str)
    }
}
