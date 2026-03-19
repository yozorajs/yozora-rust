#![allow(non_snake_case)]

use yozora_ast::Root;
use yozora_core_parser::{DefaultParser, DefaultParserProps, ParseContents, ParseOptions};
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_tokenizer_admonition::AdmonitionTokenizer;
use yozora_tokenizer_autolink::AutolinkTokenizer;
use yozora_tokenizer_autolink_extension::AutolinkExtensionTokenizer;
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
use yozora_tokenizer_inline_math::{InlineMathTokenizer, InlineMathTokenizerOptions};
use yozora_tokenizer_link::LinkTokenizer;
use yozora_tokenizer_link_reference::LinkReferenceTokenizer;
use yozora_tokenizer_list::{ListTokenizer, ListTokenizerOptions};
use yozora_tokenizer_math::MathTokenizer;
use yozora_tokenizer_paragraph::ParagraphTokenizer;
use yozora_tokenizer_setext_heading::SetextHeadingTokenizer;
use yozora_tokenizer_table::TableTokenizer;
use yozora_tokenizer_text::TextTokenizer;
use yozora_tokenizer_thematic_break::ThematicBreakTokenizer;

pub struct YozoraParser {
    inner: DefaultParser,
}

impl Default for YozoraParser {
    fn default() -> Self {
        let mut inner = DefaultParser::new(DefaultParserProps {
            blockFallbackTokenizer: Some(Box::new(ParagraphTokenizer::default())),
            inlineFallbackTokenizer: Some(Box::new(TextTokenizer::default())),
            defaultParseOptions: None,
        });
        register_builtin_tokenizers(&mut inner);

        Self { inner }
    }
}

fn register_builtin_tokenizers(inner: &mut DefaultParser) {
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(IndentedCodeTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(HtmlBlockTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(SetextHeadingTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(ThematicBreakTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(BlockquoteTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(ListTokenizer::new(ListTokenizerOptions {
                enable_task_list_item: true,
                ..ListTokenizerOptions::default()
            }))),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(HeadingTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(FencedCodeTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(AdmonitionTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(MathTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(FootnoteDefinitionTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(DefinitionTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(EcmaImportTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Block(Box::new(TableTokenizer::default())),
            None,
        );

    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(HtmlInlineTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(InlineMathTokenizer::new(
                InlineMathTokenizerOptions {
                    backtick_required: true,
                },
            ))),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(InlineCodeTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(AutolinkTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(AutolinkExtensionTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(BreakTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(FootnoteTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(FootnoteReferenceTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(ImageTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(ImageReferenceTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(LinkTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(LinkReferenceTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(InlineMathTokenizer::new(
                InlineMathTokenizerOptions {
                    backtick_required: false,
                },
            ))),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(EmphasisTokenizer::default())),
            None,
        );
    inner
        .useTokenizer(
            AnyTokenizer::Inline(Box::new(DeleteTokenizer::default())),
            None,
        );
}

impl YozoraParser {
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
        self.inner.replaceTokenizer(tokenizer, registerBeforeTokenizer);
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use yozora_ast::{Association, Node};

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

    #[test]
    fn parse_and_camel_case_methods_work() {
        let mut parser = YozoraParser::default();
        parser.setDefaultParseOptions(Some(ParseOptions {
            shouldReservePosition: Some(true),
            ..ParseOptions::default()
        }));

        let lines = ["# hello", "", "world"];
        let root = parser.parse(ParseContents::Chunks(&lines), None);
        assert!(root.position.is_some());
        assert_eq!(root.children.len(), 1);
    }

    #[test]
    fn parse_accepts_iterable_like_lines() {
        let parser = YozoraParser::default();
        let root = parser.parse(vec!["# title", "", "content"], None);
        assert_eq!(root.children.len(), 1);
    }

    #[test]
    fn parse_options_preset_definitions_resolve_link_references() {
        let parser = YozoraParser::default();

        let without_preset = parser.parse("[foo][]", None);
        let Node::Paragraph(paragraph) = &without_preset.children[0] else {
            panic!("expected paragraph node");
        };
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Text(_))));

        let with_preset = parser.parse(
            "[foo][]",
            Some(ParseOptions {
                presetDefinitions: Some(vec![Association {
                    identifier: "foo".to_string(),
                    label: "foo".to_string(),
                }]),
                ..ParseOptions::default()
            }),
        );
        let Node::Paragraph(paragraph) = &with_preset.children[0] else {
            panic!("expected paragraph node");
        };
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::LinkReference(_))));
    }

    #[test]
    fn parse_options_preset_footnotes_resolve_footnote_references() {
        let parser = YozoraParser::default();

        let without_preset = parser.parse("[^n]", None);
        let Node::Paragraph(paragraph) = &without_preset.children[0] else {
            panic!("expected paragraph node");
        };
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::Text(_))));

        let with_preset = parser.parse(
            "[^n]",
            Some(ParseOptions {
                presetFootnoteDefinitions: Some(vec![Association {
                    identifier: "n".to_string(),
                    label: "n".to_string(),
                }]),
                ..ParseOptions::default()
            }),
        );
        let Node::Paragraph(paragraph) = &with_preset.children[0] else {
            panic!("expected paragraph node");
        };
        assert!(paragraph
            .children
            .iter()
            .any(|node| matches!(node, Node::FootnoteReference(_))));
    }

    #[test]
    fn parse_options_format_url_is_applied_to_url_nodes() {
        let parser = YozoraParser::default();
        let format_url = Arc::new(|url: &str| format!("prefix:{url}"));

        let root = parser.parse(
            "[link](/u) ![alt](/img)\n\n[foo]: /def",
            Some(ParseOptions {
                formatUrl: Some(format_url),
                ..ParseOptions::default()
            }),
        );

        let Node::Paragraph(paragraph) = &root.children[0] else {
            panic!("expected paragraph node");
        };
        let link = paragraph
            .children
            .iter()
            .find_map(|node| match node {
                Node::Link(link) => Some(link),
                _ => None,
            })
            .expect("expected link node");
        assert_eq!(link.url, "prefix:/u");

        let image = paragraph
            .children
            .iter()
            .find_map(|node| match node {
                Node::Image(image) => Some(image),
                _ => None,
            })
            .expect("expected image node");
        assert_eq!(image.url, "prefix:/img");

        let definition = root
            .children
            .iter()
            .find_map(|node| match node {
                Node::Definition(definition) => Some(definition),
                _ => None,
            })
            .expect("expected definition node");
        assert_eq!(definition.url, "prefix:/def");
    }
}
