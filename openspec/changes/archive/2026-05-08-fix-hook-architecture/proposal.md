# Proposal: Fix Hook Architecture Gaps

> **Change**: fix-hook-architecture
> **Created**: 2026-05-08
> **Status**: Proposed

## Problem Statement

当前插件存在 Hook 层与 SKILL 层约束错位的问题，导致端到端流程保障不完整：

### 1. `find_active_change()` 逻辑缺陷 (P0)

**现象**：当所有 task 完成后，`pre-tool-commit-review.py` 的 `find_active_change()` 函数返回 `None`，导致 commit 前的门禁检查被绕过。

**根因**：
```python
# 当前逻辑
def find_active_change(changes_dir):
    for entry in os.listdir(changes_dir):
        total, done = count_tasks(tasks_path)
        if done < total:  # ← 只找未完成的
            return entry
    return None  # ← 全部完成后返回 None
```

**影响**：
- lint/type 检查被跳过
- 全量测试被跳过
- code review 门禁被跳过
- 可能提交有问题的代码

### 2. Archive 无 Hook 阻断 (P0)

**现象**：合规检查仅在 SKILL 层指令中，Claude 可以忽略。

**根因**：
```json
// hooks.json 只匹配 Bash，不匹配 Skill
{
  "PreToolUse": [
    {"matcher": "Write|Edit", ...},
    {"matcher": "Bash", ...}  // ← 缺少 Skill matcher
  ]
}
```

**影响**：
- 不合规的 change 可能被归档
- 缺少 proposal/tasks/review 等必需文件
- 质量门禁失效

### 3. SKILL 软约束无保障 (P1)

**现象**：apply-change SKILL 中的关键步骤（测试范围识别、即时 lint、范围测试）无 Hook 层保障，Claude 可选择跳过。

**影响**：
- TDD 流程可能被绕过
- 错误累积到 commit 时才发现
- 修复成本增加

---

## Proposed Solution

### 方案 A: 修复 Hook 逻辑 + 新增 Skill Hook

**优点**：
- 最小改动
- 保持现有 SKILL 不变
- Hook 层强制保障

**改动**：
1. 修复 `find_active_change()` 逻辑
2. 新增 `pre-tool-skill.py` Hook
3. 新增 `Skill` matcher 到 hooks.json

### 方案 B: 引入 PostToolUse Hook (未来)

**说明**：当前 Claude Code 不支持 PostToolUse Hook，无法在 Write 完成后立即触发测试。方案 A 是当前约束下的最优解。

---

## Scope

### In Scope

- [x] 修复 `find_active_change()` 逻辑
- [x] 新增 `pre-tool-skill.py` Hook
- [x] 更新 `hooks.json` 配置
- [x] 增强 `pre-tool-openspec-test.py`（可选：调用 test-generator）
- [ ] 接通 ERROR → Task → 重入 Apply 闭环
  - 增强 `pre-tool-commit-review.py` 注入指令
  - 增强 `pre-tool-skill.py` 检查 pending errors
  - 集成测试验证端到端流程

### Out of Scope

- PostToolUse Hook（平台限制）
- CI/CD 集成（P2 优先级）
- 新建 `openspec-pipeline` skill（P2 优先级）

---

## Success Criteria

1. **全部 task 完成后 commit 仍触发门禁**
   - 运行 lint/type 检查
   - 运行全量测试
   - 检查 code review 状态

2. **Archive 前强制合规阻断**
   - C1-C8 失败时 deny
   - 用户可 override（记录日志）

3. **端到端测试通过**
   - 创建测试 change → 完成所有 task → commit → archive
   - 每个环节门禁生效

---

## Risks

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Hook 过度阻断 | 用户体验下降 | 提供清晰的错误信息和修复建议 |
| 性能影响 | commit 延迟 | 异步运行检查，或缓存结果 |
| 兼容性问题 | 旧版 Claude Code | 检测平台版本，降级处理 |

---

## Dependencies

- 无外部依赖
- 需要现有 `compliance-check.py`、`lint-runner.py`、`test-runner.py` 正常工作

---

## References

- `improvement.md` — 原始问题分析
- `plugin/hooks/pre-tool-commit-review.py` — 当前实现
- `plugin/skills/openspec-archive-change/SKILL.md` — archive 流程
