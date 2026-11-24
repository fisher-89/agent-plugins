# 测试设计: backtrack-window-feedback-overlap

> **变更**: backtrack-window-feedback-overlap（回溯窗口段 × 重试反馈段的同源重叠）
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（问题五节 / 提案六节 / 变更范围实现文件 **2 行** + 测试文件 **7 行** / AC-1…AC-12 / R-1…R-8）、`design.md`（D1–D9 / 架构组件 7 行 / 新增文件「无」+ 修改文件 2 行 / 公共函数·API 5 行 / 类型定义 5 行 / 四格判据矩阵 / AC 回应 12 行）、`specs/` 下 2 份 delta（`desktop-workflow-write-face` 1 MODIFIED / `desktop-change-orchestration` 1 MODIFIED）、`tasks.md`（阶段零…三与自查判据）
> **被测范围**: design「修改文件」子表 **全 2 行**——`crates/core/workflow/src/write/phase_next.rs`（同源抑制单点）+ `packages/desktop/package.json`（版本行）；**新增文件零项、删除文件零项**；`crates/core/orchestration/src/**` 与 `crates/infra/store/**` 为 design 明令零改动面，不设 per-file 章节。design 表内 Rust 路径相对 `packages/desktop/src-tauri`，本文统一展开为**仓库根相对路径**。
> **本变更性质**: 相位 prompt 组装面的**一处条件抑制**——当前相位是某开放窗口的发起相位（同源格）时失败反馈段整段不渲染，窗口段与 `backtrack_window` 逐字不变。零新字段 / 零 schema / 零 store 改动 / 零 IPC / 零 bindings / 零前端 / 零状态迁移。用例面因此**零新建测试文件**：全部落在既有 sidecar `phase_next_test.rs`（2 处增补断言 + 2 条新增用例 + 其余逐字不动）。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust `packages/desktop/src-tauri/crates/core/workflow` | `cargo test -p workflow`（既有 `phase_next_test.rs` **74 条**用例 / 3662 行） | **sidecar 同目录同前缀** `*_test.rs`，由 `write/mod.rs:66` 的 `#[cfg(test)] mod phase_next_test;` 挂载；本变更**零新建测试文件**（2 处增补 + 2 条新增） | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为**中文短语**、断言消息载判决；装置 = 手写端口假件 `RouteStore`（只实现 `get_change` / `list_phase_records`，写半边 `unimplemented!`）+ 条目工厂 `entry` / `entry_of` / `entry_with_id` / `backtrack_entry` / `backtrack_marker` + 路由驱动 `route(&fake, RUN, &anchors)` + 断言辅助 `window_entry` / `expected_block` / `assert_both_roles_carry` / `occurrences` / `ids` / `phase_def` |
| `packages/desktop/package.json` | — | **零测试路径**（`version` 元数据面） | 登记于「不可测试项」（AC-10 / AC-11 的 diff 审查） |
| `crates/core/orchestration/src/**`（零改动） | `cargo test -p orchestration`（既有用例） | 零用例增量（不在 design 修改文件表） | 既有用例零改动全绿即回归判据，登记于「不可测试项」 |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、极值、恰好阈值、防御式分支）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有…」前缀**：该臂由既有用例锁定，本设计只做**回归登记**（断言逐字不动）；未带前缀者为新增或增补。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| `bool`（内部判据 `same_origin`，非入参） | `true`（同源格）/ `false`（异源并存格与窗口外格）两值由三态矩阵逐格覆盖；`None` 类型层不可表达 |
| `Option<&PhaseStateRecord>`（`retry_feedback`） | `Some(verdict = Fail)`（同源格 / 异源格 / 窗口外格）/ `None`（回溯分支第一拍、`latest_phase_entry` 为 pass）——`None` 与同源抑制**可辨**（第 9 行） |
| `String`（`report` / `checklist[].evidence`） | `""`（空正文形态）/ 可辨识哨兵串 / 超长（`"字".repeat(MAX_REASON_CHARS)` 归既有体积界用例）/ 含换行与 emoji（归既有原文保真用例）；同源抑制与正文取值无关（第 18 行四形态逐字节相等） |
| `Vec<ChecklistItem>`（`checklist`） | `[]`（正文退回 `report`）/ 单条 fail / 多条含 pass 项（只渲失败项）/ 100 条超大列表；`None` 不可表达（`Vec` 非 `Option`） |
| `u32`（`attempt`） | `0`、`1`、`u32::MAX`；`-1` 类型层不可表达（第 21 行组合迭代） |
| `Verdict`（Enum） | `Pass` / `Fail` 两值（`Fail` 是反馈段准入与 `fail_decided` 面的判据）；非法值不可构造（本层零 serde 面） |
| `Vec<BacktrackWindowEntry>`（`open_windows`） | `[]` / 单元素 / 恰 `MAX_BACKTRACK_WINDOWS` / `+1`（裁剪行，第 7 / 10 行）/ `+2`（既有 `上限条数与常量同源`） |
| `usize`（`from_index` / `to_index` / `id`） | `0`（表首 `initialize`）/ 表长-1（`archive`）/ `usize::MAX`（既有 `窗口段渲染与下标取值无关`）；`id` 取互异递增（真实 store 的 `max+1` 语义） |
| `float` | **零 float 面**：无 `f64` 入参 / 字段，体积界算式全整数 |

**跨模块组合用例挂靠**

- **写面内部链**（`open_backtrack_windows` → `build_phase_response` → `PhaseNextOutcome.backtrack_window` / 两角色 prompt）发起方 = 写面 `phase_next` ⇒ 三态矩阵、裁剪相容、响应字段面等组合用例**全部**写 `phase_next.rs -> phase_next_test.rs` 章节；不新建独立集成测试章节、不新建组合测试区。
- **编排链**（`PhaseNextOutcome` → `walker` 原样下发 → 决策 prompt 三面）在 `crates/core/orchestration/src/**` 零改动（design 修改文件表零行）⇒ **零组合用例增量**；其 spec Scenario（「同源格反馈段由写面省略」/「决策 prompt 不受同源抑制」）由既有 orchestration 用例零改动全绿背书，登记于「不可测试项」第 4 行。

---

## 验收范围

<!-- 测试区域：`packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs`（sidecar，运行器 `cargo test -p workflow`）。本变更零新建测试文件；2 处增补断言（`:2406` / `:2374`）+ 2 条新增用例（三态矩阵 / 同源窗口过裁剪仍存活），其余 72 条既有用例逐字不动。`packages/desktop/package.json` 无测试路径，登记于「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **同源格抑制成立且非空转**：一次 `backtrack(from=test-gen, to=dev-design, reason=R)` 后流程重走完窗口并路由回 `test-gen`（其最新条目即标记行）⇒ executor 与 evaluator prompt 均含 `## 回溯重做窗口` 与 R，且均零 `⚠️ 上次评估未通过`、零标记行 `report` / `checklist` 正文；`backtrack_window` 仍为该窗口（响应字段面不因抑制收缩） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs`（三态矩阵同源格行 / 抑制非空转行 / 响应字段面行 / `:2406` 增补） |
| AC-2 | **判据单源、零第二份判定**：抑制判据在源码中只出现一次，形如 `open_windows.iter().any(\|w\| w.from_phase == def.id)`，位于 `build_phase_response` 内；`phase_next` 两处 `latest_phase_entry` 与路由判定零改动；零新增 `marker_id` 类字段、零记录 `id` / `timestamp` 比较 | `.../write/phase_next.rs`（源码面，静态判据）+ 见「不可测试项」第 2 行；运行期侧证 = 三态矩阵（放宽 / 收窄两向判红） |
| AC-3 | **异源并存格原样**：窗口内中间相位（非发起相位）带自身已决 fail 且决策未产出 backtrack ⇒ 双角色 prompt 同时含窗口段与反馈段、窗口段在前；`phase_next_test.rs:2932` 与 `:2374` 绿 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（三态矩阵异源并存格行 / `:2374` 增补 / `:2932` 逐字不动） |
| AC-4 | **窗口外格原样**：无开放窗口且最新条目 fail（决策 stop / 缺席 / 越权拒 / `backtrack` 写面 Err 后重启）⇒ prompt 只含反馈段（含 `（attempt N）` 与失败项）、零窗口段；`:3105` 形态三绿 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（三态矩阵窗口外格行 / 反馈段正文三形态行 / 既有 `零窗口回归` 逐字不动） |
| AC-5 | **目标相位与自环零变化**：回溯后第一拍（`backtrack_to` 分支）仍 `retry_feedback = None`；自环（`to == from`）第一拍与落账后行为逐字不变，源码零 `if to == from` / `if id == next` 类特判 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（第一拍与抑制可辨行 / 既有 `自环回溯退化为单点` 与 `:2347` 逐字不动）；源码零特判见「不可测试项」第 3 行 |
| AC-6 | **裁剪相容**：命中 ≥3 个同时开放窗口且当前相位是其中某窗口的发起相位 ⇒ 该同源窗口必在渲染集合内（创建序最末，`take_last` 保留），抑制照常生效 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（裁剪相容行 / `open_backtrack_windows` 末位不变式行） |
| AC-7 | **三态矩阵有守护力**：同一 fixture 族逐格断言「段 A 在场 / 不在场」与「段 B 在场 / 不在场」；放宽成「有窗口即抑制」时异源并存格判红、收紧成「`from == to` 才抑制」时同源格判红 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（三态矩阵三行 + 判据不收窄行） |
| AC-8 | **指令面无回归**：决策 prompt 仍携 fail checklist / `fail_report` / `backtrack_window` 三面；`error_outcome` / `done` 响应逐字不变 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（响应字段面行 / 既有 `error响应携覆盖失败相位的窗口两变体`、`done响应零窗口` 逐字不动）；决策面 = `crates/core/orchestration/src/**`（零改动，见「不可测试项」第 4 行） |
| AC-9 | **生成物零 diff**：`pnpm bindings:export` 后 `src/types/generated/**` 零 diff；`pnpm api:check` 零差异退出 | 机械面（见「不可测试项」第 5 行） |
| AC-10 | **版本纪律**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch（`0.7.9` → `0.7.10`）；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本逐字不变 | `packages/desktop/package.json`（配置 / 元数据面，见「不可测试项」第 1 行） |
| AC-11 | **改动集收敛**：`git status --porcelain` ⊆ 提案「变更范围」所列文件；`openspec/specs/**`、`openspec/changes/archive/**`、`explore.md` 零改动；orchestration / store / 前端零改动 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` + `packages/desktop/package.json`（白名单核对，见「不可测试项」第 1 / 6 行） |
| AC-12 | **spec delta 形态**：恰 2 份 delta；两份 `### Requirement` 标题与基线逐字一致；MODIFIED 块保基线全量未冲突 Scenario；每条 Requirement ≥1 个 `#### Scenario:` 且 WHEN / THEN 成对；两份均含 `## Module Contract` | 文档面（见「不可测试项」第 7 行） |

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs`

既有 sidecar（3662 行 / 74 用例）**增补 2 处断言 + 新增 2 条用例**，其余 72 条逐字不动。改动面一律落在「同源格 vs 异源并存格 vs 窗口外格」的在场性判别上：`build_phase_response` 的 `same_origin` 判据只截断 `feedback_suffix`，`window_section` / `backtrack_window` / 路由判定 / error 与 done 响应零触碰。

- **增补 1**：`回溯窗口四态_上界源头相位命中`（`:2406`）——现只断窗口段在场；标记行填可辨识 `report` / `checklist`，补「双角色 prompt 零 `⚠️ 上次评估未通过`、零其正文」的否定断言（R-5：不补断言等于白改）。
- **增补 2**：`回溯窗口四态_区间内部相位双角色携背景`（`:2374`）——补「双角色 prompt 仍含反馈段」（异源并存格，防抑制放宽成「有窗口即抑制」）。
- **新增 1**：三态矩阵（同源 / 异源并存 / 窗口外 同一 fixture 族 + 同一哨兵正文，逐格断言 A / B 在场性）。
- **新增 2**：同源窗口过层级裁剪仍存活（`MAX_BACKTRACK_WINDOWS + 1` 个窗口覆盖当前相位、同源窗口创建序最末 → 抑制仍生效）。
- **逐字不动（回归登记）**：`窗口段位置先于失败反馈段`（`:2932`）、`零窗口回归_既有种子窗口空集且prompt零段`（`:3105`）、`自环回溯退化为单点且闭环后窗口关闭`（`:2511`）、`跨run继承_新锚点下窗口仍在且round归1`（`:3369`）、`回溯与正常分支窗口段同形且零退役文案`（`:3043`）、`失败项随已决fail重跑prompt注入且首轮零注入`（`:717`）以及窗口推导 / 渲染 / 体积界 / 常量锚全部既有用例。

**装置口径（不属产品面，零新增导出）**：沿用 `RouteStore` / `route` / `backtrack_marker` / `entry_with_id` / `window_entry` / `assert_both_roles_carry`；「可辨识正文」经 `backtrack_marker(...)` 返回体就地覆写 `report` / `checklist` 注入哨兵串。同源格 fixture 必须让**全局最新条目不是标记行**（否则 `backtrack_to` 分支接管、`retry_feedback` 恒 `None`，抑制断言会空转）——沿用 `:2406` 的「`implement` pass 在 `t(12)`」手法。`fail_feedback_suffix` 是 `phase_next` 模块私有函数、sidecar 与其为兄弟模块不可直调，故正文在场性只能经**窗口外格端到端差分**观测（第 4 行）。

#### 待测功能

- `phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>`（零改动）: 两处 `latest_phase_entry`（`latest_fail` / `retry_feedback`）、`backtrack_to` 分支、`fail_decided` 幂等闸、两道分叉与正常路由逐字不变；同源格下唯一可观测差异是 `executor` / `evaluator` 的 `prompt` 少一段，`next_phase` / `round` / `steps` / `artifact_manifest` / `allowed_backtrack_phases` / `last_result` / `backtrack_window` / `error` / `done` 全部不变。
- `open_backtrack_windows(entries: &[PhaseStateRecord], table: &[PhaseDefinition], phase_id: &str) -> Vec<BacktrackWindowEntry>`（零改动）: 区间（含上界 `P = from`）/ 未闭环 / 按创建序 `take_last(MAX_BACKTRACK_WINDOWS)` 三判据逐字不动；同源判据只读其**输出**，不读其内部。
- `backtrack_window_section(windows: &[BacktrackWindowEntry]) -> String`（零改动）: 同源格下窗口段照常在场，文案 / 只取 `reason` / 标签行零属性 / 空集空串 / 不截断不 trim 全部保留。
- `build_phase_response(table, def, round, name, entries, last_result, retry_feedback) -> PhaseNextOutcome`（私有，**改**：内部新增一处条件，签名不变）: `open_windows` 之后求 `let same_origin = open_windows.iter().any(|window| window.from_phase == def.id);`，`retry_feedback.filter(|_| !same_origin).map(fail_feedback_suffix).unwrap_or_default()`；`window_section` 与 `backtrack_window` 的取值、插值序（`{context_header}{spec.prompt}{window_section}{feedback_suffix}`）不变。
- `fail_feedback_suffix(record: &PhaseStateRecord) -> String`（私有，零改动仅 doc）: 失败项优先 / 全 pass 清单配 fail verdict 全量随行 / 清单空退回 `report` / `（attempt N）` 头全部保留；消费者收窄为「非同源格的 fail 重试」与「窗口外格的 fail 重启」。

<!-- design.md「公共函数 / API」表对本文件声明上面 5 行（含 2 个私有件——列此只为钉住签名与调用面不变），本章节逐行覆盖。design「类型定义」表的 `PhaseNextOutcome` / `BacktrackWindowEntry` / `ResolvedPhaseSpec` / `PhaseNextError` 不是公共函数行，其字段面经上述 `phase_next` 与 `build_phase_response` 行覆盖；`DecisionInput` 属 orchestration（design 修改文件表零行），不在本章节。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `phase_next` | 正向 | **三态矩阵 · 同源格**（AC-1 / AC-7）：种子 `proposal` / `dev-design` / `test-design` pass + `implement` pass（`t(12)`、全局最新条目、无 `backtrack_to` ⇒ 走正常路由分支）+ `backtrack_marker(20, "test-gen", t(5), "dev-design", R)`（`decision_session_id` 在位、`report` / `checklist` 填哨兵）→ `next_phase == "test-gen"`；双角色 prompt 均含 `## 回溯重做窗口` 与 `<backtrack reason>\nR\n</backtrack>`、均零 `⚠️ 上次评估未通过`、零 `（attempt`、零哨兵正文；且窗口段与 `backtrack_window_section(&outcome.backtrack_window)` **逐字节**在场（抑制不触达窗口段） | 组合迭代（2 角色 × 2 标记行正文来源） |
| `phase_next` | 正向 | **三态矩阵 · 异源并存格**（AC-3）：同一 fixture 族把窗口改为 `test-gen → proposal`（覆盖内部相位）+ `test-design` 自身「已决但未产出 marker」的 fail 条目（`id` 更大、`t(12)`、携哨兵失败项）→ 双角色 prompt **两段并存**：`find(WINDOW_TITLE) < find("⚠️ 上次评估未通过")` 且哨兵失败项逐字在场。把判据放宽成「有窗口即抑制」时本行判红 | 组合迭代（2 角色 × 段序） |
| `phase_next` | 正向 | **三态矩阵 · 窗口外格**（AC-4）：无任何 `backtrack` 条目 + 目标相位「已决 fail」（决策 stop / 缺席 / 越权拒 / 写面 Err 后重启的同形条目）→ prompt 只含反馈段（`⚠️ 上次评估未通过（attempt N）` + 哨兵失败项）、零 `## 回溯重做窗口` / 零 `<backtrack reason>`；`backtrack_window` 空集。把判据收窄成「`from == to` 才抑制」不影响本行（无窗口） | 组合迭代（2 角色 × 2 正文来源：失败项 / 清单空退回 `report`） |
| `phase_next` | 正向 | **抑制非空转（差分对照）**：同一标记行记录（同一 `report` / `checklist` 哨兵）分别喂给窗口外格与同源格 → 前者 prompt 含哨兵正文（证该正文确会被渲染）、后者零命中；两格 fixture 的条目正文逐字相同 ⇒ 差异只能来自抑制闸 | 参数化迭代（2 正文来源：`report` 全文 / 失败项 `item：evidence`） |
| `phase_next` | 异常 | **判据不收窄（AC-7 反向判红）**：同源格窗口取 `to != from`（`test-gen → dev-design`）→ 抑制仍生效（实现写成「`to == from` 才抑制」则本行判红）；同 fixture 另断「窗口内已通过相位重跑」（`retry_feedback = None`）形态同样零反馈段但 `next_phase` 与 `last_result` 不同 ⇒ 两成因可辨 | 组合迭代（2 相位位置 × 2 成因） |
| `phase_next` | 异常 | **空正文不改变判据**：标记行 `checklist == []` 且 `report == ""` → 同源格零 `⚠️ 上次评估未通过`；对照窗口外格同形条目仍渲出段头 `⚠️ 上次评估未通过（attempt 1），请针对以下失败项修复：`（正文为空）⇒ 抑制不是「正文为空所以没渲」的空转 | 单次 |
| `phase_next` | 边界 | **裁剪相容（AC-6）**：`MAX_BACKTRACK_WINDOWS + 1` 个同时开放窗口全覆盖当前相位 `test-gen`（`acceptance → proposal` / `code-review → proposal` / 同源 `test-gen → dev-design`，创建序 id 10 / 20 / 30、同源窗最末）+ 一个 `t(12)` 非回溯 pass 条目令正常分支接管 → `backtrack_window.len() == MAX_BACKTRACK_WINDOWS`、末位 `from_phase == next_phase`、最早创建者原因零命中、双角色 prompt 零反馈段（抑制不因裁剪失效） | 单次 |
| `phase_next` | 边界 | **响应字段面不收缩**（AC-1 / AC-8）：同源格逐字段断言 `done == false` / `next_phase` / `round` / `steps` / `artifact_manifest`（与 `phase_def("test-gen")` 真值相等）/ `allowed_backtrack_phases`（id 序列零收缩）/ `last_result` / `error.is_none()` / `backtrack_window` 五字段；`executor.prompt` 与 `evaluator.prompt` 是唯一差异面 | 单次 |
| `phase_next` | 边界 | **第一拍与抑制可辨**（AC-5 / D6）：回溯分支第一拍（既有 `:2347` 目标相位 / `:2511` 自环）窗口段在场、反馈段缺席且 `retry_feedback` 恒 `None`；同源格同为零反馈段但走正常路由分支（`next_phase` = 窗口发起相位、`last_result` 取全局最新条目）⇒ 两形态输出可辨；零合并实现（`same_origin \|\| retry_feedback.is_none()`）见「不可测试项」第 3 行 | 参数化迭代（2 形态：常规回溯第一拍 / 自环第一拍） |
| `phase_next` | 正向（增补既有用例） | `回溯窗口四态_上界源头相位命中`（`:2406`）增补：标记行填可辨识 `report` / `checklist`，断言双角色 prompt 零 `⚠️ 上次评估未通过` 且零其正文；其余断言（`next_phase` / `backtrack_window` 逐字段 / 双角色窗口段在场）逐字保留 | 单次 |
| `phase_next` | 正向（增补既有用例） | `回溯窗口四态_区间内部相位双角色携背景`（`:2374`）增补：双角色 prompt **仍**含 `⚠️ 上次评估未通过` 与失败项正文（异源并存格对照，与 `:2932` 段序用例互不重复） | 单次 |
| `phase_next` | 异常（既有回归） | **窗口外格与反馈段既有用例逐字绿**：`零窗口回归_既有种子窗口空集且prompt零段`（形态三：只反馈段、零窗口段）/ `失败项随已决fail重跑prompt注入且首轮零注入`（首轮零注入 + 失败项注入 + pass 项不随行 + 回溯分支零注入）/ `重试上限已决fail放行重跑不空转` / `评估only相位已决fail放行恰一次重跑`（两处「重跑携反馈段」断言） | 参数化迭代（4 用例） |
| `phase_next` | 异常（既有回归） | **指令面与两分支同形逐字绿**：`error响应携覆盖失败相位的窗口两变体`（error 响应只有 `backtrack_window`、双 spec 缺席）/ `done响应零窗口` / `错误响应不下发计划步与清单` / `回溯与正常分支窗口段同形且零退役文案`（正常分支反馈段仍在） | 参数化迭代（4 用例） |
| `phase_next` | 边界（既有回归） | **自环 / 跨 run / 体积界逐字绿**：`自环回溯退化为单点且闭环后窗口关闭` / `跨run继承_新锚点下窗口仍在且round归1` / `窗口段体积界与常量算式`（第一拍 `retry_feedback = None`；判据不读 anchor / run_id；`MAX_REASON_CHARS` 长原因逐字在场） | 参数化迭代（3 用例） |
| `open_backtrack_windows` | 正向 | **同源窗口恒在裁剪后集合内**（AC-6 结构性质，直调）：三窗覆盖 `test-gen` → 输出逐字段等于 `[window_entry("code-review", "proposal", R2), window_entry("test-gen", "dev-design", R3)]`；`R3` 窗口 `from_phase == "test-gen"` 且 `from_index == table_index("test-gen")`（= 当前相位下标，该组中最小）⇒ 同源判据 `any(from_phase == def.id)` 恒可命中 | 单次 |
| `open_backtrack_windows` | 异常 | **零窗口与表外相位**（既有 `open_windows_phase_id不在表内返回空集` / `open_windows_防御式跳过表外相位`）→ 空集 ⇒ 同源判据恒 `false`、零 panic；区间 / 未闭环 / 裁剪三判据逐字不动 | 既有（逐字不动） |
| `open_backtrack_windows` | 边界 | **集合形态零改动**（既有 `嵌套双窗并集按创建序渲染` / `三层裁剪取创建序末位两层且被裁者from_index最大` / `求值顺序固定_先排序后裁剪` / `上限条数与常量同源` / `open_windows_条目集合三形态` / `open_windows_id全零退化解稳定不panic`）→ 本变更只读其输出，不改其内 | 既有（逐字不动） |
| `backtrack_window_section` | 正向 | **同源格窗口段渲染零改动**：同源格 prompt 内窗口段与 `backtrack_window_section(&outcome.backtrack_window)` 逐字节相等，段内原因按创建序、标签行零属性、`from_phase` / `to_phase` 零命中 | 单次 |
| `backtrack_window_section` | 异常 | **空集空串 / 零相位 id**（既有 `窗口段空集逐字节空串` / `窗口段零相位id泄漏两形态`）→ 零改动回归；同源格 `backtrack_window` 恒非空（同源 ⇒ 该窗口必在集合内），故不存在「同源格空窗口段」形态 | 既有（逐字不动）+ 单次 |
| `backtrack_window_section` | 边界 | **原文保真 / 体积界 / 下标无关**（既有 `窗口段原因原文逐字保真七形态` / `窗口段体积界与常量算式` / `窗口段渲染与下标取值无关`）→ 零改动回归（抑制不动窗口段字节） | 既有（逐字不动） |
| `build_phase_response` | 正向 | **抑制闸位置与段序退化**：同源格 prompt 序为「`change: <name>` 头 → 静态角色主体 → 窗口段」，即 `find(spec.prompt 的首句) < find(WINDOW_TITLE)` 且窗口段之后无反馈段（与既有 `:2932` 的异源段序用例构成两形态）；抑制不改插值序 | 单次 |
| `build_phase_response` | 异常 | **只抑制反馈段、不抑制窗口段**：同一 fixture 内 `WINDOW_TITLE` 在场与 `⚠️ 上次评估未通过` 缺席**同时**成立（防「整段抑掉」与「窗口段被误删」两种错法）；`executor` / `evaluator` 同一闭包、两角色同判 | 单次 |
| `build_phase_response` | 边界 | **标记行 `checklist` 四形态**（`[]` + 哨兵 `report` / 单条 fail 项 / 多条含 pass 项 / 100 条 fail 项）→ 同源格四形态 prompt **逐字节相等**（抑制后正文来源无关）、均零哨兵命中、零 panic | 参数化迭代（4 形态） |
| `fail_feedback_suffix` | 正向 | **正文形状零变化**（窗口外格）：失败项优先（`- item：evidence`，pass 项不随行）/ 清单空退回 `report` 全文 / 全 pass 清单配 fail verdict 全量随行三形态逐字渲染（与既有 `:717` 的失败项 + pass 项过滤断言同源） | 参数化迭代（3 形态） |
| `fail_feedback_suffix` | 异常 | **消费面收窄不改变语义**：窗口内非同源格的 fail 重试（矩阵异源并存格）与窗口外格的 fail 重启（矩阵窗口外格）两消费者均照常渲正文 ⇒ 抑制未把函数降级为死代码；函数本体内零同源判据（静态面见「不可测试项」第 2 行） | 参数化迭代（2 消费者） |
| `fail_feedback_suffix` | 边界 | **`（attempt N）` 三值 × 两格**：`attempt ∈ {0, 1, u32::MAX}` → 同源格零 `（attempt`、窗口外格逐字渲染该值（R-1 信息损失只发生在抑制格，且未波及窗口外格） | 组合迭代（3 attempt × 2 格） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ChangeStateStore`（既有 `RouteStore` 手写假件） | 只实现 `get_change` / `list_phase_records`（可注入 `StoreFault`），其余方法 `unimplemented!("本用例不可达")` 的结构性执法——store 是**进程边界 + `phase_next` 的显式注入依赖** | 全章节 |
| 不 mock：`phase_table("requirement")` | 真实静态相位表——`col` / `from_index` / `to_index` 的序真值源；AC-6「同源窗口 `from_index` = 当前相位下标且为该组最小」只有在真表下才有判据力 | 全章节（尤其裁剪相容行 / 末位不变式行） |
| 不 mock：`open_backtrack_windows` / `backtrack_window_section` | 同模块真值直调——二者同时是被测对象与装置；替身化会让「抑制只作用于反馈段」「同源窗口恒在裁剪后集合内」自证 | 全章节 |
| 不 mock（且不可 mock）：`fail_feedback_suffix` | `phase_next` 模块私有函数，sidecar 与其为兄弟模块不可直调；正文在场性只能经窗口外格端到端观测（第 4 行差分对照即其替代装置），**不为此新增测试专用导出** | 非空转行 / 消费面行 / `（attempt N）` 行 |
| 不 mock：`SessionAnchors` | 真实实例，每用例独立（首见锚点 = 当时条目数 ⇒ `round == 1`、`window_fails` 预算为 0）；跨 run 行另建第二实例 | 全章节 |
| 不 mock：时钟 / 环境变量 | 条目 `timestamp` 全为 `t(n)` 字面量（`T0` + n 秒），零 `SystemTime`、零假时钟；哨兵正文以常量串注入，零随机 | 全章节 |
| 不 mock：`Verdict` / `ChecklistItem` / `PhaseStateRecord` / `BacktrackWindowEntry` | 真实领域类型直构（结构体字面量按需覆写；`backtrack_marker` / `entry_with_id` / `window_entry` 工厂负责形态差异） | 全章节 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` `0.7.9` → `0.7.10`） | 配置 / 元数据面，无运行期行为可断言；判据是**相对递增**（相对落地时 `HEAD` 恰 +1 patch，MUST NOT 写死字面号） | diff 审查（桌面端静态步骤）：`package.json` 恰一行变更；`src-tauri/tauri.conf.json` 零 diff（仍 `"version": "../package.json"`）；`src-tauri/Cargo.toml` 的 `version = "0.1.0"` 逐字不变（AC-10 / AC-11） |
| AC-2 判据单源（`from_phase == def.id` 在 `phase_next.rs` 命中数 == 1、`same_origin` 命中数 == 2、零记录 `id` / `timestamp` 比较、零新增 `marker_id` 类字段） | 源码位置事实 + 「不存在」类缺席事实，运行期不可观测 | 桌面端静态检查 grep（tasks.md 阶段一自查项：`grep -c 'from_phase == def.id'` / `grep -c 'same_origin'` / `grep -rn 'same_origin' … \| grep -v phase_next.rs` 零命中）+ `git diff -U0` 逐 hunk 审查；运行期侧证 = 三态矩阵（放宽 / 收窄两向判红） |
| AC-5 的**源码零特判**（零 `if to == from` / `if id == next`）与 D6 的**零合并分支**（无 `same_origin \|\| retry_feedback.is_none()` 类写法） | 缺席事实；且合并写法与本变更产出**行为等价**（抑制闸对 `None` 无作用）⇒ 运行期不可分辨 | 源码审查（design D6 明令）；运行期侧证 = 第一拍与抑制可辨行（两成因的观测面可辨）+ 既有自环与 `:2347` 用例 |
| AC-8 的决策 prompt 三面（fail checklist / `## 失败相位报告` / `## 回溯重做窗口`）与 orchestration 侧零同源判据 | `crates/core/orchestration/src/**` 不在 design 修改文件表 ⇒ 本 change 零用例增量；判据是「零改动」而非「新行为」，新增用例反而会与「零 diff」主张重复 | 既有 orchestration 用例零改动全绿即回归（`prompt_test.rs` 决策段、`walker_test.rs:16354` 反馈段逐字断言）；另 `grep -rn 'from_phase ==' crates/core/orchestration` 零命中、`grep -rn 'retry_feedback' crates/core/orchestration` 零命中 |
| AC-9 生成物零 diff（`bindings:export` / `api:check`） | 生成步骤的产物比对，非单元测试面（`PhaseNextOutcome` / `BacktrackWindowEntry` 不出 bindings，`.dev-team/architecture/models/**` 无 workflow 元素） | tasks.md 阶段三执行生成步骤 + `git diff --stat -- packages/desktop/src/types/generated/` 零输出 |
| AC-11 改动集收敛 | git 机械面 | `git status --porcelain` ⊆ 白名单（本 change 目录 + `phase_next.rs` + `packages/desktop/package.json`）；orchestration / store / prompts / 前端 / 生成物 / 基线 spec / archive / `explore.md` / `AGENT.md` / `src-tauri/Cargo.toml` 零 diff |
| AC-12 两份 spec delta 形态（标题逐字一致 / Scenario 数与 `####` 层级 / WHEN·THEN 成对 / `## Module Contract`） | 文档面，无运行期路径 | 桌面端评审 + `openspec validate`（若门禁链内含）+ 与基线 `openspec/specs/desktop-workflow-write-face/spec.md`、`desktop-change-orchestration/spec.md` 逐段对照 |
| 两处 doc 同步（`build_phase_response` 的「失败反馈段（fail 重试且非同源格时在场）」与 `fail_feedback_suffix` 的「窗口外格消费者」）与条件处一行注释 | 注释文本，零运行期路径 | `git diff` 审查（doc 行变更随产品文件，净增代码 ≤ 4 行约束） |
| `crates/core/workflow/src/write/{phase_table,mod,backtrack}.rs`、`crates/infra/store/**`、`crates/core/workflow/src/prompts/**`、`packages/desktop/src/**`、`src/types/generated/**` | 不在 design 修改文件表 ⇒ 不设 per-file 章节；「零 diff」即主张本身 | AC-11 白名单核对 + 既有用例全绿（`phase_table_test.rs` 常量锚 / store 测试 / 前端 `vp test`） |
