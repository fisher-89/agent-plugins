# 任务: config-json-zod-schema

> **变更**: config-json-zod-schema
> **日期**: 2026-06-02
> **基于**: design.md, specs/config-schema/spec.md

---

## 阶段 1: 核心 Schema 定义

- [x] 1.1 在 `plugins/dev-team/bin/src/schemas/config.schema.ts` 中创建 `configSchema` (使用 `zod/v4`)
  - `schema`: `z.literal("spec-driven")`，默认值 `"spec-driven"`
  - `context`: `z.string().optional()`
  - `rules`: `z.object({ proposal: z.array(z.string()).optional(), tasks: z.array(z.string()).optional() }).optional()`
  - 使用 `.passthrough()` 允许额外未知字段
  - 导出 `configSchema` 命名导出
- [x] 1.2 导出 `OpenSpecConfig` 类型 (使用 `z.infer<typeof configSchema>`)
- [x] 1.3 实现 `parseConfig(data: unknown): OpenSpecConfig` 函数
  - 内部调用 `configSchema.parse()`
  - 在 ZodError 上添加描述性消息前缀
  - 返回深拷贝对象
- [x] 1.4 实现 `safeParseConfig(data: unknown): { success: true; data: OpenSpecConfig } | { success: false; error: ZodError }` 函数
  - 内部调用 `configSchema.safeParse()`
  - 永不抛出异常
- [x] 1.5 在 `plugins/dev-team/bin/src/schemas/index.ts` 中导出 `config.schema` 模块的所有公共符号

## 阶段 2: Config Library — JSON 读写 + Zod 校验

- [x] 2.1 修改 `plugins/dev-team/bin/src/lib/config.ts` 中的 `CONFIG_FILE` 常量
  - 从 `'openspec/config.yaml'` 改为 `'openspec/config.json'`
- [x] 2.2 重写 `readConfig(projectRoot: string): OpenSpecConfig`
  - 使用 `fs.readFileSync` + `JSON.parse` 代替 `parse()` from yaml
  - 使用 `parseConfig()` 校验并返回类型化对象
  - 如果文件不存在，返回默认的 `OpenSpecConfig` 对象 `{ schema: "spec-driven" }`
  - 如果 JSON 解析失败，返回空默认对象（优雅降级）
- [x] 2.3 实现 YAML 自动迁移逻辑
  - 在 `readConfig()` 中：如果 `config.json` 不存在但 `config.yaml` 存在
  - 读取并解析 YAML（使用 `yaml` 包）
  - 用 `parseConfig()` 校验数据
  - 用 `JSON.stringify(data, null, 2)` 写入 `config.json`
  - 删除 `config.yaml`
  - 返回类型化配置对象
- [x] 2.4 重写 `writeConfig(projectRoot: string, data: OpenSpecConfig): void`
  - 使用 `JSON.stringify(data, null, 2)` 代替 `stringify()` from yaml
  - 写入前使用 `parseConfig()` 校验数据合法性
  - 移除旧版本中 YAML 文档结构保留逻辑
- [x] 2.5 重写 `ensureConfigFile(projectRoot: string): OpenSpecConfig`
  - 创建 `config.json` skeleton（内容：`{"schema": "spec-driven"}`）代替 YAML skeleton
  - 创建 `openspec/` 目录
  - 返回校验后的配置对象
- [x] 2.6 删除 `inferValue()` 函数及其所有调用
- [x] 2.7 保持 `getValue()` / `setValue()` / `unsetValue()` 签名不变
  - 它们操作 `Record<string, unknown>` 对象，与配置格式无关
  - 不需要修改

## 阶段 3: MCP 工具描述更新

- [x] 3.1 更新 `plugins/dev-team/bin/src/mcp.ts` 中 `config/get` 工具的 description
  - 从 `openspec/config.yaml` 改为 `openspec/config.json`
- [x] 3.2 更新 `plugins/dev-team/bin/src/mcp.ts` 中 `config/set` 工具的 description
  - 从 `openspec/config.yaml` 改为 `openspec/config.json`
  - 移除关于 `inferValue` / 类型推断的描述（JSON 不再需要启发式类型推断）
- [x] 3.3 更新 `plugins/dev-team/bin/src/mcp.ts` 中 `config/unset` 工具的 description
  - 从 `openspec/config.yaml` 改为 `openspec/config.json`
- [x] 3.4 更新 `plugins/dev-team/bin/src/mcp.ts` 中 `config/context` 工具的 description
  - 从 `openspec/config.yaml` 改为 `openspec/config.json`

## 阶段 4: Command 层适配

- [x] 4.1 修改 `config-context.ts` — 适配 `OpenSpecConfig` 类型
  - 将 `config` 变量类型从 `Record<string, unknown>` 隐式提升为 `OpenSpecConfig`
  - 移除对 `config.context` 的显式类型断言
- [x] 4.2 修改 `config-get.ts` — 适配 `OpenSpecConfig` 类型
  - 将 `config` 变量类型从 `Record<string, unknown>` 提升为 `OpenSpecConfig`
- [x] 4.3 修改 `config-set.ts` — 移除 `inferValue` 调用
  - 移除 `import { inferValue }` 语句
  - 移除 `inferValue()` 调用，直接使用 `options.value`（JSON 格式天然区分类型）
  - 保留 `ensureConfigFile` + `readConfig` + `setValue` + `writeConfig` 流程
- [x] 4.4 修改 `config-unset.ts` — 适配 `OpenSpecConfig` 类型
  - 将 `config` 变量类型从 `Record<string, unknown>` 提升为 `OpenSpecConfig`

## 阶段 5: 依赖确认 + 测试创建

- [x] 5.1 保留 `yaml` 包在 `package.json` 的 `dependencies` 中（常规 JSON 读写不加载，仅自动迁移路径通过 `getYamlParser()` 动态 `require('yaml')` 加载）
- [x] 5.2 创建 `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts`
  - 从 archive 复制测试框架，将所有 fixture 中的 `config.yaml` 路径替换为 `config.json`
  - 更新 YAML 相关断言为 JSON 断言（如 `YAML.parse(raw)` -> `JSON.parse(raw)`）
  - 删除 `inferValue` 相关测试用例
  - 删除注释保留相关的集成测试（旧 YAML 特性，JSON 不支持注释）
  - 新增自动迁移（AC-6）、JSON 优雅降级、Zod schema 校验失败场景的集成测试
  - 确保 `ensureConfigFile` 测试验证 JSON skeleton 而非 YAML skeleton
- [x] 5.3 创建 `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts`
  - 从 archive 复制测试框架，移除 `inferValue` 相关的测试
  - 新增 AC-7 断言：`"true"` 存储为字符串而非布尔值
  - 新增 config/set 写入前验证错误未创建文件的测试
- [x] 5.4 创建 `openspec/changes/config-json-zod-schema/tests/commands/config-context.test.ts`
  - 从 archive 复制测试框架，将所有 `config.yaml` 路径断言改为 `config.json`
- [x] 5.5 创建 `openspec/changes/config-json-zod-schema/tests/commands/config-get.test.ts`
  - 从 archive 复制测试框架，将所有 `config.yaml` 路径断言改为 `config.json`
  - 新增无效 JSON 优雅降级的测试
- [x] 5.6 创建 `openspec/changes/config-json-zod-schema/tests/commands/config-unset.test.ts`
  - 从 archive 复制测试框架，将所有 `config.yaml` 路径断言改为 `config.json`
- [x] 5.7 更新 `openspec/changes/config-json-zod-schema/tests/package-deps.test.ts`
  - 确认 `yaml` 在 `package.json` dependencies 中存在（保留用于自动迁移路径）
  - 新增 `zod` 依赖存在的验证（用于 config schema 校验）

## 阶段 6: 项目根清理 + 验证

- [x] 6.1 将 `openspec/config.yaml` 迁移为 `openspec/config.json`
  - 读取现有 `config.yaml` 内容
  - 写入 `config.json`（JSON 格式）
  - 删除 `config.yaml`
- [x] 6.2 运行 `npm test` 确认全部测试通过
- [x] 6.3 运行 `npm run build` 确认构建成功
- [x] 6.4 运行 `npm run check` 确认代码风格和类型检查通过
- [x] 6.5 验证自动迁移：在临时目录创建 `config.yaml`，确认 `readConfig` 能自动迁移为 `config.json`
- [x] 6.6 端到端验证：手动调用 config-context/config-get/config-set/config-unset 工具，确认读写效果反映在 `config.json` 中
