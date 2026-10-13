use std::path::{Path, PathBuf};

use checks::model::MutationBlock;

/// Stryker 共享 sandbox 目录名（全段共用同一 `tempDirPath`；全段后一次清理，
/// 段级 / 归并报告工件 MUST NOT 触碰）。
pub(crate) const STRYKER_TMP_DIR_NAME: &str = "_stryker-tmp";

/// 段级原始报告文件名（`<i>` 为段序号；工具原始输出全程留存）。
pub(crate) fn segment_report_name(index: usize) -> String {
    format!("mutation-seg-{index}.json")
}

/// 段级 `--in-diff` 补丁文件名（cargo-mutants 面；Stryker 面不产）。
pub(crate) fn segment_patch_name(index: usize) -> String {
    format!("mutation-seg-{index}.patch")
}

/// Stryker 临时 config 文件名（全段后一次清理面）。
pub(crate) fn stryker_config_name(index: usize) -> String {
    format!("mutation-stryker-seg-{index}.conf.json")
}

/// 段级原始报告路径（`<plan>/mutation/mutation-seg-<i>.json`）。
pub(crate) fn segment_report_path(mutation_dir: &Path, index: usize) -> PathBuf {
    mutation_dir.join(segment_report_name(index))
}

/// 段级补丁路径（`<plan>/mutation/mutation-seg-<i>.patch`）。
pub(crate) fn segment_patch_path(mutation_dir: &Path, index: usize) -> PathBuf {
    mutation_dir.join(segment_patch_name(index))
}

/// 归并块路径（`<plan>/mutation/mutation.json`，schema 零扩展）。
pub(crate) fn merged_block_path(mutation_dir: &Path) -> PathBuf {
    mutation_dir.join("mutation.json")
}

/// 段原始报告写盘（工具原始输出原样——不重排不丢字段；`<i>` 序号连续）。
pub(crate) fn write_segment_report(
    mutation_dir: &Path,
    index: usize,
    raw: &str,
) -> Result<PathBuf, String> {
    create_mutation_dir(mutation_dir)?;
    let path = segment_report_path(mutation_dir, index);
    std::fs::write(&path, raw)
        .map_err(|error| format!("段报告写盘失败 {}: {error}", to_posix(&path)))?;
    Ok(path)
}

/// 段级补丁写盘（`--in-diff` 段切片落位）。
pub(crate) fn write_segment_patch(
    mutation_dir: &Path,
    index: usize,
    patch: &str,
) -> Result<PathBuf, String> {
    create_mutation_dir(mutation_dir)?;
    let path = segment_patch_path(mutation_dir, index);
    std::fs::write(&path, patch)
        .map_err(|error| format!("段补丁写盘失败 {}: {error}", to_posix(&path)))?;
    Ok(path)
}

/// 归并块写盘（`MutationBlock` schema 零扩展；段级报告不被覆盖——各归各文件）。
pub(crate) fn write_merged_block(
    mutation_dir: &Path,
    block: &MutationBlock,
) -> Result<PathBuf, String> {
    create_mutation_dir(mutation_dir)?;
    let json = serde_json::to_string_pretty(block)
        .map_err(|error| format!("归并块序列化失败: {error}"))?;
    let path = merged_block_path(mutation_dir);
    std::fs::write(&path, &json)
        .map_err(|error| format!("归并块写盘失败 {}: {error}", to_posix(&path)))?;
    Ok(path)
}

/// 段级复用门（E6；判据借段报告自身 mtime，零旁置状态文件）：段报告在位 &&
/// 可解析（非 JSON 视为损坏失效）&& 段报告 mtime ≥ 段输入最新 mtime（**相等即
/// 复用**）→ 复用（零 spawn，段报告重解析进归并）；否则失效重跑（断点续跑只
/// 重跑未完 / 失效段，不 panic 不 Err）。
pub(crate) fn segment_reusable(report_path: &Path, newest_input_ms: Option<u64>) -> bool {
    let Ok(raw) = std::fs::read_to_string(report_path) else {
        return false;
    };
    if serde_json::from_str::<serde_json::Value>(&raw).is_err() {
        return false;
    }
    let Some(report_mtime) = mtime_ms(report_path) else {
        return false;
    };
    match newest_input_ms {
        Some(newest) => report_mtime >= newest,
        None => true,
    }
}

/// 文件 mtime（UTC unix 毫秒；不可读 / 无 mtime → `None`）。
pub(crate) fn mtime_ms(path: &Path) -> Option<u64> {
    let millis = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    Some(millis)
}

/// Stryker sandbox / 临时 config 全段后一次清理：只清 `_stryker-tmp/` 与
/// `mutation-stryker-seg-*.conf.json`；段级 / 归并报告等报告工件 MUST NOT 被
/// 清理（best-effort——单项清理失败不阻断收口）。
pub(crate) fn cleanup_stryker_artifacts(mutation_dir: &Path) {
    let _ = std::fs::remove_dir_all(mutation_dir.join(STRYKER_TMP_DIR_NAME));
    let Ok(entries) = std::fs::read_dir(mutation_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("mutation-stryker-seg-") && name.ends_with(".conf.json") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// 突变报告目录创建（写盘失败显式 Err——E5）。
fn create_mutation_dir(mutation_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(mutation_dir)
        .map_err(|error| format!("突变报告目录创建失败 {}: {error}", to_posix(mutation_dir)))
}

/// 路径 POSIX 化（报告路径清单面——反斜杠不出线）。
pub(crate) fn to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
#[path = "report_test.rs"]
mod report_test;
