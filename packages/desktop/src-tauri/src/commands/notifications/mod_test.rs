//! notifications 命令轨道的装配面与五命令行为面：扇出顺序（先落库后广播）/
//! 订阅四步顺序 / 收件箱读写口径 / 反馈落库的空白校验与不广播。
//!
//! 进程边界用真实 `tauri::test::mock_app()` + 真实 tempdir `WorkspaceStores`；
//! `Channel<Notification>` 用真实 `tauri::ipc::Channel::new` 捕获出线 JSON（不 mock
//! 转发任务）；广播等待取「最终态」轮询，不用固定 sleep 断言。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ::notification::{Notification, NotificationFeedbackInput, NotificationLevel};
use store::{Store, WorkspaceStores};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, AppHandle, Manager};

use super::{
    notification_list_with, notification_mark_all_read_with, notification_mark_read_with,
    notification_record_with, notification_subscribe_with, NotificationBus,
};

// ---------------------------------------------------------------------------
// 装置：tempdir 数据根 + mock app 托管两态
// ---------------------------------------------------------------------------

struct Env {
    data_dir: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("notifications-cmd-test-{tag}-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        Self { data_dir }
    }

    /// workspace 维度库实例（跨维度故障注入面：收件箱只在全局库）。
    fn workspace(&self) -> Arc<Store> {
        Arc::new(
            Store::open_workspace(&self.data_dir.path().join("ws.redb"))
                .expect("open_workspace 应成功"),
        )
    }
}

/// 以 MockRuntime 建测用 app 并托管 `Arc<NotificationBus>`（数据根开全局库，
/// 与 `src/main.rs` 同序同构）。
fn app_with(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试数据根失败");
    let global = stores.global_shared();
    app.manage(stores);
    app.manage(Arc::new(NotificationBus::new(global)));
    app
}

/// 以显式 store（可为 workspace 维度）构造 bus 并托管的测用 app（故障注入）。
fn app_with_store(store: Arc<Store>) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    app.manage(Arc::new(NotificationBus::new(store)));
    app
}

fn handle_of(app: &App<tauri::test::MockRuntime>) -> AppHandle<tauri::test::MockRuntime> {
    app.handle().clone()
}

fn bus_of(app: &App<tauri::test::MockRuntime>) -> Arc<NotificationBus> {
    Arc::clone(app.state::<Arc<NotificationBus>>().inner())
}

fn global_store(app: &App<tauri::test::MockRuntime>) -> Arc<Store> {
    app.state::<WorkspaceStores>().global_shared()
}

/// 丢弃型 Channel（发送即弃）。
fn discarding_channel() -> Channel<Notification> {
    Channel::new(|_: InvokeResponseBody| Ok(()))
}

/// 捕获型 Channel：逐条解析出线 JSON（IPC 边界捕获），共享缓冲供断言。
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<Notification>>>);

impl Captured {
    fn new() -> Self {
        Self::default()
    }

    fn channel(&self) -> Channel<Notification> {
        let sink = Arc::clone(&self.0);
        Channel::new(move |body: InvokeResponseBody| {
            if let InvokeResponseBody::Json(text) = body {
                if let Ok(notification) = serde_json::from_str::<Notification>(&text) {
                    sink.lock().expect("捕获锁不可中毒").push(notification);
                }
            }
            Ok(())
        })
    }

    fn items(&self) -> Vec<Notification> {
        self.0.lock().expect("捕获锁不可中毒").clone()
    }

    fn len(&self) -> usize {
        self.0.lock().expect("捕获锁不可中毒").len()
    }
}

/// 轮询至谓词成立（后台转发任务的观测窗口；10s 上限防挂死）。
fn wait_for(what: &str, mut probe: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !probe() {
        assert!(Instant::now() < deadline, "等待{what}超时");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 事件型通知 fixture。
fn event(id: &str, message: &str) -> Notification {
    Notification {
        id: id.to_owned(),
        kind: ::notification::NotificationKind::Event,
        level: NotificationLevel::Warning,
        message: message.to_owned(),
        body: Some("设计相位产出。".to_owned()),
        root: Some("/ws/alpha".to_owned()),
        source_ref: Some("add-feature".to_owned()),
        created_at: 1_727_000_000_000,
    }
}

/// 反馈入参 fixture。
fn feedback_input(id: &str) -> NotificationFeedbackInput {
    NotificationFeedbackInput {
        id: id.to_owned(),
        level: NotificationLevel::Error,
        message: "保存 agent 配置失败".to_owned(),
        body: None,
    }
}

/// 落库 N 条未读事件（`run:r{n}:finished`）。
fn seed_events(store: &Store, count: usize) {
    for index in 0..count {
        store
            .publish_notification(&event(
                &format!("run:r{index}:finished"),
                &format!("变更「add-feature」已完成全部相位（{index}）"),
            ))
            .expect("落库应成功");
    }
}

// ---------------------------------------------------------------------------
// NotificationBus：装配序（先落库、后广播）
// ---------------------------------------------------------------------------

#[test]
fn bus_publish先落库再广播且两sink各恰一次() {
    let env = Env::new("bus-order");
    let app = app_with(&env);
    let captured = Captured::new();

    let initial =
        notification_subscribe_with(handle_of(&app), captured.channel()).expect("订阅应成功");
    assert_eq!(initial, 0, "空库初值 0");

    bus_of(&app).publish(&event(
        "run:r1:finished",
        "变更「add-feature」已完成全部相位",
    ));

    wait_for("Channel 收到一条", || captured.len() == 1);
    let page = notification_list_with(handle_of(&app), 0, 10).expect("列表应成功");
    assert_eq!(page.items.len(), 1, "落库 sink 恰一行");
    assert_eq!(
        captured.items()[0].id,
        "run:r1:finished",
        "in-app sink 恰一条"
    );
}

#[test]
fn bus_publish无订阅者时超容量连续投递零panic且落库全量() {
    let env = Env::new("bus-no-subscriber");
    let app = app_with(&env);
    let bus = bus_of(&app);

    for index in 0..200 {
        bus.publish(&event(
            &format!("run:r{index}:finished"),
            "变更「add-feature」已完成全部相位",
        ));
    }

    let page = notification_list_with(handle_of(&app), 0, 200).expect("列表应成功");
    assert_eq!(page.total, 200, "广播丢增量不落库");
    assert_eq!(page.items.len(), 200);
}

#[test]
fn bus_publish在workspace维度store上零落库但订阅端仍收到() {
    let env = Env::new("bus-workspace");
    let app = app_with_store(env.workspace());
    let bus = bus_of(&app);
    let captured = Captured::new();

    // 故障注入面：workspace 维度库缺收件箱表，落库 sink 必失败、订阅命令的未读
    // 计数也必 Err（同一维度判据）。故本行不经 `notification_subscribe_with`，
    // 直接取 bus 的 receiver + 生产转发任务，只观测 in-app sink 的投递面。
    super::spawn_notification_forward(captured.channel(), bus.subscribe());
    bus.publish(&event(
        "run:r1:finished",
        "变更「add-feature」已完成全部相位",
    ));

    wait_for("一 sink 失败不影响 in-app sink", || {
        captured.len() == 1
    });
    assert_eq!(captured.items()[0].id, "run:r1:finished");
    assert!(
        bus.store().list_notifications(0, 10).is_err(),
        "库面零行（跨维度即 Err，收件箱不可经 workspace 实例观测）"
    );
}

// ---------------------------------------------------------------------------
// NotificationBus::retire_run
// ---------------------------------------------------------------------------

#[test]
fn bus_retire_run只清同run的等待类未读() {
    let env = Env::new("retire");
    let app = app_with(&env);
    let store = global_store(&app);
    for id in [
        "run:r1:confirm:archive",
        "run:r1:ask:tool-A",
        "run:r1:finished",
        "run:r2:confirm:dev-design",
    ] {
        store
            .publish_notification(&event(id, "变更「add-feature」停在等你确认"))
            .expect("落库应成功");
    }
    store
        .publish_notification(&Notification {
            id: "uuid-1".to_owned(),
            kind: ::notification::NotificationKind::Feedback,
            level: NotificationLevel::Error,
            message: "保存失败".to_owned(),
            body: None,
            root: None,
            source_ref: None,
            created_at: 1,
        })
        .expect("落库应成功");

    bus_of(&app).retire_run("r1");

    assert_eq!(
        store.unread_notification_count().expect("计数应成功"),
        2,
        "只 retire 两条等待类（finished / r2 各剩一条未读）"
    );
}

#[test]
fn bus_retire_run对未知run与空run_id零写入() {
    let env = Env::new("retire-miss");
    let app = app_with(&env);
    let store = global_store(&app);
    seed_events(&store, 3);

    bus_of(&app).retire_run("run-does-not-exist");
    bus_of(&app).retire_run("");

    assert_eq!(
        store.unread_notification_count().expect("计数应成功"),
        3,
        "miss 即无操作（幂等）"
    );
}

#[test]
fn bus_retire_run在workspace维度store上零上抛() {
    let env = Env::new("retire-workspace");
    let app = app_with_store(env.workspace());

    // 失败降级为一行 eprintln!：不 panic、零上抛
    bus_of(&app).retire_run("r1");
}

// ---------------------------------------------------------------------------
// notification_subscribe_with：四步顺序与初值不丢
// ---------------------------------------------------------------------------

#[test]
fn subscribe返回当前未读且此后增量经通道不丢() {
    let env = Env::new("subscribe-initial");
    let app = app_with(&env);
    seed_events(&global_store(&app), 3);
    let captured = Captured::new();

    let initial =
        notification_subscribe_with(handle_of(&app), captured.channel()).expect("订阅应成功");
    assert_eq!(initial, 3, "返回当前未读数作徽标初值");

    bus_of(&app).publish(&event(
        "run:r9:ask:tool-Z",
        "变更「add-feature」在等你的回答",
    ));
    wait_for("增量到达", || captured.len() == 1);

    let unread = global_store(&app)
        .unread_notification_count()
        .expect("计数应成功");
    assert_eq!(
        initial as usize + captured.len(),
        unread as usize,
        "初值 + 收到条数 == 真实未读（不丢）"
    );
    assert_eq!(unread, 4);
}

#[test]
fn subscribe两次的初值差等于其间发布条数() {
    let env = Env::new("subscribe-delta");
    let app = app_with(&env);
    seed_events(&global_store(&app), 2);

    let first = notification_subscribe_with(handle_of(&app), discarding_channel())
        .expect("第一次订阅应成功");
    for index in 0..3 {
        bus_of(&app).publish(&event(
            &format!("run:r{index}:ask:A"),
            "变更「add-feature」在等你的回答",
        ));
    }
    let second = notification_subscribe_with(handle_of(&app), discarding_channel())
        .expect("第二次订阅应成功");

    assert_eq!(first, 2);
    assert_eq!(second, first + 3, "对照读数取再次订阅的初值");
}

#[test]
fn subscribe空库返回零且订阅前已落库的通知不补投() {
    let env = Env::new("subscribe-empty");
    let app = app_with(&env);
    let store = global_store(&app);
    let captured = Captured::new();

    let initial =
        notification_subscribe_with(handle_of(&app), captured.channel()).expect("订阅应成功");
    assert_eq!(initial, 0);

    seed_events(&store, 3);
    assert_eq!(captured.len(), 0, "订阅前已落库的通知不补投（补拉走列表）");
    let page = notification_list_with(handle_of(&app), 0, 10).expect("列表应成功");
    assert_eq!(page.total, 3, "权威读数在列表");
}

#[test]
fn subscribe超容量滞后后仍不断流() {
    let env = Env::new("subscribe-lag");
    let app = app_with(&env);
    let captured = Captured::new();
    notification_subscribe_with(handle_of(&app), captured.channel()).expect("订阅应成功");

    let bus = bus_of(&app);
    for index in 0..200 {
        bus.publish(&event(
            &format!("run:r{index}:finished"),
            "变更「add-feature」已完成全部相位",
        ));
    }
    wait_for("滞后后仍收到至少一条", || captured.len() >= 1);

    bus.publish(&event(
        "run:r-last:finished",
        "变更「add-feature」已完成全部相位",
    ));
    wait_for("滞后不断流（后续仍收到）", || {
        captured
            .items()
            .iter()
            .any(|notification| notification.id == "run:r-last:finished")
    });
}

#[test]
fn subscribe在workspace维度store上返回err不建半成品订阅() {
    let env = Env::new("subscribe-workspace");
    let app = app_with_store(env.workspace());
    let captured = Captured::new();

    let outcome = notification_subscribe_with(handle_of(&app), captured.channel());

    let error = outcome.expect_err("计数读取失败应整体 Err");
    assert!(!error.is_empty(), "错误映射为 String（单点 .to_string()）");
    assert_eq!(captured.len(), 0, "不返回半成品订阅");
}

// ---------------------------------------------------------------------------
// notification_list_with
// ---------------------------------------------------------------------------

#[test]
fn list逐值透传且只含event() {
    let env = Env::new("list");
    let app = app_with(&env);
    let store = global_store(&app);
    seed_events(&store, 3);
    store
        .publish_notification(&Notification {
            id: "uuid-1".to_owned(),
            kind: ::notification::NotificationKind::Feedback,
            level: NotificationLevel::Error,
            message: "保存失败".to_owned(),
            body: None,
            root: None,
            source_ref: None,
            created_at: 9,
        })
        .expect("落库应成功");

    let page = notification_list_with(handle_of(&app), 0, 10).expect("列表应成功");
    assert_eq!(page.items.len(), 3, "反馈不进列表");
    assert_eq!(page.total, 3, "total = Event 全量");
    assert!(
        page.items
            .iter()
            .all(|notification| notification.kind == ::notification::NotificationKind::Event),
        "只 kind = Event"
    );

    let narrowed = notification_list_with(handle_of(&app), 1, 1).expect("分页应成功");
    assert_eq!(narrowed.items.len(), 1, "limit 透传");
    assert_eq!(narrowed.total, 3, "total 不受页界影响");
}

#[test]
fn list对limit上界钳位与越界offset返回空页() {
    let env = Env::new("list-bounds");
    let app = app_with(&env);
    seed_events(&global_store(&app), 205);

    let clamped = notification_list_with(handle_of(&app), 0, 500).expect("列表应成功");
    assert_eq!(clamped.items.len(), 200, "IPC 滥用兜底：钳位 200");
    assert_eq!(clamped.total, 205);

    let beyond = notification_list_with(handle_of(&app), u32::MAX, 10).expect("列表应成功");
    assert!(beyond.items.is_empty(), "越界 offset 返回空页");
    assert_eq!(beyond.total, 205, "total 不变");
}

#[test]
fn list在workspace维度store上返回err() {
    let env = Env::new("list-workspace");
    let app = app_with_store(env.workspace());

    let outcome = notification_list_with(handle_of(&app), 0, 10);
    assert!(outcome.is_err(), "跨维度读应 Err");
}

// ---------------------------------------------------------------------------
// notification_mark_read_with
// ---------------------------------------------------------------------------

#[test]
fn mark_read返回剩余未读且幂等() {
    let env = Env::new("mark-read");
    let app = app_with(&env);
    seed_events(&global_store(&app), 3);
    let ids = vec!["run:r0:finished".to_owned()];

    let remaining =
        notification_mark_read_with(handle_of(&app), ids.clone()).expect("置已读应成功");
    assert_eq!(remaining, 2);

    let again = notification_mark_read_with(handle_of(&app), ids).expect("置已读应成功");
    assert_eq!(again, 2, "重复标记幂等");
}

#[test]
fn mark_read对空集合与未知id与反馈id零变化() {
    let env = Env::new("mark-read-edges");
    let app = app_with(&env);
    let store = global_store(&app);
    seed_events(&store, 2);
    store
        .publish_notification(&Notification {
            id: "uuid-1".to_owned(),
            kind: ::notification::NotificationKind::Feedback,
            level: NotificationLevel::Error,
            message: "保存失败".to_owned(),
            body: None,
            root: None,
            source_ref: None,
            created_at: 9,
        })
        .expect("落库应成功");

    assert_eq!(
        notification_mark_read_with(handle_of(&app), Vec::new()).expect("空集合应成功"),
        2,
        "空 ids 零变化、返回当前未读"
    );
    assert_eq!(
        notification_mark_read_with(handle_of(&app), vec!["run:missing".to_owned()])
            .expect("未知 id 应成功"),
        2,
        "未知 id 忽略"
    );
    assert_eq!(
        notification_mark_read_with(handle_of(&app), vec!["uuid-1".to_owned()])
            .expect("反馈 id 应成功"),
        2,
        "反馈行不计未读、标记亦不改未读"
    );
}

#[test]
fn mark_read在workspace维度store上返回err() {
    let env = Env::new("mark-read-workspace");
    let app = app_with_store(env.workspace());

    assert!(
        notification_mark_read_with(handle_of(&app), vec!["run:r0:finished".to_owned()]).is_err(),
        "跨维度写应 Err"
    );
}

// ---------------------------------------------------------------------------
// notification_mark_all_read_with
// ---------------------------------------------------------------------------

#[test]
fn mark_all_read返回零且含未加载页() {
    let env = Env::new("mark-all");
    let app = app_with(&env);
    seed_events(&global_store(&app), 12);

    assert_eq!(
        notification_mark_all_read_with(handle_of(&app)).expect("全部已读应成功"),
        0,
        "返回剩余未读（恒 0）"
    );

    let page = notification_list_with(handle_of(&app), 0, 5).expect("列表应成功");
    assert_eq!(page.items.len(), 5, "首屏一页");
    // 未加载页同样置已读：未读口径不再剩行
    assert_eq!(
        notification_mark_all_read_with(handle_of(&app)).expect("全部已读应成功"),
        0,
        "幂等（已读行不重复改写）"
    );
}

#[test]
fn mark_all_read对空库与反馈行零影响() {
    let env = Env::new("mark-all-empty");
    let app = app_with(&env);

    assert_eq!(
        notification_mark_all_read_with(handle_of(&app)).expect("空库应成功"),
        0
    );

    let store = global_store(&app);
    store
        .publish_notification(&Notification {
            id: "uuid-1".to_owned(),
            kind: ::notification::NotificationKind::Feedback,
            level: NotificationLevel::Error,
            message: "保存失败".to_owned(),
            body: None,
            root: None,
            source_ref: None,
            created_at: 9,
        })
        .expect("落库应成功");
    assert_eq!(
        notification_mark_all_read_with(handle_of(&app)).expect("仅反馈行应成功"),
        0
    );
    let page = notification_list_with(handle_of(&app), 0, 10).expect("列表应成功");
    assert!(page.items.is_empty(), "列表与未读数均不变");
}

#[test]
fn mark_all_read在workspace维度store上返回err() {
    let env = Env::new("mark-all-workspace");
    let app = app_with_store(env.workspace());

    assert!(notification_mark_all_read_with(handle_of(&app)).is_err());
}

// ---------------------------------------------------------------------------
// notification_record_with：反馈落库（不广播）
// ---------------------------------------------------------------------------

#[test]
fn record落库为反馈且不入列表不计未读不广播() {
    let env = Env::new("record");
    let app = app_with(&env);
    let captured = Captured::new();
    notification_subscribe_with(handle_of(&app), captured.channel()).expect("订阅应成功");

    notification_record_with(handle_of(&app), feedback_input("uuid-1")).expect("落库应成功");

    let store = global_store(&app);
    assert_eq!(
        store.unread_notification_count().expect("计数应成功"),
        0,
        "反馈不计未读"
    );
    let page = notification_list_with(handle_of(&app), 0, 10).expect("列表应成功");
    assert!(page.items.is_empty(), "反馈不进列表");

    // 反馈不入 Hub：直接核库中行的域语义（落库命令直写 store、不广播）
    let records = store.scan("notification", 0, 10).expect("扫描应成功");
    assert_eq!(records.records.len(), 1, "库中恰一行");
    let row = &records.records[0].value;
    assert_eq!(row["kind"], serde_json::json!("feedback"));
    assert_eq!(row["id"], serde_json::json!("uuid-1"));
    assert_eq!(row["message"], serde_json::json!("保存 agent 配置失败"));
    assert_eq!(row["body"], serde_json::Value::Null);
    assert_eq!(row["root"], serde_json::Value::Null);
    assert_eq!(row["sourceRef"], serde_json::Value::Null);
    assert!(
        row["createdAt"].as_i64().unwrap_or(0) > 0,
        "服务端补 created_at"
    );
    assert_eq!(captured.len(), 0, "不广播");
}

#[test]
fn record的body三形态与超长字段落库保真() {
    let env = Env::new("record-body");
    let app = app_with(&env);
    let store = global_store(&app);
    let long = "文".repeat(1200);
    let cases: Vec<(Option<String>, serde_json::Value)> = vec![
        (None, serde_json::Value::Null),
        (Some(String::new()), serde_json::json!("")),
        (Some(long.clone()), serde_json::json!(long)),
    ];

    for (index, (body, expected)) in cases.into_iter().enumerate() {
        let id = format!("uuid-{index}");
        notification_record_with(
            handle_of(&app),
            NotificationFeedbackInput {
                id: id.clone(),
                level: NotificationLevel::Info,
                message: "写契约文件完成".to_owned(),
                body,
            },
        )
        .expect("落库应成功");

        let records = store.scan("notification", 0, 10).expect("扫描应成功");
        let row = records
            .records
            .iter()
            .find(|record| record.value["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("库中缺 {id} 行"));
        assert_eq!(row.value["body"], expected, "空串不折成 null（{id}）");
    }
}

#[test]
fn record对空白id与空白message显式err且零行写入() {
    let env = Env::new("record-blank");
    let app = app_with(&env);
    let store = global_store(&app);

    for (id, message) in [
        (String::new(), "文案".to_owned()),
        ("  ".to_owned(), "文案".to_owned()),
        ("\n".to_owned(), "文案".to_owned()),
        ("uuid-1".to_owned(), String::new()),
        ("uuid-1".to_owned(), "   ".to_owned()),
    ] {
        let outcome = notification_record_with(
            handle_of(&app),
            NotificationFeedbackInput {
                id: id.clone(),
                level: NotificationLevel::Error,
                message,
                body: None,
            },
        );
        assert!(outcome.is_err(), "空白入参应显式 Err（id={id:?}）");
    }

    let records = store.scan("notification", 0, 10).expect("扫描应成功");
    assert_eq!(records.total, 0, "空白入参不进入库链路");
}

#[test]
fn record在workspace维度store上返回err() {
    let env = Env::new("record-workspace");
    let app = app_with_store(env.workspace());

    assert!(
        notification_record_with(handle_of(&app), feedback_input("uuid-1")).is_err(),
        "store 失败上抛为命令错误"
    );
}

#[test]
fn record与list与count共用同一过滤口径() {
    let env = Env::new("record-filter");
    let app = app_with(&env);
    seed_events(&global_store(&app), 3);
    notification_record_with(handle_of(&app), feedback_input("uuid-1")).expect("落库应成功");
    notification_record_with(handle_of(&app), feedback_input("uuid-2")).expect("落库应成功");

    let page = notification_list_with(handle_of(&app), 0, 10).expect("列表应成功");
    assert_eq!(page.items.len(), 3, "列表只 3 条事件");
    assert_eq!(page.total, 3);

    assert_eq!(
        notification_mark_all_read_with(handle_of(&app)).expect("全部已读应成功"),
        0
    );
    let records = global_store(&app)
        .scan("notification", 0, 10)
        .expect("扫描应成功");
    assert_eq!(records.total, 5, "两行反馈仍在库（不受置已读影响）");
    let unread_feedback: Vec<&serde_json::Value> = records
        .records
        .iter()
        .map(|record| &record.value)
        .filter(|value| value["kind"] == serde_json::json!("feedback"))
        .collect();
    assert_eq!(unread_feedback.len(), 2);
    for value in unread_feedback {
        assert_eq!(value["readAt"], serde_json::Value::Null, "反馈行读状态不变");
    }
}

// ---------------------------------------------------------------------------
// NotificationBridge：core 的 `RunNotifier` 出口实现（落库 + 广播 + 终态清理
// 三面各一行转发——接住 core 与 store 之间的最后一跳）
// ---------------------------------------------------------------------------

use orchestration::notify::{NotifyContext, RunNotifier};

use super::NotificationBridge;

/// 库内通知行原始 JSON（`readAt` 只在记录面可见——列表载荷不带读状态）。
fn notification_rows(store: &Store) -> Vec<serde_json::Value> {
    store
        .scan("notification", 0, 200)
        .expect("扫描应成功")
        .records
        .into_iter()
        .map(|record| record.value)
        .collect()
}

/// 指定主键的通知行原始 JSON。
fn notification_row(store: &Store, id: &str) -> serde_json::Value {
    notification_rows(store)
        .into_iter()
        .find(|value| value["id"] == serde_json::json!(id))
        .unwrap_or_else(|| panic!("库中缺通知行 {id}"))
}

/// bridge 的 run 级上下文 fixture（五值由发起侧装配）。
fn context(run_id: &str) -> NotifyContext {
    NotifyContext {
        root: "/ws/alpha".to_owned(),
        change_id: "add-feature".to_owned(),
        change_name: "新增特性".to_owned(),
        run_id: run_id.to_owned(),
        workflow_type: "requirement".to_owned(),
    }
}

/// 正向：真实 bus（tempdir 全局库）构造 bridge → `deliver(event)` → 库恰一行
/// （字段逐项一致）+ 已订阅 Channel 恰收一条（`deliver` → `bus.publish` → Hub
/// 两 sink 的接线闭环）。
#[test]
fn bridge_deliver落库并入广播各一次() {
    let env = Env::new("bridge-deliver");
    let app = app_with(&env);
    let captured = Captured::new();
    notification_subscribe_with(handle_of(&app), captured.channel()).expect("订阅应成功");

    let bridge = NotificationBridge::new(context("run-b1"), bus_of(&app));
    let notification = event("run:run-b1:finished", "变更「新增特性」已完成全部相位");
    bridge.deliver(&notification);

    let page = global_store(&app)
        .list_notifications(0, 10)
        .expect("列表应成功");
    assert_eq!(page.total, 1, "库恰一行");
    let row = &page.items[0];
    assert_eq!(row, &notification, "字段与载荷逐项一致");

    wait_for("广播抵达订阅端", || captured.len() == 1);
    assert_eq!(
        captured.items()[0],
        notification,
        "Channel 恰收一条同源通知"
    );
}

/// 边界：同一 bridge 连续 `deliver` 同主键两条（`message` 不同）→ 库仍一行、
/// `message` 取后者、`read_at` 保持（幂等归 store，bridge 零状态、不去重）。
#[test]
fn bridge_deliver同主键二次仍一行且读状态保持() {
    let env = Env::new("bridge-idempotent");
    let app = app_with(&env);
    let bridge = NotificationBridge::new(context("run-b2"), bus_of(&app));

    let first = event("run:run-b2:confirm:archive", "第一次文案");
    bridge.deliver(&first);
    global_store(&app)
        .mark_notifications_read(&[first.id.clone()])
        .expect("置已读应成功");
    let read_at = notification_row(&global_store(&app), &first.id)["readAt"].clone();
    assert!(!read_at.is_null(), "首条置已读后 readAt 在场");

    let mut second = first.clone();
    second.message = "第二次文案".to_owned();
    bridge.deliver(&second);

    let page = global_store(&app)
        .list_notifications(0, 10)
        .expect("列表应成功");
    assert_eq!(page.total, 1, "同主键仍一行");
    assert_eq!(page.items[0].message, "第二次文案", "message 取后者");
    assert_eq!(
        notification_row(&global_store(&app), &first.id)["readAt"],
        read_at,
        "读状态保持（不复活已读）"
    );
}

/// 异常：bridge 持 workspace 维度 store 的 bus → `deliver` 不 panic、零上抛、
/// 全局库零行，订阅端仍收到该条（一 sink 失败不影响 in-app sink）。
#[test]
fn bridge_deliver库不可用时零上抛且广播照常() {
    let env = Env::new("bridge-unavailable");
    let app = app_with_store(env.workspace());
    let captured = Captured::new();
    // 计数读取在 workspace 维度库上必失败（订阅命令整体 Err），故直接取
    // broadcast 端起转发——本用例只观察 in-app sink 那一跳
    let bus = bus_of(&app);
    super::spawn_notification_forward(captured.channel(), bus.subscribe());
    let bridge = NotificationBridge::new(context("run-b3"), Arc::clone(&bus));

    bridge.deliver(&event("run:run-b3:finished", "故障注入下仍应广播"));

    wait_for("广播抵达订阅端", || captured.len() == 1);
    assert_eq!(
        captured.items()[0].id,
        "run:run-b3:finished",
        "落库 sink 失败不影响 in-app sink"
    );
}

/// 正向：`retire()` 的 `run_id` 取自 `context()`——预置同 run 两条等待类未读 +
/// 他 run 一条 → 同 run 转已读、他 run 不变。
#[test]
fn bridge_retire只清同run等待类未读() {
    let env = Env::new("bridge-retire");
    let app = app_with(&env);
    let store = global_store(&app);
    for id in [
        "run:run-b4:confirm:archive",
        "run:run-b4:ask:tool-A",
        "run:run-other:confirm:archive",
    ] {
        store
            .publish_notification(&event(id, "等待中"))
            .expect("落库应成功");
    }
    assert_eq!(store.unread_notification_count().expect("计数应成功"), 3);

    let bridge = NotificationBridge::new(context("run-b4"), bus_of(&app));
    bridge.retire();

    let read_of = |id: &str| !notification_row(&store, id)["readAt"].is_null();
    assert!(read_of("run:run-b4:confirm:archive"), "同 run 停等转已读");
    assert!(read_of("run:run-b4:ask:tool-A"), "同 run ask 转已读");
    assert!(!read_of("run:run-other:confirm:archive"), "他 run 不受影响");
}

/// 边界：未知 run 与空 `run_id` → 零写入零 panic（R-14：miss 即无操作）。
#[test]
fn bridge_retire未知run与空run_id零写入() {
    let env = Env::new("bridge-retire-miss");
    let app = app_with(&env);
    let store = global_store(&app);
    store
        .publish_notification(&event("run:run-b5:confirm:archive", "等待中"))
        .expect("落库应成功");

    for run_id in ["run-unknown", ""] {
        NotificationBridge::new(context(run_id), bus_of(&app)).retire();
    }

    assert_eq!(
        store.unread_notification_count().expect("计数应成功"),
        1,
        "零写入（未读数不变）"
    );
}

/// 正向：`context()` 返回构造时的五字段逐字相等，且不随 `deliver` / `retire`
/// 变化（自持一份）。
#[test]
fn bridge_context五字段自持不随转发变化() {
    let env = Env::new("bridge-context");
    let app = app_with(&env);
    let expected = context("run-b6");
    let bridge = NotificationBridge::new(expected.clone(), bus_of(&app));

    assert_eq!(bridge.context(), &expected, "构造值逐字相等");

    bridge.deliver(&event("run:run-b6:finished", "文案"));
    bridge.retire();

    assert_eq!(bridge.context(), &expected, "转发后不变（自持单份）");
}

/// 异常（结构锚）：`mod.rs` 内 `RunUpdate::` 零命中（壳层零按变体过滤，白名单
/// 只在 `notify::project`）；`fn retire_run(` / `fn retire(` / 对 `.retire_run(`
/// 的调用各恰 1 处（终态清理的唯一实现面）。
#[test]
fn 接线结构锚_壳层零变体过滤与唯一清理面() {
    let source = include_str!("mod.rs");
    assert!(
        !source.contains("RunUpdate::"),
        "壳层零按变体过滤（白名单只在 notify::project）"
    );
    for needle in ["fn retire_run(", "fn retire(", ".retire_run("] {
        assert_eq!(
            source.matches(needle).count(),
            1,
            "终态清理唯一实现面：{needle}"
        );
    }
}
