//! `shutdown` 的单元测试：退出关库后全局库与已缓存 workspace 库均拒绝后续
//! 操作（`StoreError::Closed`），`for_root` 不再开新库。

use store::{StoreError, WorkspaceStores};
use tauri::Manager;

/// 关库钩子覆盖两级库：全局 + 已缓存 workspace；关库后 for_root 关闭闸。
#[test]
fn shutdown_stores关闭全局与已缓存workspace库() {
    let data_dir = tempfile::Builder::new()
        .prefix("shutdown-test-data-")
        .tempdir()
        .expect("创建数据根临时目录失败");
    let ws_root = tempfile::Builder::new()
        .prefix("shutdown-test-root-")
        .tempdir()
        .expect("创建 workspace 根临时目录失败");
    let root = ws_root.path().to_string_lossy().into_owned();

    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(data_dir.path()).expect("打开测试全局库失败");
    let workspace = stores.for_root(&root).expect("预开 workspace 库失败");
    app.manage(stores);

    super::shutdown_stores(app.handle());

    let stores = app.state::<WorkspaceStores>();
    assert!(
        matches!(stores.global().list_workspaces(), Err(StoreError::Closed)),
        "全局库应已关闭"
    );
    assert!(
        matches!(workspace.list_change_records(), Err(StoreError::Closed)),
        "已缓存 workspace 库应已关闭（外部 Arc 持有不阻止关库）"
    );
    assert!(
        matches!(stores.for_root(&root), Err(StoreError::Closed)),
        "关库后 for_root 不应再开新库"
    );
}
