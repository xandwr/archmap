use crate::model::{AnalysisResult, IssueKind, IssueSeverity};
use crate::output::OutputFormatter;
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
        if let Some(ref root) = self.project_root {
            path.strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string()
        } else {
            path.display().to_string()
        }
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
                    writeln!(
                        writer,
                        "- `{}` - {}",
                        self.relative_path(&loc.path),
                        issue.message
                    )?;
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
                    writeln!(
                        writer,
                        "- `{}` - {}",
                        self.relative_path(&loc.path),
                        issue.message
                    )?;
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

        // Deep Dependency Chains
        let deep_chains: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::DeepDependencyChain { .. }))
            .collect();

        if !deep_chains.is_empty() {
            writeln!(writer, "### 🟡 Deep Dependency Chains\n")?;
            for issue in deep_chains {
                writeln!(writer, "- {}", issue.message)?;
                if let Some(ref suggestion) = issue.suggestion {
                    writeln!(writer, "  → {}", suggestion)?;
                }
            }
            writeln!(writer)?;
        }

        // Low Cohesion
        let low_cohesion: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::LowCohesion { .. }))
            .collect();

        if !low_cohesion.is_empty() {
            writeln!(writer, "### 🔵 Low Cohesion Modules\n")?;
            for issue in low_cohesion {
                if let Some(loc) = issue.locations.first() {
                    writeln!(
                        writer,
                        "- `{}` - {}",
                        self.relative_path(&loc.path),
                        issue.message
                    )?;
                }
                if let Some(ref suggestion) = issue.suggestion {
                    writeln!(writer, "  → {}", suggestion)?;
                }
            }
            writeln!(writer)?;
        }

        // Fat Modules
        let fat_modules: Vec<_> = filtered_issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::FatModule { .. }))
            .collect();

        if !fat_modules.is_empty() {
            writeln!(writer, "### 🔵 Fat Modules (Hidden Complexity)\n")?;
            for issue in fat_modules {
                if let Some(loc) = issue.locations.first() {
                    writeln!(
                        writer,
                        "- `{}` - {}",
                        self.relative_path(&loc.path),
                        issue.message
                    )?;
                }
                if let Some(ref suggestion) = issue.suggestion {
                    writeln!(writer, "  → {}", suggestion)?;
                }
            }
            writeln!(writer)?;
        }

        Ok(())
    }
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len { s } else { &s[..max_len] }
}
