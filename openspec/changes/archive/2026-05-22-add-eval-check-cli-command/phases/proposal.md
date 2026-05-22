# 提案: add-eval-check-cli-command

> **变更**: add-eval-check-cli-command
> **日期**: 2026-05-21
> **状态**: 提案

---

## 问题

当前 PGE 流程中的阶段技能（phase-requirements、phase-test-design、phase-dev-proposal 等）在启动工作时，没有系统化的前置条件验证机制。每个技能通过临时性的 ad-hoc 检查（如"设计文档是否存在"）来判定是否可以开始，但这种做法存在以下缺陷：

1. **无标准化门控**：每个技能用自己的方式检查前置条件，检查逻辑重复且不统一。
2. **无法保证执行顺序**：技能可能在前置阶段未通过评估的情况下启动，导致流程混乱。
3. **无回溯检测**：如果前置阶段存在活跃的 backtrack marker（如 evaluation 要求回溯到 01-requirements），当前技能无法感知，可能在不正确的状态下继续工作。
4. **重复评估**：技能无法判断当前阶段是首次运行、重试还是已通过，无法做出正确的行为决策（如首次运行需生成内容，已通过应跳过）。

现有基础设施：
- `eval-log` CLI 命令已存在，用于向 eval.json 追加评估条目
- `eval-json.ts` 已提供 `checkGate()` 和 `readEvalJson()` 等核心函数
- `workflow.ts` 已提供 `getPriorPhases()` 和阶段顺序定义
- `eval-check.py` 存在但仅用于归档阶段（检查所有 7 个阶段）
- 缺少一个在执行阶段前检查"到我为止的所有前置阶段是否已通过"的 CLI 工具

---

## 解决方案

### 方案 A：在 TypeScript CLI 中新增 eval-check 子命令（推荐）

在 `bin/src/commands/eval-check.ts` 中实现 `eval-check` 子命令，复用 `eval-json.ts` 和 `workflow.ts` 中的现有逻辑。

**核心功能**：
1. 前置阶段门控 — 每个前置阶段必须在 eval.json 中至少有一条 verdict 为 "pass" 的记录
2. 顺序性验证 — 前置阶段的 pass 记录 timestamp 必须单调递增（ts(01) < ts(02) < ts(03)），证明它们按顺序执行
3. 当前阶段状态 — 报告当前阶段是首次运行、重试还是已通过
4. 回溯标记检测 — 任何前置阶段的最新评估记录中如果包含活跃的 backtrack_to 值，则阻断执行

**退出码**：0 = 通过（可以继续），1 = 阻断（被阻塞）

**选项**：`--change <name>`（必填）、`--phase <phase>`（必填）、`--json`（程序化输出）

**集成方式**：每个阶段技能在前置验证步骤中调用 `dev-team eval-check --change <name> --phase <current-phase> --json`。

### 方案 B：新增 standalone Python 脚本

编写独立的 Python 脚本来完成相同功能。

**优点**：
- 与现有 `eval-check.py` 风格一致
- Python 对字符串处理和文件操作更简洁

**缺点**：
- 无法复用 TypeScript lib 中的 `checkGate()`、`getPriorPhases()` 等现有实现
- 需要在 Python 中维护一份阶段顺序定义，与 TypeScript 产生重复
- 与 `eval-log`（TypeScript CLI 命令）风格不一致
- 整体代码库碎片化：部分 CLI 命令用 TypeScript，部分用 Python

### 方案 C：将验证逻辑嵌入每个阶段技能

在每个阶段技能的 prompt 中，通过 shell 命令或脚本内联实现前置检查。

**优点**：
- 无需新增 CLI 命令
- 单个技能完全自包含

**缺点**：
- 检查逻辑在 7 个技能中重复 7 次
- 没有统一的 --json 输出格式，技能难以程序化解析
- 技能 prompt 膨胀，增加维护成本
- 无法独立测试门控逻辑

### 方案对比

| 维度 | 方案 A（TS CLI） | 方案 B（Python） | 方案 C（内联） |
|------|-----------------|-----------------|--------------|
| 代码复用 | 完全复用现有 lib | 需重写逻辑 | 不适用 |
| 一致性 | 与 eval-log 一致 | 与 eval-check.py 一致但碎片化 | 无标准 |
| 可测试性 | 高（jest 单元测试） | 中（pytest） | 低 |
| 维护成本 | 低 | 中 | 高 |
| 实现工作量 | 小（~100 行） | 中（~200 行） | 大（7 次） |

**推荐方案 A**：TypeScript CLI 子命令。与现有 `eval-log` 共享同一代码库，复用已存在的 core 函数，最小化新增代码量。

---

## 变更范围

### 实现以下特性

- 新增 `bin/src/commands/eval-check.ts` 文件，实现 eval-check 子命令
- 在 `bin/src/index.ts` 中注册 eval-check 子命令（通过 `registerEvalCheckCommand(cli)`）
- 支持 `--change <name>`、`--phase <phase>`、`--json` 参数
- 前置阶段门控检查：每个前置阶段需要至少一条 pass 记录
- 顺序性检查：前置阶段 pass 记录 timestamp 单调递增
- 当前阶段状态报告：首次运行 / 重试 / 已通过
- 回溯标记检测：前置阶段的最新 eval 条目中 backtrack_to 不为 null 时阻断
- --json 标志输出结构化结果供技能解析
- 退出码 0（通过）/ 1（阻断）

### 不要修改

- 不要修改现有的 `eval-json.ts`、`workflow.ts`、`change.ts` lib 文件
- 不要修改 `eval-log.ts` 命令
- 不要修改 `eval-check.py` Python 脚本（专用于归档验证，不同用途）
- 不要修改阶段技能本身的 prompt 或逻辑（门控调用在技能之外处理）
- 不要修改 eval.json 的 schema 或格式
- 不要引入新的 npm 依赖

---

## 能力

### 新增能力

- `eval-check-cli` — 新增 `dev-team eval-check` CLI 子命令，在执行阶段技能前验证前置阶段是否全部通过、按正确顺序执行、无活跃 backtrack 标记，并报告当前阶段状态（首次/重试/已通过）

### 修改的能力

- `embedded-cli` — 在 TypeScript CLI 入口 `bin/src/index.ts` 中注册新的 eval-check 子命令，包括参数定义和命令路由

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | eval-check 在前置阶段全部有 pass 记录时返回退出码 0 | 构造 eval.json，包含 01-requirements 和 02-test-design 的 pass 记录；运行 `dev-team eval-check --change test --phase 03-dev-proposal`，验证退出码为 0 | P0 |
| AC-2 | eval-check 在缺少前置阶段 pass 记录时返回退出码 1 并列出缺失阶段 | 构造 eval.json，缺失 01-requirements 的 pass 记录；运行命令，验证退出码为 1，输出中包含 "01-requirements" | P0 |
| AC-3 | eval-check 在 pass timestamp 顺序错误时返回退出码 1 | 构造 eval.json，02-test-design 的 pass timestamp 早于 01-requirements 的 pass timestamp；运行命令，验证退出码为 1 | P0 |
| AC-4 | eval-check 在 --json 模式下输出合法 JSON | 运行 `dev-team eval-check --change test --phase 03-dev-proposal --json`，验证 stdout 可被 `JSON.parse()` 解析 | P0 |
| AC-5 | eval-check --json 输出包含 passed、phase、prior_phases、block_reasons、phase_state 字段 | 解析 JSON 输出，验证所有必需字段存在且类型正确 | P0 |
| AC-6 | eval-check 在前置阶段存在活跃 backtrack marker 时返回退出码 1 | 构造 eval.json，01-requirements 的最新条目包含 `backtrack_to: "01-requirements"`；运行命令，验证退出码为 1，输出包含 "backtrack" | P0 |
| AC-7 | eval-check 报告当前阶段为"首次运行"（当前阶段无 eval 条目） | 构造 eval.json 无当前阶段的任何条目；运行命令，验证 phase_state 为 "first_run" | P1 |
| AC-8 | eval-check 报告当前阶段为"重试"（当前阶段的最新条目 verdict 为 fail） | 构造 eval.json 包含当前阶段的 fail 条目；运行命令，验证 phase_state 为 "retry" | P1 |
| AC-9 | eval-check 报告当前阶段为"已通过"（当前阶段的最新条目 verdict 为 pass） | 构造 eval.json 包含当前阶段的 pass 条目；运行命令，验证 phase_state 为 "passed" | P1 |
| AC-10 | 对首个阶段（01-requirements）执行 eval-check，仅检查当前阶段状态，不检查前置阶段 | 运行 `dev-team eval-check --change test --phase 01-requirements`，验证不检查任何前置阶段（输出中 prior_phases 为空） | P1 |
| AC-11 | 不存在的 change 名称导致有意义的错误信息 | 运行 `dev-team eval-check --change non-existent --phase 01-requirements`，验证输出包含有意义的错误信息，退出码为 1 | P2 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 并发 eval 条目时间戳精确到毫秒，但计算机时钟可能产生相同时间戳导致顺序判断歧义 | 顺序检查可能误判同一毫秒内的条目顺序 | 低 | 时间戳对比使用 >= 而非 >，对完全相同的时间戳视为顺序正确；在文档中记录此约定 |
| 对已有变更（已有 eval.json 但 pass 记录 timestamp 不单调递增）执行 eval-check 会阻断 | 已有工作流的变更无法继续 | 中 | eval-check 的文档/输出明确指明 timestamp 问题；提示用户可手动调整 eval.json 中异常的 timestamp 值以修复顺序 |
| 技能调用 eval-check 时误传 phase 名称（如非规范格式） | 门控检查跳过或错误阻断 | 低 | eval-check 验证 phase 名称是否在合法阶段列表中；非法阶段名输出明确错误并退出码 1 |
| eval-check 依赖 eval.json 文件结构，若未来 schema 升级导致不兼容 | 命令失效或误判 | 低 | eval-check 读取时同时检查 schema_version 字段，不匹配时输出警告；schema 升级时同步更新 eval-check |
