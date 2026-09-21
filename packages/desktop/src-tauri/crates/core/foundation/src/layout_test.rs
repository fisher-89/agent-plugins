//! `layout::resolve` 的单元测试：纯路径推导 + 全包命名隔离扫描（AC-2）。

use std::fs;
use std::path::{Path, PathBuf};

use crate::layout::resolve;

/// 临时目录 RAII：测试结束自动清理。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "foundation-layout-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn 存在的_root_返回以_root为前缀的三棵子树() {
    let temp = TempDir::new("existing-root");
    let layout = resolve(&temp.0);

    // 三棵子树都落在 root 之下、指向各自领域目录
    assert!(layout.changes_root.starts_with(&temp.0));
    assert!(layout.archive_root.starts_with(&temp.0));
    assert!(layout.explores_root.starts_with(&temp.0));
    assert_eq!(layout.archive_root.parent(), Some(layout.changes_root.as_path()));
    assert_eq!(
        layout.explores_root.parent(),
        layout.changes_root.parent(),
        "explores 与 changes 同属一棵领域树"
    );

    // 三棵子树各自指向不同目录
    assert_ne!(layout.changes_root, layout.explores_root);
    assert_ne!(layout.archive_root, layout.explores_root);
    // archive 物理上位于 changes 子树之内
    assert!(layout.archive_root.starts_with(&layout.changes_root));
}

#[test]
fn 不存在的_root_纯推导正常返回() {
    let ghost = std::env::temp_dir().join("foundation-layout-test-不存在的目录-π");
    // 前置：确保该目录确实不存在
    let _ = fs::remove_dir_all(&ghost);
    let layout = resolve(&ghost);

    assert!(layout.changes_root.starts_with(&ghost));
    assert!(layout.archive_root.starts_with(&ghost));
    assert!(layout.explores_root.starts_with(&ghost));
}

#[test]
fn root_指向文件而非目录时仍正常拼接路径() {
    let temp = TempDir::new("root-is-file");
    let file_root = temp.0.join("一个普通文件.txt");
    fs::write(&file_root, "内容").expect("写文件失败");

    // resolve 无目录类型校验：仍按路径拼接返回
    let layout = resolve(&file_root);
    assert!(layout.changes_root.starts_with(&file_root));
    assert!(layout.explores_root.starts_with(&file_root));
}

#[test]
fn root_为空路径时返回相对形式且不_panic() {
    let layout = resolve(Path::new(""));
    // 空 root → 纯相对形式的三路径
    assert_eq!(layout.changes_root, PathBuf::from("openspec").join("changes"));
    assert_eq!(
        layout.archive_root,
        PathBuf::from("openspec").join("changes").join("archive")
    );
    assert_eq!(layout.explores_root, PathBuf::from("openspec").join("explores"));
}

#[test]
fn root_带尾部分隔符或重复分隔符时join不追加新分隔符() {
    let base = if cfg!(windows) { r"C:\tmp" } else { "/tmp" };
    // 单个尾部分隔符：join 不再追加 → 不产生双分隔符
    let trailing = format!("{base}/");
    let layout = resolve(Path::new(&trailing));
    let joined = layout.changes_root.to_string_lossy();
    assert!(
        !joined.contains("//") && !joined.contains(r"\\"),
        "尾分隔符不应产生双分隔符: {joined}"
    );

    // 重复分隔符：resolve 为纯拼接、不做归一化，原样保留且不 panic
    let sep = std::path::MAIN_SEPARATOR;
    let doubled_text = format!("{base}{sep}{sep}");
    let layout2 = resolve(Path::new(&doubled_text));
    assert!(
        layout2.changes_root.starts_with(&doubled_text),
        "重复分隔符按 Path 拼接语义保留: {:?}",
        layout2.changes_root
    );
}

#[test]
fn resolve_对同一输入结果稳定且可比较() {
    let temp = TempDir::new("deterministic");
    let a = resolve(&temp.0);
    let b = resolve(&temp.0);
    assert_eq!(a, b);
    assert_eq!(a.changes_root, b.changes_root);
}

// ---------------------------------------------------------------------------
// 命名隔离扫描：desktop 包全部源码不含磁盘目录名字面量（唯一触点为 layout.rs）
// ---------------------------------------------------------------------------

/// 递归收集目录下全部 `.rs` 文件。
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let Some(name) = name else { continue };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if name.ends_with(".rs") && !name.ends_with("_test.rs") {
            // 排除 *_test.rs：测试文件自身包含被扫描的字面量，不属产品源码
            out.push(path);
        }
    }
}

#[test]
fn 全包源码不含_openspec_字样_layout_为唯一例外() {
    // foundation 的 manifest 目录：…/src-tauri/crates/core/foundation
    const DEPTH_TO_SRC_TAURI: usize = 3;
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut src_tauri = manifest.clone();
    for _ in 0..DEPTH_TO_SRC_TAURI {
        src_tauri.pop();
    }
    // packages/desktop（前端包根）
    let mut desktop_pkg = src_tauri.clone();
    desktop_pkg.pop();

    let forbidden = "openspec";

    let mut scan_roots: Vec<PathBuf> = Vec::new();
    // 前端源码树 packages/desktop/src
    scan_roots.push(desktop_pkg.join("src"));
    // Rust crate 源码树 src-tauri/crates/*/src
    let crates_dir = src_tauri.join("crates");
    if let Ok(entries) = fs::read_dir(&crates_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                scan_roots.push(entry.path().join("src"));
            }
        }
    }
    // desktop-app 根包源码树 src-tauri/src
    scan_roots.push(src_tauri.join("src"));

    let mut scanned = 0usize;
    // 唯一例外：layout.rs 自身（磁盘目录名的唯一合法触点，末尾单独断言）
    let layout_rs = manifest.join("src").join("layout.rs");
    for root in &scan_roots {
        let mut files = Vec::new();
        collect_rs_files(root, &mut files);
        for file in files {
            if file == layout_rs {
                continue;
            }
            let text = fs::read_to_string(&file).unwrap_or_default();
            scanned += 1;
            if text.contains(forbidden) {
                panic!(
                    "源文件 {file:?} 含被禁字面量 {forbidden:?}；磁盘目录名只允许出现在 foundation/src/layout.rs"
                );
            }
        }
    }

    // 唯一例外：layout.rs 自身必须含该字面量且确实存在
    let layout_text = fs::read_to_string(&layout_rs).expect("layout.rs 应存在");
    assert!(
        layout_text.contains(forbidden),
        "layout.rs 应是唯一的磁盘目录名触点"
    );
    assert!(scanned > 0, "命名隔离扫描应至少覆盖一个源文件");
}
