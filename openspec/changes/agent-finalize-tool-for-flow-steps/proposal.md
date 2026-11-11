# 提案: agent-finalize-tool-for-flow-steps

> **变更**: agent-finalize-tool-for-flow-steps
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/agent-finalize-tool-for-flow-steps/explore.md`（结论稿：0 一句话范围 / 1 已裁决十条 / 2 现状四处成本 / 3 三步交付 / 4 三个决定形状的约束 / 5–7 逐面定稿 / 8 测试面总量 / 9 spec 同改清单 / 10 明确不做 / 11 未决两项 / 13 三处「非显然正确」）
> **范围接缝**: 本变更只做**流程 Agent 的收口协议工具化与其挂载面单源化**（① 工具挂载统一为 `ToolMount` 清单、② `finish_step` 工具 + 本步收口、③ checklist 逐项 ID 结构化）。MUST NOT 做：`protect` / `sandbox` 规则迁 `ToolMount` 字段（裁决 6）、机械 checklist 稳定 ID 化（要动落库面与渲染面）、`parse_verdict` / `derive_verdict` 一致性守卫（② 落地后隐患从源头消失）、工具层重试计数上限、steer 通道（前序 explore 拆出，本课题完成后再议）、`slim-flow-aux-steps`（A3 与之重叠，另行更新）。
> **前序**: 自 `openspec/explores/steer-running-agent-step.md` 拆出；两者改动面几乎不重叠，用户裁决顺序为 **finish_step 先落地**。

---

## 问题

### 现象一：格式纠错要等一整轮完整请求才回到模型眼前

evaluator 走**文本协议**：模型在最终消息里输出 checklist JSON，walker 收口后 `parse_verdict` 解析（`crates/core/orchestration/src/verdict.rs:58`）。解析失败时反馈路径是「emit 门步 failed → 组装 `verdict_fix_prompt` → 以 `Continue` 续注同一会话 → 再发一整轮请求 → 再解析」，独立计数上限 5（`walker.rs:1563` 的 `step_verdict_gate` 反馈环）。模型拿到的诊断（「checklist JSON 结构漂移: …」）与被判错的输入之间隔着一整轮请求，且诊断只能重述协议文本，模型无从知道**超了多少**、**缺哪一项**。

### 现象二：「解析失败处置」被写了三份

| 位置 | 写法 |
|---|---|
| `walker.rs:1563` `step_verdict_gate` | 反馈环（≤5 次续注修复，超限升格相位 fail） |
| `walker.rs:1818` `step_verdict_gate_terminal` | 单次解析，失败即 `Terminal`（code-analyze 的 `on_fail=Terminal`） |
| `walker.rs:3416` `decision_session` | `DECISION_DRIFT_LIMIT = 2` 软拒环（解析 `Err` 与白名单 `Err` 共用漂移计数） |

同一件事（模型输出不合协议）三种写法、三套计数、三套文案。新增一处协议就得多写第四份。

### 现象三：`report` 一物双职，载体内容反向污染下一轮 prompt

`prompt.rs::evaluator_protocol`（`crates/core/orchestration/src/prompt.rs:46`）要求模型「既给结论、又给出落盘载体」。落盘载体（`report` 全文 + `checklist`）经 `phase_log` 落库后，又作为 `DecisionInput.fail_report`（`decision.rs:33`）整段进下一轮决策 prompt——同一段文本既是「给人看的明细」又是「给模型看的输入」，两边的体量诉求相互牵制。

### 现象四：超限不可归因

`MAX_REPORT_CHARS = 2000`（`verdict.rs:6`）超限的报错只写「超长（N > 2000）」——不含**哪个字段**、**期望什么**、**当前值**。模型只能靠 fix prompt 猜，猜错再等一轮。

### 隐患：verdict 与 checklist 之间没有任何一致性守卫

`parse_verdict` 接受模型自报的 `verdict` 字段，而落账侧 `derive_verdict`（`crates/core/workflow/src/write/phase_log.rs:30`）按 checklist 重新推导——两者之间零校验。模型完全可以给「全 pass 清单 + verdict=fail」，而 `walker.rs:3573` 的注释明写这个漂移形态要「全量随行不虚减」，等于承认它会发生。verdict 由 checklist 推导是唯一的语义，却没有一处机制强制它。

### 症结：工具面三源并行 + 四处按名硬编码

今天「有哪些工具」由四处分别回答，任一处漏改即静默失配：

```
sdk/tools.rs::definitions   收窄子集（按 TOOL_NAMES 过滤）+ toolset 枚举追加
core/agent SessionToolset   Explore / Archi{module,search} 受控枚举（core 承载词汇）
sdk/policy.rs               三张按名常量表（READONLY / WRITE / EXECUTE）+ 三档 match
sdk/loop.rs                 执行分派三臂 if name == EXPLORE_NOTE .. else if ARCHI .. else execute
```

加一个工具要同时改四处的其中三处。`SessionToolset` 作为「受控枚举」的初衷（专用工具仅经枚举注入，自由字符串工具名不进执行面）在只有两个专用工具时尚可维持，但它把「工具清单」这一事实拆到了 core 词汇层与 infra 定义层两处，且每加一个专用工具就要扩一次 core 的公开枚举。

## 提案

把 evaluator / decision 的**文本协议 + 收口后解析 + 重试环**换成**工具协议**（`finish_step`），并把工具挂载面从「三源并行」收敛成「一张清单」。

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
  ask → wait_answer → Continue             ask 停等上移出会话（见「决策」）

工具面 = tools(收窄) + toolset(枚举)       工具面 = Vec<ToolMount>（单源）
  + 三张按名常量表（policy）                 tier 比较
  + 四处按名 match（dispatch/protect/…）
```

### 交付分三步（各自可单独成立，按序提交）

| 步 | 内容 | 性质 |
|---|---|---|
| ① | **统一挂载**：`ToolMount` + `ToolTier` + `registry()` + `definitions()` 单源；`SessionToolset` 枚举退役；policy 三表 → tier 准入；loop 执行分派按 `match mount.exec` | 纯重构，行为零变，可独立提交与验证 |
| ② | **`finish_step`**：新增一项动态挂载 + 校验器 + loop 内收口 + `finalized` 载荷出口；两个 `*_fix_prompt` 与三处重试环退役 | 协议变更，依赖 ① |
| ③ | **checklist 逐项**：解析器（ID + 检查项）+ ID 守卫 + 落账映射（ID → md「检查项」正文） | 数据面收敛，依赖 ② |

①②混做会让「工具面重构」与「协议变更」的失败原因纠缠不清。①内部再切：**Step 0（`mount.rs` + checklist 解析器 + 守卫测试，纯新增、行为零变）单独提交**，先让 `registry` 与解析器的守卫测试变绿，后续出错即可确定问题在「接线」而非「数据」。

### 决策表（含被拒备选与理由，逐条自足成证）

| # | 决策 | 要点 | 被拒备选与理由 |
|---|---|---|---|
| D1 | `finish_step` 语义 = **本 step 结束** | 不是「本轮结束」；一轮一命下等价于本 turn 收口 | **被拒：语义定为「本轮结束」**——流程相位下 step 与 turn 已一一对应（`session_pump` 一轮一命），两种表述在实现上无法区分，而「本 step 结束」在措辞上不预设引擎拓扑，未来一轮多步时不必重命名 |
| D2 | **不进 `SessionToolset`**，走独立 `finish` 字段 | `SessionInjections.finish: Option<FinishSpec>` | **被拒：扩 `SessionToolset` 为第三变体**——该枚举本轮整体退役（工具面单源化后不再需要枚举承载清单），把它留下又要新增变体，等于给正在拆的机制加层 |
| D3 | 工具名统一 **`finish_step`**，参数与描述按 step 动态调整 | 消解与 `StepNode::Finalize`（归档 git 步）的撞名 | **被拒：`finalize`**——相位表里 `Finalize` 已是归档落盘提交步（`state.rs:299`），图面与日志两处同名不同义 |
| D4 | **所有工具统一挂载，agent 无感知** | 专用工具不再经枚举「追加」，而是名字进名字清单、定义进 registry | **被拒：保留 `toolset` 枚举并行（只把 `finish_step` 单列）**——两个真源并存时「本轮下发了什么」仍要两处拼装，成员校验与 `RunStarted.tools` 都得双读；单源化收益（§1 已裁决 4）正是本次主要目标之一 |
| D5 | `ToolMount` **落 infra** | 含 infra 内置工具与 core 传入工具；core 只传名字 + 词汇 | **被拒：`ToolMount` 落 core**——`ToolMount` 含 `parameters: serde_json::Value`，而 `Value` 无 `Eq`；`SessionOpen` / `SessionInjections` 的 `#[derive(PartialEq, Eq)]` 链会整链塌，round-trip 测试动摇。且 core/agent 是「禁 workspace 内依赖」的中立契约 crate，不该持有具体工具定义 |
| D6 | `protect` / `sandbox` 按名 match **本轮不迁字段** | 保留 `tools::input_path` 与 `protect::evaluate` 两处 | **被拒：把 `path_field` / 写入面迁进 `ToolMount` 字段**——控制变更范围：迁字段要动两条守卫链的判定面（含 glob 的 `pattern` 特判、两 archi 工具的显式 `None`），失败原因会与协议变更纠缠。代价（明写）：加「带路径参数或带写入面」的新工具仍需改这两处 |
| D7 | checklist **逐项结构化** | 模型按 md 静态清单逐项返回 `{id, pass, evidence}` | **被拒：仍由模型自拟 `item` 名称**——自拟名与 md 清单之间无对应关系，既无法断言「清单项全覆盖」，`item` 文案也随措辞漂移 |
| D8 | 解析器守卫**到 ID** | 8 份项数逐文件断言 + 段落与全文 ID 集合相等 | **被拒：只断言总数 65**——总数相等挡不住「A 文件少一项、B 文件多一项」；且实测放宽写法会切出假 ID（无边界 `[A-Z]+[0-9]+` 把 `T8-U1` 切成 `T8` / `U1` / `G1`，宽松计数 15 vs 严格 9） |
| D9 | **截断回退可接受** | 截断 → `finalized` 缺席 → 判失败 | **被拒：加降级分支**（截断时回退解析文本 JSON）——两条收口路径并存即两套判据两套测试，且截断本就意味着产物不完整，「让相位 evaluator 判它是截断的」比「引擎猜它没截断」更诚实 |
| D10 | 落账面**零变更**：ID → md「检查项」正文映射 | 模型给 `id`，工具边界把它映射成 `ChecklistItem.item` 的权威文案 | **被拒：落库 `item` 直接存 ID**——要动 store schema 语义、bindings 与前端 renderer（`desktop-change-queries` 的 checklist 线面形状），且人读报告从此只有 `C1` 没有「变更代码中无安全漏洞」 |
| D11 | 暂不考虑 `slim-flow-aux-steps` | 其间 A3 与本课题重叠 | **被拒：本轮一并处理**——A3 的执行面尚未定稿，先做会让两处对「辅助步」的定义互相污染；待本课题完成后单独更新 |

### ① 统一挂载：一张清单三个消费点

新文件 `crates/infra/agent/src/sdk/mount.rs`：

```rust
pub struct ToolMount {
    pub definition: ToolDefinition,   // rig：name / description / parameters
    pub tier: ToolTier,               // Read | Write | Execute | Protocol
    pub exec: ToolExec,               // Pure | ExploreNote | Archi | FinishStep(FinishSpec)
}

pub enum ToolTier { Read, Write, Execute, Protocol }   // Protocol = 任何档位放行

pub fn registry() -> &'static [ToolMount];             // 唯一工具事实源（十项静态挂载）
```

七内建 + `create_explore_note` + 两个 archi 工具的定义函数**原样搬入**（字面量零改）。`finish_step` 不进静态 registry（参数按 step 动态），由 `definitions()` 按 `injections.finish` 铸造后追加——「参数与描述按 step 动态调整」= 同一位置的不同数据，不是不同机制。

三处按名硬编码随之退役：`sdk/tools.rs::definitions` 的三层 `match &injections.toolset` → 名字查表；`sdk/policy.rs` 三张常量表 → 四行 tier 准入；`sdk/loop.rs` 的执行分派三臂 → `match mount.exec`。`TOOL_NAMES` 保留但**降级**为「七件内建的定义作者处」，不再参与任何判定。

tier 准入与原三档名单逐格等价：`Protocol | Read` 恒放行，`Write | Execute` 需 `mode != Default`——与原 `READONLY_TOOLS` / `WRITE_TOOLS` / `EXECUTE_TOOLS` 的 Default 档语义完全一致。

**core 只传名字**（裁决 5 的连带结论）：`SessionInjections` 最终形态 `{ preamble, tools: Option<Vec<String>>, finish: Option<FinishSpec> }`，`Eq` 链完好（`FinishSpec` 是 `Copy + Eq` 闭合枚举）。`tools` 的名字清单是**唯一单源**——infra 一份 `registry()`，`definitions()` / `policy` / 成员校验三处消费全查它；非法名在装配层（compose）`Err`。

### ② finish_step：只是清单里多一项

```
evaluator 步：
  ToolMount {
    definition: { name: "finish_step",
                  description: "本步收尾。按下列清单逐项给出结果；report ≤2000 字符。",
                  parameters: { report, items: [{ id: enum[<本轮 prompt 的静态清单 ID>], pass, evidence }] } },
    tier: ToolTier::Protocol,
    exec: ToolExec::FinishStep(FinishSpec::EvaluatorChecklist),
  }

decision 步：
  同构，parameters 为三动作形状，exec = FinishStep(FinishSpec::Decision)

其余步：
  清单里没有 finish_step 这一项 → 幻觉调用被成员校验拒
```

**参数里的 ID 清单从 prompt 自己身上解析**（关键发现）：`phase_next` 组装的 evaluator prompt 已是 `context_header + md 全文 + 回溯原因 + 反馈段`，`## 静态清单` 表就在 md 全文里；`loop::run` 手上就有 `turn.question`。由此 `definitions(&turn.injections, &turn.question)` 扫段落得 ID 集合——**零新依赖**（infra 不必认识 workflow）、**天然自一致**（校验的 ID 集 == 模型眼前那份 prompt 的 ID 集，换 prompt 变体、加 ID 自动跟随）、**解析规则可独立测试**（与守卫断言同一套规则）。代价是 `definitions()` 加一参：生产调用点恰 1 处、测试调用点 17 处。

#### loop 内收口（唯一实质改动）

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
    ... 既有成员校验 / tier 准入 / sandbox / protect ...
    if matches!(mount.exec, ToolExec::FinishStep(_)) {
        match orchestration::finish::validate(spec, &input, &turn.question) {
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

三个必须守住的点（每点都曾是被推翻的方案）：

| 点 | 理由 |
|---|---|
| 假结果 `is_error: false` | `resume::append_tool_results` 对 `is_error` 会拼「工具执行失败: …」，协议收口不是失败 |
| `results` 收齐**先于** `return` | provider 配对硬要求（同批 tool_use 的结果必须进恰一条 user 消息），顺序错了就是 400 |
| 假结果要真的经 `sender` 发出 | 否则 `resume::rebuild` 补 `INTERRUPTED_RESULT`，语义变差 |

**不需要从外部终止会话**：终止在 loop 内闭合，四层全走既有成功链（loop 校验 Ok → `TurnDone`；pump 一轮一命即结束；kernel observation 流结束 + 状态机 Completed；`run_worker` Completed → `StepSettle::passed`）。否决外部终止的理由：唯一既有通道 `RunHandle::request_stop()` 的语义链是 `Stopped → Terminal::stopped → UI「已停止」`，复用它会令一次正常的评估收尾在 UI 上显示成用户终止。另有两条天然兜底：`if !tool_calls.is_empty()` 的工具优先收口使 `finish_step` 不可能落进 `pause_turn` 续跑分支；即使漏了 `return`，下一轮模型看到收口已被接受，大概率自然收敛——只白烧一轮，不死循环。

#### 载荷出口：`SystemNotice`，不改 `TurnOutcome`

```
loop 发 SystemNotice{subtype:"finish_step", payload: FinishPayload(JSON)}
  → 密封事件进 transcript（已在库、已在时间线）
  → worker.rs 提取（沿 transcript::failure_reason 从 api_error 取值的先例）
  → orchestration::transcript::finish_payload(&transcript) -> Option<FinishPayload>
  → WorkerTurnOutcome.finalized
  → walker 直接读，不再 parse 文本
```

否决另两条：`TurnOutcome`（`core/agent/src/port.rs:24` 明说它是「store 轮行落库与 IPC 终态装配共用的唯一收口形状」，塞协议载荷 = 污染共用契约）；`TurnDone` 扩字段（infra 认识 verdict 语义，撞分层三角）。附带收益：`output_truncated` notice 同一转录可取，失败文案能区分「输出被截断」与「模型没调 finish_step」，零额外机制。

#### 分层三角（决定形状的硬约束）

```
core/agent         纯词汇：FinishSpec { EvaluatorChecklist, Decision }
   │               （不认识 EvaluatorChecklist / DecisionAction / 任何工具定义）
   ▼
infra/agent        铸 ToolMount + 分派 + 发 notice（已依赖 orchestration）
   │               （不依赖 workflow —— 拿不到 include_str! 的 md）
   ▼
core/orchestration validate / parameters / description / 载荷类型 + 落账语义
                   （持有全部域类型，零新依赖）
```

`infra/agent` 的 `Cargo.toml` 无 `workflow` 行 ⇒ 动态 schema 的唯一真源不能靠 infra 读 md 文件；解法即上文的「从 prompt 文本解析」。

#### 退役与改写

| 对象 | 处置 |
|---|---|
| `walker.rs` `step_verdict_gate` 反馈环 | 读 `finalized` 替代 `parse_verdict`，环删除 |
| `walker.rs` `step_verdict_gate_terminal` | 与上条合并为单一裁决（判据 `finalized.is_none()`）；`on_fail` 分派随之消失（两值语义合一） |
| `walker.rs` `step_verdict_gate_upgraded_phase_log`（含其 `"评估输出格式"` fail 条目） | **整函数退役**（唯一调用点是反馈环超限分支，`:1645`；环删除后零调用者）。升格落账面在 static-check / archi-check / test-execution / mutation-test 四处保留不动 |
| `walker.rs` `feedback_limit(step, "verdict-gate")` 调用点 | 删除（`feedback_limit` 保留，只服务 static-check / archi-check 两调用点） |
| `walker.rs` `decision_session` drift 环 + `DECISION_DRIFT_LIMIT` | 删除 |
| `prompt.rs::evaluator_protocol` | 从「输出 JSON」改「调 finish_step」 |
| `prompt.rs::verdict_fix_prompt` / `decision_fix_prompt` | 退役（工具 Err 文案成唯一反馈面） |
| `verdict.rs` 文本提取面（`extract_json` / `fenced_block` / `brace_slice` / `parse_verdict`） | 删除（唯一消费者是上两条退役路径）；保留 `MAX_REPORT_CHARS` 与 `EvaluatorChecklist`，新增 `Value` 入参的 `validate_checklist` |
| `decision.rs::parse_decision` | 改为 `Value` 入参的 `validate_decision` |
| compose 的 `archi_matrix` 返回值 | `SessionToolset` → 名字清单 |

**这抬高了 Err 文案的要求**：它必须自足（哪个字段、期望什么、当前值、超了上限多少）。这是本次改动里唯一不能偷懒的文案面。

#### 截断（裁决 9）

`max_tokens` / `context_window_exceeded` 走的是 `finish_success` → status=Completed 且不发 finalized。现状下截断前文本里若已含完整 JSON 仍能 parse 成功；改造后 `finalized` 恒 `None`。按裁决接受回退：`output_truncated` notice 已在 transcript，取它区分文案零成本（纯文案，不影响判据）。

#### 8 份 evaluator md 的同一句改写

8 份 `*-evaluator.md` 都含同一句原文「只产出评估结论：最终消息输出一个 checklist JSON（形状与落账红线由桌面端在会话 prompt 尾部追加的输出协议给出）」——这句把形状定义权明确交给了动态附录，所以只需把「输出 JSON」改成「调 `finish_step`」。是**确定的 8 个文件同一句改写**，不是「0 或 9」。（prompt md 实为 14 份——6 executor + 8 evaluator。）

### ③ checklist 逐项结构化

#### 解析器规范（三条规则，缺一即错）

1. 只扫 `## 静态清单` 标题起、**下一个 `## ` 标题止**的段落；
2. 行首匹配 `^\| ([A-Z]+[0-9]+) \|`（表首列，两侧空格约束）才收；
3. 结果按出现序去重。

三条都实测过反例：不限段落（全文扫）会吃到 `test-design-evaluator.md` 的 `T8-U1` / `T8-G1` 子表；无边界 `[A-Z]+[0-9]+` 会把 `T8-U1` 切成 `T8` / `U1` / `G1` 三个假 ID（宽松计数 15 vs 严格 9）；不去重则正文重复引用产生重复项。

#### 逐文件实测值（直接做成守卫断言，合计 65）

```
acceptance-evaluator          A1..A5                     5
code-analyze-evaluator        CA1..CA8                   8
code-review-evaluator         C1..C7                     7
dev-design-evaluator          D1..D10                   10
implementation-evaluator      I1..I7                     7
proposal-evaluator            R1..R11                   11
test-design-evaluator         T1..T9                     9
test-gen-evaluator            G1..G8                     8
```

两层守卫：① 每文件项数逐文件断言（表结构被改坏立刻红）；② 全文边界 ID 集合 == 段落内 ID 集合（防子表混进主清单；实测 8 个文件全部相等）。

#### 落账面零变更（关键取舍）

模型返回 `{ id, pass, evidence }`，而落账类型是 `ChecklistItem { item, pass, evidence }`。**在工具边界把 ID 映射成 md 表第二列的「检查项」正文**，落账形态完全不变：

```
finish_step 载荷  items: [{ id: "C1", pass: false, evidence: "src/x.rs:42 硬编码密钥" }]
        │
        ├─ 校验：id 集合 == prompt 解析集合（缺项 / 多余 / 未知 ID 三类 Err 文案分列）
        ├─ 映射：id → md 表「检查项」列正文（"变更代码中无安全漏洞"）
        └─ 产出：ChecklistItem { item: <正文>, pass, evidence }
                                  ↓
                     derive_verdict(checklist) → verdict 由推导，不取模型自报
```

结果：**store schema 零变更、bindings 零变更、前端 renderer 零变更**。附带改善：`item` 从「模型自拟的名字」变成「md 里的权威文案」，且 `verdict` 字段从工具参数里彻底消失（verdict 恒由 checklist 推导，`parse_verdict` 与 `derive_verdict` 之间的一致性隐患从源头消失）。`derive_verdict` 自 `phase_log` 的私有函数下沉为 `workflow::model` 的公开单点，供校验器与落账共用（MUST NOT 两份实现）。

#### 机械 checklist 必须显式豁免

`walker.rs` 的机械代写面共 **6 处 `ChecklistItem` 构造点、7 个条目文案**（`item` 为硬编码中文短语，与 md 静态清单无关）：`step_gate_fail_phase_log` 单点（经 `GateFailLog` 参数化，静态检查 `:928` / 架构检查 `:1494` 两调用点共用 `:1958` 一处构造）/ 测试执行（`:2113`）/ 突变测试（`:2187`）/ suite 结论一致（`:1999`）/ 聚合计数一致（`:2005`）/ mutation null 自动通过（`:2013`）。所以「ID 集合相等」只挂在 `ToolExec::FinishStep` 的校验器上；落账路径（`run_phase_log` + `derive_verdict`）对 checklist 完全不变，这 7 个条目 MUST NOT 被要求提供 ID。

**盘面现状是 7 处构造点 / 8 个条目文案；上表是变更后的 6 处 / 7 个。差额即 `"评估输出格式"`（`:2253`，其所在构造点为 `walker.rs` 现盘面的第 7 处）——它不在豁免表内，随所在函数 `step_verdict_gate_upgraded_phase_log` 一并退役**：该函数唯一调用点 `:1645` 位于 verdict 反馈环的超限分支，环删除后只剩零调用者的死代码。保留它即与本变更的自定义规则冲突（见下）：「ID 集合相等」只挂 `finish_step` 校验器、机械条目一律不求 ID——而 `"评估输出格式"` 属「模型输出格式」这一类，工具化后该形态**不再产生**（格式错由工具 Err 当场回灌），落账面无内容可写。

这条退役不是「可选的清理」，而是本变更内部的**判据一致性要求**：

```
判据 A（spec desktop-change-orchestration:15 / :22-25）
  finalized 缺席 → VerdictGate failed → Terminal::failed
  零反馈边重试、状态库无新增 eval 记录
        ⇓
该函数的产出面（一条 pass=false 的 fail PhaseRecord，item = 评估输出格式）
只在「解析失败 → 反馈 5 次 → 升格相位 fail → 回 phase_next 进决策」这一条路径上成立
        ⇓
这条路径已被判据 A 整体删除 ⇒ 保留该函数 = 保留一个不可达的落账面
```

### 决策会话：ask 停等上移出会话

「一调即收口」会让 `DecisionAction::Ask` 的 `wait_answer` 死在这一步。解法是把停等移出决策会话：

```
finish_step(action=ask) → 本 turn 收口（finalized = Ask）
                        → walker 看到 Ask：emit RunUpdate::Ask → wait_answer
                        → 新开一轮 decision 会话（Continue resume 全史）
```

收益：`DECISION_DRIFT_LIMIT = 2` 软拒环**整个退役**（协议校验失败变成工具 Err 当场回灌，不需要跨轮 drift 计数）。`ensure_backtrack_allowed`（白名单门）仍留 walker——它需要运行时 `allowed`，不是自包含校验。

代价（明写进风险）：白名单越权由「软拒两次后终止」变为**一次即终止**。理由是模型侧的 `decision_prompt` 与工具 description 都完整列出白名单，越权属违反明示约束；图面仍留 `WhitelistGate` failed 节点与携 `allowed` 的文案。

### 校验的边界：static 进工具，contextual 留 walker

| 校验 | 能否进 `finish_step` | 理由 |
|---|---|---|
| JSON 形状 / action tag 封闭集 | ✅ | 自包含 |
| report ≤2000 / checklist 非空 / pass 布尔 / ID ∈ prompt 集合 | ✅ | 自包含 |
| backtrack 目标在白名单 / 自环判定 | ❌ | 需运行时 `allowed` |
| ask 停等 | ❌ | 需 `guard` |

所以校验器签名只能是 `(FinishSpec, &Value, prompt) -> Result<FinishPayload, String>` 的纯函数——`infra/agent` 拿不到 `DecisionInput`，`orchestration` 拿不到 `RunGuard`。

### 明确不做（知情边界，非遗漏）

- **`path_field` / `protect` 迁 `ToolMount` 字段**（D6）：代价是加该类工具仍需改 `tools::input_path` 与 `protect::evaluate` 两处。
- **机械 checklist 改稳定 ID**：要动 `phase_log` 落库面与前端 renderer，不在本轮。
- **`parse_verdict` / `derive_verdict` 一致性守卫**：② 落地后该隐患从源头消失，不必先补。
- **工具层重试计数上限**：loop 本就无轮数上限（仅 `pause_turn` 有 5 次续跑预算），「模型修不对格式」与「模型无限调 read」同类。
- **相位表 `VerdictGate` 步的 `on_fail` 声明清理**：反馈环退役后两值语义合一（均为「verdict 缺失即终态」），声明成为惰性字段；表项保持原位（相位表零改动），`feedback_limit` 服务面收缩为 static-check / archi-check。清理属独立变更。
- **steer 通道**：前序 explore 拆出，本课题完成后再议。
- **前端除工具卡外的任何改动**：`SessionInjections` 不出线 IPC（反向锚定已守住），`agent-adapter.ts` / `agent-transport.ts` / `agent-messages.tsx` 零改动。
- **架构模型改动**：不新增 crate、不新增元素、不新增依赖边（`infra/agent` 已有 `orchestration` 依赖）。

### 未决项（不影响开工，写进风险而非范围）

1. `ToolTier::Protocol` 在 debug 会话下是否会触发——debug / explore 不挂 `finish_step`，该档位永不触发，可先恒放行。
2. `description` 随 step 的差异度——evaluator 步两句、decision 步一段，均 <500 字，无 prompt 膨胀风险。

## 能力

### 新增的能力

**无。** 本变更全部落在既有能力面内（引擎工具面 / 架构工具面 / 编排协议 / 相位角色知识 / 前端会话基建 / 写面推导单点），不引入新能力边界——`finish_step` 是既有工具面的一个挂载项、载荷走既有 `SystemNotice` 词汇、checklist 落账形状不变。

### 修改的能力

- **desktop-agent-execution** —
  - **MODIFIED**「SDK 引擎（rig 进程内租户）」：工具面约定由「收窄该面 + 追加受控专用工具（`toolset` 枚举）」改为「名字清单收窄 + 专用工具名字追加」（定义与准入档唯一真源在 infra 挂载清单）；`SessionInjections` 词面为 `preamble` / `tools` / `finish` 三位（`toolset` 零命中）；`RunStarted.tools` 语义（本轮实际下发面）逐字不变。
  - **MODIFIED**「会话级工具面注入与 explore 笔记工具」：`toolset` 位与 `SessionToolset` 枚举退役，`tools` 名字清单升为唯一单源（专用工具以名字进清单、定义由 infra registry 提供，不再经枚举追加）；新增 `finish: Option<FinishSpec>` 位；compose 三臂改产名字清单（explore 六工具 + `create_explore_note`；archi 七工具 + 两 archi 工具；change 按 `(phase, role)` 矩阵产名字），非法名在装配层 `Err`；loop 成员校验与 `RunStarted.tools` 自 mount 清单派生；`create_explore_note` 契约与执行体装配缝逐字不动。
  - **MODIFIED**「SDK 引擎静态权限与路径沙箱」：三档决策表由「档位 × 工具名」改为「档位 × tier」（`Protocol | Read` 恒放行、`Write | Execute` 需非 Default），与原三张常量表逐格等价；沙箱与写入保护闸射程不变。
  - **ADDED**「工具挂载单源与 tier 准入」：`ToolMount` / `ToolTier` / `ToolExec` / `registry()` 十项静态挂载；`definitions()` 按名字查表 + 按 `injections.finish` 铸造 `finish_step`；`TOOL_NAMES` 降级为定义作者处；`protect` / `input_path` 两处按名 match 保留为显式留白。
  - **ADDED**「`finish_step` 工具协议与本步收口」：工具名 / tier / 两形态参数（evaluator checklist 与 decision 三动作）与动态 ID enum 的 prompt 来源；loop 内收口三纪律（假结果 `is_error:false` 且真经 `sender` 发出、`results` 收齐先于 `return`、零额外请求）；载荷出口 `SystemNotice{subtype:"finish_step"}`；截断 → `finalized` 缺席的接受面；Err 文案自足要求。
- **desktop-archi-agent** —
  - **RENAMED**「SessionToolset::Archi 与 (phase, role) 注入矩阵」→「archi 工具面与 (phase, role) 注入矩阵」：矩阵值由枚举改为名字清单，archi 工具与其他专用工具同列（无第二套注入机制）。
  - **MODIFIED**「archi_module 只读工具的定位契约」：注入方式表述由「经 `SessionToolset` 受控注入」改为「经注入面名字清单下发」（响应形状、`api` 并列字段与只读属性逐字不变）。
  - **MODIFIED**「模型 = 源码（写纪律与不设专用写工具）」：「无专用写工具」的判据由「`toolset` 词表内只有两个只读工具」改为「挂载清单内只有 `archi_module` / `archi_search` 两个 archi 只读工具（tier 均为 Read）」；两通路并存的写纪律逐字不变。
  - **MODIFIED**「只读档位归属与成员校验不变」：只读归属由「决策表只读清单追加两名」改为「`ToolTier::Read`」（三档全放行语义不变）；成员校验 / 沙箱 / 写入保护零介入四条逐字不变；per-turn 铸造判据改名字包含。
- **desktop-change-orchestration** —
  - **MODIFIED**「verdict 解析与 phase-log 代写」：收口改为读 `WorkerTurnOutcome.finalized`（经 `SystemNotice{finish_step}` 提取），解析反馈环与 `on_fail` 分派退役，`finalized` 缺席即终态（区分「输出被截断」与「未调 `finish_step`」两文案），落账输入与 `phase_log` 代写语义不变；**连带退役反馈环超限的升格落账面 `step_verdict_gate_upgraded_phase_log` 与其 `"评估输出格式"` 条目**（唯一调用点即被删分支，零调用者不留存）；机械 checklist 代写面收为 7 个条目（四处构造点逐个不动：`step_gate_fail_phase_log` / `test_execution_phase_log_input` 三条目 / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`）。
  - **MODIFIED**「决策协议」：决策输出改为调 `finish_step`（三动作形状进工具参数），封闭集外的动作由工具 Err 当场回灌（不再走跨轮软拒），白名单二次校验与「每 fail 条目只触发一次决策」幂等闸不变。
  - **REMOVED**「决策会话软拒环与漂移上限」（Reason：校验失败已由工具 Err 当场回灌，跨轮 drift 计数无消费者；Migration：白名单越权改为一次即 `Terminal::failed` + `WhitelistGate` failed 节点留痕，ask 停等外移见新增 requirement）。
  - **MODIFIED**「相位 prompt 组装面静态化（静态主体 + 动态 append）」：动态面去掉「verdict 解析失败修复注入」，补「`finish_step` 工具形态由挂载面承载」（协议文本不再进 prompt 主体）。
  - **MODIFIED**「evaluator 输出协议瘦身」：协议附录由「输出 checklist JSON 形状」改为「调 `finish_step` 收口」，形状定义权移交工具参数；`report` 上限与禁调写通道红线保留；两个 `*_fix_prompt` 退役。
  - **ADDED**「决策会话经 `finish_step` 收口与 ask 停等外移」：单轮收口 / `ask` → emit + `wait_answer` → 新一轮 `Continue`（`decision_answer_prompt` 单点）/ 越权一次即停 / `decision-log` 挂账语义不变。
- **desktop-phase-prompts** —
  - **MODIFIED**「桌面端特性适配（迁移是改写非搬运）」：工具引用纪律增「收口工具」一条——`finish_step` 只出现在 8 份 evaluator md（6 份 executor md 零字样，与 archi 工具同款「不许诺未下发工具面」纪律）。
  - **MODIFIED**「evaluator 知识不含落账 / 路由指令」：输出面由「最终消息 checklist JSON」改为「调 `finish_step`」（8 份 md 同一句改写），落账 / 路由禁令逐字不变。
  - **ADDED**「静态清单表为 ID 契约面」：`## 静态清单` 三列表（`ID | 检查项 | 判断依据`）、ID 形如 `[A-Z]+[0-9]+`、子表子项 ID MUST NOT 复用该形态、段落边界与去重规则、8 份项数（5/8/7/10/7/11/9/8）。
- **desktop-agent-chat-infra** —
  - **ADDED**「`finish_step` 工具卡呈现」：`BUILD_IN_TOOL_CARDS['finish_step']` 注册与专用卡摘要（evaluator 派生 verdict + `items` 逐项 / decision `action` 与目标），未注册回退 `ToolPairCard` 的既有行为不变。
- **desktop-workflow-write-face** —
  - **ADDED**「verdict 推导单点（`derive_verdict`））：由 `phase_log` 私有函数下沉为 `workflow::model` 公开单点，校验器与落账共用（MUST NOT 两份实现）；空 checklist → Pass 语义不变。

### 其余能力零改动

- **desktop-agent-chat-infra（除新增卡片外）**：`agent-adapter.ts` / `agent-transport.ts` / `agent-messages.tsx` / `use-agent-session.ts` 零改动（`finish_step` 走既有工具部件管线）。
- **desktop-workspace-store**：`ChecklistItemRecord` 形状与 `PhaseRecord` 落账零变更（`item` 仍是人读正文）。
- **desktop-change-queries**：checklist 线面形状不变（`item` / `pass` / `evidence` 三字段仍是）。
- **desktop-change-chat**：change 来源会话的工具面仍为内建七工具（矩阵未命中行的既有结果），`cwd` 层与提示词前导零改动；spec 中「无新 `SessionToolset` 变体」的表述按本变更后的事实（枚举整体退役）自然成立，无需改文本。
- **desktop-ipc-type-bindings**：`SessionInjections` / `FinishSpec` 不出线 IPC；`bindings:export` 再生成后零 diff。
- **desktop-checks-domain / desktop-test-execution / desktop-mutation-testing**：机械 checklist 代写路径零改动（豁免面）。
- **desktop-crate-layout / repo-archi-model**：零新 crate、零新元素、零新依赖边。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/agent/src/runner.rs` | 删 `SessionToolset` 枚举与 `SessionInjections.toolset` 位；新增 `FinishSpec { EvaluatorChecklist, Decision }`（`Copy + Eq`，`#[serde(rename_all = "camelCase")]`）与 `SessionInjections.finish: Option<FinishSpec>`（`#[serde(default)]`，键恒在）；`Eq` / `PartialEq` 派生链完好 |
| `crates/core/agent/src/lib.rs` | 出口换血：去 `SessionToolset`，加 `FinishSpec` |
| `crates/core/workflow/src/model/checklist.rs`（新） | `ChecklistEntry { id, item }` + `checklist_entries(prompt: &str) -> Vec<ChecklistEntry>`（三规则：段落边界 / 表首列 ID 形态 / 出现序去重；同时取表第二列为权威「检查项」文案） |
| `crates/core/workflow/src/model/mod.rs` | 挂载 `checklist` 模块；导出 `ChecklistEntry` / `checklist_entries` / `derive_verdict` |
| `crates/core/workflow/src/model/domain.rs` | 新增 `pub fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict`（自 `phase_log` 下沉的原实现，语义零变：全 pass → Pass、空表 → Pass） |
| `crates/core/workflow/src/write/phase_log.rs` | 私有 `derive_verdict` 删除，改调 `model::derive_verdict`（行为零变） |
| `crates/core/orchestration/src/finish.rs`（新） | `FinishPayload { Evaluator(EvaluatorChecklist), Decision(DecisionAction) }`（`Serialize + Deserialize`）；`description(spec) -> &'static str`；`parameters(spec, prompt) -> serde_json::Value`（evaluator 臂的 `items[].id.enum` 由 `checklist_entries` 派生）；`validate(spec, args, prompt) -> Result<FinishPayload, String>`（纯函数，Err 文案自足：字段 / 期望 / 当前值 / 超出量） |
| `crates/core/orchestration/src/verdict.rs` | 删 `extract_json` / `fenced_block` / `brace_slice` / `parse_verdict`；`EvaluatorChecklist` 加 `Serialize`；新增 `validate_checklist(args: &Value, prompt: &str) -> Result<EvaluatorChecklist, String>`（形状 + ID 集合比对 + `item` 映射 + report 上限 + `verdict` 由 `derive_verdict` 推导）；`MAX_REPORT_CHARS = 2000` 保留（`infra/archi` 消费者在场） |
| `crates/core/orchestration/src/decision.rs` | 删 `parse_decision`；新增 `validate_decision(args: &Value) -> Result<DecisionAction, String>`（`action` tag 封闭集 / 三动作必填字段 / reason 上限 / options 非空）；`DecisionAction` 加 `Serialize`；`ensure_backtrack_allowed` 逐字不动 |
| `crates/core/orchestration/src/prompt.rs` | `evaluator_protocol` 改「调 `finish_step` 收口」（保留禁调写通道红线与 report 上限措辞）；`decision_prompt` 的协议段改工具协议；删 `verdict_fix_prompt` / `decision_fix_prompt`；新增 `decision_answer_prompt(answer: &str) -> String`（ask 应答的唯一再提示面） |
| `crates/core/orchestration/src/port.rs` | `WorkerTurnOutcome` 增 `finalized: Option<FinishPayload>` |
| `crates/core/orchestration/src/transcript.rs` | 新增 `finish_payload(transcript: &[AgentEvent]) -> Option<FinishPayload>`（末条 `SystemNotice{subtype:"finish_step"}` 的 `payload` 反序列化，失败即 `None`） |
| `crates/core/orchestration/src/walker.rs` | `step_verdict_gate` 与 `step_verdict_gate_terminal` 合并为单一裁决（读 `finalized`；缺席时区分「输出被截断」与「未调 `finish_step`」两文案后 `Terminal::failed`）；`on_fail` 分派与反馈环删除、`feedback_limit(step, "verdict-gate")` 调用点删除（`feedback_limit` 本体保留服务 static-check / archi-check）；**`step_verdict_gate_upgraded_phase_log` 整函数删除**（含其 `"评估输出格式"` fail 条目——唯一调用点在被删的反馈环超限分支，零调用者残留即死代码）；`decision_session` 改单轮收口 + drift 环删除 + ask 停等外移；`PhaseCursor` 的 `verdict` 槽改由 `finalized` 填；`diagnose_brief` 等既有工具函数不动；其余四处机械 checklist 代写点（`step_gate_fail_phase_log` / `test_execution_phase_log_input` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`）逐字不动 |
| `crates/core/orchestration/src/lib.rs` | 导出 `finish::{FinishPayload, description, parameters, validate}`；`WorkerTurnOutcome` 随 `port` 出口自动携带 `finalized` |
| `crates/infra/agent/src/sdk/mount.rs`（新） | `ToolMount { definition, tier, exec }` / `ToolTier` / `ToolExec { Pure, ExploreNote, Archi, FinishStep(FinishSpec) }` / `registry() -> &'static [ToolMount]`（十项）/ `definitions(injections, prompt) -> Vec<ToolMount>`（名字查表 + `finish` 位铸造 `finish_step` + 非法名 `Err`）；七内建与两个 archi / explore 工具的定义函数自 `tools.rs` 原样搬入（字面量零改） |
| `crates/infra/agent/src/sdk/mod.rs` | 挂载 `mount` 模块与 `mount_test` |
| `crates/infra/agent/src/sdk/tools.rs` | `definitions` / `generic_definition` / 两个 archi 定义 / `TOOL_NAMES` 迁出（`TOOL_NAMES` 降级为定义作者处）；保留 `execute` / `input_path` / `cap_result` / 七工具执行体（逐字不动） |
| `crates/infra/agent/src/sdk/policy.rs` | 三张常量表删除，改为 `pub fn allows(mode: AgentPermissionMode, tier: ToolTier) -> bool`（`Protocol \| Read → true`；`Write \| Execute → mode != Default`） |
| `crates/infra/agent/src/sdk/loop.rs` | `definitions(&turn.injections, &turn.question)`；成员校验改 mount 清单命中；`policy::allows(mode, mount.tier)`；执行分派改 `match mount.exec`；`finish_step` 收口（假结果 + `results` 先收齐 + `SystemNotice` + `finish_success` + `return`） |
| `crates/infra/agent/src/sdk/runner.rs` | per-turn 铸造判据由枚举比较改名字包含（`names.contains(EXPLORE_NOTE_TOOL)`；`names.contains(ARCHI_MODULE_TOOL) \|\| names.contains(ARCHI_SEARCH_TOOL)`）；`mint_archi_context` 入参改名字切片 |
| `crates/infra/agent/src/lib.rs` | **注释文案随动**（零行为）：`with_explore_note_store` 的 doc 由「pump 侧仅 `toolset == Explore` 且句柄在场时铸造」改「pump 侧仅名字清单含该工具且句柄在场时铸造」——枚举退役后注释不得再指向已删词汇 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | **注释文案随动**（零行为）：`ArchiContext::new` 的 doc 由「泵起播按 `toolset == Archi { .. }` 铸造」改「泵起播按名字清单含 archi 工具名铸造」 |
| `crates/infra/agent/src/sdk/explore_note.rs` | **注释文案随动**（零行为）：`ExploreNoteContext::new` 的 doc 由「pump 侧 `toolset == Explore` 且句柄与 Continue id 在场时铸造」改「pump 侧名字清单含 `create_explore_note` 且句柄与 Continue id 在场时铸造」 |
| `crates/infra/agent/src/compose.rs` | `assemble_injections` 只产名字清单 + `finish` 位（role 为 `evaluator` / `decision` 时置位）；`archi_matrix` 返回 `Vec<&'static str>`；非法名检查改查 `mount::registry()`；三臂的注释语义同步 |
| `crates/infra/agent/src/worker.rs` | `WorkerTurnOutcome.finalized = transcript::finish_payload(&transcript)` |
| `crates/core/workflow/src/prompts/*-evaluator.md`（8 份） | 「最终消息输出一个 checklist JSON」→「调用 `finish_step` 收口」（同一句改写，逐字一致）；其余正文零改动 |
| `packages/desktop/src/components/agent/agent-blocks.tsx` | 新增 `FinishStepCard`（evaluator 臂摘要 = 派生 verdict 文案 + `items` 逐项 pass 标记与 evidence + report；decision 臂摘要 = `action` 与目标相位 / reason / question+options；`input` 非预期形态回退 JSON 原样，沿 `AskUserQuestionCard` 宽容解析先例；卡根沿用既有 `data-testid="block-tool-use"` + `data-tool-id`，摘要区带 `data-testid="block-finish-step-summary"`） |
| `packages/desktop/src/components/agent/tool-cards.tsx` | `BUILD_IN_TOOL_CARDS['finish_step'] = FinishStepCard` |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前盘面 `0.6.33` → 落地时以 `git diff HEAD -- packages/desktop/package.json` 的版本行判「恰好 +1」为准；沿 `desktop-archi-api`「版本纪律」相对递增口径，MUST NOT 写死字面号——现值随每次源码改动滚动）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `FinishSpec` serde 线值（`evaluatorChecklist` / `decision`）、`finish` 缺省读 `None`、`SessionInjections` 三字段 round-trip、`Eq` 链完好；`SessionToolset` 用例整体退役 | `crates/core/agent/src/runner_test.rs`、`lib_test.rs` | 改造（旧枚举用例删除） |
| `SessionInjections` 构造面换位（新增 `finish` 字段） | `crates/core/agent/src/kernel_test.rs` | 夹具随动 |
| 三规则解析：段落边界（`###` 子表行不收）、表首列 ID 形态（`T8-U1` / `T8-G1` 零命中）、出现序去重、第二列「检查项」正文取值 | `crates/core/workflow/src/model/checklist_test.rs`（新） | 新建，含三条实测反例 |
| 8 份 evaluator prompt 逐文件项数（5/8/7/10/7/11/9/8）+ 段落与全文 ID 集合相等；表结构改坏即红 | `crates/core/workflow/src/write/phase_table_test.rs` | 新增（prompt 文本自相位表取得，零文件 IO）；同文件两处 `SessionToolset` 注释文案改写（断言本体零改） |
| `derive_verdict` 迁位后语义不变（全 pass / 含 fail / 空表三态） | `crates/core/workflow/src/write/phase_log_test.rs` | 既有用例断言面零改；新增按 `model::derive_verdict` 直接调用的三态锚定 |
| `derive_verdict` 单点行为面三态 + 单元素 / 末项决定结论 / 1000 项规模 / 只依赖 `pass` 位 + 「全树恰 1 处 `fn derive_verdict`」源码自证 | `crates/core/workflow/src/model/domain_test.rs`（新） | 新建（`model/mod.rs` 挂 `#[cfg(test)] mod domain_test;`，属编译前置的挂载声明） |
| `validate_checklist`：缺项 / 多余 / 未知 ID 三类 Err 文案分列且自足；`item` 映射为 md 正文；report 超限报当前值；`verdict` 恒由推导（模型给 verdict 无效）；`validate_decision` 三动作合法 / 未知 action / 缺字段 / reason 超限 | `crates/core/orchestration/src/verdict_test.rs`、`decision_test.rs` | 改造（文本解析用例删除，Value 校验用例新增） |
| `parameters(FinishSpec::EvaluatorChecklist, prompt).items[0].id.enum` == 该 prompt 的 ID 集；decision 臂为三动作形状；`description` 两形态文本非空 | `crates/core/orchestration/src/finish_test.rs`（新） | 新建 |
| `finish_payload` 提取：末条 notice 命中 / 缺 notice → `None` / payload 损坏 → `None` / 多条取末条 | `crates/core/orchestration/src/transcript_test.rs` | 新增 |
| `evaluator_protocol` 改写后断言（含「调 `finish_step`」、无 JSON 形状、保留禁调红线与 report 上限）；`decision_prompt` 协议段改工具协议；`decision_answer_prompt` 单点；两个 `*_fix_prompt` 零命中 | `crates/core/orchestration/src/prompt_test.rs` | 改造（退役函数段落断言删除） |
| verdict 门：`finalized` 在 → 落账并 `verdict=…` 节点 detail；缺席且 Completed → `VerdictGate` failed + `Terminal::failed`（截断 / 未调 `finish_step` 两文案）；反馈边节点零产出 | `crates/core/orchestration/src/walker_test.rs` | 改造（`FakeWorker` 产出夹具由「文本 JSON」改为注 `SystemNotice{finish_step}`，经既有 `with_transcripts()` 注入口） |
| **退役用例**：`verdict解析失败反馈边修复_续注同evaluator会话重解析`（`:3734`）与 `真实写面组合_verdict解析失败反馈边超限_升格双槽位决策stop`（`:3844`）整体删除 | `crates/core/orchestration/src/walker_test.rs` | 删除（两条用例的被测路径——反馈环续注与超限升格双槽位落账——随反馈环与 `step_verdict_gate_upgraded_phase_log` 一同消失；后者的 `item == "评估输出格式"`（`:3947`）与 30 轮 evaluator 会话计数断言无替代面，「`finalized` 缺席即 `Terminal::failed`」由上一行新用例承接） |
| `feedback_limit` 服务面收窄：仍为五值边界用例（`FeedbackLoop{0,1,u32::MAX}` / `Terminal` / `None`），但**只经 static-check 步驱动**；`feedback_limit_escalate误声明显式漂移err`（`:11094`）保留 | `crates/core/orchestration/src/walker_test.rs` | 复用（零新增、零删除——现有用例本就以 static-check 为驱动面，verdict-gate 调用点删除后不改断言） |
| 决策：单轮收口（会话数 1、轮数 1）；ask → `RunUpdate::Ask` + 应答后新一轮 `Continue`（会话数 1、轮数 2、第二轮 prompt 为 `decision_answer_prompt`）；越权 → `WhitelistGate` failed + `Terminal::failed`（一次即停）；无 drift 计数残留 | `crates/core/orchestration/src/walker_test.rs` | 改造（`with_decision_reports` 夹具改注 notice） |
| `WorkerTurnOutcome.finalized` 字段在场与默认 `None` | `crates/core/orchestration/src/port_test.rs` | 断言随动 |
| `registry()` 十项唯一名清单；名字 → mount 查表；`definitions` 非法名 `Err`；`tools: None` = 内建七件；`finish` 位置位时追加 `finish_step` 且参数 enum == prompt ID 集；其余位不追加 | `crates/infra/agent/src/sdk/mount_test.rs`（新） | 新建 |
| tier 准入 4 × 3 矩阵逐格（Default 只放 Read / Protocol）；与改造前三张常量表逐格等价 | `crates/infra/agent/src/sdk/policy_test.rs` | 重写 12 格 |
| `finish_step` 收口三纪律：同批后续调用假结果 `is_error:false` 且经 `sender` 发出（转录可见）、`results` 收齐先于 `return`（同批 tool_use 结果进恰一条 user 消息）、收口后零额外请求且 `TurnDone{is_error:false}`；校验 Err 时 is_error ToolResult 回灌且会话继续 | `crates/infra/agent/src/sdk/loop_test.rs` | 新增 + 既有 `SessionInjections` 构造面换位（9 处） |
| `definitions` 调用点补 prompt 实参（17 处）；注入面断言由 `.toolset` 改名字清单 | `crates/infra/agent/src/sdk/tools_test.rs` | 夹具换位（新增 `mount::injected(&[&str])` 夹具压平 10 处构造） |
| per-turn 铸造判据改名字包含（explore / archi 两臂正反例） | `crates/infra/agent/src/sdk/runner_test.rs` | 新增用例 + 5 处构造换位 |
| compose 三臂产名字清单、`finish` 位按 role 置位、非法名 `Err` | `crates/infra/agent/src/compose_test.rs` | 改造（21 处读侧断言换位） |
| `finalized` 提取贯通（注 notice 的转录 → outcome） | `crates/infra/agent/src/worker_test.rs` | 新增 |
| 引擎侧会话词汇不出线 IPC：反向锚定清单去掉 `SessionToolset`（加 `FinishSpec`） | `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 改造（否则编译期即红） |
| `finish_step` 卡：evaluator 形态（ids / pass 标记 / report）与 decision 形态（action 摘要）渲染；未预期 input 回退 JSON；注册表含 `finish_step` | `packages/desktop/src/components/agent/tool-cards.test.tsx`、`agent-blocks.test.tsx` | 新增 + 既有注册表断言扩位 |
| `tool-cards.tsx` **整文件突变面**（含改动前零覆盖面）：`BashCard` / `ReadCard` / `WriteCard` / `EditCard` 四卡逐态渲染（`aria-disabled` / `defaultOpen` / `data-status` / `isError` 的「错误提示 + 正文」两处并存 / 高亮行与纯文本切行以行数区分）、私有 `rangeSuffix`（有 `limit` / 无 `limit` / `offset` 非 number / `0` 边界）、私有 `languageOf`（`null` / 无点 / 尾点 / 末段 / 隐藏文件）、私有 `cardAttrs`（四卡同形 + `AskUserQuestion` 对照）、`parseAskInput` / `parseOptions` 宽容解析存活面 | `packages/desktop/src/components/agent/tool-cards.test.tsx` | 新增（现状 27.13%，见下「突变面补足目标」） |
| `agent-blocks.tsx` **整文件突变面**（含改动前零覆盖面）：`sameCitations` 比较器（三字段单点 / 部分相等 / 长度先判 / 同引用短路 / 缺席与空数组互换）、`ThinkingBlock` state 两态、`CitationList` 四级回退链与 `dedupeByUrl` 无 url 条目、`WebSearchCard` 查询词空值 / `content` 空串 / `search.error` 空串 / 来源三缺席形态、`ExternalLink` 行首 scheme 锚定与 `openUrl` reject、`isToolPart` 收窄、`FinishStepCard` 逐项行结构（span 计数）/ `text-pass`·`text-fail` 着色 / `report` 在场判据 / decision 缺字段逐字摘要 | `packages/desktop/src/components/agent/agent-blocks.test.tsx` | 新增（现状 73.16%，见下「突变面补足目标」） |

#### 突变面补足目标（`tool-cards.tsx` / `agent-blocks.tsx` 整文件口径）

**为何按整文件取面**：两文件都是本 change 的整文件编辑集（`agent-blocks.tsx` 新增 `FinishStepCard` + 收口摘要解析，`tool-cards.tsx` 增注册键），而 `desktop-mutation-testing` 的 `mutate` 面以 change 编辑集为唯一单源——既有分支与新增组件同在被测面，不存在「只测新增部分」的口径。之前一版补充代码把用例全押在 `finish_step` / `web_search` / `AskUserQuestion` 三键上，四张既有内建卡与两处比较器从未被渲染，故整文件得分不足（suites 桶仍按 80 阈值判定）。

现状基线（`reports/test/packages_desktop_vite-plus/mutation/mutation-seg-0.json`）：

| 文件 | 突变体 | 分母（剔 Ignored） | killed | 未杀死 | 得分 | 目标 |
|---|---|---|---|---|---|---|
| `tool-cards.tsx` | 130 | 129 | 35 | 94（76 零覆盖 + 18 幸存） | 27.13 | ≥ 80 |
| `agent-blocks.tsx` | 233 | 231 | 169 | 62（13 零覆盖 + 49 幸存） | 73.16 | ≥ 80 |
| suite 桶合计 | 363 | 360 | 204 | 156 | 56.67 | ≥ 80 |

成因（逐块可归因，不是「覆盖不够」一句话）：

- **`tool-cards.tsx` 的 76 枚零覆盖与 18 枚幸存全部落在四张既有内建卡与三个私有原语上**：注册表用例只判键名与引用相等（`toBe(WebSearchCard)`），从未经注册表把 `bash` / `read` / `write` / `edit` 真正渲出来。零覆盖逐块：`WriteCard` 18 / `EditCard` 26 / `languageOf` 13 / `rangeSuffix` 8 / `BashCard` 7 / `ReadCard` 2 / `AskUserQuestion` 解析面 2；幸存逐块：`ReadCard` 7 / `rangeSuffix` 6 / `AskUserQuestion` 解析面 4 / `AskQuestionView` 1。四张卡的两向分支（`output` 缺席的 `disabled` / `defaultOpen`、`isError` 的「错误提示 + 正文」并存、高亮行与纯文本切行）一枚都没被驱动过。
- **`agent-blocks.tsx` 的 62 枚未杀死里 22 枚集中在 `sameCitations` / `TextBlock` 的 memo 比较器**：比较器只有一个可观测面——「同一 `text`、`citations` 换引用后引用列表是否更新」，而现有用例每条只渲染一次，比较器恒短路在「首渲染」。其余分布在 `finish_step` 面（收口摘要解析 + 卡体渲染，合计 18 枚；主因是缺字段 / `?? ''` 兜底分支与 `report` 的「在场即渲染」判据未逐形态驱动）、`ThinkingBlock` 两态（10 枚）、`WebSearchCard` 的 `content` / `error` 空串面（5 枚）、`isToolPart` / `isHttpUrl` 的不可达合取（4 枚）、`CitationList` / `dedupeByUrl`（2 枚）、`searchQuery`（1 枚）。

三条目标判据：

1. **逐文件 score ≥ 80**（`openspec/config.json` 的 `tests[0].mutation.score`，与 suite 桶阈值同源），按 `(killed + timeout) / (total - ignored - compileError - runtimeError)` 重算，MUST NOT 取百分比平均或按文件加权。
2. **未杀死项逐条归因，不留「待补」**：确认在**任何输入下产出同一 DOM**的等价突变体不追杀，逐条登记进 `test-design.md`「不可测试项」（预期面：`isToolPart` 的第二合取 `!== 'dynamic-tool'`、`sameCitations` 的 `every → {}` 与比较器整体 `→ false`、`BashCard` / `ReadCard` 的 `{output?.content}` 在 `disabled` 折叠下的 `null` 分支、`EditCard` 两区 `key` 模板串、`searchQuery` 的 `query === null → false`）；登记后「未杀死集合 = 等价登记 ∪ 有判据用例」，任何第三类即是缺口。
3. **补齐面 MUST NOT 新增导出**：`rangeSuffix` / `languageOf` / `cardAttrs` / `parseAskInput` / `sameCitations` 等私有件一律经注册表取卡渲染或公开组件间接驱动（沿 knip 纪律）；高亮面经 `@/lib/highlighter` 替身注入（进程边界 = shiki 语法的动态 import），不为可测性改实现。

### 不要修改

- **`crates/infra/agent/src/sdk/protect/**`**：`evaluate` 的 `"write" / "edit"` 按路径、`"bash"` 按命令文本两臂保留（D6 的机械判据——该目录出现 diff 即说明实现偏离）。
- **`crates/infra/agent/src/sdk/tools.rs::input_path`**：`glob` 取 `pattern`、两 archi 工具显式 `None` 的特判保留。
- **`crates/infra/agent/src/sdk/{sandbox,context,compact,resume,normalize,web_search,bash,model_spec}.rs`**：零改动（守卫链、防线、回放、工具执行体皆不涉协议变更）。
- **`crates/infra/store/**`**：store schema 与 `ChecklistItemRecord` 形状零变更（`item` 仍是人读正文）。
- **`crates/core/workflow/src/write/phase_table.rs`**：步序列、`include_str!` 装配与 `VerdictGate` 步的 `on_fail` 声明保持原位（声明成为惰性字段，清理留独立变更）。
- **`crates/core/orchestration/src/walker.rs` 的其余四个机械 checklist 代写点**：`step_gate_fail_phase_log`（`GateFailLog` 单点，静态检查 `:928` / 架构检查 `:1494` 两调用点共用）+ `test_execution_phase_log_input`（`:1999` / `:2005` / `:2013` 三条目）+ `step_test_execution_upgraded_phase_log`（`:2113`）+ `step_mutation_test_upgraded_phase_log`（`:2187`）——合计 7 个条目文案逐个保留原位；**只有 `step_verdict_gate_upgraded_phase_log`（含 `"评估输出格式"` 条目）是退役面**，该四块出现 diff 即说明改动越界。
- **`crates/core/orchestration/src/walker.rs::feedback_limit`**：函数本体逐字不动（含 `Escalate` / `Terminal` / `None` 三臂的既有漂移 `Err` 文案），只删 verdict-gate 一处调用点。
- **`crates/core/workflow/src/prompts/*-planner.md` / `*-generator.md`**（6 份 executor）：零改动（收口协议只作用于 evaluator / decision）。
- **`packages/desktop/src/lib/agent-adapter.ts` / `agent-transport.ts` / `src/hooks/use-agent-session.ts` / `src/components/agent/agent-messages.tsx`**：零改动（`finish_step` 走既有工具部件管线与 `BUILD_IN_TOOL_CARDS` 注册表）。
- **`packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs::ensure_backtrack_allowed`**：白名单门逻辑逐字不动（只是调用方由软拒环降为一次判定）。
- **`.dev-team/architecture/models/**`**：零新元素、零新依赖边（`infra/agent` 已有 `orchestration` 依赖）。
- **`openspec/changes/agent-finalize-tool-for-flow-steps/explore.md`**：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | **工具面单源**：`mount::registry()` 是唯一工具事实源（十项：`read` / `grep` / `glob` / `ls` / `write` / `edit` / `bash` / `create_explore_note` / `archi_module` / `archi_search`）；`definitions()` 按名字查表，`tools: None` = 内建七件；专用工具（`create_explore_note` 与两 archi 工具）以**名字**进清单而非经枚举追加；非法名在 `assemble_injections` 层 `Err`（文案携非法名）；`TOOL_NAMES` 不参与任何判定（仅定义作者处引用） | `mount_test`（名字 → mount 查表 + 十项清单 + 非法名 Err）+ `compose_test` 三臂；源码复核 `TOOL_NAMES` 消费点 |
| AC-2 | **tier 准入等价**：`policy::allows(mode, tier)` 的 4 × 3 矩阵逐格——Default 只放 `Read` / `Protocol`；AcceptEdits / BypassPermissions 全放；与改造前三张常量表的判定**逐格相同**（含 `create_explore_note` 归 Write、两 archi 工具归 Read、`bash` 归 Execute） | `policy_test` 12 格重写 + 与原表矩阵对拍断言 |
| AC-3 | **core 词汇面形态**：`SessionToolset` 枚举与 `SessionInjections.toolset` 位删除；`FinishSpec { EvaluatorChecklist, Decision }` 为 `Copy + Eq` 闭合枚举且 `#[serde(rename_all = "camelCase")]`；`SessionInjections { preamble, tools, finish }` 三字段、`finish` 带 `#[serde(default)]`（旧线缺字段读 `None`）、`PartialEq + Eq` 齐全；`crates/core/agent/src/lib.rs` 出口去 `SessionToolset` 加 `FinishSpec` | `runner_test` / `lib_test` serde 与构造用例；`SessionToolset` 全仓零命中（`grep`） |
| AC-4 | **`finish_step` 挂载与动态 schema**：仅 evaluator 步（`injections.finish = Some(EvaluatorChecklist)`）挂 evaluator 形态、decision 步挂三动作形态、其余步不挂；evaluator 形态 `parameters.properties.items.items.properties.id.enum` **逐值等于该轮 prompt 的 `## 静态清单` ID 集**；`tier == Protocol`（三档全放行）；`RunStarted.tools` 含 `finish_step` 时其余名单不变 | `finish_test`（enum 与 `checklist_entries` 对拍）+ `mount_test`（按 `finish` 位挂载）+ `loop_test`（`RunStarted.tools`） |
| AC-5 | **loop 内收口三纪律**：① 同批中 `finish_step` 之后的调用产出假结果且 `is_error: false`、文本「本步已 finish_step 收口，该调用未执行」、**经 `sender` 发出**（转录内可见，无 `INTERRUPTED_RESULT`）；② 同批全部 `tool_use` 的结果进**恰一条** user 消息（`results` 收齐先于 `return`）；③ 校验 Ok → `SystemNotice{subtype:"finish_step"}` + `TurnDone{is_error: false}` + 零额外请求；校验 Err → `is_error: true` ToolResult 当场回灌且会话继续（`turn_index` 递增） | `loop_test` 脚本化流缝逐条断言（含假结果是否真发与消息条数） |
| AC-6 | **`finalized` 出口**：`orchestration::transcript::finish_payload` 从转录末条 `SystemNotice{finish_step}` 提取 `FinishPayload`（缺 notice / payload 损坏 → `None`）；`WorkerTurnOutcome.finalized` 由 `worker.rs` 填充；walker 全链**零文本解析**（`parse_verdict` / `parse_decision` 全仓零命中） | `transcript_test` / `worker_test` / `port_test` + `grep` 复核退役函数零存在 |
| AC-7 | **verdict 门**：`finalized = Some(Evaluator(..))` → `cursor.verdict` 落位、节点 detail 含 `verdict=…`、交 PhaseLog 落账；`finalized` 缺席（Completed 却未调 `finish_step`，或截断终局）→ `VerdictGate` failed 节点 + `Terminal::failed`，两成因文案可区分（截断文案取 `output_truncated` notice）；反馈边节点零产出、`verdict_fix_prompt` 退役 | `walker_test`（`with_transcripts` 注 notice 两态 + 截断态）+ `prompt_test` 退役断言 |
| AC-8 | **决策协议**：决策会话单轮内收口（`decision-log` 挂账一次、会话数 1）；`ask` → `RunUpdate::Ask` → `wait_answer` → 新一轮 `Continue`（会话数 1、轮数 2、第二轮 prompt 为 `decision_answer_prompt`），ask 不计失败、不计漂移；越权 `backtrack` → `WhitelistGate` failed 节点 + `Terminal::failed`（一次即停，文案携 `allowed`）；`DECISION_DRIFT_LIMIT` / `decision_fix_prompt` 全仓零命中；`ensure_backtrack_allowed` 逐字未改 | `walker_test`（三形态：直接收口 / ask 应答 / 越权）+ `prompt_test` + `grep` |
| AC-9 | **checklist 逐项校验**：`validate_checklist` 对「缺项 / 多余（ID 不在 prompt 集合）/ 未知 ID」三类 Err 文案分列且自足（含字段名、期望 ID 集、实际收到的 ID）；`item` 由 ID 映射为 md 表「检查项」列**逐字正文**；`report` 超 2000 时报当前字符数；模型传入的 `verdict` 字段被忽略、`verdict` 恒由 `derive_verdict(checklist)` 推导；落账 `ChecklistItem` 三字段形状不变 | `verdict_test` 逐规则 + `phase_log_test` 落账不变 |
| AC-10 | **机械 checklist 豁免与死面退役**：① `ID 集合相等` 校验只作用于 `ToolExec::FinishStep(EvaluatorChecklist)`；walker 的 **7 个机械代写条目**（静态检查 / 架构检查 / 测试执行 / 突变测试 / suite 结论一致 / 聚合计数一致 / mutation null 自动通过）零改动、不经 `finish_step`、不需要 ID。② **`step_verdict_gate_upgraded_phase_log` 与 `"评估输出格式"` 条目全仓零存在**——该函数唯一调用点在被删的反馈环超限分支，保留即零调用者死代码；`feedback_limit` 仍存但只服务 static-check / archi-check 两调用点（`grep -c 'feedback_limit(step, "verdict-gate")'` = 0） | ① `walker_test` 机械分支用例不回归 + 源码复核校验器调用点唯一；② `grep` 复核 `step_verdict_gate_upgraded_phase_log` / `"评估输出格式"` / `评估输出反馈边超限` 三项全仓零命中，`feedback_limit` 三调用点去一留二 |
| AC-11 | **解析器双层守卫**：8 份 evaluator prompt 逐文件项数 = `acceptance 5` / `code-analyze 8` / `code-review 7` / `dev-design 10` / `implementation 7` / `proposal 11` / `test-design 9` / `test-gen 8`（合计 65）；每份「全文边界 ID 集合 == 段落内 ID 集合」（无多无少，`T8-U1` / `T8-G1` 等子项零命中） | `phase_table_test`（prompt 文本取自相位表，零文件 IO） |
| AC-12 | **prompt 文本面**：8 份 `*-evaluator.md` 的输出面同句改写为「调用 `finish_step` 收口」（逐字一致、8 份全命中、旧串零命中）；6 份 planner / generator 零改动且 `finish_step` 零字样；`evaluator_protocol` 不再要求最终消息输出 JSON 形状、保留禁调写通道红线与 `report` 上限；`decision_prompt` 协议段改工具协议 | `grep -cF` 新旧串核对 + `prompt_test` 断言 |
| AC-13 | **前端工具卡**：`BUILD_IN_TOOL_CARDS['finish_step']` 在场；evaluator 形态卡呈派生 verdict 摘要 + `items` 逐项（ID / pass 标记 / evidence）+ `report`；decision 形态卡呈 `action` 摘要（backtrack 目标 / stop / ask 问题与选项）；`input` 非预期形态回退 JSON 原样且不抛错；未注册工具的 `ToolPairCard` 回退行为零变；**整文件突变面**：`tool-cards.tsx` 与 `agent-blocks.tsx` 逐文件 score ≥ 80（现状 27.13 / 73.16），未杀死项 = 等价突变体登记（逐条进 `test-design.md`「不可测试项」）∪ 有判据用例，无第三类；补齐面零新增导出 | `tool-cards.test.tsx` + `agent-blocks.test.tsx`；判据按 `reports/test/packages_desktop_vite-plus/mutation/mutation-seg-0.json` 逐条核对（见「突变面补足目标」） |

> **验收表口径**：AC-1…AC-13 是本变更的**能力验收面**（每条对应本 change `specs/` 下某个 delta requirement 的行为判据）。静态检查链、全量测试与版本递增**不进验收表**——它们是全仓既有的交付门，由主基线承载而非本变更引入（见下「交付门」）。

### 验证命令

**能力验收**（对应 AC-1…AC-13，逐包定向测试）：

```powershell
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p workflow
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p archi
pnpm -C packages/desktop exec vp test run src/components/agent/tool-cards.test.tsx src/components/agent/agent-blocks.test.tsx
```

**AC-13 突变面**（整文件口径，落定后重跑并逐文件核对 score ≥ 80）：

```powershell
# 重跑由 test-execution 相位按 change 编辑集口径产出（mutate 面收窄到本 change 的两文件）
pnpm -C packages/desktop run mutate
# 判据读 reports/test/packages_desktop_vite-plus/mutation/mutation-seg-0.json，逐文件重算
# score = (killed + timeout) / (total - ignored - compileError - runtimeError)
```

**交付门**（全仓既有门，非本变更能力判据；版本行按「落地时 HEAD +1 patch」相对递增核对）：

```powershell
cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml --all-targets
pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run check      # server:check → bindings:export → api:check → client:check
pnpm -C packages/desktop test
git diff HEAD -- packages/desktop/package.json   # version 行恰 +1 patch
```

> **验收面口径**：验收止于**单元测试**。真机 E2E（真实 provider 端点下的 evaluator / decision 会话收口）不属本工作流自动化验证面；`loop_test` 以 rig `test-utils` 流缝脚本化覆盖「收口 / Err 回灌 / 同批假结果 / 截断」四条路径，这四条即是本变更的全部行为面。静态检查链与全量测试是全仓交付门（主基线在案），不作为本变更的验收条目。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 `walker_test.rs` 夹具改造成本**（15862 行；`with_evaluator_reports` 18 处 / `with_decision_reports` 35 处从「文本 JSON」改为「注 notice」） | 改造面大、易引入无关断言漂移 | 高 | 夹具改由**注 `SystemNotice`** 承载（沿 `with_transcripts()` 已有注入口，比改 `WorkerTurnOutcome` 构造轻）；按三步交付分提交，①的测试面在 Step 3 单独一波；实测量已列在「测试文件」表 |
| **R2 同批假结果三处纪律写错即 400 / 语义污染** | provider 拒绝（同批 tool_use 结果未进恰一条消息）、或转录里出现「工具执行失败」假因 | 中 | AC-5 三条逐条断言（`is_error:false` / 消息条数 / 是否真经 `sender` 发出）；spec 正文把三点列成「必须守住的点」表 |
| **R3 白名单越权由软拒两次降为一次即停**（drift 环退役的必然代价） | 模型一次越权即终止 run，纠正窗口收窄 | 中 | 越权属违反**双处明示**的约束（`decision_prompt` 的白名单段 + `finish_step` 的 decision 形态 description 明示「`to` 必须取 prompt 白名单内相位」）；`WhitelistGate` failed 节点 + 携 `allowed` 的文案保证可归因；spec 显式记录该降级（Migration 段） |
| **R4 ID 解析漂移**（子表混入主清单 / 无边界正则切出假 ID / 重复项） | 校验集合与模型眼前清单不一致 → 误拒或误收 | 中 | AC-11 双层守卫（逐文件项数 + 段落/全文集合相等）；三条实测反例进 `checklist_test`；解析规则与校验器共用同一函数（MUST NOT 两份） |
| **R5 截断回退**：截断 → `finalized` 缺席 → 判失败（比今日「截断前文本仍可 parse」更严） | 长报告 evaluator 一次撞 `max_tokens` 即相位 fail | 中 | D9 显式接受；`output_truncated` notice 已在转录，失败文案区分「输出被截断」与「未调 `finish_step`」（可归因即用户可调 `max_tokens`）；MUST NOT 加降级分支（两套收口路径的代价高于偶发重跑） |
| **R6 `SessionInjections` 形态改动波及面**（34 处字面构造 + 47 处读侧断言 + 18 处 `definitions` 调用点） | 编译期全暴露但机械改动量大 | 高 | 新增 `mount::injected(&[&str])` 夹具压平构造面；`definitions()` 加参为机械替换；Step 0（`mount.rs` + 解析器 + 守卫）**单独提交**，此后错误可归因于「接线」而非「数据」 |
| **R7 `finish_step` 出现在 `RunStarted.tools` 与转录** | 时间线上每次收口摊一大坨 JSON（无卡片时回退 `ToolPairCard`） | 中 | AC-13 专用卡（两形态摘要）；卡片缺省时回退语义不破（既有注册表 miss 分支） |
| **R8 ask 停等外移改变决策会话拓扑**（一轮一命 → 应答后再一轮 `Continue`） | 若 `decision-log` 挂账或图面节点与轮数错位，会话槽位会漂 | 中 | AC-8 断言会话数 1 / 轮数 2 / 挂账仍幂等覆写；`WhitelistGate` 每轮一节点的既有语义只作用于「parsed 动作」轮（应答轮为新的决策轮） |
| **R9 退役函数的既有消费者遗漏**（`parse_verdict` / `extract_json` / 两个 `*_fix_prompt` / `step_verdict_gate_upgraded_phase_log`） | 残留死代码或被隐式调用点打红 | 低 | `MAX_REPORT_CHARS` 有 `infra/archi` 消费者故保留（非退役面）；AC-6 / AC-8 / AC-10② 以 `grep` 零命中为验收口径；`step_verdict_gate_upgraded_phase_log` 的唯一调用点（反馈环超限分支）与被测用例（`:3734` / `:3844`）同批删除，删除动作在**同一提交**内完成（拆开即零调用者死代码或编译期 red） |
| **R10 版本与绑定纪律** | 交付门红 | 低 | `package.json` `version` 按「落地时 HEAD 现值 +1 patch」相对递增（MUST NOT 写死字面号，判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行）；`SessionInjections` / `FinishSpec` 不出线 IPC（反向锚定守）+ `bindings:export` 再生成后零 diff；零新 crate / 零新元素 ⇒ 架构模型零改动。**该项是交付门而非能力验收**，故不进 AC 表 |
