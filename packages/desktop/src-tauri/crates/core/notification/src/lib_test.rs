//! 通知词表与扇出点的行为面：装配序即投递序 / 逐 sink 投一次 / 一 sink 失败不
//! 影响后续 / 词表线形与解码拒绝。
//!
//! 零 mock：`Sink` 替身是测试内定义的协作者（记录型 / 失败收场型），不引入替身
//! 框架；`created_at` 是显式入参，直接注入 `0` / 负值 / `i64::MAX` 字面量。

use std::sync::{Arc, Mutex};

use crate::{
    Hub, Notification, NotificationFeedbackInput, NotificationKind, NotificationLevel,
    NotificationPage, Sink,
};

// ---------------------------------------------------------------------------
// 装置：记录型 sink（收下的通知按引用内容克隆落进共享缓冲）
// ---------------------------------------------------------------------------

struct RecordingSink {
    seen: Mutex<Vec<Notification>>,
}

impl RecordingSink {
    fn new() -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
        }
    }

    fn arc() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// 收到的通知（按投递序）。
    fn seen(&self) -> Vec<Notification> {
        self.seen.lock().expect("记录缓冲未中毒").clone()
    }

    /// 收到的 `id` 序列（投递序）。
    fn ids(&self) -> Vec<String> {
        self.seen()
            .into_iter()
            .map(|notification| notification.id)
            .collect()
    }

    fn count(&self) -> usize {
        self.seen.lock().expect("记录缓冲未中毒").len()
    }
}

impl Sink for RecordingSink {
    fn deliver(&self, notification: &Notification) {
        self.seen
            .lock()
            .expect("记录缓冲未中毒")
            .push(notification.clone());
    }
}

/// 失败收场型 sink：内部把一个恒 `Err` 降级为一行留痕（落库 sink 的失败形态），
/// `deliver` 仍无返回值、零 panic、零上抛。
struct FailingSink {
    failures: Mutex<Vec<String>>,
}

impl FailingSink {
    fn new() -> Self {
        Self {
            failures: Mutex::new(Vec::new()),
        }
    }

    fn arc() -> Arc<Self> {
        Arc::new(Self::new())
    }

    fn failure_count(&self) -> usize {
        self.failures.lock().expect("失败缓冲未中毒").len()
    }
}

impl Sink for FailingSink {
    fn deliver(&self, notification: &Notification) {
        // 模拟落库失败：Err 就地降级为一行留痕（不重试、不上抛、不再发通知）
        let outcome: Result<(), String> = Err(format!("落库失败：{}", notification.id));
        if let Err(error) = outcome {
            self.failures.lock().expect("失败缓冲未中毒").push(error);
        }
    }
}

/// 记录型 sink 集合 → 扇出面入参（装配序即投递序）。
fn sinks(recorders: &[Arc<RecordingSink>]) -> Vec<Arc<dyn Sink + Send + Sync>> {
    recorders
        .iter()
        .map(|recorder| Arc::clone(recorder) as Arc<dyn Sink + Send + Sync>)
        .collect()
}

/// 事件型通知 fixture（`kind = Event`，`body` / `root` / `source_ref` 均在场）。
fn sample(id: &str) -> Notification {
    Notification {
        id: id.to_owned(),
        kind: NotificationKind::Event,
        level: NotificationLevel::Warning,
        message: format!("变更「add-feature」在等你的回答（{id}）"),
        body: Some("先确认验收范围".to_owned()),
        root: Some("/ws/alpha".to_owned()),
        source_ref: Some("add-feature".to_owned()),
        created_at: 1_727_000_000_000,
    }
}

/// 反馈入参 fixture。
fn feedback_input(id: &str, level: NotificationLevel) -> NotificationFeedbackInput {
    NotificationFeedbackInput {
        id: id.to_owned(),
        level,
        message: "保存 agent 配置失败".to_owned(),
        body: None,
    }
}

// ---------------------------------------------------------------------------
// Hub::publish：装配序 = 投递序，逐 sink 恰一次
// ---------------------------------------------------------------------------

#[test]
fn hub_publish两sink按装配序各收一次() {
    let first = RecordingSink::arc();
    let second = RecordingSink::arc();
    let hub = Hub::new(sinks(&[Arc::clone(&first), Arc::clone(&second)]));

    hub.publish(&sample("run:r1:ask:A"));

    assert_eq!(
        first.ids(),
        vec!["run:r1:ask:A".to_owned()],
        "先装配者恰收一次"
    );
    assert_eq!(
        second.ids(),
        vec!["run:r1:ask:A".to_owned()],
        "后装配者恰收一次"
    );
}

#[test]
fn hub_publish连续两条时同sink内序等于发布序且跨sink计数不交错() {
    let first = RecordingSink::arc();
    let second = RecordingSink::arc();
    let hub = Hub::new(sinks(&[Arc::clone(&first), Arc::clone(&second)]));

    hub.publish(&sample("run:r1:confirm:dev-design"));
    hub.publish(&sample("run:r1:finished"));

    let expected = vec![
        "run:r1:confirm:dev-design".to_owned(),
        "run:r1:finished".to_owned(),
    ];
    assert_eq!(first.ids(), expected, "同一 sink 内 FIFO");
    assert_eq!(second.ids(), expected, "每个 sink 各收 2 次");
    assert_eq!(first.count() + second.count(), 4, "跨 sink 计数不交错破坏");
}

#[test]
fn hub_publish首个sink失败收场时后续sink仍各收一次且返回单元() {
    let failing = FailingSink::arc();
    let second = RecordingSink::arc();
    let hub = Hub::new(vec![
        Arc::clone(&failing) as Arc<dyn Sink + Send + Sync>,
        Arc::clone(&second) as Arc<dyn Sink + Send + Sync>,
    ]);

    hub.publish(&sample("run:r1:ask:B"));

    assert_eq!(failing.failure_count(), 1, "失败 sink 内部记下一次留痕");
    assert_eq!(
        second.ids(),
        vec!["run:r1:ask:B".to_owned()],
        "失败不阻断后续 sink"
    );
}

#[test]
fn hub_publish空sink列表时零投递零panic() {
    let hub = Hub::new(Vec::new());

    hub.publish(&sample("run:r1:finished"));
}

#[test]
fn hub_publish三十二sink且极端载荷时全部收到且内容原样() {
    let recorders: Vec<Arc<RecordingSink>> = (0..32).map(|_| RecordingSink::arc()).collect();
    let hub = Hub::new(sinks(&recorders));

    let long_message = "变更".repeat(600);
    let special_body = "副文案含换行\n与空字节\0与 emoji 🎉".to_owned();
    let notification = Notification {
        id: String::new(),
        kind: NotificationKind::Event,
        level: NotificationLevel::Error,
        message: long_message.clone(),
        body: Some(special_body.clone()),
        root: Some("/ws/alpha".to_owned()),
        source_ref: None,
        created_at: i64::MAX,
    };
    hub.publish(&notification);

    for recorder in &recorders {
        let seen = recorder.seen();
        assert_eq!(seen.len(), 1, "32 个 sink 各恰收一次");
        assert_eq!(seen[0].message, long_message, "超长主文案零截断");
        assert_eq!(seen[0].id, "", "空 id 原样");
        assert_eq!(
            seen[0].body.as_deref(),
            Some(special_body.as_str()),
            "特殊字符零转义"
        );
    }
}

// ---------------------------------------------------------------------------
// Hub::new：装配与投递一一对应，不去重
// ---------------------------------------------------------------------------

#[test]
fn hub_new按规模装配时单次publish的投递总次数恰为规模() {
    for size in [1usize, 2, 8] {
        let recorders: Vec<Arc<RecordingSink>> = (0..size).map(|_| RecordingSink::arc()).collect();
        let hub = Hub::new(sinks(&recorders));

        hub.publish(&sample("run:r1:finished"));

        let total: usize = recorders.iter().map(|recorder| recorder.count()).sum();
        assert_eq!(total, size, "装配 {size} 个 sink 则投递 {size} 次");
    }
}

#[test]
fn hub_new不去重且空向量合法() {
    // 同一 Arc 重复装配两次：投递两次（不按引用合并）
    let shared = RecordingSink::arc();
    let hub = Hub::new(sinks(&[Arc::clone(&shared), Arc::clone(&shared)]));
    hub.publish(&sample("run:r1:ask:C"));
    assert_eq!(shared.count(), 2, "同一 sink 装配两次即投递两次");

    // 空向量：构造成功、publish 零投递
    let empty = Hub::new(sinks(&[]));
    empty.publish(&sample("run:r1:ask:D"));
}

#[test]
fn hub_new对必然失败的sink仍构造成功() {
    let failing = FailingSink::arc();
    let hub = Hub::new(vec![Arc::clone(&failing) as Arc<dyn Sink + Send + Sync>]);
    assert_eq!(failing.failure_count(), 0, "构造不做能力探测（零投递）");

    hub.publish(&sample("run:r1:finished"));
    assert_eq!(failing.failure_count(), 1, "失败只在投递期由 sink 自处");
}

// ---------------------------------------------------------------------------
// Sink::deliver：同步 / 无返回值 / 对象安全
// ---------------------------------------------------------------------------

#[test]
fn sink_deliver无返回值且可跨线程共享() {
    let recorder = RecordingSink::arc();
    let notification = sample("run:r1:confirm:archive");

    // 签名面：无返回值（同步、非 async、无 Result）
    let outcome: () = recorder.deliver(&notification);
    assert_eq!(outcome, ());
    assert_eq!(recorder.count(), 1);

    // `Send + Sync` 对象安全：Arc<dyn Sink + Send + Sync> 进线程后调用成功
    let erased: Arc<dyn Sink + Send + Sync> = Arc::clone(&recorder) as Arc<dyn Sink + Send + Sync>;
    let handle = std::thread::spawn(move || {
        erased.deliver(&sample("run:r1:ask:t"));
    });
    handle.join().expect("跨线程投递不 panic");
    assert_eq!(recorder.count(), 2, "跨线程调用落在同一实例上");
}

#[test]
fn sink_deliver同实例被两hub持有投递互不影响且计数累加() {
    let shared = RecordingSink::arc();
    let first_hub = Hub::new(sinks(&[Arc::clone(&shared)]));
    let second_hub = Hub::new(sinks(&[Arc::clone(&shared)]));

    first_hub.publish(&sample("run:r1:ask:A"));
    second_hub.publish(&sample("run:r1:ask:B"));

    assert_eq!(shared.count(), 2, "core 不假设 sink 无状态");
}

#[test]
fn sink_deliver内部吞错后调用方零错误面() {
    let failing = FailingSink::arc();
    let hub = Hub::new(vec![Arc::clone(&failing) as Arc<dyn Sink + Send + Sync>]);

    // 失败只能由 sink 自身留痕：Hub::publish 无 Result / 无异常通道可观测
    hub.publish(&sample("run:r1:finished"));
    assert_eq!(failing.failure_count(), 1);
}

// ---------------------------------------------------------------------------
// Notification::feedback：域语义收口（kind / root / source_ref 固定）
// ---------------------------------------------------------------------------

#[test]
fn notification_feedback固定kind与归属且其余字段逐项搬运() {
    let input = NotificationFeedbackInput {
        id: "9f1c2f6e-8b34-4f3a-9a0b-5c1d2e3f4a5b".to_owned(),
        level: NotificationLevel::Error,
        message: "删除 workspace 失败".to_owned(),
        body: Some("目录被占用".to_owned()),
    };

    let notification = Notification::feedback(input.clone(), 1_727_000_000_123);

    assert_eq!(notification.kind, NotificationKind::Feedback);
    assert_eq!(notification.root, None, "反馈不带工作区归属断言");
    assert_eq!(notification.source_ref, None);
    assert_eq!(notification.created_at, 1_727_000_000_123);
    assert_eq!(notification.id, input.id);
    assert_eq!(notification.level, NotificationLevel::Error);
    assert_eq!(notification.message, input.message);
    assert_eq!(notification.body, input.body);
}

#[test]
fn notification_feedback的body三形态原样搬运且id零校验() {
    let long_body = "副文案".repeat(400);
    let cases: Vec<(Option<String>, &str)> = vec![
        (None, "缺失档"),
        (Some(String::new()), "空串档"),
        (Some(long_body), "超长档"),
    ];
    for (body, label) in cases {
        let input = NotificationFeedbackInput {
            body: body.clone(),
            ..feedback_input("", NotificationLevel::Info)
        };
        let notification = Notification::feedback(input, 0);
        assert_eq!(notification.body, body, "{label}：body 原样搬运（零归一）");
        assert_eq!(notification.id, "", "{label}：空 id 原样（词表层零校验）");
    }

    let long_id = "i".repeat(1200);
    let notification = Notification::feedback(
        NotificationFeedbackInput {
            id: long_id.clone(),
            ..feedback_input("ignored", NotificationLevel::Info)
        },
        0,
    );
    assert_eq!(notification.id, long_id, "超长 id 原样");
}

#[test]
fn notification_feedback对四档level零特判() {
    for level in [
        NotificationLevel::Info,
        NotificationLevel::Success,
        NotificationLevel::Warning,
        NotificationLevel::Error,
    ] {
        let notification = Notification::feedback(feedback_input("id-1", level), 0);
        assert_eq!(notification.level, level, "feedback 不按 level 特判");
        assert_eq!(notification.kind, NotificationKind::Feedback);
    }
}

// ---------------------------------------------------------------------------
// Notification：serde 线形八字段 / 零读状态 / 解码拒绝
// ---------------------------------------------------------------------------

#[test]
fn notification线形八字段且多词键为camelcase且不含读状态() {
    let value = serde_json::to_value(sample("run:r1:confirm:archive")).expect("序列化应成功");
    let object = value.as_object().expect("线形为对象");

    assert_eq!(object.len(), 8, "线形恰八字段");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "body",
            "createdAt",
            "id",
            "kind",
            "level",
            "message",
            "root",
            "sourceRef"
        ],
        "多词键为 camelCase"
    );

    let text = serde_json::to_string(&sample("run:r1:finished")).expect("序列化应成功");
    assert!(!text.contains("read_at"), "读状态不进载荷");
    assert!(
        !text.contains("readAt"),
        "读状态不进载荷（camelCase 亦不得出现）"
    );
}

#[test]
fn notification解码拒绝缺字段与非法枚举线值() {
    let full = serde_json::json!({
        "id": "run:r1:ask:A",
        "kind": "event",
        "level": "warning",
        "message": "变更在等你的回答",
        "body": null,
        "root": null,
        "sourceRef": null,
        "createdAt": 1,
    });

    let rejected: Vec<(&str, serde_json::Value)> = vec![
        ("缺 id", drop_key(&full, "id")),
        ("缺 kind", drop_key(&full, "kind")),
        ("缺 level", drop_key(&full, "level")),
        ("缺 message", drop_key(&full, "message")),
        (
            "kind 非法",
            replace_key(&full, "kind", serde_json::json!("unknown")),
        ),
        (
            "level 非法",
            replace_key(&full, "level", serde_json::json!("debug")),
        ),
    ];
    for (label, value) in rejected {
        let text = serde_json::to_string(&value).expect("构造样本应可序列化");
        let decoded: Result<Notification, _> = serde_json::from_str(&text);
        assert!(decoded.is_err(), "{label}：应拒绝而非静默产出空载荷");
    }
}

#[test]
fn notification三option字段组合与时间边界原样往返() {
    let option_cases: Vec<(Option<String>, Option<String>, Option<String>)> = vec![
        (None, None, None),
        (
            Some(String::new()),
            Some(String::new()),
            Some(String::new()),
        ),
        (None, Some(String::new()), None),
        (
            Some("副文案".to_owned()),
            None,
            Some("add-feature".to_owned()),
        ),
    ];
    for (body, root, source_ref) in option_cases {
        for created_at in [0i64, -1, i64::MAX] {
            let notification = Notification {
                body: body.clone(),
                root: root.clone(),
                source_ref: source_ref.clone(),
                created_at,
                ..sample("run:r1:finished")
            };
            let text = serde_json::to_string(&notification).expect("序列化应成功");
            let decoded: Notification = serde_json::from_str(&text).expect("解码应成功");
            assert_eq!(decoded, notification, "线形原样（Some(\"\") 不折成 None）");
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationKind / NotificationLevel：变体序冻结与非法线值
// ---------------------------------------------------------------------------

#[test]
fn notification_kind两变体线值且声明序冻结() {
    assert_eq!(
        serde_json::to_value(NotificationKind::Event).expect("序列化应成功"),
        serde_json::json!("event")
    );
    assert_eq!(
        serde_json::to_value(NotificationKind::Feedback).expect("序列化应成功"),
        serde_json::json!("feedback")
    );

    // 变体序冻结：`Event` 先（落库行以本枚举编码承载，重排即旧行解码错位）
    let declared = [NotificationKind::Event, NotificationKind::Feedback];
    let wire: Vec<String> = declared
        .iter()
        .map(|kind| {
            serde_json::to_value(kind)
                .expect("序列化应成功")
                .to_string()
        })
        .collect();
    assert_eq!(
        wire,
        vec!["\"event\"".to_owned(), "\"feedback\"".to_owned()]
    );
}

#[test]
fn notification_kind拒绝非法线值() {
    for text in ["\"unknown\"", "\"Event\"", "\"\"", "null"] {
        let decoded: Result<NotificationKind, _> = serde_json::from_str(text);
        assert!(decoded.is_err(), "非法线值应拒绝：{text}");
    }
}

#[test]
fn notification_kind与level交叉编解码不互相污染() {
    let cases = [
        (NotificationKind::Event, NotificationLevel::Info),
        (NotificationKind::Feedback, NotificationLevel::Error),
    ];
    for (kind, level) in cases {
        let notification = Notification {
            kind,
            level,
            ..sample("run:r1:finished")
        };
        let text = serde_json::to_string(&notification).expect("序列化应成功");
        let decoded: Notification = serde_json::from_str(&text).expect("解码应成功");
        assert_eq!(decoded.kind, kind);
        assert_eq!(decoded.level, level);
    }
}

#[test]
fn notification_level四档线值往返闭合() {
    let cases = [
        (NotificationLevel::Info, "info"),
        (NotificationLevel::Success, "success"),
        (NotificationLevel::Warning, "warning"),
        (NotificationLevel::Error, "error"),
    ];
    for (level, wire) in cases {
        let value = serde_json::to_value(level).expect("序列化应成功");
        assert_eq!(value, serde_json::json!(wire), "线值逐字");
        let decoded: NotificationLevel = serde_json::from_value(value).expect("解码应成功");
        assert_eq!(decoded, level, "还原同变体");
    }
}

#[test]
fn notification_level线形为裸字符串且copy语义下两次取用() {
    let value = serde_json::to_value(NotificationLevel::Warning).expect("序列化应成功");
    assert!(value.is_string(), "线形为裸字符串（非对象 / 非数字）");

    let level = NotificationLevel::Success;
    let copied = level;
    assert_eq!(level, copied, "Copy 语义下两次取用无 move 报错");
}

#[test]
fn notification_level拒绝非法线值且大小写不归一() {
    for text in ["\"debug\"", "\"Info\"", "\"\"", "null", "1"] {
        let decoded: Result<NotificationLevel, _> = serde_json::from_str(text);
        assert!(decoded.is_err(), "非法线值应拒绝：{text}");
    }
}

// ---------------------------------------------------------------------------
// NotificationPage：页界信封
// ---------------------------------------------------------------------------

#[test]
fn notification_page的total不被改写成条目数() {
    let page = NotificationPage {
        items: vec![sample("run:r1:ask:A"), sample("run:r1:finished")],
        total: 5,
    };

    let value = serde_json::to_value(&page).expect("序列化应成功");
    assert_eq!(value["total"], serde_json::json!(5), "total 逐字保留");
    assert_eq!(
        value["items"].as_array().map(Vec::len),
        Some(2),
        "items 原样两项"
    );

    let text = serde_json::to_string(&page).expect("序列化应成功");
    let decoded: NotificationPage = serde_json::from_str(&text).expect("解码应成功");
    assert_eq!(decoded, page);
}

#[test]
fn notification_page空页与u32上界可序列化() {
    let empty = NotificationPage {
        items: Vec::new(),
        total: 0,
    };
    let text = serde_json::to_string(&empty).expect("序列化应成功");
    let decoded: NotificationPage = serde_json::from_str(&text).expect("解码应成功");
    assert_eq!(decoded, empty);

    let maxed = NotificationPage {
        items: Vec::new(),
        total: u32::MAX,
    };
    let value = serde_json::to_value(&maxed).expect("上界不溢出");
    assert_eq!(value["total"], serde_json::json!(u32::MAX));
}

#[test]
fn notification_page解码拒绝缺字段与类型错位() {
    let full = serde_json::json!({ "items": [], "total": 0 });
    let rejected: Vec<(&str, serde_json::Value)> = vec![
        ("缺 items", drop_key(&full, "items")),
        ("缺 total", drop_key(&full, "total")),
        (
            "total 非 u32",
            replace_key(&full, "total", serde_json::json!(-1)),
        ),
        (
            "items 非数组",
            replace_key(&full, "items", serde_json::json!({})),
        ),
    ];
    for (label, value) in rejected {
        let text = serde_json::to_string(&value).expect("构造样本应可序列化");
        let decoded: Result<NotificationPage, _> = serde_json::from_str(&text);
        assert!(decoded.is_err(), "{label}：页界信封不静默退化为空页");
    }
}

// ---------------------------------------------------------------------------
// NotificationFeedbackInput：落库入参
// ---------------------------------------------------------------------------

#[test]
fn notification_feedback_input四字段往返逐字相等() {
    let input = NotificationFeedbackInput {
        id: "0b5a2b8e-1c4d-4f2a-9d31-6e7f8a9b0c1d".to_owned(),
        level: NotificationLevel::Warning,
        message: "写契约文件完成".to_owned(),
        body: Some("写入 3 份".to_owned()),
    };

    let text = serde_json::to_string(&input).expect("序列化应成功");
    let decoded: NotificationFeedbackInput = serde_json::from_str(&text).expect("解码应成功");
    assert_eq!(decoded, input);

    let value = serde_json::to_value(&input).expect("序列化应成功");
    assert_eq!(value["id"], serde_json::json!(input.id));
    assert_eq!(value["level"], serde_json::json!("warning"));
    assert_eq!(value["message"], serde_json::json!(input.message));
    assert_eq!(value["body"], serde_json::json!("写入 3 份"));
}

#[test]
fn notification_feedback_input缺body键与null同解且空串可解码() {
    let without_body = serde_json::json!({
        "id": "id-1",
        "level": "info",
        "message": "提示",
    });
    let decoded: NotificationFeedbackInput =
        serde_json::from_value(without_body).expect("缺 body 键应解码成功");
    assert_eq!(decoded.body, None);

    let null_body = serde_json::json!({
        "id": "id-1",
        "level": "info",
        "message": "提示",
        "body": null,
    });
    let decoded: NotificationFeedbackInput =
        serde_json::from_value(null_body).expect("body: null 应解码成功");
    assert_eq!(decoded.body, None);

    let empty_strings = serde_json::json!({
        "id": "",
        "level": "success",
        "message": "",
        "body": "",
    });
    let decoded: NotificationFeedbackInput =
        serde_json::from_value(empty_strings).expect("空串可解码（词表层不拒）");
    assert_eq!(decoded.id, "");
    assert_eq!(decoded.message, "");
    assert_eq!(decoded.body, Some(String::new()));
}

#[test]
fn notification_feedback_input解码拒绝缺必填字段与非法枚举() {
    let full = serde_json::json!({
        "id": "id-1",
        "level": "info",
        "message": "提示",
        "body": null,
    });
    let rejected: Vec<(&str, serde_json::Value)> = vec![
        ("缺 id", drop_key(&full, "id")),
        ("缺 message", drop_key(&full, "message")),
        ("缺 level", drop_key(&full, "level")),
        (
            "level 非法",
            replace_key(&full, "level", serde_json::json!("debug")),
        ),
    ];
    for (label, value) in rejected {
        let decoded: Result<NotificationFeedbackInput, _> = serde_json::from_value(value);
        assert!(decoded.is_err(), "{label}：应拒绝（枚举封闭 + 三必填）");
    }
}

// ---------------------------------------------------------------------------
// 构造清单面（`include_str!("../Cargo.toml")` 结构锚）
// ---------------------------------------------------------------------------

/// 逐行取某段（下一个 `[` 段头前）的键行。
fn section_keys(manifest: &str) -> Vec<String> {
    let mut in_dependencies = false;
    let mut keys = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        keys.push(trimmed.to_owned());
    }
    keys
}

#[test]
fn 依赖面恰serde与specta两行且均取workspace() {
    assert_eq!(
        section_keys(include_str!("../Cargo.toml")),
        vec![
            "serde = { workspace = true }".to_owned(),
            "specta = { workspace = true }".to_owned(),
        ],
        "依赖面恰两行（边界中立的最小词表 crate）"
    );
}

#[test]
fn 依赖面禁用名零命中() {
    // 判据取依赖声明行（注释里的「禁 tokio」是边界声明本身，不是依赖）
    let declared = section_keys(include_str!("../Cargo.toml")).join("\n");
    // 禁用面：Tauri / tokio 运行时 / 库实现 / 任一 workspace 内部 crate（本 crate 自身除外）
    for banned in [
        "tauri",
        "tokio",
        "native_db",
        "native_model",
        "foundation",
        "workflow",
        "orchestration",
        "store",
        "checks",
    ] {
        assert!(!declared.contains(banned), "禁用依赖名零命中：{banned}");
    }

    let body = include_str!("lib.rs");
    assert!(!body.contains("async fn deliver"), "渠道缝禁 async");
    assert!(!body.contains("tokio::spawn"), "core 内零 spawn");
    assert!(!body.contains("use tauri"), "core 内零 tauri");
}

#[test]
fn 依赖面在内存副本注入tokio行即被同一判据检出() {
    // 负向样本驱动：只改内存副本，不动仓内文件（注入点必须是 `[dependencies]`
    // 段内——尾部追加会落进最后一个段，判据看不见）
    let manifest = include_str!("../Cargo.toml");
    let injected = manifest.replacen(
        "[dependencies]",
        "[dependencies]\ntokio = { workspace = true }",
        1,
    );
    assert_ne!(injected, manifest, "注入点在场（[dependencies] 段头）");
    let keys = section_keys(&injected);
    assert!(
        keys.iter().any(|key| key.starts_with("tokio")),
        "注入行进入 [dependencies] 段即被依赖面判据检出"
    );
    assert_ne!(keys.len(), 2, "两行判据对注入样本必须变红");
}

#[test]
fn 边界三句在案且lib_rs持三不变量() {
    // 三句边界写在清单头注释（依赖面的声明处）
    let manifest = include_str!("../Cargo.toml");
    for statement in ["禁 Tauri", "禁 tokio 运行时", "禁 workspace 内部依赖"] {
        assert!(
            manifest.contains(statement),
            "清单头注释缺边界声明：{statement}"
        );
    }

    // 三不变量写在 crate doc（不靠调用方自觉）
    let crate_doc = include_str!("lib.rs");
    for statement in ["三条不变量", "通知是旁路", "不递归", "只投一次"] {
        assert!(
            crate_doc.contains(statement),
            "crate doc 缺不变量：{statement}"
        );
    }
}

// ---------------------------------------------------------------------------
// serde_json 样本加工小件
// ---------------------------------------------------------------------------

fn drop_key(value: &serde_json::Value, key: &str) -> serde_json::Value {
    let mut object = value.as_object().expect("样本为对象").clone();
    object.remove(key);
    serde_json::Value::Object(object)
}

fn replace_key(
    value: &serde_json::Value,
    key: &str,
    replacement: serde_json::Value,
) -> serde_json::Value {
    let mut object = value.as_object().expect("样本为对象").clone();
    object.insert(key.to_owned(), replacement);
    serde_json::Value::Object(object)
}
