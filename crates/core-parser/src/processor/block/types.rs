use std::cell::RefCell;
use std::rc::Rc;

use yozora_core_tokenizer::MatchBlockHook;

pub type SharedMatchBlockHook<'a> = Rc<RefCell<Box<dyn MatchBlockHook + 'a>>>;

#[derive(Clone)]
pub struct MatchBlockProcessorHook<'a> {
    pub name: String,
    pub priority: i32,
    pub hook: SharedMatchBlockHook<'a>,
}

impl<'a> MatchBlockProcessorHook<'a> {
    pub fn new(name: impl Into<String>, priority: i32, hook: Box<dyn MatchBlockHook + 'a>) -> Self {
        Self {
            name: name.into(),
            priority,
            hook: Rc::new(RefCell::new(hook)),
        }
    }
}
