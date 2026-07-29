# 提案: mcp-workspace-root

> **变更**: mcp-workspace-root
> **日期**: 2026-07-29
> **状态**: 草稿

---

## 问题

上一版 `mcp-project-root-lock`（connect 硬锁唯一根 + 删除 MCP `project_root` + 进程内不可变缓存）现场仍卡：

1. **Cursor 多 workspace**：多路径被判定为 `multi_root` → 业务 tool 全部失败 → skill 入口起不来。
2. **工作区是 git 根下的子目录**：需要根 = 宿主打开的 folder 原样，不得向上 walk git / `openspec/`。
3. **跨宿主配置约束**：共享 `.mcp.json` 不能写 `${workspaceFolder}`；回灌未展开的 `CLAUDE_PROJECT_DIR` 有害。

硬锁模型把「宿主候选」与「本次调用选用的根」混为一谈；多根时进程级失败，无法通过带参调用继续工作。

---

## 提案

取消启动硬锁，改为 **connect 只采集候选** + **每次 MCP tool 调用必填 `project_root`** + **∈ 候选放行 / 同全参再调 force**。

### 行为模型

```
connect → candidates = collect(CLAUDE ∪ WORKSPACE ∪ roots)  // 只采集，不做默认根

tool(arguments incl. project_root: P)   // schema 必填
  P 合法（绝对 + exists + 非 ${...}）?  否 → invalid
  P ∈ candidates?                      是 → 清 pending，执行
  pendingKey == f(tool, arguments)?    是 → candidates.add(P)，清 pending，执行（force）
  否则 → pending = f(tool, arguments)，报错
         （附 candidates；说明再提交相同完整 arguments 即 force 并加入候选）
```

### 关键决策

| # | 决策 | 说明 |
|---|------|------|
| D1 | 取消 lock | 无 `requireLocked`、无不可变锁定缓存 |
| D2 | connect 只采候选 | 合并去重宿主通道；**不区分** `len==1` / `>1`，不做默认根 |
| D3 | 根语义 = 宿主工作区原样 | 不向上找 git / `openspec/` |
| D4 | MCP 禁止 `process.cwd()` | CLI `getProjectDir` 可保留 cwd 回退（与 MCP 契约分离） |
| D5 | `project_root` 必填 | MCP input schema `required`；skill 不写锁/resolve 状态机，按 schema 填参 |
| D6 | 与候选校验 | `P ∈ candidates` 才直接放行；否则进入 pending/force |
| D7 | 无独立 force 字段 | 同 tool 名 + 完整 arguments 再调 = force；将 `P` 加入 candidates |
| D8 | 多通道合并去重 | `CLAUDE_PROJECT_DIR` ∪ `WORKSPACE_FOLDER_PATHS` ∪ `roots/list` |
| D9 | 共享 `.mcp.json` | 不写 `${workspaceFolder}`；不回灌未展开的 `CLAUDE_PROJECT_DIR` |

### 数据来源

| 通道 | 角色 |
|------|------|
| `CLAUDE_PROJECT_DIR` | 可用则进入候选（拒绝相对 / 不存在 / `${...}`） |
| `WORKSPACE_FOLDER_PATHS` | `,`/`;` 分割后全部可用路径进入候选 |
| `roots/list` | 全部可用 file 根并入；失败不阻断其它通道 |
| `process.cwd()` | MCP **禁用** |

### API 契约

| 层面 | 行为 |
|------|------|
| MCP input schema | 所有触达项目树的 tool **必填** `project_root`（含 `phase_log` / `phase_next` / `backtrack` / `change_list` / `config_get` / `archi_*` / `test_*`） |
| 校验 | 路径合法 → ∈ 候选 → pending force → 否则写 pending 并报错 |
| MCP output | 可只读回显本次 `project_root` |
| CLI / command `options.projectRoot` | **保留**（单测与 CLI fixture；与 MCP 对外契约分离） |
| `list_changed` | **不实现** |
| 空候选 | force 逃生：第一次必拒，同全参再调将合法绝对路径加入候选并放行 |

### 实现要点

1. 重写 `lib/project-root.ts`：`collectCandidates`（connect）+ `resolveProjectRootForTool(toolName, arguments)`（∈ / pending / force）；删除 `requireLockedProjectRoot` / 不可变 lock API。
2. `mcp.ts`：connect 后只采集候选；各 tool handler 用必填 `args.project_root` 经 resolve 入口取得根。
3. 各 MCP input Zod schema 增加必填 `project_root: z.string()`（含原先无该字段的 `phase_*` / `backtrack`）。
4. 更新协议级测试；CLI/command fixture 行为保持。
5. 按项目规则升级插件版本并重建双端产物。

路径相等比较（normalize / 盘符大小写 / realpath）、错误载荷形状、roots 超时等实现细节留给 design。

---

## 能力

### 新增能力

- （无）

### 修改的能力

- **mcp-project-root** — 取消启动硬锁；connect 合并采集候选；MCP tool 必填 `project_root`；∈ 候选放行；同全参再调 force；空候选逃生；MCP 禁用 cwd；根 = 宿主工作区原样
- **config-get** — MCP `config_get` input 恢复必填 `project_root`；改用候选/force 解析根，不再依赖 lock
- **test-path-resolver** — MCP `test_resolve_paths` input 恢复必填 `project_root`；改用候选/force 解析根，不再依赖 lock

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/project-root.ts` — 候选采集、pending/force、删除 lock API；CLI `getProjectDir` 可保留 cwd 回退
- `plugins/dev-team/bin/src/mcp.ts` — connect 只 collect；tool handler 走必填 `project_root` resolve
- `plugins/dev-team/bin/src/schemas/change-list.schema.ts` — MCP input 增加必填 `project_root`
- `plugins/dev-team/bin/src/schemas/config-get.schema.ts` — 同上
- `plugins/dev-team/bin/src/schemas/archi-query.schema.ts` — 同上
- `plugins/dev-team/bin/src/schemas/archi-validate.schema.ts` — 同上
- `plugins/dev-team/bin/src/schemas/archi-write.schema.ts` — 同上
- `plugins/dev-team/bin/src/schemas/archi-check.schema.ts` — 同上
- `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` — 同上
- `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` — 同上
- `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` — 增加必填 `project_root`
- `plugins/dev-team/bin/src/schemas/phase-next.schema.ts` — 增加必填 `project_root`
- `plugins/dev-team/bin/src/schemas/backtrack.schema.ts` — 增加必填 `project_root`
- `plugins/dev-team/.mcp.json` — 保持无 `${workspaceFolder}` / 无未展开 `CLAUDE_PROJECT_DIR` 回灌（若工作树有偏差则按本方案纠正）
- 插件版本字段（`plugins/dev-team/package.json` 及/或对应 plugin manifest）— 升级版本
- 重建双端产物（`node scripts/build-plugins.mjs`）

### 测试文件

- `plugins/dev-team/bin/src/lib/project-root.test.ts` — 候选合并去重、合法/非法路径、∈ 放行、pending/force、空候选逃生、无 cwd
- `plugins/dev-team/bin/src/mcp.test.ts` — schema 必填 `project_root`；多候选 / 候选外 force；未传参失败
- `plugins/dev-team/bin/__tests__/mcp-project-root-lock/` — 按新语义重写或替换（名称可随实现调整）
- 依赖 `requireLockedProjectRoot` mock 的相关测试（如 `hooks.test.ts` 中 MCP 锁缓存用例）— 按新 API 调整

### 不要修改

- 共享 `.mcp.json` 中写入 `${workspaceFolder}` 或未展开的 `CLAUDE_PROJECT_DIR`
- CLI / command 层 `options.projectRoot` / `project_root` fixture 注入能力（保留）
- `test_cmd` 模板占位符 `{project_root}`（执行期替换，与 MCP input 无关）
- 专用 `resolve` / `set` MCP tool、独立 `force` 布尔字段、可选参 + `len==1` 自动默认（M-hint）
- 向上 walk git / `openspec/` 推断根
- LikeC4 MCP 与其它非 `dev-team` 插件
- 实现真正的 `notifications/roots/list_changed` 热刷新
- skill 内嵌根状态机（仅按 schema 填必填参）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | connect 只采候选 | connect 后 candidates = CLAUDE ∪ WORKSPACE ∪ roots 去重集合；`len==1` 时亦不自动锁定为默认根；无 `requireLocked` |
| AC-2 | MCP 必填 `project_root` | 触达项目树的全部 MCP tool 的 input schema 均含 required `project_root`；省略时校验失败 |
| AC-3 | ∈ 候选放行 | 传入合法且 ∈ candidates 的 `project_root` 时 tool 成功执行；成功后可清 pending |
| AC-4 | ∉ 候选报错 | 合法但 ∉ candidates 时返回错误，载荷含 candidates 列表与「同全参再调即 force」说明；不执行业务 I/O |
| AC-5 | 全参再调 force | 同一 tool 名 + 完整 arguments 与 pending 键相同再调时，将 `project_root` 加入 candidates 并放行执行 |
| AC-6 | 空候选逃生 | candidates 为空时第一次合法调用必拒；同全参再调加入候选并放行 |
| AC-7 | pending 键 | pending 键 = tool 名 + 稳定序列化完整 arguments；仅保留最近一次；成功或 force 后清除 |
| AC-8 | MCP 无 cwd | MCP resolve 路径不得使用 `process.cwd()`；禁用静默 cwd 回退 |
| AC-9 | 根语义原样 | 不因存在上层 `.git` / `openspec/` 而改写传入的宿主工作区路径 |
| AC-10 | 配置约束 | 共享 `.mcp.json` 不含 `${workspaceFolder}`，也不回灌未展开的 `CLAUDE_PROJECT_DIR` |
| AC-11 | CLI 分离 | CLI/command 仍可用显式 `projectRoot` / `getProjectDir`（可含 cwd）；不得把 MCP 契约绑死到 cwd |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| force 后路径留在 candidates，允许列表膨胀 | 进程内可接受更多根，弱于「set 当前根」污染但仍累积 | 中 | 文案提示；接受为已知误用面；进程重启清空 |
| agent 未读提示原样重放整包 arguments → 意外 force | 可能写入非预期根 | 中 | 报错文案明确说明机制；pending 要求完整 arguments 全同 |
| 空候选两次调用可写入任意存在路径 | 逃生口被滥用 | 低 | 刻意保留；依赖「两次相同全参」门槛 |
| Windows 路径比较不一致（尾斜杠/盘符大小写）导致误拒 | 合法路径被当成 ∉ 候选 | 中 | design 定一条 normalize 规则；测试覆盖 |
| 旧 lock 测试 / mock 大面积失效 | 实现阶段返工 | 高 | 提案范围已列测试文件；按新 API 重写 |
| skill 仍按「无参调用」习惯省略 `project_root` | tool 全失败 | 中 | schema 必填 + skill 按 schema 填参（S2/S3）；不写状态机 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 多 workspace 如何工作？ | 候选含多路径；agent 传 ∈ 列表的路径一次过 | 避免进程级失败；不猜 `[0]` | 硬锁失败 / 取 `[0]` |
| 是否删除 MCP `project_root`？ | 否，改为必填 | 调用方显式选型；兼容多根 | 继续删除 + lock |
| 如何确认候选外路径？ | 同全参再调 = force，无独立 force 字段 | 少一个参数；复用重试语义 | 独立 `force: true` / 专用 set tool |
| 单根是否可省略参数？ | 否 | schema 必填；避免隐式默认 | M-hint（可选 + len==1 默认） |
| CLI 是否禁用 cwd？ | 否，仅 MCP 禁用 | CLI/单测需要 fixture 与本地回退 | CLI 同样严格失败 |
| 根是否 walk 到 git 根？ | 否，宿主工作区原样 | 子目录工作区是真实场景 | 向上找 `.git` / `openspec/` |

### 待决问题

- 路径相等：∈ 与 pending 比较前的 normalize 规则（尾斜杠、盘符大小写、是否 `realpath`）— design 定
- 错误载荷形状（纯 text vs 结构化 `code` / `candidates` / `force_hint`）— design 定，agent 可读即可
- roots 采集是否保留 timeout — design 定
- `getProjectDir` 在无 cache 时的具体回退顺序是否微调 — 与 MCP 契约分离即可，design/tasks 定
