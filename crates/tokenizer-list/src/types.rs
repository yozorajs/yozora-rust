use yozora_ast::{NodeType, TaskStatus, PARAGRAPH_TYPE};

#[derive(Debug, Clone)]
pub struct ListTokenData {
    pub _is_empty: bool,
    pub ordered: bool,
    pub marker: u32,
    pub order_type: Option<String>,
    pub order: Option<usize>,
    pub status: Option<TaskStatus>,
    pub indent: usize,
    pub count_of_top_blank_line: i32,
}

#[derive(Debug, Clone)]
pub struct ListTokenizerOptions {
    pub name: Option<String>,
    pub priority: Option<i32>,
    pub enable_task_list_item: bool,
    pub empty_item_could_not_interrupted_types: Vec<NodeType>,
}

impl Default for ListTokenizerOptions {
    fn default() -> Self {
        Self {
            name: None,
            priority: None,
            enable_task_list_item: false,
            empty_item_could_not_interrupted_types: vec![PARAGRAPH_TYPE],
        }
    }
}

pub const LIST_TOKENIZER_NAME: &str = "@yozora/tokenizer-list";
