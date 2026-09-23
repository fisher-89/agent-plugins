# 任务: desktop-agent-execution

> 依赖顺序编排：契约 crate → CLI 适配 → store 持久化 → 命令面 → 前端。测试文件由独立工作流阶段负责，本清单不含测试任务。

## 阶段一：契约与实现 crate（core/agent → infra/agent）

- [x] 新建 `crates/core/agent` 与 `crates/infra/agent` 两 crate 骨架（`Cargo.toml` + `src/lib.rs` 模块声明与公共导出），并在 `packages/desktop/src-tauri/Cargo.toml` 注册：members 追加两 crate、`[workspace.dependencies]` 增 `agent` / `agent-cli` / `tokio`（`sync` 收敛）、根包 `[dependencies]` 增 `agent` / `agent-cli`
- [x] 实现 `crates/core/agent/src/event.rs`：`AgentEvent`（seq + timestamp_ms + kind）、`AgentEventKind` 五变体、`AgentBlock` 四变体、`AgentEvent::stamp(seq, kind)` 盖戳构造、serde camelCase 线格式（kind 内部 tag 扁平）
- [x] 实现 `crates/core/agent/src/runner.rs`：`AgentRunner` trait（`start(params) -> Result<AgentRun, AgentStartError>`）、`AgentRunParams`、`AgentEnvMode` / `AgentPermissionMode`（serde camelCase + `as_str()`）、`AgentRun`、`RunHandle`（MVP 预留空结构）、`AgentStartError`（CliMissing / SpawnFailed）
- [x] 实现 `crates/core/agent/src/state.rs`：`AgentRunState` 枚举与 `RunStateMachine`（`new` / `apply` / `current`；RunResult.is_error 驱动收敛，收敛后拒绝变更）
- [x] 实现 `crates/infra/agent/src/flags.rs`：flag 组装纯函数——`-p <prompt>`、`--output-format stream-json --verbose` 恒有；bare → `--bare`；bypassPermissions → `--dangerously-skip-permissions`、acceptEdits → `--permission-mode acceptEdits`、default → 无 flag；无 model / resume / partial flag
- [x] 实现 `crates/infra/agent/src/discover.rs`：`discover_in(dirs)` 纯函数（Windows 按 `claude.cmd` / `claude.bat` / `claude.exe` 顺序，其余 `claude`）+ 读 `PATH` 薄包装；缺失返回 `AgentStartError::CliMissing`
- [x] 实现 `crates/infra/agent/src/jsonl.rs`：逐行归一化——system/init → runStarted、system 其余 → systemNotice、assistant/user → message（blocks 四块映射 + parent_tool_use_id）、result → runResult（total_cost_usd → costUsd 等字段对齐）、未知 type / 非 JSON 行 → raw 透传；空白行跳过不占 seq
- [x] 实现 `crates/infra/agent/src/runner.rs`：`ClaudeCliRunner` `impl AgentRunner`——discover → spawn（`current_dir(cwd)`、stdout piped、Windows `.cmd`/`.bat` 经 `cmd /C` 包装）→ tokio 泵任务逐行归一化发入有界（256）mpsc → 进程退出且无 result 事件时补发合成 `RunResult`（subtype `error_process_exit`、is_error=true）

## 阶段二：store 持久化（user 维度两表）

- [x] 扩展 `crates/infra/store/src/model.rs`：新增 `AgentRunRecord`（id / prompt / cwd / env / permission_mode / status / started_at / finished_at / num_turns / cost_usd / duration_ms / session_id / error，serde camelCase）与 `encode` / `decode`（沿 `WorkspaceRecord` 模式）
- [x] 扩展 `crates/infra/store/src/store.rs`：`USER_AGENT_RUNS` / `USER_AGENT_RUN_EVENTS` 两表定义；`init_schema` 顺手打开两新表；五操作 `begin_agent_run`（事务内 max+1 分配 id 落 running 行）/ `append_agent_run_events`（`(run_id, seq)` 复合键、`serde_json::Value` 行、单事务批量）/ `finish_agent_run`（整行替换收敛）/ `list_agent_runs`（started_at 降序）/ `list_agent_run_events`（区间扫描 seq 升序）
- [x] 更新 `crates/infra/store/src/lib.rs`：`pub use model::AgentRunRecord;` 出 crate 公共面

## 阶段三：exec 轨道命令面与注册

- [x] 新建 `src/commands/exec/agent.rs`：`run_agent()` 薄入口（组装 `ClaudeCliRunner`）+ `run_agent_with()` 泛型编排——`start` → begin_agent_run → tee 循环（recv → 状态机 apply → store 逐事件追加 → Channel 发送；Channel 失败不中断、store 失败收敛 failed 并终止）→ finish_agent_run → 返回最终 `AgentRunRecord`
- [x] 重写 `src/commands/exec/mod.rs`：移除「预留空轨道」注释；落三命令——`agent_start`（async，State + `Channel<AgentEvent>` + root/prompt/env/permission_mode，body 三件事：参数转换 → 调 `run_agent()` → 错误映射）、`agent_runs` / `agent_run_events`（无状态薄包装，事件行 `Value → AgentEvent` 转换）
- [x] 更新 `src/commands/mod.rs`：模块 doc 中 exec 由「预留空轨道」改为「已开通（agent 首批三命令）」，补 `run_agent()` 微形态与 `*_inner` 同列注记（spec 指针用相对域根定式，不含被禁目录名字面量）
- [x] 更新 `src/main.rs`：`invoke_handler` 追加 `agent_start` / `agent_runs` / `agent_run_events`

## 阶段四：前端 Agent 调试页

- [x] 扩展 `src/types/dto.ts`：`AgentEnvMode` / `AgentPermissionMode` / `AgentRunStatus` 字面量 union、`AgentBlock` discriminated union、`AgentEvent`（kind 判别五变体）、`AgentRunRecord`（对齐 serde camelCase 线格式）
- [x] 新建 `src/views/agent/hooks/useAgentRun.ts`：`new Channel<AgentEvent>()` 订阅 + invoke `agent_start`（root / prompt / env / permissionMode）；events 累积、running / error / result 三态；组件不直接 invoke
- [x] 新建 `src/views/agent/hooks/useAgentRunHistory.ts`：invoke `agent_runs`（显式 refresh）+ `agent_run_events`（openRun 点开重放）；沿 `useChangeDetail` 形态，无轮询
- [x] 新建 `src/views/agent/components/AgentRunForm.tsx`：prompt 必填（空则禁用启动）、env 双档默认 default（bare 旁认证前提提示）、permission-mode 三档下拉默认 bypassPermissions；无 cwd / model 输入；导出 `AgentStartInput`
- [x] 新建 `src/views/agent/components/AgentEventTimeline.tsx`：对话流渲染、tool_use / tool_result 折叠块成对、`parentToolUseId` 子代理分组、result 汇总卡（numTurns / cost / duration / sessionId 可复制）；实时与重放共用
- [x] 新建 `src/views/agent/components/AgentRawStream.tsx`：逐事件 JSON dump 原始面板（含 Raw 变体原文）
- [x] 新建 `src/views/agent/components/AgentRunHistory.tsx`：run 列表（状态 / 时间 / 摘要）→ 点开重放 + 显式刷新按钮；run 结束不自动刷新
- [x] 新建 `src/views/agent/AgentDebugView.tsx`：组装参数面 + 实时时间线（原始流切换）+ 历史运行区；props `{ root: string | null }`
- [x] 更新 `src/components/AppSidebar.tsx`：导出 `TopPage` 类型；props 增 page / onPageChange；清单组上方新增「页面」导航组（[变更] [Agent 调试]）；workspace 清单组语义不变
- [x] 更新 `src/App.tsx`：顶层 `page` state（`TopPage`）；`SidebarInset` 内容区按 page 渲染 `ChangeView` 或 `AgentDebugView`；`useChangeList` 留在 App 层不随页面卸载

## 阶段五：守线自检（静态，不含测试执行）

- [x] 管线守线：`cd packages/desktop/src-tauri && cargo fmt && cargo clippy` 通过；`pnpm -C packages/desktop run client:check`（vp check --fix + knip）通过；产品 `.rs` 全文不含被禁磁盘目录名字面量（layout_test 约束）；无 kill / resume / partial / model 入口
