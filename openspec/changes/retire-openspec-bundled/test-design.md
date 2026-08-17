# 测试设计: retire-openspec-bundled

> **日期**: 2026-08-14

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|-------|----------|----------|---------------|
| AC-1 | change_create MCP 工具注册：`mcp.ts` 中注册名为 `change_create` 的工具，`inputSchema` 包含 `name` 字段，`outputSchema` 定义返回结构 | 集成测试 | `mcp.ts` → `change-create.ts` |
| AC-2 | change_create 创建目录和元数据：调用后在 `openspec/changes/<name>/` 创建目录，写入 `workflow.json` 包含 `workflow_type: "requirement"` 和 `created` 日期 | 单元测试、集成测试 | `plugins/dev-team/bin/src/commands/change-create.ts` |
| AC-3 | change_create 校验名称：非 kebab-case 名称返回错误，合法名称通过 | 单元测试 | `plugins/dev-team/bin/src/commands/change-create.ts` |
| AC-4 | change_create 拒绝已存在：对已存在的 change 名称返回错误，不覆盖或修改已存在目录 | 单元测试 | `plugins/dev-team/bin/src/commands/change-create.ts` |
| AC-5 | spec_list MCP 工具注册：`mcp.ts` 中注册名为 `spec_list` 的工具，返回 `openspec/specs/*/spec.md` 的 capability 列表 | 集成测试 | `mcp.ts` → `spec-list.ts` |
| AC-6 | change_list 返回 `workflow_done`：每个 entry 包含 `workflow_done: boolean`，正确反映所有 phase 完成状态 | 单元测试、集成测试 | `plugins/dev-team/bin/src/commands/change-list.ts` |
| AC-7 | openspec-bundled.js 从产物中移除：构建后 `claude-plugins/dev-team/bin/` 和 `cursor-plugins/dev-team/bin/` 中不存在该文件 | 集成测试 | `plugins/dev-team/build/assemble.ts` |
| AC-8 | bin/openspec 和 bin/openspec.cmd 从产物中移除：构建后产物目录中不存在这两个 wrapper 文件 | 集成测试 | `plugins/dev-team/build/assemble.ts` |
| AC-9 | 构建管线特判全部移除：`assemble.ts` 的 `STATIC_BIN_FILES`、`scan-files.ts` 的 `EXCLUDE_BASENAMES`、`vite.config.ts` 的 `NO_OXC_FILES`、`home-install.ts` 的特判不再引用 openspec-bundled | 单元测试、集成测试 | `plugins/dev-team/build/assemble.ts`、`plugins/dev-team/build/scan-files.ts` |
| AC-10 | proposal-planner 使用 spec_list MCP：`proposal-planner.md` 中不再引用 `source .../openspec-cli.sh`，改为 `__MCP:spec_list__` | 不可测试 | — |
| AC-11 | utils/openspec-cli.sh 被删除：文件 `plugins/dev-team/utils/openspec-cli.sh` 不存在 | 不可测试 | — |
| AC-12 | assert-no-tokens 测试通过：移除了 openspec-bundled.js 相关测试用例的测试套件全部通过 | 集成测试 | `plugins/dev-team/build/__tests__/assert-no-tokens.test.ts` |
| AC-13 | 英文 header parser 随 bundle 删除：产物中不存在 `MarkdownParser.parseChange`、`ChangeParser.parseChangeWithDeltas`、`ChangeSchema` 相关代码 | 集成测试 | `plugins/dev-team/build/assemble.ts` |

---

## 单元测试

### `plugins/dev-team/bin/src/commands/change-create.ts` → `plugins/dev-team/bin/src/commands/change-create.test.ts`

#### 待测功能

- `runChangeCreate(name, projectRoot)`: 校验 kebab-case 名称、创建 change 目录、写入 `workflow.json` 元数据。返回 `{ name, path }`。拒绝非法名称和已存在目录。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeCreate | 正向 | 合法 kebab-case 名称（如 `my-change`）创建目录并写入 `workflow.json`，文件包含 `workflow_type: "requirement"` 和 ISO 日期 `created` 字段 (AC-2) | 新增 |
| runChangeCreate | 正向 | 返回对象包含 `name` 和 `path` 字段，`path` 指向 `openspec/changes/<name>/` (AC-2) | 新增 |
| runChangeCreate | 异常 | 非 kebab-case 名称（如 `My Change`、`my_change`、`MyChange`）抛出错误或返回错误信息 (AC-3) | 新增 |
| runChangeCreate | 异常 | 已存在的 change 目录名称抛出错误，不覆盖已有目录或修改已有文件 (AC-4) | 新增 |
| runChangeCreate | 边界 | 名称仅单个单词（如 `fix`）应通过 kebab-case 校验 | 新增 |
| runChangeCreate | 边界 | 名称含数字（如 `fix-123`）应通过 kebab-case 校验 | 新增 |
| runChangeCreate | 边界 | 名称以 `-` 开头或结尾（如 `-fix`、`fix-`）应被拒绝 | 新增 |
| runChangeCreate | 边界 | 名称为空字符串时应被拒绝 | 新增 |
| runChangeCreate | 边界 | 名称超长（>100 字符）时应被拒绝或截断 | 新增 |
| runChangeCreate | 边界 | `projectRoot` 指向不存在路径时应明确失败，而非静默创建 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `fs.mkdtempSync` / `fs.mkdirSync` / `fs.writeFileSync` / `fs.rmSync` | 使用真实临时目录 (`mkdtempSync`) 创建 fixtures，测试后清理。不 mock 文件系统操作 | 全部场景 |
| `process.cwd()` | 用 `vi.spyOn(process, 'cwd').mockReturnValue(tempDir)` 控制当前工作目录 | 涉及 projectRoot 默认值的场景 |

---

### `plugins/dev-team/bin/src/commands/spec-list.ts` → `plugins/dev-team/bin/src/commands/spec-list.test.ts`

#### 待测功能

- `runSpecList(projectRoot)`: 扫描 `openspec/specs/*/spec.md`，返回 `{ specs: { name, path, description }[] }` 列表。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runSpecList | 正向 | 存在两个 spec 目录各含 `spec.md` 时，返回对应两条 capability 记录，包含 `name`、`path`、`description` 字段 | 新增 |
| runSpecList | 正向 | `description` 字段从 `spec.md` 的 `# <capability-name>` 后的第一段非空文本提取 | 新增 |
| runSpecList | 异常 | `openspec/specs/` 目录不存在时返回空数组 `{ specs: [] }`，不抛错 | 新增 |
| runSpecList | 异常 | `openspec/specs/` 存在但无 `spec.md` 文件时返回空数组 | 新增 |
| runSpecList | 边界 | 某个 spec 目录含 `spec.md` 但无 description 文本时，`description` 为 `""` 或 `null` | 新增 |
| runSpecList | 边界 | 非 spec.md 文件（如 `readme.md`、`notes.txt`）被忽略 | 新增 |
| runSpecList | 边界 | 深层嵌套目录（`openspec/specs/foo/bar/spec.md`）不被扫描（仅 `openspec/specs/*/spec.md` 模式） | 新增 |
| runSpecList | 边界 | `projectRoot` 为超长路径或含 emoji 时不崩溃 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `fs` 文件系统 | 使用真实临时目录创建 `openspec/specs/*/spec.md` fixture，测试后清理 | 全部场景 |

---

### `plugins/dev-team/bin/src/commands/change-list.ts` → `plugins/dev-team/bin/src/commands/change-list.test.ts`

#### 待测功能

- `runChangeList(projectRoot)`: 返回活跃 change 列表，每个 entry 包含 `name`、`artifacts`、`tasks`、`latest_phase`、`workflow_done` 字段。
- `workflow_done` 计算逻辑（新增内部函数 `computeWorkflowDone`）：读 `workflow.json` → `getPhaseTable` → `eval.json` → `hasPhasePassed`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeList workflow_done | 正向 | 所有 phase 都有非 stale 的 pass/skipped 条目时，`workflow_done` 为 `true` (AC-6) | 新增 |
| runChangeList workflow_done | 正向 | 任意 phase 缺少 pass 条目时，`workflow_done` 为 `false` (AC-6) | 新增 |
| runChangeList workflow_done | 正向 | 无 `eval.json` 的 change，`workflow_done` 为 `false` (AC-6) | 新增 |
| runChangeList workflow_done | 异常 | `workflow.json` 缺失时，`workflow_done` 为 `false`（无法确定 workflow_type） | 新增 |
| runChangeList workflow_done | 异常 | `eval.json` 格式非法时，`workflow_done` 为 `false` 而非崩溃 | 新增 |
| runChangeList workflow_done | 边界 | 所有 phase 的 pass 均为 stale 时，`workflow_done` 为 `false` | 新增 |
| runChangeList workflow_done | 边界 | 仅 acceptance phase 有 pass 但其他 phase 缺失时，`workflow_done` 为 `false` | 新增 |
| runChangeList workflow_done | 边界 | skipped 条目视为 pass（`skipped: true` 且非 stale）归入 `workflow_done` 计算 | 新增 |
| runChangeList workflow_done | 边界 | `workflow.json` 的 `workflow_type` 为 `bug-fix` 或 `test-only` 时，使用对应 phase table 计算 | 新增 |
| runChangeList workflow_done | 边界 | `workflow_done` 字段类型严格为 `boolean`，`changeListOutputSchema` 应验证 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `fs` 文件系统 | 使用真实临时目录创建 `openspec/changes/<name>/` + `workflow.json` + `eval.json` fixture | 全部场景 |
| `hasPhasePassed` | 不 mock — 直接复用 `phase-next.ts` 的导出函数，验证真实行为 | 全部场景 |

---

### `plugins/dev-team/bin/src/commands/phase-next.ts` → `plugins/dev-team/bin/src/commands/phase-next.test.ts`

#### 待测功能

- `hasPhasePassed(entries, phaseId)`: 判断某 phase 是否有非 stale 的 pass/skipped 条目。从 `function` 改为 `export function`（仅 export 变更，行为不变）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| hasPhasePassed | 正向 | 已有非 stale 的 pass 条目时返回 `true` | 新增 |
| hasPhasePassed | 正向 | 已有非 stale 的 skipped 条目时返回 `true` | 新增 |
| hasPhasePassed | 异常 | 仅有 fail 条目时返回 `false` | 新增 |
| hasPhasePassed | 异常 | 仅有 stale 的 pass 条目时返回 `false` | 新增 |
| hasPhasePassed | 边界 | 空 entries 数组返回 `false` | 新增 |
| hasPhasePassed | 边界 | 混合 stale + 非 stale 条目时，非 stale 通过则返回 `true` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数，无外部依赖，无需 mock | 全部场景 |

---

### `plugins/dev-team/build/assemble.ts` → `plugins/dev-team/build/assemble.test.ts`

#### 待测功能

- `assembleAll()`: 执行构建管线 assemble 阶段，包括 `copyStaticAssets`、`writeHomeExtras` 等。本变更清理 `STATIC_BIN_FILES` 和 `writeHomeExtras` 中的 openspec 引用。
- `copyStaticAssets(env)`: 内部函数，遍历 `STATIC_BIN_FILES` 复制静态资产。缺失文件通过 `existsSync` 检查后跳过，不抛出错误。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| STATIC_BIN_FILES 常量 | 正向 | `STATIC_BIN_FILES` 不再包含 `'bin/openspec'`、`'bin/openspec-bundled.js'`、`'bin/openspec.cmd'` (AC-9) | 新增 |
| STATIC_BIN_FILES 常量 | 边界 | `STATIC_BIN_FILES` 数组长度减少 3（从 N 到 N-3） | 新增 |
| `writeHomeExtras` 的 `extraManaged` | 正向 | `extraManaged` 数组不再包含 `'bin/openspec'`、`'bin/openspec-bundled.js'`、`'bin/openspec.cmd'` (AC-9) | 新增 |
| assembleAll | 异常 | 缺少 `.pack-staging/bin` 目录时 `readdirSync` 抛出 ENOENT，`assembleAll()` 拒绝而非静默部分执行 | 新增 |
| assembleAll | 异常 | 缺少 `hooks/hooks.canonical.json` 时 `readFileSync` 抛出 ENOENT，`assembleAll()` 拒绝而非静默部分执行 | 新增 |
| assembleAll | 异常 | `process.cwd()` 指向不存在路径时下级 `readdirSync`/`cpSync` 调用抛出 ENOENT | 新增 |
| assembleAll | 边界 | 空 `.pack-staging/bin` 目录（无任何 staging 产物）时 `copyStagingBins` 不复制任何文件，`assembleAll()` 仍然完成 | 新增 |
| assembleAll | 边界 | 无 `skills/` 和 `agents/` 目录时 `copySkills`/`copyAgents` 返回空数组，`assembleAll()` 仍然完成 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `STATIC_BIN_FILES` | 模块级常量，非 export。通过 `vi.importActual` 或直接 import 模块后读取常量值 | 常量验证场景 |
| `fs` 文件系统 | 使用 `mkdtempSync` 临时目录创建 fixture 产物目录，调用 `assembleAll()` 后验证产物文件列表 | 集成验证场景 |
| `process.cwd()` | `vi.spyOn(process, 'cwd').mockReturnValue(tempDir)` 控制根目录指向可预置 fixture 的临时路径 | 异常场景（缺失目录模拟） |
| `.pack-staging/` 目录结构 | 在临时目录中按需创建/省略 staging 子目录和 hooks 文件，控制 `assembleAll()` 的输入条件 | 异常/边界场景 |

---

### `plugins/dev-team/build/scan-files.ts` → `plugins/dev-team/build/scan-files.test.ts`

#### 待测功能

- `scanTextFiles(rootDir)`: 扫描目录中的文本文件，排除 `EXCLUDE_BASENAMES` 中的文件。本变更从 `EXCLUDE_BASENAMES` 移除 `'openspec-bundled.js'`。参数 `rootDir` 为字符串类型，调用 `statSync` 后递归遍历。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| EXCLUDE_BASENAMES 常量 | 正向 | `EXCLUDE_BASENAMES` 不再包含 `'openspec-bundled.js'` (AC-9) | 新增 |
| scanTextFiles | 正向 | 包含 `openspec-bundled.js` 的目录不再被排除，`scanTextFiles` 返回结果中包含该文件 | 新增 |
| scanTextFiles | 异常 | `rootDir` 为空字符串 `""` 时 `statSync` 抛出 ENOENT，`scanTextFiles` 不吞没错误 | 新增 |
| scanTextFiles | 异常 | `rootDir` 为不存在路径时 `statSync` 抛出 ENOENT，`scanTextFiles` 不吞没错误 | 新增 |
| scanTextFiles | 异常 | `rootDir` 指向文件而非目录时 `statSync` 返回 `isDirectory() === false`，`scanTextFiles` 返回空数组 `[]`（不抛错） | 新增 |
| scanTextFiles | 边界 | `EXCLUDE_BASENAMES` set 仅包含 `'dev-team-config.schema.json'` 等必要排除项 | 新增 |
| scanTextFiles | 边界 | `rootDir` 为空目录时返回空数组 `[]` | 新增 |
| scanTextFiles | 边界 | `rootDir` 含特殊字符（emoji、unicode 非 ASCII 字符）时 `statSync` 按 OS 路径编码规则处理，若路径存在则正常扫描，不存在则抛出 ENOENT | 新增 |
| scanTextFiles | 边界 | `rootDir` 为超长路径（>260 字符，Windows MAX_PATH）时可能抛出 ENAMETOOLONG/EPERM，`scanTextFiles` 不吞没异常 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `EXCLUDE_BASENAMES` | 模块级常量，非 export。通过 `vi.importActual` 或直接 import 模块后读取常量值 | 常量验证场景 |
| `fs` 文件系统 | 使用 `mkdtempSync` 临时目录创建测试文件；对异常路径使用明知不存在的路径字符串 | 全部场景 |
| `statSync` | 不需要 mock — 异常路径由真实 `node:fs` 行为触发，无需干预 | 异常场景 |

---

## 集成测试

### MCP注册 → change_create handler 接线 → `plugins/dev-team/bin/src/mcp.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/mcp.ts` | MCP 工具注册方，定义 `MCP_TOOLS` 数组 |
| `plugins/dev-team/bin/src/commands/change-create.ts` | handler 实现方，导出 `runChangeCreate` |
| `plugins/dev-team/bin/src/schemas/change-create.schema.ts` | input/output schema 定义方 |

**关联AC**: AC-1

**关系描述**:

`mcp.ts` 在其 `MCP_TOOLS` 数组中新增 `change_create` 条目，指定 `name: 'change_create'`、`inputSchema`（引用 `changeCreateInputSchema`）、`outputSchema`（引用 `changeCreateOutputSchema`）和 `handler`（调用 `runChangeCreate`）。集成测试通过 `InMemoryTransport` 启动完整 MCP 服务器，验证 `listTools` 返回 `change_create` 工具，`callTool` 正确分发到 `runChangeCreate` 并返回结构化结果。可能出错模式：注册名拼写错误、schema 与 handler 不匹配、handler 参数提取错误。

#### 场景: change_create 工具注册与分发

使用 `InMemoryTransport` 创建完整的 MCP client-server 对。验证 `change_create` 出现在 `listTools` 结果中，`callTool` 调用 `change_create` 时 `runChangeCreate` 被正确调用且参数正确传递。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `listTools()` 返回的 tool names 中包含 `'change_create'`；`EXPECTED_TOOL_NAMES` 从 12 扩展到 14 (AC-1) | 新增 |
| 正向 | `change_create` 的 `inputSchema` 包含 `name` 字段（`z.string()`）(AC-1) | 新增 |
| 正向 | `callTool({ name: 'change_create', arguments: { name: 'my-change', project_root: dir } })` 调用后 `runChangeCreate` spy 被调用且参数正确 (AC-1) | 新增 |
| 正向 | `callTool` 返回非 `isError` 结果，包含 `name` 和 `path` 字段 | 新增 |
| 异常 | 省略 `name` 参数时 Zod 校验失败返回 `isError`，`runChangeCreate` 不被调用 | 新增 |
| 边界 | `name` 为空字符串时 Zod 校验失败返回 `isError` | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@modelcontextprotocol/sdk` | 使用 `InMemoryTransport.createLinkedPair()` 创建内存传输通道，无需真实网络 | 全部场景 |
| `runChangeCreate` | `vi.spyOn(changeCreateCmd, 'runChangeCreate')` 验证调用次数和参数，可 mock 返回值避免真实文件系统 I/O | 分发验证场景 |
| 文件系统 | 使用 `mkdtempSync` 临时目录作为 `project_root`，测试后清理 | 真实调用场景 |

---

### MCP注册 → spec_list handler 接线 → `plugins/dev-team/bin/src/mcp.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/mcp.ts` | MCP 工具注册方 |
| `plugins/dev-team/bin/src/commands/spec-list.ts` | handler 实现方，导出 `runSpecList` |
| `plugins/dev-team/bin/src/schemas/spec-list.schema.ts` | input/output schema 定义方 |

**关联AC**: AC-5

**关系描述**:

`mcp.ts` 在 `MCP_TOOLS` 数组中新增 `spec_list` 条目，指向 `runSpecList` handler。集成测试验证 `listTools` 返回 `spec_list` 工具，`callTool` 正确调用 `runSpecList` 并返回 capability 列表。可能出错模式：`project_root` 参数未正确传递导致扫描目录错误。

#### 场景: spec_list 工具注册与分发

使用 `InMemoryTransport` 创建 MCP client-server 对，验证 `spec_list` 注册和 handler 分发。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `listTools()` 返回的 tool names 中包含 `'spec_list'` (AC-5) | 新增 |
| 正向 | `spec_list` 的 `inputSchema` 包含 `project_root` 字段 | 新增 |
| 正向 | `callTool({ name: 'spec_list', arguments: { project_root: dir } })` 调用后 `runSpecList` spy 被调用且参数正确 (AC-5) | 新增 |
| 异常 | 省略 `project_root` 时 Zod 校验失败返回 `isError`，`runSpecList` 不被调用 | 新增 |
| 正向 | 所有 14 个 tool 在同一 MCP server 中各 `callTool` 一次均成功（含新增的 `change_create` 和 `spec_list`） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@modelcontextprotocol/sdk` | 使用 `InMemoryTransport.createLinkedPair()` | 全部场景 |
| `runSpecList` | `vi.spyOn(specListCmd, 'runSpecList')` 验证调用次数和参数 | 分发验证场景 |

---

### MCP注册 → change_list workflow_done 字段扩展 → `plugins/dev-team/bin/src/mcp.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/mcp.ts` | MCP 工具定义方，注册 `change_list` |
| `plugins/dev-team/bin/src/commands/change-list.ts` | handler 实现方，新增 `workflow_done` 计算 |
| `plugins/dev-team/bin/src/commands/phase-next.ts` | 被依赖方，导出 `hasPhasePassed` 供复用 |
| `plugins/dev-team/bin/src/lib/workflow.ts` | 被依赖方，提供 `getPhaseTable` |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 被依赖方，提供 `readEvalJson` |

**关联AC**: AC-6

**关系描述**:

`change_list` MCP 工具返回的每个 entry 新增 `workflow_done` 字段。计算链路：读 `workflow.json` 获取 `workflow_type` → 调用 `getPhaseTable` 获取 phase 列表 → 读 `eval.json` 获取评估条目 → 调用 `hasPhasePassed` 判断所有 phase 是否通过。集成测试验证 `callTool({ name: 'change_list' })` 返回的 JSON 中每个 entry 包含 `workflow_done` 且值正确。可能出错模式：`workflow_done` 字段缺失、类型非布尔、与 `hasPhasePassed` 语义不一致。

#### 场景: change_list 返回 workflow_done 字段

创建包含 `workflow.json` 和 `eval.json` 的临时 change 目录，通过 MCP client 调用 `change_list`，验证返回 JSON 的 `workflow_done` 字段。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 所有 phase 有非 stale pass 的 change，`workflow_done` 为 `true` (AC-6) | 新增 |
| 正向 | 缺少 pass 的 change，`workflow_done` 为 `false` (AC-6) | 新增 |
| 正向 | `workflow_done` 字段类型为 `boolean`，`changeListOutputSchema` 解析通过 | 新增 |
| 异常 | `eval.json` 缺失时 `workflow_done` 为 `false`，不崩溃 | 新增 |
| 边界 | 空 `eval.json`（`[]`）时 `workflow_done` 为 `false` | 新增 |
| 边界 | 无 `workflow.json` 时 `workflow_done` 为 `false` | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@modelcontextprotocol/sdk` | 使用 `InMemoryTransport.createLinkedPair()` | 全部场景 |
| 文件系统 | 使用 `mkdtempSync` 临时目录创建完整 change fixture | 全部场景 |

---

### change_create → 文件系统持久化 → `plugins/dev-team/bin/src/commands/change-create.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/change-create.ts` | 写入方，执行 mkdir + writeFile |
| `openspec/changes/<name>/` 目录 | 数据存储方，接收目录和文件创建 |
| `node:fs` | 系统调用方，提供 `mkdirSync` / `writeFileSync` |

**关联AC**: AC-2, AC-3, AC-4

**关系描述**:

`runChangeCreate` 在真实文件系统上创建目录和写入 `workflow.json`。集成测试验证调用后目录结构符合预期，文件内容正确，重复调用和非法名称不会产生副作用。可能出错模式：目录创建成功但写入失败、文件写入内容不完整、并发创建竞争条件。

#### 场景: 目录创建与元数据写入

使用 `mkdtempSync` 创建临时项目根，调用 `runChangeCreate` 后验证文件系统状态。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 调用后 `openspec/changes/<name>/` 目录存在 (AC-2) | 新增 |
| 正向 | 目录下 `workflow.json` 文件存在，内容为 `{"workflow_type": "requirement", "created": "YYYY-MM-DD"}` (AC-2) | 新增 |
| 正向 | `created` 日期格式为 ISO 日期（`YYYY-MM-DD`），非时间戳 | 新增 |
| 正向 | 返回对象 `path` 字段等于创建的目录绝对路径 (AC-2) | 新增 |
| 异常 | 非 kebab-case 名称不创建任何目录或文件 (AC-3) | 新增 |
| 异常 | 已存在名称不覆盖或修改已有目录内容 (AC-4) | 新增 |
| 边界 | `openspec/changes/` 目录不存在时自动创建（递归 mkdir） | 新增 |
| 边界 | 名称含数字前缀（如 `123-fix`）通过 kebab-case 校验 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `node:fs` | 不 mock — 使用真实临时目录验证文件系统行为 | 全部场景 |

---

### 构建管线清理 → 产物验证 → `plugins/dev-team/build/__tests__/assemble-cleanup.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/build/assemble.ts` | 构建管线核心，管理 `STATIC_BIN_FILES` 和 `writeHomeExtras` |
| `plugins/dev-team/build/scan-files.ts` | 文件扫描方，管理 `EXCLUDE_BASENAMES` |
| `plugins/dev-team/vite.config.ts` | 构建配置方，管理 `NO_OXC_FILES` |
| `plugins/dev-team/home-install.ts` | 安装脚本方，管理 token 展开特判 |
| 产物目录（`claude-plugins/`、`cursor-plugins/`） | 数据接收方，验证文件不存在 |

**关联AC**: AC-7, AC-8, AC-9, AC-13

**关系描述**:

构建管线 assembly 阶段移除 `openspec-bundled.js`、`bin/openspec`、`bin/openspec.cmd` 的静态资产引用和特判。集成测试在临时目录中模拟组装过程，验证产物目录中不再包含这些文件，且 `scanTextFiles` 不再排除 `openspec-bundled.js`。可能出错模式：移除遗漏导致残留文件、清理过度影响其他静态资产。

#### 场景: 模拟组装后产物文件验证

使用 `mkdtempSync` 创建临时项目根，模拟 `assembleAll` 的静态资产复制行为（或直接调用 `assembleAll` 的受控版本），验证产物目录。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 模拟组装后，产物目录中不存在 `openspec-bundled.js` (AC-7) | 新增 |
| 正向 | 模拟组装后，产物目录中不存在 `bin/openspec` (AC-8) | 新增 |
| 正向 | 模拟组装后，产物目录中不存在 `bin/openspec.cmd` (AC-8) | 新增 |
| 正向 | `STATIC_BIN_FILES` 数组不再包含 openspec 相关条目 (AC-9) | 新增 |
| 正向 | `EXCLUDE_BASENAMES` 不再包含 `'openspec-bundled.js'` (AC-9) | 新增 |
| 正向 | `NO_OXC_FILES` 不再包含 `'bin/openspec-bundled.js'` (AC-9) | 新增 |
| 正向 | `home-install.ts` 的 `writeExpanded` 不再有 `base === 'openspec-bundled.js'` 特判 (AC-9) | 新增 |
| 边界 | 其他静态资产（如 `bin/dev-team-config.schema.json`）仍被正确复制，不受影响 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `node:fs` | 使用 `mkdtempSync` 临时目录模拟产物目录 | 全部场景 |
| `assembleAll` 调用 | 可使用 `vi.importActual` 读取模块常量，无需实际运行完整 assemble 流程 | 静态常量验证场景 |

---

### 构建测试更新 → assert-no-tokens 和 scan-files 测试 → `plugins/dev-team/build/__tests__/assert-no-tokens.test.ts`、`plugins/dev-team/build/__tests__/scan-files.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/build/assert-no-tokens.ts` | token 校验方，`assertNoNameTokens` 函数 |
| `plugins/dev-team/build/scan-files.ts` | 文件扫描方，`scanTextFiles` 函数 |
| `plugins/dev-team/build/__tests__/assert-no-tokens.test.ts` | 测试文件，验证无残留 token |
| `plugins/dev-team/build/__tests__/scan-files.test.ts` | 测试文件，验证文件排除行为 |

**关联AC**: AC-12, AC-9

**关系描述**:

`assert-no-tokens.test.ts` 移除 `openspec-bundled.js` 的 token 白名单测试用例，`scan-files.test.ts` 移除「排除 openspec-bundled.js」测试用例。集成测试验证更新后的测试套件通过，且不再引用已删除的 openspec 相关文件。可能出错模式：移除不完整导致残留测试用例引用已删除文件。

#### 场景: assert-no-tokens 测试更新

创建含 `openspec-bundled.js` 相关 token 的测试文件，验证 `assertNoNameTokens` 不再豁免该文件。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `assertNoNameTokens` 对包含 `openspec-bundled.js` 的目录不再有白名单，检测到 token 时报错 (AC-12) | 新增 |
| 正向 | 移除 openspec-bundled.js 相关测试用例后，剩余测试用例全部通过 (AC-12) | 新增 |
| 边界 | 无 token 的目录调用 `assertNoNameTokens` 不抛错 | 新增 |

#### 场景: scan-files 测试更新

验证 `scanTextFiles` 不再排除 `openspec-bundled.js`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 包含 `openspec-bundled.js` 的目录中，`scanTextFiles` 返回结果包含该文件名 (AC-9) | 新增 |
| 正向 | 移除「排除 openspec-bundled.js」测试用例后，剩余测试用例全部通过 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `node:fs` | 使用 `mkdtempSync` 临时目录创建测试文件 | 全部场景 |

---

## 不可测试项

- **AC-10 (proposal-planner 使用 spec_list MCP)** — 变更涉及 `proposal-planner.md` 的 markdown 文本替换，无可执行代码逻辑。MCP 工具的注册和功能已在集成测试中验证，markdown 文档本身不产生可测试的运行时行为。
- **AC-11 (utils/openspec-cli.sh 被删除)** — 文件删除操作无法通过自动化测试验证。可在构建产物验证中附带检查该文件不在仓库中，但属代码审查范畴。
- **AC-13 (英文 header parser 随 bundle 删除)** — `MarkdownParser.parseChange`、`ChangeParser.parseChangeWithDeltas`、`ChangeSchema` 是 `openspec-bundled.js` 的内部代码，随 bundle 删除自然消失。AC-7 的产物验证已覆盖此场景，无需独立测试。
- **`plugins/dev-team/bin/src/schemas/change-create.schema.ts`** — 配置文件排除在单元测试范围之外（`config.json` 的 `tests[].excludes` 包含 `"bin/src/schemas/**/*"`）。Schema 的验证行为通过 `mcp.test.ts` 的 `inputSchema` 校验和 `callTool` 参数验证间接覆盖。
- **`plugins/dev-team/bin/src/schemas/spec-list.schema.ts`** — 同上，配置文件排除在单元测试范围之外。间接通过 `mcp.test.ts` 覆盖。
- **`plugins/dev-team/bin/src/schemas/change-list.schema.ts`** — 同上，配置文件排除在单元测试范围之外。`workflow_done` 字段验证通过 `change-list.test.ts` 的 `changeListOutputSchema.safeParse` 覆盖。
- **`plugins/dev-team/home-install.ts`** — 配置文件排除在单元测试范围之外（`config.json` 的 `tests[].excludes` 包含 `"home-install.ts"`）。`writeExpanded` 中的特判清理通过 `assemble-cleanup.test.ts` 的代码审查断言覆盖。
- **`plugins/dev-team/vite.config.ts`** — Vite 配置文件，非可测试模块。`NO_OXC_FILES` 中的 `'bin/openspec-bundled.js'` 移除通过 `assemble-cleanup.test.ts` 的静态断言覆盖。
- **`plugins/dev-team/skills/openspec-archive-change/SKILL.md`** — markdown 文档变更，无可执行代码逻辑。`__MCP:phase_check__` 幽灵引用移除和 `openspec status --json` 改为 `workflow_done` 字段属 agent 指令变更，非运行时行为。
- **`plugins/dev-team/bin/openspec`** — 删除的 bash wrapper 文件，不可测试。AC-8 的产物验证覆盖其不存在。
- **`plugins/dev-team/bin/openspec.cmd`** — 删除的 Windows wrapper 文件，不可测试。AC-8 的产物验证覆盖其不存在。