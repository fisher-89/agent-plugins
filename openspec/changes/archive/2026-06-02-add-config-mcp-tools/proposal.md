# 提案: 添加 config/* MCP 工具

> **变更**: add-config-mcp-tools
> **日期**: 2026-06-01
> **状态**: 起草中

---

## 问题

当前 dev-team MCP 服务器主要提供两类工具：工作流门控（phase/*）和架构管理（archi/*）。但开发团队在执行工作流时缺少对项目级配置的编程化访问能力。具体痛点包括：

1. **配置访问依赖外部 CLI**：Agent 需要读取或修改 OpenSpec 项目配置（`openspec/config.yaml`）时，只能通过 shell 命令调用 `openspec config` 或直接读写 YAML 文件。这不仅增加了对外部 CLI 的依赖，还引入了跨平台兼容性问题。
2. **无结构化的配置读写工具**：标准流程中需要获取 `static_check` 命令列表、`test_scripts` 配置等 dev-team 专用配置项，但当前没有任何 MCP 工具提供此能力。
3. **项目上下文管理缺失**：`config.yaml` 中的 `context` 字段用于向 AI 提供项目技术栈和领域知识，但 Agent 无法通过标准接口读取或更新它。

需要在 OpenSpec 规范之外扩展 dev-team 特定的配置键，并通过 MCP 工具暴露标准化的读写接口，以消除对外部 CLI 的耦合。

---

## 提案

在 dev-team MCP 服务器中新增 4 个 `config/*` 工具：`config/get`、`config/set`、`config/unset`、`config/context`。这些工具直接通过 YAML 解析库读写 `openspec/config.yaml`，取代对外部 `openspec config` CLI 的调用。

**配置扩展**：在 `openspec/config.yaml` 现有结构基础上增加 dev-team 专用键：
- `static_check`：字符串数组，定义每次静态检查阶段执行的 shell 命令
- `test_scripts`：固定键对象，包含 `unit`、`integration`、`e2e` 三个字段，各字段值为 shell 命令字符串

**实现结构**：遵循现有 MCP 构建模式——
- 在 `schemas/` 目录下新增 Zod input/output schema 文件
- 在 `commands/` 或 `lib/` 目录下新增配置读写逻辑（推荐 `lib/config.ts`）
- 在 `mcp.ts` 中注册四个新工具
- 引入 `yaml` npm 包作为 YAML 解析依赖
- 不新增 `config/schema` 工具（Plan B，当前无此需求）

---

## 能力

### 新增能力

- `config-get` — 读取 `openspec/config.yaml` 中指定键的值，返回键名、值和存在标志
- `config-set` — 写入 `openspec/config.yaml` 中指定键的值，支持嵌套键路径
- `config-unset` — 删除 `openspec/config.yaml` 中指定键的条目
- `config-context` — 读取或写入 `openspec/config.yaml` 中的 `context` 字段

### 修改的能力

（无）

---

## 变更范围

### 实现以下特性

1. **config/get 工具**：接收 `{ key, project_root? }` 输入，返回 `{ key, value, exists }`。支持点号分隔的嵌套键路径（如 `test_scripts.unit`）。当 key 不存在时 `exists` 为 false。
2. **config/set 工具**：接收 `{ key, value, project_root? }` 输入，将值写入 YAML 并保留文件原有注释和格式（通过 YAML 文档节点操作实现），返回 `{ key, written: true }`。
3. **config/unset 工具**：接收 `{ key, project_root? }` 输入，从 YAML 中移除指定键，返回 `{ key, removed: true }`。键不存在时返回 `{ key, removed: false }`。
4. **config/context 工具**：接收 `{ context?, project_root? }` 输入。无 `context` 参数时读取并返回当前 context；有 `context` 参数时写入并返回 `{ context, written: true }`。
5. **配置扩展**：在 `openspec/config.yaml` 中新增 `static_check`（字符串数组）和 `test_scripts`（`unit`/`integration`/`e2e` 键）字段，作为可选扩展。
6. **依赖管理**：在 `plugins/dev-team/bin/package.json` 中新增 `yaml` npm 依赖。
7. **工具注册**：在 `mcp.ts` 中注册全部四个工具。

### 不要修改

- 不修改 OpenSpec 核心 schema（`schema`、`rules` 等现有字段）
- 不修改已有的 `phase/*` 和 `archi/*` MCP 工具
- 不新增 `config/schema` 工具（Plan B 排除）
- 不修改任何 agent 或 skill 定义（本次仅为 MCP 工具层变更）
- 不修改 `openspec/config.yaml` 中的 `schema` 字段值

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | config/get 能正确读取现有 config.yaml 中的 schema、context 等标准字段 | 调用 `config/get` 获取 `schema` 键，返回 `{"key":"schema","value":"spec-driven","exists":true}` | P0 |
| AC-2 | config/get 支持嵌套键路径（点号分隔） | 设置 `test_scripts.unit` 后调用 `config/get` 获取 `test_scripts.unit`，返回正确的值 | P0 |
| AC-3 | config/get key 不存在时 exists 为 false | 调用 `config/get` 获取不存在的键，返回 `{"key":"nonexistent","exists":false}` | P0 |
| AC-4 | config/set 能写入新键并正确持久化到 YAML 文件 | 调用 `config/set` 写入 `static_check` 数组，重新读取文件确认内容正确 | P0 |
| AC-5 | config/set 写入后 YAML 文件语法合法 | 使用 YAML 解析器重新解析文件，无语法错误 | P0 |
| AC-6 | config/unset 能移除指定键 | 先设置 `test_scripts.e2e`，调用 `config/unset` 移除，确认文件中不再包含该键 | P0 |
| AC-7 | config/unset 键不存在时返回 removed: false | 调用 `config/unset` 移除不存在的键，返回 `{"key":"nonexistent","removed":false}` | P1 |
| AC-8 | config/context 无参数时返回当前 context | 调用 `config/context`，返回 `{"context":"..."}` 字符串 | P0 |
| AC-9 | config/context 带参数时写入新 context | 调用 `config/context` 设置新 context，再读取确认内容更新 | P0 |
| AC-10 | 所有工具支持 project_root 参数覆盖默认项目根目录 | 在非项目目录下调用工具并指定 `project_root`，能正确读写指定项目下的 config.yaml | P1 |
| AC-11 | 所有工具正确处理 config.yaml 不存在的情况（自动创建骨架文件） | 在空目录下调用 `config/set`，确认自动创建 `openspec/config.yaml` 含 `schema: spec-driven` | P2 |
| AC-12 | 引入 `yaml` 依赖后构建通过 | 运行 `npm install` 和构建脚本，无依赖错误 | P0 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| YAML 解析/序列化导致注释丢失 | 每次 config/set 写入后文件注释被覆盖，影响可读性 | 中 | 选择保留注释的 YAML 库（如 `yaml` 包的 `keepSourceTokens` 选项），或在写入时保留未知节点 |
| config.yaml 格式错误导致解析失败 | 工具无法读取配置，阻止正常工作流 | 低 | 工具在解析失败时返回明确的错误信息，不抛出未捕获异常；提供手动修复指引 |
| 并发写入导致数据竞争 | 多个 Agent 同时修改配置导致数据丢失 | 低 | 采用读取-修改-写入的原子操作模式；MCP 工具天然串行执行 |
| 新增配置键与未来 OpenSpec 版本冲突 | `static_check` 或 `test_scripts` 键名被官方占用 | 低 | 在提案中明确这些为 dev-team 扩展键；在 `config.yaml` 中以注释标注扩展区域 |
