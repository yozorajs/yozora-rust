pub mod engine;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use yozora_ast::{Association, Node, Point, Position, ReferenceType, Root, Text};
use yozora_character::{
    calc_escaped_string_from_node_points, create_node_point_generator, fold_case,
};
use yozora_core_tokenizer::{
    leading_indent_columns, AnyFallbackTokenizer, AnyTokenizer, BlockFallbackTokenizer,
    BlockTokenizeResult, BlockTokenizer, InlineFallbackTokenizer, InlineTokenizer,
    MatchBlockPhaseApi, MatchInlinePhaseApi, NodeInterval, ParseBlockPhaseApi, ParseInlinePhaseApi,
    TokenizerKind,
};

pub type FormatUrlFn = Arc<dyn Fn(&str) -> String + Send + Sync + 'static>;

#[derive(Clone)]
pub struct ParseOptions {
    pub should_reserve_position: bool,
    pub preset_definitions: Vec<Association>,
    pub preset_footnote_definitions: Vec<Association>,
    pub format_url: Option<FormatUrlFn>,
}

impl std::fmt::Debug for ParseOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseOptions")
            .field("should_reserve_position", &self.should_reserve_position)
            .field("preset_definitions", &self.preset_definitions)
            .field(
                "preset_footnote_definitions",
                &self.preset_footnote_definitions,
            )
            .field(
                "format_url",
                &self.format_url.as_ref().map(|_| "<function>"),
            )
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum ParseContents<'a> {
    Text(&'a str),
    Lines(&'a [&'a str]),
    OwnedText(String),
    OwnedLines(Vec<String>),
}

impl<'a> From<&'a str> for ParseContents<'a> {
    fn from(value: &'a str) -> Self {
        Self::Text(value)
    }
}

impl<'a> From<&'a [&'a str]> for ParseContents<'a> {
    fn from(value: &'a [&'a str]) -> Self {
        Self::Lines(value)
    }
}

impl<'a> From<&'a String> for ParseContents<'a> {
    fn from(value: &'a String) -> Self {
        Self::Text(value.as_str())
    }
}

impl<'a> From<String> for ParseContents<'a> {
    fn from(value: String) -> Self {
        Self::OwnedText(value)
    }
}

impl<'a> From<Vec<String>> for ParseContents<'a> {
    fn from(value: Vec<String>) -> Self {
        Self::OwnedLines(value)
    }
}

impl<'a> From<&'a [String]> for ParseContents<'a> {
    fn from(value: &'a [String]) -> Self {
        Self::OwnedLines(value.to_vec())
    }
}

impl<'a> From<Vec<&'a str>> for ParseContents<'a> {
    fn from(value: Vec<&'a str>) -> Self {
        Self::OwnedLines(value.into_iter().map(str::to_string).collect())
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            should_reserve_position: false,
            preset_definitions: Vec::new(),
            preset_footnote_definitions: Vec::new(),
            format_url: None,
        }
    }
}

#[derive(Default)]
pub struct DefaultParser {
    block_tokenizers: Vec<Box<dyn BlockTokenizer>>,
    inline_tokenizers: Vec<Box<dyn InlineTokenizer>>,
    block_tokenizer_map: HashMap<String, usize>,
    inline_tokenizer_map: HashMap<String, usize>,
    block_fallback_tokenizer: Option<Box<dyn BlockFallbackTokenizer>>,
    inline_fallback_tokenizer: Option<Box<dyn InlineFallbackTokenizer>>,
    default_parse_options: ParseOptions,
}

impl DefaultParser {
    pub fn new() -> Self {
        Self {
            default_parse_options: ParseOptions::default(),
            ..Self::default()
        }
    }

    pub fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        match tokenizer {
            AnyTokenizer::Block(tokenizer) => {
                self.register_block_tokenizer(tokenizer, register_before_tokenizer)?;
            }
            AnyTokenizer::Inline(tokenizer) => {
                self.register_inline_tokenizer(tokenizer, register_before_tokenizer)?;
            }
        }
        Ok(self)
    }

    pub fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        let name = tokenizer.name().to_string();
        self.unmount_tokenizer(&name);
        self.use_tokenizer(tokenizer, register_before_tokenizer)
    }

    pub fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.unmount_inline_tokenizer(tokenizer_name);
        self.unmount_block_tokenizer(tokenizer_name);

        if self
            .block_fallback_tokenizer
            .as_ref()
            .is_some_and(|t| t.meta().name == tokenizer_name)
        {
            self.block_fallback_tokenizer = None;
        }

        if self
            .inline_fallback_tokenizer
            .as_ref()
            .is_some_and(|t| t.meta().name == tokenizer_name)
        {
            self.inline_fallback_tokenizer = None;
        }

        self
    }

    pub fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        match tokenizer {
            AnyFallbackTokenizer::Block(tokenizer) => {
                self.block_fallback_tokenizer = Some(tokenizer);
            }
            AnyFallbackTokenizer::Inline(tokenizer) => {
                self.inline_fallback_tokenizer = Some(tokenizer);
            }
        }
        self
    }

    pub fn set_default_parse_options(&mut self, options: ParseOptions) {
        self.default_parse_options = options;
    }

    fn parse_contents_impl(
        &self,
        contents: ParseContents<'_>,
        options: Option<ParseOptions>,
    ) -> Root {
        let options = options.unwrap_or_else(|| self.default_parse_options.clone());

        match contents {
            ParseContents::Text(input) => self.parse_text(input, &options),
            ParseContents::Lines(lines) => self.parse_lines(lines, &options),
            ParseContents::OwnedText(input) => self.parse_text(&input, &options),
            ParseContents::OwnedLines(lines) => {
                let line_refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
                self.parse_lines(&line_refs, &options)
            }
        }
    }

    #[allow(non_snake_case)]
    pub fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        self.use_tokenizer(tokenizer, register_before_tokenizer)
    }

    #[allow(non_snake_case)]
    pub fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        self.replace_tokenizer(tokenizer, register_before_tokenizer)
    }

    #[allow(non_snake_case)]
    pub fn unmountTokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.unmount_tokenizer(tokenizer_name)
    }

    #[allow(non_snake_case)]
    pub fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        self.use_fallback_tokenizer(tokenizer)
    }

    #[allow(non_snake_case)]
    pub fn setDefaultParseOptions(&mut self, options: ParseOptions) {
        self.set_default_parse_options(options)
    }

    pub fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        self.parse_contents_impl(contents.into(), options)
    }

    fn parse_text(&self, input: &str, options: &ParseOptions) -> Root {
        let root_position = if options.should_reserve_position {
            calc_position(input)
        } else {
            None
        };

        let lines: Vec<&str> = input.lines().collect();
        let mut block_api = ParserMatchBlockPhaseApi::default();
        let mut children = self.parse_blocks_from_lines(
            &lines,
            options.should_reserve_position,
            &mut block_api,
            None,
        );

        if input.is_empty() {
            self.flush_paragraph_lines(
                &mut children,
                &mut vec![""],
                options.should_reserve_position,
                None,
                false,
            );
        }

        let inline_api = ParserMatchInlineContextApi::new(
            &block_api.definition_identifiers,
            &block_api.footnote_definition_identifiers,
            &options.preset_definitions,
            &options.preset_footnote_definitions,
        );
        self.process_inline_children_with_options(
            &mut children,
            options.should_reserve_position,
            0,
            false,
            &inline_api,
        );

        apply_format_url(&mut children, options.format_url.as_ref());
        resolve_references(
            &mut children,
            &options.preset_definitions,
            &options.preset_footnote_definitions,
        );
        merge_adjacent_text_nodes(&mut children);

        Root {
            node_type: "root".to_string(),
            position: root_position,
            children,
        }
    }

    fn parse_lines(&self, lines: &[&str], options: &ParseOptions) -> Root {
        let root_position = if options.should_reserve_position {
            calc_position(&lines.join("\n"))
        } else {
            None
        };

        let mut block_api = ParserMatchBlockPhaseApi::default();
        let mut children = self.parse_blocks_from_lines(
            lines,
            options.should_reserve_position,
            &mut block_api,
            None,
        );
        if lines.is_empty() {
            self.flush_paragraph_lines(
                &mut children,
                &mut vec![""],
                options.should_reserve_position,
                None,
                false,
            );
        }

        let inline_api = ParserMatchInlineContextApi::new(
            &block_api.definition_identifiers,
            &block_api.footnote_definition_identifiers,
            &options.preset_definitions,
            &options.preset_footnote_definitions,
        );
        self.process_inline_children_with_options(
            &mut children,
            options.should_reserve_position,
            0,
            false,
            &inline_api,
        );

        apply_format_url(&mut children, options.format_url.as_ref());
        resolve_references(
            &mut children,
            &options.preset_definitions,
            &options.preset_footnote_definitions,
        );
        merge_adjacent_text_nodes(&mut children);

        Root {
            node_type: "root".to_string(),
            position: root_position,
            children,
        }
    }

    fn parse_blocks_from_lines(
        &self,
        lines: &[&str],
        should_reserve_position: bool,
        block_api: &mut dyn MatchBlockPhaseApi,
        base_start_point: Option<Point>,
    ) -> Vec<Node> {
        let mut blocks = Vec::new();
        let mut line_index = 0usize;
        let mut paragraph_lines: Vec<&str> = Vec::new();
        let mut paragraph_start_line_index: Option<usize> = None;
        let line_starts = if should_reserve_position {
            let start = base_start_point.unwrap_or(Point {
                line: 1,
                column: 1,
                offset: Some(0),
            });
            Some(calc_line_start_points(lines, start))
        } else {
            None
        };

        while line_index < lines.len() {
            let line = lines[line_index];

            if line.trim().is_empty() {
                let paragraph_start_point = paragraph_start_line_index.and_then(|idx| {
                    line_starts
                        .as_ref()
                        .and_then(|points| points.get(idx).copied())
                });
                self.flush_paragraph_lines(
                    &mut blocks,
                    &mut paragraph_lines,
                    should_reserve_position,
                    paragraph_start_point,
                    true,
                );
                paragraph_start_line_index = None;
                line_index += 1;
                continue;
            }

            if !paragraph_lines.is_empty() && starts_indented_code(line) {
                paragraph_lines.push(line);
                line_index += 1;
                continue;
            }

            let remaining = &lines[line_index..];
            let allow_interrupt_paragraph = paragraph_lines.is_empty();
            if let Some(mut result) = self.try_parse_block_lines(
                remaining,
                should_reserve_position,
                allow_interrupt_paragraph,
                block_api,
                line_starts
                    .as_ref()
                    .and_then(|points| points.get(line_index).copied()),
            ) {
                let paragraph_start_point = paragraph_start_line_index.and_then(|idx| {
                    line_starts
                        .as_ref()
                        .and_then(|points| points.get(idx).copied())
                });
                self.flush_paragraph_lines(
                    &mut blocks,
                    &mut paragraph_lines,
                    should_reserve_position,
                    paragraph_start_point,
                    true,
                );
                paragraph_start_line_index = None;
                self.process_block_in_node(&mut result.node, should_reserve_position, block_api);
                push_block_node(&mut blocks, result.node);
                line_index += result.consumed_lines.max(1);
                continue;
            }

            if paragraph_lines.is_empty() {
                paragraph_start_line_index = Some(line_index);
            }
            paragraph_lines.push(line);
            line_index += 1;
        }

        let paragraph_start_point = paragraph_start_line_index.and_then(|idx| {
            line_starts
                .as_ref()
                .and_then(|points| points.get(idx).copied())
        });
        self.flush_paragraph_lines(
            &mut blocks,
            &mut paragraph_lines,
            should_reserve_position,
            paragraph_start_point,
            false,
        );
        blocks
    }

    fn flush_paragraph_lines(
        &self,
        blocks: &mut Vec<Node>,
        paragraph_lines: &mut Vec<&str>,
        should_reserve_position: bool,
        paragraph_start_point: Option<Point>,
        should_extend_terminal_column: bool,
    ) {
        if paragraph_lines.is_empty() {
            return;
        }

        let normalized_lines: Vec<String> = paragraph_lines
            .iter()
            .map(|line| line.trim_start_matches(' ').to_string())
            .collect();
        let mut chunk = normalized_lines.join("\n");
        while chunk.ends_with(' ') || chunk.ends_with('\t') {
            chunk.pop();
        }
        paragraph_lines.clear();

        let Some(block_fallback) = self.block_fallback_tokenizer.as_ref() else {
            return;
        };

        let mut position = if should_reserve_position {
            if let Some(start) = paragraph_start_point {
                calc_position_with_start(&chunk, start)
            } else {
                calc_position(&chunk)
            }
        } else {
            None
        };

        if should_extend_terminal_column {
            if let Some(pos) = position.as_mut() {
                pos.end.column += 1;
                if let Some(offset) = pos.end.offset.as_mut() {
                    *offset += 1;
                }
            }
        }

        let inline_nodes = vec![Node::Text(Text {
            position: position.clone(),
            value: chunk,
        })];
        let block_api = ParserParseBlockPhaseApi {
            should_reserve_position,
        };
        let block_node = block_fallback.build_block_with_api(inline_nodes, position, &block_api);
        push_block_node(blocks, block_node);
    }

    fn process_block_in_node(
        &self,
        node: &mut Node,
        should_reserve_position: bool,
        block_api: &mut dyn MatchBlockPhaseApi,
    ) {
        self.process_block_in_node_with_depth(node, should_reserve_position, block_api, 0);
    }

    fn process_block_in_node_with_depth(
        &self,
        node: &mut Node,
        should_reserve_position: bool,
        block_api: &mut dyn MatchBlockPhaseApi,
        depth: usize,
    ) {
        if depth > 64 {
            return;
        }

        match node {
            Node::Admonition(n) => self.process_block_children(
                &mut n.children,
                should_reserve_position,
                block_api,
                depth + 1,
            ),
            Node::Blockquote(n) => self.process_block_children(
                &mut n.children,
                should_reserve_position,
                block_api,
                depth + 1,
            ),
            Node::FootnoteDefinition(n) => self.process_block_children(
                &mut n.children,
                should_reserve_position,
                block_api,
                depth + 1,
            ),
            Node::List(n) => {
                for child in n.children.iter_mut() {
                    self.process_block_in_node_with_depth(
                        child,
                        should_reserve_position,
                        block_api,
                        depth + 1,
                    );
                }
            }
            Node::ListItem(n) => self.process_block_children(
                &mut n.children,
                should_reserve_position,
                block_api,
                depth + 1,
            ),
            _ => {}
        }
    }

    fn process_block_children(
        &self,
        children: &mut Vec<Node>,
        should_reserve_position: bool,
        block_api: &mut dyn MatchBlockPhaseApi,
        depth: usize,
    ) {
        if children.is_empty() {
            return;
        }

        if children.iter().all(|child| matches!(child, Node::Text(_))) {
            let nested_base_point = children.iter().find_map(|child| {
                let Node::Text(text) = child else {
                    return None;
                };
                text.position.as_ref().map(|position| position.start)
            });
            let nested_input = children
                .iter()
                .filter_map(|child| {
                    let Node::Text(text) = child else {
                        return None;
                    };
                    Some(text.value.clone())
                })
                .collect::<Vec<_>>()
                .join("\n");

            let nested_lines: Vec<&str> = nested_input.lines().collect();
            let nested = self.parse_blocks_from_lines(
                &nested_lines,
                should_reserve_position,
                block_api,
                nested_base_point,
            );
            if !nested.is_empty() {
                *children = nested;
            }
        }

        for child in children.iter_mut() {
            self.process_block_in_node_with_depth(child, should_reserve_position, block_api, depth);
        }
    }

    fn try_parse_block_lines(
        &self,
        lines: &[&str],
        should_reserve_position: bool,
        allow_interrupt_paragraph: bool,
        block_api: &mut dyn MatchBlockPhaseApi,
        line_start_point: Option<Point>,
    ) -> Option<BlockTokenizeResult> {
        let position = if should_reserve_position {
            if let Some(start) = line_start_point {
                calc_position_with_start(lines.first().copied()?, start)
            } else {
                calc_position(lines.first().copied()?)
            }
        } else {
            None
        };

        for tokenizer in &self.block_tokenizers {
            if !allow_interrupt_paragraph && !tokenizer.can_interrupt_paragraph_with_lines(lines) {
                continue;
            }
            if let Some(result) =
                tokenizer.tokenize_block_lines_with_api(lines, position.clone(), block_api)
            {
                return Some(result);
            }
        }
        None
    }

    fn parse_inline_children_nodes(
        &self,
        mut nodes: Vec<Node>,
        skip_link_like: bool,
        inline_api: &dyn MatchInlinePhaseApi,
        should_reserve_position: bool,
    ) -> Vec<Node> {
        if self.inline_tokenizers.is_empty() {
            let mut out = Vec::with_capacity(nodes.len());
            for node in nodes {
                match node {
                    Node::Text(text) => {
                        let normalized = normalize_plain_text(&text.value);
                        if let Some(inline_fallback) = self.inline_fallback_tokenizer.as_ref() {
                            out.push(inline_fallback.find_and_handle_delimiter(
                                &normalized,
                                0,
                                normalized.len(),
                                text.position,
                                inline_api,
                            ));
                        } else {
                            out.push(Node::Text(Text {
                                position: text.position,
                                value: normalized,
                            }));
                        }
                    }
                    other => out.push(other),
                }
            }
            return out;
        }

        for tokenizer in &self.inline_tokenizers {
            if skip_link_like && is_link_like_inline_tokenizer(tokenizer.meta().name.as_str()) {
                continue;
            }
            nodes = apply_inline_tokenizer(
                tokenizer.as_ref(),
                nodes,
                inline_api,
                should_reserve_position,
            );
        }

        normalize_text_nodes(&mut nodes);

        nodes
    }

    fn process_inline_in_node_with_depth(
        &self,
        node: &mut Node,
        should_reserve_position: bool,
        depth: usize,
        inline_api: &dyn MatchInlinePhaseApi,
    ) {
        if depth > 64 {
            return;
        }

        match node {
            Node::Admonition(n) => {
                self.process_inline_children(
                    &mut n.title,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
                self.process_container_block_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Blockquote(n) => {
                self.process_container_block_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Delete(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Emphasis(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Footnote(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::FootnoteDefinition(n) => {
                self.process_container_block_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Heading(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Link(n) => {
                self.process_inline_children_with_options(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    true,
                    inline_api,
                );
            }
            Node::LinkReference(n) => {
                self.process_inline_children_with_options(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    true,
                    inline_api,
                );
            }
            Node::List(n) => {
                for child in n.children.iter_mut() {
                    self.process_inline_in_node_with_depth(
                        child,
                        should_reserve_position,
                        depth + 1,
                        inline_api,
                    );
                }

                if !n.spread {
                    tighten_list_items(&mut n.children);
                }
            }
            Node::ListItem(n) => {
                self.process_container_block_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Paragraph(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Strong(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::Table(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::TableRow(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            Node::TableCell(n) => {
                self.process_inline_children(
                    &mut n.children,
                    should_reserve_position,
                    depth + 1,
                    inline_api,
                );
            }
            _ => {}
        }
    }

    fn process_inline_children(
        &self,
        children: &mut Vec<Node>,
        should_reserve_position: bool,
        depth: usize,
        inline_api: &dyn MatchInlinePhaseApi,
    ) {
        self.process_inline_children_with_options(
            children,
            should_reserve_position,
            depth,
            false,
            inline_api,
        )
    }

    fn process_inline_children_with_options(
        &self,
        children: &mut Vec<Node>,
        should_reserve_position: bool,
        depth: usize,
        skip_link_like: bool,
        inline_api: &dyn MatchInlinePhaseApi,
    ) {
        let seed = std::mem::take(children);
        let mut parsed = self.parse_inline_children_nodes(
            seed,
            skip_link_like,
            inline_api,
            should_reserve_position,
        );

        for node in &mut parsed {
            self.process_inline_in_node_with_depth(
                node,
                should_reserve_position,
                depth,
                inline_api,
            );
        }

        *children = parsed;
    }

    fn process_container_block_children(
        &self,
        children: &mut Vec<Node>,
        should_reserve_position: bool,
        depth: usize,
        inline_api: &dyn MatchInlinePhaseApi,
    ) {
        if children.is_empty() {
            return;
        }

        for child in children.iter_mut() {
            self.process_inline_in_node_with_depth(
                child,
                should_reserve_position,
                depth,
                inline_api,
            );
        }
    }

    fn register_block_tokenizer(
        &mut self,
        tokenizer: Box<dyn BlockTokenizer>,
        register_before_tokenizer: Option<&str>,
    ) -> Result<(), String> {
        let name = tokenizer.meta().name.clone();
        if self.block_tokenizer_map.contains_key(&name) {
            return Err(format!("[use_tokenizer] Name({name}) has been registered."));
        }

        let insert_index = calc_insert_index(
            &self.block_tokenizers,
            tokenizer.meta().priority,
            register_before_tokenizer,
        );
        self.block_tokenizers.insert(insert_index, tokenizer);
        self.rebuild_block_index();
        Ok(())
    }

    fn register_inline_tokenizer(
        &mut self,
        tokenizer: Box<dyn InlineTokenizer>,
        register_before_tokenizer: Option<&str>,
    ) -> Result<(), String> {
        let name = tokenizer.meta().name.clone();
        if self.inline_tokenizer_map.contains_key(&name) {
            return Err(format!("[use_tokenizer] Name({name}) has been registered."));
        }

        let insert_index = calc_insert_index(
            &self.inline_tokenizers,
            tokenizer.meta().priority,
            register_before_tokenizer,
        );
        self.inline_tokenizers.insert(insert_index, tokenizer);
        self.rebuild_inline_index();
        Ok(())
    }

    fn unmount_block_tokenizer(&mut self, name: &str) {
        if let Some(index) = self.block_tokenizer_map.remove(name) {
            self.block_tokenizers.remove(index);
            self.rebuild_block_index();
        }
    }

    fn unmount_inline_tokenizer(&mut self, name: &str) {
        if let Some(index) = self.inline_tokenizer_map.remove(name) {
            self.inline_tokenizers.remove(index);
            self.rebuild_inline_index();
        }
    }

    fn rebuild_block_index(&mut self) {
        self.block_tokenizer_map.clear();
        for (idx, tokenizer) in self.block_tokenizers.iter().enumerate() {
            self.block_tokenizer_map
                .insert(tokenizer.meta().name.clone(), idx);
        }
    }

    fn rebuild_inline_index(&mut self) {
        self.inline_tokenizer_map.clear();
        for (idx, tokenizer) in self.inline_tokenizers.iter().enumerate() {
            self.inline_tokenizer_map
                .insert(tokenizer.meta().name.clone(), idx);
        }
    }
}

fn is_link_like_inline_tokenizer(name: &str) -> bool {
    matches!(
        name,
        "@yozora/tokenizer-link"
            | "@yozora/tokenizer-link-reference"
            | "@yozora/tokenizer-autolink"
            | "@yozora/tokenizer-autolink-extension"
    )
}

fn calc_insert_index<T: ?Sized + yozora_core_tokenizer::Tokenizer>(
    tokenizers: &[Box<T>],
    priority: i32,
    register_before_tokenizer: Option<&str>,
) -> usize {
    for (index, existing) in tokenizers.iter().enumerate() {
        if register_before_tokenizer.is_some_and(|target| target == existing.meta().name.as_str()) {
            return index;
        }
        if priority > existing.meta().priority {
            return index;
        }
    }
    tokenizers.len()
}

fn calc_position(input: &str) -> Option<Position> {
    let chunks = create_node_point_generator(input);
    let points = chunks.first()?;

    if points.is_empty() {
        return Some(Position {
            start: Point {
                line: 1,
                column: 1,
                offset: Some(0),
            },
            end: Point {
                line: 1,
                column: 1,
                offset: Some(0),
            },
            indent: None,
        });
    }

    let first = points.first()?;
    let last = points.last()?;

    let end_column = if last.code_point == yozora_character::VirtualCodePoint::LineEnd as i32 {
        1
    } else {
        last.column + 1
    };

    Some(Position {
        start: Point {
            line: first.line,
            column: first.column,
            offset: Some(first.offset),
        },
        end: Point {
            line: last.line,
            column: end_column,
            offset: Some(last.offset + 1),
        },
        indent: None,
    })
}

fn calc_position_with_start(input: &str, start: Point) -> Option<Position> {
    let end = advance_point_by_slice(start, input);
    Some(Position {
        start,
        end,
        indent: None,
    })
}

fn calc_line_start_points(lines: &[&str], start: Point) -> Vec<Point> {
    let mut points = Vec::with_capacity(lines.len());
    let mut current = start;

    for (idx, line) in lines.iter().enumerate() {
        points.push(current);
        if idx + 1 >= lines.len() {
            continue;
        }

        let mut next = advance_point_by_slice(current, line);
        if let Some(offset) = next.offset.as_mut() {
            *offset += 1;
        }
        next.line += 1;
        next.column = 1;
        current = next;
    }

    points
}

fn normalize_text_nodes(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        if let Node::Text(text) = node {
            text.value = normalize_plain_text(&text.value);
        }
    }
}

fn normalize_plain_text(input: &str) -> String {
    let mut output = if input.contains('\\') || input.contains('&') {
        let chunks = create_node_point_generator(input);
        if let Some(points) = chunks.first() {
            calc_escaped_string_from_node_points(points, 0, points.len(), false)
        } else {
            input.to_string()
        }
    } else {
        input.to_string()
    };

    if !output.contains('\n') {
        return output;
    }

    let mut normalized = String::with_capacity(output.len());
    let mut chars = output.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\n' {
            normalized.push(ch);
            continue;
        }

        while normalized.ends_with(' ') || normalized.ends_with('\t') {
            normalized.pop();
        }

        normalized.push('\n');
        while chars.peek().is_some_and(|c| *c == ' ' || *c == '\t') {
            chars.next();
        }
    }

    output = normalized;
    output
}

fn push_block_node(blocks: &mut Vec<Node>, node: Node) {
    let Some(last) = blocks.last_mut() else {
        blocks.push(node);
        return;
    };

    let Node::List(last_list) = last else {
        blocks.push(node);
        return;
    };
    let Node::List(mut next_list) = node else {
        blocks.push(node);
        return;
    };

    let can_merge = last_list.ordered == next_list.ordered
        && last_list.order_type == next_list.order_type
        && last_list.marker == next_list.marker;

    if can_merge {
        last_list.children.append(&mut next_list.children);
        return;
    }

    blocks.push(Node::List(next_list));
}

fn tighten_list_items(children: &mut Vec<Node>) {
    for child in children.iter_mut() {
        let Node::ListItem(list_item) = child else {
            continue;
        };

        let mut next_children = Vec::new();
        for item_child in std::mem::take(&mut list_item.children) {
            match item_child {
                Node::Paragraph(paragraph) => {
                    next_children.extend(paragraph.children);
                }
                other => next_children.push(other),
            }
        }
        list_item.children = next_children;
    }
}

fn apply_inline_tokenizer(
    tokenizer: &dyn InlineTokenizer,
    nodes: Vec<Node>,
    api: &dyn MatchInlinePhaseApi,
    should_reserve_position: bool,
) -> Vec<Node> {
    if tokenizer.meta().name == "@yozora/tokenizer-link"
        || tokenizer.meta().name == "@yozora/tokenizer-link-reference"
    {
        return apply_inline_tokenizer_with_placeholders(
            tokenizer,
            nodes,
            api,
            should_reserve_position,
        );
    }

    if tokenizer.meta().name == "@yozora/tokenizer-emphasis" {
        return apply_emphasis_tokenizer(tokenizer, nodes, api, should_reserve_position);
    }

    let mut out = Vec::new();

    for node in nodes {
        match node {
            Node::Text(text) => {
                let parse_api = ParserParseInlineSegmentApi {
                    source: &text.value,
                    base_position: text.position.clone(),
                    should_reserve_position,
                };
                if let Some(mut parsed) = tokenizer.tokenize_inline_with_apis(
                    &text.value,
                    text.position.clone(),
                    api,
                    &parse_api,
                ) {
                    if parsed.is_empty() {
                        out.push(Node::Text(text));
                    } else {
                        out.append(&mut parsed);
                    }
                } else {
                    out.push(Node::Text(text));
                }
            }
            _ => out.push(node),
        }
    }

    out
}

fn apply_tokenizer_text_only(
    tokenizer: &dyn InlineTokenizer,
    nodes: Vec<Node>,
    api: &dyn MatchInlinePhaseApi,
    should_reserve_position: bool,
) -> Vec<Node> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            Node::Text(text) => {
                let parse_api = ParserParseInlineSegmentApi {
                    source: &text.value,
                    base_position: text.position.clone(),
                    should_reserve_position,
                };
                if let Some(mut parsed) = tokenizer.tokenize_inline_with_apis(
                    &text.value,
                    text.position.clone(),
                    api,
                    &parse_api,
                ) {
                    if parsed.is_empty() {
                        out.push(Node::Text(text));
                    } else {
                        out.append(&mut parsed);
                    }
                } else {
                    out.push(Node::Text(text));
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn apply_inline_tokenizer_with_placeholders(
    tokenizer: &dyn InlineTokenizer,
    nodes: Vec<Node>,
    api: &dyn MatchInlinePhaseApi,
    should_reserve_position: bool,
) -> Vec<Node> {
    if nodes.iter().all(|node| matches!(node, Node::Text(_))) {
        return apply_tokenizer_text_only(tokenizer, nodes, api, should_reserve_position);
    }

    const PLACEHOLDER: char = '\u{001F}';
    let is_link_reference_tokenizer = tokenizer.meta().name == "@yozora/tokenizer-link-reference";

    let fallback = nodes.clone();
    let mut stitched = String::new();
    let mut placeholders = Vec::new();

    for node in nodes {
        match node {
            Node::Text(text) => stitched.push_str(&text.value),
            other => {
                stitched.push(PLACEHOLDER);
                placeholders.push(other);
            }
        }
    }

    if (tokenizer.meta().name == "@yozora/tokenizer-link"
        || tokenizer.meta().name == "@yozora/tokenizer-link-reference")
        && placeholders.iter().any(|node| {
            matches!(
                node,
                Node::Link(_) | Node::LinkReference(_) | Node::Image(_) | Node::ImageReference(_)
            )
        })
    {
        return apply_tokenizer_text_only(tokenizer, fallback, api, should_reserve_position);
    }

    let parse_api = ParserParseInlineSegmentApi {
        source: &stitched,
        base_position: None,
        should_reserve_position,
    };
    let Some(parsed) = tokenizer.tokenize_inline_with_apis(&stitched, None, api, &parse_api) else {
        return fallback;
    };

    let mut placeholder_index = 0usize;
    let Some(expanded) =
        expand_inline_placeholders(parsed, PLACEHOLDER, &placeholders, &mut placeholder_index)
    else {
        return fallback;
    };

    if placeholder_index != placeholders.len() {
        if is_link_reference_tokenizer
            && placeholders[placeholder_index..]
                .iter()
                .all(|node| matches!(node, Node::Break(_)))
        {
            return expanded;
        }
        return fallback;
    }

    expanded
}

fn apply_emphasis_tokenizer(
    tokenizer: &dyn InlineTokenizer,
    nodes: Vec<Node>,
    api: &dyn MatchInlinePhaseApi,
    should_reserve_position: bool,
) -> Vec<Node> {
    if nodes.iter().all(|node| matches!(node, Node::Text(_))) {
        let mut out = Vec::new();
        for node in nodes {
            let Node::Text(text) = node else {
                out.push(node);
                continue;
            };

            let parse_api = ParserParseInlineSegmentApi {
                source: &text.value,
                base_position: text.position.clone(),
                should_reserve_position,
            };
            if let Some(mut parsed) = tokenizer.tokenize_inline_with_apis(
                &text.value,
                text.position.clone(),
                api,
                &parse_api,
            ) {
                if parsed.is_empty() {
                    out.push(Node::Text(text));
                } else {
                    out.append(&mut parsed);
                }
            } else {
                out.push(Node::Text(text));
            }
        }

        return out;
    }

    const PLACEHOLDER: char = '\u{001F}';

    let fallback = nodes.clone();
    let mut stitched = String::new();
    let mut placeholders = Vec::new();

    for node in nodes {
        match node {
            Node::Text(text) => stitched.push_str(&text.value),
            other => {
                stitched.push(PLACEHOLDER);
                placeholders.push(other);
            }
        }
    }

    let parse_api = ParserParseInlineSegmentApi {
        source: &stitched,
        base_position: None,
        should_reserve_position,
    };
    let Some(parsed) = tokenizer.tokenize_inline_with_apis(&stitched, None, api, &parse_api) else {
        return fallback;
    };

    let mut placeholder_index = 0usize;
    let Some(expanded) =
        expand_inline_placeholders(parsed, PLACEHOLDER, &placeholders, &mut placeholder_index)
    else {
        return fallback;
    };

    if placeholder_index != placeholders.len() {
        return fallback;
    }

    expanded
}

fn expand_inline_placeholders(
    nodes: Vec<Node>,
    placeholder: char,
    placeholders: &[Node],
    placeholder_index: &mut usize,
) -> Option<Vec<Node>> {
    let mut out = Vec::new();

    for node in nodes {
        match node {
            Node::Text(text) => {
                let mut last = 0usize;

                for (idx, ch) in text.value.char_indices() {
                    if ch != placeholder {
                        continue;
                    }

                    if idx > last {
                        out.push(Node::Text(Text {
                            position: None,
                            value: text.value[last..idx].to_string(),
                        }));
                    }

                    let replacement = placeholders.get(*placeholder_index)?.clone();
                    *placeholder_index += 1;
                    out.push(replacement);
                    last = idx + ch.len_utf8();
                }

                if last < text.value.len() {
                    out.push(Node::Text(Text {
                        position: None,
                        value: text.value[last..].to_string(),
                    }));
                }
            }
            Node::Emphasis(mut n) => {
                n.children = expand_inline_placeholders(
                    n.children,
                    placeholder,
                    placeholders,
                    placeholder_index,
                )?;
                out.push(Node::Emphasis(n));
            }
            Node::Strong(mut n) => {
                n.children = expand_inline_placeholders(
                    n.children,
                    placeholder,
                    placeholders,
                    placeholder_index,
                )?;
                out.push(Node::Strong(n));
            }
            Node::Delete(mut n) => {
                n.children = expand_inline_placeholders(
                    n.children,
                    placeholder,
                    placeholders,
                    placeholder_index,
                )?;
                out.push(Node::Delete(n));
            }
            Node::Footnote(mut n) => {
                n.children = expand_inline_placeholders(
                    n.children,
                    placeholder,
                    placeholders,
                    placeholder_index,
                )?;
                out.push(Node::Footnote(n));
            }
            Node::Link(mut n) => {
                n.children = expand_inline_placeholders(
                    n.children,
                    placeholder,
                    placeholders,
                    placeholder_index,
                )?;
                out.push(Node::Link(n));
            }
            Node::LinkReference(mut n) => {
                n.children = expand_inline_placeholders(
                    n.children,
                    placeholder,
                    placeholders,
                    placeholder_index,
                )?;
                out.push(Node::LinkReference(n));
            }
            other => out.push(other),
        }
    }

    Some(out)
}

fn starts_indented_code(line: &str) -> bool {
    leading_indent_columns(line) >= 4
}

struct ParserParseInlineSegmentApi<'a> {
    source: &'a str,
    base_position: Option<Position>,
    should_reserve_position: bool,
}

impl ParseInlinePhaseApi for ParserParseInlineSegmentApi<'_> {
    fn should_reserve_position(&self) -> bool {
        self.should_reserve_position
    }

    fn calc_position(&self, interval: NodeInterval) -> Option<Position> {
        if !self.should_reserve_position {
            return None;
        }

        let base = self.base_position.as_ref()?;
        calc_position_within_source(self.source, base, interval)
    }

    fn format_url(&self, url: &str) -> String {
        url.to_string()
    }
}

fn calc_position_within_source(
    source: &str,
    base: &Position,
    interval: NodeInterval,
) -> Option<Position> {
    if interval.start_index > interval.end_index || interval.end_index > source.len() {
        return None;
    }
    if !source.is_char_boundary(interval.start_index)
        || !source.is_char_boundary(interval.end_index)
    {
        return None;
    }

    let start = advance_point_by_slice(base.start, &source[..interval.start_index]);
    let end = advance_point_by_slice(start, &source[interval.start_index..interval.end_index]);

    Some(Position {
        start,
        end,
        indent: None,
    })
}

fn advance_point_by_slice(mut point: Point, slice: &str) -> Point {
    for ch in slice.chars() {
        if let Some(offset) = point.offset.as_mut() {
            *offset += ch.len_utf8();
        }

        if ch == '\n' {
            point.line += 1;
            point.column = 1;
        } else {
            point.column += 1;
        }
    }
    point
}

#[derive(Default)]
struct ParserMatchBlockPhaseApi {
    definition_identifiers: HashSet<String>,
    footnote_definition_identifiers: HashSet<String>,
}

impl MatchBlockPhaseApi for ParserMatchBlockPhaseApi {
    fn register_definition_identifier(&mut self, identifier: &str) {
        self.definition_identifiers
            .insert(normalize_identifier(identifier));
    }

    fn register_footnote_definition_identifier(&mut self, identifier: &str) {
        self.footnote_definition_identifiers
            .insert(normalize_identifier(identifier));
    }
}

struct ParserMatchInlineContextApi {
    definition_identifiers: HashSet<String>,
    footnote_definition_identifiers: HashSet<String>,
}

impl ParserMatchInlineContextApi {
    fn new(
        block_definition_identifiers: &HashSet<String>,
        block_footnote_definition_identifiers: &HashSet<String>,
        preset_definitions: &[Association],
        preset_footnote_definitions: &[Association],
    ) -> Self {
        let mut definition_identifiers = block_definition_identifiers.clone();
        for definition in preset_definitions {
            definition_identifiers.insert(normalize_identifier(&definition.identifier));
            definition_identifiers.insert(normalize_identifier(&definition.label));
        }

        let mut footnote_definition_identifiers = block_footnote_definition_identifiers.clone();
        for definition in preset_footnote_definitions {
            footnote_definition_identifiers.insert(normalize_identifier(&definition.identifier));
            footnote_definition_identifiers.insert(normalize_identifier(&definition.label));
        }

        Self {
            definition_identifiers,
            footnote_definition_identifiers,
        }
    }
}

impl MatchInlinePhaseApi for ParserMatchInlineContextApi {
    fn has_definition(&self, identifier: &str) -> bool {
        self.definition_identifiers
            .contains(&normalize_identifier(identifier))
    }

    fn has_footnote_definition(&self, identifier: &str) -> bool {
        self.footnote_definition_identifiers
            .contains(&normalize_identifier(identifier))
    }
}

struct ParserParseBlockPhaseApi {
    should_reserve_position: bool,
}

impl ParseBlockPhaseApi for ParserParseBlockPhaseApi {
    fn should_reserve_position(&self) -> bool {
        self.should_reserve_position
    }

    fn format_url(&self, url: &str) -> String {
        url.to_string()
    }
}

fn resolve_references(
    nodes: &mut Vec<Node>,
    preset_definitions: &[Association],
    preset_footnote_definitions: &[Association],
) {
    let mut definition_ids = HashSet::new();
    let mut footnote_definition_ids = HashSet::new();

    for definition in preset_definitions {
        definition_ids.insert(normalize_identifier(&definition.identifier));
        definition_ids.insert(normalize_identifier(&definition.label));
    }

    for definition in preset_footnote_definitions {
        footnote_definition_ids.insert(normalize_identifier(&definition.identifier));
        footnote_definition_ids.insert(normalize_identifier(&definition.label));
    }

    collect_definition_identifiers(nodes, &mut definition_ids, &mut footnote_definition_ids);
    rewrite_unresolved_references(nodes, &definition_ids, &footnote_definition_ids);
    rebalance_chained_link_references(nodes);
}

fn apply_format_url(nodes: &mut [Node], format_url: Option<&FormatUrlFn>) {
    let Some(format_url) = format_url else {
        return;
    };

    for node in nodes {
        match node {
            Node::Admonition(n) => {
                apply_format_url(&mut n.title, Some(format_url));
                apply_format_url(&mut n.children, Some(format_url));
            }
            Node::Blockquote(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Definition(n) => n.url = format_url(&n.url),
            Node::Delete(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Emphasis(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Footnote(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::FootnoteDefinition(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Heading(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Image(n) => n.url = format_url(&n.url),
            Node::Link(n) => {
                n.url = format_url(&n.url);
                apply_format_url(&mut n.children, Some(format_url));
            }
            Node::LinkReference(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::List(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::ListItem(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Paragraph(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Strong(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::Table(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::TableRow(n) => apply_format_url(&mut n.children, Some(format_url)),
            Node::TableCell(n) => apply_format_url(&mut n.children, Some(format_url)),
            _ => {}
        }
    }
}

fn collect_definition_identifiers(
    nodes: &[Node],
    definition_ids: &mut HashSet<String>,
    footnote_definition_ids: &mut HashSet<String>,
) {
    for node in nodes {
        match node {
            Node::Definition(n) => {
                definition_ids.insert(normalize_identifier(&n.identifier));
                definition_ids.insert(normalize_identifier(&n.label));
            }
            Node::FootnoteDefinition(n) => {
                footnote_definition_ids.insert(normalize_identifier(&n.identifier));
                footnote_definition_ids.insert(normalize_identifier(&n.label));
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Admonition(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Blockquote(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Delete(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Emphasis(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Footnote(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Heading(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Link(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::LinkReference(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::List(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::ListItem(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Paragraph(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Strong(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Table(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::TableRow(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::TableCell(n) => {
                collect_definition_identifiers(
                    &n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            _ => {}
        }
    }
}

fn rewrite_unresolved_references(
    nodes: &mut Vec<Node>,
    definition_ids: &HashSet<String>,
    footnote_definition_ids: &HashSet<String>,
) {
    for node in nodes.iter_mut() {
        match node {
            Node::LinkReference(n) => {
                let identifier = normalize_identifier(&n.identifier);
                if !definition_ids.contains(&identifier) {
                    *node = Node::Text(Text {
                        position: None,
                        value: render_link_reference(n),
                    });
                    continue;
                }
                rewrite_unresolved_references(
                    &mut n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::ImageReference(n) => {
                let identifier = normalize_identifier(&n.identifier);
                if !definition_ids.contains(&identifier) {
                    *node = Node::Text(Text {
                        position: None,
                        value: render_image_reference(
                            &n.alt,
                            &n.label,
                            &n.identifier,
                            n.reference_type,
                        ),
                    });
                }
            }
            Node::FootnoteReference(n) => {
                let identifier = normalize_identifier(&n.identifier);
                if !footnote_definition_ids.contains(&identifier) {
                    *node = Node::Text(Text {
                        position: None,
                        value: format!("[^{}]", n.label),
                    });
                }
            }
            Node::Admonition(n) => {
                rewrite_unresolved_references(
                    &mut n.title,
                    definition_ids,
                    footnote_definition_ids,
                );
                rewrite_unresolved_references(
                    &mut n.children,
                    definition_ids,
                    footnote_definition_ids,
                );
            }
            Node::Blockquote(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Delete(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Emphasis(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Footnote(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::FootnoteDefinition(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Heading(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Link(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::List(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::ListItem(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Paragraph(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Strong(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::Table(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::TableRow(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            Node::TableCell(n) => rewrite_unresolved_references(
                &mut n.children,
                definition_ids,
                footnote_definition_ids,
            ),
            _ => {}
        }
    }
}

fn rebalance_chained_link_references(nodes: &mut Vec<Node>) {
    for node in nodes.iter_mut() {
        match node {
            Node::Admonition(n) => {
                rebalance_chained_link_references(&mut n.title);
                rebalance_chained_link_references(&mut n.children);
            }
            Node::Blockquote(n) => rebalance_chained_link_references(&mut n.children),
            Node::Delete(n) => rebalance_chained_link_references(&mut n.children),
            Node::Emphasis(n) => rebalance_chained_link_references(&mut n.children),
            Node::Footnote(n) => rebalance_chained_link_references(&mut n.children),
            Node::FootnoteDefinition(n) => rebalance_chained_link_references(&mut n.children),
            Node::Heading(n) => rebalance_chained_link_references(&mut n.children),
            Node::Link(n) => rebalance_chained_link_references(&mut n.children),
            Node::LinkReference(n) => rebalance_chained_link_references(&mut n.children),
            Node::List(n) => rebalance_chained_link_references(&mut n.children),
            Node::ListItem(n) => rebalance_chained_link_references(&mut n.children),
            Node::Paragraph(n) => rebalance_chained_link_references(&mut n.children),
            Node::Strong(n) => rebalance_chained_link_references(&mut n.children),
            Node::Table(n) => rebalance_chained_link_references(&mut n.children),
            Node::TableRow(n) => rebalance_chained_link_references(&mut n.children),
            Node::TableCell(n) => rebalance_chained_link_references(&mut n.children),
            _ => {}
        }
    }

    let mut i = 0usize;
    while i + 1 < nodes.len() {
        let Some(text_value) = nodes.get(i).and_then(|n| match n {
            Node::Text(t) => Some(t.value.as_str()),
            _ => None,
        }) else {
            i += 1;
            continue;
        };

        let Some(shortcut_ref) = nodes.get(i + 1).and_then(|n| match n {
            Node::LinkReference(lr) if lr.reference_type == ReferenceType::Shortcut => Some(lr),
            _ => None,
        }) else {
            i += 1;
            continue;
        };

        let Some((prefix, first_label, second_label)) = split_trailing_double_label(text_value)
        else {
            i += 1;
            continue;
        };

        let mut replacement = Vec::new();
        if !prefix.is_empty() {
            replacement.push(Node::Text(Text {
                position: None,
                value: prefix,
            }));
        }

        replacement.push(Node::Text(Text {
            position: None,
            value: format!("[{first_label}]"),
        }));

        replacement.push(Node::LinkReference(yozora_ast::LinkReference {
            position: None,
            identifier: shortcut_ref.identifier.clone(),
            label: shortcut_ref.label.clone(),
            reference_type: ReferenceType::Full,
            children: vec![Node::Text(Text {
                position: None,
                value: second_label,
            })],
        }));

        nodes.splice(i..=i + 1, replacement);
    }
}

fn split_trailing_double_label(input: &str) -> Option<(String, String, String)> {
    if !input.ends_with(']') {
        return None;
    }

    let end = input.len() - 1;
    let second_open = input[..end].rfind('[')?;
    let second = &input[second_open + 1..end];
    if second.is_empty() || second.contains('[') || second.contains(']') {
        return None;
    }

    if second_open == 0 || input.as_bytes()[second_open - 1] != b']' {
        return None;
    }

    let first_close = second_open - 1;
    let first_open = input[..first_close].rfind('[')?;
    let first = &input[first_open + 1..first_close];
    if first.is_empty() || first.contains('[') || first.contains(']') {
        return None;
    }

    let prefix = input[..first_open].to_string();
    Some((prefix, first.to_string(), second.to_string()))
}

fn render_link_reference(node: &yozora_ast::LinkReference) -> String {
    let primary_label_raw = node
        .children
        .first()
        .and_then(|n| match n {
            Node::Text(t) => Some(t.value.as_str()),
            _ => None,
        })
        .unwrap_or(node.label.as_str());
    let primary_label = decode_label_escapes(primary_label_raw);

    let secondary_label = decode_label_escapes(node.label.as_str());

    match node.reference_type {
        ReferenceType::Shortcut => format!("[{primary_label}]"),
        ReferenceType::Collapsed => format!("[{primary_label}][]"),
        ReferenceType::Full => format!("[{primary_label}][{secondary_label}]"),
    }
}

fn decode_label_escapes(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                if next.is_ascii_punctuation() {
                    out.push(next);
                } else {
                    out.push('\\');
                    out.push(next);
                }
            } else {
                out.push('\\');
            }
            continue;
        }
        out.push(ch);
    }

    out
}

fn render_image_reference(
    alt: &str,
    _label: &str,
    identifier: &str,
    reference_type: ReferenceType,
) -> String {
    match reference_type {
        ReferenceType::Shortcut => format!("![{alt}]"),
        ReferenceType::Collapsed => format!("![{alt}][]"),
        ReferenceType::Full => format!("![{alt}][{identifier}]"),
    }
}

fn normalize_identifier(input: &str) -> String {
    let mut normalized = String::new();
    for (idx, part) in input.split_whitespace().enumerate() {
        if idx > 0 {
            normalized.push(' ');
        }
        normalized.push_str(part);
    }
    fold_case(&normalized)
}

fn merge_adjacent_text_nodes(nodes: &mut Vec<Node>) {
    for node in nodes.iter_mut() {
        if let Some(children) = node_children_mut(node) {
            merge_adjacent_text_nodes(children);
        }
    }

    let mut merged = Vec::with_capacity(nodes.len());
    for node in std::mem::take(nodes) {
        match node {
            Node::Text(text) => {
                if let Some(Node::Text(last)) = merged.last_mut() {
                    last.value.push_str(&text.value);
                } else {
                    merged.push(Node::Text(text));
                }
            }
            other => merged.push(other),
        }
    }
    *nodes = merged;
}

fn node_children_mut(node: &mut Node) -> Option<&mut Vec<Node>> {
    match node {
        Node::Admonition(n) => Some(&mut n.children),
        Node::Blockquote(n) => Some(&mut n.children),
        Node::Delete(n) => Some(&mut n.children),
        Node::Emphasis(n) => Some(&mut n.children),
        Node::Footnote(n) => Some(&mut n.children),
        Node::FootnoteDefinition(n) => Some(&mut n.children),
        Node::Heading(n) => Some(&mut n.children),
        Node::Link(n) => Some(&mut n.children),
        Node::LinkReference(n) => Some(&mut n.children),
        Node::List(n) => Some(&mut n.children),
        Node::ListItem(n) => Some(&mut n.children),
        Node::Paragraph(n) => Some(&mut n.children),
        Node::Strong(n) => Some(&mut n.children),
        Node::Table(n) => Some(&mut n.children),
        Node::TableRow(n) => Some(&mut n.children),
        Node::TableCell(n) => Some(&mut n.children),
        _ => None,
    }
}

pub trait Parser {
    fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String>;

    fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String>;

    fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self;

    fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self;

    fn set_default_parse_options(&mut self, options: ParseOptions);

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>;
}

impl Parser for DefaultParser {
    fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        DefaultParser::use_tokenizer(self, tokenizer, register_before_tokenizer)
    }

    fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        DefaultParser::replace_tokenizer(self, tokenizer, register_before_tokenizer)
    }

    fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        DefaultParser::unmount_tokenizer(self, tokenizer_name)
    }

    fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        DefaultParser::use_fallback_tokenizer(self, tokenizer)
    }

    fn set_default_parse_options(&mut self, options: ParseOptions) {
        DefaultParser::set_default_parse_options(self, options)
    }

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        DefaultParser::parse(self, contents, options)
    }
}

pub const _TOKENIZER_KIND_SENTINEL: TokenizerKind = TokenizerKind::Block;

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::{Paragraph, Text};
    use yozora_core_tokenizer::TokenizerMeta;

    #[derive(Debug, Clone)]
    struct DummyBlockTokenizer {
        meta: TokenizerMeta,
    }

    impl DummyBlockTokenizer {
        fn new(name: &str, priority: i32) -> Self {
            Self {
                meta: TokenizerMeta {
                    name: name.to_string(),
                    kind: TokenizerKind::Block,
                    priority,
                },
            }
        }
    }

    impl yozora_core_tokenizer::Tokenizer for DummyBlockTokenizer {
        fn meta(&self) -> &TokenizerMeta {
            &self.meta
        }
    }

    impl BlockTokenizer for DummyBlockTokenizer {}

    #[derive(Debug, Clone)]
    struct DummyInlineTokenizer {
        meta: TokenizerMeta,
    }

    impl DummyInlineTokenizer {
        fn new(name: &str, priority: i32) -> Self {
            Self {
                meta: TokenizerMeta {
                    name: name.to_string(),
                    kind: TokenizerKind::Inline,
                    priority,
                },
            }
        }
    }

    impl yozora_core_tokenizer::Tokenizer for DummyInlineTokenizer {
        fn meta(&self) -> &TokenizerMeta {
            &self.meta
        }
    }

    impl InlineTokenizer for DummyInlineTokenizer {}

    #[derive(Debug, Clone)]
    struct DummyBlockFallbackTokenizer {
        meta: TokenizerMeta,
    }

    impl DummyBlockFallbackTokenizer {
        fn new(name: &str) -> Self {
            Self {
                meta: TokenizerMeta {
                    name: name.to_string(),
                    kind: TokenizerKind::Block,
                    priority: -1,
                },
            }
        }
    }

    impl yozora_core_tokenizer::Tokenizer for DummyBlockFallbackTokenizer {
        fn meta(&self) -> &TokenizerMeta {
            &self.meta
        }
    }

    impl BlockTokenizer for DummyBlockFallbackTokenizer {}

    impl BlockFallbackTokenizer for DummyBlockFallbackTokenizer {
        fn build_block(&self, inline_children: Vec<Node>, position: Option<Position>) -> Node {
            Node::Paragraph(Paragraph {
                position,
                children: inline_children,
            })
        }
    }

    #[derive(Debug, Clone)]
    struct DummyInlineFallbackTokenizer {
        meta: TokenizerMeta,
    }

    impl DummyInlineFallbackTokenizer {
        fn new(name: &str) -> Self {
            Self {
                meta: TokenizerMeta {
                    name: name.to_string(),
                    kind: TokenizerKind::Inline,
                    priority: -1,
                },
            }
        }
    }

    impl yozora_core_tokenizer::Tokenizer for DummyInlineFallbackTokenizer {
        fn meta(&self) -> &TokenizerMeta {
            &self.meta
        }
    }

    impl InlineTokenizer for DummyInlineFallbackTokenizer {}

    impl InlineFallbackTokenizer for DummyInlineFallbackTokenizer {
        fn build_inline(&self, value: &str, position: Option<Position>) -> Node {
            Node::Text(Text {
                position,
                value: format!("inline:{value}"),
            })
        }
    }

    #[test]
    fn parse_with_fallback_builds_root_paragraph_text() {
        let mut parser = DefaultParser::new();
        parser
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockFallbackTokenizer::new("fallback-block"),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
                DummyInlineFallbackTokenizer::new("fallback-inline"),
            )));

        let root = parser.parse("hello", None);
        assert_eq!(root.children.len(), 1);

        let paragraph = match &root.children[0] {
            Node::Paragraph(node) => node,
            other => panic!("expected paragraph node, got {other:?}"),
        };
        assert_eq!(paragraph.children.len(), 1);

        let text = match &paragraph.children[0] {
            Node::Text(node) => node,
            other => panic!("expected text node, got {other:?}"),
        };
        assert_eq!(text.value, "inline:hello");
    }

    #[test]
    fn parse_without_inline_fallback_uses_default_text_node() {
        let mut parser = DefaultParser::new();
        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
            DummyBlockFallbackTokenizer::new("fallback-block"),
        )));

        let root = parser.parse("hello", None);
        let paragraph = match &root.children[0] {
            Node::Paragraph(node) => node,
            other => panic!("expected paragraph node, got {other:?}"),
        };

        let text = match &paragraph.children[0] {
            Node::Text(node) => node,
            other => panic!("expected text node, got {other:?}"),
        };
        assert_eq!(text.value, "hello");
    }

    #[test]
    fn set_default_parse_options_applies_to_parse() {
        let mut parser = DefaultParser::new();
        parser
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockFallbackTokenizer::new("fallback-block"),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
                DummyInlineFallbackTokenizer::new("fallback-inline"),
            )));
        parser.set_default_parse_options(ParseOptions {
            should_reserve_position: true,
            ..ParseOptions::default()
        });

        let root = parser.parse("hello", None);
        assert!(root.position.is_some());
    }

    #[test]
    fn parse_options_can_override_default_options() {
        let mut parser = DefaultParser::new();
        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
            DummyBlockFallbackTokenizer::new("fallback-block"),
        )));
        parser.set_default_parse_options(ParseOptions {
            should_reserve_position: false,
            ..ParseOptions::default()
        });

        let root = parser.parse(
            "hello",
            Some(ParseOptions {
                should_reserve_position: true,
                ..ParseOptions::default()
            }),
        );
        assert!(root.position.is_some());
    }

    #[test]
    fn use_replace_unmount_tokenizer_lifecycle() {
        let mut parser = DefaultParser::new();

        parser
            .use_tokenizer(
                AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("block-a", 10))),
                None,
            )
            .expect("initial block tokenizer should register");
        let duplicate = parser.use_tokenizer(
            AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("block-a", 5))),
            None,
        );
        assert!(duplicate.is_err());

        parser
            .replace_tokenizer(
                AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("block-a", 1))),
                None,
            )
            .expect("replace block tokenizer should succeed");

        parser.unmount_tokenizer("block-a");
        parser
            .use_tokenizer(
                AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("block-a", 3))),
                None,
            )
            .expect("re-register after unmount should succeed");

        parser
            .use_tokenizer(
                AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("inline-a", 10))),
                None,
            )
            .expect("initial inline tokenizer should register");
        parser.unmount_tokenizer("inline-a");
        parser
            .use_tokenizer(
                AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("inline-a", 4))),
                None,
            )
            .expect("re-register inline tokenizer should succeed");
    }

    #[test]
    fn unmount_tokenizer_can_remove_fallback_tokenizer() {
        let mut parser = DefaultParser::new();
        parser
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockFallbackTokenizer::new("fallback-block"),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
                DummyInlineFallbackTokenizer::new("fallback-inline"),
            )));

        parser.unmount_tokenizer("fallback-block");
        let root = parser.parse("hello", None);
        assert!(root.children.is_empty());

        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
            DummyBlockFallbackTokenizer::new("fallback-block"),
        )));
        parser.unmount_tokenizer("fallback-inline");

        let root = parser.parse("hello", None);
        let paragraph = match &root.children[0] {
            Node::Paragraph(node) => node,
            other => panic!("expected paragraph node, got {other:?}"),
        };
        let text = match &paragraph.children[0] {
            Node::Text(node) => node,
            other => panic!("expected text node, got {other:?}"),
        };
        assert_eq!(text.value, "hello");
    }

    #[test]
    fn parse_accepts_lines_input() {
        let mut parser = DefaultParser::new();
        parser
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockFallbackTokenizer::new("fallback-block"),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
                DummyInlineFallbackTokenizer::new("fallback-inline"),
            )));

        let lines = ["hello", "world"];
        let root = parser.parse(ParseContents::Lines(&lines), None);
        let paragraph = match &root.children[0] {
            Node::Paragraph(node) => node,
            other => panic!("expected paragraph node, got {other:?}"),
        };
        let text = match &paragraph.children[0] {
            Node::Text(node) => node,
            other => panic!("expected text node, got {other:?}"),
        };
        assert_eq!(text.value, "inline:hello\nworld");
    }

    #[test]
    fn parse_accepts_owned_text_and_lines_inputs() {
        let mut parser = DefaultParser::new();
        parser
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockFallbackTokenizer::new("fallback-block"),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
                DummyInlineFallbackTokenizer::new("fallback-inline"),
            )));

        let root_from_string = parser.parse(String::from("hello"), None);
        assert_eq!(root_from_string.children.len(), 1);

        let root_from_vec = parser.parse(vec!["hello", "world"], None);
        let paragraph = match &root_from_vec.children[0] {
            Node::Paragraph(node) => node,
            other => panic!("expected paragraph node, got {other:?}"),
        };
        let text = match &paragraph.children[0] {
            Node::Text(node) => node,
            other => panic!("expected text node, got {other:?}"),
        };
        assert_eq!(text.value, "inline:hello\nworld");
    }

    #[test]
    fn parser_ts_methods_are_callable() {
        let mut parser = DefaultParser::new();
        parser
            .useFallbackTokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockFallbackTokenizer::new("fallback-block"),
            )))
            .useFallbackTokenizer(AnyFallbackTokenizer::Inline(Box::new(
                DummyInlineFallbackTokenizer::new("fallback-inline"),
            )));
        parser.setDefaultParseOptions(ParseOptions {
            should_reserve_position: true,
            ..ParseOptions::default()
        });

        let lines = ["compat"];
        let root = parser.parse(ParseContents::Lines(&lines), None);
        assert!(root.position.is_some());
        assert_eq!(root.children.len(), 1);
    }
}
