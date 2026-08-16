use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use yozora_ast::{Blockquote, Node};
use yozora_core_parser::ParseOptions;
use yozora_core_tokenizer::{
    AnyTokenizer, BlockToken, BlockTokenizer, MatchBlockHook, MatchBlockPhaseApi, ParseBlockError,
    ParseBlockGenerator, ParseBlockGeneratorResult, ParseBlockGeneratorResume, ParseBlockHook,
    ParseBlockHookResult, ParseBlockPhaseApi, ParseBlockResult, Tokenizer, TokenizerType,
};
use yozora_parser_gfm::GfmParser;
use yozora_tokenizer_blockquote::{BlockquoteTokenizer, BLOCKQUOTE_TOKENIZER_NAME};

struct TrackingBlockquoteTokenizer {
    inner: BlockquoteTokenizer,
    events: Rc<RefCell<Vec<String>>>,
    should_request_children: bool,
}

impl Tokenizer for TrackingBlockquoteTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        BLOCKQUOTE_TOKENIZER_NAME
    }

    fn priority(&self) -> i32 {
        self.inner.priority()
    }
}

impl BlockTokenizer for TrackingBlockquoteTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        self.inner.r#match(api)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(TrackingParseHook {
            api,
            events: Rc::clone(&self.events),
            should_request_children: self.should_request_children,
        })
    }
}

struct TrackingParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
    events: Rc<RefCell<Vec<String>>>,
    should_request_children: bool,
}

impl ParseBlockHook for TrackingParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(ParseBlockHookResult::Generator(Box::new(
            TrackingParseGenerator {
                api: self.api,
                events: Rc::clone(&self.events),
                should_request_children: self.should_request_children,
                tokens,
                next_token_index: 0,
                pending_token_index: None,
                nodes: Vec::with_capacity(tokens.len()),
                started: false,
            },
        )))
    }
}

struct TrackingParseGenerator<'a> {
    api: &'a dyn ParseBlockPhaseApi,
    events: Rc<RefCell<Vec<String>>>,
    should_request_children: bool,
    tokens: &'a [BlockToken],
    next_token_index: usize,
    pending_token_index: Option<usize>,
    nodes: Vec<Node>,
    started: bool,
}

impl<'a> ParseBlockGenerator<'a> for TrackingParseGenerator<'a> {
    fn resume(
        &mut self,
        state: ParseBlockGeneratorResume,
    ) -> ParseBlockResult<ParseBlockGeneratorResult<'a>> {
        if let Some(token_index) = self.pending_token_index.take() {
            let children = match state {
                ParseBlockGeneratorResume::Nodes(nodes) => nodes,
                ParseBlockGeneratorResume::Error(error) => return Err(error),
                ParseBlockGeneratorResume::Initial => {
                    return Err(ParseBlockError::new(
                        "tracking generator missed child nodes",
                    ));
                }
            };
            let token = &self.tokens[token_index];
            self.events
                .borrow_mut()
                .push(format!("exit:{}", token_column(token)));
            self.nodes.push(blockquote_node(self.api, token, children));
            self.next_token_index = token_index + 1;
        } else if !self.started {
            if !matches!(state, ParseBlockGeneratorResume::Initial) {
                return Err(ParseBlockError::new(
                    "tracking generator did not start from initial state",
                ));
            }
            self.started = true;
        } else {
            return Err(ParseBlockError::new(
                "tracking generator resumed unexpectedly",
            ));
        }

        while self.next_token_index < self.tokens.len() {
            let token_index = self.next_token_index;
            let token = &self.tokens[token_index];
            self.events
                .borrow_mut()
                .push(format!("enter:{}", token_column(token)));
            if self.should_request_children {
                self.pending_token_index = Some(token_index);
                return Ok(ParseBlockGeneratorResult::Yield(
                    self.api.request_block_tokens(Some(&token.children)),
                ));
            }

            self.events
                .borrow_mut()
                .push(format!("exit:{}", token_column(token)));
            self.nodes
                .push(blockquote_node(self.api, token, Vec::new()));
            self.next_token_index += 1;
        }

        Ok(ParseBlockGeneratorResult::Complete(std::mem::take(
            &mut self.nodes,
        )))
    }
}

struct RecoveringBlockquoteTokenizer {
    inner: BlockquoteTokenizer,
    events: Rc<RefCell<Vec<String>>>,
}

impl Tokenizer for RecoveringBlockquoteTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        BLOCKQUOTE_TOKENIZER_NAME
    }

    fn priority(&self) -> i32 {
        self.inner.priority()
    }
}

impl BlockTokenizer for RecoveringBlockquoteTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        self.inner.r#match(api)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(RecoveringParseHook {
            api,
            events: Rc::clone(&self.events),
        })
    }
}

struct RecoveringParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
    events: Rc<RefCell<Vec<String>>>,
}

impl ParseBlockHook for RecoveringParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(ParseBlockHookResult::Generator(Box::new(
            RecoveringParseGenerator {
                api: self.api,
                events: Rc::clone(&self.events),
                tokens,
                waiting_for_child: false,
                started: false,
            },
        )))
    }
}

struct RecoveringParseGenerator<'a> {
    api: &'a dyn ParseBlockPhaseApi,
    events: Rc<RefCell<Vec<String>>>,
    tokens: &'a [BlockToken],
    waiting_for_child: bool,
    started: bool,
}

impl<'a> ParseBlockGenerator<'a> for RecoveringParseGenerator<'a> {
    fn resume(
        &mut self,
        state: ParseBlockGeneratorResume,
    ) -> ParseBlockResult<ParseBlockGeneratorResult<'a>> {
        let token = self
            .tokens
            .first()
            .ok_or_else(|| ParseBlockError::new("expected blockquote token"))?;
        let column = token_column(token);

        if self.waiting_for_child {
            self.waiting_for_child = false;
            let children = match state {
                ParseBlockGeneratorResume::Nodes(nodes) => nodes,
                ParseBlockGeneratorResume::Error(_) => {
                    self.events.borrow_mut().push(format!("catch:{column}"));
                    Vec::new()
                }
                ParseBlockGeneratorResume::Initial => {
                    return Err(ParseBlockError::new(
                        "recovering generator missed child result",
                    ));
                }
            };
            self.events.borrow_mut().push(format!("finally:{column}"));
            return Ok(ParseBlockGeneratorResult::Complete(vec![blockquote_node(
                self.api, token, children,
            )]));
        }

        if self.started || !matches!(state, ParseBlockGeneratorResume::Initial) {
            return Err(ParseBlockError::new(
                "recovering generator resumed unexpectedly",
            ));
        }
        self.started = true;
        self.events.borrow_mut().push(format!("enter:{column}"));
        if column > 1 {
            self.events.borrow_mut().push(format!("finally:{column}"));
            return Err(ParseBlockError::new("nested blockquote failed"));
        }

        self.waiting_for_child = true;
        Ok(ParseBlockGeneratorResult::Yield(
            self.api.request_block_tokens(Some(&token.children)),
        ))
    }
}

struct CyclicBlockquoteTokenizer {
    inner: BlockquoteTokenizer,
}

impl Tokenizer for CyclicBlockquoteTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        BLOCKQUOTE_TOKENIZER_NAME
    }

    fn priority(&self) -> i32 {
        self.inner.priority()
    }
}

impl BlockTokenizer for CyclicBlockquoteTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        self.inner.r#match(api)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(CyclicParseHook { api })
    }
}

struct CyclicParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for CyclicParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(ParseBlockHookResult::Generator(Box::new(
            CyclicParseGenerator {
                api: self.api,
                tokens,
                started: false,
            },
        )))
    }
}

struct CyclicParseGenerator<'a> {
    api: &'a dyn ParseBlockPhaseApi,
    tokens: &'a [BlockToken],
    started: bool,
}

impl<'a> ParseBlockGenerator<'a> for CyclicParseGenerator<'a> {
    fn resume(
        &mut self,
        state: ParseBlockGeneratorResume,
    ) -> ParseBlockResult<ParseBlockGeneratorResult<'a>> {
        if !self.started && matches!(state, ParseBlockGeneratorResume::Initial) {
            self.started = true;
            return Ok(ParseBlockGeneratorResult::Yield(
                self.api.request_block_tokens(Some(self.tokens)),
            ));
        }
        match state {
            ParseBlockGeneratorResume::Error(error) => Err(error),
            _ => Err(ParseBlockError::new(
                "cyclic generator resumed unexpectedly",
            )),
        }
    }
}

#[test]
fn lazily_parses_children_in_parent_before_child_order() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut parser = GfmParser::default();
    parser.replace_tokenizer(
        AnyTokenizer::Block(Box::new(TrackingBlockquoteTokenizer {
            inner: BlockquoteTokenizer::default(),
            events: Rc::clone(&events),
            should_request_children: true,
        })),
        None,
    );

    let ast = parser.parse(">> x", Some(reserve_position()));
    assert!(matches!(
        ast.children.as_slice(),
        [Node::Blockquote(Blockquote { children, .. })]
            if matches!(children.as_slice(), [Node::Blockquote(_)])
    ));
    assert_eq!(
        events.borrow().as_slice(),
        ["enter:1", "enter:2", "exit:2", "exit:1"]
    );
}

#[test]
fn does_not_parse_children_that_parent_does_not_request() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut parser = GfmParser::default();
    parser.replace_tokenizer(
        AnyTokenizer::Block(Box::new(TrackingBlockquoteTokenizer {
            inner: BlockquoteTokenizer::default(),
            events: Rc::clone(&events),
            should_request_children: false,
        })),
        None,
    );

    let ast = parser.parse(">> x", Some(reserve_position()));
    assert!(matches!(
        ast.children.as_slice(),
        [Node::Blockquote(Blockquote { children, .. })] if children.is_empty()
    ));
    assert_eq!(events.borrow().as_slice(), ["enter:1", "exit:1"]);
}

#[test]
fn throws_child_failure_back_into_parent_generator() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut parser = GfmParser::default();
    parser.replace_tokenizer(
        AnyTokenizer::Block(Box::new(RecoveringBlockquoteTokenizer {
            inner: BlockquoteTokenizer::default(),
            events: Rc::clone(&events),
        })),
        None,
    );

    let ast = parser.parse(">> x", Some(reserve_position()));
    assert!(matches!(
        ast.children.as_slice(),
        [Node::Blockquote(Blockquote { children, .. })] if children.is_empty()
    ));
    assert_eq!(
        events.borrow().as_slice(),
        ["enter:1", "enter:2", "finally:2", "catch:1", "finally:1"]
    );
}

#[test]
fn rejects_cyclic_block_token_requests() {
    let mut parser = GfmParser::default();
    parser.replace_tokenizer(
        AnyTokenizer::Block(Box::new(CyclicBlockquoteTokenizer {
            inner: BlockquoteTokenizer::default(),
        })),
        None,
    );

    let panic = catch_unwind(AssertUnwindSafe(|| {
        parser.parse("> x", Some(reserve_position()));
    }))
    .expect_err("cyclic token request should panic at the parser boundary");
    assert_eq!(
        panic_message(panic),
        "[parseBlock] cyclic or shared token tree at tokenizer '@yozora/tokenizer-blockquote'"
    );
}

fn blockquote_node(api: &dyn ParseBlockPhaseApi, token: &BlockToken, children: Vec<Node>) -> Node {
    Node::Blockquote(Blockquote {
        position: if api.should_reserve_position() {
            token.position.clone()
        } else {
            None
        },
        children,
    })
}

fn token_column(token: &BlockToken) -> usize {
    token
        .position
        .as_ref()
        .expect("block token should carry position")
        .start
        .column
}

fn reserve_position() -> ParseOptions {
    ParseOptions {
        should_reserve_position: Some(true),
        ..ParseOptions::default()
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "non-string panic".to_string(),
        },
    }
}
