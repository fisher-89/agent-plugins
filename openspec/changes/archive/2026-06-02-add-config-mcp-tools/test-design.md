# 测试设计: add-config-mcp-tools

> **变更**: add-config-mcp-tools
> **日期**: 2026-06-01
> **基于**: proposal.md, design.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | `lib/config.ts` 的 YAML 读写、点号路径访问、类型推断、文件骨架创建；四个 command（config-get/config-set/config-unset/config-context）的业务逻辑；各 config/* schema 的 Zod 输入/输出验证 | vite-plus (vitest compatible) | 核心 YAML 操作和命令层全分支覆盖 >= 90%；确保所有输入验证、路径解析、类型转换、文件缺失处理等在隔离环境中通过测试 |
| 集成测试 | 临时目录下的真实 YAML 文件读写，验证 `lib/config.ts` 的各函数在真实文件系统中的完整行为（自动创建目录、骨架文件、YAML 语法合法性、注释保留等） | vite-plus (vitest compatible) | 覆盖真实文件系统的路径创建、YAML 序列化/反序列化边界以及文件持久化与重读的完整性 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 单元测试 | `readConfig` 读取标准字段，返回解析后的 JS 对象 |
| AC-1 | `openspec/changes/add-config-mcp-tools/tests/commands/config-get.test.ts` | 单元测试 | config/get 命令调用 `readConfig` 后返回 `{ key, value, exists }` |
| AC-2 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 单元测试 | `getValueByDotPath` 点号路径遍历：`test_scripts.unit` 返回正确的嵌套值 |
| AC-2 | `openspec/changes/add-config-mcp-tools/tests/commands/config-get.test.ts` | 单元测试 | config/get 处理含点号的 key 参数，将其传递给点号路径解析 |
| AC-3 | `openspec/changes/add-config-mcp-tools/tests/commands/config-get.test.ts` | 单元测试 | config/get 对于不存在的 key 返回 `{ key, exists: false }`，value 为 undefined 或 null |
| AC-3 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 单元测试 | `getValueByDotPath` 对于不存在的路径返回 undefined |
| AC-4 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 集成测试 | `writeConfig` 写入新键后读取，确认文件内容正确包含该键 |
| AC-4 | `openspec/changes/add-config-mcp-tools/tests/commands/config-set.test.ts` | 单元测试 | config/set 处理参数后调用 writeConfig 写入 |
| AC-5 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 集成测试 | 写入后使用 `yaml` 包重新解析文件，无语法错误 |
| AC-6 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 集成测试 | `unsetConfig` 移除键后文件内容不再包含该键 |
| AC-6 | `openspec/changes/add-config-mcp-tools/tests/commands/config-unset.test.ts` | 单元测试 | config/unset 命令调用 `unsetConfig` 并返回 `{ key, removed: true }` |
| AC-7 | `openspec/changes/add-config-mcp-tools/tests/commands/config-unset.test.ts` | 单元测试 | config/unset 对于不存在的键返回 `{ key, removed: false }` |
| AC-7 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 单元测试 | `unsetConfig` 移除不存在的键返回 `{ removed: false }` |
| AC-8 | `openspec/changes/add-config-mcp-tools/tests/commands/config-context.test.ts` | 单元测试 | config/context 无参数时调用 `readConfig` 读取 `context` 字段并返回 |
| AC-9 | `openspec/changes/add-config-mcp-tools/tests/commands/config-context.test.ts` | 单元测试 | config/context 带参数时调用 `writeConfig` 写入 `context` 字段并返回 `written: true` |
| AC-10 | `openspec/changes/add-config-mcp-tools/tests/commands/config-get.test.ts` | 单元测试 | config/get 使用 `project_root` 参数覆盖默认项目根目录，在非默认路径下读写 |
| AC-10 | `openspec/changes/add-config-mcp-tools/tests/commands/config-set.test.ts` | 单元测试 | config/set 使用 `project_root` 参数覆盖默认项目根目录 |
| AC-10 | `openspec/changes/add-config-mcp-tools/tests/commands/config-unset.test.ts` | 单元测试 | config/unset 使用 `project_root` 参数覆盖默认项目根目录 |
| AC-10 | `openspec/changes/add-config-mcp-tools/tests/commands/config-context.test.ts` | 单元测试 | config/context 使用 `project_root` 参数覆盖默认项目根目录 |
| AC-11 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` | 集成测试 | 在空目录下调用 `writeConfig`，自动创建 `openspec/config.yaml` 含 `schema: spec-driven` |
| AC-11 | `openspec/changes/add-config-mcp-tools/tests/commands/config-set.test.ts` | 集成测试 | 通过 config/set 在空项目目录写入，确认文件被自动创建 |
| AC-12 | `openspec/changes/add-config-mcp-tools/tests/package-deps.test.ts` | 构建验证 | 在 package.json 中检查 `yaml` 依赖已添加（通过 code review 和 npm install 验证） |
| 幂等性-1 | `openspec/changes/add-config-mcp-tools/tests/commands/config-set.test.ts` | 集成测试 | config/set 同一 key/value 重复调用两次，两次返回结果一致（value 相同，无累积副作用） |
| 幂等性-2 | `openspec/changes/add-config-mcp-tools/tests/commands/config-set.test.ts` | 集成测试 | config/set 同一 key 先后设置不同 value，第二次正确覆盖（非追加/重复条目） |
| 幂等性-3 | `openspec/changes/add-config-mcp-tools/tests/commands/config-unset.test.ts` | 集成测试 | config/unset 对已移除的 key 再次调用，返回 `{ key, removed: false }` |
| 幂等性-4 | `openspec/changes/add-config-mcp-tools/tests/commands/config-unset.test.ts` | 集成测试 | config/unset 对同一 key 调用两次，第一次返回 `removed: true`，第二次返回 `removed: false` |

---

## 3. 测试策略

### 3.1 方法

采用"三层"测试策略：

1. **单元测试（核心逻辑）**：针对 `lib/config.ts` 中可独立测试的工具函数——点号路径解析、类型推断、YAML 对象读取——使用内存中的 YAML 字符串或 mock 配置对象进行测试，不依赖真实文件系统。
2. **单元测试（命令层）**：针对四个 command 函数的业务逻辑，将 `lib/config.ts` 的能力作为可调用函数进行测试。命令层主要职责是参数解析、调用 lib 函数、格式化响应。
3. **集成测试（文件系统）**：针对 `lib/config.ts` 中需要真实文件系统的操作（文件创建、YAML 序列化/反序列化、持久化验证），使用 `fs.mkdtempSync` 创建临时目录，模拟完整的 config.yaml 读写生命周期。

### 3.2 测试分类

- **单元测试**: 不依赖真实文件系统或外部服务的测试。点号路径解析、类型推断函数、命令层参数处理和响应构建。使用 mock 的配置对象或 YAML 解析结果，通过模拟 `lib/config.ts` 的返回值来隔离命令层测试。

- **集成测试**: 依赖真实文件系统的测试。在 `beforeAll` 中使用 `fs.mkdtempSync` 创建临时目录，通过 `lib/config.ts` 的导出的函数执行 YAML 文件的创建、读取、写入、删除，并在 `afterAll` 中清理临时目录。验证 YAML 语法合法性、注释保留、骨架文件创建。

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock 或临时文件系统，不依赖真实外部环境。

### 3.3 模拟策略

| 模拟对象 | 策略 | 适用测试 |
|----------|------|----------|
| 配置文件路径 | 通过 `project_root` 参数指向 `fs.mkdtempSync` 创建的临时目录 | 全部集成测试 |
| 文件不存在场景 | 测试在空临时目录下调用 config 函数，自动创建过程由真实文件系统验证 | config.test.ts 的集成测试部分 |
| 点号路径解析 | 直接构造嵌套 JS 对象作为输入，测试 `getValueByDotPath` / `setValueByDotPath` / `unsetValueByDotPath` 的路径遍历逻辑 | config.test.ts 的纯逻辑单元测试 |
| 类型推断 | 构造字符串值作为输入测试 `coerceValue` 函数，验证其将 `"true"` 转 boolean、数字字符串转 number 等行为 | config.test.ts 的纯逻辑单元测试 |
| 命令层与 lib 的交互 | 命令层测试中 mock `lib/config.ts` 的导出函数（如 `readConfig`），验证命令层正确传递参数和格式化响应；集成测试中使用真实 lib 函数 | 命令层的四个 `*.test.ts` |
| project_root 解析 | 使用临时目录路径作为 `project_root` 参数传入命令层和 lib 函数，验证路径正确性 | 所有涉及 project_root 的测试 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| config.yaml 不存在（空项目） | 在空目录下调用 `readConfig` | 返回空对象 `{}`，不抛出异常 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config.yaml 不存在时自动创建骨架文件 | 在空目录下调用 `writeConfig` 写入 `test_scripts.unit` | 自动创建 `openspec/config.yaml`，内容包含 `schema: spec-driven` 及写入的值 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config.yaml 不存在时 config/set 自动创建 | 在空项目目录调用 config/set，写入 `static_check[0]="npm test"` | 文件被自动创建，含 `schema: spec-driven` 和写入的 `static_check` 数组 | `openspec/changes/add-config-mcp-tools/tests/config-set-skeleton.test.ts` |
| config.yaml 格式错误导致 YAML 解析失败 | config.yaml 包含无效 YAML 语法（如制表符缩进） | `readConfig` 抛出清晰的错误信息，包含文件路径和解析错误详情 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| 点号路径访问顶层键 | `key = "schema"`，配置对象 `{ schema: "spec-driven" }` | 返回 `"spec-driven"` | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| 点号路径访问两层嵌套 | `key = "test_scripts.unit"`，配置对象有嵌套 `test_scripts` | 返回 `test_scripts.unit` 的值 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| 点号路径访问深层嵌套 | `key = "a.b.c.d"`，配置对象有 4 层嵌套 | 返回最深层值 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| 点号路径中间节点缺失 | `key = "test_scripts.nonexistent"`，`test_scripts` 存在但无 `nonexistent` 子键 | 返回 undefined，不抛出异常 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| 点号路径根节点缺失 | `key = "unknown.key"`，整个顶层键不存在 | 返回 undefined | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| 点号路径空字符串 | `key = ""` | 返回整个配置对象或抛出明确错误 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/set 类型推断：字符串 "true"/"false" | `value = "true"` | 存储为 boolean `true` | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/set 类型推断：数字字符串 | `value = "42"` | 存储为 number `42` | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/set 类型推断：类 JSON 数组字符串 | `value = "[1, 2, 3]"` | 存储为数组 `[1, 2, 3]` | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/set 类型推断：非 JSON 的方括号字符串 | `value = "[not json"` | 保留为原始字符串 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/set 类型推断：已为 boolean 的值 | `value = true`（非字符串） | 保持 boolean 不变 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/set 写入嵌套键自动创建中间对象 | `key = "a.b.c"`, `value = "x"`，配置无 `a` 键 | 自动创建 `{ a: { b: { c: "x" } } }`，不抛出异常 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/unset 移除嵌套键的父键 | 移除 `test_scripts.unit`，`test_scripts` 下还有 `integration` 和 `e2e` | 仅移除 `unit` 子键，保留 `integration` 和 `e2e` | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/unset 移除整个对象最后一个子键后父键为空 | 移除 `test_scripts` 下唯一子键 `unit` | `test_scripts` 变为空对象 `{}`（或清理父键——依实现决定） | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/context 读取时 context 未设置 | config.yaml 中无 context 字段 | 返回 `{ context: null }` 或空字符串，不抛出异常 | `openspec/changes/add-config-mcp-tools/tests/commands/config-context.test.ts` |
| config/context 写入然后读取 | 先写入 "test context"，再读取 | 返回的内容与写入一致 | `openspec/changes/add-config-mcp-tools/tests/commands/config-context.test.ts` |
| config/set 写入后保留现有注释 | config.yaml 包含注释行，写入新键后重新读取 | 注释行未被覆盖或移除（通过 `yaml` 包的 `keepSourceTokens` 保证） | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| 写入非 ASCII 值（中文） | `value = "你好世界"` | YAML 文件正确写入并编码，重读后值不变 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| project_root 参数指向不存在的目录 | `project_root = "/tmp/nonexistent"` | 自动创建 `openspec/` 目录及 config.yaml，不抛出异常 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| project_root 参数包含相对路径 | `project_root = ".."` | 正确解析为绝对路径 | `openspec/changes/add-config-mcp-tools/tests/commands/config-get.test.ts` |
| 字符串值包含特殊 YAML 字符（冒号、引号、特殊符号） | `value = "key: value with: colons"` | YAML 正确转义，重读后值不变 | `openspec/changes/add-config-mcp-tools/tests/lib/config.test.ts` |
| config/set 幂等性：同一 key/value 重复调用 | `key="schema"`, `value="spec-driven"` 连续调用两次 | 两次返回结果完全一致，文件内容无重复条目 | `openspec/changes/add-config-mcp-tools/tests/commands/config-set.test.ts` |
| config/set 覆盖写入：不同值连续写入同一 key | 第一次 set `key="schema"` `value="v1"`，第二次 set `key="schema"` `value="v2"` | 第二次结果正确反映 v2，文件最终值为 `v2` 而非 `[v1, v2]` | `openspec/changes/add-config-mcp-tools/tests/commands/config-set.test.ts` |
| config/unset 幂等性：对已移除的 key 再次调用 | 先 unset `"test_scripts.unit"`，再对同一路径调用 unset | 第二次返回 `removed: false`，不抛出异常 | `openspec/changes/add-config-mcp-tools/tests/commands/config-unset.test.ts` |
| config/unset 两次调用同一 key | 对 `"test_scripts.unit"` 连续调用两次 unset | 第一次返回 `removed: true`，第二次返回 `removed: false`，文件状态不再变化 | `openspec/changes/add-config-mcp-tools/tests/commands/config-unset.test.ts` |

---

## 5. 测试数据

### 5.1 单元测试内联数据

点号路径解析测试使用以下内联配置对象：

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

类型推断测试使用以下值矩阵：

| 原始值 (string) | 预期类型 | 预期值 |
|-----------------|----------|--------|
| `"true"` | boolean | `true` |
| `"false"` | boolean | `false` |
| `"42"` | number | `42` |
| `"3.14"` | number | `3.14` |
| `"[1,2,3]"` | array | `[1, 2, 3]` |
| `'["a","b"]'` | array | `["a", "b"]` |
| `"hello"` | string | `"hello"` |
| `"[not json"` | string | `"[not json"` |
| `"true story"` | string | `"true story"` |
| `true` (已 boolean) | boolean | `true` |
| `42` (已 number) | number | `42` |

### 5.2 集成测试临时项目夹具

集成测试通过 `beforeAll` 在临时目录创建以下场景的场景：

**场景 A: 标准配置文件**
```
<tmpdir>/openspec/config.yaml
  schema: spec-driven
  context: "Test project"
```

**场景 B: 含 dev-team 扩展字段的配置文件**
```
<tmpdir>/openspec/config.yaml
  schema: spec-driven
  static_check:
    - npm run check
    - npm test
  test_scripts:
    unit: npm run test:unit
    integration: npm run test:integration
```

**场景 C: 带注释的配置文件**
```
<tmpdir>/openspec/config.yaml
  schema: spec-driven

  # dev-team extensions (managed by config/* tools)
  static_check:
    - npm run check
```

**场景 D: 空目录（无 config.yaml）**

### 5.3 Schema 验证测试数据

针对各 config/* schema 的 Zod 验证，构造以下输入：

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

- **yaml npm 包本身的 YAML 解析和序列化正确性** — 原因: 该包是经过广泛测试的第三方依赖（5000 万+ 周下载量），其解析行为由该包自身的测试套件覆盖。我们的测试仅验证集成场景下的调用结果是否与预期一致，不测试 yaml 包的内部逻辑。
- **yaml 包的 `keepSourceTokens` 注释保留行为的精确性** — 原因: 注释保留是 `yaml` 包的实现细节，在不同版本间可能有差异。测试覆盖注释在典型写入操作后是否保留（定性验证），但不测试逐行注释的精确匹配。
- **MCP 协议层传输正确性** — 原因: MCP SDK（`@modelcontextprotocol/sdk`）的序列化/反序列化和传输由第三方包处理，不属于我们的代码逻辑。工具注册后的 MCP 协议握手和 JSON-RPC 消息格式由 MCP SDK 保证，通过 code review 验证工具注册参数正确。
- **`resolveProjectRoot()` 的环境变量回退行为** — 原因: `process.env.CLAUDE_PROJECT_DIR` 和 `process.cwd()` 的值在测试环境中不可控。测试中统一使用显式传入的 `project_root` 参数，环境变量回退逻辑通过 code review 验证。
- **并发写入导致的数据竞争** — 原因: 数据竞争是多进程/多线程时序问题，无法在单线程的单元测试或集成测试中可靠复现。设计文档已指出 MCP 工具天然串行执行，通过架构保证而非测试来缓解此风险。
- **`yaml` 依赖安装成功的自动化验证** — 原因: `npm install` 的成功与否取决于网络和 npm registry 可用性，不是代码逻辑的一部分。通过 AC-12 在 CI 构建过程中验证依赖安装，不纳入自动化测试套件。
