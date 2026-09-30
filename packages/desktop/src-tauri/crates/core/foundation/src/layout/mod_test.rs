//! `layout` 的单元测试：常量组 + `resolve` / `config_path` 纯路径推导 +
//! 全包命名隔离双禁令扫描（AC-1 / AC-8）。

use std::fs;
use std::path::{Path, PathBuf};

use super::{
    config_path, domain_dir_name, resolve, ARCHIVE_DIR_NAME, CHANGES_DIR_NAME, CONFIG_FILE_NAME,
    DOMAIN_DIR_NAME, EXPLORES_DIR_NAME,
};

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
    assert_eq!(
        layout.archive_root.parent(),
        Some(layout.changes_root.as_path())
    );
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
    assert_eq!(
        layout.changes_root,
        PathBuf::from("openspec").join("changes")
    );
    assert_eq!(
        layout.archive_root,
        PathBuf::from("openspec").join("changes").join("archive")
    );
    assert_eq!(
        layout.explores_root,
        PathBuf::from("openspec").join("explores")
    );
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
// 命名隔离扫描：desktop 包全部产品源码不含磁盘目录名与配置文件名字面量
// （唯一触点为 layout/mod.rs，双禁令：openspec + config.json）
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
fn 全包产品源码双禁令扫描_openspec与config_json字面量_layout_为唯一例外() {
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
    // dev-team 根包源码树 src-tauri/src
    scan_roots.push(src_tauri.join("src"));

    // 双禁令：域目录名 + 配置文件名，任一字面量即违例；唯一例外 layout/mod.rs
    let mut scanned_total = 0usize;
    for forbidden in ["openspec", "config.json"] {
        let mut scanned = 0usize;
        // 唯一例外：layout/mod.rs 自身（磁盘名的唯一合法触点，末尾单独断言）
        let layout_mod = manifest.join("src").join("layout").join("mod.rs");
        for root in &scan_roots {
            let mut files = Vec::new();
            collect_rs_files(root, &mut files);
            for file in files {
                if file == layout_mod {
                    continue;
                }
                let text = fs::read_to_string(&file).unwrap_or_default();
                scanned += 1;
                if text.contains(forbidden) {
                    panic!(
                        "源文件 {file:?} 含被禁字面量 {forbidden:?}；磁盘目录名与配置文件名只允许出现在 foundation/src/layout/mod.rs"
                    );
                }
            }
        }
        scanned_total = scanned_total.max(scanned);

        // 唯一例外：layout/mod.rs 自身必须含该字面量且确实存在（双向断言）
        let layout_text = fs::read_to_string(&layout_mod).expect("layout/mod.rs 应存在");
        assert!(
            layout_text.contains(forbidden),
            "layout/mod.rs 应是 {forbidden:?} 的唯一触点"
        );
    }
    assert!(scanned_total > 0, "命名隔离扫描应至少覆盖一个源文件");
}

#[test]
fn 测试文件排除先例_被扫字面量在test文件自身合法且不入产品源码清单() {
    // 本文件（mod_test.rs）自身含 openspec 与 config.json 字面量（断言字面
    // 值），不属产品源码、不受禁令误伤；collect_rs_files 对 *_test.rs 的排除
    // 先例由本测试自指承载。
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let self_text = fs::read_to_string(manifest.join("src").join("layout").join("mod_test.rs"))
        .expect("应能读本测试文件");
    assert!(self_text.contains("openspec"));
    assert!(self_text.contains("config.json"));

    let mut files = Vec::new();
    collect_rs_files(&manifest.join("src"), &mut files);
    assert!(
        files.iter().all(|file| !file.ends_with("mod_test.rs")),
        "*_test.rs 应被排除在产品源码清单外"
    );
    assert!(
        files.iter().any(|file| file.ends_with("mod.rs")),
        "产品源码清单应含 layout/mod.rs"
    );
}

// ---------------------------------------------------------------------------
// 常量组：五员齐备且与既有磁盘名逐字一致（改名只动此处）
// ---------------------------------------------------------------------------

#[test]
fn 常量组五员与既有磁盘名逐字一致且archive相对changes锚定() {
    assert_eq!(DOMAIN_DIR_NAME, "openspec");
    assert_eq!(CHANGES_DIR_NAME, "changes");
    assert_eq!(ARCHIVE_DIR_NAME, "archive");
    assert_eq!(EXPLORES_DIR_NAME, "explores");
    assert_eq!(CONFIG_FILE_NAME, "config.json");
    // 公共通道：裸名消费方唯一来源，返回值与私有常量同源
    assert_eq!(domain_dir_name(), DOMAIN_DIR_NAME);
}

#[test]
fn resolve_三棵子树与常量组合路径逐字相等_体内全量常量引用的结构证明() {
    let temp = TempDir::new("const-composition");
    let layout = resolve(&temp.0);

    let domain = temp.0.join(DOMAIN_DIR_NAME);
    assert_eq!(
        layout.changes_root,
        domain.join(CHANGES_DIR_NAME),
        "changes 产物 = root/域目录/changes"
    );
    assert_eq!(
        layout.archive_root,
        domain.join(CHANGES_DIR_NAME).join(ARCHIVE_DIR_NAME),
        "archive 产物 = root/域目录/changes/archive（相对 changes 锚定）"
    );
    assert_eq!(
        layout.explores_root,
        domain.join(EXPLORES_DIR_NAME),
        "explores 产物 = root/域目录/explores"
    );
}

// ---------------------------------------------------------------------------
// config_path：纯拼接、无文件系统访问
// ---------------------------------------------------------------------------

#[test]
fn config_path等于root与域目录名和配置文件名的纯拼接_不存在root纯推导正常返回() {
    let ghost = std::env::temp_dir().join("foundation-layout-test-不存在的目录-config_path");
    // 前置：确保该目录确实不存在
    let _ = fs::remove_dir_all(&ghost);
    assert!(!ghost.exists(), "前置：root 不存在");

    // 纯推导：对不存在的 root 照常返回，无 IO
    assert_eq!(
        config_path(&ghost),
        ghost.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)
    );
}

#[test]
fn config_path空root与尾分隔符root与文件root纯拼接不panic() {
    // 空 root → 纯相对形式
    assert_eq!(
        config_path(Path::new("")),
        PathBuf::from(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)
    );

    // 尾分隔符 root：join 语义保持、不产生双分隔符
    let base = if cfg!(windows) { r"C:\tmp" } else { "/tmp" };
    let trailing = format!("{base}/");
    assert_eq!(
        config_path(Path::new(&trailing)),
        Path::new(&trailing).join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)
    );

    // 指向普通文件的 root：无目录类型校验，仍按路径拼接返回
    let temp = TempDir::new("config-path-file-root");
    let file_root = temp.0.join("一个普通文件.txt");
    fs::write(&file_root, "内容").expect("写文件失败");
    assert_eq!(
        config_path(&file_root),
        file_root.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)
    );
}

#[test]
fn config_path对同一输入结果稳定且父目录恰为resolve的域目录() {
    let temp = TempDir::new("config-path-invariants");

    // 确定性：同一输入结果稳定
    let first = config_path(&temp.0);
    let second = config_path(&temp.0);
    assert_eq!(first, second);

    // 锚定基准不变量：config_path 的父目录恰为域目录（= resolve 的 changes
    // 产物父目录），配置文件与 changes / explores 同域
    let layout = resolve(&temp.0);
    assert_eq!(
        first.parent(),
        Some(temp.0.join(DOMAIN_DIR_NAME).as_path())
    );
    assert_eq!(
        first.parent(),
        layout.changes_root.parent(),
        "config_path 与 changes / explores 同属一棵域树"
    );
    assert_eq!(
        layout.archive_root.parent(),
        Some(layout.changes_root.as_path()),
        "archive 产物的父目录恰为 changes 产物（锚定基准不变量）"
    );
}
