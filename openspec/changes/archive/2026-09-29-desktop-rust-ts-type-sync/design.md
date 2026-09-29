# 设计: desktop-rust-ts-type-sync

> **变更**: desktop-rust-ts-type-sync
> **日期**: 2026-09-28

---

## 提案与规格同步状态

- `proposal.md` 与 `specs/desktop-ipc-type-bindings/spec.md`、`specs/desktop-workspace-store/spec.md` 已由 proposal 阶段写入并过审（eval pass），本设计不重复其内容，也不把它们列入变更清单与任务。
- proposal「测试文件」节所列五组测试（runner_test / store_test / 命令层测试 / 前端 vp 测试 / 导出幂等性）归 test-design / test-gen / test-execution 阶段承接，不入本清单与任务列表。
- 本 design 为首轮（无回溯差异表）。

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| specta builder 组装 + 导出入口 | 全部 22 条命令的注册唯一入口与幂等类型导出（`export(Typescript, path)`） | `packages/desktop/src-tauri/src/lib.rs`、`src/bindings.rs`、`src/bin/export-bindings.rs`（均新增） | tauri-specta、specta-typescript、`commands` | tauri-specta v2（rc，`=` pin） |
| core/agent 导出 derive 面 | 运行枚举（含新增 `AgentRunStatus`）与事件族的类型导出面，枚举语义与 serde 线格式不变 | `crates/core/agent/src/runner.rs`、`event.rs`、`lib.rs` | specta（普通 derive crate，非 Tauri 系） | `specta::Type` derive |
| core/workflow 导出 derive 面 | 出线查询类型闭包的 derive 面，语义不动 | `crates/core/workflow/src/queries/`、`artifacts/`、`model/` | specta | `specta::Type` derive |
| store 模型枚举化 + derive 面 | `AgentRunRecord` 三字段类型化 + native_model v2→v3 原地演进 + 出线模型 derive | `crates/infra/store/src/model.rs` | specta、native_model、native_db | native_model 版本机制（禁 legacy 迁移层） |
| app 命令层信封 derive + 枚举直写 | `AgentRunMessage`（tag `ipc`）与 `FileWatchEvent` 的 typed 化；落库写入直写枚举 | `src/commands/exec/agent.rs`、`src/commands/watch/mod.rs` | tauri（`tauri::ipc::Channel`）、specta | tauri-specta 命令宏面 |
| 生成物 bindings | 前端唯一 IPC 类型与调用面（22 条命令 typed 包装 + 全部出线 DTO），入库版本控制 | `packages/desktop/src/types/generated/bindings.ts`（生成、入库） | — | specta-typescript 产物 |
| dto.ts shim | 一次性切换的过渡容错网：纯 re-export（含必要的逐名映射），判据驱动退役 | `packages/desktop/src/types/dto.ts` | 生成物 | `export type *` |
| 前端消费面 | transport 与全部 invoke 调用点切 typed bindings；运行时校验保留 | `src/lib/agent-transport.ts`、`src/hooks/`、`src/views/`（10 个调用文件） | 生成物、`@tauri-apps/api`（仅 `Channel` 底层构造保留） | 机械替换 |
| 守卫与脚本链 | check / build 前置重导出 + `git diff` 一致性守卫 + knip 边界 | `packages/desktop/package.json`、`knip.json` | git、cargo run | npm scripts |

## 设计定夺（proposal 待决问题闭环）

| 待决问题 | 定夺 | 理由 / 被拒备选 |
|----|------|------|
| 生成物落位 | `src/types/generated/bindings.ts` 单文件 | 手写代码（`src/lib`、`src/hooks`）与生成物目录级分离，knip ignore 一条规则覆盖全部生成导出；单文件贴合 specta 导出默认形态。被拒：`src/lib/bindings.ts`（生成物混入手写 lib 目录，ignore 边界模糊） |
| 导出入口形态 | 专用 bin `src/bin/export-bindings.rs` + 根包 lib 化（`src/lib.rs` 暴露 `bindings` / `commands`） | 重导出是构建步骤不是测试：bin 让 check/build 链以 `cargo run --bin export-bindings` 显式触发，检查链保持零测试执行；lib 化同时是 Tauri 2 上游模板形态（main 薄壳 + lib 承载模块）。被拒：`#[test]` 搭测试套件顺风车（副作用测试气味；检查链需串入按名过滤的测试运行命令，违背检查链静态纪律；且根包现为纯二进制 crate，测试入口够不到 `mod bindings` 需先 lib 化——那不如直接上 bin） |
| v2→v3 演进遇野值 | fail-fast panic（记因，含原字符串值） | native_model `from` 升级走 `From` 通道，无 `Result` 可传播；存量值域扫描先行（写入侧历史上只有枚举化前常量，无野值来源）确保该分支不可达。被拒：兜底默认变体（把数据损坏伪装成合法状态，违背本仓库 fail-fast 无静默降级纪律）；报错传播（`From` 通道装不下，需改 native_model 接入形态，收益为零） |
| flags.rs 字符串单一来源 | serde 序列化派生：`serde_json::to_value(mode)` 取字符串 | 「与 serde 线格式一致」由机制保证而非人肉口径，`as_str` 双轨彻底退役且不再留第二份字符串副本（保留 CLI 值域 helper 正是本变更要消灭的形态）；`agent-cli` 已依赖 serde_json，零新依赖。被拒：保留改名 helper（如 `cli_value()`，仍是第二份副本） |
| check / build 挂点 | 新增 `bindings:export`（重导出）与 `bindings:check`（重导出 + `git diff --exit-code`）脚本；`check` 与 `build` 前置串接 `bindings:check` | 守卫在重导出之后跑 diff：重导出后工作区仍非干净 ⇔ 入库物已过期，语义精确对应「改 Rust 类型不重导」场景；`build`（`tauri build`）与 `check` 同门径。挂 `server:check`（fmt/clippy）内不合适——守卫是跨栈一致性，归整条 `check` 头部 |
| 守卫口径边界 | `git diff --exit-code -- src/types/generated`（worktree vs index） | 覆盖主场景「改 Rust 后不重导出提交」与「重导了但未入库」；已知边界：若把过期生成物与源码一起提交（index 侧已脏），守卫通过——该窗口由 review 面（diff 中生成物与源码必须同批出现）兜底，机械闭合需临时目录比对 HEAD，收益不成比例 |
| 前端切换面 | 实测源码 invoke 调用点为 **10 个文件**（proposal「约 27」为含测试 mock 面的估算） | grep 实证：`src/hooks` 3、`src/views` 6、`src/lib/agent-transport.ts` 1；`app.tsx` 无 invoke 调用。测试文件 mock 的是 `@tauri-apps/api/core` 模块，生成绑定底层仍走该模块的 `invoke`，mock 机制切换后依旧生效，类型面跟改归测试阶段 |
| `AgentRunStatus` 与 `AgentRunState` 关系 | 两型并存、显式 `match` 映射，不合并 | `AgentRunState` 是 core 状态机内存态（无 serde 面、不落库），`AgentRunStatus` 是落库/出线契约；合并会把状态机类型暴露进线格式契约，违背 core 契约 crate 的 derive 边界现状 |

## PoC 前置门（实现第一步，先于一切切换）

PoC 在 bindings 模块以最小命令集（`agent_start` + `list_changes` + `get_change_detail` + `read_artifact`）跑通导出，验证对象与通过判据：

| 验证对象 | 通过判据 | PoC 实测结论（已回填） |
|----------|----------|----------|
| `AgentEvent`（`tag="kind"` + `rename_all_fields = "camelCase"` 内部标签枚举 + `#[serde(flatten)]` 信封） | 出线为 `seq` / `timestampMs` 摊平、`kind` 判别的 discriminated union，与 `dto.ts` 现镜像同构（变体字面量为 camelCase 小写值域） | **通过**。出线 `{ seq, timestampMs } & AgentEventKind`，`AgentEventKind` 为 `kind: "runStarted" \| "message" \| "systemNotice" \| "runResult" \| "raw"` 判别 union，字面量与字段名和 dto.ts 逐字一致（`{A} & (B \| C)` 与 dto.ts 交并分配形互为同构，收窄语义不变） |
| `Channel<AgentRunMessage>` / `Channel<FileWatchEvent>` | TS 侧为 typed `Channel` 参数，信封类型（tag `ipc` 双变体 / watch 载荷）来自生成物 | **通过**。`onEvent: Channel<AgentRunMessage>` typed（tauri 需启用 `specta` feature 提供 `Channel<T>` 远程 Type impl）；信封 `{ ipc: "event" } \| { ipc: "record" }` 判别 union 与 transport 手写镜像同构 |
| `Result<T, String>` 命令错误通道 | 生成绑定的错误面形态（tauri-specta `Result` 包装或 Promise reject）留档，前端 hook error 态接法随之确定 | **定夺：`ErrorHandlingMode::Throw`**。生成绑定为 `Promise<T>` reject 语义（`commands.listChanges(root)`），与前端既有 `.catch → error 态` 接线逐字兼容；被拒：默认 `Result` 模式（`{ status: "ok" \| "error" }` 包装会迫使全部调用点逐个拆包，错误处理从 reject 退化为可漏检的状态检查） |
| `serde_json::Value` 字段（`payload` / `usage` / `input` / `RecordEnvelope.key/value`） | 出线口径留档（`JsonValue` 或等价物）；与 dto.ts 现 `unknown` 的差异纳入切换期 tsc 拦截清单 | **通过（需一处语义规则）**。`Value` 为递归类型（`Value → Vec<Value> → Value`），直接出线报 recursive inline 错误；经 builder `semantic_types` 规则 `define::<serde_json::Value>(→ "unknown")` 改写，出线 `unknown` 与 dto.ts 现口径**逐字一致**（零差异，无 tsc 拦截项） |
| `time::OffsetDateTime`（`AttemptRecord` 等自定义 lenient 序列化） | specta `time` 特性下出线 `string`，与 dto.ts 现 `string \| null` 一致 | **通过（需字段级 type 覆盖）**。lenient 序列化（`serialize_with`/`deserialize_with`）被 specta 拒绝出线，报「Unsupported serde attribute」；对 lenient 时间戳字段加 `#[specta(type = Option<String>)]` 后出线 `string \| null`，与 dto.ts 逐字一致。64 位整型（时间戳/id）另需 `.dangerously_cast_bigints_to_number()`（specta-typescript 默认禁 i64/u64 出线防精度丢失；本项目值域为毫秒时间戳与 run id，远低于 2^53，与 serde_json number 线格式及 dto.ts `number` 口径一致）。**全量接入期追加发现**：serde `alias`（磁盘旧代际 snake_case 宽松解析）入镜像会出线 `start_at/startAt` 双拼 union 且渲染缺基字段（specta-serde 对含 alias 交集的括号缺陷），TS 属性访问不可用——经 `#[specta(remote)]` 投影结构（`specta_face` 子模块，真类型 serde attrs 逐字不动）实现 `ActivePhase` / `InterruptedEntry` 的 Type，出线与 dto.ts 逐字一致 |
| 导出幂等性 | 同输入连续两次导出，`git diff` 为空（无时间戳/绝对路径嵌入） | **通过**。连续两次导出 `git diff` 为空；文件头为固定注释（无时间戳），类型排序确定性 |

- PoC 结论（出线形态、生成名与 dto.ts 名称差异名单、逐名映射需求）**回填本 design**；回填后阶段五的机械替换才有权威依据。
- 不符预期即停线：回退备选方案（ts-rs 只生成类型 + 手写薄 invoke 层）并重新评审，本 change 其余阶段不作前置假设。

### PoC 生成名与 dto.ts 名称差异名单（阶段五机械替换权威依据）

- **命名直通**：类型名（`AgentEvent` / `AgentBlock` / `ChangeList` / `ChangeSummary` / `AttemptRecord` / `PhaseEntry` / `ChangeDetail` / `ArtifactEnvelope` / `ArtifactDescriptor` / `WorkspaceRecord` / `AgentRunRecord` / `ExploreRecord` / `ExploreDoc` / `ExploreScanEntry` / `FileWatchEvent` / `ModelInfo` / `RecordEnvelope`）与字段名（camelCase）全部与 dto.ts 一致，零逐名映射；dto.ts 未导出的内部类型（`Verdict` / `ChangeSource` / `FileLogOp` / `Inventory` / `ArchiveGroup` / `ActivePhase` / `InterruptedEntry` / `FileLogEntry` / `AgentEventKind` / `AgentEnvMode` / `AgentPermissionMode` / `AgentRunStatus` / `AgentRunMessage`）由生成物新导出，shim 无需映射。
- **命令包装**：`commands` 对象 + camelCase 方法名（`listChanges` / `getChangeDetail` / `readArtifact` / `agentStart` …），替代裸 `invoke<T>('snake_name', { camelArgs })`。
- **已知形态差异**（与 dto.ts 对比，均为宽松方向、不阻断替换）：
  - `#[serde(default)]` 字段（`AgentRunRecord.source` / `sourceRef` / `parentRunId`、`FileLogEntry.attempt` 等）出线为可选属性（`source?: string`），跨相级联时呈 serialize/deserialize 双相 union（`ChangeDetail` / `FileLogEntry`）——线格式实际恒有值（default 仅影响反序列化缺省）；唯一消费点 `attachments.ts` 的 attempt 判空以宽松 `== null` 覆盖 undefined，其余读取点 tsc 不拦截；
  - `ActivePhase` / `InterruptedEntry` 经 `specta_face` remote 投影出线（单相 camelCase、与 dto.ts 逐字一致），磁盘 alias 解析语义留在真类型；
  - dto.ts 私有类型（`Verdict` / `ChangeSource` 等）生成物全部具名导出，前端若需引用改从生成物取。
- **错误面**：`Result<T, String>` 命令生成 `Promise<T>`（reject），`Option<T>` 命令生成 `Promise<T | null>`——与裸 invoke 行为一致，hook 无需接法改动。

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。覆盖 proposal「变更范围-实现文件」全部条目；测试文件不在本清单（归测试阶段）。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/src/lib.rs` | 根包 lib 化入口：`pub mod bindings;` + `pub mod commands;`（Tauri 2 上游模板形态；main 与导出 bin 共享同一命令面） |
| `packages/desktop/src-tauri/src/bindings.rs` | specta builder 组装：`collect_commands!` 全量 22 条命令 + `export_bindings()` 幂等导出（`CARGO_MANIFEST_DIR` 定位生成路径，父目录 `create_dir_all`，输出无时间戳/机器路径） |
| `packages/desktop/src-tauri/src/bin/export-bindings.rs` | 导出 bin 入口：调用 `bindings::export_bindings()`，供 `cargo run --bin export-bindings` 触发 |
| `packages/desktop/src/types/generated/bindings.ts` | 生成物入库（首次全量导出后提交）：22 条命令 typed 包装 + 全部出线 DTO 类型 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace.dependencies]` 新增 `specta` / `specta-typescript` / `tauri-specta` 三件套 `=` 精确 pin（specta 启用 `derive` / `serde_json` / `time` 特性）；根包 `[dependencies]` 增 tauri-specta 与 specta-typescript | 三件套 lockstep（RC 间无 semver 兼容，升级事件驱动不追新）；tauri-specta / specta-typescript 仅落 desktop-app 壳层 |
| `packages/desktop/src-tauri/crates/core/agent/Cargo.toml` | `[dependencies]` 增 `specta = { workspace = true }` | specta 准入 core（普通类型导出 derive crate，同 serde 先例；「core/infra 禁 Tauri」不破，依赖方向不变） |
| `packages/desktop/src-tauri/crates/core/workflow/Cargo.toml` | 同上 | 同上 |
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | 同上 | 同上 |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 `AgentRunStatus` 枚举（`Running` / `Completed` / `Failed` / `Stopped`，`rename_all = "camelCase"` 与现值域逐字一致）；`AgentEnvMode` / `AgentPermissionMode` 加 `specta::Type`；删除两枚举的 `as_str()`（「与 serde 线格式一致」落库双轨口径退役） | 纯内政前置，先于 specta 接入；`AgentRunner` trait 面不动 |
| `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` | 导出 `AgentRunStatus` | 与既有两枚举同列 |
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | `AgentBlock` / `AgentEventKind` / `AgentEvent` 加 `specta::Type` | serde attrs（`tag` / `rename_all` / `rename_all_fields` / `flatten`）逐字不动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | `ChangeList` / `ChangeSummary` / `ArchiveGroup` / `ChangeSource` 加 `specta::Type` | 语义不动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `ChangeDetail` / `AttemptRecord` / `PhaseEntry` 加 `specta::Type` | 同上 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/explore.rs` | `ExploreDoc` / `ExploreScanEntry` 加 `specta::Type` | 同上 |
| `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/envelope.rs` | `ArtifactEnvelope` / `ArtifactDescriptor` 加 `specta::Type` | `payload: serde_json::Value` 出线口径随 PoC |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/inventory.rs` | `Inventory` 加 `specta::Type` | 已是真枚举，仅加 derive |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | `Verdict` / `FileLogOp` / `ChecklistItem` / `FileLogEntry` / `ActivePhase` / `InterruptedEntry` 加 `specta::Type` | 已是真枚举，仅加 derive；`Verdict::as_str` 不在退役范围（workflow 域自用，非落库双轨） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `AgentRunRecord` 三字段（`env` / `permission_mode` / `status`）String → core/agent 枚举；`native_model` version 2 → 3（`from = AgentRunRecordV2`）；新增 `AgentRunRecordV2` 版本化结构（现 16 字段形态）与 `From` 转换链；store 出线模型（`WorkspaceRecord` / `AgentRunRecord` / `ExploreRecord` / `ModelInfo` / `RecordEnvelope`，后者在 `envelope.rs`）加 `specta::Type` | v1→v2 转换既有不动，v1 存量经 v1→v2→v3 链式自动升级；`source` / `source_ref` 保持 String；无手工迁移、无 legacy 迁移层 |
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | `ModelInfo` / `RecordEnvelope` 加 `specta::Type` | db 命令出线信封 |
| `packages/desktop/src-tauri/crates/infra/agent/src/flags.rs` | `--permission-mode` flag 值改由 serde 序列化派生（`serde_json::to_value` 取字符串），替换 `AgentPermissionMode::as_str` 引用 | 单一来源定夺见「设计定夺」；flag 组装行为逐字不变（`acceptEdits` 值不变）；`agent-cli` 已依赖 serde_json，零新依赖；其余 flag 组装与租户语义不动 |
| `packages/desktop/src-tauri/src/main.rs` | `invoke_handler` 从 `generate_handler![...]` 切换为 `bindings::builder().invoke_handler()`；`mod commands;` 声明改从 lib 目标引用 | setup / 插件挂载 / db 路径解析不动；命令清单逐条迁移，总数 22 不变 |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | `AgentRunMessage` 加 `specta::Type`；`running_record` / `drive_agent_run` / `abort_with_store_failure` 直写枚举（`AgentRunState` → `AgentRunStatus` 显式 match）；`STATUS_RUNNING` / `STATUS_COMPLETED` / `STATUS_FAILED` / `STATUS_STOPPED` 四常量退役 | `as_str().to_owned()` 字符串降级随之消失；编排结构（提前 resolve、tee 双 sink、EOF 收敛优先级）不动 |
| `packages/desktop/src-tauri/src/commands/watch/mod.rs` | `FileWatchEvent` 加 `specta::Type` | 命令签名不动；桥接线程与幂等键逻辑不动 |
| `packages/desktop/src/types/dto.ts` | 收敛为纯 re-export shim：`export type * from './generated/bindings'` + PoC 名单内必要的逐名映射 | 无任何手写 interface / type 定义残留；退役见「删除文件」 |
| `packages/desktop/src/lib/agent-transport.ts` | 删除手写 `AgentRunMessage` / `AgentStartChainParams` 镜像，改用生成绑定与生成类型；`readChainParams` / `readPermissionMode` / `readNullableString` / `readNullableNumber` 运行时校验原样保留 | `new Channel<AgentRunMessage>()` 保留（`@tauri-apps/api` 底层构造，类型来自生成物） |
| `packages/desktop/src/hooks/use-agent-chat.ts` | `agent_run_chain` / `agent_run_events` / `agent_stop` 裸 invoke → typed bindings | 机械替换 |
| `packages/desktop/src/hooks/use-change-list.ts` | `list_changes` 同上 | 同上 |
| `packages/desktop/src/hooks/use-workspaces.ts` | `list_workspaces` / `add_workspace` / `remove_workspace` 同上 | 同上 |
| `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | `agent_runs` / `agent_run_events` 同上 | 同上 |
| `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | `db_models` / `db_records` 同上 | 同上 |
| `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | `read_explore` / `explore_doc_path` / `watch_subscribe` / `watch_unsubscribe` 同上 | `Channel<FileWatchEvent>` 底层构造保留，类型来自生成物 |
| `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | `list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record` 同上 | 同上 |
| `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` | `scan_explores` / `create_explore_record` 同上 | 同上 |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | `get_change_detail` / `read_artifact` 同上 | 同上 |
| `packages/desktop/knip.json` | `ignore` 增 `src/types/generated/**` | 生成物非手写代码，未引用导出不报 dead code |
| `packages/desktop/package.json` | scripts 新增 `bindings:export`（`cd src-tauri && cargo run --quiet --bin export-bindings`）与 `bindings:check`（`pnpm run bindings:export && git diff --exit-code -- src/types/generated`）；`check` 与 `build` 前置串接 `bindings:check`；版本收尾 bump | 守卫链语义见「设计定夺」 |

<!-- proposal 以目录粒度列出的 `src/commands/`（queries / workspaces / explores / db 四轨道）实测零改动：命令签名已满足 specta 面（入参/返回类型全部 `Type`），注册面集中 bindings.rs，derive 落 workflow/store 侧；`src/app.tsx` 实测无 invoke 调用，不在切换面。 -->

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/types/dto.ts` | 判据驱动退役：shim 化后 knip 报零引用即删（允许本 change 收尾任务或紧随 change，不按日历时间过渡） |

### 公共函数 / API

<!-- 仅列模块级导出函数与导出类型的公共 API；命令签名整体零变更，以汇聚行说明。 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `builder` | `src-tauri/src/bindings.rs` | 新增 | `pub fn builder() -> tauri_specta::Builder<tauri::Wry>` | 22 条命令全量注册（迁自 `generate_handler!`），main 与导出共用；events 面不引入 |
| `export_bindings` | `src-tauri/src/bindings.rs` | 新增 | `pub fn export_bindings() -> Result<(), String>` | 幂等导出 TS bindings 到 `src/types/generated/bindings.ts` |
| `main` | `src-tauri/src/bin/export-bindings.rs` | 新增 | `fn main()` | 导出 bin 入口 |
| `AgentEnvMode::as_str` | `crates/core/agent/src/runner.rs` | 删除 | （原 `pub fn as_str(self) -> &'static str`） | 落库双轨口径退役；flag 组装改 serde 派生 |
| `AgentPermissionMode::as_str` | `crates/core/agent/src/runner.rs` | 删除 | （原 `pub fn as_str(self) -> &'static str`） | 同上 |
| 22 条 `#[tauri::command]`（`list_changes` / `get_change_detail` / `read_artifact` / `list_workspaces` / `add_workspace` / `remove_workspace` / `agent_start` / `agent_stop` / `agent_runs` / `agent_run_events` / `agent_run_chain` / `read_explore` / `scan_explores` / `explore_doc_path` / `list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record` / `watch_subscribe` / `watch_unsubscribe` / `db_models` / `db_records`） | `src/commands/**` | 修改（注册面） | Rust 签名零变更 | 校验面变化：命令名 / 参数 key camelCase / 返回类型转编译期校验（生成绑定），前端裸字符串 invoke 禁用 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `AgentRunStatus` | `crates/core/agent/src/runner.rs` | 新增 | 四变体 `Running` / `Completed` / `Failed` / `Stopped`；serde camelCase 值域 `running` / `completed` / `failed` / `stopped` 与现 String 值域逐字一致；derive `specta::Type` |
| `AgentEnvMode` / `AgentPermissionMode` | `crates/core/agent/src/runner.rs` | 修改 | 加 `specta::Type`；`as_str` 删除；变体与 serde attrs 不动 |
| `AgentBlock` / `AgentEventKind` / `AgentEvent` | `crates/core/agent/src/event.rs` | 修改 | 加 `specta::Type`；内部标签 / flatten 形态不动（PoC 验证出线同构性） |
| workflow 出线闭包（`ChangeList` / `ChangeSummary` / `ArchiveGroup` / `ChangeSource` / `ChangeDetail` / `AttemptRecord` / `PhaseEntry` / `ActivePhase` / `InterruptedEntry` / `FileLogEntry` / `FileLogOp` / `ChecklistItem` / `Verdict` / `Inventory` / `ArtifactEnvelope` / `ArtifactDescriptor` / `ExploreDoc` / `ExploreScanEntry`） | `crates/core/workflow/src/` | 修改 | 仅加 `specta::Type`，语义不动 |
| `AgentRunRecord` | `crates/infra/store/src/model.rs` | 修改 | 三字段 String → 枚举；native_model v3；加 `specta::Type`；serde JSON 线格式逐字不变（bincode 落库形态变化由版本机制承接） |
| `AgentRunRecordV2` | `crates/infra/store/src/model.rs` | 新增 | 枚举化前 16 字段形态的版本化结构（不注册 `#[native_db]`），承载 v2 载荷解码与 v2→v3 升级 |
| `WorkspaceRecord` / `ExploreRecord` / `ModelInfo` / `RecordEnvelope` | `crates/infra/store/src/` | 修改 | 加 `specta::Type` |
| `AgentRunMessage` | `src/commands/exec/agent.rs` | 修改 | 加 `specta::Type`（tag `ipc` 双变体信封）；变体语义不动 |
| `FileWatchEvent` | `src/commands/watch/mod.rs` | 修改 | 加 `specta::Type` |
| 生成 TS 类型全集（`commands` 绑定对象 + 上述全部 Rust 类型的 TS 镜像） | `src/types/generated/bindings.ts` | 新增 | 生成物；dto.ts 旧镜像的类型提供者；knip ignore |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `specta` / `specta-typescript` / `tauri-specta` | `src-tauri/Cargo.toml` `[workspace.dependencies]` | 新增 | `specta = "=2.0.0-rc.25"`（features `derive` / `serde_json` / `time`）、`specta-typescript = "=0.0.12"`、`tauri-specta = "=2.0.0-rc.25"`（feature `typescript`）——crates.io 核实回填（tauri-specta rc.25 官方配对即 specta rc.25 × specta-typescript 0.0.12） | 三件套 lockstep；另 tauri 加 `specta` feature（`Channel<T>` 远程 Type impl 来源） |
| `scripts.bindings:export` | `packages/desktop/package.json` | 新增 | `cd src-tauri && cargo run --quiet --bin export-bindings` | 重导出入口（非测试执行） |
| `scripts.bindings:check` | `packages/desktop/package.json` | 新增 | `pnpm run bindings:export && git diff --exit-code -- src/types/generated` | 一致性守卫：重导出后工作区必须干净 |
| `scripts.check` | `packages/desktop/package.json` | 修改 | `bindings:check` 前置 | 守卫挂点 |
| `scripts.build` | `packages/desktop/package.json` | 修改 | `bindings:check` 前置 | 构建前保证生成物新鲜 |
| `ignore` | `packages/desktop/knip.json` | 新增 | `["src/types/generated/**"]` | 生成目录不报 dead code |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `AgentRunRecord`（v3） | `id`（主键）、`prompt`、`cwd`、`env: AgentEnvMode`、`permission_mode: AgentPermissionMode`、`status: AgentRunStatus`、`started_at`、`finished_at?`、`num_turns?`、`cost_usd?`、`duration_ms?`、`session_id?`、`error?`、`source`（String，缺省 `debug`）、`source_ref?`（String）、`parent_run_id?` | `parent_run_id` 自引用链（链还原经 store 单点）；`run_id` 关联 `AgentEventRecord` 1:N；`source` + `source_ref` 指向 explore 记录 | native_db（redb，user db）；native_model `id=2, version=3, from=AgentRunRecordV2`；bincode 编码 |
| `AgentRunRecordV2`（新增版本化结构） | 枚举化前 16 字段形态（三字段为 String） | 仅作 v3 的 `from` 前驱，不注册 `#[native_db]` | 只存在于升级转换路径（v2 载荷解码中转） |
| `AgentRunRecordV1`（既有） | 13 字段形态 | v1→v2 转换既有不动，v1 存量经 v1→v2→v3 链式自动升级 | 同上 |
| `AgentEventRecord` | 不变（`event_key` 打包键 / `run_id` 二级索引 / `event: AgentEvent`） | 同上 | native_model `with=SerdeJsonCodec`（flatten 要求自描述编码） |
| `WorkspaceRecord` / `ExploreRecord` | 不变 | — | native_db；仅加 `specta::Type` |

演进纪律：serde camelCase JSON 线格式逐字不变（枚举 unit variant 序列化值 == 旧 String 值域）；落库 bincode 编码形态必然变化（String 长度前缀 ≠ 变体索引），由 native_model 版本机制原地承接；存量值域扫描先行、野值 fail-fast（见「设计定夺」）；MUST NOT 重新引入 legacy 迁移层。

存量值域扫描结论（枚举化前落地，一次性只读 bin 扫 `home_dir()/.dev-team/desktop-store.redb` 后即删）：`agent_run` 共 4 条记录，`env` 值域 `{"default"}`、`permission_mode` 值域 `{"bypassPermissions"}`、`status` 值域 `{"completed", "stopped"}`——全部落在受控值域内，**无野值**，v2→v3 升级的 fail-fast panic 分支不可达。

## 路由/API 设计

<!-- 本变更不涉及 HTTP API。IPC 命令面 22 条的 Rust 签名零变更（见「公共函数 / API」汇聚行），变化在于注册通道（generate_handler! → specta builder）与前端调用面（裸 invoke → typed bindings）。 -->

## 依赖

### 运行时依赖

- `specta`（`=` pin，features `derive` / `serde_json` / `time`）— 类型导出 derive；普通 derive crate（同 serde 先例），准入 core/agent、core/workflow、infra/store，「core/infra 禁 Tauri」约束不破，依赖方向全部不变
- `tauri-specta`（`=` pin）— builder 注册 + 命令绑定生成；仅落 desktop-app 壳层（Tauri 系依赖不出壳层）
- `specta-typescript`（`=` pin）— TS 输出后端；仅落 desktop-app 壳层
- 供应链纪律：三件套 lockstep、`=` 精确 pin、升级事件驱动不追新（RC 间无 semver 兼容）；备选降级路径 ts-rs（只类型）/ taurpc 已调研留痕于探索记录

### 构建/测试依赖

- 无新增（导出 bin 复用根包既有依赖；`cargo run --bin export-bindings` 为既有工具链用法）

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1（新增 `AgentRunStatus`；store 三字段枚举化；`as_str()` 双轨退役） | `runner.rs` 新枚举变体钉死值域（serde camelCase 与现值域逐字一致）；`model.rs` 三字段类型化；两枚举 `as_str` 删除、`STATUS_*` 四常量退役、flags.rs 改 serde 派生（行为逐字不变）。serde JSON 线格式逐字断言与回归由测试阶段承接；判据中的测试执行归 test-execution 阶段满足 |
| AC-2（存量值域扫描 + native_model 版本演进） | 值域扫描为阶段二首任务（结论回填 design）；v3 + `AgentRunRecordV2` + `From` 链承载 v1/v2 存量自动升级，无手工迁移、无 legacy 层；野值 fail-fast 已定夺。fixture 库打开与重放断言由测试阶段承接 |
| AC-3（tauri-specta 接入 + 生成物入库 + Channel typed + PoC 留档） | 阶段一 PoC 门先行并回填本 design「PoC 前置门」；阶段三 builder 全量注册 22 条、`agent_start` / `watch_subscribe` 的 `Channel` 参数 typed、生成物入库 `src/types/generated/bindings.ts` |
| AC-4（重导出挂点 + 一致性守卫） | `bindings:export` / `bindings:check` 脚本 + `check` / `build` 前置串接；守卫口径与已知边界见「设计定夺」；幂等性为 PoC 通过判据之一并有阶段四核对任务 |
| AC-5（前端一次性切换；静态检查全绿） | 阶段五不分批一次切完（双轨共存期 = 0），10 个调用文件机械替换 + transport 镜像收编；裸 invoke 清零自检任务；`pnpm check` 为静态检查链（tsc + knip + fmt + clippy + bindings:check），由阶段七守线核对 |
| AC-6（dto.ts shim 化与判据驱动退役） | shim 仅 re-export（含逐名映射）；漏网旧 import 经 shim 编译不炸、knip unused 报告兜漏网名单；阶段六按「knip 零引用即删」执行，不设时间过渡 |

## 待决问题

- ~~**PoC 实际出线形态**~~——已闭环：六项验证全部通过，结论与差异名单回填「PoC 前置门」，无停线项
- ~~**三件套实际版本号**~~——已闭环：specta 2.0.0-rc.25 × specta-typescript 0.0.12 × tauri-specta 2.0.0-rc.25，`=` pin 落 Cargo.toml 并回填「配置」表
- ~~**dto.ts 逐名映射名单**~~——已闭环：全部同名直通，零逐名映射（见「PoC 生成名与 dto.ts 名称差异名单」）
- ~~**生成绑定 `Result<T, String>` 错误通道的前端接法**~~——已闭环：`ErrorHandlingMode::Throw`（Promise reject），hook error 态接线逐字不变
