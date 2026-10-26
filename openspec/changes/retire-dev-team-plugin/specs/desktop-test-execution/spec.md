# desktop-test-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: test-execution 确定性门禁步与绿跑零 agent

Walker SHALL 在 test-execution 相位收敛点执行确定性 test-execution 门禁步（`ToolCommand::TestExecution`，经 `TestExecutionRunner` port 由 checks 边界 infra 实现；相位序由 `phase_table` 的 `TEST_EXECUTION_STEPS` 静态定义）。结论 pass 时相位 SHALL 以机械 eval checklist 代写 `phase_log`（verdict=pass）推进，全程 MUST NOT 发起任何 WorkerAgent 会话——「executor agent 驱动 CLI 子命令跑测试」的旧路径随插件退役删除，本条为**防回退护栏**（MUST NOT 因参照物消失而放宽为可选）；结论 fail / error 时 SHALL 走「失败即升格交决策（零 agent 修复）」requirement 的升格路径——首败即落账交决策，MUST NOT 在相位内发起修复会话。runner 以 `Err` 收场（程序不可达 / 配置不支持 / 写盘失败）SHALL 停给用户（显式失败，与 WorkerAgent 失败同等呈现）。

#### Scenario: 零会话面不因参照物退役而放宽

- **WHEN** 审查 test-execution 收敛点的会话发起面
- **THEN** 全档结论（pass / fail / error / `Err` 收场）均零 WorkerAgent 会话，相位序取自 `TEST_EXECUTION_STEPS` 静态表
- **AND** 无「对齐插件工作流相位语义」类外部参照断言或注释（参照物已删除）

### Requirement: 双实现对拍黄金语料

报告 schema 与聚合判定 SHALL 以**已固化 Rust 实现 + corpus 金样**为权威：`core/checks` 的解析与聚合实现为唯一权威源，`core/checks/tests/golden`（及 `tests/fixtures` 的 summary.json / report.json 真实样本）为冻结金样；原对齐源（CLI zod schema）已随插件退役删除，MUST NOT 再作为对齐目标或断言依据。金样对拍 SHALL 拦住实现漂移——金样变更必须经显式重写流程（与 `core/workflow` corpus golden 同款纪律），MUST NOT 以 `--force` / 全量重生成方式静默吞掉差异。

#### Scenario: corpus 对拍拦截漂移

- **WHEN** 以真实 fixtures 驱动 `core/checks` 解析与聚合
- **THEN** 字段 / 计数 / conclusion 与金样逐项一致；实现语义漂移被测试显式指认（差异须以显式金样重写 + spec delta 提出）

#### Scenario: 权威源单点

- **WHEN** 审查 corpus 测试的权威断言来源
- **THEN** 权威只指向 `core/checks` 实现与 `tests/golden` 金样；无「与 CLI 实现一致 / 以 zod schema 为权威」类残留断言（参照物已删除）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/checks`（解析与聚合） | 报告语义权威 | 唯一权威源；金样（`tests/golden` + `tests/fixtures`）为冻结比对面 |
| `crates/core/checks/tests/corpus_golden_test.rs` | 金样对拍载体 | 头注释与断言语改为「已固化 Rust 实现 + corpus 金样」权威口径；行为零改 |
| `crates/core/checks/tests/fixtures/README.md` | 语料说明 | 去「CLI zod schema 形态」溯源措辞（语料本身原样保留，含旧路径样本） |
| 插件 `bin/src/schemas/test-execution-output.schema.ts` | **（删除）** | 随插件退役；MUST NOT 再作为报告 schema 的对齐源 |
