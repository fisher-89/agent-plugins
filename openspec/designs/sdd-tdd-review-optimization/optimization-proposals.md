# 优化方案总览

> 版本: 1.1
> 日期: 2026-05-08
> 更新: 新增 O/P/Q 方案（自动修复优化）

## 方案矩阵

| ID | 方案 | 解决问题 | 优先级 | 复杂度 |
|----|------|----------|--------|--------|
| A | 生成可执行测试骨架 | P1-1 | P1 | 中 |
| B | 实现后强制补测试 | P0-1 | P0 | 高 |
| C | 增量更新 test-reports | P0-3 | P1 | 中 |
| D | 测试运行门禁 | P0-1, P0-3 | P0 | 高 |
| E | Review 结果持久化 | P1-3 | P1 | 低 |
| F | 自动触发 Agent review | P0-2 | P0 | 中 |
| G | Spec 合规审查 | P2-2 | P2 | 高 |
| H | Review 门禁 (deny) | P0-2 | P1 | 低 |
| I | Propose 强制 design.md | P2-1 | P2 | 低 |
| J | 实现偏差检测 | P2-2 | P2 | 高 |
| K | Archive 合规检查 | P1-2 | P1 | 中 |
| L | 流水线 skill | P2-3 | P2 | 高 |
| M | 阶段门禁定义 | P2-3 | P2 | 中 |
| N | CI 集成 hook | - | P2 | 高 |
| **O** | **前移检测 + 即时修复** | **减少阻断** | **P0** | **中** |
| **P** | **安全问题自动修复** | **减少阻断** | **P0** | **高** |
| **Q** | **渐进式门禁** | **减少阻断** | **P1** | **中** |

---

## P0 方案详细（含新增 O/P）

### 方案 D: 测试运行门禁（测试范围驱动）

**目标**: 在 apply-change skill 中，标记 task 完成前强制运行相关测试

**核心改进**: 测试范围驱动 — 先识别测试范围，只运行范围测试，非全量

**触发点**:
- openspec-apply-change skill 完成一个 task 时
- 勾选 `- [x]` 之前

**实现逻辑（测试范围驱动）**:
```
1. 【识别测试范围】
   - 解析 task 描述，提取模块/功能关键词
   - 定位受影响的源文件（如 routes/auth.js）
   - 映射到测试文件（如 tests/auth.test.js）
   - 提取测试名称匹配模式

2. 【补充/更新测试用例】
   - 检查测试文件是否存在
   - 不存在 → 生成测试骨架
   - 存在但不完整 → 补充缺失用例
   - 存在且完整 → 继续

3. 【实现代码】
   - 执行代码修改

4. 【运行范围测试】
   - 仅运行步骤 1 识别的测试范围
   - 使用 testNamePattern 或指定文件
   - 如果测试失败 → 拒绝勾选，提示修复
   - 如果测试通过 → 标记完成
```

**测试运行策略**:
| 阶段 | 运行范围 | 命令 |
|------|----------|------|
| 单个 task | 范围测试 | `jest tests/auth.test.js` |
| 所有 task 完成 | 全量测试 | `jest` / `pytest` |
| CI/PR | 全量测试 + 覆盖率 | `jest --coverage` |

**技术方案**:
- 在 SKILL.md 中增加测试范围识别步骤
- 使用 `identify_test_scope()` 提取范围
- 使用 Bash tool 执行范围测试命令
- 新建测试不存在时，提示先编写测试

**详见**: [design-tdd-gate.md](./design-tdd-gate.md)

---

### 方案 B: 实现后强制补测试

**目标**: 每完成一个 task，确保有对应的测试文件

**触发点**:
- PreToolUse (Bash) 检测到 git commit
- 或 apply-change skill 内部在 task 完成点

**实现逻辑**:
```
1. 获取本次 task 影响的文件列表
2. 检查 tests/ 或 __tests__/ 下是否有对应测试
3. 如果没有测试文件 → 提示先编写测试
4. 如果有测试文件 → 运行测试验证通过
```

**技术方案**:
- 扩展 pre-tool-openspec-test.py 或新建 hook
- 分析 git diff 获取变更文件
- 映射变更文件到测试文件路径

**详见**: [design-tdd-gate.md](./design-tdd-gate.md)

---

### 方案 F: 自动触发 Agent review

**目标**: commit 前自动启动 code-review agent，而非仅建议

**触发点**:
- PreToolUse (Bash) 检测到 `git commit`
- 当前 pre-tool-commit-review.py 只注入 context

**改造方案**:
```python
# 当前: permissionDecision: "allow"
# 改为:
if has_staged_changes:
    # 不直接 allow，而是告诉 Claude 必须先 review
    context = (
        "MUST run code-review agent BEFORE committing. "
        "Use Agent tool with subagent_type='general-purpose' "
        "to review staged changes. Do NOT commit until review passes."
    )
    output_result("allow", context)  # 仍 allow，但指令更强制
```

**更好的方案**:
- 在 apply-change skill 完成 auto-commit 前，先调用 Agent review
- SKILL.md 中增加明确的 review 步骤

**详见**: [design-auto-review.md](./design-auto-review.md)

---

### 方案 O: 前移检测 + 即时修复（测试范围驱动）

**目标**: 把检测从 commit 时移到实现过程中，发现问题即时修复

**当前流程**:
```
[写代码] → [写更多代码] → [commit] → ❌ deny → 人为干预
```

**优化后流程（测试范围驱动）**:
```
[识别测试范围] → [补充/更新测试用例] → [写代码] → [即时检测] → [自动修复] → [运行范围测试] → [继续] → [commit] → ✅ pass
```

**触发点**:
- 每个 task 完成后（而非 commit 时）
- apply-change skill 中即时执行 lint/type check

**实现逻辑**:
```python
def on_task_complete(task, changed_files, test_scope):
    # 即时运行 lint/type check
    lint_result = run_lint(changed_files)
    if lint_result.errors:
        # 尝试自动修复
        fixed = auto_fix_lint(lint_result.errors)
        if fixed:
            return "继续下一步"
        else:
            return "阻断，提示修复"

    # 运行范围测试（非全量）
    test_result = run_scoped_tests(test_scope)
    if test_result.failed:
        return "阻断，修复测试"

    return "标记 task 完成"
```

**技术方案**:
- 在 `openspec-apply-change/SKILL.md` 中增加即时检测步骤
- 使用 `eslint --fix` / `ruff --fix` 自动修复
- 运行范围测试（`identify_test_scope()` 识别的范围）
- 无法自动修复的问题才阻断

---

### 方案 P: ERROR → Task 生成闭环

**目标**: Review 发现的所有 ERROR（不区分类型）生成新的修复 task，重入 apply 循环，消除阻断

**核心理念**：
- 不区分 ERROR 类型（安全/逻辑/空值等），统一处理
- 所有 ERROR → 生成修复 task → 追加到 tasks.md → 重新 apply
- 循环收敛：最多 3 轮，否则提示人工介入

**当前流程**:
```
[实现] → [Review] → ❌ ERROR → deny → 人为干预
```

**优化后流程**:
```
[实现] → [Review] → ⚠️ ERROR → 生成修复 task → 继续实现 → 循环直到无 ERROR
```

**可自动修复的安全问题**:

| 问题类型 | 检测模式 | 自动修复方式 | 可靠性 |
|---------|---------|-------------|--------|
| SQL 注入 | 字符串拼接 SQL | 参数化查询替换 | 高 |
| XSS | `innerHTML` 赋值 | `textContent` 转义 | 中 |
| Hardcoded secrets | 硬编码密钥/密码 | 移至环境变量 | 高（检测）|
| 路径遍历 | `fs.readFile(userInput)` | 添加 `path.resolve()` 校验 | 中 |

**实现逻辑**:
```python
def on_review_complete(review_result, change_dir):
    errors = review_result.errors  # 所有类型 ERROR

    if not errors:
        return "archive"  # 无 ERROR，继续归档

    loop_state = load_loop_state(change_dir)
    loop_state.loop_count += 1

    if loop_state.loop_count > 3:
        return "pause_manual"  # 3 轮后提示人工介入

    # 生成修复 task
    for error in errors:
        task = generate_fix_task(error)
        append_to_tasks_md(change_dir, task)

    save_loop_state(change_dir, loop_state)
    return "re_apply"  # 重新进入 apply 循环
```

**Task 生成格式**:
```markdown
- [ ] Fix: <简短描述> (<file>:<line>)
  - **Category**: <Logical/Security/NullHandling/...>
  - **Current**: `<问题代码>`
  - **Suggested**: `<修复建议>`
  - **Review**: test-reports/code-review-<timestamp>.md
```

**详见**: [design-auto-review.md](./design-auto-review.md)

---

### 方案 Q: 循环收敛机制

**目标**: 防止 ERROR → Task 循环无限执行

**收敛策略**:
- 最多 3 轮 Review 循环
- 3 轮后仍有 ERROR → 暂停，提示人工介入
- 避免因循环依赖或复杂问题导致无限循环

**状态文件** (`.wps_claude/review-loop-state.json`):
```json
{
  "change": "add-user-auth",
  "loop_count": 2,
  "max_loops": 3,
  "errors_fixed": 5,
  "current_errors": 1,
  "history": [
    {"round": 1, "errors": 5, "tasks_generated": 5},
    {"round": 2, "errors": 1, "tasks_generated": 1}
  ]
}
```

**实现逻辑**:
```python
def check_loop_convergence(change_dir):
    state = load_loop_state(change_dir)

    if state.loop_count >= state.max_loops:
        # 达到上限，提示人工介入
        return {
            "action": "pause",
            "message": f"Review 循环已达到 {state.max_loops} 轮上限，仍有 {state.current_errors} ERROR 未修复。请人工介入。"
        }

    return {"action": "continue"}
```

**技术方案**:
- 在 `openspec-apply-change` SKILL 中检查循环状态
- 每轮 Review 后更新 loop_count
- 达到上限时暂停并提示

---

## P1 方案详细

### 方案 A: 生成可执行测试骨架

**目标**: 测试模板从 markdown 占位符变为可运行的测试代码

**当前问题**:
```markdown
### Task 1: Implement user registration
- [ ] Test 1: Should correctly handle registration
- [ ] Test 2: Should return error for invalid email
```

**优化后**:
```javascript
// tests/auth.test.js
describe('User Registration', () => {
  it('should correctly handle registration', async () => {
    // TODO: Implement test
    const res = await request(app).post('/auth/register').send({
      email: 'test@example.com',
      password: 'password123'
    });
    expect(res.status).toBe(201);
    expect(res.body.user).toBeDefined();
  });

  it('should return error for invalid email', async () => {
    // TODO: Implement test
    const res = await request(app).post('/auth/register').send({
      email: 'invalid-email',
      password: 'password123'
    });
    expect(res.status).toBe(400);
  });
});
```

**技术方案**:
- 检测项目测试框架 (package.json → jest/vitest)
- 生成框架对应的测试文件结构
- 使用 LLM 或模板引擎生成测试用例

**详见**: [design-tdd-gate.md](./design-tdd-gate.md)

---

### 方案 E: Review 结果持久化

**目标**: Code review 结果写入 test-reports/，可追溯

**输出格式**:
```markdown
# Code Review: add-user-auth
> Date: 2026-04-30
> Reviewer: Claude Agent

## Staged Files
- src/routes/auth.js
- src/middleware/auth.js

### Issues Found

#### [ERROR] src/routes/auth.js:45
- **Issue**: Missing null check for req.body.email
- **Fix**: Add `if (!req.body?.email) return res.status(400)...`

#### [WARN] src/middleware/auth.js:23
- **Issue**: Redundant null check (already validated upstream)
- **Fix**: Remove redundant check

### Summary
1 ERROR, 1 WARN found. Fix ERROR before committing.

### Verdict
BLOCK - Fix required
```

**存储位置**: `openspec/changes/<name>/test-reports/code-review-<timestamp>.md`

**详见**: [design-auto-review.md](./design-auto-review.md)

---

### 方案 K: Archive 合规检查

**目标**: Archive 前验证所有 proposal scope 已实现，所有 task 已完成

**检查清单**:

```
□ proposal.md 存在且完整
□ design.md 存在 (如果配置要求)
□ tasks.md 所有 task 已勾选
□ 测试文件存在
□ 测试全部通过
□ code review 无 ERROR
□ 无 uncommitted changes
```

**技术方案**:
- 扩展 openspec-archive-change SKILL.md
- 在 Archive 步骤前增加 `pre-archive-check` 子流程
- 任何一项不通过则 pause 并报告

**详见**: [design-compliance-check.md](./design-compliance-check.md)

---

## P2 方案详细

### 方案 I: Propose 强制 design.md

**目标**: Propose 阶段必须产出技术设计文档

**design.md 结构**:
```markdown
# Design: add-user-auth

## Architecture
[系统架构图]

## API Design
### POST /auth/register
- Request: { email, password }
- Response: { user, token }
- Errors: 400, 409

## Data Model
### users table
- id: INTEGER PRIMARY KEY
- email: TEXT UNIQUE
- password_hash: TEXT
- role: TEXT DEFAULT 'user'

## Implementation Notes
- 使用 bcrypt 加密密码
- JWT 有效期 7 天
- Redis 缓存 session
```

**改造点**:
- openspec-propose SKILL.md 增加设计步骤
- 使用 openspec CLI 或直接 Write

---

### 方案 G: Spec 合规审查

**目标**: 对比 proposal scope vs 实现代码，检测偏差

**实现逻辑**:
```
1. 解析 proposal.md 的 Scope 部分
2. 扫描源代码目录
3. 提取 API endpoints / 数据模型 / 功能点
4. 对比差异
5. 生成合规报告
```

**输出示例**:
```markdown
# Spec Compliance Check

## Covered
- [x] User registration (POST /auth/register)
- [x] User login (POST /auth/login)
- [x] JWT session management

## Missing
- [ ] OAuth Google (mentioned in proposal, not implemented)
- [ ] OAuth GitHub (mentioned in proposal, not implemented)
- [ ] Redis caching (mentioned in proposal, not found)

## Extra (Not in Proposal)
- [x] Logout endpoint (POST /auth/logout)
```

---

### 方案 L: 流水线 skill

**目标**: 一键执行 explore → propose → apply → test → review → archive

**新 skill**: `openspec-pipeline`

**输入**: 需求描述字符串

**流程**:
```
1. 调用 openspec-explore (交互式探索)
2. 确认后调用 openspec-propose
3. 自动进入 openspec-apply-change
4. 每个 task 完成后自动运行测试
5. 所有 task 完成后自动 review
6. review 通过后自动 archive
```

**详见**: [design-pipeline-skill.md](./design-pipeline-skill.md)

---

## 方案依赖关系

```
D(测试门禁) ──┬──> K(合规检查)
              │
B(强制补测试) ─┘

F(自动review) ──> E(结果持久化) ──┬──> K
                                │
H(Review门禁) ──────────────────┘

A(可执行骨架) ──> D

I(design.md) ──> G(合规审查) ──> K

L(流水线) ── Depend On ──> D, F, K

O(前移检测) ──> D(减少进入Review的错误)
P(ERROR→Task) ──> F(自动闭环，消除阻断)
Q(循环收敛) ──> P(防止无限循环)
```

**实施顺序**:
1. D + F (核心门禁)
2. **O + P + Q (自动闭环优化)** ← 新增，消除阻断
3. B + A + E (增强)
4. K + H (合规)
5. G + I (设计约束)
6. L (流水线编排)