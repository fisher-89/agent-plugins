# 设计: dual-platform-plugin-build

> **变更**: dual-platform-plugin-build
> **日期**: 2026-07-27

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 根构建编排 | 读权威 `version`、在源码根调用 `vp pack`、组装 Claude/Cursor 双产物、写入平台 `plugin.json` | `scripts/build-plugins.mjs` | Node `fs`/`path`/`child_process`；vite-plus CLI | Node.js ESM |
| 插件源码树 | 可编辑源（无平台 manifest）；`package.json` 为版本权威；含 `bin/` TS 与 vite-plus 配置 | `plugins/dev-team/` | vite-plus、现有 runtime deps | TypeScript / JSON |
| vite-plus 打包 | 将 mcp / cli / hooks 打成 CJS（及 schema 旁路产物） | `plugins/dev-team` 上执行 `vp pack`；配置见 `vite.config.ts` | `vite-plus` | vite-plus `pack` |
| Claude 产物树 | 可安装、可 commit 的 Claude Code 插件树 | `claude-plugins/dev-team/` | 构建脚本组装 | 文件系统产物 |
| Cursor 产物树 | 可安装、可 commit 的 Cursor 插件树（本迭代内容可与 Claude 同构） | `cursor-plugins/dev-team/` | 构建脚本组装 | 文件系统产物 |
| Claude marketplace | 手写清单，指向 Claude 产物 | `.claude-plugin/marketplace.json` | 无 | JSON |
| Cursor marketplace | 手写清单，指向 Cursor 产物 | `.cursor-plugin/marketplace.json` | 无 | JSON |

### 构建数据流

```
plugins/dev-team/package.json (version)
        │
        ▼
scripts/build-plugins.mjs
        │
        ├─► cwd=plugins/dev-team → vp pack  → bin/*.cjs 等
        │
        ├─► 组装 claude-plugins/dev-team/ + .claude-plugin/plugin.json
        │
        └─► 组装 cursor-plugins/dev-team/  + .cursor-plugin/plugin.json
```

本迭代**不**做工具名替换表、hooks 反参适配、MCP 命名空间批量替换；两边产物允许 skills/agents/hooks 同构。

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `scripts/build-plugins.mjs` | 仓库根构建入口：读 version、调用 `vp pack`、组装双产物、写平台 manifest |
| `plugins/dev-team/package.json` | 自 `bin/package.json` 上移；`version` 设为 `2.10.3`；`scripts.build` 保持 `vp pack` |
| `plugins/dev-team/vite.config.ts` | 自 `bin/vite.config.ts` 上移（或等价迁到源码根），pack/test 路径改为相对源码根下的 `bin/` |
| `.cursor-plugin/marketplace.json` | Cursor marketplace；`dev-team.source` = `./cursor-plugins/dev-team` |
| `claude-plugins/dev-team/.claude-plugin/plugin.json` | Claude 产物 manifest（由构建写入；目录一并 commit） |
| `claude-plugins/dev-team/**` | Claude 可安装树（agents/skills/hooks/utils/templates/bin 运行时文件等） |
| `cursor-plugins/dev-team/.cursor-plugin/plugin.json` | Cursor 产物 manifest（由构建写入） |
| `cursor-plugins/dev-team/**` | Cursor 可安装树（本迭代可与 Claude 同构） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `.claude-plugin/marketplace.json` | `plugins[].source`：`./plugins/dev-team` → `./claude-plugins/dev-team` | marketplace 指向产物而非源码 |
| `.gitignore` | 增加 `!.cursor-plugin`；确认无忽略 `claude-plugins/` / `cursor-plugins/` 的规则 | 允许 Cursor 清单与双产物入库 |
| `CLAUDE.md` | 升版规则改为：改插件代码 → 升 `plugins/<name>/package.json` → 跑根构建刷新双产物 | AC-8 |
| `plugins/dev-team/bin/tsconfig.json` | 如因工程根上移导致解析失败，微调 `include`/`types` 或由根脚本/文档约定工作目录 | 路径引用微调（按需） |
| `plugins/dev-team/bin/knip.json` | 按工程根上移调整 entry/project 相对路径或 knip 调用方式 | 路径引用微调（按需） |
| `plugins/dev-team/bin/stryker.config.json` | 按需调整相对路径 | 路径引用微调（按需） |
| `plugins/dev-team/bin/pnpm-lock.yaml` | 随 `package.json` 上移到 `plugins/dev-team/pnpm-lock.yaml`（若存在） | 锁文件与包清单同根 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/package.json` | 内容并入 `plugins/dev-team/package.json` 后删除 |
| `plugins/dev-team/.claude-plugin/plugin.json` | 源码不再含平台 manifest |
| `plugins/dev-team/.claude-plugin/` | 删除整个目录 |
| `plugins/dev-team/bin/vite.config.ts` | 上移到 `plugins/dev-team/vite.config.ts` 后删除原文件（若采用「配置留在 bin + `--config`」方案则可不删，见待决；默认采用上移） |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `build-plugins`（CLI） | `scripts/build-plugins.mjs` | 新增 | `node scripts/build-plugins.mjs` → `Promise<void>`（进程 exit `0` 成功 / 非 `0` 失败） | 根构建入口。步骤：(1) 读 `plugins/dev-team/package.json` 的 `version`；(2) 在 cwd `plugins/dev-team` 执行 `vp pack`；(3) 组装 `claude-plugins/dev-team` 与 `cursor-plugins/dev-team`；(4) 写入两边平台 `plugin.json` 并注入同一 `version`。无模块级 export 要求 |

<!--
  构建脚本内部辅助（读 JSON、复制目录、写 manifest、spawn vp）为模块私有，不列入。
  业务 MCP / CLI / hooks 公共 API 本迭代不变更。
-->

<!-- 无新增/修改 TypeScript interface、type alias、enum 或公共 class：构建为脚本编排，类型若存在则仅模块内部使用 -->

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `version` | `plugins/dev-team/package.json` | 新增（上移） | `string` | `"2.10.3"` | 插件权威 semver；禁止沿用旧 bin 的 `1.0.0` |
| `scripts.build` | `plugins/dev-team/package.json` | 新增（上移） | `string` | `"vp pack"` | 锁定 vite-plus；根脚本可直接 `vp pack` 或 `pnpm run build` |
| `name` | `plugins/dev-team/package.json` | 新增（上移） | `string` | `"dev-team-cli"`（暂保留） | 是否改为 `dev-team` 见待决；不影响对外插件 `name` |
| `plugins[].source` | `.claude-plugin/marketplace.json` | 修改 | `string` | `"./claude-plugins/dev-team"` | 指向 Claude 产物 |
| `plugins[].name` | `.cursor-plugin/marketplace.json` | 新增 | `string` | `"dev-team"` | Cursor marketplace 插件名 |
| `plugins[].source` | `.cursor-plugin/marketplace.json` | 新增 | `string` | `"./cursor-plugins/dev-team"` | 指向 Cursor 产物 |
| `plugins[].description` | `.cursor-plugin/marketplace.json` | 新增 | `string` | 与产物 manifest 一致 | 与 Claude 侧描述对齐即可 |
| `name` / `version` / `description` / `bin` / `author` / `openspecVersion` | `claude-plugins/dev-team/.claude-plugin/plugin.json` | 新增（构建写入） | 见数据模型 | `name="dev-team"`，`version`=源 package.json | 由构建注入 version，其余自现有 Claude manifest 约定生成 |
| `name` / `version` / `description` | `cursor-plugins/dev-team/.cursor-plugin/plugin.json` | 新增（构建写入） | `string` | 与 Claude 侧同构的最小字段集 | 本迭代允许与 Claude 元数据同构；Cursor 特有字段不强制 |
| `!.cursor-plugin` | `.gitignore` | 新增 | ignore 例外 | — | 覆盖 `.*` 对点目录的忽略 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `PackageVersionAuthority` | `name: string`；`version: string`（semver，权威） | 一对多注入两边 `PluginManifest.version` | `plugins/dev-team/package.json` |
| `PluginManifest`（Claude） | `name: "dev-team"`；`version: string`；`description: string`；`bin: "./bin"`；`openspecVersion: string`；`author: { name: string }` | 对应 marketplace 中 `dev-team` 条目；由构建写入产物 | `claude-plugins/dev-team/.claude-plugin/plugin.json` |
| `PluginManifest`（Cursor） | `name: "dev-team"`；`version: string`；`description: string`；其余平台字段本迭代可选 | 对应 Cursor marketplace 条目；version 与 Claude 相同 | `cursor-plugins/dev-team/.cursor-plugin/plugin.json` |
| `MarketplaceManifest` | `name` / `owner`（Claude）；`plugins[]: { name, source, description }` | `source` 指向产物目录，不指向 `plugins/` | `.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json` |
| `ProductTree` | 可安装内容：`agents/`、`skills/`、`hooks/`、`utils/`、`templates/`、`.mcp.json`、`bin/` 运行时文件（`dev-team-*.cjs`、`dev-team-config.schema.json`、`openspec`、`openspec.cmd`、`openspec-bundled.js`） | 由源码树复制/同步；**不含** `bin/src`、`bin/__tests__`、开发配置、`node_modules`、`.map` | `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`（git commit） |

### 组装规则（本迭代）

1. **刷新策略**：每次根构建对目标产物目录做「清空后重建」或等价的完整同步，避免残留旧文件。
2. **同构复制**：Claude / Cursor 两边复制同一套插件内容（含现有 `.mcp.json`）；**不**做工具名/hooks 差异变换。
3. **manifest 分写**：仅平台目录名与 `plugin.json` 路径不同（`.claude-plugin` vs `.cursor-plugin`），`version` 必须相同。
4. **Cursor `mcp.json`**：本迭代不强制单独生成；可在脚本中预留扩展点（例如 `assembleCursorExtras()` 空实现）。是否首期写入 Cursor 专用 MCP 清单见待决。
5. **源码净化**：构建不得在 `plugins/dev-team` 下创建 `.claude-plugin/` 或 `.cursor-plugin/`。

### `vp pack` 工作目录与路径

- **cwd**：`plugins/dev-team`（与上移后的 `package.json` 同根）。
- **命令**：`vp pack`（vite-plus）；禁止引入 webpack / esbuild CLI / rollup / tsup 等第二套 bundler。
- **配置**：`vite.config.ts` 位于源码根；`pack` 的 `entry` / `outputOptions.file` 指向 `bin/src/*.ts` → `bin/dev-team-*.cjs`（及现有 schema 写入逻辑同步改为 `bin/dev-team-config.schema.json`）。
- **开发脚本**：`test` / `check` / `knip` 等继续通过根 `package.json` scripts 调用；include 路径改为 `bin/src/**`、`bin/__tests__/**` 等。

---

## 依赖

### 运行时依赖

- 无新增运行时 npm 依赖；产物侧继续依赖已打包进 `dev-team-*.cjs` 的现有库，以及宿主提供的 Node。

### 构建/测试依赖

- `vite-plus`（已有）— 提供 `vp pack`；工作目录迁至 `plugins/dev-team`
- Node.js 内置 `fs` / `path` / `child_process`（或 `node:fs/promises` 等）— 根构建脚本编排与文件组装
- 不新增第二套 bundler 或构建框架

---

## 待决问题

- 日常是否强制「改源后必须跑根构建再 commit」，或加 CI 校验产物与源一致（本迭代不强制 CI）
- Cursor 产物是否第一期就带专用 `mcp.json`（脚本可预留组装钩子；默认本迭代两边复制现有 `.mcp.json` 即可）
- `package.json` 的 `name` 是否从 `dev-team-cli` 改为 `dev-team`（不影响 marketplace / 平台 manifest 的 `name: "dev-team"`）
- `vite.config.ts` 若 `vp pack --config bin/vite.config.ts` + `root=bin` 更少移动文件，可在实现时二选一；验收以「cwd=`plugins/dev-team` 且命令为 `vp pack`」为准
