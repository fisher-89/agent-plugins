# desktop 相位 prompt 集中管理探索(阶段要求 + checklist 单一真源,去通配符)

日期:2026-10-10 · 状态:探索完成,待开 change

## 动机

三连问的排查结论(2026-10-10 会话):

1. **checklist 预设在插件评估 agent 的 prompt 里**(`plugins/dev-team/agents/*-evaluator.md` 的 `## Static Checklist` 静态表,Agent 工具按 subagent_type 载入)。运行时无任何注入路径:相位表只含 agent_type + 路由句,skill 原样透传,phase_log 只做 all-pass→pass 裁决。
2. **desktop 侧 checklist 为零**。`evaluator_prompt` = 路由句 + 输出协议,协议只规定形状不规定条目;检查项内容靠被拉起的会话里恰好装了 dev-team 插件。桌面自举评估(walker 代写落账、evaluator 禁 MCP)的检查项是悬空的。
3. **plugins 整体停止迭代、标记废弃、未来全删** → 桌面必须拥有自己的角色知识,迁移插件端 agent 描述是必然动作。

## 现状盘点

### prompt 文本的四个来源

| 来源 | 内容 | 去向 |
|---|---|---|
| 插件 `agents/*.md` | 阶段要求全文 + checklist 静态表 | 桌面侧零拷贝(缺口) |
| `core/workflow/src/write/phase_table.rs` | agent_type(`__CALL_AGENT:<role>__`) + 一句路由 prompt | 插件 `lib/workflow.ts` 一比一镜像(AC-9 对照口径) |
| `core/orchestration/src/prompt.rs` | role_brief 15 角色一句话要点、evaluator 输出协议、decision_prompt 三列表组装 | 会话起播 |
| `phase_next.rs:236` | `<change>` / `<phase>` 两个硬编码 string replace | 全桌面唯一"通配符" |

### 动态面完整盘点(仅五处)

| # | 变量 | 性质 |
|---|---|---|
| 1 | `<change>` / `<phase>` 插值 | 唯一模板替换 |
| 2 | 回溯原因行 `⚠️ 回溯原因: {reason}` | walker 组装一行 |
| 3 | decision 三列表(失败清单/白名单/候选报告) | walker 有界组装,非模板 |
| 4 | role_brief 静态表 | 静态 |
| 5 | 输出协议(含 evaluator 回显 phase/attempt 的要求) | 静态 |

### 结构化通道已存在(prompt 不是唯一通道)

`WorkerTurnRequest`(`core/orchestration/src/port.rs:42-59`)已携带 `root`(会话 cwd)、`provenance.source_ref = <change>/<phase>/<role>/<attempt>`、`role`、`model_level`。插件侧把 change/phase 塞进 prompt 是 Agent 工具只有 prompt+agent_type 两个通道的遗产;桌面 walker 自己知道自己在哪个 change/phase/attempt 拉会话、落账时自己盖戳。

### store 已先行一步

`infra/store/src/model.rs` 注释:v4 会话化迁移时 `prompt / cwd / env / permission_mode / source / source_ref / parent_run_id` **已随版本退役**(`AgentRunRecordV3.prompt` 只剩升级链解码用途)。转录走 `SessionEventRecord` 事件流——"prompt 不落 store、由 agent history 保留"与 store 现状对齐,**本重构对 store schema 零改动**,不触 golden wire contract。

## 结论:不需要通配符系统

动态面五处中,1 是唯一模板替换,2/3 是有界数据组装,4/5 是静态。没有任何场景需要"词汇表 + 规则 + 通用替换引擎"那么重的机制(core/agent 定义规则、infra/agent 执行替换的分层随之不成立,core/agent 继续零 prompt 概念)。

但保留一条底线:**change 名必须进 prompt 文本**。agent 在 workspace root 起播,静态块里引用 `openspec/changes/<name>/...` 产物路径,需要知道去哪个 change 目录。方案:walker 组装一行上下文头(`change: <name>`),非通配符系统。

## 收敛方案

```
core/workflow/src/prompts/           ← 角色知识集中地 (14 个 md)
├─ 6 个 executor md (proposal/dev-design/test-design/implement/test-gen/test-execution)
├─ 8 个 evaluator md (同上 + code-review/acceptance, 含 checklist 表)
└─ 每文件 = 角色自述 + 阶段要求 + checklist, 全静态零占位
        ▲ phase_table.rs 表条目 include_str!("prompts/xxx.md") 显式字面路径
        │  → 无 <role> 间接读取; 无引用的 md = 可识别的废弃文件
core/workflow/src/write/phase_table.rs
  PhaseAgentSpec { prompt: &'static str, model_level }  ← agent_type 字段随 CALL_AGENT 退役
        ▼
walker 组装面 (唯一动态, 全部 append 而非模板):
  ├─ 上下文头一行: "change: <name>"
  ├─ 回溯原因行 (现状保留)
  ├─ static-check 反馈 / 测试修复失败注入 (现状保留 — 重试/修复场景的差异所在)
  ├─ 输出协议附录 (保留 walker 追加一处: 它是 walker↔落账的对偶契约,
  │   不复制进 8 个 evaluator md; 协议瘦身为 {verdict, report, checklist},
  │   phase/attempt 由 walker 盖戳, agent 不必回声)
  └─ decision_prompt 三列表 (现状保留)
```

**删除面**:`interpolate` + `<change>`/`<phase>`、`__CALL_AGENT` 令牌 + `strip_call_agent`、role_brief 15 行表(溶解进各 md)、evaluator 路由句里遗留的 "Call phase_log" 指令(桌面 evaluator 明令禁 MCP,插件镜像的错指令,迁移时直接不搬)、phase_table 与插件 `lib/workflow.ts` 的 AC-9 对照口径(镜像关系随插件废弃终结)。

**迁移的实质是改写,不只是搬运**:插件 agent 文本里有 Agent/MCP 工具假设和"change 名在 prompt 里"的措辞;桌面侧七工具(read/grep/glob/ls/write/edit/bash)、cwd=workspace root、change 名走上下文头,每个 md 都要适配。另需在 md 中明确:**静态脚本(测试执行、static-check 等机械步骤)由桌面端执行,agent 内不重复执行**——省 token 也是本次迁移的显式约束(用户拍板 #8)。

## 决策记录(用户拍板)

1. 桌面端完全自立;插件端停止迭代、AGENT.md 标注废弃、未来删除全部代码。
2. 接受 phase_table 风格(编译期静态),但去掉 `__CALL_AGENT` 依赖;角色知识=流程中某相位 executor/evaluator 的知识,就放在 `core/workflow`。
3. 独立 md 文件,显式指定所有依赖的 md(不按 `<role>` 动态读取),便于识别废弃文件。
4. prompt 在失败重试、修复未通过用例等场景仍有差异,但 store/model 不记录,由 agent history(转录)保留。
5. 已安装的插件依然可用,"废弃"指源码层面下线——repo 源码停更、标记废弃、未来删除,不破坏已安装副本的既有功能。本 change 仍可走已安装插件的 openspec 流程。
6. **不需要"无通配符"守门测试**——静态化是内容约定,不建断言防线。
7. evaluator-only 相位:code-review / acceptance 表里 `executor: None` 依旧,md 总数 = 6 executor + 8 evaluator = 14。
8. md 内容 = 插件 agent 描述**大体搬迁 + 按桌面端特性修改**:去掉废弃通配符(`<change>`/`<phase>`、`__CALL_AGENT`、phase_log 指令),且明确说明**静态脚本由桌面端执行,不要在 agent 内执行**(省 token)——test-execution 门禁、static-check 等机械步骤已外置,agent 只做知识性工作。