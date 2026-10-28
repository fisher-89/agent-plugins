# workspace-layout-resolution Specification（delta: archi-check-and-gate）

## MODIFIED Requirements

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `foundation::layout::archi_config_path` | 看护配置路径单点 | `archi_config_path(root: &Path) -> PathBuf` = `<archi_dir>/config.json`；从 `archi_dir` 派生、文件名复用既有常量；纯推导零 IO |
| `crates/core/archi/src/drift/config.rs`（消费点） | 配置读取面 | 只经 `archi_config_path` 定位文件；缺失即空配置；`deny_unknown_fields` + reason 必填 + FQN 存在性校验 |
| `foundation::layout::change_reports`（槽位复用） | 报告目录派生 | `ReportSlot::Archi` 经既有单点派生 `reports/archi`（槽位三值见 desktop-checks-domain）；`Layout` 三字段与 `resolve` 零变化 |
