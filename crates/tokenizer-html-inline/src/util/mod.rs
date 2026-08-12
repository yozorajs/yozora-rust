mod cdata;
mod closing;
mod comment;
mod declaration;
mod instruction;
mod open;

pub use cdata::{
    eat_html_inline_cdata_delimiter, HtmlInlineCDataData, HtmlInlineCDataDelimiter,
    HtmlInlineCDataTokenData,
};
pub use closing::{
    eat_html_inline_closing_delimiter, HtmlInlineClosingDelimiter, HtmlInlineClosingTagData,
    HtmlInlineClosingTokenData,
};
pub use comment::{
    eat_html_inline_comment_delimiter, HtmlInlineCommentData, HtmlInlineCommentDelimiter,
    HtmlInlineCommentTokenData,
};
pub use declaration::{
    eat_html_inline_declaration_delimiter, HtmlInlineDeclarationData,
    HtmlInlineDeclarationDelimiter, HtmlInlineDeclarationTokenData,
};
pub use instruction::{
    eat_html_inline_instruction_delimiter, HtmlInlineInstructionData,
    HtmlInlineInstructionDelimiter, HtmlInlineInstructionTokenData,
};
pub use open::{
    eat_html_inline_token_open_delimiter, HtmlAttribute, HtmlInlineOpenDelimiter,
    HtmlInlineOpenTagData, HtmlInlineOpenTokenData,
};

use yozora_core_tokenizer::{DelimiterType, TokenDelimiter};

fn full_delimiter(start_index: usize, end_index: usize) -> TokenDelimiter {
    TokenDelimiter {
        delimiter_type: DelimiterType::Full,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
    }
}
