use crate::cli::McpArgs;
use crate::config::Config;
use crate::output::{JsonOutput, OutputFormatter};
use crate::parser::ParserRegistry;
use rmcp::handler::server::tool::cached_schema_for_type;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParam, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

pub fn cmd_mcp(args: McpArgs) -> i32 {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(run_mcp_server(args))
}

async fn run_mcp_server(args: McpArgs) -> i32 {
    let working_dir = match args.path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to resolve working directory: {}", e);
            return 1;
        }
    };

    let service = ArchmapService::new(working_dir);

    let transport = rmcp::transport::io::stdio();

    let running_service = match service.serve(transport).await {
        Ok(service) => service,
        Err(e) => {
            eprintln!("MCP server error: {}", e);
            return 1;
        }
    };

    // Keep the server running until the transport closes
    if let Err(e) = running_service.waiting().await {
        eprintln!("MCP server task error: {}", e);
        return 1;
    }

    0
}

#[derive(Clone)]
struct ArchmapService {
    working_dir: Arc<PathBuf>,
}

impl ArchmapService {
    fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir: Arc::new(working_dir),
        }
    }

    fn analyze_impl(&self, path: Option<String>, format: Option<String>) -> Result<String, String> {
        let target_path = match &path {
            Some(p) => {
                let p = PathBuf::from(p);
                if p.is_absolute() {
                    p
                } else {
                    self.working_dir.join(p)
                }
            }
            None => self.working_dir.as_ref().clone(),
        };

        let target_path = target_path
            .canonicalize()
            .map_err(|e| format!("Failed to resolve path: {}", e))?;

        let config = Config::load(&target_path).unwrap_or_default();
        let registry = ParserRegistry::new();

        let result = crate::analysis::analyze(&target_path, &config, &registry, &[]);

        let output_format = format.as_deref().unwrap_or("json");

        let mut buffer = Vec::new();
        match output_format {
            "json" => {
                let formatter = JsonOutput::new(Some(target_path));
                formatter
                    .format(&result, &mut buffer)
                    .map_err(|e| format!("Failed to format output: {}", e))?;
            }
            "markdown" => {
                use crate::model::IssueSeverity;
                use crate::output::MarkdownOutput;
                let formatter = MarkdownOutput::new(IssueSeverity::Info, Some(target_path));
                formatter
                    .format(&result, &mut buffer)
                    .map_err(|e| format!("Failed to format output: {}", e))?;
            }
            _ => return Err(format!("Unknown format: {}", output_format)),
        }

        String::from_utf8(buffer).map_err(|e| format!("Invalid UTF-8 in output: {}", e))
    }

    fn ai_impl(
        &self,
        path: Option<String>,
        tokens: Option<usize>,
        signatures: Option<bool>,
        format: Option<String>,
    ) -> Result<String, String> {
        use crate::cli::{AiOutputFormat, PriorityStrategy};
        use crate::output::AiOutput;
        use std::collections::HashMap;

        let target_path = match &path {
            Some(p) => {
                let p = PathBuf::from(p);
                if p.is_absolute() {
                    p
                } else {
                    self.working_dir.join(p)
                }
            }
            None => self.working_dir.as_ref().clone(),
        };

        let target_path = target_path
            .canonicalize()
            .map_err(|e| format!("Failed to resolve path: {}", e))?;

        let config = Config::load(&target_path).unwrap_or_default();
        let registry = ParserRegistry::new();

        // Collect sources
        let mut sources = HashMap::new();
        let walker = ignore::WalkBuilder::new(&target_path)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            let file_path = entry.path();
            if file_path.is_file() && registry.find_parser(file_path).is_some() {
                if let Ok(content) = std::fs::read_to_string(file_path) {
                    sources.insert(file_path.to_path_buf(), content);
                }
            }
        }

        let result = crate::analysis::analyze(&target_path, &config, &registry, &[]);

        let output_format = match format.as_deref() {
            Some("xml") => AiOutputFormat::Xml,
            Some("markdown") => AiOutputFormat::Markdown,
            _ => AiOutputFormat::Json,
        };

        let mut formatter = AiOutput::new(Some(target_path))
            .with_topo_order(true)
            .with_signatures_only(signatures.unwrap_or(false))
            .with_priority(PriorityStrategy::FanIn)
            .with_format(output_format)
            .with_sources(sources);

        if let Some(t) = tokens {
            formatter = formatter.with_token_budget(t);
        }

        let mut buffer = Vec::new();
        OutputFormatter::format(&formatter, &result, &mut buffer)
            .map_err(|e| format!("Failed to format output: {}", e))?;

        String::from_utf8(buffer).map_err(|e| format!("Invalid UTF-8 in output: {}", e))
    }

    fn impact_impl(
        &self,
        file: String,
        path: Option<String>,
        depth: Option<usize>,
    ) -> Result<String, String> {
        let project_path = match &path {
            Some(p) => {
                let p = PathBuf::from(p);
                if p.is_absolute() {
                    p
                } else {
                    self.working_dir.join(p)
                }
            }
            None => self.working_dir.as_ref().clone(),
        };

        let project_path = project_path
            .canonicalize()
            .map_err(|e| format!("Failed to resolve path: {}", e))?;

        let file_path = {
            let p = PathBuf::from(&file);
            if p.is_absolute() {
                p
            } else {
                project_path.join(p)
            }
        };

        let file_path = file_path
            .canonicalize()
            .map_err(|e| format!("Failed to resolve file: {}", e))?;

        let config = Config::load(&project_path).unwrap_or_default();
        let registry = ParserRegistry::new();

        let result = crate::analysis::analyze(&project_path, &config, &registry, &[]);
        let graph = crate::analysis::DependencyGraph::build(&result.modules);

        let impact = crate::analysis::compute_impact(&graph, &file_path, depth)
            .map_err(|e| format!("{}", e))?;

        Ok(crate::analysis::format_impact_json(
            &impact,
            Some(&project_path),
        ))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AnalyzeParams {
    /// Path to analyze (defaults to working directory)
    path: Option<String>,
    /// Output format: "json" or "markdown"
    format: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AiParams {
    /// Path to analyze (defaults to working directory)
    path: Option<String>,
    /// Maximum tokens for output
    tokens: Option<usize>,
    /// Output only architectural signatures (public API surface)
    signatures: Option<bool>,
    /// Output format: "json", "markdown", or "xml"
    format: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ImpactParams {
    /// File to analyze for change impact (required)
    file: String,
    /// Project path (defaults to working directory)
    path: Option<String>,
    /// Maximum depth to traverse
    depth: Option<usize>,
}

impl ServerHandler for ArchmapService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "archmap".to_string(),
                title: Some("Archmap".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Archmap provides architectural analysis tools for codebases. \
                 Use 'analyze' for full analysis, 'ai' for AI-optimized output, \
                 and 'impact' to understand change blast radius."
                    .to_string(),
            ),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send {
        async move {
            Ok(ListToolsResult {
                tools: vec![
                    Tool::new(
                        "analyze",
                        "Run full architectural analysis with coupling metrics and issue detection",
                        cached_schema_for_type::<AnalyzeParams>(),
                    ),
                    Tool::new(
                        "ai",
                        "Generate AI-optimized compact context output for LLM consumption",
                        cached_schema_for_type::<AiParams>(),
                    ),
                    Tool::new(
                        "impact",
                        "Analyze change impact for a specific file - shows what depends on it",
                        cached_schema_for_type::<ImpactParams>(),
                    ),
                ],
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send {
        let this = self.clone();
        async move {
            let args_value = request
                .arguments
                .map(serde_json::Value::Object)
                .unwrap_or(serde_json::Value::Null);

            match request.name.as_ref() {
                "analyze" => {
                    let params: AnalyzeParams =
                        serde_json::from_value(args_value).unwrap_or(AnalyzeParams {
                            path: None,
                            format: None,
                        });

                    match this.analyze_impl(params.path, params.format) {
                        Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                        Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
                    }
                }
                "ai" => {
                    let params: AiParams = serde_json::from_value(args_value).unwrap_or(AiParams {
                        path: None,
                        tokens: None,
                        signatures: None,
                        format: None,
                    });

                    match this.ai_impl(params.path, params.tokens, params.signatures, params.format)
                    {
                        Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                        Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
                    }
                }
                "impact" => {
                    let params: ImpactParams = serde_json::from_value(args_value).map_err(|e| {
                        McpError::invalid_params(format!("Invalid parameters: {}", e), None)
                    })?;

                    match this.impact_impl(params.file, params.path, params.depth) {
                        Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                        Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
                    }
                }
                _ => Err(McpError::invalid_params(
                    format!("Unknown tool: {}", request.name),
                    None,
                )),
            }
        }
    }
}
