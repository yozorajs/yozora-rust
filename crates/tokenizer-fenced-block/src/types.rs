use std::sync::Arc;

use yozora_ast::NodeType;
use yozora_character::{AsciiCodePoint, NodePoint};
use yozora_core_tokenizer::PhrasingContentLine;

pub type CheckInfoStringFn = Arc<dyn Fn(&[NodePoint], i32, usize) -> bool + Send + Sync + 'static>;

#[derive(Clone)]
pub struct FencedBlockHookContext {
    pub node_type: NodeType,
    pub markers: Vec<i32>,
    pub markers_required: usize,
    pub check_info_string: Option<CheckInfoStringFn>,
}

#[derive(Debug, Clone)]
pub struct FencedBlockTokenData {
    pub marker: i32,
    pub marker_count: usize,
    pub indent: usize,
    pub info_string: Vec<NodePoint>,
    pub lines: Vec<PhrasingContentLine>,
}

#[derive(Clone)]
pub struct FencedBlockTokenizerOptions {
    pub name: Option<String>,
    pub priority: Option<i32>,
    pub node_type: NodeType,
    pub markers: Vec<i32>,
    pub markers_required: usize,
    pub check_info_string: Option<CheckInfoStringFn>,
}

impl Default for FencedBlockTokenizerOptions {
    fn default() -> Self {
        Self {
            name: None,
            priority: None,
            node_type: FENCED_BLOCK_TYPE,
            markers: vec![
                AsciiCodePoint::BACKTICK as i32,
                AsciiCodePoint::TILDE as i32,
            ],
            markers_required: 3,
            check_info_string: None,
        }
    }
}

pub const FENCED_BLOCK_TOKENIZER_NAME: &str = "@yozora/tokenizer-fenced-block";
pub const FENCED_BLOCK_TYPE: &str = "fencedBlock";
