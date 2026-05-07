# 优化方案总览

> 版本: 1.0
> 日期: 2026-04-30

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

---

## P0 方案详细

### 方案 D: 测试运行门禁

**目标**: 在 apply-change skill 中，标记 task 完成前强制运行相关测试

**触发点**:
- openspec-apply-change skill 完成一个 task 时
- 勾选 `- [x]` 之前

**实现逻辑**:
```
1. 检测 task 对应的源文件
2. 查找关联的测试文件
3. 运行测试 (jest/vitest/pytest)
4. 如果测试失败 → 拒绝勾选，提示修复
5. 如果测试通过 → 标记完成
```

**技术方案**:
- 在 SKILL.md 中增加测试运行步骤
- 使用 Bash tool 执行 `npm test` / `pytest`
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
```

**实施顺序**:
1. D + F (核心门禁)
2. B + A + E (增强)
3. K + H (合规)
4. G + I (设计约束)
5. L (流水线编排)