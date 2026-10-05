# 任务: sdk-engine-gap-closure

> 实现边界以 `design.md` 变更清单为准。测试编写 / 测试执行由 test-design → test-gen → test-execution 阶段承接，本列表不含任何测试任务；守线阶段仅静态检查。

## 阶段一：依赖切换 rig facade（拍板 #14 第一阶段先行）

- [x] `packages/desktop/src-tauri/Cargo.toml`：`[workspace.dependencies]` 新增 `rig = { version = "=0.42.0", default-features = false, features = ["reqwest", "native-tls", "memory"] }` 条目与 `regex = "1"` 条目；`rig-core` 既有条目保留，注释改写为解析钉子角色（facade 对 rig-core 为 `^0.42.0` 需求，双条目强制锁 0.42.0；提案条件性删除不触发）
- [x] `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`：`[dependencies]` 新增 `rig = { workspace = true }` 与 `regex = { workspace = true }`，`rig-core` 声明保留；注释同步 facade 语义
- [x] `crates/infra/agent/src/sdk/{loop,normalize,resume,runner,tools}.rs` 与 `sdk/{loop,normalize,resume}_test.rs`：`rig_core::` → `rig::` 路径迁移（测试文件仅路径迁移，断言语义不动）；`loop.rs` 三处「CLI 口径」注释改为 core 协议口径
- [x] spike 收口：编译通过即确认 facade re-export 面与 `memory` feature 拉入 rig-memory 成立；若 T1 触发（re-export 面缺失 / feature 名不符）立即 backtrack proposal，不进入后续阶段

## 阶段二：AGENT.md preamble 注入（AC-2）

- [x] 新增 `crates/infra/agent/src/sdk/preamble.rs`：`load(root: &Path) -> Option<String>`——读 `<root>/AGENT.md`，存在逐字返回（超 32KB 截 char 边界前缀）、缺席或任何读取失败返回 `None`、严格无兜底回退、单文件不级联
- [x] `crates/infra/agent/src/sdk/mod.rs`：注册 `preamble` 模块
- [x] `crates/infra/agent/src/sdk/loop.rs`：请求构造 `preamble` 字段改为 `preamble::load(&turn.cwd)` 每轮调用（每轮重读，会话中改动下一轮生效）

## 阶段三：工具质量与 L1 单结果上限（AC-3 / AC-5）

- [x] `crates/infra/agent/src/sdk/tools.rs`：read 工具缺省截断 `MAX_READ_LINES = 2000`（显式 limit 同上限），截断尾部留痕「已截断，可用 offset 翻页」形态，offset/limit 翻页语义保持
- [x] `crates/infra/agent/src/sdk/tools.rs`：grep 工具改 `regex` 正则匹配（非法正则 `Err`）、新增可选 `context` 上下文行参数（匹配行 ± N，缺省 0，窗口合并去重、不连续组间 `--` 分隔），200 条命中上限维持；description 与 schema 同步
- [x] `crates/infra/agent/src/sdk/tools.rs`：`execute` 分发出口统一 L1 收口 `MAX_RESULT_BYTES = 30_000`（Ok / Err 双路截断 + 留痕），全工具覆盖

## 阶段四：bash 第七工具（AC-4 / AC-5 / AC-10 实现面）

- [x] 新增 `crates/infra/agent/src/sdk/bash.rs`：`ShellBase` 枚举 + `probe_windows_shell(path_var: &OsStr) -> ShellBase` 纯探测（PATH 扫 `Git\bin\bash.exe` / `Git\usr\bin\bash.exe` 形态，未命中退 `Cmd`；unix 侧 `Sh`）
- [x] `crates/infra/agent/src/sdk/bash.rs`：`execute(root, input)` 执行体——`tokio::process`（`cwd = root`、env 继承、stdin 置空、`kill_on_drop(true)`、Windows `CREATE_NO_WINDOW`）、`timeout_ms` 钳位 `[1_000, 600_000]` 缺省 `120_000`、stdout/stderr 并发读合并（stdout 段在前、stderr 段带标记）、非零退出码 `Err(退出码 + 输出)`、超时走 `kill_process_tree`（`taskkill /PID <pid> /T /F` 形状复制，`cli/` 零 diff）
- [x] `crates/infra/agent/src/sdk/tools.rs`：`TOOL_NAMES` 扩至七工具、`definitions()` 增 bash 条目（`command` 必填、`timeout_ms` 可选）、`execute` 增 `"bash"` 分发臂
- [x] `crates/infra/agent/src/sdk/policy.rs`：新增执行面清单常量并将 bash 纳入决策表——Default 拒、AcceptEdits 与 BypassPermissions 放；模块头改写（三档语义 + bash 无沙箱知情边界措辞）
- [x] `crates/infra/agent/src/sdk/mod.rs`：注册 `bash` 模块

## 阶段五：上下文防线 L2 / L3（AC-6 / AC-7 / AC-8）

- [x] 新增 `crates/infra/agent/src/sdk/context.rs`：`ContextDefense`（`resolve(context_window: Option<u64>)` 缺省 128K；L2/L3 触发比 0.75/0.90；保护窗 40k；prune 门槛 20k）、`estimate_history`（serde_json 字节 ÷ 4）、`prune`（保护窗外单体 >20k 的老 tool_result 占位符替换且配对完整 → 不够丢最老完整轮对，恒保首条 user，孤儿配对清扫）、`hard_prune`（保首条 user + 保护窗、中间整段丢弃）
- [x] 新增 `crates/infra/agent/src/sdk/compact.rs`：`summarize(model, history, defense)`——中文摘要指令（「历史摘要，请勿重复已完成的工作」前缀 + 决策及其原因等要点集、禁工具）、待摘要段 = 首条 user 之后至保护窗之前、一次重试、成功返回 `[摘要, 首条 user, 保护窗]` 新史、失败 `Err`
- [x] `crates/infra/agent/src/sdk/mod.rs`：注册 `context` / `compact` 模块
- [x] `crates/infra/agent/src/sdk/loop.rs`：`LoopTurn` 增 `defense: ContextDefense`；每次发请求前 L2 prune → notice 转发 → 水位过 L3 触发比则 `summarize`（成功发 `context_compacted`、失败降级 `hard_prune` 并以 `fallback:true` 留痕），run 不失败收敛；store 转录面零改动（防线只作用请求史）
- [x] `crates/infra/agent/src/sdk/runner.rs`：`SdkRunner::new` 增第三参 `context_window: Option<u64>`、泵内装配 `ContextDefense`、resume 重建后执行一次 L2 prune 并先于 `RunStarted` 转发 notice（第一调用点）

## 阶段六：provider context_length 数据面（AC-9 后端）

- [x] `crates/infra/store/src/model.rs`：`AgentProviderRecord` 增 `context_length: Option<u64>`（`#[serde(default)]`）+ native_model `version = 1` → `2` + `AgentProviderRecordV1` legacy 结构与 `From` 双向 impl（v3→v4 先例同型，缺列读兼容）+ `new` 增参 + 手写 masked `Debug` 补字段
- [x] `crates/infra/agent/src/lib.rs`：`EngineFacade` 增 `context_window: Option<u64>` 字段与 `with_context_window` builder（`new()` / `with_resume_transcript` 缺省 `None` 不变），`runner_for` Sdk 臂传参
- [x] `crates/infra/agent/src/compose.rs`：`ResolvedEngine` 增 `context_window`，`resolve_agent_engine` Sdk 臂读 `provider.context_length`，facade 构造表达式串接 `with_context_window`（`SessionInjections` 恒传 default 行零改动）
- [x] `src-tauri/src/commands/agents/mod.rs`：`save_agent_provider` 增 `context_length: Option<u64>` 平参并传入记录构造（留空 = `None`，MUST NOT 落 0 或 128000 字面）
- [x] 执行 `pnpm -C packages/desktop run bindings:export`：再生成 `src/types/generated/bindings.ts` 与 `src-tauri/src/bindings/mod_test.rs` 快照（非手改）

## 阶段七：管理页（AC-9 前端）

- [x] `packages/desktop/src/views/agents/hooks/use-agent-providers.ts`：`AgentProviderSaveInput` 增 `contextLength: number | null`，save 调用透传
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`：`ProviderFormState` 增 `contextLength: string`（空串 = 未配置）、`PROVIDER_FIELDS` 增行、`toFormState` 映射（null → 空串）、保存边界 parse（空 → `null`、非正整数拒绝提交）、「跟随缺省 128K」语义提示在场

## 阶段八：守线（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）
- [x] `pnpm -C packages/desktop run client:check`（vp check + knip）
- [x] `pnpm -C packages/desktop run bindings:check`（再生成零 diff）
- [x] 零 diff 边界断言：`git diff --exit-code -- src-tauri/crates/core/agent` 与 `git diff --exit-code -- src-tauri/crates/infra/agent/src/cli`；compose.rs 的 diff 仅限 AC-9 接线行，`SessionInjections` 恒传 default 行目视核验不动；引擎配置结构与路径沙箱两个未触动模块零 diff
- [x] `rig_core::` 残留 grep 自查：`crates/infra/agent` 源码零命中
- [x] 测试编写与执行归 test-design → test-gen → test-execution 阶段承接（proposal AC 的工作区测试全绿过关线在彼处核验，本阶段仅静态检查）

## 阶段九：交付（归档时执行）

- [x] `packages/desktop/package.json`：`version` 0.4.7 → 0.4.8（AC-11，用户可见变更：SDK 引擎能力补全；随归档提交执行）
