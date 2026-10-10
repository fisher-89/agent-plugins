# 任务: unify-run-state-persistence

> **变更**: unify-run-state-persistence
> **依据**: proposal.md + design.md（D1–D13 决策编号见 design）

任务边界：本列表只含实现任务；**新增测试锚**（两写时序断言、词汇缺席断言、标定幂等、悬挂杀除、统一视图合并、常驻渲染、重挂步表、去抖行为）由 test-design / test-gen / test-execution 阶段承接——但类型 / 签名演进触及既有测试文件编译面，本列表阶段内含**机械随动**（改签名 / 改基设，不写新断言）。任务内文件均在 design.md 变更清单内；`plugins/dev-team`、`openspec/specs/**`、归档链全族（`archive_flow.rs` / `ArchiveSink` / `use-archive-flow.ts` / `archive-panel.tsx`）、`crates/infra/agent/src/worker.rs`、`crates/core/orchestration/src/steps.rs`（审计路径）为只读红线零触点。

## 阶段一：workflow 中性类型 + 写面 + 详情读面（core）

- [x] `crates/core/workflow/src/state.rs`：run 域中性类型新增（design「数据模型」节逐字段）——`RunStatus`（serde lowercase 五值）/ `RunStepKind`（serde snake_case 五值封闭集）/ `RunStepStatus`（serde camelCase 四值）/ `RunStateRecord` / `RunStepStateRecord` / `RunStartCommand` / `RunFinishCommand`（含 `RunStepEntry`；`status` 注释注明终态三值约束、interrupted 写面拒绝）；`ChangeStateStore` trait 增四方法：`list_runs` / `list_run_steps`（读半边）、`run_start` / `run_finish`（写半边）——文档注释注明 run 运行史落库（unify-run-state-persistence 决策翻案）与词汇封闭集单点语义
- [x] `crates/core/workflow/src/write/run.rs`（新文件）+ `write/mod.rs`：写面两函数——`run_start(store, command)`（run_id 非空 + change 建档在案（`get_change` None → Err）→ `store.run_start`）；`run_finish(store, command)`（change 建档在案 + 命令 status 为终态三值且非 interrupted（否则 Err「运行期写路径不产生 interrupted」）→ `store.run_finish`）；`mod.rs` re-export 随动
- [x] `crates/core/workflow/src/queries/detail.rs`：`ChangeRunStepRecord` / `ChangeRunEntry` DTO 新增（时间戳 `iso_from_millis` + Option 口径）；`ChangeDetail` 增 `runs: Vec<ChangeRunEntry>`；`change_detail` 聚合扩展——建档 change 经 `list_runs` + 逐 run `list_run_steps` 组装（runs 按 `started_at` 升序、并列按 run_id；steps 按 seq 升序）、文档形态（db 缺记录）恒空 runs；模块头注释 golden 契约注记随动（runs/steps 字段演进走显式重写）
- [x] 编译面机械随动：workflow crate 内实现 `ChangeStateStore` 的测试假件（写面 / 查询测试既有 fake）补四方法空实现 / 委派存根，`cargo check -p workflow` 过（断言更新留测试相位）

## 阶段二：store 模型 + run 域操作面 + 启动标定（infra）

- [x] `crates/infra/store/src/model.rs`：`RunRecord`（native_model id=13, v1；run_id 主键 / change 二级索引）/ `RunStepRecord`（id=14, v1；`step_key: u128` 打包主键 `pack_run_step_key(run_id, seq)` 复用 `session_key_hash` 家族 + `event_key_serde` 十六进制 serde、run_id 二级索引）两模型新增（design「数据模型」逐字段，含与 `StepRecord` 职责分立注释）；`pack_run_step_key` 组装点与既有 `pack_session_event_key` 同置
- [x] `crates/infra/store/src/store.rs`：run 域操作面五方法——`start_change_run`（同 run_id 冲突 → `StoreError::Conflict`；插 running 行）；`finish_change_run`（**单事务**：run 行在案且 running（否则 NotFound/Conflict）→ 终态 + reason + finished_at 更新 + `RunStepRecord` 整包插入（seq 保序）+ 该 change `active_phase` 清位（D1）；任一环节失败整体回滚）；`calibrate_interrupted_runs(now)`（读扫：全部 running 行 → interrupted（finished_at=now、reason 按 design 标定记因定式，附残留 active_phase 语境）+ 全部 `active_phase=Some` 的 ChangeRecord 清位；发现零残留时零写事务；幂等）；`list_change_runs(change)`（started_at 升序）；`list_run_steps(run_id)`（seq 升序）；`open_workspace` 尾部内嵌 `calibrate_interrupted_runs(now_millis())` 调用（D12，注释锚：进程起点无在飞 run 的结构性事实 + redb 单进程文件锁）；`workspace_models()` 注册两模型、模块头注释「八模型」→「十模型」
- [x] `crates/infra/store/src/change_port.rs`：`impl ChangeStateStore for Store` 增四方法委托（`list_runs` / `list_run_steps` / `run_start` / `run_finish` → store 操作面 + `fault` 映射）
- [x] 编译面机械随动：`model_test.rs` / `store_test.rs` / `change_port_test.rs` 既有用例保编译（新模型回环 / 两写时序 / 标定幂等断言留测试相位）

## 阶段三：orchestration——通知降位 + 累积器 + 落库缝

- [x] `crates/core/orchestration/src/state.rs`：`RunNotice` 新增（五 kind-only 变体，tag `ipc` camelCase——D3）；`ChangeRunSnapshot` 增 `started_at: i64` + `steps: Vec<ChangeStepState>`；`ChangeRunStatus::is_terminal` 文档注释「图回落派生规则」措辞随翻案改写（回落语义退场、库读史常驻）
- [x] `crates/core/orchestration/src/control.rs`：`RunEntry` 增 `steps: Vec<ChangeStepState>` + `started_at: i64`；`updates` 通道改 `broadcast::Sender<RunNotice>`；`begin_run` 签名增 `started_at: i64`；`publish(update: RunUpdate)` 重构——快照面突变照旧（Step 追加 `entry.steps` + phase/attempt、Ask/ConfirmWait/Finished 状态迁移）+ 广播侧改发 `RunNotice`（`From<&RunUpdate>` kind 投影，载荷剥离单点）；`snapshot()` 含 `started_at` / `steps`（克隆累积器）；**头注释决策原文改写**（「不建 flow_runs 表；run 终态即除名，桌面重启后 run 消失」→ 翻案后语义：run 运行史落库 RunRecord/RunStepRecord（unify-run-state-persistence），注册表承载在飞 run 与停等面、步累积器驻本表，终态即除名，重启后库史可达、残留 running 经启动标定 interrupted——决策翻案留痕锚）
- [x] `crates/core/orchestration/src/port.rs`：`RunHistoryPort` trait 新增（`run_started` / `run_finished`，sync 签名零 tokio、`Err` 串语义与既有 port 同型）
- [x] `crates/core/orchestration/src/run_history.rs`（新文件）：`persisted_step(ChangeStepKind) -> Option<RunStepKind>` **10→5 过滤单点**（executor/evaluator/decision/static_check/test_execution 落，phase_start/phase_log/verdict_gate/retry_gate/whitelist_gate 弃——注释锚 D2：词汇本体在 workflow::state::RunStepKind，本函数是唯一映射消费点）；`finish_command(...)` 组装 helper（steps → RunStepEntry，seq = 全词汇 emit 序盖戳、status 映射、session_id/detail 透传）；`StoreRunHistory { store: Arc<dyn ChangeStateStore> }` 实现 `RunHistoryPort`（委派 `workflow::write::run_start` / `run_finish`）
- [x] `crates/core/orchestration/src/walker.rs`：`RunRequest` 增 `started_at: i64`；`walk_run` 签名增 `history: Arc<dyn RunHistoryPort>`——起点第一写 `history.run_started`（Err → `guard.finish(Failed, Some(reason))` fail-fast 返回，D5）；`drive` 后 `guard.steps()` 取累积器 → `run_history::finish_command` 组装 → `history.run_finished` best-effort（Err 静默，D6 注释锚）→ 既有 `guard.finish(status, reason)`（终态出口唯一不变量保持）；`RunGuard` 增 `steps()` 读取方法；模块注释随动（每 run 两写）
- [x] `crates/core/orchestration/src/lib.rs`：re-export `RunHistoryPort` / `StoreRunHistory` / `RunNotice`（经 `port` / `state` 模块既有路径）
- [x] 编译面机械随动：`control_test.rs` / `walker_test.rs` / `state_test.rs` / `port_test.rs` 既有用例保编译——`begin_run` 加参、`walk_run` 加 history 参（进程内假件 / 真件 store 均可注入：真件走 `StoreRunHistory`、计数断言留测试相位）、broadcast 接收端类型 `RunNotice` 化；`port_test.rs` 的 `RecordingSink`（RunEventSink 面）零改动核对（RunUpdate seam 未动）

## 阶段四：命令面——统一查询 + 通知桥 + 快照命令退役

- [x] `src/commands/changes/mod.rs`：shell DTO `ChangeDetailUnified { detail: ChangeDetail, active_run: Option<ActiveRunView> }` + `ActiveRunView`（specta Type，字段见 design「数据模型」；`started_at` 毫秒 → ISO 串转换收本命令层单点）；`get_change_detail` 返回型改 `Option<ChangeDetailUnified>`——core `change_detail` 后并入 `ChangeFlowControl::snapshot(root, change)` 活面投影（None → `active_run: null`）；blank root / 开库失败 None 语义不变；模块头注释「读命令薄包装」面随动（统一视图合并语义）
- [x] `src/commands/change_flow/mod.rs`：`change_flow_start_with` —— `started_at = now_millis()` 铸造 → `begin_run(&root, &change, run_id, started_at)` + `RunRequest.started_at`；装配 `Arc<dyn RunHistoryPort>`（`StoreRunHistory::new(Arc<dyn ChangeStateStore> = store 克隆)`）传入 `walk_run`；`change_flow_start` / `change_flow_watch` 的 `Channel` 面 `RunUpdate` → `RunNotice`（`spawn_channel_forward` 签名随动）；`change_flow_state` 命令 + `_with` 测试缝 + handler 注册**整体删除**（D4，重挂恢复归统一查询）；`ChangeFlowSink` / stop / confirm / answer 零改动核对（RunUpdate 内部 seam 未动）
- [x] `commands` 注册面（`all_commands!` 消费处）：`change_flow_state` 出列机械随动
- [x] 编译面机械随动：`src/commands/changes/mod_test.rs` / `change_flow/mod_test.rs` 既有用例保编译——返回型解包 / Channel 类型 / 快照命令用例删除（统一视图断言留测试相位）；`bindings:export` 先跑一遍使 `bindings.ts` 再生成（`RunNotice` / `ChangeDetailUnified` / `ActiveRunView` / `ChangeDetail.runs` / `ChangeRunSnapshot` 扩面 / `changeFlowState` 退役落位）

## 阶段五：前端——两钩并一 + reducer 收缩 + 图常驻 + 转录统一

- [x] `packages/desktop/src/views/changes/hooks/use-change-detail.ts`：统一视图承接——`getChangeDetail` 返回 `ChangeDetailUnified`，state 增 `activeRun`；新增 `notifyRefresh()`（300ms 尾随去抖 bump tick，D8 常量单点）与 `notifyTranscript()`（150ms）对外暴露（转录刷新信号经视图下传）；`refresh`（显式）保留即时语义
- [x] `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts`：缩位重写——职责 = 控制动作（start / stop / confirm / answer，invoke 面零改动）+ 通知订阅生命周期：入参增 `activeRunPresent: boolean` + `onNotice: (kind) => void`；activeRun 在场才 `changeFlowWatch`（Channel<RunNotice>，onmessage 分流回调）；发起动作先行订阅再 `changeFlowStart`；卸载弃投递；`applyRunUpdate` / `seedRunState` / `initialRunState` / `changeFlowState` 调用面全删（组件态退化为查询缓存 + 失效重取）
- [x] `packages/desktop/src/views/changes/flow/run-state.ts`：reducer 收缩——`ChangeFlowRunState` / `initialRunState` / `seedRunState` / `applyRunUpdate` / `appendEvent`（liveEvents + seq 去重）删除；`runStepNodes` 输入改统一视图：新增 `unifiedRunSteps(runs: ChangeRunEntry[], liveSteps: ChangeStepState[])` 纯函数（runs[].steps 展开 + live 步并入 + 步词汇归一单点：`static_check`→`staticCheck` / `test_execution`→`testExecution`；runs 序 = startedAt 序即稳定分层序）；`runStepNodes` 槽位配对机制保留（running→终态 + 重号 `:seq` 堆叠——attempt 复用例外吸收，design 边界留痕节注释锚）；头注释「不持久化 / 图回落」措辞随翻案改写
- [x] `packages/desktop/src/views/changes/flow/graph.ts`：`buildFlowGraph(detail, runNodes)` 签名不变——调用侧 `runNodes = runStepNodes(unifiedRunSteps(detail.runs, activeRun?.steps ?? []))` 单源拼装（双源合成语义退场注释随动）
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：`useRunViewEffects` 删除（终态 refresh 回落分支 + liveEvents 展平双退场）；`DetailLoaded` 拼装改单源（`activeRun` 自 detail hook；`run` hook 供动作 + 订阅）；`liveEvents` prop 链拆除
- [x] `packages/desktop/src/views/changes/flow/run-control-panel.tsx`：props 改 `activeRun`（状态徽章 / 停等卡片 / ask 卡片自活面）+ `lastRun`（runs 尾行——终态徽章与收口记因 `run-finished-reason`）+ 动作面；data-testid 既有挂钩（`run-status` / `run-confirm-card` / `run-ask-card` / `run-finished-reason`）零改号
- [x] `packages/desktop/src/views/changes/flow/detail-drawer.tsx` + `session-transcript-panel.tsx` + `hooks/use-session-transcript.ts`：`liveEvents` 参数链退役；`useSessionTranscript` 增 `refreshKey` 入参（通知触发重查，转录库唯一数据源）；`assembleTranscript` live 并入分支删除（重放单源）
- [x] 前端测试机械随动：`use-change-detail.test.ts` / `use-change-flow-run.test.ts` / `run-state.test.ts` / `graph.test.ts` / `change-detail-view.test.tsx` / `run-control-panel.test.tsx` / `detail-drawer.test.tsx` / `session-transcript-panel.test.tsx` / `use-session-transcript.test.ts` 基设改新契约保编译（新断言留测试相位）；knip 零未用导出残留（AC-8）

## 阶段六：语料 + golden + 版本与守线收口（静态，不含测试执行）

- [x] `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/fixtures/README.md`：run 维度种子构造器扩展——`run_start` / `run_finish`（经 store run 域操作面，**禁裸表插桩**）+ `calibrate_interrupted_runs` 构造中断样本；样本：①多 run 全史（≥2 run、五词汇步、sessionId / detail 有无两态、attempt 跨 run 递增）②interrupted 标定（run_start 后标定，reason / finished_at / active_phase 清位面）；fixtures README 覆盖面矩阵增行；流程面步骤（phase_start / phase_log / 三门词汇）经 ChangeStepState 全词汇序列喂 `finish_command` 组装——缺席断言（零流程面行）留测试相位
- [x] golden 显式重写：`DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test` 覆写——既有六 detail 投影全量增 `runs` 键（空数组 / 存量样本零 run）+ 两新语料 golden；diff 人工确认预期范围（detail 增键 + 新文件）并留痕；`语料完整性_golden目录与语料集合一致` 复核绿
- [x] `packages/desktop/package.json` `version` 0.4.27 → 0.4.28（design D13：`src-tauri/tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）
- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）零错误零新告警；crate 图审查：`workflow` 零 infra 依赖不变（store 仅 dev-dep）、`orchestration` 零依赖新增（RunHistoryPort 委派经既有 workflow 依赖）、core 各 crate 无相互新依赖
- [x] 红线自查 grep：`plugins/dev-team` 零 diff（版本 2.10.44、三类交付产物）；归档链零触点（`archive_flow.rs` / `ArchiveSink` / `archive-panel.tsx` / `use-archive-flow.ts` / `worker.rs` / `RunUpdate` 类型定义零 diff）；`walker.rs` / `control.rs` / `steps.rs` 源码零 `RunStepKind` 过滤 match（AC-3——过滤逻辑仅在 `run_history.rs::persisted_step`）；`workflow.json` 零读写触点（既有双向墙复核）；`main.rs` 零 diff（标定内嵌 store 打开）
- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零错误且 knip 零新增豁免；`pnpm -C packages/desktop run bindings:check` 绿；变更清单对账：实现文件与 design.md 变更清单逐项对账（无清单外改动、无清单内遗漏，design「变更清单核对」增补项含入）；AC-11 版本交付 0.4.28 已执行
