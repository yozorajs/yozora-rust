use std::collections::HashSet;

use yozora_ast::{Position, ECMA_IMPORT_TYPE};
use yozora_character::{
    calc_string_from_node_points, calc_trim_boundary_of_code_points, AsciiCodePoint,
};
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, EatOpenerResult, PhrasingContentLine,
};

use crate::types::EcmaImportTokenData;
use crate::util::{is_binding_identifier, regex1, regex2, regex3, resolve_name_imports};

pub(crate) fn eat_opener(line: &PhrasingContentLine) -> Option<EatOpenerResult> {
    if line.indent_width >= 4 || line.first_non_whitespace_index + 8 >= line.end_index {
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
    let text = calc_string_from_node_points(node_points, left, right, false);
    let position = || {
        Some(Position {
            start: calc_start_point(node_points, line.start_index),
            end: calc_end_point(node_points, line.end_index - 1),
            indent: None,
        })
    };

    let mut token_data = None;

    if let Some(matched) = regex1(&text) {
        token_data = Some(EcmaImportTokenData {
            module_name: matched.get(2)?.as_str().to_string(),
            default_import: None,
            named_imports: Vec::new(),
        });
    } else if let Some(matched) = regex2(&text) {
        let default_import = matched.get(1)?.as_str();
        if !is_binding_identifier(default_import) {
            return None;
        }

        token_data = Some(EcmaImportTokenData {
            module_name: matched.get(3)?.as_str().to_string(),
            default_import: Some(default_import.to_string()),
            named_imports: Vec::new(),
        });
    } else if let Some(matched) = regex3(&text) {
        let default_import = matched.get(1).map(|value| value.as_str().to_string());
        let named_imports = resolve_name_imports(matched.get(2)?.as_str());

        let mut local_bindings = HashSet::new();
        if let Some(default_import) = default_import.as_ref() {
            if !is_binding_identifier(default_import) {
                return None;
            }
            local_bindings.insert(default_import.as_str());
        }
        for item in &named_imports {
            let local_binding = item.alias.as_ref().unwrap_or(&item.src);
            if !is_binding_identifier(local_binding)
                || local_bindings.contains(local_binding.as_str())
            {
                return None;
            }
            local_bindings.insert(local_binding);
        }

        token_data = Some(EcmaImportTokenData {
            module_name: matched.get(4)?.as_str().to_string(),
            default_import,
            named_imports,
        });
    }

    let token = BlockToken::new("", ECMA_IMPORT_TYPE, position()).with_data(token_data?);
    Some(EatOpenerResult {
        token,
        next_index: line.end_index,
        saturated: true,
    })
}
