# 任务: backtrack-window-feedback-overlap

> **覆盖口径**：覆盖 proposal「变更范围 - 实现文件」**全量 2 行 / 2 个文件**（`crates/core/workflow/src/write/phase_next.rs`、`packages/desktop/package.json`）。proposal「测试文件」表归**测试相位**——本文不写用例、不跑测试、不为任何测试文件登记任务；其守护力缺口（`phase_next_test.rs:2406` 现只断窗口段在场）已登记在 design「变更清单」的随动面与 AC-1 / AC-7 回应行。
>
> **不在本 change**：全局删反馈段 / 区间上界开区间 / `reason` 信息分工 / 文案澄清；决策 prompt 的 `report` × `checklist` × 候选报告三重面去重；`latest_phase_entry` 双算与 `stale` 过滤等四项顺带清理（proposal R-6 / R-8）；`crates/core/orchestration/**`、`crates/infra/store/**`、`crates/core/workflow/src/prompts/**`、前端与生成物零改动。
>
> **静态步骤归属**：`pnpm check` 链（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `pnpm test` / `cargo test` 由桌面端 static-check / test-execution 执行，本文不重复。`bindings:export` / `api:export` 属**生成步骤**（产物须入库），按阶段列出；本变更预期零 diff。
>
> **阶段收口即可编译**：阶段零只读核对；阶段一只动 `phase_next.rs`（+4 行代码 + 两处 doc）；阶段二只动 `package.json` 一行；阶段三零实现改动（只核生成物与改动集）——每阶段收口时一次提交。
>
> 自查 cwd：仓库根；Rust 侧 `packages/desktop/src-tauri`；前端 / 生成物 `packages/desktop`。

## 阶段零：改动前核对（只读，不写文件）

- [x] 核对 `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` 的 `build_phase_response`（`:465`）：`:475` `let open_windows = open_backtrack_windows(entries, table, def.id);` → `:476` `let window_section = backtrack_window_section(&open_windows);` → `:477` `let feedback_suffix = retry_feedback.map(fail_feedback_suffix).unwrap_or_default();` → `:479-482` 闭包内 `format!("{context_header}{}{window_section}{feedback_suffix}", spec.prompt)`；与 design「起点实况」逐行一致（不一致即停：按实际行号调整再动手，MUST NOT 顺手改无关行）
- [x] 核对三处零改动基线就位：`:168` 回溯分支的 `build_phase_response(…, None)`、`:251-260` `retry_feedback` 取值、`:269` `error_outcome`；`:440` `fail_feedback_suffix` 与 `:419` `backtrack_window_section` 函数体
- [x] 核对 `packages/desktop/package.json:3` `"version"` = `0.7.9`；`packages/desktop/src-tauri/tauri.conf.json:4` = `"version": "../package.json"`；`packages/desktop/src-tauri/Cargo.toml:6` = `version = "0.1.0"`
- [x] 核对工作树：`git status --porcelain` 只有 `openspec/changes/backtrack-window-feedback-overlap/` 未跟踪（零预置实现改动）

## 阶段一：同源抑制单点（`crates/core/workflow/src/write/phase_next.rs`）

- [x] 在 `build_phase_response` 的 `let open_windows = …` 之后新增判据：`let same_origin = open_windows.iter().any(|window| window.from_phase == def.id);`——形态与 design D1 逐字一致；MUST NOT 比较记录 `id` / `timestamp`、MUST NOT 新增 `marker_id` 类字段、MUST NOT 挪到 `phase_next` 调用侧
- [x] 同处加一行注释说明非显见理由：同源格下窗口段与失败反馈段读的是同一条标记行（`reason` 是决策者的修法方向、`checklist` / `report` 是评估者旧输出的回灌），故只留窗口段
- [x] `feedback_suffix` 改为经抑制闸再渲染：`let feedback_suffix = retry_feedback.filter(|_| !same_origin).map(fail_feedback_suffix).unwrap_or_default();`——`retry_feedback` 形参与 `fail_feedback_suffix` 渲染函数**保留**（窗口外格仍是其消费者），MUST NOT 改成 `is_none()` / 哨兵值 / 「同源与 `None` 合并」的写法（design D6）
- [x] `build_phase_response` 的 doc（`:462-464`）同步：「回溯重做窗口段（静态主体之后、失败反馈段之前）+ 失败反馈段（fail 重试且**非同源格**时在场——同源格整段省略）+ 白名单随行（append 组装，零模板替换）」
- [x] `fail_feedback_suffix` 的 doc（`:437-439`）补一句：消费者为「窗口内非同源格的 fail 重试」与「窗口外格的 fail 重启」；同源格由 `build_phase_response` 整段省略，本函数零判据
- [x] 自查 diff 面（`D=packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs`）：`git diff -U0 -- $D | grep -c '^@@'` ≤ 4 且逐 hunk 能对上 design「修改文件」表的四点之一（多点、无对应即回退）；`git diff -U0 -- $D | grep -c '^-[^-]'` ≤ 4（只允许 `fail_feedback_suffix` doc 的既有行、`build_phase_response` doc 的两行、`let feedback_suffix = retry_feedback.map(fail_feedback_suffix).unwrap_or_default();` 一行被替换）；`open_backtrack_windows` / `backtrack_window_section` / `fail_feedback_suffix` 函数体 / `phase_next` / `error_outcome` / `latest_phase_entry` **零行变化**（删除行零 `fn ` / 三函数名命中）
- [x] 自查判据单源：`grep -c 'from_phase == def.id' …/phase_next.rs` = 1；`grep -c 'same_origin' …/phase_next.rs` = 2（定义 + `filter` 各一）；`grep -rn 'same_origin' packages/desktop/src-tauri/crates | grep -v 'phase_next\.rs'` 零命中；`grep -rn 'from_phase ==' packages/desktop/src-tauri/crates/core/orchestration` 零命中
- [x] 自查零特判：`grep -c 'if to == from' …/phase_next.rs` = 0、`grep -c 'if id == next' …/phase_next.rs` = 0（AC-5）

## 阶段二：版本（`packages/desktop/package.json`）

- [x] `"version"`: `0.7.9` → `0.7.10`（相对落地时 `HEAD` 恰 +1 patch）；`src-tauri/tauri.conf.json` 零 diff（仍 `"version": "../package.json"`）；`src-tauri/Cargo.toml` 的 `version = "0.1.0"` 逐字不变
- [x] 自查：`grep -n '"version"' packages/desktop/package.json` = `0.7.10`；`git diff -- packages/desktop/src-tauri/tauri.conf.json packages/desktop/src-tauri/Cargo.toml` 零输出

## 阶段三：生成物与零扩散核对（不重复执行桌面端静态步骤）

- [x] 执行 `pnpm -C packages/desktop run bindings:export`（生成步骤）：`git diff --stat -- packages/desktop/src/types/generated/` **零输出**（`PhaseNextOutcome` / `BacktrackWindowEntry` 不出 bindings，`bindings.ts` 与 `bindings.zod.ts` 均零 diff）；MUST NOT 手改生成物
- [x] 执行 `pnpm -C packages/desktop run api:check`：零差异退出（写面无契约元素，零新类型 / 零新导出 / 零新命令）
- [x] 零扩散核对：`git status --porcelain` 的改动集 ⊆ design D8 白名单 = 本 change 目录（`design.md` / `tasks.md` / `specs/**` / `proposal.md` / `explore.md` / `reports/**`）+ `crates/core/workflow/src/write/phase_next.rs` + `packages/desktop/package.json`；出现 `crates/core/orchestration/**`、`crates/infra/store/**`、`crates/core/workflow/src/write/{phase_table,mod,backtrack}.rs`、`crates/core/workflow/src/prompts/**`、`packages/desktop/src/**`、`src/types/generated/**`、`openspec/specs/**`、`openspec/changes/archive/**`、`AGENT.md` / `CLAUDE.md`、`src-tauri/Cargo.toml` 任一 diff 即回退
- [x] 验收对照：逐条回应 design「验收标准回应」表（AC-1…AC-12）——AC-2 / AC-5 / AC-8 / AC-9 / AC-10 / AC-11 / AC-12 由本文件阶段一 / 二 / 三的自查背书；AC-1 / AC-3 / AC-4 / AC-6 / AC-7 的断言面由测试相位承接（proposal「测试文件」表 7 行：`phase_next_test.rs:2406` 补否定断言 + 标记行正文零命中、`:2374` 补反馈段仍在、新增三态矩阵与「同源窗口过裁剪仍存活」两条，其余 3 行（`:2932` 段序 / `:3105` 零窗口回归 / 自环 · 跨 run · 两分支同形 · 失败项注入）逐字不动；`crates/core/orchestration/src/**` 零改动）
