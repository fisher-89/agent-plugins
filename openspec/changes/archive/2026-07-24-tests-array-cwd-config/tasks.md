# 任务: tests-array-cwd-config

## 阶段 1: Schema 与常量

- [x] 新增 `plugins/dev-team/bin/src/schemas/config/defaults.ts`，导出 `TEST_COVERAGE_LINE_DEFAULT`（80）、`TEST_COVERAGE_BRANCH_DEFAULT`（70）、`TEST_COVERAGE_FUNCTION_DEFAULT`（75）、`TEST_MUTATION_SCORE_DEFAULT`（70）
- [x] 改写 `config.schema.ts`：删除顶层 `test` 对象及相关 override schema；新增 suite schema（必填 `root`+`framework`；`cwd` prefault `"."`；可选 `config`/`includes`/`excludes`/`coverage`/`mutation`）
- [x] 为 `root` 增加校验：拒绝包含 `*`、`?`、`{`、`[` 的字符串
- [x] suite 的 `coverage` / `mutation` `prefault` 引用 `defaults.ts` 常量；顶层 `tests` 使用 `prefault([])`
- [x] 导出 `TestSuite` 类型（`OpenSpecConfig['tests'][number]`）；必要时在 `schemas/index.ts` re-export
- [x] 更新 `dev-team-config.schema.json`：以 `tests` 数组镜像 Zod suite 字段与 `required`，移除受支持的旧 `properties.test` 形状

## 阶段 2: 框架 registry 与 config 注入占位

- [x] 在 `FrameworkConfig` 上增加 `config_flag: string | null`
- [x] 为 `jest` / `vitest` / `vite-plus` 设置 `config_flag: "--config"`，并在 `shell.test_execution` 与 `cmd.test_execution` 中嵌入 `{config_args}`（按段占位，非整段末尾）
- [x] 为 `bun` / `rust` / `node-test` / `go` / `pytest` 设置 `config_flag: null`（模板可不含占位符）

## 阶段 3: test-exclude 改为 suite 语义

- [x] 重写 `getExcludeGlobs`：从 `config.tests[].excludes` 拼出相对 projectRoot 的模式（`root` + exclude glob，规范化 `.`/`..`），去重后返回
- [x] 重写 `isFileExcluded`：仅当文件落在对应 suite `root` 下且匹配该 suite scoped exclude 时返回 `true`；不再读取 `config.test`
- [x] 更新模块头注释，说明消费者与 `tests[].excludes` 语义

## 阶段 4: test-detect-frameworks 读 tests[] 构建 plan

- [x] 删除 `deriveWorkingDirectory` 及旧 `normalizeFrameworks(framework, overrides)` 调用路径
- [x] 实现从 `config.tests` 构建 plan：计算 `absRoot` / `absCwd` / 可选 `absConfig`；`directory` 为 absCwd 相对 projectRoot（POSIX）
- [x] `mutation_score` 取自解析后 suite 的 `mutation.score`；覆盖率元数据与 `mutation_framework` 仍来自 `getFrameworkConfig`
- [x] script 生成时展开 `{config_args}`：有 `config` 且 `config_flag` 非空 → `` `${config_flag} ${relative(absCwd, absConfig)}` ``；无 `config` → 空串；有 `config` 但无 flag → 抛明确错误
- [x] 文件→框架匹配改为 suite scope：`under(root) ∧ match(includes ?? default_glob) ∧ ¬excludes`；数组顺序优先
- [x] 检测路径继续通过 `isFileExcluded`（或等价 scope 判定）跳过排除文件

## 阶段 5: test-resolve-paths 与用户可见文案

- [x] `processEmptyModules`：扫描根改为各 suite 的 `root`（保证覆盖 scope，不因 `cwd` 上移误扫/漏扫）；收集源文件后应用 scope（`includesEffective = includes ?? default_glob`）/ `isFileExcluded`
- [x] 无 plan 时错误消息改为引导配置 `openspec/config.json` 的 `tests`
- [x] 更新 `test-execution.ts` 无配置日志：提及 `tests` 而非 `test.framework`
- [x] 更新 `mcp.ts` 中 `test_detect_frameworks` 工具描述：改为 `tests` suite 映射

## 阶段 6: 报告与变异执行适配

- [x] `test-report.ts`：移除全局 `test.coverage` + `test.overrides` 级联；按 `config.tests` 匹配 suite 读取覆盖率阈值
- [x] `test-report.ts`：mutation 分组（原 `computeMutationOverrides`）改为 suite 维度阈值与 scope 匹配；不再读 `config.test`
- [x] 删除或停用 `test-report.ts` 内与 schema 不一致的手写默认阈值常量级联（改用 parse 后 suite 值）
- [x] `test-runner.ts`：变异阶段继续以 `projectRoot/entry.directory`（absCwd）为 cwd；确保传入 `resolveStrykerConfig` 的源路径相对 absCwd
- [x] 核对 `stryker-config.ts`：临时配置与 `mutate` 路径相对 `rootPath`；若有相对 projectRoot 残留则修正

## 阶段 7: 仓库配置、文案与版本

- [x] 将 `openspec/config.json` 从 `test.overrides` 迁为 `tests[]`（`root: "plugins/dev-team/bin"`, `framework: "vite-plus"`, 显式 `includes`/`excludes`/`config` 等，对齐 proposal 示例）
- [x] 更新 `plugins/dev-team/agents/test-execution-executor.md` 等仍提及 `test.coverage` / `test.framework` / `test.overrides` 的表述为 `tests` / suite 字段
- [x] 按项目规则升级 `plugins/dev-team/.claude-plugin/plugin.json` 的 `version`
