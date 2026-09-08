use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::ast::Position;

pub mod admonition;
pub mod blockquote;
pub mod r#break;
pub mod code;
pub mod definition;
pub mod delete;
pub mod ecma_import;
pub mod emphasis;
pub mod footnote;
pub mod footnote_definition;
pub mod footnote_reference;
pub mod frontmatter;
pub mod heading;
pub mod html;
pub mod image;
pub mod image_reference;
pub mod inline_code;
pub mod inline_math;
pub mod link;
pub mod link_reference;
pub mod list;
pub mod list_item;
pub mod math;
pub mod paragraph;
pub mod root;
pub mod strong;
pub mod table;
pub mod table_cell;
pub mod table_row;
pub mod text;
pub mod thematic_break;

pub use admonition::{Admonition, ADMONITION_TYPE};
pub use blockquote::{Blockquote, BLOCKQUOTE_TYPE};
pub use code::{Code, CODE_TYPE};
pub use definition::{Definition, DEFINITION_TYPE};
pub use delete::{Delete, DeleteNode, DELETE_TYPE};
pub use ecma_import::{EcmaImport, EcmaImportNamedImport, ECMA_IMPORT_TYPE};
pub use emphasis::{Emphasis, EMPHASIS_TYPE};
pub use footnote::{Footnote, FOOTNOTE_TYPE};
pub use footnote_definition::{FootnoteDefinition, FOOTNOTE_DEFINITION_TYPE};
pub use footnote_reference::{FootnoteReference, FOOTNOTE_REFERENCE_TYPE};
pub use frontmatter::{Frontmatter, FRONTMATTER_TYPE};
pub use heading::{Heading, HEADING_TYPE};
pub use html::{Html, HtmlContentType, HTML_TYPE};
pub use image::{Image, IMAGE_TYPE};
pub use image_reference::{ImageReference, IMAGE_REFERENCE_TYPE};
pub use inline_code::{InlineCode, INLINE_CODE_TYPE};
pub use inline_math::{InlineMath, INLINE_MATH_TYPE};
pub use link::{Link, LINK_TYPE};
pub use link_reference::{LinkReference, LINK_REFERENCE_TYPE};
pub use list::{List, LIST_TYPE};
pub use list_item::{ListItem, TaskStatus, LIST_ITEM_TYPE};
pub use math::{Math, MATH_TYPE};
pub use paragraph::{Paragraph, PARAGRAPH_TYPE};
pub use r#break::{Break, BreakNode, BREAK_TYPE};
pub use root::{Root, ROOT_TYPE};
pub use strong::{Strong, STRONG_TYPE};
pub use table::{Table, TableColumn, TABLE_TYPE};
pub use table_cell::{TableCell, TABLE_CELL_TYPE};
pub use table_row::{TableRow, TABLE_ROW_TYPE};
pub use text::{Text, TEXT_TYPE};
pub use thematic_break::{ThematicBreak, THEMATIC_BREAK_TYPE};

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

/// Release an AST or partial parse result without recursive destructor calls.
pub fn drop_nodes(mut nodes: Vec<Node>) {
    while let Some(mut node) = nodes.pop() {
        take_node_children(&mut node, &mut nodes);
    }
}

/// Own partial AST results across fallible parsing steps. Early returns and
/// unwinding release descendants iteratively; `into_vec` transfers completed nodes.
#[derive(Default)]
pub struct NodeBuffer(Vec<Node>);

impl NodeBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub fn push(&mut self, node: Node) {
        self.0.push(node);
    }

    pub fn into_vec(mut self) -> Vec<Node> {
        std::mem::take(&mut self.0)
    }
}

impl From<Vec<Node>> for NodeBuffer {
    fn from(nodes: Vec<Node>) -> Self {
        Self(nodes)
    }
}

impl FromIterator<Node> for NodeBuffer {
    fn from_iter<T: IntoIterator<Item = Node>>(iter: T) -> Self {
        let mut nodes = Self::default();
        nodes.0.extend(iter);
        nodes
    }
}

impl Drop for NodeBuffer {
    fn drop(&mut self) {
        drop_nodes(std::mem::take(&mut self.0));
    }
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
