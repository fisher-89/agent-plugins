# 任务: migrate-archi-decide-to-mcp



> **变更**: migrate-archi-decide-to-mcp

> **日期**: 2026-08-12



---



## 阶段 1: Schema 契约



- [x] 新建 `plugins/dev-team/bin/src/schemas/archi-decide.schema.ts`：定义 `AdrStatus` 枚举与 `ArchiDecideAlternative` 形状

- [x] 实现 `archiDecideInputSchema` 为 `z.discriminatedUnion('action', …)`，三支均含必填 `project_root: projectRootSchema`

- [x] create 支：必填 `title` / `background` / `decision`；可选 `consequences`、`alternatives`、`scope`（`string[]`）、`status`（默认语义在 lib，schema 可选）

- [x] list 支：可选 `status` 过滤

- [x] update 支：必填 `file`、`status`；可选 `superseded_by`

- [x] 实现按 `action` 判别的 `archiDecideOutputSchema`（create / list / update 结果结构）

- [x] 导出供 lib 使用的 input/result 类型（`z.infer` / `z.output`）

- [x] 在 `plugins/dev-team/bin/src/schemas/index.ts` 导出新 schema 与必要类型



## 阶段 2: ADR lib（create / list / update）



- [x] 新建 `plugins/dev-team/bin/src/lib/archi-decide.ts`，常量 `openspec/architecture/decisions`

- [x] 实现插件根探测：自模块目录向上查找存在 `templates/adr.md` 的目录；找不到时 create 返回明确 `error` 且不写文件

- [x] 实现 `createAdr(projectRoot, input)`：校验 status；slugify 文件名 `YYYY-MM-DD-<kebab>.md`；已存在则失败；基于模板渲染并写入 UTF-8

- [x] create 未传 `status` 时写入 `proposed`

- [x] 渲染结果须含模板章节：`## 背景`、`## 决策`、`## 后果`、`## 备选方案`、`## 影响范围`，以及 title/date/status/alternatives/scope

- [x] 实现 `listAdrs(projectRoot, status?)`：解析 title/status/date/scope；按日期降序；目录缺失返回空列表

- [x] 实现 `updateAdr(projectRoot, input)`：合法 status；`superseded` 无 `superseded_by` 失败且不写盘；更新 `**状态**` 与可选 `**取代者**`



## 阶段 3: MCP 注册



- [x] 在 `plugins/dev-team/bin/src/mcp.ts` 导入 `archiDecideInputSchema` / `archiDecideOutputSchema`

- [x] 在 `MCP_TOOLS` 增加 `name: 'archi_decide'`（下划线，无斜杠）与简短 description

- [x] handler 使用 `withResolvedProjectRoot('archi_decide', args, …)`，将 resolve 后的根传入 lib

- [x] 按 `args.action` 分发 `createAdr` / `listAdrs` / `updateAdr`，经 `jsonContent` 返回

- [x] 确认未改动 `cli.ts`，无 `archi decide` / `archi-decide` CLI 子命令



## 阶段 4: Agent 与文档



- [x] 更新 `plugins/dev-team/agents/architecture.md` frontmatter description：decide 改为 MCP `archi_decide`，去掉 `archi-decide.py`

- [x] 工具清单：删除 Python `archi-decide.py` 条目；在 MCP tools 列表加入 `archi_decide`（create/list/update）

- [x] 重写 DECIDE mode 步骤：仅描述调用 MCP `archi_decide`（含 `project_root` 与各 action 参数），无任何 `python …/archi-decide.py` 命令

- [x] 更新根目录 `CLAUDE.md`：去掉「Python utilities … ADRs」表述，改为架构 ADR 经 MCP `archi_decide`（或等价无 Python 表述）



## 阶段 5: 下线 Python 与构建产物



- [x] 删除源码 `plugins/dev-team/utils/archi-decide.py`

- [x] 若 `templates/adr.md` 需为渲染对齐做小幅占位符调整则修改；否则保持章节不变

- [x] bump `plugins/dev-team/package.json` 的 `version`（patch）

- [x] 运行 `pnpm -C plugins/dev-team run build`，刷新 `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`

- [x] 确认三端产物中不存在 `utils/archi-decide.py`，且 `cursor-home-image/dev-team/manifest.json`（及同类清单）不再列出该路径

- [x] 确认构建后的 architecture agent 文案不再引用 `archi-decide.py`

