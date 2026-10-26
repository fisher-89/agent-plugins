# 测试执行命令行超平台上限（Windows cmd 8191）治理方案

workflow_type: bug-fix

## 主题与背景

`packages/desktop` 的 test-execution 门禁在 `desktop-build-mode-split` 变更上判 `error`，
输出只有一行 `The command line is too long.`，但同一份报告里 rust suite 实际全绿
（`total 2944 / passed 2944`）、`coverage-summary.json` 也已落盘。结论不是代码坏了，
是 **harness 把展开后的命令拼超了 Windows `cmd.exe` 的 8191 字符硬上限**，
`cmd /C` 在 76 ms 内直接拒绝执行（`exit 1`），一个用例都没跑。

本文记录：证据链、根因分层、①~⑤ 五项处置、验收口径与实施顺序。

## 证据链

**① 失败点定位**（`reports/test/exec-outputs/exec-1792996742834-28.log`）

- 首行 `The command line is too long.`，随后大段 `cargo-llvm-cov` 编译输出属 **rust plan**。
  `testexec/mod.rs:152-171` 把各 plan 输出按 plan 序裸拼进同一个 `.log`，**无 plan 分隔头**，
  因此日志看起来像「Rust 编译被拒」，实际两段不同源。
- `reports/test/summary.json`：`problems[0].framework = "vite-plus"`、`exit_code 1`、
  `duration_ms 76`（立即拒绝，非执行失败）；`conclusion = "error"`。
- `reports/test/packages_desktop_src-tauri_rust/` 下 `results.txt` + `coverage-summary.json` 齐备。

**② 上限实测**（本机 `cmd /C <script>`，脚本长 N）

| N | 结果 |
|---|---|
| 8000 / 8150 | exit 0 |
| 8200 / 8300 | exit 1，`The command line is too long.` |

**③ 长度核算**（`detect.rs:76` 模板 + `runner.rs:195` `expand_template` 的 `files.join(" ")`，
按 detect 同口径枚举文件后精确重建命令）

| 配置 | 文件数 | 命令长度 | 余量（8191） |
|---|---|---|---|
| 现状 `src/**/*.{ts,tsx}` | 180 | **8230** | **−39** |
| 收窄 `src/**/*.{test,spec}.{ts,tsx}` | 87 | 4542 | +3649 |

上一轮 baseline（`reports/baseline/summary.json`，`conclusion = pass`）是 178 个文件 ≈ 8145，
余量仅 46 字符。本变更新增 `src/hooks/use-app-environment.ts` + `.test.ts`（77 字符）即越线。
**这是一个长期贴红线运行的隐性脆弱点，不是本变更引入的缺陷。**

## 根因分层

**L1 配置面**：`openspec/config.json` 的 suite `includes` 被配成「全部源文件」
（`src/**/*.{ts,tsx}`，180 个），而 `detect_files` 的语义里 `includes` 是**测试文件发现 glob**
（缺省 `default_glob = **/*.{test,spec}.{js,ts,jsx,tsx}`，87 个）。其中 93 个纯源文件对
`vp`/`vitest` 的位置参数是**无效过滤器**，白耗 3.9k 字符。

**L2 执行面**：`runner.rs:84` 无条件把 `plan.files` 全量塞进 `{files}`，**没有任何平台长度预算**。
一旦展开超限，唯一的结局是 shell 层进程级拒绝——归因还落到了「被测代码非零退出」面。

**L3 归因面**：`runner.rs:188` `non_zero_exit_error` 走 `PlanExecution.error` →
子报告 `findings` → `problems: execution_error`。harness 限制被当作被测代码问题，
会诱导修复会话去改代码。

**L4 诊断面**：`exec-outputs` 单文件无 plan 分隔头、不记展开后命令（或其长度、是否降级），
人工回查必须反推模板。

---

## 处置一：配置面收窄（立即解 gate，零代码改动）

> **本文件受写保护，Agent 不得改写。** 以下为待你手工执行的改动。

`openspec/config.json` → `tests[0]`（`root: "packages/desktop"`, `framework: "vite-plus"`）：

```diff
- "includes": ["src/**/*.{ts,tsx}"],
+ "includes": ["src/**/*.{test,spec}.{ts,tsx}"],
```

`excludes` 原样保留（`src/components/ui/**`、`src/types/generated/**`、`src/routes.tsx`
下当前无任何测试文件，保留作为未来护栏）。

**为什么这样改而不是别的：**

- `vp`/`vitest` 的位置参数是**路径子串过滤器**，不认 glob（实测 `vp test list "src/hooks/**"`
  返回空，`vp test list "src/hooks"` 才命中）。因此「传 glob 省字符」不可行，只能
  「传更少的真实文件」或「不传」。
- 实测 `npx vp test list`（不传过滤器）= 87 个文件，与 `find` 结果 **diff IDENTICAL**；
  收窄后的 include 集合与之同源，语义零损失。
- 已按 detect 同口径重建命令验证：`vp test list <87 个文件>` → `exit 0`、87 个文件全部解析，
  命令长 4542（余量 3649）。

**已验证不受影响的消费面：**

| 消费点 | 位置 | 是否受影响 |
|---|---|---|
| `{files}` 展开 + 空集剪枝 | `testexec/detect.rs` `detect_files` / `detect_plans` | 期望内（87 > 0，仍产 plan） |
| mutation 归桶输入 | `mutation/mod.rs:157` `bucket_suites(files)` | 否——输入是 change 文件清单，`includes` 不参与 |
| suite excludes 过滤 | `aggregate.rs:346` `is_file_excluded` | 否——只读 `excludes` |
| per-suite 覆盖 override | `aggregate.rs:152` `in_suite_scope` | 否——**该面当前恒空**，见下 |

**残留耦合（必须在处置五解决，否则 ① 会埋雷）：**

`includes` 被两个语义重载：`detect` 用它发现**测试文件**，`in_suite_scope`
（`aggregate.rs:394`）用它圈定**覆盖 source 文件**。后者今天不生效——覆盖条目携带
Windows 绝对路径（`D:\...\packages\desktop\src\app.tsx`），而 include 模式是 root 相对串
（`packages/desktop/src/**/*.{ts,tsx}`），`match_glob` 全匹配失败；同文件的
`is_excluded_by_suite`（`aggregate.rs:354`）走了 `relative_to_root` 归一，所以 excludes 生效、
includes 不生效。实证：`reports/baseline/summary.json` 的 `coverage` 块**没有 `overrides` 字段**
（`compute_overrides` 每个 suite 都 `matched.is_empty() → continue`）。

后果：一旦有人修掉这个绝对路径归一（处置五），① 收窄后的 `includes` 会把「圈定 source 文件」
变成「圈定测试文件」——override 圈到的将是测试文件自身，比今天更错。所以两项必须配对处理。

---

## 处置二：runner 展开预算与安全降级

**位点**：`packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs`

- 常量（仅 Windows 臂）：`const CMD_LINE_BUDGET: usize = 7000;`——留 1191 字符给 `cmd /C`
  前缀、`raw_arg` 转引后的实际线长放大，以及不同 Windows 版本的细微差异。
  unix 臂不设降级（`ARG_MAX` 2 MB 量级，没有必要引入分叉语义）。
- 位置：`execute_plan` 第 84 行 `expand_template(...)` 之后、`ensure_program_resolvable` 之前。
- 形态：`expand_template` 增加 `expand_mode` 参数（`WithFiles` / `OmitFiles`），
  预算判定不改变模板其他占位符。

**降级策略（分两档，避免静默跑超集）**

```
展开长度 ≤ 预算                       → 原样执行（精确子集，语义不变）
展开长度 >  预算 ∧ 降级安全            → OmitFiles 重新展开 + info finding
展开长度 >  预算 ∧ 降级不安全          → RunFault::Environment（可执行指引文案）
```

**「降级安全」判据（关键设计）**：`{files}` 省略后框架走自身发现，其集合是否等于
`plan.files` 决定降级会不会跑出超集。用**集合相等**而非模式文本比较来判定：

```
safe_to_omit = glob(root, 框架 default_glob) 的集合 == plan.files 集合
```

- `includes` 未设 或 `includes` 与 `default_glob` 等价 → 集合相等 → 安全降级（跑的还是同一批）。
- `includes` 是**真子集**（例如只跑 `src/hooks/**`）→ 集合不等 → 拒绝降级，报
  `RunFault::Environment("suite includes 展开 {len} 字符 > cmd.exe 8191 上限，且 includes 为收窄集，无法安全降级：请收窄 includes 或拆分 suite")`。
  宁可显式报错，也不静默把范围放大成「全 suite + 无关失败」。

**信息留痕**：`PlanExecution` 增 `argv_note: Option<String>`，在 `mod.rs:503`
（`findings: execution.error.map(...)`）与既有 error 并列追加，例如
`"vite-plus 展开命令 8230 字符超 cmd.exe 8191 上限，已降级为框架自发现（87 个测试文件）"`。
`exit_code == 0` 时该条以 finding 形式随子报告落盘，不改变 `conclusion`。

**为什么不做分块重跑**：多段执行需合并 istanbul `json-summary`（同文件键覆盖、总数需重算）
与 `results`，且各段退出码要归并；收益仅是「保留精确子集」这一增量语义，成本与风险远高于
「安全降级 + 显式拒绝」。`mutation/segments.rs` 的分段是为**执行时长**服务，两者动机不同，
不合并复用。

**已否决的替代路径**：

| 手段 | 否决理由 |
|---|---|
| 传 glob 而非展开清单 | `vp`/`vitest` 不认（实测空结果）；jest `testPathPattern` 是 regex；node-test 不展开——跨框架无法统一 |
| `--dir` 收窄扫描根 | 只有 vitest 系支持，且单目录无法表达 include 集合 |
| 写临时 `.cmd` / 响应文件 | 批处理单行同样受 8191 限制；vitest 无 `@file` 语法 |
| argv 直传绕开 `cmd /C` | `vp` 是 `.cmd` 仍过 cmd；即便换 `.exe` 也只是把上限抬到 `CreateProcess` 32767，治标 |

---

## 处置三：归因修正——超限属执行面环境错误

`RunFault`（`core/orchestration/src/port.rs:170`）已有 `Environment(String)` 面，
testexec 也已为超时 / 瞬态耗尽 / 取消建立了「fault 先于结论」的收口（`mod.rs:158-196`）。
命令行超限与它们同层：

- 判 `Environment` 后 `mod.rs` 走 `fault_outcome`：**不写 summary、不落子报告、不进复用门**，
  结论占位 `Fail`，`exec_id` 照旧回填。这正是我们想要的——半套报告不该被复用。
- 文案要点：① 是 harness 限制不是代码问题；② 数字（展开长度 / 上限 / 文件数）；
  ③ 可执行动作（收窄 `includes` / 拆分 suite）。

配套：`openspec/specs/desktop-test-execution/spec.md` 的 diagnostics 段需补
「命令行超平台上限 → `RunFault::Environment`，MUST NOT 落 `execution_error`」，
与既有「超时 / 瞬态耗尽 → fault」并列。

---

## 处置四：诊断可读性

1. **plan 分隔头**：`mod.rs:171` 聚合前写入 `\n=== plan {id} ({framework}) ===\n`。
   当前无头拼接直接导致本次误判（rust 输出被读成超长命令错误的上下文）。
   注：spec 现有措辞「record 内容为脚本真实输出」需显式豁免分隔头（元数据，非脚本输出）。
2. **展开命令可回查**：子报告 findings / exec record 头部记录三条：展开长度、是否降级、
   文件数。完整命令串因体积原因不落盘（可复算：模板 + 同口径枚举）。

---

## 处置五：`includes` 双语义解耦 + per-suite override 复活

两个独立缺陷必须一起处理，否则 ① 落地后修后者会引入更错的判定。

**缺陷 A（`in_suite_scope` 绝对路径不归一）**

`aggregate.rs:394` 的 include 匹配直接拿 `posix` 全路径去 match root 相对模式；
同文件的 `is_excluded_by_suite` 用 `relative_to_root` 归一。修法：include 匹配同样取
`relative_to_root(path, root)`，命中则 `match_glob(rel_to_root, pattern)`。
修后 desktop 的 per-suite 覆盖 override 才真正生效（`overrides` 字段首次出现在 summary）。

**缺陷 B（`includes` 一物二义）**

- desktop `detect`：`includes` = 测试文件发现 glob（喂 `{files}`）。
- desktop `aggregate` / CLI `test-report.ts:534,749` / CLI `test-resolve-paths.ts:313`：
  `includes` = **source 文件范围**（喂覆盖 / 突变 override 与「源码→单测」映射）。

两者对同一字段的期望集合不同（测试 vs 源文件）。收窄后 CLI 的
`test-resolve-paths` 会枚举出空 source 集合、override 圈不到任何源文件——这是启动
① 的已知代价（本仓 gate 由 desktop 原生执行，CLI 路径为已退役插件，实际不受影响）。

**长期解**：config schema 增可选键（`source_globs` 或 `coverage_scope`），
缺省回落 `includes` 以保持向后兼容；`aggregate` / CLI 改读新键，`detect` 独占 `includes`。
该项需动 `plugins/dev-team/bin/dev-team-config.schema.json` + `config` crate 的 assemble
（未知键 passthrough 已支持，但受保护 / 有语义的键需显式建模），排在本方案最后。

---

## 验收口径

| # | 场景 | 期望 |
|---|---|---|
| 1 | `openspec/config.json` 收窄后跑 test-execution 门禁 | `conclusion = pass`，vite-plus 子报告 `total = 87 个文件`全绿，`exec-outputs` 无超长报错 |
| 2 | 展开命令 ≤ 预算（如 rust 档 / 收窄后的 vp 档） | 命令与改造前逐字符一致（无降级痕迹） |
| 3 | 展开命令 > 预算 ∧ includes 等价 default_glob | 执行成功 + 子报告 findings 含降级说明 + `conclusion` 不受影响 |
| 4 | 展开命令 > 预算 ∧ includes 为真子集 | `RunFault::Environment`，无 summary 落盘，文案含长度 / 上限 / 动作建议 |
| 5 | 修缺陷 A 后 | `summary.coverage.overrides` 出现两个 suite 组，且圈定的是**源文件**而非测试文件（需 B 的前置：解耦键或按源文件 glob 判定） |

## 测试设计（Rust 单测）

`testexec/runner_test.rs` 表驱动补：

- `expand_template` 五占位符 × `WithFiles` / `OmitFiles` 两态（`{files}` 连带前导空白剥离，
  与 `{config_args}` 既有语义对齐）；
- 预算判定：`len == budget` 不降级 / `len == budget + 1` 降级；
- 降级安全判据：`includes == None`、`includes == default_glob`、`includes` 真子集三臂；
- `argv_note` 进 `SubReport.findings`（`mod.rs` 组装面）；
- unix 臂不降级（`cfg` 断言）。

`aggregate_test.rs` 补：绝对路径覆盖条目（`D:/p/app/src/a.ts`）经 `in_suite_scope` 命中
includes（缺陷 A 的回归锚）。

## 实施顺序与依赖

1. **处置一**（你手工改 config）—— 立即解 gate，无依赖。
2. **处置三 + 四**（归因 + 诊断）—— 纯增信息，不改执行语义，可独立合入。
3. **处置二**（预算与安全降级）—— 依赖处置三的 `RunFault` 出口存在（已存在，可直接用）。
4. **处置五**—— 先解耦（B）再修绝对路径（A），否则 override 圈错集合；
   涉及 config schema 演进，建议单独变更。

## 影响面与风险

| 项 | 评估 |
|---|---|
| 执行耗时 | 收窄后 vp 档仍跑 87 个文件（与框架自发现一致），无变化 |
| 覆盖数值 | 覆盖率由工具自身配置（`vite.config.ts`）决定，`includes` 不参与采集，数值不变 |
| 突变门禁 | 输入是 change 文件清单，不受影响 |
| 假绿风险 | 降级只在「集合相等」时发生；收窄集一律显式报错，不静默放大范围 |
| 复用门 | `Environment` fault 不落 summary / 子报告，半套报告不会污染复用判定 |
| CLI（已退役插件）路径 | `includes` 收窄后 `test-resolve-paths` / override 圈定失效——已知代价，靠处置五的新键恢复 |

## 规格与文档同步清单

- `openspec/specs/desktop-test-execution/spec.md`：`{files}` 展开预算与安全降级规则；
  超限归 `RunFault::Environment`；exec record 分隔头豁免。
- `openspec/specs/desktop-checks-domain/spec.md`（若有覆盖 scope 措辞）：`in_suite_scope`
  绝对路径归一。
- `openspec/specs/config-schema/spec.md`：新增可选 source 范围键（处置五 B）。
- `openspec/specs/cli-unit-test-execute/spec.md`：`includes` 语义收口（若保留 CLI）。
- `openspec/todo.md`：登记「处置五」为独立条目。
