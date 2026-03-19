use yozora_ast::{
    List, ListItem, Node, NodeType, Paragraph, Point, Position, TaskStatus, LIST_TYPE,
    PARAGRAPH_TYPE,
};
use yozora_character::{
    is_ascii_digit_character, is_ascii_lower_letter, is_ascii_upper_letter, is_space_character,
    is_whitespace_character, AsciiCodePoint, NodePoint, VirtualCodePoint,
};
use yozora_core_tokenizer::engine::{
    BlockToken, EatAndInterruptPreviousSiblingResult, EatContinuationTextResult, EatOpenerResult,
    EngineBlockTokenizer, EngineTokenizer, MatchBlockHook,
    MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, PhrasingContentLine, RemainingSibling,
    TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const LIST_TOKENIZER_NAME: &str = "@yozora/tokenizer-list";

#[derive(Debug, Clone)]
pub struct ListTokenizerOptions {
    pub enable_task_list_item: bool,
    pub empty_item_could_not_interrupted_types: Vec<NodeType>,
}

impl Default for ListTokenizerOptions {
    fn default() -> Self {
        Self {
            enable_task_list_item: false,
            empty_item_could_not_interrupted_types: vec![PARAGRAPH_TYPE],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ListTokenizer {
    meta: TokenizerMeta,
    enable_task_list_item: bool,
    empty_item_could_not_interrupted_types: Vec<NodeType>,
}

impl Default for ListTokenizer {
    fn default() -> Self {
        Self::new(ListTokenizerOptions::default())
    }
}

impl ListTokenizer {
    pub fn new(options: ListTokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: LIST_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 6,
            },
            enable_task_list_item: options.enable_task_list_item,
            empty_item_could_not_interrupted_types: options.empty_item_could_not_interrupted_types,
        }
    }
}

impl Tokenizer for ListTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for ListTokenizer {
    fn can_interrupt_paragraph_with_lines(&self, lines: &[&str]) -> bool {
        r#match::can_interrupt_paragraph(lines)
    }

    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let token = r#match::match_list_token(lines, self.enable_task_list_item)?;
        Some(parse::parse_list_token(token))
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
        _api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        self.tokenize_block_lines(lines, position)
    }
}

#[derive(Debug, Clone)]
struct TokenData {
    ordered: bool,
    marker: u32,
    order_type: Option<String>,
    order: Option<usize>,
    status: Option<TaskStatus>,
    indent: usize,
    count_of_top_blank_line: i32,
}

#[derive(Debug, Clone)]
struct ParsedOpener {
    ordered: bool,
    marker: u32,
    order_type: Option<String>,
    order: Option<usize>,
    status: Option<TaskStatus>,
    indent: usize,
    count_of_top_blank_line: i32,
    next_index: usize,
}

#[derive(Debug, Clone)]
struct TaskStatusMatch {
    status: Option<TaskStatus>,
    next_index: usize,
}

impl EngineTokenizer for ListTokenizer {
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

struct ListMatchHook {
    enable_task_list_item: bool,
    empty_item_could_not_interrupted_types: Vec<NodeType>,
}

impl MatchBlockHook for ListMatchHook {
    fn is_containing_block(&self) -> bool {
        true
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        let opener = parse_list_opener(line, self.enable_task_list_item)?;

        let token = BlockToken::new(
            "",
            LIST_TYPE,
            calc_segment_position(line, line.start_index, opener.next_index),
        )
        .with_data(TokenData {
            ordered: opener.ordered,
            marker: opener.marker,
            order_type: opener.order_type,
            order: opener.order,
            status: opener.status,
            indent: opener.indent,
            count_of_top_blank_line: opener.count_of_top_blank_line,
        });

        Some(EatOpenerResult {
            token,
            next_index: opener.next_index,
            saturated: false,
        })
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        let opener = self.eat_opener(line, prev_sibling_token)?;
        let data = opener.token.data_as::<TokenData>()?;

        if self
            .empty_item_could_not_interrupted_types
            .contains(&prev_sibling_token.node_type)
        {
            if data.indent == line.end_index.saturating_sub(line.start_index) {
                return None;
            }

            if data.ordered && data.order != Some(1) {
                return None;
            }
        }

        Some(EatAndInterruptPreviousSiblingResult {
            token: opener.token,
            next_index: opener.next_index,
            saturated: opener.saturated,
            remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
        })
    }

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        let Some(mut data) = token.data_as::<TokenData>().cloned() else {
            return EatContinuationTextResult::NotMatched;
        };
        let indent = data.indent;

        if line.first_non_whitespace_index < line.end_index && line.count_of_precede_spaces < indent
        {
            return EatContinuationTextResult::NotMatched;
        }

        if line.first_non_whitespace_index >= line.end_index {
            if data.count_of_top_blank_line >= 0 {
                data.count_of_top_blank_line += 1;
                if data.count_of_top_blank_line > 1 {
                    return EatContinuationTextResult::NotMatched;
                }
            }
        } else {
            data.count_of_top_blank_line = -1;
        }

        token.data = std::sync::Arc::new(data);
        update_token_end_position(token, line);

        let next_index = std::cmp::min(line.start_index + indent, line.end_index.saturating_sub(1));

        EatContinuationTextResult::Opening { next_index }
    }
}

struct ListParseHook<'a> {
    api: &'a dyn EngineParseBlockPhaseApi,
}

impl ParseBlockHook for ListParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(tokens.len());
        let mut list_item_tokens: Vec<&BlockToken> = Vec::new();

        for token in tokens {
            let Some(data) = token.data_as::<TokenData>() else {
                continue;
            };

            if list_item_tokens.is_empty() {
                list_item_tokens.push(token);
                continue;
            }

            let Some(first_data) = list_item_tokens[0].data_as::<TokenData>() else {
                list_item_tokens.clear();
                list_item_tokens.push(token);
                continue;
            };

            if first_data.ordered == data.ordered
                && first_data.order_type == data.order_type
                && first_data.marker == data.marker
            {
                list_item_tokens.push(token);
            } else {
                if let Some(node) = resolve_list(&list_item_tokens, self.api) {
                    nodes.push(node);
                }

                list_item_tokens.clear();
                list_item_tokens.push(token);
            }
        }

        if let Some(node) = resolve_list(&list_item_tokens, self.api) {
            nodes.push(node);
        }

        nodes
    }
}

impl EngineBlockTokenizer for ListTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(ListMatchHook {
            enable_task_list_item: self.enable_task_list_item,
            empty_item_could_not_interrupted_types: self
                .empty_item_could_not_interrupted_types
                .clone(),
        })
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(ListParseHook { api })
    }
}

fn resolve_list(tokens: &[&BlockToken], api: &dyn EngineParseBlockPhaseApi) -> Option<Node> {
    let first_token = *tokens.first()?;
    let first_data = first_token.data_as::<TokenData>()?;
    let spread = calc_spread(tokens);

    let mut children = Vec::with_capacity(tokens.len());
    for token in tokens {
        let data = token.data_as::<TokenData>()?;
        let item_nodes = api.parse_block_tokens(&token.children);
        let item_children = if spread {
            item_nodes
        } else {
            flatten_paragraph_nodes(item_nodes)
        };

        children.push(Node::ListItem(ListItem {
            position: if api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            status: data.status,
            children: item_children,
        }));
    }

    let position = if api.should_reserve_position() {
        calc_list_position(tokens)
    } else {
        None
    };

    Some(Node::List(List {
        position,
        ordered: first_data.ordered,
        order_type: first_data.order_type.clone(),
        start: first_data.order,
        marker: first_data.marker,
        spread,
        children,
    }))
}

fn calc_spread(tokens: &[&BlockToken]) -> bool {
    for token in tokens {
        if token.children.len() > 1 && has_spread_between_children(&token.children) {
            return true;
        }
    }

    if tokens.len() > 1 {
        let mut previous = tokens[0].position.as_ref();
        for token in &tokens[1..] {
            let current = token.position.as_ref();
            if let (Some(prev), Some(curr)) = (previous, current) {
                if prev.end.line + 1 < curr.start.line {
                    return true;
                }
            }

            if current.is_some() {
                previous = current;
            }
        }
    }

    false
}

fn has_spread_between_children(children: &[BlockToken]) -> bool {
    if children.len() <= 1 {
        return false;
    }

    let mut previous = children[0].position.as_ref();
    for child in &children[1..] {
        let current = child.position.as_ref();
        if let (Some(prev), Some(curr)) = (previous, current) {
            if prev.end.line + 1 < curr.start.line {
                return true;
            }
        }

        if current.is_some() {
            previous = current;
        }
    }

    false
}

fn flatten_paragraph_nodes(nodes: Vec<Node>) -> Vec<Node> {
    let mut flattened = Vec::new();
    for node in nodes {
        match node {
            Node::Paragraph(Paragraph { children, .. }) => flattened.extend(children),
            _ => flattened.push(node),
        }
    }
    flattened
}

fn calc_list_position(tokens: &[&BlockToken]) -> Option<Position> {
    let first = tokens.first()?.position.as_ref()?;
    let last = tokens.last()?.position.as_ref()?;

    Some(Position {
        start: Point {
            line: first.start.line,
            column: first.start.column,
            offset: first.start.offset,
        },
        end: Point {
            line: last.end.line,
            column: last.end.column,
            offset: last.end.offset,
        },
        indent: None,
    })
}

fn parse_list_opener(
    line: &PhrasingContentLine,
    enable_task_list_item: bool,
) -> Option<ParsedOpener> {
    if line.count_of_precede_spaces >= 4 {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let start_index = line.start_index;
    let end_index = line.end_index;
    let first_non_whitespace_index = line.first_non_whitespace_index;

    if first_non_whitespace_index >= end_index {
        return None;
    }

    let mut ordered = false;
    let mut marker: Option<i32> = None;
    let mut order_type: Option<&str> = None;
    let mut order: Option<usize> = None;

    let mut i = first_non_whitespace_index;
    let mut c = node_points[i].code_point;

    if i + 1 < end_index {
        let c0 = c;
        if is_ascii_digit_character(c0) {
            let mut value = (c0 - AsciiCodePoint::DIGIT0 as i32) as usize;
            i += 1;
            while i < end_index {
                c = node_points[i].code_point;
                if !is_ascii_digit_character(c) {
                    break;
                }

                value = value * 10 + (c - AsciiCodePoint::DIGIT0 as i32) as usize;
                i += 1;
            }
            order = Some(value);
            order_type = Some("1");
        } else if is_ascii_lower_letter(c0) {
            i += 1;
            c = node_points[i].code_point;
            order = Some((c0 - AsciiCodePoint::LOWERCASE_A as i32 + 1) as usize);
            order_type = Some("a");
        } else if is_ascii_upper_letter(c0) {
            i += 1;
            c = node_points[i].code_point;
            order = Some((c0 - AsciiCodePoint::UPPERCASE_A as i32 + 1) as usize);
            order_type = Some("A");
        }

        if i > first_non_whitespace_index
            && i - first_non_whitespace_index <= 9
            && (c == AsciiCodePoint::DOT as i32 || c == AsciiCodePoint::CLOSE_PARENTHESIS as i32)
        {
            i += 1;
            ordered = true;
            marker = Some(c);
        }
    }

    if !ordered {
        i = first_non_whitespace_index;
        c = node_points[i].code_point;
        if c == AsciiCodePoint::PLUS_SIGN as i32
            || c == AsciiCodePoint::MINUS_SIGN as i32
            || c == AsciiCodePoint::ASTERISK as i32
        {
            i += 1;
            marker = Some(c);
        }
    }

    let marker = marker? as u32;

    let mut count_of_spaces = 0usize;
    let mut next_index = i;
    if next_index < end_index
        && node_points[next_index].code_point == VirtualCodePoint::Space as i32
    {
        next_index += 1;
    }

    while next_index < end_index {
        c = node_points[next_index].code_point;
        if !is_space_character(c) {
            break;
        }

        count_of_spaces += 1;
        next_index += 1;
    }

    if count_of_spaces > 4 {
        next_index -= count_of_spaces - 1;
        count_of_spaces = 1;
    }

    if count_of_spaces == 0
        && next_index < end_index
        && node_points[next_index].code_point != VirtualCodePoint::LineEnd as i32
    {
        return None;
    }

    let mut count_of_top_blank_line = -1;
    if next_index < end_index
        && node_points[next_index].code_point == VirtualCodePoint::LineEnd as i32
    {
        count_of_top_blank_line = 1;

        if count_of_spaces > 0 {
            next_index -= count_of_spaces - 1;
        }
        count_of_spaces = 1;
    }

    let indent = i - start_index + count_of_spaces;

    let mut status = None;
    if enable_task_list_item {
        let result = eat_task_status(node_points, next_index, end_index);
        status = result.status;
        next_index = result.next_index;
    }

    Some(ParsedOpener {
        ordered,
        marker,
        order_type: if ordered {
            order_type.map(str::to_string)
        } else {
            None
        },
        order: if ordered { order } else { None },
        status,
        indent,
        count_of_top_blank_line,
        next_index,
    })
}

fn eat_task_status(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> TaskStatusMatch {
    let mut i = start_index;
    while i < end_index {
        let c = node_points[i].code_point;
        if !is_space_character(c) {
            break;
        }

        i += 1;
    }

    if i + 3 >= end_index
        || node_points[i].code_point != AsciiCodePoint::OPEN_BRACKET as i32
        || node_points[i + 2].code_point != AsciiCodePoint::CLOSE_BRACKET as i32
        || !is_whitespace_character(node_points[i + 3].code_point)
    {
        return TaskStatusMatch {
            status: None,
            next_index: start_index,
        };
    }

    let status = match node_points[i + 1].code_point {
        x if x == AsciiCodePoint::SPACE as i32 => Some(TaskStatus::Todo),
        x if x == AsciiCodePoint::MINUS_SIGN as i32 => Some(TaskStatus::Doing),
        x if x == AsciiCodePoint::LOWERCASE_X as i32 || x == AsciiCodePoint::UPPERCASE_X as i32 => {
            Some(TaskStatus::Done)
        }
        _ => None,
    };

    if status.is_none() {
        return TaskStatusMatch {
            status: None,
            next_index: start_index,
        };
    }

    TaskStatusMatch {
        status,
        next_index: i + 4,
    }
}

fn calc_segment_position(
    line: &PhrasingContentLine,
    start_index: usize,
    end_index: usize,
) -> Option<Position> {
    if start_index >= end_index {
        return None;
    }

    let start = line.node_points[start_index];
    let end = line.node_points[end_index - 1];

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

fn update_token_end_position(token: &mut BlockToken, line: &PhrasingContentLine) {
    let Some(position) = token.position.as_mut() else {
        return;
    };
    if line.start_index >= line.end_index {
        return;
    }

    let end = line.node_points[line.end_index - 1];
    position.end = Point {
        line: end.line,
        column: end.column + 1,
        offset: Some(end.offset + 1),
    };
}
