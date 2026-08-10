# 设计: cursor-home-image

> **变更**: cursor-home-image
> **日期**: 2026-08-06

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| vite-plus pack（单轮） | 将 `mcp` / `cli` / `hooks` 打成 staging CJS，**不做** env/token 替换；`build:done` 触发 assemble×3 与 schema 生成 | `plugins/dev-team/vite.config.ts` | vite-plus、`build/**` | TypeScript / `vp pack` |
| env 表 | 三套产物（`claude` / `cursor` / `cursorHome`）的前缀、路径、`hooksProfile`、显式 `outDir` | `plugins/dev-team/build/env.ts` | 无 | TypeScript |
| `applyEnvTokens` | 对文本统一展开名称类 /（按阶段）路径 token | `plugins/dev-team/build/apply-env-tokens.ts` | `env.ts` | TypeScript |
| assemble | 读 staging CJS + 静态资产；FS 权威 rename；hooksProfile / mcp 分叉；宽 globs 扫 token；构建断言；写出三产物 | `plugins/dev-team/build/assemble.ts` 等 | `env`、`applyEnvTokens`、hooks 权威源 | TypeScript / Node `fs` |
| hooks 权威源 | 逻辑事件、matcher、command 模板；**非** Claude 成品 JSON | `plugins/dev-team/hooks/hooks.canonical.json` | 无 | JSON |
| hooksProfile 组装 | 从权威源生成 `claudeNested` 或 `cursorNative` | `plugins/dev-team/build/hooks-profile.ts` | `applyEnvTokens`、`env` | TypeScript |
| protect-files（hooks 运行时） | PreToolUse 写保护；识别 Claude + Cursor 工具名 | `plugins/dev-team/bin/src/hooks.ts` | `readConfig`、picomatch | TypeScript → CJS |
| 占位符化源内容 | skills / agents / templates / utils / bin 字符串只写逻辑 id 与 `__<KIND>:<id>__` | `plugins/dev-team/{skills,agents,templates,utils,bin/src}/**` | assemble | Markdown / TS |
| Home 镜像产物 | 可 commit 的 `~/.cursor` 镜像树（前缀化 + 路径 token 保留） | `cursor-home-image/dev-team/` | assemble 写出 | 文件系统产物 |
| Node 安装器 | 路径绝对化、manifest 同步、hooks/mcp managed merge、写状态、Reload 提示 | 源：`plugins/dev-team/build/home-install.mjs` → 产物：`cursor-home-image/dev-team/install.mjs` | Node 内置 `fs`/`path`/`os` | Node ESM |
| 双 marketplace 产物 | 与现网并存；assemble 刷新；plugin bin 为 `{id}.cjs` | `claude-plugins/dev-team/`、`cursor-plugins/dev-team/` | assemble | 文件系统产物 |

### 构建数据流

```
plugins/dev-team/bin/src/{mcp,cli,hooks}.ts  （字符串内 __TOKEN__）
        │
        │  vp pack：单轮 multi-entry（不替换）
        ▼
plugins/dev-team/.pack-staging/bin/{mcp,cli,hooks}.cjs  （仍含 token；gitignore）
        │
        │  build:done → assemble×3 + generateConfigJsonSchema
        ├─ assemble(claude)     → claude-plugins/dev-team/     （路径+名称全展开；hooks/hooks.json）
        ├─ assemble(cursor)     → cursor-plugins/dev-team/     （同上）
        └─ assemble(cursorHome) → cursor-home-image/dev-team/  （名称展开；路径 token 保留；根 hooks.json）
                                        │
                                        ▼ install.mjs（本机）
                                  路径 token → 绝对 ~/.cursor
                                  sync + managed merge hooks/mcp
```

原则：禁止 `SUPPORT_AGENTS × N` 倍增 pack；禁止 bundle 期按 env `define` 换 token；禁止把 `cursorHome` 塞进 pack 乘数；打包仍为 vite-plus / `vp pack`。

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/build/env.ts` | 三行 env 表、`ProductEnv` / `ProductEnvKey` 类型、`getEnv(key)` |
| `plugins/dev-team/build/apply-env-tokens.ts` | `applyEnvTokens`：名称类 + 按阶段路径 token 展开 |
| `plugins/dev-team/build/hooks-profile.ts` | 从 hooks 权威源按 `hooksProfile` 组装成品 JSON |
| `plugins/dev-team/build/assemble.ts` | `assemble` / `assembleAll`：复制、rename、token、mcp、manifest、断言 |
| `plugins/dev-team/build/assert-no-tokens.ts` | 产物内名称类 token 残留检测（`cursorHome` 允许路径 token） |
| `plugins/dev-team/build/scan-files.ts` | 宽 include / 窄 exclude globs 枚举待替换文本文件 |
| `plugins/dev-team/build/home-install.mjs` | 安装器源；assemble 原样复制到 `cursor-home-image/dev-team/install.mjs` |
| `plugins/dev-team/hooks/hooks.canonical.json` | hooks 权威元数据（替代以成品 `hooks.json` 为变换源） |
| `cursor-home-image/dev-team/**` | 构建产物整树（含 `install.mjs`、`manifest.json`、前缀化 skills/agents 等）；纳入 git |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/vite.config.ts` | pack 改为单轮三 entry → `.pack-staging/bin/{mcp,cli,hooks}.cjs`；去掉按 `SUPPORT_AGENTS` 倍增；`build:done` 调 `assembleAll()` + `generateConfigJsonSchema()` | AC-1；不再直接写出 `../../{agent}-plugins/.../bin/dev-team-*.cjs` |
| `plugins/dev-team/build/index.ts` | 导出新模块（env、applyEnvTokens、assemble 等）；调整/保留 schema 导出 | 供 vite hook 与（后续）单测引用 |
| `plugins/dev-team/build/build-agent-artifacts.ts` | 逻辑迁入 `assemble.ts` / `env.ts` 后删除或薄封装转发；移除 `` `${agent}-plugins` `` 拼接与硬编码 `dev-team-mcp.cjs` | 被 assemble×3 取代 |
| `plugins/dev-team/build/build-config-schema.ts` | 保持 `generateConfigJsonSchema`；由 assemble 收尾或 `build:done` 调用；schema 仍写源码 `bin/` | 行为基本不变 |
| `plugins/dev-team/hooks/hooks.json` | 不再作为权威成品提交；删除或改为由 assemble 生成的本地忽略产物（推荐：删除，权威改 `hooks.canonical.json`） | AC-3 |
| `plugins/dev-team/skills/**` | 硬编码 `mcp__plugin_…`、`/dev-team:`、插件限定名 → `__MCP:` / `__SKILL_SLASH:` / `__SKILL:` 等 | AC-2 |
| `plugins/dev-team/agents/**` | 同上；`proposal-evaluator` / `acceptance-evaluator` 的 `tools:` 去掉服务器级 `mcp__plugin_dev-team_dev-team`，改为具体 `__MCP:<tool>__`（至少 `phase_log`） | AC-2 |
| `plugins/dev-team/templates/**`、`plugins/dev-team/utils/**` | 路径与名称硬编码改为 `__DEV_TEAM_ROOT__` / 名称 token | AC-2 |
| `plugins/dev-team/bin/src/**` | 字符串内 token（含 `lib/workflow.ts` 的 `agent_type: '__AGENT:…__'`）；禁止写死仅某一产品最终名 | AC-2 |
| `plugins/dev-team/bin/src/hooks.ts` | `evaluateToolAccess`：`Shell` 走 Bash 检测；`StrReplace` 走 Edit/`file_path` 检测；保留 Write/Edit/Bash/PowerShell | AC-6 实现侧 |
| `claude-plugins/dev-team/`、`cursor-plugins/dev-team/` | 随 assemble 全量刷新；bin 落盘 `mcp.cjs` / `cli.cjs` / `hooks.cjs`；hooks 仍为 `hooks/hooks.json`（`claudeNested`） | AC-1、AC-3 |
| `.gitignore` | 忽略 `.pack-staging/`（及路径）；**不** ignore `cursor-home-image/` | AC-7 |
| `CLAUDE.md` | 升版后重建三产物（claude / cursor / cursorHome） | multi-plugin-layout 文档要求 |
| `plugins/dev-team/package.json` | 按惯例 bump `version` | 与三产物 version 对齐 |

<!-- 测试文件（hooks.test.ts、build 单测）属独立测试阶段；本设计不展开其实现细节。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `getEnv` | `plugins/dev-team/build/env.ts` | 新增 | `function getEnv(key: ProductEnvKey): ProductEnv` | 返回某一产物的完整 env 行 |
| `PRODUCT_ENV_KEYS` | `plugins/dev-team/build/env.ts` | 新增 | `const PRODUCT_ENV_KEYS: readonly ProductEnvKey[]` | `['claude','cursor','cursorHome']` |
| `applyEnvTokens` | `plugins/dev-team/build/apply-env-tokens.ts` | 新增 | `function applyEnvTokens(text: string, env: ProductEnv, options?: { pathTokens?: boolean }): string` | 展开 `__SKILL:` / `__AGENT:` / `__MCP:` / `__SKILL_SLASH:` / `__BIN:`；当 `pathTokens!==false` 且 `env.pathReplacePhase==='build'` 时展开路径 token；`cursorHome` 组装时 `pathTokens: false` |
| `assemble` | `plugins/dev-team/build/assemble.ts` | 新增 | `function assemble(key: ProductEnvKey): Promise<void>` | 组装单一产物到 `env.outDir` |
| `assembleAll` | `plugins/dev-team/build/assemble.ts` | 新增 | `function assembleAll(): Promise<void>` | 依次 `assemble` 三个 env；供 `build:done` 调用 |
| `buildHooksFile` | `plugins/dev-team/build/hooks-profile.ts` | 新增 | `function buildHooksFile(canonical: HooksCanonical, env: ProductEnv): string` | 返回成品 hooks JSON 字符串（已按需 apply 名称 token；路径 token 依 `pathReplacePhase`） |
| `assertNoNameTokens` | `plugins/dev-team/build/assert-no-tokens.ts` | 新增 | `function assertNoNameTokens(rootDir: string, env: ProductEnv): void` | 残留名称类 token 则抛错；`cursorHome` 不要求清除路径 token |
| `generateConfigJsonSchema` | `plugins/dev-team/build/build-config-schema.ts` | 保留 | `function generateConfigJsonSchema(): void` | 写 `bin/dev-team-config.schema.json` |
| `install.mjs`（CLI） | `cursor-home-image/dev-team/install.mjs`（源自 `build/home-install.mjs`） | 新增 | `node install.mjs [--root <path>]` → 进程 exit `0` 成功 | 默认 root=`path.resolve(os.homedir(),'.cursor')`；步骤见数据流 |

<!--
  assemble / install 内部私有辅助（复制、rename、mergeHooks、mergeMcp、写 manifest）不列入。
  `buildAgentArtifacts` / `getOutputPathByAgent` / `AgentType`：删除或不再作为公共 API。
  hooks.ts 的 evaluateToolAccess 等为模块私有，不列入。
-->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ProductEnvKey` | `plugins/dev-team/build/env.ts` | 新增 | `'claude' \| 'cursor' \| 'cursorHome'` |
| `ProductLayout` | `plugins/dev-team/build/env.ts` | 新增 | `'plugin' \| 'home-image'` |
| `PathReplacePhase` | `plugins/dev-team/build/env.ts` | 新增 | `'build' \| 'install'` |
| `ProductEnv` | `plugins/dev-team/build/env.ts` | 新增 | 见数据模型「Env 行」字段 |
| `HooksCanonical` | `plugins/dev-team/build/hooks-profile.ts` | 新增 | 权威 hooks 元数据结构（事件、matcher 族、command 模板、loop_limit 等） |
| `HomeManifest` | `plugins/dev-team/build/assemble.ts`（或独立类型文件） | 新增 | `version`、托管路径清单、mcp/hooks 识别约定等 |
| `AgentType` | `plugins/dev-team/build/build-agent-artifacts.ts` | 删除/替换 | 由 `ProductEnvKey` 取代 |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `pack` | `plugins/dev-team/vite.config.ts` | 修改 | multi-entry 单配置（或等价单次 pack） | staging → `.pack-staging/bin/*.cjs` | 不再 `SUPPORT_AGENTS.flatMap` 六次 pack |
| `hooks['build:done']` | `plugins/dev-team/vite.config.ts` | 修改 | 回调 | `assembleAll` + schema | 在 `vp pack` 生命周期内收尾 |
| `version` | `plugins/dev-team/package.json` | 修改 | `string` | bump（实现时按惯例 +1） | 权威 semver；注入三产物 |
| `.pack-staging/` | `.gitignore` | 新增 | ignore 规则 | — | staging 不入库 |
| `cursor-home-image/` | `.gitignore` | 保持不忽略 | — | — | AC-7 |
| Upgrade 规则文案 | `CLAUDE.md` | 修改 | 文档 | 重建三产物 | 含 `cursor-home-image/` |
| hooks 权威 | `plugins/dev-team/hooks/hooks.canonical.json` | 新增 | JSON | 见数据模型 | PreToolUse / SubagentStop 逻辑定义 |
| MCP server key（home） | `cursor-home-image/.../mcp.dev-team.json` | 新增（构建写入） | `string` | `"dev-team_mcp"` | merge 识别：`dev-team_` 前缀 |
| MCP server key（plugin） | 产物 `.mcp.json` / `mcp.json` | 修改 | `string` | 保持现网 `"dev-team"`（待与现网对照） | args 指向 `namePrefix` 派生的 bin 名 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `ProductEnv` | `key`；`outDir`；`contentRoot`；`runtimeRoot`；`mcpToolPrefix`；`namePrefix`；`agentRefPrefix`；`skillSlashPrefix`；`layout`；`hooksProfile`；`mcpOut`；`pathReplacePhase` | 一对一对应产物树 | 代码内常量（`env.ts`） |
| `PackageVersionAuthority` | `version: string` | 注入两边 `plugin.json` + home `manifest.json` | `plugins/dev-team/package.json` |
| `HooksCanonical` | `description?`；`preToolUse[]: { matchers: { claudeNested: string; cursorNative: string }; commandTemplate: string }`；`subagentStop[]: { agentLogicalId: string; loop_limit?: number; commandTemplate: string }` | assemble → 成品 hooks 文件 | `hooks/hooks.canonical.json` |
| `HomeManifest` | `version`；`namePrefix: "dev-team_"`；`managedPaths: string[]`（skills/agents/bin/templates/utils 相对路径）；`mcpFragment`；`hooksFile` | 约束 install 同步/卸载范围 | `cursor-home-image/dev-team/manifest.json` |
| `HomeInstallState` | `version`；`root`；`installedAt`；`managedPaths` | 安装后写入 | `~/.cursor/dev-team-install.json`（或 manifest 约定路径） |
| `McpFragment`（home） | `mcpServers: { "dev-team_mcp": { type, command, args } }` | args 含路径 token + `__BIN:mcp__` 展开后的文件名 | `mcp.dev-team.json` |
| `PluginMcpConfig` | `mcpServers: { "dev-team": { … args: runtimeRoot/bin/mcp.cjs } }` | plugin 产物 | `.mcp.json` / `mcp.json` |

### Env 行（锁定值）

| 字段 | `claude` | `cursor` | `cursorHome` |
|------|----------|----------|--------------|
| `outDir` | `../../claude-plugins/dev-team` | `../../cursor-plugins/dev-team` | `../../cursor-home-image/dev-team` |
| `contentRoot` / `runtimeRoot` | `${CLAUDE_PLUGIN_ROOT}` | `.` | 镜像保留 token；安装期同绝对路径 |
| `mcpToolPrefix` | `mcp__plugin_dev-team_dev-team__` | 同左 | `mcp__user-dev-team_mcp__` |
| `namePrefix` | `''` | `''` | `dev-team_` |
| `agentRefPrefix` | `dev-team:` | `dev-team:` | `dev-team_` |
| `skillSlashPrefix` | `/dev-team:` | `/dev-team:` | `/dev-team_` |
| `hooksProfile` | `claudeNested` → `hooks/hooks.json` | 同左 | `cursorNative` → 根 `hooks.json` |
| `mcpOut` | `.mcp.json` | `mcp.json` | `mcp.dev-team.json` |
| `layout` | `plugin` + `.claude-plugin/` | `plugin` + `.cursor-plugin/` | `home-image` |
| `pathReplacePhase` | `build` | `build` | `install` |

### Token 展开规则

```text
__SKILL:([a-z0-9-]+)__        →  namePrefix + $1
__AGENT:([a-z0-9-]+)__        →  agentRefPrefix + $1          # workflow / 文内 Agent 引用
__SKILL_SLASH:([a-z0-9-]+)__  →  skillSlashPrefix + $1
__MCP:([a-z0-9_]+)__          →  mcpToolPrefix + $1           # 必须有 tool id
__BIN:(mcp|cli|hooks)__       →  namePrefix + $1 + '.cjs'
__DEV_TEAM_ROOT__             →  contentRoot（仅 pathReplacePhase=build 或 install 时）
__DEV_TEAM_RUNTIME_ROOT__     →  runtimeRoot（同上）
```

磁盘 skill/agent/bin **文件名**一律 `namePrefix + logicalId`（+ `.md` / `.cjs`）；与 `__SKILL:` / `__BIN:` 同源。  
SubagentStop **matcher** 使用 `namePrefix + agentLogicalId`（与磁盘 agent 名对齐；plugin 侧保持现网裸名 `implementation-generator`）。  
`__AGENT:`（`agentRefPrefix`）专用于 `workflow.ts` 等运行时引用，**不**用于磁盘 rename。

### 占位符约束

- 仅出现在字符串字面量（md / json / TS string）；禁止函数形（`skill('id')`）与标识符/import 路径。
- 禁止 MCP 服务器级无 tool id 引用（如裸 `mcp__plugin_dev-team_dev-team`）。
- 逻辑 id 权威：`readdir`（`skills/<id>/`、`agents/<id>.md`、staging `bin/{mcp,cli,hooks}.cjs`）；无手写 id catalog。
- 扫描：宽 include（skills/agents/hooks/templates/utils 文本、组装 json、staging `bin/*.cjs`）；窄 exclude（`openspec-bundled.js`、二进制、`.map`）。

### hooksProfile 组装要点

| Profile | 输出路径 | 形态 |
|---------|----------|------|
| `claudeNested` | `hooks/hooks.json` | `hooks` → `PreToolUse` / `SubagentStop` + 内层 `hooks[]`；matcher：`Write\|Edit`、`Bash`、`PowerShell`；command 含展开后的 bin |
| `cursorNative` | 根 `hooks.json` | `version` + camelCase 事件；扁平 `command`/`matcher`/`loop_limit`；matcher：`Write\|StrReplace`、`Shell`（及按需保留其它）；command 可仍含路径 token |

禁止「以 Claude 成品 JSON 字符串替换出 Cursor」。

### 安装器行为（摘要）

1. 解析 `--root`（默认 `~/.cursor`）为绝对路径。  
2. 以**镜像为源**替换 `__DEV_TEAM_ROOT__` / `__DEV_TEAM_RUNTIME_ROOT__` → 同一绝对路径；非 JSON 文本将 `\` 规范为 `/`；JSON 结构化 merge 后 `JSON.stringify`。  
3. 按 `manifest.json` 同步前缀化 skills/agents 与 `bin`/`templates`/`utils`。  
4. Merge `hooks.json` / `mcp.json`：仅动 `command` 含 `dev-team_` 的 hooks 条目、key 以 `dev-team_` 开头的 mcp server；禁止整文件盲覆盖。  
5. 写安装状态；stdout 提示 Reload Window。  
6. **不做**：名称前缀化、marketplace 注册、hooks 事件名改写、MCP 工具名前缀改写、以已安装文件二次替换。

### Home 镜像目标布局

```
cursor-home-image/dev-team/
├── skills/dev-team_*/
├── agents/dev-team_*.md
├── hooks.json
├── bin/dev-team_{mcp,cli,hooks}.cjs  (+ openspec 等静态 bin)
├── templates/
├── utils/
├── mcp.dev-team.json
├── install.mjs
└── manifest.json
```

---

<!-- 本变更不涉及 HTTP API；路由/API 设计节整段省略 -->

## 依赖

### 运行时依赖

- Node.js（`fs` / `path` / `os`）— 安装器与 hooks CJS  
- 现有 `plugins/dev-team` dependencies（zod、picomatch、cac、MCP SDK 等）— 业务 bin 不变  
- **无**新增 npm 运行时依赖（安装器保持零依赖，便于用户直接 `node install.mjs`）

### 构建/测试依赖

- `vite-plus`（`vp pack`）— 唯一 bundler  
- 现有 TypeScript / Vitest 工具链 — 构建模块与 hooks 实现；**完整 build 产物树不设计自动化断言**（手动 `pnpm -C plugins/dev-team run build` + 目检）

---

## 待决问题

- SubagentStop matcher 在 Cursor 用户级 hooks 上匹配的是磁盘文件名还是 frontmatter `name`：实现期用 `namePrefix + logicalId` 对齐磁盘名；若实机不符，再微调 canonical / profile 列（不阻塞本设计）。  
- Plugin 侧 MCP `mcpServers` key 是否继续使用现网 `"dev-team"`（与 home 的 `dev-team_mcp` 不同）：默认保持现网 key，仅 bin 路径改为 `mcp.cjs`；若现网另有约束实现期对照产物修订。  
- `HomeInstallState` 落盘文件名（建议 `dev-team-install.json`）：实现时可在 manifest 中写死，无需再开 change。
