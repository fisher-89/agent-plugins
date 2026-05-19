# 设计: enhance-phase-requirements-skill

> **变更**: enhance-phase-requirements-skill
> **日期**: 2026-05-19
> **基于**: proposal.md, test-design.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| CLI 包装器 | 封装 `openspec new change`、`openspec status --json`、`openspec instructions` 等 CLI 调用，提供统一的错误处理和输出解析 | `plugins/dev-team/utils/openspec-cli.sh` | `openspec` CLI 可执行文件 | Shell (bash) |
| SKILL.md Step 1 (增强) | 检测变更是否存在；不存在时自动脚手架；检测探索上下文；AskUserQuestion 兜底 | `plugins/dev-team/skills/phase-requirements/SKILL.md` (Step 1 区域) | `openspec-cli.sh`, `AskUserQuestion` 工具 | SKILL.md (Claude 指令) |
| SKILL.md Step 3a (增强) | 调用 Planner 前收集 `openspec instructions` 动态输出和探索上下文摘要，构建富化提示词 | `plugins/dev-team/skills/phase-requirements/SKILL.md` (Step 3a 区域) | `openspec-cli.sh` | SKILL.md (Claude 指令) |
| Step 2 / 3b / 3c / 4 (不变) | Backtrack 检测、Evaluator 调用、P→E 循环、结果报告 | `plugins/dev-team/skills/phase-requirements/SKILL.md` (Step 2-4) | 无新增依赖 | SKILL.md (Claude 指令) |
| test 基础设施 | bats 单元测试和手动集成测试脚本，验证 CLI 包装器和 SKILL.md 各执行路径 | `openspec/changes/enhance-phase-requirements-skill/tests/` | `bats` v6+, `mktemp`, fixture 数据 | Shell (bats) |

### 组件图

```
用户调用 /dev-team:phase-requirements [name]
         |
         v
┌─────────────────────────────────────────────────────┐
│ Step 1: 检测活跃变更 (增强)                          │
│                                                     │
│  是否有 change-name?                                  │
│   ├─ 是 → change_exists()?                            │
│   │      ├─ 是 → 直接进入 Step 2                     │
│   │      └─ 否 → openspec-cli.sh: openspec_new_change│
│   │              → 写入 openspec/changes/<name>/     │
│   │              → 进入 Step 2                       │
│   │                                                  │
│   └─ 否 → 是否来自 openspec-explore?                  │
│          ├─ 是 → 提取探索洞察摘要                    │
│          │      → (若无 change-name) AskUserQuestion  │
│          │      → derive_kebab_case → 确认           │
│          │      → scaffolding → 进入 Step 2           │
│          │                                            │
│          └─ 否 → AskUserQuestion "想构建什么?"        │
│                 → derive_kebab_case → 展示并确认      │
│                 → scaffolding → 进入 Step 2           │
└──────────────────────┬──────────────────────────────┘
                       v
┌─────────────────────────────────────────────────────┐
│ Step 2: Backtrack 检测 (不变)                        │
│ 读取 eval.json, 检查 backtrack_to                   │
└──────────────────────┬──────────────────────────────┘
                       v
┌─────────────────────────────────────────────────────┐
│ Step 3a: 调用 Planner (增强)                         │
│                                                     │
│  1. openspec-cli.sh: openspec_status_json            │
│  2. openspec-cli.sh: openspec_instructions           │
│  3. (若有) 探索上下文摘要                            │
│  4. 构造富化提示词:                                  │
│     静态模板路径 + CLI 指令动态输出                   │
│     + 探索上下文 (可选)                              │
│     → Agent({ subagent_type: "requirements-planner" })│
└──────────────────────┬──────────────────────────────┘
                       v
┌─────────────────────────────────────────────────────┐
│ Step 3b/3c: P→E 循环 (不变)                         │
│ Evaluator → 检查 verdict → pass? → 完成              │
│                           → fail? → 重新 Planner    │
└──────────────────────┬──────────────────────────────┘
                       v
┌─────────────────────────────────────────────────────┐
│ Step 4: 报告结果 (不变)                              │
└─────────────────────────────────────────────────────┘
```

---

## 数据流

### 流程描述

**场景一：变更已存在（回归路径）**
1. 用户调用 `/dev-team:phase-requirements <name>`，name 对应已有变更
2. Step 1: `change_exists()` 返回 true，跳过脚手架
3. Step 2: 读取 `openspec/changes/<name>/phases/eval.json`，检查 backtrack marker
4. Step 3a: 调用 `openspec status --json` 和 `openspec instructions` 获取动态上下文，注入 Planner 提示词。Planner 写入 `proposal.md`
5. Step 3b-3c: P→E 循环，行为与当前完全一致
6. Step 4: 报告结果

**场景二：变更不存在（脚手架路径）**
1. 用户调用 `/dev-team:phase-requirements <name>`，name 对应新变更
2. Step 1: `change_exists()` 返回 false
3. 调用 `openspec new change "<name>"` 创建脚手架
4. 验证 `.openspec.yaml` 已创建，继续 Step 2
5. Step 2-4 与场景一一致

**场景三：来自 openspec-explore**
1. 用户完成探索会话后调用 `/dev-team:phase-requirements`（无参数）
2. Step 1: 无 change-name，检测到探索上下文
3. 提取探索洞察摘要（决策、设计选择、排除方案）
4. AskUserQuestion 询问变更名，derive_kebab_case 推导，用户确认
5. `openspec new change` 脚手架
6. Step 3a: 将探索洞察摘要作为额外上下文注入 Planner 提示词
7. Step 2-4 同上

**场景四：无上下文兜底**
1. 用户在空对话中调用 `/dev-team:phase-requirements`（无参数）
2. Step 1: 无 change-name，无探索上下文
3. AskUserQuestion "想构建什么变更？描述你想实现的功能或修复的问题"
4. derive_kebab_case 从用户回答推导 kebab-case 名称
5. 展示建议名称并请求确认
6. 用户确认后 `openspec new change` 脚手架
7. Step 2-4 同上

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| Change 目录 | `openspec/changes/<name>/` 包含 `.openspec.yaml` 和 `phases/` 子目录 | 1个 Change 对应 1个 `.openspec.yaml` + N 个 phase artifacts | 文件系统 |
| `.openspec.yaml` | `schema: spec-driven`, `created: <date>` | 从属于 Change 目录 | 文件系统 |
| `openspec status --json` 输出 | `changeName`, `artifacts[]` (id, status, dependencies), `applyRequires[]` | 描述 Change 的结构化元数据 | CLI 输出 (不持久化) |
| `openspec instructions` 输出 | `rules[]`, `context`, `template`, `instruction`, `outputPath`, `dependencies[]` | 指导 artifact 创建的结构化指令 | CLI 输出 (不持久化) |
| 探索上下文摘要 | 决策列表、设计选择、排除方案、关键分析 | 临时数据结构，仅存在于 Planner 提示词中 | 内存 (不持久化) |
| Planner 提示词 (增强) | 静态模板路径 + `openspec instructions` 输出 + (可选) 探索上下文摘要 | 输入给 requirements-planner agent | 内存 (不持久化) |
| `eval.json` (不变) | `phase`, `timestamp`, `attempt`, `verdict`, `report`, `items[]`, `backtrack_to`, `schema_version` | 1个 Change 对应 1个 `eval.json` 文件，包含 N 个 evaluation entries | 文件系统 (`phases/eval.json`) |

---

## 路由/API 设计

本变更为 SKILL.md 增强，不涉及 HTTP 路由或 REST API。等价的设计如下：

### CLI 接口 (openspec-cli.sh)

| 函数 | 描述 | 输入 | 输出 | 错误处理 |
|------|------|------|------|----------|
| `openspec_new_change(name)` | 创建变更脚手架 | change name (kebab-case) | stdout; 退出码 0 成功 | 非零退出码时输出错误消息到 stderr |
| `openspec_status_json(name)` | 获取变更状态 (JSON) | change name | JSON stdout | 空 JSON `{}` 时回退到静态模板 |
| `openspec_instructions(name)` | 获取 artifact 指令 (JSON) | change name | JSON stdout | 缺失字段时容忍，仅注入存在的字段 |
| `change_exists(name)` | 检测变更目录是否存在 | change name | 退出码 0=存在, 1=不存在 | - |
| `derive_kebab_case(description)` | 中文描述转 kebab-case 名称 | 中文/英文描述字符串 | kebab-case 字符串 (max 128 字符) | 空输入返回空字符串 |
| `validate_change_name(name)` | 校验变更名格式 | 字符串 | 退出码 0=有效, 1=空, 2=过长, 3=格式非法 | - |

### SKILL.md 入口

| 入口 | 描述 | 输入 | 输出 | 副作用 |
|------|------|------|------|--------|
| `/dev-team:phase-requirements [name]` | DESIGN 阶段 P→E 循环 | 可选 change-name (kebab-case) | `proposal.md`, `eval.json` 更新 | 无 name 且无上下文时触发 AskUserQuestion |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D01 | CLI 包装器采用 Shell 脚本而非 Python | Shell 脚本与 SKILL.md 的 bash 命令天然兼容，无需引入 Python 运行时依赖。SKILL.md Step 1 和 Step 3a 中的 CLI 调用可以直接 source 包装器。bats 测试框架对 Shell 脚本的测试支持成熟 | Python 包装器 (rejected)：引入额外的 Python 运行时依赖，且 SKILL.md 执行环境中 Python 可用性不确定；需要额外的进程调度层 |
| D02 | 探索上下文检测基于对话历史而非文件标记 | 避免修改 openspec-explore SKILL.md（不在当前变更范围内）。Claude 可以通过检查对话历史中的探索模式（"What We Figured Out"、设计决策、ASCII 图等自然标记）来检测是否来自探索会话 | 文件标记方案 (rejected)：需要在 openspec-explore 结束时写入标记文件，这超出了本变更的范围，且引入跨技能耦合 |
| D03 | 探索上下文处理内嵌在 SKILL.md 指令中而非独立实用程序 | 上下文摘要的逻辑本质上是 Claude 对自然语言的理解和提炼，适合直接由 Claude 按 SKILL.md 指令执行，无需独立的可测试代码。独立性较弱时引入额外抽象反而增加维护成本 | 独立的 explore_context.py 实用程序 (rejected)：Python 代码难以处理自由格式的对话历史摘要，且提取和汇总的质量完全依赖 LLM 而非确定性逻辑 |
| D04 | `openspec instructions` 结果缓存基于目录 mtime，单次技能执行内共享 | 一次技能执行中 Step 1 和 Step 3a 可能重复调用 status/instructions，缓存避免冗余 CLI 调用。基于 mtime 判断确保缓存不会跨执行误用 | 无缓存 (rejected)：增加 CLI 调用次数导致技能响应变慢；LRU 全局缓存 (rejected)：跨技能执行缓存可能返回过时数据 |
| D05 | kebab-case 推导采用纯 Shell 文本处理而非调用外部服务 | 名称推导是确定性字符串操作（小写、替换非字母数字为连字符、折叠连字符、截断），Shell 足以胜任且速度快。无需调用 LLM 或外部 API | 调用 LLM 推导名称 (rejected)：增加延迟和成本，且确定性算法结果更可预测和可测试 |
| D06 | Dockerfile 的 main.py 入口点文件保持不变，不修改 | Dockerfile 属于部署基础设施，当前变更不涉及容器化部署。如果将来需要，可以作为单独变更处理 | 同步修改 Dockerfile (rejected)：不相关，引入无必要的变更范围扩大 |

---

## 依赖

### 运行时依赖

- `openspec` CLI — 提供 `new change`、`status`、`instructions` 命令，必须安装在环境 PATH 中
- `AskUserQuestion` 工具 — Claude Code 内置工具，用于无参数时的用户交互
- `Agent({subagent_type: ...})` — Claude Code 内置 Agent 调度，用于调用 requirements-planner 和 requirements-evaluator
- `bats` v6+ (仅测试依赖) — Bash Automated Testing System，用于 CLI 包装器的单元测试

### 构建/测试依赖

- 无构建步骤（SKILL.md 无需编译）
- 测试依赖：`bats` v6+, `mktemp`, 标准 POSIX 工具集 (`grep`, `sed`, `tr`, `head`, `mkdir`, `diff`)

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `openspec new change` CLI 输出格式在不同版本间变化 | 中等：脚手架创建失败，用户需手动重试 | 低 | 在 `openspec-cli.sh` 中封装所有 CLI 交互，统一处理版本差异；只有退出码被用于判断成功 |
| 探索上下文传递给 Planner 时包含无关信息 | 中等：Planner 产出低质量提案，增加迭代次数 | 中 | SKILL.md 指示 Claude 仅提取结构化洞察（决策、设计选择、排除方案）；Planner 提示词中明确"探索上下文仅供参考" |
| AskUserQuestion 兜底中用户提供模糊描述 | 低：变更名不准确但仍可工作 | 低 | 推导名称后展示建议并请求确认（"I'll create a change named '<name>'. Proceed?"）；允许用户输入自定义名称 |
| CLI 调用增加导致技能响应变慢 | 低：增加约 1-2 秒延迟 | 中 | CLI 调用是轻量级本地操作；单次技能执行内缓存 status/instructions 结果 |
| 用户在脚手架确认阶段取消 | 无影响：流程正常终止 | 低 | SKILL.md 包含友好终止提示"已取消提案编写，你可以稍后重试" |

---

## 待决问题

- `openspec status --json` 和 `openspec instructions` 的具体 JSON schema 未知，需要在实现阶段通过运行 CLI 命令确认字段名和嵌套结构，调整 `openspec-cli.sh` 中的解析逻辑
- `derive_kebab_case` 对中文短语的处理效果（拼音 vs 英文翻译 vs 纯移除中文字符）需要在实际使用中验证和调优
- 测试 fixture 中 `openspec_status.json` 和 `openspec_instructions.json` 的具体内容需要在实际环境中录制，确保与当前安装的 `openspec` 版本匹配
