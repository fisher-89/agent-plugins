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
// mutation 块族（schema 保真；生产聚合恒 `None`）
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

/// 突变测试结论块（V1 桌面不产，corpus 样本保真解析不丢块）。
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
