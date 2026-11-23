# desktop-agent-db-tools Delta

## MODIFIED Requirements

### Requirement: `db_tables` 表清单与字段说明

agent 工具面 SHALL 增只读发现工具 `db_tables`，使模型在取数前先知道「有哪些表、每张表是什么、每个字段叫什么、什么意思」：

- **无参形态**：返回表清单，每行 SHALL 含 `dimension`（`user` / `workspace`）、`table`（`db_query` 的 `table` 入参取值）、`count`（记录计数）、`summary`（表介绍）。清单 SHALL 覆盖记录信封注册表的**全部**已注册模型——`user` 维度 4 张（`workspace` / `agent_provider` / `agent_instance` / `notification`）与 `workspace` 维度 12 张（`agent_run` / `session` / `session_event` / `explore` / `change` / `phase` / `checklist_item` / `step` / `run` / `run_node` / `run_edge` / `session_preamble`），`store_meta`（库级格式标记，内部治理记录）MUST NOT 出现。无参形态 MUST NOT 展开字段说明（清单约 1.4 KB，字段全量约 11 KB——每轮白烧）。
- **下钻形态**：给定 `table` 时返回该表字段清单，每字段 SHALL 含 `name`（**落库线名**，可直接喂 `db_query` 的 `field`）、`type`（渲染文本：`string` / `number` / `boolean` / `string | null` / 命名枚举名如 `ChangeStatus` / 嵌入对象名）、`doc`（字段说明）。
- **`scope` 入参**：可选 `"user" | "workspace"`，缺省两库全列（下钻形态缺省按表所属维度解析；`table` 名跨维度唯一由注册表保证）。
- 两形态的输出 SHALL 只由注册表 + 代码 doc 派生（见「说明来源与文档防腐」），`MUST NOT` 出现 per-table 手写清单或第二份字段表。
- 工具定义（`name` / `description` / `parameters`）SHALL 是 infra 挂载清单里的一项（定义与档位唯一真源），`MUST NOT` 在 core 词汇或相位提示词里另立一份。

#### Scenario: 无参返回表清单

- **WHEN** 在 explore 会话中调用 `db_tables`（无入参）
- **THEN** 返回 user 维度 4 张 + workspace 维度 12 张共 16 行，每行含 `dimension` / `table` / `count` / `summary`；`store_meta` 不在其中；响应体不含任何字段级说明

#### Scenario: 下钻单表字段说明

- **WHEN** 调用 `db_tables { "table": "phase" }`
- **THEN** 返回 `PhaseRecord` 全部字段（含 `id` / `changeId` / `verdict` / `timestamp` 等），每字段含落库线名、渲染类型文本与 doc 文本；字段名是 camelCase 线名（不是 snake_case 源码名）

#### Scenario: scope 收窄

- **WHEN** 分别以 `scope = "user"` 与 `scope = "workspace"` 调用无参形态
- **THEN** 前者只列 user 维度 4 张、后者只列 workspace 维度 12 张，两清单互不混列（与 DB 查看页 scope 口径一致）

#### Scenario: 未知表名显式失败

- **WHEN** 以下钻形态调用 `db_tables { "table": "phases" }`（拼写错误）
- **THEN** 显式 `Err` 并列出候选表名（错误文案即自纠通道）；不返回空清单、不静默成功

### Requirement: 说明来源与文档防腐

表介绍与字段说明 SHALL 由代码派生，`MUST NOT` 存在第二份手写文档表：

- 说明来源 SHALL 为 `specta::Type` 运行时类型图：`<T as specta::Type>::definition` 产出的 `NamedDataType.docs`（表介绍）与每字段 `Field.docs`（字段说明），经 `specta_serde::Format.map_types` 应用 serde 重命名后取**落库线名**。
- **线名是硬约束**：未应用 `Format` 会产出 snake_case（`change_id`），模型据此查询**恒零命中**——`describe` 与 `query` 的字段面貌 MUST 同源（同一名字空间），MUST NOT 一处 camelCase 一处 snake_case。
- 表介绍 SHALL 按首个句读（`。` / `；` / 换行）截断并以 120 字为兜底上限（`PhaseRecord.change_id` 一类整段 doc 不得截出半句）。
- 类型渲染 SHALL 为薄 formatter：`Primitive` 按位宽收成 `number` / `string` / `boolean`，`Reference(Named)` 取该命名类型名（`NamedReference` 无 `Display`，经 `types.get(&r)` 取 `name`），`Nullable` 渲染 `T | null`，`List` / `Map` 渲染为对应形态。
- 类型图 SHALL 按记录信封注册表名单过滤——内嵌命名类型（`AgentModelTiers` / `SessionConfigSnapshot` / `ChangeActivePhase` 等）MUST NOT 出现在表清单里。
- `SessionEventRecord` 不做 IPC 出线、`event` 载荷为 `Box<RawValue>`，其 3 个字段说明 SHALL 单独供给（不强制给它补 `Type` derive），但字段集与线名 MUST 与其真实 serde 键集一致。
- **ratchet 测试**：每个已注册模型的每个字段 SHALL 有非空 doc，且其线名集 SHALL 与各模型真实 serde JSON 键集**双向一致**——新增字段不写说明即测试红、重命名即测试红。文档不得不靠 review 纪律防腐。
- 4 个 `#[serde(with = "event_key_serde")]` 字段（`ChecklistItemRecord.item_key` / `SessionEventRecord.event_key` / `RunNodeRecord.node_key` / `RunEdgeRecord.edge_key`）SHALL 显式声明线格式为字符串（`#[specta(type = String)]`），否则 `Format.map_types` 直接 `Err`；落库与线格式 MUST NOT 因此改变（仍是十六进制串）。

#### Scenario: 新字段不写说明即红

- **WHEN** 给任一已注册模型新增一个字段而不写 `///` 说明，运行 store 测试
- **THEN** ratchet 测试失败并指名该模型与字段；补上说明后转绿（`describe` 输出自动带上该字段）

#### Scenario: 线名与真实 serde 键集一致

- **WHEN** 对任一模型比较 `db_tables` 下钻返回的字段名集与 `scan` 返回记录 JSON 的键集
- **THEN** 两集合逐一对应（camelCase 线名，无 `change_id` 一类蛇形名）；`event` / `configSnapshot` 一类嵌套对象名与值形态一致

#### Scenario: 内嵌类型不入表清单

- **WHEN** 审查 `db_tables` 无参输出
- **THEN** 表清单恰为 16 张记录表，`AgentModelTiers` / `SessionConfigSnapshot` / `ChangeActivePhase` 等内嵌命名类型不出现

#### Scenario: 表介绍按句读截断

- **WHEN** 查看任一含长 doc 的表的 `summary`（如 `phase` 的 `changeId` 说明所在表或 `agent_run`）
- **THEN** 截断发生在句读边界（`。` / `；` / 换行）或 120 字兜底处，MUST NOT 截出半句

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/envelope.rs` | 表清单来源（唯一） | `MODEL_ENTRIES` 增 `session_preamble` 一行，`db_tables` 无参形态自动多一行（表清单与字段说明仍只由注册表 + 代码 doc 派生，零手写清单） |
| `crates/infra/store/src/model.rs` | 字段说明来源 | `SessionPreambleRecord` 三个字段各带 `///` doc（线名 `sessionId` / `content` / `savedAt`），ratchet 测试自动纳入 |
| `crates/infra/agent/src/sdk/db_tool.rs`（消费面） | 零改动 | 工具定义 / 入参 / 只读档位 / 双档收口语义逐条不变；清单与下钻输出随注册表自动变化 |
| 本 spec 的计数面 | 文案 | 本变更只改计数与名单（`workspace` 11 → 12、合计 15 → 16）；基线文本的「`user` 3 张 / 合计 14」是 `notification` 登记后未同步的陈旧计数，随本变更一并纠正为 4 张 / 16 张。工具契约零变化 |
