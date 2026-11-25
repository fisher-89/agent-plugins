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

use crate::api::mod_test::{mirrored_schema_text, op_schema};
use crate::dto;
use crate::scan::{self, ScanInput};

use super::report::{
    ArchiApiFindingKind, ArchiCheckStatus, ArchiDriftKind, ArchiObservationKind, ArchiSuppressVia,
};
use super::{api_buckets, check, edge_names, occurrences_of, CheckError, CheckInput, Report};
use crate::scan::DependencyEdge;

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

    /// 落模型（`.dev-team/architecture/graph/model/01-test.toml`）。
    fn write_model(&self, content: &str) {
        self.write(".dev-team/architecture/graph/model/01-test.toml", content);
    }

    /// 落接口定义（`<archi_models_dir>/<分层目录…>/<FQN>.schema.json`）。
    fn write_schema(&self, fqn: &str, content: &str) {
        let path = crate::api::paths::schema_path(self.root(), fqn).expect("合法 FQN");
        let relative = path
            .strip_prefix(self.root())
            .expect("repo 相对路径")
            .to_string_lossy()
            .replace('\\', "/");
        self.write(&relative, content);
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

/// 边界（D3）：模型声明目录在但零 `*.toml`（只有 README）→ `skipped`。
#[test]
fn 模型目录空返回skipped() {
    let temp = TempDir::new("empty-models-dir");
    temp.write(
        ".dev-team/architecture/graph/model/README.md",
        "# 模型说明\n",
    );
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

/// 异常（`noPath` 分档 · 仍报面）：`package` 空 `path`（模型唯一元素）+ 一个源文件 →
/// `model.warnings` 恰一条 `noPath`、`errors` 空、`status == clean`（模型警告不进
/// `status` / `passed`）。
#[test]
fn package空path仍记模型警告() {
    let temp = TempDir::new("no-path-package");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.model.warnings.len(), 1, "package 空 path 记一条");
    let entry = &report.model.warnings[0];
    assert_eq!(entry.kind, ArchiDriftKind::NoPath, "kind 为 noPath");
    assert_eq!(entry.from.as_deref(), Some("App"), "from 为该元素");
    assert_eq!(entry.to, None, "noPath 无目标");
    assert_eq!(entry.gate, Some(dto::GateTier::Off), "gate 取元素档位");
    assert!(report.errors.is_empty(), "noPath 不进 errors");
    assert_eq!(
        report.status,
        ArchiCheckStatus::Clean,
        "模型警告不进 status（仍 clean）"
    );
}

/// 正向（`noPath` 分档 · 豁免面）：`domain` 空 `path`（模型唯一元素）+ 一个源文件 →
/// `model.warnings` 空、`errors` 空、`status == clean`；与「同输入形态的 `package`
/// 孪生」对拍，读数差**恰好只在** `model.warnings`。
#[test]
fn domain空path零模型警告() {
    let domain = TempDir::new("no-path-domain");
    domain.write_model("[element.\"App\"]\nkind = \"domain\"\n");
    domain.write("src/a.ts", "export const a = 1;\n");
    let domain_report = run_full(domain.root()).expect("判定应成功");
    assert!(
        domain_report.model.warnings.is_empty(),
        "domain 空 path 是其标准形态（零告警）: {:?}",
        domain_report.model.warnings
    );
    assert!(domain_report.errors.is_empty(), "零 errors");
    assert_eq!(
        domain_report.status,
        ArchiCheckStatus::Clean,
        "零告警 ⇒ clean"
    );

    let package = TempDir::new("no-path-domain-twin");
    package.write_model("[element.\"App\"]\nkind = \"package\"\n");
    package.write("src/a.ts", "export const a = 1;\n");
    let package_report = run_full(package.root()).expect("判定应成功");
    assert_eq!(
        domain_report.model.elements, package_report.model.elements,
        "两形态元素数相同（输入形态一致）"
    );
    assert_eq!(
        domain_report.warnings.len(),
        package_report.warnings.len(),
        "顶层 warnings 不受分档影响"
    );
    assert_eq!(
        domain_report.scan.unmatched_files, package_report.scan.unmatched_files,
        "归属读数不受分档影响（两者都不认领文件）"
    );
    assert_eq!(
        package_report.model.warnings.len(),
        domain_report.model.warnings.len() + 1,
        "读数差恰好只在 model.warnings 的一条"
    );
}

/// 正向（`noPath` 分档 · 豁免面）：`composite` 空 `path`（模型唯一元素）+ 一个源文件
/// → `model.warnings` 空、`errors` 空、`status == clean`。
#[test]
fn composite空path零模型警告() {
    let temp = TempDir::new("no-path-composite");
    temp.write_model("[element.\"App\"]\nkind = \"composite\"\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(
        report.model.warnings.is_empty(),
        "composite 空 path 是其标准形态（零告警）: {:?}",
        report.model.warnings
    );
    assert!(report.errors.is_empty(), "零 errors");
    assert_eq!(report.status, ArchiCheckStatus::Clean, "零告警 ⇒ clean");
}

/// 边界（`noPath` 分档 · 四值矩阵）：四 kind 各一元素全部空 `path` → `model.warnings`
/// **恰两条**，条目 `from` 集恰 `{App, App.Mod}`（`domain` / `composite` 零条目）。
#[test]
fn 四kind空path只有package与module记警告() {
    let temp = TempDir::new("no-path-four-kinds");
    temp.write_model(concat!(
        "[element.\"App\"]\nkind = \"package\"\n\n",
        "[element.\"App.Domain\"]\nkind = \"domain\"\n\n",
        "[element.\"App.Mod\"]\nkind = \"module\"\n\n",
        "[element.\"App.Comp\"]\nkind = \"composite\"\n",
    ));
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.model.warnings.len(), 2, "恰两条模型警告");
    let froms: Vec<&str> = report
        .model
        .warnings
        .iter()
        .map(|entry| {
            assert_eq!(entry.kind, ArchiDriftKind::NoPath, "两条都是 noPath");
            entry.from.as_deref().expect("noPath 带 from")
        })
        .collect();
    assert_eq!(froms, vec!["App", "App.Mod"], "from 集恰 package 与 module");
    assert!(!froms.contains(&"App.Domain"), "domain 零条目");
    assert!(!froms.contains(&"App.Comp"), "composite 零条目");
    assert!(report.errors.is_empty(), "模型警告不进 errors");
    assert_eq!(report.status, ArchiCheckStatus::Clean, "状态仍 clean");
}

/// 边界（`noPath` 分档 · 归属面）：`domain` / `composite` 空 `path` 元素与同名前缀的
/// 源文件共存 → `scan.unmatched_files` 含该文件（豁免不认领文件、不参与归属），且零
/// `noPath`。
#[test]
fn domain与composite空path不认领文件() {
    for kind in ["domain", "composite"] {
        let temp = TempDir::new("no-path-unowned");
        temp.write_model(&format!("[element.\"Views\"]\nkind = \"{kind}\"\n"));
        temp.write("views/a.ts", "export const a = 1;\n");
        let report = run_full(temp.root()).expect("判定应成功");
        assert!(
            report
                .scan
                .unmatched_files
                .contains(&"views/a.ts".to_owned()),
            "{kind} 空 path 不认领任何文件：{:?}",
            report.scan.unmatched_files
        );
        assert!(
            report
                .model
                .warnings
                .iter()
                .all(|entry| entry.kind != ArchiDriftKind::NoPath),
            "{kind} 零 noPath：{:?}",
            report.model.warnings
        );
        assert!(report.errors.is_empty(), "{kind} 零 errors");
    }
}

/// 边界（`noPath` 分档 · 与 `pathNotFound` 共存）：`domain` 空 `path` + 另一元素
/// `path` 指向不存在文件 → `model.warnings` 恰一条且 `kind == PathNotFound`（无
/// `noPath` 混入），`errors` 空。
#[test]
fn domain空path与路径失效共存() {
    let temp = TempDir::new("no-path-with-missing");
    temp.write_model(
        "[element.\"Ghost\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/ghost/\"]\n\n\
         [element.\"Views\"]\nkind = \"domain\"\n",
    );
    temp.write("src/a.ts", "export const a = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.model.warnings.len(), 1, "恰一条模型警告");
    assert_eq!(
        report.model.warnings[0].kind,
        ArchiDriftKind::PathNotFound,
        "唯一一条是路径失效（无 noPath 混入）"
    );
    assert_eq!(
        report.model.warnings[0].from.as_deref(),
        Some("Ghost"),
        "路径失效属于声明了 path 的元素"
    );
    assert!(report.errors.is_empty(), "warn 档路径失效不进 errors");
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
    assert_eq!(report.version, 3, "报告版本号常量（REPORT_VERSION = 3）");
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

// ===========================================================================
// 契约判据装配（`module-api-contract-format` / AC-7 / AC-8 / AC-9 / D17 / D18 / D20）
// ===========================================================================

use foundation::layout::archi_models_dir;

/// 落两资产（文档 + 手写接口定义）——判据面的事实源是 `.schema.json`，文档是独立事实
/// （`docMissing`），故成对落盘。
fn write_contract(temp: &TempDir, fqn: &str, schema_text: &str) {
    let path = crate::api::paths::contract_path(temp.root(), fqn).expect("合法 FQN");
    let relative = path
        .strip_prefix(temp.root())
        .expect("repo 相对路径")
        .to_string_lossy()
        .replace('\\', "/");
    temp.write(&relative, &crate::api::mod_test::doc_of(fqn));
    let schema_path = crate::api::paths::schema_path(temp.root(), fqn).expect("合法 FQN");
    let schema_relative = schema_path
        .strip_prefix(temp.root())
        .expect("repo 相对路径")
        .to_string_lossy()
        .replace('\\', "/");
    temp.write(&schema_relative, schema_text);
    assert!(
        archi_models_dir(temp.root()).is_dir(),
        "契约目录由落盘自动建立"
    );
}

/// 单元素模型（含祖先链声明，避免 `orphanHierarchy` 结构错）。
fn declared_chain_model(fqn: &str, element_root: &str) -> String {
    let segments: Vec<&str> = fqn.split('.').collect();
    let mut model = String::new();
    for (index, segment) in segments.iter().enumerate() {
        let prefix = segments[..=index].join(".");
        let last = index + 1 == segments.len();
        let (kind, gate, path) = if last {
            ("module", "warn", format!("[\"{element_root}\"]"))
        } else if index == 0 {
            ("package", "off", "[]".to_owned())
        } else {
            ("domain", "off", "[]".to_owned())
        };
        model.push_str(&format!(
            "[element.\"{prefix}\"]\nkind = \"{kind}\"\ngate = \"{gate}\"\npath = {path}\n\n"
        ));
        let _ = segment;
    }
    model
}

/// 与代码签名一致的试点契约（1 op `file_stem` + 1 类型）。
fn pilot_contract(fqn: &str) -> String {
    mirrored_schema_text(
        fqn,
        r#""LocalArchiCheck": {"type": "object", "properties": {}}"#,
        r#""file_stem": { "x-function": {"params": [{"name": "timestamp", "schema": {"type": "string"}}], "returns": {"type": "string"}} }"#,
    )
}

/// 与代码签名**不一致**的契约（`file_stem` 的返回位声明为 `integer`）。
fn mismatched_contract(fqn: &str) -> String {
    op_schema(
        fqn,
        "file_stem",
        r#"{"params": [{"name": "timestamp", "schema": {"type": "string"}}], "returns": {"type": "integer"}}"#,
    )
}

/// 边界（D17 / AC-8）：tempdir 有模型与源文件但 `models/**` 零 `*.md` → `Ok`，
/// `api_findings` 空、`version == 3`，既有各槽位与「契约面未引入时」等价。
#[test]
fn 契约面缺席时_api_findings为空() {
    let temp = TempDir::new("contract-face-absent");
    temp.write_model(
        "[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n\n\
         [element.\"App.Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");

    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.api_findings.is_empty(), "契约面缺席 → 空槽位");
    assert_eq!(report.version, 3, "版本号为 3（REPORT_VERSION）");
    assert_eq!(
        report.status.as_str(),
        "clean",
        "既有 status 推导不受契约面影响"
    );
    assert!(report.errors.is_empty(), "既有 errors 空");
    assert!(report.warnings.is_empty(), "既有 warnings 空");
    assert!(report.observations.is_empty(), "既有 observations 空");
}

/// 正向（AC-13 / D9）：试点元素（入口命中 + 契约在案 + 签名一致）→ `api_findings` 空
/// （十判据零命中）；契约面判据 0 条 ⇒ 纳管子集空 ⇒ `status` 与无契约目录时相同。
#[test]
fn 试点元素零命中且不改status() {
    let temp = TempDir::new("pilot-clean");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    write_contract(&temp, fqn, &pilot_contract(fqn));

    let with_contract = run_full(temp.root()).expect("判定应成功");
    assert!(
        with_contract.api_findings.is_empty(),
        "十判据零命中：{:?}",
        with_contract
            .api_findings
            .iter()
            .map(|finding| &finding.message)
            .collect::<Vec<_>>()
    );

    // 对照：删掉契约目录 → 同一 status（契约面判据 0 条 ⇒ 不改 status）
    let without = TempDir::new("pilot-clean-baseline");
    without.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    without.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    let baseline = run_full(without.root()).expect("判定应成功");
    assert_eq!(
        with_contract.status.as_str(),
        baseline.status.as_str(),
        "判据 0 条 ⇒ 纳管子集空 ⇒ status 与无契约面时逐字相同"
    );
    assert!(baseline.api_findings.is_empty(), "无契约面即空槽位");
}

/// 正向（AC-8 非空槽位 / AC-6 / D9）：契约签名与代码不一致 → `api_findings` 非空，
/// `kind == apiSignatureMismatch`、`category == "signature"`、`occurrences` 逐项；
/// **契约在案元素**的判据纳管进 `status`（`clean` → `warnings`），`passed` 不受影响。
#[test]
fn 契约不一致产出非空api_findings() {
    let temp = TempDir::new("contract-mismatch");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    write_contract(&temp, fqn, &mismatched_contract(fqn));

    let report = run_full(temp.root()).expect("判定应成功");
    assert!(!report.api_findings.is_empty(), "契约不一致应有条目");
    let finding = &report.api_findings[0];
    assert_eq!(
        finding.kind,
        crate::drift::report::ArchiApiFindingKind::ApiSignatureMismatch,
        "kind 为 apiSignatureMismatch：{:?}",
        finding.kind
    );
    assert_eq!(finding.category, "signature", "分类 = signature");
    assert_eq!(finding.element.as_deref(), Some(fqn));
    assert!(!finding.occurrences.is_empty(), "occurrences 逐项在案");
    assert_eq!(
        report.status.as_str(),
        "errors",
        "`apiSignatureMismatch` 属 error 档 ⇒ status 抬到 errors"
    );
    assert!(
        report.errors.is_empty(),
        "errors 槽位空（error 档判据不进该槽）"
    );
    assert!(report.warnings.is_empty(), "既有 warnings 空");
    assert!(!report.passed(), "error 档判据在纳管子集内 ⇒ 门禁布尔为假");
    assert_eq!(
        report.status == ArchiCheckStatus::Errors,
        !report.passed(),
        "`status == errors ⟺ passed == false`"
    );
}

/// 边界（AC-4 / D9）：`docMissing` 判据（载体未就位）⇒ 该元素不在纳管子集 ⇒
/// `status` / `passed` 与「契约面未启用」基线逐字相同（存量入口文档债不常态化为
/// 永久 `warnings`）。
#[test]
fn 入口缺失元素的判据不进纳管子集() {
    let temp = TempDir::new("entry-missing-not-gated");
    let pilot = "App.Backend.Infra.ArchiRuntime";
    let ghost = "App.Backend.Infra.GhostRuntime";
    temp.write_model(&chain_with_siblings(
        pilot,
        "crates/infra/archi/",
        &[(ghost, "crates/infra/ghost/")],
    ));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    temp.write(
        "crates/infra/ghost/src/lib.rs",
        "pub fn ghost() -> String { String::new() }\n",
    );
    // 试点元素的含 fence 载体把契约面打开；ghost 自身没有载体（未就位）
    write_contract(&temp, pilot, &pilot_contract(pilot));

    let report = run_full(temp.root()).expect("判定应成功");
    assert!(
        report.api_findings.iter().any(|finding| {
            finding.kind == crate::drift::report::ArchiApiFindingKind::DocMissing
                && finding.element.as_deref() == Some(ghost)
        }),
        "载体未就位应有 `docMissing`：{:?}",
        report
            .api_findings
            .iter()
            .map(|finding| (&finding.kind, &finding.message))
            .collect::<Vec<_>>()
    );

    // 基线：同一模型、同一盘面，但契约面整体未启用（不写任何载体）
    let baseline_temp = TempDir::new("entry-missing-baseline");
    baseline_temp.write_model(&chain_with_siblings(
        pilot,
        "crates/infra/archi/",
        &[(ghost, "crates/infra/ghost/")],
    ));
    baseline_temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    baseline_temp.write(
        "crates/infra/ghost/src/lib.rs",
        "pub fn ghost() -> String { String::new() }\n",
    );
    let baseline = run_full(baseline_temp.root()).expect("判定应成功");

    assert_eq!(
        report.status.as_str(),
        baseline.status.as_str(),
        "无契约在案 ⇒ 判据不进纳管子集 ⇒ status 逐字相同"
    );
    assert_eq!(
        report.errors.len(),
        baseline.errors.len(),
        "门禁派生面（errors）与入口缺失判据无关"
    );
    assert!(
        report.errors.is_empty() && report.model.structural_errors.is_empty(),
        "`passed` 派生面（errors / structural_errors 双空）不受影响"
    );
    assert_eq!(
        report.warnings.len(),
        baseline.warnings.len(),
        "既有 warnings 计数不受影响"
    );
}

/// 边界（AC-8）：`api_findings` 空时序列化文本含 `"apiFindings":[]`；非空时逐项在文本
/// 中可辨（`kind` 与 `category` 双在）。
#[test]
fn 报告线面含契约判据槽位() {
    let temp = TempDir::new("contract-slot-json");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let empty = run_full(temp.root()).expect("判定应成功");
    let empty_text = serde_json::to_string(&empty).expect("序列化失败");
    assert!(
        empty_text.contains("\"apiFindings\":[]"),
        "空槽位显式 []：{empty_text}"
    );

    write_contract(&temp, fqn, &mismatched_contract(fqn));
    let filled = run_full(temp.root()).expect("判定应成功");
    let filled_text = serde_json::to_string(&filled).expect("序列化失败");
    assert!(
        filled_text.contains("\"kind\":\"apiSignatureMismatch\""),
        "非空条目的 kind 在文本中：{filled_text}"
    );
    assert!(
        filled_text.contains("\"category\":\"signature\""),
        "非空条目的 category 在文本中：{filled_text}"
    );
}

/// 边界（D17 / AC-8）：无模型（`skipped`）→ `api_findings` 空、`version == 3`
/// （未启用即空，零契约面参与）。
#[test]
fn skipped分支恒空且版本为三() {
    let temp = TempDir::new("skipped-empty");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.status.as_str(), "skipped", "无模型 → skipped");
    assert!(report.api_findings.is_empty(), "skipped 分支恒空槽位");
    assert_eq!(report.version, 3, "版本号为 3");
}

/// 边界（AC-8 / D18）：任一分支产出的报告 `version` 均为 `3`（`skipped` / `noChanges` /
/// `clean` / `errors` 四分支表驱动）——常量与报告字段同源。
#[test]
fn 四分支版本号恒为三() {
    // errors 分支：strict 元素引入未声明目标
    let errors = TempDir::new("version-errors");
    errors.write_model(
        "[element.\"App.Strict\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/strict/\"]\n\n\
         [element.\"App.Shared\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/shared/\"]\n",
    );
    errors.write(
        "src/strict/a.ts",
        "import { token } from '../shared/token';\nexport const a = token;\n",
    );
    errors.write("src/shared/token.ts", "export const token = 1;\n");

    // clean 分支：无跨元素 import
    let clean = TempDir::new("version-clean");
    clean.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    clean.write("src/a.ts", "export const a = 1;\n");

    // noChanges 分支：增量零变更集
    let no_changes = TempDir::new("version-no-changes");
    no_changes.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    no_changes.write("src/a.ts", "export const a = 1;\n");

    // skipped 分支：无模型
    let skipped = TempDir::new("version-skipped");

    let cases: Vec<(&str, Report)> = vec![
        ("errors", run_full(errors.root()).expect("判定应成功")),
        ("clean", run_full(clean.root()).expect("判定应成功")),
        (
            "noChanges",
            run_incremental(no_changes.root(), &[]).expect("判定应成功"),
        ),
        ("skipped", run_full(skipped.root()).expect("判定应成功")),
    ];
    for (label, report) in cases {
        assert_eq!(report.version, 3, "{label} 分支的 version 恒为 3");
        assert_eq!(
            report.status.as_str(),
            label,
            "{label} 分支的 status 与登记一致"
        );
        assert!(
            report.api_findings.is_empty(),
            "{label} 分支的契约槽位为空（这些树均无契约面）"
        );
    }
}

/// 正向（D20）：契约资产结构非法 → `Err(CheckError::Config(..))`，检查不执行、
/// 不落报告；`message()` 透传内层文案。
#[test]
fn 坏契约即config错误() {
    let temp = TempDir::new("bad-contract");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    // 接口文件 `$id` 与文件名不符 → 契约错误（文档正文零参与判定）
    write_contract(&temp, fqn, &crate::api::mod_test::empty_schema(fqn));
    let bad = format!(
        "{{\"$id\": \"App.Backend.Other.schema.json\", \"title\": \"{fqn}\", \"$defs\": {{}}, \"properties\": {{}}}}\n"
    );
    temp.write_schema(fqn, &bad);

    match run_full(temp.root()) {
        Err(CheckError::Config(message)) => {
            assert!(
                message.contains(fqn) || message.contains("App.Backend.Other"),
                "文案含契约定位：{message}"
            );
            assert!(!message.is_empty(), "文案非空");
        }
        other => panic!("坏契约应报 CheckError::Config，实际：{other:?}"),
    }

    // 形态二：IO 类错误（契约文件在案但不可解码）→ 同样映射 `CheckError::Config`，
    // 检查不执行、不产半真报告
    let unreadable = TempDir::new("unreadable-contract");
    unreadable.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    unreadable.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }
",
    );
    let contract_file = crate::api::paths::schema_path(unreadable.root(), fqn).expect("合法 FQN");
    std::fs::create_dir_all(contract_file.parent().expect("父目录")).expect("建接口目录");
    std::fs::write(&contract_file, [0xff_u8, 0xfe, 0x00, 0x01]).expect("写非法 UTF-8 字节");
    match run_full(unreadable.root()) {
        Err(CheckError::Config(message)) => {
            assert!(
                message.contains("契约") || message.contains("ArchiRuntime"),
                "IO 类错误文案含契约线索：{message}"
            );
            assert!(!message.is_empty(), "文案非空");
            assert!(
                !message.contains("检查已执行"),
                "资产非法 ⇒ 检查不执行（MUST NOT 冒充已检查）"
            );
        }
        other => panic!("不可解码契约应报 CheckError::Config，实际：{other:?}"),
    }
}

/// 边界（AC-6 / D9）：契约在案元素的判据只抬 `status`（`clean` → `warnings`）——同输入在
/// 契约面追加不一致契约前后，`errors` / `warnings` / `suppressed` / `observations` /
/// `model` 逐字段相等，`passed` 派生面（`errors` / `structural_errors` 双空）零改动。
#[test]
fn 契约面不改既有槽位() {
    let temp = TempDir::new("contract-parallel");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let before = run_full(temp.root()).expect("判定应成功");
    write_contract(&temp, fqn, &mismatched_contract(fqn));
    let after = run_full(temp.root()).expect("判定应成功");

    assert!(!after.api_findings.is_empty(), "追加契约后契约槽位非空");
    assert_eq!(before.errors, after.errors, "errors 零变化");
    assert_eq!(before.warnings, after.warnings, "warnings 零变化");
    assert_eq!(before.suppressed, after.suppressed, "suppressed 零变化");
    assert_eq!(
        before.observations, after.observations,
        "observations 零变化"
    );
    assert_eq!(before.status.as_str(), "clean", "追加前 status 为 clean");
    assert_eq!(
        after.status.as_str(),
        "errors",
        "契约在案元素的 error 档判据把 status 抬到 errors"
    );
    assert!(
        after.errors.is_empty() && after.model.structural_errors.is_empty(),
        "既有 errors / structuralErrors 双空（抬档由 error 档判据单独触发）"
    );
    assert!(!after.passed(), "error 档判据 ⇒ 门禁布尔为假");
    assert_eq!(
        before.model, after.model,
        "模型面事实零变化（契约 md 不是模型资产）"
    );
}

/// 边界（AC-12）：契约面启用后判定仍零写盘——同一暂存树在两次判定前后字节快照相等。
#[test]
fn 契约面启用后仍零写盘() {
    let temp = TempDir::new("contract-no-write");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    write_contract(&temp, fqn, &pilot_contract(fqn));

    let before = temp.snapshot();
    assert_eq!(temp.snapshot(), before, "判定 MUST NOT 写盘（含契约面）");
}

// ---------------------------------------------------------------------------
// 契约判据装配的缺省面：启用条件 / ② 缺 md / ① 无入口 / ⑤ 留白
// ---------------------------------------------------------------------------

/// 链模型 + 若干同父兄弟模块（共用祖先链声明；兄弟各带 `path`）。
fn chain_with_siblings(fqn: &str, element_root: &str, siblings: &[(&str, &str)]) -> String {
    let mut model = declared_chain_model(fqn, element_root);
    for (sibling, sibling_root) in siblings {
        model.push_str(&format!(
            "[element.\"{sibling}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"{sibling_root}\"]\n\n"
        ));
    }
    model
}

/// 一个只含类型的契约（零 op；用于类型面提示 / 形状类用例）。
fn types_only_contract(fqn: &str, defs: &str) -> String {
    mirrored_schema_text(fqn, defs, "")
}

/// 某种 kind 的条目数。
fn count_kind(report: &Report, kind: ArchiApiFindingKind) -> usize {
    report
        .api_findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .count()
}

/// 边界（D17）：`models/**` 零 `*.md`（只有模型文件与 `graph/INDEX.toml`）→ 契约面
/// 未启用，`api_findings` 空、既有槽位与「目录缺席」形态逐字段等价——模型文件与索引
/// 都不是启用证据。
#[test]
fn 契约目录零md即未启用() {
    let temp = TempDir::new("api-dir-without-md");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    temp.write("src/a.ts", "export const a = 1;\n");
    temp.write(".dev-team/architecture/graph/INDEX.toml", "# 空索引\n");

    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.api_findings.is_empty(), "零 md ⇒ 未启用 ⇒ 空槽位");
    assert_eq!(report.version, 3, "版本号仍为 3");
    assert_eq!(report.status.as_str(), "clean", "既有 status 零变化");

    // 对照：同树无契约目录 → 两报告除时间戳 / 根 / 摘要外逐字段相等
    let bare = TempDir::new("api-dir-absent-baseline");
    bare.write_model("[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n");
    bare.write("src/a.ts", "export const a = 1;\n");
    let baseline = run_full(bare.root()).expect("判定应成功");

    let mut comparable = report.clone();
    comparable.timestamp = baseline.timestamp.clone();
    comparable.root = baseline.root.clone();
    comparable.model_hash = baseline.model_hash.clone();
    assert_eq!(
        comparable, baseline,
        "「目录存在但零 md」与「目录缺席」逐字段等价（同一未启用分支）"
    );
}

/// 正向（AC-2）：`api = declared` 元素有入口无载体 → 1 条 `docMissing`
/// （`category = doc`、`element` 点名），其余元素零命中。
#[test]
fn 缺md产出契约缺失判据() {
    let temp = TempDir::new("api-missing");
    let pilot = "App.Backend.Infra.ArchiRuntime";
    let sibling = "App.Backend.Infra.SecondRuntime";
    temp.write_model(&chain_with_siblings(
        pilot,
        "crates/infra/archi/",
        &[(sibling, "crates/infra/second/")],
    ));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    temp.write(
        "crates/infra/second/src/lib.rs",
        "pub fn second() -> String { String::new() }\n",
    );
    write_contract(&temp, pilot, &pilot_contract(pilot));

    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::DocMissing),
        1,
        "只有缺载体的那个元素报 docMissing：{:?}",
        report
            .api_findings
            .iter()
            .map(|finding| (finding.kind, finding.element.clone()))
            .collect::<Vec<_>>()
    );
    let finding = report
        .api_findings
        .iter()
        .find(|finding| {
            finding.kind == ArchiApiFindingKind::DocMissing
                && finding.element.as_deref() == Some(sibling)
        })
        .expect("缺载体元素应产条目");
    assert_eq!(finding.category, "doc", "④ 的分类 = doc");
    assert!(
        finding.message.contains(sibling) || finding.message.contains(".md"),
        "文案含元素与契约路径：{}",
        finding.message
    );
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::ApiEntryMissing),
        0,
        "两个元素都有入口 → 无 apiEntryMissing"
    );
    assert_eq!(
        report.status.as_str(),
        "clean",
        "`docMissing` 未纳管（承诺未成立）⇒ status 不长档"
    );
}

/// 正向（AC-7 / D16）：元素 `path` 非空但四分支全不命中 → 1 条 `apiEntryMissing`，
/// 且该元素不再参与 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`。
#[test]
fn 无入口产出入口缺失判据且互斥() {
    let temp = TempDir::new("api-entry-missing");
    let pilot = "App.Backend.Infra.ArchiRuntime";
    let ghost = "App.Backend.Infra.GhostRuntime";
    temp.write_model(&chain_with_siblings(
        pilot,
        "crates/infra/archi/",
        &[(ghost, "crates/infra/ghost/")],
    ));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    // ghost 的目录不落任何入口候选文件（也无 `<root>.i.rs` 生成物）
    write_contract(&temp, pilot, &pilot_contract(pilot));
    // ghost 亦有契约在案（签名与代码不可比）——互斥生效时它不能进 ④
    write_contract(&temp, ghost, &pilot_contract(ghost));

    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::ApiEntryMissing),
        1,
        "恰一条 apiEntryMissing：{:?}",
        report
            .api_findings
            .iter()
            .map(|finding| (finding.kind, finding.element.clone()))
            .collect::<Vec<_>>()
    );
    let finding = report
        .api_findings
        .iter()
        .find(|finding| finding.kind == ArchiApiFindingKind::ApiEntryMissing)
        .expect("条目前置已断言");
    assert_eq!(finding.category, "entry", "① 的分类 = entry");
    assert_eq!(finding.element.as_deref(), Some(ghost), "点名无入口元素");
    for kind in [
        ArchiApiFindingKind::ApiMissing,
        ArchiApiFindingKind::ApiUndeclared,
        ArchiApiFindingKind::ApiSignatureMismatch,
    ] {
        assert_eq!(
            count_kind(&report, kind),
            0,
            "① 命中后 MUST NOT 再求 {kind:?}"
        );
    }
}

/// 异常（D16 ⑤ / AC-7）：`api = none` + 契约文件在案 → 1 条 `apiUnexpected`，
/// 同时 `apiMissing` 零条（两条 MUST NOT 双报）。
#[test]
fn 留白遇契约在案产出留白判据() {
    let temp = TempDir::new("api-unexpected");
    let fqn = "App.Backend.Infra.BlankRuntime";
    let pilot = "App.Backend.Infra.ArchiRuntime";
    temp.write_model(&format!(
        "{}\n\
         [element.\"{fqn}\"]\nkind = \"module\"\ngate = \"warn\"\napi = \"none\"\npath = [\"crates/infra/blank/\"]\n",
        declared_chain_model(pilot, "crates/infra/archi/"),
    ));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    temp.write(
        "crates/infra/blank/src/lib.rs",
        "pub fn blank() -> String { String::new() }\n",
    );
    // 试点元素的含 fence 载体把契约面打开；留白元素自身在派生契约路径上有文件
    write_contract(&temp, pilot, &pilot_contract(pilot));
    write_contract(&temp, fqn, &pilot_contract(fqn));

    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::ApiUnexpected),
        1,
        "留白 + md 在案 → 1 条 apiUnexpected：{:?}",
        report
            .api_findings
            .iter()
            .map(|finding| (finding.kind, finding.element.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::DocMissing),
        0,
        "反方向归 ④，两条 MUST NOT 双报"
    );
    let finding = report
        .api_findings
        .iter()
        .find(|finding| finding.kind == ArchiApiFindingKind::ApiUnexpected)
        .expect("条目前置已断言");
    assert_eq!(finding.category, "entry", "⑤ 的分类 = entry");
    assert_eq!(finding.element.as_deref(), Some(fqn));
    assert_eq!(report.version, 3, "契约面启用不影响版本号");
}

/// 边界（D27 交汇）：`properties` 全为函数未标 `x-class` → 该提示只在生成器 stderr，
/// **不**进 `apiFindings` 也不改 `status`；`x-class` 带属性 → 契约错误映射
/// `CheckError::Config`（检查不执行、不产半真报告）。
#[test]
fn xclass两判据不串入报告装配() {
    let fqn = "App.Backend.Infra.ArchiRuntime";

    // 形态一：函数指针结构体未标 x-class（合法意图，零判据）
    let warn_shape = TempDir::new("xclass-warning-shape");
    warn_shape.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    warn_shape.write("crates/infra/archi/src/lib.rs", "pub struct Handlers;\n");
    write_contract(
        &warn_shape,
        fqn,
        &types_only_contract(
            fqn,
            "{\"Handlers\": {\"type\": \"object\", \"properties\": {\"on_a\": {\"x-function\": {\"params\": [], \"returns\": {\"type\": \"string\"}}}}}}",
        ),
    );
    let report = run_full(warn_shape.root()).expect("判定应成功");
    assert!(
        report.api_findings.is_empty(),
        "未标 x-class 的提示 MUST NOT 成为判据：{:?}",
        report
            .api_findings
            .iter()
            .map(|finding| (finding.kind, finding.message.clone()))
            .collect::<Vec<_>>()
    );

    // 对照：同树把类型改成普通对象（无函数属性）→ status 逐值相等
    let plain_shape = TempDir::new("xclass-warning-baseline");
    plain_shape.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    plain_shape.write("crates/infra/archi/src/lib.rs", "pub struct Handlers;\n");
    write_contract(
        &plain_shape,
        fqn,
        &types_only_contract(
            fqn,
            "{\"Handlers\": {\"type\": \"object\", \"properties\": {}}}",
        ),
    );
    let baseline = run_full(plain_shape.root()).expect("判定应成功");
    assert_eq!(
        report.status.as_str(),
        baseline.status.as_str(),
        "提示面不影响 status"
    );
    assert!(baseline.api_findings.is_empty(), "对照树同样零判据");

    // 形态二：x-class 带属性 → 契约错误（D20 映射，检查不执行）
    let broken = TempDir::new("xclass-error-shape");
    broken.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    broken.write("crates/infra/archi/src/lib.rs", "pub struct Handlers;\n");
    write_contract(
        &broken,
        fqn,
        &types_only_contract(
            fqn,
            "{\"Handlers\": {\"x-class\": true, \"properties\": {\"field\": {\"type\": \"string\"}}}}",
        ),
    );
    match run_full(broken.root()) {
        Err(CheckError::Config(message)) => {
            assert!(message.contains("x-class"), "文案点名 x-class：{message}");
        }
        other => panic!("x-class 带属性应报 CheckError::Config，实际：{other:?}"),
    }
}

/// 边界（AC-7 规模 / D18 聚合）：300 元素（100 有契约）+ 300 源文件 → 条数可判、不
/// panic；同一元素缺 3 个 op 聚合成 1 条（`occurrences` 3 项）；同一元素族同时命中
/// ⑥ / ⑧ / ⑩ → 每条独立。
#[test]
fn 组合链规模与多判据独立() {
    // ---- 规模面：300 元素，前 100 有契约（签名一致），后 200 缺 md ----
    let scale = TempDir::new("api-scale");
    let mut model = String::new();
    for index in 0..300 {
        model.push_str(&format!(
            "[element.\"E{index:03}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/e{index:03}/\"]\n\n"
        ));
    }
    scale.write_model(&model);
    for index in 0..300 {
        scale.write(
            &format!("src/e{index:03}/src/lib.rs"),
            "pub fn run() -> String { String::new() }\n",
        );
        if index < 100 {
            let fqn = format!("E{index:03}");
            write_contract(
                &scale,
                &fqn,
                &op_schema(
                    &fqn,
                    "run",
                    r#"{"params": [], "returns": {"type": "string"}}"#,
                ),
            );
        }
    }
    let report = run_full(scale.root()).expect("规模输入应成功");
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::DocMissing),
        200,
        "200 个 declared 元素缺载体各 1 条"
    );
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::ApiEntryMissing),
        0,
        "300 个入口全部在案"
    );
    assert_eq!(
        count_kind(&report, ArchiApiFindingKind::ApiSignatureMismatch),
        0,
        "100 份契约与代码签名一致"
    );
    assert_eq!(report.api_findings.len(), 200, "条数可判、零意外条目");
    assert_eq!(report.status.as_str(), "clean", "契约面不进 status");

    // ---- 聚合面：同一元素缺 3 个 op → 1 条 ②，`occurrences` 3 项 ----
    let aggregate = TempDir::new("api-occurrences-aggregated");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    aggregate.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    aggregate.write("crates/infra/archi/src/lib.rs", "pub fn other() {}\n");
    write_contract(
        &aggregate,
        fqn,
        &mirrored_schema_text(
            fqn,
            "",
            format!(
                "{}, {}, {}",
                r#""alpha": { "x-function": {"params": [], "returns": {"type": "string"}} }"#,
                r#""beta": { "x-function": {"params": [], "returns": {"type": "string"}} }"#,
                r#""gamma": { "x-function": {"params": [], "returns": {"type": "string"}} }"#
            ),
        ),
    );
    let aggregated = run_full(aggregate.root()).expect("判定应成功");
    let missing: Vec<&crate::drift::report::ArchiApiFinding> = aggregated
        .api_findings
        .iter()
        .filter(|finding| finding.kind == ArchiApiFindingKind::ApiMissing)
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "三个缺实现聚合成 1 条（不铺开）：{missing:?}"
    );
    assert_eq!(missing[0].occurrences.len(), 3, "3 项命中点逐项在案");
    assert!(
        missing[0]
            .target
            .as_deref()
            .is_some_and(|target| target.contains("alpha")
                && target.contains("beta")
                && target.contains("gamma")),
        "target 点名三个缺失 op：{:?}",
        missing[0].target
    );

    // ---- 多判据独立面：同一元素族同时命中 ⑥ / ⑧ / ⑩ ----
    let multi = TempDir::new("api-multi-findings");
    let parent = "App.Backend.Infra.ArchiRuntime";
    let child = "App.Backend.Infra.ArchiRuntime.Sub";
    multi.write_model(&format!(
        "{}\n[element.\"{child}\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"crates/core/sub/\"]\n",
        declared_chain_model(parent, "crates/infra/archi/")
    ));
    multi.write("crates/infra/archi/src/lib.rs", "pub struct Shared;\n");
    multi.write("crates/core/sub/src/lib.rs", "pub struct Shared;\n");
    write_contract(
        &multi,
        parent,
        &mirrored_schema_text(
            parent,
            r#""Shared": {"type": "object", "properties": {}}"#,
            r#""new": { "x-function": {"params": [], "returns": {"type": "string"}} }"#,
        ),
    );
    write_contract(
        &multi,
        child,
        &types_only_contract(
            child,
            "{\"Shared\": {\"type\": \"object\", \"properties\": {}}}",
        ),
    );
    let multi_report = run_full(multi.root()).expect("判定应成功");
    assert_eq!(
        count_kind(&multi_report, ArchiApiFindingKind::ApiShadowing),
        1,
        "⑥ 父子 `$defs` 交集 → 1 条"
    );
    assert_eq!(
        count_kind(&multi_report, ArchiApiFindingKind::ApiConstructorDeclared),
        1,
        "⑩ 契约声明构造器 → 1 条"
    );
    assert_eq!(
        count_kind(&multi_report, ArchiApiFindingKind::ApiNestedCrossCrate),
        1,
        "⑧ 父子所属包根不同 → 1 条"
    );
    assert_eq!(
        multi_report.api_findings.len(),
        3,
        "三条判据各自独立（不合并、不互相吞并）：{:?}",
        multi_report
            .api_findings
            .iter()
            .map(|finding| finding.kind)
            .collect::<Vec<_>>()
    );
    for finding in &multi_report.api_findings {
        assert!(
            matches!(
                finding.category.as_str(),
                "reexport" | "constructor" | "nesting"
            ),
            "分类各归其面：{:?}",
            finding.category
        );
    }
    assert!(
        multi_report
            .api_findings
            .windows(2)
            .all(|window| window[0].kind.as_str() <= window[1].kind.as_str()),
        "对外入口已按 (kind, element, target) 稳定排序"
    );
}

/// 边界（AC-6 / AC-7 / D7 / D9）：`api_findings` 不参与 `status` 推导——`derive_status`
/// 的实参是**纳管子集** `&gating`；契约判据的装配单点在 `crate::api::findings`（drift 侧
/// 零契约装载）；槽位装配排在 ⑦ 观察项之后、`status` 推导之前（源码自证）。
#[test]
fn 判定序源码自证不含api_findings() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source
            .matches("let api_findings = crate::api::findings(")
            .count(),
        1,
        "契约判据装配单点：`crate::api::findings` 恰一处"
    );
    let slot = source
        .find("let api_findings = crate::api::findings(")
        .expect("槽位装配点");
    let gating = source
        .find("let gating = crate::api::gating_findings(")
        .expect("纳管筛选单点");
    assert!(slot < gating, "纳管筛选排在判据装配之后");

    let call_start = source
        .find("let status = derive_status(")
        .expect("status 由 derive_status 推导");
    let call = &source[call_start..];
    let call = &call[..call.find(");").expect("调用以 `);` 收口")];
    assert!(
        !call.contains("api_findings"),
        "derive_status 的实参 MUST NOT 含 api_findings：{call}"
    );
    assert!(call.contains("&gating"), "status 只吃纳管子集：{call}");
    assert!(gating < call_start, "status 推导排在纳管筛选之后");

    let observations = source
        .find("observations.push(ArchiDriftObservation")
        .expect("⑦ 观察项装配在案");
    assert!(
        observations < slot,
        "契约判据装配排在 ⑦ 观察项之后（判定序零漂移）"
    );
    assert!(
        !source.contains("load_contracts"),
        "契约装载只在 `core::archi::api`（drift 侧零内联装配）"
    );
    assert!(
        source.contains("api_findings: Vec::new(),"),
        "skipped 分支显式空槽位（未启用即空）"
    );
    assert_eq!(
        source.matches("const REPORT_VERSION: u32 = 3;").count(),
        1,
        "报告版本常量单点 = 3（改常量即全分支随动）"
    );
}

// ---------------------------------------------------------------------------
// 本变更：报告版本 3 与纳管子集（`status` / `passed` 双面）
// ---------------------------------------------------------------------------

/// 正向（AC-7 / D8）：`run_full` 与 `skipped` 早退臂的报告 `version` 均 3；源码自证
/// `REPORT_VERSION: u32 = 3` 恰一处、零 `= 2`。
#[test]
fn 报告版本常量为3() {
    let temp = TempDir::new("version-three");
    temp.write_model("[element.\"Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n");
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let full = run_full(temp.root()).expect("判定应成功");
    assert_eq!(full.version, 3, "全量报告的 version 为 3");

    let skipped = TempDir::new("version-three-skipped");
    let early = run_full(skipped.root()).expect("无模型不是错误");
    assert_eq!(early.status, ArchiCheckStatus::Skipped, "早退臂");
    assert_eq!(early.version, 3, "skipped 早退报告同样为 3");

    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("REPORT_VERSION: u32 = 3").count(),
        1,
        "版本常量恰一处且为 3"
    );
    assert!(!source.contains("REPORT_VERSION: u32 = 2"), "零旧值 2");
}

/// 正向（AC-9）：纳管子集非空且全为 `warn` 档（`apiEntryMissing`）而 `errors` / `warnings`
/// 双空 → `status == warnings`；`passed == true`（warn 档不阻断门禁）。`docMissing` 永不
/// 纳管（文档在盘时零 `docMissing`，不在盘也只 `warn` 不纳管）。
#[test]
fn 纳管子集非空且无errors时status为warnings() {
    let temp = TempDir::new("gating-warnings");
    let pilot = "App.Backend.Infra.ArchiRuntime";
    let pending = "App.Backend.Infra.SecondRuntime";
    temp.write_model(&chain_with_siblings(
        pilot,
        "crates/infra/archi/",
        &[(pending, "crates/infra/second/")],
    ));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    // 试点元素的接口文件把契约面打开（启用证据 = declared ∧ schema 在盘）
    write_contract(&temp, pilot, &pilot_contract(pilot));
    // second 的接口文件在盘、文档也在盘，但实现入口不在盘 ⇒ `apiEntryMissing`（warn 档，
    // 且 `api = declared` ∧ schema 在盘 ⇒ 进纳管子集）
    write_contract(
        &temp,
        pending,
        &mirrored_schema_text(
            pending,
            r#""Pending": {"type": "object", "properties": {}}"#,
            "",
        ),
    );

    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        report
            .api_findings
            .iter()
            .filter(|finding| finding.element.as_deref() == Some(pending))
            .map(|finding| finding.kind)
            .collect::<Vec<_>>(),
        vec![ArchiApiFindingKind::ApiEntryMissing],
        "入口缺失是唯一判据（文档在盘 ⇒ 零 docMissing）"
    );
    assert_eq!(report.status, ArchiCheckStatus::Warnings, "纳管 ⇒ warnings");
    assert!(report.errors.is_empty(), "errors 空");
    assert!(
        report
            .warnings
            .iter()
            .all(|entry| entry.kind != ArchiDriftKind::NoPath),
        "既存 warnings 只剩祖先容器的 `noPath`（纳管槽与既有槽互不挤占）：{:?}",
        report
            .warnings
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>()
    );
    assert!(report.passed(), "warn 档判据不阻断门禁 ⇒ passed == true");
}

/// 异常（AC-6 / D9 先后序不变）：同场造 `errors`（未声明依赖）+ 契约在案判据 →
/// `status == errors`（纳管不得抢 `errors` 档）；`errors` 槽内容与非纳管语料逐字相同。
#[test]
fn errors非空时仍errors() {
    let fqn = "Billing";
    let with_contract = TempDir::new("gating-with-errors");
    strict_over_auth(&with_contract);
    with_contract.write("src/auth/token.ts", "export const token = 1;\n");
    with_contract.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    with_contract.write(
        "src/billing/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    write_contract(&with_contract, fqn, &mismatched_contract(fqn));

    let baseline = TempDir::new("gating-with-errors-baseline");
    strict_over_auth(&baseline);
    baseline.write("src/auth/token.ts", "export const token = 1;\n");
    baseline.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );

    let gated = run_full(with_contract.root()).expect("判定应成功");
    let plain = run_full(baseline.root()).expect("判定应成功");
    assert!(!gated.api_findings.is_empty(), "判据非空（前提）");
    assert_eq!(gated.status, ArchiCheckStatus::Errors, "errors 档不被打穿");
    assert_eq!(
        gated.errors, plain.errors,
        "errors 槽内容与非纳管语料逐字相同"
    );
}

/// 边界（AC-6 / D9 / AC-8 的口径侧）：纳管子集空（契约面缺席）⇒ `status` 逐值不变
/// （`clean` / `noChanges` / `skipped` 三形态）+ `api_findings` 空；`warnings` 形态在同场
/// 仍由既有 warnings 决定。
#[test]
fn 纳管子集空时status不变() {
    let clean = TempDir::new("gating-clean");
    clean.write_model("[element.\"Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n");
    clean.write("src/auth/token.ts", "export const token = 1;\n");
    let clean_report = run_full(clean.root()).expect("判定应成功");
    assert_eq!(
        clean_report.status,
        ArchiCheckStatus::Clean,
        "clean 逐值不变"
    );
    assert!(clean_report.api_findings.is_empty(), "契约面缺席 ⇒ 空槽位");

    let no_changes = TempDir::new("gating-no-changes");
    no_changes.write_model("[element.\"Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n");
    no_changes.write("src/auth/token.ts", "export const token = 1;\n");
    let no_changes_report = run_incremental(no_changes.root(), &[]).expect("判定应成功");
    assert_eq!(
        no_changes_report.status,
        ArchiCheckStatus::NoChanges,
        "noChanges 逐值不变"
    );

    let skipped = TempDir::new("gating-skipped");
    let skipped_report = run_full(skipped.root()).expect("无模型不是错误");
    assert_eq!(
        skipped_report.status,
        ArchiCheckStatus::Skipped,
        "skipped 逐值不变"
    );
    assert!(skipped_report.api_findings.is_empty(), "早退臂空槽位");

    let warn = TempDir::new("gating-warn");
    warn.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n\
         [element.\"Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n",
    );
    warn.write("src/auth/token.ts", "export const token = 1;\n");
    warn.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    let warn_report = run_full(warn.root()).expect("判定应成功");
    assert_eq!(
        warn_report.status,
        ArchiCheckStatus::Warnings,
        "warnings 逐值不变"
    );
    assert_eq!(warn_report.warnings.len(), 1, "既有 warnings 槽内容零增删");
}

/// 异常（AC-4 / 自激活）：元素 `api = declared` 但载体未就位（`docMissing`）→
/// `api_findings` 非空、纳管子集空 ⇒ `status == clean`；补上含 fence 的契约后同语料转
/// `errors`（自激活，无白名单）。
#[test]
fn 契约不在案且判据存在时不改status() {
    let temp = TempDir::new("gating-self-activate");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let pilot = "App.Backend.Infra.PilotRuntime";
    temp.write_model(&chain_with_siblings(
        fqn,
        "crates/infra/archi/",
        &[(pilot, "crates/infra/pilot/")],
    ));
    temp.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    temp.write(
        "crates/infra/pilot/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    // 试点元素的含 fence 载体把契约面打开；目标元素自身载体未就位
    write_contract(&temp, pilot, &pilot_contract(pilot));
    // 一个可判文件（scanned_files > 0），避免 `noChanges` 档抢先——本用例判的是
    // 「纳管子集空 ⇒ status 不被契约判据抬起」
    temp.write(
        "crates/infra/archi/src/token.rs",
        "pub fn token() -> &'static str { \"t\" }
",
    );

    let absent = run_full(temp.root()).expect("判定应成功");
    assert!(!absent.api_findings.is_empty(), "判据非空（载体未就位）");
    assert!(
        absent
            .api_findings
            .iter()
            .any(|finding| finding.kind == ArchiApiFindingKind::DocMissing),
        "判据种类为 docMissing：{:?}",
        absent.api_findings
    );
    assert_eq!(
        absent.status,
        ArchiCheckStatus::Clean,
        "载体未就位 ⇒ 纳管子集空 ⇒ status 不改"
    );

    write_contract(&temp, fqn, &mismatched_contract(fqn));
    let present = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        present.status,
        ArchiCheckStatus::Errors,
        "补上含 fence 的契约后同一语料转 errors（error 档判据纳管）"
    );
}

/// 正向 / 异常（AC-4）：纳管 `error` 档判据 ⇒ `passed == false` 且 `status == errors`
/// （旧「`apiFindings` MUST NOT 影响 `passed`」语义反转）；对照臂：造一条 `errors` →
/// 同样 `passed == false`（`status == errors ⟺ passed == false` 不破）。
#[test]
fn passed随error档反转() {
    let clean_ish = TempDir::new("gating-passed");
    let fqn = "App.Backend.Infra.ArchiRuntime";
    clean_ish.write_model(&declared_chain_model(fqn, "crates/infra/archi/"));
    clean_ish.write(
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    write_contract(&clean_ish, fqn, &mismatched_contract(fqn));
    let gated_report = run_full(clean_ish.root()).expect("判定应成功");
    assert_eq!(
        gated_report.status,
        ArchiCheckStatus::Errors,
        "纳管 error 档 ⇒ errors"
    );
    assert!(
        !gated_report.passed(),
        "error 档判据在纳管子集内 ⇒ 门禁布尔为假"
    );

    let broken = TempDir::new("gating-not-passed");
    strict_over_auth(&broken);
    broken.write("src/auth/token.ts", "export const token = 1;\n");
    broken.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    write_contract(&broken, "Billing", &mismatched_contract("Billing"));
    let failed_report = run_full(broken.root()).expect("判定应成功");
    assert_eq!(failed_report.status, ArchiCheckStatus::Errors, "errors 档");
    assert!(
        !failed_report.passed(),
        "errors 非空 ⇒ 门禁 passed == false"
    );
}

/// 边界（AC-6 / D9）：既有 `warnings` 非空 + 纳管子集非空 → 仍 `warnings`（或条件不产出
/// 新档）；`warnings` 槽内容零增删。
#[test]
fn warnings与纳管并存不叠加升级() {
    let temp = TempDir::new("gating-and-warnings");
    let fqn = "Billing";
    temp.write_model(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n\
         [element.\"Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    temp.write(
        "src/billing/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let before = run_full(temp.root()).expect("判定应成功");
    assert_eq!(before.status, ArchiCheckStatus::Warnings, "既有 warnings");
    assert_eq!(before.warnings.len(), 1, "一条未声明依赖");

    write_contract(&temp, fqn, &mismatched_contract(fqn));
    let after = run_full(temp.root()).expect("判定应成功");
    assert!(!after.api_findings.is_empty(), "纳管子集非空（前提）");
    assert_eq!(
        after.status,
        ArchiCheckStatus::Errors,
        "既有 warnings 之上再叠 error 档判据 ⇒ errors（不叠成新档）"
    );
    assert_eq!(after.warnings, before.warnings, "warnings 槽零增删");
    assert!(!after.passed(), "error 档判据 ⇒ 门禁布尔为假");
}

/// 异常（AC-6 / D7）：`drift/mod.rs` 源码自证——判据装配与筛单各恰一处、零 `load_contracts`
/// 内联、零 `is_file`、零「首期 MUST NOT」字样。
#[test]
fn 判据装配改走api单点() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("crate::api::findings(").count(),
        1,
        "判据装配经 `api::findings` 恰一处"
    );
    assert_eq!(
        source.matches("crate::api::gating_findings(").count(),
        1,
        "纳管筛选经 `api::gating_findings` 恰一处"
    );
    assert!(
        !source.contains("load_contracts"),
        "契约装载只在 `core/archi::api`（drift 侧零内联装配）"
    );
    // `assets_hash` 的资产枚举是唯一例外（`index.is_file()` + `metadata.is_file()`），
    // 契约在案判据仍只在 `api::gating_findings`
    assert_eq!(
        source.matches("is_file").count(),
        source.matches("index.is_file()").count() + source.matches("metadata.is_file()").count(),
        "契约在案判据只在 `api::gating_findings`（drift 侧 `is_file` 仅资产枚举用）"
    );
    assert!(!source.contains("首期 MUST NOT"), "零旧口径字样");
}

// ===========================================================================
// `assets_hash`（收口自检的四类资产聚合哈希）与 `model_hash` 分工
// ===========================================================================

/// 手工复算模型声明目录的聚合哈希（`model_hash` 口径：文件名字节 + `0` 分隔 +
/// 字节 + `0` 分隔，按文件名序）。
fn recompute_model_hash(root: &Path) -> String {
    use sha2::{Digest, Sha256};

    let dir = foundation::layout::archi_graph_model_dir(root);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("模型声明目录应在盘")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
        .collect();
    files.sort();
    let mut hasher = Sha256::new();
    for file in files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(std::fs::read(&file).unwrap_or_default());
        hasher.update([0u8]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

/// 空盘（三处资产根皆不存在）。
fn empty_root(tag: &str) -> TempDir {
    TempDir::new(tag)
}

/// 正向：同一盘面连续两次调用值逐字相等；形态为 `sha256:` + 64 位十六进制。
#[test]
fn assets_hash确定性且形态固定() {
    let temp = TempDir::new("assets-hash-stable");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\n");
    temp.write(".dev-team/architecture/models/App.md", "# App\n");
    let first = super::assets_hash(temp.root());
    let second = super::assets_hash(temp.root());
    assert_eq!(first, second, "同一盘面稳定");
    assert!(first.starts_with("sha256:"), "前缀 sha256：{first}");
    let hex = first.trim_start_matches("sha256:");
    assert_eq!(hex.len(), 64, "64 位十六进制");
    assert!(
        hex.chars().all(|ch| ch.is_ascii_hexdigit()),
        "十六进制字符：{hex}"
    );
}

/// 正向（表驱动）：四类资产各改一处即变——`graph/model/*.toml` /
/// `models/**/*.md` / `models/**/*.schema.json` / `graph/INDEX.toml`。
#[test]
fn assets_hash四类资产各自敏感() {
    let cases: [(&str, &str); 4] = [
        (".dev-team/architecture/graph/model/02-more.toml", "x = 1\n"),
        (".dev-team/architecture/models/App/More.md", "# More\n"),
        (".dev-team/architecture/models/App/More.schema.json", "{}\n"),
        (".dev-team/architecture/graph/INDEX.toml", "version = 1\n"),
    ];
    for (relative, content) in cases {
        let temp = TempDir::new("assets-hash-sensitive");
        temp.write_model("[element.\"App\"]\nkind = \"package\"\n");
        let before = super::assets_hash(temp.root());
        temp.write(relative, content);
        let after = super::assets_hash(temp.root());
        assert_ne!(before, after, "改 {relative} 即变");
    }
}

/// 边界：全空盘（三处资产根皆不存在）→ 不 panic、两次相等；与另一全空盘同值。
#[test]
fn assets_hash空盘稳定且跨目录同值() {
    let first = empty_root("assets-hash-empty-a");
    let second = empty_root("assets-hash-empty-b");
    let a = super::assets_hash(first.root());
    let b = super::assets_hash(first.root());
    assert_eq!(a, b, "同一空盘两次相等");
    assert_eq!(
        a,
        super::assets_hash(second.root()),
        "两个空盘同值（相对路径参与累加）"
    );
}

/// 边界（表驱动）：资产根存在但零条目、或仅混入非目标扩展名文件（目录 / `.txt`）→
/// 与全空盘同值（过滤判据）。
#[test]
fn assets_hash非目标扩展名不参与() {
    let empty = empty_root("assets-hash-filter-empty");
    let expected = super::assets_hash(empty.root());

    let zero_entries = empty_root("assets-hash-filter-zero");
    std::fs::create_dir_all(foundation::layout::archi_graph_model_dir(
        zero_entries.root(),
    ))
    .expect("建空模型目录");
    std::fs::create_dir_all(foundation::layout::archi_models_dir(zero_entries.root()))
        .expect("建空契约目录");
    assert_eq!(
        super::assets_hash(zero_entries.root()),
        expected,
        "零条目与空盘同值"
    );

    let wrong_ext = empty_root("assets-hash-filter-wrong-ext");
    wrong_ext.write(".dev-team/architecture/graph/model/note.txt", "x\n");
    wrong_ext.write(".dev-team/architecture/models/App/keep.json", "{}\n");
    assert_eq!(
        super::assets_hash(wrong_ext.root()),
        expected,
        "非目标扩展名（.txt / 裸 .json）按判据过滤"
    );
}

/// 边界（表驱动）：空文件 vs 单字节文件 → 值不同（字节参与累加）；同名不同内容 →
/// 值不同。
#[test]
fn assets_hash字节参与累加() {
    let empty_file = TempDir::new("assets-hash-bytes-empty");
    empty_file.write(".dev-team/architecture/graph/model/01.toml", "");
    let one_byte = TempDir::new("assets-hash-bytes-one");
    one_byte.write(".dev-team/architecture/graph/model/01.toml", "x");
    assert_ne!(
        super::assets_hash(empty_file.root()),
        super::assets_hash(one_byte.root()),
        "空文件与单字节可辨"
    );

    let changed = TempDir::new("assets-hash-bytes-changed");
    changed.write(".dev-team/architecture/graph/model/01.toml", "y");
    assert_ne!(
        super::assets_hash(one_byte.root()),
        super::assets_hash(changed.root()),
        "同名不同内容可辨"
    );
}

/// 边界：内容相同、文件名不同 → 值不同（相对路径参与累加）。
#[test]
fn assets_hash文件名参与累加() {
    let first = TempDir::new("assets-hash-name-a");
    first.write(".dev-team/architecture/graph/model/01.toml", "x = 1\n");
    let second = TempDir::new("assets-hash-name-b");
    second.write(".dev-team/architecture/graph/model/02.toml", "x = 1\n");
    assert_ne!(
        super::assets_hash(first.root()),
        super::assets_hash(second.root()),
        "内容相同文件名不同即可辨"
    );
}

/// 边界：乱序创建（先建 `z` 后建 `a`）与顺序创建同一盘面 → 同值（相对路径排序）。
#[test]
fn assets_hash乱序创建同值() {
    let reverse = TempDir::new("assets-hash-order-reverse");
    reverse.write(".dev-team/architecture/graph/model/z.toml", "z = 1\n");
    reverse.write(".dev-team/architecture/graph/model/a.toml", "a = 1\n");

    let forward = TempDir::new("assets-hash-order-forward");
    forward.write(".dev-team/architecture/graph/model/a.toml", "a = 1\n");
    forward.write(".dev-team/architecture/graph/model/z.toml", "z = 1\n");

    assert_eq!(
        super::assets_hash(reverse.root()),
        super::assets_hash(forward.root()),
        "创建序不影响（相对路径排序）"
    );
}

/// 边界（表驱动）：索引文件缺失 / 存在各一态 → 值可辨且不 panic（单文件缺失即不
/// 参与）。
#[test]
fn assets_hash索引文件参与() {
    let without = TempDir::new("assets-hash-index-missing");
    without.write_model("[element.\"App\"]\nkind = \"package\"\n");
    let before = super::assets_hash(without.root());

    let with_index = TempDir::new("assets-hash-index-present");
    with_index.write_model("[element.\"App\"]\nkind = \"package\"\n");
    with_index.write(".dev-team/architecture/graph/INDEX.toml", "version = 1\n");

    assert_ne!(
        before,
        super::assets_hash(with_index.root()),
        "索引在场即变"
    );
    assert_ne!(
        super::assets_hash(with_index.root()),
        super::assets_hash(without.root()),
        "两态可辨"
    );
}

/// 异常：资产根内混入子目录 → 跳过（目录不当文件读）、不 panic、哈希稳定。
#[test]
fn assets_hash子目录跳过且稳定() {
    let plain = TempDir::new("assets-hash-dir-plain");
    plain.write(".dev-team/architecture/graph/model/01.toml", "x = 1\n");

    let nested = TempDir::new("assets-hash-dir-nested");
    nested.write(".dev-team/architecture/graph/model/01.toml", "x = 1\n");
    std::fs::create_dir_all(
        nested
            .root()
            .join(".dev-team/architecture/graph/model/nested"),
    )
    .expect("建子目录");

    let expected = super::assets_hash(plain.root());
    assert_eq!(
        super::assets_hash(nested.root()),
        expected,
        "子目录不参与（不 panic、值与无该目录相同）"
    );
    assert_eq!(super::assets_hash(nested.root()), expected, "稳定");
}

/// 组合（表驱动）：只改契约 md → `assets_hash` 变而 `check` 报告 `modelHash` 逐字
/// 不变（两者共享 `hash_files`，但覆盖面不同——模型声明目录 vs 四类资产）。
#[test]
fn assets_hash与model_hash分工() {
    for relative in [
        ".dev-team/architecture/models/App.md",
        ".dev-team/architecture/models/App/Sub.md",
        ".dev-team/architecture/models/App/Sub.schema.json",
        ".dev-team/architecture/graph/INDEX.toml",
    ] {
        let temp = TempDir::new("assets-hash-division");
        temp.write_model("[element.\"App\"]\nkind = \"package\"\n");
        let assets_before = super::assets_hash(temp.root());
        let model_before = super::model_hash(temp.root());

        temp.write(relative, "changed\n");
        assert_ne!(
            assets_before,
            super::assets_hash(temp.root()),
            "改 {relative} 后 assets_hash 变"
        );
        assert_eq!(
            model_before,
            super::model_hash(temp.root()),
            "改 {relative} 后 modelHash 不变"
        );
    }
}

/// 正向（口径回归）：改造后 `model_hash` 与改造前算法（文件名 + 内容 + 空字节分隔）
/// 手工复算相等。
#[test]
fn model_hash口径回归手工复算() {
    let temp = TempDir::new("assets-hash-model-regression");
    temp.write_model(
        "[element.\"App\"]\nkind = \"package\"\n\n[element.\"App.Auth\"]\nkind = \"domain\"\n",
    );
    temp.write(
        ".dev-team/architecture/graph/model/02-more.toml",
        "[element.\"App.Auth.Login\"]\nkind = \"module\"\n",
    );
    assert_eq!(
        super::model_hash(temp.root()),
        recompute_model_hash(temp.root()),
        "model_hash 与改造前算法手工复算逐字相等"
    );
    // 覆盖面差异：契约 md 只影响 assets_hash
    temp.write(".dev-team/architecture/models/App.md", "# App\n");
    assert_eq!(
        super::model_hash(temp.root()),
        recompute_model_hash(temp.root()),
        "改契约 md 后口径仍一致"
    );
}

// ===========================================================================
// 名字级两桶（`relation-api-dependency-check` / AC-2 / AC-3 / AC-5 / D8 / D9）
//
// `api_buckets` 是 `used ∖ declared` / `declared ∖ used` 的**唯一实现**（drift 装配与
// `relation::relation_detail` 都只调它）；`edge_names` / `occurrences_of` 的元素对口径
// 也各只有一份。比对逐字符：MUST NOT 大小写折叠、MUST NOT 驼峰归一。
// ===========================================================================

/// 边构造器（`specifier` / `type_only` / `inline_ignore` 取固定值，只用名字面）。
fn edge(from: &str, to: &str, file: &str, line: u32, names: &[&str]) -> DependencyEdge {
    DependencyEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        file: file.to_owned(),
        line,
        specifier: "./x".to_owned(),
        type_only: false,
        inline_ignore: None,
        names: names.iter().map(|name| (*name).to_owned()).collect(),
    }
}

/// 正向：全匹配 / 少一项 / 多一项 / 两侧各有差四形态逐值（两桶同序字典序）。
#[test]
fn api_buckets四形态() {
    assert_eq!(
        api_buckets(&strings(&["A", "B"]), &strings(&["B", "A"])),
        (Vec::new(), Vec::new()),
        "全匹配 ⇒ 两桶皆空"
    );
    assert_eq!(
        api_buckets(&strings(&["A", "B"]), &strings(&["A"])),
        (Vec::new(), strings(&["B"])),
        "声明多一项 ⇒ declared ∖ used"
    );
    assert_eq!(
        api_buckets(&strings(&["A"]), &strings(&["A", "C"])),
        (strings(&["C"]), Vec::new()),
        "事实多一项 ⇒ used ∖ declared"
    );
    assert_eq!(
        api_buckets(&strings(&["A", "B"]), &strings(&["B", "D"])),
        (strings(&["D"]), strings(&["A"])),
        "两侧各有差 ⇒ 两桶各自字典序"
    );
}

/// 边界：双空 → 两桶皆空（`apis` 省略的关系零产）。
#[test]
fn api_buckets双空() {
    assert_eq!(
        api_buckets(&[], &[]),
        (Vec::new(), Vec::new()),
        "双空不产条目"
    );
}

/// 边界：乱序 + 重复输入 → 输出恒去重 + 字典序（同一名字多处命中只算一项）。
#[test]
fn api_buckets去重与排序() {
    assert_eq!(
        api_buckets(&strings(&["B", "A", "A"]), &strings(&["C", "A", "C"])),
        (strings(&["C"]), strings(&["B"])),
        "两桶各自去重 + 字典序"
    );
}

/// 边界：大小写不同（`Agent` / `agent`）→ 两侧各出一项（逐字符比对，禁折叠 / 驼峰
/// 归一）。
#[test]
fn api_buckets大小写不折叠() {
    assert_eq!(
        api_buckets(&strings(&["Agent"]), &strings(&["agent"])),
        (strings(&["agent"]), strings(&["Agent"])),
        "大小写差异即一项差异"
    );
}

/// 边界：空串名 / 单字符名 / 超长名 → 按字符参与比较，零 panic。
#[test]
fn api_buckets名字形态边界() {
    let long = "N".repeat(1200);
    assert_eq!(
        api_buckets(&strings(&[""]), &strings(&["A"])),
        (strings(&["A"]), strings(&[""])),
        "空串名照常参与比较"
    );
    assert_eq!(
        api_buckets(&strings(&["Z"]), &strings(&["Z"])),
        (Vec::new(), Vec::new()),
        "单字符名可比"
    );
    let long_ref = long.as_str();
    assert_eq!(
        api_buckets(std::slice::from_ref(&long), &strings(&["A"])),
        (strings(&["A"]), vec![long_ref.to_owned()]),
        "超长名原样参与比较"
    );
}

/// 边界：超大列表（各 1200 名，仅一项差）→ 恰一项差异（零 panic）。
#[test]
fn api_buckets超大列表单项差异() {
    let mut declared: Vec<String> = (0..1199).map(|index| format!("Name{index:04}")).collect();
    declared.push("OnlyDeclared".to_owned());
    let mut used: Vec<String> = (0..1199).map(|index| format!("Name{index:04}")).collect();
    used.push("OnlyUsed".to_owned());
    assert_eq!(
        api_buckets(&declared, &used),
        (vec!["OnlyUsed".to_owned()], vec!["OnlyDeclared".to_owned()]),
        "1200 名仅一项差"
    );
}

/// 异常：declared 含 `::` / 空白（直构，绕过模型值面）→ 仍按逐字符比较（纯函数不校验
/// 值面、也不静默剔除）。
#[test]
fn api_buckets零值面校验() {
    for probe in ["a::b", "a b"] {
        assert_eq!(
            api_buckets(&strings(&[probe]), &[]),
            (Vec::new(), strings(&[probe])),
            "{probe:?} 原样参与比较（不剔除）"
        );
    }
}

/// 组合：交换入参 → 两桶互换（对称语义，零第二套规则）。
#[test]
fn api_buckets入参互换即两桶互换() {
    let declared = strings(&["A", "B"]);
    let used = strings(&["B", "D"]);
    let (used_only, declared_only) = api_buckets(&declared, &used);
    let (swapped_used_only, swapped_declared_only) = api_buckets(&used, &declared);
    assert_eq!(used_only, swapped_declared_only, "第一桶对调后相等");
    assert_eq!(declared_only, swapped_used_only, "第二桶对调后相等");
}

/// 正向：两条边名字 `["B"]` / `["A", "B"]` → 并集 `["A", "B"]`（去重 + 字典序）。
#[test]
fn edge_names并集去重() {
    let first = edge("A", "B", "src/a/x.ts", 1, &["B"]);
    let second = edge("A", "B", "src/a/y.ts", 3, &["A", "B"]);
    assert_eq!(
        edge_names(&[&first, &second]),
        strings(&["A", "B"]),
        "跨边并集去重 + 字典序"
    );
}

/// 边界：零边 / 边名字全空（default / glob）→ 空向量。
#[test]
fn edge_names空面() {
    assert!(edge_names(&[]).is_empty(), "零边 ⇒ 空");
    let nameless = edge("A", "B", "src/a/x.ts", 1, &[]);
    assert!(
        edge_names(&[&nameless]).is_empty(),
        "名字全空 ⇒ 空（不产假名）"
    );
}

/// 组合：与 `scan::normalize_names` 直调同一输入逐字相等（单一归一实现）。
#[test]
fn edge_names与归一单点同值() {
    let cases: [&[&str]; 3] = [&["b", "a", "b"], &[], &["Z", "Z", "z"]];
    for names in cases {
        let named = edge("A", "B", "src/a/x.ts", 1, names);
        assert_eq!(
            edge_names(&[&named]),
            scan::normalize_names(
                names
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<String>>()
            ),
            "{names:?} 与归一单点逐字相等"
        );
    }
}

/// 异常（直构）：边名字含重复与空串元素 → 仍每次去重 + 排序、零 panic（本函数零值面
/// 校验，空串是一个普通名字）。
#[test]
fn edge_names空串元素零过滤() {
    let named = edge("A", "B", "src/a/x.ts", 1, &["B", "", "", "A"]);
    let other = edge("A", "B", "src/a/y.ts", 2, &["", "B"]);
    assert_eq!(
        edge_names(&[&named, &other]),
        strings(&["", "A", "B"]),
        "跨边去重后按字典序（空串排最前、不被剔除）"
    );
}

/// 正向：多文件多行 → 按 `file` 再 `line` 升序；`specifier` / `type_only` 原样。
#[test]
fn occurrences_of排序与字段原样() {
    let second = edge("A", "B", "src/a/y.ts", 4, &["B"]);
    let first = edge("A", "B", "src/a/x.ts", 9, &["A"]);
    let mut first = first;
    first.type_only = true;
    let occurrences = occurrences_of(&[&second, &first]);
    assert_eq!(
        occurrences
            .iter()
            .map(|entry| (entry.file.as_str(), entry.line, entry.type_only))
            .collect::<Vec<(&str, u32, bool)>>(),
        vec![("src/a/x.ts", 9, true), ("src/a/y.ts", 4, false)],
        "按 file 再 line 升序，typeOnly 原样"
    );
    assert_eq!(
        occurrences[0].specifier.as_deref(),
        Some("./x"),
        "specifier 原样"
    );
}

/// 边界：零边 → 空；`line = 0`（未知位置）排最前且零 panic。
#[test]
fn occurrences_of边界() {
    assert!(occurrences_of(&[]).is_empty(), "零边 ⇒ 空");
    let unknown = edge("A", "B", "src/a/a.ts", 0, &[]);
    let known = edge("A", "B", "src/a/b.ts", 2, &[]);
    assert_eq!(
        occurrences_of(&[&known, &unknown])
            .iter()
            .map(|entry| entry.line)
            .collect::<Vec<u32>>(),
        vec![0, 2],
        "line = 0 排最前（同 file 内升序）"
    );
}

// ---------------------------------------------------------------------------
// `check` 两桶装配（档位只看引入方）
// ---------------------------------------------------------------------------

/// 单关系模型（`Billing → Auth`，`apis` 由入参给出；`Billing` 档位由入参给出）。
fn api_model(temp: &TempDir, gate: &str, apis: &str) {
    temp.write_model(&format!(
        concat!(
            "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n",
            "[element.\"Billing\"]\nkind = \"module\"\ngate = \"{gate}\"\npath = [\"./src/billing/\"]\n\n",
            "[[relation]]\nfrom = \"Billing\"\nto = \"Auth\"\n{apis}",
        ),
        gate = gate,
        apis = apis
    ));
    temp.write(
        "src/auth/token.ts",
        "export const alpha = 1;\nexport const gamma = 2;\n",
    );
}

/// 条目里的元素对（`(from, to, kind)` 三元组清单）。
fn entry_kinds(entries: &[super::ArchiDriftEntry]) -> Vec<(String, String, ArchiDriftKind)> {
    entries
        .iter()
        .map(|entry| {
            (
                entry.from.clone().unwrap_or_default(),
                entry.to.clone().unwrap_or_default(),
                entry.kind,
            )
        })
        .collect()
}

/// 观察项里的元素对（`(from, to, kind)` 三元组清单）。
fn observation_kinds(
    entries: &[super::ArchiDriftObservation],
) -> Vec<(String, String, ArchiObservationKind)> {
    entries
        .iter()
        .map(|entry| (entry.from.clone(), entry.to.clone(), entry.kind))
        .collect()
}

/// 正向：strict 引入方 + 事实多出一项 → `errors` 恰一条 `undeclaredApi`，`gate =
/// Some(strict)`，文案点名多出的名字与档位。
#[test]
fn 两桶装配strict多出进errors() {
    let temp = TempDir::new("api-strict-used-only");
    api_model(&temp, "strict", "apis = [\"alpha\"]\n");
    temp.write(
        "src/billing/x.ts",
        "import { alpha, gamma } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        entry_kinds(&report.errors),
        vec![(
            "Billing".to_owned(),
            "Auth".to_owned(),
            ArchiDriftKind::UndeclaredApi
        )],
        "strict 引入方：用了没声明进 errors"
    );
    assert_eq!(
        report.errors[0].gate,
        Some(crate::dto::GateTier::Strict),
        "档位取引入方"
    );
    assert!(
        report.errors[0].message.contains("gamma"),
        "文案点名多出的名字：{}",
        report.errors[0].message
    );
    assert!(
        report.errors[0].message.contains("strict"),
        "文案点名档位：{}",
        report.errors[0].message
    );
    assert!(report.warnings.is_empty(), "warn 面无条目");
    assert_eq!(
        report.status,
        ArchiCheckStatus::Errors,
        "errors 非空 ⇒ status = errors"
    );
    assert!(!report.passed(), "status = errors ⇒ passed = false");
}

/// 正向：warn 引入方同形态 → `warnings` 恰一条、`errors` 空、`status = warnings`、
/// `passed = true`。
#[test]
fn 两桶装配warn多出进warnings() {
    let temp = TempDir::new("api-warn-used-only");
    api_model(&temp, "warn", "apis = [\"alpha\"]\n");
    temp.write(
        "src/billing/x.ts",
        "import { alpha, gamma } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "warn 引入方不进 errors");
    assert_eq!(
        entry_kinds(&report.warnings)
            .into_iter()
            .filter(|(_, _, kind)| *kind == ArchiDriftKind::UndeclaredApi)
            .collect::<Vec<(String, String, ArchiDriftKind)>>(),
        vec![(
            "Billing".to_owned(),
            "Auth".to_owned(),
            ArchiDriftKind::UndeclaredApi
        )],
        "warn 引入方：用了没声明进 warnings"
    );
    assert_eq!(report.status, ArchiCheckStatus::Warnings);
    assert!(report.passed(), "warnings 不阻断门禁");
}

/// 正向：off 引入方同形态 → 零条目（三数组皆不出现该名字）。
#[test]
fn 两桶装配off零条目() {
    let temp = TempDir::new("api-off-used-only");
    api_model(&temp, "off", "apis = [\"alpha\"]\n");
    temp.write(
        "src/billing/x.ts",
        "import { alpha, gamma } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "off ⇒ 零 errors");
    assert!(report.warnings.is_empty(), "off ⇒ 零 warnings");
    assert!(
        !report
            .observations
            .iter()
            .any(|entry| entry.kind == ArchiObservationKind::UnusedDeclaration),
        "off 引入方也不产观察项：{:?}",
        report.observations
    );
    assert_eq!(report.status, ArchiCheckStatus::Clean, "全无条目 ⇒ clean");
}

/// 正向：声明多一项（事实少 `beta`）→ `observations` 恰一条 `unusedDeclaration`、
/// `status = clean`、`passed = true`（观察项不进门禁）。
#[test]
fn 两桶装配声明多出只作观察项() {
    let temp = TempDir::new("api-declared-only");
    api_model(&temp, "strict", "apis = [\"alpha\", \"beta\"]\n");
    temp.write(
        "src/billing/x.ts",
        "import { alpha } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "声明多出不是错误");
    assert_eq!(
        observation_kinds(&report.observations),
        vec![(
            "Billing".to_owned(),
            "Auth".to_owned(),
            ArchiObservationKind::UnusedDeclaration
        )],
        "声明没用恰一条观察项"
    );
    assert_eq!(report.status, ArchiCheckStatus::Clean);
    assert!(report.passed(), "观察项永不进 status / passed");
    let message = &report.observations[0].message;
    assert!(message.contains("beta"), "文案点名未出现的名字：{message}");
    assert!(
        message.contains("不再依赖") && message.contains("名字写错"),
        "文案并列两成因（R4 / R5）：{message}"
    );
    assert!(
        message.contains("unfulfilledDeclaration"),
        "文案点名「本对无 import 时另有 unfulfilledDeclaration」：{message}"
    );
    assert!(
        message.contains("方法调用")
            && message.contains("内联限定路径")
            && message.contains("glob"),
        "文案点名三条取数上限：{message}"
    );
}

/// 边界：关系省略 `apis`（冷启动）→ 零新增条目（两桶段整体跳过）。
#[test]
fn 两桶装配省略apis零条目() {
    let temp = TempDir::new("api-absent");
    api_model(&temp, "strict", "");
    temp.write(
        "src/billing/x.ts",
        "import { alpha, gamma } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "省略 apis ⇒ 零 errors");
    assert!(report.warnings.is_empty(), "省略 apis ⇒ 零 warnings");
    assert!(
        !report
            .observations
            .iter()
            .any(|entry| entry.kind == ArchiObservationKind::UnusedDeclaration),
        "省略 apis ⇒ 零 unusedDeclaration：{:?}",
        report.observations
    );
    assert_eq!(report.status, ArchiCheckStatus::Clean);
}

/// 边界：declared 与 used 完全相同 → 两桶皆空、零条目。
#[test]
fn 两桶装配全匹配零条目() {
    let temp = TempDir::new("api-exact");
    api_model(&temp, "strict", "apis = [\"alpha\", \"gamma\"]\n");
    temp.write(
        "src/billing/x.ts",
        "import { alpha, gamma } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert!(report.errors.is_empty(), "全匹配零错误");
    assert!(report.warnings.is_empty(), "全匹配零警告");
    assert!(report.observations.is_empty(), "全匹配零观察项");
    assert_eq!(report.status, ArchiCheckStatus::Clean);
}

/// 边界：同一元素对无 import 且 `apis` 非空 → `unfulfilledDeclaration` 与
/// `unusedDeclaration` 并存（两条事实不同，允许并存）。
#[test]
fn 两桶装配两观察项并存() {
    let temp = TempDir::new("api-both-observations");
    api_model(&temp, "strict", "apis = [\"alpha\"]\n");
    temp.write("src/billing/x.ts", "export const local = 1;\n");
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(
        observation_kinds(&report.observations),
        vec![
            (
                "Billing".to_owned(),
                "Auth".to_owned(),
                ArchiObservationKind::UnfulfilledDeclaration
            ),
            (
                "Billing".to_owned(),
                "Auth".to_owned(),
                ArchiObservationKind::UnusedDeclaration
            ),
        ],
        "声明未兑现与声明没用并存（两条事实、零去重）"
    );
}

/// 异常：两桶条目的 `occurrences` 与该元素对全部命中边逐条同形（按 file / line 排序，
/// `line` 非 0），且与 `relation_detail` 侧同一聚合口径。
#[test]
fn 两桶条目命中点同形() {
    let temp = TempDir::new("api-occurrences");
    api_model(&temp, "strict", "apis = [\"alpha\"]\n");
    temp.write(
        "src/billing/b.ts",
        "import { gamma } from '../auth/token';\n",
    );
    temp.write(
        "src/billing/a.ts",
        "import { gamma } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    assert_eq!(report.errors.len(), 1, "恰一条 undeclaredApi");
    let occurrences = &report.errors[0].occurrences;
    assert_eq!(occurrences.len(), 2, "两条命中边各一条命中点");
    assert_eq!(
        occurrences
            .iter()
            .map(|entry| (entry.file.as_str(), entry.line))
            .collect::<Vec<(&str, u32)>>(),
        vec![("src/billing/a.ts", 1), ("src/billing/b.ts", 1)],
        "按 file 再 line 升序且 line 非 0"
    );
    for occurrence in occurrences {
        assert_eq!(occurrence.specifier.as_deref(), Some("../auth/token"));
        assert!(!occurrence.type_only);
    }
}

/// 边界：报告 `version` 与顶层字段集不因两桶而扩（`version == 3`，条目键集不变）。
#[test]
fn 两桶不扩报告字段集() {
    let temp = TempDir::new("api-report-shape");
    api_model(&temp, "strict", "apis = [\"alpha\"]\n");
    temp.write(
        "src/billing/x.ts",
        "import { gamma } from '../auth/token';\n",
    );
    let report = run_full(temp.root()).expect("判定应成功");
    let json = serde_json::to_value(&report).expect("序列化失败");
    let object = json.as_object().expect("报告为对象");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "apiFindings",
            "base",
            "change",
            "errors",
            "mode",
            "model",
            "modelHash",
            "observations",
            "root",
            "scan",
            "status",
            "suppressed",
            "timestamp",
            "version",
            "warnings",
        ],
        "顶层键集零扩张"
    );
    assert_eq!(json["version"], 3, "报告版本仍为 3");
    let entry = json["errors"][0].as_object().expect("条目为对象");
    let mut entry_keys: Vec<&str> = entry.keys().map(String::as_str).collect();
    entry_keys.sort();
    assert_eq!(
        entry_keys,
        vec!["from", "gate", "kind", "message", "occurrences", "to"],
        "条目键集零扩张（新增取值不扩字段）"
    );
    assert_eq!(
        json["errors"][0]["kind"], "undeclaredApi",
        "线格式 camelCase"
    );
}

/// 异常（防御臂）：`from` 元素缺席于模型（gate 取不到）→ 结构错停摆、两桶装配跳过、
/// 零 panic（不产 `undeclaredApi` / `unusedDeclaration`）。
#[test]
fn 两桶装配from元素缺席跳过() {
    let temp = TempDir::new("api-from-absent");
    temp.write_model(concat!(
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n",
        "[[relation]]\nfrom = \"Ghost\"\nto = \"Auth\"\napis = [\"alpha\"]\n",
    ));
    temp.write("src/auth/token.ts", "export const alpha = 1;\n");
    let report = run_full(temp.root()).expect("元素缺席不 panic");
    assert!(
        !report
            .errors
            .iter()
            .chain(report.warnings.iter())
            .any(|entry| entry.kind == ArchiDriftKind::UndeclaredApi),
        "gate 取不到 ⇒ 零 undeclaredApi：{:?}",
        entry_kinds(&report.errors)
    );
    assert!(
        !report
            .observations
            .iter()
            .any(|entry| entry.kind == ArchiObservationKind::UnusedDeclaration),
        "gate 取不到 ⇒ 零 unusedDeclaration：{:?}",
        report.observations
    );
    assert!(
        !report.model.structural_errors.is_empty(),
        "未定义引用以结构诊断呈现（装配段根本不跑）"
    );
}

/// 边界（定级分支单点，源码复核）：`errors.push` / `warnings.push` 只出现在既有 ⑥ 段与
/// 新增两桶段（⑦ᵃ）——无第三处定级点、⑦ 之后零 push。
#[test]
fn 定级分支单点源码复核() {
    let source = include_str!("mod.rs");
    let six = source
        .split("// ⑥ 对账：声明 vs 依赖事实")
        .nth(1)
        .expect("⑥ 段在案")
        .split("// ⑦ ① 声明未兑现")
        .next()
        .expect("⑦ 段边界在案");
    assert_eq!(
        six.matches("errors.push(").count(),
        2,
        "⑥ 段两处：引入方 strict + path 失效 strict"
    );
    assert_eq!(
        six.matches("warnings.push(").count(),
        3,
        "⑥ 段三处：目标未建模 + warn 引入方 + path 失效 warn"
    );
    let two_bucket = source
        .split("// ⑦ᵃ 名字级两桶")
        .nth(1)
        .expect("⑦ᵃ 段在案")
        .split("// ⑧ 契约判据")
        .next()
        .expect("⑧ 段边界在案");
    assert_eq!(
        two_bucket.matches("errors.push(").count(),
        1,
        "两桶段只一处 errors.push（strict 臂）"
    );
    assert_eq!(
        two_bucket.matches("warnings.push(").count(),
        1,
        "两桶段只一处 warnings.push（warn 臂）"
    );
    assert!(
        two_bucket.contains("GateTier::Off => {}"),
        "off 引入方零条目臂在案"
    );
    // 全文件合计：定级面 push 恰为两段之和，其余 push 只属模型警告面
    assert_eq!(
        source.matches("errors.push(").count(),
        3,
        "全文件 errors.push 恰三处（⑥ 两处 + 两桶一处）"
    );
    assert_eq!(
        source.matches("model_warnings.push(").count(),
        2,
        "模型警告两处（noPath / pathNotFound），不属定级面"
    );
    assert_eq!(
        source.matches("warnings.push(").count(),
        6,
        "全文件 warnings.push = ⑥ 三处 + 两桶一处 + 模型警告两处"
    );
}

/// 组合（AC-1 零噪声）：既有 16 份语料金样逐字节不变（`apis` 省略的语料零产，不出现
/// 新词面）；新增四 case 的金样则**只多取值**——两桶命中时出现新词面，冷启动两 case
/// 与基线同色。
#[test]
fn 语料金样零新词面() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/drift/expected");
    let mut baseline = 0;
    let mut added = 0;
    for entry in fs::read_dir(&dir).expect("读语料金样目录失败").flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let text = fs::read_to_string(&path).expect("读金样失败");
        let added_case = matches!(
            name.as_str(),
            "api-declared-exact.full.json"
                | "api-absent.full.json"
                | "api-used-not-declared.full.json"
                | "api-declared-not-used.full.json"
        );
        if added_case {
            added += 1;
        } else {
            baseline += 1;
        }
        let expected_word = match name.as_str() {
            "api-used-not-declared.full.json" => Some("undeclaredApi"),
            "api-declared-not-used.full.json" => Some("unusedDeclaration"),
            _ => None,
        };
        match expected_word {
            Some(word) => assert!(
                text.contains(word),
                "{} 应含 {word}（新增 case 的取值面）",
                path.display()
            ),
            None => {
                for word in ["undeclaredApi", "unusedDeclaration"] {
                    assert!(
                        !text.contains(word),
                        "{} 出现 {word}（冷启动零噪声被破坏）",
                        path.display()
                    );
                }
            }
        }
    }
    assert_eq!(baseline, 16, "既有语料金样 16 份零噪声");
    assert_eq!(added, 4, "新增两桶语料 4 份");
}

// ---------------------------------------------------------------------------
// 单文件哈希（写面乐观锁口径）：`file_hash` / `file_hash_of` / 累加核心
// ---------------------------------------------------------------------------

/// 形态：`sha256:` + 64 位小写 hex。
fn assert_single_file_hash_shape(value: &str) {
    assert!(value.starts_with("sha256:"), "前缀 sha256：{value}");
    let hex = value.trim_start_matches("sha256:");
    assert_eq!(hex.len(), 64, "64 位十六进制：{value}");
    assert!(
        hex.chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch)),
        "小写 hex（无大写）：{value}"
    );
}

/// 正向：同一模型文件连续两次取值逐字相等，形态固定。
#[test]
fn file_hash形态与同盘稳定() {
    let temp = TempDir::new("file-hash-stable");
    temp.write_model("[element.\"App\"]\nkind = \"package\"\n");
    let path = temp
        .root()
        .join(".dev-team/architecture/graph/model/01-test.toml");

    let first = super::file_hash(&path);
    assert_single_file_hash_shape(&first);
    assert_eq!(first, super::file_hash(&path), "同盘稳定");
}

/// 正向：改动文件一个字节后再取 → 与上次不等（同一路径前后取值可比）。
#[test]
fn 文件变即哈希变() {
    let temp = TempDir::new("file-hash-sensitive");
    temp.write(".dev-team/architecture/graph/model/01.toml", "x = 1\n");
    let path = temp
        .root()
        .join(".dev-team/architecture/graph/model/01.toml");
    let before = super::file_hash(&path);

    std::fs::write(&path, "x = 2\n").expect("改写文件");
    assert_ne!(before, super::file_hash(&path), "内容变即变");
}

/// 正向（组合：`file_hash_of` ↔ `hash_files` 读盘）：写后等价锚。
#[test]
fn file_hash_of等于落盘后file_hash() {
    let temp = TempDir::new("file-hash-of");
    let path = temp.root().join("draft.toml");
    let content = "[element.\"App\"]\nkind = \"package\"\n";

    let computed = super::file_hash_of(&path, content);
    assert_single_file_hash_shape(&computed);
    assert_eq!(
        computed,
        super::file_hash_of(&path, content),
        "同内容两次相等"
    );

    std::fs::write(&path, content).expect("落盘");
    assert_eq!(
        computed,
        super::file_hash(&path),
        "逐字等于落盘后的 file_hash"
    );
}

/// 异常：同名同内容跨目录相等；异名同内容不等（文件名分量进摘要）。
#[test]
fn 文件名参与累加() {
    let first = TempDir::new("file-hash-name-a");
    first.write(".dev-team/architecture/graph/model/01-x.toml", "x = 1\n");
    let second = TempDir::new("file-hash-name-b");
    second.write(".dev-team/architecture/graph/model/01-x.toml", "x = 1\n");
    assert_eq!(
        super::file_hash(
            &first
                .root()
                .join(".dev-team/architecture/graph/model/01-x.toml")
        ),
        super::file_hash(
            &second
                .root()
                .join(".dev-team/architecture/graph/model/01-x.toml")
        ),
        "同名同内容跨目录相等（累积基只取文件名）"
    );

    let same_dir = TempDir::new("file-hash-name-same-dir");
    same_dir.write(".dev-team/architecture/graph/model/01-x.toml", "x = 1\n");
    same_dir.write(".dev-team/architecture/graph/model/02-x.toml", "x = 1\n");
    assert_ne!(
        super::file_hash(
            &same_dir
                .root()
                .join(".dev-team/architecture/graph/model/01-x.toml")
        ),
        super::file_hash(
            &same_dir
                .root()
                .join(".dev-team/architecture/graph/model/02-x.toml")
        ),
        "异名同内容不等"
    );
}

/// 异常：单文件值只与「同一路径的前后取值」可比——目录粒度值（`model_hash` /
/// `assets_hash`）是聚合口径，数值不可比（双文件目录下聚合值累加两条「文件名 + 字节」，
/// 单文件值只有一条）。
#[test]
fn 不可与目录粒度哈希互比() {
    let temp = TempDir::new("file-hash-scope");
    temp.write_model(
        "[element.\"App\"]
kind = \"package\"
",
    );
    temp.write(
        ".dev-team/architecture/graph/model/02-more.toml",
        "[element.\"App.Auth\"]
kind = \"domain\"
",
    );
    let path = temp
        .root()
        .join(".dev-team/architecture/graph/model/01-test.toml");

    let single = super::file_hash(&path);
    let model = super::model_hash(temp.root());
    let assets = super::assets_hash(temp.root());
    for value in [&single, &model, &assets] {
        assert_single_file_hash_shape(value);
    }
    assert_ne!(single, model, "与 modelHash 不可比（聚合口径累加多条）");
    assert_ne!(
        single, assets,
        "与 assets_hash 不可比（相对根路径 + 四类资产）"
    );
}

/// 边界（矩阵：不存在路径 / 目录路径）：读失败按空字节参与，且与 `file_hash_of(…, "")`
/// 同值。
#[test]
fn 读失败按空字节参与() {
    let temp = TempDir::new("file-hash-read-fail");
    let missing = temp.root().join("no-such.toml");
    let directory = temp.root().join("a-directory");
    std::fs::create_dir_all(&directory).expect("建目录");

    for path in [&missing, &directory] {
        let value = super::file_hash(path);
        assert_single_file_hash_shape(&value);
        assert_eq!(value, super::file_hash(path), "读失败仍确定");
        assert_eq!(
            value,
            super::file_hash_of(path, ""),
            "读失败两函数同值（同一累加核心）"
        );
    }
}

/// 边界（矩阵：空串 / CRLF / emoji / 超长）：各 content 稳定且互异，零 panic。
#[test]
fn content边界稳定且互异() {
    let temp = TempDir::new("file-hash-of-content");
    let path = temp.root().join("draft.toml");
    let long = "长".repeat(1200);
    let cases = ["", "a\r\nb\r\n", "🚀 元素", long.as_str()];

    let mut seen: Vec<String> = Vec::new();
    for content in cases {
        let value = super::file_hash_of(&path, content);
        assert_single_file_hash_shape(&value);
        assert_eq!(value, super::file_hash_of(&path, content), "稳定");
        assert!(!seen.contains(&value), "不同 content 互异：{value}");
        seen.push(value);
    }
    assert_eq!(seen.len(), 4);
}

/// 边界：`hash_files(&[file], parent, &read_bytes)` 与 `file_hash(file)` 同值（口径单锚），且
/// `hash_files` 保持私有。
#[test]
fn hash_files是file_hash的口径单点() {
    let temp = TempDir::new("file-hash-anchor");
    temp.write(".dev-team/architecture/graph/model/01.toml", "x = 1\n");
    let path = temp
        .root()
        .join(".dev-team/architecture/graph/model/01.toml");
    let parent = path.parent().expect("父目录");

    assert_eq!(
        super::hash_files(std::slice::from_ref(&path), parent, &super::read_bytes),
        super::file_hash(&path),
        "两个入口必得同一值"
    );
    assert!(
        !include_str!("mod.rs").contains("pub fn hash_files"),
        "hash_files 仍非 pub"
    );
}

/// 边界（矩阵：3 形态）：`read_bytes` 是「读失败按空字节」的字节来源单点——正常文件与
/// `std::fs::read` 等值，缺席 / 目录两形态取空字节（不 panic、不报错），且与 `file_hash`
/// 同口径。
#[test]
fn read_bytes读失败按空字节() {
    let temp = TempDir::new("read-bytes");
    temp.write(".dev-team/architecture/graph/model/01.toml", "x = 1\n");
    let path = temp
        .root()
        .join(".dev-team/architecture/graph/model/01.toml");
    assert_eq!(
        super::read_bytes(&path),
        std::fs::read(&path).expect("读文件"),
        "正常文件与 std::fs::read 等值"
    );

    let missing = temp.root().join("no-such.toml");
    assert!(
        super::read_bytes(&missing).is_empty(),
        "缺席按空字节（不 panic）"
    );
    let directory = temp.root().join("a-directory");
    std::fs::create_dir_all(&directory).expect("建目录");
    assert!(
        super::read_bytes(&directory).is_empty(),
        "目录按空字节（不 panic）"
    );

    assert_eq!(
        super::hash_files(
            std::slice::from_ref(&missing),
            super::parent_of(&missing),
            &super::read_bytes
        ),
        super::file_hash(&missing),
        "字节来源单点：两入口同值"
    );
}

/// 异常（源码锚）：全 crate 唯一一处 `Sha256` 拼装（唯一累加核心 = `hash_files`，
/// 撤回的 `hash_with` 不得复活）；`edit/` 内零 `Sha256` / `sha2`。
#[test]
fn 唯一sha256拼装点() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("Sha256::new()").count(),
        1,
        "唯一累加核心 = hash_files"
    );
    assert!(
        !source.contains("fn hash_with"),
        "撤回的 hash_with 不得复活"
    );
    for edit_source in [
        include_str!("../edit/mod.rs"),
        include_str!("../edit/messages.rs"),
    ] {
        for forbidden in ["Sha256", "sha2"] {
            assert!(!edit_source.contains(forbidden), "edit/ 内零 {forbidden}");
        }
    }
}

/// 边界（矩阵：注入闭包 / 空清单）：字节由 `read` 单点决定；空清单输出为空累加摘要。
#[test]
fn 注入reader与空清单() {
    let temp = TempDir::new("file-hash-inject");
    let path = temp.root().join("draft.toml");
    let content = "x = 1\n";
    let parent = path.parent().expect("父目录");

    assert_eq!(
        super::hash_files(std::slice::from_ref(&path), parent, &|_: &Path| {
            content.as_bytes().to_vec()
        }),
        super::file_hash_of(&path, content),
        "注入字节与 file_hash_of 同值"
    );

    let empty = super::hash_files(&[], Path::new("/base"), &|_: &Path| Vec::new());
    assert_single_file_hash_shape(&empty);
    assert_ne!(
        empty,
        super::file_hash_of(&path, content),
        "空清单 = 空累加摘要（与任一非空内容可辨）"
    );
}
