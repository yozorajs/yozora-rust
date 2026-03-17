use yozora_ast::Root;
use yozora_core_parser::{DefaultParser, ParseOptions};
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_tokenizer_autolink::AutolinkTokenizer;
use yozora_tokenizer_autolink_extension::AutolinkExtensionTokenizer;
use yozora_tokenizer_backslash_escape::BackslashEscapeTokenizer;
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
use yozora_tokenizer_list::ListTokenizer;
use yozora_tokenizer_paragraph::ParagraphTokenizer;
use yozora_tokenizer_setext_heading::SetextHeadingTokenizer;
use yozora_tokenizer_soft_break::SoftBreakTokenizer;
use yozora_tokenizer_table::TableTokenizer;
use yozora_tokenizer_text::TextTokenizer;
use yozora_tokenizer_thematic_break::ThematicBreakTokenizer;

pub struct GfmExParser {
    inner: DefaultParser,
}

impl Default for GfmExParser {
    fn default() -> Self {
        let mut inner = DefaultParser::new();
        register_gfm_ex_tokenizers(&mut inner);
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

fn register_gfm_ex_tokenizers(inner: &mut DefaultParser) {
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(BackslashEscapeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-backslash-escape");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(AutolinkTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-autolink");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(AutolinkExtensionTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-autolink-extension");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(BlockquoteTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-blockquote");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(BreakTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-break");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(DefinitionTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-definition");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(DeleteTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-delete");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(EmphasisTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-emphasis");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(FencedCodeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-fenced-code");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(HeadingTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-heading");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(HtmlBlockTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-html-block");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(HtmlInlineTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-html-inline");
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
            AnyTokenizer::Block(Box::new(IndentedCodeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-indented-code");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-inline-code");
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
            AnyTokenizer::Block(Box::new(ListTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-list");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(SetextHeadingTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-setext-heading");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(SoftBreakTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-soft-break");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(TableTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-table");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(ThematicBreakTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-thematic-break");
}

impl GfmExParser {
    pub fn parse(&self, input: &str, options: Option<ParseOptions>) -> Root {
        self.inner.parse(input, options)
    }

    pub fn inner_mut(&mut self) -> &mut DefaultParser {
        &mut self.inner
    }
}
