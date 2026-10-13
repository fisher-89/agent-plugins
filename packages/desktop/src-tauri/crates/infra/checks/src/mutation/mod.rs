//! mutation-test 突变进程执行模块（checks 边界第三成员；testexec/ 旁新模块）。
//!
//! 主链（E 系定稿见 design）：suite 归桶（E10）→ 跳过面（`EmptyDiff` /
//! `NoExecutor` 记 evidence 不 Err）→ 分段顺序执行（静态初值 + 自适应修正）→
//! 段报告解析（`core/checks` 宽容解析 + 映射表）→ 计数归并（`build_mutation_block`
//! / `merge_mutation_blocks`）→ 逐 suite 阈值判定（全过即 pass）→ summary
//! mutation 位回填（E11 在位时）→ [`MutationTestOutcome`] 与 `feedback_text`
//! 装配；Stryker 共享 sandbox 与 cargo-mutants 共享 `CARGO_TARGET_DIR` 跨段复
//! 用、全段后一次清理。清单与补丁经 `MutationRunner::run` 入参注入——本模块
//! **零 vcs 依赖**（清单获取唯一入口在 vcs 边界）。

mod cargo_mutants;
mod report;
mod segments;
mod stryker;

#[cfg(test)]
mod mod_test;

use std::path::{Path, PathBuf};

use checks::aggregate::{build_mutation_block, derive_plan_id, merge_mutation_blocks};
use checks::model::{MutationBlock, MutationMeasured, MutationSegReport, MutationTool};
use config::{TestFramework, TestSuite};
use orchestration::port::{
    BoxToolFuture, MutationCounts, MutationRunner, MutationSkipReason, MutationTestConclusion,
    MutationTestOutcome, ToolStepOutput,
};

/// 突变执行并发除数：jobs = `max(1, available_parallelism() / MUTATION_JOBS_DIVISOR)`
///（本地突变仅占用一半内核，延续 CLI 现状口径）。
pub const MUTATION_JOBS_DIVISOR: usize = 2;

/// per-mutant 超时秒数（防单突变体挂死；与分段预算正交）。
pub const PER_MUTANT_TIMEOUT_SECS: u64 = 300;

/// 段预算目标毫秒（5 分钟/段——软目标，触发拆段的估算口径，非硬失败语义）。
pub const SEGMENT_BUDGET_MS: u64 = 300_000;

/// 段级软护栏毫秒（10 分钟——超限仅观察留痕，MUST NOT 硬失败；单文件超预算
/// 接受）。
pub const SEGMENT_GUARD_MS: u64 = 600_000;

/// 首段静态估算的单文件 mutant 密度（文件行数不可事先得知的兜底常数）。
pub const EST_MUTANTS_PER_FILE: u64 = 10;

/// 段 spawn 硬超时毫秒（防悬挂的最后护栏——[`SEGMENT_GUARD_MS`] 是软目标观察
/// 面，本值是进程级强杀线；超时 → 段中断即时反馈，不挂死执行链）。
pub const SEGMENT_SPAWN_TIMEOUT_MS: u64 = 1_800_000;

/// 段执行失败面（E5 分流）：`Fatal` = 工具不可达 / 配置不支持 / 写盘失败 →
/// runner 显式 `Err` 停给用户（与 WorkerAgent 失败同等呈现）；`Interrupted` =
/// 段中断（harness 崩 / 超时 / 未产出段报告）→ `conclusion = Error` 即时反馈
/// （双时点②，不等汇总）。
#[derive(Debug)]
pub(crate) enum SegmentFailure {
    Fatal(String),
    Interrupted(String),
}

impl std::fmt::Display for SegmentFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fatal(message) | Self::Interrupted(message) => formatter.write_str(message),
        }
    }
}

/// 突变执行并发（半 CPU；Stryker `concurrency` 与 cargo-mutants `--jobs` 同锚）。
pub fn mutation_jobs() -> usize {
    let parallelism = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    (parallelism / MUTATION_JOBS_DIVISOR).max(1)
}

/// 合并输出有界尾部（`testexec::runner` 同量级口径——段报告面归因原料；反馈边
/// 只取结论摘要，尾部留错误面归因）。
const OUTPUT_TAIL_LIMIT: usize = 500;

/// 合并输出有界尾部（末段优先——编译 / 执行错误通常在输出末段；超限省略号前缀
/// 示意截断）。
pub(crate) fn output_tail(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let trimmed = text.trim_end();
    let total = trimmed.chars().count();
    if total <= OUTPUT_TAIL_LIMIT {
        return trimmed.to_owned();
    }
    let tail: String = trimmed.chars().skip(total - OUTPUT_TAIL_LIMIT).collect();
    format!("…{tail}")
}

/// mutation-test 进程执行器（无状态，组合根按需构造）。
pub struct ProcessMutation;

impl ProcessMutation {
    /// 构造（组合根装配）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessMutation {
    fn default() -> Self {
        Self::new()
    }
}

impl MutationRunner for ProcessMutation {
    fn run(&self, root: &str, name: &str, files: &[String], patch: &str) -> BoxToolFuture {
        let root = root.to_owned();
        let name = name.to_owned();
        let files = files.to_vec();
        let patch = patch.to_owned();
        Box::pin(async move { execute(&root, &name, &files, &patch).await })
    }
}

/// 一次 mutation-test 执行全链。
async fn execute(
    root: &str,
    change: &str,
    files: &[String],
    patch: &str,
) -> Result<ToolStepOutput, String> {
    let root_path = Path::new(root);
    let report = config::load(root_path);
    let suites = &report.config.tests;
    let reports_dir = foundation::layout::change_test_reports(root_path, change);
    let buckets = bucket_suites(root_path, suites, files);

    // 跳过面①：突变面为空（diff 空清单 / 文件不落任何 suite 桶——桶外文件不突
    // 变）→ EmptyDiff（null 观察期：mutation 位保持 null，不 Err）
    if files.is_empty() || buckets.iter().all(|bucket| bucket.files.is_empty()) {
        return Ok(skip_outcome(MutationSkipReason::EmptyDiff));
    }
    // 跳过面②：可执行桶全空（suite 声明面皆无突变执行器——node-test 等）→
    // NoExecutor（该类 suite 跳过记 evidence，MUST NOT 静默无痕）
    let runnable: Vec<&SuiteBucket> = buckets
        .iter()
        .filter(|bucket| bucket.tool.is_some() && !bucket.files.is_empty())
        .collect();
    if runnable.is_empty() {
        return Ok(skip_outcome(MutationSkipReason::NoExecutor));
    }

    // 逐 suite 顺序执行（分段 / 复用门 / 归并全在 run_suite 内；段中断即时收束）
    let mut suite_results: Vec<(String, MutationBlock)> = Vec::new();
    let mut report_paths: Vec<String> = Vec::new();
    let mut stryker_dirs: Vec<PathBuf> = Vec::new();
    let mut interruption: Option<String> = None;
    for bucket in &runnable {
        let bucket = *bucket;
        if bucket.tool == Some(MutationTool::Stryker) {
            stryker_dirs.push(reports_dir.join(&bucket.plan_id).join("mutation"));
        }
        match run_suite(root_path, &reports_dir, bucket, patch, &mut report_paths).await {
            Ok(block) => suite_results.push((bucket.plan_id.clone(), block)),
            Err(SegmentFailure::Fatal(error)) => {
                cleanup_stryker_dirs(&stryker_dirs);
                return Err(error);
            }
            Err(SegmentFailure::Interrupted(error)) => {
                interruption = Some(error);
                break;
            }
        }
    }
    // sandbox 跨段复用后的全段一次清理（只清 `_stryker-tmp` 与临时 config，报告
    // 工件不触碰）
    cleanup_stryker_dirs(&stryker_dirs);

    let blocks: Vec<MutationBlock> = suite_results
        .iter()
        .map(|(_, block)| block.clone())
        .collect();

    // 双时点②中断面：不等汇总、不回填 summary（部分结论不落位）——错误摘要 +
    // 已产报告路径清单即时反馈
    if let Some(error) = interruption {
        let merged = merge_mutation_blocks(&blocks);
        let (score, threshold, counts) = match &merged {
            Some(block) => (block.score, block.threshold, counts_of(&block.measured)),
            None => (0.0, 0.0, MutationCounts::default()),
        };
        let feedback_text = format!(
            "突变执行中断：{error}\n已产报告路径：\n{}",
            bullet_paths(&report_paths)
        );
        return Ok(ToolStepOutput::MutationTest(MutationTestOutcome {
            conclusion: MutationTestConclusion::Error,
            skip: None,
            score,
            threshold,
            counts,
            report_paths,
            feedback_text,
        }));
    }

    let merged = merge_mutation_blocks(&blocks)
        .ok_or_else(|| "突变段归并空（可执行 suite 桶在位却零段块）".to_owned())?;
    // summary mutation 位回填（E11：归并块在位时回写；未跑 / 跳过保持 null）
    backfill_summary(&reports_dir, &merged)?;
    // 门禁结论以逐 suite 全过为准（风险 6：多阈值并存时与归并块首块阈值的口径差
    // 留痕观察，MUST NOT 擅自统一）
    let all_pass = suite_results.iter().all(|(_, block)| block.pass);
    let (conclusion, feedback_text) = if all_pass {
        (MutationTestConclusion::Pass, String::new())
    } else {
        (
            MutationTestConclusion::Fail,
            fail_feedback(&suite_results, &merged, &report_paths),
        )
    };
    Ok(ToolStepOutput::MutationTest(MutationTestOutcome {
        conclusion,
        skip: None,
        score: merged.score,
        threshold: merged.threshold,
        counts: counts_of(&merged.measured),
        report_paths,
        feedback_text,
    }))
}

/// suite 突变桶（E10：文件按 suite.root POSIX 前缀归桶；桶内文件构该 suite 突
/// 变面——js 族喂 Stryker `mutate`、rust 喂 `--in-diff` 切片；不落任何桶的文件
/// 不突变；suite 是阈值与执行器的归属单元）。
struct SuiteBucket {
    /// plan 目录 id（与 test-execution 报告树同式 `derive_plan_id` 派生）
    plan_id: String,
    /// 框架线格式名（jest / vitest / vite-plus / rust / …）
    framework: &'static str,
    /// 突变执行器（`None` = 框架无突变执行器——跳过记 evidence）
    tool: Option<MutationTool>,
    /// 逐 suite 阈值（`suite.mutation.score` 既有字段原样消费，schema 零扩展）
    threshold: f64,
    /// suite 锚点（POSIX 归一；`.` = 全匹配）
    root: String,
    /// 桶内文件（项目根相对 POSIX，字典序去重）
    files: Vec<String>,
    /// suite 绝对根（复用门扫描根——覆盖段内文件与 suite 测试文件）
    suite_root: PathBuf,
    /// 突变执行目录（`suite.mutation.cwd` 覆盖 → root/suite.root/suite.cwd）
    exec_dir: PathBuf,
    /// 框架配置文件绝对路径（suite `tests[].config`；未设为 `None`）
    suite_config: Option<PathBuf>,
}

/// suite 归桶（最长前缀匹配——嵌套 suite root 取最具体桶；跨 suite 文件只归一
/// 桶；配置序保序）。
fn bucket_suites(root_path: &Path, suites: &[TestSuite], files: &[String]) -> Vec<SuiteBucket> {
    let mut buckets: Vec<SuiteBucket> = suites
        .iter()
        .map(|suite| SuiteBucket {
            plan_id: derive_plan_id(&suite.root, framework_name(&suite.framework)),
            framework: framework_name(&suite.framework),
            tool: mutation_tool(&suite.framework),
            threshold: suite.mutation.score,
            root: normalize_root(&suite.root),
            files: Vec::new(),
            suite_root: root_path.join(&suite.root),
            exec_dir: suite_exec_dir(root_path, suite),
            suite_config: suite
                .config
                .as_ref()
                .map(|config| root_path.join(&suite.root).join(config)),
        })
        .collect();
    for file in files {
        let normalized = file.replace('\\', "/").trim_start_matches('/').to_owned();
        let mut best: Option<usize> = None;
        let mut best_len = 0usize;
        for (index, bucket) in buckets.iter().enumerate() {
            let matched = bucket.root == "."
                || normalized == bucket.root
                || normalized.starts_with(&format!("{}/", bucket.root));
            if matched && bucket.root.len() > best_len {
                best = Some(index);
                best_len = bucket.root.len();
            }
        }
        if let Some(index) = best {
            buckets[index].files.push(normalized);
        }
    }
    for bucket in &mut buckets {
        bucket.files.sort();
        bucket.files.dedup();
    }
    buckets
}

/// suite 锚点归一（POSIX、去尾斜杠；空 / `.` = 全匹配锚）。
fn normalize_root(root: &str) -> String {
    let normalized = root.replace('\\', "/").trim_end_matches('/').to_owned();
    if normalized.is_empty() || normalized == "." {
        ".".to_owned()
    } else {
        normalized
    }
}

/// 突变执行目录：`suite.mutation.cwd`（相对项目根，LCA 语义只服务执行目录）>
/// `root/suite.root/suite.cwd`（与 test-execution spawn cwd 同式）；`suite.cwd`
/// 为 `.`（默认值）时不再拼接——避免尾部 `/.` 伪段混入 `--prefix` 字符串面。
fn suite_exec_dir(root_path: &Path, suite: &TestSuite) -> PathBuf {
    let base = match &suite.mutation.cwd {
        Some(cwd) => root_path.join(cwd),
        None => root_path.join(&suite.root),
    };
    if suite.cwd.is_empty() || suite.cwd == "." {
        base
    } else {
        base.join(&suite.cwd)
    }
}

/// 框架枚举 → 线格式名（CLI 注册表键同串）。
fn framework_name(framework: &TestFramework) -> &'static str {
    match framework {
        TestFramework::Jest => "jest",
        TestFramework::Vitest => "vitest",
        TestFramework::VitePlus => "vite-plus",
        TestFramework::Rust => "rust",
        TestFramework::NodeTest => "node-test",
        TestFramework::Bun => "bun",
        TestFramework::Go => "go",
        TestFramework::Pytest => "pytest",
    }
}

/// 突变执行框架矩阵（AC-4）：js 族（jest / vitest / vite-plus）→ Stryker、
/// rust → cargo-mutants；node-test / bun / go / pytest 框架无突变执行器 →
/// `None`（跳过记 evidence，MUST NOT Err——「框架设计上无执行器」与「工具没装」
/// 语义不同，E5）。
fn mutation_tool(framework: &TestFramework) -> Option<MutationTool> {
    match framework {
        TestFramework::Rust => Some(MutationTool::CargoMutants),
        TestFramework::Jest | TestFramework::Vitest | TestFramework::VitePlus => {
            Some(MutationTool::Stryker)
        }
        TestFramework::NodeTest
        | TestFramework::Bun
        | TestFramework::Go
        | TestFramework::Pytest => None,
    }
}

/// 跳过面产出（结论恒 Pass——null 观察期不判 fail；score / threshold 0.0 占位，
/// `mutation: null` 保持合法）。
fn skip_outcome(skip: MutationSkipReason) -> ToolStepOutput {
    ToolStepOutput::MutationTest(MutationTestOutcome {
        conclusion: MutationTestConclusion::Pass,
        skip: Some(skip),
        score: 0.0,
        threshold: 0.0,
        counts: MutationCounts::default(),
        report_paths: Vec::new(),
        feedback_text: String::new(),
    })
}

/// 单 suite 分段顺序执行：段切分（静态初值 + 自适应修正）→ 逐段复用门 / 执行
/// → 段报告解析 → 段块归并 → 归并块写盘；已产报告路径随 `report_paths` 逐步外
/// 传（段中断时「已产报告路径清单」不随 Err 丢失——双时点②即时反馈面）。
async fn run_suite(
    root_path: &Path,
    reports_dir: &Path,
    bucket: &SuiteBucket,
    patch: &str,
    report_paths: &mut Vec<String>,
) -> Result<MutationBlock, SegmentFailure> {
    let mutation_dir = reports_dir.join(&bucket.plan_id).join("mutation");
    std::fs::create_dir_all(&mutation_dir).map_err(|error| {
        SegmentFailure::Fatal(format!(
            "突变报告目录创建失败 {}: {error}",
            report::to_posix(&mutation_dir)
        ))
    })?;
    // 段输入新鲜度（suite 根递归扫描覆盖段内文件与 suite 测试文件；报告树 / 依赖
    // 产物目录剪枝——同一 suite 各段同值）
    let newest_input = newest_input_mtime(root_path, &bucket.suite_root, reports_dir);

    let total = bucket.files.len();
    let static_size = segments::static_chunk_size(total);
    let mut rate: Option<f64> = None;
    let mut cursor = 0usize;
    let mut index = 0usize;
    let mut seg_blocks: Vec<MutationBlock> = Vec::new();
    while cursor < total {
        // 段大小：首段静态初值，后续段按上一段实测速率（秒 / mutant）自适应修正
        let size = match rate {
            Some(rate) => segments::adaptive_chunk_size(rate, total - cursor),
            None => static_size,
        }
        .clamp(1, total - cursor);
        let chunk = &bucket.files[cursor..cursor + size];
        cursor += size;
        let report_path = report::segment_report_path(&mutation_dir, index);

        // 段级复用门（E6 / AC-8）：在位且新鲜 → 零 spawn 复用，重解析进归并；解析
        // 失效（损坏 / 形态漂移）→ 失效重跑
        let seg_report = if report::segment_reusable(&report_path, newest_input) {
            std::fs::read_to_string(&report_path)
                .ok()
                .and_then(|raw| parse_seg(bucket.tool.expect("可执行桶恒有执行器"), &raw).ok())
        } else {
            None
        };
        let seg_report = match seg_report {
            Some(parsed) => parsed,
            None => {
                // 旧段报告清除——段报告是本次执行的权威信号（防旧报告被误判）
                let _ = std::fs::remove_file(&report_path);
                execute_segment(root_path, &mutation_dir, bucket, chunk, index, patch).await?
            }
        };

        // 段软护栏仅观察留痕（MUST NOT 硬失败；单文件超预算接受）
        if seg_report.duration_ms > SEGMENT_GUARD_MS as f64 {
            eprintln!(
                "[mutation] {} 段 {index} 耗时 {:.0} ms 超软护栏 {SEGMENT_GUARD_MS} ms（软目标仅观察留痕）",
                bucket.plan_id, seg_report.duration_ms
            );
        }
        // 自适应速率观测（段报告携耗时才有实测值；工具未携 → 静态初值续用——跨段
        // 复用门下速率源自段报告自身，段边界跨 run 稳定）
        if seg_report.duration_ms > 0.0 && seg_report.measured.total > 0 {
            rate = Some(seg_report.duration_ms / 1000.0 / seg_report.measured.total as f64);
        }
        report_paths.push(report::to_posix(&report_path));
        // 段当子报告（计数求和沿 aggregate_mutation 既有公式，MUST NOT 新增归并
        // 公式）；逐段块共用 suite 阈值，归并阈值取首块 = suite 阈值
        seg_blocks.push(build_mutation_block(seg_report.measured, bucket.threshold));
        index += 1;
    }

    let block = merge_mutation_blocks(&seg_blocks)
        .ok_or_else(|| SegmentFailure::Fatal("突变段块归并空（段清单非空却零段块）".to_owned()))?;
    let merged_path =
        report::write_merged_block(&mutation_dir, &block).map_err(SegmentFailure::Fatal)?;
    report_paths.push(report::to_posix(&merged_path));
    Ok(block)
}

/// 单段执行：补丁切片 / 临时 config 写盘 → spawn（超时 + 输出捕获）→ 段原始报
/// 告写盘 → 段报告解析。失败按 [`SegmentFailure`] 分流（E5）。
async fn execute_segment(
    root_path: &Path,
    mutation_dir: &Path,
    bucket: &SuiteBucket,
    chunk: &[String],
    index: usize,
    patch: &str,
) -> Result<MutationSegReport, SegmentFailure> {
    let tool = bucket.tool.expect("可执行桶恒有执行器");
    let raw = match tool {
        MutationTool::CargoMutants => {
            let patch_path = report::write_segment_patch(
                mutation_dir,
                index,
                &segments::slice_patch(patch, chunk),
            )
            .map_err(SegmentFailure::Fatal)?;
            cargo_mutants::run_segment(&patch_path, &bucket.exec_dir).await?
        }
        MutationTool::Stryker => {
            stryker::run_segment(&stryker::StrykerSegment {
                suite_cwd: &bucket.exec_dir,
                mutation_dir,
                index,
                files: chunk,
                project_root: root_path,
                framework: bucket.framework,
                suite_config: bucket.suite_config.as_deref(),
            })
            .await?
        }
    };
    // 段原始报告写盘（工具原始输出原样、全程留存；写盘失败 → 显式 Err 面）
    report::write_segment_report(mutation_dir, index, &raw).map_err(SegmentFailure::Fatal)?;
    parse_seg(tool, &raw).map_err(|error| {
        SegmentFailure::Interrupted(format!("段 {index} 报告不可解析：{error}"))
    })
}

/// 段报告解析（按执行器路由 core/checks 宽容解析 + outcome 映射表）。
fn parse_seg(tool: MutationTool, raw: &str) -> Result<MutationSegReport, String> {
    match tool {
        MutationTool::CargoMutants => checks::model::parse_cargo_mutants_seg(raw),
        MutationTool::Stryker => checks::model::parse_stryker_seg(raw),
    }
}

/// 段输入新鲜度（suite 根递归扫描——覆盖段内文件与 suite 测试文件；change 报告
/// 树与探索树剪枝，依赖 / 工具产物目录名排除——testexec 复用门口径同源）。
fn newest_input_mtime(root_path: &Path, suite_root: &Path, reports_dir: &Path) -> Option<u64> {
    let layout = foundation::layout::resolve(root_path);
    let prune = vec![
        layout.changes_root,
        layout.explores_root,
        reports_dir.to_path_buf(),
    ];
    crate::testexec::newest_input_mtime_ms(&[suite_root.to_path_buf()], &prune)
}

/// summary mutation 位回填（E11）：summary 在位（可解析对象）→ 回写归并块；不在
/// 位 / 损坏 → 不回填不崩（null 保持——null 观察期语义不变）；回写失败 → 显式
/// `Err`（写盘失败面，E5）。
fn backfill_summary(reports_dir: &Path, block: &MutationBlock) -> Result<(), String> {
    let path = reports_dir.join("summary.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(());
    };
    if !value.is_object() {
        return Ok(());
    }
    let block_value = serde_json::to_value(block)
        .map_err(|error| format!("归并块序列化失败: {error}"))?;
    value["mutation"] = block_value;
    let rewritten = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("summary 序列化失败: {error}"))?;
    std::fs::write(&path, rewritten)
        .map_err(|error| format!("summary mutation 位回写失败 {}: {error}", report::to_posix(&path)))
}

/// 未达标反馈文本（反馈边②报告指引式：score / threshold 结论摘要 + 逐 suite
/// 明细 + 原始报告路径清单；MUST NOT 内联存活突变体清单——明细由 executor 自行
/// 读报告）。
fn fail_feedback(
    suite_results: &[(String, MutationBlock)],
    merged: &MutationBlock,
    report_paths: &[String],
) -> String {
    let mut lines = vec![format!(
        "突变测试未达标（逐 suite 判定未全过）：归并 score {} < threshold {}",
        merged.score, merged.threshold
    )];
    lines.push("未达标 suite：".to_owned());
    for (plan_id, block) in suite_results.iter().filter(|(_, block)| !block.pass) {
        lines.push(format!(
            "- {plan_id}: score {} < threshold {}",
            block.score, block.threshold
        ));
    }
    lines.push("原始报告路径：".to_owned());
    lines.push(bullet_paths(report_paths));
    lines.join("\n")
}

/// 报告路径清单（POSIX；空清单记「（无）」）。
fn bullet_paths(paths: &[String]) -> String {
    if paths.is_empty() {
        return "（无）".to_owned();
    }
    paths
        .iter()
        .map(|path| format!("- {path}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 归并计数载荷转换（port 流量像 ↔ checks 测量面）。
fn counts_of(measured: &MutationMeasured) -> MutationCounts {
    MutationCounts {
        killed: measured.killed,
        survived: measured.survived,
        timeout: measured.timeout,
        compile_error: measured.compile_error,
        runtime_error: measured.runtime_error,
        ignored: measured.ignored,
        total: measured.total,
    }
}

/// Stryker sandbox 清单清理（全段后一次 / Fatal 收束时 best-effort）。
fn cleanup_stryker_dirs(dirs: &[PathBuf]) {
    for dir in dirs {
        report::cleanup_stryker_artifacts(dir);
    }
}
