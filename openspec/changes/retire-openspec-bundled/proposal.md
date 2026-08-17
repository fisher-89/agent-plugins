# 提案: retire-openspec-bundled

> **变更**: retire-openspec-bundled
> **日期**: 2026-08-14
> **状态**: 起草

---

## 问题

`plugins/dev-team/bin/openspec-bundled.js` 是一个约 2.7MB 的 esbuild 全量打包，内含 `@fission-ai/openspec` 的完整 CLI（20+ 子命令）及 posthog 遥测。然而，plugin 的真实调用面只有 3 个 CLI 命令：

| openspec 命令 | 调用点 | 性质 |
|---|---|---|
| `new change "<name>"` | phase-proposal / workflow-requirement / workflow-test-only 三处 skill | 纯文件系统操作（mkdir + 写元数据） |
| `status --change <name> --json` | openspec-archive-change skill | 纯文件系统扫描（读 eval.json + 检查文件存在性） |
| `spec list --json` | proposal-planner agent | 纯文件系统扫描（读 openspec/specs/*/spec.md） |

保留 2.7MB bundle 仅为了这三个纯 FS 操作，与 CLAUDE.md 声明的「slim plugin」架构严重矛盾：

- 增加构建时间（bundle 全量打包）
- 增加产物体积（2.7MB 压缩前的 JS 被复制到每个产物目录）
- 引入未使用的 posthog 遥测依赖
- 需要构建管线中的多处特判（`EXCLUDE_BASENAMES`、`NO_OXC_FILES`、`STATIC_BIN_FILES`、token 展开特判）
- `openspec` 的 bash/CMD wrapper 及 `utils/openspec-cli.sh` 封装增加维护负担

此外，`new change` 命令写入的 `.openspec.yaml` 只被 bundle 自身读取，plugin 的 `bin/src/` 零引用（Grep 全目录确认），是 dead metadata。

---

## 提案

下线 `openspec-bundled.js` 及其 wrapper，用 3 个 MCP 工具替代其全部功能，配套清理构建管线中的特判。

**核心变更：**

1. **新增 `change_create` MCP 工具**：替代 `openspec new change`，行为为 mkdir + 写 `workflow.json`（`workflow_type: "requirement"` + `created: YYYY-MM-DD`），不再写 `.openspec.yaml`。校验 kebab-case 名称格式，拒绝已存在的 change 目录。

2. **新增 `spec_list` MCP 工具**：替代 `openspec spec list --json`，扫描 `openspec/specs/*/spec.md` 返回 capability 列表。新增 vs 修改的分类逻辑留在 agent 侧（现状即 agent 判断，MCP 不下沉）。

3. **扩展 `change_list` 返回 `workflow_done` 字段**：替代 `openspec status --json` 的 workflow 完成状态判定。计算方式：读 `workflow.json` 得 `workflow_type` → 查 `lib/workflow.ts` 的 `getPhaseTable` → 读 `eval.json` → 复用 `phase-next.ts` 的 `hasPhasePassed` 语义判断所有 phase 是否都有非 stale 的 pass/skipped。archive skill 仅消费这个字段，不再依赖任何 phase 门控工具。

4. **删除 bundle + wrapper + openspec-cli.sh**：删除 `bin/openspec`、`bin/openspec.cmd`、`plugins/dev-team/utils/openspec-cli.sh`；清理 `assemble.ts`、`scan-files.ts`、`vite.config.ts`、`home-install.ts` 中的特判。

5. **更新 agent/skill 调用点**：`proposal-planner.md` 将 `source .../openspec-cli.sh && openspec_spec_list` 改为 `__MCP:spec_list__`；archive skill 将 `openspec status --json` 改为 `change_list` 的 `workflow_done` 字段。

6. **中英文 header 分歧自然消除**：bundle 的 `MarkdownParser.parseChange` / `ChangeParser.parseChangeWithDeltas` / `ChangeSchema` 随 bundle 删除，中文 proposal 模板成为唯一事实源。

---

## 能力

### 新增能力

- `change_create` — 新增 MCP 工具，替代 `openspec new change` 的纯 FS 脚手架操作。校验名称、创建目录、写 workflow.json 元数据
- `spec_list` — 新增 MCP 工具，扫描 `openspec/specs/*/spec.md` 返回 capability 列表

### 修改的能力

- `change_list` — 扩展返回 `workflow_done` 布尔字段，替代 `openspec status --json` 的 workflow 完成状态判定
- `embedded-cli` — 从「内嵌 openspec CLI」改为「不再内嵌 openspec CLI」，删除 bundle 和相关 wrapper
- `dual-platform-plugin-build` — 移除 `openspec-bundled.js` 窄排除引用
- `cursor-home-image` — 移除 installer 中 `openspec-bundled.js` 的 token 展开特判

---

## 变更范围

### 实现文件

| 操作 | 文件 | 说明 |
|------|------|------|
| 新增 | `plugins/dev-team/bin/src/commands/change-create.ts` | change_create MCP 工具实现 |
| 新增 | `plugins/dev-team/bin/src/commands/spec-list.ts` | spec_list MCP 工具实现 |
| 修改 | `plugins/dev-team/bin/src/mcp.ts` | 注册 change_create、spec_list 工具；扩展 change_list handler 返回 workflow_done |
| 修改 | `plugins/dev-team/bin/src/commands/change-list.ts` | 新增 `workflow_done` 计算逻辑 |
| 修改 | `plugins/dev-team/bin/src/schemas/change-list.schema.ts` | 新增 `workflow_done: z.boolean()` 字段 |
| 修改 | `plugins/dev-team/build/assemble.ts` | 从 `STATIC_BIN_FILES` 移除 `bin/openspec`、`bin/openspec-bundled.js`、`bin/openspec.cmd`；清理 `copyStaticAssets`、`writeHomeExtras` 中的引用 |
| 修改 | `plugins/dev-team/build/scan-files.ts` | 移除 `EXCLUDE_BASENAMES` 中的 `'openspec-bundled.js'` |
| 修改 | `plugins/dev-team/vite.config.ts` | 从 `NO_OXC_FILES` 移除 `'bin/openspec-bundled.js'` |
| 修改 | `plugins/dev-team/home-install.ts` | 移除 `openspec-bundled.js` 的 token 展开特判 |
| 修改 | `plugins/dev-team/agents/proposal-planner.md` | 将 `source .../openspec-cli.sh && openspec_spec_list` 改为 `__MCP:spec_list__` |
| 修改 | `openspec/specs/openspec-archive-change/SKILL.md` | 移除 `__MCP:phase_check__` 幽灵引用；将 `openspec status --json` 改为 `__MCP:change_list__` 的 `workflow_done` |
| 删除 | `plugins/dev-team/bin/openspec` | bash wrapper |
| 删除 | `plugins/dev-team/bin/openspec.cmd` | Windows wrapper |
| 删除 | `plugins/dev-team/utils/openspec-cli.sh` | openspec CLI 的 shell 封装（仅 `openspec_spec_list` 仍被引用，由 spec_list MCP 替代） |

### 测试文件

| 操作 | 文件 | 说明 |
|------|------|------|
| 修改 | `plugins/dev-team/build/__tests__/assert-no-tokens.test.ts` | 移除 `openspec-bundled.js` 的 token 白名单测试用例 |
| 修改 | `plugins/dev-team/build/__tests__/scan-files.test.ts` | 移除「排除 openspec-bundled.js」测试用例 |

### 不要修改

- 现有 MCP 工具（除 `change_list` 增加 `workflow_done` 字段外）的接口和行为
- 产物目录结构（`claude-plugins/`、`cursor-plugins/`、`cursor-home-image/`）
- 打包流程的 `vp pack` 阶段（仅修改 assemble 阶段的静态资产清单）
- 非 openspec-bundled 相关的 skill/agent 行为（除 archive skill 的幽灵引用清理外）
- `utils/` 目录下的其他文件（仅 `openspec-cli.sh` 被删除）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | change_create MCP 工具注册 | `mcp.ts` 中注册名为 `change_create` 的工具，`inputSchema` 包含 `name` 字段，`outputSchema` 定义返回结构 |
| AC-2 | change_create 创建目录和元数据 | 调用 `change_create` 后在 `openspec/changes/<name>/` 创建目录，写入 `workflow.json` 包含 `workflow_type: "requirement"` 和 `created` 日期 |
| AC-3 | change_create 校验名称 | 非 kebab-case 名称（如 `My Change`、`my_change`）返回错误，合法名称（如 `my-change`）通过 |
| AC-4 | change_create 拒绝已存在 | 对已存在的 change 名称返回错误，不覆盖或修改已存在目录 |
| AC-5 | spec_list MCP 工具注册 | `mcp.ts` 中注册名为 `spec_list` 的工具，返回 `openspec/specs/*/spec.md` 的 capability 列表 |
| AC-6 | change_list 返回 workflow_done | `change_list` 返回的每个 entry 包含 `workflow_done: boolean` 字段，其值正确反映 eval.json 中所有 phase 的完成状态 |
| AC-7 | openspec-bundled.js 从产物中移除 | 构建后 `claude-plugins/dev-team/bin/` 和 `cursor-plugins/dev-team/bin/` 中不存在 `openspec-bundled.js` |
| AC-8 | bin/openspec 和 bin/openspec.cmd 从产物中移除 | 构建后产物目录中不存在这两个 wrapper 文件 |
| AC-9 | 构建管线特判全部移除 | `assemble.ts` 的 `STATIC_BIN_FILES`、`scan-files.ts` 的 `EXCLUDE_BASENAMES`、`vite.config.ts` 的 `NO_OXC_FILES`、`home-install.ts` 的特判不再引用 openspec-bundled |
| AC-10 | proposal-planner 使用 spec_list MCP | `proposal-planner.md` 中不再引用 `source .../openspec-cli.sh`，改为 `__MCP:spec_list__` |
| AC-11 | utils/openspec-cli.sh 被删除 | 文件 `plugins/dev-team/utils/openspec-cli.sh` 不存在 |
| AC-12 | assert-no-tokens 测试通过 | 移除了 openspec-bundled.js 相关测试用例的测试套件全部通过 |
| AC-13 | 英文 header parser 随 bundle 删除 | 产物中不存在 `MarkdownParser.parseChange`、`ChangeParser.parseChangeWithDeltas`、`ChangeSchema` 相关代码 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 遗漏 bundle 的隐藏调用点（如 `openspec validate` 被某处引用） | 功能异常 | 低 | Explore 已穷举所有源代码引用，真实调用面仅 3 处 CLI 命令，其余命令从未被调用 |
| archive skill 的幽灵引用 `__MCP:phase_check__` 删除后产生未知影响 | 功能异常 | 极低 | 该引用从未是真实 MCP 工具，archive skill 当前阶段已失效，删除无功能损失 |
| 构建产物清理不完整导致残留 bundle | 产物体积未减少 | 低 | `assert-no-tokens` 测试可检测残留 token；验收标准 AC-7/AC-8 明确检查产物 |
| 中英文 header 分歧导致 archive 失败 | archive 功能异常 | 低 | 分歧根源是 bundle 的英文 parser，下线后自动消除；archive 的 proposal 校验本就不阻断归档 |
| `change_list` 的 `workflow_done` 计算与 `phase-next` 的 `hasPhasePassed` 语义不一致 | workflow 状态判定错误 | 低 | 复用 `phase-next.ts` 已有函数，同一代码路径保证语义一致 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| change 元数据写什么文件？ | 写 `workflow.json` 而非 `.openspec.yaml` | `.openspec.yaml` 只被 bundle 读取，`workflow.json` 已是 plugin 原生元数据（`change-config.ts` 的 `getWorkflowType()` 已读它） | 保留 `.openspec.yaml` 但移除 bundle 读取 → 增加 dead code |
| archive 如何判定 workflow 完成？ | `change_list` 返回 `workflow_done` 字段 | 复用已有 `hasPhasePassed` 逻辑，无需新增独立工具 | 新增 `workflow_status` MCP 工具 → 增加表面积 |
| 新增 vs 修改分类谁做？ | 留在 agent 侧 | 现状即 `proposal-planner.md` 内 agent 判断，bundle 只返回 capability 列表 | MCP 下沉分类逻辑 → 增加 agent-MCP 耦合 |
| 中英文 header 分歧如何处理？ | 随 bundle 下线自动消除 | 分歧根源是 bundle 的 `MarkdownParser.parseChange` 硬编码英文 header，与中文模板并存 | 改回英文模板 → 与新 agent 规范冲突 |
| `created` 时间用什么值？ | 真实 `new Date()` | 纯 TS MCP 代码，无 workflow 脚本的 Date 限制 | 固定日期 → 无法反映真实创建时间 |

### 待决问题

- 无

---