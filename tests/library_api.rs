//! Integration tests for the archmap library API.

use archmap::{
    AiFormat, AiOptions, AnalysisOptions, ArchmapError, ImpactOptions, ai_context, analyze, impact,
};
use std::path::Path;

#[test]
fn test_analyze_current_directory() {
    let result = analyze(Path::new("."), AnalysisOptions::default()).unwrap();

    // We're analyzing archmap itself, so we should have modules
    assert!(!result.modules.is_empty(), "Should find modules");
    assert!(!result.project_name.is_empty(), "Should have project name");

    // Check that we found some expected modules
    let module_names: Vec<_> = result.modules.iter().map(|m| m.name.as_str()).collect();
    assert!(
        module_names.contains(&"lib"),
        "Should find lib.rs module: {:?}",
        module_names
    );
}

#[test]
fn test_analyze_with_options() {
    let options = AnalysisOptions {
        languages: vec!["rust".to_string()],
        exclude: vec!["tests".to_string()],
        max_depth: 10,
        min_cohesion: 0.2,
    };

    let result = analyze(Path::new("."), options).unwrap();
    assert!(!result.modules.is_empty());
}

#[test]
fn test_analyze_invalid_path() {
    let result = analyze(Path::new("/nonexistent/path"), AnalysisOptions::default());

    assert!(result.is_err());
    match result {
        Err(ArchmapError::PathNotFound(_)) => {}
        Err(e) => panic!("Expected PathNotFound error, got: {:?}", e),
        Ok(_) => panic!("Expected error for invalid path"),
    }
}

#[test]
fn test_impact_analysis() {
    // First analyze to ensure the project is indexed
    let analysis = analyze(Path::new("."), AnalysisOptions::default()).unwrap();

    // Find a module that should have dependents
    let model_module = analysis
        .modules
        .iter()
        .find(|m| m.path.ends_with("model/mod.rs"));

    if let Some(module) = model_module {
        let result = impact(Path::new("."), &module.path, ImpactOptions::default()).unwrap();

        // model/mod.rs is heavily depended upon
        assert!(
            result.total_affected() > 0,
            "model/mod.rs should have dependents"
        );
    }
}

#[test]
fn test_impact_with_depth_limit() {
    let options = ImpactOptions {
        languages: vec![],
        depth: Some(1),
    };

    let result = impact(Path::new("."), Path::new("src/model/mod.rs"), options).unwrap();

    // With depth limit of 1, we should only see direct dependents
    assert!(result.max_chain_length() <= 1);
}

#[test]
fn test_impact_invalid_file() {
    let result = impact(
        Path::new("."),
        Path::new("nonexistent.rs"),
        ImpactOptions::default(),
    );

    assert!(result.is_err());
}

#[test]
fn test_ai_context_markdown() {
    let options = AiOptions {
        format: AiFormat::Markdown,
        tokens: Some(1000),
        signatures_only: false,
        topo_order: true,
        ..Default::default()
    };

    let context = ai_context(Path::new("."), options).unwrap();

    assert!(!context.is_empty());
    assert!(context.contains("archmap"), "Should mention project name");
}

#[test]
fn test_ai_context_json() {
    let options = AiOptions {
        format: AiFormat::Json,
        tokens: Some(500),
        ..Default::default()
    };

    let context = ai_context(Path::new("."), options).unwrap();

    assert!(!context.is_empty());
    // JSON output should be parseable
    assert!(
        context.starts_with('{'),
        "JSON output should start with brace"
    );
}

#[test]
fn test_ai_context_signatures_only() {
    let options = AiOptions {
        signatures_only: true,
        format: AiFormat::Markdown,
        ..Default::default()
    };

    let context = ai_context(Path::new("."), options).unwrap();
    assert!(!context.is_empty());
}

#[test]
fn test_analysis_result_types() {
    let result = analyze(Path::new("."), AnalysisOptions::default()).unwrap();

    // Test that we can access all the exported types
    for module in &result.modules {
        let _name: &str = &module.name;
        let _path = &module.path;
        let _lines: usize = module.lines;
        let _imports: &Vec<String> = &module.imports;
        let _exports: &Vec<String> = &module.exports;

        for def in &module.definitions {
            let _def_name: &str = &def.name;
            let _kind: &archmap::DefinitionKind = &def.kind;
            let _line: usize = def.line;
            let _visibility: &archmap::Visibility = &def.visibility;
        }
    }

    for issue in &result.issues {
        let _kind: &archmap::IssueKind = &issue.kind;
        let _severity: &archmap::IssueSeverity = &issue.severity;
        let _message: &str = &issue.message;

        for location in &issue.locations {
            let _path = &location.path;
            let _line: &Option<usize> = &location.line;
        }
    }
}

#[test]
fn test_impact_result_methods() {
    let result = impact(
        Path::new("."),
        Path::new("src/model/mod.rs"),
        ImpactOptions::default(),
    )
    .unwrap();

    // Test all the ImpactResult methods
    let _target = result.target();
    let _total = result.total_affected();
    let _max_chain = result.max_chain_length();
    let _by_depth = result.affected_by_depth();
    let _all = result.all_affected();

    // Test formatting
    let _markdown = result.to_markdown(true);
    let _json = result.to_json();

    // Access inner for advanced use
    let _inner = result.inner();
}
