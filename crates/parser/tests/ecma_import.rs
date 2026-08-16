use yozora_ast::{EcmaImportNamedImport, Node, Root};
use yozora_core_parser::ParseOptions;
use yozora_core_tokenizer::AnyTokenizer;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;
use yozora_tokenizer_ecma_import::EcmaImportTokenizer;

fn options() -> Option<ParseOptions> {
    Some(ParseOptions {
        should_reserve_position: Some(false),
        ..ParseOptions::default()
    })
}

fn parse_with_ecma_import(source: &str) -> [Root; 3] {
    let mut gfm = GfmParser::default();
    gfm.use_tokenizer(
        AnyTokenizer::Block(Box::new(EcmaImportTokenizer::default())),
        None,
    );

    let mut gfm_ex = GfmExParser::default();
    gfm_ex.use_tokenizer(
        AnyTokenizer::Block(Box::new(EcmaImportTokenizer::default())),
        None,
    );

    [
        gfm.parse(source, options()),
        gfm_ex.parse(source, options()),
        YozoraParser::default().parse(source, options()),
    ]
}

#[test]
fn ecma_import_omits_position_when_disabled() {
    for root in parse_with_ecma_import(
        "import Parser, { createTokenizerTester as create } from '@yozora/test-util'",
    ) {
        let Some(Node::EcmaImport(node)) = root.children.first() else {
            panic!("expected ecmaImport node, root={root:#?}");
        };
        assert_eq!(node.module_name, "@yozora/test-util");
        assert_eq!(node.default_import.as_deref(), Some("Parser"));
        assert_eq!(
            node.named_imports,
            vec![EcmaImportNamedImport {
                src: "createTokenizerTester".to_string(),
                alias: Some("create".to_string()),
            }]
        );
        assert_eq!(node.position, None);
    }
}

#[test]
fn accepts_supported_default_bindings() {
    for (source, default_import) in [
        ("import $foo from 'pkg'", "$foo"),
        ("import 变量 from 'pkg'", "变量"),
        ("import foo1 from 'pkg';", "foo1"),
    ] {
        for root in parse_with_ecma_import(source) {
            assert!(
                matches!(
                    root.children.as_slice(),
                    [Node::EcmaImport(node)]
                        if node.module_name == "pkg"
                            && node.default_import.as_deref() == Some(default_import)
                ),
                "source={source:?}, root={root:#?}"
            );
        }
    }
}

#[test]
fn accepts_supported_named_imports() {
    for root in parse_with_ecma_import(
        "import $default, { $value, default as 变量, for as loop } from 'pkg';",
    ) {
        let Some(Node::EcmaImport(node)) = root.children.first() else {
            panic!("expected ecmaImport node, root={root:#?}");
        };
        assert_eq!(node.module_name, "pkg");
        assert_eq!(node.default_import.as_deref(), Some("$default"));
        assert_eq!(
            node.named_imports,
            vec![
                EcmaImportNamedImport {
                    src: "$value".to_string(),
                    alias: None,
                },
                EcmaImportNamedImport {
                    src: "default".to_string(),
                    alias: Some("变量".to_string()),
                },
                EcmaImportNamedImport {
                    src: "for".to_string(),
                    alias: Some("loop".to_string()),
                },
            ]
        );
    }
}

#[test]
fn handles_absent_default_repeated_sources_and_trailing_comma() {
    for root in parse_with_ecma_import("import { foo } from 'pkg'") {
        assert!(
            matches!(
                root.children.as_slice(),
                [Node::EcmaImport(node)] if node.default_import.is_none()
            ),
            "root={root:#?}"
        );
    }

    for root in parse_with_ecma_import("import { foo, foo as other } from 'pkg'") {
        assert!(
            matches!(
                root.children.as_slice(),
                [Node::EcmaImport(node)]
                    if node.named_imports
                        == [
                            EcmaImportNamedImport {
                                src: "foo".to_string(),
                                alias: None,
                            },
                            EcmaImportNamedImport {
                                src: "foo".to_string(),
                                alias: Some("other".to_string()),
                            },
                        ]
            ),
            "root={root:#?}"
        );
    }

    for source in [
        "import { foo, } from 'pkg'",
        "import Foo, { bar, baz as qux, } from 'pkg';",
    ] {
        for root in parse_with_ecma_import(source) {
            assert!(
                matches!(root.children.as_slice(), [Node::EcmaImport(_)]),
                "source={source:?}, root={root:#?}"
            );
        }
    }
}

#[test]
fn rejects_unsupported_or_invalid_declarations() {
    for source in [
        "import 1foo from 'pkg'",
        "import { 1foo } from 'pkg'",
        "import for from 'pkg'",
        "import eval from 'pkg'",
        "import arguments from 'pkg'",
        "import { default } from 'pkg'",
        "import { value as for } from 'pkg'",
        "import { foo, foo } from 'pkg'",
        "import Foo, { bar as Foo } from 'pkg'",
        "import { foo as value, bar as value } from 'pkg'",
        "import { foo,, } from 'pkg'",
        "import foo from 'pkg';;",
        r"import \u0066oo from 'pkg'",
        "import { 'value-name' as value } from 'pkg'",
    ] {
        for root in parse_with_ecma_import(source) {
            assert!(
                matches!(root.children.first(), Some(Node::Paragraph(_))),
                "source={source:?}, root={root:#?}"
            );
        }
    }
}
