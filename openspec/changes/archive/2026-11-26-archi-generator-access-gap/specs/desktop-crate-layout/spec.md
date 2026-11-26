# desktop-crate-layout Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 在 `core/archi` 的落位登记里增补**一个纯端口**，并把「实现由谁注入」写清——两条既有依赖结论（`agent-runtime ↛ archi-runtime`、`agent-runtime → archi`）逐字不变：

- **端口落 core**：`ArtifactWriter` 是纯类型 trait（签名只用既有 `GeneratedFile` / `ArchiApiWriteOutcome`），零 IO、零写盘、零 spawn ⇒ 放在 `core/archi` 不破坏「core 零写盘」纪律。
- **实现在壳层注入**：唯一实现是 `src-tauri` 的 `ArchiArtifactWriter`（薄委托 `archi_runtime::api_artifacts::write_artifacts`），由组合根经 `compose_turn` 注入到 agent 工具执行体。
- **写盘单点不变**：`write_artifacts` 仍是 `infra/archi` 的写盘单点，本变更只多一个**调用方**（agent 工具），不新增写盘实现、不新增 crate、不新增依赖边。
- **agent 面仍不可达 `archi-runtime`**：`agent-runtime` 的依赖面零改动；它消费的是 `core/archi` 的端口类型，实现从上方（壳）流下来。

## MODIFIED Requirements

### Requirement: core/archi crate 落地登记

既有「crate 三层分组与依赖规则」中「未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖」的预告 SHALL 由本条兑现为落地登记（该 requirement 的 crate 树列举与规则文本本身零改写，本条为其增补登记）：

- **落位**：`crates/core/archi`，crate 裸名 `archi`（workspace 内唯一）；承载模型解析、内存图、结构校验、文档关联、结构 diff、gate 写回纯函数与三面共用 DTO。
- **workspace 内依赖**：`archi → foundation`（架构域路径派生）**唯一**。`foundation` 仍零 workspace 内依赖。
- **两条新增入边**：`desktop-app → archi`（命令面薄包装）与 `agent-runtime → archi`（agent 工具执行体复用同一 DTO 投影）。后者的理由 SHALL 与本仓既有 `agent-runtime → checks`（glob 方言单点）同构：**同一投影的第二个消费方直接消费同源实现**，MUST NOT 各写一份 JSON 投影——否则「三方一致性」纪律（见 `desktop-archi-model`）在机制上失效。两条边 MUST 在 `Cargo.toml` 处有用途注释。
- **生成物写盘端口（本变更新增）**：`archi` SHALL 定义纯端口 `ArtifactWriter`——`fn write(&self, root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`，`Send + Sync`。三条边界：① 它是**纯类型**，`archi` crate 仍全程零写盘（零 `fs::write` / 零 `create_dir_all`）与零 spawn，MUST NOT 在 core 内出现任何实现；② 唯一实现 SHALL 落在壳层（`src-tauri`，委托 `archi-runtime` 的写盘单点），由组合根经 `compose_turn` 注入 agent 工具执行体；③ 该端口 MUST NOT 被读成新依赖边——`agent-runtime` 依赖面零新增（它消费的是 `core/archi` 的类型），`agent-runtime` 的依赖中仍不出现 `archi-runtime`（写盘单点对 agent 面仍不可达，见「infra/archi crate 落地登记与 archi 依赖扩展」）。
- **零依赖面**：`archi` MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `workflow`（`workflow` ⇄ `archi` 互不依赖为硬红线，双向均须机械可验）、MUST NOT 依赖 `orchestration`、MUST NOT 进程 spawn（零 `std::process` / 零 `tokio::process`）。传参面纯函数化（gate 写回不触文件系统、文件读写归命令层）。
- **外部依赖归类**：`serde` / `serde_json` / `specta`（DTO 三件）+ `toml` / `toml_edit`（格式与渲染）+ `petgraph`（图算法）+ `ignore`（目录遍历，自动尊重 gitignore）；版本 SHALL 取自 workspace 锁内既有版本（零新增包）。端口不引入任何新外部依赖。
- **边界分类学不增类**：架构模型文件读面归既有「文件边界」的 workspace 文件读（经 core 纯读域），不在七类边界之外新增类别；`archi` MUST NOT 引入任何外部进程或 shell 触点。
- **消费者登记**：`foundation::layout` 的架构域派生函数（`archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`）以本 crate 为第二个消费者而落地，登记见 `workspace-layout-resolution`「架构域路径派生（archi_dir 族）」。
- **配置出口纪律不变**：`archi` MUST NOT 依赖 `config` crate、MUST NOT 读取工作区 `config.json`（架构域配置是另一个文件，且本期不产出其读取面）；`desktop-crate-layout` 既有「工作区配置唯一出口」结论零变化。

#### Scenario: 依赖方向与红线

- **WHEN** 审查 `crates/core/archi/Cargo.toml` 与 `crates/infra/agent/Cargo.toml`
- **THEN** `archi` 的 workspace 内依赖恰为 `foundation`，无 Tauri / 无 `store` / 无 `workflow` / 无 `orchestration`；`agent-runtime` 含 `archi = { workspace = true }` 且带用途注释、**不含** `archi-runtime`；`workflow` 的依赖中不出现 `archi`；端口落地后三个清单逐字不变（零新依赖边）

#### Scenario: 端口是纯类型且实现不在 core

- **WHEN** 审查 `crates/core/archi/src/api/mod.rs`（或 `lib.rs`）的 `ArtifactWriter` 与 `crates/core/archi/**` 全文
- **THEN** trait 只声明 `write` 一个方法、签名只用既有 `GeneratedFile` / `ArchiApiWriteOutcome`；`archi` crate 对 `fs::write` / `create_dir_all` / `File::create` / `std::process` 零命中（零实现、零 IO）；唯一实现出现在壳层且函数体只调 `archi_runtime::api_artifacts::write_artifacts` 一次

#### Scenario: foundation 不变量守住

- **WHEN** 审查 `crates/core/foundation/Cargo.toml` 与既有「foundation 保持极小」断言
- **THEN** 依赖面与变更前一致（无新增 workspace 内依赖、无新增外部依赖），新增内容仅为 layout 常量组与四个派生 / 判定函数；端口落地后同样零新增（端口只增类型，不增依赖）

#### Scenario: 零 spawn 与边界归类

- **WHEN** 审查 `archi` 的进程触点与边界归类
- **THEN** 无 `std::process` / `tokio::process` / shell 调用；边界归既有「文件边界」读面，七类边界分类学零增类，`infra/` 目录不新增成员

#### Scenario: 图层结论全量不变

- **WHEN** 复核既有「core 零 Tauri」「store 不依赖 core 行为」「config 仅依赖 foundation」三条断言
- **THEN** 全部原样成立；新增依赖边只发生在 `desktop-app → archi`（壳 → core）与 `agent-runtime → archi`（infra → core 纯层）两个既有合法方向；端口落地后依赖边集合与变更前逐条相同（只增类型，不增边）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: `crates/core/archi`（端口登记）

| 项 | 变化 | 描述 |
|---|---|---|
| `api/mod.rs` | 新增 | `pub trait ArtifactWriter: Send + Sync`（一方法 `write`）；纯类型、零 IO、零写盘、零 spawn |
| `lib.rs` | 改 | 出口随动（`ArtifactWriter` 再导出） |
| `Cargo.toml` | 零改动 | workspace 内依赖仍仅 `foundation`；外部依赖零新增 |

### Component: 实现落位与注入（壳层，登记于本能力以说明「边不变、实现在上方」）

| 项 | 描述 |
|---|---|
| `src-tauri/src/commands/archi/mod.rs` | `ArchiArtifactWriter` 实现 `archi::ArtifactWriter`，函数体只调 `archi_runtime::api_artifacts::write_artifacts`（唯一写盘单点）；零自有写盘逻辑 |
| `src-tauri/src/commands/{exec,change_flow}/mod.rs` | 构造 `Arc<ArchiArtifactWriter>` 并经 `compose_turn` 传入（组合根注入，零新 IPC 命令） |
| `crates/infra/archi/src/api_artifacts.rs` | 零改动（只增只改、从不删、非原子语义逐字不变）；本变更只多一个调用方 |
| `crates/infra/agent/Cargo.toml` | 零改动：仍 `archi = { workspace = true }`，仍无 `archi-runtime` |
