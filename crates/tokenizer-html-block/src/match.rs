use std::sync::Arc;

use yozora_ast::HTML_TYPE;
use yozora_character::calc_string_from_node_points;
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, EatAndInterruptPreviousSiblingResult,
    EatContinuationTextResult, EatOpenerResult, PhrasingContentLine, RemainingSibling,
};

#[derive(Debug, Clone)]
pub(crate) struct HtmlBlockTokenData {
    pub kind: HtmlBlockKind,
    pub lines: Vec<PhrasingContentLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HtmlBlockKind {
    Type1(String),
    Type2,
    Type3,
    Type4,
    Type5,
    Type6,
    Type7,
}

impl HtmlBlockKind {
    pub(crate) fn ends_on_blank_line(&self) -> bool {
        matches!(self, Self::Type6 | Self::Type7)
    }

    pub(crate) fn is_closed_by_line(&self, line: &str) -> bool {
        match self {
            Self::Type1(tag) => contains_case_insensitive(line, &format!("</{tag}")),
            Self::Type2 => line.contains("-->"),
            Self::Type3 => line.contains("?>"),
            Self::Type4 => line.contains('>'),
            Self::Type5 => line.contains("]]>"),
            Self::Type6 | Self::Type7 => false,
        }
    }
}

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    let source = calc_line_text(line);
    let kind = detect_html_block_kind(&source)?;

    let token =
        BlockToken::new("", HTML_TYPE, calc_line_position(line)).with_data(HtmlBlockTokenData {
            kind: kind.clone(),
            lines: vec![line.clone()],
        });

    let saturated = !kind.ends_on_blank_line() && kind.is_closed_by_line(&source);
    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated,
    })
}

pub(crate) fn eat_and_interrupt_previous_sibling(
    line: &PhrasingContentLine,
    prev_sibling_token: &BlockToken,
) -> Option<EatAndInterruptPreviousSiblingResult> {
    let opener = eat_opener(line)?;
    let data = opener.token.data_as::<HtmlBlockTokenData>()?;
    if data.kind == HtmlBlockKind::Type7 {
        return None;
    }

    Some(EatAndInterruptPreviousSiblingResult {
        token: opener.token,
        next_index: opener.next_index,
        saturated: opener.saturated,
        remaining_sibling: RemainingSibling::One(prev_sibling_token.clone()),
    })
}

pub(crate) fn eat_continuation_text(
    line: &PhrasingContentLine,
    token: &mut BlockToken,
) -> EatContinuationTextResult {
    let Some(data) = token.data_as::<HtmlBlockTokenData>().cloned() else {
        return EatContinuationTextResult::NotMatched;
    };

    let source = calc_line_text(line);
    if data.kind.ends_on_blank_line() && source.trim().is_empty() {
        return EatContinuationTextResult::NotMatched;
    }

    let mut lines = data.lines;
    lines.push(line.clone());
    let should_close = !data.kind.ends_on_blank_line() && data.kind.is_closed_by_line(&source);

    token.data = Arc::new(HtmlBlockTokenData {
        kind: data.kind,
        lines,
    });
    update_token_end_position(token, line);

    if should_close {
        EatContinuationTextResult::Closing {
            next_index: line.end_index,
        }
    } else {
        EatContinuationTextResult::Opening {
            next_index: line.end_index,
        }
    }
}

pub(crate) fn detect_html_block_kind(line: &str) -> Option<HtmlBlockKind> {
    let line = line.trim_end_matches(['\n', '\r']);
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces >= 4 {
        return None;
    }

    let trimmed = &line[leading_spaces..];
    if !trimmed.starts_with('<') {
        return None;
    }

    if starts_type1_tag(trimmed, "pre") {
        return Some(HtmlBlockKind::Type1("pre".to_string()));
    }
    if starts_type1_tag(trimmed, "script") {
        return Some(HtmlBlockKind::Type1("script".to_string()));
    }
    if starts_type1_tag(trimmed, "style") {
        return Some(HtmlBlockKind::Type1("style".to_string()));
    }

    if trimmed.starts_with("<!--") {
        return Some(HtmlBlockKind::Type2);
    }
    if trimmed.starts_with("<?") {
        return Some(HtmlBlockKind::Type3);
    }
    if trimmed.starts_with("<![CDATA[") {
        return Some(HtmlBlockKind::Type5);
    }
    if starts_declaration(trimmed) {
        return Some(HtmlBlockKind::Type4);
    }

    if starts_type6_tag(trimmed) {
        return Some(HtmlBlockKind::Type6);
    }

    if is_complete_inline_html_tag_line(trimmed) {
        return Some(HtmlBlockKind::Type7);
    }

    None
}

fn calc_line_text(line: &PhrasingContentLine) -> String {
    calc_string_from_node_points(&line.node_points, line.start_index, line.end_index, false)
}

fn calc_line_position(line: &PhrasingContentLine) -> Option<yozora_ast::Position> {
    if line.start_index >= line.end_index {
        return None;
    }

    Some(yozora_ast::Position {
        start: calc_start_point(line.node_points.as_ref(), line.start_index),
        end: calc_end_point(line.node_points.as_ref(), line.end_index - 1),
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
    position.end = yozora_ast::Point {
        line: end.line,
        column: end.column + 1,
        offset: Some(end.offset + 1),
    };
}

fn starts_type1_tag(trimmed: &str, tag: &str) -> bool {
    let lower = trimmed.to_ascii_lowercase();
    let needle = format!("<{tag}");
    if !lower.starts_with(&needle) {
        return false;
    }

    let rest = &lower[needle.len()..];
    rest.is_empty()
        || rest.starts_with('>')
        || rest.starts_with('/')
        || rest.starts_with(char::is_whitespace)
}

fn starts_declaration(trimmed: &str) -> bool {
    trimmed
        .strip_prefix("<!")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn starts_type6_tag(trimmed: &str) -> bool {
    let mut s = trimmed;
    if let Some(rest) = s.strip_prefix("</") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix('<') {
        s = rest;
    } else {
        return false;
    }

    let mut name = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            name.push(ch);
            continue;
        }

        if name.is_empty() {
            return false;
        }

        if !is_html_block_tag(&name) {
            return false;
        }

        return ch == '>' || ch == '/' || ch.is_whitespace();
    }

    false
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn is_html_block_tag(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "body"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dialog"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hr"
            | "html"
            | "iframe"
            | "legend"
            | "li"
            | "link"
            | "main"
            | "menu"
            | "meta"
            | "nav"
            | "ol"
            | "p"
            | "section"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "ul"
    )
}

fn is_complete_inline_html_tag_line(line: &str) -> bool {
    let line = line.trim_end_matches(['\n', '\r']);
    let line = line.trim_end_matches([' ', '\t']);

    let Some(end) = find_tag_end(line, 0) else {
        return false;
    };

    end + 1 == line.len() && looks_like_inline_html_tag(line)
}

fn find_tag_end(input: &str, start: usize) -> Option<usize> {
    if input.as_bytes().get(start).copied()? != b'<' {
        return None;
    }

    let rest = &input[start..];
    if rest.starts_with("<!--") {
        return rest.find("-->").map(|idx| start + idx + 2);
    }
    if rest.starts_with("<![CDATA[") {
        return rest.find("]]>").map(|idx| start + idx + 2);
    }
    if rest.starts_with("<?") {
        return rest.find("?>").map(|idx| start + idx + 1);
    }

    let bytes = input.as_bytes();
    let mut i = start + 1;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }

        if b == b'"' || b == b'\'' {
            quote = Some(b);
            i += 1;
            continue;
        }

        if b == b'>' {
            return Some(i);
        }

        i += 1;
    }

    None
}

fn looks_like_inline_html_tag(candidate: &str) -> bool {
    if !(candidate.starts_with('<') && candidate.ends_with('>')) {
        return false;
    }

    if candidate.starts_with("<!--") {
        return is_valid_html_comment(candidate);
    }
    if candidate.starts_with("<![CDATA[") {
        return candidate.ends_with("]]>");
    }
    if candidate.starts_with("<?") {
        return candidate.ends_with("?>");
    }
    if candidate.starts_with("<!") {
        return !candidate.contains('\0');
    }

    let inner = &candidate[1..candidate.len() - 1];
    let bytes = inner.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    if bytes[0] == b'/' {
        return is_valid_closing_tag(inner);
    }

    is_valid_opening_tag(inner)
}

fn is_valid_html_comment(candidate: &str) -> bool {
    if candidate.len() < 7 || !candidate.starts_with("<!--") || !candidate.ends_with("-->") {
        return false;
    }

    let body = &candidate[4..candidate.len() - 3];
    !body.contains("--") && !body.ends_with('-')
}

fn is_valid_closing_tag(inner: &str) -> bool {
    let bytes = inner.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'/' || !bytes[1].is_ascii_alphabetic() {
        return false;
    }

    let mut i = 2usize;
    while i < bytes.len() && is_tag_name_char(bytes[i]) {
        i += 1;
    }
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i == bytes.len()
}

fn is_valid_opening_tag(inner: &str) -> bool {
    let bytes = inner.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_alphabetic() {
        return false;
    }

    let mut i = 1usize;
    while i < bytes.len() && is_tag_name_char(bytes[i]) {
        i += 1;
    }

    if i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' {
        return false;
    }

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            return true;
        }

        if bytes[i] == b'/' {
            return i + 1 == bytes.len();
        }

        if !is_attr_name_start(bytes[i]) {
            return false;
        }

        i += 1;
        while i < bytes.len() && is_attr_name_char(bytes[i]) {
            i += 1;
        }

        let attr_end = i;
        let mut after_attr = i;
        while after_attr < bytes.len() && bytes[after_attr].is_ascii_whitespace() {
            after_attr += 1;
        }

        if after_attr < bytes.len() && bytes[after_attr] == b'=' {
            i = after_attr + 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                return false;
            }

            let quote = bytes[i];
            if quote == b'"' || quote == b'\'' {
                i += 1;
                let mut found_closing_quote = false;
                while i < bytes.len() {
                    if bytes[i] == quote {
                        found_closing_quote = true;
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                if !found_closing_quote {
                    return false;
                }
            } else {
                let value_start = i;
                while i < bytes.len() && is_unquoted_attr_value_char(bytes[i]) {
                    i += 1;
                }
                if i == value_start {
                    return false;
                }
            }
        } else {
            i = attr_end;
        }

        if i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'/' {
            return false;
        }
    }

    true
}

fn is_tag_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-'
}

fn is_attr_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b':'
}

fn is_attr_name_char(b: u8) -> bool {
    is_attr_name_start(b) || b.is_ascii_digit() || b == b'.' || b == b'-'
}

fn is_unquoted_attr_value_char(b: u8) -> bool {
    !b.is_ascii_whitespace() && !matches!(b, b'"' | b'\'' | b'=' | b'<' | b'>' | b'`')
}
