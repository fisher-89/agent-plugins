# desktop-test-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: 结论反馈边与独立预算

结论 fail / error 时 walker SHALL 走定向反馈边（对齐 static-check Upgraded 语义）：将 findings 诊断**明细**注入 executor 会话（Continue）修复——明细为有界装配文本（诊断面 findings 全量、缺省回落 problems 消息面 + suite 执行概览（框架 / root / 退出码 / 计数 / 覆盖三维度对照、null 维度跳过）+ 失败用例明细（名称 / 文件行号 / 错误类型 / 消息 / 堆栈）），修复会话输入面自足、MUST NOT 依赖修复会话自读报告文件；报告目录随 prompt 兜底携带（全量权威所在）。修复重入后复用门自动失效重跑。反馈边 SHALL 独立计数、上限 5 次（与 static-check 反馈计数器分立，各自计满各自升格），MUST NOT 计入相位 retry 预算；超限 walker SHALL 以代写 fail checklist 经写面 `phase_log` 升格相位 fail（不跑 evaluator，进入重试 / 决策路径）。

#### Scenario: 反馈修复重入

- **WHEN** conclusion=fail 且反馈边未超限
- **THEN** findings 明细文本（诊断 + suite 概览 + 失败用例名 / 文件行号 / 错误消息）注入 executor 会话（Continue），反馈边计数 +1；修复重入后复用门失效并重跑

#### Scenario: 明细有界装配

- **WHEN** 失败面超大（长消息 / 长堆栈 / 大量失败用例）
- **THEN** 明细按界截断（单条消息 500 / 堆栈 800 字符、每 suite 20 条、总预算 12000 字符），截断面以省略号与余量记数示意，报告目录兜底行恒在场

#### Scenario: 超限升格相位 fail

- **WHEN** 同一相位内反馈边达 5 次仍 fail / error
- **THEN** walker 代写 fail checklist 落账升格相位 fail，不再注入反馈边；相位 retry 预算独立消耗（反馈边次数不计入）

### Requirement: 步骤可观测与最小载荷

`ChangeStepKind::TestExecution` SHALL 入步词汇封闭集，门禁步状态经 `RunUpdate::Step` 上图（前端流程视图现有步骤渲染直接吃，零渲染改动）；IPC 类型随 bindings 再生成出线。`ToolStepOutput::TestExecution` 载荷面 SHALL 为 conclusion + 计数 + 诊断摘要 + 修复边明细文本 + 报告路径：摘要面服务步状态 detail（`diagnose_brief` 口径截断），明细面服务反馈边修复 prompt 直嵌（有界装配、pass 态恒空串）。载荷 SHALL 零 Serialize 不进 IPC——明细文本 MUST NOT 经 bindings / `RunUpdate` 出线，只在进程内 walker 反馈边消费；全量 findings 留报告文件，MUST NOT 经 IPC 面全量透传。

#### Scenario: 步骤上图零渲染改动

- **WHEN** 门禁步状态变化且 bindings 再生成
- **THEN** `RunUpdate::Step` 携 testExecution 步词汇流出，前端流程视图以既有步骤渲染直接消费

#### Scenario: 载荷双面各司其职

- **WHEN** 审查 `ToolStepOutput::TestExecution` 载荷
- **THEN** 无结构化 findings 全量数组字段：摘要面（`findings_brief`）走步 detail，明细面（`findings_detail`，有界文本）走反馈边修复 prompt；两者均不进 bindings / IPC，全量以报告文件为权威（`report_dir` 定位）
