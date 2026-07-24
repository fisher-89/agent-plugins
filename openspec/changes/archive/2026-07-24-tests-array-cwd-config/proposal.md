# 提案: tests-array-cwd-config

> **变更**: tests-array-cwd-config
> **日期**: 2026-07-22
> **状态**: 草稿

---

## 问题

当前 `openspec/config.json` 的测试配置采用 `test.framework` + `test.overrides` 双层模型，存在以下结构性问题：

1. **`file`/glob 一身兼两职** — 同时承担「匹配范围」与「推导执行目录」（`deriveWorkingDirectory`）。带通配符的 glob 无法保证唯一根目录（集合 ≠ 单点），导致 cwd 隐式且不稳定。
2. **无法声明框架配置文件** — 命令模板无 `--config`；只能 `cd` 进推导出的 directory 后依赖框架自动发现配置。
3. **三场景耦合在同一 `directory`** — 执行 cwd、覆盖率/突变范围、临时文件落点（`coverage/`、`.stryker-tmp/`、`stryker.config.*`、`reports/mutation/`）今日绑在一起；实际需要「执行目录在上级、范围在子树」时无法表达。
4. **全局默认 + override 级联** — 顶层 `coverage` / `mutation` / `exclude` 与 override 合成逻辑增加心智负担与实现复杂度。

本仓库典型需求（`plugins/dev-team/bin` 用 vite-plus，配置文件在包根、源码在 `src/`）无法干净声明「root / cwd / config / includes / excludes」。

---

## 提案

去掉最外层 `test` 对象，改为顶层 **`tests: []`**（复数键名，与旧 `test` 断开，**硬 breaking，不双读**）。每个数组元素 = 一个可执行 suite，采用方案 **G（语义化锚点）**：

```json
{
  "tests": [
    {
      "root": "plugins/dev-team/bin",
      "framework": "vite-plus",
      "cwd": ".",
      "config": "vite.config.ts",
      "includes": ["src/**/*.{ts,tsx}"],
      "excludes": ["./*", "src/schemas/**/*"],
      "coverage": { "lines": 80, "branches": 70, "functions": 75 },
      "mutation": { "score": 70 }
    }
  ]
}
```

### 字段语义

| 字段 | 必填 | 相对谁 | 语义 |
|------|------|--------|------|
| `root` | 是 | projectRoot；**禁止通配符** | suite 唯一锚点 |
| `framework` | 是 | enum | 测试框架 |
| `cwd` | 否 | 相对 **root**；缺省 `"."` | 执行目录；`".."` 表示上级执行 |
| `config` | 否 | 相对 **root** | 框架配置文件路径 |
| `includes` | 否 | glob[]，相对 **root** | 匹配/范围集合；缺省 = 框架 `default_glob` |
| `excludes` | 否 | glob[]，相对 **root** | 从范围中排除 |
| `coverage` | 否 | 对象 | 覆盖率阈值；缺省 schema 常量 |
| `mutation` | 否 | 对象 | 变异阈值；缺省 schema 常量 |

路径约定：

```
absRoot   = projectRoot / root
absCwd    = absRoot / (cwd ?? ".")
absConfig = absRoot / config          # 仅当 config 有值
scope     = under(root) ∩ includes ∩ ¬excludes
artifacts = absCwd 下（第一版；不引入 artifacts 字段）
```

三场景取值：

| 场景 | 取值 |
|------|------|
| (1) 执行目录 | `plan.directory` = absCwd 相对 projectRoot；script `cd` 至此 |
| (2) 覆盖率/突变范围 | suite scope；突变可再 ∩ 测试输出 `sourceFiles` |
| (3) 临时文件 | 默认落在 absCwd |

### 横切决策

1. **阈值与配置默认值在 schema 层解决** — 覆盖率 lines/branches/functions、mutation score 等常量抽到 schema 目录专用文件；**不再**提供全局 `coverage` / `mutation` / `exclude` 顶层块；消费者读 schema parse 结果，不手写级联。
2. **命令接框架 config（模板占位，禁止整段 append）** —
   - `FRAMEWORK_REGISTRY` 增加可选 `config_flag`；第一版仅 `jest` / `vitest` / `vite-plus` → `"--config"`；`pytest` / `rust` / `go` / `bun` / `node-test` → `null`（链式脚本与无文件型 CLI 的框架不适配盲追加）。
   - 各框架 `test_execution` 模板内使用占位符 **`{config_args}`**（需出现在每个会被执行的命令段内；单命令框架一段即可）。生成 script 时：有 `config` 且 `config_flag` 非空 → 展开为 `` `${config_flag} ${path.relative(absCwd, absConfig)}` ``（POSIX）；否则展开为空串。
   - **禁止**对整段 `test_execution` 字符串末尾 append（pytest/rust 等链式命令只会贴到最后一段或产生错误 flag）。
   - suite 声明了 `config` 但框架 `config_flag` 为 `null`/缺省 → **plan/校验失败并给出明确错误**（不静默忽略）。pytest 的 `-c`、rust 的 manifest 语义留待后续；本 change 不把它们列为必达。
3. **硬 breaking** — 旧 `test` 对象一次性失效；本仓库 `openspec/config.json` 同步迁入 `tests[]`。
4. **多 suite 共享同一 absCwd 时的产物碰撞** — 第一版串行执行 + 落 absCwd；不预留 `artifacts` 字段（后续可加方案 C/E）。

最小配置：

```json
{ "tests": [{ "root": "plugins/dev-team/bin", "framework": "vite-plus" }] }
```

上级执行：

```json
{
  "tests": [
    {
      "root": "plugins/dev-team/bin/src",
      "cwd": "..",
      "framework": "vite-plus",
      "config": "vite.config.ts"
    }
  ]
}
```

---

## 能力

### 新增能力

- （无）

### 修改的能力

- **config-schema** — 顶层 `test` 对象替换为 `tests[]`；suite 字段 `root`/`framework`/`cwd`/`config`/`includes`/`excludes`/`coverage`/`mutation`；默认值抽到 schema 目录常量文件；同步 JSON Schema
- **test-detect-frameworks** — 从 `tests[]` 构建 plan；`directory` = absCwd 相对项目根；移除 `deriveWorkingDirectory`；经 `{config_args}` 注入可选框架 config；无 `config_flag` 却声明 `config` 时报错；`mutation_score`/覆盖率阈值取自 suite
- **test-exclude-filter** — 基于 `tests[].excludes`（相对 root 作用域）判断排除；移除全局 `test.exclude` / `overrides[].exclude` 语义
- **test-path-resolver** — 空 modules 自动扫描改为按 suite `root`/`includes`；错误文案改为引导配置 `tests`
- **mutation-testing** — 变异阈值取自 suite；Stryker `rootPath`/临时文件相对 absCwd；mutate 路径相对 absCwd 重写；报告侧不再做全局+override 级联
- **test-get-framework-config** — `FrameworkConfig` / `FRAMEWORK_REGISTRY` 增加可选 `config_flag`；支持 config 的框架模板含 `{config_args}` 占位

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — `test` → `tests[]` suite schema
- `plugins/dev-team/bin/src/schemas/config/` 下新增常量文件（如 `defaults.ts`）— `TEST_COVERAGE_*` / `TEST_MUTATION_SCORE_DEFAULT`
- `plugins/dev-team/bin/dev-team-config.schema.json` — 镜像 Zod 结构
- `plugins/dev-team/bin/src/lib/test-framework.ts` — 增加 `config_flag`；jest/vitest/vite-plus（及后续需注入的框架）`test_execution` 模板嵌入 `{config_args}`
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` — `normalizeFrameworks` / plan / script / mutation 填充改读 `tests[]`
- `plugins/dev-team/bin/src/lib/test-exclude.ts` — suite 级 excludes
- `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` — 扫描根与错误提示
- `plugins/dev-team/bin/src/lib/test-report.ts` — 阈值读取与 overrides 合成改为 suite 维度
- `plugins/dev-team/bin/src/lib/test-runner.ts` — exclude / mutation 范围适配
- `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` — 路径相对 absCwd（若需）
- `plugins/dev-team/bin/src/commands/test-execution.ts` / `mcp.ts` — 配置缺失提示文案
- `openspec/config.json` — 迁入 `tests[]`（G 字段）
- 相关 agent / prompt 中 `test.framework` · `test.overrides` 表述
- `<plugin>/.claude-plugin/plugin.json` — 按项目规则升级插件版本

### 测试文件

- `plugins/dev-team/bin/src/schemas/config/` 相关 schema 测试（若有）及依赖 `OpenSpecConfig['test']` 的测试夹具
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`
- `plugins/dev-team/bin/src/lib/test-exclude.test.ts`
- `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts`
- `plugins/dev-team/bin/src/lib/test-runner.test.ts`
- `plugins/dev-team/bin/src/lib/test-report` 相关测试
- `plugins/dev-team/bin/__tests__/**` 中构造旧 `test.overrides` 的集成夹具（config-driven-auto-scan、dedup、mutation-diff-only、no-test-config 等）

### 不要修改

- LikeC4 / architecture 相关能力与脚本
- write-protection 语义（除文案中若提及旧 `test` 键外）
- 第一版不新增 `artifacts` / 全局 tmp 字段（方案 C/E 留待后续）
- 不保留对旧 `test` 对象的双读兼容层

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | schema：`tests[]` suite | 合法 suite（必填 `root`+`framework`）通过校验；`root` 含通配符被拒绝；缺省 `cwd` 为 `"."`；`coverage`/`mutation` 缺省数值来自 schema 常量；旧 `test` 键不再作为正式字段被解析为 suite 配置 |
| AC-2 | plan.directory = absCwd | `root: "a/b"`, `cwd: ".."` → `directory` 为 `"a"`（相对项目根）；script 的 `cd` 指向该目录 |
| AC-3 | 可选 config 注入（占位符） | suite 含 `config` 且框架有 `config_flag` 时，`{config_args}` 展开为 flag + 相对 absCwd 的路径并出现在模板占位处（非整段末尾 append）；无 `config` 时 `{config_args}` 为空、行为与今日一致；suite 含 `config` 但框架 `config_flag` 为 null 时 plan/校验失败 |
| AC-4 | scope = root∩includes∩¬excludes | 缺省 `includes` 时使用框架 `default_glob`（相对 root）；`excludes` 从范围中剔除；检测/路径推导/突变均遵守同一 scope |
| AC-5 | 阈值无全局级联 | 各 suite 自带 `coverage`/`mutation`（或 schema 默认）；报告不再依赖顶层全局块 + override 级联 |
| AC-6 | 本仓库配置迁移 | `openspec/config.json` 使用 `tests[]` 表达现有 vite-plus suite，且 `dev-team test-execution` / `test_detect_frameworks` 可正常出 plan |
| AC-7 | 回归 | 更新夹具后，bin 包内既有单测与关键tests__ 集成场景通过 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 硬 breaking 破坏外部仓库旧 `test` 配置 | 外部项目升级插件后测试配置失效 | 高（对仍用旧键的仓库） | changelog / 迁移示例；键名 `tests` 刻意断开；不双读以免隐式半迁移 |
| 多 suite 共享 absCwd 时 coverage/stryker 产物互相覆盖 | 后执行 suite 覆盖前者产物 | 中 | 第一版文档约定串行；碰撞后再做 artifacts/统一 tmp |
| `includes` 缺省用 `default_glob` 与「源文件范围」预期不符 | 源文件扫描过窄/过宽 | 中 | 明确文档：缺省匹配测试文件 glob；需要源码树时显式写 `includes` |
| `config_flag` 各框架 CLI 不一致；链式脚本盲 append 失效 | 错误 flag / 只作用于最后一段命令 | 中 | 仅 jest/vitest/vite-plus 设 flag；模板 `{config_args}` 按段占位；无 flag 框架声明 `config` 时显式失败；单测覆盖注入与拒绝路径 |
| exclude 相对 root 与旧「相对 projectRoot 的 file」习惯不同 | 迁移时 glob 写错 | 中 | 迁移指南给对照示例；本仓库 `openspec/config.json` 作样板 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 外层结构 | 顶层 `tests: []`，移除 `test` | 与旧模型断开，breaking 干净；一元素一 suite | 保留 `test.overrides` 仅加 cwd/config — 无法解耦三场景 |
| 条目形状 | 方案 G：`root`+`includes`/`excludes`+`cwd`/`config` | 锚点禁通配符；相对性统一以 root 为原点；上级执行用 `cwd: ".."` | 方案 B `file`+`cwd` — `file` 仍兼集合与锚点 |
| `includes` 缺省 | 框架 `default_glob`（相对 root） | 与今日「framework → default_glob」短写对齐；避免「省略即全文件」过宽 | (a) root 下全文件；(c) 强制必填 |
| glob 相对性 | `includes`/`excludes`/`cwd`/`config` 均相对 **root** | 相对性一致，心算成本低 | 相对 projectRoot（旧 `file` 习惯） |
| 默认值位置 | schema 常量 + `prefault`；无全局 coverage/mutation/exclude | 消灭级联；parse 后即最终值 | 保留全局默认块 |
| 旧 `test` 兼容 | 硬 breaking，不双读 | 避免两套模型长期共存 | 双读 + 警告迁移期 |
| 框架 config 注入 | registry `config_flag` + 模板 `{config_args}` 占位展开；禁止整段 append；无 flag 却写 `config` → 显式失败 | 链式脚本（pytest/rust）末尾 append 无法正确作用到每一段；静默忽略会误导用户 | 整段 append（已否决）；pytest `-c` / rust manifest 纳入本 change（推迟） |
| 临时文件 | 第一版固定 absCwd | 满足主路径；控制范围 | 同步加 `artifacts` 字段 |

### 待决问题

- 无（探索中开放项已在上表拍板；`artifacts` 明确不做第一版；pytest `-c` / rust 专用配置字段明确不纳入本 change）
