# 任务: fix-flow-refresh-degradation

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接（含 `crates/core/workflow/src/queries/detail_test.rs`、`src/commands/changes/mod_test.rs`、`crates/core/orchestration/src/snapshot_test.rs` 随动、`src/views/changes/hooks/use-change-detail.test.ts`、`src/views/changes/change-detail-view.test.tsx`、`src/views/changes/hooks/use-change-flow-run.test.ts` 订阅粘性仅补测试——`use-change-flow-run.ts` 实现零改动），本列表只覆盖实现文件。静态脚本（`vp check` / `vp test` / knip / `bindings:check`）由桌面端执行，agent 内不重复执行。
>
> 依赖序要点：`change_detail` 签名 Result 化（阶段一）是编译面根依赖——`snapshot.rs`（阶段二）与命令面（阶段三）是其仅有的两个生产调用点，须同步适配；bindings 再生（阶段四）依赖命令签名定稿并先于前端随动（阶段五/六不消费新类型——TS 类型面零漂移，仅为守卫绿与 doc 注释随动的交付门）；前端 hook（阶段五）先于视图（阶段六，`ChangeDetailState.refreshFailed` 字段与 `useDetailViewData` 透传的消费方）；版本交付（阶段七）收尾。层 3 订阅粘性零实现任务（随层 1 保 last-good 自然达成，仅补测试归测试相位）。

## 阶段一：core 错误语义分流

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`：`use` 列表引入 `StoreFault`（自 `crate::state`）
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`：`change_detail` 签名 `Option<ChangeDetail>` → `Result<Option<ChangeDetail>, StoreFault>`；`store.get_change(id)` 由 `.ok().flatten()?` 改 `let Some(record) = store.get_change(id)? else { return Ok(None); };`——读 `Err` 经 `?` 透传（吞错消除）、未知 id 仍 `Ok(None)`（零磁盘回退零文档形态不变）
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`：`phase_table(&record.workflow_type)` 未知类型分支改 `return Ok(None)`（let-else 或同构 match）；函数尾 `Some(ChangeDetail { ... })` 包 `Ok(Some(...))`；`list_phase_records` / `list_runs` / `list_run_nodes` / `list_run_edges` 的 `unwrap_or_default` 零改动；函数 doc 注释随动（None = 真「未找到」/ Err = 读失败透传、MUST NOT 降级空面）

## 阶段二：orchestration 快照机械适配

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs`：`StoreSnapshot::detail` 对 `change_detail(...)` 结果 match 三分支——`Ok(Some(detail)) => Ok(detail)`；`Ok(None) => Err(format!("change 不存在: {id}"))`（既有语义）；`Err(fault) => Err(fault.to_string())`（store 错误串透传，取代误报「不存在」）；doc 注释随动（错误保真度提升注记）；`WorkflowSnapshotPort::detail` 契约签名与 walker「快照读取失败: {message}」消费面零改动

## 阶段三：命令面 Result 化

- [x] 修改 `packages/desktop/src-tauri/src/commands/changes/mod.rs`：`get_change_detail` 返回 `Option<ChangeDetailUnified>` → `Result<Option<ChangeDetailUnified>, String>`；blank root 改 `return Ok(None)`；开库失败（`stores.for_root` Err）改 `return Ok(None)`（既有空结果语义维持）；`queries::change_detail(&layout, store.as_ref(), &id)` 由 `...?` 改 `.map_err(|fault| fault.to_string())?`（错误串映射收命令层单点，D4）；成功尾改 `Ok(Some(ChangeDetailUnified { detail, active_run }))`；doc 注释随动（IPC Result 面、Err 经 reject 通道出线、blank root / 开库失败仍空结果语义）

## 阶段四：bindings 再生（前端随动的交付门）

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`：核对 `getChangeDetail` 类型面零漂移（`__TAURI_INVOKE<{ detail: ChangeDetail; activeRun: ActiveRunView | null } | null>`——Err 走 reject 通道，非手改）；doc 注释行随命令 doc 随动（D6 口径：类型面零漂移 + 注释行随动）；提交再生成产物保持 `bindings:check` 守卫绿

## 阶段五：前端失败轮降级与有界重试

- [x] 修改 `packages/desktop/src/views/changes/hooks/use-change-detail.ts`：新增模块级常量 `REFRESH_RETRY_LIMIT = 3` 与 `REFRESH_RETRY_BACKOFF_MS = [1_000, 2_000, 4_000] as const`（与 `DETAIL_REFRESH_DEBOUNCE_MS` / `TRANSCRIPT_REFRESH_DEBOUNCE_MS` 同区单点，doc 注记有界重试与终态竞态洞理由——Finished 是 run 最后一条通知，终态重查失败须重试自带）
- [x] 修改 `packages/desktop/src/views/changes/hooks/use-change-detail.ts`：新增文件私有 hook `useRefreshRetry(fire: () => void): { schedule: () => void; cancel: () => void }`——`timer` / `attempt` refs；`schedule()` 在 `attempt ≥ REFRESH_RETRY_LIMIT` 时静默返回、否则按 `REFRESH_RETRY_BACKOFF_MS[attempt]` 挂起并自增；`cancel()` 清挂起计时并归零 attempt；卸载 cleanup 清计时（沿 `useDebouncedBump` 文件私有计时面先例，零导出）
- [x] 修改 `packages/desktop/src/views/changes/hooks/use-change-detail.ts`：`UnifiedViewPatch` 增可选键 `refreshFailed?: string | null`；新增模块私有类型 `UnifiedLoadOutcome = 'ok' | 'stale' | 'error' | 'cancelled'`；`loadUnifiedView` 增参 `hasDetail: () => boolean` 并改返回 `Promise<UnifiedLoadOutcome>`——catch 分流：`hasDetail()` 为真 → `apply({ refreshFailed: String(err), loading: false })` 并返回 `'stale'`（不清 detail / activeRun / artifacts），否则既有 `apply({ error: String(err), loading: false })` 并返回 `'error'`；成功与真 None patch 增 `refreshFailed: null`；轮启动 patch `apply({ loading: true, error: null })` 不清 `refreshFailed`（重查中提示保持，防闪烁）；取消路径返回 `'cancelled'`
- [x] 修改 `packages/desktop/src/views/changes/hooks/use-change-detail.ts`：`useChangeDetail` 布线——新增 `refreshFailed` state 与 `detailRef` 镜像（`detailRef.current = detail`）；`const retry = useRefreshRetry(bumpTick)`；`refresh` 与 `notifyRefresh`（包一层 `useCallback`：先 `retry.cancel()` 再调去抖 bump）先取消挂起重试；`useEffect(() => () => retry.cancel(), [retry, root, id])` 覆盖换 root / id 与卸载；load effect 以 `.then((outcome) => ...)` 分派——`'stale'` → `retry.schedule()`、`'ok'` → `retry.cancel()`（重置退避序列：新失败串从 1s 起算）、`'error'` / `'cancelled'` → no-op（首载失败不重试）；`ChangeDetailState` 增 `refreshFailed: string | null` 并入返回值；函数体超 `max-lines-per-function: 50` 红线时拆文件私有 helper（apply 装配 / 空参复位）

## 阶段六：视图内联提示与兜底分流

- [x] 修改 `packages/desktop/src/views/changes/change-detail-view.tsx`：新增私有组件 `RefreshStaleNote({ message }: { message: string })`——`data-testid="refresh-stale-note"`、样式沿既有失败面色（`rounded-md bg-fail-bg px-3 py-2 text-[13px] text-fail`，同 `run-error` / `error-note` 先例）、文案 `刷新失败：{message}——页面显示的是最近一次成功结果，将自动重试；也可点击「刷新详情」立即重试。`
- [x] 修改 `packages/desktop/src/views/changes/change-detail-view.tsx`：`useDetailViewData` 解构并透传 `refreshFailed`；`DetailLoadedProps` 增 `staleError: string | null`；`DetailLoaded` 在 `{header}` 与 `<DetailActionPanels>` 之间条件渲染 `{staleError !== null && <RefreshStaleNote message={staleError} />}`（页首整宽横幅，D1）
- [x] 修改 `packages/desktop/src/views/changes/change-detail-view.tsx`：`ChangeDetailView` 的 `error !== null` 分支渲染逻辑零改动、注释随动（error 已收敛为首载语义——detail 在手的刷新失败落 `refreshFailed` 走横幅，不再触整页 `DetailFallback`）；首载「加载中…」与真 `None`「未找到该 change。」兜底分支零改动；`RunControlPanel` / `useChangeFlowRun` 零改动（降级窗口动作面板随 last-good `activeRun` 照常渲染，AC-8 / AC-7 自然达成）

## 阶段七：版本交付与静态自查

- [x] 修改 `packages/desktop/package.json`：`version` 0.5.10 → 0.5.11（`src-tauri/Cargo.toml` 保持 0.1.0 不随动；`plugins/dev-team` 零改动）
- [x] 静态自查（不执行脚本，逐项核对）：`detail.rs` 子表读四处 `unwrap_or_default` 哲学未动、`get_change` 无 `.ok()` 残留；`walker.rs` 写序与 `RunNotice` 通知面零改动；`use-change-flow-run.ts` 与通知去抖常量（300ms / 150ms）零改动；golden 语料零触碰（DTO 线面形状零演进）；`list_changes` / `read_artifact` / `read_exec_output` 签名未动；`plugins/dev-team` 与 `openspec/specs/**` 既有基线零改动
