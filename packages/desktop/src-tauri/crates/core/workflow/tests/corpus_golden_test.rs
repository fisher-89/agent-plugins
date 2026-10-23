//! 集成测试：db 种子语料矩阵 → 详情/列表读链投影 → golden 快照（AC-9 / AC-3）。
//!
//! 语料 = db 种子构造器（经真实 store change 域操作面：建档 / 开相 / 落账 /
//! 回跳 / 归档翻转——workflow dev-dep store）+ 运行时合成磁盘产物树 +
//! workflow.json 惰性字节样本（desktop 对 workflow.json 双向墙，字节零读取
//! 零进投影）。种子 id 归键（D2）：一切建档 / 开相 / 落账 / run 史经
//! `ChangeStateRecord.id` 与命令载荷 `change_id` 携固定 id 字面量，逐字入
//! golden；name 恒裸名属性。防线机制：每份语料跑「detail / list 全读链」，
//! 序列化为 pretty JSON 与入仓 golden 全量对比；db 中性状态类型不出线的线面
//! 契约（id 键入、缺省 null、时间戳 RFC3339 ISO）在本目录逐字节钉死。
//!
//! ## 文档形态零发现反例（D13）
//!
//! 既有「db 缺记录文档形态」样本退役改造为**零发现反例**：磁盘目录树与惰性
//! 字节样本保留（字节零变化），db 零记录 → list 零呈现该语料、change_detail
//!（任意裸名 / 前缀名寻址）恒 None（db 单源零磁盘回退）——不再产出 golden
//!（`corpus-document-form.json` 出局删除）。
//!
//! ## 归档前缀样本（D13）
//!
//! 归档 change（db archived 裸名 + `archived_at` + 磁盘 `YYYY-MM-DD-` 前缀
//! 目录）入 golden：detail 全状态面（status=archived、9 站 pipeline 与 runs
//! 全量）与列表归档组（裸名 + id + `archived_at` 月分组）双面。
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
    finish_command, ChangeRunStatus, ChangeStepEdge, ChangeStepKind, ChangeStepState,
    ChangeStepStatus, EdgeKind, NodeKey, RunRequest,
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
/// 全读链投影的混合形态语料；归档前缀样本 = 归档全状态面 detail）。
/// 「文档形态」语料以零发现反例承载（无 golden——出局删除）。
const CORPORA: &[&str] = &[
    "corpus-initialize",
    "corpus-multi-attempt",
    "corpus-backtrack-stale",
    "corpus-slots-null",
    "corpus-list-mixed",
    "corpus-worktree",
    "corpus-run-history",
    "corpus-run-interrupted",
    "corpus-archive-prefix",
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

/// 步级时间定值族基准槽位（`t(n)` 种子族内定值——`step_at` 默认形态的
/// `started_at` 基准；步级时间零 wall-clock，desktop-step-running-snapshot
/// D11 corpus 定值注入）。
const STEP_T_BASE: u32 = 60;
/// 步级默认派生耗时定值（正常起止双值样本口径：终态 `ended_at` =
/// `started_at` + 本值，即时间族下一槽位；瞬步 / 在途样本经 `step_metrics`
/// overrides 覆写）。
const STEP_DUR_MS: i64 = 1_000;

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
            vcs: FakeVcs::new(worktree_root.clone()),
            worktree_root,
            _db: db,
            store,
        }
    }

    /// create 建域（假件 vcs 提供 worktree 落位父锚）。
    fn create(&self, name: &str, goal: &str) -> Result<CreateOutcome, String> {
        create(
            &self.root,
            &self.store,
            &self.vcs,
            name,
            goal,
            name,
            "requirement",
        )
    }

    /// 建档（requirement、active、created_at 固定；id 为固定字面量——D2
    /// 种子直携，name 为裸名属性）。
    fn seed_record(&self, id: &str, name: &str) {
        self.store
            .create_change_record(ChangeStateRecord {
                id: id.to_owned(),
                name: name.to_owned(),
                title: name.to_owned(),
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

    /// 建档（显式 title 形态——title ≠ name 的 golden 样本面）。
    fn seed_record_titled(&self, id: &str, name: &str, title: &str) {
        self.store
            .create_change_record(ChangeStateRecord {
                id: id.to_owned(),
                name: name.to_owned(),
                title: title.to_owned(),
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

    /// 开相 + 落账一步（attempt 事务内推导；时间戳全固定；id 寻址）。
    #[allow(clippy::too_many_arguments)]
    fn run_phase(
        &self,
        change_id: &str,
        phase: &str,
        verdict: Verdict,
        report: &str,
        checklist: Vec<ChecklistItem>,
        executor: Option<&str>,
        evaluator: Option<&str>,
        ts: i64,
    ) {
        self.store
            .start_change_phase(change_id, phase, ts)
            .expect("开相失败");
        self.store
            .log_change_phase(&PhaseLogCommand {
                change_id: change_id.to_owned(),
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
    fn open_only(&self, change_id: &str, phase: &str, ts: i64) {
        self.store
            .start_change_phase(change_id, phase, ts)
            .expect("开相失败");
    }

    /// run 发起落行（经 store run 域操作面——禁裸表插桩；started_at 固定，
    /// golden 确定性；id 归属键）。
    fn run_start(&self, change_id: &str, run_id: &str, started_at: i64) {
        self.store
            .start_change_run(&RunStartCommand {
                run_id: run_id.to_owned(),
                change_id: change_id.to_owned(),
                started_at,
            })
            .expect("run 发起落行失败");
    }

    /// run 收口整包落库（经 store run 域操作面）：步序列经 orchestration
    /// `finish_command` 组装——十词汇全序列 + 四 kind 边整包喂入（零过滤单点
    /// 同真实写路径），emit 序 = 语料注入序；id 归属键。
    #[allow(clippy::too_many_arguments)]
    fn run_finish(
        &self,
        change_id: &str,
        run_id: &str,
        status: RunStatus,
        reason: &str,
        finished_at: i64,
        nodes: &[ChangeStepState],
        edges: &[ChangeStepEdge],
    ) {
        let request = RunRequest {
            root: self.root.to_string_lossy().into_owned(),
            main_root: self.root.to_string_lossy().into_owned(),
            stop_before: Some("archive".to_owned()),
            change_id: change_id.to_owned(),
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
            nodes,
            edges,
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

    /// 在 change 目录（active 树，目录名 = 磁盘面 change 名）写文件（自动建
    /// 父目录）。
    fn file(&self, dir_name: &str, rel: &str, content: &str) {
        let dir = resolve(&self.root).changes_root.join(dir_name);
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

    /// detail 全读链投影（id 寻址）。
    fn detail(&self, id: &str) -> ChangeDetail {
        change_detail(&resolve(&self.root), &self.store, id)
            .expect("语料 detail 读面应 Ok")
            .unwrap_or_else(|| panic!("语料 {id} 应可定位"))
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

struct FakeVcs {
    /// worktree 落位父锚（create 经 port 派生）。
    worktree_root: PathBuf,
}

impl FakeVcs {
    fn new(worktree_root: PathBuf) -> Self {
        Self { worktree_root }
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
    fn worktree_root(&self, _main_root: &Path) -> Result<PathBuf, String> {
        Ok(self.worktree_root.clone())
    }

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
    // title 两态样本之一：显式标题（title ≠ name —— detail.title 出线面）
    corpus.seed_record_titled("id-multi-attempt", "multi-attempt", "多 attempt 语料标题");
    corpus.run_phase(
        "id-multi-attempt",
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
        "id-multi-attempt",
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
        "id-multi-attempt",
        "dev-design",
        Verdict::Pass,
        "第二轮通过",
        vec![item("组件表完整", true, "五组件齐全")],
        Some("ses-dd-exec-a2"),
        Some("ses-dd-eval-a2"),
        t(3),
    );
    // 重开 attempt 残留：开相未落账（active_phase 在场）
    corpus.open_only("id-multi-attempt", "dev-design", t(4));
    // 磁盘产物树（目录名 = 磁盘面 change 名）
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
    corpus.seed_record("id-backtrack-stale", "backtrack-stale");
    corpus.run_phase(
        "id-backtrack-stale",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    corpus.run_phase(
        "id-backtrack-stale",
        "dev-design",
        Verdict::Pass,
        "设计通过",
        vec![item("组件表完整", true, "五组件齐全")],
        None,
        None,
        t(2),
    );
    corpus.run_phase(
        "id-backtrack-stale",
        "test-design",
        Verdict::Pass,
        "测试设计通过",
        Vec::new(),
        None,
        None,
        t(3),
    );
    corpus.run_phase(
        "id-backtrack-stale",
        "implement",
        Verdict::Pass,
        "实现完成",
        Vec::new(),
        None,
        None,
        t(4),
    );
    corpus.run_phase(
        "id-backtrack-stale",
        "test-gen",
        Verdict::Pass,
        "测试生成通过",
        Vec::new(),
        None,
        None,
        t(5),
    );
    corpus.backtrack(&BacktrackCommand {
        change_id: "id-backtrack-stale".to_owned(),
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
    corpus.seed_record("id-slots-null", "slots-null");
    corpus.run_phase(
        "id-slots-null",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    corpus.run_phase(
        "id-slots-null",
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

/// 文档形态零发现反例语料（D13）：磁盘产物树（proposal / design / tasks /
/// reports）+ workflow.json 惰性字节样本、db 零记录 → list 零呈现、
/// change_detail（任意裸名 / 前缀名寻址）恒 None，磁盘字节零变化；
/// **不产出 golden**（出局删除）。
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
        "{ \"LAZY_BYTES_MARKER\": \"desktop 零读取此字节（db 单源，磁盘-only 零发现）\" }",
    );
    corpus
}

/// 列表混合形态语料（db 单源）：db active（运行态 + 磁盘目录在场）+
/// db archived（两月 + archived_at 缺失行）+ 磁盘-only active + 磁盘 archive
/// （带前缀 / 无前缀）→ `list_changes` 全读链投影——磁盘-only 条目自投影出局
/// （零呈现反例），归档条目裸名 + id + `archived_at` 月分组。
fn build_list_mixed() -> Corpus {
    let corpus = Corpus::new("list-mixed");
    // db active（带 active_phase 运行态 + 磁盘目录在场 → active 归组）
    corpus.seed_record("id-list-active", "list-active");
    corpus
        .store
        .start_change_phase("id-list-active", "implement", t(6))
        .expect("开相失败");
    corpus.dir_with_files("openspec/changes/list-active", &[("proposal.md", "# 提案")]);
    // db archived（archived_at 锚定 2026-05-20；磁盘目录带日期前缀——列表
    // 出线恒裸名，前缀仅存在于磁盘目录名）
    corpus.seed_record("id-list-archived", "list-archived");
    corpus
        .store
        .set_change_archived("id-list-archived", utc_millis(2026, time::Month::May, 20))
        .expect("归档翻转失败");
    corpus.dir_with_files(
        "openspec/changes/archive/2026-05-20-list-archived",
        &[("proposal.md", "# 归档")],
    );
    // 归档前缀样本（D13）：db archived 裸名 + `archived_at` 锚定 2026-02-02 +
    // 磁盘 `YYYY-MM-DD-` 前缀目录 → 归档组（裸名 + id + 月分组）
    corpus.seed_record("id-seeded-archived", "seeded-archived");
    corpus
        .store
        .set_change_archived(
            "id-seeded-archived",
            utc_millis(2026, time::Month::February, 2),
        )
        .expect("归档翻转失败");
    corpus.dir_with_files(
        "openspec/changes/archive/2026-02-02-seeded-archived",
        &[("proposal.md", "# 归档")],
    );
    // db archived 而 archived_at 缺失（直接构造）→ 未知时间组置尾
    corpus
        .store
        .create_change_record(ChangeStateRecord {
            id: "id-unknown-time-archived".to_owned(),
            name: "unknown-time-archived".to_owned(),
            title: "unknown-time-archived".to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: T0,
            status: ChangeStatus::Archived,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("未知时间归档建档失败");
    // 磁盘-only active（文档形态——零呈现反例）
    corpus.dir_with_files("openspec/changes/disk-only", &[("proposal.md", "# 存量")]);
    // 磁盘 archive（日期前缀，db 缺记录——零呈现反例）
    corpus.dir_with_files(
        "openspec/changes/archive/2026-09-15-disk-archived",
        &[("proposal.md", "# 归档")],
    );
    // 磁盘 archive（无日期前缀，db 缺记录——零呈现反例）
    corpus.dir_with_files(
        "openspec/changes/archive/unknown-date-archived",
        &[("proposal.md", "# 无前缀归档")],
    );
    // worktree 条目（D11 / AC-9）：db active + worktree 执行锚 + 主仓两树未
    // 命中 → 进行中组（目录名 = 建档名——不丢弃不误归未知时间组）
    let wt_entry = ChangeStateRecord {
        id: "id-list-worktree".to_owned(),
        name: "list-worktree".to_owned(),
        // title 两态样本之一：显式标题（title ≠ name —— list golden 出线面）
        title: "工作树条目标题".to_owned(),
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
            id: "id-corpus-worktree".to_owned(),
            name: "corpus-worktree".to_owned(),
            title: "corpus-worktree".to_owned(),
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
        "id-corpus-worktree",
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
/// 推进（attempt 跨 run 递增 = 2、executor 单步无会话缺省落账）+ run-3 步级
/// 时间与 token 两态样本（desktop-step-running-snapshot D11）。流程面步骤
/// 与三门经全词汇序列喂 `finish_command`（语料纪律：缺席断言留测试相位）。
/// `tag` 分域同理（见 [`build_archive_prefix`] 注）。
fn build_run_history(tag: &str) -> Corpus {
    let corpus = Corpus::new(tag);
    corpus.seed_record("id-run-history", "run-history");
    // 前置：proposal 已 pass（run-2 续走锚点推进的事实源）
    corpus.run_phase(
        "id-run-history",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    // run-1：十词汇全整包（三门 / 相位机步同序列喂入，零过滤）+ 四 kind 边
    //（含 feedback 回边与 executor 反馈重跑新实例）
    corpus.run_start("id-run-history", "run-1727000010000", t(10));
    corpus.run_finish(
        "id-run-history",
        "run-1727000010000",
        RunStatus::Completed,
        "首轮 dev-design 未过（预算内 fail）",
        t(19),
        &[
            step(
                "implement",
                1,
                ChangeStepKind::PhaseStart,
                ChangeStepStatus::Passed,
                None,
                None,
            ),
            step(
                "implement",
                1,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-impl-1"),
                Some("实现完成"),
            ),
            step_at(
                "implement",
                1,
                0,
                ChangeStepKind::StaticCheck,
                ChangeStepStatus::Failed,
                None,
                None,
                Some("exec-1727000019000"),
            ),
            // feedback 重跑：同 (phase, attempt, step) 新实例（instance_no 递增）
            step_at(
                "implement",
                1,
                1,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-impl-1b"),
                Some("按 feedback 修复重跑"),
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
                ChangeStepKind::VerdictGate,
                ChangeStepStatus::Failed,
                None,
                Some("verdict=fail"),
            ),
            step(
                "implement",
                1,
                ChangeStepKind::RetryGate,
                ChangeStepStatus::Passed,
                None,
                Some("预算内同相位重跑"),
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
                "implement",
                1,
                ChangeStepKind::WhitelistGate,
                ChangeStepStatus::Passed,
                None,
                Some("回溯白名单内"),
            ),
            step(
                "dev-design",
                1,
                ChangeStepKind::TestExecution,
                ChangeStepStatus::Passed,
                None,
                Some("conclusion=pass total=3"),
            ),
            // mutation_test 拆步新词样本（AC-1 / AC-9 十九值投影语料在案）
            step(
                "dev-design",
                1,
                ChangeStepKind::MutationTest,
                ChangeStepStatus::Passed,
                None,
                Some("conclusion=pass score=95 threshold=80"),
            ),
            step(
                "dev-design",
                1,
                ChangeStepKind::PhaseLog,
                ChangeStepStatus::Passed,
                None,
                None,
            ),
        ],
        &[
            edge(
                key("implement", 1, ChangeStepKind::PhaseStart, 0),
                key("implement", 1, ChangeStepKind::Executor, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("implement", 1, ChangeStepKind::Executor, 0),
                key("implement", 1, ChangeStepKind::StaticCheck, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("implement", 1, ChangeStepKind::StaticCheck, 0),
                key("implement", 1, ChangeStepKind::Executor, 1),
                EdgeKind::Feedback,
            ),
            edge(
                key("implement", 1, ChangeStepKind::Executor, 1),
                key("implement", 1, ChangeStepKind::Evaluator, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("implement", 1, ChangeStepKind::Evaluator, 0),
                key("implement", 1, ChangeStepKind::Decision, 0),
                EdgeKind::Retry,
            ),
            edge(
                key("implement", 1, ChangeStepKind::WhitelistGate, 0),
                key("dev-design", 1, ChangeStepKind::TestExecution, 0),
                EdgeKind::Backtrack,
            ),
        ],
    );
    // run-2：dev-design attempt 2（phase_start max+1 跨 run 递增）续走推进
    corpus.run_phase(
        "id-run-history",
        "dev-design",
        Verdict::Pass,
        "第二轮通过",
        vec![item("组件表完整", true, "五组件齐全")],
        Some("ses-dd-exec-a2"),
        Some("ses-dd-eval-a2"),
        t(21),
    );
    corpus.run_start("id-run-history", "run-1727000020000", t(20));
    corpus.run_finish(
        "id-run-history",
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
        &[],
    );
    // run-3：步级时间与 token 两态样本（desktop-step-running-snapshot D11，
    // stopped 收口样本，attempt 跨 run 递增 = 3）——瞬步 started == ended
    //（同槽定值，时间两态之二）/ token 有值 WorkerAgent 步（时间正常起止
    // 双值定值，token 两态之一）/ running 收口在途残留步（ended = None，
    // 时间两态之三）；token None 两态之二 = 工具步（run-1 / run-2 既有步全
    // 量默认形态）。定值全落 `t(n)` 种子时间族（零 wall-clock），经
    // `finish_command` → store run 域操作面落库（禁裸表插桩）
    corpus.run_start("id-run-history", "run-1727000030000", t(30));
    corpus.run_finish(
        "id-run-history",
        "run-1727000030000",
        RunStatus::Stopped,
        "用户中止：dev-design attempt 3 evaluator 在途",
        t(39),
        &[
            // 瞬步样本：gate 步零耗时（started == ended = t(60) 同槽定值；
            // token None）
            step_metrics(
                "dev-design",
                3,
                0,
                ChangeStepKind::VerdictGate,
                ChangeStepStatus::Passed,
                None,
                Some("verdict=fail 预算内同相位重跑"),
                None,
                t(60),
                Some(t(60)),
                None,
                None,
            ),
            // token 有值样本：WorkerAgent 步终态带本步 turn 增量两值（起止
            // 双值定值 t(61) → t(62)）
            step_metrics(
                "dev-design",
                3,
                0,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-dd-exec-a3"),
                Some("实现完成（token 增量样本）"),
                None,
                t(61),
                Some(t(62)),
                Some(12_345),
                Some(6_789),
            ),
            // 收口在途样本：stopped 收口时终态 emit 未达的 running 残留步
            //（ended = None，started 定值 t(63)）
            step_metrics(
                "dev-design",
                3,
                0,
                ChangeStepKind::Evaluator,
                ChangeStepStatus::Running,
                Some("ses-dd-eval-a3"),
                None,
                None,
                t(63),
                None,
                None,
                None,
            ),
        ],
        &[],
    );
    corpus
}

/// interrupted 标定语料（unify-run-state-persistence）：run 发起后中途死亡
/// （残留 running 行 + active_phase 悬挂）→ 启动标定翻 interrupted（记因附
/// 中断语境）+ active_phase 清位（悬挂杀除）。
fn build_run_interrupted() -> Corpus {
    let corpus = Corpus::new("run-interrupted");
    corpus.seed_record("id-run-interrupted", "run-interrupted");
    corpus.run_start("id-run-interrupted", "run-1727000005000", t(5));
    // 中途死亡形态：开相未落账（active_phase 悬挂在位）
    corpus.open_only("id-run-interrupted", "implement", t(6));
    // 重启装配：启动标定（open_workspace 内嵌同路径——直调操作面构造样本）
    corpus.calibrate(t(9));
    corpus
}

/// 归档前缀样本语料（D13）：db archived（固定 id + 裸名 `seeded-archived` +
/// `archived_at` 2026-02-02）+ 磁盘 `YYYY-MM-DD-` 前缀目录（产物树）+ 相位行
/// 与 run 史种子 → detail 全状态面 golden（status=archived、9 站 pipeline 与
/// runs 全量出线；错配链结构性退役——AC-2）。
/// `tag` 分域临时目录（同 crate 测试线程并行跑，`Corpus::new` 以
/// `temp/{pid}-{tag}` 为根且构造即清空——两测试共用 tag 会互删语料文件，
/// golden 投影偶发缺 markdown 产物即此竞争）。
fn build_archive_prefix(tag: &str) -> Corpus {
    let corpus = Corpus::new(tag);
    corpus
        .store
        .create_change_record(ChangeStateRecord {
            id: "id-seeded-archived".to_owned(),
            name: "seeded-archived".to_owned(),
            title: "seeded-archived".to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: T0,
            status: ChangeStatus::Archived,
            archived_at: Some(utc_millis(2026, time::Month::February, 2)),
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("归档前缀样本建档失败");
    corpus.run_phase(
        "id-seeded-archived",
        "proposal",
        Verdict::Pass,
        "提案通过",
        vec![item("问题清晰", true, "L1-10")],
        Some("ses-arch-exec"),
        Some("ses-arch-eval"),
        t(1),
    );
    // archive 相位空 checklist 落账样本（AC-4 语料半边）：report 空串 + 空
    // checklist → verdict 推导 pass，pipeline archive 站投影 attempt 全字段可辨
    corpus.run_phase(
        "id-seeded-archived",
        "archive",
        Verdict::Pass,
        "",
        Vec::new(),
        None,
        None,
        t(3),
    );
    corpus.run_start("id-seeded-archived", "run-1727000040000", t(10));
    corpus.run_finish(
        "id-seeded-archived",
        "run-1727000040000",
        RunStatus::Completed,
        "All phases have passed evaluation. Ready for archiving.",
        t(19),
        &[step(
            "proposal",
            1,
            ChangeStepKind::Executor,
            ChangeStepStatus::Passed,
            Some("ses-arch-exec"),
            None,
        )],
        &[],
    );
    // 归档相位 run 史样本（AC-6 语料半边）：archive 十一步节点（9 归档步词，含
    // cleanup 尽力步样本 + PhaseStart / PhaseLog）整包喂入 + 串行 forward 边
    //（零反馈 / 重试 / 回溯），git 步（commit / rebase / merge / cleanup /
    // finalize）与 agent 步（resolve_conflicts / spec_sync）会话面可辨
    corpus.run_start("id-seeded-archived", "run-1727000041000", t(20));
    corpus.run_finish(
        "id-seeded-archived",
        "run-1727000041000",
        RunStatus::Completed,
        "archive phase landed an empty checklist verdict=pass",
        t(29),
        &[
            step(
                "archive",
                1,
                ChangeStepKind::PhaseStart,
                ChangeStepStatus::Passed,
                None,
                None,
            ),
            step(
                "archive",
                1,
                ChangeStepKind::Preflight,
                ChangeStepStatus::Passed,
                None,
                Some("前置只读校验：8 相位已过 / 无缺产物"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::Commit,
                ChangeStepStatus::Passed,
                None,
                Some("worktree 干净，无待提交变更（跳过）"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::Rebase,
                ChangeStepStatus::Passed,
                None,
                Some("已重放至主仓当前分支，无冲突"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::ResolveConflicts,
                ChangeStepStatus::Passed,
                Some("ses-arch-resolve"),
                Some("无冲突，无需解算（跳过）"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::Merge,
                ChangeStepStatus::Passed,
                None,
                Some("主仓 ff-only 快进完成"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::SpecSync,
                ChangeStepStatus::Passed,
                Some("ses-arch-spec-sync"),
                Some("delta specs 已同步"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::Cleanup,
                ChangeStepStatus::Passed,
                None,
                Some("已移除 worktree（<WORKTREE_ROOT>/seeded-archived）"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::Seal,
                ChangeStepStatus::Passed,
                None,
                Some("双写收口：目录改名 + status 翻转"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::Finalize,
                ChangeStepStatus::Passed,
                None,
                Some("归档产物已落盘，无待提交路径（跳过）"),
            ),
            step(
                "archive",
                1,
                ChangeStepKind::PhaseLog,
                ChangeStepStatus::Passed,
                None,
                Some("空 checklist 落账 verdict=pass"),
            ),
        ],
        &[
            edge(
                key("archive", 1, ChangeStepKind::PhaseStart, 0),
                key("archive", 1, ChangeStepKind::Preflight, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::Preflight, 0),
                key("archive", 1, ChangeStepKind::Commit, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::Commit, 0),
                key("archive", 1, ChangeStepKind::Rebase, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::Rebase, 0),
                key("archive", 1, ChangeStepKind::ResolveConflicts, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::ResolveConflicts, 0),
                key("archive", 1, ChangeStepKind::Merge, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::Merge, 0),
                key("archive", 1, ChangeStepKind::SpecSync, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::SpecSync, 0),
                key("archive", 1, ChangeStepKind::Cleanup, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::Cleanup, 0),
                key("archive", 1, ChangeStepKind::Seal, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::Seal, 0),
                key("archive", 1, ChangeStepKind::Finalize, 0),
                EdgeKind::Forward,
            ),
            edge(
                key("archive", 1, ChangeStepKind::Finalize, 0),
                key("archive", 1, ChangeStepKind::PhaseLog, 0),
                EdgeKind::Forward,
            ),
        ],
    );
    // 磁盘前缀目录（archive 树；目录名 = `YYYY-MM-DD-` + 裸名——后缀扫描
    // 命中；detail 产物清单自该目录发现，name 恒自记录供给）
    let archived_dir = resolve(&corpus.root)
        .archive_root
        .join("2026-02-02-seeded-archived");
    fs::create_dir_all(&archived_dir).expect("创建归档前缀目录失败");
    fs::write(
        archived_dir.join("proposal.md"),
        "# 归档提案\n\n前缀目录样本。",
    )
    .expect("写归档产物失败");
    fs::write(archived_dir.join("tasks.md"), "- [x] 归档\n").expect("写归档产物失败");
    corpus
}

/// 全词汇步状态行 fixture（run 维度种子构造器的 ChangeStepState 组装面；首次
/// 执行实例 = 0、无输出记录）。步级时间与 token 四字段定值注入（D11 corpus
/// 定值注入，零 wall-clock——直构 `ChangeStepState`，不经 `emit_node` /
/// `now_millis`）：`started_at` 取种子时间族基准槽位、`ended_at` 按状态派生
/// 定值（running 收口在途恒 None / 终态 = started + 默认耗时定值）、token
/// None（工具步两态之一）；瞬步 / token 有值样本经 [`step_metrics`] overrides。
#[allow(clippy::too_many_arguments)]
fn step(
    phase: &str,
    attempt: u32,
    kind: ChangeStepKind,
    status: ChangeStepStatus,
    session_id: Option<&str>,
    detail: Option<&str>,
) -> ChangeStepState {
    step_at(phase, attempt, 0, kind, status, session_id, detail, None)
}

/// 携稳定身份键 + 输出引用的步状态行 fixture（feedback 重跑实例号递增 /
/// `ExecOutput::Recorded` 引用面）。四字段定值注入默认形态见 [`step`]。
#[allow(clippy::too_many_arguments)]
fn step_at(
    phase: &str,
    attempt: u32,
    instance_no: u32,
    kind: ChangeStepKind,
    status: ChangeStepStatus,
    session_id: Option<&str>,
    detail: Option<&str>,
    output_ref: Option<&str>,
) -> ChangeStepState {
    let started_at = t(STEP_T_BASE);
    let ended_at = match status {
        ChangeStepStatus::Running => None,
        ChangeStepStatus::Passed | ChangeStepStatus::Failed | ChangeStepStatus::Stopped => {
            Some(started_at + STEP_DUR_MS)
        }
    };
    ChangeStepState {
        phase: phase.to_owned(),
        attempt,
        step: kind,
        instance_no,
        status,
        session_id: session_id.map(str::to_owned),
        detail: detail.map(str::to_owned),
        output_ref: output_ref.map(str::to_owned),
        started_at,
        ended_at,
        token_input: None,
        token_output: None,
    }
}

/// 两态样本定值 overrides（D11）：在 [`step_at`] 默认派生行上覆写时间 /
/// token 四字段定值——瞬步（`started_at == ended_at` 同槽定值）/ token 有值
/// WorkerAgent 步 / 收口在途 `started_at` 定值等样本的注入面。
#[allow(clippy::too_many_arguments)]
fn step_metrics(
    phase: &str,
    attempt: u32,
    instance_no: u32,
    kind: ChangeStepKind,
    status: ChangeStepStatus,
    session_id: Option<&str>,
    detail: Option<&str>,
    output_ref: Option<&str>,
    started_at: i64,
    ended_at: Option<i64>,
    token_input: Option<u64>,
    token_output: Option<u64>,
) -> ChangeStepState {
    let mut state = step_at(
        phase,
        attempt,
        instance_no,
        kind,
        status,
        session_id,
        detail,
        output_ref,
    );
    state.started_at = started_at;
    state.ended_at = ended_at;
    state.token_input = token_input;
    state.token_output = token_output;
    state
}

/// 节点身份键 fixture（边端点口径 `(phase, attempt, step, instance_no)`）。
fn key(phase: &str, attempt: u32, kind: ChangeStepKind, instance_no: u32) -> NodeKey {
    (phase.to_owned(), attempt, kind, instance_no)
}

/// 执行图边 fixture（端点恒为节点身份键元组）。
fn edge(from: NodeKey, to: NodeKey, kind: EdgeKind) -> ChangeStepEdge {
    ChangeStepEdge { from, to, kind }
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

/// detail 语料的投影：change_detail 全读链输出（id 寻址）。
fn project_detail(corpus: &Corpus, id: &str) -> serde_json::Value {
    let detail = corpus.detail(id);
    serde_json::to_value(&detail).expect("ChangeDetail 序列化失败")
}

/// list 语料的投影：list_changes 全读链输出（db 单源）。
fn project_list(corpus: &Corpus) -> serde_json::Value {
    let list = list_changes(&corpus.store);
    serde_json::to_value(&list).expect("ChangeList 序列化失败")
}

/// 目录树全量字节快照（相对路径 → 文件字节；目录名排序——零变化断言用）。
fn snapshot_dir_bytes(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                out.push((rel, fs::read(&path).expect("读盘字节失败")));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ---------------------------------------------------------------------------
// 语料 golden 对拍（AC-9 矩阵）
// ---------------------------------------------------------------------------

/// 多 attempt 语料：同相位两轮落账 + 重开 attempt 种子 → detail golden 重组
/// 逐字段一致（id 键入、attempt 升序、checklist 打包键序、created_at /
/// start_at ISO 出线）。
#[test]
fn corpus多attempt语料_golden对拍() {
    let corpus = build_multi_attempt();
    let detail = project_detail(&corpus, "id-multi-attempt");
    // title 两态样本之一（显式标题）：detail.title 出线且 ≠ name
    assert_eq!(
        detail["title"].as_str(),
        Some("多 attempt 语料标题"),
        "显式标题样本 detail.title 出线（title ≠ name 两态之一）"
    );
    assert_ne!(detail["title"], detail["name"], "title 与 name 双字段可辨");
    check_or_rewrite("corpus-multi-attempt", &detail);
}

/// backtrack stale 语料：回跳 + dependents 闭包翻转种子 → stale 位与
/// backtrack_to / backtrack_reason 字段 golden 一致（AC-9 矩阵）。
#[test]
fn corpusbacktrack_stale语料_golden对拍() {
    let corpus = build_backtrack_stale();
    check_or_rewrite(
        "corpus-backtrack-stale",
        &project_detail(&corpus, "id-backtrack-stale"),
    );
}

/// 槽位全缺语料：三会话槽位全 None + active_phase 缺席种子 → 槽位三列 null
/// / 时间 ISO 出线的 golden 一致（冻结契约「null 留位」半边）。
#[test]
fn corpus槽位全缺语料_golden对拍() {
    let corpus = build_slots_null();
    let detail = project_detail(&corpus, "id-slots-null");
    // title 两态样本之二（缺省 = name）：恒非空且与 name 同值
    assert_eq!(
        detail["title"], detail["name"],
        "缺省样本 title = name（两态之二）"
    );
    check_or_rewrite("corpus-slots-null", &detail);
}

/// 文档形态零发现反例（D13 退役改造）：磁盘产物树 + workflow.json 惰性字节
/// 样本、db 零记录 → list 零呈现该语料、change_detail（裸名 / 前缀名寻址）
/// 恒 None、目录字节零变化、惰性字节零进投影；该 golden 出局删除。
#[test]
fn corpus文档形态零发现反例() {
    let corpus = build_document_form();
    let layout = resolve(&corpus.root);
    // 目录字节零变化：读链前后全量字节快照逐字节相等
    let before = snapshot_dir_bytes(&layout.changes_root);

    // list 零呈现：active / archive 两组皆空（db 单源——磁盘-only 零发现）
    let list = project_list(&corpus);
    assert!(
        list["active"].as_array().is_some_and(Vec::is_empty),
        "磁盘-only active 目录零呈现：{list}"
    );
    assert!(
        list["archiveGroups"].as_array().is_some_and(Vec::is_empty),
        "磁盘-only archive 目录零呈现：{list}"
    );
    // 惰性字节零进投影
    let serialized = serde_json::to_string(&list).expect("序列化应成功");
    assert!(
        !serialized.contains("LAZY_BYTES_MARKER"),
        "workflow.json 惰性字节零进投影"
    );

    // change_detail 任意寻址恒 None（裸名 / 前缀名寻址均不命中 db；读面 Ok 由
    // golden 全好语料保证——expect 收口）
    assert!(
        change_detail(&layout, &corpus.store, "document-form")
            .expect("详情读面应 Ok")
            .is_none(),
        "裸名寻址未建档 id → None（零磁盘回退）"
    );
    assert!(
        change_detail(&layout, &corpus.store, "2026-01-01-document-form")
            .expect("详情读面应 Ok")
            .is_none(),
        "前缀名寻址未建档 id → None（零磁盘回退）"
    );
    assert!(
        change_detail(&layout, &corpus.store, "id-document-form")
            .expect("详情读面应 Ok")
            .is_none(),
        "任意 id 形态寻址恒 None"
    );

    // 目录字节零变化（零读取零写入零报错）
    let after = snapshot_dir_bytes(&layout.changes_root);
    assert_eq!(before, after, "文档形态目录字节零变化");
    // golden 出局：无 corpus-document-form.json 对拍
    assert!(
        !golden_dir().join("corpus-document-form.json").exists(),
        "文档形态 golden 出局删除"
    );
}

/// 列表混合形态语料（db 单源）：db 全量记录投影 + 磁盘-only 零呈现反例 +
/// 裸名 / id / `archived_at` 月分组 → ChangeList golden（列表半边的 golden
/// 守卫；归档前缀样本行在场）。
#[test]
fn corpus列表混合语料_golden对拍() {
    let corpus = build_list_mixed();
    let layout = resolve(&corpus.root);
    // 磁盘-only 目录字节零变化（零呈现不落地任何读写）
    let disk_only_before = snapshot_dir_bytes(&layout.changes_root.join("disk-only"));
    let archived_tree_before = snapshot_dir_bytes(&layout.archive_root);

    let list = project_list(&corpus);

    // active 组 = db active 全量（裸名 + id；磁盘-only `disk-only` 零呈现）
    let active_names: Vec<&str> = list["active"]
        .as_array()
        .expect("active 为数组")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    assert!(
        active_names.contains(&"list-active") && active_names.contains(&"list-worktree"),
        "db active 条目全量在列：{active_names:?}"
    );
    assert!(
        !active_names.contains(&"disk-only"),
        "磁盘-only active 条目零呈现：{active_names:?}"
    );
    for entry in list["active"].as_array().expect("active 为数组") {
        assert!(
            entry["id"].as_str().is_some_and(|id| !id.is_empty()),
            "active 条目 id 恒在案：{entry}"
        );
        // title 线形契约：键恒在场且恒非空（title = name 缺省样本与显式标题样本同式）
        assert!(
            entry["title"]
                .as_str()
                .is_some_and(|title| !title.is_empty()),
            "active 条目 title 键恒在场且非空：{entry}"
        );
    }
    // title 两态样本：list-worktree 显式标题（title ≠ name）
    let worktree_entry = list["active"]
        .as_array()
        .expect("active 为数组")
        .iter()
        .find(|entry| entry["name"] == "list-worktree")
        .expect("worktree 条目在场");
    assert_eq!(
        worktree_entry["title"].as_str(),
        Some("工作树条目标题"),
        "显式标题样本出线 title（title ≠ name 两态之一）"
    );
    let default_entry = list["active"]
        .as_array()
        .expect("active 为数组")
        .iter()
        .find(|entry| entry["name"] == "list-active")
        .expect("缺省样本在场");
    assert_eq!(
        default_entry["title"], default_entry["name"],
        "缺省样本 title = name（两态之二）"
    );
    // 归档组：archived_at 月分组（新月份在前 + 未知时间组置尾）；条目裸名 +
    // id；磁盘前缀不参与（list-archived 磁盘目录带 2026-05-20- 前缀，出线恒
    // 裸名）；磁盘-only archive 目录（前缀 / 无前缀）零呈现
    let groups = list["archiveGroups"].as_array().expect("归档组为数组");
    let months: Vec<Option<&str>> = groups.iter().map(|group| group["month"].as_str()).collect();
    assert_eq!(
        months,
        vec![Some("2026-05"), Some("2026-02"), None],
        "月分组 archived_at 权威（新月份在前、未知时间置尾）"
    );
    for group in groups {
        for entry in group["changes"].as_array().expect("组内条目为数组") {
            let name = entry["name"].as_str().expect("条目裸名");
            assert!(
                !name.starts_with("20"),
                "归档条目 name 恒裸名（零日期前缀）：{name}"
            );
            assert!(
                entry["id"].as_str().is_some_and(|id| !id.is_empty()),
                "归档条目 id 恒在案：{entry}"
            );
            assert!(
                entry["title"]
                    .as_str()
                    .is_some_and(|title| !title.is_empty()),
                "归档条目 title 键恒在场且非空：{entry}"
            );
            assert!(
                !name.ends_with("disk-archived") && name != "unknown-date-archived",
                "磁盘-only 归档目录零呈现：{name}"
            );
        }
    }
    let all_archived: Vec<&str> = groups
        .iter()
        .flat_map(|group| {
            group["changes"]
                .as_array()
                .expect("组内条目为数组")
                .iter()
                .filter_map(|entry| entry["name"].as_str())
        })
        .collect();
    assert!(
        all_archived.contains(&"seeded-archived"),
        "归档前缀样本裸名在列：{all_archived:?}"
    );

    // 磁盘字节零变化（零呈现反例断言面）
    let disk_only_after = snapshot_dir_bytes(&layout.changes_root.join("disk-only"));
    assert_eq!(
        disk_only_before, disk_only_after,
        "磁盘-only active 目录字节零变化"
    );
    let archived_tree_after = snapshot_dir_bytes(&layout.archive_root);
    assert_eq!(
        archived_tree_before, archived_tree_after,
        "archive 树（含磁盘-only 前缀 / 无前缀目录）字节零变化"
    );

    check_or_rewrite("corpus-list-mixed", &list);
}

/// 归档前缀样本语料（D13）：db archived 裸名 + `archived_at` + 磁盘
/// `YYYY-MM-DD-` 前缀目录 → detail 全状态面 golden（status=archived、9 站
/// pipeline 与 runs 全量出线；id / 裸名出线——AC-2 golden 半边）。
#[test]
fn corpus归档前缀样本_golden对拍() {
    let corpus = build_archive_prefix("archive-prefix-golden");
    let projection = project_detail(&corpus, "id-seeded-archived");

    // 全状态面结构断言（golden 之外的语义锚）
    assert_eq!(
        projection.get("id").and_then(|id| id.as_str()),
        Some("id-seeded-archived"),
        "detail id 键入出线"
    );
    assert_eq!(
        projection.get("name").and_then(|name| name.as_str()),
        Some("seeded-archived"),
        "name 恒裸名（归档前缀形态不出现在 name）"
    );
    assert_eq!(
        projection.get("status").and_then(|status| status.as_str()),
        Some("archived"),
        "归档状态面在场（非文档形态空面）"
    );
    let pipeline = projection["pipeline"].as_array().expect("pipeline 为数组");
    assert_eq!(
        pipeline.len(),
        10,
        "10 站 pipeline 全量出线（initialize 表头 + 含 archive 末列，无 code-analyze 占位列）"
    );
    assert!(
        !pipeline[1]["attempts"]
            .as_array()
            .expect("attempts 为数组")
            .is_empty(),
        "proposal 站 attempt 出线（表头 initialize 空站——错配链结构性退役）"
    );
    let runs = projection["runs"].as_array().expect("runs 为数组");
    assert_eq!(
        runs.len(),
        2,
        "run 史全量出线（proposal 史 + archive 相位史）"
    );
    // archive 站 attempt 在案（空 checklist 落账 pass）
    let archive_station = pipeline
        .iter()
        .find(|station| station["phase"].as_str() == Some("archive"))
        .expect("archive 站在场");
    let archive_attempts = archive_station["attempts"]
        .as_array()
        .expect("attempts 为数组");
    assert_eq!(
        archive_attempts.len(),
        1,
        "archive 空 checklist 落账一条尝试"
    );
    assert_eq!(archive_attempts[0]["verdict"].as_str(), Some("pass"));
    assert_eq!(archive_attempts[0]["report"].as_str(), Some(""));
    assert!(
        archive_attempts[0]["checklist"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "空 checklist 落账样本"
    );
    // 归档相位 run 史：十一步节点全词汇（9 归档步词（cleanup 尽力步样本在案）
    // + PhaseStart / PhaseLog）
    let archive_run = runs
        .iter()
        .find(|run| run["runId"].as_str() == Some("run-1727000041000"))
        .expect("归档相位 run 在案");
    let archive_steps: Vec<&str> = archive_run["nodes"]
        .as_array()
        .expect("nodes 为数组")
        .iter()
        .map(|node| node["step"].as_str().expect("step 为串"))
        .collect();
    assert_eq!(
        archive_steps,
        vec![
            "phase_start",
            "preflight",
            "commit",
            "rebase",
            "resolve_conflicts",
            "merge",
            "spec_sync",
            "cleanup",
            "seal",
            "finalize",
            "phase_log",
        ],
        "归档十一步词逐字在案（含 git 步分类词面与 cleanup 尽力步样本）"
    );
    assert!(
        archive_run["edges"]
            .as_array()
            .expect("edges 为数组")
            .iter()
            .all(|edge| edge["edgeKind"].as_str() == Some("forward")),
        "归档串行边全 forward"
    );
    assert!(
        projection["artifacts"]
            .as_array()
            .expect("artifacts 为数组")
            .iter()
            .any(|artifact| artifact["source"].as_str() == Some("proposal.md")),
        "产物清单自归档前缀目录发现"
    );

    check_or_rewrite("corpus-archive-prefix", &projection);
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
        .find_change_record("id-corpus-worktree")
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

/// run 运行史全史语料：两次 run 全史 → detail `runs` / `nodes` / `edges` 投影
/// golden（全史不截、emit 序稳定、十词汇零过滤、四 kind 边齐备、稳定实例键、
/// 时间戳 ISO 出线——AC-3 / AC-4 / AC-6 / AC-10 矩阵）；全词汇在案断言（流程面
/// 步骤与三门不再被过滤弃落）+ feedback 边与实例号递增在场断言。
#[test]
fn corpusrun全史语料_golden对拍() {
    let corpus = build_run_history("run-history-golden");
    let projection = project_detail(&corpus, "id-run-history");

    // 全词汇断言：runs[].nodes[].step 非归档十九词在案（零过滤——flow 面步骤与三
    // 门同整包落库；mutation_test 拆步新词在案，AC-1 / AC-2；归档 9 步词样本由
    // corpus-archive-prefix golden 承载）
    const FULL_WORDS: [&str; 11] = [
        "executor",
        "evaluator",
        "decision",
        "static_check",
        "test_execution",
        "mutation_test",
        "phase_start",
        "phase_log",
        "verdict_gate",
        "retry_gate",
        "whitelist_gate",
    ];
    let runs = projection["runs"].as_array().expect("runs 为数组");
    assert_eq!(
        runs.len(),
        3,
        "三 run 全史样本（run-3 = 步级时间与 token 两态样本，desktop-step-running-snapshot D11）"
    );
    let mut nodes_seen = 0usize;
    let mut seen_words: Vec<&str> = Vec::new();
    for run in runs {
        let nodes = run["nodes"].as_array().expect("nodes 为数组");
        for node in nodes {
            let word = node["step"].as_str().expect("step 出线词");
            assert!(
                FULL_WORDS.contains(&word),
                "落库步词汇 {word} 越二十值封闭集"
            );
            if !seen_words.contains(&word) {
                seen_words.push(word);
            }
            nodes_seen += 1;
        }
    }
    assert_eq!(
        nodes_seen, 16,
        "run-1 十二条（含 mutation_test 拆步新词样本）+ run-2 一条 + run-3 三条（两态样本），实际: {nodes_seen}"
    );
    for word in FULL_WORDS {
        assert!(seen_words.contains(&word), "词汇 {word} 应在案（零过滤）");
    }
    assert!(
        !projection["runs"][0]
            .as_object()
            .expect("run 为对象")
            .contains_key("steps"),
        "steps 键退役（nodes / edges 取代）"
    );

    // 存量 `retry` 行投影前屏蔽（读面唯一屏蔽点）+ 其余三 kind 边齐备 +
    // feedback 边端点指回同相位 agent 步 + 实例号递增
    let edges = projection["runs"][0]["edges"]
        .as_array()
        .expect("edges 为数组");
    let kinds: Vec<&str> = edges
        .iter()
        .map(|row| row["edgeKind"].as_str().expect("edgeKind 出线词"))
        .collect();
    for kind in ["forward", "feedback", "backtrack"] {
        assert!(kinds.contains(&kind), "边 kind {kind} 应在案：{kinds:?}");
    }
    assert!(
        !kinds.contains(&"retry"),
        "存量 retry 边投影后缺席（AC-8 读面屏蔽）：{kinds:?}"
    );
    let feedback = edges
        .iter()
        .find(|row| row["edgeKind"] == "feedback")
        .expect("feedback 边在案");
    assert_eq!(feedback["fromNodeKey"], "implement:1:static_check:0");
    assert_eq!(feedback["toNodeKey"], "implement:1:executor:1");
    let nodes = projection["runs"][0]["nodes"]
        .as_array()
        .expect("nodes 为数组");
    let instances: Vec<u64> = nodes
        .iter()
        .filter(|node| node["step"] == "executor")
        .map(|node| node["instanceNo"].as_u64().expect("instanceNo 为数字"))
        .collect();
    assert_eq!(instances, vec![0, 1], "feedback 重跑实例号递增可辨");
    assert_eq!(
        projection["runs"][0]["nodes"][2]["outputRef"], "exec-1727000019000",
        "outputRef 出线（ExecOutput::Recorded 引用）"
    );

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
        &project_detail(&corpus, "id-run-interrupted"),
    );
}

/// worktree 语料投影：detail 全读链 + 路径归一（tempdir 绝对路径前缀 →
/// `<WORKTREE_ROOT>` 固定占位——golden 确定性；归一只作用 worktree 执行锚
/// 字符串值，其余投影零改写）。
fn project_worktree_detail(corpus: &Corpus) -> serde_json::Value {
    let mut value =
        serde_json::to_value(corpus.detail("id-corpus-worktree")).expect("ChangeDetail 序列化失败");
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
                    !matches!(
                        key.as_str(),
                        "inventory" | "fileLog" | "unparsable" | "steps"
                    ),
                    "golden {path} 含退役键 {key}（线面字段删除被破坏）"
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

/// 半包形态语料（异常，AC-5 / AC-10）：种子含半包形态——边端点悬空（`ghost`
/// 节点不在本包节点集）/ 节点身份键重复——收口单事务显式拒绝（零半包入库：
/// run 行仍 running、节点 / 边零行），详情投影 nodes / edges 恒空数组，该语料
/// **零 golden 产出**（半包不入仓，与语料完整性集合一致）。
#[test]
fn corpus半包形态种子_整包拒绝且零golden产出() {
    let corpus = Corpus::new("half-pack");
    corpus.seed_record("id-half-pack", "half-pack");
    corpus.run_start("id-half-pack", "run-1727000030000", t(30));

    let request = RunRequest {
        root: corpus.root.to_string_lossy().into_owned(),
        main_root: corpus.root.to_string_lossy().into_owned(),
        stop_before: Some("archive".to_owned()),
        change_id: "id-half-pack".to_owned(),
        run_id: "run-1727000030000".to_owned(),
        auto_next_phase: true,
        started_at: t(30),
    };

    // 半包形态一：边端点悬空（`ghost` 目标节点不在本包节点集内）
    let dangling = finish_command(
        &request,
        ChangeRunStatus::Completed,
        Some("半包形态：边端点悬空".to_owned()),
        t(39),
        &[
            step(
                "proposal",
                1,
                ChangeStepKind::PhaseStart,
                ChangeStepStatus::Passed,
                None,
                None,
            ),
            step(
                "proposal",
                1,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-half-pack"),
                None,
            ),
        ],
        &[edge(
            key("proposal", 1, ChangeStepKind::PhaseStart, 0),
            key("ghost", 1, ChangeStepKind::Executor, 0),
            EdgeKind::Backtrack,
        )],
    );
    let error = corpus
        .store
        .finish_change_run(&dangling)
        .expect_err("悬空边端点整包应收口拒绝");
    assert!(
        matches!(&error, store::StoreError::Conflict(_)),
        "整包不变量破例 → 冲突面显式拒绝: {error}"
    );
    assert!(
        error.to_string().contains("悬空"),
        "记因点名悬空端点: {error}"
    );

    // 半包形态二：节点身份键重复（同 `(phase, attempt, step, instance_no)` 两行
    // ——整包节点集非唯一；「节点 / 边数目不一致」在包闭约束下不可构造：边端点
    // 恒须指向本包节点，破例形态即上述悬空 / 撞键两支）
    let duplicated = finish_command(
        &request,
        ChangeRunStatus::Completed,
        Some("半包形态：节点身份键重复".to_owned()),
        t(39),
        &[
            step(
                "proposal",
                1,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-half-pack"),
                None,
            ),
            step(
                "proposal",
                1,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-half-pack"),
                None,
            ),
        ],
        &[],
    );
    let error = corpus
        .store
        .finish_change_run(&duplicated)
        .expect_err("节点身份键重复整包应收口拒绝");
    assert!(
        matches!(&error, store::StoreError::Conflict(_)),
        "身份键非唯一 → 冲突面显式拒绝: {error}"
    );
    assert!(
        error.to_string().contains("重复"),
        "记因点名身份键重复: {error}"
    );

    // 零半包入库：run 行仍 running、节点 / 边零行（整包拒绝即单事务回滚）
    let runs = corpus
        .store
        .list_change_runs("id-half-pack")
        .expect("run 清单应成功");
    assert_eq!(runs.len(), 1, "起始行在案");
    assert_eq!(
        runs[0].status,
        RunStatus::Running,
        "半包拒绝不写终态（run 行仍 running）"
    );
    assert!(
        corpus
            .store
            .list_run_nodes("run-1727000030000")
            .expect("节点读试应成功")
            .is_empty(),
        "半包节点零落库"
    );
    assert!(
        corpus
            .store
            .list_run_edges("run-1727000030000")
            .expect("边读试应成功")
            .is_empty(),
        "半包边零落库"
    );

    // 投影面：半包零具形（nodes / edges 空数组）、detail 仍可定位（running 起始行）
    let projection = project_detail(&corpus, "id-half-pack");
    assert_eq!(
        projection["runs"][0]["status"], "running",
        "起始行状态面出线: {projection}"
    );
    assert!(
        projection["runs"][0]["nodes"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "半包零具形：nodes 空数组（半包零投射）: {projection}"
    );
    assert!(
        projection["runs"][0]["edges"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "半包零具形：edges 空数组: {projection}"
    );

    // golden 不产出：半包语料不入仓（与「语料完整性」golden 集合一致）
    assert!(
        !golden_dir().join("corpus-half-pack.json").exists(),
        "半包形态零 golden 产出（过不了整包不变量即无可对拍投影）"
    );
}

/// 显式重写流程（边界行）：重写后重跑绿的 diff 范围进程内断言——detail 段
/// 恒含 `id` 键、无 `inventory` / `fileLog` / `unparsable` 三退役键、
/// `status` / `activePhase` 状态面键在场；list 条目含 `id`；归档前缀样本行
/// 在场（归档全状态面）。
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
        "corpus-archive-prefix",
    ] {
        let text = fs::read_to_string(golden_dir().join(format!("{name}.json")))
            .unwrap_or_else(|err| panic!("golden {name}.json 应存在: {err}"));
        let value: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("golden {name}.json 应为合法 JSON: {err}"));

        assert_no_retired_keys(&value, name);
        assert!(
            value.get("id").and_then(|id| id.as_str()).is_some(),
            "{name} detail 恒含 id 键（身份键入——D8 增量）"
        );
        assert!(
            value.get("activePhase").is_some(),
            "{name} activePhase 键恒在场（null 不省键）"
        );
        assert!(
            value.get("worktree").is_some(),
            "{name} worktree 键恒在场（legacy 建档样本 null 留位——防静默漂移）"
        );
        assert!(
            value
                .get("title")
                .and_then(|title| title.as_str())
                .is_some_and(|title| !title.is_empty()),
            "{name} title 键恒在场且恒非空（AC-9 detail title 出线）"
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

    // active / archive 两态状态面（档案判别面改自记录直读）
    for name in [
        "corpus-multi-attempt",
        "corpus-backtrack-stale",
        "corpus-slots-null",
        "corpus-run-history",
        "corpus-run-interrupted",
    ] {
        let value: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(golden_dir().join(format!("{name}.json")))
                .unwrap_or_else(|err| panic!("golden {name}.json 应存在: {err}")),
        )
        .unwrap_or_else(|err| panic!("golden {name}.json 应为合法 JSON: {err}"));
        assert_eq!(
            value.get("status").and_then(|status| status.as_str()),
            Some("active"),
            "{name} 状态面键出线（建档判别面）"
        );
        assert_eq!(
            value.get("worktree"),
            Some(&serde_json::Value::Null),
            "{name} legacy 建档样本 worktree 出线 null（D16 ①）"
        );
    }

    // 归档前缀样本（D13 新增行）：status=archived、裸名、id 键入、全状态面
    let text = fs::read_to_string(golden_dir().join("corpus-archive-prefix.json"))
        .expect("corpus-archive-prefix golden 应存在");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("corpus-archive-prefix golden 应为合法 JSON");
    assert_no_retired_keys(&value, "corpus-archive-prefix");
    assert_eq!(
        value.get("id").and_then(|id| id.as_str()),
        Some("id-seeded-archived"),
        "归档前缀样本 id 键入出线"
    );
    assert_eq!(
        value.get("name").and_then(|name| name.as_str()),
        Some("seeded-archived"),
        "归档前缀样本 name 恒裸名"
    );
    assert_eq!(
        value.get("status").and_then(|status| status.as_str()),
        Some("archived"),
        "归档前缀样本状态面 archived 在案"
    );
    let pipeline = value["pipeline"].as_array().expect("pipeline 为数组");
    assert_eq!(pipeline.len(), 10, "归档前缀样本 10 站 pipeline 全量出线");
    assert_eq!(
        pipeline
            .last()
            .and_then(|station| station["phase"].as_str()),
        Some("archive"),
        "归档前缀样本末列为 archive"
    );
    assert_eq!(
        value["runs"].as_array().expect("runs 为数组").len(),
        2,
        "归档前缀样本 runs 全量出线（proposal 史 + archive 相位史）"
    );

    // list golden：条目 id 恒在案、归档条目裸名、磁盘-only 零呈现
    let text = fs::read_to_string(golden_dir().join("corpus-list-mixed.json"))
        .expect("corpus-list-mixed golden 应存在");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("corpus-list-mixed golden 应为合法 JSON");
    assert_no_retired_keys(&value, "corpus-list-mixed");
    let active = value["active"].as_array().expect("active 为数组");
    assert!(
        active
            .iter()
            .all(|entry| entry["id"].as_str().is_some_and(|id| !id.is_empty())),
        "list active 条目 id 恒在案"
    );
    let archived_names: Vec<&str> = value["archiveGroups"]
        .as_array()
        .expect("archiveGroups 为数组")
        .iter()
        .flat_map(|group| {
            group["changes"]
                .as_array()
                .expect("组内条目为数组")
                .iter()
                .filter_map(|entry| entry["name"].as_str())
        })
        .collect();
    assert!(
        archived_names.contains(&"seeded-archived"),
        "归档前缀样本行在场（裸名）：{archived_names:?}"
    );
    assert!(
        archived_names.iter().all(|name| !name.starts_with("20")),
        "归档条目恒裸名（零日期前缀）：{archived_names:?}"
    );
    assert!(
        !archived_names
            .iter()
            .any(|name| name.contains("disk-archived")),
        "磁盘-only 归档条目零呈现：{archived_names:?}"
    );

    // worktree 建档语料（非 null 投影——两态齐备的对拍半边；D16 ②）
    let text = fs::read_to_string(golden_dir().join("corpus-worktree.json"))
        .expect("corpus-worktree golden 应存在");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("corpus-worktree golden 应为合法 JSON");
    assert_no_retired_keys(&value, "corpus-worktree");
    assert_eq!(
        value.get("id").and_then(|id| id.as_str()),
        Some("id-corpus-worktree"),
        "worktree 建档样本 id 键入出线"
    );
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
        "归档前缀",
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

    assert!(!outcome.id.is_empty(), "铸出 id 非空");
    assert_eq!(outcome.name, "create-triple", "name 裸名属性");
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
        .find_change_record(&outcome.id)
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.id, outcome.id, "outcome.id 与库内逐字一致");
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

    // db 同名 active → Err 且零目录创建（查重按 name 扫描——仅拒 active）
    corpus.seed_record("id-db-taken", "db-taken");
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
            &store,
            &FakeVcs::new(worktree_root),
            "fix-bug",
            "补偿路径 goal",
            "fix-bug",
            "requirement",
        )
        .expect_err("fs 半边失败应 Err");
        assert!(
            error.contains("补偿"),
            "Err 呈现补偿回滚事实，实际: {error}"
        );
        assert!(
            store.list_change_records().expect("db 读应成功").is_empty(),
            "补偿删除本次自插行（按 id——list 零残留）"
        );
    }

    // 重开 db：零本次残留（持久层证据）
    let reopened = Store::open_workspace(&db_path).expect("重开 db 失败");
    assert!(
        reopened
            .list_change_records()
            .expect("重开 db 读应成功")
            .is_empty(),
        "重开 db 零本次残留"
    );
}

/// create 成功立即可见可发起（AC-6 组合行）：list / detail 立即可见（db 形态
/// 状态面在场）、phase_next 路由首相位。
#[test]
fn 真件create成功立即可见可发起() {
    let corpus = Corpus::new("create-visible");
    let layout = resolve(&corpus.root);

    let outcome = corpus
        .create("visible-change", "组合用例 goal")
        .expect("create 应 Ok");

    let list = list_changes(&corpus.store);
    assert_eq!(list.active.len(), 1, "active 恰一条");
    assert_eq!(list.active[0].id, outcome.id, "列表条目 id = 铸出 id");
    assert_eq!(list.active[0].name, "visible-change");
    assert_eq!(list.active[0].status, Some(ChangeStatus::Active));

    let detail = change_detail(&layout, &corpus.store, &outcome.id)
        .expect("详情读面应 Ok")
        .expect("详情应可达");
    assert_eq!(detail.id, outcome.id, "detail id = 铸出 id");
    assert_eq!(detail.name, "visible-change");
    assert_eq!(detail.status, Some(ChangeStatus::Active));

    let route = phase_next(
        &corpus.store,
        &outcome.id,
        "run-visible",
        &SessionAnchors::new(),
    )
    .expect("建档后路由应可达");
    assert_eq!(
        route.next_phase.as_deref(),
        Some("initialize"),
        "建档立即可发起（表头相位路由——零条目落 initialize）"
    );
}

/// archive 双写：目录改名入 archive 树（日期前缀）+ db status 翻转；记录
/// id / name 双不变；随后 list 归档条目按月分组可达（裸名 + id + 月分组——
/// AC-7 + 分组可达组合行）。
#[test]
fn 真件archive双写_目录改名与db翻转且按月分组可达() {
    let corpus = Corpus::new("archive-dual");
    let layout = resolve(&corpus.root);
    corpus.seed_record("id-seed-change", "seed-change");
    fs::create_dir_all(layout.changes_root.join("seed-change")).expect("预置目录失败");
    // archive 树在场（fs::rename 不建目标父目录，真实 workspace 归档树常在）
    fs::create_dir_all(&layout.archive_root).expect("预置 archive 树失败");
    let before = utc_date_today();

    let outcome: ArchiveOutcome =
        archive(&layout, &corpus.store, "id-seed-change").expect("归档应成功");

    let after = utc_date_today();
    assert_eq!(
        outcome.name, "seed-change",
        "记录 name（裸名）不随目录改名变"
    );
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
        .find_change_record("id-seed-change")
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.id, "id-seed-change", "记录 id 不随改名变");
    assert_eq!(record.name, "seed-change", "记录 name 不随改名变");
    assert_eq!(record.status, ChangeStatus::Archived);
    assert!(record.archived_at.is_some());

    // 归档条目按月分组可达（db 单源：裸名 + id + archived_at 月——AC-7 分组半边）
    let list = list_changes(&corpus.store);
    let entry = list
        .archive_groups
        .iter()
        .flat_map(|group| group.changes.iter())
        .find(|entry| entry.name == "seed-change")
        .unwrap_or_else(|| panic!("归档条目应按月分组可达"));
    assert_eq!(entry.id, "id-seed-change", "归档条目 id 恒在案");
    assert_eq!(
        entry.name, "seed-change",
        "归档条目 name 恒裸名（零目录前缀）"
    );
    let group = list
        .archive_groups
        .iter()
        .find(|group| group.changes.iter().any(|e| e.name == "seed-change"))
        .expect("归档组应在场");
    assert_eq!(
        group.month.as_deref(),
        Some(&outcome.archived_date[..7]),
        "月分组 archived_at 权威（与目录前缀一致）"
    );
    assert_eq!(entry.status, Some(ChangeStatus::Archived), "状态面随 db");
}

/// archive 续半边重试（D6）：目录已在 archive 树 + db 仍 active → 重试仅补
/// db 翻转，archive 树源目录不被二次挪动（id 寻址；name 自记录供给改名）。
#[test]
fn 真件archive续半边重试_仅补db翻转() {
    let corpus = Corpus::new("archive-resume");
    let layout = resolve(&corpus.root);
    corpus.seed_record("id-seed-change", "seed-change");
    let archived_name = "2026-10-06-seed-change";
    fs::create_dir_all(layout.archive_root.join(archived_name)).expect("预置 archive 目录失败");

    let outcome = archive(&layout, &corpus.store, "id-seed-change").expect("续半边归档应成功");

    assert_eq!(outcome.archived_date, "2026-10-06", "取前缀日期");
    assert_eq!(outcome.name, "seed-change", "name 自记录供给");
    assert!(
        layout.archive_root.join(archived_name).is_dir(),
        "archive 树源目录原位不动"
    );
    let record = corpus
        .store
        .find_change_record("id-seed-change")
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.status, ChangeStatus::Archived, "db 补翻转");
}

/// archive 无建档拒绝：active 目录在场 db 零记录 → 显式 `Err` 且零 fs 零 db
/// 变更（存量 CLI 目录不可经桌面归档的显式面；错误文案呈现 id 语境）。
#[test]
fn 真件archive无建档拒绝_零fs零db变更() {
    let corpus = Corpus::new("archive-no-record");
    let layout = resolve(&corpus.root);
    fs::create_dir_all(layout.changes_root.join("seed-change")).expect("预置目录失败");

    let error = archive(&layout, &corpus.store, "id-no-record").expect_err("无建档应 Err");

    assert!(
        error.contains("未建档") && error.contains("id-no-record"),
        "Err 显式记因建档缺失（id 语境），实际: {error}"
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
/// 变更（id 寻址；判定点施于记录供给的 name）。
#[test]
fn 真件archive目标冲突_db零变更() {
    let corpus = Corpus::new("archive-target-conflict");
    let layout = resolve(&corpus.root);
    corpus.seed_record("id-seed-change", "seed-change");
    fs::create_dir_all(layout.changes_root.join("seed-change")).expect("预置目录失败");
    fs::create_dir_all(
        layout
            .archive_root
            .join(format!("{}-seed-change", utc_date_today())),
    )
    .expect("预置冲突目标失败");

    let error = archive(&layout, &corpus.store, "id-seed-change").expect_err("目标已存在应 Err");

    assert!(error.contains("已存在"), "先查拒绝记因，实际: {error}");
    let record = corpus
        .store
        .find_change_record("id-seed-change")
        .expect("db 读应成功")
        .expect("建档记录应在场");
    assert_eq!(record.status, ChangeStatus::Active, "db 零变更");
}

/// phase_next 读源 db（AC-3 读墙随动）：真实 store 种子驱动路由；change 目录
/// 内损坏 workflow.json 字节在场仍路由成功——零 workflow.json 读取。
#[test]
fn 真件phase_next读源db_零workflow_json读取() {
    let corpus = Corpus::new("route-read-db");
    corpus.seed_record("id-demo-change", "demo-change");
    corpus.run_phase(
        "id-demo-change",
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        None,
        None,
        t(0),
    );
    corpus.run_phase(
        "id-demo-change",
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
        "id-demo-change",
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
    corpus.seed_record("id-demo-change", "demo-change");
    corpus.run_phase(
        "id-demo-change",
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        None,
        None,
        t(0),
    );
    corpus.run_phase(
        "id-demo-change",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    // 历史 fail 挂决策标记（已决形态）：未决 fail 会被 verdict-fail 决策分叉
    // 捕获，本用例聚焦锚点 / round 语义
    corpus.run_phase(
        "id-demo-change",
        "dev-design",
        Verdict::Fail,
        "历史失败一",
        Vec::new(),
        None,
        None,
        t(2),
    );
    corpus
        .store
        .amend_change_decision_session("id-demo-change", "dev-design", "sess-seed-1")
        .expect("决策挂账种子应成功");
    corpus.run_phase(
        "id-demo-change",
        "dev-design",
        Verdict::Fail,
        "历史失败二",
        Vec::new(),
        None,
        None,
        t(3),
    );
    corpus
        .store
        .amend_change_decision_session("id-demo-change", "dev-design", "sess-seed-2")
        .expect("决策挂账种子应成功");

    let anchors = SessionAnchors::new();
    let first = phase_next(&corpus.store, "id-demo-change", "run-a", &anchors).expect("路由应成功");
    assert_eq!(
        first.round, 1,
        "首见锚点 = PhaseRecord 行数 4（initialize 表头 + 三条目），round 自 1 起"
    );
    assert_eq!(first.next_phase.as_deref(), Some("dev-design"));
    assert!(first.error.is_none(), "窗口外历史 fail 不虚触上限");

    // 锚点复见：基线后新落 1 行 → round = 1（窗口）+ 1 = 2
    corpus.run_phase(
        "id-demo-change",
        "dev-design",
        Verdict::Fail,
        "run 内新增",
        Vec::new(),
        None,
        None,
        t(4),
    );
    let second =
        phase_next(&corpus.store, "id-demo-change", "run-a", &anchors).expect("路由应成功");
    assert_eq!(second.round, 2, "round = 窗口条目数 + 1（非全量行数 + 1）");

    // run_id 键隔离：新键重新锚定当前行数 4 → round 归位 1
    let third = phase_next(&corpus.store, "id-demo-change", "run-b", &anchors).expect("路由应成功");
    assert_eq!(third.round, 1);
}

/// phase_next 重启续走（AC-5）：新 SessionAnchors 实例（模拟重启）铸新锚点
/// ——已 pass 相位行在场 → 路由直接推进不重头执行。
#[test]
fn 真件phase_next重启续走_直接推进不重头() {
    let corpus = Corpus::new("route-restart");
    corpus.seed_record("id-demo-change", "demo-change");
    corpus.run_phase(
        "id-demo-change",
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        None,
        None,
        t(0),
    );
    corpus.run_phase(
        "id-demo-change",
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        t(1),
    );
    corpus.run_phase(
        "id-demo-change",
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
        "id-demo-change",
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

/// list db 单源归组（真件）：db active 而目录已改名入 archive 树 → 仍随 db
/// 留 active 组（db status 权威归组，磁盘目录零参与），查询路径不回写 db
/// （纯读纪律）。
#[test]
fn 真件list_db单源_db_status权威且不回写db() {
    let corpus = Corpus::new("list-d11");
    corpus.seed_record("id-d11-change", "d11-change");
    corpus.dir_with_files(
        "openspec/changes/archive/2026-10-01-d11-change",
        &[("proposal.md", "# 归档")],
    );

    let list = list_changes(&corpus.store);

    let entry = list
        .active
        .iter()
        .find(|entry| entry.name == "d11-change")
        .expect("db active 条目应留 active 组（db status 权威，磁盘目录不参与归组）");
    assert_eq!(entry.id, "id-d11-change", "条目 id 恒在案");
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
        .find_change_record("id-d11-change")
        .expect("db 读应成功")
        .expect("db 记录应在场");
    assert_eq!(record.status, ChangeStatus::Active, "读后 db 仍 active");
}

/// list db 单源（真件）：磁盘-only 目录零呈现零报错零触碰（原「并集与去重
/// 以 db 形态为准」反转——db 记录全量出线且磁盘目录字节零变化）。
#[test]
fn 真件list_db单源_磁盘only零呈现零报错() {
    let corpus = Corpus::new("list-union");
    let layout = resolve(&corpus.root);
    corpus.seed_record("id-db-change", "db-change");
    corpus
        .store
        .start_change_phase("id-db-change", "dev-design", t(6))
        .expect("开相失败");
    corpus.dir_with_files("openspec/changes/db-change", &[("proposal.md", "# 提案")]);
    // 磁盘-only 目录（含惰性字节样本——零发现反例）
    corpus.dir_with_files("openspec/changes/disk-only", &[("proposal.md", "# 提案")]);
    corpus.dir_with_files(
        "openspec/changes/disk-only",
        &[("workflow.json", "{ \"LAZY_BYTES_MARKER\": \"零呈现样本\" }")],
    );
    corpus.seed_record("id-dual-change", "dual-change");
    corpus.dir_with_files("openspec/changes/dual-change", &[("proposal.md", "# 提案")]);
    let bytes_before = snapshot_dir_bytes(&layout.changes_root.join("disk-only"));

    let list = list_changes(&corpus.store);

    let names: Vec<&str> = list
        .active
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["db-change", "dual-change"],
        "db 记录全量出线（按名排序），磁盘-only 零呈现"
    );
    let dual = list
        .active
        .iter()
        .find(|entry| entry.name == "dual-change")
        .expect("dual 条目应在场");
    assert_eq!(dual.id, "id-dual-change", "条目 id 恒在案");
    assert_eq!(dual.status, Some(ChangeStatus::Active), "状态面恒在场");
    assert!(
        !list.active.iter().any(|entry| entry.name == "disk-only"),
        "磁盘-only 条目零呈现"
    );
    // 磁盘目录字节零变化（零读取零写入零报错）
    let bytes_after = snapshot_dir_bytes(&layout.changes_root.join("disk-only"));
    assert_eq!(bytes_before, bytes_after, "磁盘-only 目录字节零变化");
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
            id: "id-zero-ts".to_owned(),
            name: "zero-ts".to_owned(),
            title: "zero-ts".to_owned(),
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
        "id-zero-ts",
        "proposal",
        Verdict::Pass,
        "通过",
        Vec::new(),
        None,
        None,
        0,
    );
    corpus.dir_with_files("openspec/changes/zero-ts", &[("proposal.md", "# 提案")]);

    let detail = change_detail(&resolve(&corpus.root), &corpus.store, "id-zero-ts")
        .expect("详情读面应 Ok")
        .expect("详情应可达");

    assert_eq!(detail.created.as_deref(), Some("1970-01-01"));
    let record = &detail.pipeline[1].attempts[0];
    assert_eq!(record.timestamp.as_deref(), Some("1970-01-01T00:00:00Z"));
    assert_eq!(record.start_at.as_deref(), Some("1970-01-01T00:00:00Z"));
}

/// detail 未找到与恒可达：未建档 id（磁盘任意目录在场——裸名 / 前缀名形态）
/// → 恒 `None`（零磁盘回退零文档形态）；建档记录定位全 miss（worktree 缺席、
/// 主仓两树未命中）→ 恒可达（D12：dir 缺席、产物清单空、状态面在——source
/// 自 record.status 映射）。
#[test]
fn 真件detail未找到与建档恒可达() {
    let corpus = Corpus::new("detail-not-found");
    let layout = resolve(&corpus.root);
    corpus.dir_with_files("openspec/changes/real", &[("proposal.md", "# 提案")]);
    corpus.dir_with_files(
        "openspec/changes/archive/2026-01-01-ghost",
        &[("proposal.md", "# 归档")],
    );

    assert!(
        change_detail(&layout, &corpus.store, "不存在的change")
            .expect("详情读面应 Ok")
            .is_none(),
        "未建档 id → None（磁盘裸名目录在场不命中）"
    );
    assert!(change_detail(&layout, &corpus.store, "")
        .expect("详情读面应 Ok")
        .is_none());
    assert!(
        change_detail(&layout, &corpus.store, "a/b")
            .expect("详情读面应 Ok")
            .is_none(),
        "未建档任意 id 形态 → None（id 为不透明串，恒由 db 记录定夺）"
    );
    assert!(
        change_detail(&layout, &corpus.store, "2026-01-01-ghost")
            .expect("详情读面应 Ok")
            .is_none(),
        "前缀名寻址未建档 id → None（MUST NOT 回退目录解析）"
    );

    corpus.seed_record("id-ghost-with-record", "ghost-with-record");
    let detail = change_detail(&layout, &corpus.store, "id-ghost-with-record")
        .expect("详情读面应 Ok")
        .expect("建档记录恒可达详情（定位 miss 不虚构 None）");
    assert_eq!(detail.id, "id-ghost-with-record", "detail id = 寻址 id");
    assert_eq!(detail.name, "ghost-with-record", "name 自记录直读");
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
    corpus.seed_record("id-run-write", "run-write");
    let store = &corpus.store;
    let start = RunStartCommand {
        run_id: "run-1727000030000".to_owned(),
        change_id: "id-run-write".to_owned(),
        started_at: t(30),
    };
    workflow::write::run_start(store, &start).expect("run_start 应成功");

    let runs = store.list_change_runs("id-run-write").expect("list 应成功");
    assert_eq!(runs.len(), 1, "恰一条 running 行");
    assert_eq!(runs[0].run_id, "run-1727000030000");
    assert_eq!(runs[0].change_id, "id-run-write", "change_id 归属键在案");
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
                change_id: "id-run-write".to_owned(),
                started_at: t(31),
            },
        )
        .expect("run_start 应成功");
        let request = RunRequest {
            root: corpus.root.to_string_lossy().into_owned(),
            main_root: corpus.root.to_string_lossy().into_owned(),
            stop_before: Some("archive".to_owned()),
            change_id: "id-run-write".to_owned(),
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
                instance_no: 0,
                status: ChangeStepStatus::Passed,
                session_id: Some("ses-run-write".to_owned()),
                detail: None,
                output_ref: None,
                started_at: t(STEP_T_BASE),
                ended_at: Some(t(STEP_T_BASE) + STEP_DUR_MS),
                token_input: None,
                token_output: None,
            }],
            &[],
        );
        workflow::write::run_finish(store, &command)
            .unwrap_or_else(|e| panic!("{status:?} 收口应成功: {e}"));

        let runs = store.list_change_runs("id-run-write").expect("list 应成功");
        let row = runs
            .iter()
            .find(|row| row.run_id == run_id)
            .unwrap_or_else(|| panic!("run {run_id} 行应在场"));
        assert_eq!(row.status, status, "终态逐值落库");
        assert_eq!(row.reason.as_deref(), Some("真件收口记因"));
        assert_eq!(row.finished_at, Some(t(39)));
        let nodes = store.list_run_nodes(&run_id).expect("节点读应成功");
        assert_eq!(nodes.len(), 1, "节点整包随收口落库");
        assert_eq!(nodes[0].step, workflow::state::RunStepKind::Executor);
        assert_eq!(nodes[0].instance_no, 0, "稳定身份键实例号在案");
        assert_eq!(nodes[0].status, workflow::state::RunStepStatus::Passed);
        assert_eq!(nodes[0].session_id.as_deref(), Some("ses-run-write"));
        assert_eq!(nodes[0].timestamp, t(39), "整包同刻 = finished_at");
        assert!(
            store
                .list_run_edges(&run_id)
                .expect("边读应成功")
                .is_empty(),
            "零边 run 边整包空集合法"
        );
    }
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（D11 / AC-9）：golden 断言面——步级时间与
// token 两态样本在案、瞬步 / 在途形态入 golden、四 camelCase 键漂移守卫
//（旧 golden 未重写时显式红、非静默通过）
// ---------------------------------------------------------------------------

/// 步级时间与 token 两态样本（时间三态：正常起止双值 / 瞬步同刻 / running
/// 收口在途 ended=None；token 两态：WorkerAgent 带值 / 工具步 None）经
/// run-3 语料与全语料 nodes 深查在案；漂移守卫对投影与入仓 golden 双面深查
/// runs[].nodes[] 四 camelCase 键在场。
#[test]
fn corpusrun全史语料_步级时间与token两态golden在案与漂移守卫() {
    let corpus = build_run_history("run-history-guard");
    let projection = project_detail(&corpus, "id-run-history");
    let runs = projection["runs"].as_array().expect("runs 数组");
    assert_eq!(runs.len(), 3, "三 run 全史（run-3 两态样本）");
    let run3_nodes = runs[2]["nodes"].as_array().expect("nodes 数组");
    assert_eq!(run3_nodes.len(), 3, "run-3 三步：瞬步 / token 带值 / 在途");

    let find_node = |step: &str| -> serde_json::Value {
        run3_nodes
            .iter()
            .find(|node| node["step"] == step)
            .unwrap_or_else(|| panic!("run-3 应含 {step} 节点"))
            .clone()
    };

    // 瞬步样本（verdict_gate）：startedAt == endedAt 同刻定值（0ms 形态入
    // golden——两键同值非省键）；token 两键在场 null
    let instant = find_node("verdict_gate");
    assert_eq!(instant["startedAt"], serde_json::json!(1727000060000_i64));
    assert_eq!(
        instant["startedAt"], instant["endedAt"],
        "瞬步 started == ended 同刻定值"
    );
    assert!(instant["tokenInput"].is_null() && instant["tokenOutput"].is_null());

    // 正常起止双值步（executor）+ token 两值（WorkerAgent 步带值两态之一）
    let worker = find_node("executor");
    let started = worker["startedAt"].as_i64().expect("startedAt 数值");
    let ended = worker["endedAt"].as_i64().expect("endedAt 数值");
    assert!(ended > started, "正常起止双值：ended > started");
    assert_eq!(
        worker["tokenInput"],
        serde_json::json!(12345),
        "tokenInput 带值态"
    );
    assert_eq!(
        worker["tokenOutput"],
        serde_json::json!(6789),
        "tokenOutput 带值态"
    );

    // running 收口在途样本（evaluator）：endedAt null 出线不省键（键在场）
    let in_flight = find_node("evaluator");
    assert!(
        in_flight
            .as_object()
            .expect("节点对象")
            .contains_key("endedAt"),
        "在途样本 endedAt 键在场"
    );
    assert!(
        in_flight["endedAt"].is_null(),
        "在途 endedAt null 原样（零回填）"
    );
    assert!(in_flight["tokenInput"].is_null());

    // token 两态之二：工具步 None（run-1 任一 phase_start 行 tokenInput null）
    let tool_node = runs[0]["nodes"]
        .as_array()
        .expect("run-1 nodes")
        .iter()
        .find(|node| node["step"] == "phase_start")
        .expect("run-1 phase_start 在案");
    assert!(
        tool_node["tokenInput"].is_null(),
        "工具步 tokenInput null 两态在案"
    );
    assert!(tool_node["tokenOutput"].is_null());

    // 漂移守卫（投影面）：全语料 runs[].nodes[] 四 camelCase 键逐键在场
    let four_keys = ["startedAt", "endedAt", "tokenInput", "tokenOutput"];
    for run in runs {
        for node in run["nodes"].as_array().expect("nodes 数组") {
            let object = node.as_object().expect("节点对象");
            for key in four_keys {
                assert!(
                    object.contains_key(key),
                    "投影节点缺 {key} 键（线面漂移）: {node}"
                );
            }
        }
    }

    // 漂移守卫（golden 面）：入仓 golden 同断言——旧 golden 未重写时显式红、
    // 非静默通过
    let golden_text =
        fs::read_to_string(golden_dir().join("corpus-run-history.json")).expect("读 golden 失败");
    let golden: serde_json::Value = serde_json::from_str(&golden_text).expect("golden 解析失败");
    for run in golden["runs"].as_array().expect("golden runs") {
        for node in run["nodes"].as_array().expect("golden nodes") {
            let object = node.as_object().expect("golden 节点对象");
            for key in four_keys {
                assert!(
                    object.contains_key(key),
                    "golden 节点缺 {key} 键（golden 未随线面重写——漂移即红）"
                );
            }
        }
    }

    // 全量序列化零 panic（既有规模语料 + 四字段注入可全量出线）
    let full = serde_json::to_string(&projection).expect("全量序列化应成功");
    assert!(
        full.contains("\"tokenInput\":12345"),
        "全量序列化含 token 带值行"
    );
    check_or_rewrite("corpus-run-history", &projection);
}

/// 归档前缀语料（多节点全词汇样本）同享漂移守卫：runs[].nodes[] 四 camelCase
/// 键逐键在场（投影面 + golden 面双查——多语料覆盖不单点）。
#[test]
fn corpus归档前缀语料_节点四键漂移守卫() {
    let corpus = build_archive_prefix("archive-prefix-guard");
    let projection = project_detail(&corpus, "id-seeded-archived");
    let four_keys = ["startedAt", "endedAt", "tokenInput", "tokenOutput"];
    let runs = projection["runs"].as_array().expect("runs 数组");
    assert!(!runs.is_empty(), "归档前缀语料含 run 全史");
    let mut nodes_seen = 0usize;
    for run in runs {
        for node in run["nodes"].as_array().expect("nodes 数组") {
            let object = node.as_object().expect("节点对象");
            for key in four_keys {
                assert!(object.contains_key(key), "投影节点缺 {key} 键: {node}");
            }
            nodes_seen += 1;
        }
    }
    assert_eq!(nodes_seen, 12, "归档前缀语料 12 节点全查（多节点规模锚）");

    let golden_text = fs::read_to_string(golden_dir().join("corpus-archive-prefix.json"))
        .expect("读 golden 失败");
    let golden: serde_json::Value = serde_json::from_str(&golden_text).expect("golden 解析失败");
    for run in golden["runs"].as_array().expect("golden runs") {
        for node in run["nodes"].as_array().expect("golden nodes") {
            let object = node.as_object().expect("golden 节点对象");
            for key in four_keys {
                assert!(
                    object.contains_key(key),
                    "golden 节点缺 {key} 键（漂移即红）"
                );
            }
        }
    }
    check_or_rewrite("corpus-archive-prefix", &projection);
}

/// 防御值语料（D11 种子面零逻辑分支锚）：`step_metrics` overrides 混防御值
/// ——`started_at` 负值 / 0（缺席语义值）、token 单侧（入侧有 / 出侧无 +
/// 出侧有 / 入侧无）——经 `finish_command` → store 收口直落、golden 投影面
///（`project_detail` 全量序列化）逐值直达零改写零 panic。该语料为专项
/// ad-hoc 构造（不入 CORPORA 集合——零 golden 产出，入仓 golden 重写流程
/// 归桌面端）；库读面半边同查（list_run_nodes 原值回读）。
#[test]
fn corpus防御值语料_overrides混防御值直达投影零改写零panic() {
    let corpus = Corpus::new("step-defensive");
    corpus.seed_record("id-step-defensive", "step-defensive");
    corpus.run_start("id-step-defensive", "run-1727000040000", t(40));
    corpus.run_finish(
        "id-step-defensive",
        "run-1727000040000",
        RunStatus::Completed,
        "All phases have passed evaluation. Ready for archiving.",
        t(49),
        &[
            // 防御值样本一：started_at 负值（收口同为负）+ token 入侧单值
            //（output None）——零钳制零补值
            step_metrics(
                "dev-design",
                1,
                0,
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-dd-exec-def"),
                None,
                None,
                -1_000,
                Some(-500),
                Some(111),
                None,
            ),
            // 防御值样本二：started_at = 0（缺席语义值）同刻瞬步 + token 出侧
            // 单值（input None）——零默认零改写
            step_metrics(
                "dev-design",
                1,
                0,
                ChangeStepKind::VerdictGate,
                ChangeStepStatus::Passed,
                None,
                None,
                None,
                0,
                Some(0),
                None,
                Some(222),
            ),
        ],
        &[],
    );

    // 库读面半边：防御值逐字段原值回读（零钳制零补值）
    let db_nodes = corpus
        .store
        .list_run_nodes("run-1727000040000")
        .expect("节点读应成功");
    assert_eq!(db_nodes.len(), 2, "防御值两步全量落库");
    let executor_row = db_nodes
        .iter()
        .find(|row| row.step == workflow::state::RunStepKind::Executor)
        .expect("executor 行在案");
    assert_eq!(
        executor_row.started_at, -1_000,
        "负值 started_at 原值落库（零钳制）"
    );
    assert_eq!(executor_row.ended_at, Some(-500), "负值 ended_at 原值落库");
    assert_eq!(executor_row.token_input, Some(111), "入侧单值落库");
    assert_eq!(
        executor_row.token_output, None,
        "出侧缺席 None 原样（零补值）"
    );

    // golden 投影面：project_detail 序列化防御值逐键逐值直达（零改写零回填）
    let projection = project_detail(&corpus, "id-step-defensive");
    let wire_nodes = projection["runs"][0]["nodes"]
        .as_array()
        .expect("nodes 数组");
    assert_eq!(wire_nodes.len(), 2, "投影两节点（emit 序）");
    let four_keys = ["startedAt", "endedAt", "tokenInput", "tokenOutput"];
    for node in wire_nodes {
        let object = node.as_object().expect("节点对象");
        for key in four_keys {
            assert!(object.contains_key(key), "防御值节点缺 {key} 键: {node}");
        }
    }
    let negative = wire_nodes
        .iter()
        .find(|node| node["step"] == "executor")
        .expect("executor 节点在案");
    assert_eq!(
        negative["startedAt"],
        serde_json::json!(-1_000_i64),
        "负值 startedAt 直达投影（零改写）"
    );
    assert_eq!(
        negative["endedAt"],
        serde_json::json!(-500_i64),
        "负值 endedAt 直达投影"
    );
    assert_eq!(
        negative["tokenInput"],
        serde_json::json!(111_u64),
        "入侧单值直达"
    );
    assert!(
        negative["tokenOutput"].is_null(),
        "出侧缺席 null（键在场零补值）"
    );

    let zero = wire_nodes
        .iter()
        .find(|node| node["step"] == "verdict_gate")
        .expect("verdict_gate 节点在案");
    assert_eq!(
        zero["startedAt"],
        serde_json::json!(0_i64),
        "0 值 startedAt 原样出线（零钳制）"
    );
    assert_eq!(
        zero["endedAt"],
        serde_json::json!(0_i64),
        "0 值 endedAt 同刻瞬步原样"
    );
    assert!(zero["tokenInput"].is_null(), "入侧缺席 null（键在场）");
    assert_eq!(
        zero["tokenOutput"],
        serde_json::json!(222_u64),
        "出侧单值直达"
    );

    // 零 panic：防御值载荷全量序列化成功（负值 / 0 / 单侧 token 原文在案）
    let full = serde_json::to_string(&projection).expect("防御值投影全量序列化应成功");
    assert!(
        full.contains("\"startedAt\":-1000"),
        "负值原文在案（零改写）"
    );
    assert!(full.contains("\"startedAt\":0"), "0 值原文在案（零改写）");
    assert!(full.contains("\"tokenInput\":111"), "入侧单值原文在案");
    assert!(full.contains("\"tokenOutput\":222"), "出侧单值原文在案");

    // 专项语料不入 CORPORA 集合——零 golden 产出（入仓 golden 集合不变）
    assert!(
        !golden_dir().join("corpus-step-defensive.json").exists(),
        "防御值专项语料零 golden 产出（语料完整性集合不变）"
    );
}

// ---------------------------------------------------------------------------
// initialize 语料（check-before-flow-start / AC-8 语料半边）：initialize 相位
// 样本（正常 pass 条目 + legacy skipped 条目两形态）与 clean 步词节点样本 →
// golden 全量对比随动
// ---------------------------------------------------------------------------

/// initialize 语料：pass 条目（机械 checklist 面）+ legacy skipped 条目（跳过因
/// 面两形态）+ initialize run 五节点（clean 词样本、串行 forward 边）。
fn build_initialize(tag: &str) -> Corpus {
    let corpus = Corpus::new(tag);
    corpus.seed_record("id-initialize", "initialize-corpus");

    // pass 条目（attempt 1——worktree 分支机械 checklist 落账形态）
    corpus.run_phase(
        "id-initialize",
        "initialize",
        Verdict::Pass,
        "conclusion=pass total=12 passed=10 failed=0 skipped=2；报告目录 /ws/openspec/changes/initialize-corpus/reports/baseline",
        vec![
            ChecklistItem {
                item: "静态检查通过".to_owned(),
                pass: true,
                evidence: "diagnostics 空".to_owned(),
            },
            ChecklistItem {
                item: "测试执行通过".to_owned(),
                pass: true,
                evidence: "四计数在场".to_owned(),
            },
            ChecklistItem {
                item: "突变测试".to_owned(),
                pass: false,
                evidence: "null（未跑）".to_owned(),
            },
        ],
        None,
        None,
        t(1),
    );

    // legacy skipped 条目（attempt 2——root == main_root 短路落账形态；skipped
    // 同 pass 一次性持久语义）
    corpus
        .store
        .start_change_phase("id-initialize", "initialize", t(2))
        .expect("开相失败");
    corpus
        .store
        .log_change_phase(&PhaseLogCommand {
            change_id: "id-initialize".to_owned(),
            phase: "initialize".to_owned(),
            verdict: Verdict::Pass,
            report: "legacy 主仓执行无 worktree 基线，跳过基线检查".to_owned(),
            skipped: true,
            checklist: Vec::new(),
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: Some(t(2)),
            timestamp: t(3),
        })
        .expect("落账失败");

    // initialize run 五节点整包（clean 词样本 + 串行 forward 边）
    let nodes = [
        step(
            "initialize",
            1,
            ChangeStepKind::PhaseStart,
            ChangeStepStatus::Passed,
            None,
            None,
        ),
        step(
            "initialize",
            1,
            ChangeStepKind::Clean,
            ChangeStepStatus::Passed,
            None,
            Some("已恢复基线（reset --hard + clean -fd，排除面 8 项）"),
        ),
        step(
            "initialize",
            1,
            ChangeStepKind::StaticCheck,
            ChangeStepStatus::Passed,
            None,
            Some("静态检查通过"),
        ),
        step(
            "initialize",
            1,
            ChangeStepKind::TestExecution,
            ChangeStepStatus::Passed,
            None,
            Some("conclusion=pass total=12 passed=10 failed=0 skipped=2"),
        ),
        step(
            "initialize",
            1,
            ChangeStepKind::PhaseLog,
            ChangeStepStatus::Passed,
            None,
            None,
        ),
    ];
    let edges: Vec<ChangeStepEdge> = nodes
        .windows(2)
        .map(|pair| ChangeStepEdge {
            from: (
                pair[0].phase.clone(),
                pair[0].attempt,
                pair[0].step,
                pair[0].instance_no,
            ),
            to: (
                pair[1].phase.clone(),
                pair[1].attempt,
                pair[1].step,
                pair[1].instance_no,
            ),
            kind: EdgeKind::Forward,
        })
        .collect();
    corpus.run_start("id-initialize", "run-1727000010000", t(10));
    corpus.run_finish(
        "id-initialize",
        "run-1727000010000",
        RunStatus::Completed,
        "initialize 全绿后冻结样本（后续路由由续走语料承载）",
        t(19),
        &nodes,
        &edges,
    );
    corpus
}

/// initialize 语料 golden 对拍：pass + skipped 两形态条目出线（skipped 读侧
/// 透出——呈现面零结构改动前提）、clean 步词节点样本在案（二十一值词汇随动、
/// 串行 forward 边序逐位）。
#[test]
fn corpusinitialize语料_golden对拍() {
    let corpus = build_initialize("initialize-golden");
    let projection = project_detail(&corpus, "id-initialize");

    // pipeline：10 站、initialize 站两 attempts（pass + skipped）
    let pipeline = projection["pipeline"].as_array().expect("pipeline 为数组");
    assert_eq!(pipeline.len(), 10, "requirement 十站（initialize 表头）");
    assert_eq!(pipeline[0]["phase"], "initialize", "表头第 1 列");
    let attempts = pipeline[0]["attempts"].as_array().expect("attempts 数组");
    assert_eq!(attempts.len(), 2, "pass + skipped 两形态条目");
    assert_eq!(attempts[0]["skipped"], false, "attempt 1 pass 形态");
    assert_eq!(
        attempts[0]["checklist"].as_array().map(Vec::len),
        Some(3),
        "机械 checklist 三条目"
    );
    assert!(
        attempts[0]["report"]
            .as_str()
            .expect("report")
            .contains("reports/baseline"),
        "report 携 baseline 报告目录"
    );
    assert_eq!(
        attempts[1]["skipped"], true,
        "attempt 2 legacy skipped 形态"
    );
    assert!(
        attempts[1]["report"]
            .as_str()
            .expect("report")
            .contains("legacy"),
        "跳过因落账可见"
    );

    // runs：clean 词节点样本在案 + 串行 forward 边序逐位
    let runs = projection["runs"].as_array().expect("runs 为数组");
    assert_eq!(runs.len(), 1);
    let nodes = runs[0]["nodes"].as_array().expect("nodes 数组");
    let words: Vec<&str> = nodes
        .iter()
        .map(|node| node["step"].as_str().expect("step 出线词"))
        .collect();
    assert_eq!(
        words,
        [
            "phase_start",
            "clean",
            "static_check",
            "test_execution",
            "phase_log"
        ],
        "initialize 五节点词面逐位（clean 新词样本在案）"
    );
    let edges = runs[0]["edges"].as_array().expect("edges 数组");
    assert_eq!(edges.len(), 4, "串行 forward 边四条");
    assert_eq!(edges[0]["fromNodeKey"], "initialize:1:phase_start:0");
    assert_eq!(edges[3]["toNodeKey"], "initialize:1:phase_log:0");

    check_or_rewrite("corpus-initialize", &projection);
}
