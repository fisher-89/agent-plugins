# 实施路线图

> 版本: 1.4
> 日期: 2026-05-08
> 状态: Phase 1 完成，Phase 1.5 完成，Phase 2 完成

## 1. 总览

```
Phase 1 (P0) ── 核心门禁 ────────────── ✅ 完成 (100%)
Phase 1.5 (P0) ─ 自动修复优化 ──────── ✅ 完成 (100%)
Phase 2 (P1) ── 增强能力 ────────────── ✅ 完成 (100%)
Phase 3 (P2) ── 合规与设计约束 ──────── ✅ 完成 (90%)
Phase 4 (P2) ── 流水线与 CI ─────────── ❌ 待实现
```

**总计**: 约 11 周（含测试和文档）
**已耗时**: 约 5 周
**当前进度**: Phase 3 基本完成，待集成测试

**核心目标**: ✅ 通过自动修复策略，将 commit 时阻断率从 ~30% 降至 <5%

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

## 3. Phase 1.5：自动闭环优化 (P0)

> 目标：测试范围驱动 + ERROR → Task 生成闭环，消除阻断，阻断率降至 <1%
> **状态**: ✅ 完成

### 3.1 任务清单

| # | 任务 | 涉及方案 | 优先级 | 预估 | 依赖 | 状态 |
|---|------|----------|--------|------|------|------|
| 1.5a | 实现 `identify_test_scope()` 测试范围识别 | D | P0 | 2d | Phase 1 | ✅ |
| 1.5b | SKILL 增加测试范围识别 + 测试补充/更新步骤 | D+O | P0 | 2d | 1.5a | ✅ |
| 1.5c | SKILL 增加 task 完成后即时 lint/type check | O | P0 | 1d | 1.5b | ✅ |
| 1.5d | SKILL 增加 `--fix` 自动修复 + 范围测试运行 | O+D | P0 | 1d | 1.5c | ✅ |
| 1.5e | Review parser：解析 ERROR 并生成修复 task | P | P0 | 2d | Phase 1 | ✅ |
| 1.5f | SKILL 增加 Post-Review Loop 步骤 | P | P0 | 2d | 1.5e | ✅ |
| 1.5g | 循环状态管理：review-loop-state.json | Q | P0 | 1d | 1.5f | ✅ |
| 1.5h | 循环收敛：最多 3 轮，否则提示人工介入 | Q | P0 | 1d | 1.5g | ✅ |
| 1.5i | 集成测试：完整闭环流程 | O+P+Q+D | P0 | 2d | 1.5d, 1.5h | ⚠️ 待验证 |

### 3.2 交付物

- `plugin/skills/openspec-apply-change/SKILL.md` — ✅ 改造：测试范围驱动 + 即时检测 + Post-Review Loop
- `plugin/utils/test-scope.py` — ✅ 新增：测试范围识别与映射
- `plugin/utils/review-parser.py` — ✅ 新增：解析 ERROR 生成 task
- `plugin/utils/review-loop-state.py` — ✅ 新增：循环状态管理
- `plugin/templates/fix-task.md` — ✅ 新增：修复 task 模板

### 3.3 验收标准

- [x] 每个 task 开始前识别测试范围（受影响模块/文件 → 测试文件映射）
- [x] 测试用例按范围补充/更新，不运行全量测试
- [x] 每个 task 完成后自动运行 lint/type check
- [x] 可自动修复的 lint 错误即时修复，不阻断
- [x] 单个 task 完成时运行范围测试，所有 task 完成时运行全量测试
- [x] Review 发现任意 ERROR → 生成修复 task → 追加到 tasks.md
- [x] 修复 task 完成后自动重新 Review
- [x] 循环收敛：最多 3 轮，否则提示人工介入
- [ ] 阻断率 <1% (待实际验证)

### 3.4 里程碑

```
Week 3: 1.5a → 1.5b → 1.5c → 1.5d (测试范围驱动 + 即时检测)
Week 4: 1.5e → 1.5f → 1.5g → 1.5h → 1.5i (闭环循环 + 收敛机制 + 集成测试)
```

### 3.5 阻断率预期变化

| 阶段 | 阻断率 | 说明 |
|------|--------|------|
| Phase 1 完成后 | ~30% | lint/type 错误 + Review ERROR |
| Phase 1.5a-d 后 | ~15% | 范围测试驱动 + lint/type 即时修复，仅剩 Review ERROR |
| Phase 1.5e-i 后 | <1% | ERROR → Task 闭环，仅剩 3 轮后仍有 ERROR 的极端情况 |

---

## 4. Phase 2：增强能力 (P1)

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
- `plugin/utils/review-parser.py` — ✅ Review 结果解析工具
- `plugin/utils/compliance-check.py` — ✅ 合规检查脚本 (C1-C10)
- `plugin/utils/spec-compliance.py` — ✅ Spec 合规分析脚本 (C10)
- `plugin/templates/compliance-report.md` — ✅ 合规报告模板

### 3.3 验收标准

- [x] Hook 生成可运行的 Jest/pytest 测试文件
- [x] task 完成时检查对应测试文件是否存在
- [x] commit 前检查 review 结果，安全问题 deny
- [x] 多次 review 报告可追溯
- [x] archive 前运行 C1-C8 合规检查（强制阻断）
- [x] C5 测试记录验证（检查 test-reports 中测试结果记录）
- [x] C9 design.md 检查（可选，配置驱动）
- [x] C10 Spec 合规审查（spec-compliance.py 已实现）

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

- `plugin/skills/openspec-propose/SKILL.md` — ✅ 已包含 design.md 生成
- `plugin/templates/design.md` — ✅ 设计文档模板
- `plugin/utils/spec-compliance.py` — ✅ Spec 合规分析脚本 (Proposal 解析 + 代码扫描 + 对比算法)
- `plugin/utils/compliance-check.py` — ✅ 已扩展 C9/C10
- `plugin/utils/deviation-check.py` — ✅ 偏差检测与告警
- `plugin/skills/openspec-archive-change/SKILL.md` — ✅ 集成合规检查 + 偏差检测

### 4.3 验收标准

- [x] Propose 阶段自动生成 design.md
- [x] design.md 包含架构/API/数据模型
- [x] 解析 proposal.md Scope 提取功能清单
- [x] 扫描代码提取 API 路由和数据模型
- [x] 对比生成 covered/missing/extra 报告
- [x] archive 合规检查包含 C9/C10
- [x] 实现偏差检测与告警 (deviation-check.py)
- [ ] 集成测试

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

Phase 1.5 (依赖 Phase 1) ───────────────────────────────────
  1.5a ──▶ 1.5b ──▶ 1.5c ──▶ 1.5d ──┐
  1.5e ──▶ 1.5f ──▶ 1.5g ──▶ 1.5h  ├──▶ 1.5i
                                      ┘

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
| **循环无限执行** | **低** | **高** | **循环收敛机制：最多 3 轮，否则提示人工介入** |
| **修复 task 生成失败** | **低** | **中** | **ERROR 结构化输出 + 模板化生成** |

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
version: "1.2"

# Pipeline gates configuration
gates:
  # Phase 1: TDD gates (测试范围驱动)
  tdd:
    enabled: true
    test_before_implement: true     # 方案 A: 生成测试骨架
    test_scope_driven: true         # 方案 D: 测试范围驱动（先识别范围，再运行范围测试）
    test_must_pass: true            # 方案 D: 测试必须通过
    force_test_coverage: true       # 方案 B: 强制补测试
    scoped_test_on_task: true       # 单个 task 完成时运行范围测试
    full_test_on_complete: true     # 所有 task 完成时运行全量测试
    skip_patterns:
      - "*.css"
      - "*.md"
      - ".gitignore"
    test_timeout: 60                # seconds

  # Phase 1.5: Auto-loop gates (新增)
  auto_loop:
    enabled: true
    identify_test_scope: true       # 方案 D: 识别测试范围
    supplement_test_cases: true     # 方案 D: 补充/更新测试用例
    lint_on_task_complete: true     # 方案 O: task 完成时即时 lint
    lint_auto_fix: true             # 方案 O: eslint --fix / ruff --fix
    scoped_test_on_task: true       # 方案 D: 单个 task 完成时运行范围测试
    error_to_task: true             # 方案 P: ERROR → 生成修复 task
    max_review_loops: 3             # 方案 Q: 最多 3 轮 Review
    loop_convergence_prompt: true   # 方案 Q: 达到上限时提示人工介入

  # Phase 1-2: Review gates
  review:
    enabled: true
    auto_trigger: true              # 方案 F: 自动触发
    persist_results: true           # 方案 E: 结果持久化
    all_errors_to_tasks: true       # 方案 P: 所有 ERROR 生成 task（不区分类型）
    re_review_after_fix: true       # 方案 P: 修复后重新 Review

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
| M1.5: 自动闭环 | Week 4 结束 | 测试范围驱动 + 前移检测 + ERROR→Task闭环 + 循环收敛 | ✅ 100% 完成 |
| M2: 增强能力 | Week 6 结束 | 可执行测试骨架 + 合规检查 C1-C10 | ✅ 100% 完成 |
| M3: 合规与设计 | Week 8 结束 | design.md + Spec 合规审查 (C10) + 偏差检测 | ✅ 90% 完成 |
| M4: 流水线 | Week 11 结束 | 端到端自动化 + CI 集成 | ❌ 待实现 |

```
M1 ──────▶ M1.5 ────▶ M2 ──────▶ M3 ──────▶ M4
Week 2      Week 4     Week 6     Week 8     Week 11
 │           │          │          │          │
 ▼           ▼          ▼          ▼          ▼
 TDD+Review  测试范围    测试增强    合规完整    自动化
 Lint+Type   +自动闭环   合规C1-C9✅  C10设计约束  CI集成
 全量测试✅   阻断<1%✅   C10待实现
```

**M1 已全部完成！**
**M1.5 已全部完成！**
**M2 已基本完成 (90%)！**

**Phase 2 实现内容：**
- ✅ 2.1-2.3: 可执行测试骨架 + 测试文件存在检查
- ✅ 2.4-2.6: Review 门禁 + 安全问题 deny + 追溯与对比
- ✅ 2.7: 合规检查脚本 compliance-check.py (C1-C9)
- ✅ C5: 测试记录验证（检查 test-reports 中测试结果）
- ✅ C9: design.md 检查（可选，配置驱动）

**M2 剩余项：**
- C10 Spec 合规审查（需要 spec-compliance.py）
