# desktop-ipc-type-bindings Specification（delta: ipc-zod-runtime-schema）

## MODIFIED Requirements

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

## ADDED Requirements

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
| `packages/desktop/src-tauri/src/bindings/mod.rs` | 单 builder 两次导出 | `export_bindings()` 一次 `builder()`、两次 `.export`（`Typescript` → `bindings.ts`、zod exporter → `bindings.zod.ts`）；`ErrorHandlingMode::Throw` 与既有 TS 侧配置零改动 |
| `packages/desktop/src-tauri/src/bindings/zod.rs` | zod 侧 wrapper | `ZodLanguage` newtype（本地实现 `LanguageExt`）+ `ZodFormat`（`PhasesFormat` + 大整数 → `z.number()` + `serde_json::Value` → `z.unknown()`）+ 逐命令表注入（`Zod::with_raw`）；参数键名 lowerCamelCase 与 invoke payload 逐字一致；结果取 `Result` 的 Ok 臂 |
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/**` | vendored exporter | 基线 0.0.4 + 补丁 ①struct strict ②删 `export type` ③递归 getter 化（延后项与兜底项见 README）；每处带 `PATCH(n)` 标记 |
| `packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs` | 导出驱动 | 仍只调 `dev_team::bindings::export_bindings()`（入口唯一，两次导出在其内部）；失败以非零退出 |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | 第二产物 | 纯 schema：具名类型 schema + `argsSchemas` / `resultSchemas` / `channelArgs` + `degraded()` prelude；零 `export type`；`z.strictObject` 写死；零 `.default/.transform/.catch/.pipe/.refine` |
| `packages/desktop/package.json` / `knip.json` | 脚本与检查边界 | `bindings:export` / `bindings:check` / `check` 命令形态不变、覆盖面随生成目录扩大；生成目录 ignore 覆盖两份产物（不另开条目）。前端 `zod` 依赖的精确 pin / 单实例 / knip 可见性见 `desktop-ipc-runtime-guard`（不在本条重复） |

### API: `packages/desktop/src-tauri/src/bindings/mod.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `export_bindings()` | 增第二次 `.export` | 返回值与错误映射不变；第二产物路径与类型产物同目录（`src/types/generated/bindings.zod.ts`），`create_dir_all` 单点保留 |
| `builder()` | 零改动 | 仍为唯一注册面（`all_commands!` + `ErrorHandlingMode::Throw` + `semantic_types` + bigint 配置） |

### CLI: `packages/desktop/package.json`

| 脚本 | 变化 | 描述 |
|------|------|------|
| `bindings:export` | 命令不变 | 内部多产出一份 zod schema；自检失败以非零退出 |
| `bindings:check` | 命令不变 | `git diff --exit-code -- src/types/generated` 覆盖面含第二产物 |
| `check` / `build` | 命令不变 | 仍走「守卫型 / 前置重导出」既有形态 |

### Component

无。本变更新增零组件、零路由、零可见 UI 面（前端只新增非渲染模块 `src/lib/ipc.ts` / `src/lib/ipc-drift.ts`）。
