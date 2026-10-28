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
        Path::new(&trailing)
            .join(DOMAIN_DIR_NAME)
            .join(CONFIG_FILE_NAME)
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
    assert_eq!(first.parent(), Some(temp.0.join(DOMAIN_DIR_NAME).as_path()));
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

// ---------------------------------------------------------------------------
// change_test_reports 扩展节（desktop-checks-domain）
// ---------------------------------------------------------------------------

use super::{change_test_reports, change_test_reports_in, REPORTS_DIR_NAME, TEST_REPORTS_DIR_NAME};

/// 正向：root + change → change 目录树下 reports / test 两级拼接逐字相等
///（常量组同源引用的结构证明——resolve 的 changes 产物锚定）。
#[test]
fn change_test_reports_change目录树下reports_test两级拼接逐字相等() {
    let temp = TempDir::new("report-derive");
    let layout = resolve(&temp.0);
    let derived = change_test_reports(&temp.0, "demo-change");

    assert_eq!(
        derived,
        layout
            .changes_root
            .join("demo-change")
            .join(REPORTS_DIR_NAME)
            .join(TEST_REPORTS_DIR_NAME),
        "change 报告目录 = root/域目录/changes/<change>/reports/test（常量组同源）"
    );
    assert!(
        derived.starts_with(layout.changes_root.join("demo-change")),
        "change 目录锚定（报告树在 change 目录之下）"
    );
}

/// 正向：不存在 root / 指向文件的 root 纯推导正常返回无 IO（config_path
/// 先例同型）。
#[test]
fn change_test_reports_不存在root与文件root纯推导无io() {
    // 不存在 root：纯推导照常返回
    let ghost =
        std::env::temp_dir().join("foundation-layout-test-不存在的目录-change_test_reports");
    let _ = fs::remove_dir_all(&ghost);
    assert!(!ghost.exists(), "前置：root 不存在");
    let derived = change_test_reports(&ghost, "demo-change");
    assert_eq!(
        derived,
        ghost
            .join(DOMAIN_DIR_NAME)
            .join(CHANGES_DIR_NAME)
            .join("demo-change")
            .join(REPORTS_DIR_NAME)
            .join(TEST_REPORTS_DIR_NAME)
    );

    // 指向普通文件的 root：无目录类型校验，仍按路径拼接返回
    let temp = TempDir::new("report-file-root");
    let file_root = temp.0.join("一个普通文件.txt");
    fs::write(&file_root, "内容").expect("写文件失败");
    let derived = change_test_reports(&file_root, "demo-change");
    assert!(derived.starts_with(&file_root), "文件 root 纯拼接返回");
}

/// 边界：空 root → 纯相对形式；尾分隔符 root join 语义不产双分隔符、不 panic。
#[test]
fn change_test_reports_空root纯相对形式且尾分隔符不双分隔符() {
    // 空 root → 纯相对形式
    let derived = change_test_reports(Path::new(""), "demo-change");
    assert_eq!(
        derived,
        PathBuf::from(DOMAIN_DIR_NAME)
            .join(CHANGES_DIR_NAME)
            .join("demo-change")
            .join(REPORTS_DIR_NAME)
            .join(TEST_REPORTS_DIR_NAME)
    );

    // 尾分隔符 root：join 语义保持、不产生双分隔符
    let base = if cfg!(windows) { r"C:\tmp" } else { "/tmp" };
    let trailing = format!("{base}/");
    let derived = change_test_reports(Path::new(&trailing), "demo-change");
    let joined = derived.to_string_lossy();
    assert!(
        !joined.contains("//") && !joined.contains(r"\\"),
        "尾分隔符不应产生双分隔符: {joined}"
    );
}

/// 边界：同一输入结果稳定且父目录锚定 change 目录（确定性不变量）。
#[test]
fn change_test_reports_同输入稳定且父目录锚定change目录() {
    let temp = TempDir::new("report-deterministic");

    // 确定性：同一输入结果稳定
    let first = change_test_reports(&temp.0, "demo-change");
    let second = change_test_reports(&temp.0, "demo-change");
    assert_eq!(first, second);

    // 父目录锚定：报告目录的父 = change 目录下的 reports 目录
    assert_eq!(
        first.parent(),
        Some(
            temp.0
                .join(DOMAIN_DIR_NAME)
                .join(CHANGES_DIR_NAME)
                .join("demo-change")
                .join(REPORTS_DIR_NAME)
                .as_path()
        ),
        "reports 目录 = change 目录 / reports（两级常量组的上级）"
    );

    // 换 change 名 → 兄弟报告目录（互不重叠、同归 changes_root）
    let other = change_test_reports(&temp.0, "another-change");
    assert_ne!(first, other);
    let changes_root = resolve(&temp.0).changes_root;
    let change_dir = first
        .parent()
        .and_then(Path::parent)
        .expect("reports 有父目录");
    assert_eq!(
        change_dir.parent(),
        Some(changes_root.as_path()),
        "change 目录锚定 changes_root（报告树互不越树）"
    );
    let other_change_dir = other
        .parent()
        .and_then(Path::parent)
        .expect("reports 有父目录");
    assert_eq!(
        other_change_dir.parent(),
        Some(changes_root.as_path()),
        "同 root 下不同 change 的报告树同归 changes_root"
    );
}

/// 边界：reports / test 两员常量逐字锚定（改名只动此处——既有常量组测试
/// 扩行的独立新测试承载）。
#[test]
fn 报告目录常量组两员逐字锚定() {
    assert_eq!(REPORTS_DIR_NAME, "reports");
    assert_eq!(TEST_REPORTS_DIR_NAME, "test");
}

/// `change_test_reports_in`：change 目录 → reports / test 两级后缀（与
/// `change_test_reports` 同后缀单点——change 目录由三态定位面供给，任意锚
/// 派生的报告树同构）。
#[test]
fn change_test_reports_in_change目录后缀与root变体同构() {
    use super::change_test_reports_in;

    let change_dir = PathBuf::from("wt-root")
        .join(DOMAIN_DIR_NAME)
        .join(CHANGES_DIR_NAME)
        .join("demo");
    assert_eq!(
        change_test_reports_in(&change_dir),
        change_dir
            .join(REPORTS_DIR_NAME)
            .join(TEST_REPORTS_DIR_NAME),
        "change 目录变体 = 目录直拼 reports/test 后缀"
    );
    // 与 root + change 变体同构（后缀单点——两派生共享同一拼接）
    let temp = TempDir::new("report-in-derive");
    let via_root = change_test_reports(&temp.0, "demo");
    let via_dir = change_test_reports_in(&resolve(&temp.0).changes_root.join("demo"));
    assert_eq!(via_root, via_dir, "root 变体与 change 目录变体同构");
}

// ---------------------------------------------------------------------------
// 报告槽位与槽位驱动报告目录（check-before-flow-start）：ReportSlot 封闭二值 +
// change_reports 单点 + baseline / test 命名变体委托等值（Mock策略：无——纯
// 路径推导直测，零 IO 断言面沿既有 layout 测试形态）
// ---------------------------------------------------------------------------

use super::{
    change_baseline_reports, change_baseline_reports_in, change_reports, change_reports_in,
    ReportSlot, BASELINE_REPORTS_DIR_NAME,
};

/// 正向（枚举遍历）：`ReportSlot` 二值封闭构造互异——`Test != Baseline`；
/// `Debug` / `Clone` / `Copy` / `PartialEq` / `Eq` 派生可用（纯进程内枚举零
/// serde / specta——不进 IPC）。
#[test]
fn report_slot二值封闭构造互异且派生可用() {
    // 二值封闭 + 互异
    assert_ne!(ReportSlot::Test, ReportSlot::Baseline, "两槽位可辨");
    assert_eq!(ReportSlot::Test, ReportSlot::Test, "PartialEq 自反");

    // Copy 语义（槽位值语义传递——消费后原值不耗损）
    let slot = ReportSlot::Baseline;
    let copied = slot;
    assert_eq!(copied, slot, "Copy 后等值");
    let _use_twice = (slot, slot);

    // Clone / Debug 派生可用
    let cloned = slot;
    assert_eq!(cloned, ReportSlot::Baseline);
    assert!(!format!("{slot:?}").is_empty(), "Debug 出线非空");
}

/// 正向（枚举遍历）：`change_reports()` 槽位驱动派生——slot=Test →
/// `<root>/openspec/changes/<change>/reports/test`、slot=Baseline →
/// `…/reports/baseline`（两值逐字对拍——槽位单点派生，后缀经
/// `report_dir_name(slot)` 单点映射）。
#[test]
fn change_reports两槽位路径逐字对拍() {
    let temp = TempDir::new("slot-derive");
    let layout = resolve(&temp.0);

    let test_dir = change_reports(&temp.0, "demo-change", ReportSlot::Test);
    let baseline_dir = change_reports(&temp.0, "demo-change", ReportSlot::Baseline);

    assert_eq!(
        test_dir,
        layout
            .changes_root
            .join("demo-change")
            .join(REPORTS_DIR_NAME)
            .join(TEST_REPORTS_DIR_NAME),
        "slot=Test → reports/test（常量组同源拼接）"
    );
    assert_eq!(
        baseline_dir,
        layout
            .changes_root
            .join("demo-change")
            .join(REPORTS_DIR_NAME)
            .join(BASELINE_REPORTS_DIR_NAME),
        "slot=Baseline → reports/baseline（常量组同源拼接）"
    );
    assert_ne!(test_dir, baseline_dir, "两槽位报告树分树隔离（零混居）");

    // 字面量再锚（防常量误改后测试随之失效）
    assert!(
        test_dir.ends_with("reports/test"),
        "test 槽位后缀逐字 reports/test（组件语义）: {}",
        test_dir.display()
    );
    assert!(
        baseline_dir.ends_with("reports/baseline"),
        "baseline 槽位后缀逐字 reports/baseline（组件语义）: {}",
        baseline_dir.display()
    );
}

/// 正向（枚举遍历）：`change_reports_in()` change 目录形态同构——
/// `<change_dir>/reports/test` / `<change_dir>/reports/baseline`，且与 root 变体
/// 后缀同源（同 slot 两变体仅前缀差异断言）。
#[test]
fn change_reports_in_change目录形态与root变体同构() {
    let temp = TempDir::new("slot-in-derive");
    let change_dir = resolve(&temp.0).changes_root.join("demo");

    for slot in [ReportSlot::Test, ReportSlot::Baseline] {
        let via_root = change_reports(&temp.0, "demo", slot);
        let via_dir = change_reports_in(&change_dir, slot);
        assert_eq!(via_root, via_dir, "slot={slot:?} 两变体同构（仅前缀差异）");
        assert!(
            via_dir.starts_with(&change_dir),
            "change 目录变体锚定 change_dir 之下（slot={slot:?}）"
        );
    }

    // 后缀逐字（目录直拼形态）
    assert_eq!(
        change_reports_in(&change_dir, ReportSlot::Baseline),
        change_dir
            .join(REPORTS_DIR_NAME)
            .join(BASELINE_REPORTS_DIR_NAME),
        "change 目录形态 = 目录直拼 reports/baseline 后缀"
    );
}

/// 正向（单次）：命名变体委托等值——`change_baseline_reports()` /
/// `change_baseline_reports_in()` 与 `change_reports(.., Baseline)` /
/// `change_reports_in(.., Baseline)` 逐字节相等（委托单点防漂移）。
#[test]
fn change_baseline_reports命名变体委托等值() {
    let temp = TempDir::new("baseline-derive");
    let change_dir = resolve(&temp.0).changes_root.join("demo");

    assert_eq!(
        change_baseline_reports(&temp.0, "demo"),
        change_reports(&temp.0, "demo", ReportSlot::Baseline),
        "baseline 命名变体与槽位版逐字节相等（root 形态）"
    );
    assert_eq!(
        change_baseline_reports_in(&change_dir),
        change_reports_in(&change_dir, ReportSlot::Baseline),
        "baseline 命名变体与槽位版逐字节相等（change 目录形态）"
    );
    assert!(
        change_baseline_reports(&temp.0, "demo").ends_with("reports/baseline"),
        "baseline 变体后缀逐字（组件语义）"
    );
}

/// 正向（单次）：既有签名回归持衡——`change_test_reports()` /
/// `change_test_reports_in()` 返回值与本变更前 `reports/test` 路径逐字节一致
///（改委托零路径漂移——既有断言面更新而非退役）；与 `change_reports(.., Test)`
/// 等值。
#[test]
fn change_test_reports改委托零路径漂移() {
    let temp = TempDir::new("test-derive");
    let change_dir = resolve(&temp.0).changes_root.join("demo");

    assert_eq!(
        change_test_reports(&temp.0, "demo"),
        change_reports(&temp.0, "demo", ReportSlot::Test),
        "test 命名变体与槽位版逐字节相等（root 形态）"
    );
    assert_eq!(
        change_test_reports_in(&change_dir),
        change_reports_in(&change_dir, ReportSlot::Test),
        "test 命名变体与槽位版逐字节相等（change 目录形态）"
    );
    // 本变更前逐字节路径（字面量拼接锚——改委托零漂移）
    assert_eq!(
        change_test_reports(&temp.0, "demo"),
        temp.0
            .join(DOMAIN_DIR_NAME)
            .join(CHANGES_DIR_NAME)
            .join("demo")
            .join("reports")
            .join("test"),
        "reports/test 字面量拼接逐字节一致（既有调用点零感知）"
    );
}

/// 边界（单次）：`change_reports()` 纯推导无 IO 不 panic（沿 `config_path` /
/// `change_test_reports` 先例形态）——不存在 root / root 指向文件 / 空 root；
/// 同输入重复调用等值（确定性）；尾分隔符 root 不产双分隔符。
#[test]
fn change_reports边界_纯推导无io不panic且确定性() {
    // 不存在 root：纯推导照常返回
    let ghost = std::env::temp_dir().join("foundation-layout-test-不存在的目录-change_reports");
    let _ = fs::remove_dir_all(&ghost);
    assert!(!ghost.exists(), "前置：root 不存在");
    let derived = change_reports(&ghost, "demo", ReportSlot::Baseline);
    assert_eq!(
        derived,
        ghost
            .join(DOMAIN_DIR_NAME)
            .join(CHANGES_DIR_NAME)
            .join("demo")
            .join(REPORTS_DIR_NAME)
            .join(BASELINE_REPORTS_DIR_NAME)
    );

    // 指向普通文件的 root：无目录类型校验，仍按路径拼接返回
    let temp = TempDir::new("slot-file-root");
    let file_root = temp.0.join("一个普通文件.txt");
    fs::write(&file_root, "内容").expect("写文件失败");
    let derived = change_reports(&file_root, "demo", ReportSlot::Baseline);
    assert!(derived.starts_with(&file_root), "文件 root 纯拼接返回");

    // 空 root → 纯相对形式
    let derived = change_reports(Path::new(""), "demo", ReportSlot::Baseline);
    assert_eq!(
        derived,
        PathBuf::from(DOMAIN_DIR_NAME)
            .join(CHANGES_DIR_NAME)
            .join("demo")
            .join(REPORTS_DIR_NAME)
            .join(BASELINE_REPORTS_DIR_NAME)
    );

    // 确定性：同输入重复调用等值
    let temp = TempDir::new("slot-deterministic");
    let first = change_reports(&temp.0, "demo", ReportSlot::Baseline);
    let second = change_reports(&temp.0, "demo", ReportSlot::Baseline);
    assert_eq!(first, second, "同输入重复调用等值");

    // 尾分隔符 root：join 语义保持、不产生双分隔符
    let base = if cfg!(windows) { r"C:\tmp" } else { "/tmp" };
    let trailing = format!("{base}/");
    let derived = change_reports(Path::new(&trailing), "demo", ReportSlot::Baseline);
    let joined = derived.to_string_lossy();
    assert!(
        !joined.contains("//") && !joined.contains(r"\\"),
        "尾分隔符不应产生双分隔符: {joined}"
    );
}

/// 边界（单次）：超长 change 名（>1000 字符）与特殊字符名（emoji / `\n`）原样
/// 拼接零清洗零 panic（清洗权威在 create 前置——纯拼接负向锚）。
#[test]
fn change_reports超长与特殊字符change名零清洗零panic() {
    let temp = TempDir::new("slot-exotic-name");

    // 超长名原样拼接
    let long_name = "c".repeat(1200);
    let derived = change_reports(&temp.0, &long_name, ReportSlot::Baseline);
    let rendered = derived.to_string_lossy();
    assert!(
        rendered.contains(&long_name),
        "超长 change 名原样拼接零截断"
    );
    assert!(
        derived.ends_with("reports/baseline"),
        "超长名不破坏后缀拼接"
    );

    // 特殊字符名原样拼接
    let exotic = "突🚀变\n名";
    let derived = change_reports(&temp.0, exotic, ReportSlot::Baseline);
    let rendered = derived.to_string_lossy();
    assert!(
        rendered.contains(exotic),
        "特殊字符名原样拼接零清洗: {rendered}"
    );
    assert!(derived.ends_with("reports/baseline"));
}

/// 边界（枚举遍历）：报告目录常量锚——`BASELINE_REPORTS_DIR_NAME == "baseline"`
/// 逐字锚定 + 既有 `reports` / `test` 常量组锚随动；`report_dir_name(slot)` 两臂
/// 穷尽映射（Test→test / Baseline→baseline 无第三后缀——私有单点函数同 crate
/// 子模块直读）。
#[test]
fn 报告目录常量锚与report_dir_name两臂穷尽映射() {
    use super::report_dir_name;

    assert_eq!(BASELINE_REPORTS_DIR_NAME, "baseline", "baseline 后缀逐字");
    assert_eq!(
        REPORTS_DIR_NAME, "reports",
        "reports 后缀逐字（既有组锚随动）"
    );
    assert_eq!(
        TEST_REPORTS_DIR_NAME, "test",
        "test 后缀逐字（既有组锚随动）"
    );
    assert_ne!(
        TEST_REPORTS_DIR_NAME, BASELINE_REPORTS_DIR_NAME,
        "两槽位后缀互异（分树隔离的常量前提）"
    );

    // 两臂穷尽映射（无第三后缀——封闭二值枚举 match 穷尽）
    assert_eq!(report_dir_name(ReportSlot::Test), TEST_REPORTS_DIR_NAME);
    assert_eq!(
        report_dir_name(ReportSlot::Baseline),
        BASELINE_REPORTS_DIR_NAME
    );
}

// ---------------------------------------------------------------------------
// 架构域路径派生（`archi` 章节）：三函数同源 + `is_archi_rel_path` 判定
// ---------------------------------------------------------------------------

/// 正向：`archi_dir` / `archi_models_dir` / `archi_decisions_dir` 三者以
/// `archi_dir` 为共同前缀（同源断言：后两者 == `archi_dir.join(…)`）。
#[test]
fn archi_dir族三函数同源且逐段逐字() {
    use super::{archi_decisions_dir, archi_dir, archi_models_dir};

    let temp = TempDir::new("archi-dirs");
    let dir = archi_dir(&temp.0);
    assert_eq!(
        dir,
        temp.0.join(".dev-team").join("architecture"),
        "架构域根 = <root>/.dev-team/architecture"
    );
    assert_eq!(
        archi_models_dir(&temp.0),
        dir.join("models"),
        "模型目录从 archi_dir 派生"
    );
    assert_eq!(
        archi_decisions_dir(&temp.0),
        dir.join("decisions"),
        "决策记录目录从 archi_dir 派生"
    );
    assert!(
        archi_models_dir(&temp.0).starts_with(&dir)
            && archi_decisions_dir(&temp.0).starts_with(&dir),
        "三者以 archi_dir 为共同前缀"
    );
}

/// 边界：纯推导零 IO——调用后目录未被创建；不存在的 root 亦正常返回。
#[test]
fn archi_dir族纯推导零io与不存在root() {
    use super::{archi_decisions_dir, archi_dir, archi_models_dir};

    let temp = TempDir::new("archi-pure");
    let _ = archi_dir(&temp.0);
    let _ = archi_models_dir(&temp.0);
    let _ = archi_decisions_dir(&temp.0);
    assert!(
        fs::metadata(temp.0.join(".dev-team")).is_err(),
        "纯推导不得创建目录（零 IO 的机械证据）"
    );

    let missing = Path::new("/nonexistent/root");
    assert_eq!(
        archi_dir(missing),
        missing.join(".dev-team").join("architecture"),
        "不存在的 root 亦正常返回"
    );
}

/// 边界：`root` 形态五类——空路径 / 相对路径 / Windows 盘符 / 尾分隔符 / 超长 ——
/// 逐字节等于 `root.join(段名)` 口径、不 panic。
#[test]
fn archi_dir族root形态五类() {
    use super::{archi_dir, archi_models_dir};

    let empty = Path::new("");
    assert_eq!(
        archi_dir(empty),
        empty.join(".dev-team").join("architecture"),
        "空路径返回相对形式（分隔符由 join 决定，跨平台同准）"
    );
    assert_eq!(
        archi_models_dir(Path::new("demo")),
        Path::new("demo")
            .join(".dev-team")
            .join("architecture")
            .join("models"),
        "相对路径逐字节拼接"
    );
    let windows = Path::new(r"C:\demo\alpha");
    assert_eq!(
        archi_models_dir(windows),
        windows
            .join(".dev-team")
            .join("architecture")
            .join("models"),
        "Windows 盘符形态逐字节拼接"
    );
    let trailing = Path::new(r"C:\demo\alpha\");
    assert_eq!(
        archi_models_dir(trailing),
        trailing
            .join(".dev-team")
            .join("architecture")
            .join("models"),
        "尾分隔符形态逐字节拼接"
    );
    let long = PathBuf::from("x".repeat(1200));
    assert_eq!(
        archi_dir(&long),
        long.join(".dev-team").join("architecture"),
        "超长路径不 panic 且逐字节拼接"
    );
}

/// 正向：`is_archi_rel_path` 命中集——架构域顶层段 + 架构域段（含更深路径与祖先判定）。
#[test]
fn is_archi_rel_path命中集() {
    use super::is_archi_rel_path;

    assert!(is_archi_rel_path(
        ".dev-team/architecture/models/02-auth.toml"
    ));
    assert!(
        is_archi_rel_path(".dev-team/architecture"),
        "祖先判定（无 tail 段亦命中）"
    );
    assert!(is_archi_rel_path(".dev-team/architecture/models"));
}

/// 正向：归一形态——反斜杠与 `./` 前缀均归一后命中。
#[test]
fn is_archi_rel_path归一形态() {
    use super::is_archi_rel_path;

    assert!(
        is_archi_rel_path(r".dev-team\architecture\models\x.toml"),
        "Windows 反斜杠归一后命中"
    );
    assert!(
        is_archi_rel_path("./.dev-team/architecture/models/x.toml"),
        "`./` 前缀归一后命中"
    );
}

/// 异常：负样本——changes / 旧路径 / 段 0 非架构域顶层 / 前缀相近的兄弟段均不命中。
#[test]
fn is_archi_rel_path负样本() {
    use super::is_archi_rel_path;

    assert!(!is_archi_rel_path("openspec/changes/x/specs/y.md"));
    assert!(
        !is_archi_rel_path("openspec/architecture/models/x.toml"),
        "旧落点不再命中（架构域已与 openspec 域分树）"
    );
    assert!(
        !is_archi_rel_path(".dev-team/archived/a.toml"),
        "不做前缀模糊匹配"
    );
    assert!(
        !is_archi_rel_path("src/architecture/x.rs"),
        "段 0 非架构域顶层"
    );
    assert!(!is_archi_rel_path(".dev-team/architecturex/y"));
}

/// 边界：空串 / `/` / 双斜杠空段 / 无顶层前缀 / 超长 → `false` 且不 panic。
#[test]
fn is_archi_rel_path边界形态() {
    use super::is_archi_rel_path;

    for rel in [
        "",
        "/",
        ".dev-team//architecture",
        "architecture",
        ".dev-team",
        "openspec",
    ] {
        assert!(!is_archi_rel_path(rel), "{rel:?} 不应命中（段位判定）");
    }
    let long = format!(".dev-team/architecture/{}", "x".repeat(1200));
    assert!(is_archi_rel_path(&long), "超长路径不 panic 且命中");
}

/// 边界：大小写面——逐字节比较，不做大小写归一。
#[test]
fn is_archi_rel_path大小写敏感() {
    use super::is_archi_rel_path;

    assert!(
        !is_archi_rel_path(".DEV-TEAM/Architecture/models/x.toml"),
        "首字母大写不命中（逐字节比较）"
    );
    assert!(!is_archi_rel_path(".dev-team/Architecture/x.toml"));
}

// ---------------------------------------------------------------------------
// 架构域**相对路径**派生族（`archi_rel_dir` / `archi_models_rel_dir` /
// `archi_decisions_rel_dir`）：repo 相对、`/` 分隔、纯推导零 IO、不接 root 入参
// ——唯一消费者为架构会话知识文本（模型可见文本）。
// ---------------------------------------------------------------------------

/// 正向：三函数前缀同源——后两者皆以 `archi_rel_dir()` 为前缀，尾段恰为
/// `models` / `decisions`（不互相串位）。
#[test]
fn archi_rel族前缀同源且尾段不串位() {
    use super::{archi_decisions_rel_dir, archi_models_rel_dir, archi_rel_dir};

    let base = archi_rel_dir();
    let models = archi_models_rel_dir();
    let decisions = archi_decisions_rel_dir();

    for (label, rel) in [("models", &models), ("decisions", &decisions)] {
        assert!(
            rel.starts_with(&base),
            "{label} 相对路径应以 archi_rel_dir() 为前缀（实得 {rel}）"
        );
    }
    assert!(
        models.ends_with("/models"),
        "模型目录尾段恰为 models（实得 {models}）"
    );
    assert!(
        decisions.ends_with("/decisions"),
        "决策记录目录尾段恰为 decisions（实得 {decisions}）"
    );
    assert_ne!(models, decisions, "两子目录不互相串位");
    assert!(
        !models.contains("decisions") && !decisions.contains("models"),
        "子目录名不交叉出现"
    );
}

/// 正向：与绝对族一致——`Path::new(root).join(相对路径)` 与绝对族逐段相等
/// （root 取三形态：绝对 tempdir 路径 / 尾分隔符 root / 空 root）。
#[test]
fn archi_rel族与绝对族join一致三形态root() {
    use super::{
        archi_decisions_dir, archi_decisions_rel_dir, archi_dir, archi_models_dir,
        archi_models_rel_dir, archi_rel_dir,
    };

    let absolute = std::env::temp_dir().join("foundation-layout-archi-rel-绝对");
    let trailing = {
        let mut text = absolute.to_string_lossy().into_owned();
        text.push(std::path::MAIN_SEPARATOR);
        PathBuf::from(text)
    };
    let empty = PathBuf::new();

    for root in [absolute, trailing, empty] {
        assert_eq!(
            root.join(archi_rel_dir()),
            archi_dir(&root),
            "root = {root:?}: join(archi_rel_dir()) == archi_dir(root)"
        );
        assert_eq!(
            root.join(archi_models_rel_dir()),
            archi_models_dir(&root),
            "root = {root:?}: join(archi_models_rel_dir()) == archi_models_dir(root)"
        );
        assert_eq!(
            root.join(archi_decisions_rel_dir()),
            archi_decisions_dir(&root),
            "root = {root:?}: join(archi_decisions_rel_dir()) == archi_decisions_dir(root)"
        );
        // 逐段相等（strip_prefix 后段数与段名全同）
        let absolute_models = archi_models_dir(&root);
        let stripped: Vec<_> = absolute_models
            .strip_prefix(&root)
            .expect("绝对族落在 root 之下")
            .components()
            .collect();
        let relative_text = archi_models_rel_dir();
        let relative: Vec<_> = Path::new(&relative_text).components().collect();
        assert_eq!(
            stripped, relative,
            "root = {root:?}: 相对路径逐段等于绝对族剥前缀结果"
        );
    }
}

/// 边界：分隔符与前后缀——零 `\`（`/` 分隔）、`archi_rel_dir()` 恰 1 个 `/`、
/// 两子目录恰 2 个；零前导 `./`、零尾 `/`、零连续 `//`、零空段。
#[test]
fn archi_rel族分隔符与前后缀() {
    use super::{archi_decisions_rel_dir, archi_models_rel_dir, archi_rel_dir};

    let base = archi_rel_dir();
    let models = archi_models_rel_dir();
    let decisions = archi_decisions_rel_dir();

    for (label, rel) in [
        ("archi_rel_dir", &base),
        ("archi_models_rel_dir", &models),
        ("archi_decisions_rel_dir", &decisions),
    ] {
        assert!(!rel.contains('\\'), "{label}: 应为 `/` 分隔（实得 {rel}）");
        assert!(
            !rel.starts_with("./") && !rel.starts_with('/'),
            "{label}: 零前导 ./ 与零前导分隔符（实得 {rel}）"
        );
        assert!(!rel.ends_with('/'), "{label}: 零尾分隔符（实得 {rel}）");
        assert!(!rel.contains("//"), "{label}: 零连续分隔符（实得 {rel}）");
        assert!(
            rel.split('/').all(|segment| !segment.is_empty()),
            "{label}: 零空段（实得 {rel}）"
        );
    }
    assert_eq!(
        base.matches('/').count(),
        1,
        "架构域根相对路径恰两段（实得 {base}）"
    );
    assert_eq!(
        models.matches('/').count(),
        2,
        "模型目录相对路径恰三段（实得 {models}）"
    );
    assert_eq!(
        decisions.matches('/').count(),
        2,
        "决策记录目录相对路径恰三段（实得 {decisions}）"
    );
}

/// 异常防护：常量组派生——首段 == 架构域顶层目录名 `.dev-team`（与 `openspec`
/// 域根分树，MUST NOT 相等交叉），第二段 == `archi_dir(root)` 的末段名（架构域
/// 段名唯一持有者在常量组，函数体内零行内字面量）。
#[test]
fn archi_rel族常量组派生两段() {
    use super::{archi_dir, archi_rel_dir, domain_dir_name};

    let base = archi_rel_dir();
    let segments: Vec<&str> = base.split('/').collect();
    assert_eq!(segments.len(), 2, "架构域根相对路径恰两段（实得 {base}）");
    assert_eq!(
        segments[0], ".dev-team",
        "首段即架构域顶层目录名（常量值钉死，防漂移）"
    );
    assert_ne!(
        segments[0],
        domain_dir_name(),
        "架构域与 `openspec` 域分树（MUST NOT 嵌套于域根）"
    );

    let absolute = archi_dir(Path::new("工作区"));
    let archi_segment = absolute
        .file_name()
        .and_then(|name| name.to_str())
        .expect("架构域根有末段名");
    assert_eq!(
        segments[1], archi_segment,
        "第二段即 archi_dir 的末段名（架构域段名唯一持有者）"
    );
}

/// 边界：零 IO / 纯函数——三函数无 root 入参（零实参调用即签名证明），连续三次
/// 调用逐字相等，不存在的磁盘状态下同样成立（零 `exists()` 调用面）。
#[test]
fn archi_rel族零io纯函数() {
    use super::{archi_decisions_rel_dir, archi_models_rel_dir, archi_rel_dir};

    let first = (
        archi_rel_dir(),
        archi_models_rel_dir(),
        archi_decisions_rel_dir(),
    );
    let second = (
        archi_rel_dir(),
        archi_models_rel_dir(),
        archi_decisions_rel_dir(),
    );
    let third = (
        archi_rel_dir(),
        archi_models_rel_dir(),
        archi_decisions_rel_dir(),
    );
    assert_eq!(first, second, "同参两次调用逐字相等");
    assert_eq!(second, third, "第三次调用同样逐字相等");
    // 零 IO：函数不触盘、不查存在性（签名即全部输入——三个函数均无 root 入参，
    // 上行的零实参调用即编译期证明），返回值只在进程内做字符串拼接
    assert_eq!(
        first.0.len(),
        first.1.len() - "/models".len(),
        "子目录派生 = 基路径 + 固定尾段（零状态依赖）"
    );
}

/// 正向：与既有判据口径一致——三形态相对路径均被 `is_archi_rel_path` 命中
/// （派生族与判据单点互证）。
#[test]
fn archi_rel族与is_archi_rel_path互证() {
    use super::{archi_decisions_rel_dir, archi_models_rel_dir, archi_rel_dir, is_archi_rel_path};

    for (label, rel) in [
        ("archi_rel_dir", archi_rel_dir()),
        ("archi_models_rel_dir", archi_models_rel_dir()),
        ("archi_decisions_rel_dir", archi_decisions_rel_dir()),
    ] {
        assert!(
            is_archi_rel_path(&rel),
            "{label} 应命中架构域判据（实得 {rel}）"
        );
    }
}

/// 回归：既有绝对族四函数逐字不变（`archi_dir` / `archi_models_dir` /
/// `archi_decisions_dir` / `is_archi_rel_path` 的派生口径与判据零漂移）——新增三
/// 个相对路径函数不改既有面。
#[test]
fn archi既有绝对族零回归() {
    use super::{archi_decisions_dir, archi_dir, archi_models_dir, is_archi_rel_path};

    let root = Path::new("C:\\工作区\\alpha");
    let dir = archi_dir(root);
    assert_eq!(dir, root.join(".dev-team").join("architecture"));
    assert_eq!(archi_models_dir(root), dir.join("models"));
    assert_eq!(archi_decisions_dir(root), dir.join("decisions"));
    assert!(is_archi_rel_path(".dev-team/architecture/models/01.toml"));
    assert!(!is_archi_rel_path("openspec/archived/changes"));
}
