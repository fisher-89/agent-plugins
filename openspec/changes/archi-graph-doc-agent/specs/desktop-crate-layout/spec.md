# desktop-crate-layout Specification（delta: archi-graph-doc-agent）

## ADDED Requirements

### Requirement: core/archi crate 落地登记

既有「crate 三层分组与依赖规则」中「未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖」的预告 SHALL 由本条兑现为落地登记（该 requirement 的 crate 树列举与规则文本本身零改写，本条为其增补登记）：

- **落位**：`crates/core/archi`，crate 裸名 `archi`（workspace 内唯一）；承载模型解析、内存图、结构校验、文档关联、结构 diff、gate 写回纯函数与三面共用 DTO。
- **workspace 内依赖**：`archi → foundation`（架构域路径派生）**唯一**。`foundation` 仍零 workspace 内依赖。
- **两条新增入边**：`desktop-app → archi`（命令面薄包装）与 `agent-runtime → archi`（agent 工具执行体复用同一 DTO 投影）。后者的理由 SHALL 与本仓既有 `agent-runtime → checks`（glob 方言单点）同构：**同一投影的第二个消费方直接消费同源实现**，MUST NOT 各写一份 JSON 投影——否则「三方一致性」纪律（见 `desktop-archi-model`）在机制上失效。两条边 MUST 在 `Cargo.toml` 处有用途注释。
- **零依赖面**：`archi` MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `workflow`（`workflow` ⇄ `archi` 互不依赖为硬红线，双向均须机械可验）、MUST NOT 依赖 `orchestration`、MUST NOT 进程 spawn（零 `std::process` / 零 `tokio::process`）。传参面纯函数化（gate 写回不触文件系统、文件读写归命令层）。
- **外部依赖归类**：`serde` / `serde_json` / `specta`（DTO 三件）+ `toml` / `toml_edit`（格式与渲染）+ `petgraph`（图算法）+ `ignore`（目录遍历，自动尊重 gitignore）；版本 SHALL 取自 workspace 锁内既有版本（零新增包）。
- **边界分类学不增类**：架构模型文件读面归既有「文件边界」的 workspace 文件读（经 core 纯读域），不在七类边界之外新增类别；`archi` MUST NOT 引入任何外部进程或 shell 触点。
- **消费者登记**：`foundation::layout` 的架构域派生函数（`archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`）以本 crate 为第二个消费者而落地，登记见 `workspace-layout-resolution`「架构域路径派生（archi_dir 族）」。
- **配置出口纪律不变**：`archi` MUST NOT 依赖 `config` crate、MUST NOT 读取工作区 `config.json`（架构域配置是另一个文件，且本期不产出其读取面）；`desktop-crate-layout` 既有「工作区配置唯一出口」结论零变化。

#### Scenario: 依赖方向与红线

- **WHEN** 审查 `crates/core/archi/Cargo.toml` 与 `crates/infra/agent/Cargo.toml`
- **THEN** `archi` 的 workspace 内依赖恰为 `foundation`，无 Tauri / 无 `store` / 无 `workflow` / 无 `orchestration`；`agent-runtime` 含 `archi = { workspace = true }` 且带用途注释；`workflow` 的依赖中不出现 `archi`

#### Scenario: foundation 不变量守住

- **WHEN** 审查 `crates/core/foundation/Cargo.toml` 与既有「foundation 保持极小」断言
- **THEN** 依赖面与变更前一致（无新增 workspace 内依赖、无新增外部依赖），新增内容仅为 layout 常量组与四个派生 / 判定函数

#### Scenario: 零 spawn 与边界归类

- **WHEN** 审查 `archi` 的进程触点与边界归类
- **THEN** 无 `std::process` / `tokio::process` / shell 调用；边界归既有「文件边界」读面，七类边界分类学零增类，`infra/` 目录不新增成员

#### Scenario: 图层结论全量不变

- **WHEN** 复核既有「core 零 Tauri」「store 不依赖 core 行为」「config 仅依赖 foundation」三条断言
- **THEN** 全部原样成立；新增依赖边只发生在 `desktop-app → archi`（壳 → core）与 `agent-runtime → archi`（infra → core 纯层）两个既有合法方向

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi`（新，裸名 `archi`） | 架构模型纯读域 + 纯函数写渲染 | workspace 内依赖仅 `foundation`；外部依赖 serde 系 + `specta` + `toml` / `toml_edit` / `petgraph` / `ignore`（锁内既有版本）；零 Tauri / 零 store / 零 spawn / 零 `workflow` |
| `crates/infra/agent`（依赖登记） | 工具执行体复用同一投影 | `archi = { workspace = true }` + 用途注释；理由与 `agent-runtime → checks`（glob 方言单点）同构 |
| `crates/desktop-app`（依赖登记） | 命令面薄包装 | `archi = { workspace = true }`；命令层零领域规则（投影全在 `archi::dto`） |
| `crates/core/foundation`（消费者登记） | 架构域路径派生 | 四个函数以 `archi` 为第二个消费者落地；`foundation` 依赖面零变化 |
