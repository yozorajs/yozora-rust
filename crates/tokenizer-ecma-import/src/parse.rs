use yozora_ast::{EcmaImport, Node};

use crate::r#match::EcmaImportToken;

pub(crate) fn parse_ecma_import_token(token: EcmaImportToken) -> Node {
    Node::EcmaImport(EcmaImport {
        position: None,
        module_name: token.module_name,
        default_import: token.default_import,
        named_imports: token.named_imports,
    })
}
