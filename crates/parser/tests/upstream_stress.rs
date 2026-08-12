use yozora_ast::{Image, Node};
use yozora_core_parser::ParseOptions;
use yozora_core_tokenizer::{
    AnyTokenizer, InlineToken, InlineTokenizer, MatchInlineHook, MatchInlinePhaseApi,
    ParseInlineHook, ParseInlinePhaseApi, Tokenizer, TokenizerType,
};
use yozora_parser::YozoraParser;
use yozora_tokenizer_image::{ImageTokenizer, IMAGE_TOKENIZER_NAME};
use yozora_tokenizer_image_reference::IMAGE_REFERENCE_TOKENIZER_NAME;

struct ShallowImageTokenizer {
    inner: ImageTokenizer,
}

impl Tokenizer for ShallowImageTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Inline
    }

    fn name(&self) -> &str {
        IMAGE_TOKENIZER_NAME
    }

    fn priority(&self) -> i32 {
        self.inner.priority()
    }
}

struct ShallowImageParseHook;

impl ParseInlineHook for ShallowImageParseHook {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        tokens
            .iter()
            .map(|_| {
                Node::Image(Image {
                    position: None,
                    url: String::new(),
                    title: None,
                    alt: String::new(),
                })
            })
            .collect()
    }
}

impl InlineTokenizer for ShallowImageTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        self.inner.r#match(api)
    }

    fn parse<'a>(&'a self, _: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(ShallowImageParseHook)
    }
}

#[test]
fn parses_deeply_nested_images_without_stack_overflow() {
    let depth = std::env::var("YOZORA_STRESS_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let source = format!("{}x{}", "![".repeat(depth), "](/url)".repeat(depth));
    let mut parser = YozoraParser::default();
    parser.replace_tokenizer(
        AnyTokenizer::Inline(Box::new(ShallowImageTokenizer {
            inner: ImageTokenizer::default(),
        })),
        Some(IMAGE_REFERENCE_TOKENIZER_NAME),
    );
    let ast = parser.parse(
        source,
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph")
    };
    assert!(matches!(paragraph.children.as_slice(), [Node::Image(_)]));
}

#[test]
fn parses_deeply_nested_blocks_without_stack_overflow() {
    let depth = std::env::var("YOZORA_STRESS_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let source = format!("{}x", "> ".repeat(depth));
    let ast = YozoraParser::default().parse(
        source,
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );

    let mut node = ast.children.first().expect("expected nested blockquote");
    for _ in 0..depth {
        let Node::Blockquote(blockquote) = node else {
            panic!("expected blockquote")
        };
        node = blockquote
            .children
            .first()
            .expect("expected nested block child");
    }
    assert!(matches!(node, Node::Paragraph(_)));
}

#[test]
fn handles_unmatched_cross_tokenizer_delimiters_linearly() {
    let count = std::env::var("YOZORA_CROSS_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000);
    let source = "[x]: <".repeat(count);
    let mut parser = YozoraParser::default();
    if let Ok(names) = std::env::var("YOZORA_UNMOUNT") {
        for name in names.split(',').filter(|name| !name.is_empty()) {
            parser.unmount_tokenizer(name);
        }
    }
    let ast = parser.parse(
        source.clone(),
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Paragraph(paragraph)) = ast.children.first() else {
        panic!("expected paragraph")
    };
    assert!(matches!(paragraph.children.as_slice(), [Node::Text(text)] if text.value == source));
}

#[test]
fn materializes_wide_sibling_lists() {
    let count = std::env::var("YOZORA_WIDE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(150_000);
    let ast = YozoraParser::default().parse(
        "x\n\n".repeat(count),
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    assert_eq!(ast.children.len(), count);
    assert!(matches!(ast.children.first(), Some(Node::Paragraph(_))));
    assert!(matches!(ast.children.last(), Some(Node::Paragraph(_))));
}

#[test]
fn materializes_wide_sibling_lists_inside_container() {
    let count = std::env::var("YOZORA_WIDE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(150_000);
    let ast = YozoraParser::default().parse(
        "> x\n>\n".repeat(count),
        Some(ParseOptions {
            should_reserve_position: Some(false),
            ..ParseOptions::default()
        }),
    );
    let Some(Node::Blockquote(blockquote)) = ast.children.first() else {
        panic!("expected blockquote")
    };
    assert_eq!(ast.children.len(), 1);
    assert_eq!(blockquote.children.len(), count);
    assert!(matches!(
        blockquote.children.first(),
        Some(Node::Paragraph(_))
    ));
    assert!(matches!(
        blockquote.children.last(),
        Some(Node::Paragraph(_))
    ));
}
