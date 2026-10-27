# 测试设计: archi-graph-doc-agent

> **变更**: archi-graph-doc-agent
> **依据**: `proposal.md`（AC-1…AC-22、范围裁决 D1–D10）、`design.md`（D1–D23、「变更清单」新增 / 修改文件子表、「公共函数 / API」、「类型定义」、「数据模型」）、`specs/`（11 份 delta）
> **范围**: design「变更清单」的**新增源码 21 条**与**修改文件 26 条**。`Cargo.toml` ×3、生成绑定 `bindings.ts`、`package.json` 版本行为无单测路径（见「不可测试项」）；`sdk/mod.rs` 为纯模块挂载行（同见「不可测试项」）。删除文件不纳入。
> **路径口径**: 全部路径为**项目根相对**（`packages/desktop/...`）。design.md 中 `crates/…` / `src/…` 简写对应 `packages/desktop/src-tauri/crates/…` 与 `packages/desktop/src-tauri/src/…`。
> 「验收范围」表的「被测文件或模块」列中 `…/` 前缀 = `packages/desktop/src-tauri/`（前端文件仍写全 `packages/desktop/src/…`，避免与本表新增文件口径混淆）。

## 测试框架与既有布局

- **Rust**：`cargo test --workspace`（每 crate 一次），断言仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，无第三方断言库。
- **测试文件布局**：sidecar 文件与源文件**同目录同前缀** `*_test.rs`，源文件尾经 `#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;` 挂载（`crates/core/config/src/lib_test.rs`、`crates/infra/agent/src/sdk/policy_test.rs` 先例）；crate 级集成测试在 `tests/`（`crates/core/workflow/tests/corpus_golden_test.rs` 先例：`corpus/<case>/` 语料 + pretty JSON golden + `DESKTOP_GOLDEN_REWRITE=1` 显式重写）。
- **既有装置（沿用，零新范式）**：`tempfile::TempDir` 真实临时目录（`config` / `commands/changes` 先例）；`tauri::test::mock_app()` + 真实 `WorkspaceStores`（temp 数据根）托管 `State` 直调（`commands/changes/mod_test.rs` 先例）；orchestration 侧可编程假端口 + 调用序录制（`FakeStore` / `FakeVcs` / `FakeWorker`，`steps_test` / `walker_test` 先例）；`#[cfg(windows)]` / `#[cfg(unix)]` 门控承载平台面。
- **前端**：`vite-plus` 提供 `vp test`（vitest 4，导入面 `vite-plus/test`；全局 jsdom + `src/test-setup.ts`；`include = ["src/**/*.test.ts?(x)"]`；`testTimeout = 20000`）。组件断言用 `@testing-library/react` + `data-testid`（禁样式类名）；进程边界 mock 恒为 `vi.mock('@tauri-apps/api/core')` 的 `invoke`；纯函数测试零渲染（`src/views/changes/flow/run-state.test.ts` / `graph.test.ts` 先例）；xyflow 度量垫片（`ResizeObserver` / `offsetWidth` / `DOMMatrixReadOnly` / `getBBox`）照抄 `change-flow-graph.test.tsx`。
- **命名**：Rust 测试函数为中文短语 + 断言消息载判据；前端 `describe` / `it` 中文短语 + testid 定位。
- **语料与重写**：`packages/desktop/src-tauri/crates/core/archi/tests/corpus/dsl/<case>/*.toml`；`DESKTOP_GOLDEN_REWRITE=1` 为显式重写开关（沿既有惯例，非新条款）。

### 待测功能条目来源约定

`#### 待测功能` 只登记 design.md 明写的命名单元，条目末尾标注出处：`（公共函数 / API）` = design「公共函数 / API」子表行（含签名）；`（类型定义）` = design「类型定义」子表行；`（变更清单 ①②… / D<n>）` = design「修改文件」单元格或「关键设计决策」显式命名者。**未在 design 中命名者不入表**（不 grep 导出声明、不虚构条目）。

源文件在 design「公共函数 / API」子表**无行**时，该章节保留 `#### 待测功能` 标题 + 「design 未声明公共 API，本节无条目」占位（后附 HTML 注释说明原因），不省略该子章节；测试对象来源写在紧随的 `> 说明` 行（类型定义 / 变更清单 / spec 场景 / AC），仍不由源码导出声明反推。

### 参数类型 → 边界覆盖口径

| 类型 | 覆盖要求 | 本变更加载点（按 API 入参 / 字段） |
|---|---|---|
| int / number | `0`、`-1`、`MAX`、`None` + 1 正常 | `Diagnostic.line: u32`（`0` = 位置未知，D5）、`column: Option<u32>`、`modelLine`、`source_ref` 末段 `attempt` 的 `u32` 解析（D11）、`line_of` / `column_of` 字节偏移 |
| str / string | `""`、超长（>1000 chars）、特殊字符（`\n` / `\0` / emoji）、`None` + 1 正常 | `fqn` / `element` / `query` / `scope` / `path` / `source_ref` / `metadata` 值 / `worktree` / 前端 `shortName(fqn)` |
| bool | `true`、`false`、`None` | `Archi { module, search }` 两布尔位（4 组合）、`completed` / `dirty` 无关面不纳入 |
| list / array | `[]`、单元素、超大（数百）、`None` + 1 正常 | `elements` / `relations` / `docs` / `path` / `diagnostics` / `rebase_conflicts` / `warnings` |
| dict / object | `{}`、缺失必填字段、多余字段、`None` + 1 正常 | `metadata`（`{}` / 未知键 / 非字符串值）、`RawElement` 字段面（缺 `kind` / `gatte`） |
| Optional[T] | `None` | `summary` / `description` / `column` / `reason` / `worktree`（`None` 回落主仓根）/ 前端 `root: string \| null` |
| Enum | 每枚举值 + 非法值（N+1） | `ElementKind`(4) / `GateTier`(3) / `DiagnosticKind`(9) / `WarningKind`(1) / `DocKind`(2) / `DocSource`(3) / `EdgeMark`(4) / `NodeMark`(2) / `SearchScope`(3) / `SessionToolset`(2) |
| float | `0.0`、`-0.0`、`NaN`、`Inf`、`None` | **不适用**：本变更无 float 入参 / 字段，显式声明不覆盖（不虚构 float 面） |

---

## 验收范围

| AC ID | 验收条件（缩写，原文见 proposal） | 被测文件或模块 |
|---|---|---|
| AC-1 | 多文件字典序聚合后 `modelFile` / `modelLine` 指向表头行列、可直接 `edit`；关系行号取 `from` 行 | `…/crates/core/archi/src/parse/mod.rs`、`…/crates/core/archi/src/parse/toml.rs`、`…/crates/core/archi/src/model.rs`、`…/src/commands/archi/mod.rs`、`…/crates/infra/agent/src/sdk/archi_tool.rs` |
| AC-2 | 九类结构诊断齐备：免费五类保留 parser 原文与 `line` / `column`，自实现四类各带 `file` + `line`；一次列全不短路、不 panic | `…/crates/core/archi/src/parse/toml.rs`、`…/crates/core/archi/src/parse/mod.rs`、`…/crates/core/archi/src/graph.rs`、`…/crates/core/archi/src/dto.rs` |
| AC-3 | 跨文件重复 FQN → `duplicateElement`，`file` / `line` 指字典序靠后者，不静默保留任一份 | `…/crates/core/archi/src/parse/mod.rs`、`…/crates/core/archi/src/dto.rs` |
| AC-4 | 停摆全或无：任一结构错时 `elements` / `relations` 皆空 + 诊断清单；无结构错返回完整图 | `…/crates/core/archi/src/dto.rs`、`…/src/commands/archi/mod.rs`、`packages/desktop/src/views/architecture/components/architecture-canvas.tsx`、`packages/desktop/src/views/architecture/components/structure-error-panel.tsx` |
| AC-5 | 文档三源强度排序 + 同文件双源取强 + `docNotFound` 只报不拦 + 无关联空数组不告警 | `…/crates/core/archi/src/docs.rs` |
| AC-6 | 关联结果不含正文；`archi_read_doc` 只接受关联集内路径，越界显式 `Err` | `…/crates/core/archi/src/docs.rs`、`…/crates/core/archi/src/dto.rs`、`…/src/commands/archi/mod.rs`、`packages/desktop/src/views/architecture/components/element-drawer.tsx` |
| AC-7 | 三面（进程内诊断集 / 工具串 / IPC DTO）词汇集相等；新增词汇未同步即红 | `…/crates/core/archi/src/lib.rs`、`…/crates/core/archi/src/dto.rs`、`…/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src/views/architecture/lib/marks.ts` |
| AC-8 | `archi` workspace 内依赖仅 `foundation`；`workflow` ⇄ `archi` 互不依赖；零 Tauri / store / spawn；两条新边有用途注释 | `packages/desktop/src-tauri/crates/core/archi/Cargo.toml`、`packages/desktop/src-tauri/Cargo.toml`、`packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`（审查项，见「不可测试项」） |
| AC-9 | 路径字面量单点：`architecture` / `openspec` / `config.json` 字面量零泄漏；四函数全经 `foundation::layout` | `…/crates/core/foundation/src/layout/mod.rs`、`…/crates/core/orchestration/src/walker.rs` |
| AC-10 | `archi_module` 两形态：指定元素返回邻接 / 子元素 / 文档 / `modelFile` + `modelLine`；省略返回薄概览并受 30KB 上限截断留痕 | `…/crates/core/archi/src/dto.rs`、`…/crates/infra/agent/src/sdk/archi_tool.rs` |
| AC-11 | `archi_search` 语义：子串 + 驼峰分词、不搜正文、不碰 `specs`、scope 收窄、空 query `Err` | `…/crates/core/archi/src/search.rs`、`…/crates/core/archi/src/dto.rs`、`…/crates/infra/agent/src/sdk/archi_tool.rs` |
| AC-12 | 注入矩阵 7 行逐行命中 + 矩阵外 / 畸形 `source_ref` 恒零 archi 工具 + explore 臂逐字不变 + `RunStarted.tools` 与实际 definitions 一致 | `…/crates/infra/agent/src/compose.rs`、`…/crates/infra/agent/src/sdk/tools.rs`、`…/crates/infra/agent/src/sdk/runner.rs`、`…/crates/infra/agent/src/sdk/mod.rs`、`…/crates/core/agent/src/runner.rs` |
| AC-13 | 矩阵外相位调用 → `tool_unavailable` + `is_error` 回灌、run 不中断；`default` 档两工具均放行 | `…/crates/infra/agent/src/sdk/loop.rs`、`…/crates/infra/agent/src/sdk/policy.rs` |
| AC-14 | 模型 = 源码：worktree 内写盘成功；坏模型读取返回结构错且不 panic；内置锁面不含架构域路径 | `…/src/commands/archi/mod.rs`、`…/crates/infra/agent/src/sdk/archi_tool.rs`（内置锁面为**零改动面**回归锚，见「不可测试项」） |
| AC-15 | 归档确认面「架构变更」区块：有改动逐项呈现 + diff / 预览入口；无改动零占位；结构错红字不阻断 | `…/crates/core/archi/src/diff.rs`、`…/crates/core/archi/src/dto.rs`、`…/src/commands/archi/mod.rs`、`packages/desktop/src/views/changes/flow/run-control-panel.tsx` |
| AC-16 | 模型冲突 → `Terminal::failed`（worktree 绝对路径 + 引导文案 ≤200 字）、零会话、零 `rebase --abort`；纯代码路径逐字不变 | `…/crates/core/orchestration/src/walker.rs`、`…/crates/core/foundation/src/layout/mod.rs` |
| AC-17 | `Preflight` 检出 `rebase_head` 在场 → `Terminal`（含 worktree 绝对路径 + 两条出路）；不在场放行 | `…/crates/core/orchestration/src/steps.rs`、`…/crates/core/orchestration/src/walker.rs` |
| AC-18 | `/architecture` 全量只读 + change 详情「架构」页签增量可改档（`root = detail.worktree`、归档回落主仓根）；缺省 1-hop；三触发取数零 watch；侧栏「架构」项 active 由 URL 派生；同一元素在元素树与画布节点呈现**同一短名**（`shortName` 单点） | `packages/desktop/src/routes.tsx`、`packages/desktop/src/components/app-sidebar.tsx`、`packages/desktop/src/views/architecture/architecture-view.tsx`、`packages/desktop/src/views/architecture/components/architecture-canvas.tsx`、`packages/desktop/src/views/architecture/components/element-tree.tsx`、`packages/desktop/src/views/architecture/components/element-drawer.tsx`、`packages/desktop/src/views/architecture/lib/fqn.ts`、`packages/desktop/src/views/architecture/hooks/use-archi-graph.ts`、`packages/desktop/src/views/architecture/hooks/use-archi-module.ts`、`packages/desktop/src/views/changes/change-detail-view.tsx`、`packages/desktop/src/app.test.tsx` |
| AC-19 | `archi_set_gate` 写前校验（元素存在 / 值合法 / 渲染自证）+ 仅改目标行保留注释空行 + 任一失败零写入 | `…/crates/core/archi/src/edit.rs`、`…/src/commands/archi/mod.rs`、`packages/desktop/src/views/architecture/components/element-drawer.tsx` |
| AC-20 | 交付面：`bindings:check` 零 diff（五命令 + `Archi*` 类型 + 词汇枚举在案；`SessionToolset` 属引擎侧词汇**不出线**，见 bindings 节反向锚定）；`version` 0.6.0；`vp check` / `knip` / `clippy` 通过 | `packages/desktop/src-tauri/src/commands/mod.rs`、`packages/desktop/src-tauri/src/bindings/mod_test.rs`、`packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/package.json`（后两项为机械门面，见「不可测试项」） |
| AC-21 | 7 份矩阵命中 md 各含其勾选工具名 + 「何时用 / 返回什么」；其余 9 份零 archi 字样；16 份零占位 / 零 `__MCP:*`；`include_str!` 与角色表逐字不变 | `…/crates/core/workflow/src/prompts/`（7 份命中 md） |
| AC-22 | `marks.ts` 边四值 / 节点两值确定性样式，词汇外退化中性不抛错；本期恒 `none`；与三方对拍语料逐值一致 | `packages/desktop/src/views/architecture/lib/marks.ts`、`…/crates/core/archi/src/lib.rs` |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/lib.rs -> packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（lib.rs 只做八模块装配 + `pub use`），故本节无「待测功能」条目。 -->

> 说明：测试对象取自 design「类型定义」子表的封闭词汇行、「变更清单 新增文件」`lib.rs` 行（八模块装配 + 关键类型 `pub use`）与 D4 / D9 / D10 / AC-7。`lib.rs` 的 `#[cfg(test)] mod *_test;` 声明本身由其余各节编译可达性承载（任一缺失即编译失败）。
> 依赖方向约束：`archi` 不得依赖 `infra/agent`，故「工具串」一面在本 crate 内以 `archi::dto` 投影串（工具执行体的同一投影源）代表，工具执行体侧的词汇集等价在 `…/sdk/archi_tool.rs` 章节补一条对拍。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 三面词汇集（进程内诊断 / DTO 投影串 / 类型词表） | 正向 | 全词汇语料（`kind` ×4 元素、`gate` ×3 档、九类结构诊断各一坏点、`docNotFound` 一例、`EdgeMark` ×4、`NodeMark` ×2）驱动：① `ModelGraph::diagnostics()` 的 `DiagnosticKind` 集 ② 同语料 `dto::module_detail` / `dto::graph_view` 的 `serde_json` 串中出现的词汇字面量集 ③ 枚举全值集（`ElementKind` / `GateTier` / `DiagnosticKind` / `WarningKind` / `DocKind` / `DocSource` / `EdgeMark` / `NodeMark` / `SearchScope`）——三面相等，失败时点名缺失词汇 | 表驱动（逐词汇一行断言） |
| 封闭词汇值域 | 边界 | 每个枚举的全值集合恰为：`ElementKind` 4 / `GateTier` 3 / `DiagnosticKind` 9 / `WarningKind` 1 / `DocKind` 2 / `DocSource` 3 / `EdgeMark` 4 / `NodeMark` 2 / `SearchScope` 3；多一值或少一值即红（`NodeMark` **不含**结构错值，D6） | 表驱动（9 组） |
| 新增词汇未同步 | 异常 | 语料只含 8 类诊断而 `DiagnosticKind` 有 9 值（或反之）→ 集合不等即红；模拟"新增 kind 而语料未加坏点"时错误消息列出缺失词汇 | 单例 |
| 三面同源计数 | 正向 | 同一语料下 `dto::module_detail` 的元素字段与 `graph.depends_on` / `depended_by` / `children` 条数逐一相同；`modelFile` / `modelLine` 与 `Model::element` 取到的字段逐字相同（单点构建 + 投影零改写） | 单例 |
| 投影零改写（显式空值） | 边界 | 元素无 `path` / `docs` / `summary` / `metadata` 时，DTO 串含显式 `[]` / `null` / `{}`（字段不省略、不填占位串），三面形状一致 | 表驱动（字段 ×4） |
| 空模型与停摆态可区分 | 边界 | 空模型（零元素零关系零诊断）→ 三个数组皆空且 `diagnostics` 空；含一个坏点 → 三个数组皆空但 `diagnostics` 非空（判据 = `diagnostics` 非空，D9，无布尔位） | 表驱动（两形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型语料（文件系统） | **不 mock**：`tempfile::TempDir` 真实落 `…/models/*.toml`（路径经 `foundation::layout::archi_models_dir` 派生），真实 `parse_dir` | 全词汇语料 / 空模型 / 坏点 |
| 解析、图、投影 | **不 mock**：真实组合（`parse_dir → ModelGraph::build → dto::*`）——三面同源正是被测语义，任何一层替身都会抹掉判据 | 全部用例 |
| 工具执行体 | **不在本 crate mock / 不引入依赖**：`archi` 禁依赖 `infra/agent`（AC-8）；工具串一面等价性由 `archi_tool_test` 承担 | —— |
| specta 词表 | **不 mock**：以枚举全值集常量（测试内字面量清单）对照类型，零 IPC runtime、零 bindings 产物读取 | 值域 / 新增词汇用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/model.rs -> packages/desktop/src-tauri/crates/core/archi/src/model_test.rs

#### 待测功能

- `Model::element(fqn)`: 按 FQN 取元素引用（`elements` 线性扫描）；`dto` 投影、`edit::plan_set_gate` 的「元素 → 声明文件」定位与 `search` 薄条目组装共用，禁第二份查找实现（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Model::element` | 正向 | 三元素模型（`App` / `App.Auth` / `App.Auth.Token`）查 `App.Auth` → `Some`，`fqn` / `kind` / `gate` / `path` / `docs` / `summary` / `metadata` / `file` / `line` 逐字段等于构造值 | 单例 |
| `Model::element` | 正向 | 命中与声明序无关：首个 / 中间 / 末个元素均命中；`metadata` 多键与 `path` 多元素逐项保留 | 表驱动（3 位置） |
| `Model::element` | 异常 | 空模型（`elements: vec![]`）任意 FQN → `None`，不 panic | 单例 |
| `Model::element` | 边界 | FQN 精确匹配：`""` / `"App"`（前缀） / `"App.Auth.Token.Extra"`（超长路径）→ `None`；大小写不同 `"app.auth"` → `None` | 表驱动（4 形态） |
| `Model::element` | 边界 | `str` 边界：超长（>1000 chars） / 含 `\n` / 含 `\0` / emoji FQN → 均 `None` 且不 panic | 表驱动（4 形态） |
| `Model::element` | 边界 | 500 元素模型查末个元素 → `Some`（线性扫描不因规模改变语义） | 单例 |
| `Model` / `Element` / `Relation` 派生语义 | 边界 | `PartialEq` / `Eq` 派生：同内容两 `Model` 相等；仅 `file` 或 `line` 不同即不等；`metadata: {}` 与缺省同值相等；`Relation.span == None` 占位可比较 | 表驱动（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 全部分支 | **零 mock**：纯数据结构字面量直构，无 IO / 无 async / 无环境依赖 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/parse/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/mod_test.rs

#### 待测功能

- `SourceParser::parse(source)`: 单文件解析契约（关联函数、无接收者）；`file` 字段留空由聚合层回填（公共函数 / API）
- `parse_dir(root)`: 聚合入口——`archi_models_dir(root)` 下 `*.toml` 按**文件名**字典序逐个解析 → 合并（声明序保留）→ 跨文件重复 FQN 检出（`file` / `line` 指字典序靠后者）→ `file` 回填 repo 相对（`/` 分隔）；目录缺席 / 空目录 → 空模型零诊断（公共函数 / API）

> 文件分工：单元用例落 sidecar `crates/core/archi/src/parse/mod_test.rs`；语料 golden 三条（跨模块组合，入口 = `parse_dir`）落 crate 级集成测试 `crates/core/archi/tests/golden_test.rs`（沿既有 `DESKTOP_GOLDEN_REWRITE=1` 惯例，不新建独立集成章节）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `parse_dir` | 正向 | `01-core.toml` + `02-auth.toml`（后者引用前者元素）→ 两文件元素与关系齐全；合并序 = 文件名字典序 × 文件内声明序（`01` 全在 `02` 前）；零诊断 | 单例 |
| `parse_dir` | 正向 | `file` 回填为 repo 相对路径且 `/` 分隔（`openspec/architecture/models/01-core.toml`）；`line` 逐元素等于表头键行（与文件原文行号一致） | 单例 |
| `parse_dir` | 正向 | 文件名非数字序（`a.toml` / `B.toml` / `10.toml` / `2.toml`）→ 聚合序按字节字典序（`10` < `2`），且各文件 `file` 指向自身 | 表驱动（4 文件名） |
| `parse_dir` | 异常 | 跨文件重复 FQN：`01-core.toml` 与 `03-billing.toml` 均声明 `App.Auth` → 一条 `duplicateElement`，`file` = `03-billing.toml`、`line` = 该文件声明行；模型不静默保留任一份（两处声明均不入 `elements` 或仅保留前者均由诊断停摆兜住，断言"诊断在场且元素数不因重复而增"） | 单例 |
| `parse_dir` | 异常 | 跨文件同一 FQN 三处声明（`01` / `02` / `03`）→ 诊断恰 2 条（后两者各一条），`file` 分别指向 `02` / `03` | 表驱动 |
| `parse_dir` | 异常 | 读盘失败（以同名**目录**占据 `02-auth.toml`，或 `cfg(unix)` 置 `0o000`）→ 一条 `syntaxError`，`message` 前缀「模型文件读取失败」，`file` 回填；其余文件照常解析（单次列全、零 panic，D18） | cfg 门控（unix）+ 单例 |
| `parse_dir` | 异常 | 单文件多坏点（缺 `kind` + 非法 `gate`）→ 两诊断同时在场（不短路）；跨文件另一坏点亦在场（总数对得上坏点数） | 表驱动 |
| `parse_dir` | 边界 | `models/` 目录**不存在** → 空模型（零元素零关系零诊断），不 `Err` | 单例 |
| `parse_dir` | 边界 | `models/` 为**空目录** → 空模型；仅含空文件（`""`）与仅含注释文件 → 空模型零诊断 | 表驱动（3 形态） |
| `parse_dir` | 边界 | 非 `*.toml` 参与面：`README.md` / `notes.txt` / `x.toml.bak` / 子目录内 `nested/y.toml` 均不参与聚合（元素数只由顶层 `*.toml` 决定） | 表驱动（4 形态） |
| `parse_dir` | 边界 | 规模面：100 个 `*.toml` × 各 5 元素 / 单文件 500 元素 / 单文件 500 关系 → 聚合结果计数正确、`line` 正确、零 panic | 表驱动（3 规模） |
| `parse_dir` | 边界 | 文件名含中文 / emoji / 空格 → 不 panic；`file` 逐字保留（含 UTF-8），判定序仍为字节字典序 | 表驱动 |
| `SourceParser::parse` | 正向 | 直接以单文件源文本调用（`TomlParser`）→ `ParseOutcome.file` 为空串（聚合层回填前契约），`line` 相对该源文本 | 单例 |
| `SourceParser::parse` | 边界 | 空字符串 `""` → 空模型零诊断；超长单文件（>100 KB）→ 不 panic、诊断数与坏点数一致 | 表驱动 |
| 语料 golden 全链（跨模块组合，入口 = `parse_dir`） | 正向 | `tests/corpus/dsl/<case>/*.toml` 逐 case 驱动 `parse_dir`（→ 后续 `ModelGraph::build` / `dto` 投影面在各自章节断言）→ `ParseOutcome` 序列化为 pretty JSON 与入仓 golden 逐字节对比；case 面覆盖 proposal 列出的 13 形态（注释 / 多文件字典序 / 数组值 / `gate` + `docs` + `metadata` / 空模型 / 缺 `kind` / 非法 `kind` / 非法 `gate` / 拼错键 / 漏引号 / 漏 `]` / 同文件重复 key / **跨文件重复 FQN**） | 表驱动（13 case） |
| 语料 golden（重写开关） | 边界 | `DESKTOP_GOLDEN_REWRITE=1` 时覆写入仓 golden；随后不带该环境变量复核 → 与现 golden 等价（零 diff）；未重写即漂移 → 必红 | 单例 |
| 语料 golden（结构性 case 必在） | 异常 | `dup-cross-file` 与 `orphan` 两 case 必在语料目录内（缺失即红——explore 低估两类，R-2 的机械锚）；case 目录名与期望诊断 kind 的映射表在测试内单点声明，新增 case 未登记即红 | 表驱动（2 case + 登记面） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `models/` 目录与文件 | **不 mock**：`tempfile::TempDir` + 真实落盘（含同名目录占据、`cfg(unix)` 权限）——回填、字典序、读盘失败均为真实文件系统语义 | 全部用例 |
| TOML 解析器（`TomlParser`） | **不 mock**：真实 `toml` crate（parser 原文与 span 是被测契约，D2 / D6） | 全部用例 |
| `foundation::layout::archi_models_dir` | **不 mock**：真实派生（路径单点本身由 foundation 章节覆盖） | 全部用例 |
| `gitignore` / 环境变量 / 时钟 | **零依赖**：本模块不触达 | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs

#### 待测功能

- `TomlParser`: `RawDocument` 反序列化（`deny_unknown_fields`）→ span 提取 → 诊断（D6 按 `err.message()` 前缀归类）（公共函数 / API）
- `line_of(source, byte)`: 字节偏移 → 行号（1 起始）（公共函数 / API）
- `column_of(source, byte)`: 字节偏移 → 列（1 起始，**char** 计）（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `TomlParser` | 正向 | proposal 示例全字段（`kind` 四值各一元素、`gate` 三档、`path` 两项、`docs` 一项、`summary`、`metadata` 内联两键、`[[relation]]` 带 `desc`）→ 元素 / 关系逐字段等于期望；省略 `gate` 的元素取 `Off`；缺省 `path` / `docs` 为空数组、`metadata` 为空表 | 单例 |
| `TomlParser` | 正向 | span 精确性（AC-1）：四元素 `line` 逐行等于 `[element."…"]` 所在行；`path` 每项与 `docs` 每项的行号在解析期被消费（元素级 `line` 仍为表头行）；`[[relation]]` 的 `line` 等于其 `from` 键行 | 表驱动（元素 ×4 + 关系 ×2） |
| `TomlParser` | 正向 | 空字符串 `""` / 仅注释 / 仅空行 → 空模型零诊断 | 表驱动（3 形态） |
| `TomlParser` | 异常 | `syntaxError`：漏闭合引号（`[element."App]`）与漏闭合 `]`（`[element."App"`）→ `kind = syntaxError`，`message` **原样**含 `TOML parse error at line N, column M` + 源码片段 + `^^^^`，`line` / `column` 取自 parser | 表驱动（2 形态） |
| `TomlParser` | 异常 | `missingField`：缺 `kind` → 一条诊断；若 serde 错误无 span 则 `line == 0` 且 `column == None`（D5「位置未知」可机械判别，不伪造行号） | 单例 |
| `TomlParser` | 异常 | `invalidValue`：`kind = "pakage"` → `message` 列出 `package` / `domain` / `module` / `component` 全部四值；`gate = "stirct"` → 列出 `off` / `warn` / `strict` 全部三值 | 表驱动（2 形态） |
| `TomlParser` | 异常 | `unknownField`：元素表写 `gatte = "strict"` → 拒绝且位置在案；同文件 `metadata = { unknownKey = "x" }` **被接受**（两侧严格性不同且各自可辨） | 单例 |
| `TomlParser` | 异常 | `metadata` 非字符串值（`{ n = 1 }` / `{ flag = true }`）→ `invalidValue`；子表形态 `[element."App.Auth".metadata]` → 不产生 `App.Auth` 的 `metadata`（该表被读成元素 `App.Auth.metadata`，因其缺 `kind` 报 `missingField`） | 表驱动（3 形态） |
| `TomlParser` | 异常 | 未引号点分 `[element.App.Auth]` → `unknownField`（子键 `Auth` 不被 `RawElement` 字段面接受），且**不与** `[element."App.Auth"]` 合并：同时写两形态时合法形态元素仍在、非法形态不新增元素 | 单例 |
| `TomlParser` | 异常 | 同文件重复 `[element."App.Auth"]` → `duplicateElement`（TOML `duplicate key` 归类），零 panic | 单例 |
| `line_of` / `column_of` | 边界 | byte `0` → 行 1 / 列 1；行首 / 行尾 / 换行符字节本身 → 行号按 `\n` 计数、列 1 起始且不 panic | 表驱动（4 位置） |
| `line_of` / `column_of` | 边界 | `byte == source.len()`（EOF）→ 返回末行 / 末列，不 panic 不越界；`source == ""`（byte 0 == len）→ 不 panic | 表驱动（2 形态） |
| `line_of` / `column_of` | 边界 | 多字节面：CJK / emoji 行前后取偏移 → 行号正确、列号按 **char** 计（与 parser caret 同单位，字节数大于字符数）；UTF-8 三 / 四字节字符各一例 | 表驱动（4 形态） |
| `TomlParser` | 边界 | `str` 值边界：`summary` 超长（>1000 chars）逐字保留；含 `\n` 转义 / `\u0000` 转义 / emoji 的 `summary` 与 `metadata` 值逐字保留；`path` / `docs` 各 1 项与各 200 项均正确 | 表驱动（4 形态） |
| `TomlParser` | 边界 | `int` 面：`Diagnostic.line` 为 `u32`；`column` 为 `Option<u32>` 且缺省 `None`；构造无 span 分支断言 `line == 0` 与 `column == None` 并存（不出现 `u32::MAX` 之类哨兵） | 表驱动（2 形态） |
| `TomlParser` | 边界 | 规模面：500 元素 + 500 关系文件 → 诊断 0、逐元素 `line` 正确（防 span 溢出 / 错位） | 单例 |
| 字段类型面 | 边界 | 模型字段面不含 float / 裸 bool（`kind` / `gate` 为枚举，`path` / `docs` 为数组，`metadata` 为字符串表）→ 显式声明不覆盖，不虚构入参形态 | —— |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| TOML 解析（`toml` / `serde_spanned`） | **不 mock**：parser 的原文案 / `line` / `column` / 片段 / caret 是被测契约（AC-2 明写「原样沿用」），替身会抹掉判据 | 全部用例 |
| 文件系统 | **零依赖**：本模块只吃 `&str`，不经盘（`file` 回填由 `parse_dir` 负责） | 全部用例 |
| 时间 / 环境 / 进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/graph.rs -> packages/desktop/src-tauri/crates/core/archi/src/graph_test.rs

#### 待测功能

- `ModelGraph::build(model)`: **唯一**建图点——节点 = 元素 FQN、边 = 关系（`from → to`），持有 `model` 本体（公共函数 / API）
- `ModelGraph::model()`: 模型只读访问（避免 `(model, graph)` 双参数形态）（公共函数 / API）
- `ModelGraph::depends_on(fqn)`: 出边（按 `(from,to)` 字典序）（公共函数 / API）
- `ModelGraph::depended_by(fqn)`: 入边（同上序）（公共函数 / API）
- `ModelGraph::children(fqn)`: 直接子元素（FQN 前缀一级，字典序）（公共函数 / API）
- `ModelGraph::neighborhood(fqn)`: 1-hop 三项组合（公共函数 / API）
- `ModelGraph::diagnostics()`: 四类自实现结构诊断（`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`），一次列全、各带 `file` + `line`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ModelGraph::build` / `model` | 正向 | 3 元素 2 关系的 `Model` → `model()` 逐字段等于入参（声明序保留）；节点数 = 元素数、边数 = 关系数（经邻接查询可观测） | 单例 |
| `ModelGraph::build` | 边界 | 空模型 → `model()` 为空、`children` / 邻域全空、`diagnostics()` 空；只声明一个顶层元素 → 元素表恰一项 | 表驱动（2 形态） |
| `ModelGraph::depends_on` / `depended_by` | 正向 | 出边 / 入边各 3 条 → 按 `(from,to)` 字典序；每条 `description` 与声明一致；无出边 / 无入边 → 空 `Vec` | 表驱动（3 形态） |
| `ModelGraph::children` | 正向 | `App.Auth.Token` 之父 = `App.Auth`（**最长前缀**一级，非 TOML 嵌套）；`App.Auth` 子元素恰 `["App.Auth.Token"]`；`App` 子元素含 `App.Auth` 与 `App.Billing`（字典序） | 表驱动 |
| `ModelGraph::children` | 边界 | 前缀边界：`App.AuthLegacy` **不是** `App.Auth` 的子元素（按 `.` 边界切分，非字符串前缀）；未知 FQN / `""` / 超长 FQN → 空 `Vec`；无顶层元素时层级仍按 `.` 边界一级切分；500 子元素 → 全量返回有序 | 表驱动（5 形态） |
| `ModelGraph::neighborhood` | 正向 | 聚焦 `App.Auth`：三项与 `depends_on` / `depended_by` / `children` 逐一相等（同一数据、零二次遍历）；非邻域元素不出现 | 单例 |
| `ModelGraph::diagnostics` | 正向 | 环 `App.A → App.B → App.A` → 一条 `cycle`，`message` 含环路径（`App.A -> App.B -> App.A` 形态）与声明位置；`dependsOnAncestor`（`App.Auth → App`）→ 指明元素与祖先；`unknownReference`（`to = "App.Shared"` 未声明）→ 指明引用方与缺失目标；`orphanHierarchy`（只声明 `App.Auth.Token`）→ `file` / `line` 指该声明，且**不补建**隐式祖先（`model()` / `children` 不出现 `App` / `App.Auth`） | 表驱动（4 类） |
| `ModelGraph::diagnostics` | 异常 | 三错并存（环 + 未定义引用 + 孤儿）→ 三类同时在场、诊断数 == 坏点数（不首个短路，AC-2） | 单例 |
| `ModelGraph::diagnostics` | 异常 | 自环 `App.A → App.A` → 一条 `cycle`（单点 SCC 落法，D8）；双向 `A ↔ B` → 一条 `cycle`（每组 SCC 一条，非每边一条）；两组独立环（`A↔B` 与 `C→D→C`）→ 恰 2 条 | 表驱动（3 形态） |
| `ModelGraph::diagnostics` | 异常 | 依赖祖先边界：`App.Auth.Token → App`（跨两级）命中；`App.Auth → App.Billing`（兄弟）不命中；`App.Auth → App.AuthX`（名字相近非祖先）不命中；`App.Auth → App.Auth`（自指）归 `cycle` 而非 `dependsOnAncestor` | 表驱动（4 形态） |
| `ModelGraph::diagnostics` | 边界 | `unknownReference` 两端：`to` 未声明（spec 明写）与 `from` 未声明（同一「关系引用不存在的元素」口径）各一条，均不静默建节点；引用方 / 缺失目标均在 `message` 内 | 表驱动（2 形态） |
| `ModelGraph::diagnostics` | 边界 | clean 无环图（5 元素 4 关系、层级完整）→ 零诊断；空模型 → 零诊断；只声明 `App` 而无子元素 → 零诊断 | 表驱动（3 形态） |
| `ModelGraph::diagnostics` | 边界 | 规模面：500 节点链（无环）→ 零诊断且不栈溢出；500 节点成环 → 一条 `cycle` 且 `message` 含环上 FQN 序列（超长消息不 panic） | 表驱动（2 形态） |
| `ModelGraph::diagnostics` | 边界 | `str` 面：FQN 含 emoji / 全中文 / 超长（>1000 chars）→ 诊断消息逐字含该 FQN，不 panic | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型输入 | **不 mock**：`Model` / `Element` / `Relation` 结构体字面量直构，**不经解析**（proposal 测试表明写「直接造 AST」） | 全部用例 |
| `petgraph` | **不 mock**：真实 `DiGraph` + `tarjan_scc`（图算法即被测语义） | 环 / 邻接用例 |
| 文件系统 / 进程 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/docs.rs -> packages/desktop/src-tauri/crates/core/archi/src/docs_test.rs

#### 待测功能

- `element_docs(graph, root, fqn)`: 单元素三源关联（`explicit` > `adjacent` > `text`）+ 同文件合并取强 + 稳定序 + `docNotFound` 警告（不进 `docs`）（公共函数 / API）
- `all_docs(graph, root)`: 全元素并集（去重、稳定序）；`decisions/*.md` 正文只读一次并按 FQN 分发（避免 O(元素×ADR) 次读盘）（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `element_docs` | 正向 | `explicit` 命中：`docs = ["./src/auth/README.md"]` 且文件在盘 → 一条 `{path, title, kind: md, source: explicit}`；`path` 为 repo 相对 `/` 分隔；`title` 取首行 `# 标题` | 单例 |
| `element_docs` | 正向 | `adjacent` 命中：`path = ["./src/auth/"]` 目录内 2 个 `*.md` → 两条 `source: adjacent`；`path` 指向**单个文件**时按精确匹配、不经递归（同目录其他 md 不入） | 表驱动（目录 / 单文件） |
| `element_docs` | 正向 | `text` 命中：`decisions/0007-token-sink.md` 正文含 `App.Auth` → 一条 `kind: adr`、`source: text` | 单例 |
| `element_docs` | 正向 | 三源同时命中 → 次序 `explicit` → `adjacent` → `text`；每条严格只含四字段（无正文 / 摘要 / 行区间） | 单例 |
| `element_docs` | 异常 | `docNotFound`：`docs = ["./src/auth/MISSING.md"]` → `warnings` 一条 `{element: "App.Auth", path: "./src/auth/MISSING.md"}`，`docs` **不含**该条；不产生结构诊断（文档缺失不影响停摆） | 单例 |
| `element_docs` | 异常 | 读盘失败按 `docNotFound` 处置（D18）：`docs` 指向一个**目录**（`./src/auth/`）或 `cfg(unix)` 置不可读 → 一条 warning、`docs` 不含、不 panic | cfg 门控（unix）+ 单例 |
| `element_docs` | 异常 | 邻近扫描排除：`src/auth/__tests__/fixture.md` / `src/auth/tests/x.md` / `src/auth/node_modules/a.md` / `src/auth/target/a.md` / `src/auth/.git/a.md` / `<archi_dir>/models/notes.md` → 均不出现在 `docs`；`.txt` 不参与（仅 `*.md`） | 表驱动（7 形态） |
| `element_docs` | 异常 | 扫描不越界：元素 `path = ["./src/auth/"]` 而 `src/order/README.md` 在盘 → 该文件不出现（不扫全仓） | 单例 |
| `element_docs` | 边界 | 同文件双源合并取强：`src/auth/README.md` 既在 `docs` 又在 `path` 目录内 → `docs` 恰一条且 `source = explicit` | 单例 |
| `element_docs` | 边界 | 同强度稳定序：三条 `adjacent` → 按 `path` 字典序；连续两次调用结果逐字相同 | 单例 |
| `element_docs` | 边界 | 无任何关联（无 `docs`、`path` 目录无 md、无 ADR 提及）→ `docs: []` **且** `warnings: []`（不告警） | 单例 |
| `element_docs` | 边界 | `title` 推导：首行 `# 标题` → `标题`；无标题正文 → 文件名去扩展名；空文件 → 文件名；首行为代码围栏 → 退化为文件名 | 表驱动（4 形态） |
| `element_docs` | 边界 | 列表边界：`path` / `docs` 为 `[]` / 单元素 / 200 元素 / 同路径重复两项 → 去重后逐条判定、不 panic | 表驱动（4 形态） |
| `element_docs` | 边界 | `str` 面：`fqn` 为 `""` / 超长（>1000）/ 含 emoji → `docs` 空、`warnings` 空、不 panic；路径含中文 / emoji / 空格 → `path` 归一 `/` 并逐字返回 | 表驱动 |
| `all_docs` | 正向 | 3 元素（各带 explicit / adjacent / text 命中）→ 并集按「来源强度 → `path`」稳定序；同路径多元素命中只出现一次（身份键 = `path`） | 单例 |
| `all_docs` | 边界 | 空模型 → 空 `docs` + 空 `warnings`；200 元素 + 3 份 ADR → 结果正确、`decisions/` 正文零重复读入副作用（以"调用前后文件字节不变 + 结果无重复条目"行为化断言，不断言 IO 次数） | 表驱动（2 形态） |
| `all_docs` | 边界 | 无跨元素反查：`DocRef` 四字段不含元素 FQN（spec 明令不提供反向索引） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 元素 `path` / `docs` 覆盖的文件树 | **不 mock**：`tempfile::TempDir` 真实建 `src/auth/…` / `decisions/…` / 排除目录 / `.gitignore`，真实 `ignore::WalkBuilder`（gitignore 与剪枝是被测语义） | 全部用例 |
| 排除谓词 / `foundation::layout` | **不 mock**：真实调用（判定单点由 foundation 章节覆盖） | 排除 / `archi_dir` 相关用例 |
| ADR 正文读盘 | **不 mock**：真实文件内容（正文命中判定即被测语义） | `text` / `all_docs` |
| `ModelGraph` | **不 mock**：真实 `ModelGraph::build`（元素 `path` 来源即图内模型） | 全部用例 |
| 时钟 / 环境 / 进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/search.rs -> packages/desktop/src-tauri/crates/core/archi/src/search_test.rs

#### 待测功能

- `search(graph, root, query, scope)`: 分词 + 全段命中（AND）；空白 `query` → `Err`；`Elements` 时 `docs` 恒空、`Docs` 时 `element_fqns` 恒空（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `search` | 正向 | `query = "auth"`、`scope = All` → 命中 FQN 含 `auth` 的元素（薄条目 `fqn` / `kind` / `gate` / `summary`）+ 文件名含 `auth` 的文档（四字段条目）；元素按 FQN、文档按「来源强度 → `path`」稳定序 | 单例 |
| `search` | 正向 | 驼峰分词：`AuthToken` → 段 `auth` + `token`（AND）→ 命中 `App.Auth.Token`；`auth token`（空格）→ 同结果；`AUTH`（大写）→ 大小写不敏感同结果 | 表驱动（3 形态） |
| `search` | 正向 | 匹配对象面：`summary` 命中、`metadata` **值**命中（键名命中不算）、文档 `path` 段命中、文件名命中各一条 | 表驱动（4 形态） |
| `search` | 异常 | 仅正文命中不算：md 正文含 `auth` 而路径 / 标题不含 → 不命中（不做全文搜索） | 单例 |
| `search` | 异常 | `openspec/specs/` 不参与：该目录内文件名与正文均含 `archi`，而元素与他人文档亦命中 → 结果只来自元素与他人文档路径，`specs` 下路径零出现 | 单例 |
| `search` | 异常 | 空 query：`""` 与 `"   "`（全空白）→ `Err`（参数校验），**不**返回全量；含 `\n` / `\0` / emoji 的 query 正常搜索不 panic | 表驱动（5 形态） |
| `search` | 边界 | scope 收窄：`Elements` → `docs` 恒空数组（`[]` 非 `null`）；`Docs` → `elements` 恒空数组；`All` → 两数组均可非空 | 表驱动（3 值） |
| `search` | 边界 | 多词 AND：`"auth legacy"` 仅命中同时含两段的元素，只含其一的元素不命中（D16；OR 属待决调优，不在本次断言面） | 表驱动（2 形态） |
| `search` | 边界 | 分词边界：`HTTPServer` → `http` + `server`；`v2Token` → `v2` + `token`；全大写 `AUTHTOKEN` → 不切分（无小写边界）；重复段 `AuthAuth` → 去重保序 | 表驱动（4 形态） |
| `search` | 边界 | 无命中 → 两数组皆空 `[]`、不 `Err`；空模型 → 空结果 | 表驱动（2 形态） |
| `search` | 边界 | `str` / 规模面：`query` 超长（>1000 chars）→ 正常返回（不截断、不 panic）；500 元素 / 200 文档 → 结果序稳定、两次调用逐字相同 | 表驱动（2 形态） |
| `search` | 边界 | 未知 `scope`：枚举封闭（类型层不可表示非法值），运行期非法串拒绝归 `SearchScope::parse`（`dto_test` 覆盖）——本节不虚构入参形态 | —— |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型与文档 | **不 mock**：`tempfile::TempDir` + 真实 `parse_dir` / `ModelGraph` / `docs::all_docs` 组合（检索面 = 索引来源之和，替身会抹掉来源口径） | 全部用例 |
| `openspec/specs/` 目录 | **真实建盘**（含文件与正文），断言其零命中——不以"不建目录"回避判据 | specs 不参与用例 |
| 时钟 / 环境 / 进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/diff.rs -> packages/desktop/src-tauri/crates/core/archi/src/diff_test.rs

#### 待测功能

- `model_diff(base, next)`: 元素增删（FQN 键、字典序清单）、关系增删计数（`(from,to)` 键）、`gate` 变更清单；文件 / 行序不参与身份（公共函数 / API）
- `ModelDiff::is_empty()`: 三组皆空（「模型无改动零占位」的判据）（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `model_diff` | 正向 | spec 场景：新增 `App.Billing` / `App.Billing.Invoice`、删除 `App.Legacy`、关系 +3 −1、`App.Billing` 的 `gate` `off` → `strict` → `elements_added` 2 项（字典序）、`elements_removed` 1 项、`relations_added = 3` / `relations_removed = 1`、`gate_changes` 恰一条 `{fqn, from: Off, to: Strict}` | 单例 |
| `model_diff` | 正向 | 等价模型（元素 / 关系 / 档位相同，仅文件划分与行序不同）→ 五组皆空、`is_empty() == true`（文件与行号不作身份） | 单例 |
| `model_diff` | 异常 | 同名不同 FQN（`App.Auth` vs `App.Auth2`）→ 一加一删互不抵消；关系 `A→B` 变 `A→C` → 删 1 加 1 | 表驱动（2 形态） |
| `model_diff` | 边界 | 两侧空模型 → 五组皆空 + `is_empty`；单侧空：base 空 / next 3 元素 → `elements_added` 3；反向（next 空）→ `elements_removed` 3 | 表驱动（3 形态） |
| `model_diff` | 边界 | `gate` 变更：同值（`Off` → `Off`）不计入；`Off → Warn` 与 `Warn → Strict` 各一条；新增元素带非缺省 `gate` → 只进 `elements_added`、不进 `gate_changes`（无旧值，与 FQN 身份键口径一致） | 表驱动（3 形态） |
| `model_diff` | 边界 | 关系身份去重：同 `(from,to)` 重复声明两次 → 计数按身份键 1；同一 `from` 指向不同 `to` → 多条独立 | 表驱动（2 形态） |
| `model_diff` | 边界 | 规模面：500 元素 / 1000 关系 → 计数正确、清单字典序；空组字段为 `[]` 非 `null` | 表驱动（2 形态） |
| `model_diff` | 边界 | `str` 面：超长 FQN（>1000 chars）/ 含 emoji / 含 `\n` 的 FQN → 清单逐字保留、不 panic | 表驱动（3 形态） |
| `ModelDiff::is_empty` | 边界 | 单组非空即 `false`（元素增 / 元素删 / 关系计数 / `gate` 变更各一例） | 表驱动（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 两侧模型 | **不 mock**：`Model` 结构体字面量直构（纯函数零 IO） | 全部用例 |
| 文件系统 / 解析 | **零依赖**：不经盘、不经 `parse_dir`（身份口径即被测语义） | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/edit.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit_test.rs

#### 待测功能

- `set_gate(source, fqn, gate)`: 纯函数——`toml_edit` 解析 → 改 AST → 渲染；元素缺席（附现有 FQN 清单）/ 渲染后不可解析 / 目标值回读不符 → `Err`（零文本产出）；注释空行键序逐字保留；零文件系统（公共函数 / API）
- `plan_set_gate(root, fqn, gate)`: 读模型定位声明文件（`modelFile`）→ 读源文本 → `set_gate` → 返回待落盘 `{ file, content }`；**不写盘**（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `set_gate` | 正向 | 含注释、空行与多元素的源文本，把 `App.Billing` 的 `gate` `off` → `strict`：目标行为 `gate = "strict"`；其余元素 / 注释 / 空行 / 键序**逐字**保留（按行 diff 只有目标行不同）；再用 `TomlParser::parse` 读回得新档位且零诊断 | 单例 |
| `set_gate` | 正向 | 目标元素无显式 `gate` 行 → 插入 `gate = "strict"`（D20 显式化缺省），其余逐字保留；再次调用改回 `off` → 行仍在且值为 `off`（幂等可逆） | 表驱动（2 形态） |
| `set_gate` | 正向 | 三档逐值往返：`Off` / `Warn` / `Strict` 各改一次并读回逐字（`GateTier` 全 3 值覆盖） | 表驱动（3 值） |
| `set_gate` | 异常 | 元素缺席 → `Err` 且消息附现有 FQN 清单（如 `App` / `App.Auth`）；零文本产出（`Err` 之外无 `Ok`） | 单例 |
| `set_gate` | 异常 | 渲染后不可解析即 `Err`：以含 `"` / 换行 / `]` 的注入式 FQN 触发表头键形破坏 → 自证解析失败，**不产出**任何待落盘文本（自证纪律） | 表驱动（3 注入形态） |
| `set_gate` | 异常 | 目标值回读不符 → `Err`（自证第二判据；以注入使渲染后键被解释为另一 FQN 的形态构造） | 单例 |
| `set_gate` | 边界 | 源文本边界：`""`（无元素）→ 元素缺席 `Err` 且清单为空；单元素最小文本；500 元素文本 → 仅目标行变化（其余字节相等） | 表驱动（3 形态） |
| `set_gate` | 边界 | 排版保留面：行内注释（`gate = "off"  # 说明`）、元素间空行、缩进、无尾换行、CRLF 换行 → 改值后各自保留（CRLF 以"换行字节不变"断言） | 表驱动（5 形态） |
| `set_gate` | 边界 | `str` 面：`fqn` 为 `""` / 超长（>1000）/ emoji 元素名（引号键内合法）→ 合法键形下正常改档；特殊字符触发自证失败时按异常臂处置（不 panic） | 表驱动（3 形态） |
| `set_gate` | 边界 | 非法 `gate` 值：`GateTier` 为封闭枚举（编译期不可表示），运行期非法串拒绝归 serde 反序列化（`dto_test`）与 IPC 参数反序列化（机械门面）——本节不虚构入参形态 | —— |
| `plan_set_gate` | 正向 | tempdir 内两文件模型（目标元素在 `02-auth.toml`）→ `GateWrite.file` 为该文件**绝对路径**、`content` 含新档位；调用后两文件字节**全等原文**（不写盘，D7） | 单例 |
| `plan_set_gate` | 异常 | 元素缺席 → `Err`；`models/` 目录缺席 → `Err`；目标文件读失败（同名目录占据 / `cfg(unix)` 不可读）→ `Err` 不 panic | 表驱动（3 形态） |
| `plan_set_gate` | 边界 | 空白 `root`（`""`）→ `Err`；不存在的 `root` → `Err`（模型目录缺席）；`fqn` 含 emoji → 合法键形成功 / 否则 `Err`（自证） | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `toml_edit` | **不 mock**：解析 → 改 AST → 渲染 → 自证全链即被测语义（注释保留是其能力） | `set_gate` 全部用例 |
| 模型目录与文件 | **不 mock**：`tempfile::TempDir` 真实落盘；`plan_set_gate` 后逐字节比对「零写入」 | `plan_set_gate` 全部用例 |
| TOML 读回校验 | **不 mock**：真实 `TomlParser::parse`（自证判据与解析层同源） | 读回断言 |
| 时钟 / 进程 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

#### 待测功能

- `graph_view(root)`: 画布数据单点——`parse_dir` → `ModelGraph::build` → 诊断合并；有结构诊断 → `elements` / `relations` 空（停摆），否则完整图（稳定序、`mark` 恒 `none`）（公共函数 / API）
- `module_detail(root, fqn)`: 单元素投影——`element` + `dependsOn` / `dependedBy` / `children` + `docs` + `warnings`；元素缺席 / 结构错 → `Err`（公共函数 / API）
- `module_overview(root)`: 全元素薄概览（`fqn` / `kind` / `gate` / `summary`，字典序）；结构错 → `Err`（公共函数 / API）
- `search(root, query, scope)`: 检索投影（薄条目 + 四字段文档条目）（公共函数 / API）
- `change_summary(main_root, next_root)`: 两侧模型 diff → 摘要；`next` 侧诊断随行；两侧等价或两侧模型目录俱缺席 → `None`（公共函数 / API）
- `linked_doc_paths(root)`: 关联集路径集（`archi_read_doc` 越界校验的唯一判据）（公共函数 / API）
- `SearchScope::parse(raw)`: `"elements"` / `"docs"` / `"all"`（其余 → `Err` 并列出合法值）（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `graph_view` | 正向 | 正常模型（4 元素 / 3 关系）→ `elements` 按 FQN、`relations` 按 `(from,to)` 稳定序；`mark` 恒 `none`、`reason` 恒 `null`；`model_file` / `model_line` 精确到表头行；`metadata: {}` / `path: []` / `docs: []` / `summary: null` 显式在场（投影不补默认值） | 单例 |
| `graph_view` | 异常 | 停摆全或无：九类结构错各一例（`syntaxError` / `missingField` / `invalidValue` / `unknownField` / `duplicateElement`（同文件与跨文件各一）/ `cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`）→ `diagnostics` 非空且 `elements` / `relations` **皆空** | 表驱动（9 类 + 跨文件） |
| `graph_view` | 边界 | 空态与停摆态可区分：`models/` 缺席 / 空目录 → 三数组皆空且 `diagnostics` 空；`docNotFound` 场景 → 不进 `diagnostics`（`ArchiGraph` 无 `warnings` 字段，D10） | 表驱动（3 形态） |
| `graph_view` | 边界 | 排序 / 规模面：500 元素稳定序、同名多关系排序稳定；连续两次调用结果逐字相同 | 表驱动（2 形态） |
| `module_detail` | 正向 | 元素 + `depends_on`（`target` + `description`）+ `depended_by`（`source` + `description`）+ `children` + `docs`（强度序）+ `warnings`；三项邻接与 `neighborhood` 逐一等同（投影零改写） | 单例 |
| `module_detail` | 异常 | 元素不存在 → `Err` 含总量提示（元素总数）；结构错 → `Err` 含诊断清单文本（`kind` + `file:line`）；坏模型不 panic（AC-2 / AC-10） | 表驱动（2 形态） |
| `module_detail` | 边界 | 无 `path` / `docs` / `summary` 的元素 → `path: []` / `docs: []` / `summary: null`（显式字段）；`fqn` 为 `""` / 超长 / emoji → 元素不存在 `Err`（不 panic）；500 元素模型 → 单元素投影只含 1-hop（不随总量膨胀） | 表驱动（4 形态） |
| `module_overview` | 正向 | 多元素 → 字典序、每条恰 `fqn` / `kind` / `gate` / `summary` 四字段（无 `metadata` / `path` / 邻接） | 单例 |
| `module_overview` | 异常 / 边界 | 结构错 → `Err`；空模型 → 空数组（不 `Err`）；单元素 → 1 条 | 表驱动（3 形态） |
| `dto::search` | 正向 / 异常 | 三 scope 透传（`Elements` → `docs: []`；`Docs` → `elements: []`）；空 query → `Err` 透传；结构错 → `Err` | 表驱动（5 形态） |
| `change_summary` | 正向 | `main` 与 `next` 差异（+2 / −1 元素、关系 +3 / −1、`gate` 一条）→ 计数 / 清单 / `gate_changes` 与 `model_diff` 逐项相等；`next` 侧诊断随行 | 单例 |
| `change_summary` | 边界 | 两侧等价 → `None`（零占位）；两侧模型目录**俱缺席** → `None`；仅 `next` 有模型（冷启动）→ `Some`（元素全为新增）；仅 `main` 有模型 → `Some`（元素全为删除） | 表驱动（4 形态） |
| `change_summary` | 异常 | `next` 侧结构错 → `Some` 且 `diagnostics` 非空（红字面数据，不阻断）；`main` 侧结构错 → `Err`（无可比基线，附诊断清单） | 表驱动（2 形态） |
| `linked_doc_paths` | 正向 | 三源并集路径集（`explicit` / `adjacent` / `adr`），repo 相对 `/` 分隔；同一路径多元素命中去重 | 单例 |
| `linked_doc_paths` | 边界 | 空模型 → 空集；`docNotFound` 路径**不**入集（与 `docs` 口径一致）；结构错 → `Err`；关联集为空时任意路径均不在集内（越界校验兜底） | 表驱动（4 形态） |
| `SearchScope::parse` | 正向 | `"elements"` / `"docs"` / `"all"` → 对应变体（3 值全覆） | 表驱动（3 值） |
| `SearchScope::parse` | 异常 | `""` / `"Elements"`（大小写敏感）/ `"all "`（尾空格）/ `"unknown"` / 超长串 → `Err` 且消息列出三合法值 | 表驱动（5 形态） |
| 词汇 serde 线形 | 边界 | 逐值断言：`ElementKind` / `GateTier` / `DocKind` / `DocSource` / `SearchScope` 为 lowercase 字面量（`"package"` / `"off"` / `"md"` / `"explicit"` / `"all"`）；`DiagnosticKind` / `WarningKind` / `EdgeMark` / `NodeMark` 为 camelCase（`"syntaxError"` / `"docNotFound"` / `"crossKind"` / `"unfulfilled"`）；非法值反序列化 `Err`（每枚举 N+1） | 表驱动（N+1 × 9 组） |
| 缺省与可选形态 | 边界 | `GateTier::default() == Off`；`ArchiDiagnostic { line: 0, column: None }` 序列化为 `line: 0` + `column: null`（不省字段）；`ArchiRelation.reason` 恒 `null` 显式在场；`ArchiElement.summary` / `metadata` 空值时显式 `null` / `{}` | 表驱动（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型目录与文档 | **不 mock**：`tempfile::TempDir` 真实落盘 + 真实 `parse_dir` / `ModelGraph` / `docs`（投影只裁剪，替身会引入第二事实源） | 全部用例 |
| serde / specta | **不 mock**：真实序列化 / 反序列化（线形与词汇封闭即被测契约）；不依赖 IPC runtime，不读 `bindings.ts` 产物 | 线形 / 词汇用例 |
| 时钟 / 环境 / 进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs

#### 待测功能

- `layout::archi_dir(root)`: `<域根>/architecture`；`architecture` 段唯一持有者，纯推导、零 IO、不存在的 root 亦正常返回（公共函数 / API）
- `layout::archi_models_dir(root)`: `archi_dir(root).join(MODELS_DIR_NAME)`（公共函数 / API）
- `layout::archi_decisions_dir(root)`: `archi_dir(root).join(DECISIONS_DIR_NAME)`（公共函数 / API）
- `layout::is_archi_rel_path(rel)`: repo 相对路径判定——`\` → `/` 归一、剥 `./`、按 `/` 切段后判「段 0 == 域根 && 段 1 == architecture 段」；归档冲突分流唯一判据（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_dir` / `archi_models_dir` / `archi_decisions_dir` | 正向 | tempdir root → `<root>/openspec/architecture`、`…/architecture/models`、`…/architecture/decisions`；三者以 `archi_dir` 为共同前缀（同源断言：`models == archi_dir.join(…)`、`decisions == archi_dir.join(…)`） | 单例 |
| `archi_dir` 族 | 边界 | 纯推导零 IO：调用后断言目录**未被创建**（`metadata().is_err()`）；不存在的 root 亦正常返回（`/nonexistent/root`） | 表驱动（2 形态） |
| `archi_dir` 族 | 边界 | `root` 形态：`""`（空路径）/ 相对路径 / Windows 盘符（`C:\demo\alpha`）/ 尾分隔符（`C:\demo\alpha\`）/ 超长路径（>1000）→ 逐字节等于 `root.join(段名)` 口径、不 panic | 表驱动（5 形态） |
| `is_archi_rel_path` | 正向 | `openspec/architecture/models/02-auth.toml` → `true`；`openspec/architecture` → `true`（祖先判定，无 tail 段亦命中） | 表驱动（2 形态） |
| `is_archi_rel_path` | 正向 | 归一形态：`openspec\architecture\models\x.toml`（Windows 反斜杠）→ `true`；`./openspec/architecture/models/x.toml`（`./` 前缀）→ `true` | 表驱动（2 形态） |
| `is_archi_rel_path` | 异常 | 负样本：`openspec/changes/x/specs/y.md` → `false`；`openspec/archived/a.toml` → `false`（不做前缀模糊匹配）；`src/architecture/x.rs` → `false`（段 0 非域根）；`openspec/architecturex/y` → `false` | 表驱动（4 形态） |
| `is_archi_rel_path` | 边界 | `""` → `false`；`/` → `false`；`openspec//architecture`（双斜杠空段）→ 按段位判定为 `false`（说明：仅剥 `./`，不吞任意空段）；`architecture`（无域根前缀）→ `false`；超长（>1000）→ 不 panic | 表驱动（5 形态） |
| `is_archi_rel_path` | 边界 | 大小写面：`Openspec/Architecture/...` → `false`（逐字节比较，不做大小写归一；说明口径） | 单例 |
| `Layout` / `resolve` / `config_path` / `domain_dir_name` | 回归 | 三字段语义逐字不变（既有用例全绿）；常量组新增三**私有**常量不改变既有常量值；`resolve` 的路径组装仍全量引用常量（零行内字面量）；`domain_dir_name()` 形态与既有消费点（`protect/rules.rs`）零改动 | 回归（既有断言零改写） |
| 命名隔离扫描 | 回归 | design 事实校准 3：`mod_test` 的 `collect_rs_files` 只覆盖 `src-tauri/src` 与 `packages/desktop/src`，**不覆盖 `crates/**`** → 既有扫描断言零改写；`archi` 侧的零字面量由源码审查承载（见「不可测试项」AC-9 行） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `root` 路径 | **不 mock**：真实 `tempfile::TempDir` + 字面路径（含 Windows 形态串）——纯路径函数，不触盘 | 全部用例 |
| 文件系统 | **零写依赖**：仅以 `metadata()` 断言"未被创建"（零 IO 的机械证据） | 纯推导用例 |
| 常量组 | **非 mock**：断言经派生函数（不直接引用私有常量），使「改名只动常量组」可机械验证 | 同源 / 回归用例 |

---

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

#### 待测功能

- `archi_graph(root)`: `#[tauri::command] #[specta::specta]` 薄包装（公共函数 / API）
- `archi_graph_inner(root)`: root 有效性检查（空白 / 非目录 → `Err`）→ `archi::dto::graph_view`（公共函数 / API）
- `archi_module(root, element)`: 薄包装（公共函数 / API）
- `archi_module_inner(root, element)`: root 检查 → `dto::module_detail`（公共函数 / API）
- `archi_read_doc(root, path)`: 薄包装（公共函数 / API）
- `archi_read_doc_inner(root, path)`: root 检查 → `dto::linked_doc_paths` 成员校验（越界显式 `Err`，零内容返回）→ 归一后读盘（只读）（公共函数 / API）
- `archi_change_summary(stores, root, change_id)`: 薄包装 + store 记录解析（`for_root` → `find_change_record` → worktree / 归档回落）（公共函数 / API）
- `archi_change_summary_inner(main_root, worktree)`: `next_root = worktree` 否则 `main_root` → `dto::change_summary`（公共函数 / API）
- `archi_set_gate(root, element, gate)`: 薄包装（公共函数 / API）
- `archi_set_gate_inner(root, element, gate)`: root 检查 → `plan_set_gate`（写前校验全在此）→ 原子写回（同目录临时文件 + `rename`）；任一校验失败零写入（公共函数 / API）
- `write_atomic`: 同目录临时文件 + `rename`（变更清单 ①；模块私有）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 五组包装 ↔ inner | 正向 | 同一 tempdir 下 `archi_graph` / `archi_module` / `archi_read_doc` / `archi_set_gate` 与其 `*_inner` 结果经 `serde_json::to_value` 逐字段一致（命令层零加工，沿 `commands/config/mod_test` 先例）；`archi_change_summary` 经 `tauri::test::mock_app()` 托管真实 `WorkspaceStores` 与 `_inner` 一致 | 表驱动（5 组） |
| `archi_graph` / `_inner` | 正向 | 正常模型 → `elements` / `relations` 完整、`mark` 恒 `none` | 单例 |
| `archi_graph` / `_inner` | 异常 | 坏模型（环 / 语法错各一）→ `diagnostics` 非空且两数组皆空（停摆态，AC-4）；不 panic | 表驱动（2 形态） |
| 五命令 root 校验 | 异常 | root 为 `""` / `"   "`（空白）/ 指向文件而非目录 / 不存在路径 → 一律 `Err`（D23），且不触达 `archi` 投影（写命令零写入） | 表驱动（4 形态 × 5 命令） |
| `archi_module` / `_inner` | 正向 / 异常 | 元素存在 → `ArchiModuleDetail` 全字段；元素缺席 → `Err` 含总量提示；root 非法 → `Err` | 表驱动（3 形态） |
| `archi_read_doc` / `_inner` | 正向 | 关联集内路径（explicit 文档）→ 返回正文原文（逐字节等于文件内容） | 单例 |
| `archi_read_doc` / `_inner` | 异常 | 越界拒绝：`openspec/changes/x/proposal.md` / 绝对路径 / `../` 逃逸 / 关联集内但文件已删除 → `Err` 且**零内容返回**（AC-6） | 表驱动（4 形态） |
| `archi_read_doc` / `_inner` | 边界 | `path = ""` / 超长（>1000）/ emoji；关联集为空（空模型）时任意路径均 `Err`；目录路径 → `Err` | 表驱动（5 形态） |
| `archi_change_summary` | 正向 | change 记录 worktree 在场 → worktree 根与 main 根 diff 摘要（元素 ± / 关系 ± / `gate` 变更逐项） | 单例 |
| `archi_change_summary` | 边界 | 归档 change（`worktree = null`）→ 回落主仓根（两侧等价 → `null`）；两侧模型目录俱缺席 → `null`（零占位） | 表驱动（3 形态） |
| `archi_change_summary` / `_inner` | 异常 | `change_id` 在 store 不可达 → `Err` 附 id；`worktree = Some(不存在路径)` → `Err`；`root` 非法 → `Err` | 表驱动（3 形态） |
| `archi_set_gate` / `_inner` | 正向 | tempdir 模型 → 调用后目标行变、其余内容与注释逐字保留；目录内文件清单不变（临时文件无残留）；读回档位为新值 | 单例 |
| `archi_set_gate` / `_inner` | 异常 | 元素缺席 / 自证失败（注入式 FQN）/ root 非法 → `Err` 且**该文件字节零变化**（调用前后逐字节比对，AC-19） | 表驱动（3 形态） |
| `archi_set_gate` / `_inner` | 边界 | 三档逐值往返（`strict` → `warn` → `off` → `strict`）；目标元素在第二文件（其余文件零变化）；元素无显式 `gate` 行 → 插入 | 表驱动（3 形态） |
| 四条读命令零写盘 | 回归 | `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` 调用前后模型与文档字节全等（AC-14「读命令零写盘」） | 表驱动（4 命令） |
| `write_atomic` | 边界 | 目标目录只读（`cfg(unix)` `0o555`）→ `Err` 且原文件不变、无临时文件残留；`cfg(windows)` 以目录被占用跳过并记因 | cfg 门控（unix） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 文档文件系统 | **不 mock**：`tempfile::TempDir` 真实落盘（五命令全链真实读盘 / 写盘） | 全部用例 |
| `WorkspaceStores`（store 层） | **不 mock**：`tauri::test::mock_app()` + temp 数据根真实 `WorkspaceStores`（沿 `commands/changes/mod_test.rs` 先例），种子经 store change 域操作面建档 / 归档翻转 | `archi_change_summary` 全臂 |
| Tauri runtime | **mock（框架装置）**：`tauri::test::mock_app()` / `MockRuntime`（无窗口无事件循环）；无 State 的四命令直调原签名 | 全部用例 |
| `archi::dto` 投影 | **不 mock**：真实 `archi` 调用（命令层零领域规则即被测语义） | 全部用例 |
| 网络 / 环境变量 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（只增 `pub mod archi;` 与 `all_commands!` 五条登记）。 -->

> 说明：`all_commands!` 是命令清单**唯一登记面**（`main.rs` 的 `generate_handler` 与 `bindings/mod.rs` 的 `collect_commands` 同源消费），其机械守卫位于 `src/bindings/mod_test.rs`（`COMMAND_NAMES` / `COMMAND_WRAPPERS` / `DTO_TYPES` 清单 + 重导出零 diff）。测试对象取自 design「修改文件」`commands/mod.rs` 行与 AC-20。
>
> **词汇出线口径（前次设计笔误的修正）**：`SessionInjections` / `SessionToolset` 是引擎侧会话注入词汇（`crates/core/agent/src/runner.rs`，serde 线形由 engine crate 自持），`all_commands!` 的出线参数与返回类型里**没有**它 ⇒ 产物 `bindings.ts` 全文零该类型名，属预期形态而非缺漏。故本节**不设「变体出线」正向断言**（不虚构产物面），改以反向锚定 + 对照臂固定口径；`SessionToolset::Archi` 的线形 / 旧线兼容 / 非法值语义由 `crates/core/agent/src/runner.rs -> runner_test.rs` 章节承担。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 命令清单在册 | 正向 | `COMMAND_NAMES` / `COMMAND_WRAPPERS` 含五条新命令（`archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` / `archi_set_gate` 与 `archiGraph` / `archiModule` / `archiReadDoc` / `archiChangeSummary` / `archiSetGate`），计数由 38 → 43 且既有条目零位移（尾部追加，`app_environment` 之后） | 单例 |
| 生成物覆盖 | 正向 | 重导出后产物逐条命中五条命令的包装名与 invoke 目标名（`all_commands!` 无遗漏）；`Archi*` / `DocRef` / `SearchScope` / 词汇枚举类型声明在产物命中（`export type <Name> =`） | 表驱动（五命令 + 类型清单） |
| 引擎侧词汇不出线（反向锚定） | 边界 | 产物 `bindings.ts` 全文**零** `SessionToolset` / `SessionInjections` / `TurnQuestion` 字样（specta 不出线 = 预期形态；引擎侧词汇一旦被误接到 IPC 出线面即红）；**对照臂**：同产物含 `export type ArchiModuleDetail =`（防空产物被误判为「零命中」）；本用例不新增任何「变体形状在场」正向断言 | 单例 |
| 重导出幂等 | 边界 | 连续两次 `export_bindings()` 逐字节一致（零 diff 门禁）；断言失败时 `RestoreOnDrop` 恢复产物 | 单例 |
| 遗漏反向证明 | 异常 | 以测试内清单副本删去一条新命令 → 产物覆盖断言必红（清单与产物强耦合的反向证据） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| specta 导出与文件系统 | **不 mock**：真实 `export_bindings()` 写 `src/types/generated/bindings.ts` 并读回（幂等 / 零 diff 即被测语义）；文件互斥锁 `Mutex` 串行化（既有装置） | 全部用例 |
| 命令实现 | **不 mock**：清单驱动的是真实注册面（不替身命令体） | 全部用例 |
| Tauri runtime / 网络 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

#### 待测功能

- `ArchiContext::new(root)`: per-turn 上下文（泵起播铸造，`root` = 会话 cwd，change 会话即 worktree）（公共函数 / API）
- `ArchiContext::execute(name, input)`: `archi_module`（有 / 无 `element` 两形态）、`archi_search`（`query` + `scope`）；坏模型 / 参数缺失 → `Err`（loop 侧 `is_error` 回灌、不 panic）；输出经 `tools::cap_result`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiContext::new` | 正向 / 边界 | `root` 为 tempdir 路径 → 上下文可用；`root` 不存在 / 空白路径 → `execute` 返回 `Err`（不 panic） | 表驱动（3 形态） |
| `ArchiContext::execute`（`archi_module` 定位） | 正向 | `{ "element": "App.Auth" }` → JSON 串含 `element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine`）+ `dependsOn` / `dependedBy` / `children` / `docs`；`modelFile` + `modelLine` 可直接定位声明行（AC-10 / AC-1） | 单例 |
| `ArchiContext::execute`（`archi_module` 薄概览） | 正向 | 省略 `element`（`{}`）→ 全元素薄概览（`fqn` / `kind` / `gate` / `summary`，无邻接、无文档）；元素数 = 模型元素数 | 单例 |
| `ArchiContext::execute`（薄概览截断） | 边界 | 数百元素（如 500）使输出超 30 KB → 结果以 `tools::cap_result` 收口：长度 ≤ 30 KB + 截断留痕尾行（与 `cap_result` 文案同形），不 panic | 单例 |
| `ArchiContext::execute`（`archi_module` 异常） | 异常 | 元素不存在 → `Err` 含总量提示；坏模型（环 / 语法错）→ `Err` 含结构化诊断（`kind` + `file:line`），不 panic（AC-10 / AC-14） | 表驱动（3 形态） |
| `ArchiContext::execute`（`archi_module` 参数） | 异常 | `element` 为非字符串（`1` / `null` / `[]`）或缺失键 → 按缺失处理（走薄概览）或 `Err`；入参为 `null`（非对象）→ `Err` 不 panic | 表驱动（4 形态） |
| `ArchiContext::execute`（`archi_search`） | 正向 | `{ "query": "auth", "scope": "all" }` → 元素薄条目 + 四字段文档条目；`scope` 缺省按 `all`；`scope = "elements"` → `docs` 空、`"docs"` → `elements` 空 | 表驱动（4 形态） |
| `ArchiContext::execute`（`archi_search` 参数） | 异常 | `query` 缺失 / `null` / 数字 → `Err`；`query = ""` → `Err`（透传 dto 参数校验）；`scope = "unknown"` → `Err` 列出三合法值 | 表驱动（5 形态） |
| `ArchiContext::execute`（工具名） | 异常 | 非两 archi 名的任意名（如 `read` / 未知串 / `""`）→ `Err`（不误路由到通用工具执行体） | 表驱动（4 形态） |
| 词汇面等价（工具串 vs DTO 串） | 边界 | 同一全词汇语料：工具输出串中出现的 `DiagnosticKind` / `EdgeMark` / `NodeMark` 词汇集与 `dto` 投影 `serde_json` 串相等（补齐 `lib.rs` 章节「工具串一面」在该 crate 不可达的等价断言，AC-7） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 文档文件系统 | **不 mock**：`tempfile::TempDir` 真实落盘 + 真实 `archi::dto` 投影 | 全部用例 |
| `tools::cap_result` | **不 mock**：真实 L1 收口（30 KB 上限与留痕文案即被测契约，禁第二份上限实现） | 截断用例 |
| 会话 root 来源 | **不 mock**：`ArchiContext::new(root)` 直接注入 tempdir 路径（等价于泵起播铸造值），不启动会话 / 泵 | 全部用例 |
| 网络 / 时钟 / 环境 | **零依赖**（执行体纯读盘、无 store 句柄） | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

#### 待测功能

- `tools::cap_result(text)`: L1 30 KB 收口单点，供 `archi_tool` 复用（禁第二份上限实现）；零行为变化（公共函数 / API；仅可见性 `fn` → `pub(crate) fn`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `definitions`（`Archi` 位） | 正向 | 四组合逐一：`{module: true, search: true}` → 七通用 + `archi_module` + `archi_search`；`{true, false}` → 仅 `archi_module`；`{false, true}` → 仅 `archi_search`；`{false, false}` → 零 archi 工具（两布尔位即子集表达，R-12） | 表驱动（4 组合） |
| `definitions`（schema） | 正向 | `archi_module` 参数仅 `element`（可选 string）；`archi_search` 参数含 `query`（必填 string）+ `scope`（可选，描述三值）；两定义 `name` 字面量为 `archi_module` / `archi_search` | 表驱动（2 定义） |
| `definitions`（`None` / `Explore` 臂） | 回归 | `toolset = None` → 七通用工具零追加；`toolset = Explore` → 七 + `create_explore_note`（逐字不变）；`tools = Some([...])` 子集收窄仍按 `TOOL_NAMES` 序过滤（既有语义零回归） | 表驱动（3 形态） |
| `input_path` | 正向 | `archi_module` / `archi_search` 恒返回 `None`（不进路径沙箱，即便入参含 `path` 字段）；`glob` → `pattern`、其余七工具 → `path`（既有语义不变） | 表驱动（3 形态） |
| `TOOL_NAMES` 封闭集 | 边界 | 仍恰 7 名且顺序不变（两 archi 名**不在**封闭集内，只经 `toolset` 注入） | 单例 |
| `cap_result` | 正向 / 边界 | 长度 ≤ 30 KB（含恰 30 000 字节边界）→ 原样返回；30 001 字节 → 截断到 char 边界 + 尾行留痕（`已截断：单结果超 30000 字节上限，仅保留前 N 字节`）；`""` 空串 → 原样；超长多字节（中文 / emoji）→ char 边界不回退到 0、不 panic | 表驱动（4 形态） |
| `cap_result` 复用面 | 边界 | `tools::execute` 与 `archi_tool` 两条调用点共用同一实现：同一超长文本经两路输出**逐字节相同**（禁第二份上限实现） | 单例 |
| 既有七工具 | 回归 | `read` / `grep` / `glob` / `ls` / `write` / `edit` / `bash` 的 `execute` 签名与语义零改动（既有用例全绿，零新增断言） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `definitions` / `input_path` / `cap_result` | **零 mock**：纯函数，入参为 `SessionInjections` / `Value` / `String` 字面量 | 全部用例 |
| 执行体（`tools::execute`） | **不在本节 mock / 不调用**：只断言两路输出的一致性契约（以真实纯函数调用两路） | `cap_result` 复用面 |
| 文件系统 / 进程 / 环境 | **零依赖**（本组被测函数不触达） | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`LoopTurn` 增字段与执行臂三分派）。 -->

> 说明：测试对象取自 design「修改文件」`loop.rs` ①②③ 三处（`LoopTurn.archi` / 三分派路由 / 守卫链注释）、`desktop-archi-agent`「只读档位归属与成员校验不变」场景与 AC-13。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 执行臂三分派 | 正向 | `turn.archi = Some(ctx)` + 下发面含 `archi_module` → 走 `ArchiContext::execute`，结果为 `is_error = false` 的工具结果；`archi_search` 同式 | 表驱动（2 工具名） |
| 执行臂上下文缺席 | 异常 | `turn.archi = None` 且下发面含 `archi_module` → 工具结果 `is_error = true` 且文案「专用工具执行上下文未装配」；run 不中断（后续轮继续） | 表驱动（2 工具名） |
| 成员校验（矩阵外） | 异常 | 下发面不含 archi 工具（矩阵外相位）而模型调用 `archi_module` / `archi_search` → `SystemNotice{subtype: "tool_unavailable"}` + `is_error` ToolResult（同 tool_call id）；执行体**零调用**（以执行上下文内的调用计数 / 副作用断言）；run 不中断 | 表驱动（2 工具名） |
| 守卫链次序 | 回归 | 成员校验先于 policy、policy 先于 sandbox、sandbox 先于 protect、执行臂最后（既有次序断言全绿）；两 archi 名在 sandbox 段原样通过（`input_path` 恒 `None`）→ 无 `sandbox_denied`；protect 对两名零拦截（读面） | 回归（次序 + 两名零拦截） |
| `LoopTurn` 构造随动 | 边界 | `LoopTurn` 字面量新增必填字段 `archi` 后，既有全部构造点编译随动（`Some` / `None` 两形态均覆盖）；`explore_note` / `protect` 字段语义零改动 | 批量改写 |
| `EXPLORE_NOTE_TOOL` 臂 | 回归 | 既有 explore 笔记工具路由与上下文缺席文案逐字不变（新增分支不改既有 `if/else` 语义） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| LLM 提供者 IO | **不 mock 网络，沿既有装置**：以假引擎 / 注入式 tool_call 序列驱动 loop（`loop_test` 既有 `Channel` 收集 + 假 `TurnRequest`），HTTP 层不触达 | 全部分派 / 成员校验用例 |
| `ArchiContext` 执行体 | **不 mock 主体**：真实上下文 + tempdir 模型（读面即被测语义）；仅"零调用"断言使用计数型替身（`Arc<AtomicUsize>` 包装的调用记录，沿既有 guard 替身先例） | 成员校验 / 分派用例 |
| `tools::execute` | **不 mock**：七通用工具路径保持真实纯函数（路由分支的对照臂） | 回归用例 |
| 时钟 / 环境 | **零依赖**；`Channel` 事件流为既有装置 | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（per-turn 铸造 `ArchiContext` + `LoopTurn` 字面量补字段）。 -->

> 说明：测试对象取自 design「修改文件」`sdk/runner.rs` ①②③（泵起播铸造、`use` 引入、`LoopTurn` 补字段）与「事实校准 2」（proposal 漏列的机械挂载）。
>
> **观测口径（G3 补口的端到端路径）**：`session_pump` 恒为私有面、runner 的 provider 恒指 discard 端点（无模型脚本注入缝）⇒ per-turn 铸造以 `runner::mint_archi_context(toolset, root)` **单点函数**承担（`session_pump` 唯一调用处，实参即 `open.injections.toolset` + `open.ctx.workspace_root`）；测试对同两点直断，`MUST NOT` 为测试提升 `session_pump` / `ArchiContext.root` 可见性。故 per-turn 铸造按**三段闭合**断言：⓪ 铸造段（`sdk/runner_test.rs`）= `Some(Archi { .. })` → `Some(ctx)`、`None` / `Explore` → `None`，且 `ctx` 读面只取所传 root 下模型（两 root 各一套元素，换 root 即换内容）；① 下发段（`open_session` 端到端，既有装置）= `RunStarted.tools` 按位追加两 archi 名（下发面由 `injections` 组装）；② 执行段（`sdk/loop_test.rs` **既有假引擎会话**：`FakeModel` + `drive_loop` 跑完整一轮）= `LoopTurn.archi = Some(ctx)` 下 `archi_module` 工具结果逐字段只取自**该 root** 下的模型（换 root 即换模型内容）→ `root` = 会话 cwd（change 会话即 worktree）的传递面反向锚定。三段合并即「per-turn 铸造 + root 传递」的端到端证明。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| per-turn 铸造（端到端，入口 = 泵起播） | 正向 | 链路 `open_session → 泵铸造 → loop 执行臂` 三段合证：⓪ 铸造点（`runner::mint_archi_context` 单点，实参即 `injections.toolset` + `ctx.workspace_root`）= `injections.toolset = Some(Archi { module, search })` → `Some(ctx)`、`None` / `Explore` → `None`，且 `ctx` 的 `archi_module` 结果只取自所传 root 下模型；① `injections.toolset = Some(Archi { module, search })` + 会话 `ctx.workspace_root = <worktree tempdir>` 起播 → `RunStarted.tools` 按位追加两 archi 名（`{true,true}` / `{true,false}` 两组合）；② 既有假引擎会话（`FakeModel` + `drive_loop`）以该 root 铸造的上下文（`LoopTurn.archi = Some(ctx)`）跑完整一轮（脚本下发 `archi_module`）→ 工具结果 JSON 的 `element.modelFile` / `modelLine` / 元素集合**只**取自该 root 下模型；换第二个 root（另一 tempdir、另一套元素）→ 结果逐字段随之切换（root 传错即红） | 表驱动（2 组合 × 2 root） |
| per-turn 铸造（非 Archi 会话） | 边界 | `toolset = None` / `Some(Explore)` → 下发面零 archi 名（`LoopTurn.archi` 恒 `None` 的行为面），`Explore` 仍得 `create_explore_note`（两专用上下文互不串道） | 表驱动（2 形态） |
| `RunStarted.tools` 一致性 | 正向 | Archi 会话起播：`RunStarted.tools` 与 `definitions(&injections)` 的 name 清单逐名相同（含 archi 工具与七通用工具） | 表驱动（2 组合） |
| 既有泵面 | 回归 | `explore_note` 铸造条件与 `PumpPayload` 逻辑逐字不变（`Explore` 会话仍得 `Some`）；`protect` / `web_search` / 模型解析零改动 | 回归 |
| `LoopTurn` 字段 | 边界 | `archi` 字段与 `explore_note` 并列存在（两上下文互不影响：Explore 会话 `archi = None`、Archi 会话 `explore_note = None`） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 会话 / 泵 IO | **沿既有装置**：① runner 段 = discard 端点 provider + `Channel` 收集 + 真实 `SessionCtx`（`sdk/runner_test.rs` 既有先例），不触达网络；② 执行段 = `sdk/loop_test.rs` 既有**假引擎会话**（`FakeModel` 脚本化流 + `drive_loop`），零网络 | 全部用例 |
| `ArchiContext` | **不 mock**：真实 `runner::mint_archi_context(toolset, root)` 铸造（`root` 来源即被测语义，`ArchiContext::new` 零替身） | 铸造用例 |
| 文件系统 | **不 mock**：铸造段 / 执行段 tempdir 内真实落 `openspec/architecture/models/*.toml`（两 root 各一套元素，工具结果取自盘面；铸造本身不读盘，`root` 绑定由执行体读面反向锚定） | 全部用例 |
| 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`READONLY_TOOLS` 清单扩至六名）。 -->

> 说明：测试对象取自 design「修改文件」`policy.rs` 行（`[&str; 4]` → `[&str; 6]`）、`desktop-archi-agent`「默认档放行两个 archi 工具」/「未知工具仍拒」与 AC-13。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `READONLY_TOOLS` | 正向 | 清单恰含 `read` / `grep` / `glob` / `ls` / `archi_module` / `archi_search` 六名；三档（`Default` / `AcceptEdits` / `BypassPermissions`）对两 archi 工具**全部放行**（6 格断言） | 表驱动（3 档 × 2 工具） |
| 只读面未扩宽 | 异常 | `Default` 档仍拒 `write` / `edit` / `bash` / `create_explore_note`（只读面扩两名的同时不扩其他面） | 表驱动（4 工具） |
| 未知工具 | 异常 | 词汇外工具名（`archi_write` / `""` / `read2`）在任何档位一律拒绝（封闭清单语义不变） | 表驱动（3 名 × 3 档） |
| `WRITE_TOOLS` / `EXECUTE_TOOLS` / `allows` 结构 | 回归 | 写面 / 执行面清单逐字不变；`allows` 三分支结构零改动（既有断言全绿，仅只读清单补两行断言） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 权限决策表 | **零 mock**：纯决策函数 + 工具名字面量（与 `TOOL_NAMES` / 两常量同源引用） | 全部用例 |
| 会话 / 文件系统 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`assemble_injections` 为模块私有，签名由 `(source)` 改 `(provenance)`）。 -->

> 说明：测试对象取自 design「修改文件」`compose.rs` ①②③（签名改造、`archi_matrix` 七行单源、`change_phase_role` 尾三段解析）、`desktop-archi-agent`「SessionToolset::Archi 与 (phase, role) 注入矩阵」矩阵表与 AC-12。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 矩阵 7 行逐行命中 | 正向 | `source = "change"`、`source_ref = "<id>/<phase>/<role>/<attempt>"` 逐行：`proposal/executor` → `{module:false, search:true}`；`dev-design/executor` 与 `dev-design/evaluator` → `{true, true}`；`test-design/executor` → `{false, true}`；`implement/executor`、`code-review/evaluator`、`acceptance/evaluator` → `{true, false}`；每行 `tools = None`（七通用工具全量） | 表驱动（7 行） |
| 矩阵位组合 | 边界 | 命中行的 `toolset` 布尔位与矩阵逐位相等（4 组合中 `{false,false}` 不由矩阵产出——该形态仅可经外部构造，见 `tools_test`） | 表驱动（3 组合） |
| 矩阵外相位 | 异常 | `<id>/test-gen/executor/1`、`<id>/test-execution/executor/1`、`<id>/archive/spec_sync/1`、`<id>/code-analyze/executor/1`、`<id>/initialize/executor/1` → `SessionInjections::default()`（零 archi 工具），不 `Err` | 表驱动（5 形态） |
| 畸形 `source_ref` | 异常 | `""`（空）/ `<id>/implement/executor`（3 段，末段非 `u32`）/ `<id>/implement/1`（role 段为数字、段数不足）/ `<id>/implement/executor/x`（末段非 `u32`）/ `<id>/implement/executor/1/extra`（末段 `extra` 非 `u32`，D11 仅取尾三段且末段须为数字）→ 恒 `default()`（零 archi 工具），不抛装配错误、不回退全量 archi 工具 | 表驱动（5 形态） |
| `attempt` 段边界 | 边界 | 可解析即命中（仅校验可解析性、不校验范围、不做符号黑名单）：`0` / `1` / `4294967295`（`u32::MAX`）/**`+1`**（Rust `u32::from_str` 接受前导 `+`，故命中）→ 各自命中矩阵行；不可解析 → `default()`（零 archi 工具）：`-1`（负号非 `u32` 语义）/ ` 1`（前导空格）/ `1.0` / `u32`（末段非数字）/ 超出 `u32` 的 `4294967296` | 表驱动（9 形态：4 命中 + 5 回落） |
| explore 臂 | 回归 | `source = "explore"` → `tools = Some([read, grep, glob, ls, edit, bash])`（按 `TOOL_NAMES` 序）+ `toolset = Some(Explore)`，`preamble = None`，逐字与变更前一致；六名成员校验仍生效 | 回归（逐字段） |
| 其余来源 | 回归 | `source = "debug"` / 未来源 → `SessionInjections::default()`（七工具面、零 archi） | 表驱动（2 形态） |
| 调用点契约 | 边界 | `begin` 传 `&provenance`（`source` 与 `source_ref` 同源取自 `SessionProvenance`）；装配结果进入 `TurnRequest.injections`，`resolve_agent_engine` 零改动 | 单例 |
| 非法名校验 | 回归 | explore 六名 ∉ `TOOL_NAMES` 的 `Err` 面保留（archi 面经枚举天然封闭，无自由字符串） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 装配纯函数 | **不 mock**：`SessionProvenance` 字面量直构（矩阵单源即被测语义），零 IO | 矩阵 / 畸形 / explore 用例 |
| `WorkspaceStores` | **不 mock**：真实 temp 库（`DualEnv` 既有装置）——`begin` 装配链路载荷校验用 | 调用点用例 |
| 网络 / 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`SessionToolset` 增变体属「类型定义」行）。 -->

> 说明：测试对象取自 design「类型定义」`SessionToolset` 行（线值 `{"archi":{"module":bool,"search":bool}}`、旧线缺字段读 `None`、`Explore` 臂零变化）、`desktop-archi-agent` 词表行与 AC-12。既有 `SessionToolset` serde 用例位于本文件（`runner_test.rs` 的既有词表 serde 小节），故本节落此而非 `lib_test.rs`；再导出锚定（`crate::SessionToolset::Archi` 可构造）与 serde 用例同文件落一行（不新增断言语义，见下）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionToolset::Archi` 序列化 | 正向 | `Archi { module: true, search: true }` → `{"archi":{"module":true,"search":true}}`（camelCase 外层变体名 + 内层两布尔字段）；`{true, false}` / `{false, true}` / `{false, false}` 各一线值（4 组合全覆） | 表驱动（4 组合） |
| `SessionToolset::Archi` 反序列化 | 正向 | 四线值 → 对应变体（round-trip 等值）；`Explore` 线值 `"explore"` → `Explore`（既有语义逐字） | 表驱动（5 形态） |
| 旧线兼容 | 边界 | `SessionInjections` 旧线 JSON 无 `toolset` 字段 → 读出 `toolset = None`（`#[serde(default)]`）；`toolset: null` → `None` | 表驱动（2 形态） |
| 非法线值 | 异常 | `"Archi"`（大小写敏感）/ `{"archi":{}}`（缺布尔字段）/ `{"archi":{"module":1,"search":true}}`（非 bool）/ `{"unknown":{}}` / `"unknown"` / `""` → 反序列化 `Err` | 表驱动（6 形态） |
| `Explore` 臂零回归 | 回归 | `Explore` 序列化 / 反序列化 / `SessionInjections` 线形逐字不变；新增变体不改变该臂任何可观察行为 | 回归 |
| `SessionInjections` doc 面 | 回归 | `tools` 收窄位、`preamble` 位语义零改动（既有断言全绿）；新增 `Archi` 变体不进入 `tools` 名单表达（枚举位唯一入口） | 回归 |
| 再导出锚定 | 边界 | crate 根与模块路径同词汇（`crate::SessionToolset::Archi` 可构造），一行锚定（不新增语义断言；与 serde 用例同文件，`lib_test.rs` 不另设用例） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde | **不 mock**：真实 `serde_json` 线值 / round-trip（线形即被测契约） | 全部用例 |
| 会话 / 文件系统 / 环境 | **零依赖**：`core/agent` 只承载词汇 | —— |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件（`check_rebase_halfway` 与文案函数在「修改文件」单元格命名，`pub(crate)`）。 -->

> 说明：测试对象取自 design「修改文件」`steps.rs` 行（`check_rebase_halfway` + `rebase_halfway_message`，D12 / D13 / D14）与 AC-17 的探测面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check_rebase_halfway` | 正向 | `worktree_snapshot().rebase_head = Some("abc")` → `Err(msg)`：`msg` 含 worktree **绝对路径**、`git rebase --continue` 与 `git rebase --abort` 两条可照抄命令、「再重跑归档」语义 | 单例 |
| `check_rebase_halfway` | 正向 | `rebase_head = None`（干净 / 已 continue / 已 abort 后重跑）→ `Ok(())` | 表驱动（2 形态） |
| `check_rebase_halfway` | 异常 | `worktree_snapshot` 自身 `Err` → `Err`（fail-closed，D13），文案附探测错误与两条出路 | 单例 |
| `rebase_halfway_message` | 边界 | 文案 `chars().count() <= 200`（含超长 worktree 路径）；三要素并列（路径 / 两条命令 / 重跑）；换行不破坏可照抄性 | 表驱动（3 路径长度：短 / 长 / 近上限） |
| `check_rebase_halfway` | 边界 | `worktree = ""`（空路径）由调用方遮蔽（walker 侧 `payload.worktree.as_deref()` 过滤），本节以空路径调用只断言不 panic；重复调用幂等（只读探测） | 表驱动（2 形态） |
| `run_archive_preflight` | 回归 | 本体**零改动**：停等面共用的 `Option` 返回、完成度 / delta specs / worktree 字段逐字不变（既有断言全绿，不新增错误通道） | 回归 |
| 探测实现面 | 回归 | 零新 vcs 能力、零 spawn：`grep`-式源码断言不在本测试面（机械自查），测试以假端口调用序断言仅 `worktree_snapshot` 一次 | 单例（调用序） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiveVcsPort` | **mock（既有装置扩展）**：`FakeVcs` 的 `worktree_snapshot` 增可编程 `rebase_head` / `Err` 两形态（现固定 `None`），调用序录制保留 | 全部探测用例 |
| 文件系统 | **不 mock**：worktree 仅作路径字符串传入（探测不读盘），tempdir 路径用于文案断言 | 文案用例 |
| git 子进程 | **零依赖**：不 spawn（零新 git 能力即被测契约） | —— |
| 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`step_preflight` / `step_resolve_conflicts` 内插入短路 + `archi_conflict_message`）。 -->

> 说明：测试对象取自 design「修改文件」`walker.rs` ①②（Preflight 前置挡回、模型冲突前置分流 + 文案函数）、D12 / D14 / D15 与 AC-16 / AC-17。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `step_preflight`（半程挡回） | 正向 | `payload.worktree = Some(wt)` 且 `worktree_snapshot().rebase_head = Some(_)` → `Terminal::failed`，文案含 wt 绝对路径 + 两条命令；节点落 failed；`settle_archive_node(passed)` 零调用 | 单例 |
| `step_preflight`（挡回前置性） | 异常 | 命中半程时：`commit_all` / `rebase_branch` / `ff_merge` / `rebase_abort` 调用计数全 0（`Commit` 步零执行）；db 落账零变化 | 单例 |
| `step_preflight`（放行） | 正向 | `rebase_head = None` → 既有 Preflight 流程逐字续走（完成度警告 / delta specs / 目录存在性分支任一形态）；`worktree = None`（legacy）→ 不探测、不挡回 | 表驱动（3 形态） |
| `step_preflight`（探测失败） | 异常 | `worktree_snapshot` 自身 `Err` → `Terminal::failed`（fail-closed）且文案附探测错误 | 单例 |
| `step_resolve_conflicts`（模型冲突） | 正向 | `rebase_conflicts = ["openspec/architecture/models/02-auth.toml"]` → `Terminal::failed`：文案含 worktree 绝对路径 + 「解完模型冲突后重跑归档」+ 人工裁决语义；`chars().count() <= 200` | 单例 |
| `step_resolve_conflicts`（零副作用） | 异常 | 模型冲突臂：agent 会话零发起（`FakeWorker` 调用计数 0 / run 节点无会话槽位）、`rebase_abort` 零调用、A 快照零取（`worktree_snapshot` 计数 0，D15 前置时机）、`rebase --continue` 零调用 | 单例 |
| `step_resolve_conflicts`（混合冲突） | 异常 | 冲突清单同时含模型路径与代码路径 → 走模型臂（`Terminal::failed`、零会话），不得先解代码（整体交人） | 单例 |
| `step_resolve_conflicts`（纯代码路径） | 回归 | 冲突清单全为代码路径 → 既有解算流程逐字不变：唤起会话、仅编辑清单内文件、pathspec 收口、后验三查、成功续走 / lean 收口两臂 | 回归（既有断言全绿） |
| `archi_conflict_message` | 边界 | 路径形态：`/` 与 `\` 分隔、祖先路径 `openspec/architecture`、多冲突路径（3 条）→ 均命中；`openspec/changes/x/specs/y.md` 等负样本不命中（判据单点 `is_archi_rel_path`） | 表驱动（5 形态） |
| 既有 archive 链 | 回归 | 11 步序、幂等短路（干净 / 已合入 / 已落盘 / worktree 不在场）、`lean_converge` 的 `rebase_abort` 语义、`diagnose_brief` 200 字截断均逐字不变 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiveVcsPort` | **mock（既有装置扩展）**：`FakeVcs` 的 `rebase_conflicts` / `worktree_snapshot.rebase_head` / `Err` 可编程 + 调用序录制（`calls()` / `call_count()` 既有方法） | 全部分流 / 挡回用例 |
| WorkerAgent（解冲突会话） | **mock**：`FakeWorker` 记录会话发起（模型冲突臂断言零发起） | 零会话断言 |
| store / 写面 | **mock**：`FakeStore` 既有装置（断言 db 零变化） | 挡回 / 冲突臂 |
| 文件系统 | **不 mock**：worktree / 主仓以 tempdir 路径参与（分流判据为纯路径） | 全部分支 |
| git 子进程 / 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/（7 份矩阵命中 md）-> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（角色 prompt md 为 `include_str!` 静态数据，无函数面）。 -->

> 说明：测试对象取自 design「修改文件」的 7 份 md 行（`proposal-planner` / `dev-design-planner` / `dev-design-evaluator` / `test-design-planner` / `implementation-generator` / `code-review-evaluator` / `acceptance-evaluator`）、`desktop-phase-prompts` 的「archi 工具引用纪律」与 AC-21。「其余 9 份 md 零 archi 字样」与「16 份零占位 / 零 `__MCP:*`」同为逐文件扫描面，故并入本节（同一测试文件、同一扫描装置）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 7 份命中 md（逐文件） | 正向 | `proposal-planner.md` 含 `archi_search` 且**不含** `archi_module`；`dev-design-planner.md` / `dev-design-evaluator.md` 两名皆含；`test-design-planner.md` 仅 `archi_search`；`implementation-generator.md` / `code-review-evaluator.md` / `acceptance-evaluator.md` 仅 `archi_module`（矩阵单源逐文件对拍） | 表驱动（7 文件 × 勾选集合） |
| 7 份命中 md（表述面） | 正向 | 每份命中工具的表述含「何时用」与「返回什么」两类信息：定位类提及邻接 / 文档引用 / `modelFile` + `modelLine` 可直接 `edit`；发现类提及「还不知道归谁管」语义 | 表驱动（7 文件） |
| 其余 9 份 md | 异常 | `proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-generator` / `test-gen-evaluator` / `test-execution-executor` / `test-execution-evaluator` / `code-analyze-planner` / `code-analyze-evaluator` 逐文件扫描 → `archi` 字样零命中（不许诺未下发工具面） | 表驱动（9 文件） |
| 两名并集 | 正向 | 16 份 md 中 `archi_module` 与 `archi_search` 的并集恰为 7 份命中文件（无遗漏、无越权）；两工具名之外无第三个 archi 工具名 | 表驱动（2 名 + 越权扫描） |
| 零路径模板 | 异常 | 7 份命中 md 不含 `openspec/architecture` / `architecture/` 一类字面路径、不含「用专用工具写模型」指令（路径知识归工具返回值） | 单例（逐文件 `!contains`） |
| 零占位与零 MCP | 边界 | 16 份 md 均无 `<change>` / `<phase>` / `{{ }}` / `__INCLUDE:*` / `__MODEL_*__` / `__DEV_TEAM_ROOT__` 与 `__MCP:*` 片段（既有扫描仍绿） | 表驱动（16 文件） |
| 装配面逐字不变 | 回归 | `phase_table.rs` 的 16 条 `include_str!` 字面路径与角色表**逐字不变**；16 份 md 静态非空；工具面真源仍为 `SessionToolset` 注入（prompt 不承载工具清单） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| prompt md 文件 | **不 mock**：真实读 `env!("CARGO_MANIFEST_DIR")/src/prompts` 目录（逐文件扫描需盘面真件；`*_test.rs` 不受路径字面量禁令约束） | 全部用例 |
| `phase_table` | **不 mock**：真实相位表 / `include_str!` 常量（装配面零改动即被测契约） | 回归用例 |
| 文件系统写面 / 网络 / 环境 | **零依赖**（只读） | —— |

---

### packages/desktop/src/routes.tsx -> packages/desktop/src/app.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的前端导出函数变更（仅插一条 `<Route>`）。 -->

> 说明：测试对象取自 design「修改文件」`routes.tsx` 行、`desktop-page-routing`「架构页按当前根渲染」场景与 AC-18 的路由面；`app.test.tsx` 是既有 App 级路由切换守门测试（`describe('App：路由化顶层页面切换…')`），故本节落此。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `/architecture` 路由 | 正向 | hash `#/architecture` → 渲染 `ArchitectureView`（`archi-canvas` / `archi-refresh` 在场）；既有 10 条路由与兜底 `*` 行为零变化 | 单例 |
| 侧栏入口跳转 | 正向 | 点击 `nav-architecture` → hash 落 `#/architecture`，变更清单 / Agent / DB 分支卸载；再点 `nav-changes` → 回 `#/changes` | 单例 |
| invoke 序列 | 边界 | 进入 `/architecture` 后取数只发 `archi_graph`（携当前 root，零 `list_changes` / `code_stats` 附带）；切走再切回 → `archi_graph` 重发恰一次（挂载触发）；以 `filter` 口径核对既有 invoke 序列断言（R-14） | 表驱动（2 形态） |
| 页面取数失败 | 异常 | `archi_graph` reject（root 不可读 / 读盘失败）→ 架构页呈现错误态（不在场任何节点边），路由仍停在 `#/architecture`（不白屏、不重定向）；切到 `nav-changes` 正常 | 单例 |
| 未知路径兜底 | 回归 | `#/unknown` → 重定向 `#/changes`；`#/` → `#/changes` | 回归 |
| 既有 10 条路由 | 回归 | `/changes` / `/changes/:id` / `/info` / `/config` / `/agents` / `/agent` / `/explores` / `/explores/:name` / `/db` 渲染面零变化（既有用例全绿） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（按命令名分发 fixture，`archi_graph` 返回图 / 停摆两形态） | 全部用例 |
| Tauri 插件 | **mock**：`@tauri-apps/api/app` / `plugin-dialog` / `plugin-updater`（既有 `app.test.tsx` 装置） | 全部用例 |
| 路由 | **不 mock**：真实 `HashRouter`（仅 `vi.mock` 观测 props，既有装置） | 路由用例 |
| 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/components/app-sidebar.tsx -> packages/desktop/src/components/app-sidebar.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`PageNavGroup` 增一项）。 -->

> 说明：测试对象取自 design「修改文件」`app-sidebar.tsx` 行（「架构」项、`data-testid="nav-architecture"`、组序 `[基础信息] [变更] [探索] [架构] [配置]`）与 AC-18 的侧栏面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 页面组新增项 | 正向 | 页面组呈现 `nav-architecture`（label「架构」），位于「探索」与「配置」之间；既有 `nav-info` / `nav-changes` / `nav-explores` / `nav-config`（及系统组 `nav-agents` / `nav-agent`）逐字不变 | 单例 |
| active 派生 | 正向 | 位于 `/architecture` 时该 NavLink 呈 active 标记；位于 `/changes` / `/explores` / `/config` 时不 active | 表驱动（4 路径） |
| active 派生 | 边界 | 精确匹配单点：`pathname = "/architecture"` → active；更深路径（`/architecture/models`）与非本级路径不 active（`pathname === '/architecture'` 判据，非前缀匹配） | 表驱动（3 路径） |
| active 派生 | 异常 | 相似前缀不误命中：`/architectures` / `/architecture-old` → 不 active（防前缀误判）；既有四项 active 语义不被新增项影响 | 表驱动（2 路径） |
| 点击语义 | 正向 | 点击「架构」→ 导航至 `/architecture`（`to` 属性同源），不触发 workspace 动作命令 | 单例 |
| 系统工具组 | 回归 | 「系统工具」组（`nav-agents` / `nav-agent`）与工作区选择器交互零变化（既有用例全绿） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ResizeObserver` | **mock（环境垫片）**：`vi.stubGlobal` 空实现（既有装置，非业务 mock） | 全部用例 |
| 路由 | **不 mock**：真实 `MemoryRouter` + `useLocation`（active 由 URL 派生的唯一判据） | 全部用例 |
| 工作区动作回调 | **非 mock**：`vi.fn()` 观测参数（既有装置） | 点击用例 |
| IPC / 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/architecture-view.tsx -> packages/desktop/src/views/architecture/architecture-view.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（页面组件，仅页头 + 画布挂载）。 -->

> 说明：测试对象取自 design「变更清单 新增文件」`architecture-view.tsx` 行（挂载点 A：页头 + 显式刷新 + `<ArchitectureCanvas root mode="full" />`）、`desktop-archi-canvas`「两处挂载点」「三触发取数」场景与 AC-18。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 挂载点 A | 正向 | 渲染 `ArchitectureView`（`root` 传入）→ 页头标题在场、`archi-refresh` 在场、`archi-canvas` 在场；画布以 `mode = "full"` 装配（元素抽屉内无 `gate` 可提交控件） | 单例 |
| 显式刷新 | 正向 | 点击 `archi-refresh` → `archi_graph` 以同一 `root` 重发恰一次（手动刷新为三触发之一）；不引入定时器 | 单例 |
| `root` 为 `null` | 异常 | 未选中工作区（`root = null`）→ 不发 invoke，呈现空态 / 占位（不一闪而空）；恢复 `root` 后取数发起 | 表驱动（2 形态） |
| 全量只读 | 边界 | 全页无 `archiSetGate` 调用点、无「新增 / 删除元素」「连边」入口（源码级断言以测试内 `grep` 替代不可行，改以行为断言：抽屉内无可提交控件 + 「改档需要开一个 change」提示在场） | 单例 |
| 零 watch / 零轮询 | 边界 | 挂载后推进假计时器（`vi.useFakeTimers` + `advanceTimers`）→ 无新增 invoke（三触发之外零取数） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（`archi_graph` 返回正常 / 停摆两形态） | 全部用例 |
| xyflow 度量 | **mock（环境垫片）**：`ResizeObserver` / `offsetWidth` / `getBBox` / `DOMMatrixReadOnly`（照抄 `change-flow-graph.test.tsx`） | 画布渲染用例 |
| 子组件 | **不 mock**：真实 `ArchitectureCanvas` 组合（两挂载点共用同一组件的判据） | 全部用例 |
| 计时器 | **mock（时间控制）**：`vi.useFakeTimers` 仅用于「零轮询」断言 | 零轮询用例 |
| 网络 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/components/architecture-canvas.tsx -> packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（组件入参 `{ root, mode }`）。 -->

> 说明：测试对象取自 design「变更清单 新增文件」`architecture-canvas.tsx` 行（三段布局、1-hop 缺省、两态切换、局部节点组件、`neighborhoodPositions` / `toNodes` / `toEdges`）、D9 / D21 与 AC-4 / AC-18；节点文案的「同一元素同一名字」判据取 `lib/fqn.ts` 的 `shortName`（单点，`fqn` 章节为纯函数面，本节为组件面）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 停摆态 | 正向 | `archi_graph` 返回 `diagnostics` 非空（`elements` / `relations` 空）→ 只渲 `archi-error-panel`：`archi-canvas` 零节点零边、元素树零 `archi-tree-node`、无降级样式 / 幽灵节点 | 单例 |
| 正常态 | 正向 | 完整图（4 元素 / 3 关系）→ 聚焦元素 + 1-hop 节点 / 边在场；三段布局齐备（元素树 / 画布 / 抽屉） | 单例 |
| 缺省 1-hop | 正向 | 数十元素模型、无显式聚焦 → 聚焦 = 首个顶层元素（前端按「FQN 无 `.`」取首个）；仅其出边 / 入边 / 直接子元素入图，非邻域元素零出现（可机械计数） | 单例 |
| 聚焦切换 | 边界 | 点击元素树另一节点 → 图按新邻域重算；`archi_graph` 调用计数不变（**零取数**，D21 / 三触发纪律） | 表驱动（2 次切换） |
| 空模型 | 边界 | 三数组皆空且零诊断 → `archi-canvas-empty` 空态（不是停摆面板） | 单例 |
| `mode` 差异 | 正向 | `mode = "full"` → 抽屉无 gate 编辑控件；`mode = "incremental"` → 抽屉有 gate 控件（`element-drawer` 侧断言，本组件只断言透传） | 表驱动（2 模式） |
| 改档后刷新 | 正向 | 抽屉改档成功回调 → 画布触发一次 `archi_graph` 重取（图与抽屉同步刷新）；改档失败 → 不重取、错误行内呈现 | 表驱动（2 形态） |
| `root` 变化 | 正向 | `root` 由 A 换到 B → 重新取数（三触发之一）；`root = null` → 不取数 | 表驱动（2 形态） |
| 节点文案（同一元素同一名字） | 正向 | 画布节点标签 = `shortName(fqn)`（`App.Auth.Token` → `Token`），与元素树同 fixture 逐字相同；节点标题 / tooltip 可携全 FQN，但 label 不含 `.` 前缀（两处取法同源，不各写一份） | 表驱动（3 形态） |
| 停摆不因文档警告 | 边界 | 图数据正常（`diagnostics` 空，模型含 `docs` 指向缺失文件）→ 正常态渲染（`docNotFound` 不触发停摆） | 单例 |
| 取数失败 | 异常 | `archi_graph` reject → 呈现错误面板 / 错误文案，零节点零边（与停摆同视觉纪律：不给部分图、不空白崩溃）；恢复后手动刷新回正常态 | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（`archi_graph` / `archi_module` / `archi_set_gate` 按命令名分发） | 全部用例 |
| xyflow 度量 | **mock（环境垫片）**：`ResizeObserver` / `offsetWidth` / `getBBox` / `DOMMatrixReadOnly` / viewport transform（jsdom 无布局引擎） | 画布渲染用例 |
| 子组件（元素树 / 抽屉 / 停摆面板） | **不 mock**：真实组合（三段联动即被测语义）；`marks` 纯函数真实组合 | 全部用例 |
| 网络 / 存储 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/components/element-tree.tsx -> packages/desktop/src/views/architecture/components/element-tree.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（组件）。 -->

> 说明：测试对象取自 design「变更清单 新增文件」`element-tree.tsx` 行（FQN 前缀派生层级、`nodeDot` 状态点、停摆态零节点）、D17 与 AC-18 / AC-22；节点文案经 `lib/fqn.ts` 的 `shortName`（末段短名单点，纯函数面在 `fqn` 章节）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 节点文案（同一元素同一名字） | 正向 | 节点 label 为 `shortName(fqn)`（`App.Auth.Token` → `Token`、`App` → `App`），与画布节点标签同源（`fqn.ts` 单点，两处不各写取法）；`fqn` 全文不进 label；点段空形态（`App.`）标签为空串且不抛错 | 表驱动（4 形态） |
| 层级派生 | 正向 | `App` / `App.Auth` / `App.Auth.Token` / `App.Billing` → 三层树（父 = 最长前缀一级），`App` 为根、`App.Auth` 展开后含 `Token`；子元素按 FQN 字典序 | 单例 |
| 前缀边界 | 边界 | `App.Auth` 与 `App.AuthLegacy` 为兄弟（非父子，按 `.` 边界）；无顶层元素（全部为子元素）→ 兜底渲染为扁平列表不丢节点 | 表驱动（2 形态） |
| 节点状态点 | 正向 | 节点 `mark = none` → `⚪`；`unfulfilled` → `🟡`（`archi-node-mark` 逐节点断言）；词汇外值 → 退化为 `⚪`（不抛错、不空白） | 表驱动（3 值） |
| 停摆 / 空态 | 异常 | `diagnostics` 非空 → 零 `archi-tree-node`；空模型 → 零节点（空态由画布承载） | 表驱动（2 形态） |
| 交互 | 正向 | 点击节点 → 上抛选中 FQN（父级据此切换聚焦，零取数）；展开 / 收起不改变节点数据 | 表驱动（2 形态） |
| 规模 | 边界 | 500 节点（含深链 `A.B.C.D.E.F`）→ 渲染不抛错、不丢节点；超长 FQN（>1000 chars）/ emoji FQN → 逐字呈现 | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 树数据 | **非 mock**：`ArchiElement[]` fixture 直构（纯展示组件） | 全部用例 |
| `nodeDot` / `shortName` | **不 mock**：真实 `marks.ts` / `fqn.ts` 组合（状态点词汇、退化与末段取法即被测语义） | 状态点 / 节点文案用例 |
| IPC / 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/components/element-drawer.tsx -> packages/desktop/src/views/architecture/components/element-drawer.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（组件）。 -->

> 说明：测试对象取自 design「变更清单 新增文件」`element-drawer.tsx` 行（概览 / docs / ADR 条目与按需正文 / `incremental` 下 gate 编辑 / `full` 提示 / 本期零「相邻报告」区块）、D22 与 AC-6 / AC-18 / AC-19。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 概览 | 正向 | 选中元素 → `kind` / `path`（逐项）/ `gate` / `archi-drawer-position`（`modelFile:line`）在场；`modelLine == 0` 时只渲 `modelFile`（位置未知形态） | 表驱动（2 形态） |
| docs 列表 | 正向 | 三源条目按强度序（`explicit` → `adjacent` → `text`）逐个 `archi-doc-item`；条目不含正文（仅四字段） | 单例 |
| docs 空态 | 边界 | `docs: []` → 列表空、无占位框、无告警行（`docNotFound` 只在 `warnings` 面呈现，不阻断） | 单例 |
| ADR 正文按需取 | 正向 | 展开 ADR 条目 → 调 `archi_read_doc`（携该条目 `path`）→ `archi-adr-body` 渲染 markdown（标题 / 列表 / 代码块）；未展开零 invoke | 表驱动（2 形态） |
| ADR 取数失败 | 异常 | `archi_read_doc` 返回 `Err`（路径不在关联集 / 文件已删）→ 行内错误呈现（`error` 行），画布不崩、其余区块不变 | 单例 |
| `incremental` 改档 | 正向 | `mode = "incremental"` → gate 三档选择 + 提交 → 调 `archiSetGate(root, fqn, gate)`；成功后 `onChanged()` 触发父级重取、`gate` 显示新值 | 表驱动（3 档值） |
| `incremental` 改档失败 | 异常 | `archiSetGate` 返回 `Err` → `archi-gate-error` 行内错误在场、磁盘零变化（前端不做本地乐观改写：`gate` 保持旧值）、不触发重取 | 单例 |
| `full` 只读 | 正向 | `mode = "full"` → 零可提交控件（无 select / 无提交按钮）、「改档需要开一个 change」提示在场；`archiSetGate` 零调用 | 单例 |
| 本期零「相邻报告」 | 边界 | 抽屉内无「相邻报告」区块、无 `errors` / `warnings` 计数、无空占位框 | 单例 |
| 元素缺席 / 结构错 | 异常 | `archi_module` 返回 `Err` → 抽屉呈现错误文本（含总量提示 / 诊断清单），不渲染半截概览 | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（`archi_read_doc` / `archi_set_gate` 按命令名分发） | 全部用例 |
| markdown 渲染 | **不 mock**：真实 `react-markdown` + `remark-gfm`（模块级 `REMARK_PLUGINS` 装配与 `markdown-doc-renderer` 同款） | ADR 正文用例 |
| 父级回调 | **非 mock**：`vi.fn()` 观测 `onChanged` 调用（重取触发即被测语义） | 改档用例 |
| 网络 / 时钟 / 存储 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/components/structure-error-panel.tsx -> packages/desktop/src/views/architecture/components/structure-error-panel.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（组件）。 -->

> 说明：测试对象取自 design「变更清单 新增文件」`structure-error-panel.tsx` 行（逐条 `kind` + `message` + `file:line` + 「修好后再看」；`line === 0` 只渲 `file`）与 D5 / D9、AC-4。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 诊断逐条呈现 | 正向 | 3 条诊断（`cycle` / `unknownReference` / `missingField`）→ 3 行 `archi-error-row`，每行含 `kind` 字面量、`message` 全文、`file:line` | 单例 |
| 位置未知 | 边界 | `line == 0` 且 `column == None` → 只渲 `file`（不出现 `:0`）；`column` 有值时按 `file:line:column` 或 `file:line`（说明口径）呈现 | 表驱动（3 形态） |
| 修好后再看 | 正向 | 面板固定引导文案在场（「修好后再看」语义）；无图数据、无降级样式类 | 单例 |
| 诊断清单规模 | 边界 | 9 条诊断（九类各一）→ 9 行不截断、不重复；单条超长 `message`（>1000 chars）→ 逐字呈现不截断 | 表驱动（2 形态） |
| 空清单 | 异常 | `diagnostics: []` → 面板不渲染（由父级两态切换保证），本组件不出现零数据标题 / 空框 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 诊断数据 | **非 mock**：`ArchiDiagnostic[]` fixture 直构 | 全部用例 |
| IPC / 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/lib/fqn.ts -> packages/desktop/src/views/architecture/lib/fqn.test.ts

#### 待测功能

- `shortName(fqn)`: FQN 末段短名（`App.Auth.Token` → `Token`）；元素树节点与画布节点**共用同一取法**（公共函数 / API）

> 说明：测试对象取自 design「公共函数 / API」`shortName` 行与「变更清单 新增文件」`lib/fqn.ts` 行（末段短名单点，`desktop-archi-canvas`「同一元素同一名字」）、AC-18 的同名面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `shortName` | 正向 | 末段取法：`App.Auth.Token` → `Token`；`App.Auth` → `Auth`；`App` → `App`（无点即自身）；`App.Auth.Token.Sink` → `Sink`；大小写逐字保留（`app.auth` → `auth`，不做归一） | 表驱动（5 形态） |
| `shortName` | 正向 | 单点取法对拍：同一 `App.Auth.Token` 入参在元素树与画布节点两侧产出**同一短名**（以独立镜像实现逐值对拍，任一取法漂移即红）；结果内不含 `.`，故再次取末段幂等（`shortName(shortName(x)) == shortName(x)`） | 表驱动（12 样本） |
| `shortName` | 异常 | 点段空形态逐字映射、不补默认名、不抛错：`""` → `""`；`.` → `""`；`App.` → `""`；`.Token` → `Token`；`App..Token` → `Token`；`App.Auth.` → `""` | 表驱动（6 形态） |
| `shortName` | 边界 | `str` 特殊字符逐字保留：`App.Auth\nToken` → `Auth\nToken`；`App.Auth\0Token` → `Auth\0Token`；`App.🚀Token` → `🚀Token`；`应用.认证.令牌` → `令牌`；`App.认证.🚀` → `🚀` | 表驱动（5 形态） |
| `shortName` | 边界 | 超长面（>1000 chars）不截断：`App.<1200 段>` → 该段原样；整串无点且超长 → 原样返回；`'Seg.'×400 + 'Tail'` → `Tail`（多点前缀只取末段） | 表驱动（3 形态） |
| `shortName` | 边界 | 深链规模：500 段 FQN 与其前缀化形态 → 均只取末段（`Seg499`），不栈溢出、不抛错、不变异 | 表驱动（2 形态） |
| `shortName` | 边界 | 纯函数面：同入参连续调用等值（`App.Auth.Token` / `""` / `App.` / `App.🚀Token`）；返回恒为 `string`（不出现 `undefined` / `null`）；入参类型为 `string`（非 `string \| null`），`null` / `undefined` 类型层不可表示，不虚构该形态 | 表驱动（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 被测函数 | **零 mock**：纯函数直调，无渲染、无 jsdom 垫片、无 IPC、无计时器 | 全部用例 |
| 末段取法镜像 | **非 mock**：测试内独立镜像实现（按 `.` 切段取末段）作单点对拍基线（不 import 源实现内部逻辑） | 单点取法用例 |
| 网络 / 存储 / 环境 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/lib/marks.ts -> packages/desktop/src/views/architecture/lib/marks.test.ts

#### 待测功能

- `edgeVisual(mark)`: 边标记四值 → `{ stroke, dasharray }`；词汇外 → 与 `none` 同款中性（不抛错、不空白）（公共函数 / API）
- `nodeDot(mark)`: 节点标记两值 → `'⚪' \| '🟡'`；词汇外 → `'⚪'`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `edgeVisual` | 正向 | `none` → 实线（`dasharray == null`）；`unfulfilled` → 灰虚线；`crossKind` → 橙虚线；`exempt` → 灰实线（四值各自的 `stroke` / `dasharray` 逐值断言，AC-22） | 表驱动（4 值） |
| `edgeVisual` | 异常 | 词汇外值：`""` / `"unknown"` / `"None"`（大小写）/ `"crosskind"` → 返回与 `none` **同款**对象（确定性退化），不抛错、不返回 undefined | 表驱动（4 形态） |
| `edgeVisual` | 边界 | 返回值形状恒为 `{ stroke: string, dasharray: string \| null }`（两字段恒在场）；连续调用同一入参返回等值对象（纯函数） | 表驱动（2 形态） |
| `nodeDot` | 正向 | `none` → `'⚪'`；`unfulfilled` → `'🟡'`（两值） | 表驱动（2 值） |
| `nodeDot` | 异常 / 边界 | 词汇外值（`""` / `"cycle"` / `"unfulfilled "` 尾空格）→ `'⚪'`；**结构错值不得存在**（`NodeMark` 不含结构错，词汇表断言 2 值） | 表驱动（3 形态） |
| 零标记基线 | 边界 | 本期图数据恒 `none` → 全部边实线（`dasharray == null`）、全部节点 `⚪`；与 `tests/vocabulary_test.rs` 的边 4 / 节点 2 词表逐值一致（跨语言对拍：词表以测试内常量镜像并断言集合相等，AC-7 / AC-22） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 被测函数 | **零 mock**：纯函数直调，无渲染、无 jsdom 垫片、无 IPC | 全部用例 |
| 词汇镜像 | **非 mock**：测试内常量清单（`EDGE_MARKS` / `NODE_MARKS`）与 `marks.ts` 行为逐值对照（Rust 侧同清单在 `vocabulary_test` 断言，两侧手工同源） | 零标记 / 词表用例 |
| 网络 / 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/hooks/use-archi-graph.ts -> packages/desktop/src/views/architecture/hooks/use-archi-graph.test.ts

#### 待测功能

- `useArchiGraph(root)`: 三触发取数（挂载 / `root` 变化 / 手动刷新），零 watch 零轮询；`root` 为 `null` 时不发 invoke；返回 `{ graph, loading, error, refresh }`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 挂载取数 | 正向 | `root = "C:\demo\alpha"` 挂载 → `invoke("archi_graph", { root })` 恰一次；`loading` 由 true → false；`graph` = 应答（正常态） | 单例 |
| `root` 变化 | 正向 | `root` 由 A 换 B → 以 B 重发一次（A 应答迟到不覆盖 B，竞态丢弃）；`null` → `graph = null`、不发 invoke | 表驱动（3 形态） |
| 手动刷新 | 正向 | 调 `refresh()` → 原 `root` 重发一次；连续两次刷新 → 两次 invoke（不做去重缓存） | 表驱动（2 形态） |
| 停摆态透出 | 正向 | 应答 `diagnostics` 非空 + 两数组空 → `graph` 原样透出（不裁剪、不补默认值），由画布判两态 | 单例 |
| 错误面 | 异常 | `invoke` reject（空白 root / 读盘失败）→ `error` 为该串、`graph = null`、`loading = false`；再刷新成功后 `error` 清空 | 表驱动（2 形态） |
| 卸载 | 边界 | 卸载时未完成应答不回写状态（无 act 警告 / 无状态更新）；`root = null` 时 `refresh()` 不发 invoke | 表驱动（2 形态） |
| 零 watch / 零轮询 | 边界 | 假计时器推进（秒级多项）→ invoke 计数不变；`root` 未变且未刷新 → 零重发（三触发之外零取数） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke` + `vi.hoisted` 句柄（既有 hook 测试装置，含挂起门闩驱动 loading / 竞态） | 全部用例 |
| `renderHook` | **非 mock**：`@testing-library/react` 的 `act` / `waitFor`（既有先例） | 全部用例 |
| 计时器 | **mock（时间控制）**：`vi.useFakeTimers` 仅用于零轮询断言 | 零轮询用例 |
| 文件系统 / 网络 / 存储 | **零依赖**（hook 只经 IPC） | —— |

---

### packages/desktop/src/views/architecture/hooks/use-archi-module.ts -> packages/desktop/src/views/architecture/hooks/use-archi-module.test.ts

#### 待测功能

- `useArchiModule(root, fqn)`: 抽屉元素详情（`fqn` 变化即重取；`null` 不发），返回 `{ detail, loading, error }`（公共函数 / API）
- `useArchiDoc(root)`: ADR 正文按需取（展开才发 invoke；`Err` 落 `error` 行内呈现），返回 `{ body, loading, error, load, reset }`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useArchiModule` 挂载 | 正向 | `root` 与 `fqn` 均在场 → `invoke("archi_module", { root, element: fqn })` 恰一次；`detail` 落地（含 `element` / `dependsOn` / `children` / `docs`） | 单例 |
| `useArchiModule` 重取 | 正向 | `fqn` 由 `App.Auth` 换 `App.Billing` → 以新 `fqn` 重发；`root` 变化亦重发；同一 `fqn` 重复渲染（父级重渲）→ 不重复 invoke | 表驱动（3 形态） |
| `useArchiModule` 空参 | 边界 | `fqn = null` 或 `root = null` → 不发 invoke、`detail = null`（不发起空串查询） | 表驱动（3 形态：仅 fqn null / 仅 root null / 双 null） |
| `useArchiModule` 错误面 | 异常 | `invoke` reject（元素缺席 / 结构错 / 坏模型）→ `error` 为文案、`detail = null`、`loading = false`；切换到有效 `fqn` 后 `error` 清空 | 表驱动（2 形态） |
| `useArchiDoc` 按需取 | 正向 | 初始 `body = null` 且零 invoke；`load(path)` → `invoke("archi_read_doc", { root, path })` 一次、`body` 为正文 | 表驱动（2 形态） |
| `useArchiDoc` 展开语义 | 边界 | 连续 `load` 两个不同路径 → 各自一次 invoke，后到者覆盖 `body`；`reset()` → `body` / `error` 清空、不再 invoke；`root = null` 时 `load` 返回错误 / 不发 invoke | 表驱动（4 形态） |
| `useArchiDoc` 错误面 | 异常 | `invoke` reject（越界路径 / 文件已删）→ `error` 承载文案、`body = null`（不残留上一份正文）；画布其余状态不变 | 表驱动（2 形态） |
| 零 watch / 零轮询 | 边界 | 假计时器推进 → `archi_module` / `archi_read_doc` 计数不变（取数只由 `fqn` / `root` 变化与显式 `load` 触发） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke` + `vi.hoisted` 句柄（含挂起门闩驱动 loading） | 全部用例 |
| `renderHook` / `act` / `waitFor` | **非 mock**：`@testing-library/react`（既有 hook 测试先例） | 全部用例 |
| 计时器 | **mock（时间控制）**：`vi.useFakeTimers` 仅用于零轮询断言 | 零轮询用例 |
| markdown 渲染 / 文件系统 / 网络 | **零依赖**（本 hook 只经 IPC） | —— |

---

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（右栏增页签 + `archiRoot` 派生 + `root` 透传）。 -->

> 说明：测试对象取自 design「修改文件」`change-detail-view.tsx` 行 ①–④（页签、`archiRoot` 派生单点、条件挂载、`DetailActionPanels` / `RunControlPanel` 透传）、`desktop-archi-canvas`「挂载点 B」「归档 change 回落主仓根」场景与 AC-18。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 页签呈现 | 正向 | 右栏呈现 `detail-tab-flow` 与 `detail-tab-architecture` 两页签，缺省 `flow`（流程图 + 产物抽屉在场、架构画布不在场） | 单例 |
| 页签切换 | 正向 | 点击 `detail-tab-architecture` → 架构画布在场（`archi-canvas`）、流程图卸载；切回 `flow` → 流程图恢复、画布卸载 | 单例 |
| 条件挂载 | 边界 | 未激活架构页签时**零** `archi_graph` / `archi_module` invoke；首次激活发起一次；反复切换（激活 → 卸载 → 再激活）→ 每次激活各一次（不重复取数、不缓存跨挂载） | 表驱动（3 形态） |
| `archiRoot` 派生 | 正向 | `detail.worktree = "C:\demo\alpha\.worktrees\add-feature"` → 画布以其为 `root`（`archi_graph` 参数逐字等于该值） | 单例 |
| 归档回落 | 正向 | `detail.source = "archive"`（`worktree = null`）→ `root` 为主仓根（`archi_graph` 以页面 `root` 发起），**不**以已清理 worktree 取数 | 表驱动（2 形态：source=archive / worktree=null） |
| 增量模式 | 正向 | 页签内画布以 `mode = "incremental"` 装配 → 抽屉 gate 可提交（`archiSetGate` 以 worktree root 发起） | 单例 |
| 既有流程图 / 抽屉 / 左栏 | 回归 | 流程图节点 / 边、产物抽屉、左栏 Agent 面板语义零改动（既有用例全绿）；`RunControlPanel` / `DetailActionPanels` 收 `root` / `changeId` 后既有行为不变 | 回归 |
| invoke 序列 | 边界 | 详情页首次挂载的 invoke 序列以 `filter` 口径核对（新增取数只在架构页签激活时出现，R-14） | 单例 |
| 架构页签取数失败 | 异常 | 激活「架构」页签后 `archi_graph` reject → 页签内呈现错误态（画布错误面板），流程图页签仍可切回且内容不变；详情页不崩、不清空既有 `detail` | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock**：`vi.mock('@tauri-apps/api/core')` 的 `invoke` + `Channel` 桩（`get_change_detail` / `archi_graph` / `archi_module` / `archi_set_gate` 按命令名分发） | 全部用例 |
| 路由 | **不 mock**：`MemoryRouter` + `Route /changes/:id`（既有装置） | 全部用例 |
| 画布 / 流程图子组件 | **不 mock**：真实 `ArchitectureCanvas` 与既有流程组件组合（挂载点 B 与页签联动即被测语义）；xyflow 度量垫片照抄既有先例 | 页签 / 派生用例 |
| 网络 / 存储 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`ArchiveConfirmCard` 增区块 + `root` / `changeId` 入参）。 -->

> 说明：测试对象取自 design「修改文件」`run-control-panel.tsx` 行 ①②（`ArchitectureChangeBlock`：独立取数 / 零占位 / 逐项计数 / 两入口 / 红字不阻断 / 插入位置）、`desktop-change-archive`「架构变更区块」场景与 AC-15。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有改动 → 区块呈现 | 正向 | `archiChangeSummary` 返回 `Some`（元素 +2 / −1、关系 +3 / −1、`gate` 一条）→ `run-archi-block` 在场：元素 `+2` / `−1` 含 FQN 清单、关系 `+3` / `−1`、`gate` 行为 `fqn：off → strict` | 单例 |
| 无改动 → 零占位 | 边界 | `summary = null` → 区块**完全不渲染**（无标题、无空框、无 0 计数行）；既有确认卡片区块照常 | 单例 |
| 结构错红字 | 异常 | `summary.diagnostics` 非空 → `run-archi-error` 逐条红字（`kind` + `message` + `file:line`）+「修复后再归档」；确认按钮仍可点击（不阻断，`confirm(proceed, true)` 语义不变） | 表驱动（2 形态） |
| 两入口 | 正向 | 点 `run-archi-diff` → 就地展开明细（元素 / 关系 / gate 清单）；点 `run-archi-preview` → 就地展开 `archi-canvas`（`root = preflight().worktree`，`mode = "incremental"`） | 表驱动（2 入口） |
| 取数时机与参数 | 正向 | 确认面出现且 `changeId` 在场 → 调一次 `archiChangeSummary(root, changeId)`；`changeId` 缺席 → 零 invoke；`root` / `changeId` 由 `RunControlPanel` 下传（不读 `preflight` 的 archi 字段——`ArchivePreflight` 零扩展） | 表驱动（4 形态） |
| 取数失败 | 异常 | `archiChangeSummary` reject → 区块呈现行内错误（不阻断确认），或按 `null` 处置零占位（说明口径：错误不静默，至少一行错误文本） | 单例 |
| 插入位置 | 边界 | 区块位于 `run-archive-delta-specs` 之后、`run-archive-sync-skip` 之前（DOM 顺序断言） | 单例 |
| 既有确认卡片 | 回归 | `run-archive-confirm-card` / `run-archive-warning` / `run-archive-delta-specs` / `run-archive-merge-target` / `run-archive-confirm-proceed` / `run-archive-confirm-stop` / `run-archive-sync-skip` 的 testid、文案与交互逐字不变；`archive: null` 降级渲染、非 archive 相位走普通确认卡片等既有用例全绿 | 回归 |
| 本期射程外信息 | 边界 | 区块内不出现「跨语言依赖」「符号级契约」「相邻报告」一类字段 / 文案 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（`archi_change_summary` / `archi_graph` 按命令名 + 参数分发） | 全部用例 |
| `preflight()` fixture | **非 mock**：既有构造器直构（`worktree` 字段供预览画布 root；**不新增 archi 字段**——`ArchivePreflight` 零扩展） | 全部用例 |
| 画布子组件 | **不 mock**：预览入口展开真实 `ArchitectureCanvas`（`mode = "incremental"`）；xyflow 度量垫片照抄既有先例 | 预览入口用例 |
| 网络 / 存储 / 时钟 | **零依赖** | —— |

---

## 不可测试项

| 模块 / 面 | 原因 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/Cargo.toml`（AC-8） | 依赖清单为声明式配置，无单测路径；由 `Cargo.toml` 审查核验（`archi` workspace 内依赖仅 `foundation`、零 Tauri / store / spawn / workflow / orchestration / config），并以 `cargo clippy -D warnings` 与编译面兜底 |
| `packages/desktop/src-tauri/Cargo.toml`（AC-8 / AC-20） | `members` / `[workspace.dependencies]` / 壳依赖与 `[package] version` 不随动属机械配置面；由 `git diff` 审查 + 版本纪律核验（`tauri.conf.json` 经 `"version": "../package.json"` 跟随） |
| `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`（AC-8 / AC-13 的边登记） | 依赖行 + 用途注释为声明式配置；`agent-runtime → archi` 边的**行为面**由 `archi_tool_test` / `tools_test` 承载，边本身以源码审查核验 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 仅 `pub(crate) mod archi_tool;` 一行挂载，无行为面；可达性由 `archi_tool_test` 的编译期 `use` 与 `loop` 引用机械保证（漏挂载即编译失败） |
| `packages/desktop/src/types/generated/bindings.ts`（AC-20） | 生成物（禁手改）；零 diff 门禁由 `bindings:check` 与 `src/bindings/mod_test.rs` 的重导出断言承载，不在生成物内部设用例 |
| `packages/desktop/package.json`（AC-20 版本行） | 版本字符串无单测路径；由版本 diff 审查核验（0.5.32 → 0.6.0，`src-tauri/Cargo.toml` 版本行不随动） |
| AC-8 的「`workflow` ⇄ `archi` 互不依赖」与「零 spawn」 | 依赖方向 / 进程触点为静态结构事实，本仓无既有依赖方向断言测试可扩展（design 事实校准 3 同源）；由源码审查 + `grep` 自查承载（`crates/core/archi/src` 零 `std::process` / `tokio::process`；`workflow` 依赖清单不含 `archi`） |
| AC-9 的 `archi` / 前端零路径字面量扫描 | `foundation/src/layout/mod_test.rs` 的命名隔离扫描只覆盖 `src-tauri/src` 与 `packages/desktop/src`、**不覆盖 `crates/**`**（design 事实校准 3，MUST NOT 借机改该测试）；`archi` 的零 `architecture` / 零 `openspec` / 零 `config.json` 字面量由源码审查 + 自查 `grep` 承载。`is_archi_rel_path` 与派生函数的**语义**由 foundation 章节用例覆盖 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**`（AC-14 内置锁面） | design 明列**零改动面**（不经本变更清单）；「内置清单仍恰为工作区 `config.json` 一条、不含架构域路径」由既有 `protect/rules_test` 回归锚承载（零生产改动，不新增本变更的源文件映射）。若评审要求新增显式断言，落在既有 `rules_test` 的单条用例内，不新建测试文件 |
| AC-14 的「worktree 内 `write` / `edit` 可直改 `models/*.toml`」 | 写入本身由既有通用工具用例覆盖（零改动）；本变更只断言「内置锁面零命中」（见上一行）与「写坏后读取面返回结构错」（`archi_tool_test` / 命令面用例） |
| AC-18 的「源码零 watch / 零轮询」 | 静态扫描面（`grep` 自查，任务书阶段十七自查项）；行为面（三触发之外零取数）由 `use-archi-graph` / `architecture-view` 的假计时器用例覆盖 |
| AC-20 的 `bindings:check` / `vp check` / `knip` / `cargo clippy -D warnings` | 静态脚本与机械门禁由桌面端执行；本设计只做分支枚举与用例映射，不复制运行命令细节 |
| `openspec/changes/archi-graph-doc-agent/**`（proposal / design / tasks / specs / explore） | 文档面，无测试路径（`explore.md` 为只读输入） |
| 零改动能力（`desktop-app-shell` / `desktop-ipc-type-bindings` / `desktop-file-watch` / `desktop-workspace-config` / `desktop-workspace-store` / `desktop-checks-domain` / `desktop-test-execution` / `desktop-mutation-testing` / `desktop-release-pipeline` / `desktop-corpus-regression`） | proposal「零改动能力与理由」逐条明列；本变更不改其代码，沿用既有用例断言，不另设章节 |
