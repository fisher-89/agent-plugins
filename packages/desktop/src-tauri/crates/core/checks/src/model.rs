use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 枚举词汇（线格式小写词，与 CLI zod enum 同串）
// ---------------------------------------------------------------------------

/// 汇总结论三值（CLI zod `enum(['pass', 'fail', 'error'])` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Conclusion {
    Pass,
    Fail,
    Error,
}

impl Conclusion {
    /// 线格式小写词。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Error => "error",
        }
    }
}

/// 用例状态三值（CLI zod `enum(['passed', 'failed', 'skipped'])` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestCaseStatus {
    Passed,
    Failed,
    Skipped,
}

/// 问题类别三值（CLI zod `enum` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemType {
    TestFailure,
    CoverageFailure,
    ExecutionError,
}

// ---------------------------------------------------------------------------
// 用例与问题
// ---------------------------------------------------------------------------

/// 单条用例结果（CLI `testCaseResultSchema` 逐字段对齐；`errorType` /
/// `errorMessage` / `stackTrace` 线格式驼峰为 CLI 原样键名，仅失败态填充）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TestCaseResult {
    /// 用例全名（层级形式，如 "describe > it title"）
    pub name: String,
    /// 测试文件路径（相对项目根；未设为 `None`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// 执行耗时毫秒（未设 / 不适用为 `None`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    /// 状态三值
    pub status: TestCaseStatus,
    /// 失败用例行号（未设为 `None`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<f64>,
    /// 错误类型（仅失败态）
    #[serde(rename = "errorType", default, skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
    /// 错误消息（仅失败态）
    #[serde(
        rename = "errorMessage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub error_message: Option<String>,
    /// 堆栈（仅失败态）
    #[serde(
        rename = "stackTrace",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub stack_trace: Option<String>,
}

/// 问题三元：框架 + 类别 + 人读描述（CLI `problemSchema` 对齐）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Problem {
    /// 问题发生的框架（覆盖 / 突变全局问题为 "all"）
    pub framework: String,
    /// 问题类别
    #[serde(rename = "type")]
    pub problem_type: ProblemType,
    /// 人读问题描述
    pub message: String,
}

// ---------------------------------------------------------------------------
// 覆盖率块族
// ---------------------------------------------------------------------------

/// 覆盖率测量三维度（`None` = 该维度不受支持或无数据，判定按 null 感知跳过）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoverageMeasured {
    /// 行覆盖百分点
    #[serde(default)]
    pub lines: Option<f64>,
    /// 分支覆盖百分点
    #[serde(default)]
    pub branches: Option<f64>,
    /// 函数覆盖百分点
    #[serde(default)]
    pub functions: Option<f64>,
}

/// 覆盖率阈值三维度（恒有值）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoverageThresholds {
    pub lines: f64,
    pub branches: f64,
    pub functions: f64,
}

/// per-suite 覆盖率 override 组结果（CLI `coverageOverrideSchema` 对齐）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoverageOverride {
    /// 本组适用的 glob（suite root）
    pub glob: String,
    /// 本组阈值
    pub thresholds: CoverageThresholds,
    /// 本组实测
    pub measured: CoverageMeasured,
    /// 本组是否达阈
    pub pass: bool,
    /// 命中文件数
    pub file_count: u64,
    /// 逐文件达阈数
    pub passed_count: u64,
}

/// 覆盖率结论块（子报告与汇总共用；`overrides` 未产组时缺省）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoverageBlock {
    pub pass: bool,
    pub measured: CoverageMeasured,
    pub thresholds: CoverageThresholds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<CoverageOverride>>,
}

// ---------------------------------------------------------------------------
// mutation 块族（schema 保真零扩展；突变位由 MutationTest 步归并回填——本域
// 另提供段级报告解析与归并模型，fixtures 直驱可测、零 spawn 零 Tauri）
// ---------------------------------------------------------------------------

/// 突变测量计数（CLI `mutationMeasuredSchema` 逐字段对齐；驼峰键名为 CLI
/// 原样键名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct MutationMeasured {
    pub killed: u64,
    pub survived: u64,
    pub timeout: u64,
    #[serde(rename = "noCoverage")]
    pub no_coverage: u64,
    #[serde(rename = "compileError")]
    pub compile_error: u64,
    #[serde(rename = "runtimeError")]
    pub runtime_error: u64,
    pub ignored: u64,
    pub total: u64,
    pub detected: u64,
    pub undetected: u64,
}

/// per-suite 突变 override 组结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MutationOverride {
    pub glob: String,
    pub score: f64,
    pub threshold: f64,
    pub pass: bool,
    pub file_count: u64,
    pub passed_count: u64,
}

/// 突变测试结论块（由 MutationTest 步归并回填 summary 的 mutation 位；
/// corpus 样本保真解析不丢块）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MutationBlock {
    pub pass: bool,
    pub score: f64,
    pub threshold: f64,
    pub measured: MutationMeasured,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<MutationOverride>>,
}

/// 突变执行器标识两值（段级解析分支与 evidence 归因用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationTool {
    /// cargo-mutants（rust 档，`--in-diff` 行级精度）
    CargoMutants,
    /// Stryker（js 族 jest / vitest / vite-plus，`mutate` 面 = diff 文件清单）
    Stryker,
}

impl MutationTool {
    /// 线格式 snake_case 词。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CargoMutants => "cargo_mutants",
            Self::Stryker => "stryker",
        }
    }
}

/// 段级报告解析纯层模型：`mutation-seg-<i>.json`（工具原始输出）经宽容解析后
/// 的计数像。`MutationMeasured` / `MutationBlock` 既有字段**零扩展**（schema
/// 双侧对齐不变）。
#[derive(Debug, Clone, PartialEq)]
pub struct MutationSegReport {
    /// 执行器标识
    pub tool: MutationTool,
    /// 段计数（outcome 映射表归位后的测量面）
    pub measured: MutationMeasured,
    /// 段耗时毫秒（工具报告未携时 0.0）
    pub duration_ms: f64,
}

// ---------------------------------------------------------------------------
// 逐文件覆盖
// ---------------------------------------------------------------------------

/// 单文件覆盖（百分点 + 原始计数；`None` = 该维度不受支持）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FileCoverageEntry {
    /// 行覆盖百分点
    pub lines: f64,
    /// 分支覆盖百分点
    #[serde(default)]
    pub branches: Option<f64>,
    /// 函数覆盖百分点
    #[serde(default)]
    pub functions: Option<f64>,
    /// 可执行行总数
    #[serde(default)]
    pub total_lines: Option<u64>,
    /// 覆盖行数
    #[serde(default)]
    pub covered_lines: Option<u64>,
    /// 分支总数
    #[serde(default)]
    pub total_branches: Option<u64>,
    /// 覆盖分支数
    #[serde(default)]
    pub covered_branches: Option<u64>,
    /// 函数总数
    #[serde(default)]
    pub total_functions: Option<u64>,
    /// 覆盖函数数
    #[serde(default)]
    pub covered_functions: Option<u64>,
}

/// 逐文件覆盖原始计数条目（override 聚合的原始计数源）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceFileEntry {
    /// 源文件路径
    pub file: String,
    /// 覆盖计数（未采集为 `None`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<FileCoverageEntry>,
}

// ---------------------------------------------------------------------------
// 子报告与汇总报告
// ---------------------------------------------------------------------------

/// 用例计数四元（CLI `summarySchema` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CaseSummary {
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
}

/// 子报告（每 plan 一份 `report.json`；CLI `testExecutionSubReportSchema`
/// 逐字段对齐）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubReport {
    /// 测试框架标识
    pub framework: String,
    /// suite 锚点（plan root；planId 推导基准）
    pub root: String,
    /// 报告生成时刻（ISO 8601）
    pub timestamp: String,
    /// 命令退出码
    pub exit_code: i32,
    /// 执行耗时毫秒
    pub duration_ms: f64,
    /// 用例计数
    pub summary: CaseSummary,
    /// 失败 / 超时用例集
    #[serde(default)]
    pub error_cases: Vec<TestCaseResult>,
    /// 涉及的测试文件
    #[serde(default)]
    pub test_files: Vec<String>,
    /// 逐文件覆盖原始计数
    #[serde(default)]
    pub source_files: Vec<SourceFileEntry>,
    /// 覆盖率结论（采集失败为 `None`）
    #[serde(default)]
    pub coverage: Option<CoverageBlock>,
    /// 突变结论（跳过 / 不支持为 `None`）
    #[serde(default)]
    pub mutation: Option<MutationBlock>,
    /// 诊断 findings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub findings: Option<Vec<String>>,
}

/// plan 路径索引条目（summary.plans[] 元素；无状态字段，成败在子报告）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PlanIndexEntry {
    /// plan 目录 id（报告树子目录名）
    pub id: String,
    /// 测试框架标识
    pub framework: String,
    /// suite 锚点（与 planId 同基准）
    pub root: String,
    /// 工件目录路径（POSIX）
    pub path: String,
}

/// 汇总报告（`summary.json`；CLI `testExecutionSummaryReportSchema` 逐字段
/// 对齐）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SummaryReport {
    /// 工作流相位（恒 "test-execution"）
    pub phase: String,
    /// 产出本报告的命令描述
    pub command: String,
    /// 报告生成时刻（ISO 8601；复用门新鲜度判据）
    pub timestamp: String,
    /// 总耗时秒
    pub duration_seconds: f64,
    /// 聚合用例总数
    pub total: u64,
    /// 聚合通过数
    pub passed: u64,
    /// 聚合失败数
    pub failed: u64,
    /// 聚合跳过数
    pub skipped: u64,
    /// 聚合结论
    pub conclusion: Conclusion,
    /// 问题清单
    #[serde(default)]
    pub problems: Vec<Problem>,
    /// 全局覆盖结论（全部框架未采到为 `None`）
    #[serde(default)]
    pub coverage: Option<CoverageBlock>,
    /// 全局突变结论（跳过 / 不支持为 `None`）
    #[serde(default)]
    pub mutation: Option<MutationBlock>,
    /// 尝试过的 plan 路径索引
    #[serde(default)]
    pub plans: Vec<PlanIndexEntry>,
    /// 诊断 findings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub findings: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// 宽容解析入口
// ---------------------------------------------------------------------------

/// 子报告宽容解析：未知字段忽略（zod strip 语义）、可选块缺键与显式 null
/// 同处置；必需字段缺失 / 非法 `Err`（required 集与 CLI zod schema 一致）。
pub fn parse_sub_report(json: &str) -> Result<SubReport, String> {
    serde_json::from_str(json).map_err(|error| format!("子报告解析失败: {error}"))
}

/// 汇总报告宽容解析（语义同 [`parse_sub_report`]）。
pub fn parse_summary_report(json: &str) -> Result<SummaryReport, String> {
    serde_json::from_str(json).map_err(|error| format!("汇总报告解析失败: {error}"))
}

// ---------------------------------------------------------------------------
// 段级 mutation 报告宽容解析（结构面严格、词面宽容——未知 outcome 计
// runtimeError 不丢数据；原始文本全程留存段报告文件）
// ---------------------------------------------------------------------------

/// cargo-mutants 段报告宽容解析：`--json` 段原始输出（`mutants` 数组或 summary
/// 计数面）→ [`MutationSegReport`]（`tool = CargoMutants`）。结构面严格：非
/// JSON / 缺计数面字段显式 `Err`；词面宽容：未知 outcome 计 `runtimeError` 并
/// 保留原文（段报告文件全程留存）。
pub fn parse_cargo_mutants_seg(raw: &str) -> Result<MutationSegReport, String> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|error| format!("cargo-mutants 段报告解析失败: {error}"))?;
    let mut measured = MutationMeasured::default();
    match &value {
        serde_json::Value::Array(mutants) => {
            tally_cargo_mutants(mutants, &mut measured);
        }
        serde_json::Value::Object(root) => {
            let duration_ms = read_duration_ms(root);
            if let Some(mutants) = root.get("mutants").and_then(serde_json::Value::as_array) {
                tally_cargo_mutants(mutants, &mut measured);
                return Ok(finish_cargo_seg(measured, duration_ms));
            }
            let counts = root
                .get("summary")
                .and_then(serde_json::Value::as_object)
                .or_else(|| (cargo_counts_face(root)).then_some(root))
                .ok_or_else(|| {
                    "cargo-mutants 段报告缺计数面字段（无 mutants 数组 / 计数面）".to_owned()
                })?;
            tally_cargo_counts(counts, &mut measured);
            return Ok(finish_cargo_seg(measured, duration_ms));
        }
        _ => {
            return Err("cargo-mutants 段报告结构非法（顶层非对象 / 数组）".to_owned());
        }
    }
    Ok(finish_cargo_seg(measured, 0.0))
}

/// 段收尾装配（计数一致化 + 执行器标识）。
fn finish_cargo_seg(measured: MutationMeasured, duration_ms: f64) -> MutationSegReport {
    let mut measured = measured;
    normalize_detected(&mut measured);
    MutationSegReport {
        tool: MutationTool::CargoMutants,
        measured,
        duration_ms,
    }
}

/// outcome 词面归一（大小写 / `_` / `-` 不敏——`Caught` / `caught` / `NOT_MUTATED`
/// / `not-mutated` 同词）。
fn normalize_outcome_word(word: &str) -> String {
    word.to_ascii_lowercase()
        .chars()
        .filter(|ch| *ch != '_' && *ch != '-')
        .collect()
}

/// cargo-mutants mutants 数组逐条计数（outcome 映射表单点消费）。
fn tally_cargo_mutants(mutants: &[serde_json::Value], measured: &mut MutationMeasured) {
    for mutant in mutants {
        let Some(outcome) = mutant.get("outcome").and_then(serde_json::Value::as_str) else {
            // 缺 outcome 字段的条目按未知处置（计 runtimeError，宽容不丢）
            measured.runtime_error += 1;
            continue;
        };
        match normalize_outcome_word(outcome).as_str() {
            "caught" => measured.killed += 1,
            "missed" => measured.survived += 1,
            "timeout" => measured.timeout += 1,
            "unviable" => measured.compile_error += 1,
            "notmutated" => measured.ignored += 1,
            _ => measured.runtime_error += 1,
        }
    }
    measured.total += mutants.len() as u64;
}

/// 计数面存在判定（顶层对象直接携 outcome 计数键的形态）。
fn cargo_counts_face(root: &serde_json::Map<String, serde_json::Value>) -> bool {
    [
        "caught",
        "missed",
        "timeout",
        "unviable",
        "not-mutated",
        "not_mutated",
    ]
    .iter()
    .any(|key| root.contains_key(*key))
}

/// cargo-mutants 计数面直读（summary 对象 / 顶层计数键形态）。
fn tally_cargo_counts(
    counts: &serde_json::Map<String, serde_json::Value>,
    measured: &mut MutationMeasured,
) {
    let read = |keys: &[&str]| -> u64 {
        keys.iter()
            .filter_map(|key| counts.get(*key).and_then(serde_json::Value::as_u64))
            .sum()
    };
    measured.killed += read(&["caught"]);
    measured.survived += read(&["missed"]);
    measured.timeout += read(&["timeout"]);
    measured.compile_error += read(&["unviable"]);
    measured.ignored += read(&["not-mutated", "not_mutated"]);
    measured.runtime_error += read(&["runtime_error", "runtimeError"]);
    let total = read(&["total", "totalMutants"]);
    let counted: u64 = [
        measured.killed,
        measured.survived,
        measured.timeout,
        measured.compile_error,
        measured.ignored,
        measured.runtime_error,
    ]
    .iter()
    .sum();
    measured.total += if total > 0 { total } else { counted };
}

/// detected / undetected 一致化（计数求和口径：detected = killed + timeout、
/// undetected = survived + noCoverage——工具报告未显式携时按公式回填）。
fn normalize_detected(measured: &mut MutationMeasured) {
    if measured.detected == 0 {
        measured.detected = measured.killed + measured.timeout;
    }
    if measured.undetected == 0 {
        measured.undetected = measured.survived + measured.no_coverage;
    }
}

/// Stryker 段报告宽容解析：mutation 报告（`metrics` 聚合面，缺位回落 `files`
/// 逐 mutant 计数面）→ [`MutationSegReport`]（`tool = Stryker`）。结构面严格
/// （非 JSON / 两计数面俱缺 → `Err`）、词面宽容（未知 status 计 `runtimeError`）。
pub fn parse_stryker_seg(raw: &str) -> Result<MutationSegReport, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|error| format!("Stryker 段报告解析失败: {error}"))?;
    let root = value
        .as_object()
        .ok_or_else(|| "Stryker 段报告结构非法（顶层非对象）".to_owned())?;
    let duration_ms = read_duration_ms(root);
    let mut measured = MutationMeasured::default();
    let mut used = false;
    if let Some(metrics) = root.get("metrics").and_then(serde_json::Value::as_object) {
        if stryker_metrics_face(metrics) {
            measured.killed = read_count(metrics, &["killed"]);
            measured.survived = read_count(metrics, &["survived"]);
            measured.timeout = read_count(metrics, &["timeout"]);
            measured.no_coverage = read_count(metrics, &["noCoverage", "no_coverage"]);
            measured.compile_error = read_count(metrics, &["compileErrors", "compileError"]);
            measured.runtime_error = read_count(metrics, &["runtimeErrors", "runtimeError"]);
            measured.ignored = read_count(metrics, &["ignored"]);
            measured.total = read_count(metrics, &["totalMutants", "total"]);
            measured.detected = read_count(metrics, &["totalDetected"]);
            measured.undetected = read_count(metrics, &["totalUndetected"]);
            used = true;
        }
    }
    if !used {
        let Some(files) = root.get("files").and_then(serde_json::Value::as_object) else {
            return Err("Stryker 段报告缺计数面字段（无 metrics / files）".to_owned());
        };
        let mut mutant_durations: f64 = 0.0;
        for file in files.values() {
            let Some(mutants) = file.get("mutants").and_then(serde_json::Value::as_array) else {
                continue;
            };
            for mutant in mutants {
                if let Some(duration) = mutant.get("duration").and_then(serde_json::Value::as_f64) {
                    mutant_durations += duration;
                }
                let Some(status) = mutant.get("status").and_then(serde_json::Value::as_str) else {
                    measured.runtime_error += 1;
                    measured.total += 1;
                    continue;
                };
                measured.total += 1;
                match normalize_outcome_word(status).as_str() {
                    "killed" => {
                        measured.killed += 1;
                        measured.detected += 1;
                    }
                    "survived" => {
                        measured.survived += 1;
                        measured.undetected += 1;
                    }
                    "timeout" => {
                        measured.timeout += 1;
                        measured.detected += 1;
                    }
                    "nocoverage" => {
                        measured.no_coverage += 1;
                        measured.undetected += 1;
                    }
                    "compileerror" => measured.compile_error += 1,
                    "runtimeerror" => measured.runtime_error += 1,
                    "ignored" => measured.ignored += 1,
                    _ => measured.runtime_error += 1,
                }
            }
        }
        if duration_ms == 0.0 {
            return Ok(MutationSegReport {
                tool: MutationTool::Stryker,
                measured,
                duration_ms: mutant_durations,
            });
        }
    }
    Ok(MutationSegReport {
        tool: MutationTool::Stryker,
        measured,
        duration_ms,
    })
}

/// metrics 计数面存在判定（任一已知计数键在位）。
fn stryker_metrics_face(metrics: &serde_json::Map<String, serde_json::Value>) -> bool {
    [
        "killed",
        "survived",
        "timeout",
        "noCoverage",
        "compileErrors",
        "runtimeErrors",
        "ignored",
        "totalMutants",
    ]
    .iter()
    .any(|key| metrics.contains_key(*key))
}

/// 计数键族直读（多别名键取首个可读值）。
fn read_count(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> u64 {
    keys.iter()
        .filter_map(|key| map.get(*key).and_then(serde_json::Value::as_u64))
        .sum()
}

/// 段耗时毫秒读取（`duration_ms` / `durationMs` 顶层面键；未携为 0.0）。
fn read_duration_ms(root: &serde_json::Map<String, serde_json::Value>) -> f64 {
    ["duration_ms", "durationMs"]
        .iter()
        .filter_map(|key| root.get(*key).and_then(serde_json::Value::as_f64))
        .sum()
}
