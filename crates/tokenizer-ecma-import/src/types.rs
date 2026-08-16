use yozora_ast::EcmaImportNamedImport;

#[derive(Debug, Clone)]
pub struct EcmaImportTokenData {
    pub module_name: String,
    pub default_import: Option<String>,
    pub named_imports: Vec<EcmaImportNamedImport>,
}

pub const ECMA_IMPORT_TOKENIZER_NAME: &str = "@yozora/tokenizer-ecma-import";
