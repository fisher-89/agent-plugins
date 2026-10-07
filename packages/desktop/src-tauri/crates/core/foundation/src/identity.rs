//! workspace 身份段派生单点：db 文件名与 worktree 子目录同源消费（design
//! D6——「第二消费者出现才下沉」纪律的正触发，算法与参数自 store.rs 逐字
//! 平移；纯函数零 fs 零 IO，确定性：同根恒同名、跨重启可复现、异根必不同名）。

use sha2::{Digest, Sha256};

/// 可读段截断上限（字符数）：文件名 / 目录名防超长的清洗预算。
const READABLE_SEGMENT_MAX_CHARS: usize = 24;

/// 文件名非法字符（`/` `\` `:` 路径分隔与盘符、`< > " | ? *` Windows 保留、
/// 控制字符不可见）：可读段清洗时逐字置换 `_`。
fn is_illegal_name_char(c: char) -> bool {
    matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
}

/// 末段目录名（canonical root 的 `file_name` 分量；裸根 / 尾分隔形态回退整串）。
fn dir_name(canonical_root: &str) -> String {
    std::path::Path::new(canonical_root)
        .file_name()
        .map(|segment| segment.to_string_lossy().into_owned())
        .unwrap_or_else(|| canonical_root.to_owned())
}

/// 可读段（末段目录名）清洗：非法字符置换 `_` → 按 char boundary 截断
/// ≤24 字符 → 去尾部 `.` 与空格（Windows 保留语义）；清洗后为空返回 `None`。
fn readable_segment(canonical_root: &str) -> Option<String> {
    let cleaned: String = dir_name(canonical_root)
        .chars()
        .map(|c| if is_illegal_name_char(c) { '_' } else { c })
        .collect();
    let truncated: String = cleaned.chars().take(READABLE_SEGMENT_MAX_CHARS).collect();
    let trimmed = truncated.trim_end_matches(['.', ' ']);
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// workspace 身份段（单点）：`{可读段}-{hash}`，可读段清洗后为空回退纯
/// `{hash}`——哈希 = SHA-256(canonical root UTF-8 字节) 前 16 字节的 32 位
/// 小写 hex（128-bit 抗碰撞，异根必不同名）。canonical 锚口径：以命令入参
/// root（前端清单回传的 canonical root 契约）为锚，与 `for_root` 的 canonical
/// key 同信任级别，不重复 canonicalize（纯字符串，零路径 IO）。
pub fn workspace_identity_segment(canonical_root: &str) -> String {
    let digest = Sha256::digest(canonical_root.as_bytes());
    let hash: String = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    match readable_segment(canonical_root) {
        Some(readable) => format!("{readable}-{hash}"),
        None => hash,
    }
}
