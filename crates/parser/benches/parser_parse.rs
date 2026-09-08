use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use yozora_core_parser::ParseOptions;
use yozora_parser::YozoraParser;
use yozora_parser_gfm::GfmParser;
use yozora_parser_gfm_ex::GfmExParser;

struct BenchmarkFixture {
    name: &'static str,
    content: &'static str,
}

fn benchmark_fixtures() -> &'static [BenchmarkFixture] {
    &[
        BenchmarkFixture {
            name: "commonmark_spec",
            content: include_str!("fixtures/commonmark_spec.md"),
        },
        BenchmarkFixture {
            name: "github_basic_writing_syntax",
            content: include_str!("fixtures/github_basic_writing_syntax.md"),
        },
        BenchmarkFixture {
            name: "rfc9110_txt",
            content: include_str!("fixtures/rfc9110.txt"),
        },
    ]
}

fn parse_options(should_reserve_position: bool) -> ParseOptions {
    ParseOptions {
        should_reserve_position: Some(should_reserve_position),
        ..ParseOptions::default()
    }
}

fn bench_profile(c: &mut Criterion, fixture: &BenchmarkFixture, should_reserve_position: bool) {
    let mut group = c.benchmark_group(if should_reserve_position {
        format!("parser_parse/{}/with_position", fixture.name)
    } else {
        format!("parser_parse/{}/without_position", fixture.name)
    });

    let input = fixture.content;
    let options = parse_options(should_reserve_position);

    let yozora = YozoraParser::default();
    group.bench_with_input(
        BenchmarkId::new("yozora", input.len()),
        &input,
        |b, content| {
            b.iter(|| {
                let ast = yozora.parse(black_box(*content), Some(options.clone()));
                black_box(ast);
            })
        },
    );

    let gfm = GfmParser::default();
    group.bench_with_input(
        BenchmarkId::new("gfm", input.len()),
        &input,
        |b, content| {
            b.iter(|| {
                let ast = gfm.parse(black_box(*content), Some(options.clone()));
                black_box(ast);
            })
        },
    );

    let gfm_ex = GfmExParser::default();
    group.bench_with_input(
        BenchmarkId::new("gfm_ex", input.len()),
        &input,
        |b, content| {
            b.iter(|| {
                let ast = gfm_ex.parse(black_box(*content), Some(options.clone()));
                black_box(ast);
            })
        },
    );

    group.finish();
}

fn bench_parser_parse(c: &mut Criterion) {
    for fixture in benchmark_fixtures() {
        bench_profile(c, fixture, false);
        bench_profile(c, fixture, true);
    }
}

fn bench_multiline_paragraphs(c: &mut Criterion) {
    let parser = YozoraParser::default();
    let options = parse_options(true);
    let mut group = c.benchmark_group("parser_parse/multiline_paragraphs");
    for count in [1_000, 4_000, 8_000] {
        for (name, line, suffix) in [
            ("plain", "text\n", ""),
            ("references", "[t][old]\n", "\n\n[old]: /url"),
        ] {
            let input = format!("{}{suffix}", line.repeat(count));
            group.bench_with_input(BenchmarkId::new(name, count), &input, |b, content| {
                b.iter(|| black_box(parser.parse(black_box(content), Some(options.clone()))));
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_parser_parse, bench_multiline_paragraphs);
criterion_main!(benches);
