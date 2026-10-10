//! `verdict.rs`（evaluator verdict 解析器）的单元测试（test-design「verdict.rs
//! -> verdict_test.rs」节）：合法裸 JSON / 围栏代码块容忍 / verdict 值域封闭 /
//! report 长度门 / checklist 行解析 / 结构漂移显式失败 / 多余字段静默忽略
//!（evaluator 若回声 phase / attempt / skipped，walker 权威忽略）。
//! 无进程边界依赖：最终消息以字符串 fixture 内存构造（合法 / 围栏 / 漂移 /
//! 超长四族）——Mock策略：无 mock。

use crate::verdict::{parse_verdict, EvaluatorChecklist, MAX_REPORT_CHARS};
use workflow::model::Verdict;

/// 合法 checklist JSON（瘦身形状：仅 verdict / report / checklist 三键）。
const VALID_JSON: &str = r#"{
  "verdict": "pass",
  "report": "实现与变更清单一致",
  "checklist": [
    { "item": "写面五写操作齐备", "pass": true, "evidence": "phase_table/phase_next/phase_start/phase_log/backtrack 全部在位" },
    { "item": "spawn 不进 core", "pass": true, "evidence": "orchestration 零进程 spawn" }
  ]
}"#;

#[test]
fn 合法裸json解析逐字段相等且checklist落域类型() {
    let checklist = parse_verdict(VALID_JSON).expect("合法 JSON 应解析成功");

    // 逐字段相等（载荷换血承接——checklist 行落 workflow::model::ChecklistItem
    // 域类型，无视图镜像层）
    assert_eq!(checklist.verdict, Verdict::Pass);
    assert_eq!(checklist.report, "实现与变更清单一致");
    assert_eq!(checklist.checklist.len(), 2);
    let first = &checklist.checklist[0];
    assert_eq!(first.item, "写面五写操作齐备");
    assert!(first.pass);
    assert!(first.evidence.contains("phase_table"));
}

#[test]
fn 围栏代码块包裹与混杂叙述容忍提取解析成功() {
    // ```json 围栏包裹
    let fenced = format!("```json\n{VALID_JSON}\n```");
    let parsed = parse_verdict(&fenced).expect("围栏包裹应解析成功");
    assert_eq!(parsed.verdict, Verdict::Pass);

    // 围栏前后混杂叙述文本（evaluator 真实输出形态）
    let noisy =
        format!("评估完成，结论如下：\n\n```json\n{VALID_JSON}\n```\n\n以上为静态清单结论。");
    let parsed = parse_verdict(&noisy).expect("围栏 + 混杂叙述应解析成功");
    assert_eq!(parsed.report, "实现与变更清单一致");

    // 裸 JSON 兜底：无围栏但首尾夹叙述（首个 { 到末个 } 切片）
    let braced = format!("结论：\n{VALID_JSON}\n（完毕）");
    let parsed = parse_verdict(&braced).expect("裸 JSON 兜底应解析成功");
    assert_eq!(parsed.verdict, Verdict::Pass);
}

#[test]
fn verdict值域封闭_越界字符串err() {
    // pass / fail 两值可辨
    let pass = parse_verdict(VALID_JSON).expect("pass 解析");
    assert_eq!(pass.verdict, Verdict::Pass);
    let fail_json = VALID_JSON.replace("\"verdict\": \"pass\"", "\"verdict\": \"fail\"");
    let fail = parse_verdict(&fail_json).expect("fail 解析");
    assert_eq!(fail.verdict, Verdict::Fail);

    // 值域外字符串 → Err（封闭集，不臆测）
    let bogus = VALID_JSON.replace("\"verdict\": \"pass\"", "\"verdict\": \"warning\"");
    let err = parse_verdict(&bogus).expect_err("值域外应 Err");
    assert!(
        err.contains("结构漂移") || err.contains("verdict"),
        "封闭集拒绝记因，实际: {err}"
    );
}

#[test]
fn report超长拒绝_解析层先拒() {
    // report > 2000 字符 → Err 拒绝（解析层先拒、写面落账层再拒双闸）
    let long_report = "评".repeat(MAX_REPORT_CHARS + 1);
    let overflowing =
        format!(r#"{{ "verdict": "pass", "report": "{long_report}", "checklist": [] }}"#);
    let err = parse_verdict(&overflowing).expect_err("超长 report 应 Err");
    assert!(
        err.contains("超长") && err.contains("2000"),
        "长度门记因，实际: {err}"
    );

    // 恰 2000 字符通过（边界含端点）
    let exact = "评".repeat(MAX_REPORT_CHARS);
    let exact_json = format!(r#"{{ "verdict": "pass", "report": "{exact}", "checklist": [] }}"#);
    assert!(parse_verdict(&exact_json).is_ok(), "恰 2000 字符边界通过");
}

#[test]
fn checklist行解析_空数组与多行() {
    // 空数组
    let empty = parse_verdict(r#"{ "verdict": "pass", "report": "OK", "checklist": [] }"#)
        .expect("空 checklist 应解析");
    assert!(empty.checklist.is_empty());

    // 多行 item / pass / evidence
    let multi = parse_verdict(
        r#"{ "verdict": "fail", "report": "两条未过", "checklist": [
            { "item": "rust 套件全绿", "pass": true, "evidence": "cargo test 通过" },
            { "item": "前端套件全绿", "pass": false, "evidence": "2 用例失败" },
            { "item": "knip 无新增", "pass": true, "evidence": "报告零新增豁免" }
        ] }"#,
    )
    .expect("多行 checklist 应解析");
    assert_eq!(multi.checklist.len(), 3);
    assert_eq!(multi.checklist[1].pass, false);
    assert_eq!(multi.checklist[1].evidence, "2 用例失败");
}

#[test]
fn 多余字段静默忽略_walker权威盖戳() {
    // evaluator 若仍回声 phase / attempt / skipped（旧协议形态），serde 无
    // deny_unknown_fields → 静默忽略；phase / attempt / skipped 由 walker 盖戳
    let legacy = parse_verdict(
        r#"{ "phase": "implement", "attempt": 2, "verdict": "pass", "report": "OK", "checklist": [], "skipped": false }"#,
    )
    .expect("多余字段应静默忽略");
    // 结构收敛（编译期锚）：穷尽解构恰 verdict / report / checklist 三字段
    //——phase / attempt / skipped 无字段可落，回声值静默丢弃
    let EvaluatorChecklist {
        verdict,
        report,
        checklist,
    } = legacy;
    assert_eq!(verdict, Verdict::Pass);
    assert_eq!(report, "OK");
    assert!(checklist.is_empty());
}

#[test]
fn 结构漂移显式失败均err停给用户() {
    // 缺 verdict
    let no_verdict = r#"{ "report": "OK", "checklist": [] }"#;
    let err = parse_verdict(no_verdict).expect_err("缺 verdict 应 Err");
    assert!(err.contains("结构漂移"), "记因: {err}");

    // checklist 行缺 evidence
    let no_evidence = r#"{ "verdict": "pass", "report": "OK",
        "checklist": [ { "item": "项", "pass": true } ] }"#;
    let err = parse_verdict(no_evidence).expect_err("缺 evidence 应 Err");
    assert!(err.contains("结构漂移"), "记因: {err}");

    // 非 JSON 文本
    let err = parse_verdict("这不是 JSON，是一段话。").expect_err("非 JSON 应 Err");
    assert!(err.contains("JSON"), "载体缺失记因: {err}");

    // 纯叙述无 JSON（无 { } 可切片）
    let err = parse_verdict("评估完成，一切正常。").expect_err("纯叙述应 Err");
    assert!(err.contains("JSON"), "记因: {err}");

    // 空文本
    assert!(parse_verdict("").is_err(), "空消息显式缺席");
    assert!(parse_verdict("   \n\t").is_err(), "空白消息显式缺席");
}
