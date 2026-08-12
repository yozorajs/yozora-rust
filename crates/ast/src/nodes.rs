use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

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

impl Drop for Root {
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.children);
        while let Some(mut node) = stack.pop() {
            take_node_children(&mut node, &mut stack);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Admonition(Admonition),
    Blockquote(Blockquote),
    Break(BreakNode),
    Code(Code),
    Definition(Definition),
    Delete(DeleteNode),
    EcmaImport(EcmaImport),
    Emphasis(Emphasis),
    Footnote(Footnote),
    FootnoteDefinition(FootnoteDefinition),
    FootnoteReference(FootnoteReference),
    Frontmatter(Frontmatter),
    Heading(Heading),
    Html(Html),
    Image(Image),
    ImageReference(ImageReference),
    InlineCode(InlineCode),
    InlineMath(InlineMath),
    Link(Link),
    LinkReference(LinkReference),
    List(List),
    ListItem(ListItem),
    Math(Math),
    Paragraph(Paragraph),
    Strong(Strong),
    Table(Table),
    TableRow(TableRow),
    TableCell(TableCell),
    Text(Text),
    ThematicBreak(ThematicBreak),
    Custom(CustomNode),
}

fn take_node_children(node: &mut Node, stack: &mut Vec<Node>) {
    match node {
        Node::Admonition(node) => {
            stack.extend(std::mem::take(&mut node.title));
            stack.extend(std::mem::take(&mut node.children));
        }
        Node::Blockquote(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Delete(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Emphasis(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Footnote(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::FootnoteDefinition(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Heading(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Link(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::LinkReference(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::List(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::ListItem(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Paragraph(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Strong(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Table(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::TableRow(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::TableCell(node) => stack.extend(std::mem::take(&mut node.children)),
        Node::Custom(node) => {
            if let Some(children) = node.children.take() {
                stack.extend(children);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomNode {
    pub node_type: String,
    pub position: Option<Position>,
    pub children: Option<Vec<Node>>,
    pub data: Map<String, Value>,
}

impl Serialize for Node {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        fn tagged<T, S>(node_type: &str, node: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            T: Serialize,
            S: Serializer,
        {
            let mut value = serde_json::to_value(node).map_err(serde::ser::Error::custom)?;
            let object = value
                .as_object_mut()
                .ok_or_else(|| serde::ser::Error::custom("node should serialize as object"))?;
            object.insert("type".to_string(), Value::String(node_type.to_string()));
            value.serialize(serializer)
        }

        match self {
            Self::Admonition(node) => tagged(ADMONITION_TYPE, node, serializer),
            Self::Blockquote(node) => tagged(BLOCKQUOTE_TYPE, node, serializer),
            Self::Break(node) => tagged(BREAK_TYPE, node, serializer),
            Self::Code(node) => tagged(CODE_TYPE, node, serializer),
            Self::Definition(node) => tagged(DEFINITION_TYPE, node, serializer),
            Self::Delete(node) => tagged(DELETE_TYPE, node, serializer),
            Self::EcmaImport(node) => tagged(ECMA_IMPORT_TYPE, node, serializer),
            Self::Emphasis(node) => tagged(EMPHASIS_TYPE, node, serializer),
            Self::Footnote(node) => tagged(FOOTNOTE_TYPE, node, serializer),
            Self::FootnoteDefinition(node) => tagged(FOOTNOTE_DEFINITION_TYPE, node, serializer),
            Self::FootnoteReference(node) => tagged(FOOTNOTE_REFERENCE_TYPE, node, serializer),
            Self::Frontmatter(node) => tagged(FRONTMATTER_TYPE, node, serializer),
            Self::Heading(node) => tagged(HEADING_TYPE, node, serializer),
            Self::Html(node) => tagged(HTML_TYPE, node, serializer),
            Self::Image(node) => tagged(IMAGE_TYPE, node, serializer),
            Self::ImageReference(node) => tagged(IMAGE_REFERENCE_TYPE, node, serializer),
            Self::InlineCode(node) => tagged(INLINE_CODE_TYPE, node, serializer),
            Self::InlineMath(node) => tagged(INLINE_MATH_TYPE, node, serializer),
            Self::Link(node) => tagged(LINK_TYPE, node, serializer),
            Self::LinkReference(node) => tagged(LINK_REFERENCE_TYPE, node, serializer),
            Self::List(node) => tagged(LIST_TYPE, node, serializer),
            Self::ListItem(node) => tagged(LIST_ITEM_TYPE, node, serializer),
            Self::Math(node) => tagged(MATH_TYPE, node, serializer),
            Self::Paragraph(node) => tagged(PARAGRAPH_TYPE, node, serializer),
            Self::Strong(node) => tagged(STRONG_TYPE, node, serializer),
            Self::Table(node) => tagged(TABLE_TYPE, node, serializer),
            Self::TableRow(node) => tagged(TABLE_ROW_TYPE, node, serializer),
            Self::TableCell(node) => tagged(TABLE_CELL_TYPE, node, serializer),
            Self::Text(node) => tagged(TEXT_TYPE, node, serializer),
            Self::ThematicBreak(node) => tagged(THEMATIC_BREAK_TYPE, node, serializer),
            Self::Custom(node) => {
                let mut object = node.data.clone();
                object.insert("type".to_string(), Value::String(node.node_type.clone()));
                if let Some(position) = &node.position {
                    object.insert(
                        "position".to_string(),
                        serde_json::to_value(position).map_err(serde::ser::Error::custom)?,
                    );
                }
                if let Some(children) = &node.children {
                    object.insert(
                        "children".to_string(),
                        serde_json::to_value(children).map_err(serde::ser::Error::custom)?,
                    );
                }
                object.serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let mut object = value
            .as_object()
            .cloned()
            .ok_or_else(|| serde::de::Error::custom("node should be an object"))?;
        let node_type = object
            .remove("type")
            .and_then(|value| value.as_str().map(str::to_string))
            .ok_or_else(|| serde::de::Error::custom("node.type should be a string"))?;
        let data = Value::Object(object.clone());
        macro_rules! parse {
            ($variant:ident, $type:ty) => {
                serde_json::from_value::<$type>(data)
                    .map(Self::$variant)
                    .map_err(serde::de::Error::custom)
            };
        }
        match node_type.as_str() {
            ADMONITION_TYPE => parse!(Admonition, Admonition),
            BLOCKQUOTE_TYPE => parse!(Blockquote, Blockquote),
            BREAK_TYPE => parse!(Break, BreakNode),
            CODE_TYPE => parse!(Code, Code),
            DEFINITION_TYPE => parse!(Definition, Definition),
            DELETE_TYPE => parse!(Delete, DeleteNode),
            ECMA_IMPORT_TYPE => parse!(EcmaImport, EcmaImport),
            EMPHASIS_TYPE => parse!(Emphasis, Emphasis),
            FOOTNOTE_TYPE => parse!(Footnote, Footnote),
            FOOTNOTE_DEFINITION_TYPE => parse!(FootnoteDefinition, FootnoteDefinition),
            FOOTNOTE_REFERENCE_TYPE => parse!(FootnoteReference, FootnoteReference),
            FRONTMATTER_TYPE => parse!(Frontmatter, Frontmatter),
            HEADING_TYPE => parse!(Heading, Heading),
            HTML_TYPE => parse!(Html, Html),
            IMAGE_TYPE => parse!(Image, Image),
            IMAGE_REFERENCE_TYPE => parse!(ImageReference, ImageReference),
            INLINE_CODE_TYPE => parse!(InlineCode, InlineCode),
            INLINE_MATH_TYPE => parse!(InlineMath, InlineMath),
            LINK_TYPE => parse!(Link, Link),
            LINK_REFERENCE_TYPE => parse!(LinkReference, LinkReference),
            LIST_TYPE => parse!(List, List),
            LIST_ITEM_TYPE => parse!(ListItem, ListItem),
            MATH_TYPE => parse!(Math, Math),
            PARAGRAPH_TYPE => parse!(Paragraph, Paragraph),
            STRONG_TYPE => parse!(Strong, Strong),
            TABLE_TYPE => parse!(Table, Table),
            TABLE_ROW_TYPE => parse!(TableRow, TableRow),
            TABLE_CELL_TYPE => parse!(TableCell, TableCell),
            TEXT_TYPE => parse!(Text, Text),
            THEMATIC_BREAK_TYPE => parse!(ThematicBreak, ThematicBreak),
            _ => {
                let position = object
                    .remove("position")
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(serde::de::Error::custom)?;
                let children = object
                    .remove("children")
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(serde::de::Error::custom)?;
                Ok(Self::Custom(CustomNode {
                    node_type,
                    position,
                    children,
                    data: object,
                }))
            }
        }
    }
}

impl Node {
    pub fn node_type(&self) -> &str {
        match self {
            Self::Admonition(_) => ADMONITION_TYPE,
            Self::Blockquote(_) => BLOCKQUOTE_TYPE,
            Self::Break(_) => BREAK_TYPE,
            Self::Code(_) => CODE_TYPE,
            Self::Definition(_) => DEFINITION_TYPE,
            Self::Delete(_) => DELETE_TYPE,
            Self::EcmaImport(_) => ECMA_IMPORT_TYPE,
            Self::Emphasis(_) => EMPHASIS_TYPE,
            Self::Footnote(_) => FOOTNOTE_TYPE,
            Self::FootnoteDefinition(_) => FOOTNOTE_DEFINITION_TYPE,
            Self::FootnoteReference(_) => FOOTNOTE_REFERENCE_TYPE,
            Self::Frontmatter(_) => FRONTMATTER_TYPE,
            Self::Heading(_) => HEADING_TYPE,
            Self::Html(_) => HTML_TYPE,
            Self::Image(_) => IMAGE_TYPE,
            Self::ImageReference(_) => IMAGE_REFERENCE_TYPE,
            Self::InlineCode(_) => INLINE_CODE_TYPE,
            Self::InlineMath(_) => INLINE_MATH_TYPE,
            Self::Link(_) => LINK_TYPE,
            Self::LinkReference(_) => LINK_REFERENCE_TYPE,
            Self::List(_) => LIST_TYPE,
            Self::ListItem(_) => LIST_ITEM_TYPE,
            Self::Math(_) => MATH_TYPE,
            Self::Paragraph(_) => PARAGRAPH_TYPE,
            Self::Strong(_) => STRONG_TYPE,
            Self::Table(_) => TABLE_TYPE,
            Self::TableRow(_) => TABLE_ROW_TYPE,
            Self::TableCell(_) => TABLE_CELL_TYPE,
            Self::Text(_) => TEXT_TYPE,
            Self::ThematicBreak(_) => THEMATIC_BREAK_TYPE,
            Self::Custom(node) => &node.node_type,
        }
    }

    pub fn position(&self) -> Option<&Position> {
        match self {
            Self::Admonition(node) => node.position.as_ref(),
            Self::Blockquote(node) => node.position.as_ref(),
            Self::Break(node) => node.position.as_ref(),
            Self::Code(node) => node.position.as_ref(),
            Self::Definition(node) => node.position.as_ref(),
            Self::Delete(node) => node.position.as_ref(),
            Self::EcmaImport(node) => node.position.as_ref(),
            Self::Emphasis(node) => node.position.as_ref(),
            Self::Footnote(node) => node.position.as_ref(),
            Self::FootnoteDefinition(node) => node.position.as_ref(),
            Self::FootnoteReference(node) => node.position.as_ref(),
            Self::Frontmatter(node) => node.position.as_ref(),
            Self::Heading(node) => node.position.as_ref(),
            Self::Html(node) => node.position.as_ref(),
            Self::Image(node) => node.position.as_ref(),
            Self::ImageReference(node) => node.position.as_ref(),
            Self::InlineCode(node) => node.position.as_ref(),
            Self::InlineMath(node) => node.position.as_ref(),
            Self::Link(node) => node.position.as_ref(),
            Self::LinkReference(node) => node.position.as_ref(),
            Self::List(node) => node.position.as_ref(),
            Self::ListItem(node) => node.position.as_ref(),
            Self::Math(node) => node.position.as_ref(),
            Self::Paragraph(node) => node.position.as_ref(),
            Self::Strong(node) => node.position.as_ref(),
            Self::Table(node) => node.position.as_ref(),
            Self::TableRow(node) => node.position.as_ref(),
            Self::TableCell(node) => node.position.as_ref(),
            Self::Text(node) => node.position.as_ref(),
            Self::ThematicBreak(node) => node.position.as_ref(),
            Self::Custom(node) => node.position.as_ref(),
        }
    }

    pub fn children(&self) -> Option<&[Node]> {
        match self {
            Self::Admonition(node) => Some(&node.children),
            Self::Blockquote(node) => Some(&node.children),
            Self::Delete(node) => Some(&node.children),
            Self::Emphasis(node) => Some(&node.children),
            Self::Footnote(node) => Some(&node.children),
            Self::FootnoteDefinition(node) => Some(&node.children),
            Self::Heading(node) => Some(&node.children),
            Self::Link(node) => Some(&node.children),
            Self::LinkReference(node) => Some(&node.children),
            Self::List(node) => Some(&node.children),
            Self::ListItem(node) => Some(&node.children),
            Self::Paragraph(node) => Some(&node.children),
            Self::Strong(node) => Some(&node.children),
            Self::Table(node) => Some(&node.children),
            Self::TableRow(node) => Some(&node.children),
            Self::TableCell(node) => Some(&node.children),
            Self::Custom(node) => node.children.as_deref(),
            _ => None,
        }
    }
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
