# Explore: MCP 工作区目录获取方案重设计

> 日期: 2026-07-29  
> 状态: **已收敛，可进 phase-proposal**（无活跃 change）  
> 相关归档: `openspec/changes/archive/2026-07-27-mcp-project-root-lock`、`use-mcp-roots-list`

---

## 问题背景

上一版 `mcp-project-root-lock`（connect 硬锁 + 删除 MCP `project_root` + 进程内不可变缓存）现场仍卡：

1. **Cursor 多 workspace**：多路径 → `multi_root` → 业务 tool 全废 → skill 入口起不来。
2. **工作区是 git 根下的子目录**：需明确根 = 宿主工作区（不向上找 git/openspec）。
3. **兼容 Claude Code / Cursor**：共享 `.mcp.json` 不能写 `${workspaceFolder}`；回灌未展开的 `CLAUDE_PROJECT_DIR` 有害。

探索中否决：专用 `resolve`/`set`（多 workspace 进程污染）、M-hint（可选参 + 唯一候选默认，分支少但与「∈ 候选 + 二次确认」目标不符）。

---

## 最终方案（权威）

### 决策一览

| # | 决策 | 说明 |
|---|------|------|
| D1 | **取消 lock** | 无 `requireLocked`、无不可变锁定缓存 |
| D2 | **connect 只采候选** | 合并去重宿主通道；**不区分** `len==1` / `>1`，不做默认根 |
| D3 | **根语义 = 宿主工作区原样** | 不向上找 git / `openspec/` |
| D4 | **MCP 禁止 `process.cwd()`** | CLI/单测另议（proposal 定） |
| D5 | **`project_root` 必填** | MCP input schema `required`；skill 不写锁/resolve 状态机，按 schema 填参 |
| D6 | **与候选校验** | `P ∈ candidates` 才直接放行；否则报错（附 candidates） |
| D7 | **无 force 字段；同全参再调 = force** | 将 `project_root` 加入 candidates 并放行；报错文案说明机制 |
| D8 | **多通道合并去重** | `CLAUDE_PROJECT_DIR` ∪ `WORKSPACE_FOLDER_PATHS` ∪ `roots/list` |
| D9 | **共享 `.mcp.json`** | 不写 `${workspaceFolder}`；不回灌未展开的 `CLAUDE_PROJECT_DIR` |
| S1 | force 比较范围 | **tool 名 + 完整 arguments** 全同 |
| S2 | 必填落地 | schema 即可，无 skill 特判 |
| S3 | 「skill 无感」含义 | 不写根状态机；**不是**可省略 `project_root` |
| S4 | 空候选 | **force 逃生**：`[]` 时第一次必拒，同全参再调加入候选 |
| S5 | pending 键 | tool + 稳定序列化 arguments；仅保留最近一次；成功/force 后清除 |
| S6 | 校验顺序 | 路径合法 → ∈ 候选 → pending force → 否则写 pending 报错 |

### 行为

```
connect → candidates = collect(CLAUDE ∪ WORKSPACE ∪ roots)  // 只采集

tool(arguments incl. project_root: P)   // schema 必填
  P 合法（绝对 + exists + 非 ${...}）?  否 → invalid
  P ∈ candidates?                      是 → 清 pending，执行
  pendingKey == f(tool, arguments)?    是 → candidates.add(P)，清 pending，执行（force）
  否则 → pending = f(tool, arguments)，报错
         （附 candidates；说明再提交相同完整 arguments 即 force 并加入候选）
```

成功时可只读回显本次 `project_root`。

### 数据来源

| 通道 | 角色 |
|------|------|
| `CLAUDE_PROJECT_DIR` | 可用则进入候选（拒绝相对 / 不存在 / `${...}`） |
| `WORKSPACE_FOLDER_PATHS` | `,`/`;` 分割后全部可用路径进入候选 |
| `roots/list` | 可用 file 根并入；失败不阻断其它通道 |
| ~~`process.cwd()`~~ | MCP **禁用** |

### 场景

| 场景 | 行为 |
|------|------|
| 多 workspace | 候选含多路径；agent 传 ∈ 列表的 `project_root` 一次过；传列表外则二次全参确认 |
| git 子目录工作区 | 根 = 宿主打开的 folder；不 walk |
| 单根 Claude/Cursor | 仍须 schema 必填传入该路径（通常等于唯一候选）；**无**省参默认 |
| 宿主候选全空 | S4 逃生：合法绝对路径 + 两次相同全参 |

### 相对归档版 `mcp-project-root-lock`

| | 归档版 | 本方案 |
|--|--------|--------|
| MCP `project_root` | 删除 | **必填** |
| connect | 尝试锁唯一根 | **只填 candidates** |
| 多根 | 进程级失败 | 本调用报错 + 带参/force 重试 |
| 缓存 | 不可变 lock | candidates（可 force 追加）+ 一个 pending |
| cwd | MCP 禁止 | 同左 |

### 否决方向

启动硬锁、删除 MCP `project_root`、专用 resolve/set、静默 cwd、多根取 `[0]`、可选参 + `len==1` 自动默认（M-hint）、独立 `force` 布尔字段。

---

## 与现状实现对照（便于改代码）

现状（`lib/project-root.ts` + lock）：connect 互斥通道锁定 → `requireLocked`；无候选一等概念；MCP 无 `project_root`。

本方案替换为：connect 只 `collect` → 每 tool 用必填 `project_root` + ∈/pending 状态机；删除 lock API（CLI `getProjectDir` 是否保留 cwd 回退 → proposal）。

工作树曾 staged 的 `.mcp.json` 去回灌、`project-root` 调通道序等：**勿当终态**；以本节为准重做。

---

## 误用面（已知、接受）

- force 后路径留在 candidates → 允许列表膨胀（弱于「set 当前根」污染）。
- agent 未读提示原样重放整包 arguments → 会 force；靠文案约束。
- 空候选两次调用可写入任意存在路径 → 刻意逃生口。

---

## 提案前仅余实现级细节（不阻塞方向）

以下不必再 explore 拍板产品语义，留给 design/tasks 即可：

1. **路径相等**：∈ 与 pending 比较前是否 `normalize`（尾斜杠、盘符大小写、`realpath`）——建议 design 定一条，避免 Windows 误拒。
2. **CLI `getProjectDir`**：是否仍允许 cwd；与 MCP 契约分离即可。
3. **错误载荷形状**：纯 text vs 结构化（`code` / `candidates` / `force_hint`）；agent 可读即可。
4. **roots 超时**：connect 采集时是否保留 timeout，防止挂死。
5. **哪些 tool 必填**：所有触达项目树的 MCP tool（含 `phase_*` / `backtrack`）；与「schema 必填」一致列清单。

---

## 探索演变（摘要，非权威）

曾讨论：可选 `project_root`、M-hint、专用 resolve/set、独立 force 字段。均已由「必填 + ∈ 候选 + 全参再调 force」取代。旧草稿段落已删除，避免与最终方案矛盾。

---

## 下一步

1. `/dev-team:phase-proposal`（本文件可 promote 为 `openspec/changes/<name>/explore.md`）。
2. AC 至少覆盖：connect 只采候选、必填 `project_root`、∈ 放行、∉ 报错含候选与 force 说明、全参再调 force 并加入候选、空候选逃生、pending 全参键、MCP 无 cwd。
