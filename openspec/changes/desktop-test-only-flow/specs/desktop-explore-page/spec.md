# desktop-explore-page — desktop-test-only-flow delta

## MODIFIED Requirements

### Requirement: 会话链与 stance 前导

每个 explore SHALL 对应一条会话链：打开详情页 SHALL 默认续最近链（按 `source="explore"` + 该 explore 的 `source_ref` 定位链头，沿 `parent_run_id` 回溯还原全部历史 run 并重放落库事件），无链则下次发送起链（首条 run 创建链）。MVP MUST NOT 提供「新会话」动作。发送时 SHALL 将 desktop 内置的精简 explore stance 前导模板拼接进 prompt 头部（模板归属 desktop 包，注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链）；stance 前导 SHALL 注入当前记录 `name`（agent 落盘文件名 MUST 等于 `record.name`，MUST NOT 自拟）并明示「笔记首行写 `# <标题>`」与「推算变更类型并写约定标记行」两条约定（标记行与首行标题同构，具体格式由 design 定稿）。续话 SHALL 以链尾 run 的 `session_id` 作为 `resume_session_id` 发起。双恢复 SHALL 为：文档从磁盘读 + 对话重放该链落库事件，两者互不依赖对方存在。

#### Scenario: 重开续链双恢复

- **WHEN** 用户完成两轮对话后离开并重新打开该 explore
- **THEN** 预览从磁盘恢复当前 explore.md，对话区按序重放两条 run 的全部落库事件；再次发送时以链尾 `session_id` 续话而非新会话

#### Scenario: stance 拼接注入 name

- **WHEN** 查看任一 explore 发起 run 的 prompt 原文
- **THEN** 内容以 stance 前导模板开头、含当前记录 name（落盘文件名 MUST 等于 name）与「首行写 `# 标题`」约定、用户输入随后；调试页发起的 run prompt 不含该模板

#### Scenario: stance 注入类型推算约定

- **WHEN** 查看任一 explore 发起 run 的 prompt 原文
- **THEN** stance 前导含「推算变更类型并写约定标记行」约定与判断标准（补测试 / 生成测试用例 → test-only，否则 requirement）；调试页发起的 run prompt 不含该约定

### Requirement: promote 启动变更入口

探索详情页 SHALL 在记录 `promoted_to` 为空时提供「启动变更」入口，并在入口旁提供工作流类型选择器（普通 select，内联实现，MUST NOT 抽共享组件）：默认值 = `record.workflow_type`（推算值），可改 `requirement` / `test-only`。点击后 SHALL 调 `promote_explore(root, name, workflow_type)`，成功后刷新清单并停留在详情页（promoted 态随即呈现「已转变更」+ 跳转，MUST NOT 自动导航离开）。`promoted_to` 非空时 SHALL 不呈现「启动变更」入口。后端错误 SHALL 行内呈现（break-all 错误块），用户可改后重试。测试挂钩 SHALL 使用 data-testid，MUST NOT 以样式类名作查询挂钩。

#### Scenario: promote 成功流转

- **WHEN** 用户在草稿态 explore 详情以默认（推算）类型点击「启动变更」且 promote 成功
- **THEN** 清单刷新、记录 `promoted_to` 打标，详情呈 promoted 态（「已转变更」+ 跳转），URL 不离开 `/explores/<name>`；铸出 change 的 `workflow_type` = 用户选择值

#### Scenario: 改选类型后 promote

- **WHEN** 用户把类型选择器从推算值改为 `test-only` 后点击「启动变更」
- **THEN** `promote_explore(root, name, "test-only")` 被调用，铸出 change 的 `workflow_type = "test-only"`

#### Scenario: promote 失败行内呈现

- **WHEN** promote 因笔记未落盘 / 非 kebab name / change 冲突等原因失败
- **THEN** 详情页行内呈现后端错误，记录与笔记零改动，用户可改后重试

## ADDED Requirements

### Requirement: workflow_type 回填与类型推算解析

详情页 SHALL 在 watch 重读笔记内容后解析约定标记行（纯函数 `extractWorkflowType`，返回 `"requirement"` / `"test-only"` / `null`）：解析结果合法且与 `record.workflow_type` 不同时 SHALL 调显式写命令 `update_explore_workflow_type(root, name, workflow_type)` 回填 store `workflow_type`，成功后刷新清单。回填 SHALL 仅经显式写命令完成，MUST NOT 在读取路径（`read_explore`）写入或修改任何文件；解析失败（无约定标记行 / 非法值）SHALL 保持 `record.workflow_type` 不变，不报错。类型选择器的人工改选 SHALL 同样经 `update_explore_workflow_type` 回填落库（沿 title 改即写模式）。

#### Scenario: 标记解析回填

- **WHEN** 笔记含约定标记行 `test-only` 而 `record.workflow_type` 为 `"requirement"`，watch 重读后
- **THEN** 前端调 `update_explore_workflow_type` 回填 `"test-only"`，成功后清单刷新、promote 选择器默认值随动；`read_explore` 路径零写入

#### Scenario: 无标记或非法值不报错

- **WHEN** 笔记内容无约定标记行（或标记值非法）
- **THEN** `record.workflow_type` 保持原值，无 invoke 调用、无错误呈现

#### Scenario: 人工改选落库

- **WHEN** 用户在 promote 入口把类型从 `"requirement"` 改为 `"test-only"`
- **THEN** 前端调 `update_explore_workflow_type` 回填 `"test-only"`（改即写），下次进详情仍是改后值

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/lib/explore-stance.ts` | stance 前导模板 | 增「推算变更类型并写约定标记行」约定 + 判断标准；与 SKILL.md 双源注释互链 |
| `packages/desktop/src/lib/explore-title.ts`（或旁置新文件） | 类型标记解析 | `extractWorkflowType(content)` 纯函数：合法值 / `null` |
| `packages/desktop/src/views/explores/explore-detail-view.tsx` | 类型回填 + promote 类型选择器 | `useWorkflowTypeBackfill`（沿 `useTitleBackfill`）；promote 入口内联 select（默认 `record.workflow_type`，可改）；`promote(name, workflowType)` |
| `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | 清单动作面 | `promote(name, workflowType)` + `updateWorkflowType(name, workflowType)` 签名随动 |
