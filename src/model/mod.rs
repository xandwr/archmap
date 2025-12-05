mod boundary;
mod issue;
mod module;

pub use boundary::{Boundary, BoundaryKind, BoundaryViolation};
pub use issue::{Issue, IssueKind, IssueSeverity, Location};
pub use module::{Definition, DefinitionKind, Module, Visibility};

use petgraph::graph::DiGraph;
use std::path::PathBuf;

pub struct AnalysisResult {
    pub project_name: String,
    pub modules: Vec<Module>,
    pub issues: Vec<Issue>,
    pub dependency_graph: DiGraph<PathBuf, ()>,
}
