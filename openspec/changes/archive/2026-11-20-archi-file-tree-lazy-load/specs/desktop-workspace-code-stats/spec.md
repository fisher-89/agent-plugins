# desktop-workspace-code-stats Specification（delta: archi-file-tree-lazy-load）

## Purpose

本 delta 只改**一句话的口径**并补一条否定 scenario：本能力原写「命令形态沿用 sync 命令惯例（sync 命令不在主线程执行）」——后半句是事实错误。Tauri 同步命令落 `ExecutionContext::Blocking`，命令体在 IPC 线程（Windows = 主线程）内联执行，长任务会冻结 webview；这正是 `code_stats`（tokei 全仓解析）与 archi 四条取数命令同病的原因。

- **零代码改动**：`commands/stats/` 与 `use-code-stats` 逐字不变，命令仍是 `pub fn`。
- **债务归位**：`code_stats` 的 async 化登记在 `desktop-app-shell`「命令执行线程纪律」的登记债务里，不在此处另立判据。
- **其余全部不动**：单次遍历三面、不落库无缓存、DTO 出线、呈现面、目录树、深度调节、测试边界逐字不变。

## MODIFIED Requirements

### Requirement: 代码统计解析命令

Rust 侧 SHALL 新增 `commands/stats/` 命令轨道，提供 `code_stats(root: String, depth: u32) -> Result<CodeStatsReport, String>`：

- 命令体 SHALL 为三件事薄包装（参数转换 / 调用 / 错误映射），领域组装收在 `code_stats_inner` 纯函数（`*_inner` app 层微形态先例，可不经 Tauri 运行时直接测试）；
- 解析 SHALL 经 tokei library（依赖仅落 desktop-app，版本精确 pin），MUST NOT spawn tokei CLI 子进程；
- 一次调用 SHALL 完成一次遍历并产出三面数据：汇总（文件数 / 代码行 / 注释行 / 空行）、语言行（按代码行降序的逐语言统计）、目录树（至多 `depth` 级目录节点的聚合统计 + 父目录在深度内的文件叶，见目录树面 Requirement）——树面 MUST NOT 触发第二次独立遍历；
- 命令 MUST NOT 注册 store 模型、MUST NOT 落库、MUST NOT 持有任何缓存（内存或磁盘）——每次调用都完整重新解析；
- DTO SHALL `specta::Type` + `serde(rename_all = "camelCase")` 出线，经既有 bindings 重导出管线生成 typed 包装，前端 MUST NOT 以裸 `invoke('code_stats')` 调用；
- 命令形态 SHALL 沿用同步形态（`pub fn`，本 delta 零代码改动），且本能力 MUST NOT 陈述「sync 命令不在主线程执行」——Tauri 同步命令在 IPC 线程（Windows = 主线程）内联执行，长任务会冻结窗口。`code_stats`（tokei 全仓解析）属 `desktop-app-shell`「命令执行线程纪律」的**登记债务**：其 async 化（`pub async fn` + `spawn_blocking`）SHALL 归独立变更，本能力 MUST NOT 预铺、MUST NOT 无该变更时擅自改命令形态；
- root 不存在或不可读 SHALL 返回 `Err(String)` 走前端 reject，MUST NOT panic、MUST NOT 静默返回空。

#### Scenario: 薄包装结构证明

- **WHEN** 审查 `commands/stats` 实现与测试
- **THEN** 命令体为参数转换 + `code_stats_inner` 调用 + 错误映射三段，测试以 serde_json 对照证明命令与 inner 结果一致（不经 Tauri 运行时）

#### Scenario: 单次遍历产出三面

- **WHEN** 对同一 fixture 工作区调用一次 `code_stats`
- **THEN** 返回的汇总、语言行、目录树来自同一次解析（树目录聚合与语言行总数自洽），无第二次遍历命令或调用

#### Scenario: 绑定出线与 typed 调用

- **WHEN** 执行 bindings 重导出并审查前端调用点
- **THEN** `bindings.ts` 含 `codeStats` 包装与 `CodeStatsReport` 系 camelCase 类型，前端经生成绑定调用，无裸字符串 invoke

#### Scenario: 不落库无缓存

- **WHEN** 审查 `commands/stats`、store 注册模型与全链代码
- **THEN** store 无新注册模型，命令无 memo / 静态缓存 / 磁盘缓存，连续两次调用各自完整解析

#### Scenario: 失败走 reject

- **WHEN** 以不存在的 root 调用 `code_stats`
- **THEN** 命令返回 `Err(String)`，前端收到错误字符串，进程无 panic、无空报告假成功

#### Scenario: sync 形态与登记债务在案

- **WHEN** 审查 `commands/stats` 的命令形态与本 spec 的表述
- **THEN** 命令仍为 `pub fn`（本 delta 零代码改动）；spec 内零「sync 命令不在主线程执行」一类陈述；async 化的债务、判据与适用面只在 `desktop-app-shell`「命令执行线程纪律」一处持有（本能力不复述判据、不新增计时 / 线程断言）

## Module Contract

### 模块增量：sync 形态口径纠正（archi-file-tree-lazy-load）

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `packages/desktop/src-tauri/src/commands/stats/mod.rs` | 代码统计命令轨道 | **零改动**：`pub fn code_stats(root: String, depth: u32) -> Result<CodeStatsReport, String>` 与 `code_stats_inner` 逐字不变；async 化不在本 delta（登记债务） |
| `packages/desktop/src/views/info/hooks/use-code-stats.ts` | 取数收口 | **零改动**：显式刷新模型、深度参数化 invoke、失败 inline 持久 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs`（交叉点） | 执行线程纪律的本次落地处 | 纪律与判据归 `desktop-app-shell`「命令执行线程纪律」；本能力只登记「`code_stats` 是该纪律的登记债务、本次不改」 |

### 零改动面

| 项 | 说明 |
|---|---|
| 统计呈现面 / 目录树展开折叠 / 深度可调与显式刷新 / 解析语义边界与测试纪律 | 逐字不变 |
| `reports/` 与测试语料 | 零变化（本 delta 无代码改动 ⇒ 零新增用例） |
