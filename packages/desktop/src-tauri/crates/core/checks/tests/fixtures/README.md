# 语料清单（checks corpus）

> 采集自真实工作区产出形态、不手造语义漂移样本；golden 期望以 CLI zod 权威
> （`plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts`）形态
> 录制，重写开关与两步工作流见 `tests/golden/README.md`。

## 解析器语料（单元测试 include_str 消费）

| 路径 | 形态 | 消费方 |
|------|------|--------|
| `istanbul/coverage-summary.json` | istanbul coverage-summary（vite-plus 档真实形态：total + 双文件键） | `parser/js_test.rs`、`parser/coverage_test.rs` |
| `llvm-cov/coverage.json` | llvm-cov JSON（双 data 窗口：多 segment / 空 segments / 单 segment + totals 块） | `parser/rust_test.rs`、`parser/coverage_test.rs` |
| `node-spec/output.txt` | node-test `--test-reporter=spec` 绿跑文本（5 用例：4 pass 1 skipped，含子测试缩进与套件收口行） | `parser/text_test.rs`、`corpus_golden_test.rs` |
| `node-spec/output-fail.txt` | node-test spec 红跑文本（失败用例 + 缩进错误块） | `parser/text_test.rs` |
| `cargo/output.txt` | cargo test 文本（ok / FAILED / ignored + `---- x stdout ----` 失败详情块） | `parser/text_test.rs` |
| `cargo-compile-error/output.txt` | cargo test 编译失败红态文本（零结果行） | `parser/text_test.rs` |

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
