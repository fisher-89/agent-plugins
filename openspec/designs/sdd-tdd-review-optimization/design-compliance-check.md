# 设计文档：Archive 合规检查

> 版本: 1.3
> 日期: 2026-05-08
> 涉及方案: K (合规检查) / G (Spec 合规审查)
>
> **实现状态**: C1-C10 全部实现（C1-C8 强制阻断，C9-C10 配置驱动）

## 1. 问题定义

### 1.1 当前状态

```
apply-change 所有 task 完成
    ↓
auto-commit
    ↓
archive-change（无校验）
    ↓
同步 spec 到 main，移动到 archive/
```

**缺失的校验**:
- proposal scope 是否全部实现？
- 所有 task 是否真正完成？
- 测试是否通过？
- code review 是否通过？
- 是否有未提交的更改？

### 1.2 目标状态

```
apply-change 所有 task 完成
    ↓
运行合规检查 (pre-archive-check)
    ↓
┌─ 全部通过 → archive
└─ 有未通过项 → 阻断，报告缺失项
    ↓
archive（同步 + 移动）
```

---

## 2. 合规清单定义

### 2.1 检查项

| ID | 检查项 | 级别 | 说明 | 状态 |
|----|--------|------|------|------|
| C1 | proposal.md 存在 | 必须 | 规范文档存在 | ✅ 已实现 |
| C2 | tasks.md 存在 | 必须 | 任务清单存在 | ✅ 已实现 |
| C3 | 所有 task 已勾选 | 必须 | 100% 完成率 | ✅ 已实现 |
| C4 | 测试文件存在 | 必须 | 至少一个测试文件 | ✅ 已实现 |
| C5 | 测试全部通过 | 必须 | 检查 test-reports 测试结果记录 | ✅ 已实现 |
| C6 | code review 存在 | 必须 | 有 review 报告 | ✅ 已实现 |
| C7 | code review 无 ERROR | 必须 | 最新 review 为 PASS | ✅ 已实现 |
| C8 | 无 uncommitted changes | 必须 | `git status` 干净 | ✅ 已实现 |
| C9 | design.md 存在 | 可选 | 配置驱动 | ✅ 已实现 |
| C10 | spec 合规审查 | 可选 | 实现覆盖 proposal scope | ✅ 已实现 |

**当前约束级别**: 硬约束（C1-C8 阻断 Archive，C9-C10 配置驱动）

### 2.2 检查结果输出

```markdown
# Pre-Archive Compliance Check

> **Change**: <change-name>
> **Date**: <timestamp>
> **Result**: PASS / FAIL

---

## Checklist

| ID | Check | Status | Details |
|----|-------|--------|---------|
| C1 | proposal.md exists | ✅ PASS | - |
| C2 | tasks.md exists | ✅ PASS | - |
| C3 | All tasks complete | ✅ PASS | 10/10 tasks |
| C4 | Test files exist | ✅ PASS | 3 test files |
| C5 | Tests pass | ❌ FAIL | 2 tests failing |
| C6 | Code review exists | ✅ PASS | 2 review reports |
| C7 | Review no ERROR | ✅ PASS | Latest: PASS |
| C8 | No uncommitted | ✅ PASS | Working tree clean |
| C9 | design.md exists | ⚠️ SKIP | Not configured |
| C10 | Spec compliance | ❌ FAIL | Missing: OAuth Google |

---

## Summary

- **Passed**: 7
- **Failed**: 2
- **Skipped**: 1

## Blocking Issues

1. **[C5] Tests failing**: Run `npm test` and fix failures
   - tests/auth.test.js:45 - "should validate email format"
   - tests/auth.test.js:67 - "should hash passwords"

2. **[C10] Spec not fully implemented**:
   - Missing: OAuth Google login (mentioned in proposal scope)
   - Missing: OAuth GitHub login (mentioned in proposal scope)

## Verdict

**FAIL** — Fix 2 blocking issues before archiving.
```

---

## 3. 方案 K：Archive 前合规检查

### 3.1 触发时机

在 `openspec-archive-change` SKILL.md 的 Step 1 之前插入：

```markdown
### Step 0: Pre-Archive Compliance Check

Before archiving, verify all requirements are met:

1. **Run compliance check script**
   ```bash
   openspec compliance --change "<name>" --json
   ```

2. **Parse JSON result**
   - `passed`: bool — overall pass/fail
   - `checks`: array — individual check results
   - `blocking_issues`: array — issues that must be fixed

3. **Handle result**

   If `passed: false`:
   - Display blocking issues
   - Offer: fix issues / force archive (with warning)
   - Default: DO NOT proceed to archive

   If `passed: true`:
   - Display summary: "All compliance checks passed"
   - Proceed to Step 1
```

### 3.2 合规检查实现

**新增 CLI 命令** (如果 openspec CLI 支持):

```bash
openspec compliance --change "add-user-auth" --json
```

**或在 Skill 内部实现**:

```markdown
### Compliance Check Implementation

Run each check in sequence:

**C1: proposal.md exists**
- Check: `test -f openspec/changes/<name>/proposal.md`
- Result: exists / missing

**C2: tasks.md exists**
- Check: `test -f openspec/changes/<name>/tasks.md`
- Result: exists / missing

**C3: All tasks complete**
- Parse tasks.md, count `- [ ]` vs `- [x]`
- Result: N/M complete (FAIL if N < M)

**C4: Test files exist**
- Glob: `tests/**/*.{test,spec}.{js,ts,py}`
- Or: `openspec/changes/<name>/test-reports/*.md`
- Result: count of test files

**C5: Tests pass**
- Run: `npm test 2>&1` or `pytest --tb=short`
- Parse exit code
- Result: exit_code / failing_tests

**C6: Code review exists**
- Glob: `openspec/changes/<name>/test-reports/code-review-*.md`
- Result: exists / missing

**C7: Review no ERROR**
- Parse latest review report
- Look for "Verdict: PASS"
- Result: PASS / BLOCK / missing

**C8: No uncommitted**
- Run: `git status --porcelain`
- Result: clean / dirty_files

**C9: design.md exists** (optional)
- Check `.openspec.yaml` for `requireDesign: true`
- If required: check `test -f openspec/changes/<name>/design.md`

**C10: Spec compliance** (optional)
- See 方案 G below
```

### 3.3 合规报告持久化

保存到 `openspec/changes/<name>/test-reports/compliance-<timestamp>.md`

---

## 4. 方案 G：Spec 合规审查

### 4.1 目标

对比 proposal.md 定义的 Scope 与实际实现，发现：

- **Missing**: proposal 提到但未实现的功能
- **Extra**: 实现了但 proposal 未提到的功能
- **Mismatch**: 实现与 proposal 描述不一致

### 4.2 实现逻辑

```
┌─────────────────────────────────────────────────────────┐
│              Spec Compliance Analysis                    │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. Parse proposal.md                                   │
│     - Extract "## Scope" section                        │
│     - Parse feature list (bullet points)                │
│     - Extract API mentions (HTTP methods + paths)       │
│     - Extract data model mentions (entity names)         │
│                                                          │
│  2. Scan implementation                                 │
│     - Parse route definitions (Express routes)          │
│     - Parse API handlers                                 │
│     - Parse database schemas                             │
│     - Build feature inventory from code                 │
│                                                          │
│  3. Compare                                              │
│     - Missing features (in proposal, not in code)       │
│     - Extra features (in code, not in proposal)          │
│                                                          │
│  4. Generate report                                      │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### 4.3 Proposal 解析

**示例 proposal.md Scope 部分**:

```markdown
## Scope

This change implements user authentication with the following features:

- Email/password registration
- Email/password login
- OAuth providers: Google, GitHub
- JWT session management
- Redis-based session caching
- Role-based access control (RBAC)
```

**解析输出**:
```json
{
  "features": [
    "Email/password registration",
    "Email/password login",
    "OAuth providers: Google, GitHub",
    "JWT session management",
    "Redis-based session caching",
    "Role-based access control (RBAC)"
  ],
  "api_mentions": [],
  "data_models": []
}
```

### 4.4 代码扫描

**Route 提取 (Express)**:

```python
def extract_express_routes(src_dir: str) -> list[dict]:
    """Extract route definitions from Express app."""
    routes = []

    for file in glob.glob(f"{src_dir}/**/*.js", recursive=True):
        content = open(file).read()

        # Match: router.get('/path', ...), router.post('/path', ...)
        pattern = r'(?:router|app)\.(get|post|put|delete|patch)\s*\(\s*[\'"]([^\'"]+)[\'"]'
        matches = re.findall(pattern, content)

        for method, path in matches:
            routes.append({
                "method": method.upper(),
                "path": path,
                "file": file
            })

    return routes
```

**API Handler 提取**:

```python
def extract_api_handlers(src_dir: str) -> list[dict]:
    """Extract API handler functions."""
    handlers = []

    # Look for function definitions that handle requests
    for file in glob.glob(f"{src_dir}/routes/*.js"):
        content = open(file).read()

        # Match: async function name(req, res)
        pattern = r'(?:async\s+)?function\s+(\w+)\s*\(\s*req\s*,\s*res'
        matches = re.findall(pattern, content)

        for name in matches:
            handlers.append({
                "name": name,
                "file": file
            })

    return handlers
```

**Schema 提取 (SQL)**:

```python
def extract_sql_schemas(src_dir: str) -> list[str]:
    """Extract table names from SQL schema."""
    tables = []

    for file in glob.glob(f"{src_dir}/db/*.sql"):
        content = open(file).read()

        # Match: CREATE TABLE IF NOT EXISTS name
        pattern = r'CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)'
        matches = re.findall(pattern, content, re.IGNORECASE)

        tables.extend(matches)

    return tables
```

### 4.5 对比算法

```python
def compare_spec_impl(proposal_features: list[str],
                      impl_routes: list[dict],
                      impl_handlers: list[dict]) -> dict:
    """
    Compare proposal scope with implementation.

    Returns:
        {
            "covered": [...],
            "missing": [...],
            "extra": [...]
        }
    """
    covered = []
    missing = []
    extra = []

    # Build implementation feature map
    impl_features = set()

    # Map routes to features
    route_keywords = {
        "register": "registration",
        "login": "login",
        "logout": "logout",
        "oauth": "OAuth",
        "google": "Google",
        "github": "GitHub",
        "auth": "authentication"
    }

    for route in impl_routes:
        for keyword, feature in route_keywords.items():
            if keyword in route["path"].lower():
                impl_features.add(feature)

    # Check each proposal feature
    for feature in proposal_features:
        matched = False
        feature_lower = feature.lower()

        for impl in impl_features:
            if impl.lower() in feature_lower:
                matched = True
                break

        if matched:
            covered.append(feature)
        else:
            missing.append(feature)

    # Check for extra features
    for impl in impl_features:
        found = False
        for feature in proposal_features:
            if impl.lower() in feature.lower():
                found = True
                break
        if not found:
            extra.append(impl)

    return {
        "covered": covered,
        "missing": missing,
        "extra": extra
    }
```

### 4.6 报告输出

```markdown
# Spec Compliance Report

> **Change**: add-user-auth
> **Date**: 2026-04-30

---

## Proposal Scope

| # | Feature |
|---|---------|
| 1 | Email/password registration |
| 2 | Email/password login |
| 3 | OAuth providers: Google, GitHub |
| 4 | JWT session management |
| 5 | Redis-based session caching |
| 6 | Role-based access control (RBAC) |

---

## Implementation Inventory

### API Routes
| Method | Path | File |
|--------|------|------|
| POST | /auth/register | src/routes/auth.js |
| POST | /auth/login | src/routes/auth.js |
| POST | /auth/logout | src/routes/auth.js |
| GET | /auth/google | src/routes/auth.js |
| GET | /auth/google/callback | src/routes/auth.js |
| GET | /auth/github | src/routes/auth.js |
| GET | /auth/github/callback | src/routes/auth.js |

### Data Models
| Table | File |
|-------|------|
| users | src/db/schema.js |
| oauth_accounts | src/db/schema.js |

---

## Coverage Analysis

### ✅ Covered
- [x] Email/password registration → POST /auth/register
- [x] Email/password login → POST /auth/login
- [x] JWT session management → middleware/auth.js
- [x] Role-based access control → middleware/auth.js

### ❌ Missing
- [ ] OAuth providers: Google → Handler exists but missing tests
- [ ] OAuth providers: GitHub → Handler exists but missing tests
- [ ] Redis-based session caching → Not found in implementation

### ⚠️ Extra (Not in Proposal)
- [x] Logout endpoint → POST /auth/logout (added for completeness)

---

## Verdict

**Coverage**: 4/6 proposal features implemented (67%)
**Missing Items**: 3

⚠️ INCOMPLETE — Consider updating proposal or implementing missing features.
```

---

## 5. 文件改造清单

### 5.1 需要修改的文件

| 文件 | 改动 | 影响 |
|------|------|------|
| `plugin/skills/openspec-archive-change/SKILL.md` | 增加 Step 0 合规检查 | 核心流程 |

### 5.2 需要新增的文件

| 文件 | 用途 |
|------|------|
| `plugin/utils/compliance-check.py` | 合规检查脚本 |
| `plugin/utils/spec-compliance.py` | Spec 对比分析脚本 |
| `plugin/templates/compliance-report.md` | 合规报告模板 |

---

## 6. 与其他方案的集成

```
                  ┌─────────────────┐
                        │  apply-change   │
                        │  (方案 D/B)      │
                        └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │  auto-review    │
                        │  (方案 F)        │
                        └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │ compliance-check │
                        │ (本方案 K)        │
                        └────────┬────────┘
                                 │
                    ┌────────────┼────────────┐
                    │            │            │
                    ▼            ▼            ▼
              ┌──────────┐ ┌──────────┐ ┌──────────┐
              │ C1-C8    │ │ C9       │ │ C10      │
              │ 基础检查  │ │ design.md│ │ spec合规 │
              └──────────┘ └──────────┘ └──────────┘
                    │            │            │
                    └────────────┼────────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │ archive-change   │
                        └─────────────────┘
```

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Spec 解析不准确 | 误报缺失功能 | 启发式匹配 + 用户确认 |
| 额外功能报告噪音 | 干扰判断 | 标记为 INFO 级别，不阻断 |
| 合规检查耗时 | 延迟 archive | 并行执行各项检查 |
| 强制要求过严 | 开发体验差 | 允许 force archive (带警告) |

---

## 8. 验收标准

- [x] archive 前自动运行合规检查 (软约束)
- [x] C1-C4 检查已实现
- [ ] C5 测试运行验证（待实现）
- [x] C6-C8 检查已实现
- [x] C9 design.md 存在检查已实现
- [x] C10 Spec 合规审查（spec-compliance.py 已实现）
- [ ] 检查结果持久化到 test-reports/（待实现）
- [ ] 有 FAIL 项时阻断 archive (可选覆盖)（待实现）
- [x] Spec 合规分析至少覆盖 API 路由对比（已实现）
- [x] 报告清晰展示 covered/missing/extra（已实现）

---

## 9. 待实现：C5 测试运行验证

### 9.1 目标

Archive 前实际运行测试并验证通过，而非仅检查测试文件存在。

### 9.2 设计

```python
def check_c5_tests_pass(project_root: str) -> dict:
    """
    C5: Tests pass.

    Returns:
        {"passed": bool, "message": str, "details": dict}
    """
    # Detect framework
    framework, runner = detect_test_framework(project_root)
    if framework == "unknown":
        return {"passed": True, "message": "No test framework detected"}

    # Run tests
    result = run_tests(project_root, framework=framework)

    if result.success:
        return {
            "passed": True,
            "message": f"All {result.total} tests passed",
            "details": {"passed": result.passed, "total": result.total}
        }
    else:
        return {
            "passed": False,
            "message": f"{result.failed}/{result.total} tests failed",
            "details": {"output": result.full_output()}
        }
```

### 9.3 文件改造清单

| 文件 | 改动 |
|------|------|
| `plugin/utils/compliance-check.py` | 新增 `check_c5_tests_pass()` 函数 |
| `plugin/skills/openspec-archive-change/SKILL.md` | Step 0 增加 C5 检查 |

---

## 10. 待实现：C10 Spec 合规审查

### 10.1 目标

对比 proposal.md Scope 与实际实现，检测：
- Missing: proposal 提到但未实现的功能
- Extra: 实现了但 proposal 未提到的功能

### 10.2 设计

```python
def check_c10_spec_compliance(change_dir: str, src_dir: str) -> dict:
    """
    C10: Spec compliance.

    Returns:
        {
            "passed": bool,
            "covered": list[str],
            "missing": list[str],
            "extra": list[str]
        }
    """
    # Parse proposal scope
    proposal_path = os.path.join(change_dir, "proposal.md")
    scope_features = parse_proposal_scope(proposal_path)

    # Scan implementation
    impl_routes = extract_routes(src_dir)
    impl_models = extract_models(src_dir)

    # Compare
    result = compare_spec_impl(scope_features, impl_routes, impl_models)

    return {
        "passed": len(result["missing"]) == 0,
        **result
    }
```

### 10.3 文件改造清单

| 文件 | 改动 |
|------|------|
| `plugin/utils/spec-compliance.py` | 新增 Spec 合规分析脚本 |
| `plugin/utils/compliance-check.py` | 增加 `check_c10_spec_compliance()` |
| `plugin/skills/openspec-archive-change/SKILL.md` | Step 0 增加 C10 检查 |

---

## 11. 待实现：强制合规检查

### 11.1 目标

将合规检查从软约束（仅警告）变为硬约束（阻断 Archive）。

### 11.2 设计

修改 `openspec-archive-change/SKILL.md` Step 0：

```markdown
### Step 0: Pre-Archive Compliance Check (强制)

Before archiving, verify all requirements are met:

1. **Run compliance check**:
   - C1-C8: 必须检查
   - C9-C10: 可选检查（配置驱动）

2. **Handle result**:
   - If any MUST check fails → **BLOCK**: Display issues, do NOT proceed
   - User can override with explicit confirmation (logged)

3. **Persist report**:
   - Save to `openspec/changes/<name>/test-reports/compliance-<timestamp>.md`
```

### 11.3 文件改造清单

| 文件 | 改动 |
|------|------|
| `plugin/skills/openspec-archive-change/SKILL.md` | Step 0 改为强制阻断 |
| `plugin/utils/compliance-check.py` | 新增合规检查脚本 |
| `plugin/templates/compliance-report.md` | 新增合规报告模板 |