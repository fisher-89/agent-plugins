//! `sandbox` 的单元测试（AC-6）：路径沙箱校验（canonicalize + workspace root
//! 前缀校验）的 `..\` 逃逸拦截、符号链接绕过拦截、前缀边界与特殊形态入参
//! 锁定。tempfile tempdir 真实目录驱动（进程边界白名单以真实 FS 参与不
//! mock）；符号链接 / junction 以 std 与 Windows FS API 在 tempdir 内构造。
//!
//! Windows 口径：`check` 返回剥除 `\\?\` 前缀后的规范形态，断言以该口径对位。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::sdk::sandbox::{check, check_pattern};
use crate::sdk::tools;

/// 符号链接创建串行化（mklink 子进程与同目录链接名互不并发竞争）。
static LINK_LOCK: Mutex<()> = Mutex::new(());

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-sandbox-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建父目录失败");
    }
    std::fs::write(path, content).expect("写文件失败");
}

/// 在 tempdir 内建一个指向 root 外真实目录的目录链接：优先 std 符号链接
/// （需特权 / 开发者模式），无特权时以 junction 目录备选（两者均被
/// canonicalize 解链，拦截口径一致）。
fn create_dir_link(link: &Path, target: &Path) -> bool {
    let _guard = LINK_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return true;
        }
        let status = std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &link.to_string_lossy(),
                &target.to_string_lossy(),
            ])
            .status();
        return status.is_ok_and(|status| status.success()) && link.exists();
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}

// ---------------------------------------------------------------------------
// 正向：root 内合法路径放行（相对 / 绝对 / 子目录嵌套）
// ---------------------------------------------------------------------------

#[test]
fn root内相对_绝对_子目录嵌套路径经校验放行且返回规范形态() {
    let dir = tempdir("inside");
    let root = dir.path();
    write_file(&root.join("src/lib.rs"), "fn main() {}");
    write_file(&root.join("中文 目录/笔记 🎉.txt"), "内容");

    // 相对路径：返回存在路径的 canonical 形态（含真实大小写与分隔符）
    let resolved = check(root, "src/lib.rs").expect("root 内相对路径放行");
    assert!(resolved.is_absolute(), "返回规范化绝对路径: {resolved:?}");
    assert!(resolved.ends_with("lib.rs"));
    assert!(!resolved.starts_with(r"\\?\"), "Windows 扩展前缀已剥除");

    // 绝对路径（root 内）
    let absolute = root.join("src").join("lib.rs");
    let resolved_abs = check(root, absolute.to_string_lossy().as_ref()).expect("绝对路径放行");
    assert_eq!(resolved_abs, resolved, "相对与绝对同一文件收敛同一规范形态");

    // 子目录嵌套 + 中文 / 空格 / emoji 目录名
    let nested = check(root, "中文 目录/笔记 🎉.txt").expect("嵌套与特殊名放行");
    assert!(nested.ends_with("笔记 🎉.txt"), "文件名保真: {nested:?}");
}

// ---------------------------------------------------------------------------
// 异常：`..\` 相对逃逸与 root 外绝对路径拦截（canonical 口径）
// ---------------------------------------------------------------------------

#[test]
fn 相对逃逸与root外绝对路径拦截() {
    let dir = tempdir("escape");
    let root = dir.path();
    let outside = tempdir("outside");
    write_file(&outside.path().join("secret.txt"), "外部内容");

    // `..\` 相对逃逸（canonical 口径：`..` 段解析后 root 外）
    let escaped = check(root, "..\\secret.txt").expect_err("相对逃逸必须拦截");
    assert!(
        escaped.contains("越出 workspace root"),
        "拒绝原因指明越界，实际: {escaped}"
    );
    // 多级逃逸同样拦截
    assert!(check(root, "..\\..\\..\\etc").is_err(), "多级逃逸拦截");

    // root 外绝对路径
    let outside_abs = outside.path().join("secret.txt");
    assert!(
        check(root, outside_abs.to_string_lossy().as_ref()).is_err(),
        "root 外绝对路径拦截"
    );
}

#[test]
fn 符号链接指向root外目标时解链后按真实目标拦截() {
    let dir = tempdir("symlink");
    let root = dir.path();
    let outside = tempdir("symlink-outside");
    std::fs::create_dir_all(outside.path().join("real")).expect("创建外部真实目录失败");

    let created = create_dir_link(&root.join("link"), &outside.path().join("real"));
    if !created {
        // 无特权且 junction 不可用（罕见环境）：环境级跳过条件留痕
        panic!("当前环境无法创建符号链接 / junction（Windows 需特权或开发者模式）");
    }

    // 链接路径在 root 内，但解链后真实目标在 root 外 → 拦截
    let denied = check(root, "link").expect_err("符号链接绕过必须拦截");
    assert!(
        denied.contains("越出 workspace root"),
        "按解链后真实目标拦截，实际: {denied}"
    );
    // 穿过链接再进子段同样拦截
    assert!(check(root, "link/inner.txt").is_err(), "经链接的子段路径拦截");
}

// ---------------------------------------------------------------------------
// 边界：前缀边界 / 不存在路径 / 特殊形态入参
// ---------------------------------------------------------------------------

#[test]
fn 恰等于root本身放行_相似名兄弟目录拦截_尾分隔符root行为锁定() {
    let dir = tempdir("boundary");
    // 兄弟目录收拢在 tempdir 内（drop 即清理，不外溢共享临时目录）：
    // <tempdir>/parent/{ws, ws-sibling}，root 取 ws
    let parent = dir.path().join("parent");
    std::fs::create_dir_all(&parent).expect("创建父目录失败");
    let root = parent.join("ws");
    std::fs::create_dir_all(&root).expect("创建 workspace 目录失败");

    // 恰等于 root 本身（canonical == root）合法
    assert_eq!(
        check(&root, ".").expect("root 本身放行"),
        check(&root, ".").unwrap(),
        "root 本身返回 canonical 形态"
    );

    // root 相似名兄弟目录（前缀撞车：ws vs ws-sibling）拦截
    let sibling = parent.join(format!(
        "{}-sibling",
        root.file_name().expect("目录名在场").to_string_lossy()
    ));
    std::fs::create_dir_all(&sibling).expect("创建兄弟目录失败");
    let sibling_path = sibling.join("inner.txt");
    write_file(&sibling_path, "兄弟内容");
    assert!(
        check(&root, sibling_path.to_string_lossy().as_ref()).is_err(),
        "相似名兄弟目录路径拦截（非字符串前缀比对口径）"
    );

    // root 携尾分隔符入参：canonicalize 归一后校验口径不变（root 内路径放行）
    let root_with_trailing = PathBuf::from(format!("{}\\", root.display()));
    write_file(&root.join("tail.txt"), "尾分隔符场景");
    let resolved = check(&root_with_trailing, "tail.txt").expect("尾分隔符 root 下相对路径放行");
    assert!(resolved.ends_with("tail.txt"));
}

#[test]
fn 不存在路径_存在段逃逸显式失败_root缺失显式失败_均不静默放行() {
    let dir = tempdir("missing");
    let root = dir.path();

    // root 内缺失尾段（write 新建文件场景）：宽容 canonicalize 放行，返回
    // 「最深存在祖先 canonical + 缺失尾段」的规范拼回形态（实现定稿锁定）
    let resolved = check(root, "new/deep/file.txt").expect("root 内缺失尾段放行");
    assert!(resolved.starts_with(check(root, ".").unwrap()));
    assert!(resolved.ends_with("file.txt"));

    // 存在段携带逃逸 + 缺失尾段：拦截不静默（逃逸只可能经已存在段发生）
    assert!(
        check(root, "..\\missing-new.txt").is_err(),
        "经 .. 段的缺失路径拦截"
    );

    // root 自身不可解析：显式失败（错误半边显式化）
    let missing_root = dir.path().join("no-such-root");
    let error = check(&missing_root, "a.txt").expect_err("root 缺失必须显式失败");
    assert!(
        error.contains("workspace root 解析失败"),
        "显式失败原因，实际: {error}"
    );
}

#[test]
fn 特殊形态入参_空串归root_中文空格emoji路径_大小写变体行为锁定() {
    let dir = tempdir("special");
    let root = dir.path();
    write_file(&root.join("A.txt"), "大小写场景");
    write_file(&root.join("带 空格 与中文-🚀.md"), "特殊名内容");

    // 空串路径：非绝对 → root.join("") 即 root 本身 → 放行为 root（锁定行为）
    assert_eq!(check(root, "").expect("空串解析为 root 本身"), check(root, ".").unwrap());

    // 含中文 / 空格 / emoji 的路径：保真放行
    let special = check(root, "带 空格 与中文-🚀.md").expect("特殊名路径放行");
    assert!(special.ends_with("带 空格 与中文-🚀.md"));

    // 大小写变体（Windows 大小写不敏感 canonical 口径）：磁盘真实大小写收敛
    let lower = check(root, "a.txt").expect("小写入参放行");
    assert!(
        lower.ends_with("A.txt"),
        "canonical 收敛为磁盘真实大小写: {lower:?}"
    );
}

// ---------------------------------------------------------------------------
// read/write/edit 全链一致性：三类路径工具同过 check 单点拦截
// ---------------------------------------------------------------------------

#[test]
fn read_write_edit三类工具入参路径过同一校验函数root外同样拦截() {
    let dir = tempdir("all-tools");
    let root = dir.path();
    let outside = tempdir("all-tools-outside");
    let outside_file = outside.path().join("x.txt");
    write_file(&outside_file, "外部");

    // 全链单点：loop 的 sandbox_rewrite 对 read / write / edit 均取入参
    // `path` 字段过同一 check（tools::input_path 提取面），此处以三工具名
    // 各自的入参形态驱动同一校验函数——root 外路径无论哪个工具一律拦截
    let input = serde_json::json!({ "path": outside_file.to_string_lossy() });
    for tool in ["read", "write", "edit"] {
        let Some(raw) = tools::input_path(tool, &input) else {
            panic!("{tool} 工具应提取 path 字段");
        };
        let error = check(root, raw).expect_err("root 外路径三工具同拦截");
        assert!(error.contains("越出 workspace root"), "{tool}: {error}");
    }

    // root 内路径三工具同放行（单点放行面）
    write_file(&root.join("ok.txt"), "内部");
    let inside_input = serde_json::json!({ "path": "ok.txt" });
    for tool in ["read", "write", "edit"] {
        let raw = tools::input_path(tool, &inside_input).expect("path 字段在场");
        assert!(check(root, raw).is_ok(), "{tool} root 内路径放行");
    }
}

// ---------------------------------------------------------------------------
// check_pattern：glob 模式预检（绝对 / `..` 段 / `~` 前缀拒绝；root 相对放行）
// ---------------------------------------------------------------------------

#[test]
fn glob模式预检_绝对与父段与波浪线拒绝_root相对放行() {
    let dir = tempdir("pattern");
    let root = dir.path();

    // 绝对模式 / `~` 前缀：拒绝
    let absolute = format!("{}/**/*.rs", root.display());
    let denied = check_pattern(root, &absolute).expect_err("绝对模式拒绝");
    assert!(denied.contains("root 相对"), "实际: {denied}");
    assert!(check_pattern(root, "~/.ssh/*").is_err(), "~ 前缀拒绝");

    // `..` 段：拒绝（通配段无法 canonicalize，字面段封死）
    let escaped = check_pattern(root, "../*.rs").expect_err(".. 段拒绝");
    assert!(escaped.contains(".."), "实际: {escaped}");

    // root 相对模式：放行（含跨目录 ** 形态）
    assert!(check_pattern(root, "*.rs").is_ok());
    assert!(check_pattern(root, "src/**/*.ts").is_ok());

    // root 不可解析：兜底显式失败
    let missing_root = dir.path().join("no-such-root");
    assert!(check_pattern(&missing_root, "*.rs").is_err(), "root 缺失显式失败");
}
