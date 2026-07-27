# 提案: mcp-project-root-lock

> **变更**: mcp-project-root-lock
> **日期**: 2026-07-24
> **状态**: 草稿

---

## 问题

`dev-team` MCP 在 Cursor 中以子进程运行时，`process.cwd()` 常为用户主目录（如 `C:\Users\wps`），且共享插件 `.mcp.json` 未向 `dev-team` 注入可用的项目根环境变量。上一版 `use-mcp-roots-list` 虽增加了 `roots/list` 缓存，但现场仍会落到错误路径：

1. **Cursor roots 不可靠**：客户端常广告 `roots` capability，但 `roots/list` 返回 `-32601 Method not found`；现实现 catch 后静默回退，最终 `getProjectDir()` → `process.cwd()`（主目录）。
2. **`WORKSPACE_FOLDER_PATHS` 解析不完整**：宿主可能注入逗号拼接的多路径；当前实现仅按 `;` 分割，整串被当成 path，或在多根时误取 `[0]`。
3. **模型可覆盖项目根**：`change_list` / `config_get` / `archi_*` / `test_*` 仍接受可选 `project_root`，与「宿主绑定根」语义冲突，易被误用。
4. **`list_changed` 假复杂度**：协议有 `notifications/roots/list_changed`，但 Cursor / Claude Code 几乎不推送换根；实现亦未真正注册 handler，与「启动锁定」目标冲突。

现场探针（2026-07-24）：`change_list` 返回 `"project_root": "C:\\Users\\wps"`，说明缓存未写入且回退到错误 cwd。

---

## 提案

在 MCP 服务器 **connect 后一次性解析并锁定** `projectRoot`（进程生命周期内不变）；**删除 MCP tool input 中的 `project_root`**，使模型无法 override；采用严格失败策略，**禁止回退 `cwd`**。换工作区依赖宿主重启 MCP 子进程。

### 启动锁定优先级

```
1. CLAUDE_PROJECT_DIR 为已存在绝对路径（拒绝 ${...} 字面量）→ 锁定
   【Claude Code 宿主注入；Cursor 若已展开绝对路径亦可】
2. roots/list 成功且恰好 1 个可用 file 根 → 锁定
3. WORKSPACE_FOLDER_PATHS 解析（, 或 ;）后恰好 1 个绝对路径 → 锁定
4. 否则严格失败（不回退 cwd；多路径列表不取 [0]）
```

### 失败时机

- **connect**：尽力解析并写入缓存（不因失败直接 exit，避免宿主只显示「Not connected」）。
- **每个 MCP tool handler 入口**：强制 `requireLockedProjectRoot()`（或等价公共校验）；未锁定时返回结构化错误（未锁定 / 多根歧义 / 字面量 `${...}`），覆盖 `phase_log` / `phase_next` / `backtrack` 等本无 `project_root` 参数的工具。

### API 契约

| 层面 | 行为 |
|------|------|
| MCP input schema | 删除 `project_root`（`change_list`、`config_get`、`archi_*`、`test_detect_frameworks`、`test_resolve_paths`） |
| MCP output | 可保留只读 `project_root` 回显（如 `change_list`），便于排查锁到哪 |
| CLI / command `options.projectRoot` | **保留**（单测与 CLI fixture 注入；与 MCP 对外契约分离） |
| `list_changed` | **不实现**；对外叙事仅为「启动锁定」 |

### 配置约束（跨宿主）

- 共享插件 `.mcp.json`：**禁止**写 `${workspaceFolder}`（Claude Code 启动报错）。
- Claude Code：宿主会注入 `CLAUDE_PROJECT_DIR`；`.mcp.json` 可不写该 env。
- Cursor：依赖宿主注入的 `WORKSPACE_FOLDER_PATHS`（单值时可用）+ 可选可用的 roots；不在共享配置中写 Cursor 专用变量。

### 实现要点

1. 扩展 `lib/project-root.ts`：统一 resolve → cache；校验绝对路径 / 拒绝 `${...}`；解析 `WORKSPACE_FOLDER_PATHS`（`,` 与 `;`）；roots 仅在恰好 1 个可用时采纳；新增 `requireLockedProjectRoot()`。
2. **模块归并**：将 `utils/constant.ts` 中的 `getProjectDir()`（及其项目根解析职责）**移入** `lib/project-root.ts`，与 MCP 锁定/缓存同模块；删除 `utils/constant.ts`（及对应单测迁入 `project-root.test.ts`）。`utils/index.ts` 可继续 re-export `getProjectDir` from `../lib/project-root`，避免 command 层大面积改 import（或一并改为从 `lib/project-root` 导入，以实现时 knip/风格为准）。
3. `getProjectDir()` 语义：CLI / command 辅助入口；MCP handler **必须**走 `requireLockedProjectRoot()`。MCP 路径未锁定时不得静默返回 `cwd`。
4. `mcp.ts`：去掉 `resolveProjectRoot(args.project_root)`；全部 tool handler 入口校验锁定根；不再注册 `list_changed`。
5. 各 MCP input Zod schema 删除 `project_root` 字段。
6. 更新协议级测试（`mcp.test.ts`、`project-root.test.ts`）；删除或迁并 `constant.test.ts`；command 层 `project_root` / `projectRoot` fixture **保留**。
7. 按项目规则升级 `plugins/dev-team/.claude-plugin/plugin.json` 并重建 MCP bundle。
---

## 能力

### 新增能力

- （无）

### 修改的能力

- **mcp-project-root** — 启动锁定优先级与严格失败；移除 `list_changed` 刷新与 MCP `project_root` 覆盖；tool 入口强制校验；CLI/单测注入与 MCP 契约分离；`getProjectDir` 归并入 `lib/project-root`（删除 `utils/constant`）
- **config-get** — MCP `config_get` input 去掉 `project_root`；改用锁定根读取 `openspec/config.json`
- **test-path-resolver** — MCP `test_resolve_paths` input 去掉 `project_root`；改用锁定根解析路径

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/project-root.ts` — 启动锁定 resolve/cache；`getProjectDir` / `requireLockedProjectRoot`；绝对路径与字面量校验；`WORKSPACE_FOLDER_PATHS` 分隔；roots 恰好 1 个才采纳（承接原 `utils/constant` 职责）
- `plugins/dev-team/bin/src/utils/constant.ts` — **删除**（功能迁入 `lib/project-root`）
- `plugins/dev-team/bin/src/utils/index.ts` — 调整 re-export（若保留 `getProjectDir` 对外路径）
- `plugins/dev-team/bin/src/mcp.ts` — connect 后 init；tool 入口 `requireLocked`；删除 `args.project_root` 解析；不注册 `list_changed`
- `plugins/dev-team/bin/src/schemas/change-list.schema.ts`
- `plugins/dev-team/bin/src/schemas/config-get.schema.ts`
- `plugins/dev-team/bin/src/schemas/archi-query.schema.ts`
- `plugins/dev-team/bin/src/schemas/archi-validate.schema.ts`
- `plugins/dev-team/bin/src/schemas/archi-write.schema.ts`
- `plugins/dev-team/bin/src/schemas/archi-check.schema.ts`
- `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts`
- `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts`
- `plugins/dev-team/.claude-plugin/plugin.json` — 升级版本
- 重建 `plugins/dev-team/bin/dev-team-mcp.cjs`（若构建流程要求）

### 测试文件

- `plugins/dev-team/bin/src/lib/project-root.test.ts` — 锁定优先级、字面量拒绝、多路径严格失败、roots 恰好 1 个、不可变缓存；迁入原 `constant.test.ts` 中 `getProjectDir` 用例并按新语义调整
- `plugins/dev-team/bin/src/utils/constant.test.ts` — **删除**（用例迁入 `project-root.test.ts`）
- `plugins/dev-team/bin/src/mcp.test.ts` — 去掉 `arguments.project_root`；改为 fixture 注入缓存/env；未锁定时 tool 报错
- 如有需要：`plugins/dev-team/bin/src/lib/change.test.ts` 中依赖 MCP 缓存的用例；其它 `from '../utils'` / `getProjectDir` 的 import 随 barrel 或直接路径更新
### 不要修改

- 共享插件 `.mcp.json` 中写入 `${workspaceFolder}`（跨宿主不兼容）
- CLI / command 层 `options.projectRoot` / `project_root`（单测与 CLI fixture 保留）
- `test_cmd` 模板占位符 `{project_root}`（执行期替换，与 MCP input 无关）
- skills / agents 提示词（当前无 `project_root` 引用）
- 非 `dev-team` 插件与 LikeC4 MCP
- 实现真正的 `notifications/roots/list_changed` 热刷新

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 启动锁定优先级 | `CLAUDE_PROJECT_DIR` 为已存在绝对路径时锁定该路径；字面量 `${...}` 被拒绝；roots 恰好 1 个可用时锁定；`WORKSPACE_FOLDER_PATHS` 按 `,`/`;` 解析后恰好 1 个时锁定 |
| AC-2 | 严格失败 | 无可用唯一根时不回退 `cwd`；多路径列表不取 `[0]`；tool 返回可观测错误 |
| AC-3 | 进程内不可变 | connect 后缓存一旦锁定，进程内不再因二次 init / 通知而改变 |
| AC-4 | MCP 无 input `project_root` | 上述工具 Zod/MCP schema 不再接受 `project_root`；传入该字段被校验拒绝或忽略（以实现为准：schema 删除后拒绝） |
| AC-5 | 全 tool 入口校验 | `phase_log` / `phase_next` / `backtrack` 及带参工具在未锁定时均失败，不写主目录 |
| AC-6 | 输出回显 | `change_list` 等仍可返回只读 `project_root` 字段等于锁定根 |
| AC-7 | CLI/单测分离 | command 层显式 `project_root` / `projectRoot` fixture 仍可用；CLI 行为不强制 MCP 锁定 |
| AC-8 | Cursor 探针 | 重装/指向本地 plugin 后，`change_list` 不传参返回工作区路径；断根时报错 |
| AC-9 | Claude Code 探针 | 宿主注入的 `CLAUDE_PROJECT_DIR` 生效 |
| AC-10 | 模块归并 | `getProjectDir` 定义于 `lib/project-root.ts`；`utils/constant.ts` 已删除；无残留对 `utils/constant` 的实现依赖 |
| AC-11 | 回归 | `plugins/dev-team/bin` 单测通过；`plugin.json` 版本已升级 |
---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Cursor 多根窗口且无唯一 `CLAUDE_PROJECT_DIR`、roots 失败、`WORKSPACE_FOLDER_PATHS` >1 | MCP 工具全部报错 | 中（已接受） | 严格失败；文档说明需单值绑定根或可用 env；不取 `[0]` 猜测 |
| Cursor 双进程 / 未展开字面量 env | 部分进程锁定失败 | 中 | 拒绝 `${...}`；tool 入口返回结构化错误便于诊断 |
| 严格模式破坏依赖 cwd 的旧脚本/测试 | MCP 路径测试失败 | 中 | MCP 测试改注入缓存/env；CLI 路径保留 `options.projectRoot` |
| cache 版插件与工作树不一致 | 验收仍看到旧行为 | 高（操作风险） | 验收前重装或指向本地 plugin；探针确认进程路径 |
| roots capability 欺骗导致误调 listRoots | 噪音日志 / 短暂失败路径 | 低 | listRoots 失败计入「该通道不可用」，继续下一通道；区分日志 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 启动失败策略 | 严格模式：无唯一可信根 → 不回退 cwd | 静默主目录比报错更危险 | 保留 cwd 回退（上一版，已否决） |
| 失败时机 | connect 尽力缓存 + tool 入口强制校验 | connect exit 时模型拿不到结构化错误；无参工具也必须覆盖 | 仅 connect 失败退出 |
| `list_changed` | 不实现 | 宿主几乎不推送；与硬锁冲突 | 注册 notification 热刷新 |
| MCP `project_root` 参数 | 删除 | 模型不可 override 绑定根 | 保留显式覆盖（上一版 AC-7，已否决） |
| 输出 `project_root` | 保留只读回显 | 便于排查锁定结果 | 完全删除输出字段 |
| CLI / 单测 `options.projectRoot` | 保留 | 与 MCP 对外契约分离 | MCP/CLI 一并删除 |
| 多 root | 找不到唯一绑定根才失败；全量 >1 不取 `[0]` | `WORKSPACE_FOLDER_PATHS` 是窗口全量，≠ agent 绑定根 | 始终取 `[0]` |
| 共享 `.mcp.json` | 不写 `${workspaceFolder}` | Claude Code 启动报错（实测） | Cursor 专用配置副本 |
| 锁定优先级 | env 绝对路径 → roots 恰 1 → WORKSPACE 恰 1 | Claude 可靠；Cursor 第二通道靠宿主 env | roots 优先于 env |
| `getProjectDir` 模块位置 | 迁入 `lib/project-root.ts`，删除 `utils/constant.ts` | 项目根解析与 MCP 锁定同域，避免双模块分叉 | 保留 thin `constant.ts` re-export（已否决为长期形态；允许 `utils/index` 短期 re-export） |

### 待决问题

- 无（探索中开放项已拍板；残留「多根严格失败」已接受为预期行为；模块归并已补充）
