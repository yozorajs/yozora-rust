use yozora_character::{is_whitespace_character, AsciiCodePoint, NodePoint};

pub fn eat_start_condition6(
    node_points: &[NodePoint],
    start: usize,
    end: usize,
    tag: &str,
) -> Option<usize> {
    if !is_included_tag(tag) {
        return None;
    }
    if start >= end {
        return Some(end);
    }
    let c = node_points[start].code_point;
    if is_whitespace_character(c) || c == AsciiCodePoint::CLOSE_ANGLE as i32 {
        return Some(start + 1);
    }
    (c == AsciiCodePoint::SLASH as i32
        && start + 1 < end
        && node_points[start + 1].code_point == AsciiCodePoint::CLOSE_ANGLE as i32)
        .then_some(start + 2)
}

fn is_included_tag(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "blockquote"
            | "body"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
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
            | "menuitem"
            | "nav"
            | "noframes"
            | "ol"
            | "optgroup"
            | "option"
            | "p"
            | "param"
            | "section"
            | "source"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "track"
            | "ul"
    )
}
