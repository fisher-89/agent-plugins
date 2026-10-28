//! `scan`（扫描编排单点）的单元测试：枚举 → 排除 → 归属 → 双解析器抽取 →
//! 跨元素边聚合，`scannedFiles` / `changedFiles` 计数不变量，取消与留痕，以及
//! `inline_ignore` 的行号 / 理由文本边界。
//!
//! 零 mock：真实临时目录 + 真实 `ignore::WalkBuilder` + 真实 `ExcludeSet` /
//! `OwnerIndex` / `TsResolver` / `RustResolver` 组合（编排语义即被测对象）；
//! 取消用真实 `CancelFlag` 原子。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::cancel::CancelFlag;

use crate::drift::report::ArchiDriftMode;
use crate::drift::CheckError;
use crate::dto::{ArchiElement, ElementKind, GateTier, NodeMark};

use super::exclude::DEFAULT_EXCLUDE_RULES;
use super::{inline_ignore, scan, DependencyEdge, ScanInput, ScanOutcome};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-scan-test-{}-{}-{}",
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

    /// 落一个 repo 相对文件（自动建父目录）。
    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    /// 全量字节快照（相对路径 → 字节；用于「零写盘」对拍）。
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut frontier: Vec<PathBuf> = vec![self.0.clone()];
        while let Some(dir) = frontier.pop() {
            let Ok(read) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    frontier.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(&self.0)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                entries.insert(relative, fs::read(&path).expect("读文件失败"));
            }
        }
        entries
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 元素构造器（`path` 原文直传）。
fn element(fqn: &str, gate: GateTier, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/models/01-test.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    }
}

/// 两元素模型（`App.Auth` / `App.Billing`，各持 `./src/auth/` 与 `./src/billing/`）。
fn two_element_model() -> Vec<ArchiElement> {
    vec![
        element("App.Auth", GateTier::Warn, &["./src/auth/"]),
        element("App.Billing", GateTier::Warn, &["./src/billing/"]),
    ]
}

/// 全量模式扫描（无变更集）。
fn scan_full<'a>(
    root: &'a Path,
    elements: &'a [ArchiElement],
    exclude: &'a [String],
) -> Result<ScanOutcome, CheckError> {
    scan(ScanInput {
        root,
        mode: ArchiDriftMode::Full,
        changed: &[],
        exclude,
        elements,
        cancel: CancelFlag::never(),
    })
}

/// 增量模式扫描。
fn scan_incremental<'a>(
    root: &'a Path,
    elements: &'a [ArchiElement],
    exclude: &'a [String],
    changed: &'a [String],
) -> Result<ScanOutcome, CheckError> {
    scan(ScanInput {
        root,
        mode: ArchiDriftMode::Incremental,
        changed,
        exclude,
        elements,
        cancel: CancelFlag::never(),
    })
}

/// 边摘要（`(from, to, file, line, specifier, type_only, inline_ignore)`）。
fn edge_summary(
    edge: &DependencyEdge,
) -> (String, String, String, u32, String, bool, Option<String>) {
    (
        edge.from.clone(),
        edge.to.clone(),
        edge.file.clone(),
        edge.line,
        edge.specifier.clone(),
        edge.type_only,
        edge.inline_ignore.clone(),
    )
}

/// 字符串切片 → 配置 / 变更集向量。
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

// ---------------------------------------------------------------------------
// 正向：全链路单跨元素边
// ---------------------------------------------------------------------------

/// 正向：跨元素单条 import → `edges` 恰 1 条（from / to 按归属单点、file 仓相对
/// `/` 分隔、`line > 0`、specifier 原文、`type_only = false`、`inline_ignore =
/// None`），`matched[]` 两元素且 files 字典序，`unmatched_files` 空。
#[test]
fn 全链路单跨元素边() {
    let temp = TempDir::new("single-edge");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    let elements = two_element_model();
    let outcome = scan_full(temp.root(), &elements, &[]).expect("全量扫描应成功");

    assert_eq!(outcome.edges.len(), 1, "恰一条跨元素边");
    let edge = &outcome.edges[0];
    assert_eq!(edge.from, "App.Billing", "引入方按归属单点");
    assert_eq!(edge.to, "App.Auth", "被引入方按归属单点");
    assert_eq!(edge.file, "src/billing/x.ts", "file 为仓相对 / 分隔");
    assert_eq!(edge.line, 1, "行号为 1 起始且 > 0");
    assert_eq!(edge.specifier, "../auth/token", "specifier 原文");
    assert!(!edge.type_only, "普通 import 非 type-only");
    assert_eq!(edge.inline_ignore, None, "无内联豁免标记");

    assert_eq!(
        outcome
            .report
            .matched
            .iter()
            .map(|entry| (entry.element.as_str(), entry.files.clone()))
            .collect::<Vec<(&str, Vec<String>)>>(),
        vec![
            ("App.Auth", vec!["src/auth/token.ts".to_owned()]),
            ("App.Billing", vec!["src/billing/x.ts".to_owned()]),
        ],
        "matched[] 按元素 FQN 字典序、files 字典序"
    );
    assert!(
        outcome.report.unmatched_files.is_empty(),
        "两文件均有主 ⇒ unmatched 空"
    );
}

/// 边界：同元素两文件互 import → `edges` 空且 `unresolved` 空（同元素内不报是判定
/// 对象边界，非留痕项）。
#[test]
fn 同元素内边剔除() {
    let temp = TempDir::new("same-element");
    temp.write(
        "src/auth/token.ts",
        "import { helper } from './helper';\nexport const token = 1;\n",
    );
    temp.write(
        "src/auth/helper.ts",
        "import { token } from './token';\nexport const helper = 2;\n",
    );
    let elements = vec![element("App.Auth", GateTier::Warn, &["./src/auth/"])];
    let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
    assert!(outcome.edges.is_empty(), "同元素内边不进判定面");
    assert!(
        outcome.report.unresolved.is_empty(),
        "同元素内边不是留痕项：{:?}",
        outcome.report.unresolved
    );
}

/// 正向：同一 `(from, to)` 在 3 文件各 1 处 → `edges` 3 行（`scan` 只出事实，按元素
/// 对聚合归 `drift`）。
#[test]
fn 逐文件一行不聚合() {
    let temp = TempDir::new("no-aggregation");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    for index in 0..3 {
        temp.write(
            &format!("src/billing/f{index}.ts"),
            "import { token } from '../auth/token';\n",
        );
    }
    let elements = two_element_model();
    let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
    assert_eq!(
        outcome.edges.len(),
        3,
        "scan 逐文件逐条出事实，不按元素对聚合"
    );
    let mut files: Vec<&str> = outcome
        .edges
        .iter()
        .map(|edge| edge.file.as_str())
        .collect();
    files.sort();
    assert_eq!(
        files,
        vec![
            "src/billing/f0.ts",
            "src/billing/f1.ts",
            "src/billing/f2.ts"
        ],
        "三条边各来自一个文件"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界：排除面
// ---------------------------------------------------------------------------

/// 正向（AC-1）：七类测试文件形态各被排除并逐条留痕（`rule` 为命中的具体
/// glob），且跨元素 import 不产边。
#[test]
fn 测试文件排除留痕七形态() {
    let cases = [
        ("src/billing/a.test.ts", "**/*.test.ts"),
        ("src/billing/a.spec.tsx", "**/*.spec.tsx"),
        ("src/billing/__tests__/a.ts", "**/__tests__/**"),
        ("crates/x/src/a_test.rs", "**/*_test.rs"),
        ("src/billing/tests/a.ts", "**/tests/**"),
        ("src/billing/fixtures/a.ts", "**/fixtures/**"),
        ("src/billing/__mocks__/a.ts", "**/__mocks__/**"),
    ];
    for (probe, rule) in cases {
        let temp = TempDir::new("excluded");
        temp.write("src/auth/token.ts", "export const token = 1;\n");
        temp.write(probe, "import { token } from '../auth/token';\n");
        let elements = two_element_model();
        let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
        assert!(
            outcome.edges.is_empty(),
            "{probe} 被排除后不产边：{:?}",
            outcome.edges
        );
        assert_eq!(
            outcome
                .report
                .excluded
                .iter()
                .map(|entry| (entry.file.as_str(), entry.rule.as_str()))
                .collect::<Vec<(&str, &str)>>(),
            vec![(probe, rule)],
            "{probe} 的排除留痕逐条在案"
        );
        assert_eq!(
            outcome.report.scanned_files, 1,
            "可判源文件仅 token.ts（被排除者不计入）"
        );
    }
}

/// 边界（AC-1）：配置 `exclude` 追加而非替换——`src/legacy/**` 与默认规则各命中
/// 一条，`rule` 各异。
#[test]
fn 配置追加不替换() {
    let temp = TempDir::new("append");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/legacy/old.ts",
        "import { token } from '../auth/token';\n",
    );
    temp.write(
        "src/billing/a.test.ts",
        "import { token } from '../auth/token';\n",
    );
    let elements = two_element_model();
    let exclude = strings(&["src/legacy/**"]);
    let outcome = scan_full(temp.root(), &elements, &exclude).expect("扫描应成功");
    let mut excluded: Vec<(String, String)> = outcome
        .report
        .excluded
        .iter()
        .map(|entry| (entry.file.clone(), entry.rule.clone()))
        .collect();
    excluded.sort();
    assert_eq!(
        excluded,
        vec![
            (
                "src/billing/a.test.ts".to_owned(),
                "**/*.test.ts".to_owned()
            ),
            ("src/legacy/old.ts".to_owned(), "src/legacy/**".to_owned()),
        ],
        "配置规则与默认规则各命中一条（追加，非替换）"
    );
    assert!(outcome.edges.is_empty(), "两文件均被排除 ⇒ 零边");
}

/// 边界（AC-1）：工作区 `openspec/config.json` 的 `tests[].excludes` 对扫描面零
/// 影响（前后两次扫描逐值相同）。
#[test]
fn 工作区测试scope零耦合() {
    let temp = TempDir::new("workspace-config");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    let elements = two_element_model();
    let before = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");

    temp.write(
        "openspec/config.json",
        "{\n  \"tests\": [{ \"excludes\": [\"src/\"] }]\n}\n",
    );
    let after = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");

    assert_eq!(
        before.report.scanned_files, after.report.scanned_files,
        "工作区测试 scope 变化不得改变扫描面"
    );
    assert_eq!(
        before.report.excluded, after.report.excluded,
        "排除留痕逐值不变"
    );
    assert_eq!(
        before.edges.len(),
        after.edges.len(),
        "跨元素边逐值不变（scan 零读工作区配置）"
    );
}

// ---------------------------------------------------------------------------
// 边界：计数不变量
// ---------------------------------------------------------------------------

/// 边界（D22）：`scanned_files == 可判源文件数 − excluded.len()` 恒成立（零可判 /
/// 全部被排除 / 单文件三形态），`unresolved` 不计入计数。
#[test]
fn scanned_files不变量() {
    // 形态一：零可判源文件。
    let empty = TempDir::new("no-source");
    empty.write("README.md", "# 无源文件\n");
    let elements = two_element_model();
    let outcome = scan_full(empty.root(), &elements, &[]).expect("扫描应成功");
    assert_eq!(outcome.report.scanned_files, 0, "零源文件");
    assert_eq!(outcome.report.excluded.len(), 0, "零排除");

    // 形态二：全部被排除。
    let all_excluded = TempDir::new("all-excluded");
    all_excluded.write("src/auth/a.test.ts", "// 测试\n");
    all_excluded.write("src/auth/b.spec.ts", "// 测试\n");
    let outcome = scan_full(all_excluded.root(), &elements, &[]).expect("扫描应成功");
    assert_eq!(outcome.report.scanned_files, 0, "全部被排除 ⇒ 零可判");
    assert_eq!(outcome.report.excluded.len(), 2, "两条排除留痕");
    assert_eq!(
        outcome.report.scanned_files as usize,
        2 - outcome.report.excluded.len(),
        "不变量：可判数 = 候选数 − 排除数"
    );

    // 形态三：单文件 + 一条不可判定留痕（unresolved 不计入 scanned_files）。
    let single = TempDir::new("single-file");
    single.write(
        "src/auth/x.ts",
        "import('./dynamic');\nexport const x = 1;\n",
    );
    let outcome = scan_full(single.root(), &elements, &[]).expect("扫描应成功");
    assert_eq!(outcome.report.scanned_files, 1, "单文件可判");
    assert_eq!(outcome.report.unresolved.len(), 1, "动态导入一条留痕");
    assert_eq!(
        outcome.report.scanned_files, 1,
        "unresolved 不计入 scanned_files"
    );
}

/// 边界（D22）：`changedFiles` 模式分流——全量恒 0；增量零变更集 0；非源扩展名
/// 不计入；测试文件计入但被排除。
#[test]
fn changed_files模式分流() {
    let temp = TempDir::new("changed-files");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write("src/auth/a.test.ts", "// 测试\n");
    let elements = two_element_model();

    let full = scan_full(temp.root(), &elements, &[]).expect("全量扫描应成功");
    assert_eq!(full.report.changed_files, 0, "全量模式恒 0");

    let empty_changed = scan_incremental(temp.root(), &elements, &[], &[]).expect("扫描应成功");
    assert_eq!(empty_changed.report.changed_files, 0, "零变更集 ⇒ 0");
    assert_eq!(
        empty_changed.report.scanned_files, 0,
        "零变更集不扫（incremental 只留变更集内文件）"
    );

    let doc_only = scan_incremental(
        temp.root(),
        &elements,
        &[],
        &strings(&["README.md", "package.json"]),
    )
    .expect("扫描应成功");
    assert_eq!(
        doc_only.report.changed_files, 0,
        "非源扩展名不计入 changed_files"
    );

    let with_test = scan_incremental(
        temp.root(),
        &elements,
        &[],
        &strings(&["src/auth/a.test.ts"]),
    )
    .expect("扫描应成功");
    assert_eq!(
        with_test.report.changed_files, 1,
        "测试文件计入 changed_files"
    );
    assert_eq!(
        with_test.report.excluded.len(),
        1,
        "但进 excluded[]（排除在归属之前）"
    );
    assert_eq!(with_test.report.scanned_files, 0, "被排除 ⇒ 零可判");
}

/// 正向：增量只扫变更集——未变更的跨元素文件不产边。
#[test]
fn 增量模式只扫变更集() {
    let temp = TempDir::new("incremental-only");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/changed.ts",
        "import { token } from '../auth/token';\n",
    );
    temp.write(
        "src/billing/unchanged.ts",
        "import { token } from '../auth/token';\n",
    );
    let elements = two_element_model();
    let changed = strings(&["src/billing/changed.ts"]);
    let outcome = scan_incremental(temp.root(), &elements, &[], &changed).expect("扫描应成功");
    assert_eq!(outcome.edges.len(), 1, "只出变更集的边");
    assert_eq!(
        outcome.edges[0].file, "src/billing/changed.ts",
        "edges[].file 必须落在变更集内"
    );
    assert_eq!(outcome.report.scanned_files, 1, "只扫变更集内文件");

    let no_change = scan_incremental(temp.root(), &elements, &[], &[]).expect("扫描应成功");
    assert!(no_change.edges.is_empty(), "零变更集 ⇒ 零边");
}

/// 边界：`changed` 1000 条（多数不存在）→ 不 panic 且计数只数源扩展名条目。
#[test]
fn 大列表边界() {
    let temp = TempDir::new("large-changed");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let elements = two_element_model();
    let mut changed: Vec<String> = Vec::new();
    for index in 0..1000 {
        changed.push(format!("src/missing/f{index}.ts"));
    }
    let outcome = scan_incremental(temp.root(), &elements, &[], &changed).expect("不 panic");
    assert_eq!(
        outcome.report.changed_files, 1000,
        "1000 条源扩展名条目全部计入 changed_files"
    );
    assert_eq!(outcome.report.scanned_files, 0, "均不存在 ⇒ 零可判");
}

// ---------------------------------------------------------------------------
// 异常：取消与留痕
// ---------------------------------------------------------------------------

/// 异常：`cancel` 预置位 → `Err(CheckError::Stopped)`（零部分结果）。
#[test]
#[allow(non_snake_case)]
fn 取消返回Stopped() {
    let temp = TempDir::new("cancel");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let elements = two_element_model();
    let cancel = CancelFlag::never();
    cancel.handle().set();
    let outcome = scan(ScanInput {
        root: temp.root(),
        mode: ArchiDriftMode::Full,
        changed: &[],
        exclude: &[],
        elements: &elements,
        cancel,
    });
    match outcome {
        Err(error) => assert_eq!(error, CheckError::Stopped, "取消必须是 Stopped"),
        Ok(_) => panic!("取消置位后不应返回部分结果"),
    }
}

/// 边界（D6）：「源文件归 `off` 元素 → 计 `unmatched_files[]`、零违例，但依赖
/// 照常抽取」——抽取是事实面、定级才是结论面；`off` MUST NOT 让事实面消失（否则
/// gate 全 `off` 的工程会把全部声明边误判成「声明未兑现」）。
#[test]
fn off元素覆盖源文件仍抽取依赖() {
    let temp = TempDir::new("off-source");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/legacy/x.ts",
        "import { token } from '../auth/token';\n",
    );
    let elements = vec![
        element("App.Auth", GateTier::Strict, &["./src/auth/"]),
        element("App.Legacy", GateTier::Off, &["./src/legacy/"]),
    ];
    let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
    assert_eq!(
        outcome
            .report
            .unmatched_files
            .iter()
            .filter(|file| file.as_str() == "src/legacy/x.ts")
            .count(),
        1,
        "off 覆盖的文件计入 unmatched：{:?}",
        outcome.report.unmatched_files
    );
    assert!(
        !outcome
            .report
            .matched
            .iter()
            .any(|entry| entry.element == "App.Legacy"),
        "off 元素不进 matched[]（不参与定级）"
    );
    assert_eq!(outcome.edges.len(), 1, "依赖照常抽取成边");
    assert_eq!(
        edge_summary(&outcome.edges[0]),
        (
            "App.Legacy".to_owned(),
            "App.Auth".to_owned(),
            "src/legacy/x.ts".to_owned(),
            1,
            "../auth/token".to_owned(),
            false,
            None,
        ),
        "off 源文件的边事实完整（定级由 drift 只看引入方决定）"
    );
}

/// 异常（AC-2 交汇）：同长多命中的文件 → 进 `unmatched_files`、不进 `matched[]`、
/// 零边，并产一条 `ambiguousOwner` 事实留痕（不任选）。
#[test]
fn 归属不确定留痕() {
    let temp = TempDir::new("ambiguous");
    temp.write("src/aa/x.ts", "import { token } from '../auth/token';\n");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let elements = vec![
        element("App.Zeta", GateTier::Warn, &["src/aa/"]),
        element("App.Alpha", GateTier::Warn, &["src/aa/"]),
        element("App.Auth", GateTier::Warn, &["src/auth/"]),
    ];
    let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
    assert!(
        outcome
            .report
            .unmatched_files
            .contains(&"src/aa/x.ts".to_owned()),
        "归属不确定的文件计入 unmatched：{:?}",
        outcome.report.unmatched_files
    );
    assert!(
        !outcome
            .report
            .matched
            .iter()
            .any(|entry| entry.files.contains(&"src/aa/x.ts".to_owned())),
        "不任选 ⇒ 不进 matched[]"
    );
    assert!(outcome.edges.is_empty(), "归属不确定不产边");
    assert_eq!(outcome.findings.len(), 1, "一条 ambiguousOwner 事实留痕");
    assert_eq!(
        outcome.findings[0].kind,
        crate::drift::ArchiDriftKind::AmbiguousOwner,
        "留痕 kind 为 ambiguousOwner"
    );
}

/// 异常（AC-2 交汇）：模型含无 `path` 元素 → 该元素不进 `matched[]`，零归属副作用
/// （`noPath` 条目由 `drift` 装配）。
#[test]
fn path缺失元素不进matched() {
    let temp = TempDir::new("no-path");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let elements = vec![
        element("App.Auth", GateTier::Warn, &["./src/auth/"]),
        element("App.NoPath", GateTier::Warn, &[]),
    ];
    let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
    assert_eq!(
        outcome.report.matched.len(),
        1,
        "只有声明 path 的元素可归属"
    );
    assert_eq!(
        outcome.report.matched[0].element, "App.Auth",
        "归属到持 path 者"
    );
    assert!(
        !outcome
            .report
            .matched
            .iter()
            .any(|entry| entry.element == "App.NoPath"),
        "无 path 元素不进 matched[]"
    );
    assert!(
        !outcome
            .report
            .unmatched_files
            .contains(&"src/auth/token.ts".to_owned()),
        "无 path 元素不产生归属副作用"
    );
}

/// 异常（AC-3）：动态 `import()` / `require()` 逐条进 `unresolved`；不可落盘的
/// specifier 静默丢边（不冒充留痕、不产边）。
#[test]
fn 未解析留痕三形态() {
    let temp = TempDir::new("unresolved");
    temp.write(
        "src/auth/dynamic.ts",
        "import('../billing/lazy');\nexport const dynamic = 1;\n",
    );
    temp.write(
        "src/auth/common.ts",
        "require('../billing/lazy');\nexport const common = 1;\n",
    );
    temp.write(
        "src/auth/missing.ts",
        "import { lazy } from '../billing/nowhere';\nexport const missing = 1;\n",
    );
    temp.write("src/billing/lazy.ts", "export const lazy = 1;\n");
    let elements = two_element_model();
    let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");

    let files: Vec<&str> = outcome
        .report
        .unresolved
        .iter()
        .map(|entry| entry.file.as_str())
        .collect();
    assert_eq!(
        files,
        vec!["src/auth/common.ts", "src/auth/dynamic.ts"],
        "动态导入与 require 两形态逐条留痕（按文件字典序）"
    );
    assert!(
        outcome
            .report
            .unresolved
            .iter()
            .all(|entry| !entry.message.is_empty()),
        "留痕文案非空"
    );
    assert!(
        outcome.edges.is_empty(),
        "不可落盘 specifier 静默丢边（不冒充违例、不留痕）：{:?}",
        outcome.edges
    );
}

/// 异常：`changed` 指向不存在的文件 → 不 panic，不留痕（既不进 `unresolved` 也不进
/// `excluded`），计数不变量仍成立。
#[test]
fn 源文件读盘失败不panic() {
    let temp = TempDir::new("missing-source");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let elements = two_element_model();
    let changed = strings(&["src/auth/deleted.ts"]);
    let outcome = scan_incremental(temp.root(), &elements, &[], &changed).expect("不 panic");
    assert!(
        outcome.report.unresolved.is_empty(),
        "不存在的变更项不留痕：{:?}",
        outcome.report.unresolved
    );
    assert!(
        outcome.report.excluded.is_empty(),
        "不存在的变更项不进 excluded"
    );
    assert_eq!(outcome.report.scanned_files, 0, "不变量仍成立");
    assert_eq!(outcome.report.changed_files, 1, "变更集计数照实");
}

/// 边界：扫描前后 tempdir 全量字节快照相等（`archi` 只读盘纪律的装置化断言）。
#[test]
fn 零写盘() {
    let temp = TempDir::new("no-write");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    temp.write("src/auth/broken.ts", "const x = ((((\n");
    let elements = two_element_model();
    let before = temp.snapshot();
    let _ = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
    let after = temp.snapshot();
    assert_eq!(before, after, "扫描前后字节快照必须相等（零写盘）");
}

// ---------------------------------------------------------------------------
// inline_ignore：行位置与理由文本
// ---------------------------------------------------------------------------

/// 正向：注释行自身作为 `line` 传入 → `Some(reason)`。
#[test]
fn inline_ignore同行命中() {
    let source = "const a = 1;\nconst b = 2;\nconst c = 3;\nconst d = 4;\n// archi-ignore-next-line: 迁移期\nimport { token } from './token';\n";
    assert_eq!(
        inline_ignore(source, 5),
        Some("迁移期".to_owned()),
        "注释行自身即命中（取该行与上一行）"
    );
}

/// 正向：注释写在 import 的上一行 → 同样 `Some(reason)`（与「同行」同权）。
#[test]
fn inline_ignore上一行命中() {
    let source = "const a = 1;\nconst b = 2;\nconst c = 3;\n// archi-ignore-next-line: 迁移期\nimport { token } from './token';\n";
    assert_eq!(
        inline_ignore(source, 5),
        Some("迁移期".to_owned()),
        "第 5 行 import 的上一行（第 4 行）注释生效"
    );
}

/// 边界：理由文本形态——空理由 / 纯空白 / 无冒号 / 大小写变体 → 全 `None`；
/// 含 `\0` / emoji / >1000 chars 的理由原样返回；`\r\n` 行尾仍命中。
#[test]
fn inline_ignore理由文本边界() {
    let empty_reason = "// archi-ignore-next-line:\nimport a from './a';\n";
    assert_eq!(inline_ignore(empty_reason, 2), None, "空理由视为未标记");

    let blank_reason = "// archi-ignore-next-line:    \nimport a from './a';\n";
    assert_eq!(inline_ignore(blank_reason, 2), None, "纯空白理由视为未标记");

    let no_colon = "// archi-ignore-next-line\nimport a from './a';\n";
    assert_eq!(inline_ignore(no_colon, 2), None, "缺冒号不命中");

    let case_variant = "// Archi-Ignore-Next-Line: x\nimport a from './a';\n";
    assert_eq!(
        inline_ignore(case_variant, 2),
        None,
        "词面精确（大小写敏感）"
    );

    let nul_reason = "// archi-ignore-next-line: 含\u{0}空字符\nimport a from './a';\n";
    assert_eq!(
        inline_ignore(nul_reason, 2),
        Some("含\u{0}空字符".to_owned()),
        "理由含 NUL 原样返回"
    );

    let emoji_reason = "// archi-ignore-next-line: 迁移期🎯\nimport a from './a';\n";
    assert_eq!(
        inline_ignore(emoji_reason, 2),
        Some("迁移期🎯".to_owned()),
        "理由含 emoji 原样返回"
    );

    let long_reason = format!(
        "// archi-ignore-next-line: {}\nimport a from './a';\n",
        "长".repeat(1200)
    );
    assert_eq!(
        inline_ignore(&long_reason, 2),
        Some("长".repeat(1200)),
        ">1000 chars 的理由原样返回（不截断）"
    );

    let crlf = "// archi-ignore-next-line: 迁移期\r\nimport a from './a';\r\n";
    assert_eq!(
        inline_ignore(crlf, 2),
        Some("迁移期".to_owned()),
        "\\r\\n 行尾仍命中（trim 掉 CR）"
    );
}

/// 边界：行号边界——`line = 1` 只看该行；`line = 0` 位置未知 → `None`；越界 →
/// `None`；空源 → `None`。
#[test]
fn inline_ignore行号边界() {
    let source = "import a from './a';\nconst b = 1;\n";
    assert_eq!(
        inline_ignore(source, 1),
        None,
        "首行无注释 ⇒ None（只看该行）"
    );
    assert_eq!(inline_ignore(source, 0), None, "line = 0 = 位置未知");
    assert_eq!(
        inline_ignore(source, 99),
        None,
        "行号超总行数 ⇒ 越界即 None"
    );
    assert_eq!(inline_ignore("", 1), None, "空源 ⇒ None");

    let first_line_comment = "// archi-ignore-next-line: 首行\nimport a from './a';\n";
    assert_eq!(
        inline_ignore(first_line_comment, 1),
        Some("首行".to_owned()),
        "line = 1 时该行自身可命中"
    );
}

// ---------------------------------------------------------------------------
// 装配面回归：边摘要形状与默认排除常量同源
// ---------------------------------------------------------------------------

/// 边界：`edge_summary` 逐字段取到实值（防字段语义漂移），默认排除常量非空。
#[test]
fn 边字段面与默认排除常量() {
    let temp = TempDir::new("edge-shape");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import type { Token } from '../auth/token';\n",
    );
    let elements = two_element_model();
    let outcome = scan_full(temp.root(), &elements, &[]).expect("扫描应成功");
    assert_eq!(outcome.edges.len(), 1, "type-only import 同为一条事实");
    assert_eq!(
        edge_summary(&outcome.edges[0]),
        (
            "App.Billing".to_owned(),
            "App.Auth".to_owned(),
            "src/billing/x.ts".to_owned(),
            1,
            "../auth/token".to_owned(),
            true,
            None,
        ),
        "type-only 只标注 `type_only`，specifier / 行号 / 豁免面逐字段在案"
    );
    assert_eq!(DEFAULT_EXCLUDE_RULES.len(), 13, "默认排除常量单点");
}
