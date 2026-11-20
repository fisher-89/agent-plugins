# desktop-ipc-type-bindings Specification (Delta)

## ADDED Requirements

### Requirement: archi 族第九条命令与 ArchiRelationDetail 的清单随动

`bindings/mod_test.rs` 的清单守卫 SHALL 在本 change 内随 `archi_relation` 命令与 `ArchiRelationDetail` 出线类型同批随动（口径与既有「清单守卫随单 change 随动」一致：长度断言只作「清单被同步过」的连带核对，MUST NOT 被当作语义判据；真实判据是清单条目名与生成物逐名命中）：

| 清单 | 变化 | 插入位 / 依据 |
|---|---|---|
| `COMMAND_NAMES` | **56 → 57** | `archi_relation` 按登记序追加在 `archi_api_generate` 之后、`read_file` 之前（`read_file` 仍为尾条） |
| `COMMAND_WRAPPERS` | **56 → 57** | `archiRelation`，与 `COMMAND_NAMES` 一一对应（两清单持衡） |
| `DTO_TYPES` | **104 → 105** | `ArchiRelationDetail` 按字典序插在 `ArchiRelation` 与 `ArchiScanReport` 之间（首锚仍 `ActivePhase`；尾锚仍 **`NodeMark`**——`WarningKind` 已随 `docs` 三源退役，尾锚早已回落到封闭词汇末项） |
| `ARCHI_COMMANDS` | **8 → 9** | 末条由 `archi_api_generate` 变 `archi_relation`；既有八条零位移 |
| `ARCHI_DTO_TYPES` | **21 → 22** | `ArchiRelationDetail` 插在 `ArchiRelation` 之后（该清单按字典序） |

- **尾部追加序断言 SHALL 同步**：`all_commands!` 的 archi 段期望 vec 追加一项 `archi_relation`（`… archi_api_check, archi_api_generate, archi_relation, read_file`），且既有条目零位移——命令登记面不得插入到既有 element 之间（否则 builder 与生成物的顺序对照失效）。
- **`ArchiRelation` 的字段级进场不增清单成员**：`ArchiRelation.apis` 是既有类型的字段扩充（`ArchiRelation` 本就在两份清单内），只有生成物内容变化；MUST NOT 因它新增清单条目、MUST NOT 改动首尾锚断言。
- **生成物零手改**：`bindings.ts` 的 `commands.archiRelation` typed 包装、`ArchiRelationDetail` 类型与 `bindings.zod.ts` 的 `argsSchemas` / `resultSchemas` 表项 SHALL 全部来自 `bindings:export`（提交前人工核对 `git diff`；`check` 链只前置重导出、不做生成物比对——零 diff 守卫已退役）。
- **参数键形 SHALL 与命令签名逐字一致**：`archi_relation(root, from, to)` 的生成包装入参键为 `root` / `from` / `to`（三者均为必填字符串）；`from` / `to` 是 TOML / JS 的保留观感词但**不是保留字**，MUST NOT 为此改名（改名会让命令面与模型字段名分叉）。

#### Scenario: 四处计数随动且持衡

- **WHEN** 运行 `bindings/mod_test.rs` 的清单守卫
- **THEN** `COMMAND_NAMES.len() == 57`、`COMMAND_WRAPPERS.len() == 57`、`DTO_TYPES.len() == 105`、`ARCHI_COMMANDS.len() == 9`、`ARCHI_DTO_TYPES.len() == 22`；两两持衡（命令名 ↔ 包装名一一对应），清单首锚 / 尾锚逐字不变

#### Scenario: 生成物含新命令与新类型

- **WHEN** 跑一次 `pnpm run bindings:export` 后审查 `bindings.ts` / `bindings.zod.ts`
- **THEN** 生成物含 `archiRelation` 的 typed 包装（invoke 串 `"archi_relation"`、参数键 `root` / `from` / `to`）、`export type ArchiRelationDetail = { … }` 与 `ArchiRelation` 的 `apis` 字段；`argsSchemas` / `resultSchemas` 各含对应表项；`git diff` 显示这些行只由重导出产生（零手改痕迹）

#### Scenario: 字段级进场不改清单成员

- **WHEN** 审查 `DTO_TYPES` 与 `ARCHI_DTO_TYPES` 的条目集合（`ArchiRelation` 的 `apis` 进场后）
- **THEN** 两份清单的成员集合与改动前相同（仅长度因 `ArchiRelationDetail` +1）；`ArchiRelation` 仍在册且不重复登记；首尾锚断言零改动

#### Scenario: 清单不同步即红

- **WHEN** 只改命令登记面（`all_commands!` 加一行）而不同步 `COMMAND_NAMES` / `COMMAND_WRAPPERS` / `ARCHI_COMMANDS`，或只加 DTO 而不同步两份 DTO 清单
- **THEN** 清单守卫报红并点名缺失条目（长度断言或逐名命中失败），MUST NOT 静默通过

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 清单守卫（四处计数 + 序断言） | `COMMAND_NAMES` / `COMMAND_WRAPPERS` 56 → 57（`archi_relation` / `archiRelation` 插在 `archi_api_generate` 之后、`read_file` 之前）；`DTO_TYPES` 104 → 105 与 `ARCHI_DTO_TYPES` 21 → 22（`ArchiRelationDetail` 按字典序插在 `ArchiRelation` 之后）；尾部追加序期望 vec 随动；「长度只作连带核对」口径逐字保留 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | 生成物（非手改） | 新增 `ArchiRelationDetail`、`ArchiRelation.apis`、`commands.archiRelation` 包装与两份 schema 表项；类型 / 字段序由生成器决定 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 命令登记面（见 `desktop-archi-model`） | `all_commands!` archi 段 8 → 9（`archi_relation` 追在 `archi_api_generate` 之后、`read_file` 之前） |
| `packages/desktop/src/lib/ipc-contract.test.ts` | 前端载荷语料（随动） | DTO 夹具补 `ArchiRelationDetail` 与 `ArchiRelation.apis`；既有断言的字段集随动（零新增判据语言） |

### API: 生成物出线面（`bindings.ts`）

| 项 | 变化 | 描述 |
|---|---|---|
| `commands.archiRelation` | 新增 | `(root: string, from: string, to: string) => Promise<ArchiRelationDetail>`；invoke 串 `"archi_relation"` |
| `type ArchiRelationDetail` | 新增 | `{ from, to, description, mark, reason, modelFile, modelLine, declared, used, usedNotDeclared, declaredNotUsed, occurrences }` |
| `type ArchiRelation` | 字段级 | 增 `apis: string[]`（字段不省略） |
| `argsSchemas` / `resultSchemas`（`bindings.zod.ts`） | 新增表项 | 与命令签名 / 返回类型逐字对应 |
