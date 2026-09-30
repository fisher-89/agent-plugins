//! 路径沙箱纯函数：入参原始路径 → canonicalize → workspace root（canonical）
//! 前缀校验。
//!
//! bypassPermissions 默认档下 sdk 引擎的唯一护栏：`..\` 相对逃逸与符号链接
//! 绕过统一拦截——canonicalize 把 `..` 段与链接目标一并解析为真实绝对路径，
//! 前缀校验在解析后的形态上做，两类绕过无法各自成立。
//!
//! Windows 口径：std `canonicalize` 两侧（root 与入参）都产出 `\\?\` 扩展
//! 前缀形态，比较前两侧一致化剥除；校验通过的返回路径同样为剥除后的
//! 规范形态（工具执行体直接消费，不再二次解析）。

use std::path::{Path, PathBuf};

/// 剥除 Windows 扩展长度前缀 `\\?\`（其余平台原样）；两侧一致化，避免
/// 「canonical root 带 `\\?\`、入参解析结果不带」的伪前缀失配。
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => path.to_path_buf(),
    }
}

/// 宽容 canonicalize：路径已存在按 std 解析（`..` 与符号链接一并收敛）；
/// 尾段不存在（write 新建文件场景）时自最深存在祖先 canonicalize 后拼回
/// 缺失尾段——缺失段不可能携带链接或 `..` 语义逃逸（拼回前父链已解析），
/// 逃逸只可能经已存在段发生，而已存在段已被解析。
fn canonicalize_lenient(path: &Path) -> Result<PathBuf, String> {
    if let Ok(canonical) = path.canonicalize() {
        return Ok(canonical);
    }
    let mut existing = path.to_path_buf();
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    loop {
        let Some(name) = existing.file_name().map(|name| name.to_os_string()) else {
            return Err("路径无可用祖先段".to_owned());
        };
        let Some(parent) = existing.parent() else {
            return Err("路径无可用祖先目录".to_owned());
        };
        missing.push(name);
        existing = parent.to_path_buf();
        if existing.symlink_metadata().is_ok() {
            break;
        }
    }
    let mut canonical = existing
        .canonicalize()
        .map_err(|e| format!("路径解析失败: {e}"))?;
    for segment in missing.into_iter().rev() {
        canonical.push(segment);
    }
    Ok(canonical)
}

/// 沙箱校验：`raw` 为模型给出的原始路径字符串（相对或绝对）。返回规范化
/// 后的 workspace 内路径，或拒绝原因。root 自身合法（canonical == root）。
///
/// 校验次序：root 先行 canonicalize（root 本身经符号链接落盘是合法形态，
/// 校验基准取解析后形态）→ 入参绝对则原样、相对则拼 root → 宽容
/// canonicalize → 前缀校验。
pub fn check(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let root_canonical = root
        .canonicalize()
        .map_err(|e| format!("workspace root 解析失败: {e}"))?;
    let root_canonical = strip_verbatim(&root_canonical);
    let candidate = {
        let raw_path = Path::new(raw);
        if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            root.join(raw_path)
        }
    };
    let canonical = strip_verbatim(&canonicalize_lenient(&candidate)?);
    if canonical == root_canonical || canonical.starts_with(&root_canonical) {
        Ok(canonical)
    } else {
        Err(format!("路径越出 workspace root: {raw}"))
    }
}

/// glob 模式沙箱预检：模式必须 root 相对（不以盘符 / `/` / `~` 开头）且
/// 不携带 `..` 段——glob 通配段无法 canonicalize，逃逸只在字面段发生，
/// 字面段封死即无逃逸面。通过后执行体以 root 为基座拼接扫描。
pub fn check_pattern(root: &Path, pattern: &str) -> Result<(), String> {
    let raw_path = Path::new(pattern);
    if raw_path.is_absolute() || pattern.starts_with('~') {
        return Err(format!("glob 模式必须为 root 相对: {pattern}"));
    }
    if raw_path
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(format!("glob 模式不得携带 .. 段: {pattern}"));
    }
    // root 可解析性兜底（与 [`check`] 同口径；失败即 workspace 异常）
    root.canonicalize()
        .map(|_| ())
        .map_err(|e| format!("workspace root 解析失败: {e}"))
}
