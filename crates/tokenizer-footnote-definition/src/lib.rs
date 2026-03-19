mod r#match;
mod parse;
mod tokenizer;

pub use r#match::eat_footnote_label;
pub use tokenizer::{FootnoteDefinitionTokenizer, FOOTNOTE_DEFINITION_TOKENIZER_NAME};
