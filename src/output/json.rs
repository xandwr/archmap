use crate::model::AnalysisResult;
use crate::output::OutputFormatter;
use serde::Serialize;
use std::io::Write;

pub struct JsonOutput;

impl JsonOutput {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct JsonResult<'a> {
    project_name: &'a str,
    modules: Vec<JsonModule<'a>>,
    issues: Vec<JsonIssue<'a>>,
}

#[derive(Serialize)]
struct JsonModule<'a> {
    path: String,
    name: &'a str,
    lines: usize,
    imports: &'a [String],
    exports: &'a [String],
}

#[derive(Serialize)]
struct JsonIssue<'a> {
    kind: String,
    severity: String,
    message: &'a str,
    locations: Vec<JsonLocation<'a>>,
    suggestion: Option<&'a str>,
}

#[derive(Serialize)]
struct JsonLocation<'a> {
    path: String,
    line: Option<usize>,
    context: Option<&'a str>,
}

impl OutputFormatter for JsonOutput {
    fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        let json_result = JsonResult {
            project_name: &result.project_name,
            modules: result
                .modules
                .iter()
                .map(|m| JsonModule {
                    path: m.path.display().to_string(),
                    name: &m.name,
                    lines: m.lines,
                    imports: &m.imports,
                    exports: &m.exports,
                })
                .collect(),
            issues: result
                .issues
                .iter()
                .map(|i| JsonIssue {
                    kind: format!("{:?}", i.kind),
                    severity: i.severity.to_string(),
                    message: &i.message,
                    locations: i
                        .locations
                        .iter()
                        .map(|l| JsonLocation {
                            path: l.path.display().to_string(),
                            line: l.line,
                            context: l.context.as_deref(),
                        })
                        .collect(),
                    suggestion: i.suggestion.as_deref(),
                })
                .collect(),
        };

        let json = serde_json::to_string_pretty(&json_result)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        writeln!(writer, "{}", json)
    }
}
