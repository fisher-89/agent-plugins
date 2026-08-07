# cursor-home-image

探索时间：2026-08-06  
参考对话：[Cursor settings update](388e4961-9b87-40d9-98ab-1feed2244f80)（Home）

## 问题

团队策略可关闭第三方 / 本地插件通道（`userLocal=false`），导致现有 `cursor-plugins/` marketplace 产物无法安装。  
Cursor 仍会加载用户级原生路径（`~/.cursor/skills`、`agents`、`hooks.json`、`mcp.json` 等）。需要第三种 **可分开复制并安装到 `~/.cursor`** 的 Cursor 产物，在受限环境下提供与插件等价的全功能。

本机随手复制到 `~/.cursor` 的内容仅作验证线索，**不作为**产物设计依据。

## 已锁定决策

1. **产物目录名**：`cursor-home-image/`（与 `claude-plugins/`、`cursor-plugins/` 并列；“image”= 面向 home 的镜像树）。
2. **形态**：带模板的 **镜像目录 + 安装器**。
3. **首版范围**：现有全部功能（skills / agents / hooks / mcp / bin / templates / utils）。
4. **安装目标**：仅 `~/.cursor`（不做 workspace `.cursor`）。
5. **隔离方式（名称前缀，非目录命名空间）**：不在 `~/.cursor` 下用 `dev-team/` 目录隔离发现位。镜像内 **skill / agent / hook 名称**携带 `<插件名>_` 前缀（例：`dev-team_phase-acceptance`、`dev-team_proposal-planner.md`）。安装后与用户其它内容共存于同一 `skills/`、`agents/` 等目录；托管范围靠前缀 + manifest 识别。
6. **组装方式**：从 **源码** 组装（非从 `cursor-plugins` 二次变换）。
7. **源码路径**：使用 **占位符**；`claude` / `cursor` 在 **构建 assemble** 时按 env 替换完毕。
8. **安装器**：Node，跨平台。除同步 + merge + manifest 外，对 `cursorHome` **在安装时**把镜像内路径占位符替换为 **本机绝对路径**（见决策 17）。
9. **与 marketplace 关系**：`cursor-plugins` 与 `cursor-home-image` **并存、同 version**（权威仍为 `plugins/dev-team/package.json`）。
10. **构建形状**：**一轮 pack（不替换）+ assemble ×3**（claude / cursor / cursorHome）。  
    - **pack**：只 bundle `mcp` / `cli` / `hooks` → staging CJS，**不做** env / token 替换；字符串内 `__…__` 原样保留。优先 **单个 pack 配置 + 多 entry**（一轮 build 写齐三入口），避免今日 `SUPPORT_AGENTS × 3` = pack 6 次，也避免 `pack[]` 数组多次 `build:done` 抢跑。  
    - **assemble×3**：对 staging CJS + 静态资产跑 `applyEnvTokens`、rename、`hooksProfile` / mcp 分叉，写入各 `outDir`。  
    - **触发**：assemble 在 **`vp pack` 生命周期内**完成（`hooks['build:done']` 或等价 vite-plus/tsdown pack 插件收尾）；默认不另挂 `node build/assemble.ts` 作为主路径（assemble 逻辑仍可抽成模块供 hook 调用与单测）。  
    - **禁止**：把 `cursorHome` 塞进 `SUPPORT_AGENTS` 导致按产品倍增 pack；禁止在 bundle 期 `define`/transform 按 env 换 token（那会逼回 pack×产品）。
11. **`outDir` 显式配置**：不得依赖 `` `${agent}-plugins` `` 拼接；`cursorHome` → `cursor-home-image/dev-team`。
12. **占位符体系**（见下节「占位符」）：统一 `__<KIND>:<id>__`；含 skill/agent/mcp/slash/**bin**；构建/安装按阶段展开。
13. **hooksProfile 分叉**（源=关键信息，构建组装格式）：  
    - **源码不维护**成品 Claude/Cursor `hooks.json`；只维护 hooks **关键信息**（逻辑事件、matcher、command 模板含 `__BIN:`/`__DEV_TEAM_RUNTIME_ROOT__`、可选 `loop_limit`、description 等）。  
    - **assemble** 按 `hooksProfile` 组装成对应格式文件（**禁止**「以 Claude 嵌套 JSON 为源再字符串替换出 Cursor」）。  
    - **格式族×2**：`claudeNested`（`hooks` → `PreToolUse`/`SubagentStop` + 内层 `hooks[]`）与 `cursorNative`（`version` + `hooks` → camelCase 事件，条目扁平 `command`/`matcher`/`loop_limit`）。  
    - **env 映射**：`claude` / `cursor`（marketplace 插件内）→ `claudeNested` 落 `hooks/hooks.json`；`cursorHome` → `cursorNative` 落镜像根 `hooks.json`（路径 token 留安装）。marketplace `cursor` 是否改发 `cursorNative` **不在本决策**（现网仍嵌套形）。  
    - matcher 可按 profile 分列（例：Claude `Write|Edit`/`Bash`/`PowerShell`；Cursor `Write|StrReplace`/`Shell`），与决策 18 脚本侧工具名适配配套。
14. **安装器合并策略**：`mcp.json` / `hooks.json` **按 managed key merge**，禁止整文件盲覆盖。  
    **识别规则统一：`dev-team_` 前缀**（与 skill/agent 同一约定）：  
    - **hooks**：托管条目的 `command`（或可选 name/id）**包含** `dev-team_`；构建保证（bin/脚本名由 `__BIN:…__` 展开后满足此前缀规则）。更新/卸载只动这些条目。  
    - **mcp**：`mcpServers` 的 key **以 `dev-team_` 开头**（例：`dev-team_mcp`）；只 upsert/删除此前缀 key，保留用户其它 server。  
    不靠旁路「等价 id」第二套规则。
15. **产物 commit**：与双产物一致，**commit** `cursor-home-image/`，clone 即可装。
16. **templates / utils / bin 目录层级与插件内一致**：安装后相对 `~/.cursor` 的布局同插件根相对布局，即  
    `~/.cursor/templates`、`~/.cursor/utils`、`~/.cursor/bin`（镜像产物内亦为同级 `templates/`、`utils/`、`bin/`）。  
    不另造 `~/.cursor/dev-team/templates` 之类包目录。
17. **cursorHome 路径占位符延迟到安装期**：  
    - **镜像内保留** `__DEV_TEAM_ROOT__` / `__DEV_TEAM_RUNTIME_ROOT__`。  
    - **安装时**替换为绝对路径；名称类 token（含 `__BIN:`）已在构建期展开完毕。  
18. **`hooks.ts` Cursor 工具名适配放在本 change**：protect-files 等须识别 Cursor 侧工具名（至少 `Shell` / `StrReplace`，并与现有 `Write`/`Edit`/`Bash`/`PowerShell` 兼容）；hooksProfile（原生 `hooks.json`）与脚本行为同一变更交付，避免 home「全功能」空心。cursor 插件产物一并受益。
19. **占位符落点：字符串内可用（含 TS），assemble 与 md 一并展开**：  
    - Token **仅出现在字符串字面量**中（md / json / 脚本 / **`bin/src` 里的 string**），不作标识符/变量名，故 **不破坏 lint / typecheck**。  
    - **pack×1** 把含 token 的字符串原样打进 CJS；**assemble×3** 对「静态资产 + 该份 CJS」跑同一套替换（与 md 同一管道），再按 env 输出到三产物。  
    - **minify 假设（已接受）**：terser/esbuild 类压缩只改写标识符，**不改写字符串字面量内容**；完整 `'__AGENT:…__'` 会保留到 CJS，供 assemble 替换。不把「minify 破坏 token」当作阻塞风险；若未来换用会改写字符串的极端优化再重评。  
    - 源码/测试只写 **token 或逻辑 id**，不手写各产品最终限定名；落盘 rename 与扫描策略见决策 22。  
    - 测试：断言 **token 形态**或「先按 `claude` env 展开再比最终串」；不在测试里写死仅某一产品的最终名（除非该测试绑定单 env）。
20. **安装器路径分隔符（实用即可）**：不维护 `/` vs `path.sep` 双轨策略。  
    - 安装根用 `path.resolve` 得本机绝对路径。  
    - **JSON**（`hooks.json` / `mcp.json`）：结构化 merge / `JSON.stringify` 写入（转义交给 JSON）。  
    - **非 JSON 文本**（md / agent 等内嵌路径）：写入前把 `\` 规范成 `/`（Node / Cursor 在 Win 上通常可用）。  
    - 仍以镜像为源替换再写入，不以已安装文件二次替换。
21. **磁盘文件名前缀拉通（skill / agent / bin 同一旋钮）**：  
    - 现状分歧：plugin 内 skill/agent 磁盘名无前缀，bin 却是 `dev-team-*.cjs`。  
    - **锁定**：落盘文件名一律由 `namePrefix` 派生；`__BIN:id__` → `namePrefix + id + '.cjs'`（可仍暴露为派生字段 `binNamePattern`，但不得与 `namePrefix` 分叉）。  
    - 三行结果：`claude` / `cursor` → `mcp.cjs` / `cli.cjs` / `hooks.cjs`；`cursorHome` → `dev-team_mcp.cjs` 等（满足 hooks merge「command 含 `dev-team_`」）。  
    - **不**在 plugin 内固定保留 `dev-team_` bin 名——那会让 bin 与 skill/agent 的 `namePrefix=''` 脱节。  
    - 范围仅限**磁盘文件名**；`agentRefPrefix` / `mcpToolPrefix` / `skillSlashPrefix` 仍可独立（引用形 ≠ 落盘名）。
22. **无手写 id catalog；FS 权威 + globs 全量扫**：  
    - **逻辑 id 权威**：`skills/<id>/`、`agents/<id>.md`、staging `bin/{mcp,cli,hooks}.cjs` 的**文件系统布局**（`readdir`），不维护 `refs.catalog.json` 之类手写 id 表（与 `namePrefix` rename 重复、易漂）。  
    - **落盘 rename**：目录/文件名 + `namePrefix` 派生（决策 21），无单独 rename 映射表。  
    - **token 替换范围**：按 **宽 include globs 全量扫**（skills/agents/hooks/templates/utils 文本、assemble 写入的 json 片段、**staging `bin/*.cjs`**）；**窄 exclude**（如 `openspec-bundled.js`、二进制、sourcemap）。否决「只扫 catalog 白名单路径」。  
    - 若需要 include/exclude 配置，可薄封装为 assemble 配置（非 id 权威源）；也可内联在 assemble 代码。  
    - **构建断言**：各产物无名称类 token 残留（`cursorHome` 可留路径 token）；可选：源码中出现的 `__AGENT:id__` / `__SKILL:id__` 必须能在 FS 找到对应条目。不靠手写清单防漏扫。
23. **`cursorHome` 用户级引用前缀（实机钉死）**：本条只钉用户级 `~/.cursor`，不重开 marketplace `cursor` 前缀问题（插件侧沿用现网）。  
    - **MCP server key**：`dev-team_mcp`（满足 merge「key 以 `dev-team_` 开头」）。  
    - **Cursor Agent namespace**（实机）：`user-` + server key → **`user-dev-team_mcp`**（对照插件 namespace `plugin-dev-team-dev-team`）。UI 可显示短名 `dev-team_mcp`；写进 skills/agents 的调用串跟 Agent 命名空间对齐。  
    - **`mcpToolPrefix`**：`mcp__user-dev-team_mcp__`（`__MCP:phase_next__` → `mcp__user-dev-team_mcp__phase_next`，与现网 `mcp__plugin_…__` 同形）。  
    - **`skillSlashPrefix`**：`/dev-team_`（与 `namePrefix` / skill 目录名对齐）。  
    - **`agentRefPrefix`**：`dev-team_`（与磁盘 agent 名对齐）。  
    - 证据：2026-08-06 将探测 MCP 挂入 `~/.cursor/mcp.json`（key=`dev-team_mcp`）后，Agent 工具上下文出现 namespace `user-dev-team_mcp`；确认后已从 `mcp.json` 移除（恢复 `mcpServers: {}`）。仓库内 `openspec/explores/spike-user-mcp-probe.mjs` 可删可留，不影响产物。
24. **不做 MCP 服务器级引用**：业务只需具体工具 API。源码/`tools:` frontmatter **禁止** `mcp__…` 无 tool id 的 server 白名单（今日 `proposal-evaluator` / `acceptance-evaluator` 的 `mcp__plugin_dev-team_dev-team`）；改为列出实际调用的 `__MCP:<tool>__`（例：`__MCP:phase_log__`）。不引入 `__MCP_SERVER__` 或空 id 特例；`__MCP:` 一律带 snake tool id。

## 三套 env：统一替换策略

目标：多种产物共用同一套「占位符 → env 表」心智。

| env key | 输出目录 | 用途 |
|---------|----------|------|
| `claude` | `claude-plugins/dev-team/` | Claude Code marketplace 插件 |
| `cursor` | `cursor-plugins/dev-team/` | Cursor marketplace 插件 |
| `cursorHome` | `cursor-home-image/dev-team/` | `~/.cursor` 镜像 + Node 安装器 |

### 占位符（源码约定）

源码只写 **逻辑名** 与占位符，不写某产品下的最终限定名。

#### A. 路径（字符串 token）

| 占位符 | 含义 | 出现位置（例） |
|--------|------|----------------|
| `__DEV_TEAM_ROOT__` | 内容根：templates / utils / 文档内 Read 路径 | agents、utils 引用 |
| `__DEV_TEAM_RUNTIME_ROOT__` | 运行时根：hooks / mcp 启动 bin | `hooks.json`、mcp 片段 |

#### B. 名称引用 token（构建期展开）

**设计约束**：占位符必须一眼可辨为「组装期宏」，避免与 prompt / 运行时工具名撞车。

今日源码里已经大量出现：

- `Agent({ ... subagent_type: ... })` — Cursor/Claude 调子代理
- `Skill` / slash `/dev-team:phase-proposal` — 调 skill
- `mcp__plugin_...__phase_next` — 调 MCP

因此 **否决** 裸函数形：`skill('…')` / `agent('…')` / `mcp('…')`——阅读时极易当成运行时 API，也难和 `Agent(` 区分，误替换/误读风险高。

**推荐语法**：与路径占位符同一族的 **双下划线 + 种类前缀 + 逻辑 id**：

| Token | 展开规则 | 例（插件 env） | 例（`cursorHome`） |
|-------|----------|----------------|------------------|
| `__SKILL:phase-proposal__` | `namePrefix + id` | `phase-proposal` | `dev-team_phase-proposal` |
| `__AGENT:dev-design-planner__` | 同上 | `dev-design-planner` | `dev-team_dev-design-planner` |
| `__MCP:phase_next__` | `mcpToolPrefix + id`（**必须有 tool id**；禁止无 id 的 server 级串，决策 24） | `mcp__plugin_dev-team_dev-team__phase_next` | `mcp__user-dev-team_mcp__phase_next` |
| `__SKILL_SLASH:phase-proposal__` | `skillSlashPrefix + id` | `/dev-team:phase-proposal` | `/dev-team_phase-proposal` |
| `__BIN:mcp__` | `namePrefix + id + '.cjs'` | `mcp.cjs` | `dev-team_mcp.cjs` |
| `__BIN:cli__` / `__BIN:hooks__` | 同上 | `cli.cjs` / `hooks.cjs` | `dev-team_cli.cjs` / `dev-team_hooks.cjs` |

**收敛规律（全部同形）**

```text
__<KIND>:<logical-id>__
```

| KIND | 用途 | 绑定 env 字段 |
|------|------|----------------|
| `SKILL` | skill 目录名 / frontmatter `name` / 非 slash 引用 | `namePrefix` |
| `AGENT` | agent 文件名 / frontmatter / hooks matcher | `namePrefix` |
| `MCP` | MCP 工具全名 | `mcpToolPrefix` |
| `SKILL_SLASH` | 文内 slash 调用 | `skillSlashPrefix` |
| `BIN` | `bin/` 下产物文件名（及 hooks/mcp 中对它们的引用） | **派生自 `namePrefix`**：`namePrefix + id + '.cjs'`（与 skill/agent 磁盘名同一旋钮） |
| （路径）`DEV_TEAM_ROOT` / `DEV_TEAM_RUNTIME_ROOT` | 文件系统根 | `contentRoot` / `runtimeRoot`（home：安装期） |

逻辑 id：skill/agent/slash → kebab-case；mcp tool → snake；**bin** → 短逻辑名 `mcp` / `cli` / `hooks`（不是最终文件名）。  
**禁止**函数形占位符。

**为何选这套而不是其它**

| 候选 | 结论 | 原因 |
|------|------|------|
| `skill('id')` / `agent('id')` | 否决 | 像运行时 API |
| `{{skill:id}}` | 否决 | 与 `__…__` 两套风格 |
| `__SKILL:id__` 族含 `SKILL_SLASH` / `BIN` | **锁定** | 一种语法；正则统一 |

#### C. 字符串内 token；TS 与 md 同一 assemble 管道

| 层 | token | 说明 |
|----|-------|------|
| FS：`skills/`、`agents/`、staging `bin/` | 权威 id | `readdir`；无手写 id catalog（决策 22） |
| assemble include/exclude globs | 扫描范围 | 宽扫文本 + staging `*.cjs`；窄 exclude 二进制等 |
| skills / agents / hooks / mcp / templates / utils | 字符串内 | 互引、command、matcher、slash |
| `bin/src/**/*.ts` | **允许，仅 string** | 典型：`workflow.ts` 里 `agent_type: '__AGENT:proposal-planner__'`（今日为 `'dev-team:proposal-planner'`） |
| pack staging `*.cjs` | 保留未展开 token | minify 后字符串字面量仍在，供 assemble 替换 |
| assemble → 三产物 | 展开 | **md/json/脚本 + CJS** 走同一 `applyEnvTokens(text, env)` |

```
bin/src/{mcp,cli,hooks}.ts  (string 内 __TOKEN__)
      │
      │  vp pack：一轮 multi-entry（不替换）
      ▼
 .pack-staging/bin/{mcp,cli,hooks}.cjs  (仍含 token；gitignore)
      │
      │  同一 vp pack 收尾：build:done / pack 插件
      ├─ assemble(claude)     → claude-plugins/...
      ├─ assemble(cursor)     → cursor-plugins/...
      └─ assemble(cursorHome) → cursor-home-image/...  (路径 token 可仍保留)
```

原则：

- **可以**在 TS 字符串里用占位符，与 md **一并**在 assemble 处理——这是关闭「`phase_next` 动态 agent_type」缺口的正道（表数据在 TS里，必须能按产品展开）。  
- **不要**把 token 写进标识符/import 路径；入口文件名由 `namePrefix` + 逻辑 id 在 assemble rename（`__BIN:` 展开同理）。  
- **FS 权威**逻辑 id；TS/md 只引用 token，不手写各产品最终限定名。  
- lint/typecheck：把 token 当普通 string → **无影响**。test：期望值用 token 或 `expand(claude)` 后比较。

**`__AGENT:` 与文件名（重要）**

今日运行时 `agent_type` 已是 `dev-team:proposal-planner`（**冒号**插件限定），而插件产物磁盘文件名是 `proposal-planner.md`（无前缀）。故：

| 用途 | env 字段 | 插件例 | home 例 |
|------|----------|--------|---------|
| 运行时 / hooks matcher 等引用 | `agentRefPrefix` | `dev-team:` | `dev-team_` |
| 磁盘 skill/agent 文件名 | `namePrefix` | `''` | `dev-team_` |

`__AGENT:proposal-planner__` → `agentRefPrefix + id`（供 `workflow.ts`、matcher）。  
磁盘 rename 仍用 `namePrefix`（与 `__SKILL:` 一致）。二者在 home 上都是 `dev-team_` 时可对齐；在插件上 **有意不同**（`dev-team:` vs 无前缀文件名）。

展开实现（示意）：

```text
__SKILL:([a-z0-9-]+)__        →  namePrefix + $1
__AGENT:([a-z0-9-]+)__        →  agentRefPrefix + $1
__SKILL_SLASH:([a-z0-9-]+)__  →  skillSlashPrefix + $1
__MCP:([a-z0-9_]+)__          →  mcpToolPrefix + $1
__BIN:(mcp|cli|hooks)__       →  namePrefix + $1 + '.cjs'
```

构建断言：各产物内名称类 token 不得残留（`cursorHome` 可残留路径 token）；staging 允许残留。

### env 表

| 字段 | `claude` | `cursor` | `cursorHome` |
|------|----------|----------|--------------|
| `contentRoot` → `__DEV_TEAM_ROOT__` | `${CLAUDE_PLUGIN_ROOT}`（构建） | `.`（构建） | **镜像保留**；安装 → 绝对路径 |
| `runtimeRoot` → `__DEV_TEAM_RUNTIME_ROOT__` | `${CLAUDE_PLUGIN_ROOT}`（构建） | `.`（构建） | **镜像保留**；安装 → 同一绝对路径 |
| `mcpToolPrefix` → `__MCP:…__` | `mcp__plugin_dev-team_dev-team__` | 同左（现网；本探索不重钉） | **`mcp__user-dev-team_mcp__`**（决策 23） |
| `namePrefix` → `__SKILL:` / 磁盘 agent 名 / **`__BIN:`** | `''` → `mcp.cjs` 等 | `''` → 同左 | `dev-team_` → `dev-team_mcp.cjs` 等 |
| `agentRefPrefix` → `__AGENT:…__`（workflow / matcher） | `dev-team:` | `dev-team:`（现网） | **`dev-team_`**（决策 23） |
| `skillSlashPrefix` → `__SKILL_SLASH:…__` | `/dev-team:` | `/dev-team:`（现网） | **`/dev-team_`**（决策 23） |
| `binNamePattern` → `__BIN:…__` | **派生** `namePrefix + '{id}.cjs'`（勿与 `namePrefix` 分叉；非独立第三套规则） | 同左 | 同左（home 因 `namePrefix` 自动带 `dev-team_`） |
| `layout` | plugin + `.claude-plugin/` | plugin + `.cursor-plugin/` | home-image |
| `hooksProfile` | `claudeNested` → `hooks/hooks.json` | 同左（插件内；现网） | `cursorNative` → 根 `hooks.json`（路径 token 留安装） |
| `mcpOut` | `.mcp.json` | `mcp.json` | merge 片段（含路径/`__BIN:` 于构建期展开 bin 名） |
| `outDir` | `../../claude-plugins/dev-team` | `../../cursor-plugins/dev-team` | `../../cursor-home-image/dev-team` |
| `pathReplacePhase` | `build` | `build` | `install`（仅路径两列） |

```
源码（占位符）
      │ vp pack：一轮（不替换）→ staging
      ▼
 assemble(claude)  ──构建──▶ 路径+MCP 全部替换 → claude-plugins/
 assemble(cursor)  ──构建──▶ 路径+MCP 全部替换 → cursor-plugins/
 assemble(cursorHome)─构建──▶ 前缀化 + hooks 形态 + MCP 前缀
                              路径占位符保留     → cursor-home-image/
      │
      ▼ install.mjs（本机）
  __DEV_TEAM_ROOT__ / __DEV_TEAM_RUNTIME_ROOT__
      → <absolute ~/.cursor>
  再写入 ~/.cursor/（merge skills/agents/hooks/mcp/bin/...）
```

原则：

- **一张表、三行 env**；差异优先加列。
- `claude` / `cursor`：assemble 时展开路径 token + 名称 token（`namePrefix` 可为空）。
- `cursorHome`：**构建**展开全部名称 token（含 `__SKILL_SLASH:` / `__BIN:`）与 `hooksProfile`；**路径 token 保留**；**安装**做路径绝对化。
- 安装器 **只**替换路径 token；名称/`BIN` token 构建后不得残留。
- `__<KIND>:<id>__` + **FS 权威 / globs 全量扫**（决策 22）；**TS 字符串与 md 同一 `applyEnvTokens`**；**一轮 pack（不替换）+ assemble×3**（在 `vp pack` 内收尾）。

## 目标布局（cursor-home-image）

镜像内 skill/agent/hook **已带前缀**；安装时摊到 `~/.cursor` 同级目录，**不再**使用 `~/.cursor/dev-team/` 作为发现位隔离。

```
cursor-home-image/dev-team/
├── skills/
│   └── dev-team_phase-acceptance/SKILL.md
│   └── dev-team_...
├── agents/
│   └── dev-team_proposal-planner.md   # 内含 __DEV_TEAM_ROOT__/templates/...
│   └── dev-team_...
├── hooks.json                       # Cursor 原生形态；command 内仍为 __DEV_TEAM_RUNTIME_ROOT__/bin/...
├── hooks/                           # 可选；脚本名带 dev-team_ 前缀
├── bin/
├── templates/
├── utils/
├── mcp.dev-team.json                # args 内仍为路径占位符
├── install.mjs                      # 替换占位符 → 绝对路径，再同步/merge
└── manifest.json
```

安装后（路径已是绝对路径）：

```
~/.cursor/
├── skills/dev-team_.../
├── agents/dev-team_*.md             # __DEV_TEAM_ROOT__ → D:\...\.cursor 等
├── hooks.json                       # runtime 绝对路径已写入
├── mcp.json                         # merge；bin 绝对路径
├── bin/
├── templates/
└── utils/
```

同构对照（插件根 ≡ `~/.cursor`）：

| 插件内 | 安装后 |
|--------|--------|
| `<pluginRoot>/templates/...` | `~/.cursor/templates/...` |
| `<pluginRoot>/utils/...` | `~/.cursor/utils/...` |
| `<pluginRoot>/bin/...` | `~/.cursor/bin/...` |
| `<pluginRoot>/skills/<name>/` | `~/.cursor/skills/dev-team_<name>/`（仅名称加前缀） |
| `<pluginRoot>/agents/<name>.md` | `~/.cursor/agents/dev-team_<name>.md` |

**隔离**：skills/agents/hooks 靠 `dev-team_` 前缀 + manifest；templates/utils 内部相对路径保持插件原样（如 `templates/artifacts/...`），托管范围由 manifest 列出的路径清单约束（避免误删用户自有同名文件）。

## 安装器（Node）职责

1. 解析 install root 绝对路径（默认 `path.resolve(os.homedir(), '.cursor')`，可允许 flag 覆盖）。
2. 对镜像内需落盘的文本，将 `__DEV_TEAM_ROOT__` / `__DEV_TEAM_RUNTIME_ROOT__` **替换为该绝对路径**（两列同值）；分隔符按决策 20（JSON 结构化写入；非 JSON 文本规范为 `/`）。
3. 按 manifest 同步前缀化 `skills/`、`agents/` 及 `bin/` / `templates/` / `utils/`。
4. Merge `hooks.json`、`mcp.json`：仅处理带 `dev-team_` 前缀识别的托管项；禁止盲覆盖整文件。
5. 写入安装状态（version、root、托管清单）；提示 Reload Window。

不做：hooks 事件名/matcher 改写、MCP **工具名**前缀改写（那是构建期 `__MCP_TOOL_PREFIX__`）、名称前缀化、marketplace 注册。

## 构建与运行时结论（落地推演）

| 结论 | 说明 |
|------|------|
| 一轮 pack（不替换）+ assemble×3 | 优先 multi-entry 单 pack；token 只在 assemble；`vp pack` 内 `build:done`/插件触发；取代今日 pack×6 |
| MCP 前缀必须进 env | 用户级 MCP 与插件 MCP 工具名不同；**构建期**写入镜像 |
| hooks 配置要分叉 | 源=关键信息；assemble 按 profile 组装 `claudeNested` / `cursorNative`（非 Claude JSON 字符串替换） |
| hooks **脚本**认 Cursor 工具名 | **本 change** 改 `hooks.ts`（与 home hooksProfile 一起交） |
| 名称前缀替代目录命名空间 | skill/agent/hook/mcp 托管识别统一 `dev-team_` |
| 名称用同形 token | 含 `__BIN:`；一律 `__KIND:id__` |
| TS 字符串可含 token | 与 md 一并 assemble；关闭动态 `agent_type` 缺口；lint 无影响 |
| 无手写 id catalog | FS 权威；宽 globs（含 staging cjs）+ 窄 exclude；构建断言抓残留 token |
| hooks + mcp managed merge | 均靠 `dev-team_` 识别；非整文件覆盖 |
| templates/utils/bin 与插件同构 | 安装根=`~/.cursor` |
| 路径占位符安装期绝对化 | 镜像可分发、与机器无关；消除 `~` 展开歧义 |
| 安装路径分隔符实用即可 | `path.resolve` + JSON 转义 / 非 JSON 用 `/`；无双轨 sep 策略 |
| 磁盘名与 `namePrefix` 拉通 | skill/agent/bin 同一旋钮；plugin bin 去 `dev-team-` 前缀 → `{id}.cjs` |
| cursorHome 用户级前缀实机钉死 | MCP `mcp__user-dev-team_mcp__`；slash/agentRef 跟 `dev-team_`；server key `dev-team_mcp` |

## 明确不做（本探索意图）

- 不支持安装到项目 `.cursor/`（可后续另开）。
- 不以本机已有 `~/.cursor` 脏拷贝为规范。
- 不删除或取代 `cursor-plugins/` marketplace 产物。
- 不在 `~/.cursor` 下用插件专属顶层目录做发现位隔离（已改前缀方案）。

## 风险

| 风险 | 影响 | 缓解方向 |
|------|------|----------|
| Cursor 变更用户级 MCP namespace 规则（非 `user-`+key） | home 产物工具名失效 | 决策 23 已按实机钉死；若 Cursor 改规则再开 change 重钉 |
| `hooks.ts` 未适配 `Shell`/`StrReplace` | protect-files 在 Cursor 上无效 | 本 change 或硬依赖变更中改 bin；AC 要求冒烟 |
| SubagentStop 反参（`decision/reason` vs `followup_message`）在用户级 hooks 上行为不一致 | static-check 续跑失效 | 实机确认；必要时 cursorHome 构建/包装输出原生格式 |
| 名称 token 漏写或残留 `__SKILL:` / `__SKILL_SLASH:` 等 | 调用失败 | 构建断言无残留；slash 与 skill 名同源 id |
| `skillSlashPrefix` 与宿主真实 slash 登记不一致 | 文档里的 `/…` 点了无效 | 各 env 的 prefix 以实机为准钉进表；home 与 `namePrefix` 对齐 |
| 用户已有同名 `dev-team_*` | 安装覆盖冲突 | install 提示 / manifest 校验 |
| Windows 上 `openspec-cli.sh`（bash source） | Win 全功能不等价 | 文档写清平台预期；不假装三端等价 |
| 安装后用户搬迁/改名 home 导致绝对路径失效 | hooks/mcp/agents 指到旧盘 | 文档要求重跑 install；可选 `--root` |
| 更新安装时对已绝对化文件再次替换 | 误伤或漏替换 | 以镜像为源重新替换再写入，不以已安装文件为模板二次替换 |
| `~/.cursor/templates`（或 utils）与用户/其它工具文件冲突 | 同路径覆盖或误删 | manifest 精确列出托管文件；安装前提示冲突 |
| 前缀化后 Task/subagent 的 matcher（`implementation-generator`）是否仍匹配 | static-check 不触发 | 确认 Cursor 匹配的是 agent 文件名还是 frontmatter name；前缀策略与之对齐 |

## 已关闭（原开放问题）

- **`phase_next` / `agent_type`**：采用 TS 字符串占位符 + assemble 与 md 一并展开；`workflow.ts` 等改为 `__AGENT:…__`，按 `agentRefPrefix` 生成 `dev-team:…`（插件）或 `dev-team_…`（home）。不再依赖运行时猜前缀。
- **安装器路径分隔符**：决策 20；实用即可，不维护双轨。
- **`binNamePattern` / 落盘前缀**：决策 21；与 `namePrefix` 拉通。否决「plugin bin 固定 `dev-team_*`、skill/agent 仍裸名」——那是制造分歧。plugin 侧去掉现有 `dev-team-` bin 前缀，改为 `{id}.cjs`。
- **pack / assemble 落地形状**（决策 10 细化）：一轮 pack（multi-entry、**不替换**）→ staging；同一 `vp pack` 收尾 assemble×3。可用 `build:done` 或 vite-plus pack 插件编排；不在 bundle 期按 env 换 token；不以独立 assemble CLI 作主路径。staging 目录 gitignore（如 `.pack-staging/`）。
- **catalog / patch 范围**（决策 22）：不做手写 id catalog；FS 权威 + 宽 globs 全量扫（含 staging `*.cjs`）+ 窄 exclude；构建断言防残留。
- **`cursorHome` 用户级前缀**（决策 23）：实机确认 namespace=`user-dev-team_mcp`；锁定 `mcpToolPrefix` / `skillSlashPrefix` / `agentRefPrefix` 与 server key。marketplace `cursor` 不在本开放问题范围。
- **MCP 服务器级引用**（决策 24）：业务不需要；frontmatter/`tools:` 只列具体 `__MCP:tool__`，不引入 server 级 token。实现期把现有两处 server 白名单改成实际 API（至少 `phase_log`）。
- **hooks 源形态**（决策 13 细化）：源码只维护关键信息；构建组装 `claudeNested` / `cursorNative`。今日 `hooks/hooks.json`（Claude 成品）改为 canonical 源（或迁入 `hooks/*.canonical.*` 并由 assemble 生成各产物，源树不再当 Claude 权威成品提交）。

## 开放问题

（无 — 探索期决策已收齐；matcher 是否匹配前缀文件名等实现期风险见上表，不阻塞进 proposal。）

## 下一步

- 探索决策已收齐；探测 MCP 已从本机 `~/.cursor/mcp.json` 移除。
- 走 `/dev-team:phase-proposal`；change 名建议：`cursor-home-image`（promote 本文件为 `openspec/changes/cursor-home-image/explore.md`）。
