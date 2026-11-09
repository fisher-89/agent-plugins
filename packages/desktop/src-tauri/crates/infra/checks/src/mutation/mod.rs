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
use foundation::cancel::CancelFlag;
use foundation::layout::ReportSlot;
use orchestration::port::{
    BoxToolFuture, MutationCounts, MutationRunner, MutationSkipReason, MutationTestConclusion,
    MutationTestOutcome, OutputRefSink, RunFault, ToolStepOutput,
};

/// 突变执行并发除数（每份 worker 占用的逻辑核数）：jobs =
/// `max(1, 可用并行度 / MUTATION_JOBS_DIVISOR)`。
///
/// 除以 4 是把每个 worker 的「编译并发 + 测试并发」一并计入——worker 不是单线程
/// 的；不再另减余量，因为编译侧的并发由 cargo-mutants 的 GNU jobserver 在所有
/// 子进程间全局封顶（默认 NCPUS），不会随 worker 数叠乘；测试侧的峰值进程数
/// `jobs × MUTATION_TEST_THREADS` 已等于逻辑核数，超出即互相饿死。
///
/// 历史：早前版本额外 `- 2` 留余量，16 线程机器上给到 3 个 worker。实测 3 / 4 / 6
/// 档端到端墙钟差异在噪声内，但 3 档的 per-mutant 余量更薄，长用例更容易撞
/// `PER_MUTANT_TIMEOUT_SECS`；改为整除口径后 16 线程 → 4。
pub const MUTATION_JOBS_DIVISOR: usize = 4;

/// 单个 worker 内的测试线程数（nextest `-j` / libtest `--test-threads`）。
///
/// 峰值测试进程 = `mutation_jobs() × MUTATION_TEST_THREADS`。两层的乘积控制在
/// 核数附近，避免「N 个 worker 各开满线程」把机器压到互相饿死（实测后果是纯函数
/// 突变体撞满 per-mutant 超时 → 假 timeout）。
pub const MUTATION_TEST_THREADS: usize = 4;

/// per-mutant 超时秒数（防单突变体挂死；与分段预算正交）。
pub const PER_MUTANT_TIMEOUT_SECS: u64 = 300;

/// 段预算目标毫秒（5 分钟/段——软目标，触发拆段的估算口径，非硬失败语义）。
pub const SEGMENT_BUDGET_MS: u64 = 300_000;

/// 段级软护栏毫秒（10 分钟——超限仅观察留痕，MUST NOT 硬失败；单文件超预算
/// 接受）。
pub const SEGMENT_GUARD_MS: u64 = 600_000;

/// 首段静态估算的单文件 mutant 密度（文件行数不可事先得知的兜底常数）。
///
/// 取值依据是本仓 rust 面的实测，取上界。**分母口径 MUST 是段内文件数**（它是
/// [`segments::static_chunk_size`] 块大小的乘数），MUST NOT 是「实际产出突变体的
/// 文件数」——后者只数有产出的文件，会把密度虚高数倍，两个口径不可混用。
pub const EST_MUTANTS_PER_FILE: u64 = 22;

/// 段 spawn 硬超时毫秒（防悬挂的**下限**护栏——[`SEGMENT_GUARD_MS`] 是软目标观察
/// 面，本值是进程级强杀线；超时 → 段中断即时反馈，不挂死执行链）。
///
/// 实际生效值由 [`segments::segment_spawn_timeout_ms`] 按段内文件数折算上抬；本常量
/// 只在短段 / 零文件段处生效。
pub const SEGMENT_SPAWN_TIMEOUT_MS: u64 = 3_600_000;

/// 段执行失败面（E5 分流）：`Fatal` = 工具不可达 / 配置不支持 / 写盘失败 →
/// runner 显式 `Err` 停给用户（与 WorkerAgent 失败同等呈现）；`Interrupted` =
/// 段中断（harness 崩 / 超时 / 未产出段报告）→ `conclusion = Error` 即时反馈
///（双时点②，不等汇总）；`Cancelled` = 用户停止（cancel 置位 → 执行体杀整树）→
/// 产出以 `fault = Some(RunFault::Stopped)` 正常返回（节点 stopped、run stopped），
/// **不走 `Fatal`**（不把用户停止呈现为「环境坏了停给用户」）、也不走
/// `conclusion = Error` + 取消文案的升格面（不把用户停止呈现为失败）。变体独立
/// （不复用 `Interrupted` 文案）便于终态面按变体直接映射。
#[derive(Debug)]
pub(crate) enum SegmentFailure {
    Fatal(String),
    Interrupted(String),
    Cancelled(String),
}

impl std::fmt::Display for SegmentFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fatal(message) | Self::Interrupted(message) | Self::Cancelled(message) => {
                formatter.write_str(message)
            }
        }
    }
}

/// 突变执行并发（cargo-mutants `--jobs`）：`max(1, 可用并行度 / 4)`——16 核 → 4。
///
/// 这个折算专门服务 **rust 桶**：每个 cargo-mutants worker 各持一份副本并各自冷编
/// 整包（实测单份 ~110 s，份数即 jobs），同时提交的测试进程数为 `jobs × 测试线程数`。
/// worker 按「每份吃 4 核」折算，把总压力钉在核数附近。
///
/// Stryker 桶**不**用本函数——它没有「每份副本各自冷编」的成本，见
/// [`stryker_concurrency`]。
pub fn mutation_jobs() -> usize {
    let parallelism = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    (parallelism / MUTATION_JOBS_DIVISOR).max(1)
}

/// Stryker 并发（同一进程内并行的测试运行器数）：半 CPU。
///
/// 与 [`mutation_jobs`] 的区别：Stryker 只有一个进程、一份 sandbox，`concurrency`
/// 就是进程内的运行器槽位数，不存在「N 份副本各冷编一遍」的乘法放大，因此按半
/// CPU 取即可（延续 CLI 现状口径）。
pub fn stryker_concurrency() -> usize {
    let parallelism = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    (parallelism / 2).max(1)
}

/// Stryker 可插桩扩展名 allowlist（镜像 `@stryker-mutator/instrumenter`
/// `getFormat` 注册表——白名单之外的扩展名插桩器抛 `Unable to parse … No
/// parser registered`，整个 Stryker run 崩溃且不产出报告）。不带点形态
/// （与 `rsplit_once('.')` 产出的扩展名直接比对）。
const STRYKER_INSTRUMENTABLE_EXTS: [&str; 12] = [
    "js", "jsx", "mjs", "cjs", "ts", "mts", "cts", "tsx", "vue", "html", "htm", "svelte",
];

/// 文件是否 Stryker 可插桩（扩展名小写比对 allowlist；无扩展名 / 未知扩展名
/// 均不可插桩）。
fn is_stryker_instrumentable(file: &str) -> bool {
    file.rsplit_once('.').is_some_and(|(_, extension)| {
        STRYKER_INSTRUMENTABLE_EXTS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// 文件是否测试文件（CLI `deriveSourceFiles` 反向口径）：路径含 `__tests__/`
/// 目录或 basename 含 `.test.` / `.spec.` 标记。测试文件是判定 oracle，不作
/// 突变对象（CLI 注释声称的 `_test.` / `test_` 标记代码未实现，不移植）。
fn is_test_file(file: &str) -> bool {
    if file.contains("__tests__/") {
        return true;
    }
    let name = file.rsplit('/').next().unwrap_or(file);
    name.contains(".test.") || name.contains(".spec.")
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
    fn run(
        &self,
        root: &str,
        name: &str,
        files: &[String],
        patch: &str,
        cancel: CancelFlag,
        on_output_ref: Option<OutputRefSink>,
    ) -> BoxToolFuture {
        let root = root.to_owned();
        let name = name.to_owned();
        let files = files.to_vec();
        let patch = patch.to_owned();
        Box::pin(async move {
            execute(&root, &name, &files, &patch, cancel, on_output_ref).await
        })
    }
}

/// 一次 mutation-test 执行全链。
///
/// **记录链**（与 static-check / test-execution 同序）：跳过面判定之后
/// `LiveExec::begin` → 逐桶 spawn 前写 `.cmd` 段头 → 上抛 `exec_id` → 各 runner
/// 的 `live` tee 边跑边落盘 → 收口以**同一 id** `commit` 权威内容（逐桶
/// 「段头 + stdout + stderr」合并流）。记录面是观测面，MUST NOT 阻断门禁主流程：
/// `begin` 失败静默降级（`exec_id` 空串 = 未记录）。
async fn execute(
    root: &str,
    change: &str,
    files: &[String],
    patch: &str,
    cancel: CancelFlag,
    on_output_ref: Option<OutputRefSink>,
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

    // 运行中记录链起点（跳过面之后：零 spawn 面 MUST NOT 留孤儿空记录，与
    // static-check「无配置直过不建 live」同口径）。**先建目录建空 `.log` 再上抛
    // exec_id**——父目录缺失会让前端 watch 订阅显式失败，运行中增量跟随因此失效。
    //
    // 降级面：`begin` 失败静默——记录面是观测面，MUST NOT 因它阻断门禁主流程；
    // 缺席时 `exec_id` 以空串回填（与 test-execution「落盘失败降级空串」同口径）。
    let live = exec::LiveExec::begin(root_path, change, ReportSlot::Test).ok();
    if let (Some(live), Some(on_ref)) = (live.as_ref(), on_output_ref.as_ref()) {
        on_ref.emit(live.exec_id());
    }
    let sink = live.as_ref().map(exec::LiveExec::sink);
    let mut merged_output: Vec<u8> = Vec::new();

    // 逐 suite 顺序执行（分段 / 复用门 / 归并全在 run_suite 内；段中断即时收束）
    let mut suite_results: Vec<(String, MutationBlock)> = Vec::new();
    let mut report_paths: Vec<String> = Vec::new();
    let mut stryker_dirs: Vec<PathBuf> = Vec::new();
    let mut interruption: Option<SegmentFailure> = None;
    // 失败桶临时面保留：失败（Interrupted / Cancelled）与 Fatal 的 Stryker 桶临时
    // config 与 sandbox 不清理，反馈面附路径供单段复现（保留粒度 = 失败桶全临时
    // 面——段 index 不上抛（`SegmentFailure` 只携 String）、sandbox 本跨段共享、
    // 清单有界且失败段 config 必在其中（段串行））；成功面照清，rust 桶无临时面
    let mut retain: Option<PathBuf> = None;
    for bucket in &runnable {
        let bucket = *bucket;
        let mutation_dir = reports_dir.join(&bucket.plan_id).join("mutation");
        if bucket.tool == Some(MutationTool::Stryker) {
            stryker_dirs.push(mutation_dir.clone());
        }
        // 命令 sidecar：spawn 之前追加一段「桶分隔头 + 展开后的命令」（纯文本，
        // 与 `.log` 同目录同 id 前缀）。段格式单点在本处——runner 只回吐命令
        // 文本，零格式化、零记录职责；写入失败静默降级（面板呈「命令未记录」）
        let on_command = |command: &str| {
            if let Some(live) = live.as_ref() {
                let _ = live.write_command(&format!(
                    "\n=== {} ({}) · files={} ===\n{command}\n",
                    bucket.plan_id,
                    bucket.framework,
                    bucket.files.len()
                ));
            }
        };
        let record = RecordFace {
            live: sink.clone(),
            on_command: Some(&on_command),
        };
        match run_suite(
            root_path,
            &reports_dir,
            bucket,
            patch,
            &mut report_paths,
            cancel.clone(),
            &record,
        )
        .await
        {
            // 记录正文按桶序累积（头在前、正文在后——与 `.cmd` 段头同序）
            Ok((block, output)) => {
                push_bucket_head(&mut merged_output, bucket);
                merged_output.extend_from_slice(&output);
                suite_results.push((bucket.plan_id.clone(), block));
            }
            Err(SegmentFailure::Fatal(error)) => {
                if bucket.tool == Some(MutationTool::Stryker) {
                    retain = Some(mutation_dir);
                }
                let retained = cleanup_stryker_dirs(&stryker_dirs, retain.as_ref());
                // 失败面不 commit：live 已把各桶字节边跑边落盘，覆盖收口会丢掉
                // 失败桶的部分输出（无头版本可读即可）
                return Err(format!("{error}{}", retained_artifacts_section(&retained)));
            }
            // 中断 / 取消同走「保留失败桶临时面 + 收束」，文案与终态面按变体分
            Err(failure @ (SegmentFailure::Interrupted(_) | SegmentFailure::Cancelled(_))) => {
                if bucket.tool == Some(MutationTool::Stryker) {
                    retain = Some(mutation_dir);
                }
                interruption = Some(failure);
                break;
            }
        }
    }
    // sandbox 跨段复用后的全段一次清理（只清 `_stryker-tmp` 与临时 config，报告
    // 工件不触碰；keep 面 = 失败桶临时面保留）
    let retained = cleanup_stryker_dirs(&stryker_dirs, retain.as_ref());
    // 中断 / 取消面不 commit（同上：保留 live 已落盘的部分输出）
    let exec_id = if interruption.is_some() {
        live.as_ref().map(|live| live.exec_id().to_owned()).unwrap_or_default()
    } else {
        commit_exec_id(live.as_ref(), &merged_output)
    };

    let blocks: Vec<MutationBlock> = suite_results
        .iter()
        .map(|(_, block)| block.clone())
        .collect();

    // 中断 / 取消同走「保留失败桶临时面 + 收束」，终态面按变体分派（取消失败面改判
    // 为 `fault = Stopped`，见下方 interruption 分支）
    if let Some(failure) = interruption {
        let merged = merge_mutation_blocks(&blocks);
        let (score, threshold, counts) = match &merged {
            Some(block) => (block.score, block.threshold, counts_of(&block.measured)),
            None => (0.0, 0.0, MutationCounts::default()),
        };
        if matches!(failure, SegmentFailure::Cancelled(_)) {
            // 用户停止 → 执行面归因（walker 收敛 stopped）：结论取 `Fail` 占位
            //（MUST NOT 取 `Error`；fault 在位时不参与门禁分派）、`feedback_text`
            // 清空（stopped 面不消费它，保留会被误读为「需修复」）；已产报告路径
            // 与失败桶临时面保留纪律不变
            return Ok(ToolStepOutput::MutationTest(MutationTestOutcome {
                conclusion: MutationTestConclusion::Fail,
                skip: None,
                score,
                threshold,
                counts,
                report_paths,
                feedback_text: String::new(),
                fault: Some(RunFault::Stopped),
                exec_id,
            }));
        }
        // 双时点②中断面：不等汇总、不回填 summary（部分结论不落位）——错误摘要 +
        // 已产报告路径清单即时反馈
        let head = "突变执行中断";
        let feedback_text = format!(
            "{head}：{failure}\n已产报告路径：\n{}{}",
            bullet_paths(&report_paths),
            retained_artifacts_section(&retained)
        );
        return Ok(ToolStepOutput::MutationTest(MutationTestOutcome {
            conclusion: MutationTestConclusion::Error,
            skip: None,
            score,
            threshold,
            counts,
            report_paths,
            feedback_text,
            fault: None,
            exec_id,
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
        fault: None,
        exec_id,
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
        // excludes 全桶消费（CLI `isFileExcluded` 同口径——任何 suite 的排除
        // glob 命中即不入任何桶，含 rust 桶）
        if checks::aggregate::is_file_excluded(&normalized, suites) {
            continue;
        }
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
            // Stryker 桶专属过滤（tool-scoped——node-test 等无执行器桶不受
            // 约束，NoExecutor 跳过面口径不变）：不可插桩扩展名 / 测试文件不
            // 进 mutate 面（不可插桩文件让插桩器整 run 崩溃；测试文件是判定
            // oracle 非突变对象）
            if buckets[index].tool == Some(MutationTool::Stryker)
                && (!is_stryker_instrumentable(&normalized) || is_test_file(&normalized))
            {
                continue;
            }
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
        fault: None,
        // 跳过面零 spawn ⇒ 无 live 记录（与 static-check「零 spawn 不建记录」同口径）
        exec_id: String::new(),
    })
}

/// 记录收口（同 id 覆盖为权威内容）：`live` 缺席（记录面降级）→ 空串。
fn commit_exec_id(live: Option<&exec::LiveExec>, merged_output: &[u8]) -> String {
    live.and_then(|live| live.commit(merged_output).ok())
        .unwrap_or_default()
}

/// 记录正文的桶分隔头（元数据面，非脚本输出；与 `.cmd` sidecar 的段头同词）。
fn push_bucket_head(merged: &mut Vec<u8>, bucket: &SuiteBucket) {
    merged.extend_from_slice(
        format!(
            "\n=== {} ({}) · files={} ===\n",
            bucket.plan_id,
            bucket.framework,
            bucket.files.len()
        )
        .as_bytes(),
    );
}

/// 记录面透传束（`execute` 建一次、逐桶共享）：tee 口交给各 runner 的
/// `run_shell`，命令回吐口交给 `execute` 组装的桶段头 sidecar。两字段同生同灭
/// ——都只在 `execute` 的 live 生命周期内有效。
struct RecordFace<'a> {
    live: Option<exec::LiveSink>,
    on_command: Option<&'a (dyn Fn(&str) + Sync)>,
}

/// 单 suite 整体执行：一次 spawn 跑完该桶的突变面（不切段）→ 段报告解析 → 块
/// 落盘；已产报告路径随 `report_paths` 外传（中断时「已产报告路径清单」不随 Err
/// 丢失——双时点②即时反馈面）。
///
/// **为何不切段**：每次 spawn 有约 210 s 的固定开销（源码树复制 + 冷编），与突变
/// 体数无关。切段省下的是「被杀时少丢一些」，代价是每段都重付固定开销——实测 25
/// 段形态下固定开销占总墙钟 45%。整体执行 + 中断时回读部分成果（见
/// [`cargo_mutants::recover_partial`]）才是正确取舍。
async fn run_suite(
    root_path: &Path,
    reports_dir: &Path,
    bucket: &SuiteBucket,
    patch: &str,
    report_paths: &mut Vec<String>,
    cancel: CancelFlag,
    record: &RecordFace<'_>,
) -> Result<(MutationBlock, Vec<u8>), SegmentFailure> {
    let mutation_dir = reports_dir.join(&bucket.plan_id).join("mutation");
    std::fs::create_dir_all(&mutation_dir).map_err(|error| {
        SegmentFailure::Fatal(format!(
            "突变报告目录创建失败 {}: {error}",
            report::to_posix(&mutation_dir)
        ))
    })?;
    // 整体复用门（取代段级门）：段报告在位且新鲜 → 零 spawn 复用，重解析进归并。
    // 改任何一个输入文件都会使它失效——这是对「重跑必复现」的正当取舍：整体执行
    // 没有段级断点，半成品由中断回读承担。
    let newest_input = newest_input_mtime(root_path, &bucket.suite_root, reports_dir);
    let report_path = report::segment_report_path(&mutation_dir, 0);

    let report = if report::segment_reusable(&report_path, newest_input) {
        std::fs::read_to_string(&report_path)
            .ok()
            .and_then(|raw| parse_seg(bucket.tool.expect("可执行桶恒有执行器"), &raw).ok())
    } else {
        None
    };
    let (report, recovered, output) = match report {
        // 复用门命中：零 spawn ⇒ 本次零输出（`.log` 正文只有段头——与
        // test-execution 复用分支「落空记录」同口径）
        Some(parsed) => (parsed, false, Vec::new()),
        None => {
            // 旧段报告清除——段报告是本次执行的权威信号（防旧报告被误判）
            let _ = std::fs::remove_file(&report_path);
            execute_suite(
                root_path,
                reports_dir,
                &mutation_dir,
                bucket,
                patch,
                cancel.clone(),
                record,
            )
            .await?
        }
    };

    // 耗时留痕：进程输出面（MUST NOT 进报告 JSON / `feedback_text` / IPC——复用
    // 门命中与实跑两态皆同）。
    let rate_note = if report.duration_ms > 0.0 && report.measured.total > 0 {
        format!(
            "{:.0} ms ⇒ {:.2} s/mutant（{}）",
            report.duration_ms,
            report.duration_ms / 1000.0 / report.measured.total as f64,
            if recovered {
                "中断回读"
            } else {
                "完整执行"
            }
        )
    } else {
        "耗时不可得（工具未携）".to_owned()
    };
    eprintln!(
        "[mutation] {}: {} 文件 / {} 突变体 / {rate_note}",
        bucket.plan_id,
        bucket.files.len(),
        report.measured.total
    );
    report_paths.push(report::to_posix(&report_path));

    let block = build_mutation_block(report.measured, bucket.threshold);
    let merged_path =
        report::write_merged_block(&mutation_dir, &block).map_err(SegmentFailure::Fatal)?;
    report_paths.push(report::to_posix(&merged_path));
    Ok((block, output))
}

/// 单次整体执行：补丁写盘 → 经 `exec::run_shell` 执行（超时 / 双管道 / 进程树
/// 击杀 / 取消单点驻 exec）→ 原始报告写盘 → 报告解析。
///
/// **中断时回读部分成果**（`recovered = true`）：cargo-mutants 只在正常退出时写
/// `outcomes.json`，被强杀时该文件缺失。但它边跑边写 `caught.txt` / `missed.txt` /
/// `timeout.txt` / `unviable.txt`（每行一个突变体），故超时 / 取消时用这四个文件
/// 合成计数面，已判完的成果不丢。合成的段报告照常写盘（留痕「本次为部分结果」）。
///
/// 失败按 [`SegmentFailure`] 分流（E5；取消走 `Cancelled` 变体，与 `Fatal` 面隔离）。
/// 回读成功时**不走失败面**——部分结果的 score 照常判定（可能因分母偏小而不达标，
/// 由阈值门禁处置，那是正确语义）。
async fn execute_suite(
    root_path: &Path,
    reports_dir: &Path,
    mutation_dir: &Path,
    bucket: &SuiteBucket,
    patch: &str,
    cancel: CancelFlag,
    record: &RecordFace<'_>,
) -> Result<(MutationSegReport, bool, Vec<u8>), SegmentFailure> {
    let tool = bucket.tool.expect("可执行桶恒有执行器");
    let (raw, recovered, output) = match tool {
        MutationTool::CargoMutants => {
            // 执行目录相对项目根的 POSIX 路径（cargo-mutants 以 spawn cwd 相对路径
            // 对源树匹配 `--in-diff`）
            let exec_rel = bucket
                .exec_dir
                .strip_prefix(root_path)
                .map(report::to_posix)
                .unwrap_or_default();
            let patch_path = report::write_segment_patch(
                mutation_dir,
                0,
                &segments::slice_patch(patch, &bucket.files, &exec_rel),
            )
            .map_err(SegmentFailure::Fatal)?;
            let mutant_count = segments::estimate_mutants(bucket.files.len());
            cargo_mutants::run_suite(
                &patch_path,
                &bucket.exec_dir,
                mutant_count,
                cancel,
                record.live.clone(),
                record.on_command,
            )
            .await?
        }
        MutationTool::Stryker => {
            let (raw, recovered, output) = stryker::run_suite(
                &stryker::StrykerSegment {
                    suite_cwd: &bucket.exec_dir,
                    mutation_dir,
                    index: 0,
                    files: &bucket.files,
                    project_root: root_path,
                    framework: bucket.framework,
                    suite_config: bucket.suite_config.as_deref(),
                },
                // 单轮测试耗时：同 plan 的 test-execution 报告（一次真实全量墙钟）
                round_duration_ms(reports_dir, &bucket.plan_id),
                cancel,
                record.live.clone(),
                record.on_command,
            )
            .await?;
            (raw, recovered, output)
        }
    };
    // 原始报告写盘（工具原始输出原样、全程留存）
    report::write_segment_report(mutation_dir, 0, &raw).map_err(SegmentFailure::Fatal)?;
    parse_seg(tool, &raw)
        .map(|parsed| (parsed, recovered, output))
        .map_err(|error| SegmentFailure::Interrupted(format!("突变报告不可解析：{error}")))
}

/// 单轮测试耗时（毫秒）：读同 plan 的 test-execution 报告
/// `<reports>/<plan_id>/report.json` 的 `duration_ms`。
///
/// 该文件由同一次 test-execution 相位产出（walker 保证 test-execution 先于
/// mutation-test），是「跑一遍完整测试要多久」的唯一权威读数。缺失 / 不可解析
/// → `None`（消费侧退回固定下限，不阻断）。
fn round_duration_ms(reports_dir: &Path, plan_id: &str) -> Option<f64> {
    let raw = std::fs::read_to_string(reports_dir.join(plan_id).join("report.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("duration_ms")
        .and_then(serde_json::Value::as_f64)
        .filter(|duration| *duration > 0.0)
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
    let block_value =
        serde_json::to_value(block).map_err(|error| format!("归并块序列化失败: {error}"))?;
    value["mutation"] = block_value;
    let rewritten = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("summary 序列化失败: {error}"))?;
    std::fs::write(&path, rewritten).map_err(|error| {
        format!(
            "summary mutation 位回写失败 {}: {error}",
            report::to_posix(&path)
        )
    })
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

/// 保留产物段（Err / 中断反馈面追加；空清单 → 空串——rust 桶失败等零保留
/// 场景段落不出现）。
fn retained_artifacts_section(retained: &[String]) -> String {
    if retained.is_empty() {
        return String::new();
    }
    format!(
        "\n保留的失败段调试产物（供单段复现；已由 .git/info/exclude 排除在归档外）：\n{}",
        bullet_paths(retained)
    )
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

/// Stryker sandbox 清单清理（全段后一次 / Fatal 收束时 best-effort；`keep` 面
/// = 失败桶临时面保留——收集在盘保留产物路径返回，供反馈面引用）。
fn cleanup_stryker_dirs(dirs: &[PathBuf], keep: Option<&PathBuf>) -> Vec<String> {
    let mut retained = Vec::new();
    for dir in dirs {
        if keep.is_some_and(|keep| keep == dir) {
            retained.extend(report::stryker_retained_artifacts(dir));
            continue;
        }
        report::cleanup_stryker_artifacts(dir);
    }
    retained
}
