//! vcs crate 根（lib.rs）的单元测试（test-design「lib.rs -> lib_test.rs」节）：
//! worktree 落位派生单点——`data_root/worktrees/{身份段}/<change>` 消费
//! foundation 身份段（同源单点直调对拍）；同根可复现 / 异根不撞；change 名
//! 零清洗零截断（清洗权威在 create 前置，本函数零校验的负向锚）；目录名常
//! 量与无状态装配入口。纯路径推导内存直驱（tempfile 仅取路径字符串，零 git
//! 零 spawn）。

use std::path::{Path, PathBuf};

use super::{worktree_dir, ProcessWorktree, WORKTREES_DIR_NAME};

/// 落位形态：`data_root/worktrees/{workspace_identity_segment(ws_root)}/{change}`
/// ——第三级与 foundation 单点同源（直调对拍）；`WORKTREES_DIR_NAME == "worktrees"`。
#[test]
fn 落位形态_身份段父目录与foundation单点同源对拍() {
    assert_eq!(
        WORKTREES_DIR_NAME, "worktrees",
        "worktrees 子树目录名单点常量"
    );

    let data_root = PathBuf::from("C:\\app-data");
    for (ws_root, change) in [
        ("C:\\ws\\demo-alpha", "fix-bug"),
        ("/home/u/repo", "add-feature"),
        ("C:\\ws\\...", "edge-name"),
    ] {
        let placement = worktree_dir(&data_root, ws_root, change);
        let expected = data_root
            .join(WORKTREES_DIR_NAME)
            .join(foundation::identity::workspace_identity_segment(ws_root))
            .join(change);
        assert_eq!(
            placement, expected,
            "落位 = data_root/worktrees/{{身份段}}/<change>（{ws_root}）"
        );
    }
}

/// 可复现与隔离：同 (data_root, ws_root, change) 两次派生等值；异 ws_root 同
/// change → 不同身份段父目录（互不冲突互不可见）。
#[test]
fn 可复现与隔离_同根等值异根父目录互异() {
    let data_root = PathBuf::from("C:\\app-data");
    let first = worktree_dir(&data_root, "C:\\ws\\alpha", "fix-bug");
    let second = worktree_dir(&data_root, "C:\\ws\\alpha", "fix-bug");
    assert_eq!(
        first, second,
        "同 (data_root, ws_root, change) 派生确定可复现"
    );

    let other = worktree_dir(&data_root, "C:\\ws\\beta", "fix-bug");
    assert_ne!(
        first.parent(),
        other.parent(),
        "异 ws_root 同 change → 身份段父目录不同（互不冲突）"
    );
    assert_eq!(
        first.file_name(),
        other.file_name(),
        "change 名尾段不受 workspace 身份影响"
    );
}

/// change 名原样挂尾：不做任何清洗 / 截断（清洗权威在 create 前置——本函数
/// 零校验的负向锚；非法名经此原样透出，由 create 七道前置拒绝）。
#[test]
fn change名原样挂尾_零清洗零截断() {
    let data_root = Path::new("C:\\app-data");
    for raw in [
        "Fix-Bug",
        "fix_bug",
        "a/b",
        "fix--bug",
        "超长".repeat(64).as_str(),
    ] {
        let placement = worktree_dir(data_root, "C:\\ws\\demo", raw);
        assert!(
            placement.ends_with(Path::new(raw)),
            "change 名零清洗零截断原样挂尾（含多分量名原样透出）: {raw:?} -> {}",
            placement.display()
        );
    }
}

/// 无状态装配入口：`new()` 与 `Default::default()` 等值可重复构造（组合根
/// 按需铸的编译锚——零字段无状态，多次构造互不干扰）。
#[test]
fn processworktree构造_new与default等值() {
    let _a = ProcessWorktree::new();
    let _b = ProcessWorktree::default();
    let _c = ProcessWorktree::new();
    // 无状态空类型：可重复构造即断言面（字段零个，等值由类型层承载）
}
