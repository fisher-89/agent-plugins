//! 三类差异的比对单点：应生成集 vs 盘上实况。
//!
//! **纯读盘、零写盘**。仓库级遍历一律经 [`crate::scan::walk_source_files`]（gitignore
//! 感知 + 剪构建产物树；MUST NOT 自建递归、MUST NOT 复制第二份遍历），契约目录为
//! **有界本地枚举**（与 [`super::enabled`] 的 `has_markdown` 同形，不构成仓库遍历）。
//! 路径串一律经 [`super::display`]（跨 crate 唯一归一单点）。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::render::workspace_render;
use super::{display, ApiError};

/// 三类差异（进程内类型；出线由 [`super::artifact_check`] 投影为 `ArchiApiCheck`）。
///
/// 三列均为 repo 相对 POSIX 串；`mismatched` / `missing` 按 [`workspace_render`] 的
/// 输出序（契约 FQN 序 → 各元素 `.i.*` → `INDEX.toml` 末位），`orphaned` 按路径
/// 字节序——同一稳定序是「CLI / IPC 两驱动面逐条相等」的可验形态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDiff {
    /// 内容不符（文件在盘但字节与渲染结果不同）
    pub mismatched: Vec<String>,
    /// 应删未删（盘上存在、命名符合生成物判据、不在应生成集）
    pub orphaned: Vec<String>,
    /// 应生成未生成
    pub missing: Vec<String>,
}

/// 比对（零写入）：`mismatched` / `missing` 由应生成集逐份读盘分流，
/// `orphaned` 由盘上生成物集与应生成集的差集得出。
pub fn compare(root: &Path) -> Result<ArtifactDiff, ApiError> {
    let expected = workspace_render(root)?;
    let mut mismatched: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut expected_paths: BTreeSet<PathBuf> = BTreeSet::new();
    for file in &expected {
        expected_paths.insert(file.path.clone());
        match fs::read_to_string(&file.path) {
            Ok(actual) if actual == file.content => {}
            Ok(_) => mismatched.push(display(root, &file.path)),
            Err(_) => missing.push(display(root, &file.path)),
        }
    }
    let orphaned: Vec<String> = generated_files_on_disk(root)
        .into_iter()
        .filter(|path| !expected_paths.contains(path))
        .map(|path| display(root, &path))
        .collect();
    Ok(ArtifactDiff {
        mismatched,
        orphaned,
        missing,
    })
}

/// 生成物命名判据（`*.i.rs` / `*.i.ts`）：**命名即身份**，与落点深度无关。
fn is_generated_module_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".i.rs") || name.ends_with(".i.ts"))
}

/// 盘上现有的四类生成物（`sorted` + `dedup` 前的原始集合）。
///
/// 元素根同级的 `.i.*` 可在仓库任意深度（`crates/infra/archi` →
/// `crates/infra/archi.i.rs`、`views/welcome-view.tsx` → `views/welcome-view.tsx.i.ts`），
/// 故枚举经 [`crate::scan::walk_source_files`]（与文件树同源，gitignore 感知）；
/// 契约目录内的 `.schema.json` / `INDEX.toml` 经 [`api_dir_artifacts`]。
fn generated_files_on_disk(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = crate::scan::walk_source_files(root)
        .into_iter()
        .filter(|path| is_generated_module_file(path))
        .collect();
    found.extend(api_dir_artifacts(root));
    found.sort();
    found.dedup();
    found
}

/// 契约目录内的生成物（`*.schema.json` 与 `INDEX.toml`）：有界本地枚举（深度上限
/// 复用 [`super::MAX_API_WALK_DEPTH`]、符号链接不跟随），与 [`super::enabled`] 的
/// `has_markdown` 同形。
fn api_dir_artifacts(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let index = super::paths::index_path(root);
    collect_api_dir(&super::paths::directory(root), &index, 0, &mut found);
    found
}

/// 契约目录递归（深度上限防环；符号链接不跟随）。
fn collect_api_dir(directory: &Path, index: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > super::MAX_API_WALK_DEPTH {
        return;
    }
    let Ok(reader) = fs::read_dir(directory) else {
        return;
    };
    for entry in reader.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_api_dir(&path, index, depth + 1, found);
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let schema = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(super::SCHEMA_FILE_SUFFIX));
        if schema || path == index {
            found.push(path);
        }
    }
}
