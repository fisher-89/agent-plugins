# 提案: desktop-prompt-centralize

> **变更**: desktop-prompt-centralize
> **日期**: 2026-10-10
> **状态**: draft
> **探索**: `openspec/changes/desktop-prompt-centralize/explore.md`（决策 D1–D8 用户拍板在案）

---

## 问题

桌面端 change run 的相位 prompt 今天只有「一句话路由句」，真正的角色知识（阶段要求 + 静态 checklist）仍寄居在插件 `plugins/dev-team/agents/*.md` 里。三连问排查结论（2026-10-10 会话）：

1. **checklist 预设在插件评估 agent 的 prompt 里**：`*-evaluator.md` 的 `## Static Checklist` 静态表由 Agent 工具按 subagent_type 载入，运行时无任何注入路径。桌面相位表只含 agent_type + 路由句，skill 原样透传，phase_log 只做 all-pass→pass 裁决。
2. **桌面侧 checklist 为零**：`evaluator_prompt` = 路由句 + 输出协议，协议只规定形状不规定条目；检查项内容靠被拉起的会话里恰好装了 dev-team 插件。桌面自举评估（walker 代写落账、evaluator 禁 MCP）的检查项是悬空的。
3. **plugins 整体停止迭代、标记废弃、未来全删**：桌面必须拥有自己的角色知识，把插件端 agent 描述迁移到桌面是必然动作。

explore 同时把「动态面」完整盘点到仅五处：① `<change>`/`<phase>` 插值（唯一模板替换，`phase_next.rs:236`）；② 回溯原因行（walker 组装一行）；③ decision 三列表（walker 有界组装）；④ role_brief 静态表；⑤ 输出协议（静态）。因此**不需要「词汇表 + 规则 + 通用替换引擎」那么重的通配符系统**——唯一需要保留的底线是 **change 名必须进 prompt 文本**（agent 在 workspace root 起播，静态块引用 `openspec/changes/<name>/...` 产物路径需要知道去哪个 change 目录），方案是 walker 组装一行上下文头 `change: <name>`，而非通配符系统。

结构化通道已存在：`WorkerTurnRequest` 已携带 `root`（会话 cwd）、`provenance.source_ref = <change>/<phase>/<role>/<attempt>`、`role`、`model_level`；store 已先行一步——`AgentRunRecordV3.prompt` 只剩升级链解码用途，转录走 `SessionEventRecord` 事件流，本重构对 store schema 零改动。

---

## 提案

把角色知识（阶段要求 + checklist）从插件 agent 描述整体迁移为桌面 `core/workflow/src/prompts/` 下的 14 个**全静态** md 文件，相位表以编译期 `include_str!` 显式字面路径装配；动态面全部改为 walker 侧 append（非模板替换），删除通配符与 `__CALL_AGENT` 令牌机制。

```
core/workflow/src/prompts/           ← 角色知识集中地（14 个 md）
├─ 6 个 executor md（proposal/dev-design/test-design/implement/test-gen/test-execution）
├─ 8 个 evaluator md（同上 + code-review/acceptance，含 checklist 表）
└─ 每文件 = 角色自述 + 阶段要求 + checklist，全静态零占位
        ▲ phase_table.rs 表条目 include_str!("prompts/xxx.md") 显式字面路径
        │  → 无 <role> 间接读取；无引用的 md = 可识别的废弃文件
core/workflow/src/write/phase_table.rs
  PhaseAgentSpec { prompt: &'static str, model_level }  ← agent_type 字段随 __CALL_AGENT 退役
        ▼
walker 组装面（唯一动态，全部 append 而非模板）：
  ├─ 上下文头一行："change: <name>"
  ├─ 回溯原因行（现状保留）
  ├─ static-check 反馈 / 测试修复失败注入（现状保留）
  ├─ 输出协议附录（walker 追加一处；协议瘦身为 {verdict, report, checklist}，
  │   phase/attempt 由 walker 盖戳，agent 不必回声）
  └─ decision_prompt 三列表（现状保留）
```

**删除面**：`interpolate` + `<change>`/`<phase>`、`__CALL_AGENT` 令牌 + `strip_call_agent`、role_brief 15 行表（溶解进各 md）、evaluator 路由句里遗留的 "Call phase_log" 指令（桌面 evaluator 明令禁 MCP，插件镜像的错指令，迁移时直接不搬）、phase_table 与插件 `lib/workflow.ts` 的 AC-9 对照口径（镜像关系随插件废弃终结）。

**迁移的实质是改写，不只是搬运**：插件 agent 文本里有 Agent/MCP 工具假设和「change 名在 prompt 里」的措辞；桌面侧七工具（read/grep/glob/ls/write/edit/bash）、cwd=workspace root、change 名走上下文头，每个 md 都要适配。另在 md 中明确：**静态脚本（测试执行、static-check 等机械步骤）由桌面端执行，agent 内不重复执行**（省 token 是本次迁移的显式约束）。

**用户拍板要点**：接受 phase_table 编译期静态风格，但去掉 `__CALL_AGENT` 依赖；独立 md 文件、显式指定全部依赖（不按 `<role>` 动态读取）；不建「无通配符」守门测试（静态化是内容约定）；evaluator-only 相位（code-review / acceptance）`executor: None` 不变，md 总数 = 14；插件端仅标记废弃、不改功能代码、不破坏已安装副本，本 change 仍可走已安装插件的 openspec 流程。

---

## 能力

### 新增能力

- **desktop-phase-prompts** — 桌面相位角色知识集中地：`core/workflow/src/prompts/` 14 个全静态 md（6 executor + 8 evaluator）的清单、内容结构（角色自述 + 阶段要求 + checklist）、零占位符不变量、`include_str!` 显式字面路径（无 `<role>` 间接读取）、桌面端特性适配（七工具 / cwd=workspace root / change 名走上下文头 / 静态脚本外置）。

### 修改的能力

- **desktop-workflow-write-face** — 相位表与 prompt 模板单源收敛升级：`PhaseAgentSpec` 去 `agent_type`（`__CALL_AGENT` 令牌退役）、prompt 静态化为 `&'static str`、`interpolate` 与 `<change>`/`<phase>` 占位符删除；相位序 / model_level / 白名单 / 前置依赖 / 相位机操作语义不变；phase_table 与插件 `lib/workflow.ts` 的 AC-9 镜像对照口径终结。
- **desktop-change-orchestration** — prompt 组装面静态化：`role_brief` 15 行表与 `strip_call_agent` 退役（角色自述溶解进静态 md）；executor / evaluator prompt 组装改为静态主体 + 动态 append（上下文头 `change: <name>`、回溯原因行、static-check / 测试修复反馈注入、输出协议附录、decision 三列表）；evaluator 输出协议瘦身为 `{verdict, report, checklist}`（phase / attempt / skipped 由 walker 盖戳，agent 不回声）。

### 引用沿用（零 delta）

- **desktop-checks-domain** — static-check / test-execution 机械门禁外置不变；md 文案仅新增「静态脚本由桌面端执行、agent 内不重复执行」的说明，检查域语义零改动。
- **desktop-agent-execution** — `WorkerTurnRequest` 的 root / provenance / role / model_level 结构化通道零改动；phase / attempt 盖戳与 cwd=workspace root 语义不变。
- **pge-workflow-engine / phase-agents / phase-skills** — 插件端相位引擎与 agent 描述冻结，不再作为桌面路径依赖；本 change 不改其功能代码。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/prompts/`（新目录）— 14 个全静态角色知识 md（6 executor + 8 evaluator），内容 = 插件 agent 描述大体搬迁 + 桌面端特性改写
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — `PhaseAgentSpec` 去 `agent_type`、prompt 静态化为 `&'static str`；表条目改 `include_str!("prompts/xxx.md")` 显式字面路径；删除 `interpolate` 与 `<change>`/`<phase>` 注释口径
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` — 删除 `interpolate` 调用；`build_phase_response` 改为静态 prompt + 上下文头 `change: <name>` + 回溯原因行（append 而非模板替换）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` — 导出面随 `PhaseAgentSpec` 形态调整
- `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` — 删除 `role_brief` / `strip_call_agent`；`executor_prompt` 不再前置角色要点（签名随 agent_type 退役简化）；`evaluator_prompt` 只追加输出协议附录；`evaluator_protocol` 瘦身为 `{verdict, report, checklist}`
- `packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs` — `EvaluatorChecklist` 去 `phase` / `attempt` / `skipped` 字段（phase / attempt / skipped 由 walker 盖戳）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `executor_prompt` / `evaluator_prompt` 调用点适配（不再传 agent_type）
- `AGENT.md`（仓库根，插件项目指引）— 顶部加 DEPRECATED 注记：插件端停止迭代、未来删除；不改任何插件功能代码、不破坏已安装副本既有功能
- `packages/desktop/package.json` — `version` 0.4.31 → 0.4.32

### 测试文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs` — 删除 `interpolate` 测试；改为断言：`PhaseAgentSpec` 无 `agent_type` 字段、prompt 为静态非空文本、相位序与 model_level 对照不变
- `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs` — 删除 `role_brief` / `strip_call_agent` 相关断言；改为断言 executor prompt = 静态主体直出、evaluator 协议附录含瘦身 checklist JSON 形状（无 phase / attempt 回声要求）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` — `PhaseAgentSpec` 构造去掉 `agent_type`；新增上下文头 `change: <name>` 在场断言
- `packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs`（测试伴随）— `EvaluatorChecklist` 瘦身形状解析回环

> 按用户拍板 D6：静态化（零占位 / 显式 `include_str!` 路径 / 全仓令牌零残留）是内容约定，**不建通配符守门测试**；对应验收由 AC-1 ~ AC-4 以 grep / read 人工核验承载，不落自动化断言。

### 删除文件

- 无（删除的是函数 / 字段 / 令牌机制，非整文件；插件功能代码不删）。

### 不要修改

- `plugins/dev-team` 全部功能代码、MCP 工具、hooks、CLI 工作流、版本 2.10.44 与三类交付产物（仅根 `AGENT.md` 加废弃注记，不触源码）
- `crates/core/workflow` 相位机操作语义（phase_next / phase_start / phase_log / backtrack / archive 双写——路由、重试上限、白名单、attempt 计时、eval 落账、stale 传播）
- `crates/infra/store` schema / `SessionEventRecord` 转录（store 零改动，不触 golden wire contract）
- `crates/infra/checks`（static-check / test-execution 确定性门禁实现）
- `crates/infra/agent` 的 `WorkerTurnRequest` / provenance / cwd 通道
- 插件 `lib/workflow.ts`（冻结，不再作为桌面对照口径）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 角色知识集中地 | `core/workflow/src/prompts/` 在案且含 14 个 md（6 executor + 8 evaluator）；文件名与角色一一对应；code-review / acceptance 无 executor 文件；每文件含角色自述 + 阶段要求 + checklist（evaluator 含静态 checklist 表） |
| AC-2 | 全静态零占位 | 14 个 md 全文无 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:*` / `__INCLUDE:*` / `{{...}}` 等运行时占位符或令牌；change 名引用不依赖「change 名在 prompt 里」措辞 |
| AC-3 | 显式 include_str! | `phase_table.rs` 每表条目以 `include_str!("prompts/<role>.md")` 显式字面路径装配；无 `<role>` 动态读取、无运行时文件 IO；无引用的 md 可被识别为废弃文件 |
| AC-4 | PhaseAgentSpec 静态化 | `PhaseAgentSpec` 仅含 `prompt: &'static str` 与 `model_level`；全仓 desktop 源码无 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>` 残留；相位序与 model_level 对照不变（6 executor + 8 evaluator 齐备，evaluator-only 相位 executor 为 None） |
| AC-5 | 上下文头进 prompt | 相位响应下发的 executor / evaluator prompt 首部（或经组装）含一行 `change: <name>`（change 名取自 db 记录 name，非模板插值）；回溯原因行在 backtrack 场景照常 append |
| AC-6 | evaluator 协议瘦身 | evaluator 输出协议要求最终消息 checklist JSON 形状为 `{verdict, report, checklist}`，MUST NOT 要求 phase / attempt 回声；`EvaluatorChecklist` 解析接受该形状，phase / attempt / skipped 由 walker 盖戳落账 |
| AC-7 | 桌面端特性适配 | 每个 md 只引用桌面七工具（read/grep/glob/ls/write/edit/bash）与 cwd=workspace root；无插件 Agent/MCP 工具假设、无 "Call phase_log" / phase_next / phase_start / backtrack 落账路由指令（evaluator 禁 MCP）；每个 md 明确「静态脚本由桌面端执行、agent 内不重复执行」 |
| AC-8 | 相位机语义零漂移 | 以既有假写面 / 假引擎驱动等价相位序列，路由结果、重试上限、白名单下发、attempt 计时、eval 落账、stale 传播与改造前一致；`vp test` / `client:check` / knip 全绿 |
| AC-9 | 插件废弃注记与版本交付 | 根 `AGENT.md` 顶部含 DEPRECATED 注记；`plugins/dev-team` 版本与三类交付产物零改动；`packages/desktop` version 0.4.32 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 迁移文本丢语义：插件 agent 描述的 checklist / 阶段要求改写时遗漏或扭曲 | 桌面评估缺检查项或标准漂移，产出质量下降 | 中 | 迁移 = 大体搬迁 + 桌面特性改写，逐文件对照插件源；evaluator 静态 checklist 条目逐条平移；AC-1 / AC-7 守门 |
| R2 全静态文本超长 / 冗余导致 token 上升（与省 token 目标冲突） | 会话成本上升 | 中 | md 只保留角色知识性内容，机械步骤（static-check / test-execution）外置声明不重复执行；测试执行等长流程不复制脚本细节 |
| R3 上下文头 change 名缺失导致 agent 找不到 change 目录 | executor / evaluator 读产物失败 | 低 | AC-5 上下文头断言；change 名单源自 db 记录 name（既有 phase_next 分辨率），无模板插值路径可丢 |
| R4 evaluator 协议瘦身与既有 verdict 解析不兼容 | 解析失败停给用户、无法落账 | 低 | `EvaluatorChecklist` 同步去字段 + 解析回环测试；phase / attempt / skipped 由 walker 盖戳单点覆盖 |
| R5 桌面源码残留通配符 / 令牌形成半迁移态 | 行为不可预期、后续维护混乱 | 低 | AC-4 全仓 grep 断言零残留；删除面清单化逐一核销 |
| R6 插件废弃注记与已安装副本预期冲突 | 用户误以为插件不可用 | 低 | 注记明确「源码停更、已安装副本照常可用、未来删除」；不改任何插件功能代码 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 桌面自立与插件去向 | 桌面完全自立；插件停止迭代、`AGENT.md` 标注废弃、未来删除全部代码 | D1 用户拍板：角色知识随插件废弃必然迁移，桌面自持 | 桌面继续依赖插件 agent 描述（与废弃方向矛盾） |
| 角色知识落位 | `core/workflow` 相位表编译期静态风格，但去掉 `__CALL_AGENT` 依赖 | D2 拍板：角色知识 = 流程相位 executor / evaluator 的知识，随相位表驻 core/workflow | 独立运行时加载层（重，无收益） |
| md 组织 | 独立 md 文件，显式 `include_str!` 指定所有依赖（不按 `<role>` 动态读取） | D3 拍板：无引用的 md 可识别为废弃文件 | 目录遍历 / `<role>` 间接读取（无法识别废弃） |
| prompt 差异留存 | 失败重试 / 修复未通过用例等场景的 prompt 差异不落 store，由 agent history（转录）保留 | D4 拍板：与 store「prompt 不落库」现状对齐 | 落库 prompt（触 store schema，零改动红线破） |
| 插件废弃语义 | 已安装副本照常可用；「废弃」指源码停更、标记废弃、未来删除；本 change 仍走已安装插件 openspec 流程 | D5 拍板：不破坏既有副本功能 | 直接删除插件（破坏已安装副本） |
| 通配符守门 | 不建「无通配符」守门测试——静态化是内容约定，不建断言防线 | D6 拍板：内容约定无需机械断言 | 建全仓通配符 grep 守门测试（过度防御） |
| evaluator-only 相位 | code-review / acceptance `executor: None` 不变，md 总数 = 6 + 8 = 14 | D7 拍板：与既有相位表一致 | 为 evaluator-only 相位虚设 executor md |
| md 内容 | 插件 agent 描述大体搬迁 + 桌面特性修改：去通配符、去 `__CALL_AGENT` / phase_log 指令、明确静态脚本桌面端执行不重复 | D8 拍板：省 token 是显式约束 | 逐字搬运插件文本（携带工具假设与错误指令） |

### 待决问题

- 14 个 md 的逐文件正文定稿与 checklist 条目核对清单（随实现留痕，可与插件源逐条 diff）。
- 上下文头 `change: <name>` 的精确落点（`phase_next` 响应组装面 vs walker 组装面）与回溯原因行的拼接次序（design 定稿）。
- `executor_prompt` / `evaluator_prompt` 简化后的函数签名形态（是否保留独立组装函数，还是 walker 直接 append 输出协议附录）（design 定稿）。
- 插件 `AGENT.md` 废弃注记的确切文案与位置（顶部注记 vs 架构段前注记）（design 定稿）。
