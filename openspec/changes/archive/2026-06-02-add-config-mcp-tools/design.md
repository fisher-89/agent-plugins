# 设计: add-config-mcp-tools

> **变更**: add-config-mcp-tools
> **日期**: 2026-06-01
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| config 核心库 | YAML 文件的读取、解析、写入、序列化。提供点号分隔的嵌套键路径访问。处理文件不存在时的骨架创建。 | `plugins/dev-team/bin/src/lib/config.ts` | `yaml`, `fs`, `path` | TypeScript |
| config/get schema | 定义 `config/get` 工具的 Zod 输入/输出 schema | `plugins/dev-team/bin/src/schemas/config-get.schema.ts` | `zod` | Zod v4 |
| config/set schema | 定义 `config/set` 工具的 Zod 输入/输出 schema | `plugins/dev-team/bin/src/schemas/config-set.schema.ts` | `zod` | Zod v4 |
| config/unset schema | 定义 `config/unset` 工具的 Zod 输入/输出 schema | `plugins/dev-team/bin/src/schemas/config-unset.schema.ts` | `zod` | Zod v4 |
| config/context schema | 定义 `config/context` 工具的 Zod 输入/输出 schema | `plugins/dev-team/bin/src/schemas/config-context.schema.ts` | `zod` | Zod v4 |
| config/get 命令 | `config/get` 工具的业务逻辑：根据 key 读取配置值，返回 key/value/exists | `plugins/dev-team/bin/src/commands/config-get.ts` | `lib/config.ts` | TypeScript |
| config/set 命令 | `config/set` 工具的业务逻辑：将 key/value 写入 YAML，含类型推断和嵌套键创建 | `plugins/dev-team/bin/src/commands/config-set.ts` | `lib/config.ts` | TypeScript |
| config/unset 命令 | `config/unset` 工具的业务逻辑：从 YAML 中删除指定 key | `plugins/dev-team/bin/src/commands/config-unset.ts` | `lib/config.ts` | TypeScript |
| config/context 命令 | `config/context` 工具的业务逻辑：读取或写入 `context` 字段 | `plugins/dev-team/bin/src/commands/config-context.ts` | `lib/config.ts` | TypeScript |
| schemas 索引 | 重新导出所有 config/* schema | `plugins/dev-team/bin/src/schemas/index.ts` | 各 schema 文件 | TypeScript |
| MCP 服务器入口 | 注册四个 `config/*` 工具到 MCP server | `plugins/dev-team/bin/src/mcp.ts` | 各 command, schema | TypeScript / MCP SDK |

### 组件图

```
Agent / AI 客户端
     |
     v
MCP Server (mcp.ts)
     |--- config/get  ----\
     |--- config/set  -----> commands/config-*.ts --> lib/config.ts --> openspec/config.yaml
     |--- config/unset ---/
     |--- config/context -/
     |
     |--- phase/* (现有)
     |--- archi/* (现有)
```

---

## 数据流

### 流程描述

1. Agent 通过 MCP 协议调用 `config/get`、`config/set`、`config/unset` 或 `config/context` 工具。
2. `mcp.ts` 中的工具 handler 接收到参数后，使用 `resolveProjectRoot()` 解析项目根目录。
3. handler 调用对应 command 模块的 `runXxx()` 函数。
4. command 函数调用 `lib/config.ts` 提供的 YAML 读写方法。
5. `lib/config.ts` 使用 `yaml` 包解析 `openspec/config.yaml` 为 JavaScript 对象，执行读取/修改操作后序列化回文件。
6. 结果通过 `jsonContent()` 包装为结构化 JSON 返回给 Agent。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| Config 文件 | `schema`, `context`, `rules`, `static_check`, `test_scripts` | 顶层键，无嵌套约束（点号路径用于访问嵌套子键） | `openspec/config.yaml` |
| config/get 输出 | `key: string`, `value: unknown`, `exists: boolean` | 直接映射 YAML 键 | 无（仅响应） |
| config/set 输出 | `key: string`, `written: boolean` | — | 无（仅响应） |
| config/unset 输出 | `key: string`, `removed: boolean` | — | 无（仅响应） |
| config/context 输出（读） | `context: string` | 映射 `config.yaml` 中的 `context` 字段 | 无（仅响应） |
| config/context 输出（写） | `context: string`, `written: boolean` | — | 无（仅响应） |

---

## API 设计

所有工具通过 MCP 协议暴露，无 HTTP 路由。

### config/get

| 属性 | 值 |
|------|-----|
| 工具名 | `config/get` |
| 描述 | 读取 `openspec/config.yaml` 中指定键的值，支持点号分隔的嵌套路径 |
| 输入 | `{ key: string, project_root?: string }` |
| 输出 | `{ key: string, value: unknown, exists: boolean }` |

### config/set

| 属性 | 值 |
|------|-----|
| 工具名 | `config/set` |
| 描述 | 写入 `openspec/config.yaml` 中指定键的值。支持嵌套键路径（如 `test_scripts.unit`）。当文件不存在时自动创建包含 `schema: spec-driven` 的骨架文件。 |
| 输入 | `{ key: string, value: unknown, project_root?: string }` |
| 输出 | `{ key: string, written: boolean }` |

**类型推断规则**：
- 字符串 `"true"` / `"false"` 自动转为 boolean
- 以 `[` 开头且可解析为 JSON 数组的字符串，自动转为数组
- 纯数字字符串自动转为 number
- 其他值保持原样

### config/unset

| 属性 | 值 |
|------|-----|
| 工具名 | `config/unset` |
| 描述 | 从 `openspec/config.yaml` 中删除指定键及其值 |
| 输入 | `{ key: string, project_root?: string }` |
| 输出 | `{ key: string, removed: boolean }` |

### config/context

| 属性 | 值 |
|------|-----|
| 工具名 | `config/context` |
| 描述 | 不传 `context` 参数时读取并返回当前 `context` 字段值；传入 `context` 参数时更新该字段 |
| 输入 | `{ context?: string, project_root?: string }` |
| 输出（读） | `{ context: string }` |
| 输出（写） | `{ context: string, written: boolean }` |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 选择 `yaml` npm 包作为 YAML 解析库 | `yaml` 包（`npmjs.com/package/yaml`）是目前最广泛使用的 TypeScript YAML 库，支持 `keepSourceTokens` 选项以保留注释，API 简洁。该包已在 npm 上有超过 5000 万周下载量，可靠性有保障。 | **js-yaml**: 解析功能类似但不原生支持保留注释。需额外的自定义逻辑来在写入时保留注释，增加实现复杂度。 |
| D2 | 将核心 YAML 操作封装在 `lib/config.ts` 中，与 command 层分离 | 遵循现有 `lib/` 模式（如 `lib/eval-json.ts`、`lib/archi-query.ts`）。分离后核心逻辑可被多个 command 复用（如 config/context 和 config/get 都需读取 YAML），也便于独立测试。 | **在 command 中直接操作文件**: 会导致代码重复，每个 command 都要实现 YAML 解析逻辑，不利于维护和测试。 |
| D3 | 点号分隔的嵌套键路径（如 `test_scripts.unit`） | 符合直觉，与 lodash/object-path 等工具的路径语法一致。实现简单：按 `.` 分割路径后逐层遍历 JSON 对象。 | **JSON Pointer (RFC 6901)**: 标准化的 JSON 路径语法，但语法复杂（`/test_scripts/unit`），与 YAML 社区的 dot-path 习惯不符，增加用户学习成本。 |
| D4 | config/set 中实现类型推断（string "true" 转 boolean 等） | Agent 通过 MCP 参数传入的值在 JSON 序列化后全是字符串。类型推断使工具的使用更自然，用户无需手动进行类型转换。 | **严格类型传递**: 要求调用者自行处理类型，通过 `valueType` 参数显式指定类型。增加了接口复杂度，且与 MCP 协议的 JSON 传输特性不匹配。 |
| D5 | config/context 作为独立工具而非 config/get/set 的特例 | `context` 是工作流中最常读写的字段，提供专用工具可减少 Agent 的调用步骤，提升效率。且工具名 `config/context` 语义清晰。 | **仅用 config/get 和 config/set**: 功能上完全可行，但每次操作 context 都需要指定 `key: "context"`，对高频操作不够友好。 |
| D6 | 文件不存在时自动创建骨架文件（`schema: spec-driven`） | 避免首次使用时的文件缺失错误。初始 schema 值与 OpenSpec 默认值一致，确保向后兼容。 | **报错提示用户先运行 openspec init**: 严格但用户体验差，增加 Agent 工作流的失败路径。 |
| D7 | 输出统一使用 `jsonContent()` 包装 | 与现有 `phase/*`、`archi/*` 工具保持一致的响应格式，确保客户端能统一解析。 | **直接返回原始文本**: 不符合现有 MCP 工具的 JSON 输出约定，增加客户端解析复杂度。 |

---

## 依赖

### 运行时依赖

- `yaml` — YAML 解析与序列化，支持保留注释

### 构建/测试依赖

- 无新增 — 使用现有 `typescript`、`vite-plus` 构建工具链

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| YAML 解析/序列化导致注释丢失 | 每次 config/set 写入后文件注释被覆盖，影响可读性 | 中 | 使用 `yaml` 包的 `keepSourceTokens` 选项，在写入时仅修改目标键的节点，保留未知节点的原始表示 |
| config.yaml 格式错误导致解析失败 | 工具无法读取配置，阻止正常工作流 | 低 | 解析失败时返回明确的错误信息而非抛出未捕获异常；错误信息中包含手工修复指引 |
| 并发写入导致数据竞争 | 多个 Agent 同时修改配置导致数据丢失 | 低 | MCP 工具天然串行执行（单个 StdioServerTransport 单线程处理请求）；采用读取-修改-写入的原子操作模式 |
| 新增配置键与未来 OpenSpec 版本冲突 | `static_check` 或 `test_scripts` 键名被官方占用 | 低 | 在设计文档和 config.yaml 中以注释明确标注这些为 dev-team 扩展键 |

---

## 迁移步骤

1. 安装 `yaml` npm 依赖：在 `plugins/dev-team/bin/` 目录运行 `npm install yaml`
2. 无 schema 迁移需求——新增键均为可选扩展，不影响现有 OpenSpec 核心字段

---

## 待决问题

- 是否需要为 `test_scripts` 添加写时验证（只允许 `unit`、`integration`、`e2e` 三个子键）？当前设计将验证放在 `config/set` 工具中，当写入 `test_scripts` 下的非法键时返回警告而不是错误。
- 未来是否需要为 config 工具添加 `config/list` 工具列出所有顶层键？当前提案已排除此需求，但在 config/get 支持不传 key 返回整个配置对象可作为轻量替代。
