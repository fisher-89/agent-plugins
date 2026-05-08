# 实施路线图

> 版本: 1.1
> 日期: 2026-05-07
> 状态: Phase 1/2 部分完成

## 1. 总览

```
Phase 1 (P0) ── 核心门禁 ────────────── ✅ 完成 (100%)
Phase 2 (P1) ── 增强能力 ────────────── ⚠️ 部分完成 (60%)
Phase 3 (P2) ── 合规与设计约束 ──────── ❌ 待实现
Phase 4 (P2) ── 流水线与 CI ─────────── ❌ 待实现
```

**总计**: 约 9 周（含测试和文档）
**已耗时**: 约 2 周
**当前进度**: Phase 1/2 核心功能已实现，剩余门禁强化待完成

---

## 2. Phase 1：核心门禁 (P0)

> 目标：TDD 有强制力，Review 自动执行

### 2.1 任务清单

| # | 任务 | 涉及方案 | 优先级 | 预估 | 依赖 |
|---|------|----------|--------|------|------|
| 1.1 | 检测项目测试框架 | D | P0 | 1d | - |
| 1.2 | 生成可执行测试骨架 (Jest/Vitest) | A | P0 | 2d | 1.1 |
| 1.3 | 生成可执行测试骨架 (pytest) | A | P0 | 1d | 1.1 |
| 1.4 | 改造 apply-change SKILL.md：增加测试步骤 | D | P0 | 2d | 1.2 |
| 1.5 | 实现测试运行门禁逻辑 | D | P0 | 2d | 1.4 |
| 1.6 | 改造 apply-change SKILL.md：增加自动 Review | F | P0 | 2d | - |
| 1.7 | 增强 code-review Agent prompt | F | P0 | 1d | - |
| 1.8 | Review 结果持久化到 test-reports/ | E | P0 | 1d | 1.6 |
| 1.9 | 集成测试：完整 apply 流程 | D+F | P0 | 2d | 1.5, 1.8 |

### 2.2 交付物

- `plugin/utils/test-framework.py` — ✅ 测试框架检测
- `plugin/templates/test-jest.js` — ✅ Jest 骨架模板
- `plugin/templates/test-pytest.py` — ✅ pytest 骨架模板
- `plugin/utils/test-runner.py` — ✅ 测试运行逻辑
- `plugin/utils/lint-runner.py` — ✅ 类型/Lint 检查运行器
- `plugin/skills/openspec-apply-change/SKILL.md` — ✅ 改造后含 TDD+Review+全量测试
- `plugin/agents/code-review.md` — ✅ 增强后 Agent prompt
- `plugin/templates/code-review-report.md` — ✅ Review 报告模板
- `plugin/hooks/pre-tool-commit-review.py` — ✅ Review 门禁 + 追溯 + Lint/Type/Test 门禁

### 2.3 验收标准

- [x] apply-change 中每个 task 完成前自动运行测试
- [x] 测试失败时阻止标记 task 完成
- [x] 所有 task 完成后自动启动 code review
- [x] Review 结果写入 test-reports/code-review-*.md
- [x] Review BLOCK 时阻止 commit (安全问题 deny)
- [x] 类型/Lint 检查集成
- [x] 全量测试门禁（所有 Task 完成后）

### 2.4 里程碑

```
Week 1: 1.1 → 1.3 → 1.4 (测试骨架 + Skill 改造)
Week 2: 1.5 → 1.6 → 1.7 → 1.8 → 1.9 (门禁 + Review + 集成测试)
Week 2+: Lint/Type 门禁 + 全量测试门禁 ✅
```

---

## 3. Phase 2：增强能力 (P1)

> 目标：测试骨架可执行，强制补测试，Review 结果可追溯

### 3.1 任务清单

| # | 任务 | 涉及方案 | 优先级 | 预估 | 依赖 |
|---|------|----------|--------|------|------|
| 2.1 | 改造 pre-tool-openspec-test.py：生成可执行测试 | A | P1 | 2d | Phase 1 |
| 2.2 | 增量更新 test-reports 测试用例 | C | P1 | 2d | Phase 1 |
| 2.3 | 实现 task 完成时强制补测试检查 | B | P1 | 2d | 2.1 |
| 2.4 | Hook 增强：pre-tool-commit-review 检查已有 review | F | P1 | 1d | Phase 1 |
| 2.5 | Review 门禁：安全问题 deny | H | P1 | 1d | 2.4 |
| 2.6 | 多次 Review 追溯与对比 | E | P1 | 1d | Phase 1 |
| 2.7 | Archive 合规检查基础版 (C1-C8) | K | P1 | 2d | - |
| 2.8 | 集成测试：Phase 2 完整流程 | A+B+E+K | P1 | 2d | 2.3, 2.7 |

### 3.2 交付物

- `plugin/hooks/pre-tool-openspec-test.py` — ✅ 改造后生成可执行测试
- `plugin/hooks/pre-tool-commit-review.py` — ✅ 增强：检查已有 review
- `plugin/utils/review-parser.py` — ❌ Review 结果解析工具（待实现）
- `plugin/utils/compliance-check.py` — ⚠️ 合规检查脚本 (C1-C4, C6-C8 已实现)
- `plugin/templates/compliance-report.md` — ❌ 合规报告模板（待实现）

### 3.3 验收标准

- [x] Hook 生成可运行的 Jest/pytest 测试文件
- [x] task 完成时检查对应测试文件是否存在
- [x] commit 前检查 review 结果，安全问题 deny
- [x] 多次 review 报告可追溯
- [ ] archive 前运行 C1-C8 合规检查（当前为软约束）
- [ ] C5 测试运行验证（待实现）
- [ ] C10 Spec 合规审查（待实现）

### 3.4 里程碑

```
Week 3: 2.1 → 2.2 → 2.3 (测试增强)
Week 4: 2.4 → 2.5 → 2.6 → 2.7 → 2.8 (Review 增强 + 合规)
```

---

## 4. Phase 3：合规与设计约束 (P2)

> 目标：Propose 强制 design.md，Spec 合规审查

### 4.1 任务清单

| # | 任务 | 涉及方案 | 优先级 | 预估 | 依赖 |
|---|------|----------|--------|------|------|
| 3.1 | Propose 阶段强制生成 design.md | I | P2 | 2d | - |
| 3.2 | design.md 模板设计 | I | P2 | 1d | 3.1 |
| 3.3 | Proposal Scope 解析器 | G | P2 | 2d | - |
| 3.4 | 代码扫描：API 路由提取 | G | P2 | 2d | - |
| 3.5 | 代码扫描：数据模型提取 | G | P2 | 1d | 3.4 |
| 3.6 | Scope vs 实现对比算法 | G | P2 | 2d | 3.3, 3.5 |
| 3.7 | Spec 合规报告生成 | G | P2 | 1d | 3.6 |
| 3.8 | 合规检查扩展：C9 (design.md) + C10 (spec 合规) | K | P2 | 2d | 3.7, Phase 2 |
| 3.9 | 实现偏差检测与告警 | J | P2 | 2d | 3.6 |
| 3.10 | 集成测试：Phase 3 完整流程 | I+G+K | P2 | 2d | 3.8, 3.9 |

### 4.2 交付物

- `plugin/skills/openspec-propose/SKILL.md` — 改造后含 design.md 生成
- `plugin/templates/design.md` — 设计文档模板
- `plugin/utils/spec-compliance.py` — Spec 合规分析脚本
- `plugin/utils/compliance-check.py` — 扩展 C9/C10

### 4.3 验收标准

- [ ] Propose 阶段自动生成 design.md
- [ ] design.md 包含架构/API/数据模型
- [ ] 解析 proposal.md Scope 提取功能清单
- [ ] 扫描代码提取 API 路由和数据模型
- [ ] 对比生成 covered/missing/extra 报告
- [ ] archive 合规检查包含 C9/C10

### 4.4 里程碑

```
Week 5: 3.1 → 3.2 → 3.3 → 3.4 (design.md + 解析器)
Week 6: 3.5 → 3.6 → 3.7 → 3.8 → 3.9 → 3.10 (合规 + 偏差检测)
```

---

## 5. Phase 4：流水线与 CI (P2)

> 目标：端到端自动化，CI 集成

### 5.1 任务清单

| # | 任务 | 涉及方案 | 优先级 | 预估 | 依赖 |
|---|------|----------|--------|------|------|
| 4.1 | 流水线状态管理 (pipeline-state.json) | L | P2 | 2d | - |
| 4.2 | 流水线主 Skill：5 阶段编排 | L | P2 | 3d | 4.1, Phase 1-3 |
| 4.3 | 阶段门禁函数库 | M | P2 | 2d | 4.2 |
| 4.4 | 暂停/恢复机制 | L | P2 | 2d | 4.1 |
| 4.5 | Pipeline Completion Report 生成 | L | P2 | 1d | 4.2 |
| 4.6 | opsx:pipeline 快捷命令 | L | P2 | 1d | 4.2 |
| 4.7 | GitHub Actions workflow: openspec-pr.yml | N | P2 | 2d | Phase 2-3 |
| 4.8 | 分支保护配置脚本 | N | P2 | 1d | 4.7 |
| 4.9 | on-user-prompt.py 增强：检测 pipeline 状态 | L | P2 | 1d | 4.1 |
| 4.10 | 端到端集成测试 | L+M+N | P2 | 3d | 4.6, 4.7 |
| 4.11 | 文档更新 | - | P2 | 2d | 4.10 |

### 5.2 交付物

- `plugin/skills/openspec-pipeline/SKILL.md` — 流水线主 skill
- `plugin/skills/openspec-pipeline/state.py` — 状态管理
- `plugin/skills/openspec-pipeline/gates.py` — 门禁函数
- `plugin/commands/opsx/pipeline.md` — 快捷命令
- `.github/workflows/openspec-pr.yml` — CI workflow

### 5.3 验收标准

- [ ] `/openspec-pipeline` 一键触发完整流程
- [ ] 5 阶段自动转换，门禁检查
- [ ] 支持暂停/恢复/状态查询
- [ ] 输出 Completion Report
- [ ] PR 创建时 CI 自动运行检查
- [ ] 分支保护要求 OpenSpec 检查通过

### 5.4 里程碑

```
Week 7: 4.1 → 4.2 → 4.3 (流水线核心)
Week 8: 4.4 → 4.5 → 4.6 → 4.9 (暂停恢复 + 报告)
Week 9: 4.7 → 4.8 → 4.10 → 4.11 (CI + 集成测试 + 文档)
```

---

## 6. 依赖关系图

```
Phase 1 ─────────────────────────────────────────────────────
  1.1 ──▶ 1.2 ──▶ 1.4 ──▶ 1.5 ──┐
  1.1 ──▶ 1.3                     │
  1.6 ──▶ 1.8                     ├──▶ 1.9
  1.7 ────────────────────────────┘

Phase 2 (依赖 Phase 1) ──────────────────────────────────────
  2.1 ──▶ 2.3 ──┐
  2.4 ──▶ 2.5   │
  2.6            ├──▶ 2.8
  2.7 ───────────┘

Phase 3 (部分依赖 Phase 2) ─────────────────────────────────
  3.1 ──▶ 3.2
  3.3 ──┐
  3.4 ──▶ 3.5 ──▶ 3.6 ──▶ 3.7 ──▶ 3.8 ──┐
  3.6 ──▶ 3.9                              ├──▶ 3.10
  Phase2 ──▶ 3.8                           ┘

Phase 4 (依赖 Phase 1-3) ────────────────────────────────────
  4.1 ──▶ 4.2 ──▶ 4.3
  4.1 ──▶ 4.4                ┐
  4.2 ──▶ 4.5                │
  4.2 ──▶ 4.6                ├──▶ 4.10 ──▶ 4.11
  Phase2-3 ──▶ 4.7 ──▶ 4.8  │
  4.1 ──▶ 4.9                ┘
```

---

## 7. 风险管理

### 7.1 技术风险

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Hook 限制无法满足需求 | 中 | 高 | Skill 层面补偿，不过度依赖 Hook |
| 测试框架检测不准确 | 中 | 中 | 多维度检测 + fallback + 用户确认 |
| Agent review 不稳定 | 高 | 中 | 结构化 checklist + 人工可覆盖 |
| CI 与本地行为不一致 | 中 | 中 | 共享检查脚本，本地/CI 复用 |

### 7.2 流程风险

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 门禁过严降低开发效率 | 高 | 高 | 可配置门禁强度，支持用户覆盖 |
| Phase 间依赖导致阻塞 | 中 | 中 | Phase 2/3 部分任务可并行 |
| 用户不信任自动 review | 中 | 中 | 初期 soft-gate，逐步增强 |
| 流水线状态管理复杂 | 中 | 中 | 简单 JSON + 原子写入 |

### 7.3 回滚策略

每个 Phase 独立，可单独回滚：

- Phase 1 回滚：恢复 SKILL.md 原始版本
- Phase 2 回滚：禁用增强 Hook，使用原始 Hook
- Phase 3 回滚：跳过 design.md 生成，合规检查降级
- Phase 4 回滚：禁用 pipeline skill，使用独立 skill

---

## 8. 配置驱动

### 8.1 .openspec.yaml 扩展

```yaml
# OpenSpec Configuration
version: "1.1"

# Pipeline gates configuration
gates:
  # Phase 1: TDD gates
  tdd:
    enabled: true
    test_before_implement: true     # 方案 A: 生成测试骨架
    test_must_pass: true            # 方案 D: 测试必须通过
    force_test_coverage: true       # 方案 B: 强制补测试
    skip_patterns:
      - "*.css"
      - "*.md"
      - ".gitignore"
    test_timeout: 60                # seconds

  # Phase 1-2: Review gates
  review:
    enabled: true
    auto_trigger: true              # 方案 F: 自动触发
    persist_results: true           # 方案 E: 结果持久化
    security_deny: true             # 方案 H: 安全问题 deny
    allow_override: true            # 非安全问题可覆盖

  # Phase 2-3: Compliance gates
  compliance:
    enabled: true
    require_design: false           # 方案 I: 强制 design.md (默认关闭)
    spec_compliance: true           # 方案 G: Spec 合规审查
    blocking_checks: [C1, C2, C3, C4, C5, C6, C7, C8]
    optional_checks: [C9, C10]

  # Phase 4: Pipeline
  pipeline:
    enabled: false                  # 默认关闭，Phase 4 完成后开启
    stages: [explore, propose, apply, review, archive]
    allow_pause: true
    allow_resume: true

  # CI integration
  ci:
    enabled: false
    pr_check: true
    merge_gate: true
```

### 8.2 配置优先级

```
命令行参数 > 环境变量 > .openspec.yaml > 默认值
```

---

## 9. 度量指标

### 9.1 过程度量

| 指标 | 计算方式 | 目标 |
|------|----------|------|
| TDD 遵循率 | 测试先行的 task 数 / 总 task 数 | > 80% |
| 测试通过率 | 通过的测试 / 总测试数 | > 95% |
| Review 覆盖率 | 有 review 的 change 数 / 总 change 数 | 100% |
| 一次 Review 通过率 | 首次 PASS 的 change 数 / 总 change 数 | > 70% |
| 合规检查通过率 | archive 一次通过的 change 数 / 总 change 数 | > 85% |

### 9.2 效率度量

| 指标 | 计算方式 | 目标 |
|------|----------|------|
| 平均 task 完成时间 | sum(task_duration) / task_count | 基线测量后设定 |
| Review 平均耗时 | sum(review_duration) / review_count | < 5 min |
| Pipeline 端到端时间 | archive_time - explore_time | 基线测量后设定 |
| 门禁阻断次数 | gate_block_count / total_checks | < 20% |

---

## 10. 里程碑总结

| 里程碑 | 日期 | 交付内容 | 状态 |
|--------|------|----------|------|
| M1: 核心门禁 | Week 2 结束 | TDD 门禁 + 自动 Review + Lint/Type 检查 + 全量测试 | ✅ 100% 完成 |
| M2: 增强能力 | Week 4 结束 | 可执行测试骨架 + 合规检查 C1-C8 | ⚠️ 60% 完成 |
| M3: 合规与设计 | Week 6 结束 | design.md + Spec 合规审查 | ❌ 待实现 |
| M4: 流水线 | Week 9 结束 | 端到端自动化 + CI 集成 | ❌ 待实现 |

```
M1 ──────▶ M2 ──────▶ M3 ──────▶ M4
Week 2      Week 4      Week 6      Week 9
 │           │           │           │
 ▼           ▼           ▼           ▼
 TDD+Review  测试增强    合规完整    自动化
 Lint+Type   合规基础⚠️   设计约束    CI集成
 全量测试✅   C5/C10❌
```

**M1 已全部完成！**

**M2 剩余项：**
- C5 测试运行验证
- C10 Spec 合规审查
- 合规检查强制（软约束 → 硬约束）
