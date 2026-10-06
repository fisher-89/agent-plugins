//! 磁盘布局解析：workspace 根 → change 域三棵目录树 + 配置文件路径。
//!
//! 常量组（域目录名 + changes / archive / explores 子目录名 + 配置文件名，
//! 文件私有、仅供本文件路径组装引用）与 `resolve` / `config_path` 两个纯推导
//! 函数是整个 desktop 包内唯一知晓当前磁盘目录名、子目录名与配置文件名的
//! 位置；目录或文件名未来改名只需修改常量组，model / parse / queries /
//! config / desktop-app 与前端零改动（layout_test 命名隔离扫描双禁令不变量
//! 执法）。纯路径推导、无任何文件系统访问，对不存在的 root 亦正常返回
//! （存在性检查交由上层按空结果处理）。

use std::path::{Path, PathBuf};

/// 域根目录名。
const DOMAIN_DIR_NAME: &str = "openspec";

/// 进行中 change 子目录名（相对域根锚定）
const CHANGES_DIR_NAME: &str = "changes";

/// 已归档 change 子目录名（相对 changes 子目录锚定，物理嵌套于 changes 之内）
const ARCHIVE_DIR_NAME: &str = "archive";

/// 探索笔记子目录名（相对域根锚定）
const EXPLORES_DIR_NAME: &str = "explores";

/// 工作区配置文件名（相对域根锚定）：`config` crate 经 [`config_path`] 取路径，
/// 全包其余产品源码禁写此字面量（layout_test 隔离扫描唯一例外即本文件）。
const CONFIG_FILE_NAME: &str = "config.json";

/// change 报告子目录名（相对 change 目录锚定）
const REPORTS_DIR_NAME: &str = "reports";

/// 测试报告子目录名（相对 reports 子目录锚定）
const TEST_REPORTS_DIR_NAME: &str = "test";

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
/// 裸名语义（任意层级同名目录整棵剪枝）的调用方由此取字面量。本模块保持
/// 全包唯一磁盘字面量触点（layout_test 命名隔离扫描不变量）。
pub fn domain_dir_name() -> &'static str {
    DOMAIN_DIR_NAME
}

/// 把用户选定的 workspace 根目录解析为布局结构。
pub fn resolve(root: &Path) -> Layout {
    let domain_root = root.join(DOMAIN_DIR_NAME);
    Layout {
        changes_root: domain_root.join(CHANGES_DIR_NAME),
        archive_root: domain_root.join(CHANGES_DIR_NAME).join(ARCHIVE_DIR_NAME),
        explores_root: domain_root.join(EXPLORES_DIR_NAME),
    }
}

/// 把 workspace 根目录解析为配置文件路径：`<root>/<域目录名>/<配置文件名>`。
/// 纯拼接、无文件系统访问；`config` crate 取配置文件路径的唯一通道，
/// 与 `resolve` 的域目录推导同源引用常量组。
pub fn config_path(root: &Path) -> PathBuf {
    root.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)
}

/// 把 workspace 根目录与 change 名解析为 change 测试报告目录：
/// `<root>/<域目录名>/<changes>/<change>/<报告目录名>/<测试报告目录名>`。
/// 纯拼接、无文件系统访问；checks 域报告写盘与复用门取报告目录的唯一
/// 通道，与 `resolve` / `config_path` 同源引用常量组。
pub fn change_test_reports(root: &Path, change: &str) -> PathBuf {
    resolve(root)
        .changes_root
        .join(change)
        .join(REPORTS_DIR_NAME)
        .join(TEST_REPORTS_DIR_NAME)
}

#[cfg(test)]
mod mod_test;
