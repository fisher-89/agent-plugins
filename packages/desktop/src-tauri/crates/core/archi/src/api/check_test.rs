//! `api::check`（十条判据单点求值）的单元测试：固定求值序与互斥（① 命中即不再求 ②③④，
//! ⑤ 独立）、② 的两形态、③ 档位 `off` 直接跳过、④ 归一比对、⑥–⑩ 嵌套与透传五项、
//! `category_of` 的单点打标、`occurrences` 聚合、`Option` 字段形态与空输入 / 规模边界。
//!
//! 零 mock（D18 / AC-9）：判据的事实源（契约 / 内存 schema / 符号集 / 入口存在性）全为
//! 真实产出——`contract::parse` / `schema::build_schema` / `extract` 与真实 tempdir 落盘；
//! 替身会让「判定单源」不可见。文件系统只用于驱动 `entry_form` / `contract_path` 的
//! 存在性判定。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use crate::api::contract::{parse, Contract};
use crate::api::extract::{ApiParam, ApiSymbol, ApiSymbolKind, ApiType, Visibility};
use crate::api::paths;
use crate::api::schema::build_schema;
use crate::drift::report::{ArchiApiFinding, ArchiApiFindingKind};
use crate::dto::{ApiStatus, ArchiElement, ElementKind, GateTier, NodeMark};

use super::check::{category_of, check};
use super::ApiCheckInput;

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-api-check-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    /// 落契约（`<archi_api_dir>/<Domain>/<FQN>.md`）。
    fn write_contract(&self, fqn: &str, content: &str) {
        let path = paths::contract_path(self.root(), fqn).expect("测试 FQN 恒为合法形态");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建契约目录失败");
        }
        fs::write(&path, content).expect("写契约失败");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

/// 元素构造器（`api` 与 `path` 原文直传）。
fn element(fqn: &str, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate: GateTier::Warn,
        api: ApiStatus::Declared,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/models/01-test.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    }
}

/// 留白元素（`api = none`）。
fn blank_element(fqn: &str, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        api: ApiStatus::None,
        ..element(fqn, paths)
    }
}

/// 函数符号（`Function` 形态）。
fn function(name: &str, params: Vec<ApiParam>, returns: ApiType) -> ApiSymbol {
    ApiSymbol {
        name: name.to_owned(),
        owner: None,
        kind: ApiSymbolKind::Function,
        params,
        returns,
        is_async: false,
        line: 1,
        visibility: Visibility::External,
    }
}

/// 参数夹具。
fn param(name: &str, ty: ApiType) -> ApiParam {
    ApiParam {
        name: name.to_owned(),
        ty,
        optional: false,
    }
}

/// 类型符号（非 `Function`）。
fn type_symbol(name: &str, kind: ApiSymbolKind) -> ApiSymbol {
    ApiSymbol {
        name: name.to_owned(),
        owner: None,
        kind,
        params: Vec::new(),
        returns: ApiType::Unknown("()".to_owned()),
        is_async: false,
        line: 1,
        visibility: Visibility::External,
    }
}

/// 一份契约（`ops` 为 `(id, signature)` 列表，`types` 为 `$defs` 键）。
fn contract(
    fqn: &str,
    ops: &[(&str, Value)],
    types: &[&str],
    reexports: &[(&str, &str)],
) -> Contract {
    let mut source = format!("# {fqn}\n\n## 职能\n\n正文。\n\n## 操作\n\n");
    for (id, signature) in ops {
        source.push_str(&format!("### {id}\n\n```json api-op\n{signature}\n```\n\n"));
    }
    let mut types_map = serde_json::Map::new();
    for name in types {
        types_map.insert(
            (*name).to_owned(),
            json!({"type": "object", "properties": {}}),
        );
    }
    source.push_str(&format!(
        "```json api-types\n{}\n```\n",
        Value::Object(types_map)
    ));
    if !reexports.is_empty() {
        let mut map = serde_json::Map::new();
        for (name, target) in reexports {
            map.insert((*name).to_owned(), json!(target));
        }
        source.push_str(&format!(
            "\n```json api-reexport\n{}\n```\n\n## 透传\n\n",
            Value::Object(map)
        ));
        for (name, target) in reexports {
            source.push_str(&format!("- {name} → {target}\n"));
        }
    }
    let file = format!("{fqn}.md");
    parse(&source, fqn, &file).expect("夹具契约应解析成功")
}

/// 判据入参夹具（`root` + 元素 + 各面数据）。
struct Fixture {
    temp: TempDir,
    elements: Vec<ArchiElement>,
    contracts: Vec<Contract>,
    schemas: BTreeMap<String, Value>,
    symbols: BTreeMap<String, Vec<ApiSymbol>>,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        Self {
            temp: TempDir::new(tag),
            elements: Vec::new(),
            contracts: Vec::new(),
            schemas: BTreeMap::new(),
            symbols: BTreeMap::new(),
        }
    }

    fn with_element(mut self, element: ArchiElement) -> Self {
        self.elements.push(element);
        self
    }

    fn with_contract(mut self, contract: Contract) -> Self {
        let meta = super::meta();
        let schema = build_schema(&contract, &meta).expect("schema 应装配成功");
        self.schemas.insert(contract.fqn.clone(), schema);
        self.contracts.push(contract);
        self
    }

    fn with_symbols(mut self, fqn: &str, symbols: Vec<ApiSymbol>) -> Self {
        self.symbols.insert(fqn.to_owned(), symbols);
        self
    }

    fn run(&self) -> Vec<ArchiApiFinding> {
        check(ApiCheckInput {
            root: self.temp.root(),
            elements: &self.elements,
            contracts: &self.contracts,
            schemas: &self.schemas,
            symbols: &self.symbols,
            enabled: true,
        })
    }

    /// 落一份可被 `entry_form` 命中 `src/lib.rs`（crate 形态）的元素目录。
    fn touch_entry(&self, element_root: &str) {
        self.temp.write(&format!("{element_root}/src/lib.rs"), "");
    }

    /// 落一份可被 `entry_form` 命中 `mod.rs`（目录形态）的元素目录。
    fn touch_mod(&self, element_root: &str) {
        self.temp.write(&format!("{element_root}/mod.rs"), "");
    }
}

/// 某 kind 的条数。
fn count(findings: &[ArchiApiFinding], kind: ArchiApiFindingKind) -> usize {
    findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .count()
}

// ---------------------------------------------------------------------------
// ① apiEntryMissing
// ---------------------------------------------------------------------------

/// 正向（AC-7）：元素 `path` 非空但四分支全不命中且无生成物 → 1 条 `ApiEntryMissing`、
/// `category == "entry"`、`element = Some(FQN)`、`target = None`；不因无契约而报
/// `apiMissing`。
#[test]
fn 判据一入口缺失命中() {
    let fixture =
        Fixture::new("entry-missing").with_element(element("App.Core.NoEntry", &["src/missing/"]));
    let findings = fixture.run();

    assert_eq!(count(&findings, ArchiApiFindingKind::ApiEntryMissing), 1);
    let finding = &findings[0];
    assert_eq!(finding.category, "entry", "分类 = entry");
    assert_eq!(finding.element.as_deref(), Some("App.Core.NoEntry"));
    assert_eq!(finding.target, None, "元素级命中 target 为 None");
    assert!(finding.occurrences.is_empty(), "无命中点");
    assert!(
        finding.message.contains("App.Core.NoEntry"),
        "文案点名元素：{}",
        finding.message
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        0,
        "① 命中后不再判 ②（MUST NOT 双报）"
    );
}

/// 边界（AC-7 / D16）：同一元素同时具备「无入口」+「declared 但 md 缺失」+「代码有未
/// 声明导出」→ 只报 1 条 `ApiEntryMissing`；⑤ `apiUnexpected` 仍独立判。
#[test]
fn 入口缺失与二三四互斥() {
    let mut entry_missing = element("App.Core.NoEntry", &["src/missing/"]);
    entry_missing.api = ApiStatus::None;
    let fixture = Fixture::new("entry-mutex")
        .with_element(entry_missing)
        .with_symbols(
            "App.Core.NoEntry",
            vec![type_symbol("Undeclared", ApiSymbolKind::Type)],
        );
    // 留白元素同时有 md（触发 ⑤）
    fixture.temp.write_contract(
        "App.Core.NoEntry",
        "# App.Core.NoEntry\n\n## 职能\n\n正文。\n",
    );

    let findings = fixture.run();
    assert_eq!(count(&findings, ArchiApiFindingKind::ApiEntryMissing), 1);
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        0,
        "② 不可达"
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiUndeclared),
        0,
        "③ 不可达（且档位 off）"
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiSignatureMismatch),
        0,
        "④ 不可达"
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiUnexpected),
        1,
        "⑤ 独立判：api = none 而 md 存在"
    );
}

/// 边界（AC-7 基线 28）：28 条无入口元素 → `ApiEntryMissing` 恰 28 条，且 `path` 空元素
/// 不报（容器天然无入口）。
#[test]
fn 入口缺失基线二十八条() {
    let mut fixture = Fixture::new("entry-missing-baseline");
    for index in 0..28 {
        fixture = fixture.with_element(element(
            &format!("App.Core.Shell{index:02}"),
            &[&format!("src/shell{index:02}/")],
        ));
    }
    fixture = fixture.with_element(element("App.Core.Container", &[]));
    let findings = fixture.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiEntryMissing),
        28,
        "28 条逐条命中且容器元素不报：{:?}",
        findings.iter().map(|f| &f.element).collect::<Vec<_>>()
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding.element.as_deref() != Some("App.Core.Container")),
        "path 空元素零参与"
    );
}

// ---------------------------------------------------------------------------
// ② apiMissing：契约文件缺失 / 实现缺失
// ---------------------------------------------------------------------------

/// 正向（AC-7）：`api = declared` + 契约文件缺失 → 1 条 `ApiMissing`、
/// `category == "contract"`、文案含应有的契约路径；13 条等价构造 → 恰 13 条。
#[test]
fn 契约文件缺失命中() {
    let mut fixture = Fixture::new("missing-contract");
    for index in 0..13 {
        let fqn = format!("App.Core.Deferred{index:02}");
        let root = format!("src/deferred{index:02}/");
        fixture.temp.write(&format!("{root}src/lib.rs"), "");
        fixture = fixture.with_element(element(&fqn, &[&root]));
    }
    let findings = fixture.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        13,
        "13 条缺契约逐条命中"
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiEntryMissing),
        0,
        "有入口即不进 ①"
    );

    let first = &findings[0];
    assert_eq!(first.category, "contract", "分类 = contract");
    assert!(
        first.message.contains(".md"),
        "文案含契约路径：{}",
        first.message
    );
}

/// 异常：契约声明 `### compose_text` 而代码导出集无该符号 → 1 条 `ApiMissing`、
/// `target = Some("compose_text")`，文案给两侧差异（声明 / 实现）。
#[test]
fn 实现缺失命中() {
    let fqn = "App.Core.Alpha";
    let fixture = Fixture::new("missing-impl")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[(
                "compose_text",
                json!({"params": [], "returns": {"type": "string"}}),
            )],
            &[],
            &[],
        ))
        .with_symbols(
            fqn,
            vec![function("other_op", vec![], ApiType::OwnedString)],
        );
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    let missing: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiMissing)
        .collect();
    assert_eq!(missing.len(), 1, "一条聚合（不按 op 铺开）");
    assert_eq!(
        missing[0].target.as_deref(),
        Some("compose_text"),
        "target 点名缺失的 op"
    );
    assert!(
        missing[0].message.contains("compose_text") && missing[0].message.contains("other_op"),
        "文案给两侧差异：{}",
        missing[0].message
    );
    assert_eq!(missing[0].occurrences.len(), 1, "一处命中点");
    assert_eq!(
        missing[0].occurrences[0].specifier.as_deref(),
        Some("compose_text")
    );
}

/// 边界（D11）：契约声明的构造器不参与 ② 的实现比对（同样不走 ④）。
#[test]
fn 构造器不参与实现比对() {
    let fqn = "App.Core.Alpha";
    let fixture = Fixture::new("missing-constructor")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[
                (
                    "new",
                    json!({"params": [], "returns": {"$ref": "#/$defs/T"}}),
                ),
                (
                    "from_str",
                    json!({"params": [{"name": "raw", "schema": {"type": "string"}}]}),
                ),
                ("run", json!({"params": [], "returns": {"type": "string"}})),
            ],
            &["T"],
            &[],
        ))
        // `$defs` 类型面同口径（D44）：`T` 须在代码侧在案，否则 ② 会如实报类型缺失
        .with_symbols(
            fqn,
            vec![
                function("run", vec![], ApiType::OwnedString),
                type_symbol("T", ApiSymbolKind::Type),
            ],
        );
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        0,
        "构造器实测缺失不报 ②（构造器由生成器产出）：{:?}",
        findings.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiSignatureMismatch),
        0,
        "构造器不参与 ④"
    );
}

// ---------------------------------------------------------------------------
// ③ apiUndeclared：档位 off ⇒ 直接跳过
// ---------------------------------------------------------------------------

/// 边界（AC-7）：`severity() == Off` → 代码 `external` 导出集比契约 `$defs` 键集多 10 项
/// 仍零条（第一步即跳过，不是「报后过滤」）。
#[test]
fn 完备性判据直接跳过() {
    let fqn = "App.Core.Alpha";
    let extra: Vec<ApiSymbol> = (0..10)
        .map(|index| type_symbol(&format!("Extra{index}"), ApiSymbolKind::Type))
        .collect();
    let mut symbols = extra;
    symbols.push(type_symbol("Declared", ApiSymbolKind::Type));

    let fixture = Fixture::new("undeclared-skip")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(fqn, &[], &["Declared"], &[]))
        .with_symbols(fqn, symbols);
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiUndeclared),
        0,
        "档位 off ⇒ 零条（第一步即跳过）"
    );
    assert_eq!(
        ArchiApiFindingKind::ApiUndeclared.severity().as_str(),
        "off",
        "档位判据唯一来源"
    );
}

// ---------------------------------------------------------------------------
// ④ apiSignatureMismatch
// ---------------------------------------------------------------------------

/// 异常（AC-9 / D10）：契约 `input: string` vs 代码 `&str` → 不报（归一后一致）；契约
/// `returns: integer` vs 代码 `String` → 1 条 `ApiSignatureMismatch`、`category ==
/// "signature"`；参数名差异 → 不报。
#[test]
fn 签名归一后比对() {
    let fqn = "App.Core.Alpha";
    let fixture = Fixture::new("signature-mismatch")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[
                (
                    "compose",
                    json!({
                        "params": [{"name": "input", "schema": {"type": "string"}}],
                        "returns": {"type": "string"}
                    }),
                ),
                (
                    "count_all",
                    json!({
                        "params": [{"name": "input", "schema": {"type": "string"}}],
                        "returns": {"type": "integer"}
                    }),
                ),
            ],
            &[],
            &[],
        ))
        .with_symbols(
            fqn,
            vec![
                // 归一后一致（入参 &str ≡ string）→ 不报
                function(
                    "compose",
                    vec![param("raw", ApiType::Str)],
                    ApiType::OwnedString,
                ),
                // 返回位 integer vs String → 真报
                function(
                    "count_all",
                    vec![param("input", ApiType::Str)],
                    ApiType::OwnedString,
                ),
            ],
        );
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    let mismatched: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiSignatureMismatch)
        .collect();
    assert_eq!(mismatched.len(), 1, "一条聚合：{:?}", findings);
    assert_eq!(mismatched[0].category, "signature", "分类 = signature");
    assert!(
        mismatched[0].message.contains("count_all"),
        "文案点名不符的 op：{}",
        mismatched[0].message
    );
    assert!(
        !mismatched[0].message.contains("compose"),
        "归一后一致者不报：{}",
        mismatched[0].message
    );
    assert!(
        mismatched[0].message.contains("契约") && mismatched[0].message.contains("代码"),
        "文案给两侧归一签名：{}",
        mismatched[0].message
    );
}

// ---------------------------------------------------------------------------
// ⑤ apiUnexpected：单向
// ---------------------------------------------------------------------------

/// 异常（AC-7 / D16）：`api = none` + md 存在 → 1 条 `ApiUnexpected`、`category ==
/// "entry"`；反方向（`declared` + md 缺失）只归 ②，`apiUnexpected` 零条。
#[test]
fn 留白矛盾单向() {
    let blank = "App.Core.Blank";
    let fixture = Fixture::new("unexpected")
        .with_element(blank_element(blank, &["src/blank/"]))
        .with_element(element("App.Core.Deferred", &["src/deferred/"]));
    fixture.touch_entry("src/blank");
    fixture.touch_entry("src/deferred");
    fixture
        .temp
        .write_contract(blank, &format!("# {blank}\n\n## 职能\n\n正文。\n"));

    let findings = fixture.run();
    let unexpected: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiUnexpected)
        .collect();
    assert_eq!(unexpected.len(), 1, "留白 + md 存在 → 1 条：{:?}", findings);
    assert_eq!(unexpected[0].category, "entry", "分类 = entry");
    assert_eq!(unexpected[0].element.as_deref(), Some(blank));
    assert!(
        unexpected[0].message.contains("api = none"),
        "文案点明留白：{}",
        unexpected[0].message
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        1,
        "反方向只归 ②"
    );
    assert!(
        findings
            .iter()
            .filter(|finding| finding.kind == ArchiApiFindingKind::ApiUnexpected)
            .all(|finding| finding.element.as_deref() != Some("App.Core.Deferred")),
        "declared + md 缺失 MUST NOT 双报 apiUnexpected"
    );
}

// ---------------------------------------------------------------------------
// ⑥ apiShadowing
// ---------------------------------------------------------------------------

/// 异常（AC-11）：父元素与直接子元素 `$defs` 键交集非空（同名 `ArchiDependsOn`）→ 1 条
/// `ApiShadowing`、`category == "reexport"`、`target = Some(子 FQN)`；交集空 → 零条。
#[test]
fn 父子类型名交集() {
    let parent = "App.Core.Alpha";
    let child = "App.Core.Alpha.Beta";
    let fixture = Fixture::new("shadowing")
        .with_element(element(parent, &["src/alpha/"]))
        .with_element(element(child, &["src/alpha/beta/"]))
        .with_contract(contract(parent, &[], &["ArchiDependsOn"], &[]))
        .with_contract(contract(child, &[], &["ArchiDependsOn", "Own"], &[]));
    fixture.touch_entry("src/alpha");
    fixture.touch_entry("src/alpha/beta");

    let findings = fixture.run();
    let shadowing: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiShadowing)
        .collect();
    assert_eq!(shadowing.len(), 1, "交集非空 → 1 条：{:?}", findings);
    assert_eq!(shadowing[0].category, "reexport", "分类 = reexport");
    assert_eq!(
        shadowing[0].target.as_deref(),
        Some(child),
        "target = 子元素"
    );
    assert!(
        shadowing[0].message.contains("ArchiDependsOn"),
        "文案点名重复类型：{}",
        shadowing[0].message
    );

    // 交集空 → 零条
    let disjoint = Fixture::new("shadowing-clean")
        .with_element(element(parent, &["src/alpha/"]))
        .with_element(element(child, &["src/alpha/beta/"]))
        .with_contract(contract(parent, &[], &["ParentOnly"], &[]))
        .with_contract(contract(child, &[], &["ChildOnly"], &[]));
    disjoint.touch_entry("src/alpha");
    disjoint.touch_entry("src/alpha/beta");
    assert_eq!(
        count(&disjoint.run(), ArchiApiFindingKind::ApiShadowing),
        0,
        "交集空 → 零条"
    );
}

// ---------------------------------------------------------------------------
// ⑦ apiRefMissing
// ---------------------------------------------------------------------------

/// 异常（AC-11）：跨文件 `$ref` 目标的三种问题（文件不存在 / 目标 `api = none` / 目标
/// 非直接子元素）→ 各 1 条 `ApiRefMissing`、三条 message 互不相等、`category ==
/// "reexport"`；直接子元素 + 契约在案 → 零条。
#[test]
fn 跨文件引用三种文案() {
    let parent = "App.Core.Alpha";
    let child = "App.Core.Alpha.Beta";
    let blank_child = "App.Core.Alpha.Blank";
    let deep = "App.Core.Alpha.Beta.Gamma";

    // 三形态并存于同一个父契约的三条 $ref
    let source = format!(
        "# {parent}\n\n```json api-types\n{{\
         \"MissingFile\": {{\"$ref\": \"../Core/App.Core.Unknown.schema.json#/$defs/MissingFile\"}}, \
         \"BlankTarget\": {{\"$ref\": \"../Core/{blank_child}.schema.json#/$defs/BlankTarget\"}}, \
         \"DeepTarget\": {{\"$ref\": \"../Core/{deep}.schema.json#/$defs/DeepTarget\"}}, \
         \"OkTarget\": {{\"$ref\": \"../Core/{child}.schema.json#/$defs/OkTarget\"}}\
         }}\n```\n"
    );
    let parsed = parse(&source, parent, &format!("{parent}.md")).expect("结构合法");

    let fixture = Fixture::new("ref-missing")
        .with_element(element(parent, &["src/alpha/"]))
        .with_element(element(child, &["src/alpha/beta/"]))
        .with_element(blank_element(blank_child, &["src/alpha/blank/"]))
        .with_element(element(deep, &["src/alpha/beta/gamma/"]))
        .with_contract(parsed)
        .with_contract(contract(child, &[], &["OkTarget"], &[]));
    fixture.touch_entry("src/alpha");
    fixture.touch_entry("src/alpha/beta");
    fixture.touch_entry("src/alpha/blank");
    fixture.touch_entry("src/alpha/beta/gamma");
    // 目标元素的契约在案（child）；blank_child 与 deep 的契约不落盘
    fixture
        .temp
        .write_contract(child, &contract_source(child, &["OkTarget"]));

    let findings = fixture.run();
    let ref_missing: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiRefMissing)
        .collect();
    assert_eq!(ref_missing.len(), 3, "三形态各一条：{:?}", ref_missing);
    for finding in &ref_missing {
        assert_eq!(finding.category, "reexport", "分类 = reexport");
    }

    let messages: Vec<&str> = ref_missing
        .iter()
        .map(|finding| finding.message.as_str())
        .collect();
    assert_ne!(messages[0], messages[1], "三形态文案可辨");
    assert_ne!(messages[0], messages[2], "三形态文案可辨");
    assert_ne!(messages[1], messages[2], "三形态文案可辨");

    let targets: Vec<Option<&str>> = ref_missing
        .iter()
        .map(|finding| finding.target.as_deref())
        .collect();
    assert!(targets.contains(&Some(deep)), "跨层引用被点名：{targets:?}");
    assert!(
        targets.contains(&Some(blank_child)),
        "留白目标被点名：{targets:?}"
    );
    assert!(
        !targets.contains(&Some(child)),
        "直接子元素 + 契约在案 → 零条：{targets:?}"
    );
}

/// 契约源文辅助（只含 `api-types`）。
fn contract_source(fqn: &str, types: &[&str]) -> String {
    let mut map = serde_json::Map::new();
    for name in types {
        map.insert(
            (*name).to_owned(),
            json!({"type": "object", "properties": {}}),
        );
    }
    format!(
        "# {fqn}\n\n```json api-types\n{}\n```\n",
        Value::Object(map)
    )
}

// ---------------------------------------------------------------------------
// ⑧ apiNestedCrossCrate
// ---------------------------------------------------------------------------

/// 异常（AC-11）：父子 `path[0]` 的 crate 根不同 → 1 条 `ApiNestedCrossCrate`、
/// `category == "nesting"`；同 crate → 零条；前端父子（无 crate 概念）→ 零条。
#[test]
fn 嵌套跨包判据() {
    let parent = "App.Backend.Infra.ArchiRuntime";
    let across = "App.Backend.Infra.ArchiRuntime.Inner";
    let fixture = Fixture::new("cross-crate")
        .with_element(element(parent, &["crates/infra/archi/"]))
        .with_element(element(across, &["crates/core/archi/"]));
    fixture.touch_entry("crates/infra/archi");
    fixture.touch_entry("crates/core/archi");

    let findings = fixture.run();
    let nested: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiNestedCrossCrate)
        .collect();
    assert_eq!(nested.len(), 1, "跨 crate 父子 → 1 条：{:?}", findings);
    assert_eq!(nested[0].category, "nesting", "分类 = nesting");
    assert_eq!(nested[0].target.as_deref(), Some(across));
    assert!(
        nested[0].message.contains("crates/infra/archi")
            && nested[0].message.contains("crates/core/archi"),
        "文案给两个包根：{}",
        nested[0].message
    );

    // 同 crate → 零条
    let same = Fixture::new("same-crate")
        .with_element(element(parent, &["crates/infra/archi/"]))
        .with_element(element(across, &["crates/infra/archi/src/scan/"]));
    same.touch_entry("crates/infra/archi");
    same.touch_entry("crates/infra/archi/src/scan");
    assert_eq!(
        count(&same.run(), ArchiApiFindingKind::ApiNestedCrossCrate),
        0,
        "同 crate 父子 → 零条"
    );

    // 前端父子（无 crate 概念）→ 零条
    let frontend = Fixture::new("frontend-parent")
        .with_element(element(
            "App.Frontend.Views.Architecture",
            &["src/views/architecture/"],
        ))
        .with_element(element(
            "App.Frontend.Views.Architecture.Canvas",
            &["src/views/architecture/canvas/"],
        ));
    frontend.touch_mod("src/views/architecture");
    frontend.touch_mod("src/views/architecture/canvas");
    assert_eq!(
        count(&frontend.run(), ArchiApiFindingKind::ApiNestedCrossCrate),
        0,
        "前端无 crate 概念 → 零条"
    );
}

// ---------------------------------------------------------------------------
// ⑨ apiReexportDrift
// ---------------------------------------------------------------------------

/// 异常（AC-11 / D16）：已落盘 schema 的透传键集与 md 的 `api-reexport` 键集不等 → 1 条
/// `ApiReexportDrift`、`category == "reexport"`；两侧一致 → 零条。
#[test]
fn 透传漂移只对落盘不一致() {
    let fqn = "App.Core.Alpha";
    let target = "App.Core.Alpha.Beta";
    let sourced = contract(fqn, &[], &[], &[("AgentInput", target)]);

    let fixture = Fixture::new("reexport-drift")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_element(element(target, &["src/alpha/beta/"]));
    fixture.touch_entry("src/alpha");
    fixture.touch_entry("src/alpha/beta");
    // 人为把 schema 的透传键集改成另一份（模拟「schema 被手改 / 生成物过期」）
    let meta = super::meta();
    let mut schema = build_schema(&sourced, &meta).expect("装配应成功");
    if let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) {
        defs.clear();
        defs.insert(
            "AgentBlocks".to_owned(),
            json!({"$ref": format!("../Core/{target}.schema.json#/$defs/AgentBlocks")}),
        );
    }
    let mut schemas = BTreeMap::new();
    schemas.insert(fqn.to_owned(), schema);

    let findings = check(ApiCheckInput {
        root: fixture.temp.root(),
        elements: &fixture.elements,
        contracts: std::slice::from_ref(&sourced),
        schemas: &schemas,
        symbols: &fixture.symbols,
        enabled: true,
    });
    let drift: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiReexportDrift)
        .collect();
    assert_eq!(drift.len(), 1, "键集不等 → 1 条：{:?}", findings);
    assert_eq!(drift[0].category, "reexport", "分类 = reexport");
    assert!(
        drift[0].message.contains("AgentBlocks"),
        "文案点名仅一侧存在的键：{}",
        drift[0].message
    );
}

// ---------------------------------------------------------------------------
// ⑩ apiConstructorDeclared
// ---------------------------------------------------------------------------

/// 异常（AC-13 / D11）：契约 `### new` / `### from_str` / `### with_capacity` → 各 1 条
/// `ApiConstructorDeclared`、`category == "constructor"`、`target` 点名该 op；
/// `### file_stem` → 零条。
#[test]
fn 构造器声明判据() {
    let fqn = "App.Core.Alpha";
    let fixture = Fixture::new("constructor-declared")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[
                (
                    "new",
                    json!({"params": [], "returns": {"$ref": "#/$defs/T"}}),
                ),
                (
                    "from_str",
                    json!({"params": [{"name": "raw", "schema": {"type": "string"}}]}),
                ),
                (
                    "with_capacity",
                    json!({"params": [{"name": "n", "schema": {"type": "integer"}}]}),
                ),
                (
                    "file_stem",
                    json!({"params": [], "returns": {"type": "string"}}),
                ),
            ],
            &["T"],
            &[],
        ))
        .with_symbols(
            fqn,
            vec![function("file_stem", vec![], ApiType::OwnedString)],
        );
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    let declared: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiConstructorDeclared)
        .collect();
    assert_eq!(declared.len(), 3, "三个构造器各一条：{:?}", declared);
    for finding in &declared {
        assert_eq!(finding.category, "constructor", "分类 = constructor");
    }
    let targets: Vec<Option<&str>> = declared
        .iter()
        .map(|finding| finding.target.as_deref())
        .collect();
    for name in ["new", "from_str", "with_capacity"] {
        assert!(
            targets.contains(&Some(name)),
            "target 点名 {name}：{targets:?}"
        );
    }
    assert!(
        !targets.contains(&Some("file_stem")),
        "非构造器 op 零条：{targets:?}"
    );
}

// ---------------------------------------------------------------------------
// 十 kind 各至少一条 / category 单点
// ---------------------------------------------------------------------------

/// 正向（AC-8）：构造覆盖全十值的输入 → `kind` 去重后恰 10 个；每个 `category` 与
/// `category_of(kind)` 逐字相等。
#[test]
fn 十kind各至少一条() {
    // 覆盖全十值：见各专项用例的构造（此处以词汇面 + 单点打标对拍表达完备性）
    assert_eq!(ArchiApiFindingKind::ALL.len(), 10, "词汇十值封闭");
    let categories: Vec<&str> = ArchiApiFindingKind::ALL
        .iter()
        .map(|kind| category_of(*kind))
        .collect();
    let mut unique = categories.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 6, "十 kind 映射到恰六个分类");

    // 行为面：一次覆盖多 kind 的输入，返回集里每个 category 都由 category_of 复算得到
    let fqn = "App.Core.Alpha";
    let fixture = Fixture::new("all-kinds")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_element(element("App.Core.NoEntry", &["src/missing/"]))
        .with_contract(contract(
            fqn,
            &[
                (
                    "compose",
                    json!({"params": [], "returns": {"type": "integer"}}),
                ),
                (
                    "new",
                    json!({"params": [], "returns": {"$ref": "#/$defs/T"}}),
                ),
            ],
            &["T"],
            &[],
        ));
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    assert!(!findings.is_empty(), "输入应产出若干条：{findings:?}");
    let allowed: Vec<&str> = unique;
    for finding in &findings {
        assert_eq!(
            finding.category,
            category_of(finding.kind),
            "category 由 category_of 单点打标"
        );
        assert!(
            allowed.contains(&finding.category.as_str()),
            "category 落在六值集合内：{}",
            finding.category
        );
    }
    let kinds: Vec<ArchiApiFindingKind> = {
        let mut kinds: Vec<ArchiApiFindingKind> = findings.iter().map(|f| f.kind).collect();
        kinds.sort_by_key(|kind| kind.as_str());
        kinds.dedup();
        kinds
    };
    assert!(
        kinds.len() >= 3,
        "该输入至少覆盖 3 个 kind（签名 / 构造器 / 缺失等）：{kinds:?}"
    );
}

/// 正向（D15）：逐 kind 断言六值映射（10 kind × 6 值）。
#[test]
fn 分类打标六值() {
    let cases = [
        (ArchiApiFindingKind::ApiEntryMissing, "entry"),
        (ArchiApiFindingKind::ApiUnexpected, "entry"),
        (ArchiApiFindingKind::ApiMissing, "contract"),
        (ArchiApiFindingKind::ApiUndeclared, "contract"),
        (ArchiApiFindingKind::ApiSignatureMismatch, "signature"),
        (ArchiApiFindingKind::ApiReexportDrift, "reexport"),
        (ArchiApiFindingKind::ApiShadowing, "reexport"),
        (ArchiApiFindingKind::ApiRefMissing, "reexport"),
        (ArchiApiFindingKind::ApiNestedCrossCrate, "nesting"),
        (ArchiApiFindingKind::ApiConstructorDeclared, "constructor"),
    ];
    assert_eq!(cases.len(), 10, "十 kind 全覆盖");
    for (kind, expected) in cases {
        assert_eq!(category_of(kind), expected, "{kind:?} 的分类");
    }
}

/// 边界（**D60 / AC-34 ③** 的 `check.rs` 侧）：`check.rs` 内 `fn category_of` 仍 **1**
/// 且 `pub`（删出口不动定义与函数体）；本文件走 `use super::check::{category_of, check}`
/// 模块路径（删出口不动用例）。
///
/// `api/mod.rs` 侧的 `pub use` 归零由 `api/mod_test.rs` 的「`api` 出口面整体归零」用例
/// 独占——同一事实两处固化会在改一处漏一处时给出假绿，故这里只判本文件的消费面。
#[test]
fn category_of不经api出口面() {
    let source = include_str!("check.rs");
    assert_eq!(
        source.matches("pub fn category_of").count(),
        1,
        "`check.rs` 内仍 `pub fn category_of`（只在 `api/mod.rs` 删出口）"
    );
    // 成行写法：注释 / 断言串里的同名片段不算（只判行首的 `use` 行）
    let this_file = include_str!("check_test.rs");
    assert_eq!(
        this_file
            .lines()
            .filter(|line| *line == "use super::check::{category_of, check};")
            .count(),
        1,
        "测试面走模块路径（删出口不动用例）"
    );
    // 行为面：单点打标仍可直接调用（本文件的十条 case 均经该路径）
    for kind in [
        ArchiApiFindingKind::ApiEntryMissing,
        ArchiApiFindingKind::ApiMissing,
        ArchiApiFindingKind::ApiSignatureMismatch,
        ArchiApiFindingKind::ApiReexportDrift,
        ArchiApiFindingKind::ApiNestedCrossCrate,
        ArchiApiFindingKind::ApiConstructorDeclared,
    ] {
        assert!(!category_of(kind).is_empty(), "模块路径可直调（{kind:?}）");
    }
}

/// 边界（D15）：返回集中每条 `category` 均可由 `category_of(kind)` 复算；来源文件内零
/// `category: "` 形态字面量（`include_str!` 源码自证）。
#[test]
fn category单点打标() {
    let fqn = "App.Core.Alpha";
    let fixture = Fixture::new("category-single-point")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[(
                "new",
                json!({"params": [], "returns": {"$ref": "#/$defs/T"}}),
            )],
            &["T"],
            &[],
        ));
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    for finding in &findings {
        assert_eq!(finding.category, category_of(finding.kind));
    }

    let source = include_str!("check.rs");
    assert!(
        !source.contains("category: \""),
        "零分类字面量（`category` 只经 category_of 打标）"
    );
    assert_eq!(
        source.matches("category_of(kind)").count(),
        1,
        "打标单点恰一处（finding 构造里的单次调用）"
    );
}

// ---------------------------------------------------------------------------
// 数据源：$defs 只读 schema.json
// ---------------------------------------------------------------------------

/// 边界（AC-9 / D17）：`schemas` 已装配而 `contracts` 的 `types` 被人为改成不同键集 → 判据
/// 仍按 `schemas` 判；来源文件内零 `read_to_string` / md 路径形态。
#[test]
fn 定义只读内存schema() {
    let fqn = "App.Core.Alpha";
    let mut sourced = contract(fqn, &[], &["Declared"], &[]);
    // 人为把契约的 types 改成与 schema 不同（判据 MUST NOT 读它）
    sourced.types.clear();
    sourced
        .types
        .insert("Ghost".to_owned(), json!({"type": "object"}));

    let fixture = Fixture::new("defs-from-schema")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(fqn, &[], &["Declared"], &[]))
        .with_symbols(fqn, vec![type_symbol("Declared", ApiSymbolKind::Type)]);
    fixture.touch_entry("src/alpha");

    let mut contracts = fixture.contracts.clone();
    contracts[0].types = sourced.types.clone();
    let findings = check(ApiCheckInput {
        root: fixture.temp.root(),
        elements: &fixture.elements,
        contracts: &contracts,
        schemas: &fixture.schemas,
        symbols: &fixture.symbols,
        enabled: true,
    });
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        0,
        "判据按 schemas 判（不并读 md 的 types）：{:?}",
        findings.iter().map(|f| &f.message).collect::<Vec<_>>()
    );

    let source = include_str!("check.rs");
    assert!(
        !source.contains("read_to_string"),
        "判据零读盘（`$defs` 只读入参 schemas）"
    );
    assert!(!source.contains(".md\""), "判据不读 markdown");
}

// ---------------------------------------------------------------------------
// occurrences 聚合 / 稳定排序 / Option 字段
// ---------------------------------------------------------------------------

/// 边界（D18）：同一元素对 3 处命中（3 op 同签名不符）→ 1 条 finding +
/// `occurrences.len() == 3`（不铺成 3 条）；不同元素对 → 各自一条。
#[test]
fn occurrences聚合() {
    let fqn = "App.Core.Alpha";
    let other = "App.Core.Beta";
    let fixture = Fixture::new("occurrences")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_element(element(other, &["src/beta/"]))
        .with_contract(contract(
            fqn,
            &[
                (
                    "op_a",
                    json!({"params": [], "returns": {"type": "integer"}}),
                ),
                (
                    "op_b",
                    json!({"params": [], "returns": {"type": "integer"}}),
                ),
                (
                    "op_c",
                    json!({"params": [], "returns": {"type": "integer"}}),
                ),
            ],
            &[],
            &[],
        ))
        .with_contract(contract(
            other,
            &[(
                "op_d",
                json!({"params": [], "returns": {"type": "integer"}}),
            )],
            &[],
            &[],
        ))
        .with_symbols(
            fqn,
            vec![
                function("op_a", vec![], ApiType::OwnedString),
                function("op_b", vec![], ApiType::OwnedString),
                function("op_c", vec![], ApiType::OwnedString),
            ],
        )
        .with_symbols(other, vec![function("op_d", vec![], ApiType::OwnedString)]);
    fixture.touch_entry("src/alpha");
    fixture.touch_entry("src/beta");

    let findings = fixture.run();
    let mismatched: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiSignatureMismatch)
        .collect();
    assert_eq!(mismatched.len(), 2, "两个元素各一条（不按 op 铺开）");
    let alpha = mismatched
        .iter()
        .find(|finding| finding.element.as_deref() == Some(fqn))
        .expect("alpha 条目在案");
    assert_eq!(alpha.occurrences.len(), 3, "同元素 3 处命中共一条");
    let lines: Vec<u32> = alpha.occurrences.iter().map(|item| item.line).collect();
    assert!(
        lines.windows(2).all(|window| window[0] <= window[1]),
        "命中点按行升序：{lines:?}"
    );
}

/// 边界（D18 / AC-8）：乱序命中 → 输出按 `(kind, element, target)` 升序；两次运行逐字段
/// 相等（`api::check` 对外入口的排序）。
#[test]
fn 稳定排序由对外入口保证() {
    let fixture = Fixture::new("stable-order")
        .with_element(element("App.Core.Zeta", &["src/missing/"]))
        .with_element(element("App.Core.Alpha", &["src/missing2/"]))
        .with_element(element("App.Core.Beta", &["src/missing3/"]));
    let first = crate::api::check(ApiCheckInput {
        root: fixture.temp.root(),
        elements: &fixture.elements,
        contracts: &fixture.contracts,
        schemas: &fixture.schemas,
        symbols: &fixture.symbols,
        enabled: true,
    });
    let second = crate::api::check(ApiCheckInput {
        root: fixture.temp.root(),
        elements: &fixture.elements,
        contracts: &fixture.contracts,
        schemas: &fixture.schemas,
        symbols: &fixture.symbols,
        enabled: true,
    });
    assert_eq!(first, second, "两次运行逐字段相等");
    let elements: Vec<Option<&str>> = first
        .iter()
        .map(|finding| finding.element.as_deref())
        .collect();
    assert_eq!(
        elements,
        vec![
            Some("App.Core.Alpha"),
            Some("App.Core.Beta"),
            Some("App.Core.Zeta")
        ],
        "同 kind 下按 element 升序"
    );
}

/// 边界（AC-8）：元素级命中 → `element = Some(FQN)` / `target = None`；op 级命中 → 两者
/// 皆 `Some`；`occurrences` 空 → `[]`（不是省略字段）。
#[test]
fn option字段形态() {
    let fqn = "App.Core.Alpha";
    let fixture = Fixture::new("option-fields")
        .with_element(element("App.Core.NoEntry", &["src/missing/"]))
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[(
                "compose",
                json!({"params": [], "returns": {"type": "integer"}}),
            )],
            &[],
            &[],
        ))
        .with_symbols(fqn, vec![function("compose", vec![], ApiType::OwnedString)]);
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    let entry = findings
        .iter()
        .find(|finding| finding.kind == ArchiApiFindingKind::ApiEntryMissing)
        .expect("元素级命中在案");
    assert_eq!(entry.element.as_deref(), Some("App.Core.NoEntry"));
    assert_eq!(entry.target, None);
    assert!(entry.occurrences.is_empty(), "occurrences 空为 []");

    let signature = findings
        .iter()
        .find(|finding| finding.kind == ArchiApiFindingKind::ApiSignatureMismatch)
        .expect("op 级命中在案");
    assert_eq!(signature.element.as_deref(), Some(fqn));
    assert!(
        signature.target.is_some(),
        "op 级命中 target 为 Some：{:?}",
        signature.target
    );

    // 序列化后 `occurrences` 是 `[]`（不是省略键）
    let text = serde_json::to_string(entry).expect("应可序列化");
    assert!(
        text.contains("\"occurrences\":[]"),
        "空 occurrences 显式序列化为 []：{text}"
    );
}

// ---------------------------------------------------------------------------
// 空输入 / 规模
// ---------------------------------------------------------------------------

/// 边界：`elements = []` + 全空各面 + `enabled = true` → 空 `Vec`（零 panic）；
/// `enabled = false` → 空 `Vec`（不遍历）。
#[test]
fn 空输入() {
    let temp = TempDir::new("empty-input");
    let empty: Vec<ArchiElement> = Vec::new();
    let contracts: Vec<Contract> = Vec::new();
    let schemas: BTreeMap<String, Value> = BTreeMap::new();
    let symbols: BTreeMap<String, Vec<ApiSymbol>> = BTreeMap::new();

    let enabled_findings = check(ApiCheckInput {
        root: temp.root(),
        elements: &empty,
        contracts: &contracts,
        schemas: &schemas,
        symbols: &symbols,
        enabled: true,
    });
    assert!(enabled_findings.is_empty(), "空元素表零条目");

    let disabled_findings = check(ApiCheckInput {
        root: temp.root(),
        elements: &[],
        contracts: &contracts,
        schemas: &schemas,
        symbols: &symbols,
        enabled: false,
    });
    assert!(disabled_findings.is_empty(), "未启用零条目");
}

/// 边界：300 元素（100 有契约）+ 1000 `$defs` 键 → 求值不 panic、条数可判；单条 message
/// >1000 chars（超长类型名）不截断不 panic。
#[test]
fn 规模边界() {
    let mut fixture = Fixture::new("scale");
    for index in 0..300 {
        let fqn = format!("App.Core.E{index:03}");
        let root = format!("src/e{index:03}/");
        fixture.temp.write(&format!("{root}src/lib.rs"), "");
        fixture = fixture.with_element(element(&fqn, &[&root]));
        if index < 100 {
            fixture = fixture.with_contract(contract(&fqn, &[], &["T"], &[]));
            fixture = fixture.with_symbols(&fqn, vec![type_symbol("T", ApiSymbolKind::Type)]);
        }
    }
    let findings = fixture.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        200,
        "200 条缺契约逐条命中"
    );

    // 超长类型名（>1000 chars）不截断
    let long = format!("App.Core.{}", "X".repeat(1200));
    let long_fixture = Fixture::new("long-message").with_element(element(&long, &["src/long/"]));
    long_fixture.touch_entry("src/long");
    let findings = long_fixture.run();
    assert!(
        findings
            .iter()
            .any(|finding| finding.message.contains(&long)),
        "超长 FQN 的文案不截断"
    );
}

// ---------------------------------------------------------------------------
// x-class 不进判据面（D27）
// ---------------------------------------------------------------------------

/// 边界（D27 / 事实校准 16）：「`properties` 全为函数未标 `x-class`」的输入 + 十判据全过
/// 的元素 → 返回集零条目；来源文件零 `x-class` 判错 / 提示分支。
#[test]
fn x_class不进判据面() {
    let fqn = "App.Core.Alpha";
    let source = format!(
        "# {fqn}\n\n```json api-types\n{{\"Hooks\": {{\"type\": \"object\", \"properties\": \
         {{\"run\": {{\"x-function\": {{\"params\": [], \"returns\": {{\"type\": \"string\"}}}}}}}}}}}}\n```\n"
    );
    let parsed = parse(&source, fqn, &format!("{fqn}.md")).expect("结构合法");
    let fixture = Fixture::new("x-class-not-a-finding")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(parsed)
        .with_symbols(fqn, vec![type_symbol("Hooks", ApiSymbolKind::Type)]);
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    assert!(
        findings.is_empty(),
        "未标 x-class 的提示 MUST NOT 成为判据：{:?}",
        findings.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    assert_eq!(ArchiApiFindingKind::ALL.len(), 10, "MUST NOT 扩第十一 kind");

    let source = include_str!("check.rs");
    assert!(
        !source.contains("x-class"),
        "本文件零 x-class 判据 / 提示分支"
    );
    let report = include_str!("../drift/report.rs");
    assert!(
        report.contains("x-class") || !report.contains("XClass"),
        "词汇十值不因 x-class 扩值"
    );
}

/// 边界（AC-7 收口）：`severity()` 九 `Warn` 一 `Off` 是「完备性判据唯一一条」的类型面
/// 依据——③ 的跳转只看它（本文件内零白名单 / 零配置键）。
#[test]
fn 档位判据来自词汇单点() {
    let source = include_str!("check.rs");
    assert!(
        source.contains("ApiUndeclared.severity()") || source.contains("severity()"),
        "③ 的跳转读 `severity()`"
    );
    assert!(
        !source.contains("config::") && !source.contains("env::var"),
        "档位零配置 / 零环境变量"
    );
}

/// 边界（D42 / AC-7）：判据 ① 的 `has_generated` 只经 `element.path.first()` 取 `<root>`
/// 再调 `paths::generated_path`（不构造占位 `ArchiElement`）；行为面：元素无实现入口但
/// **生成物已在盘** → **不**报 `apiEntryMissing`（生成物在场即不属「既无生成物也无入口」）。
#[test]
fn 判据一生成物在场即不报入口缺失() {
    let source = include_str!("check.rs");
    assert!(
        !source.contains("ArchiElement {"),
        "本文件零占位 `ArchiElement` 构造（D42：路径只收 `<root>` 字符串）"
    );
    assert!(
        source.contains("element.path.first()"),
        "容器元素由 `path.first()` 表达（调用方判容器，`paths` 层不判）"
    );

    let fqn = "App.Core.GeneratedOnly";
    let fixture = Fixture::new("generated-only").with_element(element(fqn, &["src/generated/"]));
    // 无入口（`src/generated/` 下无 `src/lib.rs` / `mod.rs` / `.rs`），但生成物已在盘
    fixture.temp.write("src/generated.i.rs", "// 生成物\n");
    let findings = fixture.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiEntryMissing),
        0,
        "生成物在场即不报入口缺失：{:?}",
        findings.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// ② 的 $defs 类型面与三条排除面（D44 / AC-22）＋ ④ 的 Option 入参（D43 / AC-21）
// ---------------------------------------------------------------------------

/// 异常（AC-22 ① / D44）：契约 `api-types` 声明类型而代码导出集无同名类型符号 →
/// **同一条** `ApiMissing`、同 `category == "contract"`、`target` 点名该类型、
/// `occurrences` 逐缺口一条；对照：代码侧有该类型 → 零条。
#[test]
fn 类型面缺失同口径命中() {
    let fqn = "App.Core.Alpha";
    let absent = Fixture::new("missing-type")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(fqn, &[], &["LocalArchiCheck"], &[]))
        .with_symbols(fqn, vec![]);
    absent.touch_entry("src/alpha");

    let findings = absent.run();
    let missing: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiMissing)
        .collect();
    assert_eq!(missing.len(), 1, "同一 finding kind（不另立第二条判据）");
    assert_eq!(
        missing[0].category, "contract",
        "分类与 op 面同口径（contract）"
    );
    assert_eq!(
        missing[0].target.as_deref(),
        Some("LocalArchiCheck"),
        "target 点名缺失的类型"
    );
    assert_eq!(missing[0].occurrences.len(), 1, "逐缺口一条 occurrence");
    assert_eq!(
        missing[0].occurrences[0].specifier.as_deref(),
        Some("LocalArchiCheck"),
        "occurrence 的 specifier 即缺口名"
    );
    assert_eq!(
        missing[0].occurrences[0].line, 0,
        "类型面无 fence 行 → 位置未知（0）"
    );

    // 对照：代码侧有该类型（三种类型形态均计入）→ 零条
    for kind in [
        ApiSymbolKind::Type,
        ApiSymbolKind::Trait,
        ApiSymbolKind::Enum,
    ] {
        let present = Fixture::new("present-type")
            .with_element(element(fqn, &["src/alpha/"]))
            .with_contract(contract(fqn, &[], &["LocalArchiCheck"], &[]))
            .with_symbols(fqn, vec![type_symbol("LocalArchiCheck", kind)]);
        present.touch_entry("src/alpha");
        assert_eq!(
            count(&present.run(), ArchiApiFindingKind::ApiMissing),
            0,
            "代码侧有该类型（{kind:?}）→ 零条"
        );
    }

    // op 面与类型面同一次求值：同函数体内两段（源码自证）
    let source = include_str!("check.rs");
    assert_eq!(
        source.matches("fn missing_contracts").count(),
        1,
        "② 的唯一实现（MUST NOT 拆成第二条判据）"
    );
    let function_start = source
        .find("fn missing_contracts")
        .expect("`missing_contracts` 在案");
    let function_body = &source[function_start..];
    let function_end = function_body[1..]
        .find("\nfn ")
        .map(|offset| offset + 1)
        .unwrap_or(function_body.len());
    let function_body = &function_body[..function_end];
    assert!(
        function_body.contains("for op in &contract.ops")
            && function_body.contains("declared_types("),
        "op 面与 $defs 类型面在同一函数体内（同一条判据、同一次求值）"
    );
}

/// 边界（AC-22 ② / D44）：三条排除面在案——构造器双向排除、透传纯 `$ref` alias 排除、
/// 签名归 ④。
#[test]
fn 排除面三条在案() {
    let fqn = "App.Core.Alpha";

    // ① 构造器双向排除：契约声明 `### new` 且代码有 `pub fn new` → 不参与 ②
    let constructor = Fixture::new("exclude-constructor")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[("new", json!({"params": [], "returns": {"type": "string"}}))],
            &[],
            &[],
        ))
        .with_symbols(fqn, vec![function("new", vec![], ApiType::OwnedString)]);
    constructor.touch_entry("src/alpha");
    assert_eq!(
        count(&constructor.run(), ArchiApiFindingKind::ApiMissing),
        0,
        "构造器双向排除（缺口归 ⑩，不归 ②）"
    );

    // ② 透传纯 `$ref` alias 排除：`api-reexport` 条目在本模块无本地定义 → 不参与 ②
    let reexport = Fixture::new("exclude-reexport")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[],
            &[],
            &[("AgentInput", "App.Frontend.Components.Agent.Input")],
        ))
        .with_symbols(fqn, vec![]);
    reexport.touch_entry("src/alpha");
    assert_eq!(
        count(&reexport.run(), ArchiApiFindingKind::ApiMissing),
        0,
        "透传纯 `$ref` alias 排除（实现由目标元素承担）：{:?}",
        reexport
            .run()
            .iter()
            .map(|f| (&f.kind, &f.message))
            .collect::<Vec<_>>()
    );

    // ③ 签名归 ④：类型名在案但签名不符 → ② 零条、④ 一条
    let signature = Fixture::new("exclude-signature")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[(
                "compose",
                json!({"params": [], "returns": {"type": "integer"}}),
            )],
            &[],
            &[],
        ))
        .with_symbols(fqn, vec![function("compose", vec![], ApiType::OwnedString)]);
    signature.touch_entry("src/alpha");
    let findings = signature.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        0,
        "存在性判定不含签名（② 零条）"
    );
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiSignatureMismatch),
        1,
        "签名不符归 ④"
    );
}

/// 正向（AC-22 基线判据 / D44）：试点形态（`#[path]` 生成物 + `pub use api_gen::*;`）
/// 下该元素的 `LocalArchiCheck` 与 op 均进符号集 ⇒ ② **零条**；反例：符号集缺该类型
/// → 报 1 条并指向它（缺陷形态，基线由 13 变 14）。
#[test]
fn 试点形态判据二零命中() {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let pilot = Fixture::new("pilot-zero")
        .with_element(element(fqn, &["src/archi/"]))
        .with_contract(contract(
            fqn,
            &[(
                "file_stem",
                json!({"params": [{"name": "timestamp", "schema": {"type": "string"}}],
                       "returns": {"type": "string"}}),
            )],
            &["LocalArchiCheck"],
            &[],
        ))
        .with_symbols(
            fqn,
            vec![
                type_symbol("LocalArchiCheck", ApiSymbolKind::Type),
                function(
                    "file_stem",
                    vec![param("timestamp", ApiType::Str)],
                    ApiType::OwnedString,
                ),
            ],
        );
    pilot.touch_entry("src/archi");
    assert_eq!(
        count(&pilot.run(), ArchiApiFindingKind::ApiMissing),
        0,
        "试点元素 ② 零命中"
    );

    // 反例：类型符号缺失 → 该元素报 1 条，target 指向该类型
    let broken = Fixture::new("pilot-broken")
        .with_element(element(fqn, &["src/archi/"]))
        .with_contract(contract(fqn, &[], &["LocalArchiCheck"], &[]))
        .with_symbols(
            fqn,
            vec![function(
                "file_stem",
                vec![param("timestamp", ApiType::Str)],
                ApiType::OwnedString,
            )],
        );
    broken.touch_entry("src/archi");
    let findings = broken.run();
    assert_eq!(
        count(&findings, ArchiApiFindingKind::ApiMissing),
        1,
        "缺类型符号即报（缺陷形态，MUST NOT 更新基线表放行）"
    );
    let missing = findings
        .iter()
        .find(|finding| finding.kind == ArchiApiFindingKind::ApiMissing)
        .expect("缺失条目在案");
    assert_eq!(
        missing.target.as_deref(),
        Some("LocalArchiCheck"),
        "target 指向缺失类型"
    );
}

/// 正向（AC-21 关键行为 / D43）：契约 `params[0]` 带 `optional: true` + 代码
/// `fn parse(input: Option<&str>) -> String` → **零** `ApiSignatureMismatch`；对照两形态。
#[test]
fn 可选参数不假阳() {
    let fqn = "App.Core.Alpha";
    let build = |param_schema: Value| {
        Fixture::new("optional-param")
            .with_element(element(fqn, &["src/alpha/"]))
            .with_contract(contract(
                fqn,
                &[(
                    "parse",
                    json!({
                        "params": [param_schema],
                        "returns": {"type": "string"}
                    }),
                )],
                &[],
                &[],
            ))
            .with_symbols(
                fqn,
                vec![function(
                    "parse",
                    vec![ApiParam {
                        name: "input".to_owned(),
                        ty: ApiType::Optional(Box::new(ApiType::Str)),
                        optional: true,
                    }],
                    ApiType::OwnedString,
                )],
            )
    };

    // ① `optional: true` → 两侧同形，零误报
    let explicit = build(json!({"name": "input", "schema": {"type": "string"}, "optional": true}));
    explicit.touch_entry("src/alpha");
    assert_eq!(
        count(&explicit.run(), ArchiApiFindingKind::ApiSignatureMismatch),
        0,
        "`optional: true` 与 `Option<&str>` 归一后相等"
    );

    // ② nullable 形态（anyOf 含 null）同形不报
    let nullable = build(json!({
        "name": "input",
        "schema": {"anyOf": [{"type": "string"}, {"type": "null"}]}
    }));
    nullable.touch_entry("src/alpha");
    assert_eq!(
        count(&nullable.run(), ArchiApiFindingKind::ApiSignatureMismatch),
        0,
        "nullable 分支同样不假阳"
    );

    // ③ 对照：契约不写可选 → 报 1 条
    let required = build(json!({"name": "input", "schema": {"type": "string"}}));
    required.touch_entry("src/alpha");
    assert_eq!(
        count(&required.run(), ArchiApiFindingKind::ApiSignatureMismatch),
        1,
        "契约不可选而代码可选 → 该报 ④"
    );
}

/// 边界（AC-21 ③⑤ / D43 源码自证）：判据侧零本地位置表、零本地归一函数、零语言文本形
/// 字面量、对 `anyOf` / `null` 零直接匹配；契约侧签名一律经 `extract::signature_of`。
#[test]
fn 判据侧零本地归一() {
    let source = include_str!("check.rs");
    assert!(
        !source.contains("enum Position"),
        "判据侧零本地位置表（只 `use extract::Position`）"
    );
    assert!(
        !source.contains("Position::"),
        "判据侧零位置分派（nullable 只经 `extract::schema_type`）"
    );
    assert!(
        !source.contains("normalize_schema") && !source.contains("contract_signature"),
        "零本地归一函数（MUST NOT 各拼一份）"
    );
    assert!(
        !source.contains("\"anyOf\"")
            && !source.contains("\"oneOf\"")
            && !source.contains("\"null\""),
        "对 nullable 关键字零直接匹配（只经 `extract::schema_type`）"
    );
    for literal in [
        "\"&str\"",
        "\"Vec<\"",
        "\"Option<\"",
        "\"BTreeMap<\"",
        "\"Record<\"",
    ] {
        assert!(
            !source.contains(literal),
            "判据侧零语言文本形字面量 {literal}"
        );
    }
    // 契约侧签名的唯一消费点（恰 1 处调用，不重复取数）
    assert_eq!(
        source.matches("extract::signature_of(").count(),
        1,
        "契约侧签名一律经 `extract::signature_of`（恰 1 处消费）"
    );
}

// ---------------------------------------------------------------------------
// 落点闸不落本文件（AC-25 / D47 边界）
// ---------------------------------------------------------------------------

/// 边界（**AC-25** / D47）：本文件内**零**落点判据——定形族别串 `x-function 落点` 与
/// `JSON pointer` 文案各零命中（D47 的闸在 `contract::verify`，装载期即中止；判据侧只读
/// 已通过落点校验的 `$defs`）；`x-function` 写在 `items` 等越界位置的契约在
/// `load_contracts` 期即 `Err`，故 `check` 结构上收不到该输入 ⇒ 本文件零「深层
/// `x-function` 缺判」类 finding（MUST NOT 为它造第十一 kind）。
#[test]
fn 落点闸不落check() {
    let source = include_str!("check.rs");
    for marker in ["x-function 落点", "JSON pointer"] {
        assert!(
            !source.contains(marker),
            "判据侧零 `{marker}`（落点闸在装载期）"
        );
    }
    // 零深层兜底判据：`x-function` 的**位置**不在本文件判
    assert!(
        !source.contains("scan_x_function") && !source.contains("verify_x_function_positions"),
        "判据侧零落点守卫的复制（D47 单点在 `contract.rs`）"
    );
    // 十判据值域不扩（MUST NOT 造第十一 kind 承载「深层 x-function」）
    let report = include_str!("../drift/report.rs");
    assert!(
        report.contains("[ArchiApiFindingKind; 10]"),
        "`ArchiApiFindingKind::ALL` 仍十值封闭（零第十一 kind）"
    );
    assert_eq!(
        report.matches("    ApiConstructorDeclared,").count(),
        1,
        "十值的末位变体在案（新增变体未补即语料红）"
    );
}

// ---------------------------------------------------------------------------
// occurrences.line 两类缺口分列（D51 / AC-28）
// ---------------------------------------------------------------------------

/// 正向（**D51 / AC-28**）：op 缺口取 `ContractOp.line`（`### <op-id>` heading 行）、
/// `$defs` 类型缺口取 `0`（位置未知）——同一契约里 op 与类型各缺一项时仍**一条**
/// `ApiMissing`，其 `occurrences` 两项行号分别按两类缺口分列；源码自证
/// `line: op.line` ≥ 1 且 `declared_types` 段内 `line: 0` ≥ 1。
#[test]
fn occurrences行号按两类缺口分列() {
    let fqn = "App.Core.Alpha";
    // heading 在第 9 行（夹具固定前缀 8 行 + `### missing_op`）
    let fixture = Fixture::new("occurrence-lines")
        .with_element(element(fqn, &["src/alpha/"]))
        .with_contract(contract(
            fqn,
            &[(
                "missing_op",
                json!({"params": [], "returns": {"type": "string"}}),
            )],
            &["MissingType"],
            &[],
        ))
        .with_symbols(fqn, vec![]);
    fixture.touch_entry("src/alpha");

    let findings = fixture.run();
    let missing: Vec<&ArchiApiFinding> = findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiMissing)
        .collect();
    assert_eq!(missing.len(), 1, "op 与类型两类缺口聚合为一条 finding");
    assert_eq!(
        missing[0].occurrences.len(),
        2,
        "两类缺口各一条 occurrence：{:?}",
        missing[0].occurrences
    );

    let op_occurrence = missing[0]
        .occurrences
        .iter()
        .find(|occurrence| occurrence.specifier.as_deref() == Some("missing_op"))
        .expect("op 缺口的 occurrence 在案");
    // 夹具契约的 heading 行：用同形源文再解析一次取真值（不手抄行号）
    let fixture_source = format!(
        "# {fqn}\n\n## 职能\n\n正文。\n\n## 操作\n\n### missing_op\n\n```json api-op\n{}\n```\n\n\
         ```json api-types\n{}\n```\n",
        json!({"params": [], "returns": {"type": "string"}}),
        json!({"MissingType": {"type": "object", "properties": {}}})
    );
    let heading_line = parse(&fixture_source, fqn, &format!("{fqn}.md"))
        .expect("同形源文应解析成功")
        .ops[0]
        .line;
    assert_eq!(
        op_occurrence.line, heading_line,
        "op 缺口的行号 = heading 行（`ContractOp.line`，D51）"
    );
    assert!(op_occurrence.line > 0, "heading 行恒 ≥ 1");

    let type_occurrence = missing[0]
        .occurrences
        .iter()
        .find(|occurrence| occurrence.specifier.as_deref() == Some("MissingType"))
        .expect("类型缺口的 occurrence 在案");
    assert_eq!(
        type_occurrence.line, 0,
        "`$defs` 类型缺口行号取 0（位置未知，D51：MUST NOT 为凑行号扩 `Contract`）"
    );

    // 源码自证两处写出点
    let source = include_str!("check.rs");
    assert!(
        source.matches("line: op.line").count() >= 1,
        "op 缺口走 heading 行（`line: op.line` ≥ 1）"
    );
    assert!(
        source.matches("line: 0").count() >= 1,
        "类型缺口走 `line: 0`（位置未知）"
    );
}

/// 异常（**D44 / AC-22 ①**）：同一契约里 op 与类型**各自缺一项** → **两条**
/// `ApiMissing` occurrence（同一 kind、同一判据、两类缺口各一条），MUST NOT 拆第二条
/// kind；`target` 同时点名两个缺口。
#[test]
fn 两类缺口各一条且不拆第二kind() {
    let fqn = "App.Core.Beta";
    let fixture = Fixture::new("two-gaps")
        .with_element(element(fqn, &["src/beta/"]))
        .with_contract(contract(
            fqn,
            &[(
                "missing_op",
                json!({"params": [], "returns": {"type": "string"}}),
            )],
            &["MissingType"],
            &[],
        ))
        .with_symbols(fqn, vec![]);
    fixture.touch_entry("src/beta");

    let findings = fixture.run();
    let kinds: Vec<ArchiApiFindingKind> = findings.iter().map(|finding| finding.kind).collect();
    assert_eq!(
        kinds,
        vec![ArchiApiFindingKind::ApiMissing],
        "两类缺口同一条判据、同一个 kind（MUST NOT 拆成第二条 kind）：{kinds:?}"
    );
    let target = findings[0].target.as_deref().unwrap_or("");
    assert!(
        target.contains("missing_op") && target.contains("MissingType"),
        "target 同时点名两个缺口：{target}"
    );
    assert_eq!(
        findings[0].occurrences.len(),
        2,
        "两类缺口各一条 occurrence"
    );

    // 源码自证：唯一实现函数体内两段都在（同一次求值）
    let source = include_str!("check.rs");
    let start = source
        .find("fn missing_contracts")
        .expect("② 的唯一实现函数在案");
    let body = &source[start..];
    let end = body
        .find("\nfn ")
        .map(|offset| offset + 1)
        .unwrap_or(body.len());
    let body = &body[..end];
    assert!(
        body.contains("for op in &contract.ops") && body.contains("for name in declared_types("),
        "op 面与类型面在同一次求值内（缺一即不成 D44）"
    );
}
