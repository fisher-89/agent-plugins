# 提案: dual-platform-plugin-build

> **变更**: dual-platform-plugin-build
> **日期**: 2026-07-27
> **状态**: 草稿

---

## 问题

当前 `dev-team` 插件仅面向 Claude Code：源码树内嵌 `.claude-plugin/`，根 `.claude-plugin/marketplace.json` 直接指向 `./plugins/dev-team`。引入 Cursor 后，工具名、hooks 反参等平台差异会导致部分功能失效，但源码与可安装产物未分离，无法为两个宿主各自产出可安装插件。

本迭代先解决布局与构建骨架问题：没有「源码 / 双产物 / 双清单」结构，就无法在不污染源码的前提下分别面向 Claude Code 与 Cursor 发布，也无法把版本权威收敛到单一 `package.json`。

---

## 提案

将仓库改造成 **源码不直接安装、根脚本构建双产物、手写双 marketplace** 的布局；本迭代只落地骨架与最小可装差异，**不**钉死工具名替换表与 hooks 反参适配。

### 目标布局

```
wps-claude-plugin/
├── .claude-plugin/marketplace.json   # → ./claude-plugins/dev-team
├── .cursor-plugin/marketplace.json   # → ./cursor-plugins/dev-team
├── scripts/build-plugins.*           # 根构建入口
├── plugins/dev-team/                 # 源码（无平台 manifest）
│   ├── package.json                  # version = 权威版本
│   ├── bin/                          # TS + vite-plus（不再自带 package.json）
│   └── skills/ agents/ hooks/ …
├── claude-plugins/dev-team/          # Claude 产物 · commit
│   └── .claude-plugin/plugin.json
└── cursor-plugins/dev-team/          # Cursor 产物 · commit
    └── .cursor-plugin/plugin.json
```

### 实现要点

1. **源码净化**：`plugins/dev-team` 移除 `.claude-plugin/`；将 `bin/package.json` 上移为 `plugins/dev-team/package.json`，`version` 迁入当前对外发布版 **2.10.3**（不得沿用 `bin/package.json` 的 `1.0.0`）。
2. **双产物目录**：新增并 **commit** `claude-plugins/`、`cursor-plugins/`；git 即 marketplace 源，clone 后可直接安装。
3. **根构建脚本**（如 `scripts/build-plugins.mjs`）负责编排：
   - 读取 `plugins/dev-team/package.json` → `version`
   - 在插件源码根（`package.json` 上移后的 `plugins/dev-team`）执行 **`vp pack`（vite-plus）**，打包 mcp / cli / hooks 的 CJS 产物
   - 组装到 `claude-plugins/dev-team` 与 `cursor-plugins/dev-team`
   - 写入各自平台 `plugin.json` 并注入同一 `version`
4. **JS 打包工具锁定**：继续使用现有 **vite-plus**，命令为 **`vp pack`**。不引入第二套 bundler；与现有 `bin` 打包方式保持一致，仅迁移工作目录（至插件源码根）与产物组装路径。
5. **双清单手写**：
   - `.claude-plugin/marketplace.json` → `source: "./claude-plugins/dev-team"`
   - `.cursor-plugin/marketplace.json` → `source: "./cursor-plugins/dev-team"`
6. **`.gitignore`**：增加 `!.cursor-plugin`（现有 `.*` 会忽略点目录）；**不要** ignore 产物目录。
7. **文档**：`CLAUDE.md` 升版规则改为：改插件代码 → 升 `plugins/<name>/package.json` → 重建双产物。

本迭代允许两边产物在 skills/agents/hooks **内容暂同构或仅做最小可装差异**；工具名模板替换、hooks 反参适配、全量 MCP 命名空间替换等留待实机调试后的后续变更。

---

## 能力

### 新增能力

- **dual-platform-plugin-build** — 根构建入口：以 `vp pack`（vite-plus）打包 JS、组装 Claude/Cursor 双产物、注入统一版本、产物可提交安装

### 修改的能力

- **multi-plugin-layout** — `plugins/` 改为源码树（无平台 manifest）；`package.json` 上移并成为版本权威；可安装内容落在 `claude-plugins/` / `cursor-plugins/`
- **marketplace-config** — 手写 Claude / Cursor 双 marketplace，分别指向对应产物目录

---

## 变更范围

### 实现文件

- `scripts/build-plugins.mjs`（或等价根构建入口）— 读版本、在 `plugins/dev-team` 调用 `vp pack`、组装双产物、写平台 `plugin.json`
- `plugins/dev-team/package.json` — 自 `bin/package.json` 上移；`version` 迁为 `2.10.3`（或约定的下一 semver）；保留 `build` 脚本为 `vp pack`（vite-plus）
- `plugins/dev-team/bin/package.json` — **删除**（内容并入上级）
- `plugins/dev-team/.claude-plugin/` — **删除**（源码不再含平台 manifest）
- `claude-plugins/dev-team/` — Claude 可安装产物（含 `.claude-plugin/plugin.json`）
- `cursor-plugins/dev-team/` — Cursor 可安装产物（含 `.cursor-plugin/plugin.json`）
- `.claude-plugin/marketplace.json` — `source` 改为 `./claude-plugins/dev-team`
- `.cursor-plugin/marketplace.json` — **新增**，指向 `./cursor-plugins/dev-team`
- `.gitignore` — 增加 `!.cursor-plugin`；确保产物目录不被忽略
- `CLAUDE.md` — 升版规则改为 `plugins/<name>/package.json` + 重建双产物
- 构建所需的路径引用微调（`vp pack` 工作目录迁至插件源码根、文档中的安装路径说明）

### 测试文件

- 构建脚本的冒烟/集成测试（若仓库惯例为脚本级测试：断言产物目录存在、两边 `plugin.json` 的 `version` 与源 `package.json` 一致、源码树无平台 manifest）
- 既有 `plugins/dev-team/bin` 单测：随 `package.json` 上移调整工作目录/路径假设（若有）

### 不要修改

- 业务 MCP / CLI / hooks 的行为逻辑（除非组装产物所必需的路径微调）
- 引入第二套 bundler，或以 webpack / esbuild / rollup / tsup 等替代 `vp pack`（vite-plus）
- 全量工具名替换表（`Bash`↔`Shell`、`Edit`↔`StrReplace` 等）— 留待后续迭代
- hooks 反参适配（PreToolUse / SubagentStop 等平台差异）— 留待实机调试后处理
- skill / agent 中 MCP 工具命名空间的批量替换
- 发明新的 change 目录结构或 OpenSpec 工作流语义
- 自动将 `claude-plugins/` / `cursor-plugins/` 加入 `.gitignore`

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 源码无平台 manifest | `plugins/dev-team` 下不存在 `.claude-plugin/` 或 `.cursor-plugin/` |
| AC-2 | 版本权威上移 | 存在 `plugins/dev-team/package.json`；无 `plugins/dev-team/bin/package.json`；`version` 为 `2.10.3`（或约定下一 semver），非 `1.0.0` |
| AC-3 | 根构建入口 | 仓库根存在可执行的 `scripts/build-plugins.*`；运行后在 `plugins/dev-team` 调用 `vp pack`（vite-plus）并刷新双产物 |
| AC-4 | Claude 产物 | `claude-plugins/dev-team/.claude-plugin/plugin.json` 存在，且 `version` 等于源 `package.json` 的 `version` |
| AC-5 | Cursor 产物 | `cursor-plugins/dev-team/.cursor-plugin/plugin.json` 存在，且 `version` 等于源 `package.json` 的 `version` |
| AC-6 | 双 marketplace | `.claude-plugin/marketplace.json` 的 dev-team `source` 为 `./claude-plugins/dev-team`；`.cursor-plugin/marketplace.json` 的 `source` 为 `./cursor-plugins/dev-team` |
| AC-7 | git 可跟踪 | `.gitignore` 含 `!.cursor-plugin`；`claude-plugins/`、`cursor-plugins/` 未被 ignore，可被 commit |
| AC-8 | 文档升版规则 | `CLAUDE.md` 写明：改插件代码后升 `plugins/<name>/package.json` 并重建双产物 |
| AC-9 | 最小可装 | 本迭代两边产物可按 marketplace 路径安装；允许 skills/agents/hooks 内容暂同构 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 上移 `package.json` 时误用 `1.0.0` | 对外版本回退 | 中 | 验收强制核对迁入 `2.10.3`（或约定下一 semver） |
| 贡献者改源后未跑构建即 commit | 产物与源不一致 | 中 | 文档明确流程；开放项可后续加 CI 校验（本迭代不强制） |
| Cursor 产物缺 `mcp.json` 导致 MCP 不可用 | Cursor 侧部分功能失败 | 中 | 本迭代允许预留组装钩子；是否首期带入列为待决；功能适配属后续迭代 |
| 同构产物在 Cursor 上因工具名/反参差异仍失效 | 安装成功但运行异常 | 高（已接受） | 明确本迭代只交付布局与构建骨架；差异表留待实机调试 |
| `.*` ignore 导致 `.cursor-plugin` 无法入库 | Cursor marketplace 丢失 | 中 | `.gitignore` 显式 `!.cursor-plugin` |
| `package.json` 上移后 `vp pack` 工作目录/相对路径失效 | 构建失败 | 中 | 根脚本固定在 `plugins/dev-team`（源码根）调用 `vp pack`；不换 bundler；跑通一次全量构建 |
| 误引入第二套 bundler 导致双轨打包 | 维护成本上升、产物不一致 | 低 | 决策锁定 vite-plus / `vp pack`；范围明确禁止替代 bundler |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 产物是否 commit | 提交 `claude-plugins/`、`cursor-plugins/` | git 即 marketplace；clone 即可装 | 仅 CI 产物 / release 附件 |
| 构建入口位置 | 仓库根 `scripts/` | 跨插件、跨平台统一入口 | 放在 `plugins/dev-team/bin` |
| JS 打包工具 | **vite-plus**，命令 **`vp pack`** | 与现有 `bin` 打包一致；仅迁 cwd 与组装路径 | 引入 webpack / esbuild / rollup / tsup 等第二套 bundler |
| `vp pack` 工作目录 | `plugins/dev-team`（`package.json` 上移后的源码根） | 版本与 pack 配置同根；根脚本只做编排 | 仍在 `bin/` 下 pack；或根脚本内联另一套打包 |
| 源码是否含平台 manifest | 不含 | 避免单平台污染源；由构建写入产物 | 源码保留 `.claude-plugin` 再复制 |
| 版本权威 | `plugins/<name>/package.json` 的 `version` | 单一来源；构建注入两边 `plugin.json` | 继续以 `.claude-plugin/plugin.json` 为准 |
| marketplace | 手写双清单 | 路径稳定、意图清晰 | 构建生成 marketplace |
| 本迭代适配深度 | 仅布局 + 构建骨架；内容可同构 | 先可装可版本化，再实机调差异 | 一次做完全量替换表 |
| `.gitignore` | 放行 `.cursor-plugin` 与产物目录 | 否则 Cursor 清单/产物无法入库 | 改全局 `.*` 规则 |

### 待决问题

- 日常是否强制「改源后必须跑根构建再 commit」，或加 CI 校验产物与源一致
- Cursor 产物是否第一期就带 `mcp.json`（可在组装时预留）
- `package.json` 的 `name` 是否从 `dev-team-cli` 改为 `dev-team`
