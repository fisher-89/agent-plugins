# 提案: test-resolve-paths-config-dirs

> **变更**: test-resolve-paths-config-dirs
> **日期**: 2026-06-26
> **状态**: 草稿

---

## 问题

`test_resolve_paths` 是纯路径推导工具，当前存在以下不足：

1. 调用方必须自行获取源文件目录信息，增加集成负担
2. 无法复用 `config.json` 中已有的 `test.framework` 和 `test.overrides` 配置——即使调用方提供了 `modules`，也不会根据测试配置过滤，可能为不在测试范围内的文件生成测试路径
3. `modules` 为空数组时直接由 Zod schema 拒绝（`.min(1)`），没有降级或自动推导逻辑
4. 在 CI/工作流场景中，调用方需要先手动运行 `git diff` 获取变更文件，再传入 `modules`，流程繁琐

---

## 提案

三项核心变更：

### 变更 1: 移除 `modules` 空数组限制，允许空数组触发 config-driven 自动扫描

当 `modules` 为空时，自动调用 `runTestDetectFrameworks` 获取配置计划（`plan`），使用 `plan[].directory` 作为扫描目录列表来发现源文件。

### 变更 2: modules 非空时也根据测试配置过滤

即使调用方显式传入 `modules`，`test_resolve_paths` 也 SHOULD 调用 `runTestDetectFrameworks({ files: modules })` 获取文件→框架的检测结果。仅对能匹配到测试框架配置的文件推导单元测试路径，不在任何测试配置范围内的文件 SHOULD 被跳过（或写入 `errors` 提示）。

这确保即使调用方随意传入了源文件列表，返回的测试路径也只包含项目测试配置覆盖范围内的文件。

### 变更 3: modules 扩展为 union 类型，支持 `"git-change"` 字面量

`modules` 字段接受两种类型：
- `string[]` — 文件或目录路径列表（可为空数组）
- `"git-change"` — 触发 `git diff HEAD --name-only` 读取当前工作树变更文件，作为 `modules` 列表

当 `modules` 为 `"git-change"` 时，行为等价于：先运行 `git diff HEAD --name-only` 获取变更文件列表，再以该列表作为 `modules` 传入（同样受测试配置过滤）。

优先级链（适用于 config-driven 目录推导，即 `modules: []` 时）：
1. `test.overrides[].file` （显式配置的覆盖规则）
2. `test.framework` （全局默认框架的默认 glob）
3. 若两者均未配置 → 在 `errors` 中添加提示信息，指导用户在 `openspec/config.json` 中配置 `test.framework` 或 `test.overrides`

核心变更：
- 修改 `test-resolve-paths.schema.ts`：`modules` 从 `z.array(z.string()).min(1)` 改为 `z.union([z.array(z.string()), z.literal("git-change")])`
- 修改 `test-resolve-paths.ts`：
  - `modules` 为空时调用 `runTestDetectFrameworks({})` 获取 plan 并扫描目录
  - `modules` 非空时调用 `runTestDetectFrameworks({ files: modules })` 获取检测结果，过滤掉不匹配的文件
  - `modules` 为 `"git-change"` 时先执行 `git diff HEAD --name-only` 获取文件列表，后续同非空 modules 处理
- 不新增配置项，复用现有的 `test.overrides` 和 `test.framework` 配置
- 不修改 `test_detect_frameworks` 自身的公共 API 与行为

---

## 能力

### 修改的能力

- **test-path-resolver** — 为 `test_resolve_paths` 增加 config-driven 过滤、空 modules 自动扫描、以及 `"git-change"` 快捷输入能力

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` — 增加 test config 过滤逻辑（非空 modules）、config-driven 目录扫描（空 modules）、`"git-change"` 模式（git diff → 文件列表）
- `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` — 将 `modules` 从 `z.array(z.string()).min(1)` 改为 `z.union([z.array(z.string()), z.literal("git-change")])`
- `plugins/dev-team/bin/src/mcp.ts` — 更新 `test_resolve_paths` 工具的 description，反映新行为

### 测试文件

- `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.test.ts` — 更新测试用例：`modules: []` 应通过验证，新增 `modules: "git-change"` 正向测试，确认 `modules: 123` 等非合法值被拒绝
- `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` — 新增 config-driven 过滤测试、空 modules 自动扫描测试、`"git-change"` 模式测试

### 不要修改

- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` — 不修改 test_detect_frameworks 的公共 API 与内部逻辑，仅调用其已有接口
- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — 不新增配置项
- `plugins/dev-team/bin/src/lib/config.ts` — 不修改 config 读取逻辑
- `openspec/config.json` — 配置格式已满足需求，无需变更

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | modules 为空时调用 test_detect_frameworks 获取扫描目录 | 当 `modules: []` 且 config.json 中配置了 `test.overrides`（如 `plugins/dev-team/bin` → `vite-plus`）时，`unit_tests` 包含该目录下的源文件推导结果 |
| AC-2 | 无 test 配置时给出指导性错误 | 当 `modules: []` 且 config.json 中既无 `test.framework` 也无 `test.overrides` 时，`errors` 包含提示用户配置 framework 的消息，`unit_tests` 为空 |
| AC-3 | modules 非空时根据 test config 过滤 | 当 `modules: ["src/config.ts", "scripts/not-in-test-scope.ts"]` 且 test config 仅覆盖 `src/` 时，`unit_tests` 仅包含 `src/config.ts` 的推导结果，`scripts/not-in-test-scope.ts` 不出现在 `unit_tests` 中 |
| AC-4 | 输入 schema 接受空数组和 `"git-change"` | `testResolvePathsInputSchema` 接受 `{modules: []}` 和 `{modules: "git-change"}`，拒绝 `{modules: 123}` 等非法类型 |
| AC-5 | `"git-change"` 返回变更文件的单测路径 | 当 `modules: "git-change"` 且 `git diff HEAD --name-only` 返回 `["src/foo.ts"]` 时，`unit_tests` 包含 `src/foo.ts` 的推导结果 |
| AC-6 | 扫描目录 / 文件去重 | 当多个 override 指向同一目录，或 `modules` 和 git diff 产生重复文件时，`unit_tests` 中的源文件不重复 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 现有调用方依赖 modules 必填行为 | 将空 modules 从「静默拒绝」改为「自动推导」，可能改变部分调用方预期 | 低 | 空 modules 在现有 schema 下被拒绝，无生产调用方依赖此行为 |
| modules 非空时过滤行为变更 | 原来传入任意文件都会返回测试路径，现在仅返回 test config 覆盖范围内的 | 中 | 在 design 中明确此行为变更；调用方可通过 results.errors 感知被跳过的文件 |
| test_detect_frameworks 的 plan 为空但无错误 | 空扫描目录导致 unit_tests 为空，调用方误以为无源文件 | 低 | 明确在 errors 中添加指导性消息，告知用户配置 test.framework 或 test.overrides |
| 循环依赖（test_resolve_paths 调用 test_detect_frameworks） | 间接循环依赖增加维护复杂度 | 低 | test_detect_frameworks 是纯数据查询（读 config.json + 返回 plan），不依赖 test_resolve_paths，无循环风险 |
| git diff 在非 git 仓库中失败 | `"git-change"` 模式无法获取文件列表 | 低 | 捕获 git 命令异常，在 errors 中返回清晰错误消息 |
| git diff 返回大量文件 | 生成过多测试路径 | 低 | 仍受 test config 过滤；git diff 仅在用户主动使用 `"git-change"` 时触发 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 空 modules 时如何确定扫描范围 | 方案 B：调用 test_detect_frameworks 获取 plan[].directory | 复用现有 normalizeFrameworks() 链，不新增配置项 | 方案 A：新增独立配置项；方案 C：约定默认目录 |
| modules 非空时是否过滤 | 是，调用 test_detect_frameworks({ files }) 过滤 | 确保输出始终在测试配置覆盖范围内，避免为无关文件生成测试路径 | 方案 A：保持原样不过滤 — 违背"测试配置是唯一真相来源"原则 |
| git diff 范围 | `git diff HEAD --name-only`（工作树 vs HEAD 的全部变更） | 覆盖 staged + unstaged 变更，是大多数 CI 场景的预期行为 | `git diff --name-only`（仅 unstaged）— 遗漏已暂存文件 |
| `"git-change"` 失败行为 | 捕获异常，在 errors 中返回消息 | 不中断调用方流程，保持与现有错误处理一致 | 抛出异常中断 — 与现有 errors 收集模式不一致 |

### 待决问题

- 无
