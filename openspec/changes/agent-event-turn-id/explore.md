# AgentEvent 信封补 turn_id（契约前置 change）

workflow_type: requirement

> 本文档是 `unified-agent-stream` 拆分后的第一份，对应 change β-1 `agent-event-turn-id`。
> 第二份见 `openspec/explores/agent-stream-single-message.md`。

## 背景：这份 change 存在的理由

`agent-stream-single-message`（下称 D）要把实时流改成「一轮一条 assistant 消息」，前提是**每条事件都能自证属于哪一轮**。当前事件流里没有任何轮标识：

| 键 | 持有者 | 在事件流里 | 可派生 |
|---|---|---|---|
| `turn_id` | `SessionKernel`（`kernel.rs:183`，`sink.begin_turn` 分配） | ❌ | ❌ |
| `turn_index` | `sdk/loop.rs:193`（引擎私有局部量） | ❌ | — |
| `step_index` | 不存在 | ❌ | ✅ 前端数 assistant Message |

于是 D 的 fold 只能靠 `RunStarted` 顺序猜轮，而零事件轮 / 停止轮会让猜错位（详见 D 文档）。本 change 只做一件事：**把 `turn_id` 送到每个事件上**，为 D 铺路。

## 决定：`AgentEvent` 信封加 `turn_id: Option<i64>`

与 `seq` 平级（信封级），而非塞进 `RunStarted` 或各 `kind` 变体：

```rust
pub struct AgentEvent {
    pub turn_id: Option<i64>,   // 新增，与 seq 平级
    pub seq: u64,
    pub timestamp_ms: i64,
    #[serde(flatten)]
    pub kind: AgentEventKind,
}
```

**生产点只有一处**——`kernel.rs` 的盖戳循环，`turn_id` 就在 `RunningTurn` 的 `self` 上：

```rust
// kernel.rs:247 —— 全事件唯一生产点
while let Some(kind) = session.observations.recv().await {
    let event = AgentEvent { turn_id, seq, timestamp_ms: now_millis(), kind };
```

引擎侧零改动：`LoopTurn` / `SessionOpen` 签名不动，因为 `turn_id` 是持久层分配的治理信息，引擎不该知道。

## 为什么是 `Option` 而不是裸 `i64`

`AgentEvent` 是「线格式 = 落库形态 = 前端 DTO 基准」（`event.rs:206`）。落库为整条 JSON 原文（`model.rs:569` 的 `Box<RawValue>`），因此：

- 加字段**无需 store schema 迁移**（无新列、无版本号）；
- 旧转录的 JSON 缺该键，须能读回 `None` —— 裸 `i64` 会让旧转录反序列化失败。

字段面纪律禁止 `skip_serializing_if`（`event.rs:36`：会令 bindings 分裂为相位伴生类型），故**键恒在、值可为 `null`** —— 与既有 `citations` 字段同形。

## 被否的备选

- **塞进 `RunStarted`**：停止轮、失败轮、重订中途都可能看不到 `RunStarted`；且零事件轮（`kernel.rs:234` ask 失败）根本没有 `RunStarted`。
- **塞进各 `kind` 变体**：重复 7 次，delta 也得带，明显更差。
- **引擎带 `step_index` 一并做**：`turn_index` 在 `pause_turn` 续跑时与步数分岔（续跑不递增 `turn_index`），不能复用；新开计数轴属另一量级，且 D 已决定不传 step。

## 顺带收益

`SessionEventRecord` 载荷是整条 JSON，加了 `turn_id` 之后**转录天然可按轮查询**。现在 `transcript()` 是按 `(session_id, seq)` 全量扫；将来若要"按轮分页加载"或"跳到某一轮"，不需要再改表结构。

## 改动清单

| # | 改动 | 文件 |
|---|---|---|
| C1 | `AgentEvent` 加 `turn_id: Option<i64>`（`#[serde(default)]`，不加 `skip_serializing_if`） | `crates/core/agent/src/event.rs` |
| C2 | `AgentEvent::stamp` 签名扩参（现为唯一生产入口） | `crates/core/agent/src/event.rs` |
| C3 | 盖戳点传入 `turn_id`（`turn_id` 在 `self` 上，零引擎介入） | `crates/core/agent/src/kernel.rs:248` |
| C4 | 契约测试：信封字段、旧 JSON 无键可读、往返保真 | `crates/core/agent/src/event_test.rs` |
| F1 | `pnpm bindings:export` → 出线加 `turnId: number \| null` | 生成物 |
| F2 | 同步 golden 守卫（已锁 `seq` / `timestampMs` / 六变体字面量） | `src-tauri/src/bindings/mod_test.rs:548` |
| F3 | 测试 fixture 补字段 | 39 处直接构造 + 66 处 `stamp()`（其中 65 处在测试） |

## 验证

```
cargo nextest run -p agent        ← C1~C4
pnpm bindings:export && git diff  ← F1 人工核对出线
cargo nextest run -p dev-team     ← F2 golden 守卫
```

## 明确不做

- 任何行为变化：本 change 只加字段，实时流 / 折叠逻辑一行不动。
- 前端消费 `turnId`：留给 D（β-2）。
- `step_index`：D 已决定不传，前端按 assistant Message 序数计步。
- store 迁移：JSON 原文存储，无需迁移。

## 命名提醒

`TurnDone { session_id }` 里那个 `session_id` 是**引擎侧 id**（双 id 映射半边），与新增的信封级 `turn_id` 语义完全不同。落地时须避免再次混用「session」与「turn」。
