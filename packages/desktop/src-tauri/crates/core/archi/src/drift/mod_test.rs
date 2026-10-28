//! `drift`（判定单点）的单元测试：模型读取 → 看护配置 → 扫描 → 对账 → 定级
//! （只看引入方）→ 豁免裁决（声明 > 配置 > 内联）→ `occurrences` 聚合 →
//! `status` 推导 → 报告装配，以及 `CheckError::message` 的三值文案。
//!
//! 零 mock：真实 tempdir 落模型 / 源文件 / `config.json`（判定链路的输入事实即被测
//! 语义），真实 `dto::graph_view` 与真实两套解析器；时钟与摘要只断言形态与变化性。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::cancel::CancelFlag;
use foundation::layout::archi_config_path;

use crate::dto;
use crate::scan::{self, ScanInput};

use super::report::{ArchiCheckStatus, ArchiDriftKind, ArchiObservationKind, ArchiSuppressVia};
use super::{check, CheckError, CheckInput, Report};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-drift-test-{}-{}-{}",
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

    /// 落模型（`.dev-team/architecture/models/01-test.toml`）。
    fn write_model(&self, content: &str) {
        self.write(".dev-team/architecture/models/01-test.toml", content);
    }

    /// 落架构域看护配置（唯一合法位置）。
    fn write_config(&self, content: &str) {
        let path = archi_config_path(&self.0);
        fs::create_dir_all(path.parent().expect("父目录")).expect("创建架构域目录失败");
        fs::write(&path, content).expect("写配置失败");
    }

    /// 全量字节快照（相对路径 → 字节；用于「零写盘」对拍）。
    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
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
                entries.push((relative, fs::read(&path).expect("读文件失败")));
            }
        }
        entries.sort();
        entries
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 判定驱动与夹具
// ---------------------------------------------------------------------------

/// 全量判定（`changed` 空、无 change 上下文）。
fn run_full(root: &Path) -> Result<Report, CheckError> {
    check(CheckInput {
        root,
        mode: super::ArchiDriftMode::Full,
        changed: &[],
        change: None,
        cancel: CancelFlag::never(),
    })
}

/// 增量判定。
fn run_incremental(root: &Path, changed: &[String]) -> Result<Report, CheckError> {
    check(CheckInput {
        root,
        mode: super::ArchiDriftMode::Incremental,
        changed,
        change: Some("archi-check-and-gate"),
        cancel: CancelFlag::never(),
    })
}

/// 字符串切片 → 向量。
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// 两元素模型（`strict` 引入方 + `warn` 被引入方），两目录均落盘。
fn strict_over_auth(temp: &TempDir) {
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n",
    );
}

/// 三文件各一处跨元素 import 的夹具（聚合用例）。
fn three_occurrence_fixture(temp: &TempDir) {
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    for index in 0..3 {
        temp.write(
            &format!("src/billing/f{index}.ts"),
            "import { token } from '../auth/token';\n",
        );
    }
}

/// 元素对集合（`(from, to)`，按序）。
fn pairs(entries: &[super::ArchiDriftEntry]) -> Vec<(String, String)> {
    entries
        .iter()
        .filter_map(|entry| match (&entry.from, &entry.to) {
            (Some(from), Some(to)) => Some((from.clone(), to.clone())),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 边界：无模型 / 空模型
// ---------------------------------------------------------------------------

/// 边界（D3）：`<archi_dir>/models` 缺席 → `skipped`，两数组全空。
#[test]
fn 无模型返回skipped() {
    let temp = TempDir::new("no-model");
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("无模型不是错误");
    assert_eq!(report.status, ArchiCheckStatus::Skipped, "无模型 ⇒ skipped");
    assert!(report.errors.is_empty(), "零 errors");
    assert!(report.warnings.is_empty(), "零 warnings");
    assert!(report.model.structural_errors.is_empty(), "零结构错");
    assert_eq!(report.scan.scanned_files, 0, "不扫描");
}

/// 边界（D3）：模型目录在但零 `*.toml`（只有 README）→ `skipped`。
#[test]
fn 模型目录空返回skipped() {
    let temp = TempDir::new("empty-models-dir");
    temp.write(".dev-team/architecture/models/README.md", "# 模型说明\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("零 toml 不是错误");
    assert_eq!(
        report.status,
        ArchiCheckStatus::Skipped,
        "零个 *.toml ⇒ skipped（非空模型）"
    );
}

/// 正向：模型在但零元素（空 toml 语料）/ 单元素零关系 → `clean`（冷启动合法）。
#[test]
fn 空模型与单元素模型clean() {
    let empty_model = TempDir::new("empty-model");
    empty_model.write_model("# 空模型（只有注释）\n");
    empty_model.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(empty_model.root()).expect("空模型合法");
    assert_eq!(report.status, ArchiCheckStatus::Clean, "空模型 ⇒ clean");
    assert_eq!(report.model.elements, 0, "零元素");
    assert!(report.model.structural_errors.is_empty(), "零结构错");

    let single = TempDir::new("single-element");
    single.write_model("[element.\"Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n");
    single.write("src/auth/token.ts", "export const token = 1;\n");
    let report = run_full(single.root()).expect("单元素合法");
    assert_eq!(report.status, ArchiCheckStatus::Clean, "零违例 ⇒ clean");
    assert_eq!(report.model.elements, 1, "一个元素");
    assert_eq!(report.model.relations, 0, "零关系");
}

// ---------------------------------------------------------------------------
// 正向：三档定级
// ---------------------------------------------------------------------------

/// 正向：`warn` 档元素引入未声明依赖 → `warnings` + 一条 `undeclaredDependency`。
#[test]
fn warn档产出warnings() {
    let temp = TempDir::new("warn");
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        report.status,
        ArchiCheckStatus::Warnings,
        "warn 档 ⇒ warnings"
    );
    assert!(report.errors.is_empty(), "零 errors");
    assert_eq!(report.warnings.len(), 1, "一条提示");
    assert_eq!(
        report.warnings[0].kind,
        ArchiDriftKind::UndeclaredDependency,
        "kind 为未声明依赖"
    );
}

/// 正向（AC-4）：`strict` 档 → `errors` + 一条条目（字段面逐值）。
#[test]
fn strict档产出errors() {
    let temp = TempDir::new("strict");
    strict_over_auth(&temp);
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        report.status,
        ArchiCheckStatus::Errors,
        "strict 档 ⇒ errors"
    );
    assert_eq!(report.errors.len(), 1, "一条违例");
    let entry = &report.errors[0];
    assert_eq!(entry.kind, ArchiDriftKind::UndeclaredDependency, "kind");
    assert_eq!(entry.from.as_deref(), Some("Billing"), "from 为引入方");
    assert_eq!(entry.to.as_deref(), Some("Auth"), "to 为目标");
    assert_eq!(entry.gate, Some(dto::GateTier::Strict), "gate 为引入方档位");
    assert_eq!(entry.occurrences.len(), 1, "一处命中");
}

/// 正向（AC-8）：五值全覆盖且互斥——五种构造分别命中 `skipped` / `noChanges` /
/// `clean` / `warnings` / `errors`，`ALL` 中每个值都可达。
#[test]
fn 状态五值全覆盖() {
    let mut reached: Vec<ArchiCheckStatus> = Vec::new();

    let skipped = TempDir::new("v-skipped");
    skipped.write("src/a.ts", "export const a = 1;\n");
    reached.push(run_full(skipped.root()).expect("skipped 构造").status);

    let no_changes = TempDir::new("v-no-changes");
    no_changes.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    no_changes.write("src/a.ts", "export const a = 1;\n");
    reached.push(
        run_incremental(no_changes.root(), &[])
            .expect("noChanges 构造")
            .status,
    );

    let clean = TempDir::new("v-clean");
    clean.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    clean.write("src/a.ts", "export const a = 1;\n");
    reached.push(run_full(clean.root()).expect("clean 构造").status);

    let warnings = TempDir::new("v-warnings");
    warnings.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n",
    );
    warnings.write("src/auth/token.ts", "export const token = 1;\n");
    warnings.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    reached.push(run_full(warnings.root()).expect("warnings 构造").status);

    let errors = TempDir::new("v-errors");
    strict_over_auth(&errors);
    errors.write("src/auth/token.ts", "export const token = 1;\n");
    errors.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    reached.push(run_full(errors.root()).expect("errors 构造").status);

    assert_eq!(reached.len(), 5, "五种构造各一：{reached:?}");
    for status in ArchiCheckStatus::ALL {
        assert_eq!(
            reached.iter().filter(|hit| **hit == status).count(),
            1,
            "{status:?} 必须恰由一个构造命中（五值互斥、无死值）"
        );
    }
}

// ---------------------------------------------------------------------------
// 异常 / 边界：推导序与结构错
// ---------------------------------------------------------------------------

/// 异常（D10）：坏模型（`dependsOnAncestor`）+ 全部源文件被排除 → `errors`（不是
/// `noChanges`）。
#[test]
#[allow(non_snake_case)]
fn 结构错先于noChanges() {
    let temp = TempDir::new("structural-first");
    temp.write_model(
        "[element.\"Root\"]\nkind = \"package\"\npath = [\"./src/\"]\n\n[element.\"Root.Child\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n\n[[relation]]\nfrom = \"Root.Child\"\nto = \"Root\"\n",
    );
    temp.write("src/auth/a.test.ts", "// 测试\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        report.status,
        ArchiCheckStatus::Errors,
        "结构错必须先于 noChanges 判定"
    );
    assert_eq!(report.scan.scanned_files, 0, "确实无可判文件");
}

/// 边界（D10）：同一次检查同时命中 strict 与 warn 违例 → `errors`，两数组各非空。
#[test]
fn errors先于warnings() {
    let temp = TempDir::new("errors-first");
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n\n[element.\"Search\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/search/\"]\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    temp.write(
        "src/search/y.ts",
        "import { token } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.status, ArchiCheckStatus::Errors, "errors 优先");
    assert_eq!(report.errors.len(), 1, "一条 strict 违例");
    assert_eq!(report.warnings.len(), 1, "一条 warn 提示");
}

/// 边界（AC-7）：增量零变更集 → `noChanges` + `scanned_files == 0`；全量零可判文件
/// → 同样 `noChanges`。
#[test]
#[allow(non_snake_case)]
fn noChanges判据() {
    let temp = TempDir::new("no-changes");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    temp.write("src/a.ts", "export const a = 1;\n");

    let incremental = run_incremental(temp.root(), &[]).expect("判定应成功");
    assert_eq!(
        incremental.status,
        ArchiCheckStatus::NoChanges,
        "零变更集 ⇒ noChanges"
    );
    assert_eq!(incremental.scan.scanned_files, 0, "零可判文件");

    let all_excluded = TempDir::new("no-changes-full");
    all_excluded.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    all_excluded.write("src/a.test.ts", "// 测试\n");
    let full = run_full(all_excluded.root()).expect("判定应成功");
    assert_eq!(full.status, ArchiCheckStatus::NoChanges, "全量零可判同判");
}

/// 边界（D11）：结构错不进 `errors[]`——`model.structural_errors` 非空、
/// `errors[]` 空、`status == errors`。
#[test]
fn 结构错不进errors数组() {
    let temp = TempDir::new("structural-only");
    temp.write_model(
        "[element.\"Root\"]\nkind = \"package\"\npath = [\"./src/\"]\n\n[element.\"Root.Child\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n\n[[relation]]\nfrom = \"Root.Child\"\nto = \"Root\"\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.status, ArchiCheckStatus::Errors, "结构错 ⇒ errors");
    assert!(
        !report.model.structural_errors.is_empty(),
        "结构错在 model 段"
    );
    assert!(report.errors.is_empty(), "结构错不进 errors[]");
    let diagnostic = &report.model.structural_errors[0];
    assert_eq!(
        diagnostic.kind,
        dto::DiagnosticKind::DependsOnAncestor,
        "kind 复用 dto 词汇"
    );
    assert!(!diagnostic.file.is_empty(), "结构错带 file");
}

// ---------------------------------------------------------------------------
// 正向：定级只看引入方
// ---------------------------------------------------------------------------

/// 正向（AC-4）：同一 `A 引入 B` 在引入方 strict / warn / off 三档下分别出 error /
/// warning / 零记录。
#[test]
fn 定级只看引入方三档() {
    for (gate, expect_errors, expect_warnings) in
        [("strict", 1usize, 0usize), ("warn", 0, 1), ("off", 0, 0)]
    {
        let temp = TempDir::new("grading-from");
        temp.write_model(&format!(
            "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"{gate}\"\npath = [\"./src/billing/\"]\n"
        ));
        temp.write("src/auth/token.ts", "export const token = 1;\n");
        temp.write(
            "src/billing/x.ts",
            "import { token } from '../auth/token';\n",
        );
        let report = run_full(temp.root()).expect("判定应成功");
        assert_eq!(
            report.errors.len(),
            expect_errors,
            "引入方 gate={gate} 的 errors 数"
        );
        assert_eq!(
            report.warnings.len(),
            expect_warnings,
            "引入方 gate={gate} 的 warnings 数"
        );
        if gate == "off" {
            assert!(
                report
                    .scan
                    .unmatched_files
                    .contains(&"src/billing/x.ts".to_owned()),
                "off 档文件计入 unmatched_files"
            );
        }
    }
}

/// 正确性（AC-4）：目标元素档位三档替换 → 结论不变（只看引入方）。
#[test]
fn 目标档位不影响结论() {
    for gate in ["strict", "warn", "off"] {
        let temp = TempDir::new("grading-to");
        temp.write_model(&format!(
            "[element.\"Auth\"]\nkind = \"module\"\ngate = \"{gate}\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n"
        ));
        temp.write("src/auth/token.ts", "export const token = 1;\n");
        temp.write(
            "src/billing/x.ts",
            "import { token } from '../auth/token';\n",
        );
        let report = run_full(temp.root()).expect("判定应成功");
        assert_eq!(
            report.errors.len(),
            1,
            "目标档 gate={gate} 时结论不变（仍 1 条 errors）"
        );
        assert_eq!(
            report.errors[0].gate,
            Some(dto::GateTier::Strict),
            "gate 字段恒为引入方档位"
        );
    }
}

/// 边界（D6）：`strict` 元素引入 `off` 元素 → 仍产 `errors[]`（`to` 为该 off 元素
/// FQN，`gate` 为引入方档）。
#[test]
fn 目标off仍可作to() {
    let temp = TempDir::new("off-target");
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.errors.len(), 1, "off 目标仍可作 to");
    assert_eq!(
        report.errors[0].to.as_deref(),
        Some("Auth"),
        "to 为 off 元素 FQN"
    );
}

/// 异常（AC-4 / D7）：目标文件无元素覆盖 → 该边被丢弃（看不见的依赖不变成红点），
/// 文件进 `unmatched_files`；`errors[]` / `warnings[]` / `suppressed[]` /
/// `observations[]` 四段全空、`status = clean`——目标不可归属一律**不冒充违例**，
/// 也不记 warning（留痕只走 `scan.unmatched_files`）。
#[test]
fn 目标不可归属时丢边并留痕() {
    let temp = TempDir::new("unknown-target");
    temp.write_model(
        "[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n",
    );
    temp.write("src/vendor/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../vendor/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "目标不可归属 ⇒ 零 errors");
    assert_eq!(
        report.warnings.len(),
        0,
        "零 warnings（目标不可归属不冒充判定：恰 0 条，不是被折叠的 1 条）"
    );
    assert!(
        report.suppressed.is_empty(),
        "零 suppressed（丢边不产豁免留痕）"
    );
    assert!(
        report.observations.is_empty(),
        "零 observations（未建模目标不产观察项）"
    );
    assert_eq!(
        report.status,
        ArchiCheckStatus::Clean,
        "状态仍为 clean（丢边不是偏差）"
    );
    assert!(
        report
            .scan
            .unmatched_files
            .contains(&"src/vendor/token.ts".to_owned()),
        "目标文件在 unmatched_files 留痕"
    );
}

/// 边界（spec AC-4 逐字「`to` 元素名不在模型内」）：该分支在实现里是**防御臂**
/// ——`owner_of` 只返模型内元素，运行期无输入可达（运行期断言面归「不可测试
/// 项」），故以源码旁证自证三件事：分支条件在案；分支体产 `undeclaredDependency`
/// 且 `gate: None` 的 **warnings** 条目（MUST NOT 记 error）；文案指明「目标未
/// 建模，归因不明」。
#[test]
fn to元素名不在模型内的防御臂源码自证() {
    let source = include_str!("mod.rs");
    assert!(
        source.contains("!known.contains(to)"),
        "防御臂分支条件 `!known.contains(to)` 必须在案（spec AC-4 逐字）"
    );
    assert!(
        source.contains("目标未建模，归因不明"),
        "防御臂文案必须在案（归因不明 → warning 而非 error）"
    );
    let arm = source
        .split("!known.contains(to)")
        .nth(1)
        .expect("防御臂分支体存在");
    let body = &arm[..arm.find("continue;").expect("防御臂分支体以 continue 收口")];
    assert!(
        body.contains("ArchiDriftKind::UndeclaredDependency"),
        "防御臂条目 kind 为 undeclaredDependency：{body}"
    );
    assert!(
        body.contains("gate: None"),
        "防御臂条目 gate 为 None（未建模目标无档位可归）：{body}"
    );
    assert!(
        body.contains("warnings.push"),
        "防御臂条目进 warnings 面：{body}"
    );
    assert!(
        !body.contains("errors.push"),
        "防御臂 MUST NOT 记 error（目标未建模归因不明）：{body}"
    );
}

// ---------------------------------------------------------------------------
// 边界：type-only 同权与 occurrences 聚合
// ---------------------------------------------------------------------------

/// 边界（AC-4）：`import type` 同权 —— 单条 type import → 一条 error 且
/// `occurrences[0].type_only == true`；普通 + type 同元素对 → 两条 occurrence。
#[test]
fn type_only同权() {
    let only_type = TempDir::new("only-type");
    strict_over_auth(&only_type);
    only_type.write("src/auth/token.ts", "export type Token = number;\n");
    only_type.write(
        "src/billing/x.ts",
        "import type { Token } from '../auth/token';\n",
    );
    let report = run_full(only_type.root()).expect("判定应成功");
    assert_eq!(report.errors.len(), 1, "type-only 同权产出");
    assert!(
        report.errors[0].occurrences[0].type_only,
        "occurrence 标注 type_only = true"
    );

    let mixed = TempDir::new("mixed-type");
    strict_over_auth(&mixed);
    mixed.write("src/auth/token.ts", "export const token = 1;\n");
    mixed.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\nimport type { Token } from '../auth/token';\n",
    );
    let report = run_full(mixed.root()).expect("判定应成功");
    assert_eq!(report.errors.len(), 1, "仍是一条元素对条目");
    assert_eq!(report.errors[0].occurrences.len(), 2, "两条命中点");
    let flags: Vec<bool> = report.errors[0]
        .occurrences
        .iter()
        .map(|occurrence| occurrence.type_only)
        .collect();
    assert_eq!(flags, vec![false, true], "普通与 type 各一条");
}

/// 正向（AC-5）：同一 `(from, to)` 3 文件各 1 处 → 一条记录 + 3 项 occurrences
/// （不铺开）。
#[test]
fn occurrences按元素对聚合() {
    let temp = TempDir::new("aggregate");
    strict_over_auth(&temp);
    three_occurrence_fixture(&temp);
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.errors.len(), 1, "聚合为一条记录");
    let occurrences = &report.errors[0].occurrences;
    assert_eq!(occurrences.len(), 3, "三项命中点");
    assert!(
        occurrences
            .iter()
            .all(|occurrence| !occurrence.file.is_empty() && occurrence.line > 0),
        "每项带 file / line"
    );
    assert!(
        occurrences
            .iter()
            .all(|occurrence| occurrence.specifier.as_deref() == Some("../auth/token")),
        "每项带原文 specifier"
    );
    let mut files: Vec<&str> = occurrences
        .iter()
        .map(|occurrence| occurrence.file.as_str())
        .collect();
    files.sort();
    assert_eq!(
        files,
        vec![
            "src/billing/f0.ts",
            "src/billing/f1.ts",
            "src/billing/f2.ts"
        ],
        "命中文件逐条在案"
    );
}

/// 边界：同规模聚合序确定——两次判定的 occurrences 逐字段相等。
#[test]
fn 聚合序确定性() {
    let temp = TempDir::new("aggregate-order");
    strict_over_auth(&temp);
    three_occurrence_fixture(&temp);
    let first = run_full(temp.root()).expect("判定应成功");
    let second = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        first.errors[0].occurrences, second.errors[0].occurrences,
        "两次判定的 occurrences 顺序逐字段相等（扫描序稳定）"
    );
}

// ---------------------------------------------------------------------------
// 正向：豁免两层与层序
// ---------------------------------------------------------------------------

/// 正向（AC-6）：配置元素对豁免 → 不进 errors / warnings，`suppressed[]` 一条
/// （`via = config`、`file` / `line` 双 `None`）。
#[test]
fn 配置豁免留痕() {
    let temp = TempDir::new("config-suppress");
    strict_over_auth(&temp);
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    temp.write_config(
        "{\n  \"ignore\": [{ \"from\": \"Billing\", \"to\": \"Auth\", \"reason\": \"迁移期允许\" }]\n}\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "豁免后零 errors");
    assert!(report.warnings.is_empty(), "豁免后零 warnings");
    assert_eq!(report.suppressed.len(), 1, "一条豁免留痕");
    let suppressed = &report.suppressed[0];
    assert_eq!(suppressed.via, ArchiSuppressVia::Config, "来源为配置");
    assert_eq!(suppressed.reason, "迁移期允许", "理由逐字");
    assert_eq!(suppressed.file, None, "配置豁免无文件");
    assert_eq!(suppressed.line, None, "配置豁免无行号");
}

/// 正向（AC-6）：内联注释豁免 → 不进 errors，`suppressed[]` 一条带 `file` /
/// `line`。
#[test]
fn 内联豁免留痕() {
    let temp = TempDir::new("inline-suppress");
    strict_over_auth(&temp);
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "// archi-ignore-next-line: 迁移期\nimport { token } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "内联豁免后零 errors");
    assert_eq!(report.suppressed.len(), 1, "一条豁免留痕");
    let suppressed = &report.suppressed[0];
    assert_eq!(suppressed.via, ArchiSuppressVia::Inline, "来源为内联");
    assert_eq!(suppressed.reason, "迁移期", "理由逐字");
    assert_eq!(
        suppressed.file.as_deref(),
        Some("src/billing/x.ts"),
        "文件为命中文件"
    );
    assert_eq!(suppressed.line, Some(2), "行号为 import 行");
}

/// 边界（AC-6）：层序 声明 > 配置 > 内联 —— 已声明时零 `errors` 零 `suppressed`；
/// 配置与内联同时命中且无声明 → 只记 `via = config` 一条。
#[test]
fn 豁免层序() {
    let declared = TempDir::new("declared-wins");
    declared.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n\n[[relation]]\nfrom = \"Billing\"\nto = \"Auth\"\n",
    );
    declared.write("src/auth/token.ts", "export const token = 1;\n");
    declared.write(
        "src/billing/x.ts",
        "// archi-ignore-next-line: 内联理由\nimport { token } from '../auth/token';\n",
    );
    declared.write_config(
        "{\n  \"ignore\": [{ \"from\": \"Billing\", \"to\": \"Auth\", \"reason\": \"配置理由\" }]\n}\n",
    );
    let report = run_full(declared.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "声明即豁免 ⇒ 零 errors");
    assert!(
        report.suppressed.is_empty(),
        "声明生效时配置 / 内联不再留痕：{:?}",
        report.suppressed
    );

    let layered = TempDir::new("config-over-inline");
    strict_over_auth(&layered);
    layered.write("src/auth/token.ts", "export const token = 1;\n");
    layered.write(
        "src/billing/x.ts",
        "// archi-ignore-next-line: 内联理由\nimport { token } from '../auth/token';\n",
    );
    layered.write_config(
        "{\n  \"ignore\": [{ \"from\": \"Billing\", \"to\": \"Auth\", \"reason\": \"配置理由\" }]\n}\n",
    );
    let report = run_full(layered.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "零 errors");
    assert_eq!(report.suppressed.len(), 1, "只记一条（层级更高者胜）");
    assert_eq!(
        report.suppressed[0].via,
        ArchiSuppressVia::Config,
        "配置层胜出"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界：声明未兑现（observations）
// ---------------------------------------------------------------------------

/// 正向（D13）：模型声明 `A -> B` 而代码零 import → 一条 `unfulfilledDeclaration`
/// 观察项，`status` 仍 `clean`。
#[test]
fn 声明未兑现为观察项() {
    let temp = TempDir::new("unfulfilled");
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\npath = [\"./src/billing/\"]\n\n[[relation]]\nfrom = \"Billing\"\nto = \"Auth\"\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write("src/billing/x.ts", "export const x = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.observations.len(), 1, "一条观察项");
    let observation = &report.observations[0];
    assert_eq!(
        observation.kind,
        ArchiObservationKind::UnfulfilledDeclaration,
        "kind 为声明未兑现"
    );
    assert!(
        observation
            .message
            .contains("全量模式已覆盖观察根下的全部源文件，仍未见 Billing → Auth 的 import"),
        "全量模式文案写明「已覆盖全部源文件」并带元素对：{}",
        observation.message
    );
    assert!(
        !observation.message.contains("只覆盖"),
        "全量模式 MUST NOT 用「只覆盖」的退让句式（D13）：{}",
        observation.message
    );
    assert_eq!(
        report.status,
        ArchiCheckStatus::Clean,
        "观察项不影响 status"
    );
}

/// 边界（D13）：增量模式同样产出观察项（`status` 不因它变 `warnings`）。
#[test]
fn 声明未兑现增量同样产出() {
    let temp = TempDir::new("unfulfilled-incremental");
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\npath = [\"./src/billing/\"]\n\n[[relation]]\nfrom = \"Billing\"\nto = \"Auth\"\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write("src/billing/x.ts", "export const x = 1;\n");
    let report =
        run_incremental(temp.root(), &strings(&["src/auth/token.ts"])).expect("判定应成功");
    assert_eq!(report.observations.len(), 1, "增量同样产出观察项");
    assert!(
        report.observations[0]
            .message
            .contains("增量模式只覆盖变更集内的源文件，本次扫描面内未见 Billing → Auth 的 import"),
        "增量模式文案声明只覆盖变更集（D13）：{}",
        report.observations[0].message
    );
    assert_ne!(
        report.status,
        ArchiCheckStatus::Warnings,
        "观察项不得把 status 拉成 warnings"
    );
}

// ---------------------------------------------------------------------------
// 异常：模型警告（path 失效 / noPath / 归属不确定）
// ---------------------------------------------------------------------------

/// 异常（AC-4）：`path` 失效按元素档定级——`strict` → `errors`、`warn` / `off` →
/// `warnings`；条目 `occurrences` 一条且 `line = 0`。
#[test]
fn path失效按元素档定级() {
    for (gate, in_errors) in [("strict", true), ("warn", false), ("off", false)] {
        let temp = TempDir::new("path-not-found");
        temp.write_model(&format!(
            "[element.\"Ghost\"]\nkind = \"module\"\ngate = \"{gate}\"\npath = [\"./src/ghost/\"]\n"
        ));
        temp.write("src/a.ts", "export const a = 1;\n");
        let report = run_full(temp.root()).expect("判定应成功");
        let errors: Vec<&super::ArchiDriftEntry> = report
            .errors
            .iter()
            .filter(|entry| entry.kind == ArchiDriftKind::PathNotFound)
            .collect();
        let warnings: Vec<&super::ArchiDriftEntry> = report
            .warnings
            .iter()
            .filter(|entry| entry.kind == ArchiDriftKind::PathNotFound)
            .collect();
        if in_errors {
            assert_eq!(errors.len(), 1, "strict 档 path 失效进 errors");
            assert!(warnings.is_empty(), "同条不进 warnings");
            assert_eq!(errors[0].occurrences[0].line, 0, "path 类条目 line = 0");
        } else {
            assert!(errors.is_empty(), "{gate} 档 path 失效不进 errors");
            assert_eq!(warnings.len(), 1, "{gate} 档进 warnings");
        }
        assert_eq!(
            report.model.warnings.len(),
            1,
            "model.warnings 恒留痕（元素档无关）"
        );
        assert_eq!(
            report.model.warnings[0].kind,
            ArchiDriftKind::PathNotFound,
            "kind 为 pathNotFound"
        );
    }
}

/// 异常：元素无 `path` → `model.warnings[]` 一条 `noPath`，不参与归属，
/// `errors[]` 空。
#[test]
#[allow(non_snake_case)]
fn noPath警告() {
    let temp = TempDir::new("no-path");
    temp.write_model("[element.\"Ghost\"]\nkind = \"module\"\ngate = \"strict\"\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.model.warnings.len(), 1, "一条模型警告");
    assert_eq!(
        report.model.warnings[0].kind,
        ArchiDriftKind::NoPath,
        "kind 为 noPath"
    );
    assert!(report.errors.is_empty(), "noPath 不进 errors");
    assert!(
        report.scan.unmatched_files.contains(&"src/a.ts".to_owned()),
        "无 path 元素不认领任何文件"
    );
}

/// 异常（AC-2 交汇）：同长多命中文件 → `warnings[]` 一条 `ambiguousOwner` +
/// `unmatched_files` 含该文件 + `errors[]` 空。
#[test]
#[allow(non_snake_case)]
fn ambiguousOwner警告() {
    let temp = TempDir::new("ambiguous-owner");
    temp.write_model(
        "[element.\"Zeta\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"src/aa/\"]\n\n[element.\"Alpha\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"src/aa/\"]\n",
    );
    temp.write("src/aa/x.ts", "import { token } from '../auth/token';\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "归属不确定不冒充 errors");
    assert_eq!(report.warnings.len(), 1, "一条 ambiguousOwner 提示");
    assert_eq!(
        report.warnings[0].kind,
        ArchiDriftKind::AmbiguousOwner,
        "kind 为 ambiguousOwner"
    );
    assert!(
        report
            .scan
            .unmatched_files
            .contains(&"src/aa/x.ts".to_owned()),
        "该文件计 unmatched"
    );
}

// ---------------------------------------------------------------------------
// 边界：摘要 / 时间戳 / 字段面
// ---------------------------------------------------------------------------

/// 边界（AC-8）：`model_hash` 稳定且随模型字节变化（`sha256:` 前缀）。
#[test]
fn model_hash随模型变化() {
    let temp = TempDir::new("model-hash");
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let first = run_full(temp.root()).expect("判定应成功");
    let second = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        first.model_hash, second.model_hash,
        "模型字节不变 ⇒ 哈希相等"
    );
    assert!(
        first.model_hash.starts_with("sha256:"),
        "哈希前缀：{}",
        first.model_hash
    );

    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/auth/\"]\n",
    );
    let changed = run_full(temp.root()).expect("判定应成功");
    assert_ne!(
        first.model_hash, changed.model_hash,
        "改一个 gate 值 ⇒ 哈希变化（零缓存）"
    );
}

/// 正向：报告字段面与计数——`model` 段与构造模型逐值相等，`scan` 段逐字段等于
/// `scan()` 直出（同一事实不在 `drift` 二次改写）。
#[test]
fn 报告字段面与计数() {
    let temp = TempDir::new("field-surface");
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n\n[[relation]]\nfrom = \"Billing\"\nto = \"Auth\"\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write("src/billing/x.ts", "export const x = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.model.elements, 2, "元素数");
    assert_eq!(report.model.relations, 1, "关系数");
    assert_eq!(report.model.by_gate.strict, 1, "strict 档计数");
    assert_eq!(report.model.by_gate.warn, 1, "warn 档计数");
    assert_eq!(report.model.by_gate.off, 0, "off 档计数");

    let graph = dto::graph_view(temp.root()).expect("图视图应成功");
    let direct = scan::scan(ScanInput {
        root: temp.root(),
        mode: super::ArchiDriftMode::Full,
        changed: &[],
        exclude: &[],
        elements: &graph.elements,
        cancel: CancelFlag::never(),
    })
    .expect("直出扫描应成功");
    assert_eq!(
        report.scan, direct.report,
        "报告 scan 段逐字段等于 scan() 直出"
    );
}

/// 边界（D9）：`timestamp` 为 RFC3339 UTC 形态、`version` 为常量非 0。
#[test]
fn timestamp与version形态() {
    let temp = TempDir::new("timestamp");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.version, 1, "报告版本号常量");
    assert_ne!(report.version, 0, "版本号非 0");
    let timestamp = &report.timestamp;
    assert!(timestamp.ends_with('Z'), "UTC 结尾 Z：{timestamp}");
    let head: String = timestamp.chars().take(19).collect();
    assert_eq!(head.len(), 19, "YYYY-MM-DDTHH:MM:SS 前缀齐备：{timestamp}");
    assert_eq!(
        head.chars().nth(10),
        Some('T'),
        "日期与时间以 T 分隔：{timestamp}"
    );
    assert_eq!(
        head.chars().nth(4),
        Some('-'),
        "年-月以 - 分隔：{timestamp}"
    );
    assert_eq!(
        head.chars().nth(7),
        Some('-'),
        "月-日以 - 分隔：{timestamp}"
    );
    assert_eq!(
        head.chars().nth(13),
        Some(':'),
        "时:分以 : 分隔：{timestamp}"
    );
    assert_eq!(
        head.chars().nth(16),
        Some(':'),
        "分:秒以 : 分隔：{timestamp}"
    );
    assert!(
        head.chars()
            .filter(|character| character.is_ascii_digit())
            .count()
            == 14,
        "前缀含 14 位数字（年月日时分秒）：{timestamp}"
    );
}

// ---------------------------------------------------------------------------
// 异常：取消 / 配置非法 / 根边界
// ---------------------------------------------------------------------------

/// 异常：`cancel` 预置 → `Err(CheckError::Stopped)`（不产报告）。
#[test]
#[allow(non_snake_case)]
fn 取消返回Stopped() {
    let temp = TempDir::new("cancel");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    let cancel = CancelFlag::never();
    cancel.handle().set();
    let outcome = check(CheckInput {
        root: temp.root(),
        mode: super::ArchiDriftMode::Full,
        changed: &[],
        change: None,
        cancel,
    });
    match outcome {
        Err(error) => assert_eq!(error, CheckError::Stopped, "取消必须是 Stopped"),
        Ok(_) => panic!("取消置位后不应产出报告"),
    }
}

/// 异常（AC-6 / AC-9）：看护配置含未知键 → `Err(CheckError::Config(msg))`，检查不
/// 执行（零报告写出）。
#[test]
fn 配置非法即不执行() {
    let temp = TempDir::new("bad-config");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    temp.write_config("{ \"rules\": [] }");
    let outcome = check(CheckInput {
        root: temp.root(),
        mode: super::ArchiDriftMode::Full,
        changed: &[],
        change: None,
        cancel: CancelFlag::never(),
    });
    match outcome {
        Err(CheckError::Config(message)) => {
            assert!(message.contains("unknown field"), "文案指明条目：{message}");
        }
        other => panic!("期望 Config 错误，实际：{other:?}"),
    }
    assert!(
        !temp.root().join("openspec").exists(),
        "检查不执行 ⇒ 零报告目录"
    );
}

/// 异常（AC-6 的另一形态）：`<archi_dir>/config.json` 的 `exclude` 含语法非法
/// glob（`[` 起始未闭合字符类）→ `load_config` 的**形态**校验放行（本层只管
/// 空白 / 无通配符）→ `scan` 建 `ExcludeSet` 时 `Err` ⇒ `check` 返回
/// `Err(CheckError::Scan(msg))`（与 `Config` 面错误变体可辨），`msg` 含条目原文；
/// 零报告产出。非法 glob 的直证另挂 `scan/exclude_test.rs`。
#[test]
fn exclude语法非法归扫描面错误() {
    for entry in ["src/[legacy", "src/a[bc"] {
        let temp = TempDir::new("bad-glob");
        temp.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
        temp.write("src/a.ts", "export const a = 1;\n");
        temp.write_config(&format!("{{ \"exclude\": [\"{entry}\"] }}"));
        let outcome = check(CheckInput {
            root: temp.root(),
            mode: super::ArchiDriftMode::Full,
            changed: &[],
            change: None,
            cancel: CancelFlag::never(),
        });
        match outcome {
            Err(CheckError::Scan(message)) => {
                assert!(
                    message.contains(entry),
                    "扫描面错误文案含非法条目原文 {entry:?}：{message}"
                );
            }
            Err(CheckError::Config(message)) => {
                panic!("形态校验应放行、编译期报 Scan（不是 Config）：{message}")
            }
            Err(CheckError::Stopped) => panic!("非法 glob 不得表现为取消"),
            Ok(report) => panic!("非法 glob 必须 Err，实际产出报告：{report:?}"),
        }
        assert!(
            !temp.root().join("openspec").exists(),
            "零报告产出（非法 glob 即检查不执行）"
        );
    }
}

/// 边界（D3）：`root` 不存在 / 指向普通文件 → `skipped`（模型目录必然缺席），零
/// panic、零 `Err`；含空格与非 ASCII 的合法 root 正常走判定。
#[test]
fn 根路径边界() {
    let temp = TempDir::new("root-boundary");
    let missing = temp.root().join("no-such-root");
    assert_eq!(
        run_full(&missing).expect("不存在 root 不报错").status,
        ArchiCheckStatus::Skipped,
        "不存在 root ⇒ skipped"
    );

    let file_root = temp.root().join("plain.txt");
    fs::write(&file_root, "not a directory\n").expect("写占位文件失败");
    assert_eq!(
        run_full(&file_root).expect("文件 root 不报错").status,
        ArchiCheckStatus::Skipped,
        "文件 root ⇒ skipped"
    );

    let spaced = TempDir::new("根 空格 与中文");
    spaced.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    spaced.write("src/a.ts", "export const a = 1;\n");
    assert_eq!(
        run_full(spaced.root())
            .expect("合法 root 应正常判定")
            .status,
        ArchiCheckStatus::Clean,
        "含空格与非 ASCII 的 root 正常判定"
    );
}

/// 边界（AC-14 反向）：判定纯读盘——`check` 前后 tempdir 全量字节快照相等。
#[test]
fn 零写盘() {
    let temp = TempDir::new("no-write");
    strict_over_auth(&temp);
    three_occurrence_fixture(&temp);
    temp.write_config("{ \"exclude\": [\"src/legacy/**\"] }\n");
    let before = temp.snapshot();
    let _ = run_full(temp.root()).expect("判定应成功");
    let after = temp.snapshot();
    assert_eq!(before, after, "判定前后字节快照必须相等（零写盘）");
}

// ---------------------------------------------------------------------------
// 边界：两模式一致性与规模
// ---------------------------------------------------------------------------

/// 边界（AC-7）：增量报告的元素对级结论 ⊆ 全量报告（按 `(from, to)` 比较）。
#[test]
fn 两模式元素对级一致() {
    let temp = TempDir::new("mode-parity");
    strict_over_auth(&temp);
    three_occurrence_fixture(&temp);
    let full = run_full(temp.root()).expect("全量判定应成功");
    let changed = strings(&[
        "src/billing/f0.ts",
        "src/billing/f1.ts",
        "src/billing/f2.ts",
        "src/auth/token.ts",
    ]);
    let incremental = run_incremental(temp.root(), &changed).expect("增量判定应成功");

    let full_pairs: BTreeSet<(String, String)> = pairs(&full.errors)
        .into_iter()
        .chain(pairs(&full.warnings))
        .collect();
    let incremental_pairs: BTreeSet<(String, String)> = pairs(&incremental.errors)
        .into_iter()
        .chain(pairs(&incremental.warnings))
        .collect();
    assert!(
        incremental_pairs.is_subset(&full_pairs),
        "增量结论必须是全量子集：{incremental_pairs:?} ⊄ {full_pairs:?}"
    );
    assert_eq!(incremental.base.as_deref(), Some("HEAD"), "增量基准恒 HEAD");
    assert_eq!(full.base, None, "全量无基准");
}

/// 边界（D4）：300 文件各一处同一元素对 → `errors` 仍 1 条、`occurrences` 300 项。
#[test]
fn 三百文件规模聚合() {
    let temp = TempDir::new("scale");
    strict_over_auth(&temp);
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    for index in 0..300 {
        temp.write(
            &format!("src/billing/f{index:03}.ts"),
            "import { token } from '../auth/token';\n",
        );
    }
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.errors.len(), 1, "聚合不随规模膨胀");
    assert_eq!(report.errors[0].occurrences.len(), 300, "300 项命中点");
}

// ---------------------------------------------------------------------------
// 异常：错误面文案
// ---------------------------------------------------------------------------

/// 异常：`CheckError::message` 三值——`Stopped` 逐字「检查已取消」，`Config` /
/// `Scan` 原样透传，三值互不相等。
#[test]
fn 错误面文案三值() {
    let stopped = CheckError::Stopped.message();
    let config = CheckError::Config("配置坏了".to_owned()).message();
    let scan = CheckError::Scan("扫描坏了".to_owned()).message();
    assert_eq!(stopped, "检查已取消", "Stopped 文案逐字");
    assert_eq!(config, "配置坏了", "Config 原样透传");
    assert_eq!(scan, "扫描坏了", "Scan 原样透传");
    assert_ne!(stopped, config, "三值互不相等");
    assert_ne!(config, scan, "三值互不相等");
    assert_ne!(stopped, scan, "三值互不相等");
}
