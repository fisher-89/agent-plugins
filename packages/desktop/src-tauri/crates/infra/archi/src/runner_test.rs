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

use crate::LocalArchiCheck;

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

// ===========================================================================
// 试点元素与契约字段面（`module-api-contract-format` / AC-13 / AC-16 / D18 / D23）
//
// `file_stem` 的实现体迁入 `runner.rs` 的 `impl LocalArchiCheck`（`report.rs` 零副本），
// 故它的**行为断言只在**本文件；`compose_text` / `write_report` 的可见性、生成物
// `.i.rs` 的签名见证与 `api_findings` 对 `passed` 的零影响也在此收口。
// ===========================================================================

use archi::drift::report::ArchiApiFindingKind;

/// 落一份契约 md（`<archi_api_dir>/<Domain>/<FQN>.md`；目录段取父 FQN 末段）。
fn write_contract(root: &Path, fqn: &str, content: &str) {
    let path = archi::api::paths::contract_path(root, fqn).expect("合法 FQN");
    let relative = path
        .strip_prefix(root)
        .expect("repo 相对路径")
        .to_string_lossy()
        .replace('\\', "/");
    write(root, &relative, content);
}

/// 单元素模型（含祖先链声明，避免 `orphanHierarchy` 结构错）。
fn declared_chain_model(fqn: &str, element_root: &str, api: &str) -> String {
    let segments: Vec<&str> = fqn.split('.').collect();
    let mut model = String::new();
    for (index, _segment) in segments.iter().enumerate() {
        let prefix = segments[..=index].join(".");
        let last = index + 1 == segments.len();
        let (kind, gate, path, api_line) = if last {
            (
                "module",
                "warn",
                format!("[\"{element_root}\"]"),
                format!("api = \"{api}\"\n"),
            )
        } else if index == 0 {
            ("package", "off", "[]".to_owned(), String::new())
        } else {
            ("domain", "off", "[]".to_owned(), String::new())
        };
        model.push_str(&format!(
            "[element.\"{prefix}\"]\nkind = \"{kind}\"\ngate = \"{gate}\"\n{api_line}path = {path}\n\n"
        ));
    }
    model
}

/// 与代码签名一致的试点契约（1 op `file_stem` + 1 类型）。
fn pilot_contract(fqn: &str) -> String {
    format!(
        "# {fqn}\n\n## 职能\n\n门禁执行体。\n\n## 操作\n\n### file_stem\n\n```json api-op\n\
         {{\"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"type\": \"string\"}}}}\n```\n\n## 类型\n\n```json api-types\n\
         {{\"LocalArchiCheck\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    )
}

/// 与代码签名**不一致**的契约（`file_stem` 的返回位声明为 `integer`）。
fn mismatched_contract(fqn: &str) -> String {
    format!(
        "# {fqn}\n\n## 操作\n\n### file_stem\n\n```json api-op\n\
         {{\"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"type\": \"integer\"}}}}\n```\n"
    )
}

/// 试点形态树（模型 + 入口 + 契约），返回（临时目录，试点 FQN）。
fn pilot_tree() -> (TempDir, &'static str) {
    let fqn = "App.Backend.Infra.ArchiRuntime";
    let temp = TempDir::new().expect("建临时目录失败");
    write_model(
        temp.path(),
        &declared_chain_model(fqn, "crates/infra/archi/", "declared"),
    );
    write(
        temp.path(),
        "crates/infra/archi/src/lib.rs",
        "pub struct LocalArchiCheck;\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String { String::new() }\n}\n",
    );
    write_contract(temp.path(), fqn, &pilot_contract(fqn));
    (temp, fqn)
}

/// 正向（AC-13）：`LocalArchiCheck::file_stem` 的紧凑形态（三个真实时间戳）——过滤出
/// ASCII 数字 + `T` + `Z`，无冒号 / 短横线。
#[test]
fn file_stem紧凑形态() {
    for (timestamp, expected) in [
        ("2026-10-20T08:31:12Z", "20261020T083112Z"),
        ("2026-10-31T00:00:00+00:00", "20261031T0000000000"),
        ("2026-10-20T08:31:12.1234567Z", "20261020T0831121234567Z"),
    ] {
        let stem = LocalArchiCheck::file_stem(timestamp);
        assert_eq!(stem, expected, "{timestamp} 的紧凑形态");
        assert!(!stem.contains(':'), "结果无冒号：{stem}");
        assert!(!stem.contains('-'), "结果无短横线：{stem}");
    }
    let with_offset = LocalArchiCheck::file_stem("2026-10-31T00:00:00+00:00");
    assert!(
        with_offset
            .chars()
            .all(|character| character.is_ascii_digit() || character == 'T' || character == 'Z'),
        "带时区偏移的输入也只留数字 / T / Z：{with_offset}"
    );
    assert!(
        with_offset.starts_with("20261031T"),
        "日期与时间段的数字原序保留：{with_offset}"
    );
}

/// 边界（AC-13）：空输入退化 `report`；零数字零 T/Z 的输入同样退化；单字符 `T` / `Z`
/// 不退化。
#[test]
fn file_stem空输入退化() {
    assert_eq!(LocalArchiCheck::file_stem(""), "report", "空串退化");
    assert_eq!(LocalArchiCheck::file_stem("abc"), "report", "零字符集退化");
    assert_eq!(
        LocalArchiCheck::file_stem("中文时间"),
        "report",
        "非 ASCII 退化"
    );
    assert_eq!(
        LocalArchiCheck::file_stem("\u{0}\n\t"),
        "report",
        "控制字符退化"
    );
    assert_eq!(
        LocalArchiCheck::file_stem("T"),
        "T",
        "有字符即不退化（判据只看字符集）"
    );
    assert_eq!(LocalArchiCheck::file_stem("Z"), "Z", "Z 单独在场亦不退化");
    assert_eq!(
        LocalArchiCheck::file_stem("2026t10z"),
        "202610",
        "小写 t / z 不在字符集内（剥掉）"
    );
    assert_eq!(
        LocalArchiCheck::file_stem("abcT123Zdef"),
        "T123Z",
        "非时间戳文本按字符集过滤（不做格式校验）"
    );
}

/// 边界（AC-13）：路径安全——任意输入下结果不含平台非法字符、超长输入不 panic
/// （纯字符过滤，不截断）。
#[test]
fn file_stem路径安全() {
    let inputs = [
        "2026-10-20T08:31:12Z",
        "abc/def\\ghi:jkl*mno?pqr\"stu<vwx>yz|",
        "2026-10-20T08:31:12Z 含 中文 与 emoji 🎯",
    ];
    for input in inputs {
        let stem = LocalArchiCheck::file_stem(input);
        for forbidden in [":", "/", "\\", "*", "?", "\"", "<", ">", "|"] {
            assert!(
                !stem.contains(forbidden),
                "{input:?} 的结果 {stem} MUST NOT 含 {forbidden}"
            );
        }
    }

    let long = "9".repeat(1200);
    let stem = LocalArchiCheck::file_stem(&long);
    assert_eq!(stem.len(), 1200, "超长纯数字全保留（不 panic、不截断）");
    let long_mixed = format!("{}T{}Z", "7".repeat(600), "8".repeat(600));
    let stem = LocalArchiCheck::file_stem(&long_mixed);
    assert_eq!(stem.len(), 1202, "超长混合输入的数字与 T / Z 全保留");
}

/// 边界（AC-16 / D23）：`file_stem` 已提升为 `pub` 且只有一处实现——`report.rs` 零
/// `fn file_stem` 定义；生成物 `.i.rs` 内的签名见证指向它（契约与实现的编译期绑定），
/// 且类型与构造器也**只**来自生成物。
#[test]
fn file_stem可见性与实现单点() {
    // `pub` 生效：可作函数值取用（签名与本 crate 外可见）
    let stem: fn(&str) -> String = LocalArchiCheck::file_stem;
    assert_eq!(stem("2026-10-20T08:31:12Z"), "20261020T083112Z");

    let runner = include_str!("runner.rs");
    assert!(
        runner.contains("pub fn file_stem(timestamp: &str) -> String"),
        "实现体在 runner.rs 的 impl 内且为 pub"
    );
    assert!(
        !include_str!("report.rs").contains("fn file_stem"),
        "report.rs 零 file_stem 定义（非复制）"
    );

    let generated = include_str!("../../archi.i.rs");
    assert!(
        generated.contains("const _: fn(&str) -> String = super::LocalArchiCheck::file_stem;"),
        "生成物内的签名见证指向 file_stem：{generated}"
    );
    assert!(
        generated
            .contains("const _: fn() -> super::LocalArchiCheck = super::LocalArchiCheck::new;"),
        "生成物内的构造器见证在案（签名不符即 E0308）"
    );
    assert!(
        generated.contains("pub struct LocalArchiCheck;"),
        "类型定义只来自生成物（手写侧无第二份）"
    );
    assert!(
        generated.contains("pub fn new() -> Self"),
        "构造器由生成物产出"
    );
    assert!(
        !runner.contains("pub struct LocalArchiCheck"),
        "手写侧零 `pub struct LocalArchiCheck` 定义（否则 E0255）"
    );
    assert!(
        !runner.contains("pub use"),
        "本地引用面零第二出口（判据串锚全串 `pub use`，非复合串）：\
         `runner.rs` 只以普通 `use crate::LocalArchiCheck;` 引入该名供本文件 `impl` 块解析，\
         `LocalArchiCheck` 由 `lib.rs` 的 `pub use api_gen::*;` 单点提供"
    );
    assert!(
        runner.contains("use crate::LocalArchiCheck;"),
        "正面锚在案：该名确以普通 `use` 引入（否定断言非恒真自证）"
    );
}

/// 异常（空转禁例 / AC-30 ④ / D38）：上一用例的否定判据 MUST **可判红**——把普通 `use`
/// 引入误写成带可见性前缀的形态，以及仅排版变化（前缀与路径分行）的形态漂移，都必须被
/// 同一判据捕获。本用例以**同一判据单独求值**证明其判据力：复合串形态对这两类注入为
/// **绿**（空转），全串形态为**红**。
#[test]
fn 全串pub_use判据可判红() {
    let runner = include_str!("runner.rs");

    // 注入按**行**求值（逐行前缀匹配）：以含换行的字面针做 `replacen` 会把行尾与空行
    // 纳入匹配面；逐行匹配只剩「行首前缀」一个自由度。
    fn inject(source: &str, replacement: &str) -> String {
        let needle = "use crate::LocalArchiCheck;";
        let mut hit = false;
        let mut out = String::with_capacity(source.len() + replacement.len());
        for line in source.lines() {
            if line.trim_start().starts_with(needle) && !line.contains("pub use") {
                out.push_str(replacement);
                hit = true;
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        assert!(hit, "注入前提：产品面以普通 `use` 引入该名");
        out
    }

    assert!(
        runner.contains("use crate::LocalArchiCheck;"),
        "产品面以普通 `use` 引入该名"
    );

    // ① 全形误写：复合串判据（旧形态）**绿**、全串判据（新形态）**红**
    let reexported = inject(runner, "pub use crate::LocalArchiCheck;");
    assert!(
        reexported.contains("pub use"),
        "判据力自证：全串 `pub use` 判据在本注入下必红（上一用例的断言即此判据）"
    );
    assert!(
        reexported
            .lines()
            .any(|line| line.starts_with("pub use crate::")),
        "注入形态落在行首（可被形态漂移观察到）"
    );

    // ② 仅排版漂移（前缀与路径分行）：复合串失配而全串仍命中
    let wrapped = inject(runner, "pub use \ncrate::LocalArchiCheck;");
    assert!(
        !wrapped.contains("pub use crate::LocalArchiCheck;"),
        "排版漂移令复合串失配 ⇒ 形态漂移对旧判据不可见"
    );
    assert!(
        wrapped.contains("pub use"),
        "排版漂移下全串判据仍命中 ⇒ 形态漂移可见"
    );

    // ③ 产品面零带前缀再导出：两处判据的被检文件 MUST 分列（生成物侧断言见
    //    `透传查表值经paths单点`，此处不动生成物）
    assert!(!runner.contains("pub use"), "工作树产品面本身零 `pub use`");
}

/// 边界（D23）：`compose_text` 保持私有、不进契约（生成物内零 `compose_text` 见证，
/// 源码内无可见性前缀）。
#[test]
fn compose_text保持私有() {
    let source = include_str!("runner.rs");
    assert!(
        source.contains("fn compose_text(report: &ArchiDriftReport"),
        "compose_text 在案"
    );
    assert!(
        !source.contains("pub fn compose_text") && !source.contains("pub(crate) fn compose_text"),
        "compose_text MUST NOT 带可见性前缀（私有实现细节）"
    );
    let generated = include_str!("../../archi.i.rs");
    assert!(
        !generated.contains("compose_text"),
        "生成物内零 compose_text 见证（不进契约）：{generated}"
    );
    assert!(
        !include_str!("lib.rs").contains("compose_text"),
        "lib.rs 不导出 compose_text"
    );
}

/// 边界（事实校准 11）：`write_report` 仍 `pub(crate)` 且不进契约（`lib.rs` 无
/// `pub use report::write_report`；生成物内零见证）。
#[test]
fn write_report不进契约() {
    assert!(
        include_str!("report.rs").contains("pub(crate) fn write_report("),
        "write_report 可见性零变化（pub(crate)）"
    );
    assert!(
        !include_str!("lib.rs").contains("write_report"),
        "lib.rs MUST NOT 导出 write_report（E0364 反例不在案）"
    );
    assert!(
        !include_str!("../../archi.i.rs").contains("write_report"),
        "crateOnly 可见性 ⟹ 不进契约"
    );
}

/// 正向（AC-13）：试点形态（模型 + 入口 + 契约 + 生成物）→ `run` 的 `passed == true`
/// 且报告 `api_findings` 为空（十判据零命中）。
#[test]
fn 试点元素判据零命中() {
    let (temp, _fqn) = pilot_tree();

    let outcome = run(
        temp.path(),
        "demo",
        vec!["crates/infra/archi/src/lib.rs".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    assert!(outcome.passed, "试点元素判据零命中 ⇒ passed");
    let reference = outcome.report_ref.expect("报告应落盘");
    let report = read_report(&reference);
    assert!(
        report.api_findings.is_empty(),
        "试点元素十判据零命中：{:?}",
        report
            .api_findings
            .iter()
            .map(|finding| (finding.kind, finding.message.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(report.version, 3, "报告版本 3（REPORT_VERSION）");
}

/// 正向（AC-6 / AC-13）：`run` 产出的报告 `api_findings` 只把 `status` 抬到 `warnings`
/// （契约在案元素的判据纳管），**不影响** `passed` / `diagnostics` 的结论面——契约不一致
/// 时槽位非空，`errors` 面与无契约形态逐值相等。
#[test]
fn api_findings不影响passed() {
    let fqn = "App.Backend.Infra.ArchiRuntime";

    // 树一：无契约目录（契约面未启用）
    let baseline = TempDir::new().expect("建临时目录失败");
    write_model(
        baseline.path(),
        &declared_chain_model(fqn, "crates/infra/archi/", "declared"),
    );
    write(
        baseline.path(),
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    let before = run(
        baseline.path(),
        "demo",
        vec!["crates/infra/archi/src/lib.rs".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    let before_report = read_report(&before.report_ref.clone().expect("报告应落盘"));

    // 树二：同一代码 + 不一致契约
    let with_contract = TempDir::new().expect("建临时目录失败");
    write_model(
        with_contract.path(),
        &declared_chain_model(fqn, "crates/infra/archi/", "declared"),
    );
    write(
        with_contract.path(),
        "crates/infra/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    write_contract(with_contract.path(), fqn, &mismatched_contract(fqn));
    let after = run(
        with_contract.path(),
        "demo",
        vec!["crates/infra/archi/src/lib.rs".to_owned()],
        CancelFlag::never(),
    )
    .expect("runner 不 Err");
    let after_report = read_report(&after.report_ref.clone().expect("报告应落盘"));

    assert!(before_report.api_findings.is_empty(), "无契约面 ⇒ 空槽位");
    assert!(
        !after_report.api_findings.is_empty(),
        "不一致契约 ⇒ 非空槽位：{:?}",
        after_report
            .api_findings
            .iter()
            .map(|finding| finding.kind)
            .collect::<Vec<_>>()
    );
    assert!(
        after_report
            .api_findings
            .iter()
            .any(|finding| finding.kind == ArchiApiFindingKind::ApiSignatureMismatch),
        "非空槽位由签名不符构成"
    );
    assert_eq!(
        before.passed, after.passed,
        "api_findings MUST NOT 影响 passed（D18 在门禁面的体现）"
    );
    // 文本面：两侧同属「通过」摘要（passed 同真的表达），差异只在 status 词——契约判据
    // 不逐条进反馈文本（逐条事实在报告文件里）
    assert!(
        before
            .diagnostics
            .contains("架构检查通过（mode=incremental status=clean"),
        "无契约面 ⇒ clean 摘要：{}",
        before.diagnostics
    );
    assert!(
        after
            .diagnostics
            .contains("架构检查通过（mode=incremental status=warnings"),
        "契约在案的判据把摘要词抬到 warnings：{}",
        after.diagnostics
    );
    assert!(
        !after.diagnostics.contains("apiSignatureMismatch"),
        "反馈文本零契约词汇（判据不逐条进 diagnostics）：{}",
        after.diagnostics
    );
    assert_eq!(
        before_report.status,
        ArchiCheckStatus::Clean,
        "无契约面 ⇒ clean"
    );
    assert_eq!(
        after_report.status,
        ArchiCheckStatus::Warnings,
        "纳管子集非空 ⇒ warnings（只有这一档被抬起）"
    );
    assert_eq!(before_report.errors, after_report.errors, "errors 面零变化");
}

/// 边界（D18）：`ArchiCheckOutcome` 的五字段面（`passed` / `diagnostics` / `fault` /
/// `report_dir` / `report_ref`）不含契约槽位——槽位只在报告文件里；`passed` 的派生
/// 单点只读 `errors` + 结构错（源码自证）。
#[test]
fn 槽位不进outcome且passed派生单点() {
    let source = include_str!("runner.rs");
    assert!(
        !source.contains("api_findings"),
        "runner.rs 内零 api_findings 引用（槽位由 drift 装配、只随报告落盘）"
    );
    assert!(
        source.contains("report.errors.is_empty() && report.model.structural_errors.is_empty()"),
        "passed 只由 errors + 结构错派生"
    );
    assert!(
        !source.contains("api_findings.is_empty()"),
        "passed MUST NOT 读契约槽位（D18 首期）"
    );
}

/// 边界（AC-16 零债务消解）：`ArchiCheckRunner` 实现体、`FIX_HINT` / `TRUNCATED` 常量
/// 与报告装配逻辑逐字未变——源码自证零新增判定分支、零第二套文本装配。
#[test]
fn 实现体与常量零新增分支() {
    let source = include_str!("runner.rs");
    for needle in [
        "const FIX_HINT: &str = \"先按模型改代码；若真实设计如此，应回到 dev-design 补模型声明，",
        "const TRUNCATED: &str = \"…（已截断，全量见报告文件）\";",
        "impl ArchiCheckRunner for LocalArchiCheck {",
        "let report_ref = write_report(root_path, &name, &report).ok();",
        "let passed = report.errors.is_empty() && report.model.structural_errors.is_empty();",
        "let diagnostics = compose_text(&report, report_ref.as_deref());",
    ] {
        assert!(source.contains(needle), "实现体逐字在案：{needle}");
    }
    // 四种 `status` 分支的文本装配在 `compose_text` 内单点（零第二处拼装）
    assert_eq!(
        source.matches("fn compose_text").count(),
        1,
        "compose_text 单点定义"
    );
    assert_eq!(
        source.matches("ArchiCheckStatus::").count(),
        5,
        "status 四分支 + 一处 use（零新增分支）"
    );
    assert!(
        !source.contains("api_findings") && !source.contains("ArchiApiFinding"),
        "runner.rs 零契约判据引用（零新增分支）"
    );
}

// ---------------------------------------------------------------------------
// 透传查表值经 `paths` 单点（D40 / D48）
// ---------------------------------------------------------------------------

/// 边界（**D40 / D48**）：透传查表值（`ModuleRef.name` / `module_path`）与 TS specifier
/// 一律取自 `paths` 的三个单点函数——执行面（`runner.rs` / `report.rs`）**零二次派生**
/// 与零 FQN 派生：本 crate 内零 `rsplit('.')` / 零 `replace('.', "::")`（把 FQN 点分段当
/// 语言路径即非法）；试点元素的生成物零透传条目（契约无 `api-reexport`），故
/// `crates/infra/archi.i.rs` 内零 `pub use`。
#[test]
fn 透传查表值经paths单点() {
    use archi::api::paths::{rust_module_path, ts_import_specifier, EntryForm};

    // ① 两值单点的逐字形态（真值源在本 crate 外，本 crate 只消费）
    assert_eq!(
        rust_module_path("crates/infra/archi"),
        "crate::archi",
        "Rust 透传模块路径取 `<root>` 末段（MUST NOT 用 FQN 点分段）"
    );
    assert_eq!(
        rust_module_path("crates/core/archi"),
        "crate::archi",
        "两个同名末段的不同 crate 给同一模块路径（跨 crate 父子另由 `apiNestedCrossCrate` 判）"
    );
    assert_eq!(
        ts_import_specifier(EntryForm::SubdirModule, "archi"),
        "../archi.i",
        "TS 目录形态 specifier"
    );
    assert_eq!(
        ts_import_specifier(EntryForm::SingleFile, "architecture.tsx"),
        "./architecture.tsx.i",
        "TS 单文件形态 specifier（带扩展名合法且必需）"
    );

    // ② 试点元素的生成物零透传条目（契约无 `api-reexport`）
    let pilot = include_str!("../../archi.i.rs");
    assert!(
        !pilot.contains("pub use "),
        "试点契约零透传条目 ⇒ 生成物零 `pub use`：\n{pilot}"
    );

    // ③ 执行面零二次派生：本 crate 零 FQN 点分段当语言路径的形态
    for (path, source) in [
        ("runner.rs", include_str!("runner.rs")),
        ("report.rs", include_str!("report.rs")),
    ] {
        assert!(
            !source.contains("rsplit('.')"),
            "`{path}` 内零 `rsplit('.')`（FQN 派生的第二套命名，D40 明禁）"
        );
        assert!(
            !source.contains("replace('.', \"::\")"),
            "`{path}` 内零 `replace('.', \"::\")`（点分段冒充 Rust 路径即非法）"
        );
    }
}
