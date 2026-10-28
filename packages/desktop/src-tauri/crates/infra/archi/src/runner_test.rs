//! `runner`（架构漂移门禁执行体）的单元测试：`check`（增量、`change = Some(name)`）
//! → `write_report` → `{ passed, diagnostics, fault, report_dir, report_ref }`；
//! 三态分派、`skipped` / `noChanges` 逐字文案、写盘失败降级、取消落 `fault`、反馈
//! 文本有界，以及「零第二套判定 / 零进程」的源码自证。
//!
//! 零 mock：真实 `archi::drift::check`（同源即被测语义，AC-10）+ 真实写盘 + 真实
//! `tempfile::TempDir`；失败面以「目录位置普通文件占位」注入（进程边界真实、零桩）。

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use archi::drift::{ArchiCheckStatus, ArchiDriftMode, ArchiDriftReport};
use foundation::cancel::CancelFlag;
use foundation::layout::{change_reports, ReportSlot};
use orchestration::port::{ArchiCheckOutcome, ArchiCheckRunner, RunFault, ToolStepOutput};
use orchestration::verdict::MAX_REPORT_CHARS;
use tempfile::TempDir;

use crate::runner::LocalArchiCheck;

// ---------------------------------------------------------------------------
// 最小执行器（本 crate 零 tokio 依赖，故手写 noop-waker block_on）
// ---------------------------------------------------------------------------

/// 单 future 阻塞执行：被测 future 全同步推进（零 IO 等待），poll 到 `Ready` 即返回。
fn block_on<F: Future>(future: F) -> F::Output {
    fn noop(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

/// 跑一次 runner（`Err` 面原样返回，产出类型漂移即断言失败）。
fn run(
    root: &Path,
    name: &str,
    changed: Vec<String>,
    cancel: CancelFlag,
) -> Result<ArchiCheckOutcome, String> {
    let runner = LocalArchiCheck::new();
    let output = block_on(runner.run(&root.to_string_lossy(), name, changed, cancel))?;
    match output {
        ToolStepOutput::ArchiCheck(outcome) => Ok(outcome),
        other => panic!("产出类型漂移（期望 ArchiCheck）：{other:?}"),
    }
}

/// 从 `ToolStepOutput` 取出 `ArchiCheck` 载荷（类型漂移即失败）。
fn unwrap_check(output: ToolStepOutput) -> ArchiCheckOutcome {
    match output {
        ToolStepOutput::ArchiCheck(outcome) => outcome,
        other => panic!("产出类型漂移（期望 ArchiCheck）：{other:?}"),
    }
}

/// 落一个 repo 相对文件（自动建父目录）。
fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("创建父目录失败");
    }
    fs::write(&path, content).expect("写文件失败");
}

/// 落模型。
fn write_model(root: &Path, content: &str) {
    write(root, ".dev-team/architecture/models/01-test.toml", content);
}

/// 报告目录（被测派生单点）。
fn report_dir(root: &Path, name: &str) -> PathBuf {
    change_reports(root, name, ReportSlot::Archi)
}

/// 落「strict 引入方 + off 被引入方 + 跨元素 import」夹具。
fn strict_fixture(root: &Path) {
    write_model(
        root,
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n",
    );
    write(root, "src/auth/token.ts", "export const token = 1;\n");
    write(
        root,
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
}

/// 从落盘报告读回结构（`report_ref` 必在位）。
fn read_report(report_ref: &str) -> ArchiDriftReport {
    serde_json::from_slice(&fs::read(report_ref).expect("读报告失败")).expect("反序列化失败")
}

// ---------------------------------------------------------------------------
// 正向：三态分派与全字段
// ---------------------------------------------------------------------------

/// 正向：clean 全字段——`passed`、有界摘要、`fault = None`、`report_dir` 派生、
/// `report_ref` 在位且文件存在。
#[test]
fn clean全字段() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_model(
        temp.path(),
        "[element.\"Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n",
    );
    write(
        temp.path(),
        "src/auth/token.ts",
        "export const token = 1;\n",
    );

    let outcome = run(
        temp.path(),
        "demo",
        vec!["src/auth/token.ts".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    assert!(outcome.passed, "clean ⇒ passed");
    assert!(!outcome.diagnostics.is_empty(), "摘要非空");
    assert!(
        !outcome.diagnostics.contains("未检查"),
        "检查过的结论不得写成「未检查」：{}",
        outcome.diagnostics
    );
    assert_eq!(outcome.fault, None, "生产路径 fault 恒 None");
    assert_eq!(
        outcome.report_dir,
        report_dir(temp.path(), "demo").to_string_lossy(),
        "report_dir 经 layout 槽位单点派生"
    );
    let reference = outcome.report_ref.expect("clean 应回填报告路径");
    assert!(Path::new(&reference).is_file(), "报告文件应真实存在");
    assert_eq!(
        read_report(&reference).status,
        ArchiCheckStatus::Clean,
        "报告 status 原样保留"
    );
}

/// 正向（AC-12 三态分派）：`warn` 档 → `passed`，摘要含「N 条提示」量词且不含
/// 固定处置提示。
#[test]
fn warnings档通过() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_model(
        temp.path(),
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n",
    );
    write(
        temp.path(),
        "src/auth/token.ts",
        "export const token = 1;\n",
    );
    write(
        temp.path(),
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );

    let outcome = run(
        temp.path(),
        "demo",
        vec!["src/billing/x.ts".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    assert!(outcome.passed, "warnings ⇒ passed");
    assert!(
        outcome.diagnostics.contains("1 条提示"),
        "摘要含量词：{}",
        outcome.diagnostics
    );
    assert!(
        !outcome.diagnostics.contains("回到 dev-design"),
        "通过态不附固定处置提示：{}",
        outcome.diagnostics
    );
}

/// 正向（AC-12 三态分派）：`strict` 违例 → `passed = false`，摘要含元素对 / 档位 /
/// 出现次数 / `file:line` / 固定处置提示。
#[test]
fn errors档失败() {
    let temp = TempDir::new().expect("建临时目录失败");
    strict_fixture(temp.path());
    let outcome = run(
        temp.path(),
        "demo",
        vec!["src/billing/x.ts".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    assert!(!outcome.passed, "errors ⇒ passed = false");
    let text = &outcome.diagnostics;
    assert!(text.contains("Billing → Auth"), "含元素对：{text}");
    assert!(text.contains("gate=strict"), "含引入方档位：{text}");
    assert!(text.contains("命中 1 处"), "含出现次数：{text}");
    assert!(
        text.contains("src/billing/x.ts:1"),
        "含至多两处 file:line：{text}"
    );
    assert!(
        text.contains("回到 dev-design"),
        "失败态含固定处置提示：{text}"
    );
}

/// 异常（AC-12）：模型含 `dependsOnAncestor` → `passed = false`，摘要含结构错的
/// file / 元素名，零 panic。
#[test]
fn 结构错拦截() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_model(
        temp.path(),
        "[element.\"Root\"]\nkind = \"package\"\npath = [\"./src/\"]\n\n[element.\"Root.Child\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n\n[[relation]]\nfrom = \"Root.Child\"\nto = \"Root\"\n",
    );
    write(
        temp.path(),
        "src/auth/token.ts",
        "export const token = 1;\n",
    );
    let outcome = run(temp.path(), "demo", Vec::new(), CancelFlag::never()).expect("runner 不 Err");
    assert!(!outcome.passed, "结构错 ⇒ passed = false");
    assert!(
        outcome.diagnostics.contains("结构错"),
        "摘要单列结构错：{}",
        outcome.diagnostics
    );
    assert!(
        outcome.diagnostics.contains("dependsOnAncestor"),
        "含结构错 kind：{}",
        outcome.diagnostics
    );
    assert!(
        outcome.diagnostics.contains("01-test.toml"),
        "含结构错 file：{}",
        outcome.diagnostics
    );
}

// ---------------------------------------------------------------------------
// 边界：skipped / noChanges 逐字文案
// ---------------------------------------------------------------------------

/// 边界（AC-13）：无 `<archi_dir>/models` → `passed = true` 且 detail **逐字**
/// 「未检查（无模型）」。
#[test]
fn skipped逐字文案() {
    let temp = TempDir::new().expect("建临时目录失败");
    write(temp.path(), "src/a.ts", "export const a = 1;\n");
    let outcome = run(temp.path(), "demo", Vec::new(), CancelFlag::never()).expect("runner 不 Err");
    assert!(outcome.passed, "skipped 不失败（不冒充检查过）");
    assert_eq!(
        outcome.diagnostics, "未检查（无模型）",
        "逐字文案：不附路径、不写「通过」"
    );
    assert_eq!(outcome.fault, None, "skipped 非执行面故障");
    let reference = outcome.report_ref.expect("skipped 也落报告");
    assert_eq!(
        read_report(&reference).status,
        ArchiCheckStatus::Skipped,
        "报告 status 原样保留"
    );
}

/// 边界（AC-13）：增量零变更 → `passed = true` 且 detail 逐字
/// 「未检查（无可判文件）」，报告 `status` 原样为 `noChanges`。
#[test]
#[allow(non_snake_case)]
fn noChanges逐字文案() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_model(
        temp.path(),
        "[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n",
    );
    write(temp.path(), "src/a.ts", "export const a = 1;\n");
    let outcome = run(temp.path(), "demo", Vec::new(), CancelFlag::never()).expect("runner 不 Err");
    assert!(outcome.passed, "noChanges 不失败");
    assert_eq!(
        outcome.diagnostics, "未检查（无可判文件）",
        "逐字文案（不附路径）"
    );
    let reference = outcome.report_ref.expect("noChanges 也落报告");
    let report = read_report(&reference);
    assert_eq!(
        report.status,
        ArchiCheckStatus::NoChanges,
        "报告 status 原样保留"
    );
    assert_eq!(report.scan.scanned_files, 0, "零可判文件");
}

// ---------------------------------------------------------------------------
// 正向：判定同源与变更集注入
// ---------------------------------------------------------------------------

/// 正向（AC-10 前置）：落盘报告与同输入 `drift::check` 直出逐字段相等（仅
/// `timestamp` 允许不同），且 `change` / `mode` / `base` 逐值。
#[test]
fn 报告与判定同源() {
    let temp = TempDir::new().expect("建临时目录失败");
    strict_fixture(temp.path());
    let changed = vec!["src/billing/x.ts".to_owned()];
    let outcome =
        run(temp.path(), "demo", changed.clone(), CancelFlag::never()).expect("runner 不 Err");
    let reference = outcome.report_ref.expect("报告应落盘");
    let mut written = read_report(&reference);

    let direct = archi::drift::check(archi::drift::CheckInput {
        root: temp.path(),
        mode: ArchiDriftMode::Incremental,
        changed: &changed,
        change: Some("demo"),
        cancel: CancelFlag::never(),
    })
    .expect("直出判定应成功");

    assert_eq!(written.change.as_deref(), Some("demo"), "change 为磁盘名");
    assert_eq!(written.mode, ArchiDriftMode::Incremental, "恒增量模式");
    assert_eq!(written.base.as_deref(), Some("HEAD"), "增量基准恒 HEAD");

    written.timestamp = direct.timestamp.clone();
    assert_eq!(
        written, direct,
        "除 timestamp 外逐字段相等（判定单源，无第二份实现）"
    );
}

/// 边界（AC-15）：变更集注入——`changed = [文件A]` → `scan.changed_files == 1` 且
/// 只判 A；`changed = []` → `noChanges`。
#[test]
fn 变更集注入() {
    let temp = TempDir::new().expect("建临时目录失败");
    strict_fixture(temp.path());
    let changed = vec!["src/billing/x.ts".to_owned()];
    let outcome = run(temp.path(), "demo", changed, CancelFlag::never()).expect("runner 不 Err");
    let report = read_report(&outcome.report_ref.expect("报告应落盘"));
    assert_eq!(report.scan.changed_files, 1, "变更集计数 1");
    assert_eq!(report.scan.scanned_files, 1, "只判变更集内文件");
    assert_eq!(report.errors.len(), 1, "变更集内的违例照报");

    let no_change =
        run(temp.path(), "demo", Vec::new(), CancelFlag::never()).expect("runner 不 Err");
    let report = read_report(&no_change.report_ref.expect("报告应落盘"));
    assert_eq!(
        report.status,
        ArchiCheckStatus::NoChanges,
        "零变更集 ⇒ noChanges（不降级为假通过）"
    );
}

// ---------------------------------------------------------------------------
// 边界：反馈文本有界
// ---------------------------------------------------------------------------

/// 边界（AC-12）：12 条违例（其中一条聚合 40 处命中）→ 文本长度 ≤
/// `MAX_REPORT_CHARS`，每条至多两处 `file:line`，全量明细只可从报告文件回查。
#[test]
fn 反馈文本有界() {
    let temp = TempDir::new().expect("建临时目录失败");
    let mut model = String::new();
    for index in 0..12 {
        model.push_str(&format!(
            "[element.\"Source{index}\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/s{index}/\"]\n\n"
        ));
        model.push_str(&format!(
            "[element.\"Target{index}\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/t{index}/\"]\n\n"
        ));
    }
    write_model(temp.path(), &model);
    for index in 0..12 {
        write(
            temp.path(),
            &format!("src/t{index}/y.ts"),
            "export const y = 1;\n",
        );
        let fan_out = if index == 0 { 40 } else { 1 };
        for file in 0..fan_out {
            write(
                temp.path(),
                &format!("src/s{index}/f{file:02}.ts"),
                &format!("import {{ y }} from '../t{index}/y';\n"),
            );
        }
    }

    let mut changed: Vec<String> = Vec::new();
    for index in 0..12 {
        let fan_out = if index == 0 { 40 } else { 1 };
        for file in 0..fan_out {
            changed.push(format!("src/s{index}/f{file:02}.ts"));
        }
    }
    let outcome = run(temp.path(), "demo", changed, CancelFlag::never()).expect("runner 不 Err");
    assert!(!outcome.passed, "12 条违例 ⇒ 失败");
    assert!(
        outcome.diagnostics.chars().count() <= MAX_REPORT_CHARS,
        "文本长度 {} 超过上限 {MAX_REPORT_CHARS}",
        outcome.diagnostics.chars().count()
    );
    assert!(
        outcome.diagnostics.contains("另 38 处"),
        "聚合条目的余量以「另 N 处」收口：{}",
        outcome.diagnostics
    );
    let report = read_report(&outcome.report_ref.expect("全量明细在报告文件"));
    assert_eq!(report.errors.len(), 12, "报告保留全量条目");
    assert_eq!(
        report.errors[0].occurrences.len(),
        40,
        "聚合条目的 40 处命中点全量在报告内"
    );
}

/// 边界：单条 `message` > 1000 chars（超长 FQN）→ 文本仍截断到上限内且不 panic
/// （不出现 UTF-8 边界截断崩）。
#[test]
fn 超长单条message() {
    let temp = TempDir::new().expect("建临时目录失败");
    // 扁平原名（无点 ⇒ 无层级，避免 orphanHierarchy 抢先停摆）
    let long_fqn = "长".repeat(1200);
    write_model(
        temp.path(),
        &format!(
            "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"{long_fqn}\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/long/\"]\n"
        ),
    );
    write(
        temp.path(),
        "src/auth/token.ts",
        "export const token = 1;\n",
    );
    write(
        temp.path(),
        "src/long/x.ts",
        "import { token } from '../auth/token';\n",
    );

    let outcome = run(
        temp.path(),
        "demo",
        vec!["src/long/x.ts".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    assert!(!outcome.passed, "超长元素名的违例仍失败");
    assert!(
        outcome.diagnostics.chars().count() <= MAX_REPORT_CHARS,
        "超长文本仍截断到上限内"
    );
    assert!(
        outcome.diagnostics.ends_with("（已截断，全量见报告文件）"),
        "截断留痕尾注在文本末尾（实际长度 {}）",
        outcome.diagnostics.chars().count()
    );
}

// ---------------------------------------------------------------------------
// 异常：写盘失败降级与取消
// ---------------------------------------------------------------------------

/// 异常（AC-14）：报告目录位置被普通文件占位 → `Ok`（不 `Err`）、`passed` 与
/// `diagnostics` 与正常一致、`report_ref == None`、`report_dir` 照常。
#[test]
fn 写盘失败降级() {
    let temp = TempDir::new().expect("建临时目录失败");
    strict_fixture(temp.path());
    let blocking = report_dir(temp.path(), "demo");
    fs::create_dir_all(blocking.parent().expect("父目录")).expect("建父目录失败");
    fs::write(&blocking, "占位普通文件\n").expect("写占位文件失败");

    let outcome = run(
        temp.path(),
        "demo",
        vec!["src/billing/x.ts".to_owned()],
        CancelFlag::never(),
    )
    .expect("降级不返回 Err");
    assert!(!outcome.passed, "结论保留（strict 违例仍失败）");
    assert!(
        outcome.diagnostics.contains("Billing → Auth"),
        "违例明细保留：{}",
        outcome.diagnostics
    );
    assert_eq!(outcome.report_ref, None, "落盘失败 ⇒ report_ref 为 None");
    assert_eq!(
        outcome.report_dir,
        blocking.to_string_lossy(),
        "report_dir 照常为预期目录"
    );
}

/// 异常（AC-13 / D18）：`cancel` 预置 → `Ok(outcome)` 且 `fault ==
/// Some(RunFault::Stopped)`（取消不以 `Err` 表达）。
#[test]
fn 取消落fault() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_model(
        temp.path(),
        "[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n",
    );
    write(temp.path(), "src/a.ts", "export const a = 1;\n");
    let cancel = CancelFlag::never();
    cancel.handle().set();
    let outcome = run(temp.path(), "demo", Vec::new(), cancel).expect("取消不返回 Err");
    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "取消落 fault（正常返回）"
    );
    assert!(!outcome.passed, "取消不冒充通过");
    assert_eq!(outcome.report_ref, None, "取消不产报告");
}

// ---------------------------------------------------------------------------
// 边界：零进程 / 零第二套判定（源码自证）
// ---------------------------------------------------------------------------

/// 边界（AC-14 零进程）：clean / warnings / errors / skipped 四态 → `fault` 全
/// `None`；`runner.rs` 源码不含任何进程创建面。
#[test]
#[allow(non_snake_case)]
fn 生产路径fault恒None且零进程() {
    let clean = TempDir::new().expect("建临时目录失败");
    write_model(
        clean.path(),
        "[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n",
    );
    write(clean.path(), "src/a.ts", "export const a = 1;\n");

    let warnings = TempDir::new().expect("建临时目录失败");
    write_model(
        warnings.path(),
        "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n[element.\"Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n",
    );
    write(
        warnings.path(),
        "src/auth/token.ts",
        "export const token = 1;\n",
    );
    write(
        warnings.path(),
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );

    let errors = TempDir::new().expect("建临时目录失败");
    strict_fixture(errors.path());

    let skipped = TempDir::new().expect("建临时目录失败");
    write(skipped.path(), "src/a.ts", "export const a = 1;\n");

    for (tag, root, changed) in [
        ("clean", clean.path(), vec!["src/a.ts".to_owned()]),
        (
            "warnings",
            warnings.path(),
            vec!["src/billing/x.ts".to_owned()],
        ),
        ("errors", errors.path(), vec!["src/billing/x.ts".to_owned()]),
        ("skipped", skipped.path(), vec!["src/a.ts".to_owned()]),
    ] {
        let outcome = run(root, "demo", changed, CancelFlag::never())
            .unwrap_or_else(|error| panic!("{tag} 应正常返回：{error}"));
        assert_eq!(outcome.fault, None, "{tag} 态 fault 恒 None（零进程）");
    }

    let source = include_str!("runner.rs");
    for needle in ["std::process", "Command::new", "tokio::process"] {
        assert!(
            !source.contains(needle),
            "runner 源码 MUST NOT 含进程创建面 {needle}"
        );
    }
}

/// 边界（AC-10）：`runner.rs` 零归属 / 定级 / `status` 推导实现——`status` 只从
/// 报告读取，`passed` 只由 `errors` + 结构错派生。
#[test]
fn 零第二套判定() {
    let source = include_str!("runner.rs");
    for forbidden in ["owner_of", "ArchiDriftKind", "derive_status", "OwnerIndex"] {
        assert!(
            !source.contains(forbidden),
            "runner 源码 MUST NOT 含判定面实现 {forbidden}"
        );
    }
    assert!(
        source.contains("check(archi::drift::CheckInput"),
        "判定必须经 archi::drift::check 单点"
    );
    assert!(
        source.contains("report.errors.is_empty()")
            && source.contains("structural_errors.is_empty()"),
        "passed 只由 errors + 结构错派生"
    );
}

// ---------------------------------------------------------------------------
// 边界：构造面 / 根边界 / 槽位隔离
// ---------------------------------------------------------------------------

/// 边界：`LocalArchiCheck::new()` 与 `default()` 行为等价（同输入同 outcome 的
/// 结论面），零参构造不触盘、不 panic。
#[test]
fn 构造面等价() {
    let temp = TempDir::new().expect("建临时目录失败");
    strict_fixture(temp.path());
    let by_new = block_on(LocalArchiCheck::new().run(
        &temp.path().to_string_lossy(),
        "demo",
        vec!["src/billing/x.ts".to_owned()],
        CancelFlag::never(),
    ))
    .expect("new 不 Err");
    let by_default = block_on(<LocalArchiCheck as Default>::default().run(
        &temp.path().to_string_lossy(),
        "demo",
        vec!["src/billing/x.ts".to_owned()],
        CancelFlag::never(),
    ))
    .expect("default 不 Err");

    let ArchiCheckOutcome {
        passed: new_passed,
        fault: new_fault,
        report_dir: new_dir,
        ..
    } = unwrap_check(by_new);
    let ArchiCheckOutcome {
        passed: default_passed,
        fault: default_fault,
        report_dir: default_dir,
        ..
    } = unwrap_check(by_default);
    assert_eq!(new_passed, default_passed, "结论一致");
    assert_eq!(new_fault, default_fault, "fault 一致");
    assert_eq!(new_dir, default_dir, "报告目录一致");
}

/// 边界：`root` 指向不存在路径 → `Ok`（skipped）且不 panic；`name` 空串 → 报告
/// 目录派生结果可判。
#[test]
fn 根边界() {
    let temp = TempDir::new().expect("建临时目录失败");
    let missing = temp.path().join("no-such-root");
    let outcome = run(&missing, "demo", Vec::new(), CancelFlag::never()).expect("不 panic");
    assert!(outcome.passed, "无模型 ⇒ skipped ⇒ 不失败");
    assert_eq!(outcome.diagnostics, "未检查（无模型）", "skipped 逐字文案");

    let named = TempDir::new().expect("建临时目录失败");
    write_model(
        named.path(),
        "[element.\"App\"]\nkind = \"package\"\npath = [\"./src/\"]\n",
    );
    write(named.path(), "src/a.ts", "export const a = 1;\n");
    let outcome = run(named.path(), "", Vec::new(), CancelFlag::never()).expect("不 panic");
    assert!(
        outcome
            .report_dir
            .starts_with(&*named.path().to_string_lossy()),
        "空 name 的目录派生仍在 root 内：{}",
        outcome.report_dir
    );
}

/// 边界（AC-14）：一次 `run` 后 `reports/test` / `reports/baseline` 零创建（与
/// `report_test` 的写盘面单点交叉闭合）。
#[test]
fn 槽位隔离() {
    let temp = TempDir::new().expect("建临时目录失败");
    strict_fixture(temp.path());
    let _ = run(
        temp.path(),
        "demo",
        vec!["src/billing/x.ts".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    assert!(
        !change_reports(temp.path(), "demo", ReportSlot::Test).exists(),
        "reports/test 零创建"
    );
    assert!(
        !change_reports(temp.path(), "demo", ReportSlot::Baseline).exists(),
        "reports/baseline 零创建"
    );
    assert!(
        report_dir(temp.path(), "demo").is_dir(),
        "只创建 reports/archi"
    );
}
