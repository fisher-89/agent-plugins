# desktop-app-shell Specification（delta: archi-file-tree-lazy-load）

## Purpose

本 delta 只**新增一条执行线程纪律**，不改任何既有 requirement 的陈述：命令的轨道归属（changes / exec / db）、三件事纪律、`*_inner` 微形态、`run_agent()` 编排缝、错误双轨、app crate 翻转信号**逐字不动**。

1. 把「同步命令 = `ExecutionContext::Blocking` = IPC 线程（Windows = 主线程）内联」从默认事实升级为**纪律**：批量子系统 IO / 阻塞计算的命令 MUST 走 `pub async fn` + `spawn_blocking`（blocking 池），MUST NOT 同步注册、MUST NOT 退到 `#[tauri::command(async)]` + `pub fn`（worker 池 = 核数，与 agent 流式任务抢同一池）。
2. 给出**机械判据**：命令 future 的首次 poll 必为 `Pending`——同步实现的 async 体型（体内直接调 `*_inner`）在首次 poll 就返回 `Ready`，该形态即违例。
3. 明确**本次适用面**（archi 四条取数命令）与**登记债务**（`code_stats`），并写死一条容易被读反的语义：async 化**不改变并发度**——无取消、无去重、无节流，重复触发会真正并行。

本 delta 不含：`code_stats` 的 async 化（登记债务，归独立变更）、任何命令的参数 / 返回 / 错误文案变化、取消通道的扩展。

## ADDED Requirements

### Requirement: 命令执行线程纪律（批量子系统 IO）

Tauri 命令的执行线程由**命令形态**决定，批量子系统 IO / 阻塞计算的命令 SHALL 落阻塞池而非 IPC 线程：

- **同步形态即冻结**：`pub fn` 命令 = `ExecutionContext::Blocking` = 命令体在 IPC 线程（Windows = 主线程）内联执行 = webview 冻结。命令体「看起来快」不构成豁免——同一线程上的慢命令会把快命令一起排队，症状总落在最后未返回的那条命令上。
- **形态**：此类命令 SHALL 为 `#[tauri::command]` + `#[specta::specta]` + `pub async fn`，命令体内经 `tauri::async_runtime::spawn_blocking(move || …)` 调既有 `*_inner` 领域组装纯函数，`.await` 后映射错误。`*_inner` 的签名与语义 MUST NOT 因 async 化而改动（三件事纪律与 `*_inner` 微形态不变），领域组装 MUST NOT 被搬进 async 体的同步段。
- **反形态 MUST NOT 采用**：`#[tauri::command(async)]` + 保持 `pub fn`——该形态落 tokio **worker** 池（上限 = 核数），与 agent 流式任务抢同一池；blocking 池才是为阻塞 IO 设计的。
- **判据（机械可验）**：命令 future 的**首次 poll MUST 为 `Pending`**。同步实现（async 体内直接调 `*_inner`）在首次 poll 内跑完返回 `Ready`；`spawn_blocking` 实现必然先 `Pending`。该判据 MUST NOT 被替换为计时断言（时长随机器 / 缓存漂移）。
- **`JoinError` 为前向防御**：阻塞任务 panic 时 `JoinError` SHALL 映射为 `Err`（文案含错误原因）；该分支只在阻塞任务 panic 时出现，属不可测面（前向防御），MUST NOT 为其新增 test-only 注入点。
- **并发度不变**：async 化 MUST NOT 改变并发语义——命令面恒 `CancelFlag::never()`、无 in-flight 去重、无节流，重复触发（连点刷新 / 反复切页 / 刷新触发的 `key` 重挂）SHALL **真正并行**执行每次任务。本纪律 MUST NOT 被读作提供取消 / 去重 / 节流；此类治理属各命令所属能力的独立变更。
- **出线面零变化**：`specta` 显式处理 asyncness ⇒ 生成绑定（typed 包装 / zod 表项 / 逐字签名）与 `all_commands!` 命令清单 SHALL 逐字不变；async 化 MUST NOT 改动命令名、参数、返回类型与错误文案。
- **本次适用面（四条取数命令）**：`archi_graph` / `archi_drift` / `archi_file_tree` / `archi_api_check`。形态统一的理由：只把最慢的一条改 async 也能解冻，但四条取数命令留在同一 IPC 线程上排队时，快命令仍被慢命令按线程阻塞。
- **明确不在适用面**：`archi_module`（点击级、单模型解析）、`archi_change_summary`（低频；带 `State<WorkspaceStores>`，改 async 须先 `for_root` 取 `Arc<Store>` 再移入闭包）、`archi_set_gate`（毫秒级写）、`archi_api_generate`（写命令；`workspace_render` 只读模型 + 契约资产，成本随模型规模而非仓库规模增长）、`workspace_config`（单文件读，非批量子系统 IO）。
- **登记债务**：`code_stats`（tokei 全仓解析）同属本纪律的病灶面、SHALL 判为待清偿；其 async 化归独立变更。本纪律 MUST NOT 被读作「全仓已满足」，MUST NOT 为消债在本变更内顺带改它。

#### Scenario: 四条命令首次 poll 为 Pending

- **WHEN** 对 `archi_graph` / `archi_drift` / `archi_file_tree` / `archi_api_check` 各取一条命令 future，并以 `Waker::noop()` 的 `Context` 做首次 `poll`
- **THEN** 四次均返回 `Pending`（命令体未在 IPC 线程内联执行）；若把任一命令体改回「async 体内直接调 `*_inner`」，该用例立刻变 `Ready` 而红

#### Scenario: 新增批量子系统 IO 命令的审查判据

- **WHEN** 新增或改动一条命令，其命令体做整仓枚举 / 全文件读 / 全模型解析 / 全量 AST 扫描
- **THEN** 该命令为 `pub async fn` + `spawn_blocking` 调 `*_inner`，`*_inner` 签名与语义不变；MUST NOT 以 `#[tauri::command(async)]` + `pub fn` 形态落地；`code_stats` 的 async 化 MUST NOT 被顺带夹带进其他变更

#### Scenario: 出线面与命令清单零变化

- **WHEN** 四条命令 async 化后执行 `bindings:export` 并审查 `all_commands!`
- **THEN** `src/types/generated/**` 零 diff（typed 包装签名逐字不变）、`all_commands!` 零 diff、命令清单与相邻序断言照旧通过；IPC 参数、返回类型与错误文案逐字不变

#### Scenario: 并发度不承诺取消与去重

- **WHEN** 用户在检查未完成时连点刷新两次（每次刷新经 `key` 重挂重新发起取数）
- **THEN** 两条扫描并行执行、各自产出完整结果（后到者覆盖呈现），命令面零取消、零去重、零节流；该现象 SHALL 被视为「命令不再冻结窗口」的直接后果，MUST NOT 被读作 async 化回归

#### Scenario: JoinError 映射且无注入缝

- **WHEN** 审查四条命令的 `JoinError` 分支与测试面
- **THEN** 该分支映射为 `Err`（文案含原因）并在本 requirement 登记为前向防御；源码零 test-only 注入点（无「测试用 panic 开关」一类导出）

## Module Contract

### API: `packages/desktop/src-tauri/src/commands/archi/mod.rs`（命令面形态）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_graph` / `archi_drift` / `archi_file_tree` / `archi_api_check` | 改 | `pub fn` → `pub async fn` + `tauri::async_runtime::spawn_blocking(move \|\| *_inner(…))`，`.await` 后 `JoinError` 映射为 `Err`。IPC 签名（命令名 / 参数 / 返回类型 / 错误文案）逐字不变 ⇒ 出线面零 diff |
| `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_api_generate` | 零改动 | 保持同步形态（理由见本 requirement「明确不在适用面」）；`workspace_config` 同理 |

### Function: `*_inner` 与领域组装（零变化）

| 函数 | 变化 | 描述 |
|---|---|---|
| `archi_graph_inner` / `archi_drift_inner` / `archi_file_tree_inner` / `archi_api_check_inner` | 零改动 | 签名（`&Path` / `&dyn ArchiveVcsPort` / `ArchiDriftMode`）、语义、错误文案逐字不变；只被移入阻塞闭包 |
| `check_root` 与领域投影（`archi::dto` / `archi::tree` / `archi::drift` / `archi::api`） | 零改动 | root 校验与全部领域规则仍在闭包内执行，命令层零规则不变 |

### CLI: 生成链与检查链

| 项 | 变化 | 描述 |
|---|---|---|
| `bindings:export` | 零变化 | specta 显式处理 asyncness ⇒ 产物逐字节不变（幂等） |
| `all_commands!`（`src-tauri/src/commands/mod.rs`） | 零变化 | 命令清单与顺序逐字不变；builder 注册同源 |
| `pnpm check` 链（`server:check` → `bindings:export` → `api:check` → `client:check`） | 零变化 | 段序与命令串逐字不变 |

### Component: 前端

| 项 | 描述 |
|---|---|
| 前端调用面 | **零改动**：生成的 typed 包装仍返回 `Promise<T>`，hooks 与视图无感（本 delta 无前端改动） |

### 零 delta 的能力交叉点

| 能力 | 理由 |
|---|---|
| `desktop-archi-model`（IPC 八命令契约） | 其陈述「八命令薄包装 + 各配 `*_inner`」在 async 化后仍成立（wrapper 仍是薄包装）；线程形态的唯一持有者是本 requirement，MUST NOT 在两处复述 |
| `desktop-ipc-type-bindings` | 生成物零 diff、清单零改动 |
| `desktop-workspace-config` | 其 spec 未陈述错误事实；`workspace_config` 不在适用面内 |
