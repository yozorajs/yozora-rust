#[path = "ast/clone.rs"]
pub mod clone;
#[path = "ast/collect/mod.rs"]
pub mod collect;
pub mod definition;
pub mod excerpt;
pub mod footnote;
pub mod matcher;
#[path = "ast/mutate/mod.rs"]
pub mod mutate;
#[path = "ast/mutate_async/mod.rs"]
pub mod mutate_async;
#[path = "ast/position.rs"]
pub mod position;
#[path = "ast/search.rs"]
pub mod search;
#[path = "ast/toc.rs"]
pub mod toc;
#[path = "ast/traverse.rs"]
pub mod traverse;
pub mod url;

#[path = "ast/util.rs"]
mod tree;

pub use clone::shallow_clone_ast;
pub use collect::{
    collect_definitions, collect_footnote_definitions, collect_inline_nodes, collect_nodes,
    collect_nodes_from_node, collect_root_nodes, collect_texts, create_shallow_node_collector,
    inline_node_matcher, ShallowNode, ShallowNodeCollector,
};
pub use definition::{calc_definition_map, calc_identifier_set, DefinitionMapResult};
pub use excerpt::{calc_excerpt_ast, get_excerpt_ast};
pub use footnote::{
    calc_footnote_definition_map, replace_footnotes_in_references, FootnoteDefinitionMapResult,
    DEFAULT_FOOTNOTE_IDENTIFIER_PREFIX,
};
pub use matcher::{create_node_matcher, NodeMatcher};
pub use mutate::{
    mutate_node, mutate_root, shallow_mutate_ast_in_postorder, shallow_mutate_ast_in_preorder,
    NodeReplacement,
};
pub use mutate_async::{
    mutate_node_async, mutate_root_async, shallow_mutate_ast_in_postorder_async,
    shallow_mutate_ast_in_preorder_async, NodeReplacementFuture,
};
pub use position::remove_positions;
pub use search::search_node;
pub use toc::{
    calc_heading_identifiers, calc_heading_toc, calc_identifier_from_nodes, HeadingToc,
    HeadingTocNode,
};
pub use traverse::{traverse_ast, ParentRef};
pub use url::{
    default_url_resolver, resolve_urls_for_ast, resolve_urls_for_ast_matching,
    resolve_urls_for_ast_with, resolve_urls_for_ast_with_resolver, UrlResolver,
};
