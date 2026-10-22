use std::sync::Mutex;

use super::phase_next::{phase_next, PhaseNextError, SessionAnchors};
use super::phase_table::{BacktrackOption, MAX_RETRY_TIMES};
use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseStateRecord, RunEdgeStateRecord,
    RunFinishCommand, RunNodeStateRecord, RunStartCommand, RunStateRecord, StepCommand,
    StepStateRecord, StoreFault,
};

/// 身份锚字面量（路由入参——一切寻址以 id 为准；prompt 插值取 name）。
const CHANGE_ID: &str = "0198f7a0-0000-7000-8000-0000000000e6";
/// change 名（上下文头 `change:` 段供给值——裸名，非身份键）。
const CHANGE_NAME: &str = "demo-change";
/// 第二 change（锚点键 id 隔离用例：同 run_id 异 id 两键）。
const CHANGE_ID_B: &str = "0198f7a0-0000-7000-8000-0000000000e7";
const CHANGE_NAME_B: &str = "other-change";
/// 库内不存在的 id（未建档拒绝面）。
const UNKNOWN_ID: &str = "0198f7a0-0000-7000-8000-0000000000fa";
const RUN: &str = "run-1";

/// 白名单 id 投影（断言面：既有用例只看相位 id；描述随行由 `phase_table_test`
/// 与决策 prompt 用例覆盖）。
fn ids(options: &[BacktrackOption]) -> Vec<&str> {
    options.iter().map(|option| option.phase.as_str()).collect()
}

/// requirement 全表相位序（10 相位 = initialize 表头 + 8 路由相位 + archive
/// 确定性相位；路由全 pass 判定须表内全项在位）。
const ALL_PHASES: [&str; 10] = [
    "initialize",
    "proposal",
    "dev-design",
    "test-design",
    "implement",
    "test-gen",
    "test-execution",
    "code-review",
    "acceptance",
    "archive",
];

/// 归档相位之前的 9 路由相位（archive 前停等路由用例的种子相位集——initialize
/// 表头先行入集）。
const ROUTED_PHASES: [&str; 9] = [
    "initialize",
    "proposal",
    "dev-design",
    "test-design",
    "implement",
    "test-gen",
    "test-execution",
    "code-review",
    "acceptance",
];

/// 确定性时间戳基（UTC unix millis）；`t(n)` = 基线 + n 秒。
const T0: i64 = 1_727_000_000_000;
fn t(n: u32) -> i64 {
    T0 + i64::from(n) * 1000
}

/// 内存构造一条相位评估条目（假件 / 断言共用的中性快照工厂）。
fn entry(phase: &str, attempt: u32, verdict: Verdict, ts: i64) -> PhaseStateRecord {
    entry_of(CHANGE_ID, phase, attempt, verdict, ts)
}

/// 指定 change id 归属的评估条目（锚点键隔离用例的第二 change 条目面）。
fn entry_of(
    change_id: &str,
    phase: &str,
    attempt: u32,
    verdict: Verdict,
    ts: i64,
) -> PhaseStateRecord {
    PhaseStateRecord {
        id: 0,
        change_id: change_id.to_owned(),
        phase: phase.to_owned(),
        attempt,
        verdict,
        report: format!("{phase} 报告"),
        checklist: Vec::new(),
        skipped: false,
        stale: false,
        backtrack_to: None,
        backtrack_reason: None,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
        start_at: None,
        timestamp: ts,
    }
}

/// 携 backtrack 回跳标记的条目（backtrack 路由用例的种子形态）。
fn backtrack_entry(phase: &str, attempt: u32, ts: i64, to: &str, reason: &str) -> PhaseStateRecord {
    let mut record = entry(phase, attempt, Verdict::Pass, ts);
    record.backtrack_to = Some(to.to_owned());
    record.backtrack_reason = Some(reason.to_owned());
    record
}

/// requirement 建档中性快照（active、无 active_phase 残留；id 为身份锚、
/// name 为展示 / prompt 插值供给值——两字面量互异以可辨寻址面）。
fn requirement_record(id: &str, name: &str) -> ChangeStateRecord {
    ChangeStateRecord {
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
    }
}

// ---------------------------------------------------------------------------
// 假件 store：只实现读半边（get_change / list_phase_records），写半边一律
// unimplemented（越权触写即 panic——只读性的结构性执法）；可注入 StoreFault。
// ---------------------------------------------------------------------------

struct RouteStore {
    record: Mutex<Option<ChangeStateRecord>>,
    entries: Mutex<Vec<PhaseStateRecord>>,
    fault: Mutex<Option<StoreFault>>,
}

impl RouteStore {
    /// 种子装置统一预置 initialize 表头 pass 条目（check-before-flow-start：表头
    /// 先行已落账——各用例种子语义「已过表头」与既有行为等价；表头自身的路由
    /// 形态由 `初始路由落首相位` 专测承载零条目形态）。
    fn with_header(id: &str, entries: Vec<PhaseStateRecord>) -> Vec<PhaseStateRecord> {
        let mut all = vec![entry_of(id, "initialize", 1, Verdict::Pass, t(0))];
        all.extend(entries);
        all
    }

    fn seeded(entries: Vec<PhaseStateRecord>) -> Self {
        Self::with_record(
            Some(requirement_record(CHANGE_ID, CHANGE_NAME)),
            Self::with_header(CHANGE_ID, entries),
        )
    }

    /// 第二 change 建档（锚点键 id 隔离用例：同 run_id 异 id 两份条目序列）。
    fn seeded_b(entries: Vec<PhaseStateRecord>) -> Self {
        Self::with_record(
            Some(requirement_record(CHANGE_ID_B, CHANGE_NAME_B)),
            Self::with_header(CHANGE_ID_B, entries),
        )
    }

    fn missing() -> Self {
        Self::with_record(None, Vec::new())
    }

    fn with_workflow_type(workflow_type: &str) -> Self {
        let mut record = requirement_record(CHANGE_ID, CHANGE_NAME);
        record.workflow_type = workflow_type.to_owned();
        Self::with_record(Some(record), Vec::new())
    }

    fn with_record(record: Option<ChangeStateRecord>, entries: Vec<PhaseStateRecord>) -> Self {
        Self {
            record: Mutex::new(record),
            entries: Mutex::new(entries),
            fault: Mutex::new(None),
        }
    }

    /// run 期间落账驱动窗口演进（模拟真实 phase-log 节奏）。
    fn push(&self, record: PhaseStateRecord) {
        self.entries.lock().expect("条目锁不可中毒").push(record);
    }

    fn entry_count(&self) -> usize {
        self.entries.lock().expect("条目锁不可中毒").len()
    }

    fn set_fault(&self, fault: StoreFault) {
        *self.fault.lock().expect("故障锁不可中毒") = Some(fault);
    }
}

impl ChangeStateStore for RouteStore {
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        Ok(self
            .record
            .lock()
            .expect("记录锁不可中毒")
            .clone()
            .filter(|record| record.id == id))
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(&self, _change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        Ok(self.entries.lock().expect("条目锁不可中毒").clone())
    }

    fn list_steps(
        &self,
        _change_id: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn create_change_record(&self, _record: ChangeStateRecord) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn delete_change_record(&self, _id: &str) -> Result<bool, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn start_phase(
        &self,
        _change_id: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<crate::state::PhaseStartState, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn log_phase(&self, _command: &crate::state::PhaseLogCommand) -> Result<u32, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn apply_backtrack(&self, _command: &crate::state::BacktrackCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn amend_decision_session(
        &self,
        _change_id: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn set_archived(&self, _id: &str, _archived_at: i64) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn append_step(&self, _command: &StepCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_runs(&self, _change: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_run_nodes(&self, _run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_run_edges(&self, _run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_start(&self, _command: &RunStartCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_finish(&self, _command: &RunFinishCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }
}

/// 一次路由调用（假件 + 独立锚点实例；id 形参 = 主 change）。
fn route(
    fake: &RouteStore,
    run_id: &str,
    anchors: &SessionAnchors,
) -> super::phase_next::PhaseNextOutcome {
    phase_next(fake, CHANGE_ID, run_id, anchors).expect("phase_next 应成功")
}

// ---------------------------------------------------------------------------
// 正向：初始路由 / pass 推进 / fail 预算内重试 / 重试上限 / 全 pass 终态 /
// backtrack 路由
// ---------------------------------------------------------------------------

/// 初始路由：建档零相位行（id 与 name 双值可辨）→ 表头相位 initialize、
/// round=1、零角色 spec（确定性表头）、白名单为空（idx==0 早退——表首自身无
/// 前置）；表头 pass 条目在场 → 次相位 proposal、executor / evaluator prompt
/// 首部上下文头 `change:` 取 `record.name`（id 串不误入 prompt——分辨率单点
/// 行为锚）。
#[test]
fn 初始路由落首相位且prompt携上下文头change名() {
    // 零条目形态：路由落 initialize 表头
    let record = requirement_record(CHANGE_ID, CHANGE_NAME);
    let fake = RouteStore::with_record(Some(record), Vec::new());

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert!(!outcome.done);
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("initialize"),
        "表头相位路由（零条目——check-before-flow-start）"
    );
    assert_eq!(outcome.round, 1, "零相位行窗口首轮");
    assert!(outcome.error.is_none());
    assert!(
        outcome.executor.is_none() && outcome.evaluator.is_none(),
        "initialize 表头零角色 spec（确定性相位）"
    );
    assert!(
        outcome.allowed_backtrack_phases.is_empty(),
        "表头白名单为空（idx==0 早退）"
    );

    // 表头条目在场（seeded 预置）→ 次相位 proposal 路由（prompt 断言面）
    let fake = RouteStore::seeded(Vec::new());
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("proposal"),
        "表头已过 → 次相位路由"
    );

    let executor = outcome.executor.expect("proposal 应有 executor");
    let evaluator = outcome.evaluator.expect("proposal 应有 evaluator");
    let header = format!("change: {CHANGE_NAME}\n\n");
    assert!(
        executor.prompt.starts_with(&header),
        "executor prompt 首部上下文头取 record.name: {}",
        executor.prompt
    );
    assert!(
        !executor.prompt.contains(CHANGE_ID),
        "id 串不误入 executor prompt（上下文头单点 = record.name）: {}",
        executor.prompt
    );
    assert!(
        evaluator.prompt.starts_with(&header),
        "evaluator prompt 首部上下文头取 record.name"
    );
    assert!(
        !evaluator.prompt.contains(CHANGE_ID),
        "id 串不误入 evaluator prompt"
    );
    for (role, prompt) in [
        ("executor", &executor.prompt),
        ("evaluator", &evaluator.prompt),
    ] {
        assert!(
            !prompt.contains("<change>") && !prompt.contains("<phase>"),
            "{role} prompt 零模板占位残留（上下文头取代 <change> / <phase> 插值）: {prompt}"
        );
    }
    assert!(
        executor.prompt.contains("## 角色自述"),
        "executor prompt 主体为静态角色知识文本"
    );
    assert_eq!(
        ids(&outcome.allowed_backtrack_phases),
        vec!["proposal"],
        "proposal 白名单 = 恰自身（含自身语义；initialize 表头被 backtrackable 过滤排除——非位置排除）"
    );
    let last = outcome.last_result.expect("表头 pass 条目随行 last_result");
    assert_eq!(last.phase, "initialize", "last_result = 表头条目（最近落账）");
    assert_eq!(last.verdict, Verdict::Pass);
}

/// 无建档 id → 显式 `Err`（不静默空产出）：库空与库内有建档但 id 未登记
/// 两态皆拒（get_change miss 面——name 不作寻址回退）。
#[test]
fn 未建档id显式err不静默空产出() {
    let fake = RouteStore::missing();
    let err = phase_next(&fake, CHANGE_ID, RUN, &SessionAnchors::new()).expect_err("库空应 Err");
    assert!(err.contains("未建档"), "Err 显式记因建档缺失，实际: {err}");
    assert!(err.contains(CHANGE_ID), "拒绝面携 id 语境，实际: {err}");

    let fake = RouteStore::seeded(Vec::new());
    let err =
        phase_next(&fake, UNKNOWN_ID, RUN, &SessionAnchors::new()).expect_err("未登记 id 应 Err");
    assert!(
        err.contains("未建档") && err.contains(UNKNOWN_ID),
        "Err 记因携未登记 id 语境（不回退 name 寻址），实际: {err}"
    );
    assert!(
        !err.contains(CHANGE_NAME),
        "拒绝面零 name 感知（未解析到记录），实际: {err}"
    );
}

/// workflow_type 未知 → Err 显式分层出口（W8 写面侧）：文案通吃
/// `{requirement, test-only}` 两值、不再出现「V1 仅 requirement」旧字样。
#[test]
fn workflow_type未知时err显式分层出口文案通吃两值() {
    let long_unknown = "x".repeat(1200);
    for workflow_type in [
        "bug-fix",
        "refactor",
        "",
        " ",
        "test_only",
        long_unknown.as_str(),
        "test-only🚀",
        "require\nment",
    ] {
        let fake = RouteStore::with_workflow_type(workflow_type);

        let err = phase_next(&fake, CHANGE_ID, RUN, &SessionAnchors::new())
            .expect_err("未知 workflow_type 应 Err");

        assert!(
            err.contains("workflow_type"),
            "Err 记因携 workflow_type 字段名，实际: {err}"
        );
        assert!(
            err.contains("requirement") && err.contains("test-only"),
            "文案通吃合法值集 {{requirement, test-only}}，实际: {err}"
        );
        assert!(
            !err.contains("仅 requirement"),
            "旧「V1 仅 requirement」字样退役，实际: {err}"
        );
        if !workflow_type.trim().is_empty() {
            assert!(
                err.contains(workflow_type),
                "文案携实际类型值 {workflow_type:?}，实际: {err}"
            );
        }
    }
}

/// test-only 类型分派可走通（正向 / 新增）：`workflow_type="test-only"` 的
/// change 按 `TEST_ONLY_TABLE` 序推进——不因类型被拒（proposal pass →
/// next_phase = code-analyze，白名单 = test-only 表序前置集）。
#[test]
fn workflow_type_test_only可推进且白名单随test_only表() {
    let fake = RouteStore::with_workflow_type("test-only");
    fake.push(entry("initialize", 1, Verdict::Pass, t(0)));
    fake.push(entry("proposal", 1, Verdict::Pass, t(1)));

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("code-analyze"),
        "test-only 表序下一相位为 code-analyze"
    );
    assert_eq!(
        ids(&outcome.allowed_backtrack_phases),
        vec!["proposal", "code-analyze"],
        "白名单 = test-only 新相位的表序前置集"
    );
    let last = outcome.last_result.expect("有相位史应携 last_result");
    assert_eq!(last.phase, "proposal");
    assert_eq!(last.verdict, Verdict::Pass);

    // 末相位（archive）推进 → 无下一相位（内容面收口）
    let fake_done = RouteStore::with_workflow_type("test-only");
    fake_done.push(entry("initialize", 1, Verdict::Pass, t(0)));
    for (phase, attempt) in [
        ("proposal", 1),
        ("code-analyze", 1),
        ("test-design", 1),
        ("test-gen", 1),
        ("test-execution", 1),
    ] {
        fake_done.push(entry(phase, attempt, Verdict::Pass, t(attempt as u32)));
    }
    let done = route(&fake_done, RUN, &SessionAnchors::new());
    assert_eq!(
        done.next_phase.as_deref(),
        Some("archive"),
        "五相位 pass 后下一相位 = archive"
    );
    assert!(!done.done, "未全 pass 前不落终态");
    fake_done.push(entry("archive", 1, Verdict::Pass, t(9)));
    let finished = route(&fake_done, RUN, &SessionAnchors::new());
    assert!(finished.done, "test-only 全表 pass 后 done");
    assert_eq!(
        finished.next_phase, None,
        "test-only 全表 pass 后无下一相位（零 requirement 相位越表）"
    );
    assert!(
        finished.allowed_backtrack_phases.is_empty(),
        "终态白名单空集"
    );
}

/// test-only 表外条目（异常 / 新增）：requirement 独有相位（implement）条目
/// 不参与 test-only 推进判定（表外条目零计数——`all_passed` 只认本表相位）。
#[test]
fn workflow_type_test_only表外条目不参与推进判定() {
    let fake = RouteStore::with_workflow_type("test-only");
    fake.push(entry("initialize", 1, Verdict::Pass, t(0)));
    for (phase, attempt) in [
        ("proposal", 1),
        ("code-analyze", 1),
        ("test-design", 1),
        ("test-gen", 1),
        ("test-execution", 1),
        // 表外条目（requirement 独有相位）不得使 test-only 表「全 pass」
        ("implement", 1),
        ("dev-design", 1),
    ] {
        fake.push(entry(phase, attempt, Verdict::Pass, t(attempt as u32)));
    }

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert!(
        !outcome.done,
        "表外条目零计数：test-only 六相位表缺 archive → 非终态"
    );
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("archive"),
        "下一相位仍为表内未过相位 archive（表外条目被忽略）"
    );
    assert_eq!(
        ids(&outcome.allowed_backtrack_phases),
        vec![
            "proposal",
            "code-analyze",
            "test-design",
            "test-gen",
            "test-execution",
            "archive",
        ],
        "白名单 = test-only 表序前缀集（不含表外相位）"
    );
}

/// pass 推进：当前相位最新条目 pass → next_phase=下一相位、round 归位、
/// 白名单 = 新相位的表序前置集。
#[test]
fn pass推进到下一相位且白名单随行() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert_eq!(outcome.next_phase.as_deref(), Some("dev-design"));
    assert_eq!(
        outcome.round, 1,
        "round 归位（锚点 = 首见 PhaseRecord 行数）"
    );
    assert_eq!(
        ids(&outcome.allowed_backtrack_phases),
        vec!["proposal", "dev-design"],
        "白名单 = 新相位的表序前置集"
    );
    assert_eq!(
        outcome.allowed_backtrack_phases[1].description, "详细设计与任务拆解",
        "白名单项携相位特性描述（自相位表 description 下发）"
    );
    let last = outcome.last_result.expect("有相位史应携 last_result");
    assert_eq!(last.phase, "proposal");
    assert_eq!(last.verdict, Verdict::Pass);
}

/// 已决 fail 放行同相位重跑（带 executor 相位）：决策已消费该 fail 条目后
///（`decision_session_id` 在位）→ 同相位重入、round 递增、失败反馈段进 executor
/// prompt（决策 backtrack 兑现的重做面）；再落的已决 fail 同式递增。
#[test]
fn 已决fail放行同相位重跑且round递增() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();

    let first = route(&fake, RUN, &anchors);
    assert_eq!(first.next_phase.as_deref(), Some("dev-design"));
    assert_eq!(first.round, 1);

    // run 期间逐条落账两条已决 fail（决策 backtrack 各兑现一次重跑）
    let mut decided = entry("dev-design", 1, Verdict::Fail, t(2));
    decided.decision_session_id = Some("sess-decision-1".to_owned());
    fake.push(decided);
    let second = route(&fake, RUN, &anchors);
    assert_eq!(
        second.next_phase.as_deref(),
        Some("dev-design"),
        "已决 fail 放行同相位重跑"
    );
    assert_eq!(second.round, 2, "窗口条目数 + 1");
    assert!(second.error.is_none(), "已决条目不触发决策分叉");
    let second_prompt = second
        .executor
        .as_ref()
        .expect("executor 在场")
        .prompt
        .clone();
    assert!(
        second_prompt.starts_with(&format!("change: {CHANGE_NAME}\n\n"))
            && !second_prompt.contains(CHANGE_ID),
        "executor prompt 上下文头取 record.name（id 串零入 prompt）: {second_prompt}"
    );
    assert!(
        !second_prompt.contains("<change>") && !second_prompt.contains("<phase>"),
        "重跑 prompt 零模板占位残留（相位 id 不经模板注入）: {second_prompt}"
    );
    assert!(
        second_prompt.contains("⚠️ 上次评估未通过（attempt 1）")
            && second_prompt.contains("dev-design 报告"),
        "重跑 prompt 携失败反馈段（清单空退回 report 全文）: {second_prompt}"
    );
    assert!(
        second
            .evaluator
            .as_ref()
            .expect("evaluator 在场")
            .prompt
            .starts_with(&format!("change: {CHANGE_NAME}\n\n")),
        "evaluator prompt 上下文头同相位在场"
    );
    assert!(
        second
            .evaluator
            .as_ref()
            .expect("evaluator 在场")
            .prompt
            .contains("上次评估未通过"),
        "evaluator prompt 同携失败反馈段（下轮会话双槽位随行）"
    );

    let mut decided = entry("dev-design", 2, Verdict::Fail, t(3));
    decided.decision_session_id = Some("sess-decision-2".to_owned());
    fake.push(decided);
    let third = route(&fake, RUN, &anchors);
    assert_eq!(third.round, 3, "round 随窗口条目递增");
    let last = third.last_result.expect("last_result 在场");
    assert_eq!(last.phase, "dev-design");
    assert_eq!(last.verdict, Verdict::Fail, "last_result 携最新 fail 条目");
}

/// 失败反馈段内容面：已决 fail 放行重跑时随 prompt 下发失败项（item +
/// evidence），pass 项不随行；首轮（无失败条目）零注入。
#[test]
fn 失败项随已决fail重跑prompt注入且首轮零注入() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();

    let first = route(&fake, RUN, &anchors);
    assert!(
        !first
            .executor
            .as_ref()
            .expect("executor 在场")
            .prompt
            .contains("上次评估未通过"),
        "首轮无失败条目零注入"
    );

    // 已决 fail（决策 backtrack 兑现的重跑面）：decision_session_id 在位故放行
    // 正常路由——失败反馈段随 executor prompt 下发
    let mut failed = entry("dev-design", 1, Verdict::Fail, t(2));
    failed.decision_session_id = Some("sess-decision-1".to_owned());
    failed.checklist = vec![
        ChecklistItem {
            item: "设计完整性".to_owned(),
            pass: false,
            evidence: "缺产物区组件（design.md:12）".to_owned(),
        },
        ChecklistItem {
            item: "任务分解".to_owned(),
            pass: true,
            evidence: "tasks 齐备".to_owned(),
        },
    ];
    fake.push(failed);

    let second = route(&fake, RUN, &anchors);
    let executor = second.executor.expect("executor 在场");
    assert!(
        executor.prompt.contains("⚠️ 上次评估未通过（attempt 1）")
            && executor
                .prompt
                .contains("- 设计完整性：缺产物区组件（design.md:12）"),
        "失败项（item + evidence）随 prompt 注入: {}",
        executor.prompt
    );
    assert!(
        !executor.prompt.contains("- 任务分解"),
        "pass 项不随行（只携失败项）: {}",
        executor.prompt
    );

    // backtrack 路由目标相位携历史 fail 条目：反馈段仍零注入（回溯原因行独立，
    // 判别 None 直传——若误走正常路由过滤，该目标相位最新条目为 Fail 必注入）
    fake.push(backtrack_entry(
        "test-design",
        1,
        t(3),
        "dev-design",
        "设计返工：缺产物区组件",
    ));
    let third = route(&fake, RUN, &anchors);
    assert_eq!(third.next_phase.as_deref(), Some("dev-design"));
    let third_prompt = third.executor.expect("executor 在场").prompt;
    assert!(
        third_prompt.contains("⚠️ 回溯原因: 设计返工：缺产物区组件")
            && !third_prompt.contains("上次评估未通过"),
        "backtrack 路由零失败反馈注入（回溯原因行独立）: {third_prompt}"
    );
}

#[test]
fn 重试上限恰达返回max_retries_exceeded() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors); // 首见登记基线 1

    for attempt in 1..=MAX_RETRY_TIMES {
        fake.push(entry("dev-design", attempt, Verdict::Fail, t(attempt + 1)));
        let outcome = route(&fake, RUN, &anchors);
        if attempt < MAX_RETRY_TIMES {
            assert_eq!(
                outcome.error,
                Some(PhaseNextError::VerdictFail {
                    phase: "dev-design".to_owned(),
                    round: attempt + 1,
                }),
                "第 {attempt} 条未决 fail 走 VerdictFail（决策分叉同语义）"
            );
        } else {
            assert_eq!(
                outcome.error,
                Some(PhaseNextError::MaxRetriesExceeded {
                    phase: "dev-design".to_owned(),
                    round: u32::from(MAX_RETRY_TIMES) + 1,
                }),
                "第 {MAX_RETRY_TIMES} 条 fail 恰达上限（标注变体）"
            );
            assert_eq!(outcome.next_phase, None, "上限响应不下发相位");
            assert!(!outcome.done, "上限非终态（决策分叉待定）");
            assert!(outcome.executor.is_none() && outcome.evaluator.is_none());
        }
    }
}

#[test]
fn verdict_fail立即决策分叉不自走重试() {
    // 带 executor 相位：proposal pass 后 dev-design 为表序首个未过相位
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);
    fake.push(entry("dev-design", 1, Verdict::Fail, t(2)));
    let outcome = route(&fake, RUN, &anchors);
    assert_eq!(
        outcome.error,
        Some(PhaseNextError::VerdictFail {
            phase: "dev-design".to_owned(),
            round: 2,
        }),
        "带 executor 相位第 1 条 fail 即决策分叉（任意带 evaluator 相位同式）"
    );
    assert_eq!(outcome.next_phase, None, "分叉响应不下发相位");
    assert!(!outcome.done, "分叉非终态（决策待定）");
    assert!(
        outcome.executor.is_none() && outcome.evaluator.is_none(),
        "分叉响应零角色 spec"
    );
    assert!(
        outcome.steps.is_empty() && outcome.artifact_manifest.is_empty(),
        "分叉响应零计划步 / 清单"
    );
    assert!(
        outcome.last_result.is_some(),
        "last_result 快照随行（决策输入面）"
    );

    // 评估-only 相位（code-review）：前 6 相位 pass → code-review#1 fail →
    // 立即分叉（评估-only 与双角色相位同一分叉路径）
    let base: Vec<PhaseStateRecord> = [
        ("proposal", 1u32),
        ("dev-design", 1),
        ("test-design", 1),
        ("implement", 1),
        ("test-gen", 1),
        ("test-execution", 1),
    ]
    .iter()
    .enumerate()
    .map(|(idx, (phase, attempt))| entry(phase, *attempt, Verdict::Pass, t(idx as u32 + 1)))
    .collect();
    let fake = RouteStore::seeded(base.clone());
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);
    fake.push(entry("code-review", 1, Verdict::Fail, t(7)));
    let outcome = route(&fake, RUN, &anchors);
    assert_eq!(
        outcome.error,
        Some(PhaseNextError::VerdictFail {
            phase: "code-review".to_owned(),
            round: 2,
        }),
        "评估-only 相位同式立即分叉"
    );

    // acceptance（同为评估-only）同式
    let fake = RouteStore::seeded({
        let mut seeded = base;
        seeded.push(entry("code-review", 1, Verdict::Pass, t(7)));
        seeded
    });
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);
    fake.push(entry("acceptance", 1, Verdict::Fail, t(8)));
    let outcome = route(&fake, RUN, &anchors);
    assert_eq!(
        outcome.error,
        Some(PhaseNextError::VerdictFail {
            phase: "acceptance".to_owned(),
            round: 2,
        }),
        "acceptance 同式立即分叉"
    );
}

/// 已决 fail 放行恰一次重跑：决策已消费该 fail 条目后（决策会话挂账最新 fail
/// 条目 decision_session_id），同一条目不再重触发分叉——正常路由回同相位
///（评估-only 相位 evaluator 携失败反馈段重评）。
#[test]
fn 评估only相位已决fail放行恰一次重跑() {
    let base: Vec<PhaseStateRecord> = [
        ("proposal", 1u32),
        ("dev-design", 1),
        ("test-design", 1),
        ("implement", 1),
        ("test-gen", 1),
        ("test-execution", 1),
    ]
    .iter()
    .enumerate()
    .map(|(idx, (phase, attempt))| entry(phase, *attempt, Verdict::Pass, t(idx as u32 + 1)))
    .collect();
    let fake = RouteStore::seeded(base);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);

    // 决策 retry 已挂账的 fail 条目（decision_session_id 在位）
    let mut decided = entry("code-review", 1, Verdict::Fail, t(7));
    decided.decision_session_id = Some("sess-decision-1".to_owned());
    fake.push(decided);
    let outcome = route(&fake, RUN, &anchors);
    assert!(
        outcome.error.is_none(),
        "已决条目不再重触发决策分叉（决策已消费）"
    );
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("code-review"),
        "放行同相位重跑（恰一次）"
    );
    assert!(
        outcome
            .evaluator
            .as_ref()
            .expect("评估-only 相位 evaluator 在场")
            .prompt
            .contains("上次评估未通过"),
        "重跑 evaluator 携失败反馈段"
    );

    // 对照：新落账的未决 fail 条目（决策未消费）→ 分叉照常触发
    fake.push(entry("code-review", 2, Verdict::Fail, t(8)));
    let outcome = route(&fake, RUN, &anchors);
    assert_eq!(
        outcome.error,
        Some(PhaseNextError::VerdictFail {
            phase: "code-review".to_owned(),
            round: 3,
        }),
        "新 fail 条目（无决策标记）分叉照常生效"
    );
}

/// 重试上限已决 fail 放行重跑（决策不空转的回归锚）：窗口 fail 数达上限但最新
/// fail 条目已挂决策会话 → 不再返回 MaxRetriesExceeded，正常路由回同相位——
/// 否则决策 backtrack 重入会陷入 phase-next ↔ 决策无限空转（相位永不重执行、
/// 决策会话无限唤起）。
#[test]
fn 重试上限已决fail放行重跑不空转() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);

    for attempt in 1..MAX_RETRY_TIMES {
        fake.push(entry("dev-design", attempt, Verdict::Fail, t(attempt + 1)));
    }
    let mut decided = entry("dev-design", MAX_RETRY_TIMES, Verdict::Fail, t(6));
    decided.decision_session_id = Some("sess-decision-1".to_owned());
    fake.push(decided);

    let outcome = route(&fake, RUN, &anchors);
    assert!(
        outcome.error.is_none(),
        "上限已决条目不再重触发分叉（fail_decided 放行一次重跑）"
    );
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("dev-design"),
        "正常路由回同相位重跑"
    );
    assert!(
        outcome
            .executor
            .as_ref()
            .expect("带 executor 相位 spec 在场")
            .prompt
            .contains("上次评估未通过"),
        "重跑 executor 携失败反馈段（带 executor 相位的「重试修复」面）"
    );
}

/// 全部 pass 终态：表内全相位（含 archive）pass → done=true、next_phase=None、
/// 双 prompt 缺席、白名单空（walker 收敛 completed 的判定输入；archive 以空
/// checklist 落 pass 后 all_passed 才成立，AC-4 / AC-7）。
#[test]
fn 全部pass终态done收敛() {
    let entries = ALL_PHASES
        .iter()
        .enumerate()
        .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
        .collect();
    let fake = RouteStore::seeded(entries);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert!(outcome.done, "全相位 pass → done");
    assert_eq!(outcome.next_phase, None);
    assert!(outcome.executor.is_none() && outcome.evaluator.is_none());
    assert!(outcome.error.is_none());
    assert!(outcome.allowed_backtrack_phases.is_empty());
}

/// backtrack 路由：最新条目 backtrack_to 在场 → 路由至目标相位重开，回溯
/// 原因后缀随 prompt、白名单 = 目标的表序前置集。
#[test]
fn backtrack字段落库路由至目标相位() {
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Pass, t(2)),
        backtrack_entry(
            "test-design",
            1,
            t(3),
            "dev-design",
            "设计返工：缺产物区组件",
        ),
    ]);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("dev-design"),
        "backtrack_to 在场 → 路由至目标相位"
    );
    let executor = outcome.executor.expect("目标相位应有 executor");
    assert!(
        executor
            .prompt
            .contains("⚠️ 回溯原因: 设计返工：缺产物区组件"),
        "回溯原因后缀随 prompt 下发: {}",
        executor.prompt
    );
    assert!(
        executor
            .prompt
            .starts_with(&format!("change: {CHANGE_NAME}\n\n")),
        "backtrack prompt 上下文头（change: <name>）与回溯原因后缀并存: {}",
        executor.prompt
    );
    assert!(
        !executor.prompt.contains("上次评估未通过"),
        "backtrack 路由零失败反馈注入（回溯原因行独立）: {}",
        executor.prompt
    );
    assert_eq!(
        ids(&outcome.allowed_backtrack_phases),
        vec!["proposal", "dev-design"],
        "白名单 = 目标相位的表序前置集"
    );
}

/// backtrack 路由 · C3 同位叠加终态：store 落库后发起相位标记行常与 stale
/// 翻转同行并存（发起相位在自身下游闭包内为常态），路由检测只认 backtrack_to
/// 在场——stale 位不遮蔽回跳路由，目标照常重开。
#[test]
fn backtrack标记与stale同行并存仍路由至目标相位() {
    let mut origin = backtrack_entry("test-gen", 1, t(5), "dev-design", "设计返工：缺产物区组件");
    origin.stale = true;
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Pass, t(2)),
        entry("test-design", 1, Verdict::Pass, t(3)),
        entry("implement", 1, Verdict::Pass, t(4)),
        origin,
    ]);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("dev-design"),
        "标记行 stale=true 不遮蔽 backtrack 路由（同位叠加终态照常重开目标）"
    );
    let executor = outcome.executor.expect("目标相位应有 executor");
    assert!(
        executor
            .prompt
            .contains("⚠️ 回溯原因: 设计返工：缺产物区组件"),
        "回溯原因后缀随 prompt 下发: {}",
        executor.prompt
    );
    assert!(
        executor
            .prompt
            .starts_with(&format!("change: {CHANGE_NAME}\n\n")),
        "backtrack prompt 上下文头（change: <name>）与回溯原因后缀并存: {}",
        executor.prompt
    );
    assert_eq!(
        ids(&outcome.allowed_backtrack_phases),
        vec!["proposal", "dev-design"],
        "白名单 = 目标相位的表序前置集"
    );
    assert_eq!(outcome.round, 1, "新锚点基线 = 全量行数 5，round 自 1 起");
}

// ---------------------------------------------------------------------------
// 边界：会话锚点（基线自 PhaseRecord 行数平移——D8 / run_id 隔离 / 重启续走）
// ---------------------------------------------------------------------------

/// 锚点首见记基线：新 (change, run_id) 首次 phase_next 记录当时 PhaseRecord
/// 行数基线——窗口外历史不重复记账（历史 fail 不计入重试窗口，round 自 1 起）。
#[test]
fn 锚点首见登记行数基线且窗口外历史不重复记账() {
    // 历史 fail 挂决策标记（已决形态）：未决 fail 会被 verdict-fail 决策分叉
    // 捕获（跨 run 续走同样先裁决再重跑），本用例聚焦锚点 / round 语义
    let mut fail_a = entry("dev-design", 1, Verdict::Fail, t(2));
    fail_a.decision_session_id = Some("sess-a".to_owned());
    let mut fail_b = entry("dev-design", 2, Verdict::Fail, t(3));
    fail_b.decision_session_id = Some("sess-b".to_owned());
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        fail_a,
        fail_b,
    ]);

    let anchors = SessionAnchors::new();
    let first = route(&fake, RUN, &anchors);
    assert_eq!(
        first.round, 1,
        "基线 = 首见时 PhaseRecord 行数 3，round 自 1 起"
    );
    assert_eq!(first.next_phase.as_deref(), Some("dev-design"));
    assert!(
        first.error.is_none(),
        "窗口外 2 条历史 fail 不计入重试窗口（不虚触上限标注）"
    );

    // 锚点复见：基线后新落 1 条 → round = 1（窗口）+ 1 = 2
    fake.push(entry("dev-design", 3, Verdict::Fail, t(4)));
    let second = route(&fake, RUN, &anchors);
    assert_eq!(second.round, 2, "round = 窗口条目数 + 1（非全量行数 + 1）");

    // 新 run（同锚点实例、新 run_id 键）：重新锚定当前行数 4 → round 归位 1
    let third = route(&fake, "run-b", &anchors);
    assert_eq!(third.round, 1, "run_id 键隔离：新键重新登记基线");
}

/// 锚点键 id 隔离（重写）：SessionAnchors 键为 `(change_id, run_id)`——同
/// run_id 异 change id 锚点不串台（各 id 自持行数基线，round 互不影响）。
#[test]
fn 锚点键id隔离_同run_id异id不串台() {
    let fake_a = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    // B 的 fail 挂决策标记（已决形态——未决 fail 会触发 verdict-fail 决策分叉，
    // 本用例聚焦锚点键隔离语义）
    let mut fail_b = entry_of(CHANGE_ID_B, "dev-design", 1, Verdict::Fail, t(2));
    fail_b.decision_session_id = Some("sess-b".to_owned());
    let fake_b = RouteStore::seeded_b(vec![
        entry_of(CHANGE_ID_B, "proposal", 1, Verdict::Pass, t(1)),
        fail_b,
    ]);
    let anchors = SessionAnchors::new();

    // id A 首见：基线 1 → round 1
    assert_eq!(route(&fake_a, RUN, &anchors).round, 1, "A 首见登记基线");

    // run 期间 A 落 1 条 → 复用 A 基线 1：round = 1（窗口）+ 1 = 2
    fake_a.push(entry("dev-design", 1, Verdict::Fail, t(2)));
    assert_eq!(route(&fake_a, RUN, &anchors).round, 2, "A 复用自身基线");

    // 同 run_id 异 id B：新键重新登记基线（B 行数 2）→ round 归位 1（不串 A 键）
    let outcome_b = phase_next(&fake_b, CHANGE_ID_B, RUN, &anchors).expect("B 路由应成功");
    assert_eq!(
        outcome_b.round, 1,
        "同 run_id 异 id 锚点不串台（B 自持基线，round 归位 1）"
    );
    assert_eq!(
        outcome_b.next_phase.as_deref(),
        Some("dev-design"),
        "B 路由自 B 条目序列推导（proposal 已 pass）"
    );

    // B 复见复用 B 键：窗口内 0 条新增 → round 仍 1
    assert_eq!(
        phase_next(&fake_b, CHANGE_ID_B, RUN, &anchors)
            .expect("B 复见路由应成功")
            .round,
        1,
        "B 键复见返回既有锚点"
    );
    // A 键零受影响（基线 / 条目序列各归各）
    assert_eq!(
        route(&fake_a, RUN, &anchors).round,
        2,
        "A 键不受 B 调用影响"
    );
}

/// 锚点按 run_id 隔离：同 change 不同 run_id 基线互不影响；同 (change,
/// run_id) 复用同一基线。
#[test]
fn 锚点按run_id隔离且同run复用基线() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();

    // run-a 首见：基线 1
    assert_eq!(route(&fake, "run-a", &anchors).round, 1);

    // run 期间落账 2 条 → run-a 复用基线 1：round = 1（窗口 2）+ 1 = 3
    fake.push(entry("dev-design", 1, Verdict::Fail, t(2)));
    fake.push(entry("dev-design", 2, Verdict::Fail, t(3)));
    assert_eq!(route(&fake, "run-a", &anchors).round, 3, "同 run 复用基线");

    // run-b（新窗口）：基线取当前条目数 3 → round 归位 1（跨 run 不共享）
    assert_eq!(
        route(&fake, "run-b", &anchors).round,
        1,
        "不同 run_id 基线互不影响"
    );
}

/// 锚点基线平移（D8）·重启续走：新 SessionAnchors 实例（模拟重启）铸新锚点
/// ——已 pass 相位行在场 → 路由直接推进不重头执行（AC-5 重启不重跑半边）。
#[test]
fn 锚点基线平移_重启新锚点直接推进不重头() {
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Pass, t(2)),
    ]);

    // 桌面重启后全新锚点实例 + 新 run_id
    let outcome = phase_next(
        &fake,
        CHANGE_ID,
        "run-after-restart",
        &SessionAnchors::new(),
    )
    .expect("重启后续走路由应成功");

    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("test-design"),
        "已 pass 相位行在场 → 直接推进（proposal / dev-design 不重跑）"
    );
    assert_eq!(outcome.round, 1, "新锚点基线 = 全量行数，round 自 1 起");
    assert!(outcome.error.is_none());
}

// ---------------------------------------------------------------------------
// 异常：run_id 空白 / StoreFault 故障传播；边界：LastResult i64 直透 / 只读性
// ---------------------------------------------------------------------------

/// run_id 空白 → `Err(missing_run_id)`（保留）。
#[test]
fn run_id空白显式err() {
    let fake = RouteStore::seeded(Vec::new());

    for run_id in ["", "   "] {
        let err = phase_next(&fake, CHANGE_ID, run_id, &SessionAnchors::new())
            .expect_err("空白 run_id 应 Err");
        assert!(
            err.contains("missing_run_id"),
            "Err 记因 missing_run_id，实际: {err}"
        );
    }
}

/// 假件 store 注入 `StoreFault` → `Err` 串显式失败不静默。
#[test]
fn store故障传播err显式不静默() {
    let fake = RouteStore::seeded(Vec::new());
    fake.set_fault(StoreFault::Db("注入的读故障".to_owned()));

    let err = phase_next(&fake, CHANGE_ID, RUN, &SessionAnchors::new())
        .expect_err("StoreFault 应传播为 Err");

    assert!(
        err.contains("db:") && err.contains("注入的读故障"),
        "Err 记因携带 fault 语境，实际: {err}"
    );
}

/// LastResult 演进：last_result.timestamp 出 i64 millis 直透（db 时间戳原样，
/// ISO 转换不在此层）；条目时间戳恒在位（`Option<i64>` 收窄为恒值兼容保留）。
#[test]
fn last_result时间戳i64直透() {
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(5)),
        entry("dev-design", 1, Verdict::Fail, t(9)),
    ]);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    let last = outcome.last_result.expect("有相位史应携 last_result");
    assert_eq!(last.phase, "dev-design", "timestamp 降序定位最新条目");
    assert_eq!(
        last.timestamp,
        Some(t(9)),
        "i64 millis 原样直透（不做 ISO 转换）"
    );
}

/// phase_next 只读性：反复路由后假件条目序列不变（本假件写半边全部
/// `unimplemented!`——任何写面触达即 panic，结构性执法零写触点）。
#[test]
fn 只读路由_零写面触达() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let before = fake.entry_count();
    let anchors = SessionAnchors::new();

    for _ in 0..3 {
        let _ = route(&fake, RUN, &anchors);
    }

    assert_eq!(fake.entry_count(), before, "只读路由：条目序列零变更");
    // 复核条目内容未被改写（stale / backtrack 位不被动；initialize 表头条目
    // 预置在案 + proposal 种子 = 两行）
    let entries = fake
        .list_phase_records(CHANGE_ID)
        .expect("假件读半边应可用");
    assert_eq!(entries.len(), 2, "initialize 表头预置 + proposal 种子");
    assert!(!entries[0].stale && !entries[1].stale);
    assert!(entries[0].backtrack_to.is_none() && entries[1].backtrack_to.is_none());
}

// ---------------------------------------------------------------------------
// 计划步序列与产物清单下发（AC-1 下发半边：路由权威不变，响应结构扩字段）
// ---------------------------------------------------------------------------

/// 相位定义查表（真件只读；steps / manifest 逐项等值断言的对照源）。
fn phase_def(phase: &str) -> &'static super::phase_table::PhaseDefinition {
    super::phase_table::phase_table("requirement")
        .expect("requirement 表恒可达")
        .iter()
        .find(|def| def.id == phase)
        .unwrap_or_else(|| panic!("相位 {phase} 应在表内"))
}

/// 初始路由（零相位史）→ `steps` / `artifact_manifest` 为 next 相位（首相位
/// proposal）的声明克隆；pass 推进后随 next 相位换形（dev-design 清单在场）。
#[test]
fn 计划步序列与清单随next相位下发() {
    let fake = RouteStore::seeded(Vec::new());
    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert_eq!(outcome.next_phase.as_deref(), Some("proposal"));
    let proposal = phase_def("proposal");
    assert_eq!(
        outcome.steps,
        proposal.steps.to_vec(),
        "初始路由下发首相位计划步序列（与 phase_table 声明逐项等值）"
    );
    assert_eq!(
        outcome.artifact_manifest,
        proposal.artifact_manifest.to_vec(),
        "产物清单随行（proposal 清单两项）"
    );
    assert_eq!(
        outcome.steps.len(),
        5,
        "proposal 五步（含三门之 verdict 门）"
    );

    // pass 推进：next 相位换形 → steps / manifest 随新相位下发
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert_eq!(outcome.next_phase.as_deref(), Some("dev-design"));
    let dev_design = phase_def("dev-design");
    assert_eq!(outcome.steps, dev_design.steps.to_vec());
    assert_eq!(
        outcome.artifact_manifest,
        dev_design.artifact_manifest.to_vec()
    );
    assert!(
        outcome
            .artifact_manifest
            .iter()
            .any(|spec| spec.kind == "markdown-doc" && spec.path == "tasks.md"),
        "dev-design 清单含 tasks.md（markdown-doc 文档视图）"
    );
}

/// 相位无产物清单（test-gen 等）→ `artifact_manifest` 空 Vec 下发（零占位不
/// 虚设）；同时 steps 携门禁步声明（StaticCheck 携 FeedbackLoop{5}）——walker
/// 消费步声明的下发行。（implement 清单现声明 `tasks-progress·tasks.md`，
/// 空清单示例相位让位给 test-gen。）
#[test]
fn 无清单相位下发空manifest与门禁步声明() {
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Pass, t(2)),
        entry("test-design", 1, Verdict::Pass, t(3)),
        entry("implement", 1, Verdict::Pass, t(4)),
    ]);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert_eq!(outcome.next_phase.as_deref(), Some("test-gen"));
    assert!(
        outcome.artifact_manifest.is_empty(),
        "test-gen 无产物清单 → 空 Vec 下发"
    );
    let test_gen = phase_def("test-gen");
    assert_eq!(outcome.steps, test_gen.steps.to_vec());
    assert!(
        outcome.steps.iter().any(|step| matches!(
            step.on_fail,
            Some(crate::state::OnFail::FeedbackLoop { limit: 5 })
        )),
        "门禁步声明随 steps 下发（FeedbackLoop 上限 5）"
    );
}

/// 末相位 all pass → `done=true` / `next_phase=None` 时 steps 与 manifest 均空
///（无 next 相位即无计划步 / 无清单；既有收敛语义零改写）。
#[test]
fn 全pass终态不下发计划步与清单() {
    let entries: Vec<PhaseStateRecord> = ALL_PHASES
        .iter()
        .enumerate()
        .map(|(index, phase)| entry(phase, 1, Verdict::Pass, t(index as u32 + 1)))
        .collect();
    let fake = RouteStore::seeded(entries);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert!(outcome.done, "全相位 pass → done");
    assert_eq!(outcome.next_phase, None);
    assert!(outcome.steps.is_empty(), "无 next 相位 → steps 空");
    assert!(
        outcome.artifact_manifest.is_empty(),
        "无 next 相位 → 清单空"
    );
}

/// 重试上限分叉（error）响应同样不下发计划步 / 清单（错误响应结构面：
/// `steps` / `manifest` 空、next_phase=None、error 在场）。
#[test]
fn 错误响应不下发计划步与清单() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors); // 首见登记基线

    let mut outcome = None;
    for attempt in 1..=MAX_RETRY_TIMES {
        fake.push(entry(
            "dev-design",
            attempt,
            Verdict::Fail,
            t(u32::from(attempt) + 1),
        ));
        outcome = Some(route(&fake, RUN, &anchors));
    }
    let outcome = outcome.expect("到达重试上限应有响应");

    assert!(outcome.error.is_some(), "重试上限 → 决策分叉错误");
    assert_eq!(outcome.next_phase, None);
    assert!(outcome.steps.is_empty(), "错误响应不下发计划步");
    assert!(
        outcome.artifact_manifest.is_empty(),
        "错误响应不下发产物清单"
    );
}

// ---------------------------------------------------------------------------
// 归档相位路由（AC-1 / AC-4 / AC-7 / AC-8）：零 archive 特判——archive 为表内
// 第 9 相位，`has_phase_passed` / `allowed_backtrack_phases` 与其余相位同式。
// ---------------------------------------------------------------------------

/// 归档前停等路由（正向）：前 8 路由相位（proposal…acceptance）全
/// `verdict=pass` 落账后 → `phase_next` 返回 `next_phase=Some("archive")`（archive
/// 未落账故 `all_passed` 为假），steps 随行十一步序列（Cleanup 尽力步随表自然
/// 出线）、executor / evaluator 缺席（archive `executor=None` / `evaluator=None`）、
/// artifact_manifest 空（AC-1 / AC-4 / AC-7 stop-before-archive 路由依据）。
#[test]
fn 八路由相位pass后路由至archive且零角色spec() {
    let entries = ROUTED_PHASES
        .iter()
        .enumerate()
        .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
        .collect();
    let fake = RouteStore::seeded(entries);

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert!(!outcome.done, "archive 未落账 → 非收口");
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("archive"),
        "8 路由相位 pass 后下一相位即 archive"
    );
    assert!(outcome.error.is_none(), "归档路由非错误面");
    assert_eq!(outcome.steps.len(), 11, "archive 十一步序列随行下发");
    assert_eq!(
        outcome.steps.first().map(|step| step.node),
        Some(crate::state::StepNode::PhaseStart)
    );
    assert_eq!(
        outcome.steps.last().map(|step| step.node),
        Some(crate::state::StepNode::PhaseLog)
    );
    assert!(
        outcome.artifact_manifest.is_empty(),
        "archive 零产物清单（空数组下发）"
    );
    assert!(
        outcome.executor.is_none() && outcome.evaluator.is_none(),
        "archive 无会话角色 spec（确定性相位）"
    );
    assert_eq!(
        ids(&outcome.allowed_backtrack_phases),
        ALL_PHASES[1..].to_vec(),
        "archive 白名单 = 除 initialize 外表序 9 相位（含自身、不含未来相位——表头被 backtrackable 过滤）"
    );
}

/// archive 空 checklist 落账后收口（正向）：archive 以 `report=""` + 空 checklist
/// 落 `verdict=pass` 后 → `phase_next` 返回 `done`（`all_passed` 真；archive 完成
/// 才 Completed，AC-4 / AC-7）。
#[test]
fn archive空checklist落账后路由done() {
    let mut entries: Vec<PhaseStateRecord> = ROUTED_PHASES
        .iter()
        .enumerate()
        .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
        .collect();
    // archive 落账形态：report 空、checklist 空表、verdict = pass（空 checklist 推导）
    let mut archive_entry = entry("archive", 1, Verdict::Pass, t(20));
    archive_entry.report = String::new();
    archive_entry.checklist = Vec::new();
    assert!(archive_entry.report.is_empty() && archive_entry.checklist.is_empty());
    entries.push(archive_entry);

    let fake = RouteStore::seeded(entries);
    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert!(outcome.done, "archive 落账 pass → 全表 all_passed → done");
    assert_eq!(outcome.next_phase, None);
    assert!(outcome.steps.is_empty() && outcome.artifact_manifest.is_empty());
}

/// archive 未落账 / 落 `verdict=fail` 或 stale（边界）：`next_phase` 仍指 archive、
/// 未 done——archive 完成才 Completed，失败落账不自作推进（AC-7 / AC-8）。
#[test]
fn archive未完成或落fail时仍指archive未done() {
    let base: Vec<PhaseStateRecord> = ROUTED_PHASES
        .iter()
        .enumerate()
        .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
        .collect();

    // 未落账：仍指 archive
    let fake = RouteStore::seeded(base.clone());
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert_eq!(outcome.next_phase.as_deref(), Some("archive"));
    assert!(!outcome.done);

    // 落 fail：仍指 archive（未过）
    let mut with_fail = base.clone();
    with_fail.push(entry("archive", 1, Verdict::Fail, t(30)));
    let fake = RouteStore::seeded(with_fail);
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("archive"),
        "archive 失败落账未过 → 仍路由 archive"
    );
    assert!(!outcome.done, "archive 未完成 → 不 Completed");

    // 落 pass 但 stale（回溯作废）：与其余相位同式——stale 条目不算过
    let mut with_stale = base.clone();
    let mut stale_archive = entry("archive", 1, Verdict::Pass, t(31));
    stale_archive.stale = true;
    with_stale.push(stale_archive);
    let fake = RouteStore::seeded(with_stale);
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("archive"),
        "stale 落账不算过（has_phase_passed 同式语义）"
    );
    assert!(!outcome.done);
}

/// archive 判定同式（零特判边界）：`archive` 以 `skipped=true` 落账视为已过
///（与其余相位同式——`verdict==Pass || skipped`）；archive 落账条目在案后
/// `round` 随窗口条目数递增，路由面无 `if phase == "archive"` 语义差异（AC-4）。
#[test]
fn archive判定与其余相位同式零特判() {
    let mut entries: Vec<PhaseStateRecord> = ROUTED_PHASES
        .iter()
        .enumerate()
        .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
        .collect();
    // 归档未落账路由：anchor 在首轮登记，round = 1
    let fake = RouteStore::seeded(entries.clone());
    let anchors = SessionAnchors::new();
    let outcome = route(&fake, RUN, &anchors);
    assert_eq!(outcome.next_phase.as_deref(), Some("archive"));
    assert_eq!(outcome.round, 1, "首轮路由 round=1");

    // 归档 skipped 落账（skipped=true 且 verdict=pass）→ 视为已过（同式语义）
    let mut skipped_archive = entry("archive", 1, Verdict::Pass, t(40));
    skipped_archive.skipped = true;
    entries.push(skipped_archive);
    let fake = RouteStore::seeded(entries);
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert!(
        outcome.done,
        "archive skipped 落账与其余相位同式（视为已过）"
    );
    assert_eq!(outcome.next_phase, None);
}

/// archive 不在任何相位回溯白名单闭包（边界 / 回归）：archive 为末相位、非任何
/// 相位前置，故 acceptance 及其前的各相位 `allowed_backtrack_phases` 均不含
/// archive（AC-8 / 零改动锚——archive 仅前向）；archive 自身白名单含己恰一次
///（白名单含自身语义，与其余相位同式）。
#[test]
fn archive不在任何相位回溯白名单内() {
    let entries: Vec<PhaseStateRecord> = ALL_PHASES
        .iter()
        .enumerate()
        .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
        .collect();
    // 全 pass → 无白名单下发（收口面）
    let fake = RouteStore::seeded(entries);
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert!(outcome.done && outcome.allowed_backtrack_phases.is_empty());

    // acceptance 为下一相位（archive 未落账）：白名单 = 除 initialize 外表序
    // 前 9 相位，不含 archive
    let fake = RouteStore::seeded(
        ROUTED_PHASES[..8]
            .iter()
            .enumerate()
            .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
            .collect(),
    );
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert_eq!(outcome.next_phase.as_deref(), Some("acceptance"));
    assert_eq!(
        outcome.allowed_backtrack_phases.len(),
        8,
        "acceptance 白名单 = proposal … acceptance 八相位（含自身；initialize 表头被属性过滤——排除非位置推导）"
    );
    assert!(
        !outcome
            .allowed_backtrack_phases
            .iter()
            .any(|option| option.phase == "archive"),
        "archive 不在 acceptance 回溯白名单内（末相位仅前向）"
    );

    // archive 自身白名单：含己恰一次（含自身语义与其余相位同式）
    let fake = RouteStore::seeded(
        ROUTED_PHASES
            .iter()
            .enumerate()
            .map(|(idx, phase)| entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)))
            .collect(),
    );
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert_eq!(outcome.next_phase.as_deref(), Some("archive"));
    assert_eq!(
        outcome
            .allowed_backtrack_phases
            .iter()
            .filter(|option| option.phase == "archive")
            .count(),
        1,
        "archive 自身在自身白名单内恰一次（白名单含自身语义）"
    );
    assert_eq!(
        outcome
            .allowed_backtrack_phases
            .last()
            .map(|option| option.phase.as_str()),
        Some("archive"),
        "archive 白名单末位为自身（不含未来相位）"
    );
}

/// store 读半 Err 透传（异常 / 回归）：`list_phase_records` 返回 Err /
/// 未建档 change → `Err(String)` 透传（归档路由不吞错、不降级空表——既有语义
/// 不改写，AC-4）。
#[test]
fn 归档路由store_read_err显式透传() {
    // 未建档：显式 Err（不静默空产出）
    let missing = RouteStore::missing();
    let error = phase_next(&missing, CHANGE_ID, RUN, &SessionAnchors::new())
        .expect_err("未建档 change 应显式 Err");
    assert!(error.contains("未建档"), "未建档错误语汇: {error}");

    // 读半 Err 透传
    let faulty = RouteStore::seeded(Vec::new());
    faulty.set_fault(StoreFault::Db("读面故障".to_owned()));
    let error = phase_next(&faulty, CHANGE_ID, RUN, &SessionAnchors::new())
        .expect_err("store 读半 Err 应透传");
    assert!(error.contains("读面故障"), "StoreFault 语汇透传: {error}");
}

// ---------------------------------------------------------------------------
// 空 checklist 落账链路（AC-4）：`derive_verdict([]) = Pass` —— archive 相位经
// 写面 `phase_log` 以 `report=""` + 空 checklist 落账，随后路由出 done。
// ---------------------------------------------------------------------------

/// 落账 + 路由双面假件（`phase_log` 的 log_phase 捕获并镜像真件落库语义：
/// attempt = 既有条目数 + 1、条目追加、active_phase 清位；读半供 `phase_next`）。
struct ArchiveLandingStore {
    record: Mutex<Option<ChangeStateRecord>>,
    entries: Mutex<Vec<PhaseStateRecord>>,
    commands: Mutex<Vec<crate::state::PhaseLogCommand>>,
}

impl ArchiveLandingStore {
    /// 归档相位开相形态（active_phase = archive，可落账）。
    fn with_archive_open() -> Self {
        let mut record = requirement_record(CHANGE_ID, CHANGE_NAME);
        record.active_phase = Some(crate::state::ActivePhaseState {
            phase: "archive".to_owned(),
            attempt: 1,
            start_at: t(0),
        });
        Self {
            record: Mutex::new(Some(record)),
            entries: Mutex::new(Vec::new()),
            commands: Mutex::new(Vec::new()),
        }
    }

    fn command_count(&self) -> usize {
        self.commands.lock().expect("命令锁不可中毒").len()
    }

    fn last_command(&self) -> crate::state::PhaseLogCommand {
        self.commands
            .lock()
            .expect("命令锁不可中毒")
            .last()
            .expect("落账命令应在场")
            .clone()
    }
}

impl ChangeStateStore for ArchiveLandingStore {
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self
            .record
            .lock()
            .expect("记录锁不可中毒")
            .clone()
            .filter(|record| record.id == id))
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(&self, _change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        Ok(self.entries.lock().expect("条目锁不可中毒").clone())
    }

    fn list_steps(
        &self,
        _change_id: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn create_change_record(&self, _record: ChangeStateRecord) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn delete_change_record(&self, _id: &str) -> Result<bool, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn start_phase(
        &self,
        _change_id: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<crate::state::PhaseStartState, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn log_phase(&self, command: &crate::state::PhaseLogCommand) -> Result<u32, StoreFault> {
        let mut entries = self.entries.lock().expect("条目锁不可中毒");
        let attempt = entries
            .iter()
            .filter(|entry| entry.phase == command.phase)
            .count() as u32
            + 1;
        let id = entries.len() as i64;
        entries.push(PhaseStateRecord {
            id,
            change_id: command.change_id.clone(),
            phase: command.phase.clone(),
            attempt,
            verdict: command.verdict,
            report: command.report.clone(),
            checklist: command.checklist.clone(),
            skipped: command.skipped,
            stale: false,
            backtrack_to: None,
            backtrack_reason: None,
            executor_session_id: command.executor_session_id.clone(),
            evaluator_session_id: command.evaluator_session_id.clone(),
            decision_session_id: command.decision_session_id.clone(),
            start_at: command.start_at,
            timestamp: command.timestamp,
        });
        drop(entries);
        if let Some(record) = self.record.lock().expect("记录锁不可中毒").as_mut() {
            record.active_phase = None;
        }
        self.commands
            .lock()
            .expect("命令锁不可中毒")
            .push(command.clone());
        Ok(attempt)
    }

    fn apply_backtrack(&self, _command: &crate::state::BacktrackCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn amend_decision_session(
        &self,
        _change_id: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn set_archived(&self, _id: &str, _archived_at: i64) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn append_step(&self, _command: &StepCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_runs(&self, _change: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_run_nodes(&self, _run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_run_edges(&self, _run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_start(&self, _command: &RunStartCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_finish(&self, _command: &RunFinishCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }
}

/// 空 checklist 识别（正向）：archive 以 `report=""` + 空 checklist 经写面
/// [`phase_log`](super::phase_log::phase_log) 落账 → `derive_verdict([]) = Pass`
///（空表 every 语义），落库载荷 verdict 为 `pass`、report 空、checklist 空表、
/// 三会话槽位 None；随后 `phase_next` 出 done（AC-4 端到端）。
#[test]
fn archive空checklist落账推导pass并随后done() {
    let store = ArchiveLandingStore::with_archive_open();
    // 前置：8 路由相位全 pass 在案
    {
        let mut entries = store.entries.lock().expect("条目锁不可中毒");
        for (idx, phase) in ROUTED_PHASES.iter().enumerate() {
            entries.push(entry(phase, 1, Verdict::Pass, t(idx as u32 + 1)));
        }
    }

    let outcome = super::phase_log::phase_log(
        &store,
        CHANGE_ID,
        &super::phase_log::PhaseLogInput {
            phase: "archive".to_owned(),
            report: String::new(),
            checklist: Vec::new(),
            skipped: false,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        },
    )
    .expect("archive 空 checklist 落账应成功");

    assert_eq!(outcome.phase, "archive");
    assert_eq!(outcome.attempt, 1, "首次落账 attempt=1");
    assert_eq!(store.command_count(), 1, "恰一条落账写命令");

    let command = store.last_command();
    assert_eq!(
        command.verdict,
        Verdict::Pass,
        "derive_verdict([]) = Pass（空 checklist 识别为通过）"
    );
    assert_eq!(command.report, "", "归档落账 report 为空串");
    assert!(command.checklist.is_empty(), "归档落账 checklist 空表");
    assert!(!command.skipped, "归档落账 skipped=false（非跳过语义）");
    assert!(
        command.executor_session_id.is_none()
            && command.evaluator_session_id.is_none()
            && command.decision_session_id.is_none(),
        "归档相位三会话槽位恒 None"
    );
    assert_eq!(command.change_id, CHANGE_ID, "归属键随行");
    assert_eq!(command.phase, "archive");

    // 落账后路由：全表 all_passed → done（archive 完成才 Completed）
    let anchors = SessionAnchors::new();
    let routed = phase_next(&store, CHANGE_ID, RUN, &anchors).expect("路由应成功");
    assert!(routed.done, "archive 空 checklist 落账后路由 done");
    assert_eq!(routed.next_phase, None);
    assert!(routed.allowed_backtrack_phases.is_empty());

    // 落账清位：active_phase 归 None（写事务内清位语义）
    assert!(
        store
            .record
            .lock()
            .expect("记录锁不可中毒")
            .as_ref()
            .expect("建档记录应在场")
            .active_phase
            .is_none(),
        "落账后 active_phase 清位"
    );
}

// ---------------------------------------------------------------------------
// fail_decided 幂等闸边界（test-execution-zero-agent-decision AC-7）
// ---------------------------------------------------------------------------

/// 无 fail 条目（边界，表驱动 2 形态）：entries 为空 / entries 全 pass →
/// `error = None`，直接出可执步序列（决策分叉零触发）。
#[test]
fn 无fail条目出可执步序列零分叉() {
    // 形态一：零条目（表头未落账形态 → 落 initialize 表头）
    let fake = RouteStore::with_record(
        Some(requirement_record(CHANGE_ID, CHANGE_NAME)),
        Vec::new(),
    );
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert!(outcome.error.is_none(), "零条目零分叉");
    assert_eq!(outcome.next_phase.as_deref(), Some("initialize"));

    // 形态二：全 pass（表头 + 前四相位 pass → 落 implement，可执步序列在场）
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Pass, t(2)),
        entry("test-design", 1, Verdict::Pass, t(3)),
    ]);
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert!(outcome.error.is_none(), "全 pass 零分叉");
    assert_eq!(outcome.next_phase.as_deref(), Some("implement"));
    assert!(!outcome.steps.is_empty(), "可执步序列随行");
    assert!(outcome.executor.is_some() && outcome.evaluator.is_some());
}

/// 最新条目为 pass（边界）：最新条目 `verdict = Pass`（即便携
/// `decision_session_id`）→ 不算 `fail_decided`，判据只看最新 fail 条目——
/// 放行推进而非停在同相位重跑。
#[test]
fn 最新条目为pass不算fail_decided() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);

    // dev-design#1 fail 后重做 pass（携决策挂账键——历史痕迹）
    fake.push(entry("dev-design", 1, Verdict::Fail, t(2)));
    let _ = route(&fake, RUN, &anchors);
    let mut redone = entry("dev-design", 2, Verdict::Pass, t(3));
    redone.decision_session_id = Some("sess-decision-1".to_owned());
    fake.push(redone);

    let outcome = route(&fake, RUN, &anchors);
    assert!(
        outcome.error.is_none(),
        "最新条目 pass → 零分叉（fail_decided 判据只看最新条目）"
    );
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("test-design"),
        "放行推进至下一相位（非停在 dev-design 重跑）"
    );
}

/// fail 条目非最新（边界）：早先 fail、其后 pass 条目 → 不触发决策分叉
/// （latest 过滤语义：早先 fail 不复活）。
#[test]
fn fail条目非最新不触发分叉() {
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Fail, t(2)),
        entry("dev-design", 2, Verdict::Pass, t(3)),
    ]);
    let outcome = route(&fake, RUN, &SessionAnchors::new());
    assert!(
        outcome.error.is_none(),
        "早先 fail 非最新 → 零分叉（历史 fail 不复活）"
    );
    assert_eq!(outcome.next_phase.as_deref(), Some("test-design"));
}

/// 回溯重入后新 fail 条目照常分叉（正向）：已决 fail（backtrack 重入的第一
/// 次尝试）→ 目标相位真执行后新 attempt 落 fail（`decision_session_id = None`）
/// → 再次 `VerdictFail` 分叉（幂等闸只认「该条目」不认「该相位」）。
#[test]
fn 回溯重入新fail条目照常分叉() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);

    // 首次 fail → 分叉 → 决策挂账（backtrack 已落账，重入 dev-design）
    fake.push(entry("dev-design", 1, Verdict::Fail, t(2)));
    let first = route(&fake, RUN, &anchors);
    assert_eq!(
        first.error,
        Some(PhaseNextError::VerdictFail {
            phase: "dev-design".to_owned(),
            round: 2,
        })
    );
    let mut decided = entry("dev-design", 1, Verdict::Fail, t(3));
    decided.decision_session_id = Some("sess-decision-1".to_owned());
    // 以最新条目替换语义等价：直接追加已决条目（同 attempt 重挂，时间戳更新）
    fake.push(decided);
    let allowed = route(&fake, RUN, &anchors);
    assert!(allowed.error.is_none(), "同一条目已决不重复分叉");

    // 重入后真执行再落新 attempt fail → 照常分叉
    fake.push(entry("dev-design", 2, Verdict::Fail, t(5)));
    let again = route(&fake, RUN, &anchors);
    assert_eq!(
        again.error,
        Some(PhaseNextError::VerdictFail {
            phase: "dev-design".to_owned(),
            round: 4,
        }),
        "新 fail 条目（无决策标记）照常分叉"
    );
}

/// backtrack 分支先手接管（边界）：已决 fail 条目之后落 backtrack 条目 →
/// 路由直取回跳目标、`error = None`（回溯分支先于 fail_decided 判定，不二次
/// 分叉决策）；目标相位重执行后再落新 fail 才照常分叉。
#[test]
fn backtrack条目先手接管不二次分叉() {
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Pass, t(2)),
        entry("test-design", 1, Verdict::Pass, t(3)),
    ]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors);

    // implement 首轮 fail → 决策已消费（挂账）
    fake.push(entry("implement", 1, Verdict::Fail, t(4)));
    let forked = route(&fake, RUN, &anchors);
    assert_eq!(
        forked.error,
        Some(PhaseNextError::VerdictFail {
            phase: "implement".to_owned(),
            round: 2,
        })
    );
    let mut decided = entry("implement", 1, Verdict::Fail, t(5));
    decided.decision_session_id = Some("sess-decision-1".to_owned());
    fake.push(decided);

    // 决策 backtrack 落账（backtrack_to 标记条目为最新）→ 路由直取目标
    fake.push(backtrack_entry(
        "implement",
        1,
        t(6),
        "dev-design",
        "设计返工",
    ));
    let targeted = route(&fake, RUN, &anchors);
    assert!(
        targeted.error.is_none(),
        "回溯分支先手接管——已决条目不再二次分叉"
    );
    assert_eq!(
        targeted.next_phase.as_deref(),
        Some("dev-design"),
        "路由直取回跳目标相位"
    );
    assert!(
        targeted
            .executor
            .expect("目标相位 executor 在场")
            .prompt
            .contains("⚠️ 回溯原因: 设计返工"),
        "回溯原因行独立注入（零失败反馈段）"
    );
}

/// `PhaseNextError` 字段面边界（边界，参数化 4 值）：`phase` 为空串 / 8 字符、
/// `round` = 0 与 `u32::MAX` 均可构造且 `PartialEq` 可辨（字段收缩后两变体零
/// 冗余载荷；标注差异不吞相位 / 轮次）。
#[test]
fn phase_next_error字段面边界() {
    let empty_phase = PhaseNextError::VerdictFail {
        phase: String::new(),
        round: 0,
    };
    let named_phase = PhaseNextError::VerdictFail {
        phase: "dev-desi".to_owned(),
        round: u32::MAX,
    };
    assert_ne!(empty_phase, named_phase, "phase / round 逐字段可辨");
    assert_eq!(empty_phase.clone(), empty_phase, "PartialEq 自反");

    let max_retries = PhaseNextError::MaxRetriesExceeded {
        phase: String::new(),
        round: 0,
    };
    assert_ne!(
        max_retries,
        PhaseNextError::MaxRetriesExceeded {
            phase: "dev-desi".to_owned(),
            round: 0,
        },
        "空串 / 8 字符相位名可辨"
    );
    assert_ne!(
        max_retries,
        PhaseNextError::MaxRetriesExceeded {
            phase: String::new(),
            round: u32::MAX,
        },
        "round 0 / u32::MAX 可辨"
    );
    // 同变体同载荷等值（PartialEq 按字段），两变体同载荷仍可辨（标注语义独立）
    assert_eq!(
        empty_phase,
        PhaseNextError::VerdictFail {
            phase: String::new(),
            round: 0,
        },
        "同值不同实例等值（PartialEq 按字段）"
    );
    assert_ne!(
        empty_phase,
        PhaseNextError::MaxRetriesExceeded {
            phase: String::new(),
            round: 0,
        },
        "VerdictFail / MaxRetriesExceeded 同载荷仍可辨"
    );
}
