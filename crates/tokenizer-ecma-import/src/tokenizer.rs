use yozora_ast::{EcmaImport, EcmaImportNamedImport, Node, Point, Position, ECMA_IMPORT_TYPE};
use yozora_character::VirtualCodePoint;
use yozora_core_tokenizer::engine::{
    BlockToken, EatOpenerResult, EngineBlockTokenizer, EngineTokenizer, MatchBlockHook,
    MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const ECMA_IMPORT_TOKENIZER_NAME: &str = "@yozora/tokenizer-ecma-import";

#[derive(Debug, Clone)]
pub struct EcmaImportTokenizer {
    meta: TokenizerMeta,
}

impl Default for EcmaImportTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: ECMA_IMPORT_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 11,
            },
        }
    }
}

impl Tokenizer for EcmaImportTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for EcmaImportTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        let node = self.tokenize_block(first, position)?;
        Some(BlockTokenizeResult {
            node,
            consumed_lines: 1,
        })
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
        _api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        self.tokenize_block_lines(lines, position)
    }

    fn tokenize_block(&self, input: &str, _position: Option<yozora_ast::Position>) -> Option<Node> {
        let token = r#match::match_ecma_import_token(input)?;
        Some(parse::parse_ecma_import_token(token))
    }
}

#[derive(Debug, Clone)]
struct EcmaImportTokenData {
    module_name: String,
    default_import: Option<String>,
    named_imports: Vec<EcmaImportNamedImport>,
}

impl EngineTokenizer for EcmaImportTokenizer {
    fn tokenizer_type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        &self.meta.name
    }

    fn priority(&self) -> i32 {
        self.meta.priority
    }
}

struct EcmaImportMatchHook;

impl MatchBlockHook for EcmaImportMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        let source = line_to_string(line);
        let matched = r#match::match_ecma_import_token(&source)?;

        let token = BlockToken::new("", ECMA_IMPORT_TYPE, calc_line_position(line)).with_data(
            EcmaImportTokenData {
                module_name: matched.module_name,
                default_import: matched.default_import,
                named_imports: matched.named_imports,
            },
        );

        Some(EatOpenerResult {
            token,
            next_index: line.end_index,
            saturated: true,
        })
    }
}

struct EcmaImportParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for EcmaImportParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some(data) = token.data_as::<EcmaImportTokenData>() else {
                continue;
            };

            nodes.push(Node::EcmaImport(EcmaImport {
                position: if self.api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                module_name: data.module_name.clone(),
                default_import: data.default_import.clone(),
                named_imports: data.named_imports.clone(),
            }));
        }

        nodes
    }
}

impl EngineBlockTokenizer for EcmaImportTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(EcmaImportMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(EcmaImportParseHook { api })
    }
}

fn line_to_string(line: &PhrasingContentLine) -> String {
    let mut source = String::new();
    for point in line
        .node_points
        .iter()
        .skip(line.start_index)
        .take(line.end_index.saturating_sub(line.start_index))
    {
        let code_point = point.code_point;
        let ch = if code_point == VirtualCodePoint::Space as i32 {
            Some(' ')
        } else if code_point == VirtualCodePoint::LineEnd as i32 {
            Some('\n')
        } else {
            char::from_u32(code_point as u32)
        };

        if let Some(ch) = ch {
            source.push(ch);
        }
    }
    source
}

fn calc_line_position(line: &PhrasingContentLine) -> Option<Position> {
    if line.start_index >= line.end_index {
        return None;
    }

    let start = line.node_points[line.start_index];
    let end = line.node_points[line.end_index - 1];

    Some(Position {
        start: Point {
            line: start.line,
            column: start.column,
            offset: Some(start.offset),
        },
        end: Point {
            line: end.line,
            column: end.column + 1,
            offset: Some(end.offset + 1),
        },
        indent: None,
    })
}
