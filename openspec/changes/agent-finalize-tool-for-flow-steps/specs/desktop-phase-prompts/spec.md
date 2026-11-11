# desktop-phase-prompts Specification (Delta)

## MODIFIED Requirements

### Requirement: 桌面端特性适配（迁移是改写非搬运）

每个 md SHALL 适配桌面执行环境：只引用桌面七工具（read / grep / glob / ls / write / edit / bash）、**注入矩阵命中的 archi 只读工具**（`archi_module` / `archi_search`，矩阵单源见 `desktop-archi-agent`「archi 工具面与 (phase, role) 注入矩阵」）与 cwd=workspace root；MUST NOT 携带插件 Agent / MCP 工具假设（`__MCP:spec_list__` / `__MCP:phase_log__` / `__MCP:workflow_files__` / `__MCP:test_detect_frameworks__` 等）；SHALL 明确**静态脚本由桌面端执行、agent 内不重复执行**（static-check / test-execution 等机械步骤已外置，agent 只做知识性工作，省 token 为显式约束）。原文的「迁移」已完成且源侧文件已随插件退役删除——本条自此为**防回退护栏**：新增或改写 md MUST NOT 反向携带插件时代的工具 / 令牌假设。

**archi 工具引用纪律**：仅矩阵命中该工具的角色 md SHALL 提及它（`proposal` 仅提及 `archi_search`；`dev-design` 两者；`test-design` 仅 `archi_search`；`implement` / `code-review` / `acceptance` 仅 `archi_module`）；未命中角色（`initialize` 相关、`test-gen`、`test-execution`、`code-analyze`、`archive` SpecSync）MUST NOT 提及任何 archi 工具——否则等于在 prompt 里许诺一个未下发的工具面（模型会尝试调用并被成员校验拒绝）。archi 工具的提及 SHALL 只给「何时用（定位 / 发现）」与「返回什么（邻接与文档引用、`modelFile` + `modelLine` 可直接 `edit`）」两类信息：MUST NOT 插值具体路径模板、MUST NOT 教模型拼 `.dev-team/architecture/...` 字面路径（路径知识归工具返回值）、MUST NOT 指示模型写模型文件（写面走既有 `write` / `edit` 与 `dev-design` 的常规纪律，模型 = 源码见 `desktop-archi-agent`）。

**收口工具引用纪律**：`finish_step` 只为 evaluator / decision 角色下发，故 8 份 evaluator md SHALL 提及它（收口一句话，见「evaluator 知识不含落账 / 路由指令」）；6 份 executor md MUST NOT 提及 `finish_step`（未下发工具面的许诺与 archi 工具同款纪律）。

#### Scenario: 只引桌面七工具与矩阵命中的 archi 工具

- **WHEN** 审查 14 个 md 的工具引用
- **THEN** 通用工具仅出现 read / grep / glob / ls / write / edit / bash 七项，无任何 `__MCP:*` 调用
- **AND** archi 工具只出现在矩阵命中该工具的角色 md 内（`proposal` → 仅 `archi_search`；`dev-design` → 两者；`test-design` → 仅 `archi_search`；`implement` / `code-review` / `acceptance` → 仅 `archi_module`），其余 md 零 archi 工具字样
- **AND** `finish_step` 只出现在 8 份 evaluator md（6 份 executor md 零字样）
- **AND** 无插件侧 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` / `__MODEL_*__` 片段注入标记

#### Scenario: archi 工具只给用法不给路径与写指令

- **WHEN** 读取命中角色的 md 内 archi 工具表述
- **THEN** 只含「何时用（定位 / 发现）」与「返回什么（邻接、文档引用、`modelFile` + `modelLine` 可直接 `edit`）」；不含具体路径模板、不含教模型拼架构域目录字面路径的措辞、不含「用某专用工具写模型」的指令

#### Scenario: 静态脚本外置声明

- **WHEN** 读取 executor / evaluator md 中对 static-check / test-execution 等机械步骤的表述
- **THEN** 含「由桌面端执行、agent 内不重复执行」类说明，agent 不承担重复运行测试执行门禁或静态检查脚本的职责

### Requirement: evaluator 知识不含落账 / 路由指令

8 个 evaluator md MUST NOT 含 "Call phase_log" / `phase_next` / `phase_start` / `backtrack` 等 MCP 落账或路由指令（桌面 evaluator 禁 MCP，落账由 walker 代写）；evaluator SHALL 只产出评估结论，且结论 SHALL 经 **`finish_step` 工具调用**提交（形状与落账红线由工具 parameters 与桌面端追加的输出协议给出，见 desktop-change-orchestration「evaluator 输出协议瘦身」），MUST NOT 要求最终消息输出 checklist JSON、MUST NOT 指示用 Write/Edit/Bash 写 `eval.json` / `workflow.json`。8 份 md 的同一句输出面表述 SHALL 逐字一致（「调用 `finish_step` 收口」），MUST NOT 出现「最终消息输出 checklist JSON」的旧措辞。

#### Scenario: evaluator md 零落账指令

- **WHEN** 读取 8 个 evaluator md
- **THEN** 无 phase_log / phase_next / phase_start / backtrack 调用指令
- **AND** 无「写入 eval.json / workflow.json」指令；输出面表述为「调用 `finish_step` 收口」

#### Scenario: 八份同一句零漂移

- **WHEN** 核对 8 份 `*-evaluator.md` 的输出面那一行
- **THEN** 8 份逐字一致（同一句改写）；旧措辞「最终消息输出一个 checklist JSON」在全 `prompts/*.md` 中 `grep` 零命中

## ADDED Requirements

### Requirement: 静态清单表为 ID 契约面

8 个 evaluator md 的 `## 静态清单` 段 SHALL 是 checklist 的 **ID 契约面**：表首列为检查项 ID（形如 `[A-Z]+[0-9]+`），第二列为该检查项的权威人读文案（落账 `ChecklistItem.item` 的唯一来源），第三列为判断依据。

- **段落边界**：扫描范围 SHALL 为 `## 静态清单` 标题起、下一个 `## ` 标题止；段内 `###` 子表的子项行 MUST NOT 被当成本段清单项。
- **ID 形态**：表首列匹配 `^\| ([A-Z]+[0-9]+) \|`（两侧空格约束）才收；带连字符的子项形态（如 `T8-U1` / `T8-G1`）MUST NOT 出现在主清单表首列——它会被无边界解析切出假 ID（`T8` / `U1` / `G1`），使校验集合与模型眼前清单错位。
- **去重与序**：ID 按表内出现序去重（同一 ID 只出现一次）；正文重复引用不产生重复项。
- **项数**：8 份的清单项数 SHALL 为 `acceptance 5` / `code-analyze 8` / `code-review 7` / `dev-design 10` / `implementation 7` / `proposal 11` / `test-design 9` / `test-gen 8`（合计 65）；项数与表结构由机械守卫断言，改坏表结构即刻红。
- **集合自洽**：每份 md 的「全文边界 ID 集合」SHALL 等于「`## 静态清单` 段内 ID 集合」（无多无少）——防止子表 ID 混进主清单。
- **消费者**：`checklist_entries(prompt)` 自 prompt 文本解析出 `(id, 检查项正文)` 二元组，同时驱动 `finish_step` 的 `id` enum 与校验集合、以及落账 `item` 的映射（见 desktop-workflow-write-face「verdict 推导单点」与 desktop-agent-execution「finish_step 工具协议与本步收口」）。md MUST NOT 为解析器额外提供结构标记（表格即契约）。

#### Scenario: 段落边界挡住子表

- **WHEN** 对 `test-design-evaluator.md`（含 `### T8 格式检查详细项` 与 `T8-U1` / `T8-G1` 子表行）求 `checklist_entries`
- **THEN** 得到 `T1..T9` 恰 9 项；`T8` / `U1` / `G1` 等由子表行产生的假 ID 零出现

#### Scenario: 逐文件项数守卫

- **WHEN** 对 8 份 evaluator prompt 逐份断言清单项数
- **THEN** 依次为 5 / 8 / 7 / 10 / 7 / 11 / 9 / 8（合计 65）；任一份表结构被改坏（缺列、改首列形态、段内混入子表）即断言失败

#### Scenario: 全文集合等于段落集合

- **WHEN** 分别以「仅 `## 静态清单` 段」与「全文」为扫描范围求边界 ID 集合
- **THEN** 8 份 md 的两集合逐份相等（无多无少）；不相等即失败并指出差异 ID

#### Scenario: 第二列为权威文案

- **WHEN** 解析 `code-review-evaluator.md` 的 `C1` 行
- **THEN** 返回的「检查项正文」为该表第二列文本（如「变更代码中无安全漏洞」），逐字等于 md 原文，MUST NOT 由模型自拟

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/*-evaluator.md`（8 份） | 输出面表述 | 同一句改为「调用 `finish_step` 收口」（逐字一致，旧措辞零命中）；`## 静态清单` 表结构（3 列 / 首列 ID / 子表不含连字符子项）逐字保持 |
| `crates/core/workflow/src/prompts/*-planner.md` / `*-generator.md`（6 份） | 零改动 | executor 角色知识与工具引用纪律逐字不变（收口协议只作用于 evaluator / decision） |
| `crates/core/workflow/src/write/phase_table.rs` | 装配零改动 | 14 条 `include_str!` 字面路径与角色表逐字不变；工具面由挂载清单承载，prompt 不含工具清单真源 |
| `crates/core/workflow/src/model/checklist.rs`（新） | 解析器 | `checklist_entries(prompt) -> Vec<ChecklistEntry>`（三规则：段落边界 / 表首列 ID 形态 / 出现序去重，同时取第二列正文）；`ChecklistEntry { id, item }` |
| `crates/core/workflow/src/write/phase_table_test.rs` | 语料守卫 | 8 份项数逐文件断言 + 全文/段落集合相等（prompt 文本自相位表取得，零文件 IO）；两处 `SessionToolset` 注释文案改写（断言本体零改） |
