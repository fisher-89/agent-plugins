//! 事件投影单点的行为面：六变体白名单 / level 映射 / 主键铸造 / 相位表查表降级。
//!
//! 零 mock：`project` 是进程内纯函数，`now` 为显式入参（确定性时钟注入），相位
//! 表取真实 `workflow::write::phase_table`——MUST NOT 在测试里复制 `description`
//! 字面（复制会抹掉「相位表漂移」判据）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use notification::{Notification, NotificationKind, NotificationLevel};
use workflow::write::phase_table;

use crate::notify::{deliver, project, NotifyContext, RunNotifier};
use crate::state::{
    AskPayload, ChangeRunStatus, ChangeStepEdge, ChangeStepKind, ChangeStepState, ChangeStepStatus,
    EdgeKind, NodeKey, RunUpdate,
};

const NOW: i64 = 1_727_000_000_123;

/// 投影上下文 fixture（五个值全部由发起侧已读出的记录携带）。
fn context() -> NotifyContext {
    NotifyContext {
        root: "/ws/alpha".to_owned(),
        change_id: "add-feature".to_owned(),
        change_name: "新增特性".to_owned(),
        run_id: "run-1".to_owned(),
        workflow_type: "requirement".to_owned(),
    }
}

fn ask(question: &str, options: Vec<String>, tool_call_id: Option<String>) -> RunUpdate {
    RunUpdate::Ask {
        ask: AskPayload {
            question: question.to_owned(),
            options,
            tool_call_id,
            session_id: None,
        },
    }
}

fn confirm_wait(phase: &str) -> RunUpdate {
    RunUpdate::ConfirmWait {
        phase: phase.to_owned(),
        archive: None,
    }
}

fn finished(status: ChangeRunStatus, reason: Option<&str>) -> RunUpdate {
    RunUpdate::Finished {
        status,
        reason: reason.map(str::to_owned),
    }
}

fn step_state() -> ChangeStepState {
    ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 0,
        step: ChangeStepKind::Executor,
        instance_no: 0,
        status: ChangeStepStatus::Running,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: 0,
        ended_at: None,
        token_input: None,
        token_output: None,
    }
}

fn node_key(phase: &str, instance_no: u32) -> NodeKey {
    (phase.to_owned(), 0, ChangeStepKind::Executor, instance_no)
}

fn session_event() -> RunUpdate {
    RunUpdate::SessionEvent {
        session_id: "ses-1".to_owned(),
        event: agent::AgentEvent::stamp(
            None,
            0,
            agent::AgentEventKind::Raw {
                event_type: "x".to_owned(),
                raw_json: "{}".to_owned(),
            },
        ),
    }
}

/// 相位表内某相位的人话标签（唯一来源＝相位表，测试只查表不复制字面）。
fn description_of(workflow_type: &str, phase: &str) -> String {
    phase_table(workflow_type)
        .expect("fixture 工作流类型应在表内")
        .iter()
        .find(|definition| definition.id == phase)
        .unwrap_or_else(|| panic!("fixture 相位应在 {workflow_type} 表内：{phase}"))
        .description
        .to_owned()
}

// ---------------------------------------------------------------------------
// 白名单正向：三变体各产出 Some 且三条恒定值收口
// ---------------------------------------------------------------------------

#[test]
fn project三纳入变体均返回some且kind与归属恒定() {
    let ctx = context();
    let updates = [
        ask("要选哪个？", Vec::new(), Some("tool-1".to_owned())),
        confirm_wait("dev-design"),
        finished(ChangeRunStatus::Completed, None),
    ];

    for update in updates {
        let notification = project(&update, &ctx, NOW)
            .unwrap_or_else(|| panic!("纳入变体应产出通知：{}", update.kind()));
        assert_eq!(notification.kind, NotificationKind::Event);
        assert_eq!(notification.root, Some(ctx.root.clone()));
        assert_eq!(notification.source_ref, Some(ctx.change_id.clone()));
        assert_eq!(notification.created_at, NOW);
    }
}

#[test]
fn project_ask映射warning与question副文案与工具调用主键() {
    let ctx = context();
    let update = ask(
        "改哪一层？",
        vec!["前端".to_owned(), "后端".to_owned()],
        Some("tool-call-7".to_owned()),
    );

    let notification = project(&update, &ctx, NOW).expect("ask 应产出通知");

    assert_eq!(notification.level, NotificationLevel::Warning);
    assert!(
        notification.message.contains(&ctx.change_name),
        "文案必含变更名，实际：{}",
        notification.message
    );
    assert_eq!(notification.body.as_deref(), Some("改哪一层？"));
    assert_eq!(notification.id, "run:run-1:ask:tool-call-7");
    assert_eq!(notification.created_at, NOW);
}

#[test]
fn project_confirmwait的body取相位表description且message含变更名与相位id() {
    let ctx = context();
    let update = confirm_wait("dev-design");

    let notification = project(&update, &ctx, NOW).expect("confirmWait 应产出通知");

    assert_eq!(notification.level, NotificationLevel::Warning);
    assert!(
        notification.message.contains(&ctx.change_name),
        "文案必含变更名，实际：{}",
        notification.message
    );
    assert!(
        notification.message.contains("dev-design"),
        "文案含相位 id，实际：{}",
        notification.message
    );
    assert_eq!(
        notification.body.as_deref(),
        Some(description_of("requirement", "dev-design").as_str()),
        "副文案逐字等于相位表 description（不另立翻译表）"
    );
    assert_eq!(notification.id, "run:run-1:confirm:dev-design");
}

#[test]
fn project_finished三终态level与body映射且主键同式() {
    let ctx = context();
    let cases = [
        (ChangeRunStatus::Completed, NotificationLevel::Success, None),
        (
            ChangeRunStatus::Failed,
            NotificationLevel::Error,
            Some("写面失败：static-check 未过"),
        ),
        (ChangeRunStatus::Stopped, NotificationLevel::Info, None),
    ];

    for (status, level, reason) in cases {
        let notification = project(&finished(status, reason), &ctx, NOW)
            .unwrap_or_else(|| panic!("终态应产出通知：{status:?}"));
        assert_eq!(notification.level, level, "level 映射：{status:?}");
        assert_eq!(
            notification.body.as_deref(),
            reason,
            "reason 映射：{status:?}"
        );
        assert_eq!(notification.id, "run:run-1:finished", "三终态主键同式");
    }
}

// ---------------------------------------------------------------------------
// 三变体排除 + 非终态 Finished
// ---------------------------------------------------------------------------

#[test]
fn project三排除变体均返回none() {
    let ctx = context();
    let updates = [
        RunUpdate::Step { step: step_state() },
        RunUpdate::Edge {
            edge: ChangeStepEdge {
                from: node_key("implement", 0),
                to: node_key("implement", 1),
                kind: EdgeKind::Forward,
            },
        },
        session_event(),
    ];

    for update in updates {
        assert!(
            project(&update, &ctx, NOW).is_none(),
            "运行期信号零通知：{}",
            update.kind()
        );
    }
}

#[test]
fn project_finished非终态status时防御性返回none() {
    let ctx = context();
    for status in [
        ChangeRunStatus::Running,
        ChangeRunStatus::WaitingConfirm,
        ChangeRunStatus::WaitingAsk,
    ] {
        assert!(
            project(&finished(status, None), &ctx, NOW).is_none(),
            "非终态 Finished 不产通知：{status:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 相位表降级
// ---------------------------------------------------------------------------

#[test]
fn project相位表降级时body为none且仍产出通知() {
    let cases = [
        (
            NotifyContext {
                workflow_type: "nope".to_owned(),
                ..context()
            },
            "dev-design",
            "未知工作流类型",
        ),
        (
            NotifyContext {
                workflow_type: String::new(),
                ..context()
            },
            "dev-design",
            "空工作流类型",
        ),
        (context(), "archive-x", "相位不在表内"),
    ];

    for (ctx, phase, label) in cases {
        let notification = project(&confirm_wait(phase), &ctx, NOW)
            .unwrap_or_else(|| panic!("{label}：仍应产出通知（不 panic）"));
        assert_eq!(notification.body, None, "{label}：查不到即 None");
        assert!(
            notification.message.contains(phase),
            "{label}：相位 id 仍进文案"
        );
    }
}

// ---------------------------------------------------------------------------
// ask 主键与多轮 ask
// ---------------------------------------------------------------------------

#[test]
fn project_ask主键在工具调用缺位时退化为发起时刻() {
    let ctx = context();
    let notification =
        project(&ask("要选哪个？", Vec::new(), None), &ctx, NOW).expect("ask 应产出通知");
    assert_eq!(
        notification.id,
        format!("run:run-1:ask:{NOW}"),
        "退化主键与 created_at 同源"
    );
    assert_eq!(notification.created_at, NOW);
}

#[test]
fn project同run两次ask的主键互不相同且选项串不进body() {
    let ctx = context();
    let first = project(
        &ask("第一轮", vec!["A".to_owned()], Some("tool-A".to_owned())),
        &ctx,
        NOW,
    )
    .expect("第一轮 ask 应产出通知");
    let second = project(
        &ask(
            "第二轮",
            vec!["B".to_owned(), "C".to_owned()],
            Some("tool-B".to_owned()),
        ),
        &ctx,
        NOW,
    )
    .expect("第二轮 ask 应产出通知");

    assert_ne!(first.id, second.id, "两主键逐字不同（不互相覆盖）");
    assert_eq!(
        second.body.as_deref(),
        Some("第二轮"),
        "body 逐字等于 question"
    );
    let body = second.body.expect("body 在场");
    assert!(!body.contains('B'), "选项串零出现：{body}");
}

// ---------------------------------------------------------------------------
// 空值与极端值
// ---------------------------------------------------------------------------

#[test]
fn project空值上下文字段仍产出通知且串逐字拼接() {
    let ctx = NotifyContext {
        root: String::new(),
        change_id: String::new(),
        change_name: String::new(),
        run_id: String::new(),
        workflow_type: "requirement".to_owned(),
    };

    let ask_notification =
        project(&ask("空值问句", Vec::new(), None), &ctx, NOW).expect("零值上下文仍产出");
    assert_eq!(
        ask_notification.id,
        format!("run::ask:{NOW}"),
        "空 run_id 逐字拼接"
    );
    assert_eq!(ask_notification.root, Some(String::new()), "空 root 原样");

    let finished_notification =
        project(&finished(ChangeRunStatus::Completed, None), &ctx, NOW).expect("终态仍产出");
    assert_eq!(finished_notification.id, "run::finished");

    let empty_phase = project(&confirm_wait(""), &ctx, NOW).expect("空相位仍产出");
    assert_eq!(empty_phase.id, "run::confirm:", "空相位 id 逐字拼接");
    assert_eq!(empty_phase.body, None, "空相位不在表内即 None");
}

#[test]
fn project超长与特殊字符的变更名原样嵌入文案() {
    let long_name = "名".repeat(1200);
    let ctx = NotifyContext {
        change_name: long_name.clone(),
        ..context()
    };

    let notification =
        project(&finished(ChangeRunStatus::Completed, None), &ctx, NOW).expect("终态应产出通知");
    assert!(
        notification.message.contains(&long_name),
        "超长变更名原样嵌入（零截断）"
    );

    let special = "变更\n含换行 🎉 与引号\"".to_owned();
    let ctx = NotifyContext {
        change_name: special.clone(),
        ..context()
    };
    let notification = project(&confirm_wait("dev-design"), &ctx, NOW).expect("停等应产出通知");
    assert!(
        notification.message.contains(&special),
        "特殊字符原样嵌入（零转义）"
    );
}

// ---------------------------------------------------------------------------
// 确定性
// ---------------------------------------------------------------------------

#[test]
fn project同一输入两次调用结果相等且now只改时间与退化主键() {
    let ctx = context();
    let update = finished(ChangeRunStatus::Failed, Some("引擎失败"));

    let first = project(&update, &ctx, NOW).expect("终态应产出通知");
    let again = project(&update, &ctx, NOW).expect("终态应产出通知");
    assert_eq!(first, again, "同一 (update, ctx, now) 结果相等");

    let later = project(&update, &ctx, NOW + 1).expect("终态应产出通知");
    assert_eq!(later.created_at, NOW + 1, "now 只改 created_at");
    assert_eq!(
        Notification {
            created_at: 0,
            ..first
        },
        Notification {
            created_at: 0,
            ..later.clone()
        },
        "除 created_at 外其余字段不变（终态主键不含 now）"
    );
    assert_eq!(later.id, "run:run-1:finished", "终态主键与 now 无关");

    // ask 的退化主键随 now 变化（与 created_at 同源）
    let ask_at_now = project(&ask("问", Vec::new(), None), &ctx, NOW).expect("ask 应产出通知");
    let ask_at_later =
        project(&ask("问", Vec::new(), None), &ctx, NOW + 5).expect("ask 应产出通知");
    assert_ne!(ask_at_now.id, ask_at_later.id, "退化主键随 now 变化");
}

// ---------------------------------------------------------------------------
// 五字段消费面（NotifyContext）
// ---------------------------------------------------------------------------

#[test]
fn project五字段各自进对应输出位且零额外查询() {
    let base = context();
    let update = confirm_wait("dev-design");
    let baseline = project(&update, &base, NOW).expect("停等应产出通知");

    let other_root = NotifyContext {
        root: "/ws/beta".to_owned(),
        ..context()
    };
    let by_root = project(&update, &other_root, NOW).expect("停等应产出通知");
    assert_eq!(by_root.root, Some("/ws/beta".to_owned()), "root → root");
    assert_eq!(by_root.source_ref, baseline.source_ref, "其余字段不变");

    let other_id = NotifyContext {
        change_id: "fix-bug".to_owned(),
        ..context()
    };
    let by_change_id = project(&update, &other_id, NOW).expect("停等应产出通知");
    assert_eq!(
        by_change_id.source_ref,
        Some("fix-bug".to_owned()),
        "change_id → source_ref"
    );

    let other_name = NotifyContext {
        change_name: "修一个缺陷".to_owned(),
        ..context()
    };
    let by_name = project(&update, &other_name, NOW).expect("停等应产出通知");
    assert_ne!(by_name.message, baseline.message, "change_name → message");

    let other_run = NotifyContext {
        run_id: "run-2".to_owned(),
        ..context()
    };
    let by_run = project(&update, &other_run, NOW).expect("停等应产出通知");
    assert_eq!(by_run.id, "run:run-2:confirm:dev-design", "run_id → 主键");

    // workflow_type 决定相位表归属：`code-analyze` 只在 test-only 表内
    let test_only = NotifyContext {
        workflow_type: "test-only".to_owned(),
        ..context()
    };
    let in_test_only =
        project(&confirm_wait("code-analyze"), &test_only, NOW).expect("停等应产出通知");
    assert_eq!(
        in_test_only.body.as_deref(),
        Some(description_of("test-only", "code-analyze").as_str()),
        "workflow_type 选表 → body"
    );
    let in_requirement =
        project(&confirm_wait("code-analyze"), &base, NOW).expect("停等应产出通知");
    assert_eq!(
        in_requirement.body, None,
        "requirement 表无该相位（查表降级）"
    );
}

// ---------------------------------------------------------------------------
// 归档前置载荷不进通知
// ---------------------------------------------------------------------------

#[test]
fn project_confirmwait的archive载荷不影响投影结果() {
    let ctx = context();
    let without_archive = project(&confirm_wait("archive"), &ctx, NOW).expect("停等应产出通知");
    let with_archive = project(
        &RunUpdate::ConfirmWait {
            phase: "archive".to_owned(),
            archive: Some(crate::state::ArchivePreflight {
                name: "add-feature".to_owned(),
                completed: true,
                incomplete_phases: vec!["acceptance".to_owned()],
                missing_artifacts: vec!["specs/x/spec.md".to_owned()],
                delta_specs: vec!["desktop-notification-hub".to_owned()],
                worktree: Some("/ws/wt".to_owned()),
                exec_root: None,
                branch: Some("change/add-feature".to_owned()),
                merge_target: Some("main".to_owned()),
            }),
        },
        &ctx,
        NOW,
    )
    .expect("停等应产出通知");

    assert_eq!(with_archive, without_archive, "归档前置数据不进通知");
}

// ---------------------------------------------------------------------------
// `deliver`：投影 → 投递的唯一组合点
// ---------------------------------------------------------------------------

/// 替身调用记录（顺序敏感：`deliver` 必先于 `retire`）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum NotifierCall {
    Deliver(Notification),
    Retire(String),
}

/// 替身共享日志：调用序 / `context` 读取次数 / 失败自留痕（旁路语义的观测面）。
#[derive(Default)]
struct NotifierLog {
    context_reads: AtomicUsize,
    calls: Mutex<Vec<NotifierCall>>,
    swallowed: Mutex<Vec<String>>,
}

impl NotifierLog {
    fn calls(&self) -> Vec<NotifierCall> {
        self.calls.lock().expect("调用日志锁不可中毒").clone()
    }

    fn delivers(&self) -> Vec<Notification> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                NotifierCall::Deliver(notification) => Some(notification),
                NotifierCall::Retire(_) => None,
            })
            .collect()
    }

    fn retires(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                NotifierCall::Deliver(_) => None,
                NotifierCall::Retire(run_id) => Some(run_id),
            })
            .collect()
    }

    fn context_reads(&self) -> usize {
        self.context_reads.load(Ordering::SeqCst)
    }

    fn swallowed(&self) -> Vec<String> {
        self.swallowed.lock().expect("失败留痕锁不可中毒").clone()
    }
}

/// 记录型替身（`Send + Sync`，`context()` 返回真实上下文）。
struct RecordingNotifier {
    context: NotifyContext,
    log: Arc<NotifierLog>,
}

impl RecordingNotifier {
    fn new(context: NotifyContext) -> (Arc<Self>, Arc<NotifierLog>) {
        let log = Arc::new(NotifierLog::default());
        let notifier = Arc::new(Self {
            context,
            log: Arc::clone(&log),
        });
        (notifier, log)
    }
}

impl RunNotifier for RecordingNotifier {
    fn context(&self) -> &NotifyContext {
        self.log.context_reads.fetch_add(1, Ordering::SeqCst);
        &self.context
    }

    fn deliver(&self, notification: &Notification) {
        self.log
            .calls
            .lock()
            .expect("调用日志锁不可中毒")
            .push(NotifierCall::Deliver(notification.clone()));
    }

    fn retire(&self) {
        self.log
            .calls
            .lock()
            .expect("调用日志锁不可中毒")
            .push(NotifierCall::Retire(self.context.run_id.clone()));
    }
}

/// 失败收场型替身：投递期失败只在自身留痕、不 panic、不上抛（旁路语义）。
struct SwallowingNotifier {
    context: NotifyContext,
    log: Arc<NotifierLog>,
    failure: String,
}

impl SwallowingNotifier {
    fn new(context: NotifyContext, failure: &str) -> (Arc<Self>, Arc<NotifierLog>) {
        let log = Arc::new(NotifierLog::default());
        let notifier = Arc::new(Self {
            context,
            log: Arc::clone(&log),
            failure: failure.to_owned(),
        });
        (notifier, log)
    }
}

impl RunNotifier for SwallowingNotifier {
    fn context(&self) -> &NotifyContext {
        self.log.context_reads.fetch_add(1, Ordering::SeqCst);
        &self.context
    }

    fn deliver(&self, notification: &Notification) {
        self.log
            .swallowed
            .lock()
            .expect("失败留痕锁不可中毒")
            .push(format!("{}:{}", notification.id, self.failure));
        self.log
            .calls
            .lock()
            .expect("调用日志锁不可中毒")
            .push(NotifierCall::Deliver(notification.clone()));
    }

    fn retire(&self) {
        self.log
            .swallowed
            .lock()
            .expect("失败留痕锁不可中毒")
            .push(format!("retire:{}", self.failure));
        self.log
            .calls
            .lock()
            .expect("调用日志锁不可中毒")
            .push(NotifierCall::Retire(self.context.run_id.clone()));
    }
}

/// 三排除变体（运行期信号）——白名单与终态清理的零命中面共用。
fn excluded_updates() -> [RunUpdate; 3] {
    [
        RunUpdate::Step { step: step_state() },
        RunUpdate::Edge {
            edge: ChangeStepEdge {
                from: node_key("implement", 0),
                to: node_key("implement", 1),
                kind: EdgeKind::Forward,
            },
        },
        session_event(),
    ]
}

#[test]
fn deliver三纳入变体各投递一次且载荷与project直调相等() {
    let ctx = context();
    for update in [
        ask("要选哪个？", Vec::new(), Some("tool-1".to_owned())),
        confirm_wait("dev-design"),
        finished(ChangeRunStatus::Completed, None),
    ] {
        let (notifier, log) = RecordingNotifier::new(context());
        deliver(&update, notifier.as_ref(), NOW);

        let delivered = log.delivers();
        assert_eq!(
            delivered.len(),
            1,
            "纳入变体每次恰投递一条：{}",
            update.kind()
        );
        assert_eq!(
            delivered[0],
            project(&update, &ctx, NOW).expect("纳入变体应产出通知"),
            "组合点不加料、不改写、不二次铸造主键"
        );
    }
}

#[test]
fn deliver_finished三终态各投递一次并清理一次且序为投递先清理后() {
    let ctx = context();
    for (status, reason) in [
        (ChangeRunStatus::Completed, None),
        (ChangeRunStatus::Failed, Some("引擎失败")),
        (ChangeRunStatus::Stopped, None),
    ] {
        let update = finished(status, reason);
        let (notifier, log) = RecordingNotifier::new(context());
        deliver(&update, notifier.as_ref(), NOW);

        assert_eq!(
            log.calls(),
            vec![
                NotifierCall::Deliver(project(&update, &ctx, NOW).expect("终态应产出通知")),
                NotifierCall::Retire(ctx.run_id.clone()),
            ],
            "终态同一次投递内序为 deliver 先、retire 后（{status:?}）"
        );
    }
}

#[test]
fn deliver三排除变体零投递零清理且context各读一次() {
    for update in excluded_updates() {
        let (notifier, log) = RecordingNotifier::new(context());
        deliver(&update, notifier.as_ref(), NOW);

        assert!(
            log.calls().is_empty(),
            "运行期信号零投递零清理：{}",
            update.kind()
        );
        assert_eq!(
            log.context_reads(),
            1,
            "投影本身需要上下文（各读恰一次）：{}",
            update.kind()
        );
    }
}

#[test]
fn deliver同一更新连续两次不去重且finished两次清理两次() {
    let ctx = context();
    let update = confirm_wait("archive");
    let (notifier, log) = RecordingNotifier::new(context());
    deliver(&update, notifier.as_ref(), NOW);
    deliver(&update, notifier.as_ref(), NOW);
    assert_eq!(log.delivers().len(), 2, "组合点不缓存、不去重");

    let terminal = finished(ChangeRunStatus::Completed, None);
    let (terminal_notifier, terminal_log) = RecordingNotifier::new(context());
    deliver(&terminal, terminal_notifier.as_ref(), NOW);
    deliver(&terminal, terminal_notifier.as_ref(), NOW);
    assert_eq!(
        terminal_log.retires(),
        vec![ctx.run_id.clone(), ctx.run_id.clone()],
        "接口不做幂等裁剪（幂等由 retire_run_notifications 承担）"
    );
}

#[test]
fn deliver_now三态原样入载荷与ask退化主键() {
    for now in [0_i64, -1, i64::MAX] {
        let (notifier, log) = RecordingNotifier::new(context());
        deliver(&ask("问", Vec::new(), None), notifier.as_ref(), now);
        let degenerate = &log.delivers()[0];
        assert_eq!(degenerate.created_at, now, "created_at 不钳位：{now}");
        assert_eq!(
            degenerate.id,
            format!("run:run-1:ask:{now}"),
            "退化主键与 created_at 同源：{now}"
        );

        let (token_notifier, token_log) = RecordingNotifier::new(context());
        deliver(
            &ask("问", Vec::new(), Some("tool-9".to_owned())),
            token_notifier.as_ref(),
            now,
        );
        assert_eq!(
            token_log.delivers()[0].created_at,
            now,
            "组合点不钳位时间：{now}"
        );
    }
}

#[test]
fn deliver旁路失败吞错时零panic且后续清理仍发生() {
    let ctx = context();
    let (notifier, log) = SwallowingNotifier::new(context(), "落库失败");
    deliver(&confirm_wait("archive"), notifier.as_ref(), NOW);
    deliver(
        &finished(ChangeRunStatus::Completed, None),
        notifier.as_ref(),
        NOW,
    );

    assert_eq!(
        log.calls(),
        vec![
            NotifierCall::Deliver(project(&confirm_wait("archive"), &ctx, NOW).expect("停等产出")),
            NotifierCall::Deliver(
                project(&finished(ChangeRunStatus::Completed, None), &ctx, NOW).expect("终态产出")
            ),
            NotifierCall::Retire(ctx.run_id.clone()),
        ],
        "失败由实现自降级：组合点照常推进到终态清理（零 catch_unwind）"
    );
    assert_eq!(log.swallowed().len(), 3, "实现内部留痕（失败不上抛）");
}

// ---------------------------------------------------------------------------
// `RunNotifier` 出口面
// ---------------------------------------------------------------------------

#[test]
fn notifier_context在每个some变体经deliver读取恰一次且五字段逐项进载荷() {
    for update in [
        ask("问", Vec::new(), Some("tool-1".to_owned())),
        confirm_wait("dev-design"),
        finished(ChangeRunStatus::Completed, None),
    ] {
        let (notifier, log) = RecordingNotifier::new(context());
        deliver(&update, notifier.as_ref(), NOW);
        assert_eq!(
            log.context_reads(),
            1,
            "每个纳入变体恰读一次上下文：{}",
            update.kind()
        );
    }

    // 五字段逐项进投递载荷（自持上下文由 deliver 消费，投影零额外查询）
    let ctx = NotifyContext {
        root: "/ws/gamma".to_owned(),
        change_id: "wire-notify".to_owned(),
        change_name: "接线变更".to_owned(),
        run_id: "run-wire".to_owned(),
        workflow_type: "test-only".to_owned(),
    };
    let (notifier, log) = RecordingNotifier::new(ctx.clone());
    deliver(&confirm_wait("code-analyze"), notifier.as_ref(), NOW);
    let payload = &log.delivers()[0];
    assert_eq!(payload.root, Some(ctx.root.clone()), "root → root");
    assert_eq!(
        payload.source_ref,
        Some(ctx.change_id.clone()),
        "change_id → source_ref"
    );
    assert!(
        payload.message.contains(&ctx.change_name),
        "change_name → message"
    );
    assert_eq!(
        payload.id, "run:run-wire:confirm:code-analyze",
        "run_id → 主键"
    );
    assert_eq!(
        payload.body.as_deref(),
        Some(description_of("test-only", "code-analyze").as_str()),
        "workflow_type → ConfirmWait 的 body"
    );
}

#[test]
fn notifier_deliver无返回值且send_sync可跨线程共享() {
    let (notifier, log) = RecordingNotifier::new(context());
    let erasure: Arc<dyn RunNotifier> = Arc::clone(&notifier) as Arc<dyn RunNotifier>;
    let notification =
        project(&finished(ChangeRunStatus::Completed, None), &context(), NOW).expect("终态产出");

    let _: () = erasure.deliver(&notification);

    let shared = Arc::clone(&erasure);
    let payload = notification.clone();
    std::thread::spawn(move || shared.deliver(&payload))
        .join()
        .expect("Send + Sync 对象安全：跨线程投递应成功");

    assert_eq!(log.delivers().len(), 2, "两次投递各记一条");
}

#[test]
fn notifier_deliver两次记录序为调用序且不假设无状态() {
    let (notifier, log) = RecordingNotifier::new(context());
    let first = project(&confirm_wait("archive"), &context(), NOW).expect("停等产出");
    let second = project(&confirm_wait("dev-design"), &context(), NOW).expect("停等产出");

    notifier.deliver(&first);
    notifier.deliver(&second);

    assert_eq!(
        log.delivers(),
        vec![first, second],
        "记录序 = 调用序（接口不假设单次投递）"
    );
}

#[test]
fn notifier_retire取上下文run_id且与finished载荷同源() {
    let ctx = context();
    let (notifier, log) = RecordingNotifier::new(context());
    let update = finished(ChangeRunStatus::Failed, Some("引擎失败"));
    deliver(&update, notifier.as_ref(), NOW);

    let delivered = log.delivers();
    assert_eq!(delivered.len(), 1, "deliver 恰 1 次");
    assert_eq!(
        delivered[0].id,
        format!("run:{}:finished", ctx.run_id),
        "终态载荷的主键含上下文 run_id"
    );
    assert_eq!(
        log.retires(),
        vec![ctx.run_id.clone()],
        "retire 的 run_id 取自 context()（不由入参传）"
    );
}

#[test]
fn notifier_retire非终态零调用且终态重复两轮各一次() {
    let mut updates = vec![
        ask("问", Vec::new(), Some("tool-1".to_owned())),
        confirm_wait("dev-design"),
    ];
    updates.extend(excluded_updates());
    for update in updates {
        let (notifier, log) = RecordingNotifier::new(context());
        deliver(&update, notifier.as_ref(), NOW);
        assert!(log.retires().is_empty(), "非终态零清理：{}", update.kind());
    }

    let (notifier, log) = RecordingNotifier::new(context());
    for _ in 0..2 {
        deliver(
            &finished(ChangeRunStatus::Stopped, None),
            notifier.as_ref(),
            NOW,
        );
    }
    assert_eq!(log.retires().len(), 2, "终态重复两轮即 2 次（接口不裁剪）");
}
