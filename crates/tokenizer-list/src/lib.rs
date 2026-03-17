use yozora_ast::{List, ListItem, Node, TaskStatus, Text};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta,
    leading_indent_columns, split_indent_prefix, strip_indent_columns,
    strip_indent_columns_with_base,
};

pub const LIST_TOKENIZER_NAME: &str = "@yozora/tokenizer-list";

#[derive(Debug, Clone)]
pub struct ListTokenizer {
    meta: TokenizerMeta,
}

impl Default for ListTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: LIST_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 6,
            },
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
        let Some(first) = lines.first().copied() else {
            return false;
        };
        let Some(marker) = parse_list_item_marker(first) else {
            return false;
        };

        if marker.content.is_empty() {
            return false;
        }

        !marker.ordered || marker.start == Some(1)
    }

    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        _position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first_line = *lines.first()?;
        let first_marker = parse_list_item_marker(first_line)?;

        let mut children = Vec::new();
        let mut consumed_lines = 0usize;
        let mut spread = false;

        while consumed_lines < lines.len() {
            let line = lines[consumed_lines];
            if line.trim().is_empty() {
                break;
            }

            let Some(marker) = parse_list_item_marker(line) else {
                break;
            };
            if !is_same_list_kind(&first_marker, &marker) {
                break;
            }

            let (item, item_consumed, item_has_internal_blank) =
                consume_list_item(lines, consumed_lines, &marker);
            if item_consumed == 0 {
                break;
            }

            if item_has_internal_blank {
                spread = true;
            }

            children.push(Node::ListItem(item));
            consumed_lines += item_consumed;

            let mut blank_count = 0usize;
            while consumed_lines + blank_count < lines.len()
                && lines[consumed_lines + blank_count].trim().is_empty()
            {
                blank_count += 1;
            }

            if blank_count == 0 {
                continue;
            }

            let next_index = consumed_lines + blank_count;
            let Some(next_line) = lines.get(next_index).copied() else {
                break;
            };
            let Some(next_marker) = parse_list_item_marker(next_line) else {
                break;
            };
            if !is_same_list_kind(&first_marker, &next_marker) {
                break;
            }

            spread = true;
            consumed_lines = next_index;
        }

        if children.is_empty() {
            return None;
        }

        Some(BlockTokenizeResult {
            node: Node::List(List {
                position: None,
                ordered: first_marker.ordered,
                order_type: first_marker.order_type,
                start: first_marker.start,
                marker: first_marker.marker,
                spread,
                children,
            }),
            consumed_lines,
        })
    }
}

#[derive(Debug, Clone)]
struct ParsedListMarker {
    indent: usize,
    ordered: bool,
    marker: u32,
    order_type: Option<String>,
    start: Option<usize>,
    marker_width: usize,
    padding: usize,
    content: String,
}

fn consume_list_item(
    lines: &[&str],
    start_index: usize,
    marker: &ParsedListMarker,
) -> (ListItem, usize, bool) {
    let mut logical_lines = Vec::new();
    let mut consumed_lines = 1usize;
    let mut index = start_index + 1;
    let mut saw_internal_blank_line = false;
    let mut saw_non_blank_content = !marker.content.is_empty();
    let mut count_of_top_blank_line = if marker.content.is_empty() { 1usize } else { 0usize };
    let mut fence_state: Option<(char, usize)> = None;
    let mut previous_was_list_marker_line = is_list_marker_like(&marker.content);

    logical_lines.push(marker.content.clone());
    update_fence_state(&marker.content, &mut fence_state);

    let content_indent = if marker.content.is_empty() {
        marker.indent + marker.marker_width + 1
    } else {
        marker.indent + marker.marker_width + marker.padding
    };

    while index < lines.len() {
        let line = lines[index];

        if line.trim().is_empty() {
            if !saw_non_blank_content {
                count_of_top_blank_line += 1;
                if count_of_top_blank_line > 1 {
                    break;
                }
            }

            if !should_consume_blank_in_item(lines, index + 1, content_indent) {
                break;
            }

            logical_lines.push(String::new());
            let mut significant_blank = fence_state.is_none();
            if significant_blank
                && previous_was_list_marker_line
                && next_non_empty_leading_spaces(lines, index + 1)
                    .is_some_and(|leading| leading > content_indent + 1)
            {
                significant_blank = false;
            }
            if significant_blank {
                saw_internal_blank_line = true;
            }
            index += 1;
            consumed_lines += 1;
            continue;
        }

        let leading_spaces = count_leading_spaces(line);

        if leading_spaces < content_indent {
            if !saw_non_blank_content || !can_lazy_continue(line, marker, content_indent) {
                break;
            }

            let content_line = normalize_lazy_content(line);
            logical_lines.push(content_line);
            previous_was_list_marker_line = logical_lines
                .last()
                .is_some_and(|line| is_list_marker_like(line.as_str()));
            if let Some(last) = logical_lines.last() {
                update_fence_state(last, &mut fence_state);
            }
            saw_non_blank_content = true;
            index += 1;
            consumed_lines += 1;
            continue;
        }

        let content_line = strip_n_spaces(line, content_indent);

        logical_lines.push(content_line);
        previous_was_list_marker_line = logical_lines
            .last()
            .is_some_and(|line| is_list_marker_like(line.as_str()));
        if let Some(last) = logical_lines.last() {
            update_fence_state(last, &mut fence_state);
        }
        saw_non_blank_content = true;
        index += 1;
        consumed_lines += 1;
    }

    while logical_lines.last().is_some_and(|line| line.is_empty()) {
        logical_lines.pop();
    }

    let (status, first_line) =
        parse_task_status(logical_lines.first().map(String::as_str).unwrap_or(""));
    if logical_lines.is_empty() {
        logical_lines.push(first_line);
    } else {
        logical_lines[0] = first_line;
    }

    let value = logical_lines.join("\n");
    let children = if value.is_empty() {
        Vec::new()
    } else {
        vec![Node::Text(Text {
            position: None,
            value,
        })]
    };

    (
        ListItem {
            position: None,
            status,
            children,
        },
        consumed_lines,
        saw_internal_blank_line,
    )
}

fn should_consume_blank_in_item(lines: &[&str], from: usize, content_indent: usize) -> bool {
    let mut index = from;
    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            index += 1;
            continue;
        }

        return count_leading_spaces(line) >= content_indent;
    }

    false
}

fn next_non_empty_leading_spaces(lines: &[&str], from: usize) -> Option<usize> {
    let mut index = from;
    while index < lines.len() {
        let line = lines[index];
        if !line.trim().is_empty() {
            return Some(count_leading_spaces(line));
        }
        index += 1;
    }
    None
}

fn normalize_lazy_content(line: &str) -> String {
    let content = line.trim_start_matches([' ', '\t']);
    if content.is_empty() {
        return String::new();
    }

    if starts_list_marker(content)
        || is_thematic_break_line(content)
        || is_setext_underline(content)
        || is_fenced_code_marker_line(content)
        || content.starts_with('#')
        || content.starts_with('>')
    {
        return format!("\\{content}");
    }

    content.to_string()
}

fn starts_list_marker(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    if matches!(bytes[0], b'-' | b'+' | b'*') {
        return bytes.len() == 1 || bytes[1] == b' ' || bytes[1] == b'\t';
    }

    let mut idx = 0usize;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }

    if idx == 0 || idx > 9 || idx >= bytes.len() {
        return false;
    }

    if bytes[idx] != b'.' && bytes[idx] != b')' {
        return false;
    }

    idx += 1;
    idx >= bytes.len() || bytes[idx] == b' ' || bytes[idx] == b'\t'
}

fn is_list_marker_like(line: &str) -> bool {
    parse_list_item_marker(line).is_some() || starts_list_marker(line.trim_start())
}

fn update_fence_state(line: &str, state: &mut Option<(char, usize)>) {
    let trimmed = line.trim_start();
    let Some(first) = trimmed.chars().next() else {
        return;
    };
    if first != '`' && first != '~' {
        return;
    }

    let run_len = trimmed.chars().take_while(|ch| *ch == first).count();
    if run_len < 3 {
        return;
    }

    match *state {
        Some((marker, min_len)) => {
            if marker == first && run_len >= min_len && trimmed[run_len..].trim().is_empty() {
                *state = None;
            }
        }
        None => {
            *state = Some((first, run_len));
        }
    }
}

fn can_lazy_continue(line: &str, _marker: &ParsedListMarker, content_indent: usize) -> bool {
    let leading_spaces = count_leading_spaces(line);
    if leading_spaces >= content_indent {
        return false;
    }

    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    if leading_spaces >= 4 || line.starts_with('\t') {
        return true;
    }

    if parse_list_item_marker(line).is_some() {
        return false;
    }

    if is_thematic_break_line(trimmed)
        || is_setext_underline(trimmed)
        || is_fenced_code_marker_line(trimmed)
        || trimmed.starts_with('#')
        || trimmed.starts_with('>')
    {
        return false;
    }

    true
}

fn is_setext_underline(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let Some(marker) = trimmed.chars().next() else {
        return false;
    };
    if marker != '=' && marker != '-' {
        return false;
    }

    trimmed.chars().all(|ch| ch == marker)
}

fn is_fenced_code_marker_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn parse_list_item_marker(line: &str) -> Option<ParsedListMarker> {
    let (indent, indent_bytes) = split_indent_prefix(line, 0);
    if indent >= 4 {
        return None;
    }

    let trimmed = &line[indent_bytes..];
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    if let Some(marker) = parse_bullet_marker(trimmed, indent) {
        return Some(marker);
    }

    parse_ordered_marker(trimmed, indent)
}

fn parse_bullet_marker(trimmed: &str, indent: usize) -> Option<ParsedListMarker> {
    if is_thematic_break_line(trimmed) {
        return None;
    }

    let mut chars = trimmed.chars();
    let bullet = chars.next()?;
    if !matches!(bullet, '-' | '+' | '*') {
        return None;
    }

    let rest = chars.as_str();
    let (padding, content) = parse_padding_and_content(rest, indent + 1)?;

    Some(ParsedListMarker {
        indent,
        ordered: false,
        marker: bullet as u32,
        order_type: None,
        start: None,
        marker_width: 1,
        padding,
        content,
    })
}

fn is_thematic_break_line(trimmed: &str) -> bool {
    let mut marker = None;
    let mut count = 0usize;

    for ch in trimmed.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }

        if !matches!(ch, '-' | '_' | '*') {
            break;
        }

        if marker.is_none() {
            marker = Some(ch);
        } else if marker != Some(ch) {
            return false;
        }

        count += 1;
    }

    marker.is_some() && count >= 3
}

fn parse_ordered_marker(trimmed: &str, indent: usize) -> Option<ParsedListMarker> {
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let (marker_width, start, order_type) = if bytes[0].is_ascii_digit() {
        let mut marker_width = 0usize;
        while marker_width < bytes.len() && bytes[marker_width].is_ascii_digit() {
            marker_width += 1;
        }
        if marker_width == 0 || marker_width > 9 {
            return None;
        }

        let start = trimmed[..marker_width].parse::<usize>().ok();
        (marker_width, start, Some("1".to_string()))
    } else {
        let ch = trimmed.chars().next()?;
        if ch.is_ascii_lowercase() {
            (
                ch.len_utf8(),
                Some((ch as u8 - b'a' + 1) as usize),
                Some("a".to_string()),
            )
        } else if ch.is_ascii_uppercase() {
            (
                ch.len_utf8(),
                Some((ch as u8 - b'A' + 1) as usize),
                Some("A".to_string()),
            )
        } else {
            return None;
        }
    };

    let separator = *bytes.get(marker_width)?;
    if separator != b'.' && separator != b')' {
        return None;
    }

    let rest = &trimmed[marker_width + 1..];
    let (padding, content) = parse_padding_and_content(rest, indent + marker_width + 1)?;

    Some(ParsedListMarker {
        indent,
        ordered: true,
        marker: separator as u32,
        order_type,
        start,
        marker_width: marker_width + 1,
        padding,
        content,
    })
}

fn parse_padding_and_content(rest: &str, base_column: usize) -> Option<(usize, String)> {
    if rest.is_empty() {
        return Some((1, String::new()));
    }

    let Some(first) = rest.chars().next() else {
        return Some((1, String::new()));
    };
    if first != ' ' && first != '\t' {
        return None;
    }

    let (padding, padding_bytes) = split_indent_prefix(rest, base_column);

    if padding <= 4 {
        let content = rest[padding_bytes..].to_string();
        return Some((padding.max(1), content));
    }

    // Item starting with indented code: marker only consumes one following space.
    let content = strip_indent_columns_with_base(rest, 1, base_column)?;
    Some((1, content))
}

fn is_same_list_kind(left: &ParsedListMarker, right: &ParsedListMarker) -> bool {
    if left.ordered != right.ordered {
        return false;
    }

    if left.ordered {
        left.marker == right.marker && left.order_type == right.order_type
    } else {
        left.marker == right.marker
    }
}

fn parse_task_status(content: &str) -> (Option<TaskStatus>, String) {
    if let Some(rest) = content.strip_prefix("[ ] ") {
        return (Some(TaskStatus::Todo), rest.to_string());
    }
    if let Some(rest) = content.strip_prefix("[x] ") {
        return (Some(TaskStatus::Done), rest.to_string());
    }
    if let Some(rest) = content.strip_prefix("[X] ") {
        return (Some(TaskStatus::Done), rest.to_string());
    }
    if let Some(rest) = content.strip_prefix("[-] ") {
        return (Some(TaskStatus::Doing), rest.to_string());
    }

    (None, content.to_string())
}

fn count_leading_spaces(line: &str) -> usize {
    leading_indent_columns(line)
}

fn strip_n_spaces(line: &str, n: usize) -> String {
    strip_indent_columns(line, n).unwrap_or_else(|| line.to_string())
}
