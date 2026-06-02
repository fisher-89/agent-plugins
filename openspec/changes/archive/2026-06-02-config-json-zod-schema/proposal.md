# 提案: 配置存储迁移至 JSON + Zod 模式验证

> **变更**: config-json-zod-schema
> **日期**: 2026-06-02
> **状态**: 草案

---

## 问题

当前配置系统使用 YAML 格式 (`openspec/config.yaml`) 存储项目配置，并通过启发式类型推断 (`inferValue`) 处理值类型，存在以下问题：

1. **YAML 库依赖与管理成本**：需要外部 `yaml` npm 包来解析和序列化配置。YAML 的注释保留机制（`keepSourceTokens`）增加了实现复杂度，且每次写入都可能面临注释丢失风险。对于仅存储简单键值结构的配置文件，YAML 库的文档模型能力显得过于重量级。

2. **缺乏类型安全与结构验证**：配置读写使用 `Record<string, unknown>`，所有类型信息在运行时丢失。访问配置值时需要手动断言类型，拼写错误的键名或错误的值类型无法被提前捕获，只能在运行时暴露。

3. **启发式类型推断不可靠**：`inferValue()` 函数对字符串值进行启发式类型推断（`"true"` -> boolean，`"42"` -> number，`"[1,2]"` -> array）。这种隐式转换行为不可预测——`"null"` 不会被推断为 null，`"true story"` 不会被推断为 boolean，边界情况难以推理。

4. **格式不匹配**：MCP 工具的输入输出均为 JSON 格式（通过 `jsonContent()`），而持久化层使用 YAML，存在格式转换开销和认知负担。

5. **编辑器体验差**：YAML 文件缺乏自动补全和校验。JSON 格式配合 Zod schema 可以提供编译时类型检查、IDE 自动补全和运行时校验。

---

## 提案

将配置文件从 YAML 格式迁移到 JSON 格式 (`openspec/config.json`)，同时引入 Zod schema 定义配置结构。

**核心变更**：

1. **文件格式迁移**：`openspec/config.yaml` -> `openspec/config.json`。常规读写使用 Node.js 内置 `JSON.parse`/`JSON.stringify`，无需外部库。

2. **Zod Schema 定义**：创建 `schemas/config.schema.ts`，使用 `zod/v4` 定义配置文件的完整结构 schema：
   - `schema`: `z.literal('spec-driven')` 并设置默认值
   - `context`: 可选字符串
   - `rules`: 可选对象，含可选的 `proposal` 和 `tasks` 字符串数组
   - 使用 `.passthrough()` 允许额外未知字段（如工具扩展键）

3. **TypeScript 类型推导**：通过 `z.infer<typeof configSchema>` 自动生成 `OpenSpecConfig` 类型，替代 `Record<string, unknown>`。

4. **运行时校验**：配置读取时使用 `safeParseConfig()` 验证（优雅降级），写入前使用 `parseConfig()` 验证（拒绝不合法数据）。

5. **自动迁移**：首次读取时，如果 `config.json` 不存在但 `config.yaml` 存在，自动读取、验证并转换为 `config.json`，然后删除 `config.yaml`。

6. **yaml 依赖策略**：`yaml` 包保留为运行时依赖，但仅在自动迁移路径中通过动态 `require('yaml')` 加载。常规读写操作使用原生 JSON API，不加载 yaml。移除此依赖需要等所有项目完成迁移后单独进行。

**API 保持兼容**：所有四个 config/* 工具的 MCP 输入/输出 schema 保持不变。点号分隔的嵌套键路径访问方式不变。项目根目录解析逻辑不变。

---

## 能力

### 新增能力

- `config-schema` — 定义 `config.json` 的 Zod schema，提供 `parseConfig()`/`safeParseConfig()` 运行时校验函数和 `OpenSpecConfig` TypeScript 类型

### 修改的能力

- `config-get` — 配置读取目标从 `config.yaml` 改为 `config.json`，读取时通过 Zod 验证配置结构
- `config-set` — 配置写入目标从 `config.yaml` 改为 `config.json`，写入前通过 Zod 验证配置结构；不再使用 yaml 库进行序列化
- `config-unset` — 配置删除目标从 `config.yaml` 改为 `config.json`
- `config-context` — 配置上下文读写目标从 `config.yaml` 改为 `config.json`，读写均经过 Zod 验证

---

## 变更范围

### 实现以下特性

1. **Zod Schema 定义**：创建 `plugins/dev-team/bin/src/schemas/config.schema.ts`，定义 `config.json` 的 Zod schema，导出 `configSchema`、`parseConfig()`、`safeParseConfig()` 和 `OpenSpecConfig` 类型
2. **存储迁移**：`lib/config.ts` 中 `readConfig`/`writeConfig`/`ensureConfigFile` 的操作目标从 `config.yaml` 改为 `config.json`
3. **读时验证**：`readConfig()` 读取 JSON 后通过 `safeParseConfig()` 验证，验证失败时优雅降级返回原始解析数据（不崩溃）
4. **写前验证**：`writeConfig()` 在写入 JSON 前通过 `parseConfig()` 验证，验证失败抛出错误不触及文件系统
5. **类型提升**：核心函数返回类型从 `Record<string, unknown>` 升级为 `OpenSpecConfig`（由 Zod schema 推导）
6. **移除 `inferValue`**：移除启发式类型推断函数（JSON 格式天然保留类型，不再需要字符串启发式推断）
7. **自动迁移**：`readConfig()` 检测 `config.json` 不存在但 `config.yaml` 存在时，读取 YAML、用 Zod 验证、写入 JSON、删除 YAML
8. **保留 yaml 依赖**：`yaml` 包保留在 `package.json` dependencies 中，用于自动迁移路径（动态 `require('yaml')`，非常规加载）
9. **所有工具更新**：4 个 config 工具的注册描述和 handler 更新为引用 `config.json`
10. **schemas/index.ts 导出**：新增 `configSchema`、`parseConfig`、`safeParseConfig` 和 `OpenSpecConfig` 的导出

### 不要修改

- 不修改 config/* 工具的 MCP 输入/输出 schema（工具接口保持不变，内部实现变更）
- 不修改配置字段的语义（`schema`、`context`、`rules` 等字段的键名和含义不变）
- 不修改点号分隔的嵌套键路径访问方式
- 不修改项目根目录解析逻辑 (`resolveProjectRoot`)
- 不修改已有的 `phase/*` 和 `archi/*` MCP 工具
- 不修改任何 agent 或 skill 定义

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | `readConfig()` 读取 `config.json` 并返回 Zod schema 校验后的类型化对象 | 单元测试：创建有效的 `config.json`，读取后验证返回类型为 `OpenSpecConfig` 且字段类型正确 | P0 |
| AC-2 | `writeConfig()` 将类型化配置写入 `config.json`，写入内容为合法 JSON | 集成测试：写入配置，读取文件内容，使用 `JSON.parse` 验证 JSON 合法性 | P0 |
| AC-3 | Zod schema 拒绝不合法的配置（如字段类型错误）并报告明确错误 | 单元测试：构造非法配置对象，传入 `parseConfig()`，验证抛出特定错误信息 | P0 |
| AC-4 | `ensureConfigFile()` 创建包含 `{"schema": "spec-driven"}` 的 `config.json` | 集成测试：在空目录调用，验证文件存在且内容为合法 JSON | P0 |
| AC-5 | config-context/get/set/unset MCP 工具使用新 JSON 格式正常工作 | E2E 测试：调用各 MCP 工具，验证读写效果反映在 `config.json` 中 | P0 |
| AC-6 | 自动迁移：`config.json` 不存在但 `config.yaml` 存在时，自动读取 YAML、写入 JSON、删除 YAML | 集成测试：创建 `config.yaml`，首次调用 `readConfig()` 后验证 `config.json` 被创建且 `config.yaml` 被删除 | P1 |
| AC-7 | `inferValue()` 不再存在（已移除） | 代码审查：确认 `lib/config.ts` 中不再有 `inferValue` 函数 | P1 |
| AC-8 | `yaml` 包保留为运行时依赖（未被移除），用于自动迁移路径 | 检查 `package.json`：确认 `yaml` 在 dependencies 中存在 | P0 |
| AC-9 | 现有测试套件通过（含更新后的 config 测试） | 运行 `npm test`，验证所有测试通过 | P0 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 自动迁移过程中 `config.yaml` 解析失败（YAML 语法错误或格式不合法） | 配置数据丢失；已有 YAML 文件不会被删除（保留备份） | 低 | `migrateYamlToJson()` 在 Zod 验证失败时抛出错误，不删除 YAML 文件；数据不会丢失 |
| 已有项目存在 `openspec/config.yaml`，升级后首次读取触发迁移 | 首次读取稍有延迟 | 中 | 自动迁移是一次性操作，迁移后 YAML 被删除；后续读取直接走 JSON 路径 |
| Zod schema 过于严格导致合法的扩展键被拒绝 | 工具不可用 | 低 | schema 使用 `.passthrough()` 允许额外未知字段；所有非核心字段使用 `.optional()` |
| `yaml` 包长期保留但仅用于迁移路径，成为技术债 | 不必要的依赖膨胀 | 中 | 动态 `require('yaml')` 确保迁移完成后 yaml 不再加载；作为已知技术债在后续所有项目完成迁移后清理 |
| JSON 不支持注释，部分用户习惯在配置中加注释 | 用户体验下降 | 中 | 配置文件通常由工具管理而非手写。迁移后 YAML 注释不再保留，但可通过 `context` 字段和 `rules` 字段传递说明性信息 |
