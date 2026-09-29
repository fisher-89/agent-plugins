# 任务: desktop-rust-ts-type-sync

> 依赖排序：阶段一（PoC 门 + 依赖 pin）→ 阶段二（Rust enum 化前置，线格式零变化）→ 阶段三（specta 接入 + 生成物入库）→ 阶段四（挂点 + 守卫）→ 阶段五（前端一次性切换）→ 阶段六（shim 判据退役）→ 阶段七（守线）。
> 测试编写与测试执行不在本列表：proposal「测试文件」节所列各面由 test-design / test-gen 阶段承接，验收判据中的测试执行由 test-execution 阶段满足；守线阶段仅静态检查。

## 阶段一：PoC 前置门（实现第一步，先于一切接入）

- [x] 核实 `specta` / `specta-typescript` / `tauri-specta` 三件套当前 rc 版本号（lockstep），在 `packages/desktop/src-tauri/Cargo.toml` `[workspace.dependencies]` 以 `=` 精确 pin（specta 启用 `derive` / `serde_json` / `time` 特性）
- [x] 依赖落位：tauri-specta 与 specta-typescript 仅入根包 `[dependencies]`；`specta = { workspace = true }` 准入 `crates/core/agent/Cargo.toml`、`crates/core/workflow/Cargo.toml`、`crates/infra/store/Cargo.toml`（tauri-specta / specta-typescript MUST NOT 出现在 core/infra 任何 crate）
- [x] 为 `AgentEvent` 族（`event.rs` 三类型）加临时 `specta::Type` derive（serde attrs 逐字不动），搭建最小 bindings 模块与导出入口（先注册 `agent_start` 与 `list_changes` 两条命令）跑通首次导出
- [x] PoC 出线形态核对并回填 design.md「PoC 前置门」：`AgentEvent`（`tag="kind"` + `rename_all_fields` + flatten 信封）判别 union 同构性、`Channel<AgentRunMessage>` typed、`Result<T, String>` 错误通道形态、`serde_json::Value` 与 `OffsetDateTime` 出线口径、生成名与 dto.ts 名称差异名单
- [x] 导出幂等性核对：同输入连续两次导出，`git diff` 为空（无时间戳/机器路径嵌入）
- [x] PoC 不符预期即停线：六项验证全部通过，无需停线（无 ts-rs 回退触发）

## 阶段二：Rust enum 化前置（纯内政，线格式逐字不变）

- [x] 存量库三字段值域扫描（先于枚举化落地）：对既有 `home_dir()/.dev-team/desktop-store.redb` 经 db 查看命令面或一次性只读脚本枚举 `env` / `permission_mode` / `status` 全部现值，确认无野值，结论回填 design.md
- [x] `crates/core/agent/src/runner.rs` 新增 `AgentRunStatus` 枚举（`Running` / `Completed` / `Failed` / `Stopped`，`rename_all = "camelCase"` 与现值域逐字一致），`lib.rs` 导出
- [x] `crates/infra/store/src/model.rs`：`AgentRunRecord` 三字段（`env` / `permission_mode` / `status`）String → core/agent 枚举；native_model version 2 → 3（`from = AgentRunRecordV2`）
- [x] `crates/infra/store/src/model.rs` 新增 `AgentRunRecordV2` 版本化结构（现 16 字段形态，不注册 `#[native_db]`）与 `From` 转换链：v2→v3 升级（三字段字符串解析为枚举，野值 fail-fast panic 记因）、反向 downgrade 约束保留、v1→v2 既有转换接续链式升级
- [x] `src/commands/exec/agent.rs`：`running_record` / `drive_agent_run` / `abort_with_store_failure` 直写枚举（`AgentRunState` → `AgentRunStatus` 显式 match），`STATUS_RUNNING` / `STATUS_COMPLETED` / `STATUS_FAILED` / `STATUS_STOPPED` 四常量退役
- [x] `crates/infra/agent/src/flags.rs`：`--permission-mode` flag 值改由 serde 序列化派生（`serde_json::to_value` 取字符串，行为逐字不变）
- [x] 删除 `crates/core/agent/src/runner.rs` 的 `AgentEnvMode::as_str` 与 `AgentPermissionMode::as_str`（落库双轨口径退役），同步清理全部引用点

## 阶段三：specta 接入与生成物入库

- [x] `crates/core/agent/src/runner.rs`：`AgentEnvMode` / `AgentPermissionMode` / `AgentRunStatus` 加 `specta::Type`（收敛阶段一临时 derive）
- [x] `crates/core/agent/src/event.rs`：`AgentBlock` / `AgentEventKind` / `AgentEvent` derive 收敛为正式面
- [x] `crates/core/workflow/src/`：出线类型闭包补 `specta::Type`（`queries/list.rs`、`queries/detail.rs`、`queries/explore.rs`、`artifacts/envelope.rs`、`model/inventory.rs`、`model/workflow.rs`），语义不动
- [x] `crates/infra/store/src/`：`WorkspaceRecord` / `AgentRunRecord` / `ExploreRecord`（`model.rs`）与 `ModelInfo` / `RecordEnvelope`（`envelope.rs`）补 `specta::Type`
- [x] `src/commands/exec/agent.rs` 的 `AgentRunMessage` 与 `src/commands/watch/mod.rs` 的 `FileWatchEvent` 补 `specta::Type`
- [x] 根包 lib 化：新增 `src/lib.rs`（`pub mod bindings;` + `pub mod commands;`），`src/main.rs` 改从 lib 目标引用（setup、插件挂载、db 路径解析不动）
- [x] 新增 `src/bindings.rs`：`builder()` 以 `collect_commands!` 全量注册 22 条命令（`generate_handler!` 清单逐条迁移，events 面不引入）；`export_bindings()` 幂等导出（`CARGO_MANIFEST_DIR` 定位 `../src/types/generated/bindings.ts`，父目录创建）
- [x] 新增 `src/bin/export-bindings.rs`：bin 入口调用 `export_bindings()`
- [x] `src/main.rs`：`invoke_handler` 从 `generate_handler![...]` 切换为 `bindings::builder().invoke_handler()`，命令总数核对为 22 不变
- [x] 全量导出并入库 `src/types/generated/bindings.ts`：核对 22 条命令 typed 包装与全部出线 DTO 齐全、`agent_start` / `watch_subscribe` 的 Channel 参数 typed

## 阶段四：check/build 挂点与一致性守卫

- [x] `packages/desktop/package.json`：新增 `bindings:export`（`cd src-tauri && cargo run --quiet --bin export-bindings`）与 `bindings:check`（`pnpm run bindings:export && git diff --exit-code -- src/types/generated`）脚本
- [x] `packages/desktop/package.json`：`check` 与 `build` 前置串接 `bindings:check`
- [x] `packages/desktop/knip.json`：`ignore` 增 `src/types/generated/**`
- [x] 守卫行为核对：人工改动一处 Rust 类型字段且不重导出时，`bindings:check` 以非零退出报工作区 diff；核对后还原

## 阶段五：前端一次性切换（双轨共存期为零，不分批）

- [x] `src/lib/agent-transport.ts`：删除手写 `AgentRunMessage` / `AgentStartChainParams` 镜像，改用生成绑定与生成类型；`readChainParams` / `readPermissionMode` / `readNullableString` / `readNullableNumber` 运行时校验原样保留；`new Channel<AgentRunMessage>()` 底层构造保留（类型来自生成物）
- [x] hooks 轨道机械替换：`src/hooks/use-agent-chat.ts`、`src/hooks/use-change-list.ts`、`src/hooks/use-workspaces.ts` 裸 invoke → typed bindings
- [x] views 轨道机械替换：`src/views/agent/hooks/use-agent-run-history.ts`、`src/views/db/hooks/use-db-inspector.ts`、`src/views/explores/hooks/use-explore-doc.ts`、`src/views/explores/hooks/use-explore-list.ts`、`src/views/explores/components/explore-create-dialog.tsx`、`src/views/changes/hooks/use-change-detail.ts` 裸 invoke → typed bindings
- [x] `src/types/dto.ts` 收敛为纯 re-export shim：`export type * from './generated/bindings'` + 按阶段一名单的逐名映射（实测零逐名映射）；无任何手写 interface / type 定义残留
- [x] 生成绑定 `Result<T, String>` 错误通道的前端接法落地（`ErrorHandlingMode::Throw`：`Promise<T>` reject，hook `.catch → error 态` 接线逐字不变）
- [x] 裸 invoke 清零自检：以命令名字符串模式扫描 `src/`（排除生成物与测试内断言）确认业务代码零 `invoke('command_name')` 直接调用

## 阶段六：dto.ts 判据驱动退役

- [x] 以 knip 报告核对 shim 引用计数：零引用即删除 `src/types/dto.ts`；非零则按报告收口漏网 import 后再删（判据驱动，不按日历时间过渡）。**核对结论**：引用非零且跨出本 change 清单——25 个非测试类型消费文件（清单外，design 仅列 10 个 invoke 调用文件为切换面）+ 约 30 个测试文件（归 test-gen 阶段，测试文件不在本清单）仍 import shim；tsconfig 含全部 `src`，shim 删除即 tsc 红，非测试侧单侧收口则 knip（project 排除测试）反报 shim 未用。判据未达即不删，shim 以纯 re-export 形态留置（零运行时代价）；退役落点 = test-gen 阶段测试 import 收口后（design「删除文件」节明示允许「紧随 change」落地），不按日历时间过渡

## 阶段七：守线（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run check` 全绿：`bindings:check` 零 diff + `client:check`（tsc + knip）+ `server:check`（cargo fmt + clippy）
- [x] 变更清单复核：design.md 变更清单各条目与实际落地文件一致；清单外漂移仅两处且已回填 design——`Cargo.lock`（依赖 pin 的机械伴生）与 `src/views/changes/flow/attachments.ts`（FileLogEntry 双相 union 的 attempt 判空收窄，见「PoC 前置门」已知形态差异）
- [x] `packages/desktop/package.json` `version` 收尾 bump（0.3.5 → 0.3.6）
- [x] 测试执行归 test-execution 阶段承接（本阶段仅静态检查，不含任何测试运行）
