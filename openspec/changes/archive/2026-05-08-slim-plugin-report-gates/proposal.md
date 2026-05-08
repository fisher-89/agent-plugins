# Proposal: Slim Plugin with Report-Driven Gates

> **Change**: slim-plugin-report-gates
> **Created**: 2026-05-08
> **Status**: Proposed
> **Supersedes**: fix-hook-architecture (archived, Phase 5 merged here)

## Problem Statement

当前插件将 OpenSpec skill 内容（SKILL.md）内嵌在 `plugin/skills/openspec-*/` 中，导致三个问题：

### 1. Skill 内容与 CLI 版本耦合 (P0)

`session-start-sync-skills.py` 在 SessionStart 时从 openspec CLI 同步 skill 到本地。本地 SKILL.md 可能被定制（如加入 TDD Gate、Review Loop），但每次同步会覆盖定制内容。不同步则与 CLI 版本漂移。

**根因**：插件不应维护 skill 内容的副本。openspec CLI 是 skill 的权威来源。

### 2. 流程完整性依赖 SKILL 软约束 (P0)

SDD 流程步骤（test-scope → test-gen → implement → lint → scoped-test → full-test → review）仅在 SKILL.md 中以指令形式存在。Claude 可以选择跳过任何步骤，Hook 层无法检测"跳步"行为。

**根因**：缺少可审计的步骤执行记录，Hook 无法判断流程是否完整。

### 3. ERROR → Task 闭环未接通 (P1)

`fix-hook-architecture` 的 Phase 5 未完成：review 发现 ERROR 后，缺少自动生成 fix task 并重入 apply 的 Hook 保障。

---

## Proposed Solution

### 核心思路：瘦插件 + 报告驱动门禁

```
当前：内嵌 SKILL + Hook 点状拦截
提议：删除内嵌 SKILL + 报告链完整性检查
```

**三个关键变化**：

1. **删除内嵌 SKILL** — openspec CLI 是 skill 权威来源，插件不再维护副本
2. **报告驱动** — 每个步骤的 utils 输出报告文件，Hook 检查报告链的存在与时间顺序
3. **流程门禁** — 通过报告链完整性判断 SDD 流程是否被严格执行

### 报告驱动原理

```
每个步骤执行后产出报告：
  openspec/changes/<name>/reports/
    task-1_scope.json      (test-scope.py --save-report)
    task-1_skeleton.json   (test-generator.py --save-report)
    task-1_lint.json       (lint-runner.py --save-report)
    task-1_scoped-test.json (test-runner.py --save-report)
    full-test.json          (全量测试)
    code-review.json        (代码审查)

Hook 检查报告链：
  commit 时 → 当前 task 的 scope.ts < lint.ts < scoped-test.ts ?
  archive 时 → 所有 task 报告链 + full-test.ts < review.ts ?
  缺失或乱序 → deny
```

---

## Scope

### In Scope

- 删除 `plugin/skills/openspec-*/` 目录
- 重写 `session-start-sync-skills.py` → `session-start-ensure-openspec.py`（仅检测+询问+安装）
- 增强 utils 添加 `--save-report` 报告输出能力
- 增强 Hook 添加报告链完整性检查
- 接通 ERROR → Task → Re-enter Apply 闭环（从 fix-hook-architecture Phase 5 继承）
- 更新 `improvement.md`、`CLAUDE.md`

### Out of Scope

- CI/CD 集成
- PostToolUse Hook（平台限制）
- openspec CLI 本身的修改

---

## Success Criteria

1. **插件不含 openspec skill 副本** — `plugin/skills/openspec-*/` 不存在
2. **SessionStart 只做安装引导** — 无 skill 同步逻辑
3. **报告链可审计** — 每个 SDD 步骤产出带时间戳的报告文件
4. **Hook 强制执行流程** — 报告链断裂时 deny commit/archive
5. **ERROR → Task 闭环** — review ERROR 自动生成 fix task

---

## Risks

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| openspec CLI 未安装时 skill 不可用 | 功能降级 | SessionStart 询问安装 |
| 报告文件体积增长 | 磁盘占用 | archive 时清理 |
| Hook 报告检查增加延迟 | commit 变慢 | 仅检查元数据，不重跑 |
| Claude 不调用 --save-report | 报告缺失 | Hook 在无报告时默认 deny |

---

## Dependencies

- openspec CLI >= 1.2.0
- fix-hook-architecture 已完成的基础设施（`find_active_change()`、`pre-tool-skill.py`、TDD GATE）
