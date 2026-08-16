use yozora_ast::{EcmaImport, Node};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

use crate::types::EcmaImportTokenData;

pub(crate) fn parse_ecma_import_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<EcmaImportTokenData>() else {
            continue;
        };

        nodes.push(Node::EcmaImport(EcmaImport {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            module_name: data.module_name.clone(),
            default_import: data.default_import.clone(),
            named_imports: data.named_imports.clone(),
        }));
    }

    nodes
}
