//! `write::phase_next` 的单元测试（test-design「phase_next.rs ->
//! phase_next_test.rs」节）：只读路由状态机（形参 change **id**）的初始 / 推进
//! / fail 重试 / 重试上限 / 终态 / backtrack 目标路由 / 白名单下发 /
//! last_result（i64 millis 直透）/ prompt 插值（`<change>` 段取 `record.name`
//! ——id 串不误入 prompt，D7 插值改点）/ 会话锚点（复合键 `(change_id,
//! run_id)`、基线自 PhaseRecord 行数平移——D8、同 run_id 异 id 不串台、重启
//! 新实例直接推进）/ StoreFault 故障传播 / 只读性 / 未建档 id 显式 Err。
//!
//! Mock策略（test-design 本节 Mock 表）：路由语义 / 故障传播 / 锚点各 describe
//! 全部走进程内假件实现 [`ChangeStateStore`]（design D1 fake port 先例——本假
//! 件只实现读半边，写半边 `unimplemented!`，phase_next 若越权触写即 panic，
//! 兼作只读性执法）。「读源 db」真实 tempfile Store 行收 tests/
//! corpus_golden_test.rs 集成面（workflow 自环 dev-dep 在 lib-test 与普通
//! lib 双工件下类型不统一，`&Store` 无法满足 lib-test 视角的 trait——真实 db
//! 组合只在集成目标可编译，见变更报告）。确定性时间戳全部 i64 unix millis
//! 常量，零 wall-clock 等值比较。

use std::sync::Mutex;

use super::phase_next::{phase_next, PhaseNextError, SessionAnchors};
use super::phase_table::MAX_RETRY_TIMES;
use crate::model::Verdict;
use crate::state::{
    ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseStateRecord, RunFinishCommand,
    RunStartCommand, RunStateRecord, RunStepStateRecord, StepCommand, StepStateRecord, StoreFault,
};

/// 身份锚字面量（路由入参——一切寻址以 id 为准；prompt 插值取 name）。
const CHANGE_ID: &str = "0198f7a0-0000-7000-8000-0000000000e6";
/// change 名（prompt `<change>` 段供给值——裸名，非身份键）。
const CHANGE_NAME: &str = "demo-change";
/// 第二 change（锚点键 id 隔离用例：同 run_id 异 id 两键）。
const CHANGE_ID_B: &str = "0198f7a0-0000-7000-8000-0000000000e7";
const CHANGE_NAME_B: &str = "other-change";
/// 库内不存在的 id（未建档拒绝面）。
const UNKNOWN_ID: &str = "0198f7a0-0000-7000-8000-0000000000fa";
const RUN: &str = "run-1";

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
    fn seeded(entries: Vec<PhaseStateRecord>) -> Self {
        Self::with_record(Some(requirement_record(CHANGE_ID, CHANGE_NAME)), entries)
    }

    /// 第二 change 建档（锚点键 id 隔离用例：同 run_id 异 id 两份条目序列）。
    fn seeded_b(entries: Vec<PhaseStateRecord>) -> Self {
        Self::with_record(
            Some(requirement_record(CHANGE_ID_B, CHANGE_NAME_B)),
            entries,
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

    fn list_run_steps(&self, _run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault> {
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

/// 初始路由：建档零相位行（id 与 name 双值可辨）→ 首相位 proposal、
/// round=1、executor / evaluator prompt 的 `<change>` 段插值为 `record.name`
/// （id 串不误入 prompt——D7 插值改点行为锚）、`<phase>` 段插值、白名单为空
///（首相位无前置）。
#[test]
fn 初始路由落首相位且prompt插值取record_name() {
    let fake = RouteStore::seeded(Vec::new());

    let outcome = route(&fake, RUN, &SessionAnchors::new());

    assert!(!outcome.done);
    assert_eq!(
        outcome.next_phase.as_deref(),
        Some("proposal"),
        "首相位路由"
    );
    assert_eq!(outcome.round, 1, "零相位行窗口首轮");
    assert!(outcome.error.is_none());

    let executor = outcome.executor.expect("proposal 应有 executor");
    let evaluator = outcome.evaluator.expect("proposal 应有 evaluator");
    assert!(
        executor.prompt.contains(CHANGE_NAME) && !executor.prompt.contains("<change>"),
        "executor prompt 的 <change> 段插值为 record.name: {}",
        executor.prompt
    );
    assert!(
        !executor.prompt.contains(CHANGE_ID),
        "id 串不误入 executor prompt（插值单点 = record.name——D7）: {}",
        executor.prompt
    );
    assert!(
        evaluator.prompt.contains(CHANGE_NAME) && !evaluator.prompt.contains("<change>"),
        "evaluator prompt 的 <change> 段插值为 record.name"
    );
    assert!(
        !evaluator.prompt.contains(CHANGE_ID),
        "id 串不误入 evaluator prompt（D7）"
    );
    assert!(
        executor.prompt.contains("proposal"),
        "executor prompt 已完成 <phase> 插值"
    );
    assert!(
        outcome.allowed_backtrack_phases.is_empty(),
        "首相位白名单为空"
    );
    assert!(outcome.last_result.is_none(), "零相位行无 last_result");
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

/// workflow_type 非 requirement → Err 显式分层出口（W8 写面侧）。
#[test]
fn workflow_type非requirement时err显式分层出口() {
    let fake = RouteStore::with_workflow_type("bug-fix");

    let err = phase_next(&fake, CHANGE_ID, RUN, &SessionAnchors::new())
        .expect_err("非 requirement 应 Err");

    assert!(
        err.contains("bug-fix") && err.contains("requirement"),
        "W8 分层出口记因，实际: {err}"
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
        outcome.allowed_backtrack_phases,
        vec!["proposal".to_owned(), "dev-design".to_owned()],
        "白名单 = 新相位的表序前置集"
    );
    let last = outcome.last_result.expect("有相位史应携 last_result");
    assert_eq!(last.phase, "proposal");
    assert_eq!(last.verdict, Verdict::Pass);
}

/// fail 预算内重试：窗口 fail 数 < MAX_RETRY_TIMES → 同相位重入、round 递增、
/// prompt 同相位插值（上限判定不自建——写面 phase-next 权威返回）。
#[test]
fn fail预算内重试同相位且round递增() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();

    let first = route(&fake, RUN, &anchors);
    assert_eq!(first.next_phase.as_deref(), Some("dev-design"));
    assert_eq!(first.round, 1);

    // run 期间逐条落账两条 fail（窗口 fail 数 2 < 5）
    fake.push(entry("dev-design", 1, Verdict::Fail, t(2)));
    let second = route(&fake, RUN, &anchors);
    assert_eq!(
        second.next_phase.as_deref(),
        Some("dev-design"),
        "fail 同相位重试"
    );
    assert_eq!(second.round, 2, "窗口条目数 + 1");
    assert!(second.error.is_none(), "预算内不触发上限");
    let second_prompt = second
        .executor
        .as_ref()
        .expect("executor 在场")
        .prompt
        .clone();
    assert!(
        second_prompt.contains(CHANGE_NAME) && !second_prompt.contains(CHANGE_ID),
        "executor prompt 同相位插值取 record.name（id 串零入 prompt）: {second_prompt}"
    );
    assert!(
        second
            .evaluator
            .as_ref()
            .expect("evaluator 在场")
            .prompt
            .contains("dev-design"),
        "evaluator prompt 同相位插值（<phase> → dev-design）"
    );

    fake.push(entry("dev-design", 2, Verdict::Fail, t(3)));
    let third = route(&fake, RUN, &anchors);
    assert_eq!(third.round, 3, "round 随窗口条目递增");
    let last = third.last_result.expect("last_result 在场");
    assert_eq!(last.phase, "dev-design");
    assert_eq!(last.verdict, Verdict::Fail, "last_result 携最新 fail 条目");
}

/// 重试上限恰达：窗口 fail 数 == MAX_RETRY_TIMES →
/// error=MaxRetriesExceeded{phase, round}、next_phase=None、done=false
///（决策分叉触发点保留）。
#[test]
fn 重试上限恰达返回max_retries_exceeded() {
    let fake = RouteStore::seeded(vec![entry("proposal", 1, Verdict::Pass, t(1))]);
    let anchors = SessionAnchors::new();
    let _ = route(&fake, RUN, &anchors); // 首见登记基线 1

    for attempt in 1..=MAX_RETRY_TIMES {
        fake.push(entry(
            "dev-design",
            attempt,
            Verdict::Fail,
            t(u32::from(attempt) + 1),
        ));
        let outcome = route(&fake, RUN, &anchors);
        if attempt < MAX_RETRY_TIMES {
            assert!(outcome.error.is_none(), "第 {attempt} 条 fail 未达上限");
        } else {
            assert_eq!(
                outcome.error,
                Some(PhaseNextError::MaxRetriesExceeded {
                    phase: "dev-design".to_owned(),
                    round: u32::from(MAX_RETRY_TIMES) + 1,
                }),
                "第 {MAX_RETRY_TIMES} 条 fail 恰达上限"
            );
            assert_eq!(outcome.next_phase, None, "上限响应不下发相位");
            assert!(!outcome.done, "上限非终态（决策分叉待定）");
            assert!(outcome.executor.is_none() && outcome.evaluator.is_none());
        }
    }
}

/// 全部 pass 终态：表内全相位 pass → done=true、next_phase=None、双 prompt
/// 缺席、白名单空（walker 收敛 completed 的判定输入）。
#[test]
fn 全部pass终态done收敛() {
    let phases = [
        "proposal",
        "dev-design",
        "test-design",
        "implement",
        "test-gen",
        "test-execution",
        "code-review",
        "acceptance",
    ];
    let entries = phases
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
    assert_eq!(
        outcome.allowed_backtrack_phases,
        vec!["proposal".to_owned(), "dev-design".to_owned()],
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
    assert_eq!(
        outcome.allowed_backtrack_phases,
        vec!["proposal".to_owned(), "dev-design".to_owned()],
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
    let fake = RouteStore::seeded(vec![
        entry("proposal", 1, Verdict::Pass, t(1)),
        entry("dev-design", 1, Verdict::Fail, t(2)),
        entry("dev-design", 2, Verdict::Fail, t(3)),
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
        "窗口外 2 条历史 fail 不计入重试窗口（不虚触上限）"
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
    let fake_b = RouteStore::seeded_b(vec![
        entry_of(CHANGE_ID_B, "proposal", 1, Verdict::Pass, t(1)),
        entry_of(CHANGE_ID_B, "dev-design", 1, Verdict::Fail, t(2)),
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
    // 复核条目内容未被改写（stale / backtrack 位不被动）
    let entries = fake
        .list_phase_records(CHANGE_ID)
        .expect("假件读半边应可用");
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].stale);
    assert!(entries[0].backtrack_to.is_none());
}
