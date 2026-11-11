//! `verdict.rs`（checklist 校验器）的单元测试：`validate_checklist` 的形状校验、
//! ID 三分类（缺项 / 清单外 ID / 未知 ID）、`id → item` 映射、report 上限与
//! verdict 推导。文本解析面（`parse_verdict` / `extract_json` / 围栏 / brace 切片）
//! 随 `parse_verdict` 退役——被测函数已删除，本节零对应用例。
//!
//! 不 mock：`args` 为内存 `serde_json::json!` 字面量，`prompt` 为内存合成静态
//! 清单字符串；`checklist_entries` / `derive_verdict` 真实调用（单点语义即被测
//! 面）；零文件 / 网络 / DB / 子进程 / 环境变量。

use workflow::model::Verdict;

use crate::verdict::{validate_checklist, EvaluatorChecklist, MAX_REPORT_CHARS};

// ---------------------------------------------------------------------------
// 装置：合成静态清单 prompt + 逐项参数构造
// ---------------------------------------------------------------------------

/// 两行静态清单（C1 / C2）prompt。
fn prompt_c1_c2() -> String {
    "## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n\
     | C1 | 变更代码中无安全漏洞 | 检查：SQL注入、XSS（全角括号） |\n\
     | C2 | 变更代码有对应测试 | 检查测试文件存在 |\n"
        .to_owned()
}

/// 单项参数（`id` / `pass` / `evidence`）。
fn item_args(id: &str, pass: bool, evidence: &str) -> serde_json::Value {
    serde_json::json!({ "id": id, "pass": pass, "evidence": evidence })
}

/// 全项参数（report 固定为短串）。
fn args_with_items(items: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({ "report": "报告", "items": items })
}

/// 两行 prompt 的全覆盖合法参数（`pass_c1` / `pass_c2` 控制两项结论）。
fn full_args(pass_c1: bool, pass_c2: bool) -> serde_json::Value {
    args_with_items(vec![
        item_args("C1", pass_c1, "甲证据"),
        item_args("C2", pass_c2, "乙证据"),
    ])
}

/// 源码自证读产品源文件（`CARGO_MANIFEST_DIR` 相对）。
fn source_of(relative: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("读取 {relative} 失败: {error}"))
}

// ---------------------------------------------------------------------------
// 正向：全链路与 verdict 两态
// ---------------------------------------------------------------------------

#[test]
fn 全链路正向_item映射md正文且verdict由推导且report保真() {
    let prompt = prompt_c1_c2();
    let checklist: EvaluatorChecklist =
        validate_checklist(&full_args(true, false), &prompt).expect("全覆盖合法参数应通过");

    assert_eq!(checklist.checklist.len(), 2, "两项全收");
    assert_eq!(
        checklist.checklist[0].item, "变更代码中无安全漏洞",
        "item 映射为 md 第二列正文（非 ID）"
    );
    assert_ne!(checklist.checklist[0].item, "C1", "item 不等于 ID");
    assert_eq!(checklist.checklist[1].item, "变更代码有对应测试");
    assert_eq!(checklist.report, "报告", "report 原文保真");
    assert_eq!(
        checklist.verdict,
        Verdict::Fail,
        "含 fail 项 → 推导 Fail（不是模型提交值）"
    );
}

#[test]
fn 全pass两态可辨_推导为pass() {
    let prompt = prompt_c1_c2();
    let checklist = validate_checklist(&full_args(true, true), &prompt).expect("全 pass 应通过");
    assert_eq!(checklist.verdict, Verdict::Pass, "全 pass → 推导 Pass");
    assert!(
        checklist.checklist.iter().all(|item| item.pass),
        "逐项 pass 原样保留"
    );
}

#[test]
fn item映射按id对位_含全角标点时逐字保真() {
    let prompt = prompt_c1_c2();
    // 参数序与清单序相反：映射按 ID 对位，不按顺序猜
    let args = args_with_items(vec![
        item_args("C2", true, "乙证据"),
        item_args("C1", true, "甲证据"),
    ]);
    let checklist = validate_checklist(&args, &prompt).expect("逆序参数应通过");
    assert_eq!(
        checklist.checklist[0].item, "变更代码有对应测试",
        "首项按 C2 对位（不按清单首行猜）"
    );
    assert_eq!(
        checklist.checklist[1].item, "变更代码中无安全漏洞",
        "次项按 C1 对位"
    );
    assert_eq!(checklist.checklist[0].evidence, "乙证据", "evidence 原样");
}

// ---------------------------------------------------------------------------
// 异常：ID 三分类（缺项 / 清单外 ID / 未知 ID）
// ---------------------------------------------------------------------------

#[test]
fn 缺项err_自足三要素且不返回部分checklist() {
    let prompt = prompt_c1_c2();
    let args = args_with_items(vec![item_args("C1", true, "甲")]);
    let error = validate_checklist(&args, &prompt).expect_err("缺项应 Err");

    assert!(error.contains("items"), "文案含字段名 items: {error}");
    assert!(error.contains("C2"), "文案含缺失 ID C2: {error}");
    assert!(
        error.contains("缺项"),
        "文案标明缺项分类（与另两类分列）: {error}"
    );
    assert!(
        error.contains("C1"),
        "文案含期望集合与实收集合（C1 在两集合内）: {error}"
    );
    assert!(
        !error.contains("清单外"),
        "缺项文案不得含清单外分类词（互斥）: {error}"
    );
}

#[test]
fn 清单外id_err分列且不含缺项文案() {
    let prompt = prompt_c1_c2();
    let args = args_with_items(vec![
        item_args("C1", true, "甲"),
        item_args("C2", true, "乙"),
        item_args("C9", true, "丙"),
    ]);
    let error = validate_checklist(&args, &prompt).expect_err("清单外 ID 应 Err");

    assert!(error.contains("C9"), "文案含清单外 ID: {error}");
    assert!(
        error.contains("C1") && error.contains("C2"),
        "文案含期望 ID 集: {error}"
    );
    assert!(
        !error.contains("缺项"),
        "清单外文案不得含缺项分类词（互斥）: {error}"
    );
    assert!(error.contains("清单外"), "文案标明清单外分类: {error}");
}

#[test]
fn 未知id形态err_四形态文案含字段路径与期望形态() {
    let prompt = prompt_c1_c2();
    for (index, shape) in [
        serde_json::json!({ "pass": true, "evidence": "甲" }),
        serde_json::json!({ "id": 123, "pass": true, "evidence": "甲" }),
        serde_json::json!({ "id": "", "pass": true, "evidence": "甲" }),
        serde_json::json!({ "id": null, "pass": true, "evidence": "甲" }),
    ]
    .into_iter()
    .enumerate()
    {
        let args = serde_json::json!({ "report": "报告", "items": [shape] });
        let error = validate_checklist(&args, &prompt).expect_err("未知 id 形态应 Err");
        assert!(
            error.contains("items[0].id"),
            "形态 {index} 文案定位到字段 items[0].id: {error}"
        );
        assert!(
            error.contains("未知"),
            "形态 {index} 文案标明未知 ID 分类: {error}"
        );
        assert!(
            error.contains("C1") && error.contains("C2"),
            "形态 {index} 文案含期望 ID 集: {error}"
        );
        assert!(
            !error.contains("缺项") && !error.contains("清单外"),
            "形态 {index} 与另两类互斥: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：模型 verdict 被忽略
// ---------------------------------------------------------------------------

#[test]
fn verdict恒由推导_模型传值不读不校验() {
    let prompt = prompt_c1_c2();

    // 带 verdict: pass 但含 fail 项 → 推导 Fail
    let mut args = full_args(true, false);
    args["verdict"] = serde_json::json!("pass");
    assert_eq!(
        validate_checklist(&args, &prompt)
            .expect("verdict 字段被忽略，不应 Err")
            .verdict,
        Verdict::Fail,
        "模型 verdict 被忽略，按 checklist 推导 Fail"
    );

    // 带 verdict: fail 但全 pass → 推导 Pass
    let mut args = full_args(true, true);
    args["verdict"] = serde_json::json!("fail");
    assert_eq!(
        validate_checklist(&args, &prompt)
            .expect("verdict 字段被忽略，不应 Err")
            .verdict,
        Verdict::Pass,
        "模型 verdict 被忽略，按 checklist 推导 Pass"
    );

    // 带非法 verdict 值 → 仍 Ok（既不读也不校验）
    let mut args = full_args(true, true);
    args["verdict"] = serde_json::json!("bogus");
    assert_eq!(
        validate_checklist(&args, &prompt)
            .expect("非法 verdict 值也不读不校验")
            .verdict,
        Verdict::Pass
    );
}

// ---------------------------------------------------------------------------
// 异常：report 上限
// ---------------------------------------------------------------------------

#[test]
fn report上限_恰上限过_超一字符err携三要素() {
    let prompt = prompt_c1_c2();

    let mut args = full_args(true, true);
    args["report"] = serde_json::json!("报".repeat(MAX_REPORT_CHARS));
    validate_checklist(&args, &prompt).expect("恰上限应通过");

    args["report"] = serde_json::json!("报".repeat(MAX_REPORT_CHARS + 1));
    let error = validate_checklist(&args, &prompt).expect_err("超一字符应 Err");
    assert!(
        error.contains(&MAX_REPORT_CHARS.to_string()),
        "文案含上限 2000: {error}"
    );
    assert!(error.contains("2001"), "文案含当前值 2001: {error}");
    assert!(error.contains("超出 1"), "文案含超出量 1: {error}");

    // 5000 字符 → 报 5000 / 超出 3000
    args["report"] = serde_json::json!("报".repeat(5000));
    let error = validate_checklist(&args, &prompt).expect_err("超长应 Err");
    assert!(error.contains("5000"), "文案含当前值 5000: {error}");
    assert!(error.contains("3000"), "文案含超出量 3000: {error}");

    // 空串 → Ok（无下限约束）
    args["report"] = serde_json::json!("");
    assert!(
        validate_checklist(&args, &prompt).is_ok(),
        "report 空串无下限约束"
    );
}

// ---------------------------------------------------------------------------
// 异常：形状（dict 边界）
// ---------------------------------------------------------------------------

#[test]
fn 形状异常九形态_逐形态err且零panic() {
    let prompt = prompt_c1_c2();
    let items = full_args(true, true)["items"].clone();
    let cases: [(&str, serde_json::Value, &str); 9] = [
        ("空对象", serde_json::json!({}), "report"),
        ("缺 report", serde_json::json!({ "items": items }), "report"),
        ("缺 items", serde_json::json!({ "report": "报告" }), "items"),
        (
            "items 空数组",
            serde_json::json!({ "report": "报告", "items": [] }),
            "items",
        ),
        (
            "items null",
            serde_json::json!({ "report": "报告", "items": null }),
            "items",
        ),
        (
            "items 为对象",
            serde_json::json!({ "report": "报告", "items": {} }),
            "items",
        ),
        (
            "report 为浮点",
            serde_json::json!({ "report": 1.5, "items": items.clone() }),
            "report",
        ),
        (
            "report null",
            serde_json::json!({ "report": null, "items": items }),
            "report",
        ),
        ("顶层 null", serde_json::Value::Null, "对象"),
    ];
    for (label, args, field) in cases {
        let error = validate_checklist(&args, &prompt).expect_err(&format!("形态 {label} 应 Err"));
        assert!(
            error.contains(field),
            "形态 {label} 文案含字段名 / 形态词 {field}: {error}"
        );
    }
}

#[test]
fn 元素级形状异常五形态_定位到元素索引() {
    let prompt = prompt_c1_c2();
    let cases: [(&str, serde_json::Value, &str); 5] = [
        (
            "缺 pass",
            serde_json::json!({ "id": "C1", "evidence": "甲" }),
            "items[0].pass",
        ),
        (
            "pass 为字符串",
            serde_json::json!({ "id": "C1", "pass": "true", "evidence": "甲" }),
            "items[0].pass",
        ),
        (
            "pass null",
            serde_json::json!({ "id": "C1", "pass": null, "evidence": "甲" }),
            "items[0].pass",
        ),
        (
            "缺 evidence",
            serde_json::json!({ "id": "C1", "pass": true }),
            "items[0].evidence",
        ),
        (
            "evidence 为数字",
            serde_json::json!({ "id": "C1", "pass": true, "evidence": 42 }),
            "items[0].evidence",
        ),
    ];
    for (label, shape, field) in cases {
        let args = serde_json::json!({ "report": "报告", "items": [shape] });
        let error = validate_checklist(&args, &prompt).expect_err(&format!("形态 {label} 应 Err"));
        assert!(
            error.contains(field),
            "形态 {label} 文案定位到 {field}: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：多余字段 / 重复 ID / 规模 / 特殊字符
// ---------------------------------------------------------------------------

#[test]
fn 多余字段静默忽略_label面逐键ok() {
    let prompt = prompt_c1_c2();
    let mut args = full_args(true, true);
    for key in ["phase", "attempt", "skipped"] {
        args[key] = serde_json::json!("任意值");
    }
    let checklist = validate_checklist(&args, &prompt).expect("多余键静默忽略");
    assert_eq!(checklist.checklist.len(), 2, "多余键零影响");
}

#[test]
fn 重复id集合相等即ok_不静默去重() {
    let prompt = prompt_c1_c2();
    let args = args_with_items(vec![
        item_args("C1", true, "甲"),
        item_args("C1", false, "甲二"),
        item_args("C2", true, "乙"),
    ]);
    let checklist = validate_checklist(&args, &prompt).expect("集合相等即通过（重复项是模型噪声）");
    assert_eq!(checklist.checklist.len(), 3, "重复项不静默去重");
    assert_eq!(
        checklist.verdict,
        Verdict::Fail,
        "verdict 按全部项推导（含重复项）"
    );
}

#[test]
fn 规模边界_百项ok千项不panic() {
    // 100 项清单 + 100 项 args（全覆盖）
    let mut prompt = String::from("## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n");
    let mut items = Vec::with_capacity(100);
    for index in 0..100 {
        prompt.push_str(&format!("| C{index} | 检查项 {index} | 依据 |\n"));
        items.push(item_args(&format!("C{index}"), true, "据"));
    }
    let checklist = validate_checklist(&args_with_items(items), &prompt).expect("百项应通过");
    assert_eq!(checklist.checklist.len(), 100);
    assert_eq!(checklist.verdict, Verdict::Pass);

    // 1000 项：不 panic、不超时
    let mut prompt = String::from("## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n");
    let mut items = Vec::with_capacity(1000);
    for index in 0..1000 {
        prompt.push_str(&format!("| C{index} | 检查项 {index} | 依据 |\n"));
        items.push(item_args(&format!("C{index}"), index % 2 == 0, "据"));
    }
    let checklist = validate_checklist(&args_with_items(items), &prompt).expect("千项应通过");
    assert_eq!(checklist.checklist.len(), 1000);
    assert_eq!(checklist.verdict, Verdict::Fail, "含 fail 项 → Fail");
}

#[test]
fn 文本特殊字符_逐字保真且不污染id解析() {
    let prompt = prompt_c1_c2();

    let mut args = full_args(true, true);
    args["report"] = serde_json::json!("含\n换行\0空字节 🚀 emoji");
    args["items"][0]["evidence"] = serde_json::json!("证据\n含换行\0与🚀");
    let checklist = validate_checklist(&args, &prompt).expect("特殊字符应通过");
    assert_eq!(
        checklist.report, "含\n换行\0空字节 🚀 emoji",
        "report 特殊字符逐字保真"
    );
    assert_eq!(
        checklist.checklist[0].evidence, "证据\n含换行\0与🚀",
        "evidence 逐字保真（无长度上限）"
    );

    // 5000 字符 evidence → Ok（无长度上限）
    let mut args = full_args(true, true);
    args["items"][0]["evidence"] = serde_json::json!("据".repeat(5000));
    let checklist = validate_checklist(&args, &prompt).expect("超长 evidence 应通过");
    assert_eq!(checklist.checklist[0].evidence.chars().count(), 5000);

    // prompt 含 CRLF / emoji → ID 解析与映射不受污染
    let noisy = format!("🚀 前置说明\r\n{}", prompt_c1_c2().replace('\n', "\r\n"));
    let checklist = validate_checklist(&full_args(true, true), &noisy).expect("含噪 prompt 应通过");
    assert_eq!(
        checklist.checklist[0].item, "变更代码中无安全漏洞",
        "CRLF / emoji 噪声不污染的映射"
    );
}

// ---------------------------------------------------------------------------
// 异常：退役面零残留（源码自证）
// ---------------------------------------------------------------------------

#[test]
fn 退役面零残留且上限常量仍场() {
    assert_eq!(MAX_REPORT_CHARS, 2000, "report 上限常量仍在场且为 2000");

    let own = source_of("src/verdict.rs");
    for retired in [
        "parse_verdict",
        "extract_json",
        "fenced_block",
        "brace_slice",
    ] {
        assert!(
            !own.contains(retired),
            "verdict.rs 内 {retired} 零命中（文本解析面已退役）"
        );
    }
}
