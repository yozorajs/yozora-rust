use serde::{Deserialize, Serialize};

use crate::ast::{AlignType, Position, ReferenceType};

pub const ROOT_TYPE: &str = "root";
pub const ADMONITION_TYPE: &str = "admonition";
pub const BLOCKQUOTE_TYPE: &str = "blockquote";
pub const BREAK_TYPE: &str = "break";
pub const CODE_TYPE: &str = "code";
pub const DEFINITION_TYPE: &str = "definition";
pub const DELETE_TYPE: &str = "delete";
pub const ECMA_IMPORT_TYPE: &str = "ecmaImport";
pub const EMPHASIS_TYPE: &str = "emphasis";
pub const FOOTNOTE_TYPE: &str = "footnote";
pub const FOOTNOTE_DEFINITION_TYPE: &str = "footnoteDefinition";
pub const FOOTNOTE_REFERENCE_TYPE: &str = "footnoteReference";
pub const FRONTMATTER_TYPE: &str = "frontmatter";
pub const HEADING_TYPE: &str = "heading";
pub const HTML_TYPE: &str = "html";
pub const IMAGE_TYPE: &str = "image";
pub const IMAGE_REFERENCE_TYPE: &str = "imageReference";
pub const INLINE_CODE_TYPE: &str = "inlineCode";
pub const INLINE_MATH_TYPE: &str = "inlineMath";
pub const LINK_TYPE: &str = "link";
pub const LINK_REFERENCE_TYPE: &str = "linkReference";
pub const LIST_TYPE: &str = "list";
pub const LIST_ITEM_TYPE: &str = "listItem";
pub const MATH_TYPE: &str = "math";
pub const PARAGRAPH_TYPE: &str = "paragraph";
pub const STRONG_TYPE: &str = "strong";
pub const TABLE_TYPE: &str = "table";
pub const TABLE_ROW_TYPE: &str = "tableRow";
pub const TABLE_CELL_TYPE: &str = "tableCell";
pub const TEXT_TYPE: &str = "text";
pub const THEMATIC_BREAK_TYPE: &str = "thematicBreak";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Root {
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

impl Default for Root {
    fn default() -> Self {
        Self {
            node_type: ROOT_TYPE.to_string(),
            position: None,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Node {
    #[serde(rename = "admonition")]
    Admonition(Admonition),
    #[serde(rename = "blockquote")]
    Blockquote(Blockquote),
    #[serde(rename = "break")]
    Break(BreakNode),
    #[serde(rename = "code")]
    Code(Code),
    #[serde(rename = "definition")]
    Definition(Definition),
    #[serde(rename = "delete")]
    Delete(DeleteNode),
    #[serde(rename = "ecmaImport")]
    EcmaImport(EcmaImport),
    #[serde(rename = "emphasis")]
    Emphasis(Emphasis),
    #[serde(rename = "footnote")]
    Footnote(Footnote),
    #[serde(rename = "footnoteDefinition")]
    FootnoteDefinition(FootnoteDefinition),
    #[serde(rename = "footnoteReference")]
    FootnoteReference(FootnoteReference),
    #[serde(rename = "frontmatter")]
    Frontmatter(Frontmatter),
    #[serde(rename = "heading")]
    Heading(Heading),
    #[serde(rename = "html")]
    Html(Html),
    #[serde(rename = "image")]
    Image(Image),
    #[serde(rename = "imageReference")]
    ImageReference(ImageReference),
    #[serde(rename = "inlineCode")]
    InlineCode(InlineCode),
    #[serde(rename = "inlineMath")]
    InlineMath(InlineMath),
    #[serde(rename = "link")]
    Link(Link),
    #[serde(rename = "linkReference")]
    LinkReference(LinkReference),
    #[serde(rename = "list")]
    List(List),
    #[serde(rename = "listItem")]
    ListItem(ListItem),
    #[serde(rename = "math")]
    Math(Math),
    #[serde(rename = "paragraph")]
    Paragraph(Paragraph),
    #[serde(rename = "strong")]
    Strong(Strong),
    #[serde(rename = "table")]
    Table(Table),
    #[serde(rename = "tableRow")]
    TableRow(TableRow),
    #[serde(rename = "tableCell")]
    TableCell(TableCell),
    #[serde(rename = "text")]
    Text(Text),
    #[serde(rename = "thematicBreak")]
    ThematicBreak(ThematicBreak),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Admonition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub keyword: String,
    pub title: Vec<Node>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blockquote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Code {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Emphasis {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Footnote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootnoteDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootnoteReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heading {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    pub depth: u8,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HtmlContentType {
    Cdata,
    Closing,
    Comment,
    Declaration,
    Instruction,
    Open,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Html {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    #[serde(rename = "referenceType")]
    pub reference_type: ReferenceType,
    pub alt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub alt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineCode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineMath {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub identifier: String,
    pub label: String,
    #[serde(rename = "referenceType")]
    pub reference_type: ReferenceType,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub ordered: bool,
    #[serde(rename = "orderType", skip_serializing_if = "Option::is_none")]
    pub order_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    pub marker: u32,
    pub spread: bool,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Math {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strong {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableColumn {
    pub align: Option<AlignType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub columns: Vec<TableColumn>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThematicBreak {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}
