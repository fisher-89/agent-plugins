# 语料清单（checks corpus）

> 采集自真实工作区产出形态、不手造语义漂移样本；golden 期望基准为已固化
> `core/checks` 实现 + 入仓金样，重写开关与两步工作流见 `tests/golden/README.md`。

## 解析器语料（单元测试 include_str 消费）

| 路径 | 形态 | 消费方 |
|------|------|--------|
| `istanbul/coverage-summary.json` | istanbul coverage-summary（vite-plus 档真实形态：total + 双文件键） | `parser/js_test.rs`、`parser/coverage_test.rs` |
| `llvm-cov/coverage.json` | llvm-cov JSON（双 data 窗口：多 segment / 空 segments / 单 segment + totals 块） | `parser/rust_test.rs`、`parser/coverage_test.rs` |
| `node-spec/output.txt` | node-test `--test-reporter=spec` 绿跑文本（5 用例：4 pass 1 skipped，含子测试缩进与套件收口行） | `parser/text_test.rs`、`corpus_golden_test.rs` |
| `node-spec/output-fail.txt` | node-test spec 红跑文本（失败用例 + 缩进错误块） | `parser/text_test.rs` |
| `libtest-json/output.txt` | nextest `--message-format libtest-json`（格式版本 0.1）真实捕获事件流：3 binary 块 / 5 终态事件（3 ok + 1 failed + 1 ignored），失败事件携 `stdout` 块。采集：nextest 0.9.150 / cargo-llvm-cov 0.9.1 / rustc 1.95，`cargo llvm-cov nextest --message-format libtest-json --message-format-version 0.1 --no-fail-fast`（红跑形态，覆盖率工件不出） | `parser/libtest_test.rs` |

## corpus 语料（全链黄金对拍消费）

| 目录 | 档位 | 形态 | 对应 golden |
|------|------|------|-------------|
| `corpus/green-istanbul/` | vite-plus（istanbul）绿跑 | `sub-report.json` + `summary.json`（全字段，coverage 达阈 80/70/75） | `golden/green-istanbul.json` |
| `corpus/green-llvm-cov/` | rust（llvm-cov）绿跑 | `sub-report.json` + `summary.json`（branches/functions null 维度，阈值 70/65/60） | `golden/green-llvm-cov.json` |
| `corpus/green-node-test/` | node-test 文本绿跑 | `sub-report.json` + `summary.json`（5 用例 4/0/1 + 覆盖实测 82.5/74.1/90.3） | `golden/green-node-test.json` |
| `corpus/red-run/` | vite-plus 红跑 | `sub-report.json` + `summary.json`（2 失败用例 + test_failure problems ×2 + 聚类 findings） | `golden/red-run.json` |
| `corpus/mutation-block/` | 真实 mutation 块历史产出保真 | `sub-report.json`（mutation 块 + override 组；无 summary 面） | `golden/mutation-block.json` |
| `corpus/unknown-fields/` | 未知字段宽容样本 | `sub-report.json`（顶层 / summary / error_case / coverage 多余键混入） | `golden/unknown-fields.json` |
| `corpus/corrupt-invalid-json/` | 损坏样本（截断 JSON） | `sub-report.json` | `golden/corrupt-invalid-json.json` |
| `corpus/corrupt-missing-required/` | 损坏样本（必填缺失：framework / duration_ms / summary） | `sub-report.json` | `golden/corrupt-missing-required.json` |

## corpus 投影使用的 config::TestSuite 集（Rust 侧构造，`suites_for()`）

| corpus | suite 配置 |
|--------|-----------|
| `green-istanbul` / `red-run` | root `.`，framework `vite-plus`，coverage 80/70/75 |
| `green-llvm-cov` | root `crates/core`，framework `rust`，coverage 70/65/60 |
| `green-node-test` | root `.`，framework `node-test`，coverage 60/70/75 |
| 解析保真族（无 summary 面） | 空 suites（不参与聚合投影） |

## 维护约定

- 语料样本零改写原则：golden 漂移优先怀疑实现漂移，重写前先 review diff。
- 新增语料：目录 + 清单行 + `CORPUS_FIXTURES` 常量 + `suites_for` 映射同步维护。
- 样本规模下限八套（绿跑三档 + 红跑 + mutation / 未知字段 / 损坏 ×2）。

## 段级 mutation 语料（mutation-seg——desktop-fix-mutation-test / AC-9）

独立子树（不入 corpus 八套清单；golden 独立目录 `tests/golden-mutation-seg/`，
重写开关同 `DESKTOP_GOLDEN_REWRITE`）。

| 目录 | 执行器 | 形态 | 对应 golden |
|------|--------|------|-------------|
| `mutation-seg/mutation-seg-cargo/` | cargo-mutants | `seg-report.json`（`--json` mutants 数组：Caught / missed / TIMEOUT / unviable / not-mutated 全映射表样本 + 未知 outcome + 缺 outcome 字段条目） | `golden-mutation-seg/mutation-seg-cargo.json` |
| `mutation-seg/mutation-seg-stryker/` | Stryker | `seg-report.json`（metrics 聚合面 + files 逐 mutant 面——metrics 优先口径；含 NoCoverage / RuntimeError / Ignored 词面） | `golden-mutation-seg/mutation-seg-stryker.json` |

消费方：`corpus_golden_test.rs` 段级对拍节（`project_seg_fixture`：宽容解析 →
单段归并 → 归并块结论投影）；映射表逐位归位与 runtimeError 留痕由
`段级未知outcome样本runtimeError留痕` 断言承载。
