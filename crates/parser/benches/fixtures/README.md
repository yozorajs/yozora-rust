# Benchmark Fixtures

## commonmark_spec.md

- Source: https://raw.githubusercontent.com/commonmark/commonmark-spec/master/spec.txt
- Upstream project: https://github.com/commonmark/commonmark-spec
- Retrieved at: 2026-03-25 (Asia/Singapore)
- Retrieval command:

```bash
curl -fsSL https://raw.githubusercontent.com/commonmark/commonmark-spec/master/spec.txt \
  -o crates/parser/benches/fixtures/commonmark_spec.md
```

This fixture is intentionally kept as an upstream snapshot to benchmark parser behavior on a large, real-world Markdown specification document instead of synthetic repeated content.

## github_basic_writing_syntax.md

- Source: https://raw.githubusercontent.com/github/docs/main/content/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax.md
- Upstream project: https://github.com/github/docs
- Retrieved at: 2026-03-25 (Asia/Singapore)
- Retrieval command:

```bash
curl -fsSL https://raw.githubusercontent.com/github/docs/main/content/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax.md \
  -o crates/parser/benches/fixtures/github_basic_writing_syntax.md
```

This fixture represents a medium-sized technical Markdown guide with headings, lists, links, tables, and fenced code blocks.

## rfc9110.txt

- Source: https://www.rfc-editor.org/rfc/rfc9110.txt
- Upstream project: https://www.rfc-editor.org/
- Retrieved at: 2026-03-25 (Asia/Singapore)
- Retrieval command:

```bash
curl -fsSL https://www.rfc-editor.org/rfc/rfc9110.txt \
  -o crates/parser/benches/fixtures/rfc9110.txt
```

This fixture represents a very large neutral technical document to stress parser throughput and memory behavior under long plain-text sections.
