# 测试设计: slim-flow-aux-steps

> **变更**: slim-flow-aux-steps
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（问题四则 / 决策表 D1–D6 / 实施清单 A1–A6 + B1–B3 / 词汇冻结 / attempt 口径 / 变更范围实现文件 **6 行** + 测试文件 **9 行** / AC-1…AC-12 / R1–R11）、`design.md`（起点核实 / 口径校正六条 / D1–D12 / 架构组件 9 行 / 新增文件「无」+ 修改文件 6 行 / 公共函数·API「零公共 API 面变化」+ 内部函数签名 13 行 / 类型定义 8 行 / 配置 1 行 / 明确不做 5 条）、8 份 delta spec、`tasks.md`（五阶段 + 交接备忘）
> **被测范围**: design「变更清单 - 新增文件」= **无**；「修改文件」子表 **全 6 项**——`core/orchestration/src/walker.rs`、`core/orchestration/src/steps.rs`、`core/workflow/src/write/phase_table.rs`、`core/workflow/src/state.rs`（仅注释）、`core/orchestration/src/state.rs`（仅注释）、`packages/desktop/package.json`（版本行）。**删除文件零项、新增依赖零项、前端零改动、bindings 零 diff、store 零改动**。另按 proposal「测试文件」表与 `tasks.md` 交接备忘纳入既有集成语料文件 `crates/core/workflow/tests/corpus_golden_test.rs`（含 `tests/golden/*.json`）——它是 AC-11 / AC-12 的唯一执行面，非 design「修改文件」项（见该节注释）。
> **本变更性质**: 相位表辅助步（`PhaseStart` / `VerdictGate` / `PhaseLog`）退场与其内化动作落位——开相 → 固定工程动作 `enter_phase`、verdict 裁决 → evaluator 步内读 `finalized`、落账 → 固定动作 `settle_phase`；决策门节点（`WhitelistGate` / `RetryGate`）节点写面归零。**零 db 迁移、零 IPC 出线变化、零公共 API 面变化**（`walk_run` / `RunRequest` / `ToolCommand` / `ToolStepOutput` / 四个 runner port 签名逐字不动）。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/**`） | `cargo test`（crate 级 `-p orchestration` / `-p workflow`；`cargo nextest` 亦在基线 in-tree） | **sidecar 同目录同前缀** `*_test.rs`，源文件尾 / `lib.rs` 以 `#[cfg(test)] mod <name>_test;` 挂载（`orchestration/src/lib.rs:35-58` 的 12 条、`workflow/src/write/mod.rs` 的 11 条先例）；**本变更零新建测试文件**——**5 个既有 sidecar 改写 + 追加**（`walker_test.rs` 16816 行 / 186 用例、`steps_test.rs` 4980 行 / 60 用例、`phase_table_test.rs` 3522 行 / 76 用例、`workflow/src/state_test.rs` 1926 行 / 38 用例、`orchestration/src/state_test.rs` 1789 行 / 42 用例）+ **1 个既有 crate 级集成文件改写**（`workflow/tests/corpus_golden_test.rs` 3938 行 / 37 用例） | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为**中文短语**、断言消息载判决；装置为手写端口假件（`FakeWorker` / `FakeTools` / `FakeVcs` / `StubSnapshot` / `RecordingHistory` / `FakeStore` / `FakeRunner` / `FakeTestExecutionRunner`）与 `TestDb` / `tempfile` 真库；`.rs` 与 `.md` 源自证经 `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string`（`walker_test.rs:16402` / `finish_test.rs:72` 先例） |
| golden 快照面 | `cargo test -p workflow --test corpus_golden_test`（重写开关 `DESKTOP_GOLDEN_REWRITE=1`） | `tests/golden/*.json` 九份入仓快照 + `tests/fixtures/README.md` 覆盖面矩阵（文档面） | 每份语料跑 detail / list 全读链 → pretty JSON 与入仓 golden 全量对比；重写 → 复核两步流程 |
| 前端 / 生成物 / 配置文件 | — | **零测试路径**（`packages/desktop/src/**`、`src/types/generated/**`、`package.json` 均零改动或零行为面） | 登记于「不可测试项」 |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据、**缺席断言**）/ `边界`（空、空串、极值、恰好阈值、零元素集合、类型层不可表达形态）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有…」/「（回归）」标注**：该对象由既有用例锁定，本设计**只做回归登记**（断言零改动即绿，行为侧证）；未标注者为新增或按新形状改写（改写面仅限 proposal「改写原则」三行所列断言对象与其相邻说明，语义断言逐条保留）。

**断言改写原则（proposal「改写原则：改断言对象，不改断言意图」，执行面逐条落表）**

```
node_of(PhaseStart, 0) 找相位入口锚点        → 该相位首个真实步节点（node_of(<首真实步>, 0)）
step_instances(RunStepKind::WhitelistGate)  → step_instances(RunStepKind::Decision)
node.step == PhaseLog 的「已落账」断言        → phase_log_input(commands, phase).<字段> / pipeline attempts
「PhaseStart 开相」断言                      → active_phase 在案（真实 store）+ 零 PhaseStart 节点
```

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| int / number（`u32` attempt / `u64` token / `i64` 时间） | `attempt`：`0`（未开相 / `PhaseCursor::new` 中性值）、`1`（首 run）、`u32::MAX`（直构脏值，`PhaseLogOutcome.attempt` 原样回携不 panic）；**重入维度**：同相位连调两次 `enter_phase` → attempt 取同一值（写面条数未变即不抬升，见 `enter_phase` 边界行），落账连调两次 → 第二次 `Err` 且行数不变（见 `settle_phase` 边界行）；`-1` / `MAX_U64` 在类型层不可表达（越界形态以 `u64::MAX` usage 求和表达）；token：`None`（无 `TurnDone`）、单键缺席、两值在场、`u64::MAX`（`saturating_add` 不 panic） |
| str / string（`phase` / `change_id` / `item` / `report` / `evidence` / `what` / `detail`） | `""`、`"   "`、未知相位串、超长（>1000 chars）、特殊字符（`\n` / `\0` / emoji）、`None`（`Option<String>` 槽位）。`report` 另有恰 `MAX_REPORT_CHARS`(2000) 与 `+n` 两个截断阈值形态 |
| bool（`cursor.failed` / `skipped`） | `true` / `false` / 「未置位」（`PhaseCursor::new` 中性 `false`）三形态；`failed` 的回写单点（D7）逐形态断言 |
| list / array（`steps` / `checklist` / `transcript` / `report_paths` / `nodes` / `edges`） | `[]`（空 `steps` → `Terminal`；空 checklist → `derive_verdict([])=Pass`；空 transcript → token `None` + 「会话失败收敛」通用文案；`report_paths=[]` → 「（无报告产物）」）、单元素、超大（1000 元素不 panic） |
| dict / object（`finalized` / `PhaseFail` / `WorkerTurn` / `EvaluatorTurn` / `WorkerTurnOutcome`） | `finalized = None`（未收口）、`Some(Evaluator(..))`（合规）、`Some(Decision(..))`（evaluator 角色漂移）；`PhaseFail` 四字段必给（缺字段编译失败、`executor_session: None` 为合法缺省）；结构体字段集由编译器强制 |
| Optional[T] | `None` / `Some(..)` 各一（`finalized`、`executor_session`、`evaluator` 定义、`wait_answer` 应答） |
| Enum（`PhaseSettle` / `SettleMode` / `AgentRunStatus` / `FinishPayload` / `ChangeStepStatus`） | `PhaseSettle` 三形态 + 无第四形态（编译期封闭）；`SettleMode` 两值；`AgentRunStatus` 四值；`FinishPayload` 两值；`ChangeStepStatus` 四值（passed / failed / stopped / running）——每值至少一行 |
| float | **零 float 面**：本变更无 `f64` 入参 / 字段（`MutationTestOutcome.score` 既存面、非本变更断言对象） |

**跨模块组合用例挂靠**

- **相位机整链**（`drive` → `enter_phase` → `run_step` → `settle_phase` → 写面 `phase_next`）：发起方 / 最上层调用方 = `walker`（`drive`）⇒ 组合用例写 `core/orchestration/src/walker.rs -> walker_test.rs` 章节，执行文件 `walker_test.rs`，装置 = 真实 `LocalToolSteps` + `TestDb`（既有 `spawn_real_run` / `真实写面组合…` 用例先例）。
- **写面静态表 → 相位机**（`phase_table()` / `steps_of(phase)` 真件克隆 → 步循环）：入口在 `walker`，真值源在 `workflow`；组合用例仍写 `walker` 章节（`steps_of` 走真件，不手写步序副本）。
- **审计面 → store 链**（`LocalToolSteps::run` → `workflow::write::*` → `StepRecord`）：发起方 = `steps.rs`（`ToolStepPort` 实现）⇒ 组合用例写 `core/orchestration/src/steps.rs -> steps_test.rs` 章节。
- **语料 → golden**（种子构造 → `finish_command` / `log_change_phase` → detail 投影 → `tests/golden/*.json`）：入口 = 集成文件自身 ⇒ 自映射章节，不新建独立集成测试章节。

---

## 验收范围

<!-- 测试区域：`packages/desktop/src-tauri/crates/core/{orchestration,workflow}/src/**/*_test.rs`（sidecar）+ `crates/core/workflow/tests/corpus_golden_test.rs`（crate 级集成）。本变更零新建测试文件：5 个 sidecar 改写 + 追加、1 个集成文件改写，另 `FakeTools` / `FakeWorker` / `phase_log_input` / `two_phase_routes` / `archive_route_outcome` / `steps_of` 等既有装置随新形参与新形状调整（编译闭合 + 断言对象改写），不新增用例面。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **辅助步全退场**：11 张 `*_STEPS` 内 `PhaseStart` / `VerdictGate` / `PhaseLog` 出现 0 次（现状 30 处）；requirement 表 10 相位步数 = 3/2/2/2/4/3/2/1/1/9（合计 29）；test-only 表 7 相位 = 3/2/2/2/3/2/9（合计 23） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` -> `.../write/phase_table_test.rs`（序表 + 步数 + 零声明守卫）；`.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（整链节点数 = 真实步数） |
| AC-2 | **相位入口内化**：`enter_phase` 为唯一开相路径，写 active_phase（attempt = PhaseRecord 全量条数 + 1，含 stale）且零 `RunUpdate::Step` / 零 `StepRecord`；legacy initialize 短路亦经 `enter_phase` + `settle_phase(Skipped)` | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（零节点 + active_phase 在案 + attempt 口径 + 同相位连调两次不漂移）；`.../core/orchestration/src/steps.rs` -> `.../orchestration/src/steps_test.rs`（`PhaseStart` 命令零审计行） |
| AC-3 | **相位收口内化**：`settle_phase` 三分流逐分支可达（test_outcome 双槽 / evaluator checklist / 空 checklist）；archive 空 checklist 落 `verdict=pass` 且 `active_phase` 清位；落账失败时节点全绿 + run `failed` + reason 前缀「相位 X 全部步通过但落账失败」 | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（三分流 + 落账失败注入 + 双失败面文案对拍 + 三形态连调两次不重复落账） |
| AC-4 | **verdict 裁决内化**：`step_evaluator` 读 `outcome.finalized`；`Some(Evaluator(..))` → 同实例 passed + detail `verdict=…` + 交 `settle_phase`；缺席 → 同实例 failed + `Terminal`，「输出被截断」与「未调用 finish_step」两文案可辨；零 `feedback` 边、`VerdictGate` 节点零产出 | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（三态 + 会话 `Failed` / `Stopped` 仍红节点） |
| AC-5 | **决策节点承载裁决**：一次决策分叉只产 decision 节点（`instance_no` 按轮递增），detail 携动作或越权文案（含 `allowed`）；`WhitelistGate` / `RetryGate` 节点零产出；`ensure_backtrack_allowed` 仍纯函数、越权一次即 `Terminal` | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（通过 / 越权 / ask 应答三形态 + 两 gate 零节点） |
| AC-6 | **词汇冻结**：`StepNode` / `RunStepKind` / `ChangeStepKind` 三枚举各 22 值且变体序不变；`StepGroup::Gate` 分支在案（三 gate 仍归 Gate）；五词带 `decode-only` 注释 | `.../core/workflow/src/state.rs` -> `.../workflow/src/state_test.rs`（22 值 + 序 + Gate 归属 + 注释源自证）；`.../core/orchestration/src/state.rs` -> `.../orchestration/src/state_test.rs`（22 值线词 + 序 + 注释源自证）；bindings 零 diff 见「不可测试项」 |
| AC-7 | **归档链 9 步**：`ARCHIVE_STEPS` 序 = Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize；开相与落账不在 `steps` 内；`ARCHIVE_STEP_ORDER` 断言 9 | `.../core/workflow/src/write/phase_table.rs` -> `.../write/phase_table_test.rs`（`ARCHIVE_STEP_ORDER` 9 + 首末步）；`.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（archive 节点恰 9、无 phase_start / phase_log） |
| AC-8 | **计划图漂移升级为显式失败**：`run_step` 不可达臂对六词（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` / `Decision`）返回 `Terminal::failed`；`match` 仍穷尽 22 变体 | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（六词直构 steps 逐词 `Terminal`）；穷尽性由 `cargo check` 保证（见「不可测试项」） |
| AC-9 | **行为面零回归**：`ToolCommand` 载荷断言逐条不变；七条机械 checklist 条目文案（静态检查 / 架构检查 / 测试执行 / 突变测试 / suite 结论一致 / 聚合计数一致 / mutation null 自动通过）全部在案且不经任何门步 | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（载荷逐字段 + 七条目逐字）；`.../core/orchestration/src/steps.rs` -> `.../orchestration/src/steps_test.rs`（七臂审计回归） |
| AC-10 | **前端零改动可解存量**：`run-state.ts` 的 `STEP_LABEL` 仍含五键、gate 分组仍归 `gate`；`vp test` 既有渲染用例零改写零新增 | 零 diff 面 + 既有前端套件绿（见「不可测试项」） |
| AC-11 | **golden 零 diff**：`DESKTOP_GOLDEN_REWRITE=1` 重写后不带环境变量复跑为绿；真实相位形态样本 initialize 3 节点 / archive 9 节点按序 forward 串行 | `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` -> 同文件（真实形态样本 + 重写复核）（golden JSON 见「不可测试项」其重写流程） |
| AC-12 | **语料双口径**：五词（`phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate`）各至少一个节点样本仍在校（存量解码形态），与真实相位形态样本并列存在、互不替换；二十二值全词汇断言不变 | `.../core/workflow/tests/corpus_golden_test.rs` -> 同文件（五词样本 + 全词汇断言）；`tests/fixtures/README.md` 两条目见「不可测试项」 |

> **被测文件全覆盖对照**（design「修改文件」6 项 → 上表落点）：`core/orchestration/src/walker.rs` → AC-2 / 3 / 4 / 5 / 8 / 9；`core/orchestration/src/steps.rs` → AC-2 / 9；`core/workflow/src/write/phase_table.rs` → AC-1 / 7；`core/workflow/src/state.rs` → AC-6；`core/orchestration/src/state.rs` → AC-6；**`packages/desktop/package.json`（版本行）无对应 AC**（proposal R10 明定「该条是交付门而非能力验收，故不进 AC 表」），登记于「不可测试项」并以 `git diff` 审查验证。另纳入的 `crates/core/workflow/tests/corpus_golden_test.rs` 承载 AC-11 / AC-12（design「修改文件」表未列——它是测试面本身，见该节 HTML 注释）。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

既有 sidecar（16816 行 / 186 用例）**改写 + 追加**。改写面按 proposal「改写原则」三行逐条落：① `node_of(…, PhaseStart, 0)` 的相位入口锚点 → 该相位首个真实步节点（约 25 处命中）；② `step_instances(…, WhitelistGate)` → `step_instances(…, Decision)`（12 处命中）；③ `node.step == PhaseLog` 的「已落账」断言 → `phase_log_input(&commands, phase)` 载荷字段 / `TestDb` 的 pipeline attempts（66 处命中）；④ `VerdictGate` 节点断言（16 处）→ 该 attempt 的 evaluator 节点末实例（`node_of(…, Evaluator, 0)` 的终态与 `detail`）。追加面 = `enter_phase` / `settle_phase` / `SettleMode` / `step_evaluator` 三态 / 决策三态 / 六词不可达臂 / 2 个 `PhaseFail` 装配函数 / `entry_override` 边承载 / 双失败面文案对拍 / `enter_phase` 与 `settle_phase` 的重入幂等（同相位连调两次：开相不漂移、落账零重复）/ 6 处整链期望值重定。装置沿用 `FakeWorker`（预录 `finalized` 载荷 / 转录 / 失败与停止注入 / evaluator 停等门）+ `FakeTools`（预录 `PhaseStart` / `PhaseLog` / 三 runner 产出 + `fail_on` 标签注入）+ `FakeVcs` + `StubSnapshot` / `StoreSnapshot` + `RecordingHistory`（落包捕获）+ `TestDb`（真实 workspace 库）+ `spawn_run` / `spawn_run_with_vcs` / `spawn_real_run` / `spawn_real_run_archive` 四驱动。

#### 待测功能

- `enter_phase(tools, guard, request, phase) -> Result<u32, Terminal>`（新增，吸收 `step_phase_start`）: 经 `ToolCommand::PhaseStart` 单事务写 active_phase（phase / attempt / start_at）；**零 `begin_node` / 零 `emit_node` / 零 `StepRecord`**；`Err` → `Terminal::failed("phase-start 失败: <err>")`；重入（同相位连调）为 `active_phase` 原地覆写——零 `PhaseRecord` 新增、attempt 按「条数 + 1」不漂移。
- `settle_phase(tools, guard, request, cursor: &mut PhaseCursor, phase, settle: PhaseSettle<'_>) -> Result<u32, Terminal>`（新增，吸收 `step_phase_log` + `step_initialize_skipped_phase_log`）: 三形态 → `run_phase_log` 单事务（含 `active_phase` 清位）→ 回写 `cursor.failed`；落账 `Err` 保留原 `status` 并按形态重写 `reason` 前缀（`Pass` → 「相位 <phase> 全部步通过但落账失败: <原因>」、`Fail` → 「门禁失败落账失败」、`Skipped` → 「跳过落账失败」，D12）；`Pass` 三分流逐字搬入（`test_outcome` 双槽 / `evaluator_turn` checklist / 空 checklist）；重入（同相位连调）由写面 `active_phase` 前置守卫拒绝——零新增 `PhaseRecord`、零审计累积、清位保持。
- `PhaseSettle<'a>`（enum，新增）: `{ Pass, Fail(PhaseFail<'a>), Skipped(&'a str) }`——相位收口单入口三形态（私有）。
- `PhaseFail<'a>`（struct，新增）: `{ item: &'a str, report: String, evidence: String, executor_session: Option<String> }`；`item` ∈ 静态检查 / 架构检查 / 测试执行 / 突变测试；`report` 装配侧截断 ≤ `MAX_REPORT_CHARS`。
- `run_worker(…, mode: SettleMode) -> Result<WorkerTurn, Terminal>`（修改，+末位形参 + 返回 `WorkerTurn`）: `Immediate` 语义逐字不变；`Deferred` 只发 running 拍、成功路径零终态 emit；`worker.run` 的 `Err` 臂两模式都发 failed 节点并 `Err`。
- `worker_settle(outcome: &WorkerTurnOutcome) -> (StepSettle, Option<Terminal>)`（新增）: 会话状态面分类单点（`Completed` / `Failed` / `Stopped` / `Running` 四臂逐字搬入既有文案）。
- `worker_tokens(outcome: &WorkerTurnOutcome) -> (Option<u64>, Option<u64>)`（新增）: `sum_usage_tokens` 两键求和单点（`input_tokens` / `output_tokens`，缺席 `None`）。
- `WorkerTurn`（struct，新增）: `{ outcome: WorkerTurnOutcome, instance_no: u32 }`——`Deferred` 调用方据 `instance_no` 在同实例补终态拍（私有）。
- `EvaluatorTurn`（struct，新增）: `{ outcome: WorkerTurnOutcome, checklist: EvaluatorChecklist }`——会话结果与收口载荷同源，消除「有 `eval_outcome` 无 `verdict`」的非法可达态（私有）。
- `SettleMode`（enum，新增）: `{ Immediate, Deferred }`（`pub(crate)`，D4）。
- `PhaseCursor`（struct，修改）: 删 `entry_kind` / `eval_outcome` / `verdict`，增 `evaluator_turn: Option<EvaluatorTurn>`；`PhaseCursor::new(next, sync_specs)` 两形参（13 → 11 字段）。
- `EdgeTrace`（struct，修改）: 增 `entry_override: Option<EdgeKind>`（相位入口边一次性覆盖值，D3 / D9）。
- `begin_node(guard, trace, phase, attempt, step, incoming) -> u32`（修改，签名零变）: 落边时一次性消费 `trace.entry_override`（有覆盖值用覆盖值，否则用调用方 `incoming`）。
- `step_evaluator(ctx, trace, cursor)`（修改，签名零变）: `Deferred` 会话 → `worker_settle` / `worker_tokens` → 会话状态面先行（红节点 / 停止节点）→ 按 `finalized` 分派；合规写 `cursor.evaluator_turn` 并同实例 passed + `detail=verdict=…`；缺席按截断 / 未收口两文案 failed + `Terminal`。
- `decision_session(worker, tools, guard, request, input, trace)`（修改，签名零变）: 删 `WhitelistGate` 节点；每轮 decision 节点同实例收口（`passed` + 动作摘要 / `failed` + 越权文案）；`ask` 应答后开下一轮（`instance_no` 递增）。
- `resolve_deadlock(...)`（修改，签名零变）: 删 `RetryGate` 三次 emit；`trace.incoming = Some(Backtrack)` 与 `run_tool(Backtrack)` 落账逐字不动。
- `run_step(ctx, cursor, trace, step)`（修改，签名零变）: 20 → 17 分支；六词（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` / `Decision`）合并为单一不可达臂返回 `Terminal::failed`（D8 文案携相位与步词）；其余 16 臂逐字不动（含 `ArchiCheck`）。
- `open_step(ctx, trace, cursor, kind) -> u32` / `close_step(ctx, cursor, kind, instance_no, settle)`（重命名，原 `begin_archive_node` / `settle_archive_node`）: 语义零变（12 + 25 处调用点随动）。
- `test_execution_fail(outcome) -> PhaseFail<'static>` / `mutation_test_fail(outcome) -> PhaseFail<'static>`（新增）: 原两个 `*_upgraded_phase_log` 的 `item` / `report`（含空明细兜底与 `truncate_report`）/ `evidence` 装配逐字搬入，`executor_session = None`。
- `step_gate_fail_phase_log(tools, guard, request, cursor, phase, fail: GateFailLog<'_>) -> Result<(), Terminal>`（修改，去 `trace` / `attempt` 形参、加 `cursor`）: 体内只装配 `PhaseFail` 并调 `settle_phase(Fail)`；原 `begin_node` + 两次 `emit_node` 删除。
- `drive(...)`（修改，签名零变）: `enter_phase` → 迭代 `next.steps` → `settle_phase(Pass)`；legacy initialize 短路改 `enter_phase` + `settle_phase(Skipped)`；`steps.is_empty()` 判据保留；`entry_override` 置位与兜底清零；`failed_phase` 两次置位（D10）。
- **退役面（缺席断言）**: `step_phase_start` / `step_phase_log` / `step_initialize_skipped_phase_log` / `step_verdict_gate` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` / `begin_archive_node` / `settle_archive_node` 在 `walker.rs` 源码（去注释）零命中；`PhaseCursor::{entry_kind, eval_outcome, verdict}` 零命中。

<!-- design.md「公共函数 / API」表明写「本变更零公共 API 面变化…故无「公共函数 / API」条目」（`walk_run` / `RunRequest` / `new_run_id` / `ToolCommand` / `ToolStepOutput` / 四个 runner port 逐字不动）。上列条目全部取自同文件「内部函数签名（非导出，仅本变更内约定；供实现与评审对拍）」13 行与「类型定义」8 行，逐条带「新增 / 修改 / 重命名」标注——不 grep 源码导出声明反推、不虚构条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `enter_phase` | 正向 | **零节点零审计 + active_phase 在案**（AC-2）：`TestDb` + 真实 `LocalToolSteps` 跑两个相位 → 每相位 `find_change_record(..).active_phase` 在落账前呈 `Some{phase, attempt, start_at>0}`、落账后 `None`；`list_change_steps(change_id, run)` 内 `StepKind::PhaseStart` 行数 **0**；落包 `nodes` 内 `RunStepKind::PhaseStart` 节点数 **0**（原「节点 passed + attempt」断言改写为「无节点 + active_phase 在案」） | 单次 |
| `enter_phase` | 正向 | **attempt 口径**（R3 / ADDED「attempt 推导口径」）：同一 `(change_id, phase)` 预置 2 条 `PhaseRecord`（其一 `stale=true`，由 seed + `apply_backtrack` 造出）+ 一次新 run 进该相位 → 开相 attempt = **全量条数 + 1**（含 stale）；断言该值 **≠** `detail.pipeline[phase].attempts.len() + 1`（后者因 stale 过滤而回退）——节点身份键不撞号 | 单次 |
| `enter_phase` | 异常 | 写面 Err（`FakeTools::fail_on("phase-start")`）→ `Terminal::failed` 文案以 `phase-start 失败: ` 起首并携原错误串；零 `RunUpdate::Step` 拍、零 `StepRecord`（原「开相失败 failed 节点」断言改写为缺席断言） | 单次 |
| `enter_phase` | 边界 | 相位串边界：`""` / `"   "` / 未知相位 `"no-such-phase"` / 超长（1200 chars）/ 含 emoji → 一律经写面 `Err` 传播为 `Terminal::failed`（不 panic、不生节点）；`attempt` 起点取首 run 的 `1`（`PhaseCursor::new` 的 `0` 不泄漏为落账 attempt） | 参数化迭代（5 形态） |
| `enter_phase` | 边界 | **同相位连调两次不漂移**（重入幂等口径）：同一 `(change_id, phase)` 连调两次、两次之间无 `settle_phase`。形态经 `walk_run` 整链不可达（整链内两次同相开相之间恒隔一次落账），驱动面 = 同 crate 直调（`enter_phase` 非导出私有：以 `pub(crate)` 暴露——与 D4 的 `SettleMode` 同口径——或经 `walker.rs` 内 `#[cfg(test)]` 探针转发，两法语义等价、择一即可），装置 = 真实 `LocalToolSteps`（写面直调链，attempt 真推导）+ `TestDb` + `control.begin_run` 取得的 `RunGuard`（**不可换 `FakeTools`**：其 `PhaseStart` 产出为预录值，attempt 口径会自证）。断言：`assert_eq!(second_attempt, first_attempt)`（**取同一 attempt，不 +1**；口径 = 写面推导式「`PhaseRecord` 全量条数 + 1」，两次开相之间零落账 ⇒ 条数不变 ⇒ attempt 抬升只由落账产生、不由重入产生）；`list_phase_records` 行数**不变**（两次开相零 `PhaseRecord` 新增）；`active_phase` 为原地覆写（`attempt` 同值、`start_at` 取第二次开相时刻即 `>= 首次 start_at` 且 `> 0`，不产生第二条 active_phase 事实）；两轮皆零 `RunUpdate::Step` 拍 / 零 `StepKind::PhaseStart` 审计行；对照行：预置 1 条 `PhaseRecord`（`stale=true`，由 seed + `apply_backtrack` 造出）后连调两次 → 两次均取 `2`（条数 1 + 1），证「+1 不随重入累加、stale 行仍计入」 | 单次 |
| `settle_phase` | 正向 | **`Pass` 三分流 ①**：`test_outcome` 在场（test-execution 相位）→ 机械 checklist 三条目（`suite 结论一致` / `聚合计数一致` / `mutation null 自动通过`）全 pass、`report` 携 conclusion + 四计数 + 报告目录、三会话槽 `None`、`skipped=false`；`mutation_outcome` 在场 / 缺位两形态下第 3 条 `evidence` 取三态（跑过且达标 / diff 空跳过 / null）——逐字等价旧 `test_execution_phase_log_input` | 参数化迭代（2 形态） |
| `settle_phase` | 正向 | **`Pass` 三分流 ②**：`evaluator_turn` 在场（P→E 相位）→ 载荷 `checklist` 逐字段 = `turn.checklist`、`report` = 该 turn 的 report、`evaluator_session_id = Some(turn.outcome.session_id)`、`executor_session_id = cursor.executor_session`、`decision_session_id = None`；`cursor.evaluator_turn` 被 `take` 后为 `None`（**cursor 侧**不重复携带；写面侧的「二次收口零重复落账」由本表 `settle_phase` 的连调两次行承载） | 单次 |
| `settle_phase` | 正向 | **`Pass` 三分流 ③**：二者皆无（archive）→ `PhaseLogInput { report: "", checklist: [], skipped: false, 三槽 None }`，写面 `derive_verdict([]) = Pass`；`active_phase` 清位；零 `phase == "archive"` 特判（同一段代码承载三种相位形态） | 单次 |
| `settle_phase` | 正向 | **`Fail` 形态**（门禁升格）：`PhaseFail { item, report, evidence, executor_session }` → 载荷 `checklist` 恰 1 条 `{ item, pass: false, evidence }`、`report` = `PhaseFail.report`、`executor_session_id` = 该槽、`evaluator_session_id = None`、`skipped=false`；`item` 四值逐值（静态检查 / 架构检查 / 测试执行 / 突变测试） | 参数化迭代（4 item） |
| `settle_phase` | 正向 | **`Skipped` 形态**（legacy initialize 短路）：`PhaseSettle::Skipped(跳过因)` → `report` = 跳过因、`skipped = true`、`checklist = []`、三槽 `None`；由此该相位后续 run 持久跳过（`has_phase_passed` 认 skipped） | 单次 |
| `settle_phase` | 正向 | **`cursor.failed` 回写单点**（D7）：`Pass` + checklist verdict `Fail` → `true`；`Pass` + verdict `Pass` → `false`；`Fail(..)` → `true`；`Skipped(..)` → `false`；且 `drive` 的 `trace.failed_phase` 只在收口后置位（D10：verdict fail 相位的同相位重跑边判定读到新值） | 组合迭代（4 形态 × failed 两值） |
| `settle_phase` | 异常 | **`Pass` 落账失败**（AC-3 后半）：`FakeTools::fail_on("phase-log")` → run `Failed`、`reason` 以「相位 <phase> 全部步通过但落账失败: 」起首并携 `phase-log 失败: …`；落包内该相位节点**全部 passed**（零 failed 节点）；`Terminal.status` 保留原值（不降级） | 单次 |
| `settle_phase` | 异常 | **三形态前缀分文案**（D12）：`Fail` 落账失败 → 「相位 <phase> 门禁失败落账失败: …」、`Skipped` 落账失败 → 「相位 <phase> 跳过落账失败: …」；三条 reason 两两互不相同，且后两条**零**「全部步通过」字样（R2 的事实错误归因防线） | 参数化迭代（2 形态） |
| `settle_phase` | 边界 | 返回值与 phase 串边界：`PhaseLogOutcome.attempt` 取 `1` 与 `u32::MAX` → `settle_phase` 原样返回该值；`phase` 取 `""` / 超长 / emoji → 载荷 `phase` 逐字透传不裁剪、前缀文案逐字插入不 panic | 组合迭代（2 attempt × 3 phase） |
| `settle_phase` | 边界 | **连调两次不重复落账**（重入幂等口径，三形态各一行）：同一相位 `settle_phase` 连续调用两次（`Pass` / `Fail(..)` / `Skipped(..)` 三形态分别驱动；驱动面与装置同上一行——同 crate 直调（`pub(crate)` 暴露或 `walker.rs` 内 `#[cfg(test)]` 探针）+ 真实 `LocalToolSteps` + `TestDb` + `RunGuard`；`walk_run` 整链内同样不可达）→ ① `list_phase_records(change_id)` 行数**不变**（首次落账恒 1 行、第二次零新增）；② `StepKind::PhaseLog` 审计行**零累积**（两次调用后仍 0 行，写面归零与重入正交）；③ `active_phase` **保持清位**（两次调用后恒 `None`，第二次不重新开相）；④ 第二次返回 `Err(Terminal)`，`reason` 为该形态前缀 + `run_tool` label 内的写面拒绝因（`Pass` → 「相位 <phase> 全部步通过但落账失败: phase-log 失败: phase \"…\" 未开启（无 active_phase）…」/ `Fail` → 门禁失败前缀 / `Skipped` → 跳过前缀，D12 分文案不变），且 `status` 保留写面原值（重入拒绝路径为 `Failed`；若写面因取消返回 `Stopped` 则原样保留、不降级为 `Failed`）；⑤ 落包节点集在两次调用间**零新增**（落账动作零 emit——「节点全绿 + run failed」的 D3 面在重入下不变）；`Pass` 形态的第二次调用因 `evaluator_turn` / `test_outcome` 已被首次收口消费而退化为第三分支（空 checklist），仍被写面 `active_phase` 守卫拒绝——三形态各自的退化路径都不产生第二次落账。写面纵深侧证：`(change_id, phase, attempt)` 查重为第二道防线（`store_test` 的「同相位同轮二次落账」先例），第一道为 `active_phase` 前置守卫 | 参数化迭代（3 形态） |
| `PhaseSettle<'a>` / `PhaseFail<'a>` | 正向 | 类型面：`PhaseSettle` 三形态可构造且互异（`assert_ne!` 双向）；`PhaseFail` 四字段逐字段可辨；`item: &'a str` 为借用面（`'static` 装配函数返回 `PhaseFail<'static>` 直接可传）；`report` 经装配侧 `truncate_report` 后 ≤ `MAX_REPORT_CHARS`（2000） | 单次 |
| `PhaseSettle<'a>` / `PhaseFail<'a>` | 异常 | 类型封闭与非法形态：`PhaseSettle` 无第四变体（`match` 穷尽三形态，新增形态即编译失败）；`PhaseFail.item` 由调用方给（`"静态检查"` / `"架构检查"` / `"测试执行"` / `"突变测试"` 之外的值不校验——本层纯数据载体、不裁决 item 词面）；`PhaseFail.report` 直构越界串（`"字".repeat(2001)`）不 panic（截断归装配函数） | 参数化迭代（3 形态） |
| `PhaseSettle<'a>` / `PhaseFail<'a>` | 边界 | 退化形态：`Skipped("")` 空跳过因 → `skipped = true` + `report = ""`（零 panic、零「跳过因缺位」分支）；`PhaseFail { evidence: String::new(), executor_session: None }` → 空 evidence 与空槽位落库，checklist 条目 `evidence = ""` 不省键 | 参数化迭代（2 形态） |
| `run_step` | 正向 | 回归登记（16 实臂）：`implement` 相位四步（`Executor` → `StaticCheck` → `ArchiCheck` → `Evaluator`）逐臂执行体语义零变（`ArchiCheck` 仍座位在 static-check 之后、evaluator 之前，`FeedbackLoop{3}` 独立计数）；归档九步族经 `open_step` / `close_step` 的节点序列与 detail 逐字不变；`StepFlow::Proceed` / `Upgraded` 两返回面逐臂不变 | 参数化迭代（16 臂 + 归档九步） |
| `run_step` | 异常 | **计划图漂移显式失败**（AC-8 / D8）：直构 `next.steps` 分别含 `PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` / `Decision` 各一项 → run `Failed`，`reason` 含「计划图漂移」+ 相位串 + 该步词（`StepNode::as_str()`）；同实例零终态节点（原静默 `Proceed` 被显式失败取代） | 参数化迭代（6 词） |
| `run_step` | 边界 | 判据边界：`next.steps = []`（R7）→ `Terminal::failed` 文案「缺计划步序列（PhaseDefinition.steps）」，且文案不再提「恒含 PhaseStart 起步」（改「恒含至少一个真实步」）；`steps` 内两不可达词并列 → 首词即 `Terminal`（不继续迭代后词）；不可达词位于 `steps` 末尾（其余真实步已执行完）→ 仍 `Terminal::failed`、不落账 | 参数化迭代（3 形态） |
| `run_worker` | 正向 | **`Immediate` 语义逐字等价**：五调用点（executor / static-check 反馈轮 / archi-check 反馈轮 / resolve-conflicts / spec-sync）→ running 拍 + 终态单拍、会话 id 与 token 两值落值、`Completed` 返回 `Ok(WorkerTurn)`（`.outcome` 取用）；步骤时间关系式 `ended_at >= started_at` 仍成立 | 参数化迭代（5 调用点） |
| `run_worker` | 正向 | **`Deferred` 只发 running 拍**：evaluator / decision 两会话步 → 返回后同实例**无终态**（`ended_at = None`、`status = running`）；调用方判定后终态**恰一拍**（`accumulate_node` 合并后一行终态，无第二拍覆盖） | 单次 |
| `run_worker` | 异常 | `worker.run` Err 臂两模式同式：`Deferred` 与 `Immediate` 均发 failed 节点（`detail` 携 `diagnose_brief(&error)`）并 `Err(Terminal::failed("会话执行失败: …"))`（R11：`Deferred` 漏发终态的防线） | 参数化迭代（2 模式） |
| `run_worker` | 边界 | 模式 × 会话状态交叉（`SettleMode` 两值 × `AgentRunStatus` 四值 = 8 格）：`Completed` 两模式均无终态 Err（`Immediate` 已发 passed、`Deferred` 由调用方补）；`Failed` / `Stopped` / `Running` 三态两模式均落对应节点 + `Err`（`Deferred` 不因延迟而漏判定） | 组合迭代（8 格） |
| `worker_settle` | 正向 | 四态分类单点（`AgentRunStatus` N+1 形态）：`Completed` → `(passed(None), None)`；`Failed` → `failed("会话失败收敛: <因>")` + `Err(failed)`；`Stopped` → `stopped("会话被终止")` + `Err(stopped)`；`Running` → `failed("会话收口缺终态")` + `Err(failed)`——四形态逐文案与既有 `run_worker` 匹配臂逐字等价 | 参数化迭代（4 形态） |
| `worker_settle` | 异常 | `failure_reason` 缺席：转录零 `api_error` / 零 `TurnDone{subtype}` → `Failed` 分支回落通用文案「会话失败收敛」（无 `: 因` 后缀）；成因串超长 → `diagnose_brief` 截断（不 panic、不溢出 `MAX_REPORT_CHARS` 类上限） | 参数化迭代（2 形态） |
| `worker_settle` | 边界 | transcript 边界：空 transcript + `Failed` → 通用文案；1000 事件 transcript + `Running` → 「会话收口缺终态」（事件数不参与判据）；`session_id` 空串不参与本函数（分类只看 `status` + 转录） | 参数化迭代（3 形态） |
| `worker_tokens` | 正向 | 求和单点：两 `TurnDone.usage` 各带 `input_tokens` / `output_tokens` → 两值分别求和；单键缺席 → 对应 `None`（另一键仍求和）；非 `TurnDone` 事件零参与 | 参数化迭代（3 形态） |
| `worker_tokens` | 异常 | usage 非法形态：键值非数字（字符串 `"10"` / `null` / 对象）→ `as_u64()` 失败即该值不计入且 `any` 不置位（→ `None`），不 panic、不把 0 当事实值 | 参数化迭代（3 形态） |
| `worker_tokens` | 边界 | 空 transcript → `(None, None)`；usage 值 `u64::MAX` + 1 → `saturating_add` 饱和不 panic（不溢出、不 panic 断言）；usage 值 `0` → `Some(0)`（在场而非 `None`，与「缺席」可辨） | 参数化迭代（3 形态） |
| `step_evaluator` | 正向 | **合规态**（AC-4）：`finalized = Some(Evaluator(checklist))` → 该 attempt 的 evaluator 节点**唯一实例**终态 `passed`、`detail = "verdict=pass"` / `"verdict=fail"`（两值）、`session_id` 与 token 随行；`cursor.evaluator_turn = Some(..)`（`outcome` 与 `checklist` 同源）；零 `feedback` 边、零 `verdict_gate` 节点 | 参数化迭代（2 verdict） |
| `step_evaluator` | 异常 | **未收口态**：`status = Completed` 且 `finalized = None`（转录零 `finish_step` notice）→ 同实例 `failed` + `detail` 「未调用 finish_step 收口（相位 evaluator 会话 Completed 但无收口载荷）」+ `Terminal::failed`；状态库零新增 eval 记录、零 `settle_phase` 调用（`phase_log` 命令数 0） | 单次 |
| `step_evaluator` | 异常 | **截断态**：转录含 `SystemNotice{subtype:"output_truncated"}` + `finalized = None` → `detail` 「评估输出被截断（output_truncated），未产生 finish_step 收口产物」；与未收口态文案 `assert_ne!` 互不包含（两文案可辨） | 单次 |
| `step_evaluator` | 异常 | 角色漂移：`finalized = Some(FinishPayload::Decision(..))` → 走同一失败分支（与 `None` 同 `reason` 族），不写 `cursor.evaluator_turn`、不落账 | 单次 |
| `step_evaluator` | 边界 | **会话状态面先行**（D5 第二半 / AC-4 后半）：`Deferred` 下 `AgentRunStatus::Stopped` → 停止节点（`status = stopped`）+ `Terminal::stopped`；`Failed` → failed 节点 + `Terminal::failed`；两态 MUST NOT 被业务分支吞成 passed（断言终态节点 status 非 passed 且 run 非 Completed） | 参数化迭代（2 态） |
| `step_evaluator` | 边界 | `ctx.next.evaluator = None`（评估-only 相位定义漂移）→ `Terminal::failed` 文案「计划步漂移：Evaluator 步缺 evaluator 定义」，零会话发起、零节点 | 单次 |
| `decision_session` | 正向 | **动作裁决写 decision 节点 detail**（AC-5）：`Backtrack { to }` → decision 节点同实例 `passed` + `detail = "backtrack → <to>"`；`Stop { reason }` → `passed` + `detail = "stop：<reason>"`；两形态 `session_id` 与 token 随行；`step_instances(.., RetryGate)` 与 `(.., WhitelistGate)` 均**空**（原 12 处断言改读 `Decision`） | 参数化迭代（2 动作） |
| `decision_session` | 正向 | **ask 应答后 instance_no 递增**：`Ask { question, options }` → `RunUpdate::Ask` 流出 → `wait_answer` 应答 → 新一轮 decision 节点 `instance_no = 1` 且 `passed` + `detail = "ask：<摘要>"`；ask 不计失败（run 继续推进至 Completed）；决策会话数 1、轮数 2、第二轮 prompt 为 `decision_answer_prompt` | 单次 |
| `decision_session` | 异常 | **越权一次即停**：`Backtrack { to ∉ allowed }` → 同实例 `failed` + `detail` 携 `diagnose_brief` 截断的拒因（含 `allowed`）；`Terminal::failed` 文案「决策越权：<message>」；零第二轮会话（`ensure_backtrack_allowed` 纯函数逐字不动） | 单次 |
| `decision_session` | 异常 | `finalized` 三异常态（N+1）：`None` 未收口 / `None` + `output_truncated` / `Some(Evaluator(..))` → 三文案（「未调用 finish_step 收口（决策会话…）」/「决策输出被截断…」/「决策会话返回 evaluator 收口载荷（相位表角色漂移）」）逐条可辨，均 `Terminal::failed` | 参数化迭代（3 形态） |
| `decision_session` | 边界 | `wait_answer` 返回 `None`（取消置位）→ `Terminal::stopped("ask 停等被终止")`；该 decision 节点停在 ask 轮（`passed` + `ask：…`）不留 running 残留 | 单次 |
| `resolve_deadlock` | 正向 | 节点面归零：`Backtrack` 决议路径 → `step_instances(.., RetryGate)` 空；`ToolCommand::Backtrack` 载荷逐字段不变（phase / to / reason / allowed）；`trace.incoming = Some(Backtrack)` 生效 → 目标相位**首个真实步节点**的 incoming 边 = `backtrack`；回 `phase_next` 重路由 | 单次 |
| `resolve_deadlock` | 异常 | 早退两形态逐字不变：`next.last_result = None` → `Terminal::failed("…缺 last_result 快照…")`，零决策会话、零 `Backtrack` 命令、零节点；快照 `detail` 读取 `Err` → `Terminal::failed("快照读取失败: …")`，同上零副作用 | 参数化迭代（2 形态） |
| `resolve_deadlock` | 边界 | 决议边界：`Backtrack` 落账 `Err`（`fail_on("backtrack")`）→ `Terminal` 且 `trace.incoming` **未置位**（下一相位边仍 `forward`）；`Stop` → `Terminal{status: Stopped, reason: "决策 stop: <reason>"}`（status 不降级为 Failed）；`Ask` 携出会话外 → `Terminal::failed("决策漂移：ask 动作未被应答循环消化")` | 参数化迭代（3 形态） |
| `begin_node` / `EdgeTrace::entry_override` | 正向 | 入口边承载（D3）：`drive` 在非 legacy 分支置 `entry_override = Some(kind)` → 相位首个真实步节点落边取覆盖值（`backtrack`）；无覆盖时取调用方 `incoming`（`forward`）；覆盖值**一次性消费**（该相位第二节点边 = `forward`） | 参数化迭代（2 形态） |
| `begin_node` / `EdgeTrace::entry_override` | 异常 | 覆盖值不泄漏（D9）：legacy initialize 短路（零步执行）→ 下一相位首节点边 = `forward`（覆盖值未被消费即清零）；首步即 `Err` 的相位 → 下一相位边仍 `forward` | 参数化迭代（2 形态） |
| `begin_node` / `EdgeTrace::entry_override` | 边界 | 边落点边界：run 首节点（`trace.tail = None`）→ 不落边（覆盖值仍被消费并清零，不泄漏到第二节点）；覆盖值与调用方 `incoming` 同为 `backtrack` → 无重复边；同一相位内 feedback 边的 `instance_no` 递增节点其 incoming 仍为 `feedback`（覆盖值已消费） | 参数化迭代（3 形态） |
| `open_step` / `close_step` | 正向 | 重命名语义零变：`step_clean` / 归档九步族 / `fail_archive_step` 三调用面的节点序列、`instance_no` 分配与终态 detail 逐字不变（原 `begin_archive_node` / `settle_archive_node` 断言零改动即绿）；落账面不再调用二者（`step_gate_fail_phase_log` 与升格两段零节点） | 参数化迭代（3 调用面） |
| `open_step` / `close_step` | 异常 | 旧名零命中（源自证）：`begin_archive_node` / `settle_archive_node` 在 `walker.rs` 去注释原文零命中（重命名未留别名）；`close_step` 用未由 `open_step` 开出的 `instance_no` 时仍按身份键就地合并（不 panic、不新增节点）——调用方不变量由成对书写保证 | 参数化迭代（2 形态） |
| `open_step` / `close_step` | 边界 | `fail_archive_node` 与 `step_clean` 的 legacy 分支（`root == main_root` 直接 `passed` + 跳过因）→ 节点序列与 detail 逐字不变；`instance_no` 从 `0` 起（同 `(phase, attempt, step)` 单点递增） | 参数化迭代（3 形态） |
| `test_execution_fail` / `mutation_test_fail` | 正向 | 装配逐字等价：`item = "测试执行"` / `"突变测试"`；`report` = `truncate_report(<findings_detail 或 feedback_text>)`（非空时逐字）；`evidence = "<diagnose_brief>；报告目录 <report_dir>"` / `"<diagnose_brief>；<mutation_report_refs>"`；`executor_session = None`；下落账载荷与旧 `*_upgraded_phase_log` 逐字段一致 | 参数化迭代（2 装配函数 × 明细空 / 非空） |
| `test_execution_fail` / `mutation_test_fail` | 异常 | 截断与报告缺位：`report` 源串 > `MAX_REPORT_CHARS`(2000) → 截断留痕（旧 `truncate_report` 行为逐字保留，`chars().count() == 2000`）；`report_paths = []` → evidence 携「（无报告产物）」；`findings_brief = ""` / `feedback_text = ""` → evidence 只有「；<后缀>」不 panic | 参数化迭代（3 形态） |
| `test_execution_fail` / `mutation_test_fail` | 边界 | 阈值边界：`report` 源串恰 2000 字符 → 不截断（逐字透传）；2001 → 截断；`report_paths` 单元素 vs 多元素 → 逐条 `、` 连接零改写；`item` 恒 `'static` 字面量（`PhaseFail<'static>` 可直传 `settle_phase`） | 参数化迭代（3 形态） |
| `step_gate_fail_phase_log` | 正向 | 载荷构造辅助（D6）：形参去 `trace` / `attempt` 后 → `PhaseFail { item, report: fail.reason, evidence: fail.diagnostics, executor_session: cursor.executor_session.clone() }`；落账载荷 = 单条 `{ item, pass: false, evidence }` + `report` 首句；零 `PhaseLog` 节点（原两次 emit 断言改缺席）；静态检查 / 架构检查两条反馈边逐字同形 | 参数化迭代（2 item） |
| `step_gate_fail_phase_log` | 异常 | 落账失败：`fail_on("phase-log")` → `settle_phase(Fail)` 的 `Err` → `Terminal` 携「相位 <phase> 门禁失败落账失败: …」前缀（**不**用「全部步通过」前缀，D12）；门禁步自身的 failed 节点仍在（红节点不被覆盖） | 单次 |
| `step_gate_fail_phase_log` | 边界 | `executor_session = None`（无 executor 会话的相位）→ `PhaseLogInput.executor_session_id = None`、`evaluator_session_id = None` 不 panic；`diagnostics = ""` → 条目 `evidence = ""` 不省键；`reason = ""` → `report = ""`、单条 checklist 仍落 | 参数化迭代（3 形态） |
| 七条机械 checklist 条目文案（AC-9） | 正向 | 条目名逐字在案且**不经任何门步**：`PhaseFail` 四 `item`（静态检查 / 架构检查 / 测试执行 / 突变测试）与 `settle_phase` 机械分支三条（suite 结论一致 / 聚合计数一致 / mutation null 自动通过）——七串在 `walker.rs` 各命中 ≥ 1（源自证），且七者均经 `settle_phase` 落账路径（`phase_log` 命令载荷断言），零 `WhitelistGate` / `RetryGate` / `VerdictGate` 参与 | 参数化迭代（7 条目） |
| 七条机械 checklist 条目文案（AC-9） | 异常 | 零删除判据：七条目名**逐字**在源文件中命中（任一缺失即红）；旧门步路径零残留——`step_verdict_gate` / `step_gate_fail_phase_log` 的两次 emit 均不再产生 `PhaseLog` 节点（`step_instances(.., PhaseLog)` 空） | 参数化迭代（7 条目 + 1 缺席） |
| 七条机械 checklist 条目文案（AC-9） | 边界 | `ToolCommand` 载荷回归边界：`PhaseLogInput` / `DecisionLog` 三字段 / 三个 runner 命令（`StaticCheck{change_id, slot}` / `TestExecution{change_id, slot}` / `MutationTest{change_id}` / `ArchiCheck{change_id}`）逐字段断言不变；`report` 上限 2000 与 `evidence` 空串两边界形态不改 | 参数化迭代（4 命令面 + 2 边界） |
| `SettleMode` / `WorkerTurn` / `EvaluatorTurn` / `PhaseCursor`（类型面） | 正向 | `SettleMode` 两值 `Immediate` / `Deferred` 可构造、互异且 `pub(crate)` 可见面不外溢（`walk_run` / `RunRequest` 公开面零变化）；`WorkerTurn.instance_no` 使 `Deferred` 调用方在同实例补终态（零新节点：`step_instances` 该步实例数 = 轮数）；`EvaluatorTurn` 两字段同源（`outcome.session_id` 与 `checklist` 来自同一 turn）；`PhaseCursor` 字段集 11（`evaluator_turn` 在场），`PhaseCursor::new(next, sync_specs)` 两形参 | 参数化迭代（4 类型） |
| `SettleMode` / `WorkerTurn` / `EvaluatorTurn` / `PhaseCursor`（类型面） | 异常 | **非法可达态已消除**（A3 的核心收益）：`evaluator_turn` 在场即 `checklist` 在场（同源 struct，不存在「有 `outcome` 无 `checklist`」形态）；旧 `eval_outcome` / `verdict` 双槽在 `walker.rs` 零命中；`entry_kind` 字段零命中（由 `EdgeTrace.entry_override` 承载）；`immediate` / `deferred` 之外的第三模式不可构造（编译期封闭）——五判据源自证 | 参数化迭代（5 判据） |
| `SettleMode` / `WorkerTurn` / `EvaluatorTurn` / `PhaseCursor`（类型面） | 边界 | `PhaseCursor::new` 中性值：`attempt = 0` / `executor_session = None` / `evaluator_turn = None` / 三 outcome 槽 `None` / `failed = false`；`executor_level` 由 `next.executor` 缺席时取 `ModelLevel::default()`（不 panic）；`next: &PhaseNextOutcome` 为借用面（`sync_specs` 两值） | 参数化迭代（2 形态） |
| 退役面（缺席断言） | 异常 | 源自证（`walker_code_lines()` 去注释）：`step_phase_start` / `step_phase_log` / `step_initialize_skipped_phase_log` / `step_verdict_gate` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` / `begin_archive_node` / `settle_archive_node` / `cursor.entry_kind` / `cursor.eval_outcome` / `cursor.verdict` 逐项零命中；`ChangeStepKind::{PhaseStart, PhaseLog, VerdictGate, RetryGate, WhitelistGate}` 在 `walker.rs` 零命中 | 参数化迭代（判据表逐项） |
| 退役面（缺席断言） | 正向 | 反向判据（防「顺手清词汇」）：`StepNode::{PhaseStart, PhaseLog, VerdictGate, RetryGate, WhitelistGate}` 与 `RunStepKind` / `ChangeStepKind` 的同五词**仍在**词汇定义中（源自证命中 ≥ 1）；`StepNode::as_str()` 五词逐字可调（存量解码面在场）——「计划图零引用」与「词汇在场」两判据并列，删任一侧即红 | 参数化迭代（5 词） |
| 退役面（缺席断言） | 边界 | 落账前缀计数边界：`walk_run` 运行期 reason 前缀「全部步通过但落账失败」/「门禁失败落账失败」/「跳过落账失败」在 `walker.rs` 各**恰 1 处**（0 处 = 漏写、2 处 = 复制了第二份文案）；且三串互不为子串（`assert!(!a.contains(b))` 双向） | 参数化迭代（3 前缀） |
| 组合：`drive` 整链 | 正向 | **真件组合**（入口 = `walker`，执行文件 `walker_test.rs`）：真实 `LocalToolSteps` + `TestDb` + `StoreSnapshot` 跑完全相位 → 落包节点总数 = requirement 表真实步数之和（29，其中 initialize 3 / archive 9）；节点词集内 `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` **零出现**；边数相应减少且全 `forward`（除 feedback / backtrack）；`PhaseRecord` 十相位各 1 条（initialize skipped 1）、`active_phase` 终态 `None` | 单次 |
| 组合：`drive` 整链 | 异常 | 真实写面下的红节点路径：某真实步系统异常（`FakeRunner::erring` / executor `Failed`）→ 该步节点 `failed` + run `failed`，`active_phase` 停在当前相位未清位（写面事实）、无后续相位节点；同 run 内 `StepRecord` 内 `phase_start` / `phase_log` 仍零行（审计归零与失败面正交） | 参数化迭代（2 异常源） |
| 组合：`drive` 整链 | 边界 | 规模与相位边界：跑满十相位（含 initialize / archive）→ 每相位首个真实步节点的 incoming 边按 entry 语义（首相位 `forward`、回溯后 `backtrack`）；`steps = []` 的相位定义注入 → `Terminal::failed` 且 `active_phase` **未写入**（`enter_phase` 前即拒）；节点身份键 `(phase, attempt, step, instance_no)` 全 run 唯一 | 参数化迭代（3 形态） |
| 组合：archive 相位 | 正向 | stop-before 停等面语义逐字不变：确认后 `enter_phase` + 九步 → `settle_phase(Pass)` 空 checklist 落账 `verdict=pass`；archive 节点恰 **9**（`Preflight`…`Finalize`，零 `phase_start` / `phase_log`）+ 8 条 forward 串行边 | 单次 |
| 组合：archive 相位 | 异常 | 未确认（`spawn_confirmer(&control, false)`）→ `Terminal::stopped`「phase \"archive\" 停等未获继续确认（归档为独立手动执行门）」；archive 零节点、`active_phase` 未写入、零 vcs 调用 | 单次 |
| 组合：archive 相位 | 边界 | 归档预案载荷边界：`ArchivePreflight` 缺席（legacy 主 root 形态）→ `ConfirmWait{archive: None}` 仍发、停等判据不变；确认回流 `sync_specs = false` → `SpecSync` 步按既有语义跳过（`PhaseCursor.sync_specs` 传递不变） | 参数化迭代（2 形态） |
| 组合：legacy initialize 短路 | 正向 | `root == main_root` → `enter_phase` + `settle_phase(Skipped(跳过因))`：零步执行（零 vcs 调用、零检查 spawn）、零节点、`PhaseRecord` 一条 `skipped=true` + 空 checklist、`report` = 跳过因 | 单次 |
| 组合：legacy initialize 短路 | 异常 | 写面 Err 两分支分别归因：`enter_phase` 失败 → `Terminal::failed("phase-start 失败: …")` 且**无** `phase_log` 命令；`settle_phase(Skipped)` 失败 → `Terminal::failed("相位 initialize 跳过落账失败: …")`（跳过前缀，非「全部步通过」） | 参数化迭代（2 形态） |
| 组合：legacy initialize 短路 | 边界 | 判据边界：`root != main_root`（worktree 形态）→ **不**短走，execute 三步真实执行（`clean` / `static_check` / `test_execution`）后 `settle_phase(Pass)`；`main_root` 与 `root` 仅尾部斜杠差异的形态登记为装置口径（相等判定逐字，不做路径归一） | 参数化迭代（2 形态） |
| 组合：双失败面文案对拍（R2 / AC-3 + AC-4） | 正向 | 单型识别（两型各自可归因）：(a) `executor` `Failed` → 该步节点 `failed` + run failed，`reason` 走既有「会话执行失败 / 会话失败收敛 / 会话被终止」文案；(b) `settle_phase(Pass)` 落账失败 → 节点**全绿** + run failed + 「全部步通过但落账失败」前缀 | 参数化迭代（2 型） |
| 组合：双失败面文案对拍（R2 / AC-3 + AC-4） | 异常 | 两型互斥判据：断言 (a) 的 reason 不含「全部步通过」而 (b) 含之；两串互不包含（`assert!(!a.contains(b))` 双向）；(a) 节点集含 ≥ 1 failed，(b) 节点集 failed 数 = 0，且 (b) 的节点集非空（否则「全绿」断言空转） | 组合迭代（2 型 × 2 系统异常文案） |
| 组合：双失败面文案对拍（R2 / AC-3 + AC-4） | 边界 | 同 run 双相位混合：相位 1 全绿 + 落账失败、相位 2 步内异常 → run reason 取**先发生**者（`Terminal` 首出即返回），未被采纳那一型的文案零出现；`status` 二者皆 `Failed`（可辨性只落在 reason，不在 status） | 单次 |
| 整链期望值重定（6 处 `walk_run` 用例） | 正向 | 推进序判据保留：`全循环工具调用序…` 时间线由 8 项 → 6 项（去掉 `phase-start` / `phase-log`），序为 `phase-next → executor → static-check → archi-check → evaluator → phase-next`；`pass自动推进至done…` 的 `PhaseStart` / `PhaseLog` **命令载荷断言保留**（写通道仍在：change id + 相位序逐条对齐），节点 / 边计数按新步数重定；`真实写面组合全程演进对照一致` 的 `PhaseRecord` 十相位条数与 attempt 序不变 | 参数化迭代（6 处） |
| 整链期望值重定（6 处 `walk_run` 用例） | 异常 | 终止性判据保留：`two_phase_routes` / `archive_route_outcome` 两族驱动下 run 仍收敛 `Completed`（非空转、非活锁）；失败族用例（工具步失败 / 会话失败 / 决策越权）的 `Terminal` 文案断言逐条保留，仅节点计数期望随退场下调 | 参数化迭代（2 族） |
| 整链期望值重定（6 处 `walk_run` 用例） | 边界 | 计数边界：节点总数期望由「真实步数 + 辅助步数」改为「真实步数」（initialize 5 → 3、archive 11 → 9、P→E 5 → 2、implement 7 → 4、test-gen 6 → 3、test-execution 4 → 2、code-review / acceptance 4 → 1）；边数 = 节点数 − 1（纯 forward 链的形态）；`emit_node` 计数相对改造前减少恰为相位机 / 落账面拍数（读码核对，无「落账 emitted 成节点」残留） | 参数化迭代（7 相位） |
| `emit铸造` 瞬步面 | 正向 | 时间关系式回归：`executor` / `static_check` 等既有双 emit 步 → `started_at > 0` 且 `ended_at >= started_at`；会话步终态 `ended_at` 在场（断言零改写即绿） | 参数化迭代（2 步类） |
| `emit铸造` 瞬步面 | 异常 | 残缺形态：未收口步（只有 running 拍）→ `ended_at = None`、`status = running`（`Deferred` 下的合法在途态，MUST NOT 由 `emit_node` 补 ended）；终态后无第二拍覆盖（`accumulate_node` 合并侧不新增宽容分支） | 参数化迭代（2 形态） |
| `emit铸造` 瞬步面 | 边界 | 瞬步生产例退场后语义仍由直构承载：`accumulate_node` 的「`started_at` 保首见」与「单 emit 即 `started == ended`」由 `ChangeStepState` 直构断言（原 `phase_start 瞬步单emit同刻` 用例改写为直构形态），**MUST NOT 删除合并分支**（`control_test` 既有断言零改动即绿） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorkerAgentPort`（`FakeWorker` 手写假件） | 既有假件：按角色预录产出（`with_evaluator_reports` / `with_decision_reports` / `with_transcripts` / `with_executor_failures` / `with_executor_stops`）+ `EvaluatorGate` 停等门 + `WorkerTurnRequest` 全量捕获——agent 会话是**进程边界**且是 `walk_run` 的显式注入依赖，故可 mock | 全章节（尤其 evaluator 三态 / 决策三态 / `Deferred` 拍序） |
| `ToolStepPort`（`FakeTools` 手写假件） | 既有假件：`with_phase_next` / `with_static_check` / `with_test_execution` / `with_mutation` / `with_archi_check` / `with_archive_preflight` / `with_archive_seal` 预录 + `fail_on(label)` 注入（`phase-start` / `phase-log` 落账失败注入面）+ 命令载荷全量捕获——工具步是**进程边界**（子进程 / 写面）且是显式注入依赖 | 全章节（尤其落账失败 / 六词不可达臂 / 载荷断言） |
| `ArchiveVcsPort`（`FakeVcs`）/ `WorkflowSnapshotPort`（`StubSnapshot`） | 既有假件：`FakeVcs::clean()` 及 `with_*` 结果注入；`StubSnapshot` 用于不消费 detail 的用例（决策分叉用例走 `StoreSnapshot` + `TestDb`）——vcs 与快照均为进程 / DB 边界 | 归档链 / 决策分叉 |
| 不 mock：`ChangeStateStore`（`TestDb` 真实 workspace 库 + `LocalToolSteps`） | 真件组合（`spawn_real_run` 既有驱动）：`enter_phase` 的 active_phase / attempt 口径、`settle_phase` 的落账与清位、审计零行三判据的**事实源只在真实写面**；替身化会让「attempt 含 stale」与「零 StepRecord」自证 | `enter_phase` 口径行、零审计行、相位机整链组合行、legacy 短路行、两条重入幂等行（连调两次的「行数不变 / 清位保持 / attempt 不漂移」只在真实事务语义下成立，写面 `active_phase` 前置守卫与 `(change_id, phase, attempt)` 查重是纵深两道防线） |
| 不 mock：`phase_table()` / `steps_of(phase)` / `phase_step(phase, node)` | 真实静态表（`steps_of` 真件克隆，不手写步序副本）——AC-1 / AC-7 / AC-8 的真值源；替身化会让「辅助步零声明」空转 | 全章节（步序列 / 归档九步 / 六词直构除外的真实步） |
| 不 mock：`worker_settle` / `worker_tokens` / `test_execution_fail` / `mutation_test_fail` / `settle_phase` / `enter_phase` | 同模块内真值直调——它们同时是被测对象与其余用例的装置（分类单点 / 载荷装配单点） | 全章节 |
| 不 mock：`RunGuard` / `ChangeFlowControl` / `RunNotice` 订阅 / 落包 `RecordingHistory` | 真实实例 + 既有 `timeline` 记录（事件序断言面）；`begin_run` 真实登记 run | 全章节 |
| 不 mock：时钟 | 步时间只做关系式断言（`ended >= started`、`started > 0`），绝对值不入断言；corpus 侧定值注入另见语料章节 | 全章节 |

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

既有 sidecar（4980 行 / 60 用例）**改写 + 追加**。改写面 = `七臂审计落库` 族的行数与臂序（9 → 7 行、去 `PhaseStart` / `PhaseLog` 两侧位置）、`成功臂与失败臂皆落行` 的 `PhaseStart` 错误行断言（改「零行」）、`reference随行` 的 `phase_log` 两行断言（改「零行」）、`全链落库组合` 的六行臂序（去两侧位置）；追加面 = `resolve_change_name` 合流的四门禁文案等价与零发起断言。装置沿用 `TestDb`（真实 workspace 库）+ `FakeStore`（进程内假件，可注入 `StoreFault`）+ `FakeRunner` / `FakeTestExecutionRunner`（可编程产出）+ `assemble` / `drive_seven_arms` / `run_step` 辅助。

#### 待测功能

- `resolve_change_name(store: &dyn ChangeStateStore, change_id: &str, what: &str) -> Result<String, String>`（新增，私有）: 四门禁臂（`StaticCheck` / `TestExecution` / `MutationTest` / `ArchiCheck`）的 id→name 解析与「未建档」文案合流单点；`what` ∈ `"静态检查"` / `"测试门禁"` / `"突变门禁"` / `"架构漂移门禁"`，合成文案 `change "<id>" 未建档（无 ChangeRecord），无从执行<what>` 与原四处逐字等价。
- `ToolCommand::PhaseStart` 分支（修改）: 写面直调 `workflow::write::phase_start` 与产出返回**逐字保留**，**`audit_outcome` 调用删除**（审计面归零，D11）。
- `ToolCommand::PhaseLog` 分支（修改）: 同上（写面直调 `phase_log` + 产出返回保留，`audit_outcome` 删除）。
- `其余七命令分支`（回归）（`PhaseNext` / `Backtrack` / `DecisionLog` / `StaticCheck` / `TestExecution` / `MutationTest` / `ArchiCheck`）: `audit_outcome` 照常落行，摘要格式与 `reference` 规则逐字不变。
- `audit_outcome` / `clip_summary`（回归）: 函数本体与 500 字符截断留痕逐字不动（余下七臂仍消费）。

<!-- design.md「公共函数 / API」表明写本变更零公共 API 面变化（无本文件行）；上列条目取自同文件「内部函数签名（非导出）」表的 `resolve_change_name` 行、`修改文件` 表的 `core/orchestration/src/steps.rs` 行（①②）与 D11 决策（审计归零点），四项「回归」条目取自 proposal「不要修改」面与 AC-9。`audit_outcome` / `clip_summary` 不在 design 内部签名表内，其条目来自 `修改文件` 行「函数本体与其余七个命令分支照常留审计」一句——不作导出面推导。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `resolve_change_name` | 正向 | 四门禁臂合流：已建档记录（`id ≠ name`）→ 各臂均把记录 `name` 交磁盘面 runner（`roots_handle` / runner 捕获面）；合成「未建档」文案与原四处**逐字等价**（保留原 `change "<id>" 未建档（无 ChangeRecord），无从执行<what>` 结构） | 参数化迭代（4 臂 × 4 what） |
| `resolve_change_name` | 异常 | 未建档 → `Err` 携该 `what` 对应门名且 **零 runner 发起**（假 runner 调用计数 0）、零 `StepRecord` error 行差异（除既有七臂外无新增）；`StoreFault` 注入（`get_change` 缝）→ `Err(error.to_string())` 透传、零 runner 发起 | 参数化迭代（2 形态） |
| `resolve_change_name` | 边界 | `change_id` 边界：`""` / `"   "` / `"不存在的-change"` / 超长（1200 chars）/ 含 emoji 与 `\0` → 文案逐字携原 id（不裁剪 / 不 trim）、不 panic；`what = ""` 退化 → 尾句为「无从执行」（不 panic，仅登记退化形态，生产四调用点不产该值） | 组合迭代（5 id × 2 what） |
| `ToolCommand::PhaseStart` 分支 | 正向 | 写面仍被直调：命令发出 → 返回 `ToolStepOutput::PhaseStart(outcome)` 且 `TestDb` 内 `active_phase` 在案（`phase` / `attempt = 1` / `start_at > 0`）——写通道保留的判据 | 单次 |
| `ToolCommand::PhaseStart` 分支 | 异常 | **审计面归零**（AC-2 / D11）：同一命令发出后 `list_change_steps(change_id, run_id)` 内 `StepKind::PhaseStart` 行数 **0**（原「恰一行 status ok」断言改写为「零行」）；`list_steps` 缝同判；余下臂行数不变（命令总数 9 → 审计行 7）；未建档时 `Err` 携「未建档」记因上抛（写面错误传播不变）且**零审计行**（原「error 行携记因」断言改写为缺席断言，该 id 桶零行） | 参数化迭代（2 形态：成功 / 未建档） |
| `ToolCommand::PhaseStart` 分支 | 边界 | phase 串边界：`""` / `"   "` / 未知相位 / 超长（1200 chars）/ 含 emoji → 写面 `Err` 上抛、零审计行、零 panic；`run_id = ""` 时审计面仍零行（归零点与 run 维无关） | 参数化迭代（5 形态） |
| `ToolCommand::PhaseLog` 分支 | 正向 | 写面仍被直调：命令发出 → 返回 `ToolStepOutput::PhaseLog(outcome)`（`PhaseLogOutcome.attempt` 在场）、`active_phase` 经命令链清位（`find_change_record(..).active_phase == None`）——写通道保留的两判据 | 单次 |
| `ToolCommand::PhaseLog` 分支 | 异常 | **审计面归零**：`StepKind::PhaseLog` 审计行 **0**（原两行 `reference` 断言改写为缺席断言）；未建档 → `Err` 上抛且零审计行；`list_steps` 缝同判（七臂桶内无 `phase_log` 词） | 参数化迭代（2 形态） |
| `ToolCommand::PhaseLog` 分支 | 边界 | 落账载荷边界：`skipped = true` + 空 checklist / `skipped = false` + 单条目 / 三会话槽全 `None` vs 单槽在场 → 三者均 `Ok` 且零审计行（载荷面差异不改变审计归零判据）；`report` 恰 `MAX_REPORT_CHARS`(2000) → 写面接受、零审计行 | 参数化迭代（4 形态） |
| 其余七命令分支（回归） | 正向 | 七臂审计照常：`PhaseNext` / `Backtrack` / `DecisionLog` / `StaticCheck` / `TestExecution` / `MutationTest` / `ArchiCheck` 各一行（status `ok`、`run_id` 串链、`timestamp > 0`、摘要非空）；摘要格式逐字（`phase_next → …（round N）`、`static_check passed=…`、`test_execution conclusion=… total=… passed=… failed=… skipped=…`）；`reference` 规则不变（报告目录 / 会话 id / `None` 三形态）；臂序与命令序逐一对应（去两侧辅助步位置后 7 行） | 参数化迭代（7 臂） |
| 其余七命令分支（回归） | 异常 | 失败臂皆落行（回归）：runner `Err`（`FakeRunner::erring`）→ `status = "error"` + 摘要携错误串 + `reference = None`；写面 `Err`（未建档 `Backtrack` / `DecisionLog`）→ 同行形；`StoreFault` 注入 → 业务 Err 记因上抛且审计行仍落（`append_step` 未故障） | 参数化迭代（3 形态） |
| 其余七命令分支（回归） | 边界 | 摘要阈值与归键边界：恰 500 / 501 / 600 字符 → 不截断 / 截断留痕两态（`chars().count()` 口径）；`change_id = ""` 桶零行（空串占位退役判据）；超长 `change_id`（1200 chars）→ 审计行按原 id 归键 | 参数化迭代（4 形态） |
| `audit_outcome` / `clip_summary`（回归） | 正向 | 摘要 ≤500 原样透传；参考值与状态映射正确（`Ok((summary, ref))` → `"ok"` + summary + ref；`Err(e)` → `"error"` + `e.to_string()` + `None`）——两既有用例断言零改动即绿 | 参数化迭代（2 形态） |
| `audit_outcome` / `clip_summary`（回归） | 异常 | 审计失败不阻断臂（回归）：`append_step` 缝 `StoreFault` → 业务产出仍返回 `Ok`（`let _ =` 语义），零 panic、零 Err 传播 | 单次 |
| `audit_outcome` / `clip_summary`（回归） | 边界 | 截断边界：恰 500 不截断、501 → `…（截断，共 501 字符）`；`clip_summary` 在余下七臂仍被调用（无 `dead_code`，源自证命中 ≥ 7 处调用点） | 参数化迭代（2 形态） |
| 组合：审计面 → store 链 | 正向 | 真件组合（执行文件 `steps_test.rs`）：七臂 + `PhaseStart` + `PhaseLog` 命令各一（共 9 命令）→ 真实 `TestDb` 内 `StepRecord` 总数 **7**（原 9）、按 change id 单桶可枚举、`StepKind::PhaseStart` / `PhaseLog` 零行；`active_phase` 开 / 清交替在案 | 单次 |
| 组合：审计面 → store 链 | 异常 | 词面仍可解码（防「顺手清词」）：`StepKind::PhaseStart.as_str() == "phase_start"` / `PhaseLog == "phase_log"`（存量行解码必需）；`list_change_steps` 对存量形态行（手工经写面/直构造一条 `phase_start` 审计行）仍可读出——「新写面零产出」与「存量可解码」两判据并列 | 单次 |
| 组合：审计面 → store 链 | 边界 | 全命令链失败路径：`fail_on` 各标签（`phase-late` 类注入）逐标签 → `StepRecord` 总数仍 7（`PhaseStart` / `PhaseLog` 分支无论成功失败皆零行，含错误路径）；`active_phase` 在 `PhaseStart` 失败时不被写入（写面事务性） | 参数化迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ChangeStateStore`（`FakeStore` 手写假件） | 既有假件：`seed_change` / `fail_log_phase` / 审计行桶捕获——store 是**进程边界（DB）**且是 `LocalToolSteps::new` 的显式注入依赖 | `resolve_change_name` 全部行、`StoreFault` 注入行 |
| `StaticCheckRunner` / `TestExecutionRunner`（`FakeRunner` / `FakeTestExecutionRunner`） | 既有假件：`passing()` / `erring(msg)` 可编程产出、调用计数——runner 是**子进程边界** | 四门禁臂行、摘要截断行 |
| 不 mock：`LocalToolSteps` | 真实实现（被测面本体）：审计行为与写面直调是它的方法体 | 全章节 |
| 不 mock：`workflow::write::{phase_start, phase_log, backtrack, decision_log}` | 真实写面（经 `TestDb` 真实 workspace 库）——「写通道保留、审计面归零」两半分别落在写面与 `steps.rs`，替身任一侧即空转 | 正向行、组合行 |
| 不 mock：`audit_outcome` / `clip_summary` | 真实实现（回归登记面）：替身化会让「余下七臂照常留审计」自证 | 回归行 |
| 不 mock：`vcs`（`FakeVcs::clean()`）| 既有假件但有真实调用面：`MutationTest` / `ArchiCheck` 臂的变更集取数走它（vcs 为进程边界） | 两门禁臂行 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

既有 sidecar（3522 行 / 76 用例）**改写 + 追加**。改写面 = 三处序表常量（`INITIALIZE_STEP_ORDER` 5 → 3、`CODE_ANALYZE_STEP_ORDER` 5 → 2、`ARCHIVE_STEP_ORDER` 11 → 9）+ `expected_steps(phase)` 表（两表 17 相位步序）+ 各相位步数 / 首末步断言（`archive 十一步…` → 九步、`initialize 五步` → 三步、`code-analyze` 五步 → 两步、`P→E` 五步 → 两步、`test-execution` 四步 → 两步）+ `on_fail` 组四（archive 11 → 9）。追加面 = 「辅助步零声明」遍历守卫 + doc 注释步序串源自证（AC-1 的第二判据面）。其余用例（相位序 / 前置依赖 / 白名单 / 产物清单 / prompt 装配 / 常量锚定）零改动即回归判据。

#### 待测功能

- `11 张 *_STEPS` 静态表（修改，11 个 const 逐表步数见下）: `INITIALIZE_STEPS` 3 步 / `PROPOSAL_STEPS` 2 / `DEV_DESIGN_STEPS` 2 / `TEST_DESIGN_STEPS` 2 / `IMPLEMENT_STEPS` 4 / `TEST_GEN_STEPS` 3 / `TEST_EXECUTION_STEPS` 2 / `CODE_REVIEW_STEPS` 1 / `ACCEPTANCE_STEPS` 1 / `ARCHIVE_STEPS` 9 / `CODE_ANALYZE_STEPS` 2——全部 `PhaseStart` / `VerdictGate` / `PhaseLog` 项删除（30 处）。
- `phase_table(workflow_type)`（体内零变、数据面变）: requirement 表 10 相位步数 3/2/2/2/4/3/2/1/1/9（29）；test-only 表 7 相位 3/2/2/2/3/2/9（23）。
- `StepDefinition.on_fail`（声明面，值不变）: static-check `FeedbackLoop{FEEDBACK_LOOP_LIMIT=5}`（implement / test-gen）、archi-check `FeedbackLoop{ARCHI_FEEDBACK_LIMIT=3}`、test-execution / mutation-test `Escalate`（test-execution 相位）与 `Terminal`（initialize 相位）、archive 九步 `None`。
- `各表 doc 注释步序串`（修改）: 「开相与相位收口不在 `steps` 内（`enter_phase` / `settle_phase` 承载）」；`ARCHIVE_STEPS` 九步、`TEST_EXECUTION_STEPS` `TestExecution → MutationTest`；`ARCHI_FEEDBACK_LIMIT` doc 去「verdict 的 5 次」计数项。

<!-- design.md「公共函数 / API」表明写本变更零公共 API 面变化（无本文件行）；上列条目取自 `修改文件` 表 `core/workflow/src/write/phase_table.rs` 行与 `类型定义` 表的 `StepNode` 行（同文件 `StepDefinition` / `PhaseDefinition` 结构零变）、以及 desktop-workflow-write-face delta 的 MODIFIED requirement。doc 注释面条目（第四条）来自 `修改文件` 行末句与 tasks 阶段四，行为面以「源自证」用例承载（见下表末行）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 11 张 `*_STEPS` 静态表 | 正向 | **步数逐表**（AC-1）：requirement 表 10 相位 = `[3,2,2,2,4,3,2,1,1,9]`（合计 29）；test-only 表 7 相位 = `[3,2,2,2,3,2,9]`（合计 23）；initialize 两表共用同一 static（3 步） | 参数化迭代（17 表项） |
| 11 张 `*_STEPS` 静态表 | 异常 | **辅助步零声明守卫**：遍历两表全部 `steps`，统计 `PhaseStart` / `PhaseLog` / `VerdictGate` 出现次数合计 **0**（现状 30）；同断言对 `phase_table("requirement")` 与 `phase_table("test-only")` 各跑一遍（漏改一表即红，R7 的守卫面） | 参数化迭代（2 表） |
| 11 张 `*_STEPS` 静态表 | 边界 | 表内元素边界：单步表（code-review / acceptance 各 1 步）→ 首步即末步、`steps.len() == 1`、零 `PhaseStart` / `PhaseLog` 边界项；`IMPLEMENT_STEPS` 4 步内 `ArchiCheck` 座位 = static-check 之后、evaluator 之前（D6 保留判据）；`Cleanup` 位次仍介于 `SpecSync` 与 `Seal`；`ARTIFACT` 清单与 `steps` 步数解耦（改 `steps` 不动产物清单） | 参数化迭代（4 形态） |
| 序表常量 / 步序 | 正向 | 六处序表逐位：`INITIALIZE_STEP_ORDER = [Clean, StaticCheck, TestExecution]`；`ARCHIVE_STEP_ORDER = [Preflight, Commit, Rebase, ResolveConflicts, Merge, SpecSync, Cleanup, Seal, Finalize]`（AC-7）；`CODE_ANALYZE_STEP_ORDER = [Executor, Evaluator]`；proposal / dev-design / test-design = `[Executor, Evaluator]`；`IMPLEMENT_STEP_ORDER = [Executor, StaticCheck, ArchiCheck, Evaluator]`；`TEST_GEN_STEP_ORDER = [Executor, StaticCheck, Evaluator]`；`TEST_EXECUTION_STEP_ORDER = [TestExecution, MutationTest]`；code-review / acceptance = `[Evaluator]`；各序表长度与 `steps.len()` 一致 | 参数化迭代（9 序表） |
| 序表常量 / 步序 | 异常 | 反向断言（防「改表不改断言」）：`INITIALIZE_STEP_ORDER` 首元素 **≠** `PhaseStart` 且末元素 **≠** `PhaseLog`；`ARCHIVE_STEP_ORDER` 首 **≠** `PhaseStart`、末 **≠** `PhaseLog`；任一序表内 `assert!(!order.contains(&PhaseStart/&PhaseLog/&VerdictGate))` 三判据全绿 | 参数化迭代（9 序表 × 3 词） |
| 序表常量 / 步序 | 边界 | 索引边界：`order[0]` / `order[len-1]` 双向断言（首末步不经位置推导绕过）；`ARCHIVE_STEP_ORDER.len() == 9` 恰为常量算式（零字面 9 散落）；`TEST_EXECUTION_STEP_ORDER.len() == 2` 与 `ARCHIVE` 末尾 `Finalize` 各自独立断言 | 参数化迭代（3 形态） |
| `StepDefinition.on_fail` | 正向 | 回归：`FeedbackLoop{5}`（implement / test-gen 的 static-check）、`FeedbackLoop{ARCHI_FEEDBACK_LIMIT}`（archi-check）、`Escalate`（test-execution 相位两步）、`Terminal`（initialize 两检查步）、archive 九步 `None`；`FEEDBACK_LOOP_LIMIT == 5` / `ARCHI_FEEDBACK_LIMIT == 3` 常量锚定不变 | 参数化迭代（5 组） |
| `StepDefinition.on_fail` | 异常 | 辅助步退场后**零 `on_fail` 声明随之消失**的证明：原 `VerdictGate` 的 `FeedbackLoop{FEEDBACK_LOOP_LIMIT}`（proposal / dev-design / test-design）与 `Terminal`（code-analyze）声明已一并删除——断言四相位内**不存在** `on_fail != None` 的步（若残留即红）；archive 九步零 `Escalate` / 零 `FeedbackLoop` | 参数化迭代（5 相位） |
| `StepDefinition.on_fail` | 边界 | 同一步词跨表语义不同（`MUST NOT` 按相位名判断）：`TestExecution` 在 initialize 相位 = `Terminal`、在 test-execution 相位 = `Escalate`；`StaticCheck` 在 initialize 相位 = `Terminal`、在 implement / test-gen = `FeedbackLoop{5}`——四处逐断言 | 参数化迭代（4 形态） |
| `phase_table` 其余面（回归） | 正向 | 相位序（10 / 7 相位 id 序）、`prerequisites` 自描述、`dependents` BFS、回溯白名单（两表各按本表序、initialize 恒排除）、`artifact_manifest` 逐相位、`executor` / `evaluator` 分类（DUAL_ROLE 5 / EVAL_ONLY 2 / DETERMINISTIC 3）、模型档位对照、prompt 静态非空——既有断言零改动全绿（步数收缩不改相位序与角色面） | 参数化迭代（既有用例集） |
| `phase_table` 其余面（回归） | 异常 | 未知 workflow_type → `None`（既有：`""` / `"   "` / 超长 / 大小写混写 / 特殊字符五形态）；`prerequisites` 未知相位 → 空集 / `Err`（既有）；`dependents` 未知相位与空表 → 空 `vec`（既有）——零改动 | 参数化迭代（既有用例集） |
| `phase_table` 其余面（回归） | 边界 | 相位定界：`initialize` 零前置依赖且 `backtrackable == false`；表末 `archive` 的 `dependents` 为空集；两表 `INITIALIZE_STEPS` 共享同一 `'static`（指针相等或逐字段相等）；`phase_table` 幂等（同参数两次调用逐字段相等） | 参数化迭代（4 形态） |
| 各表 doc 注释步序串 | 正向 | 口径串在场：`INITIALIZE_STEPS` doc 含「开相与相位收口不在 `steps` 内（`enter_phase` / `settle_phase` 承载）」；`ARCHIVE_STEPS` doc 含「九步」/「9 步」与「开相 / 落账移出」；`TEST_EXECUTION_STEPS` doc 含 `TestExecution → MutationTest` | 参数化迭代（3 表） |
| 各表 doc 注释步序串 | 异常 | 源自证（读 `phase_table.rs` 原文）：`PhaseStart →` / `PhaseLog` / `VerdictGate` 的**步序串**零残留（只允许出现在 `decode-only` 说明句内）；`verdict 的 5 次` 零命中（`grep -c` 口径的测试侧等价断言）；`十一步` / `五步序` / `四步` 类旧口径串零残留 | 参数化迭代（3 判据） |
| 各表 doc 注释步序串 | 边界 | `ARCHI_FEEDBACK_LIMIT` doc 边界：去「verdict 的 5 次」后仍含「与 static-check 的 5 次互不占用」（正向词在场）；`IMPLEMENT_STEPS` 注释同判；「verdict」字样在该两处 doc 零命中 | 参数化迭代（2 doc） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为 `const` / `static` 静态表与纯查表函数（`phase_table`），无 IO、无注入依赖、无时钟；全部真值直取 | 全章节 |
| 不 mock：`phase_table("requirement")` / `phase_table("test-only")` | 真实静态表——上位判据（步数 / 零声明 / 相位序）的唯真值源；以假表替代会让 AC-1 空转 | 全章节 |
| 文件读（源自证） | `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string` 读本 crate 产品源（既有 `prompt_text` / `prompts_dir` 装置先例）——读的是**产品文件原文**，非替身 | doc 注释判据行 |

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

既有 sidecar（1926 行 / 38 用例）**追加守卫用例 + 注释在场面自证**；`step_node二十二值全词汇snake_case逐字` / `step_node与run_step_kind逐字同源二十二值` / `step_node分组与角色派生访问器一致` / `step_group四值线面…` / `非法词反序列化拒绝_封闭集防线` 五组断言**零改动**即回归判据（本文件仅注释改动，零语义）。

#### 待测功能

- `StepNode`（enum，修改仅注释）: 22 值保持、序不变（`PhaseStart` / `PhaseLog` / `VerdictGate` 标 `decode-only`）；`as_str()` / serde 线词逐字不变。
- `StepGroup::Gate` / `StepGroup::Exec` / `StepNode::group()`（修改仅注释）: 枚举值与三词归属分支保留（`VerdictGate` / `RetryGate` / `WhitelistGate` → `Gate`）；分支注释说明「辅助步退场后无 `StepNode` 归属，词汇面仍冻结」。
- `RunStepKind`（enum，修改仅注释）: 22 值、bincode 下标钉死；五词（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate`）`decode-only` 注释。

<!-- design.md「公共函数 / API」表明写本变更零公共 API 面变化（无本文件行）；上列条目取自 `修改文件` 表 `core/workflow/src/state.rs` 行（明写「仅注释」）与 `类型定义` 表 `StepNode / RunStepKind / ChangeStepKind` 行，以及 desktop-workflow-write-face delta 的 MODIFIED requirement。零语义改动意味着**行为断言应零改写**——本节的追加用例全部落在「值集 / 序 / 归属 / 注释在场」四类守卫上，不新增行为面条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `StepNode` 22 值 | 正向 | 既有回归：22 值 snake_case 线词逐字（含 `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate`）、与 `RunStepKind` 逐字同源、`as_str()` 同词面——三组断言零改动 | 参数化迭代（22 值） |
| `StepNode` 22 值 | 异常 | **防回退守卫**（AC-6）：值集长度恰 22 且**变体序固定**（按文档声明序逐位比对，删变体 / 重排即红）；非法线词（`"PhaseStart"` PascalCase / `"phase-start"` 连字符 / `""` / `null`）反序列化拒绝（既有封闭集防线零改动）；三词在**计划图声明面**零引用由 `phase_table_test` 的零声明守卫承载（本节只锁词汇本体） | 参数化迭代（22 位序 + 4 非法词） |
| `StepNode` 22 值 | 边界 | 序边界：首变体 `Executor`、末变体 `ArchiCheck`（尾部追加先例不变）；`PhaseStart` / `PhaseLog` / `VerdictGate` 三词的**位置下标**与改造前一致（`PhaseStart` 第 4 位、`PhaseLog` 第 9 位、`VerdictGate` 第 10 位——bincode 下标稳定判据）；`retry_gate` 第 11 / `whitelist_gate` 第 12 位不变 | 参数化迭代（5 下标） |
| `StepGroup::Gate` / `StepGroup::Exec` / `StepNode::group()` | 正向 | 既有回归：四分类线词 `exec` / `agent` / `git` / `gate`；分组计数 `Exec 8 / Agent 5 / Git 6 / Gate 3`；三 gate 归 `Gate` | 参数化迭代（4 组） |
| `StepGroup::Gate` / `StepGroup::Exec` / `StepNode::group()` | 异常 | 分支不可删的反向判据（R9）：`StepGroup::Gate` 枚举值仍在（serde 出线 `"gate"`、`from_str` 回转）；三词 `group()` **不得**改归 `Exec` / `Agent` / `Git`（`assert_ne!` 三向）；`role()` 三词恒 `None` | 参数化迭代（3 词 × 3 反向） |
| `StepGroup::Gate` / `StepGroup::Exec` / `StepNode::group()` | 边界 | 计数边界：22 值分组计数和 = 22（`8+5+6+3`，无遗漏无重叠）；`StepGroup::Exec` 的 doc 已去 `phase-start` / `phase-log` 举例但 `PhaseStart` / `PhaseLog` 实测仍归 `Exec`（注释改动不改归属——两判据并列） | 参数化迭代（2 形态） |
| `decode-only` 注释在场（源自证） | 正向 | 读 `state.rs` **原文**：`StepNode` 的 `PhaseStart` / `PhaseLog` / `VerdictGate` 三变体邻域各含 `decode-only`；`RunStepKind` 的五词邻域各含 `decode-only` 且含「MUST NOT 删除或重排」（或等价表述）字样 | 参数化迭代（5 词） |
| `decode-only` 注释在场（源自证） | 异常 | 说明面在场：`StepGroup::Gate` 的枚举 doc 与 `group()` 分支含「冻结 / 不删」类说明（含「无 `StepNode` 归属」表述）；`StepGroup::Exec` doc **零** `phase-start` / `phase-log` 举例（旧举例即红） | 参数化迭代（3 说明面） |
| `decode-only` 注释在场（源自证） | 边界 | 注释不等于语义：以「去注释后代码行」再跑一次既有 22 值 / 分组 / 线词断言 → 全绿（证明本文件零语义改动的行为侧证；正向注释与行为断言互不干扰） | 单次 |
| `RunStepKind` 线词 / 序（回归） | 正向 | 既有回归：22 值 snake_case 逐字、`RunStepStatus` 四档线词、`RunRecordStatus` 五值——零改动 | 参数化迭代（既有用例集） |
| `RunStepKind` 线词 / 序（回归） | 异常 | 非法词反序列化拒绝（封闭集防线，既有零改动）；`bincode` 下标稳定的判据（`RunNodeRecord.step` 落库编码）；`StepKind`（审计词面）9 值保持、`phase_start` / `phase_log` 词面仍在（新写面零产出但解码必需） | 参数化迭代（3 形态） |
| `RunStepKind` 线词 / 序（回归） | 边界 | `StepKind` 与 `RunStepKind` 词面差异边界：`StepKind` 无 `clean` / `commit` 等 git 步（9 值 vs 22 值），`PhaseStart` / `PhaseLog` 在两表**各有**词面（不是只在一侧） | 参数化迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为封闭枚举 + 派生访问器 + 线格式（纯函数），无 IO、无端口、无时钟 | 行为行全部 |
| 不 mock：`StepNode` / `RunStepKind` / `StepGroup` 真值直取 | 枚举本体即被测对象；以字符串表替代会让「22 值与序」自证 | 全章节 |
| 文件读（源自证） | `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string` 读 `state.rs` 原文（注释在场面判据必需，故**不剔注释**） | 注释判据行 |

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

既有 sidecar（1789 行 / 42 用例）**追加守卫用例 + 注释在场面自证**；`change_step_kind二十二值词汇线格式逐字且四类可辨` / `change_step_kind二十二值词汇互异无碰撞` / `change_step_kind非法词反序列化拒绝` / `change_step_kind归档九步词camelcase与snake同源` 四组断言**零改动**即回归判据（本文件仅注释改动，零语义；`run_history.rs` 的 22 值映射表逐字不动）。

#### 待测功能

- `ChangeStepKind`（enum，修改仅注释）: 22 值保持、序不变（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` 标 `decode-only`，注明「出线词与节点身份键的存量行依赖，新写面零产出」）；`as_str()` snake 与 serde camel 双词面逐字不变。
- `decode-only` 注释在场（源自证）（修改，含枚举 doc 的步词汇分族表述）: 五词 doc 邻域 `decode-only`；枚举 doc 表述「新写面只产 WorkerAgent / ToolStep / Git 三类节点，Gate 类为零产出的存量解码词汇」。

<!-- design.md「公共函数 / API」表明写本变更零公共 API 面变化（无本文件行）；上列条目取自 `修改文件` 表 `core/orchestration/src/state.rs` 行（明写「仅注释」）与 `类型定义` 表 `ChangeStepKind` 行，以及 desktop-change-state-store delta 的 MODIFIED requirement。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChangeStepKind` 22 值 | 正向 | 既有回归：22 值 camelCase 线词逐字（`phaseStart` / `phaseLog` / `verdictGate` / `retryGate` / `whitelistGate` 在内的五词逐字）+ snake `as_str()` 同源 + 值与词互异无碰撞——三组断言零改动（AC-6 的出线面判据） | 参数化迭代（22 值） |
| `ChangeStepKind` 22 值 | 异常 | **防回退守卫**：值集长度恰 22 且变体序固定（按声明序逐位比对，删变体 / 重排即红）；`ChangeStepKind` 与 `RunStepKind` 的 22 值 1:1 映射表（`run_history.rs`）逐字不变（`run_history_test` 零改动即侧证，本节补显式序比对）；非法线词（`""` / `"PhaseStart"` / `"phase_start"` snake 形态 / `null`）反序列化拒绝 | 参数化迭代（22 位序 + 4 非法词） |
| `ChangeStepKind` 22 值 | 边界 | 序边界：首变体 `Executor`、末变体 `ArchiCheck`（尾部追加先例）；五词下标稳定（`PhaseStart` 第 4 / `PhaseLog` 第 9 / `VerdictGate` 第 10 / `RetryGate` 第 11 / `WhitelistGate` 第 12 位——节点身份键与 bincode 下标判据）；五词 camel 与 snake 双词面**各有**词（不是只在一侧） | 参数化迭代（5 下标 + 2 词面） |
| `decode-only` 注释在场（源自证） | 正向 | 读 `state.rs` **原文**：五词（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate`）各自 doc 邻域含 `decode-only` | 参数化迭代（5 词） |
| `decode-only` 注释在场（源自证） | 异常 | 语义说明在场：各词注释含「出线词与节点身份键的存量行依赖」或「新写面零产出」类表述（五词至少各一处命中）；缺任一项即红 | 参数化迭代（5 词） |
| `decode-only` 注释在场（源自证） | 边界 | 枚举 doc 分族表述边界：doc 含「新写面只产 WorkerAgent / ToolStep / Git 三类节点」与 Gate 类「存量解码词汇」表述；且 doc **零**「新写面产出 Gate 节点」类相反表述（反向断言） | 参数化迭代（2 doc 判据） |
| `ChangeStepKind` 线格式（回归） | 正向 | 既有回归：`归档九步词` camel 与 snake 同源；`_testexecution` / `mutationTest` / `archiCheck` 专词出线逐字；`RunUpdate` / `RunNotice` 五变体 tag `ipc` 出线——零改动 | 参数化迭代（既有用例集） |
| `ChangeStepKind` 线格式（回归） | 异常 | 封闭集防线：非法词反序列化拒绝（既有）；`ChangeStepStatus` 四值封闭（`running` / `passed` / `failed` / `stopped`）；`RunUpdate` 非法 tag 拒绝——零改动 | 参数化迭代（3 形态） |
| `ChangeStepKind` 线格式（回归） | 边界 | 出线形状边界：`ChangeStepState` 四档状态与缺省字段往返无损；`InstanceNo` 从 `0` 起递增可辨；快照 nodes 全词汇 camelCase 出线（`step` 键值由 `normalizeKind` 消费，五词必在） | 参数化迭代（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为封闭枚举 + 线格式序列化（纯函数） | 行为行全部 |
| 不 mock：`ChangeStepKind` 真值直取 / `RunStepKind` 映射表 | 真值与真映射表（跨 crate 编译强制 1:1） | 全章节 |
| 文件读（源自证） | `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string` 读 `state.rs` 原文（不剔注释） | 注释判据行 |

### packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

<!-- 本节自我映射（测试文件即被测文件）：design「修改文件」表不含本文件（它是测试面，归 test-gen / test-execution 相位），条目来源 = proposal「测试文件」表 golden 行 + AC-11 / AC-12 + tasks「交接备忘」的 corpus 条目 + desktop-corpus-regression delta 的 MODIFIED requirement。用例面落在**语料种子构造器与 golden 断言**（`build_*` / `project_*` / `FULL_WORDS` / `corpus-archive-prefix` 断言族），不新增产品行为断言。 -->

既有 crate 级集成文件（3938 行 / 37 用例）**改写**。改写面 = 真实相位形态样本（`corpus-initialize` 5 → 3 节点、`corpus-archive-prefix` 11 → 9 节点及其 forward 边数）、`corpus-run-history` 的 `FULL_WORDS` 全词汇断言（保留 22 值词汇面，但「真实形态」与「存量解码形态」两口径分列标注）、瞬步样本（`verdict_gate` 的 `startedAt == endedAt` 改为存量解码样本承载，非新写面生产例）、`edges[0]` / `edges[3]` 的身份串断言（`initialize:1:phase_start:0` 类端点随真实形态样本重定）。**种子构造词汇面 MUST NOT 删减**：五词（`phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate`）各至少一个节点样本继续在语料内（存量解码回归面）。装置沿用真实 `Store`（tempdir 库）+ store change / run 域操作面（禁裸表插桩）+ `finish_command` + `DESKTOP_GOLDEN_REWRITE=1` 开关。

#### 待测功能

- `initialize 真实形态样本`（修改）: 三节点 `clean` → `static_check` → `test_execution`（零 `phase_start` / `phase_log`），节点行四字段（时间 / token）两态在案。
- `archive 真实形态样本`（修改）: 九节点 `preflight` → … → `finalize` + 8 条 `forward` 串行边（零 `phase_start` / `phase_log`）。
- `五词存量解码样本`（不变）: `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` 各至少一个节点样本在校（标注存量解码形态），与真实形态样本并列存在、互不替换。
- `FULL_WORDS` 全词汇断言（修改表述）: 二十二值词汇面仍全（`runs[].nodes[].step` 越集即红），但「真实相位形态」与「存量解码形态」两类样本并列、各带注释。
- `golden 重写与复核`（不变）: `DESKTOP_GOLDEN_REWRITE=1` 重写 → 不带环境变量复跑零 diff。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| initialize 真实形态样本 | 正向 | 投影 `runs[].nodes[].step` 序恰 `["clean", "static_check", "test_execution"]`（3 节点、零 `phase_start` / `phase_log`）；前向边 2 条全 `forward`；`attempt` 1 与表序一致（AC-7 同族口径） | 单次 |
| initialize 真实形态样本 | 异常 | 缺席断言（防「旧 golden 静默通过」）：该 run 的 `nodes` 内 `phase_start` / `phase_log` **零出现**；`edges` 内不存在以 `phase_start` / `phase_log` 为端点的边（旧 golden 未重写即红，非静默）；节点总数严格 3（多一即红） | 参数化迭代（3 判据） |
| initialize 真实形态样本 | 边界 | 节点行四字段边界：`startedAt` / `endedAt` 毫秒数值在案、`tokenInput` / `tokenOutput` 两键恒在场（工具步为 `null` 不省键）；在途样本 `endedAt: null` 出线不省键 | 参数化迭代（3 形态） |
| archive 真实形态样本 | 正向 | 投影序恰 `["preflight","commit","rebase","resolve_conflicts","merge","spec_sync","cleanup","seal","finalize"]`（9 节点）+ 8 条 `forward` 边；archive 相位 `PhaseRecord` 空 checklist + `report=""` + `verdict="pass"` 不变（AC-11） | 单次 |
| archive 真实形态样本 | 异常 | 缺席断言：9 节点内零 `phase_start` / `phase_log`；边集全 `forward`（零 `backtrack` / `feedback` / `retry`）；节点数严格 9（原 11 的旧 golden 即红） | 参数化迭代（3 判据） |
| archive 真实形态样本 | 边界 | 位次边界：`cleanup` 位于 `spec_sync` 与 `seal` 之间（尽力步位次不变）；`seal` 与 `finalize` 相邻；首端点 `preflight`、末端点 `finalize` 的边端点身份串逐字（`archive:1:preflight:0` 形态） | 参数化迭代（3 形态） |
| 五词存量解码样本 | 正向 | **双口径并列**（AC-12）：五词各至少一个节点样本在语料内（`phase_start` / `phase_log` 在 run-history 或 run-interrupted 语料、`verdict_gate` / `retry_gate` / `whitelist_gate` 在 run-history 语料）；两类样本注释分列 | 参数化迭代（5 词） |
| 五词存量解码样本 | 异常 | 互不替换判据：五词均**不出现在真实形态样本**（initialize / archive）内；且真实形态样本的步词**不被**用于替代五词样本（删任一侧即红——`seen_words` 断言与「真实形态样本内零五词」断言并列） | 参数化迭代（2 侧 × 5 词） |
| 五词存量解码样本 | 边界 | 样本会计边界：五词样本的 `attempt` / `instanceNo` / 边数与「真实相位形态」样本无重叠（身份键不撞号）；`verdict_gate` 同刻样本（`startedAt == endedAt`）保留在存量侧 | 参数化迭代（3 形态） |
| `FULL_WORDS` 全词汇断言 | 正向 | 二十二值词汇面仍全：语料 `runs[].nodes[].step` 的并集覆盖非归档十二词 + 归档九步词（`seen_words` 逐词断言），越集即红；节点数与边数按真实形态收缩后重定 | 参数化迭代（2 语料族） |
| `FULL_WORDS` 全词汇断言 | 异常 | 越集与缺失双向：任一 `nodes[].step` 不在 22 值封闭集内 → 红（既有越集断言保留）；`seen_words` 缺任一应有词 → 红（词汇面减少即红，防「删样本腾红」） | 参数化迭代（2 方向） |
| `FULL_WORDS` 全词汇断言 | 边界 | 词面计数边界：非归档十二词全部在场（`executor` / `evaluator` / `decision` / `static_check` / `archi_check` / `test_execution` / `mutation_test` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate`）；归档九步词由 `corpus-archive-prefix` 承载（两 golden 分工不重叠） | 参数化迭代（12 词 + 9 词） |
| golden 重写与复核 | 正向 | 重写流程两侧（AC-11）：`DESKTOP_GOLDEN_REWRITE=1` 下产物覆写入仓路径（不对比）；不带环境变量复跑时逐字段等价（零 diff）——本层断言落「重写产物与现仓 golden 等价」 | 单次 |
| golden 重写与复核 | 异常 | 漂移守卫：故意注入非预期差异（如多一节点 / 词面变更）→ 复跑红（对比非静默通过）；重写差异**只允许**步数 / 边数收缩与端点身份串随动（词汇面减少即红） | 参数化迭代（2 形态） |
| golden 重写与复核 | 边界 | 开关边界：环境变量未设 / 设为 `"0"` / 设为 `"1"` 三形态 → 仅 `"1"` 走重写（其余走对比）；重写后同进程再对比亦等价（幂等） | 参数化迭代（3 形态） |
| 瞬步样本 | 正向 | `startedAt == endedAt` 同刻形态由**存量解码样本**（`verdict_gate`）承载且仍出线两键同值 | 单次 |
| 瞬步样本 | 异常 | 新写面零瞬步生产例：initialize / archive 样本内**无** `startedAt == endedAt` 节点（同刻形态只属存量侧）；若新形态样本出现同刻 → 红（判据反转的证据） | 单次 |
| 瞬步样本 | 边界 | `endedAt: null` 的在途样本不省键；`startedAt` / `endedAt` 与 token 两键的 null / 数值四态组合在 golden 上逐形态在案 | 参数化迭代（2 形态） |
| 既有语料维度（回归） | 正向 | 多 attempt（`attempt` 跨 run 递增不撞号）、backtrack stale、槽位全缺、worktree 两态、归档前缀 list 月分组、run 中断标定——六维语料构造与断言零改动（步数收缩只作用于真实形态样本） | 参数化迭代（6 维） |
| 既有语料维度（回归） | 异常 | 缓存 / 坏行防线（既有）：坏行语料不伪造（`fixtures/README.md` 明示由 store 域测试承载）；文档形态零发现反例（list 零呈现、`change_detail` 恒 `None`）不产 golden 的判据零改动 | 参数化迭代（2 形态） |
| 既有语料维度（回归） | 边界 | 时间与 token 定值边界：种子 `t(n)` 族基准槽零 wall-clock；`token` 两态（worker 步带值 / 工具步 `null`）；两次运行输出逐字节稳定（确定性锚） | 参数化迭代（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `store::Store`（tempdir 工作区库） | **真件**（进程边界真库）：语料种子经 store change / run 域操作面植入（禁裸表插桩——既有纪律）；零 mock | 全章节 |
| 不 mock：`finish_command` / `phase_next` / `change_detail` / `list_changes` | 真实产品实现——投影链即被测链（golden 是它与库史的关系式） | 全章节 |
| 不 mock：golden 文件 | 真实入仓 JSON（对比对象本体） | 全章节 |
| 时钟 | 不入语料：种子直构 `ChangeStepState` 定值 / `t(n)` 种子时间族，零 `emit_node` / `now_millis` 触点；重写开关（`DESKTOP_GOLDEN_REWRITE=1`）归桌面端执行、不入用例断言 | 全章节 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` 相对 `HEAD` +1 patch） | 配置 / 元数据面，无运行期行为可断言；判据是**相对递增**（MUST NOT 写死字面号） | 交付门 diff 审查：`git diff HEAD -- packages/desktop/package.json` 版本行恰 `+1` patch；`tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本行零改动 |
| `packages/desktop/src/**`（含 `views/changes/flow/run-state.ts` 的 `STEP_LABEL` 五键与 gate 分组、`change-flow-graph.tsx` 的静默节点判据） | **AC-10 的整树零改动面**：本变更后端零出线变化 ⇒ 前端零 diff；`run-state.ts` 的五键解码是**存量行解码必需**（删键即旧库渲染崩），其行为已由既有 `run-state.test.ts` 锁定——本变更**零改写零新增**（改写即违反 AC-10） | `git diff HEAD -- packages/desktop/src` 为空；`vp test run src/views/changes/flow` 既有用例全绿（含五键在案 + gate 分组断言）；「新 run initialize 列恰三节点」由既有渲染路径天然承载 |
| `packages/desktop/src/types/generated/**`（`bindings.ts`） | specta 生成物零手改；`ChangeStepKind` 值集不变 ⇒ 再生成应零 diff | `pnpm -C packages/desktop run bindings:export` 后 `git diff` 为空（桌面端 static-check） |
| `crates/infra/store/**`（`PhaseRecord` / `StepRecord` / `RunNodeRecord` 形状与 `StepKind` 词面；attempt 推导式子） | 「不要修改」面（词汇冻结即零迁移）：零 diff 即本变更不触碰；其行为已由既有 store 测试锁定 | `git diff` 为空 + 既有 store 测试全绿；attempt 口径的运行期侧证落 `walker_test` 的 `enter_phase` 口径行（真实库） |
| `crates/core/orchestration/src/{port.rs, control.rs, snapshot.rs, run_history.rs, finish.rs, verdict.rs, decision.rs, prompt.rs, transcript.rs}`、`crates/core/workflow/src/{queries/**, model/**, write/{phase_log.rs, phase_start.rs, phase_next.rs, backtrack.rs, archive.rs}}`、`crates/core/workflow/src/prompts/*.md`（14 份）、`crates/infra/agent/**` | 「不要修改」面（整文件零 diff）；`ToolCommand` 封闭集 / `accumulate_node` 合并语义 / `finish_step` 协议 / 14 份角色 md 均不在本变更射程 | `git diff --stat` 逐项为空；`control_test` 的 `accumulate_node` 合并语义断言（`started_at` 保首见）零改写即绿；`finish_test` / `verdict_test` / `decision_test` / `prompt_test` / `transcript_test` / `port_test` 既有套件全绿 |
| **源码零命中类静态判据**（`steps.rs` 的 `StepKind::PhaseStart` / `PhaseLog` 零命中与 `get_change(&change_id)` 恰 1 处；`walker.rs` 的六词 `ChangeStepKind::*` 零命中；`crates/core/workflow/tests/corpus_golden_test.rs` 的种子词汇面零删减；`.dev-team/architecture/models/**` 零改动） | 源码位置事实，运行期不可观测（用例只能覆盖被驱动的路径，覆盖不到「某串不存在」这类**缺席**事实中的**跨文件**部分；同文件内的缺席事实已由 `walker_test` / `state_test` 的源自证用例承载） | 桌面端 static-check 相位执行 `grep -c` / `git diff` 判据（tasks 阶段一 / 二 / 三 / 五的自查项逐条） |
| `run_step` 的 `match` **穷尽性**（22 变体全覆盖） | 编译期事实（`match` 非穷尽即编译失败），无运行期断言面 | `cargo check` / `cargo clippy --all-targets` 绿（桌面端 static-check） |
| `crates/core/workflow/tests/fixtures/README.md`（「解码专用词汇样本 vs 真实相位形态样本」两条目） | 覆盖面矩阵文档，非可执行代码（golden 目录下 `README.md` 同款） | 文档人工复核 + `corpus_golden_test.rs` 的双口径用例（五词样本与真实形态样本并列存在的运行期侧证） |
| `openspec/changes/slim-flow-aux-steps/**`（proposal / design / tasks / 8 份 delta spec 本身） | 规格与设计文档，非代码；本相位只消费不验证 | 合并评审（spec 与实现的 D11 / D12 措辞校正见 design「待决问题 4 / 5」） |
| `enter_phase` / `settle_phase` 等处 doc 注释的**措辞**（相位机形态 / attempt 口径 / 瞬步表述） | 注释面零运行期路径；源自证只能判「关键串在场」，判不了措辞是否自洽 | 桌面端 static-check 读码复核（tasks 阶段四自查项）；「attempt 口径」的运行期侧证由 `enter_phase` 口径行承载 |
