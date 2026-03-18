use yozora_ast::{Html, Node, Text};

use crate::r#match::HtmlInlineToken;

pub(crate) fn parse_html_inline_tokens(input: &str, tokens: &[HtmlInlineToken]) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token {
            HtmlInlineToken::Text(interval) => {
                nodes.push(Node::Text(Text {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
            HtmlInlineToken::Html(interval) => {
                nodes.push(Node::Html(Html {
                    position: None,
                    value: input[interval.start_index..interval.end_index].to_string(),
                }));
            }
        }
    }

    nodes
}
