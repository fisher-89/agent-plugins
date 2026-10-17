//! 依赖与工具产物树名单共享单点：目录名封闭集 + dot 剪枝判定纯函数。
//!
//! 语义为「依赖与工具产物树，探测与清理均应跳过」——两消费点同源消费：
//! testexec detect 剪枝（探测不入清单）与 vcs `restore_baseline` 排除面
//!（clean 不删依赖）。名单扩展一处修改双点生效；`openspec/` 工件树增补属
//! vcs 消费点语义，不入名单本体。

/// 依赖与工具产物树目录名封闭集（CLI `collectFiles` excludedDirs 非 dot
/// 成员同集；dot 成员 `.nyc_output` 在案——dot 剪枝判定之外显式收录）。
pub const PRUNED_DIR_NAMES: [&str; 6] = [
    "node_modules",
    "dist",
    "build",
    "target",
    "coverage",
    ".nyc_output",
];

/// 目录名剪枝判定（dot 前缀或名单命中；路径组件迭代语义驻各消费点）。
pub fn is_pruned_dir_name(name: &str) -> bool {
    name.starts_with('.') || PRUNED_DIR_NAMES.contains(&name)
}
