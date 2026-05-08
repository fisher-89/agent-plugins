# Design: Slim Plugin with Report-Driven Gates

> **Change**: slim-plugin-report-gates
> **Date**: 2026-05-08

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        新架构：瘦插件 + 报告驱动                             │
└─────────────────────────────────────────────────────────────────────────────┘

plugin/
├── skills/
│   └── code-review/           (保留：这是插件自己的 skill)
│
├── hooks/
│   ├── hooks.json
│   ├── on-user-prompt.py      (不变)
│   ├── pre-tool-openspec-test.py   (不变：TDD GATE)
│   ├── pre-tool-commit-review.py   (增强：报告链检查)
│   ├── pre-tool-skill.py           (增强：archive 报告链检查 + ERROR→Task)
│   └── session-start-ensure-openspec.py  (重写：检测+询问+安装)
│
├── utils/
│   ├── active-change.py       (已有)
│   ├── test-scope.py          (增强：--save-report)
│   ├── test-generator.py      (增强：--save-report)
│   ├── lint-runner.py         (增强：--save-report)
│   ├── test-runner.py         (增强：--save-report)
│   ├── compliance-check.py    (已有)
│   ├── review-parser.py       (已有)
│   └── review-loop-state.py   (已有)
│
└── templates/
    └── step-report.json       (新增：报告模板)

openspec/changes/<name>/
├── reports/                   (新增：报告目录)
│   ├── task-1_scope.json
│   ├── task-1_lint.json
│   ├── ...
│   ├── full-test.json
│   └── code-review.json
│
├── proposal.md
├── design.md
├── tasks.md
└── ...
```

---

## Component 1: Delete Embedded Skills

### 删除目录

```
删除：
  plugin/skills/openspec-explore/
  plugin/skills/openspec-propose/
  plugin/skills/openspec-apply-change/
  plugin/skills/openspec-archive-change/

保留：
  plugin/skills/code-review/    (插件自己的 skill)
```

### 重写 SessionStart Hook

**删除**：`session-start-sync-skills.py` 的 skill 同步逻辑

**新建**：`session-start-ensure-openspec.py`

```python
def main():
    # Hook mode: check and optionally install
    if check_openspec_installed():
        output_result("")  # 已安装，无额外 context
        return

    # 未安装 — 注入询问 context
    context = (
        "OpenSpec CLI is not installed. "
        "This plugin requires openspec for SDD workflow. "
        "Install now? Run: npm install -g openspec-cli"
    )
    output_result(context)


def check_openspec_installed():
    """Check if openspec CLI is available."""
    return shutil.which("openspec") is not None


def output_result(additional_context):
    result = {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }
    json.dump(result, sys.stdout)
```

**关键变化**：
- 不再自动安装（改为提示用户）
- 不再同步 skill（删除整个 sync 逻辑）

---

## Component 2: Report Generation

### 报告格式

```json
{
  "schema": "openspec-step-report/v1",
  "change": "slim-plugin-report-gates",
  "task_id": "3",
  "step": "scope",
  "status": "pass",
  "timestamp": "2026-05-08T17:45:00.123Z",
  "duration_ms": 150,
  "details": {
    "affected_files": ["src/auth.js"],
    "test_files": ["src/auth.test.js"],
    "patterns": ["login", "validate"]
  }
}
```

### 报告目录结构

```
openspec/changes/<name>/reports/
├── task-1_scope.json
├── task-1_skeleton.json
├── task-1_lint.json
├── task-1_scoped-test.json
├── task-2_scope.json
├── task-2_lint.json
├── task-2_scoped-test.json
├── ...
├── full-test.json          (所有 task 完成后)
└── code-review.json        (full-test 通过后)
```

### Utils 增强

#### test-scope.py

```python
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--task", required=True)
    parser.add_argument("--project-root", default=".")
    parser.add_argument("--change", required=True)
    parser.add_argument("--save-report", action="store_true")
    args = parser.parse_args()

    scope = identify_test_scope(args.task, args.project_root)

    if args.save_report:
        save_step_report(
            change=args.change,
            task_id=extract_task_id(args.task),
            step="scope",
            status="pass",
            details={
                "affected_files": scope.affected_files,
                "test_files": scope.test_files,
                "missing_test_files": scope.missing_test_files,
                "patterns": scope.test_patterns,
            },
            project_root=args.project_root,
        )

    # 输出到 stdout 供 Claude 读取
    print(json.dumps(asdict(scope)))


def save_step_report(change, task_id, step, status, details, project_root):
    """Save step report to reports directory."""
    reports_dir = os.path.join(
        project_root, "openspec", "changes", change, "reports"
    )
    os.makedirs(reports_dir, exist_ok=True)

    report = {
        "schema": "openspec-step-report/v1",
        "change": change,
        "task_id": task_id,
        "step": step,
        "status": status,
        "timestamp": datetime.utcnow().isoformat() + "Z",
        "details": details,
    }

    filename = f"task-{task_id}_{step}.json"
    path = os.path.join(reports_dir, filename)

    with open(path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
```

类似增强应用于：
- `test-generator.py` → step="skeleton"
- `lint-runner.py` → step="lint"
- `test-runner.py` → step="scoped-test" 或 step="full-test"
- `review-parser.py` → 已有报告输出，对齐格式

---

## Component 3: Hook Report Chain Check

### 报告链定义

每个 task 的标准报告链：

```
scope → skeleton → lint → scoped-test
  │         │        │         │
  └─────────┴────────┴─────────┘
              │
              ▼
        timestamp 严格递增
```

全量流程的报告链：

```
full-test → code-review
     │           │
     └───────────┘
          │
          ▼
    full-test.ts < review.ts
```

### pre-tool-commit-review.py 增强

```python
def check_report_chain(change_dir, task_id):
    """Check if report chain is complete for a task."""
    reports_dir = os.path.join(change_dir, "reports")
    if not os.path.isdir(reports_dir):
        return False, "No reports directory found"

    required_steps = ["scope", "lint", "scoped-test"]
    reports = []

    for step in required_steps:
        path = os.path.join(reports_dir, f"task-{task_id}_{step}.json")
        if not os.path.isfile(path):
            return False, f"Missing report: {step}"
        try:
            with open(path, "r", encoding="utf-8") as f:
                report = json.load(f)
            reports.append(report)
        except (json.JSONDecodeError, OSError):
            return False, f"Invalid report: {step}"

    # 检查时间戳顺序
    timestamps = [r["timestamp"] for r in reports]
    if timestamps != sorted(timestamps):
        return False, "Report timestamps out of order"

    return True, None


def handle_commit(cwd, change_name, change_dir):
    # ... 现有逻辑 ...

    # 新增：报告链检查
    current_task = get_current_task(change_dir)
    if current_task:
        ok, err = check_report_chain(change_dir, current_task)
        if not ok:
            context = (
                f"REPORT CHAIN INCOMPLETE for task {current_task}: {err}. "
                f"Ensure you executed: test-scope → lint → scoped-test with --save-report."
            )
            output_result("deny", context)
            return

    # ... 继续 lint/test/review 检查 ...
```

### pre-tool-skill.py 增强

#### archive 检查：所有 task 报告链 + full-test + review

```python
def handle_archive_skill(cwd):
    # ... 现有合规检查 ...

    # 新增：报告链完整性检查
    all_ok, issues = check_all_report_chains(change_dir, tasks_md)
    if not all_ok:
        context = (
            f"ARCHIVE BLOCKED: Report chain incomplete.\n"
            f"Issues:\n" + "\n".join(f"  - {i}" for i in issues) + "\n"
            f"Ensure all tasks have complete report chains before archiving."
        )
        output_result("deny", context)
        return

    # 检查 full-test 和 code-review 报告
    ok, err = check_final_reports(change_dir)
    if not ok:
        output_result("deny", f"ARCHIVE BLOCKED: {err}")
        return

    output_result("allow", "All report chains complete.")


def check_all_report_chains(change_dir, tasks_md):
    """Check report chains for all tasks."""
    issues = []
    tasks = parse_tasks(tasks_md)

    for task_id, task_desc in tasks:
        ok, err = check_report_chain(change_dir, task_id)
        if not ok:
            issues.append(f"Task {task_id}: {err}")

    return len(issues) == 0, issues


def check_final_reports(change_dir):
    """Check full-test and code-review reports exist and are ordered."""
    reports_dir = os.path.join(change_dir, "reports")

    full_test_path = os.path.join(reports_dir, "full-test.json")
    review_path = os.path.join(reports_dir, "code-review.json")

    if not os.path.isfile(full_test_path):
        return False, "Missing full-test.json"
    if not os.path.isfile(review_path):
        return False, "Missing code-review.json"

    try:
        with open(full_test_path, "r") as f:
            full_test = json.load(f)
        with open(review_path, "r") as f:
            review = json.load(f)

        if full_test["timestamp"] >= review["timestamp"]:
            return False, "full-test must run before code-review"

        if full_test["status"] != "pass":
            return False, "full-test did not pass"

        if review["status"] == "fail":
            return False, "code-review has errors"

    except (json.JSONDecodeError, OSError, KeyError) as e:
        return False, f"Invalid report file: {e}"

    return True, None
```

---

## Component 4: ERROR → Task → Re-enter Apply

### 继承自 fix-hook-architecture Phase 5

**pre-tool-commit-review.py** 检测 review BLOCK 时：

```python
def handle_review_block(change_name, change_dir, report_path, error_count):
    """Inject ERROR→Task instruction on review BLOCK."""
    tasks_md_path = os.path.join(change_dir, "tasks.md")

    context = (
        f"REVIEW BLOCK: {error_count} error(s) found in '{change_name}'.\n"
        f"BEFORE committing, you MUST:\n"
        f"1. Generate fix tasks:\n"
        f"   python plugin/utils/review-parser.py {report_path} "
        f"     --generate-tasks --append-to {tasks_md_path}\n"
        f"2. Record loop state:\n"
        f"   python plugin/utils/review-loop-state.py {change_dir} "
        f"     --record {error_count} {error_count}\n"
        f"3. Re-enter apply: invoke the openspec-apply-change skill "
        f"   to implement the fix tasks.\n"
        f"This ensures all review errors are addressed before commit."
    )
    output_result("allow", context)
```

**pre-tool-skill.py** apply 检查：

```python
def handle_apply_skill(cwd):
    # ... 现有逻辑 ...

    # 检查 review loop state
    state_path = os.path.join(change_dir, "review-loop-state.json")
    if os.path.isfile(state_path):
        try:
            with open(state_path, "r") as f:
                state = json.load(f)

            status = state.get("status", "idle")
            loop_count = state.get("loop_count", 0)
            max_loops = state.get("max_loops", 3)
            current_errors = state.get("current_errors", 0)

            if status == "paused":
                context = (
                    f"Review loop PAUSED for '{change_name}'. "
                    f"Loop {loop_count}/{max_loops} reached. "
                    f"Manual intervention required."
                )
                output_result("allow", context)
                return

            if status == "running" and current_errors > 0:
                context = (
                    f"Review loop active for '{change_name}': "
                    f"Loop {loop_count}/{max_loops}, {current_errors} error(s) remaining. "
                    f"Fix tasks have been appended to tasks.md. "
                    f"Implement the fix tasks before proceeding."
                )
                output_result("allow", context)
                return

        except (json.JSONDecodeError, OSError):
            pass

    output_result("allow", "")
```

---

## Component 5: Updated hooks.json

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/on-user-prompt.py"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/pre-tool-openspec-test.py"
          }
        ]
      },
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/pre-tool-commit-review.py"
          }
        ]
      },
      {
        "matcher": "Skill",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/pre-tool-skill.py"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/session-start-ensure-openspec.py"
          },
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/session-start-worktree.py"
          }
        ]
      }
    ]
  }
}
```

---

## Tasks Reorganization

原 fix-hook-architecture Phase 5 的 task + 新 task 合并：

```
Phase 1: 删除内嵌 SKILL
  1. 删除 plugin/skills/openspec-*/ 目录
  2. 创建 session-start-ensure-openspec.py
  3. 更新 hooks.json SessionStart 配置
  4. 删除 session-start-sync-skills.py

Phase 2: 报告生成能力
  5. 创建报告模板 plugin/templates/step-report.json
  6. 增强 test-scope.py 添加 --save-report
  7. 增强 test-generator.py 添加 --save-report
  8. 增强 lint-runner.py 添加 --save-report
  9. 增强 test-runner.py 添加 --save-report

Phase 3: Hook 报告链检查
  10. 增强 pre-tool-commit-review.py 添加报告链检查
  11. 增强 pre-tool-skill.py archive 检查所有报告链
  12. 创建报告链检查工具函数

Phase 4: ERROR → Task 闭环 (继承自 fix-hook-architecture Phase 5)
  13. 增强 pre-tool-commit-review.py 注入 ERROR→Task 指令
  14. 增强 pre-tool-skill.py handle_apply_skill() 检查 loop state
  15. 创建 ERROR→Task 流程集成测试

Phase 5: 文档更新
  16. 更新 improvement.md
  17. 更新 CLAUDE.md
  18. 更新 README.md
```

---

## Testing Strategy

### Unit Tests

| 组件 | 测试场景 |
|------|----------|
| `save_step_report()` | 报告文件正确生成 |
| `check_report_chain()` | 完整链 → True |
| `check_report_chain()` | 缺失报告 → False + error |
| `check_report_chain()` | 时间戳乱序 → False + error |
| `check_all_report_chains()` | 所有 task 链完整 → True |
| `check_final_reports()` | full-test + review 存在且有序 → True |

### Integration Tests

1. **删除 skill 后调用 /openspec-propose** → 使用 CLI 生成
2. **步骤无报告时 commit** → deny + 提示
3. **报告链完整时 commit** → allow
4. **archive 时检查所有报告** → deny 或 allow
5. **review BLOCK → ERROR→Task → 重入 apply** → fix task 生成
