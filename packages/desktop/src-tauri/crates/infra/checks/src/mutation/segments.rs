use std::collections::BTreeSet;

use super::{EST_MUTANTS_PER_FILE, SEGMENT_SPAWN_TIMEOUT_MS};

/// 每突变体的墙钟秒数（s/mutant）——整体估算的**唯一**待校准常数。
///
/// 取**墙钟**口径而非 CPU 值：单突变体各阶段 CPU 耗时合计远高于墙钟（已被
/// cargo-mutants 的默认 `--jobs` 摊薄），而执行超时是墙钟量，故速率也取墙钟。
///
/// **本值随项目规模变化**（每突变体的编译 + 测试成本取决于被突变的包有多大），
/// 换仓库时应按新仓库实测重标：一次真实 run 后取 `outcomes.json` 里
/// `Σ(build + test) / 突变体数`（或直接取中位数），再留一档上界。本仓 rust 面实测
/// 约 8.5 s/mutant，取上界 **10.0**——低估会撞硬超时使整次执行作废，高估只让护栏
/// 更宽，两害相权取高估。
pub(crate) const SEC_PER_MUTANT: f64 = 10.0;

/// 执行超时的保守倍数（估算耗时 × 本值 = spawn 硬超时）。
///
/// 实测速率有 ±20% 离散，2 倍足以吸收机器负载抖动与慢用例；更宽的护栏没有额外
/// 收益——真挂死的执行会饱和在 per-mutant 超时上，由 cargo-mutants 自身判定。
pub(crate) const TIMEOUT_SAFETY_FACTOR: f64 = 2.0;

/// 整体执行耗时估算（秒）：突变体数 × 单突变体墙钟。
///
/// **不含固定开销项**：冷编与源码树复制的成本随项目规模变化，写死一个秒数换仓库
/// 必然失真。小规模执行的冷启动由 [`SEGMENT_SPAWN_TIMEOUT_MS`] 这个**下限**兜住
/// ——「一小时」对任何项目都是同一个量，不需要按仓库校准：
///
/// | 突变体 | 本式估算 | 实际耗时（本仓拟合） | 生效超时 |
/// |---|---|---|---|
/// | 5 | 50 s | ≈ 250 s | 3600 s（下限） |
/// | 180 | 1800 s | ≈ 1740 s | 3600 s（下限） |
/// | 739 | 7390 s | ≈ 6490 s | 14780 s（2.3× 余量） |
///
/// 分母口径 MUST 是 `--list` 给出的精确突变体数；不可用时由 [`estimate_mutants`]
/// 按文件数 × 密度兜底。
pub(crate) fn estimate_duration_sec(mutant_count: u64) -> f64 {
    mutant_count as f64 * SEC_PER_MUTANT
}

/// 突变体数兜底估算（`cargo mutants --list` 不可用时）：文件数 × 单文件密度。
///
/// **分母口径 MUST 是文件数**，MUST NOT 是「实际产出突变体的文件数」——后者只数
/// 有产出的文件，会把密度虚高数倍。实测本仓 rust 面 302 突变体 / 14 文件 = 21.6。
pub(crate) fn estimate_mutants(file_count: usize) -> u64 {
    (file_count as u64).saturating_mul(EST_MUTANTS_PER_FILE)
}

/// 整体执行 spawn 硬超时（毫秒）：估算耗时 × 安全倍数，下限是
/// [`SEGMENT_SPAWN_TIMEOUT_MS`]（短面 / 零突变体逐字不变）。
///
/// 语义是**防悬挂护栏**，不是耗时目标。突变体数取精确值时（`--list` 可用）估算才
/// 有意义；取兜底密度时同样成立——兜底本身就是保守上界。
///
/// 全程饱和运算：超大突变体数不溢出。
pub(crate) fn run_spawn_timeout_ms(mutant_count: u64) -> u64 {
    let estimated = estimate_duration_sec(mutant_count) * TIMEOUT_SAFETY_FACTOR * 1000.0;
    if !estimated.is_finite() || estimated <= 0.0 {
        return SEGMENT_SPAWN_TIMEOUT_MS;
    }
    (estimated as u64).max(SEGMENT_SPAWN_TIMEOUT_MS)
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
