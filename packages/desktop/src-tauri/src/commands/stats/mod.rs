//! 代码统计命令轨道：单命令 `code_stats`——工作区根上一次 tokei library 解析
//! 产出三面 DTO（汇总 / 语言行 / 目录树），无状态薄包装 + `code_stats_inner`
//! 领域组装纯函数（`*_inner` app 层微形态先例，离 Tauri 运行时可测）。
//!
//! 三件事纪律沿 `commands/queries` 模板：参数转换 → 调用 → 错误映射。与既有
//! 查询轨道的 blank root → 空结果语义反向：spec 裁定无效 root（缺失 / 不可读 /
//! 非目录 / 空白）统一 `Err`（壳态 root 恒有值，blank 只能来自调用 bug），
//! `code_stats_inner` 前置 `fs::metadata` 有效性检查作为 Err 通道唯一来源。
//! 解析语义（识别 / hidden / ignore）全部委托 tokei 默认行为
//! （`Config::default()` 零字段覆写，不读任何 tokei 配置文件），命令层零自有
//! 过滤规则；唯一显式排除是规约域目录（用户裁决：规约文档目录非被统计
//! 代码资产），目录名经 `foundation::layout::domain_dir_name` 取全包唯一触点
//! 字面量，走 `get_statistics` 的 `ignored_directories` 通道下发（CLI
//! `--exclude` 同源，gitignore 裸名语义 = 任意层级同名目录整棵剪枝）。无
//! State、无缓存、不落库——每次调用完整重新解析。
//!
//! 深度语义（design「PoC 前置门」定案）：tokei `Config` 无 depth 字段，深度是
//! `code_stats_inner` 的**树面聚合截断参数**而非遍历限制——遍历恒全量，汇总面
//! 与语言面数字不随 depth 变化。树面为目录节点（≤ depth 级）+ 文件叶（父目录
//! 在深度内的直接文件；目录级数 0 = 根层直属文件挂树顶层）的混合条目，深度外
//! 的文件仅并入祖先聚合、不出叶。三面同源自同一批 per-file 报告
//! （`Language::reports`）的三种投影，树面零第二次遍历。
//!
//! 能力 spec：`specs/desktop-workspace-code-stats/spec.md`（路径相对域根）。

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};

use foundation::layout::domain_dir_name;
use serde::Serialize;
use specta::Type;
use tokei::{Config, Languages};

/// 工作区代码统计报告（三面聚合根）：同一次 `Languages::get_statistics` 解析
/// 的三种投影——汇总合计 / 逐语言行 / 目录树（目录节点 + 文件叶混合条目），
/// 三面数字互洽。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CodeStatsReport {
    /// 全部被解析文件合计
    pub totals: CodeTotals,
    /// 逐语言统计（按代码行降序，tie 语言名字典序）
    pub languages: Vec<LanguageStats>,
    /// 树（≤ depth 级目录节点 + 深度内文件叶；根层直属文件为顶层文件叶）
    pub tree: Vec<TreeEntry>,
}

/// 汇总面四项总量；`lines` 不设字段（派生值 `code + comments + blanks` 前端可算）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CodeTotals {
    /// 被识别文件数
    pub files: u64,
    /// 代码行合计
    pub code: u64,
    /// 注释行合计
    pub comments: u64,
    /// 空行合计
    pub blanks: u64,
}

/// 单语言统计行：`name` 取 tokei `LanguageType::name()`；`share` 为代码行份额
/// （该语言 code / Σ全部语言 code，百分点 0–100；Σcode 为 0 时 0.0）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageStats {
    /// 语言名（tokei 识别名，如 "Rust" / "Markdown"）
    pub name: String,
    /// 该语言被识别文件数
    pub files: u64,
    /// 代码行
    pub code: u64,
    /// 注释行
    pub comments: u64,
    /// 空行
    pub blanks: u64,
    /// 代码行份额（百分点 0–100，与排序键同轴）
    pub share: f64,
}

/// 目录树节点：统计为该目录子树内**全部**被解析文件合计（祖先链逐级累计，
/// 含更深文件）；`path` 为相对 root 的 POSIX 路径（`/` 分隔，Windows 下
/// tokei 报告的分隔符混排经 components 归一化）；`children` 为直接子目录 +
/// 直接文件叶（目录先于文件，各自按 `name` 字典序）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DirNode {
    /// 末段目录名
    pub name: String,
    /// 相对 root 的 POSIX 路径（`/` 分隔）
    pub path: String,
    /// 子树内被识别文件数
    pub files: u64,
    /// 子树内代码行合计
    pub code: u64,
    /// 子树内注释行合计
    pub comments: u64,
    /// 子树内空行合计
    pub blanks: u64,
    /// 直接子目录与直接文件叶（≤ depth 截断；目录在前、文件在后，各按 name 字典序）
    pub children: Vec<TreeEntry>,
}

/// 文件叶节点：单个被解析文件的行统计；父目录在深度内时挂其 `children`
/// （目录级数 0 = 根层直属文件挂树顶层），深度外的文件不出叶（仅并入祖先聚合）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    /// 文件名（含扩展名）
    pub name: String,
    /// 相对 root 的 POSIX 路径（`/` 分隔）
    pub path: String,
    /// 代码行
    pub code: u64,
    /// 注释行
    pub comments: u64,
    /// 空行
    pub blanks: u64,
}

/// 树条目信封（tag `kind` 判别，`AgentRunMessage` 同式）：目录节点与文件叶
/// 双变体，前端以 `kind` 收窄行形态（目录可展开、文件叶不可）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TreeEntry {
    /// 目录节点（子树聚合统计，可展开）
    Dir { node: DirNode },
    /// 文件叶（单文件行统计，不可展开）
    File { node: FileNode },
}

/// 工作区代码统计：root 有效性检查 → tokei 单次解析 → 汇总 / 语言 / 树三面组装。
/// 缺失 / 不可读 / 非目录 root 返回 `Err`（MUST NOT panic、MUST NOT 静默空报告）；
/// 目录存在但无被识别文件返回空 report（空态由前端呈现）。
#[tauri::command]
#[specta::specta]
pub fn code_stats(root: String, depth: u32) -> Result<CodeStatsReport, String> {
    code_stats_inner(Path::new(&root), depth)
}

/// 领域组装纯函数（无 Tauri State）：单次 `get_statistics`（规约域目录经
/// `ignored_directories` 在遍历层剪枝）的 per-file 报告同时投喂三面——汇总
/// 逐报告累计、语言逐语言行聚合、目录前缀祖先链逐级累计 + 文件叶归属。
pub fn code_stats_inner(root: &Path, depth: u32) -> Result<CodeStatsReport, String> {
    // Err 通道唯一来源：metadata 失败即缺失 / 不可读，非目录同拒
    let metadata =
        fs::metadata(root).map_err(|e| format!("root 无效（{}）：{e}", root.display()))?;
    if !metadata.is_dir() {
        return Err(format!("root 不是目录（{}）", root.display()));
    }

    // 单次遍历（全链仅此一次）：识别 / hidden / ignore 语义委托 tokei 默认；
    // 规约域目录排除走 ignored_directories（CLI --exclude 同源）遍历层剪枝
    let mut languages = Languages::new();
    languages.get_statistics(&[root], &[domain_dir_name()], &Config::default());

    let mut totals = CodeTotals {
        files: 0,
        code: 0,
        comments: 0,
        blanks: 0,
    };
    let mut rows: Vec<LanguageStats> = Vec::new();
    // 平面目录聚合表：相对 POSIX 目录路径 → 节点（先聚合后嵌套装配）
    let mut dirs: BTreeMap<String, DirNode> = BTreeMap::new();
    // 文件叶表：父目录 POSIX 路径（"" = 根层直属）→ 直接文件叶（name 字典序）
    let mut file_leaves: BTreeMap<String, Vec<FileNode>> = BTreeMap::new();

    for (ty, language) in &languages {
        let mut files = 0u64;
        let mut code = 0u64;
        let mut comments = 0u64;
        let mut blanks = 0u64;
        for report in &language.reports {
            // 前缀剥离 miss 不计入（实际不可达：报告由同一 root 遍历派生）
            let Ok(relative) = report.name.strip_prefix(root) else {
                continue;
            };
            files += 1;
            code += report.stats.code as u64;
            comments += report.stats.comments as u64;
            blanks += report.stats.blanks as u64;
            accumulate_tree(&mut dirs, &mut file_leaves, relative, report, depth);
        }
        rows.push(LanguageStats {
            name: ty.name().to_owned(),
            files,
            code,
            comments,
            blanks,
            share: 0.0,
        });
        totals.files += files;
        totals.code += code;
        totals.comments += comments;
        totals.blanks += blanks;
    }

    // 语言行：代码行降序（排序键与占比键同轴），tie 语言名字典序
    rows.sort_by(|a, b| b.code.cmp(&a.code).then_with(|| a.name.cmp(&b.name)));
    for row in &mut rows {
        row.share = if totals.code == 0 {
            0.0
        } else {
            row.code as f64 / totals.code as f64 * 100.0
        };
    }

    Ok(CodeStatsReport {
        totals,
        languages: rows,
        tree: assemble_tree(dirs, file_leaves),
    })
}

/// components → 相对 root 的 POSIX 路径串（`/` 分隔；Windows 下 tokei 报告的
/// 分隔符混排经 components 归一化）。
fn components_to_posix(components: &[Component]) -> String {
    components
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// 单个 per-file 报告的树面投影（`relative` 为调用方剥离 root 的相对路径）：
/// `components()` 取相对目录链，祖先链逐级累计（子树全量聚合），≤ `depth`
/// 级截断——更深目录的文件统计并入最深可达祖先；父目录在深度内（目录级数
/// ≤ `depth`，0 = 根层直属）时另产文件叶，深度外的文件仅并入聚合、不出叶。
fn accumulate_tree(
    dirs: &mut BTreeMap<String, DirNode>,
    file_leaves: &mut BTreeMap<String, Vec<FileNode>>,
    relative: &Path,
    report: &tokei::Report,
    depth: u32,
) {
    let components: Vec<_> = relative.components().collect();
    let dir_depth = components.len().saturating_sub(1); // 末段为文件名
    let levels = dir_depth.min(depth as usize);
    for level in 1..=levels {
        let path = components_to_posix(&components[..level]);
        let name = components[level - 1]
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        let node = dirs.entry(path.clone()).or_insert_with(|| DirNode {
            name,
            path,
            files: 0,
            code: 0,
            comments: 0,
            blanks: 0,
            children: Vec::new(),
        });
        node.files += 1;
        node.code += report.stats.code as u64;
        node.comments += report.stats.comments as u64;
        node.blanks += report.stats.blanks as u64;
    }
    if dir_depth <= depth as usize {
        let parent = if dir_depth == 0 {
            String::new()
        } else {
            components_to_posix(&components[..dir_depth])
        };
        file_leaves.entry(parent).or_default().push(FileNode {
            name: components[dir_depth]
                .as_os_str()
                .to_string_lossy()
                .into_owned(),
            path: components_to_posix(&components),
            code: report.stats.code as u64,
            comments: report.stats.comments as u64,
            blanks: report.stats.blanks as u64,
        });
    }
}

/// 平面目录表 + 文件叶表 → 嵌套树：父路径恒为子路径真前缀，目录按「直接子
/// 目录」分组递归装配，每父 children = 子目录（name 字典序承 BTreeMap）后接
/// 直接文件叶（name 字典序）——目录先于文件；顶层 = 根层子目录 + 根层直属
/// 文件叶。
fn assemble_tree(
    dirs: BTreeMap<String, DirNode>,
    mut file_leaves: BTreeMap<String, Vec<FileNode>>,
) -> Vec<TreeEntry> {
    let mut by_parent: BTreeMap<String, Vec<DirNode>> = BTreeMap::new();
    for (path, node) in dirs {
        let parent = match path.rsplit_once('/') {
            Some((prefix, _)) => prefix.to_owned(),
            None => String::new(),
        };
        by_parent.entry(parent).or_default().push(node);
    }
    nest_children(&mut by_parent, &mut file_leaves, "")
}

/// 递归装配：取 `parent` 的直接子目录（先递归挂接自身子树）与直接文件叶
/// （递归深度 ≤ depth ≤ 10，无栈风险）。文件叶按 name 字典序在此定序——
/// 累积序随 tokei 报告迭代序非确定，排序是叶序的唯一确定性来源。
fn nest_children(
    by_parent: &mut BTreeMap<String, Vec<DirNode>>,
    file_leaves: &mut BTreeMap<String, Vec<FileNode>>,
    parent: &str,
) -> Vec<TreeEntry> {
    let mut entries: Vec<TreeEntry> = Vec::new();
    for mut node in by_parent.remove(parent).unwrap_or_default() {
        node.children = nest_children(by_parent, file_leaves, &node.path);
        entries.push(TreeEntry::Dir { node });
    }
    let mut files = file_leaves.remove(parent).unwrap_or_default();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    for file in files {
        entries.push(TreeEntry::File { node: file });
    }
    entries
}

#[cfg(test)]
mod mod_test;
