# yozora Rust 复刻契约

## 基准

- 唯一行为、API 与实现结构基准：`/Users/wanchenfang/ws/github/yozora/yozora`
- 基准 commit：`4bbc3d4f591a87efe7d7f474c1f9f07f0dcbfdc1`
- 后续同步只前移到用户明确指定的新基准；不得自行设计替代语义。

## 范围

Rust workspace 逐包映射以下 production package，并保留独立 crate 边界：

- `ast`
- `ast-util`
- `character`
- `core-tokenizer`
- `core-parser`
- `invariant`
- `markup-weaver`
- `parser-gfm`
- `parser-gfm-ex`
- `parser`
- `test-util`
- 全部 `tokenizer-*`

`suitecases` 仅作为 Rust 对拍基础设施，不替代 `test-util` 的公开契约。

## 命名与结构

- TypeScript `camelCase` / `PascalCase` 分别映射为 Rust `snake_case` / `PascalCase`。
- package、module、type、function、field 和 tokenizer unique name 均按上游同义映射。
- 上游同级模块在 Rust 中保持同级职责；公共 helper 不合并进 tokenizer 主实现。
- Rust 必需的 ownership、lifetime、trait object 和 enum 表达可以不同，但不得改变输入、输出、默认值、错误条件、执行顺序或 AST JSON。

## 核心边界与数据流

```text
ast <- character <- core-tokenizer <- core-parser <- parser-*
  ^                         ^              ^
  |                         |              |
ast-util              tokenizer-*      test-util
  ^
  |
markup-weaver
```

- `core-parser` 是 tokenizer 注册状态的唯一 owner。
- tokenizer 通过 `match` / `parse` 两阶段 contract 接入；core 不依赖具体 tokenizer crate。
- token dispatch 与上游一致使用 tokenizer name；移除上游不存在的 hash UID、UID map 和冲突语义。
- parser profile 只负责按上游顺序装配 tokenizer，不复制 core parsing logic。
- core 在没有 optional tokenizer 时仍可运行；注册、替换、卸载和 fallback 使用统一 contract。
- block parse hook 与上游一致支持同步 nodes 或 lazy generator；generator 只通过
  `request_block_tokens` 请求 child parsing，core scheduler 是 frame、active token 与 error
  propagation 的唯一 owner。

## 错误与默认行为

- 重复 tokenizer name、fallback name collision 和缺失 parse hook 的失败条件及消息对齐上游。
- 未显式配置时，`should_reserve_position = false`、definition 列表为空、URL formatter 为 `encode_link_destination`。
- tokenizer hook 错误不被静默吞掉；只在上游明确 rollback 的分支执行 rollback。
- 外部输入边界验证 fixture schema、node interval 和 parser profile；内部 invariant 失败立即终止当前操作。

## 验收

1. exact-tag `fixtures/gfm` 与 `fixtures/custom` 全量纳入仓库，目录与内容保持上游布局。
2. `gfm`、`gfm-ex`、`yozora`、inline-math backtick-required 四个 profile 在 L1/L2 均为零差异。
3. 上游 package 与 tokenizer 单测语义逐项迁移；stack-safety 与大输入回归通过。
4. 公共 export 对照表无缺项；Rust 命名仅做约定允许的 snake_case 转换。
5. `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 全部通过。
6. `test-parity/reference-tests.json` 固定上游 direct tests、fixture scanners、fixture 文件 hash 与 fixture cases；
   `test-parity/coverage.json` 为每个适用 direct case 显式登记 case ID 并指向 Rust tests，
   只为不可表示的语言差异保留精确 `N/A`。

Reference test inventory 通过以下命令防止漂移：

```sh
./script/test-reference-parity.sh /path/to/yozora
```

## 实施顺序

1. 固定 exact-tag fixtures 与对拍 profile。
2. 对齐 AST、character、core contracts，移除非上游状态。
3. 对齐全部 tokenizer 与 parser profile。
4. 完整迁移 `ast-util`、`invariant`、`markup-weaver`、`test-util`。
5. 补齐公开 export 对照与全量验证。

## Open Questions

无。实现选择均由上述基准 commit 的源码、测试与 fixture 决定。
