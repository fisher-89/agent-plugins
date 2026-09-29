# desktop-ipc-type-bindings Delta

## ADDED Requirements

### Requirement: tauri-specta 接入与命令绑定生成

desktop-app SHALL 以 tauri-specta builder 注册全部 IPC 命令（既有 `main.rs` `generate_handler!` 全量迁移，当前 22 条），并生成 TypeScript 类型与 typed invoke 绑定：命令名、参数 key 的 camelCase 形状、返回类型 SHALL 全部受编译期校验，前端 MUST NOT 再以裸 `invoke('command_name', ...)` 字符串调用命令（生成物内部与 `@tauri-apps/api` 底层调用除外）。流式命令 SHALL 一并覆盖：`agent_start` 与 `watch_subscribe` 的 `Channel<...>` 参数 SHALL 被 typed（specta 对 `tauri::ipc::Channel` 的支持面）；应用无 `emit` / `listen` 事件，specta events 面 MUST NOT 引入。Rust 出线类型（core/agent 的 `AgentEvent` / `AgentEventKind` / `AgentBlock`、core/workflow 的 `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry`、store 模型、app 层 `AgentRunMessage` / `FileWatchEvent`）SHALL 全部加 `specta::Type` derive 且类型语义 MUST NOT 因此改变。

接入 SHALL 以 PoC 先行：实现第一步 MUST 验证 rc.25 对 `AgentEvent`（`tag="kind"` 内部标签 + `rename_all_fields = "camelCase"`）与 `Channel<AgentRunMessage>` 信封的实际出线形态，结论留档于 design；不符预期时 SHALL 回退备选方案（ts-rs 只生成类型 + 手写薄 invoke 层）并重新评审。

#### Scenario: builder 注册覆盖全部命令

- **WHEN** 审查 specta builder 组装点与生成的 bindings
- **THEN** `generate_handler!` 既有 22 条命令全部经 builder 注册且在 bindings 中有对应 typed 包装，命令数与 `main.rs` 迁移前一致

#### Scenario: 流式 Channel 被 typed

- **WHEN** 审查 `agent_start` 与 `watch_subscribe` 的生成绑定签名
- **THEN** `onEvent: Channel<AgentRunMessage>` / `Channel<FileWatchEvent>` 参数在 TS 侧为 typed Channel，消息信封（tag `ipc` / watch 载荷）类型来自生成物

#### Scenario: 类型漂移编译期拦截

- **WHEN** Rust 侧改名字段或改命令参数形状而不更新生成物
- **THEN** 重导出后一致性守卫报工作区 diff；若生成物同步更新，前端消费处的编译错误由 tsc 拦截，运行期不再首见漂移

#### Scenario: PoC 先行留档

- **WHEN** 审查 design 与实现顺序
- **THEN** `AgentEvent` 内部标签枚举与 `Channel<AgentRunMessage>` 的出线形态验证记录在案，且先于前端切换任务执行

### Requirement: 生成物入库与一致性守卫

生成的 bindings 文件 SHALL 入库版本控制（落位 `src/lib/bindings.ts` 或 `src/types/generated/`，由 dev-design 定夺并同步 knip 配置）。Rust 侧 SHALL 提供唯一导出入口执行 `builder.export(Typescript, path)`（形态为搭 `server:test = cargo test --workspace` 顺风车的 `#[test]` 或专用 bin，design 定夺）。`check` 与 `build` SHALL 前置重导出（npm `pre` hook 或显式脚本串接），重导出后 SHALL 以 `git diff --exit-code` 类一致性守卫校验工作区干净——入库物过期 MUST 被检查捕获，MUST NOT 依赖人工记得重导。导出 SHALL 幂等（同输入同输出，重导出无 diff）。

#### Scenario: 重导出幂等

- **WHEN** 连续两次执行导出入口而不改任何 Rust 类型
- **THEN** 第二次导出后 `git diff` 为空，无格式抖动

#### Scenario: 过期生成物被守卫抓住

- **WHEN** 修改 Rust 类型后仅提交源码、不重导生成物，再执行 `pnpm check`
- **THEN** 前置重导出产生工作区 diff，守卫以非零退出报「生成物过期」，检查不通过

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

`tauri-specta` / `specta-typescript` / `specta` 三件套 SHALL 以 `=` 精确 pin 并 lockstep 升级（RC 间无 semver 兼容），workspace.dependencies 收敛。依赖落位 SHALL 为：`tauri-specta` 与 `specta-typescript` 仅落 desktop-app（Tauri 系依赖不出壳层）；`specta` derive SHALL 准入 core/infra（specta 为普通类型导出 derive crate，非 Tauri 系，与 serde 先例同列，「core/infra 禁 Tauri」约束不破，crate 依赖方向规则全部不变）。生成物目录 SHALL 配置 knip ignore（生成物非手写代码，未引用的命令包装 MUST NOT 报 dead code）。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查各 crate 的 `Cargo.toml`
- **THEN** tauri-specta / specta-typescript 仅出现在 desktop-app 依赖；core/agent、core/workflow、infra/store 仅有 specta（非 Tauri 系）且依赖方向既有规则不变

#### Scenario: 三件套精确 pin

- **WHEN** 审查 workspace.dependencies 中三件套版本
- **THEN** `specta` / `specta-typescript` / `tauri-specta` 均为 `=` 精确 pin 且版本 lockstep

#### Scenario: knip 不报生成物

- **WHEN** 执行 `client:check`（含 knip）
- **THEN** 生成目录在 knip.json ignore 内，生成物未引用导出不报 dead code，检查通过

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/`（specta builder 组装 + 导出入口） | 命令注册与类型导出 | builder 注册全部 22 条命令（迁自 `generate_handler!`）；`export(Typescript, path)` 唯一导出入口（`#[test]` 或 bin，幂等）；events 面不引入 |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 前端唯一 IPC 类型与调用面 | 全部命令 typed 包装 + 全部出线 DTO 类型；入库 + check/build 前置重导出 + `git diff --exit-code` 守卫；knip ignore |
| `packages/desktop/src-tauri/crates/core/agent`（derive 增量） | 出线事件类型导出面 | `AgentEvent` / `AgentEventKind` / `AgentBlock` + `specta::Type`；枚举语义与 serde 线格式不变；禁 Tauri 不破 |
| `packages/desktop/src-tauri/crates/core/workflow`（derive 增量） | 出线查询类型导出面 | `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry` + `specta::Type`；语义不动 |
| `packages/desktop/src-tauri/crates/infra/store`（derive 增量） | 出线模型导出面 | store 模型 + `specta::Type`；native_model 版本治理不变 |
| `packages/desktop/src-tauri/src/commands/`（`AgentRunMessage` / `FileWatchEvent`） | app 层 IPC 信封导出面 | tag `ipc` 信封与 watch 载荷 typed；`STATUS_*` 裸常量随枚举化退役 |
| `packages/desktop/src/lib/agent-transport.ts` | transport 适配层 | 手写镜像删除、接生成绑定；`readChainParams` 等运行时校验保留 |
| `packages/desktop/src/types/dto.ts`（shim → 删） | 过渡容错网 | 纯 re-export（含逐名映射）；knip 零引用即删，不按时间过渡 |
| `packages/desktop/package.json` / `knip.json` | 脚本与检查边界 | check/build 前置重导出 + diff 守卫；生成目录 ignore |
