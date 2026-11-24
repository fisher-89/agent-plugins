//! 判定单点：`check(CheckInput) -> Result<Report, CheckError>`。
//!
//! 唯一入口（命令面 `archi_drift` 与门禁 runner `LocalArchiCheck` 都调本函数）：
//! 模型读取（[`crate::dto::graph_view`]，图单点构建）→ 看护配置 → 扫描
//! （[`crate::scan`]）→ ① / ② / ④ 对账 → 定级（**只看引入方**）→ 豁免裁决
//! （声明 > 配置 > 内联）→ 名字级两桶（仅 `apis` 非空的关系：`used ∖ declared` 按
//! 引入方档位定级、`declared ∖ used` 只作观察项，求差单点见 [`api_buckets`]）→
//! `occurrences` 聚合 → `status` 推导 → 报告装配。
//!
//! ④ 段的 `noPath` 按 `kind` 分档：`package` / `module` 空 `path` 记一条警告，
//! `domain` / `composite` 空 `path` 是其标准形态、零告警——但所有空 `path` 元素都
//! 不进 path 存在性循环。
//!
//! 边界：纯读盘、**零写盘**、**零 spawn**（报告落盘在 `crates/infra/archi`，本
//! crate 只出结构）。`status` 推导序见 [`derive_status`]——`api_findings` 经纳管子集
//! （入口文档承诺已成立元素的判据）影响 `status` 与 `passed`：子集内任一 `error` 档
//! 条目把 `status` 抬到 `errors`，`passed` 同步为 `false`。

pub mod config;
pub mod report;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use foundation::cancel::CancelFlag;
use sha2::{Digest, Sha256};

use crate::dto::{self, ElementKind, GateTier};
use crate::scan::{self, DependencyEdge, ScanInput};
use report::{
    ArchiDriftModel, ArchiDriftObservation, ArchiDriftOccurrence, ArchiDriftSuppressedEntry,
    ArchiExcludedFile, ArchiGateCount, ArchiScanReport, ArchiUnresolvedFile,
};

/// 报告结构短名（spec Module Contract 的 `Report`；线面名带 `Archi` 前缀是
/// specta 注册表按裸名唯一的约束）。
pub use report::ArchiDriftReport as Report;
pub use report::{
    ArchiCheckStatus, ArchiDriftEntry, ArchiDriftKind, ArchiDriftMode, ArchiDriftReport,
    ArchiObservationKind, ArchiSuppressVia,
};

/// 报告结构版本号（字段演进须递增并走显式兼容说明）。
///
/// 兼容三点：`version = 1` 的报告没有 `api_findings` 字段；`version = 2` 有该字段但
/// `status` 推导不读契约判据；`version = 3` 起**入口文档承诺已成立**元素的判据纳管进
/// `status` **与** `passed`。消费方按 `version` 分支，MUST NOT 对旧报告补默认值。
const REPORT_VERSION: u32 = 3;

/// 判定域错误面（封闭三值）：取消与失败面必须**可辨**——runner 靠 `Stopped` 映射
/// `fault = Some(RunFault::Stopped)`；配置非法 / 扫描硬失败透传文案（检查不执行、
/// 不产出报告）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// 取消（扫描长尾命中）
    Stopped,
    /// 判定输入资产非法（看护配置未知键 / reason 缺失 / 非元素 FQN / 非法 exclude，
    /// 或契约资产结构非法）
    Config(String),
    /// 扫描硬失败（清单不可解析 / 排除规则编译失败）
    Scan(String),
}

impl CheckError {
    /// 错误面人读文案（`Stopped` → 「检查已取消」；其余透传内层）。
    pub fn message(&self) -> String {
        match self {
            CheckError::Stopped => "检查已取消".to_owned(),
            CheckError::Config(message) | CheckError::Scan(message) => message.clone(),
        }
    }
}

/// 判定入参（观察根 + 模式 + 变更集 + change 上下文 + 取消标志）。
///
/// `root` 单参贯穿：模型与源码同根读盘（MUST NOT 出现「模型读主仓、源码读
/// worktree」的混根）；`changed` 由调用方经 `ArchiveVcsPort::change_edit_set`
/// 取数后注入（本 crate 零 git）。
pub struct CheckInput<'a> {
    /// 观察根（workspace 级 = 主仓根 / change 级 = worktree）
    pub root: &'a Path,
    /// 检查模式
    pub mode: ArchiDriftMode,
    /// 变更集（增量模式只扫其中的源文件；全量模式忽略）
    pub changed: &'a [String],
    /// change 磁盘名（全量模式 / 命令面无 change 上下文时为 `None`）
    pub change: Option<&'a str>,
    /// 取消标志（扫描循环内轮询）
    pub cancel: CancelFlag,
}

/// 判定唯一入口（两消费面同源）。
pub fn check(input: CheckInput<'_>) -> Result<Report, CheckError> {
    let timestamp = now_timestamp();
    let model_hash = model_hash(input.root);

    // ① 模型存在性：目录缺席或零个 `*.toml` → `skipped`（空模型 = 冷启动合法
    //    「clean」，与无模型「没检查」在门禁语义上完全相反，故判据放在读模型之前）
    if model_file_count(input.root) == 0 {
        return Ok(skipped_report(&input, timestamp, model_hash));
    }

    // ② 模型读取（图单点构建：本 crate 内 `ModelGraph::build` 的既有唯一调用点）
    let graph = dto::graph_view(input.root).map_err(CheckError::Scan)?;

    // ③ 模型内元素 FQN 集合 + 看护配置（配置非法即 Err——检查不执行、不落报告）
    let known: BTreeSet<String> = graph
        .elements
        .iter()
        .map(|element| element.fqn.clone())
        .collect();
    let config = config::load_config(input.root, &known).map_err(CheckError::Config)?;

    // ④ 模型警告（`noPath` / `pathNotFound`）与 path 失效条目（按元素档位定级）；
    //    `noPath` 按 `kind` 分档（`domain` / `composite` 豁免）
    let mut model_warnings: Vec<ArchiDriftEntry> = Vec::new();
    let mut path_entries: Vec<ArchiDriftEntry> = Vec::new();
    for element in &graph.elements {
        if element.path.is_empty() {
            if matches!(element.kind, ElementKind::Package | ElementKind::Module) {
                model_warnings.push(ArchiDriftEntry {
                    kind: ArchiDriftKind::NoPath,
                    from: Some(element.fqn.clone()),
                    to: None,
                    gate: Some(element.gate),
                    message: format!(
                        "元素 {} 未声明 path（不参与归属；依赖提取会把它视作无主）",
                        element.fqn
                    ),
                    occurrences: Vec::new(),
                });
            }
            continue;
        }
        for path in &element.path {
            if input.root.join(path).exists() {
                continue;
            }
            let entry = ArchiDriftEntry {
                kind: ArchiDriftKind::PathNotFound,
                from: Some(element.fqn.clone()),
                to: None,
                gate: Some(element.gate),
                message: format!(
                    "元素 {} 的 path 指向不存在的目录 / 文件：{path}",
                    element.fqn
                ),
                occurrences: vec![ArchiDriftOccurrence {
                    file: path.clone(),
                    line: 0,
                    specifier: None,
                    type_only: false,
                }],
            };
            model_warnings.push(entry.clone());
            path_entries.push(entry);
        }
    }

    // ⑤ 扫描（事实面：枚举 / 排除 / 归属 / 抽取 / 聚合）
    let outcome = scan::scan(ScanInput {
        root: input.root,
        mode: input.mode,
        changed: input.changed,
        exclude: &config.exclude,
        elements: &graph.elements,
        cancel: input.cancel,
    })?;

    // ⑥ 对账：声明 vs 依赖事实（按 `(from, to)` 分桶 → 按元素对聚合，不按行铺开）
    let mut errors: Vec<ArchiDriftEntry> = Vec::new();
    let mut warnings: Vec<ArchiDriftEntry> = outcome.findings.clone();
    let mut suppressed: Vec<ArchiDriftSuppressedEntry> = Vec::new();
    let declared: BTreeSet<(String, String)> = graph
        .relations
        .iter()
        .map(|relation| (relation.from.clone(), relation.to.clone()))
        .collect();
    let gates: BTreeMap<&str, GateTier> = graph
        .elements
        .iter()
        .map(|element| (element.fqn.as_str(), element.gate))
        .collect();
    let mut buckets: BTreeMap<(String, String), Vec<&DependencyEdge>> = BTreeMap::new();
    for edge in &outcome.edges {
        buckets
            .entry((edge.from.clone(), edge.to.clone()))
            .or_default()
            .push(edge);
    }
    for ((from, to), edges) in &buckets {
        // 第 1 层豁免：声明即豁免（模型写了 `A -> B`，该边即合法）
        if declared.contains(&(from.clone(), to.clone())) {
            continue;
        }
        // `off` 引入方：本模块不参与判定 → 零违例、零留痕（其边仍被抽取，只用于
        // 「声明是否兑现」的事实核对；定级只看引入方，见 D6）
        if gates.get(from.as_str()) == Some(&GateTier::Off) {
            continue;
        }
        // 第 2 层：配置元素对豁免（留痕）
        if let Some(rule) = config
            .ignore
            .iter()
            .find(|rule| &rule.from == from && &rule.to == to)
        {
            suppressed.push(ArchiDriftSuppressedEntry {
                from: from.clone(),
                to: to.clone(),
                via: ArchiSuppressVia::Config,
                reason: rule.reason.clone(),
                file: None,
                line: None,
            });
            continue;
        }
        // 第 3 层：内联豁免（逐 occurrence 判定——全部命中才抑制；留痕带 file /
        // line）
        if edges.iter().all(|edge| edge.inline_ignore.is_some()) {
            for edge in edges {
                suppressed.push(ArchiDriftSuppressedEntry {
                    from: from.clone(),
                    to: to.clone(),
                    via: ArchiSuppressVia::Inline,
                    reason: edge.inline_ignore.clone().unwrap_or_default(),
                    file: Some(edge.file.clone()),
                    line: Some(edge.line),
                });
            }
            continue;
        }
        let occurrences = occurrences_of(edges);
        // 目标未建模：归因不明 → warning（MUST NOT 记 error）
        if !known.contains(to) {
            warnings.push(ArchiDriftEntry {
                kind: ArchiDriftKind::UndeclaredDependency,
                from: Some(from.clone()),
                to: Some(to.clone()),
                gate: None,
                message: format!("未声明依赖（目标未建模，归因不明）：{from} → {to}"),
                occurrences,
            });
            continue;
        }
        let Some(gate) = gates.get(from.as_str()).copied() else {
            continue;
        };
        let entry = ArchiDriftEntry {
            kind: ArchiDriftKind::UndeclaredDependency,
            from: Some(from.clone()),
            to: Some(to.clone()),
            gate: Some(gate),
            message: format!(
                "未声明依赖：{from} → {to}（引入方 gate={}，{} 处命中）",
                gate.as_str(),
                edges.len()
            ),
            occurrences,
        };
        // 定级只看引入方：strict → errors / warn → warnings / off → 不出现
        match gate {
            GateTier::Strict => errors.push(entry),
            GateTier::Warn => warnings.push(entry),
            GateTier::Off => {}
        }
    }

    // path 失效按元素档位定级（strict → errors / warn 与 off → warnings）
    for entry in path_entries {
        match entry.gate {
            Some(GateTier::Strict) => errors.push(entry),
            _ => warnings.push(entry),
        }
    }

    // ⑦ ① 声明未兑现：永远只作观察项（不进 `status`、不进 `passed`）。`message`
    //    显式声明扫描面——全量模式覆盖观察根下全部源文件，增量模式只覆盖变更集，
    //    后者 MUST NOT 被读成全仓结论。
    let mut observations: Vec<ArchiDriftObservation> = Vec::new();
    for (from, to) in &declared {
        if buckets.contains_key(&(from.clone(), to.clone())) {
            continue;
        }
        let message = match input.mode {
            ArchiDriftMode::Full => format!(
                "全量模式已覆盖观察根下的全部源文件，仍未见 {from} → {to} 的 import\
                 （观察项，不参与门禁判据）"
            ),
            ArchiDriftMode::Incremental => format!(
                "增量模式只覆盖变更集内的源文件，本次扫描面内未见 {from} → {to} 的 import\
                 （观察项，不参与门禁判据）"
            ),
        };
        observations.push(ArchiDriftObservation {
            kind: ArchiObservationKind::UnfulfilledDeclaration,
            from: from.clone(),
            to: to.clone(),
            message,
        });
    }

    // ⑦ᵃ 名字级两桶（前置：关系在模型内且写了 `apis`）：`used ∖ declared` 有证据
    //     （import 是硬事实）⇒ 按引入方档位落 errors / warnings；`declared ∖ used`
    //     无证据（≠ 反证）⇒ 观察项，永不进 `status` / `passed`。`apis` 为空即跳过
    //     （缺省语义是「不声明 API 粒度」，不是「零依赖」）。
    for relation in &graph.relations {
        if relation.apis.is_empty() {
            continue;
        }
        let key = (relation.from.clone(), relation.to.clone());
        let edges: &[&DependencyEdge] = buckets.get(&key).map_or(&[], Vec::as_slice);
        let (used_not_declared, declared_not_used) =
            api_buckets(&relation.apis, &edge_names(edges));
        let Some(gate) = gates.get(relation.from.as_str()).copied() else {
            continue;
        };
        if !used_not_declared.is_empty() {
            let entry = ArchiDriftEntry {
                kind: ArchiDriftKind::UndeclaredApi,
                from: Some(relation.from.clone()),
                to: Some(relation.to.clone()),
                gate: Some(gate),
                message: format!(
                    "用了没声明：{} → {} 的 import 用了 apis 之外的名字（{}）；引入方 \
                     gate={}，{} 处命中",
                    relation.from,
                    relation.to,
                    used_not_declared.join(" / "),
                    gate.as_str(),
                    edges.len()
                ),
                occurrences: occurrences_of(edges),
            };
            match gate {
                GateTier::Strict => errors.push(entry),
                GateTier::Warn => warnings.push(entry),
                GateTier::Off => {}
            }
        }
        if !declared_not_used.is_empty() {
            // 「同一个值 MUST NOT 承载两种缺口」的显式豁免点：`used` 是 under-approx，
            // 两种成因（真没用 / 写错或看不见）不可机械分辨，故只作观察项且文案并列。
            observations.push(ArchiDriftObservation {
                kind: ArchiObservationKind::UnusedDeclaration,
                from: relation.from.clone(),
                to: relation.to.clone(),
                message: format!(
                    "声明没用：{} → {} 的 apis 中 {} 未出现在扫描事实里——可能已不再依赖，\
                     也可能名字写错或该依赖以方法调用 / 内联限定路径 / glob 形态存在而扫描\
                     看不见（观察项，不参与门禁判据；本对无 import 时另有 \
                     unfulfilledDeclaration 一条）",
                    relation.from,
                    relation.to,
                    declared_not_used.join(" / ")
                ),
            });
        }
    }

    // ⑧ 契约判据（与模型面并列；装配单点在 `crate::api::findings`，drift 侧零第二份
    //    装配）。`status` 只吃**纳管子集**（入口文档承诺已成立元素的判据，筛选单点在
    //    `crate::api::gating_findings`）；`passed` 经 `Report::passed` 读同一子集的
    //    `error` 档（唯一判据 `report::has_error_tier`）。
    let api_findings = crate::api::findings(input.root, &graph.elements)
        .map_err(|error| CheckError::Config(error.message()))?;
    let gating = crate::api::gating_findings(input.root, &graph.elements, &api_findings);

    let structural_errors = graph.diagnostics.clone();
    let status = derive_status(
        &structural_errors,
        outcome.report.scanned_files,
        &errors,
        &warnings,
        &gating,
    );
    Ok(Report {
        version: REPORT_VERSION,
        timestamp,
        mode: input.mode,
        root: input.root.to_string_lossy().into_owned(),
        change: input.change.map(str::to_owned),
        base: (input.mode == ArchiDriftMode::Incremental).then(|| "HEAD".to_owned()),
        model_hash,
        model: model_section(&graph, model_warnings, structural_errors),
        scan: outcome.report,
        errors,
        warnings,
        suppressed,
        observations,
        api_findings,
        status,
    })
}

/// `status` 推导（封闭五值，序固定）：
///
/// `skipped`（无模型，调用方早退）→ `errors`（模型结构错 / `errors[]` 非空 /
/// 纳管子集含任一 `error` 档判据）→ `noChanges`（可判文件为 0）→ `warnings`
/// （`warnings[]` 非空 **或** 纳管子集非空且无 `error` 档）→ `clean`。
///
/// **错误面先于 `noChanges`**：结构错、`errors[]` 与 `error` 档判据都必须在「无文件
/// 可判」之前判定——否则会得到「步 failed 且 detail 写没检查」的自相矛盾。按本序
/// `status == errors ⟺ passed == false` 恒成立。
///
/// **纳管两档**：`warn` 档只把 `clean` 抬到 `warnings`；`error` 档把 `clean` /
/// `warnings` / `noChanges` 一律抬到 `errors`（门禁阻断）。
fn derive_status(
    structural_errors: &[dto::ArchiDiagnostic],
    scanned_files: u32,
    errors: &[ArchiDriftEntry],
    warnings: &[ArchiDriftEntry],
    gating: &[report::ArchiApiFinding],
) -> ArchiCheckStatus {
    if !structural_errors.is_empty() || !errors.is_empty() || report::has_error_tier(gating) {
        return ArchiCheckStatus::Errors;
    }
    if scanned_files == 0 {
        return ArchiCheckStatus::NoChanges;
    }
    if !warnings.is_empty() || !gating.is_empty() {
        return ArchiCheckStatus::Warnings;
    }
    ArchiCheckStatus::Clean
}

/// 模型面事实装配（元素 / 关系计数 + gate 分布 + 结构诊断 + 模型警告）。
fn model_section(
    graph: &dto::ArchiGraph,
    warnings: Vec<ArchiDriftEntry>,
    structural_errors: Vec<dto::ArchiDiagnostic>,
) -> ArchiDriftModel {
    let mut by_gate = ArchiGateCount::default();
    for element in &graph.elements {
        match element.gate {
            GateTier::Strict => by_gate.strict += 1,
            GateTier::Warn => by_gate.warn += 1,
            GateTier::Off => by_gate.off += 1,
        }
    }
    ArchiDriftModel {
        elements: graph.elements.len() as u32,
        relations: graph.relations.len() as u32,
        by_gate,
        structural_errors,
        warnings,
    }
}

/// 无模型早退报告（`skipped`）：扫描计数全零、零条目、零豁免、零观察项。
fn skipped_report(input: &CheckInput<'_>, timestamp: String, model_hash: String) -> Report {
    Report {
        version: REPORT_VERSION,
        timestamp,
        mode: input.mode,
        root: input.root.to_string_lossy().into_owned(),
        change: input.change.map(str::to_owned),
        base: (input.mode == ArchiDriftMode::Incremental).then(|| "HEAD".to_owned()),
        model_hash,
        model: ArchiDriftModel {
            elements: 0,
            relations: 0,
            by_gate: ArchiGateCount::default(),
            structural_errors: Vec::new(),
            warnings: Vec::new(),
        },
        scan: ArchiScanReport {
            mode: input.mode,
            changed_files: 0,
            scanned_files: 0,
            excluded: Vec::<ArchiExcludedFile>::new(),
            unmatched_files: Vec::new(),
            matched: Vec::new(),
            unresolved: Vec::<ArchiUnresolvedFile>::new(),
        },
        errors: Vec::new(),
        warnings: Vec::new(),
        suppressed: Vec::new(),
        observations: Vec::new(),
        api_findings: Vec::new(),
        status: ArchiCheckStatus::Skipped,
    }
}

/// 依赖 API 两桶求差（**唯一实现**：drift 对账与 `relation::relation_detail` 都只调
/// 本函数，MUST NOT 第二份集合差 / 第二处 `contains`）。
///
/// 返回 `(used ∖ declared, declared ∖ used)`；两侧各自去重 + 字典序（同一名字在多
/// 文件命中只算一项，两消费面对同一元素对输出逐字相同）。比对**逐字符**：MUST NOT
/// 大小写折叠、MUST NOT 驼峰归一——否则判定侧常驻第二处映射规则。
///
/// 两侧不对称的原因在取数精度：`used` 是 under-approx（方法调用、内联限定路径、
/// 观察根无 workspace 清单时的 Rust 事实都看不见），故 `declared ∖ used` 只作观察项。
pub fn api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>) {
    let declared: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    let used: BTreeSet<&str> = used.iter().map(String::as_str).collect();
    let used_not_declared = used
        .difference(&declared)
        .map(|name| (*name).to_owned())
        .collect();
    let declared_not_used = declared
        .difference(&used)
        .map(|name| (*name).to_owned())
        .collect();
    (used_not_declared, declared_not_used)
}

/// 元素对的事实名并集（去重 + 字典序）：drift 两桶装配与 `relation` 的按需取样共用
/// 同一聚合口径。
pub(crate) fn edge_names(edges: &[&DependencyEdge]) -> Vec<String> {
    scan::normalize_names(
        edges
            .iter()
            .flat_map(|edge| edge.names.iter().cloned())
            .collect::<Vec<String>>(),
    )
}

/// 命中点聚合（按文件 / 行排序，同元素对不再铺开）。`pub(crate)`：`relation` 的按需
/// 取样复用同一聚合形状与排序（MUST NOT 另拼第二份）。
pub(crate) fn occurrences_of(edges: &[&DependencyEdge]) -> Vec<ArchiDriftOccurrence> {
    let mut occurrences: Vec<ArchiDriftOccurrence> = edges
        .iter()
        .map(|edge| ArchiDriftOccurrence {
            file: edge.file.clone(),
            line: edge.line,
            specifier: Some(edge.specifier.clone()),
            type_only: edge.type_only,
        })
        .collect();
    occurrences.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.line.cmp(&right.line))
    });
    occurrences
}

/// RFC3339 UTC 时间戳（`2026-10-20T08:31:12Z` 形态）。
fn now_timestamp() -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// 模型声明文件清单（`graph/model/*.toml`，按文件名序；缺席 → 空）。
fn model_files(root: &Path) -> Vec<PathBuf> {
    let dir = foundation::layout::archi_graph_model_dir(root);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    files.sort();
    files
}

/// 模型文件数（`skipped` 判据）。
fn model_file_count(root: &Path) -> usize {
    model_files(root).len()
}

/// `graph/model/*.toml` 聚合哈希（按文件名序逐份累加字节 → `sha256:<hex>`；读失败
/// 按空内容参与累加，哈希仍确定）。累加输入仅文件名 + 字节、不含父目录——模型声明
/// 目录迁移不改变哈希。
fn model_hash(root: &Path) -> String {
    let dir = foundation::layout::archi_graph_model_dir(root);
    hash_files(&model_files(root), &dir)
}

/// 累加 helper（`model_hash` 与 [`assets_hash`] 共用）：按给定序逐文件累加「相对
/// `base` 的路径 + 字节」，空字节分隔，输出 `sha256:<hex>`；读失败按空字节参与。
fn hash_files(files: &[PathBuf], base: &Path) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        let name = file
            .strip_prefix(base)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        let bytes = std::fs::read(file).unwrap_or_default();
        hasher.update(&bytes);
        hasher.update([0u8]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

/// 架构资产聚合哈希（四类覆盖面：`graph/model/*.toml`、`models/**/*.md`、
/// `models/**/*.schema.json`、`graph/INDEX.toml`）。
///
/// 用途单一是收口自检的「本轮是否动过架构资产」判据：MUST NOT 写进报告、MUST NOT
/// 取代报告的 `modelHash`、MUST NOT 当作任何缓存的失效依据。同一份盘面必得同一值
/// ——故遍历**不走 `ignore`**（`.gitignore` 状态不得污染哈希）。缺失目录不参与、
/// 单文件读失败按空字节；纯读盘、零写盘、零 spawn。
pub fn assets_hash(root: &Path) -> String {
    hash_files(&collect_assets(root), root)
}

/// 四类资产的盘面清单（相对 `root` 的路径字典序——稳定序是哈希确定性的前提）。
fn collect_assets(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_by_extension(
        &foundation::layout::archi_graph_model_dir(root),
        &|name| name.ends_with(".toml"),
        &mut files,
    );
    collect_by_extension(
        &foundation::layout::archi_models_dir(root),
        &|name| name.ends_with(".md") || name.ends_with(".schema.json"),
        &mut files,
    );
    let index = foundation::layout::archi_graph_dir(root).join(crate::api::paths::INDEX_FILE_NAME);
    if index.is_file() {
        files.push(index);
    }
    files.sort_by_key(|file| {
        file.strip_prefix(root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/")
    });
    files
}

/// 递归收集 `dir` 下文件名满足 `keep` 的文件（不跟随符号链接；目录缺席即零条目）。
fn collect_by_extension(dir: &Path, keep: &dyn Fn(&str) -> bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            collect_by_extension(&path, keep, files);
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(keep)
        {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod config_test;
#[cfg(test)]
mod mod_test;
#[cfg(test)]
mod report_test;
