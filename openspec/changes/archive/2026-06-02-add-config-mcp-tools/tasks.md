# 任务: add-config-mcp-tools

> **变更**: add-config-mcp-tools
> **日期**: 2026-06-01
> **基于**: design.md

---

## 阶段一：基础设施

- [x] 在 `plugins/dev-team/bin/` 目录安装 `yaml` npm 依赖
- [x] 创建 `plugins/dev-team/bin/src/lib/config.ts`，实现以下核心方法：
  - `readConfig(projectRoot: string): object` — 读取并解析 `openspec/config.yaml`，文件不存在时返回空对象
  - `writeConfig(projectRoot: string, data: object): void` — 序列化对象并写回 `openspec/config.yaml`，使用 `keepSourceTokens` 保留注释
  - `getValue(config: object, keyPath: string): { value: unknown, exists: boolean }` — 按点号分隔路径读取嵌套值
  - `setValue(config: object, keyPath: string, value: unknown): object` — 按点号分隔路径设置嵌套值，自动创建中间对象
  - `unsetValue(config: object, keyPath: string): { config: object, removed: boolean }` — 按点号分隔路径删除键
  - `inferValue(raw: string): unknown` — 类型推断：`"true"/"false"` 转 boolean，JSON 数组字符串转 array，数字字符串转 number
  - `ensureConfigFile(projectRoot: string): object` — 检查文件是否存在，不存在则创建含 `schema: spec-driven` 的骨架文件
- [x] 编写 `lib/config.ts` 的单元测试（`lib/config.test.ts`），覆盖：
  - 点号路径读写嵌套值
  - 文件不存在时的自动创建
  - 类型推断的四种场景
  - 不存在的 key 读取返回 `exists: false`
  - unset 已存在和不存在的 key

## 阶段二：Schema 定义

- [x] 创建 `plugins/dev-team/bin/src/schemas/config-get.schema.ts`，定义 input（`key`, `project_root?`）和 output（`key`, `value`, `exists`）的 Zod schema
- [x] 创建 `plugins/dev-team/bin/src/schemas/config-set.schema.ts`，定义 input（`key`, `value`, `project_root?`）和 output（`key`, `written`）的 Zod schema
- [x] 创建 `plugins/dev-team/bin/src/schemas/config-unset.schema.ts`，定义 input（`key`, `project_root?`）和 output（`key`, `removed`）的 Zod schema
- [x] 创建 `plugins/dev-team/bin/src/schemas/config-context.schema.ts`，定义 input（`context?`, `project_root?`）和 output（`context`, `written?`）的 Zod schema
- [x] 更新 `plugins/dev-team/bin/src/schemas/index.ts`，导出全部四个新 schema

## 阶段三：Command 实现

- [x] 创建 `plugins/dev-team/bin/src/commands/config-get.ts`，实现 `runConfigGet(options: ConfigGetOptions): ConfigGetResult`：
  - 调用 `ensureConfigFile()` 确保文件存在
  - 调用 `readConfig()` 读取配置
  - 调用 `getValue()` 按点号路径获取值
  - 返回 `{ key, value, exists }` 结构
- [x] 创建 `plugins/dev-team/bin/src/commands/config-set.ts`，实现 `runConfigSet(options: ConfigSetOptions): ConfigSetResult`：
  - 调用 `ensureConfigFile()` 确保文件存在
  - 调用 `readConfig()` 读取配置
  - 调用 `inferValue()` 对 value 进行类型推断
  - 调用 `setValue()` 设置值
  - 调用 `writeConfig()` 写回文件
  - 返回 `{ key, written: true }` 结构
- [x] 创建 `plugins/dev-team/bin/src/commands/config-unset.ts`，实现 `runConfigUnset(options: ConfigUnsetOptions): ConfigUnsetResult`：
  - 调用 `readConfig()` 读取配置
  - 调用 `unsetValue()` 删除键
  - 调用 `writeConfig()` 写回文件
  - 返回 `{ key, removed: boolean }` 结构
- [x] 创建 `plugins/dev-team/bin/src/commands/config-context.ts`，实现 `runConfigContext(options: ConfigContextOptions): ConfigContextResult`：
  - 无 `context` 参数时：读取配置，返回 `{ context: string }`
  - 有 `context` 参数时：更新配置中的 context 字段，写回文件，返回 `{ context, written: true }`

## 阶段四：MCP 工具注册

- [x] 在 `plugins/dev-team/bin/src/mcp.ts` 中导入四个新 schema 和四个 command 函数
- [x] 在 `mcp.ts` 的 `main()` 函数中注册 `config/get` 工具
- [x] 在 `mcp.ts` 的 `main()` 函数中注册 `config/set` 工具
- [x] 在 `mcp.ts` 的 `main()` 函数中注册 `config/unset` 工具
- [x] 在 `mcp.ts` 的 `main()` 函数中注册 `config/context` 工具

## 阶段五：构建与验证

- [x] 运行 `pnpm install` 确认 `yaml` 依赖安装成功（使用 pnpm 替代 npm，因 npm 11 存在 bug）
- [x] 运行构建命令（`vp pack`），确认无编译错误
- [x] 运行测试（`vp test`），确认所有测试通过（143/143，7 test files）
- [ ] 手动验证 AC-1：调用 `config/get` 读取现有 config.yaml 中的 `schema` 键，确认返回正确值
- [ ] 手动验证 AC-2：设置 `test_scripts.unit` 后，用 `config/get` 读取 `test_scripts.unit`，确认返回正确值
- [ ] 手动验证 AC-3：调用 `config/get` 读取不存在的键，确认 `exists` 为 false
- [ ] 手动验证 AC-4：调用 `config/set` 写入 `static_check` 数组值，确认文件内容正确
- [ ] 手动验证 AC-5：写入后使用 YAML 解析器重新解析文件，确认无语法错误
- [ ] 手动验证 AC-6：先设置 `test_scripts.e2e`，再调用 `config/unset` 移除，确认文件中不再包含该键
- [ ] 手动验证 AC-7：调用 `config/unset` 移除不存在的键，确认返回 `removed: false`
- [ ] 手动验证 AC-8：调用 `config/context` 无参数，确认返回当前 context 字符串
- [ ] 手动验证 AC-9：调用 `config/context` 带 context 参数写入，再读取确认内容更新
- [ ] 手动验证 AC-10：在非项目目录下调用工具并指定 `project_root`，确认能正确读写
- [ ] 手动验证 AC-11：在空目录下调用 `config/set`，确认自动创建 `openspec/config.yaml` 含 `schema: spec-driven`
- [x] 确认构建通过，无依赖错误
