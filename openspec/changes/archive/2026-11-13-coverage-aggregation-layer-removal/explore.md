# 取消覆盖率汇总层：从「加权成一个数」到「逐 suite 独立判定」

workflow_type: requirement

**来源**：`openspec/explores/test-exec-coverage-scope.md`（下称「前篇」）的复盘——
前篇在两个选项间反复权衡（修 override 面 vs 退役 override 面），却没问
**「汇总层的全局覆盖率数字本身该不该存在」**。本篇讨论这个上游问题。
**调研日期**：2026-11-12（基线 `2383814e0349`）
**产品前置**：用户明确裁定 —— **产品不需要覆盖率数字**。

**结论先行**：汇总层的 `coverage.measured` 是一个**语义不成立**的量：
它把不同文件范围、不同排除规则、不同度量口径的多个 suite 加权成一个百分比，
既不代表任何真实对象，又系统性掩盖单 suite 未达阈。该系统**自己在四份报告里
发现了这个缺陷**（由 agent 手工写进 findings），但门禁代码从未发现。
取消汇总层加权、改为逐 suite 独立判定，可一次性消解前篇 13 条遗留问题中的 11 条。

---

## 1. 现状：一次「加权的意义是什么」的追问

```
        子报告层（per-suite）              汇总层（summary）
  ┌──────────────────────────┐    ┌──────────────────────────────┐
  │ vite-plus                │    │ coverage.measured            │
  │   lines 95.40  ≥ 90      │    │   lines  = 条目加权（两 suite）│
  │   br    94.06  ≥ 85      │───▶│   br     = 只有 vite 有数据    │
  │   fn    91.60  ≥ 90  pass│    │   fn     = 只有 vite 有数据    │
  ├──────────────────────────┤    │                              │
  │ rust                     │    │ + overrides[]（第二副本）      │
  │   lines 95.94  ≥ 90      │    │ + conclusion（第二次判定）      │
  │   br    None   （跳过）   │    └──────────────────────────────┘
  │   fn    None   （跳过）   │                 │
  │        pass              │                 ▼
  └──────────────────────────┘        determine_conclusion
                                      （只看汇总块，不看子报告）
```

两层判定口径不同，于是同一个事实出现多套数字（前篇 §4.7 实测）：

| 口径 | rust 行覆盖 | 说明 |
|---|---|---|
| 子报告块 | 95.73 | `llvm-cov totals.lines`（144259 行） |
| 条目加权 | 95.94 | `aggregate_measured`（112709 行） |
| 汇总全局 | 95.92 | 两 suite 条目累加 |

**三个数都是"覆盖率"，没有一个是错的，但它们量的不是同一个东西。**

---

## 2. 为什么加权在语义上不成立

### 2.1 两个 suite 的量根本不是同一种东西

本仓两个 suite 的差异远超"文件不同"：

| | vite-plus | rust |
|---|---|---|
| 文件范围 | `src/**`（139~153 条） | `crates/**/src/**`（297~334 条） |
| 排除规则 | `components/ui/**` + `routes.tsx` + `types/generated/**` | 无 |
| 测试形态 | 独立 `.test.tsx` 文件 | **行内模块**（`*_test.rs` 与源码同 crate） |
| 行覆盖率含义 | 生产代码被测试触达的比例 | **测试代码本身也被计入分母** |

最后一条是致命的（前篇 §5.2 实测）：

| rust 范围 | 行覆盖 |
|---|---|
| 全部 334 条 | 95.94% |
| 只算生产码（`!*_test.rs`，154 条） | **87.60%** |
| 只算测试码（`*_test.rs`，161 条） | 97.80% |

rust 的 95.94% 里有 84% 的行数来自测试代码自身——**它天然虚高约 8 个百分点**。
把它和 vite 的 95.40% 加权平均，等于把两种不同的度量混在一起。

### 2.2 加权唯一的实际效果是掩盖

全库 90 份报告实测（前篇 §5.3）：**26 份**报告里子报告判 `pass=false`、
汇总层判 `pass=true`、`conclusion=pass`。

| 维度 | 失守次数 |
|---|---|
| `branches` | 25 |
| `functions` | 19 |
| `lines` | **0** |

`lines` 一次都没失守，因为这个漏斗专挑 branches/functions——**加权的净效果
就是让某一维度的不达标在小数点后被另一个 suite 稀释掉**。

### 2.3 「产品不需要数字」使加权失去最后的存在理由

加权的两个可能理由：

1. **展示**：产品面需要一个"总体覆盖率"——**用户已明确不需要**；
2. **门禁**：需要一个总体门槛——但门槛的正确形态是"每个 suite 各自达标"，
   不是"平均达标"（平均达标允许一个 100% 掩盖一个 0%）。

两条都不成立，`measured` 就没有存在理由。

---

## 3. 决定性证据：系统自己早就发现了，代码却没发现

agent 在四份报告的 `findings` 里**手工写下了这个缺陷的完整诊断**，
而门禁的机械判定全部判了 pass：

### 3.1 `2026-09-30-desktop-agent-management`（conclusion = **pass**）

```
summary.coverage = { pass: true,
                     measured: { lines: null, branches: null, functions: null },
                     overrides: [ { file_count: 60, passed_count: 0, pass: true } ] }
problems = []
```

而它所依据的两个子报告：

```
vite-plus   pass True   { lines: 93.78, branches: 87.52, functions: 92.56 }
rust        pass False  { lines: 96.90, branches: 0,     functions: 89.76 }   ← 未达阈
```

agent 的原话（`findings` 第 3 条）：

> **汇总层 coverage 聚合异常**：summary.coverage.measured 三维均 null 且 pass=true，
> 而两份 plan report 均有实测值（vite-plus 93.78/87.52/92.56 达标；rust 96.90/0/89.76），
> override 记 file_count 60 / passed_count 0；跨框架聚合未合并 plan 级数据，
> 需排查 coverage 聚合逻辑。

### 3.2 `2026-09-28-agent-component-ai-sdk`（conclusion = **pass**）

> **报告口径缺陷**：summary 顶层 coverage.measured 全为 null 且 coverage.pass=true，
> 但两份计划级 report.json 的 coverage.pass 均为 false（vite-plus functions 89.63<90、
> rust branches 0<85），顶层聚合把两处真实覆盖失败掩盖为通过；
> **conclusion=pass 仅在字段自洽层面成立**。

### 3.3 这四份的共同点

| 报告 | conclusion | agent 是否点名掩盖 |
|---|---|---|
| `2026-09-28-agent-component-ai-sdk` | pass | ✅ |
| `2026-09-30-desktop-agent-management` | pass | ✅ |
| `2026-10-01-agent-core-session-kernel` | pass | ✅ |
| `2026-10-03-agent-turn-eof` | pass | ✅（计数面同源） |

**四次都是 pass。** 也就是说：这个缺陷从未导致过一次门禁失败——
它只在有人（agent）人工读报告时才被发现，而 `findings` **不参与判定**
（`determine_conclusion` 不看 findings）。

> 这一条比前篇 §5.3 的 26 份统计更尖锐：26 份是"机械判定漏了"，
> 而这 4 份是"**人工已经查出并写进报告，门禁照样放行**"。

---

## 4. 替代方案：逐 suite 独立判定，汇总只做布尔与

### 4.1 目标形态

```
        子报告层（per-suite，唯一判定点）        汇总层（只做归并与聚合）
  ┌──────────────────────────┐        ┌──────────────────────────────┐
  │ vite-plus                │        │ conclusion =                 │
  │   lines 95.40 ≥ 90       │───────▶│   execution_error ? Error    │
  │   br    94.06 ≥ 85   pass│        │   : 任一 suite 非 pass ? Fail │
  │   fn    91.60 ≥ 90       │        │   : Pass                     │
  ├──────────────────────────┤        │                              │
  │ rust                     │        │ problems = 各 suite 归并       │
  │   lines 95.94 ≥ 90       │        │ findings = 各 suite 归并       │
  │   br    None（不支持）    │        │ total/passed/failed/skipped  │
  │   fn    87.60 < 90   fail│        │   = 求和（仅供人读）           │
  │                    ↑     │        │                              │
  │           判在自己这一层  │        │ ✗ 不产 coverage.measured      │
  └──────────────────────────┘        │ ✗ 不产 mutation.score         │
                                      │ ✗ 不产 overrides[]            │
                                      └──────────────────────────────┘
```

**关键：`determine_conclusion` 不再读任何覆盖率数字，只读各 suite 的 `pass` 布尔。**

### 4.2 为什么这样能一次性消解前篇 11 条

| 前篇遗留问题 | 取消汇总层后 |
|---|---|
| A 绝对路径未归一 | **消失**——`in_suite_scope` 不再被调用 |
| B `includes` 双语义 | **消失**——没有"归因范围"这个概念了 |
| §4.1 嵌套 root 串味 | **消失**——不再跨 suite 摊平 |
| §4.2 excludes 全局生效 | **消失**——excludes 只在发现面用 |
| §4.3 零命中不产组 | **消失**——不产组这件事不存在了 |
| §4.4 零计数假绿 | **消失**（但需保留"suite 无覆盖数据 ≠ 达标"，见 §5.1） |
| §4.5 诊断只读非 pass 组 | **消失**——诊断逐 suite 产出 |
| §4.7 数值口径三套并存 | **消失**——只剩子报告一个口径 |
| §4.8 rust 半边不参与加权 | **消失**——不存在加权 |
| §5 口径一变就红 | **消失**——没有"选哪个 source 范围"的决策了 |
| §2.1 spec「不变锚」 | **消失**——`compute_overrides` 整个删掉 |
| §4.6 `default_glob` 两份镜像 | **保留**（发现面仍需要，与前篇无关） |
| 前篇 §1 override 休眠 | **消失**——面本身不存在了 |

**13 条里 11 条直接蒸发，剩 1 条（§4.6）与本议题无关。**

---

## 5. 必须回答的设计问题

### 5.1 「不支持该维度」要成为一等公民

当前 rust 的 `branches` 恒 `None`（llvm-cov 在 rust 项目里 `branches.count` 恒 0），
而 `None` 现在的语义是**双关**的：既表示"没有数据"，又被判定为"跳过 = 达标"。

取消加权后这个双关必须拆开，否则同一个坑换个地方复发：

| 情形 | 应判定 |
|---|---|
| suite 报告了该维度且达阈 | pass |
| suite 报告了该维度且未达阈 | **fail** |
| suite **声明不支持**该维度（如 rust 的 branches） | **跳过**（且报告须显式标注"该维度不适用"） |
| suite 应报未报（采集缺失） | **fail**（不可测 ≠ 达标） |

关键区别在「**声明不支持**」与「**该报没报**」——前者是配置事实，后者是缺陷。
当前代码无法区分，`coverage_meets_thresholds` 对 `None` 一律跳过。

前篇 §4.4 的 51 条假绿全部落在"该报没报"这一格，却被当成"声明不支持"。

### 5.2 25 次 `branches` 失守里绝大多数是 rust `branches: 0`

前篇实测：CLI 时代 25 次 `branches` 失守，绝大多数是 rust `branches: 0`。
agent 也手工指出过这个问题（`2026-09-28`）：

> rust branches=0 属**度量口径问题**而非真实低覆盖：cargo llvm-cov --json 输出的
> files[].branches 为空数组，解析为 0% < 85 阈值……建议 rust 计划 branches 阈值置 null
> 或启用分支插桩，否则 rust 计划级覆盖恒红。

所以本条要定：rust 的 `branches` 是「显式不支持（阈值置 null + 报告标注）」，
还是「应当采集（需启用分支插桩）」？**这是配置/产品决策，不是实现细节。**

### 5.3 `mutation.score` 是否同构处理

`merge_mutation_blocks`（`aggregate.rs:579`）也是纯求和 + 取首块阈值：

```rust
let first = blocks.first()?;
let mut measured = MutationMeasured::default();
for block in blocks { /* 逐字段 += */ }
Some(build_mutation_block(measured, first.threshold))
```

同样面临"两个 suite 加权成一个 score 有意义吗"。本仓 rust mutation 阈值 70、
Stryker 80，加权后的 score 会同时掩盖两者。**建议一并改为逐 suite 判定。**

### 5.4 报告 schema 变窄的兼容面

删除 `SummaryReport.coverage` / `.mutation` 后：

- `parse_summary_report` 是宽容解析（无 `deny_unknown_fields`）→ 旧报告仍可读；
- **corpus golden 需重写**：8 份 golden 的 `computedCoverage` 会整块消失
  （前篇曾确认只删 `overrides` 是零 diff，但删除整块不是）；
- `check_integrity` 的 ①③ 两项失去对象（①是计数对账、③是 conclusion 一致性），
  需重新定义或删除。

### 5.5 复用门与 `check_integrity` 的连带影响

`reuse.rs::is_reusable` 依据 `summary.conclusion` 与 `timestamp`，
**这两项都保留**，所以复用门判据本身不受影响。但复用门还要求
`check_integrity(...).is_empty()`——若去掉 ①③，"齐整"前提要改由什么保证，
需一并设计。

---

## 6. 成本对照

| 面 | 取消汇总层加权 | 对照：前篇路径一(B+A) | 对照：前篇路径二(退役 override) |
|---|---|---|---|
| `SummaryReport` | 删 `coverage` / `mutation` | 无变化 | 删 `overrides` 字段 |
| `aggregate_coverage` | **删** | 改 | 改 |
| `weighted_measured` | **删** | 无变化 | 无变化 |
| `compute_overrides` / `in_suite_scope` / `is_file_excluded` 归因消费 | **全删** | 改 | 部分删 |
| `determine_conclusion` | 改（读布尔不读数） | 无变化 | 无变化 |
| `merge_mutation_blocks` | 改/删 | 无变化 | 无变化 |
| `check_integrity` ①③ | 重定义 | 无变化 | 无变化 |
| `diagnose_findings` | 改为逐 suite 归并 | 改 override 分支 | 删 override 分支 |
| `parse_llvm_cov` | **可选**：补 functions/branches 逐文件计数（§5.1 需要） | 无变化 | 无变化 |
| corpus golden | **需重写整块** | 零 diff（(a)） | 零 diff |
| spec delta | `desktop-checks-domain` + `desktop-test-execution`（聚合措辞全改） | 2 处 | 2 处 |
| 前篇 13 条遗留 | **11 条消失** | 全部需回答 | 全部消失 |
| config `tests[]` | **可增「维度不适用」声明**（§5.1） | 增 source 键 | 无变化 |

---

## 7. 与前篇的关系

前篇的框架是「override 那个第二副本要不要留」。本篇指出：
**第一副本（汇总层全局块）本身就是问题**，留着它再讨论第二副本没有意义。

如果本篇方案落地，前篇的 13 条里 11 条连同议题本身一起消失，
剩下 `default_glob` 两份镜像（前篇 §4.6）与本议题无关，可独立处理。

**建议**：前篇暂缓推进，先裁定本篇。若本篇采纳，前篇可整体关闭
（仅保留 §4.6 一条另立）。

---

## 8. 待裁定

1. **是否采纳「取消汇总层加权」**？
   —— 产品不需要数字（已定），门禁语义上平均达标不等价于各自达标，
   加上系统自己四次点名掩盖，我倾向**采纳**。
2. 若采纳：§5.1 的「声明不支持 vs 该报没报」如何在 config 表达？
   （`coverage.branches: null`？还是独立的 `unsupported_dimensions: ["branches"]`？）
3. §5.2：rust 的 `branches` 判「显式不支持」还是「应采集」？
4. §5.3：`mutation.score` 是否一并改为逐 suite？
5. §5.4 / §5.5：`check_integrity` ①③ 是重定义还是删除？复用门的"齐整"前提改由什么保证？
6. 前篇（`test-exec-coverage-scope`）是否随之关闭？

---

## 9. 启动条件复核（2026-11-12，基线 `7ed419e72055`）

**结论：具备启动条件，但先补 2 句裁定 + 1 处事实订正。**

复核方式：把 §4 的目标形态逐条落到现网代码上对位，看「要建的东西是否已经存在」。

### 9.1 已就位（目标形态的骨架已在生产链上，不是要新建）

| 目标形态要素 | 现网实况 |
|---|---|
| 「判定点在子报告层」 | **已存在**：`infra/checks/src/testexec/mod.rs:547` `build_sub_report` 逐 suite 用 `coverage_meets_thresholds` 算 `coverage.pass` |
| 「结论只读布尔」 | 改造量 = `determine_conclusion` 入参从汇总块换成子报告布尔；`SummaryReport.coverage/mutation` 删除 |
| 产品面零影响 | `TestExecutionOutcome` 载荷只有 conclusion / 四计数 / findings；前端 `bindings.ts` 唯一 coverage 引用是 config 阈值 |
| API 契约零摩擦 | `.dev-team/architecture/api/INDEX.toml` 中 `App.Backend.Core.Checks` = `status = "absent"` |
| 删除面干净 | `aggregate_measured` / `compute_overrides` / `file_meets_thresholds` / `weighted_measured` / `to_thresholds` / `in_suite_scope` / `default_glob`(镜像) 全链成孤；`is_file_excluded` 保留（`mutation/mod.rs:435` 归桶消费者）；`coverage_meets_thresholds` 保留（子报告判定） |
| 前篇 §4.6 也一并消失 | `aggregate.rs::default_glob` 的**唯一**消费者是 `in_suite_scope`（grep 实测）→ 删 `in_suite_scope` 即删镜像，权威注册表在 `detect.rs` 不动。故前篇 13 条是 **13/13** 消失，不是 §4.2 说的 11/13 |

### 9.2 两个真岔口（会让 spec delta 变形 / 可能溢出到 config）

**A. 覆盖率「块级缺席」判 fail 还是跳过（§5.1 + §5.2 合成一件事）**

- 缺口确认可达：suite 绿跑但无覆盖工件（jest 未开 coverage 等）→ 子报告 `coverage=None` → `misses_coverage` 不触发（它要求「非 pass 且 None」）→ 汇总 `weighted_measured` 取**别的** suite 的数值 → `pass=true`。这就是前篇 51 条假绿的机理，现网仍在。
- **维度级**的「不支持」其实已有声明机制，无需 config 新键：`parser/coverage.rs:72` llvm-cov `count==0 → None`（工具自述不支持）、node-test `-` 列 → `None`。§5.1 里真正需要产品裁定的只有**块级**这一格。
- 选 fail →「未配 coverage 的 workspace 立刻变红」，需要 config opt-out（`TestSuite` + `config.schema.json` + bindings/zod 全链）；选跳过 → 口径延续，`该报没报` 继续无人判。
- rust `branches`（§5.2）建议裁「显式不支持」：`count==0 → None` 已是工具侧声明。若裁「应采集」，连带撞 `desktop-test-execution` 的硬条款「覆盖率口径与阈值 SHALL 零调整（不排除 `*_test.rs`，lines 90 / branches 85 / functions 90）」+ llvm-cov 分支插桩 → 属另一个变更的量级。

**B. `summary.mutation` 归并块的去留（§5.3 降级为纯实现选择）**

复核发现门禁**已经**是逐 suite 判定：`mutation/mod.rs:371` `all_pass = suite_results.iter().all(|(_, block)| block.pass)`。归并块只做三件事：回填 `summary.mutation`、喂 `diagnose` 4f、作 `MutationTestOutcome.score/threshold` 展示。所以不存在「加权掩盖门禁」，只有「要不要持久化一个加权 score」。三选一：保留回填（零改动）／不写 summary（1 处删 + `desktop-mutation-testing` 一条 requirement 改，与覆盖率同一条纪律）／逐 suite 写进 summary（schema 扩面最大）。

### 9.3 事实订正（进 proposal 前先改笔记）

1. **§5.4「`check_integrity` ①③ 失去对象」半错**：① 是四计数对账、② 是 plan 齐全性，两者与覆盖率无关，零改动；只有 ③（结论一致性）受影响。
2. **§5.5 的「齐整前提」可解**：③ 不必删也不必重新发明——`check_integrity` 已收 `sub_reports` 入参，把 ③ 改成「以子报告的 `coverage.pass` / 布尔重算结论」即可，且比现状更强（现状是自己跟自己比）。复用门三条件（`plan_ids_match` + `integrity` 空 + `is_reusable`）语义全保留。
3. **§5.4 的 golden 影响面偏大**：8 份 corpus 里只有 4 份含 `computedCoverage`（`green-istanbul` / `green-llvm-cov` / `green-node-test` / `red-run`）；`mutation-block` / `unknown-fields` / 两份 `corrupt-*` 无 summary 面 → 零 diff。另需改 `corpus_golden_test.rs` 里 3 条 `computedCoverage.*` 断言。

### 9.4 可留到 design 定（不阻塞启动）

- **报告自描述度**：删 `summary.coverage` 后，单看 `summary.json` 还能否回答「哪个 suite 覆盖未达」？方向 a：`problems` 逐 suite 归并（零 schema 扩展）；方向 b：`plans[]` 增 per-plan 结论字段（schema 加宽，但 9.3-② 的一致性校验可离线）。这是本变更唯一的形态分叉，值得在 design 里显式择一。
- `merge_mutation_blocks` 的计数求和公式保留与否（`desktop-mutation-testing` 有「沿既有公式」明文）。
- 前篇关闭时机（建议本变更 archive 时一并标注）。

---

## 10. 两项裁定落地（2026-11-12，基线 `7ed419e72055`）

### 10.1 裁定 A：覆盖率「采集声明」就是 CLI 参数本身，且恒为开

用户问「能否通过 CLI 参数确定开启（fail）或跳过」。复核结论：**声明的载体已经存在，就是注册表模板**，而且五框架全部**恒带**采集旗标：

| 框架 | 模板里的采集旗标（`detect.rs` REGISTRY） | `coverage_format` | `coverage_output` |
|---|---|---|---|
| jest | `--coverage --coverageDirectory="{report_dir}" --coverageReporters=json-summary` | Istanbul | `coverage-summary.json` |
| vitest | `--coverage --coverage.reportsDirectory=… --coverage.reporter=json-summary` | Istanbul | 同上 |
| vite-plus | 同 vitest（`vp test`） | Istanbul | 同上 |
| rust | `cargo llvm-cov … --output-path "{coverage_file}"` | LlvmCov | 同上 |
| node-test | `--experimental-test-coverage --test-reporter-destination="{results_file}"` | NodeTest | `results.txt`（与结果同文件） |

注册表里 `coverage_format: CoverageFormat` **非 Option**、`coverage_output` **恒有值**——「这个 suite 采不采覆盖率」在本仓不是一个可变量，是常量。`config::TestSuite` 也没有任何 coverage 开关（`core/config/src/lib.rs:121` 只有三阈值，缺失即默认 80/70/75）。

**因此 A 落地为零 schema 变更的一格判定**：

```
子报告判定（唯一判定点，build_sub_report）
  measured 有值 + 达阈                  → pass
  measured 有值 + 未达阈                → fail
  measured 有值、某维度 None            → 该维度跳过（工具自述不支持：llvm-cov count==0 → None）
  measured == None（工件缺失 / 解析失败）→ fail   ← 今天静默、改造后显式
```

最后一行今天的静默位置已定位：`runner.rs::parse_coverage_artifact` 读不到工件即 `(Vec::new(), None)`，**不进 `error` 也不进 `notes`**——覆盖采集失败在子报告上完全无声，聚合层再拿另一个 suite 的数值顶上。

**首跑即红的风险实测为 0**：最近 6 份自产报告（`2026-11-07` ~ `2026-11-12`），两个 suite 子报告 `measured` 全部非 None：

| 报告 | rust measured | vite-plus measured |
|---|---|---|
| 2026-11-12-archi-file-tree | lines 96.06 / br null / fn 91.35 | 95.54 / 93.92 / 91.51 |
| 2026-11-11-backtrack-window-context | 95.92 / null / 91.12 | 94.89 / 91.79 / 91.26 |
| 2026-11-11-agent-finalize-tool-for-flow-steps | 95.96 / null / 91.18 | 95.60 / 93.90 / 91.76 |
| 2026-11-10-module-api-contract-format | 95.87 / null / 91.04 | 93.71 / 91.14 / 90.07 |
| 2026-11-07-agent-stream-single-message | 95.70 / null / 90.45 | 95.62 / 93.73 / 91.93 |

（另 `2026-11-11-agent-messages-virtual-list-pinning` 同形，6/6 全有值。）

**顺带实证了加权掩盖**：同一批报告里 `summary.coverage.measured.branches` 恒有值（93.73 / 91.15 / 93.91 …），而 rust 半边 `branches` 恒 `null`——这个 93.73 **100% 来自 vite 那半边**，加权把它写成了「全局分支覆盖率」。取消加权后 branches 只判 vite 一家（93.73 ≥ 85 pass），rust 半边显式「不适用」。

**A 的两个已知缺口（进 proposal 风险节）**：
1. **没有「不采集」出口**：将来若某 suite 不该采覆盖率，无处表达（三阈值非 Option 且无 enabled 开关）。今天无此需求，作为后续增量，MUST NOT 在本变更里预埋。
2. **结论语义从 Error 降为 Fail**：现网「非 pass 且无覆盖数据」走 `ProblemType::ExecutionError` → `conclusion=Error`；改造后走 `coverage.pass=false` → `conclusion=Fail`。方向上更准（`Error` 的成因面已被收窄为「工件解析 / harness 崩 / 探测失败」，不可测覆盖不属于这三类），但连带两点：`is_reusable` 只排除 `Error`（fail 报告可被复用门复用——与「覆盖未达阈」的 fail 同待遇，可接受）；`desktop-checks-domain` 的「覆盖率采集完整性（缺测不假通过）」整条 requirement + 4 个 Scenario 需重写成新形态。

### 10.2 裁定 B：`summary.mutation` 不写（归并块留在内存，只服务载荷）

删除清单（精确到函数）：

| 位置 | 动作 |
|---|---|
| `model.rs::SummaryReport.mutation` | 删字段 |
| `aggregate.rs::aggregate_mutation`（:600） | **删**（唯二消费者：summary 组装 + `merge_mutation_blocks` 输入） |
| `aggregate.rs::build_summary_report` 的 mutation 入参与 fail 问题条目（:634-645） | 删 |
| `infra/checks/src/mutation/mod.rs::backfill_summary`（:767） | **删**，调用点（:366）一并删 |
| `core/checks/src/diagnose.rs` 4f 分支（:176） | 删（`summary.mutation` 不复存在） |
| `diagnose.rs::check_integrity` ③ 的 mutation 入参 | 删（重算只剩子报告布尔） |

保留清单（**MUST NOT 顺手删**）：

- `MutationBlock` / `MutationMeasured` / `MutationOverride` 模型——`SubReport.mutation` 仍消费（corpus 保真样本）。
- `build_mutation_block`（逐 suite 段块判定，`pass` 已是逐 suite 口径）。
- `merge_mutation_blocks`——`MutationTestOutcome.score` / `threshold` / `counts` 这三个**载荷展示字段**仍要它算（`port.rs:369-374`）；只删落盘那一半。
- 门禁判据本就是逐 suite：`mutation/mod.rs:371` `all_pass = suite_results.iter().all(|(_, block)| block.pass)`——**零改动**。

一个待 design 的小岔口：`MutationTestOutcome.score` 展示的仍是「计数求和后按首块阈值算的加权分」。两个选项——(a) 原样保留并注明「展示口径，非门禁判据」；(b) 改呈「最差 suite 的 score/threshold」。建议 (a)（零改动 + 与逐 suite 明细并存），(b) 属新口径。

spec delta 面：`desktop-mutation-testing`（「归并 MutationBlock SHALL 回填 summary.json 的 mutation 位」→ 删/改）+ `desktop-test-execution`（「summary 的 `mutation` 位由 MutationTest 归并块回填（在位时），未跑 / diff 空跳过时保持 null」→ 改）。

### 10.3 启动判定

**具备。** 六条待裁定里 1 / 2 / 3 / 4 已定（采纳；无 config 新键；rust branches 显式不支持；mutation 不写 summary），5 有明确解（③ 改为以子报告布尔重算，① ② 零改动），6 留到 archive 时标注。spec delta 面收敛为三处：`desktop-checks-domain`（覆盖完整性 requirement 重写 + 聚合措辞）、`desktop-test-execution`（聚合链措辞 + golden 纪律）、`desktop-mutation-testing`（回填条款删除）。
