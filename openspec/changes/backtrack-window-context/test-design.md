# 测试设计: backtrack-window-context

> **变更**: backtrack-window-context
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（问题 / 提案 1–7 / 变更范围实现文件 **7 行** + 测试文件 **13 行** / AC-1..AC-15 / R-1..R-11）、`design.md`（D1–D16 / 架构组件 9 行 / 新增文件「无」+ 修改文件 6 行 + 版本行 / 编译闭合随动 4 文件 9 处 / 公共函数·API 4 行 / 类型定义 3 行 / 派生判据伪码 / 渲染产物形状 / 待决问题 1..6）、两份 delta spec（`desktop-workflow-write-face` 2 MODIFIED + 2 ADDED / `desktop-change-orchestration` 3 MODIFIED）、`tasks.md`（七阶段与预期红灯登记）
> **被测范围**: design「变更清单 - 修改文件」子表 **全 7 项**——6 个产品源码面（`phase_next.rs` / `phase_table.rs` / `write/mod.rs` / `decision.rs` / `prompt.rs` / `walker.rs`）+ `packages/desktop/package.json`（版本行）；**新增文件零项、删除文件零项、零新增依赖、零前端改动、零 bindings 改动**。
> **本变更性质**: 写面读面派生语义补全——「回溯背景」由「目标相位的一次性 prompt 后缀」改为「回溯窗口内每个相位的每个 agent 步随行的背景段」。窗口由既有 `PhaseStateRecord` 派生（零新表 / 零新列 / 零 schema 迁移 / 零新时钟），`PhaseNextOutcome` 与 `DecisionInput` 各增一个 `Vec` 字段，写面删 `reason_suffix` 并新增「推导 + 渲染」两个单点函数。用例集中在两处纯函数写法（推导判据 / 渲染字节契约）与两处响应填充 / 决策 prompt 段序。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/**`） | `cargo test --workspace`（既有基线 **7429** 条） | **sidecar 同目录同前缀** `*_test.rs`，源文件尾 / `mod.rs` / `lib.rs` 以 `#[cfg(test)] mod <name>_test;` 挂载（`write/mod.rs:52-73` 的 11 条、`orchestration/src/lib.rs:35-53` 的 11 条先例）；**本变更零新建测试文件**——**5 个既有 sidecar 改写 + 追加**（`phase_next_test.rs` 2156 行 / 41 用例、`phase_table_test.rs` 3176 行 / 65 用例、`prompt_test.rs` 802 行 / 20 用例、`decision_test.rs` 423 行 / 14 用例、`walker_test.rs` 15862 行），另 `port_test.rs` 只做结构体字面量补字段（零用例改动） | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为**中文短语**、断言消息载判决；装置为手写端口假件（`RouteStore` / `FakeTools` / `FakeWorker` / `StubSnapshot`）与 `TestDb` 真库 |
| 前端 / 生成物 / 配置文件 | — | **零测试路径**（`src/**`、`src/types/generated/**`、`package.json`、`config.schema.json` 均零改动或零行为面） | 登记于「不可测试项」 |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、极值、恰好阈值、防御式分支）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有…」前缀**：该臂由既有用例锁定，本设计**只做回归登记**；未带前缀者为新增或按新形状改写（改写面仅限 4 处 `⚠️ 回溯原因` 断言与其相邻说明，语义断言逐条保留）。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| `usize`（`from_index` / `to_index`） | `0`（表首 `initialize`）、`1`、`表长-1`（`archive`）、`usize::MAX`（直构脏值，判据不 panic）；`-1` 在 `usize` 类型层不可表达；`None` 不可表达（结构体字段必给） |
| `String`（`reason`） | `""`、首尾空白 `"  R  "`、含换行 `"a\nb"`、`\0` + emoji、`---`（撞 Markdown 分隔线语义的形态）、`== MAX_REASON_CHARS`(1000) 长文、`>1000`（上层写面已拒，本层不设第二道门）；`None` 出现在上游 `backtrack_reason: Option<String>`（→ `""`，**不 trim**） |
| `bool` | 零新增 bool 字段；既有 `PhaseStateRecord.stale` / `skipped` 仍是 `has_phase_passed` 的判据面（两值回归） |
| `Vec<BacktrackWindowEntry>` | `[]`（整段省略）、单元素、恰 `MAX_BACKTRACK_WINDOWS`、`+1` 与 `+2`（超限裁剪）、1000 元素（超大列表）；`None` 不可表达（`Vec` 非 `Option`，design 明令） |
| `BacktrackWindowEntry`（object） | 五字段必给（缺字段 / 多余字段均编译失败，`{}` 不可构造）；各字段值取上列边界；`Eq` 可辨 |
| `Option<String>`（`backtrack_to` / `backtrack_reason`） | `None` / `Some("")` / `Some("   ")` / `Some("  dev-design  ")`（trim 命中） |
| `Verdict`（Enum） | `Pass` / `Fail` 两值（条目 verdict 影响 `has_phase_passed` 与两处分叉）；非法值不可构造（本层零 serde 面） |
| `float` | 零 float 面：无 `f64` 入参 / 字段，体积界算式全整数 |

**跨模块组合用例挂靠**

- **写面内部链**（`open_backtrack_windows` → `build_phase_response` / `error_outcome` → `PhaseNextOutcome.backtrack_window` → 两角色 prompt）发起方 = 写面 `phase_next` ⇒ 组合用例写 `phase_next.rs -> phase_next_test.rs` 章节。
- **编排链**（`PhaseNextOutcome` error 响应 → `resolve_deadlock` → `build_decision_input` → 真实 `decision_prompt` 组装 → 决策会话请求 prompt）发起方 / 最上层调用方 = `walker` ⇒ 组合用例写 `walker.rs -> walker_test.rs` 章节，**执行文件 `walker_test.rs`**（既有 `decision_prompt_of(&requests)` 装置）。
- 不新建独立集成测试章节、不新建组合测试区；写面推导 / 渲染与编排侧 prompt 组装在跨 crate 单点上以「逐字节相等」对拍（文案单点判据）。

---

## 验收范围

<!-- 测试区域：`packages/desktop/src-tauri/crates/core/{workflow,orchestration}/src/**/*_test.rs`（sidecar，运行器 `cargo test --workspace`）。本变更零新建测试文件；5 个既有 sidecar 改写 + 追加（`phase_next_test.rs` / `phase_table_test.rs` / `prompt_test.rs` / `decision_test.rs` / `walker_test.rs`），另 `port_test.rs` 与三处 `walker_test.rs` 装置函数、两处 `prompt_test.rs` 完整构造器、`decision_test.rs` 的 `base` 只做结构体字面量补字段的编译闭合随动，不新增用例。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **窗口内每个相位的每个 agent 步都带背景**：一次 `backtrack(to=T, reason=R)` 后，从 T 到发起相位 S 的每个相位的 executor **与** evaluator prompt 均含 `## 回溯重做窗口` 段且含 R；窗口外相位零该段；`S` 自身命中（裁决 ①） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` -> `.../write/phase_next_test.rs`（四态用例）；`.../orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（evaluator 协议附录在窗口段之后） |
| AC-2 | **区间判据单源**：`to_index ≤ col(P) ≤ from_index`，`col` 取自相位表 position（与 `detail.pipeline` 下标同源）；自环（`to == from`）退化为单点、同一条代码承载，源码零 `if to == from` 特判 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（自环 + 嵌套 + 索引边界行）；源码零特判的静态判据见「不可测试项」 |
| AC-3 | **未闭环判据不读 run 状态**：`open(W)` 仅由「发起相位是否有 id 更大的条目」判定；跨 run（新 `run_id`、新 anchor、`round` 归 1）仍命中同一未闭环窗口；判据源码零 `SessionAnchors` / `run_id` 命中 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（跨 run 行：`round == 1` 与窗口在场并存）；源码面见「不可测试项」 |
| AC-4 | **A2 并集与上限的双分支**：命中 0 / 1 / 2 层全量并集（创建序升序）；命中 3 层只渲染末位 2 层，被裁恒为创建序最早那条（结构性等价：`from_index` **最大**）；窗口段零「另有 N 条」类元信息 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（双窗并集行 + 三层裁剪行 + 渲染文本负向断言）；`.../write/phase_table.rs` -> `.../write/phase_table_test.rs`（上限常量锚） |
| AC-5 | **求值顺序固定**：`filter(open) → filter(connect) → sort(创建序) → take_last(2)`；「先 `take_last` 再 `sort`」被反例用例判红（输入条目序 ≠ 创建序） | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（顺序行：输入条目序与创建序不同） |
| AC-6 | **单点位置与响应填充**：正常响应填「覆盖 `next_phase` 的开放窗口」、error 响应填「覆盖失败相位的开放窗口」；两处共用同一推导 fn | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（error 两变体行 + 正常/`done` 行） |
| AC-7 | **S2 取代原因行**：`⚠️ 回溯原因` 与 `reason_suffix` 在产品 `.rs` 零命中；`build_phase_response` 的 `backtrack_reason` 入参删除、两分支入参收敛；窗口段位置 = 静态主体之后、`FAIL` 反馈段之前 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（4 处改写断言 + 段位置索引断言 + prompt 文本零 `⚠️ 回溯原因`）；`.../orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（executor / evaluator 段序）；零命中静态判据见「不可测试项」 |
| AC-8 | **渲染零元信息**：标签行零属性、可被行首精确匹配；原因逐字保留（换行 / 首尾空白 / 标点，不截断不压平不 trim）；prompt 正文零相位 id 字面量 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（原文保真 + 渲染零 id 行）；`.../orchestration/src/prompt.rs` -> `.../orchestration/src/prompt_test.rs`（零泄漏行） |
| AC-9 | **文案与常量单点**：窗口段文案 + 渲染 fn 只在写面一处（经 `write/mod.rs` 再导出），orchestration 侧零第二份文案 / 零第二份判据；`MAX_BACKTRACK_WINDOWS` 只在 `phase_table.rs` 定义一处，prompt 文案与测试断言零写死 `2` | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（裁剪条数断言引用常量）；`.../write/phase_table.rs` -> `.../write/phase_table_test.rs`（常量锚）；`.../orchestration/src/prompt.rs` -> `.../orchestration/src/prompt_test.rs`（与写面渲染产物逐字节相等）；`.../write/mod.rs` 出口面见「不可测试项」 |
| AC-10 | **决策面携带窗口**：`DecisionInput.backtrack_window` 由 walker 透传自 error 响应（零 store 读取、零相位表推导）；决策 prompt 含段、位置在 `## 失败相位报告` 之前、空集整段省略、只渲染原因；条目数语义与写面双分支一致，orchestration 零二次裁剪 | `.../orchestration/src/decision.rs` -> `.../orchestration/src/decision_test.rs`（字段面）；`.../orchestration/src/prompt.rs` -> `.../orchestration/src/prompt_test.rs`（多条 / 空集 / 段位置 / 零泄漏 / 2 条与 3 条均逐条渲染）；`.../orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（透传） |
| AC-11 | **零结构扩散**：零新表 / 零新列 / 零 schema 迁移 / 零 store 改动；`BacktrackInput` / `backtrack.rs` 逐字不变；`port.rs` / `steps.rs` / `control.rs` / `state.rs` 零改动；零新 port / step 词 / 通知类型 | 零 diff 面（见「不可测试项」）；行为侧证 = `phase_next_test` / `walker_test` 既有用例零断言改动即全绿 |
| AC-12 | **交付面**：`cargo clippy --all-targets -D warnings` 与 `cargo test` 全绿（含既有 7429 条零回归）；`backtrack_window` 不出 bindings ⇒ `bindings:export` 零 diff；`vp check` / `knip` 通过；`packages/desktop` 的 `version` = 当前 `HEAD` 值 +1 patch | `packages/desktop/package.json`（配置面）+ 机械门面（均见「不可测试项」）；运行期判据是本表其余各行 |
| AC-13 | **体积有界**：单相位窗口段 ≤ `MAX_BACKTRACK_WINDOWS` ×（标签行 + `MAX_REASON_CHARS`）；实现零自行截断 | `.../write/phase_next.rs` -> `.../write/phase_next_test.rs`（1000 字符原因 × 上限条数的界断言 + 逐字在场）；`.../write/phase_table.rs` -> `.../write/phase_table_test.rs`（界算式用常量） |
| AC-14 | **明确不做项零触碰**：archive 的 `spec_sync_prompt` / `merge_conflict_prompt` 零窗口段；图面 `graph.ts`、前端 `src/**`、`src/types/generated/**` 零 diff | `.../orchestration/src/prompt.rs` -> `.../orchestration/src/prompt_test.rs`（archive 两步零窗口段的行为侧证）；diff 面见「不可测试项」 |
| AC-15 | **spec 双分支如实写清**：两处 delta 均写「≤ 上限全量并集 / 超限取创建序末位两层」，裁剪方向只有一种表述（创建序最早 ≡ `take_last` 首先丢弃 ≡ `from_index` **最大**） | 文档面（见「不可测试项」）；本设计的用例断言把等价链钉成运行期判据（三层裁剪行断「`from_index` 最大者不出线」） |

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs`

既有 sidecar（2156 行 / 41 用例）**改写 4 处 S2 断言 + 追加窗口判据用例组**。改写面 = 行 `:718` / `:1035` / `:1084` / `:2098` 的 `⚠️ 回溯原因` 断言及其相邻 8 处说明，语义断言（路由目标、白名单、零反馈注入、上下文头）逐条保留；追加面 = 四态 / 自环 / 嵌套双窗 / 三层裁剪 / 求值顺序 / 内层回跳 / 跨 run / error 响应 / `done` / 直调推导 / 直调渲染。装置沿用 `RouteStore`（只实现读半边、写半边 `unimplemented!` 的结构性执法）+ `entry` / `backtrack_entry` / `entry_of` 工厂 + `SessionAnchors`（跨 run 行新建第二实例）。

**装置适配（本变更的必要前提，不属产品面）**：既有工厂 `entry` / `entry_of` 恒写 `id: 0`，而窗口判据的创建序与未闭环判据都取 `PhaseStateRecord.id` ⇒ 窗口用例的种子条目必须带**递增且互异**的 id（同真实 store 的 `max+1` 分配）。落地形态 = 新增 `entry_with_id(...)` / `backtrack_entry_with_id(...)` 装置函数（或在种子后按输入序回填 id），**既有 41 条用例的 `id: 0` 基线与断言值零改动**（它们的路由面不走窗口判据）；「全部 id 为 0 的退化解」本身是一条异常用例（见下表末行）。

#### 待测功能

- `phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>`（体内变化、签名不变）: 三处 `backtrack_window` 填充（`done` 响应空集 / `error_outcome` 覆盖失败相位 / `build_phase_response` 覆盖 `next_phase`）；`round` 上限、backtrack 分支、两处分叉、幂等闸、正常路由语义逐字不变；窗口段插在原 `reason_suffix` 处（静态主体之后、反馈段之前）。
- `open_backtrack_windows(entries: &[PhaseStateRecord], table: &[PhaseDefinition], phase_id: &str) -> Vec<BacktrackWindowEntry>`（新增，`pub(crate)`）: 候选标记行 = `backtrack_to` trim 后非空 → 区间判据 `to_index ≤ col(phase_id) ≤ from_index` → 未闭环判据（发起相位无 `id > 标记行 id` 的条目）→ 按标记行 `id` 升序 → `skip(len.saturating_sub(MAX_BACKTRACK_WINDOWS))`；`phase_id` 不在表内 / 标记行相位或目标不在表内 → 不收该候选（防御式）；入参不含 `SessionAnchors` / `run_id` / `anchor`。
- `backtrack_window_section(windows: &[BacktrackWindowEntry]) -> String`（新增，`pub`，经 `write/mod.rs` 再导出）: 空集 → `""`；非空 → 前导 `\n\n` + 标题 + 说明段 + 逐条 `<backtrack reason>\n{reason}\n</backtrack>`（块间 `\n\n`）、尾随零换行；**只取 `reason`**，零属性、零相位 id、不截断不压平不 trim。

<!-- design.md「公共函数 / API」表对本文件只声明上面三行。design「类型定义」表另声明 `BacktrackWindowEntry`（五字段，derive Debug/Clone/PartialEq/Eq，不 derive Serialize）与 `PhaseNextOutcome.backtrack_window`（Vec 非 Option），二者不是公共函数行，覆盖归上述三行（字段面由推导输出逐字段断言、Serialize 缺席由「不可测试项」的生成物判据守住）。私有件 `build_phase_response` / `error_outcome` / `latest_*` / `fail_feedback_suffix` 按 design 纪律不入本表，其新形参与填充面经 `phase_next` 的公共面覆盖。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `phase_next` | 正向 | **四态**（AC-1）：一次 `backtrack_entry(from="test-gen", to="dev-design", reason=R)` 后逐相位路由 —— 目标 `dev-design` / 中间 `test-design` / 源头 `test-gen`（裁决 ① 上界命中）三态的 executor 与 evaluator prompt 均含 `## 回溯重做窗口` 且含 R；窗外形态取「区间下界之前」的 `proposal`（`col < to_index`）→ prompt 零该段、`backtrack_window` 为空集 | 组合迭代（4 相位位置 × 2 角色） |
| `phase_next` | 正向 | **自环单点**（AC-2）：`backtrack_entry(phase="implement", to="implement", reason=R)` → 路由 implement、prompt 含段与 R（`to_index == from_index` 退化为单点）；随后 push `entry("implement", 2, Pass)`（`id` 更大）→ 再路由时该窗口已闭环（`backtrack_window` 空、prompt 零段）。反例断言：源码零 `if to == from` 特判（静态判据见「不可测试项」，此处以「常规回溯与自环走同一落点、输出形状同构」侧证） | 参数化迭代（未闭环 / 已闭环 2 形态） |
| `phase_next` | 正向 | **嵌套双窗并集**（AC-4 的 A2 面）：Win1 `from=test-gen → to=dev-design`、Win2 `from=test-design → to=proposal` 同时开放 → 路由 `test-design` 两窗均命中，段内按创建序 `[R1, R2]`（早者在前）；路由 `implement` 只命中 Win1 一条；路由 `proposal` 只命中 Win2 一条 | 组合迭代（3 相位 × 2 窗口） |
| `phase_next` | 正向 | **三层上限取末位两层**（AC-4）：三窗同命中（Win1 `from_index=5` / Win2 `4` / Win3 `3`，创建序即 Win1 < Win2 < Win3）→ 段内恰 2 条、原因为 Win2 与 Win3、Win1 原因文本零命中；并断 Win1 的 `from_index` 在三条中**最大**（结构性等价链的可断言形式），零「另有 N 条」/「仅显示最近」类元信息 | 单次 |
| `open_backtrack_windows` | 正向 | **求值顺序固定**（AC-5）：构造三窗但 `entries` 输入序 ≠ 创建序（如 id 序 30 / 10 / 20），`from_index` 仍 5 / 4 / 3 → 输出等于「按标记行 id 升序后取末位 `MAX_BACKTRACK_WINDOWS` 条」；反例断言：按「先取输入末位两条再排序」得的集合含 Win1（`from_index` 最大者）⇒ 该实现被判红 | 单次 |
| `phase_next` | 正向 | **内层回跳不设状态机**（AC-2）：外层 Win `[dev-design..test-gen]` 开放期间内层 `from=implement → to=proposal`；路由 `proposal` 只渲内层一条（外层 `to_index` 之外）、`dev-design` 两条（两区间均含）、`test-gen` 只渲外层一条；零暂停 / 恢复标志参与（判据由区间自愈） | 组合迭代（3 相位 × 2 窗口） |
| `phase_next` | 正向 | **跨 run 继承**（AC-3）：回溯后未闭环，新建 `SessionAnchors::new()` + 新 `run_id`（`round` 归 1）→ 目标相位 prompt 仍含段与原因；同时断言 `outcome.round == 1`（预算层按新 run 窗口）与 `backtrack_window` 非空（背景层按全量事件序）并存——若判据误切片 `entries[anchor..]`，新 run 下该切片为空必得空窗口，与断言相反 | 单次 |
| `phase_next` | 正向 | **error 响应携窗口**（AC-6）：失败相位落在开放窗口内并触发决策分叉 → `outcome.error` 为对应变体且 `outcome.backtrack_window` 逐字段等于覆盖失败相位的开放窗口（`from_phase` / `from_index` / `to_phase` / `to_index` / `reason`） | 参数化迭代（`VerdictFail` / `MaxRetriesExceeded` 2 变体） |
| `phase_next` | 正向 | **`done` 响应零窗口**（AC-6）：全相位 pass → `done == true` 且 `backtrack_window.is_empty()`、`executor` / `evaluator` 双缺席（`done` 分支不消费窗口面，与既有终态断言并存） | 单次 |
| `phase_next` | 正向 | **段位置契约**（AC-7 / spec「段位置先于反馈段」）：窗口内相位同时带 fail 重试反馈（`latest_phase_entry` 为 fail 且决策已挂账 ⇒ 走正常路由）→ `prompt.find("## 回溯重做窗口") < prompt.find("⚠️ 上次评估未通过")` 同时在场时成立 | 单次 |
| `phase_next` | 异常 | **S2 断言改写**（AC-7）：4 处原 `⚠️ 回溯原因: <R>` 断言改写为「`## 回溯重做窗口` 在场 + `<backtrack reason>` 行在场 + R 逐字在场」，并新增反向断言 prompt 零 `⚠️ 回溯原因` 字样；回溯分支与正常分支产出形状收敛（窗口段同形、仅反馈段在场性不同） | 参数化迭代（4 处改写点） |
| `phase_next` | 异常 | **窗口段零相位 id**（AC-8，写面侧）：以 `backtrack_window_section(&outcome.backtrack_window)` 的产物在 prompt 内定位窗口段子串 → 该子串对 `from_phase` / `to_phase` 字面量零命中；**MUST NOT** 断言整篇 prompt（静态角色 md 可能自然提及相位名，断言会空转） | 参数化迭代（2 形态：单条 / 多条窗口） |
| `phase_next` | 异常 | **零窗口回归**（AC-7 的空集分支）：库内零 backtrack 条目的既有种子（表头 pass / 正常推进 / fail 重试）→ `backtrack_window == []` 且 prompt 零 `## 回溯重做窗口`（空集分支与改动前 prompt 逐字节等同的侧证：既有非窗口断言零改动即全绿） | 单次 |
| `open_backtrack_windows` | 异常 | `phase_id` 不在表内 → 空集（`"unknown-phase"` / `""` / `"   "` 三形态，零 panic） | 参数化迭代（3 形态） |
| `open_backtrack_windows` | 异常 | 防御式跳过：标记行自身相位不在表内 / `backtrack_to` 目标不在表内 → 该候选被跳过、其余候选照常出线（生产不可达形态，判据不得 panic 或产越界索引） | 参数化迭代（2 形态） |
| `open_backtrack_windows` | 异常 | 候选判据对 `backtrack_to` 的读取口径：`Some("dev-design")` / `Some("  dev-design  ")`（trim 命中）/ `Some("")` / `Some("   ")` / `None` → 前三类中仅前两组为候选，空串与空白串**不是**候选（trim 后空即无标记） | 参数化迭代（5 形态） |
| `open_backtrack_windows` | 异常 | legacy 缺 `reason`（D14）：`backtrack_reason = None` 与 `Some("")` 两形态均**仍入选**（判据三条不含 `reason`），输出 `reason == ""`；对应渲染块为空正文 `<backtrack reason>\n\n</backtrack>`，不 panic 不剔除 | 参数化迭代（2 形态） |
| `open_backtrack_windows` | 边界 | 条目集合三形态：`entries = []` → 空集；仅一条标记行 → 恰一条窗口；1000 条（多条标记行 + 大量非标记行混合）→ 不 panic 且长度 ≤ `MAX_BACKTRACK_WINDOWS` | 参数化迭代（3 形态） |
| `open_backtrack_windows` | 边界 | 索引边界：`to_index == 0`（`initialize`）与 `from_index == 表长-1`（`archive`）的合成标记行 → 区间覆盖全表（任意相位均命中）；断判据面无 `backtrackable` 额外过滤（白名单约束在写面 `backtrack` 侧，不在窗口判据） | 单次 |
| `backtrack_window_section` | 正向 | 形状（AC-8）：1 条与 2 条两形态 → 标题行 `## 回溯重做窗口（背景信息）` 在场、说明段在场、每条以 `\n<backtrack reason>\n` / `\n</backtrack>` 定界、块间 `\n\n`、**结束无尾随换行**；另按行切分断言「恰有一类以 `<backtrack` 起首的行，且该行逐字等于 `<backtrack reason>`」（零属性，含 `from=` / `order=` 形态零命中） | 参数化迭代（2 形态） |
| `backtrack_window_section` | 正向 | 顺序与不重排：入参 2 条按 `[R1, R2]` 传入 → 产物中 R1 索引 < R2 索引；同输入两次调用逐字节相等（纯函数确定性） | 单次 |
| `backtrack_window_section` | 异常 | 空集 → 逐字节空串（`assert_eq!(section, "")`）：两处消费点（写面 prompt 拼接 / 决策 prompt 插值）因此与非窗口形态逐字节等同 | 单次 |
| `backtrack_window_section` | 异常 | 零相位 id 泄漏（AC-8）：`from_phase` / `to_phase` 取可辨串的窗口 → 产物对二者零命中；`<backtrack ` 起首的行恰为 `<backtrack reason>`（逐行断言，抓「顺手带出相位名」的唯一失效方式） | 参数化迭代（2 形态：单条 / 多条） |
| `backtrack_window_section` | 边界 | 原因原文逐字保真（AC-8 / AC-13）：`""`（空正文块）/ `"  R  "`（首尾空白不 trim）/ `"a\nb"`（保留换行）/ `"a\0b🚀"`（特殊字符）/ `"---"`（不动用 Markdown 分隔线）/ `"字".repeat(MAX_REASON_CHARS)`（恰好上限）/ `"字".repeat(1200)`（越上游门限的直构脏值）→ 每形态 `section.contains(reason)` 逐字命中，零截断零压平（越界值不 panic、本层不设第二道长度门） | 参数化迭代（7 形态） |
| `phase_next` | 边界 | 体积界（AC-13）：`MAX_BACKTRACK_WINDOWS` 条窗口 × `MAX_REASON_CHARS`(1000) 字符原因 → 窗口段长度 ≤ `MAX_BACKTRACK_WINDOWS × (标签与说明行上界 + MAX_REASON_CHARS)`（界用两常量算式表达，零字面量 `2` / `1000` 的业务语义）；每原因仍逐字在场（证实现零 `truncate` / `chars().take(…)`） | 单次 |
| `phase_next` | 边界 | 上限条数与常量同源（AC-9）：构造 `MAX_BACKTRACK_WINDOWS + 1` 与 `+2` 条命中 → 响应条数恒 `== super::phase_table::MAX_BACKTRACK_WINDOWS`（断言引用常量而非字面量；orchestration 侧取不到该常量，故该断言只允许出现在本 crate 内） | 参数化迭代（2 形态） |
| `phase_next` | 边界 | id 退化形态（装置口径的显式登记）：全部条目 `id == 0`（既有 `entry` 工厂的原样输出）→ 判据不 panic、不产越界索引；此时「发起相位无 `id > 0` 的条目」对多条标记行可能同时成立，故断言只落在「稳定且可重复」而非具体条数（真实 store 的 `max+1` 保证 id 互异，见上「装置适配」；本行防「装置退化解被当成契约」） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ChangeStateStore`（`RouteStore` 手写假件） | 只实现 `get_change` / `list_phase_records`（可注入 `StoreFault`），其余方法 `unimplemented!("本用例不可达")` 的结构性执法——store 是**进程边界 + `phase_next` 的显式注入依赖**，故可 mock | 全章节 |
| 不 mock：`phase_table()` 与两张相位表 | 真实相位表——`col` / `to_index` / `from_index` 的序真值源；替身化会让 AC-2「区间判据单源」与 `detail.pipeline` 同源断言自证 | 全章节（尤其索引边界 / 嵌套 / 三层行） |
| 不 mock：`open_backtrack_windows` / `backtrack_window_section` | 同模块内真值直调（推导一次、渲染一次），二者同时是被测对象与装置 | 全章节 |
| 不 mock：`SessionAnchors` | 真实实例；跨 run 行新建**第二个**实例以证判据不读 anchor（第二实例的 `get_or_create` 基线 = 当时条目数 ⇒ `round` 归 1） | 跨 run 行、预算层口径行 |
| 不 mock：时钟 / 环境变量 | 条目 `timestamp` 全为 `t(n)` 字面量（`T0` + n 秒），零 `SystemTime` 依赖、零假时钟 | 全章节 |
| 不 mock：`Verdict` / `ChecklistItem` / `PhaseStateRecord` | 真实领域类型直构（结构体字面量按需覆写；`backtrack_entry` / `entry_of` 工厂负责形态差异） | 全章节 |

### `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`

既有 sidecar（3176 行 / 65 用例）**追加常量锚定用例组**；其余用例（两表相位序 / 前置依赖 / 白名单 / `include_str!` 装配面）零改动，作为「新增常量零触碰既有内容」的回归登记。**映射口径说明**：proposal「测试文件」表未列本文件（其常量断言本可寄居 `phase_next_test.rs`），本设计按 sidecar 惯例落在本文件——`MAX_RETRY_TIMES` / `MAX_ROUNDS` / `MAX_REASON_CHARS` 一类写面常量的既有锚定断言全在此处（「重试上限常量锚定为5」先例），AC-9 的常量锚定与既有常量同址才不出现「常量分散两处验收」。

#### 待测功能

- `MAX_BACKTRACK_WINDOWS`: `pub(crate) const MAX_BACKTRACK_WINDOWS: usize = 2;`——回溯窗口层级上限单点，与 `MAX_REASON_CHARS` / `MAX_REPORT_CHARS` / `MAX_ROUNDS` 同区；写面推导的裁剪量由它驱动，prompt 文案与测试断言引用它、零写死数字。

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无 phase_table.rs 行）；本条目取自 design「修改文件」表该文件行与两份 spec 的 Module Contract，行为面经 phase_next_test 的裁剪条数断言与 prompt_test 的零二次裁剪断言观测。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `MAX_BACKTRACK_WINDOWS` | 正向 | 常量锚定（AC-4 / AC-9）：`assert_eq!(MAX_BACKTRACK_WINDOWS, 2)`（沿既有「重试上限常量锚定为 5」先例）；类型为 `usize`（参与 `saturating_sub` 与 `skip` 的算式） | 单次 |
| `MAX_BACKTRACK_WINDOWS` | 正向 | 新增常量零触碰既有常量与表体：`MAX_RETRY_TIMES == 5` / `MAX_ROUNDS == 20` / `MAX_REASON_CHARS == 1000` / `MAX_REPORT_CHARS == 2000` 逐值不变，`phase_table("requirement")` / `phase_table("test-only")` 的相位序与既有断言逐条相等 | 参数化迭代（4 常量 + 2 表） |
| `MAX_BACKTRACK_WINDOWS` | 边界 | 语义下界：`MAX_BACKTRACK_WINDOWS >= 1`（取 0 会让窗口段恒空、上限语义退化）且 `< phase_table("requirement").len()`（上限不越表内相位数，否则裁剪永不触发） | 单次 |
| `MAX_BACKTRACK_WINDOWS` | 边界 | 体积界算式可用（AC-13）：`MAX_BACKTRACK_WINDOWS * (标签与说明行上界 + MAX_REASON_CHARS)` ≤ 2400（≈2.1K 加余量）——用常量算式断言，零浮点面、零字面量 `2` | 单次 |
| `MAX_BACKTRACK_WINDOWS` | 异常 | 零写死数字的跨文件侧证：`phase_next_test` 的裁剪条数断言以 `MAX_BACKTRACK_WINDOWS` 表达（同 crate 可见）、orchestration 侧取不到该常量（`pub(crate)`）；源码 / 文案 grep 零命中归「不可测试项」 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为常量与 `'static` 相位表，无 IO、无注入依赖、无时钟；全部真值直取 | 全章节 |
| 不 mock：`phase_table()` | 真实静态表（表长边界行需要真实值，不得以假表替代） | 语义下界比较行 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs`

既有 sidecar（423 行）**追加字段面用例组**；`DecisionAction` 三动作封闭集 / `parse_decision` / `ensure_backtrack_allowed` / `MAX_REASON_CHARS = 500` 的既有用例零改动（回归登记即「零触碰」判据）。`base` 完整构造器（`:361`）补 `backtrack_window` 字段属编译闭合随动（不新增用例）。

#### 待测功能

- `DecisionInput.backtrack_window: Vec<workflow::write::BacktrackWindowEntry>`（新增第六面，置于 `candidates` 之后）: 透传面——值原样取自写面 error 响应，walker MUST NOT 自读 store、MUST NOT 自相位表推导；`Vec` 非 `Option`（空集即「无窗口」）。

<!-- design.md 未声明该文件的公共函数 / API 变更（该表无 decision.rs 行）；本条目取自 design「类型定义」表该文件行与两份 spec 的 Module Contract。行为面（透传与渲染）经 walker_test 与 prompt_test 章节覆盖，本文件章节只锁字段面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DecisionInput.backtrack_window` | 正向 | 字段面保真（AC-10）：构造含 2 条窗口的 `DecisionInput` → 逐字段等值（`from_phase` / `from_index` / `to_phase` / `to_index` / `reason`）、`PartialEq` 可辨、与既有五面（`phase` / `attempt` / `fail_checklist` / `fail_report` / `allowed` / `candidates`）并列且互不扰动 | 单次 |
| `DecisionInput.backtrack_window` | 正向 | spread 构造自动随动（AC-10 / R-9）：`..base_input()` / `..base.clone()` 的既有 19 处用例编译并全绿；两处完整构造器（`prompt_test.rs:124` 权威 fixture、`:610` 确定性用例；`decision_test.rs:361` 的 `base`）补字段后既有断言逐条不变 | 参数化迭代（既有站点全集） |
| `DecisionInput.backtrack_window` | 异常 | 空集与非空集可辨：`assert_ne!` 双向；字段不参与 `parse_decision` / `ensure_backtrack_allowed`（两函数既有用例零改动即证零语义耦合）；`eval_only` 等退役字段零残留 | 参数化迭代（2 形态） |
| `DecisionInput.backtrack_window` | 边界 | 集合三形态：`[]` / 单元素 / 1000 元素 → 可构造、可比较、不 panic；`None` 语义不可表达（`Vec` 非 `Option`，编译期事实，见「不可测试项」） | 参数化迭代（3 形态） |
| `DecisionInput.backtrack_window` | 边界 | 字段值边界：`from_index` / `to_index` 取 `0` 与 `usize::MAX`；`reason` 取 `""` / `"字".repeat(MAX_REASON_CHARS)`（本 crate 的 `decision::MAX_REASON_CHARS` = 500，与写面 1000 是两个常量，本层不裁决长度）/ `"a\n\0🚀"`；`from_phase` / `to_phase` 取 `""` 与超长串 → 逐字保真、不裁剪（截断归写面，本层纯数据载体） | 组合迭代（2 index × 3 reason） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面是纯数据结构（字段面 / `PartialEq`），无 IO、无端口、无时钟 | 全章节 |
| 不 mock：`DecisionAction` / `parse_decision` / `ensure_backtrack_allowed` | 真实实现（回归登记面）——替身化会让「新增字段零语义耦合」自证 | 异常行的耦合回归 |
| 不 mock：`workflow::write::BacktrackWindowEntry` | 真实类型直构（字段顺序与类型由跨 crate 编译强制） | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs`

既有 sidecar（802 行）**追加决策窗口段用例组**；`base_input()`（`:124`）与内联完整构造器（`:610`）补字段属编译闭合随动。五个既有决策用例（五段在场 / 失败报告段 / 白名单自注记 / 空白名单 / 确定性）与 archive 两步用例零改动，作为「空集分支与非窗口形态逐字节等同」的回归判据。

#### 待测功能

- `decision_prompt(input: &DecisionInput) -> String`（体内变化、签名不变）: 段序 = 开头句 → （窗口段）→ `## 失败相位报告` → 失败清单 → 白名单 → 候选报告 → 报告文件 → 决策协议；窗口段文案与渲染**只调写面单点** `backtrack_window_section`（零第二份文案 / 零第二份判据 / 零二次裁剪），空集整段省略。

<!-- design.md「公共函数 / API」表对本文件只声明上面一行（体内变化）；`evaluator_protocol` / `evaluator_prompt` / `verdict_fix_prompt` / `decision_fix_prompt` / `spec_sync_prompt` / `merge_conflict_prompt` 由 design 明令逐字不动，本设计只登记其零窗口段的回归断言，不列待测功能。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `decision_prompt` | 正向 | 多条按创建序渲染（AC-10 / spec「决策 prompt 携回溯重做窗口段」）：2 条窗口 → `## 回溯重做窗口` 在场、两条原因按入参序各自被 `<backtrack reason>` / `</backtrack>` 定界、块间空行、渲染块数 == 2 | 单次 |
| `decision_prompt` | 正向 | 段位置断言（AC-10）：`prompt.find("## 回溯重做窗口") < prompt.find("## 失败相位报告")`，且 `> prompt.find("需要你做出路由决策")`（段序最前、紧跟开头句） | 单次 |
| `decision_prompt` | 正向 | 文案单点跨 crate（AC-9）：prompt 中的窗口段片段与 `workflow::write::backtrack_window_section(&input.backtrack_window)` **逐字节相等**（orchestration 侧零第二份文案的机械判据） | 单次 |
| `decision_prompt` | 正向 | 其余段零扰动：失败清单 / 可回溯白名单 / 候选报告 / 失败相位报告 / 报告文件五段仍在场，协议段仍恰列 backtrack / stop / ask 三动作、零退役 `retry` 行；`fail_report` 原文与白名单逐行渲染（含「= 重做本相位」自注记两形态）与既有断言逐条相等 | 参数化迭代（既有 5 段 + 2 注记） |
| `decision_prompt` | 异常 | 空集整段省略（AC-10）：`backtrack_window == []` → prompt 零 `## 回溯重做窗口` 字样、零空标题残留；该形态下组装结果与「窗口段引入前」的组装逐字节等同（以既有五段 / 协议 / 确定性用例零改动全绿为侧证） | 单次 |
| `decision_prompt` | 异常 | 零二次裁剪（AC-10 / spec「条目数与双分支一致」）：直接构造 3 条窗口的 `DecisionInput` → 三条原因**全部**逐条渲染（块数 == 3）、零「另有 N 条」/「仅显示最近」类元信息（文本负向断言）——证明本侧只消费写面集合，上限裁剪唯一点在写面 | 单次 |
| `decision_prompt` | 异常 | 零相位 id 泄漏（AC-8）：fixture 的 `from_phase` / `to_phase` 取与白名单 / 候选段任何相位 id **均不重名**的可辨合成串（如 `"phase-from-x"` / `"phase-to-y"`）→ 二者在整篇 prompt 零命中（使「零命中」是可整篇断言的判据，而非只能局部断言） | 参数化迭代（2 形态：单条 / 多条） |
| `decision_prompt` | 边界 | 入参条数四形态（0 / 1 / 2 / 3）→ 渲染块数恒等于入参长度（1 条不做「退化一行」短式、3 条不被裁），段标题在场性 = 入参是否非空 | 参数化迭代（4 形态） |
| `decision_prompt` | 边界 | 原因原文保真（AC-8）：`""` / `"  R  "`（首尾空白）/ `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1000)`（写面 `MAX_REASON_CHARS` 上限；该常量为 workflow 侧 `pub(crate)`，本侧取不到，故以字面 1000 构造）→ 逐字在 prompt 中，不 trim 不截断不压平 | 参数化迭代（5 形态） |
| `decision_prompt` | 边界 | 信封插值与窗口段并存：`attempt` 取 `0` 与 `u32::MAX`、`phase` 取 `""` 与超长串 → 开头句插值不受窗口段影响、不 panic、段序稳定 | 参数化迭代（4 形态） |
| `spec_sync_prompt` / `merge_conflict_prompt` / `decision_fix_prompt` | 异常（零改动回归） | AC-14 行为侧证：archive 两步产物零 `## 回溯重做窗口`、零 `<backtrack reason>` 字样（archive 恒在窗口外，另一套组装面）；软拒纠正文案仍只重申目标 + 白名单一行、零窗口段 | 参数化迭代（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | `decision_prompt` 是纯函数（`format!` 单串），零 IO、零端口、零时钟、零环境依赖 | 全章节 |
| 不 mock：`workflow::write::backtrack_window_section` | 真实调用——「文案单点」判据本体；替身化会让 AC-9 的跨 crate 单点断言空转 | 文案单点行、泄漏行、条数行 |
| 不 mock：本文件私有件（`backtrack_option_head` / `backtrack_option_artifacts` / `backtrack_option_label`） | 真实组装（白名单逐行渲染是被测输出的一部分） | 其余段零扰动行 |
| 不 mock：`DecisionInput` fixture | 真实结构体直构（`base_input()` + 字段覆写）；窗口 fixture 用可辨合成串以支撑零泄漏断言 | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`

既有 sidecar（15862 行）**追加透传用例组**；五处装置（`route_with_round` `:931` / `done_outcome` `:952` / `max_retries_outcome` `:967` / `verdict_fail_outcome` `:992` / `archive_route_outcome` `:1770`）补 `backtrack_window: Vec::new()` 属编译闭合随动（装置语义保持「零窗口」基线，不新增用例）。既有决策输入用例（`fail_checklist` 只失败项 / 全 pass 清单不虚减 / station 缺席占位 / 快照失败 Terminal）零改动。

#### 待测功能

- `build_decision_input(detail: &ChangeDetail, fail_phase: &str, allowed: &[BacktrackOption], backtrack_window: &[BacktrackWindowEntry]) -> DecisionInput`（私有，新增第四形参）: `backtrack_window: backtrack_window.to_vec()` 原样透传；函数体内零 store 读取、零相位表推导（窗口来源唯一 = 写面 error 响应）。
- `resolve_deadlock(...)`（私有，调用点随行）: 以 `&next.backtrack_window` 传入 `build_decision_input`（不传整个 `&PhaseNextOutcome`，保持取数面窄）。

<!-- design.md 未声明该文件的公共函数 / API 变更（该表无 walker.rs 行）；上述两条取自 design「修改文件」表该文件行与 D9。design 另要求 `step_executor` 的 doc 注释由「回溯原因后缀全在写面」改为「回溯重做窗口段全在写面」——注释面零运行期路径，登记于「不可测试项」。零新 port / 零新 step 词 / 零新 route 规则由 AC-11 的零 diff 面守住。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `build_decision_input` / `resolve_deadlock` | 正向 | 透传（AC-10）：`FakeTools` 下发携 2 条窗口的 error outcome → 决策会话请求 prompt（既有 `decision_prompt_of(&requests)` 装置）含 `## 回溯重做窗口` 与两条原因；同时既有四面断言逐条保持（fail checklist 只失败项、`fail_report` 取失败条目 report 原文、allowed 逐行、candidates verdict + report） | 参数化迭代（`VerdictFail` / `MaxRetriesExceeded` 2 变体） |
| `build_decision_input` / `resolve_deadlock` | 正向 | 原样不改写（AC-10）：`next.backtrack_window` 构造 3 条 → 决策 prompt 渲 3 条且逐条等序等值于 `workflow::write::backtrack_window_section(&windows)` 对应片段（walker 不重排、不裁剪、不改写——生产不变式「写面恒 ≤ 上限」的断言在 `phase_next_test`） | 单次 |
| `build_decision_input` / `resolve_deadlock` | 异常 | 空集基线（零窗口回归）：五处装置（`route_with_round` / `done_outcome` / `max_retries_outcome` / `verdict_fail_outcome` / `archive_route_outcome`）补空集后 → 既有 walker 决策用例零断言改动全绿，且决策 prompt 零 `## 回溯重做窗口`（空集分支与改动前逐字节等同） | 参数化迭代（5 装置） |
| `build_decision_input` / `resolve_deadlock` | 异常 | 取数面窄化（D9）：`last_result` 缺席 → `Terminal::failed("…缺 last_result 快照…")`、零决策请求（既有早退不受新形参影响）；快照 `detail` 读取 `Err` → `Terminal`（既有；窗口字段零参与） | 参数化迭代（2 形态） |
| `build_decision_input` / `resolve_deadlock` | 边界 | 段序（AC-1 / spec「段序窗口段先于反馈段」）：写面 outcome 的 `executor.prompt`（含窗口段 + 反馈段）直出 → 序为「上下文头 → 主体 → 窗口段 → 反馈段」；evaluator 经 `evaluator_prompt` append 协议 → `find("窗口段") < find("⚠️ 上次评估未通过") < find("## 输出协议")` 三者同时在场 | 单次 |
| `build_decision_input` / `resolve_deadlock` | 边界 | 窗内相位同时带 fail 反馈 + 窗口段并存：`max_retries_outcome` 携非空窗口 → 决策 prompt 的窗口段仍在 `## 失败相位报告` 之前、失败明细段内容零变化（walker 零重排） | 单次 |
| 组合：写面 → walker 决策链（入口 = 本文件，执行文件 `walker_test.rs`） | 组合 | 端到端：`TestDb` 种子回溯落账 + 真实 `StoreSnapshot` + 携该窗口的 error outcome → 决策会话请求 prompt 同时满足「窗口段在场且条数 == 写面响应 `backtrack_window.len()`」与「零相位 id 泄漏」；与 `prompt_test` 的单元行构成同一契约的两级判据（本行证明链路接线，不重复断言段内形状） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorkerAgentPort` / `ToolStepPort`（`FakeWorker` / `FakeTools`） | 既有手写假件：按调用序弹出预录产出（`with_phase_next` / `with_decision_reports`），并捕获 `WorkerTurnRequest` 供 prompt 断言——二者是**进程边界**（agent 会话 / 工具步），且是 `walk_run` 的显式注入依赖 | 全章节 |
| `WorkflowSnapshotPort` | 既有 `StubSnapshot`（用例不消费 detail 时）/ 真实 `StoreSnapshot` + `TestDb`（组合行需真实 detail）——快照是 DB 边界 | 全章节 |
| 不 mock：`decision_prompt` / `evaluator_prompt` | 真实组装——段序与文案单点是被测契约本体；替身化会让 AC-10 的段位置断言自证 | 透传行、段序行、组合行 |
| 不 mock：`workflow::write::backtrack_window_section` | 真实调用（跨 crate 文案单点判据） | 原样不改写行、组合行 |
| 不 mock：`DecisionInput` / `PhaseNextOutcome` 装置结构 | 真实结构体直构（字段补齐由编译器强制） | 全章节 |
| 不 mock：`RunGuard` / `ChangeFlowControl` / 时间线 | 真实实例 + 既有 `timeline` 记录（事件序断言面） | 全章节 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`（再导出两行：`BacktrackWindowEntry` + `backtrack_window_section`） | 纯 `pub use` 出口面，零运行期行为；design 明令零新 `mod` 行 / 零新 `#[cfg(test)] mod` 行，故无独立 sidecar | 跨 crate 编译侧证（`prompt.rs` 的 `use workflow::write::backtrack_window_section;` 与 `decision.rs` 的 `BacktrackWindowEntry` 引用即出口面接线判据）+ `prompt_test` 的文案单点逐字节相等行 |
| `packages/desktop/package.json`（`version` 0.6.31 → 0.6.32） | 配置 / 元数据面，无运行期行为可断言；判据是**相对递增**（当前 `HEAD` 值 +1 patch，MUST NOT 写死字面号） | AC-12 的 diff 审查：`package.json` 恰一行变更、`tauri.conf.json` 仍为 `"version": "../package.json"`、`src-tauri/Cargo.toml` crate 版本行未动 |
| AC-2 的**源码零特判**（`if to == from` 在产品 `.rs` 零命中）与 AC-3 / AC-7 的**判据不读运行期状态**（`open_backtrack_windows` 的签名与函数体零 `SessionAnchors` / `run_id` / `anchor` / `entries[anchor..]`） | 源码位置事实，运行期不可观测（用例只能覆盖被驱动的路径，覆盖不到「某分支不存在」这类**缺席**事实） | 桌面端静态检查相位执行 grep 判据；运行期侧证 = 自环行（与常规回溯同落点、输出同构）+ 跨 run 行（新 anchor + `round == 1` + 窗口在场：若判据切片 `entries[anchor..]` 必得空窗口而判红） |
| AC-7 的**S2 静态零残留**（`⚠️ 回溯原因` / `reason_suffix` / `build_phase_response` 的 `backtrack_reason` 形参在**产品** `.rs` 零命中；命中仅允许出现在 `*_test.rs` 的改写前状态） | 静态检索；写成测试会与桌面端静态检查重复且随排版漂移 | `grep` 判据由桌面端执行（tasks.md 阶段三自查项）；运行期侧证 = `phase_next_test` 的改写断言 + 反向「prompt 零 `⚠️ 回溯原因`」断言 |
| AC-13 的**零自行截断**（窗口渲染路径零新增 `truncate` / `chars().take(…)`）与 AC-9 的**零写死数字**（prompt 文案与测试断言零字面量 `2` 的业务语义） | 「不存在」的运行期断言只能退化为负向 grep | `grep` 判据由桌面端执行；运行期侧证 = 体积界行（1000 字符原因逐字在场）+ 上限条数行（断言引用 `MAX_BACKTRACK_WINDOWS`）+ prompt 章节的「零另有 N 条」文本负向断言 |
| 编译闭合随动的 **9 处结构体字面量**（`port_test.rs:27`；`walker_test.rs:931 / 952 / 967 / 992 / 1770`；`prompt_test.rs:124 / 610`；`decision_test.rs:361`） | 只补字段、只求可编译；design 明令**不新增用例、不改断言值、不改用例名**。缺字段即 E0063，无运行期中间态 | `cargo check --workspace --all-targets` 通过（`git diff --stat` 对这 4 个文件显示为纯新增行，每处 1 行）；装置语义「零窗口」基线由空集回归行侧证 |
| `walker.rs` 的 `step_executor` doc 同步（「回溯原因后缀」→「回溯重做窗口段」）与 `phase_next.rs` 的 `ResolvedPhaseSpec` / `build_phase_response` doc 同步 | 注释文本，零运行期路径 | 桌面端 diff 审查（doc 行变更随产品文件）；行为判据在段序与 S2 改写行 |
| `BacktrackWindowEntry` **不 derive `Serialize`** / `PhaseNextOutcome` 与 `DecisionInput` **不出 bindings** | 生成物与类型面事实；`bindings:export` 的零 diff 是「正确性主张」本身，在测试里读产物会把用例绑死在路径布局上 | `pnpm run bindings:export` 后 `git diff --stat -- src/types/generated` 为空（桌面端执行）；TS 侧零引用由 `knip` / `tsc` 覆盖 |
| `PhaseNextOutcome` 的 `impl TryFrom<ToolStepOutput>`（`walker.rs:3877`）为**拷贝解构**（`Ok(*outcome)`），无需改动 | 登记防误改的静态事实（design 事实校准 6）；新字段随 `Box` 载荷自动携带 | 桌面端 diff 审查该行零改动 + `cargo check` 通过（若误改为字面量构造必编译失败） |
| `phase_next.rs` 的私有件（`build_phase_response` / `error_outcome` 的新形参、`latest_entry` / `latest_phase_entry` / `latest_result` / `fail_feedback_suffix`）的**独立测试路径** | design 明令私有函数不列公共 API 表；`error_outcome` 的 `phase` 取自 `PhaseNextError` 的两变体（单点 `match` 取出）——直测需开导出 | 经 `phase_next` 公共面覆盖：error 两变体行（`backtrack_window` 逐字段断言）、正常响应行（覆盖 `next_phase`）、`done` 行（空集）、段位置行（反馈段与窗口段序） |
| 窗口推导的**中间元组排序键 `order = 标记行 id` 不落结构体** | 结构面缺席事实（`BacktrackWindowEntry` 五字段不含 `order`） | 结构体字段面由 `phase_next_test` 的逐字段断言与编译期字面量（缺字段即失败）双向守住；顺序语义由求值顺序行侧证 |
| AC-11 的**零结构扩散**（`crates/infra/store/**` 零 schema / 迁移 / 列 / 写事务改动；`write/backtrack.rs`、`orchestration/src/{port.rs,steps.rs,control.rs,state.rs}` 逐字不变；零新 port / step 词 / 通知类型） | 「正确性主张」是这些文件一行不变，运行期不可观测 | `git diff --stat` 逐文件审查（桌面端执行）；行为侧证 = 既有 `phase_next_test` / `backtrack_test` / `walker_test` 用例零断言改动即全绿（既有基线 7429 条零回归） |
| AC-12 的**全量门禁**（`cargo clippy --all-targets -D warnings` / `cargo test` 全绿 / `vp check` / `knip`） | 机械门禁步骤，由桌面端静态检查与 test-execution 相位执行；在设计里复述脚本细节等于复制运行脚本 | 门禁执行结果 + 本设计各章节对覆盖率的支撑（断言全部落在真实契约面：三判据 / 双分支 / 段序 / 原文保真，不为覆盖率造句） |
| AC-14 的**零触碰 diff 面**（`crates/core/workflow/src/prompts/**` 14 份 md、`packages/desktop/src/**` 含 `views/changes/flow/graph.ts`、`src/types/generated/**`、`openspec/specs/**`、`.github/workflows/**`、`tauri.conf.json`、`src-tauri/Cargo.toml` 版本行） | 「正确性主张」是这些文件零 diff | `git diff --stat` 逐项审查（桌面端执行）；行为侧证 = `prompt_test` 的 archive 两步零窗口段行 + `phase_table_test` 的 14 条 `include_str!` 逐字一致用例全绿 |
| AC-15 的**spec 双分支文本面**（两份 delta 各写一处双分支；`grep "最小"` 仅允许出现在「出线 / 最近创建」语境） | 文档文本面，无运行期行为 | 桌面端 spec 逐条比对 + `grep` 判据；本设计的等价链断言（三层裁剪行断「`from_index` 最大者不出线、末位两条出线」）把方向钉成运行期可判形态 |
| `MAX_BACKTRACK_WINDOWS` 的 `pub(crate)` 可见性纪律（orchestration 侧取不到该常量） | 编译期事实：orchestration 无法引用即无法写第二份裁剪分支 | `cargo check` 通过（若 orchestration 侧引用必编译失败）+ 零引用 grep（桌面端执行）；行为侧证 = `prompt_test` 的 3 条入参逐条渲染行（本侧零裁剪） |
