# Explore: test-execution 结果文件化采集

> 日期: 2026-07-31  
> 状态: 探索中（尚无 change）  
> 拟议 change 名: `test-execution-result-file`（暂定）  
> 修订: 2026-07-31 — 对齐布局/管线/`prepare` 职责与未决项（审查修订）

---

## 问题

`dev-team test-execution` 通过子进程 **stdout** 收集测试结果。实际使用中，子进程打印或运行报错会导致 stdout 非标准格式，解析失败。

当前痛点集中在 **jest**：`js-parser` 对整段 stdout 做 `JSON.parse`，前后缀噪声即导致 `Failed to parse JSON output`。

对照：覆盖率、mutation 已走文件通道；仅测试用例结果仍绑 stdout。

---

## 已定方向

1. **采集通道**：结果进文件再解析。优先级：
   - **框架原生「输出到文件」**（如 jest `--json --outputFile=`、vitest reporter outputFile、llvm-cov `--output-path`、pytest `--cov-report=json:path`）——优先，避免脏 stdout；
   - **无原生能力时**再壳层 **段级 `>`** 重定向；
   - **不**做 tee；**不**做脏文本括号扫描抽 JSON（见 B1）。
2. **范围**：**本轮全框架**；产物统一到 `reports/test/<planId>/`；summary 含 **`plans[]` 索引**。  
   （主痛点是 jest；路径搬家与全框架垂直 parser 同轮交付，风险见「范围与回滚」。）
3. **链式框架**：测试段与覆盖率段**分段落同一 reportDir**（各用原生文件输出或段级 `>`）；权威读取范围始终是该 plan 目录。
4. **Parser 形态**：**多框架垂直设计**（含垂直定义产物文件名/后缀）；runner 变瘦；execute 前统一走 `preparePlanArtifacts`（见下：算路径/占位符；临时 config 仅 C3 例外）。
5. **覆盖率**：有 coverage 文件时由垂直 parser 从 **plan 目录**读；无则 null；**不**从 jest 结果内 coverageMap fallback；**不**读 suite cwd 旧路径。

### Jest 落地形态（按最新结论）

```
npx jest --json --outputFile=<planDir>/results.json \
  --coverage --coverageDirectory=<planDir> --coverageReporters=json-summary \
  --silent ...
  → 垂直 parser 直接读 results.json + coverage-summary.json
  → 无需 JSON.parse(stdout)、无需脏扫描、无需对该命令追加 `>`
```
（具体文件名由 jest 垂直定义，见 A1-c。）

---

## 统一 plan 产物目录（2026-07-31 续）

### 已定

1. **无 `--change`**：对称缩到仓库根下 `reports/test/<planId>/`（不在 `openspec/changes/` 内）。
2. **目录改名 + summary 进目录**：`reports/test-execution/` → `reports/test/`；summary 不再做兄弟文件 `test-execution.json`，改为 **`reports/test/summary.json`**（有 change 时：`openspec/changes/<change>/reports/test/summary.json`）。
3. **plan 目录命名**：复用现有 `derivePlanId` **算法**，但返回值改为**目录 id**（去掉今日实现里的 `.json` 后缀）；见下方。
4. **bun**：主路径 = 临时 bunfig 直写 `lcov.info` 到 planDir；**不做** post-copy 作为默认策略（仅实现期应急备选，不进正式方案）。
5. **存量配置**：不兼容迁移、不读旧路径。后续框架直写 plan 目录，parser/runner **只从 plan 目录读**。

### 目标布局（跨框架强制名 vs 垂直名）

**跨框架强制统一**（仅此两项）：

| 文件 | 含义 |
|------|------|
| `summary.json` | 聚合报告（在 `reports/test/`，不在 plan 子目录） |
| `<planId>/report.json` | 原子报告（原 sub_report） |

**测试结果 / 覆盖率 / mutation 文件名由各框架（或工具）垂直定义**（A1-c）。mutation 当前只有 Stryker 一种格式，本轮约定落盘为 `mutation.json`，**不是**「多工具强制统一名」——日后若有第二种 mutation 工具，按其垂直模块自定文件名即可。

下图中的 `results.*` / coverage / `mutation.json` 名为示意（mutation 为现行唯一实现的约定名）：

```
# 有 --change
openspec/changes/<change>/reports/test/
  summary.json
  <planId>/
    results.json | results.txt | results.ndjson   # 垂直定义（原生 output 或段级 >）
    coverage-summary.json | lcov.info | …         # 垂直定义（有则写）
    mutation.json                                 # 现行仅 stryker；非跨工具强制名
    report.json                                   # 原子报告（统一）

# 无 --change
reports/test/
  summary.json
  <planId>/...
```

CLI / phase 名仍叫 `test-execution`；**仅报告路径**缩短为 `test`。agent / specs 文案写路径时用 `reports/test/`，勿与 phase 名混用。

### plan 目录命名 — 已定

**复用现有消毒算法，产出目录名（不含 `.json`）+ summary 内嵌索引。**

今日代码 `derivePlanId` 返回的是**文件名**（如 `vitest.json` / `plugins_dev-team_bin_vite-plus.json`）。本 change 起：

```
planId = sanitize(directory) + framework   # 无 .json 后缀
# directory === '.' → 空前缀 → planId = "<framework>"     （A5-a；与现逻辑去掉 .json 后一致）
# 例: '.' + vitest                    → "vitest"
# 例: plugins/dev-team/bin + vite-plus → "plugins_dev-team_bin_vite-plus"

路径 = reports/test/<planId>/
原子报告 = reports/test/<planId>/report.json
```

> 注：源码 JSDoc 曾写 `'_vitest.json'`，与实现/单测不符（实际是 `vitest.json`）。**以 A5-a / 现实现为准**，不做 `_vitest` 变体。

| 获取方式 | 谁用 | 怎么拿 |
|----------|------|--------|
| **公式计算** | CLI runner、已持有 plan 条目的步骤 | `join(reportsRoot, 'test', planId(directory, framework))` — 不依赖先跑完、不依赖 list dir |
| **summary 索引** | executor / evaluator / 只读报告的下游 | 读 `summary.json` 的 `plans[]`：`{ id, framework, directory, path }` → 打开 `report.json` |
| **扫目录** | 调试/人工 | `reports/test/*/` 即各 plan（忽略 `summary.json`） |

为何不推荐「真路径嵌套」`test/plugins/dev-team/bin/vite-plus/`：
- `directory === '.'` 与多层相对路径在 agent 拼路径时易错
- Windows 路径更长；与现有 `derivePlanId` 测试/心智不一致

### 第 3 点续：有无 `summary.plans[]` 索引对比

先澄清：`summary.json` **聚合报告本身始终需要**（conclusion / problems / 总覆盖率——executor、evaluator 已依赖）。此处对比的是：**要不要在 summary 里再嵌 `plans[]` 路径索引**。

| | **A. 有 `plans[]` 索引** | **B. 无索引（仅公式 / 扫目录）** |
|--|--------------------------|----------------------------------|
| **下游怎么找 plan 目录** | 读 summary → `plans[].path` | 自备 `derivePlanId` 规则，或 `list reports/test/*/` |
| **多 suite 同 framework** | 显式列出，不易漏 | 公式 OK；扫目录也 OK；agent 若误用「只有 framework 名」仍会错 |
| **与消毒规则耦合** | 弱——规则变了只要 CLI 重写索引 | 强——一切消费者必须与 CLI 同算法、同边界（`.` → 前缀等） |
| **半失败 / 部分 plan 未跑** | 索引含实际尝试项（仍给 path）；成败看 `report.json` / summary `problems` | 扫目录看到的是「有文件夹的」；公式会指向尚未创建的路径 |
| **schema / 体积** | summary 多一段数组；要定 path 相对根 | summary 更瘦，维持现状字段即可 |
| **单测 / 契约** | 多测「索引与落盘一致」 | 少测索引；改名算法时要全仓搜消费者 |
| **实现成本** | 低（写 summary 时顺手塞） | 表面更低，长期把路径知识复制到 agent 文档与多处代码 |

**依赖步骤视角**

```
                    ┌─ executor：要读各 plan/report.json 做失败诊断
CLI 跑完 ──summary─┤
                    └─ evaluator：通常只读聚合 conclusion（可不碰 plans[]）

另：CLI 内部 execute 时自有 plan 条目 → 公式足够，不依赖索引
```

- **evaluator**：索引可有可无（只看聚合）。
- **executor / 人工 / 外部工具**：索引价值高——今日 agent 已误写成 `<framework>.json`，说明「约定公式靠文档传播」不可靠。
- **CLI 自身**：不需要索引。

**已定（2026-07-31）**：**要 `plans[]` 索引**。summary 聚合字段保留；新增 `plans[]` 供 executor / 下游在无 plan 对象时定位各 plan 目录。

**半失败约定（补充已定）**：每个尝试执行的 plan 都进 `plans[]` 并给出 `path`（目录可能已 mkdir）。`plans[]` **不**携带 status——它只做路径索引；成败与原因落在该 plan 的 `report.json` 以及 summary 既有的 `conclusion` / `problems`。不靠「有没有子目录」推断「跑没跑过」。

---

### bun 问题详解

插件现状（`test-framework.ts`）：

```
bun test --coverage --coverageReporters=json-summary {files}
coverage_format: istanbul
coverage_output: coverage/coverage-summary.json
```

与 Bun 官方能力错位：

| 维度 | 插件假设（偏 Jest） | Bun 实际（文档） |
|------|---------------------|------------------|
| 覆盖率 reporter | `json-summary` → istanbul `coverage-summary.json` | 仅 **`text`**（打控制台）与 **`lcov`**（写 `coverageDir/lcov.info`） |
| 输出目录 CLI | 假定类似 jest 的 coverageDirectory / Reporters 旗标 | 持久化目录主要通过 **`bunfig.toml` 的 `test.coverageDir`**；`coverageDir` 明确作用于 lcov 这类 persistent reporter |
| 旗标形态 | `--coverageReporters=json-summary`（Jest 风格） | 文档为 `--coverage-reporter=lcov` / bunfig `coverageReporter` |

因此「写入 plan 目录」对 bun 的摩擦不只是路径，而是 **格式契约就不成立**：

1. **没有原生 istanbul json-summary** — 即使指定目录，也不会出现插件 istanbul parser 期望的 `coverage-summary.json`。
2. **text 覆盖率在 stdout** — 测试段本就用 `>`；覆盖率若只走 text，则无侧车文件，`coverage=null` 或从同一 raw 解析（本轮倾向文件型 lcov）。
3. **已定主路径**：临时 bunfig，设 `coverageReporter = ["lcov"]`、`coverageDir = <planDir>`，直写 plan 目录；parser 读 `lcov.info`；`coverage_format` → **lcov**。post-copy **不**作为正式策略。
4. summary 层对 lcov：垂直 parser 先归一成现有 coverage 结构（与 istanbul/llvm 一样交给横向库），再参与聚合；细节 design 定字段映射。

**结论**：bun 不是「不能重定向测试输出」，而是「当前 registry 按 Jest/istanbul 建模，和 Bun 真实产物不一致」。临时 bunfig 把「目录 + reporter」收进配置层；需处理与项目已有 `bunfig.toml` 的合并/覆盖策略（读入 overlay，**不改用户文件**）。`bun --config=` 参数形态见 §8，实现前钉死。

### execute 前准备：`preparePlanArtifacts`（原 prepareFrameworkConfig）

> 命名修订：钩子主责是 **决议 reportDir / 占位符 /（可选）临时 config**，不是「每框架都写临时测试 config」。旧称 `prepareFrameworkConfig` 易误导，下文统一用 `preparePlanArtifacts`。

现状两条时间线：

```
detect 时（现在）:
  resolveConfigArgs(suite.config) → expandConfigArgs(template)
  → script 字符串已烤死 --config <user>

execute 时（Stryker 已有先例）:
  resolveStrykerConfig() 写临时 stryker.config.<rand>.json
  → 再拼 mutation 命令
```

```
executePlanEntry(plan):
  reportDir = reports/test/<planId>/
  mkdir reportDir
  清空 reportDir 内既有内容（A3-a：只清该 plan 目录，不影响其他 plan）

  prepared = preparePlanArtifacts({
    framework,
    absCwd,
    reportDir,
    userConfigPath?,
  })
  // 始终返回：placeholders（结果/覆盖率路径等）+ configArgs（可能仍是用户 --config）
  // 仅 C3 例外框架额外：tempPaths（临时 bunfig 等）

  cmd = expand + substitute(prepared) 
      + (需要段级重定向时) ` > "{results_file}"`   // 条件追加，见下
  run(cmd)
  verticalParse(reportDir)
  cleanup(prepared.tempPaths)
```

| 框架 | 临时文件 | 如何挂上命令 | 与用户 config |
|------|----------|--------------|----------------|
| **bun** | `bunfig.dev-team-<rand>.toml`：`coverageDir`、`coverageReporter=["lcov"]` | `bun --config="<temp>" …`（§8 钉死位置）；cwd 写、用后删（C4） | 读已有 bunfig 再 overlay；不改用户文件 |
| **jest / vitest / vite-plus** | **默认无**（C3） | CLI 旗标指 `results_file` / coverage 目录；`{config_args}` 传用户 config | 输出相关路径以 CLI 为准；若实证 CLI 无法覆盖 config 内同名键 → **降级**该框架临时 config（例外，非默认） |
| **stryker** | 已有临时 JSON | `jsonReporter.fileName → reportDir/mutation.json` | cwd 临时、用后删 |
| **go / rust / pytest / node-test** | 无 | 占位符 + 原生 path 旗标和/或段级 `>` | N/A |

**关键设计约束**

1. **时机在 execute，不在 detect** — `reportDir` 依赖 `--change` / 最终 reports 根；detect 烤死路径会错。`{config_args}` / `{results_file}` / … 留给 execute 展开（今日 detect 已烤死 config，要改）。
2. **顺序**：`preparePlanArtifacts` → 得到 `configArgs` + placeholders → `expandConfigArgs` / 替占位符 → **按需**追加 `>`。
3. **cleanup**：与 Stryker 一样 best-effort 删临时文件；失败不阻断报告。
4. **bun 特有风险**：固定名 `bunfig.toml` 发现机制 vs 显式 `--config`；实现前钉死加载方式。不短时改写用户 `bunfig.toml`。
5. **C3**：能 CLI 解决的不做临时测试 config；临时 config 仅 bun（及 CLI 实证无效的例外）。

**已定（2026-07-31）**：
- 引入 execute 期 `preparePlanArtifacts`：解析 plan 产物路径、填占位符、按需生成临时配置，再拼命令。
- **C3** / **C4** 见决议录。
- **本轮 change 修改所有框架**；细节见草案。

### 存量路径（硬约束，已定）

**无需、也不做**对旧 coverage / mutation 路径的兼容：

- **不读** suite cwd 下 `coverage/`、`coverage.json`、`coverage.out`、`reports/mutation/mutation.json` 等旧位置。
- **不双读**、不 fallback、不做「plan 缺失则回退旧路径」。
- **不迁移**历史产物；本 change 起权威来源唯一为 `reports/test/<planId>/`（及同树下 `summary.json`）。
- 若框架因用户配置仍向旧路径多写一份：**忽略**，解析与报告只认 plan 目录。
- 实现上删除 suite `coverage_cleanup` 及一切「解析旧路径」的测试/文档引用（A3-a）。

### 无 tee 时的可观测性与失败诊断（补充已定）

- **不做 tee**（A4-a）：原生 outputFile 族依赖框架自己的静默/进度；`>` 族父进程看不到实时 stdout。
- 执行结束后诊断一律读 plan 目录产物 + `report.json`，不依赖捕获的 stdout。
- 若命令非 0 且约定结果文件缺失/空/无法解析：该 plan `execution_error`（或等价），写入 `report.json`；summary 聚合照旧计入 failed/problems。`plans[]` 仍只提供 path。
- 验收：对 `>` 族保留「噪声/前缀」类 AC；对原生 outputFile 族以「读 plan 文件成功」为主 AC。

### 范围与回滚（风险说明，非缩小范围）

- 本轮仍 **全框架** + 路径搬家（已推翻 Jest-first）。
- 实现顺序建议先落地路径层 + jest/vitest 文件通道（主痛点），再并行 C 类与 bun；若 bun `--config` 实证失败，bun 可暂时 `coverage=null` + 测试结果 `>`，不阻断其它框架合并——design/tasks 里拆可独立验收的切片。

---

## Config / 产物实现细节草案（供后续 design 引用）

> 状态：explore 级思考，非正式 schema。本轮 change 范围 = **全部框架**。

### 0. 产物约定（plan 目录内）— **A1-c 已定**

**无全局统一「测试结果 / 覆盖率 / mutation」文件名**。各框架（或工具）垂直模块定义自己的文件名与后缀；runner / summary 只保证目录是 `reports/test/<planId>/`，并写统一的 `report.json`。

```
reports/test/<planId>/
  <vertical-defined artifacts...>   # 测试结果、覆盖率、mutation、中间态
  report.json                       # 原子报告（全框架统一）
```

mutation：现行仅 Stryker → 本轮 overlay 到 `reportDir/mutation.json`；属垂直约定，不是跨工具强制契约。

**垂直命名草案（design 可微调，原则已定）**：

| 框架 | 测试结果文件 | 覆盖率文件 | 采集方式 |
|------|--------------|------------|----------|
| jest | `results.json` | `coverage-summary.json` | 原生 `--outputFile` + `--coverageDirectory`；**不** `>` |
| vitest / vite-plus | `results.json` | `coverage-summary.json` | 原生 reporter/outputFile + reportsDirectory（CLI 优先）；**不** `>`（除非 §8 证实无 CLI） |
| bun | `results.txt`（`>`） | `lcov.info` | 测试无原生 JSON 文件 → `>`；覆盖率临时 bunfig → coverageDir |
| go | `results.ndjson`（`>`） | `func-summary.txt` + `coverage.out` | `-json` 无官方文件旗标 → `>`；coverprofile 原生路径；再 `go tool cover`；垂直 parser 归一成单一 coverage 结构 |
| rust | `results.txt`（`>` cargo test） | `coverage-summary.json` | llvm-cov `--output-path` 原生 |
| pytest | `results.txt`（`>` 测试段） | `coverage.json` | `--cov-report=json:path` 原生 |
| node-test | `results.txt`（`>`） | （可选无侧车，含在 results 文本表） | 无原生结果文件 → `>` |

### 1. `summary.json` + `plans[]`

```json
{
  "phase": "test-execution",
  "command": "dev-team test-execution",
  "plans": [
    {
      "id": "plugins_dev-team_bin_vite-plus",
      "framework": "vite-plus",
      "directory": "plugins/dev-team/bin",
      "path": "reports/test/plugins_dev-team_bin_vite-plus"
    },
    {
      "id": "vitest",
      "framework": "vitest",
      "directory": ".",
      "path": "openspec/changes/my-feature/reports/test/vitest"
    }
  ],
  "conclusion": "pass",
  "total": 0,
  "passed": 0,
  "failed": 0,
  "skipped": 0,
  "coverage": null,
  "mutation": null,
  "problems": [],
  "findings": []
}
```

- **path 相对谁（已定 A2-a）**：相对 **project root**。  
  - 无 change：`reports/test/<planId>`  
  - 有 change：`openspec/changes/<change>/reports/test/<planId>`  
  （上例第二条示意有 change 形态；同一 summary 内不会混两种根，仅作文档对照。）
- `id` === 目录名 === 新 `planId(directory, framework)`；**根 suite（A5-a）**：`directory === '.'` → id 为 `<framework>`（无前缀）。

### 2. 核心钩子：`preparePlanArtifacts`

```ts
// 探索级伪代码
interface PrepareInput {
  framework: TestFramework;
  absCwd: string;                 // suite 执行 cwd
  reportDir: string;              // 绝对路径 …/reports/test/<planId>
  userConfigPath: string | null;  // suite.config 绝对路径（若有）
  projectRoot: string;
}

interface PrepareResult {
  /** 替换模板 {config_args}；无则 ''；A 类通常仍是用户 --config */
  configArgs: string;
  /** 是否对「测试结果段」追加壳层重定向（原生 outputFile 则为 false） */
  redirectStdoutToResults: boolean;
  /** 额外占位符，供 test_execution 模板使用（命名统一如下） */
  placeholders: {
    report_dir: string;         // 建议相对 absCwd；内部可保留 abs
    results_file: string;       // 垂直定义的测试结果路径（含文件名）
    coverage_file: string;      // 含文件名；无覆盖率侧车时可空
    coverprofile_file?: string; // go
    mutation_file?: string;
  };
  tempPaths: string[];          // execute 结束 best-effort 删除；A 类默认 []
  env?: Record<string, string>;
}
```

**调用顺序（execute，所有框架）**

```
1. planId / reportDir 决议；mkdir；清空该 reportDir（A3-a）
2. preparePlanArtifacts(...)         ← 路径/占位符；按需临时 config
3. expandConfigArgs(template, configArgs)
4. substitutePlaceholders(..., prepared.placeholders + files/directory/…)
5. 若 prepared.redirectStdoutToResults：追加 `> "{results_file}"`
   （原生 outputFile 族：false，不追加）
6. runCommand
7. verticalParse(framework, reportDir)  ← 按垂直约定文件名读；不是「从 raw 抽 JSON」
8. 写 report.json；收集进 summary.plans[]（仅索引字段）
9. cleanup tempPaths；mutation 临时文件同理
```

**detect 阶段变更（已定 C1-b）**：detect 仍生成 **带占位符的 script**；execute 只替换 `{config_args}` / `{results_file}` / `{coverage_file}` / `{report_dir}` / … 与 prepare 产物，**不**在 detect 烤死 reportDir 相关路径。

### 3. 分框架策略（按 C3：CLI 优先，临时 config 兜底）

**C3 原则（已定）**：**凡 CLI 能表达的输出路径 / reporter，先走 CLI，不做临时 config。**  
临时 config / bunfig 仅当框架无法用旗标把产物指到 plan 目录时（典型：bun 的 `coverageDir`）。

**C3 降级（已定）**：若 design 前/中实证「CLI 不能覆盖用户 config 内输出路径」，该框架升临时 config overlay（仍 cwd + 用后删），不阻塞其它框架。§8 清单用于关闭这条分支。

| 类 | 框架 | 策略 |
|----|------|------|
| **A. CLI 指路径 + 用户 `--config` 原样传递** | jest, vitest, vite-plus | `suite.config` → `{config_args}`；另加 CLI：`--outputFile` / coverage 目录旗标 → planDir；**默认不**为输出路径生成临时 jest/vitest config；`redirectStdoutToResults=false` |
| **B. 必须临时 bunfig** | bun | CLI 不足以稳定设置 coverageDir → cwd 写临时 bunfig → `--config=` 加载 → **用后删除（C4)**；测试段 `redirectStdoutToResults=true` |
| **C. 纯 CLI / 重定向占位符** | go, rust, pytest, node-test | 无临时 config；路径占位符 + 段级 `>` / 原生 output-path |

#### A — jest（CLI）

```
npx jest --json --outputFile="{results_file}" --silent --coverage \
  --coverageDirectory="{report_dir}" --coverageReporters="json-summary" \
  {config_args} {files}
```

- 不 `>` stdout（结果已在 `results_file`）
- 用户 config 仍经 `{config_args}`；输出路径以 CLI 为准（§8 实证）；若否 → 该框架降级临时 config
- mutation：stryker 临时文件已有；`jsonReporter.fileName` → `reportDir/mutation.json`；cwd 临时、用后删

#### A — vitest / vite-plus（CLI）

- 用 CLI / 等价旗标指定 json reporter 的 **outputFile** 与 `coverage.reportsDirectory`（具体旗标名 §8 / design 核对）
- `{config_args}` 传用户 config；**默认不为** merge 输出路径写临时 vite config（C3）
- 若某版本 CLI 无法指定 outputFile → 该版本降级 `>`（`redirectStdoutToResults=true`）或临时 config（例外）

#### B — bun（临时 bunfig，C4）

- 纠正 registry：去掉伪 `--coverageReporters=json-summary`
- 在 **absCwd** 写 `bunfig.dev-team-<rand>.toml`，overlay `coverageDir` / `coverageReporter=["lcov"]`；若有用户 bunfig 则读入再 overlay
- `bun --config="<temp>" test --coverage {files} > "{results_file}"`
- **用后删除**临时 bunfig（C4）；失败路径也要 try/finally cleanup
- `coverage_format` → **lcov**；`lcov.info` 在 planDir

#### C — rust / pytest / go / node-test

- **C2**：  
  - 有原生文件输出的段：用旗标写到 planDir（llvm-cov `--output-path`、pytest `--cov-report=json:path`、go `-coverprofile=`）。  
  - 无原生结果文件的段：段级 `>` 到垂直定义的 `results.*`。  
  - 不把整条链式命令的所有 stdout 糊成一个文件，除非该框架只有一段输出。  
- go：补 `go tool cover -func=… > func-summary.txt`（均在 planDir）；parser 把 `func-summary.txt`（+ 需要时 `coverage.out`）归一进现有 coverage 字段  
- node-test：仅 `>` → `results.txt`；parser 从同一文件抽 tests + coverage 表  
- Windows cmd：与现有 rust `errorlevel` 模式对齐段级重定向

### 4. 临时文件原则（C4 已定）

1. **不修改**用户仓库内的长期 config / bunfig。
2. 临时文件落在 **suite cwd（absCwd）**，不用完就扔到系统 tmp（与现 stryker 一致、相对路径短）。
3. **用后删除**（正常结束与异常路径均 best-effort）。
4. 命名前缀可识别：`.dev-team-*` / `bunfig.dev-team-*`，避免撞测试 glob。
5. prepare 失败 → 该 plan `execution_error`，仍写 `report.json` 并进入 `plans[]`（path），不继续跑该 plan。
6. **A 类默认不创建临时测试 config**（C3）；仅 bun（及经确认 CLI 无效的例外）创建。

### 5. Registry / plan schema 可能变化

| 字段 | 变化方向 |
|------|----------|
| `coverage_output` | 改为相对 **reportDir** 的约定文件名，或由 `coverage_format` 推导，减少每框架硬编码 suite-相对路径 |
| `coverage_cleanup` | 改为「清空/重建 reportDir」+ 删 tempPaths；删除 suite cwd 内 cleanup |
| `script.shell/cmd` | 含 `{results_file}` `{coverage_file}` `{report_dir}` `{config_args}`；detect 不展开 report 相关 |
| `config_flag` | bun 从 null → 支持显式 config；或 bun 走 prepare 特化不共用简单拼接 |
| plan 新增？ | 可选 `report_dir` / `plan_id` 在 execute 填，detect 可不放 |

### 6. 垂直 parser 分发（全框架）

```
parsePlanArtifacts(framework, reportDir) -> { testCases, coverage, ... }
  jest/vitest/vite-plus → 读 results.json + coverage-summary.json（直接 parse 文件，非 stdout）
  bun → 读 results.txt + lcov.info
  go → 读 results.ndjson + func-summary.txt（归一 coverage）
  rust → 读 results.txt + llvm json
  pytest → 读 results.txt + coverage.json
  node-test → 仅 results.txt（tests + coverage 表）
```

横向 `coverage-parser`：**保留为库函数**，由垂直模块调用（istanbul/llvm/lcov/…），runner 不再直接调。

### 7. B2-a：`ExecutionResult` 扩展草案

在现有字段（`framework`, `exitCode`, `testCases`, `coverage`, `mutation?`, `durationMs`, …）上增量，而非新类型替换：

| 字段 | 说明 |
|------|------|
| `planId` | 目录 id |
| `reportDir` | 绝对或相对 project root 的 plan 目录 |
| `resultsFile?` | 实际读取的测试结果文件（垂直名） |
| `error?` | 保留；prepare / 解析失败原因 |

summary 的 `plans[]` 只从 `ExecutionResult` 投影索引字段（id / framework / directory / path），不投影成败状态。

### 8. 实现顺序建议（tasks 级，非正式）

1. 路径层：`reports/test/`、`planId`（无 `.json`）、`summary.json` + `plans[]`、agent/spec 路径迁移  
2. execute 管线：`preparePlanArtifacts` 骨架 + detect 推迟展开占位符；**条件 `>`**  
3. JS 族：jest/vitest/vite-plus 原生文件输出 + 垂直 parser（主痛点优先）  
4. C 类：go / rust / pytest / node-test 占位符 + 段级 `>`  
5. stryker `mutation.json` 指入 planDir  
6. bun：临时 bunfig + lcov parser（`--config` 未钉死前可先 coverage=null）  
7. 清理 suite cwd cleanup / 旧路径测试

### 9. 已知风险 / 实现期核对（非 explore 拍板项，但阻塞 C3 默认路径）

- [ ] jest：CLI `--coverageDirectory` / `--outputFile` 是否稳定覆盖用户 config 内同名键 → 否则该框架降级临时 config
- [ ] vitest/vite-plus：json reporter **outputFile** 与 coverage **reportsDirectory** 的确切 CLI 形态
- [ ] bun：`bun --config=f test` 参数位置；cwd 临时 bunfig 用后删；加载失败时的降级（测试 `>` + coverage=null）
- [ ] Windows `cmd` 段级重定向与 `errorlevel`
- [ ] 各垂直模块文件名终表写入 design（上表为草案）
- [ ] go：`func-summary.txt` → 现有 coverage 聚合字段的映射

---

## 未决项决议录（2026-07-31）

### 采集与路径

| ID | 决议 |
|----|------|
| A1-c | 各框架**垂直定义**测试结果/覆盖率/mutation 文件名与后缀；仅 `report.json` / `summary.json` 跨框架强制统一。mutation 现行仅 stryker → 约定 `mutation.json`，非多工具强制名 |
| A2-a | `plans[].path` 相对 **project root**（有/无 change 两种前缀，见 §1 示例） |
| A3-a | 跑前清空**该** `reportDir` 再写；**删除** suite 内 `coverage_cleanup`（产物不再落 cwd） |
| A4-a | 不做 tee；需要时读 plan 内结果文件；失败诊断见「无 tee 时的可观测性」 |
| A5-a | `directory === '.'` → planId = `<framework>`（无前缀、无前导 `_`、无 `.json`） |

### Parser / 管线

| ID | 决议 |
|----|------|
| B1 | **不**做脏 stdout/raw 括号扫描；**优先框架自带输出到文件**；无则段级 `>` |
| B2-a | 扩展现有 `ExecutionResult`（字段草案见 §7） |
| B3-a | 无 coverage 侧车则 coverage=null；**不** fallback coverageMap |
| B4-a | 横向 coverage-parser 保留为库函数，垂直模块调用 |

### Config / 管线 API

| ID | 决议 |
|----|------|
| C1-b | detect 产带占位符 script；execute 替换占位符 + prepare 产物 |
| C2 | 有原生文件输出用旗标；无则段级 `>`；同一 reportDir；**条件**追加 `>`（`redirectStdoutToResults`） |
| C3 | **能 CLI 解决的先不做临时测试 config**；默认仅 bun；CLI 实证无效则该框架降级临时 config |
| C4 | 临时文件在 **cwd**，**用后删除** |

### 验收 D（同意）

- 噪声场景：对「仍可能脏」的路径（`>` 族）保留 AC；jest 等原生 outputFile 以「读 plan 文件成功」为主 AC
- agent + specs 路径迁移 MUST（`test-execution` → `reports/test/` + `summary.json`；子报告为 `<planId>/report.json`，禁止再写 `<framework>.json`）
- 八框架各至少 1 条「产物在 reportDir」单测（可 mock fs）
- Win cmd + Git Bash：链式框架路径敏感单测；真机抽查可上手动 tasks
- 半失败：失败 plan 仍出现在 `plans[]`（path 可定位），细节在对应 `report.json` / summary `problems`

---

## 明确不在范围内 / 已推翻的旧选项

- ~~统一只用 `>`、排除原生 `--outputFile`~~ → **已推翻**；改为原生文件输出优先
- ~~脏 JSON 括号扫描~~ → **不做**（B1）
- ~~为 jest/vitest 默认写临时 config 指覆盖率目录~~ → **不做**（C3）；CLI 优先，实证失败再降级
- ~~目标布局统一 `raw.json` / `coverage.json`~~ → **已推翻**；与 A1-c 对齐为垂直命名
- ~~planId `_vitest` 变体~~ → **不做**；A5-a = `<framework>`
- ~~bun post-copy 作为正式策略~~ → **不做**；临时 bunfig + lcov
- 默认 tee（A4-a 不做）
- 兼容 / 双读 / fallback 到 suite cwd 旧 coverage/mutation 路径（硬约束：不做）
- Jest-first 分批（本轮全框架；实现切片可先主痛点）
- coverageMap fallback（B3-a）

---

## 下一步（未执行）

- 未决项已收敛 → 可开 `/dev-team:phase-proposal`，promote 本文件为 `openspec/changes/<name>/explore.md`
- design 阶段核对 §9 清单（jest CLI 覆盖、vitest outputFile 旗标、bun `--config=`）——其中前三项关闭 C3 降级分支
