# dual-platform-plugin-build

探索时间：2026-07-27

## 问题

当前 `dev-team` 插件仅面向 Claude Code。引入 Cursor 时，工具名称、hooks 反参结构等差异导致部分功能失效。目标：将插件整体改造成构建产物，区分 Claude Code 与 Cursor 两套产物。

## 本轮收敛：先区分产物

本迭代优先落地 **源码 / 双产物 / 双清单** 布局与构建骨架。  
**工具名模板替换、hooks 反参适配等细节** 留到实际在 Cursor / Claude Code 上调试后再开下个迭代处理。

## 已锁定决策

1. **产物 commit**：`claude-plugins/`、`cursor-plugins/` 提交进仓库（git 即 marketplace 源，clone 后可直接安装）。
2. **构建入口**：仓库根 `scripts/`（例如 `scripts/build-plugins.mjs`）。
3. **源码不带平台 manifest**：
   - `plugins/` 为源码目录，**不含** `.claude-plugin` / `.cursor-plugin`。
   - 将现有 `plugins/dev-team/bin/package.json` **上移**到 `plugins/dev-team/package.json`。
   - 插件版本号以该 `package.json` 的 `version` 为权威来源；构建时写入两边产物的 `plugin.json`。
4. **marketplace 手写双清单**：
   - `.claude-plugin/marketplace.json` → `source: "./claude-plugins/dev-team"`
   - `.cursor-plugin/marketplace.json` → `source: "./cursor-plugins/dev-team"`

## 目标布局

```
wps-claude-plugin/
├── .claude-plugin/
│   └── marketplace.json          # 手写 → ./claude-plugins/dev-team
├── .cursor-plugin/
│   └── marketplace.json          # 手写 → ./cursor-plugins/dev-team
├── scripts/
│   └── build-plugins.*           # 根构建入口
│
├── plugins/                      # 源码（不直接安装）
│   └── dev-team/
│       ├── package.json          # version = 插件权威版本
│       ├── bin/                  # TS 源 + vite/tsconfig（不再自带 package.json）
│       ├── skills/ agents/ hooks/ templates/ utils/
│       └── （无 .claude-plugin / 无 .cursor-plugin）
│
├── claude-plugins/               # Claude 产物 · commit
│   └── dev-team/
│       ├── .claude-plugin/plugin.json
│       └── …
│
└── cursor-plugins/               # Cursor 产物 · commit
    └── dev-team/
        ├── .cursor-plugin/plugin.json
        └── …
```

## 版本迁移动作（实现时注意）

- 对外发布版当前在 `plugins/dev-team/.claude-plugin/plugin.json`：**2.10.3**
- `bin/package.json` 现为 **1.0.0**，上移时 **不得**沿用 1.0.0；应迁入 **2.10.3**（或下一个约定 semver）
- `CLAUDE.md` 升版规则改为：改插件代码 → 升 `plugins/<name>/package.json` → 重建双产物

## `.gitignore`

- 增加 `!.cursor-plugin`（现有 `.*` 规则会忽略点目录）
- **不要** ignore `claude-plugins/`、`cursor-plugins/`

## 构建骨架职责（本迭代）

根脚本大致：

1. 读 `plugins/dev-team/package.json` → `version`
2. 在 `plugins/dev-team` 执行现有 JS pack（mcp / cli / hooks）
3. 组装目录到 `claude-plugins/dev-team` 与 `cursor-plugins/dev-team`
4. 写入各自平台 `plugin.json`（注入同一 `version`）

本迭代允许两边产物在 skills/agents/hooks **内容上暂同构或仅做最小可装差异**；不要求一次做完替换表。

## 已知差异（下迭代 / 调试后再收敛）

以下已观察到，**本轮不钉死替换表**：

| 维度 | Claude Code | Cursor（观察） |
|------|-------------|----------------|
| Shell 工具 | `Bash` | `Shell` |
| Edit 工具 | `Edit` | `StrReplace` |
| MCP 在 skill 中的引用 | `mcp__plugin_dev-team_dev-team__*` | 命名空间形式不同，需实机确认 |
| PreToolUse 反参 | `hookSpecificOutput.permissionDecision` | flat `permission`（Cursor 声称可映射 Claude 格式） |
| SubagentStop 反参 | `{ decision, reason }` | `{ followup_message }` — 硬差异 |
| hooks matcher | `Bash` / `Write\|Edit` | 需对齐 `Shell` / `Write\|StrReplace` 等 |
| 问用户 | `AskUserQuestion` | 无同名工具 |
| Cursor MCP 打包 | — | 可能需要产物内 `mcp.json` |

## 明确不做（本探索轮 / 本迭代意图）

- 不在未实机调试前写死全量模板占位符与替换表
- 不自动改业务 MCP 逻辑（除非组装产物所必需）
- 不发明 change 目录；待布局方案够稳再 `/dev-team:phase-proposal`

## 开放问题（可随实现微调）

- 日常是否强制「改源后必须跑根构建再 commit」，或加 CI 校验产物与源一致
- Cursor 产物是否第一期就带 `mcp.json`（可在组装时预留）
- `package.json` 的 `name` 是否从 `dev-team-cli` 改为 `dev-team`

## 补充（2026-07-27）：打包工具锁定

- JS 打包（mcp / cli / hooks CJS）**使用 vite-plus**，命令为 `vp pack`。
- 根 `scripts/` 构建入口负责编排：在插件源码根（`package.json` 上移后的 `plugins/dev-team`）调用 `vp pack`，再组装 `claude-plugins/` 与 `cursor-plugins/`。
- 不引入第二套 bundler；与现有 `bin` 打包方式保持一致，仅迁移工作目录与产物组装路径。
