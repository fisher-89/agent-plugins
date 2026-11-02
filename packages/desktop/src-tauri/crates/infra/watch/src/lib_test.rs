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

// ---------------------------------------------------------------------------
// 父目录缺失的显式失败面（change-detail-live-stdout，AC-10 / D2）
//
// 被测面：`subscribe` 在**父目录亦缺失**时返回 `Err(WatchError::ParentUnavailable`
// `(parent))`（不再静默 `Ok`）；三条 MUST NOT：不替调用方造目录、不递归挂祖先、
// 不引入退避重试 / 重挂状态机；恢复路径是调用方的顺序不变量（先建记录再暴露
// 可订阅地址）——补齐父目录后**重新订阅**才 `Ok`。
//
// Mock 策略（见 test-design 本节 Mock策略表）：零替身——真实 tempfile 临时目录
// + 真实文件系统事件（notify 不被 mock，其语义不在断言面）；mpsc
// `recv_timeout` 收信号并设超时上限防挂起。
// ---------------------------------------------------------------------------

use crate::WatchError;

/// 临时根 RAII（父目录缺失 / 补齐 / 嵌套三形态的落位面）。
struct TempRoot {
    dir: tempfile::TempDir,
}

impl TempRoot {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("watch-parent-test-{tag}-"))
            .tempdir()
            .expect("创建临时目录失败");
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// 取 `Err` 臂（`Watcher` 零 `Debug`，故不用 `expect_err`）。
fn expect_unavailable(result: Result<crate::Watcher, WatchError>, label: &str) -> WatchError {
    match result {
        Ok(_) => panic!("{label}: 应为 Err(ParentUnavailable)，实际 Ok"),
        Err(error) => error,
    }
}

/// 正向：父目录缺失 → `Err(ParentUnavailable(parent))`，`parent` 为缺失父目录
/// 的路径（不是目标自身）、`Display` 带「目标父目录不存在」与路径。
#[test]
fn 父目录缺失显式err且携带缺失父目录() {
    let root = TempRoot::new("missing");
    let parent = root.path().join("absent-dir");
    let target: PathBuf = parent.join("exec.log");
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();

    let error = expect_unavailable(subscribe(target.clone(), sender), "父目录缺失");
    match &error {
        WatchError::ParentUnavailable(carried) => assert_eq!(
            *carried, parent,
            "错误携带缺失父目录（非目标自身）: {carried:?}"
        ),
        other => panic!("应为 ParentUnavailable，实际: {other:?}"),
    }
    let rendered = error.to_string();
    assert!(
        rendered.contains("目标父目录不存在"),
        "Display 文案: {rendered}"
    );
    assert!(
        rendered.contains(&parent.to_string_lossy().to_string()),
        "Display 携带父目录路径: {rendered}"
    );
    // 零信号（订阅未建立）
    assert!(
        receiver.recv_timeout(Duration::from_millis(200)).is_err(),
        "订阅失败面不产任何信号"
    );
}

/// 异常（MUST NOT 一）：`Err` 臂零 fs 副作用——父目录仍不存在、目标文件仍不
/// 存在（不替调用方造目录）。
#[test]
fn err臂零fs副作用不造目录() {
    let root = TempRoot::new("no-mkdir");
    let parent = root.path().join("nested").join("absent");
    let target = parent.join("exec.log");
    let (sender, _receiver) = mpsc::channel::<FileWatchSignal>();

    assert!(
        subscribe(target.clone(), sender).is_err(),
        "父目录缺失应 Err"
    );
    assert!(!parent.exists(), "不替调用方造目录（父目录仍缺失）");
    assert!(!target.exists(), "不替调用方造目标文件");
    let entries: Vec<String> = fs::read_dir(root.path())
        .expect("读临时根")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(entries.is_empty(), "零新增条目: {entries:?}");
}

/// 异常（MUST NOT 二）：递归挂祖先——祖父目录存在而父目录缺失 → 仍 `Err`
///（MUST NOT 上溯到祖父挂载，「监听会在目标出现后生效」的乐观 `Ok` 不再返回）。
#[test]
fn 不递归挂祖先_祖父存在仍err() {
    let root = TempRoot::new("no-ancestor");
    let grandparent = root.path().join("present-grandparent");
    fs::create_dir_all(&grandparent).expect("建祖父目录");
    let parent = grandparent.join("absent-parent");
    let target = parent.join("exec.log");
    let (sender, _receiver) = mpsc::channel::<FileWatchSignal>();

    let error = expect_unavailable(subscribe(target, sender), "父目录缺失（上溯）");
    match &error {
        WatchError::ParentUnavailable(carried) => assert_eq!(*carried, parent),
        other => panic!("应为 ParentUnavailable，实际: {other:?}"),
    }
    assert!(!parent.exists(), "祖父在而父目录仍缺失");
}

/// 异常（MUST NOT 三）：零退避重试 / 零自愈——连调三次均 `Err`（不因前次失败
/// 改变后次形态），补齐父目录**后**才 `Ok`（恢复靠重新订阅，不靠 crate 自旋）。
#[test]
fn 连续err不重试_补齐后重新订阅才ok() {
    let root = TempRoot::new("recover");
    let parent = root.path().join("late-parent");
    let target = parent.join("exec.log");

    for round in 1..=3 {
        let (sender, _receiver) = mpsc::channel::<FileWatchSignal>();
        let error = expect_unavailable(subscribe(target.clone(), sender), "连续 Err 面");
        assert!(matches!(error, WatchError::ParentUnavailable(_)));
        assert!(!parent.exists(), "第 {round} 次 err 后父目录仍缺失");
    }

    // 调用方顺序不变量：先建目录（写入方责任）
    fs::create_dir_all(&parent).expect("补齐父目录");
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();
    let watcher = subscribe(target.clone(), sender).expect("补齐后重新订阅应 Ok");

    // 目标文件出现（模拟「先建记录再暴露可订阅地址」）→ 信号生效
    fs::write(&target, "first line\n").expect("落盘目标文件");
    let signal = receiver
        .recv_timeout(SIGNAL_TIMEOUT)
        .expect("补齐并重订后信号应生效");
    assert_eq!(signal.path, target.to_string_lossy().into_owned());
    drop(watcher);
}

/// 边界（参数化 2 形态）：目标文件缺失而父目录在 → 仍 `Ok`（语义零变）；
/// 父目录恰为深嵌套尾级（`a/b/c/d`，前三级已建、末级缺）→ `Err` 且携带末级。
#[test]
fn 目标缺失ok与深嵌套末级缺失err() {
    // 形态一：父目录在、目标文件缺 → Ok（既有语义零变）
    let root = TempRoot::new("target-missing");
    let parent = root.path().join("present");
    fs::create_dir_all(&parent).expect("建父目录");
    let target = parent.join("not-yet.log");
    let (sender, _receiver) = mpsc::channel::<FileWatchSignal>();
    let watcher = subscribe(target, sender).expect("目标文件缺失应 Ok（语义零变）");
    drop(watcher);

    // 形态二：深嵌套末级缺失 → Err 且携带末级
    let deep_root = TempRoot::new("deep");
    let prefix = deep_root.path().join("a").join("b").join("c");
    fs::create_dir_all(&prefix).expect("建前三级");
    let missing = prefix.join("d");
    let target = missing.join("exec.log");
    let (sender, _receiver) = mpsc::channel::<FileWatchSignal>();
    let error = expect_unavailable(subscribe(target, sender), "深嵌套末级缺失");
    match &error {
        WatchError::ParentUnavailable(carried) => {
            assert_eq!(*carried, missing, "携带末级父目录");
            assert_ne!(*carried, prefix, "不携带已存在的祖父（不递归挂祖先）");
        }
        other => panic!("应为 ParentUnavailable，实际: {other:?}"),
    }
}

/// 边界：零长度父路径（相对裸文件名 `exec.log`，无父）→ 按「根路径无父时回退
/// 目标自身」的既有口径 `Err(ParentUnavailable(target))`（不 panic）。
#[test]
fn 无父路径回退目标自身() {
    let (sender, _receiver) = mpsc::channel::<FileWatchSignal>();
    let target = PathBuf::from("bare-name.log");
    let error = expect_unavailable(subscribe(target.clone(), sender), "无父相对路径");
    match &error {
        WatchError::ParentUnavailable(carried) => {
            assert_eq!(*carried, target, "根路径无父时回退目标自身（既有口径）")
        }
        other => panic!("应为 ParentUnavailable，实际: {other:?}"),
    }
}

/// 正向：`Display` / `Debug` 两形态——`Notify` 变体文案带 `watch:` 前缀、
/// `ParentUnavailable` 文案带路径；`Error` trait 可装箱（`Box<dyn Error>`）。
#[test]
fn 错误两面文案与error_trait() {
    let parent = PathBuf::from("C:\\ws\\openspec\\changes\\c\\reports\\test\\exec-outputs");
    let unavailable = WatchError::ParentUnavailable(parent.clone());
    assert_eq!(
        unavailable.to_string(),
        format!("watch: 目标父目录不存在 {}", parent.display()),
        "ParentUnavailable 文案定式"
    );
    let boxed: Box<dyn std::error::Error> = Box::new(WatchError::ParentUnavailable(parent));
    assert!(
        boxed.to_string().contains("目标父目录不存在"),
        "Error trait 对象文案保持"
    );
}

/// 异常：`WatchError::Notify` 臂仍可构造且 `Display` 文案逐字 `"watch: {notify 错误}"`
/// （既有文案零变）；`std::error::Error` impl 在位（可装箱为 `Box<dyn Error>`）。
#[test]
fn notify变体display文案与error_trait() {
    let notify_error = notify::Error::generic("inotify watch limit reached");
    let rendered = format!("watch: {notify_error}");
    let error = WatchError::Notify(notify::Error::generic("inotify watch limit reached"));
    assert_eq!(
        error.to_string(),
        rendered,
        "Notify 变体文案定式 `watch: {{notify 错误}}`"
    );
    assert!(
        error.to_string().starts_with("watch: "),
        "文案带 `watch: ` 前缀"
    );
    let boxed: Box<dyn std::error::Error> = Box::new(error);
    assert!(
        boxed.to_string().contains("inotify watch limit reached"),
        "Error trait 对象文案保持"
    );
    // 两变体的 `Debug` 均可打印（壳层错误映射的前置面，零 panic）
    let debug = format!(
        "{:?}",
        WatchError::ParentUnavailable(PathBuf::from("missing-parent"))
    );
    assert!(debug.contains("ParentUnavailable"), "Debug 面可读: {debug}");
}

/// 边界：目标本身是**目录**（父目录在）→ 返回 `Ok` 且零 panic（本 crate 不做路径
/// 类型校验，只按父目录挂监听；此臂只锁「不 panic、不新造语义」）。
#[test]
fn 目标为目录_返回ok且零panic() {
    let root = TempRoot::new("target-dir");
    let parent = root.path().join("reports");
    fs::create_dir_all(&parent).expect("建父目录");
    let target_dir = parent.join("exec-outputs");
    fs::create_dir_all(&target_dir).expect("建目标目录");

    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();
    let watcher = subscribe(target_dir.clone(), sender).expect("目标为目录不报错（父目录在）");

    // 目录内落一个文件：父目录挂载语义下不 panic、不误判为 Err 即可
    fs::write(target_dir.join("exec-1.log"), "line\n").expect("目录内写文件");
    // 该信号可能命中（路径不同则被过滤）也可能不命中——契约只锁「零 panic」
    let _ = receiver.recv_timeout(Duration::from_millis(300));
    drop(watcher);
    assert!(target_dir.is_dir(), "订阅零 fs 副作用（目录仍在）");
}

/// 异常（既有失败面零变）：目标被删除后**再订阅**（同一路径、父目录仍在）→
/// 仍返回 `Ok`（不新造 `Err` 变体、不退化为 panic）；随后重建目标并修改 → 信号
/// 照常送达（「删除后再订阅」与既有「删除重建」两臂互不牵连）。
#[test]
fn 目标删除后再订阅仍ok且信号可达() {
    let target = TempFile::new("delete-resubscribe").with_content("# 初稿");
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();
    let watcher = subscribe(target.path.clone(), sender).expect("首订应 Ok");

    // 删除目标后仍在同一订阅下重建（既有语义：父目录挂载，事件不丢）
    fs::remove_file(&target.path).expect("删除目标文件");
    let _ = receiver.recv_timeout(Duration::from_millis(200));
    drop(watcher);

    // 目标缺席状态下**再订阅**同一路径（父目录在）：仍 `Ok`
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();
    let watcher = subscribe(target.path.clone(), sender).expect("目标删除后再订阅应仍 Ok");

    fs::write(&target.path, "# 再订后重建\n").expect("重建目标文件");
    let signal = receiver
        .recv_timeout(SIGNAL_TIMEOUT)
        .expect("再订阅后目标重建+写入应送达信号");
    assert_eq!(signal.path, target.path_str(), "信号 path 为订阅路径");
    drop(watcher);
}

/// 边界（参数化 2 形态）：空路径 `PathBuf::new()` 与 `"."`（当前目录）→ 均返回
/// `Err(ParentUnavailable(_))` 而非 panic（`parent_of` 的「根路径无父回退目标
/// 自身」与「`"."` 组件无可挂父目录」两分支）。
#[test]
fn 空路径与当前目录两形态_显式err不panic() {
    for (label, target) in [("空路径", PathBuf::new()), ("当前目录", PathBuf::from("."))] {
        let (sender, _receiver) = mpsc::channel::<FileWatchSignal>();
        let error = expect_unavailable(subscribe(target.clone(), sender), label);
        match &error {
            WatchError::ParentUnavailable(carried) => {
                assert_eq!(*carried, target, "{label}: 回退目标自身（既有口径）")
            }
            other => panic!("{label}: 应为 ParentUnavailable，实际: {other:?}"),
        }
        assert!(
            !error.to_string().is_empty(),
            "{label}: Display 文案非空（可出线给前端）"
        );
    }
}
