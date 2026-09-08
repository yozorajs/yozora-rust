use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl From<yozora_ast::Point> for Position {
    fn from(point: yozora_ast::Point) -> Self {
        // Document size is bounded below u32::MAX; parser coordinates use UTF-16.
        Self {
            line: point.line.saturating_sub(1) as u32,
            character: point.column.saturating_sub(1) as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn contains(self, position: Position) -> bool {
        self.start <= position && position < self.end
    }
}

impl From<&yozora_ast::Position> for Range {
    fn from(position: &yozora_ast::Position) -> Self {
        Self {
            start: position.start.into(),
            end: position.end.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: u32,
    pub range: Range,
    pub selection_range: Range,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DocumentSymbol>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRange {
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Serialize)]
pub struct DocumentLink {
    pub range: Range,
    pub target: String,
}

#[derive(Deserialize)]
pub struct WorkspaceFolder {
    pub uri: String,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationOptions {
    #[serde(default)]
    pub heading_id_prefix: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub root_uri: Option<String>,
    pub workspace_folders: Option<Vec<WorkspaceFolder>>,
    pub initialization_options: Option<InitializationOptions>,
}

#[derive(Deserialize)]
pub struct WorkspaceFoldersChange {
    pub added: Vec<WorkspaceFolder>,
    pub removed: Vec<WorkspaceFolder>,
}

#[derive(Deserialize)]
pub struct DidChangeWorkspaceFoldersParams {
    pub event: WorkspaceFoldersChange,
}

#[derive(Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

#[derive(Deserialize)]
pub struct VersionedTextDocumentIdentifier {
    pub uri: String,
    pub version: i32,
}

#[derive(Deserialize)]
pub struct TextDocumentItem {
    pub uri: String,
    pub version: i32,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenParams {
    pub text_document: TextDocumentItem,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentParams {
    pub text_document: TextDocumentIdentifier,
}

#[derive(Debug, Deserialize)]
pub struct ContentChange {
    pub range: Option<Range>,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeParams {
    pub text_document: VersionedTextDocumentIdentifier,
    // Retain the URI/version even if decoding the edit batch fails.
    #[serde(default)]
    pub content_changes: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameParams {
    #[serde(flatten)]
    pub position_params: TextDocumentPositionParams,
    pub new_name: String,
}

#[derive(Debug, Serialize)]
pub struct PrepareRenameResult {
    pub range: Range,
    pub placeholder: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceContext {
    pub include_declaration: bool,
}

#[derive(Deserialize)]
pub struct ReferencesParams {
    #[serde(flatten)]
    pub position_params: TextDocumentPositionParams,
    pub context: ReferenceContext,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    pub kind: u32,
    pub detail: String,
    pub filter_text: String,
    pub text_edit: TextEdit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
}

impl ResponseError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(-32602, message)
    }

    pub fn response(&self, id: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": self.code, "message": self.message },
        })
    }
}
