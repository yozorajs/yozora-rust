use std::collections::HashSet;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use yozora_ast::EcmaImportNamedImport;

const IDENTIFIER_START_PATTERN: &str = r"[$_\p{ID_Start}]";
const IDENTIFIER_PART_PATTERN: &str = r"[$_\x{200C}\x{200D}\p{ID_Continue}]";

static IDENTIFIER_NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "^{IDENTIFIER_START_PATTERN}{IDENTIFIER_PART_PATTERN}*$"
    ))
    .expect("identifier name regex should be valid")
});
static INVALID_BINDING_IDENTIFIERS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "arguments",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "eval",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "null",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ])
});
static NAMED_IMPORT_ITEM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let identifier_name_pattern = format!("{IDENTIFIER_START_PATTERN}{IDENTIFIER_PART_PATTERN}*");
    Regex::new(&format!(
        "^({identifier_name_pattern})(?:\\s+as\\s+({identifier_name_pattern}))?$"
    ))
    .expect("named import item regex should be valid")
});
static REGEX1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^import\s+(['"])([^'"]+)(['"])\s*;?\s*$"#)
        .expect("side-effect import regex should be valid")
});
static REGEX2: LazyLock<Regex> = LazyLock::new(|| {
    let identifier_name_pattern = format!("{IDENTIFIER_START_PATTERN}{IDENTIFIER_PART_PATTERN}*");
    Regex::new(&format!(
        r#"^import\s+({identifier_name_pattern})\s+from\s+(['"])([^'"]+)(['"])\s*;?\s*$"#
    ))
    .expect("default import regex should be valid")
});
static REGEX3: LazyLock<Regex> = LazyLock::new(|| {
    let identifier_name_pattern = format!("{IDENTIFIER_START_PATTERN}{IDENTIFIER_PART_PATTERN}*");
    let named_import_item_pattern =
        format!("{identifier_name_pattern}(?:\\s+as\\s+{identifier_name_pattern})?");
    let named_import_pattern = format!(
        "\\{{\\s*((?:{named_import_item_pattern}\\s*,\\s*)*{named_import_item_pattern})\\s*,?\\s*\\}}\\s*"
    );
    Regex::new(&format!(
        r#"^import\s+(?:({identifier_name_pattern})\s*,\s*)?{named_import_pattern}from\s+(['"])([^'"]+)(['"])\s*;?\s*$"#
    ))
    .expect("named import regex should be valid")
});

/// `import '@yozora/parser'`
pub(crate) fn regex1(text: &str) -> Option<Captures<'_>> {
    let captures = REGEX1.captures(text)?;
    (captures.get(1)?.as_str() == captures.get(3)?.as_str()).then_some(captures)
}

/// `import Parser from '@yozora/parser'`
pub(crate) fn regex2(text: &str) -> Option<Captures<'_>> {
    let captures = REGEX2.captures(text)?;
    (captures.get(2)?.as_str() == captures.get(4)?.as_str()).then_some(captures)
}

/// `import Parser, { YozoraParser } from '@yozora/parser'`
pub(crate) fn regex3(text: &str) -> Option<Captures<'_>> {
    let captures = REGEX3.captures(text)?;
    (captures.get(3)?.as_str() == captures.get(5)?.as_str()).then_some(captures)
}

/// Check whether an unescaped IdentifierName can be used as a local binding in a module.
pub(crate) fn is_binding_identifier(identifier: &str) -> bool {
    IDENTIFIER_NAME_REGEX.is_match(identifier) && !INVALID_BINDING_IDENTIFIERS.contains(identifier)
}

pub(crate) fn resolve_name_imports(text: &str) -> Vec<EcmaImportNamedImport> {
    let items = text
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty());
    let mut result = Vec::new();
    for item in items {
        let captures = NAMED_IMPORT_ITEM_REGEX
            .captures(item)
            .expect("named import should have been validated by regex3");
        result.push(EcmaImportNamedImport {
            src: captures[1].to_string(),
            alias: captures.get(2).map(|value| value.as_str().to_string()),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{is_binding_identifier, regex1, regex2, regex3, resolve_name_imports};

    #[test]
    fn matches_side_effect_imports() {
        assert!(regex1("import '@yozora/parser'").is_some());
        assert!(regex1("import \"@yozora/parser\"").is_some());
        assert!(regex1("import '@yozora/parser\"").is_none());
        assert!(regex1("import \"@yozora/parser'").is_none());
        assert!(regex1("import Parser from '@yozora/parser'").is_none());
    }

    #[test]
    fn matches_default_imports() {
        assert!(regex2("import Parser from '@yozora/parser'").is_some());
        assert!(regex2("import Parser from \"@yozora/parser\"").is_some());
        assert!(regex2("import Parser from '@yozora/parser\"").is_none());
        assert!(regex2("import Parser from \"@yozora/parser'").is_none());
        assert!(regex2("import { Parser } from \"@yozora/parser'").is_none());
    }

    #[test]
    fn matches_named_imports() {
        assert!(regex3("import Parser, { a, b as c } from '@yozora/parser'").is_some());
        assert!(regex3("import Parser, { a as c, b as d, e } from \"@yozora/parser\"").is_some());
        assert!(regex3("import Parser, { a, b, c } from \"@yozora/parser\"").is_some());
        assert!(regex3("import { a, b as c } from '@yozora/parser'").is_some());
        assert!(regex3("import { a as c, b as d, e } from \"@yozora/parser\"").is_some());
        assert!(regex3("import { a, b, c } from \"@yozora/parser\"").is_some());
        assert!(regex3("import { a, b, c }, Parser from \"@yozora/parser\"").is_none());
        assert!(regex3("import Parser from '@yozora/parser\"").is_none());
        assert!(regex3("import Parser from \"@yozora/parser'").is_none());
    }

    #[test]
    fn validates_bindings_and_resolves_named_imports() {
        for identifier in ["$foo", "变量", "foo1", "a\u{200c}"] {
            assert!(
                is_binding_identifier(identifier),
                "identifier={identifier:?}"
            );
        }
        for identifier in ["1foo", "for", "eval", "arguments", r"\u0066oo"] {
            assert!(
                !is_binding_identifier(identifier),
                "identifier={identifier:?}"
            );
        }

        assert_eq!(
            resolve_name_imports("$value, default as 变量, for as loop"),
            vec![
                yozora_ast::EcmaImportNamedImport {
                    src: "$value".to_string(),
                    alias: None,
                },
                yozora_ast::EcmaImportNamedImport {
                    src: "default".to_string(),
                    alias: Some("变量".to_string()),
                },
                yozora_ast::EcmaImportNamedImport {
                    src: "for".to_string(),
                    alias: Some("loop".to_string()),
                },
            ]
        );
    }
}
