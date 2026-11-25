//! `edit::warnings` 的单元测试：写面反馈面七条判据（`noPath` / `pathNotFound` /
//! `docMissing` 两支 / `docNotFound` / `apiUnexpected` / 删除后孤儿契约）逐条红绿、
//! `kind` 与 `severity` 逐值取自既有两个单点、降噪不变式（非诱发字段零 warning、
//! `created` 视为全字段、两类归因互不串门）、`message` 形态，以及「零 `drift::check`
//! 零写盘」的源码锚。
//!
//! 真值面：`api::paths::{contract_path, doc_path}` 与真实盘上文件——「契约 md 在 / 不在」
//! 与「`path` 条目在 / 不在盘」正是被测判据，替身会把它抹掉；`Element` 由真实
//! `TomlParser::parse` 解析合成源取得（不手搓残形）。临时根由 std 自建（本 crate 零
//! dev-dependency），沿 `edit/mod_test.rs` 的 `TempDir` RAII + 计数器先例。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::api::paths::{contract_path, doc_path};
use crate::drift::report::{ApiFindingSeverity, ArchiApiFindingKind, ArchiDriftKind};
use crate::model::Element;
use crate::parse::{SourceParser, TomlParser};

use super::warnings::{delete_warnings, upsert_warnings, EditWarning};

// ---------------------------------------------------------------------------
// 合成源与临时根
// ---------------------------------------------------------------------------

/// `package` 且 `path` 空（第 1 条 `noPath` 的标准触发形态）。
const PACKAGE_EMPTY_PATH: &str = concat!(
    "[element.\"App.Core\"]\n",
    "kind = \"package\"\n",
    "path = []\n",
);

/// `module` 且 `path` 空（第 1 条的另一触发形态）。
const MODULE_EMPTY_PATH: &str = concat!(
    "[element.\"App.Core\"]\n",
    "kind = \"module\"\n",
    "path = []\n",
);

/// `composite` 且 `path` 空（对照组：容器空 `path` 是合法形态）。
const COMPOSITE_EMPTY_PATH: &str = concat!(
    "[element.\"App.Core\"]\n",
    "kind = \"composite\"\n",
    "path = []\n",
);

/// `path` 两项都不在盘（第 2 条的矩阵形态）。
const PATH_MISSING: &str = concat!(
    "[element.\"App.Core\"]\n",
    "kind = \"module\"\n",
    "path = [\"src/absent-a\", \"src/absent-b\"]\n",
);

/// `api = declared` 且派生契约 md 缺席（第 3 条形态）。
const DECLARED_NO_CONTRACT: &str = concat!(
    "[element.\"App.Core\"]\n",
    "kind = \"domain\"\n",
    "api = \"declared\"\n",
);

/// `api = none` 且 `doc` 未声明（第 4 条形态）。
const NONE_WITHOUT_DOC: &str = concat!(
    "[element.\"App.Core\"]\n",
    "kind = \"domain\"\n",
    "api = \"none\"\n",
);

/// `api = none` + `doc` 指向盘上缺席的文件（第 5 条形态）。
const NONE_DOC_ABSENT: &str = concat!(
    "[element.\"App.Core\"]\n",
    "kind = \"domain\"\n",
    "api = \"none\"\n",
    "doc = \"docs/core.md\"\n",
);

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 临时根（RAII 清理；本 crate 零 dev-dependency，故用 std 自建）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-warnings-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时根失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    /// 落派生契约 md（`declared` 的入口文档载体）。
    fn contract(&self, fqn: &str) {
        let path = contract_path(self.root(), fqn).expect("契约路径应可派生");
        fs::create_dir_all(path.parent().expect("契约 md 有父目录")).expect("建契约目录失败");
        fs::write(&path, "# 契约\n").expect("写契约 md 失败");
    }

    /// 落 `doc` 声明的入口文档（repo 相对串）。
    fn doc(&self, doc: &str) {
        let path = doc_path(self.root(), doc).expect("doc 路径应可派生");
        fs::create_dir_all(path.parent().expect("doc 有父目录")).expect("建 doc 目录失败");
        fs::write(&path, "# 入口文档\n").expect("写 doc 失败");
    }

    /// 落一个 `path` 条目（目录形态）。
    fn dir_entry(&self, entry: &str) {
        fs::create_dir_all(self.root().join(entry)).expect("建 path 条目失败");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 解析合成源并取目标元素（真实解析器，零手搓）。
fn element_of(source: &str, fqn: &str) -> Element {
    let outcome = TomlParser::parse(source);
    assert!(
        outcome.diagnostics.is_empty(),
        "合成源应零诊断：{:?}",
        outcome.diagnostics
    );
    outcome
        .model
        .element(fqn)
        .cloned()
        .unwrap_or_else(|| panic!("合成源应含元素 {fqn}"))
}

/// `changed` 参数面字段名切片。
fn changed(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|field| (*field).to_owned()).collect()
}

/// 判据词清单（按产出序）。
fn kinds(warnings: &[EditWarning]) -> Vec<&'static str> {
    warnings.iter().map(|warning| warning.kind).collect()
}

/// 单条 warning 的 `(kind, severity)`。
fn pair(warning: &EditWarning) -> (&'static str, &'static str) {
    (warning.kind, warning.severity)
}

// ---------------------------------------------------------------------------
// 第 1、2 条：`noPath` / `pathNotFound`
// ---------------------------------------------------------------------------

/// 正向（矩阵 3 形态）：`package` / `module` 且 `path` 空 ⇒ 一条 `noPath` / `warn`；
/// `composite` 且 `path` 空 ⇒ 零条（纯分组容器外的空 `path` 就是缺陷）。
#[test]
#[allow(non_snake_case)]
fn 纯分组容器外的空path报noPath() {
    let temp = TempDir::new("no-path");
    temp.contract("App.Core");
    for (source, expected) in [
        (PACKAGE_EMPTY_PATH, 1usize),
        (MODULE_EMPTY_PATH, 1),
        (COMPOSITE_EMPTY_PATH, 0),
    ] {
        let element = element_of(source, "App.Core");
        let warnings = upsert_warnings(temp.root(), &element, &changed(&["path"]), false);
        assert_eq!(
            warnings.len(),
            expected,
            "形态 {:?} 的 warning 条数",
            element.kind.as_str()
        );
        if expected == 1 {
            assert_eq!(pair(&warnings[0]), ("noPath", "warn"), "判据词与档位");
            assert!(
                warnings[0].message.contains("path 为空")
                    && warnings[0].message.contains("composite"),
                "判据句 + 下一步：{}",
                warnings[0].message
            );
        }
    }
}

/// 边界（矩阵 2 形态）：`noPath` 只由 `changed ∋ path` 或 `changed ∋ kind` 诱发；
/// 只改 `api` / `doc` 时零条。
#[test]
#[allow(non_snake_case)]
fn noPath只由path或kind诱发() {
    let temp = TempDir::new("no-path-induced");
    temp.contract("App.Core");
    let element = element_of(PACKAGE_EMPTY_PATH, "App.Core");

    for fields in [&["api"][..], &["doc"][..]] {
        let warnings = upsert_warnings(temp.root(), &element, &changed(fields), false);
        assert!(
            !kinds(&warnings).contains(&"noPath"),
            "{fields:?} 不诱发 noPath：{warnings:?}"
        );
    }
    for fields in [&["path"][..], &["kind"][..]] {
        let warnings = upsert_warnings(temp.root(), &element, &changed(fields), false);
        assert_eq!(
            kinds(&warnings),
            vec!["noPath"],
            "{fields:?} 各诱发一条 noPath"
        );
    }
}

/// 异常（矩阵 3 形态）：`path` 缺席项**逐条**一条 `pathNotFound` / `warn`（与
/// `drift::check` 的「每条失效 path 各一条」同形），逐条 message 含该路径；全部在盘
/// ⇒ 零条。
#[test]
#[allow(non_snake_case)]
fn path缺席逐条报pathNotFound() {
    let temp = TempDir::new("path-missing");
    temp.contract("App.Core");
    let element = element_of(PATH_MISSING, "App.Core");

    let warnings = upsert_warnings(temp.root(), &element, &changed(&["path"]), false);
    assert_eq!(warnings.len(), 2, "每条缺席项一条：{warnings:?}");
    assert_eq!(
        kinds(&warnings),
        vec!["pathNotFound", "pathNotFound"],
        "逐条同判据词、不合并成一条"
    );
    for (warning, entry) in warnings.iter().zip(["src/absent-a", "src/absent-b"]) {
        assert_eq!(pair(warning), ("pathNotFound", "warn"), "判据词与档位");
        assert!(
            warning.message.contains(entry),
            "逐条 message 含该路径 {entry}：{}",
            warning.message
        );
    }

    // 文件形态的缺席项同判（目录与文件都只问 `exists`）。
    let file_form = concat!(
        "[element.\"App.Core\"]\n",
        "kind = \"module\"\n",
        "path = [\"docs/absent.md\"]\n",
    );
    let warnings = upsert_warnings(
        temp.root(),
        &element_of(file_form, "App.Core"),
        &changed(&["path"]),
        false,
    );
    assert_eq!(kinds(&warnings), vec!["pathNotFound"], "文件形态亦逐条");

    temp.dir_entry("src/absent-a");
    temp.dir_entry("src/absent-b");
    let warnings = upsert_warnings(temp.root(), &element, &changed(&["path"]), false);
    assert!(warnings.is_empty(), "全部在盘 ⇒ 零条：{warnings:?}");
}

// ---------------------------------------------------------------------------
// 第 3–6 条：契约面三判据
// ---------------------------------------------------------------------------

/// 正向（矩阵 2 形态）：`api = declared` 且派生契约 md 缺席 ⇒ `docMissing` / `warn`
/// 且尾句指向 `archi_api` 的 `bootstrap`；契约 md 在盘 ⇒ 零条。
#[test]
#[allow(non_snake_case)]
fn declared缺契约报docMissing() {
    let temp = TempDir::new("declared-missing");
    let element = element_of(DECLARED_NO_CONTRACT, "App.Core");

    let warnings = upsert_warnings(temp.root(), &element, &changed(&["api"]), false);
    assert_eq!(warnings.len(), 1, "只此一条：{warnings:?}");
    assert_eq!(pair(&warnings[0]), ("docMissing", "warn"));
    assert!(
        warnings[0].message.contains("bootstrap"),
        "下一步指向 bootstrap：{}",
        warnings[0].message
    );

    temp.contract("App.Core");
    let warnings = upsert_warnings(temp.root(), &element, &changed(&["api"]), false);
    assert!(warnings.is_empty(), "契约 md 在盘 ⇒ 零条：{warnings:?}");
}

/// 异常（矩阵 2 形态）：`api = none` 且 `doc` 未声明 ⇒ `docMissing` / `warn`，
/// 尾句指向经 `doc` 声明；`doc` 有值即在盘时该条零命中（转由第 5 条判）。
#[test]
#[allow(non_snake_case)]
fn none未声明doc报docMissing() {
    let temp = TempDir::new("none-no-doc");
    let element = element_of(NONE_WITHOUT_DOC, "App.Core");

    let warnings = upsert_warnings(temp.root(), &element, &changed(&["api"]), false);
    assert_eq!(warnings.len(), 1, "只此一条：{warnings:?}");
    assert_eq!(pair(&warnings[0]), ("docMissing", "warn"));
    assert!(
        warnings[0].message.contains("入口文档经 doc 声明"),
        "尾句指向 doc 声明：{}",
        warnings[0].message
    );

    temp.doc("docs/core.md");
    let element = element_of(NONE_DOC_ABSENT, "App.Core");
    let warnings = upsert_warnings(temp.root(), &element, &changed(&["doc"]), false);
    assert!(warnings.is_empty(), "doc 在盘 ⇒ 零条：{warnings:?}");
}

/// 异常（矩阵 2 形态）：`api = none` + `doc` 指不到盘 ⇒ `docNotFound` / **`error`**
/// （七条中唯一 `error` 档）；`doc` 指到盘 ⇒ 零条。
#[test]
#[allow(non_snake_case)]
fn doc指不到盘报docNotFound且档位为error() {
    let temp = TempDir::new("doc-not-found");
    let element = element_of(NONE_DOC_ABSENT, "App.Core");

    let warnings = upsert_warnings(temp.root(), &element, &changed(&["doc"]), false);
    assert_eq!(warnings.len(), 1, "只此一条：{warnings:?}");
    assert_eq!(pair(&warnings[0]), ("docNotFound", "error"));
    assert!(
        warnings[0].message.contains("docs/core.md"),
        "message 应含声明的 doc：{}",
        warnings[0].message
    );

    temp.doc("docs/core.md");
    let warnings = upsert_warnings(temp.root(), &element, &changed(&["doc"]), false);
    assert!(warnings.is_empty(), "doc 指到盘 ⇒ 零条：{warnings:?}");
}

/// 异常（矩阵 2 形态）：`api = none` 且派生契约 md 在盘 ⇒ `apiUnexpected` / `warn`
/// 且尾句给「删契约 md / 改 declared」两条出路；`api = declared` 且在盘 ⇒ 零条。
#[test]
#[allow(non_snake_case)]
fn none有契约报apiUnexpected() {
    let temp = TempDir::new("none-with-contract");
    temp.contract("App.Core");
    temp.doc("docs/core.md");
    let element = element_of(NONE_DOC_ABSENT, "App.Core");

    let warnings = upsert_warnings(temp.root(), &element, &changed(&["api"]), false);
    assert_eq!(warnings.len(), 1, "只此一条：{warnings:?}");
    assert_eq!(pair(&warnings[0]), ("apiUnexpected", "warn"));
    assert!(
        warnings[0].message.contains("删契约 md") && warnings[0].message.contains("declared"),
        "两条出路齐备：{}",
        warnings[0].message
    );

    let declared = element_of(DECLARED_NO_CONTRACT, "App.Core");
    let warnings = upsert_warnings(temp.root(), &declared, &changed(&["api"]), false);
    assert!(warnings.is_empty(), "declared + 在盘 ⇒ 零条：{warnings:?}");
}

// ---------------------------------------------------------------------------
// 降噪不变式
// ---------------------------------------------------------------------------

/// 边界（矩阵 4 形态）：`changed` 只含 `summary` / `gate` / `metadata` 或为空 ⇒
/// 事实面虽可触发判据，`warnings` 恒空。
#[test]
fn 非诱发字段零warning() {
    let temp = TempDir::new("quiet-fields");
    let element = element_of(PACKAGE_EMPTY_PATH, "App.Core");

    for fields in [&["summary"][..], &["gate"][..], &["metadata"][..], &[][..]] {
        let warnings = upsert_warnings(temp.root(), &element, &changed(fields), false);
        assert!(
            warnings.is_empty(),
            "{fields:?} 不诱发任何判据：{warnings:?}"
        );
    }
}

/// 边界（矩阵 2 形态）：`created = true` 时 `changed` 视为全字段（该报的都在）；
/// `created = false` + 同条件按 `changed` 收窄。
#[test]
fn created放行全集() {
    let temp = TempDir::new("created-full");
    let element = element_of(PACKAGE_EMPTY_PATH, "App.Core");

    let created = upsert_warnings(temp.root(), &element, &changed(&["kind"]), true);
    assert_eq!(
        kinds(&created),
        vec!["noPath", "docMissing"],
        "created 视为全字段（path 面 + 契约面各一条）"
    );

    let narrowed = upsert_warnings(temp.root(), &element, &changed(&["kind"]), false);
    assert_eq!(
        kinds(&narrowed),
        vec!["noPath"],
        "按 changed 收窄（api 面零诱发）"
    );
}

/// 边界（矩阵 2 形态）：只改 `path` ⇒ 契约三条零命中；只改 `doc` ⇒ `noPath` /
/// `pathNotFound` 零命中（两类 warning 归因互不串门）。
#[test]
fn 两类warning归因互不串门() {
    let temp = TempDir::new("attribution");
    let element = element_of(PACKAGE_EMPTY_PATH, "App.Core");

    let path_only = upsert_warnings(temp.root(), &element, &changed(&["path"]), false);
    assert_eq!(kinds(&path_only), vec!["noPath"], "只走 path 面");

    let api_only = upsert_warnings(temp.root(), &element, &changed(&["doc"]), false);
    assert_eq!(kinds(&api_only), vec!["docMissing"], "只走契约面");
}

// ---------------------------------------------------------------------------
// 词汇 / 档位单点与 message 形态
// ---------------------------------------------------------------------------

/// 异常（穷举）：每条产出的 `kind` ∈ 两枚举 `as_str` 并集；`severity` 逐值取自既有
/// 单点（`docNotFound` ⇒ `error`，`docMissing` / `apiUnexpected` ⇒ `warn`，drift 两值
/// 恒 `warn`）。
#[test]
fn kind与severity逐值取自既有单点() {
    let temp = TempDir::new("vocabulary");
    temp.contract("App.Core");
    let mut produced: Vec<EditWarning> = Vec::new();

    // 五个 upsert 判据逐条触发（`none` 支的 docNotFound 与 apiUnexpected 同批齐发）。
    produced.extend(upsert_warnings(
        temp.root(),
        &element_of(PACKAGE_EMPTY_PATH, "App.Core"),
        &changed(&["path"]),
        false,
    ));
    produced.extend(upsert_warnings(
        temp.root(),
        &element_of(PATH_MISSING, "App.Core"),
        &changed(&["path"]),
        false,
    ));
    produced.extend(upsert_warnings(
        temp.root(),
        &element_of(DECLARED_NO_CONTRACT, "App.Core"),
        &changed(&["api"]),
        false,
    ));
    produced.extend(upsert_warnings(
        temp.root(),
        &element_of(NONE_WITHOUT_DOC, "App.Core"),
        &changed(&["api"]),
        false,
    ));
    produced.extend(upsert_warnings(
        temp.root(),
        &element_of(NONE_DOC_ABSENT, "App.Core"),
        &changed(&["doc"]),
        false,
    ));
    // 第 7 条（删除后的孤儿契约）同批纳入词汇面。
    produced.extend(delete_warnings(temp.root(), "App.Core"));

    let vocabulary: Vec<&'static str> = ArchiDriftKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .chain(ArchiApiFindingKind::ALL.iter().map(|kind| kind.as_str()))
        .collect();
    assert_eq!(
        produced
            .iter()
            .map(|warning| warning.kind)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5,
        "五个判据词齐现（七条判据按元素形态取子集）：{produced:?}"
    );
    for warning in &produced {
        assert!(
            vocabulary.contains(&warning.kind),
            "{} 应取自既有枚举的 as_str",
            warning.kind
        );
        let expected = match warning.kind {
            "noPath" | "pathNotFound" => ApiFindingSeverity::Warn.as_str(),
            "docMissing" => ArchiApiFindingKind::DocMissing.severity().as_str(),
            "docNotFound" => ArchiApiFindingKind::DocNotFound.severity().as_str(),
            "apiUnexpected" => ArchiApiFindingKind::ApiUnexpected.severity().as_str(),
            other => panic!("未预期的判据词 {other}"),
        };
        assert_eq!(
            warning.severity, expected,
            "{} 的档位取自单点",
            warning.kind
        );
    }
    assert_eq!(
        produced
            .iter()
            .map(|warning| pair(warning))
            .collect::<std::collections::BTreeSet<_>>(),
        [
            ("noPath", "warn"),
            ("pathNotFound", "warn"),
            ("docMissing", "warn"),
            ("docNotFound", "error"),
            ("apiUnexpected", "warn"),
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>(),
        "五档组合逐值对齐（docNotFound 唯一 error）"
    );
}

/// 边界（矩阵 2 形态）：每条 message 非空、≤ 200 B、不含自身 kind 字面量、含判据句
/// 与元素 FQN；同一元素上「判据命中数 == warnings 条数」（不重不漏）。
#[test]
fn message只写判据句事实与下一步() {
    let temp = TempDir::new("message-shape");
    temp.contract("App.Core");
    let cases: [(&str, &[&str], bool); 3] = [
        (PACKAGE_EMPTY_PATH, &["path"], false),
        (PATH_MISSING, &["path"], false),
        (NONE_DOC_ABSENT, &["doc"], false),
    ];
    for (source, fields, created) in cases {
        let element = element_of(source, "App.Core");
        let warnings = upsert_warnings(temp.root(), &element, &changed(fields), created);
        assert!(!warnings.is_empty(), "{fields:?} 应有判据命中");
        for warning in &warnings {
            assert!(!warning.message.is_empty(), "message 非空");
            assert!(
                warning.message.len() <= 200,
                "message 应 ≤ 200 B，实际 {} B：{}",
                warning.message.len(),
                warning.message
            );
            assert!(
                !warning.message.contains(warning.kind),
                "message 不重复 kind 字面量（{}）：{}",
                warning.kind,
                warning.message
            );
            assert!(
                !warning.message.contains('\n'),
                "message 单行（判据句 + 事实 + 下一步）：{}",
                warning.message
            );
        }
        assert!(
            warnings
                .iter()
                .any(|warning| warning.message.contains("App.Core")),
            "判据句以元素 FQN 起始（不重不漏的定位面）：{warnings:?}"
        );
        let unique: std::collections::BTreeSet<&str> = warnings
            .iter()
            .map(|warning| warning.message.as_str())
            .collect();
        assert_eq!(
            unique.len(),
            warnings.len(),
            "逐条 message 互异（同判据的多条缺席项各自定位，不重不漏）：{warnings:?}"
        );
    }

    // 三判据齐发的形态：条数 == 命中判据数（缺席 `path` + `api = none` 无 `doc` + 契约在盘）。
    let both = concat!(
        "[element.\"App.Core\"]\n",
        "kind = \"module\"\n",
        "api = \"none\"\n",
        "path = [\"src/absent\"]\n",
    );
    let element = element_of(both, "App.Core");
    let warnings = upsert_warnings(temp.root(), &element, &changed(&["path", "api"]), false);
    assert_eq!(
        kinds(&warnings),
        vec!["pathNotFound", "docMissing", "apiUnexpected"],
        "三判据逐条各一条（`noPath` 与 `pathNotFound` 互斥：空 `path` 无缺席项）"
    );
}

// ---------------------------------------------------------------------------
// 删除面（第 7 条）与恒进场
// ---------------------------------------------------------------------------

/// 正向（矩阵 2 形态）：元素派生契约 md 在盘 ⇒ 恰一条 `apiUnexpected` / `warn` 且尾句
/// 为「一并清理」；md 不在盘 ⇒ 零条。
#[test]
fn 删除后孤儿契约报一条() {
    let temp = TempDir::new("delete-orphan");
    temp.contract("App.Core");

    let warnings = delete_warnings(temp.root(), "App.Core");
    assert_eq!(warnings.len(), 1, "只此一条：{warnings:?}");
    assert_eq!(pair(&warnings[0]), ("apiUnexpected", "warn"));
    assert!(
        warnings[0].message.contains("一并清理"),
        "尾句为「一并清理」：{}",
        warnings[0].message
    );

    let empty = TempDir::new("delete-clean");
    assert!(
        delete_warnings(empty.root(), "App.Core").is_empty(),
        "契约 md 不在盘 ⇒ 零条"
    );
}

/// 异常（矩阵 2 形态）：元素已从模型消失（只传 FQN）仍返回；`root` 不存在 / 空模型
/// 目录 ⇒ 零条且不 panic（不读模型）。
#[test]
fn 不读模型恒可调用() {
    let temp = TempDir::new("delete-no-model");
    let absent_root = temp.root().join("no-such-root");
    assert!(
        delete_warnings(&absent_root, "App.Legacy").is_empty(),
        "root 不存在 ⇒ 零条"
    );
    let empty = TempDir::new("delete-empty-model");
    assert!(
        delete_warnings(empty.root(), "App.Legacy").is_empty(),
        "空模型目录 ⇒ 零条"
    );
    // FQN 形态派生不出路径 ⇒ 按「不在盘」处理（零条，不 panic）。
    assert!(
        delete_warnings(temp.root(), "..").is_empty(),
        "非法 FQN 形态 ⇒ 零条"
    );
}

/// 边界（矩阵 2 形态）：删除动作只允许孤儿契约一条——同一 `root` 下同时具备空 `path` /
/// 缺席 `doc` 等事实的元素被删 ⇒ 仍只出第 7 条；`api = none` + 契约 md 在盘 ⇒ 一条
/// （不叠加第 6 条）。
#[test]
fn 删除动作只允许孤儿契约一条() {
    let temp = TempDir::new("delete-only-one");
    temp.contract("App.Core");
    let warnings = delete_warnings(temp.root(), "App.Core");
    assert_eq!(kinds(&warnings), vec!["apiUnexpected"], "只此一条");

    let temp = TempDir::new("delete-only-one-none");
    temp.contract("App.Legacy");
    let warnings = delete_warnings(temp.root(), "App.Legacy");
    assert_eq!(
        kinds(&warnings),
        vec!["apiUnexpected"],
        "api = none 元素也不叠加第 6 条"
    );
}

// ---------------------------------------------------------------------------
// 类型面与源码锚
// ---------------------------------------------------------------------------

/// 边界：`EditWarning` 的字段面封闭（`kind` / `severity` / `message` 三项解构即编译期
/// 判据），`kind` / `severity` 与两枚举 `as_str` / `severity().as_str()` 逐值相等
/// （同一字面量、非新拼串）。
#[test]
#[allow(non_snake_case)]
fn EditWarning零新词表() {
    let temp = TempDir::new("type-shape");
    temp.contract("App.Core");
    let warnings = upsert_warnings(
        temp.root(),
        &element_of(PACKAGE_EMPTY_PATH, "App.Core"),
        &changed(&["path"]),
        false,
    );
    let EditWarning {
        kind,
        severity,
        message,
    } = warnings[0].clone();
    assert_eq!(kind, ArchiDriftKind::NoPath.as_str(), "kind 即枚举返回值");
    assert_eq!(
        severity,
        ApiFindingSeverity::Warn.as_str(),
        "drift 两值恒 warn"
    );
    assert!(message.contains("App.Core"), "message 为 String 事实串");
    assert_eq!(
        warnings[0],
        EditWarning {
            kind: ArchiDriftKind::NoPath.as_str(),
            severity: ApiFindingSeverity::Warn.as_str(),
            message: warnings[0].message.clone(),
        },
        "结构可比（PartialEq 三字段齐）"
    );
}

/// 异常（源码锚）：`warnings.rs` 内零 `drift::check` / `ArchiDriftMode` / `scan::` /
/// `parse_dir`、零 `fs::write` / `fs::rename`（写面 warning 只由零成本事实推导）。
#[test]
fn warnings零drift调用零写盘() {
    let own = include_str!("warnings.rs");
    let code = own
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = code.as_str();
    for forbidden in [
        "drift::check",
        "ArchiDriftMode",
        "scan::",
        "parse_dir",
        "fs::write",
        "fs::rename",
    ] {
        assert!(!source.contains(forbidden), "warnings.rs 内零 {forbidden}");
    }
    assert_eq!(
        source.matches("pub fn upsert_warnings(").count(),
        1,
        "upsert 判据组装恰一处"
    );
    assert_eq!(
        source.matches("pub fn delete_warnings(").count(),
        1,
        "删除面判据恰一处"
    );
}
