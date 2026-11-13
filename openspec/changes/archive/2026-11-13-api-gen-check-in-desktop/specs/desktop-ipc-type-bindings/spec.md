# desktop-ipc-type-bindings Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 只改一条 requirement 的**清单守卫**条款：新增两条命令与三个出线类型时，`bindings/mod_test.rs` 的五处计数与两组 archi 清单在同一 change 内随动。其余七条 requirement（生成物入库、跨进程锁、typed bindings 切换、`dto.ts` 退役、knip 边界、第二产物职责切分、生成期 tsc 自检、vendored exporter、实时输出三命令）逐字不变。

## MODIFIED Requirements

### Requirement: tauri-specta 接入与命令绑定生成

desktop-app SHALL 以 tauri-specta builder 注册全部 IPC 命令（既有 `main.rs` `generate_handler!` 全量迁移；命令清单以 `all_commands!` 单一登记面为准——bin 组装与绑定生成同源，MUST NOT 在文档 / 测试中硬编码命令条数），并生成 TypeScript 类型与 typed invoke 绑定：命令名、参数 key 的 camelCase 形状、返回类型 SHALL 全部受编译期校验，前端 MUST NOT 再以裸 `invoke('command_name', ...)` 字符串调用命令（生成物内部与 `@tauri-apps/api` 底层调用除外）。流式命令 SHALL 一并覆盖：`agent_start`、`agent_session_watch` 与 `watch_subscribe` 的 `Channel<...>` 参数 SHALL 被 typed（specta 对 `tauri::ipc::Channel` 的支持面）；应用无 `emit` / `listen` 事件，specta events 面 MUST NOT 引入。Rust 出线类型（core/agent 的 `AgentEvent` / `AgentEventKind` / `AgentBlock`、core/workflow 的 `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry`、store 模型、app 层 `AgentRunMessage` / `FileWatchEvent`）SHALL 全部加 `specta::Type` derive 且类型语义 MUST NOT 因此改变。

- **清单守卫随单 change 随动**：`bindings/mod_test.rs` 的命令名清单、包装名清单、出线 DTO 清单与分族清单（如 archi 族的命令 / 出线类型两组）SHALL 在新增或删除命令 / 出线类型的**同一 change 内**随动更新；这些清单的长度断言 SHALL 只作「清单被同步过」的连带核对，MUST NOT 被当作语义判据（真实判据是清单条目名与生成物逐名命中）。尾部追加序断言 SHALL 同步保持（新条目按登记顺序插入，既有条目零位移）。本变更实测随动面 = 命令 51 → 53、出线 DTO 101 → 104、`ARCHI_COMMANDS` 6 → 8、`ARCHI_DTO_TYPES` 23 → 26，另有两处**非计数**断言随动：尾部追加序期望 vec 增加两条、`ARCHI_COMMANDS` 末条对照臂由 `archi_file_tree` 变 `archi_api_generate`。
- **分族清单 SHALL 覆盖全族**：某一命令族的分族清单 MUST NOT 漏登该族既有命令——漏登会让「新增一条」的 diff 看起来比实际小，从而把「补齐既有漏项 + 新增本次条目」两步混淆成一步（本次 archi 族即为此情形：现状 5 条命令而清单只列 4 条，缺 `archi_drift`）。

接入 SHALL 以 PoC 先行：实现第一步 MUST 验证 rc.25 对 `AgentEvent`（`tag="kind"` 内部标签 + `rename_all_fields = "camelCase"`）与 `Channel<AgentRunMessage>` 信封的实际出线形态，结论留档于 design；不符预期时 SHALL 回退备选方案（ts-rs 只生成类型 + 手写薄 invoke 层）并重新评审。

#### Scenario: builder 注册覆盖全部命令

- **WHEN** 审查 specta builder 组装点与生成的 bindings
- **THEN** `all_commands!` 登记面内全部命令（含本次新增 `archi_api_check` / `archi_api_generate`）均经 builder 注册且在 bindings 中有对应 typed 包装；命令清单与 `main.rs` 迁移前一致（新增项显式计入）

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

- **WHEN** 新增两条命令（`archi_api_check` / `archi_api_generate`）与三个出线类型（`ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome`）
- **THEN** 命令名清单与包装名清单条目数各增二（且两者持衡）、出线 DTO 清单条目数增三、两组 archi 分族清单同增；`bindings:export` 后生成物含两条命令的 typed 包装（参数键 lowerCamelCase 与 invoke payload 逐字一致）与三个 `export type`，`bindings.zod.ts` 的 `argsSchemas` / `resultSchemas` 各含对应表项；尾部追加序断言把新命令按登记序插在 `archi_file_tree` 之后、`read_file` 之前（`read_file` 仍为尾条）、既有条目零位移

#### Scenario: 分族清单漏登既有条目即补

- **WHEN** 审查任一命令族的分族清单与其 `all_commands!` 段
- **THEN** 两者条目名集合相等（无漏登、无多余）；若发现漏登（如 archi 族清单只有 4 条而该段实有 5 条），该 change SHALL 一并补齐并在同一条断言里体现「补齐 + 新增」两段变化，MUST NOT 只加新条目让清单继续缺项

## Module Contract

### Component: `packages/desktop/src-tauri/src/bindings/mod_test.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `COMMAND_NAMES` / `COMMAND_WRAPPERS` | 51 → 53 | 追加 `("archi_api_check", "archiApiCheck")` / `("archi_api_generate", "archiApiGenerate")`（登记序在 `archi_file_tree` 之后）；三处 `len()` 断言与尾部序期望 vec 同步 |
| `DTO_TYPES` | 101 → 104 | 追加 `ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome`（按字典序插入 archi 段） |
| `ARCHI_COMMANDS` | 6 → 8 | 族末条由 `archi_file_tree` 变 `archi_api_generate`；「取末条做副本删一条」对照臂同步 |
| `ARCHI_DTO_TYPES` | 23 → 26 | 三个新类型按字典序入册 |

### Component: 生成物（入库）

| 项 | 描述 |
|---|---|
| `packages/desktop/src/types/generated/bindings.ts` | 再生成：`archiApiCheck` / `archiApiGenerate` typed 包装 + 三个新 `export type` |
| `packages/desktop/src/types/generated/bindings.zod.ts` | 再生成：两条命令的 `argsSchemas` / `resultSchemas` 表项与三个具名 schema |
| `packages/desktop/src/lib/ipc-contract.test.ts` | 零改动：键集全量断言（`Object.keys(argsSchemas)` 与 `commands` 键集相等）自动覆盖新命令，核对随动即可 |
