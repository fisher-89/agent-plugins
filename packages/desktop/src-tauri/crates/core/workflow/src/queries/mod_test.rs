//! `queries::locate_change` 的单元测试（test-design「queries/mod.rs ->
//! queries/mod_test.rs」节，新建）：change 目录定位——active 树精确名命中 →
//! archive 树精确名 → archive 树日期前缀后缀匹配（db 名 `foo` ↔
//! `YYYY-MM-DD-foo`，归档条目按月分组可达的路径推导根基——AC-7）；单分量名
//! 校验保留（多分量 / 路径穿越 / 空白名 → None）；双树同名 active 优先；
//! 两树均未命中 → None。
//!
//! Mock策略（test-design 本节 Mock 表）：无 mock——真实 tempdir Layout 目录
//! 树（纯路径推导，零进程边界依赖）。

use std::fs;
use std::path::PathBuf;

use super::locate_change;
use crate::queries::ChangeSource;
use foundation::layout::resolve;

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-queries-mod-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 在 workspace 内创建目录（相对 root 的目录串）。
    fn mkdir(&self, rel_dir: &str) {
        fs::create_dir_all(self.0.join(rel_dir)).expect("创建目录失败");
    }

    fn locate(&self, name: &str) -> Option<super::ChangeLocation> {
        locate_change(&resolve(&self.0), name)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 正向：active 精确名 / archive 精确名 / 日期前缀后缀匹配
// ---------------------------------------------------------------------------

/// active 树同名目录 → 命中 ChangeLocation（source=active 口径不变）。
#[test]
fn active树精确名命中() {
    let ws = TempWs::new("active-exact");
    ws.mkdir("openspec/changes/foo");

    let location = ws.locate("foo").expect("active 树应命中");

    assert_eq!(location.source, ChangeSource::Active);
    assert_eq!(
        location.dir,
        resolve(&ws.0).changes_root.join("foo"),
        "dir = active 树精确路径"
    );
}

/// archive 树同名目录（无前缀）→ 命中 source=archive。
#[test]
fn archive树精确名命中() {
    let ws = TempWs::new("archive-exact");
    ws.mkdir("openspec/changes/archive/foo");

    let location = ws.locate("foo").expect("archive 树应命中");

    assert_eq!(location.source, ChangeSource::Archive);
    assert_eq!(location.dir, resolve(&ws.0).archive_root.join("foo"));
}

/// archive 树 `2026-10-06-foo` → db 名 `foo` 命中（归档后按月分组可达的
/// 推导半边——AC-7）。
#[test]
fn archive日期前缀后缀匹配命中() {
    let ws = TempWs::new("archive-prefixed");
    ws.mkdir("openspec/changes/archive/2026-10-06-foo");

    let location = ws.locate("foo").expect("日期前缀后缀应命中");

    assert_eq!(location.source, ChangeSource::Archive);
    assert_eq!(
        location.dir,
        resolve(&ws.0).archive_root.join("2026-10-06-foo"),
        "dir = 带前缀的实际目录"
    );
}

// ---------------------------------------------------------------------------
// 异常：多分量 / 路径穿越 / 空白名 → None（单分量校验持衡）
// ---------------------------------------------------------------------------

/// 多分量名（含 `/` / `\` / `:`）与 `..` 路径穿越形态、空名 → None。
/// 注记：仅空串受「空白名拒绝」覆盖——含空白字符名（如 "  "）在 Windows
/// Win32 路径归一下会尾随空格剥离而解析到 changes 目录本身（实现现状，
/// 非本变更范围，见变更报告）。
#[test]
fn 多分量与穿越与空名一律none() {
    let ws = TempWs::new("hostile-names");
    ws.mkdir("openspec/changes/foo");
    ws.mkdir("openspec/changes/archive/2026-10-06-foo");

    for name in ["a/b", "..", ".", "", "a\\b", "a:b"] {
        assert!(
            ws.locate(name).is_none(),
            "非法名 {name:?} 必须在定位入口被拒绝（防目录穿越）"
        );
    }
}

/// 非日期形态前缀不参与后缀匹配（`not-a-date-foo` / `foo-2026-10-06` 均不
/// 被识别为 `foo` 的归档目录）。
#[test]
fn 非日期形态前缀不误匹配() {
    let ws = TempWs::new("bad-prefix");
    ws.mkdir("openspec/changes/archive/not-a-date-foo");
    ws.mkdir("openspec/changes/archive/foo-2026-10-06");

    assert!(
        ws.locate("foo").is_none(),
        "仅合法 `YYYY-MM-DD-` 前缀参与后缀匹配"
    );
}

// ---------------------------------------------------------------------------
// 边界：双树同名 active 优先 / 两树均未命中 / 多前缀同后名
// ---------------------------------------------------------------------------

/// 同名同时在 active 与 archive 树 → active 精确命中优先。
#[test]
fn 双树同名_active精确命中优先() {
    let ws = TempWs::new("dual-tree");
    ws.mkdir("openspec/changes/foo");
    ws.mkdir("openspec/changes/archive/2026-01-01-foo");

    let location = ws.locate("foo").expect("应命中");

    assert_eq!(location.source, ChangeSource::Active, "active 精确名优先");
    assert_eq!(location.dir, resolve(&ws.0).changes_root.join("foo"));
}

/// 两树均未命中 → None（不虚构定位）。
#[test]
fn 两树均未命中none() {
    let ws = TempWs::new("missing");
    ws.mkdir("openspec/changes/bar");

    assert!(ws.locate("foo").is_none(), "未知名称返回 None");
}

/// 多日期前缀同后名：命中 archive 树其一（source=archive）。
/// 注记：test-design 本行原判「取最新」与实现不符（`read_dir` 首个命中、
/// 顺序 OS 定，不保证最新）——本用例只钉「命中可达」面（discrepancy 见
/// 变更报告）。
#[test]
fn 多日期前缀同后名命中其一() {
    let ws = TempWs::new("multi-prefix");
    ws.mkdir("openspec/changes/archive/2026-01-01-foo");
    ws.mkdir("openspec/changes/archive/2026-10-06-foo");

    let location = ws.locate("foo").expect("同后名多前缀应命中其一");

    assert_eq!(location.source, ChangeSource::Archive);
    let name = location
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert!(
        name == "2026-01-01-foo" || name == "2026-10-06-foo",
        "dir 为两前缀目录之一，实际: {name}"
    );
}
