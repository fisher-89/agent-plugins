# desktop-workspace-store Specification (Delta)

## ADDED Requirements

### Requirement: 信封新增字段的零迁移口径

会话转录的事件载荷 SHALL 以 **JSON 原文**存储（`SessionEventRecord.event: Box<RawValue>`，写入时序列化一次、读取时现场反序列化），MUST NOT 以类型化重建回写。由此确立信封字段增量的落库纪律：

- **零结构迁移**：`AgentEvent` 增字段（带 `#[serde(default)]`）SHALL NOT 触发新增列、`#[native_model]` 版本号提升、版本升级链或任何迁移代码路径——转录模型版本头保持不变，既有 workspace 库文件照常打开（`event_key` 打包键 = `(hash64(session_id), seq)` 亦不变，因为新字段不参与键）。
- **旧行可读**：缺该键的存量行经 `SessionEventRecord::event()` 读出 SHALL 得默认值（如 `turn_id = None`），MUST NOT 因缺键使**整条事件**解码失败。
- **字节不变**：旧行 record 层 decode → encode SHALL 逐字节不变（`native_db` 的 `remove` 以「库内字节 == 当前重编码字节」严格判等，补默认值即误报 `IncorrectInputData`）——这是「载荷原文回写、不做类型化重建」的直接推论。注意两层不可混：`AgentEvent` 层往返会给 `None` 落出显式的 `"turnId":null`（键恒在纪律），故 MUST NOT 以「读出 `AgentEvent` → 重新序列化 → 覆盖存量行」的方式改写既有转录原文；新字段只随之进入**新写入**的行。
- **字段不参与键与统计**：新字段 MUST NOT 进入转录主键、二级索引、查询过滤面或轮统计聚合（`list_session_events` / `transcript()` 仍按 `(session_id, seq)` 扫描）。

**按轮查询为后续能力**：载荷携带轮归属（见 desktop-agent-execution「AgentEvent 信封与 loop 可见性」）使「按轮分页加载 / 跳到某一轮」**在数据上可行**，但本要求 MUST NOT 被解读为已具备该能力——本期不建按轮索引、不加按轮查询 API、不改转录表结构；需要时以其自身变更在既有主键上扩展。

#### Scenario: 加字段不迁库

- **WHEN** 在带存量转录行的 workspace 库上以新增了信封字段的版本打开并继续写入
- **THEN** 打开成功、存量行原样可读；`SessionEventRecord` 字段面与 `native_model` 版本头零改动；无迁移代码路径被执行

#### Scenario: 旧行缺键读出默认值

- **WHEN** 取一条载荷缺新字段键的存量转录行，经 `SessionEventRecord::event()` 读出类型化事件
- **THEN** 新字段读出默认值（如 `turn_id = None`），其余字段与原值逐字段相等，整条事件解码不失败

#### Scenario: 旧行解码重编码字节不变

- **WHEN** 对缺新字段键的存量转录行执行 `native_model::decode` → `native_model::encode`（记录层，不经 `AgentEvent` 类型化重建）
- **THEN** 重编码字节与原文逐字节相同（缺键未被补成显式默认值）；该行仍可被 `native_db` `remove`（严格字节判等不破）

#### Scenario: 类型化重写口径被禁止

- **WHEN** 审查 store 的新增 / 更新路径
- **THEN** 既有转录行不经「读出 `AgentEvent` → 重新序列化 → 覆盖」的批量重写；`AgentEvent` 层往返（`None` → 显式 `"turnId":null`）只作用于新写入行，既有行原文一字不动

#### Scenario: 新行往返保真

- **WHEN** 以新字段有值（如 `turn_id = Some(7)`）的密封事件写入后回读
- **THEN** 载荷含该键且值相等，`seq` 排序与同会话全史连续性不变

#### Scenario: 按轮查询未随本要求落地

- **WHEN** 检索 store 的转录查询 API 与表结构
- **THEN** 仍只有 `(session_id, seq)` 扫描面，无按轮索引 / 按轮查询方法 / 新列；按轮能力属后续变更

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 转录形与载荷存取（**零改动**） | `SessionEventRecord { event_key, session_id, event: Box<RawValue> }` 字段面与 `#[native_model(id = 8, version = 1)]` 版本头逐字不变；`new()` 序列化原文、`event()` 现场反序列化（旧行缺键按 `#[serde(default)]` 补默认值，`None` 形态键恒在） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 转录查询面（**零改动**） | `list_session_events` / `transcript()` 仍按 `(session_id, seq)` 扫描；新字段不进键、不进过滤面、不进统计聚合 |
| `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs` | 零迁移口径回归 | 旧 schema 行（含缺新字段键形态）decode → encode 逐字节不变；`event()` 读出默认值；新行往返保真；`native_db` insert / scan / remove 全链不破 |

### Function: `packages/desktop/src-tauri/crates/infra/store/src/model.rs`（零改动）

| 项 | 签名 | 描述 |
|------|------|------|
| `SessionEventRecord::new` | `(session_id: &str, event: AgentEvent) -> Self` | 载荷一次序列化为 `Box<RawValue>` 原文（新字段随之自然落键），`event_key` 打包不变 |
| `SessionEventRecord::event` | `(&self) -> Result<AgentEvent, serde_json::Error>` | 现场反序列化；旧行缺键按 `#[serde(default)]` 补默认值 |
| `SessionEventRecord::{seq, session_id}` | `(&self) -> u64` / `(&self) -> &str` | 自打包键 / 索引列读取，不受信封字段增量影响 |

### API: `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（零改动）

| 命令 / 方法 | 签名 | 描述 |
|------|------|------|
| `list_session_events` | `(&self, session_id: &str) -> Result<Vec<AgentEvent>, String>` | 按 `(session_id, seq)` 全量扫描；信封新字段只随载荷透出，不参与过滤 |
| `sum_usage_tokens`（复用 `core/agent` 实现） | `(&[AgentEvent], &str) -> Option<u64>` | 只读 `TurnDone.usage`，信封字段透明 |
| 会话统计 / 对账 | 既有签名不变 | 轮统计行与聚合统计字段面零演进（不取 `turn_id`） |

### Component / CLI

无。本变更零组件、零脚本、零命令面。
