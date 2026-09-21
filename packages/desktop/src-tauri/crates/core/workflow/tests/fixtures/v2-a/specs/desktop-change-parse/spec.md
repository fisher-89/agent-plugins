# desktop-change-parse Specification

## ADDED Requirements

### Requirement: 三代结构代际探测

workflow crate SHALL 将 change 目录的代际判定为 `Inventory` 枚举（v2 / v1 / v0）：

- 有 `workflow.json` 且含 `file_log` → **v2**（现在：完整数据）
- 有 `workflow.json` 无 `file_log` → **v1**（中期：仅 eval 历史与旧 files 桶）
- 无 `workflow.json` → **v0**（早期：仅 markdown 产物）

判定 SHALL 基于磁盘事实（文件与字段存在性），SHALL NOT 依赖目录命名约定（archive 目录名的日期前缀仅作时间线分组素材，不参与代际判定）。

#### Scenario: v2 判定

- **WHEN** 解析含 `workflow.json`（有 `file_log` 字段）的 change 目录
- **THEN** 代际判定为 v2

#### Scenario: v1 判定

- **WHEN** 解析含 `workflow.json`（有 `eval` 无 `file_log`）的 change 目录
- **THEN** 代际判定为 v1

#### Scenario: v0 判定

- **WHEN** 解析仅含 proposal / design / tasks 等 markdown 产物、无 `workflow.json` 的 change 目录
- **THEN** 代际判定为 v0，且仍产出可展示的文档清单

### Requirement: serde 宽松解析与损坏降级

workflow crate SHALL 使用 serde 宽松解析 `workflow.json`：未知字段 SHALL 被忽略（含 legacy `files` 桶、`source` 旁挂映射），MUST NOT 因未知字段失败。单条损坏 SHALL 降级不炸整份：单条 `eval[]` 条目或单条 `file_log[]` 条目解析失败时，SHALL 跳过该条并保留其余数据，MUST NOT 使整个 change 的解析失败。必填核心字段（如 `workflow_type`）缺失或非法时，SHALL 将该文件整体降级为"不可解析"标记并仍返回 change 的文件系统层信息（代际、产物清单），MUST NOT panic。

解析层的字段形状 SHALL 以 TS 侧 `plugins/dev-team/bin/src/schemas/workflow.schema.ts` 为唯一真理源（v2：`workflow_type` / `created` / `eval[]` / `file_log[]` / `active_phase` / `interrupted[]`），Rust 侧不反向约束 TS schema。

#### Scenario: 未知字段忽略

- **WHEN** 解析含未知顶层字段与 legacy `files` 桶的 `workflow.json`
- **THEN** 解析成功，未知字段被忽略，已知字段正常产出

#### Scenario: 单条 eval 损坏降级

- **WHEN** `eval` 数组中某条缺 `verdict` 字段或类型非法
- **THEN** 该条被跳过（或以降级占位呈现），其余条目与 change 级数据正常返回

#### Scenario: workflow.json 整体损坏仍有产物

- **WHEN** 某 change 的 `workflow.json` 核心字段损坏不可解析
- **THEN** 该 change 被标记为不可解析，仍列出其目录内 markdown 产物，不 panic、不影响其他 change

#### Scenario: 时间戳字段兼容

- **WHEN** `eval` 条目的 `timestamp` / `active_phase.start_at` 为合法 ISO 8601 字符串
- **THEN** 解析为时间类型供耗时与分组使用；无法解析的值降级为空而不失败

## Module Contract

| 模块                                                  | 职责                   | 关键契约                                                                   |
| ----------------------------------------------------- | ---------------------- | -------------------------------------------------------------------------- |
| `workflow::model`                                     | 领域类型               | `Change` / `Workflow` / `Inventory`(v2\|v1\|v0)；不含 `openspec` 字样命名  |
| `workflow::parse`                                     | 宽松解析 + 代际探测    | serde 解析 workflow.json；未知字段忽略；单条损坏降级；核心字段损坏整体降级 |
| `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | 唯一真理源（只读参照） | Rust 侧对齐其 v2 形状；MUST NOT 修改该文件                                 |
