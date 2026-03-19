use yozora_ast::{EcmaImport, EcmaImportNamedImport, Node, Point, Position, ECMA_IMPORT_TYPE};
use yozora_character::{
    calc_string_from_node_points, calc_trim_boundary_of_code_points, AsciiCodePoint,
};
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
        if line.count_of_precede_spaces >= 4
            || line.first_non_whitespace_index + 8 >= line.end_index
        {
            return None;
        }

        let node_points = line.node_points.as_ref();
        let i = line.first_non_whitespace_index;
        if node_points[i].code_point != AsciiCodePoint::LOWERCASE_I as i32
            || node_points[i + 1].code_point != AsciiCodePoint::LOWERCASE_M as i32
            || node_points[i + 2].code_point != AsciiCodePoint::LOWERCASE_P as i32
            || node_points[i + 3].code_point != AsciiCodePoint::LOWERCASE_O as i32
            || node_points[i + 4].code_point != AsciiCodePoint::LOWERCASE_R as i32
            || node_points[i + 5].code_point != AsciiCodePoint::LOWERCASE_T as i32
        {
            return None;
        }

        let (left, right) = calc_trim_boundary_of_code_points(
            node_points,
            line.first_non_whitespace_index,
            line.end_index,
        );
        let source = calc_string_from_node_points(node_points, left, right, false);
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
