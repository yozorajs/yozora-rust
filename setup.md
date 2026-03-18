# yozora-rust Setup

## 0. 前置条件

- `rustup` + `cargo`
- 可选: `jq`（排查 fixture 数据时更方便）

## 1. 安装依赖并编译

```bash
cargo check --workspace
```

## 2. 一键跑主测试

```bash
cargo test --workspace -- --skip fixture_runner_smoke_case
```

说明:

- `fixture_runner_smoke_case` 依赖 `fixtures/local/smoke/minimal.json`。
- 当前仓库默认不保证该本地样例存在，所以默认跳过。

## 3. 跑上游 fixture 回归（按 parser profile）

```bash
# profile: yozora | gfm | gfm_ex
YOZORA_PARSER_PROFILE=yozora YOZORA_ASSERT_LEVEL=l1 cargo test -p yozora-test-util fixture_batch_report -- --ignored
```

可选严格模式（位置断言，等价 L2）:

```bash
YOZORA_PARSER_PROFILE=yozora YOZORA_ASSERT_LEVEL=l2 YOZORA_FAIL_ON_DIFF=1 cargo test -p yozora-test-util fixture_batch_report -- --ignored
```

## 4. 常用开发循环

```bash
# 只测核心解析器
cargo test -p yozora-core-parser

# 只测预设 parser
cargo test -p yozora-parser -p yozora-parser-gfm -p yozora-parser-gfm-ex
```

