# rust suite 单构建合并（S1）

workflow_type: requirement

> **一句话**：把 `cargo llvm-cov` 与 `cargo test --workspace` 两条命令合并为一条，
> 消除重复构建、修复覆盖率口径（25 → 209 文件）、消除 `&` 吞退出码、消除跨相位 target 冲突。
>
> **状态**：已实测验证（2026-10-19，full 模式全绿），可直接实施。
> **依赖**：无。本篇可独立创建 change。

---

## 1. 问题

`crates/infra/checks/src/testexec/detect.rs` 的 rust 档模板（现状）：

```rust
// shell 臂
"cargo test --workspace; _X=$?; cargo llvm-cov --json --output-path \"{coverage_file}\"; exit $_X"
// cmd 臂
"cargo llvm-cov --json --output-path \"{coverage_file}\" & cargo test --workspace"
```

三个独立缺陷：

### 1.1 重复构建 + 重复跑测试

`cargo llvm-cov` **自己就会运行测试**，后面再跟一条 `cargo test` 是第二遍。
且两者用**不同 target 目录**（`target/llvm-cov-target` / `target/debug`），无法复用。

### 1.2 `&` 吞退出码（覆盖率可能假绿）

`cmd` 的 `&` 是**无条件顺序分隔符**，退出码取**最后一条**：

```
cargo llvm-cov ...（失败：覆盖率工件未产出）& cargo test --workspace（成功）→ 整体退出 0 → suite 判 pass
```

历史实证：`openspec/changes/archive/2026-10-18-desktop-agent-chat-component/reports/baseline/exec-outputs/exec-1792234957813-1.log`
里 `cargo llvm-cov` 因文件锁失败、`cargo test` 跑绿 → suite 判 pass。

冷/热 target 的失败概率不同（冷 target 先死、热 target 后死），这个窗口是真实存在的。

### 1.3 覆盖率只测 25 个文件（最严重）

`cargo llvm-cov` **不带 `--workspace`** 时只测根包 `dev-team`：

| 口径 | 覆盖文件 | 分布 |
|---|---|---|
| 现状（无 `--workspace`） | **25** | 全部 `src-tauri/src/**`；**`crates/**` 零覆盖** |
| 合并后（带 `--workspace`） | **209** | `crates/core` 104 + `crates/infra` 79 + `crates/tools` 1 + `src-tauri/src` 25 |

而 2180 条用例跑在 `cargo test --workspace`（另一份 target、不产覆盖率）。

> **结论：门禁用的覆盖率数字，与门禁用的测试集，不是同一批用例产出的。**
> 这解释了历史 summary 里 coverage 长期稳定 96–97%、从不下滑。

### 1.4 跨相位 target 冲突

一次 run 内：

```
StaticCheck   → cargo clippy + cargo run   → target/debug
TestExecution → cargo llvm-cov             → target/llvm-cov-target（冷，全量重编）
              → cargo test --workspace     → target/debug  ← 要清理 static-check 刚写下的 .rcgu.o
```

实测失败（`desktop-explore-list-archive/reports/baseline/exec-outputs/exec-1792379250452-2.log`）：

```
error: failed to remove …\target\debug\deps\foundation-…*.rcgu.o: 另一个程序正在使用此文件 (os error 32)
```

合并为单条 `cargo llvm-cov --workspace` 后，test-execution **不再触碰 `target/debug`**，
该冲突从根上消失。

---

## 2. 实测验证（2026-10-19）

命令：

```
cargo llvm-cov --workspace --json --output-path "<cov>" > "<results>"
```

| 项 | 值 | 判读 |
|---|---|---|
| 退出码 | **0** | ✅ |
| 耗时 | 393.3 s | 与现状（391 s）基本持平 —— 但少了一遍全量构建 |
| `results.txt` 用例行 | **2198** | ✅ 与历史成功值完全一致 |
| `running` 块 | **17 个**（146/273/99/22/128/40/274/0/29/0/40/349/300/88/6/371/33） | ✅ 全成员覆盖 |
| 含中文用例名 | 2184 行 | ✅ 编码无损 |
| `results` 首行是 JSON | False | ✅ `--output-path` 生效，stdout 未被污染 |
| coverage JSON | 14.8 MB / **209 文件** / 可解析 | ✅ |
| `lines.percent` | **96.64 %** | ✅ |

**关键前提已验证**：`cargo llvm-cov` 完整转发 test harness 的 stdout（`running N tests` /
`test x ... ok`），所以 `> results.txt` 可直接作为结果工件。

---

## 3. 覆盖率口径与阈值

原始 totals 混入了 103 个 `*_test.rs`（占 81% 代码行、覆盖率 97.48%），会显著虚高：

| 口径 | 文件数 | lines | functions |
|---|---|---|---|
| 原样（含 `_test.rs`） | 209 | **96.64 %** | 90.22 % |
| 排除 `_test.rs` | 106 | 93.00 % | 85.84 % |
| 现网阈值 | — | 90 / 85 / 90 | — |

按业务代码（106 文件）分组：

| 组 | 文件 | lines | functions |
|---|---|---|---|
| `crates/core` | 52 | 93.36 % | 91.88 % |
| `crates/infra` | 40 | 94.01 % | 84.61 % |
| `crates/tools` | 1 | **0.00 %** | **0.00 %**（export-bindings 无测试） |
| `src-tauri/src` | 13 | 85.87 % | 68.95 % |

**决议**：沿用现网口径（**不排除 `_test.rs`**，现网 25 文件里同样含 `mod_test.rs`），
则 lines 96.64 ≥ 90、functions 90.22 ≥ 90，**三项全过，零阈值调整**。

> 若将来要收紧口径（排除 `_test.rs`），需另立变更：functions 85.84 < 90，
> 阈值要调到 85，或给 `export-bindings` 补测试。

---

## 4. 改动清单

| 文件 | 改动 |
|---|---|
| `crates/infra/checks/src/testexec/detect.rs` | rust 档两臂模板改为单条 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" > "{results_file}"` |
| `crates/infra/checks/src/testexec/runner.rs` | 删 `applyResultsRedirect` 分支（82–92 行）与 `native_output_file`（45–48 行）——S1 后不再有 `cargo test` 字样，重定向已进模板 |
| `crates/infra/checks/src/testexec/detect_test.rs` | 157–159 行模板常量、258 行 `cmd_template` 断言、370 行 `starts_with("cargo test --workspace")` 断言随动 |
| `openspec/specs/desktop-test-execution/spec.md` | 补「rust suite 单构建」要求：一条命令同时产结果与覆盖率、退出码直继承、覆盖全 workspace |

**不改**：jest / vitest / vite-plus / node-test 四档模板（它们本来就是单条命令）。

> **与篇 2 的协作**：本篇只动 `runner.rs` 的 82–92 / 45–48 两段；
> `exec-run-shell.md` 动的是同文件 99–150 的 spawn 段。同文件不同区块，
> 建议先合入本篇。

---

## 5. 测试策略

| 层 | 用例 |
|---|---|
| `detect_test` | rust 档两臂模板逐字断言（新串）；断言**不含** ` & `、**含** `--workspace`、**含** `> "{results_file}"` |
| `runner_test` | 平台等价锚：Windows `cmd /C` 下单条命令的 `>` 重定向落位（沿既有 `windows引号占位模板_重定向工件落位` 先例）；断言命令串里不再出现 `cargo test` |
| 真件回归 | 手工跑一次 full（本次已做，393 s / 2198 用例 / 96.64 %），作为验收基线 |

---

## 6. 验收标准

1. rust suite 执行命令为单条 `cargo llvm-cov --workspace ... > "{results_file}"`，退出码直接来自该命令。
2. `results.txt` 用例行 ≥ 2180，`coverage-summary.json` 覆盖文件数 ≥ 200。
3. 覆盖率三维度达现网阈值（lines ≥ 90、functions ≥ 90、branches 按现网口径）。
4. `target/debug` 不再被 test-execution 触碰（静态检查产物无跨相位清理冲突）。
5. 四档 JS 框架模板零改动，其用例全绿。

---

## 7. 风险

| 风险 | 缓释 |
|---|---|
| `cargo llvm-cov` 未来版本不再转发 harness stdout | `runner_test` 加真件锚；若失效，回退到「两条命令 + `&&`」 |
| `--workspace` 使覆盖率分母扩大、某天跌破阈值 | 已实测 96.64 / 90.22；跌破时按 §3 走口径调整变更 |
| 冷构建首次耗时（llvm-cov-target 全量） | 现状同样存在；本篇不恶化 |
| `export-bindings` 0 % 覆盖率拉低整组 | 已计入实测（lines 96.64 仍达标）；补测试可另立变更 |
