# 任务: desktop-change-flow

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖实现文件。前一版 implement 已落盘大量代码；本列表含回退/删除任务（插件侧 walker-* 与 infra/devteam），顺序按依赖排列：回退 → 写面 → 编排 → infra → 命令 → 前端 → 交付 → 守线。

## 阶段一：dev-team 插件回退（零新增）

- [x] 删除 `plugins/dev-team/bin/src/commands/walker-io.ts`、`walker-phase-next.ts`、`walker-phase-start.ts`、`walker-phase-log.ts`、`walker-backtrack.ts`、`walker-change-files.ts`、`walker-static-check.ts` 七文件及其同名 `.test.ts` 测试
- [x] 回退 `plugins/dev-team/bin/src/cli.ts` 与 `cli.test.ts` 中 walker-* 注册（恢复 2.10.44 已发布形态；MCP 注册面与 `lib/workflow.ts` 相位表逻辑零触碰）
- [x] 回退 `plugins/dev-team/package.json` `version` 2.10.45 → 2.10.44
- [x] 执行 `pnpm -C plugins/dev-team run build` 刷新三类交付产物（`dist/claude-plugins/dev-team/`、`dist/cursor-plugins/dev-team/`、`dist/cursor-home-image/dev-team/`——CLAUDE.md「Project Rules」按旧仓库根路径书写，实际以 `dist/` 子树为准），静态核对产物中零 walker-* 痕迹

## 阶段二：core/workflow 写面（sync 无 tokio）

- [x] 新增 `crates/core/workflow/src/write/phase_table.rs`：`PhaseAgentSpec` / `PhaseDefinition` / `MAX_RETRY_TIMES = 5` / `phase_table`（V1 仅 `requirement` 返回 Some）/ `interpolate`（`<change>`/`<phase>` 占位符，与插件 `interpolatePrompt` 语义一致）/ `allowed_backtrack_phases`（表序前置相位）
- [x] 新增 `crates/core/workflow/src/write/persist.rs`：raw Value 保形定点改写层——`serde_json::Value` 载入 + 既有宽松解析 typed `Workflow` 作逻辑面；写触点仅限 `active_phase` 置/清、`eval` 追加、`eval[i].stale` 翻转、`eval[i].backtrack_to/backtrack_reason` 标记，未知/legacy 字段原样保留，pretty JSON 写回（crate 内私有，无 crate 外导出）
- [x] 新增 `crates/core/workflow/src/write/phase_next.rs`：`SessionAnchors`（内部 `Mutex<HashMap<(change, run_id), usize>>`，每 run 一个实例）、`phase_next` 只读路由状态机——初始 / 推进 / fail 重试（≤`MAX_RETRY_TIMES`）/ `PhaseNextError::MaxRetriesExceeded` 上限判定 / `SessionAnchors` 锚点比对承接 mid-phase interruption；产出 `PhaseNextOutcome`（已插值 executor/evaluator prompt + `allowed_backtrack_phases` 白名单 + `last_result` 快照）；不改 eval store
- [x] 新增 `crates/core/workflow/src/write/phase_start.rs`：`phase_start`——`active_phase` 定点写入（phase / attempt / start_at），attempt 自既有值递增（重入即新一轮计时）
- [x] 新增 `crates/core/workflow/src/write/phase_log.rs`：`phase_log`——verdict 推导（checklist 全 pass，skipped 约束）+ report ≤500 校验 + **纯追加落账**（attempt 推导只数该相位既有条目 + 1，不因既有 pass 短路——插件丢重评条目缺陷在此修复）+ start_at 自 active_phase 继承 + 落账后清 active_phase
- [x] 新增 `crates/core/workflow/src/write/backtrack.rs`：`backtrack`——白名单二次校验（越权 Err 不写）+ reason ≤500 校验 + 最新条目标记 backtrack_to/backtrack_reason + stale 标记与相位表依赖向后传播
- [x] 新增 `crates/core/workflow/src/write/mod.rs`（模块声明与公共再导出）；修改 `crates/core/workflow/src/lib.rs` 挂载 `pub mod write` 并修订 crate 根「纯读库」注释措辞

## 阶段三：core/orchestration 语义换血（主体保留）

- [x] 修改 `crates/core/orchestration/src/port.rs`：`ToolCommand` 封闭集收缩（`ChangeFiles` 出局，载荷直载 `PhaseLogInput` / `BacktrackInput`）、`ToolStepOutput` 载荷换 `workflow::write` 原生类型（PhaseNext 变体 Box 收敛保持）、`ToolStepError` 出局（统一 `Result<_, String>`）、新增 `StaticCheckRunner` / `StaticCheckOutcome` / `DiffContextPort` / `BoxDiffFuture`；`WorkerAgentPort` / `WorkflowSnapshotPort` / `RunEventSink` 及 `WorkerTurnRequest` / `WorkerTurnOutcome` 保持
- [x] 新增 `crates/core/orchestration/src/steps.rs`：`LocalToolSteps::new(anchors, static_check)` 实现 `ToolStepPort`——相位机四步（phase-next / phase-start / phase-log / backtrack）经 `foundation::layout::resolve` 直调 `workflow::write::*`，StaticCheck 步委托注入的 `StaticCheckRunner`
- [x] 修改 `crates/core/orchestration/src/walker.rs`：补录步删除（循环 ①→⑧ 收敛为 ①→⑦）、`walk_run` 增 `diff: Arc<dyn DiffContextPort>` 参数并在每次 WorkerAgent prompt 组装前取 diff 上下文、决策输入来源换写面类型（白名单 `Vec<String>` + `last_result`）；反馈边计数 / phase 间停等 / 决策分叉 / 停止续走 / run_id 铸造骨架保持；注释中「CLI 工具步」措辞同步换血
- [x] 修改 `crates/core/orchestration/src/transcript.rs`：删除 `extract_write_paths` 半边及其测试模块对应半边（仅删除失效代码使编译面成立，新测试面归 test-design / test-gen 阶段）；`final_assistant_text` 保留
- [x] 修改 `crates/core/orchestration/src/verdict.rs`：`EvaluatorChecklist.checklist` 载荷换 `workflow::model::ChecklistItem`（自有 `ChecklistItemView` 出局），`parse_verdict` 签名与围栏容忍逻辑保持
- [x] 修改 `crates/core/orchestration/src/decision.rs`：`DecisionInput.allowed` 与 `ensure_backtrack_allowed` 白名单类型换 `Vec<String>`，verdict 引用换 `workflow::model::Verdict`；四动作封闭集形状保持
- [x] 修改 `crates/core/orchestration/src/prompt.rs`：`executor_prompt` / `evaluator_prompt` 增 diff 上下文段参数（evaluator 的 change/phase 参数出局——写面已插值）；决策 prompt 保持
- [x] 修改 `crates/core/orchestration/src/state.rs`：`ChangeStepKind` 收缩（`ChangeFiles` 变体出局），步词汇 doc 措辞「CLI 步」→「相位机/工具步」
- [x] 修改 `crates/core/orchestration/src/control.rs` 与 `snapshot.rs`：注释与错误消息中 CLI 通道措辞清理（control 逻辑零改动；snapshot 保留缩水）
- [x] 删除 `crates/core/orchestration/src/views.rs` 与 `views_test.rs`；修改 `crates/core/orchestration/src/lib.rs` 摘除 `mod views`、挂载 `mod steps`；`Cargo.toml` crate 注释措辞换血（依赖面不变）

## 阶段四：infra 执行面（spawn 不进 core）

- [x] 删除 `crates/infra/devteam/` 整 crate（`Cargo.toml`、`src/lib.rs`、`src/discover.rs`、`src/runner.rs` 及同名测试）
- [x] 修改 `packages/desktop/src-tauri/Cargo.toml`：workspace members / dependencies 增 `crates/core/orchestration`（已落盘保持）、移除 `crates/infra/devteam` 注册与根包 `devteam` 依赖；刷新 `Cargo.lock`
- [x] 新增 `crates/infra/agent/src/static_check.rs`：`ProcessStaticCheck::new()` 实现 `StaticCheckRunner`——`core::config` 读 static_analysis 命令 → `tokio::process` spawn（cwd = workspace root）→ 诊断捕获 + 退出码映射；无配置 / 空命令 = passed 直接过
- [x] 新增 `crates/infra/agent/src/git_diff.rs`：`GitDiffSource::new()` 实现 `DiffContextPort`——`git status --porcelain` 文件清单 + `git diff HEAD` 补丁体拼接，`DIFF_CONTEXT_LIMIT` 字符截断带省略标记
- [x] 修改 `crates/infra/agent/Cargo.toml`（增 `config`、`foundation` workspace 依赖；crate 注释去 devteam 措辞）与 `src/lib.rs`（`mod static_check` / `mod git_diff` 挂载与再导出）
- [x] 核对 `crates/infra/agent/src/worker.rs`：`WorkerAgentPort` 契约未变，仅随 port 中性类型调整（`ToolStepError` 出局等）核对编译面并随动适配；provenance / permission 组装保持

## 阶段五：change-flow 命令组与绑定

- [x] 修改 `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`：`change_flow_start` 删 `discover_cli` 前置校验、增 workflow_type=requirement 校验（`phase_table` 返回 None → 显式 Err「仅支持 requirement 工作流」）、组合根装配换血（`LocalToolSteps` + `ProcessStaticCheck` + `GitDiffSource` + `FsSnapshot` + run 级 `SessionAnchors` 注入）；其余五命令签名与逻辑保持
- [x] 核对 `packages/desktop/src-tauri/src/commands/mod.rs`（`pub mod change_flow;` + `all_commands!` 六命令）与 `main.rs`（`app.manage(Arc::new(ChangeFlowControl::new()))`）已落盘登记保持
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`（非手改；`src-tauri/src/bindings/mod.rs` 经宏自动纳入，预期零 diff）

## 阶段六：前端随动（已落盘主体保持）

- [x] 核对 `packages/desktop/src/views/changes/flow/run-state.ts`：步词汇映射随 `ChangeStepKind` 生成绑定收缩（`changeFiles` 步词汇出清后同步删除对应分支），`initialRunState` / `applyRunUpdate` / `runStepNodes` 其余逻辑保持
- [x] 核对 `packages/desktop/src/views/changes/flow/types.ts`、`graph.ts`、`run-step-node.tsx`、`change-flow-graph.tsx`、`detail-drawer.tsx` 与 `hooks/use-change-flow-run.ts`、`hooks/use-session-transcript.ts`、`flow/run-control-panel.tsx`、`flow/session-transcript-panel.tsx` 已落盘实现继续（时间序边推导规则、9 列布局、抽屉交互、降级规则零改动）
- [x] 核对 `packages/desktop/src/views/changes/change-detail-view.tsx` 运行控制入口组装保持（发起 / 停止 / phase 间确认 / ask 应答卡片 + run 终态显式 refresh）

## 阶段七：版本交付

- [x] 核对 `packages/desktop/package.json` `version` 0.4.0（已就位保持；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）；dev-team 插件零 bump（保持 2.10.44，阶段一已回退）

## 阶段八：守线（静态，不含测试执行）

- [x] 静态检查全绿：`cargo fmt --manifest-path src-tauri/Cargo.toml` 与 `cargo clippy --manifest-path src-tauri/Cargo.toml`、`pnpm -C packages/desktop run client:check`（`vp check --fix` + knip，无新增豁免条目）；自动化套件的执行验证由 test-execution 阶段承接
- [x] `pnpm -C packages/desktop run bindings:check` 零 diff（再生成链路确定性验证）
- [x] 静态自查：写通道唯一——walker 与命令组源码零 CLI 子进程写 workflow.json 代码路径、全部写触点经 `workflow::write`；零 file_log 写触点；`discover_cli` / `DEV_TEAM_PLUGIN_ROOT` / walker-* 零残留引用；`PIPELINE_PHASES` 与 `detail.rs` 相位列保持纯布局身份
- [x] 静态自查：新增/修改的产品 `.rs` 源码（含注释与 doc comment）不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名，豁免面以 `foundation/src/layout/mod.rs` 为准）
- [x] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏）
