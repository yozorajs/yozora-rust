use yozora_ast::{Node, ECMA_IMPORT_TYPE};

use crate::{NodeMarkup, NodeMarkupWeaveContext, NodeWeaver};

pub struct EcmaImportWeaver;

impl NodeWeaver for EcmaImportWeaver {
    fn node_types(&self) -> Vec<&str> {
        vec![ECMA_IMPORT_TYPE]
    }

    fn is_block_level(&self, _: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> bool {
        true
    }

    fn weave(&self, node: &Node, _: &NodeMarkupWeaveContext<'_>, _: usize) -> NodeMarkup {
        let Node::EcmaImport(node) = node else {
            unreachable!()
        };
        let named = node
            .named_imports
            .iter()
            .map(|item| match item.alias.as_deref() {
                Some(alias) => format!("{} as {alias}", item.src),
                None => item.src.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let named = (!named.is_empty()).then(|| format!("{{ {named} }}"));
        let imports = match (node.default_import.as_deref(), named) {
            (Some(default_import), Some(named)) => Some(format!("{default_import}, {named}")),
            (Some(default_import), None) => Some(default_import.to_string()),
            (None, Some(named)) => Some(named),
            (None, None) => None,
        };
        NodeMarkup {
            opener: Some(match imports {
                Some(imports) => format!("import {imports} from \"{}\";", node.module_name),
                None => format!("import \"{}\";", node.module_name),
            }),
            ..NodeMarkup::default()
        }
    }
}
