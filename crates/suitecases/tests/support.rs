#![allow(dead_code)]

use serde_json::Value;
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;
use yozora_suitecases::{AssertLevel, SuiteAdapter};
use yozora_tokenizer_inline_math::INLINE_MATH_TOKENIZER_NAME;

pub struct ParserSuiteAdapter {
    profile: String,
    parse_options: Option<ParseOptions>,
}

impl ParserSuiteAdapter {
    pub fn new(profile: &str, assert_level: AssertLevel) -> Self {
        let parse_options = Some(ParseOptions {
            should_reserve_position: Some(matches!(assert_level, AssertLevel::L2)),
            ..ParseOptions::default()
        });

        Self {
            profile: profile.to_string(),
            parse_options,
        }
    }
}

impl SuiteAdapter for ParserSuiteAdapter {
    fn name(&self) -> &str {
        &self.profile
    }

    fn run_case(&self, input: &str) -> Result<Value, String> {
        let root = match self.profile.as_str() {
            "yozora" => YozoraParser::default().parse(input, self.parse_options.clone()),
            "yozora_inline_math_backtick_required" => {
                let mut parser = YozoraParser::default();
                parser.unmount_inline_tokenizer(INLINE_MATH_TOKENIZER_NAME);
                parser.parse(input, self.parse_options.clone())
            }
            "gfm" => GfmParser::default().parse(input, self.parse_options.clone()),
            "gfm_ex" => GfmExParser::default().parse(input, self.parse_options.clone()),
            other => return Err(format!("unsupported parser profile: {other}")),
        };

        serde_json::to_value(root).map_err(|err| format!("serialize parse result failed: {err}"))
    }
}

pub fn env_parser_profile() -> String {
    std::env::var("YOZORA_PARSER_PROFILE").unwrap_or_else(|_| "yozora".to_string())
}

pub fn env_assert_level() -> AssertLevel {
    std::env::var("YOZORA_ASSERT_LEVEL")
        .ok()
        .and_then(|value| value.parse::<AssertLevel>().ok())
        .unwrap_or(AssertLevel::L2)
}

pub fn env_fail_on_diff() -> bool {
    std::env::var("YOZORA_FAIL_ON_DIFF").is_ok_and(|value| value == "1")
}

pub fn env_report_max_print() -> usize {
    std::env::var("YOZORA_REPORT_MAX_PRINT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20)
}
