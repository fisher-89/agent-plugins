# desktop-ipc-type-bindings Specification（delta: archi-agent-context-review）

## Purpose

本 delta 只登记 `archi_module` 出线 DTO 的字段面追加与两份清单守卫的随动。

- **字段**：`ArchiModuleDetail` 增 `assets: ArchiAssetSet` 与 `reading: string`；既有八字段零变化。
- **类型**：新增 `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset` 四个出线类型，各进 `DTO_TYPES` 与 `ARCHI_DTO_TYPES`（+4，字典序插入），长度断言同批更新。
- **枚举**：`ArchiApiFindingKind` 联合十三 → 十值（三值离场）。
- **零新命令**：`COMMAND_NAMES` / `COMMAND_WRAPPERS` / `ARCHI_COMMANDS` 三清单零变化；生成物只经 `bindings:export` 产出。

## ADDED Requirements

### Requirement: archi 模块三资产的出线 DTO 面

`archi_module` 的响应体是出线 DTO（`ArchiModuleDetail`），本变更给它追加**三资产事实**字段与 4 个新出线类型，两份产物（`bindings.ts` / `bindings.zod.ts`）SHALL 经同一导出入口再生成：

| 项 | 形态 | 语义 |
|---|---|---|
| `ArchiModuleDetail.assets` | `ArchiAssetSet` | 三份资产的结构面（拓扑 / 接口 / 文档），三段各自 `present` |
| `ArchiModuleDetail.reading` | `String` | 一句话读法指引（静态文本） |
| `ArchiAssetSet` | `{ topology: ArchiTopologyAsset, interface: ArchiInterfaceAsset, doc: ArchiDocAsset }` | 新的聚合类型 |
| `ArchiTopologyAsset` | `{ file, line, out, in }` | 模型声明位置与出入度数 |
| `ArchiInterfaceAsset` | `{ path: String \| null, present: bool, bytes: u64, ops: String[], types: String[], reexports: String[], abstract: String }` | 接口键名摘要（**零 content**） |
| `ArchiDocAsset` | `{ path: String \| null, present: bool, bytes: u64, lines: u32, headings: String[] }` | 文档段名与体量（**零正文**） |

- **只增字段、不改既有字段**：`ArchiModuleDetail` 既有字段（`element` / `dependsOn` / `dependedBy` / `children` / `doc` / `api` / `resolvedEntry` / `fileHash`）的名字、类型、顺序 MUST NOT 变化；`ArchiApiRef` 四字段零变化（`status` 的取值语义换锚属实现面，形状不变）。
- **零新命令**：本变更不新增 IPC 命令——`COMMAND_NAMES` / `COMMAND_WRAPPERS` / `ARCHI_COMMANDS` 三份清单 SHALL 零变化；`DTO_TYPES` 与 `ARCHI_DTO_TYPES` 两份 DTO 清单 SHALL 各按新类型数（+4）追加并按字典序插入，长度断言同批更新（真实判据是「清单条目名与生成物逐名命中」，长度只作同步核对；MUST NOT 手抄旧值）。
- **枚举面收缩同批**：`ArchiApiFindingKind` 的联合由十三值收为十值（`apiReexportDrift` / `docNotFound` / `apiDocMissing` 离场）——该枚举 SHALL NOT 进两份 DTO 清单（它不是 DTO 类型），其变化只体现在 `bindings.ts` / `bindings.zod.ts` 的联合成员上。
- **生成物零手改**：全部条目 SHALL 来自 `bindings:export`（提交前人工核对 `git diff`；`check` 链只前置重导出、不做生成物比对）。
- **zod 面同随**：`bindings.zod.ts` 的 `resultSchemas` / DTO schema SHALL 含新字段与新类型；`degraded(...)` 标记面零新增（三资产的字段全可表达）。

#### Scenario: 新字段与四类型在案

- **WHEN** 跑一次 `pnpm run bindings:export` 后审查两份产物
- **THEN** `bindings.ts` 含 `export type ArchiAssetSet =` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset` 四条类型声明，且 `ArchiModuleDetail` 含 `assets: ArchiAssetSet` 与 `reading: string`；`bindings.zod.ts` 含对应 schema 且挂在 `archi_module` 的 `resultSchemas` 表项上

#### Scenario: 清单守卫四处对拍

- **WHEN** 运行 `bindings/mod_test.rs` 的清单守卫
- **THEN** `DTO_TYPES` 与 `ARCHI_DTO_TYPES` 各恰增 4 个新条目（按字典序插入、无重复），`COMMAND_NAMES` / `COMMAND_WRAPPERS` / `ARCHI_COMMANDS` 的成员与长度零变化；删掉任一新条目即红并点名

#### Scenario: 三值离场

- **WHEN** 审查 `bindings.ts` 与 `bindings.zod.ts` 的 `ArchiApiFindingKind`
- **THEN** `"apiReexportDrift"` / `"docNotFound"` / `"apiDocMissing"` 零命中；十值逐字在场；两份产物联合成员逐字相同

#### Scenario: 重导出幂等

- **WHEN** 连续两次执行导出入口而不改任何 Rust 类型
- **THEN** 第二次导出后 `git diff` 为空；生成目录只含本次变更产生的可归因差异（四类型 + 两字段 + 枚举收缩）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/dto.rs` | 出线 DTO 定义单点 | 新增 `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`（均 `specta::Type` + `serde(rename_all = "camelCase")`）；`ArchiModuleDetail` 追加 `assets` / `reading` 两字段（声明序在 `file_hash` 之后）；既有字段零改动 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 清单守卫 | `DTO_TYPES` 与 `ARCHI_DTO_TYPES` 各 +4（字典序插入）、计数断言同批更新；`COMMAND_*` 三清单零改动；`ArchiApiFindingKind` 十值断言 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | 生成物（再生成，零手改） | 四新类型 + 两新字段 + 枚举十值；提交前人工核对 |
| `packages/desktop/knip.json` | 边界（零改动） | `src/**/*.i.ts` 的 ignore 面不受影响（DTD 类产物是 `bindings.*`，非 `.i.ts`） |

### API: 出线类型（形状面）

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiModuleDetail` | 加两字段 | `assets` / `reading`；既有八字段零变化 |
| `ArchiAssetSet` 等四类型 | 新增 | 见正文形态表；`u64` 出线为 `number`（沿既有 bigint 配置口径） |
| `ArchiApiFindingKind` | 成员收缩 | 十三 → 十；线格式词逐字不变的那十个零改动 |
