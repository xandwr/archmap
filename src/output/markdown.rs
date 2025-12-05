use crate::model::{AnalysisResult, IssueKind, IssueSeverity};
use crate::output::OutputFormatter;
use std::io::Write;

pub struct MarkdownOutput {
    pub min_severity: IssueSeverity,
}

impl MarkdownOutput {
    pub fn new(min_severity: IssueSeverity) -> Self {
        Self { min_severity }
    }
}

impl OutputFormatter for MarkdownOutput {
    fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer, "# Architecture Analysis: {}\n", result.project_name)?;

        // Module Graph
        writeln!(writer, "## Module Graph\n")?;
        for module in &result.modules {
            let imports: Vec<_> = module
                .imports
                .iter()
                .map(|i| {
                    // Shorten to first segment
                    i.split("::").next().unwrap_or(i)
                })
                .collect();

            if imports.is_empty() {
                writeln!(writer, "- `{}` (no imports)", module.path.display())?;
            } else {
                writeln!(
                    writer,
                    "- `{}` → imports: [{}]",
                    module.path.display(),
                    imports.join(", ")
                )?;
            }
        }

        // Filter and group issues
        let filtered_issues: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity >= self.min_severity)
            .collect();

        if filtered_issues.is_empty() {
            writeln!(writer, "\n## No Issues Found\n")?;
            writeln!(writer, "No architectural issues detected.")?;
            return Ok(());
        }

        writeln!(writer, "\n## Issues Found\n")?;

        // Circular Dependencies (Error severity)
        let circular: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::CircularDependency))
            .collect();

        if !circular.is_empty() {
            writeln!(writer, "### 🔴 Circular Dependencies\n")?;
            for issue in circular {
                writeln!(writer, "- {}", issue.message)?;
                if let Some(ref suggestion) = issue.suggestion {
                    writeln!(writer, "  → {}", suggestion)?;
                }
            }
            writeln!(writer)?;
        }

        // God Objects
        let god_objects: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::GodObject))
            .collect();

        if !god_objects.is_empty() {
            writeln!(writer, "### 🟡 God Objects\n")?;
            for issue in god_objects {
                if let Some(loc) = issue.locations.first() {
                    writeln!(writer, "- `{}` - {}", loc.path.display(), issue.message)?;
                }
            }
            writeln!(writer)?;
        }

        // High Coupling
        let coupling: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::HighCoupling))
            .collect();

        if !coupling.is_empty() {
            writeln!(writer, "### 🟡 High Coupling\n")?;
            for issue in coupling {
                if let Some(loc) = issue.locations.first() {
                    writeln!(writer, "- `{}` - {}", loc.path.display(), issue.message)?;
                }
            }
            writeln!(writer)?;
        }

        // Boundary Violations
        let boundary_violations: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::BoundaryViolation { .. }))
            .collect();

        if !boundary_violations.is_empty() {
            writeln!(writer, "### 🟡 Boundary Violations\n")?;
            for issue in boundary_violations {
                if let IssueKind::BoundaryViolation { ref boundary_name } = issue.kind {
                    writeln!(
                        writer,
                        "**{}** crossed in {} locations:",
                        boundary_name,
                        issue.locations.len()
                    )?;

                    // Show first few locations
                    for loc in issue.locations.iter().take(5) {
                        let line_info = loc.line.map(|l| format!(":{}", l)).unwrap_or_default();
                        let context = loc
                            .context
                            .as_ref()
                            .map(|c| format!(" - `{}`", truncate(c, 50)))
                            .unwrap_or_default();

                        writeln!(writer, "- `{}{}`{}", loc.path.display(), line_info, context)?;
                    }

                    if issue.locations.len() > 5 {
                        writeln!(writer, "- ... and {} more", issue.locations.len() - 5)?;
                    }

                    if let Some(ref suggestion) = issue.suggestion {
                        writeln!(writer, "\n→ {}\n", suggestion)?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len { s } else { &s[..max_len] }
}
