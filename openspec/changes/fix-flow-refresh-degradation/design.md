# 设计: fix-flow-refresh-degradation

> **变更**: fix-flow-refresh-degradation
> **日期**: 2026-10-16
> **基于**: proposal.md、explore.md（2026-10-15 定稿：三因素分解 + 层 1–3 修复方案定案）、specs/ 两份 delta（desktop-change-queries / desktop-change-flow-view）

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 两份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」七个文件并把 proposal 遗留的五个待决问题定稿到提示面 / 计时面 / 状态面 / 错误串面。

工作树实况（本变更起点，`packages/desktop` 0.5.10 基线）：`change_detail` 现为 `Option<ChangeDetail>`，`get_change` 读 `Err` 经 `.ok().flatten()?` 吞成 `None`；命令面 `get_change_detail` 返回 `Option<ChangeDetailUnified>`（blank root / 开库失败均 `None`）；`snapshot.rs` 以 `.ok_or_else` 把 `None` 与 `Err` 同译「change 不存在: {id}」；前端 `loadUnifiedView` 对 reject 一律 `apply({ error })` → 视图 `error !== null` 整页 `DetailFallback`，且 `!unified.detail` 轮 `apply({ detail: null, activeRun: null, artifacts: [] })` 清面；`useChangeDetail` 已有双去抖计时器（统一视图 300ms / 转录 150ms，`useDebouncedBump` 文件私有单点）。本设计描述**本变更完成后的目标形态**：后端 None / Err 分流出线、前端失败轮保 last-good + 内联提示 + 有界重试；零新表、零 DTO 线面演进、零 golden 重写、零命令新增。

---

## 关键设计决策（proposal 待决问题定稿）

| # | 问题 | 定稿 | 理由 | 备选方案 |
|---|------|------|------|----------|
| D1 | 内联「刷新失败」提示的呈现位置、文案与 data-testid | **页首整宽横幅**：`DetailLoaded` 内、`header` 与 `DetailActionPanels` 之间条件渲染；新增私有组件 `RefreshStaleNote`，样式沿既有失败面色（`rounded-md bg-fail-bg px-3 py-2 text-[13px] text-fail`——同 `run-error` / `error-note` 先例）；`data-testid="refresh-stale-note"`（kebab-case 命名族）；文案 `刷新失败：{message}——页面显示的是最近一次成功结果，将自动重试；也可点击「刷新详情」立即重试。`（`message` = 后端错误串原样嵌入）。呈现条件 = `refreshFailed !== null`（detail 在手的正常渲染态下天然成立）；成功轮 `refreshFailed` 清 null 即消失 | 陈旧是页面级信号——last-good 覆盖徽章 / 动作面板 / 图面全部，非仅图面；错误串自现（层 2 观测面，AC-9）；不进 `DetailHeader`（header 为组合传入的 `React.JSX.Element`，塞横幅须改组合面且语义混杂） | header 行内小字（错误串长、挤压标题区）；图面内浮层（xyflow 画布坐标面复杂、测试锚定弱） |
| D2 | 重试调度落位与退避常量 | **文件私有 hook `useRefreshRetry(fire)`**（驻 `use-change-detail.ts`，沿 `useDebouncedBump` 文件私有计时面先例——零新文件、零导出）：内部 `timer` ref + `attempt` ref；`schedule()`——`attempt ≥ REFRESH_RETRY_LIMIT` 直接静默返回，否则按 `REFRESH_RETRY_BACKOFF_MS[attempt]` 挂起并自增；`cancel()`——清挂起计时 + `attempt` 归零；卸载 cleanup 清计时。常量 `REFRESH_RETRY_LIMIT = 3`、`REFRESH_RETRY_BACKOFF_MS = [1_000, 2_000, 4_000] as const`，与既有 `DETAIL_REFRESH_DEBOUNCE_MS` / `TRANSCRIPT_REFRESH_DEBOUNCE_MS` 同区单点。`fire = bumpTick`——重试即「自动补发的刷新意图」，走同一 tick 通道 → 同一 effect → 同一查询路径，天然无交错重复查询。取消通道三条腿：① `refresh`（手动）与 `notifyRefresh`（**通知到达时**、去抖拍前）先 `retry.cancel()` 再 bump；② `useEffect(() => () => retry.cancel(), [retry, root, id])` 覆盖换 root / id 与卸载；③ 调度钩子自身卸载 cleanup。**load effect（`[root, id, tick]`）本体不做 cancel**——重试自身 bump 的 tick 重跑 effect 时不得重置退避序列（否则序列恒回 1s 死循环）；成功轮 outcome 归 `retry.cancel()`（重置 streak：新失败串从 1s 重新起算）。`transcriptTick`（sessionEvent）**不入**取消通道——它不产生统一刷新意图（非「新意图取代旧重试」范畴），且长 agent 步期间 sessionEvent 频繁、逐次取消会使重试链永不成形，恰毁掉本变更要封的降级窗 | 计时面独立成钩子保 `useChangeDetail` 主体在 `max-lines-per-function: 50` 红线内（R2）；重试与通知 / 手动刷新共用 tick 通道 = 取消语义单点（任一时刻至多一个待生效刷新意图）；终态竞态洞（Finished 是 run 最后一条通知）由重试自带封堵（AC-4） | 全内联 ref 计时器（函数体超 50 行红线 + 双计时生命周期难对齐）；独立 scheduler 文件（消费面单一，过度拆分） |
| D3 | 保 last-good 期间 `loading` 面表现 | **既有 `loading` 语义零改动**：整页「加载中…」占位仅 `loading && detail === null`（首载）呈现；detail 在手时 `loading` 只作用于「刷新详情」按钮禁用面（既有行为）。失败轮 patch 仅 `{ refreshFailed, loading: false }`——不触碰 `detail` / `activeRun` / `artifacts`，无整页闪烁；轮启动 patch `apply({ loading: true, error: null })` **不清** `refreshFailed`（重查中提示保持，成功才消失——防提示闪烁） | 既有视图分流已满足「detail 在手不闪整页」；防闪烁要求提示的消隐只由成功轮驱动 | 轻量顶部进度条（零既有先例、无收益） |
| D4 | `StoreFault` → String 映射形态 | **命令层单点 `.map_err(\|fault\| fault.to_string())`**——kind 前缀已在 `StoreFault` Display 单点（`db: {msg}` / `conflict: {msg}` / `not_found: {msg}`；state.rs doc 明文「命令层以 `.to_string()` 呈现，`Display` 恒带语境前缀」），零新增格式化、零第二处前缀。开库失败（`stores.for_root` Err）**维持 `Ok(None)` 空结果语义**（既有读命令降级面；本变更只透传 `change_detail` 的 StoreFault，doc 注释随动） | 沿 `create_change_with` 既有 `.map_err(\|e\| e.to_string())` 先例；fault 前缀双处组装必漂移 | 命令层自组语境前缀（`format!("读取失败: {fault}")`——与 Display 前缀重复） |
| D5 | 前端 `error` 与失败轮状态分流 | **`error` 收敛为首载语义**：仅 `hasDetail() === false`（首载或清面后）的失败轮置位 → 整页 `DetailFallback`（既有）；detail 在手的失败轮一律落新状态 `refreshFailed: string | null`（`ChangeDetailState` 增字段）→ 内联横幅。`loadUnifiedView` 新增 `hasDetail: () => boolean` 参（effect 侧 `detailRef` 镜像供给——effect 闭包内 state 快照过期）并返回轮结局判别 `UnifiedLoadOutcome = 'ok' \| 'stale' \| 'error' \| 'cancelled'`：`'ok'`（Some 与真 None 均计——None 清面走既有语义）→ `retry.cancel()`；`'stale'` → 保 last-good patch + `retry.schedule()`；`'error'` → 既有整页 error patch，**不调度**（首载失败维持既有兜底页，AC-6）；真 None patch 增 `refreshFailed: null`（清面同时清陈旧提示） | 状态面显式分流：视图渲染逻辑零判断负担（`error` 即首载兜底、`refreshFailed` 即横幅），测试锚定两态各自可查（AC-1 / AC-6）；outcome 返回值把调度决策留在 effect（retry 驻所），`loadUnifiedView` 保持声明式 | 沿用单一 `error` 字段 + 视图层判 detail 在手（呈现分流散到视图、语义混同） |
| D6 | bindings「零 diff」口径 | **按类型面零漂移执行**：`Result<Option<T>, String>` 经 specta 出线类型面不变（`__TAURI_INVOKE<{ detail; activeRun: ActiveRunView \| null } \| null>`——Err 走 reject 通道）。`get_change_detail` 的 Rust doc 注释描述 IPC 语义（「签名不变 / 均 None」已成伪）必须随动 → 生成物 doc 注释行随之 diff——该 diff 限注释行、非类型面；提交再生成产物保持 `bindings:check` 守卫绿；golden 零重写（DTO 线面形状零演进） | doc 与事实背离不可取；守卫语义 = 生成物与库一致（防手改），非冻结注释 | Rust doc 逐字冻结换 byte 级零 diff（doc 撒谎，弃） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| core 详情查询错误分流单点 | `change_detail` 签名 Result 化：`get_change` 读 `Err` 经 `?` 透传（`.ok()` 吞错消除）、未知 id（含 `workflow_type` 不在相位表）仍 `Ok(None)`、子表读 `unwrap_or_default` 哲学不动 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | workflow::state（`ChangeStateStore` / `StoreFault`）、serde、specta | Rust（sync 纯读） |
| IPC 命令面 Result 化 | `get_change_detail` 返回 `Result<Option<ChangeDetailUnified>, String>`：blank root / 开库失败仍 `Ok(None)`；`StoreFault` → 错误串映射收命令层单点（D4） | `packages/desktop/src-tauri/src/commands/changes/mod.rs` | workflow::queries、orchestration::control、store、specta | Rust（tauri command） |
| orchestration 快照机械适配 | `StoreSnapshot::detail` 对新签名 match 三分支：`Ok(Some)` → `Ok(detail)`；`Ok(None)` → 既有「change 不存在: {id}」；`Err` → store 错误串透传（不再误报不存在）。`WorkflowSnapshotPort` 契约与 walker「快照读取失败: {message}」消费面零改动 | `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` | workflow::queries、foundation::layout | Rust（sync port 实现） |
| bindings 再生 | `getChangeDetail` 类型面零漂移（Err 走 reject 通道）；doc 注释行随命令 doc 随动（D6） | `packages/desktop/src/types/generated/bindings.ts` | export-bindings 管线 | TypeScript（生成物） |
| 前端失败轮降级与有界重试单点 | `loadUnifiedView` 失败分流（reject 且 detail 在手 → 保 last-good + `refreshFailed` + 退避重试调度；首载失败维持整页 error）；重试计时器与 tick / 去抖同生命周期（D2 三条腿取消通道）；退避常量单点 | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | dto / bindings（`commands.getChangeDetail`）、React hooks | TypeScript、React |
| 内联提示与兜底分流 | `RefreshStaleNote` 横幅（data-testid 挂钩，D1）随 last-good 态呈现 / 成功轮消失；`error` 分支收敛为首载语义（detail 在手的刷新失败不再整页 `DetailFallback`）；首载错误 / 真 `None` 兜底页分支不变 | `packages/desktop/src/views/changes/change-detail-view.tsx` | hooks（useChangeDetail / useChangeFlowRun）、dto | TypeScript、React |
| 订阅粘性（零实现改动） | `useChangeFlowRun` 订阅生命周期实现不动——层 1 保 last-good 后 `activeRun` 不清 → `activeRunPresent` 派生（`useDetailViewData` 内 `activeRun !== null`）保持真值 → `change_flow_watch` 订阅不被 effect cleanup 拆除（AC-7 随层 1 自然达成，仅补测试） | `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | bindings（Channel / 四命令） | TypeScript、React（零改动） |
| 版本交付 | `packages/desktop` version 0.5.11（`src-tauri/Cargo.toml` 不随动） | `packages/desktop/package.json` | — | JSON |

---

## 变更清单

### 新增文件

<!-- 不涉及：本变更零新增文件。重试调度为 use-change-detail.ts 文件私有 hook（沿 useDebouncedBump 先例）、内联提示为 change-detail-view.tsx 私有组件、常量驻既有文件单点。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `change_detail` 签名 `Option<ChangeDetail>` → `Result<Option<ChangeDetail>, StoreFault>`（`use` 引入 `StoreFault`）；`store.get_change(id)` 由 `.ok().flatten()?` 改 `let Some(record) = store.get_change(id)? else { return Ok(None); }`——读 `Err` 经 `?` 透传（吞错消除）、未知 id 仍 `Ok(None)`（零磁盘回退零文档形态不变）；`phase_table` 未知类型分支 `return Ok(None)`（既有语义）；聚合体尾包 `Ok(Some(...))`；`list_phase_records` / `list_runs` / `list_run_nodes` / `list_run_edges` 的 `unwrap_or_default` 零改动；函数 doc 注释随动（None / Err 分流语义——「读失败 MUST NOT 降级 None / 空面」） | 层 2 错误语义分流单点（AC-5） |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `get_change_detail` 返回 `Option<ChangeDetailUnified>` → `Result<Option<ChangeDetailUnified>, String>`：blank root `return Ok(None)`、开库失败 `return Ok(None)`（既有空结果语义，doc 随动）；`queries::change_detail(...)` 由 `...?` 改 `.map_err(\|fault\| fault.to_string())?`（D4 单点）；成功尾 `Ok(Some(ChangeDetailUnified { detail, active_run }))`；doc 注释随动（IPC Result 面、Err 经 reject 通道出线、blank root / 开库失败仍空结果） | IPC 面分流（AC-5 / AC-6）；`list_changes` / `read_artifact` / `read_exec_output` 的 Option 空结果语义不动 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` | `StoreSnapshot::detail` 对 `change_detail` 结果 match 三分支：`Ok(Some(detail)) => Ok(detail)`；`Ok(None) => Err(format!("change 不存在: {id}"))`（既有）；`Err(fault) => Err(fault.to_string())`（store 错误串透传，取代误报「不存在」）；doc 注释随动（错误保真度提升注记） | 机械适配（R5）：`WorkflowSnapshotPort::detail` 契约与 walker「快照读取失败: {message}」格式零改动 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `pnpm -C packages/desktop run bindings:export` 再生成：`getChangeDetail` 类型面零漂移（`__TAURI_INVOKE<{ detail: ChangeDetail; activeRun: ActiveRunView \| null } \| null>`——Err 走 reject 通道）；doc 注释行随命令 doc 随动（D6）；非手改，提交再生成产物保持 `bindings:check` 绿 | 类型零漂移 + 观测守卫（AC-9） |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | ① 常量：`REFRESH_RETRY_LIMIT = 3`、`REFRESH_RETRY_BACKOFF_MS = [1_000, 2_000, 4_000] as const`（与去抖常量同区单点，doc 注记终态竞态洞理由）；② 私有 hook `useRefreshRetry(fire: () => void): { schedule: () => void; cancel: () => void }`（D2：timer / attempt refs、schedule 穷尽静默、cancel 清计时归零、卸载 cleanup）；③ `loadUnifiedView` 新增 `hasDetail: () => boolean` 参、返回 `UnifiedLoadOutcome`——catch 分流：`hasDetail()` → `apply({ refreshFailed: String(err), loading: false })` + `'stale'`，否则既有 `apply({ error: String(err), loading: false })` + `'error'`；成功 / 真 None patch 增 `refreshFailed: null`；轮启动 patch 不清 `refreshFailed`（D3）；④ `useChangeDetail` 布线：`refreshFailed` state、`detailRef` 镜像、`refresh` / `notifyRefresh` 先 `retry.cancel()` 再 bump、`useEffect(() => () => retry.cancel(), [retry, root, id])`、load effect 按 outcome 分派（`'stale'` → `retry.schedule()`；`'ok'` → `retry.cancel()`；`'error'` / `'cancelled'` → no-op）；⑤ `ChangeDetailState` 增 `refreshFailed: string \| null` 并入返回值；函数体超 50 行红线时按既有做法拆文件私有 helper（apply 装配 / 空参复位） | 层 1 失败轮降级与有界重试单点（AC-1–AC-4、AC-8）；`None` 清面语义不变 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | ① 私有组件 `RefreshStaleNote({ message }: { message: string })`（D1：data-testid="refresh-stale-note"、失败面色、文案含错误串与重试引导）；② `useDetailViewData` 透传 `refreshFailed`；③ `DetailLoadedProps` 增 `staleError: string \| null`，`DetailLoaded` 在 `{header}` 与 `<DetailActionPanels>` 之间条件渲染横幅；④ `ChangeDetailView` 的 `error !== null` 分支渲染逻辑不变、注释随动（error 已收敛为首载语义——detail 在手的刷新失败落 `refreshFailed` 不再触整页 `DetailFallback`）；首载「加载中…」与真 `None`「未找到该 change。」兜底分支零改动 | 内联提示与兜底分流（AC-1 / AC-6 / AC-9）；降级窗口动作面板（停止 / 确认 / 应答）随 last-good `activeRun` 照常渲染（AC-8，零实现改动） |
| `packages/desktop/package.json` | `version` 0.5.10 → 0.5.11 | 版本交付（AC-11；`src-tauri/Cargo.toml` 保持 0.1.0；`plugins/dev-team` 零改动） |

> **零改动沿用**：`crates/core/orchestration/src/walker.rs` 写序与 `RunNotice` 通知面（已核验）；queries 其余读面哲学与 `list_changes` / `read_artifact` / `read_exec_output` IPC 签名（产物单读失败已有 Fallback 信封降级，不扩面）；通知去抖常量 D8（300ms / 150ms）与 `use-change-flow-run.ts` 订阅生命周期实现（层 3 仅补测试）；转换层纯函数（`flow/graph.ts` / `attachments.ts` / `run-state.ts`）与 9 列布局 / 边表读取规则；golden 语料与 `DESKTOP_GOLDEN_REWRITE` 流程（DTO 线面形状零演进）；层 4 长步活性反馈（独立 change）；`plugins/dev-team` 全部。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `change_detail` | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 修改 | `pub fn change_detail(layout: &Layout, store: &dyn ChangeStateStore, id: &str) -> Result<Option<ChangeDetail>, StoreFault>` | 错误语义分流单点：`get_change` 读 `Err` 透传（`.ok()` 吞错消除）；未知 id（含未知 workflow_type）→ `Ok(None)`；子表读 `unwrap_or_default` 哲学不变（单区块读失败不升级整份 `Err`） |
| `get_change_detail`（IPC 命令） | `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 修改 | `#[tauri::command] pub fn get_change_detail(stores: State<'_, WorkspaceStores>, control: State<'_, Arc<ChangeFlowControl>>, root: String, id: String) -> Result<Option<ChangeDetailUnified>, String>` | 命令面 Result 化：blank root / 开库失败仍 `Ok(None)`；core `Err` 经 `.map_err(\|fault\| fault.to_string())` 以错误串 reject 出线（非 `null` 应答）；TS 类型面零漂移（Err 走 reject 通道） |
| `StoreSnapshot.detail`（`WorkflowSnapshotPort` impl） | `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` | 修改（trait 契约签名不变） | `fn detail(&self, root: &str, id: &str) -> Result<ChangeDetail, String>` | 机械适配：`Ok(None)` 仍「change 不存在: {id}」；`Err` 由 store 错误串透传（错误保真度提升——不再误报不存在）；walker 消费面「快照读取失败: {message}」格式不变 |
| `useChangeDetail` | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 修改（签名不变，返回型扩字段） | `export function useChangeDetail(root: string \| null, id: string \| null): ChangeDetailState` | 失败轮分流与有界重试编排：返回值增 `refreshFailed: string \| null`（内联失败态）；`refresh` / `notifyRefresh` 语义不变（新增先取消挂起重试的副作用） |

> 模块私有符号不入表：`useRefreshRetry`（重试调度钩子）、`loadUnifiedView`（取数函数，签名扩展见修改文件表）、`RefreshStaleNote`（横幅组件）、`UnifiedViewPatch` / `UnifiedLoadOutcome`（模块私有类型，见类型定义表注）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `ChangeDetailState` | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 修改 | 导出接口增 `refreshFailed: string \| null`——内联失败态（null = 无陈旧窗口；非 null = 保 last-good 中，视图呈 `refreshFailed` 横幅）；其余字段零变化 |
| `UnifiedViewPatch`（模块私有） | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 修改 | 部分写 patch 增可选键 `refreshFailed?: string \| null`——失败轮落提示不清 detail / activeRun / artifacts |
| `UnifiedLoadOutcome`（模块私有） | `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 新增 | 轮结局判别 `'ok' \| 'stale' \| 'error' \| 'cancelled'`——effect 据此分派重试调度（stale → schedule；ok → cancel；error / cancelled → no-op） |

> Rust 侧 `StoreFault` / `ChangeDetail` / `ChangeDetailUnified` / `WorkflowSnapshotPort` 零改动（Result 化仅为函数签名随动，错误类型与 DTO 形状复用既有）。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.5.11` | 0.5.10 → 0.5.11（`tauri.conf.json` 经 `../package.json` 自动跟随；`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 零改动） |

> 重试常量不进配置：`REFRESH_RETRY_LIMIT` / `REFRESH_RETRY_BACKOFF_MS` 为 `use-change-detail.ts` 模块级源码常量（D2，与去抖常量同区单点），零新增 config 键。

---

## 数据模型

### 后端出线语义（None / Err 分流）

| 形态 | core `change_detail` | 命令面 `get_change_detail` | 前端行为 |
|------|---------------------|---------------------------|----------|
| 记录缺席（未知 id）/ `workflow_type` 不在相位表 | `Ok(None)` | `Ok(None)`（invoke resolve `null`） | 清面「未找到该 change。」兜底页（既有，AC-6） |
| blank root / 开库失败 | 不进入（命令层早退） | `Ok(None)` | 前端 `root === null` 早退本就不发查询（R4 侧封）；既有空结果语义 |
| `get_change` 读 `Err` | `Err(StoreFault)` | `Err(fault.to_string())`（invoke reject） | 保 last-good + 内联提示 + 有界重试（AC-1–AC-4） |
| `get_change` Ok 而子表读 `Err` | `Ok(Some(...))`（单区块空面） | `Ok(Some(...))` | 正常呈现（单区块缺失，哲学不变） |

### 前端取数状态机（`useChangeDetail` 状态面）

```
轮启动:          apply({ loading: true, error: null })            ← refreshFailed 不清（重查中提示保持，D3）
轮成功(Some):    apply({ detail, activeRun, [artifacts], refreshFailed: null, loading: false }) → retry.cancel()
轮成功(None):    apply({ detail: null, activeRun: null, artifacts: [], refreshFailed: null, loading: false }) → retry.cancel()
轮失败·detail 在手: apply({ refreshFailed: String(err), loading: false }) → retry.schedule()    ← 保 last-good（AC-1）
轮失败·detail 不在手: apply({ error: String(err), loading: false })                              ← 首载整页兜底，不重试（AC-6）
轮取消(换 root/id/卸载): 丢弃（isCancelled）                                                    ← 'cancelled'，no-op
```

陈旧窗口生命周期：失败 → `refreshFailed ≠ null`（横幅在）→ 重试成功 / 下一条通知触发轮成功 → `refreshFailed = null`（横幅消失，图面更新到最新——迁移间更新不被吃掉）；连续失败 3 次重试穷尽 → 静默（横幅保持、无第 4 次自动查询，等待下一条通知或显式刷新兜底）。

重试取消语义（任一即作废挂起重试并重置退避序列，至多一个待生效刷新意图）：新通知到达（`notifyRefresh` 入口）/ 手动刷新（`refresh`）/ 换 root / id / 卸载。`transcriptTick`（sessionEvent）不入取消通道（D2：非统一刷新意图）。

### 持久化

零 db schema 变化（零迁移、零 golden 重写——DTO 线面形状零演进）；`refreshFailed` / 重试计数 / 挂起计时器为纯前端瞬态（React state / ref），零持久化、卸载即消散。

<!-- 路由 / API 设计见下节（IPC 命令面签名随动属 API 面变化，保留该节）。 -->

---

## 路由 / API 设计

| method | 命令 / 端点 | 说明 | 输入 | 输出 | auth |
|--------|------------|------|------|------|------|
| IPC invoke | `get_change_detail` | 统一视图聚合（库读史 ∪ 在飞 run 活面）；返回面 Result 化——Err 经 reject 通道出线，TS 类型面零漂移（AC-5 / AC-9） | `root: string, id: string` | Rust `Result<Option<ChangeDetailUnified>, String>` → TS `Promise<{ detail: ChangeDetail; activeRun: ActiveRunView \| null } \| null>`（resolve 面 = `Ok`；reject 面 = 错误串） | 既有桌面进程内 IPC 面（无新增鉴权层） |

> 零新增 / 修改 HTTP 端点与路由；`list_changes` / `read_artifact` / `read_exec_output` / `change_flow_*` 命令签名零改动。

---

## 依赖

### 运行时依赖

零新增。既有依赖复用：`workflow::state::StoreFault`（错误面既有类型，Display 恒带 kind 前缀）、`Result<T, String>` IPC 既有错误通道（tauri / specta reject 语义）、React hooks（`useState` / `useRef` / `useCallback` / `useEffect`——重试计时面沿 `useDebouncedBump` 同族手法）。

### 构建 / 测试依赖

零新增。bindings 再生走既有 `pnpm -C packages/desktop run bindings:export` 管线（`bindings:check` 守卫）；golden 零重写（无 `DESKTOP_GOLDEN_REWRITE` 触发）；fake timers 重试节奏测试（AC-2 / AC-3）走既有 vitest `vi.useFakeTimers` 能力，归测试相位。

---

## 待决问题

- **store 读错根因未钉死**（explore 遗留开放问题 1 / proposal R1）：层 2 落地后错误串经内联提示自现（AC-9 观测面），根因（redb 读事务 vs 写并发的瞬时错？）另立 change 钉死——不阻塞本变更。
- **AC-10 真机走查时机**：实现与测试相位完成后由桌面端以 CDP 手法执行一次全程走查留痕（发起 → 长步 → 迁移 → 收口图面不塌、终态不悬）——真机竞态本身不可稳定复现，不作为复现 AC；走查留痕不属 agent 产物。
