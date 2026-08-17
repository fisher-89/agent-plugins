# dual-platform-plugin-build Specification

## Purpose
Build pipeline that packs plugin JS once and assembles Claude marketplace, Cursor marketplace, and cursor home-image products from a shared source tree with env tokens and hooks profiles.

## Requirements

### Requirement: Root build script packs and assembles dual platform products

The repository SHALL provide a root build entry under `scripts/` and/or the plugin `package.json` `build` script that builds installable Claude Code and Cursor plugin products **and** the `cursorHome` home image from source. The build SHALL:

1. Read the plugin version from `plugins/dev-team/package.json`
2. Run **one** JS packaging pass for mcp / cli / hooks CJS via **vite-plus** using **`vp pack`**, with cwd `plugins/dev-team`, writing staging CJS **without** env/token replacement (string literals retaining `__…__` tokens)
3. Assemble **three** products (`claude`, `cursor`, `cursorHome`) by applying env tokens, renames, and `hooksProfile` / mcp forks into each explicit `outDir`
4. Write platform manifests (where applicable) that inject the same `version` into all products

Assemble SHALL run inside the `vp pack` lifecycle (e.g. `hooks['build:done']` or an equivalent vite-plus/tsdown pack plugin trailer). The build MUST NOT multiply pack entries by product count (MUST NOT put `cursorHome` into a `SUPPORT_AGENTS`-style pack multiplier). The build MUST NOT perform per-env token `define`/transform during bundling.

The root/plugin build orchestration SHALL NOT introduce a second bundler. Packaging MUST remain vite-plus / `vp pack`.

#### Scenario: Build refreshes three product trees

- **WHEN** the plugin build is executed successfully
- **THEN** `claude-plugins/dev-team`, `cursor-plugins/dev-team`, and `cursor-home-image/dev-team` SHALL exist as assembled trees
- **AND** each tree SHALL include the packed JS artifacts required for its install path

#### Scenario: Same version injected into all products

- **WHEN** `plugins/dev-team/package.json` has `"version": "2.10.3"`
- **AND** the build completes successfully
- **THEN** `claude-plugins/dev-team/.claude-plugin/plugin.json` SHALL contain `"version": "2.10.3"`
- **AND** `cursor-plugins/dev-team/.cursor-plugin/plugin.json` SHALL contain `"version": "2.10.3"`
- **AND** the home image metadata SHALL record the same version

#### Scenario: Source remains free of platform manifests after build

- **WHEN** the build completes successfully
- **THEN** `plugins/dev-team` MUST NOT contain a `.claude-plugin/` or `.cursor-plugin/` directory

#### Scenario: JS pack uses a single vp pack pass without token replacement

- **WHEN** the build runs the JS packaging step
- **THEN** it SHALL invoke `vp pack` (vite-plus) with cwd `plugins/dev-team`
- **AND** staging CJS for mcp / cli / hooks SHALL retain unresolved `__…__` string tokens when present in source
- **AND** it MUST NOT invoke a different bundler for those CJS artifacts
- **AND** it MUST NOT run one full pack graph per product env

### Requirement: Dual products are committed marketplace sources

The directories `claude-plugins/`, `cursor-plugins/`, and `cursor-home-image/` SHALL be tracked in git (not ignored) so that cloning the repository yields installable marketplace sources and a copyable home image without a separate publish step.

#### Scenario: Product directories are not gitignored

- **WHEN** examining repository ignore rules
- **THEN** `claude-plugins/`, `cursor-plugins/`, and `cursor-home-image/` MUST NOT be ignored by `.gitignore`
- **AND** a successful build's product trees SHALL be eligible to commit

#### Scenario: Claude product exposes Claude plugin manifest

- **WHEN** examining `claude-plugins/dev-team`
- **THEN** the tree SHALL contain `.claude-plugin/plugin.json` with `"name": "dev-team"`

#### Scenario: Cursor product exposes Cursor plugin manifest

- **WHEN** examining `cursor-plugins/dev-team`
- **THEN** the tree SHALL contain `.cursor-plugin/plugin.json` with `"name": "dev-team"`

#### Scenario: Home image coexists with marketplace Cursor product

- **WHEN** examining repository product directories after build
- **THEN** `cursor-plugins/dev-team` and `cursor-home-image/dev-team` SHALL both exist
- **AND** neither product SHALL replace the other as the sole Cursor delivery

### Requirement: Cursor plugin directory is not ignored by dot-directory rules

The repository `.gitignore` SHALL explicitly un-ignore `.cursor-plugin` (e.g. `!.cursor-plugin`) so that the Cursor marketplace manifest can be versioned despite a broad `.*` ignore rule.

#### Scenario: .cursor-plugin is trackable

- **WHEN** `.cursor-plugin/marketplace.json` is added under the repository root
- **THEN** git ignore rules SHALL allow tracking that path
- **AND** `.gitignore` SHALL contain an exception for `.cursor-plugin`

### Requirement: Unified env table and placeholder tokens

源内容 SHALL 仅在字符串字面量内使用逻辑 id 与 `__<KIND>:<logical-id>__` 形式占位符（markdown、JSON 片段、脚本、TypeScript 字符串）。禁止函数形占位符（如 `skill('id')`）。占位符 MUST NOT 作为标识符或 import 路径出现。

支持的名称类 KIND 与绑定：

| KIND | Expansion |
|------|-----------|
| `SKILL` | `namePrefix + id` |
| `CALL_SKILL` | `pluginPrefix + namePrefix + id` |
| `AGENT` | `namePrefix + id` |
| `CALL_AGENT` | `pluginPrefix + namePrefix + id` |
| `MCP` | `mcpToolPrefix + id`（tool id 必需） |
| `BIN` | `namePrefix + id + '.cjs'`（由 `namePrefix` 派生） |

另支持构建期 `__INCLUDE:<id>__`（语义见能力 `include-fragments`）；它不是上表名称类 token，但 MUST 在 `applyEnvTokens` 之前展开完毕。

路径 token `__DEV_TEAM_ROOT__` / `__DEV_TEAM_RUNTIME_ROOT__` 绑定 `contentRoot` / `runtimeRoot`。对 `claude` / `cursor`，路径 token SHALL 在 assemble 期展开；对 `cursorHome`，路径 token SHALL 保留至安装期。

逻辑 id SHALL 由文件系统发现（`skills/<id>/`、`agents/<id>.md`、staging `bin/{mcp,cli,hooks}.cjs`）——构建 MUST NOT 以手写 id 目录为权威。Token 替换 SHALL 使用宽扫描 include globs（skills/agents/hooks/templates/utils 文本、组装 JSON、staging `bin/*.cjs`）与窄排除（如二进制、sourcemap）。

磁盘上 skill/agent/bin 文件名 SHALL 由 `namePrefix` 派生（plugin env → `mcp.cjs` / `cli.cjs` / `hooks.cjs`；`cursorHome` → `dev-team_mcp.cjs` 等）。

#### Scenario: Assemble expands skill token per env

**WHEN** 源含 `__SKILL:phase-proposal__`
**AND** assemble 对 `claude` 或 `cursor`（`namePrefix=''`）运行
**THEN** 写出文本 SHALL 含 `phase-proposal`
**AND** 对 `cursorHome`（`namePrefix='dev-team_'`）运行时写出文本 SHALL 含 `dev-team_phase-proposal`

#### Scenario: Assemble expands call-skill token per env

**WHEN** 源含 `__CALL_SKILL:propose__`
**AND** assemble 对 `claude` 或 `cursor`（`pluginPrefix='dev-team:'`，`namePrefix=''`）运行
**THEN** 写出文本 SHALL 含 `dev-team:propose`
**AND** 对 `cursorHome`（`pluginPrefix=''`，`namePrefix='dev-team_'`）运行时写出文本 SHALL 含 `dev-team_propose`

#### Scenario: Assemble expands agent and call-agent tokens per env

**WHEN** 源含 `__AGENT:proposal-planner__` 与 `__CALL_AGENT:proposal-planner__`
**AND** assemble 对 `cursor`（`namePrefix=''`，`pluginPrefix='dev-team:'`）运行
**THEN** `__AGENT:…__` SHALL 变为 `proposal-planner`，`__CALL_AGENT:…__` SHALL 变为 `dev-team:proposal-planner`
**AND** 对 `cursorHome`（`namePrefix='dev-team_'`，`pluginPrefix=''`）运行时二者 SHALL 均变为 `dev-team_proposal-planner`

#### Scenario: MCP token requires tool id

**WHEN** 源或 frontmatter 引用 MCP 工具
**THEN** SHALL 使用 `__MCP:<snake_tool_id>__`
**AND** MUST NOT 使用仅 server 的 token 或无 tool id 的裸 server 白名单

#### Scenario: Bin filename follows namePrefix

**WHEN** assemble 为 `cursor`（`namePrefix=''`）写入 bin
**THEN** 输出文件名 SHALL 为 `mcp.cjs`、`cli.cjs`、`hooks.cjs`（非 `dev-team-mcp.cjs`）
**AND** 为 `cursorHome` 写入时 SHALL 为 `dev-team_mcp.cjs`、`dev-team_cli.cjs`、`dev-team_hooks.cjs`

#### Scenario: Build asserts no leftover name-class tokens

**WHEN** 某 product env 的 assemble 结束
**THEN** 该产物树 MUST NOT 含未解析的 `__SKILL:`、`__CALL_SKILL:`、`__AGENT:`、`__CALL_AGENT:`、`__MCP:`、`__BIN:` 或 `__INCLUDE:` token
**AND** 对 `cursorHome`，路径 token MAY 仍保留

### Requirement: hooksProfile assembles platform hook formats from canonical metadata

源码 SHALL 维护 hooks **canonical metadata**（逻辑事件、matchers、含 `__BIN:` / `__DEV_TEAM_RUNTIME_ROOT__` 的 command 模板、可选 `loop_limit`、description）。Assemble SHALL 据此发射平台文件：

- `claudeNested` → nested `hooks` → `PreToolUse` / `SubagentStop` 与内层 `hooks[]`，用于 **`claude`** env，路径 `hooks/hooks.json`
- `cursorNative` → `version` + camelCase 事件、扁平 `command` / `matcher` / `loop_limit`，用于 **`cursor`（marketplace）与 `cursorHome`**，路径分别为 `hooks/hooks.json` 与 image-root `hooks.json`

选择规则 SHALL 为：`env.agent === 'claude'` 时使用 `buildClaudeNested`，否则使用 `buildCursorNative`。MUST NOT 再将 marketplace `cursor` 描述为 nested 形态（纠正相对实现的漂移）。

构建 MUST NOT 把已完成的 Claude nested `hooks.json` 当作 Cursor-native 的变换源（禁止「字符串替换 Claude JSON → Cursor JSON」）。Matchers 可按 profile 不同（例如 Claude `Write|Edit` / `Bash` / `PowerShell` vs Cursor `Write|StrReplace` / `Shell`）。

对任一事件数组：`matchers.<platform>` 为 `null` 的条目 SHALL 被过滤；若过滤后为空，`cursorNative` SHALL **省略该事件键**，MUST NOT 写入空数组（与省略 `subagentStop` 整键对齐；`preToolUse` 既有过滤行为保持）。

#### Scenario: Claude product receives nested hooks file

**WHEN** assemble 针对 `claude` 运行
**THEN** SHALL 以 `claudeNested` 形态写入 `hooks/hooks.json`

#### Scenario: Marketplace cursor product receives native hooks file

**WHEN** assemble 针对 `cursor` 运行
**THEN** SHALL 以 `cursorNative` 形态写入 `hooks/hooks.json`
**AND** MUST NOT 使用 `claudeNested` 形状

#### Scenario: Home product receives native hooks file

**WHEN** assemble 针对 `cursorHome` 运行
**THEN** SHALL 以 `cursorNative` 形态写入根目录 `hooks.json`
**AND** command 模板 MAY 仍保留供安装期展开的路径 token

#### Scenario: Canonical source is not Claude finished JSON authority

**WHEN** 检查本变更后的插件源码树
**THEN** hooks canonical metadata SHALL 为组装权威
**AND** 已检入的 Claude 成品 `hooks/hooks.json` MUST NOT 作为经字符串替换派生 `cursorNative` 的唯一变换输入

#### Scenario: 过滤后空事件键被省略

**WHEN** canonical 某事件在当前平台 matcher 全部为 `null`
**AND** `buildCursorNative` 生成文档
**THEN** 输出 `hooks` 对象 MUST NOT 包含该事件键

### Requirement: assemble 在 env token 前展开 `__INCLUDE`

对 outDir 内已复制的可扫描文本，assemble SHALL 在 `applyEnvTokens` 之前执行 `expandIncludes`。fragment 文件 MUST 始终从源码树 `plugins/dev-team/_fragments/` resolve，MUST NOT 从 outDir 或「相对宿主文件路径」解析。

`copyAgents` / `copySkills` 及其它静态 copy 清单 MUST NOT 将 `_fragments/` 纳入产物。

#### Scenario: 展开顺序

**WHEN** 某 agent 文件同时含 `__INCLUDE:static-analysis-gate__` 与名称类 token
**AND** assemble 处理该文件
**THEN** include SHALL 先展开
**AND** 再对结果调用 `applyEnvTokens`

#### Scenario: fragment 始终读源根

**WHEN** include 在 outDir 内已 copy 的 agent 文本上展开
**THEN** 读取的 fragment 路径 SHALL 指向源码 `plugins/dev-team/_fragments/…`
**AND** MUST NOT 要求 outDir 内存在 `_fragments/`

### Requirement: Explicit outDir per env

Each env SHALL declare an explicit `outDir`. The build MUST NOT derive product directories solely by concatenating `` `${agent}-plugins` ``. At minimum:

| env | outDir |
|-----|--------|
| `claude` | `../../claude-plugins/dev-team` |
| `cursor` | `../../cursor-plugins/dev-team` |
| `cursorHome` | `../../cursor-home-image/dev-team` |

#### Scenario: cursorHome outDir is explicit

- **WHEN** assemble runs for `cursorHome`
- **THEN** outputs SHALL be written under `cursor-home-image/dev-team`
- **AND** the path MUST NOT be invented by appending `-plugins` to an agent key

### Requirement: Token narrow exclusion no longer mentions openspec-bundled.js

The token scanning narrow exclusion list SHALL NOT include `openspec-bundled.js`, as that file no longer exists in the source tree.

#### Scenario: No openspec-bundled.js exclusion in token scan

- **WHEN** examining the token scanning exclusion list in `assemble.ts` or `scan-files.ts`
- **THEN** `openspec-bundled.js` SHALL NOT appear in the exclusion list
- **AND** the token scanning SHALL still exclude binary files and sourcemaps as before

## Module Contract

### Pack + Assemble pipeline

| 阶段 | 输入 | 输出 | 约束 |
|------|------|------|------|
| `vp pack`（一轮 multi-entry） | `bin/src/{mcp,cli,hooks}.ts` | staging `bin/{mcp,cli,hooks}.cjs` | **不**做 env/token 替换；minify 不得改写字符串字面量中的 token |
| assemble×3（pack 生命周期内） | staging CJS + 静态资产 + env 表 | 三产物 `outDir` | 同一 `applyEnvTokens`；FS 权威 rename；宽 globs |
| staging 目录 | — | 如 `.pack-staging/` | gitignore |

### Env 表（字段摘要）

| 字段 | `claude` | `cursor` | `cursorHome` |
|------|----------|----------|--------------|
| `contentRoot` / `runtimeRoot` | `${CLAUDE_PLUGIN_ROOT}` | `.` | 镜像保留；安装期绝对路径 |
| `mcpToolPrefix` | `mcp__plugin_dev-team_dev-team__` | 同左（现网） | `mcp__user-dev-team_mcp__` |
| `namePrefix` | `''` | `''` | `dev-team_` |
| `pluginPrefix` | `dev-team:` | `dev-team:` | `''` |
| `hooksFilePath` | `hooks/hooks.json` | `hooks/hooks.json` | `hooks.json` |
| `mcpFilePath` | `.mcp.json` | `mcp.json` | `mcp.json` |
| `outDir` | `../../claude-plugins/dev-team` | `../../cursor-plugins/dev-team` | `../../cursor-home-image/dev-team` |
| `pathReplacePhase` | `build` | `build` | `install` |

### Function: `applyEnvTokens(text, env, options?)`

| 方面 | 描述 |
|------|------|
| **用途** | 对 md/json/脚本/CJS 文本统一展开名称与（按阶段）路径 token |
| **输入** | 源文本 + env 行；可选 `{ pathTokens?: boolean }`（默认跟随 `pathReplacePhase`） |
| **输出** | 展开后文本 |
| **名称 token** | `__SKILL:` / `__CALL_SKILL:` / `__AGENT:` / `__CALL_AGENT:` / `__MCP:` / `__BIN:` |
| **前置** | 调用前 MUST 已完成 `expandIncludes`（`__INCLUDE:` 非本函数职责） |
| **禁止** | 函数形占位符；无 tool id 的 MCP server token；已废弃的 `__SKILL_SLASH:` |

### Function: `buildHooksFile(canonical, env)`（更新）

| 方面 | 描述 |
|------|------|
| **claude** | `buildClaudeNested` |
| **cursor / cursorHome** | `buildCursorNative` |
| **空事件** | cursorNative 过滤后空列表 → 省略键 |
| **subagentStop** | Cursor matcher null 时不出现在 Cursor 产物 |

### Assemble 文本管道（更新）

| 阶段 | 约束 |
|------|------|
| copy 静态资产 / agents / skills | 不含 `_fragments/` |
| `expandIncludes` | 读源根 fragments；环/缺文件失败 |
| `applyEnvTokens` | 名称 + 按阶段路径 token |
| `assertNoNameTokens` | 含残留 `__INCLUDE:` 失败 |

### CLI: 构建入口（更新）

| 方面 | 描述 |
|------|------|
| **打包** | `vp pack` 一轮；cwd `plugins/dev-team` |
| **组装** | assemble(`claude`\|`cursor`\|`cursorHome`) |
| **输出** | 三产物目录；同一 `version` |
| **禁止** | 第二 bundler；按产品倍增 pack；bundle 期 token define |

### Manifest: `claude-plugins/dev-team/.claude-plugin/plugin.json`

| 字段 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `name` | `string` | 插件名，值为 `"dev-team"` | 由构建写入产物 |
| `version` | `string` | semver，与源 `package.json` 一致 | 由构建注入 |
| 其他元数据 | — | description / author / bin 等 | 由构建从源约定生成或复制 |

### Manifest: `cursor-plugins/dev-team/.cursor-plugin/plugin.json`

| 字段 | 类型 | 描述 | 变更 |
|------|------|------|------|
| `name` | `string` | 插件名，值为 `"dev-team"` | 由构建写入产物 |
| `version` | `string` | semver，与源 `package.json` 一致 | 由构建注入 |
| 其他元数据 | — | 平台所需字段 | 由构建写入；本迭代允许与 Claude 侧内容同构 |
