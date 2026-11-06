# 提案: agent-event-turn-id

> **变更**: agent-event-turn-id
> **日期**: 2026-11-05
> **状态**: draft
> **来源**: `openspec/changes/agent-event-turn-id/explore.md`（requirement 类型：契约前置 change，服务后续 `agent-stream-single-message` 的「一轮一条 assistant 消息」折叠）
> **性质**: 纯契约增量——给事件信封补一个可空字段，零行为变化、零库结构变化、前端零消费。

---

## 问题

### 事件流里没有任何轮标识

后续 change（D：一轮一条 assistant 消息）要把实时流按轮折叠，前提是**每条事件都能自证属于哪一轮**。当前三处轮相关键没有一个到达线格式：

| 键 | 持有者 | 在事件流里 | 可派生 |
|---|---|---|---|
| `turn_id` | `SessionKernel`（`kernel.rs` `begin_turn` 由 `sink.begin_turn` 分配） | ❌ | ❌ |
| `turn_index` | `sdk/loop.rs` 的引擎私有局部量（`pause_turn` 续跑不递增） | ❌ | ❌ |
| `step_index` | 不存在 | ❌ | ✅ 前端数 assistant Message |

于是 D 的折叠只能靠 `RunStarted` 顺序猜轮：**零事件轮**（`kernel.rs` ask 送达失败臂：`sink.begin_turn` 已分配轮行，但观察流一枚事件都没有）与**停止轮**（`TurnDone` 缺失或时序漂移）会让「第 N 个 `RunStarted` 即第 N 轮」这一推断错位，错位后整段对话的轮归属全偏。

`turn_id` 是唯一现成、由持久层权威分配的轮身份（`SessionEventRecord` 的轮统计行主键同源），但它今天停在 `RunningTurn` 的 `self` 上，从未进入 `AgentEvent`。

### 为什么必须在 D 之前单独做

D 的折叠语义一旦建立，就必须以「事件自带轮 id」为输入。若 D 先用顺序推断落地、之后再补字段，则 D 的 fold 实现、其快照测试与全部 fixture 都要二次改写——先立契约、再实现语义，成本最低。本 change 因此只做一件事：**把 `turn_id` 送到每个事件上**。

---

## 提案

### 1. `AgentEvent` 信封加 `turn_id: Option<i64>`（与 `seq` 平级）

```rust
pub struct AgentEvent {
    /// 所属轮 id（内核 `begin_turn` 分配；`None` 仅出现在无轮归属的构造面）
    #[serde(default)]
    pub turn_id: Option<i64>,
    pub seq: u64,
    pub timestamp_ms: i64,
    #[serde(flatten)]
    pub kind: AgentEventKind,
}
```

- **信封级而非变体内级**：塞进 `RunStarted` 会被三面否掉——停止轮 / 失败轮 / 重订中途可能根本看不到 `RunStarted`；零事件轮连一枚事件都没有；而 `TurnDone` 缺席正是最需要轮归属的场景。塞进 6 个 `kind` 变体则重复 6 次、delta 也要重来一遍（`MessageDelta` 亦有轮归属需求）。
- **`#[serde(default)]` + 不加 `skip_serializing_if`**：沿 `Text.citations` / `WebSearch.{results,error,raw_use,raw_result}` 既有口径——键恒在、值可为 `null`。条件省略会让 bindings 分裂出 `_Serialize` / `_Deserialize` 相位伴生类型，仓库 golden 契约不允（见 desktop-agent-execution「AgentEvent 信封与 loop 可见性」）。
- **必须是 `Option` 而非裸 `i64`**：`AgentEvent` 是「线格式 = 落库形态 = 前端 DTO 基准」三位一体；落库为整条 JSON 原文。旧转录 JSON 缺该键，裸 `i64` 会让**整条事件解码失败**（不是少一个字段）。`Option` 的代价仅是出线多一个可空位。
- **字段位置**：置于 `seq` 之前（沿 explore 取值），JSON 键序随之变为 `turnId` / `seq` / `timestampMs` / `kind`。JSON 对象键序无语义，且全仓不存在「序列化后逐字节比对事件 JSON」的用例（存量行比对走记录层 `RawValue` 原文，与键序无关），故零影响；记于此以免落地时误判为需要保持旧键序。

### 2. 唯一生产点：内核盖戳循环

```rust
// kernel.rs drive()：全事件唯一盖戳点，turn_id 就在 self 上
while let Some(kind) = session.observations.recv().await {
    let event = AgentEvent::stamp(Some(turn_id), seq, kind);
    seq += 1;
    ...
}
```

- 引擎侧**零改动**：`LoopTurn` / `SessionOpen` / 观察通道载荷（`mpsc::Sender<AgentEventKind>`）签名一条不动。轮 id 是持久层分配的治理信息，引擎（`sdk/loop.rs`、`cli/` 已退役）不该知道。
- 命令层 / sink / walker 一律不得补写该字段（`sink.append_sealed` 收到的是已盖戳事件；`agent_session_watch` 投递的是注册表里的同一份事件）。
- 两个实时出口（发起流 `agent_start` 的 `Channel` 与订阅流 `agent_session_watch`）因此自动同源同值——两出口投递的是同一 `AgentEvent` 值。

### 3. `stamp` 签名扩首参（编译期强制不漏写）

```rust
/// 盖当前时钟毫秒的统一生产入口：轮归属 + seq 由事件生产者传入
pub fn stamp(turn_id: Option<i64>, seq: u64, kind: AgentEventKind) -> Self
```

定型为**首参**（而非尾参），与「归属 → 身份 → 载荷」的阅读序一致；已有 `stamp(seq, kind)` 的 65 处测试调用点机械补 `None`（表达「该 fixture 不归属任何轮」，语义真实）。备选「保留两参 + 新增 `stamp_turn`」被否：两条生产入口会让「漏写轮归属」从编译错误退化为运行期静默 `null`，而本 change 的全部价值就是让轮归属不可能被漏写。

### 4. 顺带收益（本期只记账，不实现）

`SessionEventRecord` 载荷是整条 JSON，加了 `turn_id` 之后**转录天然可按轮查询**。今天 `transcript()` 按 `(session_id, seq)` 全量扫；将来要「按轮分页加载」或「跳到某一轮」，无需再改表结构（`event_key` 打包键与模型版本均不动）。

---

## 能力

### 新增的能力

**无。** 本 change 是既有信封契约的字段增量，落在 `desktop-agent-execution`（信封与盖戳治理）与 `desktop-workspace-store`（转录载荷零迁移口径）两条既有能力内；另立能力会让「信封」与「信封字段」分居两条 requirement。

### 修改的能力

- **desktop-agent-execution** — `MODIFIED`「AgentEvent 信封与 loop 可见性」：信封增 `turn_id: Option<i64>`（`#[serde(default)]`、无 `skip_serializing_if`、camelCase 出线 `turnId`）；唯一生产点钉为内核 `drive` 盖戳循环（故两实时出口自动同源同值，既有「两条实时出口载荷一致」用例覆盖）；引擎契约零改动；存量 JSON 缺键读出 `None`；`stamp` 签名扩首参；本字段不参与任何行为（折叠 / 收敛 / 统计）。既有 5 条 scenario 逐字保留，新增 5 条锁定「同轮全事件同值 / 续轮不串轮 / 旧转录缺键 / 键恒在值可 null / 引擎零介入」。Module Contract 新增 5 行（`event.rs` / `kernel.rs` / 两测试文件 / 引擎零改面）。
- **desktop-workspace-store** — `ADDED`「信封新增字段的零迁移口径」：载荷为 JSON 原文（`Box<RawValue>`，不做类型化重建），故加字段 MUST NOT 新增列 / 提升 `native_model` 版本 / 引入迁移；旧行缺键读出 `None`；decode→encode 逐字节不变（`native_db` 的 `remove` 严格字节判等依赖此口径）；按轮查询与索引留待后续，本期 MUST NOT 建。5 条 scenario 把该口径钉成可验断言。Module Contract 三行（`model.rs` 零改动 / `store.rs` 零改动 / `model_test.rs` 回归）。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-agent-chat-infra** | 前端本期**零消费** `turnId`（折叠留给 D）。出线形态为**可选键** `turnId?: number \| null`（`#[serde(default)]` 的既有出线口径，同 `citations?` / `results?`），故 17 个前端测试文件里的 70 余处事件字面量（`timestampMs` 命中 72 处，含 2 处默认参数声明）**无需补字段**、编译与既有断言全绿；适配层 `agent-adapter.ts` / `agent-transport.ts` / `use-agent-session.ts` 一条不改（`data` 部件载荷不带该字段） |
| **desktop-ipc-type-bindings** | 仅重导出生成物（`bindings.ts` + `bindings.zod.ts`），生成管线、`all_commands!` 登记面、导出幂等与入库纪律全部不动；`turnId` 的 zod 形态由 vendored exporter 按既有 `#[serde(default)]` 规则自动产出（`AgentEvent` intersection 变体侧的 `turnId: z.unknown().optional()` 亦自动派生）。字段增量不构成该能力的新契约（先例：`Text.citations` / `WebSearch` 变更未触本能力） |
| **desktop-workspace-store 的查询与对账面** | `list_session_events` / `transcript()` 的 `(session_id, seq)` 扫描语义、轮统计行与聚合统计字段面零变化（`turn_id` 只在载荷内部，不进键、不进统计） |
| **desktop-change-orchestration / desktop-checks-domain / desktop-change-flow-view** | `RunUpdate::SessionEvent` 只是转手 core 类型；walker / worker 的转录消费（`final_assistant_text` / `failure_reason` / `sum_usage_tokens`）只看 `kind`，信封新字段对其透明 |
| **desktop-write-protection / desktop-crate-layout / desktop-workspace-config / desktop-app-shell** | 零配置面 / 零依赖方向变化 / 零新命令 / 零新轨道（`AgentEvent` 未新增 crate 依赖，`specta::Type` derive 沿用） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | `AgentEvent` 增 `turn_id: Option<i64>`（`#[serde(default)]`，无 `skip_serializing_if`，serde camelCase 归 `rename_all` 自动出线 `turnId`）；`AgentEvent::stamp` 签名扩首参 `turn_id: Option<i64>`；结构体与构造器文档补「轮归属为持久层治理信息、引擎不该知道」 |
| `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | `drive` 盖戳循环改为 `AgentEvent::stamp(Some(turn_id), seq, kind)`（`turn_id` 已在 `self` 上，解构处已有绑定）；其余（收敛 / 落库 / 注册表发布）零改动 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | 经 `pnpm run bindings:export` 重导出：`AgentEvent` 增 `turnId?: number \| null`，zod 侧 `get turnId() { return z.number().nullable().optional(); }` 与变体侧 `turnId: z.unknown().optional()`。**MUST NOT 手改** |
| `packages/desktop/package.json` | `version` 0.6.18 → **0.6.19**（一次 patch；`src-tauri/Cargo.toml` 的 crate 版本行**不跟**） |

### 测试文件

机械改动（编译期强制，漏改即编译失败）——`stamp` 调用点 66 处（65 处在测试，按首参补 `None`）+ 结构体字面量 5 处（按字段补 `turn_id: None`）：

| 面 | 文件 | 改动 |
|---|---|---|
| core/agent 夹具 | `event_test.rs`(14) / `lib_test.rs`(4) / `usage_test.rs`(7) / `stream_test.rs`(3) / `state_test.rs`(1) / `port_test.rs`(1) | `stamp(None, …)` |
| core/orchestration 夹具 | `state_test.rs`(2) / `transcript_test.rs`(3) / `walker_test.rs`(5) | `stamp(None, …)` |
| core/orchestration 字面量 | `control_test.rs`（3 处 `RunUpdate::SessionEvent` 字面量） | 补 `turn_id: None` |
| infra/agent 夹具 | `compose_test.rs`(2) / `worker_test.rs`(2) / `store_port_test.rs`(3) / `sdk/{loop_test.rs}(2), resume_test.rs(1), runner_test.rs(6)` | `stamp(None, …)` |
| infra/store 夹具 | `envelope_test.rs`(1) / `store_test.rs`(1) / `model_test.rs`(1 字面量) | `stamp(None, …)` / 补 `turn_id: None` |
| 壳层命令夹具 | `commands/{db,explores,workspaces,exec/agent_test,exec/mod_test,change_flow}/mod_test`：stamp 7 处（`change_flow` 1 + 其余 6）+ `change_flow` 结构体字面量 1 处 | `stamp(None, …)` / 补 `turn_id: None` |

新增用例（覆盖契约面）：

| 位置 | 覆盖点 |
|---|---|
| `crates/core/agent/src/event_test.rs` | ① 信封线格式含 `turnId` 顶层键（`Some` → 数字、`None` → `null`，键恒在，六变体各一枚）；② 旧 JSON（无 `turnId` 键）反序列化读出 `turn_id = None` 且其余字段不变；③ `Some(turn_id)` 往返逐字段相等；④ 全文断言 `skip_serializing_if` 不存在（`None` 形态必留 `"turnId":null`） |
| `crates/core/agent/src/kernel_test.rs` | ⑤ `drive` 流出的事件（delta + 密封）`turn_id` 全等于 `RunningTurn.turn_id`；⑥ `Continue` 续轮（第二轮 `turn_id` 递增）时两轮事件各自同值、互不串轮 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | ⑦ golden 守卫（`agent_event出线为kind判别联合…`）追加 `turnId` 摊平断言与 `turnId?: number \| null` 形态断言；既有 `seq` / `timestampMs` / 六变体字面量断言不变 |
| `crates/infra/store/src/model_test.rs` | ⑧ 旧行（载荷缺 `turnId`）经 `SessionEventRecord::event()` 读出 `turn_id = None`；⑨ 新行落库→回读键在场且值等于写入轮 id；⑩ 旧行 decode→encode 逐字节不变（既有用例形态延展，`native_db` `remove` 口径不破） |

### 不要修改

- **引擎侧**：`crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`（`LoopTurn` / `SessionOpen` / `mpsc::Sender<AgentEventKind>` / `PumpPayload`）——轮 id 不进引擎，签名零改动；`cli/` 已退役，不在讨论面。
- **前端源码**：`src/lib/agent-adapter.ts` / `agent-transport.ts` / `hooks/use-agent-session.ts` / `components/agent/**` / `views/**` 一条不改（`turnId` 零消费）；`src/types/dto.ts` 为 `export type *` shim，自动跟随。
- **前端测试夹具**：17 个 `*.test.ts(x)` 里 70 余处事件字面量（`timestampMs` 命中 72 处）——出线为**可选键**，MUST NOT 为「补字段」逐处改写（改动即越界，且会掩盖「可选键」这一契约事实）。
- **store 落库实现**：`crates/infra/store/src/model.rs`（`SessionEventRecord` 载荷原文存取）与 `store.rs` 查询面零改动；`event_key` 打包键、模型版本（`#[native_model(id = 8, version = 1)]`）零改动。既有转录行 MUST NOT 被「读出 `AgentEvent` → 重新序列化 → 覆盖」的批量重写触碰（会补出显式键、破坏 `remove` 的字节判等）。
- **`crates/infra/store` / `crates/core/workflow` 的 golden 语料**：`tests/golden/*.json` 不含事件信封，零 diff。
- **`openspec/specs/**` 基线**：只在 change 目录出 delta（归档时合入）；`openspec/config.json` 零改动。
- **`openspec/changes/agent-event-turn-id/explore.md`**：只读。
- **`step_index`**：D 已裁定不传，前端按 assistant Message 序数计步；本 change MUST NOT 顺带新增第二计数轴。

### explore 偏差与待决项的落地裁定

| # | explore 原文 | 本提案裁定 |
|---|---|---|
| Q1 | C2「`stamp` 签名扩参」未定首参 / 尾参 | **首参** `stamp(turn_id, seq, kind)`（归属 → 身份 → 载荷的阅读序）；备选「另立 `stamp_turn`」被否（两条入口让漏写从编译错误退化为静默 `null`） |
| Q2 | F3「39 处直接构造 + 66 处 `stamp()`」 | 实盘清点纠正：`AgentEvent` **结构体字面量仅 5 处**（`control_test.rs` ×3、`model_test.rs` ×1、`change_flow/mod_test.rs` ×1）；`stamp()` **66 处**（24 文件，其中 1 处是 `kernel.rs` 生产代码，余 65 处在测试）。explore 的「39 处直接构造」实为 `fn … -> AgentEvent {` 函数返回型声明被 grep 误计（那些不是构造点，签名不变、零改动）。落地按 **5 + 66 = 71 处**机械改动计 |
| Q3 | 「顺带收益：转录天然可按轮查询」 | 只记账，**本期不实现**（不建列、不建索引、不加按轮查询命令）；写入 store 能力的「按轮查询为后续能力」条款，防后人误以为已具备 |
| Q4 | 命名提醒：`TurnDone.session_id` 是引擎侧 id | 采纳为落地纪律：新字段名恒 `turn_id` / `turnId`，MUST NOT 与 `session_id` / `remote_session_id` 混用；`event.rs` 文档显式写明「轮 id ≠ 引擎侧会话 id」 |

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | 信封字段形态：`AgentEvent` 含 `turn_id: Option<i64>`，`#[serde(default)]` 在场且**无** `skip_serializing_if`；线格式为顶层 `turnId` 键（`Some` → 数字、`None` → `null`），`kind` 仍扁平 | `event_test.rs` 六变体线格式断言（`None` 形态下 `"turnId":null` 键在场）+ 源码审查 |
| AC-2 | 唯一生产点：全仓检索 `turn_id` 赋值点，真实运行事件只在 `kernel.rs` `drive` 盖戳循环一处盖 `Some(turn_id)`；引擎侧（`sdk/**`）零 `turn_id` / `turnId` 字样 | 源码审查 + `kernel_test.rs` 断言事件 `turn_id` 等于 `RunningTurn.turn_id` |
| AC-3 | delta 亦带轮归属：一轮内 `MessageDelta` 与密封事件的 `turn_id` 全部同值（同轮恒定），且两个实时出口（发起流 / 订阅流）逐字段一致 | `kernel_test.rs`（delta + 密封混合流断言同值）+ 既有「两条实时出口载荷一致」用例在新字段下仍绿 |
| AC-4 | 续轮不串轮：`Continue{id}` 第二轮事件的 `turn_id` 为新轮 id，与第一轮事件值不同、各自轮内恒定 | `kernel_test.rs` 两轮驱动用例 |
| AC-5 | 旧转录可读：载荷缺 `turnId` 键的存量行经类型化视图读出 `turn_id = None`（整条事件不因缺键解码失败），其余字段逐字段不变 | `event_test.rs`（裸 JSON 缺键）+ `model_test.rs`（旧行 `event()`） |
| AC-6 | 零迁移：`native_model` 版本头不变、`SessionEventRecord` 字段面不变（无新列）、既有 workspace 库文件照常打开；旧行**记录层** decode→encode 逐字节不变（`native_db` `remove` 口径不破——与「`AgentEvent` 层往返必落出显式 `turnId` 键」是两个层级，MUST NOT 混用；新字段只进入新写入行） | `model_test.rs` 既有「旧 schema 转录行解码重编码逐字节不变」用例延展 + `git diff --stat`（`model.rs` 零 diff） |
| AC-7 | 往返保真：`Some(turn_id)` 事件 serde 往返逐字段相等；`u64::MAX` seq / `i64` 边界轮 id 不截断 | `event_test.rs` |
| AC-8 | 出线形态：重导出后 `bindings.ts` 的 `AgentEvent` 含 `turnId?: number \| null`（摊平在 envelope 对象内），`bindings.zod.ts` 含 `get turnId()` 可空可选；`bindings/mod_test.rs` golden 守卫显式断言二者 | `pnpm run bindings:export` + `cargo nextest run -p dev-team bindings` |
| AC-9 | 前端零改动即绿：`vp test` / `tsc` 全绿，17 个前端测试文件零 diff；`agent-adapter.ts` / `agent-transport.ts` / `use-agent-session.ts` / `components/agent/**` 零 diff | `pnpm -C packages/desktop run check` + `git diff --stat` |
| AC-10 | 行为零变化：折叠 / 收敛 / 轮统计 / `transcript()` 扫描语义一条不改；`sum_usage_tokens` / `final_assistant_text` / `failure_reason` 零改动且用例全绿 | 既有 rust 套件全绿 + `git diff --stat` |
| AC-11 | 机械改动完整：71 处（66 `stamp` + 5 字面量）全部收敛，无 `#[allow]` / 无 `..Default::default()` 逃逸；`cargo nextest run --workspace` 全绿 | 编译 + 全量 rust 套件 |
| AC-12 | 门禁与版本：`cargo` 全量测试、`vp check`、`vp test`（覆盖率 lines ≥ 90 / branches ≥ 85 / functions ≥ 90）全绿；`package.json` 0.6.18 → 0.6.19（一次 patch），`src-tauri/Cargo.toml` crate 版本行不跟 | `pnpm -C packages/desktop run check` + 版本行审查 |
| AC-13 | 边界纪律：`crates/infra/agent/src/sdk/**`、`crates/infra/store/src/{model,store}.rs`、`src/hooks/**`、`src/components/agent/**`、`src/views/**`、`src/types/generated/**` 的手写编辑均为零 diff（生成目录改动只来自重导出） | `git diff --stat` 审查 |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | 信封字段加了但某条铸造路径漏盖（如未来新增旁路生产点） | 该路事件 `turnId` 恒 `null`，D 的折叠在该轮失明而**不报错** | 中 | 生产入口唯一（`stamp` 首参强制）+ `kernel_test.rs` 断言「流出事件 `turn_id` 全等于轮 id」+ AC-2 的源码检索判据（引擎侧零字样） |
| R-2 | `stamp` 签名扩参波及 66 处调用点（24 文件），改动面看似很大 | 审阅噪声、误改语义（把 `None` 写成真轮 id 等） | 中 | 改动机械且编译期强制（漏改即编译失败、多改即类型错）；AC-11 明令无 `#[allow]` / 默认值逃逸；测试夹具一律 `None`（语义真实：fixture 不归属任何轮） |
| R-3 | 旧转录兼容被误判为「需要迁移」 | 多出一次数据迁移与版本号 bump，风险与成本远超收益 | 低 | `#[serde(default)]` + 载荷 JSON 原文存储两面已足够；desktop-workspace-store 的 ADDED requirement 与 AC-6 显式禁止建列 / 提版本 / 引迁移 |
| R-4 | 出线形态被误判为「必填键」而连带改写 70 余处前端夹具 | 无收益的大量 diff，且掩盖「可选键」契约 | 中 | AC-8 / AC-9 与「不要修改」条目双向钉住；先例口径 `citations?` / `results?` 已在案 |
| R-5 | `skip_serializing_if` 被后人「顺手优化」加上 | bindings 分裂为相位伴生类型（`_Serialize` / `_Deserialize`），线格式键面塌陷，golden 与 `ipc.ts` 结果面校验双双失真 | 中 | 字段面纪律写进 requirement（MUST NOT）+ AC-1 断言 `None` 形态必落 `"turnId":null`；与 `Text.citations` 同款措辞并在契约里点明先例 |
| R-6 | 命名混用：`TurnDone.session_id`（引擎侧 id）与信封 `turn_id` 被误接 | 消费方按引擎句柄当轮 id 过滤，静默取空集 | 中 | requirement 与 `event.rs` 文档显式写明二者语义不同；本 change 无任何消费点（D 才接），风险窗口最小 |
| R-7 | 覆盖率门禁：新字段若只加不改逻辑，可能拉低分支覆盖 | 门禁红，逼迫补无意义用例 | 低 | 新增断言集中在 `event_test.rs` / `kernel_test.rs` / `model_test.rs`，均为真实契约面（含 `None` 与 `Some` 两支），不写为覆盖率而生的用例 |
| R-8 | 本 change 完成后被误认为「轮折叠已具备」 | 后续实现跳过 D 的折叠设计，直接在前端按 `turnId` 猜测渲染 | 低 | 提案与 store 条款均写明「消费属后续变更（D）」；`AgentDataParts` / 适配层零消费可由 AC-9 的零 diff 机械举证 |
| R-9 | 生成物未随 Rust 改动重导出（漏 `pnpm run bindings:export`） | `ipc.ts` 的结果面与 Channel 信封面用旧 schema（`z.strictObject` 不含 `turnId`）校验新事件，逐事件上报漂移（soft-report：不 throw、UI 不崩，但每枚事件一条噪声，`ipc-drift` 去重键涨满） | 中 | AC-8 以「重导出后断言 `turnId` 在案」为判据；`check` 前置重导出使漏改在本地即被刷新；提交前人工核对 `git status` 中 `src/types/generated/`（既有入库纪律） |
