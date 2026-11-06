# 任务: agent-event-turn-id

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 4 项 + 机械改动 67 处（`stamp` 调用点 62 + 结构体字面量 5，见 design D5）。
> **零新增文件**；除生成物外全部为**修改**。
>
> **本相位不改测试文件的用例与断言**：`crates/core/agent/src/{event_test,kernel_test}.rs`、`crates/infra/store/src/model_test.rs`、`src-tauri/src/bindings/mod_test.rs` 的**新增用例**（proposal「新增用例」表 ①–⑩）归 test-design / test-gen / test-execution 相位。本相位对测试文件的唯一动作是**让工作区随新签名 / 新字段编译通过**（`stamp` 首参补 `None` / 结构体字面量补 `turn_id: None`），MUST NOT 新增用例、MUST NOT 改断言口径、MUST NOT 顺手把任何夹具改为 `Some(具体轮 id)`（会伪造轮归属）。
>
> **依赖序（四阶段）**：
>
> - **阶段一**（`core/agent` 契约面：`event.rs` 字段 + `stamp` 签名 + `kernel.rs` 生产点 + **全部 67 处机械适配**）是**不可再分的编译闭合批**——`stamp` 扩参后 62 处调用点缺首参即 `E0061`；`AgentEvent` 加字段后 5 处字面量缺字段即 `E0063`。拆开任一侧工作区即编译失败、无可验证中间态。批末判据是 `cargo clippy --workspace --all-targets` 通过 + 既有用例全绿（**零行为变化**，故既有断言不该有任何一条变红）。
> - **阶段二**（生成物重导出）依赖阶段一（`AgentEvent` 字段在场才会出现在产物里）。它必须在阶段三之前——不重导出则前端 `ipc` 的 Channel 信封校验对携带 `turnId` 的每枚事件软失败（design D7 的实测证据）。
> - **阶段三**（版本 + 边界自查）依赖阶段一、二。
> - **阶段四**（交接备忘）零代码依赖，可在任意时点填写。
>
> 阶段一与二串行刚性强（编译闭合批 → 重导出），阶段三可与阶段四并行。
>
> **关键约束（贯穿全部任务）**：⓪ `turn_id` 的 `#[serde(default)]` **MUST 在场**、`skip_serializing_if` **MUST NOT 出现**（后者在 unified 相位模型下不可表达，会令 bindings 分裂 `_Serialize` / `_Deserialize`，破 `src/bindings/mod_test.rs` 的既有 golden 断言）；① 机械改动一律机械，**MUST NOT** 引入 `#[allow(...)]`、**MUST NOT** 用 `..Default::default()` 逃逸（`AgentEvent` 无 `Default`）；② **引擎侧**四文件 `crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`（与 proposal「不要修改 - 引擎侧」同口径）、`crates/infra/store/src/{model,store}.rs`、`crates/core/agent/src/lib.rs`、全部前端 `src/**`（除生成物）**MUST 零 diff**；**显式例外**：`crates/infra/agent/src/sdk/{runner_test,loop_test,resume_test}.rs` **有且仅有 9 处** `stamp` 首参补 `None` 的 diff（`runner_test` 6 / `loop_test` 2 / `resume_test` 1，见阶段一第 9 项与 design D5）——它们是 `sdk/` 下的**测试**文件、属编译闭合批，与「引擎零改动」（AC-13）**同义不矛盾**；核对时 MUST NOT 用 `sdk/**` 通配 `git diff` 判红，须逐文件比对四个引擎 `.rs`，或对 `sdk/` 全量 diff 逐条确认「只含 `stamp(` 首参补 `None`」；③ 生成物 **MUST NOT 手改**（`bindings.ts` / `bindings.zod.ts` 只由 `bindings:export` 产生，手改会在下次重导出被覆盖并破坏幂等）；④ `src-tauri/Cargo.toml` 的 crate 版本行 **MUST NOT** 跟随 `package.json`；⑤ 静态脚本（static-check / test-execution 等机械步骤）由桌面端执行，本相位不重复执行。

## 阶段一：core/agent 契约面 + 编译闭合批（`crates/core/agent` + 全部调用点）

- [x] 修改 `crates/core/agent/src/event.rs`：`AgentEvent` 增**首字段** `#[serde(default)] pub turn_id: Option<i64>`，doc 写全五点——① 轮归属 = **持久层权威分配的轮 id**（内核 `begin_turn` 经 `sink.begin_turn` 取得，与轮统计行主键同源），一轮内全部事件（含 `MessageDelta`）同值；② `None` 仅出现在无轮归属的构造面（测试夹具 / 手工构造）；③ **与 `TurnDone.session_id`（引擎侧会话标识，双 id 映射半边）语义完全不同，MUST NOT 互相替代或混用命名**；④ 属性面只标 `default`——`skip_serializing_if` 在 unified 相位模型下无法表达，加则 bindings 分裂为相位伴生类型（沿 `Text.citations` 同款措辞）；⑤ 线格式键恒在、值可 `null`
- [x] 修改 `event.rs` 的 `AgentEvent` 结构体头 doc：由「每事件携带 `seq`（单调序号，入库排序键）与时间戳」改为「每事件携带 `turn_id`（轮归属）、`seq`（单调序号，入库排序键）与时间戳」；`kind` 扁平进线格式（线格式 = 落库形态（密封） = 前端 DTO 基准）一句**逐字保留**
- [x] 修改 `event.rs` 的 `AgentEvent::stamp`：签名改为 `pub fn stamp(turn_id: Option<i64>, seq: u64, kind: AgentEventKind) -> Self`，体内 `Self { turn_id, seq, timestamp_ms: now_millis(), kind }`；doc 改为「盖当前时钟毫秒的唯一生产入口：**轮归属为首参**（『归属 → 身份 → 载荷』阅读序），`seq` 由调用方（事件生产者）传入；轮 id 是持久层分配的治理信息、引擎不该知道；MUST NOT 另立第二条生产入口（两条入口会让漏写轮归属从编译错误退化为运行期静默 `null`）」
- [x] 修改 `crates/core/agent/src/kernel.rs:248`：`let event = AgentEvent::stamp(seq, kind);` → `let event = AgentEvent::stamp(Some(turn_id), seq, kind);`（`turn_id` 来自 `drive` 开头 `:211-221` 已有的解构绑定，**零新增字段 / 零新增形参**）；就地加一行注释「全事件唯一盖戳点：轮归属就在 `self` 上；delta 与密封共用此点，两实时出口投递同一值」
- [x] `kernel.rs` 其余面**逐字不动**：`begin_turn`、`converge`、`report_session_id`、`RunStateMachine::apply` 调用、`sink.append_sealed` / `bind_session` / `finish_turn` 调用序、ask 送达失败臂、停止 / 兜底收敛分支、`TurnStats` 摘取
- [x] 机械适配 core/agent 夹具（`stamp` 首参补 `None`，共 **29** 处）：`event_test.rs`(14) / `usage_test.rs`(6) / `lib_test.rs`(4) / `stream_test.rs`(3) / `port_test.rs`(1) / `state_test.rs`(1)
- [x] 机械适配 core/orchestration 夹具（首参补 `None`，共 **7** 处）：`transcript_test.rs`(3) / `walker_test.rs`(3) / `state_test.rs`(1)
- [x] 机械适配 core/orchestration 字面量（补 `turn_id: None`，共 **3** 处）：`control_test.rs:79` / `:560` / `:868` 的 `agent::AgentEvent { seq, timestamp_ms, kind }`
- [x] 机械适配 infra/agent 夹具（首参补 `None`，共 **16** 处）：`sdk/runner_test.rs`(6) / `store_port_test.rs`(3) / `compose_test.rs`(2) / `worker_test.rs`(2) / `sdk/loop_test.rs`(2) / `sdk/resume_test.rs`(1)——其中 `sdk/` 下 **9 处**是 `sdk/` 目录里的**测试**文件（`*_test.rs`），属编译闭合批；引擎四文件 `sdk/{loop,normalize,runner,resume}.rs` **一条不动**（design D10 / D11）
- [x] 机械适配 infra/store 夹具：`stamp` 首参补 `None` —— `envelope_test.rs`(1) / `store_test.rs`(1)；结构体字面量补 `turn_id: None` —— `model_test.rs:22` 的 `event()` fixture
- [x] 机械适配壳层命令夹具：`stamp` 首参补 `None` —— `src/commands/exec/agent_test.rs`(2) / `commands/{change_flow,db,exec,explores,workspaces}/mod_test` 各(1)；结构体字面量补 `turn_id: None` —— `src/commands/change_flow/mod_test.rs:1658`
- [x] 随动校正注释里的**具体参数序**：`crates/core/agent/src/usage_test.rs:7` 的模块 doc 由 `AgentEvent::stamp(seq, AgentEventKind::TurnDone { usage, .. })` 改为 `AgentEvent::stamp(None, seq, AgentEventKind::TurnDone { usage, .. })`；`core/orchestration/src/state_test.rs:9`、`walker_test.rs:286` / `:460` 三处只提方法名，**逐字保留**（改它们属噪音 diff）
- [x] 自查 阶段一：`grep -rn "AgentEvent::stamp(" --include=*.rs --exclude-dir=target .` 逐条核对**首实参**为 `Some(turn_id)`（仅 `kernel.rs:248`）或 `None`（其余）；`grep -rn "AgentEvent {" --include=*.rs --exclude-dir=target . | grep -v -E "struct AgentEvent \{|impl AgentEvent \{|-> .*AgentEvent \{"` 得 **5 处**字面量且均含 `turn_id:`（裸 `grep "AgentEvent {"` 会连 `struct` / `impl` / 返回型声明一并命中，不足以判据）；`grep -rn "skip_serializing_if" crates/core/agent/src/event.rs` 零命中；`grep -rn "turn_id\|turnId" crates/infra/agent/src/sdk/` 零命中（AC-2 / D10；该目录的 9 处 `*_test.rs` 只补 `None` 首参、不写该字样，故判据在改动后照旧成立）；`crates/core/agent/Cargo.toml` 与 `crates/core/agent/src/lib.rs` 零 diff；`cargo clippy --workspace --all-targets` 通过，既有用例全绿（零行为变化）

## 阶段二：生成物重导出（`src/types/generated`）

> 前置：阶段一（`AgentEvent` 字段在场）。**MUST NOT 手改**两份产物。

- [x] 执行 `pnpm -C packages/desktop run bindings:export`（内部先 `cargo run -p export-bindings` 落两份产物，再 `bindings:zod-check` 以 `tsc --noEmit -p tsconfig.zod.json` 编译自检；自检失败即中断，产物不入库）
- [x] 核对 `packages/desktop/src/types/generated/bindings.ts`：`export type AgentEvent = {` 段内在 `seq: number,` **之前**出现 `turnId?: number | null,`；`} & AgentEventKind;` 结尾形态与 `AgentEventKind` 六变体字面量（`kind: "runStarted"` 等）**逐字不变**；`TurnSummary.turnId: number`（轮行面）与命令签名**零 diff**
- [x] 核对 `packages/desktop/src/types/generated/bindings.zod.ts`：`AgentEventSchema` 基座（`z.strictObject({` 段）出现 `get turnId() { return z.number().nullable().optional(); },`；六变体 `z.strictObject` 各出现一行 `turnId: z.unknown().optional(),`（与既有 `seq` / `timestampMs` 同式，由 exporter 的 `allowed_object_keys` 自动补齐——**零手补**）；`argsSchemas` / `resultSchemas` / `channelArgs` 三张表**零 diff**
- [x] 核对产物**无相位分裂**：`grep -c "_Deserialize\|_Serialize" packages/desktop/src/types/generated/bindings.ts` 为 0（AC-1 的判据；若出现即说明 `skip_serializing_if` 被误加，回阶段一修正）
- [x] 自查 阶段二：`git diff --stat -- packages/desktop/src/types/generated` 只含 `AgentEvent` / `AgentEventSchema` 相关行（无其它 DTO 漂移）；连续两次 `bindings:export` 的产物零 diff（幂等）

## 阶段三：版本与边界自查

- [x] 修改 `packages/desktop/package.json` 的 `version`：`0.6.18` → `0.6.19`（一次 patch）；`dependencies` / `devDependencies` **零变化**
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（**零手改**，跟随即生效）
- [x] 核对 `packages/desktop/src-tauri/Cargo.toml` 的 crate 版本行 `version = "0.1.0"` **未被改动**（MUST NOT 跟随 `package.json`）
- [x] 自查 边界零 diff（`git diff --stat` 逐项核对，design D11 的清单）——**引擎按文件枚举，不用 `sdk/**` 通配判红**：`packages/desktop/src-tauri/crates/infra/agent/src/sdk/{loop.rs,normalize.rs,runner.rs,resume.rs}`（引擎四文件）、`packages/desktop/src-tauri/crates/infra/store/src/{model.rs,store.rs}`、`packages/desktop/src-tauri/crates/core/agent/src/lib.rs`、`packages/desktop/src/lib/**`、`packages/desktop/src/hooks/**`、`packages/desktop/src/components/agent/**`、`packages/desktop/src/views/**`、`openspec/specs/**`、`openspec/config.json`、`packages/desktop/src-tauri/crates/core/config/config.schema.json`、`packages/desktop/src-tauri/crates/*/*/tests/golden/*.json`
- [x] 自查 `sdk/` 下的**预期 diff**（**不是越界**）：`git diff -- 'packages/desktop/src-tauri/crates/infra/agent/src/sdk/'` 应只含 `runner_test.rs`(6) / `loop_test.rs`(2) / `resume_test.rs`(1) 共 **9 处** `stamp(` 首参补 `None`（design D5 / D11）——逐条确认 diff 行**只**为 `AgentEvent::stamp(seq, kind)` → `AgentEvent::stamp(None, seq, kind)` 形态；若出现任何 `turn_id` / `turnId` 字样或引擎四文件（`loop.rs` / `normalize.rs` / `runner.rs` / `resume.rs`）的非零 diff，即为越界（AC-2 / AC-13）
- [x] 自查 机械改动完整性：`grep -rn "AgentEvent::stamp(" --include=*.rs --exclude-dir=target packages/desktop/src-tauri | wc -l` 得 **63**（基线：62 处代码点 + 1 处 `usage_test.rs:7` 的模块 doc 提及）且逐条核对每个 `stamp(` 的**首实参**为 `Some(turn_id)`（仅 `kernel.rs`）或 `None`（其余）——**不要以绝对数 62 作断言**（测试相位新增用例后会增长，见 design 待决问题 3）；`grep -rn "#\[allow(" <本变更触及的 26 个文件>` 无新增；`grep -rn "Default::default()" <同上>` 无新增（AC-11；26 = 24 个含 `stamp(` 的文件 + `core/orchestration/src/control_test.rs` + `crates/infra/store/src/model_test.rs` 两处字面量文件）
- [x] 自查 前端零改动：`git diff --stat -- packages/desktop/src` 只含 `src/types/generated/{bindings.ts,bindings.zod.ts}`（AC-9 / AC-13）

## 阶段四：交接备忘（发给 test-design / test-gen / test-execution）

> 本相位不写用例，只把「哪个契约面需要被钉住」与「为何某些面不可写」交接出去。

- [x] 交接 `crates/core/agent/src/event_test.rs`：① 信封线格式**六变体各一枚**含顶层 `turnId` 键（`Some` → 数字、`None` → `null`，键恒在场）；② 旧 JSON（无 `turnId` 键）反序列化读出 `turn_id = None` 且其余字段逐字段不变；③ `Some(turn_id)` 往返逐字段相等；④ `None` 形态序列化必留 `"turnId":null`（即 `skip_serializing_if` 不存在的机械判据）；⑤ `u64::MAX` seq / `i64` 边界轮 id 不截断（既有 `stamp对u64最大seq…` 用例邻域）
- [x] 交接 `crates/core/agent/src/kernel_test.rs`：⑥ `drive` 流出事件（delta + 密封混合流）`turn_id` 全等于 `RunningTurn.turn_id`（无一枚 `None`）；⑦ `Continue{id}` 续轮时两轮事件各自轮内恒定、跨轮不同（可复用 `begin_turn_continue不重复建会话行且多轮轮id递增…` 与 `同会话次轮盖戳自库内最大seq续排…` 的驱动骨架）；⑧ 既有「两实时出口载荷逐条相等」用例（`drive的delta与密封同源发布_订阅端序列与输出回调逐条相等`）在新字段下仍绿（自动覆盖「同源同值」）
- [x] 交接 `packages/desktop/src-tauri/src/bindings/mod_test.rs`：⑨ 在 `agent_event出线为kind判别联合且seq摊平变体字面量camel_case` 内追加 `turnId?: number | null,` 形态断言（`AgentEvent` 摊平段）；既有 `seq` / `timestampMs` / `} & AgentEventKind;` / 六变体字面量断言**逐字保留**（零改动即成立，见 design「事实偏差 3」）
- [x] 交接 `crates/infra/store/src/model_test.rs`：⑩ 旧行（载荷缺 `turnId`）经 `SessionEventRecord::event()` 读出 `turn_id = None` 且其余字段不变；⑪ 新行（`Some(7)`）落库 → 回读键在场且值相等、`seq` 排序与全史连续性不变；⑫ 既有 `旧schema转录行解码重编码逐字节不变且可被native_db删除` 用例的**延展**（旧行原文一律不含 `turnId`，逐字节判据与 `native_db::remove` 全链不破）——design D6 已明确：`AgentEvent` 层往返会落出显式 `"turnId":null`，故**必须**区分「记录层字节不变」与「类型层补默认值」两个层级
- [x] 交接不可写面（防误加用例）：`crates/infra/agent/src/sdk/{loop,normalize,runner,resume}.rs`（引擎四文件）无可测面（轮 id 不进引擎，AC-2 用**源码检索**而非用例举证）；`sdk/{runner,loop,resume}_test.rs` 的 9 处 `stamp` 首参补 `None` 属**编译闭合批**（已在阶段一完成），其上**MUST NOT** 增补任何断言「证明」轮归属（那只会伪造引擎侧有轮概念的印象）；前端 17 个测试文件零改动（可选键出线，AC-9 用 `git diff --stat` 举证）；`step_index` **不存在也不得新增**（后续 change D 已裁定由前端按 assistant Message 序数计步）；「按轮查询」**未实现**（store 侧 spec 已钉，MUST NOT 建按轮索引 / 查询 API 的用例来「证明」它）
- [x] 交接计数口径提醒：design D5 的 62（`stamp` 调用点）/ 61（测试）/ 5（字面量）/ 67（合计）是**落地时刻**的实测值；测试相位新增用例会增加 `stamp(` 调用点，故门禁自查**不得**以「全仓恰 62 处」为断言（见 design 待决问题 3）
