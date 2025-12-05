//! Module complexity detection - identifies "fat files" that accumulate
//! too much internal logic without being true "god objects" (which have
//! many public exports).
//!
//! The key distinction:
//! - God object: Large file with many PUBLIC exports (wide interface)
//! - Fat file: Large file with many PRIVATE functions (internal sprawl)
//!
//! Fat files often arise from:
//! - Command handlers accumulating in one place (like main.rs)
//! - Utility functions piling up
//! - Tests/examples embedded in source

use crate::config::Config;
use crate::model::{DefinitionKind, Issue, Module, Visibility};

/// Metrics computed for a single module
#[derive(Debug, Clone)]
pub struct ModuleComplexity {
    pub path: std::path::PathBuf,
    pub lines: usize,
    pub total_functions: usize,
    pub private_functions: usize,
    pub public_functions: usize,
    pub exports: usize,
    /// Ratio of private functions to exports - high values suggest internal sprawl
    pub sprawl_ratio: f64,
    /// Lines per export - high values suggest the module does more than its interface suggests
    pub lines_per_export: f64,
}

impl ModuleComplexity {
    pub fn compute(module: &Module) -> Self {
        let total_functions = module
            .definitions
            .iter()
            .filter(|d| d.kind == DefinitionKind::Function)
            .count();

        let private_functions = module
            .definitions
            .iter()
            .filter(|d| d.kind == DefinitionKind::Function && d.visibility == Visibility::Private)
            .count();

        let public_functions = module
            .definitions
            .iter()
            .filter(|d| d.kind == DefinitionKind::Function && d.visibility == Visibility::Public)
            .count();

        let exports = module.exports.len();

        // Sprawl ratio: private functions / (exports + 1)
        // Higher = more internal complexity hidden behind a small interface
        let sprawl_ratio = private_functions as f64 / (exports.max(1) as f64);

        // Lines per export: how much code backs each public item
        let lines_per_export = module.lines as f64 / (exports.max(1) as f64);

        Self {
            path: module.path.clone(),
            lines: module.lines,
            total_functions,
            private_functions,
            public_functions,
            exports,
            sprawl_ratio,
            lines_per_export,
        }
    }
}

/// Detect modules with excessive internal complexity ("fat files")
pub fn detect_fat_modules(modules: &[Module], config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Configurable thresholds (using existing config pattern)
    let min_lines = config.thresholds.fat_module_lines;
    let min_private_functions = config.thresholds.fat_module_private_functions;
    let max_lines_per_export = config.thresholds.fat_module_lines_per_export;

    for module in modules {
        // Skip test files - they naturally have many private test functions
        if is_test_file(&module.path) {
            continue;
        }

        let complexity = ModuleComplexity::compute(module);

        // Skip small modules
        if complexity.lines < min_lines {
            continue;
        }

        // Detect fat file pattern:
        // - Many private functions (internal sprawl)
        // - High lines-per-export ratio (lots of hidden complexity)
        let is_fat = complexity.private_functions >= min_private_functions
            && complexity.lines_per_export > max_lines_per_export;

        if is_fat {
            issues.push(Issue::fat_module(
                module.path.clone(),
                complexity.lines,
                complexity.private_functions,
                complexity.public_functions,
                complexity.exports,
            ));
        }
    }

    issues
}

/// Check if a file is a test file based on path patterns
fn is_test_file(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();

    // Common test file patterns
    path_str.contains("/tests/")
        || path_str.contains("/test/")
        || path_str.starts_with("tests/")
        || path_str.starts_with("test/")
        || path_str.ends_with("_test.rs")
        || path_str.ends_with("_tests.rs")
        || path_str.ends_with("/tests.rs")
        || path_str.ends_with(".test.ts")
        || path_str.ends_with(".spec.ts")
        || path_str.ends_with("_test.py")
        || path_str.ends_with("test_.py")
        || path_str.contains("/__tests__/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Definition, Module};
    use std::path::PathBuf;

    fn make_module(lines: usize, private_fns: usize, public_fns: usize, exports: usize) -> Module {
        let mut definitions = Vec::new();

        for i in 0..private_fns {
            definitions.push(Definition {
                name: format!("private_fn_{}", i),
                kind: DefinitionKind::Function,
                line: i + 1,
                visibility: Visibility::Private,
                signature: None,
            });
        }

        for i in 0..public_fns {
            definitions.push(Definition {
                name: format!("public_fn_{}", i),
                kind: DefinitionKind::Function,
                line: private_fns + i + 1,
                visibility: Visibility::Public,
                signature: None,
            });
        }

        Module {
            path: PathBuf::from("test.rs"),
            name: "test".to_string(),
            lines,
            imports: vec![],
            exports: (0..exports).map(|i| format!("export_{}", i)).collect(),
            definitions,
        }
    }

    #[test]
    fn test_complexity_computation() {
        let module = make_module(500, 15, 3, 2);
        let complexity = ModuleComplexity::compute(&module);

        assert_eq!(complexity.lines, 500);
        assert_eq!(complexity.total_functions, 18);
        assert_eq!(complexity.private_functions, 15);
        assert_eq!(complexity.public_functions, 3);
        assert_eq!(complexity.exports, 2);
        assert!((complexity.sprawl_ratio - 7.5).abs() < 0.01); // 15/2
        assert!((complexity.lines_per_export - 250.0).abs() < 0.01); // 500/2
    }

    #[test]
    fn test_detects_fat_module() {
        let fat_module = make_module(600, 12, 2, 1); // 600 lines, 12 private fns, 1 export
        let config = Config::default();

        let issues = detect_fat_modules(&[fat_module], &config);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_ignores_small_module() {
        let small_module = make_module(100, 15, 2, 1); // Small despite many private fns
        let config = Config::default();

        let issues = detect_fat_modules(&[small_module], &config);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_ignores_well_exported_module() {
        // Large module but with proportional exports - not a fat file
        let module = make_module(600, 10, 10, 20);
        let config = Config::default();

        let issues = detect_fat_modules(&[module], &config);
        assert!(issues.is_empty()); // lines_per_export = 30, below threshold
    }

    #[test]
    fn test_ignores_test_files() {
        let test_paths = vec![
            "src/tests/foo.rs",
            "src/foo_test.rs",
            "src/foo_tests.rs",
            "src/tests.rs",
            "src/__tests__/bar.ts",
            "tests/integration.rs",
        ];

        for path in test_paths {
            assert!(
                is_test_file(std::path::Path::new(path)),
                "{} should be detected as test file",
                path
            );
        }
    }

    #[test]
    fn test_does_not_flag_test_files() {
        let mut module = make_module(600, 12, 2, 1);
        module.path = PathBuf::from("src/tests/my_test.rs");
        let config = Config::default();

        let issues = detect_fat_modules(&[module], &config);
        assert!(issues.is_empty(), "Test files should not be flagged");
    }
}
