#[test]
fn tokenizer_specific_token_fields_are_public() {
    let admonition = |token: &yozora_tokenizer_admonition::AdmonitionToken| {
        let _ = (token.indent, token.marker, token.marker_count);
    };
    let autolink = |token: &yozora_tokenizer_autolink::AutolinkToken| {
        let _ = token.content_type;
    };
    let autolink_extension =
        |token: &yozora_tokenizer_autolink_extension::AutolinkExtensionToken| {
            let _ = token.content_type;
        };
    let definition = |token: &yozora_tokenizer_definition::DefinitionToken| {
        let _ = (
            &token.lines,
            &token.label,
            &token.destination,
            &token.title,
            token.line_no_of_label,
            token.line_no_of_destination,
            token.line_no_of_title,
            &token._label,
            &token._identifier,
        );
    };
    let ecma_import = |token: &yozora_tokenizer_ecma_import::EcmaImportToken| {
        let _ = (
            &token.module_name,
            &token.default_import,
            &token.named_imports,
        );
    };
    let emphasis = |token: &yozora_tokenizer_emphasis::EmphasisToken| {
        let _ = token.thickness;
    };
    let fenced_code = |token: &yozora_tokenizer_fenced_code::FencedCodeToken| {
        let _ = (&token.lines, &token.info_string);
    };
    let footnote_definition =
        |token: &yozora_tokenizer_footnote_definition::FootnoteDefinitionToken| {
            let _ = (
                &token.label.node_points,
                token.label.start_index,
                token.label.end_index,
                &token._label,
                &token._identifier,
            );
        };
    let footnote_reference =
        |token: &yozora_tokenizer_footnote_reference::FootnoteReferenceToken| {
            let _ = (&token.label, &token.identifier);
        };
    let heading = |token: &yozora_tokenizer_heading::HeadingToken| {
        let _: &yozora_ast::Position = &token.position;
        let _ = (token.depth, &token.line);
    };
    let html_block = |token: &yozora_tokenizer_html_block::HtmlBlockToken| {
        let _ = (token.condition, &token.lines);
    };
    let html_inline = |token: &yozora_tokenizer_html_inline::HtmlInlineToken| match token.data() {
        yozora_tokenizer_html_inline::HtmlInlineTokenData::Open(data) => {
            let _ = (
                data.html_type,
                data.tag_name,
                &data.attributes,
                data.self_closed,
            );
        }
        yozora_tokenizer_html_inline::HtmlInlineTokenData::Closing(data) => {
            let _ = (data.html_type, data.tag_name);
        }
        yozora_tokenizer_html_inline::HtmlInlineTokenData::Comment(data) => {
            let _ = data.html_type;
        }
        yozora_tokenizer_html_inline::HtmlInlineTokenData::Instruction(data) => {
            let _ = data.html_type;
        }
        yozora_tokenizer_html_inline::HtmlInlineTokenData::Declaration(data) => {
            let _ = (data.html_type, data.tag_name);
        }
        yozora_tokenizer_html_inline::HtmlInlineTokenData::Cdata(data) => {
            let _ = data.html_type;
        }
    };
    let image = |token: &yozora_tokenizer_image::ImageToken| {
        let _ = (token.destination_content, token.title_content);
    };
    let image_reference = |token: &yozora_tokenizer_image_reference::ImageReferenceToken| {
        let _ = (&token.identifier, &token.label, token.reference_type);
    };
    let indented_code = |token: &yozora_tokenizer_indented_code::IndentedCodeToken| {
        let _ = &token.lines;
    };
    let inline_code = |token: &yozora_tokenizer_inline_code::InlineCodeToken| {
        let _ = token.thickness;
    };
    let inline_math = |token: &yozora_tokenizer_inline_math::InlineMathToken| {
        let _ = token.thickness;
    };
    let link = |token: &yozora_tokenizer_link::LinkToken| {
        let _ = (token.destination_content, token.title_content);
    };
    let link_reference = |token: &yozora_tokenizer_link_reference::LinkReferenceToken| {
        let _ = (&token.identifier, &token.label, token.reference_type);
    };
    let list = |token: &yozora_tokenizer_list::ListToken| {
        let _ = (
            token._is_empty,
            token.ordered,
            token.marker,
            &token.order_type,
            token.order,
            token.status,
            token.indent,
            token.count_of_top_blank_line,
        );
    };
    let math = |token: &yozora_tokenizer_math::MathToken| {
        let _ = (token.indent, token.marker, token.marker_count);
    };
    let paragraph = |token: &yozora_tokenizer_paragraph::ParagraphToken| {
        let _ = &token.lines;
    };
    let setext_heading = |token: &yozora_tokenizer_setext_heading::SetextHeadingToken| {
        let _ = (token.marker, &token.lines);
    };
    let table = |token: &yozora_tokenizer_table::TableToken| {
        let _ = (&token.columns, &token.rows);
    };
    let table_row = |token: &yozora_tokenizer_table::TableRowToken| {
        let _ = (
            &token.tokenizer,
            token.node_type,
            &token.children,
            &token.position,
            &token.cells,
        );
    };
    let table_cell = |token: &yozora_tokenizer_table::TableCellToken| {
        let _ = (
            &token.tokenizer,
            token.node_type,
            &token.children,
            &token.position,
            &token.lines,
        );
    };
    let thematic_break = |token: &yozora_tokenizer_thematic_break::ThematicBreakToken| {
        let _ = (token.marker, token.continuous);
    };

    let _ = (
        admonition,
        autolink,
        autolink_extension,
        definition,
        ecma_import,
        emphasis,
        fenced_code,
        footnote_definition,
        footnote_reference,
        heading,
        html_block,
        html_inline,
        image,
        image_reference,
        indented_code,
        inline_code,
        inline_math,
        link,
        link_reference,
        list,
        math,
        paragraph,
        setext_heading,
        table,
        table_row,
        table_cell,
        thematic_break,
    );
}

#[test]
fn tokenizer_public_readonly_fields_are_visible() {
    let list = yozora_tokenizer_list::ListTokenizer::default();
    assert!(!list.enable_task_list_item);
    assert!(!list.empty_item_could_not_interrupted_types.is_empty());

    let footnote = yozora_tokenizer_footnote_definition::FootnoteDefinitionTokenizer::default();
    assert_eq!(footnote.indent, 4);
}

#[test]
fn core_tokenizer_named_contracts_are_public() {
    fn assert_block_fallback<T: yozora_core_tokenizer::BlockFallbackTokenizer>() {}

    let _: Option<&yozora_core_tokenizer::MatchBlockHookCreator<'_>> = None;
    let _: Option<&yozora_core_tokenizer::MatchInlineHookCreator<'_>> = None;
    let _: Option<&yozora_core_tokenizer::ParseBlockHookCreator<'_>> = None;
    let _: Option<&yozora_core_tokenizer::ParseInlineHookCreator<'_>> = None;
    let _: yozora_core_tokenizer::BaseBlockTokenizerProps = Default::default();
    let _: yozora_core_tokenizer::BaseInlineTokenizerProps = Default::default();
    let _: yozora_core_tokenizer::ResultOfEatOpener = None;
    let _: yozora_core_tokenizer::ResultOfEatAndInterruptPreviousSibling = None;
    let _: yozora_core_tokenizer::ResultOfOnClose = None;
    let _: yozora_core_tokenizer::ResultOfProcessSingleDelimiter = Vec::new();
    assert_block_fallback::<yozora_tokenizer_paragraph::ParagraphTokenizer>();
}
