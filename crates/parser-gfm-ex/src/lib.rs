#![allow(non_snake_case)]

use yozora_ast::Root;
use yozora_core_parser::{DefaultParser, DefaultParserProps, ParseContents, ParseOptions};
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_tokenizer_autolink::AutolinkTokenizer;
use yozora_tokenizer_autolink_extension::AutolinkExtensionTokenizer;
use yozora_tokenizer_blockquote::BlockquoteTokenizer;
use yozora_tokenizer_break::BreakTokenizer;
use yozora_tokenizer_definition::DefinitionTokenizer;
use yozora_tokenizer_delete::DeleteTokenizer;
use yozora_tokenizer_emphasis::EmphasisTokenizer;
use yozora_tokenizer_fenced_code::FencedCodeTokenizer;
use yozora_tokenizer_heading::HeadingTokenizer;
use yozora_tokenizer_html_block::HtmlBlockTokenizer;
use yozora_tokenizer_html_inline::HtmlInlineTokenizer;
use yozora_tokenizer_image::ImageTokenizer;
use yozora_tokenizer_image_reference::ImageReferenceTokenizer;
use yozora_tokenizer_indented_code::IndentedCodeTokenizer;
use yozora_tokenizer_inline_code::InlineCodeTokenizer;
use yozora_tokenizer_link::LinkTokenizer;
use yozora_tokenizer_link_reference::LinkReferenceTokenizer;
use yozora_tokenizer_list::{ListTokenizer, ListTokenizerOptions};
use yozora_tokenizer_paragraph::ParagraphTokenizer;
use yozora_tokenizer_setext_heading::SetextHeadingTokenizer;
use yozora_tokenizer_table::TableTokenizer;
use yozora_tokenizer_text::TextTokenizer;
use yozora_tokenizer_thematic_break::ThematicBreakTokenizer;

pub struct GfmExParser {
    inner: DefaultParser,
}

impl Default for GfmExParser {
    fn default() -> Self {
        let mut inner = DefaultParser::new(DefaultParserProps {
            blockFallbackTokenizer: Some(Box::new(ParagraphTokenizer::default())),
            inlineFallbackTokenizer: Some(Box::new(TextTokenizer::default())),
            defaultParseOptions: None,
        });
        register_gfm_ex_tokenizers(&mut inner);

        Self { inner }
    }
}

fn register_gfm_ex_tokenizers(inner: &mut DefaultParser) {
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(IndentedCodeTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(HtmlBlockTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(SetextHeadingTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(ThematicBreakTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(BlockquoteTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(ListTokenizer::new(ListTokenizerOptions {
            enable_task_list_item: true,
            ..ListTokenizerOptions::default()
        }))),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(HeadingTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(FencedCodeTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(DefinitionTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Block(Box::new(TableTokenizer::default())),
        None,
    );

    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(HtmlInlineTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(AutolinkTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(AutolinkExtensionTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(BreakTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(ImageTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(ImageReferenceTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(LinkTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(LinkReferenceTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(EmphasisTokenizer::default())),
        None,
    );
    inner.useTokenizer(
        AnyTokenizer::Inline(Box::new(DeleteTokenizer::default())),
        None,
    );
}

impl GfmExParser {
    pub fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self {
        self.inner.useTokenizer(tokenizer, registerBeforeTokenizer);
        self
    }

    pub fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self {
        self.inner
            .replaceTokenizer(tokenizer, registerBeforeTokenizer);
        self
    }

    pub fn unmountTokenizer(&mut self, tokenizerName: &str) -> &mut Self {
        self.inner.unmountTokenizer(tokenizerName);
        self
    }

    pub fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        self.inner.useFallbackTokenizer(tokenizer);
        self
    }

    pub fn setDefaultParseOptions(&mut self, options: Option<ParseOptions>) {
        self.inner.setDefaultParseOptions(options);
    }

    pub fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        self.inner.parse(contents, options)
    }

    pub fn inner_mut(&mut self) -> &mut DefaultParser {
        &mut self.inner
    }
}
