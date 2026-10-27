# desktop-ipc-type-bindings Specification

## Purpose

定义 desktop-app IPC 面的类型绑定契约：tauri-specta 命令注册与 TypeScript 生成物、生成物入库与一致性守卫、前端 typed 调用面切换、dto.ts shim 过渡与判据驱动退役、依赖落位与 knip 边界。Rust 类型为单一事实源,前端调用面 MUST NOT 维护手写镜像。

## Requirements

### Requirement: tauri-specta 接入与命令绑定生成

desktop-app SHALL 以 tauri-specta builder 注册全部 IPC 命令（既有 `main.rs` `generate_handler!` 全量迁移；命令清单以 `all_commands!` 单一登记面为准——bin 组装与绑定生成同源，MUST NOT 在文档 / 测试中硬编码命令条数），并生成 TypeScript 类型与 typed invoke 绑定：命令名、参数 key 的 camelCase 形状、返回类型 SHALL 全部受编译期校验，前端 MUST NOT 再以裸 `invoke('command_name', ...)` 字符串调用命令（生成物内部与 `@tauri-apps/api` 底层调用除外）。流式命令 SHALL 一并覆盖：`agent_start`、`agent_session_watch` 与 `watch_subscribe` 的 `Channel<...>` 参数 SHALL 被 typed（specta 对 `tauri::ipc::Channel` 的支持面）；应用无 `emit` / `listen` 事件，specta events 面 MUST NOT 引入。Rust 出线类型（core/agent 的 `AgentEvent` / `AgentEventKind` / `AgentBlock`、core/workflow 的 `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry`、store 模型、app 层 `AgentRunMessage` / `FileWatchEvent`）SHALL 全部加 `specta::Type` derive 且类型语义 MUST NOT 因此改变。

接入 SHALL 以 PoC 先行：实现第一步 MUST 验证 rc.25 对 `AgentEvent`（`tag="kind"` 内部标签 + `rename_all_fields = "camelCase"`）与 `Channel<AgentRunMessage>` 信封的实际出线形态，结论留档于 design；不符预期时 SHALL 回退备选方案（ts-rs 只生成类型 + 手写薄 invoke 层）并重新评审。

#### Scenario: builder 注册覆盖全部命令

- **WHEN** 审查 specta builder 组装点与生成的 bindings
- **THEN** `all_commands!` 登记面内全部命令（含本次新增 `agent_session_watch`）均经 builder 注册且在 bindings 中有对应 typed 包装；命令清单与 `main.rs` 迁移前一致（新增项显式计入）

#### Scenario: 流式 Channel 被 typed

- **WHEN** 审查 `agent_start`、`agent_session_watch` 与 `watch_subscribe` 的生成绑定签名
- **THEN** `onEvent: Channel<AgentRunMessage>` / `Channel<FileWatchEvent>` 参数在 TS 侧为 typed Channel，消息信封（tag `ipc` / watch 载荷）类型来自生成物；订阅命令的 `sinceSeq` 为 typed 数值参数

#### Scenario: 类型漂移编译期拦截

- **WHEN** Rust 侧改名字段或改命令参数形状而不更新生成物
- **THEN** 重导出后一致性守卫报工作区 diff；若生成物同步更新，前端消费处的编译错误由 tsc 拦截，运行期不再首见漂移

#### Scenario: PoC 先行留档

- **WHEN** 审查 design 与实现顺序
- **THEN** `AgentEvent` 内部标签枚举与 `Channel<AgentRunMessage>` 的出线形态验证记录在案，且先于前端切换任务执行

### Requirement: 生成物入库与一致性守卫

生成的 bindings 产物（`bindings.ts` 与 `bindings.zod.ts` 两份）SHALL 全部入库版本控制，同落位 `src/types/generated/`（既有实现已定，MUST NOT 迁址；第二产物 MUST 与类型产物同目录、同一次导出产出）。Rust 侧 SHALL 提供唯一导出入口执行两次 `builder.export`（同一 builder、同一 `all_commands!` 登记面：`Typescript` 一次 + zod exporter 一次；形态为搭 `server:test = cargo test --workspace` 顺风车的 `#[test]` 或专用 bin，design 定夺）。`check` 与 `build` SHALL 前置重导出（npm `pre` hook 或显式脚本串接），重导出后 SHALL 以 `git diff --exit-code` 类一致性守卫校验工作区干净——入库物过期 MUST 被检查捕获，MUST NOT 依赖人工记得重导。`check` SHALL 走守卫型脚本（`bindings:check` = 导出 + `git diff --exit-code -- src/types/generated`，覆盖面为整个生成目录、含第二产物，命令形态不变），MUST NOT 走「就地导出即最新」的 `bindings:export`（后者使 tsc 永不报漂移，守卫形同虚设）；`build` SHALL 维持导出（产物构建需最新生成物）。导出 SHALL 幂等（同输入同输出，重导出无 diff；两份产物各自幂等，且两次导出之间 MUST NOT 相互影响）。

#### Scenario: 重导出幂等

- **WHEN** 连续两次执行导出入口而不改任何 Rust 类型
- **THEN** 第二次导出后 `git diff` 为空，无格式抖动

#### Scenario: 过期生成物被守卫抓住

- **WHEN** 修改 Rust 类型后仅提交源码、不重导生成物，再执行 `pnpm check`
- **THEN** `bindings:check` 先重导出、再由 `git diff --exit-code -- src/types/generated` 判非空并以非零退出报「生成物过期」，检查不通过

#### Scenario: 新命令绑定同步

- **WHEN** 新增会话订阅命令 `agent_session_watch` 后执行 `pnpm check`
- **THEN** 生成物含该命令的 typed 包装（`onEvent: Channel<AgentRunMessage>` + `sinceSeq`）且入库；未重导时守卫报 diff，MUST NOT 静默通过

#### Scenario: 第二产物同受守卫

- **WHEN** 修改 Rust 侧出线 DTO 后重导出，但仅提交 `bindings.ts`（`bindings.zod.ts` 手工漏更或只手改其一份）
- **THEN** `git diff --exit-code -- src/types/generated` 判非空并以非零退出——守卫覆盖整个生成目录，MUST NOT 只校验类型产物

#### Scenario: 第二产物幂等

- **WHEN** 连续两次执行导出入口且不改任何 Rust 类型
- **THEN** 两份产物均零 diff；导出顺序（`bindings.ts` 先、`bindings.zod.ts` 后）MUST NOT 改变任一产物的字节

### Requirement: 前端一次性切换 typed bindings

前端 SHALL 在本变更内一次性完成切换：`src/` 全部 invoke 调用点（约 27 个文件）从裸字符串 invoke 改为 typed bindings，MUST NOT 分批（双轨共存期 MUST 为 0）。`agent-transport.ts` 手写的 `AgentRunMessage` / `AgentStartChainParams` 镜像 SHALL 收编进生成物并删除手写副本；`readChainParams` 等运行时校验 SHALL 保留（ai-sdk body 穿透所需，与静态类型互补，非重复防线）。切换的真实风险（生成物与手写镜像的字段级差异，如 null vs optional）由 tsc 全量类型检查拦截，替换本身为机械操作。

#### Scenario: 裸 invoke 清零

- **WHEN** 以命令名字符串模式扫描 `packages/desktop/src`（排除生成物文件与测试内对生成物的断言）
- **THEN** 无业务代码直接调用 `invoke('command_name', ...)`，全部经生成绑定；`agent-transport.ts` 内无手写 IPC 镜像类型

#### Scenario: 运行时校验保留

- **WHEN** 审查切换后的 `agent-transport.ts`
- **THEN** `readChainParams` / `readPermissionMode` 等运行时校验仍在，body 穿透路径的清单外值拒绝语义不变

#### Scenario: 一次性切换可整体回滚

- **WHEN** 切换后需要回退
- **THEN** 单 change 整体 revert 即回到裸 invoke + 手写镜像形态，无半 typed 半裸字符串的中间态残留

### Requirement: dto.ts shim 过渡与判据驱动退役

`dto.ts` SHALL 收敛为纯 re-export shim（`export type * from` 生成物；生成名与现名有出入时经逐名 re-export 做名称映射），使一次性切换的漏网旧 import 不炸编译/运行时，并由 knip unused 报告兜出漏网名单。shim MUST NOT 承载任何手写类型定义（dto.ts 全量为 Rust 镜像、无前端独有类型，终局必删）。退役判据 SHALL 为 knip 报告 shim 零引用即删除，MUST NOT 按时间过渡。

#### Scenario: shim 纯 re-export

- **WHEN** 审查 shim 化后的 `dto.ts`
- **THEN** 文件仅含 re-export 语句（含必要的逐名映射），无任何手写 interface / type 定义

#### Scenario: 漏网 import 不炸

- **WHEN** 切换后存在未被机械替换覆盖的旧 `from '../types/dto'` import
- **THEN** 编译经 shim re-export 通过，knip unused 报告列出该漏网引用供收口

#### Scenario: 零引用退役

- **WHEN** knip 报告 dto.ts（shim）零引用
- **THEN** 删除该文件且全量检查仍绿；退役不以日历时间为准

### Requirement: 依赖落位与 knip 边界

`tauri-specta` / `specta-typescript` / `specta` 三件套 SHALL 以 `=` 精确 pin 并 lockstep 升级（RC 间无 semver 兼容），workspace.dependencies 收敛；`specta-serde` SHALL 与 `specta-typescript` 同版本同 `=` pin（其 `PhasesFormat` 是 zod 侧复用的 serde 相位语义层，版本错配会让两份产物的相位规则分叉）。依赖落位 SHALL 为：`tauri-specta` 与 `specta-typescript` 仅落 desktop-app（Tauri 系依赖不出壳层）；`specta` derive SHALL 准入 core/infra（specta 为普通类型导出 derive crate，非 Tauri 系，与 serde 先例同列，「core/infra 禁 Tauri」约束不破，crate 依赖方向规则全部不变）；zod exporter SHALL 以 vendored path 依赖（`crates/tools/specta-zod-patched`）仅落 desktop-app，MUST NOT 出现在 core/infra。生成物目录 SHALL 配置 knip ignore（生成物非手写代码，未引用的命令包装 MUST NOT 报 dead code；第二产物同受该目录 ignore 覆盖，MUST NOT 为它另开 ignore 条目）。本条只约束 Rust 侧 specta 系与 vendored exporter 的落位；前端**运行期** `zod` 依赖的声明形态、精确 pin、单实例与 knip 可见性归 `desktop-ipc-runtime-guard` 的「前端运行期 zod 依赖」requirement（前端消费面），MUST NOT 在本条内二次约束（两处约束即两处漂移面）。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查各 crate 的 `Cargo.toml`
- **THEN** tauri-specta / specta-typescript 仅出现在 desktop-app 依赖；core/agent、core/workflow、infra/store 仅有 specta（非 Tauri 系）且依赖方向既有规则不变

#### Scenario: 三件套精确 pin

- **WHEN** 审查 workspace.dependencies 中三件套版本
- **THEN** `specta` / `specta-typescript` / `tauri-specta` 均为 `=` 精确 pin 且版本 lockstep

#### Scenario: 四件套与 vendored exporter 锁定同一窗口

- **WHEN** 审查 `specta` / `tauri-specta` / `specta-typescript` / `specta-serde` 的 pin 与 vendored `specta-zod` 的基线版本
- **THEN** 四者 `=` 精确 pin 且相互兼容（同一 specta RC 窗口），vendored exporter 的 `Cargo.toml` 内 pin 与该窗口一致；`cargo tree` 中不存在两个 specta RC 版本并存

#### Scenario: knip 不报生成物

- **WHEN** 执行 `client:check`（含 knip）
- **THEN** 生成目录在 knip.json ignore 内，生成物未引用导出不报 dead code，检查通过

### Requirement: 第二产物职责切分（纯 schema，零 `export type`）

生成目录 SHALL 承载两份职责互斥的产物：`bindings.ts` 为**唯一类型源**（typed invoke 包装 + 全部出线 DTO 类型 + JSDoc 说明，形态与既有导出零变更），`bindings.zod.ts` 为**纯 schema**（具名类型 schema + 逐命令 args / result 表 + channel 参数元数据）。第二产物 MUST NOT 输出任何 `export type` / 相位伴生别名，MUST NOT 承担命令包装渲染；前端 MUST NOT 从第二产物取类型（禁 `z.infer` 作业务类型），类型一律出自 `bindings.ts`。

理由（结构性，不是偏好）：zod exporter 只吃 `&Types`、看不到 `BuilderConfiguration`，typed invoke 包装 / `Channel<…>` 参数 / 错误面变换全部活在 `cfg.commands` 里，只有 tauri-specta 能渲染；反之 `z.infer` 在递归类型上会塌成 `any` 并静默传播。故两份产物不可互换、不可一份替代另一份。

#### Scenario: 第二产物零类型别名

- **WHEN** 对 `src/types/generated/bindings.zod.ts` grep `export type` 与 `z.infer`
- **THEN** 零命中（无类型别名、无相位伴生别名、无 `z.infer` 转发）

#### Scenario: 类型主权单向

- **WHEN** 审查前端业务代码的 IPC 类型来源
- **THEN** 全部类型 import 自 `src/types/generated/bindings.ts`（或其 shim），无一处以 `z.infer<…>` / 第二产物作为业务类型声明

#### Scenario: 命令包装只在类型产物

- **WHEN** 审查两份产物的内容面
- **THEN** typed invoke 包装（含 `Channel` 参数映射与错误面变换）只出现在 `bindings.ts`；第二产物只有 schema 与命令表

### Requirement: 生成期 tsc 自检与降级清单

导出链 SHALL 在产物落地后对「只 import `bindings.zod.ts` 的桩文件」执行一次 `tsc --noEmit` 自检（单文件、零运行期依赖），一条自检同时看住三件事：struct 面 strict 化、递归引用 getter 化（无 `z.lazy` 自引用初值）、第二产物无类型别名。自检失败 MUST 以非零退出阻断 `bindings:export`（产物不可用 MUST NOT 被当作「已导出」入库）。

导出期 SHALL 收集降级清单（哪些类型 / 参数位落到了 `degraded("原因")`）并在 stderr 输出；降级 MUST NOT 静默发生。

#### Scenario: 自检挡住不合法产物

- **WHEN** 产物出现 `z.lazy` 自引用（未走 getter）或 struct 面未 strict 化
- **THEN** 自检桩文件 `tsc --noEmit` 报错，`bindings:export` 以非零退出，产物不入库

#### Scenario: 降级清单可见

- **WHEN** 导出过程中有类型被降级为 `degraded("原因")`
- **THEN** stderr 列出该清单（类型名 / 命令参数位 + 原因），退出码仍为 0（降级是可接受收口，不是失败）

#### Scenario: 自检不引入运行期依赖

- **WHEN** 审查自检桩文件
- **THEN** 它只 import 生成产物、只做类型层使用（无 DOM、无 Tauri、无网络），可独立 `tsc` 执行

### Requirement: vendored exporter 落位与跟版

zod exporter SHALL 以 vendored crate 落位 `packages/desktop/src-tauri/crates/tools/specta-zod-patched`（workspace 成员，与 `export-bindings` 同层；MUST NOT 落 core/infra，MUST NOT 引入 Tauri 依赖到 core/infra）。每处本地改动 MUST 带可 grep 的标记（如 `PATCH(n)`）并在 crate README 记录：基线版本、每个补丁的位置与理由、跟版核对清单。上游合入对应能力后，MUST 删除 vendor 改回 registry 依赖（判据是上游版本覆盖补丁语义，MUST NOT 按时间过渡）。

#### Scenario: 补丁可机器清点

- **WHEN** 对 vendored crate 源码 grep 补丁标记
- **THEN** 命中数与 README 记录的补丁条数一致，每条标记行紧邻改动说明

#### Scenario: 落位与依赖方向

- **WHEN** 检查 workspace members 与各 crate 依赖
- **THEN** vendored crate 位于 `crates/tools/` 且仅被 desktop-app 依赖；core / infra 任一 crate 的依赖中不出现它

#### Scenario: 上游合入即删 vendor

- **WHEN** 上游发布覆盖现有补丁语义的版本
- **THEN** 改为 registry 依赖并删除 `crates/tools/specta-zod-patched`，产物零 diff（判据驱动，不以日历时间为准）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/`（specta builder 组装 + 导出入口） | 命令注册与类型导出 | builder 注册 `all_commands!` 单一登记面内全部命令（迁自 `generate_handler!`，命令条数不硬编码）；`export(Typescript, path)` 唯一导出入口（`#[test]` 或 bin，幂等）；events 面不引入 |
| `packages/desktop/src-tauri/src/bindings/mod.rs` | 单 builder 两次导出 | `export_bindings()` 一次 `builder()`、两次 `.export`（`Typescript` → `bindings.ts`、zod exporter → `bindings.zod.ts`）；`ErrorHandlingMode::Throw` 与既有 TS 侧配置零改动 |
| `packages/desktop/src-tauri/src/bindings/zod.rs` | zod 侧 wrapper | `ZodLanguage` newtype（本地实现 `LanguageExt`）+ `ZodFormat`（`PhasesFormat` + 大整数 → `z.number()` + `serde_json::Value` → `z.unknown()`）+ 逐命令表注入（`Zod::with_raw`）；参数键名 lowerCamelCase 与 invoke payload 逐字一致；结果取 `Result` 的 Ok 臂 |
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/**` | vendored exporter | 基线 0.0.4 + 补丁 ①struct strict ②删 `export type` ③递归 getter 化（延后项与兜底项见 README）；每处带 `PATCH(n)` 标记 |
| `packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs` | 导出驱动 | 仍只调 `dev_team::bindings::export_bindings()`（入口唯一，两次导出在其内部）；失败以非零退出 |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 前端唯一 IPC 类型与调用面 | 全部命令 typed 包装 + 全部出线 DTO 类型；入库 + check/build 前置重导出 + `git diff --exit-code` 守卫；knip ignore；类型与 typed invoke 包装来源不变，`ipc` 与 Equal 断言只从此处取类型，MUST NOT 转发 zod 类型 |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | 第二产物 | 纯 schema：具名类型 schema + `argsSchemas` / `resultSchemas` / `channelArgs` + `degraded()` prelude；零 `export type`；`z.strictObject` 写死；零 `.default/.transform/.catch/.pipe/.refine` |
| `packages/desktop/src-tauri/crates/core/agent`（derive 增量） | 出线事件类型导出面 | `AgentEvent` / `AgentEventKind` / `AgentBlock` + `specta::Type`；枚举语义与 serde 线格式不变；禁 Tauri 不破 |
| `packages/desktop/src-tauri/crates/core/workflow`（derive 增量） | 出线查询类型导出面 | `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry` + `specta::Type`；语义不动 |
| `packages/desktop/src-tauri/crates/infra/store`（derive 增量） | 出线模型导出面 | store 模型 + `specta::Type`；native_model 版本治理不变 |
| `packages/desktop/src-tauri/src/commands/`（`AgentRunMessage` / `FileWatchEvent`） | app 层 IPC 信封导出面 | tag `ipc` 信封与 watch 载荷 typed；`STATUS_*` 裸常量随枚举化退役 |
| `packages/desktop/src/lib/agent-transport.ts` | transport 适配层 | 手写镜像删除、接生成绑定；`readChainParams` 等运行时校验保留 |
| `packages/desktop/src/types/dto.ts`（shim → 删） | 过渡容错网 | 纯 re-export（含逐名映射）；knip 零引用即删，不按时间过渡 |
| `packages/desktop/package.json` / `knip.json` | 脚本与检查边界 | check/build 前置重导出 + diff 守卫；生成目录 ignore 覆盖两份产物（不另开条目）；`bindings:export` / `bindings:check` / `check` 命令形态不变、覆盖面随生成目录扩大；前端 `zod` 依赖的精确 pin / 单实例 / knip 可见性见 `desktop-ipc-runtime-guard`（不在本条重复） |

### API: `packages/desktop/src-tauri/src/commands/mod.rs`（单一登记面）

| 项 | 变化 | 描述 |
|------|------|------|
| `all_commands!` | 新增 `commands::exec::agent_session_watch` | builder 注册与 `generate_handler!` 同源；绑定生成随之一致 |

### API: `packages/desktop/src-tauri/src/bindings/mod.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `export_bindings()` | 增第二次 `.export` | 返回值与错误映射不变；第二产物路径与类型产物同目录（`src/types/generated/bindings.zod.ts`），`create_dir_all` 单点保留 |
| `builder()` | 零改动 | 仍为唯一注册面（`all_commands!` + `ErrorHandlingMode::Throw` + `semantic_types` + bigint 配置） |

### CLI: `packages/desktop/package.json`（脚本）

| 脚本 | 命令 | 描述 |
|------|------|------|
| `bindings:check` | `pnpm run bindings:export && git diff --exit-code -- src/types/generated` | 导出 + 工作区干净守卫（`check` 前置调用）；diff 覆盖面含第二产物 |
| `check` | `pnpm run server:check && pnpm run bindings:check && pnpm run client:check` | 守卫型检查（由 `bindings:export` 改为 `bindings:check`）；命令形态不变 |
| `bindings:export` / `build` | 不变 | 导出免守卫（构建产物需最新生成物）；`bindings:export` 内部多产出一份 zod schema，自检失败以非零退出 |

### Component

无。本变更新增零组件、零路由、零可见 UI 面（前端只新增非渲染模块 `src/lib/ipc.ts` / `src/lib/ipc-drift.ts`）。
