use yozora_ast::{Html, Node};
use yozora_core_tokenizer::BlockTokenizeResult;

use crate::r#match::HtmlBlockToken;

pub(crate) fn parse_html_block_token(token: HtmlBlockToken) -> BlockTokenizeResult {
    BlockTokenizeResult {
        node: Node::Html(Html {
            position: None,
            value: token.value,
        }),
        consumed_lines: token.consumed_lines,
    }
}
