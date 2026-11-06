# 测试设计: agent-event-turn-id

> **变更**: agent-event-turn-id
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（变更范围 / 测试文件表 / 新增用例 ①–⑩ / AC-1..AC-13 / R-1..R-9）、`design.md`（D1..D11 / 架构组件 4 行 / 新增文件「无」、修改文件 5 项、编译闭合批 67 处 / 公共函数·API 表 2 行 / 类型定义 3 行 / 数据模型两层 / 出线面 3 项 / AC 落点 / 待决问题 1..3）、两份 delta spec（`desktop-agent-execution` / `desktop-workspace-store`）、`tasks.md`（四阶段与交接清单）及工作树源码与既有测试盘点
> **被测范围**: design「变更清单 - 修改文件」子表 **5 项全列**——4 个代码 / 生成物面（`crates/core/agent/src/event.rs`、`crates/core/agent/src/kernel.rs`、`packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src/types/generated/bindings.zod.ts`）+ `packages/desktop/package.json`（版本行，配置面无测试路径，登记于「不可测试项」）；**新增文件零项、删除文件零项**（design 明示「无新增文件」）。另按 design「数据模型 - 持久化」与 delta spec「desktop-workspace-store」的功能表追加一处**零改动被测面** `crates/infra/store/src/model.rs`（见「测试布局决策」第 3 条，proposal 新增用例 ⑧⑨⑩ 全落在其测试文件 `model_test.rs`）。
> **本变更性质**: 纯契约增量——信封加一个可空字段（`turn_id: Option<i64>`）+ 一个生产入口签名扩首参（`stamp(turn_id, seq, kind)`）+ 一个生产点补 `Some(turn_id)`；**零行为变化、零库结构变化、零前端消费、零引擎改动**。因此本设计的用例全部是**契约面断言**（线格式键面 / 轮归属值 / 字节层次），不写任何「为覆盖率而生」的行为用例（R-7）。

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri`） | `cargo nextest`（`cargo clippy --workspace --all-targets` 为编译闭合批的判据）；async 用例 `#[tokio::test]`，同步用例 `#[test]`；用例名中文长句 | 与被测源文件**同目录**的 `<模块名>_test.rs`，经 crate 根 `lib.rs` 尾部 `#[cfg(test)] mod <模块名>_test;` 挂载（本变更涉及面：`crates/core/agent/src/lib.rs:31-48` 挂 `event_test` / `kernel_test` / `usage_test` / `lib_test` / `port_test` / `stream_test` / `state_test`；`crates/infra/store/src/lib.rs:19-26` 挂 `model_test` / `envelope_test` / `store_test`；`src/bindings/mod.rs:66-67` 挂 `mod_test`） | 断言用 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`；**无 mock 库**：依赖面替身是 crate 内的手写 fake（`FakeSink` / `FakeRunner`，经 `SessionKernel::new(Arc<dyn SessionSink>, ..)` 与 `begin_turn(runner: Arc<dyn AgentRunner>, ..)` **显式注入**，非内部模块替身）；文件名类事实（`AgentEvent` 段位置）用 `String::from_utf8(..)` + `contains` / `type_section` / `field_names` 辅助；跨进程产物互斥用 OS 文件锁 `lock()` |
| 前端（`packages/desktop/src`） | `vite-plus` 的 test 面（vitest API，`vp test`，jsdom）；`vp check` 的 typeCheck | `src/**/*.test.ts(x)` 共置 | **本变更前端零改动**（AC-9 / tasks.md 约束②：`src/**` 除生成物 MUST 零 diff）⇒ 本节不新增前端测试文件、不新增前端用例，17 个含事件字面量的测试文件（`timestampMs` 命中 72 处）只为**回归登记**出现在验收范围与不可测试项中 |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、解码失败、失败收敛路径、被禁止的写法）/ `边界`（空、`None`、极值、键在场性、生命周期交错点）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增用例**（AC 的「既有用例全绿」判据）；未带前缀的为**新增**。
- **参数类型边界口径**：`turn_id: Option<i64>` 取 `None` / `Some(0)` / `Some(-1)` / `Some(i64::MIN)` / `Some(i64::MAX)`（Optional 合并 int 边界，≥ 5 形态）；`seq: u64` 取 `0` / `1` / `u64::MAX`；`AgentEventKind`（6 变体枚举）取全 6 值（非法判别值 `runResult` 的拒绝面已由既有 `旧线值run_result反序列化返回err更名不别名` 用例锁定，本变更不重复）。下文各章节按此铺开，不重复声明。

**待测功能条目口径**

design「公共函数 / API」表只有 **2 行**：`AgentEvent::stamp`（`crates/core/agent/src/event.rs`）与 `RunningTurn::drive`（`crates/core/agent/src/kernel.rs`，签名零变化、体内盖戳调用变化）。两行逐行落在对应源文件的 `#### 待测功能` 中。

其余被测文件的公共 API 面不由 design 的该表声明（`bindings.ts` / `bindings.zod.ts` 是**生成物**，design「类型定义」表标注为修改项；`model.rs` 在 design 中是**零改动**面），故这些章节以 HTML 注释说明「design.md 未声明该文件的公共 API 变更」，并按 archive 先例（`2026-11-02-change-detail-live-stdout`）用**显式标注的一条行为面条目**承接——来源为 design「类型定义」表 /「数据模型」表 / Module Contract / AC 落点，以及 delta spec 的功能表，**不虚构 design 未有的 API 行**。

**覆盖口径与不可写面的前置裁定（贯穿全篇）**

- 新增断言集中在 4 个测试文件：`crates/core/agent/src/event_test.rs`、`crates/core/agent/src/kernel_test.rs`、`crates/infra/store/src/model_test.rs`、`packages/desktop/src-tauri/src/bindings/mod_test.rs`（proposal 新增用例 ①–⑩ 的落点）。
- **AC-11 的 67 处机械改动不是用例**：`stamp` 调用点 62 处（生产 1 + 测试 61）+ 结构体字面量 5 处（见 design D5 分布表）全部落在测试夹具里，属「让工作区随新签名编译通过」的编译闭合批（漏改即 `E0061` / `E0063`）。这些夹具文件**MUST NOT** 被本相位增补断言——尤其 `sdk/{runner,loop,resume}_test.rs` 的 9 处（tasks.md 交接末条：在其上增补「证明轮归属」的断言只会伪造「引擎侧有轮概念」的印象）。
- **前端零 diff 是硬约束**（tasks.md 约束②：`src/**` 除生成物 MUST 零 diff）⇒ 「`turnId` 缺失 / `null` / 数字三形态经 zod `safeParse` 放行」这类运行时断言**不可新增**（`packages/desktop/src/lib/ipc-contract.test.ts` / `ipc.test.ts` 亦在禁令内），只能以既有用例全绿 + design D7 的机制证据侧证。该限制已在「不可测试项」逐条留痕。
- 时钟不 mock：`now_millis()`（`crates/core/agent/src/event.rs:5-10`）无注入点（design 明示「零新时钟」），故与时间戳相关的断言只锁区间 / 非负，不锁具体值。

**测试布局决策（源文件 → 测试文件的映射依据）**

1. **Rust 全部沿用既有同目录 `*_test.rs`，无一处新建测试文件**：`event.rs` / `kernel.rs` / `model.rs` 三个源文件的测试文件全部已在盘上并由 crate 根挂载（清单见上表）。`bindings.ts` / `bindings.zod.ts` 两个**生成物**的文本面锚点在 `packages/desktop/src-tauri/src/bindings/mod_test.rs`（该文件既有 `authoritative_snapshot()` / `zod_authoritative_snapshot()` 双快照装置 + `agent_event出线为kind判别联合且seq摊平变体字面量camel_case` golden 守卫），与 design Module Contract 第 5 行及 tasks.md 交接 ⑨ 的落点一致。
2. **`packages/desktop/package.json` 无测试路径**（版本行是配置面，见「不可测试项」），不虚构测试文件。
3. **`crates/infra/store/src/model.rs` 作为零改动被测面单列章节**：design「修改文件」表不含它（D6 明令零改动），但 proposal 新增用例 ⑧⑨⑩ 与 AC-5 / AC-6 的验证方法都指向 `model_test.rs`，且 delta spec「desktop-workspace-store」把「旧行缺键读出默认值 / 记录层字节不变 / 新行往返保真」钉成可验断言。故按「被测行为面在 `model.rs`、测试文件是 `model_test.rs`」登记章节，并在该章节头以 HTML 注释标明「design 未声明该文件的公共 API 变更」。
4. **不接受「生成物不可测」的推脱**：两份产物的形态（`turnId?: number | null` / `get turnId()` / 六变体 `turnId: z.unknown().optional()`）是 AC-8 的唯一可执行判据，且 `mod_test.rs` 已有 `lock()` + 重导出快照装置 ⇒ 全部可写，无替代判据。
5. **`crates/core/agent/src/event.rs` 的 `AgentEvent.turn_id` 字段**不在 design「公共函数 / API」表（该表只有 `stamp` 一行），而在「类型定义」表与「数据模型 - 线格式」表中 ⇒ 其覆盖归入 `event.rs` 章节的补充覆盖（HTML 注释标注），不另立 API 条目。
6. **组合用例挂靠**：AC-3 的「两个实时出口逐字段一致（含 `turn_id`）」链路发起方是内核 `drive`（`streams.publish` 与 `on_output` 投递同一枚 `event.clone()`，见 design D3），故主判据写 `kernel.rs` 章节；跨层端到端侧证（`crates/infra/agent/src/compose_test.rs:2181` 起的 `change轨真实驱动_同一streams收终态且run_update出口照常`）作为**回归登记行**挂在该章节，不新建组合测试区、不新建章节。
7. **不为测试新增导出**（AGENT.md 规则 5 与 `knip` 判据）：`model.rs` / `event.rs` / `bindings/mod.rs` 的私有辅助与被测 API 一律不动；`bindings.ts` / `bindings.zod.ts` **MUST NOT 手改**（tasks.md 约束③）。

---

## 验收范围

<!-- 测试区域：Rust 侧 `packages/desktop/src-tauri/crates/**/src/*_test.rs` 与 `packages/desktop/src-tauri/src/bindings/mod_test.rs`（与被测源文件同目录，经 crate 根 `#[cfg(test)] mod` 挂载；运行器 `cargo nextest`）。
     本变更**新建测试文件零个**：全部为既有测试文件的用例追加（4 个文件：`crates/core/agent/src/{event_test,kernel_test}.rs`、`crates/infra/store/src/model_test.rs`、`packages/desktop/src-tauri/src/bindings/mod_test.rs`）或既有夹具的机械适配（编译闭合批 67 处，不增断言）。
     前端测试文件零改动（tasks.md 约束②）：17 个含事件字面量的 `*.test.ts(x)` 中的 72 处 `timestampMs` 字面量只作 AC-9 的回归登记，见「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|------------------|----------------|
| AC-1 | 信封字段形态：`AgentEvent` 含 `turn_id: Option<i64>`，`#[serde(default)]` 在场且**无** `skip_serializing_if`；线格式为顶层 `turnId` 键（`Some` → 数字、`None` → `null`），`kind` 仍扁平 | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` → `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs` |
| AC-2 | 唯一生产点：真实运行事件只在 `kernel.rs` `drive` 盖戳循环一处盖 `Some(turn_id)`；引擎侧（`sdk/**`）零 `turn_id` / `turnId` 字样 | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` → `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs`（值面判据）；源码检索判据见「不可测试项」 |
| AC-3 | delta 亦带轮归属：一轮内 `MessageDelta` 与密封事件的 `turn_id` 全部同值；两个实时出口（发起流 / 订阅流）逐字段一致 | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` → `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs`（含 `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs` 的跨层回归登记） |
| AC-4 | 续轮不串轮：`Continue{id}` 第二轮事件的 `turn_id` 为新轮 id，与第一轮不同、各自轮内恒定 | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` → `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs` |
| AC-5 | 旧转录可读：载荷缺 `turnId` 键的存量行经类型化视图读出 `turn_id = None`，其余字段逐字段不变（整条事件不因缺键解码失败） | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` → `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs`；`packages/desktop/src-tauri/crates/infra/store/src/model.rs` → `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs` |
| AC-6 | 零迁移：`native_model` 版本头不变、`SessionEventRecord` 字段面不变（无新列）；旧行**记录层** decode→encode 逐字节不变（`native_db` `remove` 口径不破），新字段只进入新写入行 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` → `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`（`model.rs` / `store.rs` 零 diff 判据见「不可测试项」） |
| AC-7 | 往返保真：`Some(turn_id)` 事件 serde 往返逐字段相等；`u64::MAX` seq / `i64` 边界轮 id 不截断 | `packages/desktop/src-tauri/crates/core/agent/src/event.rs` → `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs` |
| AC-8 | 出线形态：重导出后 `bindings.ts` 的 `AgentEvent` 含 `turnId?: number \| null`（摊平在 envelope 对象内），`bindings.zod.ts` 含 `get turnId()` 可空可选；golden 守卫显式断言二者 | `packages/desktop/src/types/generated/bindings.ts` → `packages/desktop/src-tauri/src/bindings/mod_test.rs`；`packages/desktop/src/types/generated/bindings.zod.ts` → `packages/desktop/src-tauri/src/bindings/mod_test.rs` |
| AC-9 | 前端零改动即绿：`vp test` / `tsc` 全绿，17 个前端测试文件零 diff；适配层与组件、视图零 diff | `packages/desktop/src/lib/agent-adapter.ts` / `agent-transport.ts` / `src/hooks/use-agent-session.ts` / `src/components/agent/**` / `src/views/**` / 17 个含事件字面量的 `*.test.ts(x)`（**零 diff 面**，见「不可测试项」；全仓前端测试文件共 109 个，本变更零新增零改动）；可选键出线形态由 AC-8 的 `mod_test.rs` 断言举证 |
| AC-10 | 行为零变化：折叠 / 收敛 / 轮统计 / `transcript()` 扫描语义一条不改；`sum_usage_tokens` / `final_assistant_text` / `failure_reason` 零改动且用例全绿 | `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` → `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs`（既有驱动面回归）；`event.rs` → `event_test.rs`（既有线格式回归）；`sum_usage_tokens` / `final_assistant_text` / `failure_reason` 的既有用例见「不可测试项」回归登记 |
| AC-11 | 机械改动完整：67 处（62 `stamp` + 5 结构体字面量）全部收敛，无 `#[allow]` / 无 `..Default::default()` 逃逸；`cargo nextest run --workspace` 全绿 | 编译闭合批落地口径（design D5 + tasks.md 阶段三自查行）：`stamp` 调用点落在 **24 个文件**（`kernel.rs` 生产点 1 + 23 个测试夹具）+ 结构体字面量落在 **2 个只含字面量的文件**（`core/orchestration/src/control_test.rs` 3 处、`crates/infra/store/src/model_test.rs` 1 处）= **26 个文件 / 67 处**（清单见「不可测试项」；只补 `None` / `turn_id: None`，不增断言）；运行期判据是本表其余各行新增用例 + 既有套件全绿 |
| AC-12 | 门禁与版本：`cargo` 全量测试、`vp check`、`vp test`（覆盖率 lines ≥ 90 / branches ≥ 85 / functions ≥ 90）全绿；`package.json` 0.6.18 → 0.6.19（一次 patch），`src-tauri/Cargo.toml` crate 版本行不跟 | `packages/desktop/package.json`（**配置面**，见「不可测试项」）；`packages/desktop/src-tauri/Cargo.toml`（版本行零跟，见「不可测试项」） |
| AC-13 | 边界纪律：`crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`、`crates/infra/store/src/{model,store}.rs`、`src/hooks/**`、`src/components/agent/**`、`src/views/**`、`src/types/generated/**`（手写编辑）零 diff | 零 diff 面清单（见「不可测试项」）；`sdk/{runner,loop,resume}_test.rs` 的 9 处属编译闭合批、不计越界（design D11） |

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/agent/src/event.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs`

既有测试文件**追加独立用例组**（模块内注释分节，建议标题 `信封轮归属 turn_id（agent-event-turn-id）`）；既有线格式 / 判别 / 块面用例的断言行零改动（AC-10 的「行为零变化」判据）。装置沿用文件内既有 `variant_events()`（六变体各一枚，`stamp` 生产）与 `millis_now()`（时钟区间断言）；本变更给 `variant_events()` 的六处 `stamp` 补首参、并**新增**一个轮归属夹具（见下）。

规模提醒（tasks.md 交接末条 + design 待决问题 3）：本文件既有 14 处 `AgentEvent::stamp(` 调用点补 `None` 首参属**编译闭合批**（不改断言）；本节的 `turnId` 用例**另增** `stamp(` 调用点 —— 门禁自查 MUST NOT 以「全仓恰 62 处 `stamp(`」为断言。

#### 待测功能

- `AgentEvent::stamp(turn_id: Option<i64>, seq: u64, kind: AgentEventKind) -> Self`: 唯一盖戳生产入口，轮归属为**首参**（「归属 → 身份 → 载荷」阅读序）；`timestamp_ms` 仍取 `now_millis()`（时钟不可注入）；返回 `Self { turn_id, seq, timestamp_ms, kind }`。签名扩参使「漏写轮归属」成为编译错误，MUST NOT 另立第二条生产入口。

<!-- design.md「公共函数 / API」表只有上面 1 行声明本文件；下列补充覆盖条目的来源是 design「类型定义」表与「数据模型 - 线格式」表（不是 grep 源码得来，也不代表新增 API 行）。 -->

- `AgentEvent.turn_id` 字段（design「类型定义」表 + 「数据模型 - 线格式」表声明）: `Option<i64>`，`#[serde(default)]` 在场、**无** `skip_serializing_if`；camelCase 出线键 `turnId`；键恒在（`None` → `null`）；语义 = 持久层权威分配的轮 id（与 `TurnDone.session_id` 无关系）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentEvent::stamp` | 正向 | `stamp(Some(7), 3, Raw{..})` → `event.turn_id == Some(7)`、`event.seq == 3`、`kind` 逐字保留、`timestamp_ms` 落在 `[before, after]` 区间（既有 `stamp以0盖戳产出seq为0且时间戳为当前时钟毫秒` 的同款区间判据 + 首参断言） | 单次 |
| `AgentEvent::stamp` | 正向 | `stamp(None, 0, TurnDone{..})` → `turn_id == None` 而 `seq` / `kind` / `timestamp_ms` 语义与首参扩参前逐字段等价（`None` 不改盖戳口径；夹具「不归属任何轮」是语义真实值，非敷衍默认） | 单次 |
| `AgentEvent::stamp` | 异常（编译期面） | 首参不可省：`stamp` 为**全函数**（无运行期失败输入——`turn_id` 任意 `Option<i64>`、`seq` 任意 `u64`、`kind` 任意变体都合法），其唯一的「异常」形态是**漏传首参**，表现为 `stamp(seq, kind)` 两参调用报 `E0061`。故本条为登记性条目：不写运行期用例，判据见「不可测试项」的编译期判据行；本组全部用例以三参形态书写即为其运行期侧证 | 单次 |
| `AgentEvent::stamp` | 边界 | `turn_id` 五形态 `None` / `Some(0)` / `Some(-1)` / `Some(i64::MIN)` / `Some(i64::MAX)` 各盖戳一枚 → `event.turn_id` 逐字相等（不夹取、不改符号），且 `to_value` → `from_value` 往返后仍相等 | 参数化迭代（5 形态） |
| `AgentEvent::stamp` | 边界 | `seq` 三形态 `0` / `1` / `u64::MAX` × `turn_id = Some(i64::MAX)` → 两字段互不干扰、无截断（既有 `stamp对u64最大seq原样保留不截断且往返无损` 的扩参延展） | 组合迭代（3 × 1） |
| `AgentEvent.turn_id`（线格式） | 正向 | 六变体各一枚以 `stamp(Some(轮id), seq, kind)` 盖戳 → `serde_json::to_value` 后顶层 `value["turnId"] == json!(轮id)`，且 `value["kind"]` 为驼峰判别字符串、变体字段与 `turnId` / `seq` / `timestampMs` **同层**（`kind` 仍扁平，`turnId` 不是变体内键） | 参数化迭代（6 变体） |
| `AgentEvent.turn_id`（线格式） | 边界 | `turn_id = None` 盖戳 → `value.as_object()` 的 `contains_key("turnId")` 为真且 `value["turnId"] == Value::Null`（**键恒在**；反向判据：键缺席即说明 `skip_serializing_if` 被加上，与 `Text.citations` 同款纪律） | 单次 |
| `AgentEvent.turn_id`（线格式） | 边界 | 序列化后顶层键集合 == `{turnId, seq, timestampMs, kind}` ∪ 该变体自有字段（逐变体核对，六枚各一次）——钉住「信封字段面只增 `turnId` 一个键、无相位伴生键」 | 参数化迭代（6 变体） |
| `AgentEvent.turn_id`（线格式） | 异常 | `to_string` 后的文本中 `"turnId"` 片段**恰出现 1 次**（`kind` 扁平 + 信封字段不重复落键；反向判据：出现 2 次说明字段被误置于变体内层与信封双层） | 参数化迭代（6 变体） |
| `AgentEvent.turn_id`（旧 JSON 兼容） | 正向 | 六变体各一枚**裸 JSON 无 `turnId` 键**（存量转录载荷形态）→ `from_value::<AgentEvent>` 为 `Ok`，`turn_id == None`，`seq` / `timestampMs` / `kind` 与变体字段逐字段等于旧值（AC-5 事件面：缺键不使整条事件解码失败） | 参数化迭代（6 变体旧 JSON） |
| `AgentEvent.turn_id`（旧 JSON 兼容） | 边界 | 显式 `"turnId": null` 与缺键两种输入解出**同一形态**（`turn_id == None`），且两者再序列化均落 `"turnId":null`（显式 null 与缺键不可分辨地归一到键恒在形式） | 单次 |
| `AgentEvent.turn_id`（旧 JSON 兼容） | 异常 | `turnId` 非法形态四类——字符串 `"7"` / 浮点 `7.5` / 超 `i64` 上界的大整数 `9223372036854775808` / 数组 `[7]` → `from_value::<AgentEvent>` 为 `Err`（不产半成品事件；与既有「缺 `seq` / 缺 `timestampMs` 返回 err」同款判据） | 参数化迭代（4 形态） |
| `AgentEvent.turn_id`（旧 JSON 兼容） | 异常 | snake_case 键 `{"turn_id": 7, …}` 输入 → **不产出 `turn_id == Some(7)`**（要么 `Err`、要么 `turn_id == None`）：线键**只**认 camelCase `turnId`，`rename_all` 面别无入口（D9 的命名纪律在 serde 面同样只有一种键）。用例写成「非 `Some(7)`」的析取形态，避免把 serde 对未知键的忽略/拒绝策略钉进断言 | 单次 |
| `AgentEvent.turn_id`（往返） | 正向 | `Some(轮id)` × 六变体 → `to_value` → `from_value` 往返 `assert_eq!(roundtrip, event)` 逐字段相等（契约「往返保真」的最短判据；既有无 `turnId` 版本已覆盖，本条钉住新字段参与往返） | 参数化迭代（6 变体） |
| `AgentEvent.turn_id`（往返） | 边界 | `None` × 六变体 → 往返 `assert_eq!(roundtrip, event)`（`None` → `null` → `None` 闭环；反向判据：`None` 若被落成省略键则仍能解出 `None`，故本条必须与上一行「键恒在」合并读才成立） | 参数化迭代（6 变体） |
| `AgentEvent.turn_id`（往返） | 边界 | `turn_id` 极端值 `i64::MIN` / `i64::MAX` 经 `to_string`（非 `to_value`）→ `from_str` → 逐字段相等（JSON 文本面不发生 f64 化截断、不失符号，与 `u64::MAX` seq 既有断言互补） | 参数化迭代（2 值） |
| `AgentEvent.turn_id`（往返） | 异常 | 反向判据（防 `#[serde(skip)]` 式退化）：`turn_id` 不同、其余全同的两枚事件 `assert_ne!`，且 `turn_id` 相同的两枚 `assert_eq!`——钉住 derive 面（`Debug, Clone, PartialEq, Serialize, Deserialize, Type`）对 `turn_id` **敏感**。这是 AC-10 得以用「既有断言全绿」举证行为零变化的前提：`AgentEvent` 的 `PartialEq` 会把新字段纳入比较（若该字段被 skip，既有比较将静默忽略它） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：serde / `serde_json` | 真实 `serde_json::to_value` / `to_string` / `from_value` / `from_str`——线格式是被测契约本体，替身化会让「键恒在」「缺键兼容」两条断言自证 | 全章节 |
| 不 mock：时钟 | `now_millis()` 无注入点（design「零新时钟」）；断言只锁区间（`millis_now()` 前取样 / 后取样夹逼）与非负，不锁具体值 | `stamp` 正向 / 边界行 |
| 不 mock：`AgentEventKind` / `AgentBlock` | 六变体真实构造（`variant_events()` 既有装置），不为测试伪造判别值 | 全章节 |
| 内部夹具（非 mock） | `variant_events()` 的六处 `stamp` 补 `None` 首参属编译闭合批；本组用例另以 `Some(轮id)` 就地构造（不修改既有夹具语义） | 线格式 / 往返行 |

### `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs`

既有测试文件**追加独立用例组**（建议标题 `盖戳治理单点的轮归属（agent-event-turn-id）`）；既有 `.drive(` 调用点的断言零改动。`drive` 签名与泛型约束零变化（design 公共函数表），故全仓 11 处 `.drive(` 调用点（`kernel_test.rs` 2 / `compose_test.rs` 7 / `worker.rs` 1 / `commands/exec/mod.rs` 1）**零改动**。装置沿用既有 `FakeSink`（`begin_turn` 返回自增轮 id、调用序可断言）+ `FakeRunner`（预录观察流）+ `drive_all`（分账 `KernelOutput::Observation` / `TurnFinished`）+ `drain_stream`（订阅端 drain）。

#### 待测功能

- `RunningTurn::drive<F>(self, on_output: F) -> TurnOutcome where F: FnMut(KernelOutput)`: 观察泵内改为 `AgentEvent::stamp(Some(turn_id), seq, kind)`（`turn_id` 取自 `self` 解构绑定，零新增字段 / 零新增形参）；delta 与密封共用该唯一盖戳点，`streams.publish` 与 `on_output` 投递同一枚值。显式参数面与泛型约束逐字不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RunningTurn::drive` | 正向 | 一轮混合流（RunStarted + 两枚 delta + Message + SystemNotice + TurnDone）驱动收口 → 每枚 `KernelOutput::Observation` 的 `turn_id == Some(running.turn_id)`，**无一枚为 `None`**（AC-2 值面 + AC-3 同轮全事件同值；借 `drive的delta与密封同源发布…` 骨架） | 单次 |
| `RunningTurn::drive` | 正向 | 同一条流内按 `kind.is_delta()` 分账 → delta 组与密封组的 `turn_id` 集合**恰为一个单元素集**且等于 `Some(轮id)`（AC-3 的「delta 亦带轮归属」强形态：不做「delta 允许缺省」的豁免） | 单次 |
| `RunningTurn::drive` | 正向 | 两个实时出口同源同值：`streams.subscribe(&session_id, 0)` 的 drain 结果与 `on_output` 的 Observation 逐条相等（既有 `assert_eq!(streamed, observations[1..])` 的 `AgentEvent` 逐字段比较**自动**比较 `turn_id`，故本条为既有断言在新字段下的覆盖登记 + 显式 `turn_id` 相等断言） | 单次 |
| `RunningTurn::drive` | 正向 | `Continue{id}` 续轮：首轮与次轮各驱动一次（复用 `begin_turn_continue不重复建会话行且多轮轮id递增…` + `同会话次轮盖戳自库内最大seq续排…` 骨架）→ 首轮全部事件 `turn_id == Some(first.turn_id)`、次轮全部 == `Some(second.turn_id)`，且 `first.turn_id != second.turn_id`（AC-4 续轮不串轮：两轮各自轮内恒定、跨轮互不污染） | 单次 |
| `RunningTurn::drive` | 异常 | 失败收敛（`FakeSink.append_failures_from = Some(1)`）→ 失败前已投递的 Observation 的 `turn_id` 仍为 `Some(轮id)`（失败路径不把轮归属降级为 `None`），`TurnFinished.turn_id == running.turn_id` | 单次 |
| `RunningTurn::drive` | 异常 | ask 送达失败臂（观察流零事件即失败收敛）→ `observations` 为空（**零事件轮不产生「补盖戳」伪事件**），`outcome.turn_id` 与开轮 id 相等——这是 R-1「漏盖即静默 `null`」的负面对照臂：本变更不为零事件轮造事件 | 单次 |
| `RunningTurn::drive` | 异常 | 空观察流兜底 failed（无停止信号）→ 零 Observation、`TurnFinished.turn_id == running.turn_id`（既有 `drive空观察流无停止信号时兜底failed且不悬挂` 延展；确认兜底不合成事件） | 单次 |
| `RunningTurn::drive` | 边界 | `FakeSink.begin_turn` 返回的轮 id 取 `1` / `i64::MAX` / `-1` 三形态（`FakeSink` 是同文件既有测试装置，其 `next_turn_id: Mutex<i64>` 可在 `begin_turn` 前预置，无需新增公开 API）→ 流出事件 `turn_id` 逐字等于该值（`Option<i64>` 不做符号归一 / 不夹取 / 不与 `seq` 混淆） | 参数化迭代（3 形态） |
| `RunningTurn::drive` | 边界 | 同会话两轮 seq 续排（两轮各 2 枚密封）→ `appends == vec![0,1,2,3]`（既有）且两轮事件的 `turn_id` 集合互异：轮归属与 seq 空间**正交**（新字段不进 `next_seq` / 不改打包键） | 单次 |
| `RunningTurn::drive` | 边界 | 观察洪峰 64 枚 delta + 2 枚密封（有界通道背压）→ 全部 66 枚 `turn_id` 均为 `Some(轮id)`、`seq` 仍 `0..66` 单调无跳号（背压不丢 `Option` 值） | 单次 |
| `RunningTurn::drive` | 边界 | 停止置位收敛（`request_stop` 已置位 + 观察流关闭）→ 置位前流出的密封事件 `turn_id == Some(轮id)`，收敛路径**零新增事件**（stopped 不合成事件、不改既有事件字段） | 单次 |
| `RunningTurn::drive` | 正向（既有回归） | 既有九组驱动面用例全绿：`begin_turn_new同步段sink调用序…` / `begin_turn_continue…轮id递增` / `drive增量只上输出回调…` / `drive密封事件write_through恰一次…` / `drive_turn_done统计收口…` / `drive_append失败…` / `drive停止置位…` / `drive观察洪峰…` / `drive同会话次轮_以首轮最大seq为since_seq…`（`AgentEvent` 的 `PartialEq` 比较自动纳入 `turn_id`，故除本组新用例外 AC-10 的「行为零变化」由此组全绿举证） | 参数化迭代（既有站点全集） |
| `RunningTurn::drive`（组合，跨层回归） | 组合 | `crates/infra/agent/src/compose_test.rs` 的 `change轨真实驱动_同一streams收终态且run_update出口照常` 在新字段下全绿：订阅端 `SessionStreamItem::Event` 的 `turn_id` 与 `RunUpdate::SessionEvent` 携带事件的 `turn_id` 相等（两实时出口跨层同源同值；AC-3 的端到端侧证。用例挂在链路发起方内核章节，**执行文件是 `compose_test.rs`**） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `SessionSink` 端口替身 | 既有手写 `FakeSink`（crate 内 test 模块），经 `SessionKernel::new(Arc<dyn SessionSink>, ..)` **显式入参注入**——它同时承担「轮 id 分配器」角色（`begin_turn` 自增 / 可编程返回极值），这正是被测契约的一部分（轮 id 来自持久层）；把它当内部模块替身会误解，它是被测 API 的显式依赖 | 全章节（轮 id 三形态行尤需） |
| `AgentRunner` 端口替身 | 既有手写 `FakeRunner`（预录观察流 + 可编程 `open` Err），经 `begin_turn(runner: Arc<dyn AgentRunner>, ..)` 显式入参注入；**不** mock 引擎内部（`LoopTurn` / `SessionOpen` / 观察通道签名零改动本身即 AC-2 的结构保证） | 全章节 |
| `SessionStreamRegistry` | 真实实例（`Arc::new(SessionStreamRegistry::new())`）——两出口同源同值的判据依赖真实注册表投递同一枚值；替身会让「同值」自证 | 两出口同源行、停止置位行 |
| 不 mock：时钟 / seq 分配 | `now_millis()` 与 `seq_base` 均真实；断言只锁「`turn_id` 与 `seq` 正交」「seq 单调」，不锁具体毫秒 | 全章节 |
| 不 mock：`Converge` 路径 | 收敛分支（`converge` / `TurnStats` / 落库调用序）真实执行——AC-10「行为零变化」的判据 | 失败收敛 / 空流兜底 / 停止置位行 |

### `packages/desktop/src-tauri/crates/infra/store/src/model.rs` -> `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`

既有测试文件**追加独立用例组**（建议标题 `信封新增字段的零迁移口径（agent-event-turn-id）`）；既有 `旧schema转录行解码重编码逐字节不变且可被native_db删除` 与三个嵌装往返用例（`嵌装往返密封五变体各构造一条逐字段相等` / `嵌装往返raw逃生舱…` / `嵌装往返大seq…`）的**断言行保留**（本组只做延展，不改既有断言口径）。`model.rs` 与 `store.rs` **源码零改动**（design D6）——本节的用例是「零改动仍然成立」的回归判据，不是新行为的新增覆盖。装置沿用既有 `roundtrip()`（`native_model::encode` → `decode` 并断言版本 1）、`event()` / `message()` / `run_started()` 等夹具（本变更只给 `event()` 字面量补 `turn_id: None`）、`native_db` 内存库全链装置。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明本文件的公共 API 变更（design「修改文件」表不含本文件，D6 明令 model.rs 零改动）。设计中的对应声明在 delta spec「desktop-workspace-store」的 Function 表与 design「数据模型 - 持久化」表；下列三条按 archive 先例显式标注为**行为面承接条目**（不是 design 未有的新 API）。 -->

- `SessionEventRecord::new(session_id: &str, event: AgentEvent) -> Self`（delta spec 功能表；本变更零改动、行为面需回归）: 载荷一次序列化为 `Box<RawValue>` 原文——新字段随之自然落键（`turnId` 排首位），`event_key` 打包自 `hash64(session_id)` + `event.seq` 且**新字段不进键**。
- `SessionEventRecord::event(&self) -> Result<AgentEvent, serde_json::Error>`（同上）: 载荷原文现场反序列化——旧行缺 `turnId` 按 `#[serde(default)]` 补 `None`，整条事件解码不失败。
- 记录层字节面（`native_model::encode` / `decode` + `native_db` `remove` 的严格字节判等，design「数据模型 - 持久化」表「字节保真」行）: 旧行 decode → encode 逐字节不变（不经 `AgentEvent` 类型化重建；缺键 MUST NOT 被补成显式 `"turnId":null`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionEventRecord::new` | 正向 | 新行 `new("ses-turn", AgentEvent::stamp(Some(7), 5, Message{..}))` → 载荷原文（`serde_json::from_str::<Value>` 复核）含 `turnId == 7`，`event_key == pack_session_event_key("ses-turn", 5)`（新字段不进键），`seq() == 5`、`session_id()` 逐字相等 | 单次 |
| `SessionEventRecord::new` | 正向 | 新行经 `roundtrip()`（native_model encode→decode，版本恒 1）→ 记录相等、`event().turn_id == Some(7)`、整条事件逐字段等于写入值（AC-6 新行往返保真） | 单次 |
| `SessionEventRecord::new` | 异常 | 记录层**零业务判别**：六变体（含永不由 `append_session_events` 写入的 `MessageDelta`）各以 `Some(轮id)` 直构记录 → 载荷原文可回读且 `turn_id` 逐字相等（`new()` 不看 `kind`）。附注：`new()` 的两处 `expect`（自产事件序列化 / `RawValue::from_string`）为不可达臂，见「不可测试项」的对应行 | 参数化迭代（6 变体） |
| `SessionEventRecord::new` | 边界 | `turn_id` 四形态 `Some(0)` / `Some(-1)` / `Some(i64::MIN)` / `Some(i64::MAX)` 落库 → `event().turn_id` 逐字相等（JSON 数字不截断、不改符号） | 参数化迭代（4 形态） |
| `SessionEventRecord::new` | 边界 | `turn_id = None` 落库 → 载荷原文**仍含** `"turnId":null`（D1 的「键恒在」纪律在落库原文层同样成立，不因存储面而省略键）；`serde_json::from_str::<serde_json::Value>` 复核该键存在且为 `Value::Null` | 单次 |
| `SessionEventRecord::event` | 正向 | 旧行（裸 JSON 六变体均无 `turnId` 键）→ `event()` 为 `Ok`，`turn_id == None`，`seq` / `timestampMs` / `kind` / 变体字段逐字段等于旧值（AC-5 存储面：缺键不使整条事件解码失败） | 参数化迭代（6 变体旧 JSON） |
| `SessionEventRecord::event` | 异常 | 旧行原文含**非法** `turnId`（`"turnId":"x"` / `7.5` / `[7]` 三形态）→ `event()` 返回 `Err`（类型错误仍报错；`#[serde(default)]` 只放行**缺键**，不吞类型错——防「旧行兼容」被误做成宽松） | 参数化迭代（3 形态） |
| `SessionEventRecord::event` | 边界 | 载荷原文为**非对象合法 JSON**（`RawValue::from_string("null")` / `"[]"` / `"42"`——`RawValue` 构造期只校验 JSON 合法性，故三种原文可构造）→ `event()` 返回 `Err`：新字段**不改变既有错误面**（既无新增失败路径，也无静默兜底） | 参数化迭代（3 形态） |
| 记录层字节面 | 正向 | 旧行 `native_model::encode` → `decode` → 再 `encode` 逐字节相等，且重编码文本**不含** `turnId`（缺键未被补成显式默认值；既有 `旧schema转录行解码重编码逐字节不变…` 用例的延展，原文一律不含 `turnId`） | 单次 |
| 记录层字节面 | 异常 | 两层不可混的反向判据：同一旧行经 `event()` → `serde_json::to_string` 后与载荷原文**不相等**（`None` 落出显式 `"turnId":null`）——钉住「MUST NOT 读出 `AgentEvent` → 重新序列化 → 覆盖存量行」（D6） | 单次 |
| 记录层字节面 | 异常 | `native_db` 真库全链：insert 旧行 → scan（类型化解码）→ `remove` 严格字节判等成功（既有 `旧schema转录行…且可被native_db删除` 延展；新字段不参与键、不破字节判等） | 单次 |
| 记录层字节面 | 边界 | 模型元数据面：`native_model_id() == 8`、`native_model_version() == 1`（既有断言）+ `SessionEventRecord` 字段名集合 == `{event_key, session_id, event}`（钉住「无新列」；AC-6 的字段面判据） | 单次 |
| 记录层字节面 | 边界 | 新行与旧行**混存**同一会话（旧行无 `turnId`、新行有）：`list_session_events` 按 `(session_id, seq)` 升序返回两行、新旧行各自 `turn_id` 分别为 `None` / `Some(轮id)`（两层形态共存于同一转录，无迁移即成立；新字段不进键故排序不受影响） | 单次 |
| 查询面零变化（`store.rs` 契约） | 正向（既有回归） | 既有 `list_session_events` 三例（`多会话交错落库索引圈定完整升序互不串号` / `空洞容忍升序不补洞` / `跨session隔离且不存在会话空vec`，**执行文件是 `store_test.rs`**）+ 本节 `嵌装往返` 三例在新字段下全绿（扫描仍按 `(session_id, seq)`，新字段只随载荷透出；AC-10 的存储面判据。`store.rs` 源码零 diff ⇒ 该行只作回归登记，不新增断言） | 参数化迭代（既有站点全集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：`native_model` / `serde_json` 编解码 | 真实 `native_model::encode` / `decode` 与真实 `serde_json`——「字节不变」与「缺键补默认值」是两个层级的同一机制，替身化即失去判据 | 全章节 |
| 不 mock：`native_db` | 真库（`native_db::Models` + `create_in_memory`）承载 insert / scan / `remove` 全链——`remove` 的严格字节判等是 AC-6 的唯一可执行判据 | 全链行 |
| 不 mock：`foundation::layout` 与 `AgentEvent` 构造 | 事件由真实 `AgentEvent::stamp` / 结构体字面量（补 `turn_id: None`）构造；`RawValue::from_string` 造旧行原文（`model_test.rs` 是同 crate 子模块，可直接构造记录，无需新增导出） | 旧行 / 新行全章节 |
| 不 mock：文件系统 | 本组以内存库与内存编解码为主；无需 tempdir（既有 `Store` 类用例的 tempdir 装置保持原样） | 全章节 |

### `packages/desktop/src/types/generated/bindings.ts` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

生成物**不是手写实现**，其文本面锚点由 `src-tauri/src/bindings/mod_test.rs` 承担（design Module Contract 第 5 行 / tasks.md 交接 ⑨）。用例追加在既有 `agent_event出线为kind判别联合且seq摊平变体字面量camel_case` 内（该用例已持 `lock()` 与 `authoritative_snapshot()` 重导出装置），既有 `seq` / `timestampMs` / `} & AgentEventKind;` / 六变体字面量断言**逐字保留**（design「事实偏差 3」：新键只增不删，既有断言零改动即成立）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明生成物的 API 变更（`bindings.ts` 在 design「修改文件」表与「类型定义」表中标注为**生成物重导出**，MUST NOT 手改）。下列一条为行为面承接条目，来源是 design「类型定义」表第 2 行 +「出线面 / IPC」表。 -->

- `AgentEvent`（TS 类型别名，生成物）: `{ turnId?: number | null, seq: number, timestampMs: number } & AgentEventKind`——`turnId` 摊平在 envelope 对象段内、排在 `seq` 之前（D2 / D7）；仍为「对象 + intersection」，**零** `_Serialize` / `_Deserialize` 相位伴生类型（D1 的判据）。本相位不新增 / 不修改任何导出。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentEvent`（生成物文本） | 正向 | 重导出后 `type_section(&content, "AgentEvent")` 段内含 `turnId?: number \| null,`，且段内位置满足 `find("turnId") < find("seq") < find("timestampMs")`（D2 字段序 + D7 出线形态；既有 `field_names` 辅助按 `split(':')` 取名会把 `turnId?` 整行跳过，故改用 `find` 位置比较——不新增辅助、不改既有辅助） | 单次 |
| `AgentEvent`（生成物文本） | 正向（既有回归） | 既有 `agent_event出线为kind判别联合且seq摊平变体字面量camel_case` 的全组断言零改动即成立：`export type AgentEvent = {` / `} & AgentEventKind;` / `seq: number,` / `timestampMs: number,` / 六变体字面量 `kind: "runStarted"` … `kind: "raw"` / `turnDone` 五个 camelCase 字段行逐字在场（本变更只增新键，不删旧键） | 参数化迭代（既有字面量全集） |
| `AgentEvent`（生成物文本） | 异常 | 产物全文**不含** `_Deserialize` / `_Serialize`（`skip_serializing_if` 被「顺手优化」加上的机械判据：加则相位分裂，golden 契约破） | 单次 |
| `AgentEvent`（生成物文本） | 异常 | 产物全文不含必填形态 `turnId: number,`、不含 `eventTurnId`（D9-a 明令禁止的重命名）——只允许 `turnId?: number \| null` 一种形态 | 单次 |
| `AgentEvent`（生成物文本） | 边界 | `TurnSummary.turnId: number`（轮行面，既有）与 `AgentEvent.turnId?: number \| null`（信封面）**并存**：两条声明各自在场且类型不同（D9-a 的同名不同型事实被机械钉住，防后人「为避免重名」改名） | 单次 |
| `AgentEvent`（生成物文本） | 边界 | 命令面相依类型零漂移：`export type AgentRunMessage =` + `{ ipc: "event"; event: AgentEvent }` + `{ ipc: "record"; record: TurnSummary }` 逐字在场（信封多一键不改命令签名） | 单次 |
| `AgentEvent`（生成物文本） | 边界 | 连续两次 `authoritative_snapshot()`（各自内部重导出）逐字节零 diff（幂等；新键不引入不确定性输出） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：导出管线 | 真实 `export_bindings()`（`authoritative_snapshot()` 内部先重导出再读盘）——产物文本是被测对象本体 | 全章节 |
| 不 mock：文件系统 | 真实读 `src/types/generated/bindings.ts`（`generated_path()` 由 `CARGO_MANIFEST_DIR` 同式定位），不注入虚拟 fs | 全章节 |
| 跨进程互斥（非 mock） | 既有 OS 文件锁 `lock()`（`bindings-test.lock`）串行化产物改写类用例；nextest 每用例独立进程，进程内 `Mutex` 无效——这是既有装置，不新增 | 全章节 |

### `packages/desktop/src/types/generated/bindings.zod.ts` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

同一测试文件、另一产物；装置为既有 `zod_authoritative_snapshot()` + `zod_generated_path()`。六变体的 `turnId: z.unknown().optional()` 由 exporter 的 `allowed_object_keys` 机制**自动补齐**（design D7，先例 `seq` / `timestampMs`）；该机制的单元面已由 `packages/desktop/src-tauri/src/bindings/zod_test.rs` 的 fixture 类型用例锁定（该文件本变更**零改动、不新增用例**），本节断言是同一机制的**真实产物侧**证据。用例追加在**新增**独立用例（建议名 `agent_event_zod基座判型且六变体放行turn_id…`）内，或并入既有 `双产物同目录同时存在且zod产物含三表与降级哨兵`；既有三表 / 5 处 `degraded(` / 禁用片段 / 幂等 / 篡改恢复断言**逐字保留**。**MUST NOT 手改产物**（tasks.md 约束③）。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明生成物的 API 变更（`bindings.zod.ts` 在 design「修改文件」表与「类型定义」表第 3 行标注为**生成物重导出**，且「出线面 / IPC」表给定了 zod 形态）。下列一条为行为面承接条目。 -->

- `AgentEventSchema`（zod 生成物）: 基座 `z.strictObject` 增 `get turnId() { return z.number().nullable().optional(); }`（判类型），六变体 `z.strictObject` 各增 `turnId: z.unknown().optional()`（只为放行不重复判型，exporter 的 `allowed_object_keys` 自动补齐）；整体仍为 `strictObject(...).and(union(...))`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentEventSchema`（生成物文本） | 正向 | 产物含 `get turnId() { return z.number().nullable().optional(); },`，且该行位于 `AgentEventSchema` 的基座段（`export const AgentEventSchema = z.strictObject({` 之后、`.and(z.union([` 之前） | 单次 |
| `AgentEventSchema`（生成物文本） | 正向 | 六个变体 `z.strictObject` 各含 `turnId: z.unknown().optional(),`（与既有 `seq: z.unknown().optional()` / `timestampMs: z.unknown().optional()` 同式）；产物内该片段出现次数恰为 **6**（逐变体一次，不多不少） | 参数化迭代（6 变体） |
| `AgentEventSchema`（生成物文本） | 正向（既有回归） | `AgentEventSchema = z.strictObject({` … `}).and(z.union([` 的 intersection 形态、三张表 `export const {argsSchemas,resultSchemas,channelArgs} = {`、`degraded(` 恰 5 处全部保持（既有断言零改动即成立） | 参数化迭代（既有断言全集） |
| `AgentEventSchema`（生成物文本） | 异常 | 禁用片段零命中（`export type ` / `z.infer` / `z.object(` / `.refine(` / `.default(`）——新键未引入禁用写法（`.optional()` / `.nullable()` 不在禁用表内，`citations` 先例同款） | 参数化迭代（5 片段） |
| `AgentEventSchema`（生成物文本） | 异常 | 产物**不含** `turnId: z.number()`（必填形态）与 `AgentEventSchema_S*` 相位伴生名（手改 / 相位分裂的两种退化形态） | 单次 |
| `AgentEventSchema`（生成物文本） | 边界 | 三张表的键集与 `commands` 完全一致（既有断言）+ 逐表条目数不变：`turnId` 只进 `AgentEventSchema` 与其六变体，不进 `argsSchemas` / `resultSchemas` / `channelArgs`（新字段不触命令面） | 单次 |
| `AgentEventSchema`（生成物文本） | 边界 | 连续两次 `zod_authoritative_snapshot()` 逐字节零 diff（幂等） | 单次 |
| `AgentEventSchema`（生成物文本） | 边界 | 手工篡改 zod 产物（删表 / 注入 `export type`）后重导出恢复权威内容——恢复后的内容仍含 `get turnId()` 行，证明该行是**产物**而非手写残留（既有用例延展） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：导出管线 | 真实 `export_bindings()`（`zod_authoritative_snapshot()` 内部重导出 + 落盘自检链路） | 全章节 |
| 不 mock：文件系统 | 真实读 `src/types/generated/bindings.zod.ts`（`zod_generated_path()`） | 全章节 |
| 不 mock：zod 库 | 不做运行期 `safeParse`（前端零 diff 约束下不可新增前端用例，见「不可测试项」）；本文件断言**只读文本**，不实例化 schema——故无需 zod 替身，也不引入 `zod` dev-dependency | 全章节 |
| 篡改 / 删除产物 | 既有 `RestoreOnDrop` / `RestoreStubOnDrop` Drop 守卫（断言失败 unwind 时尽力重导出恢复），非 mock | 幂等 / 篡改恢复两行 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` 0.6.18 → 0.6.19） | 配置 / 元数据，无运行期行为可断言；版本纪律的判据是「一次 patch、`src-tauri/Cargo.toml` crate 版本行不跟」 | AC-12：diff 审查（`package.json` 单行变更 + `src-tauri/Cargo.toml:6` 的 `version = "0.1.0"` 未动 + `tauri.conf.json` 的 `"version": "../package.json"` 引用行零改） |
| `packages/desktop/src-tauri/Cargo.toml` | 零 diff 面（D11）——「正确性主张」就是版本行一行不变；在测试里读 `Cargo.toml` 会把用例绑死在路径布局上 | diff 审查 + `grep -n '^version' src-tauri/Cargo.toml` 仍为 `0.1.0` |
| AC-2 / AC-13 的**源码检索判据**（`grep -rn "turn_id\|turnId" crates/infra/agent/src/sdk/` 零命中；引擎四文件 `sdk/{loop,normalize,runner,resume}.rs` 零 diff；`sdk/**` 下仅 9 处 `stamp` 首参补 `None`） | 这是 grep / diff 类静态核对；把引擎源文件读进测试扫描，会与桌面端静态检查重复且随路径布局漂移、脆 | AC-2 / AC-13 / tasks.md 阶段一末条与阶段三自查行（桌面端静态检查执行）；运行期侧证是 `kernel_test.rs` 的「流出事件 `turn_id` 全等于 `RunningTurn.turn_id`」与本表 AC-11 判据 |
| AC-1 / AC-11 的「`skip_serializing_if` 不存在」与「无 `#[allow]` / 无 `..Default::default()` 逃逸」 | 属性面 / 记法面事实，运行期不可观测（`skip_serializing_if` 加上后 `None` 仍解出 `None`，唯键面变化可观） | ① 运行期侧证：`event_test.rs` 的「`turnId = None` 序列化后 `contains_key("turnId")` 为真」——这是 `skip_serializing_if` 缺席的**行为等价判据**（加上则键缺席，用例必红）；② 静态：`grep -rn "skip_serializing_if" crates/core/agent/src/event.rs` 零命中 + `grep -rn "#\[allow(" <67 处触及文件>` 无新增 + `grep -rn "Default::default()" <同上>` 无新增 |
| `stamp` 两参形态的**编译期拒绝**（`E0061`） | 编译期事实：两参调用在任何测试运行前即编译失败（`E0061` / `E0063`），无法写成运行期用例且不该写成运行时用例 | 编译闭合批本身即判据：`cargo clippy --workspace --all-targets` 通过 ⇒ 全仓 62 处调用点与 5 处字面量全部收敛三参 / 含 `turn_id`；`event_test.rs` / `kernel_test.rs` 各条 `stamp` 用例全部以三参形态书写，构成第二重机械证据 |
| AC-11 的 67 处机械改动的**完整性计数**（62 `stamp` + 5 字面量，分布在 26 个文件，见本表末的逐文件清单） | 计数判据属 grep / diff；且 design 待决问题 3 明确：测试相位新增用例会增加 `stamp(` 调用点 ⇒ **不得**以「全仓恰 62 处」为断言（那是会自坏的断言） | 逐条静态核对：每个 `stamp(` 的首实参为 `Some(turn_id)`（仅 `kernel.rs:248` 一处）或 `None`（其余），5 处结构体字面量均含 `turn_id:`；正确判据是「首参都非 `seq` 表达式」这一形态核查 + `cargo clippy --workspace --all-targets` 通过，而非绝对数 |
| `crates/infra/agent/src/sdk/{runner_test,loop_test,resume_test}.rs` 的 9 处 `stamp` 首参补 `None` 之上的**任何新增断言** | tasks.md 交接末条明令：在其上增补「证明轮归属」的断言只会伪造「引擎侧有轮概念」的印象（引擎结构上拿不到轮 id——观察通道载荷是 `AgentEventKind`，信封在核心泵才成形） | 该 9 处只是编译闭合批；轮归属判据全部落 `kernel_test.rs`；引擎零介入判据见本表「源码检索判据」行 |
| `crates/infra/store/src/model.rs` / `store.rs` 的**零 diff** 主张本身 | 「正确性主张」是两文件一行不变（D6 / AC-6 / AC-13），运行期不可观测 | diff 审查（`git diff --stat` 为空）+ 本文件 `model.rs` 章节十四行用例（零改动仍成立的运行期回归）+ 既有 `native_db` 全链用例全绿 |
| `crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`（引擎四文件，无可测面） | 轮 id 不进引擎：`LoopTurn` / `SessionOpen` / `PumpPayload` / `mpsc::Sender<AgentEventKind>` 签名与语义逐条不变，无任何新行为可测；为其加测试分支会篡改「轮归属是持久层治理信息」这一契约 | 编译期机械保证（观察通道载荷类型为 `AgentEventKind`，不含信封 ⇒ 引擎**结构上**拿不到轮 id）+ AC-2 静态判据 + 本表「源码检索判据」行 |
| AC-8 / AC-9 的**前端运行期 `safeParse` 三形态断言**（`turnId` 缺失 / `null` / 数字均放行）与「前端测试文件零 diff」 | tasks.md 约束②与 proposal「不要修改」条：`packages/desktop/src/**`（除生成物）MUST 零 diff ⇒ `packages/desktop/src/lib/{ipc-contract,ipc}.test.ts` 亦在禁令内，**不可新增**前端用例。为「证明可选键」改写 70 余处夹具更被 R-4 明令禁止（改即越界、且掩盖契约事实） | ① AC-8：`bindings.ts` 文本面 `turnId?: number \| null` 由 `mod_test.rs` 断言（可选位形态即「缺失合法」的静态判据）；`bindings.zod.ts` 的 `get turnId() { return z.number().nullable().optional(); }` 同文件断言（`.optional()` 即 zod 面放行）——两者合起来就是运行期行为的静态等价判据；② AC-9：`git diff --stat -- packages/desktop/src` 只含两份生成物 + 既有 109 个前端测试文件全绿（`vp test`） |
| 17 个前端测试文件（`src/**/*.test.ts(x)`，含事件字面量 `timestampMs` 命中 72 处）的**新增覆盖** | 同上：零 diff 约束；且这些文件的字面量缺 `turnId` 本身就是「可选键」契约的运行期证据——补字段即销毁证据 | 回归登记：既有用例零改全绿即 AC-9 的判据（`agent-error.test.ts` / `agent-messages.test.tsx` / `agent-render-memo.test.tsx` / `agent-session-view.test.tsx` / `use-agent-session.test.ts` / `agent-adapter.test.ts` / `agent-transport.test.ts` / `agent-debug-view.test.tsx` / `agent-raw-stream.test.tsx` / `agent-run-history.test.tsx` / `archi-chat-panel.test.tsx` / `change-chat-panel.test.tsx` / `change-detail-view.test.tsx` / `detail-drawer.test.tsx` / `session-transcript-panel.test.tsx` / `explore-detail-pane.test.tsx` / `use-explore-session.test.ts`） |
| AC-3 的「两实时出口（`agent_start` 的 `Channel` 与 `agent_session_watch` 的订阅流）**IPC 面**逐字段一致」 | 该一致性的机制是「两出口投递同一枚 `AgentEvent` 值」（design D3：`streams.publish(&session_id, event.clone())` 与 `on_output(KernelOutput::Observation(event))` 同源），跨 IPC 的逐字段比较需要真实 Tauri 应用与前端 Channel，超出单元测试面；且命令层本变更零改动 | ① 内核面强判据：`kernel_test.rs` 的「订阅端序列与 `on_output` 逐条相等」（`AgentEvent` 的 `PartialEq` 自动纳入 `turn_id`）+ AC-3 的三行用例；② 出线面：`bindings.ts` 的信封形态断言；③ 命令面零 diff 审查（`agent_start` / `agent_session_watch` 签名与注册表零变化） |
| 编译闭合批的 23 个测试夹具文件（`stamp` 首参补 `None`）+ 2 个只含结构体字面量的文件 上的**新增断言** | 这些文件在本变更中只有机械适配（`stamp` 首参补 `None` / 字面量补 `turn_id: None`），属编译闭合批；在其上增补断言会让 diff 从「机械」变「有语义」，破坏 AC-11 的可复核性（且 `sdk/**` 面另有明令禁止） | 编译闭合批的收敛由 `cargo clippy --workspace --all-targets` 与既有套件全绿举证；语义判据全部集中在 4 个新增用例文件（`event_test` / `kernel_test` / `model_test` / `bindings/mod_test`） |
| `crates/core/agent/src/event.rs` 的 `now_millis()`「时钟早于 epoch」臂 | 零参数函数、无时钟注入点（design 明示「零新时钟」）；要测该臂须给生产代码加时钟缝——属投机抽象（且本变更对该函数零改动） | 代码审查（`.duration_since(UNIX_EPOCH).unwrap_or(0)` 单一表达式，无 panic 路径）；运行期只锁「落在调用区间内」（`stamp` 正向行沿用既有 `millis_now()` 夹逼装置） |
| **JSON 序列化后的顶层键序**（`turnId` / `seq` / `timestampMs` / `kind` 的先后） | `serde_json` 未开 `preserve_order`（design D2）：`to_value` 走 BTreeMap（按键排序）、`to_string` 走 `FlatMapSerializer`（先非扁平字段、后扁平整形）；D2 明确**不承担保持旧键序的义务**，且「序列化后逐字节比对事件 JSON」在全仓不存在也不该被引入（存量行比对走 `SessionEventRecord.event: Box<RawValue>` 原文）。断言它会把 serde 内部序列化顺序钉进契约 | 不作 JSON 键序断言；只断言**键在场性与值**（`event.rs` 章节的线格式 / 往返行）与**新旧行混存可读**（`model.rs` 章节）。**注意区分**：`bindings.ts` 章节的 `find("turnId") < find("seq")` 断言对象是**生成物 TS 文本的字段声明序**（由 Rust 结构体字段序确定性派生，不是 `serde_json` 运行期键序），不属本条禁令 |
| 「按轮查询 / 按轮索引 / 按轮分页」的**任何用例** | delta spec「desktop-workspace-store」明令：该能力**未随本要求落地**，本期 MUST NOT 建列 / 建索引 / 加查询 API。写用例「证明」它会虚构不存在的契约（tasks.md 交接末条逐字禁止） | 负向静态判据：`grep -rn "turn_id" crates/infra/store/src/store.rs` 零命中（新字段不进查询面）+ `model.rs` 字段面断言（无新列）+ 既有 `list_session_events` 组全绿（扫描语义仍按 `(session_id, seq)`） |
| `step_index`（第二计数轴） | proposal「不要修改」末条：D 已裁定**不存在也不得新增**（前端按 assistant Message 序数计步）；本变更 MUST NOT 顺带新增 | 负向判据：`grep -rn "step_index\|stepIndex" crates/core/agent/src` 零命中 |
| AC-10 的 `sum_usage_tokens` / `final_assistant_text` / `failure_reason` **零改动的运行期主张**（三者只看 `kind`，信封新字段对其透明） | 三函数源码零 diff——「正确性主张」是签名与体一行不变；本节不给它们新增用例（那会把零改动做成有语义 diff，破坏 AC-11 的可复核性） | 回归登记：`crates/core/agent/src/usage_test.rs`（6 处 `stamp` 补 `None` 后既有断言零改全绿）+ `crates/core/orchestration/src/{transcript,state,walker}_test.rs` 既有用例全绿；语义侧证是这三支函数都只 `match` `event.kind`（信封字段不被读取），故新字段不改变其输出 |
| `crates/core/agent/src/event.rs` / `kernel.rs` 的**新增文档语义**（「轮归属为持久层治理信息、引擎不该知道」「轮 id ≠ `TurnDone.session_id`（引擎侧标识）」D9 / D4 的两条不变量） | 注释文本，运行期不可观测；把文档写进测试扫描会与桌面端静态检查重复且脆 | 源码审查（tasks.md 阶段一前三条逐字给定 doc 内容）+ 语义分开的行为侧证：`event_test.rs` 的「`TurnDone.session_id = Some("s-1")` 与 `turn_id = Some(7)` 并存且互不影响」形态（`turnDone` 变体线格式行天然覆盖两字段同层共存） |
| `SessionEventRecord::new` 的两处 `expect` 臂（`serde_json::to_string(&event).expect("AgentEvent 含可序列化 JSON 形态")` 与 `RawValue::from_string(..).expect(..)`） | 不可达臂：`AgentEvent` 全字段（`u64` / `i64` / `String` / `Value` / `Option` / 嵌套块）逐条可 JSON 化，`RawValue::from_string` 的输入是同进程自产的 `serde_json::to_string` 输出（必为合法 JSON）；要逼出该臂须给生产代码注入不可序列化字段——属投机抽象，且本变更对 `new()` 零改动 | 代码审查（两处表达式链无可失败输入）+ `model_test.rs` 六变体 × `turn_id` 四形态的落库回读用例（若 `expect` 可达则这些用例必然 panic 而非通过） |
| `packages/desktop/src-tauri/src/bindings/{mod.rs,zod.rs,zod_test.rs}`（导出管线与生成期硬约束）的**零改动**主张 | design D7 明示「零改 exporter」：`turnId` 的两处 zod 形态（基座判型 / 变体放行）完全由既有 `allowed_object_keys` 机制派生（先例 `seq` / `timestampMs` 同式），故管线侧无新行为可测；为「证明补齐机制」在 `zod_test.rs` 加 fixture 属重复覆盖既有机制、且会让该文件出现与本变更无关的 diff | diff 审查（三文件零 diff）+ 既有 `zod_test.rs` 全绿（`allowed_object_keys` 机制既有用例）+ `bindings.zod.ts` 产物文本的六变体断言（同一机制的真实产物侧判据） |
| `crates/core/agent/src/lib.rs` 的**零 diff**（`AgentEvent` 已在 `pub use` 名单，本变更不新增导出名） | 「正确性主张」是一行不变（design「事实偏差 2」）；运行期不可观测 | diff 审查（`git diff --stat` 为空）+ `lib.rs:10-13` 的 `pub use event::{.., AgentEvent, ..}` 逐字在场（既有）+ 编译通过（若导出缺失则全部测试文件无法 `use crate::event::AgentEvent`） |
| AC-12 的全量门禁（`cargo` 全量测试 / `vp check` / `vp test` 覆盖率 lines 90 / branches 85 / functions 90） | 机械门禁步骤，由桌面端静态检查与 test-execution 相位执行；在测试设计里复述阈值等于复制运行脚本细节（stage 约束） | 门禁执行结果 + 本文件各章节用例对覆盖率的支撑（新增断言集中在 4 个文件，含 `None` 与 `Some` 两支，不为覆盖率造句——R-7） |

编译闭合批的完整文件清单（design D5 逐文件分布，全部**只补 `None` / `turn_id: None`**；下表路径除壳层两行外均为 `packages/desktop/src-tauri/` 下的相对路径）：

| 面 | 文件（括号内为实测点位数） | 点位小计 |
|---|---|---|
| `core/agent` | `event_test.rs`(14) / `usage_test.rs`(6) / `lib_test.rs`(4) / `stream_test.rs`(3) / `port_test.rs`(1) / `state_test.rs`(1) —— 6 文件 | 29 |
| `core/orchestration` | `transcript_test.rs`(3) / `walker_test.rs`(3) / `state_test.rs`(1) —— 3 文件 | 7 |
| `core/orchestration`（字面量） | `control_test.rs:79` / `:560` / `:868` —— 1 文件（**无 `stamp` 点**） | 3 |
| `infra/agent` | `sdk/runner_test.rs`(6) / `store_port_test.rs`(3) / `compose_test.rs`(2) / `worker_test.rs`(2) / `sdk/loop_test.rs`(2) / `sdk/resume_test.rs`(1) —— 6 文件 | 16 |
| `infra/store` | `envelope_test.rs`(1) / `store_test.rs`(1) —— 2 文件；`model_test.rs:22` —— 1 文件（**字面量，无 `stamp` 点**） | 3 |
| 壳层命令 | `src/commands/exec/agent_test.rs`(2) / `src/commands/{change_flow,db,exec,explores,workspaces}/mod_test.rs` 各 1 —— 6 文件；`src/commands/change_flow/mod_test.rs:1658` 另含 1 处字面量（与上列同文件） | 8 |
| 生产 | `crates/core/agent/src/kernel.rs:248`（`Some(turn_id)`） | 1 |
| **合计** | **26 个文件**（23 个含 `stamp` 的测试夹具 + 1 生产 + 2 个字面量文件） | **67** |
