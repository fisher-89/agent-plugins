# 下线 openspec-bundled.js

## 目标

下线 `plugins/dev-team/bin/openspec-bundled.js`（~2.7MB，`@fission-ai/openspec` 全量 esbuild 打包），
用 plugin 自身的 MCP 工具 / CLI 替代其剩余功能。方向与 `migrate-archi-decide-to-mcp`（已 archive，30/30 完成）同构。

## 现状调查

### bundle 是什么

```
bin/openspec-bundled.js   (~2.7MB，esbuild 打包 @fission-ai/openspec 全量，含 posthog 遥测)
  ├── bin/openspec        (bash wrapper: node "$SCRIPT_DIR/openspec-bundled.js" "$@")
  └── bin/openspec.cmd    (Windows wrapper: node "%SCRIPT_DIR%openspec-bundled.js" %*)
```

承载完整 openspec CLI（`new / status / spec / list / change / validate / init / ...` 20+ 子命令）。

### 真实调用面（只有 3 处 CLI 命令）

| openspec 命令 | 调用点 | 现有替代 |
|---|---|---|
| `new change "<name>"` | `phase-proposal` / `workflow-requirement` / `workflow-test-only` 三处 SKILL.md | 需新增 `change_create` MCP 工具 |
| `status --change "<name>" --json` | `openspec-archive-change` SKILL.md | `change_list` MCP 已返回 artifacts + tasks + latest_phase，**缺 schemaName** |
| `status --json` | `openspec-archive-change` SKILL.md（guardrail 注释） | 同上 |
| `spec list --json` | `proposal-planner.md`（经 `utils/openspec-cli.sh` 的 `openspec_spec_list`） | 需 `spec_list` MCP 工具（纯 FS 扫 `openspec/specs/<capability>/spec.md`） |

### `new change` 的真实语义（源码级）

反编译 `createChange()`（bundled.js:60977）+ `newChangeCommand()`（61514）：

1. `validateChangeName` → kebab-case 校验（`^[a-z][a-z0-9]*(-[a-z0-9]+)*$`）
2. 确定 schema（options.schema → config.schema → 默认 `spec-driven`）
3. `validateSchemaName`
4. 检查 `openspec/changes/<name>/` 是否已存在（存在则报错）
5. `mkdir openspec/changes/<name>/`
6. 写 `.openspec.yaml`（仅两字段）：
   ```yaml
   schema: spec-driven
   created: YYYY-MM-DD
   ```
7. 可选 `--description` 写 README.md

**结论**：纯文件系统操作，无模板、无隐藏依赖。skill 调用从不传 `--schema`/`--description`，全走默认。
`change_create` 只需 mkdir + 写元数据文件。

### `.openspec.yaml` 可以整个移除（已决策）

证据：`.openspec.yaml` 只被即将删除的 bundle CLI 读取（`METADATA_FILENAME`），plugin 自身 `bin/src/` 零引用（Grep 全目录确认）。
唯一额外命中是 `openspec-archive-change/SKILL.md:114` 的一句 guardrail 文案（从 bundle 归档逻辑抄来，需一并改）。

- `schema` 字段 / `schemaName` 概念只服务于 `@fission-ai/openspec` 的 `status`/`list` 输出；plugin 自己的 `change_list` 从未返回过 `schemaName`。
- 落点：`workflow.json`（plugin 原生元数据，`bin/src/lib/change-config.ts` 的 `getWorkflowType()` 已读它）。
- 因此 `change_create` 职责从「mkdir + 写 `.openspec.yaml`」变为「mkdir + 写 `workflow.json`」：

  ```json
  { "workflow_type": "requirement", "created": "YYYY-MM-DD" }
  ```

  skill 侧「确认 workflow 类型」不变：用户选非 requirement 时覆写 `workflow_type`，`created` 保留。
- `created` 语义：锁定在 `change_create` 那一刻（与目录创建同瞬间，无歧义）。

### `status` / `spec list` 的替代差距

- `change_list` 已是纯 FS 扫描（`commands/change-list.ts`），返回 artifacts（存在性）/ task 计数 / latest_phase。
  archive skill 的「artifact done 判定」实际就是文件存在性，与 `change_list.artifacts` 等价。
  尚缺「workflow 是否 done」→ 由 `change_list` 新增 `workflow_done` 字段补齐（见「落地设计」第 3 条），无需 schemaName。
- `spec list --json` → 纯 FS 扫 `openspec/specs/*/spec.md`，返回 capability 列表。

### 爆炸半径（删 bundle 要清理）

**源文件（6）**
- `build/assemble.ts` — `STATIC_BIN_FILES` 常量、`copyStaticAssets()`、`writeHomeExtras()` 的 extraManaged
- `home-install.ts` — 对 `openspec-bundled.js` 的 token 展开特判
- `build/scan-files.ts` — `EXCLUDE_BASENAMES = { 'openspec-bundled.js' }`
- `vite.config.ts` — `NO_OXC_FILES` + lint/fmt 排除
- `bin/openspec` / `bin/openspec.cmd` — wrapper 本身

**测试（2）**
- `build/__tests__/assert-no-tokens.test.ts`
- `build/__tests__/scan-files.test.ts`

**产物（3 dirs + manifests）**
- `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`（含各自 manifest.json managedPaths）

**utils**
- `utils/openspec-cli.sh` — 整文件是 openspec CLI 的 shell 封装；现在只有 `openspec_spec_list` 被 proposal-planner 引用，其余函数（`openspec_new_change` / `openspec_status_json` / `openspec_instructions`）已无调用方，可整体删除

## 决策：路线 A（全量下线 + FS 脚手架）

理由：
- `new change` / `status` / `spec list` 三者都是纯 FS 操作，MCP 工具可 1:1 替代，无功能损失
- 彻底消除 2.7MB bundle 依赖，符合 CLAUDE.md 的「slim plugin」架构
- 有 `migrate-archi-decide-to-mcp` 直接先例

**架构张力（需在提案中明确）**：这反转了 `2026-05-13-embed-openspec-remove-likec4` 决议（目标「用户零手动步骤」）。
下线后 plugin 完全不依赖外部 openspec CLI —— change 状态/脚手架/spec 列表全部由 MCP/CLI 自管。
需更新 `openspec/specs/embedded-cli/spec.md`（当前仍描述「plugin bundles openspec CLI」）。

## 落地设计

1. **新增 MCP 工具 `change_create`**（对称 `change_list`）
   - input: `name`（kebab-case 校验复用现有逻辑或新写）
   - 行为：mkdir + 写 `workflow.json`（`workflow_type: "requirement"` + `created: YYYY-MM-DD`），不写 `.openspec.yaml`
   - 已存在 → 报错
2. **新增 MCP 工具 `spec_list`**（不再补 schemaName —— schema 概念随 `.openspec.yaml` 一起消失）
   - 扫 `openspec/specs/*/spec.md`，返回 capability 列表（纯数据，1:1 复刻 bundle 的 `spec list --json`）
   - 「新增 vs 修改」分类**留在 agent 侧**（`proposal-planner.md:33` 现状即 agent 判断，bundle 只返回列表），MCP 不下沉该逻辑
3. **扩展 `change_list`：新增 `workflow_done` 布尔字段**
   - 计算方式：读 `workflow.json` 得 `workflow_type` → 查 phase 表（`lib/workflow.ts` 的 `getPhaseTable`）→ 读 `eval.json` → 判断所有 phase 是否都有非 stale 的 pass/skipped（复用 `phase-next.ts` 的 `hasPhasePassed` 语义）
   - archive skill 仅消费这个字段，不再依赖任何 phase 门控工具
4. **改 skill/agent 调用点**：`openspec new change "<name>"` → `__MCP:change_create__`；`openspec status --json` → `__MCP:change_list__`（artifact 完成度 = `artifacts`，workflow done = `workflow_done`）；`openspec spec list --json` → `__MCP:spec_list__`；archive skill 删除 `__MCP:phase_check__`（见下方「已决策」）
5. **删 bundle + wrapper + openspec-cli.sh**，清理 assemble/scan-files/vite/home-install 的特判
6. **更新 spec**：`embedded-cli` capability 标记 REMOVED 或改写为「不再内嵌 openspec CLI」

## 已决策（原开放问题，全部落定）

1. **`created` 用真实 `new Date()`**：纯 TS MCP 代码，无 workflow 脚本的 Date 限制。同意。
2. **archive 不依赖 `phase_next`**：由 `change_list` 返回 `workflow_done`，archive 只消费该状态。
   - 关键证据：`__MCP:phase_check__` **根本不是真实 MCP 工具**（现有 12 工具无此名）——archive skill 是一段早已失效的幽灵引用，归档时的「eval 链校验」实际从未真正运行。删除它无功能损失。
3. **「新增 vs 修改」分类在 agent 侧**：现状即 `proposal-planner.md` 内 agent 判断，bundle 只返回 capability 列表。`spec_list` MCP 只复刻列表，分类逻辑不下沉。

## 补充：proposal.md 结构分歧（中英文 header）

用户提出：「archive 依赖 proposal.md 的文档结构，但 proposal 模板已调整为中文，整改时顺便消除分歧」。源码级追踪后的结论如下。

### archive 有两条链，结构依赖不同

```
                        proposal.md 文档结构
                                │
        ┌───────────────────────┴───────────────────────┐
   [A] plugin archive SKILL                       [B] bundle 的 openspec archive 命令
   (openspec-archive-change/SKILL.md)             (bundled.js ArchiveCommand)
        │  step2: openspec status --json                 │  validateChange()
        │  step5: 自己 mkdir + mv                        │  → MarkdownParser.parseChange()
        │                                               │  → 需 ## Why / ## What Changes
        │  ✗ 只查文件存在性(detectCompleted,             ✓ 解析章节结构(英文精确匹配)
        │    glob 匹配 artifact.generates)                │
   结构不敏感                                      中文标题永远匹配不到 → 抛错
```

- **plugin 的 archive SKILL 不解析 proposal 章节**。step2 的 `openspec status --json` 走 `detectCompleted`（bundled.js:60559），只做 `fs.existsSync` / glob 匹配 `artifact.generates`（`proposal.md`、`specs/**`），不看标题。step5 自己搬目录，不碰 `ArchiveCommand`。
- **bundle 的 `openspec archive` 会解析 proposal，但对中文标题不阻断**。`validateChange` 内部 try/catch 把抛错转成 ERROR issue → `changeReport.valid=false` → 只打印黄色 "Proposal warnings (non-blocking)"（54876）继续归档。真正能 abort 的是 delta-spec ERROR（54909），而 delta 标题（`## ADDED Requirements` 等）plugin 本来就英文，不会触发。

### 中文模板 vs CLI 英文 header（findSection 精确匹配）

`findSection`（bundled.js:53531）是 `title.toLowerCase() === title.toLowerCase()` 精确相等，非语义匹配；`parseChange`（53468）/`parseChangeWithDeltas`（53852）在 `!why`/`!whatChanges` 时直接 `throw "Change must have a Why section"`。

| plugin 模板（中文） | bundle CLI 期望（英文） | 等价？ |
|---|---|---|
| `## 问题` | `## Why` | ✗ |
| `## 提案` | `## What Changes` | ✗ |
| `## 能力` / `### 新增能力` / `### 修改的能力` | `## Capabilities` / `### New Capabilities` / `### Modified Capabilities` | ✗ |
| `## 变更范围` | `## Impact` | ✗ |
| `## 验收标准` / `## 风险` / `## 过程` | （无对应） | CLI 模板根本没有 |

`ChangeSchema`（53831）还要求 `why`（min 长度）/`whatChanges`（min 1）/`deltas`（min 1），这套字段只从英文 header 抽取。

### 真正硬失败的命令（但没人调）

`openspec validate`（proposal 报 invalid）、`openspec show`/`view`/`change`（`parseChangeWithDeltas` 无 try/catch 直接 throw）都会因中文标题崩。但 plugin 的真实调用面只有 `new change` / `status --json` / `spec list --json`（见上文「真实调用面」表），一个都不调这些命令 → 分歧在现状下是「幽灵」。

### 结论：分歧随下线自动消失，无需改回英文

分歧根源不是「模板写错」，而是 bundle 里硬编码了一套英文 header 的 parser + `ChangeSchema`，与中文模板并存。二者不可能靠「对齐标题」消除——那套是 `@fission-ai/openspec` 固定契约，改 CLI 等于 fork bundle，违背 slim 架构。

因此「消除分歧」是 `retire-openspec-bundled` 整改的**自然产物**，不单独做：

```
下线 openspec-bundled.js
   ├── 删 MarkdownParser.parseChange / ChangeParser.parseChangeWithDeltas
   ├── 删 ChangeSchema（why/whatChanges 的 min 校验）
   ├── 删 ArchiveCommand.validateChange 里的 proposal 校验
   └── 保留 change_create / spec_list / change_list(+workflow_done)
        ↓ 中文 proposal 模板成为唯一事实源，
          消费方只剩 proposal-planner / proposal-evaluator（都已按中文标题写）
        ↓ 分歧自动消失
```

**落点**：本次 change 的 proposal/验收标准需显式列出「删除英文 header parser（`parseChange`/`parseChangeWithDeltas`/`ChangeSchema`）+ 移除 archive 命令的 proposal 校验」，作为消除分歧的验收点。
