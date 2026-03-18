#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HtmlBlockToken {
    pub consumed_lines: usize,
    pub value: String,
}

#[derive(Debug, Clone)]
enum HtmlBlockKind {
    Type1(String),
    Type2,
    Type3,
    Type4,
    Type5,
    Type6,
    Type7,
}

impl HtmlBlockKind {
    fn ends_on_blank_line(&self) -> bool {
        matches!(self, Self::Type6 | Self::Type7)
    }

    fn is_closed_by_line(&self, line: &str) -> bool {
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

pub(crate) fn can_interrupt_paragraph_with_lines(lines: &[&str]) -> bool {
    let Some(first) = lines.first().copied() else {
        return false;
    };

    !matches!(detect_html_block_kind(first), Some(HtmlBlockKind::Type7))
}

pub(crate) fn match_html_block_token(lines: &[&str]) -> Option<HtmlBlockToken> {
    let first = *lines.first()?;
    let kind = detect_html_block_kind(first)?;

    let mut consumed_lines = 0usize;
    let mut body = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 && kind.ends_on_blank_line() && line.trim().is_empty() {
            break;
        }

        body.push((*line).to_string());
        consumed_lines += 1;

        if kind.is_closed_by_line(line) {
            break;
        }
    }

    if consumed_lines == 0 {
        return None;
    }

    let mut value = body.join("\n");
    if consumed_lines < lines.len() {
        value.push('\n');
    } else if matches!(kind, HtmlBlockKind::Type6)
        && should_append_unclosed_type7_newline(first, &body)
        && !value.ends_with('\n')
    {
        value.push('\n');
    }

    Some(HtmlBlockToken {
        consumed_lines,
        value,
    })
}

fn detect_html_block_kind(line: &str) -> Option<HtmlBlockKind> {
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

fn should_append_unclosed_type7_newline(first_line: &str, body: &[String]) -> bool {
    let trimmed = first_line.trim_start();
    if trimmed.starts_with("</") {
        return false;
    }

    if body.len() > 2 {
        return false;
    }

    if !is_complete_inline_html_tag_line(trimmed) {
        return false;
    }

    let Some(tag) = extract_opening_tag_name(trimmed) else {
        return false;
    };

    if body.len() == 1 && trimmed.trim_end().ends_with("/>") {
        return false;
    }

    let closing = format!("</{tag}");
    if contains_case_insensitive(trimmed, &closing) {
        return false;
    }

    !body
        .iter()
        .skip(1)
        .any(|line| contains_case_insensitive(line, &closing))
}

fn extract_opening_tag_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix('<')?;
    let mut name = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            name.push(ch);
            continue;
        }
        break;
    }

    if name.is_empty() {
        return None;
    }

    Some(name.to_ascii_lowercase())
}
