# MCP projectRoot 启动锁定

> 日期: 2026-07-24  
> 状态: 探索中（尚无 change）  
> 相关 archive: `openspec/changes/archive/2026-06-18-use-mcp-roots-list`

---

## 问题

`dev-team` MCP 在 Cursor 中以子进程运行时，`process.cwd()` 常为用户主目录（如 `C:\Users\wps`），且 `.mcp.json` 未给 `dev-team` 注入项目根环境变量。下游 `change_list` / `phase_log` / `phase_next` 等会读写错误路径。

现场探针（2026-07-24 Cursor 会话）：`change_list` 返回 `"project_root": "C:\\Users\\wps"`，说明当前会话里 MCP roots 缓存未生效（或回退到错误 cwd）。

---

## 目标方向

1. **MCP 服务器启动时**读取并**锁定** `projectRoot`（进程生命周期内不变）。
2. **MCP tool API 不再支持**传入 `project_root` 参数（模型不能 override）。
3. **兼容 Claude Code 与 Cursor** 两条可信来源。

---

## 现状（代码）

| 点 | 现状 |
|----|------|
| `lib/project-root.ts` | `connect` 后 `initProjectRootFromMcp` → `listRoots()` → 模块级缓存 |
| `getProjectDir()` | 缓存 → `CLAUDE_PROJECT_DIR` → `WORKSPACE_FOLDER_PATHS[0]` → `cwd` |
| `list_changed` | 上一版 design 写了刷新；实现**未注册** notification handler；`refresh*` 仅被 init 复用 |
| tool schema | `change_list` / `config_get` / `archi_*` / `test_*` 仍有可选 `project_root` |
| `phase_log` / `phase_next` | 本来就无 `project_root`，只靠 `getProjectDir()` |
| `.mcp.json` | `dev-team` 无 env；`likec4` 使用 `${CLAUDE_PROJECT_DIR}/openspec/architecture` |

与上一版 `use-mcp-roots-list` 的差异：该 change **保留**显式 `project_root` 优先（AC-7）。本次探索刻意收权。

---

## 宿主兼容矩阵

```
Claude Code                         Cursor
─────────────                       ──────
CLAUDE_PROJECT_DIR 通常可靠          cwd ≈ 用户主目录，env 常未注入
.mcp.json 可展开 ${CLAUDE_PROJECT_DIR}   roots/list 是正解（若声明）
roots 若声明也可                     不能只靠 env
```

建议启动时解析优先级（锁定用）：

1. `roots/list` 首个 root（客户端声明 `roots` 时）
2. `CLAUDE_PROJECT_DIR`
3. `WORKSPACE_FOLDER_PATHS[0]`（若仍有场景）
4. `cwd` / 或 fail-fast（待决）

配置侧：可给 `dev-team` 对齐 `likec4`，在 `.mcp.json` 注入 `CLAUDE_PROJECT_DIR=${CLAUDE_PROJECT_DIR}`，作为 Claude Code 保险带；Cursor 仍依赖 roots。

---

## 决策：不实现 list_changed

协议 / SDK 定义了：

- capability: `roots.listChanged`
- notification: `notifications/roots/list_changed`

但工程现实：

- Cursor / Claude Code 实际几乎不靠它推送换根
- 我们代码未注册 handler；测试用二次 `init` mock「刷新」，并非真 notification
- 与「启动锁定」目标冲突，属于假复杂度

**结论：硬锁。** `connect` 后一次性 `roots/list`（或 env 回退），写入缓存后进程内不变。换工作区依赖宿主重启 MCP 子进程。

命名上 `refreshProjectRootFromMcp` 可视为 init 内部拉取实现细节，对外叙事只保留「启动锁定」。

---

## API 契约变更（拟议）

从 MCP **input schema 删除** `project_root`：

- `change_list` / `config_get`
- `archi_query` / `archi_validate` / `archi_write` / `archi_check`
- `test_detect_frameworks` / `test_resolve_paths`

输出字段可保留 `project_root`（只读回显，便于排查锁到哪）。

CLI / 单测内部的 `options.projectRoot` **可保留**（进程内注入 fixture），与 MCP 对外契约分离。

---

## 已拍板（2026-07-24）

| # | 议题 | 结论 |
|---|------|------|
| 2 | 启动失败策略 | **严格模式**：roots 与可信 env 皆失败 → 不回退 `cwd`；tool 报错 / 启动失败可观测 |
| 3 | 输出 `project_root` | **保留**（只读回显） |
| 5 | CLI / 单测 `options.projectRoot` | **保留**（与 MCP 对外契约分离） |
| 4 | 多 root | 全量 >1 且无其它唯一源 → **严格失败（可接受）**；有 `CLAUDE_PROJECT_DIR` 真路径则锁定绑定根 |
| 6 | 失败时机 | **connect 时尽力解析并缓存；tool 入口强制校验**（理由见下） |

---

## 问题 1：Cursor 文档 — 第二通道怎么给根

### 官方文档（[cursor.com/docs/mcp](https://cursor.com/docs/mcp)）

| 能力 | 文档说法 | 本会话现实 |
|------|----------|------------|
| **Roots** | Protocol 表标为 **Supported**（server 可询问 URI/filesystem boundaries） | 广告 capability 但 `roots/list` 常 `-32601`；本会话缓存未写入 |
| **Config interpolation**（`.cursor/mcp.json` / `~/.cursor/mcp.json`） | `env` 等字段可展开：`${workspaceFolder}`、`${env:NAME}`、`${userHome}` 等 | **文档写明的 Cursor 侧可靠注入方式** |
| `${workspaceFolder}` | 「project root（含 `.cursor/mcp.json` 的文件夹）」— **单根语义** | 适合作为 Cursor 第二通道 |

示例（官方）：

```json
{
  "mcpServers": {
    "local-server": {
      "command": "python",
      "args": ["${workspaceFolder}/tools/mcp_server.py"],
      "env": { "API_KEY": "${env:API_KEY}" }
    }
  }
}
```

### Plugin `mcp.json`（[plugins reference](https://cursor.com/docs/reference/plugins)）

- 插件根目录 `mcp.json` 自动发现。
- 文档强调的占位符是 **`${VAR}` plugin variables**（`plugin.json` → `variables` schema → 用户/Dashboard 配置），**不是** `${workspaceFolder}`。
- 未写明：Claude 风格 `${CLAUDE_PLUGIN_ROOT}` / `${CLAUDE_PROJECT_DIR}` 在 Cursor 加载插件时如何展开。
  - 本会话实证：`CLAUDE_PLUGIN_ROOT` **已展开**（进程命令行指向 cache 绝对路径）。
  - 本会话实证：`dev-team` **未配置** `CLAUDE_PROJECT_DIR` env；进程内该变量不存在；`WORKSPACE_FOLDER_PATHS=""`.

### 对「双通道」的含义（已修正）

```
共享插件 .mcp.json
  ❌ 不可写 ${workspaceFolder}（Claude Code 启动报错；用户实测）
  ✅ 仅 Claude 占位符：${CLAUDE_PLUGIN_ROOT} / ${CLAUDE_PROJECT_DIR} / …
  ✅ 项目根锁定放在 MCP server 内读进程 env，不靠 Cursor 专用变量

Claude Code：宿主自动向 MCP 子进程注入 CLAUDE_PROJECT_DIR
Cursor：WORKSPACE_FOLDER_PATHS（+ 不可靠的 roots）；勿把 ${workspaceFolder} 写进共享插件
```

---

## 收敛草案（更新）

```
启动锁定（server 内）:
  1. CLAUDE_PROJECT_DIR 为已存在绝对路径（拒 ${...}）→ 锁定
  2. roots/list 恰好 1 个可用 → 锁定
  3. WORKSPACE_FOLDER_PATHS 解析（,|;）后恰好 1 个 → 锁定
  4. 严格失败（不 cwd；多路径不取 [0]）

API / 锁 / CLI: 同前
配置: 共享 .mcp.json 保持现状风格；不引入 ${workspaceFolder}
```

验收探针：

- Cursor：`change_list` 不传参 → 工作区路径；断根时报错
- Claude Code：宿主注入的 `CLAUDE_PROJECT_DIR` 生效

---

## 诊断：为何本会话 roots 未生效（2026-07-24）

### 观测

| 探针 | 结果 |
|------|------|
| `change_list` | `"project_root": "C:\\Users\\wps"`（重复确认） |
| 实际 MCP 进程 | `node C:\Users\wps\.claude\plugins\cache\wps-ai\dev-team\2.10.0/bin/dev-team-mcp.cjs`（PID 曾为 16872） |
| 进程 cwd | `C:\Users\wps\` |
| 进程 env | **无** `CLAUDE_PROJECT_DIR`；`WORKSPACE_FOLDER_PATHS=`（空串）；非工作区路径 |
| 运行的不是仓库工作树 | cache `2.10.0`，不是 `D:\Projects\wps-claude-plugin\plugins\dev-team` |
| bundle | 含 `initProjectRootFromMcp` / `listRoots` / stderr「Failed to refresh MCP…」路径 |

### 解析链为何落到主目录

```
getProjectDir()
  ├─ MCP 缓存  → null（roots 未成功写入）
  ├─ CLAUDE_PROJECT_DIR → 未设置
  ├─ WORKSPACE_FOLDER_PATHS → "" → filter(Boolean) 后无元素
  └─ process.cwd() → C:\Users\wps\   ← change_list 所见
```

### 根因（Cursor 客户端缺口，非「没写 init」）

Cursor 在 initialize 时常**广告** `roots: { listChanged: false }`，但 `roots/list` 请求返回 JSON-RPC `-32601 Method not found`（社区长期 bug / feature gap：
[forum#77248](https://forum.cursor.com/t/mcp-client-does-not-support-roots-list/77248)）。

与我们代码的交互：

```
getClientCapabilities()?.roots  为 truthy
        → 进入 listRoots()
        → 抛错 Method not found
        → catch：stderr 一行，缓存保持 null
        → 静默回退 cwd（主目录）
```

因此：**不是「没调用 roots」**，更像是 **「capability 骗人 → listRoots 失败 → 静默兜底」**。  
`list_changed` 本会话无关（Cursor 也不发；我们亦未注册）。

补充：部分较新讨论称部分 Cursor 版本已能答 `roots/list`（甚至返回裸绝对路径）；**本会话实测仍未生效**（cwd/env/返回值三者一致指向主目录）。

### 对设计的含义

1. **不能把 Cursor 的 roots 当唯一真源**；需要 env / `.mcp.json` 注入等第二通道。
2. 启动失败应可观测：`listRoots` 失败不应只写 stderr，至少日志/回显区分「无 capability」vs「有 capability 但 Method not found」。
3. Claude Code 路径仍可靠：`CLAUDE_PROJECT_DIR` +（可选）真实可用的 roots。
4. cache `2.10.0` vs 工作树：改源码不自动进当前 MCP 进程；验收需重装/指向本地 plugin。

---

## Spike 实测（2026-07-24，完成）

### Reload 前（kill + mcp_auth，未整窗重载）

| 探针 | 结果 |
|------|------|
| 插件 cache `.mcp.json` 加 SPIKE env | **未进入进程**（配置未重读） |
| `WORKSPACE_FOLDER_PATHS` | `d:\Projects\wps-claude-plugin,d:\Projects\wpsweb\client\app`（逗号、多根） |
| `getProjectDir()` / `change_list` | 整串当 path（`split(';')` 拆不开逗号） |

### Reload Window 后

同时存在 **两个** `dev-team-mcp` 进程，env 不一致：

| | PID A | PID B（与工具结果一致） |
|--|-------|------------------------|
| `CLAUDE_PROJECT_DIR` | 字面量 `${workspaceFolder}` | `D:\Projects\wps-claude-plugin`（已展开） |
| `SPIKE_WORKSPACE_FOLDER` | 字面量 `${workspaceFolder}` | `D:\Projects\wps-claude-plugin` |
| `SPIKE_CLAUDE_PROJECT_DIR_RAW` | 字面量 `${CLAUDE_PROJECT_DIR}` | 字面量 `${CLAUDE_PROJECT_DIR}` |
| `WORKSPACE_FOLDER_PATHS` | `d:\Projects\wps-claude-plugin`（单根） | 同左 |
| cwd | `C:\Users\wps\` | 同左 |

- `change_list` → `"project_root":"D:\\Projects\\wps-claude-plugin"`（正确）
- 项目级 `.cursor/mcp.json` 探针 **未启动**（无进程、无 `%TEMP%\cursor-mcp-spike-env.json`、工具目录无 spike）

### 结论（问题 1 + 议题 4）

1. **插件 `.mcp.json` 的 `${workspaceFolder}`：Cursor 支持展开**（整窗重载后可见），可作为 Claude Code 之外的注入通道。  
   - 注意：`${CLAUDE_PROJECT_DIR}` 在 Cursor **不展开**（保持字面量）——不能指望 Claude 变量 passthrough。  
   - 风险：同会话可出现「未展开」的残留/双进程；严格模式应拒绝字面量 `${...}` 与非绝对路径。
2. **宿主已注入 `WORKSPACE_FOLDER_PATHS`**（`,` 或历史 `;`）：应作为 Cursor 主第二通道解析；禁止整串当 path。
3. **多 root**：见下节「如何区分 agent 绑定根」——`WORKSPACE_FOLDER_PATHS` 多值 ≠ 无法锁定。
4. 项目级 `.cursor/mcp.json` 探针本次未自动拉起 → 不依赖用户再配一份；优先插件 `${workspaceFolder}` → `CLAUDE_PROJECT_DIR`。

### 如何区分：窗口多根 vs agent 绑定根（文档 + 实测）

| 信号 | 语义（文档） | 是否 = agent 绑定 |
|------|--------------|-------------------|
| `WORKSPACE_FOLDER_PATHS` | Cursor 员工在论坛说明：给出 **workspace folders（复数）**；实测可为逗号拼接多路径 | **否** — 窗口内全部 folder |
| Hooks `workspace_roots: string[]` | [hooks 文档](https://cursor.com/docs/hooks.md)：「normally just one, but **multiroot can have multiple**」；**未**定义 `[0]`=绑定根 | **否（全量列表）**；生态常取 `[0]`，官方未保证 |
| `${workspaceFolder}` | [MCP 文档](https://cursor.com/docs/mcp)：**单数**「project root（含 `.cursor/mcp.json` 的文件夹）」 | **是（配置展开用的当前项目根）**；spike 展开为 `D:\Projects\wps-claude-plugin`，与本 agent 会话一致，而非多根整表 |
| `~/.cursor/projects/<slug>/` | 会话/MCP metadata 按**单路径 slug**分桶（如 `d-Projects-wps-claude-plugin` vs `d-Projects-wpsweb-client-app`） | 侧面印证：agent 侧按单项目绑定 |
| Search FAQ multi-root | [文档](https://cursor.com/docs/agent/tools/search)：多根时 **各 codebase 都可被 Agent 检索**；Cloud Agents **不支持** multi-root | 「可访问上下文」≠ MCP 锁定根；锁定仍靠单数变量 |

**推论（修正议题 4）**：

```
窗口打开多个 workspace folder
        │
        ├─ WORKSPACE_FOLDER_PATHS / workspace_roots  → 全量（可能 >1）
        │
        └─ ${workspaceFolder} → env.CLAUDE_PROJECT_DIR  → 当前绑定/当前项目（应唯一）
```

严格失败条件应是：**找不到唯一绑定根**（无可用 `CLAUDE_PROJECT_DIR`、roots 失败、且 `WORKSPACE_FOLDER_PATHS` 无法收敛到 1 个），而不是「全量列表长度 >1 就失败」。

多根时推荐锁定源顺序：

1. `CLAUDE_PROJECT_DIR`（由插件 `env: ${workspaceFolder}` 注入，真绝对路径）— **绑定根**
2. `roots/list` 若恰好 1 个可用；若多个且与 (1) 一致可采纳，否则忽略/失败
3. `WORKSPACE_FOLDER_PATHS` **仅当解析后恰好 1 个**时用作回退；**>1 时不要取 `[0]` 猜**，应依赖 (1)
4. 皆无 → 严格失败

### 收敛（更新）

```
启动锁定优先级:
  1. CLAUDE_PROJECT_DIR 为已存在绝对路径（拒绝 ${...} 字面量）→ 锁定  【Cursor 绑定根 / Claude 宿主注入】
  2. roots/list 成功且恰好 1 个可用 file 根 → 锁定
  3. WORKSPACE_FOLDER_PATHS 解析（, 或 ;）后恰好 1 个绝对路径 → 锁定
  4. 否则严格失败（不回退 cwd；多路径列表不取 [0]）

配置建议（跨宿主约束）:
  ❌ 插件 .mcp.json 禁止写 ${workspaceFolder}
     → Cursor 变量；Claude Code 启动会报错（用户实测）
  ✅ Claude Code 官方占位符仅: ${CLAUDE_PLUGIN_ROOT} / ${CLAUDE_PLUGIN_DATA} / ${CLAUDE_PROJECT_DIR}
     （见 https://code.claude.com/docs/en/mcp ）
  ✅ Claude Code：宿主会向 MCP 子进程注入 CLAUDE_PROJECT_DIR，.mcp.json 可不写
  ✅ Cursor：勿在共享 .mcp.json 写 ${workspaceFolder}；依赖
       WORKSPACE_FOLDER_PATHS（宿主注入）+ 可选 roots；
       绑定根优先用「进程内已是绝对路径的 CLAUDE_PROJECT_DIR」
       （若 Cursor 未注入则走 WORKSPACE_FOLDER_PATHS 单值）
  ⚠️ Cursor 下 ${CLAUDE_PROJECT_DIR} 在 .mcp.json 里常不展开（spike 字面量）
     → likec4 的 LIKEC4_WORKSPACE=${CLAUDE_PROJECT_DIR}/... 在 Cursor 侧本就不稳

API / 锁 / CLI: 同前
```

### 补充：`${workspaceFolder}` 与 Claude Code 不兼容（2026-07-24）

用户反馈：插件 `.mcp.json` 使用 `${workspaceFolder}` 时，**Claude Code 启动报错**。

与官方一致——Claude Code 插件 MCP 只替换：

- `${CLAUDE_PLUGIN_ROOT}`
- `${CLAUDE_PLUGIN_DATA}`
- `${CLAUDE_PROJECT_DIR}`

并写明：Claude Code **会把 `CLAUDE_PROJECT_DIR` 写入 MCP 子进程环境**（与 hooks 同源），服务端直接读 `process.env.CLAUDE_PROJECT_DIR` 即可。

因此共享插件配置**不能**靠 `${workspaceFolder}` 做双宿主兼容；锁定逻辑应放在 **server 内解析 env/roots**，而不是在 `.mcp.json` 里写 Cursor 专用变量。

临时探针已还原/删除：cache `.mcp.json`、仓库 `.cursor/mcp.json`、`openspec/explores/_spike_*.py`。

### 失败时机：为何建议 tool 入口校验（而非仅 connect 失败）

1. **MCP 仍可注册**：connect 直接 exit 时，Cursor/Claude 常显示「server failed / Not connected」，agent **拿不到结构化错误**；tool 返回 JSON/文本错误时，模型能读到原因（未锁定 / 多根歧义 / 字面量 `${...}`）。
2. **覆盖所有读写路径**：`phase_log` / `phase_next` / `backtrack` **本来就没有** `project_root` 参数，只走 `getProjectDir()` / `getChangeDir()`。仅改带参 tool 的 handler 不够；应在「取锁定根」公共入口（或 MCP handler 统一 `requireLockedProjectRoot()`）校验，避免漏网写到主目录。
3. **connect 仍做 eager 解析**：`await init` 后写入缓存，正常路径上 tool 只做同步断言（已锁定则零成本）；不是「每次 tool 重新 listRoots」。
4. **宿主怪异可诊断**：Cursor 双进程、env 字面量、`WORKSPACE_FOLDER_PATHS` 多值等，更适合在**第一次业务调用**暴露，而不是让整个 MCP namespace 起不来。
5. **CLI 不受影响**：CLI 不走 MCP connect；单测继续注入 `options.projectRoot`；严格锁定只约束 MCP 会话路径。

模式：`connect → resolve & cache（尽力）` + `每个 MCP tool handler 入口 → requireLocked()（强制）`。

### 影响范围（删 MCP input `project_root`）

#### 必须改（MCP 契约）

| 区域 | 文件/点 |
|------|---------|
| Input schemas | `change-list`、`config-get`、`archi-{query,validate,write,check}`、`test-detect-frameworks`、`test-resolve-paths` 的 `project_root` 字段 |
| `mcp.ts` | 去掉 `resolveProjectRoot(args.project_root)`；handler 改用锁定根；`phase_log`/`phase_next`/`backtrack` 同样走校验 |
| 协议级测试 | `mcp.test.ts`（多处 `arguments: { …, project_root: dir }` → 改 fixture 注入缓存/env） |
| 全局 specs | `openspec/specs/mcp-project-root/spec.md`（删除「Explicit project_root overrides」） |
| | `openspec/specs/config-get/spec.md`（input 去掉 `project_root`） |
| | `openspec/specs/test-path-resolver/spec.md`（MCP input 场景「支持 project_root 覆盖」改为锁定根） |

#### 技能 / Agent 文案

| 区域 | 结果 |
|------|------|
| `plugins/dev-team/skills/**` | **无** `project_root` 引用 |
| `plugins/dev-team/agents/**` | **无** `project_root` 引用 |

→ 无需改 skill/agent 提示词（它们本来就不传该参）。

#### 保留（非 MCP 对外契约）

| 区域 | 说明 |
|------|------|
| `runChangeList` / `runTestResolvePaths` / `runConfigGet` 等 command 的 `options.projectRoot` / `project_root` | 单测与 CLI 注入 fixture 根，**保留** |
| `*.test.ts` / `__tests__/**` 大量 `project_root: project.root` | 测 command 层，**保留**（除非改测「MCP schema 不再接受该字段」） |
| Output `project_root`（如 `change_list`） | **保留回显** |
| `test_cmd` 模板占位符 `{project_root}` | 执行期替换，与 MCP input 无关，**不动** |

#### 顺带受益（无 schema 变更）

- `phase_log` / `phase_next` / `backtrack`：无 input 字段可删，但必须纳入锁定校验，否则严格模式对它们无效。

### 下一步

- 残留 1（Cursor 多根严格失败）**已接受**
- 失败时机与影响范围已记入
- 可开 change（如 `lock-mcp-project-root`）→ `/dev-team:phase-proposal`

---

## 提案补充（2026-07-24）

- 将 `utils/constant.ts` 的 `getProjectDir` 职责**移入** `lib/project-root.ts`，删除 `constant.ts`；单测迁入 `project-root.test.ts`。
- 理由：项目根解析与 MCP 锁定/缓存同域，避免 `constant` ↔ `project-root` 双模块分叉。
