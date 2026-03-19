use yozora_ast::{EcmaImportNamedImport, ECMA_IMPORT_TYPE};
use yozora_character::{
    calc_string_from_node_points, calc_trim_boundary_of_code_points, AsciiCodePoint,
};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, EatOpenerResult, PhrasingContentLine,
};

use crate::parse::EcmaImportTokenData;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EcmaImportToken {
    pub module_name: String,
    pub default_import: Option<String>,
    pub named_imports: Vec<EcmaImportNamedImport>,
}

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.count_of_precede_spaces >= 4 || line.first_non_whitespace_index + 8 >= line.end_index {
        return None;
    }

    let node_points = line.node_points.as_ref();
    let i = line.first_non_whitespace_index;
    if node_points[i].code_point != AsciiCodePoint::LOWERCASE_I as i32
        || node_points[i + 1].code_point != AsciiCodePoint::LOWERCASE_M as i32
        || node_points[i + 2].code_point != AsciiCodePoint::LOWERCASE_P as i32
        || node_points[i + 3].code_point != AsciiCodePoint::LOWERCASE_O as i32
        || node_points[i + 4].code_point != AsciiCodePoint::LOWERCASE_R as i32
        || node_points[i + 5].code_point != AsciiCodePoint::LOWERCASE_T as i32
    {
        return None;
    }

    let (left, right) = calc_trim_boundary_of_code_points(
        node_points,
        line.first_non_whitespace_index,
        line.end_index,
    );
    let source = calc_string_from_node_points(node_points, left, right, false);
    let matched = match_ecma_import_token(&source)?;

    let token = BlockToken::new("", ECMA_IMPORT_TYPE, calc_line_position(line)).with_data(
        EcmaImportTokenData {
            module_name: matched.module_name,
            default_import: matched.default_import,
            named_imports: matched.named_imports,
        },
    );

    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: true,
    })
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

fn calc_line_position(line: &PhrasingContentLine) -> Option<yozora_ast::Position> {
    if line.start_index >= line.end_index {
        return None;
    }

    Some(yozora_ast::Position {
        start: calc_start_point(line.node_points.as_ref(), line.start_index),
        end: calc_end_point(line.node_points.as_ref(), line.end_index - 1),
        indent: None,
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
