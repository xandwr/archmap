use super::AiContext;
use crate::analysis::DependencyGraph;
use crate::model::{AnalysisResult, Visibility};
use std::io::Write;

pub struct XmlFormatter {
    ctx: AiContext,
}

impl XmlFormatter {
    pub fn new(ctx: AiContext) -> Self {
        Self { ctx }
    }

    pub fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        let graph = DependencyGraph::build(&result.modules);
        let ordered = self.ctx.order_modules(&result.modules, &graph);

        writeln!(
            writer,
            "<architectural_context project=\"{}\">",
            escape_xml(&result.project_name)
        )?;

        // Refactoring order section
        writeln!(
            writer,
            "  <refactoring_order description=\"Modules listed leaf-first, safest to modify first\">"
        )?;
        for (i, module) in self
            .ctx
            .refactoring_order(&result.modules, &graph)
            .iter()
            .enumerate()
        {
            let rel_path = self.ctx.relative_path(&module.path);
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
                let recs = self.ctx.file_recommendations(m, &result.issues, &graph);
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
                let rel_path = self.ctx.relative_path(&module.path);
                writeln!(writer, "    <file path=\"{}\">", escape_xml(&rel_path))?;
                for rec in recs {
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
            let rel_path = self.ctx.relative_path(&module.path);
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
