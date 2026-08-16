use serde::{Deserialize, Serialize};

use crate::ast::Position;

pub const ECMA_IMPORT_TYPE: &str = "ecmaImport";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcmaImportNamedImport {
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EcmaImport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(rename = "moduleName")]
    pub module_name: String,
    #[serde(rename = "defaultImport", skip_serializing_if = "Option::is_none")]
    pub default_import: Option<String>,
    #[serde(rename = "namedImports")]
    pub named_imports: Vec<EcmaImportNamedImport>,
}
