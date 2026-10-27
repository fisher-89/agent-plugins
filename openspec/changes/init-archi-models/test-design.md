# 测试设计: init-archi-models

> **变更**: init-archi-models
> **日期**: 2026-10-27
> **基于**: `proposal.md`（验收标准 **AC-1 ~ AC-10**；「变更范围 - 实现文件」7 项、「测试文件」无新增、「不要修改」8 面）；`design.md`（「变更清单」新增文件 **7 项** / 修改文件**零项**（空注释）/「公共函数 / API」「类型定义」「配置」三处均为空注释；D1 ~ D11；「数据模型 - 逐文件内容计划」「计数恒等式」「ADR 结构」；「依赖」表「无新增」）；`tasks.md`（首部「本相位不写测试」+ 阶段一～九落笔任务 + 阶段十「交付自查（命令 + 留痕，非测试）」）；`specs/repo-archi-model/spec.md`（7 requirement / 21 scenario）
> **被测模块来源**: design「新增文件」表 7 项 —— `openspec/architecture/models/{01-app,02-backend-core,03-backend-infra,04-backend-shell,05-backend-tools,06-frontend}.toml` 与 `openspec/architecture/decisions/0001-model-granularity.md`。design「修改文件」为空注释（本变更零修改文件）、「删除文件」不存在 ⇒ **无修改面 / 无删除面**纳入被测范围。
> **测试框架（既有，本变更零新增）**: **Rust 侧** `cargo test`，sidecar 文件与源文件同目录同前缀 `<name>_test.rs`，源文件尾以 `#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;` 挂载（先例 `packages/desktop/src-tauri/crates/core/config/src/lib_test.rs`）；crate 级集成测试在 `tests/`（先例 `packages/desktop/src-tauri/crates/core/archi/tests/{golden_test,vocabulary_test}.rs`，语料在 `tests/corpus/dsl/**`）；断言仅 `assert!` / `assert_eq!` / `matches!`，无第三方断言库。**前端侧** `vite-plus/test`（`import { describe, expect, it } from 'vite-plus/test'`，收集面 `src/**/*.test.ts?(x)`，执行器 `vp test`）。本变更**零新增测试依赖、零新增运行器、零新增导出、零新增测试文件**。
> **本稿结论（先读此段再读表）**: 本变更新增面全部是**仓库数据资产**（6 个模型 TOML）与**口径记录**（1 份 Markdown ADR），零源码改动 ⇒ **per-file 单元测试映射对 0 组**，`## 单元测试` 下零章节；7 个文件全部登记于 `## 不可测试项` 1～7，验收判据由 design D9 / proposal「测试文件」表规定的「命令 + 报告留痕」承接（本稿不复述命令细节）。

### 零映射对的成因（三条依据）

1. **无待测功能来源**：design「公共函数 / API」「类型定义」「配置」三处均为空注释（「本变更零源码改动，不新增 / 不修改任何模块级导出函数、IPC 命令、agent 工具、前端导出函数」）。按本相位纪律，`#### 待测功能` 只登记 design 明写的命名单元，MUST NOT grep 源文件导出声明或虚构条目 ⇒ 7 个文件均无 `#### 待测功能` 行可写。
2. **设计明令零测试代码**：design D9（「核对以命令 + 报告留痕执行；MUST NOT 新增测试代码、MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**`」）与 `tasks.md` 首部（「不新增 `#[test]`、不新增 `*.test.tsx`」）是同一约束的两处表述；proposal「测试文件」列为「无新增测试」。模型若进 crate 语料，会使 `core/archi` 单测随本仓内容演进变红（语料须自足、可移植）。
3. **既有测试面无该文件的读取者（实测）**: `crates/core/archi/{src/*_test.rs, tests/*.rs}` 的语料全部是 `tempfile::TempDir` 或 `tests/corpus/**` 内的自造语料；`packages/desktop/src/**/*.test.ts?(x)` 中的 `modelFile` / ADR 路径均为 fixture 字面量（例如 `openspec/architecture/models/01-core.toml` 这种**不存在的**占位路径）。仓内零测试以真实仓库路径读 `openspec/architecture/**` ⇒ 新增 7 个文件不改变任何既有用例的输入，也无法在不新增测试代码的前提下成为单测输入。

### 参数类型 → 边界覆盖口径（不适用声明）

本变更**不引入任何函数入参、字段、枚举或类型**（design「公共函数 / API」「类型定义」「配置」三处空注释），故参数类型边界表**不适用**，逐项显式声明而不虚构加载点：

| 类型 | 本变更加载点 | 判定 |
|---|---|---|
| int / number | 无 | 不适用：无新增入参 / 字段 |
| str / string | 无（模型文本值由既有 `archi` 解析器消费，其 `""` / 超长 / 特殊字符边界已由 `crates/core/archi/src/parse/toml_test.rs` 既有用例覆盖） | 不适用（不在本变更加载） |
| bool | 无 | 不适用 |
| list / array | 无新增字段（`path` / `docs` 数组类型与语义零变化） | 不适用 |
| dict / object | 无（`metadata` 本期整键不写，格式契约零变化） | 不适用 |
| Optional[T] | 无 | 不适用 |
| Enum | 无新增 / 无修改枚举（`kind` 四值 / `gate` 三档 / 四类结构诊断为既有封闭词表） | 不适用 |
| float | 无 | 不适用 |

---

## 验收范围

| AC ID | 验收条件（缩写，原文见 proposal） | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 模型可解析、零诊断：`archi_graph` 正常态，`diagnostics` / `warnings` 空，`elements` 49、`relations` 107（= 后端 43 + 前端 64）；`/architecture` 页面元素树 + 画布 + 抽屉三段且无结构错面板 | `openspec/architecture/models/01-app.toml`、`openspec/architecture/models/02-backend-core.toml`、`openspec/architecture/models/03-backend-infra.toml`、`openspec/architecture/models/04-backend-shell.toml`、`openspec/architecture/models/05-backend-tools.toml`、`openspec/architecture/models/06-frontend.toml`（无单元测试载体，见 `## 不可测试项` 1～6）；页面三段渲染见 9 |
| AC-2 | 后端覆盖齐备：14 个 crate 目录 + `src-tauri/src/` 与叶元素 `path` 集合双向相等；crate 内部 `.rs` 零元素 | `02-backend-core.toml`（7 core）、`03-backend-infra.toml`（6 infra）、`05-backend-tools.toml`（1 tools）、`04-backend-shell.toml`（壳）、`01-app.toml`（`App.Backend` / `App.Backend.Core` 等祖先容器；漏声明即 `orphanHierarchy`）——见 1～5 |
| AC-3 | 后端边口径：边集 == `[dependencies]` 段内 workspace 内声明集合（`{ workspace = true }` 与 `{ path = … }` 同权），43 条；dev 边零出现（在案反例 `workflow --dev--> orchestration`、`orchestration --dev--> store`，另实测两条）；`export-bindings → dev-team` 反向边在案 | `02-backend-core.toml`（9 条）、`03-backend-infra.toml`（21 条）、`04-backend-shell.toml`（12 条）、`05-backend-tools.toml`（1 条反向边）——见 2～5 |
| AC-4 | 前端覆盖齐备：9 视图 + 3 视图内子域 + 10 共享组件文件 + `components/ui` 与前端叶元素 `path` 集合双向相等；封闭排除面零元素；`views/changes/components`（1 文件）不构成子域 | `06-frontend.toml`（28 元素）、`01-app.toml`（`App.Frontend` 容器，仅此一处声明）——见 1、6 |
| AC-5 | 前端边口径：64 条边（22 源）逐条可指到覆盖范围内文件的 `import` 语句；6 条为 `App.Frontend.Components.Agent` barrel 端点边（2 出 + 4 入），其余 58 条为叶 → 叶 / 叶 → 容器；`types/generated/bindings` 的 import 恰映射 `App.Backend.Shell`；指向 `hooks/` / `lib/` / `types/dto` 零边；无 `dependsOnAncestor` 形态父子边 | `06-frontend.toml`——见 6 |
| AC-6 | 图自洽：零 `cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；容器 `path` 恒空（唯一例外 barrel）；容器端点仅限该 barrel 且恰 6 条 | `01-app.toml`、`02-backend-core.toml`、`03-backend-infra.toml`、`05-backend-tools.toml`、`06-frontend.toml`（10 个容器均须显式声明）——见 1～3、5、6 |
| AC-7 | 字段纪律：49 元素 `gate` 全 `off`；`summary` 全非空一行中文；叶 `path` 全 `./` 前缀；`docs` 仅 `App` 且不指向 `openspec/specs/**`；`metadata` 全空；每条 `desc` 一句用途且不含证据路径清单 | `01-app.toml` ～ `06-frontend.toml` 全部 6 文件——见 1～6 |
| AC-8 | 落位与文件划分：6 文件在 `models/` 且名 `NN-<域>.toml`；字典序聚合零 `duplicateElement`；ADR 在 `decisions/` 且正文引用元素 FQN（`archi_module` 的 `docs` 出现该条目，`source = "text"`） | `01-app.toml` ～ `06-frontend.toml` + `openspec/architecture/decisions/0001-model-granularity.md`——见 1～7 |
| AC-9 | 零代码改动：`git diff --stat` 只含 `openspec/architecture/**` 与本 change 目录；静态检查 / 测试与基线一致（3239 通过、覆盖率不降、`bindings:check` 零 diff） | —（机械核对 + 既有 3239 条套件回归锚，无新增测试载体，见 8） |
| AC-10 | 检索可用：`archi_search(query="archi", scope=all)` 命中 `App.Backend.Core.Archi` 薄条目与 ADR 文档条目（`kind = adr`、`source = text`、恰四字段）；`archi_module(element="App.Backend.Core.Archi")` 返回出边 `Foundation`、入边 `AgentRuntime` / `Shell` 与 `modelFile` / `modelLine` | `decisions/0001-model-granularity.md`（`text` 源关联）、`02-backend-core.toml`（`modelFile` / `modelLine` 落点）——见 7、9 |

> **列口径**：`被测文件或模块` 列按模板填**承载用例的测试文件路径**；本变更无任何测试载体，故全部行填「被测数据文件 + `## 不可测试项` 序号」形式，`—` 表示无被测文件。为避免重复，AC-2 ～ AC-8 行内的模型文件简写（`01-app.toml` ～ `06-frontend.toml`）均指 `openspec/architecture/models/<name>.toml`。AC 对应的「源文件 -> 测试文件」映射对 **0 组**，`## 单元测试` 下无 per-file 章节。
> **覆盖核对**：proposal AC-1 ～ AC-10 逐条在表内出现（10 行，无遗漏、无合并的独立编号）；design「新增文件」7 项逐项出现在本表（`01` 于 AC-1/2/4/6/7/8；`02` 于 AC-1/2/3/6/7/8；`03` 于 AC-1/2/3/6/7/8；`04` 于 AC-1/2/3/7/8；`05` 于 AC-1/2/3/6/7/8；`06` 于 AC-1/4/5/6/7/8；ADR 于 AC-8/10）。
> **被测范围边界**：`packages/desktop/src-tauri/crates/core/archi/**`（含 `tests/corpus/**`）、`packages/desktop/src-tauri/src/**`、`packages/desktop/src/**`、`Cargo.toml` / `Cargo.lock`、`openspec/config.json`、`.gitignore`、`openspec/specs/**`、根 `README.md` / `AGENT.md` / `CLAUDE.md`、`package.json` 均为 design「修改文件」空注释与「依赖」表「无新增」所界定的**零触碰面**，不纳入被测范围；其行为不变性由 AC-9 的既有套件全绿承接。

---

## 单元测试

<!-- 零 per-file 映射对：design「新增文件」7 项均为 *.toml / *.md 数据与文档资产，「公共函数 / API」「类型定义」为空注释（无待测功能来源），D9 与 tasks.md 首部明令零新增测试代码，且仓内零测试以真实路径读取 openspec/architecture/** ⇒ 无一源文件可解析到测试文件。故本节不建任何 ### 源文件 -> 测试文件 章节，也不建 #### 待测功能 / #### 用例 / #### Mock策略 表；7 个文件的判定与承接通道见 ## 不可测试项 1～7。跨模块「组合」三条核对（双向覆盖 / 关系证据 / 图自检）由 design D9 定为命令 + 报告留痕，不是单元或组合测试，因此也不新建集成测试章节或组合测试区。 -->

本变更新增文件 7 项，**per-file 单元测试映射对 0 组**，故本节零章节。逐文件清点（`测试文件` 列恒为 `—`，即不可解析，理由指向 `## 不可测试项`）：

| 新增文件（design「变更清单」） | 解析到的测试文件 | 判定依据 |
|---|---|---|
| `openspec/architecture/models/01-app.toml` | — | 数据资产；无公共 API；D9 零新增测试代码 —— 见 `## 不可测试项` 1 |
| `openspec/architecture/models/02-backend-core.toml` | — | 同上 —— 见 2 |
| `openspec/architecture/models/03-backend-infra.toml` | — | 同上 —— 见 3 |
| `openspec/architecture/models/04-backend-shell.toml` | — | 同上 —— 见 4 |
| `openspec/architecture/models/05-backend-tools.toml` | — | 同上 —— 见 5 |
| `openspec/architecture/models/06-frontend.toml` | — | 同上 —— 见 6 |
| `openspec/architecture/decisions/0001-model-granularity.md` | — | 文档资产（被 `docs.rs` 的 `text` 源读取）；无公共 API —— 见 7 |

> 若后续相位（`archi-check-and-gate`）把覆盖核对与关系证据核对接成门禁，其测试载体应在该变更内设计（届时判据与语料由该变更裁定）——本变更不预铺（design D10）。

---

## 不可测试项

1. **`openspec/architecture/models/01-app.toml`（3 元素 / 0 关系）** —— 判定：**无单元测试载体**。原因：① 纯数据资产，无函数 / 类型 / 入参（design「公共函数 / API」空注释）；② design D9 与 `tasks.md` 首部明令零新增测试代码，且 MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**`（语料须自足可移植，否则 crate 单测随本仓内容演进变红）；③ 既有测试语料全部为 `tempfile` / `tests/corpus/**` 自造语料，仓内零测试以真实路径读取该文件（实测）。承接通道：本文件的 `App` 根元素与 2 个域容器是全部 FQN 的祖先（缺它即 `orphanHierarchy`），其判据由 AC-1 / AC-2 / AC-4 / AC-6 / AC-7 / AC-8 承接，核对以 design D9 的「命令 + 报告留痕」执行（`tasks.md` 阶段二自查项 + 阶段十计数 / 字段纪律自查），由桌面端与评估相位落盘。

2. **`openspec/architecture/models/02-backend-core.toml`（8 元素 / 9 关系）** —— 判定：无单元测试载体（原因同 1）。承接通道：AC-1（可解析零诊断）、AC-2（7 个 core crate 覆盖）、AC-3（9 条 `[dependencies]` 边，且 `workflow --dev--> orchestration` 与 `workflow --dev--> store` 两条 dev 边零出现——它们与生产边合起来即 `cycle`）、AC-6（`App.Backend.Core` 容器显式声明）、AC-7（字段纪律）、AC-8（`modelFile` / `modelLine` 落点，AC-10 依赖它）；核对方式为 `tasks.md` 阶段三 / 阶段十自查 + `archi_module` 实调留痕。

3. **`openspec/architecture/models/03-backend-infra.toml`（7 元素 / 21 关系）** —— 判定：无单元测试载体（原因同 1）。承接通道：AC-2（6 个 infra crate 覆盖）、AC-3（21 条边含跨域 `Store → App.Backend.Core.Workflow`；`orchestration --dev--> store`、`checks-runtime --dev--> workflow` 零出现）、AC-6、AC-7、AC-8；核对方式同 2。

4. **`openspec/architecture/models/04-backend-shell.toml`（1 元素 / 12 关系）** —— 判定：无单元测试载体（原因同 1）。承接通道：AC-2（`App.Backend.Shell` 覆盖 `packages/desktop/src-tauri/src/`）、AC-3（12 条全依赖汇聚出边；`to` 中不出现 `Core.Checks`）、AC-7（`path` 单目录形态 `./` 前缀）、AC-8；核对方式同 2。

5. **`openspec/architecture/models/05-backend-tools.toml`（2 元素 / 1 关系）** —— 判定：无单元测试载体（原因同 1）。承接通道：AC-2（`export-bindings` 覆盖）、AC-3（唯一反向边 `App.Backend.Tools.ExportBindings → App.Backend.Shell` 在案，来自 `path = "../../../"` 声明）、AC-6（`App.Backend.Tools` 容器显式声明）、AC-7、AC-8；核对方式同 2。

6. **`openspec/architecture/models/06-frontend.toml`（28 元素 / 64 关系）** —— 判定：无单元测试载体（原因同 1）。承接通道：AC-1（前端边计入 107 总量）、AC-4（9 视图 + 3 子域 + 10 共享组件 + `Ui` 双向覆盖；封闭排除面零元素；`views/changes/components` 不构成子域）、AC-5（64 条 import 证据、6 条 barrel 端点边、`bindings` → `App.Backend.Shell`、无父子向边）、AC-6（`App.Frontend.Components.Agent` 为唯一持 `path` 的容器且端点边恰 6 条）、AC-7、AC-8（`App.Frontend` MUST NOT 在本文件重复声明）；核对方式为 `tasks.md` 阶段七 / 八 / 十自查 + 报告内逐边证据清单。

7. **`openspec/architecture/decisions/0001-model-granularity.md`** —— 判定：**无单元测试载体**。原因：① 纯文档资产，被 `crates/core/archi/src/docs.rs` 的 `text` 源以 `content.contains(fqn)` 读取，不提供任何可调用单元；② 关联行为的实现（`docs.rs` 的三源排序、`docNotFound` 只报不拦）已由既有 `crates/core/archi/src/docs_test.rs` 覆盖，本文件只是其**输入语料**，为其新增用例即违反 D9（语料自足）；③ 仓内既有 `docs_test` 的 ADR 语料全在 `tempfile` 内自造。承接通道：AC-8（`archi_module` 响应中 `docs` 出现该条目、`source = "text"`）与 AC-10（`archi_search` 命中 `kind = adr` / `source = text` / 恰四字段的文档条目），通过 agent 只读工具实调一次留痕（见 9）。

8. **AC-9 零代码改动与基线一致（机械面）** —— 判定：**无单元测试载体，且不得以新增测试代替**。原因：判据是仓库级 diff 与既有套件结果（`git diff --stat` 只含 `openspec/architecture/**` 与本 change 目录；`bindings:check` 零 diff；静态检查 / 测试与 `reports/baseline/summary.json` 的基线同值——**3239 通过 / 0 失败 / 覆盖率 97.27%（lines）/ 93.81%（branches）/ 94.59%（functions）**）。为本变更新增一条「仓库无改动」测试会把仓库状态耦合进单测（同 D9 的理由），且该判据不由任何 `#[test]` 表达。承接通道：桌面端 static-check / test-execution 相位执行，结果与基线对拍；本稿不复制运行脚本细节。

9. **AC-1 页面实跑与 AC-10 工具实调（进程边界消费面）** —— 判定：**无单元测试载体**。原因：`/architecture` 页面的「元素树 + 画布 + 抽屉三段 / 无结构错面板」是**桌面端实跑**观察项（design「图表消费面（零改动）」；既有 `packages/desktop/src/views/architecture/*.test.tsx` 以 fixture 驱动该面，其 49 / 107 规模的真实数据展示不属于单测判据）；`archi_search` / `archi_module` 的检索输出是 agent 会话内的一次**工具实调**（proposal AC-10 明写验证方法「工具实调」）。两者都无法在不新增测试代码的前提下落到既有测试载体，且新增即违反 D9 / 「零代码改动」。承接通道：AC-1 由页面实跑留痕、AC-10 由工具响应留痕（`tasks.md` 阶段十「图自检」「检索自查」行），归报告相位。

10. **版本纪律（非 AC：design D8 / proposal R-8 / design O-2）** —— 判定：**无被测文件**。原因：本变更零 desktop 源码改动 ⇒ 按「版本随 desktop 源码变更」纪律 `packages/desktop/package.json` 保持 `0.6.0`、`tauri.conf.json` 与 `src-tauri/Cargo.toml` 零 diff；无代码可测。若评审裁定模型属 desktop 资产变更（O-2 备选），则改为 `0.6.1` 单行——该判定是评审口径而非测试判据，登记于此以免被误当作遗漏。承接通道：`tasks.md` 阶段十「版本自查」+ 桌面端 diff 留痕。
