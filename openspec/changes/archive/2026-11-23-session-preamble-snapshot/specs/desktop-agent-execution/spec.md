# desktop-agent-execution Delta

## ADDED Requirements

### Requirement: 会话系统提示词快照的拍存与续轮复用

会话的系统提示词 SHALL 以**一段会话一份快照**为语义：快照在会话首轮拍存、此后逐字复用，MUST NOT 逐轮重算。持久载体为 store 的 `SessionPreambleRecord`（workspace 维度、主键 `session_id`；模型形态与写一次语义单源见 `desktop-workspace-store`「会话系统提示词快照记录（写一次冻结）」）。三处判据与落点：

- **首轮判据**：`SessionRef::New`，或 `SessionRef::Continue{id}` 且 `Store::is_unstarted_session(id)` 为真（无引擎句柄 + 无轮行 + 无转录——与既有「首轮定型」`restamp_session_config` 同一判据来源，MUST NOT 另立第二套）。首轮 SHALL 在 `SessionCtx.workspace_root`（会话执行根；worktree 场景即分支版）读取一次 `AGENT.md`，与装配层知识合成快照正文（合成次序与截断口径见「SDK 引擎系统提示词（AGENT.md 注入）」），本轮直接使用该内存产物。
- **落库时机**：快照 MUST 在会话行与轮行建立之后落库（`New` 路径只有此刻才知会话 id），MUST NOT 早于会话建立；本轮 MUST NOT 依赖读回自己刚写的快照。落库经内核组合根直写 store（沿 `restamp_session_config` 同处同型先例），MUST NOT 为此扩 `SessionSink` 端口契约。
- **读点（续轮复用）**：`Continue` 且会话已开轮 SHALL 以 `Store::find_session_preamble(id)` 的正文**覆盖** `SessionInjections.preamble`；MUST NOT 读取 `AGENT.md`、MUST NOT 重算知识段。快照行存在而正文为空串 → 覆盖为 `None`（该会话零 System 消息，见「SDK 引擎系统提示词（AGENT.md 注入）」）。
- **存量补拍**：已开轮但无快照行的会话（本变更前创建）SHALL 在其下一次 `Continue` 时按首轮口径现算 + 落库 + 使用；此为首轮之外的**唯一**现算入口，且至多发生一次（写一次）。该次切换令该会话前缀一次性重建（缓存 miss 一次），属显式接受的代价。
- **写一次与不可变**：`stamp_session_preamble` SHALL 为写一次语义——行已存在即不再写入；快照一经落库 MUST NOT 被覆盖，MUST NOT 提供重载 / 刷新 / 编辑的命令面或前端写面。
- **失败降级**：快照落库失败 SHALL NOT 中断本轮（本轮使用内存中已装配的正文，日志留痕——沿 `bind_remote_session` 尽力语义先例）；该会话此后按存量补拍路径再尝试一次。
- **知识版本语义**：应用升级后旧会话 SHALL 沿用拍存时刻的知识文本与 `AGENT.md` 内容（前缀稳定优先），新会话自然取新内容；`AGENT.md` 缺席时拍下的快照 MUST NOT 因该文件后来出现而补入该段。
- **元信息**：快照 SHALL 落库 `saved_at`（拍存时刻，由 store 侧时钟取，沿「时间戳归落库侧」口径）以便出线呈现「冻结于何时」；其余元信息 MUST NOT 冗余落库——体量按正文长度导出即可；「用户改了 `AGENT.md` 但本会话未生效」的检测与提示面不在本能力内（未来落地时按既有 native_model 版本链演进字段面，另立变更）。
- **core 边界**：快照的合成 / 落库 / 读回全在 infra（内核组合根 + store）；`crates/core/agent` 只承载字符串（`SessionInjections.preamble`），MUST NOT 出现「快照」/「AGENT.md」概念，字段面零结构改动。
- **载荷纪律**：快照正文 MUST NOT 随会话清单返回（出口见「session_detail 单查面」）；清单读面 MUST NOT 触碰快照表。

#### Scenario: 首轮拍存与正文合成

- **WHEN** 以 `source = "archi"` 发起 `New` 会话且 workspace 执行根存在 `AGENT.md`
- **THEN** 会话行与轮行建立后库中出现该会话的 `session_preamble` 行，正文为「架构会话知识 + 空行 + `<agent_md …>` 段 + 空行 + 冻结指引常量」，`saved_at` 在场；本轮请求的 System 消息逐字等于该正文

#### Scenario: 续轮逐字复用且零文件读取

- **WHEN** 某会话首轮收口后修改（或删除）`AGENT.md`，随后以 `Continue` 续发下一轮
- **THEN** 下一轮 System 消息与首轮逐字相同（本轮不读文件、不重算知识）；即便该文件已不可读，续轮照常完成

#### Scenario: 存量会话补拍一次后冻结

- **WHEN** 一条本变更前创建、已开轮且无快照行的会话首次 `Continue`，随后修改库中该快照正文再次 `Continue`
- **THEN** 首次续轮现算并把正文落库（该轮即用新正文，前缀一次性重建）；第二次续轮仍使用首次落库的正文（写一次语义，二次写入不生效）

#### Scenario: 空快照不注入也不补入

- **WHEN** `source = "debug"`（无知识）且执行根无 `AGENT.md` 时首轮，之后在同一执行根创建 `AGENT.md` 再续轮
- **THEN** 首轮快照正文为空串、请求零 System 消息；续轮读回空正文仍零 System 消息，MUST NOT 因文件后来出现而改变本会话前缀

#### Scenario: 落库失败不中断本轮

- **WHEN** 首轮快照落库失败（库写错误注入）
- **THEN** 本轮照常跑完（System 消息使用内存中已装配的正文），日志留痕，MUST NOT 抛发起点 `Err`、MUST NOT 产生半成品会话

#### Scenario: 升级后旧会话沿用旧知识

- **WHEN** 某知识注入会话首轮拍存后，应用的 `archi` / `config` 知识文本随版本变化（测试缝替换生产者）再续轮，同时开一个新会话
- **THEN** 旧会话续轮 System 仍为首轮拍存的正文（旧知识）；新会话取其时刻的新知识

## MODIFIED Requirements

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎的系统提示词 SHALL 来自会话级注入位 `SessionInjections.preamble` 的**逐字**值：值在场 → chat_history 首条为**恰一条** `Message::System`（rig 0.43 起 `preamble` 字段退役，以首条 System 承载；不包装、不改写、不追加引擎侧自读内容）；值缺席 → 不注入 System 消息。引擎执行体（`sdk/loop.rs`）MUST NOT 为组装系统提示词读取任何文件（读取函数可仍驻 `sdk/` 模块、由装配层调用，但引擎执行体 MUST NOT 调用它）——`AGENT.md` 的读取与合并 SHALL 发生在装配层（内核组合根的会话首轮），产物为会话系统提示词快照并逐字复用于本会话后续每一轮（拍存 / 读回 / 存量补拍 / 写一次语义见「会话系统提示词快照的拍存与续轮复用」）。

`AGENT.md` SHALL 是快照的**成分之一**，读取口径逐条不变：读取根为会话执行根 `SessionCtx.workspace_root`（首轮一次读取，worktree 场景即分支版）；读取范围严格限于 `AGENT.md` 单份文件，MUST NOT 做 `CLAUDE.md` / `AGENTS.md` 等兜底回退；内容**逐字**（不删改、不重排）并以 `<agent_md path="…">…</agent_md>` 标签包裹；体量上限 32KB（char 边界截断，只作用于该文件自身）、嵌套 `AGENT.md` 不级联；文件缺席 → 快照不含该段（与注入机制上线前的语义一致）。

快照正文的合成次序 SHALL 为：会话 preamble 知识（`source = "archi"` 的架构会话知识见 `desktop-archi-agent`「架构会话的系统知识注入」；`source = "config"` 的配置协议知识见 `desktop-config-agent`「配置协议知识的静态 preamble」；其余来源恒无此段，「会话级系统前导不进 MVP」的裁定对它们继续生效）在前，`AGENT.md` 段在后，两段之间以空行分隔；**冻结指引常量**作为末段追加。合成产物即快照正文，单条 System 口径不变（工具面注入位仍自「会话级工具面注入与 explore 笔记工具」起按 provenance 来源装配，与本条的知识段来源判定同源）：

- **单侧在场**：仅知识在场 → 单条为「知识 + 空行 + 指引常量」；仅 `AGENT.md` 在场 → 单条为「`<agent_md …>` 段 + 空行 + 指引常量」；两者皆无 → 快照正文为空串、不注入 System 消息（也不追加指引常量——无正文可冻结，缺省语义维持不变）。
- **指引常量**：SHALL 为装配层静态常量（非前端 / 自由字符串供给），语义须表达三点——本会话系统提示词冻结于会话开始时、项目规则真身为 `AGENT.md`、用户提示规则已更新时应以 `read` 重新读取 `AGENT.md` 并按新规则执行。体量 SHALL ≤ 512 B（char 计），MUST NOT 含路径字面量（用文件名 `AGENT.md`）、MUST NOT 含 `<>` 一类占位符；MUST NOT 单独成条（只随非空正文出现）。
- **截断口径分立**：`AGENT.md` 的 32KB / char 边界截断口径不变；知识段与指引常量不在该截断逻辑内，体量由生产者侧测试锚定上限。
- **来源约束**：注入位只由装配层按 provenance 决定并经快照供给，前端 / 自由字符串 MUST NOT 直接供给；本机制仍属引擎 / 装配侧特性（P2 原则：跨会话持久上下文是 agent 产品特性、非协议义务）。`crates/core/agent` 本变更**零结构改动**（六位字段面与词汇不变，`preamble` 仍为 `Option<String>` 不透明承载，仅字段 doc 语义改写为「会话系统提示词快照正文」）。
- **缓存纪律**：会话内 System 消息 SHALL 逐字恒定——除「本会话首轮拍存」与「存量会话首次补拍」两个时刻外，任何一轮 MUST NOT 因 `AGENT.md` 或二进制内容变化而改变 System 消息。`context` 防线（L2 剪裁 / L3 compaction）改写的是历史消息而非 System 前缀，故剪裁时刻之后的前缀重建属**已承认的既有代价**，不在本条款承诺内。

#### Scenario: 首轮逐字注入与常量末段

- **WHEN** workspace 执行根存在 `AGENT.md`（`source = "debug"`，无知识段）时以 sdk 引擎发起首轮
- **THEN** 请求系统提示词恰一条 System 消息，内容为「`<agent_md path="…">…</agent_md>` 段（内含文件内容逐字，不删改不重排）+ 空行 + 指引常量」；MUST NOT 出现第二条 System 消息

#### Scenario: 缺席维持现状

- **WHEN** 执行根无 `AGENT.md` 且来源无知识段时发起会话
- **THEN** 请求不注入 System 消息，请求形状与该注入机制上线前一致（指引常量 MUST NOT 单独成条）

#### Scenario: 会话中修改不改前缀

- **WHEN** 会话运行中途修改 `AGENT.md` 后续发下一轮
- **THEN** 已发轮次不受影响，下一轮也 MUST NOT 改变 System 消息（首轮拍存后冻结）；规则更新走「用户提示 + `read` 重读」通道落在史尾

#### Scenario: 无兜底回退

- **WHEN** 执行根只有 `CLAUDE.md` 或 `AGENTS.md` 而无 `AGENT.md`
- **THEN** 不发生兜底读取，快照不含该段（严格单文件语义，复数形态不在读取源内）

#### Scenario: config 臂同样合为单条 System

- **WHEN** 以 `source = "config"` 发起会话且执行根存在 `AGENT.md`
- **THEN** chat_history 恰有一条 System 消息，内容为「配置协议知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」；MUST NOT 出现第二条 System 消息

#### Scenario: 会话 preamble 与 AGENT.md 合为单条

- **WHEN** 以 `source = "archi"` 发起会话且执行根存在 `AGENT.md`
- **THEN** chat_history 恰有一条 System 消息，内容为「架构会话知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」；MUST NOT 出现第二条 System 消息

#### Scenario: 仅单侧在场

- **WHEN** 分别以 `source = "archi"`（无 `AGENT.md`）与 `source = "debug"`（有 `AGENT.md`）发起会话
- **THEN** 前者 System 消息为「知识 + 空行 + 指引常量」；后者为「`<agent_md …>` 段 + 空行 + 指引常量」；两者皆无时零 System 消息

#### Scenario: 引擎零文件读取

- **WHEN** 审查 `crates/infra/agent/src/sdk/loop.rs` 并跑一轮续会话（注入位给定非空正文）
- **THEN** 源码中 `preamble::load` 与 `AGENT.md` 字样零命中；该轮 System 消息逐字等于注入位给定值（MUST NOT 被任何文件内容覆盖或追写）

### Requirement: session_detail 单查面

会话查询契约 SHALL 增加按 session id 的单查面：core `SessionQuery` trait SHALL 增加单查方法（按 id 返回会话行 + 聚合统计 + 轮行清单），infra 实现以 store `find_session` 为底座（会话行与轮行为既有表；本变更另读快照表以携带系统提示词快照面，见下），轮行的运行状态 SHALL 自轮行清单推导（存在 running 轮行即 running，否则取终态）。命令面 SHALL 提供薄包装命令 `session_detail(root, session_id)`（经 builder 注册、bindings 再生成；命令体三件事纪律）。语义约定：不存在 session id SHALL 显式 `Err`（单查语义与清单空态区分，查无此 id 视为调用方错误）；blank root SHALL 返回空结果（与 `agent_sessions` / `agent_session_transcript` 同口径）；单查 SHALL 按 root 解析所属 workspace 库，跨库隔离语义与既有查询命令一致。既有 `agent_session_transcript` 的签名与语义 MUST NOT 收窄或变更；`agent_sessions` SHALL 为分页查询面——`offset` / `limit` 入参、随页返回 `SessionPage { sessions, total }` 分页信封（`total` 为过滤集全量计数、越界 offset 空页恒 total、limit 超上限截断），来源过滤与 `updated_at` 降序稳定序语义不变。

单查 SHALL 另携**会话系统提示词快照面**：`SessionSummary.preamble: SessionPreambleInfo | null`，`SessionPreambleInfo` 字段面为 `{ savedAt, content }`（`savedAt` = 拍存时刻 UTC unix 毫秒；`content` = 快照正文全文，可能为空串）。约束：

- **单查独携**：无快照的会话（存量会话、从未开轮的空会话）SHALL 返回 `null`，MUST NOT 以空正文伪造快照；
- **清单恒 `null` 且零读**：`agent_sessions` / `list_change_chat_sessions` / `list_archi_chat_sessions` / `list_config_chat_sessions` 与内核 `SessionQuery::list_sessions` 的返回项 SHALL 恒 `preamble = null`，且 MUST NOT 读取快照表（会话清单的全表扫描不因 37KB 级载荷膨胀，页宽 × 快照体量不进外线载荷）；
- **只读**：`preamble` 面 MUST NOT 引入写命令 / 编辑面（写一次与不可变见「会话系统提示词快照的拍存与续轮复用」）；`SessionRow` / `SessionStats` 字段面零变更。

#### Scenario: 单查返回会话行与轮行

- **WHEN** 以已存在（含 running 轮行与已终态两种形态）的 session id invoke `session_detail`
- **THEN** 返回该会话行、聚合统计与轮行清单，running 判定自轮行推导；结果仅来自该 root 所属 workspace 库

#### Scenario: 单查携快照面

- **WHEN** 对一条已拍快照的会话 invoke `session_detail`
- **THEN** `preamble` 非空且含 `savedAt` 与 `content`（`content` 为该会话快照正文全文，与首轮 System 消息逐字相同）

#### Scenario: 清单路径不带快照载荷

- **WHEN** invoke `agent_sessions`（或任一同源会话清单命令）
- **THEN** 返回项 `preamble` 恒为 `null`，且实现未读取快照表（清单载荷与扫描成本零增长）

#### Scenario: 无快照会话返回 null

- **WHEN** 对一条存量（本变更前创建、尚未续轮补拍）会话 invoke `session_detail`
- **THEN** `preamble` 为 `null`；不产生快照行、不现算正文（快照只在会话续轮时拍存）

#### Scenario: 查无此 id 显式失败

- **WHEN** 以不存在的 session id invoke `session_detail`
- **THEN** 前端收到 `Err(String)`，无静默空态、无新会话或记录产生

#### Scenario: blank root 空结果

- **WHEN** 以空白 root invoke `session_detail`
- **THEN** 返回空结果不进入库解析链路，与既有查询命令口径一致

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/preamble.rs` | 系统提示词快照合成（**行替换**：原「起播前读 + 每轮重读」行以本行为准） | `load(root)` 读取 + 32KB char 边界截断 + `<agent_md>` 包装（签名与口径零改动，仅调用方由引擎改为组合根）；`freeze(knowledge, agent_md)` 合成单点（知识 → 空行 → `AGENT.md` 段 → 空行 → 指引常量，空段跳过、全空 → 空串）；`GUIDANCE` 常量（≤ 512 B、零路径字面量、零占位符）；模块可见性保持 `pub(crate)`，零新增第三方依赖 |
| `crates/infra/agent/src/compose.rs` | 快照拍存 / 读回 / 存量补拍单点 | `ComposedTurn::begin`：首轮判据 = `New` ∨ `is_unstarted_session`；首轮读 `ctx.workspace_root` 的 `AGENT.md` → `preamble::freeze` → `kernel.begin_turn` 后经 `self.query.store()` 落库（不扩 `SessionSink`），失败仅日志；续轮 `find_session_preamble` 覆盖 `injections.preamble`（空正文 → `None`）；存量补拍同首轮口径 |
| `crates/infra/agent/src/sdk/loop.rs` | 系统消息一元消费 | 删 `preamble::load` 调用与二元 `system_prompt(knowledge, agent_md)` 形参；`turn.injections.preamble` 非空 → 恰一条 `Message::system`；`AGENT.md` / `preamble::load` 字样零残留（机械可查） |
| `crates/core/agent/src/session.rs` | 出线 DTO 扩面 | `SessionPreambleInfo { saved_at, content }`（`specta::Type`）+ `SessionSummary.preamble: Option<SessionPreambleInfo>`；`SessionRow` / `SessionStats` 零变更 |
| `crates/core/agent/src/runner.rs` | 注入位语义（零结构改动） | `SessionInjections.preamble` 的 doc 由「会话级系统前导」改写为「会话系统提示词快照正文」；六位字段面、`serde` 形态、`PartialEq/Eq` 派生链零变更 |
| `crates/infra/store/src/{model,store,envelope}.rs` | 持久载体与操作面 | 见 `desktop-workspace-store`「会话系统提示词快照记录（写一次冻结）」；本能力只作消费者，MUST NOT 另写第二份模型或写面 |
| `packages/desktop/src/components/agent/agent-session-view.tsx` | 快照只读可见面 | 见 `desktop-agent-chat-infra`「会话系统提示词快照的只读可见面」；本能力只作消费者 |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs`

| 项 | 签名 | 描述 |
|------|------|------|
| `load` | `async fn load(root: &Path) -> Option<String>` | 签名与口径逐字不变（存在 → `<agent_md path="…">` + 截断后内容 + `</agent_md>`；缺席 / 读取失败 → `None`）；调用方由 `loop` 改为 `compose` |
| `truncate` | `fn truncate(text: &str) -> String` | 32KB char 边界截断单点（`load` 唯一消费） |
| `freeze` | `fn freeze(knowledge: Option<&str>, agent_md: Option<&str>) -> String` | 快照正文合成单点：非空段按「知识 → `AGENT.md` 段 → 指引常量」以空行连接；全空返回空串 |
| `GUIDANCE` | `const GUIDANCE: &str` | 冻结指引常量（≤ 512 B、零路径字面量、零占位符） |

### Function: `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`

| 项 | 签名 / 形态 | 描述 |
|------|------|------|
| `ComposedTurn::begin` | 签名零变更 | 签名不变（`session` / `question` / `ctx` / `provenance`），内部增首轮 / 续轮 / 补拍三分支；`ctx.workspace_root` 在移入 `TurnRequest` 前读取 |
| 快照分支 | 内部 | 首轮：`preamble::load` → `preamble::freeze` → 送入 `injections.preamble` → `begin_turn` 后 `store.stamp_session_preamble`；续轮：`store.find_session_preamble` → 覆盖 `injections.preamble`；补拍：续轮分支 miss 时走首轮口径 |

### Component: `packages/desktop/src/types/generated/`

| 项 | 变化 | 描述 |
|------|------|------|
| `bindings.ts` / `bindings.zod.ts` | 再生成 | 增 `SessionPreambleInfo` 类型与 `SessionSummary.preamble`（`nullable`）；非手改，`bindings:export` 产出 |
