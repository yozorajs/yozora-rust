use std::sync::OnceLock;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;

fn benchmark_input() -> &'static str {
    static INPUT: OnceLock<String> = OnceLock::new();
    INPUT
        .get_or_init(|| {
            let section = r#"
# Heading 1

A paragraph with **strong**, *emphasis*, ~~delete~~, `inline code`, [link](https://example.com), ![img](https://img.example.com/a.png), and <https://example.com>.

> blockquote line 1
> blockquote line 2 with `code`

1. ordered item
2. ordered item with nested list
   - child item A
   - child item B

- [x] task item done
- [ ] task item todo

| col_a | col_b | col_c |
| :--- | ---: | :---: |
| a    | b    | c     |
| aa   | bb   | cc    |

```ts
export const value = 42
console.log(value)
```

$$
a^2 + b^2 = c^2
$$

Paragraph tail with escaped chars: \* \_ \`.
"#;

            section.repeat(50)
        })
        .as_str()
}

fn parse_options(should_reserve_position: bool) -> ParseOptions {
    ParseOptions {
        shouldReservePosition: Some(should_reserve_position),
        ..ParseOptions::default()
    }
}

fn bench_profile(c: &mut Criterion, should_reserve_position: bool) {
    let mut group = c.benchmark_group(if should_reserve_position {
        "parser_parse/with_position"
    } else {
        "parser_parse/without_position"
    });

    let input = benchmark_input();
    let options = parse_options(should_reserve_position);

    let yozora = YozoraParser::default();
    group.bench_with_input(BenchmarkId::new("yozora", input.len()), &input, |b, content| {
        b.iter(|| {
            let ast = yozora.parse(black_box(*content), Some(options.clone()));
            black_box(ast);
        })
    });

    let gfm = GfmParser::default();
    group.bench_with_input(BenchmarkId::new("gfm", input.len()), &input, |b, content| {
        b.iter(|| {
            let ast = gfm.parse(black_box(*content), Some(options.clone()));
            black_box(ast);
        })
    });

    let gfm_ex = GfmExParser::default();
    group.bench_with_input(BenchmarkId::new("gfm_ex", input.len()), &input, |b, content| {
        b.iter(|| {
            let ast = gfm_ex.parse(black_box(*content), Some(options.clone()));
            black_box(ast);
        })
    });

    group.finish();
}

fn bench_parser_parse(c: &mut Criterion) {
    bench_profile(c, false);
    bench_profile(c, true);
}

criterion_group!(benches, bench_parser_parse);
criterion_main!(benches);
