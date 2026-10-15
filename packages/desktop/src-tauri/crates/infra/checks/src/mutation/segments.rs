use std::collections::BTreeSet;

use super::{EST_MUTANTS_PER_FILE, SEGMENT_BUDGET_MS};

/// 首段静态估算的保守速率（s/mutant；历史实测 1~2 s/mutant 取上界——静态初值
/// 偏小段、超预算风险优于偏大段）。
pub(crate) const CONSERVATIVE_SEC_PER_MUTANT: f64 = 2.0;

/// 静态估算耗时（毫秒）：`EST_MUTANTS_PER_FILE × 文件数 × 保守速率`。
pub(crate) fn estimate_duration_ms(file_count: usize) -> u64 {
    (file_count as u64)
        .saturating_mul(EST_MUTANTS_PER_FILE)
        .saturating_mul((CONSERVATIVE_SEC_PER_MUTANT * 1000.0) as u64)
}

/// 静态段数（触发口径：估算耗时 > 段预算才切；`k = ceil(估算 / 段预算)`，下限
/// 1——估算恰等段预算不切，超 1 即切）。
pub(crate) fn segment_count(file_count: usize) -> usize {
    let estimate = estimate_duration_ms(file_count);
    if estimate <= SEGMENT_BUDGET_MS {
        return 1;
    }
    estimate.div_ceil(SEGMENT_BUDGET_MS) as usize
}

/// 首段静态初值块大小（字典序均分 k 段的初值：`ceil(n / k)`；单文件超预算接
/// 受——段是软目标，[`super::SEGMENT_GUARD_MS`] 软护栏仅观察留痕）。
pub(crate) fn static_chunk_size(file_count: usize) -> usize {
    if file_count == 0 {
        return 0;
    }
    let parts = segment_count(file_count);
    file_count.div_ceil(parts)
}

/// 自适应块大小（后续段按上一段实测速率——`段预算 / (速率 × 单文件 mutant 密
/// 度)` 折算可容纳文件数；速率越慢段越小、越快段越大，下限 1（单文件超预算接
/// 受），上限 remaining）。
pub(crate) fn adaptive_chunk_size(rate_sec_per_mutant: f64, remaining: usize) -> usize {
    if remaining == 0 {
        return 0;
    }
    if !rate_sec_per_mutant.is_finite() || rate_sec_per_mutant <= 0.0 {
        return remaining;
    }
    let per_file_ms = rate_sec_per_mutant * EST_MUTANTS_PER_FILE as f64 * 1000.0;
    let size = (SEGMENT_BUDGET_MS as f64 / per_file_ms).floor() as usize;
    size.clamp(1, remaining)
}

/// 补丁按文件段切片
pub(crate) fn slice_patch(patch: &str, files: &[String], strip_dir: &str) -> String {
    let wanted: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    let mut sections: Vec<(Option<String>, String)> = Vec::new();
    let mut current: Option<(Option<String>, String)> = None;
    for piece in patch.split_inclusive('\n') {
        if piece.starts_with("diff --git ") {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            let header = piece.trim_end_matches(['\n', '\r']);
            current = Some((header_path(header), piece.to_owned()));
        } else if let Some(section) = current.as_mut() {
            section.1.push_str(piece);
        } else {
            // 段头前的前导文本（异常形态）丢弃
        }
    }
    if let Some(section) = current.take() {
        sections.push(section);
    }
    let mut sliced = String::new();
    for (header, text) in sections {
        // 段头 a/ 侧路径不可辨 → 退回 `+++ b/<path>` 行解析（引用 / 特殊文件名）
        let path = match header {
            Some(path) => Some(path),
            None => plus_plus_path(&text),
        };
        if let Some(path) = path {
            if wanted.contains(path.as_str()) {
                sliced.push_str(&rewrite_section_paths(&text, strip_dir));
            }
        }
    }
    sliced
}

/// 段补丁路径重写（项目根相对 → suite 执行目录相对）：`diff --git a/<p> b/<p>`
/// / `--- a/<p>` / `+++ b/<p>` 三类行剥 suite 根前缀，其余行（`/dev/null` 侧、
/// hunk 体、rename 元数据）逐字保留；`strip_dir` 空串 = suite 锚项目根，全段逐
/// 字透传。路径行尾终止符（`\r` 等）原样保留。
fn rewrite_section_paths(section: &str, strip_dir: &str) -> String {
    section
        .split_inclusive('\n')
        .map(|line| rewrite_path_line(line, strip_dir))
        .collect()
}

/// 单行路径重写（三类路径行剥前缀；不匹配行原样返回）。
fn rewrite_path_line(line: &str, strip_dir: &str) -> String {
    if let Some(rest) = line.strip_prefix("diff --git a/") {
        if let Some(split) = rest.rfind(" b/") {
            let (old, new) = (&rest[..split], &rest[split + 3..]);
            return format!(
                "diff --git a/{} b/{}",
                strip_suite_dir(old, strip_dir),
                strip_suite_dir(new, strip_dir),
            );
        }
    } else if let Some(rest) = line.strip_prefix("--- a/") {
        return format!("--- a/{}", strip_suite_dir(rest, strip_dir));
    } else if let Some(rest) = line.strip_prefix("+++ b/") {
        return format!("+++ b/{}", strip_suite_dir(rest, strip_dir));
    }
    line.to_owned()
}

/// 剥 suite 根前缀（`<strip_dir>/<path>` → `<path>`；不匹配 → 原样——前缀空串
/// 时 `strip_prefix("")` 恒命中、再剥 `/` 不命中即原样，项目根锚自然退化为透传）。
fn strip_suite_dir<'a>(path: &'a str, strip_dir: &str) -> &'a str {
    path.strip_prefix(strip_dir)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(path)
}

/// `diff --git a/<path> b/<path>` 段头路径解析（a 侧；` rfind(" b/")` 兜住路径
/// 含空格——两侧同名，末次 ` b/` 即分界）。引用形态（首字符 `"）→ `None` 交
/// [`plus_plus_path`] 兜底。
fn header_path(header: &str) -> Option<String> {
    let rest = header.strip_prefix("diff --git ")?;
    let rest = rest.trim_end_matches('\r');
    if rest.starts_with('"') {
        return None;
    }
    rest.strip_prefix("a/")?;
    let split = rest.rfind(" b/")?;
    if split <= 2 {
        return None;
    }
    Some(rest[2..split].replace('\\', "/"))
}

/// `+++ b/<path>` 行路径解析（段头不可辨时的兜底；`/dev/null`（删除面）无路径）。
fn plus_plus_path(section: &str) -> Option<String> {
    for line in section.split_inclusive('\n') {
        let line = line.trim_end_matches(['\n', '\r']);
        if line.starts_with("+++ /dev/null") {
            return None;
        }
        if let Some(rest) = line.strip_prefix("+++ b/") {
            let path = rest.split('\t').next().unwrap_or(rest);
            return Some(decode_git_quoted(path).replace('\\', "/"));
        }
    }
    None
}

/// git C 引用路径解码（`"a/\346\226\207.txt"` 形态：八进制转义 → UTF-8 字节；
/// 非引用形态原样返回）。
fn decode_git_quoted(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return trimmed.to_owned();
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut bytes: Vec<u8> = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
            continue;
        }
        let octal: String = chars.by_ref().take(3).collect();
        if octal.len() == 3 && octal.chars().all(|ch| ('0'..='7').contains(&ch)) {
            if let Ok(byte) = u8::from_str_radix(&octal, 8) {
                bytes.push(byte);
                continue;
            }
        }
        // 非八进制转义（`\"` / `\\` / `\t` 等）：转义后字符按 UTF-8 回填
        for escaped in octal.chars() {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(escaped.encode_utf8(&mut buffer).as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
#[path = "segments_test.rs"]
mod segments_test;
