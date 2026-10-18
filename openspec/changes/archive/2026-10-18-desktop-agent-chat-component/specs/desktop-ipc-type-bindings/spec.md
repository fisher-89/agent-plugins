# desktop-ipc-type-bindings Specification (Delta)

## MODIFIED Requirements

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

生成的 bindings 文件 SHALL 入库版本控制（落位 `src/lib/bindings.ts` 或 `src/types/generated/`，由 dev-design 定夺并同步 knip 配置）。Rust 侧 SHALL 提供唯一导出入口执行 `builder.export(Typescript, path)`（形态为搭 `server:test = cargo test --workspace` 顺风车的 `#[test]` 或专用 bin，design 定夺）。`check` 与 `build` SHALL 前置重导出（npm `pre` hook 或显式脚本串接），重导出后 SHALL 以 `git diff --exit-code` 类一致性守卫校验工作区干净——入库物过期 MUST 被检查捕获，MUST NOT 依赖人工记得重导。`check` SHALL 走守卫型脚本（`bindings:check` = 导出 + `git diff --exit-code -- src/types/generated`），MUST NOT 走「就地导出即最新」的 `bindings:export`（后者使 tsc 永不报漂移，守卫形同虚设）；`build` SHALL 维持导出（产物构建需最新生成物）。导出 SHALL 幂等（同输入同输出，重导出无 diff）。

#### Scenario: 重导出幂等

- **WHEN** 连续两次执行导出入口而不改任何 Rust 类型
- **THEN** 第二次导出后 `git diff` 为空，无格式抖动

#### Scenario: 过期生成物被守卫抓住

- **WHEN** 修改 Rust 类型后仅提交源码、不重导生成物，再执行 `pnpm check`
- **THEN** `bindings:check` 先重导出、再由 `git diff --exit-code -- src/types/generated` 判非空并以非零退出报「生成物过期」，检查不通过

#### Scenario: 新命令绑定同步

- **WHEN** 新增会话订阅命令 `agent_session_watch` 后执行 `pnpm check`
- **THEN** 生成物含该命令的 typed 包装（`onEvent: Channel<AgentRunMessage>` + `sinceSeq`）且入库；未重导时守卫报 diff，MUST NOT 静默通过

## Module Contract

### API: `packages/desktop/src-tauri/src/commands/mod.rs`（单一登记面）

| 项 | 变化 | 描述 |
|------|------|------|
| `all_commands!` | 新增 `commands::exec::agent_session_watch` | builder 注册与 `generate_handler!` 同源；绑定生成随之一致 |

### CLI: `packages/desktop/package.json`（脚本）

| 脚本 | 命令 | 描述 |
|------|------|------|
| `bindings:check` | `pnpm run bindings:export && git diff --exit-code -- src/types/generated` | 导出 + 工作区干净守卫（`check` 前置调用） |
| `check` | `pnpm run server:check && pnpm run bindings:check && pnpm run client:check` | 守卫型检查（由 `bindings:export` 改为 `bindings:check`） |
| `bindings:export` / `build` | 不变 | 导出免守卫（构建产物需最新生成物） |
