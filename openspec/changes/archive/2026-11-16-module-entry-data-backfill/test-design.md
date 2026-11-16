# 测试设计: module-entry-data-backfill

> **变更**: module-entry-data-backfill（`entry` 三处归位 · 分桶 15 / 26 → 12 / 27 · 基线换段）
> **日期**: 2026-11-16
> **基于**: `proposal.md`（AC-1 ~ AC-9、「变更范围 - 测试文件」表「新增无 / 改写无」+「验收留痕」行、「不要修改」7 面）；`design.md`（「新增文件」空注释、「修改文件」**6 项**、「公共函数 / API」「类型定义」「路由 / API 设计」三处整段省略、「配置」表 5 行、事实校准 9「零测试面」、D1 ~ D10）；`tasks.md`（阶段一～五，零测试任务）；四份 spec delta（`repo-archi-model` / `desktop-archi-api-check` / `desktop-archi-api` / `desktop-archi-model`）
> **被测模块来源**: design「修改文件」子表 6 项 —— `.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/models/05-backend-tools.toml`、`openspec/changes/module-entry-data-backfill/specs/desktop-archi-api/spec.md`、`openspec/changes/module-entry-data-backfill/specs/desktop-archi-api-check/spec.md`、`packages/desktop/package.json`、`packages/desktop/src-tauri/tauri.conf.json`。design「新增文件」为空注释（本变更零新增文件）、无删除文件 ⇒ **无新增面 / 无删除面**纳入被测范围。
> **测试框架（既有，本变更零新增）**: **Rust 侧** `cargo test --workspace`，sidecar 测试文件与源文件同目录同前缀 `<name>_test.rs`（`#[cfg(test)] mod` 挂载），断言 `assert!` / `assert_eq!` / `matches!`，零第三方断言库；crate 级集成测试在 `tests/`（`crates/core/archi/tests/{golden_test,pilot_test,vocabulary_test,perf}.rs`，语料 `tests/corpus/{dsl,drift}/**` 自足）。**前端侧** `vite-plus/test`（`import { describe, expect, it, vi } from 'vite-plus/test'`，收集面 `src/**/*.test.ts?(x)`，执行器 `vp test`）。本变更**零新增测试依赖、零新增运行器、零新增测试文件、零改写测试文件**。
> **本稿结论（先读此段再看表）**: 6 个修改文件全部是**模型数据资产**（2 份 TOML）、**spec delta 文本**（2 份 Markdown）与**版本 / 配置面**（1 份 JSON + 1 份零内容改动 JSON），零源码改动 ⇒ **per-file 单元测试映射对 0 组**，`## 单元测试` 下零章节。6 项逐项登记于 `## 不可测试项` 1 ~ 8，另 3 项承接面（宿主内实测、`api/**` 零 diff 面、存活 spec 归档合并）登记于 9 ~ 11。

### 零映射对的成因（三条设计依据 + 一条实测修正）

1. **无待测功能来源**：design「公共函数 / API」与「类型定义」两处为整段省略注释（原话：`api::paths` 的 `entry_candidates` / `entry_state` / `generated_path`、`api::check::check`、`api::mod::{index_entries, generated_path_of}`、`entries` 的 IPC 面「无任何签名 / 行为变化」；`Element.entry` / `RawElement.entry` / `ArchiElement.entry` 三处字段面由变更 A 交付）。按本相位纪律，`#### 待测功能` 只登记 design 明写的命名单元，MUST NOT grep 源文件导出声明或虚构条目 ⇒ 6 个文件均无 `#### 待测功能` 行可写。
2. **设计与提案明令零测试代码**：`proposal.md`「测试文件」表「新增测试 **无** / 改写既有测试 **无**」，理由为「仓库模型属数据资产，`desktop-archi-api-check` 明写 MUST NOT 把本仓模型与契约塞进 crate 语料，基线表亦明写不写进断言（易随元素增减漂移）」；`design.md` 事实校准 9「零测试面」同结论。为两行数据新增测试、或把 `12 / 27` 写进期望值，会把「报告数与实测对拍」的责任从 change 留痕搬到脆弱常量上。
3. **数据 / 文本资产无测试文件命名惯例可映射**：`04-backend-shell.toml`、`05-backend-tools.toml`、两份 delta `spec.md`、`package.json`、`tauri.conf.json` 在既有布局下均无同前缀测试文件形态（无 `04-backend-shell_test.toml` / `spec_test.md` / `package_test.json` 之类），仓内也无任何一侧测试以「读取该文件并断言其内容」为职责的专用载体 ⇒ 6 项皆无可解析的测试路径。
4. **实测修正（登记事实，不改本稿结论）**：proposal「测试文件」表称「全仓测试零读取 `.dev-team/architecture/models/**`」——按**逐元素断言**这一读法成立（前端 fixture 的 `modelFile` 是字面占位串，如 `packages/desktop/src/app.test.tsx:188` 的 `.dev-team/architecture/models/01-core.toml`，不读盘；`crates/core/archi/tests/corpus/**` 与各 `*_test.rs` 的模型语料均在 `tempfile` 内自造），但 **`packages/desktop/src-tauri/crates/core/archi/tests/pilot_test.rs` 会以真实仓库根整目录解析模型**：`:542` 的 `api_ref(&root, FQN)` → `api::mod::elements_of` → `dto::graph_view(root)`，且 `diagnostics` 非空即 `Err`。故本变更的三行数据**有既有消费面**：值形态写坏（数组 / 越界 / 与块结构不匹配）会使该用例红。**但它对 `App.Backend.Infra.ArchiRuntime` 的两条断言（`by_fqn == projection`）不因三行 `entry` 改变**——`entry_state` 的跨元素唯一判据只看各元素 `path` 派生的候选表，三处新值与试点入口零交集（design 事实校准 3 同结论）⇒ **无需改写该测试**，故仍不构成 per-file 映射对；它以「既有回归锚」身份登记于 `## 不可测试项` 3。

### 参数类型 → 边界覆盖口径（不适用声明）

本变更**不引入任何函数入参、类型、枚举或配置键**（design「公共函数 / API」「类型定义」整段省略；「配置」表 5 行的 `entry` / `version` 行变化均为既有键的**值**，无新增键、无类型变化）。逐项显式声明而不虚构加载点：

| 类型 | 本变更加载点 | 判定 |
|---|---|---|
| int / number | 无新增入参 / 字段（`INDEX.toml` / 报告的计数字段面零改动） | 不适用：无加载点 |
| str / string | `entry` 三处字面值（`""` / 超长 / 特殊字符 / 绝对路径等形态边界） | **不适用（不在本变更加载）**：`entry` 的形态与落位四判据（`repo-relative` / `within_path` / `is_file` / `duplicate_of`）及 `Malformed` / `OutsidePath` 分支已由变更 A 连同 `crates/core/archi/src/api/paths_test.rs`（85 个用例，含 `generated_path与entry_state拒收越界形态`）与 `check_test.rs`（`touch_entry` 三态驱动）覆盖；本变更只写数据值，判据面零改动 |
| bool | 无新增字段（`passed` 为派生读数，非本变更输入） | 不适用 |
| list / array | 无新增集合字段（`path` / `docs` 的数组语义零变化，`entry` 明确为单值字符串） | 不适用 |
| dict / object | 无新增对象字段（`metadata` 全部保持缺省） | 不适用 |
| Optional[T] | 无新增可选字段（`entry` 的 `Option<String>` 语义由 A 交付，`None` 分支即 `Absent`，已由 `paths_test.rs` 覆盖） | 不适用 |
| Enum | 无新增 / 修改枚举（`kind` 四值、`gate` 三档、`status` 三态、`EntryForm` 两值、十值判据词汇全部逐字不动） | 不适用 |
| float | 无 | 不适用 |

---

## 验收范围

<!-- 逐条映射 proposal.md「验收标准」AC-1 ~ AC-9：
  - `AC ID`：proposal 验收标准编号
  - `验收条件`：原文摘录（缩写）
  - `被测文件或模块`：本变更无任何测试载体，故按「被测数据 / 文本文件 + 不可测试项序号」填写；`—` 表示无被测文件。
    源文件 → 测试文件的映射对为 0 组（见上文「零映射对的成因」），`## 单元测试` 下无 per-file 章节。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 三处 `entry` 在案且形态合法：`grep -c '^entry = ' .dev-team/architecture/models/*.toml` 合计 **3**（`04` 1 / `05` 2 / 其余四份 0）；三行均为带 `./` 前缀的单值字符串、带扩展名、逐条落在各自元素 `path` 之内；MUST NOT 第四处（合计 3 即机械拦挡） | `.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/models/05-backend-tools.toml`（无单元测试载体，见 `## 不可测试项` 1、2） |
| AC-2 | 模型零结构错、计数不变：`archi_graph(root)`（宿主须由本工作树重建）返回 51 元素 / 113 关系、`diagnostics` 与 `warnings` 为空；逐文件计数与表头注释同值（`04` 1 / 13、`05` 3 / 3）；反向判据：出现指向 `entry` 的 `unknownField` 即宿主落后，MUST 重建宿主重跑 | `.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/models/05-backend-tools.toml`（见 1、2；宿主内实测面见 9；既有模型解析消费面回归锚见 3） |
| AC-3 | 分桶换段、逐条可点名：一次全量检查后 `apiFindings` 总 **39** = `apiEntryMissing` **12**（逐条 = `App.Frontend.Components.Ui` + 8 个视图目录根 + 3 个视图内子域）+ `apiMissing` **27**（13 crate + `App.Frontend.Shell` + 12 个前端单文件根 + `App.Backend.Shell`）+ 其余七条各 0 + `apiUndeclared` 0；壳 MUST NOT 同桶两占，两工具 crate 两桶皆零 | `.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/models/05-backend-tools.toml`（见 1、2）+ 单次全量检查实测留痕（落 `reports/`，见 9） |
| AC-4 | 壳的两类债各归其位：`kind == "apiEntryMissing" && element == "App.Backend.Shell"` 条目 **0**；同趟该元素 `apiMissing` **恰 1 条**（文案点名缺失的契约文件路径）；MUST NOT 读成「壳已清」 | `.dev-team/architecture/models/04-backend-shell.toml`（见 1） |
| AC-5 | 两工具 crate 零判据：`element ∈ {App.Backend.Tools.ExportBindings, App.Backend.Tools.ArchiApiGen}` 在 `apiFindings` 内**零条目**；结论依赖「`entry` 与 `api = "none"` 两条同时在场」，任一缺失即失败 | `.dev-team/architecture/models/05-backend-tools.toml`（见 2） |
| AC-6 | 生成物与索引零 diff：`pnpm run api:check` 零差异（`missing` / `mismatched` / `orphaned` 三列皆空）；`.dev-team/architecture/api/INDEX.toml` 与四类生成物 `git status --porcelain` 为空；三处 `generated = ""` 逐字不变 | `.dev-team/architecture/api/**`（design「不要修改」表登记的**零触碰面**，非 design 修改文件；见 10） |
| AC-7 | spec 六处换段且互不矛盾：四份 delta 内 `15 / 26` 命中只作已退场口径、`26 条` 零命中、`12 / 27` 在基线表与「基线单段」两处在案、`repo-archi-model` 内「显式 `entry` SHALL 为 0 处」与「归后续变更」零命中且「恰三处 / 恰三行」在案、`desktop-archi-api` 内「届时」零命中且 `Resolved` 30 / `Absent` 12 在案、`desktop-archi-model` 示例注释内旧句零命中；归档后同一批判据在 `openspec/specs/**` 逐条成立 | `openspec/changes/module-entry-data-backfill/specs/desktop-archi-api/spec.md`、`openspec/changes/module-entry-data-backfill/specs/desktop-archi-api-check/spec.md`（见 4、5）；`openspec/changes/module-entry-data-backfill/specs/repo-archi-model/spec.md`、`openspec/changes/module-entry-data-backfill/specs/desktop-archi-model/spec.md`（**已在案、非本次修改项**，见 6、7）；存活 spec 合并面见 11 |
| AC-8 | 版本纪律：`packages/desktop/package.json` `version` = **`0.6.47`**（相对 `HEAD` 现值 `0.6.46` 恰好 +1 patch，只加一次）；`tauri.conf.json` 仍为 `"version": "../package.json"` 引用形态、`git diff` 为空；`src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变 | `packages/desktop/package.json`、`packages/desktop/src-tauri/tauri.conf.json`（见 8）；`packages/desktop/src-tauri/Cargo.toml`（零改动对照面，见 8） |
| AC-9 | 不抬黄灯：本次全量检查 `status = clean`、门禁步 `passed = true`；纳管子集仍只有 `App.Backend.Infra.ArchiRuntime`（判据 0 条）；MUST NOT 因 39 条 backlog 把状态词常规化为 `warnings` | —（宿主内实测面，见 9；无被测文件） |

> **列口径**：本变更零测试载体，故「被测文件或模块」列按「被测数据 / 文本文件 + `## 不可测试项` 序号」填写，不虚填测试文件路径。
> **覆盖核对**：proposal AC-1 ~ AC-9 九条逐条在表内出现（9 行，编号独立、无合并、无遗漏）；design「修改文件」子表 6 项逐项落在本表 —— `04-backend-shell.toml`（AC-1 / 2 / 3 / 4）、`05-backend-tools.toml`（AC-1 / 2 / 3 / 5）、`specs/desktop-archi-api/spec.md`（AC-7）、`specs/desktop-archi-api-check/spec.md`（AC-7）、`package.json`（AC-8）、`tauri.conf.json`（AC-8）；无遗漏、无表外新增。
> **被测范围边界**：`crates/**`、`packages/desktop/src-tauri/src/**`、`packages/desktop/src/**`、`packages/desktop/src/types/generated/**`、`packages/desktop/src-tauri/Cargo.toml`、`AGENT.md` / `CLAUDE.md` / `archi-knowledge.md` / `decisions/0001-model-granularity.md`、`models/{01-app,02-backend-core,03-backend-infra,06-frontend}.toml`、`.dev-team/architecture/api/**`、`openspec/specs/**` 均为 design「不要修改」表与事实校准 7 界定的**零触碰面**，不纳入被测范围；其行为不变性由 AC-6 / AC-8 的零 diff 核对与既有套件全绿承接（基线见 3）。

---

## 单元测试

<!-- 零 per-file 映射对：design「修改文件」6 项均为 *.toml / *.md / *.json 数据、文本与配置资产，「公共函数 / API」「类型定义」两处整段省略（无待测功能来源）；
     proposal「测试文件」表与 design 事实校准 9 明令零新增 / 零改写测试；且数据与文本资产在既有布局下无同前缀测试文件形态。
     故本节不建任何 ### 源文件 -> 测试文件 章节，也不建 #### 待测功能 / #### 用例 / #### Mock策略 表；
     6 项判定与承接通道见 ## 不可测试项 1 ~ 8。
     跨模块「组合」形态在本变更不存在（零代码链路改动：三行数据 → 既有 entry_state → 既有 check 判据面，全部逐字未改），因此也不新建集成测试章节或组合测试区。 -->

本变更修改文件 6 项，**per-file 单元测试映射对 0 组**，故本节零章节。逐文件清点（`测试文件` 列恒为 `—`，即不可解析，理由指向 `## 不可测试项`）：

| 修改文件（design「变更清单」） | 解析到的测试文件 | 判定依据 |
|---|---|---|
| `.dev-team/architecture/models/04-backend-shell.toml` | — | 模型数据资产；无公共 API；proposal 明令零新增测试 —— 见 `## 不可测试项` 1 |
| `.dev-team/architecture/models/05-backend-tools.toml` | — | 同上 —— 见 2 |
| `openspec/changes/module-entry-data-backfill/specs/desktop-archi-api/spec.md` | — | spec delta 文本资产；无公共 API；判据面零改动 —— 见 4 |
| `openspec/changes/module-entry-data-backfill/specs/desktop-archi-api-check/spec.md` | — | 同上 —— 见 5 |
| `packages/desktop/package.json` | — | 版本号单值变更；无公共 API；版本纪律由 AC-8 diff 核对承接 —— 见 8 |
| `packages/desktop/src-tauri/tauri.conf.json` | — | **零内容改动**（引用形态确认项）；其相对路径解析行为由既有 `infra/checks` 的 `cargo_mutants_test.rs` 自有 fixture 覆盖，不读本仓真实文件 —— 见 8 |

> `entry` 字段与三态解析链的行为面（`entry_candidates` / `entry_state` / `within_path` / `duplicate_of` / `form_of` 及四条判据）已由变更 A 连测试交付（`crates/core/archi/src/api/paths_test.rs` / `check_test.rs` / `mod_test.rs`），本变更对它们**零改动**，因此不为它们重复设计用例——重复即违反「零改写测试」并制造第二套判定语料（`desktop-archi-api-check` 明写 MUST NOT）。

---

## 不可测试项

1. **`.dev-team/architecture/models/04-backend-shell.toml`（1 元素 / 13 关系，本次 +1 行 `entry`）** —— 判定：**无单元测试载体**。原因：① 纯数据资产，无函数 / 类型 / 入参（design 公共函数 / API 与类型定义两处整段省略）；② proposal「测试文件」表与 design 事实校准 9 明令零新增测试，且 `desktop-archi-api-check` 明写 MUST NOT 把本仓模型塞进 crate 语料（语料须自足可移植，否则 crate 单测随本仓内容演进变红）；③ 数据 / 文本资产无同前缀测试文件形态可映射。承接通道：AC-1（`grep -c '^entry = '` 合计 3、单值 / `./` 前缀 / 带扩展名 / 落位在 `path` 之内）、AC-2（计数 1 / 13 与表头同值、零诊断）、AC-3（壳转 ② ⇒ `apiMissing` 恰 1 条）、AC-4（壳的 `apiEntryMissing` 为 0）；核对以 `tasks.md` 阶段一 / 阶段二 / 阶段五自查与宿主内实测（见 9）执行，由桌面端 static-check / test-execution 落盘。

2. **`.dev-team/architecture/models/05-backend-tools.toml`（3 元素 / 3 关系，本次 +2 行 `entry`）** —— 判定：无单元测试载体（原因同 1）。承接通道：AC-1（该文件 `entry` 恰 2 行、两值逐字照 proposal §一且落在各自 crate 目录内、其余四份 0 行）、AC-2（计数 3 / 3 与表头同值、3 条关系与两行 `api = "none"` 逐字不动）、AC-3（两工具 crate 两桶皆零 ⇒ ① −2）、AC-5（零判据，依赖 `entry` 与 `api = "none"` 同批在场）；核对方式同 1。

3. **既有模型解析消费面回归锚：`packages/desktop/src-tauri/crates/core/archi/tests/pilot_test.rs`（零改写）** —— 判定：**不是本变更的 per-file 映射对，但是既有的间接回归锚**（事实修正见上文「零映射对的成因」4）。原因：该 crate 级集成测试以 `CARGO_MANIFEST_DIR` 上溯六层取真实仓库根（`:42` `repository_root()`），在 `:542` 以 `api_ref(&root, "App.Backend.Infra.ArchiRuntime")` 驱动 `api::mod::elements_of` → `dto::graph_view(root)`，要求整目录模型 `diagnostics` 为空；三行 `entry` 因此**可被该用例感知**（`entry = [...]` 之类形态错误会让模型解析出诊断 ⇒ 该用例红）。不改写理由：该用例对试点元素的断言（`by_fqn == projection`、三路径逐字相等）不因三行 `entry` 改变 —— `entry_state` 的候选表只由各元素 `path` 派生，三处新值与 `.../crates/infra/archi/src/lib.rs` 零交集（design 事实校准 3 同结论），且试点元素的 `entry` 保持 `None`（`api_ref` 投影不变）。承接通道：AC-2 的「零结构错」由该用例提供**无需新增代码**的既有守卫；核对以既有套件全绿执行（基线 `openspec/changes/module-entry-data-backfill/reports/baseline/summary.json`：`total 8974 / passed 8974 / failed 0 / conclusion pass`，2026-11-16）。

4. **`openspec/changes/module-entry-data-backfill/specs/desktop-archi-api/spec.md`（本次改 `## Purpose` 第 1、2 条）** —— 判定：**无单元测试载体**。原因：① spec delta 是规范性文本资产，不提供可调用单元，仓内也无任何测试以读取 spec 文件并断言其措辞为职责；② AC-7 的判据本身是 `grep` 形态的机械口径（「届时」零命中、`Resolved` 30 / `Absent` 12 在案），`design` D9 明确其成本是「两处措辞、零判据零数字改动」；③ 若为该面新增测试，等于把 spec 措辞耦进测试常量，与「基线数字不进断言」同一条纪律冲突。承接通道：AC-7 ⑤（`grep -rn '届时' desktop-archi-api/spec.md` 零命中 + 该 delta 的「本仓入口面实测命中」scenario 写 `Resolved` 30 / `Absent` 12；实测现状为「届时」**2 处命中**，正是本次 D9 待收敛项）；核对以 `tasks.md` 阶段三自查命令执行，由桌面端 static-check 承接。

5. **`openspec/changes/module-entry-data-backfill/specs/desktop-archi-api-check/spec.md`（本次在「基线表」条目补一句）** —— 判定：无单元测试载体（原因同 4）。承接通道：AC-7 ③（`grep -rn '12 / 27'` 在「基线表」与「基线单段」两处在案；实测现状为 4 处命中，其中「基线表」条目**尚缺**字面 `12 / 27`，正是 D9 待补项）、AC-7 ① / ②（`15 / 26` 命中只作已退场口径、`26 条` 零命中）；另 AC-3 / AC-9 的数字口径以该 delta 为事实源，但判据执行面在宿主实测（见 9）。核对方式同 4。

6. **`openspec/changes/module-entry-data-backfill/specs/repo-archi-model/spec.md`（已在案，非本次修改项）** —— 判定：无单元测试载体（原因同 4）；登记于此以保证 AC-7 的四份 delta 覆盖完整、不被误判为遗漏。实测现状：「恰三处」1 处命中、字段纪律 scenario 写「恰三行」、`归后续变更` 零命中 ⇒ 已满足 AC-7 ④。承接通道：AC-7 ④；核对以阶段三自查执行。

7. **`openspec/changes/module-entry-data-backfill/specs/desktop-archi-model/spec.md`（已在案，非本次修改项）** —— 判定：无单元测试载体（原因同 4）。实测现状：`本变更模型数据零处` 仅出现在 `## Purpose` 与 `## Module Contract` 的历史叙述行，需求正文示例 TOML 注释块内零命中 ⇒ 已满足 AC-7 ⑥。承接通道：AC-7 ⑥；核对方式同上。

8. **版本与配置面：`packages/desktop/package.json`、`packages/desktop/src-tauri/tauri.conf.json`、`packages/desktop/src-tauri/Cargo.toml`（后两者零改动）** —— 判定：**无单元测试载体，且不得以新增测试代替**。原因：① 判据是「`version` 单值 = `0.6.47`、只 +1 一次」「`tauri.conf.json` 仍是 `"version": "../package.json"` 引用形态且 diff 为空」「crate 版本行 `0.1.0` 逐字不变」，全部是文件内容事实，没有行为可测；② 仓内唯一涉及 `tauri.conf.json` 相对版本解析的用例是 `crates/infra/checks/src/mutation/cargo_mutants_test.rs:390`（`tauri_config覆盖_相对版本路径解析为字面值`），它以 `TempWs` 自造 `{"version": "../package.json"}` 副本、不读本仓真实文件 ⇒ 与本变更零耦合，无需随动；③ 为本变更新增「版本号 = X」测试会把版本纪律耦进套件（同 3、4 的理由）。承接通道：AC-8（`grep -n '"version"' packages/desktop/package.json` ⇒ `0.6.47`；`git diff --stat packages/desktop/src-tauri/{Cargo.toml,tauri.conf.json}` 零输出）；核对以 `tasks.md` 阶段四自查 + 桌面端 diff 留痕执行。另：`tauri.conf.json` 属 design 修改文件表中「**零内容改动**」的确认项，其 `git diff` 必须为空。

9. **宿主内实测面（AC-2 / AC-3 / AC-4 / AC-5 / AC-9 的执行载体）** —— 判定：**无单元测试载体，且必须由宿主进程与工作树 CLI 承接**。原因：① `apiFindings` 分桶（39 = 12 + 27）、`entry_state` 分布（`Resolved` 30 / `Absent` 12 / `Invalid` 0）、`status = clean` / `passed = true` 是**本仓数据跑出来的读数**，其判据面逐字未改（design D3），为它写测试等于测试既有判据而非本变更；② 门禁与 `archi_drift` 跑在**宿主进程内**（宿主须由本工作树重建，否则旧宿主对 `entry` 报 `unknownField` 并连带 `unknownReference`，design R-1 / D6）；③ `pnpm run api:check`（`cargo run -p archi-api-gen -- --check`）是工作树编译的 CLI 读盘比对。承接通道：AC-2 / AC-3 / AC-4 / AC-5 / AC-9 与 AC-7 的归档后存活 spec 判据；核对以 `tasks.md` 阶段五「移交说明」执行 —— 先 `archi_graph(root)` 确认能读出 `entry`（而非 `unknownField`）、`diagnostics` / `warnings` 空、51 元素 / 113 关系（不满足则 MUST 重建宿主后重跑，MUST NOT 删 `entry` 迁就宿主），再跑一次全量检查并对拍分桶，实测与 `grep` 计数结果落 `openspec/changes/module-entry-data-backfill/reports/`（按 `kind` 分桶的元素 FQN 清单 + 三段读数 `28 + 13` → `15 + 26` → `12 + 27` 的成因说明）。该面由桌面端 static-check / test-execution 与评估相位承接，本稿不复述运行脚本细节。

10. **`.dev-team/architecture/api/**` 零 diff 面（AC-6）** —— 判定：**无单元测试载体，且判据方向是「零产物」**。原因：① 该目录是 design「不要修改」表登记的零触碰面，本变更**不修改**其中任何文件 ⇒ 不存在「源文件 → 测试文件」映射；② 零生成物是结构必然而非「碰巧没跑生成」：`api/mod.rs` 的 `index_entries` 中 `status` 只看 `present`（契约文件在案）、`generated_rel = generated_path_of(...).filter(|_| present)`，三处元素均无契约 ⇒ `present = false` ⇒ `status` 恒 `absent`、`generated` 恒 `""`（design 事实校准 1）；③ 出现 diff 只可能是 `entry` 落位写错，处置是回退数据而非改生成器（design D4）。承接通道：AC-6（`pnpm run api:check` 三列皆空；`git status --porcelain .dev-team/architecture/api` 零输出；`INDEX.toml` 内三处 `generated = ""` 与 `[[entry]]` 42 条逐字不变；`*.i.rs` 全仓仍只 `crates/infra/archi/src/lib.i.rs` 一份、`*.i.ts` 零份、`*.schema.json` 仍一份）。既有测试面对该目录只做**路径派生与存在性**断言（`tests/pilot_test.rs:121` 的 `index_path(&root).is_file()`），不读其内容 ⇒ 不构成该面的测试载体。核对以 `tasks.md` 阶段二执行。

11. **`openspec/specs/**` 存活 spec 的换段合并（AC-7 后半句）** —— 判定：**无单元测试载体，且实现面 MUST NOT 手改**。原因：存活 spec 由归档链的 spec-sync 段按 delta 增量合并（`core/orchestration/src/prompt.rs` 的 `spec_sync_prompt`：只编辑 `openspec/specs/**`、不改 change 目录）；在实现期直接编辑会造成与归档合并重复并失去幂等（design 事实校准 7）。承接通道：AC-7 末句（四份存活 spec 内不得残留与 `12 / 27` 或 `30 / 12` 冲突的旧数）；核对归属归档链 spec-sync 段与评估相位，以 `grep` 形态留痕，不落测试代码。现状锚点：`openspec/specs/repo-archi-model/spec.md:30`「本变更落地时显式 `entry` SHALL 为 0 处」与 `openspec/specs/desktop-archi-api-check/spec.md:189`「本段基线 = 15 / 26」两处即待合并的旧口径。

> **不做项（明确不登记为不可测试缺口）**：① `desktop-archi-drift` / `desktop-archi-gate` / `desktop-archi-contract-panel` / `desktop-archi-agent` / `desktop-ipc-type-bindings` / `desktop-archi-file-tree` / `desktop-archi-canvas` 六能力在 proposal「不涉及」表中零改动，无被测对象；② 变更 B（12 个前端 barrel）与变更 C（TS 取数面）不在本变更范围，若 B 先落地则 AC-3 的 ① 期望改读 0、② 改读 39 并留痕说明成因（design D8 / R-6），属留痕口径而非新的测试设计；③ 结构化诊断、finding kind、配置键零新增（design D10 ⑦）⇒ 无新增断言面。
