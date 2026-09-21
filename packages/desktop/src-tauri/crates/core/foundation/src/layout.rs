//! 磁盘布局解析：workspace 根 → change 域三棵目录树。
//!
//! `resolve` 是整个 desktop 包内唯一知晓当前磁盘目录名的位置；
//! 目录未来改名只需修改此函数，model / parse / queries / desktop-app 与前端零改动。
//! 纯路径推导、无任何文件系统访问，对不存在的 root 亦正常返回
//! （存在性检查交由上层查询按空结果处理）。

use std::path::{Path, PathBuf};

/// workspace 磁盘布局：进行中 / 已归档 / 探索笔记三棵目录树。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// 进行中 change 目录树
    pub changes_root: PathBuf,
    /// 已归档 change 目录树
    pub archive_root: PathBuf,
    /// 探索笔记目录树
    pub explores_root: PathBuf,
}

/// 把用户选定的 workspace 根目录解析为布局结构。
pub fn resolve(root: &Path) -> Layout {
    let domain_root = root.join("openspec");
    Layout {
        changes_root: domain_root.join("changes"),
        archive_root: domain_root.join("changes").join("archive"),
        explores_root: domain_root.join("explores"),
    }
}
