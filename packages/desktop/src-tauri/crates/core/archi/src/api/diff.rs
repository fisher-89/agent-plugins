//! 三类差异的比对单点：应生成集 vs 盘上实况。
//!
//! **纯读盘、零写盘**。仓库级遍历一律经 [`crate::scan::walk_source_files`]（gitignore
//! 感知 + 剪构建产物树；MUST NOT 自建递归、MUST NOT 复制第二份遍历），索引目录为**一处
//! 有界本地枚举**（深度上限防环、符号链接不跟随，不构成仓库遍历）。
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

/// 盘上现有的**三类**生成物（`sorted` + `dedup` 前的原始集合）。
///
/// 生成物落**实现入口同级**，故可在仓库任意深度（`crates/infra/archi/src/lib.rs` →
/// `crates/infra/archi/src/lib.i.rs`、`views/welcome-view.tsx` → `views/welcome-view.i.ts`），
/// 枚举因而经 [`crate::scan::walk_source_files`]（与文件树同源，gitignore 感知）；索引
/// （`<archi_graph_dir>/INDEX.toml`）经 [`api_dir_artifacts`] 派生。
///
/// **`<FQN>.schema.json` 不在枚举面**：它是手写真相（第二份资产），进孤儿扫描就会让每份
/// 手写接口文件全进 `orphaned`——症状是「全红」而不是「少一条」。
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

/// 索引侧的生成物（`<archi_graph_dir>/**` 的 `INDEX.toml`）：**只剩一个根**。
///
/// 资产目录（`<archi_models_dir>/**`）**退出枚举**——那里的 `*.schema.json` 与 `*.md` 都是
/// 手写资产。口径与「`render_all` 只产三类」同一提交落地（只改一半的症状是全红）。
fn api_dir_artifacts(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    collect_artifacts(
        &foundation::layout::archi_graph_dir(root),
        super::paths::INDEX_FILE_NAME,
        0,
        &mut found,
    );
    found
}

/// 有界递归收面（深度上限防环；符号链接不跟随；只收文件名以 `suffix` 结尾者）。
fn collect_artifacts(directory: &Path, suffix: &str, depth: usize, found: &mut Vec<PathBuf>) {
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
            collect_artifacts(&path, suffix, depth + 1, found);
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let matches = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix));
        if matches {
            found.push(path);
        }
    }
}
