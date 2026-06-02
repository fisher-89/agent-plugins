# 设计: config-json-zod-schema

> **变更**: config-json-zod-schema
> **日期**: 2026-06-02
> **基于**: proposal.md, specs/ 目录下的 5 份规范文档

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Config Schema | 定义 `config.json` 的 Zod schema，提供运行时校验和 TypeScript 类型推导 | `plugins/dev-team/bin/src/schemas/config.schema.ts` | zod/v4 | TypeScript, Zod |
| Config Library | 封装配置文件读写、点号路径操作、自动迁移逻辑 | `plugins/dev-team/bin/src/lib/config.ts` | fs, path, config.schema | TypeScript |
| config/context Command | 读写 `config.json` 中的 `context` 字段 | `plugins/dev-team/bin/src/commands/config-context.ts` | lib/config | TypeScript |
| config/get Command | 通过点号路径读取 `config.json` 中的值 | `plugins/dev-team/bin/src/commands/config-get.ts` | lib/config | TypeScript |
| config/set Command | 通过点号路径向 `config.json` 写入值 | `plugins/dev-team/bin/src/commands/config-set.ts` | lib/config | TypeScript |
| config/unset Command | 通过点号路径删除 `config.json` 中的值 | `plugins/dev-team/bin/src/commands/config-unset.ts` | lib/config | TypeScript |
| MCP Server | 注册所有 MCP 工具，处理输入/输出 schema 校验 | `plugins/dev-team/bin/src/mcp.ts` | MCP SDK, schemas/*, commands/* | TypeScript, MCP SDK |
| MCP Input/Output Schemas | 各工具的 Zod input/output schema 定义 | `plugins/dev-team/bin/src/schemas/config-{get,set,unset,context}.schema.ts` | zod/v4 | TypeScript, Zod |

### 组件图

```
+-------------------+          +-------------------+
|   MCP Server      | -------> | Command Layer     |
|   (mcp.ts)        |          | (config-context,  |
|                   |          |  config-get,      |
|                   |          |  config-set,      |
|                   |          |  config-unset)    |
+-------------------+          +--------+----------+
                                         |
                                         v
                               +---------+----------+
                               |  Config Library    |
                               |  (lib/config.ts)   |
                               |                    |
                               |  readConfig()      |
                               |  writeConfig()     |
                               |  ensureConfigFile()|
                               |  getValue()        |
                               |  setValue()        |
                               |  unsetValue()      |
                               +---------+----------+
                                         |
                              +----------+----------+
                              |  Config Schema      |
                              |  (schemas/config.   |
                              |   schema.ts)        |
                              |                     |
                              |  configSchema       |
                              |  parseConfig()      |
                              |  safeParseConfig()  |
                              |  OpenSpecConfig     |
                              +---------------------+
```

---

## 数据流

### 流程描述

#### 读流程 (config/get, config/context read)

1. MCP 工具收到请求，解析输入参数（`key`、`project_root` 等）
2. `resolveProjectRoot()` 确定项目根目录
3. `readConfig(projectRoot)` 读取 `<projectRoot>/openspec/config.json`：
   - 如果文件不存在，返回空对象 `{}`
   - 如果文件存在，使用 `fs.readFileSync` + `JSON.parse` 读取
   - 使用 `safeParseConfig()` 进行 Zod schema 校验（对于 config/get 采用优雅降级，校验失败仍返回原始数据）
4. `getValue(config, keyPath)` 按点号分隔路径遍历对象
5. 返回结果对象（`{ key, value?, exists }` 或 `{ context }`）

#### 写流程 (config/set, config/context write)

1. MCP 工具收到请求，解析输入参数
2. `resolveProjectRoot()` 确定项目根目录
3. `ensureConfigFile(projectRoot)` 确保文件存在（不存在则创建 skeleton）
4. `readConfig(projectRoot)` 读取当前配置
5. 对于 config/set：`setValue(config, keyPath, value)` 设置值（不再使用 `inferValue`）
6. 对于 config/context：直接修改 `config.context` 字段
7. **写入前**：使用 `parseConfig()` 校验完整配置对象的合法性
8. 校验通过后，使用 `JSON.stringify(data, null, 2)` 写入文件

#### YAML 自动迁移流程

1. 调用 `readConfig()` 时，检测 `config.json` 是否存在
2. 如果 `config.json` 不存在但 `config.yaml` 存在：
   - 读取并解析 `config.yaml`（使用 `yaml` 包）
   - 将解析后的数据写入 `config.json`（通过 `parseConfig()` 校验）
   - 删除 `config.yaml` 文件
   - 返回类型化配置对象
3. 如果两者都不存在，返回空对象

### 数据模型

#### 配置文件: `openspec/config.json`

```json
{
  "schema": "spec-driven",
  "context": "Tech stack: TypeScript, Node.js",
  "rules": {
    "proposal": ["Keep it under 500 words"],
    "tasks": ["One file per task"]
  },
  "static_check": ["npm run check", "npm test"],
  "test_scripts": {
    "unit": "npm run test:unit",
    "integration": "npm run test:integration"
  }
}
```

#### TypeScript 类型 (由 Zod schema 推导)

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `OpenSpecConfig` | `schema: "spec-driven"` (字面量, 默认值) | 顶级字段 | `openspec/config.json` |
|  | `context?: string` | 顶级可选字段 | `openspec/config.json` |
|  | `rules?: { proposal?: string[]; tasks?: string[] }` | 顶级可选嵌套对象 | `openspec/config.json` |
|  | `[key: string]: unknown` (`.passthrough()`) | 额外未知字段 | `openspec/config.json` |

#### 核心函数签名变更

| 函数 | 旧签名 (YAML 版本) | 新签名 (JSON + Zod 版本) |
|------|---------------------|--------------------------|
| `readConfig` | `(projectRoot: string): Record<string, unknown>` | `(projectRoot: string): OpenSpecConfig` |
| `writeConfig` | `(projectRoot: string, data: Record<string, unknown>): void` | `(projectRoot: string, data: OpenSpecConfig): void` |
| `ensureConfigFile` | `(projectRoot: string): Record<string, unknown>` | `(projectRoot: string): OpenSpecConfig` |
| `getValue` | `(config: Record<string, unknown>, keyPath: string): { value: unknown; exists: boolean }` | 签名不变（操作对象类型随输入类型提升） |
| `setValue` | `(config: Record<string, unknown>, keyPath: string, value: unknown): Record<string, unknown>` | 签名不变 |
| `unsetValue` | `(config: Record<string, unknown>, keyPath: string): { config: Record<string, unknown>; removed: boolean }` | 签名不变 |
| `inferValue` | 存在 | **已移除** |

---

## 路由/API 设计

MCP 工具接口（输入/输出 schema）保持不变，仅内部实现变更。

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP tool | `config/context` | 读取或写入 `config.json` 中的 `context` 字段 | `{ context?: string, project_root?: string }` | `{ context: string, written?: boolean }` | 无 |
| MCP tool | `config/get` | 读取 `config.json` 中指定键的值 | `{ key: string, project_root?: string }` | `{ key: string, value?: unknown, exists: boolean }` | 无 |
| MCP tool | `config/set` | 向 `config.json` 写入指定键的值 | `{ key: string, value: unknown, project_root?: string }` | `{ key: string, written: boolean }` | 无 |
| MCP tool | `config/unset` | 从 `config.json` 删除指定键 | `{ key: string, project_root?: string }` | `{ key: string, removed: boolean }` | 无 |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 使用 JSON 格式替代 YAML 格式 | JSON 解析/序列化使用 Node.js 原生 `JSON.parse`/`JSON.stringify`，常规读写无需外部库。`yaml` 包保留为运行时依赖但仅通过 `getYamlParser()` 动态加载用于自动迁移路径。Zod schema 提供运行时校验和 IDE 自动补全。向后兼容通过自动迁移实现 | **保留 YAML**：需要继续在常规路径中加载 `yaml` 包（~200KB bundle），无法享受原生 JSON 的性能优势和零常规依赖。**改为 TOML**：TOML 同样需要第三方库解析，生态系统支持不如 JSON 广泛 |
| D2 | 使用 `zod/v4` 而非 `zod/v3` 定义 schema | 项目中已存在 `zod/v4` 依赖（版本 `^4.4.3`），与现有 MCP 工具的 schema 定义保持一致。v4 提供了更好的类型推导和性能 | **zod/v3**：项目中未使用，需要额外添加兼容层。**JSON Schema + ajv**：需要引入 `ajv` 包作为验证器。**TypeScript interface only**：仅在编译时提供类型检查，缺少运行时校验 |
| D3 | 使用 `.passthrough()` 允许额外未知字段 | 配置文件由工具管理，可能包含工具特有的扩展字段。允许未知字段可以避免工具写入额外字段后导致校验失败。与现有 `config.yaml` 的行为一致（YAML 天然接受任意键） | **`.strict()`**：拒绝未知字段，会在工具写入自定义字段时触发错误。**`.strip()`**：静默删除未知字段，可能导致数据丢失 |
| D4 | 移除 `inferValue()` 函数 | `inferValue()` 的启发式类型推断（`"true"` -> boolean, `"42"` -> number 等）行为不可预测。JSON 格式天然支持类型区分（字符串带引号 vs 数字/布尔值无引号），不再需要字符串级别推断。这是该变更带来的本质改进 | **保留 `inferValue()`**：继续维护不可靠的推断逻辑，增加测试和维护成本 |
| D5 | `config/get` 使用 `safeParseConfig` 优雅降级 | `config/get` 是只读操作，即便配置文件结构不符合 schema（例如混入非标准字段），用户仍应能读取到值。优雅降级避免因校验失败而无法读取配置。这是与旧 YAML 版本一致的行为 | **使用 `parseConfig` 严格校验**：一旦 config.json 包含不合法字段则拒绝读取，导致用户无法修复配置。**不进行任何校验**：失去类型安全优势 |
| D6 | `config/set` 使用 `parseConfig` 严格校验，写入前拒绝非法数据 | `config/set` 是写操作，应当确保写入磁盘的数据是合法的。严格校验可以尽早捕获拼写错误或类型错误的数据，避免写入垃圾数据后难以排查 | **跳过校验直接写入**：可能导致非法的配置数据被写入文件。**使用 `safeParseConfig` 并静默修复**：静默修改用户数据可能违反用户预期 |
| D7 | 自动迁移：检测 config.yaml 存在时读取并转换为 JSON，然后删除 YAML | 已有项目可能依赖 `config.yaml`。自动迁移确保零人工干预即可升级到新格式。迁移后删除 `.yaml` 避免双文件混淆 | **仅读取 YAML 不转换**：每次启动都需要读取 YAML，无法利用 JSON 的优势。**要求用户手动迁移**：增加用户升级负担，可能造成配置丢失 |
| D8 | JSON 序列化使用 2 空格缩进 | 2 空格是 Node.js 生态系统中最常见的 JSON 格式化约定（如 `tsconfig.json`、`package.json` 的默认格式化）。可读性好且节省空间 | **4 空格缩进**：浪费空间。**无缩进（最小化）**：不可读，不适合手动编辑。**保留原格式**：`JSON.stringify` 不支持保留原格式，需要额外 diff 逻辑 |

---

## 依赖

### 运行时依赖

- `zod` (`^4.4.3`) — 用于定义配置 schema、运行时校验和 TypeScript 类型推导（已有依赖，不变）
- `@modelcontextprotocol/sdk` (`^1.29.0`) — MCP 服务器框架（已有依赖，不变）
- `yaml` (`^2.7.1`) — 保留为运行时依赖，但仅通过 `getYamlParser()` 动态 `require('yaml')` 在自动迁移代码路径中加载。常规 JSON 读写操作不加载此包。待所有项目完成迁移后可考虑清理此依赖

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 已有项目存在 `config.yaml`，升级后配置丢失 | 已有项目配置不可读，工具行为异常 | 高 | 实现自动迁移：检测 YAML 存在时读取并转换为 JSON，然后删除 YAML。记录迁移日志 |
| Zod schema 过于严格导致合法配置被拒绝 | 工具不可用 | 中 | schema 定义使用 `.passthrough()` 和 `.optional()` 宽松策略。`config/get` 使用 `safeParseConfig` 优雅降级 |
| `yaml` 包的动态 `require()` 在特定运行时环境（如打包后）失败 | 自动迁移路径不可用，已有 `config.yaml` 无法自动转换 | 低 | `getYamlParser()` 在 `require` 失败时抛出明确错误信息，引导用户手动安装或手动迁移。`yaml` 保留在 `package.json` dependencies 中，正常安装后 `require` 可用 |
| JSON 不支持注释，部分 YAML 用户习惯在配置中加注释 | 用户体验下降 | 中 | 自动迁移会丢失注释。配置文件主要由工具管理而非手写。在提案/设计中明确此 trade-off |

---

## 迁移步骤

1. **创建 Config Schema**：新建 `schemas/config.schema.ts`，定义 `configSchema`、`parseConfig`、`safeParseConfig`、`OpenSpecConfig`
2. **修改 lib/config.ts**：将全部文件操作从 YAML 迁移到 JSON，引入 Zod schema 校验，移除 `inferValue` 函数
3. **更新 MCP 工具描述**：更新 `mcp.ts` 中各 config 工具的 description（从 `config.yaml` 改为 `config.json`）
4. **更新命令模块**：修改 `config-context.ts`、`config-get.ts`、`config-set.ts`、`config-unset.ts`，适配新的类型化接口（config-set 移除 `inferValue` 调用）
5. **更新测试**：修改 `tests/lib/config.test.ts` 和命令测试，适配 JSON 格式、新的文件路径和移除 `inferValue`
6. **保留 yaml 依赖**：确认 `package.json` 中 `yaml` 保留为运行时依赖，更新 `package-deps.test.ts` 验证 yaml 存在（仅迁移路径使用）
7. **清理**：将项目根 `openspec/config.yaml` 替换为 `openspec/config.json`

---

## 待决问题

- 自动迁移过程中 `config.yaml` 文件存在且包含注释，转换为 JSON 后注释会丢弃。是否需要在迁移日志中输出警告？
- 是否需要在 `config.json` 文件头添加 `// This file is auto-managed. Manual edits may be overwritten.` 之类的提示？
  - JSON 标准不支持注释，但 `JSON.parse` 会忽略 `__comments` 这类自定义字段。可以在 skeleton 中添加 `_note` 字段（通过 `.passthrough()` 允许）
