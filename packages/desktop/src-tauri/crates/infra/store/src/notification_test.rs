//! 通知落库面的行为面：记录 ↔ 载荷一比一映射（`read_at` 是唯一单向差）+ 落库
//! sink 的旁路降级。
//!
//! 零 mock：真实 tempdir 红库（全局库 / workspace 库各开一次表达两种维度）；
//! `eprintln!` 不捕获——失败判据取「不 panic / 零返回错误 / 库中零行」。

use std::path::PathBuf;
use std::sync::Arc;

use ::notification::{Notification, NotificationKind, NotificationLevel, Sink};

use crate::model::NotificationRecord;
use crate::notification::StoreNotificationSink;
use crate::store::Store;

// ---------------------------------------------------------------------------
// 装置：tempdir + 两维度库文件（同一文件不能双维度混开）
// ---------------------------------------------------------------------------

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("store-notification-test-{tag}-"))
            .tempdir()
            .expect("创建临时目录失败");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(format!("{name}.redb"))
    }

    /// 全局库（收件箱是 user 维度，落库 sink 的合法入参）。
    fn global(&self) -> Arc<Store> {
        Arc::new(Store::open_global(&self.path("global")).expect("open_global 应成功"))
    }

    /// workspace 库实例（跨维度误传的故障注入面）。
    fn workspace(&self) -> Arc<Store> {
        Arc::new(Store::open_workspace(&self.path("ws")).expect("open_workspace 应成功"))
    }
}

/// 事件型载荷 fixture。
fn event(id: &str) -> Notification {
    Notification {
        id: id.to_owned(),
        kind: NotificationKind::Event,
        level: NotificationLevel::Warning,
        message: format!("变更「add-feature」停在 dev-design 前等你确认（{id}）"),
        body: Some("设计相位产出。".to_owned()),
        root: Some("/ws/alpha".to_owned()),
        source_ref: Some("add-feature".to_owned()),
        created_at: 1_727_000_000_000,
    }
}

/// 反馈型载荷 fixture（`root` / `source_ref` 恒 None）。
fn feedback(id: &str) -> Notification {
    Notification {
        id: id.to_owned(),
        kind: NotificationKind::Feedback,
        level: NotificationLevel::Error,
        message: "保存 agent 配置失败".to_owned(),
        body: None,
        root: None,
        source_ref: None,
        created_at: 1_727_000_000_500,
    }
}

// ---------------------------------------------------------------------------
// NotificationRecord::from_notification：载荷 → 记录
// ---------------------------------------------------------------------------

#[test]
fn from_notification逐项对应九字段且读状态恒none() {
    let notification = event("run:r1:confirm:dev-design");
    let record = NotificationRecord::from_notification(&notification);

    assert_eq!(record.id, notification.id);
    assert_eq!(record.kind, notification.kind);
    assert_eq!(record.level, notification.level);
    assert_eq!(record.message, notification.message);
    assert_eq!(record.body, notification.body);
    assert_eq!(record.root, notification.root);
    assert_eq!(record.source_ref, notification.source_ref);
    assert_eq!(record.created_at, notification.created_at);
    assert_eq!(record.read_at, None, "新行语义：未读");
}

#[test]
fn from_notification三option字段组合与时间边界原样() {
    let option_cases: Vec<(Option<String>, Option<String>, Option<String>)> = vec![
        (None, None, None),
        (
            Some(String::new()),
            Some(String::new()),
            Some(String::new()),
        ),
        (None, Some("/ws/beta".to_owned()), None),
        (Some("副文案".to_owned()), None, Some("fix-bug".to_owned())),
    ];
    for (body, root, source_ref) in option_cases {
        for created_at in [0i64, -1, i64::MAX] {
            let notification = Notification {
                body: body.clone(),
                root: root.clone(),
                source_ref: source_ref.clone(),
                created_at,
                ..event("run:r1:finished")
            };
            let record = NotificationRecord::from_notification(&notification);
            assert_eq!(record.body, body, "body 原样（Some(\"\") 不折成 None）");
            assert_eq!(record.root, root);
            assert_eq!(record.source_ref, source_ref);
            assert_eq!(record.created_at, created_at);
            assert_eq!(record.read_at, None);
        }
    }

    let long_id = "i".repeat(1200);
    let record = NotificationRecord::from_notification(&event(&long_id));
    assert_eq!(record.id, long_id, "超长 id 原样");
    let record = NotificationRecord::from_notification(&event(""));
    assert_eq!(record.id, "", "空 id 原样");
}

#[test]
fn from_notification不掺域语义_kind与归属原样() {
    let event_record = NotificationRecord::from_notification(&event("run:r1:ask:A"));
    assert_eq!(event_record.kind, NotificationKind::Event);
    assert_eq!(event_record.root, Some("/ws/alpha".to_owned()));

    let feedback_record = NotificationRecord::from_notification(&feedback("uuid-1"));
    assert_eq!(feedback_record.kind, NotificationKind::Feedback);
    assert_eq!(feedback_record.root, None, "反馈轨归属恒 None（零特判）");
    assert_eq!(feedback_record.source_ref, None);
}

// ---------------------------------------------------------------------------
// NotificationRecord::to_notification：记录 → 载荷
// ---------------------------------------------------------------------------

#[test]
fn to_notification八字段相等且无读状态() {
    let mut record = NotificationRecord::from_notification(&event("run:r1:finished"));
    record.read_at = Some(1_727_000_001_000);

    let notification = record.to_notification();

    assert_eq!(notification.id, record.id);
    assert_eq!(notification.kind, record.kind);
    assert_eq!(notification.level, record.level);
    assert_eq!(notification.message, record.message);
    assert_eq!(notification.body, record.body);
    assert_eq!(notification.root, record.root);
    assert_eq!(notification.source_ref, record.source_ref);
    assert_eq!(notification.created_at, record.created_at);
}

#[test]
fn to_notification对读状态三态输出完全相同() {
    let mut record = NotificationRecord::from_notification(&event("run:r1:finished"));
    let outputs: Vec<Notification> = [None, Some(0), Some(i64::MAX)]
        .into_iter()
        .map(|read_at| {
            record.read_at = read_at;
            record.to_notification()
        })
        .collect();

    assert_eq!(outputs[0], outputs[1], "读状态不进载荷（恒定行为）");
    assert_eq!(outputs[1], outputs[2], "读状态不进载荷（恒定行为）");
}

#[test]
fn to_notification保持feedback的kind不归一成event() {
    let mut record = NotificationRecord::from_notification(&feedback("uuid-2"));
    record.read_at = Some(1);

    let notification = record.to_notification();
    assert_eq!(
        notification.kind,
        NotificationKind::Feedback,
        "过滤口径只在 store 查询层"
    );
}

// ---------------------------------------------------------------------------
// 往返保真
// ---------------------------------------------------------------------------

#[test]
fn from_to往返与载荷逐字段相等() {
    for notification in [
        event("run:r1:confirm:dev-design"),
        feedback("uuid-3"),
        Notification {
            id: String::new(),
            body: None,
            ..event("run:r1:finished")
        },
    ] {
        let round_trip = NotificationRecord::from_notification(&notification).to_notification();
        assert_eq!(round_trip, notification);
    }
}

#[test]
fn 往返对极端字段值保真() {
    let long_message = "长文案".repeat(600);
    let cases = [
        Notification {
            message: long_message.clone(),
            ..event("run:r1:finished")
        },
        Notification {
            message: "含换行\n与空字节\0与 emoji 🎉".to_owned(),
            ..event("run:r1:finished")
        },
        Notification {
            body: None,
            ..event("run:r1:finished")
        },
        Notification {
            root: Some(String::new()),
            ..event("run:r1:finished")
        },
    ];
    for notification in cases {
        let round_trip = NotificationRecord::from_notification(&notification).to_notification();
        assert_eq!(round_trip, notification, "极端字段值往返保真");
    }
}

// ---------------------------------------------------------------------------
// StoreNotificationSink：旁路落库
// ---------------------------------------------------------------------------

#[test]
fn sink_deliver落库后恰一行且计数与列表同源() {
    let env = Env::new("deliver");
    let store = env.global();
    let sink = StoreNotificationSink::new(Arc::clone(&store));

    let notification = event("run:r1:confirm:dev-design");
    sink.deliver(&notification);

    let page = store.list_notifications(0, 10).expect("列表应成功");
    assert_eq!(page.items.len(), 1, "恰一行");
    assert_eq!(page.total, 1);
    let row = &page.items[0];
    assert_eq!(row.id, notification.id);
    assert_eq!(row.kind, notification.kind);
    assert_eq!(row.level, notification.level);
    assert_eq!(row.message, notification.message);
    assert_eq!(row.body, notification.body);
    assert_eq!(row.root, notification.root);
    assert_eq!(row.source_ref, notification.source_ref);
    assert_eq!(row.created_at, notification.created_at);
    assert_eq!(
        store.unread_notification_count().expect("计数应成功"),
        1,
        "落库即可见未读"
    );
}

#[test]
fn sink_两实例同主键投递后仍一行且读状态保持() {
    let env = Env::new("idempotent");
    let store = env.global();
    let first = StoreNotificationSink::new(Arc::clone(&store));
    let second = StoreNotificationSink::new(Arc::clone(&store));

    first.deliver(&event("run:r1:finished"));
    let id = "run:r1:finished".to_owned();
    assert_eq!(
        store
            .mark_notifications_read(std::slice::from_ref(&id))
            .expect("置已读应成功"),
        0,
        "首条置已读后无剩余未读"
    );

    second.deliver(&Notification {
        message: "变更「add-feature」运行失败".to_owned(),
        ..event("run:r1:finished")
    });

    let page = store.list_notifications(0, 10).expect("列表应成功");
    assert_eq!(page.items.len(), 1, "幂等由 store 承担：仍一行");
    assert_eq!(
        page.items[0].message, "变更「add-feature」运行失败",
        "文案取后者"
    );
    assert_eq!(
        store.unread_notification_count().expect("计数应成功"),
        0,
        "sink 不掺读状态语义（重投不复活已读）"
    );
}

#[test]
fn sink_持workspace维度库时静默降级零上抛零记录() {
    let env = Env::new("workspace-dimension");
    let store = env.workspace();
    let sink = StoreNotificationSink::new(Arc::clone(&store));

    // 跨维度写入必失败：sink 就地 eprintln! 降级，不 panic、不上抛
    sink.deliver(&event("run:r1:finished"));

    // 结构性判据：该库的模型组里没有通知表，读面同样不可达
    assert!(
        store.unread_notification_count().is_err(),
        "workspace 库零通知记录（模型未注册，读亦不可达）"
    );
}

#[test]
fn sink与store链路闭环_重投更新文案而不复位读状态() {
    let env = Env::new("chain");
    let store = env.global();
    let sink = StoreNotificationSink::new(Arc::clone(&store));
    let id = "run:r1:confirm:archive".to_owned();

    sink.deliver(&Notification {
        id: id.clone(),
        ..event(&id)
    });
    let page = store.list_notifications(0, 10).expect("列表应成功");
    assert_eq!(page.items.len(), 1);
    assert_eq!(store.unread_notification_count().expect("计数应成功"), 1);

    assert_eq!(
        store
            .mark_notifications_read(std::slice::from_ref(&id))
            .expect("置已读应成功"),
        0
    );

    sink.deliver(&Notification {
        id: id.clone(),
        message: "变更「add-feature」停在 archive 前等你确认".to_owned(),
        body: Some("归档前最后一道门。".to_owned()),
        ..event(&id)
    });

    let page = store.list_notifications(0, 10).expect("列表应成功");
    assert_eq!(page.items.len(), 1, "同主键仍一行");
    assert_eq!(
        page.items[0].message, "变更「add-feature」停在 archive 前等你确认",
        "文案更新"
    );
    assert_eq!(
        page.items[0].body.as_deref(),
        Some("归档前最后一道门。"),
        "副文案更新"
    );
    assert_eq!(
        store.unread_notification_count().expect("计数应成功"),
        0,
        "读状态不复位（AC-5 在 sink 链路成立）"
    );
}
