use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use yozora_core_tokenizer::MatchBlockHook;

pub type SharedMatchBlockHook<'a> = Rc<RefCell<Box<dyn MatchBlockHook + 'a>>>;

#[derive(Clone)]
pub struct MatchBlockProcessorHook<'a> {
    pub name: Arc<str>,
    pub priority: i32,
    pub hook: SharedMatchBlockHook<'a>,
}

impl<'a> MatchBlockProcessorHook<'a> {
    pub fn new(name: impl Into<String>, priority: i32, hook: Box<dyn MatchBlockHook + 'a>) -> Self {
        Self {
            name: Arc::<str>::from(name.into()),
            priority,
            hook: Rc::new(RefCell::new(hook)),
        }
    }
}
