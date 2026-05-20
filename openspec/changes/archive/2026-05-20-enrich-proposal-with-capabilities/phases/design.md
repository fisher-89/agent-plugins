# 设计: enrich-proposal-with-capabilities

> **变更**: enrich-proposal-with-capabilities
> **日期**: 2026-05-20
> **基于**: proposal.md, test-design.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| proposal.md.template | 定义 proposal 文档的静态结构模板，新增 Capabilities 章节引导 agent 输出能力清单 | `plugins/dev-team/templates/artifacts/proposal.md.template` | 无（文件模板，无运行时依赖） | Markdown |
| phase-requirements SKILL.md | 编排 P→E 循环的 skill 定义：从 CLI 获取动态上下文、构造 prompt、调用 Planner 和 Evaluator agent | `plugins/dev-team/skills/phase-requirements/SKILL.md` | `openspec-cli.sh`、`requirements-planner` agent、`requirements-evaluator` agent | Claude Code Skill（Markdown + bash） |
| openspec-cli.sh | CLI 包装器，提供 `openspec_spec_list()` 等 shell 函数供 skill 和 agent 调用，封装 CLI 交互细节并实现 fail-safe | `plugins/dev-team/utils/openspec-cli.sh` | `openspec` CLI 二进制（运行时可选） | Bash |
| requirements-planner | 编写 proposal.md 的 AI agent：读取模板，调用 `openspec_spec_list()` 获取已有 capability 列表，在 Capabilities 章节中区分 New 和 Modified | `plugins/dev-team/agents/requirements-planner.md` | `proposal.md.template`、`openspec-cli.sh`（通过 skill 注入的 context） | Claude Code Agent（Markdown） |
| requirements-evaluator | 评估 proposal.md 的 AI agent：运行静态 checklist（含 Capabilities 检查项），追加结果到 eval.json | `plugins/dev-team/agents/requirements-evaluator.md` | `proposal.md`、`eval.schema.json` | Claude Code Agent（Markdown） |

### 组件图

```
+------------------+       reads        +-----------------------+
|  openspec CLI    |<-------------------|  openspec-cli.sh      |
|  (external bin)  |    spec list       |  (CLI wrapper)        |
+------------------+    --json          +----------+------------+
                                                  | sourced by
                                                  v
+------------------+  constructs prompt  +-----------------------+
|  SKILL.md        |------------------->|  requirements-planner |
|  (phase-req.)    |    (with context)  |  (AI agent)           |
+--------+---------+                    +-----------+-----------+
         |                                         | writes
         |  invokes                                v
         |  +------------------+           +-----------------------+
         +->|  requirements-   |<----------|  proposal.md          |
           |  evaluator       |   reads    |  (artifact)           |
           |  (AI agent)      |           +-----------------------+
           +--------+---------+                     ^
                    |                               | template
                    | appends                       | reference
                    v                               |
            +---------------+      +----------------+----------+
            | eval.json     |      | proposal.md.template       |
            | (evaluation   |      | (with Capabilities section)|
            |  results)     |      +---------------------------+
            +---------------+
```

---

## 数据流

### 流程描述

**Phase-requirements P→E 完整流程（含 Capabilities）：**

1. **SKILL.md Step 3a 收集上下文**：调用 `openspec_cli_cache status "<name>"` 和 `openspec_cli_cache instructions "<name>"` 获取动态 CLI 上下文。
2. **构造 Planner prompt**：将静态模板路径、CLI 的 `rules`/`context` 字段（不含 `template`）、可选的探索上下文合并为 prompt。
3. **Planner 执行**：
   a. 读取静态模板 `proposal.md.template`（含 Capabilities 章节）
   b. 调用 `openspec_spec_list()` 获取全局已有 capability ID 列表 `["auth", "storage"]`
   c. 对每个要列出的 capability：如果在列表中 → 写入 `Modified Capabilities` 子章节；不在列表中 → 写入 `New Capabilities` 子章节
   d. 如果 `openspec_spec_list()` 返回 `[]`（CLI 不可用或无 spec），所有条目标记为 `New`
   e. 写入 `openspec/changes/<name>/phases/proposal.md`
4. **SKILL.md Step 3b 调用 Evaluator**：构造 prompt 引用静态 checklist。
5. **Evaluator 执行**：
   a. 读取 `proposal.md`
   b. 运行 R1-R6 + R7(R8) 静态检查项，其中 Capabilities 检查项（R8）为必须通过项
   c. 追加结果到 `eval.json`
6. **SKILL.md Step 3c 判断 verdict**：pass → 完成；fail → 带着失败项重新调用 Planner（回到步骤 3）

**Capabilities 数据流向：**

```
openspec spec list --json
        |
        v
  openspec_spec_list()
        |
        +-- CLI 可用: 返回 JSON 数组, e.g. ["auth", "storage"]
        +-- CLI 不可用: 返回 "[]" (fallback)
        |
        v
  requirements-planner agent
        |
        +-- capability in list? → Modified Capabilities 子章节
        +-- capability not in list? → New Capabilities 子章节
        |
        v
  proposal.md 中的 Capabilities 章节
        |
        v
  requirements-evaluator agent (检查 R8)
        |
        v
  eval.json (verdict: pass/fail)
```

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `openspec spec list --json` 输出 | JSON 数组 `["capability-id-1", "capability-id-2"]` — 每个元素为 capability ID 字符串 | 直接输入到 Planner 的 Capabilities 分类逻辑 | 临时（每次调用刷新） |
| `openspec_spec_list()` 返回值 | JSON 数组字符串（shell stdout）：正常时返回 CLI 输出的数组，CLI 失败时返回 `[]` | 上游：CLI 输出；下游：Planner 读取 stdout 后解析为 shell 数组 | 无持久化（每次调用实时获取） |
| `proposal.md` Capabilities 章节 | `## New Capabilities` 和 `## Modified Capabilities` 两个子章节，各为无序列表 | 每个条目对应一个 `specs/<capability>/spec.md` 文件（由后续 spec 编写阶段创建或更新） | 文件系统（Markdown 文件） |
| `eval.json` 条目 | `phase`, `timestamp`, `attempt`, `verdict`, `report`, `items[]`, `backtrack_to`, `schema_version` | `items[].item_id` 中 `R8` 对应 Capabilities 章节检查 | 文件系统（JSON 文件） |

---

## 路由/API 设计

本变更仅涉及现有 CLI 命令的包装，不新增 HTTP API 或 CLI 命令。

| 方法/命令 | 路径/调用方式 | 描述 | 输入 | 输出 | 认证/前置条件 |
|-----------|-------------|------|------|------|-------------|
| `openspec spec list --json` | 外部 CLI 调用 | 获取项目中所有全局 capability ID 列表 | 无（CLI 读取 `openspec/specs/` 目录结构） | JSON 数组 `["id1","id2"]` 或空数组 `[]` | `openspec` CLI 已安装且项目已初始化 |
| `openspec_spec_list()` | `plugins/dev-team/utils/openspec-cli.sh` 中的 shell 函数 | 封装 `openspec spec list --json`，含输出格式校验和 fail-safe | change name（未使用，保留参数兼容） | stdout JSON 数组；CLI 故障时返回 `[]` 并输出 stderr 警告 | 无（fail-safe 设计，CLI 不可用时仍可调用） |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 在静态模板中增加 Capabilities 章节，而非依赖 CLI 动态注入 | 静态模板是 agent 行为的主驱动，修改模板对 agent 行为的影响最直接、最可控。每个 agent 定义中已有静态模板路径引用，修改一处即可生效 | **备选：CLI 动态注入** — 完全依赖 `openspec instructions` 的 `template` 字段驱动 agent。排除理由：CLI 端返回的 template 内容不稳定（返回空 `{}` 时需回退），agent 定义中硬编码了静态模板路径，动态注入内容无法被 evaluator 静态 checklist 可靠引用 |
| D2 | CLI wrapper 中新增 `openspec_spec_list()` 函数而非直接内联 `openspec spec list --json` 调用 | 保持 CLI 交互集中管理在 `openspec-cli.sh` 中，与现有的 `openspec_status_json()`、`openspec_instructions()` 等函数模式一致。方便增加输出校验和 fail-safe 逻辑，易于单元测试 | **备选：在 agent 定义中直接写 CLI 调用** — 排除理由：违反 DRY 原则，每个 agent 都需要重复同样的 CLI 调用和错误处理逻辑，且无法通过 bats 单元测试覆盖 |
| D3 | `openspec_spec_list()` 输出校验：检查是否为有效 JSON 数组，非数组时返回 `[]` | 防御性编程，防止 CLI 输出格式变更（如字段重命名、错误响应）导致 agent 解析失败 | **备选：不校验直接透传** — 排除理由：CLI 可能返回 `{"error":"..."}` 等非数组 JSON，agent 解析非数组内容可能导致意外行为或中断 |
| D4 | 基于全局 `openspec/specs/` 目录做 New/Modified 分类，而非基于 change-local 的 `openspec/changes/<name>/specs/` | `openspec spec list --json` 查询的是全局 capabilities，代表项目中已存在的能力清单。change-local 的 spec 属于当前变更的新增或修改范围，不应作为分类依据 | **备选：基于 change-local specs 目录分类** — 排除理由：change-local 的 specs 是当前变更将要产生的内容，在 proposal 编写阶段还不存在（或为空），无法作为"已有能力"的判断依据 |
| D5 | Evaluator 的 Capabilities 检查项（R8）标记为必须（`true`），替换原 R7 为非必须 | 确保 Agent 不会遗漏 Capabilities 章节。R7（"所有模板章节已填写实质性内容"）在原模板中仅需检查 6 个章节，新增 Capabilities 后一并归入 R7；新 R8 专门针对 Capabilities 章节的存在性和完整性做强校验 | **备选：将 Capabilities 检查设为非必须项** — 排除理由：与非必须项的设计意图不符（非必须项意味着"最好有但可以没有"），会导致 agent 可能跳过 Capabilities 章节 |

---

## 依赖

### 运行时依赖

- **openspec CLI**（外部二进制）— 提供 `spec list --json` 命令用于获取全局 capability 列表。此依赖为软依赖：CLI 不可用时 `openspec_spec_list()` 返回 `[]`，agent 仍能正常生成 proposal（所有条目标记为 New）

### 构建/测试依赖

- **bats (Bash Automated Testing System) v6+** — 用于单元测试 `openspec_spec_list()` 函数
- **Python 3** — 用于 JSON 断言（`python3 -c "import json; ..."`）和 fixture 验证
- **mktemp** — 用于创建测试沙箱目录

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 已有 proposal（无 Capabilities 章节）在 evaluator 升级后首次检查 fail | 中等 | 中 | Evaluator 的 Capabilities 检查项仅在模板包含该章节时触发；对已有 proposal 增加过渡期，首次检查仅记录 warning 不导致 fail |
| `openspec spec list --json` 输出格式与预期不一致 | 高 | 低 | `openspec_spec_list()` 增加输出校验：检查是否为 JSON 数组；若不是，返回 `[]` 并记录 stderr 警告 |
| Agent 将 global specs 与 change-local specs 混淆 | 中等 | 中 | Agent 定义中明确限定只查询 `openspec spec list --json`（global），并在 prompt 中说明返回的是"项目中已存在的能力清单"；evaluator 检查项验证 Modified 条目对应 global 目录下的 spec |
| SKILL.md 移除 template 注入后，依赖 CLI template 字段的自定义扩展停止工作 | 高 | 低 | CLI 的 `openspec instructions` 输出仍保留 `template` 字段，仅 SKILL.md 不再读取它；未来可通过 agent context 字段传递而非 prompt 注入 |
| Template 新增 Capabilities 章节后旧版 agent 生成 proposal 缺少该章节 | 中等 | 中 | Evaluator 的新 R8 检查项会捕获缺失 Capabilities 的 proposal，导致 P→E 循环 fail 并触发重新生成 |

---

## 迁移步骤

1. **修改 openspec-cli.sh**：新增 `openspec_spec_list()` 函数，包含 CLI 调用、输出校验和 fail-safe 逻辑，以及缓存注册
2. **修改 proposal.md.template**：在 `## 风险` 章节之后、文件末尾之前插入 Capabilities 章节，包含 `New Capabilities` 和 `Modified Capabilities` 两个三级子章节及其引导注释
3. **修改 SKILL.md**：移除 Step 3a 中 prompt 构造中对 `INSTRUCTIONS_JSON` 的 `template` 字段引用，保留 `rules` 和 `context` 字段
4. **修改 requirements-planner.md**：在 Process 中新增步骤：编写 proposal 前调用 `openspec_spec_list()`；在 Output 说明部分增加 Capabilities 章节的编写指南和 New/Modified 区分规则
5. **修改 requirements-evaluator.md**：在 checklist 中新增 R8（Capabilities 章节检查，必须），将 R7 调整为对 Capabilities 章节的实质性内容检查（原 R7 的通用章节覆盖逻辑保持但降低权重）
6. **创建测试辅助库**：基于现有 `setup_test_env.sh` 扩展，新增 `mock_spec_list()`、`mock_specs_dir()`、`assert_template_section()`、`assert_capability_classified()` 辅助函数
7. **创建测试 Fixtures**：在 `openspec/changes/enrich-proposal-with-capabilities/tests/fixtures/` 下创建所有测试数据文件
8. **创建单元测试**：`test_openspec_spec_list.bats` — 覆盖正常调用、CLI 不可用、输出格式异常等场景
9. **创建集成测试**：5 个 Shell 测试脚本覆盖模板结构、prompt 注入移除、Planner 流程、Evaluator 检查、CLI 故障回退
10. **创建端到端测试**：`test_e2e_full_flow.sh` — 覆盖完整 P→E 流程

---

## 待决问题

- 无。本变更的设计决策已在 proposal.md 中充分评审，所有方案选择有明确理由。
