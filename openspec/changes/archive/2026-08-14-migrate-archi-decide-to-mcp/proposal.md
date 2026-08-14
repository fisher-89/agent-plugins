# 提案: migrate-archi-decide-to-mcp

> **变更**: migrate-archi-decide-to-mcp
> **日期**: 2026-08-12
> **状态**: 起草中

---

## 问题

`archi-model.py` / `archi-validate.py` 已在既有变更中迁入 MCP（`archi_query` / `archi_validate` / `archi_write` / `archi_check`），但 ADR 管理仍停留在 `plugins/dev-team/utils/archi-decide.py`，成为插件源码中最后的 Python 孤岛。

由此产生断层：

1. **Agent 表面不一致**：`architecture` agent 的 propose / validate 已走 MCP，decide 仍要求 `python .../archi-decide.py`，需 Bash、Python 运行时与 shell 参数转义。
2. **结构化参数不适合 shell**：`create` 需传入多字段与 JSON `alternatives`，Bash 引号/转义易失败；MCP 结构化入参更稳。
3. **与 CLI 职责冲突**：`cli.ts` 面向门禁与长跑（`run_static_analysis`、`test-execution`），不是架构短事务；同类 `archi_*` 已只挂在 `mcp.ts`。
4. **模板债**：脚本声明 `TEMPLATE_PATH → templates/adr.md` 但实际硬编码拼装内容，与模板章节细节略不一致。
5. **产品契约仍依赖该能力**：`architecture-decisions` 与 agent Decide mode 已写入正式 spec；单纯删除脚本会与契约冲突。此前 isolate-test-and-code-gen 的 D12 仅延后迁移，并非取消能力。
6. **残留引用**：`architecture.md` 工具清单与 Decide 步骤、`hooks.test.ts` PowerShell 豁免 fixture 仍指向 `archi-decide.py`；`openspec/todo.md` 亦将「下线 python 脚本」列为待办。

---

## 提案

保留 ADR create / list / update 能力，将其从 Python 脚本迁到 MCP，并下线 `archi-decide.py`。不新增 CLI 入口。

### 方案要点

1. **单工具 `archi_decide` + `action`**：`action` 为 `create` | `list` | `update`，对应原脚本三子命令；避免拆成三个 tool 增加 `tools/list` 噪音。
2. **实现落点**：`plugins/dev-team/bin/src/lib/archi-decide.ts` + Zod schema；在 `mcp.ts` 注册，经 `withResolvedProjectRoot` 解析 `project_root`（与其它 `archi_*` 一致）。
3. **行为等价为主**：文件名 `YYYY-MM-DD-<kebab-title>.md`、状态枚举、`superseded` 需 `superseded_by`、按状态过滤与按日期降序列表等行为 SHALL 与现有 `architecture-decisions` 契约一致。
4. **吃模板**：创建 ADR 时基于 `plugins/dev-team/templates/adr.md`（或安装后的等价路径）填充字段，消除「模板存在但未用」。
5. **Agent 对齐**：`architecture.md` Decide mode 改为调用 MCP `archi_decide`，描述与步骤中不再引用 Python 脚本。
6. **下线脚本与产物引用**：删除源码 `archi-decide.py`，并确保构建产物 / `manifest.json` / 测试 fixture 不再依赖该路径；`hooks.test.ts` 中以其它非 ADR 命令替换 PowerShell 豁免用例的 python fixture。
7. **不改 CLI**：`cli.ts` 不增加 `archi decide` 子命令。

### 方案对比（已定）

| 方案 | 结论 |
|------|------|
| A. 保留能力 → MCP（单工具 + action） | **采用** — 与 `archi_*` 表面一致，参数结构化，改动面可控 |
| B. 保留能力 → 仅 CLI | 否 — 与现有 archi MCP 化逆向，长参数体验差 |
| C. lib 共享 + CLI/MCP 双入口 | 否 — ADR 低频，双入口维护过重 |
| D. 连能力一起砍 | 否 — 破坏已归档正式契约，迁移成本反而更低 |

---

## 能力

### 新增能力

（无）

### 修改的能力

- `architecture-decisions` — ADR create/list/update 的权威交付面改为 MCP `archi_decide`；下线 Python 脚本；创建时使用 `templates/adr.md`
- `architecture-agent` — Decide mode 改为通过 MCP `archi_decide` 起草/写入 ADR，不再调用 `archi-decide.py`
- `mcp-project-root` — 将 `archi_decide` 纳入须带 `project_root` 并按 candidates/force 解析的 MCP 工具清单

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/archi-decide.ts` — ADR create/list/update 逻辑（新建）
- `plugins/dev-team/bin/src/schemas/archi-decide.schema.ts` — Zod input/output（新建）
- `plugins/dev-team/bin/src/schemas/index.ts` — 导出新 schema
- `plugins/dev-team/bin/src/mcp.ts` — 注册 `archi_decide`
- `plugins/dev-team/agents/architecture.md` — Decide mode 与工具列表改为 MCP
- `plugins/dev-team/utils/archi-decide.py` — 删除
- `plugins/dev-team/templates/adr.md` — 作为 create 渲染源（内容可小幅对齐占位符，若实现需要）
- 构建产物与清单：`claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/` 中对应 utils/agent/manifest 引用（经 `pnpm -C plugins/dev-team run build` 刷新）
- `plugins/dev-team/package.json` — 按项目规则 bump 版本（源码变更后）
- `CLAUDE.md` — 若仍声称「Python utilities … ADRs」，同步为 MCP 表述（文档对齐，非行为变更）

### 测试文件

- `plugins/dev-team/bin/src/lib/archi-decide.test.ts` — create/list/update、状态校验、模板渲染、错误路径（新建）
- `plugins/dev-team/bin/src/mcp.test.ts` — 注册名、input schema、`project_root` 解析、调用到 lib 的集成断言
- `plugins/dev-team/bin/src/hooks.test.ts` — 替换依赖 `archi-decide.py` 的 fixture 命令
- 相关 workspace-root / MCP 工具列表测试中补充 `archi_decide`（若现有表枚举全部 archi 工具）

### 不要修改

- `cli.ts` 及现有 CLI 子命令（不新增 `archi decide`）
- ADR 目录约定 `openspec/architecture/decisions/` 与状态枚举语义（除非发现与模板对齐的纯格式修正）
- 其它已有 MCP 工具（`archi_query` / `validate` / `write` / `check` 等）的行为契约
- `openspec-cli.sh` 与非 ADR 的 workflow/phase 工具
- 不引入 scope 与 `archi_query` 的硬校验联动（可作为后续增强，本次不做）
- 不保证 create 输出与旧 Python 脚本字节级一致（以模板与正式字段契约为准）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-01 | MCP `archi_decide` 注册 | `tools/list` 含 `archi_decide`；命名为 `xx_yy`，无斜杠 |
| AC-02 | create | `action=create` 在 `openspec/architecture/decisions/` 写入 `YYYY-MM-DD-<slug>.md`，含 title/date/status/background/decision/consequences/alternatives/影响范围 |
| AC-03 | 默认状态 | create 未传 status 时 status 为 `proposed` |
| AC-04 | list | `action=list` 返回按日期降序的 ADR 列表；可选 status 过滤生效 |
| AC-05 | update | `action=update` 可改状态；`superseded` 无 `superseded_by` 时失败 |
| AC-06 | project_root | `archi_decide` 要求 `project_root`，走与其它 MCP 工具相同的 candidates/force 解析 |
| AC-07 | 模板 | create 输出结构与 `templates/adr.md` 章节对齐（非旧脚本硬编码漂移） |
| AC-08 | Agent | `architecture.md` Decide 步骤仅引用 MCP `archi_decide`，无 `archi-decide.py` |
| AC-09 | 下线 Python | 源码与构建产物中不存在 `utils/archi-decide.py`；manifest 无该条目 |
| AC-10 | 测试 | `archi-decide` 单测与 `mcp.test.ts` 覆盖通过；`hooks.test.ts` 不再依赖已删脚本路径 |
| AC-11 | 无 CLI | `dev-team` CLI help / 路由中不出现 archi-decide 子命令 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 模板渲染与旧脚本输出字节级不一致 | 既有手工 ADR 或外部文档比对失败 | 中 | 以正式 `architecture-decisions` 字段契约与模板为准；不保证与旧 py 字节级一致 |
| Agent/构建产物未同步仍提 python 路径 | Decide 模式调用失败 | 中 | 源码改完后强制 rebuild；grep 构建产物确认无 `archi-decide.py` |
| hooks 测试仍引用已删脚本 | CI 失败 | 低 | 同步改 `hooks.test.ts` fixture |
| 单工具 action 分发参数校验复杂 | create/list/update 字段互相干扰 | 中 | Zod 按 action 做判别联合或 handler 内校验；单测覆盖缺参/非法 status |
| 模板路径在安装态如何解析 | create 找不到模板 | 中 | 实现时优先插件根/`__DEV_TEAM_ROOT__` 约定，失败时返回明确错误且不写半文件；design 阶段敲定 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 是否保留 ADR 工具能力 | 保留并迁移 | 正式 spec 已承诺；迁移成本低 | 砍能力并改/删 spec |
| 入口形态 | 仅 MCP | 与现有 `archi_*` 一致；CLI 职责不匹配 | 仅 CLI；CLI+MCP 双入口 |
| 工具粒度 | 单工具 `archi_decide` + `action` | 对应原三子命令；listTools 更干净 | `archi_decide_create` / `_list` / `_update` |
| 是否使用 `templates/adr.md` | 使用 | 消除模板未用技术债 | 继续硬编码字符串 |
| 是否联动 `archi_query` 校验 scope | 否（本次） | 超出迁移最小范围 | 后续增强 |

### 待决问题

- create 时模板文件的运行时解析根（源码树 vs 安装后 `templates/`）在 design 阶段最终敲定
- Zod 判别联合 vs handler 内按 action 校验的具体写法（实现细节，不阻塞本提案）
