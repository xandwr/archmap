use crate::model::{AnalysisResult, Issue, IssueKind, IssueSeverity};
use crate::output::{OutputFormatter, relative_path};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct MarkdownOutput {
    pub min_severity: IssueSeverity,
    pub project_root: Option<PathBuf>,
}

impl MarkdownOutput {
    pub fn new(min_severity: IssueSeverity, project_root: Option<PathBuf>) -> Self {
        Self {
            min_severity,
            project_root,
        }
    }

    fn relative_path(&self, path: &Path) -> String {
        relative_path(path, self.project_root.as_ref())
    }

    /// Write a section with issues that show message and optional suggestion (no location).
    fn write_message_section<W: Write>(
        &self,
        writer: &mut W,
        header: &str,
        issues: &[&&Issue],
    ) -> std::io::Result<()> {
        if issues.is_empty() {
            return Ok(());
        }
        writeln!(writer, "{}\n", header)?;
        for issue in issues {
            writeln!(writer, "- {}", issue.message)?;
            if let Some(ref suggestion) = issue.suggestion {
                writeln!(writer, "  → {}", suggestion)?;
            }
        }
        writeln!(writer)
    }

    /// Write a section with issues that show location path and message.
    fn write_location_section<W: Write>(
        &self,
        writer: &mut W,
        header: &str,
        issues: &[&&Issue],
        include_suggestion: bool,
    ) -> std::io::Result<()> {
        if issues.is_empty() {
            return Ok(());
        }
        writeln!(writer, "{}\n", header)?;
        for issue in issues {
            if let Some(loc) = issue.locations.first() {
                writeln!(
                    writer,
                    "- `{}` - {}",
                    self.relative_path(&loc.path),
                    issue.message
                )?;
            }
            if include_suggestion {
                if let Some(ref suggestion) = issue.suggestion {
                    writeln!(writer, "  → {}", suggestion)?;
                }
            }
        }
        writeln!(writer)
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
                    // Shorten to first segment and wrap in backticks
                    let short = i.split("::").next().unwrap_or(i);
                    format!("`{}`", short)
                })
                .collect();

            let rel_path = self.relative_path(&module.path);
            if imports.is_empty() {
                writeln!(writer, "- `{}` (no imports)", rel_path)?;
            } else {
                writeln!(
                    writer,
                    "- `{}` → imports: [{}]",
                    rel_path,
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

        // Circular Dependencies (Error severity) - message only
        let circular: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::CircularDependency))
            .collect();
        self.write_message_section(writer, "### 🔴 Circular Dependencies", &circular)?;

        // God Objects - location + message, no suggestion
        let god_objects: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::GodObject))
            .collect();
        self.write_location_section(writer, "### 🟡 God Objects", &god_objects, false)?;

        // High Coupling - location + message, no suggestion
        let coupling: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::HighCoupling))
            .collect();
        self.write_location_section(writer, "### 🟡 High Coupling", &coupling, false)?;

        // Boundary Violations - special formatting (keep inline)
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

                    for loc in issue.locations.iter().take(5) {
                        let line_info = loc.line.map(|l| format!(":{}", l)).unwrap_or_default();
                        let context = loc
                            .context
                            .as_ref()
                            .map(|c| format!(" - `{}`", truncate(c, 50)))
                            .unwrap_or_default();

                        writeln!(
                            writer,
                            "- `{}{}`{}",
                            self.relative_path(&loc.path),
                            line_info,
                            context
                        )?;
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

        // Deep Dependency Chains - message only
        let deep_chains: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::DeepDependencyChain { .. }))
            .collect();
        self.write_message_section(writer, "### 🟡 Deep Dependency Chains", &deep_chains)?;

        // Low Cohesion - location + message + suggestion
        let low_cohesion: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::LowCohesion { .. }))
            .collect();
        self.write_location_section(writer, "### 🔵 Low Cohesion Modules", &low_cohesion, true)?;

        // Fat Modules - location + message + suggestion
        let fat_modules: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::FatModule { .. }))
            .collect();
        self.write_location_section(
            writer,
            "### 🔵 Fat Modules (Hidden Complexity)",
            &fat_modules,
            true,
        )?;

        Ok(())
    }
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len { s } else { &s[..max_len] }
}
