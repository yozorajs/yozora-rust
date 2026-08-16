use std::sync::Arc;

use yozora_ast::{NodeType, Position, TableColumn};
use yozora_core_tokenizer::{BlockTokenChildren, PhrasingContentLine};

#[derive(Debug, Clone)]
pub struct TableCellTokenData {
    pub tokenizer: Option<Arc<str>>,
    pub node_type: NodeType,
    pub children: Option<BlockTokenChildren>,
    pub position: Position,
    pub lines: Vec<PhrasingContentLine>,
}

#[derive(Debug, Clone)]
pub struct TableRowTokenData {
    pub tokenizer: Option<Arc<str>>,
    pub node_type: NodeType,
    pub children: Option<BlockTokenChildren>,
    pub position: Position,
    pub cells: Vec<TableCellTokenData>,
}

#[derive(Debug, Clone)]
pub struct TableTokenData {
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRowTokenData>,
}

pub const TABLE_TOKENIZER_NAME: &str = "@yozora/tokenizer-table";
