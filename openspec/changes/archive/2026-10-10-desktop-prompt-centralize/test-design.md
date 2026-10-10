# 测试设计: desktop-prompt-centralize

> **变更**: desktop-prompt-centralize
> **日期**: 2026-10-10
> **依据**: proposal.md（AC-1..AC-9）+ design.md（D1–D9 决策编号）；实现任务边界见 tasks.md（测试文件清单与其「test-design / test-gen / test-execution 阶段承接」条目由本文件展开）

---

## 测试边界与框架识别

<!-- 逐文件推导自 design.md 变更清单（实现文件 + 测试文件两节）与公共 API 表；
     实现阶段零清单外测试文件。 -->

- **纯 Rust 面**：src-tauri cargo workspace 套件，测试文件为共置 `*_test.rs` 模块（沿 workflow / orchestration 既有先例）。本变更**零前端、零进程边界、零新增依赖**——`include_str!` 为 std 宏，无新 crate 引入。
- **测试文件面**（与 proposal「测试文件」节 + tasks.md 三阶段逐条对齐）：
  - `crates/core/workflow/src/write/phase_table_test.rs` — 删 `interpolate` 三用例与 `__CALL_AGENT` 断言；新增 `PhaseAgentSpec` 结构收敛 + prompt 静态非空锚；相位序 / model_level 对照不变。
  - `crates/core/workflow/src/write/phase_next_test.rs` — 插值断言改上下文头 `change: <name>` 在场（无 `<change>` / `<phase>` / CHANGE_ID）；回溯原因后缀断言保留。
  - `crates/core/orchestration/src/prompt_test.rs` — 删 `executor_prompt` / `role_brief` / `strip_call_agent` 断言；`evaluator_prompt` 协议断言改瘦身形状；`prompt确定性` 去 executor 恒等断言。
  - `crates/core/orchestration/src/verdict_test.rs` — `EvaluatorChecklist` 瘦身形状解析回环；phase / attempt / skipped 三字段用例删除或改走「多余字段静默忽略」面。
  - `crates/core/orchestration/src/walker_test.rs` — `route_with_round` 装置 `PhaseAgentSpec` → `ResolvedPhaseSpec`（去 `agent_type`）；新增上下文头消费面断言。
- **迭代类型词表**：**新增**（新用例）/ **重写**（既有用例语义演进改写）/ **适配**（机械传参或 fixture 补齐，既有断言零改动）/ **持衡（沿用）** / **废弃**（随删除面删除或改写）。本变更零迁移。
- **无新增集成测试**：本变更不改 store schema / 相位机操作语义 / 结构化通道，proposal「全管线测试（`vp test` / `client:check` / knip）与 AC-1 ~ AC-4 人工核验归 test-execution 阶段承载」，不落本文件自动化用例。真实 db 组合（workflow 自环 dev-dep 双工件类型不统一的既有结论）不在本变更测试面——相位机语义回归由既有假写面 / 假引擎用例持衡承载（AC-8）。
- **静态内容约定守线**（D6 用户拍板「不建通配符守门测试」）：AC-1（14 md 清单 / 内容结构）、AC-2（零占位）、AC-3（显式 `include_str!`）、AC-7（桌面特性适配）为 md 内容约定与人工核验面，**不建自动化断言**，统一落「不可测试项」。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 9 条 AC。纯静态 / 流程性 / 人工核验半边落
     「—（见不可测试项 N）」；可自动化半边落到具体测试文件。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 角色知识集中地：`core/workflow/src/prompts/` 14 个 md（6 executor + 8 evaluator）在案、文件名与角色一一对应、每文件含角色自述 + 阶段要求 + checklist（evaluator 含静态清单表） | —（见不可测试项 1） |
| AC-2 | 全静态零占位：14 md 无 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:*` / `__INCLUDE:*` / `{{...}}` 等运行时占位符或令牌 | —（见不可测试项 1；D6 内容约定不建守门测试） |
| AC-3 | 显式 `include_str!`：`phase_table.rs` 每表条目 `include_str!("prompts/<role>.md")` 字面路径装配；无 `<role>` 动态读取 / 运行时文件 IO | —（见不可测试项 1；phase_table 结构收敛的半边由 phase_table_test 正向承载） |
| AC-4 | `PhaseAgentSpec` 静态化：仅 `{ prompt: &'static str, model_level }`；全仓 desktop 源码零残留 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>`；相位序与 model_level 对照不变（6 executor + 8 evaluator 齐备，evaluator-only 相位 executor=None） | `crates/core/workflow/src/write/phase_table_test.rs`（结构收敛 + 相位序 / 档位不变）；全仓零残留 grep → 见不可测试项 2 |
| AC-5 | 上下文头进 prompt：相位响应下发的 executor / evaluator prompt 首部含一行 `change: <name>`（change 名取 db 记录 name）；回溯原因行在 backtrack 场景照常 append | `crates/core/workflow/src/write/phase_next_test.rs`（组装面在场 + 无占位）＋ `crates/core/orchestration/src/walker_test.rs`（消费面直出上下文头） |
| AC-6 | evaluator 协议瘦身：协议要求 `{verdict, report, checklist}`，MUST NOT 要求 phase / attempt 回声；`EvaluatorChecklist` 解析接受该形状，phase / attempt / skipped 由 walker 盖戳 | `crates/core/orchestration/src/prompt_test.rs`（协议形状）＋ `crates/core/orchestration/src/verdict_test.rs`（瘦身形状解析回环 + 多余字段静默忽略） |
| AC-7 | 桌面端特性适配：每个 md 只引用桌面七工具与 cwd=workspace root；无插件 Agent / MCP 假设、无 phase_log 等落账路由指令；静态脚本外置声明 | —（见不可测试项 1） |
| AC-8 | 相位机语义零漂移：等价相位序列下路由结果、重试上限、白名单下发、attempt 计时、eval 落账、stale 传播与改造前一致；全管线全绿 | `crates/core/workflow/src/write/phase_next_test.rs` ＋ `crates/core/orchestration/src/walker_test.rs`（既有回归持衡）；全管线全绿 → 见不可测试项 3 |
| AC-9 | 插件废弃注记与版本交付：根 `AGENT.md` 顶部 DEPRECATED 注记；`plugins/dev-team` 版本与三类交付产物零改动；`packages/desktop` version 0.4.32 | —（见不可测试项 4） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test]，共置 *_test.rs）。门面 / 纯类型 /
     md 内容文件 / 生成物不建独立测试文件，统一落「不可测试项」。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- 修改既有测试文件节。删 interpolate 三用例与 __CALL_AGENT 断言（删除面零残留
     的自动化半边）；新增 PhaseAgentSpec 结构收敛 + prompt 静态非空锚（AC-4 半边）；
     相位序 / 角色齐备 / 模型档位 / 白名单 / 重试上限 / 非 requirement 拒绝全部持衡
     （AC-8 相位机语义零漂移的相位表半边）。 -->

#### 待测功能

- `PhaseAgentSpec { prompt: &'static str, model_level: ModelLevel }` — 去 `agent_type`，prompt 编译期静态（`include_str!("../prompts/<role>.md")` 产物）；`pub`，phase_next 组装 + 同 crate 测试消费
- `PhaseDefinition { id, description, executor, evaluator }` — `executor` / `evaluator` 为 `Option<PhaseAgentSpec>`；code-review / acceptance 为 evaluator-only（`executor: None`）
- `phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>` — V1 仅 `requirement` 返回 `Some`；内部 `static REQUIREMENT_TABLE: &[PhaseDefinition]`（去 `OnceLock` / `requirement_table()` / `spec` 闭包，const 化）
- `allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String>` — 表序前置集（含自身）；表外 / 首相位 → 空集（不变）
- `MAX_RETRY_TIMES: u32 = 5` — 不变（重试上限常量锚）
- 删除 `interpolate(template: &str, change: &str, phase: Option<&str>) -> String` 与 `proposal_explore_handoff()`（零残留）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_table_test · PhaseAgentSpec 结构收敛 | 正向 | 以穷尽解构 `let PhaseAgentSpec { prompt, model_level } = spec;` 绑定（任何 `agent_type` 引用即编译失败——结构收敛编译锚）；`prompt` 类型为 `&str` 且非空；对全表 14 条目（6 executor + 8 evaluator）逐条断言 prompt 非空 | 新增 |
| phase_table_test · 相位序与角色齐备对照 | 重写 | `phase_table("requirement")` 返回 8 相位（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance）逐项与表序一致；前 6 相位 executor / evaluator 双 `PhaseAgentSpec` 齐备且 prompt 非空；code-review / acceptance executor 为 None、evaluator 在场；**删除**原 `__CALL_AGENT:<role>__` 前缀 / 后缀断言 | 重写 |
| phase_table_test · 模型档位分派对照不变 | 适配 | 8 相位 `(executor_level, evaluator_level)` 对照（implement / test-gen / test-execution executor 为 Low、其余 High；test-execution evaluator 为 Low、其余 High；code-review / acceptance executor None）逐条相等；Clone 后 `model_level` 保真（解构改为 `PhaseAgentSpec { prompt: _, model_level }`） | 适配 |
| phase_table_test · 非 requirement 拒绝返回 None | 持衡 | `bug-fix` / `test-only` / `refactor` / `""` 四态 `phase_table(...)` 返回 None | 持衡 |
| phase_table_test · 重试上限常量锚定为 5 | 持衡 | `MAX_RETRY_TIMES == 5` | 持衡 |
| phase_table_test · 白名单首相位为空集 | 持衡 | `allowed_backtrack_phases(table, "proposal")` 返回空 | 持衡 |
| phase_table_test · 白名单返回表序前置集含自身 | 持衡 | `allowed_backtrack_phases(table, "test-gen")` = proposal/dev-design/test-design/implement/test-gen；`"acceptance"` = 全 8 相位 | 持衡 |
| phase_table_test · 白名单未知相位返回空 vec | 持衡 | 表外相位 / `""` → 空 Vec，不崩不臆测 | 持衡 |
| phase_table_test · interpolate 三用例 | — | 删除：`interpolate双占位符全量替换且其余字节保真` / `interpolate_phase缺席时占位符保留而change照常替换` / `interpolate无占位符原样返回且未知占位符不误替换`（随 `interpolate` 函数退役） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯静态数据断言（相位表驻 `'static`，无 IO / 无时钟 / 无依赖注入） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

<!-- 修改既有测试文件节。插值断言改上下文头 `change: <name>` 在场（无 `<change>` /
     `<phase>` / CHANGE_ID）；回溯原因后缀断言保留；其余路由语义（pass 推进 / 重试
     上限 / 全 pass 终态 / 锚点 / 只读 / 故障传播）全部持衡（AC-8 相位机语义零漂移
     的路由半边）。 -->

#### 待测功能

- `phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` — 只读路由状态机（不改状态库；路由 / 重试上限 / 白名单 / attempt 计时 / stale 传播语义不变）
- `ResolvedPhaseSpec { prompt: String, model_level: ModelLevel }` — 新增（`Debug, Clone, PartialEq, Eq`）；静态主体 + 上下文头 + 回溯原因已组装
- `PhaseNextOutcome { done, next_phase, round, executor: Option<ResolvedPhaseSpec>, evaluator: Option<ResolvedPhaseSpec>, allowed_backtrack_phases, last_result, error }` — `executor` / `evaluator` 类型 `Option<PhaseAgentSpec>` → `Option<ResolvedPhaseSpec>`
- `build_phase_response`（私有）— `change: <name>\n\n` 头部 + 静态主体 +（backtrack 时）`\n\n⚠️ 回溯原因: <reason>` 尾部；`name` 取 `record.name`（id → 记录 → name 分辨率单点），MUST NOT 模板插值
- `SessionAnchors` / `LastResult` / `PhaseNextError` — 不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_next_test · 初始路由上下文头在场 | 重写 | 建档零相位行 → 首相位 proposal / round=1；executor / evaluator prompt 首部含 `change: demo-change`；`!prompt.contains("<change>")`、`!prompt.contains("<phase>")`、`!prompt.contains(CHANGE_ID)`（id 串零入 prompt——上下文头单点 = record.name）；白名单为空 | 重写 |
| phase_next_test · fail 重试上下文头在场 | 适配 | 窗口内 fail 重试（round 递增）→ executor prompt 含 `change: demo-change`、零 `<change>` / `<phase>` / CHANGE_ID；`last_result` 携最新 fail 条目；删除原「`<phase>` → dev-design 插值」断言（相位 id 不再经模板注入） | 适配 |
| phase_next_test · backtrack 回溯原因后缀保留 | 适配 | 最新条目 `backtrack_to` 在场 → 路由至目标相位；executor prompt 含 `⚠️ 回溯原因: <reason>` 且含上下文头 `change: demo-change`；白名单 = 目标相位表序前置集 | 适配 |
| phase_next_test · backtrack 与 stale 同行并存仍路由 | 适配 | `backtrack_to` + `stale=true` 同位 → 目标相位重开；回溯原因后缀 + 上下文头在场（stale 不遮蔽回跳路由） | 适配 |
| phase_next_test · pass 推进 / 重试上限 / 全 pass 终态 | 持衡 | 既有路由语义用例零改动：pass 推进白名单随行；窗口 fail 恰达 MAX_RETRY_TIMES 返 `MaxRetriesExceeded`；全 pass → done | 持衡 |
| phase_next_test · 锚点 / 只读 / 故障 / 未建档 / run_id 空白 | 持衡 | 既有边界用例零改动：锚点首见基线 / run_id 隔离 / 重启续走；只读零写面触达；StoreFault 传播 Err；未建档 id 显式 Err；run_id 空白 Err | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ChangeStateStore`（注入依赖入参） | 既有 `RouteStore` 进程内假件：只实现读半边（`get_change` / `list_phase_records`），写半边 `unimplemented!`（phase_next 越权触写即 panic——只读性结构性执法）；可注入 `StoreFault` | 本节全部用例 |
| `SessionAnchors` | 独立实例（`SessionAnchors::new()`），复合键 `(change_id, run_id)` 真实哈希表 | 锚点相关用例 |
| 时间戳 | 确定性 `i64` unix millis 常量（`t(n)` 基线 + n 秒），零 wall-clock 等值比较 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs

<!-- 修改既有测试文件节。删 executor_prompt 三段组装 / 角色要点 / 兜底用例（随
     role_brief / strip_call_agent / executor_prompt 退役）；evaluator_prompt 协议
     断言改瘦身形状（无 phase / attempt / skipped 回声要求，含 verdict / report /
     checklist 与 MCP 红线）；decision_prompt 五段 / 空集三态持衡；prompt 确定性去
     executor 恒等断言（AC-6 协议半边）。 -->

#### 待测功能

- 删除 `executor_prompt(agent_type: &str, phase_prompt: &str) -> String`（角色前导退役——角色自述溶解进静态 md）
- 删除 `strip_call_agent(agent_type: &str) -> &str`（`__CALL_AGENT` 令牌解析退役）
- 删除 `role_brief(role: &str) -> &'static str`（15 行角色要点静态表退役）
- `evaluator_prompt(phase_prompt: &str) -> String` — 签名不变，仅追加瘦身协议附录
- `evaluator_protocol`（私有）— 瘦身：禁调 MCP 写通道红线（phase_log / phase_next / phase_start / backtrack）+ `{verdict, report, checklist}` 形状（去 phase / attempt / skipped 回声要求；checklist 至少一项、pass 布尔、evidence 事实依据、report ≤2000）
- `decision_prompt(input: &DecisionInput) -> String` — 不变（fail checklist + 白名单 + 候选 report + 四动作封闭集）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| prompt_test · evaluator 协议附录瘦身形状 | 重写 | `evaluator_prompt(phase_prompt)` 含 `## 输出协议`；含禁调 MCP 红线（`禁止调用` + `phase_log` / `phase_next` / `phase_start` / `backtrack`）；含 `"verdict"` / `"report"` / `"checklist"` 三键与 `2000` 上限；**不含** `"phase"` / `"attempt"` / `"skipped"`（带引号的 JSON 键——回声要求已删）；`phase_prompt` 主体逐字节保真（仅追加协议附录） | 重写 |
| prompt_test · decision_prompt 五段在场 | 持衡 | fail 相位信封 + fail checklist 行 + 白名单行 + 候选 eval report + 四动作封闭集（backtrack / retry / stop / ask）+ reason ≤500 + 越权红线 | 持衡 |
| prompt_test · decision_prompt 空集三态组装不崩 | 持衡 | fail_checklist / 白名单 / candidates 三空态；候选 verdict / report 双 None 降级文案稳定 | 持衡 |
| prompt_test · prompt 确定性 | 重写 | `evaluator_prompt` 与 `decision_prompt` 同输入重复组装逐字节一致（无时钟 / 随机参与）；**删除** `executor_prompt` 恒等断言（函数已退役） | 重写 |
| prompt_test · executor_prompt 三段组装 / 角色要点 / 兜底用例 | — | 删除：`executor_prompt三段组装角色要点与prompt主体`（含 `__CALL_AGENT` 剥离 / 未收录角色兜底 / 裸 agent_type 三态）——随 `executor_prompt` / `role_brief` / `strip_call_agent` 退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯字符串组装函数（`evaluator_prompt` / `decision_prompt`），输入 fixture 内存构造 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs

<!-- 修改既有测试文件节。EvaluatorChecklist 去 phase / attempt / skipped 三字段：
     合法 fixture 与逐字段断言改瘦身形状；attempt 缺席 / skipped 三形态用例删除或
     改走「多余字段静默忽略」面（serde 无 deny_unknown_fields，walker 权威）；report
     长度门 / 值域封闭 / 结构漂移 / 围栏提取持衡（AC-6 解析半边）。 -->

#### 待测功能

- `EvaluatorChecklist { verdict: Verdict, report: String, checklist: Vec<ChecklistItem> }` — 去 `phase` / `attempt` / `skipped`（walker 按 provenance 盖戳）
- `parse_verdict(text: &str) -> Result<EvaluatorChecklist, String>` — JSON 载体提取 → 封闭结构校验 → report 长度门；逻辑不变
- `MAX_REPORT_CHARS: usize = 2000` — 不变（report 长度门）
- `extract_json` / `fenced_block` / `brace_slice`（私有）— 不变（裸 JSON / 围栏 / 兜底切片三态）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| verdict_test · 合法裸 JSON 解析瘦身形状 | 重写 | 合法 fixture `{ verdict, report, checklist }`（无 phase / attempt / skipped）→ `parse_verdict` 成功；`checklist.verdict == Pass`、`checklist.report` 逐字、`checklist.checklist` 落 `ChecklistItem` 域类型（item / pass / evidence）；**删除** phase / attempt / skipped 逐字段断言 | 重写 |
| verdict_test · 围栏 / 混杂叙述 / 裸 JSON 兜底 | 适配 | ```` ```json ```` 围栏 / 前后混杂叙述 / 无围栏首尾夹叙述三态提取成功（fixture 换瘦身形状） | 适配 |
| verdict_test · verdict 值域封闭 | 持衡 | pass / fail 两值可辨；值域外 `"warning"` → Err（含 `结构漂移` / `verdict` 记因） | 持衡 |
| verdict_test · report 超长拒绝 | 适配 | report > 2000 → Err（含 `超长` + `2000`）；恰 2000 → Ok（fixture 换瘦身形状，去 phase / attempt / skipped 键） | 适配 |
| verdict_test · checklist 行解析 | 重写 | 空数组 / 多行 item-pass-evidence 三行解析；**删除** attempt 缺席断言（attempt 字段已删）与 skipped 断言 | 重写 |
| verdict_test · 多余字段静默忽略 | 新增 | 输入 JSON 仍携带 `phase` / `attempt` / `skipped` 多余字段 → `parse_verdict` 仍成功且结构仅 verdict / report / checklist（serde 无 `deny_unknown_fields`，evaluator 回声多余字段静默忽略——walker 权威，AC-6 scenario「合法 checklist JSON（无 phase / attempt / skipped）」的容忍面） | 新增 |
| verdict_test · 结构漂移显式失败 | 适配 | 缺 verdict / checklist 行缺 evidence / 非 JSON 文本 / 纯叙述无 JSON / 空文本 / 空白消息 → 均 Err（fixture 换瘦身形状，去 phase / attempt / skipped 键） | 适配 |
| verdict_test · skipped 三形态解析 | — | 删除：`skipped三形态解析`（true / false / 缺席）——`skipped` 字段随 `EvaluatorChecklist` 瘦身退役，改由「多余字段静默忽略」用例承接 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 最终消息以字符串 fixture 内存构造（合法 / 围栏 / 漂移 / 超长 / 多余字段五族），零进程边界依赖 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

<!-- 修改既有测试文件节。route_with_round 装置 PhaseAgentSpec → ResolvedPhaseSpec
     （去 agent_type）；新增上下文头消费面断言（executor 直出静态主体含 `change: <name>`
     头、零 __CALL_AGENT 残留）；其余 walker 回归（反馈边 / 决策 / test-execution /
     步状态流 / 升格路径）全部持衡（AC-5 消费面 + AC-8 编排半边）。 -->

#### 待测功能

- walker 消费面：executor 分支 `executor.prompt.clone()`（直出 `ResolvedPhaseSpec.prompt` 静态主体，不再调 `executor_prompt(&executor.agent_type, …)`）；evaluator 分支 `evaluator_prompt(&evaluator.prompt)` 不变；不再消费 `agent_type`
- 导入面：`use crate::prompt::{decision_prompt, evaluator_prompt, executor_prompt}` → `{decision_prompt, evaluator_prompt}`
- `route_with_round`（测试装置）— `executor` / `evaluator` 构造 `workflow::write::ResolvedPhaseSpec { prompt: String, model_level }`（去 `agent_type`；prompt 已含 `change: <name>` 上下文头）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| walker_test · route_with_round 装置适配 | 适配 | `route_with_round` 构造 `ResolvedPhaseSpec`（无 `agent_type` 字段，编译期结构锚）；executor / evaluator `model_level` 档位保真 | 适配 |
| walker_test · 上下文头消费面在场 | 新增 | 以 `route_with_round` 驱动 walker 起 executor / evaluator 会话 → 捕获 `WorkerTurnRequest.prompt` 含 `change: c` 上下文头；零 `__CALL_AGENT` 残留；evaluator prompt 尾随 `## 输出协议`（`evaluator_prompt` 追加面） | 新增 |
| walker_test · 反馈边 / 决策 / 升格路径 / 步状态流 | 持衡 | 既有 walker 回归用例零改动：static-check 反馈边续注 / 修复计数递进；决策 prompt 三列表与用户应答；test-execution 升格零 evaluator 会话；步状态流上图 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `WorkerAgentPort` | 既有假 worker 引擎（预录 `WorkerTurnOutcome` + `WorkerTurnRequest` 全字段捕获——prompt / role / model_level 断言面） | 上下文头消费面 / 反馈边 / 决策 / 升格路径用例 |
| `ToolStepPort` | 既有脚本化假件（static-check / test-execution 队列可编程结论） | 反馈边 / test-execution 用例 |
| `ChangeStateStore` | 既有假写面 / 假引擎装置（store 真实或脚本化按用例） | 本节全部用例 |
| tokio 运行时 | `#[tokio::test]`（async walker 直驱） | 本节全部用例 |

---

## 不可测试项

<!-- 门面 / 纯类型 / md 内容文件 / 版本元数据 / 全管线门禁不建独立测试文件，
     统一在此声明行为去向。 -->

1. **AC-1 / AC-2 / AC-3 / AC-7：14 md 清单与内容约定、零占位、显式 `include_str!`、桌面特性适配** — **原因**: md 是纯内容文件（非 `.rs`，无运行时行为），「全静态零占位」「只引桌面七工具」「静态脚本外置声明」等为内容约定，D6 用户拍板「不建通配符守门测试」（静态化是内容约定，不建断言防线）。`include_str!` 显式路径装配是编译期字面量事实（无引用的 md 可静态识别为废弃），非可运行行为。**行为去向**：AC-1 ~ AC-4 由 test-execution 阶段以 grep / read 人工核验承载；AC-4 的「PhaseAgentSpec 仅 prompt + model_level」半边由 phase_table_test 结构收敛锚正向承接（可自动化半边），「全仓零残留 grep」半边归人工核验。

2. **AC-4：全仓 desktop 源码零残留 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>`** — **原因**: 否定性静态约束（「某符号 / 令牌不得存在」）无法通过正向运行时测试直接断言；全仓 grep 是内容面核销，非单条用例可表达。**行为去向**：删除面清单（interpolate / proposal_explore_handoff / role_brief / strip_call_agent / executor_prompt / __CALL_AGENT 令牌）逐一由编译（删除后引用即编译失败）+ test-execution 阶段 grep 核销；phase_table_test / prompt_test 删除面用例的「废弃」标记即半边的可追踪留痕。

3. **AC-8：`vp test` / `client:check` / knip 全绿** — **原因**: 全量测试套件与静态检查门禁属于 test-execution 阶段流程验证，非本文件单条用例可表达；相位机语义零漂移的可自动化半边由 phase_next_test / walker_test 既有回归用例持衡承载。

4. **AC-9：根 `AGENT.md` 顶部 DEPRECATED 注记与 `packages/desktop/package.json` version 0.4.32** — **原因**: 文档注记与版本号是发布 / 清单元数据，无对应单元测试文件；通过代码审查确认三要素注记（源码停更 / 已安装副本照常可用 / 未来删除）与版本号递增；`plugins/dev-team` 零 diff 由 test-execution 阶段 `git diff` 核验（版本 2.10.44 + 三类交付产物）。

5. **门面 / 纯类型 / 装配模块不建独立测试文件** — `crates/core/workflow/src/write/mod.rs` 导出面（`interpolate` 去导出、`ResolvedPhaseSpec` 增导出）、`crates/core/orchestration/src/verdict.rs` 的 `ChecklistItem` 域类型、`prompts/*.md` 14 文件、`crates/core/workflow/src/prompts/` 目录本体 — **原因**: re-export / 纯类型 / 纯文本无自有行为；**行为去向**：导出面经 phase_table_test（`PhaseAgentSpec` 消费）与 phase_next_test / walker_test（`ResolvedPhaseSpec` 消费）承载；md 内容经 AC-1 ~ AC-4 / AC-7 人工核验承载。
