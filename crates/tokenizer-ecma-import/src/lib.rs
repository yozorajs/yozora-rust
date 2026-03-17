use yozora_ast::{EcmaImport, EcmaImportNamedImport, Node};
use yozora_core_tokenizer::{BlockTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const ECMA_IMPORT_TOKENIZER_NAME: &str = "@yozora/tokenizer-ecma-import";

#[derive(Debug, Clone)]
pub struct EcmaImportTokenizer {
    meta: TokenizerMeta,
}

impl Default for EcmaImportTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: ECMA_IMPORT_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 11,
            },
        }
    }
}

impl Tokenizer for EcmaImportTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for EcmaImportTokenizer {
    fn tokenize_block(&self, input: &str, _position: Option<yozora_ast::Position>) -> Option<Node> {
        if input.contains('\n') {
            return None;
        }

        let statement = input.trim();
        let statement = statement.strip_suffix(';').unwrap_or(statement);
        let rest = statement.strip_prefix("import ")?.trim();

        if let Some(module_name) = parse_quoted_literal(rest) {
            return Some(Node::EcmaImport(EcmaImport {
                position: None,
                module_name,
                default_import: None,
                named_imports: Vec::new(),
            }));
        }

        let (spec, from_part) = rest.split_once(" from ")?;
        let module_name = parse_quoted_literal(from_part.trim())?;

        let (default_import, named_imports) = parse_import_spec(spec.trim())?;
        Some(Node::EcmaImport(EcmaImport {
            position: None,
            module_name,
            default_import,
            named_imports,
        }))
    }
}

fn parse_quoted_literal(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.len() < 2 {
        return None;
    }

    let quote = trimmed.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    if !trimmed.ends_with(quote) {
        return None;
    }

    Some(trimmed[1..trimmed.len() - 1].to_string())
}

fn parse_import_spec(input: &str) -> Option<(Option<String>, Vec<EcmaImportNamedImport>)> {
    if input.starts_with('{') && input.ends_with('}') {
        let named_imports = parse_named_imports(&input[1..input.len() - 1]);
        return Some((None, named_imports));
    }

    if let Some((default_part, named_part)) = input.split_once(',') {
        let default_import = default_part.trim();
        if default_import.is_empty() {
            return None;
        }

        let named_part = named_part.trim();
        if !(named_part.starts_with('{') && named_part.ends_with('}')) {
            return None;
        }

        let named_imports = parse_named_imports(&named_part[1..named_part.len() - 1]);
        return Some((Some(default_import.to_string()), named_imports));
    }

    let default_import = input.trim();
    if default_import.is_empty() {
        return None;
    }

    Some((Some(default_import.to_string()), Vec::new()))
}

fn parse_named_imports(input: &str) -> Vec<EcmaImportNamedImport> {
    input
        .split(',')
        .filter_map(|chunk| {
            let item = chunk.trim();
            if item.is_empty() {
                return None;
            }

            if let Some((src, alias)) = item.split_once(" as ") {
                let src = src.trim();
                let alias = alias.trim();
                if src.is_empty() || alias.is_empty() {
                    return None;
                }
                return Some(EcmaImportNamedImport {
                    src: src.to_string(),
                    alias: Some(alias.to_string()),
                });
            }

            Some(EcmaImportNamedImport {
                src: item.to_string(),
                alias: None,
            })
        })
        .collect()
}
