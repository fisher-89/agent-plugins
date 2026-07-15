# 提案: test-exclude-source-globs

> **变更**: test-exclude-source-globs
> **日期**: 2026-07-14
> **状态**: 草稿

---

## 问题

当前所有匹配测试框架 glob 的源文件都会被纳入测试管线的全部阶段：测试设计、测试文件生成和变异测试。用户无法排除特定文件（例如自动生成的代码、类型声明文件 `.d.ts`、vendor 代码等），导致以下问题：

1. **生成的测试文件无效** — 对自动生成的代码生成测试文件没有意义，浪费 LLM token 和开发者时间
2. **变异测试噪音** — 生成的代码被变异测试覆盖产生低分，但这不代表业务代码质量
3. **人工跳过成本高** — 开发者在每次变更后需要手动识别并跳过应当排除的文件

需要一个声明式的配置机制，让用户指定应排除在测试管线之外的源文件路径模式。

---

## 提案

在 `openspec/config.json` 的 `test` 配置和 `test.overrides` 子配置中各增加一个 `exclude` 字段，接受字符串数组（glob 模式）。被排除的源文件完全豁免于测试管线：

- 不出现在文件-框架检测结果中
- 不推导其单元测试路径
- 不参与变异测试（StrykerJS）

核心设计决策：

1. `exclude` 在 `test` 层 = 全局排除规则，在任何 override 匹配之前生效
2. `test.overrides[].exclude` = 仅作用于该 override 的 `file` glob 范围内的排除规则
3. 全局 + override 的 exclude 列表以并集(union)方式合并
4. 过滤逻辑抽离为共享工具函数 `lib/test-exclude.ts`，被三个消费者共用
5. Agents（test-design-planner, test-gen-generator）无需修改 — 它们通过 MCP 工具获取已过滤的数据

配置示例：

```jsonc
{
  "test": {
    "framework": "vitest",
    "exclude": ["**/generated/**", "**/*.d.ts"],
    "overrides": [
      {
        "file": "plugins/dev-team/bin",
        "framework": "vite-plus",
        "exclude": ["**/vendor/**"]
      }
    ]
  }
}
```

上述配置效果：
- 全局排除 `**/generated/**` 和 `**/*.d.ts`
- 在 `plugins/dev-team/bin` override 范围内额外排除 `**/vendor/**`
- `plugins/dev-team/bin/vendor/` 下的文件被双重排除（全局 + override）

---

## 能力

### 新增能力

- **test-exclude-filter** — 共享工具函数 `isFileExcluded(filePath, config)` 和 `getExcludeGlobs(config)`，用判断源文件是否被 `test.exclude` 或 `test.overrides[].exclude` 排除。同时消费端 `test-runner` 在变异测试前使用该工具过滤源文件

### 修改的能力

- **config-schema** — Zod schema 和 JSON Schema 的 `test` 和 `test.overrides` 对象各增加可选 `exclude: string[]` 字段
- **test-detect-frameworks** — `detectFrameworksForFiles` 在检测文件-框架映射时，跳过被排除的源文件，使其不进入 `detected[]` 结果
- **test-path-resolver** — `resolveTestPaths` 在推导单元测试路径时，跳过被排除的源文件，使其不进入 `unit_tests[]` 结果

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/test-exclude.ts` — 新增共享排除过滤工具模块
- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — 在 `test` 和 `test.overrides` Zod schema 中增加 `exclude` 字段
- `plugins/dev-team/bin/dev-team-config.schema.json` — 在 `test` 和 `test.overrides` JSON Schema 中增加 `exclude` 字段
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` — 在 `detectFrameworksForFiles` 中调用 `isFileExcluded` 过滤
- `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` — 在 `processNonEmptyModules` 和 `processEmptyModules` 中调用 `isFileExcluded` 过滤
- `plugins/dev-team/bin/src/lib/test-runner.ts` — 在 `runMutationPhase`/`deriveSourceFiles` 阶段调用 `isFileExcluded` 过滤源文件

### 测试文件

- `plugins/dev-team/bin/src/lib/test-exclude.test.ts` — 新增工具函数测试
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` — 新增 exclude 过滤场景
- `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` — 新增 exclude 过滤场景
- `plugins/dev-team/bin/src/lib/test-runner.test.ts` — 新增 exclude 过滤场景

### 不要修改

- Agents（test-design-planner, test-gen-generator, implementation-generator 等）— 它们通过 MCP 工具获取已过滤的数据，无需变更
- `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` — 与本变更无关
- `plugins/dev-team/bin/src/lib/test-framework.ts` — 框架注册表，不变
- 其他未列出的 CLI 命令和 MCP 工具

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | config-schema 支持 test.exclude | `test.exclude` 接受 glob 模式字符串数组；`test.overrides[].exclude` 同样有效；两者在 TypeScript 类型 `OpenSpecConfig` 中为可选字段 |
| AC-2 | 共享 exclude 过滤函数 | `isFileExcluded` 在文件匹配任意 exclude glob 时返回 `true`；glob 匹配复用现有 `picomatch` + `matchGlob` 实现；`getExcludeGlobs` 正确合并全局和 override 的 exclude 列表 |
| AC-3 | test-detect-frameworks 排除 | 配置 `test.exclude` 后，被排除的文件不应出现在 `detected[]` 数组中；未被排除的文件行为不变 |
| AC-4 | test-resolve-paths 排除 | 配置 `test.exclude` 后，被排除的文件不应出现在 `unit_tests[]` 数组中；在空 modules 自动扫描模式下同样生效；未被排除的文件行为不变 |
| AC-5 | test-runner 突变排除 | 配置 `test.exclude` 后，被排除的源文件不应出现在 StrykerJS 的变异目标列表中；未配置 exclude 时所有源文件正常运行 |
| AC-6 | 向后兼容 | 不配置 `test.exclude` 和 `test.overrides[].exclude` 时，全部现有测试通过，行为不变 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Glob 匹配与现有 `matchGlob` 行为不一致 | 错误排除文件或漏排除 | 低 | 直接复用现有的 `matchGlob` 实现，不重新发明；添加覆盖边界用例的测试 |
| 大量 exclude glob 导致性能下降 | 文件检测/路径解析延迟增加 | 低 | exclude glob 解析在函数调用时统一完成，后续 O(n) 线性扫描。预期用户配置不超过 20 条 |
| 用户误排除全部源文件 | 无测试执行 | 低 | 属于配置错误；工具层面不设防护，仅通过测试日志提示无检测结果 |
| Override 级别 exclude 与全局 exclude 语义混淆 | 用户预期行为与实际不符 | 中 | 文档明确：union 语义，override exclude 仅在该 override 的 file 范围内额外排除 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| exclude 放在 `test` 层还是新增 `test.run` 子对象 | `test` 层 | 与 `framework`、`coverage`、`mutation` 同级，结构简洁，无嵌套膨胀 | 嵌套在 `test.run.exclude` 下 — 过度设计 |
| Override 级别是否支持 exclude | 支持 | 用户可能只对特定目录（如 vendor）排除文件，不应强制全局生效 | 仅全局 exclude — 使用场景受限 |
| 过滤逻辑应抽取共享函数还是各模块独立实现 | `isFileExcluded` 共享函数 | 三处消费逻辑相同，DRY 原则 | 各模块独立实现 — 违反 DRY，维护成本高 |
| Exclude 是否影响 mutation 以外的阶段 | 是，影响所有阶段 | 排除的文件没有测试设计、测试文件、变异测试的必要 | 仅过滤 mutation — 语义不一致，用户仍需手动跳过 |
| Agents 是否需感知 exclude | 否 | Agents 通过 MCP 工具获取数据，MCP 工具已过滤排除文件，Agent 无需感知 | Agents 自行过滤 — 更多代码、更高的 Agent 错误风险 |

### 待决问题

- 无
