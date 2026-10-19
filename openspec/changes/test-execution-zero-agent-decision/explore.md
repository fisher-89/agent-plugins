# test-execution 移除 agent 执行、失败走决策回溯

workflow_type: requirement

## 1. 需求

`openspec/todo.md:73`：

> 【Desktop】移除test-execution阶段的agent执行，测试不通过时，走决策回溯

即：test-execution 相位内零 WorkerAgent 会话；测试红 / 覆盖率不达标 / 突变未达标一律升格相位 fail，把裁决权交回决策面。

## 2. 目标态

```
phase_next → next = test-execution
   PhaseStart
   TestExecution ──pass──► cursor.test_outcome = Some
        ├ fail ──► failed 节点 → 代写 fail phase-log → StepFlow::Upgraded（截断余下步）
        └ runner Err ──► Terminal 停车（环境问题，不进决策）
   MutationTest  ──pass──► cursor.mutation_outcome = Some（仅上一步 pass 才走到）
        ├ fail / error ──► 同上
        └ runner Err ──► Terminal 停车
   PhaseLog（两步全 pass，机械 checklist 落 pass）
        ↓
   phase_next: VerdictFail（evaluator 在场 && 最新 fail 未挂决策 && 窗口 fail < 5）
        ↓
   decision_session（ModelLevel::High）循环：
        决策会话 → parse
          ├ parse Err ──► drift++ ──► 软拒回注（同会话续注）
          ├ WhitelistGate 节点：越权 / 自环 ──► drift++ ──► 软拒回注
          │                                    drift > 2 ──► Terminal
          └ Ok ──► backtrack / stop / ask
```

**相位内 WorkerAgent 会话数 = 0**；唯一的 agent 是决策会话本身。

## 3. 定论

| # | 议题 | 结论 | 依据 |
|---|---|---|---|
| 1 | 决策主体 | 决策 agent 裁决（backtrack / stop / ask） | 保留根因判断，与既有 `pipeline-backtrack` 契约同源 |
| 2 | MutationTest | 一并移除（反馈边②同步骤） | 否则「相位内零 agent」不成立 |
| 3 | retry 语义 | **从决策协议中退役**；「重做本相位」改由「白名单包含自身」表达 | 一个概念一个载体 |
| 4 | 失败策略表表达 | 从**步序列派生**（`role().is_some()`），零新字段 | `PhaseDefinition` 一行不改 |
| 5 | agent 输出 retry | 软拒回注（同会话重出一轮），drift > 2 才 Terminal | 一次模型漂移不该打死 run |
| 6 | self-backtrack 摘除点 | `allowed_backtrack_phases`（表侧单一源） | 写面 `backtrack` 二次校验天然继承 |
| 7 | flaky | 不给 `rerun` 口子 | 测试不允许 flaky，必须回退修复 |
| 8 | `EdgeKind::Retry` 存量 | **变体保留**（解码契约），查询投影处屏蔽 | 删变体 ⇒ 旧行 decode 失败 ⇒ 历史流程图整份打不开 |
| 9 | 库格式 | **不变（v3）**，不 bump、不建 from 链 | 旧库作废是项目初期无人使用时代的产物；今后须保兼容 |
| 10 | `fail_decided` | **保留**（改注释） | 承重墙，见 §4.6 |
| 11 | 循环检测 | 不做周期检测；保持 5 轮单相位计数 + `MAX_ROUNDS=20` 兜底 | token 损失已封顶 |
| 12 | 步失败语义 | 新增 `OnFail::Escalate`（无载荷） | 见 §4.1 |

## 4. 详细设计

### 4.1 `OnFail::Escalate`

`OnFail` 现有两个变体覆盖不了第三种处置（`workflow/src/state.rs:389-406`）：

| 变体 | agent 会话 | 落 fail 条目 | run 终态 | 进决策 |
|---|---|---|---|---|
| `FeedbackLoop{limit}` | 有（注入修复） | 超限时 | 回路由 | 是 |
| `Terminal` | 零 | **不落** | failed 停车 | **否** |
| **`Escalate`**（新） | **零** | **落** | 回路由 | **是** |

语义：**本相位本轮判负，落账后交给决策面**——不修复、不停机、不跑 evaluator。

```rust
pub enum OnFail {
    FeedbackLoop { limit: u32 },
    Terminal,
    Escalate,          // 无载荷：升格动作完全固定，无可调项
}
```

升格动作固定三段：代写 fail checklist（一条，evidence = 诊断摘要 + 报告目录）→ `phase_log` 落账 → `StepFlow::Upgraded` 截断本相位余下步回 `phase_next`。

**为什么不用 `FeedbackLoop{limit:0}`**：技术上零代码改动即可工作（首次失败 `0>=0` 成立，`run_worker` 前 `return`），但那是「禁用」不是「移除」——`loop { run_worker(..) }`、`fix_prompt` 构造、`feedback += 1` 仍留在代码里，只是运行时不达。且 `{limit:0}` 的声明读起来像 bug。

**同一步词、不同表项不同语义**（表内自描述，无需相位名判断）：

```
{ id: "initialize",      steps: INITIALIZE_STEPS }     → TestExecution: Terminal
{ id: "test-execution",  steps: TEST_EXECUTION_STEPS } → TestExecution: Escalate
```

`step_test_execution` 保留两个前置分支并存（`if Terminal {...}` / `if Escalate {...}`），与既有 `step_verdict_gate` 的 Terminal 分支同形。

**守卫**：`on_fail` 只有 static-check / test-execution / mutation-test / verdict-gate 四个执行体读，误声明会静默失效 → `phase_table_test` 加断言：`Escalate` 只出现在 `TestExecution` / `MutationTest` 上。

**不上 IPC**：`OnFail` 不在 `bindings.ts`，`StepDefinition` 无 serde 派生 → 前端零影响。

### 4.2 白名单自排除（retry 语义的唯一载体）

```rust
pub fn allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String> {
    let Some(idx) = table.iter().position(|p| p.id == current_phase) else { return Vec::new() };
    if idx == 0 { return Vec::new(); }
    // 「自身在白名单内」= 支持重做本相位。零 agent 步的相位没有能改变结论的执行体
    // → 自身不入白名单 → 决策 agent 上下文里不存在「重做本相位」这个选项。
    let redoable = table[idx].steps.iter().any(|step| step.node.role().is_some());
    table[..=idx]
        .iter()
        .filter(|p| p.backtrackable)
        .filter(|p| redoable || p.id != current_phase)
        .map(|p| p.id.to_owned())
        .collect()
}
```

- **摘掉自身**：`test-execution`、`archive`（`initialize` 本就 idx==0 早退空）。
- **保留自身（= 可重做）**：proposal / dev-design / test-design / implement / test-gen / code-analyze / code-review / acceptance。
- 白名单缓存链零额外传参：`drive` 缓存的 `allowed` 来自「上一站正常路由」的下发值，而失败相位恒为上一站 ⇒ 自排除自动生效。

### 4.3 决策协议：四动作 → 三动作

```json
{"action": "backtrack", "to": "<白名单内相位 id>", "reason": "<=500"}
{"action": "stop", "reason": "<=500"}
{"action": "ask", "question": "...", "options": ["..."]}
```

test-execution 失败时白名单渲染（`prompt.rs:decision_prompt`）：

```
## 可回溯白名单（仅可从中选择 backtrack 目标）

- proposal
- dev-design
- test-design
- implement
- test-gen
```

自身不在列 ⇒ `retry` 这个词从 agent 上下文里消失。对可重做相位需自注记，避免误判自环非法：

```
- implement（= 重做本相位）
```

`DecisionAction::Retry` 变体删除。`{"action":"retry"}` 落入「未知动作」→ 软拒一次。

### 4.4 软拒环：白名单门迁入决策会话

现状 parse → 白名单门 → 越权即 `Terminal::failed`（`walker.rs:3063-3098`）。改为 `decision_session` 内闭环：

```
loop {
  run_worker(Decision, prompt, continue_session)
  decision-log 挂账（幂等覆写）
  match parse_decision(text) {
    Err(e)        => drift+=1; 超限->Err; prompt = decision_fix_prompt(e)
    Ok(action)    => {
      emit WhitelistGate(running)                    // 每轮一个节点实例
      Err(msg)    => drift+=1; 超限->Err;
                     emit WhitelistGate(failed,msg);
                     prompt = decision_fix_prompt(msg)
      Ok(())      => emit WhitelistGate(passed); return Ok(action)
    }
  }
}
```

- `DECISION_DRIFT_LIMIT = 2`；**ask 轮次不计数**（用户驱动的停等）。
- 拒绝文案两类：自环（`to == 相位` 且不可重做）→「本相位为确定性门禁相位（零 agent 步），不支持重做；请从白名单其他相位中选择，或 stop / ask」；其他越权 → 沿用现状文案（携 allowed 列表）。
- 图面证据：`WhitelistGate` 每轮 emit（`instance_no` 递增），驳回可见。

### 4.5 决策信息面（必须同批补，否则决策质量无底）

现状 `build_decision_input`（`walker.rs:3249`）只取 checklist 失败项，`record.report` 不进决策 prompt，evidence 是 `diagnose_brief`（200 字硬截断）。决策 agent 只凭 ~200 字判断根因，且 prompt 未告知可读文件。

三处改进：

1. 升格落账的 `report` 字段放**完整** `findings_detail` / 突变 `feedback_text`（<=2000，`MAX_REPORT_CHARS`）；evidence 放 brief + `report_dir`。
2. `DecisionInput` 增 `fail_report: String`；决策 prompt 增「失败相位报告」段。
3. 决策 prompt 增一句「可用 read 读 `reports/test/summary.json` 与突变报告」。

`TestExecutionOutcome.findings_detail` 移除反馈边后失去唯一消费者，正好改喂这里的 report 字段。

### 4.6 重入路径与循环边界

`fail_decided`（`phase_next.rs:184-190`）**保留**——它是「**每个 fail 条目只触发一次决策**」的幂等闸，不是 retry 的附属品。删掉后的实际后果：

```
test-execution #1 fail        → entry#1(fail, 无决策槽)
phase_next 命中 VerdictFail    → 决策 → backtrack 到 implement
implement 重做 pass           → entry#2(pass)
phase_next: next = test-execution
   latest_fail 仍是 entry#1（该相位最新条目）→ 又命中
   → 又一次决策，test-execution 根本没被执行
   → 烧到 MAX_ROUNDS=20 硬失败
```

即删掉它，**任何 fail 过的相位都无法再被执行**。仅改注释：删「retry 决议兑现」字样，语义改为「该 fail 条目已进入过决策，不再重复分叉」。

**唯一合法重入路径**（既有实现）：决策 backtrack → 写面改写最新条目 `backtrack_to` → 下一轮 `phase_next:133-155` 分支先手接管路由（走不到两道判断）→ 相位真执行 → 新 fail 条目（无决策槽）→ 下次照常分叉。

**预算**：`MAX_RETRY_TIMES = 5` 单相位窗口计数原样保留（第 5 次失败后每次失败都走决策）；`MAX_ROUNDS = 20`（`entries.len()` 计）硬兜底，每轮循环必落 >=1 条条目 ⇒ token 损失封顶在 20 轮决策内。**不做周期检测**。

### 4.7 存量数据与 `EdgeKind::Retry`

`retry` 边在存量中真实存在（决策 retry 兑现时 `PhaseStart` 首节点的 incoming 记 Retry，`walker.rs:290-294`）。

处理：

- `WORKSPACE_STORE_FORMAT_VERSION` **保持 3**（不 bump、不丢库、不建 from 链）。
- `RunEdgeKind::Retry` **变体保留**——它同时是解码契约：记录信封走 serde_json，删变体会让旧行 decode 失败、`list_run_edges` 整份 Err、历史 run 的流程图整份打不开。注释改为「**存量解码专用**，新写面零产出，查询面投影前屏蔽」。
- 屏蔽点单处（`queries/detail.rs:271-274`）：

```rust
run_entry.edges = store.list_run_edges(&run_entry.run_id)?
    .iter()
    .filter(|record| record.edge_kind != RunEdgeKind::Retry)
    .map(ChangeRunEdgeRecord::from).collect();
```

- 前端因此少几条连线（已接受）。`graph.ts:45-46` 对端点缺失的边本就 `continue`。
- 新写面零产出：walk→retry 边的赋值点随 `Retry` 动作退役变为不可达。

### 4.8 两处连带的措辞/语义

- `ChangeStepKind::RetryGate`：当前实际是「决策动作兑现门」（backtrack 时 emit，`walker.rs:3102-3151`），只在 retry 预算门意义上是历史命名。**变体名保留**（IPC 词面冻结），注释改为「决策动作兑现门」。
- `PhaseNextError::MaxRetriesExceeded`：保留变体（进决策），文案从「重试上限」改为「本相位已连续失败 N 次」。

## 5. 改面清单

### 5.1 Rust

| 文件 | 改动 |
|---|---|
| `workflow/src/state.rs:389-406` | `OnFail` 增 `Escalate` |
| `workflow/src/state.rs:562-583` | `RunEdgeKind::Retry` 保留，注释改「存量解码专用」 |
| `workflow/src/write/phase_table.rs:320-345` | `TEST_EXECUTION_STEPS` 两步 `on_fail: FeedbackLoop{5} -> Escalate` |
| `workflow/src/write/phase_table.rs` `allowed_backtrack_phases` | 加 `redoable` 自排除 |
| `workflow/src/queries/detail.rs:271-274` | 投影前 `.filter(edge_kind != Retry)` |
| `workflow/src/write/phase_next.rs:184-190` | `fail_decided` 保留，注释改写 |
| `workflow/src/write/phase_next.rs:191-219` | `eval_only` 派生删；`PhaseNextError` 两变体去 `eval_only` 字段 |
| `orchestration/src/walker.rs:1015-1122` | `step_test_execution`：删反馈循环，加 `Escalate` 前置分支（保留 Terminal 分支） |
| `orchestration/src/walker.rs:1262-1382` | `step_mutation_test`：同上（保留 runner Err -> Terminal） |
| `orchestration/src/walker.rs:1875-1998` | 两升级落账：改文案、去 `fix_session` 槽位、report 携完整明细、evidence 携 report_dir |
| `orchestration/src/walker.rs:1648-1658` | `feedback_limit` 增 `Escalate` arm（漂移 Err，不可达） |
| `orchestration/src/walker.rs:3027-3175` | 白名单门迁入 `decision_session`；`DecisionAction::Retry` 分支删 |
| `orchestration/src/walker.rs:3182-3246` | 软拒环（drift <=2）、ask 续问文案三动作化 |
| `orchestration/src/decision.rs` | 删 `Retry` 变体；`DecisionInput` 删 `eval_only`、增 `fail_report` |
| `orchestration/src/prompt.rs:33-84` | 三动作协议、白名单自注记、删 retry 指引二分支、增失败报告段与读文件提示 |

### 5.2 测试

| 用例 | 位置 | 处置 |
|---|---|---|
| 反馈边 fail / error 走反馈边 / 恰五次超限 / 修复重入 / 双门禁独立计数 | `walker_test.rs:4908 / 4976 / 5021 / 5204 / 5120` | 改为「首败即升格、零会话」；`5120` 退役 |
| mutation 四条反馈边用例 | `walker_test.rs:8614 / 8695 / 8748 / 8804` | 改为「report 携 feedback_text -> 首败升格」；计数两条退役 |
| `test_execution门控声明锚定` | `walker_test.rs:5270` | `feedback_limit_of` 断言改 `Escalate` |
| `{"action":"retry"}` fixture | `walker_test.rs` 23 处 | 改为软拒一次 + 合法动作，或直接改 stop / backtrack 序列 |
| `OnFail` 往返 | `state_test.rs:1212-1245` | 补 `Escalate` 往返 + 非法词表 |
| 拆步 / on_fail 断言 | `phase_table_test.rs:1934-2080` | 两步 on_fail 期望值更新 + `Escalate` 归属守卫 |
| `eval_only` 断言 | `phase_next_test.rs:775-880` | 随字段删除一并清理 |
| `decision_session_id` latch 用例 | `phase_next_test.rs:564-660 / 825-905 / 1049-1088` | **保留**（latch 仍在） |
| `RunEdgeKind` 四值往返 | `run_history_test.rs:410-422` | **保留**（测的是解码兼容） |
| 新增 | — | 白名单自排除、查询面屏蔽、软拒漂移、首败升格（零 Executor / Evaluator 请求） |

### 5.3 spec（openspec）

| spec | 改动 |
|---|---|
| `desktop-test-execution` | 「结论反馈边与独立预算」整条重写为「失败即升格交决策」；删「修复会话输入面自足」口径 |
| `desktop-change-orchestration:15 / 52-55 / 457 / 461-467` | 反馈边注入对象清单去 test-execution / mutation-test；拆步语义改「两步各自首败升格」；决策四动作 -> 三动作；`fail_decided` 语义改写 |
| `desktop-mutation-testing:81 / 100` | 反馈边②整段重写；**显式声明「两口径反向并存，MUST NOT 被后来者统一掉」随反馈边退役失效**，免后来者误以为红线仍在 |
| `desktop-workflow-write-face:162 / 205` | `on_fail` 声明改 `Escalate`；`OnFail` 变体清单；`allowed_backtrack_phases` 自排除规则 |
| `desktop-change-queries` | run 边投影屏蔽 retry 的读面契约 |
| `desktop-change-flow-view:80 / 125` | 图面不再出现 feedback / retry 边；静态用例图序核对 |

### 5.4 前端

`bindings.ts` 无需重生成（`OnFail` / `DecisionInput` 均不上 IPC；`RunEdgeKind` 值域仍四值）。`graph.ts` / `change-flow-graph.tsx` 零改动（`retry` 本就落默认样式）。`graph.test.ts:311 / 402` 的 retry fixture 可改写为「输入含 retry 边 -> 输出图不含该边」的投影锚。

## 6. 显式不做

| 不做 | 理由 |
|---|---|
| 库格式 bump / from 升级链 | 今后须保兼容；变体保留 + 查询屏蔽已足够 |
| 周期检测（KMP 最短周期） | `MAX_ROUNDS=20` 已封顶 token 损失 |
| `PhaseRecord.run_id` | `SessionAnchors` 进程内 anchor 的「重启即重置预算」恰是需要的语义 |
| `EdgeKind::Retry` 变体删除 | 解码契约，删了旧流程图整份打不开 |
| `fail_decided` 删除 | 承重墙（§4.6） |
| `rerun`（破复用门）动作 | 测试不允许 flaky |
| `decide_on_fail` 表字段 | `evaluator.is_some()` 标记暂留（其 prompt 文本零消费点，属待清理资产） |

## 7. 验证锚

1. `allowed_backtrack_phases(test-execution)` 不含自身；`(implement)` 含自身。
2. test-execution 首败：零 `WorkerRole::Executor` / `Evaluator` 请求、恰一条 fail PhaseRecord、`WhitelistGate` 之前恰一次决策会话（High 档）。
3. 决策输出 `{"action":"retry"}` -> 软拒一次 -> 同会话续注（`continue_session` 断言）-> 第二轮 backtrack 成功。
4. 决策 backtrack `to = 自身` -> `WhitelistGate` failed 节点 -> 二次仍自环 -> `Terminal::failed("决策漂移")`。
5. 组合链：fail -> 决策 backtrack -> implement -> test-execution 复用门因 mtime 变化失效 -> 真重跑 -> pass。
6. `fail_decided` 回归：backtrack 重入后新 fail 条目照常分叉；同一条目不重复分叉。
7. 查询面：库中含 `retry` 边的旧 run -> detail 投影后 edges 不含该边，其余边序不变。
8. 回归锚不变：initialize 基线 `on_fail=Terminal`、implement / test-gen 的 static-check 反馈边、verdict 解析失败反馈边、archive 11 步。

## 8. 已否决方案（一行备忘，防止复提）

| 方案 | 否决理由 |
|---|---|
| 规则式自动回溯（按 `prerequisites` 固定目标） | 分不清测试错 / 设计错 |
| fail -> 直接 UI 中断让用户选 | 决策 agent 的 ask 已覆盖此退路 |
| `FeedbackLoop{limit:0}` 替代 `Escalate` | 「禁用」非「移除」，死代码留存 |
| `eval_only` 保留 + 二分支指引 | retry 退役后无消费者 |
| 保留 `retry` 动作、仅对确定性相位拒绝 | 保留了一个无处可用的概念 |
