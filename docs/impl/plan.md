# yozora-rust parser/tokenizer-*/ast/character/ast-util 复刻实施计划

## 1. 目标与范围

### 1.1 目标

在 `yozora-rust` 中按 `../yozora` 的架构和行为做 **一比一镜像**（当前阶段覆盖 `ast/character/parser/crates/tokenizer-*`，并落地 `ast-util` 骨架），先得到可插拔的 markdown-like 解析内核，再逐步迁移 tokenizer 集合。

### 1.2 当前范围（In Scope）

- monorepo/workspace 结构（`ast/character/ast-util/parser/crates/tokenizer-*` 相关 crate）
- `ast` crate（节点类型、公共 AST 结构、位置信息模型）
- `character` crate（`NodePoint`、字符分类、位置生成器）
- `ast-util` crate 骨架（crate 边界、模块占位与最小导出）
- `core-tokenizer` 双阶段接口（`match` / `parse`）
- `core-parser` 处理管线（block 阶段 + inline 阶段 + fallback）
- parser 组装层（`parser` / `parser-gfm` / `parser-gfm-ex`）
- `crates/tokenizer-*` 的独立 crate 与可插拔注册机制
- fixtures 驱动的一致性测试（优先复用 `../yozora/fixtures`）

### 1.3 暂不覆盖（Out of Scope）

- `ast-util` 的完整功能迁移（`collect/mutate/mutate-async` 等）
- `markup-weaver`、render/react 生态
- 文档站、发布流程、版本变更自动化
- 非 `ast/character/parser/crates/tokenizer-*` 的完整生态迁移


## 2. 设计原则

- 行为镜像优先：先对齐语义与测试结果，再考虑 Rust 风格优化。
- 分层单向依赖：`ast -> ast-util`、`ast/character -> core-tokenizer -> core-parser -> parser-* -> crates/tokenizer-* (装配使用)`，禁止反向依赖。
- 可插拔优先：任何 tokenizer 必须可独立注册、替换、卸载、重排。
- 回归可证：每个里程碑必须通过 fixture 对比验证（至少 AST 结构一致）。


## 3. 方案对比与结论

### 3.1 方案 A（已选）: 一比一镜像

- 做法：按 `../yozora` 同构拆包，保留 `match/parse` 双阶段 API、priority/fallback/rollback 语义。
- 优点：迁移风险最低；fixtures 可直接复用；定位差异成本最低。
- 缺点：初期代码量较大；部分实现不够 Rust-idiomatic。

### 3.2 方案 B: Rust-idiomatic 重构

- 做法：直接用单阶段规则引擎 + Rust 定制抽象，不保留原 hooks 形态。
- 优点：长期代码更“Rust”。
- 缺点：无法直接一比一验证；行为漂移风险高。

### 3.3 方案 C: 镜像内核 + 同步重构

- 做法：一边镜像，一边把接口改成 Rust 化。
- 优点：理论上可少一次重构。
- 缺点：耦合高，排障困难，进度和质量都容易失控。

推荐结论：当前阶段固定采用 **方案 A**。理由是目标明确要求“一比一复刻”，应把“正确性可证明”放在第一优先级。


## 4. 目标仓库结构（首期）

```text
yozora-rust/
├── Cargo.toml
├── crates/
│   ├── ast/
│   ├── ast-util/
│   ├── character/
│   ├── core-tokenizer/
│   ├── core-parser/
│   ├── parser/
│   ├── parser-gfm/
│   ├── parser-gfm-ex/
│   ├── test-util/
│   ├── tokenizer-paragraph/
│   ├── tokenizer-text/
│   ├── tokenizer-emphasis/
│   ├── tokenizer-inline-code/
│   ├── tokenizer-heading/
│   └── ...
└── fixtures/               # 首期可软链接到 ../yozora/fixtures 或复制快照
```

说明：

- `ast` / `character` 作为基础能力层，首期即完整镜像。
- `ast-util` 首期仅建骨架与最小导出，完整能力放到后续里程碑。
- `core-tokenizer` 与 `core-parser` 是唯一内核层。
- `parser*` 只做 tokenizer 装配，不实现规则细节。
- `crates/tokenizer-*` 每个 crate 只负责单一语法单元（SRP）。


## 5. 核心接口镜像设计

### 5.0 `ast` / `character` 基础契约

- `ast`：节点类型枚举、`Root`/`Parent`/`Literal` 等核心结构、`Position/Point`。
- `character`：`NodePoint`、`NodeInterval`、`create_node_point_generator`、字符分类函数。
- 要求：索引与位置计算语义必须与上游一致，供 `core-tokenizer/core-parser` 复用。

### 5.0.1 `ast-util` 首期策略

- 首期目标：仅创建 `ast-util` crate、模块边界与公共入口，不阻塞 parser 主线。
- 导出策略：提供最小稳定入口（可为空实现或 `todo!()` 占位，但接口路径固定）。
- 约束：`ast-util` 不得反向依赖 parser/crates/tokenizer-*，保持工具层定位。

### 5.1 Tokenizer 基础模型

- `TokenizerKind`：`Block | Inline`
- `name`：全局唯一（用于替换/卸载/fallback 指向）
- `priority`：决定匹配顺序

### 5.2 Block 双阶段

- `match` 阶段：产生中间 token tree（可包含 rollback）
- `parse` 阶段：按 tokenizer 分组消费 token，产出 AST nodes

### 5.3 Inline 双阶段

- delimiter 扫描 -> pairing -> 内部递归解析 -> fallback 补齐
- 与上游一致保留“按 priority 分组 + DFS 递归处理”策略

### 5.4 fallback 机制

- block fallback：默认 `paragraph`
- inline fallback：默认 `text`
- 当高优先级 tokenizer 没有覆盖区间时，由 fallback 补齐连续区间


## 6. 可插拔能力约束

`DefaultParser` 必须支持与 TS 版等价能力：

- `use_tokenizer(tokenizer, register_before)`
- `replace_tokenizer(tokenizer, register_before)`
- `unmount_tokenizer(name)`
- `use_fallback_tokenizer(tokenizer)`
- `set_default_parse_options(options)`

行为约束：

- 同名重复注册时报错。
- `replace` 等价于“先卸载同名，再按规则插入”。
- 同类型 tokenizer（block/inline）各自维护独立有序列表与索引映射。


## 7. 三个关键实现示例（含对比）

### 示例 1: tokenizer 注册顺序

- 例子：`InlineCode(priority=10)`、`Emphasis(priority=2)`、`Text(fallback=-1)`
- 对比：
  - 若忽略 priority，只按注册顺序，`*` 与 `` ` `` 混合文本会出现错误吞噬。
  - 按 priority 分层，可保证 code span 比 emphasis 更“紧”。
- 结论：必须保留 priority 插入与分组处理。

### 示例 2: block rollback

- 例子：某行最初被 `table` 识别，后续行不满足闭合条件，需要 rollback 为 `paragraph`。
- 对比：
  - 无 rollback：会产出结构错误且很难补救。
  - 有 rollback：可回放原始 lines，重新匹配为低优先级 token。
- 结论：`rollback_phrasing_lines` 是核心能力，不能省略。

### 示例 3: parser 预设装配

- 例子：`parser-gfm` 与 `parser-gfm-ex` 只差几个 tokenizer（如 `table`、`delete`、`autolink-extension`）。
- 对比：
  - 复制粘贴整套 parser 逻辑：维护成本高，行为漂移快。
  - 共用 `DefaultParser` + 不同装配清单：最稳定。
- 结论：首期坚持“统一内核 + 配置化装配”。


## 8. 里程碑与交付

### M0: Workspace 与骨架

- 建立 workspace、crate 清单、基础 CI（`fmt`/`clippy`/`test`）
- 创建 `ast/ast-util/character/core-tokenizer/core-parser/parser*/test-util` 空壳
- 准备 `fixtures` 读取能力
- 落地 `fixtures/profile-map.toml` 与 `fixtures/known-failures.toml` 模板

### M1: `ast` / `character` 完整镜像

- 镜像 `ast` 的节点类型与公共类型定义
- 镜像 `character` 的 `NodePoint`、字符判定、位置生成能力
- 建立与 parser/tokenizer 依赖方向一致的 crate 关系

验收：`core-tokenizer` 与 `core-parser` 可仅依赖 Rust 版 `ast/character` 编译通过。

### M2: 内核打通（最小可运行）

- 落地 `TokenizerKind/priority/fallback` 基础抽象
- 完成 `DefaultParser` 注册、替换、卸载、parse 主流程
- 接入最小 tokenizer：`paragraph`（block fallback）+ `text`（inline fallback）
- 完成 Comparator 的 `L1`/`L2` 断言实现及单测

验收：可把任意输入解析为 `root -> paragraph -> text`。

### M3: GFM 基线 tokenizer

- 优先迁移：`inline-code`、`emphasis`、`heading`、`thematic-break`、`list`
- 打通 block/inline 交叉场景和优先级冲突场景

验收：选取 `fixtures/gfm` 子集通过。

### M4: parser 预设完成

- 完成 `parser-gfm`、`parser-gfm-ex`、`parser` 的装配清单
- 对齐 fallback 与 parse options 默认行为
- `L2`（strict-position）进入主线门禁

验收：三个 parser 预设在对应 fixture 子集上行为一致。

### M5: tokenizer 扩展批量迁移

- 迁移剩余 `crates/tokenizer-*`
- 扩充 fixture 覆盖并补充回归测试

验收：首批目标 tokenizer 全量可用，差异报告清零或可解释。

### M6: `ast-util` 完整迁移

- 按上游模块补齐 `collect/mutate/mutate-async` 等能力
- 补齐 `ast-util` 单测与集成测试
- 验证 `ast-util` 与 `ast` 的 API 兼容性和性能基线

验收：`ast-util` 对外 API 与上游对齐，核心用例可通过。


## 9. 测试与一致性策略

### 9.1 Fixture 复用

- 输入来源优先复用 `../yozora/fixtures`。
- 每个 case 至少验证：`parseAnswer`（AST 结构）
- `position` 对齐策略：首期可允许“可配置严格模式”

### 9.2 分层测试

- 单元测试：单 tokenizer 的 `match/parse` 行为
- 集成测试：parser 预设 + fixture 批量回归
- 变更测试：`replace/unmount/register_before` 行为回归

### 9.3 差异报告

- 输出 case 级 diff（路径、触发输入、节点路径）
- 禁止“只统计通过率不看差异明细”

### 9.4 Fixture Protocol v1（执行规范）

#### 9.4.1 数据源与目录

- `fixtures/`：上游真值数据镜像（目录结构对齐 `../yozora/fixtures`）
- `fixtures/local/`：Rust 侧增量场景（仅当上游无覆盖时新增）
- 原则：不修改上游 fixture 文件内容，Rust 侧仅新增 harness 与本地增量数据。

#### 9.4.2 Case 展开模型（方案 3）

- 保留“单 JSON 含多 case”的上游格式。
- 测试运行时在内存展开为 `CaseRecord` 列表（case 级执行与断言）。
- 每个 `CaseRecord` 生成稳定 `case_id`：
  - 规则：`{relative_fixture_path}::{case_index}::{description_slug}`
  - 示例：`gfm/emphasis/rule#1/#360.json::0::rule-1`

#### 9.4.3 断言等级

- `L0`（smoke）：仅验证 parser 不 panic 且输出合法 root。
- `L1`（default）：严格校验 `parseAnswer` AST 结构（不含 position）。
- `L2`（strict-position）：在 `L1` 基础上严格校验 `position`。
- `L3`（future）：校验 `markupAnswer`/`htmlAnswer`（当前阶段不启用）。

默认策略：

- 本地开发默认 `L1`。
- CI 主流程至少覆盖 `L1`。
- 夜间任务覆盖 `L2`（position 严格模式）。

#### 9.4.4 执行过滤与调试

- `YOZORA_FIXTURE_GLOB`：按路径模式筛选 fixture。
- `YOZORA_CASE_ID`：仅执行单 case。
- `YOZORA_PARSER_PROFILE`：指定 parser 预设（`gfm`/`gfm_ex`/`yozora`）。
- `YOZORA_ASSERT_LEVEL`：指定断言等级（`L0`/`L1`/`L2`/`L3`）。

#### 9.4.5 CI 分级策略

- PR（快速门禁）：
  - 运行 `L1` 的 smoke 子集 + 改动相关 fixture 子集。
- Main 分支（完整门禁）：
  - 运行全部 parser profile 的全量 `L1`。
- Nightly（深度回归）：
  - 运行全量 `L2` + 慢场景性能统计。

#### 9.4.6 失败报告规范

失败输出必须包含：

- `fixture_path`
- `case_id`
- `parser_profile`
- `assert_level`
- `node_path`（如 `root.children[0].children[1]`）
- `expected_excerpt` / `actual_excerpt`

目标：任何单个失败都可在 1 分钟内定位到具体输入与 AST 路径。

#### 9.4.7 兼容性演进策略

- 允许在早期里程碑以 `L1` 先达成结构对齐。
- `L2` 不得无限期后置，最晚在 `M4` 进入主线门禁。
- 当上游新增 fixture 时，默认纳入 `fixtures/` 并参与下一轮全量回归。

### 9.5 Conformance Contract v2（补充约束）

#### 9.5.1 `fixture -> parser profile` 路由矩阵

- 通过 `fixtures/profile-map.toml` 维护唯一映射规则，测试运行时必须加载该文件。
- 若某 fixture 未命中任何 profile 映射，测试直接失败（禁止静默跳过）。
- 首版映射以对齐上游测试行为为准：

```toml
[gfm]
include = ["gfm/**/*.json"]
exclude = [
  "gfm/autolink-extension/**/*",
  "gfm/delete/**/*",
  "gfm/list-item/task list items\\(extension\\)/**/*",
  "gfm/table/**/*",
]

[gfm_ex]
include = ["gfm/**/*.json"]
exclude = ["gfm/**/#616.json", "gfm/**/#619.json", "gfm/**/#620.json"]

[yozora]
include = ["custom/**/*.json", "gfm/**/*.json"]
exclude = [
  "custom/inline-math/backtick-required/**/*",
  "gfm/**/#616.json",
  "gfm/**/#619.json",
  "gfm/**/#620.json",
]

[yozora_inline_math_backtick_required]
include = ["custom/inline-math/backtick-required/**/*.json"]
exclude = []
```

#### 9.5.2 Known Failures 协议

- 通过 `fixtures/known-failures.toml` 管理临时失败集合。
- 每条记录必须包含：
  - `case_id`
  - `parser_profile`
  - `assert_level`
  - `reason`
  - `issue`
  - `introduced_in`
  - `expires_in_milestone`
- CI 约束：
  - PR 不允许新增无 `reason/issue/expires_in_milestone` 的记录。
  - Main 上实际失败集合必须是 known-failures 的子集。
  - Nightly 若发现过期记录仍未清理则直接失败。

#### 9.5.3 AST Comparator 规范

- 比较器必须先做 canonicalization，再做深比较。
- canonicalization 规则：
  - 对象键按字典序输出。
  - `L1` 忽略 `position` 字段；`L2` 保留并严格比较。
  - 数组顺序严格保留（不得排序）。
  - 禁止额外字段（除显式忽略字段外）。
- diff 输出规则：
  - 首个不一致路径（`node_path`）必须明确。
  - 同时输出 `expected_excerpt` 与 `actual_excerpt`。
  - 输出中必须带 `case_id` 与 `parser_profile`。

#### 9.5.4 进入实现前的门槛

- 在 `M0` 结束前，`profile-map.toml` 与 `known-failures.toml` 模板必须落地。
- 在 `M2` 结束前，Comparator 的 `L1`/`L2` 行为必须有单测覆盖。
- 未满足上述门槛前，不进入批量 tokenizer 迁移。


## 10. 风险清单（触发/证据/影响）

### 风险 1: UTF-8 索引与 TS UTF-16 索引差异

- 触发：包含 emoji、CJK 扩展字符、组合字符的输入
- 证据：Rust 常用 `char_indices` 与 TS `string index` 语义不同
- 影响：`position` 和 delimiter 边界可能偏移，导致 fixture 不一致

缓解：`character` 首期即镜像上游索引/位置规则，并用跨语言 fixture 对齐测试锁定行为。

### 风险 2: Inline 递归处理性能退化

- 触发：大量嵌套 emphasis/link 的长文本
- 证据：递归分组 + token 切片会产生频繁分配
- 影响：吞吐下降，测试超时

缓解：首期先保证正确性，后续通过 `SmallVec`、切片复用、arena 进行局部优化。

### 风险 3: rollback 语义遗漏

- 触发：table、list、blockquote 等中断式 block 场景
- 证据：无 rollback 时常见“误匹配后无法恢复”
- 影响：AST 结构级错误，且会级联影响后续节点

缓解：在 M1 即实现 rollback 主链路，并用 fixture 场景锁定回归。


## 11. 首批执行顺序（建议）

1. `M0` + `M1`（先完成 `ast/character`）
2. `M2` 打通 `DefaultParser + fallback`
3. tokenizer 先迁移 `paragraph/text/inline-code/emphasis`
4. 再迁移 `heading/thematic-break/list/blockquote`
5. 最后批量补齐其余 `crates/tokenizer-*`
6. `M6` 补齐 `ast-util` 完整能力


## 12. Definition of Done（当前阶段）

- `docs/impl/plan.md` 设计冻结
- workspace 中 `ast/character/ast-util/parser/crates/tokenizer-*` 相关 crate 结构落地
- `ast` / `character` 已完成一比一镜像并可被内核依赖
- `ast-util` 骨架与入口已落地（完整实现可后置）
- `DefaultParser` 可运行并支持可插拔 API
- `paragraph + text` fallback 能稳定工作
- 已建立 fixture 回归脚手架并可执行
- `Fixture Protocol v1` 已落地并被测试 harness 执行
- `Conformance Contract v2` 已落地（路由矩阵、known-failures、comparator）


## 13. 当前进度快照（2026-03-17）

- `M0`：已完成（workspace/crate 骨架、fixture 协议模板已落地）。
- `M1`：主体已完成（`ast`/`character` 已可支撑内核编译）。
  - 说明：`character` 内 `entity_reference`、`fold_case` 仍为简化实现，后续在 `M3+` 按上游继续补齐。
- `M2`：已完成（最小闭环 + comparator 门槛已打通）。
  - 已完成：
    - `DefaultParser` 支持 `use/replace/unmount/use_fallback/set_default_parse_options/parse`。
    - `paragraph` + `text` fallback 已接入 `parser`/`parser-gfm`/`parser-gfm-ex`。
    - 已补 `core-parser` 行为单测，覆盖 fallback 闭环与可插拔 API 生命周期。
    - `test-util` 已落地 fixture comparator `L1/L2`、profile-map/known-failures 解析与 case 级 diff 单测。
- `M3`：进行中（结构镜像已完成，行为迁移进行中）。
  - 已完成：
    - `crates/tokenizer-*` 已补齐到与上游同数量（31 个 crate）。
    - 非 fallback tokenizer 已统一为可注册 stub（实现 `Tokenizer + BlockTokenizer/InlineTokenizer`）。
    - `parser`/`parser-gfm`/`parser-gfm-ex` 已按上游依赖清单挂载内置 tokenizer 列表。
    - 已完成真实语义迁移（当前批次）：`heading`、`thematic-break`、`list`、`setext-heading`、`blockquote`、`indented-code`、`fenced-code`、`html-block`、`math`、`table`、`admonition`、`ecma-import`、`footnote-definition`、`inline-code`、`inline-math`、`emphasis`、`delete`、`break`、`link`、`image`、`autolink`、`autolink-extension`、`html-inline`、`definition`、`link-reference`、`image-reference`、`footnote`、`footnote-reference`。
    - `core-parser` 已升级为“多行 block tokenizer + inline tokenizer + fallback 混合流水线”。
    - `core-tokenizer` 已支持 block tokenizer 一次消费多行（`consumed_lines`），用于 fenced/setext/blockquote 场景。
    - `core-parser` 已支持 blockquote 内层 block 递归解析（heading/paragraph/code 等）。
    - 已修正缩进边界冲突：`heading/thematic-break/list` 不再误吞 `indented-code` 场景。
  - 待完成：
    - `fenced-block` 仍为占位实现（当前不在 parser 预设装配路径）。
- `M4`：进行中（fixture runner 骨架已可执行）。
  - 已完成：
    - `fixtures/local/smoke/minimal.json` 落地。
    - `test-util` 集成测试可加载 `profile-map.toml` + `known-failures.toml`，并按 profile/L1|L2 执行 smoke case。
    - 已接入上游 fixture 子集回归（`gfm` 的 `heading/inline-code/emphasis/thematic-break/list/setext-heading/blockquote/indented-code/fenced-code/link/image/autolink/autolink-extension/html-inline/break/delete/definition/link-reference/image-reference/table/html-block` 代表样例）。
    - 已新增 `custom` 子集回归（`admonition/ecma-import/inline-math/math/footnote/footnote-definition/table` 代表样例）。
    - 已新增 fixture 批量 runner（`#[ignore]` 手动触发），支持 profile 路由、known-failures 豁免与差异汇总输出。
    - 最新一次 `yozora + L1` 批跑观测：`total_cases=731`，`unexpected=426`（用于后续逐批压降）。
  - 待完成：
    - 扩充 known-failures 基线并逐批压降全量批跑 diff 到可控范围。
- `M6`：进行中（从纯骨架升级为最小可用）。
  - 已完成：
    - `collect_root_nodes/collect_nodes` 深度遍历能力。
    - `mutate_root/mutate_node` 深度变更能力。
    - `mutate_root_async/mutate_node_async` 异步入口（当前为同步遍历封装）。
    - `ast-util` 自测已覆盖 collect/mutate 基本行为。
  - 待完成：
    - 与上游 `collect/mutate/mutate-async` API 细节完全对齐。
