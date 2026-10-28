# workspace-layout-resolution Specification

## Purpose

定义 foundation crate 对用户选定 workspace 根目录的布局解析契约：`resolve` 纯函数返回 `Layout` 结构，作为整个 desktop 包内唯一知晓磁盘目录名的位置，支撑 workspace 语义通用化（可读取任意项目根下的 change 记录）。

## Requirements

### Requirement: resolve 函数返回 Layout 结构

foundation crate SHALL 提供纯函数 `resolve(root: &Path) -> Layout`，将用户选定的 workspace 根目录解析为布局结构：

```
Layout {
  changes_root:  PathBuf   # 进行中 change 目录树
  archive_root:  PathBuf   # 已归档 change 目录树
  explores_root: PathBuf   # 探索笔记目录树
}
```

layout/mod.rs SHALL 在模块顶部设**目录名常量组**：域目录名（`DOMAIN_DIR_NAME` = `openspec`）、`changes` / `archive` / `explores` 子目录名、架构域顶层目录名（`ARCHI_ROOT_DIR_NAME` = `.dev-team`）、架构域段名（`architecture`）、`models` / `decisions` 子目录名、`config.json` 文件名。架构域顶层段 `.dev-team` 与架构域段 `architecture`（及其中 `models` / `decisions` 子目录）SHALL 只经本模块的派生函数暴露（见「架构域路径派生」requirement）；架构域 SHALL 与 `openspec` 域根**分树**（顶层隐藏目录 `.dev-team`），MUST NOT 嵌套于域根之下。`Layout` 三字段（`changes_root` / `archive_root` / `explores_root`）SHALL NOT 因架构域新增而增字段或改语义——架构域是第四条独立派生面，不属于 change 树。`resolve` SHALL 全量引用常量组装路径（MUST NOT 行内字面量）；`domain_dir_name` 的 fn 形态 SHALL 收敛为常量（公开形态 design 定夺：导出 `pub const` 或保留 fn 包装），既有消费点（stats 轨道的 tokei `ignored_directories` 通道）随之更新、语义零变化。

`resolve` 与常量组 SHALL 是整个 desktop 包内唯一知晓磁盘目录名、子目录名与 config.json 文件名的位置（字面量隔离执法见 desktop-crate-layout：`openspec` 与 `config.json` 双禁令）；目录或子目录未来改名 SHALL 只需修改 layout/mod.rs 常量组。`resolve` MUST NOT 访问文件系统（纯路径推导），对不存在的 root 亦 SHALL 正常返回 Layout（存在性检查交由上层查询处理）。

#### Scenario: 从任意根解析布局

- **WHEN** 以某个包含 change 数据的项目根目录调用 `resolve(root)`
- **THEN** 返回的 `Layout` 三字段分别指向该根下进行中、归档、探索笔记三个目录树

#### Scenario: 改名只动常量组

- **WHEN** 假设磁盘目录 `openspec/` 或任一子目录 / config.json 文件名更名
- **THEN** 仅需修改 layout/mod.rs 常量组，model / parse / queries / config / desktop-app 与前端零改动

#### Scenario: resolve 全量引用常量

- **WHEN** 审查 `resolve` 实现与 `mod_test` 断言
- **THEN** 路径组装全部经常量引用，无常量组之外的行内目录名 / 文件名字面量；常量组覆盖域目录名、三个域根子目录名（`changes` / `archive` / `explores`）、架构域顶层目录名与 `architecture` 段名、`models` / `decisions` 子目录名与 config.json 文件名

#### Scenario: 架构域不改 Layout 形状

- **WHEN** 审查 `Layout` 定义与 `resolve` 的既有消费点
- **THEN** `Layout` 仍恰为三字段、语义逐字不变；架构域路径只经派生函数取得（见「架构域路径派生」），MUST NOT 出现第四字段或第二份架构域路径拼接

#### Scenario: 对不存在的根仍可解析

- **WHEN** 以一个不存在的路径调用 `resolve`
- **THEN** 不报错，正常返回 Layout 结构；目录缺失由查询层按空结果处理

### Requirement: workspace 语义通用化

App SHALL 支持读取任意用户选定的项目根目录下的 change 记录，MUST NOT 将路径绑定到本仓库。

#### Scenario: 切换 workspace 根

- **WHEN** 用户通过文件夹选择器选定另一个项目根目录
- **THEN** 列表与详情取数均以新根为基准重新扫描

### Requirement: config_path 提供 config.json 路径解析

foundation crate SHALL 提供 config.json 路径解析能力：给定 workspace 根返回 `<root>/<域目录名>/config.json` 路径（倾向形态为独立函数 `config_path(root: &Path) -> PathBuf`——config.json 是文件而非目录树，`Layout` 加第四字段不贴切；最终形态 design 定夺并记录理由）。路径 SHALL 全量引用目录名常量组，MUST NOT 行内字面量；解析 SHALL 为纯路径推导、无任何文件系统访问（文件存在性交由 `config` crate 读取层处理）；该函数 SHALL 是 `config` crate 获取配置文件路径的唯一通道。

#### Scenario: 路径解析正确

- **WHEN** 以某 workspace 根调用 config.json 路径解析
- **THEN** 返回路径为根目录拼接域目录名常量再拼接 config.json 文件名常量的结果，与 `resolve` 的域目录推导同源

#### Scenario: 纯推导无 IO

- **WHEN** 以不存在的 root 调用 config.json 路径解析
- **THEN** 正常返回拼接路径，无文件系统访问、不报错

#### Scenario: 唯一通道

- **WHEN** 审查 `config` crate 的取路径方式
- **THEN** config.json 路径仅来自 foundation 的该解析能力，crate 内无第二处路径拼接、无文件名字面量

### Requirement: 架构域路径派生（archi_dir 族）

foundation crate SHALL 提供架构域的路径派生与判定函数（纯路径推导、零文件系统访问，对不存在的 root 亦正常返回）：

```
archi_dir(root)            → <root>/.dev-team/architecture  （架构域顶层单点，与域根分树）
archi_models_dir(root)     → <archi_dir>/models
archi_decisions_dir(root)  → <archi_dir>/decisions
archi_config_path(root)    → <archi_dir>/config.json      （架构域看护配置单点）
is_archi_rel_path(rel)     → rel 是否落在 <archi_dir> 之内（repo 相对路径判定）
```

- 全部路径组件 SHALL 取自 layout/mod.rs 的目录名常量组，MUST NOT 行内字面量；架构域顶层段（`.dev-team`）与 `architecture` 段 SHALL 只由 `archi_dir` 单点持有，其余四项（含 `archi_config_path`）全从它派生。
- `is_archi_rel_path` SHALL 接受 `/` 分隔的 repo 相对路径，MUST 同时对 `\` 分隔形态归一后判定（Windows 来源清单），并明确区分祖先判定（`.dev-team/architecture` 与 `.dev-team/architecture/models/x.toml` 均命中，`openspec/architecture/...` 与 `.dev-team/archived/...` 不命中）。
- 该判定函数 SHALL 为归档冲突分流（`desktop-change-archive`「worktree 提交与主仓合入」）的唯一合法判据——core 侧 MUST NOT 出现 `architecture` 字面量比较。
- **`archi_config_path`（本变更新增）**：`archi_config_path(root)` → `<archi_dir>/config.json`，SHALL 从 `archi_dir` 派生（`architecture` 段与 `config.json` 文件名均取自 layout 常量组，`config.json` 文件名常量沿用既有 `CONFIG_FILE_NAME` —— MUST NOT 新增第二个文件名字面量）。它 SHALL 是架构域看护配置（豁免元素对 / 扫描排除 glob，见 `desktop-archi-drift`「架构域看护配置」）的**唯一合法定位函数**——消费者（`crates/core/archi` 的配置读取面）MUST NOT 行内拼 `config.json`，产品 `.rs`（`*_test.rs` 除）的 `config.json` 字面量隔离纪律零变化。工作区配置仍只经 `config_path(root)`（`desktop-workspace-config` 唯一出口）。
- 本族函数的落地 SHALL 在 `desktop-crate-layout` 的依赖与消费者登记中同步在案（`archi` crate 为路径派生函数的消费者；本变更新增 `archi_config_path` 的消费者登记）。

#### Scenario: 路径派生正确且同源

- **WHEN** 以某 workspace 根分别调用三个派生函数
- **THEN** 结果依次为 `<root>/.dev-team/architecture`、`.../architecture/models`、`.../architecture/decisions`；三者共享同一 `architecture` 段来源（改名只动常量组）
- **AND** 架构域根不以 `openspec` 域根为前缀（两树分居，常量组零复用）

#### Scenario: 相对路径判定正负样本

- **WHEN** 分别以 `.dev-team/architecture/models/02-auth.toml`、`.dev-team/architecture`、`.dev-team\architecture\models\x.toml`、`openspec/changes/x/specs/y.md`、`openspec/architecture/models/x.toml` 调用 `is_archi_rel_path`
- **THEN** 前三者命中（含反斜杠归一），后两者不命中（旧落点已与架构域分树）

#### Scenario: 纯推导无 IO 且配置路径单点

- **WHEN** 以不存在的 root 调用四个派生函数，并审查 `crates/core/archi` 的配置读取面与产品源码字面量
- **THEN** 路径照常返回、无文件系统访问；`archi_config_path(root)` 为 `<root>/.dev-team/architecture/config.json`；源码无行内 `config.json` 拼接（唯一触点仍是 `foundation/src/layout/mod.rs`）

#### Scenario: 与工作区配置路径互不遮蔽

- **WHEN** 以同一 root 分别调用 `config_path(root)` 与 `archi_config_path(root)`
- **THEN** 前者为 `<root>/openspec/config.json`、后者为 `<root>/.dev-team/architecture/config.json`，两路径无前缀关系；`Layout` 三字段与 `resolve` 语义零变化

### Requirement: 架构域 repo 相对路径派生（会话知识用）

foundation crate SHALL 提供架构域的 **repo 相对路径**派生函数（与服务绝对路径族并列，纯路径推导、零文件系统访问）：

```
archi_rel_dir()          → 架构域根的 repo 相对路径（`/` 分隔）
archi_models_rel_dir()   → <archi_rel_dir>/models
archi_decisions_rel_dir()→ <archi_rel_dir>/decisions
```

- 三函数 SHALL 全部从 layout/mod.rs 的目录名常量组派生，MUST NOT 行内字面量；`archi_models_rel_dir` / `archi_decisions_rel_dir` SHALL 从 `archi_rel_dir` 派生（架构域段名不二次出现）。
- 返回形态 SHALL 为 `String`（或等价的 `&'static str` 派生形态，取值形态由 design 定型），内容 MUST 为 `/` 分隔、无前导 `./`、无反斜杠——该形态直接用于模型可见文本（提示词 / 知识文本），MUST NOT 依赖调用方再归一。
- 唯一消费者 SHALL 为架构会话知识文本（`desktop-archi-agent`「架构会话的系统知识注入」），MUST NOT 在产品源码手写 `.dev-team/architecture` 一类相对路径字面量。
- 既有绝对路径族（`archi_dir` / `archi_models_dir` / `archi_decisions_dir`）、`is_archi_rel_path` 与 `Layout` 三字段 SHALL 语义零变化；`archi_config_path` 继续不在射程内（不因本变更预建）。

#### Scenario: 三函数同源且前缀一致

- **WHEN** 分别调用 `archi_rel_dir()` / `archi_models_rel_dir()` / `archi_decisions_rel_dir()`
- **THEN** 结果依次为架构域根、`<根>/models`、`<根>/decisions`（后两者以第一者为前缀），全部 `/` 分隔、零反斜杠、零前导 `./`

#### Scenario: 与绝对路径族尾部一致

- **WHEN** 以某 workspace 根调用绝对族（`archi_models_dir(root)`）并与 `archi_models_rel_dir()` 比对
- **THEN** 绝对路径剥去 `root` 前缀后的相对段与相对族返回值逐字相等（同源可验：改名只动常量组，两侧同时变化）

#### Scenario: 零 IO 与零字面量

- **WHEN** 审查实现与产品源码字面量
- **THEN** 三函数无任何文件系统访问（对不存在的 root 无关，因为不接 root 入参）；产品 `.rs`（`*_test.rs` 除）无架构域顶层段 `.dev-team` 与 `architecture` 段字面量（唯一触点仍为 `foundation/src/layout/mod.rs`；壳层 `env` 的数据根常量 `.dev-team` 指向 `<home>` 侧，同名不同物，不构成架构域路径字面量）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `foundation::layout` | 磁盘布局解析 + 目录名常量组 | `resolve(root: &Path) -> Layout`；常量组覆盖域目录名 / `changes` / `archive` / `explores` / `config.json`；`config_path`（形态 design 定夺）同源引用常量；纯路径推导、无 IO；全包唯一字面量触点（`openspec` + `config.json` 双禁令） |
| `domain_dir_name`（fn → 常量形态） | 域目录名裸名消费通道 | tokei `ignored_directories` 等裸名语义消费方经此取字面量；公开形态（`pub const` vs fn 包装）design 定夺，消费点语义零变化 |
| `dev-team`（workspace 状态） | 当前 workspace 持有 | 持有选定的根目录 |
| `foundation::layout`（常量组扩展，架构域段） | 架构域段名单点 | 新增架构域顶层目录名 `.dev-team`、`architecture` 段名与 `models` / `decisions` 子目录名常量；架构域与 `openspec` 域根分树；`Layout` 三字段与 `resolve` 语义零变化 |
| `foundation::layout::archi_dir` | 架构域根派生单点 | `<root>/.dev-team/architecture`；架构域顶层段与架构域段唯一持有者 |
| `foundation::layout::archi_models_dir` / `archi_decisions_dir` | 子目录派生 | 全从 `archi_dir` 派生；纯推导无 IO |
| `foundation::layout::is_archi_rel_path` | 相对路径判定 | 反斜杠归一 + 祖先判定；core 侧归档冲突分流的唯一判据（零 `architecture` 字面量比较） |
| `foundation::layout::archi_rel_dir` | 架构域根相对路径 | 由目录名常量组派生；`/` 分隔、零前导 `./`；零 IO |
| `foundation::layout::archi_models_rel_dir` / `archi_decisions_rel_dir` | 子目录相对路径 | 全从 `archi_rel_dir` 派生；供架构会话知识文本注入（唯一消费者） |
| `crates/infra/agent/src/archi_knowledge.rs`（消费点，见 desktop-archi-agent） | 知识拼装 | 以三函数注入目录路径，MUST NOT 手写相对路径字面量 |
| `foundation::layout::archi_config_path` | 看护配置路径单点 | `archi_config_path(root: &Path) -> PathBuf` = `<archi_dir>/config.json`；从 `archi_dir` 派生、文件名复用既有常量；纯推导零 IO |
| `crates/core/archi/src/drift/config.rs`（消费点） | 配置读取面 | 只经 `archi_config_path` 定位文件；缺失即空配置；`deny_unknown_fields` + reason 必填 + FQN 存在性校验 |
| `foundation::layout::change_reports`（槽位复用） | 报告目录派生 | `ReportSlot::Archi` 经既有单点派生 `reports/archi`（槽位三值见 desktop-checks-domain）；`Layout` 三字段与 `resolve` 零变化 |
