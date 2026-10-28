# 提案: archi-check-and-gate

> **变更**: archi-check-and-gate
> **日期**: 2026-10-28
> **状态**: draft
> **来源**: `openspec/changes/archi-check-and-gate/explore.md`（requirement 类型；扫描范围 / 漂移分类 / 定级 / 模式 / 报告结构 / 门禁挂载 / 技术选型 / 产物阈值均已成文；`unresolved` 上限与真实工程规模为未决项）
> **范围接缝**: 模型格式 / 存储 / 结构校验 / 画布 / 文档关联 / agent 工具归 `archi-graph-doc-agent`（已归档落地）。本变更只**消费** `ModelGraph`：负责「扫哪些文件、怎么归属、什么算漂移、什么时候 fail、报告长什么样、依赖成本多少」，MUST NOT 重定义模型语义（`kind` / `gate` / `path` 的形状）。
> **前置依赖**: `crates/core/archi`（模型 / 图 / 结构诊断 / DTO）已在位；`ArchiveVcsPort::change_edit_set`（变更清单唯一入口）已在位；`ReportSlot` 二值槽位与 `change_reports` 派生已在位；`OnFail::FeedbackLoop` 的「超限升格相位 fail」语义已由 `step_static_check` 证实（无需新 `OnFail` 变体）。
> **模型同步（范围硬约束）**: 本变更新增 workspace crate（`crates/infra/archi`）⇒ 按 `repo-archi-model` 的漂移纪律（`:82`）与双向覆盖判据（`:81`），MUST 在同一 change 内追加元素与容器归属：基线 49→50 元素 / 107→111 关系（`03` 段 8 元素 / 24 关系、`04` 段 13 关系，新增 `App.Backend.Infra.ArchiRuntime` 指 `./packages/desktop/src-tauri/crates/infra/archi/`）。「本仓模型零改动」的原口径只覆盖 `gate` 档位收紧（仍全 `off`），不覆盖元素 / 关系 / 计数的同步。

---

## 问题

### 现象一：模型有了，判据仍是空的

上一变更把架构模型建成可解析、可检索、可上图的一等资产（49 元素 / 107 关系，`gate` 全 `off`），并在 spec 里明确把 `gate` 档位的**语义消费**（参与归属、违例定级、门禁结论）留给本变更。今天的结果是：一个标了 `gate = strict` 的元素**什么都不拦**——三处失配都无法用机器表达：

| 失配 | 现状 |
|---|---|
| 模型声明了依赖、代码里没有 | 只能靠人读模型猜 |
| 代码里有跨元素依赖、模型没声明 | 只能在 code review 时偶然撞见 |
| 元素 `path` 指向的目录早就不存在 | 只有画布上一个提示点，不拦任何流程 |

### 现象二：门禁点上没有架构这一关

唯一的刚性拦截形态是相位表的步 + `OnFail`。implement 相位在 executor 收口后已经拦了两道（static-check、verdict），但**架构债不参与任何一道**：编译过、测试绿、评审看 checklist——一条 `package → package` 的越界依赖可以一路走到归档。而架构问题一旦进了 acceptance，就只能开新 change 收尾（验收不负责重构）。

### 现象三：这件事的成本几乎全在两处，且都不可估

- **语言解析深度**：`import './foo'` 在 JS 里直接给出文件路径；`use crate::foo::bar` 在 Rust 里只给出模块路径，要靠模块树反查文件。两者不共享解析器，但必须共享归属规则。
- **依赖体积**：`ignore` / `globset` / `petgraph` / `syn` 都已在 `Cargo.lock` 里以传递依赖身份存在，可估；只有 `oxc_*`（TS/JS 语法树 + 模块解析）是**真新增**，特性开关默认会带进一串小包，不实测就只能猜。（`cargo_metadata` 同样在锁内，但本变更**不用**——它会跑 `cargo metadata` 子进程，与 `archi` 零 spawn 纪律冲突，见「范围裁决」。）

### 症结一：三条判定规则必须先写死，否则每处实现各判一次

1. **谁负责**：`A` 引入 `B` 且模型无 `A -> B` 时，级别由谁定？若取两端档位的严者，一个 `strict` 元素会把全仓库 `off` 元素拽进门禁——定级只能看引入方。
2. **什么算合法**：不存在「允许的依赖方向」的权威来源（kind 是包含层级，不是约束层级），唯一自洽规则是「声明了就合法」——所以「这条依赖是对的」的正确动作是补声明，不是加豁免。
3. **什么不算违例**：解析不到的 specifier、动态导入、宏生成的 `mod`、归属不确定的文件，一律不许冒充违例（看不见的依赖不该变成红点），只能留痕。

### 症结二：豁免机制一旦开 glob 粒度，ratchet 就白做

`src/legacy/**` 一类豁免等于永久后门：今天为「迁移期」开的口子，三年后还在。豁免只允许 `(元素, 元素)` 元素对 + 必填理由，且被抑制的条目必须留痕——「豁免了什么」本身要可审计。

### 症结三：报告是四个消费面的唯一形状

门禁诊断文本（注入 executor 的反馈）、审计面（`reports/archi/*.json`）、命令面（画布标记取数）、未来的 ratchet 决策，全部吃同一份报告。报告字段面一旦分裂，四方立刻各说各话；因此报告结构、`status` 五值、`occurrences` 聚合口径、`suppressed` 留痕都在本变更一次钉死。

### 症结四：成本必须实测并留可复核基线，否则回退没有触发条件

「要不要砍 oxc」若无法量化就永远变成凭感觉的争论。本期把安装包 / 主二进制 / 锁包数 / 编译用时 / 运行期 p95 五项阈值写死，并把回退序按代价递增固定（减特性 → 自算路径 → 正则 import），使超标时第一个动作是确定性的。

### 症结五：新 step 词是一次跨五能力的白名单扩展

`ArchiCheck` 作为第 22 个 step 词，会 ripple 到写面（`StepNode` 与 implement 步序列）、库（`RunStepKind` / `StepKind` 尾部追加）、查询（detail `stepKind` 出线）、前端（`normalizeKind` / `stepWord` / `STEP_LABEL` 穷尽表）、语料（全词汇 golden）。这不是「顺手加个枚举」，而是一条有 bincode 下标纪律的横切改动——不显式排进来就会在实现期炸开。

## 提案

新增一块**判定域**（`crates/core/archi` 的扫描 + 漂移 + 看护配置）与一个**薄执行体**（`crates/infra/archi`，只做「取输入 → 调判定 → 落盘 → 派生 outcome」），把执行挂进 implement 相位（`FeedbackLoop{3}`），报告落 `reports/archi/`，并经命令面出线给画布做标记。

```
                   crates/core/archi（纯读、零 spawn、零写盘）
                   ┌──────────────────────────────────────────────┐
  模型 * 源码  ──▶ │ scan/  遍历 + 排除 + 归属 + 依赖提取(TS|Rust) │
                   │   │                                          │
                   │   ▼                                          │
                   │ drift/ 对账 → 定级 → 豁免 → status 五值      │
                   │   │  = 报告结构（唯一形状）                  │
                   └───┼───────────────────────────┬──────────────┘
                       │                           │
        ┌──────────────▼───────────┐   ┌───────────▼──────────────┐
        │ infra/archi（执行体）     │   │ IPC archi_drift(root,mode)│
        │ 落 reports/archi/*.json  │   │  → 画布标记（取数侧纯函数）│
        │ → ArchiCheckOutcome      │   └──────────────────────────┘
        └──────────────┬───────────┘
                       ▼
   implement: Executor → StaticCheck{5} → ArchiCheck{3} → Evaluator → VerdictGate{5} → PhaseLog
```

### 1. 落位与依赖

- `crates/core/archi` 扩展四个模块族：`scan/`（遍历 / 排除 / TS·JS 提取 / Rust 提取 / 归属）、`drift/`（对账定级 / 报告结构 / 看护配置）。外部依赖新增 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn`，`globset` 提为直接依赖；`foundation` 仍是唯一 workspace 内依赖，**仍零 spawn / 零 Tauri / 零 store / 零 workflow / 零 config**。
- 新建 `crates/infra/archi`（裸名 `archi-runtime`）：实现 `ArchiCheckRunner`，依赖 `archi` + `orchestration`（port 类型）+ `foundation`（报告目录派生）。零 spawn、零 vcs、零 store、零 config。它存在的唯一理由是「报告要写盘」——`archi` 只读盘不写盘的纪律不动。
- 复用既有缝：变更清单走 `ArchiveVcsPort::change_edit_set(root, "HEAD")`（由 `steps.rs` 注入，与 mutation 同式）；报告目录走 `change_reports(root, name, ReportSlot::Archi)`；配置路径走新增的 `foundation::layout::archi_config_path`。
- 新 crate 落位 ⇒ 本仓模型同 change 追加元素与关系（见 §11）：`repo-archi-model` 的漂移纪律要求「新增 workspace crate 必须在引入它的同一 change 内补齐」，本变更不是例外。
- **明确不用**：`madge` / `dependency-cruiser` / `ts-morph`（要 node）、`cargo-modules` / `cargo-depgraph`（要 `cargo install` 二进制）、`cargo metadata` 子进程（要 `cargo`，且与「archi 零 spawn」冲突）。要「不依赖本地环境、开箱即用」。

### 2. 扫描：两套解析器，一套归属

- **排除面与测试 scope 正交**：默认排除测试 / 夹具（`*.test.*` / `*.spec.*` / `__tests__` / `*_test.rs` / `tests/` / `fixtures/` / `__mocks__`），可被架构域配置的 `exclude` 追加覆盖；MUST NOT 复用 `config.json` 的 `tests[].excludes`（改测试范围不能改架构判定面）。排除逐条留痕。
- **归属只一套**：元素 `path` **最长前缀匹配**，语言无关（目录 / crate 目录 / 单文件同一条规则）。同长多命中 → 归属不确定（记 warning + 计 unmatched），不任选；只被 `off` 元素覆盖的文件计 unmatched；`path` 缺失 → `noPath` warning 且不参与归属。
- **TS / JS**：静态 `import`（含 `import type`）+ `export … from`，经 `oxc_parser` 取 specifier 与行号、`oxc_resolver` 按**就近 tsconfig / jsconfig** 的 `baseUrl` + `paths` 与 node 规则落盘；解析失败 / 动态导入 / `require` 记 `unresolved`，不冒充违例。
- **Rust**：解析 workspace 与各 member 的 `Cargo.toml` 得 crate 清单（零子进程）→ 逐 crate 建模块树（`mod x;` / `mod x { … }` / `#[path]` 优先 / `cfg` 两臂同路径）→ `use` 树（`crate::` / `self::` / `super::` / workspace crate 名）做模块路径最长前缀匹配 → 文件 → 元素。宏生成 `mod` / `include!` / 孤儿文件记 `unresolved`；外部 crate 不进归属。**不做**类型检查、宏展开、完整 name resolution；`pub use` 只算声明，不穿透。

### 3. 三类漂移与定级

| # | 漂移 | 定义 | 产出 |
|---|---|---|---|
| ① | 声明未兑现 | 模型声明 `A -> B`，代码无该 import | `observations[]`（永不影响 `status` / `passed`） |
| ② | 未声明依赖 | 跨元素 import，模型无该关系 | 按**引入方** `gate` 档：`strict` → `errors`、`warn` → `warnings`、`off` → 不出现 |
| ④ | path 失效 | 元素 `path` 指向不存在路径 | 按**该元素** `gate` 档 + `model.warnings` |

层级逆向并入 ②（无权威方向来源，声明即合法）；`import type` 与普通 import 同权（`typeOnly` 只作标注）；目标元素未建模记 warning；模型结构错（`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`…）虽非漂移，门禁同样判 fail——读取照常返回结构化错误、绝不 panic。

### 4. 豁免两层，粒度封闭

第 1 层「声明即豁免」（零配置，补声明即消除）；第 2 层仅两形态：内联 `// archi-ignore-next-line: <reason>`（按 import 的 span 行号回原文取该行 / 上一行匹配，不读 AST 注释 trivia）与 `<archi_dir>/config.json` 的 `ignore[]` 元素对（`reason` 必填、FQN 必须存在于模型）。**禁 glob 粒度**。被抑制条目逐条进 `suppressed[]`（`via = config | inline` + reason + file/line），画布渲染成 `exempt` 边。

### 5. 全量 / 增量与观察根

| | 全量 | 增量 |
|---|---|---|
| 观察根 | 主仓根 | `detail.worktree`（归档 change 回落主仓根） |
| 基准 | 无 | `change_edit_set(root, "HEAD")`（唯一入口，与 mutation 同锚） |
| 产出 | ①②④ | ②④（① 只作观察项） |
| 落盘 | 否（可重算的观测，落盘只积垃圾） | 是（`reports/archi/<timestamp>.json`，门禁审计面） |

命令签名一律带 `root`，不引入「当前选中 change」隐式上下文；模型与源码必须**同根**读盘。

### 6. 报告结构与 `status` 五值

一份结构、两个消费面共用：`version` / `timestamp` / `mode` / `root` / `change` / `base` / `modelHash` / `model{...}` / `scan{...}` / `errors` / `warnings` / `suppressed` / `observations` / `status`。四条裁决：

- `status ∈ { clean, errors, warnings, noChanges, skipped }`，**`noChanges` 与 `skipped` 不是「通过」**——UI 与门禁日志必须显式区分，否则「没检查」会被读成「检查过了没问题」。
- `warnings`（有问题的依赖 / 目标 / 可解析性 / 归属）与 `observations`（设计态陈述）分界固定：后者永不进 `status`。
- `occurrences` 按 `(from, to)` 聚合，不按行铺开（反馈 prompt 有长度上限）。
- `suppressed` 必留痕；`modelHash` 供「这份报告是不是当前模型生成的」判断，本期零缓存，不当失效依据。

### 7. 架构域看护配置

`<archi_dir>/config.json`（经 `foundation::layout::archi_config_path` 派生），字段封闭为 `ignore[]` + `exclude[]`：缺失即空配置（零报错）、未知键拒绝、`reason` 必填、`ignore[].from/to` 必须是模型内存在的 FQN、`exclude[]` 为文件级 glob。**管辖面写死**：`openspec/config.json` 管执行（命令 / scope / 阈值 / 保护），本文件管看护策略（豁免 / 扫描排除），不允许出现第三个配置文件；工作区 `KNOWN_KEYS` 七项与 schema 镜像零变化。

### 8. 门禁挂载：只挂 implement，用 `FeedbackLoop` 不用 `Escalate`

```
implement: PhaseStart → Executor → StaticCheck{5} → ArchiCheck{3} → Evaluator → VerdictGate{5} → PhaseLog
```

- **只挂 implement**：acceptance 关注验收行为（架构债转新 change）；test-only 表无 implement 相位且测试文件被扫描全排除（挂上必空转）；initialize 只做基线绿门。
- **`FeedbackLoop{3}` 而非 `Escalate`**：② 的成因有两种（代码写了不该写的依赖 → 改代码；模型声明落后 → 该走 dev-design 回溯），无法自动区分；缺省权威是模型，所以第一反应是「让代码符合模型」——executor 拿到具体违例去改。而 `FeedbackLoop` 超限本来就会升格相位 fail 进决策分叉，`Escalate` 想要的「修不动就交决策」因此零新增变体即已具备。
- **`fault` 恒 `None`**：本步零进程 spawn，没有「命令挂死 / 文件锁」环境面；字段保留只为与既有三 outcome 同形，walker 分派不写特例。写盘失败降级 `report_ref = None`，不因审计面失败而中断检查。
- **`noChanges` / `skipped` 不冒充通过**：步 `passed` 但 detail 逐字写「未检查（无模型）」/「未检查（无可判文件）」。

### 9. 标记生产：报告驱动，映射在取数侧

`observations[]` 的 `unfulfilledDeclaration` → 边 `unfulfilled` + `from` 节点 `unfulfilled`（🟡）；`suppressed[]` → 边 `exempt`（reason 随抽屉）。映射是**取数侧纯函数**，只读报告、不重算判定；`marks.ts` 的样式契约与词汇零改动。两挂载点同源取数（`/architecture` 用 `full`、change 详情用 `incremental`）。

### 10. 依赖成本：阈值 + 执行方式 + 回退序

| 指标 | 软线 | 硬线 |
|---|---|---|
| NSIS 安装包 | +0.8 MB | **+1.5 MB（≤ 8.2 MB）** |
| 主二进制 | +3 MB | **+5 MB（≤ 31.6 MB）** |
| `Cargo.lock` 净新增包数 | ≤ 25 | **≤ 40** |
| CI 冷构建（单腿） | +45 s | **+90 s** |
| 本地增量（改 `archi` 一文件） | +8 s | **+15 s** |
| 单次检查（300 元素 / 1000 源文件） | 1.5 s | **3 s（p95）** |

四段式实测（基线 → 变体 → 差分 → 判读，全程同一台机器；不常态化跑 CI，共享 runner 抖动会淹没差值）。回退序按代价递增：① 收 `oxc_parser` 特性（只 parse，不引 semantic / codegen）→ ② 自算路径替代 `oxc_resolver`（约 150 行，`exports` / `imports` 退化）→ ③ 正则 import 替代 `oxc_parser`（type-only 与行号退化，且实质推翻「type-only 算架构依赖」的裁决，必须回到设计讨论）。实测表落 `crates/core/archi/tests/README.md` 并抄一份进 `design.md`。

### 11. 本仓模型同步（新 crate 的漂移纪律）

本变更新建 workspace crate，因此 `repo-archi-model` 的两条判据同时生效：双向覆盖（每个成员 crate 目录恰一个元素，`:81`）与漂移纪律（同一 change 内补齐，`:82`）。落地口径固定为**只追加、不收紧**：

| 项 | 现值 | 变更后 | 落点 |
|---|---|---|---|
| 元素总数 | 49 | **50** | 新增 `App.Backend.Infra.ArchiRuntime`（`kind = module`、`gate = off`、`path = ["./packages/desktop/src-tauri/crates/infra/archi/"]`、中文 `summary`） |
| `03-backend-infra.toml` | 7 元素 / 21 关系 | **8 元素 / 24 关系** | 元素 + 3 条 `[[relation]]`（→ `Core.Archi` 判定单源 / → `Core.Foundation` 报告目录派生 / → `Core.Orchestration` 步骤端口），全部 `[dependencies]` workspace 内声明 |
| `04-backend-shell.toml` | 1 元素 / 12 关系 | 1 元素 / **13 关系** | 新增 `Shell → Infra.ArchiRuntime`（装配注入 `LocalArchiCheck`） |
| 关系总数 | 107（后端 43 + 前端 64） | **111（后端 47 + 前端 64）** | 前端零新增：`lib/drift-marks.ts` / `hooks/use-archi-drift.ts` 的 `types/generated/bindings` import 落在既有 `Architecture → Shell` 边上；视图内 `lib/` / `hooks/` 不建元素（既有「粒度 ≠ 解析面」规则），故 `views/architecture/` 不因新增文件多出子域元素 |

五条纪律：

- **`gate` 档位全 `off` 不动**：本变更只让 `gate` 的语义可消费，收紧仍按模块独立变更（冷启动 ratchet）。原「本仓模型零改动」的表述仅在此意义上成立，MUST NOT 被用来豁免元素 / 关系 / 计数的同步。
- **计数三处同值**：模型文件表头注释、`repo-archi-model` 的逐文件计数表、`decisions/0001-model-granularity.md` 的 `§1` / `§4` / `§5` / `§6` / `§9` 复述必须同为 50 / 111、03 段 8 / 24、04 段 13——ADR 的 `§9` 是「可逐项复算」表，留旧值即违反该能力自身判据。
- **既有缺口不顺手补**：`crates/tools/specta-zod-patched`（vendored 上游 exporter）先于本变更进入 workspace 而未建元素，属既有漂移；本变更在核对时把它**单列为已知差集**，MUST NOT 借「补齐覆盖」扩到无关 crate（其补缺或豁免判据走独立变更）。
- **归档时同步 Purpose 复述**：`repo-archi-model` 主基线 `Purpose` 首段写有「全基线 49 元素 / 107 关系」，该章节不属 delta 可表达的范围，故归档 `SpecSync` 落主基线时 SHALL 一并对齐为 50 / 111（否则主基线的 Purpose 与本要求的计数表自相矛盾）。
- **零诊断零告警**：追加后 `archi_graph(root)` 仍须正常态（`diagnostics` / `warnings` 空）；`ArchiRuntime` 三条边全为 infra → core / 同层，不触 `dependsOnAncestor`，且 dev 边仍排除。

### 范围裁决（explore 决议的落地口径）

| explore 口径 | 落地裁决 | 理由 |
|---|---|---|
| `archi_reports_dir(root) → <archi_dir>/reports`（全局兜底） | **不建** | 本期唯一落盘面是 change 报告树（`reports/archi`）；全量模式不落盘 ⇒ 该函数零消费者，按「第二个消费者到场才下沉」纪律不预建 |
| Rust crate 清单经 `cargo_metadata` | **改为解析 `Cargo.toml` + member glob** | `cargo_metadata` 会跑 `cargo metadata` 子进程，违反 `archi` 零 spawn 纪律，且每次检查多付一次 cargo 启动 |
| tsconfig 按 `config.json` 的 `tests[].root` 逐子树加载 | **改为自被解析文件向上取最近 tsconfig / jsconfig** | 读工作区 `config.json` 违 `archi` 零工作区配置读取纪律；tsconfig 自带 `baseUrl` / `paths` 已足够 |
| 增量基准 = change 的 `base_commit` | **改为 `change_edit_set(root, "HEAD")`** | 与 mutation 同锚、同唯一入口；worktree 语义下 HEAD 差即本 change 编辑集，避免第二套取数口径 |
| >500 源文件转「显式按钮触发」三档策略 | **不在本期**（挂载即算） | 分档是 UI 策略且无法在取数前得知规模；本期以 p95 ≤ 3 s 硬线兜底，实测后再按需加档（属纯增量） |
| `ArchiCheckRunner::run(root, name, slot, cancel)` | **去掉 `slot` 形参** | 槽位恒 `Archi`，保留恒一值的形参只会诱导误用；报告槽位由执行体内部取用 |

## 能力

### 新增的能力

- **desktop-archi-drift** — 判定域：① 扫描范围与测试排除正交（排除留痕、配置追加覆盖）；② 文件归属（`path` 最长前缀 + 归属不确定不任选 + 两套解析器一套归属 + 无法判定只留痕）；③ 三类漂移与「定级只看引入方」（层级逆向并入 ②、type-only 同权、结构错同时拦）；④ 豁免两层与留痕（禁 glob、reason 必填）；⑤ 全量与增量两模式（观察根显式、变更集单点、增量 ⊆ 全量）；⑥ 报告结构与 `status` 五值（`noChanges` / `skipped` 不算通过、`observations` 与 `warnings` 分界、`occurrences` 聚合、`modelHash`）；⑦ 架构域看护配置（`<archi_dir>/config.json`，缺失即空、未知键拒绝、元素对粒度）；⑧ 判定单源（命令面与门禁面同源）；⑨ 运行期预算与语料（p95 ≤ 3 s、双份语料、回退序）。
- **desktop-archi-gate** — 门禁挂载：① 只挂 implement（acceptance / test-only / initialize 不挂、无运行时开关）；② 结论语义与反馈边（`passed` 只看 `errors` + 结构错、`skipped` / `noChanges` 显式非通过、反馈文本有界、不自动改代码）；③ runner port 与报告落盘（port 落 orchestration、实现落 `infra/archi`、零进程 `fault` 恒 `None`、写盘失败降级）；④ 输入口径与观察根（模型与源码同根、变更集注入失败即步失败、零读工作区配置）。

### 修改的能力

- **workspace-layout-resolution** — **MODIFIED**「架构域路径派生（archi_dir 族）」：新增 `archi_config_path(root) → <archi_dir>/config.json`（从 `archi_dir` 派生、文件名复用既有常量、唯一合法定位函数），并解除上一变更「本期不预建」的表述；新增两条 scenario（配置路径单点 / 与工作区配置路径互不遮蔽）。
- **desktop-checks-domain** — **MODIFIED**「baseline 报告槽位参数化」：槽位由封闭二值扩为封闭三值 `test` / `baseline` / `archi`；checks 两 runner MUST NOT 接受 `archi`（传入即 `Err`）；三变体同驻 foundation 单点派生；`archi` 槽位零 exec-outputs。
- **desktop-crate-layout** — **ADDED**「infra/archi crate 落地登记与 archi 依赖扩展」：`archi-runtime` 落位与 `archi` + `orchestration` + `foundation` 依赖面、零 spawn / 零 vcs / 零 store / 零 config、`desktop-app → archi-runtime` 入边、`archi` 的 `oxc_*` / `syn` 依赖扩展与「workflow ⇄ archi 互不依赖」复核、七类边界不增类、锁包数 ≤ 40 护栏。
- **desktop-workflow-write-face** — **MODIFIED**「相位定义有序 step 序列与 artifact 清单」：`StepNode` 21 → 22 值（`archi_check` 尾部追加、归 `Exec`）；implement 相位声明 7 步序列（`ArchiCheck{3}` 位于 static-check 与 evaluator 之间）；test-only 表零 `ArchiCheck`；新增两条 scenario（22 值分组 / 架构门禁步只在 implement）。
- **desktop-change-orchestration** — **ADDED**「ArchiCheck 步接线与反馈边」：`ToolCommand` / `ToolStepOutput` 尾部追加、`ArchiCheckOutcome` 字段面、`ArchiCheckRunner` port（`run(root, name, changed, cancel)`）、`steps.rs` 分派与审计行、walker `step_archi_check`（fault 先行 / 计数 ≤3 / 超限代写 fail `phase_log` 升格 / 续注同一 executor 会话 / 反馈文本有界）、`ChangeStepKind → RunStepKind` 映射臂。
- **desktop-change-state-store** — **MODIFIED**「run 执行图落库」：`RunNodeRecord` 步词汇 21 → 22（`archi_check` 尾部追加，顺带补正原文「二十类」漏列 `clean` 的笔误）；明确「新增步词一律尾部追加」为唯一合法扩展形态。
- **desktop-change-queries** — **MODIFIED**「change 详情聚合」：detail `stepKind` 21 → 22 值出线（`archi_check` 在案），字段演进仍走 golden 显式重写。
- **desktop-change-flow-view** — **MODIFIED**「运行态节点与实时执行视图」：步词汇归一 21 → 22 值（补 `archi_check` ↔ `archiCheck`、归 exec / toolStep 分类）；**MODIFIED**「initialize 相位列与 clean 步词呈现」：`STEP_LABEL` / `normalizeKind` / `stepWord` 补 `archiCheck` 键，穷尽表 22 值。
- **desktop-corpus-regression** — **MODIFIED**「run 运行史语料维度」：节点全词汇样本扩为 22 类（补 `archi_check` 节点），聚合 golden 随动。
- **desktop-archi-canvas** — **MODIFIED**「漂移标记呈现契约（只认标记，不认判定）」：解除「本期标记生产者为空」，改为报告驱动——`observations[].unfulfilledDeclaration` → 边 / 节点 `unfulfilled`，`suppressed[]` → 边 `exempt`（reason 随抽屉）；映射为取数侧纯函数，`marks.ts` 样式契约零改动；两挂载点同源取数（`full` / `incremental`）。
- **repo-archi-model** — **MODIFIED**「基线落位与模型文件划分」「后端下钻到 crate 层的元素覆盖」「后端关系只取 Cargo 依赖声明」「图自洽与停摆禁令」「模型与裁定记录的可检索性」「覆盖判据的机械核对与漂移纪律」：新增 workspace crate `crates/infra/archi` 在同 change 内追加元素 `App.Backend.Infra.ArchiRuntime`（`module` / `gate = off` / `path = ./packages/desktop/src-tauri/crates/infra/archi/`）并归属既有 `App.Backend.Infra` 容器，`03` 段 7→8 元素 / 21→24 关系、`04` 段 12→13 关系，全基线 49→50 元素 / 107→111 关系（后端 43→47、前端 64 不变）；「计数复述单源」新增为显式约束（模型表头 / spec 计数表 / ADR `§9` 三处同值）；`crates/tools/specta-zod-patched` 的既有元素缺口单列为已知差集（本变更 MUST NOT 顺手补齐）；`gate` 档位全 `off` 不动。

### 零改动能力与理由

- **desktop-archi-model**：模型格式 / 字段面 / 诊断分类 / DTO 词汇全不触碰；`gate` 的语义消费本就由该 spec 指定归属本变更（「档位的语义消费归 archi-check-and-gate」），本变更**消费**其 `ModelGraph`，不新增模型字段、不改解析。本仓模型数据的追加（元素 / 关系 / 计数）归 `repo-archi-model`（见上），不在本能力内扩字段。
- **desktop-archi-agent**：不新增 agent 工具（本变更的 `archi_drift` 是命令面；是否给 agent 一份漂移只读工具不在本期），注入矩阵与 `SessionToolset` 零变化。
- **desktop-workspace-config**：架构域配置不是工作区配置——`KNOWN_KEYS` 七项、`config.schema.json` 镜像、前端 config 页全零变化；架构域配置读取面落 `archi`（不经 `config` crate）。
- **desktop-app-shell**：`commands/archi/` 是既有命令轨道的又一成员（轨道枚举非穷尽且非规范性）；命令契约由 `desktop-archi-drift` 承载。
- **desktop-ipc-type-bindings**：「生成物入库与一致性守卫」已覆盖任意新命令与 DTO（`bindings:check` 零 diff 即既有门禁），新增 `archi_drift` 与报告 DTO 不需要新条款。
- **desktop-file-watch**：显式取数纪律零变化（漂移取数走挂载 / 根切换 / 手动刷新三触发，零 watch）。
- **desktop-change-archive**：`reports/archi` 属 change 目录内工件，随归档一并进主仓；归档七段链、冲突分流、Cleanup 语义零改动。
- **desktop-phase-prompts**：16 份角色 md 零改动——门禁失败诊断经反馈边注入既有 executor 会话（组装面零扩张），不需要给 prompt 加工具用法或架构说明。
- **desktop-write-protection**：架构域路径本就不在内置锁面；本期新增的写面只有报告文件（非源码 / 非模型），不构成锁面变更。
- **desktop-test-execution / desktop-mutation-testing**：两条执行链、阈值、复用门、升格语义零触碰；本变更只新增一个不 spawn 的门禁步。
- **desktop-build-modes / desktop-release-pipeline**：构建与发版链零改动（依赖体积变化由 `Cargo.lock` 自然承载，不新增 CI 阶段；成本实测不常态化跑 CI）。
- **repo-surface-scope**：两个新增 capability 各对应可指认的存活实现（`crates/core/archi` 的 `scan/` + `drift/`、`crates/infra/archi`、implement 相位步）——纪律被遵守而非被改动；本变更不退役任何资产，活跃面不新增指向已删资产的路径引用。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/archi/Cargo.toml` | 依赖扩展：`oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn`（+ `globset` 提为直接）；仍零 Tauri / 零 store / 零 spawn / 零 workflow / 零 config |
| `crates/core/archi/src/lib.rs` | 模块装配与出口（`pub mod scan; pub mod drift;`） |
| `crates/core/archi/src/scan/mod.rs` | 扫描编排：遍历 → 排除 → 归属 → 依赖收集 → `ScanOutcome` |
| `crates/core/archi/src/scan/exclude.rs` | 默认排除约定常量 + 配置 `exclude` 追加匹配（globset）；产出 `excluded[{ file, rule }]` |
| `crates/core/archi/src/scan/ts.rs` | TS / JS import 提取（specifier / `typeOnly` / 行号）+ tsconfig / jsconfig 解析（就近向上） |
| `crates/core/archi/src/scan/rust.rs` | crate 清单（`Cargo.toml` + member glob，零子进程）+ 模块树 + `use` 树最长前缀匹配 |
| `crates/core/archi/src/scan/owner.rs` | 文件 → 元素最长前缀归属单点（含同长多命中 / `off` 覆盖 / `noPath` 处理） |
| `crates/core/archi/src/drift/mod.rs` | 判定单点 `check(CheckInput) -> Report`：对账 / 定级 / 豁免 / status 推导 |
| `crates/core/archi/src/drift/report.rs` | 报告结构体 + `serde_json` 序列化 + `status` 五值封闭枚举 |
| `crates/core/archi/src/drift/config.rs` | `<archi_dir>/config.json` 读取（经 `archi_config_path`）：缺失即空、`deny_unknown_fields`、reason 与 FQN 校验 |
| `crates/infra/archi/Cargo.toml`（新） | crate 声明：`archi` + `orchestration` + `foundation` + `serde_json`；零 Tauri / 零 spawn / 零 vcs / 零 store / 零 config |
| `crates/infra/archi/src/lib.rs`（新） | 出口（`LocalArchiCheck`） |
| `crates/infra/archi/src/runner.rs`（新） | `impl ArchiCheckRunner`：调判定 → 落盘 → 派生 outcome（`fault` 除取消外恒 `None`；写盘失败降级） |
| `crates/infra/archi/src/report.rs`（新） | 报告文件名与目录创建单点（经 `foundation::layout` 派生，零字面量） |
| `crates/core/foundation/src/layout/mod.rs` | 新增 `ARCHI_REPORTS_DIR_NAME` 常量、`ReportSlot::Archi` 分臂、`archi_config_path(root)`（均从 `archi_dir` 派生） |
| `crates/core/workflow/src/state.rs` | `StepNode` / `RunStepKind` / `ChangeStepKind` 尾部追加 `ArchiCheck`（+ `as_str` / `group` / `role` 臂）；审计 `StepKind` 八臂 → 九臂 |
| `crates/core/workflow/src/write/phase_table.rs` | `IMPLEMENT_STEPS` 7 步（插入 `ArchiCheck{3}`）；常量 `ARCHI_FEEDBACK_LIMIT: u32 = 3` |
| `crates/core/orchestration/src/port.rs` | `ToolCommand::ArchiCheck{ change_id }`、`ToolStepOutput::ArchiCheck`、`ArchiCheckOutcome`、`ArchiCheckRunner` port |
| `crates/core/orchestration/src/steps.rs` | `LocalToolSteps` 第四 runner 字段与构造入参；`ArchiCheck` 分派（name 解析 → `change_edit_set` → runner → 审计行） |
| `crates/core/orchestration/src/walker.rs` | `step_archi_check`（fault 先行 / 反馈边 ≤3 / 超限代写 fail `phase_log` + `StepFlow::Upgraded` / `continue_session` 续注）；步分派臂 |
| `crates/core/orchestration/src/run_history.rs` / `control.rs` / `lib.rs` | `mapped_step` 臂 / `is_session_step` 判假 / 出口 re-export |
| `src-tauri/src/commands/archi/mod.rs` | `archi_drift(root, mode)` + `archi_drift_inner`（增量模式经 vcs 缝取变更集后调判定纯函数；命令层零判定） |
| `src-tauri/src/commands/change_flow/mod.rs` | 装配注入 `Arc::new(LocalArchiCheck::new())` |
| `src-tauri/src/commands/mod.rs` | `all_commands!` 增 `archi_drift` 一条 |
| `src-tauri/Cargo.toml` | `members` 增 `crates/infra/archi`；`[workspace.dependencies]` 增 `archi-runtime` 与 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn`；壳依赖增 `archi-runtime`（带用途注释） |
| `packages/desktop/src/views/architecture/lib/drift-marks.ts`（新） | 报告 → 标记纯函数（只读 `observations` / `suppressed`） |
| `packages/desktop/src/views/architecture/hooks/use-archi-drift.ts`（新） | 显式取数（`archi_drift(root, mode)`，三触发、零 watch） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 接线：`graph = driftMarks(report, graph)`；取数失败降级零标记 + 行内提示 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | `normalizeKind` / `stepWord` 补 `archi_check` ↔ `archiCheck` |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `STEP_LABEL` 补 `archiCheck: '架构检查'` |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 再生成（`archi_drift` + 报告 DTO + `archiCheck` 步词） |
| `packages/desktop/package.json` | `version` 0.6.4 → 0.6.5（`tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 版本不随动） |
| `.dev-team/architecture/models/03-backend-infra.toml` | 表头注释 7 元素 / 21 关系 → **8 元素 / 24 关系**；新增 `[element."App.Backend.Infra.ArchiRuntime"]`（`kind = "module"`、`gate = "off"`、`path = ["./packages/desktop/src-tauri/crates/infra/archi/"]`、中文 `summary`），归属既有 `App.Backend.Infra` 容器；新增 3 条 `[[relation]]`（→ `Core.Archi` / `Core.Foundation` / `Core.Orchestration`，逐条中文 `desc`），其余 21 条边零改动 |
| `.dev-team/architecture/models/04-backend-shell.toml` | 表头注释 1 元素 / 12 关系 → 1 元素 / **13 关系**；新增 1 条 `[[relation]]`（`App.Backend.Shell → App.Backend.Infra.ArchiRuntime`，`desc` 写 runner 构造与注入），其余 12 条零改动 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 计数复述同步（`§1` 基线、`§4` 后端边总数 43→47、`§5` `Shell` 出边 12→13、`§6` 元素总数 49→50、`§9` 复算表 50 / 111 与后端 47）；三档粒度 / 容器 `path` / dev 排除 / ui 收敛 / `gate` 全 `off` / 封闭排除面六条判据零改动 |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 判定语料：未声明依赖三档（strict / warn / off）/ 聚合 occurrences / 目标未建模 / 测试文件排除 / `import type` / 内联豁免 / 配置豁免 / 解析失败 / 声明未兑现（增量双模式）/ 结构错 / 空模型 / 无变更 / 全量与增量一致性 | `crates/core/archi/tests/corpus/<case>/` + `expected/<case>.json` + `tests/golden_test.rs` | 新增（golden 沿 `DESKTOP_GOLDEN_REWRITE=1` 显式重写） |
| Rust 模块树语料：`mod foo;` / `foo/mod.rs` / `#[path]` / `cfg` 两臂 / 内联模块 / `use crate::` 最长前缀 / `super::` / workspace crate / 外部 crate / 缺 crate / 宏生成 / 孤儿文件 | `crates/core/archi/tests/corpus/rust/<case>/` | 新增 |
| 归属单点：最长前缀 / 同长多命中 / `off` 覆盖 / `path` 缺失 / 文件级 `path` | `crates/core/archi/src/scan/owner_test.rs` | 新增（直接造模型，不经解析） |
| 排除矩阵：七条默认约定逐条命中 / 配置 `exclude` 追加（不替换）/ `excluded` 留痕逐条 | `crates/core/archi/src/scan/exclude_test.rs` | 新增 |
| 看护配置：缺失即空 / 未知键拒绝 / reason 缺失 / FQN 不存在 / `exclude` 生效 / glob 形态 `ignore` 拒绝 | `crates/core/archi/src/drift/config_test.rs` | 新增（tempdir） |
| 报告与 status：五值逐值 / `observations` 不影响结论 / `occurrences` 聚合 / `suppressed` 两来源 / `modelHash` 随模型变化 / JSON 往返 | `crates/core/archi/src/drift/report_test.rs` | 新增 |
| 运行期预算：1000 源文件 + 300 元素，release，10 次取 p95 ≤ 3 s | `crates/core/archi/tests/perf.rs` + `tests/README.md`（实测表） | 新增 |
| layout：`archi_config_path` 路径与同源 / 与 `config_path` 互不遮蔽 / `ReportSlot::Archi` 派生 `reports/archi` / 三槽位互异 | `crates/core/foundation/src/layout/mod_test.rs` | 扩展 |
| 词面：`StepNode` / `RunStepKind` / `ChangeStepKind` / `StepKind` 22 值（审计 9 臂）全映射 / `archi_check` 单双向词面 / `group()` / `role()` / 尾部追加顺序 | `crates/core/workflow/src/state_test.rs` | 扩展 |
| 相位表：implement 7 步序与 `on_fail` / `ArchiCheck` 只在 implement / test-only 表零 `ArchiCheck` / 回溯白名单不回归 | `crates/core/workflow/src/write/phase_table_test.rs` | 扩展 |
| walker 反馈边：违例 → 续注同一 executor 会话（`EdgeKind::Feedback` + `instance_no` 递增）/ 3 次超限升格（代写 fail `phase_log`、`StepFlow::Upgraded`）/ 通过进 evaluator / `noChanges` / `skipped` 步 passed 且 detail 非「通过」/ 反馈文本含处置提示 / 变更集取数失败即步失败 / 取消 → `fault=Stopped` | `crates/core/orchestration/src/walker_test.rs` | 扩展 |
| 分派与审计：`ArchiCheck` 命令 → runner 调用与 `StepKind::ArchiCheck` 审计行 / 未建档 `Err` / 产出类型漂移 `Err` | `crates/core/orchestration/src/steps_test.rs` | 扩展 |
| port 契约：`ArchiCheckOutcome` 字段面 / `ToolStepOutput` 变体匹配 | `crates/core/orchestration/src/port_test.rs` | 扩展 |
| runner 执行体：报告落盘与 `report_ref` / 写盘失败降级 `None` 而结论保留 / 零进程（源码扫描）/ 报告目录经 layout 派生 | `crates/infra/archi/src/runner_test.rs`（新） | 新增（tempdir） |
| 命令面：`archi_drift` 全量 / 增量两模式 / 增量经 vcs 缝取变更集 / 无模型 `skipped` / 坏模型结构化错误不 panic / 与 runner 同源逐字段相等 | `src-tauri/src/commands/archi/mod_test.rs` | 扩展（tempdir，假 vcs port） |
| 报告 → 标记映射：`unfulfilledDeclaration` → 边 + 节点 / `suppressed` → `exempt`（reason）/ 零报告零标记 / 未命中项保持 `none` / 不重算判定 | `packages/desktop/src/views/architecture/lib/drift-marks.test.ts`（新） | 新增（纯函数直调） |
| 漂移取数 hook：挂载 / 根切换 / 手动刷新三触发 / `mode` 透传 / 失败降级 | `packages/desktop/src/views/architecture/hooks/use-archi-drift.test.ts`（新） | 新增（照抄 `use-archi-graph.test.ts`） |
| 画布接线：有报告时标记上边 / 节点 / 取数失败仍渲染零标记图 | `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx` | 扩展（补 drift 命令 mock） |
| 步词汇：全词汇归一表扩 `archi_check` / `stepFromRecord` 分组 `toolStep` / `STEP_LABEL` 穷尽 | `packages/desktop/src/views/changes/flow/run-state.test.ts` | 扩展 |
| 语料与 golden：全词汇样本补 `archi_check` 节点 / detail 投影 / 快照 | `crates/core/workflow/tests/corpus_golden_test.rs` | 扩展（显式重写） |
| 模型同步核对：`crates/{core,infra,tools}` 目录 ↔ 元素 `path` 双向差集（已知缺口单列）/ `03` 段 8 元素 24 关系 / `04` 段 13 关系 / 模型表头与 ADR `§9` 计数同值 / `archi_graph` 零诊断零告警 / `warn`·`strict` 零命中 | 命令 + 留痕（结果落 `openspec/changes/archi-check-and-gate/reports/archi/`） | 新增（按 `repo-archi-model`「核对以命令 + 留痕执行」，MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**`） |

### 不要修改

- `openspec/specs/**` 主基线（落主基线只在归档 `SpecSync` 步）。
- `.dev-team/architecture/models/**` 的 `gate` 档位：全 `off` 不动——收紧按模块独立变更（一个 change 收紧一块）；本变更只追加元素 / 关系 / 计数（见 §11），MUST NOT 出现 `warn` / `strict`。
- `.dev-team/architecture/models/{01-app,02-backend-core,05-backend-tools,06-frontend}.toml`：四文件零改动（`01` 容器、`02` core、`05` tools、`06` 前端均不因本变更增减元素或边；前端新增文件的依赖落在既有 `Architecture → Shell` 边上，视图内 `lib/` 不建元素）。
- `.dev-team/architecture/config.json`：架构域看护配置**不作为交付物创建**（缺失即空配置；示例配置只在语料与用例里出现）。
- `openspec/config.json`、`crates/core/config/**`、`crates/core/config/config.schema.json`、前端 config 页：工作区配置零变化（架构域配置是另一个文件，且其读取面落 `archi`）。
- `crates/core/workflow/src/prompts/**`（16 份角色 md）：零改动（门禁反馈走既有反馈边，不给 prompt 加架构说明）。
- `crates/infra/agent/**`（工具面 / 注入矩阵 / policy）：不新增 agent 工具、不改 `SessionToolset`。
- `crates/core/archi/src/{model,parse,graph,docs,search,diff,edit,dto}.rs`：模型解析、图构建、结构诊断、文档关联、检索、diff、gate 写回、三面投影零改动（本变更只在同 crate 新增 `scan/` 与 `drift/`，并复用既有 `dto` 的 `EdgeMark` / `NodeMark` 词汇）。
- `crates/infra/checks/**`、`crates/infra/vcs/**`、`crates/infra/store/**`：检查域 runner、git 能力、库表结构零改动（变更清单与报告槽位复用既有单点）。
- `src/views/changes/flow/**` 除 `run-state.ts` / `run-step-node.tsx` 两处词面改动与其测试；`src/components/ui/**`；`src/views/explores/**`。
- `.github/workflows/**`（成本实测不常态化跑 CI）、`src-tauri/tauri.conf.json`（版本经 `../package.json` 跟随）、`src-tauri/capabilities/default.json`（权限面零扩张）。
- `openspec/changes/archi-check-and-gate/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 扫描范围与测试 scope 正交：测试文件跨元素 import 零违例且 `scan.excluded[]` 留痕；改 `tests[].excludes` 后架构扫描范围逐条不变；配置 `exclude` 为追加而非替换 | 判定语料 `exclude` case + `scan/exclude_test` |
| AC-2 | 归属单点：最长前缀命中；同长多命中记 `ambiguousOwner` 且计 unmatched；只被 `off` 元素覆盖的文件计 unmatched；`path` 缺失记 `noPath` | `scan/owner_test` 四条用例 |
| AC-3 | 两套解析器一套归属：TS 别名经 tsconfig `paths` 解析；Rust 四种模块树形态（`mod foo;` / `foo/mod.rs` / `#[path]` / `cfg` 两臂）均可反查文件；外部 crate 不进归属；宏生成 / `include!` / 孤儿文件 / 动态导入逐条进 `unresolved` | Rust 模块树语料 + 判定语料 |
| AC-4 | 定级只看引入方：同一 `A 引入 B` 在 `strict` / `warn` / `off` 三档分别产出 error / warning / 零记录；目标未建模记 warning；`import type` 同权（`typeOnly: true`） | 判定语料三档 + type-only case |
| AC-5 | `occurrences` 按元素对聚合：3 文件各 1 处 → 1 条记录 + 3 项 occurrences（不铺开 3 条） | 判定语料聚合 case |
| AC-6 | 豁免两层留痕且粒度封闭：内联注释与配置元素对各不报并进 `suppressed[]`（带 reason / file / line）；`reason` 缺失、FQN 不存在、glob 形态 `ignore` 三者均显式 `Err`；补声明后该对转为正常声明 | `drift/config_test` + 豁免语料 |
| AC-7 | 两模式一致且落盘面分离：同一 commit 下增量报告为全量报告在变更集上的子集（同元素对级别相同）；全量不落盘、增量落 `reports/archi/<timestamp>.json`；无变更文件 → `noChanges` | 判定语料一致性 case + runner 用例 |
| AC-8 | `status` 五值可辨且 `skipped` / `noChanges` 不被当通过：五次构造分别出 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；`observations` 不影响 `status`；`modelHash` 随模型变化 | `drift/report_test` 五值用例 |
| AC-9 | 看护配置面写死：`<archi_dir>/config.json` 缺失即空配置；未知键拒绝；工作区 `KNOWN_KEYS` / schema 镜像 / 前端 config 页零变化；产品 `.rs`（`*_test.rs` 除）无 `config.json` 字面量拼接 | `drift/config_test` + 既有命名隔离扫描 |
| AC-10 | 判定单源：`archi_drift` 命令与门禁 runner 对同一输入产出的报告除 `timestamp` / `change` 字段与落盘路径外逐字段相等；两处源码无归属 / 定级 / status 第二份实现 | 命令面同源用例 + 源码审查 |
| AC-11 | 门禁只挂 implement：implement 步序为 7 步且 `ArchiCheck{3}` 位于 static-check 与 evaluator 之间；其余相位与 test-only 全表零 `ArchiCheck`；无运行时开关（零配置键 / 零环境变量 / 零相位名判断） | `phase_table_test` + walker 用例 + 源码审查 |
| AC-12 | 反馈边语义：违例注入同一 executor 会话（`EdgeKind::Feedback` + `instance_no` 递增）；3 次超限升格相位 fail（代写 fail `phase_log`、不跑 evaluator）；通过进 evaluator；三段计数互不占用；反馈文本含固定处置提示（回到 dev-design 补声明）且 16 份 prompt md 字节零变化 | `walker_test` 五条用例 |
| AC-13 | `skipped` / `noChanges` 不冒充通过：步 `passed` 且 detail 逐字为「未检查（无模型）」/「未检查（无可判文件）」；报告 `status` 原样保留 | walker + 报告两侧用例 |
| AC-14 | runner 落盘与降级：报告落 `reports/archi/` 且 `report_ref` 回填；目录不可写时结论与诊断保留、`report_ref = None`、步不失败；`reports/test` / `reports/baseline` 字节零变化 | `infra/archi/runner_test` + 命令面用例 |
| AC-15 | 输入口径：模型与源码同根（worktree 场景主仓零读取）；`change_edit_set` 失败即步失败（不降级空清单）；零读工作区配置 | walker 用例 + 源码审查 |
| AC-16 | 步词汇 22 值全链路：`StepNode` / `RunStepKind` / `ChangeStepKind` / 审计 `StepKind` 同词面 1:1；`archi_check` 尾部追加；detail 出线在案；前端 `normalizeKind` / `stepWord` / `STEP_LABEL` / 分组穷尽（编译期穷尽表兜底）；语料 golden 覆盖样本在案 | `state_test` + `phase_table_test` + `run-state.test.ts` + corpus golden |
| AC-17 | 依赖与分层：`archi` 的 workspace 内依赖仍仅 `foundation`、零 spawn / 零 Tauri / 零 store / 零 workflow / 零 config；`archi-runtime` 依赖恰为 `archi` + `orchestration` + `foundation` 且零 spawn；`workflow` 零 `archi` 依赖；`desktop-app → archi-runtime` 有用途注释 | `Cargo.toml` 审查 + 既有依赖方向断言 |
| AC-18 | 成本与预算：`Cargo.lock` 净增 ≤ 40 包；`archi_check` p95 ≤ 3 s（1000 文件 / 300 元素 release）；实测表（五项）落 `crates/core/archi/tests/README.md` 并在 `design.md` 抄录；越硬线时回退序执行留痕 | `perf.rs` + 实测表审查 |
| AC-19 | 标记生产与呈现：`unfulfilledDeclaration` → 边 + 节点 `unfulfilled`、`suppressed` → `exempt`（抽屉 reason）、其余保持 `none`；报告 `clean` / `skipped` / `noChanges` 或取数失败 → 全 `none` 且图照常渲染；`marks.ts` 四值样式契约零改动 | `drift-marks.test.ts` + `architecture-canvas.test.tsx` |
| AC-20 | 交付面：`bindings:check` 重导出后零 diff；`packages/desktop` `version` 0.6.5、`src-tauri/Cargo.toml` 版本不随动；`vp check` / `knip` / `cargo clippy -D warnings` 通过 | 机械门面 |
| AC-21 | 模型与新增 crate 同 change 同步：`crates/infra/archi` 目录 ↔ `App.Backend.Infra.ArchiRuntime`（`kind = module`、`gate = off`、`path` 逐字、`summary` 非空且不复述 FQN）；`03` 段 8 元素 / 24 关系、`04` 段 13 关系、全基线 50 元素 / 111 关系（后端 47 + 前端 64）；15 个自研 crate 目录 ↔ 15 个 crate 元素零差集（`crates/tools/specta-zod-patched` 单列为既有已知缺口，其余零差集）；模型表头 / spec 计数表 / ADR `§9` 三处同值；`archi_graph(root)` 零诊断零 `warnings`、`warn` / `strict` 零命中；`App.Backend.Shell → App.Backend.Infra.ArchiRuntime` 与三条 `ArchiRuntime` 出边在案 | 双向覆盖比对命令 + `archi_graph(root)` + `archi_module("App.Backend.Infra.ArchiRuntime")` + `grep` 计数 + 核对留痕（`reports/archi/`） |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **`oxc_*` 依赖成本超预算**：这是本变更唯一不可估的成本项（特性开关会带进一串小包） | 安装包 / 二进制增大吃掉发版与本地增量反馈时长；超硬线则要动判定面 | 中 | 阈值（软 / 硬线）与回退序先写死并进 spec；回退第 ①② 步在方案预设容差内；走到第 ③ 步（正则 import）属设计裁决变更，必须回到设计讨论而非实现层自定（AC-18） |
| R-2 | **Rust 归属器的长尾**：宏生成的 `mod`、`include!`、`#[path]` 组合、孤儿文件在真实 crate 上比语料多 | 归属漏判 → 少报违例（假绿）而非误报 | 中高 | 全部长尾一律落 `unresolved` 而非猜；spike C2 以本仓 13 个 workspace crate 为靶先行验证；`unresolved` 逐条留痕可审计 |
| R-3 | **`unresolved` 淹没真实违例**（explore 未决 2）：大量宏生成模块的工程里 `unresolved` 可能多到没人看 | 报告可读性下降，真实违例被噪声盖住 | 中 | 本期只警告不设阈值（决策留观察）；`unresolved` 与 `errors` 分段呈现、不参与 `passed`；若首个真实工程反馈淹没，再加「`unresolved` 超 N% 单独提示」属纯增量 |
| R-4 | **本仓基线全 `off`，门禁在自家仓库近乎空转** | 本变更落地后在本仓看不到「拦截」效果，容易误判门禁没工作 | 高 | 这是冷启动 ratchet 的**设计选择**而非缺陷：全 `off` ⇒ ②不产出 error、④只出 warning，门禁恒绿；效果由语料（三档 / 豁免 / 结构错）与本仓 `observations`（声明未兑现）验证；收紧一个模块的档位是后续独立变更。本变更只按漂移纪律追加 `ArchiRuntime` 元素与四条边（§11），不改变「全 `off` ⇒ 恒绿」这一事实 |
| R-5 | **归属粒度与模型 `path` 写法互相咬合**：`path` 写目录不写 `./` 前缀、或多个元素 `path` 重叠，会让归属结果反直觉 | 违例记到错元素上，反馈把 executor 引向错文件 | 中 | 规则写死为最长前缀 + 同长不任选（记 warning 而非猜）；`repo-archi-model` 已钉「叶元素必填、`./` 前缀、目录带 `/`」，语料含重叠 / 缺失 / 文件级三种形态（AC-2） |
| R-6 | **增量 ⊆ 全量的近似性**：增量只看变更文件，模型边可能由未变更文件证成 | 增量报「声明未兑现」而全量不报（① 在增量只作观察项，已规避主要形态）；② 可能漏报（变更文件外的旧违例不报） | 中 | ① 在增量模式只作 observation（不进门禁）；② 的漏报是模式定义（本 change 的实况），全量体检另跑；AC-7 断言「同元素对级别一致」而非「集合相等」 |
| R-7 | **反馈边把 executor 拉进「改代码 vs 改模型」的错解**：模型落后于真实设计时，executor 会硬改代码来消除违例 | 架构设计被代码反向绑架（最坏的情况：真实设计被退化） | 中 | 反馈文本明确列出「哪个文件引入哪个元素」并提示「若真实设计如此，任务应回到 dev-design 补模型」（提示进反馈模板，不进 prompt 静态面）；3 次超限即升格相位 fail 交决策分叉——决策者才拥模型权威 |
| R-8 | **新 step 词的 bincode 下标纪律**：`RunStepKind` / `StepKind` 落库按下标编码，中途插入会让存量行步词整体错位 | 历史 run 节点显示错词，审计面失真 | 低（有明文警告）但有前例事故 | 尾部追加写进 spec（`desktop-change-state-store` MODIFIED）+ `state_test` 断言语义词面与追加顺序；三枚举 + 审计包络四处同步（AC-16） |
| R-9 | **画布标记生产扩大范围**：报告 → 标记接线是本变更除「模型数据同步」（R-13）外唯一非「判定 / 拦截」的改动 | 范围蔓延，验证成本上升 | 中 | 接线收敛为一纯函数 + 一取数 hook + 画布内一行合并；样式契约与 `marks.ts` 零改动；若评审要求更瘦，此二文件可整体剥离为后续纯增量（不阻塞 AC-1..AC-18） |
| R-10 | **worktree 内报告的归档可见性**：`reports/archi/*.json` 随 change 目录进主仓，报告会长期堆积 | 仓库噪声 | 中 | 与 `reports/test` / `reports/baseline` 同口径（既有工件树惯例，不新增忽略规则）；增量模式一次门禁一份，量级与 test 报告同阶 |
| R-11 | **增量基准取 `HEAD` 与 explore 的 `base_commit` 差异**：若将来相位执行改为「阶段提交」，`HEAD` 差会漏掉已提交部分 | 增量范围变小 → 假绿 | 低（当前相位链零提交，提交只发生在 archive） | 取 `change_edit_set(root, "HEAD")` 与 mutation 同锚、同唯一入口（避免第二套口径）；该假设写进 spec「增量模式」段，若提交时机改变须同步两处（mutation 与本步） |
| R-12 | **`archi_config_path` 与工作区 `config.json` 混淆**：用户可能把架构豁免写进 `openspec/config.json` | 配置静默不生效 | 中 | 两侧 spec 与错误文案都写死管辖面；工作区配置 `deny_unknown_fields` 会直接拒绝 `architecture` 键（拼错即报错，不静默）；文档面在 `desktop-archi-drift`「架构域看护配置」段明示 |
| R-13 | **模型追加与既有「零改动」口径冲突**：本 change 的 `test-design.md` 与 `tasks.md` 已按旧口径写下「`.dev-team/architecture/models/*.toml` 与 `decisions/*` 不要修改」与 32 行实现清单 | 实现期照旧稿执行会漏改模型 ⇒ `repo-archi-model` 的双向覆盖（`:81`）与漂移纪律（`:82`）同时红，本变更即便功能全绿也不合格 | 高（旧稿已在案） | 本提案已把 `.dev-team/architecture/**` 从「不要修改」移入「实现文件」表并给出逐文件改法（§11）；设计 / 测试设计相位 MUST 同步这两处口径（实现文件 32 → 35 行、测试设计删该「不要修改」行）后再落地，旧稿不得作为豁免依据 |
| R-14 | **模型计数复算错一位**：50 / 111、后端 47、`03` 段 8 / 24、`04` 段 13 五组数字分散在模型表头、spec 计数表、ADR `§1`/`§4`/`§5`/`§6`/`§9` 四处 | 违反该能力自身「逐文件可加、两种拆分同值」判据，`archi_graph` 虽绿但模型与文档各说各话（第二事实源） | 中 | 五组数字由同一处复算得出（§11 表）；AC-21 逐项断言三处同值；核对留痕落 `reports/archi/`，MUST NOT 用「大致对齐」放行 |
| R-15 | **既有已知缺口被误当成本变更的缺陷**：`crates/tools/specta-zod-patched` 无元素（先于本变更存在） | 核对时若把该差集算进本变更，会把 50 元素 / 111 关系的合法基线判成不合格；反之若顺手补齐，则范围蔓延到无关 crate（元素与计数再变） | 中 | delta 明写「已知缺口单列、本变更 MUST NOT 补齐」，AC-21 按「自研 crate 15 ↔ 15 零差集 + 该条单列」核对；补缺或豁免判据归独立变更 |
