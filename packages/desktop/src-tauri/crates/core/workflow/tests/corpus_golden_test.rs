//! 集成测试：db 种子语料矩阵 → 详情/列表读链投影 → golden 快照（AC-9 / AC-3）。
//!
//! 语料 = db 种子构造器（经真实 store change 域操作面：建档 / 开相 / 落账 /
//! 回跳 / 归档翻转——workflow dev-dep store）+ 运行时合成磁盘产物树 +
//! workflow.json 惰性字节样本（desktop 对 workflow.json 双向墙，字节零读取
//! 零进投影）。防线机制：每份语料跑「detail / list 全读链」，序列化为
//! pretty JSON 与入仓 golden 全量对比；db 中性状态类型不出线的线面契约
//!（缺省 null、时间戳 RFC3339 ISO）在本目录逐字节钉死。
//!
//! ## golden 重写开关
//!
//! 环境变量 `DESKTOP_GOLDEN_REWRITE=1` 时，投影不再对比而是覆写入仓 golden
//!（写入本文件的 `golden_dir()` 规范路径）。预期工作流：
//!
//! ```text
//! DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test   # 重写
//! cargo test -p workflow --test corpus_golden_test                            # 复核：与现 golden 等价
//! ```
//!
//! ## db 坏行语料注记
//!
//! fixtures/README.md 矩阵「坏行」行（库内 native_model 解码失败行）不在本
//! 文件构造：坏字节行经公共 API 不可达，直写注入依赖 native_db 裸表句柄与
//! 内部表命名（infra 内部位），workflow 测试侧不引入该依赖——该行的读侧
//! StoreError 显式记因面由 store 域测试真件节承接（store_test
//! `建档表坏行直写注入_读侧store_error显式记因不静默`：裸 redb 直写截断
//! 坏字节行 → list / find 读面显式 Err 不静默），本文件不伪造。

use std::fs;
use std::path::{Path, PathBuf};

use foundation::layout::resolve;
use orchestration::{
    finish_command, ChangeRunStatus, ChangeStepKind, ChangeStepState, ChangeStepStatus, RunRequest,
};
use store::Store;
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries::{change_detail, list_changes, ChangeDetail};
use workflow::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStatus, PhaseLogCommand, RunStartCommand, RunStatus,
};
use workflow::write::{
    archive, create, phase_next, ArchiveOutcome, CreateOutcome, InstallRun, RepoProbe,
    SessionAnchors, WorktreePort,
};

/// golden 重写开关环境变量（沿既有 harness 命名）。
const REWRITE_ENV: &str = "DESKTOP_GOLDEN_REWRITE";

/// db 种子语料全集（对应 fixtures/README.md 覆盖面矩阵；list 语料 = 列表
/// 全读链投影的混合形态语料）。
const CORPORA: &[&str] = &[
    "corpus-multi-attempt",
    "corpus-backtrack-stale",
    "corpus-slots-null",
    "corpus-document-form",
    "corpus-list-mixed",
    "corpus-worktree",
    "corpus-run-history",
    "corpus-run-interrupted",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn golden_dir() -> PathBuf {
    manifest_dir().join("tests").join("golden")
}

fn rewrite_mode() -> bool {
    std::env::var(REWRITE_ENV).is_ok_and(|value| value != "0" && !value.is_empty())
}

/// 确定性时间戳基（UTC unix millis，2024-09-22T10:13:20Z）。
const T0: i64 = 1_727_000_000_000;
fn t(n: u32) -> i64 {
    T0 + i64::from(n) * 1000
}

/// UTC 日界锚定的毫秒（`2026-MM-DD` 零点），归档分组断言的确定性来源。
fn utc_millis(year: i32, month: time::Month, day: u8) -> i64 {
    time::Date::from_calendar_date(year, month, day)
        .expect("日界应合法")
        .midnight()
        .assume_utc()
        .unix_timestamp()
        * 1000
}

// ---------------------------------------------------------------------------
// 语料装置：真实 tempfile workspace db + 真实磁盘产物树（RAII）
// ---------------------------------------------------------------------------

struct Corpus {
    root: PathBuf,
    /// worktree 落位父锚夹具根（create 建域组合的 vcs 半边以进程内假件承载，
    /// 真实 git 夹具行收 vcs-runtime crate 测试面）
    worktree_root: PathBuf,
    _db: tempfile::TempDir,
    store: Store,
    vcs: FakeVcs,
}

impl Corpus {
    /// fs 根与 db 分置两个临时目录；db 经真实 `Store::open_workspace` 打开。
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "workflow-corpus-golden-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&root);
        let db = tempfile::tempdir().expect("创建临时 db 目录失败");
        let store =
            Store::open_workspace(&db.path().join("ws.redb")).expect("打开 workspace db 失败");
        let worktree_root = std::env::temp_dir().join(format!(
            "workflow-corpus-worktrees-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&worktree_root);
        Self {
            root,
            worktree_root,
            _db: db,
            store,
            vcs: FakeVcs::new(),
        }
    }

    /// create 建域（假件 vcs + worktree 落位夹具根）。
    fn create(&self, name: &str, goal: &str) -> Result<CreateOutcome, String> {
        create(
            &self.root,
            &self.worktree_root,
            &self.store,
            &self.vcs,
            name,
            goal,
        )
    }

    /// 建档（requirement、active、created_at 固定）。
    fn seed_record(&self, name: &str) {
        self.store
            .create_change_record(ChangeStateRecord {
                name: name.to_owned(),
                workflow_type: "requirement".to_owned(),
                created_at: T0,
                status: ChangeStatus::Active,
                archived_at: None,
                active_phase: None,
                worktree: None,
                base_commit: None,
            })
            .expect("建档失败");
    }

    /// 开相 + 落账一步（attempt 事务内推导；时间戳全固定）。
    fn run_phase(
        &self,
        name: &str,
        phase: &str,
        verdict: Verdict,
        report: &str,
        checklist: Vec<ChecklistItem>,
        executor: Option<&str>,
        evaluator: Option<&str>,
        ts: i64,
    ) {
        self.store
            .start_change_phase(name, phase, ts)
            .expect("开相失败");
        self.store
            .log_change_phase(&PhaseLogCommand {
                change: name.to_owned(),
                phase: phase.to_owned(),
                verdict,
                report: report.to_owned(),
                skipped: false,
                checklist,
                executor_session_id: executor.map(str::to_owned),
                evaluator_session_id: evaluator.map(str::to_owned),
                decision_session_id: None,
                start_at: Some(ts),
                timestamp: ts,
            })
            .expect("落账失败");
    }

    /// 仅开相不落账（重开 attempt 的运行态残留——active_phase 在场）。
    fn open_only(&self, name: &str, phase: &str, ts: i64) {
        self.store
            .start_change_phase(name, phase, ts)
            .expect("开相失败");
    }

    /// run 发起落行（经 store run 域操作面——禁裸表插桩；started_at 固定，
    /// golden 确定性）。
    fn run_start(&self, name: &str, run_id: &str, started_at: i64) {
        self.store
            .start_change_run(&RunStartCommand {
                run_id: run_id.to_owned(),
                change: name.to_owned(),
                started_at,
            })
            .expect("run 发起落行失败");
    }

    /// run 收口整包落库（经 store run 域操作面）：步序列经 orchestration
    /// `finish_command` 组装——10 词汇全序列喂入、5 落库过滤单点同真实写
    /// 路径（unify-run-state-persistence 语料纪律），seq = emit 序。
    fn run_finish(
        &self,
        name: &str,
        run_id: &str,
        status: RunStatus,
        reason: &str,
        finished_at: i64,
        steps: &[ChangeStepState],
    ) {
        let request = RunRequest {
            root: self.root.to_string_lossy().into_owned(),
            change: name.to_owned(),
            run_id: run_id.to_owned(),
            auto_next_phase: true,
            started_at: finished_at,
        };
        let orchestration_status = match status {
            RunStatus::Completed => ChangeRunStatus::Completed,
            RunStatus::Stopped => ChangeRunStatus::Stopped,
            RunStatus::Failed => ChangeRunStatus::Failed,
            RunStatus::Running | RunStatus::Interrupted => {
                panic!("语料收口仅产生终态三值（interrupted 仅标定产生）")
            }
        };
        let command = finish_command(
            &request,
            orchestration_status,
            Some(reason.to_owned()),
            finished_at,
            steps,
        );
        self.store
            .finish_change_run(&command)
            .expect("run 收口落包失败");
    }

    /// 启动标定（corpus / 测试构造中断样本直调——D12 `pub` 操作面）。
    fn calibrate(&self, now: i64) {
        self.store
            .calibrate_interrupted_runs(now)
            .expect("启动标定失败");
    }

    /// 回跳落库（stale 闭包随命令下发，与写面 backtrack 同构造）。
    fn backtrack(&self, command: &BacktrackCommand) {
        self.store
            .apply_change_backtrack(command)
            .expect("回跳落库失败");
    }

    /// 在 change 目录写文件（自动建父目录）。
    fn file(&self, name: &str, rel: &str, content: &str) {
        let dir = resolve(&self.root).changes_root.join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        if let Some(parent) = dir.join(rel).parent() {
            fs::create_dir_all(parent).expect("创建子目录失败");
        }
        fs::write(dir.join(rel), content).expect("写文件失败");
    }

    /// 在 workspace 内创建目录并写入文件（列表语料的磁盘半边）。
    fn dir_with_files(&self, rel_dir: &str, files: &[(&str, &str)]) {
        let dir = self.root.join(rel_dir);
        fs::create_dir_all(&dir).expect("创建目录失败");
        for (name, content) in files {
            fs::write(dir.join(name), content).expect("写文件失败");
        }
    }

    fn detail(&self, name: &str) -> ChangeDetail {
        change_detail(&resolve(&self.root), &self.store, name)
            .unwrap_or_else(|| panic!("语料 {name} 应可定位"))
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.worktree_root);
    }
}

// ---------------------------------------------------------------------------
// 假件 vcs（WorktreePort）：probe 固定干净仓（HEAD 恒定）、add 即建目录
// （HEAD 检出的最小模拟——真实进程 git 夹具行收 vcs-runtime crate 测试面）
// ---------------------------------------------------------------------------

struct FakeVcs;

impl FakeVcs {
    fn new() -> Self {
        Self
    }
}

/// 递归复制目录内容（假件 add_worktree 的「HEAD 检出」最小模拟）。
fn mirror_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("假件建 worktree 失败: {error}"))?;
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(()); // 空主仓 → 空 worktree 检出
    };
    for entry in entries.flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            mirror_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .map(|_| ())
                .map_err(|error| format!("假件镜像检出失败: {error}"))?;
        }
    }
    Ok(())
}

impl WorktreePort for FakeVcs {
    fn probe(&self, _main_root: &Path) -> Result<RepoProbe, String> {
        Ok(RepoProbe {
            head: "0000000000000000000000000000000000000001".to_owned(),
            dirty: false,
        })
    }

    fn branch_exists(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
        Ok(false)
    }

    fn add_worktree(&self, main_root: &Path, worktree: &Path, _branch: &str) -> Result<(), String> {
        // 建目录 + 镜像主仓树（HEAD 检出的最小模拟——lockfile / 目录树用例的
        // 检出半边；真实进程 git 夹具行收 vcs-runtime crate 测试面）
        mirror_tree(main_root, worktree)
    }

    fn remove_worktree(&self, _main_root: &Path, worktree: &Path) -> Result<(), String> {
        fs::remove_dir_all(worktree).map_err(|error| format!("假件移除 worktree 失败: {error}"))
    }

    fn delete_branch(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
        Ok(())
    }

    fn run_install(&self, _worktree: &Path, _command: &str) -> Result<InstallRun, String> {
        Ok(InstallRun {
            success: true,
            summary: String::new(),
        })
    }
}

fn item(name: &str, pass: bool, evidence: &str) -> ChecklistItem {
    ChecklistItem {
        item: name.to_owned(),
        pass,
        evidence: evidence.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// 语料构造器（对应 fixtures/README.md 覆盖面矩阵四行）
// ---------------------------------------------------------------------------

/// 多 attempt 语料：同相位两轮落账（fail → 重开 attempt → pass）+ 重开
/// attempt 残留（active_phase 在场）；checklist 多条验打包键序。
fn build_multi_attempt() -> Corpus {
    let corpus = Corpus::new("multi-attempt");
    corpus.seed_record("multi-attempt");
    corpus.run_phase(
        "multi-attempt",
        "proposal",
        Verdict::Pass,
        "提案通过",
        vec![
            item("问题清晰", true, "L1-10"),
            item("范围明确", true, "四能力边界齐备"),
            item("验收可判", true, "AC-1..AC-10"),
        ],
        Some("ses-prop-exec"),
        Some("ses-prop-eval"),
        t(1),
    );
    corpus.run_phase(
        "multi-attempt",
        "dev-design",
        Verdict::Fail,
        "首轮未过",
        vec![
            item("组件表完整", false, "缺 renderers 职责"),
            item("任务可执行", true, "阶段拆解连续"),
        ],
        Some("ses-dd-exec-a1"),
        None,
        t(2),
    );
    corpus.run_phase(
        "multi-attempt",
        "dev-design",
        Verdict::Pass,
        "第二轮通过",
        vec![item("组件表完整", true, "五组件齐全")],
        Some("ses-dd-exec-a2"),
        Some("ses-dd-eval-a2"),
        t(3),
    );
    // 重开 attempt 残留：开相未落账（active_phase 在场）
    corpus.open_only("multi-attempt", "dev-design", t(4));
    // 磁盘产物树
    corpus.file(
        "multi-attempt",
        "proposal.md",
        "# 提案\n\n多 attempt 语料。",
    );
    corpus.file(
        "multi-attempt",
        "tasks.md",
        "- [x] 设计\n- [x] 实现\n- [ ] 测试\n",
    );
    corpus
}

/// backtrack stale 语料：回跳标记 + 目标最新 pass 置 stale + 下游闭包全条目
/// stale（`apply_change_backtrack`，`stale_dependents` 与写面同闭包）。
fn build_backtrack_stale() -> Corpus {
    let corpus = Corpus::new("backtrack-stale");
    corpus.seed_record("backtrack-stale");
    corpus.run_phase(
        "backtrack-stale",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    corpus.run_phase(
        "backtrack-stale",
        "dev-design",
        Verdict::Pass,
        "设计通过",
        vec![item("组件表完整", true, "五组件齐全")],
        None,
        None,
        t(2),
    );
    corpus.run_phase(
        "backtrack-stale",
        "test-design",
        Verdict::Pass,
        "测试设计通过",
        Vec::new(),
        None,
        None,
        t(3),
    );
    corpus.run_phase(
        "backtrack-stale",
        "implement",
        Verdict::Pass,
        "实现完成",
        Vec::new(),
        None,
        None,
        t(4),
    );
    corpus.run_phase(
        "backtrack-stale",
        "test-gen",
        Verdict::Pass,
        "测试生成通过",
        Vec::new(),
        None,
        None,
        t(5),
    );
    corpus.backtrack(&BacktrackCommand {
        change: "backtrack-stale".to_owned(),
        phase: "test-gen".to_owned(),
        to: "dev-design".to_owned(),
        reason: "设计返工：缺产物区组件".to_owned(),
        stale_dependents: vec![
            "test-design".to_owned(),
            "implement".to_owned(),
            "test-gen".to_owned(),
            "test-execution".to_owned(),
            "code-review".to_owned(),
            "acceptance".to_owned(),
        ],
    });
    corpus.file(
        "backtrack-stale",
        "proposal.md",
        "# 提案\n\nbacktrack stale 语料。",
    );
    corpus
}

/// 槽位全缺语料：三会话槽位全 None 的 PhaseRecord（缺省落账）+ active_phase
/// 缺席（null 留位面）。
/// 注记：`start_at=None` 形态经 store change 域操作面不可达
///（`log_change_phase` 恒回填 active start_at），本语料的 start_at 出线为
/// 开相时刻 ISO 串；start_at=null 面由 store 层列缺省语义承接（差异见变更
/// 报告）。
fn build_slots_null() -> Corpus {
    let corpus = Corpus::new("slots-null");
    corpus.seed_record("slots-null");
    corpus.run_phase(
        "slots-null",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    corpus.run_phase(
        "slots-null",
        "implement",
        Verdict::Fail,
        "首轮未过",
        vec![item("构建绿", false, "测试未过")],
        None,
        None,
        t(2),
    );
    corpus.file("slots-null", "design.md", "# 设计\n\n槽位全缺语料。");
    corpus
}

/// 文档形态语料：磁盘产物树（proposal / design / tasks / reports）+
/// workflow.json 惰性字节样本、db 零记录 → 空流水线 + 产物清单，字节零进
/// 投影。
fn build_document_form() -> Corpus {
    let corpus = Corpus::new("document-form");
    corpus.file(
        "document-form",
        "proposal.md",
        "# 存量提案\n\nCLI 时代产物。",
    );
    corpus.file("document-form", "design.md", "# 设计\n\n零 db 记录。");
    corpus.file("document-form", "tasks.md", "- [x] 迁移\n- [ ] 收尾\n");
    corpus.file(
        "document-form",
        "reports/test/report.md",
        "# 测试报告\n\n全绿。\n",
    );
    corpus.file(
        "document-form",
        "workflow.json",
        "{ \"LAZY_BYTES_MARKER\": \"desktop 不解析此字节，仅目录存在即 change\" }",
    );
    corpus
}

/// 列表混合形态语料：db active（运行态 + 磁盘目录在场）+ db archived +
/// 磁盘-only active + 磁盘 archive（带前缀 / 无前缀）→ `list_changes` 全读
/// 链投影。
fn build_list_mixed() -> Corpus {
    let corpus = Corpus::new("list-mixed");
    // db active（带 active_phase 运行态 + 磁盘目录在场 → active 归组）
    corpus.seed_record("list-active");
    corpus
        .store
        .start_change_phase("list-active", "implement", t(6))
        .expect("开相失败");
    corpus.dir_with_files("openspec/changes/list-active", &[("proposal.md", "# 提案")]);
    // db archived（archived_at 锚定 2026-05-20）
    corpus.seed_record("list-archived");
    corpus
        .store
        .set_change_archived("list-archived", utc_millis(2026, time::Month::May, 20))
        .expect("归档翻转失败");
    // 磁盘-only active（文档形态）
    corpus.dir_with_files("openspec/changes/disk-only", &[("proposal.md", "# 存量")]);
    // 磁盘 archive（日期前缀，db 缺记录）
    corpus.dir_with_files(
        "openspec/changes/archive/2026-09-15-disk-archived",
        &[("proposal.md", "# 归档")],
    );
    // 磁盘 archive（无日期前缀 → 未知时间组）
    corpus.dir_with_files(
        "openspec/changes/archive/unknown-date-archived",
        &[("proposal.md", "# 无前缀归档")],
    );
    // worktree 条目（D11 / AC-9）：db active + worktree 执行锚 + 主仓两树未
    // 命中 → 进行中组（目录名 = 建档名——不丢弃不误归未知时间组）
    let wt_entry = ChangeStateRecord {
        name: "list-worktree".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: T0,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: Some("<WORKTREE_ROOT>/list-worktree".to_owned()),
        base_commit: Some("0000000000000000000000000000000000000001".to_owned()),
    };
    corpus
        .store
        .create_change_record(wt_entry)
        .expect("worktree 条目建档失败");
    corpus
}

/// worktree 建档语料（spec desktop-corpus-regression「worktree 两态」行）：
/// 建档携 worktree 执行锚（占位路径经投影归一映射到真实 tempdir 目录）+
/// base_commit 固定 sha + worktree 内磁盘产物树（merge 前主仓两树未命中）；
/// detail 全读链投影经路径归一（`<WORKTREE_ROOT>` 占位）保持 golden 确定性
///——投影只消费记录字段与目录树，不依赖真实 git。
fn build_worktree() -> Corpus {
    let corpus = Corpus::new("worktree");
    // worktree 内产物树（changes_root 下 change 目录 + 产物文件）
    let worktree = corpus.worktree_root.join("corpus-worktree");
    let change_dir = resolve(&worktree).changes_root.join("corpus-worktree");
    fs::create_dir_all(&change_dir).expect("创建 worktree change 目录失败");
    fs::write(
        change_dir.join("proposal.md"),
        "# worktree 内提案

merge 前主仓两树未命中的产物形态。",
    )
    .expect("写 worktree 产物失败");
    fs::write(
        change_dir.join("tasks.md"),
        "- [x] 建域
- [ ] 实现
",
    )
    .expect("写 worktree 产物失败");
    // 建档（created_at 固定 T0——golden 确定性；worktree 执行锚经归一投影）
    corpus
        .store
        .create_change_record(ChangeStateRecord {
            name: "corpus-worktree".to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: T0,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: Some(worktree.to_string_lossy().into_owned()),
            base_commit: Some("0000000000000000000000000000000000000001".to_owned()),
        })
        .expect("worktree 建档失败");
    corpus.run_phase(
        "corpus-worktree",
        "proposal",
        Verdict::Pass,
        "提案通过",
        vec![item("worktree 出线", true, "detail.worktree 与记录一致")],
        Some("ses-wt-exec"),
        Some("ses-wt-eval"),
        t(1),
    );
    corpus
}

/// run 运行史全史语料（unify-run-state-persistence）：两次 run 全史留存——
/// run-1 五词汇步整包（sessionId / detail 有无两态；attempt 1）+ run-2 续走
/// 推进（attempt 跨 run 递增 = 2、executor 单步无会话缺省落账）。流程面步骤
/// 与三门经全词汇序列喂 `finish_command`（语料纪律：缺席断言留测试相位）。
fn build_run_history() -> Corpus {
    let corpus = Corpus::new("run-history");
    corpus.seed_record("run-history");
    // 前置：proposal 已 pass（run-2 续走锚点推进的事实源）
    corpus.run_phase(
        "run-history",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    // run-1：dev-design attempt 1 五词汇全整包（三门 / 相位机步同序列喂入）
    corpus.run_start("run-history", "run-1727000010000", t(10));
    corpus.run_finish(
        "run-history",
        "run-1727000010000",
        RunStatus::Completed,
        "首轮 dev-design 未过（预算内 fail）",
        t(19),
        &[
            step(
                "implement",
                1,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-impl-1"),
                Some("实现完成"),
            ),
            step(
                "implement",
                1,
                ChangeStepKind::StaticCheck,
                ChangeStepStatus::Passed,
                None,
                None,
            ),
            step(
                "implement",
                1,
                ChangeStepKind::Evaluator,
                ChangeStepStatus::Failed,
                Some("ses-eval-1"),
                Some("首轮评估 fail"),
            ),
            step(
                "implement",
                1,
                ChangeStepKind::Decision,
                ChangeStepStatus::Passed,
                Some("ses-decision-1"),
                None,
            ),
            step(
                "dev-design",
                1,
                ChangeStepKind::TestExecution,
                ChangeStepStatus::Passed,
                None,
                Some("conclusion=pass total=3"),
            ),
        ],
    );
    // run-2：dev-design attempt 2（phase_start max+1 跨 run 递增）续走推进
    corpus.run_phase(
        "run-history",
        "dev-design",
        Verdict::Pass,
        "第二轮通过",
        vec![item("组件表完整", true, "五组件齐全")],
        Some("ses-dd-exec-a2"),
        Some("ses-dd-eval-a2"),
        t(21),
    );
    corpus.run_start("run-history", "run-1727000020000", t(20));
    corpus.run_finish(
        "run-history",
        "run-1727000020000",
        RunStatus::Completed,
        "All phases have passed evaluation. Ready for archiving.",
        t(29),
        &[step(
            "dev-design",
            2,
            ChangeStepKind::Executor,
            ChangeStepStatus::Passed,
            Some("ses-dd-exec-a2"),
            None,
        )],
    );
    corpus
}

/// interrupted 标定语料（unify-run-state-persistence）：run 发起后中途死亡
///（残留 running 行 + active_phase 悬挂）→ 启动标定翻 interrupted（记因附
/// 中断语境）+ active_phase 清位（悬挂杀除）。
fn build_run_interrupted() -> Corpus {
    let corpus = Corpus::new("run-interrupted");
    corpus.seed_record("run-interrupted");
    corpus.run_start("run-interrupted", "run-1727000005000", t(5));
    // 中途死亡形态：开相未落账（active_phase 悬挂在位）
    corpus.open_only("run-interrupted", "implement", t(6));
    // 重启装配：启动标定（open_workspace 内嵌同路径——直调操作面构造样本）
    corpus.calibrate(t(9));
    corpus
}

/// 全词汇步状态行 fixture（run 维度种子构造器的 ChangeStepState 组装面）。
#[allow(clippy::too_many_arguments)]
fn step(
    phase: &str,
    attempt: u32,
    kind: ChangeStepKind,
    status: ChangeStepStatus,
    session_id: Option<&str>,
    detail: Option<&str>,
) -> ChangeStepState {
    ChangeStepState {
        phase: phase.to_owned(),
        attempt,
        step: kind,
        status,
        session_id: session_id.map(str::to_owned),
        detail: detail.map(str::to_owned),
    }
}

// ---------------------------------------------------------------------------
// golden harness（沿既有 DESKTOP_GOLDEN_REWRITE 显式重写流程）
// ---------------------------------------------------------------------------

/// 对比或覆写单份 golden。
fn check_or_rewrite(golden_name: &str, projection: &serde_json::Value) {
    let normalized = format!(
        "{}\n",
        serde_json::to_string_pretty(projection).expect("投影序列化失败")
    );
    let golden_path = golden_dir().join(format!("{golden_name}.json"));
    if rewrite_mode() {
        fs::create_dir_all(golden_dir()).expect("创建 golden 目录失败");
        fs::write(&golden_path, &normalized).expect("写 golden 失败");
        return;
    }
    let expected = fs::read_to_string(&golden_path).unwrap_or_else(|err| {
        panic!("golden {golden_name}.json 缺失（先以 {REWRITE_ENV}=1 生成）: {err}")
    });
    assert_eq!(
        normalized, expected,
        "语料 {golden_name} 的投影与 golden 漂移；若为有意的线面演进，以 {REWRITE_ENV}=1 重写并 review diff"
    );
}

/// detail 语料的投影：change_detail 全读链输出。
fn project_detail(corpus: &Corpus, name: &str) -> serde_json::Value {
    let detail = corpus.detail(name);
    serde_json::to_value(&detail).expect("ChangeDetail 序列化失败")
}

/// list 语料的投影：list_changes 全读链输出。
fn project_list(corpus: &Corpus) -> serde_json::Value {
    let list = list_changes(&resolve(&corpus.root), &corpus.store);
    serde_json::to_value(&list).expect("ChangeList 序列化失败")
}

// ---------------------------------------------------------------------------
// 语料 golden 对拍（AC-9 矩阵）
// ---------------------------------------------------------------------------

/// 多 attempt 语料：同相位两轮落账 + 重开 attempt 种子 → detail golden 重组
/// 逐字段一致（attempt 升序、checklist 打包键序、created_at / start_at ISO
/// 出线）。
#[test]
fn corpus多attempt语料_golden对拍() {
    let corpus = build_multi_attempt();
    check_or_rewrite(
        "corpus-multi-attempt",
        &project_detail(&corpus, "multi-attempt"),
    );
}

/// backtrack stale 语料：回跳 + dependents 闭包翻转种子 → stale 位与
/// backtrack_to / backtrack_reason 字段 golden 一致（AC-9 矩阵）。
#[test]
fn corpusbacktrack_stale语料_golden对拍() {
    let corpus = build_backtrack_stale();
    check_or_rewrite(
        "corpus-backtrack-stale",
        &project_detail(&corpus, "backtrack-stale"),
    );
}

/// 槽位全缺语料：三会话槽位全 None + active_phase 缺席种子 → 槽位三列 null
/// / 时间 ISO 出线的 golden 一致（冻结契约「null 留位」半边）。
#[test]
fn corpus槽位全缺语料_golden对拍() {
    let corpus = build_slots_null();
    check_or_rewrite("corpus-slots-null", &project_detail(&corpus, "slots-null"));
}

/// 文档形态语料：磁盘产物树 + workflow.json 惰性字节样本、db 零记录 → 空流
/// 水线 + 产物清单 golden，字节零进投影（AC-9 / AC-3 矩阵）。
#[test]
fn corpus文档形态语料_golden对拍() {
    let corpus = build_document_form();
    let projection = project_detail(&corpus, "document-form");
    // 字节零进投影：序列化全量不含惰性样本标记
    let serialized = serde_json::to_string(&projection).expect("序列化应成功");
    assert!(
        !serialized.contains("LAZY_BYTES_MARKER"),
        "workflow.json 惰性字节零进投影"
    );
    check_or_rewrite("corpus-document-form", &projection);
}

/// 列表混合形态语料：db ∪ 磁盘并集 + 按月分组全读链 → ChangeList golden
///（列表半边的 golden 守卫，沿既有 layout golden 职能迁入）。
#[test]
fn corpus列表混合语料_golden对拍() {
    let corpus = build_list_mixed();
    check_or_rewrite("corpus-list-mixed", &project_list(&corpus));
}

/// worktree 建档语料（D16 ②）：worktree 执行锚 + worktree 内产物树 → detail
/// 全读链投影 golden——`worktree` 出线与库内记录逐字一致（归一占位）、
/// artifacts 命中 worktree 树（主仓两树未命中）。
#[test]
fn corpusworktree建档语料_golden对拍() {
    let corpus = build_worktree();
    let projection = project_worktree_detail(&corpus);
    // worktree 出线与库内记录逐字一致（归一前缀下的记录值对照）
    let record = corpus
        .store
        .find_change_record("corpus-worktree")
        .expect("查档应成功")
        .expect("建档在案");
    let expected = record
        .worktree
        .as_deref()
        .expect("记录携 worktree")
        .replace(
            corpus.worktree_root.to_string_lossy().as_ref(),
            "<WORKTREE_ROOT>",
        );
    assert_eq!(
        projection.get("worktree").and_then(|value| value.as_str()),
        Some(expected.as_str()),
        "worktree 出线与库内记录逐字一致（占位归一）"
    );
    // artifacts 命中 worktree 树（产物清单自 worktree 目录发现）
    let sources: Vec<&str> = projection["artifacts"]
        .as_array()
        .expect("产物清单为数组")
        .iter()
        .filter_map(|artifact| artifact["source"].as_str())
        .collect();
    assert!(
        sources.contains(&"proposal.md") && sources.contains(&"tasks.md"),
        "artifacts 命中 worktree 内文件，实际: {sources:?}"
    );
    check_or_rewrite("corpus-worktree", &projection);
}

/// run 运行史全史语料：两次 run 全史 → detail `runs` / `steps` 投影 golden
///（全史不截、emit seq 稳定序、五词汇封闭集、时间戳 ISO 出线——AC-11 矩阵）；
/// steps 投影恰五词汇封闭集缺席断言（RunStepKind 类型封闭使流程面词汇结构
/// 不可表达——词汇集钉死随快照常驻，AC-3/AC-11 缺席断言）。
#[test]
fn corpusrun全史语料_golden对拍() {
    let corpus = build_run_history();
    let projection = project_detail(&corpus, "run-history");

    // 缺席断言：runs[].steps[].step 全量限于五值封闭集（流程面步骤与三门零行
    // ——run-1 种子含三门与相位机步喂入，落库面结构上收不到）
    const CLOSED_SET: [&str; 5] = [
        "executor",
        "evaluator",
        "decision",
        "static_check",
        "test_execution",
    ];
    let runs = projection["runs"].as_array().expect("runs 为数组");
    assert_eq!(runs.len(), 2, "两 run 全史样本");
    let mut steps_seen = 0usize;
    for run in runs {
        for step_row in run["steps"].as_array().expect("steps 为数组") {
            let word = step_row["step"].as_str().expect("step 出线词");
            assert!(
                CLOSED_SET.contains(&word),
                "落库步词汇 {word} 越封闭集（流程面 / 三门缺席断言被破坏）"
            );
            steps_seen += 1;
        }
    }
    assert!(steps_seen >= 6, "五词汇样本步整包非空，实际: {steps_seen}");

    check_or_rewrite("corpus-run-history", &projection);
}

/// interrupted 标定语料：残留 running + 悬挂 active_phase → 标定后 runs 投影
/// golden（status=interrupted、记因附中断语境、finished_at 标定时刻、
/// activePhase 清位 null——AC-4 / AC-5 矩阵半边）。
#[test]
fn corpusrun中断标定语料_golden对拍() {
    let corpus = build_run_interrupted();
    check_or_rewrite(
        "corpus-run-interrupted",
        &project_detail(&corpus, "run-interrupted"),
    );
}

/// worktree 语料投影：detail 全读链 + 路径归一（tempdir 绝对路径前缀 →
/// `<WORKTREE_ROOT>` 固定占位——golden 确定性；归一只作用 worktree 执行锚
/// 字符串值，其余投影零改写）。
fn project_worktree_detail(corpus: &Corpus) -> serde_json::Value {
    let mut value =
        serde_json::to_value(&corpus.detail("corpus-worktree")).expect("ChangeDetail 序列化失败");
    if let Some(worktree) = value
        .get("worktree")
        .and_then(|worktree| worktree.as_str())
        .map(str::to_owned)
    {
        let normalized = worktree.replace(
            corpus.worktree_root.to_string_lossy().as_ref(),
            "<WORKTREE_ROOT>",
        );
        value["worktree"] = serde_json::Value::String(normalized);
    }
    value
}

// ---------------------------------------------------------------------------
// 显式重写流程的进程内断言（AC-9 diff 范围键集）+ 语料完整性
// ---------------------------------------------------------------------------

/// 递归断言投影不含退役键（`inventory` / `fileLog` / `unparsable`——detail
/// 线面三字段删除的 golden 面）。
fn assert_no_retired_keys(value: &serde_json::Value, path: &str) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                assert!(
                    !matches!(key.as_str(), "inventory" | "fileLog" | "unparsable"),
                    "golden {path} 含退役键 {key}（线面三字段删除被破坏）"
                );
                assert_no_retired_keys(child, path);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                assert_no_retired_keys(child, path);
            }
        }
        _ => {}
    }
}

/// 显式重写流程（边界行）：重写后重跑绿的 diff 范围进程内断言——detail 段
/// 无 `inventory` / `fileLog` / `unparsable` 三退役键、`status` / `activePhase`
/// 状态面键在场（文档形态为 null）。
#[test]
fn corpusgolden重写后_diff范围键集断言() {
    if rewrite_mode() {
        // 重写模式正在覆写 golden，键集断言延至复核轮
        return;
    }
    for name in [
        "corpus-multi-attempt",
        "corpus-backtrack-stale",
        "corpus-slots-null",
        "corpus-run-history",
        "corpus-run-interrupted",
    ] {
        let text = fs::read_to_string(golden_dir().join(format!("{name}.json")))
            .unwrap_or_else(|err| panic!("golden {name}.json 应存在: {err}"));
        let value: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("golden {name}.json 应为合法 JSON: {err}"));

        assert_no_retired_keys(&value, name);
        assert_eq!(
            value.get("status").and_then(|status| status.as_str()),
            Some("active"),
            "{name} 状态面键出线（建档判别面）"
        );
        assert!(
            value.get("activePhase").is_some(),
            "{name} activePhase 键恒在场（null 不省键）"
        );
        assert!(
            value.get("worktree").is_some(),
            "{name} worktree 键恒在场（legacy 建档样本 null 留位——防静默漂移）"
        );
        assert_eq!(
            value.get("worktree"),
            Some(&serde_json::Value::Null),
            "{name} legacy 建档样本 worktree 出线 null（D16 ①）"
        );
        assert!(
            value.get("runs").is_some(),
            "{name} runs 键恒在场（run 运行史读面——null 不省键）"
        );
        // 尝试序列键恒在场（AttemptRecord 形状不变；语料无落账条目时跳过——
        // run-interrupted 仅建档 + run 残留，零 PhaseRecord）
        let first_attempt = &value["pipeline"][0]["attempts"][0];
        if !value["pipeline"][0]["attempts"]
            .as_array()
            .is_some_and(Vec::is_empty)
        {
            for key in ["startAt", "timestamp", "executorSessionId"] {
                assert!(
                    first_attempt.get(key).is_some(),
                    "{name} attempt 键 {key} 恒在场"
                );
            }
        }
    }

    // 文档形态语料：status / activePhase 为 null 留位
    let text = fs::read_to_string(golden_dir().join("corpus-document-form.json"))
        .expect("document-form golden 应存在");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("document-form golden 应为合法 JSON");
    assert_no_retired_keys(&value, "corpus-document-form");
    assert_eq!(value.get("status"), Some(&serde_json::Value::Null));
    assert_eq!(value.get("activePhase"), Some(&serde_json::Value::Null));
    assert_eq!(
        value.get("worktree"),
        Some(&serde_json::Value::Null),
        "文档形态 worktree 键 null 留位（恒在场）"
    );
    assert!(
        value
            .get("pipeline")
            .and_then(|pipeline| pipeline.as_array())
            .is_some_and(Vec::is_empty),
        "文档形态空流水线"
    );

    // worktree 建档语料（非 null 投影——两态齐备的对拍半边；D16 ②）
    let text = fs::read_to_string(golden_dir().join("corpus-worktree.json"))
        .expect("corpus-worktree golden 应存在");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("corpus-worktree golden 应为合法 JSON");
    assert_no_retired_keys(&value, "corpus-worktree");
    assert_eq!(
        value.get("worktree").and_then(|worktree| worktree.as_str()),
        Some(if cfg!(windows) {
            r"<WORKTREE_ROOT>\corpus-worktree"
        } else {
            "<WORKTREE_ROOT>/corpus-worktree"
        }),
        "worktree 建档样本出线执行锚（占位归一——非 null 投影）"
    );
    assert_eq!(
        value.get("status").and_then(|status| status.as_str()),
        Some("active"),
        "worktree 建档样本状态面在"
    );
}

/// 语料完整性：golden 目录与语料集合一致（防快照被静默删减 / 陈旧快照滞留）。
#[test]
fn 语料完整性_golden目录与语料集合一致() {
    if rewrite_mode() {
        // 重写模式下 golden 目录正在被覆写，跳过该一致性断言（复核轮再验）
        return;
    }
    let mut expected: Vec<String> = CORPORA.iter().map(|name| format!("{name}.json")).collect();
    expected.push("README.md".to_string());
    expected.sort();

    let mut actual: Vec<String> = fs::read_dir(golden_dir())
        .expect("golden 目录应存在（先以 DESKTOP_GOLDEN_REWRITE=1 生成）")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    assert_eq!(actual, expected, "golden 文件集应与语料集合一致");
}

/// 语料说明文档在位（fixtures/README.md 的 db 种子语料矩阵语义由本文件
/// 全链用例承载）。
#[test]
fn 语料完整性_fixtures说明文档在位() {
    let readme = manifest_dir()
        .join("tests")
        .join("fixtures")
        .join("README.md");
    assert!(readme.is_file(), "tests/fixtures/README.md 应存在");
    let text = fs::read_to_string(&readme).expect("fixtures README 应可读");
    for row in [
        "多 attempt",
        "backtrack stale",
        "槽位全缺",
        "文档形态",
        "坏行",
        "worktree 两态",
    ] {
        assert!(
            text.contains(row),
            "fixtures/README.md 覆盖面矩阵应含「{row}」行（矩阵语义与语料集合对账）"
        );
    }
}

// ---------------------------------------------------------------------------
// db 真件组合面：test-design 写面 / 查询面各节的「真实 tempfile Store + 真实
// tempdir fs」行收本集成面——workflow 的 dev-dep store 成环（store 普通 dep →
// workflow）在 lib-test 与普通 lib 双工件下类型不统一（cargo/rustc 限制：
// `&Store` 无法满足 lib-test 视角的 `dyn ChangeStateStore`），真实 db 组合
// 只能在集成目标统一；单元共置面（*_test.rs）以进程内假件承载 port 校验
// 行为，本节承载 db 持久化语义与「零 workflow.json 触点」读墙。
// ---------------------------------------------------------------------------

/// 当前 UTC 日历日期 `YYYY-MM-DD`（「当日」断言的界用，防御恰跨 UTC 午夜）。
fn utc_date_today() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

fn collect_file_names(root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_file_names(&entry.path(), out);
        } else {
            out.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
}

/// create 建域四段（真件 db 半边 + 假件 vcs 半边）：worktree 内目录树 +
/// explore.md（goal 原文）+ ChangeRecord 建档（携 worktree / base_commit）；
/// 主仓 active 树不含该目录；目录树零 workflow.json 产出（双向墙写半边）。
#[test]
fn 真件create建域_worktree内目录与explore与db建档且主仓零目录() {
    let corpus = Corpus::new("create-triple");
    let layout = resolve(&corpus.root);

    let outcome: CreateOutcome = corpus
        .create("create-triple", "三合一 goal 正文")
        .expect("建档应成功");

    assert_eq!(outcome.name, "create-triple");
    let worktree = corpus.worktree_root.join("create-triple");
    assert_eq!(
        outcome.worktree,
        worktree.to_string_lossy(),
        "worktree 出线"
    );
    let dir = resolve(&worktree).changes_root.join("create-triple");
    assert!(dir.is_dir(), "worktree 内 change 目录创建");
    assert_eq!(
        fs::read(dir.join("explore.md")).expect("读 explore.md 失败"),
        "三合一 goal 正文".as_bytes(),
        "explore.md 落 goal 原文"
    );
    let record = corpus
        .store
        .find_change_record("create-triple")
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.workflow_type, "requirement");
    assert_eq!(record.status, ChangeStatus::Active);
    assert_eq!(
        record.worktree.as_deref(),
        Some(worktree.to_string_lossy().as_ref()),
        "建档记录携 worktree 执行锚"
    );
    assert_eq!(
        record.base_commit.as_deref(),
        Some("0000000000000000000000000000000000000001"),
        "建档记录携 base_commit 基线"
    );
    assert!(
        !layout.changes_root.join("create-triple").exists(),
        "主仓 active 树不含该目录"
    );

    let mut files = Vec::new();
    collect_file_names(&resolve(&worktree).changes_root, &mut files);
    assert!(
        !files.iter().any(|name| name == "workflow.json"),
        "目录树零 workflow.json 产出，实际: {files:?}"
    );
}

/// create 冲突双检查前置：目录已存在 / db 同名 active 各显式 `Err` 且对侧
/// 载体零副作用（D5 全 IO 前置）。
#[test]
fn 真件create冲突双检查前置零副作用() {
    let corpus = Corpus::new("create-conflict");
    let layout = resolve(&corpus.root);

    // 目录已存在 → Err 且 db 零建档
    fs::create_dir_all(layout.changes_root.join("taken")).expect("预置目录失败");
    let error = corpus
        .create("taken", "新建 goal")
        .expect_err("同名目录应 Err");
    assert!(error.contains("已存在"), "实际: {error}");
    assert!(
        corpus
            .store
            .list_change_records()
            .expect("db 读应成功")
            .is_empty(),
        "db 零建档"
    );

    // db 同名 active → Err 且零目录创建
    corpus.seed_record("db-taken");
    let error = corpus
        .create("db-taken", "新建 goal")
        .expect_err("同名 active 应 Err");
    assert!(error.contains("已存在"), "实际: {error}");
    assert!(
        !layout.changes_root.join("db-taken").exists(),
        "零目录创建（检查全 IO 前置）"
    );
}

/// create fs 失败补偿（D5）：预置 changes_root 为文件占位注入真实写失败
///（create_dir_all 对文件路径分量必然失败；目录位占位会先被「目录已存在」
/// 前置检查拦截）→ 补偿删除本次建档；重开 db 零本次残留（重开前先 drop
/// 既有句柄——redb 单进程单句柄，双开不保证安全）。
#[test]
fn 真件create_fs失败补偿_重开db零残留() {
    let ws = tempfile::tempdir().expect("创建临时 workspace 根失败");
    let db = tempfile::tempdir().expect("创建临时 db 目录失败");
    let db_path = db.path().join("ws.redb");
    // worktree 落位父锚独立 tempdir（嵌在主仓根内会使假件 mirror_tree 自我
    // 递归——主仓树包含 worktrees 子树自身）
    let worktree_anchor = tempfile::tempdir().expect("创建 worktree 锚临时目录失败");
    let worktree_root = worktree_anchor.path().to_path_buf();
    // 占位打在主仓 openspec/changes 路径分量上（假件 add 镜像检出携入
    // worktree，目录树 / explore.md 写出段真实失败；直接预置 worktree 会先
    // 被「worktree 目录已存在」前置拦截）
    fs::create_dir_all(ws.path().join("openspec")).expect("预置主仓域根失败");
    fs::write(
        ws.path().join("openspec").join("changes"),
        "changes 文件占位",
    )
    .expect("预置占位失败");

    {
        let store = Store::open_workspace(&db_path).expect("打开 workspace db 失败");
        let error = create(
            ws.path(),
            &worktree_root,
            &store,
            &FakeVcs::new(),
            "fix-bug",
            "补偿路径 goal",
        )
        .expect_err("fs 半边失败应 Err");
        assert!(
            error.contains("补偿"),
            "Err 呈现补偿回滚事实，实际: {error}"
        );
        assert!(
            store
                .find_change_record("fix-bug")
                .expect("db 读应成功")
                .is_none(),
            "补偿删除本次自插行"
        );
    }

    // 重开 db：零本次残留（持久层证据）
    let reopened = Store::open_workspace(&db_path).expect("重开 db 失败");
    assert!(
        reopened
            .find_change_record("fix-bug")
            .expect("重开 db 读应成功")
            .is_none(),
        "重开 db 零本次残留"
    );
}

/// create 成功立即可见可发起（AC-6 组合行）：list / detail 立即可见（db 形态
/// 状态面在场）、phase_next 路由首相位。
#[test]
fn 真件create成功立即可见可发起() {
    let corpus = Corpus::new("create-visible");
    let layout = resolve(&corpus.root);

    corpus
        .create("visible-change", "组合用例 goal")
        .expect("create 应 Ok");

    let list = list_changes(&layout, &corpus.store);
    assert_eq!(list.active.len(), 1, "active 恰一条");
    assert_eq!(list.active[0].name, "visible-change");
    assert_eq!(list.active[0].status, Some(ChangeStatus::Active));

    let detail = change_detail(&layout, &corpus.store, "visible-change").expect("详情应可达");
    assert_eq!(detail.status, Some(ChangeStatus::Active));

    let route = phase_next(
        &corpus.store,
        "visible-change",
        "run-visible",
        &SessionAnchors::new(),
    )
    .expect("建档后路由应可达");
    assert_eq!(
        route.next_phase.as_deref(),
        Some("proposal"),
        "建档立即可发起（首相位路由）"
    );
}

/// archive 双写：目录改名入 archive 树（日期前缀）+ db status 翻转；主键
/// name 不变；随后 list 按月分组可达（AC-7 + 分组可达组合行）。
#[test]
fn 真件archive双写_目录改名与db翻转且按月分组可达() {
    let corpus = Corpus::new("archive-dual");
    let layout = resolve(&corpus.root);
    corpus.seed_record("seed-change");
    fs::create_dir_all(layout.changes_root.join("seed-change")).expect("预置目录失败");
    // archive 树在场（fs::rename 不建目标父目录，真实 workspace 归档树常在）
    fs::create_dir_all(&layout.archive_root).expect("预置 archive 树失败");
    let before = utc_date_today();

    let outcome: ArchiveOutcome =
        archive(&layout, &corpus.store, "seed-change").expect("归档应成功");

    let after = utc_date_today();
    assert_eq!(outcome.name, "seed-change", "主键 name 不随目录改名变");
    assert!(
        outcome.archived_date == before || outcome.archived_date == after,
        "archived_date 为 UTC 当日，实际: {}",
        outcome.archived_date
    );
    assert!(
        !layout.changes_root.join("seed-change").exists(),
        "active 树源目录已被改名挪走"
    );
    let target = layout
        .archive_root
        .join(format!("{}-seed-change", outcome.archived_date));
    assert!(target.is_dir(), "archive 树带日期前缀目录在场");

    let record = corpus
        .store
        .find_change_record("seed-change")
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.status, ChangeStatus::Archived);
    assert!(record.archived_at.is_some());

    // 归档条目按月分组可达（目录日期前缀后缀定位——AC-7 分组半边）
    let list = list_changes(&layout, &corpus.store);
    let archived_name = format!("{}-seed-change", outcome.archived_date);
    let group = list
        .archive_groups
        .iter()
        .find(|group| {
            group
                .changes
                .iter()
                .any(|entry| entry.name == archived_name)
        })
        .unwrap_or_else(|| panic!("归档条目应按月分组可达"));
    assert_eq!(
        group.month.as_deref(),
        Some(&outcome.archived_date[..7]),
        "db archived_at 分组与目录前缀一致"
    );
    assert_eq!(
        group.changes[0].status,
        Some(ChangeStatus::Archived),
        "状态面随 db"
    );
}

/// archive 续半边重试（D6）：目录已在 archive 树 + db 仍 active → 重试仅补
/// db 翻转，archive 树源目录不被二次挪动。
#[test]
fn 真件archive续半边重试_仅补db翻转() {
    let corpus = Corpus::new("archive-resume");
    let layout = resolve(&corpus.root);
    corpus.seed_record("seed-change");
    let archived_name = "2026-10-06-seed-change";
    fs::create_dir_all(layout.archive_root.join(archived_name)).expect("预置 archive 目录失败");

    let outcome = archive(&layout, &corpus.store, "seed-change").expect("续半边归档应成功");

    assert_eq!(outcome.archived_date, "2026-10-06", "取前缀日期");
    assert!(
        layout.archive_root.join(archived_name).is_dir(),
        "archive 树源目录原位不动"
    );
    let record = corpus
        .store
        .find_change_record("seed-change")
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.status, ChangeStatus::Archived, "db 补翻转");
}

/// archive 无建档拒绝：active 目录在场 db 零记录 → 显式 `Err` 且零 fs 零 db
/// 变更（存量 CLI 目录不可经桌面归档的显式面）。
#[test]
fn 真件archive无建档拒绝_零fs零db变更() {
    let corpus = Corpus::new("archive-no-record");
    let layout = resolve(&corpus.root);
    fs::create_dir_all(layout.changes_root.join("seed-change")).expect("预置目录失败");

    let error = archive(&layout, &corpus.store, "seed-change").expect_err("无建档应 Err");

    assert!(
        error.contains("未建档") && error.contains("seed-change"),
        "Err 显式记因建档缺失，实际: {error}"
    );
    assert!(
        layout.changes_root.join("seed-change").is_dir(),
        "零 fs 变更"
    );
    assert!(
        corpus
            .store
            .list_change_records()
            .expect("db 读应成功")
            .is_empty(),
        "db 零变更"
    );
}

/// archive 目标冲突：archive 树已存在 `YYYY-MM-DD-<name>` → 先查拒绝，db 零
/// 变更。
#[test]
fn 真件archive目标冲突_db零变更() {
    let corpus = Corpus::new("archive-target-conflict");
    let layout = resolve(&corpus.root);
    corpus.seed_record("seed-change");
    fs::create_dir_all(layout.changes_root.join("seed-change")).expect("预置目录失败");
    fs::create_dir_all(
        layout
            .archive_root
            .join(format!("{}-seed-change", utc_date_today())),
    )
    .expect("预置冲突目标失败");

    let error = archive(&layout, &corpus.store, "seed-change").expect_err("目标已存在应 Err");

    assert!(error.contains("已存在"), "先查拒绝记因，实际: {error}");
    let record = corpus
        .store
        .find_change_record("seed-change")
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.status, ChangeStatus::Active, "db 零变更");
}

/// phase_next 读源 db（AC-3 读墙随动）：真实 store 种子驱动路由；change 目录
/// 内损坏 workflow.json 字节在场仍路由成功——零 workflow.json 读取。
#[test]
fn 真件phase_next读源db_零workflow_json读取() {
    let corpus = Corpus::new("route-read-db");
    corpus.seed_record("demo-change");
    corpus.run_phase(
        "demo-change",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    // 损坏 workflow.json 惰性字节在场（若被读取即路由失败）
    corpus.file("demo-change", "workflow.json", "{ 残缺字节 not json");

    let outcome = phase_next(
        &corpus.store,
        "demo-change",
        "run-read",
        &SessionAnchors::new(),
    )
    .expect("读源 db 路由应成功");

    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("dev-design"),
        "路由权威 = db 相位行（proposal 已 pass → 推进）"
    );
}

/// phase_next 锚点基线平移（D8）：首见锚点 = PhaseRecord 行数 → round=1；
/// 条目增长后复见 → round = 窗口条目数 + 1；run_id 键隔离。
#[test]
fn 真件phase_next锚点基线平移_首见与复见与run隔离() {
    let corpus = Corpus::new("route-anchor");
    corpus.seed_record("demo-change");
    corpus.run_phase(
        "demo-change",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    corpus.run_phase(
        "demo-change",
        "dev-design",
        Verdict::Fail,
        "历史失败一",
        Vec::new(),
        None,
        None,
        t(2),
    );
    corpus.run_phase(
        "demo-change",
        "dev-design",
        Verdict::Fail,
        "历史失败二",
        Vec::new(),
        None,
        None,
        t(3),
    );

    let anchors = SessionAnchors::new();
    let first = phase_next(&corpus.store, "demo-change", "run-a", &anchors).expect("路由应成功");
    assert_eq!(
        first.round, 1,
        "首见锚点 = PhaseRecord 行数 3，round 自 1 起"
    );
    assert_eq!(first.next_phase.as_deref(), Some("dev-design"));
    assert!(first.error.is_none(), "窗口外历史 fail 不虚触上限");

    // 锚点复见：基线后新落 1 行 → round = 1（窗口）+ 1 = 2
    corpus.run_phase(
        "demo-change",
        "dev-design",
        Verdict::Fail,
        "run 内新增",
        Vec::new(),
        None,
        None,
        t(4),
    );
    let second = phase_next(&corpus.store, "demo-change", "run-a", &anchors).expect("路由应成功");
    assert_eq!(second.round, 2, "round = 窗口条目数 + 1（非全量行数 + 1）");

    // run_id 键隔离：新键重新锚定当前行数 4 → round 归位 1
    let third = phase_next(&corpus.store, "demo-change", "run-b", &anchors).expect("路由应成功");
    assert_eq!(third.round, 1);
}

/// phase_next 重启续走（AC-5）：新 SessionAnchors 实例（模拟重启）铸新锚点
/// ——已 pass 相位行在场 → 路由直接推进不重头执行。
#[test]
fn 真件phase_next重启续走_直接推进不重头() {
    let corpus = Corpus::new("route-restart");
    corpus.seed_record("demo-change");
    corpus.run_phase(
        "demo-change",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    corpus.run_phase(
        "demo-change",
        "dev-design",
        Verdict::Pass,
        "设计通过",
        Vec::new(),
        None,
        None,
        t(2),
    );

    let outcome = phase_next(
        &corpus.store,
        "demo-change",
        "run-after-restart",
        &SessionAnchors::new(),
    )
    .expect("重启后续走路由应成功");

    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("test-design"),
        "已 pass 相位行在场 → 直接推进（不重跑）"
    );
    assert_eq!(outcome.round, 1, "新锚点基线 = 全量行数，round 自 1 起");
}

/// list D11 归组语义修订（真件）：db active 而目录已改名入 archive 树 →
/// 仍随 db 留 active 组（db status 权威归组，磁盘目录仅决定目录名取位），
/// 查询路径不回写 db（纯读纪律）。
#[test]
fn 真件list_d11归组_db_status权威且不回写db() {
    let corpus = Corpus::new("list-d11");
    corpus.seed_record("d11-change");
    corpus.dir_with_files(
        "openspec/changes/archive/2026-10-01-d11-change",
        &[("proposal.md", "# 归档")],
    );

    let list = list_changes(&resolve(&corpus.root), &corpus.store);

    let entry = list
        .active
        .iter()
        .find(|entry| entry.name == "d11-change")
        .expect("db active 条目应留 active 组（db status 权威）");
    assert_eq!(entry.status, Some(ChangeStatus::Active), "状态面 = db 记录");
    assert_eq!(entry.source, workflow::queries::ChangeSource::Active);
    assert!(
        !list
            .archive_groups
            .iter()
            .flat_map(|group| group.changes.iter())
            .any(|entry| entry.name == "d11-change"),
        "db active 条目不因磁盘目录误归 archive 组"
    );

    // 纯读纪律：查询路径不回写 db
    let record = corpus
        .store
        .find_change_record("d11-change")
        .expect("db 读应成功")
        .expect("db 记录应在场");
    assert_eq!(record.status, ChangeStatus::Active, "读后 db 仍 active");
}

/// list 并集与去重（真件）：db 条目 + 磁盘-only 条目并集，同名以 db 形态为
/// 准（status / active_phase 状态面在场）。
#[test]
fn 真件list并集与去重_db形态为准() {
    let corpus = Corpus::new("list-union");
    corpus.seed_record("db-change");
    corpus
        .store
        .start_change_phase("db-change", "dev-design", t(6))
        .expect("开相失败");
    corpus.dir_with_files("openspec/changes/db-change", &[("proposal.md", "# 提案")]);
    corpus.dir_with_files("openspec/changes/disk-only", &[("proposal.md", "# 提案")]);
    corpus.seed_record("dual-change");
    corpus.dir_with_files("openspec/changes/dual-change", &[("proposal.md", "# 提案")]);

    let list = list_changes(&resolve(&corpus.root), &corpus.store);

    let names: Vec<&str> = list
        .active
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["db-change", "disk-only", "dual-change"],
        "并集按名排序，同名只出现一次"
    );
    let dual = list
        .active
        .iter()
        .find(|entry| entry.name == "dual-change")
        .expect("dual 条目应在场");
    assert_eq!(
        dual.status,
        Some(ChangeStatus::Active),
        "同名共存以 db 为准"
    );
    let disk = list
        .active
        .iter()
        .find(|entry| entry.name == "disk-only")
        .expect("disk-only 条目应在场");
    assert_eq!(disk.status, None, "文档形态无状态面");
    assert_eq!(disk.created, None);
}

/// detail 时间出线（真件）：created_at / start_at / timestamp 的 i64 millis →
/// RFC3339 ISO 串；epoch 0 → `1970-01-01T00:00:00Z` 口径（冻结契约半边，
/// golden 守卫绿的前提断言）。
#[test]
fn 真件detail时间出线_epoch零口径() {
    let corpus = Corpus::new("detail-epoch");
    corpus
        .store
        .create_change_record(ChangeStateRecord {
            name: "zero-ts".to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: 0,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("建档失败");
    corpus.run_phase(
        "zero-ts",
        "proposal",
        Verdict::Pass,
        "通过",
        Vec::new(),
        None,
        None,
        0,
    );
    corpus.dir_with_files("openspec/changes/zero-ts", &[("proposal.md", "# 提案")]);

    let detail =
        change_detail(&resolve(&corpus.root), &corpus.store, "zero-ts").expect("详情应可达");

    assert_eq!(detail.created.as_deref(), Some("1970-01-01"));
    let record = &detail.pipeline[0].attempts[0];
    assert_eq!(record.timestamp.as_deref(), Some("1970-01-01T00:00:00Z"));
    assert_eq!(record.start_at.as_deref(), Some("1970-01-01T00:00:00Z"));
}

/// detail 未找到与恒可达：record 与定位双缺 → `None`（文档形态未知名）；
/// 建档记录定位全 miss（worktree 缺席、主仓两树未命中）→ 恒可达（D12：
/// dir 缺席、产物清单空、状态面在——source 自 record.status 映射）。
#[test]
fn 真件detail未找到与建档恒可达() {
    let corpus = Corpus::new("detail-not-found");
    let layout = resolve(&corpus.root);
    corpus.dir_with_files("openspec/changes/real", &[("proposal.md", "# 提案")]);

    assert!(change_detail(&layout, &corpus.store, "不存在的change").is_none());
    assert!(change_detail(&layout, &corpus.store, "").is_none());
    assert!(change_detail(&layout, &corpus.store, "a/b").is_none());

    corpus.seed_record("ghost-with-record");
    let detail = change_detail(&layout, &corpus.store, "ghost-with-record")
        .expect("建档记录恒可达详情（定位 miss 不虚构 None）");
    assert_eq!(detail.status, Some(ChangeStatus::Active), "状态面在");
    assert_eq!(
        detail.source,
        workflow::queries::ChangeSource::Active,
        "source 自 record.status 映射"
    );
    assert!(detail.artifacts.is_empty(), "定位 miss 产物清单空");
    assert!(!detail.pipeline.is_empty(), "建档流水线 9 站全量输出");
}

/// run 写面真件回环（unify-run-state-persistence，test-design「write/run.rs ->
/// run_test.rs」节真件半边交叉承载）：`workflow::write::run_start` /
/// `run_finish` 经真实 workspace 库落库——running 行回读（status=running、
/// finished_at=None、started_at 原值，AC-2 第一写）与三终态收口回读（终态 +
/// reason + finished_at + 步整包，AC-1/AC-2 第二写）。
#[test]
fn 真件run写面回环_running行与三终态整包() {
    let corpus = Corpus::new("run-write");
    corpus.seed_record("run-write");
    let store = &corpus.store;
    let start = RunStartCommand {
        run_id: "run-1727000030000".to_owned(),
        change: "run-write".to_owned(),
        started_at: t(30),
    };
    workflow::write::run_start(store, &start).expect("run_start 应成功");

    let runs = store.list_change_runs("run-write").expect("list 应成功");
    assert_eq!(runs.len(), 1, "恰一条 running 行");
    assert_eq!(runs[0].run_id, "run-1727000030000");
    assert_eq!(runs[0].status, RunStatus::Running, "running 行起步");
    assert_eq!(runs[0].started_at, t(30), "started_at 原值（AC-2 第一写）");
    assert_eq!(runs[0].finished_at, None);
    assert_eq!(runs[0].reason, None);

    // 三终态各自收口回读（终态 + reason + finished_at + 步整包同事务落库）
    for (idx, status) in [
        (0, RunStatus::Completed),
        (1, RunStatus::Stopped),
        (2, RunStatus::Failed),
    ] {
        let run_id = format!("run-1727000031{idx}00");
        workflow::write::run_start(
            store,
            &RunStartCommand {
                run_id: run_id.clone(),
                change: "run-write".to_owned(),
                started_at: t(31),
            },
        )
        .expect("run_start 应成功");
        let request = RunRequest {
            root: corpus.root.to_string_lossy().into_owned(),
            change: "run-write".to_owned(),
            run_id: run_id.clone(),
            auto_next_phase: true,
            started_at: t(31),
        };
        let orchestration_status = match status {
            RunStatus::Completed => ChangeRunStatus::Completed,
            RunStatus::Stopped => ChangeRunStatus::Stopped,
            _ => ChangeRunStatus::Failed,
        };
        let command = finish_command(
            &request,
            orchestration_status,
            Some("真件收口记因".to_owned()),
            t(39),
            &[ChangeStepState {
                phase: "proposal".to_owned(),
                attempt: 1,
                step: ChangeStepKind::Executor,
                status: ChangeStepStatus::Passed,
                session_id: Some("ses-run-write".to_owned()),
                detail: None,
            }],
        );
        workflow::write::run_finish(store, &command)
            .unwrap_or_else(|e| panic!("{status:?} 收口应成功: {e}"));

        let runs = store.list_change_runs("run-write").expect("list 应成功");
        let row = runs
            .iter()
            .find(|row| row.run_id == run_id)
            .unwrap_or_else(|| panic!("run {run_id} 行应在场"));
        assert_eq!(row.status, status, "终态逐值落库");
        assert_eq!(row.reason.as_deref(), Some("真件收口记因"));
        assert_eq!(row.finished_at, Some(t(39)));
        let steps = store.list_run_steps(&run_id).expect("步读应成功");
        assert_eq!(steps.len(), 1, "步整包随收口落库");
        assert_eq!(steps[0].step, workflow::state::RunStepKind::Executor);
        assert_eq!(steps[0].status, workflow::state::RunStepStatus::Passed);
        assert_eq!(steps[0].session_id.as_deref(), Some("ses-run-write"));
        assert_eq!(steps[0].timestamp, t(39), "整包同刻 = finished_at");
    }
}
