# 测试设计: desktop-checks-domain

> **日期**: 2026-10-06

---

## 验收范围

<!-- 逐条映射 proposal.md 的 12 条 AC。被测文件或模块为承载用例的测试文件（单值）；
     纯静态 / 无行为面 AC 落「—（见不可测试项 N）」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | `crates/core/checks` 与 `crates/infra/checks` 存在且为 workspace member；`cargo test --workspace` 全绿 | packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs（core 侧注册编译证据；infra 侧经 static_check_test 与 testexec 各节随 AC-2 / AC-4 / AC-5 承载；套件级绿见不可测试项 3） |
| AC-2 | `static_check.rs` + 测试落 `infra/checks`，`infra/agent` 无 static_check 残留；平移后测试全绿且逻辑零改动 | packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs |
| AC-3 | `port.rs` 具 `ToolCommand::TestExecution` / `ToolStepOutput::TestExecution` / `TestExecutionRunner`；core/orchestration 源码零进程 spawn | packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs（零 spawn 半边为 crate 依赖图编译期约束——core/orchestration 零 tokio process 依赖即不可越线；消费半边经 steps_test.rs 同条随动） |
| AC-4 | detect 对 jest / vitest / vite-plus / rust / node-test 产出 plan；bun / go / pytest 配置显式 `Err` 不静默跳过 | packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect_test.rs |
| AC-5 | fixture 驱动 detect → 完整性门禁 → 执行 → 报告写盘 → 聚合全链，conclusion=pass 且报告落 change 报告目录 | packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs |
| AC-6 | 输入未变复用上次 summary 零 spawn；输入变化失效重跑（单测覆盖两分支） | packages/desktop/src-tauri/crates/core/checks/src/reuse_test.rs（复用门装配半边经 mod_test.rs 同条随动） |
| AC-7 | test-execution 相位门禁步上图（`ChangeStepKind::TestExecution`）；fail/error 反馈边独立计数 5 次超限升格相位 fail；绿跑零 agent 会话 | packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs |
| AC-8 | 真实 summary / report fixtures 的解析与聚合结论和 CLI zod 权威一致 | packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs |
| AC-9 | change 报告目录推导入 foundation layout；desktop 产品源码零磁盘路径字面量（命名隔离双禁令扫描通过） | packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs |
| AC-10 | 组合根装配新 runner；bindings 再生成（ChangeStepKind 新变体出线）；前端流程视图步骤渲染零改动直接吃 | packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs（testExecution 出线进程内半边；装配缝经 steps_test.rs 承载——组合根一行构造与前端零改动见不可测试项 5 / 6） |
| AC-11 | orchestration「唯一 spawn 例外」与 Module Contract worker.rs 行、crate-layout 边界分类学六类措辞与实现一致 | —（见不可测试项 1） |
| AC-12 | `packages/desktop/package.json` version 0.4.12（用户可见变更：run 内新增确定性测试执行门禁） | —（见不可测试项 2） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test] / #[tokio::test]）。测试框架识别结果：
     全部模块落 packages/desktop/src-tauri rust 套件（cargo test），测试文件为共置 *_test.rs
     模块（沿 orchestration / workflow / foundation 既有先例）与 tests/ 语料黄金测试。
     门面模块（core/checks/src/lib.rs、infra/checks/src/lib.rs、parser/mod.rs）不建独立测试
     文件，见不可测试项 7。既有 *_test.rs 扩展节（port / state / steps / walker / layout）
     仅声明新增用例行，既有用例零改动持衡。 -->

### packages/desktop/src-tauri/crates/core/checks/src/model.rs -> packages/desktop/src-tauri/crates/core/checks/src/model_test.rs

<!-- 挂 AC-8（报告 schema 以 CLI zod 权威逐字段对齐——模型半边）、AC-1（新 crate 测试面）。 -->

#### 待测功能

- parse_sub_report(): 子报告宽容解析（未知字段忽略，zod 对齐）
- parse_summary_report(): 汇总报告宽容解析

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| model_test · parse_sub_report | 正向 | 真实形态子报告 JSON（framework / root / timestamp / exit_code / duration_ms / summary / error_cases / test_files / source_files / coverage 全字段）解析出 SubReport 逐字段相等 | 新增 |
| model_test · parse_sub_report | 异常 | 非法 JSON 文本 → Err 记因（解析失败面可读） | 新增 |
| model_test · parse_sub_report | 异常 | 必填字段缺失（缺 framework / 缺 summary 计数块）→ Err，required 集与 CLI zod schema 一致 | 新增 |
| model_test · parse_sub_report | 边界 | 未知字段混入（多余键）→ 解析成功且忽略未知键、已知字段不丢（zod 宽容对齐） | 新增 |
| model_test · parse_sub_report | 边界 | coverage / mutation / findings 缺省（null 或缺键）→ None 形态解析与再出线往返无损 | 新增 |
| model_test · parse_sub_report | 边界 | 含真实 mutation 块样本 → MutationBlock 保真解析不丢块（schema 保真半边——生产聚合恒 null 与解析保真两半分离） | 新增 |
| model_test · parse_sub_report | 边界 | 超长字符串与特殊字符（换行 / unicode / emoji）字段值保真往返 | 新增 |
| model_test · parse_summary_report | 正向 | 真实形态 summary JSON（phase / command / timestamp / 四计数 / conclusion / problems[] / coverage / plans[] / findings[]）解析出 SummaryReport 逐字段相等 | 新增 |
| model_test · parse_summary_report | 异常 | conclusion 非法枚举值 → Err（pass / fail / error 封闭集拒识） | 新增 |
| model_test · parse_summary_report | 边界 | 空 problems[] / 空 plans[] / 四计数全 0 的最小合法 summary 解析成功 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——fixtures JSON 串字面量与 include_str 真实语料入参，零进程边界零 IO 依赖 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/parser/js.rs -> packages/desktop/src-tauri/crates/core/checks/src/parser/js_test.rs

<!-- 挂 AC-8（istanbul 族解析——真实工作区 coverage-summary 对拍输入半边）。 -->

#### 待测功能

- parse_istanbul_summary(): istanbul coverage-summary.json → 逐文件覆盖原始计数（jest / vitest / vite-plus 共享）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| js_test · parse_istanbul_summary | 正向 | 真实 istanbul coverage-summary 样本（total 键 + 多文件键）→ SourceFileEntry 逐文件四维计数逐字对齐 | 新增 |
| js_test · parse_istanbul_summary | 正向 | 多文件样本全量提取——文件集合与计数逐条对应（顺序无关断言） | 新增 |
| js_test · parse_istanbul_summary | 异常 | 非法 JSON → Err 记因 | 新增 |
| js_test · parse_istanbul_summary | 异常 | 文件键结构残缺（缺 statements.covered 等计数字段）→ Err 记因，不产半截条目 | 新增 |
| js_test · parse_istanbul_summary | 边界 | 仅 total 键无文件键 → 空条目集（total 键不产条目）且不 panic | 新增 |
| js_test · parse_istanbul_summary | 边界 | 全零覆盖文件（四维 covered=0）→ 计数 0 保真（不丢不抹平） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——真实 istanbul 样本 JSON 串（语料文件 include_str）入参 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/parser/rust.rs -> packages/desktop/src-tauri/crates/core/checks/src/parser/rust_test.rs

<!-- 挂 AC-8（llvm-cov 解析——src-tauri rust 档真实产出对拍输入半边）。 -->

#### 待测功能

- parse_llvm_cov(): llvm-cov JSON → 逐文件覆盖原始计数

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| rust_test · parse_llvm_cov | 正向 | 真实 llvm-cov JSON 样本（data[].files[] filename + segments）→ 逐文件覆盖计数（行覆盖由 segments 推导） | 新增 |
| rust_test · parse_llvm_cov | 正向 | 多文件多 data 窗口样本全量提取 | 新增 |
| rust_test · parse_llvm_cov | 异常 | 非法 JSON → Err 记因 | 新增 |
| rust_test · parse_llvm_cov | 异常 | 结构残缺（缺 data / files 键）→ Err 记因 | 新增 |
| rust_test · parse_llvm_cov | 边界 | 空 files 数组 → 空条目集不 panic | 新增 |
| rust_test · parse_llvm_cov | 边界 | 无 segments / 单 segment 文件 → 计数 0 边界保真 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——真实 llvm-cov 样本 JSON 串（语料文件 include_str）入参 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/parser/coverage.rs -> packages/desktop/src-tauri/crates/core/checks/src/parser/coverage_test.rs

<!-- 挂 AC-8（覆盖格式路由面——CoverageFormat 三值封闭集）。 -->

#### 待测功能

- parse_coverage(): 按覆盖格式路由至子解析器

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| coverage_test · parse_coverage | 正向 | CoverageFormat istanbul 值路由至 istanbul 解析（与 js_test 同构样本同结果） | 新增 |
| coverage_test · parse_coverage | 正向 | CoverageFormat llvm-cov 值路由至 llvm-cov 解析（与 rust_test 同构样本同结果） | 新增 |
| coverage_test · parse_coverage | 正向 | CoverageFormat node-test 值在 V1 支持面路由可达（三值封闭集齐备） | 新增 |
| coverage_test · parse_coverage | 异常 | 结构残缺样本经各路由 Err 上抛且记因（路由层不吞错不换语义） | 新增 |
| coverage_test · parse_coverage | 边界 | CoverageFormat 三值互异（istanbul / llvm-cov / node-test 构造可辨） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——样本 JSON 串入参，路由纯分发 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/parser/text.rs -> packages/desktop/src-tauri/crates/core/checks/src/parser/text_test.rs

<!-- 挂 AC-8（node-test spec / rust 测试文本解析——CLI parseTextOutput 两分支同语义移植）。 -->

#### 待测功能

- parse_spec_report(): node-test `--test-reporter=spec` 文本结果
- parse_cargo_test(): rust 测试文本输出解析（与 node-test 同路由 text 引擎，CLI `parseTextOutput` rust 分支同语义）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| text_test · parse_spec_report | 正向 | 真实 node-test spec 输出样本（pass / fail / skip 结果行 + 子测试缩进层级）→ TestCaseResult 列表 status 三值正确 | 新增 |
| text_test · parse_spec_report | 正向 | 失败用例行 → errorType / errorMessage / stackTrace 仅失败态填充且内容捕获不丢 | 新增 |
| text_test · parse_spec_report | 边界 | 噪声行（诊断 / 汇总 / 空行）不误提取为用例（提取器只认结果行锚模式） | 新增 |
| text_test · parse_spec_report | 边界 | 测试名含特殊字符（空格 / unicode / 中文）保真到用例行 | 新增 |
| text_test · parse_spec_report | 边界 | 空文本输入返回空用例集不 panic（零用例行计数面交完整性门禁对账） | 新增 |
| text_test · parse_cargo_test | 正向 | 真实 cargo test 文本输出样本（`test xxx ... ok / FAILED / ignored`）→ 用例行 status 三值正确 | 新增 |
| text_test · parse_cargo_test | 正向 | 失败行携断言输出 → errorMessage / stackTrace 捕获 | 新增 |
| text_test · parse_cargo_test | 异常 | 编译失败输出（零结果行）→ 与 CLI parseTextOutput rust 分支同语义（corpus 红态样本对拍锚定），不误产用例行 | 新增 |
| text_test · parse_cargo_test | 边界 | 超长输出（>1000 行噪声夹杂少量结果行）提取不丢不误增 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——真实测试器文本样本（语料文件 include_str）入参 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs -> packages/desktop/src-tauri/crates/core/checks/src/aggregate_test.rs

<!-- 挂 AC-8（聚合结论与 CLI determineConclusion / computeOverrides 同语义）；AC-5（链上聚合环节的纯函数半边）。 -->

#### 待测功能

- determine_conclusion(): CLI `determineConclusion` 同语义——error 优先 → failed>0 / 覆盖未达 / mutation 未达 → fail → pass
- aggregate_coverage(): 全局聚合 + per-suite override 组（glob 范围纯匹配，suite 阈值取 `config::CoverageThresholds`）
- build_summary_report(): 汇总组装——计数合并、problems 归并、coverage / mutation 块、plans 路径索引、conclusion 判定

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| aggregate_test · determine_conclusion | 正向 | failed=0、覆盖达阈、无 execution_error → Conclusion pass | 新增 |
| aggregate_test · determine_conclusion | 正向 | failed>0 → fail（error 不在场时 fail 优先于 pass） | 新增 |
| aggregate_test · determine_conclusion | 正向 | 覆盖未达阈（coverage Some 且任一维度低于阈值）→ fail | 新增 |
| aggregate_test · determine_conclusion | 正向 | has_execution_error=true → error（error 优先级最高——CLI determineConclusion 同序） | 新增 |
| aggregate_test · determine_conclusion | 边界 | 覆盖恰等于阈值 → 达标（≥ 边界不误判未达） | 新增 |
| aggregate_test · determine_conclusion | 边界 | coverage=None 且 mutation=None（null 维度感知跳过）→ 不因缺块误 fail | 新增 |
| aggregate_test · determine_conclusion | 边界 | failed=0 与全失败大计数两极判定一致（u64 计数边界） | 新增 |
| aggregate_test · determine_conclusion | 边界 | mutation Some 且低于阈值 → fail（函数面完整语义；生产聚合恒 None 不触达） | 新增 |
| aggregate_test · aggregate_coverage | 正向 | 多子报告全局聚合 + per-suite override 组（suite glob 纯匹配命中 source_files 取该 suite 阈值） | 新增 |
| aggregate_test · aggregate_coverage | 异常 | suite 阈值缺失（config coverage null）→ 默认档语义不 panic（CLI 同语义） | 新增 |
| aggregate_test · aggregate_coverage | 边界 | 子报告 source_files 全空 → 聚合产出零维面（不产假计数） | 新增 |
| aggregate_test · aggregate_coverage | 边界 | suite glob 不命中任何文件 → 该 suite override 不产条目 | 新增 |
| aggregate_test · build_summary_report | 正向 | 计数合并 / problems 归并 / plans 路径索引 / coverage 块 / conclusion 判定组装逐字段相等 | 新增 |
| aggregate_test · build_summary_report | 边界 | 零子报告 → 空汇总（四计数 0、problems 空不 panic） | 新增 |
| aggregate_test · build_summary_report | 边界 | command 字段断言仅存在性不锚措辞（桌面固定串实现期定稿——design 待决遗留） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——config::TestSuite / glob::Pattern / SubReport 内存构造入参（glob 库匹配语义不复测，只测自研聚合组装） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/diagnose.rs -> packages/desktop/src-tauri/crates/core/checks/src/diagnose_test.rs

<!-- 挂 AC-5（完整性门禁链上环节）；AC-8（诊断树分支与 CLI executor 定义 4a/4c/4d/4e/4f 对齐）；AC-7（findings 为反馈边原料的上游半边）。 -->

#### 待测功能

- check_integrity(): 完整性 checklist——聚合计数对账、plan 报告齐全性、conclusion 枚举合法性（违例即 findings）
- diagnose_findings(): 诊断树确定性分支——阈值比对（null 维度感知跳过）、execution_error 归因、失败聚类措辞

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| diagnose_test · check_integrity | 正向 | 一致 summary + 子报告全集 → 空 findings（计数对账 / 报告齐全 / conclusion 合法三查全过） | 新增 |
| diagnose_test · check_integrity | 异常 | 聚合计数与子报告之和对不上 → findings 记因（对账违例面可读） | 新增 |
| diagnose_test · check_integrity | 异常 | summary.plans 引用缺失子报告 → findings 记因 | 新增 |
| diagnose_test · check_integrity | 异常 | conclusion 与计数不一致（全绿计数却 fail）→ findings 记因 | 新增 |
| diagnose_test · check_integrity | 边界 | 零子报告零 plans 的空 summary → 不 panic 且结论明确（空集或记因） | 新增 |
| diagnose_test · diagnose_findings | 正向 | 覆盖未达阈 summary → 阈值比对 findings（措辞携维度与数值——CLI 定义同型） | 新增 |
| diagnose_test · diagnose_findings | 正向 | execution_error 问题在场 → 归因 findings | 新增 |
| diagnose_test · diagnose_findings | 正向 | 多文件失败用例 → 失败聚类措辞 findings | 新增 |
| diagnose_test · diagnose_findings | 边界 | coverage 维度 null → null 感知跳过不产覆盖 findings | 新增 |
| diagnose_test · diagnose_findings | 边界 | 全绿 summary → 空 findings（零误报） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——SummaryReport / SubReport 内存 fixture 构造 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/reuse.rs -> packages/desktop/src-tauri/crates/core/checks/src/reuse_test.rs

<!-- 挂 AC-6（复用门两分支单测——纯判定半边；mtime 扫描与零 spawn 装配半边经 mod_test.rs 承载）。 -->

#### 待测功能

- is_reusable(): 结论非 error 且时间戳可解析且 ≥ 最新输入 mtime → 复用（CLI `tryReuseFreshSummary` 同语义）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| reuse_test · is_reusable | 正向 | 结论 pass + 时间戳可解析且 ≥ 最新输入 mtime → true（复用零 spawn 判据） | 新增 |
| reuse_test · is_reusable | 正向 | 结论 fail + 时间戳新鲜 → true（fail 结论同属可复用——「结论非 error」语义对偶面） | 新增 |
| reuse_test · is_reusable | 异常 | 结论 error → false（失效重跑） | 新增 |
| reuse_test · is_reusable | 异常 | 时间戳不可解析（缺失 / 非法格式）→ false | 新增 |
| reuse_test · is_reusable | 边界 | 时间戳恰等于最新输入 mtime → true（≥ 边界含等号） | 新增 |
| reuse_test · is_reusable | 边界 | 输入 mtime 晚于时间戳 → false（输入已变失效——反馈边修复重入判据） | 新增 |
| reuse_test · is_reusable | 边界 | newest_input_mtime 为 None（空输入集）的行为与 CLI tryReuseFreshSummary 逐字段对齐（复用门判定防漂移对照——proposal 风险缓解行） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱——SummaryReport 与 mtime 值内存构造 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/checks/src/lib.rs -> packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs

<!-- 挂 AC-8（corpus 黄金对拍）、AC-1（core/checks crate 注册编译证据）。
     design.md 未声明 lib.rs 自有公共 API（纯层门面：模块组织与重导出）；本节承载
     解析 → 聚合 → 诊断全链组合用例，链路入口 = crate 公共 API 门面（组合用例挂靠
     规则——发起方 / 最上层调用方；来源：proposal 测试文件清单行 + AC-8）。 -->

#### 待测功能

<!-- design.md 未声明 lib.rs 自有公共 API 变更（纯 re-export 门面）；本节为跨模块
     组合用例章节，被测面为 crate 公共 API 全链（parse_sub_report / parse_summary_report /
     parse_coverage / aggregate / diagnose），golden 期望以 CLI zod 权威产出录制。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| corpus_golden_test · 全链黄金对拍 | 正向 | istanbul 族绿跑真实 summary + report fixtures → 解析 → 聚合 → 诊断全链 conclusion=pass 且与 golden 期望（CLI 权威产出录制）逐字段一致 | 新增 |
| corpus_golden_test · 全链黄金对拍 | 正向 | llvm-cov 绿跑 fixtures → 同链 conclusion 与聚合面一致（src-tauri rust 档） | 新增 |
| corpus_golden_test · 全链黄金对拍 | 正向 | node-test 文本 fixtures → text 解析计数聚合与 golden 一致 | 新增 |
| corpus_golden_test · 全链黄金对拍 | 异常 | 红跑 fixtures（含 test_failure problems）→ conclusion=fail 且 findings 与 CLI 权威一致 | 新增 |
| corpus_golden_test · 全链黄金对拍 | 异常 | 损坏样本（非法 JSON / 必填缺失）→ 宽容解析不悄悄吞数据（unparsable 标记与跳过统计在 golden 上可见——workflow corpus_golden_test 先例同款） | 新增 |
| corpus_golden_test · 全链黄金对拍 | 边界 | 含真实 mutation 块样本 → mutation 位保真解析不丢块（解析保真与生产聚合恒 null 两半分离） | 新增 |
| corpus_golden_test · 全链黄金对拍 | 边界 | 未知字段样本 → 宽容忽略对拍一致 | 新增 |

<!-- fixtures 来源与样本规模（design.md 待决问题——test-design 阶段定）：
     采集自真实工作区产出、不手造——istanbul 族取 packages/desktop（vite-plus，档 80）
     真实绿 / 红跑各一套；llvm-cov 取 src-tauri rust 档（70）真实绿跑一套；node-test 取
     detect 链真实 spec 文本绿 / 红各一；特殊样本 ≥3（真实 mutation 块历史产出保真一套、
     未知字段宽容一套、损坏一套）。规模下限 8 套，落 crates/core/checks/tests/fixtures/
     并沿 workflow corpus 先例配 README 清单表；golden 期望值录制沿用重写开关机制
     （DESKTOP_GOLDEN_REWRITE 先例同款，env 开关 + 复核两步工作流）。 -->

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | fixtures 真实文件直驱（tests/fixtures/ 语料读盘 + golden 快照对比），零进程边界 | 全部对拍用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

<!-- 挂 AC-2（ProcessStaticCheck 平移零逻辑改动）。
     平移注记：本测试文件随源整体 verbatim 迁移（design.md 变更清单注记——属实现迁移
     非新写，随平移条目走、不经测试生成阶段重写）；import 面随 crate 边界调整
     （orchestration::port 依赖保持）。下表为迁入后该文件承载的既有覆盖，「新增」指
     落位新路径而非新写断言。 -->

#### 待测功能

- ProcessStaticCheck::new(): 无状态构造
- ProcessStaticCheck.run(): StaticCheckRunner 实现——config 读命令 → shell 语义 spawn → 诊断捕获 + 退出码映射（零逻辑改动平移：无配置直过 / 程序不可达显式 Err / 退出码映射不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| static_check_test · run | 正向 | 通过形态——root 标记文件经相对路径可读 → passed=true 且 diagnostics 空、cwd=root 生效 | 新增（随源平移） |
| static_check_test · run | 异常 | 失败形态——非零退出码 → passed=false 且 stdout / stderr 诊断捕获拼接不丢（反馈边注入原料） | 新增（随源平移） |
| static_check_test · run | 边界 | 无配置 / 空白命令 → 直过 passed=true（不拉子进程） | 新增（随源平移） |
| static_check_test · run | 异常 | 非零退出且诊断空 → passed=false（退出码即判据，不误判通过） | 新增（随源平移） |
| static_check_test · run | 异常 | 裸名命令不在 PATH（含 Windows 额外查 cwd 臂）→ Err 显式记因（shell 吞缺失不烧反馈边预算） | 新增（随源平移） |
| static_check_test · run | 异常 | 相对 root 路径不可达 → Err 记因「相对 root 不存在」 | 新增（随源平移） |
| static_check_test · run | 异常 | shell 本体拉起失败（PATH 隔离臂，Unix）→ Err 记因「拉起失败」 | 新增（随源平移） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 子进程边界真实组合——真实可执行命令 fixture（退出码 / 输出可编程的 shell 语义命令串）+ tempdir workspace 真实 config 读取；不 mock tokio::process 本体；PATH 隔离窗口经 crate 级互斥锁串行化（既有装置随源平移） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect_test.rs

<!-- 挂 AC-4（五框架矩阵探测 + bun / go / pytest 显式 Err）。 -->

#### 待测功能

- detect_plans(): 逐 suite 探测——glob 文件清单 + 排除过滤 + plan id；bun / go / pytest 显式 Err

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detect_test · detect_plans | 正向 | tempdir 树 + vite-plus 形态 suite 配置（includes 命中样本文件）→ TestPlan 产出（id 沿 colocated 命名映射、cwd、config_args、coverage_format=istanbul） | 新增 |
| detect_test · detect_plans | 正向 | rust suite → plan（coverage_format=llvm-cov、命令模板与 CLI 同源） | 新增 |
| detect_test · detect_plans | 正向 | excludes 过滤（排除目录 / dot 目录剪枝）后文件清单正确 | 新增 |
| detect_test · detect_plans | 异常 | bun / go / pytest suite 配置 → 显式 Err 记因（无静默跳过分支——AC-4 反向半边） | 新增 |
| detect_test · detect_plans | 边界 | includes 全不命中 → 该 suite 不产 plan（空清单面）且整体不 panic | 新增 |
| detect_test · 五框架注册表 | 正向 | jest / vitest / vite-plus / rust / node-test 注册表常量（命令模板 / coverage_format / coverage_output / default_glob / config_flag / version_command）与 CLI test-framework.ts 逐字对齐锚定（防漂移——常量表移植） | 新增 |
| detect_test · 五框架注册表 | 边界 | jest 版本探测——版本 ≥ 29.5.0 追加 --randomize；探测失败 / 空版本按「特性不支持」走基础模板，不失败不报错（CLI isVersionAtLeast 同路径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统边界 | tempdir 真实文件树 + 真实 glob::glob 遍历（探测即 FS 语义，不 mock glob 本体） | detect_plans 全部用例 |
| 子进程边界（版本探测） | 真实组合——版本命令走 PATH 既有真实程序，探测失败臂以不可达命令触发，无 mock | jest 版本探测边界用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs

<!-- 挂 AC-5（执行环节——模板展开 / 程序解析前置 / spawn / 捕获）；proposal 测试清单行「runner 测试（Windows .cmd shim、超时、程序解析前置检查）」。 -->

#### 待测功能

- execute_plan(): 模板展开 + 程序解析前置检查 + spawn + 超时 + 捕获 + 工件解析

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runner_test · execute_plan | 正向 | 占位符模板展开——results_file / coverage_file / report_dir / config_args / files 五占位符全替换逐字断言（config_args 空 / 有值双形态） | 新增 |
| runner_test · execute_plan | 正向 | Windows .cmd shim 真实 spawn——退出码 0 命令 + 预置工件拷贝到占位路径 → PlanExecution 携解析后子报告与 stdout / stderr 捕获 | 新增 |
| runner_test · execute_plan | 异常 | 裸名程序不可达 → Err 显式记因（程序解析前置——shell 吞缺失不产假产出，复用 static_check 经验） | 新增 |
| runner_test · execute_plan | 异常 | 相对 root 程序不可达 → Err 记因 | 新增 |
| runner_test · execute_plan | 异常 | 命令非零退出且工件不可解析 → PlanExecution 携 execution_error 面（Err 与 conclusion=error 两态分立——被测域失败走产出供反馈边，基础设施失败才 Err） | 新增 |
| runner_test · execute_plan | 边界 | 命令成功退出但零工件（未写 results / coverage）→ 工件解析错误面（不 panic 不误判绿） | 新增 |
| runner_test · execute_plan | 边界 | 超时常量锚定——suite 命令 600s / 版本探测 30s 与 CLI runCommand 600000 同值（常量钉 runner.rs；真实超时触发臂见不可测试项 4） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 子进程边界真实组合——真实可执行命令 fixture（Windows 以 cmd /C 起手、Unix 用 PATH 真实二进制）+ tempdir report_dir 真盘 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/report.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/report_test.rs

<!-- 挂 AC-5（报告写盘环节）；AC-9（report_dir 经 layout 推导传入的组合半边）。 -->

#### 待测功能

- write_reports(): 子报告 + summary 原子落盘

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| report_test · write_reports | 正向 | 子报告 + summary 落盘——planId 子目录 report.json 与 summary.json 读回逐字段相等、目录不存在自动创建 | 新增 |
| report_test · write_reports | 正向 | report_dir 由 change_test_reports 推导传入（tempdir change 树）——报告落 change 报告目录（layout 推导 → 写盘组合半边） | 新增 |
| report_test · write_reports | 异常 | 写盘失败（report_dir 指向普通文件）→ Err 记因（基础设施失败停给用户——不产部分结论） | 新增 |
| report_test · write_reports | 边界 | 重复写覆盖——同目录既有旧报告被完整替换无半写残留（重跑幂等） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempdir 真盘 + layout 真实推导（core/checks 模型与 foundation layout 真实组合，不 mock） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

<!-- 挂 AC-5（执行链绿跑全链——链路入口章节）、AC-6（复用门装配半边：mtime 扫描 + 零 spawn）。
     跨模块组合用例（detect → 完整性门禁 → 复用门 → 逐 suite 执行 → 报告写盘 → 聚合 →
     诊断）挂靠本节——链路发起方 / 最上层调用方 = ProcessTestExecution。 -->

#### 待测功能

- ProcessTestExecution::new(): 无状态构造（组合根装配）
- ProcessTestExecution.run(): TestExecutionRunner 实现——执行链编排 + checks 结论映射 port 最小载荷
- newest_input_mtime_ms(): 输入最新 mtime 递归扫描（剪枝域根目录树与报告目录，dot 目录剪枝）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · ProcessTestExecution.run | 正向 | 绿跑全链组合——tempdir workspace + 真实 config tests[]（vite-plus 形态 suite）+ 真实命令 fixture（退出 0 且拷贝预置 coverage-summary / results 工件到占位路径）→ ToolStepOutput::TestExecution conclusion=pass、四计数正确、summary.json 与 planId/report.json 落 change_test_reports 推导目录（AC-5） | 新增 |
| mod_test · ProcessTestExecution.run | 正向 | 复用门装配（复用分支）——首跑落 summary 后输入未变二次 run 零命令执行（命令 fixture 每执行追加 marker、marker 计数不增长）→ 复用上次 summary 结论（AC-6 复用半边，与 reuse_test 纯判定半边互为表里） | 新增 |
| mod_test · ProcessTestExecution.run | 正向 | 复用门装配（失效分支）——touch 输入文件推进 mtime 后 run → 失效重跑（marker 计数增长、summary 刷新，AC-6 失效半边） | 新增 |
| mod_test · ProcessTestExecution.run | 异常 | 测试程序不可达 → run 返回 Err（基础设施失败经 run_tool 映射 run 显式失败终态，不产部分结论——Err 与 error 两态分立） | 新增 |
| mod_test · ProcessTestExecution.run | 异常 | 命令 fixture 非零退出且工件不可解析 → conclusion=error 产出 + findings_brief 在场（报告级 error 走反馈边原料面，不 Err） | 新增 |
| mod_test · ProcessTestExecution.run | 边界 | findings_brief 装配口径——单条 200 字符截断、至多 10 条、report_dir 在载荷中可定位全量 findings 报告文件（walker diagnose_brief 口径对齐） | 新增 |
| mod_test · ProcessTestExecution.run | 边界 | checks Conclusion → port TestExecutionConclusion 三值映射逐字（pass / fail / error 小写线格式——checks-runtime 装配点唯一映射面） | 新增 |
| mod_test · ProcessTestExecution.new | 正向 | 无状态构造可经 Arc<dyn TestExecutionRunner> 注入（组合根同式装配编译锚） | 新增 |
| mod_test · newest_input_mtime_ms | 正向 | 多层文件树最新 mtime 提取——touch 递进可观测（剪枝后仍取到输入最新值） | 新增 |
| mod_test · newest_input_mtime_ms | 边界 | 报告目录 / dot 目录内改动不计入（剪枝生效——复用门不被自身报告误失效） | 新增 |
| mod_test · newest_input_mtime_ms | 边界 | 空扫描根（无文件）→ None | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 进程边界真实组合——真实子进程命令 fixture + tempdir workspace / config / 报告树；core/checks 纯层（parse / aggregate / diagnose / reuse）真实组合不 mock | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

<!-- 挂 AC-3（port 三件套扩展）。既有 port_test 扩展节——仅新增用例行，既有六变体
     封闭集 / 五假 port 装置用例零改动持衡。 -->

#### 待测功能

- TestExecutionRunner(): test-execution spawn 缝 port（W4——spawn 不进 core；checks-runtime 实现，产出契约同 ToolStepOutput 封闭集）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| port_test · ToolCommand 封闭集 | 正向 | 七变体封闭集 match 穷尽编译锚扩 ToolCommand::TestExecution（change 载荷透传、变体可辨——既有六变体测试扩行） | 新增 |
| port_test · ToolStepOutput 封闭集 | 正向 | TestExecution(TestExecutionOutcome) 变体载荷构造与提取逐字段相等（既有产出封闭集测试扩行） | 新增 |
| port_test · TestExecutionRunner | 正向 | 假实现经 Arc<dyn> 注入、root / change 透传、BoxToolFuture resolve 出 ToolStepOutput::TestExecution（object safety + Send+Sync 编译锚——StaticCheckRunner 同型） | 新增 |
| port_test · TestExecutionRunner | 异常 | 假实现 Err(String) 臂经 BoxToolFuture resolve 上抛（写盘失败类基础设施错误面——run_tool 映射输入） | 新增 |
| port_test · TestExecutionOutcome | 边界 | 最小载荷字段面——conclusion + total / passed / failed / skipped 四计数 + findings_brief + report_dir 构造、Clone / PartialEq 派生可用 | 新增 |
| port_test · TestExecutionConclusion | 边界 | 三值 as_str 线格式小写逐字（pass / fail / error——WorkerRole 先例同型） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| TestExecutionRunner 注入依赖 | 内存假实现（入参例外）——Vec 记录 root / change 调用、可编程 Ok(ToolStepOutput::TestExecution) / Err(String)，沿既有五假 port 装置先例 | trait 面与封闭集用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

<!-- 挂 AC-10（bindings 出线进程内半边——testExecution camelCase 出线 + specta Type 锚）、
     AC-7（步词汇上图半边）。既有 state_test 扩展节——仅新增用例行。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表无 state.rs 行；被测面为类型定义表条目。 -->

- ChangeStepKind::TestExecution（类型定义表）: 步词汇封闭集白名单式扩展，serde / specta camelCase `testExecution` 出线

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| state_test · ChangeStepKind | 正向 | TestExecution 出线 `testExecution` camelCase 逐字断言（九值词汇测试扩为十值） | 新增 |
| state_test · ChangeStepKind | 边界 | 十值词汇互异（camelCase 判别值集合无碰撞——三类节点可辨不变量保持） | 新增 |
| state_test · ChangeStepKind | 边界 | specta Type 可达编译锚自动覆盖新变体（既有 assert_type 测试零改动持衡——bindings 生成前提） | 新增 |
| state_test · ChangeStepState | 正向 | 步状态行携 TestExecution 步 + detail 摘要经 RunUpdate::Step 出线（步可观测 IPC 面——前端步骤渲染直接吃） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | serde_json / specta 内存构造与出线断言（既有装置先例） | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

<!-- 挂 AC-3（port 三件套消费半边——分发臂委托注入 runner）；AC-10（三参装配缝同型构造，
     组合根装配的进程内证据半边）。steps.rs 为 design.md 修改文件（机械必需条目，proposal
     未单列测试文件）；既有 steps_test 扩展节——仅新增用例行。 -->

#### 待测功能

- LocalToolSteps::new(): 第三 runner 注入参数（test_execution: Arc<dyn TestExecutionRunner>）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| steps_test · LocalToolSteps::new | 正向 | 三参构造——锚点 + static_check + test_execution 双 runner 注入（组合根同式装配） | 新增 |
| steps_test · ToolCommand::TestExecution → TestExecutionRunner 注入 | 正向 | 分发臂委托——root / change 透传、ToolStepOutput::TestExecution 原样上抛（与 StaticCheck 臂同型——AC-3 port 消费面） | 新增 |
| steps_test · ToolCommand::TestExecution → TestExecutionRunner 注入 | 异常 | TestExecutionRunner Err(String) 臂原样上抛（步层不吞错——run 显式失败面） | 新增 |
| steps_test · ToolCommand::TestExecution → TestExecutionRunner 注入 | 边界 | 相位机四步既有臂回归持衡（封闭集扩臂后 match 穷尽不回退——AC-6 写通道唯一不变量） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| TestExecutionRunner 注入依赖 | 假实现（入参例外）——记录 root / change 调用、可编程产出 / Err（FakeRunner 装置同款扩展） | 分发臂用例 |
| workflow::write 与 SessionAnchors | 真实组合不 mock——tempdir 真盘 fixture（进程内缝非进程边界，最小 mock 原则） | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

<!-- 挂 AC-7（test-execution 相位门禁步上图 + 反馈边独立预算 + 超限升格 + 绿跑零 agent）。
     既有 walker_test 扩展节——仅新增用例行，static-check 既有门禁 / 反馈边用例零改动持衡。 -->

#### 待测功能

- TEST_EXECUTION_FEEDBACK_LIMIT(): 反馈边独立上限常量（与 STATIC_CHECK_FEEDBACK_LIMIT 分立，各自计满各自升格）
- TEST_EXECUTION_PHASES(): 相位门控布局常量（STATIC_CHECK_PHASES 同型，布局词汇非路由权威）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| walker_test · 相位门禁步 | 正向 | test-execution 站（TEST_EXECUTION_PHASES 命中）TestExecution 步必经——ChangeStepKind::TestExecution 步状态行流出、时间线位置在相位收敛点；非门控相位零调用（AC-7 上图半边） | 新增 |
| walker_test · 绿跑零 agent | 正向 | pass 结论路径零 WorkerAgentPort 调用——机械 checklist 代写 phase_log 三条全 pass（suite 结论一致 / 聚合 conclusion 与计数一致 / mutation null 恒真）、report 携 conclusion + 计数 + 报告路径摘要、三会话槽位恒 None | 新增 |
| walker_test · 反馈边 | 正向 | fail 结论——findings 摘要 + 报告路径 prompt 注入修复会话：首个 fail 新会话（continue_session=None）、后续 Continue 同会话续注、provenance 沿 `<change>/test-execution/executor/<attempt>` 定式 | 新增 |
| walker_test · 反馈边 | 正向 | error 结论同 fail 通路走反馈边（不升 Err、不静默） | 新增 |
| walker_test · 反馈边 | 异常 | 反馈边独立计数恰 ≤ TEST_EXECUTION_FEEDBACK_LIMIT 且不消耗相位 retry 预算；第 5 次仍失败 → 升格相位 fail（单条 pass=false item + findings 摘要 evidence、executor 槽位携反馈会话 id、evaluator / decision 恒 None、不跑 evaluator——static-check 超限用例同型） | 新增 |
| walker_test · 反馈边 | 边界 | 独立计数与 static-check 计数分立——同 run 双门禁各自计满各自升格互不挤占（组合：static-check 失败反馈与 test-execution fail 反馈并存不串账） | 新增 |
| walker_test · 反馈边 | 边界 | 反馈边修复重入经复用门自动失效重跑（修复编辑推进 mtime → 重入门禁实跑非复用——装配语义直测在 mod_test，本节断言 walker 重入步产出随 runner 刷新） | 新增 |
| walker_test · TryFrom 窄化 | 边界 | TryFrom&lt;ToolStepOutput&gt; for TestExecutionOutcome——TestExecution 变体窄化逐字段、非 TestExecution 变体不匹配面 | 新增 |
| walker_test · 常量锚定 | 边界 | TEST_EXECUTION_FEEDBACK_LIMIT == 5、TEST_EXECUTION_PHASES == ["test-execution"]（既有常量锚定测试扩行） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| WorkerAgentPort / ToolStepPort 注入依赖 | 既有 FakeWorker / FakeTools 装置扩展（入参例外）——FakeTools 增 TestExecution 可编程产出臂（pass / fail / error / Err 四态序列），记录命令序与会话请求 | 全部 walker 用例 |
| workflow 写面 | 真实组合不 mock——tempdir 真盘 workflow.json（相位落账 / phase_log 代写经真实写面持久化断言） | 全部 walker 用例 |

### packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs

<!-- 挂 AC-9（报告路径经 layout 推导 + 命名隔离双禁令扫描随新 crate 自动执法）。
     既有 layout mod_test 扩展节——仅新增用例行。 -->

#### 待测功能

- change_test_reports(): change 报告目录纯推导（config_path 先例同型，与 resolve 同源常量组）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| layout mod_test · change_test_reports | 正向 | root + change → change 目录树下 reports / test 两级拼接逐字相等（常量组同源引用的结构证明） | 新增 |
| layout mod_test · change_test_reports | 正向 | 不存在 root / 指向文件的 root 纯推导正常返回无 IO（config_path 先例同型） | 新增 |
| layout mod_test · change_test_reports | 边界 | 空 root → 纯相对形式；尾分隔符 root join 语义不产双分隔符、不 panic | 新增 |
| layout mod_test · change_test_reports | 边界 | 同一输入结果稳定且父目录锚定 change 目录（确定性不变量） | 新增 |
| layout mod_test · 常量组 | 边界 | reports / test 两员常量逐字锚定（改名只动此处——既有常量组测试扩行） | 新增 |
| layout mod_test · 命名隔离扫描 | 边界 | 既有双禁令扫描自动覆盖新 crates 源码树——checks 两 crate 产品源码零 openspec / config.json 字面量、layout/mod.rs 唯一触点不变量随新目录自动执法（AC-9 扫描半边，无需新测试文件） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempdir 真盘 + 纯路径推导（既有装置先例） | 全部用例 |

---

## 不可测试项

- AC-11 spec delta 措辞修订（orchestration「唯一 spawn 例外」→ 检查域家族、Module Contract worker.rs 行、crate-layout 边界分类学六类） — **原因**: spec markdown 文本静态约束、无行为面；措辞与实现一致性随归档时 spec 合并流程生效（proposal 载明「随归档生效」），desktop 产品源码无对应可断言行为。
- AC-12 版本交付 `packages/desktop/package.json` version 0.4.12 — **原因**: 版本号静态声明（归档时执行），无行为面可自动化。
- AC-1「cargo test --workspace 全绿」套件级门 — **原因**: 整仓套件运行非单一测试文件可承载；两 crate 的注册与编译证据经 corpus_golden_test.rs（core 侧）与 static_check_test / testexec 各节（infra 侧）承载，套件绿由 test-execution 阶段承接验证（design.md 验收标准对齐行同口径）。
- runner 超时臂真实触发（超时终止子进程收敛 execution_error、不悬挂不烧反馈边预算） — **原因**: 超时常量钉 600s（CLI runCommand 同值），真实悬挂进程等待在 CI 时长不可行；收敛语义经常量锚定（runner_test 边界行）与 Err 收场用例承载，超时路径逻辑属薄封装随实现评审把关。
- 组合根装配一行构造（`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`——ProcessStaticCheck import 切新路径 + LocalToolSteps::new 传 ProcessTestExecution） — **原因**: 组合根为一行构造、无独立测试文件与独立行为面；注入缝行为经 steps_test（三参构造 + 分发臂）与 walker_test（假 runner 直驱）承载，真实装配编译期可达。
- bindings 再生成产物与前端零改动（`packages/desktop/src/types/generated/bindings.ts` 的 testExecution 变体出线、前端流程视图步骤渲染零改动直接吃） — **原因**: 生成物不建测试文件（bindings:check 守卫拦截过期生成物）；「前端零改动」为静态约束——白名单式扩展无前端源码变更面；出线的进程内半边经 state_test 承载。
- 门面模块不建独立测试文件（`crates/core/checks/src/lib.rs`、`crates/infra/checks/src/lib.rs`、`crates/core/checks/src/parser/mod.rs`） — **原因**: 纯 re-export / 类型声明无自有行为，空框架章节自相矛盾；行为经成员模块各节承载（CoverageFormat 三值路由断言落 coverage_test.rs，两 crate 导出面经 static_check_test / mod_test / corpus_golden_test 编译与组合承载）。
