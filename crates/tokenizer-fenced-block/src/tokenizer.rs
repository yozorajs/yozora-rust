use yozora_ast::{Node, NodeType};
use yozora_character::AsciiCodePoint;
use yozora_core_tokenizer::{
    BlockToken, BlockTokenizer, MatchBlockHook, MatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi, Tokenizer, TokenizerMeta, TokenizerPriority, TokenizerType,
};

use crate::r#match::{self, CheckInfoStringFn, FencedBlockHookContext};

pub const FENCED_BLOCK_TOKENIZER_NAME: &str = "@yozora/tokenizer-fenced-block";
pub const FENCED_BLOCK_TYPE: &str = "fencedBlock";

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

#[derive(Clone)]
pub struct FencedBlockTokenizer {
    meta: TokenizerMeta,
    context: FencedBlockHookContext,
}

impl Default for FencedBlockTokenizer {
    fn default() -> Self {
        Self::new(FencedBlockTokenizerOptions::default())
    }
}

impl FencedBlockTokenizer {
    pub fn new(options: FencedBlockTokenizerOptions) -> Self {
        let context = FencedBlockHookContext {
            node_type: options.node_type,
            markers: options.markers,
            markers_required: options.markers_required,
            check_info_string: options.check_info_string,
        };
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| FENCED_BLOCK_TOKENIZER_NAME.to_string()),
                kind: yozora_core_tokenizer::TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::FENCED_BLOCK),
            },
            context,
        }
    }

    pub fn context(&self) -> &FencedBlockHookContext {
        &self.context
    }
}

impl Tokenizer for FencedBlockTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        &self.meta.name
    }

    fn priority(&self) -> i32 {
        self.meta.priority
    }
}

struct FencedBlockParseHook;

impl ParseBlockHook for FencedBlockParseHook {
    fn parse(&self, _tokens: &[BlockToken]) -> Vec<Node> {
        panic!(
            "[{}] FencedBlockTokenizer is an abstract base tokenizer and does not implement parse().",
            FENCED_BLOCK_TOKENIZER_NAME
        );
    }
}

impl BlockTokenizer for FencedBlockTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(r#match::fenced_block_match(self.context.clone()))
    }

    fn parse<'a>(&'a self, _api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(FencedBlockParseHook)
    }
}
