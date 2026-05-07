# 设计文档：端到端流水线 Skill

> 版本: 1.0
> 日期: 2026-04-30
> 涉及方案: L (流水线 skill) / M (阶段门禁定义) / N (CI 集成 hook)

## 1. 问题定义

### 1.1 当前状态

```
用户手动调用各 skill
    ↓
/openspec-explore → 思考伙伴
    ↓ (用户主动触发)
/openspec-propose → 生成 proposal/tasks
    ↓ (用户主动触发)
/openspec-apply-change → 执行 task 清单
    ↓ (手动 commit)
/openspec-archive-change → 归档
```

**断裂点**:
- 各阶段独立触发，无自动化编排
- 每个阶段完成后需要用户手动触发下一阶段
- 无端到端的状态跟踪
- 中间状态丢失，需要重复输入上下文

### 1.2 目标状态

```
用户输入需求描述
    ↓
/openspec-pipeline "实现用户认证"
    ↓
自动编排: explore → propose → apply → review → archive
    ↓
阶段门禁自动检查
    ↓
输出完成报告
```

---

## 2. 方案 L：流水线 Skill

### 2.1 设计理念

**核心理念**: 一次输入，全程自动化，门禁护航。

**关键特性**:
- 单一入口，减少用户认知负担
- 自动阶段转换，保持上下文连续性
- 门禁检查，确保质量
- 断点续传，支持暂停恢复

### 2.2 Skill 定义

**文件路径**: `plugin/skills/openspec-pipeline/SKILL.md`

```markdown
---
name: openspec-pipeline
description: End-to-end OpenSpec workflow automation
tools: [Read, Write, Edit, Bash, Agent, Glob, Grep]
---

You are the OpenSpec Pipeline Orchestrator. Execute the full SDD+TDD workflow from a single requirement input.

## Input

User provides a requirement description:
- "实现用户认证功能"
- "Add API rate limiting"
- "Fix the database connection pooling issue"

## Pipeline Stages

### Stage 1: Explore (Interactive)

1. Launch explore mode analysis
2. Ask clarifying questions about:
   - Scope boundaries
   - Technical constraints
   - Existing code impact
3. Confirm understanding before proceeding

**Gate**: User confirms understanding

### Stage 2: Propose

1. Generate proposal.md with:
   - What/Why/Scope sections
   - Technical approach
   - Affected components

2. Generate tasks.md:
   - Ordered implementation steps
   - Dependencies between tasks
   - Estimated complexity

3. Generate design.md (if required):
   - Architecture diagram
   - API contracts
   - Data model changes

**Gate**: User approves proposal

### Stage 3: Apply (TDD Enforced)

For each task:

1. **Pre-implementation**:
   - Check/generate test skeleton
   - Identify affected files

2. **Implementation**:
   - Write test cases first
   - Implement functionality
   - Run tests

3. **Post-implementation**:
   - Verify tests pass
   - Mark task complete

**Gate**: All tests pass for current task

### Stage 4: Review

1. Launch code-review Agent
2. Save review report
3. Handle findings:
   - ERROR → Block, require fix
   - WARN → Log, optionally fix
   - INFO → Log only

**Gate**: Review verdict = PASS

### Stage 5: Archive

1. Run compliance check
2. Verify all gates passed:
   - ✅ All tasks complete
   - ✅ Tests pass
   - ✅ Review passed
   - ✅ No uncommitted changes

**Gate**: Compliance check passed

## State Management

Save pipeline state to `.openspec/pipeline-state.json`:

```json
{
  "change_name": "add-user-auth",
  "current_stage": "apply",
  "current_task": 3,
  "total_tasks": 7,
  "stage_status": {
    "explore": "completed",
    "propose": "completed",
    "apply": "in_progress",
    "review": "pending",
    "archive": "pending"
  },
  "gates_passed": ["explore", "propose"],
  "started_at": "2026-04-30T10:00:00Z",
  "updated_at": "2026-04-30T14:30:00Z"
}
```

## Pause and Resume

At any point, the pipeline can:
- **Pause**: Save current state, exit
- **Resume**: Read state, continue from last checkpoint

User commands:
- `/openspec-pipeline --pause`
- `/openspec-pipeline --resume`
- `/openspec-pipeline --status`

## Error Handling

| Stage | Error | Recovery |
|-------|-------|----------|
| Explore | Ambiguous requirements | Ask clarifying questions |
| Propose | User rejects | Iterate on proposal |
| Apply | Test failure | Block, show errors, offer fix options |
| Review | ERROR found | Block, offer fix or override |
| Archive | Compliance fail | Block, show missing items |

## Output

At completion, generate summary:

```markdown
# Pipeline Completion Report

> Change: add-user-auth
> Started: 2026-04-30 10:00
> Completed: 2026-04-30 16:30
> Duration: 6h 30m

## Stages Completed

1. ✅ Explore (30min) - Clarified OAuth providers
2. ✅ Propose (45min) - Generated proposal + tasks + design
3. ✅ Apply (4h) - 7 tasks implemented with TDD
4. ✅ Review (30min) - 1 WARN fixed, verdict: PASS
5. ✅ Archive (15min) - All gates passed

## Artifacts

- `openspec/changes/add-user-auth/proposal.md`
- `openspec/changes/add-user-auth/design.md`
- `openspec/changes/add-user-auth/tasks.md`
- `openspec/changes/add-user-auth/test-reports/`
- Commit: `abc1234 feat(auth): add user authentication`

## Metrics

- Tasks: 7/7 complete
- Tests: 15 passing, 0 failing
- Coverage: 87%
- Review: PASS

## Next Steps

- Consider adding OAuth Apple (mentioned but out of scope)
- Monitor JWT token refresh in production
```
```

### 2.3 流程图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        openspec-pipeline                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│   │ Explore │───▶│ Propose │───▶│  Apply  │───▶│ Review  │───▶│ Archive │
│   └────┬────┘    └────┬────┘    └────┬────┘    └────┬────┘    └────┬────┘
│        │              │              │              │              │
│        ▼              ▼              ▼              ▼              ▼
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│   │ Gate:   │    │ Gate:   │    │ Gate:   │    │ Gate:   │    │ Gate:   │
│   │ Confirm │    │ Approve │    │Test Pass│    │ PASS    │    │Comply   │
│   └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                      State: pipeline-state.json              │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │               Error Recovery: Pause → Resume                  │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. 方案 M：阶段门禁定义

### 3.1 门禁清单

| 阶段 | 门禁名称 | 检查内容 | 失败处理 |
|------|----------|----------|----------|
| Explore | Understanding Confirm | Claude 已提问，用户已确认 | 循环提问直到确认 |
| Propose | Proposal Approved | proposal.md 完整，用户已批准 | 迭代修改 proposal |
| Apply (per task) | Test Pass | 测试文件存在且通过 | 阻断，显示错误 |
| Apply (all tasks) | All Complete | tasks.md 全部勾选 | 阻断，显示未完成项 |
| Review | Review Pass | 最新 review verdict = PASS | 阻断，显示 ERROR |
| Archive | Compliance Pass | C1-C8 检查全部通过 | 阻断，显示失败项 |

### 3.2 门禁状态记录

```json
{
  "gates": {
    "explore_confirm": {
      "status": "passed",
      "passed_at": "2026-04-30T10:30:00Z",
      "notes": "User confirmed OAuth providers: Google, GitHub"
    },
    "propose_approve": {
      "status": "passed",
      "passed_at": "2026-04-30T11:15:00Z",
      "notes": "User approved v2 of proposal"
    },
    "apply_task_1_test": {
      "status": "passed",
      "passed_at": "2026-04-30T12:00:00Z"
    },
    "apply_task_2_test": {
      "status": "passed",
      "passed_at": "2026-04-30T12:45:00Z"
    },
    "apply_all_complete": {
      "status": "passed",
      "passed_at": "2026-04-30T15:30:00Z"
    },
    "review_pass": {
      "status": "passed",
      "passed_at": "2026-04-30T16:00:00Z",
      "review_file": "test-reports/code-review-20260430-155500.md"
    },
    "archive_compliance": {
      "status": "passed",
      "passed_at": "2026-04-30T16:15:00Z"
    }
  }
}
```

### 3.3 门禁检查函数

```python
def check_gate(gate_name: str, context: dict) -> dict:
    """
    Check if a gate passes.

    Returns:
        {
            "passed": bool,
            "message": str,
            "details": dict
        }
    """
    gates = {
        "explore_confirm": check_explore_confirm,
        "propose_approve": check_propose_approve,
        "apply_task_test": check_apply_task_test,
        "apply_all_complete": check_apply_all_complete,
        "review_pass": check_review_pass,
        "archive_compliance": check_archive_compliance
    }

    checker = gates.get(gate_name)
    if not checker:
        return {"passed": False, "message": f"Unknown gate: {gate_name}"}

    return checker(context)


def check_apply_task_test(context: dict) -> dict:
    """Check if tests pass for current task."""
    change_dir = context["change_dir"]
    task_id = context["task_id"]

    # Run tests
    result = subprocess.run(
        ["npm", "test", "--", "--testNamePattern=Task{task_id}"],
        capture_output=True,
        text=True
    )

    if result.returncode == 0:
        return {
            "passed": True,
            "message": f"Tests passed for task {task_id}",
            "details": {"output": result.stdout}
        }
    else:
        return {
            "passed": False,
            "message": f"Tests failed for task {task_id}",
            "details": {"output": result.stderr}
        }
```

---

## 4. 方案 N：CI 集成 Hook

### 4.1 目标

将 OpenSpec 流程与 CI/CD 管道集成：

- PR 创建时运行 spec compliance check
- Merge 前运行 review gate
- Deploy 前运行 archive validation

### 4.2 CI 集成点

```
┌─────────────────────────────────────────────────────────────────┐
│                        GitHub PR Workflow                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   PR Created                                                     │
│       │                                                          │
│       ▼                                                          │
│   ┌─────────────┐                                               │
│   │ OpenSpec    │ ◀── .github/workflows/openspec-pr.yml         │
│   │ PR Check    │                                               │
│   └─────┬───────┘                                               │
│         │                                                        │
│         ▼                                                        │
│   ┌─────────────────────────────────────────────────┐           │
│   │ Checks:                                          │           │
│   │ - proposal.md exists                            │           │
│   │ - tasks.md exists                               │           │
│   │ - design.md exists (if requireDesign)           │           │
│   │ - All tasks complete                            │           │
│   │ - Tests pass                                    │           │
│   │ - Review report exists with PASS verdict        │           │
│   │ - Spec compliance check passed                  │           │
│   └─────────────────────────────────────────────────┘           │
│         │                                                        │
│         ▼                                                        │
│   ┌─────────────┐                                               │
│   │ Merge Gate  │ ◀── Required status check                     │
│   └─────────────┘                                               │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 GitHub Actions Workflow

**文件**: `.github/workflows/openspec-pr.yml`

```yaml
name: OpenSpec PR Check

on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  openspec-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Detect OpenSpec Change
        id: detect
        run: |
          # Find active change from PR branch
          CHANGE=$(ls -d openspec/changes/*/ 2>/dev/null | head -1)
          if [ -n "$CHANGE" ]; then
            echo "change_dir=$CHANGE" >> $GITHUB_OUTPUT
            echo "change_name=$(basename $CHANGE)" >> $GITHUB_OUTPUT
          else
            echo "No active change found"
            exit 0
          fi

      - name: Check Proposal Exists
        if: steps.detect.outputs.change_dir
        run: |
          test -f "${{ steps.detect.outputs.change_dir }}proposal.md"

      - name: Check Tasks Exists
        if: steps.detect.outputs.change_dir
        run: |
          test -f "${{ steps.detect.outputs.change_dir }}tasks.md"

      - name: Check All Tasks Complete
        if: steps.detect.outputs.change_dir
        run: |
          TASKS="${{ steps.detect.outputs.change_dir }}tasks.md"
          PENDING=$(grep -c '^\s*- \[ \]' "$TASKS" || echo 0)
          if [ "$PENDING" -gt 0 ]; then
            echo "ERROR: $PENDING tasks not complete"
            exit 1
          fi

      - name: Run Tests
        if: steps.detect.outputs.change_dir
        run: |
          npm ci
          npm test

      - name: Check Review Passed
        if: steps.detect.outputs.change_dir
        run: |
          REPORTS="${{ steps.detect.outputs.change_dir }}test-reports"
          LATEST_REVIEW=$(ls -t $REPORTS/code-review-*.md 2>/dev/null | head -1)
          if [ -z "$LATEST_REVIEW" ]; then
            echo "ERROR: No review report found"
            exit 1
          fi
          if ! grep -q "Verdict:.*PASS" "$LATEST_REVIEW"; then
            echo "ERROR: Latest review did not pass"
            exit 1
          fi

      - name: Spec Compliance Check
        if: steps.detect.outputs.change_dir
        run: |
          # Run spec compliance analysis
          python plugin/utils/spec-compliance.py \
            --change "${{ steps.detect.outputs.change_name }}" \
            --fail-on-missing

  update-status:
    needs: openspec-check
    if: always()
    runs-on: ubuntu-latest
    steps:
      - name: Update PR Status
        uses: actions/github-script@v7
        with:
          script: |
            const status = '${{ needs.openspec-check.result }}' === 'success' ? 'success' : 'failure';
            await github.rest.repos.createCommitStatus({
              owner: context.repo.owner,
              repo: context.repo.repo,
              sha: context.payload.pull_request.head.sha,
              state: status,
              context: 'openspec/compliance',
              description: status === 'success' ? 'All OpenSpec checks passed' : 'OpenSpec checks failed'
            });
```

### 4.4 Branch Protection 配置

```bash
# 使用 gh CLI 配置分支保护
gh api repos/{owner}/{repo}/branches/main/protection \
  --method PUT \
  -f required_status_checks='{"strict":true,"contexts":["openspec/compliance"]}' \
  -f enforce_admins=false \
  -f required_pull_request_reviews='{"dismiss_stale_reviews":true,"require_code_owner_reviews":false}'
```

---

## 5. 与其他方案的集成

```
Pipeline Skill (本方案 L)
    │
    ├──依赖──▶ 方案 D (TDD 门禁) ──▶ Stage 3 内嵌测试运行
    │
    ├──依赖──▶ 方案 F (自动 Review) ──▶ Stage 4 自动触发
    │
    ├──依赖──▶ 方案 K (合规检查) ──▶ Stage 5 门禁
    │
    └──输出──▶ CI 集成 (方案 N) ──▶ PR 自动检查
```

---

## 6. 文件改造清单

### 6.1 需要新增的文件

| 文件 | 用途 |
|------|------|
| `plugin/skills/openspec-pipeline/SKILL.md` | 流水线主 skill |
| `plugin/skills/openspec-pipeline/state.py` | 状态管理工具 |
| `plugin/skills/openspec-pipeline/gates.py` | 门禁检查函数 |
| `.github/workflows/openspec-pr.yml` | CI 检查工作流 |

### 6.2 需要修改的文件

| 文件 | 改动 |
|------|------|
| `plugin/commands/opsx/` | 增加 `opsx:pipeline` 快捷命令 |
| `plugin/hooks/on-user-prompt.py` | 检测 pipeline 状态并显示进度 |

---

## 7. 用户使用示例

### 7.1 启动新流程

```
User: /openspec-pipeline "实现API限流功能，防止滥用"

Claude: Starting OpenSpec Pipeline for "API Rate Limiting"...

Stage 1: Explore
─────────────────
Analyzing codebase for rate limiting integration points...

Questions:
1. 应该限制什么粒度的请求？
   - IP 级别
   - 用户级别
   - API endpoint 级别

2. 限流策略偏好？
   - 固定窗口 (simple)
   - 滑动窗口 (smooth)
   - 令牌桶 (flexible)

User: 用户级别，滑动窗口

Claude: Understood. Proceeding to Stage 2...
```

### 7.2 查看进度

```
User: /openspec-pipeline --status

Claude: Pipeline Status: add-rate-limiting

┌──────────────────────────────────────────────┐
│ Stage        │ Status    │ Progress          │
├──────────────────────────────────────────────┤
│ Explore      │ ✅ Done   │ 30min ago         │
│ Propose      │ ✅ Done   │ 15min ago         │
│ Apply        │ 🔄 Running│ Task 4/7          │
│ Review       │ ⏳ Pending│ -                 │
│ Archive      │ ⏳ Pending│ -                 │
└──────────────────────────────────────────────┘

Current Task: Implement Redis-backed rate counter
Tests: 12 passing, 0 failing

Elapsed: 2h 15m
```

### 7.3 暂停恢复

```
User: /openspec-pipeline --pause

Claude: Pipeline paused. State saved to:
.openspec/pipeline-state.json

Resume anytime with: /openspec-pipeline --resume

---

User: /openspec-pipeline --resume

Claude: Resuming pipeline for "add-rate-limiting"...

Last checkpoint: Stage 3 (Apply), Task 4/7
Current task: Implement Redis-backed rate counter

Continuing from where we left off...
```

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 流水线中断丢失上下文 | 需要重新开始 | 状态持久化 + 断点续传 |
| 门禁过严导致卡顿 | 开发效率降低 | 可配置门禁强度 |
| CI 配置复杂 | 维护成本高 | 提供模板和自动化脚本 |
| 单一 skill 过于庞大 | 性能问题 | 分阶段加载，按需触发 |

---

## 9. 验收标准

- [ ] 单一入口 `/openspec-pipeline` 触发完整流程
- [ ] 5 个阶段按序执行并自动转换
- [ ] 每个阶段有门禁检查，失败时阻断
- [ ] 支持暂停/恢复/状态查询
- [ ] 输出完整的 Pipeline Completion Report
- [ ] CI 集成：PR 创建时自动运行检查
- [ ] 分支保护：OpenSpec 检查通过才允许合并