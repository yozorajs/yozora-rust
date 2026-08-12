use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use yozora_ast::{Node, Root, ROOT_TYPE};

use crate::{Ancestor, Escaper, MarkupWeaverContract, NodeMarkupWeaveContext, NodeWeaver};

#[derive(Clone)]
struct MarkupToken<'a> {
    children: &'a [Node],
    indent: String,
    spread: bool,
}

struct EscaperState {
    depth: usize,
    index: usize,
}

struct WeaveState<'a> {
    ancestors: Vec<Ancestor<'a>>,
    escapers: Vec<Escaper>,
    escaper_state_map: HashMap<String, EscaperState>,
}

struct Output {
    lines: Vec<String>,
    should_expose_line_start: bool,
}

impl Output {
    fn new() -> Self {
        Self {
            lines: vec![String::new()],
            should_expose_line_start: false,
        }
    }

    fn current(&self) -> &str {
        self.lines.last().expect("output should contain one line")
    }

    fn current_mut(&mut self) -> &mut String {
        self.lines
            .last_mut()
            .expect("output should contain one line")
    }

    fn append(&mut self, value: &str, indent: &str) {
        let parts = value.split('\n').collect::<Vec<_>>();
        if parts.is_empty() {
            return;
        }

        self.current_mut().push_str(parts[0]);
        for part in &parts[1..] {
            self.lines.push(format!("{indent}{part}"));
        }
        self.should_expose_line_start = parts.len() > 1 && parts.last() == Some(&"");
    }
}

#[derive(Default)]
pub struct MarkupWeaver {
    weaver_map: HashMap<String, Arc<dyn NodeWeaver>>,
}

impl MarkupWeaver {
    pub fn new() -> Self {
        Self::default()
    }

    fn enqueue(&self, node: &Node, weaver: &Arc<dyn NodeWeaver>, state: &mut WeaveState<'_>) {
        let Some(escaper) = weaver.escape_content() else {
            return;
        };
        let node_type = node.node_type();
        if let Some(existing) = state.escaper_state_map.get_mut(node_type) {
            existing.depth += 1;
            return;
        }

        state.escaper_state_map.insert(
            node_type.to_string(),
            EscaperState {
                depth: 1,
                index: state.escapers.len(),
            },
        );
        state.escapers.push(escaper);
    }

    fn dequeue(&self, node: &Node, state: &mut WeaveState<'_>) {
        let node_type = node.node_type();
        let Some(existing) = state.escaper_state_map.get_mut(node_type) else {
            return;
        };
        existing.depth -= 1;
        if existing.depth > 0 {
            return;
        }

        let index = existing.index;
        state.escaper_state_map.remove(node_type);
        if index + 1 == state.escapers.len() {
            state.escapers.pop();
        }
    }

    fn escape_content(&self, content: &str, state: &WeaveState<'_>) -> String {
        state
            .escapers
            .iter()
            .rev()
            .fold(content.to_string(), |value, escape| escape(&value))
    }

    fn context<'a>(&'a self, state: &'a RefCell<WeaveState<'a>>) -> NodeMarkupWeaveContext<'a> {
        NodeMarkupWeaveContext::new(state.borrow().ancestors.clone(), move |nodes| {
            self.weave_inline_nodes_with_state(nodes, state)
        })
    }

    fn process_node<'a>(
        &'a self,
        node: &'a Node,
        parent: &MarkupToken<'a>,
        child_index: usize,
        state: &'a RefCell<WeaveState<'a>>,
        output: &mut Output,
    ) {
        let weaver = self
            .weaver_map
            .get(node.node_type())
            .unwrap_or_else(|| {
                panic!(
                    "[MarkupWeaver.weave] Cannot recognize node type({})",
                    node.node_type()
                )
            })
            .clone();
        self.enqueue(node, &weaver, &mut state.borrow_mut());

        let context = self.context(state);
        let is_block_level = weaver.is_block_level(node, &context, child_index);
        let markup = weaver.weave(node, &context, child_index);
        let indent = format!(
            "{}{}",
            parent.indent,
            markup.indent.as_deref().unwrap_or_default()
        );

        if let Some(opener) = markup.opener.as_deref() {
            output.append(opener, &indent);
        }

        if let Some(content) = markup.content.as_deref() {
            let is_at_line_start = output.should_expose_line_start && output.current() == indent;
            let exposed = if is_at_line_start {
                format!("\n{content}")
            } else {
                content.to_string()
            };
            let escaped = self.escape_content(&exposed, &state.borrow());
            let escaped = if is_at_line_start {
                escaped.strip_prefix('\n').unwrap_or(&escaped)
            } else {
                &escaped
            };
            output.append(escaped, &indent);
        } else if let Some(children) = node.children() {
            let token = MarkupToken {
                children,
                indent: indent.clone(),
                spread: markup.spread.unwrap_or(parent.spread),
            };
            state.borrow_mut().ancestors.push(Ancestor::Node(node));

            let mut previous_is_block = true;
            for (index, child) in children.iter().enumerate() {
                let child_weaver = self.weaver_map.get(child.node_type()).unwrap_or_else(|| {
                    panic!(
                        "[MarkupWeaver.weave] Cannot recognize node type({})",
                        child.node_type()
                    )
                });
                let child_context = self.context(state);
                let next_is_block = child_weaver.is_block_level(child, &child_context, index);
                if next_is_block && !previous_is_block {
                    output.lines.push(indent.clone());
                }
                previous_is_block = next_is_block;
                self.process_node(child, &token, index, state, output);
            }

            state.borrow_mut().ancestors.pop();
        }

        if let Some(closer) = markup.closer.as_deref() {
            output.append(closer, &indent);
        }

        if is_block_level && child_index + 1 < parent.children.len() {
            output.should_expose_line_start = false;
            if output.current() == indent || output.current() == parent.indent {
                *output.current_mut() = parent.indent.clone();
            } else {
                output.lines.push(parent.indent.clone());
            }
            if parent.spread {
                output.lines.push(parent.indent.clone());
            }
        }

        self.dequeue(node, &mut state.borrow_mut());
    }

    fn weave_inline_nodes_with_state<'a>(
        &'a self,
        nodes: &'a [Node],
        state: &'a RefCell<WeaveState<'a>>,
    ) -> String {
        let mut result = String::new();
        for (child_index, node) in nodes.iter().enumerate() {
            let weaver = self
                .weaver_map
                .get(node.node_type())
                .unwrap_or_else(|| {
                    panic!(
                        "[MarkupWeaver.weave] Cannot recognize node type({})",
                        node.node_type()
                    )
                })
                .clone();
            self.enqueue(node, &weaver, &mut state.borrow_mut());

            let context = self.context(state);
            if weaver.is_block_level(node, &context, child_index) {
                self.dequeue(node, &mut state.borrow_mut());
                panic!("[MarkupWeaver.weave] Cannot processInline for block-level node.");
            }

            let markup = weaver.weave(node, &context, child_index);
            if let Some(opener) = markup.opener {
                result.push_str(&opener);
            }
            if let Some(content) = markup.content {
                result.push_str(&self.escape_content(&content, &state.borrow()));
            } else if let Some(children) = node.children() {
                state.borrow_mut().ancestors.push(Ancestor::Node(node));
                result.push_str(&self.weave_inline_nodes_with_state(children, state));
                state.borrow_mut().ancestors.pop();
            }
            if let Some(closer) = markup.closer {
                result.push_str(&closer);
            }

            self.dequeue(node, &mut state.borrow_mut());
        }
        result
    }
}

impl MarkupWeaverContract for MarkupWeaver {
    fn use_weaver(&mut self, weaver: Box<dyn NodeWeaver>, force_replace: bool) -> &mut Self {
        let weaver = Arc::<dyn NodeWeaver>::from(weaver);
        let node_types = weaver
            .node_types()
            .into_iter()
            .map(str::to_string)
            .collect::<std::collections::HashSet<_>>();
        for node_type in node_types {
            if force_replace || !self.weaver_map.contains_key(&node_type) {
                self.weaver_map.insert(node_type, weaver.clone());
            } else {
                eprintln!("[useWeaver] Type({node_type}) has been registered.");
            }
        }
        self
    }

    fn unmount_weaver(&mut self, node_type: &str) -> &mut Self {
        self.weaver_map.remove(node_type);
        self
    }

    fn weave(&self, ast: &Root) -> String {
        let root_escaper = self
            .weaver_map
            .get(ROOT_TYPE)
            .and_then(|weaver| weaver.escape_content());
        let state = RefCell::new(WeaveState {
            ancestors: vec![Ancestor::Root(ast)],
            escapers: root_escaper.into_iter().collect(),
            escaper_state_map: HashMap::new(),
        });
        let mut output = Output::new();
        let root = MarkupToken {
            children: &ast.children,
            indent: String::new(),
            spread: true,
        };
        for (index, node) in ast.children.iter().enumerate() {
            self.process_node(node, &root, index, &state, &mut output);
        }
        output.lines.join("\n")
    }
}
