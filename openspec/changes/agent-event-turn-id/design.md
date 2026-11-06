# 设计: agent-event-turn-id

> **变更**: agent-event-turn-id
> **日期**: 2026-11-05
> **基于**: `proposal.md`（1–4 提案条款、变更范围、AC-1–AC-13、R-1–R-9）、`explore.md`（只读）与两份 delta spec（`desktop-agent-execution` / `desktop-workspace-store`）
> **性质**: 纯契约增量——信封加一个可空字段 + 一个生产入口签名扩参；零行为变化、零库结构变化、零前端消费
>
> **本篇边界**: `crates/core/agent` 两文件（`event.rs` / `kernel.rs`）+ 生成物两份（`bindings.ts` / `bindings.zod.ts`，**只由重导出产生**）+ 版本一行。**零引擎改动**（引擎 = `crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`，与 proposal「不要修改 - 引擎侧」同口径；**`sdk/{runner,loop,resume}_test.rs` 的 9 处 `stamp` 首参补 `None` 属编译闭合批，不是引擎改动**，见 D5 / D11）、**零 store 改动**（`crates/infra/store/src/{model,store}.rs`）、**零前端源码改动**（`src/lib/**` / `src/hooks/**` / `src/components/agent/**` / `src/views/**`）、**零命令面改动**（无新命令、无新 DTO）、**零依赖面改动**（无 `Cargo.toml` / `package.json` 依赖行变化）。

---

## 提案与规格同步状态

proposal 的四条提案与五条 explore 裁定（Q1–Q4）已定稿，两份 delta spec 已定稿。本设计不重述论证，只做五件事：

1. 把 spec 已钉语义、未钉落点的三处收敛到具体形态：`turn_id` 的**属性面与字段位置**（D1 / D2）、**唯一生产点**在 `drive` 循环内的确切语句（D3）、`stamp` 的**参数定序与文档口径**（D4）。
2. 把 proposal「变更范围」的**计数口径按工作树实测纠正**并留痕（D5）：`stamp` 调用点实为 **62 处代码点**（1 生产 + 61 测试）+ **4 处注释提及**（无需改动），结构体字面量 **5 处**；提案记的 66 / 71 把 4 处注释提及计入。落地口径为 **67 处机械改动**。
3. 用**实测证据**钉住「漏重导出即逐事件漂移」这条风险（D7）：`AgentEventSchema` 是 `strictObject(...).and(union(...))`，变体的 `seq` / `timestampMs` 由 exporter 的 `allowed_object_keys` 机制自动补 `z.unknown().optional()`；新字段若不加进 `AgentEvent`，旧产物的六个变体仍是 `strictObject` ⇒ 携带 `turnId` 的每枚事件被 `ipc` 的 Channel 校验判失败（soft-report：不 throw、UI 不崩，但每事件一条噪声）。
4. 逐条 AC 给落点（末尾「验收标准落点」表），不写测试策略（测试由独立相位承接）。
5. 把三处需要**显式留痕**的边界写成可实现形态：`turn_id` 与 `TurnDone.session_id` 的语义分家（D9）、与既有 `TurnSummary.turnId` 的同域同名关系（D9-a）、以及「按轮查询」只记账不实现（D6 / store 侧 spec 已钉）。

### 起点实况核实（工作树实测；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）

- **信封现状**：`crates/core/agent/src/event.rs:206-218` —— `AgentEvent { seq: u64, timestamp_ms: i64, kind }`（`#[serde(rename_all = "camelCase")]`、`kind` 为 `#[serde(flatten)]`）；`impl AgentEvent` 的唯一方法 `stamp`（`:220-229`，`pub fn stamp(seq: u64, kind: AgentEventKind) -> Self`，`timestamp_ms` 取 `now_millis()`）。导出面 `crates/core/agent/src/lib.rs:10-13` 已是 `pub use event::{AgentBlock, AgentCitation, AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole, WebSearchSource}`——本变更**不新增导出名**，故 `lib.rs` 零改动。
- **盖戳点现状**：`crates/core/agent/src/kernel.rs:247-249` —— `while let Some(kind) = session.observations.recv().await { let event = AgentEvent::stamp(seq, kind); seq += 1; ... }`；`drive`（`:207`）开头 `:211-221` 已把 `self` 解构出 `turn_id`（`:213`），故生产点零新增字段、零新增形参。`RunningTurn.turn_id`（`:103`）由 `sink.begin_turn`（`:183-186`）分配。delta 与密封共用该循环（delta 分支 `:250-256` 早 `continue`，密封走 `:257-300`），两实时出口 `streams.publish`（`:253` / `:298`）与 `on_output` 投递**同一枚 `event.clone()`**。
- **`stamp` 调用点实测**：全仓 `AgentEvent::stamp(` 代码点 **62** 处（24 文件）：生产 **1**（`kernel.rs:248`）+ 测试 **61**。另有 **4 处注释 / 模块文档提及**（`core/agent/src/usage_test.rs:7`、`core/orchestration/src/state_test.rs:9`、`core/orchestration/src/walker_test.rs:286`、`:460`）——不是调用点，不改则只影响文档真实性（`grep 'AgentEvent::stamp('` 得 63 行，即 62 代码点 + 1 处注释命中）。逐文件分布见 D5。
- **结构体字面量实测**：`AgentEvent { … }` 字面量 **5** 处，全在测试：`core/orchestration/src/control_test.rs:79` / `:560` / `:868`（`RunUpdate::SessionEvent { event: agent::AgentEvent { seq, timestamp_ms, kind } }`）、`infra/store/src/model_test.rs:22`（`event()` fixture）、`src/commands/change_flow/mod_test.rs:1658`。**生产侧零字面量**（唯一构造面是 `stamp`）。无任何 `AgentEvent { .. }` 解构点（`grep 'let AgentEvent'` 只命中 `AgentEventKind` 模式）。
- **store 现状**：`crates/infra/store/src/model.rs:553-570` —— `#[native_model(id = 8, version = 1, with = SerdeJsonCodec)]` + `SessionEventRecord { event_key: u128, session_id: String, event: Box<RawValue> }`（`:569`）；`new()`（`:575-586`）一次 `serde_json::to_string(&event)` 落原文，`event()`（`:596-600`）现场反序列化。`store.rs` 的 `append_session_events`（`:652-668`，写入前 `is_delta` 防御性跳过）、`next_session_seq`（`:673-698`）、`list_session_events`（`:826`）实现面零涉信封字段。
- **引擎侧现状**：`grep -rn 'turn_id\|turnId' crates/infra/agent/src/sdk/` **零命中**（`LoopTurn` / `SessionOpen` / `PumpPayload` / `mpsc::Sender<AgentEventKind>` 均不含轮标识；**含** `sdk/*_test.rs`——测试文件今日亦无该字样，改动后仍无，因为那 9 处只补 `None` 首参、不写 `turn_id`）——AC-2 的机械判据今天即成立，本变更须让它继续成立。
- **出线现状**：`packages/desktop/src/types/generated/bindings.ts:400-405` —— `export type AgentEvent = { seq: number, timestampMs: number } & AgentEventKind`；`bindings.zod.ts:517-519` —— `AgentEventSchema = z.strictObject({ seq, timestampMs }).and(z.union([...六变体...]))`，六变体各自带 `seq: z.unknown().optional()` / `timestampMs: z.unknown().optional()`（exporter 的 `allowed_object_keys` 机制，`crates/tools/specta-zod-patched/src/primitives.rs:646-668` / `:1548-1560`）。builder 配 `disable_serde_phases()`（`src/bindings/mod.rs:40`）⇒ `#[serde(default)]` 出**可选字段**（先例 `Text.citations` → `citations?: AgentCitation[] | null`）。
- **前端现状**：`src/lib/agent-adapter.ts:95` 用 `Exclude<AgentEvent, { kind: 'messageDelta' }>`（分配条件类型，信封加可选键不受影响）；前端测试里 `timestampMs` 命中 **72 处**（17 个 `*.test.ts(x)`），事件字面量均为对象字面量——出线为可选键 ⇒ 零补字段即编译通过（AC-9 的机制基础）。
- **既有同名键**：`TurnSummary.turnId`（`bindings.ts:1947`，`turnId: number`）是**轮行 / 终态**上的轮 id，与新增 `AgentEvent.turnId` **同值域、不同型、不同承载面**（见 D9-a）。
- **版本现状**：`packages/desktop/package.json:3` `version = "0.6.18"`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml:6` crate `version = "0.1.0"`**不跟**。

### 事实偏差（已在本文档裁定）

1. **`stamp` 调用点计数**：proposal「Q2」与「测试文件」表记 66 处（24 文件，1 生产 + 65 测试），并据此得「5 + 66 = 71 处机械改动」。工作树实测：代码调用点 **62**（1 生产 + 61 测试），另有 **4 处注释提及**被计入 66。落地口径改为 **62 + 5 = 67 处机械改动**；逐文件实测分布见 D5。结论面不受影响（改动依然机械且编译期强制），但**自查锚点数字必须用 62 / 61 / 5 / 67**，否则自查行会误报。
2. **`lib.rs` 零改动（proposal 未列的核对结论）**：`AgentEvent` 已在 `lib.rs:11` 的 `pub use` 名单内，本变更不新增公共类型名 ⇒ `crates/core/agent/src/lib.rs` **零 diff**（登记防误改）。
3. **`bindings/mod_test.rs` 本相位零必改**：既有 golden 断言（`:556` `export type AgentEvent = {`、`:559` `seq: number,`、`:560` `timestampMs: number,`、`:562` `} & AgentEventKind;`）在新字段下**全部仍成立**（新键只增不删），故不需为「可编译 / 可复核」而改；新增 `turnId` 断言归测试相位（proposal 新增用例 ⑦）。
4. **「零引擎改动」的口径收窄（措辞级纠偏，proposal 字面 + 实测共同裁定）**：proposal 的「不要修改 - 引擎侧」是**按四个产品文件**列的——`crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`；而 `crates/infra/agent/src/sdk/` 下另有夹具，实测 `AgentEvent::stamp(` 在 `sdk/` 内共 **9 处**（`runner_test.rs` 6 / `loop_test.rs` 2 / `resume_test.rs` 1），全部**必须**补 `None` 首参（D5 编译闭合批，漏改即 `E0061`）。因此 §本篇边界 / D10 / D11 / 变更清单的 infra/agent 行、以及 `tasks.md` 约束②与阶段三清单**一律不用 `sdk/**` 通配**表述，改为「引擎四文件零 diff + `sdk/{runner,loop,resume}_test.rs` 9 处预期 diff」。二者与 AC-13「零引擎改动」**同义**：`*_test.rs` 不是引擎源码，且该 9 处 diff 不引入任何 `turn_id` / `turnId` 字样（AC-2 的零命中判据在改动后照旧成立，实测今日 `sdk/` 全目录零命中）。登记以免阶段三用 `git diff --stat -- 'crates/infra/agent/src/sdk/**'` 自查时报红误判为越界。**同口径读法**：delta spec「desktop-agent-execution」Module Contract 的 `sdk/**` 行本即按「`LoopTurn` / `SessionOpen` / `PumpPayload` / 观察通道签名与语义逐条不变，`turn_id` / `turnId` 零字样」定义（见 spec 该行说明列），故本纠偏是**与 spec 一致的落地表述**，不构成对 spec 的偏离。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `turn_id` 的属性面 | `AgentEvent` 增 `#[serde(default)] pub turn_id: Option<i64>`，**只标 `default`，MUST NOT 加 `skip_serializing_if`**；`#[serde(rename_all = "camelCase")]` 自动出线 `turnId` | 与 `Text.citations` / `WebSearch.{results,error,raw_use,raw_result}` 同款纪律：`skip_serializing_if` 在 `specta-serde` 的 unified 校验下直接不可表达（会令 bindings 分裂 `_Serialize` / `_Deserialize` 相位伴生类型，仓库 golden 契约不允——`src/bindings/mod_test.rs:637` 显式断言产物不含 `_Deserialize`）。`Option` 而非裸 `i64`：`AgentEvent` 是「线格式 = 落库形态 = 前端 DTO 基准」三位一体，旧转录载荷缺该键，裸 `i64` 会让**整条事件解码失败**（不是少一个字段）。键恒在、值可 `null` 的代价仅是线格式多一个可空位 |
| D2 | 字段在结构体内的位置 | 置于 `seq` **之前**（结构体首字段） | 与「归属 → 身份 → 载荷」的阅读序一致（`turn_id` 先于 `seq`，`kind` 最后扁平）。JSON 键序无语义：`serde_json` 未开 `preserve_order`，`to_value` 走 BTreeMap（按键排序）、`to_string` 走 `FlatMapSerializer`（先非扁平字段、后扁平整形），仓库不存在「序列化后逐字节比对事件 JSON」的用例（存量行比对走 `SessionEventRecord.event: Box<RawValue>` 的原文比较，与键序无关）。据此**不承担**保持旧键序的义务，登记以免落地时误判 |
| D3 | 唯一生产点 | `kernel.rs` `drive` 的观察泵单点：`let event = AgentEvent::stamp(Some(turn_id), seq, kind);`（`turn_id` 取自 `self` 解构绑定，`:213` 已绑定） | 轮 id 是**持久层分配的治理信息**（`sink.begin_turn` 分配，与轮统计行主键同源），引擎不该知道 ⇒ 盖戳只能在持久面在场的内核泵内。两实时出口（`agent_start` 的 `Channel` 与 `agent_session_watch` 的订阅流）投递**同一枚 `AgentEvent` 值**（`streams.publish(&session_id, event.clone())` 与 `on_output(KernelOutput::Observation(event))` 同源），故「同源同值」不是纪律而是构造结果。命令层 / `StoreSink::append_sealed`（收到的是已盖戳事件）/ 注册表 / walker **MUST NOT** 补写该字段 |
| D4 | `stamp` 的参数定序与文档 | `pub fn stamp(turn_id: Option<i64>, seq: u64, kind: AgentEventKind) -> Self`——轮归属为**首参**；备选「保留两参 + 新增 `stamp_turn`」被否 | 两条生产入口会把「漏写轮归属」从编译错误退化为运行期静默 `null`，而本变更的全部价值就是让轮归属不可能被漏写（签名扩参 ⇒ 62 处调用点全部编译期强制收敛，AC-11 的判据）。首参而非尾参：与「归属 → 身份 → 载荷」阅读序一致。文档须写明两条不变量：①「`timestamp_ms` 仍取 `now_millis()`」；②「轮 id ≠ `TurnDone.session_id`（引擎侧标识）」 |
| D5 | 机械改动的实测口径与分布 | `stamp` 调用点 **62 处**补首参（生产 1 处补 `Some(turn_id)`、测试 61 处补 `None`）+ 结构体字面量 **5 处**补 `turn_id: None` = **67 处**。逐文件：（core/agent）`event_test` 14 / `usage_test` 6 / `lib_test` 4 / `stream_test` 3 / `port_test` 1 / `state_test` 1；（core/orchestration）`transcript_test` 3 / `walker_test` 3 / `state_test` 1；（infra/agent）`sdk/runner_test` 6 / `store_port_test` 3 / `compose_test` 2 / `worker_test` 2 / `sdk/loop_test` 2 / `sdk/resume_test` 1；（infra/store）`envelope_test` 1 / `store_test` 1；（壳层）`commands/exec/agent_test` 2 / `commands/{change_flow,db,exec,explores,workspaces}/mod_test` 各 1 | 机械且编译期强制（漏改即 `E0061`）。测试夹具一律 `None` 是**语义真实**：「该 fixture 不归属任何轮」，而不是为了过编译的敷衍值。MUST NOT 用 `#[allow]` / `..Default::default()` 逃逸（`AgentEvent` 无 `Default`，后者亦不成立）；MUST NOT 顺手把某测试夹具改成 `Some(具体轮 id)`（会伪造轮归属，掩盖「唯一生产点」事实——真实轮归属只能出自 `kernel.rs`）。**本行中 `sdk/runner_test` / `sdk/loop_test` / `sdk/resume_test` 三项（共 9 处）均为 `*_test.rs` 夹具**，与「引擎四文件 `sdk/{loop,normalize,runner,resume}.rs` 零 diff」不冲突（D10 / D11） |
| D6 | 旧转录零迁移的口径落地 | `crates/infra/store/src/model.rs` 与 `store.rs` **零改动**；`#[native_model(id = 8, version = 1)]` 版本头、`event_key` 打包键、`new()` / `event()` 体**逐字不动** | 载荷是 `Box<RawValue>` 原文（写入时至多序列化一次、读取时现场反序列化），加字段不触碰列面 ⇒ 无新列、无版本提升、无迁移代码路径。`#[serde(default)]` 使旧行 `event()` 读出 `turn_id = None`。**两层不可混**：记录层 decode → encode 逐字节不变（`native_db` 的 `remove` 严格字节判等依赖此），而 `AgentEvent` 层往返会给 `None` 落出显式 `"turnId":null`（D1 的键恒在纪律）——故 MUST NOT 以「读出 `AgentEvent` → 重新序列化 → 覆盖存量行」的方式改写既有转录原文。「按轮查询 / 按轮索引」**只记账不实现**（不建列、不建索引、不加查询命令），store 侧 spec 已把该边界写成断言 |
| D7 | 生成物必须重导出（漏则逐事件漂移），以及为什么**必然**自动补齐 | 执行 `pnpm run bindings:export` 后：`bindings.ts` 的 `AgentEvent` 对象段增 `turnId?: number \| null,`（排在 `seq` 之前，因 D2）；`bindings.zod.ts` 的 `AgentEventSchema` 基座增 `get turnId() { return z.number().nullable().optional(); },`，且六个变体对象各增一行 `turnId: z.unknown().optional(),` | **机制（源码级 + 实测）**：exporter 的 `allowed_object_keys` 从 intersection 里 `DataType::Struct` 的字段名收集（`specta-zod-patched/src/primitives.rs:646-668`），再对每个变体补齐缺失键为 `z.unknown().optional()`（`:1548-1560`）。因此新字段进 `AgentEvent` 后，变体侧的 `turnId: z.unknown().optional()` **自动派生**（与既有 `seq` / `timestampMs` 同式），无需也不得手改。**漏重导出的后果实测**：旧产物（基座与变体均无 `turnId`）对携带 `turnId` 的载荷 `safeParse` 失败 ⇒ `ipc` 的 Channel 信封校验逐事件 soft-report（`src/lib/ipc.ts` 的 `reportMismatch`（`:94`）经 onmessage forwarder（`:187-189`）逐条调用，永不 throw），`ipc-drift` 去重键涨满；重导出后同载荷通过。这是 R-9 的可复现证据，也是「MUST NOT 手改生成物」的落地理由 |
| D8 | 前端零改动即可绿 | `src/lib/agent-adapter.ts` / `agent-transport.ts` / `hooks/use-agent-session.ts` / `components/agent/**` / `views/**` **一条不改**；17 个前端测试文件里 72 处 `timestampMs` 事件字面量**不补字段** | 出线为**可选键**（`turnId?: number \| null`，`#[serde(default)]` 的既有出线口径，先例 `citations?` / `contextLength?`），TS 对象字面量缺可选属性合法 ⇒ `tsc` 与既有断言全绿；zod 侧 `.optional()` ⇒ 运行期校验也通过。`src/types/dto.ts` 是 `export type *` shim，自动跟随。**为「补字段」逐处改写 70 余处夹具即越界**（R-4），且会掩盖「可选键」这一契约事实 |
| D9 | 命名纪律：`turn_id` vs `session_id` | 新字段名恒 `turn_id` / `turnId`；`event.rs` 的字段 doc 与 `stamp` doc 显式写明「轮 id 是持久层分配的轮身份，与 `TurnDone.session_id`（引擎侧会话标识，双 id 映射半边）语义完全不同，MUST NOT 互相替代或混用命名」 | 两个 id 在代码里长期同屏（`TurnDone { session_id }` 就在信封同一层），混用会让消费方按引擎句柄当轮 id 过滤、静默取空集（R-6）。本变更**零消费点**（前端消费留给后续 change D），风险窗口最小，但文档必须现在写准 |
| D9-a | 与既有 `TurnSummary.turnId` 的关系（显式留痕） | `AgentEvent.turn_id` 与 `TurnSummary.turnId` **同值域**（同一持久层轮 id）、**不同型**（一个是事件信封上的 `Option<i64>`，一个是轮行上的 `i64`）、**不同承载面**（事件流 vs 轮行/终态）。二者同名是**语义正确**而非冲突，MUST NOT 为「避免重名」改成 `eventTurnId` 之类 | 后续 change D 的折叠正是要拿「事件上的 turnId」对齐「轮行上的 turnId」，同名让这条对齐在类型面自然成立。TS 面两者分属不同对象类型，无遮蔽问题 |
| D10 | 引擎零介入的机械判据 | `grep -rn 'turn_id\|turnId' crates/infra/agent/src/sdk/` **MUST 零命中**；`LoopTurn` / `SessionOpen` / `PumpPayload` / `mpsc::Sender<AgentEventKind>` 签名零 diff（即 `sdk/{loop,normalize,runner,resume}.rs` 零 diff）。**判据与编译闭合批不冲突**：`sdk/{runner,loop,resume}_test.rs` 的 9 处 `stamp` 首参补 `None` 既不引入 `turn_id` / `turnId` 字样、也不触碰引擎签名，故「零命中」判据在改动后照旧成立 | 观察通道载荷是 `AgentEventKind`（不含信封），信封在核心泵才成形 ⇒ 引擎**结构上**拿不到轮 id，这条判据是「轮归属是治理信息」的可机械复核面（AC-2 的一半） |
| D11 | 版本与零改动边界 | `packages/desktop/package.json` `version` 0.6.18 → **0.6.19**（一次 patch）；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 **0.1.0 不跟**。零 diff 面（**引擎按文件枚举，不用 `sdk/**` 通配**）：`crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`（引擎四文件，与 proposal「不要修改 - 引擎侧」同口径）、`crates/infra/store/src/{model,store}.rs`、`crates/core/agent/src/lib.rs`、`openspec/specs/**`、`openspec/config.json`、`crates/core/config/config.schema.json`、`crates/**/tests/golden/*.json`（golden 语料不含事件信封）、全部前端 `src/**`（除生成物）。**例外（显式登记，与 AC-13「零引擎改动」同义）**：`crates/infra/agent/src/sdk/{runner_test,loop_test,resume_test}.rs` **必有 9 处 diff**（`stamp` 首参补 `None`：`runner_test` 6 / `loop_test` 2 / `resume_test` 1），属 D5 编译闭合批——它们是 `sdk/` 下的**测试**文件，不是引擎源码；引擎侧（`loop.rs` / `normalize.rs` / `runner.rs` / `resume.rs` 四个产品文件）仍是零 diff | AGENT.md 版本纪律；零 diff 面是「零 DB / 零线格式迁移 / 零引擎 / 零前端消费」的可机械判据（AC-13）。**判据措辞警示**：`git diff --stat -- 'crates/infra/agent/src/sdk/**'` 会命中那 9 处测试适配而报红，**不得当作越界**——正确核对方式是逐文件比对上述四个引擎 `.rs`，或对 `sdk/` 全量 diff 逐条确认「只含 `stamp(` 首参补 `None`」 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 事件信封与盖戳入口（**修改**） | `AgentEvent` 增 `turn_id: Option<i64>`；`stamp` 首参承载轮归属；结构体 / 方法 doc 写明「轮归属为持久层治理信息、引擎不该知道」「轮 id ≠ 会话 id」 | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | `serde` / `serde_json` / `specta`（既有，crate 依赖面零变化） | 既有 serde/specta 属性面（`rename_all = "camelCase"` + `#[serde(default)]`），**零新属性种类** |
| 盖戳治理单点（**修改**） | `RunningTurn::drive` 观察泵内以 `Some(turn_id)` 盖戳；delta 与密封共用该点；收敛 / 落库 / 注册表发布 / ask 失败臂 / 停止收敛路径**全部零改动** | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | `crate::event`、`crate::stream`、`crate::port`（既有） | 既有 async 泵循环（零新任务、零新通道） |
| 出线产物（**重导出**） | `AgentEvent.turnId?: number \| null`（TS）+ `get turnId()`（zod 基座）+ 六变体 `turnId: z.unknown().optional()` | `packages/desktop/src/types/generated/{bindings.ts,bindings.zod.ts}` | `cargo run -p export-bindings`（既有管线） | specta-typescript + vendored `specta-zod-patched`（**零改 exporter**） |
| 转录载荷（**零改动，登记为契约**） | `SessionEventRecord` 载荷 JSON 原文存取；新字段只随新写入行入载荷；版本头 / 打包键 / 查询面不动 | `packages/desktop/src-tauri/crates/infra/store/src/{model.rs,store.rs}` | `native_model` / `native_db` / `serde_json`（既有） | 既有 `SerdeJsonCodec` + `Box<RawValue>` |

<!-- 无新增 crate / 无新增模块目录 / 无新增文件：本变更全部落在既有文件的既有声明与调用点上。 -->

---

## 变更清单

> 路径口径：本表路径为仓库根相对全路径。测试文件与用例归测试相位；本清单只列**实现文件**，另设「编译闭合批」段登记必须随签名一起收敛的夹具点位（它们不是新增用例，只是让工作区可编译）。

### 新增文件

**无。** 本变更是既有信封契约的字段增量与既有一处调用点的参数补齐，不引入任何新文件（新模块 / 新文件会与「零抽象、surgical changes」相悖；轮归属字段天然属于 `event.rs`）。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | ① `AgentEvent` 增首字段 `#[serde(default)] pub turn_id: Option<i64>`（doc：轮归属 = 持久层权威分配的轮 id，一轮内全部事件同值（含 `MessageDelta`）；`None` 仅出现在无轮归属的构造面（测试夹具 / 手工构造）；**与 `TurnDone.session_id`（引擎侧标识）语义完全不同，MUST NOT 互相替代或混用命名**；属性面只标 `default`——`skip_serializing_if` 在 unified 相位模型下不可表达，加则 bindings 分裂出相位伴生类型）；② `AgentEvent::stamp` 签名扩首参 `turn_id: Option<i64>`（doc：唯一盖戳生产入口——「归属 → 身份 → 载荷」；轮归属由事件生产者传入，`timestamp_ms` 仍取 `now_millis()`；MUST NOT 另立第二条生产入口）；③ 结构体头 doc 由「每事件携带 `seq` 与时间戳」改为「携带 `turn_id`（轮归属）、`seq`（单调序号，入库排序键）与时间戳」 | AC-1 / AC-7；`AgentEventKind` / `AgentBlock` / `AgentCitation` / `WebSearchSource` / `AgentDelta` / `AgentMessageRole` / `now_millis` **逐字不动**；`lib.rs` 零改动（`AgentEvent` 已在导出名单） |
| `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` | `drive` 的盖章循环 `:248` 改 `let event = AgentEvent::stamp(Some(turn_id), seq, kind);`（`turn_id` 来自 `:213` 已有的解构绑定，零新增字段 / 零新增形参；可加一行注释「全事件唯一盖戳点：轮归属就在 `self` 上；两实时出口投递同一值」） | AC-2 / AC-3 / AC-4；`begin_turn` / `converge` / `report_session_id` / 状态机应用 / 落库 / 注册表发布 / ask 失败臂 / 停止收敛分支**全部零改动** |
| `packages/desktop/src/types/generated/bindings.ts` | 由 `pnpm run bindings:export` **重导出**（非手改）：`AgentEvent` 对象段在 `seq` 之前增 `turnId?: number \| null,`；`AgentEventKind` 六变体面**零变化**（变体只在 zod 侧补键）；`TurnSummary` / `AgentRunMessage` / 命令签名**零变化** | AC-8；判据 = 重导出后 `git diff` 只含 `AgentEvent` 与 `AgentEventSchema` 相关行 |
| `packages/desktop/src/types/generated/bindings.zod.ts` | 同上由重导出产生：`AgentEventSchema` 基座增 `get turnId() { return z.number().nullable().optional(); },`；六个变体对象各增一行 `turnId: z.unknown().optional(),`（exporter 的 `allowed_object_keys` 自动补齐，与既有 `seq` / `timestampMs` 同式）；`argsSchemas` / `resultSchemas` / `channelArgs` 三张表**零变化** | AC-8 / D7；**MUST NOT 手改**（手改会在下次重导出被覆盖，且破坏幂等）|
| `packages/desktop/package.json` | `version` `0.6.18` → `0.6.19`（一次 patch）；`dependencies` / `devDependencies` **零变化** | AC-12 / D11；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随（零手改），`src-tauri/Cargo.toml` 的 crate 版本行**不跟** |

### 编译闭合批（随签名收敛的夹具点位；MUST NOT 新增用例 / 改断言口径）

> 这 **67 处**不是「测试编写」，而是「让工作区随 `stamp` 新签名与 `AgentEvent` 新字段编译通过」的机械适配（漏改即 `E0061` / `E0063`）。全部机械改动落在测试文件里，故与实现文件同批落地——**中间态不可编译**。测试相位在其上新增契约用例（proposal「新增用例」表 ①–⑩）。

| 面 | 文件（括号内为 `AgentEvent::stamp` 代码点位实测数） | 改动 |
|----|----|------|
| core/agent | `event_test.rs`(14) / `usage_test.rs`(6) / `lib_test.rs`(4) / `stream_test.rs`(3) / `port_test.rs`(1) / `state_test.rs`(1) —— 合计 **29** | 首参补 `None` |
| core/orchestration | `transcript_test.rs`(3) / `walker_test.rs`(3) / `state_test.rs`(1) —— 合计 **7** | 首参补 `None` |
| core/orchestration 字面量 | `control_test.rs:79` / `:560` / `:868` —— **3** | 字面量补 `turn_id: None` |
| infra/agent | `sdk/runner_test.rs`(6) / `store_port_test.rs`(3) / `compose_test.rs`(2) / `worker_test.rs`(2) / `sdk/loop_test.rs`(2) / `sdk/resume_test.rs`(1) —— 合计 **16** | 首参补 `None`。其中 `sdk/` 下 **9 处**（`runner_test` 6 / `loop_test` 2 / `resume_test` 1）是**测试**文件，属本批而非「引擎侧改动」——引擎四文件 `sdk/{loop,normalize,runner,resume}.rs` 仍零 diff（D10 / D11） |
| infra/store | `envelope_test.rs`(1) / `store_test.rs`(1) —— **2**（`stamp`）；`model_test.rs:22` —— **1**（字面量） | 首参补 `None` / 字面量补 `turn_id: None` |
| 壳层命令 | `src/commands/exec/agent_test.rs`(2) / `commands/{change_flow,db,exec,explores,workspaces}/mod_test` 各(1) —— 合计 **7**（`stamp`）；`commands/change_flow/mod_test.rs:1658` —— **1**（字面量） | 首参补 `None` / 字面量补 `turn_id: None` |
| **合计** | `stamp` 调用点 **62**（含生产 `kernel.rs` 1 处）+ 字面量 **5** | **67 处** |

注释 / 模块文档中的 4 处签名提及（`usage_test.rs:7`、`core/orchestration/src/state_test.rs:9`、`walker_test.rs:286` / `:460`）不是调用点，**不改不影响编译**；其中 `usage_test.rs:7` 写有具体参数序（会失真），随动校正；其余三处只提方法名，**逐字保留**（改它们属噪音 diff）。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `AgentEvent::stamp` | `crates/core/agent/src/event.rs` | 修改（扩首参） | `pub fn stamp(turn_id: Option<i64>, seq: u64, kind: AgentEventKind) -> Self` | 唯一盖戳生产入口；返回 `Self { turn_id, seq, timestamp_ms: now_millis(), kind }`。签名扩参使「漏写轮归属」成为编译错误（D4）；全仓 62 处调用点随之收敛（其中生产 1 处传 `Some(turn_id)`、测试 61 处传 `None`） |
| `RunningTurn::drive` | `crates/core/agent/src/kernel.rs` | 修改（体内盖戳调用；**签名零变化**） | `pub async fn drive<F>(self, on_output: F) -> TurnOutcome where F: FnMut(KernelOutput)` | 观察泵内改为 `AgentEvent::stamp(Some(turn_id), seq, kind)`；显式参数面与泛型约束逐字不变，故全仓 11 处 `.drive(` 调用点（`kernel_test.rs` 2 / `compose_test.rs` 7 / `worker.rs` 1 / `commands/exec/mod.rs` 1）**零改动** |

<!-- 无 CLI 子命令 / 无 HTTP 端点 / 无新增 IPC 命令：本变更零命令面演进（信封多一个可空键，命令签名与注册表零变化）。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `AgentEvent`（struct） | `crates/core/agent/src/event.rs` | 修改 | 增 `pub turn_id: Option<i64>`（`#[serde(default)]`，camelCase 出线 `turnId`，键恒在、值可 `null`）。字段序：`turn_id` → `seq` → `timestamp_ms` → `kind`（扁平）。derive 面（`Debug, Clone, PartialEq, Serialize, Deserialize, Type`）零变化 |
| `AgentEvent`（TS 类型别名） | `packages/desktop/src/types/generated/bindings.ts` | 修改（生成物） | `{ turnId?: number \| null, seq: number, timestampMs: number } & AgentEventKind`——仍是「对象 + intersection」，**零** `_Serialize` / `_Deserialize` 伴生类型（D1 的判据） |
| `AgentEventSchema`（zod） | `packages/desktop/src/types/generated/bindings.zod.ts` | 修改（生成物） | 基座 `z.strictObject` 增 `get turnId() { return z.number().nullable().optional(); }`；六变体 `z.strictObject` 各增 `turnId: z.unknown().optional()`；整体仍是 `strictObject(...).and(union(...))` |

<!-- 无新增 interface / type alias / enum：轮归属复用既有 `Option<i64>` 表达，不引入新类型。 -->

<!-- 无配置项：本变更零 `KNOWN_KEYS` 增删、零 `TestFramework` 变体、零 `openspec/config.json` 改动、零 `crates/core/config/config.schema.json` 镜像改动（故省略「配置」子表）。`packages/desktop/package.json` 的 `version` 是版本纪律项，已登记在「修改文件」表。 -->

---

## 数据模型

### 线格式（`AgentEvent`）

| 键 | 类型 | 在场性 | 值域 | 说明 |
|----|------|--------|------|------|
| `turnId` | `number \| null` | **恒在**（`#[serde(default)]` + 无 `skip_serializing_if`） | `Some(轮 id)` → 数字；`None` → `null` | 轮归属；一轮内全部事件（含 `messageDelta`）同值；持久层权威分配（`sink.begin_turn`），与轮统计行主键同源 |
| `seq` | `number` | 恒在（必填） | `u64` 全值域（出线为 `number`，`dangerously_cast_bigints_to_number`） | 会话级续排单调序号（转录主键低 64 位） |
| `timestampMs` | `number` | 恒在（必填） | `i64` | 盖戳时刻（UTC unix 毫秒） |
| `kind` + 变体字段 | 判别联合 | 恒在（必填） | 六变体 | 扁平进顶层（`#[serde(flatten)]`，内部 tag `kind`） |

- **字段关系**：`turn_id` 与 `seq` 平级（信封级），**不是**变体内字段；`TurnDone` 自带的 `session_id` 是引擎侧会话标识，与 `turn_id` 无语义关系（D9）。`turn_id` 不参与任何键、索引、过滤面或统计聚合。
- **不变量**：同一轮内 `turn_id` 恒定；`None` 只出现在无轮归属的构造面（测试夹具 / 手工构造）；真实运行事件恒 `Some(_)`（唯一生产点是 `kernel.rs` 的 `Some(turn_id)`）。

### 持久化（`SessionEventRecord`）

| 项 | 现状 | 本变更 |
|----|------|--------|
| 表 / 模型 | `#[native_model(id = 8, version = 1, with = SerdeJsonCodec)]` + `#[native_db]` | **零改动**（版本头、id、codec 全不动） |
| 主键打包键 | `event_key: u128 = (hash64(session_id) as u128) << 64 \| seq` | **零改动**（新字段不进键） |
| 二级索引 | `session_id` | **零改动** |
| 载荷 | `event: Box<RawValue>`（JSON 原文） | **零改动**：新字段随之进入**新写入**行；旧行缺键读出 `None` |
| 写入 | `new()`：`serde_json::to_string(&event)` 一次 | **零改动**（新字段自然落键，`turnId` 排首位） |
| 读取 | `event()`：`serde_json::from_str` 现场反序列化 | **零改动**（旧行缺键按 `#[serde(default)]` 补 `None`） |
| 查询 | `list_session_events` / `transcript()` 按 `(session_id, seq)` | **零改动**；**不建**按轮索引 / 按轮查询（后续变更） |
| 字节保真 | 记录层 decode → encode 逐字节不变（`native_db::remove` 严格字节判等） | **零改动**且 MUST NOT 被破坏；MUST NOT 对存量行做「读出 `AgentEvent` → 重新序列化 → 覆盖」（会在原文里补出显式 `"turnId":null`，与 D6 的字节口径冲突） |

无关系变化（无新表、无外键、无跨模型引用）。

---

## 出线面 / IPC（无新命令）

本变更不新增 method / path，只让三类既有出线载荷多一个可空键 `turnId`：

| 出线面 | 现状签名 | 载荷形态变化 | auth |
|--------|----------|--------------|------|
| `agent_start` 的 `onEvent: Channel<AgentRunMessage>`（发起流实时出口） | `(onEvent, root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd) -> TurnSummary` | `AgentRunMessage::Event { event: AgentEvent }` 内的 `event` 增 `turnId`（可选键）；`Record { record: TurnSummary }` 零变化 | 无（本地 IPC，进程内） |
| `agent_session_watch` 的 `onEvent: Channel<AgentRunMessage>`（订阅流实时出口） | `(onEvent, root, sessionId, sinceSeq) -> null` | 同上（与发起流投递**同一份 `AgentEvent` 值**，故自动同源同值） | 无 |
| `agent_session_transcript`（转录查询） | `(root, sessionId) -> AgentEvent[]` | 数组元素增 `turnId`（新行有值、旧行 `null`） | 无 |

- **输入面零变化**：三命令的形参名 / 序 / 类型全不动 ⇒ `argsSchemas` / `channelArgs` 三张表零 diff（`channelArgs.agentStart` 现为 `{ onEvent: degraded(...) }`，与信封字段无关）。
- **运行期校验**：`src/lib/ipc.ts` 的 Channel 信封 soft-report 依赖重导出后的 `AgentRunMessageSchema`；漏重导出即逐事件漂移（D7 的实测证据）。
- **zod 形态**：`get turnId() { return z.number().nullable().optional(); }`（基座，判类型）+ `turnId: z.unknown().optional()`（六变体，只为放行不重复判型）——与既有 `seq` / `timestampMs` 完全同式。

---

## 依赖

- **运行时依赖**：**零新增**。`AgentEvent` 只用既有的 `serde` / `serde_json` / `specta`；`kernel.rs` 零新依赖。`crates/core/agent/Cargo.toml` **零 diff**（`Option<i64>` 无需新 crate）。
- **构建 / 测试依赖**：**零新增**。生成物由既有 `pnpm run bindings:export`（`cargo run -p export-bindings` + `tsc --noEmit -p tsconfig.zod.json`）产出；exporter 的 `specta-zod-patched` **零改动**（D7 的补齐机制本已存在，`seq` / `timestampMs` 即先例）。
- **无 feature 开关变化**：`serde_json` 不开 `preserve_order`（D2 的键序结论前提与现状一致）。

---

## 验收标准落点

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 字段形态 | `event.rs` 的 `turn_id` 声明（`#[serde(default)]`、无 `skip_serializing_if`）+ 六变体线格式断言（测试相位，proposal ①④） |
| AC-2 唯一生产点 | `kernel.rs:248` 单点 `Some(turn_id)`；`grep 'turn_id\|turnId' crates/infra/agent/src/sdk/` 零命中（D10） |
| AC-3 delta 亦带轮归属 | delta 与密封共用 `kernel.rs` 同一盖戳点（`:250-256` 与 `:257-300` 同一 `event`） |
| AC-4 续轮不串轮 | `kernel.rs` 每轮 `drive` 各取自己的 `turn_id`（`begin_turn` 每轮新分配，`kernel_test.rs:519` 邻域已断言轮 id 递增） |
| AC-5 旧转录可读 | `#[serde(default)]`（`event.rs`）+ `SessionEventRecord::event()` 现场反序列化（`model.rs:597-599`，零改动） |
| AC-6 零迁移 | `model.rs` / `store.rs` 零 diff；记录层 decode→encode 字节不变（D6） |
| AC-7 往返保真 | `Option<i64>` + `u64` seq 全值域（`event.rs`；`event_test.rs:443` 邻域既有断言） |
| AC-8 出线形态 | 重导出后 `bindings.ts` / `bindings.zod.ts` 三处形态（D7 / 类型定义表） |
| AC-9 前端零改动 | 可选键出线（D8）+ `git diff --stat` 前端零改动 |
| AC-10 行为零变化 | `state.rs` / `usage.rs` / `transcript.rs` / `walker` 转录消费只看 `kind`，全部零 diff |
| AC-11 机械改动完整 | 62 + 5 = 67 处（D5 分布表）；无 `#[allow]` / 无默认值逃逸 |
| AC-12 门禁与版本 | `package.json` 0.6.18 → 0.6.19（D11）；门禁由桌面端执行 |
| AC-13 边界纪律 | 零 diff 面清单（D11，引擎按文件枚举）；**例外已在 D11 显式登记**：`sdk/{runner,loop,resume}_test.rs` 的 9 处 `stamp` 首参补 `None` 属编译闭合批，不计入越界 |

---

## 待决问题

1. **`bindings.zod.ts` 变体侧的 `turnId: z.unknown().optional()` 会削弱变体级的类型判据（已知、接受）**：基座 `get turnId()` 仍按 `number | null | undefined` 判定，而变体上的 `z.unknown().optional()` 只为「放行而不重复判型」——与既有 `seq` / `timestampMs` 完全同式（exporter 的 `allowed_object_keys` 机制，`specta-zod-patched/src/primitives.rs:1548-1560`）。**不修 exporter**：改成 `.omit()` 或精确类型会波及全部经 `flatten` 出线的 intersection 类型（今日仅 `AgentEvent` 一处），属超出本变更范围的渲染器改造。此处登记以备后续若出现「变体级字段需独立判型」的需求时重新评估。
2. **`stamp` 的 `turn_id` 是否应在未来改为非 `Option`（待观察，不在本期）**：全部**生产**调用点（今日 1 处）都传 `Some(_)`，`None` 只服务测试夹具与「无轮归属的构造面」。若后续出现第三种生产点形态（例如脱离内核泵的事件重放补发），需重新评估「是否该把 `stamp` 拆成 `stamp_in_turn(turn_id, …)` 与一个仅供测试的构造器」。本期不做：拆两条入口会在生产面保留一个可传 `None` 的口子，与本变更「漏写即编译错误」的核心价值相悖（D4）。
3. **计数锚点的后续维护**：D5 的 62 / 61 / 5 / 67 是**本变更落地时刻**的实测值。测试相位新增用例（proposal ①–⑩）会新增 `stamp(` 调用点，届时「全仓调用点总数」不再是 62——故自查行必须以「机械改动的收敛」为判据（`grep` 到的每个 `stamp(` 首参都非 `seq` 表达式），而**不**以「恰好 62 处」为判据。proposal 的 66 / 71 与实测 62 / 67 的差额（4 处注释提及）同理只作留痕，不作断言。
