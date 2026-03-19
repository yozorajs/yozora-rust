use yozora_ast::PARAGRAPH_TYPE;
use yozora_core_tokenizer::{
    calc_position_from_phrasing_content_lines, BlockToken, EatContinuationTextResult,
    EatLazyContinuationTextResult, EatOpenerResult, PhrasingContentLine,
};

#[derive(Debug, Clone)]
pub(crate) struct ParagraphTokenData {
    pub lines: Vec<PhrasingContentLine>,
}

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.first_non_whitespace_index >= line.end_index {
        return None;
    }

    let lines = vec![line.clone()];
    let position = calc_position_from_phrasing_content_lines(&lines);
    let token =
        BlockToken::new("", PARAGRAPH_TYPE, Some(position)).with_data(ParagraphTokenData { lines });
    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: false,
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    if line.first_non_whitespace_index >= line.end_index {
        return EatContinuationTextResult::NotMatched;
    }

    let Some(data) = token.data_as::<ParagraphTokenData>() else {
        return EatContinuationTextResult::NotMatched;
    };

    let mut lines = data.lines.clone();
    lines.push(line.clone());
    token.data = std::sync::Arc::new(ParagraphTokenData {
        lines: lines.clone(),
    });
    token.position = Some(calc_position_from_phrasing_content_lines(&lines));

    EatContinuationTextResult::Opening {
        next_index: line.end_index,
    }
}

pub(crate) fn eat_lazy_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatLazyContinuationTextResult {
    match eat_continuation_text(line, token) {
        EatContinuationTextResult::Opening { next_index } => {
            EatLazyContinuationTextResult::Opening { next_index }
        }
        _ => EatLazyContinuationTextResult::NotMatched,
    }
}

pub(crate) fn extract_lines(token: &BlockToken) -> Option<Vec<PhrasingContentLine>> {
    token
        .data_as::<ParagraphTokenData>()
        .map(|data| data.lines.clone())
}

pub(crate) fn trim_blank_lines(lines: &[PhrasingContentLine]) -> Vec<PhrasingContentLine> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut left = 0usize;
    while left < lines.len() {
        let line = &lines[left];
        if line.first_non_whitespace_index < line.end_index {
            break;
        }
        left += 1;
    }

    if left >= lines.len() {
        return Vec::new();
    }

    let mut right = lines.len();
    while right > left {
        let line = &lines[right - 1];
        if line.first_non_whitespace_index < line.end_index {
            break;
        }
        right -= 1;
    }

    lines[left..right].to_vec()
}

pub(crate) fn build_block_token(
    lines: &[PhrasingContentLine],
    original_token: &BlockToken,
) -> Option<BlockToken> {
    let lines = trim_blank_lines(lines);
    if lines.is_empty() {
        return None;
    }

    let position = calc_position_from_phrasing_content_lines(&lines);
    let token = BlockToken::new(
        original_token.tokenizer.clone(),
        original_token.node_type,
        Some(position),
    )
    .with_data(ParagraphTokenData { lines });
    Some(token)
}
