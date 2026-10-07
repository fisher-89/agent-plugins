use std::path::{Path, PathBuf};

/// 域根目录名。
const DOMAIN_DIR_NAME: &str = "openspec";

/// 进行中 change 子目录名（相对域根锚定）
const CHANGES_DIR_NAME: &str = "changes";

/// 已归档 change 子目录名（相对 changes 子目录锚定，物理嵌套于 changes 之内）
const ARCHIVE_DIR_NAME: &str = "archive";

/// 探索笔记子目录名（相对域根锚定）
const EXPLORES_DIR_NAME: &str = "explores";

/// 工作区配置文件名（相对域根锚定）
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

/// 域根目录名（裸名消费方唯一来源）
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

/// 把 workspace 根目录解析为配置文件路径
pub fn config_path(root: &Path) -> PathBuf {
    root.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)
}

/// 把 workspace 根目录与 change 名解析为 change 测试报告目录
pub fn change_test_reports(root: &Path, change: &str) -> PathBuf {
    resolve(root)
        .changes_root
        .join(change)
        .join(REPORTS_DIR_NAME)
        .join(TEST_REPORTS_DIR_NAME)
}

#[cfg(test)]
mod mod_test;
