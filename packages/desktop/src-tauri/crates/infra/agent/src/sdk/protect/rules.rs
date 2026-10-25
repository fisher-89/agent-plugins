// ---------------------------------------------------------------------------
// 写入保护规则装配（内置锁定 + 用户规则合并 + 路径归一 / 解析 + 子树相交）
//
// 本文件是 protect 模块**唯一**的 IO 触点（`config::load`），且只在泵起播时
// 被调用一次（`ProtectRules::load`）。判定侧（`mod` / `shell` / `inline` /
// `reason`）零 IO：拿到的是本文件铸造好的纯数据结构。
//
// 两条纪律（规格硬约束）：
// - 内置 glob 与两条结构 glob MUST 由 `foundation::layout` 派生——本源码
//   （含注释与 doc）零域目录名 / 零配置文件名字面量；
// - 规则匹配 MUST 一律经 `checks::globmatch::match_glob`（glob 方言单点）。
//   本文件不自建第二套匹配引擎（不自定匹配选项、不自行展开花括号）。
// ---------------------------------------------------------------------------

use std::path::Path;

use foundation::layout;

/// 单条保护规则：内置 / 用户同型，差别只在 `reason` 变体与位次（内置在前，
/// 首个命中即定，MUST NOT 拼接多条 reason）。
#[derive(Debug, Clone)]
pub(super) struct Rule {
    /// glob 模式（绝对形态原样；相对形态已加 `**/` 前缀；内置与结构 glob 均
    /// 由 `foundation::layout` 派生后加 `**/`）
    pub(super) glob: String,
    /// 拒绝串取串依据（三态载体——不做 glob 字面量比对认内置）
    pub(super) reason: ReasonKind,
}

/// 规则来源三态：`BuiltinConfig` 走内置模板，`User(Some(_))` 透传配置文案，
/// `User(None)` 落通用回落串。
#[derive(Debug, Clone)]
pub(super) enum ReasonKind {
    /// 内置工作区配置锁定（不可配置、不可移除）
    BuiltinConfig,
    /// 用户规则（`reason` 缺席 → `None`）
    User(Option<String>),
}

/// 批量还原拦截用的两条结构 glob（域目录树 / changes 子树），由
/// `foundation::layout` 派生——批量动作对按路径判定的护栏不可见，故走独立的
/// 子树相交判定。
#[derive(Debug, Clone)]
pub(super) struct BulkGlobs {
    /// 域目录树（git clean 的目录参数判定面）
    pub(super) artifacts: String,
    /// changes 子树（git restore / checkout 的 revert 目标判定面）
    pub(super) changes: String,
}

/// 规则合并单点：内置恒在首位；（root 在场时）追加用户规则。
pub(super) fn load(workspace_root: Option<&Path>) -> Vec<Rule> {
    let mut rules = builtin_rules();
    if let Some(root) = workspace_root {
        rules.extend(user_rules(root));
    }
    rules
}

/// 内置锁定：恒一条「`**/` + 配置文件名路径」规则。
///
/// glob 由 `layout::config_path` 的 root 相对形态派生（`*_FILE_NAME` 常量是
/// `foundation` 私有，本源码不自写文件名）；工作流状态文件的插件端内置规则
/// **不复刻**（状态以库为唯一权威，legacy 文件零读取零产出）。
pub(super) fn builtin_rules() -> Vec<Rule> {
    let relative = forward_slash(&layout::config_path(Path::new("")));
    vec![Rule {
        glob: format!("**/{relative}"),
        reason: ReasonKind::BuiltinConfig,
    }]
}

/// 用户规则：逐条取 `write_protection.files[]`。`glob` 缺席跳过（组装层已
/// 剔除空串 / 非法值）；绝对形态原样、相对形态加 `**/`；`reason` 原样保留。
/// `diagnostics` 不参与保护面判定（内置面独立于配置解析结果）。
pub(super) fn user_rules(root: &Path) -> Vec<Rule> {
    let report = config::load(root);
    report
        .config
        .write_protection
        .and_then(|protection| protection.files)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|file| {
            let glob = file.glob?;
            Some(Rule {
                glob: normalize_user_glob(&glob),
                reason: ReasonKind::User(file.reason),
            })
        })
        .collect()
}

/// 批量还原的两条结构 glob（派生单点：域目录树 / changes 子树）。
pub(super) fn bulk_globs() -> BulkGlobs {
    let changes = forward_slash(&layout::resolve(Path::new("")).changes_root);
    BulkGlobs {
        artifacts: format!("**/{}/**", layout::domain_dir_name()),
        changes: format!("**/{changes}/**"),
    }
}

/// 子树相交判定（`find` 搜索根与 `git clean` 目录参数共用）：把 glob 折算成
/// concrete 静态前缀后判三面之一——`前缀 == dir`、`前缀` 以 `dir/` 起手
/// （dir 是前缀的祖先：删整棵 dir 必然波及受保护目标）、`dir` 以 `前缀/` 起手
/// （dir 落在受保护子树内）。
///
/// 不采用「`**/` 规则对任意目录皆可命中」的退化口径：那会让 `find src -delete`
/// 与 `git clean src/` 一并误拦，规格双向断言不允许。`dir` 为相对形态（`find .`
/// 的搜索根）时以 `anchor` 为基座归一；全空白按无目录处理（不误判为命中）；
/// 纯 `**` / 空 glob 不构成可命中的目录规则。
pub(super) fn glob_dir_hit(dir: &str, glob: &str, anchor: &Path) -> bool {
    let raw = dir.trim();
    if raw.is_empty() || !glob_has_body(glob) {
        return false;
    }
    let dir = if is_absolute_text(raw) {
        normalize(raw)
    } else {
        normalize(&anchor.join(raw).to_string_lossy())
    };
    dir_hits_prefix(&dir, &concrete_prefix(glob, anchor))
}

/// 目录与 concrete 前缀的三面相交判定（空目录 / 空前缀 → 不命中；`mod.rs` 的
/// 按规则判定与 [`glob_dir_hit`] 共用同一比较面）。
pub(super) fn dir_hits_prefix(dir: &str, prefix: &str) -> bool {
    if dir.is_empty() || prefix.is_empty() {
        return false;
    }
    dir == prefix
        || prefix.starts_with(&format!("{dir}/"))
        || dir.starts_with(&format!("{prefix}/"))
}

/// glob 是否含实义段（空串 / 纯 `**` 不构成可命中的规则）。
fn glob_has_body(glob: &str) -> bool {
    glob.split('/')
        .any(|segment| !segment.is_empty() && segment != "**" && segment != "**/")
}

impl Rule {
    /// glob → concrete 静态前缀（`**/` 起手者锚定到 `anchor`；首个通配段之后
    /// 一律截断）。
    pub(super) fn concrete_prefix(&self, anchor: &Path) -> String {
        concrete_prefix(&self.glob, anchor)
    }
}

/// 文本归一单点（零 IO；`mod.rs` / `shell.rs` / `concrete_prefix` 共用）：
/// 剥 Windows 扩展长度前缀 → 反斜杠转 `/` → 按 `/` 切段去空段与 `.` 段、
/// **保留** `..` 段 → 重拼并保留前导 `/`。
///
/// 保留 `..` 是刻意的：`**/` 能吸收 `..` 段，消解反而要在无 IO 前提下猜语义；
/// `.` 段必须消解，否则 `find . -delete` 的搜索根与规则前缀无法比较。
pub(super) fn normalize(raw: &str) -> String {
    let text = match raw.strip_prefix(r"\\?\") {
        Some(rest) => rest,
        None => raw,
    };
    let text = text.replace('\\', "/");
    let absolute = text.starts_with('/');
    let mut segments: Vec<&str> = Vec::new();
    for segment in text.split('/') {
        match segment {
            "" | "." => {}
            other => segments.push(other),
        }
    }
    let joined = segments.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

/// 提取路径解析单点（零 IO）：空白串或含 `$` → `None`（未展开变量不可知，
/// 该 op 放行）；绝对路径原样归一；相对路径 `anchor.join` 后归一。
pub(super) fn resolve(anchor: &Path, raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() || text.contains('$') {
        return None;
    }
    if is_absolute_text(text) {
        return Some(normalize(text));
    }
    Some(normalize(&anchor.join(text).to_string_lossy()))
}

/// 绝对路径文本判据：`Path::is_absolute` 为主，另认前导 `/` / `\` 与盘符前缀
/// `X:`（跨平台口径一致——Windows 形态的绝对路径 / glob 在任一平台都按绝对
/// 处理，不做「相对 anchor 拼接」的误判）。
fn is_absolute_text(raw: &str) -> bool {
    raw.starts_with('/')
        || raw.starts_with('\\')
        || Path::new(raw).is_absolute()
        || (raw.len() > 1 && raw.as_bytes().get(1) == Some(&b':'))
}

/// 相对 glob 前缀归一（与插件端同款处理：无通配模式在匹配面走「相等或
/// `pattern/` 起手」语义，不加前缀只命中 root 相对形态）。
fn normalize_user_glob(glob: &str) -> String {
    let text = glob.replace('\\', "/");
    if is_absolute_text(&text) {
        text
    } else {
        format!("**/{text}")
    }
}

/// glob → concrete 静态前缀：`**/` 起手者以 `anchor` 归一文本为基座，其后
/// 只取到首个通配段之前的静态段；无 `**/` 起手者直接取 glob 自身的静态段。
fn concrete_prefix(glob: &str, anchor: &Path) -> String {
    let glob = glob.replace('\\', "/");
    match glob.strip_prefix("**/") {
        Some(rest) => {
            let base = normalize(&anchor.to_string_lossy());
            let tail = static_segments(rest);
            if tail.is_empty() {
                base
            } else if base.is_empty() {
                tail
            } else {
                format!("{base}/{tail}")
            }
        }
        None => static_segments(&glob),
    }
}

/// 首个通配段之前的静态段拼接（通配段与其后内容一律丢弃）。
fn static_segments(glob: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in glob.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment.contains(['*', '?', '[', '{']) {
            break;
        }
        segments.push(segment);
    }
    segments.join("/")
}

/// `Path` → 正斜杠文本（相对形态原样、无前导 `/`）。
fn forward_slash(path: &Path) -> String {
    normalize(&path.to_string_lossy())
}
