use yozora_ast::Root;
use yozora_core_parser::{DefaultParser, ParseContents, ParseOptions};
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

impl Default for GfmParser {
    fn default() -> Self {
        let mut inner = DefaultParser::new();
        register_gfm_tokenizers(&mut inner);
        inner
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                ParagraphTokenizer::default(),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
                TextTokenizer::default(),
            )));

        Self { inner }
    }
}

fn register_gfm_tokenizers(inner: &mut DefaultParser) {
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(IndentedCodeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-indented-code");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(HtmlBlockTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-html-block");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(SetextHeadingTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-setext-heading");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(ThematicBreakTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-thematic-break");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(BlockquoteTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-blockquote");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(ListTokenizer::new(ListTokenizerOptions {
                enable_task_list_item: false,
            }))),
            None,
        )
        .expect("failed to register tokenizer-list");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(HeadingTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-heading");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(FencedCodeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-fenced-code");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(DefinitionTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-definition");

    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(HtmlInlineTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-html-inline");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-inline-code");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(AutolinkTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-autolink");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(BreakTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-break");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(ImageTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-image");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(ImageReferenceTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-image-reference");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(LinkTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-link");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(LinkReferenceTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-link-reference");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(EmphasisTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-emphasis");
}

impl GfmParser {
    pub fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        self.inner
            .use_tokenizer(tokenizer, register_before_tokenizer)?;
        Ok(self)
    }

    pub fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        self.inner
            .replace_tokenizer(tokenizer, register_before_tokenizer)?;
        Ok(self)
    }

    pub fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.inner.unmount_tokenizer(tokenizer_name);
        self
    }

    pub fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        self.inner.use_fallback_tokenizer(tokenizer);
        self
    }

    pub fn set_default_parse_options(&mut self, options: ParseOptions) {
        self.inner.set_default_parse_options(options);
    }

    pub fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        self.inner.parse(contents, options)
    }

    #[allow(non_snake_case)]
    pub fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        self.use_tokenizer(tokenizer, register_before_tokenizer)
    }

    #[allow(non_snake_case)]
    pub fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        self.replace_tokenizer(tokenizer, register_before_tokenizer)
    }

    #[allow(non_snake_case)]
    pub fn unmountTokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.unmount_tokenizer(tokenizer_name)
    }

    #[allow(non_snake_case)]
    pub fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        self.use_fallback_tokenizer(tokenizer)
    }

    #[allow(non_snake_case)]
    pub fn setDefaultParseOptions(&mut self, options: ParseOptions) {
        self.set_default_parse_options(options)
    }

    pub fn inner_mut(&mut self) -> &mut DefaultParser {
        &mut self.inner
    }
}
