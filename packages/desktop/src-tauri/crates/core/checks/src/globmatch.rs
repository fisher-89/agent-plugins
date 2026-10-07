// ---------------------------------------------------------------------------
// Glob 匹配（CLI `matchGlob` / picomatch 方言对齐——含 `{a,b}` 交替展开）
//
// `glob` crate 原生无花括号交替方言（`*.{ts,tsx}` 恒零命中），本模块以
// 展开预处理补齐：交替组展开为无括号变体后复用 `glob::Pattern` 引擎
// （`*` 不跨路径分隔符、dot 文件可匹配——picomatch `{ dot: true }` 对齐）。
// detect（includes 枚举 / excludes 过滤）与 aggregate（suite scope 匹配）
// 三个面的唯一方言实现。
// ---------------------------------------------------------------------------

/// 花括号展开变体数上限（防病态配置组合爆炸；注册表缺省 glob 最大展开 8，
// 用户 includes 常规个位数量级）。
const EXPANSION_LIMIT: usize = 256;

/// 花括号交替展开（picomatch `{a,b}` 方言子集）：最左可展开组（组内任一
/// 深度含逗号——嵌套组随之生效）拆顶层逗号选枝，前缀 / 选枝 / 后缀拼接后
/// 递归。无逗号组（`{foo}`）、未闭合花括号按字面保留；空选枝（`{,a}`）
/// 允许。变体数超上限 → `Err`（枚举面显式报错，匹配面按不命中收敛）。
pub fn try_expand_braces(pattern: &str) -> Result<Vec<String>, String> {
    let Some((prefix, group, suffix)) = first_alternation_group(pattern) else {
        return Ok(vec![pattern.to_owned()]);
    };
    let mut expanded = Vec::new();
    for alternative in split_alternatives(group) {
        expanded.extend(try_expand_braces(&format!(
            "{prefix}{alternative}{suffix}"
        ))?);
        if expanded.len() > EXPANSION_LIMIT {
            return Err(format!(
                "glob 花括号展开超限（>{EXPANSION_LIMIT} 变体）: {pattern}"
            ));
        }
    }
    Ok(expanded)
}

/// 首个可展开组定位：返回 `(前缀, 组内文本, 后缀)`。无逗号组整体字面
/// （跳过闭合括号后继续扫描）；未闭合 `{` 字面（其后继续扫描）。
fn first_alternation_group(pattern: &str) -> Option<(&str, &str, &str)> {
    let bytes = pattern.as_bytes();
    let mut scan = 0;
    while let Some(found) = pattern[scan..].find('{') {
        let open = scan + found;
        let mut depth = 1usize;
        let mut has_comma = false;
        let mut close = None;
        let mut index = open + 1;
        while index < bytes.len() {
            match bytes[index] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(index);
                        break;
                    }
                }
                b',' => has_comma = true,
                _ => {}
            }
            index += 1;
        }
        match close {
            Some(close) if has_comma => {
                return Some((
                    &pattern[..open],
                    &pattern[open + 1..close],
                    &pattern[close + 1..],
                ));
            }
            // 无逗号组：组内任何深度都无逗号即无可展开内容，整体跳过
            Some(close) => scan = close + 1,
            // 未闭合：该 `{` 字面，向后继续找下一组
            None => scan = open + 1,
        }
    }
    None
}

/// 组内顶层逗号切分（嵌套组内的逗号不切——`{a,b{c,d}}` → `a` / `b{c,d}`）。
fn split_alternatives(group: &str) -> Vec<&str> {
    let mut depth = 0usize;
    let mut alternatives = Vec::new();
    let mut start = 0;
    for (index, ch) in group.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                alternatives.push(&group[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    alternatives.push(&group[start..]);
    alternatives
}

/// glob 匹配（CLI `matchGlob` 同语义，含花括号交替方言）：无通配符模式按
/// 目录前缀（相等或 `pattern/` 起手）；含通配符（花括号计入）经花括号
/// 展开 + `glob::Pattern` 全匹配，任一变体命中即命中——无通配符变体仍走
/// 全匹配（picomatch 口径：花括号形态不享受目录前缀语义）。展开超限 /
/// 模式非法按不命中收敛。
pub fn match_glob(path: &str, pattern: &str) -> bool {
    let pattern = to_forward_slash(pattern);
    if !pattern.contains(['*', '?', '{', '[']) {
        let path = to_forward_slash(path);
        return path == pattern || path.starts_with(&format!("{pattern}/"));
    }
    try_expand_braces(&pattern)
        .unwrap_or_default()
        .iter()
        .any(|variant| {
            glob::Pattern::new(variant)
                .map(|compiled| compiled.matches_with(&to_forward_slash(path), match_options()))
                .unwrap_or(false)
        })
}

/// glob 匹配选项（picomatch `{ dot: true }` 对齐：`*` 不跨路径分隔符、
/// dot 文件可匹配）。
fn match_options() -> glob::MatchOptions {
    glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    }
}

/// POSIX 正斜杠归一。
fn to_forward_slash(path: &str) -> String {
    path.replace('\\', "/")
}
