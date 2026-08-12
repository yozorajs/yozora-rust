mod r#match;
mod parse;
mod tokenizer;
mod util;

pub use tokenizer::{LinkTokenizer, LINK_TOKENIZER_NAME};
pub use util::{eat_link_destination, eat_link_title};
pub use yozora_core_tokenizer::check_balanced_brackets_status;

pub type LinkToken = yozora_core_tokenizer::InlineToken;
pub type LinkHookContext = LinkTokenizer;
pub type LinkTokenizerProps = yozora_core_tokenizer::TokenizerOptions;
