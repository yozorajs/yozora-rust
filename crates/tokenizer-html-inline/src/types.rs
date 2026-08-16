use crate::util::{
    HtmlInlineCDataDelimiter, HtmlInlineCDataTokenData, HtmlInlineClosingDelimiter,
    HtmlInlineClosingTokenData, HtmlInlineCommentDelimiter, HtmlInlineCommentTokenData,
    HtmlInlineDeclarationDelimiter, HtmlInlineDeclarationTokenData, HtmlInlineInstructionDelimiter,
    HtmlInlineInstructionTokenData, HtmlInlineOpenDelimiter, HtmlInlineOpenTokenData,
};
use yozora_core_tokenizer::TokenDelimiter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlInlineDelimiter {
    Open(HtmlInlineOpenDelimiter),
    Closing(HtmlInlineClosingDelimiter),
    Comment(HtmlInlineCommentDelimiter),
    Instruction(HtmlInlineInstructionDelimiter),
    Declaration(HtmlInlineDeclarationDelimiter),
    Cdata(HtmlInlineCDataDelimiter),
}

impl HtmlInlineDelimiter {
    pub fn delimiter(&self) -> &TokenDelimiter {
        match self {
            Self::Open(value) => &value.delimiter,
            Self::Closing(value) => &value.delimiter,
            Self::Comment(value) => &value.delimiter,
            Self::Instruction(value) => &value.delimiter,
            Self::Declaration(value) => &value.delimiter,
            Self::Cdata(value) => &value.delimiter,
        }
    }

    pub fn to_core(&self) -> TokenDelimiter {
        self.delimiter().clone()
    }

    pub fn token_data(&self) -> HtmlInlineTokenData {
        match self {
            Self::Open(value) => HtmlInlineTokenData::Open(HtmlInlineOpenTokenData {
                html_type: value.html_type,
                tag_name: value.tag_name,
                attributes: value.attributes.clone(),
                self_closed: value.self_closed,
            }),
            Self::Closing(value) => HtmlInlineTokenData::Closing(HtmlInlineClosingTokenData {
                html_type: value.html_type,
                tag_name: value.tag_name,
            }),
            Self::Comment(value) => HtmlInlineTokenData::Comment(HtmlInlineCommentTokenData {
                html_type: value.html_type,
            }),
            Self::Instruction(value) => {
                HtmlInlineTokenData::Instruction(HtmlInlineInstructionTokenData {
                    html_type: value.html_type,
                })
            }
            Self::Declaration(value) => {
                HtmlInlineTokenData::Declaration(HtmlInlineDeclarationTokenData {
                    html_type: value.html_type,
                    tag_name: value.tag_name,
                })
            }
            Self::Cdata(value) => HtmlInlineTokenData::Cdata(HtmlInlineCDataTokenData {
                html_type: value.html_type,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlInlineTokenData {
    Open(HtmlInlineOpenTokenData),
    Closing(HtmlInlineClosingTokenData),
    Comment(HtmlInlineCommentTokenData),
    Instruction(HtmlInlineInstructionTokenData),
    Declaration(HtmlInlineDeclarationTokenData),
    Cdata(HtmlInlineCDataTokenData),
}

pub const HTML_INLINE_TOKENIZER_NAME: &str = "@yozora/tokenizer-html-inline";
