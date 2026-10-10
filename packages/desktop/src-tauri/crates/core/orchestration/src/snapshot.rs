use std::path::Path;
use std::sync::Arc;

use foundation::layout;
use workflow::queries::{change_detail, ChangeDetail};
use workflow::state::ChangeStateStore;

use crate::port::WorkflowSnapshotPort;

/// db 快照源：绑定 workspace 根与 store 缝（组合根装配）。
pub struct StoreSnapshot {
    /// 绑定的 workspace 根
    pub root: String,
    /// change 状态读源（`for_root` 实例，组合根注入）
    store: Arc<dyn ChangeStateStore>,
}

impl StoreSnapshot {
    /// 绑定 workspace 根与 store 缝构造（组合根装配；`detail` 的 root 参数与
    /// 其一致，参数面保留 port 契约的中性性——测试假件不依赖绑定根）。
    pub fn new(root: String, store: Arc<dyn ChangeStateStore>) -> Self {
        Self { root, store }
    }
}

impl WorkflowSnapshotPort for StoreSnapshot {
    fn detail(&self, root: &str, id: &str) -> Result<ChangeDetail, String> {
        let layout_ = layout::resolve(Path::new(root));
        change_detail(&layout_, self.store.as_ref(), id)
            .ok_or_else(|| format!("change 不存在: {id}"))
    }
}

impl std::fmt::Debug for StoreSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreSnapshot")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Clone for StoreSnapshot {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            store: Arc::clone(&self.store),
        }
    }
}
