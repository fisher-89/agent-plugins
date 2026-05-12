## Context

当前项目是 `wps-claude-plugin`，一个 Claude Code 插件，通过 OpenSpec SDD 工作流管理开发过程。插件已经有 `openspec-*` 技能（explore, propose, apply-change, archive-change）和 `code-review` 技能。

但缺少架构层级的结构化约束。代码变更可以绕过架构规则提交，没有人检查新代码是否与声明的系统结构一致。本次变更引入架构师角色，基于 likec4 提供三个子代理技能和一个提交时验证门禁钩子。

## Goals / Non-Goals

**Goals:**
- 提供 `archi-model` 子代理，通过 likec4 API 读取模型、DSL 文本编辑写入模型
- 提供 `archi-validate` 子代理，解析变更文件的 import 依赖交叉比对模型关系
- 提供 `archi-decide` 子代理，创建和维护 ADR 记录
- 提供 PreToolUse 提交钩子，确保所有代码变更提交前经过架构验证
- 架构产物存放在 `openspec/architecture/` 目录下

**Non-Goals:**
- 不提供面向人类交互的技能（如 archi-view 可视化、archi-generate 生成站点）
- 不在 SessionStart 时加载架构上下文
- 不提供独立的 archi-diff 技能（diff 信息已包含在验证报告中）
- 不在归档时验证（唯一检查点在提交）
- 不对 `.c4` 文件做 PreToolUse Write/Edit 拦截（钩子门禁在提交时）

## Decisions

### 决策 1：子代理模式，非顶层命令

三个 archi 技能作为子代理加载，由 openspec-propose [未来]、archi-validate 钩子、或用户直接调用触发。不作为独立的 `/archi-*` 顶层命令暴露。

**理由**：面向代理，非面向人类。子代理在需要时才被加载，不增加命令列表噪音。

**备选方案**：顶层命令（/archi-model 等）。被拒绝原因：用户不需要手动输入这些命令，它们都是被工作流自动触发的。

### 决策 2：DSL 文本编辑 + fromSource 验证

likec4 的 `LikeC4Model.Computed`（`@likec4/core/model`）只支持读取；`Builder` API（`@likec4/core/builder`）支持创建但无法序列化回 DSL 文本。因此模型修改走 DSL 文本编辑路径：

```
1. Read model.c4 → DSL 字符串
2. 解析当前结构，确定插入位置
3. 拼接新 DSL 片段
4. fromSource(newDsl) → 验证语法和模型合法性
5. 通过 → Write；失败 → 报错
```

**理由**：likec4 不提供结构化修改 API。这是唯一可行的修改路径。`fromSource()` 在写入前提供语法保证。

**备选方案**：使用 Builder API 完全重建模型。被拒绝原因：需要从现有模型提取所有元素/关系，再追加新内容，最后序列化——但 Builder 不支持序列化回 DSL 文本，它只输出 LikeC4Model 对象。

### 决策 3：metadata.path 映射代码目录

元素通过 `metadata.path` 关联代码。一个元素可以指定多个 path：

```c4
component paymentService {
    metadata path ["./src/services/payment/", "./src/shared/billing.ts"]
}
```

`archi-validate` 匹配时，`metadata.path` 指向的目录包含所有子文件。

**理由**：`link` 是 likec4 标准属性用于文档链接，`metadata.path` 专注代码映射。数组支持多个入口。

### 决策 4：依赖级验证（非仅存在性检查）

验证分三层，全部执行：

- **L1 存在性**：metadata.path 指向的目录/文件在代码库中是否存在
- **L2 依赖对账**：解析变更文件的 import，检查每个 import 的目标是否在模型中有对应的 element，且两个 element 之间有声明的 relationship
- **L3 完整性**：变更文件是否都有对应的 model element；模型中已声明的 element 是否在代码库中有迹可查

**理由**：仅检查文件存在性价值有限。依赖对账是真正的架构约束——"A 调用 B" 不应该是因为编码方便，而是因为架构明确允许。

### 决策 5：提交时验证，归档时不验证

唯一的检查点在 `git commit`。PreToolUse(Bash) 钩子检测提交命令，检查所有变更文件（全部文件，非仅匹配项），查找 `reports/validate-*.json`。有报告则放行，无报告则拒绝并要求代理运行 archi-validate。

归档时不再重复验证。

**理由**：提交是变更进入历史的最后一道门。提交时被拦截，变更还没进入仓库，修复成本最低。归档时已验证过的变更不会出现新问题。

### 决策 6：验证报告 JSON 格式 + Ask 决策

验证报告格式与现有 SDD 报告一致（JSON）：

```jsonc
{
  "timestamp": "2026-05-12T10:30:00Z",
  "commit_diff_hash": "abc123",
  "changed_files": ["src/payment/handler.ts", "src/gateway/middleware.ts"],
  "matched": [
    { "element_id": "paymentService", "files": ["src/payment/handler.ts"] },
    { "element_id": "apiGateway", "files": ["src/gateway/middleware.ts"] }
  ],
  "violations": [
    {
      "type": "unmodeled_dependency",
      "source_file": "src/payment/handler.ts",
      "import_target": "./utils/logger",
      "detail": "paymentService imports logger which is not a modeled element"
    }
  ],
  "warnings": [
    {
      "type": "unused_relationship",
      "element_a": "paymentService",
      "element_b": "notificationService",
      "detail": "model declares paymentService->notificationService but no import found"
    }
  ],
  "unmatched_files": ["src/one-off-script.ts"],
  "model_changes": {
    "added": [],
    "removed": [],
    "modified": []
  }
}
```

有违规时，代理用 AskUserQuestion 让用户选择：更新模型 / 修复代码 / 标记例外。

### 决策 7：ADR 按日期命名 + 影响范围

```
openspec/architecture/decisions/YYYY-MM-DD-<kebab-title>.md
```

模板：

```markdown
# ADR: <title>
- **日期**: YYYY-MM-DD
- **状态**: proposed | accepted | deprecated | superseded
- **背景**: ...
- **决策**: ...
- **后果**: ...
- **备选方案**: ...
- **影响范围**: [model element IDs]
```

**理由**：日期命名支持按时间线浏览。影响范围字段连接 ADR 和模型元素，防止脱节。

## Risks / Trade-offs

- **likec4 API 稳定性**：likec4 仍在积极开发中，API 可能变化。缓解：子代理通过标准化接口封装 likec4 调用，变更时只需修改子代理 SKILL.md。
- **提交钩子性能**：依赖解析所有变更文件可能较慢，大型项目提交体验受影响。缓解：实现上缓存解析结果，仅分析变更文件。
- **DSL 编辑风险**：文本编辑比结构化 API 更容易引入语法错误。缓解：写入前必须通过 `fromSource()` 验证。
- **假阳性**：import 解析可能标记出代码生成、类型导入等非运行时依赖。缓解：warning 级别可忽略，violation 由用户确认。
