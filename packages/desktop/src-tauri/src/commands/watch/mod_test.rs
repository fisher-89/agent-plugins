//! watch 命令轨道的单元测试（AC-7 壳层半）+ 「notify 单文件订阅 → WatchRegistry
//! → Channel 桥接」集成关系（R4：订阅-修改-推送-退订全程 / 幂等防叠加）。
//!
//! 装置：`tauri::test::mock_app()` manage 真实 `WatchRegistry`；被监视文件为
//! tempfile 真实文件触发真实 notify 事件（存储与文件系统层不 mock）；Tauri
//! `Channel` 以捕获回调收集 `InvokeResponseBody` 帧（沿 exec/mod_test.rs 惯例），
//! 断言帧数与 JSON 形态（仅 `path` 无内容键）。事件到达为异步桥接线程转发，
//! 全部等待设轮询上限防环境无事件时用例悬挂。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use super::{watch_subscribe, watch_unsubscribe, WatchRegistry};

/// 单次事件等待上限：真实文件系统事件 + 桥接线程转发，轮询至上限即判失败。
const WAIT_LIMIT: Duration = Duration::from_secs(5);
/// 「无后续帧」断言的静默窗口：窗口内零新增帧即认定退订生效。
const QUIET_WINDOW: Duration = Duration::from_millis(1200);

// ---------------------------------------------------------------------------
// 装置：mock_app + WatchRegistry + 捕获型 Channel + tempfile 真实文件
// ---------------------------------------------------------------------------

fn app_with_registry() -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    app.manage(WatchRegistry::default());
    app
}

fn registry_of(app: &App<tauri::test::MockRuntime>) -> tauri::State<'_, WatchRegistry> {
    app.state::<WatchRegistry>()
}

/// 捕获型 Channel：逐帧收下序列化 JSON（桥接线程序列快照）。
fn capturing_channel() -> (
    Channel<super::FileWatchEvent>,
    Arc<Mutex<Vec<serde_json::Value>>>,
) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&captured);
    let channel = Channel::new(move |body: InvokeResponseBody| {
        if let InvokeResponseBody::Json(text) = body {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                sink.lock().expect("捕获锁不可中毒").push(value);
            }
        }
        Ok(())
    });
    (channel, captured)
}

/// 被监视目标：tempfile 真实文件。
struct TempFile {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl TempFile {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("watch-cmd-test-{tag}-"))
            .tempdir()
            .expect("创建临时目录失败");
        let path = dir.path().join("note.md");
        Self { _dir: dir, path }
    }

    fn with_content(self, content: &str) -> Self {
        std::fs::write(&self.path, content).expect("写初始文件失败");
        self
    }

    fn modify(&self, content: &str) {
        std::fs::write(&self.path, content).expect("修改文件失败");
    }

    fn path_str(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

/// 轮询等待帧数达到 count（上限 WAIT_LIMIT，超时即失败）。
fn wait_frames(
    captured: &Arc<Mutex<Vec<serde_json::Value>>>,
    count: usize,
) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + WAIT_LIMIT;
    loop {
        let snapshot = captured.lock().expect("捕获锁不可中毒").clone();
        if snapshot.len() >= count {
            return snapshot;
        }
        if Instant::now() > deadline {
            panic!("等待 {count} 帧超时，实收 {} 帧", snapshot.len());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 收敛等待：直到 QUIET_WINDOW 内无新增帧（一次修改在 Windows 下可产生多条
/// notify 事件——Create/Modify/Data/Metadata 等各自成帧，桥接层不去重不合并，
/// 帧数断言一律以「收敛基线」为准，不假设恰一帧）。
fn settle(captured: &Arc<Mutex<Vec<serde_json::Value>>>) -> usize {
    let deadline = Instant::now() + WAIT_LIMIT;
    let mut baseline = captured.lock().expect("捕获锁不可中毒").len();
    while Instant::now() < deadline {
        std::thread::sleep(QUIET_WINDOW);
        let now = captured.lock().expect("捕获锁不可中毒").len();
        if now == baseline {
            return now;
        }
        baseline = now;
    }
    baseline
}

/// 静默窗口断言：QUIET_WINDOW 内无新增帧。
fn assert_no_new_frames(captured: &Arc<Mutex<Vec<serde_json::Value>>>, baseline: usize) {
    std::thread::sleep(QUIET_WINDOW);
    let now = captured.lock().expect("捕获锁不可中毒").len();
    assert_eq!(
        now, baseline,
        "静默窗口内不得有新增帧，实收 {now} 帧（基线 {baseline}）"
    );
}

/// 帧形态断言：仅 `path` 键、无任何内容字节键（AC-7 通知无内容）。
fn assert_frame_is_path_only(frame: &serde_json::Value, expected_path: &str) {
    let object = frame.as_object().expect("帧载荷为 JSON 对象");
    let keys: Vec<&String> = object.keys().collect();
    assert_eq!(keys, vec!["path"], "帧恰含 path 一个键，实际: {frame:?}");
    assert_eq!(object["path"], expected_path, "帧 path 与被订阅路径一致");
}

// ---------------------------------------------------------------------------
// watch_subscribe：订阅、信号形态、幂等、独立订阅（AC-7）
// ---------------------------------------------------------------------------

#[test]
fn 订阅返回id且修改后channel收到仅含path的信号帧() {
    let app = app_with_registry();
    let target = TempFile::new("hit").with_content("# 初稿");
    let (channel, captured) = capturing_channel();

    let id = watch_subscribe(registry_of(&app), channel, target.path_str()).expect("订阅应成功");

    target.modify("# 修改后的内容 🎉");

    // 一次修改可产生多条 notify 事件（桥接不去重）：至少一帧、每帧仅 path 键
    let frames = wait_frames(&captured, 1);
    for frame in &frames {
        assert_frame_is_path_only(frame, &target.path_str());
    }
    let _ = id;
}

#[test]
fn 同一路径重复订阅返回同一subscription_id且信号帧数与单订阅一致不翻倍() {
    let app = app_with_registry();
    let doubled = TempFile::new("idempotent").with_content("# 初稿");
    let control = TempFile::new("idempotent-ctrl").with_content("# 初稿");
    let (doubled_channel, doubled_captured) = capturing_channel();
    let (control_channel, control_captured) = capturing_channel();

    let first = watch_subscribe(
        registry_of(&app),
        doubled_channel.clone(),
        doubled.path_str(),
    )
    .expect("首次订阅应成功");
    let second = watch_subscribe(registry_of(&app), doubled_channel, doubled.path_str())
        .expect("重复订阅应成功");
    assert_eq!(
        first, second,
        "同一路径（canonical 键）幂等返回既有 id（D5）"
    );
    watch_subscribe(registry_of(&app), control_channel, control.path_str())
        .expect("对照订阅应成功");

    doubled.modify("# 双订阅文件的唯一修改");
    control.modify("# 单订阅文件的唯一修改");

    // 同一修改形态：双订阅与单订阅收敛帧数一致即未叠加（幂等防叠加，D5）
    let doubled_count = settle(&doubled_captured);
    let control_count = settle(&control_captured);
    assert!(doubled_count >= 1, "修改后至少送达一帧");
    assert_eq!(
        doubled_count, control_count,
        "重复订阅不叠加双桥接线程：帧数与单订阅一致（双 {doubled_count} vs 单 {control_count}）"
    );
}

#[test]
fn 目标文件不存在时订阅返回ok且文件出现后修改信号送达() {
    let app = app_with_registry();
    let target = TempFile::new("late"); // 不落盘初始文件
    let (channel, captured) = capturing_channel();

    let id = watch_subscribe(registry_of(&app), channel, target.path_str())
        .expect("目标缺失不报错（AC-7）");
    assert!(id > 0, "返回有效 subscription_id");

    std::fs::write(&target.path, "后来出现的笔记").expect("创建文件失败");

    let frames = wait_frames(&captured, 1);
    assert_frame_is_path_only(&frames[0], &target.path_str());
}

#[test]
fn 不同路径分别订阅各自独立id且事件互不串扰() {
    let app = app_with_registry();
    let first_target = TempFile::new("multi-a").with_content("# 甲");
    let second_target = TempFile::new("multi-b").with_content("# 乙");
    let (first_channel, first_captured) = capturing_channel();
    let (second_channel, second_captured) = capturing_channel();

    let first_id = watch_subscribe(registry_of(&app), first_channel, first_target.path_str())
        .expect("甲订阅应成功");
    let second_id = watch_subscribe(registry_of(&app), second_channel, second_target.path_str())
        .expect("乙订阅应成功");
    assert_ne!(first_id, second_id, "不同路径各自独立 subscription_id");

    first_target.modify("# 甲修改");

    let first_frames = wait_frames(&first_captured, 1);
    assert_frame_is_path_only(&first_frames[0], &first_target.path_str());
    assert!(
        second_captured.lock().expect("捕获锁不可中毒").is_empty(),
        "甲的修改不得串入乙的通道"
    );

    second_target.modify("# 乙修改");
    let second_frames = wait_frames(&second_captured, 1);
    assert_frame_is_path_only(&second_frames[0], &second_target.path_str());
}

// ---------------------------------------------------------------------------
// watch_unsubscribe：退订停流 + miss 幂等（AC-7）
// ---------------------------------------------------------------------------

#[test]
fn 以有效id退订返回true且此后修改channel无后续帧() {
    let app = app_with_registry();
    let target = TempFile::new("unsub").with_content("# 初稿");
    let (channel, captured) = capturing_channel();

    let id = watch_subscribe(registry_of(&app), channel, target.path_str()).expect("订阅应成功");
    target.modify("# 退订前修改");
    let baseline = settle(&captured);
    assert!(baseline >= 1, "退订前修改应产生信号帧");

    let unsubscribed = watch_unsubscribe(registry_of(&app), id).expect("退订应成功");
    assert!(unsubscribed, "有效 id 退订返回 true");

    target.modify("# 退订后修改");
    assert_no_new_frames(&captured, baseline);
}

#[test]
fn 未知id与重复退订均返回false幂等() {
    let app = app_with_registry();
    let target = TempFile::new("unsub-miss").with_content("# 初稿");
    let (channel, _captured) = capturing_channel();

    let id = watch_subscribe(registry_of(&app), channel, target.path_str()).expect("订阅应成功");

    assert!(
        !watch_unsubscribe(registry_of(&app), id + 999).expect("未知 id 不报错"),
        "未知 id 幂等 false（AC-7）"
    );
    assert!(watch_unsubscribe(registry_of(&app), id).expect("退订应成功"));
    assert!(
        !watch_unsubscribe(registry_of(&app), id).expect("重复退订不报错"),
        "重复退订幂等 false（AC-7）"
    );
}
