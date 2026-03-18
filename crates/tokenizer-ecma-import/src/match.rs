use yozora_ast::EcmaImportNamedImport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EcmaImportToken {
    pub module_name: String,
    pub default_import: Option<String>,
    pub named_imports: Vec<EcmaImportNamedImport>,
}

pub(crate) fn match_ecma_import_token(input: &str) -> Option<EcmaImportToken> {
    if input.contains('\n') {
        return None;
    }

    let statement = input.trim();
    let statement = statement.strip_suffix(';').unwrap_or(statement);
    let rest = statement.strip_prefix("import ")?.trim();

    if let Some(module_name) = parse_quoted_literal(rest) {
        return Some(EcmaImportToken {
            module_name,
            default_import: None,
            named_imports: Vec::new(),
        });
    }

    let (spec, from_part) = rest.split_once(" from ")?;
    let module_name = parse_quoted_literal(from_part.trim())?;

    let (default_import, named_imports) = parse_import_spec(spec.trim())?;
    Some(EcmaImportToken {
        module_name,
        default_import,
        named_imports,
    })
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
