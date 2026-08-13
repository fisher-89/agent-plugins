# 测试设计: migrate-archi-decide-to-mcp

> **日期**: 2026-08-12

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-01 | `tools/list` 含 `archi_decide`；命名为 `xx_yy`，无斜杠 | 单元测试 / 集成测试 | `plugins/dev-team/bin/src/mcp.test.ts`；`mcp.ts` → `lib/archi-decide.ts` |
| AC-02 | `action=create` 在 `openspec/architecture/decisions/` 写入 `YYYY-MM-DD-<slug>.md`，含 title/date/status/background/decision/consequences/alternatives/影响范围 | 单元测试 / 集成测试 | `plugins/dev-team/bin/src/lib/archi-decide.test.ts`；create → 模板 → decisions |
| AC-03 | create 未传 status 时 status 为 `proposed` | 单元测试 | `plugins/dev-team/bin/src/lib/archi-decide.test.ts` |
| AC-04 | `action=list` 返回按日期降序的 ADR 列表；可选 status 过滤生效 | 单元测试 | `plugins/dev-team/bin/src/lib/archi-decide.test.ts` |
| AC-05 | `action=update` 可改状态；`superseded` 无 `superseded_by` 时失败 | 单元测试 / 集成测试 | `plugins/dev-team/bin/src/lib/archi-decide.test.ts`；`mcp.ts` → lib |
| AC-06 | `archi_decide` 要求 `project_root`，走与其它 MCP 工具相同的 candidates/force 解析 | 单元测试 / 集成测试 | `plugins/dev-team/bin/src/mcp.test.ts`；`mcp.ts` → `project-root` → lib |
| AC-07 | create 输出结构与 `templates/adr.md` 章节对齐 | 单元测试 / 集成测试 | `plugins/dev-team/bin/src/lib/archi-decide.test.ts`；create → 模板 → decisions |
| AC-08 | `architecture.md` Decide 步骤仅引用 MCP `archi_decide`，无 `archi-decide.py` | 集成测试 | `agents/architecture.md` ↔ 文案契约 |
| AC-09 | 源码与构建产物中不存在 `utils/archi-decide.py`；manifest 无该条目 | 集成测试 | 源码树 / build 产物 / manifest |
| AC-10 | `archi-decide` 单测与 `mcp.test.ts` 覆盖通过；`hooks.test.ts` 不再依赖已删脚本路径 | 单元测试 | `archi-decide.test.ts`；`mcp.test.ts`；`hooks.test.ts` |
| AC-11 | `dev-team` CLI help / 路由中不出现 archi-decide 子命令 | 单元测试 | `plugins/dev-team/bin/src/cli.test.ts` |

框架：`vite-plus`（`vp test`，断言库为 vitest API：`describe` / `it` / `expect` / `vi`）。集成测试置于 `plugins/dev-team/bin/__tests__/`。

---

## 单元测试

### `plugins/dev-team/bin/src/lib/archi-decide.ts` -> `plugins/dev-team/bin/src/lib/archi-decide.test.ts`

#### 待测功能

- `createAdr(projectRoot: string, input: ArchiDecideCreateInput): ArchiDecideCreateResult`: 自插件根解析 `templates/adr.md` 并渲染；写入 `openspec/architecture/decisions/YYYY-MM-DD-<kebab>.md`；默认 `status: 'proposed'`；模板缺失 / 非法 status / 文件已存在 → `{ success: false, error }` 且不写半文件
- `listAdrs(projectRoot: string, status?: AdrStatus): ArchiDecideListResult`: 目录不存在返回空列表；按日期降序；可选 status 过滤；解析 title/status/date/scope/path
- `updateAdr(projectRoot: string, input: ArchiDecideUpdateInput): ArchiDecideUpdateResult`: 更新 `**状态**`；`superseded` 无 `superseded_by` 失败且不写盘；成功时可写入 `**取代者**`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `createAdr — 写入与字段 (AC-02)` | 正向 | 合法 title/background/decision 时成功写入 `YYYY-MM-DD-<kebab>.md`，返回 `success: true` 与 `filename`/`path`，文件含标题、日期、状态、背景、决策、后果、备选方案、`## 影响范围` | 新增 |
| `createAdr — 默认状态 (AC-03)` | 正向 | 省略 `status` 时文件 `**状态**` 为 `proposed` | 新增 |
| `createAdr — 显式状态` | 正向 | `status` 分别为 `proposed` / `accepted` / `deprecated` / `superseded` 时写入对应状态字面量 | 新增 |
| `createAdr — 模板章节 (AC-07)` | 正向 | 产出含 `templates/adr.md` 章节标题：`## 背景`、`## 决策`、`## 后果`、`### 正面后果`、`### 负面后果`、`## 备选方案`、`## 影响范围` | 新增 |
| `createAdr — alternatives / scope` | 正向 | `alternatives` 含 name/description/pros/cons（string 与 string[]）时渲染为 `### 方案 N：{name}`；`scope` 渲染为 `- item` 列表 | 新增 |
| `createAdr — 非法 status` | 异常 | `status` 为非法枚举（如 `draft`）→ `success: false`，decisions 目录无新文件 | 新增 |
| `createAdr — 文件已存在` | 异常 | 目标 `YYYY-MM-DD-slug.md` 已存在 → `success: false`，原文件内容不变 | 新增 |
| `createAdr — 模板缺失` | 异常 | 解析不到 `templates/adr.md` → `success: false` 且含明确 error，不创建 decisions 文件 | 新增 |
| `createAdr — title 边界` | 边界 | `title: ""` → 失败或可观测拒绝；不写无效文件名 | 新增 |
| `createAdr — title 边界` | 边界 | `title` 超长（>1000 chars）→ slug/文件名行为可预期（截断或成功写出），不得抛未捕获异常 | 新增 |
| `createAdr — title 边界` | 边界 | `title` 含空白、标点、emoji、`\n` → kebab slug 合法（小写、空白→`-`、去非常用字符） | 新增 |
| `createAdr — title 边界` | 边界 | `title` 为 `undefined`/`null`（cast）→ 结构化失败 | 新增 |
| `createAdr — alternatives 边界` | 边界 | `alternatives: []` → 仍含 `## 备选方案` 章节，不崩溃 | 新增 |
| `createAdr — alternatives 边界` | 边界 | `alternatives` 单元素 / 超大列表（如 50 项）→ 方案序号连续且全部写出 | 新增 |
| `createAdr — alternatives 边界` | 边界 | `alternatives: undefined` 与元素缺 `description`/`pros`/`cons` → 使用占位或空优点/缺点，不抛错 | 新增 |
| `createAdr — scope 边界` | 边界 | `scope: []` 或省略 → 影响范围为 `- (none)` 或模板等价占位 | 新增 |
| `createAdr — scope 边界` | 边界 | `scope` 单元素 / 超大列表 / 含特殊字符 id → 逐条 `-` 列表 | 新增 |
| `createAdr — consequences 边界` | 边界 | `consequences` 为空串 / 省略 / 超长文本 → 保留后果子节标题且不丢 `## 后果` | 新增 |
| `listAdrs — 排序与过滤 (AC-04)` | 正向 | 多份不同日期 ADR 时 `adrs` 按 `date` 降序；`count` 等于条目数 | 新增 |
| `listAdrs — 排序与过滤 (AC-04)` | 正向 | `status: 'accepted'` 仅返回该状态；其它状态被过滤 | 新增 |
| `listAdrs — 空目录` | 正向 | `decisions/` 不存在 → `{ adrs: [], count: 0 }` | 新增 |
| `listAdrs — 解析失败文件` | 异常 | 目录含不可读或非 `.md` 文件 → 跳过坏文件，其余仍返回 | 新增 |
| `listAdrs — status 边界` | 边界 | 过滤 `proposed`/`accepted`/`deprecated`/`superseded` 各枚举值各测一次 | 新增 |
| `listAdrs — status 边界` | 边界 | `status` 省略与 `undefined` 等价于不过滤 | 新增 |
| `updateAdr — 改状态 (AC-05)` | 正向 | 已有 ADR 更新为 `accepted` → `success: true`，文件 `**状态**` 变为 `accepted`，返回 old/new status | 新增 |
| `updateAdr — superseded (AC-05)` | 正向 | `status: 'superseded'` 且提供 `superseded_by` → 状态更新且插入 `**取代者**`（原先无该行时） | 新增 |
| `updateAdr — 缺 superseded_by (AC-05)` | 异常 | `status: 'superseded'` 且无 `superseded_by` → `success: false`，文件内容字节级不变 | 新增 |
| `updateAdr — 文件不存在` | 异常 | `file` 指向不存在 ADR → `success: false`，不创建新文件 | 新增 |
| `updateAdr — 非法 status` | 异常 | `status` 非法 → `success: false`，文件不变 | 新增 |
| `updateAdr — file 边界` | 边界 | `file: ""` / 超长 / 含路径穿越片段 → 失败且不写到 decisions 外 | 新增 |
| `updateAdr — superseded_by 边界` | 边界 | `superseded_by: ""` 视为缺失 → 与无字段同样失败 | 新增 |
| `updateAdr — superseded_by 边界` | 边界 | 文件已有 `**取代者**` 时再次 superseded 不重复插入多行 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 `projectRoot` | `fs.mkdtemp` 真实目录；其下构造/断言 `openspec/architecture/decisions/` | create/list/update 全部 |
| `templates/adr.md` | 默认依赖真实插件根模板；模板缺失场景通过隔离 cwd/模块路径或临时改名/spy `fs.readFileSync` 模拟找不到模板 | create 正向与模板缺失 |
| 系统日期 | 必要时 spy `Date` 固定日历日，断言文件名前缀 `YYYY-MM-DD` | create 文件名 |
| 文件系统错误 | spy `fs.readFileSync`/`writeFileSync` 抛 `OSError` 等价错误 | list 跳过坏文件 / update 读失败 |

---

### `plugins/dev-team/bin/src/mcp.ts` -> `plugins/dev-team/bin/src/mcp.test.ts`

#### 待测功能

- `connectToServer(transport: Transport): Promise<McpServer>`: 在既有工具集上注册 `archi_decide`（`name` 精确为 `archi_decide`，无斜杠）；`inputSchema`/`outputSchema` 绑定 `archiDecideInputSchema`/`archiDecideOutputSchema`；handler 经 `withResolvedProjectRoot` 后按 `action` 调用 `createAdr` / `listAdrs` / `updateAdr`，经 `jsonContent` 返回
- MCP 工具清单常量（`EXPECTED_TOOL_NAMES` 等测试夹具）: 工具总数由 11 增至 12，排序名集合含 `archi_decide`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `listTools — archi_decide (AC-01)` | 正向 | `listTools()` name 集合经 sort 后含 `archi_decide`，且无 `archi/decide`；总数与更新后的 `EXPECTED_TOOL_NAMES` 严格相等 | 新增 |
| `listTools — description` | 正向 | `archi_decide` description 与注册字面量逐字节相等 | 新增 |
| `inputSchema — project_root (AC-06)` | 正向 | `archi_decide.inputSchema.required` 含 `project_root` 与 `action`；与其它须解析根的工具一并枚举 | 新增 |
| `inputSchema — action 判别` | 正向 | create 分支必填 `title`/`background`/`decision`；list 仅根+action；update 必填 `file`/`status`（Zod/JSON Schema 可观测） | 新增 |
| `callTool — 分发到 lib` | 正向 | mock `createAdr`/`listAdrs`/`updateAdr` 后，`action` 各值各 callTool 一次，对应 spy 调用 1 次且入参含解析后的 project root | 新增 |
| `callTool — create 成功结构 (AC-02)` | 正向 | lib 返回成功时 MCP 非 `isError`，structured/text 含 success 与 filename/path | 新增 |
| `withResolvedProjectRoot (AC-06)` | 正向 | resolve 成功后 `withResolvedProjectRoot` 被调用，run 收到的根等于 mock resolve 根 | 新增 |
| `withResolvedProjectRoot (AC-06)` | 异常 | resolve 抛错时返回 `isError`，lib spy 调用次数为 0 | 新增 |
| `callTool — Zod 拒参` | 异常 | 缺 `project_root` / 缺 `action` / create 缺 `title` → 校验失败，不调用 lib | 新增 |
| `callTool — 非法 action` | 异常 | `action: 'delete'` 或空串 → 校验失败 | 新增 |
| `全工具回归` | 正向 | 既有「全部 tool 各 callTool 一次」用例扩至含 `archi_decide`（合法最小 create/list/update 参数之一） | 新增 |
| `工具数量字面量` | 边界 | 文案/断言中「11 个 tool」全部更新为 12，避免旧常量残留导致假绿 | 新增 |
| `name 边界` | 边界 | 注册名严格等于 `archi_decide`（禁止 `archiDecide` / `archi-decide`） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `./lib/archi-decide` | `vi.mock`：`createAdr`/`listAdrs`/`updateAdr` 返回可控成功/失败结构 | callTool 分发与错误透传 |
| `withResolvedProjectRoot` | 沿用现有 mock：成功透传 run / 抛错映射 isError | AC-06 |
| MCP in-memory transport | 既有 `connectToServer` + client 辅助 | listTools / callTool |
| `EXPECTED_TOOL_NAMES` / descriptions | 测试夹具增补 `archi_decide` 并排序 | AC-01 回归 |

---

### `plugins/dev-team/bin/src/hooks.ts` -> `plugins/dev-team/bin/src/hooks.test.ts`

#### 待测功能

- `runProtectFiles(): void`: PowerShell/Bash 写保护；本变更将依赖 `archi-decide.py` 的 python 豁免 fixture 替换为其它非 ADR 命令，行为仍为「行首 python/python3/node 豁免」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `detectPowerShellWrite python 豁免 (AC-10)` | 正向 | fixture 命令改为非 `archi-decide.py` 的 python 脚本路径（如 `python scripts/process.py`）时仍返回 `allow` | 新增 |
| `detectPowerShellWrite python 豁免 (AC-10)` | 废弃 | 原 `python plugins/dev-team/utils/archi-decide.py list` fixture 用例 | 废弃 |
| `python 豁免回归` | 正向 | 行首 `python`/`python3`/`node` 写入受保护路径仍 `allow`（既有参数化用例保持） | 新增 |
| `python 豁免回归` | 异常 | `python3x`（无空格粘连）不得误豁免，写入受保护路径仍 `deny` | 新增 |
| `fixture 路径断言 (AC-10)` | 边界 | 全文件源码/字符串中不再出现 `archi-decide.py` 字面量（可用测试内 `readFileSync` 自检或静态审查配套） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| stdin / `readFileSync` 工具事件 | 沿用现有 hooks mock：注入 `tool_name: 'PowerShell'` + `command` | 豁免与 deny 用例 |
| stdout 捕获 | 解析 `permissionDecision` | 全部 hooks 用例 |

---

### `plugins/dev-team/bin/src/cli.ts` -> `plugins/dev-team/bin/src/cli.test.ts`

#### 待测功能

- `cli`（`cac('dev-team')` 导出）: 命令路由与 help；本变更**不**新增 `archi-decide` / `archi decide` 子命令

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `CLI 无 archi-decide (AC-11)` | 正向 | 已注册命令名列表（或 `cli.commands` / help 文本）不含 `archi-decide`、`archi`+`decide` 组合 | 新增 |
| `CLI 未知命令` | 异常 | 执行 `archi-decide` / `archi decide` 作为 CLI 参数时走未注册命令错误路径（非静默成功） | 新增 |
| `既有命令回归` | 正向 | `test-execution` / `run_static_analysis` 等既有注册不受影响 | 新增 |
| `help 文本边界` | 边界 | help 输出字符串中不包含 `archi-decide` / `archi decide` 子串 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `process.exit` | 沿用 `spyOnProcessExit` | 未知命令 / 缺参 |
| command 实现模块 | 既有 `vi.mock` 命令 runner | 注册回归 |

---

## 集成测试

### MCP `archi_decide` → `withResolvedProjectRoot` → `lib/archi-decide` → `plugins/dev-team/bin/__tests__/archi-decide-mcp/archi-decide-mcp.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/mcp.ts` | 触发方：注册并分发 `archi_decide` |
| `plugins/dev-team/bin/src/lib/project-root.ts` | 中间件：candidates/force 解析 `project_root` |
| `plugins/dev-team/bin/src/lib/archi-decide.ts` | 执行方：create/list/update 落盘与查询 |
| `plugins/dev-team/bin/src/schemas/archi-decide.schema.ts` | 校验方：按 `action` 判别联合入参 |

**关联AC**: AC-01, AC-05, AC-06

**关系描述**:

宿主通过 MCP `callTool('archi_decide')` 传入结构化参数；`mcp.ts` 先走与其它 `archi_*` 相同的 `withResolvedProjectRoot`，再按 `action` 调用 lib。该链路验证工具名注册、根解析失败短路、以及 update 业务错误（如 superseded 缺字段）经 MCP 表面返回结构化失败。单测可分别 mock 根解析或 lib，集成测试则在真实 handler 接线（可对文件系统用 temp root）下暴露「schema 通过但未传入解析根」或「根失败仍写盘」类错误。

#### 场景: listTools 含 underscore 工具名

验证 `tools/list` 出现 `archi_decide` 且不含斜杠名。前置为 in-memory MCP 连接。输入为 listTools。预期 name 集合含 `archi_decide`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | listTools 含且仅含合法名 `archi_decide`（无 `archi/decide`） | 新增 |
| 异常 | 缺 `project_root` 的 callTool 被拒且不触碰 temp decisions 目录 | 新增 |
| 边界 | `project_root` 为空串时走与其它工具一致的解析失败，不调用写盘 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| MCP transport | in-memory client/server | listTools / callTool |
| 真实 temp project root | `mkdtemp` + 可选写入既有 ADR | update/list 场景 |

#### 场景: update 经 MCP 的 superseded 校验

验证经 MCP 调用 `action=update` 且 `status=superseded` 无 `superseded_by` 时返回失败且文件不变。前置为 temp 根下已有 ADR。输入为 callTool update。预期结构化失败、磁盘内容不变。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | update 为 `accepted` 后文件状态变更且 MCP 非 isError | 新增 |
| 异常 | superseded 无 superseded_by → 失败且文件不变 | 新增 |
| 边界 | resolve 根与 args 中原始 `project_root` 不一致时，以 resolve 根读写 ADR | 新增 |

##### Mock策略

<!-- 无跨进程外部服务；文件系统使用真实 temp 目录，省略网络 Mock。 -->

---

### create → `templates/adr.md` → decisions 落盘 → `plugins/dev-team/bin/__tests__/archi-decide-create-template/archi-decide-create-template.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/archi-decide.ts` | 写入方：渲染并写 ADR |
| `plugins/dev-team/templates/adr.md` | 读取方：章节结构权威模板 |
| `openspec/architecture/decisions/`（用户项目目录） | 持久化目标 |

**关联AC**: AC-02, AC-03, AC-07

**关系描述**:

`createAdr` 必须从插件根加载真实 `templates/adr.md` 填充字段后写入用户项目 decisions 目录，而不是沿用旧 Python 硬编码字符串。集成测试用真实模板文件 + temp projectRoot，断言章节标题与默认 `proposed` 状态，并覆盖模板不可解析时「失败且无半文件」。这能抓到「单测 stub 了模板内容但安装态路径探测错误」的问题。

#### 场景: 真实模板渲染 create

验证 create 产出与模板章节对齐且默认 status 为 proposed。前置为可解析到插件 `templates/adr.md` 的运行环境与空 temp 项目根。输入为最小合法 create 字段。预期文件名 `YYYY-MM-DD-<slug>.md`，内容含全部章节。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 真实模板渲染后含全部章节标题与填入的 background/decision | 新增 |
| 正向 | 未传 status 时状态为 `proposed` | 新增 |
| 异常 | 人为切断模板解析路径时失败且 decisions 下无新文件 | 新增 |
| 边界 | scope 为空时影响范围为 `(none)` 或模板等价占位；alternatives 为空仍保留备选方案节 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 模板路径探测 | 正向使用真实插件模板；异常场景临时移走/改名或注入不可达根 | 模板缺失 |
| 系统日期 | 可选固定 Date 以稳定文件名前缀 | 文件名断言 |

---

### Agent / 产物下线契约 → `plugins/dev-team/bin/__tests__/archi-decide-retirement/archi-decide-retirement.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/agents/architecture.md` | 文案契约：Decide mode 仅引用 MCP |
| `plugins/dev-team/utils/`（源码树） | 负向存在性：不应再有 `archi-decide.py` |
| `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/` | 构建产物与 manifest 同步面 |

**关联AC**: AC-08, AC-09

**关系描述**:

迁移完成后 agent 说明与三端构建产物必须与源码一致：Decide 步骤只提 `archi_decide`，且任何 `utils/archi-decide.py` 路径不得残留于源码或 manifest。该类断言跨文档/产物目录，适合放在 `__tests__` 集成套件；在 CI 于 `pnpm -C plugins/dev-team run build` 之后运行时最有价值。可能的出错模式是源码已删但 assemble 仍复制旧文件，或 agent 文案漏改导致 Agent 仍尝试 Bash 调 Python。

#### 场景: architecture.md 仅 MCP

验证 Decide 相关段落含 `archi_decide` 且不含 `archi-decide.py` / `python ... archi-decide`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `architecture.md` 含 `archi_decide` 与 Decide 步骤 MCP 指引 | 新增 |
| 异常 | 全文不出现 `archi-decide.py` 与 `python .../archi-decide` | 新增 |
| 边界 | 工具清单段不再列出 `__DEV_TEAM_ROOT__/utils/archi-decide.py` | 新增 |

##### Mock策略

<!-- 只读文件系统断言，无需 Mock。 -->

#### 场景: Python 脚本与 manifest 下线

验证源码与（若测试环境已 build）产物中不存在 `utils/archi-decide.py`，且 manifest 无该条目。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `plugins/dev-team/utils/archi-decide.py` 不存在 | 新增 |
| 正向 | 已构建时三端插件目录均无该文件；`manifest.json` 无 `archi-decide.py` 条目 | 新增 |
| 异常 | 若仅源码删除但 manifest 仍列出该路径则测试失败 | 新增 |
| 边界 | 构建产物未生成时跳过产物断言并标明 skip 原因，源码断言仍执行 | 新增 |

##### Mock策略

<!-- 文件系统存在性检查；产物缺失时用条件 skip，无外部服务 Mock。 -->

---

## 不可测试项

- `plugins/dev-team/bin/src/schemas/archi-decide.schema.ts` — **原因**: `test_resolve_paths` 返回 `Not in test config scope`；schema 行为通过 `mcp.test.ts` 的 inputSchema/拒参用例与 lib 入参间接覆盖
- `plugins/dev-team/bin/src/schemas/index.ts` — **原因**: barrel 导出不在测试配置 scope；由 mcp 导入路径编译/运行间接保证
- create 输出与旧 `archi-decide.py` 字节级一致 — **原因**: proposal/design 明确不保证；以模板章节与字段契约为准（AC-07）
- Cursor/Claude 宿主内真实 Agent Decide 会话端到端点选 MCP 工具 — **原因**: 依赖宿主 UI/运行时；以 `architecture.md` 文案契约与 MCP 注册集成测试代替
- `CLAUDE.md` 文档表述同步 — **原因**: 非行为验收项，属文档对齐，不纳入自动化 AC 门禁
