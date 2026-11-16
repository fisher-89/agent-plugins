# desktop-agent-db-tools Specification (Delta)

## ADDED Requirements

### Requirement: `db_tables` 表清单与字段说明

agent 工具面 SHALL 增只读发现工具 `db_tables`，使模型在取数前先知道「有哪些表、每张表是什么、每个字段叫什么、什么意思」：

- **无参形态**：返回表清单，每行 SHALL 含 `dimension`（`user` / `workspace`）、`table`（`db_query` 的 `table` 入参取值）、`count`（记录计数）、`summary`（表介绍）。清单 SHALL 覆盖记录信封注册表的**全部**已注册模型——`user` 维度 3 张（`workspace` / `agent_provider` / `agent_instance`）与 `workspace` 维度 11 张（`agent_run` / `session` / `session_event` / `explore` / `change` / `phase` / `checklist_item` / `step` / `run` / `run_node` / `run_edge`），`store_meta`（库级格式标记，内部治理记录）MUST NOT 出现。无参形态 MUST NOT 展开字段说明（清单约 1.4 KB，字段全量约 11 KB——每轮白烧）。
- **下钻形态**：给定 `table` 时返回该表字段清单，每字段 SHALL 含 `name`（**落库线名**，可直接喂 `db_query` 的 `field`）、`type`（渲染文本：`string` / `number` / `boolean` / `string | null` / 命名枚举名如 `ChangeStatus` / 嵌入对象名）、`doc`（字段说明）。
- **`scope` 入参**：可选 `"user" | "workspace"`，缺省两库全列（下钻形态缺省按表所属维度解析；`table` 名跨维度唯一由注册表保证）。
- 两形态的输出 SHALL 只由注册表 + 代码 doc 派生（见「说明来源与文档防腐」），`MUST NOT` 出现 per-table 手写清单或第二份字段表。
- 工具定义（`name` / `description` / `parameters`）SHALL 是 infra 挂载清单里的一项（定义与档位唯一真源），`MUST NOT` 在 core 词汇或相位提示词里另立一份。

#### Scenario: 无参返回表清单

- **WHEN** 在 explore 会话中调用 `db_tables`（无入参）
- **THEN** 返回 user 维度 3 张 + workspace 维度 11 张共 14 行，每行含 `dimension` / `table` / `count` / `summary`；`store_meta` 不在其中；响应体不含任何字段级说明

#### Scenario: 下钻单表字段说明

- **WHEN** 调用 `db_tables { "table": "phase" }`
- **THEN** 返回 `PhaseRecord` 全部字段（含 `id` / `changeId` / `verdict` / `timestamp` 等），每字段含落库线名、渲染类型文本与 doc 文本；字段名是 camelCase 线名（不是 snake_case 源码名）

#### Scenario: scope 收窄

- **WHEN** 分别以 `scope = "user"` 与 `scope = "workspace"` 调用无参形态
- **THEN** 前者只列 user 维度 3 张、后者只列 workspace 维度 11 张，两清单互不混列（与 DB 查看页 scope 口径一致）

#### Scenario: 未知表名显式失败

- **WHEN** 以下钻形态调用 `db_tables { "table": "phases" }`（拼写错误）
- **THEN** 显式 `Err` 并列出候选表名（错误文案即自纠通道）；不返回空清单、不静默成功

### Requirement: `db_query` 条件取数原语

agent 工具面 SHALL 增只读取数工具 `db_query`，以声明式结构表达「一次把要的取完」，语义由 store 侧查询原语单点实现：

```json
{ "table": "phase", "scope": "workspace",
  "where":  [ { "field": "changeId", "op": "=", "value": "2026-11-14-x" } ],
  "order":  { "field": "timestamp", "desc": true },
  "select": ["id", "phase", "verdict"],
  "offset": 0, "limit": 20 }
```

- **条件**：`where` 为条件数组，多项按 **AND** 组合（无 OR）；`op` SHALL 为封闭词表 `=` / `!=` / `>` / `>=` / `<` / `<=` / `LIKE` / `IN` / `IS NULL` / `IS NOT NULL`（词表由单点导出，工具描述据其拼接）。`field` SHALL 支持点号下钻（`event.kind`、`configSnapshot.model`），**数组下标 MUST NOT 支持**（`event.blocks.0.name` 报错并提示改看整条记录）。
- **LIKE**：`%` 匹配任意长度（含空串），支持前缀（`x%`）/ 后缀（`%x`）/ 包含（`%x%`）三形；无 `%` 时退化为等值（与 SQL 同）；`\%` / `\_` 表示字面 `%` 与 `_`；区分大小写；`_` 单字符通配、`ESCAPE` 子句、正则 MUST NOT 支持（标识符类数据里 `_` 通配极易误伤）。`null` 不匹配任何 `LIKE`。
- **比较**：`=` / `!=` 同类型比较，`null` 与任何值比较时 `=` 为 false、`!=` 为 true；`>` / `>=` / `<` / `<=` 按 `number` 数值序、`string` Unicode 码点序、`bool`（false < true），任一侧 `null` 不匹配；`IN` 的 `value` 必须是数组，逐元素按 `=` 语义；`IS NULL` / `IS NOT NULL` 不接 `value`。`null` 在排序中恒排末位。
- **排序**：`order` 缺省即主键自然序；给出 `order` 时 SHALL 在全量匹配集上排序后切页（不早停）。
- **投影**：`select` 为字段路径数组；输出 `value` 只保留选中路径（点号路径按嵌套对象重建），`key` 恒在（不占 `select` 名额）。
- **分页**：`offset` 缺省 0；`limit` 缺省 20、上限 1000，越界 SHALL 显式 `Err`（MUST NOT 静默钳位——钳位会让模型把「被压到 22 行」误读为「库里只有 22 行」）。
- **出参**：`{ table, total, matched, returned, truncated, truncatedBy, records: [{ key, value }] }`。`total` = 该表全量计数、`matched` = 过 `where` 后的行数、`returned` = 本页实际返回行数，三者 SHALL 可区分（`returned ≤ matched ≤ total`）。
- 查询实现 MUST NOT 复用查看页分页上限 `MAX_SCAN_LIMIT`（500）：查询面的 `limit` 上限是 agent 表达力，查看页的单页上限是 IPC 载重，两者口径独立。
- 只读：本工具与 store 侧查询原语 MUST NOT 提供任何写操作；`total` / `matched` 与 `records` SHALL 取同一读事务快照。

#### Scenario: 索引字段等值 + 点号下钻

- **WHEN** 调用 `db_query { "table": "session_event", "where": [{ "field": "sessionId", "op": "=", "value": "ses-0-…" }, { "field": "event.kind", "op": "=", "value": "message" }], "select": ["sessionId", "event.kind", "event.role"], "limit": 20 }`
- **THEN** 只返回该会话中 `event.kind === "message"` 的记录，`value` 仅含选中的三个路径（`event` 被重建为只含 `kind` / `role` 的对象），`key` 仍在

#### Scenario: 比较与 null 语义

- **WHEN** 分别以 `{ "field": "verdict", "op": "!=", "value": null }`、`{ "field": "attempt", "op": ">", "value": 1 }`、`{ "field": "phase", "op": "LIKE", "value": "dev-%" }`、`{ "field": "attempt", "op": "IN", "value": [1, 2] }` 查询
- **THEN** 四者按上述语义过滤（`!= null` 命中非空行；`>` 按数值；`LIKE` 前缀匹配 `dev-design` 类值；`IN` 逐元素等值）；多条件并存时按 AND 收窄

#### Scenario: 排序与分页三元口径

- **WHEN** 对 `phase` 以 `order: { field: "timestamp", desc: true }`、`offset: 20`、`limit: 20` 查询
- **THEN** 全量匹配集按 `timestamp` 降序（`null` 排末位）后取第 21~40 行；`total` 为表全量、`matched` 为过滤后行数、`returned` 为本页行数，三者互不替代

#### Scenario: limit 越界显式拒绝

- **WHEN** 以 `limit: 2000`（超上限）调用 `db_query`
- **THEN** 显式 `Err` 报上限 1000（不静默钳位、不返回 1000 行装作成功）

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
- **THEN** 表清单恰为 14 张记录表，`AgentModelTiers` / `SessionConfigSnapshot` / `ChangeActivePhase` 等内嵌命名类型不出现

#### Scenario: 表介绍按句读截断

- **WHEN** 查看任一含长 doc 的表的 `summary`（如 `phase` 的 `changeId` 说明所在表或 `agent_run`）
- **THEN** 截断发生在句读边界（`。` / `；` / 换行）或 120 字兜底处，MUST NOT 截出半句

### Requirement: 双档收口（行数上限与字节预算）

`db_query` SHALL 以「行数 + 字节预算」两档收口，任一先到即停（单结果上限 L1 = 50 KB 是硬墙，撞上去会在 JSON 中间截断；预算闸必须在它之前生效）：

- 行数档：`limit` 缺省 20、上限 1000（见「`db_query` 条件取数原语」）。
- 字节档：工具内 SHALL 有 40 KB 量级的字节预算，累加至「加入该条将超预算」时停止，返回已收部分。
- 两档任一触发 SHALL 置 `truncated: true` 并以 `truncatedBy: "limit" | "bytes"` 指明触发者；未触发时 `truncated: false`。
- 截断时 SHALL 明示收缩方式（用 `select` 收窄字段 / 加 `where` 条件 / 翻 `offset` 并提示深翻在大表上较慢），且必须如实回报 `returned < matched`（或 `returned < limit` 的行数档）。
- 返回体 SHALL 恒为**合法完整 JSON**：单条记录要么整条在结果里，要么不在——MUST NOT 出现被字节切开的半截 JSON。
- 出参体量 SHALL 恒低于 L1 单结果上限（`MAX_RESULT_BYTES` 50 KB），使 `tools::cap_result` 对 `db_query` 结果永不实际触发。
- 工具描述 SHALL 声明「上限是名义值、预算是实际生效值」这一事实，避免模型把「只回 22 行」误读为「库里只有 22 行」。

#### Scenario: 字节预算先到者止

- **WHEN** 对 `session_event`（行均约 1.8 KB）以 `limit: 50`、不投影查询
- **THEN** 返回体停在 40 KB 预算处（约 20 余条整条记录），`truncated: true`、`truncatedBy: "bytes"`、`returned < matched`，并带「用 select 收窄 / 加 where / 翻 offset」提示；无半截 JSON

#### Scenario: 行数档截断

- **WHEN** 对 `phase`（行均数百字节）以 `limit: 20` 查询而匹配行数 > 20
- **THEN** `truncated: true`、`truncatedBy: "limit"`、`returned == 20`、`returned < matched`

#### Scenario: 缺省 limit 落在预算内

- **WHEN** 对最坏表 `session_event` 以缺省参数（无 `limit`）查询
- **THEN** 走缺省 `limit = 20`，结果体落在 40 KB 预算内、`truncatedBy` 为 `"limit"` 或 `"bytes"` 之一且如实回报；MUST NOT 触发 `cap_result` 的 50 KB 截断留痕

#### Scenario: 结果永不撞 L1 硬墙

- **WHEN** 以 `limit: 1000` 查询任一表
- **THEN** 结果体仍低于 50 KB（预算先停），`tools::cap_result` 的截断分支对该工具的输出不可达

### Requirement: 错误文案即自纠通道

`db_tables` / `db_query` 的失败面 SHALL 显式 `Err`（`is_error` ToolResult 回灌，run 不中断），且错误文案 SHALL 足以让模型自我纠正：

- 未知 `table` → `Err` + 候选表名清单。
- 未知 `field`（含蛇形名 `change_id`）→ `Err` + 该表**候选线名**清单（这是「静默零命中」的唯一防线）。
- 未知 `op` → `Err` + op 词表全集。
- 值类型与字段类型不符（如以字符串查 `attempt`）→ `Err` + 该字段的渲染类型（静默 false 会让模型误以为查无数据）。
- 数组下标路径（`event.blocks.0.name`）→ `Err` + 「不支持下标，改看整条记录或用点号取对象字段」指引。
- `IN` 的 `value` 非数组 / `IS NULL` 带 `value` / `select` 非数组等形状错误 → `Err` + 期望形状。
- 上述错误 MUST NOT 退化为「返回空结果集」（空结果与拼错字段对模型不可区分，是本节存在的全部理由）。

#### Scenario: 未知字段列候选

- **WHEN** 以 `where: [{ "field": "change_id", "op": "=", "value": "x" }]` 查询 `phase`
- **THEN** 显式 `Err`，文案含 `change_id` 与候选线名（至少含 `changeId`），MUST NOT 返回空结果集

#### Scenario: 类型不符指名类型

- **WHEN** 以 `{ "field": "attempt", "op": "=", "value": "1" }`（字符串查数值字段）查询 `phase`
- **THEN** 显式 `Err` 并指明 `attempt` 的类型（`number`），不返回空集

#### Scenario: 未知 op 与形状错误

- **WHEN** 分别以 `op: "contains"`、`op: "IN"` 而 `value` 为字符串、`op: "IS NULL"` 而带 `value` 调用
- **THEN** 三者均显式 `Err`：前者列 op 词表全集，后两者给出期望形状

#### Scenario: 数组下标被拒

- **WHEN** 以 `field: "event.blocks.0.name"` 查询 `session_event`
- **THEN** 显式 `Err` 并提示不支持数组下标、改用点号取对象字段或整条查看

### Requirement: 只读档位与敏感字段

两工具 SHALL 为纯只读面，并对敏感字段施加字段级保护：

- **档位**：挂载清单中两工具的准入档 SHALL 为 `ToolTier::Read`，由此三档（`default` / `acceptEdits` / `bypassPermissions`）全部放行；MUST NOT 为两工具引入单独放行名单或第二套判据。
- **零介入**：两工具无路径参数，路径沙箱改写面对其恒原样返回；写入保护闸 MUST NOT 拦截（读面全开放是既有裁定）。
- **只读**：store 侧新增读面 MUST NOT 提供写入口；本轮不新增任何写事务。
- **敏感字段打码**：`agent_provider` 记录的 `api_key` 值在工具输出中 SHALL 恒为 `"***"`（含 `select` 显式投影到该字段时）。
- **敏感字段不可作比较面**：`api_key` SHALL 在 `where` / `order` 中被引用即 `Err`（理由：只打码输出仍留前缀探测通道——`LIKE 'sk-a%'` 可逐字符重建明文；数据面泄密不需要模型看到值）。`select` 引用 SHALL 允许且输出仍为 `"***"`（投影不构成比较通道，值与不查询等价）。
- **自读警示**：`session` / `session_event` 是本会话自身转录所在表，工具描述 SHALL 明示「查询它们会把当前对话内容再次喂回上下文」；MUST NOT 为此建立会话级或表级黑名单（预算闸是唯一硬拦）。
- 打码与预算政策 SHALL 住在 agent 执行体，store 读面保持纯数据语义（MUST NOT 在数据层写 agent 政策）。

#### Scenario: 三档全放行

- **WHEN** 分别以 `default` / `acceptEdits` / `bypassPermissions` 三档发起 explore 会话并调用 `db_query`
- **THEN** 三档均放行到执行体（无 `permission_denied` notice）；路径沙箱与写入保护零介入

#### Scenario: api_key 打码

- **WHEN** 以 `scope: "user"`、`table: "agent_provider"` 查询（含 `select: ["id", "apiKey"]`）
- **THEN** 输出中 `apiKey` 值恒为 `"***"`，明文不出现在工具结果的任何位置

#### Scenario: api_key 不可作为比较面

- **WHEN** 分别以 `where: [{ "field": "apiKey", "op": "LIKE", "value": "sk-a%" }]`、`where: [{ "field": "apiKey", "op": "=", "value": "sk-…" }]`、`order: { "field": "apiKey" }` 查询 `agent_provider`
- **THEN** 三者均显式 `Err` 指明该字段不可用于过滤 / 排序；前缀探测与等值探测通道都不成立

#### Scenario: 读面无写入口

- **WHEN** 审查 store 新增读面与两工具执行体
- **THEN** 全部为读事务与纯函数加工，无任何写方法、无第二份 JSON 读取实现绕过注册表

### Requirement: 会话可见性与执行上下文铸造

两工具 SHALL 只在指定来源的会话里可见，执行上下文按名字清单 per-turn 铸造：

- **可见臂**（名单单源见 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」）：`source = "explore"` 与 `source = "change"` 两会话臂 SHALL 下发 `db_tables` 与 `db_query`；`source = "archi"` 与其余来源（debug 等）MUST NOT 下发。
- **执行上下文**：`DbContext`（全局库句柄 + 会话 workspace 库句柄）SHALL 由组合根经门面 builder 注入，sdk 泵在起播时以**名字清单包含任一 DB 工具名**为判据 per-turn 铸造（与 `create_explore_note` / archi 三工具同一类判据），MUST NOT 用枚举比较或自由字符串嗅探。
- **身份锚**：workspace 库句柄 SHALL 取会话的 workspace root 库实例（change 会话 cwd 是 worktree，但库身份锚恒为主仓 root）；全局库句柄 SHALL 经 `WorkspaceStores` 的共享出口取得（redb 单进程单句柄，MUST NOT 二次 open）。
- **scope 缺省**：`workspace`（会话恒在某个 workspace 里跑）；`user` 走全局库。
- **上下文缺席**：`DbContext` 缺席时执行体 SHALL 以 `is_error` ToolResult 回灌（生产不可达：两臂恒带名字）。
- **成员校验**：未下发两工具的会话调用它们 SHALL 走既有成员校验拒绝（`SystemNotice{subtype:"tool_unavailable"}` + `is_error`，run 不中断），MUST NOT 触执行体。
- **提示词零改动**：16 份静态相位提示词 MUST NOT 提及两工具（工具描述自足；在 prompt 里许诺未下发工具面等于制造幻觉调用）。

#### Scenario: explore 与 change 两臂可见

- **WHEN** 分别以 `source="explore"` 与 `source="change"`（任意 `source_ref`）发起会话并审查该轮下发面
- **THEN** 两臂 `RunStarted.tools` 均含 `db_tables` / `db_query`；change 臂不分 phase / 不分矩阵命中（`test-gen` 等矩阵未命中行同样含）

#### Scenario: archi 与 debug 臂不含

- **WHEN** 分别以 `source="archi"` 与 `source="debug"` 发起会话并调用 `db_query`
- **THEN** 两臂下发面均不含两工具；调用被成员校验拒绝（`tool_unavailable` + `is_error` 回灌，run 不中断），执行体未被触达

#### Scenario: 双库 scope 与身份锚

- **WHEN** 在 change 会话（cwd 为 worktree）中分别以 `scope: "workspace"` 与 `scope: "user"` 查询
- **THEN** 前者读到主仓 root 对应的 workspace 库（非 worktree 路径派生的库）、后者读到全局库；两库记录互不混列，且进程内无二次 open（单开纪律不破）

#### Scenario: 上下文按名字清单铸造

- **WHEN** 某轮会话的名字清单不含任一 DB 工具（如 debug 臂），或含其一（explore / change 臂）
- **THEN** 前者不铸造 `DbContext`，后者铸造且随 `LoopTurn` 下发；两者均不影响其余工具行为

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/infra/store/src/envelope.rs`（扩） | 记录信封注册表 | `MODEL_ENTRIES` 11 → 14 行（补 `run` / `run_node` / `run_edge` + 三个 `key_of`）；`ModelEntry` 增 `schema` / `walk` 两字段与 `entries()` / `entry()` / `dimension_of` 读出口；`list_models` / `scan` / `MAX_SCAN_LIMIT = 500` 逐字不动 |
| `crates/infra/store/src/describe.rs`（新） | 表 / 字段说明派生 | `tables(db, dimension) -> Vec<ModelDoc>` / `table(dimension, name) -> Result<TableDoc, QueryError>`（未知表 `Err` 携本维度候选）：`Types` + `specta_serde::Format` 派生表介绍与字段说明（线名 / 渲染类型 / doc），按注册表过滤内嵌命名类型；`SessionEventRecord` 三字段单独供给；doc 按句读截断 |
| `crates/infra/store/src/query.rs`（新） | 只读查询原语 | `query(spec: &QuerySpec) -> Result<QueryPage, QueryError>`：字段校验（未知表 / 字段 / op 与类型不符显式 `Err` 携候选）→ 过滤 → 排序 → 投影 → 分页（`limit` 上限 1000，`MUST NOT` 复用 `MAX_SCAN_LIMIT`）；`QUERY_OPS` 十词单点导出（工具 description 拼接同源） |
| `crates/infra/store/src/store.rs`（扩） | 实例级薄包装 + 双库共享 | `Store::describe()` / `Store::describe_table(&str)` / `Store::query(&QuerySpec)` 三行委托；`WorkspaceStores.global: Arc<Store>` + `global_shared() -> Arc<Store>`（`global()` 语义不变）；`for_root` 缓存与单开纪律零改动 |
| `crates/infra/store/src/model.rs`（doc 补齐） | schema 文档来源 | 12 条字段 `///` 补齐（`PhaseRecord` ×7 / `ChecklistItemRecord` ×3 / `RunNodeRecord` ×2）；4 个 `event_key_serde` 字段 `#[specta(type = String)]`；模型版本头与字段面零改动 |
| `crates/infra/store/src/lib.rs`（导出） | 公共读面词汇 | `mod describe;` / `mod query;` 声明；`pub use` 增 `ModelDoc` / `FieldDoc` / `TableDoc` / `QuerySpec` / `QueryCond` / `QueryOrder` / `QueryPage` / `QueryError` / `DEFAULT_QUERY_LIMIT` / `MAX_QUERY_LIMIT` / `QUERY_OPS`（**不 derive `Type`**，零 IPC）；补导出 `RunRecord` / `RunNodeRecord` / `RunEdgeRecord` |
| `crates/infra/agent/src/sdk/db_tool.rs`（新） | 两工具执行体 | `pub(crate) struct DbContext { global: Arc<Store>, workspace: Arc<Store> }`；`async fn execute(&self, name: &str, input: &Value) -> Result<String, String>`：scope 解析 / 40 KB 字节预算 / 截断提示 / `api_key` 打码 / 错误文案装配；`MUST NOT` 实现第二份过滤排序语义 |
| `crates/infra/agent/src/sdk/mount.rs`（扩） | 挂载单源 | `DB_TABLES_TOOL` / `DB_QUERY_TOOL` 常量；两个 `ToolDefinition`（description 覆盖 op 词表 / scope / 预算 / 自读警示）；`ToolExec::Db` 变体；tier `ToolTier::Read`；`TOOL_NAMES` 七件不动（挂载清单 11 → 13 项） |
| `crates/infra/agent/src/sdk/{loop,runner}.rs`（扩） | 分派与 per-turn 铸造 | `LoopTurn.db: Option<DbContext>`；`ToolExec::Db` 分派臂（上下文缺席 `is_error` 回灌）；`PumpPayload` 双库句柄 + 泵起播按 `contains_tool` 判据铸造 |
| `crates/infra/agent/src/{compose,lib}.rs`（扩） | 装配缝与两臂名单 | `EngineFacade::with_db_stores(...)`（与 `with_explore_note_store` 同型）；explore 臂名字清单 +2；change 臂矩阵之后恒 push 两名 |
| `crates/infra/agent/src/sdk/tools.rs`（扩） | 沙箱改写豁免 | `input_path` 增两 DB 工具名显式 `None` 臂（照 archi 三工具先例）；七工具 `execute` 签名零改动 |
| `crates/infra/agent/src/sdk/{policy,sandbox,protect}` | 零改动面 | `Read` 档三档放行天然覆盖；MUST NOT 引入第二份按名清单；`policy_test.rs` 的逐工具三档等价表随挂载名数 11 → 13 补齐（期望随动，不改判据） |
| `src-tauri/src/bindings` / `packages/desktop/src/**` / commands 生产面 | 零改动面 | 无新 IPC 命令、无 `specta::Type` 溢出；DB 查看页在注册表补齐后自动多出 `run` / `run_node` / `run_edge` 三表（既有「新模型零改动可浏览」红利） |
| `src-tauri/src/commands/db/mod_test.rs` | 注册表补齐的期望随动 | workspace 维度清单期望 8 → 11 行（含 `run` / `run_node` / `run_edge`）与计数向量、用例名同步；`db_models` / `db_records` 的生产签名与行为零改动（测试期望随动，非命令面改动） |
