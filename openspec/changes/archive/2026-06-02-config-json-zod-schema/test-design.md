# 测试设计: config-json-zod-schema

> **变更**: config-json-zod-schema
> **日期**: 2026-06-02
> **基于**: proposal.md, design.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | `schemas/config.schema.ts` 的 Zod schema 定义、`parseConfig`/`safeParseConfig` 验证函数；`lib/config.ts` 的 JSON 读写、点号路径访问、骨架文件创建；四个 command（config-get/config-set/config-unset/config-context）的业务逻辑；各 config/* schema 的 Zod 输入/输出验证 | vite-plus (vitest compatible) | 核心 JSON 操作和命令层全分支覆盖 >= 90%；确保 Zod schema 校验、类型推导、路径解析、文件缺失处理、自动迁移逻辑等在隔离环境中通过测试 |
| 集成测试 | 临时目录下的真实 JSON 文件读写，验证 `lib/config.ts` 的各函数在真实文件系统中的完整行为（自动创建目录、骨架文件、JSON 语法合法性、YAML 到 JSON 的自动迁移、迁移后 YAML 删除） | vite-plus (vitest compatible) | 覆盖真实文件系统的路径创建、JSON 序列化/反序列化边界、自动迁移路径、文件持久化与重读的完整性 |

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock 或临时文件系统，不依赖真实外部环境。

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` | 单元测试 | `readConfig` 读取有效 `config.json`，返回类型为 `OpenSpecConfig` 且 `schema` 字段正确 |
| AC-1 | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` | 单元测试 | Zod schema 对有效配置 `{"schema":"spec-driven","context":"..."}` 校验通过，返回类型化对象 |
| AC-2 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` | 集成测试 | `writeConfig` 写入后读取文件内容，使用 `JSON.parse` 验证 JSON 合法性 |
| AC-2 | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` | 单元测试 | config/set 通过写入后使用 `JSON.parse` 验证文件内容为合法 JSON |
| AC-3 | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` | 单元测试 | `parseConfig` 对非法配置（如 `{"schema":123}`）抛出 ZodError，错误信息包含字段路径 |
| AC-3 | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` | 单元测试 | `safeParseConfig` 对非法配置（如 `{"schema":false}`）返回 `{success:false}` 和 ZodError |
| AC-4 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` | 集成测试 | 在空目录调用 `ensureConfigFile`，确认 `config.json` 被创建且内容为 `{"schema":"spec-driven"}` |
| AC-5 | `openspec/changes/config-json-zod-schema/tests/commands/config-get.test.ts` | 单元测试 | config/get 读取 `config.json` 中的值，返回正确结果 |
| AC-5 | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` | 单元测试 | config/set 向 `config.json` 写入值，返回 `{key, written:true}` |
| AC-5 | `openspec/changes/config-json-zod-schema/tests/commands/config-unset.test.ts` | 单元测试 | config/unset 从 `config.json` 删除键，返回 `{key, removed:true}` |
| AC-5 | `openspec/changes/config-json-zod-schema/tests/commands/config-context.test.ts` | 单元测试 | config/context 读取/写入 `config.json` 中的 `context` 字段 |
| AC-5 | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` | 集成测试 | config/set 写入后查看 `config.json` 文件内容正确反映写入值 |
| AC-6 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` | 集成测试 | 创建 `config.yaml`，调用 `readConfig` 后 `config.json` 被创建、`config.yaml` 被删除、返回数据包含原 YAML 内容 |
| AC-7 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` | 单元测试 | 确认 `lib/config.ts` 不再导出 `inferValue` 函数（import 时为 undefined） |
| AC-7 | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` | 单元测试 | config/set 不再对字符串值做类型推断，`"true"` 存储为字符串而非布尔值 |
| AC-8 | `openspec/changes/config-json-zod-schema/tests/package-deps.test.ts` | 构建验证 | `package.json` 中 `yaml` 保留为运行时依赖（用于自动迁移路径） |
| AC-9 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` | 全部测试文件 | 运行 `npm test` 执行全部单元测试和集成测试，验证所有测试通过（含更新后的 config 测试，各命令测试和 schema 测试不出现回归失败） |
| AC-9 | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` | 全部测试文件 | 同上 — Zod schema 校验测试在 schema 定义变更后保持通过 |
| AC-9 | `openspec/changes/config-json-zod-schema/tests/commands/config-get.test.ts` | 全部测试文件 | 同上 — config/get 命令测试在 JSON 格式迁移后保持通过 |
| AC-9 | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` | 全部测试文件 | 同上 — config/set 命令测试在 JSON 格式迁移后保持通过 |
| AC-9 | `openspec/changes/config-json-zod-schema/tests/commands/config-unset.test.ts` | 全部测试文件 | 同上 — config/unset 命令测试在 JSON 格式迁移后保持通过 |
| AC-9 | `openspec/changes/config-json-zod-schema/tests/commands/config-context.test.ts` | 全部测试文件 | 同上 — config/context 命令测试在 JSON 格式迁移后保持通过 |
| AC-9 | `openspec/changes/config-json-zod-schema/tests/package-deps.test.ts` | 全部测试文件 | 同上 — 依赖校验测试在 yaml 保留为运行时依赖后保持通过 |
| 幂等性-1 | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` | 集成测试 | config/set 同一 key/value 重复调用两次，两次返回结果一致（无累积副作用） |
| 幂等性-2 | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` | 集成测试 | config/set 同一 key 先后设置不同 value，第二次正确覆盖 |
| 幂等性-3 | `openspec/changes/config-json-zod-schema/tests/commands/config-unset.test.ts` | 集成测试 | config/unset 对已移除的 key 再次调用返回 `{key, removed:false}` |
| 幂等性-4 | `openspec/changes/config-json-zod-schema/tests/commands/config-unset.test.ts` | 集成测试 | config/unset 同一 key 两次调用，第一次 `removed:true`，第二次 `removed:false` |

---

## 3. 测试策略

### 3.1 方法

采用"三层"测试策略：

1. **单元测试（Zod Schema 验证层）**：针对 `schemas/config.schema.ts` 中新增的 `configSchema`、`parseConfig`、`safeParseConfig` 函数进行测试。使用内联数据构造合法/非法配置对象，验证 Zod 校验的准确性、错误信息的可读性、`.passthrough()` 允许未知字段、默认值填充等行为。不依赖文件系统。

2. **单元测试（核心逻辑 + 命令层）**：针对 `lib/config.ts` 中可独立测试的工具函数——点号路径解析、JSON 对象操作——使用内存中的配置对象进行测试，不依赖真实文件系统。针对四个 command 函数的业务逻辑，参数解析和响应格式化进行测试。特别关注：不再有 `inferValue` 调用、`OpenSpecConfig` 类型化对象的传导、`config/set` 中 JSON 值直接传递（字符串 `"true"` 不再转为布尔值）。

3. **集成测试（文件系统）**：针对 `lib/config.ts` 中需要真实文件系统的操作（JSON 文件创建、序列化/反序列化、YAML 到 JSON 自动迁移、持久化验证），使用 `fs.mkdtempSync` 创建临时目录，模拟完整的 config.json 读写生命周期。

### 3.2 测试分类

- **单元测试**: 不依赖真实文件系统或外部服务的测试。Zod schema 校验函数、点号路径解析、命令层参数处理和响应构建。通过直接构造 JS 对象作为 `parseConfig`/`safeParseConfig`/`getValue`/`setValue`/`unsetValue` 的输入来隔离测试。

- **集成测试**: 依赖真实文件系统的测试。在 `beforeAll` 中使用 `fs.mkdtempSync` 创建临时目录，通过 `lib/config.ts` 的导出函数执行 JSON 文件的创建、读取、写入、删除，以及 YAML 到 JSON 的自动迁移。在 `afterAll` 中清理临时目录。验证 JSON 语法合法性、骨架文件创建、自动迁移的完整流程。

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock 或临时文件系统，不依赖真实外部环境。

> **关于 yaml 依赖**: `yaml` 包保留为运行时依赖（未被移除），仅在 YAML 到 JSON 自动迁移路径中通过动态 `require('yaml')` 加载。常规 JSON 读写操作不加载 yaml。`tests/package-deps.test.ts`（AC-8）验证 `package.json` 中 `yaml` 在 `dependencies` 中存在。

### 3.3 模拟策略

| 模拟对象 | 策略 | 适用测试 |
|----------|------|----------|
| 配置文件路径 | 通过 `project_root` 参数指向 `fs.mkdtempSync` 创建的临时目录 | 全部集成测试 |
| 文件不存在场景 | 测试在空临时目录下调用 config 函数，自动创建过程由真实文件系统验证 | config.test.ts 的集成测试部分 |
| 点号路径解析 | 直接构造嵌套 JS 对象作为输入，测试 `getValue`/`setValue`/`unsetValue` 的路径遍历逻辑 | config.test.ts 的纯逻辑单元测试 |
| Zod schema 校验 | 构造已知合法/非法的配置对象作为输入，测试 `parseConfig`/`safeParseConfig` 的行为 | config.schema.test.ts |
| 命令层与 lib 的交互 | 命令层测试中直接调用真实的 lib 函数（四个 config 命令本身是薄包装层，逻辑集中在 lib），使用临时目录进行文件系统操作 | 命令层的四个 `*.test.ts` |
| project_root 解析 | 使用临时目录路径作为 `project_root` 参数传入命令层和 lib 函数，验证路径正确性 | 所有涉及 project_root 的测试 |
| YAML 自动迁移 | 在临时目录中手动创建 `config.yaml`，然后调用 `readConfig` 触发迁移，验证 JSON 文件被创建、YAML 文件被删除、内容正确 | config.test.ts 的自动迁移集成测试 |
| JSON 解析失败 | 在临时目录中创建内容为无效 JSON 的 `config.json`，调用 `readConfig` 验证优雅降级 | config.test.ts 的边界测试 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| config.json 不存在（空项目） | 在空目录下调用 `readConfig` | 返回默认 `OpenSpecConfig` 对象 `{"schema":"spec-driven"}`，不抛出异常 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config.json 不存在时自动创建骨架文件 | 在空目录下调用 `writeConfig` | 自动创建 `openspec/config.json`，内容包含 `{"schema":"spec-driven"}` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config.json 内容为无效 JSON | config.json 包含无效 JSON 语法（如尾逗号、未闭合引号） | `readConfig` 优雅降级，返回默认 `OpenSpecConfig` 对象，不抛出异常 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config.json 内容合法但不符合 Zod schema（额外非标准字段） | config.json 包含 schema 之外的合法 JSON 字段 | `readConfig` 通过 `.passthrough()` 允许额外字段，返回包含该字段的类型化对象 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config.json 内容不合法（schema 字段类型错误） | config.json 中 `schema` 字段为数字而非字符串 | `readConfig` 抛出 ZodError 或优雅降级返回默认对象（取决于实现策略） | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| Zod schema 默认值填充 | 配置对象完全为空 `{}` 传入 `parseConfig` | 返回对象包含默认值 `schema:"spec-driven"` | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` |
| Zod schema 拒绝非法字段类型 | `parseConfig({"schema":123})` | 抛出 ZodError，错误消息包含字段路径 `schema` | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` |
| Zod schema 拒绝非法嵌套类型 | `parseConfig({"rules":{"proposal":"not an array"}})` | 抛出 ZodError，错误消息包含 `rules.proposal` 路径 | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` |
| safeParseConfig 不抛出异常 | `safeParseConfig(null)` | 返回 `{success:false, error:ZodError}`，不抛出异常 | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` |
| passthrough 保留未知字段 | `parseConfig({"schema":"spec-driven","custom_tool_key":"value"})` | 校验成功，返回对象包含 `custom_tool_key` 字段 | `openspec/changes/config-json-zod-schema/tests/schemas/config.schema.test.ts` |
| YAML 自动迁移：标准流程 | 创建 `config.yaml`（含 context、rules 等字段），调用 `readConfig` | 自动创建 `config.json`，删除 `config.yaml`，返回数据包含原 YAML 所有字段 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| YAML 自动迁移：空 YAML | 创建仅含 `schema:spec-driven` 的 `config.yaml`，调用 `readConfig` | 创建 `config.json`，内容为 `{"schema":"spec-driven"}`，删除 `config.yaml` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| YAML 自动迁移：YAML 包含非法值 | `config.yaml` 包含非法字段类型（如 `schema: 123`） | `readConfig` 抛出错误（parseConfig 校验失败），不删除 `config.yaml` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| 自动迁移幂等性：再次调用 readConfig | 迁移已完成后再次调用 `readConfig` | 直接读取已有的 `config.json`，不再次迁移 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| 点号路径访问顶层键 | `key = "schema"`，配置对象 `{ schema: "spec-driven" }` | 返回 `"spec-driven"` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| 点号路径访问两层嵌套 | `key = "test_scripts.unit"`，配置对象有嵌套 | 返回嵌套值 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| 点号路径访问深层嵌套 | `key = "a.b.c.d"`，配置对象有 4 层嵌套 | 返回最深层值 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| 点号路径中间节点缺失 | `key = "test_scripts.nonexistent"` | 返回 `{exists:false}`，不抛出异常 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| 点号路径根节点缺失 | `key = "unknown.key"` | 返回 `{exists:false}` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| 点号路径空字符串 | `key = ""` | 返回整个配置对象或抛出明确错误 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config/set 字符串值不再推断类型 | `value = "true"`（字符串） | 存储为字符串 `"true"` 而非布尔值 `true` | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` |
| config/set 数字值保持类型 | `value = 42`（数字） | 存储为数字 `42` | `openspec/changes/config-json-zod-schema/tests/commands/config-set.test.ts` |
| config/set 写入嵌套键自动创建中间对象 | `key = "a.b.c"`, `value = "x"`，配置无 `a` 键 | 自动创建 `{ a: { b: { c: "x" } } }` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config/unset 移除嵌套键保留兄弟键 | 移除 `test_scripts.unit`，`test_scripts` 下还有 `integration` | 仅移除 `unit`，保留 `integration` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config/unset 移除最后一个子键后父键为空 | 移除 `test_scripts` 下唯一子键 | `test_scripts` 变为空对象 `{}` | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| config/context 读取时 context 未设置 | config.json 中无 context 字段 | 返回 `{ context: "" }` | `openspec/changes/config-json-zod-schema/tests/commands/config-context.test.ts` |
| config/context 写入然后读取 | 先写入 "test context"，再读取 | 返回的内容与写入一致 | `openspec/changes/config-json-zod-schema/tests/commands/config-context.test.ts` |
| config/context 自动创建文件 | 在空项目目录写入 context | 自动创建 `openspec/config.json` | `openspec/changes/config-json-zod-schema/tests/commands/config-context.test.ts` |
| 写入非 ASCII 值（中文） | `value = "你好世界"` | JSON 文件正确写入（UTF-8 编码），重读后值不变 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| project_root 参数指向不存在的目录 | `project_root = "/tmp/nonexistent"` | 自动创建 `openspec/` 目录及 config.json，不抛出异常 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| project_root 参数包含相对路径 | `project_root = ".."` | 正确解析为绝对路径，在正确位置写入文件 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| JSON 序列化格式：2 空格缩进 | `writeConfig` 写入含嵌套对象的配置 | 生成的 JSON 使用 2 空格缩进，每行以 `\n` 结尾 | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| inferValue 不再导出 | 尝试从 `lib/config` 导入 `inferValue` | `inferValue` 为 `undefined`（未导出） | `openspec/changes/config-json-zod-schema/tests/lib/config.test.ts` |
| package.json 中 yaml 保留为运行时依赖 | 读取 `package.json` | `dependencies` 中包含 `yaml`（用于自动迁移路径） | `openspec/changes/config-json-zod-schema/tests/package-deps.test.ts` |

---

## 5. 测试数据

### 5.1 单元测试内联数据

点号路径解析测试使用以下内联配置对象（与旧版本一致，签名不变）：

```typescript
const sampleConfig = {
  schema: 'spec-driven',
  context: 'Tech stack: TypeScript, Node.js',
  static_check: ['npm run check', 'npm test'],
  test_scripts: {
    unit: 'npm run test:unit',
    integration: 'npm run test:integration',
    e2e: 'npm run test:e2e',
  },
  nested: {
    level1: {
      level2: {
        level3: 'deep-value',
      },
    },
  },
};
```

### 5.2 Zod Schema 验证测试数据

**合法配置对象集合**（所有应通过 `parseConfig`/`safeParseConfig` 校验）：

```typescript
const validConfigs = [
  // 最小配置（自动填充默认值）
  {},
  // 标准配置
  { schema: 'spec-driven', context: 'Tech stack: TypeScript' },
  // 含 rules 嵌套对象
  { schema: 'spec-driven', rules: { proposal: ['Keep it short'], tasks: ['One per file'] } },
  // 含扩展字段（passthrough）
  { schema: 'spec-driven', static_check: ['npm test'], custom_field: 'value' },
  // 全部字段
  {
    schema: 'spec-driven',
    context: 'test',
    rules: { proposal: ['p1'] },
    extra: 'allowed',
  },
];
```

**非法配置对象集合**（所有应被 `parseConfig`/`safeParseConfig` 拒绝）：

```typescript
const invalidConfigs = [
  // schema 字段类型错误
  { schema: 123 },
  { schema: null },
  { schema: true },
  { schema: ['spec-driven'] },
  // context 字段类型错误
  { schema: 'spec-driven', context: 42 },
  { schema: 'spec-driven', context: true },
  { schema: 'spec-driven', context: ['array'] },
  // rules 字段类型错误
  { schema: 'spec-driven', rules: 'string' },
  { schema: 'spec-driven', rules: 123 },
  // rules.proposal 类型错误
  { schema: 'spec-driven', rules: { proposal: 'not an array' } },
  { schema: 'spec-driven', rules: { proposal: [123, 456] } },
  // rules.tasks 类型错误
  { schema: 'spec-driven', rules: { tasks: 'not an array' } },
];
```

### 5.3 集成测试临时项目夹具

集成测试通过 `beforeAll` 在临时目录创建以下场景的场景：

**场景 A: 标准配置文件 (JSON)**
```
<tmpdir>/openspec/config.json
  {"schema": "spec-driven", "context": "Test project"}
```

**场景 B: 含 dev-team 扩展字段的配置文件**
```
<tmpdir>/openspec/config.json
  {
    "schema": "spec-driven",
    "static_check": ["npm run check", "npm test"],
    "test_scripts": {
      "unit": "npm run test:unit",
      "integration": "npm run test:integration"
    }
  }
```

**场景 C: 待迁移的旧 YAML 配置文件**
```
<tmpdir>/openspec/config.yaml
  schema: spec-driven
  context: "Legacy project"
  rules:
    proposal:
      - "Keep it short"
```

**场景 D: 空目录（无任何配置文件）**

**场景 E: 无效 JSON 配置文件**
```
<tmpdir>/openspec/config.json
  { invalid json content here
```

### 5.4 Schema 验证测试数据

针对各 config/* schema 的 Zod 验证（输入/输出 schema 保持不变，测试用例与 add-config-mcp-tools 阶段一致），构造以下输入：

| 场景 | Schema | 输入 | 预期 |
|------|--------|------|------|
| config/get 合法输入 | input schema | `{ key: "schema" }` | 通过验证 |
| config/get 带 project_root | input schema | `{ key: "schema", project_root: "/tmp" }` | 通过验证 |
| config/get 缺少 key | input schema | `{}` | Zod 验证失败 |
| config/get key 为空字符串 | input schema | `{ key: "" }` | Zod 验证失败（min(1)） |
| config/set 合法输入 | input schema | `{ key: "x", value: "y" }` | 通过验证 |
| config/set 嵌套路径 | input schema | `{ key: "a.b", value: 42 }` | 通过验证 |
| config/set 缺少 value | input schema | `{ key: "x" }` | Zod 验证失败 |
| config/unset 合法输入 | input schema | `{ key: "x" }` | 通过验证 |
| config/context 读取 | input schema | `{}` | 通过验证 |
| config/context 写入 | input schema | `{ context: "new context" }` | 通过验证 |

---

## 6. 不可测试项

- **Zod v4 包本身的校验正确性** — 原因: Zod 是经过广泛测试的第三方库（1000 万+ 周下载量）。我们的测试仅验证 schema 定义的正确性和 `parseConfig`/`safeParseConfig` 的行为是否符合规范，不测试 Zod 内部的校验逻辑。
- **JSON.parse / JSON.stringify 的原生行为** — 原因: 这两个函数是 Node.js 内置的 JavaScript 引擎实现，由 V8 团队保证正确性。测试中验证 JSON 输出的语法合法性和编码正确性（通过 `JSON.parse` 反向验证），但不测试引擎原生的 JSON 序列化/反序列化行为。
- **MCP 协议层传输正确性** — 原因: MCP SDK（`@modelcontextprotocol/sdk`）的序列化/反序列化和传输由第三方包处理，不属于我们的代码逻辑。工具注册后的 MCP 协议握手和 JSON-RPC 消息格式由 MCP SDK 保证，通过 code review 验证工具注册参数正确。
- **`resolveProjectRoot()` 的环境变量回退行为** — 原因: `process.env.CLAUDE_PROJECT_DIR` 和 `process.cwd()` 的值在测试环境中不可控。测试中统一使用显式传入的 `project_root` 参数，环境变量回退逻辑通过 code review 验证。
- **并发写入导致的数据竞争** — 原因: 数据竞争是多进程/多线程时序问题，无法在单线程的单元测试或集成测试中可靠复现。设计文档已指出 MCP 工具天然串行执行，通过架构保证而非测试来缓解此风险。
- **`yaml` 依赖保留的持久化验证** — 原因: `package-deps.test.ts` 已通过验证 `package.json` 中 `dependencies` 包含 `yaml`（AC-8）。实际的 `npm install` 安装成功与否取决于网络和 npm registry 可用性，不是代码逻辑的一部分，通过 CI 构建过程验证。
- **自动迁移过程中 YAML 注释的保留** — 原因: YAML 到 JSON 的转换策略决定注释不保留（JSON 标准不支持注释）。迁移日志中输出警告信息，此项属于用户体验而非功能正确性，通过代码审查验证。
