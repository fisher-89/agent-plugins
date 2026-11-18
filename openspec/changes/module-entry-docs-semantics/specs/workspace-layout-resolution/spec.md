# workspace-layout-resolution Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**派生族换段**（E-1 目录迁移的机械面）：契约资产由独立的 `api` 平面搬进模型目录 `models/`，契约索引换家到新建的 `graph/` 段。`archi_dir` / `archi_config_path` / `is_archi_rel_path` / `Layout` 三字段与 `resolve` 语义**逐字不动**；`archi_models_dir` / `archi_models_rel_dir` 零改动（它们被契约面**追加**为消费者）。

## MODIFIED Requirements

### Requirement: 架构域路径派生（archi_dir 族）

foundation crate SHALL 提供架构域的路径派生与判定函数（纯路径推导、零文件系统访问，对不存在的 root 亦正常返回）：

```
archi_dir(root)            → <root>/.dev-team/architecture  （架构域顶层单点，与域根分树）
archi_models_dir(root)     → <archi_dir>/models
archi_decisions_dir(root)  → <archi_dir>/decisions
archi_graph_dir(root)      → <archi_dir>/graph             （契约索引单点）
archi_config_path(root)    → <archi_dir>/config.json      （架构域看护配置单点）
is_archi_rel_path(rel)     → rel 是否落在 <archi_dir> 之内（repo 相对路径判定）
```

- 全部路径组件 SHALL 取自 layout/mod.rs 的目录名常量组，MUST NOT 行内字面量；架构域顶层段（`.dev-team`）与 `architecture` 段 SHALL 只由 `archi_dir` 单点持有，其余各项（含 `archi_config_path` 与 `archi_graph_dir`）全从它派生。
- `is_archi_rel_path` SHALL 接受 `/` 分隔的 repo 相对路径，MUST 同时对 `\` 分隔形态归一后判定（Windows 来源清单），并明确区分祖先判定（`.dev-team/architecture` 与 `.dev-team/architecture/models/x.toml` 均命中，`openspec/architecture/...` 与 `.dev-team/archived/...` 不命中）。
- 该判定函数 SHALL 为归档冲突分流（`desktop-change-archive`「worktree 提交与主仓合入」）的唯一合法判据——core 侧 MUST NOT 出现 `architecture` 字面量比较。
- **`archi_config_path`**：`archi_config_path(root)` → `<archi_dir>/config.json`，SHALL 从 `archi_dir` 派生（`architecture` 段与 `config.json` 文件名均取自 layout 常量组，`config.json` 文件名常量沿用既有 `CONFIG_FILE_NAME` —— MUST NOT 新增第二个文件名字面量）。它 SHALL 是架构域看护配置（豁免元素对 / 扫描排除 glob，见 `desktop-archi-drift`「架构域看护配置」）的**唯一合法定位函数**——消费者（`crates/core/archi` 的配置读取面）MUST NOT 行内拼 `config.json`，产品 `.rs`（`*_test.rs` 除）的 `config.json` 字面量隔离纪律零变化。工作区配置仍只经 `config_path(root)`（`desktop-workspace-config` 唯一出口）。
- **`archi_graph_dir`（本变更新增）**：`archi_graph_dir(root)` → `<archi_dir>/graph`，SHALL 从 `archi_dir` 派生（`graph` 段取自 layout 常量组新增的 `GRAPH_DIR_NAME`）。它 SHALL 是契约索引 `INDEX.toml`（`desktop-archi-api`「schema.json 与 INDEX.toml 为纯派生且不可手改」）的**唯一合法定位函数**——消费者（`crates/core/archi` 的契约面、`crates/tools/archi-api-gen`、`crates/infra/archi` 的写盘单点）MUST NOT 行内拼 `graph` 段。它是「契约索引的家」：本变更内该目录**恰住 `INDEX.toml` 一份**；将来若容纳其他图派生资产（模型快照 / 图导出），函数名不变、索引路径退化为本目录下的一层（属独立变更，MUST NOT 在本变更内预建）。
- **`api` 段一族退役（本变更删除）**：`API_DIR_NAME` / `archi_api_dir(root)` / `archi_api_rel_dir()` SHALL 全部删除，产品 `.rs`（`*_test.rs` 除）内 `archi_api_dir` / `archi_api_rel_dir` / `API_DIR_NAME` 零命中、`"api"` 段字面量零命中。**契约资产目录不新增派生函数**——它 SHALL 直接取既有 `archi_models_dir(root)`（契约 md 与 `<Domain>/` 子目录在模型目录内，见 `desktop-archi-api`「契约资产形态与唯一手写真相」）；「一个目录两个名字」（`archi_api_dir` 改指 models）MUST NOT 出现，那会让目录语义与函数名永久错位。
- 本族函数的落地 SHALL 在 `desktop-crate-layout` 的依赖与消费者登记中同步在案（`archi` crate 为路径派生函数的消费者；`archi_config_path` 与 `archi_graph_dir` 的消费者亦在案）。

#### Scenario: 路径派生正确且同源

- **WHEN** 以某 workspace 根分别调用三个派生函数
- **THEN** 结果依次为 `<root>/.dev-team/architecture`、`.../architecture/models`、`.../architecture/decisions`；三者共享同一 `architecture` 段来源（改名只动常量组）
- **AND** 架构域根不以 `openspec` 域根为前缀（两树分居，常量组零复用）

#### Scenario: 相对路径判定正负样本

- **WHEN** 分别以 `.dev-team/architecture/models/02-auth.toml`、`.dev-team/architecture`、`.dev-team\architecture\models\x.toml`、`openspec/changes/x/specs/y.md`、`openspec/architecture/models/x.toml` 调用 `is_archi_rel_path`
- **THEN** 前三者命中（含反斜杠归一），后两者不命中（旧落点已与架构域分树）

#### Scenario: 纯推导无 IO 且配置路径单点

- **WHEN** 以不存在的 root 调用五个派生函数，并审查 `crates/core/archi` 的配置读取面 / 契约面与产品源码字面量
- **THEN** 路径照常返回、无文件系统访问；`archi_config_path(root)` 为 `<root>/.dev-team/architecture/config.json`、`archi_graph_dir(root)` 为 `<root>/.dev-team/architecture/graph`；源码无行内 `config.json` / `graph` / `models` 段拼接（唯一触点仍是 `foundation/src/layout/mod.rs`）

#### Scenario: 与工作区配置路径互不遮蔽

- **WHEN** 以同一 root 分别调用 `config_path(root)` 与 `archi_config_path(root)`
- **THEN** 前者为 `<root>/.dev-team/config.json`、后者为 `<root>/.dev-team/architecture/config.json`，两路径无前缀关系；`Layout` 三字段与 `resolve` 语义零变化

#### Scenario: archi_graph_dir 与其他派生互不遮蔽

- **WHEN** 以同一 root 分别调用 `archi_graph_dir(root)` 与 `archi_models_dir(root)` / `archi_decisions_dir(root)` / `archi_config_path(root)`
- **THEN** 依次为 `<root>/.dev-team/architecture/graph`、`.../models`、`.../decisions`、`.../config.json`；四者互不为前缀、共享同一 `architecture` 段来源（改名只动常量组）；`archi_graph_dir` 纯推导零 IO（不存在的 root 照常返回）
- **AND** 契约资产目录 == `archi_models_dir(root)`（同值、零新增函数）；`archi_api_dir` / `archi_api_rel_dir` / `API_DIR_NAME` 三符号在源码内零命中（已删）

### Requirement: 架构域 repo 相对路径派生（会话知识用）

foundation crate SHALL 提供架构域的 **repo 相对路径**派生函数（与服务绝对路径族并列，纯路径推导、零文件系统访问）：

```
archi_rel_dir()          → 架构域根的 repo 相对路径（`/` 分隔）
archi_models_rel_dir()   → <archi_rel_dir>/models
archi_decisions_rel_dir()→ <archi_rel_dir>/decisions
archi_graph_rel_dir()    → <archi_rel_dir>/graph
```

- 各函数 SHALL 全部从 layout/mod.rs 的目录名常量组派生，MUST NOT 行内字面量；`archi_models_rel_dir` / `archi_decisions_rel_dir` / `archi_graph_rel_dir` SHALL 从 `archi_rel_dir` 派生（架构域段名不二次出现）。
- 返回形态 SHALL 为 `String`（或等价的 `&'static str` 派生形态，取值形态由 design 定型），内容 MUST 为 `/` 分隔、无前导 `./`、无反斜杠——该形态直接用于模型可见文本（提示词 / 知识文本），MUST NOT 依赖调用方再归一。
- 消费者 SHALL 限于三处：架构会话知识文本（`desktop-archi-agent`「架构会话的系统知识注入」——`<archi_models_rel_dir>` 与 `<archi_graph_rel_dir>` 两个方位）、契约索引 `INDEX.toml` 的说明段（`desktop-archi-api`「schema.json 与 INDEX.toml 为纯派生且不可手改」——`<archi_models_rel_dir>` 作「契约面目录」）、与 `desktop-crate-layout` 的段名纪律登记。MUST NOT 在产品源码手写 `.dev-team/architecture` 一类相对路径字面量。
- 既有绝对路径族（`archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `archi_config_path` / `archi_graph_dir`）、`is_archi_rel_path` 与 `Layout` 三字段 SHALL 语义零变化；`archi_api_rel_dir()` SHALL 删除（`api` 段的相对形态随段名退役，MUST NOT 保留兼容别名）。

#### Scenario: 三函数同源且前缀一致

- **WHEN** 分别调用 `archi_rel_dir()` / `archi_models_rel_dir()` / `archi_decisions_rel_dir()`
- **THEN** 结果依次为架构域根、`<根>/models`、`<根>/decisions`（后两者以第一者为前缀），全部 `/` 分隔、零反斜杠、零前导 `./`

#### Scenario: 与绝对路径族尾部一致

- **WHEN** 以某 workspace 根调用绝对族（`archi_models_dir(root)`）并与 `archi_models_rel_dir()` 比对
- **THEN** 绝对路径剥去 `root` 前缀后的相对段与相对族返回值逐字相等（同源可验：改名只动常量组，两侧同时变化）

#### Scenario: 零 IO 与零字面量

- **WHEN** 审查实现与产品源码字面量
- **THEN** 各函数无任何文件系统访问（对不存在的 root 无关，因为不接 root 入参）；产品 `.rs`（`*_test.rs` 除）无架构域顶层段 `.dev-team` 与 `architecture` 段字面量（唯一触点仍为 `foundation/src/layout/mod.rs`；壳层 `env` 的数据根常量 `.dev-team` 指向 `<home>` 侧，同名不同物，不构成架构域路径字面量）

#### Scenario: archi_graph_rel_dir 同源且前缀一致

- **WHEN** 分别调用 `archi_rel_dir()` / `archi_graph_rel_dir()` 与 `archi_graph_dir(root)`
- **THEN** 前者以 `archi_rel_dir()` 为前缀、`/` 分隔零反斜杠零前导 `./`；`archi_graph_dir(root)` 剥去 `root` 前缀后的相对段与 `archi_graph_rel_dir()` 逐字相等（同源可验：改名只动常量组）

## Module Contract

### Function: `foundation::layout`（`crates/core/foundation/src/layout/mod.rs`）

| 符号 | 变化 | 描述 |
|---|---|---|
| `foundation::layout::archi_graph_dir`（新增） | 新增 | `archi_graph_dir(root: &Path) -> PathBuf` = `<archi_dir>/graph`；从 `archi_dir` 派生、段名取常量 `GRAPH_DIR_NAME`；纯推导零 IO；契约索引 `INDEX.toml` 的唯一定位入口 |
| `foundation::layout::archi_graph_rel_dir`（新增） | 新增 | `archi_graph_rel_dir() -> String` = `<archi_rel_dir>/graph`；`/` 分隔、零前导 `./`；消费者限于知识文本与段名纪律登记 |
| `foundation::layout::archi_api_dir` / `archi_api_rel_dir` / `API_DIR_NAME` | **删除** | `api` 段退役；契约资产目录改取既有 `archi_models_dir`，MUST NOT 保留改指型别名；产品 `.rs` 三符号零命中 |
| `foundation::layout::archi_models_dir` / `archi_models_rel_dir` | 语义零变化、消费者扩容 | 追加契约面（`crates/core/archi` 的路径单点、`crates/tools/archi-api-gen`、`crates/infra/archi` 的写盘单点）与 `INDEX.toml` 说明段为消费者；函数体与返回值逐字不变 |
| `foundation::layout`（常量组） | 增删同批 | 新增 `GRAPH_DIR_NAME = "graph"`；删除 `API_DIR_NAME`；`archi_dir` 族其余函数语义零变化、`Layout` 三字段零变化 |
