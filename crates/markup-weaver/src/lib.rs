mod default_markup_weaver;
mod markup_weaver;
mod types;
pub mod util;
pub mod weaver;

pub use default_markup_weaver::DefaultMarkupWeaver;
pub use markup_weaver::MarkupWeaver;
pub use types::{
    Ancestor, Escaper, MarkupWeaverContract, NodeMarkup, NodeMarkupWeaveContext, NodeWeaver,
};
pub use util::{create_character_escaper, find_max_continuous_symbol, minmax, split_lines};
pub use weaver::*;
