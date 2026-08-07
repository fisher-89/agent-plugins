# 提案: cursor-home-image

> **变更**: cursor-home-image
> **日期**: 2026-08-06
> **状态**: 草稿

---

## 问题

团队策略可关闭第三方 / 本地插件通道（`userLocal=false`），导致现有 `cursor-plugins/` marketplace 产物无法安装。Cursor 仍会加载用户级原生路径（`~/.cursor/skills`、`agents`、`hooks.json`、`mcp.json` 等），但仓库缺少第三种 **可分开复制并安装到 `~/.cursor`** 的产物，无法在受限环境下提供与插件等价的全功能。

同时，今日构建以 `SUPPORT_AGENTS × 3` 入口按产品倍增 pack，且源码大量硬编码插件侧限定名（如 `mcp__plugin_dev-team_dev-team__…`、`dev-team:proposal-planner`），无法从同一源码干净地组装出用户级前缀 / Cursor 原生 hooks 形态的 home 镜像。

---

## 提案

新增 **`cursor-home-image/`** 第三产物（与 `claude-plugins/`、`cursor-plugins/` 并列、同 version、可 commit），形态为 **带模板的镜像目录 + Node 安装器**；安装目标仅 `~/.cursor`。隔离靠 skill / agent / hook / mcp 的 **`dev-team_` 名称前缀** + manifest，不在 `~/.cursor` 下造 `dev-team/` 发现位目录。

构建改为 **一轮 pack（不替换）+ assemble ×3**（`claude` / `cursor` / `cursorHome`）：源码只写 `__<KIND>:<id>__` 占位符与逻辑 id；assemble 按 env 表展开名称类 token、按 `hooksProfile` 组装 hooks、按显式 `outDir` 写出；`cursorHome` 路径 token 保留到安装期再绝对化。`hooks.ts` 在本变更内适配 Cursor 工具名（`Shell` / `StrReplace`），保证 home「全功能」非空心。

### 目标布局（镜像）

```
cursor-home-image/dev-team/
├── skills/dev-team_…/
├── agents/dev-team_*.md
├── hooks.json                 # cursorNative；路径 token 可仍保留
├── bin/                       # dev-team_mcp.cjs 等
├── templates/  utils/
├── mcp.dev-team.json          # merge 片段
├── install.mjs
└── manifest.json
```

安装后摊到 `~/.cursor/{skills,agents,hooks.json,mcp.json,bin,templates,utils}`（路径已绝对化；hooks/mcp 按 `dev-team_` managed merge）。

### 实现要点

1. **三套 env + 占位符**：源码字符串内写 `__SKILL:` / `__AGENT:` / `__MCP:` / `__SKILL_SLASH:` / `__BIN:` / 路径 token；禁止函数形占位符与 MCP 服务器级无 tool id 引用。
2. **pack×1 → staging CJS（保留 token）→ assemble×3**：在 `vp pack` 生命周期内收尾；禁止把 `cursorHome` 塞进 `SUPPORT_AGENTS` 倍增 pack；禁止 bundle 期按 env `define` 换 token。
3. **hooks 源 = 关键信息**：assemble 按 `claudeNested` / `cursorNative` 组装成品；禁止「Claude JSON 字符串替换出 Cursor」。
4. **磁盘名与 `namePrefix` 拉通**：plugin → `mcp.cjs` 等；home → `dev-team_mcp.cjs` 等；逻辑 id 以 FS（`readdir`）为权威，宽 globs 扫 token，无手写 id catalog。
5. **安装器（Node）**：路径绝对化 + 同步 + managed merge + manifest；不做名称前缀化 / marketplace 注册。
6. **`cursorHome` 用户级前缀（实机钉死）**：MCP server key `dev-team_mcp`；`mcpToolPrefix=mcp__user-dev-team_mcp__`；`agentRefPrefix` / `skillSlashPrefix` / `namePrefix` 对齐 `dev-team_`。

---

## 能力

### 新增能力

- **cursor-home-image** — `~/.cursor` 镜像产物布局、manifest、Node 安装器（路径绝对化、前缀化内容同步、hooks/mcp managed merge）

### 修改的能力

- **dual-platform-plugin-build** — 扩展为三产物：一轮 pack + assemble×3、env/占位符体系、`hooksProfile`、显式 `outDir`、bin/`namePrefix` 拉通；`cursor-home-image/` 与双产物同 version 并 commit
- **multi-plugin-layout** — 增加第三产物目录 `cursor-home-image/`；源码以占位符与 hooks 关键信息为权威，不再以 Claude 成品 hooks JSON 为源
- **protect-files-hook** — protect-files 识别 Cursor 工具名 `Shell` / `StrReplace`，并与既有 `Write` / `Edit` / `Bash` / `PowerShell` 兼容

---

## 变更范围

### 实现文件

- `plugins/dev-team/vite.config.ts` — 改为单轮 multi-entry pack（不按产品倍增）；`build:done`（或等价）触发 assemble×3
- `plugins/dev-team/build/**` — env 表、`applyEnvTokens`、assemble、hooksProfile 组装、FS 权威 rename、宽 globs 扫描、构建断言（名称类 token 无残留；`cursorHome` 可留路径 token）
- `plugins/dev-team/hooks/**` — hooks 关键信息源（替代以 Claude 成品 `hooks.json` 为权威）
- `plugins/dev-team/skills/**`、`plugins/dev-team/agents/**`、`plugins/dev-team/templates/**`、`plugins/dev-team/utils/**` — 硬编码限定名改为 `__<KIND>:<id>__`；MCP `tools:` frontmatter 去掉服务器级白名单，改为具体 `__MCP:<tool>__`
- `plugins/dev-team/bin/src/**` — 字符串内 token（含 `workflow.ts` 的 `agent_type`）；`hooks.ts` Cursor 工具名适配
- `cursor-home-image/dev-team/` — 构建产物（镜像 + `install.mjs` + `manifest.json`）；纳入 git
- `claude-plugins/dev-team/`、`cursor-plugins/dev-team/` — 随 assemble 刷新（plugin bin 落盘名为 `{id}.cjs`，与 `namePrefix=''` 对齐）
- `.gitignore` — staging（如 `.pack-staging/`）忽略；**不** ignore `cursor-home-image/`
- `CLAUDE.md` — 升版后重建三产物（claude / cursor / cursorHome）
- `plugins/dev-team/package.json` — 按惯例 bump version

### 测试文件

- `plugins/dev-team/bin/src/hooks.test.ts`（及邻近 hooks 单测）— 覆盖 `Shell` / `StrReplace` 与既有工具名兼容；断言可用 token 或 `expand(claude)` 后比较，不写死仅某一产品最终名（除非绑定单 env）
- `plugins/dev-team/build/**` 的单元测试（若已有/新增模块）— `applyEnvTokens`、hooksProfile 组装、managed-merge 纯函数；期望值用 token 或按 env 展开
- **不要**为「跑完整 build 并断言产物树」设计自动化测试；构建/产物验收通过手动执行构建并检查产物完成

### 不要修改

- 不删除或取代 `cursor-plugins/` marketplace 产物；不改 `.cursor-plugin/marketplace.json` 指向 home 镜像
- 不支持安装到项目 workspace `.cursor/`（可后续另开）
- 不以本机已有 `~/.cursor` 脏拷贝为规范
- 不在 `~/.cursor` 下用插件专属顶层目录做发现位隔离
- 不引入手写 id catalog / `refs.catalog.json`
- 不在 bundle 期按 env 做 token `define`/transform；不把 `cursorHome` 塞进产品倍增 pack
- 不发明第二套 bundler；打包仍为 vite-plus / `vp pack`
- marketplace `cursor` 前缀是否改为用户级规则 — **不在本变更**（插件侧沿用现网）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 三产物构建（pack×1 + assemble×3） | 手动执行插件 build 后，`claude-plugins/dev-team`、`cursor-plugins/dev-team`、`cursor-home-image/dev-team` 均存在且 version 与 `plugins/dev-team/package.json` 一致；staging 含未展开 token 的 CJS；三产物内名称类 token 无残留（`cursorHome` 可残留路径 token） |
| AC-2 | 占位符与 env 表 | 源码 skills/agents/bin 字符串使用 `__<KIND>:<id>__`；plugin 产物展开为现网插件前缀；home 产物 skill/agent/bin 带 `dev-team_`，MCP 为 `mcp__user-dev-team_mcp__…`；无 MCP 服务器级无 tool id 引用 |
| AC-3 | hooksProfile | Claude/Cursor 插件产物为 `claudeNested` 落 `hooks/hooks.json`；home 为 `cursorNative` 落根 `hooks.json`；源码不为 Claude 成品 JSON 权威 |
| AC-4 | cursor-home-image 布局 | 镜像含前缀化 skills/agents、`bin`/`templates`/`utils`、`install.mjs`、`manifest.json`、mcp merge 片段；路径 token 仍为 `__DEV_TEAM_*__` |
| AC-5 | 安装器 | 对干净或已有用户内容的 `~/.cursor`（或 `--root`）执行 `install.mjs` 后：路径已绝对化；仅 upsert/删除 `dev-team_` 托管 hooks/mcp 项；用户其它 server/条目保留；Reload 提示可见 |
| AC-6 | protect-files Cursor 工具名 | 单测：stdin `tool_name` 为 `Shell` / `StrReplace` 时与 `Bash` / `Edit` 等价保护行为；既有 Write/Edit/Bash/PowerShell 用例仍通过 |
| AC-7 | 产物可提交 | `cursor-home-image/` 未被 gitignore；与双产物一样可 commit，clone 后可装 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Cursor 变更用户级 MCP namespace（非 `user-`+key） | home 产物工具名失效 | 低 | 决策已按 2026-08-06 实机钉死；若规则变再开 change |
| SubagentStop 反参在用户级 hooks 上不一致 | static-check 续跑失效 | 中 | 实机确认；必要时 cursorHome 构建/包装原生输出 |
| 名称 token 漏写或残留 | 调用失败 | 中 | 构建断言无残留；宽 globs + FS 权威 |
| 前缀化后 Task/subagent matcher 不匹配 | static-check 不触发 | 中 | 确认 Cursor 匹配文件名还是 frontmatter；前缀策略与之对齐 |
| 用户已有同名 `dev-team_*` 或 templates/utils 冲突 | 覆盖/误删 | 中 | install 提示 + manifest 精确托管清单 |
| 安装后搬迁 home 致绝对路径失效 | hooks/mcp/agents 指旧盘 | 中 | 文档要求重跑 install；支持 `--root` |
| Windows 上 bash `openspec-cli.sh` 不等价 | Win 全功能缺口 | 中 | 文档写清平台预期 |
| minify 极端优化改写字符串字面量 | assemble 找不到 token | 低 | 已接受 terser/esbuild 不改字符串；换工具时再评 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 第三产物形态 | 镜像目录 + Node 安装器；目录名 `cursor-home-image/` | 可分发、与机器无关；安装期绝对化路径 | 仅文档指引手拷；从 cursor-plugins 二次变换 |
| 隔离方式 | 名称前缀 `dev-team_` + manifest | 与用户内容共存于同一发现目录 | `~/.cursor/dev-team/` 目录命名空间 |
| 构建形状 | pack×1（不替换）+ assemble×3 | 避免 pack 按产品倍增；token 统一管道 | 继续 SUPPORT_AGENTS×3 pack；bundle 期 define |
| 占位符语法 | `__<KIND>:<id>__` | 与路径 token 同族；不像运行时 API | `skill('id')`；`{{skill:id}}` |
| 路径 token 时机 | home：安装期；插件：构建期 | 镜像可 commit；消除 `~` 歧义 | 构建期写死机器路径 |
| hooks 源 | 关键信息 → profile 组装 | 禁止 Claude JSON 字符串替换 | 以 Claude hooks.json 为源再替换 |
| bin 落盘名 | 派生自 `namePrefix` | 与 skill/agent 同一旋钮；满足 merge 识别 | plugin 固定 `dev-team-*.cjs` |
| MCP 引用 | 仅具体 tool；禁止 server 级 | 业务只需工具 API | `__MCP_SERVER__` 特例 |
| 安装目标 | 仅 `~/.cursor` | 首版范围 | 兼支持 workspace `.cursor` |

### 待决问题

- （无阻塞项）matcher 是否匹配前缀文件名等属实现期风险，见上表；不阻塞本提案。
