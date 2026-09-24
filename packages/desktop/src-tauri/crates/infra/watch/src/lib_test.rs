//! `watch` 的单元测试（AC-7 后端半）：`subscribe` 订阅组装、信号形态与
//! [`Watcher`] 生命周期句柄。tempfile 建临时目录与真实文件，真实触发文件系统
//! 事件（被监视文件不 mock）；mpsc `recv_timeout` 收信号，全部等待设超时上限
//! 防环境无事件时用例悬挂。不测 notify 库自身的事件语义，只测本 crate 的
//! 订阅组装、信号形态（仅 path 无内容）与 drop 即退订契约。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::{subscribe, FileWatchSignal};

/// 单次事件等待上限：真实文件系统事件延迟存在，超时即判失败（防悬挂）。
const SIGNAL_TIMEOUT: Duration = Duration::from_secs(5);

/// 临时目录 RAII + 目标文件路径。
struct TempFile {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl TempFile {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("watch-test-{tag}-"))
            .tempdir()
            .expect("创建临时目录失败");
        let path = dir.path().join("note.md");
        Self { _dir: dir, path }
    }

    /// 落盘初始文件并返回路径（订阅入参形态）。
    fn with_content(self, content: &str) -> Self {
        fs::write(&self.path, content).expect("写初始文件失败");
        self
    }

    fn path_str(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

/// 编辑器 rename-replace 原子保存形态：写临时文件后 rename 覆盖目标
/// （Windows 目标平台的真实保存路径，crate 文档平台边界的一致性前提）。
fn write_via_rename_replace(target: &Path, content: &str) {
    let staging = target.with_extension("md.tmp");
    fs::write(&staging, content).expect("写暂存文件失败");
    fs::rename(&staging, target).expect("rename 覆盖目标失败");
}

#[test]
fn 订阅已存在文件后rename_replace修改在超时窗口内收到仅含path的信号() {
    let target = TempFile::new("hit").with_content("# 初稿");
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();

    let watcher = subscribe(target.path.clone(), sender).expect("订阅应成功");

    write_via_rename_replace(&target.path, "# 修改后的内容 🎉");

    let signal = receiver
        .recv_timeout(SIGNAL_TIMEOUT)
        .expect("修改事件应在超时窗口内送达");
    assert_eq!(
        signal.path,
        target.path_str(),
        "信号 path 与被订阅路径（订阅时原样回传）一致"
    );
    drop(watcher);
}

#[test]
fn 目标不存在时订阅返回ok且文件后出现再修改信号生效送达() {
    let target = TempFile::new("late-created"); // 不落盘初始文件
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();

    let watcher =
        subscribe(target.path.clone(), sender).expect("目标缺失不报错（D5 未落盘照常订阅）");

    fs::write(&target.path, "后来才出现的笔记").expect("创建文件失败");
    let signal = receiver
        .recv_timeout(SIGNAL_TIMEOUT)
        .expect("目标（重）出现后事件应生效送达");
    assert_eq!(signal.path, target.path_str());
    drop(watcher);
}

#[test]
fn 信号序列化形态仅含path键无任何内容字节键() {
    // AC-7 通知无内容：serde 输出恰为 { "path": ... }，无 content / bytes 类键
    let signal = FileWatchSignal {
        path: "C:\\ws\\openspec\\explores\\foo.md".to_owned(),
    };

    let value = serde_json::to_value(&signal).expect("序列化应成功");

    let object = value.as_object().expect("载荷为 JSON 对象");
    let keys: Vec<&String> = object.keys().collect();
    assert_eq!(
        keys,
        vec!["path"],
        "信号结构仅含 path 字段，实际: {object:?}"
    );
    assert_eq!(object["path"], "C:\\ws\\openspec\\explores\\foo.md");
}

#[test]
fn watcher_drop后修改文件超时窗口内收不到任何信号() {
    let target = TempFile::new("drop-unsub").with_content("# 初稿");
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();

    let watcher = subscribe(target.path.clone(), sender).expect("订阅应成功");
    drop(watcher); // drop 即停流退订：notify 监听与通道发送端一起释放

    write_via_rename_replace(&target.path, "# 退订后的修改");

    // 先留出事件投递窗口，再确认窗口内无任何信号
    std::thread::sleep(Duration::from_millis(500));
    match receiver.recv_timeout(SIGNAL_TIMEOUT) {
        Err(mpsc::RecvTimeoutError::Timeout) => {}
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // 发送端已随 Watcher drop 关闭：同样证明无信号送达
        }
        Ok(signal) => panic!("退订后不应收到任何信号，实际: {signal:?}"),
    }
}

#[test]
fn 文件删除后重建再修改信号仍送达() {
    let target = TempFile::new("recreate").with_content("# 初稿");
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();

    let watcher = subscribe(target.path.clone(), sender).expect("订阅应成功");

    fs::remove_file(&target.path).expect("删除文件失败");
    fs::write(&target.path, "# 重建后的笔记").expect("重建文件失败");

    let signal = receiver
        .recv_timeout(SIGNAL_TIMEOUT)
        .expect("父目录挂载语义：删除/重建后事件不丢");
    assert_eq!(
        signal.path,
        target.path_str(),
        "重建后信号 path 仍指向订阅路径"
    );
    drop(watcher);
}

#[test]
fn 短窗口多次修改信号流就绪不丢不挂() {
    let target = TempFile::new("rapid").with_content("# 初稿");
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();

    let watcher = subscribe(target.path.clone(), sender).expect("订阅应成功");

    for round in 1..=3 {
        write_via_rename_replace(&target.path, &format!("# 第 {round} 次修改"));
    }

    // 本层契约是「转发不丢不挂」，合并属前端防抖职责：OS 可能合并相邻事件，
    // 故断言窗口内至少收到一帧且不悬挂（阻塞面由 recv_timeout 上限兜底）
    let mut received = 0;
    while let Ok(signal) = receiver.recv_timeout(Duration::from_secs(1)) {
        assert_eq!(signal.path, target.path_str());
        received += 1;
        if received == 3 {
            break;
        }
    }
    assert!(
        (1..=3).contains(&received),
        "短窗口多次修改：至少送达一帧、至多三帧（无悬挂无放大），实际 {received} 帧"
    );
    drop(watcher);
}
