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

/// 域根目录名（裸名消费方唯一来源）：tokei `ignored_directories` 等 gitignore
/// 裸名语义（任意层级同名目录整棵剪枝）的调用方由此取字面量，本模块保持
/// 全包唯一磁盘目录名触点（layout_test 命名隔离扫描不变量）。
pub fn domain_dir_name() -> &'static str {
    "openspec"
}

/// 把用户选定的 workspace 根目录解析为布局结构。
pub fn resolve(root: &Path) -> Layout {
    let domain_root = root.join(domain_dir_name());
    Layout {
        changes_root: domain_root.join("changes"),
        archive_root: domain_root.join("changes").join("archive"),
        explores_root: domain_root.join("explores"),
    }
}
