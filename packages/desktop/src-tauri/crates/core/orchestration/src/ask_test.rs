//! `ask.rs`（`AskUserQuestion` 工具协议单点）与 crate 根出口面的单元测试：
//! 工具名单点 / description 语义与体量 / parameters 静态形状 / `validate` 六态
//! 与边界 / 解耦守卫（源码自证）+ `lib.rs` 的出口面锚定。
//!
//! 零 mock：被测面是纯函数 + `serde_json` 字面量（无 IO、无时钟、无注入依赖）。
//! 源码自证装置用 `std::fs::read_to_string` + `env!("CARGO_MANIFEST_DIR")`
//!（沿 `finish_test.rs` 的 `source_of` / `prompt_test.rs` 先例）。

use serde_json::{json, Value};

use crate::ask::{self, AskQuestion, TOOL_NAME};

// ---------------------------------------------------------------------------
// 装置：源码自证读盘
// ---------------------------------------------------------------------------

/// 源码自证读产品源文件（`CARGO_MANIFEST_DIR` 相对）。
fn source_of(relative: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("读取 {relative} 失败: {error}"))
}

/// 剔除注释行（文档 / 行注释不承载字面量作者面，自证只看代码行）。
fn code_lines(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("///") || trimmed.starts_with("//"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 合法入参 fixture。
fn valid_args() -> Value {
    json!({
        "question": "选哪个重试策略？",
        "options": ["指数退避", "固定间隔"],
    })
}

// ---------------------------------------------------------------------------
// 正向：工具名单点
// ---------------------------------------------------------------------------

#[test]
fn 工具名单点_线值逐字且源码字面量单处() {
    assert_eq!(TOOL_NAME, "AskUserQuestion", "工具名单点线值");

    let own = code_lines(&source_of("src/ask.rs"));
    assert_eq!(
        own.matches("\"AskUserQuestion\"").count(),
        1,
        "ask.rs 代码行内字面量恰一处（单点判据）"
    );
    assert!(
        own.contains("pub const TOOL_NAME"),
        "字面量作者处为 TOOL_NAME 常量"
    );
}

// ---------------------------------------------------------------------------
// 正向：parameters 的 schema 形状
// ---------------------------------------------------------------------------

#[test]
fn parameters的schema形状逐项锚定且连调两次相等() {
    let schema = ask::parameters();

    assert_eq!(schema["type"], json!("object"), "顶层为对象");

    let properties = schema["properties"].as_object().expect("properties 对象");
    let mut keys: Vec<&str> = properties.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["options", "question"], "属性键集恰两键");

    assert_eq!(
        schema["required"],
        json!(["question", "options"]),
        "required 逐字序（question 在前）"
    );

    assert_eq!(
        schema["properties"]["question"]["type"],
        json!("string"),
        "question 为字符串"
    );

    let options = &schema["properties"]["options"];
    assert_eq!(options["type"], json!("array"), "options 为数组");
    assert_eq!(options["minItems"], json!(1), "options 下界 1（非空选项）");
    assert_eq!(
        options["items"]["type"],
        json!("string"),
        "options 元素为字符串"
    );

    assert_eq!(ask::parameters(), schema, "连调两次结果相等（零动态段）");
}

// ---------------------------------------------------------------------------
// 正向：description 语义覆盖与体量
// ---------------------------------------------------------------------------

#[test]
fn description含单问与选项非空与自由文本应答语义且体量受控() {
    let text = ask::description();

    assert!(text.contains("一次只问一个问题"), "含单问语义: {text}");
    assert!(text.contains("非空"), "含选项非空语义: {text}");
    assert!(
        text.contains("自由作答"),
        "含自由文本应答语义（应答不限于选项）: {text}"
    );
    assert!(
        text.chars().count() < 300,
        "描述 <300 字符（无 prompt 膨胀），实际 {}",
        text.chars().count()
    );
}

// ---------------------------------------------------------------------------
// 正向：合法入参
// ---------------------------------------------------------------------------

#[test]
fn validate合法入参逐字段保真且顺序不重排() {
    let question = ask::validate(&valid_args()).expect("合法入参应 Ok");
    assert_eq!(
        question,
        AskQuestion {
            question: "选哪个重试策略？".to_owned(),
            options: vec!["指数退避".to_owned(), "固定间隔".to_owned()],
        },
        "题干与选项逐字相等（顺序保留）"
    );
}

#[test]
fn validate多余字段宽容且选项顺序与入参一致() {
    let args = json!({
        "question": "选哪个？",
        "options": ["乙", "甲", "丙"],
        "extra": 1,
        "另一多余键": { "nested": true },
    });
    let question = ask::validate(&args).expect("多余字段不进校验面");
    assert_eq!(
        question.options,
        vec!["乙".to_owned(), "甲".to_owned(), "丙".to_owned()],
        "不排序、不去重"
    );
}

// ---------------------------------------------------------------------------
// 异常：六态 Err（文案自足：字段名 + 期望 + 当前值）
// ---------------------------------------------------------------------------

#[test]
fn validate六态err文案自足() {
    let cases: [(&str, Value, &[&str]); 6] = [
        (
            "question 缺失",
            json!({ "options": ["a"] }),
            &["question", "非空字符串"],
        ),
        (
            "question 空白",
            json!({ "question": "   ", "options": ["a"] }),
            &["question", "非空字符串"],
        ),
        (
            "options 缺失",
            json!({ "question": "q" }),
            &["options", "非空字符串数组"],
        ),
        (
            "options 非数组",
            json!({ "question": "q", "options": "x" }),
            &["options", "非空字符串数组"],
        ),
        (
            "options 元素非字符串",
            json!({ "question": "q", "options": ["a", null] }),
            &["options", "字符串数组"],
        ),
        (
            "options 空数组",
            json!({ "question": "q", "options": [] }),
            &["options", "至少一个选项"],
        ),
    ];
    for (label, args, required_terms) in cases {
        let err = ask::validate(&args).expect_err(&format!("{label} 应 Err"));
        for term in required_terms {
            assert!(
                err.contains(term),
                "{label}：Err 文案须含「{term}」，实际: {err}"
            );
        }
    }
}

#[test]
fn validate入参非对象四形态err且零panic() {
    for (label, args) in [
        ("数组", json!([])),
        ("字符串", json!("x")),
        ("null", Value::Null),
        ("数值", json!(42)),
    ] {
        let err = ask::validate(&args).expect_err(&format!("{label} 应 Err"));
        assert!(
            err.contains("期望对象"),
            "{label}：文案含期望形状，实际: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：question 五形态
// ---------------------------------------------------------------------------

#[test]
fn question边界五形态() {
    for (label, question, expect_ok) in [
        ("空串", "", false),
        ("纯空白", "   ", false),
        ("含换行", "a\nb", true),
        ("NUL 与 emoji", "a\0b🚀", true),
        ("超长", &"字".repeat(1001), true),
    ] {
        let args = json!({ "question": question, "options": ["a"] });
        let result = ask::validate(&args);
        assert_eq!(
            result.is_ok(),
            expect_ok,
            "question {label}：期望 ok={expect_ok}，实际: {result:?}"
        );
        if expect_ok {
            assert_eq!(
                result.expect("已判 ok").question,
                question,
                "question {label} 逐字保真（本层零长度门、零截断）"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 边界：options 四形态
// ---------------------------------------------------------------------------

#[test]
fn options边界四形态() {
    // 单元素：最小合法
    let single =
        ask::validate(&json!({ "question": "q", "options": ["唯一"] })).expect("单元素为最小合法");
    assert_eq!(single.options, vec!["唯一".to_owned()]);

    // 空数组：下界 Err
    assert!(
        ask::validate(&json!({ "question": "q", "options": [] })).is_err(),
        "空数组 Err（minItems: 1 的校验半边）"
    );

    // 1000 元素：超大且逐项保真
    let many: Vec<String> = (0..1000).map(|index| format!("选项{index}")).collect();
    let validated = ask::validate(&json!({ "question": "q", "options": many.clone() }))
        .expect("超大列表不截断");
    assert_eq!(validated.options.len(), 1000, "逐项保真不截断");
    assert_eq!(validated.options, many, "元素逐字与顺序保真");

    // 单元素为空串：本层不裁量内容
    let blank =
        ask::validate(&json!({ "question": "q", "options": [""] })).expect("空串选项本层不裁量");
    assert_eq!(blank.options, vec![String::new()]);
}

// ---------------------------------------------------------------------------
// 边界：AskQuestion 构造面与相等性
// ---------------------------------------------------------------------------

#[test]
fn ask_question构造面与相等性可辨() {
    let empty = AskQuestion {
        question: String::new(),
        options: Vec::new(),
    };
    assert_eq!(empty.question, "", "校验产物之外的结构不设约束");

    let base = AskQuestion {
        question: "q".to_owned(),
        options: vec!["a".to_owned(), "b".to_owned()],
    };
    let other_question = AskQuestion {
        question: "q2".to_owned(),
        ..base.clone()
    };
    let other_options = AskQuestion {
        question: "q".to_owned(),
        options: vec!["a".to_owned(), "c".to_owned()],
    };
    assert_ne!(base, other_question, "题干不同即不等");
    assert_ne!(base, other_options, "任一选项不同即不等");
    assert_eq!(base.clone(), base, "Clone 后逐字段相等");
    assert!(!format!("{base:?}").is_empty(), "Debug 非空");
}

// ---------------------------------------------------------------------------
// 边界：词汇隔离（schema 不复用收口工具的动作枚举）
// ---------------------------------------------------------------------------

#[test]
fn parameters序列化全文零收口动作词汇() {
    let text = ask::parameters().to_string();
    for forbidden in ["action", "backtrack", "stop"] {
        assert!(
            !text.contains(forbidden),
            "ask schema 全文不含收口动作词汇 {forbidden}: {text}"
        );
    }
}

// ---------------------------------------------------------------------------
// 异常：解耦守卫（源码自证，全文判）
// ---------------------------------------------------------------------------

#[test]
fn ask_rs全文零finish字样() {
    // 与 finish_test 的「只判代码行」相反：本处**全文**判（含 doc / 注释 / 字符串）
    //——`use crate::finish` 与共享类型因此不可能存在
    let text = source_of("src/ask.rs");
    for forbidden in ["finish", "Finish"] {
        assert!(
            !text.contains(forbidden),
            "ask.rs 全文零 {forbidden} 字样（与收口模块零交叉）"
        );
    }
}

// ---------------------------------------------------------------------------
// 正向：crate 根出口面（`lib.rs`：`pub mod ask;` + `pub use ask::{AskQuestion, TOOL_NAME}`）
// ---------------------------------------------------------------------------

#[test]
fn crate根出口面可达且与ask模块路径同源() {
    // `lib.rs` 的 `pub use ask::{AskQuestion, TOOL_NAME}` 两名经 crate 根可达；
    // `description` / `parameters` / `validate` 三面与并行收口模块同名，库注释
    // 明示「不进根再导出」，消费方经 `ask::` 模块路径引用（下方同源断言覆盖
    // 该设计：模块路径调用是真值面）。
    assert_eq!(crate::TOOL_NAME, "AskUserQuestion", "根路径工具名可达");
    assert_eq!(
        crate::TOOL_NAME,
        ask::TOOL_NAME,
        "根路径与模块路径同一常量（再导出不是第二份定义）"
    );

    // 模块路径三函数同源可调用：连调两次结果相等（纯静态 / 纯函数，零动态段）
    assert_eq!(
        ask::parameters(),
        ask::parameters(),
        "模块路径 parameters 稳定"
    );
    assert_eq!(
        ask::description(),
        ask::description(),
        "模块路径 description 稳定（&'static str）"
    );
    assert_eq!(
        ask::validate(&valid_args()),
        ask::validate(&valid_args()),
        "模块路径 validate 确定"
    );

    // 类型可达且可构造：根路径与模块路径为同一类型（赋值即证）
    let root_shape: crate::AskQuestion = AskQuestion {
        question: "q".to_owned(),
        options: vec!["a".to_owned()],
    };
    let module_shape: ask::AskQuestion = root_shape.clone();
    assert_eq!(root_shape, module_shape, "字段可读且相等");

    // `AskPort` 与既有 port 出口并存（新增一行 `pub use` 不遮蔽既有名）
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<dyn crate::AskPort>();
}

#[test]
fn crate根既有再导出面零改动且ask_question与ask_payload不冲突() {
    use crate::{
        ChangeRunStatus, ChangeStepEdge, ChangeStepKind, ChangeStepState, ChangeStepStatus,
        EdgeKind, NodeKey, RunNotice, RunUpdate,
    };

    fn key(phase: &str, instance_no: u32) -> NodeKey {
        (phase.to_owned(), 0, ChangeStepKind::Executor, instance_no)
    }

    fn step_state() -> ChangeStepState {
        ChangeStepState {
            phase: "p".to_owned(),
            attempt: 0,
            step: ChangeStepKind::Executor,
            instance_no: 0,
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 0,
            ended_at: None,
            token_input: None,
            token_output: None,
        }
    }

    // 六变体经 crate 根名可得，且 kind 逐字与通知投影同值（再导出面扩位不改映射）
    let updates = [
        ("step", RunUpdate::Step { step: step_state() }),
        (
            "edge",
            RunUpdate::Edge {
                edge: ChangeStepEdge {
                    from: key("p", 0),
                    to: key("q", 1),
                    kind: EdgeKind::Forward,
                },
            },
        ),
        (
            "sessionEvent",
            RunUpdate::SessionEvent {
                session_id: "s".to_owned(),
                event: agent::AgentEvent::stamp(
                    None,
                    0,
                    agent::AgentEventKind::Raw {
                        event_type: "x".to_owned(),
                        raw_json: "{}".to_owned(),
                    },
                ),
            },
        ),
        (
            "ask",
            RunUpdate::Ask {
                ask: crate::AskPayload {
                    question: "q".to_owned(),
                    options: vec!["a".to_owned()],
                    tool_call_id: None,
                    session_id: None,
                },
            },
        ),
        (
            "confirmWait",
            RunUpdate::ConfirmWait {
                phase: "archive".to_owned(),
                archive: None,
            },
        ),
        (
            "finished",
            RunUpdate::Finished {
                status: ChangeRunStatus::Completed,
                reason: None,
            },
        ),
    ];
    let kinds: Vec<&str> = updates.iter().map(|(_, update)| update.kind()).collect();
    assert_eq!(
        kinds,
        vec![
            "step",
            "edge",
            "sessionEvent",
            "ask",
            "confirmWait",
            "finished"
        ],
        "既有五臂与 ask 臂 kind 逐字不变（六变体序）"
    );
    let notices: Vec<RunNotice> = updates
        .iter()
        .map(|(_, update)| RunNotice::from(update))
        .collect();
    assert_eq!(
        notices,
        vec![
            RunNotice::Step,
            RunNotice::Edge,
            RunNotice::SessionEvent,
            RunNotice::Ask,
            RunNotice::ConfirmWait,
            RunNotice::Finished,
        ],
        "通知投影六变体一一对应且同序"
    );

    // 既有再导出名仍指向原定义
    let _ = crate::FinishPayload::Decision(crate::DecisionAction::Stop {
        reason: "r".to_owned(),
    });
    let _control =
        crate::ChangeFlowControl::new(std::sync::Arc::new(agent::AskRegistry::default()));
    let _snapshot_word: Option<crate::StoreSnapshot> = None;

    // `AskQuestion`（ask 模块）与 `AskPayload`（state 模块）两名字不冲突
    let question_word: AskQuestion = AskQuestion {
        question: "q".to_owned(),
        options: Vec::new(),
    };
    let payload_word = crate::AskPayload {
        question: question_word.question.clone(),
        options: question_word.options.clone(),
        tool_call_id: None,
        session_id: None,
    };
    assert_eq!(payload_word.question, "q");
}
