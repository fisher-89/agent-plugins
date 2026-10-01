//! [`WorkflowSnapshotPort`] 的 fs 实现：foundation Layout 解析 +
//! `workflow::change_detail` 只读装配（决策输入与前置校验的输入面）。纯读
//! 缝（W6 保留缩水）——读、写是两个关注点，workflow.json 写触点唯一经
//! `workflow::write` 写面进程内直调。对编排面严格：workflow.json 不可解析
//! → 显式 `Err`（坏文档不进决策输入，编排停给用户而非带病续走）。

use std::path::Path;

use foundation::layout;
use workflow::queries::{change_detail, ChangeDetail};

use crate::port::WorkflowSnapshotPort;

/// fs 快照源：绑定 workspace 根（组合根装配）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsSnapshot {
    /// 绑定的 workspace 根
    pub root: String,
}

impl FsSnapshot {
    /// 绑定 workspace 根构造（组合根装配；`detail` 的 root 参数与其一致，
    /// 参数面保留 port 契约的中性性——测试假件不依赖绑定根）。
    pub fn new(root: String) -> Self {
        Self { root }
    }
}

impl WorkflowSnapshotPort for FsSnapshot {
    fn detail(&self, root: &str, change: &str) -> Result<ChangeDetail, String> {
        let layout = layout::resolve(Path::new(root));
        let detail =
            change_detail(&layout, change).ok_or_else(|| format!("change 不存在: {change}"))?;
        // 不可解析显式 `Err`（编排内部 port 实现严格语义；读面
        // `change_detail` 的 `unparsable` 旗标建模保留，归 UI 展示面消费）
        if detail.unparsable {
            return Err(format!("change \"{change}\" 的 workflow.json 无法解析"));
        }
        Ok(detail)
    }
}
