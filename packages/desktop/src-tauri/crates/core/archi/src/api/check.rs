//! 十三条契约判据的单点求值：`check(ApiCheckInput) -> Vec<ArchiApiFinding>`。
//!
//! **求值序固定：按 `api` 状态分两条互斥支链，支内首错短路**（一个元素至多一条元素
//! 锚点；跨支不比较，同一元素只走一支）：
//!
//! ```text
//! api = none 支（三步；不依赖入口）
//!   ① apiUnexpected   派生契约路径在盘 ⇒ 留白被违反（与 doc 无关，必须先判）
//!   ② docMissing      `doc` 未声明（无入口文档）
//!   ③ docNotFound     `doc` 声明在案但指不到盘
//!
//! api = declared 支（六步）
//!   ④ docMissing      派生契约 md 不在盘
//!   ⑤ apiDocMissing   契约 md 在盘但正文无 api fence 语言标
//!   ⑥ apiUnexpected   元素写了 `doc`（入口文档已由派生落点给出 ⇒ 两事实矛盾）
//!   ⑦ apiEntryMissing entry_state = Absent / Invalid（code 面的结构前提）
//!   ⑧ apiMissing      契约声明 op / `$defs` 类型而代码导出集里没有
//!   ⑨ 一致性（首条命中即停）：apiSignatureMismatch → apiReexportDrift →
//!                      apiConstructorDeclared
//!
//! pair 锚点（不进元素循环，与元素锚点并列、互不挤占）
//!   ⓐ apiNestedCrossCrate → ⓑ apiRefMissing → ⓒ apiShadowing
//! ```
//!
//! **分界不是排版而是依赖**：①–⑥ 只读模型 + 盘（载体存活性 / 契约路径存活性 / fence
//! 有无），**零导出集依赖**——载体事实在装载期由 `api::doc_facts` 一次装配进
//! `input.doc_facts`，本文件仍零内容读；⑦ 起经导出集需 `EntryState::Resolved`——入口非
//! `Resolved` 时导出集为空，⑧⑨ 无对象可判，故短路的不是真问题而是**尚不存在的对象**。
//!
//! **`api = none` 支把 `apiUnexpected` 排第一**：该判据与 `doc` 无关（判的是「派生路径
//! 上冒出了契约」），而 `docMissing` 是 `warn` 且不纳管——若让 `docMissing` 先短路，一条
//! 没写 README 的 `api = none` 元素就能把它那份放错位置的契约遮住。
//!
//! **`$defs` 只读 `schema.json`**（`input.schemas`），MUST NOT 并读 markdown：安全性由
//! 前序门禁保证（`contract::verify` 保证 fence 与 `$defs` 一致、零 diff 门保证 schema
//! 零手改自由度）。
//!
//! **类型词汇与归一单点**（D43）：逻辑类型词汇、位置词汇与短形写出点各只有一份，全在
//! [`extract`] 内（本文件零位置表、零短形拼装）。契约侧签名经 [`extract::signature_of`]、
//! 代码侧经取数器的归一签名函数——两者同一短形词汇，本文件只调用不自建；「位置 × 语言 →
//! 文本形」的映射只在 [`super::emit`]。
//!
//! **零第二份判定**：本文件是十三条判据的唯一实现，命令面（`archi_drift`）与门禁面
//! （`LocalArchiCheck`）都经 [`crate::api::check`] 抵达这里。
//!
//! 边界：纯读 `input`（零读盘、零写盘、零 spawn）。

use std::collections::BTreeSet;

use serde_json::Value;

use crate::drift::report::{
    ApiFindingSeverity, ArchiApiFinding, ArchiApiFindingKind, ArchiDriftOccurrence,
};
use crate::dto::{ApiStatus, ArchiElement};

use super::contract::{is_constructor, Contract};
use super::extract::{self, ApiSymbol, ApiSymbolKind};
use super::paths::{self, EntryProblem, EntryState};
use super::{ApiCheckInput, DocFact};

/// 开放分类的单点打标（七值）：新增分类只改本函数，不动枚举、不动 `ALL`、不动金样
/// 词汇对拍。
pub fn category_of(kind: ArchiApiFindingKind) -> &'static str {
    match kind {
        ArchiApiFindingKind::ApiEntryMissing | ArchiApiFindingKind::ApiUnexpected => "entry",
        ArchiApiFindingKind::ApiMissing | ArchiApiFindingKind::ApiUndeclared => "contract",
        ArchiApiFindingKind::ApiSignatureMismatch => "signature",
        ArchiApiFindingKind::ApiReexportDrift
        | ArchiApiFindingKind::ApiShadowing
        | ArchiApiFindingKind::ApiRefMissing => "reexport",
        ArchiApiFindingKind::ApiNestedCrossCrate => "nesting",
        ArchiApiFindingKind::ApiConstructorDeclared => "constructor",
        ArchiApiFindingKind::DocMissing
        | ArchiApiFindingKind::DocNotFound
        | ArchiApiFindingKind::ApiDocMissing => "doc",
    }
}

/// 十三条判据求值（单点实现；返回前不做排序——排序在 [`super::check`] 的对外入口）。
///
/// 元素锚点两支三段各自首错短路；三条 pair 锚点在元素循环之外，与元素锚点互不挤占。
pub fn check(input: ApiCheckInput<'_>) -> Vec<ArchiApiFinding> {
    let mut findings: Vec<ArchiApiFinding> = Vec::new();
    for element in input.elements {
        if element.path.is_empty() {
            continue;
        }
        // 载体事实由装载期装配（`doc_facts` 以同一份元素表装配 ⇒ 本处缺表项不可达；
        // 兜底取 `Absent` 只是零 panic 而非判据分流——「已声明而越根」那类成因在
        // `doc_facts` 里已被归 `Missing`，MUST NOT 在此另写第二份分流）
        let fact = input
            .doc_facts
            .get(&element.fqn)
            .copied()
            .unwrap_or(DocFact::Absent);
        match element.api {
            ApiStatus::None => {
                // ① 留白被违反：派生契约路径上冒出契约（与 `doc` 无关，必须先判）
                if paths::contract_path(input.root, &element.fqn).is_some_and(|path| path.is_file())
                {
                    findings.push(finding(
                        ArchiApiFindingKind::ApiUnexpected,
                        element,
                        None,
                        format!(
                            "元素 {} 声明 `api = none`（本模块无 API），但派生契约路径上存在契约文件——要么删掉契约，要么把模型改成 `declared`",
                            element.fqn
                        ),
                        Vec::new(),
                    ));
                    continue;
                }
                match fact {
                    // ② 未声明 `doc`：无入口文档（warn，永不纳管）
                    DocFact::Absent => findings.push(doc_missing_none(element)),
                    // ③ 声明在案但指不到盘（error，纳管）
                    DocFact::Missing => findings.push(doc_not_found(element)),
                    DocFact::Present { .. } => {}
                }
                continue;
            }
            ApiStatus::Declared => {}
        }
        // ④ 派生契约 md 不在盘（原 `apiMissing` 形态①，改名不改事实）
        match fact {
            DocFact::Missing | DocFact::Absent => {
                findings.push(doc_missing_declared(input.root, element));
                continue;
            }
            // ⑤ 有文档无约定：契约 md 在盘但正文无 api fence 语言标
            DocFact::Present {
                has_api_fence: false,
            } => {
                findings.push(api_doc_missing(input.root, element));
                continue;
            }
            DocFact::Present {
                has_api_fence: true,
            } => {}
        }
        // ⑥ 入口文档已由派生落点给出，`doc` 是第二个事实 ⇒ 矛盾（该值不参与其它判定）
        if element.doc.is_some() {
            findings.push(finding(
                ArchiApiFindingKind::ApiUnexpected,
                element,
                None,
                format!(
                    "元素 {} 声明 `api = declared`（入口文档 = 派生契约 md），却又写了 `doc`——入口文档由派生落点给出，`doc` 是第二个事实（要么删掉 `doc`，要么改判 `api = none`）",
                    element.fqn
                ),
                Vec::new(),
            ));
            continue;
        }
        // ⑦ code 面的结构前提：`entry_state` 非 `Resolved` ⇒ 导出集为空，⑧⑨ 无对象可判
        let state = paths::entry_state(input.root, input.elements, element);
        if !matches!(state, EntryState::Resolved { .. }) {
            findings.push(finding(
                ArchiApiFindingKind::ApiEntryMissing,
                element,
                None,
                entry_missing_message(element, &state),
                Vec::new(),
            ));
            continue;
        }
        let contract = input
            .contracts
            .iter()
            .find(|contract| contract.fqn == element.fqn);
        let schema = input.schemas.get(&element.fqn);
        let symbols = input
            .symbols
            .get(&element.fqn)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        // ⑧ 契约 → 代码缺失（只剩「有约定无兑现」形态）
        if let Some(found) = missing_contracts(element, contract, schema, symbols) {
            findings.push(found);
            continue;
        }
        // ⑨ 一致性（首条命中即停）：签名 → 透传 → 构造器
        if let Some(found) = signature_mismatch(element, contract, symbols) {
            findings.push(found);
            continue;
        }
        if let (Some(contract), Some(schema)) = (contract, schema) {
            if let Some(found) = reexport_drift(element, contract, schema) {
                findings.push(found);
                continue;
            }
        }
        if let Some(found) = constructor_declared(element, contract) {
            findings.push(found);
            continue;
        }
        // 完备性判据（档位 `off` ⇒ 本步恒不产出；保留实现以便将来单值收紧）
        if ArchiApiFindingKind::ApiUndeclared.severity() != ApiFindingSeverity::Off {
            if let Some(found) = undeclared(schema, symbols, element) {
                findings.push(found);
            }
        }
    }

    // pair 锚点（不进元素循环、不吃元素名额）：结构前提 → 目标可达性 → 命名冲突
    nested_cross_crate(&input, &mut findings);
    ref_missing(&input, &mut findings);
    shadowing(&input, &mut findings);
    findings
}

/// `api = none` 且未声明 `doc`（成因：入口文档未就位）。
fn doc_missing_none(element: &ArchiElement) -> ArchiApiFinding {
    finding(
        ArchiApiFindingKind::DocMissing,
        element,
        None,
        format!(
            "元素 {} 声明 `api = none` 且未声明 `doc`——本模块没有入口文档（要给出入口文档，写 `doc = \"<repo 相对路径>\"`）",
            element.fqn
        ),
        Vec::new(),
    )
}

/// `api = declared` 而派生契约 md 不在盘（成因：入口文档未就位；文案点名派生路径）。
fn doc_missing_declared(root: &std::path::Path, element: &ArchiElement) -> ArchiApiFinding {
    let expected = paths::contract_path(root, &element.fqn)
        .map(|path| super::display(root, &path))
        .unwrap_or_else(|| format!("<无法派生：FQN `{}` 形态非法>", element.fqn));
    finding(
        ArchiApiFindingKind::DocMissing,
        element,
        None,
        format!(
            "元素 {} 声明 `api = declared`（入口文档 = 派生契约 md）但该文件不在盘：{expected}",
            element.fqn
        ),
        Vec::new(),
    )
}

/// `api = none` 且 `doc` 指不到盘（成因：声明写错；文案点名声明值）。
fn doc_not_found(element: &ArchiElement) -> ArchiApiFinding {
    let declared = element.doc.clone().unwrap_or_default();
    finding(
        ArchiApiFindingKind::DocNotFound,
        element,
        None,
        format!(
            "元素 {} 的 `doc` 声明 `{declared}` 指不到盘上的文件——要么改对路径，要么删掉声明",
            element.fqn
        ),
        Vec::new(),
    )
}

/// 契约 md 在盘但正文无 api fence 语言标（成因：契约尚未展开）。
fn api_doc_missing(root: &std::path::Path, element: &ArchiElement) -> ArchiApiFinding {
    let carrier = paths::contract_path(root, &element.fqn)
        .map(|path| super::display(root, &path))
        .unwrap_or_else(|| format!("<无法派生：FQN `{}` 形态非法>", element.fqn));
    finding(
        ArchiApiFindingKind::ApiDocMissing,
        element,
        None,
        format!(
            "元素 {} 的派生契约 md 在盘（{carrier}）但正文无 `api-op` / `api-types` 语言标——契约尚未展开（补 fence 之前本元素无声明集可对拍）",
            element.fqn
        ),
        Vec::new(),
    )
}

/// ① 的文案（R-3）：**按 `EntryProblem` 分列理由**，MUST NOT 只写通用「无入口」。
///
/// 四条判据同报一个 finding kind、同档同分类（入口面因此不需要第 11 个值）；理由只能
/// 落在文案里，故必须点名具体对象——越界目标 / 缺失文件 / 冲突方 FQN / 形态非法的声明值。
fn entry_missing_message(element: &ArchiElement, state: &EntryState) -> String {
    match state {
        EntryState::Resolved { entry, .. } => {
            format!("元素 {} 已解析出可用入口 {entry}", element.fqn)
        }
        EntryState::Absent => {
            let candidates = paths::entry_candidates(element);
            let expected = if candidates.is_empty() {
                "（`path` 为空或形态非法，无候选可派生）".to_owned()
            } else {
                format!(
                    "约定候选 {} 皆不在盘",
                    candidates
                        .iter()
                        .map(|candidate| format!("`{candidate}`"))
                        .collect::<Vec<String>>()
                        .join(" / ")
                )
            };
            format!(
                "元素 {} 无可用入口：未声明 `entry`，且{expected}（目录型根缺 \
                 `index.ts(x)`、或该元素本就没有门）——补约定入口、显式声明 `entry`，\
                 或判 `api = none`",
                element.fqn
            )
        }
        EntryState::Invalid(EntryProblem::Malformed { entry }) => format!(
            "元素 {} 声明的入口 `{entry}` 形态非法：入口 MUST 是 repo 相对 POSIX 路径\
             （无绝对前缀 / 盘符 / `..` 段 / 空段）",
            element.fqn
        ),
        EntryState::Invalid(EntryProblem::OutsidePath { entry }) => format!(
            "元素 {} 声明的入口 `{entry}` 落在该元素的 `path` 之外：入口 MUST 等于某条 \
             `path` 条目，或落在某条目录型 `path` 之内",
            element.fqn
        ),
        EntryState::Invalid(EntryProblem::NotOnDisk { entry }) => format!(
            "元素 {} 声明的入口 `{entry}` 不在盘上——写契约之前先修入口或删掉声明",
            element.fqn
        ),
        EntryState::Invalid(EntryProblem::Duplicated { entry, other_fqn }) => format!(
            "元素 {} 声明的入口 `{entry}` 与元素 {other_fqn} 是同一个文件——\
             一个入口只能属于一个模块",
            element.fqn
        ),
    }
}

/// `apiMissing`：**② 只剩「有约定无兑现」**（契约声明了代码里没有的实现）。
///
/// **`$defs` 类型面与 op 面同一条判据、同一次求值**（D44）：两类都**只判存在**（签名
/// 归一致性第一步），共同排除面三条——**构造器双向排除**（`is_constructor` 既不进声明集也不进
/// 导出集）、**透传纯 `$ref` alias 排除**（`api-reexport` 的条目在本模块无本地定义，
/// 其实现由目标元素承担；判据与 [`reexport_drift`] 同源，不另写一份）、**签名不在此判**。
/// `ApiSymbolKind::{Type, Trait, Enum}` 与 `Function` 各归各自的面，MUST NOT 交叉。
///
/// **`occurrences.line` 按两类缺口分列**（D51）：op 缺口取 [`ContractOp::line`]——它是
/// `### <op-id>` **heading 行**（人读定位的锚点，**不是** `api-op` fence 行，两者相邻但
/// 不是同一行）；`$defs` 类型缺口取 `0`（[`ArchiDriftOccurrence::line`] 的既有「位置未知」
/// 口径，见 [`declared_types`] 的 doc）。
///
/// [`ContractOp::line`]: super::contract::ContractOp::line
/// [`ArchiDriftOccurrence::line`]: crate::drift::report::ArchiDriftOccurrence::line
///
/// **契约文件缺失不再归本判据**（原形态① 已改名 `docMissing`）：同一事实不占两个名字。
fn missing_contracts(
    element: &ArchiElement,
    contract: Option<&Contract>,
    schema: Option<&Value>,
    symbols: &[ApiSymbol],
) -> Option<ArchiApiFinding> {
    let contract = contract?;
    let exports: Vec<&ApiSymbol> = symbols
        .iter()
        .filter(|symbol| symbol.kind == ApiSymbolKind::Function)
        .filter(|symbol| !is_constructor(&symbol.name))
        .collect();
    let types: Vec<&ApiSymbol> = symbols
        .iter()
        .filter(|symbol| is_type_symbol(symbol.kind))
        .filter(|symbol| !is_constructor(&symbol.name))
        .collect();
    let mut occurrences: Vec<ArchiDriftOccurrence> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for op in &contract.ops {
        if is_constructor(&op.id) {
            continue;
        }
        if exports.iter().any(|symbol| symbol.name == op.id) {
            continue;
        }
        missing.push(op.id.clone());
        occurrences.push(ArchiDriftOccurrence {
            file: contract.file.clone(),
            line: op.line,
            specifier: Some(op.id.clone()),
            type_only: false,
        });
    }
    for name in declared_types(schema) {
        if is_constructor(&name) {
            continue;
        }
        if types.iter().any(|symbol| symbol.name == name) {
            continue;
        }
        missing.push(name.clone());
        occurrences.push(ArchiDriftOccurrence {
            file: contract.file.clone(),
            line: 0,
            specifier: Some(name),
            type_only: false,
        });
    }
    if missing.is_empty() {
        return None;
    }
    Some(finding(
        ArchiApiFindingKind::ApiMissing,
        element,
        Some(missing.join(" / ")),
        format!(
            "契约 {} 声明了 {} 但代码导出集里没有实现（代码导出集：{}）",
            contract.file,
            missing.join(" / "),
            if exports.is_empty() {
                "空".to_owned()
            } else {
                exports
                    .iter()
                    .map(|symbol| symbol.name.as_str())
                    .chain(types.iter().map(|symbol| symbol.name.as_str()))
                    .collect::<Vec<&str>>()
                    .join(" / ")
            }
        ),
        occurrences,
    ))
}

/// 类型类符号的形态判据（`Type` / `Trait` / `Enum` 同列类型面，D44；`Function` 不在此列）。
fn is_type_symbol(kind: ApiSymbolKind) -> bool {
    matches!(
        kind,
        ApiSymbolKind::Type | ApiSymbolKind::Trait | ApiSymbolKind::Enum
    )
}

/// 契约声明的类型名清单（`$defs` 键集，**纯 `$ref` alias 排除**，D44）。
///
/// 取数面是**已装配的 `schema.json`**（`input.schemas`）：它的 `$defs` 恰是 `api-types` ∪
/// `api-reexport` 两处 fence 的装配结果，故「取 schema 即取两处」——MUST NOT 并读
/// markdown（D17：同一事实的第二个来源必然漂移）。**排除面与 [`reexport_drift`] 同一判据**
/// （[`is_pure_ref_alias`]）：透传条目在本模块无本地定义、其实现由目标元素承担，参与
/// `apiMissing` 会让每个父子契约恒报缺失。
///
/// **行号取 `0`（位置未知，D51）**：`Contract` 不承载 `api-types` / `api-reexport` 的 fence
/// 条目级行号（`types` / `reexports` 是 fence 原文的裸 `Map`），而
/// [`ArchiDriftOccurrence::line`] 的既有口径已用 `0` 表达「位置未知」（`drift/mod.rs` /
/// `parse/mod.rs` / `scan/mod.rs` 三处先例）——MUST NOT 为凑行号在 `Contract` 上新增
/// `types_line` / `reexports_line` 一类只此一处消费的字段（扩公共类型面换「行号更准」这一
/// 非判据收益，与 D36 删零消费者 `absent`、D42 收窄 `generated_path` 同族），也 MUST NOT
/// 为拿条目级行号去扫 fence 文本（那是第二套解析）。
///
/// [`ArchiDriftOccurrence::line`]: crate::drift::report::ArchiDriftOccurrence::line
fn declared_types(schema: Option<&Value>) -> Vec<String> {
    let Some(defs) = schema
        .and_then(|value| value.get("$defs"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    defs.iter()
        .filter(|(_, definition)| !is_pure_ref_alias(definition))
        .map(|(name, _)| name.clone())
        .collect()
}

/// 纯 `$ref` alias 判据（`{"$ref": …}` 且仅此一键）：透传条目与本地定义的唯一分界，
/// `apiReexportDrift` 与 `apiMissing` 的类型面共用本函数（MUST NOT 各写一份）。
fn is_pure_ref_alias(definition: &Value) -> bool {
    definition
        .as_object()
        .map(|object| object.contains_key("$ref") && object.len() == 1)
        .unwrap_or(false)
}

/// 完备性判据 `apiUndeclared`：代码 `external` 导出集 − 契约 `$defs` 键集（档位
/// `off`，调用点即按档位跳过；保留实现以便将来单值收紧）。
fn undeclared(
    schema: Option<&Value>,
    symbols: &[ApiSymbol],
    element: &ArchiElement,
) -> Option<ArchiApiFinding> {
    let declared: BTreeSet<&str> = schema
        .and_then(|value| value.get("$defs"))
        .and_then(Value::as_object)
        .map(|defs| defs.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let mut undeclared: Vec<&str> = symbols
        .iter()
        .filter(|symbol| symbol.visibility == extract::Visibility::External)
        .filter(|symbol| symbol.kind != ApiSymbolKind::Function)
        .filter(|symbol| !is_constructor(&symbol.name))
        .map(|symbol| symbol.name.as_str())
        .filter(|name| !declared.contains(name))
        .collect();
    undeclared.sort_unstable();
    if undeclared.is_empty() {
        return None;
    }
    Some(finding(
        ArchiApiFindingKind::ApiUndeclared,
        element,
        Some(undeclared.join(" / ")),
        format!(
            "元素 {} 有 external 导出未在契约声明：{}",
            element.fqn,
            undeclared.join(" / ")
        ),
        Vec::new(),
    ))
}

/// 一致性第一序 `apiSignatureMismatch`（归一后比对；参数名 / 空白 / 多行排列不参与）。
fn signature_mismatch(
    element: &ArchiElement,
    contract: Option<&Contract>,
    symbols: &[ApiSymbol],
) -> Option<ArchiApiFinding> {
    let contract = contract?;
    let mut occurrences: Vec<ArchiDriftOccurrence> = Vec::new();
    let mut details: Vec<String> = Vec::new();
    for op in &contract.ops {
        if is_constructor(&op.id) {
            continue;
        }
        let Some(symbol) = symbols
            .iter()
            .find(|symbol| symbol.name == op.id && symbol.kind == ApiSymbolKind::Function)
        else {
            continue;
        };
        let declared = extract::signature_of(&op.signature);
        let actual = extract::normalize_signature(symbol);
        if declared == actual {
            continue;
        }
        details.push(format!("{}：契约 {declared} / 代码 {actual}", op.id));
        occurrences.push(ArchiDriftOccurrence {
            file: contract.file.clone(),
            line: op.line,
            specifier: Some(op.id.clone()),
            type_only: false,
        });
    }
    if details.is_empty() {
        return None;
    }
    Some(finding(
        ArchiApiFindingKind::ApiSignatureMismatch,
        element,
        Some(contract.fqn.clone()),
        format!(
            "元素 {} 的契约签名与代码不一致（归一后仍需相等）：{}",
            element.fqn,
            details.join("；")
        ),
        occurrences,
    ))
}

/// 一致性第三序 `apiConstructorDeclared`：契约声明了构造器（契约自身矛盾——构造器由
/// 生成器产出）。
fn constructor_declared(
    element: &ArchiElement,
    contract: Option<&Contract>,
) -> Option<ArchiApiFinding> {
    let contract = contract?;
    let op = contract.ops.iter().find(|op| is_constructor(&op.id))?;
    Some(finding(
        ArchiApiFindingKind::ApiConstructorDeclared,
        element,
        Some(op.id.clone()),
        format!(
            "契约 {}:{} 声明了构造器 `{}`——构造器由生成器的 `$defs` 条目自动产出，契约 MUST NOT 声明它（否则必然误报实现缺失）",
            contract.file, op.line, op.id
        ),
        Vec::new(),
    ))
}

/// 一致性第二序 `apiReexportDrift`：已落盘 schema 的透传条目键集与契约 `api-reexport` 键集不等。
///
/// 解析期的 `contract::verify` 已在内存装配后判过一遍；此处只针对**落盘 schema 与 md
/// 不一致**的形态（schema 被手改或生成物过期），故不与解析期错误重复报。
fn reexport_drift(
    element: &ArchiElement,
    contract: &Contract,
    schema: &Value,
) -> Option<ArchiApiFinding> {
    let mut schema_reexports: BTreeSet<&str> = BTreeSet::new();
    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for (name, definition) in defs {
            if is_pure_ref_alias(definition) {
                schema_reexports.insert(name.as_str());
            }
        }
    }
    let fence: BTreeSet<&str> = contract.reexports.keys().map(String::as_str).collect();
    if schema_reexports == fence {
        return None;
    }
    let only_schema: Vec<&str> = schema_reexports.difference(&fence).copied().collect();
    let only_fence: Vec<&str> = fence.difference(&schema_reexports).copied().collect();
    Some(finding(
        ArchiApiFindingKind::ApiReexportDrift,
        element,
        Some(contract.fqn.clone()),
        format!(
            "契约 {} 的透传键集与 schema 的纯 `$ref` 条目不等（仅 schema：{}；仅契约：{}）",
            contract.file,
            if only_schema.is_empty() {
                "无".to_owned()
            } else {
                only_schema.join(" / ")
            },
            if only_fence.is_empty() {
                "无".to_owned()
            } else {
                only_fence.join(" / ")
            }
        ),
        Vec::new(),
    ))
}

/// pair 锚点 ⓒ `apiShadowing`：父 `$defs` 键集 ∩ 任一直接子元素 `$defs` 键集非空。
fn shadowing(input: &ApiCheckInput<'_>, findings: &mut Vec<ArchiApiFinding>) {
    for element in input.elements {
        if element.path.is_empty() {
            continue;
        }
        let Some(parent_defs) = def_names(input.schemas.get(&element.fqn)) else {
            continue;
        };
        for child in children_of(element, input.elements) {
            let Some(child_defs) = def_names(input.schemas.get(&child.fqn)) else {
                continue;
            };
            let shared: Vec<&str> = parent_defs
                .iter()
                .copied()
                .filter(|name| child_defs.contains(*name))
                .collect();
            if shared.is_empty() {
                continue;
            }
            findings.push(finding(
                ArchiApiFindingKind::ApiShadowing,
                element,
                Some(child.fqn.clone()),
                format!(
                    "父契约 {} 与子元素 {} 的类型名交集非空：{}——父子 `$defs` 名字必须互斥\
                     （glob 重复定义编译器只给 warning，MUST NOT 依赖它兜底）",
                    element.fqn,
                    child.fqn,
                    shared.join(" / ")
                ),
                Vec::new(),
            ));
        }
    }
}

/// pair 锚点 ⓑ `apiRefMissing`：跨模块 `$ref` 目标的三种问题（文件不存在 / 未 declared / 非直接
/// 子元素）。
fn ref_missing(input: &ApiCheckInput<'_>, findings: &mut Vec<ArchiApiFinding>) {
    for element in input.elements {
        if element.path.is_empty() {
            continue;
        }
        let Some(schema) = input.schemas.get(&element.fqn) else {
            continue;
        };
        for (name, reference) in cross_file_refs(schema) {
            let Some(target) = reference_target(&reference) else {
                continue;
            };
            let target_element = input
                .elements
                .iter()
                .find(|candidate| candidate.fqn == target);
            let adjacent = children_of(element, input.elements)
                .iter()
                .any(|child| child.fqn == target);
            let message = match target_element {
                None => format!(
                    "元素 {} 的 `$ref`（{name}）指向 {target}，但模型里没有该元素",
                    element.fqn
                ),
                Some(candidate) if candidate.api == ApiStatus::None => format!(
                    "元素 {} 的 `$ref`（{name}）指向 {target}，但目标元素声明 `api = none`\
                     （无契约可引用）",
                    element.fqn
                ),
                Some(_) if !adjacent => format!(
                    "元素 {} 的 `$ref`（{name}）指向 {target}，但目标不是**直接子元素**——\
                     跨层引用架空了层级语义",
                    element.fqn
                ),
                Some(candidate)
                    if !paths::contract_path(input.root, &candidate.fqn)
                        .is_some_and(|path| path.is_file()) =>
                {
                    format!(
                        "元素 {} 的 `$ref`（{name}）指向 {target}，但目标元素的契约文件不存在",
                        element.fqn
                    )
                }
                Some(_) => continue,
            };
            findings.push(finding(
                ArchiApiFindingKind::ApiRefMissing,
                element,
                Some(target.to_owned()),
                message,
                Vec::new(),
            ));
        }
    }
}

/// pair 锚点 ⓐ `apiNestedCrossCrate`：父子元素的 crate 根不同（嵌套元素必须同属一个 crate）。
fn nested_cross_crate(input: &ApiCheckInput<'_>, findings: &mut Vec<ArchiApiFinding>) {
    for element in input.elements {
        if element.path.is_empty() {
            continue;
        }
        let Some(parent_crate) = crate_root(element) else {
            continue;
        };
        for child in children_of(element, input.elements) {
            let Some(child_crate) = crate_root(child) else {
                continue;
            };
            if child_crate == parent_crate {
                continue;
            }
            findings.push(finding(
                ArchiApiFindingKind::ApiNestedCrossCrate,
                element,
                Some(child.fqn.clone()),
                format!(
                    "父元素 {} 与子元素 {} 的所属包根不同（{parent_crate} vs {child_crate}）——\
                     嵌套元素父子必须同属一个包，跨包不可表达",
                    element.fqn, child.fqn
                ),
                Vec::new(),
            ));
        }
    }
}

/// 元素的包根（`crates/{core,infra,tools}/<dir>` 前三段；非该形态 → `None`）。
fn crate_root(element: &ArchiElement) -> Option<String> {
    let normalized = paths::normalize_root(element.path.first()?);
    let segments: Vec<&str> = normalized.split('/').collect();
    if segments.len() < 3 || segments[0] != "crates" {
        return None;
    }
    Some(segments[..3].join("/"))
}

/// 直接子元素（FQN 前缀相邻：`<父>.<末段>` 且无更深的点）。
fn children_of<'a>(parent: &ArchiElement, elements: &'a [ArchiElement]) -> Vec<&'a ArchiElement> {
    let prefix = format!("{}.", parent.fqn);
    elements
        .iter()
        .filter(|candidate| {
            candidate
                .fqn
                .strip_prefix(&prefix)
                .is_some_and(|tail| !tail.is_empty() && !tail.contains('.'))
        })
        .collect()
}

/// `$defs` 键集（缺 schema / 缺 `$defs` → `None`）。
fn def_names(schema: Option<&Value>) -> Option<BTreeSet<&str>> {
    schema
        .and_then(|value| value.get("$defs"))
        .and_then(Value::as_object)
        .map(|defs| defs.keys().map(String::as_str).collect())
}

/// schema 内的跨文件 `$ref`（引用含 `.schema.json#` 形态的条目）。
fn cross_file_refs(schema: &Value) -> Vec<(String, String)> {
    let mut refs: Vec<(String, String)> = Vec::new();
    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for (name, definition) in defs {
            let Some(reference) = definition.get("$ref").and_then(Value::as_str) else {
                continue;
            };
            if reference.contains(".schema.json#") {
                refs.push((name.clone(), reference.to_owned()));
            }
        }
    }
    refs
}

/// 跨文件 `$ref` → 目标元素 FQN（[`super::reexport_target`] 的单点封装；本文件零
/// 第二份切分逻辑）。
fn reference_target(reference: &str) -> Option<String> {
    super::reexport_target(reference)
}

/// 条目构造（`category` 只经 [`category_of`] 打标——本文件零分类字面量）。
fn finding(
    kind: ArchiApiFindingKind,
    element: &ArchiElement,
    target: Option<String>,
    message: String,
    occurrences: Vec<ArchiDriftOccurrence>,
) -> ArchiApiFinding {
    ArchiApiFinding {
        kind,
        category: category_of(kind).to_owned(),
        element: Some(element.fqn.clone()),
        target,
        message,
        occurrences,
    }
}
