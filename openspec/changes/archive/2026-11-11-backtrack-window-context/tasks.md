# 任务: backtrack-window-context

> **读法（本文件全局，先读这条）**：产品代码改动 **7 个文件**（`phase_next.rs` / `phase_table.rs` / `write/mod.rs` / `decision.rs` / `prompt.rs` / `walker.rs` / `package.json`），另加**测试文件的结构体字面量补字段 9 处**（编译闭合随动，不新增任何用例）。**零新增文件、零新增依赖、零 schema 迁移、零前端改动、零 bindings 改动。**
>
> **本相位不写测试、不执行测试**：`phase_next_test.rs` 的四态 / 自环 / 嵌套双窗 / 三层裁剪 / 内层回跳 / 跨 run / error 响应 / 原文保真 / 零相位 id / 求值顺序 / S2 断言改写，以及 `prompt_test.rs` 的决策窗口段五用例，全部归 test-design / test-gen / test-execution 相位。`cargo clippy` / `vp check` / `knip` / `bindings:export` 一律不由本相位外跑。
>
> **交付态的预期红灯（必读，防误判）**：S2 删掉 `reason_suffix` 后，`phase_next_test.rs` 的 4 处 `⚠️ 回溯原因` 断言（当时行号 `:718` / `:1035` / `:1084` / `:2098`）**在本相位末段必然失败**——它们断言的是已删除的文案，其改写归测试相位（design 事实校准 8）。故本相位的判据是**编译绿 + clippy 零新告警**，不是 `cargo test` 全绿。MUST NOT 为让门禁绿而改这些断言，亦 MUST NOT 为回避红而保留 `reason_suffix`。
>
> **后续相位已收口（回溯复检如实登记）**：测试相位已把这 4 处断言改写为窗口段断言（`## 回溯重做窗口` + `<backtrack reason>` + 原因文本），`reports/test/summary.json` 实测**全绿**（`total 7489` / `passed 7489` / `failed 0`）；故当前树内 `⚠️ 回溯原因` 的文字残留只剩「断言其**不出现**」的负向断言（`phase_next_test.rs:785` / `:1107` / `:1162` / `:3015` 与 `:2977` 的说明），断言方向与本相位「必须删除」的裁定一致，非隐藏回退。
>
> **唯一例外（编译闭合，逐条登记）**：`PhaseNextOutcome` / `DecisionInput` 新增字段后，结构体字面量不给全字段即 E0063 ⇒ 下表 9 处**只补字段、只求可编译**，不改断言值、不改用例名、不新增 / 删除用例。落点见阶段五 / 阶段六。
>
> 自查命令 cwd 除特别标注外均为 `packages/desktop/src-tauri`；版本自查 cwd 为 `packages/desktop`；git 自查 cwd 为仓库根。

---

## 阶段一：上限常量（解锁阶段二）

- [x] 在 `crates/core/workflow/src/write/phase_table.rs` 的写面常量区（`MAX_REASON_CHARS`（`:46`）之后、`REQUIREMENT_WORKFLOW_TYPE`（`:49`）之前）新增 `pub(crate) const MAX_BACKTRACK_WINDOWS: usize = 2;`，doc 写明：回溯窗口层级上限（命中 ≤ 本上限时全量并集、超限取创建序末位两层），与 `MAX_REASON_CHARS` / `MAX_REPORT_CHARS` / `MAX_ROUNDS` 同区单点，prompt 文案与测试断言引用本常量、零写死数字
- [x] 自查 阶段一：`cargo check -p workflow`（cwd `packages/desktop/src-tauri`）通过；`grep -c "MAX_BACKTRACK_WINDOWS" crates/core/workflow/src/write/phase_table.rs` 为 `1`（仅 const 定义行——doc 内不复述常量名，避免第二处字面量）；`cargo clippy -p workflow` 零新告警（`pub(crate)` 常量暂未被消费时的 dead_code 告警属预期，阶段二消费后消失）

## 阶段二：写面的推导与渲染单点（解锁阶段三、四）

- [x] 在 `crates/core/workflow/src/write/phase_next.rs` 的 import 区（`:8-11`）把 `MAX_BACKTRACK_WINDOWS` 加入 `use crate::write::phase_table::{…}` 列表
- [x] 新增 `pub struct BacktrackWindowEntry`（置于 `ResolvedPhaseSpec`（`:63-70`）之后、`PhaseNextOutcome`（`:75`）之前）：五字段 `pub from_phase: String` / `pub from_index: usize` / `pub to_phase: String` / `pub to_index: usize` / `pub reason: String`，derive `Debug, Clone, PartialEq, Eq`，**不 derive `Serialize`**；struct doc 写明「一次未闭环回溯 = 一个窗口；结构面服务判据 / 堆叠 / 裁剪 / 测试断言，`from_phase` / `to_phase` / 两个 index 一律不进 prompt（裁决 16）」，逐字段 doc 写明语义与「进不进 prompt」
- [x] 新增 `pub(crate) fn open_backtrack_windows(entries: &[PhaseStateRecord], table: &[PhaseDefinition], phase_id: &str) -> Vec<BacktrackWindowEntry>`（置于 `latest_result`（`:302-309`）之后、`fail_feedback_suffix`（`:314`）之前），doc 写明三条判据与两层口径（预算层按 run / 背景层按全量事件序）、入参零 `SessionAnchors`
- [x] 函数体按固定四步实现（顺序即契约）：① 枚举候选标记行——`entries` 中 `backtrack_to` 经 `trim` 后非空的条目，取 `from = entry.phase` / `to = backtrack_to.trim()` / `order = entry.id` / `reason = backtrack_reason.clone().unwrap_or_default()` 组成中间元组，`from` / `to` 任一不在 `table` 内则跳过该候选（防御式，不 `Err`）；② `filter(open)`——发起相位不存在 `entry.phase == from && entry.id > order` 的条目；③ `filter(connect)`——`to_idx ≤ col(phase_id) ≤ from_idx`，`col` 用 `table.iter().position(|def| def.id == pid)`，`phase_id` 不在表内则直接返回空集；④ `sort_by_key(order)` 后 `skip(len.saturating_sub(MAX_BACKTRACK_WINDOWS))`，最后投影成 `BacktrackWindowEntry`（`order` 是中间元组字段，**不落结构体**）
- [x] 实现约束（逐条 MUST NOT）：零 `if to == from` 特判；零 `entries[anchor..]` 切片、零 `run_id` / `SessionAnchors` 入参或函数体引用；零 `truncate` / `chars().take(…)`；「先裁剪后排序」的写法错序（必须 `sort` 在 `skip` 之前）
- [x] 新增 `pub fn backtrack_window_section(windows: &[BacktrackWindowEntry]) -> String`（紧跟推导 fn），doc 写明「文案与渲染单点，经 `write/mod.rs` 再导出供 orchestration 复用；只取 `reason`，零属性、零相位 id、不截断不压平不 trim；前导 `\n\n` 归段内、尾随零换行」
- [x] 渲染实现：空切片返回 `String::new()`；非空时以 `"\n\n## 回溯重做窗口（背景信息）\n\n"` 起首，接说明段（单段行文：「本 change 发生过下列相位回退，且均尚未回到闭环。这些是此前回溯留下的、需要解决的问题——原因原文按发生先后排列（最上面最早），保留原始换行与标点，未做截断或改写。它们是背景，不构成额外验收条件。」），再对每个窗口追加 `"\n\n<backtrack reason>\n" + &window.reason + "\n</backtrack>"`。**只读 `window.reason`**，其余四字段零引用
- [x] 自查 阶段二：`cargo check -p workflow` 通过；`grep -n "from_phase\|to_phase" crates/core/workflow/src/write/phase_next.rs` 全部命中的是 struct 定义 / doc / 推导（排序与判据）而非渲染函数体（渲染函数体内零命中）；`grep -n "serde" crates/core/workflow/src/write/phase_next.rs` 零命中

## 阶段三：写面的响应填充与 S2 删除（依赖阶段二）

- [x] `PhaseNextOutcome`（`:75-92`）在 `evaluator` 与 `allowed_backtrack_phases` 之间增 `pub backtrack_window: Vec<BacktrackWindowEntry>`，doc 写明：正常响应填覆盖 `next_phase` 的开放窗口、error 响应填覆盖失败相位的开放窗口（决策输入取此处）；创建序升序（最早在前）；`Vec` 非 `Option`，空集即「无窗口」，`Default` 语义与「整段省略」同义
- [x] `error_outcome`（`:239-256`）签名改为 `fn error_outcome(entries: &[PhaseStateRecord], table: &'static [PhaseDefinition], round: u32, last_result: Option<LastResult>, error: PhaseNextError) -> PhaseNextOutcome`；体内以 `match &error` 的 `MaxRetriesExceeded { phase, .. } | VerdictFail { phase, .. }` 两臂单点取出失败相位（不另开形参，与 `resolve_deadlock` 既有同款手法一致），响应字面量加 `backtrack_window: open_backtrack_windows(entries, table, phase)`
- [x] 两处 `error_outcome` 调用点（`:194`（`MaxRetriesExceeded`）/ `:215`（`VerdictFail`））按新签名传 `&entries` / `table`
- [x] `build_phase_response`（`:339-374`）签名**删** `backtrack_reason: Option<&str>`、**增** `entries: &[PhaseStateRecord]`（建议置于 `table` 之后，保持 `entries` / `table` 相邻）；doc 删「回溯原因后缀」改「回溯重做窗口段（静态主体之后、失败反馈段之前）」
- [x] `build_phase_response` 体内：删 `let reason_suffix = …`（`:349-351` 整段），改为两行（推导一次、渲染一次，响应字面量末尾 move 入结构体）：`let backtrack_window = open_backtrack_windows(entries, table, def.id);` + `let window_section = backtrack_window_section(&backtrack_window);`；拼接式（`:353-357`）改为 `format!("{context_header}{}{window_section}{feedback_suffix}", spec.prompt)`；响应字面量（`:360-373`）加 `backtrack_window`
- [x] 两处 `build_phase_response` 调用点按新签名改写：backtrack 分支（`:146-154`）删实参 `entry.backtrack_reason.as_deref()`、补 `&entries`；正常路由分支（`:226-234`）删实参 `None`、补 `&entries`——两分支实参序列收敛为同一形（只差 `retry_feedback`）
- [x] `done` 响应（`:164-175`）补 `backtrack_window: Vec::new()`
- [x] doc 同步：`ResolvedPhaseSpec`（`:63-65`）「失败反馈段 / 回溯原因行（append）」→「失败反馈段 / 回溯重做窗口段（append）」
- [x] 自查 阶段三（S2 零残留）：`grep -rn "回溯原因\|reason_suffix" crates/core/workflow/src crates/core/orchestration/src` 在产品 `.rs` 内**零命中**（`phase_next_test.rs` 的 4 处断言与 8 处相邻说明按 proposal「测试文件」表归测试相位改写，本相位**不动测试文件**）；`grep -rn "⚠️ 回溯原因" packages/desktop/src-tauri/crates/core/workflow/src packages/desktop/src-tauri/crates/core/orchestration/src` 同理（命中仅允许出现在 `*_test.rs`）
- [x] 自查 阶段三：`cargo check -p workflow` 通过；`grep -c "backtrack_window:" crates/core/workflow/src/write/phase_next.rs` 为 `4`（1 处 struct 字段声明 + done / error_outcome / build_phase_response 三处响应字面量）；`grep -c "BacktrackWindowEntry {" crates/core/workflow/src/write/phase_next.rs` 为 `2`（struct 定义行 + 推导 fn 内唯一构造点——超过 2 即说明出现了第二处构造点）

## 阶段四：出口面扩项（依赖阶段二、三）

- [x] `crates/core/workflow/src/write/mod.rs` 的 `pub use phase_next::{…}`（`:21-23`）追加 `BacktrackWindowEntry` 与 `backtrack_window_section`（单个 `pub use`，零新 `mod` 行、零新 `#[cfg(test)] mod` 行）
- [x] 自查 阶段四：`cargo check -p workflow` 通过；`cargo doc -p workflow --no-deps` 无 broken intra-doc link 告警；`grep -n "BacktrackWindowEntry\|backtrack_window_section" crates/core/workflow/src/write/mod.rs` 恰 1 行命中（同一 `pub use` 行内两标识符）

## 阶段五：决策面消费（依赖阶段四；与阶段六同批闭合）

- [x] `crates/core/orchestration/src/decision.rs` 的 `use workflow::write::{BacktrackOption};`（`:3`）扩为 `{BacktrackOption, BacktrackWindowEntry}`
- [x] `DecisionInput`（`:31-49`）在 `candidates` 之后增 `pub backtrack_window: Vec<BacktrackWindowEntry>`，字段 doc 写明「回溯重做窗口集合，透传自写面 error 响应；walker MUST NOT 自读 store、MUST NOT 自相位表二次推导；判据与渲染单源 = 写面『回溯重做窗口推导与 prompt 携带』」
- [x] `crates/core/orchestration/src/prompt.rs` 的 `use workflow::write::BacktrackOption;`（`:1`）扩为 `{backtrack_window_section, BacktrackOption}`
- [x] `decision_prompt`（`:75-137`）体内新增 `let window_section = backtrack_window_section(&input.backtrack_window);`；`format!`（`:120-136`）在开头句（`:121`）与 `## 失败相位报告`（`:122`）之间插入 `{window_section}`——写成 `"相位「{phase}」第 {attempt} 次 attempt 评估失败，需要你做出路由决策。{window_section}\n\n## 失败相位报告\n\n{fail_report}\n\n…"`，使空集时（`""`）与现状逐字节相等
- [x] `decision_prompt` doc（`:72-74`）的有界输入清单补「回溯重做窗口（透传自写面 error 响应）」一项
- [x] `crates/core/orchestration/src/walker.rs` 的 `use workflow::write::{…}`（`:8-13`）加 `BacktrackWindowEntry`
- [x] `build_decision_input`（`:3550-3606`）签名增第四形参 `backtrack_window: &[BacktrackWindowEntry]`；`DecisionInput` 字面量（`:3600-3606`）填 `backtrack_window: backtrack_window.to_vec()`；fn doc 补该面来源（原样取自写面 error 响应，零 store 读取、零相位表推导）
- [x] `resolve_deadlock`（`:3302`）内 `build_decision_input(&detail, fail_phase, allowed)`（`:3328`）改为传 `&next.backtrack_window`（**不传整 `&next`**，保持取数面窄）
- [x] `step_executor` doc（`:791-792`）「上下文头 + 失败反馈段 + 回溯原因后缀全在写面」→「上下文头 + 失败反馈段 + 回溯重做窗口段全在写面」
- [x] 决策面约束（逐条 MUST NOT）：orchestration 侧零第二份文案（只调 `backtrack_window_section`）、零第二份判据、零二次裁剪（不得引用 / 重定义上限常量——`MAX_BACKTRACK_WINDOWS` 为 `pub(crate)`，跨 crate 取不到即结构性保障）、零 `from_phase` / `to_phase` 渲染
- [x] 自查 阶段五（源码判据）：`grep -rn "回溯重做窗口（背景信息）" crates/core/workflow/src crates/core/orchestration/src` 在**产品 `.rs`** 内恰 1 处命中（`phase_next.rs` 渲染 fn 内的标题字面量）——orchestration 侧零副本（唯一第二处命中是测试相位后来新增的 `phase_next_test.rs:3384` `section.contains(...)` 断言串，非第二份文案）；`grep -rn "MAX_BACKTRACK_WINDOWS" crates/core/orchestration/src` 零命中；`grep -n "backtrack_window" crates/core/orchestration/src/walker.rs` 命中的是 import / 形参 / 透传三处，`build_decision_input` 函数体内零 store 方法调用

## 阶段六：编译闭合随动（依赖阶段三、五；只补字段，不写用例）

- [x] `crates/core/orchestration/src/port_test.rs` 的 `sample_phase_next_outcome()`（`:27`）补 `backtrack_window: Vec::new()`
- [x] `crates/core/orchestration/src/walker_test.rs` 的 5 处 `PhaseNextOutcome` 字面量补同一字段：`route_with_round`（`:931`）/ `done_outcome`（`:952`）/ `max_retries_outcome`（`:967`）/ `verdict_fail_outcome`（`:992`）/ `archive_route_outcome`（`:1770`）
- [x] `crates/core/orchestration/src/prompt_test.rs` 的 2 处 `DecisionInput` 完整构造器补 `backtrack_window: Vec::new()`：`base_input()`（`:124`）、确定性用例内联构造（`:610`）——其余 19 处 `..base_input()` / `..base` spread 自动随动，MUST NOT 逐个改写
- [x] `crates/core/orchestration/src/decision_test.rs` 的 1 处 `DecisionInput` 完整构造器（`:361` 的 `base`）补同一字段
- [x] 阶段六约束：**只补字段**——零断言值改动、零用例名改动、零新增 / 删除用例、零装置语义改动（`route_with_round` / 决策两产出装置的窗口集合留空集，窗口透传用例归测试相位新增）
- [x] 自查 阶段六：`cargo check --workspace --all-targets`（cwd `packages/desktop/src-tauri`）通过（E0063 全消）；`git diff --stat` 对这 4 个测试文件**在本相位交付时**显示为**纯新增行**（每处 1 行字段补齐）。**后续相位态**：测试相位在同一批测试文件上追加了断言改写与新增用例（`phase_next_test.rs` / `phase_table_test.rs` 亦有改动），故当前树内这 4 个文件的 diff 不再是纯新增——非本相位范围，登记以免误判为「装置语义被改」

## 阶段七：版本与交付面自查（依赖阶段六）

- [x] `packages/desktop/package.json` 的 `version` **相对递增一次 patch**（**MUST NOT 写死**）；`dependencies` / `devDependencies` / `scripts` 零改动。**实测落地值**：`0.6.32 → 0.6.33`——`HEAD` 已由本相位起始时的 `52f32a8`（`version = 0.6.31`，见 design 起点实况）推进到 `ab9c931`，该提交随无关 change 把基线抬到 `0.6.32`，故本变更的「`HEAD` 值 +1 patch」落在 `0.6.33`（`git diff packages/desktop/package.json` 仅版本一行）
- [x] 自查 阶段七（模块契约零 diff，cwd 仓库根）：`git diff --stat .dev-team/architecture/` **零输出**（新增两个 `pub` 项落在 `App.Backend.Core.Workflow`，该元素现状 `status = absent`、`apiUndeclared` 档位 `off` ⇒ 不引入新 finding，见 design 事实校准 9）；`git diff --stat packages/desktop/src/types/generated/` 零输出（`PhaseNextOutcome` / `DecisionInput` 不出 bindings）
- [x] 自查 阶段七（版本纪律）：`grep -n '"version"' packages/desktop/package.json` 恰 1 行且为 **`0.6.33`**（相对 `HEAD` 的 `0.6.32` 递增一次 patch；`HEAD` 已由 `52f32a8` 推进到 `ab9c931`，见上一条）；`grep -n '"version"' packages/desktop/src-tauri/tauri.conf.json` 仍为 `"../package.json"`；`git diff packages/desktop/src-tauri/Cargo.toml` **零输出**（crate 版本不随动）
- [x] 自查 阶段七（零 diff 面，cwd 仓库根）：下列命令**全部零输出**——`git diff --stat packages/desktop/src-tauri/crates/core/workflow/src/prompts/`、`git diff --stat packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs`、`git diff --stat packages/desktop/src-tauri/crates/infra/store/`、`git diff --stat packages/desktop/src-tauri/crates/core/orchestration/src/port.rs packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs packages/desktop/src-tauri/crates/core/orchestration/src/control.rs packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`、`git diff --stat packages/desktop/src/ packages/desktop/src/types/generated/`、`git diff --stat openspec/specs/`、`git diff --stat .github/workflows/`、`git diff --stat .dev-team/architecture/`。**已解决的历史前置条件（如实登记）**：`pnpm check` 链的 `client:check`（`vp check --fix`）曾顺带格式化 `packages/desktop/src/hooks/use-highlighted-code.test.ts`（当时仅 `:143-149` 数组字面量折行压成单行，语义零变化），该文件格式违规当时**在 `HEAD` 既已存在**、与本变更无关，故曾要求跑完门禁后 `git checkout --` 复原。**现状**：该格式化已被无关提交 `ab9c931` 收进 `HEAD` 基线，工作树副本与 `HEAD` blob 逐字节相同（`git hash-object` = `git rev-parse HEAD:…` = `193b860a8f3a`）、`vp fmt --check` 通过 ⇒ **不再需要任何复原动作**，门禁跑完后 `git diff --stat packages/desktop/src/` 仍为零输出（实测：跑完整 `vp check` 后零 diff）。该文件不在本变更范围、零改动；`.github/workflows/desktop-release.yml` 只跑 `pnpm install --frozen-lockfile` + `tauri-action`，不跑 `pnpm check` / `vp check`
- [x] 自查 阶段七（产品改动面收敛）：本相位交付时 `git status --porcelain -uall | grep -v '^.. openspec/changes/backtrack-window-context/'` 的文件集合为 7 个产品文件 + 4 个测试文件（`port_test.rs` / `walker_test.rs` / `prompt_test.rs` / `decision_test.rs`），零第 8 个产品文件、零新增文件。**测试相位落地后收敛为 7 产品 + 6 测试**（追加 `phase_next_test.rs` / `phase_table_test.rs` 的断言改写与新增用例，见下一条实测 13 行），产品面仍为零第 8 个、零新增文件
- [x] 复核 阶段七（格式化污染收尾，**最终状态**）：test-execution 末段 `pnpm check` 的 `client:check`（`vp check --fix`）曾把 `packages/desktop/src/hooks/use-highlighted-code.test.ts` 格式化（当时 `1 insertion(+), 6 deletions(-)`，仅 `:143-149` 数组字面量折行压成单行，语义零变化），本相位当时以 `git checkout -- packages/desktop/src/hooks/use-highlighted-code.test.ts` 单点复原。**该格式化已被无关提交 `ab9c931` 收进 `HEAD` 基线**，故现无需复原、也无可复原的差异。本次复核实测：`git hash-object packages/desktop/src/hooks/use-highlighted-code.test.ts` = `git rev-parse HEAD:packages/desktop/src/hooks/use-highlighted-code.test.ts` = `193b860a8f3a…`（逐字节相同），`git diff --quiet HEAD --` 该文件返回零差异，`vp fmt --check` 通过，跑完整 `vp check`（245 文件 fmt + 235 文件 lint/type）后 `git status --porcelain -uall -- packages/desktop/src` 仍为空。复核证据：`git diff --stat packages/desktop/src/` **零输出**（与 design.md:10 / proposal.md:275 的 `src/**` 零 diff 声明及 AC-14 一致）；`git status --porcelain -uall | grep -v '^.. openspec/changes/backtrack-window-context/'` 恰 13 行 = 7 个产品文件（`package.json` / `phase_next.rs` / `phase_table.rs` / `write/mod.rs` / `decision.rs` / `prompt.rs` / `walker.rs`）+ 6 个测试文件（`phase_next_test.rs` / `phase_table_test.rs` / `port_test.rs` / `walker_test.rs` / `prompt_test.rs` / `decision_test.rs`），零第 8 个产品文件、零 `src/` 残留（本次以 `git status` / `git diff --stat` / `git diff --quiet` / `git hash-object` / `grep` 复核并实跑 `vp check`，两者结论一致）
- [x] 自查 阶段七（文案单点，AC-9）：`grep -rn "回溯重做窗口" packages/desktop/src-tauri/crates/core/workflow/src` 命中的**文案字面量**只在 `write/phase_next.rs` 的渲染 fn 内（`phase_next.rs` 另 3 处为 doc 注释；测试相位在 `phase_next_test.rs` 内的命中是断言串，非第二份文案）；`grep -rn "回溯重做窗口" packages/desktop/src-tauri/crates/core/orchestration/src` 在产品 `.rs` 侧零文案字面量命中（`decision.rs` / `prompt.rs` / `walker.rs` 的命中全为 doc / 注释引用；`prompt_test.rs` / `walker_test.rs` 内为测试断言串）
- [x] 自查 阶段七（渲染面纪律，AC-8 / AC-13）：`grep -n "from_phase\|to_phase\|from_index\|to_index" packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` 的命中全部落在 struct 定义 / doc / 推导 fn 内，**渲染 fn 体内零命中**；`grep -n "truncate\|chars().take" packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` **零命中**
- [x] 自查 阶段七（口径分层，AC-3）：`grep -n "anchor\|run_id\|SessionAnchors" packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` 的命中全部落在 `SessionAnchors` 定义 / `phase_next` 函数体（既有预算层）/ doc，**推导 fn 内零命中**

## 收尾

- [x] 全量自查（**本相位只判编译与告警，不判 `cargo test` 全绿**）：`cargo clippy --workspace --all-targets -D warnings` 与 `cargo check --workspace --all-targets` 绿（cwd `packages/desktop/src-tauri`，本相位实测通过）。**`cargo test` 的相位态**见 design「事实校准 8」：S2 落地时 `phase_next_test.rs` 的 4 处 `⚠️ 回溯原因` 断言（当时行号 `:718` / `:1035` / `:1084` / `:2098`）必红，其改写归测试相位；本相位 MUST NOT 为「让门禁绿」而改这些断言，亦 MUST NOT 为回避红而保留 `reason_suffix`。**后续相位收口态**：测试相位已改写这 4 处，`reports/test/summary.json` 实测 `total 7489` / `passed 7489` / `failed 0`（全绿）
- [x] 复核设计-实现偏差：design「变更清单」的 7 个产品文件逐一对应实改；「编译闭合随动」表 9 处全部落地；若实现期出现设计未覆盖的必要改动，先回写 `design.md` 再落地（MUST NOT 在实现里暗增范围）
- [x] 复核明确不做项（AC-14）：archive 两步（`spec_sync_prompt` / `merge_conflict_prompt`）零窗口段；`graph.ts` / 前端 / 生成物零 diff；`backtrack.rs` 写面裁定逐字不变；store 零 schema / 零列 / 零迁移
