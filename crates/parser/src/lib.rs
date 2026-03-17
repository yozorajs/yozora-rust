use yozora_ast::Root;
use yozora_core_parser::{DefaultParser, ParseOptions};
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_tokenizer_admonition::AdmonitionTokenizer;
use yozora_tokenizer_autolink::AutolinkTokenizer;
use yozora_tokenizer_autolink_extension::AutolinkExtensionTokenizer;
use yozora_tokenizer_backslash_escape::BackslashEscapeTokenizer;
use yozora_tokenizer_blockquote::BlockquoteTokenizer;
use yozora_tokenizer_break::BreakTokenizer;
use yozora_tokenizer_definition::DefinitionTokenizer;
use yozora_tokenizer_delete::DeleteTokenizer;
use yozora_tokenizer_ecma_import::EcmaImportTokenizer;
use yozora_tokenizer_emphasis::EmphasisTokenizer;
use yozora_tokenizer_fenced_code::FencedCodeTokenizer;
use yozora_tokenizer_footnote::FootnoteTokenizer;
use yozora_tokenizer_footnote_definition::FootnoteDefinitionTokenizer;
use yozora_tokenizer_footnote_reference::FootnoteReferenceTokenizer;
use yozora_tokenizer_heading::HeadingTokenizer;
use yozora_tokenizer_html_block::HtmlBlockTokenizer;
use yozora_tokenizer_html_inline::HtmlInlineTokenizer;
use yozora_tokenizer_image::ImageTokenizer;
use yozora_tokenizer_image_reference::ImageReferenceTokenizer;
use yozora_tokenizer_indented_code::IndentedCodeTokenizer;
use yozora_tokenizer_inline_code::InlineCodeTokenizer;
use yozora_tokenizer_inline_math::InlineMathTokenizer;
use yozora_tokenizer_link::LinkTokenizer;
use yozora_tokenizer_link_reference::LinkReferenceTokenizer;
use yozora_tokenizer_list::ListTokenizer;
use yozora_tokenizer_math::MathTokenizer;
use yozora_tokenizer_paragraph::ParagraphTokenizer;
use yozora_tokenizer_setext_heading::SetextHeadingTokenizer;
use yozora_tokenizer_soft_break::SoftBreakTokenizer;
use yozora_tokenizer_table::TableTokenizer;
use yozora_tokenizer_text::TextTokenizer;
use yozora_tokenizer_thematic_break::ThematicBreakTokenizer;

pub struct YozoraParser {
    inner: DefaultParser,
}

impl Default for YozoraParser {
    fn default() -> Self {
        let mut inner = DefaultParser::new();
        register_builtin_tokenizers(&mut inner);
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

fn register_builtin_tokenizers(inner: &mut DefaultParser) {
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(BackslashEscapeTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-backslash-escape");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(AdmonitionTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-admonition");
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
            AnyTokenizer::Block(Box::new(EcmaImportTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-ecma-import");
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
            AnyTokenizer::Inline(Box::new(FootnoteTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-footnote");
    inner
        .use_tokenizer(
            AnyTokenizer::Block(Box::new(FootnoteDefinitionTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-footnote-definition");
    inner
        .use_tokenizer(
            AnyTokenizer::Inline(Box::new(FootnoteReferenceTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-footnote-reference");
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
            AnyTokenizer::Inline(Box::new(InlineMathTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-inline-math");
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
            AnyTokenizer::Block(Box::new(MathTokenizer::default())),
            None,
        )
        .expect("failed to register tokenizer-math");
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

impl YozoraParser {
    pub fn parse(&self, input: &str, options: Option<ParseOptions>) -> Root {
        self.inner.parse(input, options)
    }

    pub fn inner_mut(&mut self) -> &mut DefaultParser {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::Node;

    #[test]
    fn parse_heading_block() {
        let parser = YozoraParser::default();
        let root = parser.parse("## hello", None);

        assert_eq!(root.children.len(), 1);
        let Node::Heading(heading) = &root.children[0] else {
            panic!("expected heading node");
        };
        assert_eq!(heading.depth, 2);
    }

    #[test]
    fn parse_thematic_break_block() {
        let parser = YozoraParser::default();
        let root = parser.parse("---", None);

        assert_eq!(root.children.len(), 1);
        let Node::ThematicBreak(_) = &root.children[0] else {
            panic!("expected thematicBreak node");
        };
    }

    #[test]
    fn parse_list_block() {
        let parser = YozoraParser::default();
        let root = parser.parse("- a\n- b", None);

        assert_eq!(root.children.len(), 1);
        let Node::List(list) = &root.children[0] else {
            panic!("expected list node");
        };
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn parse_emphasis_and_inline_code_in_paragraph() {
        let parser = YozoraParser::default();
        let root = parser.parse("*a* `b`", None);

        let Node::Paragraph(paragraph) = &root.children[0] else {
            panic!("expected paragraph node");
        };
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Emphasis(_))));
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::InlineCode(_))));
    }

    #[test]
    fn parse_setext_heading_block() {
        let parser = YozoraParser::default();
        let root = parser.parse("Foo\n---", None);

        assert_eq!(root.children.len(), 1);
        let Node::Heading(heading) = &root.children[0] else {
            panic!("expected heading node");
        };
        assert_eq!(heading.depth, 2);
    }

    #[test]
    fn parse_fenced_code_block() {
        let parser = YozoraParser::default();
        let root = parser.parse("```rs\nlet x = 1;\n```", None);

        assert_eq!(root.children.len(), 1);
        let Node::Code(code) = &root.children[0] else {
            panic!("expected code node");
        };
        assert_eq!(code.lang.as_deref(), Some("rs"));
        assert_eq!(code.value, "let x = 1;\n");
    }

    #[test]
    fn parse_blockquote_with_nested_heading() {
        let parser = YozoraParser::default();
        let root = parser.parse("> # Foo\n> bar\n> baz", None);

        assert_eq!(root.children.len(), 1);
        let Node::Blockquote(blockquote) = &root.children[0] else {
            panic!("expected blockquote node");
        };
        assert!(blockquote
            .children
            .iter()
            .any(|node| matches!(node, Node::Heading(_))));
    }

    #[test]
    fn parse_inline_link_image_autolink_html() {
        let parser = YozoraParser::default();
        let root = parser.parse(
            "[link](/uri \"title\") ![foo](/url \"title\") <http://foo.bar.baz> <a>",
            None,
        );

        let Node::Paragraph(paragraph) = &root.children[0] else {
            panic!("expected paragraph node");
        };

        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Link(_))));
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Image(_))));
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Html(_))));
    }

    #[test]
    fn parse_delete_break_reference_and_definition() {
        let parser = YozoraParser::default();
        let root = parser.parse("~~Hi~~\\\n[foo][]\n\n[foo]: /url \"title\"", None);

        assert!(root
            .children
            .iter()
            .any(|node| matches!(node, Node::Definition(_))));

        let Node::Paragraph(paragraph) = &root.children[0] else {
            panic!("expected paragraph node");
        };

        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Delete(_))));
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Break(_))));
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::LinkReference(_))));
    }

    #[test]
    fn parse_custom_math_and_footnote() {
        let parser = YozoraParser::default();
        let root = parser.parse("$x^2$ ^[n]", None);

        let Node::Paragraph(paragraph) = &root.children[0] else {
            panic!("expected paragraph node");
        };

        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::InlineMath(_))));
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Footnote(_))));
    }

}
