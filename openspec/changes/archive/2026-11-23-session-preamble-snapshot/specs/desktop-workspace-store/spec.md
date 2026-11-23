# desktop-workspace-store Delta

## ADDED Requirements

### Requirement: 会话系统提示词快照记录（写一次冻结）

store SHALL 以**独立模型** `SessionPreambleRecord` 承载 agent 会话的系统提示词快照（语义与拍存时机归 `desktop-agent-execution`「会话系统提示词快照的拍存与续轮复用」；本能力只承载持久形态与操作面）：

- **模型形态**：workspace 维度（落所属 workspace 的独立 db 文件，进 workspace 模型组与记录信封注册表）；`#[native_model(id = 19, version = 1)]`——id 取现役最大值 + 1（沿 `NotificationRecord` 注释约定：不补历史空洞 3 / 14，上界之外必不冲突）+ `#[native_db]`；`#[serde(rename_all = "camelCase")]` + `specta::Type`（表与字段说明经 `///` doc 由 `describe` 自动派生，db 检视页与 `db_tables` 零模型特定代码）。
- **字段面**：`session_id: String`（`#[primary_key]`，core 铸会话 id，与 `SessionRecord` 一对一；**非外键约束**——跨表引用不引申为存在性检查或级联删除）；`content: String`（快照正文；**空串 = 快照成立但无正文**，与「无行 = 未快照」是两个不同状态，MUST NOT 混用）；`saved_at: i64`（拍存时刻，UTC unix 毫秒，由 store 侧时钟取）。其余元信息（体量 / 摘要 / 指纹）MUST NOT 落库——体量按 `content` 长度导出，「文件是否已变更」的检测面不在本模型内。
- **无版本链**：本模型为**新增**模型而非既有模型的字段演进，`SessionRecord`（现 `version = 2`）与全部既有行零改动、零 `from` 链、零迁移代码路径；「未快照」由**行缺席**表达，MUST NOT 以 nullable 列或哨兵值表达。
- **写一次语义**：`stamp_session_preamble(session_id, content) -> Result<bool, StoreError>` SHALL 在单写事务内先查主键——行已存在 → 不写入并返回 `Ok(false)`（快照不可变）；不存在 → 写入并返回 `Ok(true)`。`saved_at` 由 store 侧取时钟（`now_millis` 口径，与 `create_session` / `bind_session_remote` 同源）。MUST NOT 提供「更新 / 覆盖 / 重置」语义的快照写函数。
- **读面**：`find_session_preamble(session_id) -> Result<Option<SessionPreambleRecord>, StoreError>` 按主键直查（miss → `Ok(None)`）。会话清单读面（`query_sessions` 系）MUST NOT 读取本模型（清单载荷与全表扫描成本纪律见 `desktop-agent-execution`「session_detail 单查面」）；`find_session_summary` SHALL 读本模型并把 `saved_at` / `content` 映射为出线 DTO。
- **信封与说明**：`MODEL_ENTRIES` SHALL 增 `session_preamble` 一行（`count` / `scan` / `key_of` / `schema` / `walk` 五件与既有行同型），使 `list_models` / `scan` / `describe` / `describe_table` / `db_query` 自动覆盖；`store_meta` 之外的模型清单计数随之 +1（workspace 维度）。MUST NOT 出现 per-table 手写清单或第二份字段说明。
- **不可变与保留**：本模型 MUST NOT 提供删除 / 修改操作面；会话与其快照同生命周期（同落一个 workspace 库文件，随库整体作废口径，见「workspace 库格式版本与旧库作废重建」），MUST NOT 新增级联删除代码路径。

#### Scenario: 写一次不改写

- **WHEN** 对同一 `session_id` 连续两次调用 `stamp_session_preamble`（正文不同）
- **THEN** 首次返回 `Ok(true)` 且行内容为首次正文；第二次返回 `Ok(false)`，库中 `content` / `saved_at` 均保持首次值

#### Scenario: 主键直查与缺席

- **WHEN** 分别对已拍快照会话与存量（无行）会话调用 `find_session_preamble`
- **THEN** 前者返回该行（含 `content` / `saved_at`）；后者返回 `Ok(None)`（不伪造空行、不报错）

#### Scenario: 信封与说明自动出表

- **WHEN** 在 workspace 库上调用 `list_models` / `scan("session_preamble", …)` / `describe` / `describe_table("session_preamble")`
- **THEN** 表清单含 `session_preamble` 且维度为 `workspace`；`scan` 返回记录信封（key = `sessionId`）；字段说明含 `sessionId` / `content` / `savedAt` 与各自 doc 文本（零模型特定代码）

#### Scenario: 旧库与旧会话前向兼容

- **WHEN** 以新版本打开本变更前的存量 workspace 库（无 `session_preamble` 表数据）
- **THEN** 打开成功、既有记录原样可读、`SessionRecord` 行零改动；`find_session_preamble` 对任一会话返回 `Ok(None)`，无迁移步骤、无版本 bump

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs` | 模型定义 | `SessionPreambleRecord`（`id = 19, version = 1`，workspace 维度；主键 `session_id`；`content` / `saved_at`；`///` doc 供 `describe` 派生） |
| `crates/infra/store/src/store.rs` | 模型注册与操作面 | `workspace_models()` 注册（doc 模型计数同步）；`stamp_session_preamble`（写一次，行存在即 `Ok(false)`）/ `find_session_preamble`（主键直查）/ `find_session_summary` 读快照并填 DTO；清单读面零读 |
| `crates/infra/store/src/envelope.rs` | 信封注册 | `MODEL_ENTRIES` 增 `session_preamble`（count / scan / key_of / schema / walk） |
| `crates/core/agent/src/session.rs` | 出线 DTO（消费方） | `SessionPreambleInfo { saved_at, content }` 与 `SessionSummary.preamble`（见 `desktop-agent-execution`）；本能力不定义 DTO、只提供持久形态 |

### Function: `packages/desktop/src-tauri/crates/infra/store`

| 项 | 签名 / 形态 | 描述 |
|------|------|------|
| `SessionPreambleRecord` | `{ session_id, content, saved_at }`，`#[native_model(id = 19, version = 1)]` + `#[native_db]` + `specta::Type` | 会话系统提示词快照；`content` 空串 = 快照成立但无正文；无版本链 |
| `stamp_session_preamble` | `fn(&self, session_id: &str, content: &str) -> Result<bool, StoreError>` | 单写事务写一次：行已存在 → `Ok(false)` 不改写；否则写 `saved_at = now_millis()` 并 `Ok(true)` |
| `find_session_preamble` | `fn(&self, session_id: &str) -> Result<Option<SessionPreambleRecord>, StoreError>` | 主键直查（miss → `None`） |
| `find_session_summary` | 签名零变更 | 装配时读快照并映射 DTO（`savedAt` / `content`）；清单面 `query_sessions` 零读本模型 |
