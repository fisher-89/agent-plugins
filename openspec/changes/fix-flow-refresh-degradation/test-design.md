# 测试设计: fix-flow-refresh-degradation

> **变更**: fix-flow-refresh-degradation
> **角色**: test-design-planner（executor）
> **输入**: proposal.md / design.md（本轮无既有 test-design.md，属首产）
> **范围**: 仅设计单元测试范围与公共 API 签名，不写测试命令细节；静态脚本（`vp check` / `vp test` / `bindings:check` / knip / cargo test 汇总执行）由桌面端执行，agent 内不重复。

## 测试框架与测试区域识别

- **前端 `packages/desktop/src/`**：Vitest（`vite-plus/test` 提供 `describe` / `it` / `expect` / `vi`），hook 用 `renderHook`、组件用 `render` / `screen`（`@testing-library/react`）+ jsdom（xyflow 渲染需 `ResizeObserver` 等环境垫片）。测试文件与源文件同目录同前缀，扩展名 `.test.ts`（hook）/ `.test.tsx`（组件）；进程边界 IPC 经 `vi.hoisted` + `vi.mock('@tauri-apps/api/core')`（invokeMock 按命令名 + 调用序可编程，ChannelMock 捕获 `onmessage` 供测试直投通知）；计时面（去抖 / 退避）经 `vi.useFakeTimers` + `advanceTimersByTime`。
- **后端 `packages/desktop/src-tauri/`**：Rust `cargo test --workspace`，测试为同目录 `*_test.rs`（经 `#[cfg(test)] mod xxx_test;` 内联挂载），断言 `assert!` / `assert_eq!`；workflow 查询沿 `detail_test.rs` 既有进程内假件 store（`DetailStore` 实现 `ChangeStateStore` + 可编程 fault 注入开关），orchestration 快照沿 `snapshot_test.rs` 真实 tempfile db + `Arc<dyn ChangeStateStore>` 注入缝，命令面沿 `mod_test.rs` 既有 `tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）装置。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 失败轮保 last-good：第 2 轮起 `get_change_detail` reject（detail 已在手）→ `detail` / `activeRun` / `artifacts` 维持上一成功轮值、nodes 不清零，内联「刷新失败」提示呈现（`data-testid` 可查），MUST NOT 出「未找到该 change」兜底页或 nodes=0 空图 | `packages/desktop/src/views/changes/hooks/use-change-detail.ts`（状态半边）、`packages/desktop/src/views/changes/change-detail-view.tsx`（横幅 / 图面半边） |
| AC-2 | 有界重试节奏：fake timers 断言重试间隔 1s / 2s / 4s 且至多 3 次自动补发；某次重试成功则提示消失、图面更新到最新（迁移间更新不被吃掉）；3 次穷尽后静默（无第 4 次自动查询） | `packages/desktop/src/views/changes/hooks/use-change-detail.ts`（节奏 / 穷尽半边）、`packages/desktop/src/views/changes/change-detail-view.tsx`（提示消失 / 图面更新半边） |
| AC-3 | 重试取消语义：重试挂起期间新通知（或手动刷新 / 换 root / id / 卸载）→ 旧重试作废、退避序列重置、不产生交错重复查询（invoke 计数可断言） | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` |
| AC-4 | 终态重试兜底：「Finished 通知 → 终态重查失败 → 重试成功」序列终局自愈不悬（不依赖下一条通知），全程 last-good 不塌 | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` |
| AC-5 | 后端 None / Err 分流：mock store `get_change` 返回 `Err(StoreFault)` → core `change_detail` 出 `Err`（错误串透传，MUST NOT 降级 None / 空面）；未知 id 仍 `Ok(None)`；命令面以错误串 reject（非 `null` 应答）；子表读失败仍单区块空面（命令层 Err 直测无注入缝，组合承载见不可测试项 4） | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`、`packages/desktop/src-tauri/src/commands/changes/mod.rs`（blank root / 未知 id / 成功面）、`packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs`（Err 错误串逐字）、`packages/desktop/src/views/changes/hooks/use-change-detail.ts`（reject 线面注入） |
| AC-6 | 首载与真 None 兜底不变：首载失败 → 整页错误兜底页；重查成功返回 `None`（真未找到）→ 清面「未找到该 change」兜底页；blank root 命令面 `Ok(None)` | `packages/desktop/src/views/changes/hooks/use-change-detail.ts`、`packages/desktop/src/views/changes/change-detail-view.tsx`、`packages/desktop/src-tauri/src/commands/changes/mod.rs` |
| AC-7 | 订阅粘性：失败轮保 last-good 期间 `activeRunPresent` 保持真值，`change_flow_watch` 订阅不被 effect cleanup 拆除，下一通知照常触发重查（零重订 / 零丢通知）；`use-change-flow-run.ts` 实现零改动，经链路组合用例承载 | `packages/desktop/src/views/changes/change-detail-view.tsx`（链路入口：useChangeDetail 保 last-good → `activeRunPresent` 派生 → useChangeFlowRun 订阅）、`packages/desktop/src/views/changes/hooks/use-change-detail.ts`（activeRun 保持半边） |
| AC-8 | 降级窗口动作可用：保 last-good 窗口内停止 / phase 间确认 / ask 应答动作照常可发（独立 invoke 不受降级态影响），控制入口状态与 last-good `activeRun` 一致 | `packages/desktop/src/views/changes/change-detail-view.tsx`（RunControlPanel 组合面） |
| AC-9 | 零漂移与观测性：bindings 再生成类型面零 diff（守卫面归静态）、错误串自现于前端内联提示（观测面成立） | `packages/desktop/src/views/changes/change-detail-view.tsx`（错误串自现半边）、`packages/desktop/src/types/generated/bindings.ts`（守卫面——见不可测试项 1） |
| AC-10 | 真机走查留痕：CDP 手法全程走查（发起 → 长步 → 迁移 → 收口图面不塌、终态不悬）——真机竞态不作为复现 AC | —（见不可测试项 3） |
| AC-11 | 版本交付：`packages/desktop` version 0.5.11；`vp check` / `vp test` / knip 全绿、无新增豁免（桌面端静态门禁） | `packages/desktop/package.json`（—见不可测试项 2） |

---

## 单元测试

> 说明：`路径类型` 取值——`正向` / `异常` / `边界` / `组合`（跨模块链路用例，按最上层调用方挂靠到入口模块章节）；`迭代类型` 取值——`新增`（本轮新增用例）、`修改`（既有测试文件随行为变化改写）、`保留`（既有用例不变）。每份 `#### 用例` 表对每个 `#### 待测功能` 条目至少覆盖正向 / 异常 / 边界三类。

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- `change_detail(layout, store, id): Result<Option<ChangeDetail>, StoreFault>` — 错误语义分流单点：`get_change` 读 `Err` 经 `?` 透传（`.ok()` 吞错消除）；未知 id（含 `workflow_type` 不在相位表）→ `Ok(None)`；`list_phase_records` / `list_runs` / `list_run_nodes` / `list_run_edges` 的 `unwrap_or_default` 单区块空面哲学不变（不升级整份 `Err`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `change_detail` | 正向 | 建档记录 + 相位 / run 子表种子 → `Ok(Some(...))` 全状态面聚合（id 锚 / 9 站流水线 / runs 投影既有断言面，签名随动后 `Ok` 包装拆包） | 修改 |
| `change_detail` | 异常 | 假件 `get_change` 返回 `Err(StoreFault::Db("…注入失败"))`（该 id 记录实际在库）→ 返回 `Err` 且与注入 fault 逐字相等（`StoreFault` `PartialEq`，Display 恒 `db: ` 前缀）；MUST NOT `Ok(None)`（未找到假象）、MUST NOT `Ok(Some)` 空面降级 | 新增 |
| `change_detail` | 边界 | `StoreFault` 三 kind（`Db` / `Conflict` / `NotFound`）逐变体注入 → 各自 `Err` 原样透传（枚举全词汇零改写） | 新增 |
| `change_detail` | 边界 | `get_change` 读 `Err` 且磁盘同名 change 目录在场 → 仍 `Err`（读失败与未找到不混同的磁盘半边——MUST NOT 回退磁盘目录解析成 `Ok` 面） | 新增 |
| `change_detail` | 边界 | 未知 id（db 无记录，磁盘同名目录在场）→ `Ok(None)`（真「未找到」回归——零目录回退、零 workflow.json 读取）；`workflow_type` 不在相位表（如 `bug-fix`）→ 同 `Ok(None)`（Ok 分支非 Err） | 修改 |
| `change_detail` | 异常 | `get_change` `Ok(Some)` 而 `list_runs` / `list_run_nodes` / `list_run_edges` 各自注入读 `Err` → 仍 `Ok(Some(...))`，对应区块空数组（单区块空面哲学保持，不升级整份 `Err`）；`list_phase_records` 注入读 `Err` → 同式 `Ok(Some)`、pipeline 各站 attempts 空（清单面同哲学回归） | 修改 |
| `change_detail` | 边界 | `get_change` 读 `Err` 与子表读 `Err` 同时注入 → 整份 `Err`（`get_change` 先行短路，子表降级仅在记录可达后生效——分流次序锚定） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `ChangeStateStore`（进程边界 DB，被测 API 显式入参 `&dyn`） | 沿既有 `DetailStore` 进程内假件（id 键记录表 + 归属键过滤条目 / run / nodes / edges 序列），**新增注入开关**：`get_change` 可编程 `Err(StoreFault)`（三 kind）、`list_phase_records` 可编程 `Err`；其余 unimplemented | Err 透传 / 分流 / 子表降级全部用例 |
| 磁盘树（进程边界 fs） | 不 mock：真实 tempdir `Env` RAII（active 裸名 / archive 前缀目录 + 产物文件），时间戳确定性 i64 常量 | 定位 / 产物 / 未知 id 磁盘半边 |
| 内部模块 | 不 mock：`phase_table` / `locate_change` / `discover_artifacts` 真实组合 | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

- `get_change_detail(stores, control, root, id)（IPC 命令）: Result<Option<ChangeDetailUnified>, String>` — 命令面 Result 化：blank root / 开库失败仍 `Ok(None)`（空结果语义维持）；core `Err` 经 `.map_err(\|fault\| fault.to_string())` 以错误串 reject 出线（非 `null` 应答，D4 单点映射）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `get_change_detail` | 正向 | 建档 id（tempdir 根 + 真实 store 种子）→ `Ok(Some(ChangeDetailUnified))`，detail 与 core 直调 serde 等值、`active_run` 投影照常（签名随动后 `Ok(Some(...))` 包装回归） | 修改 |
| `get_change_detail` | 边界 | blank root（`""` / `"  "` / `"\t"`）→ `Ok(None)`（既有空结果语义——MUST NOT `Err`）；root 不可寻址（数据根下不存在目录，canonicalize 失败 → `for_root` Err）→ `Ok(None)`（开库失败空结果语义维持，D4） | 修改 |
| `get_change_detail` | 边界 | 未知 id（磁盘同名目录在场）与 name 冒充 id → `Ok(None)`（真未找到回归，零磁盘回退、不虚构活面） | 修改 |
| `get_change_detail` | 异常 | core 读 `Err` 面组合承载：真实 `WorkspaceStores` 无 store 注入缝（见不可测试项 4）——命令层错误串 reject（非 `null`）的线面语义经 use-change-detail.test.ts invokeMock reject 注入直测（链路方向：命令 Err reject → 前端 catch 面分流）；错误串逐字（`fault.to_string()`，Display 前缀单点）经 detail_test.rs / snapshot_test.rs 断言；命令层 `.map_err` 唯一映射通路由返回类型编译面承载 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri 托管态 | 不 mock：沿既有 `tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）+ `Arc<ChangeFlowControl>` 托管，命令经 `app.state::<…>()` 直调（State 可 clone） | 全部用例 |
| fs / db（进程边界） | 不 mock：tempdir workspace 根真实目录树 + store change 域操作面真实种子（建档 / 开相 / 落账 / run 登记） | 全部用例 |
| 内部模块 | 不 mock：`queries::change_detail` / `resolve` 真实组合（`snapshot.rs` 半边归其专属节） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs

#### 待测功能

- `StoreSnapshot.detail(root, id)（WorkflowSnapshotPort impl，trait 契约签名 `Result<ChangeDetail, String>` 不变）` — match 三分支机械适配：`Ok(Some)` 透传；`Ok(None)` 仍 `Err("change 不存在: {id}")`（既有）；`Err(fault)` → store 错误串透传（`fault.to_string()`，取代误报「不存在」）；walker「快照读取失败: {message}」消费面格式零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `StoreSnapshot.detail` | 正向 | 建档 id（真实 tempfile db 种子）→ `Ok(detail)`，与 `queries::change_detail` 直调 serde 等值（换血不加工回归；直调拆 `Ok` 包装随动） | 修改 |
| `StoreSnapshot.detail` | 异常 | 假件 store `get_change` 返回 `Err(StoreFault::Db("…"))` → `Err("db: …注入串")` 错误串逐字透传；MUST NOT 含「change 不存在」误报文案（Err 与 None 分流出线不混同） | 新增 |
| `StoreSnapshot.detail` | 边界 | `StoreFault` 三 kind（`Db` / `Conflict` / `NotFound`）逐变体注入 → 各自 Display 前缀（`db: ` / `conflict: ` / `not_found: `）逐字透传（D4 Display 单点回归） | 新增 |
| `StoreSnapshot.detail` | 边界 | 未知 id（假件 `get_change` 返回 `Ok(None)` 与真实库未建档两形态）→ 既有 `Err("change 不存在: {id}")` 文案逐字不变（None 分支零改动回归） | 修改 |
| `StoreSnapshot.detail` | 边界 | trait object 装配（`Arc<dyn WorkflowSnapshotPort>`）下三分支同样成立（port 契约持衡——walker 消费面同式调用） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `ChangeStateStore`（进程边界 DB，被测 API 显式注入依赖 `StoreSnapshot::new(root, Arc<dyn …>)`） | 双轨：① 真实 tempfile db（`Ok(Some)` / `Ok(None)` 面，沿既有装置）；② 进程内假件 store（沿 `detail_test.rs` `DetailStore` 先例）——`get_change` 可编程序列 `Ok(None)` / `Err(StoreFault)`，其余 unimplemented（Err 面唯一可达注入缝） | Err 透传 / None 回归用例 |
| 磁盘树（进程边界 fs） | 不 mock：真实 tempdir `TempRoot` RAII（change 目录骨架 + 产物文件） | 定位 / 产物面 |
| walker 消费面 | 不 mock：`walker.rs` 零改动，错误格式契约经 port 断言间接锁定（「快照读取失败: {message}」前缀由消费方拼接，不在本节断言面） | 全部用例 |

---

### packages/desktop/src/views/changes/hooks/use-change-detail.ts -> packages/desktop/src/views/changes/hooks/use-change-detail.test.ts

#### 待测功能

- `useChangeDetail(root, id): ChangeDetailState` — 失败轮分流与有界重试编排：返回值增 `refreshFailed: string | null`（null = 无陈旧窗口，非 null = 保 last-good 中）；`refresh` / `notifyRefresh` 语义不变（新增先取消挂起重试的副作用）；退避常量（3 次 / 1s / 2s / 4s）单点、重试与 tick / 去抖共用取消通道（新意图取代旧重试）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `useChangeDetail` | 正向 | 首轮成功（detail / activeRun / artifacts 在手）→ 通知触发的第 2 轮 `get_change_detail` reject → `detail` / `activeRun` / `artifacts` 维持上一成功轮值（AC-1 / AC-7 的 activeRun 保持半边）、`refreshFailed` = 错误串（含后端串原样）、`loading` 复位 false、`error` 保持 null | 新增 |
| `useChangeDetail` | 正向 | 失败轮后第 1 次重试成功（应答含步迁移新数据）→ `refreshFailed` 清 null、`detail` / `runs` 更新到最新轮值（迁移间更新不被吃掉） | 新增 |
| `useChangeDetail` | 正向 | 失败 → 重试发起（`loading: true`）挂起期间 `refreshFailed` 保持（轮启动 patch 不清提示，D3 防闪烁）；成功轮 `refreshFailed` 才清 null | 新增 |
| `useChangeDetail` | 异常 | 首载失败（detail 不在手，首轮即 reject）→ `error` 置位（既有整页兜底态）、`refreshFailed` 保持 null、不调度重试（推进 8s+ 零自动补发）——既有「reject 置错误态」用例改写分流（首载 / 刷新两态对照） | 修改 |
| `useChangeDetail` | 异常 | 连续失败（首败 + 3 次重试全败）→ 每轮 `refreshFailed` 更新为最新错误串；终轮后仍 last-good（`detail` 不清） | 新增 |
| `useChangeDetail` | 边界 | 退避节奏（fake timers）：首败后推进恰 1s → 第 2 次自动查询；再败推进恰 2s → 第 3 次；再败推进恰 4s → 第 4 次（自动补发共 ≤ 3 次，`get_change_detail` 计数 4） | 新增 |
| `useChangeDetail` | 边界 | 穷尽静默：3 次重试穷尽后推进超长时间（> 4s）→ 无第 5 次自动查询（计数不增）、`refreshFailed` 保持，等待通知 / 显式刷新兜底 | 新增 |
| `useChangeDetail` | 边界 | 手动刷新取消：1s 重试挂起期间 `refresh()` → 推进 1s 无旧重试补发（计数不增）、以手动刷新轮为准；其后新失败从 1s 重新起算（退避序列重置） | 新增 |
| `useChangeDetail` | 边界 | 新通知取消：重试挂起期间 `notifyRefresh()`（去抖 300ms 拍后触发新轮）→ 旧重试作废、退避重置；全程 `get_change_detail` 计数无交错重复查询（任一时刻至多一个待生效刷新意图） | 新增 |
| `useChangeDetail` | 边界 | 换 root / id（rerender 变更 id）→ 挂起重试作废（新 id 上推进原退避时间零补发）；卸载（unmount）后推进时间 → 零自动补发（零遗留计时器） | 新增 |
| `useChangeDetail` | 边界 | `transcriptTick` 不入取消通道：重试挂起期间 `notifyTranscript()`（sessionEvent → 150ms 去抖）→ 重试不取消，仍按原退避点补发（长 agent 步期间重试链不被转录刷新打断，D2） | 新增 |
| `useChangeDetail` | 边界 | 真 None：失败轮后重试成功返回 `null` → 清面（`detail` / `activeRun` / `artifacts` 置空）且 `refreshFailed` 清 null（None 清面同时清陈旧提示——None 与 Err 分流不混同） | 新增 |
| `useChangeDetail` | 边界 | 终态竞态洞自愈（AC-4 序列）：首轮成功（activeRun 在场）→ 通知重查 reject（Finished 后终态重查失败、无下一条通知）→ 1s 重试成功返回终态数据（activeRun=null）→ 终局自愈不悬、全程 last-good 不塌 | 新增 |
| `useChangeDetail` | 边界 | 既有回归保留：成功轮 `refreshFailed` 初始 null；`read_artifact` 单产物降级 Fallback；id 为 null 早退零 invoke；显式 refresh 触发重拉；无失败时零轮询（推进 5min 计数不增） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：按命令名 + 调用序可编程（第 N 轮 `get_change_detail` resolve / reject / `null` 分流，`read_artifact` resolve）；记录调用计数（退避节奏 / 取消语义 / 交错断言面） | 全部 describe |
| 计时面（运行环境） | `vi.useFakeTimers` + `advanceTimersByTime`（退避 1s / 2s / 4s、去抖 300ms / 150ms 节奏逐拍推进），`afterEach` 复位真计时器 | 重试节奏 / 取消 / 穷尽用例 |
| 内部模块 | 不 mock：`useDebouncedBump` / `useRefreshRetry` / `loadUnifiedView` 文件私有计时与取数面真实组合（经 `useChangeDetail` 公共 API 驱动） | 全部用例 |

---

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件的公共 API 变更（`RefreshStaleNote` 为私有组件、`ChangeDetailView` 签名零变化、`useDetailViewData` 为模块私有函数）；以下列表项锚定「修改文件」表该文件行 ①–④ 的修改内容，非公共 API 声明。 -->

- `RefreshStaleNote({ message })` — 新增私有横幅组件（D1）：`data-testid="refresh-stale-note"`、样式沿既有失败面色、文案含后端错误串原样与「刷新详情」重试引导；呈现条件 `refreshFailed !== null`，成功轮清 null 即消失
- `useDetailViewData(root, id)` — 透传 `refreshFailed` 至视图 props；`activeRunPresent` 派生（`activeRun !== null`）在失败轮保 last-good 期间保持真值——订阅粘性链路面（AC-7）
- `DetailLoaded(props)` / `DetailLoadedProps` — props 增 `staleError: string | null`；在 `{header}` 与 `<DetailActionPanels>` 之间条件渲染 `RefreshStaleNote`（页首整宽横幅，D1）
- `ChangeDetailView({ root })` — `error` 分支收敛为首载语义（detail 在手的刷新失败落 `refreshFailed` 横幅，不再整页 `DetailFallback`）；首载「加载中…」与真 None「未找到该 change。」兜底分支零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 内联横幅呈现（`RefreshStaleNote`） | 正向 | 首载成功 → 第 2 轮 `get_change_detail` reject（detail 在手）→ 页面呈现 `data-testid="refresh-stale-note"` 横幅（文案含后端错误串原样与「刷新详情」重试引导），header / 动作面板 / 图面照常渲染 last-good（AC-1 / AC-9 错误串自现半边） | 新增 |
| 内联横幅消失 | 正向 | 失败（横幅在）→ 重试成功 → 横幅出销（`queryByTestId` 为空）、图面节点 / runs 更新到最新轮 detail（AC-2 视图半边）；横幅位于 header 与动作面板之间（页首整宽，D1） | 新增 |
| 降级窗口动作可用（组合，链路方向：ChangeDetailView → RunControlPanel → change_flow_stop / confirm / answer invoke） | 正向 | 保 last-good 窗口内（activeRun 在场形态）停止 / phase 间确认 / ask 应答入口在案且点击 → 对应 `change_flow_*` invoke 照常发出（独立 invoke 不受降级态影响）；控制入口状态与 last-good `activeRun` 一致（AC-8） | 新增 |
| `error` 收敛为首载语义（组合，链路方向：useChangeDetail 失败分流 → ChangeDetailView 渲染分流） | 异常 | detail 在手的刷新失败轮 → 不渲染整页 `DetailFallback`（`error-note` / 「未找到该 change。」零出现，`refresh-stale-note` 在案）；对照首载失败（detail 不在手）→ 整页错误兜底页（`error-note` 在案、横幅零出现）——既有 error 面用例改写分流 | 修改 |
| 真 None 清面兜底 | 边界 | 失败轮后重查成功返回 `null` → 清面「未找到该 change。」兜底页（`detail-note` 在案）、横幅同步消失（None 与 Err 分流）；首载「加载中…」占位分支零回归 | 新增 |
| 订阅粘性（组合，链路方向：useChangeDetail 失败轮保 last-good → `useDetailViewData` 派生 `activeRunPresent` → useChangeFlowRun `change_flow_watch` 订阅生命周期） | 边界 | 失败轮期间 `change_flow_watch` 零重订（Channel 实例数不增、订阅不被 effect cleanup 拆除）；经既有 Channel 直投下一条通知 → 统一视图重查照常触发（`get_change_detail` 计数 +1，零丢通知）——`use-change-flow-run.ts` 实现零改动经本链路断言承载（AC-7） | 新增 |
| 图面不塌 | 边界 | 失败轮期间图面节点集合保持上一成功轮（nodes 不清零、无 nodes=0 空图占位、运行徽章在案）；`loading` 仅作用于「刷新详情」按钮禁用面（无整页加载占位闪烁，D3） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke / Channel（进程边界 IPC） | 既有 `vi.hoisted`（invokeMock 按命令名 + 参数分发，`get_change_detail` 第 N 轮可编程 resolve / reject / `null`、挂起门闩驱动 loading 形态；ChannelMock 捕获 `onmessage` 供测试直投 RunNotice——订阅粘性链路的通知面） | 全部渲染用例 |
| jsdom 环境垫片（运行环境） | 既有 `ResizeObserver` / `DOMMatrixReadOnly` / `getBBox` 等垫片（xyflow 渲染必需，环境垫片非业务 mock） | 全部渲染用例 |
| 计时面（运行环境） | `vi.useFakeTimers` + `advanceTimersByTime`（重试退避与通知去抖推进——横幅呈现 / 消失节奏） | 横幅与订阅链路用例 |
| 内部模块 | 不 mock：`useChangeDetail` / `useChangeFlowRun` / `RunControlPanel` / `useFlowGraphData` 真实组合（沿既有 `vi.mock(import('./flow/attachments'), { spy: true })` 观测先例，本轮无新增观测需求） | 全部用例 |

---

## 不可测试项

| 被测文件或模块 | 原因 |
|----------------|------|
| `packages/desktop/src/types/generated/bindings.ts` | codegen 生成物非手改：`getChangeDetail` 类型面零漂移（Err 走 reject 通道）与 doc 注释随动（D6）由 `pnpm -C packages/desktop run bindings:export` 再生 + `bindings:check`（`git diff --exit-code`）一致性守卫承接（AC-9 静态半边，桌面端执行门禁）；golden 零重写由语料回归既有断言承载，无独立单元测试路径 |
| `packages/desktop/package.json` | `version` 0.5.10 → 0.5.11 为机械变更，无运行时逻辑；AC-11 的 `vp check` / `vp test` / knip 全绿与零新增豁免为桌面端静态门禁，无 agent 侧测试落点（`src-tauri/Cargo.toml` 保持不随动） |
| AC-10 真机走查留痕 | CDP 手法驱真实 UI 全程走查（发起 → 长步 → 迁移 → 收口）属进程 / 真机边界，不进进程内单测；proposal 明示真机竞态不可稳定复现、不作为复现 AC——单测层已以注入失败序稳定复现降级契约（AC-1–AC-4 / AC-7 / AC-8），走查由桌面端执行并留痕 |
| 命令层 `get_change` 读 `Err` 直测（`src/commands/changes/mod.rs`） | `WorkspaceStores` 为具体类型（`for_root` 返回 `Arc<Store>`），命令面无 store 注入缝；真实 redb 读事务失败不可确定性构造（强删 / 损毁 db 文件在打开句柄下平台行为不定，且零新增测试依赖面）——错误串逐字（`fault.to_string()` Display 单点，D4）由 detail_test / snapshot_test（`Arc<dyn ChangeStateStore>` 缝注入）断言，命令层 `.map_err(\|fault\| fault.to_string())` 为唯一映射通路（编译面），Err reject 非 `null` 的线面语义由 use-change-detail.test.ts 的 invokeMock reject 注入直测——AC-5 组合承载 |
