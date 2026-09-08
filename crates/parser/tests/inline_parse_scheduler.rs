use std::cell::Cell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::{
    AnyTokenizer, InlineToken, InlineTokenizer, MatchInlineHook, MatchInlinePhaseApi,
    ParseInlineGenerator, ParseInlineGeneratorResult, ParseInlineHook, ParseInlineHookResult,
    ParseInlinePhaseApi, Tokenizer, TokenizerType,
};
use yozora_parser::YozoraParser;
use yozora_tokenizer_inline_code::InlineCodeTokenizer;

struct ScopedCodeTokenizer {
    inner: InlineCodeTokenizer,
    scopes: Rc<Cell<usize>>,
}

impl Tokenizer for ScopedCodeTokenizer {
    fn r#type(&self) -> TokenizerType {
        self.inner.r#type()
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn priority(&self) -> i32 {
        self.inner.priority()
    }
}

struct ScopedCodeHook<'a> {
    inner: Box<dyn ParseInlineHook + 'a>,
    count: Cell<usize>,
}

impl ParseInlineHook for ScopedCodeHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let mut nodes = self.inner.parse(tokens);
        for node in &mut nodes {
            if let Node::InlineCode(code) = node {
                self.count.set(self.count.get() + 1);
                code.value = format!("{}:{}", code.value, self.count.get());
            }
        }
        nodes
    }
}

impl InlineTokenizer for ScopedCodeTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        self.inner.r#match(api)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        self.scopes.set(self.scopes.get() + 1);
        Box::new(ScopedCodeHook {
            inner: self.inner.parse(api),
            count: Cell::new(0),
        })
    }
}

#[test]
fn custom_parse_hook_state_is_scoped_to_each_token_list() {
    let scopes = Rc::new(Cell::new(0));
    let mut parser = YozoraParser::default();
    parser.replace_tokenizer(
        AnyTokenizer::Inline(Box::new(ScopedCodeTokenizer {
            inner: InlineCodeTokenizer::default(),
            scopes: Rc::clone(&scopes),
        })),
        None,
    );
    let root = parser.parse("`a` ![`b`](image) `c` ![`d`](image)", None);
    let Node::Paragraph(paragraph) = &root.children[0] else {
        panic!("expected paragraph")
    };
    let values: Vec<_> = paragraph
        .children
        .iter()
        .filter_map(|node| match node {
            Node::InlineCode(code) => Some(code.value.as_str()),
            Node::Image(image) => Some(image.alt.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(values, ["a:1", "b:1", "c:2", "d:1"]);
    assert_eq!(scopes.get(), 3);
}

struct CyclicCodeTokenizer(InlineCodeTokenizer);

impl Tokenizer for CyclicCodeTokenizer {
    fn r#type(&self) -> TokenizerType {
        self.0.r#type()
    }
    fn name(&self) -> &str {
        self.0.name()
    }
    fn priority(&self) -> i32 {
        self.0.priority()
    }
}

struct CyclicGenerator<'a>(&'a [InlineToken]);

impl<'a> ParseInlineGenerator<'a> for CyclicGenerator<'a> {
    fn resume(&mut self, _: Option<Vec<Node>>) -> ParseInlineGeneratorResult<'a> {
        ParseInlineGeneratorResult::Yield(self.0)
    }
}

impl InlineTokenizer for CyclicCodeTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        self.0.r#match(api)
    }
    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        self.0.parse(api)
    }
    fn parse_deferred<'a>(
        &'a self,
        tokens: &'a [InlineToken],
        _: &'a dyn ParseInlinePhaseApi,
    ) -> Option<ParseInlineHookResult<'a>> {
        Some(ParseInlineHookResult::Generator(Box::new(CyclicGenerator(
            tokens,
        ))))
    }
}

#[test]
#[should_panic(expected = "[parseInline] cyclic token tree")]
fn rejects_a_generator_requesting_a_subslice_of_an_active_token_list() {
    let mut parser = YozoraParser::default();
    parser.replace_tokenizer(
        AnyTokenizer::Inline(Box::new(
            CyclicCodeTokenizer(InlineCodeTokenizer::default()),
        )),
        None,
    );
    // The yielded code batch starts inside the active [text, code] token list.
    // Comparing only whole-slice identities would miss this overlap.
    parser.parse("prefix `code`", None);
}
