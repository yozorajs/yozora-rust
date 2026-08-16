use yozora_ast::Root;
use yozora_core_parser::{DefaultParser, DefaultParserProps, ParseContents, ParseOptions, Parser};
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_tokenizer_autolink::AutolinkTokenizer;
use yozora_tokenizer_blockquote::BlockquoteTokenizer;
use yozora_tokenizer_break::BreakTokenizer;
use yozora_tokenizer_definition::DefinitionTokenizer;
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
use yozora_tokenizer_text::TextTokenizer;
use yozora_tokenizer_thematic_break::ThematicBreakTokenizer;

pub struct GfmParser {
    inner: DefaultParser,
}

pub type GfmParserProps = DefaultParserProps;

impl Default for GfmParser {
    fn default() -> Self {
        Self::new(DefaultParserProps::default())
    }
}

impl GfmParser {
    pub fn new(mut props: GfmParserProps) -> Self {
        props
            .block_fallback_tokenizer
            .get_or_insert_with(|| Box::new(ParagraphTokenizer::default()));
        props
            .inline_fallback_tokenizer
            .get_or_insert_with(|| Box::new(TextTokenizer::default()));
        let mut inner = DefaultParser::new(props);
        register_gfm_tokenizers(&mut inner);
        Self { inner }
    }
}

fn register_gfm_tokenizers(inner: &mut DefaultParser) {
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(IndentedCodeTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(HtmlBlockTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(SetextHeadingTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(ThematicBreakTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(BlockquoteTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(ListTokenizer::new(ListTokenizerOptions {
            enable_task_list_item: false,
            ..ListTokenizerOptions::default()
        }))),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(HeadingTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(FencedCodeTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Block(Box::new(DefinitionTokenizer::default())),
        None,
    );

    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(HtmlInlineTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(AutolinkTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(BreakTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(ImageTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(ImageReferenceTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(LinkTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(LinkReferenceTokenizer::default())),
        None,
    );
    inner.use_tokenizer(
        AnyTokenizer::Inline(Box::new(EmphasisTokenizer::default())),
        None,
    );
}

impl GfmParser {
    pub fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        self.inner
            .use_tokenizer(tokenizer, register_before_tokenizer);
        self
    }

    pub fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        self.inner
            .replace_tokenizer(tokenizer, register_before_tokenizer);
        self
    }

    pub fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.inner.unmount_tokenizer(tokenizer_name);
        self
    }

    pub fn unmount_block_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.inner.unmount_block_tokenizer(tokenizer_name);
        self
    }

    pub fn unmount_inline_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.inner.unmount_inline_tokenizer(tokenizer_name);
        self
    }

    pub fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        self.inner.use_fallback_tokenizer(tokenizer);
        self
    }

    pub fn set_default_parse_options(&mut self, options: Option<ParseOptions>) {
        self.inner.set_default_parse_options(options);
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

impl Parser for GfmParser {
    fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        GfmParser::use_tokenizer(self, tokenizer, register_before_tokenizer)
    }

    fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        GfmParser::replace_tokenizer(self, tokenizer, register_before_tokenizer)
    }

    fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        GfmParser::unmount_tokenizer(self, tokenizer_name)
    }

    fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        GfmParser::use_fallback_tokenizer(self, tokenizer)
    }

    fn set_default_parse_options(&mut self, options: Option<ParseOptions>) {
        GfmParser::set_default_parse_options(self, options)
    }

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        GfmParser::parse(self, contents, options)
    }
}
