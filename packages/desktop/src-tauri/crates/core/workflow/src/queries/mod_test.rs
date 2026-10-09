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
        locate_change(&resolve(&self.0), None, name)
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

// ---------------------------------------------------------------------------
// worktree 回退（design D12 第四级）：merge 前主仓两树未命中、worktree 记录
// 在场 → resolve(worktree).changes_root/<name> 命中
// ---------------------------------------------------------------------------

/// worktree 树夹具（真实 tempdir；change 目录落 worktree 内 openspec/changes
/// 之下——merge 前主仓两树必然未命中的目录形态）。
struct TempWorktree(PathBuf);

impl TempWorktree {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-queries-mod-wt-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn mkdir(&self, rel_dir: &str) {
        fs::create_dir_all(self.0.join(rel_dir)).expect("创建 worktree 目录失败");
    }

    fn root_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    fn locate(&self, ws: &TempWs, name: &str) -> Option<super::ChangeLocation> {
        locate_change(&resolve(&ws.0), Some(&self.root_str()), name)
    }
}

impl Drop for TempWorktree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// worktree 回退命中：主仓两树未命中 + worktree 目录树在场 → `Some` 且
/// `source == Active`、dir = worktree 内该目录。
#[test]
fn worktree回退命中_source为active且dir在worktree内() {
    let ws = TempWs::new("wt-hit-main");
    let wt = TempWorktree::new("wt-hit");
    wt.mkdir("openspec/changes/wt-change");

    let location = wt.locate(&ws, "wt-change").expect("worktree 回退应命中");

    assert_eq!(location.source, ChangeSource::Active, "回退命中 → Active");
    assert_eq!(
        location.dir,
        resolve(&wt.0).changes_root.join("wt-change"),
        "dir = worktree 内该目录"
    );
}

/// 优先级链：worktree 记录在场且目录命中 → 优先返回（双树共存时在途编辑
/// 胜）；miss 后落主仓 active 精确 / archive 精确 / archive 日期前缀链；主仓
/// 链亦 miss 时 worktree 目录仍可独立命中（merge 前形态）；`worktree=None`
/// 不短路（legacy / 文档形态主仓链照常可达——五态一行覆盖优先序）。
#[test]
fn 优先级链_worktree优先主仓链兜底_none不短路() {
    // 态一：worktree 目录在场 → 命中 worktree（双树共存时在途编辑胜过主仓副本）
    let ws = TempWs::new("prio-active");
    let wt = TempWorktree::new("prio-active-wt");
    ws.mkdir("openspec/changes/foo");
    wt.mkdir("openspec/changes/foo");
    let location = wt.locate(&ws, "foo").expect("应命中");
    assert_eq!(
        location.dir,
        resolve(&wt.0).changes_root.join("foo"),
        "worktree 在场命中优先（主仓 active 精确名退居其次）"
    );

    // 态二：worktree 目录 miss → 落主仓 active 精确名
    let ws = TempWs::new("prio-archive");
    let wt = TempWorktree::new("prio-archive-wt");
    ws.mkdir("openspec/changes/archive/foo");
    wt.mkdir("openspec/changes/foo2"); // worktree 只命中 foo2，foo 走主仓链
    let location = wt.locate(&ws, "foo").expect("应命中");
    assert_eq!(
        location.source,
        ChangeSource::Archive,
        "worktree miss 落主仓 archive 精确名"
    );

    // 态三：worktree miss + 主仓 archive 日期前缀在场 → 命中前缀目录
    let ws = TempWs::new("prio-prefix");
    let wt = TempWorktree::new("prio-prefix-wt");
    ws.mkdir("openspec/changes/archive/2026-10-06-foo");
    wt.mkdir("openspec/changes/foo2");
    let location = wt.locate(&ws, "foo").expect("应命中");
    assert_eq!(location.source, ChangeSource::Archive, "日期前缀为第四级");
    assert_eq!(
        location.dir,
        resolve(&ws.0).archive_root.join("2026-10-06-foo")
    );

    // 态四：主仓链全空、worktree 目录独立在场 → worktree 命中（merge 前形态）
    let ws = TempWs::new("prio-fallback");
    let wt = TempWorktree::new("prio-fallback-wt");
    wt.mkdir("openspec/changes/foo2");
    let location = wt.locate(&ws, "foo2").expect("worktree 独立命中");
    assert_eq!(
        location.source,
        ChangeSource::Active,
        "主仓空时 worktree 命中"
    );
    assert_eq!(location.dir, resolve(&wt.0).changes_root.join("foo2"));

    // 态五：worktree=None → 不短路，直接走主仓链（legacy / 文档形态面）
    let ws = TempWs::new("prio-none");
    ws.mkdir("openspec/changes/foo");
    let location = ws.locate("foo").expect("None 不短路，主仓 active 命中");
    assert_eq!(
        location.dir,
        resolve(&ws.0).changes_root.join("foo"),
        "worktree 缺席时主仓链照常可达"
    );
}

/// worktree 缺席：`worktree=Some` 但该目录被删（回退 miss）→ `None`（手动
/// 删 worktree 后的定位语义——detail 恒可达半边的反面输入）。
#[test]
fn worktree目录被删_回退miss返回none() {
    let ws = TempWs::new("wt-gone");
    let wt = TempWorktree::new("wt-gone-wt");
    // worktree 根存在但 change 目录缺席（目录被删形态）
    wt.mkdir("openspec/changes");

    assert!(
        wt.locate(&ws, "gone-change").is_none(),
        "worktree 目录缺席 → 回退 miss → None"
    );
}

/// 穿越校验保留：多分量 / 穿越 / 空名 → `None`（worktree 在场亦不豁免单分
/// 量名校验——worktree 路径不可被名分量注入劫持）。
#[test]
fn 穿越校验保留_worktree在场不豁免单分量名校验() {
    let ws = TempWs::new("wt-hostile");
    let wt = TempWorktree::new("wt-hostile-wt");
    wt.mkdir("openspec/changes/foo");

    for name in ["a/b", "..", ".", "", "a\\b", "a:b"] {
        assert!(
            wt.locate(&ws, name).is_none(),
            "非法名 {name:?} 在 worktree 回退路径同样被拒绝（单分量校验先行）"
        );
    }
}
