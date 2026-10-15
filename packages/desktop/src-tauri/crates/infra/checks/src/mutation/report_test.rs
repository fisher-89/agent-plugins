//! `report.rs` 的单元测试（test-design「report.rs -> report_test.rs」节）：
//! 段级与归并报告写盘（工具原始输出原样留存、`<i>` 序号连续、归并块 schema
//! 零扩展）+ 段级复用门（AC-8：在位 + mtime ≥ 输入最新 → 复用零 spawn；缺失
//! / 损坏 / 报告旧于输入 → 失效重跑，不 panic 不 Err）+ 清理边界（只清
//! `_stryker-tmp` 与临时 config，报告工件不触碰）。
//!
//! Mock策略（test-design 本节 Mock 表）：文件系统经真实 tempdir（mtime 经
//! `File::set_modified` 可设）；零旁置状态文件（E6 判据借段报告自身 mtime）；
//! 零 spawn（复用门语义 = 零调用，以调用计数缺席承载）。

use std::fs;
use std::path::Path;

use checks::model::{MutationBlock, MutationMeasured};
use tempfile::TempDir;

use super::{
    cleanup_stryker_artifacts, merged_block_path, mtime_ms, segment_patch_name, segment_patch_path,
    segment_report_name, segment_report_path, segment_reusable, stryker_config_name, to_posix,
    write_merged_block, write_segment_patch, write_segment_report, STRYKER_TMP_DIR_NAME,
};

/// 段报告 fixture（工具原始输出原样——含未知键与特殊字符，写盘不重排不丢）。
const SEG_RAW: &str = r#"{"mutants":[{"outcome":"Caught","file":"src/文件 🚀.rs"}],"duration_ms":1.5,"unknownKey":{"nested":[1,2]}}"#;

/// tempdir 装置（每用例独立）。
fn mutation_dir() -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("checks-mutation-report-test-")
        .tempdir()
        .expect("创建临时目录失败");
    let mutation = dir.path().join("mutation");
    (dir, mutation)
}

/// 文件 mtime 设定（复用门判据注入面——Unix / Windows 双侧 std API）。
fn set_mtime(path: &Path, millis_since_epoch: u64) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("打开文件设 mtime 失败");
    let time =
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(millis_since_epoch);
    file.set_modified(time).expect("set_modified 失败");
}

/// 正向：段报告写盘 = 工具原始输出原样（不重排不丢字段）；路径命名 `<i>`
/// 序号连续（`mutation-seg-<i>.json` / `.patch`）。
#[test]
fn 段报告写盘_原样留存与序号连续() {
    let (_dir, mutation) = mutation_dir();
    for index in 0..3 {
        let path = write_segment_report(&mutation, index, SEG_RAW).expect("段报告写盘应成功");
        assert_eq!(
            path.file_name().expect("文件名在场").to_string_lossy(),
            format!("mutation-seg-{index}.json"),
            "段报告命名 <i> 序号连续"
        );
        let read_back = fs::read_to_string(&path).expect("读回段报告");
        assert_eq!(read_back, SEG_RAW, "工具原始输出原样（不重排不丢字段）");
    }

    // 段补丁命名同序
    let patch_path =
        write_segment_patch(&mutation, 1, "diff --git a/x b/x\n").expect("段补丁写盘应成功");
    assert_eq!(
        patch_path
            .file_name()
            .expect("文件名在场")
            .to_string_lossy(),
        "mutation-seg-1.patch",
        "段补丁命名同序"
    );
    assert_eq!(
        fs::read_to_string(&patch_path).expect("读回段补丁"),
        "diff --git a/x b/x\n"
    );
}

/// 正向：归并块写盘 = `MutationBlock` 形态（schema 零扩展——serde 往返等值）；
/// 段报告全程留存不被覆盖（各归各文件）。
#[test]
fn 归并块写盘_schema零扩展且段报告不被覆盖() {
    let (_dir, mutation) = mutation_dir();
    write_segment_report(&mutation, 0, SEG_RAW).expect("段报告写盘应成功");

    let block = MutationBlock {
        pass: true,
        score: 92.5,
        threshold: 80.0,
        measured: MutationMeasured {
            killed: 8,
            survived: 1,
            timeout: 0,
            no_coverage: 0,
            compile_error: 0,
            runtime_error: 0,
            ignored: 1,
            total: 10,
            detected: 8,
            undetected: 1,
        },
        overrides: None,
    };
    let path = write_merged_block(&mutation, &block).expect("归并块写盘应成功");
    assert_eq!(
        path.file_name().expect("文件名在场").to_string_lossy(),
        "mutation.json"
    );

    // serde 往返等值（schema 零扩展——既有字段面）
    let text = fs::read_to_string(&path).expect("读回归并块");
    let back: MutationBlock = serde_json::from_str(&text).expect("归并块可回读");
    assert_eq!(back, block, "归并块 serde 往返等值（schema 零扩展）");

    // 段报告留存（不被归并块覆盖）
    assert_eq!(
        fs::read_to_string(segment_report_path(&mutation, 0)).expect("段报告仍在"),
        SEG_RAW,
        "段报告全程留存不被覆盖"
    );
}

/// 复用门正向（AC-8）：段报告在位且 mtime ≥ 段输入最新 mtime → 复用（零
/// spawn——以复用门真值承载）；`newest_input = None`（无输入扫描面）→ 在位
/// 即复用。
#[test]
fn 复用门_在位且新鲜则复用() {
    let (_dir, mutation) = mutation_dir();
    let path = write_segment_report(&mutation, 0, SEG_RAW).expect("段报告写盘应成功");

    // 报告 mtime = 10_000；输入最新 = 5_000（报告更新）→ 复用
    set_mtime(&path, 10_000);
    assert!(
        segment_reusable(&path, Some(5_000)),
        "报告 mtime ≥ 输入最新 → 复用（零 spawn）"
    );

    // newest_input = None → 在位即复用（无输入扫描面）
    assert!(segment_reusable(&path, None), "输入扫描缺席 → 在位即复用");

    // 相等边界：mtime 相等 → 复用（`>=` 判据）
    set_mtime(&path, 7_000);
    assert!(
        segment_reusable(&path, Some(7_000)),
        "mtime 相等 → 复用（>= 边界）"
    );
}

/// 复用门失效（异常）：段报告缺失 / 损坏（非 JSON）→ 失效（不 panic 不 Err
/// ——调用方据此重跑）。
#[test]
fn 复用门失效_缺失与损坏不panic() {
    let (_dir, mutation) = mutation_dir();

    // 缺失
    assert!(
        !segment_reusable(&segment_report_path(&mutation, 0), None),
        "段报告缺失 → 失效重跑（不 panic 不 Err）"
    );

    // 损坏（非 JSON）
    let path = segment_report_path(&mutation, 0);
    fs::create_dir_all(&mutation).expect("建目录");
    fs::write(&path, "not json{{{").expect("写损坏报告");
    assert!(!segment_reusable(&path, None), "损坏（非 JSON）→ 失效重跑");

    // 空文件同样损坏面
    fs::write(&path, "").expect("写空报告");
    assert!(!segment_reusable(&path, None), "空文件 → 失效");
}

/// mtime 判据边界：报告**旧于**输入 → 失效重跑（断点续跑只重跑失效段）；
/// 段内文件 / suite 测试文件任一 mtime 更新 → 失效（以 newest 输入参数注入
/// 承载）。
#[test]
fn 复用门mtime判据_报告旧于输入则失效() {
    let (_dir, mutation) = mutation_dir();
    let path = write_segment_report(&mutation, 0, SEG_RAW).expect("段报告写盘应成功");

    // 报告 mtime = 5_000；输入最新 = 10_000（报告旧于输入）→ 失效
    set_mtime(&path, 5_000);
    assert!(
        !segment_reusable(&path, Some(10_000)),
        "报告旧于输入 → 失效重跑（输入更新段失效）"
    );
}

/// 清理边界（AC-5）：`cleanup_stryker_artifacts` 只删 `_stryker-tmp/` 与
/// `mutation-stryker-seg-*.conf.json`；段级 / 归并报告等报告工件 MUST NOT 被
/// 清理；不存在目录 → best-effort 静默（不 panic）。
#[test]
fn stryker清理边界_报告工件不触碰() {
    let (_dir, mutation) = mutation_dir();
    fs::create_dir_all(&mutation).expect("建目录");

    // sandbox 目录 + 内部文件
    let sandbox = mutation.join(STRYKER_TMP_DIR_NAME);
    fs::create_dir_all(sandbox.join("nested")).expect("建 sandbox");
    fs::write(sandbox.join("nested").join("work.js"), "x").expect("写 sandbox 文件");

    // 两份临时 config
    fs::write(mutation.join(stryker_config_name(0)), "{}").expect("写 config 0");
    fs::write(mutation.join(stryker_config_name(3)), "{}").expect("写 config 3");

    // 报告工件（段级 × 2 + 归并块 + 段补丁）
    write_segment_report(&mutation, 0, SEG_RAW).expect("段报告 0");
    write_segment_report(&mutation, 1, SEG_RAW).expect("段报告 1");
    write_segment_patch(&mutation, 0, "patch").expect("段补丁 0");
    let block = MutationBlock {
        pass: true,
        score: 100.0,
        threshold: 80.0,
        measured: MutationMeasured::default(),
        overrides: None,
    };
    write_merged_block(&mutation, &block).expect("归并块");

    cleanup_stryker_artifacts(&mutation);

    // sandbox 与两份 config 已清
    assert!(!sandbox.exists(), "_stryker-tmp 已清理");
    assert!(
        !mutation.join(stryker_config_name(0)).exists(),
        "config 0 已清理"
    );
    assert!(
        !mutation.join(stryker_config_name(3)).exists(),
        "config 3 已清理"
    );

    // 报告工件不触碰
    assert!(segment_report_path(&mutation, 0).is_file(), "段报告 0 留存");
    assert!(segment_report_path(&mutation, 1).is_file(), "段报告 1 留存");
    assert!(segment_patch_path(&mutation, 0).is_file(), "段补丁留存");
    assert!(merged_block_path(&mutation).is_file(), "归并块留存");

    // 二次清理幂等（best-effort——目录已在首轮删除）
    cleanup_stryker_artifacts(&mutation);
    assert!(
        segment_report_path(&mutation, 0).is_file(),
        "二次清理后报告仍留存"
    );

    // 不存在目录 → 静默不 panic
    cleanup_stryker_artifacts(&mutation.join("no-such-dir"));
}

/// 命名与路径面：段报告 / 段补丁 / 归并块 / 临时 config 名逐字；`to_posix`
/// 反斜杠归一（Windows 面报告路径清单反斜杠不出线）。
#[test]
fn 命名与路径面_逐字与posix归一() {
    assert_eq!(segment_report_name(7), "mutation-seg-7.json");
    assert_eq!(segment_patch_name(7), "mutation-seg-7.patch");
    assert_eq!(stryker_config_name(7), "mutation-stryker-seg-7.conf.json");
    assert_eq!(STRYKER_TMP_DIR_NAME, "_stryker-tmp");

    let dir = Path::new("C:\\ws\\reports\\app_rust\\mutation");
    assert_eq!(
        to_posix(&segment_report_path(dir, 0)),
        "C:/ws/reports/app_rust/mutation/mutation-seg-0.json",
        "反斜杠归一 POSIX（报告路径清单面）"
    );

    // mtime 读取：不存在文件 → None（不 panic）
    assert_eq!(
        mtime_ms(Path::new("no-such-file.json")),
        None,
        "缺文件 mtime None"
    );
}
