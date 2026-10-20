# AC-6 真件证据（工作树按目标命令实跑）

> 采集：2026-10-19 12:10–12:17（本地）；工作树 `desktop-explore-list-archive`（git worktree，**非**已安装应用）；
> cwd `<worktree>/packages/desktop/src-tauri`；命令见 `run.cmd`（`cmd /C` 形态，`--output-path` + `> "{results_file}"`）；
> 退出码 0（`explore.md` §2 记录，stderr 以 `Finished report saved to …` 收尾）；耗时 393.3 s；`cargo-llvm-cov` 0.9.1。
> 来源临时目录 `/tmp/llvmcov-s1-20261019-121051/`，本目录为持久化留痕。

## 产物

| 文件 | 内容 |
|---|---|
| `run.cmd` | 目标命令原文（`cargo llvm-cov --workspace --json --output-path "<coverage_file>" > "<results_file>"`） |
| `results.txt` | 命令 stdout 全文：2198 条 `test … ... ` 用例行 / 17 个 `running` 块 / 首行非 JSON（`--output-path` 生效） |
| `coverage-files.txt` | 209 条覆盖文件路径（llvm-cov 顺序，逐条绝对路径） |
| `coverage-totals.json` | totals 原始计数与百分比 + 分组计数 + 阈值判定 |

未入库：全量 `coverage-summary.json`（14.8 MB 量级），仅保留上表派生件。

## 判据映射（AC-6）

| AC-6 项 | 证据文件 | 实测 |
|---|---|---|
| `results.txt` 用例行 ≥ 2180 | `results.txt` | 2198 |
| 覆盖文件数 ≥ 200 且含 `crates/**` | `coverage-files.txt` / `coverage-totals.json` | 209（`crates/core` 104 + `crates/infra` 79 + `crates/tools` 1 + `src-tauri/src` 25），`tests_glob_hits = 0` |
| lines ≥ 90 | `coverage-totals.json` | 96.6439 %（85784 / 88763） |
| functions ≥ 90 | `coverage-totals.json` | 90.2164 %（6713 / 7441） |
| 全局 `coverage.pass=true`（加权） | `coverage-totals.json` × `../baseline/packages_desktop_vite-plus` 原始计数 | lines 96.63 / functions 90.58 / branches 93.02，三项 ≥ 阈值（90 / 90 / 85） |
| `conclusion=pass` | 退出码 0 + 双工件齐全 + 三维度达阈值（`aggregate` / `determine_conclusion` 零改动的等价判定） | 满足；门禁 summary 侧待自举回验（见末节） |

## 局限与留痕

1. **本证据不是门禁进程产物**：跑的是工作树里的目标命令，不经 `crates/core/checks` 聚合，故 summary / `overrides` / `conclusion` 三面只能以等价判定承接（上表末两行）。
2. run 起始阶段 `cargo-llvm-cov` 内部的 `cargo clean` 因文件锁失败（stderr `error: failed to remove file …dev_team-….exe`），属清理旧产物的非致命告警，随后以 `Finished report saved` 收尾、totals 完整（`lines.count = 88763` 与 baseline 一致），不影响本证据数值有效性；该告警本身即 proposal R6（残留文件锁）的现网实证。
3. 覆盖口径含 `*_test.rs`（与现网一致，见 design D5）。
4. 另有两次更早实验（`115744` / `120225`）带 `-- --list`（不跑用例 → 覆盖 0 %），只作「stdout 未被 JSON 污染」面参照，**不**作覆盖数字证据。
5. `run.cmd` 里额外的 `2> "…/stderr.txt"` 是人工采集留痕用（把 stderr 与 stdout 分开存），**不属于** registry 模板；模板只有 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"`，stderr 由 spawn 捕获面承载（design D10）。

## 自举次序（门禁产物何时翻转）

当前门禁进程是已安装 **dev-team 0.5.17**（exe 于 2026-10-18 15:31 构建；进程于 2026-10-19 10:45 启动），早于 `detect.rs` 改动（2026-10-19 16:56）。因此本 change `reports/test/` 必为旧双段链口径（rust 25 文件 / 97.17 / 91.00；summary 加权 97.48 / 93.02 / 93.35），**retry 与重测 test-execution 都不改此事实**。翻转次序：

1. 合入本 change（`detect.rs` / `runner.rs` / `package.json` 0.5.19）；
2. 构建并安装 desktop 0.5.19；
3. 重启门禁进程（新 exe 生效）；
4. 此后任一 test-execution run 的 `reports/test/` 才呈现新口径（209 文件 / 2198 用例行 / lines 96.64 / functions 90.22），届时回验 `conclusion=pass`、`coverage.pass=true`、`coverage.overrides=null`。

在此之前，`reports/test/` 的旧值不构成回归失败，也不得被引作「变更后」数字的证据。
