# desktop-ipc-type-bindings Specification（delta: archi-file-tree）

## MODIFIED Requirements

### Requirement: tauri-specta 接入与命令绑定生成

desktop-app SHALL 以 tauri-specta builder 注册全部 IPC 命令（既有 `main.rs` `generate_handler!` 全量迁移；命令清单以 `all_commands!` 单一登记面为准——bin 组装与绑定生成同源，MUST NOT 在文档 / 测试中硬编码命令条数），并生成 TypeScript 类型与 typed invoke 绑定：命令名、参数 key 的 camelCase 形状、返回类型 SHALL 全部受编译期校验，前端 MUST NOT 再以裸 `invoke('command_name', ...)` 字符串调用命令（生成物内部与 `@tauri-apps/api` 底层调用除外）。流式命令 SHALL 一并覆盖：`agent_start`、`agent_session_watch` 与 `watch_subscribe` 的 `Channel<...>` 参数 SHALL 被 typed（specta 对 `tauri::ipc::Channel` 的支持面）；应用无 `emit` / `listen` 事件，specta events 面 MUST NOT 引入。Rust 出线类型（core/agent 的 `AgentEvent` / `AgentEventKind` / `AgentBlock`、core/workflow 的 `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry`、store 模型、app 层 `AgentRunMessage` / `FileWatchEvent`）SHALL 全部加 `specta::Type` derive 且类型语义 MUST NOT 因此改变。

- **清单守卫随单 change 随动**：`bindings/mod_test.rs` 的命令名清单、包装名清单、出线 DTO 清单与分族清单（如 archi 族的命令 / 出线类型两组）SHALL 在新增或删除命令 / 出线类型的**同一 change 内**随动更新；这些清单的长度断言 SHALL 只作「清单被同步过」的连带核对，MUST NOT 被当作语义判据（真实判据是清单条目名与生成物逐名命中）。尾部追加序断言 SHALL 同步保持（新条目按登记顺序插入，既有条目零位移）。
- **分族清单 SHALL 覆盖全族**：某一命令族的分族清单 MUST NOT 漏登该族既有命令——漏登会让「新增一条」的 diff 看起来比实际小，从而把「补齐既有漏项 + 新增本次条目」两步混淆成一步（本次 archi 族即为此情形：现状 5 条命令而清单只列 4 条，缺 `archi_drift`）。

接入 SHALL 以 PoC 先行：实现第一步 MUST 验证 rc.25 对 `AgentEvent`（`tag="kind"` 内部标签 + `rename_all_fields = "camelCase"`）与 `Channel<AgentRunMessage>` 信封的实际出线形态，结论留档于 design；不符预期时 SHALL 回退备选方案（ts-rs 只生成类型 + 手写薄 invoke 层）并重新评审。

#### Scenario: builder 注册覆盖全部命令

- **WHEN** 审查 specta builder 组装点与生成的 bindings
- **THEN** `all_commands!` 登记面内全部命令（含本次新增 `archi_file_tree`）均经 builder 注册且在 bindings 中有对应 typed 包装；命令清单与 `main.rs` 迁移前一致（新增项显式计入）

#### Scenario: 流式 Channel 被 typed

- **WHEN** 审查 `agent_start`、`agent_session_watch` 与 `watch_subscribe` 的生成绑定签名
- **THEN** `onEvent: Channel<AgentRunMessage>` / `Channel<FileWatchEvent>` 参数在 TS 侧为 typed Channel，消息信封（tag `ipc` / watch 载荷）类型来自生成物；订阅命令的 `sinceSeq` 为 typed 数值参数

#### Scenario: 类型漂移编译期拦截

- **WHEN** Rust 侧改名字段或改命令参数形状而不更新生成物
- **THEN** 重导出把生成物刷新为最新；若生成物自检失败（`bindings:zod-check` 非零）检查即失败，否则前端消费处的编译错误由 tsc 拦截，运行期不再首见漂移

#### Scenario: PoC 先行留档

- **WHEN** 审查 design 与实现顺序
- **THEN** `AgentEvent` 内部标签枚举与 `Channel<AgentRunMessage>` 的出线形态验证记录在案，且先于前端切换任务执行

#### Scenario: 新增命令与出线类型时清单同 change 随动

- **WHEN** 新增一条命令（如 `archi_file_tree`）与四个出线类型（如 `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark`）
- **THEN** 命令名清单与包装名清单条目数同增（且两者持衡）、出线 DTO 清单条目数同增四、分族清单同增；`bindings:export` 后生成物含该命令的 typed 包装（参数键 lowerCamelCase 与 invoke payload 逐字一致）与四个 `export type`，`bindings.zod.ts` 的 `argsSchemas` / `resultSchemas` 各含对应表项；尾部追加序断言把新命令插在族内既有末条之后、既有条目零位移

#### Scenario: 分族清单漏登既有条目即补

- **WHEN** 审查任一命令族的分族清单与其 `all_commands!` 段
- **THEN** 两者条目名集合相等（无漏登、无多余）；若发现漏登（如 archi 族清单只有 4 条而该段实有 5 条），该 change SHALL 一并补齐并在同一条断言里体现「补齐 + 新增」两段变化，MUST NOT 只加新条目让清单继续缺项

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/mod.rs` | 命令登记面 | `all_commands!` 的 archi 段追加 `commands::archi::archi_file_tree`（5 → 6 条；builder 注册与 `generate_handler!` 同源） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 清单守卫 | `COMMAND_NAMES` / `COMMAND_WRAPPERS` 48 → 49（`archi_file_tree` / `archiFileTree` 插在 `archi_drift` 之后、`read_file` 之前）；`DTO_TYPES` 97 → 101（清单尾部两个段落本身是字典序：`ArchiFileEntry` / `ArchiFileTree` 落在 archi 段的 `ArchiExcludedFile` 与 `ArchiGateChange` 之间，`FileEntryKind` / `FileTreeMark` 落在末段的 `EdgeMark` 与 `GateTier` 之间；清单首锚仍 `ActivePhase`、尾锚仍 `WarningKind`）；`ARCHI_COMMANDS` 4 → 6（补齐 `archi_drift` + 新增 `archi_file_tree`）；`ARCHI_DTO_TYPES` 19 → 23（同样按字典序：`ArchiFileEntry` / `ArchiFileTree` 插在 `ArchiDependsOn` 与 `ArchiDiagnostic` 之间，`FileEntryKind` / `FileTreeMark` 插在 `EdgeMark` 与 `GateTier` 之间）；3 处 `COMMAND_NAMES.len()` 断言同改；既有尾部追加序断言的期望 vec 同步为 `… archi_set_gate, archi_drift, archi_file_tree, read_file`；取 `ARCHI_COMMANDS` 末条做「副本删一条」对照臂的用例（`清单遗漏即产物覆盖断言必红`）随之改判（该族末条由 `archi_set_gate` 变 `archi_file_tree`）；`archi四命令在册且既有条目零位移` 一条用例名改为符实（该文件内只有这一处「四命令」用例名；`ARCHI_COMMANDS` 注释「架构域四命令」同步改为「架构域命令族」） |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 前端唯一 IPC 类型与调用面 | `archiFileTree` typed 包装 + `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark` 类型；随前置重导出更新（过期由提交前人工核对） |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | 第二产物 | 该命令的 `argsSchemas` / `resultSchemas` 表项 + 四个具名 schema（纯接受面、零修饰、零 `export type`） |
| `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts`（新增，见 `desktop-archi-file-tree`） | 前端消费面 | 经 `commands.archiFileTree(root)` 调用（生成物 typed 包装，零裸 invoke 字符串） |
