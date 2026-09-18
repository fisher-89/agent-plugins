# 测试设计: workflow-files-query-api

> **日期**: 2026-09-17

---

## 验收范围

<!-- AC-1 ~ AC-4 由测试框架（vite-plus，测试根 plugins/dev-team，colocated *.test.ts + bin/__tests__ 集成套件）覆盖；
     AC-5 / AC-6 由 proposal/design 指定的非测试框架手段（grep / build 产物验证）承接，见「不可测试项」。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | `mcp.ts` 注册 `workflow_files`：MCP tools/list 含 `workflow_files`，inputSchema 含必填 `change`；description 表述为只读查询 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts`（注册元数据断言）；跨模块链路另见集成测试关系 1 |
| AC-2 | 对含 `files` 的 change 返回 `{ written, deleted }` 净状态；输出不含 `source`；调用前后 `workflow.json` 逐字节不变 | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/files-query.test.ts`；MCP 层闭环见集成测试关系 1、关系 2 |
| AC-3 | `workflow.json` 缺失 / JSON 非法 / schema 不通过 / 缺 `files` 字段时返回错误且文案含重建指引，MUST NOT 回退 git diff | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/files-query.test.ts`（四态）；isError 映射见集成测试关系 1 |
| AC-4 | `workflow/index.ts` 导出四函数及类型；仓库内无 `lib/file-inventory` 导入残留；`lib/file-inventory.ts` 已删除 | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/index.test.ts`（barrel 导出）、`plugins/dev-team/bin/src/modules/workflow/file-inventory.test.ts`（迁移）、导入方套件回归（`change-files.test.ts`、`test-resolve-paths.test.ts`、`test-execution.test.ts`、`c4-cross-ref.test.ts`、`backtrack.test.ts`、`hooks.test.ts`） |
| AC-4 | （集成回归面）读写通道与只读消费方在导入改道 barrel 后组合行为不变 | 集成测试 | `plugins/dev-team/bin/src/mcp.test.ts`、`plugins/dev-team/bin/__tests__/archi-check-inventory/archi-check-inventory.test.ts`、`plugins/dev-team/bin/__tests__/inventory-backtrack-preserve/inventory-backtrack-preserve.test.ts`、`plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts` |
| AC-5 | 三个 evaluator md 的范围圈定步骤指向 `__MCP:workflow_files__`，不再含「Read `workflow.json`」清单读取指令 | 静态检查（grep，非测试框架） | `plugins/dev-team/agents/implementation-evaluator.md`、`plugins/dev-team/agents/code-review-evaluator.md`、`plugins/dev-team/agents/acceptance-evaluator.md` |
| AC-6 | `plugins/dev-team/package.json` `version` 提升，build 成功且产物目录刷新 | 构建验证（非测试框架） | `plugins/dev-team/package.json`、`claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/` |

---

## 单元测试

<!-- 框架：vite-plus（`vp test` 运行器，colocated `*.test.ts`）。
     既有模式：文件系统不 mock（mkdtempSync 临时工程 + 真实读写）；模块依赖经 vi.mock / vi.spyOn。
     测试文件放置遵循 config.json tests 套件（root `plugins/dev-team`，includes `bin/src/**/*.{ts,tsx}`，
     excludes `bin/src/schemas/**/*`）。 -->

### plugins/dev-team/bin/src/modules/workflow/files-query.ts -> plugins/dev-team/bin/src/modules/workflow/files-query.test.ts

#### 待测功能

- runWorkflowFiles(options: WorkflowFilesOptions): WorkflowFilesOutput — 只读聚合指定 change 的 `files` 净状态，投影为 `{ written, deleted }`（新建字面量对象，结构性排除 `source`）；无任何 fs 写调用、无 git diff 回退
- WorkflowFilesOptions — 查询入参类型：`change` 必填；`project_root` 可选，缺省（含空串 falsy）回退 `getProjectDir()`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runWorkflowFiles — 净状态投影 | 正向 | files 含 `written`/`deleted` 双数组与 `source` 审计映射 → 返回 `{ written, deleted }`，输出对象无 `source` 键（`Object.hasOwn` 为 false，`JSON.stringify` 不含 `"source"`）(AC-2) | 新增 |
| runWorkflowFiles — 净状态投影 | 正向 | 显式传入 `project_root` → 从该根定位 `openspec/changes/<change>/workflow.json` 并返回其净状态 (AC-2) | 新增 |
| runWorkflowFiles — 净状态投影 | 边界 | 空净状态 `{ written: [], deleted: [] }`（change_create 初始形态）→ 返回双空数组，不补建字段 | 新增 |
| runWorkflowFiles — 净状态投影 | 边界 | 路径含空串 / 超长（>1000 chars）/ 空格、中文、emoji → 原样透传不做语义裁剪 | 新增 |
| runWorkflowFiles — project_root 解析 | 边界 | 省略 `project_root` → 回退 `getProjectDir()`（`CLAUDE_PROJECT_DIR` 指向 fixture 根时查询成功） | 新增 |
| runWorkflowFiles — project_root 解析 | 边界 | `project_root` 为空字符串（falsy）→ 同样回退 `getProjectDir()`，不当作字面根路径 | 新增 |
| runWorkflowFiles — project_root 解析 | 异常 | `project_root` 指向无 workflow.json 的目录 → 抛「workflow.json 不存在」态①错误，路径指向该根下 | 新增 |
| runWorkflowFiles — 只读不变量 | 边界 | 成功查询前后 `workflow.json` 原始文本逐字节一致（含缩进与尾换行），且 change 目录内无任何新增/修改文件 (AC-2) | 新增 |
| runWorkflowFiles — 只读不变量 | 边界 | files 含非空 `source` 时查询后盘上 `files.source` 原样保留（只读投影不改盘）(AC-2) | 新增 |
| runWorkflowFiles — 硬报错四态 | 异常 | 态①：change 目录 / workflow.json 不存在 → 抛错含「workflow.json 不存在」与 change_create 指引 (AC-3) | 新增 |
| runWorkflowFiles — 硬报错四态 | 异常 | 态②：JSON 截断 / 含注释 → 抛「解析失败」；根为数组 / 字符串 / null → 抛「根元素必须是对象」(AC-3) | 新增 |
| runWorkflowFiles — 硬报错四态 | 异常 | 态③：`files.written` 非数组 / `workflow_type` 非法枚举 → 抛「格式非法」且含 issue 明细 (AC-3) | 新增 |
| runWorkflowFiles — 硬报错四态 | 异常 | 态④：合法 workflow.json 缺 `files` 字段 → 抛错文案含「该 change 创建于文件清单机制之前，请重建」(AC-3) | 新增 |
| runWorkflowFiles — 硬报错四态 | 异常 | 四态错误路径下 `workflow.json` 与 change 目录逐字节不变、不创建任何文件（硬报错不落盘）(AC-3) | 新增 |
| runWorkflowFiles — 硬报错四态 | 异常 | 任一四态错误均以抛错结束（调用方拿到 Error），不存在返回降级结果或 git diff 内容的回退路径（错误文案不含 diff 输出）(AC-3) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 不 mock — `fs.mkdtempSync` 临时工程 + 真实读写（同 `lib/file-inventory.test.ts` / `commands/change-files.test.ts` 既有模式） | 全部 describe |
| 运行环境 CLAUDE_PROJECT_DIR | `vi.stubEnv('CLAUDE_PROJECT_DIR', fixtureRoot)` 指向临时工程根（`getProjectDir` 解析链优先级），`afterEach` 恢复（同 `bin/__tests__/inventory-backtrack-preserve` 模式，不用 process.chdir） | 「project_root 解析」describe 的回退两用例 |

---

### plugins/dev-team/bin/src/modules/workflow/index.ts -> plugins/dev-team/bin/src/modules/workflow/index.test.ts

#### 待测功能

- barrel 出口：re-export `file-inventory.ts` 的 readFileInventory / foldFileOps / writeFileInventory 与 FileInventory / FileOp 类型，及 `files-query.ts` 的 runWorkflowFiles（AC-4 的直接断言面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| workflow barrel — 函数导出 | 正向 | `import * as workflow from './index'` → readFileInventory / foldFileOps / writeFileInventory / runWorkflowFiles 均为 `function` 类型 (AC-4) | 新增 |
| workflow barrel — 函数导出 | 正向 | barrel 导出与源模块直接导入为同一函数引用（`toBe`），证明 re-export 而非复制 (AC-4) | 新增 |
| workflow barrel — 类型导出 | 边界 | `FileInventory` / `FileOp` / `WorkflowFilesOptions` 类型可从 barrel 编译期导入并用于变量标注（tsc 通过；运行时以函数断言承载） | 新增 |
| workflow barrel — 导出面精确 | 边界 | barrel 模块运行时导出键恰好等于 4 个函数名，无多余导出（防意外面与 knip dead export） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯模块导入与引用断言，不涉及跨进程边界，无 mock 需求 | 全部用例 |

---

### plugins/dev-team/bin/src/modules/workflow/file-inventory.ts -> plugins/dev-team/bin/src/modules/workflow/file-inventory.test.ts

<!-- 自 lib/file-inventory.test.ts 整体迁移：内容逐字保留（断言、fixture、describe 结构均不变），
     仅文件位置与相对导入调整（'./file-inventory' → './file-inventory' 同目录不变、schemas/utils 层级 +1）。
     以下用例行即既有套件的 describe 组清单，测试生成阶段执行整体迁移而非重写。 -->

#### 待测功能

- readFileInventory(changeDir: string): FileInventory — 读取并校验 `<changeDir>/workflow.json` 的 files 字段，四态硬报错（无 git 回退）
- foldFileOps(inventory: FileInventory, ops: FileOp[]): FileInventory — 纯函数折叠 write/delete/revert 操作净状态，维护 `source` 旁挂审计映射
- writeFileInventory(changeDir: string, files: FileInventory): void — 保留键写回纪律（workflow_type / created / eval / 未知键保留；2 空格缩进 + 尾换行；绝不创建文件）
- FileInventory / FileOp — 类型定义（迁移）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| readFileInventory — 正向 | 正向 | 合法 workflow.json 全形态用例迁移：source 透传 / 缺 source 不补键 / files 未知字段宽松 / 空净状态 / 空串超长中文 emoji 路径透传（原文件断言不变） | 新增 |
| readFileInventory — 异常 | 异常 | 四态硬报错既有用例迁移：文件不存在 / 解析失败 / 根非对象 / 格式非法 / 缺 files 重建指引（原文件断言不变） | 新增 |
| foldFileOps — 折叠规则 | 正向 | write / delete / revert 对称折叠、delete→write 归位、mv 双条目、12 步长序列结合性、重复 op 去重、目录路径折叠既有用例迁移 | 新增 |
| foldFileOps — source 旁挂来源维护 | 边界 | last-writer-wins / 无 agentType 重写清除 / revert 联动移除 / delete 联动移除 / 空映射丢弃既有用例迁移 | 新增 |
| foldFileOps — 异常与纯函数不变量 | 边界 | 未知 op 运行时兜底忽略 / ops=[] 深拷贝不修改入参 / 单桶不变量扫描既有用例迁移 | 新增 |
| writeFileInventory — 写回纪律 | 正向 | 保留键写回 / 2 空格缩进尾换行 / source 旁挂与省略 / 空净状态写回既有用例迁移 | 新增 |
| writeFileInventory — 写回纪律 | 异常 | 绝不创建契约（缺文件抛错不建目录）/ JSON 非法不改盘 / 根非对象不改盘既有用例迁移 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 不 mock — mkdtempSync 临时工程 + 真实读写（迁移文件既有模式，无变化） | 全部 describe |

---

### plugins/dev-team/bin/src/mcp.ts -> plugins/dev-team/bin/src/mcp.test.ts

#### 待测功能

- connectToServer(transport): Promise&lt;McpServer&gt; — 按 MCP_TOOLS 数组注册全部工具（本次新增 `workflow_files` 条目，工具总数 15 → 16）
- workflow_files handler — `withResolvedProjectRoot('workflow_files', …)` 注入解析根后委托 runWorkflowFiles 并经 jsonContent 返回（mcp.ts 顶部静态导入自 `./modules/workflow`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| listTools — workflow_files (AC-1) | 正向 | names 含 `workflow_files`；EXPECTED_TOOL_NAMES 扩至 16 且「严格等于排序后 16 个 name」「长度 16」「无重复」既有断言同步更新 (AC-1) | 新增 |
| listTools — workflow_files (AC-1) | 正向 | inputSchema.properties 含 `change` 与 `project_root`；required 含 `change` 与 `project_root`（AC-1 必填 change 断言） | 新增 |
| listTools — description (AC-1) | 正向 | description 与 EXPECTED_TOOL_DESCRIPTIONS.workflow_files 注册字面量字节级相等，且含只读查询 / 从不修改 workflow.json / 不含 source 审计映射 / 缺失硬报错不回退 git diff / 补录修正请用 change_files 要点 (AC-1) | 新增 |
| registerTool spy — 注册顺序 | 边界 | REGISTER_ORDER 在 change_files 后插入 workflow_files（读写成对），顺序逐项断言扩至 16（既有 15 项断言同步） | 新增 |
| 必填 project_root 套件 | 边界 | toolsWithProjectRoot 与 ALL_INPUT_SCHEMAS 增 workflow_files 条目；最小合法入参 `{ change: 'c' }` 缺 project_root 时 safeParse 失败（既有循环分支扩展） | 新增 |
| workflowFilesInputSchema — change 校验 | 异常 | 缺 `change` / `change` 为空串 / `change` 为非 string → safeParse 失败；仅 `{ change: 'c' }` + project_root 通过 | 新增 |
| 全 handler 套件（两组） | 边界 | 「全 15 handler 各 callTool 一次」两组套件扩为 16：workflow_files 以真实 fixture 调用非 isError 且返回非空文本；resolve 抛错组中 isError 且 spy 0 次 | 新增 |
| resolve 错误映射 | 异常 | mock resolve 抛 not_in_candidates → callTool workflow_files 得 isError JSON（code/project_root/candidates/force_hint 齐全）且 runWorkflowFiles spy 次数 0 | 新增 |
| change_files 成对回归 | 边界 | 既有 change_files 注册 / description / callTool 用例在 16 工具格局下继续通过（纯新增工具，既有工具名与 description 不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| project root 解析 | 沿用本文件既有 `vi.mock('./lib/project-root')`（mockResolve.root 指向 fixture 根 / makeResolveError 注入 resolve 错误） | 全部 workflow_files 用例 |
| 文件系统 | 不 mock — setupTempProject + setupChangeWithWorkflow 既有 fixture 模式，扩展含 `source` 的 files 形态 | 正向 / 边界场景 |
| runWorkflowFiles | `vi.spyOn(workflowFilesCmd, 'runWorkflowFiles')`（`import * as workflowFilesCmd from './modules/workflow'`，与 mcp.ts 静态导入为同一模块实例），用例内 mockRestore | 「次数 0」与委托参数断言场景 |

---

### plugins/dev-team/bin/src/commands/test-resolve-paths.ts -> plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts

<!-- 源文件逻辑零改动（readFileInventory 导入改道 '../modules/workflow'）；测试文件第 17 行直接导入同步改道，断言不变。 -->

#### 待测功能

- runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult — 签名与行为不变；`modules: "change"` 清单模式经迁移后的 readFileInventory 读净状态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestResolvePaths — 清单模式回归 | 正向 | 导入改道后既有 `modules: "change"` 用例全绿：清单净状态 → colocated 测试路径解析结果与改道前一致 (AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无新增 | 沿用既有 mock 格局；仅更新测试文件顶部 `readFileInventory` 导入路径为 `'../modules/workflow'` | 全部用例 |

---

### plugins/dev-team/bin/src/commands/test-execution.ts -> plugins/dev-team/bin/src/commands/test-execution.test.ts

<!-- 源文件逻辑零改动（readFileInventory 导入改道）；测试文件 73-74 行 vi.mock('../lib/file-inventory') 必须同步改指
     '../modules/workflow'（含 importActual 路径），否则删除旧文件后 mock 落空、被测代码读真实盘导致用例语义漂移。 -->

#### 待测功能

- runTestExecution(options: TestExecutionOptions): Promise&lt;number&gt; — 签名与行为不变；经 mockReadFileInventory 注入清单净状态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestExecution — 清单读取回归 | 正向 | mock 路径改道后既有用例全绿：mockReadFileInventory 注入的净状态仍被 test-execution 链路消费（mock 行为与断言不变）(AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| file-inventory 模块 | `vi.mock('../modules/workflow', importActual)` 保留 actual 展开、仅覆写 readFileInventory（原 '../lib/file-inventory' 工厂原样搬移） | 全部依赖清单注入的用例 |

---

### plugins/dev-team/bin/src/commands/change-files.ts -> plugins/dev-team/bin/src/commands/change-files.test.ts

<!-- 源文件仅导入改道 barrel（FileInventory / readFileInventory / writeFileInventory），逻辑零改动；
     测试文件不经 file-inventory 模块导入（经 runChangeCreate fixture 真盘验证），本次零改动，作为迁移回归网。 -->

#### 待测功能

- runChangeFiles(options: ChangeFilesOptions): ChangeFilesOutput — 签名与行为不变（append / set 语义、保留键写回、硬报错前置校验）
- ChangeFilesOptions — 命令入参类型（不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeFiles — 迁移回归 | 正向 | 源文件导入改道后既有全部用例（append 折叠合并 / set 覆写与 source 清理 / 保留键写回 / 缺 files 硬报错）零断言改动通过 (AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无新增 | 沿用既有模式（文件系统不 mock + runChangeCreate 真实 fixture），测试文件不改动 | 全部用例 |

---

### plugins/dev-team/bin/src/commands/record-files.ts -> plugins/dev-team/bin/src/commands/record-files.test.ts

<!-- test_resolve_paths 解析出 colocated 路径 record-files.test.ts，但该文件在仓库中不存在：
     记录器的既有覆盖在 bin/__tests__/file-inventory-recording 集成套件（stdin 事件回放全真实链路）。
     本变更对 record-files.ts 仅做导入改道（FileOp / foldFileOps / readFileInventory / writeFileInventory →
     '../modules/workflow'），逻辑零改动，不新建 colocated 单测（避免为迁移重复造轮子），行为回归由集成套件保障（见集成测试关系 5）。 -->

#### 待测功能

- runRecordFiles(): void — PostToolUse 记录器入口，签名与行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runRecordFiles — 迁移回归 | 正向 | 导入改道后 file-inventory-recording 集成套件全绿（折叠 / 写回 / 自污染过滤行为不变），见集成测试关系 5 (AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 见集成测试关系 5 | 沿用该套件既有 stdin（fd 0）/ os.tmpdir / getProjectDir mock，无新增 | 集成回归 |

---

### plugins/dev-team/bin/src/lib/c4-cross-ref.ts -> plugins/dev-team/bin/src/lib/c4-cross-ref.test.ts

<!-- 源文件仅导入改道（'./file-inventory' → '../modules/workflow'），逻辑零改动；测试文件不直接导入
     file-inventory（经 fixture 造盘），本次零改动，作为迁移回归网。 -->

#### 待测功能

- runCrossRefCheck(projectRoot, options): Promise&lt;ArchiCheckResult&gt; — 签名与行为不变；change 清单模式经迁移后的 readFileInventory 读被查文件集

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runCrossRefCheck — 迁移回归 | 正向 | 导入改道后既有清单模式用例全绿（files.written 驱动被查文件集）；四态硬报错文案与 readFileInventory 迁移体保持一致 (AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无新增 | 沿用既有 mock 格局，测试文件不改动 | 全部用例 |

---

### plugins/dev-team/bin/src/lib/shell-file-ops.ts -> plugins/dev-team/bin/src/lib/shell-file-ops.test.ts

<!-- test_resolve_paths 解析出 colocated 路径 shell-file-ops.test.ts，但该文件在仓库中不存在：
     extractFileOps 的既有覆盖经 file-inventory-recording 集成套件的 shell 命令事件回放间接达成。
     本变更仅将 `type FileOp` 导入改道 '../modules/workflow'（类型层引用），运行时逻辑零改动，
     不新建单测；类型改道由 tsc 全量编译验证，行为回归由集成套件保障。 -->

#### 待测功能

- tokenize(args: string): string[] / isFlag(token: string): boolean / extractGitOps(cmd: string): FileOp[] / extractFileOps(cmd: string): FileOp[] — 签名与行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| extractFileOps — 类型改道回归 | 正向 | `FileOp` 类型改道后既有集成套件中 shell 命令提取行为不变（编译期验证 + 集成回归，见集成测试关系 5）(AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 类型层改道无运行时行为变化，无 mock 需求 | — |

---

### plugins/dev-team/bin/src/commands/backtrack.ts -> plugins/dev-team/bin/src/commands/backtrack.test.ts

<!-- backtrack.ts 源文件不在本变更范围（eval/元数据逻辑不迁移）；仅测试文件第 59 行
     `import { readFileInventory, writeFileInventory } from '../lib/file-inventory'` 改指 '../modules/workflow'，
     否则旧文件删除后编译失败。断言零改动。 -->

#### 待测功能

- runBacktrack(options: BacktrackOptions): BacktrackResult — 源文件不改动；测试文件导入改道（fixture 造盘用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runBacktrack — 导入改道回归 | 正向 | 导入改道后既有全部用例（backtrack 状态写入、清单中立）零断言改动通过 (AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无新增 | 沿用既有 mock 格局；仅更新测试文件顶部导入路径为 '../modules/workflow' | 全部用例 |

---

### plugins/dev-team/bin/src/hooks.ts -> plugins/dev-team/bin/src/hooks.test.ts

<!-- hooks.ts 源文件不在本变更范围；hooks.test.ts:63 的 vi.mock('./lib/file-inventory') 必须改指
     './modules/workflow'（含 importOriginal 路径），否则删除旧文件后 mock 模块解析落空，
     mockReadFileInventory / mockWriteFileInventory 不再被 hooks → record-files 链路命中。 -->

#### 待测功能

- main() / runProtectFiles / runRecordFiles / runStaticCheck — hooks 调度面不改动；测试文件 mock 路径改道

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runRecordFiles（hooks 入口）— mock 改道回归 | 正向 | mock 路径改道后既有「记录器折叠写回」用例全绿：mockReadFileInventory / mockWriteFileInventory 仍被经 record-files 的调用链命中 (AC-4) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| file-inventory 模块 | `vi.mock('./modules/workflow', importOriginal)` 保留 actual 展开、覆写 readFileInventory / writeFileInventory（原 './lib/file-inventory' 工厂原样搬移，mock 函数复用既有 vi.hoisted 实例） | 全部依赖清单 mock 的用例 |

---

## 集成测试

<!-- 集成测试验证跨模块交互。本变更的跨模块面集中在两处：
     ① mcp.test.ts 的 InMemoryTransport 端到端（既有模式：注册元数据 + callTool 全链路 + 真实 fs fixture）；
     ② bin/__tests__/ 下三个既有集成套件的导入改道回归（真实链路不 mock，仅断言改道后行为不变）。 -->

### agent 调用 workflow_files → files-query → file-inventory 查询链路 → `plugins/dev-team/bin/src/mcp.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/mcp.ts` | 注册与分发方（withResolvedProjectRoot 注入根） |
| `plugins/dev-team/bin/src/modules/workflow/files-query.ts` | 查询方（净状态投影、排除 source） |
| `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 清单读取方（四态硬报错承载） |
| `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts` | 输入校验方（change 必填、project_root 绑定） |

**关联AC**: AC-1, AC-2, AC-3

**关系描述**: agent（evaluator）经 `__MCP:workflow_files__` 发起调用后，请求穿过 MCP 注册层（schema 校验 → project root 解析 → handler 委托）、查询模块（定位 change 目录、投影净状态）与清单库（读盘与四态校验）三层。这是本变更新增的主链路，单元测试各自 mock 掉相邻层后无法暴露组合缺陷：schema 与 handler 之间字段错位（如 change 未透传）、resolve 根与 getChangeDir 拼接错位、投影层意外携带 source、硬报错被 MCP 层吞成空成功等，都只在三层真实组合时暴露。出错模式还包括硬报错文案丢失重建指引导致 agent 无从恢复。关系 1 的场景在 mcp.test.ts 内以 InMemoryTransport 客户端对真实 fixture 工程调用验证。

#### 场景: 正向查询端到端（净状态投影 + 无 source + 只读不落盘）

前置：临时工程 fixture（setupTempProject + 手写 workflow.json，files 含 `written`/`deleted` 与非空 `source`）；mockResolve.root 指向该工程。输入：`client.callTool({ name: 'workflow_files', arguments: { change, project_root: dir } })`。预期：非 isError；返回文本与 structuredContent 均为 `{ written, deleted }` 且不含 source；调用前后 workflow.json 原始字节一致。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 真实 fixture（files 含 source）callTool workflow_files → 非 isError，返回 `{ written, deleted }`，序列化文本不含 `"source"`；查询前后 workflow.json 原始文本逐字节一致 (AC-1, AC-2) | 新增 |
| 边界 | change_create 创建的初始空净状态 change → 查询返回 `{ written: [], deleted: [] }`（链路对最小净状态成立）(AC-2) | 新增 |
| 边界 | 省略 change（schema 必填缺失）→ isError 且 runWorkflowFiles spy 次数 0（校验层拦截，不到查询模块）(AC-1) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| project root 解析 | 沿用 mcp.test.ts 既有 `vi.mock('./lib/project-root')`（mockResolve.root 指向 fixture 根） | 全部场景 |
| 文件系统 | 不 mock — 真实临时工程 fixture（setupChangeWithWorkflow 模式扩展含 source 的 files 形态） | 正向 / 边界场景 |
| runWorkflowFiles | `vi.spyOn(workflowFilesCmd, 'runWorkflowFiles')` 仅用于调用次数与入参断言，用例内恢复 | 边界（次数 0）场景 |

#### 场景: 四态硬报错经 MCP 层映射为 isError（不回退 git diff）

前置：对四态各造一个 fixture（无 workflow.json 的 change 名 / 截断 JSON / files 类型非法 / 合法但缺 files）。输入：callTool workflow_files。预期：均 isError，错误文案含重建指引（change_create / 请重建），不含 git diff 回退输出。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 态① workflow.json 不存在 → isError 且文案含「不存在」与 change_create 指引 (AC-3) | 新增 |
| 异常 | 态② JSON 截断 → isError 且文案含「解析失败」(AC-3) | 新增 |
| 异常 | 态③ files 类型非法（schema 不通过）→ isError 且文案含「格式非法」(AC-3) | 新增 |
| 异常 | 态④ 缺 files 字段 → isError 且文案含「创建于文件清单机制之前，请重建」(AC-3) | 新增 |
| 边界 | 四态错误均不修改盘上 workflow.json、不创建文件；文案均不含 git diff 字样（无回退语义）(AC-3) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 文件系统 | 不 mock — 四态各自真实 fixture 文件（截断 JSON / 非法 files 直接写盘） | 全部异常用例 |
| project root 解析 | 沿用既有 mock（root 指向 fixture 根） | 全部异常用例 |

---

### change_files 写通道 → workflow_files 读通道 净状态闭环 → `plugins/dev-team/bin/src/mcp.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/change-files.ts` | 写入方（append/set 折叠净状态并落盘） |
| `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 折叠与写回方（写通道经 barrel 消费） |
| `plugins/dev-team/bin/src/modules/workflow/files-query.ts` | 读取方（只读投影经 barrel 消费） |

**关联AC**: AC-2, AC-4

**关系描述**: 迁移后读写两端首次分离到不同模块边界（写在 commands/change-files、读在 modules/workflow/files-query，共同底层在 modules/workflow/file-inventory），两端经磁盘上的 files 净状态解耦。值得测试的原因：若迁移期间读端或写端任一侧残留旧 `lib/file-inventory` 副本导入，或 barrel 导出面不全，单侧单元测试仍可能绿，而组合后读写看到的是两份不一致的实现——写通道落盘的净状态查询不到、或查询读到过期状态。可能的出错方式还包括投影层误把 source 带进查询输出（违反规格的消费方禁令）。闭环用例在同一真实工程上先写后读，让两端在模块组合层面互相锚定。

#### 场景: append 写入后查询可见（含 source 隔离）

前置：fixture 工程 change 的 files 预置 `{ written: ['src/a.ts'], deleted: [], source: { 'src/a.ts': 'dev-team:implementation-generator' } }`。输入：先 callTool change_files（append written ['src/new.ts']），再 callTool workflow_files。预期：查询返回 `{ written: ['src/a.ts', 'src/new.ts'], deleted: [] }`（顺序按实现，断言含成员即可）；输出不含 source；盘上 files.source 仍保留（写通道 append 不清审计映射，查询仅投影层排除）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | change_files append 后 callTool workflow_files → 净状态含新写入路径，与写工具返回的净状态一致 (AC-2, AC-4) | 新增 |
| 正向 | 预置 source 的 fixture：append 后查询输出仍无 source，而盘上 files.source 原样保留（写读两端对 source 的契约分工正确）(AC-2) | 新增 |
| 边界 | set 覆写后查询可见覆写结果（写通道两种 op 与读通道均闭环）(AC-2, AC-4) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| project root 解析 | 沿用既有 mock（root 指向 fixture 根） | 全部场景 |
| 文件系统 | 不 mock — 全链路真实（change_files 与 workflow_files 均真实执行，无 spy），这是本关系与关系 1 的关键差异 | 全部场景 |

---

### c4-cross-ref（archi_check）→ modules/workflow 清单消费改道回归 → `plugins/dev-team/bin/__tests__/archi-check-inventory/archi-check-inventory.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` | 清单消费方（被查文件集组装与交叉引用） |
| `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 清单来源（迁移后经 readFileInventory 真盘读取） |
| `plugins/dev-team/bin/src/commands/change-create.ts` | fixture 创建方（真实 change 通道） |

**关联AC**: AC-4

**关系描述**: c4-cross-ref 是清单的既有只读消费方，本变更将其导入改道到 barrel。值得测试的原因：消费方与写通道分属不同导入路径，若改道期间出现双实现（barrel 与残留旧路径副本并存），检查方可能读到旧副本或空净状态，清单直通检查静默失真。该套件全真实链路（真实 .c4 模型解析、真实 workflow.json 读盘、仅 child_process spy），改道后整体回归即可锚定消费方读到的仍是同一份清单实现。

#### 场景: 清单直通检查改道后全绿

前置：导入改道 `../../src/lib/file-inventory` → `../../src/modules/workflow`，其余零改动。输入：既有用例套件（真实模型 + writeFileInventory 预置净状态 + runCrossRefCheck）。预期：全部用例零断言改动通过。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 既有清单直通用例全绿：files.written 驱动被查文件集、未建模文件可见、violation 判定不变 (AC-4) | 新增 |
| 异常 | 既有清单缺失 / 空净状态用例行为不变（错误与空结果路径不因改道漂移）(AC-4) | 新增 |
| 边界 | child_process spy not-called 断言不变（git 退场防回归在改道后仍成立）(AC-4) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| child_process | 沿用既有 `vi.mock`（execSync / execFileSync 委托真实实现，仅作 not-called 断言），无新增 mock | 全部用例 |

---

### backtrack → workflow.json.files 清单中立改道回归 → `plugins/dev-team/bin/__tests__/inventory-backtrack-preserve/inventory-backtrack-preserve.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 回溯触发方（真实 runBacktrack，不经 barrel，本变更不改） |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | eval 写回方（本变更不改） |
| `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 清单重读方（迁移后 readFileInventory 锚定回溯后净状态） |

**关联AC**: AC-4

**关系描述**: 该套件验证回溯对清单的中立语义，其中 readFileInventory 直接导入自旧路径（测试文件内），是删除旧文件后会编译失败的直接消费方。值得测试的原因：backtrack.ts 源文件不在本变更范围，其运行时经自身导入读取清单；若测试文件的导入改道但运行时导入链残旧（或反之），会出现「测试锚定的是新实现、被测代码用的是旧实现」的错位。全真实链路回归能同时锚定两侧一致。

#### 场景: 回溯清单中立改道后全绿

前置：导入改道 `../../src/modules/workflow`，env stub（CLAUDE_PROJECT_DIR 指向临时工程）不变。输入：既有用例套件（runBacktrack + readFileInventory 重读对账）。预期：全部用例零断言改动通过。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 任意回溯目标后 files 逐项不变（含 source 审计映射）既有用例全绿 (AC-4) | 新增 |
| 边界 | 机制前旧 change（缺 files）回溯成功、test-only 工作流回溯成功、回溯后记录器可继续折叠等既有用例全绿 (AC-4) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 运行环境 | 沿用既有 `vi.stubEnv('CLAUDE_PROJECT_DIR')`（不用 process.chdir），无模块 mock | 全部用例 |

---

### PostToolUse 记录器 → 清单归账改道回归 → `plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/hooks.ts` | 记录器入口（runRecordFiles 事件解析与编排） |
| `plugins/dev-team/bin/src/lib/session-registry.ts` | session 绑定注册表 |
| `plugins/dev-team/bin/src/modules/workflow/file-inventory.ts` | 折叠与写回方（record-files.ts 经 barrel 消费，迁移后） |

**关联AC**: AC-4

**关系描述**: 记录器链路（hooks → record-files → file-inventory）在源文件侧改道 barrel，同时 hooks.test.ts 的 vi.mock 路径必须同步改道——mock 与被测代码双侧指向不同模块实例时，mock 会静默失效（readFileInventory 真实执行而非 mock），用例可能假绿或假红。该套件全真实链路（仅 stdin / tmpdir / getProjectDir 边界 mock），是记录器折叠、自污染过滤、agent_type 来源审计的权威回归面。另：该文件头注释引用旧路径 `bin/src/lib/file-inventory.ts`（非导入，不阻断编译），随改道顺带刷新。

#### 场景: 事件回放归账改道后全绿

前置：导入与 mock 路径双侧改道，其余零改动。输入：既有事件回放用例（stdin 事件序列 → 折叠 → 真盘 workflow.json）。预期：全部用例零断言改动通过，且 mockReadFileInventory 类命中路径与改道前一致（无 mock 失效迹象）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 工作流事件序列回放净状态 / session 隔离与换绑既有用例全绿 (AC-4) | 新增 |
| 边界 | agent_type 来源审计（source 旁挂维护）既有用例全绿：折叠写入的 source 与查询投影排除契约不冲突（查询侧契约见关系 2）(AC-4) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| stdin | 沿用既有 readFileSync fd 0 分支拦截（node:fs 与 fs 两种拼写） | 全部用例 |
| os.tmpdir / getProjectDir | 沿用既有隔离 mock（注册表落盘与工程根指向临时目录） | 全部用例 |

---

## 不可测试项

- AC-5（三个 evaluator md 的范围圈定切换）— **原因**: proposal/design 指定验证方式为仓库 grep（实现阶段任务：断言 md 含 `__MCP:workflow_files__`、不含「Read `workflow.json`」清单读取指令），属静态文本检查而非 vitest 可自动化的运行时行为；占位符与工具名一致性的运行时侧已由 mcp.test.ts 的 listTools 名称与 description 断言（AC-1）双重保障
- AC-6（version 提升与产物刷新）— **原因**: 属构建验证——build 产物目录（claude-plugins / cursor-plugins / cursor-home-image）由构建流程刷新，以 build 输出与 git status 确认，不在测试框架覆盖范围
- `plugins/dev-team/bin/src/schemas/workflow-files.schema.ts` 与 `plugins/dev-team/bin/src/schemas/index.ts` — **原因**: test_resolve_paths 返回错误「Not in test config scope」（config.json excludes `bin/src/schemas/**/*`，目录级排除）；schema 行为经 mcp.test.ts 直接导入断言（workflowFilesInputSchema safeParse）与 files-query / mcp 集成场景间接覆盖，不单独建测试文件
- 真实 agent 宿主经 MCP 客户端调用 workflow_files 的端到端体验 — **原因**: 依赖 Claude 宿主环境（工具权限、根候选确认 UI 流程），InMemoryTransport 已覆盖协议层接线，宿主侧行为无法进程内自动化
- evaluator 提示词「三态对账判定语义不变」的推理执行效果 — **原因**: 判定发生在 agent 推理过程中，提示词文本可由 AC-5 的 grep 静态检查，但语义执行质量无法用进程内测试断言，由实现后的 evaluator 评估阶段人工对账验证
