# 流程 Agent 的 finish_step 工具（统一挂载 + 收口 + 输出校验前移）

workflow_type: requirement

> 本文是**结论稿**。五轮探索的比选、推翻与纠偏已剔除，只保留定稿、代码锚点与
> 可执行的改动面。
>
> 前序：从 `openspec/explores/steer-running-agent-step.md` 拆出。steer 等本课题
> 落地后再回来讨论（两者改动面几乎不重叠）。

---

## 0. 一句话范围

把 evaluator / decision 的**文本协议 + 收口后解析 + 重试环**换成**工具协议**
（`finish_step`），并把工具挂载面从「三源并行」收敛成「一张清单」。

```
今天                                     之后
────────────────────────────────────     ────────────────────────────────────
evaluator 最终消息 = JSON 文本             evaluator 调 finish_step(...)
  └─ 收口后 parse_verdict                  └─ 工具体内校验，Err 当场回灌
     Err → verdict_fix_prompt              Err → is_error ToolResult（模型当场重试）
       → 同会话续注（≤5 次）                 Ok  → 记 finalized → 本 turn 收口
       → 超限升格相位 fail

decision 最终消息 = JSON 文本              decision 调 finish_step(...)
  Err → drift++（≤2）                      drift 环退役
  ask → wait_answer → Continue             ask 停等上移出会话（见 §6.4）

工具面 = tools(收窄) + toolset(枚举)       工具面 = Vec<ToolMount>（单源）
  + 三张按名常量表（policy）                 tier 比较
  + 四处按名 match（dispatch/protect/…）
```

---

## 1. 已裁决清单

| # | 裁决 | 影响面 |
| --- | --- | --- |
| 1 | `finish_step` 语义 = **本 step 结束**（不是本轮结束） | 一轮一命下等价于本 turn 收口 |
| 2 | **不进 `SessionToolset`**，走独立字段 | 该枚举本轮整体退役，无第三源 |
| 3 | 工具名统一 **`finish_step`**，参数与描述**按 step 动态调整** | 消解与 `StepNode::Finalize`（归档 git 步）的撞名 |
| 4 | **所有工具统一挂载，agent 无感知** | 工具面单源化（§5） |
| 5 | `ToolMount` **落 infra**，含 infra 内置工具与 core 传入工具 | core 只能传名字 + 词汇（§4.1 / §4.3） |
| 6 | `protect` / `sandbox` **本轮不迁字段**（控制变更范围） | 保留两处按名 match（§5.3） |
| 7 | checklist **逐项结构化**，让模型按结构返回每项结果 | §7 |
| 8 | 解析器守卫**到 ID** | 8 条项数断言 + 全文/段落集合相等（§7.2） |
| 9 | **截断回退可接受**（截断 → finalized 缺席 → 判失败） | 不加降级分支（§6.7） |
| 10 | 暂不考虑 `slim-flow-aux-steps` | 其间 A3 与之重叠，待完成本课题后更新 |

---

## 2. 现状的四处成本

| 成本 | 位置 | 说明 |
| --- | --- | --- |
| 反馈延迟一整轮 | `walker.rs:1563` `step_verdict_gate` 循环 | 解析失败的诊断要等**下一轮完整请求**才回到模型眼前 |
| 重试逻辑重复三份 | `step_verdict_gate` / `step_verdict_gate_terminal` / `decision_session` | 同一件事三种写法（feedback 环 / 单次失败 / drift 环） |
| `report` 双职 | `prompt.rs::evaluator_protocol` | 模型既给结论又给落盘载体，载体内容会进下一轮 prompt（`DecisionInput.fail_report`） |
| 解析失败不可归因 | `MAX_REPORT_CHARS` 超限 | 模型无从得知超了多少，只能靠 fix prompt 猜 |

另有一处隐患：`parse_verdict` 与 `derive_verdict`（`phase_log.rs:30`）之间**没有任何
一致性守卫**——模型可以给「全 pass 清单 + verdict=fail」，`walker.rs:3573` 明写这个
漂移形态要「全量随行不虚减」，即承认它会发生。工具化后 verdict 恒由 checklist 推导，
隐患从源头消失。

---

## 3. 交付分三步

```
① 统一挂载        纯重构，行为零变，可独立提交与验证
② finish_step     新增一项 ToolMount + 校验器；依赖 ① 的形状
③ checklist 逐项  解析器 + ID 守卫 + 落账映射；依赖 ②
```

三步各自可单独成立。①②混做会让工具面重构与协议变更的失败原因纠缠不清。

---

## 4. 三个决定形状的约束

### 4.1 分层三角：core 传词汇 / infra 校验 / orchestration 解释

工具执行体要调 `parse_verdict` / `parse_decision`，而它们住在 `core/orchestration`；
`core/agent` 是「禁 workspace 内依赖」的中立契约 crate。

```
core/agent        纯词汇：FinishSpec { EvaluatorChecklist, Decision }
   │  (不认识 EvaluatorChecklist / DecisionAction / 任何工具定义)
   ▼
infra/agent       铸 ToolMount + 执行体校验（已依赖 orchestration ✅）
   │  (不依赖 workflow —— 拿不到 include_str! 的 md)
   ▼
core/orchestration  finalized 载荷 → 落账（持有全部域类型，零新依赖）
```

**连带结论**：`infra/agent` 不依赖 `workflow`（`Cargo.toml` 无该行），所以动态
schema 的唯一真源不能靠 infra 去读 md 文件。→ §4.2 的解法。

### 4.2 ID 清单从 prompt 自己身上解析（关键发现）

```
phase_next 组装 prompt = context_header + md 全文 + 回溯原因 + 反馈段
                       └─ ## 静态清单 表就在 md 全文里
WorkerTurnRequest.prompt: String           ← 已经是全文
        ↓
loop::run 里 definitions(&turn.injections, &turn.question)   ← question 就在手边
        ↓
FinishSpec → 从 question 扫 ## 静态清单 段 → ID 集合 → 铸 parameters.enum
```

三个好处：

| 好处 | 说明 |
| --- | --- |
| **零新依赖** | infra 不必认识 workflow；md 以文本形式本来就在 prompt 里 |
| **天然自一致** | 校验的 ID 集合 == **模型眼前那份 prompt 里**的 ID 集合。换 prompt 变体、加 ID，校验自动跟随 |
| **解析规则可独立测试** | 与守卫断言同一套规则，见 §7.1 |

代价：`definitions()` 加一参。生产调用点**恰 1 处**（`loop.rs:163`），测试调用点 17 处。

### 4.3 core 传「名字」，不传「结构体」

`SessionOpen` 有 `#[derive(PartialEq, Eq)]`，而 `ToolMount` 含
`parameters: serde_json::Value`——**`Value` 无 `Eq`**。

| | 名字清单（选定） | 结构体清单 |
| --- | --- | --- |
| core 携带 | `tools: Option<Vec<String>>`（形态不变） | `Vec<ToolMount>` |
| `ToolMount` 落点 | infra ✅ 合裁决 5 | core ❌ 违裁决 5 |
| Eq / serde | 零破坏 | 整链去 Eq，round-trip 测试动摇 |
| 单源程度 | **仍单源**——infra 一份 `registry()`，三处消费全查它 | 强一点点 |

`SessionInjections` 最终形态（`Eq` 链完好，`FinishSpec` 是 `Copy + Eq` 闭合枚举）：

```rust
pub struct SessionInjections {
    pub preamble: Option<String>,
    pub tools: Option<Vec<String>>,        // 名字清单；None = 内建七件全是
    pub finish: Option<FinishSpec>,        // 唯一新增
    // toolset: 退役
}
```

`definitions()` / `policy` / `dispatch` 之外的清单内容（description / parameters）与
既有的 `WorkerTurnRequest.prompt` 同性质——都是 infra 自己产的数据，**根本不进 core**。

---

## 5. ① 统一挂载（纯重构）

### 5.1 类型（新文件 `infra/agent/src/sdk/mount.rs`）

```rust
pub struct ToolMount {
    pub definition: ToolDefinition,   // rig：name / description / parameters
    pub tier: ToolTier,               // Read | Write | Execute | Protocol
    pub exec: ToolExec,               // Pure | ExploreNote | Archi | FinishStep(FinishSpec)
}

pub enum ToolTier { Read, Write, Execute, Protocol }   // Protocol = 任何档位放行

pub fn registry() -> &'static [ToolMount];             // 唯一工具事实源
```

七内建 + `create_explore_note` + 两 archi 的定义函数**原样搬入**（字面量零改）。

### 5.2 随之退役的四处「按工具名硬编码」

| 位置 | 今天 | 之后 |
| --- | --- | --- |
| `sdk/tools.rs::definitions` | 3 层 `match &injections.toolset` | 名字 → `registry()` 查表 |
| `sdk/policy.rs` | `READONLY_TOOLS` / `WRITE_TOOLS` / `EXECUTE_TOOLS` 三张表 + 三档 match | 见下（4 行） |
| `sdk/loop.rs` 执行分派 | 3 臂 `if name == EXPLORE_NOTE_TOOL .. else if ARCHI .. else execute` | `match mount.exec` |
| `compose.rs::assemble_injections` | 产出 `SessionInjections { tools, toolset }` | 只产出名字清单 |

```rust
pub fn allows(mode: AgentPermissionMode, tier: ToolTier) -> bool {
    match tier {
        ToolTier::Protocol | ToolTier::Read => true,
        ToolTier::Write | ToolTier::Execute => mode != AgentPermissionMode::Default,
    }
}
```

`TOOL_NAMES` 保留但**降级**：从「准入白名单」变成「七件内建的定义作者处」，不再参与
任何判定。

### 5.3 本轮**不动**的两处（裁决 6）

| 位置 | 保留原因 |
| --- | --- |
| `sdk/tools.rs::input_path` | `glob` 取 `pattern`、两 archi 工具显式 `None` 的特判保留 |
| `sdk/protect/mod.rs::evaluate` | `"write" / "edit"` 按路径、`"bash"` 按命令文本的 match 保留 |

代价：加一个「带路径参数或带写入面」的工具仍需改这两处。收益：变更面收缩到
tools / policy / dispatch 三处。

### 5.4 需要连带改的判据

`infra/agent/src/sdk/runner.rs` 两处 per-turn 上下文铸造判据从枚举比较改名字包含：

```rust
// 今天：open.injections.toolset == Some(SessionToolset::Explore)
// 之后：names.contains(EXPLORE_NOTE_TOOL)

// 今天：mint_archi_context 按 Some(SessionToolset::Archi { .. })
// 之后：names.contains(ARCHI_MODULE_TOOL) || names.contains(ARCHI_SEARCH_TOOL)
```

### 5.5 迁移量（实测，非估算）

```
SessionInjections 字面构造        34 处
  tools_test 10 / loop_test 9 / sdk-runner_test 5 / core-runner_test 5
  compose.rs 3（生产）/ compose_test 1 / kernel_test 1

读侧断言（.tools / .toolset）      42 处（测试）+ 5 处（生产）
  compose_test 21 / core-runner_test 13 / sdk-runner_test 5 / core-lib_test 3

definitions() 调用点               1 生产 + 17 测试（全部加 prompt 实参）
```

建议夹具（一次压平改动面）：

```rust
// mount.rs，pub(crate)
pub(crate) fn injected(names: &[&str]) -> SessionInjections { .. }
```

### 5.6 步骤切分与回滚点

```
Step 0  mount.rs + checklist_ids + 守卫测试     纯新增，行为零变   可独立提交 ★回滚点
Step 1  core 词汇面（删枚举 / 加 FinishSpec）   破坏性，编译期全暴露  } 同一提交
Step 2  infra 消费面（registry/policy/dispatch）语义等价重构         }
Step 3  测试面 + spec 改写
Step 4  回归门
```

**Step 0 单独提交是关键**：先让 `registry` 与 `checklist_ids` 的守卫测试变绿，
后续出错即可确定问题在「接线」而非「数据」。

---

## 6. ② finish_step

### 6.1 不新增机制，只是清单里多一项

```
evaluator 步：
  ToolMount {
    definition: { name: "finish_step",
                  description: "本步收尾。按下列清单逐项给出结果；report ≤2000 字符。",
                  parameters: { report, items: [{ id: enum[..], pass, evidence }] } },
    tier: ToolTier::Protocol,                        // 任何档位放行
    exec: ToolExec::FinishStep(FinishSpec::EvaluatorChecklist),
  }

decision 步：
  同构，parameters 为三动作形状，exec = FinishStep(FinishSpec::Decision)

其余步：
  清单里没有 finish_step 这一项 → 幻觉调用被成员校验拒
```

「参数与描述按 step 动态调整」= **同一位置的不同数据**，不是不同机制。

### 6.2 loop 内的收口（唯一实质改动）

```rust
let mut finalized: Option<FinishPayload> = None;
for tool_call in tool_calls {
    if finalized.is_some() {
        // 同批后续调用：假结果必须真的经 sender 发出（进转录）
        let reason = "本步已 finish_step 收口，该调用未执行";
        emit ToolResult { is_error: false, content: reason };   // 不是「失败」
        results.push(outcome_result(tool_call, reason));
        continue;
    }
    ... 既有成员校验 / policy / sandbox / protect ...
    if matches!(mount.exec, ToolExec::FinishStep(_)) {
        match validator(...) {
            Ok(payload) => { finalized = Some(payload); ("已收口", false) }
            Err(msg)    => (msg, true),                          // 当场回灌
        }
    }
    ...
}
history.push(Message::tool_results(results));   // 配对闭合先于收口
if let Some(payload) = finalized {
    emit SystemNotice { subtype: "finish_step", payload };   // 载荷出口
    finish_success(...);                                      // TurnDone is_error=false
    return history;                                            // 零额外请求
}
turn_index += 1;
```

三个必须守住的点：

| 点 | 理由 |
| --- | --- |
| 假结果 `is_error: false` | `resume::append_tool_results` 对 is_error 会拼「工具执行失败: …」，协议收口不是失败 |
| `results` 收齐**先于** `return` | provider 配对硬要求（同批 tool_use 的结果必须进恰一条 user 消息） |
| 假结果要真的经 `sender` 发出 | 否则 `resume::rebuild` 补 `INTERRUPTED_RESULT`，语义变差 |

**不需要从外部终止会话**。终止在 loop 内闭合，四层全走既有成功链：

```
① loop      校验 Ok → finalized → TurnDone + return        ← 唯一新增点
② pump      一轮一命，loop return 即泵结束                  零改动
③ kernel    observation 流结束 + 状态机 Completed           零改动
④ run_worker Completed → StepSettle::passed                零改动
```

两道天然兜底：`if !tool_calls.is_empty() { break 'continuation tool_calls; }` 意味着
工具优先收口，`finish_step` 不可能落进 `pause_turn` 续跑分支；即使漏了 `return`，
下一轮模型看到收口已被接受，大概率自然收敛——只白烧一轮，不死循环。

**否决外部终止的理由**：唯一既有通道 `RunHandle.request_stop()` 的语义链是
`Stopped → Terminal::stopped → UI「已停止」`，复用它会令一次正常的评估收尾在 UI 上
显示成用户终止。

### 6.3 载荷出口：SystemNotice，不改 TurnOutcome

```
loop 发 SystemNotice{subtype:"finish_step", payload}
  → 密封事件进 transcript
  → worker.rs 提取（沿 transcript::failure_reason 从 api_error 取值的先例）
  → WorkerTurnOutcome.finalized
  → walker 直接读，不再 parse 文本
```

否决另两条：`TurnOutcome`（`core/agent/src/port.rs:24` 明说它是「store 轮行落库与
IPC 终态装配共用的唯一收口形状」，塞协议载荷 = 污染共用契约）；`TurnDone` 扩字段
（infra 认识 verdict 语义，撞 §4.1 三角）。

额外收益：`SystemNotice` 已在 transcript，截断留痕（`output_truncated`）同样可取，
失败文案可区分「输出被截断」与「模型没调 finish_step」，零额外机制。

### 6.4 ask：停等上移出会话

「一调即收口」会让 `DecisionAction::Ask` 的 `wait_answer` 死在这一步。解法是把停等
移出决策会话：

```
finish_step(action=ask) → 本 turn 收口（finalized = Ask）
                        → walker 看到 Ask：emit RunUpdate::Ask → wait_answer
                        → 新开一轮 decision 会话（Continue resume 全史）
```

收益：`DECISION_DRIFT_LIMIT = 2` 软拒环**整个退役**（校验失败变成工具 Err 当场回灌，
不需要跨轮 drift 计数）。

`ensure_backtrack_allowed`（白名单门）仍留 walker——它需要运行时 `allowed`，不是
自包含校验（`decision.rs:60` 四参含 `failed_phase` + `allowed`）。

### 6.5 校验的边界：static 进工具，contextual 留 walker

| 校验 | 能否进 finish_step | 理由 |
| --- | --- | --- |
| JSON 形状 / action tag 封闭集 | ✅ | 自包含 |
| report ≤2000 / checklist 非空 / pass 布尔 | ✅ | 自包含 |
| backtrack 目标在白名单 / 自环判定 | ❌ | 需运行时 `allowed` |
| ask 停等 | ❌ | 需 `guard` |

所以校验器签名只能是 `Value → Result<FinishPayload, String>` 纯函数，
`infra/agent` 拿不到 `DecisionInput`。

### 6.6 退役与改写

| 对象 | 处置 |
| --- | --- |
| `walker.rs` `step_verdict_gate` 反馈环 | 读 `finalized` 替代 `parse_verdict`，环删除 |
| `walker.rs` `step_verdict_gate_terminal` | 同上（判据 `finalized.is_none() && Completed`） |
| `walker.rs` `decision_session` drift 环 | 删除，`DECISION_DRIFT_LIMIT` 常量退役 |
| `prompt.rs::evaluator_protocol` | 从「输出 JSON」改「调 finish_step」 |
| `prompt.rs::verdict_fix_prompt` / `decision_fix_prompt` | **退役**——工具 Err 文案成唯一反馈面 |
| `verdict.rs::parse_verdict` | 保留（infra 校验用），拆出「只校验不落账」的面 |

**这抬高了 Err 文案的要求**：它必须自足（哪个字段、期望什么、当前值、超了上限多少）。
这是本次改动里唯一不能偷懒的文案面。

### 6.7 截断（裁决 9）

`max_tokens` / `context_window_exceeded` 走的是 `finish_success` → status=Completed
且不发 finalized。现状下截断前的文本里若已含完整 JSON 仍能 parse 成功；改造后
`finalized` 恒 `None`。

按裁决接受回退。若要给人读的差异，`output_truncated` notice 已在 transcript，取它
零成本（纯文案，不影响判据）。

### 6.8 8 个 evaluator md 的同一句改写

8 份 `*evaluator.md` 都含同一句原文：

```
- 只产出评估结论：最终消息输出一个 checklist JSON（形状与落账红线由桌面端在
  会话 prompt 尾部追加的输出协议给出）。
```

这句把形状定义权明确交给了动态附录，所以只需把「输出 JSON」改成「调 `finish_step`」。
**是确定的 8 个文件同一句改写**，不是「0 或 9」。

（注：prompt md 实为 **14 份**——6 executor + 8 evaluator，不是早期写的 16 份。）

---

## 7. ③ checklist 逐项结构化

### 7.1 解析器规范（三条规则，缺一即错）

1. 只扫 `## 静态清单` 标题起、**下一个 `## ` 标题止**的段落；
2. 行首匹配 `^\| ([A-Z]+[0-9]+) \|`（表首列，两侧空格约束）才收；
3. 结果按出现序去重。

实测过反例：

| 放宽的写法 | 后果 |
| --- | --- |
| 不限段落（全文扫） | `test-design-evaluator.md` 含 `T8-U1` / `T8-G1` 子表 → 污染 |
| 无边界 `[A-Z]+[0-9]+` | `T8-U1` 切出 `T8` / `U1` / `G1` 三个假 ID（宽松计数 15 vs 严格 9） |
| 不去重 | 正文重复引用会出现重复项 |

### 7.2 逐文件实测值（直接做成守卫断言）

```
acceptance-evaluator          A1..A5                     5
code-analyze-evaluator        CA1..CA8                   8
code-review-evaluator         C1..C7                     7
dev-design-evaluator          D1..D10                   10
implementation-evaluator      I1..I7                     7
proposal-evaluator            R1..R11                   11
test-design-evaluator         T1..T9                     9
test-gen-evaluator            G1..G8                     8
                                                   合计 65
```

两层守卫：

```
① 每文件项数（上表 8 个数字逐文件断言）—— 表结构被改坏立刻红
② 全文边界 ID 集合 == 段落内 ID 集合（无多无少）—— 防子表混进主清单
   实测 8 个文件全部相等
```

### 7.3 落账面零变更（关键取舍）

模型返回 `{ id, pass, evidence }`，而落账类型是 `ChecklistItem { item, pass, evidence }`。
**在工具边界把 ID 映射成 md 表第二列的「检查项」正文**，落账形态完全不变：

```
finish_step 载荷  items: [{ id: "C1", pass: false, evidence: "src/x.rs:42 硬编码密钥" }]
        │
        ├─ 校验：id 集合 == 解析集合（缺项 / 多项 / 未知 ID 三种 Err 文案分列）
        ├─ 映射：id → md 表「检查项」列正文（"变更代码中无安全漏洞"）
        └─ 产出：ChecklistItem { item: <正文>, pass, evidence }
                                  ↓
                     derive_verdict(checklist) → verdict 由推导，不取模型自报
```

结果：**store schema 零变更、bindings 零变更、前端 renderer 零变更**。
附带改善：`item` 从「模型自拟的名字」变成「md 里的权威文案」。

### 7.4 机械 checklist 必须显式豁免

`walk_item` 的 9 个 `ChecklistItem` 构造点里，**5 个是机械代写**，`item` 是中文短语，
与 md 静态清单无关：

```
静态检查 / 架构检查 / 测试执行 / 突变测试 / 评估输出格式
suite 结论一致 / 聚合计数一致 / mutation null 自动通过
```

所以「ID 集合相等」只挂 `ToolExec::FinishStep`。落账路径（`run_phase_log` +
`derive_verdict`）对 checklist 完全不变。

（后续可选项：机械 checklist 改稳定 ID，但那要动落库面与渲染面，不在本轮。）

---

## 8. 测试面总量

| 组 | 量 | 性质 |
| --- | --- | --- |
| ① 夹具形态变更 | 34 构造 + 47 读断言 | 形态变更，非加断言 |
| ① `definitions()` 加参 | 18 调用点 | 机械 |
| ① `policy_test` 矩阵 | 重写 12 格（4 tier × 3 档） | 从硬编码名单改枚举 |
| ② `walker_test` | `with_evaluator_reports` 18 / `with_decision_reports` 35 | 断言面改造，非纯新增 |
| ② `prompt_test` | 802 行，`verdict_fix_prompt` / `decision_fix_prompt` 段落断言 ≈17 处 | 随函数退役删除 |
| ② `verdict_test` | 156 行 | 保留（parse 仍用） |
| ③ 解析器守卫 | 新增 8 项数断言 + 1 集合相等断言 | 纯新增 |

`walker_test.rs` 15862 行是最大风险面：`FakeWorker` 的产出夹具要从「文本 JSON」改成
「finalized 载荷」。建议路径是注 `SystemNotice`（`with_transcripts()` 已有注入口），
比改 `WorkerTurnOutcome` 构造轻。

---

## 9. spec / 语料同改清单（AGENT.md 硬要求）

| 对象 | 动作 |
| --- | --- |
| `openspec/specs/desktop-agent-execution/spec.md:361,363` | 「专用工具仅经 `toolset` 枚举注入，自由字符串工具名 MUST NOT 进执行面」→ 改为「工具清单真源在 infra registry；core 传名字，非法名在装配层 Err」 |
| `openspec/specs/desktop-archi-agent/spec.md:97` | 同款改写（`SessionToolset::Archi` 变体本身退役） |
| `openspec/specs/desktop-change-orchestration/spec.md:198,219` | verdict 解析与决策协议 → finish_step 协议（②） |
| `openspec/specs/desktop-phase-prompts/spec.md:90,467` | 「最终消息 checklist JSON」→「调 finish_step」（②） |
| `src/bindings/mod_test.rs:429` | 反向锚定清单去掉 `"SessionToolset"`（否则编译期即红） |
| `core/workflow/src/write/phase_table_test.rs:3133,3144` | 两处**仅注释**引用 `SessionToolset`，改文案，断言本体零改 |
| `packages/desktop/package.json` | 每次源码改动 patch 递增（版本纪律） |

---

## 10. 明确不做

| 项 | 原因 |
| --- | --- |
| `path_field` / `protect` 迁 `ToolMount` 字段 | 裁决 6：控制变更范围（代价是加该类工具仍需改两处） |
| 机械 checklist 改稳定 ID | 要动 `phase_log` 落库面 + 前端 renderer |
| `parse_verdict` / `derive_verdict` 一致性守卫 | ② 落地后该隐患从源头消失，不必先补 |
| 工具层重试计数上限 | loop 本就无轮数上限（仅 `pause_turn` 有 5 次续跑预算），「模型修不对格式」与「模型无限调 read」同类 |
| 前端改 | `SessionInjections` 不出线 IPC，反向锚定已守住 |

### 可选优化（建议做，属 ② 范围）

新前端 `finish_step` 专用工具卡。不加则回退 `ToolPairCard` 裸 JSON——时间线上每次
收口摊一大坨 JSON。加一张按 `input.action` / `input.verdict` 显摘要行的卡即可。

---

## 11. 仍未定（不影响开工）

1. `ToolTier::Protocol` 在 debug 会话下是否需要——debug / explore 不挂 `finish_step`，
   该档位永不触发，可先恒放行。
2. `description` 随 step 的差异度——evaluator 步两句、decision 步一段，均 <500 字，
   无 prompt 膨胀风险。

---

## 12. 与 steer 的关系

```
steer：   用户 ──► 运行中 turn（通道注入），   不产生 step 产出
finish：  agent ──► 编排（工具 + 收口），      产生 step 产出

共享：loop 的「非自然收口」概念（steer 是外部注入，finish_step 是内部触收口）
共享：core/agent 的注册表形态思想（steer 需 registry，finish_step 走 tool_call 无需）
```

用户裁决顺序：**finish_step 先落地，之后回来讨论 steer**。

---

## 13. 给未来的自己：三处「非显然正确」

改动时先看这里，这三点每个都曾是被推翻过的方案：

1. **同批 tool_calls 的假结果**必须 `is_error: false` 且真的经 `sender` 发出——
   `is_error: true` 会让 `resume::append_tool_results` 拼上「工具执行失败: 」；
   不发则 `resume::rebuild` 补 `INTERRUPTED_RESULT`，语义变差。
2. **`results` 收齐必须先于 `return`**——provider 要求同批 tool_use 的结果进恰一条
   user 消息，顺序错了就是 400。
3. **`finish_step` 的 Err 文案是唯一反馈面**——两个 `fix_prompt` 已退役，文案必须
   自足（哪个字段 / 期望什么 / 当前值 / 超了多少）。
