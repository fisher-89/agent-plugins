# 测试设计: steer-running-agent-step

> **变更**: steer-running-agent-step
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（四现象 / D1–D10 裁决 / 三层改动 / 变更范围实现文件 19 行 + 测试文件 16 行 / AC-1..AC-12 / R1–R11）、`design.md`（DD1–DD12 / 架构组件 10 行 / 新增文件 **1 行** + 修改文件 **17 行** / 编译闭合批 5 行 / 公共函数·API **16 行** / 类型定义 8 行 / 数据模型 / 路由与错误语义 / AC 落点表）、三份 delta spec（`desktop-agent-execution` 3 MODIFIED + 1 ADDED / `desktop-agent-chat-infra` 1 MODIFIED + 1 ADDED / `desktop-change-flow-view` 1 MODIFIED）
> **被测范围**: design「新增文件」1 项（`crates/core/agent/src/steer.rs`）+ design「修改文件」全 17 项。**删除文件零项**（本变更为纯增量加位，design 无删除面），故零文件排除。
> **本变更性质**: 一条**跨四层的用户 → turn 单向注入通道**——① 纯表（`SteerRegistry` 三方法 + ack 语义 + `AskRegistry::is_pending` 只读判据）；② 会话地基（`AgentSession` 第四件套 + 内核 `begin_turn` 登记 / `converge` 除名，与 `StopRegistry` / `AskRegistry` 同址）；③ 引擎（`open_session` 铸通道 → 泵透传 → `loop::run` 请求边界 `try_recv` 循环 drain：独立密封事件 + 独立 `Message::user` + 独立 ack）与壳层（`agent_steer` 四段薄包装，三态 + 三 blank 共六条文案）；④ 前端（headless `steer` / `steering`、适配层「一轮多条 user 消息」的 id 规则、`AgentInput` 两可选位、抽屉步面板接 composer）。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/**` 与 `src/**`） | `cargo nextest run --workspace`（`server:test`；本变更为 `-p agent` / `-p agent-runtime` / `-p dev-team` 三包定向） | **sidecar 同目录同前缀** `*_test.rs`，源文件尾 / `lib.rs` / `mod.rs` 以 `#[cfg(test)] mod <name>_test;`（或 `#[path = "…_test.rs"] mod <name>_test;`）挂载；**本变更新建 1 个测试文件**（`core/agent/src/steer_test.rs`）+ **10 个既有 Rust sidecar 改造/追加**（另 `worker_test.rs` 仅夹具随动） | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为**中文短语**；装置为手写端口假件（`FakeSink` / `FakeRunner` / rig `test_utils` 流缝 / `tauri::test::mock_app`）+ `tempfile` 真库 |
| 前端（`packages/desktop/src/**`） | `vp test`（vite-plus 承载 vitest；`client:test`） | **同目录同前缀** `*.test.tsx` / `*.test.ts`；**本变更零新建测试文件，5 个既有测试文件追加** | `vite-plus/test` 的 `describe` / `it` / `expect` / `vi`；`@testing-library/react`（`render` / `renderHook` / `screen` / `fireEvent` / `act` / `waitFor`）；`// @vitest-environment jsdom` 首行；进程边界一律 `vi.mock('@tauri-apps/api/core', …)` + `vi.hoisted` 的 `ChannelMock` / `invokeMock` |
| 生成物 / 版本行 | — | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/package.json` **零直接测试路径**（生成物由 `bindings:export` + `src/bindings/mod_test.rs` 反向锚定守住；版本行走 diff 审查） | 登记于「不可测试项」 |

**测试文件面清单（本变更触及的 11 个 Rust 测试文件 = 1 新建 + 10 既有，+ 5 个前端测试文件）**

| 面 | 新建 | 改造 / 追加 |
|---|---|---|
| Rust | `core/agent/src/steer_test.rs` | `core/agent/src/{ask,kernel,runner,lib}_test.rs`；`infra/agent/src/sdk/{loop,runner}_test.rs`；`infra/agent/src/compose_test.rs`；`src/commands/{exec,change_flow}/mod_test.rs`；`src/bindings/mod_test.rs` |
| 前端 | —（零新建） | `hooks/use-agent-session.test.ts`；`lib/agent-adapter.test.ts`；`components/agent/{agent-session-view,agent-input}.test.tsx`；`views/changes/flow/session-transcript-panel.test.tsx` |
| 夹具随动（零新增用例，见「不可测试项」机械点位行） | — | `infra/agent/src/worker_test.rs`；上述各文件的构造点补位（`SessionKernel::new` 57 处 / `AgentSession` 字面构造 7 处 / `loop::run` 21 处 / `compose_turn` 52 处） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、极值、恰好阈值、并发与竞态窗口）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有…」前缀**：该断言由既有用例锁定，本设计**只做回归登记**（不新增断言、不改断言口径）；未带前缀者为新增或按新形状改写。
- **「↔ 编译闭合」**：该行只是让工作区随字段面 / 签名收敛编译通过的机械点位（design「编译闭合批」），**MUST NOT 新增用例或改断言值**；行内只登记点位与计数。
- **「夹具随动」**：构造点补字段 / 补实参，断言口径零改动（`E0063` / `E0061` 由编译器兜住，无运行期中间态）。
- **「组合行」**：跨模块链路用例，挂靠到该链路的**发起方 / 最上层调用方**模块章节（见「跨模块组合用例挂靠」），不另立集成测试章节。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| `String`（`text` / 补充说明正文） | `""`（空串：命令面拒、表与 loop 面零校验逐字入史）、纯空白 `"   "`、`"\n"`、`"\0"`、emoji（`"a\0b🚀"`）、超长（`"字".repeat(1200)`，>1000 字符）；**无 `None`**（非 `Option`） |
| `String`（`session_id` / 注册表键） | `""`、`"   "`（命令面 blank 拒；表不 trim 不截断）、`"ses-…"` 正常键、超长键；近邻键（`""` vs `" "`）互不命中 |
| `String`（`root`） | `""` / `"   "` / `"\t\n"`（blank → `Err` 携字段名）、非空白但不存在的路径（命令面不探测，零落库零寻址） |
| `Result<(), String>`（命令出口） | 六条文案各自独立：blank root / blank session_id / blank text / 「等待应答中…」/「会话已收口…」/「未送达（…生效前结束）」——两两不等且各含成因关键词 |
| `mpsc::UnboundedSender<SteerMessage>`（表槽） | 三形态：未登记（miss）/ 登记且接收端在场（命中）/ 登记但接收端已 drop（`send` 失败同归 miss 臂） |
| `oneshot::Sender<()>`（ack） | 四形态：正常消费一次 / 二次 `send` 返 `Err` / 未被消费即 drop（等待侧 `RecvError`）/ 轮收口随接收端 drop（drop-Err） |
| `bool` | `AskRegistry::is_pending` 真 / 假；`steering` 置位 / 复位；`pending` / `hidePermissionMode` / `running` / `disabled` 的真假交叉（无 `None`，`Option` 由两可选位承担） |
| `Option<String>`（前端 `root` / `sessionId`） | `null`（零 invoke 守卫）/ 值（invoke 逐字）；`undefined` 与 `null` 同语义 |
| `Vec<Message>`（loop 史） | 空史 / 尾部为 `tool_results`（drain 落点）/ 超水位（防线裁剪面，`R5` 已知代价——**不为 steer 开免裁特权**）/ 收口后不再追加 |
| `u64`（`seq`） | 起点 0 形态与递增：同轮多条补充说明事件 seq 单调 |
| TS `AgentSessionActions` / `AgentInputProps` | 新字段全为 `additive` / 可选：缺省（既有消费点零变化）/ 显式 `false` 与缺省等价 / 显式 `true`；引用相等判据（`steer` 函数）只能经探针或 `renderHook` 直接持有引用观测 |

**跨模块组合用例挂靠（按链路发起方 / 最上层调用方）**

- **注入链**（`agent_steer` 命令 → `AskRegistry::is_pending` → `SteerRegistry::request` → 通道 → `ack`）最上层调用方 = 命令面 `commands/exec` ⇒ 组合行写 `src-tauri/src/commands/exec/mod.rs -> mod_test.rs` 章节。
- **登记 / 除名链**（组合根 `compose_turn` → `SessionKernel::new` 第 5 参 → `begin_turn` 登记 → `converge` 除名）发起方 = 组合根 `compose.rs` ⇒ 透传 + 生命周期组合行写 `crates/infra/agent/src/compose.rs -> compose_test.rs` 章节；纯内核半边（登记 / 除名 / 隔离）留 `kernel_test.rs`。
- **引擎送达链**（通道 → `loop::run` drain → 密封事件 → `Message::user` → ack）发起方 = `sdk/loop.rs` ⇒ 全部 drain 行为行写 `crates/infra/agent/src/sdk/loop.rs -> sdk/loop_test.rs` 章节。
- **呈现链**（密封 user 事件 → 适配层 / 实时路 → `AgentSessionActions` → 抽屉 composer）最上层消费点 = 视图 `session-transcript-panel` ⇒ 端到端呈现组合行写 `views/changes/flow/session-transcript-panel.tsx -> session-transcript-panel.test.tsx` 章节；两路（实时 / 折叠）各在自己的适配层章节锁定。
- 不新建独立集成测试章节、不新建组合测试区；跨 crate 的「同一实例」判据以 `Arc` 指针持有 + `request` 命中/落空的二值观测承载。

---

## 验收范围

<!-- 测试区域：Rust 侧 `packages/desktop/src-tauri/crates/{core/agent,infra/agent}/src/**/*_test.rs` 与 `packages/desktop/src-tauri/src/**/mod_test.rs`（sidecar，`cargo nextest`）；前端侧 `packages/desktop/src/**/*.test.ts(x)`（`vp test`）。本变更新建 1 个测试文件（`core/agent/src/steer_test.rs`）、改造 10 个既有 Rust sidecar（另 `worker_test.rs` 仅夹具随动）与 5 个既有前端测试文件 → 16 个 per-file 章节（`src/bindings/mod_test.rs` 承接 `commands/mod.rs` 一个源文件）。不可测面（`main.rs` / 生成物 / 版本行）见文末「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **同轮送达**：一轮内注入的补充说明在下个请求边界进入本轮史（`Message::user` 出现在 `tool_results` 之后，MUST NOT 并入那条消息），转录出现 `role = user` 的密封事件（seq 单调、内核单点盖戳），会话 id / 轮 id 不变、零新会话 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（drain 入史 + 请求史两条独立条目）；`packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` -> `kernel_test.rs`（`begin_turn` 登记，键 = core 铸 id）；`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `compose_test.rs`（第 5 参透传同一实例） |
| AC-2 | **不合并**：同一边界前到达三条 → 三条独立密封 user 事件 + 三条独立 `Message::user` + 三次 ack（三条命令各自 `Ok`）；无滞留到下一请求边界的项 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（一次边界取净）；`packages/desktop/src-tauri/crates/core/agent/src/steer.rs` -> `steer_test.rs`（一条一 ack） |
| AC-3 | **收口即 Err**：轮已收口（`converge` 后 / 接收端已 drop）再注入 → `Err`「会话已收口，补充说明不可送达」；不排队、不 Continue 重开、会话状态零改动 | `packages/desktop/src-tauri/crates/core/agent/src/steer.rs` -> `steer_test.rs`（miss 与 `send` 失败同臂）；`packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` -> `kernel_test.rs`（`converge` 除名）；`packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `mod_test.rs`（命令面 miss 臂 + 零副作用） |
| AC-4 | **ask 优先判据**：ask 阻塞期间注入 → `Err`「等待应答中，补充说明不可送达」，且该文本不入通道、应答后不补投；`is_pending` 为唯一判据（`SteerRegistry` 条目此刻仍在场） | `packages/desktop/src-tauri/crates/core/agent/src/ask.rs` -> `ask_test.rs`（`is_pending` 四态）；`packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `mod_test.rs`（判据次序 + 不补投） |
| AC-5 | **ack 竞态诚实**：投递成功但 ack 前轮收口 → `Err`「未送达（会话在补充说明生效前结束）」；零超时机制、零额外清理表项 | `packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `mod_test.rs`（ack drop 臂）；`packages/desktop/src-tauri/crates/core/agent/src/steer.rs` -> `steer_test.rs`（ack 单次语义 / 未消费即 drop） |
| AC-6 | **停止不改写转录**：补充说明在途时停止 → 轮收敛 `stopped`、命令收 `Err`、转录无该文本事件、注册表键已除名（后续 `request` → `Err`） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（停止截停先于 drain）；`packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` -> `kernel_test.rs`（两表同址除名） |
| AC-7 | **一轮一命不破**：全程恰一个泵、恰一个提问（`questions.recv` 一次）、恰一个轮终态；`EngineFacade` / `PumpPayload` / `LoopTurn` 三处零改动 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（`RunStarted` / `TurnDone` 各一条护栏）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`（零改动结构事实，见「不可测试项」） |
| AC-8 | **命令面纪律**：`agent_steer` 四段薄包装（blank 三参 → ask 判据 → 注册表投递 → ack），blank 臂携字段名；零新开会话 / 轮 / 状态机 / 广播；登记在 `all_commands!` 与 `COMMAND_NAMES`（50 → 51）；`bindings:export` 后再生成零 diff | `packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `mod_test.rs`（六态文案 + 零副作用）；`packages/desktop/src-tauri/src/commands/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`（登记面 + 计数 + 生成序） |
| AC-9 | **前端可见（实时路）**：运行中收到轮中段 `role = user` 密封事件 → 新增 `evt-<seq>` user 气泡（MUST NOT 被「同轮已有同名 id」丢弃），提问气泡仍恰一条 | `packages/desktop/src/hooks/use-agent-session.ts` -> `use-agent-session.test.ts` |
| AC-10 | **前端可见（折叠路）**：含补充说明的转录重放 → 提问（`turn-<turnId>-user`）+ 补充说明（`evt-<seq>`）+ 恰一条 assistant，序为 提问 → 补充说明 → assistant；两路 id 同源、两两不等 | `packages/desktop/src/lib/agent-adapter.ts` -> `agent-adapter.test.ts`；`packages/desktop/src/hooks/use-agent-session.ts` -> `use-agent-session.test.ts`（收口整表替换同形） |
| AC-11 | **动作面与判据不预判**：headless `steer` 三参逐字、null 守卫零 invoke、`steering` 提交置位 / 成功与 `Err` 均复位、`Err` 进既有错误面；已收口步仍可点发送并由后端 `Err` 说话 | `packages/desktop/src/hooks/use-agent-session.ts` -> `use-agent-session.test.ts`；`packages/desktop/src/components/agent/agent-session-view.tsx` -> `agent-session-view.test.tsx`（动作面组装）；`packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` -> `session-transcript-panel.test.tsx` |
| AC-12 | **composer 变体加位**：`hidePermissionMode` 时无档位行；`pending` 时发送禁用且呈「排队中…」、输入可编辑、停止入口照常；默认变体（两位缺省）渲染与交互逐字不变 | `packages/desktop/src/components/agent/agent-input.tsx` -> `agent-input.test.tsx`；`packages/desktop/src/components/agent/agent-session-view.tsx` -> `agent-session-view.test.tsx`（`steer` / `steering` 组装后进插槽） |

> **口径说明**：AC-7 的「三处零 diff」与 AC-8 的「`bindings:export` 后再生成零 diff」是**结构事实**，运行期无法断言（见「不可测试项」）；本表只登记行为侧可断言半边。静态检查链、全量测试与版本递增是全仓既有交付门，由桌面端 static-check / test-execution 相位承接。

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/agent/src/steer.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/steer_test.rs`

**新建 sidecar**（沿 `ask_test.rs` / `kernel_test.rs` 的 `StopRegistry` 同族先例）；`lib.rs` 补 `#[cfg(test)] mod steer_test;`（与 `steer_test.rs` 同批落地）。装置为零进程边界对拍：用例直接持有 `mpsc::unbounded_channel` 的接收端与 `oneshot::channel` 的 ack 两端，注册表为真实实例。

#### 待测功能

- `SteerRegistry::new() -> Self`: 空注册表（`Default` 派生的薄包装，壳层 `manage` 需要 `Default`）。
- `SteerRegistry::register(&self, session_id: &str, tx: mpsc::UnboundedSender<SteerMessage>)`: 内核开轮登记 sender（键 = core 铸会话 id，与 `StopRegistry` / `AskRegistry` 同址）；同键覆盖即上一轮接收入端已 drop。
- `SteerRegistry::request(&self, session_id: &str, msg: SteerMessage) -> Result<(), String>`: 投递一条补充说明；**送达语义**——miss 与 `send` 失败同归 `Err("会话已收口，补充说明不可送达")`（MUST NOT 照抄 `answer` 的受理语义）。
- `SteerRegistry::remove(&self, session_id: &str)`: 轮收敛除名；miss 幂等。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SteerRegistry::new` / `SteerRegistry::request` | 正向 | 空表形态：`new()` 后 `request("s1", msg)` → `Err`，文案逐字含「会话已收口，补充说明不可送达」；`default()` 与 `new()` 行为等价（空表、可登记、可请求） | 参数化迭代（2 构造面） |
| `SteerRegistry::register` / `SteerRegistry::request` | 正向 | 命中往返：`register("s1", tx)` → `request("s1", {text: "先跑 lint", ack})` → `Ok`；接收端 `recv().await` 得该 `SteerMessage`（`text` 逐字、`ack` 在场），`ack.send(())` 后等待侧收 `Ok(())` | 单次 |
| `SteerRegistry::request` | 正向 | 不合并（AC-2 的表半边）：同键连续三次 `request` → 三次各 `Ok`；接收端按序收到**三条独立** `SteerMessage`（无合并、无覆盖），三个 ack 各自独立可回 | 单次 |
| `SteerRegistry::remove` | 正向 | 除名与幂等：`register` → `remove` → `request` → `Err`；`remove` 对未登记键连调两次不 panic、不波及其它键 | 参数化迭代（2 形态：已登记键 / 从未登记键） |
| `SteerRegistry::request` | 异常 | `send` 失败同臂（DD1）：`register("s1", tx)` 后 `drop(rx)` → `request` → `Err`，文案与 miss 臂**逐字相同**（两成因同一事实：「该会话当前没有在跑的轮」） | 单次 |
| `SteerRegistry::register` | 异常 | 同键覆盖：先 `register("s1", tx1)` 再 `register("s1", tx2)` → 其后 `request` 只落 `tx2`（`rx2` 收到、`rx1` 收 `Err(Disconnected)`），旧接收端不再收到任何项 | 单次 |
| `SteerRegistry::register` / `SteerRegistry::request` | 正向 | 多键隔离：`s1` / `s2` 各自登记 → `request("s1")` 只落 `rx1`（`rx2.try_recv` 为 `Empty`）；随后 `request("s2")` 落 `rx2` | 组合迭代（2 键 × 2 序） |
| `SteerRegistry::register` / `SteerRegistry::request` | 边界 | 键边界：`""` / `"   "` / `"a\0b🚀"` / `"字".repeat(1200)` 四形态 → 键不 trim 不截断，同串 `request` 命中；近邻键（`""` vs `" "`）互不命中 | 参数化迭代（4 形态） |
| `SteerRegistry::request` | 边界 | 正文边界（零加工）：`text` 为 `""` / `"   "` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → 接收端 `text` 与入参逐字相等（表零校验、零 trim、零截断） | 参数化迭代（5 形态） |
| `SteerMessage.ack` | 边界 | ack 单次语义（AC-5 的类型承载）：`send(())` 后等待侧得 `Ok(())`；二次 `send` 返 `Err`；ack 未被消费即 drop → 等待侧 `RecvError`（drop-Err 机制面，零超时） | 参数化迭代（3 形态） |
| `SteerRegistry::request` | 边界 | 并发取用原子性：同键 `register` 后两线程并发 `request` → 两者均 `Ok`（无丢失、无 panic），接收端收到恰两条 | 单次 |
| `SteerRegistry::register` / `remove` / `request` | 边界 | 并发读写不 panic：多线程混合 `register` 同键 / `remove` / `request` → 无 panic、无死锁；结束态结果二值可考（末次请求 `Ok` 或 `Err`） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面是进程内内存表 + 既有 `tokio::sync::{mpsc, oneshot}`；无 IO、无配置、无环境变量、无子进程 | 全章节 |
| 不 mock：`mpsc::unbounded_channel` / `oneshot::channel` | 真实通道对拍——miss 与 `send` 失败同臂、逐条 ack、drop-Err 三判据只能由真实通道的 `Disconnected` / `RecvError` 观测 | 全章节 |
| 不 mock：`Mutex` / `HashMap` | 真实容器（并发行需要真实锁语义，替身化会让原子性断言空转） | 并发两行 |

### `packages/desktop/src-tauri/crates/core/agent/src/ask.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/ask_test.rs`

**既有 sidecar 追加**：只追加 `is_pending` 相关用例；`register` / `answer` / `remove` 的既有用例（take 语义、同键覆盖、除名幂等、并发、键与正文边界）**零改动通过**即回归判据。

#### 待测功能

- `AskRegistry::is_pending(&self, session_id: &str) -> bool`: 只读判据（`slots.contains_key`，O(1)，取锁即释放）——ask 阻塞窗口的唯一真相（此刻 `SteerRegistry` 条目仍在场，条目在场不能证明可送达）；不改 `register` / `answer` 的 take 语义。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AskRegistry::is_pending` | 正向 | 登记态：`register("s1", tx)` → `is_pending("s1") == true`；与同键 `SteerRegistry` 条目并存（ask 阻塞窗口形态） | 单次 |
| `AskRegistry::is_pending` | 正向 | `answer` 后翻转：`register` → `answer("s1", "选项A")` → `is_pending == false`；`answer` 的受理返回值口径零改动（首次 `Ok`） | 单次 |
| `AskRegistry::is_pending` | 正向 | `remove` 后翻转并可重开：`register` → `remove` → `false`；同键再 `register` → `true` | 单次 |
| `AskRegistry::is_pending` | 异常 | 未登记形态：空表 / 从未登记键 / `""` / `"   "` / 超长键 → 恒 `false`，不 panic | 参数化迭代（5 形态） |
| `AskRegistry::is_pending` | 边界 | 只读不改表：连续两次 `is_pending` 皆 `true`，其后 `answer` 仍命中（只读、不 take、不消费，取锁即释放） | 单次 |
| `AskRegistry::is_pending` | 边界 | 同键覆盖形态：同键 `register` 两次（第二次返回值 `false`）→ `is_pending` 恒 `true`（诊断值差异不影响判据） | 单次 |
| `AskRegistry::is_pending` | 边界 | 两实例隔离：两 `AskRegistry` 各自登记不同键 → 互不命中（无全局静态串表） | 单次 |
| `register` / `answer` / `remove` | 异常 | 「既有…」三方法体逐字不动：take 语义、同键覆盖、除名幂等、键 / 正文边界、并发取用共 13 条既有用例零改动全绿（`git diff` 出现三方法体 hunk 即越界） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 与既有 `ask_test.rs` 同装置：进程内内存表 + 真实 `oneshot` | 全章节 |
| 不 mock：既有装置 | 既有 `register_waiter` 辅助与真实通道原样复用（新用例不改装置形态） | 全章节 |

### `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs`

<!-- design.md 未声明该文件的公共函数 / API 变更（「公共函数 / API」无本文件行）：本文件只加 `AgentSession` 第四字段
  `steers`（「类型定义」行的加位），无新增/修改的方法签名。字段面的编译闭合由 7 处字面构造随动兜住（含生产构造点），
  语义半边归 kernel_test.rs（登记用的 sender 即该字段）与 sdk/loop_test.rs（接收端 drain）。
  runner_test.rs 只做构造点补位（夹具随动），不新增用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释（字段语义由 kernel_test.rs / sdk/loop_test.rs 承接） -->

#### Mock策略

<!-- 无 -->

### `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/kernel_test.rs`

**既有 sidecar 改造 + 追加**：`SessionKernel::new` 加第 5 参（52 处调用点夹具随动）；登记 / 除名断言新增。盖戳单点、状态机与 `drive` 的收口缝语义**零改动**（既有用例零改动通过即回归判据）。

#### 待测功能

- `SessionKernel::new(sink, registry, streams, ask_registry, steer_registry: Arc<SteerRegistry>) -> Self`: 第 5 参注入会话级 steer 注册表（由组合根注入，MUST NOT 内部自铸）；登记 / 除名落在 `begin_turn` / `converge`（与 `StopRegistry` / `AskRegistry` 同址）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionKernel::new` / `begin_turn` | 正向 | 登记（AC-1 的内核半边）：`new(..., Arc::clone(&steers))` → `begin_turn` 成功 → 测试持有的**同一** `Arc<SteerRegistry>` 上 `request(&running.session_id, msg)` → `Ok`（键 = `RunningTurn.session_id`） | 单次 |
| `SessionKernel::new` / `begin_turn` | 正向 | 键形态：`SessionRef::New` → 登记键为 `ses-` 前缀 core 铸 id（非引擎侧 `sdk-` 形态）；`SessionRef::Continue{id}` → 登记键 = 引用 id（同会话同键） | 参数化迭代（2 形态） |
| `converge` | 正向 / 异常 | 两收敛态除名（AC-3 / AC-6）：Completed（`TurnDone`）与 Stopped（停止置位）各走一遍 → 收敛后 `request` → `Err`「会话已收口…」，且 `StopRegistry.request_stop` 同键 miss（两表同一出口） | 组合迭代（2 收敛态） |
| `converge` | 边界 | 只除本会话键：两会话并行 `begin`（各持独立假 runner）→ 会话 A 收敛后 `request(A)` → `Err`、`request(B)` → `Ok`（按 core session id 隔离） | 组合迭代（2 会话 × 2 收敛序） |
| `SessionKernel::new` | 边界 | 实例不共享：两个 `SteerRegistry` 实例各建一 kernel → A 的 `begin_turn` 后 B 实例 `request` → `Err`（组合根注入的才是可送达的那张表） | 单次 |
| `begin_turn` | 异常 | 开轮失败不登记：假 runner `open` Err / 会话行写入失败两失败点 → `begin_turn` 返 `Err` → 该会话键零条目（`request` → `Err`，无残留） | 参数化迭代（2 失败点） |
| `begin_turn` / `converge` | 边界 | 登记序与既有同址：`registry.register` → `streams.begin` → steer 登记三步在位（`StopRegistry` / `SessionStreamRegistry` 查询面照常可用）；除名点唯一（`converge`），`request_stop` 与观测通道 EOF 两既有路径不额外除名 | 单次 |
| `SessionKernel::new` | 异常 | 「既有…」盖戳单点、状态机、`drive` 的「观测通道关闭即唯一收口缝」三处零改动：既有内核用例（含 ask 除名、停止、EOF 收敛）零改动全绿 | 单次 |
| ↔ 编译闭合 | — | 57 处 `SessionKernel::new` 调用补第 5 参（`kernel_test.rs` 52 / `lib_test.rs` 1 / `compose.rs` 2 生产 / `compose_test.rs` 2）；测试侧每用例一条显式 `Arc::new(SteerRegistry::default())`，不引入全局静态 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `SessionSink` 端口 | 既有手写 `FakeSink`（调用序记录 + 可编程失败，进程边界替身：避免真实 redb 库）；不新增替身 | 全部用例 |
| `AgentRunner` 端口 | 既有手写 `FakeRunner`（预录观察序列 + `close_stream` 单边 EOF + 可编程 `open` Err；替身：避免真实引擎与网络） | 登记 / 除名 / 失败臂 |
| 不 mock：`StopRegistry` / `SessionStreamRegistry` / `AskRegistry` / `SteerRegistry` | 四张表一律真实实例（被测面就是「同址同键空间」这一事实，替身化会让断言空转） | 全部用例 |
| 不 mock：假 runner 的 `AgentSession` | `steers` 用就地 `mpsc::unbounded_channel()` 的发送端（接收端就地 drop 即「无在途」，与生产语义一致） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` -> `packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs`

<!-- design.md 未声明该文件的公共函数 / API 变更（「公共函数 / API」无本文件行）：本文件只加 `mod steer;` 与
  `pub use steer::{SteerMessage, SteerRegistry};` 两行出口面（字母序定位），无新增行为。
  出口可达性由编译期机械锚定：`steer_test.rs` 以 `crate::steer::…` 导入、壳层以 `agent::SteerRegistry` 导入，
  任一名丢失即编译失败；既有 lib_test.rs 的「根导出面快照」骨架零改动。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释（出口面 = 编译锚）；既有用例只做 `SessionKernel::new` 第 5 参夹具随动 -->

#### Mock策略

<!-- 无 -->

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`

<!-- design.md 未声明该文件的公共函数 / API 变更：改动面是私有缝——`open_session` 内铸
  `mpsc::unbounded_channel::<agent::SteerMessage>()`、`session_pump` 新形参（紧跟 `questions`）与
  `AgentSession { steers: steer_tx }` 回填；`PumpPayload` / `EngineFacade` 零改动。
  判据是编译闭合（`AgentSession` 字面构造 7 处 + `loop::run` 第 4 实参）与两处行为侧证：
  通道语义归 sdk/loop_test.rs（接收端 drain）、登记语义归 kernel_test.rs（sender 落表）。
  runner_test.rs 只做 `open_session` 相关夹具随动，不新增用例（真机 provider 往返不属自动化验收面）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`

**既有 sidecar 改造 + 追加**：`loop::run` 加第 4 位置参数（20 处既有调用点补就地 drop 的接收端 ⇒ 零副作用、断言口径零改）；drain 行为行新增。装置复用既有 rig `test_utils` 流缝（`MockScript` / `MockStreamEvent` + 脚本化 transport + 请求捕获）。

#### 待测功能

- `loop::run(model, turn, history, steers, sender, handle) -> Vec<Message>`: 第 4 位置参数为 steer 接收端；主循环顶部（`stop_requested` 快检之后、`defend` 与发请求之前）`try_recv` **循环** drain，单条语义 = 独立密封事件 → `history.push(Message::user)` → `ack.send(())`（三步序不可换）；事件通道关闭即停止 drain 并 `return history`（剩余项不补 ack）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `loop::run` / drain | 正向 | 请求边界送达（AC-1）：脚本化流缝跑一次工具轮 → 在首个流起手钩子内推入一条补充说明（等价「流式中到达」）→ 下一边界 drain：事件流出现一条 `Message { role: User, blocks: [Text{text, citations: None}], parent_tool_use_id: None }`；`history` 尾部为「`tool_results` 消息 + 新的 `Message::user(steer)`」 | 单次 |
| `loop::run` / drain | 正向 | 不并入 `tool_results`（AC-1 硬约束）：断言 `tool_results` 那条消息正文**不含** steer 文本，且第二请求的 `chat_history` 中二者为**两条独立条目**、steer 排在 `tool_results` 之后（rig 0.43.0 不合并相邻同角色消息；并入会让 openai 侧 user 先于 tool → provider 400） | 单次 |
| `loop::run` / drain | 正向 | 一次边界三条不合并（AC-2）：同一边界前推入三条 → 一次 drain 取净：三条独立密封 user 事件、三条独立 `Message::user`、三次 `ack.send(())` 各自可得；后两条不滞留到下一边界（事件序 = 推入序，seq 单调） | 单次 |
| `loop::run` / ack | 正向 | 三步序（DD2）：接收侧在 ack 就绪后立即读 `history` → 该文本**已在场**（ack 先于入史即本行红）且密封事件亦已发出 | 单次 |
| `loop::run` | 异常 | 无在途零副作用：「既有…」既有 20 处调用点传就地 drop 的接收端（`try_recv` 立即 `Disconnected`）→ 事件序列与 `history` 与本变更前逐字一致；既有全部 loop 用例零改动全绿 | 组合迭代（2 形态：sender 已 drop / sender 在场零消息） |
| `loop::run` | 异常 | 停止截停先于 drain（AC-6）：在途一条 + `handle.request_stop()` 先置位 → 轮 `return history`、零合成 `TurnDone`；事件流无该 text 的 user 密封事件、`history` 无该 `Message::user`；接收端 drop 后 ack 收 `RecvError` | 单次 |
| `loop::run` / drain | 边界 | 事件通道关闭即停（DD2 返回 `false`）：消费端先行 drop + 在途一条 → loop 立即 `return history`（不悬挂），剩余项不补 ack（其 ack 随接收端 drop，等待侧 `RecvError`） | 单次 |
| `loop::run` | 异常 | 不在 ask 阻塞臂 drain（D5）：`ask = Some(_)` 的轮模型挂 `AskUserQuestion` 阻塞期间推入补充说明 → 该窗口零 drain（无该 text 的 user 密封事件）；应答后工具结果入史 → **下一边界**才 drain | 单次 |
| `loop::run` | 异常 | 收口即终点（AC-3 的引擎半边）：收口（`loop::run` 返回）后再推入 → 零事件、零入史（不再有 drain 机会），ack 随接收端 drop 收 `Err` | 单次 |
| `loop::run` | 边界 | 正文边界（零加工）：`text` 为 `""` / `"   "` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` 五形态 → 密封事件 `Text.text` 与 `Message::user` 正文与入参逐字相等（零 trim、零截断），事件序不受长度影响 | 参数化迭代（5 形态） |
| `loop::run` | 边界 | 跨边界不混序：第一边界两条 + 第二边界一条 → 两次 drain 各自独立取净：事件与 hist 依推入序追加三条，无一条被吞、无一条重复 | 组合迭代（2 边界 × 3 条） |
| `loop::run` | 异常 | 一轮一命护栏（AC-7）：含 supplement 的整轮内 `RunStarted` 恰一条、`TurnDone` 恰一条（停止形态零合成）、user 密封事件恰「提问 1 条 + 补充说明 n 条」（steer 不伪装成第二条提问） | 单次 |
| `loop::run` / `defend` | 边界 | 不为 steer 开免裁特权（`R5` 已知代价）：超水位 `history` + 在途补充说明 → 防线按既有规则裁决（尾部追加照旧、无 steer 专用分支）；密封事件已在事件流（转录原文可核对） | 单次 |
| ↔ 编译闭合 | — | 21 处 `loop::run` 调用点补第 4 实参（`sdk/loop_test.rs` 20 / `sdk/runner.rs` 1 生产）；既有 20 处断言口径零改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / provider 端点（进程边界） | 既有 rig `test_utils` 流缝装置：`MockScript` + `MockStreamEvent` + 脚本化 `Transport`（`FakeModel` / `FakeState`，可预录流事件、捕获请求、注入悬挂 / 故障 / 工具调用）；**不触真实网络** | 全部用例 |
| 工作目录 | `tempfile` 真目录作 `cwd`（真实文件系统——preamble / protect 读面需要真实文件；不 mock `std::fs`） | 全部用例 |
| 事件通道 / steer 通道 | 真实 `mpsc::channel(256)`（事件）+ 真实 `mpsc::unbounded_channel`（steer）+ 真实 `oneshot`（ack）；**不 mock** | 全部用例 |
| 不 mock：内部模块 | `defend` / `context` / `compact` / `protect` / `policy` / `sandbox` / `tools` / `mount` 全部真实组合（steer 与它们的关系是「尾部追加一条 Message」，替身化会掩盖 prune / 序列化面差异） | drain 与防线相邻行 |
| 接收端就地 drop | 既有 20 处调用点以 `mpsc::unbounded_channel::<agent::SteerMessage>().1` 传空接收端（sender 就地 drop ⇒ `try_recv` 即 `Disconnected` ⇒ 零副作用）；drain 新用例持 sender 推消息 | 既有调用点与「无在途」行 |

### `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`

**既有 sidecar 改造 + 追加**：`compose_turn` 加实参（紧跟 `ask_registry`，41 处调用点夹具随动）；新实参透传断言新增。`assemble_injections` / `resolve_agent_engine` / `map_protocol` / `ComposedTurn::begin` 零改动。

#### 待测功能

- `compose_turn(stores, registry, streams, ask_registry, steer_registry, store, agent, workspace_root, ask_port) -> Result<ComposedTurn, String>`: 新实参紧跟 `ask_registry`（两张会话级注册表成组）；只透传内核，引擎门面不参与。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `compose_turn` / `begin` | 正向 | 透传同一实例（AC-1 的组合行）：以测试持有的 `Arc<agent::SteerRegistry>` 传入 → `composed.begin(...)` 返 `RunningTurn` → 同一实例 `request(&running.session_id, msg)` → `Ok`（非自铸的第二张表） | 单次 |
| `compose_turn` / `begin` | 正向 | 生命周期闭合（AC-3 的组合行）：`begin` 后驱动至收敛（不可达 provider 快失败）→ 同一实例 `request` → `Err`（`converge` 除名经注入表生效） | 单次 |
| `compose_turn` | 边界 | 实例隔离：两次 `compose_turn` 各传不同 `Arc` → 各自 `begin` 后只命中本表（互不串表） | 单次 |
| `compose_turn` | 异常 | 解析失败零接线：无默认 agent / provider 两成因 `Err` 的既有用例 → 注册表零触点（`request` 仍 `Err`），`Err` 文案零改动、零落库 | 参数化迭代（2 成因） |
| `compose_turn` | 异常 | 「既有…」门面零改动：`EngineFacade` / `PumpPayload` / 注入装配面（`SessionInjections` 四字段）既有用例零改动全绿 | 单次 |
| ↔ 编译闭合 + 夹具随动 | — | 52 处 `compose_turn` 调用补实参（`compose_test.rs` 41 / `worker_test.rs` 9 / 两生产点随阶段落地）；`composed_with_runner` 测试夹具补 `Arc::new(agent::SteerRegistry::default())` | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据根 / 库文件 | `tempfile` 真目录 + 真 `native_db`（既有 `DualEnv` 装置）；存储层不 mock | 全部用例 |
| provider 配置 | fixture 落库（真实配置读面），`base_url` 指向 `127.0.0.1:9` 不可达端口——用例断言不依赖网络结果（收敛只以 `request` 二值观测） | 生命周期闭合行 |
| 不 mock：`SessionKernel` / 四张注册表 | 真实组合（被测面就是「组合根注入的实例即内核登记的那张表」） | 全部用例 |

### `packages/desktop/src-tauri/src/commands/exec/mod.rs` -> `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`

**既有 sidecar 追加**：`app_with_stores` 增 `app.manage(Arc::new(SteerRegistry::default()))`（与 `stop` / `streams` / `ask` 同址）；`agent_steer` 六态 + 组合行新增。既有 `agent_answer` 四态与 watch / query 用例零改动通过。

#### 待测功能

- `agent_steer(steer: State<Arc<SteerRegistry>>, ask: State<Arc<AskRegistry>>, root: String, session_id: String, text: String) -> Result<(), String>`: 四段薄包装——blank 三参校验（各携字段名）→ `ask.is_pending` 判据 → `steer.request` 投递 → `ack.await`；三条不可送达文案各自独立；零新开会话 / 轮、零状态机、零广播、零排队（两个 `State` 先 `Arc::clone` 再 await）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `agent_steer` | 正向 | 命中（AC-8）：预置 `register("core-1", tx)` → 同任务 `tokio::join!` 驱动命令与接收端（收到消息后 `ack.send(())`）→ 命令返 `Ok(())`，接收端 `text` 逐字（`Ok` 时该文本必已入该轮史的口径由 loop 侧锁定） | 单次 |
| `agent_steer` | 异常 | blank root：`""` / `"   "` / `"\t\n"` → `Err` 含「非法 root」；注册表零触点（其后合法 root 仍可命中） | 参数化迭代（3 形态） |
| `agent_steer` | 异常 | blank session_id：`""` / `"   "` → `Err` 含「非法 session_id」；注册表零触点 | 参数化迭代（2 形态） |
| `agent_steer` | 异常 | blank text（DD9）：`""` / `"   "` / `"\n"` → `Err` 含「非法 text」；该文本不入通道（接收端 `Empty`） | 参数化迭代（3 形态） |
| `agent_steer` | 异常 | 校验次序：三参同时 blank → 报 root 臂；root 合法 + 后两 blank → 报 session_id 臂（先于 text）；六条文案互不归一 | 组合迭代（3 组合） |
| `agent_steer` | 异常 | ask 优先（AC-4）：同键 `AskRegistry::register` 在场 + steer 已登记 → `Err`「等待应答中，补充说明不可送达」；steer 通道零项；随后 `answer` 应答后接收端仍 `Empty`（**不补投、无排队**） | 单次 |
| `agent_steer` | 异常 | miss / 已收口（AC-3）：从未登记键 / `remove` 后 → `Err`「会话已收口，补充说明不可送达」；与 `send` 失败臂（接收端已 drop）文案逐字相同 | 参数化迭代（2 形态） |
| `agent_steer` | 异常 | ack 竞态（AC-5）：投递成功后接收侧 drop 该 `SteerMessage`（不回 ack）→ 命令返 `Err`「未送达（会话在补充说明生效前结束）」；用例内零计时器（瞬时返回，无超时代偿） | 单次 |
| `agent_steer` | 异常 | 六文案可辨：三态 + 三 blank 的错误串两两不等，且各含成因关键词（成因 + 用户下一步） | 参数化迭代（6 形态对拍） |
| `agent_steer` | 异常 | 零副作用（AC-8）：命中与三态失败各跑一遍 → 库面无新会话行 / 新轮行 / 新事件（`sessions_in_root` 计数不变）；命令体零 `agent_start` / `agent_stop` / `agent_session_watch` 触点、零广播 | 组合迭代（2 路径 × 4 观测面） |
| `agent_steer` | 边界 | 多会话隔离：两会话各登记 → 对 A 投递只落 A 的接收端，B `Empty` | 组合迭代（2 会话 × 2 序） |
| `agent_steer` | 边界 | 正文零加工：`text` = `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → 接收端逐字相等（命令体不 trim、不截断；仅 blank 被拒） | 参数化迭代（3 形态） |
| `agent_steer`（组合行） | 正向 | 命令 → 注册表 → 通道 → ack 全链：`app_with_stores` 托管真实注册表（与 `main.rs` 同型 `manage`）→ 预置发送端（`AgentSession.steers` 形态）→ 命令投递 → 接收侧完成「密封入史 + ack」→ 命令 `Ok`；同一实例两次调用可并发完成（三段各自块内取放，不跨 await 持锁 / 持 `State`） | 单次 |
| `agent_steer`（组合行） | 异常 | 不触引擎：命中与失败两路径均零 provider 依赖（无 SDK 配置 / 无网络即可完成），命令体为四段薄包装（design「三件事纪律」+ 第三节 ack） | 组合迭代（2 路径） |
| `agent_steer` | 边界 | 装配面锚定：`app_with_stores` 的 `manage(Arc::new(SteerRegistry::default()))` 与 `main.rs` 同址同型；`agent_start_with` 的 `compose_turn` 调用面（`ask_registry` 之后）编译通过 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri 应用装配 | 既有 `tauri::test::mock_app()` + `app.manage(...)`（进程边界替身：真实 runtime 不启动），四张注册表按 `main.rs` 同型托管 | 全部用例 |
| workspace 库 | `tempfile` 真目录 + 真 `WorkspaceStores`（既有 `Env` 装置）；库面计数用既有 `sessions_in_root` 观测 | 零副作用行 |
| steer 通道 / ack | 真实 `mpsc::unbounded_channel` + 真实 `oneshot`（被测链路本体，**不 mock**）；命令与接收端以 `tokio::join!` 同任务并发驱动（不 spawn 跨 `State` 借用） | 命中 / 竞态 / ask 优先行 |
| 不 mock：`SteerRegistry` / `AskRegistry` | 真实实例（被测面） | 全部用例 |

### `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` -> `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`

<!-- design.md 未声明该文件的公共函数 / API 变更（「公共函数 / API」无本文件行）：改动面是把 `steer_registry`
  句柄取法与实参补进 `change_flow_start_with` 的 `compose_turn` 调用（1 处，与 `ask_registry` 同址同式），
  `ChangeFlowControl` / sink / walker 零改动。行为面归 compose_test.rs（第 5 参透传）与 kernel_test.rs（登记 / 除名）；
  本文件只做调用点夹具随动，不新增用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释（仅 `compose_turn` 实参面夹具随动） -->

#### Mock策略

<!-- 无 -->

### `packages/desktop/src-tauri/src/commands/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

**既有 sidecar 改造**：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 各补一条（`agent_steer` / `agentSteer`，紧随 `agent_answer`）；条目数 50 → 51；`DTO_TYPES` 清单零补录（`SteerMessage` / `SteerRegistry` 不出线，只出命令包装）。

#### 待测功能

- `all_commands!` 登记面（单一登记清单）: `$crate::commands::exec::agent_steer` 紧随 `agent_answer` 登记（`commands/exec` 轨道）；`bindings:export` 产物含 `agentSteer` 包装。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `all_commands!` / 两份清单 | 正向 | 命令补录（AC-8）：`COMMAND_NAMES` 含 `"agent_steer"`、`COMMAND_WRAPPERS` 含 `"agentSteer"`；两份清单条目数**同时** 50 → 51 且持衡（漏登记或重复登记即红）；首锚仍 `list_changes`、尾锚仍 `read_file` | 单次 |
| 生成序 | 正向 | 序断言：`agentSteer` 在产物中的偏移紧随 `agentAnswer`（`all_commands!` 登记序 = 生成序，沿既有相对序先例） | 单次 |
| 产物 `agentSteer` | 正向 | 签名出线：产物内 `agentSteer: (root: string, sessionId: string, text: string) => Promise<null>`（三参逐字、返回 `null` = `Result<(), String>` 的出线形态） | 单次 |
| 产物 / 反向锚定 | 异常 | 引擎面不出线：产物全文零 `"SteerMessage"` / 零 `"SteerRegistry"` 类型名（`DTO_TYPES` 清单零补录即覆盖；新增类型名会让既有反面断言红） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 静态清单断言 + 读盘产物快照（既有装置）；无进程边界 | 全章节 |

### `packages/desktop/src/hooks/use-agent-session.ts` -> `packages/desktop/src/hooks/use-agent-session.test.ts`

**既有 sidecar 追加**：`UseAgentSessionState` 加 `steer` / `steering`；`applyUserQuestion` 分化为「首条归一 / 其余追加」；`error` 保序链扩为四源。既有 `answer` / `stop` / `sendMessage` / 重放 / 订阅调度用例**零改动通过**。

#### 待测功能

- `useAgentSession(params: UseAgentSessionParams): UseAgentSessionState`: 返回面加位 `steer` / `steering`；`error` 四源保序合并（重放 > 流 > 动作）；`steer` 提交即置 `steering = true`、成功与 `Err` 两收尾均复位。
- `UseAgentSessionState.steer(text: string): void`: `agent_steer(root, session_id, text)` 薄包装；`root` / `sessionId` 为 `null` 时 MUST NOT invoke；`Err` 进既有错误面。
- `UseAgentSessionState.steering: boolean`: 「已提交、等 ack 回流」的 pending 态（不引入计数）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `state.steer` | 正向 | 三参逐字（AC-11）：`steer('先跑 lint')`（`root` / `sessionId` 在场）→ `agent_steer` 恰一次，参数 `{ root, sessionId, text }` 逐字；同批零 `agent_start` / `agent_stop` / 零重取 | 单次 |
| `state.steer` | 异常 | null 守卫：`root = null` / `sessionId = null` / 两者皆 null → 零 invoke、`steering` 保持 `false`（无副作用） | 参数化迭代（3 形态） |
| `state.steering` | 正向 | 置位与成功复位：提交后、命令未 resolve 期间 `steering === true`；resolve 后 `false` | 单次 |
| `state.steering` / `state.error` | 异常 | `Err` 复位与错误面：命令 reject（「会话已收口…」）→ `steering` 复位 `false`、`error` 含该文案（不静默吞） | 单次 |
| `state.error` | 异常 | 四源保序：重放错误与 steer 错误并存 → `error` 取重放错误（更权威者优先）；重放错误消失后 steer 错误可见 | 组合迭代（2 源 × 2 顺序） |
| `messages`（首条 user） | 正向 | 首条归一（AC-9 的正向半边）：发起流本地副本 + 该轮首条 user 密封事件（`seq = 3`）→ 原地换 id 为 `turn-<turnId>-user`、消息数不增、位置不变（既有两条用例零改动） | 单次 |
| `messages`（轮中段补一句） | 正向 | 不被吞（AC-9）：该轮提问已在场 + 第二条 user 密封事件（`seq = 9`）→ 追加 `evt-9` user 气泡（内容为该事件 text 块），消息数 +1，位置在提问之后、该轮 assistant 之前 | 单次 |
| `messages`（轮中段补一句） | 异常 | 反向判据：`turn-<turnId>-user` 已在场时再收第二条 user 事件 → **仍追加**（不 return、不去重丢弃）；提问气泡恒一条（无第二条 `turn-<turnId>-user`） | 单次 |
| `messages`（旁观 / 重挂路） | 边界 | 判据来源（DD4）：无本地副本（订阅流起点）收到该轮**首条** user 事件 → 按提问处理（`turn-<turnId>-user`，不误判为补充说明）；随后第二条 → `evt-<seq>` 追加 | 单次 |
| `messages` | 边界 | 三条补一句：同轮三条 user 密封事件（`seq = 3/9/11`）→ 提问 1 条 + 补充说明 2 条（`evt-9` / `evt-11`），按到达序 | 单次 |
| `messages` | 边界 | 重投幂等：同一 `evt-<seq>` 事件重投两次 → 只一条气泡（按自身 id 幂等，不重挂） | 单次 |
| `messages` | 正向 | 收口同形（AC-10 的实时半边）：含补充说明的轮收口（`Record` + 重放）→ 重建后仍为 提问 → 补充说明 → assistant 三段，id 与实时路同源 | 单次 |
| `state.steer` | 边界 | 正文边界：`text` = `""`（前端不 trim、不预判，由后端 `Err` 说话）/ `"a\nb"` / `"字".repeat(1200)` → 逐字进 invoke 参数 | 参数化迭代（3 形态） |
| `messages` | 边界 | `turnId = null` 旧转录：user 事件 → 按提问路径处理（id 兜 `evt-<seq>`），不产生补充说明分支 | 单次 |
| `state.steer` | 边界 | 切会话不串投：提交后 `sessionId` 变更 → 不投到新会话；`steering` 不永久挂起（复位路径与两收尾一致） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界） | 既有 `vi.hoisted` 装置：`invoke` 按命令名可编程 / 可计数（`session_detail` / `agent_session_transcript` / `agent_session_watch` / `agent_start` / `agent_stop` / `agent_answer` / `agent_steer`）+ `Channel` mock 直投信封 | 全部用例 |
| 不 mock：`TauriAgentTransport` / 适配层 / `@ai-sdk/react` `useChat` | 真实组合（hook 即链路入口；重放与折叠面差异只能由真实 reducer 观测） | 全部用例 |
| 不 mock：镜像与轮行状态 | 真实 `useSessionMirrors` 状态机（`steering` 与 `error` 保序都在其上成立） | 全部用例 |

### `packages/desktop/src/lib/agent-adapter.ts` -> `packages/desktop/src/lib/agent-adapter.test.ts`

**既有 sidecar 追加。** 说明：design「公共函数 / API」子表未单列本文件行，但 design「变更清单」明写本文件 `entryToMessages` 的 user 分支改「首条 → 提问 / 其余 → 补充说明」与 `foldSessionMessages` 的按轮产出面细化（AC-10 落点）——`entryToMessages` 为私有件，其行为经公共入口 `foldSessionMessages` 观测；`assistantMessage` 的 user 跳过逻辑与 `foldSessionMessages` 分组 / 配对 / 发射零改动。

#### 待测功能

- `foldSessionMessages(events: AgentEvent[], turns: TurnSummary[]): UIMessage[]`: 每轮产出由「提问（组内**首条** `role = user`）+ 恰一条 assistant」细化为「提问 + **补充说明（组内其余各一条，id `evt-<seq>`，按 seq 序排在提问之后、assistant 之前）** + 恰一条 assistant + 逐事件 aux」。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `foldSessionMessages` | 正向 | 轮中段呈现（AC-10）：user(`seq = 3`) → assistant 密封块 → user(`seq = 9`) → assistant 密封块 → `TurnDone` + 轮行 → 产出三条对话消息：提问（`turn-<turnId>-user`）→ 补充说明（`evt-9`）→ assistant（`turn-<turnId>`）；aux 逐事件独立不变 | 单次 |
| `foldSessionMessages` | 正向 | 补充说明 parts：`parts` = 该 user 事件 text 块直映射（文本逐字、`state: 'done'`），与提问消息同一表达式产出（DD5 单点复用，不新开「只收 text 块」分支） | 单次 |
| `foldSessionMessages` | 正向 | id 族不撞：一轮一条提问 + 两条补充说明 → 三条 user 消息 id 两两不等（`turn-<turnId>-user` / `evt-<seq₁>` / `evt-<seq₂>`），assistant 恰一条 | 单次 |
| `foldSessionMessages` | 异常 | 补充说明不入 assistant（反向判据）：assistant 的 `parts` 不含补充说明文本（user 跳过逻辑零改动），且 assistant 仍为该轮**末条** | 单次 |
| `foldSessionMessages` | 异常 | 重放不丢：同一转录连续折叠两次 → id 序列与消息序逐字一致（零漂移、零重复、零丢项） | 单次 |
| `foldSessionMessages` | 边界 | 旧转录（`turnId` 全 null）：组内多条 user 事件 → 组内首条为提问（id 兜 `evt-<seq>`）、其余为补充说明（id `evt-<seq>`），全部两两不等且不与 assistant 兜底 id 相撞 | 单次 |
| `foldSessionMessages` | 边界 | 零 / 单 user 形态：单条 user 事件 → 仅提问、零补充说明（既有形状零变）；零 user 事件 → 零 user 消息、assistant parts 可为空 | 参数化迭代（2 形态） |
| `foldSessionMessages` | 边界 | 正文边界：补充说明 text = `""` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → parts 文本逐字保留（不截断、不转义） | 参数化迭代（4 形态） |
| `foldSessionMessages` | 边界 | 多轮隔离：两轮各一条补充说明 → 各自落在本轮的提问与 assistant 之间（不跨轮、不串序） | 组合迭代（2 轮 × 2 位置） |
| `foldSessionMessages` | 边界 | 插入锚点（DD6）：组内 user 事件的 seq 与 assistant 块交错（补充说明在中间）→ 补充说明仍排在提问之后、assistant 之前 | 单次 |
| `foldSessionMessages` | 异常 | 「既有…」零改动面：assistant 恒末条、aux 逐事件独立、跨消息工具配对、视野差异、未知事件透传等既有用例零改动全绿 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 纯函数模块（只依赖类型），无 IO、无进程边界 | 全章节 |
| 不 mock：`plansToParts` / `planBlocks` / `PairingScope` | 真实组合（DD5 要求复用同一单点；替身化会让「两路同表达式」这条判据空转） | 全部用例 |

### `packages/desktop/src/components/agent/agent-session-view.tsx` -> `packages/desktop/src/components/agent/agent-session-view.test.tsx`

**既有 sidecar 追加**（additive）：`AgentSessionActions` 加 `steer` / `steering`；既有 `running` / `loading` / `send` / `stop` 与 `AgentMessages` 透传零改动。

#### 待测功能

- `AgentSessionActions.steer: (text: string) => void`: 由 `state.steer` 组装后进 composer render prop（composer 插槽动作面加位）。
- `AgentSessionActions.steering: boolean`: 由 `state.steering` 组装（供插槽呈「排队中…」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentSessionActions.steer` | 正向 | 注入形态组装：注入 `session.steer` → composer render prop 收到 `actions.steer === state.steer`（引用相等）、`actions.steering === state.steering`（值透传） | 单次 |
| `AgentSessionActions.steer` | 正向 | 自持形态组装：`root` + `sessionId` 自持 → composer 收到 `steer` 为函数、点击后走 `agent_steer`（`root` / `sessionId` / `text` 逐字） | 单次 |
| `AgentSessionActions.steering` | 边界 | 变化即时透传：`steering` `true` → `false` 两次渲染 → 插槽各收一次新值（不缓存首次值、不停留在 `false` 初始态） | 单次 |
| `AgentSessionActions` | 正向 | 「既有…」四字段零变化：`running` / `loading` / `send` / `stop` 的既有插槽用例零改动通过（additive 不改既有语义） | 单次 |
| `composer` 插槽 | 异常 | 不传 `composer` 的消费点零影响：无发送输入、无停止按钮、零动作面接线（既有用例零改动） | 单次 |
| `composer` 插槽 | 边界 | 空态：`sessionId === null` 自持形态 → 空态占位呈现、`composer` 未被调用（slot 零渲染） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（自持形态进程边界） | 既有 `vi.mock` + `ChannelMock` / `invokeMock`（自持 hook 走真实链路） | 自持形态行 |
| 不 mock：`AgentMessages` / `AgentSessionView` 本体 | 真实组合（插槽内容与消息区共存关系是被测面） | 全部用例 |
| 不 mock：注入形态的 `session` | 直接构造 `UseAgentSessionState` 形参（引用相等判据需要同一函数引用） | 注入形态行 |

### `packages/desktop/src/components/agent/agent-input.tsx` -> `packages/desktop/src/components/agent/agent-input.test.tsx`

**既有 sidecar 追加**：`AgentInputProps` 加两个可选位（皆缺省 `false`）；默认变体（两位缺省）渲染与交互**逐字不变**（既有用例零改动通过）。

#### 待测功能

- `AgentInputProps.hidePermissionMode?: boolean`: 缺省 `false`；`true` 时不呈 permission-mode 档位行（`onSend` 的 `permissionMode` 位保留但下游 MUST NOT 消费）。
- `AgentInputProps.pending?: boolean`: 缺省 `false`；`true` 时发送入口禁用 + 呈可见「排队中…」文案（`data-testid="<idPrefix>-pending"`），输入框与停止入口不受影响。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `hidePermissionMode` | 正向 | `true` → 档位行缺席（`<idPrefix>-permission-mode` 为 null）；输入框 / 发送 / 停止照常呈现 | 单次 |
| `pending` | 正向 | `true` → 发送按钮 `disabled`、`<idPrefix>-pending` 文案「排队中…」在场（可见文案而非仅 tooltip，DD7 的可观测形态） | 单次 |
| `pending` | 正向 | 输入与停止不受影响：`pending = true` 且 `running = true` → textarea 可编辑（`fireEvent.change` 生效）、`<idPrefix>-stop` 在场可点 | 单次 |
| `pending` | 边界 | 复位即恢复：有 prompt + `pending` → 禁发；`pending` 置 `false` 后同一 prompt 立即可发（无残留禁用态） | 单次 |
| `pending` | 边界 | 与 `disabled` 并存：`disabled = true` + `pending = true` → 发送禁用（两成因叠加不产生第二句文案），`disabled-reason` 行与 `pending` 行各自独立呈现 | 组合迭代（2 组合） |
| `pending` | 异常 | 快捷键路径同禁：`pending = true` 时 `Ctrl+Enter` 不触发 `onSend`（与点击路径同一 `canSend` 合成式） | 单次 |
| `hidePermissionMode` | 边界 | 显式 `false` 与缺省等价：档位行在场（缺省 `false` 的类型口径） | 单次 |
| `hidePermissionMode` | 边界 | 参数位保留：`hidePermissionMode = true` 时 `onSend` 仍携 `permissionMode`（缺省 `bypassPermissions`），类型面零改动、下游不消费 | 单次 |
| 默认变体 | 正向 | 「既有…」零变化：两位不传 → 档位行在场、空串禁发、`running` 呈停止、`disabled` 整面板禁用 + 理由文案（既有用例零改动全绿） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 纯渲染组件，回调经 props 注入，无进程边界 | 全章节 |
| 不 mock：`StandardSelect` / `Textarea` / `Tooltip` | 真实组件族（「档位行是否呈现」与「发送禁用」必须由真实 DOM 观测） | 全部用例 |

### `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` -> `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx`

**既有 sidecar 追加**（首个 steer 消费点）。说明：design「公共函数 / API」子表未单列本文件行，但 design「变更清单」与 desktop-change-flow-view「节点会话转录联动」Module Contract 明写本面板增 composer 插槽（AC-11 落点）——`composer` 是 `AgentSessionView` 的既有公共插槽，本面板只接线。

#### 待测功能

- `SessionTranscriptPanel(props)`: 自持形态 `AgentSessionView` 增 `composer` 插槽——`AgentInput`（`idPrefix = "step-session"`、`hidePermissionMode`、`pending = actions.steering`、`running = actions.running`、`onSend = ({ text }) => actions.steer(text)`、`onStop = actions.stop`）；`roleRef` 缺席 / `sessionId === null` 仍空态且**不渲染 composer**；`askAnswer` 注入位照旧。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| composer 插槽 | 正向 | 在场形态：`roleRef.sessionId` 在案 → `step-session-composer` / `step-session-prompt` / `step-session-send` 在场；`step-session-permission-mode` **缺席**（`hidePermissionMode`） | 单次 |
| composer 插槽 | 正向 | `onSend` → `actions.steer`（AC-11）：输入文本 + 点击发送 → `agent_steer` invoke 三参逐字（`root` / `sessionId` / `text`）；零 `agent_start`（本面板不发起新轮） | 单次 |
| composer 插槽 | 正向 | `onStop` → `actions.stop`：库态 `running`（轮行 running）→ `step-session-stop` 在场，点击 → `agent_stop` 参数恰 `{ root, sessionId }` | 单次 |
| composer 插槽 | 正向 | pending 联动：ack 未回流（`agent_steer` 未 resolve）期间 → 发送禁用 + `step-session-pending` 呈「排队中…」；resolve 后复位可发 | 单次 |
| composer 插槽 | 异常 | 零预判判据：该步已收口（轮行全收口，`running = false`）→ 发送入口**未被禁用**，点击仍 invoke `agent_steer`；后端 `Err`（「会话已收口…」）呈现在会话错误面（`agent-session-error`） | 单次 |
| composer 插槽 | 异常 | 空态不渲染 composer：`roleRef === null`（「当前选中无关联会话」）与 `roleRef.sessionId === null`（「此步无会话记录」）两形态 → 空态文案在场、`step-session-composer` 为 null、零 invoke | 参数化迭代（2 空态） |
| composer 插槽 | 正向 | 与 askAnswer 互不前置：传 `askAnswer` + 插槽 → 探针（`importOriginal` 保真）见 `viewProps.askAnswer` 引用相等，且 `composer` 同时在场；既有 askAnswer 用例（透传 / 引用相等 / 两动作面独立）零改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界） | 既有 `vi.mock` + `ChannelMock` / `invokeMock`（自持 hook 全链真实） | 全部用例 |
| `AgentSessionView` props 探针 | 既有装置：`vi.mock('@/components/agent/agent-session-view', importOriginal)`——保留真实实现，只把实收 props 记入 `propsLog`（引用相等判据无法由 DOM 反推）；**不新增产品导出** | aidAnswer 同源行、`composer` 是否透传 |
| 不 mock：`useAgentSession` / `AgentMessages` / `AgentInput` | 真实组合（composer 接线与气泡呈现是被测面） | 全部用例 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/src-tauri/src/main.rs`（`use agent::{…, SteerRegistry, …}` + `app.manage(Arc::new(SteerRegistry::default()))` 一行） | `setup_hook` 是**进程启动路径**，`main.rs` 是 bin 目标、不参与 `cfg(test)`（既有仓库无 `main_test` 先例）；托管态在生产运行期无测试入口 | `cargo clippy --all-targets` 编译闭合（`compose_turn` / `agent_steer` 取 `State<Arc<SteerRegistry>>`，缺 `manage` 即命令测试红）+ 各命令测试 `app_with_stores` 的同形态 `manage` 调用（同址同型）+ 桌面端 `git diff` 复核该行在位 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（生成物） | 由 `bindings:export` 产出、**禁手改**；写成测试会与生成链重复并随排版漂移 | `bindings:export` 后 `git diff --stat -- src/types/generated` 为空（桌面端执行）+ `src/bindings/mod_test.rs` 章节的产物快照断言（`agentSteer` 三参与生成序） | 
| `packages/desktop/package.json`（`version` 一行） | 配置 / 元数据面，无运行期行为可断言；判据是**相对递增**（落地时 `HEAD` 现值 +1 patch，MUST NOT 写死字面号） | 桌面端 diff 审查：`git diff HEAD -- packages/desktop/package.json` 的 `version` 行恰 +1 patch；`tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本行未动 |
| 编译闭合批的**纯机械点位**（`SessionKernel::new` 57 处 / `compose_turn` 52 处 / `loop::run` 21 处 / `AgentSession` 字面构造 7 处 / `AgentInputProps`·`AgentSessionActions` 既有使用面） | 只补实参 / 只补字段、只求可编译；design 明令**不新增用例、不改断言值、不改用例名**；缺字段即 `E0063` / `E0061`，无运行期中间态 | `cargo clippy --all-targets` 通过（`git diff --stat` 对这些文件显示为纯新增行）+ 各章节「↔ 编译闭合」「夹具随动」行只登记计数与点位 |
| AC-7 的**三处零改动结构事实**（`LoopTurn` / `PumpPayload` / `EngineFacade` 零 diff） | 「某结构未被改」是源码事实，运行期断言只能证行为等价，不能证逐字未改 | 桌面端 `git diff` 逐文件审查（design 明列三处为「逐字不动面」）+ `loop_test` / `compose_test` 既有用例零改动全绿（行为侧证） |
| AC-8 的**再生成零 diff** 与 `all_commands!` 的「单一登记面」 | 生成链幂等是工具事实；「只有一处登记」是缺席事实 | 桌面端 `bindings:export` + `git diff` 审查；`bindings/mod_test.rs` 计数 50 → 51 双向持衡即反向锚定 |
| `crates/core/agent/src/ask.rs` 的 `register` / `answer` / `remove` **三方法体逐字未改** | 「零 diff」是结构事实；既有用例全绿只证行为等价 | 桌面端 `git diff` 逐块比对 + `ask_test.rs` 既有 13 条零改动全绿 |
| `crates/infra/agent/src/sdk/loop.rs` 的 `execute_ask`、`crates/core/agent/src/kernel.rs` 的 `stamp` 单点 / 状态机 / `drive` 收口缝、以及 `crates/core/orchestration/**`、`crates/infra/store/**`、`crates/core/workflow/**`、工具面 / 防线 / 沙箱 / 保护 / 压缩 / 重建全套的**零改动**（design「不要修改」列表） | 均为「一行不变」的结构性主张（ask 阻塞臂未加 steer 臂、无 steer 工具、无新 store 列、16 份 prompt md 零改动） | 桌面端 `git diff --stat` 逐文件审查；行为侧证 = 相应既有测试零改动全绿（`loop_test` / `kernel_test` / `walker_test` / `store` 测试 / `mount_test` / `protect/**`） |
| `crates/core/agent/src/lib.rs` 的出口面（`pub use steer::{SteerMessage, SteerRegistry}`） | 纯再导出，无运行期分支；`steer_test.rs` 与壳层导入即编译锚 | `cargo clippy --all-targets` 编译可达 + 既有 `lib_test.rs` 根导出快照骨架零改动 |
| `worker_test.rs` / `runner_test.rs` 等**夹具随动**点（`AgentSession` 7 处 / `compose_turn` 9 处等） | 与「编译闭合批」同性质：补位不改语义 | 编译通过 + `git diff` 显示纯新增行 |
| 真机 E2E（真实 provider 端点下的「运行中补一句」往返） | proposal 明写「验收止于单元测试；真机 E2E 不属本工作流自动化验证面」 | `sdk/loop_test.rs` 的流缝脚本化四条路径（drain 入史 → 下个请求可见 / 多条逐条 ack / 停止截停 / 收口 Err）即本变更的全部行为面 |
| 机械门禁（`cargo clippy --all-targets` / `pnpm run check` 四段链 / `vp test` 全量 / `knip` / 版本行相对递增） | 机械门禁步骤，由桌面端 static-check / test-execution 相位执行；在设计里复述脚本细节等于复制运行脚本 | 门禁执行结果 + 本设计各章节对覆盖率的支撑（断言全部落在真实契约面：同臂 Err 口径 / 三步序 / 逐条 ack / 首条判据 / id 族 / 两可选位） |
| `.dev-team/architecture/models/**` 与 `openspec/specs/**` 的**零 diff**（`SteerRegistry` / `agent_steer` 只在 `apiFindings` 槽位留痕、不影响 `status`） | 「正确性主张」是这些文件一行不变 | 桌面端 `git diff --stat` 逐项审查；行为侧证 = 零新 crate / 零新依赖边（`Cargo.toml` 与 `package.json` 依赖段零 diff） |
