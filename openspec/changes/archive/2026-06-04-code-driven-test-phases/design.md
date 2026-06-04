# 设计: code-driven-test-phases

> **变更**: code-driven-test-phases
> **日期**: 2026-06-03
> **基于**: proposal.md, specs/phase-agents/spec.md, specs/phase-skills/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| test-design-planner | 基于 proposal.md + design.md 撰写 test-design.md；通过 Grep 源码提取真实 API 签名作为补充输入；输出正向 AC 和反向 AC 两类业务场景，不输出参数类型和风险标记 | `plugins/dev-team/agents/test-design-planner.md` | proposal.md, design.md, 源码文件（Grep） | Markdown prompt (Agent) |
| test-design-evaluator | 评估 test-design.md 的完整性和覆盖度；T2 检查项验证每个正向 AC 和反向 AC 在 coverage map 中有对应条目 | `plugins/dev-team/agents/test-design-evaluator.md` | test-design.md, proposal.md | Markdown prompt (Agent) |
| test-gen-generator | 读取 test-design.md 和源码文件，直接撰写列于源码目录的测试文件；移除文件类型黑名单；从源码提取参数类型并系统化推导边界场景 | `plugins/dev-team/agents/test-gen-generator.md` | test-design.md, 源码文件（无限制读取） | Markdown prompt (Agent) |
| test-gen-evaluator | 评估生成的测试代码；G1 检查共存测试文件存在性；G2 检查文件命名和共存位置规范 | `plugins/dev-team/agents/test-gen-evaluator.md` | test-design.md, git diff | Markdown prompt (Agent) |
| test-design.md.template | 测试设计模板：新增反向 AC（Reverse ACs）章节；移除参数类型表和风险标记章节 | `plugins/dev-team/templates/artifacts/test-design.md.template` | 无 | Markdown template |
| phase-test-design skill | 纯 P→E 编排器，行为不变。所有正向/反向 AC 逻辑由代理定义承载 | `plugins/dev-team/skills/phase-test-design/SKILL.md` | test-design-planner, test-design-evaluator | Skill (YAML + Markdown) |
| phase-test-gen skill | 纯 G→E 编排器，行为不变。所有源码读取、共存输出、边界推导逻辑由代理定义承载 | `plugins/dev-team/skills/phase-test-gen/SKILL.md` | test-gen-generator, test-gen-evaluator | Skill (YAML + Markdown) |

### 不受影响的组件

| 组件 | 原因 | 文件位置 |
|------|------|----------|
| phase-proposal | out_of_scope | `plugins/dev-team/skills/phase-proposal/SKILL.md` |
| phase-dev-design | out_of_scope | `plugins/dev-team/skills/phase-dev-design/SKILL.md` |
| phase-implement | out_of_scope | `plugins/dev-team/skills/phase-implement/SKILL.md` |
| phase-unit-test | out_of_scope | `plugins/dev-team/skills/phase-unit-test/SKILL.md` |
| phase-integration-test | out_of_scope | `plugins/dev-team/skills/phase-integration-test/SKILL.md` |
| phase-code-review | out_of_scope | `plugins/dev-team/skills/phase-code-review/SKILL.md` |
| unit-test-evaluator / executor | out_of_scope | `plugins/dev-team/agents/unit-test-*.md` |
| integration-test-evaluator / executor | out_of_scope | `plugins/dev-team/agents/integration-test-*.md` |
| code-review-evaluator | out_of_scope | `plugins/dev-team/agents/code-review-evaluator.md` |
| architecture agent | out_of_scope | `plugins/dev-team/agents/architecture.md` |

### 组件图

```
         +-- 前置技能 (proposal, dev-design) -- gate check --+
         |                                                    |
         v                                                    |
  phase-test-design (SKILL, UNCHANGED)                        |
    P→E                                                       |
    ├─ test-design-planner (MODIFIED) —— Grep 源码提取 API 签名
    │   └── 输出: Forward ACs + Reverse ACs (无参数类型/风险标记)
    └─ test-design-evaluator (MODIFIED)
        └── T2: 验证正向/反向 AC 全部有 coverage map 条目
                    |
                    v
  phase-test-gen (SKILL, UNCHANGED)
    G→E
    ├─ test-gen-generator (MODIFIED)
    │   ├── 输入: 源码文件 (移除黑名单) + test-design.md
    │   ├── 输出: 共存的测试文件 (e.g., src/test_auth.py)
    │   └── 逻辑: 参数类型 → 边界场景系统映射
    └─ test-gen-evaluator (MODIFIED)
        ├── G1: 共存测试文件存在性检查
        └── G2: 命名规范 + 共存位置检查
                    |
                    v
         +-- 后续技能 (unit-test, implement, ...)
```

---

## 数据流

### 流程描述

**test-design 阶段 (WHAT to test)**

```
phase-test-design skill (编排器不变)
  │
  ├─ Gate check: phase_check("03-test-design") → 通过则继续
  │
  ├─ [P] test-design-planner (代理定义已修改)
  │     ├─ 输入: proposal.md + design.md
  │     ├─ 新增补充输入: Grep 源码提取变更涉及的真实 API 签名
  │     ├─ 输出: test-design.md
  │     │   ├─ Forward ACs: 快乐路径业务场景 (引用 proposal.md AC)
  │     │   └─ Reverse ACs: 异常路径业务场景 (错误处理、无效输入等)
  │     └─ 约束: 不输出参数类型表、不输出风险标记
  │
  ├─ [E] test-design-evaluator (代理定义已修改)
  │     ├─ T2 检查项: 每个 Forward AC 和 Reverse AC 在 coverage map 中有对应条目
  │     └─ 输出: phase_log("03-test-design", verdict)
  │
  └─ Verdict 循环: fail → 回退 Planner → 最多 5 次
```

**test-gen 阶段 (HOW to test)**

```
phase-test-gen skill (编排器不变)
  │
  ├─ Gate check: phase_check("04-test-gen") → 通过则继续
  │
  ├─ [G] test-gen-generator (代理定义已修改)
  │     ├─ 输入: test-design.md
  │     ├─ 新增: 直接读取源码文件 (无黑名单限制)
  │     │   └── 提取参数类型、方法签名、业务逻辑
  │     ├─ 输出: 共存测试文件
  │     │   ├── .py → test_<module>.py (同目录)
  │     │   ├── .ts/.tsx → <module>.test.ts (同目录)
  │     │   └── .rs → <module>_test.rs 或 inline mod tests
  │     │   └── .go → <module>_test.go (同目录)
  │     └─ 生成内容:
  │         ├── 快乐路径测试 (从 Forward ACs 推导)
  │         ├── 异常路径测试 (从 Reverse ACs 推导)
  │         └── 边界场景测试 (从参数类型系统推导)
  │
  ├─ [E] test-gen-evaluator (代理定义已修改)
  │     ├─ G1: 源码中每个公开方法在源码目录中有对应的测试文件 (共存)
  │     └─ G2: 测试文件命名遵循语言规范且与源码共存于同一目录
  │
  └─ Verdict 循环: fail → 回退 Generator → 最多 5 次
```

### 参数类型 → 边界场景映射

test-gen-generator 在生成边界测试时使用的系统映射：

| 类型 | 边界值 | 用例数 |
|------|--------|--------|
| int / number | 0, -1, MAX_INT, None/undefined | 4+1 正常值 |
| str / string | "", 超长字符串(>1000 chars), 特殊字符(\n \0 emoji), None | 4+1 正常值 |
| bool | True, False, None | 3 |
| list / array | [], [单元素], 超大列表, None | 4+1 正常值 |
| dict / object | {}, 缺失必填字段, 多余字段, None | 4+1 正常值 |
| Optional[T] | None | 1（与其他边界合并） |
| Enum | 每个枚举值, 非法枚举值 | N+1 |
| float | 0.0, -0.0, NaN, Inf, None | 5+1 正常值 |

### 数据模型

本变更不引入新数据模型。受影响的数据结构如下：

| 模型 | 字段 | 变更 | 持久化 |
|------|------|------|--------|
| test-design.md | Forward ACs 章节 (新增), Reverse ACs 章节 (新增), 参数类型表 (移除), 风险标记 (移除) | 模板结构更新 | `openspec/changes/<name>/test-design.md` |
| 测试文件 | 无固定 schema — 源码目录中的测试文件 | 输出位置从 `openspec/changes/<name>/tests/` 改为源码同一目录 | 源码目录 (colocated) |

---

## 路由/API 设计

不适用。本变更不涉及 API、CLI 或 MCP 工具的新增或修改。

### 受影响的路由/API

| 方法 | 路径/命令 | 描述 | 变更类型 |
|------|----------|------|----------|
| Agent | `dev-team:test-design-planner` | 测试设计规划 | Input 增加 Grep 源码；Output 增加 Forward/Reverse ACs，禁止参数类型/风险标记 |
| Agent | `dev-team:test-design-evaluator` | 测试设计评估 | T2 检查项语义变更 |
| Agent | `dev-team:test-gen-generator` | 测试代码生成 | 移除文件黑名单；新增源码读取和边界映射；输出路径变更 |
| Agent | `dev-team:test-gen-evaluator` | 测试代码评估 | G1、G2 检查项语义变更 |
| Skill | `phase-test-design` | test-design 阶段编排 | **无变更** — 代理定义承载所有新行为 |
| Skill | `phase-test-gen` | test-gen 阶段编排 | **无变更** — 代理定义承载所有新行为 |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **test-design 不输出参数类型和风险标记** | test-design 聚焦 WHAT to test（业务场景），过早引入具体参数类型会导致类型假设与最终实现矛盾。test-gen 阶段从源码直接提取类型参数，信息更准确 | **备选：test-design 也输出参数类型表**。被拒理由：违背 WHAT vs HOW 职责分离原则；类型信息在 test-design 阶段可能已过时，test-gen 直接从源码读取更可靠 |
| D2 | **test-gen-generator 移除所有文件类型黑名单** | test-gen-generator 需要读取源码理解方法语义和参数类型才能生成有意义的测试。黑名单（.ts/.tsx/.js/.py/.rs/.go 等）阻止了核心功能的实现 | **备选：保留黑名单但扩大允许类型列表**。被拒理由：黑名单本身是脆弱的约束，任何新增语言都需要更新黑名单；代理应通过提示约束而非硬编码黑名单来控制行为 |
| D3 | **测试文件与源码共存（colocated）** | 多数主流语言/框架（Python unittest/pytest、Jest、cargo test、go test）默认从源码目录发现测试文件。共存降低了开发者理解测试与源码对应关系的心智负担 | **备选：保留 `openspec/changes/<name>/tests/` 隔离存放**。被拒理由：隔离存放与实际项目实践不符；开发者需在目录间跳转；无法直接利用框架的测试发现机制 |
| D4 | **边界场景从参数类型系统化推导** | 常见边界值（int→0/-1/MAX、str→空/超长/特殊字符等）可枚举和系统化。映射表消除了人工编写的重复劳动和遗漏风险 | **备选：保持当前人工编写边界场景方式**。被拒理由：缺少类型依据的边界场景易遗漏；不同开发者推导边界的标准不一致；无法保证系统性覆盖 |
| D5 | **test-design 阶段技能编排不变，代理定义承载新行为** | phase-test-design 和 phase-test-gen 技能已是精简编排器（单行 prompt + gate check + verdict 循环），满足全部需求。代理定义拥有完整 Input/Process/Output/Constraints，承载领域逻辑最合适 | **备选：在技能层添加 Forward/Reverse AC 分类和参数提取指令**。被拒理由：技能层已有清晰的前后分离（gate/execute/report），添加领域知识会导致编排与领域知识耦合，违背 slim 架构 |
| D6 | **test-gen 对无类型文件使用参数名推断类型，标注 P2 + TODO** | 无类型语言（JavaScript、无类型提示 Python）无法从签名获取准确类型，参数名推断（count→int, username→str）是当前最佳近似方案。标记为 P2 并添加 TODO 提示开发者审阅 | **备选：对无类型文件跳过边界测试生成**。被拒理由：完全跳过意味着无类型文件得不到任何边界测试覆盖，降低测试质量；P2 + TODO 方案至少提供了骨架和提示，开发者可快速启用 |
| D7 | **生成的测试骨架包含 TODO/skip 标记** | 测试文件写入源码目录后可能被测试框架自动发现。TODO/skip 标记防止新生成的空骨架测试影响现有的 CI/CD 流水线 | **备选：通过 `.gitignore` 过滤测试骨架文件**。被拒理由：gitignore 粒度不够精细，可能过滤掉后续开发者完善后的测试文件；TODO/skip 是显式的意图表达 |

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。所有修改在现有 agent.md 文件和模板文件范围内，使用现有 Agent 工具和 MCP 工具。

### 构建/测试依赖

- 无新增构建/测试依赖。测试框架的测试发现机制依赖正确的文件命名规范（`test_*.py`, `*.test.ts`, `*_test.rs`, `*_test.go`），这些规范沿用项目的现有测试约定。

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 测试文件写入源码目录可能被误提交 | 源码目录混入测试文件 | 低 | 测试文件遵循明确命名规范（test_* / *.test.ts / *_test.rs / *_test.go），便于 gitignore 过滤 |
| 移除黑名单后 test-gen 读取过多文件超出上下文窗口 | generator 上下文溢出或输出质量下降 | 中 | 限制读取范围为变更涉及的文件（参考 proposal in_scope 和 spec files）；通过代理定义中的约束控制范围 |
| 无类型语言参数推断不准确导致无效测试 | 生成的边界测试无法实际运行 | 中 | test-gen 对无类型参数生成的测试标记为 P2 优先级并添加 TODO 标记，提示开发者审阅后启用 |
| 共存测试文件与现有 CI/CD 配置冲突 | CI 流水线可能尝试执行新生成的骨架测试 | 低 | 生成的测试骨架包含 TODO 或 skip 标记，不会影响现有测试套件；明确告知用户需审阅后再启用 |
| T2 从 tests/ 路径检查改为 AC 覆盖检查后评估器错过源码级问题 | 评估器不再检查方法级的测试覆盖 | 低 | 设计上 test-design 关注业务场景（WHAT），test-gen 负责技术实现（HOW）；G1/G2 在 test-gen 阶段补全了方法级的文件检查 |

---

## 迁移步骤

1. 修改 `plugins/dev-team/agents/test-design-planner.md` — 添加 Grep 源码指令、新增 Forward ACs / Reverse ACs 输出约束、禁止输出参数类型和风险标记
2. 修改 `plugins/dev-team/templates/artifacts/test-design.md.template` — 新增 Reverse ACs 章节、移除参数类型表和风险标记章节
3. 修改 `plugins/dev-team/agents/test-design-evaluator.md` — 更新 T2 检查项从 tests/ 路径检查为「coverage map 覆盖所有正向 AC 和反向 AC」
4. 修改 `plugins/dev-team/agents/test-gen-generator.md` — 移除文件类型黑名单约束；添加源码读取指令；添加共存文件输出指令；添加参数类型 → 边界场景映射表
5. 修改 `plugins/dev-team/agents/test-gen-evaluator.md` — 更新 G1 从 tests/ 文件检查为源码共存文件存在性检查；更新 G2 从 tests/ 位置检查为命名规范 + 共存位置检查
6. 验证 phase-test-design 和 phase-test-gen 技能文件无需修改
7. 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号

---

## 待决问题

- 无
