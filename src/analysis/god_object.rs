use crate::config::Config;
use crate::model::{DefinitionKind, Issue, Module};

pub fn detect_god_objects(modules: &[Module], config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    for module in modules {
        if module.lines < config.thresholds.god_object_lines {
            continue;
        }

        // Detect mixed responsibilities
        let responsibilities = detect_responsibilities(module);

        if responsibilities.len() > 1 {
            issues.push(Issue::god_object(
                module.path.clone(),
                module.lines,
                responsibilities,
            ));
        }
    }

    issues
}

fn detect_responsibilities(module: &Module) -> Vec<String> {
    let mut responsibilities = Vec::new();

    let has_structs = module
        .definitions
        .iter()
        .any(|d| d.kind == DefinitionKind::Struct);
    let has_functions = module
        .definitions
        .iter()
        .any(|d| d.kind == DefinitionKind::Function);
    let has_traits = module
        .definitions
        .iter()
        .any(|d| d.kind == DefinitionKind::Trait);
    let has_impls = module
        .definitions
        .iter()
        .any(|d| d.kind == DefinitionKind::Impl);

    // Count distinct types being implemented
    let impl_count = module
        .definitions
        .iter()
        .filter(|d| d.kind == DefinitionKind::Impl)
        .count();

    let struct_count = module
        .definitions
        .iter()
        .filter(|d| d.kind == DefinitionKind::Struct)
        .count();

    // Heuristics for responsibility detection
    if has_structs && struct_count > 3 {
        responsibilities.push(format!("data definitions ({} structs)", struct_count));
    }

    if has_functions {
        let fn_count = module
            .definitions
            .iter()
            .filter(|d| d.kind == DefinitionKind::Function)
            .count();
        if fn_count > 10 {
            responsibilities.push(format!("business logic ({} functions)", fn_count));
        }
    }

    if has_traits {
        responsibilities.push("trait definitions".to_string());
    }

    if has_impls && impl_count > 5 {
        responsibilities.push(format!("implementations ({} impls)", impl_count));
    }

    // If we couldn't detect specific responsibilities but it's still large
    if responsibilities.is_empty() && module.lines >= 500 {
        responsibilities.push("large module".to_string());
    }

    responsibilities
}
