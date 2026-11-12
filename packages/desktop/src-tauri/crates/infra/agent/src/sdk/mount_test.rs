//! `mount.rs`（工具挂载单源）的单元测试：十项静态清单 / 名字查表 / 非法名
//! `Err` / `finish` 位置位铸造 / `lookup` 单点 / 夹具形态 / 类型派生面。
//!
//! 不 mock：`registry()` / `definitions` / `lookup` 全为进程内纯函数
//!（`OnceLock` 一次性静态表，无 IO / 无 spawn / 无网络）；`prompt` 用内存合成
//! 静态清单字符串；`orchestration::finish::{description, parameters}` 真实调用
//!（铸造内容即被测面，替身会掩盖 enum 对拍）。

use agent::{AskMode, FinishSpec, SessionInjections};

use crate::sdk::mount::{
    builtin, definitions, injected, injected_finish, lookup, registry, ToolExec, ToolMount,
    ToolTier, ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL, EXPLORE_NOTE_TOOL, TOOL_NAMES,
};

/// 十项名字的期望序（前七恰七内建）。
const EXPECTED_NAMES: [&str; 10] = [
    "read",
    "grep",
    "glob",
    "ls",
    "write",
    "edit",
    "bash",
    "create_explore_note",
    "archi_module",
    "archi_search",
];

/// 名字投影（顺序断言面）。
fn names(mounts: &[ToolMount]) -> Vec<String> {
    mounts
        .iter()
        .map(|mount| mount.definition.name.clone())
        .collect()
}

/// 两行静态清单 prompt（动态 enum 对拍用例的合成语料）。
fn prompt_of(ids: &[&str]) -> String {
    let mut text = String::from("## 静态清单\n\n| ID | 检查项 | 判断依据 |\n|---|---|---|\n");
    for id in ids {
        text.push_str(&format!("| {id} | 检查项 {id} | 依据 |\n"));
    }
    text
}

/// 源码自证读产品源文件（`CARGO_MANIFEST_DIR` 相对）。
fn source_of(relative: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("读取 {relative} 失败: {error}"))
}

// ---------------------------------------------------------------------------
// 正向：registry 十项
// ---------------------------------------------------------------------------

#[test]
fn registry十项名字唯一且有序且前七恰内建() {
    let names = names(registry());
    assert_eq!(names, EXPECTED_NAMES.to_vec(), "十项名字逐值有序");
    assert_eq!(names.len(), 10, "十项");

    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 10, "无重复名: {names:?}");

    assert_eq!(
        registry()[..7]
            .iter()
            .map(|mount| mount.definition.name.as_str())
            .collect::<Vec<_>>(),
        TOOL_NAMES.to_vec(),
        "前七项恰为七内建（`tools: None` 的语义载体）"
    );
    assert_eq!(builtin(), TOOL_NAMES.as_slice(), "内建名字清单同源");
}

#[test]
fn registry的tier归属逐项对应改造前三张常量表() {
    let expected: [(&str, ToolTier); 10] = [
        ("read", ToolTier::Read),
        ("grep", ToolTier::Read),
        ("glob", ToolTier::Read),
        ("ls", ToolTier::Read),
        ("write", ToolTier::Write),
        ("edit", ToolTier::Write),
        ("bash", ToolTier::Execute),
        ("create_explore_note", ToolTier::Write),
        ("archi_module", ToolTier::Read),
        ("archi_search", ToolTier::Read),
    ];
    for (name, tier) in expected {
        let mount = registry()
            .iter()
            .find(|mount| mount.definition.name == name)
            .unwrap_or_else(|| panic!("挂载清单应含 {name}"));
        assert_eq!(mount.tier, tier, "工具 {name} 的 tier 归属");
    }
    // 计数面：Read 六 / Write 三 / Execute 一 / Protocol 零
    let count = |tier: ToolTier| registry().iter().filter(|mount| mount.tier == tier).count();
    assert_eq!(count(ToolTier::Read), 6, "Read 六项");
    assert_eq!(count(ToolTier::Write), 3, "Write 三项");
    assert_eq!(count(ToolTier::Execute), 1, "Execute 一项");
    assert_eq!(
        count(ToolTier::Protocol),
        0,
        "静态表零 Protocol（finish_step 不入表）"
    );
}

#[test]
fn registry的exec归属且finish_step不入静态表() {
    for name in TOOL_NAMES {
        let mount = registry()
            .iter()
            .find(|mount| mount.definition.name == name)
            .expect("内建名在表内");
        assert_eq!(mount.exec, ToolExec::Pure, "内建 {name} 走纯函数执行体");
    }
    assert_eq!(
        registry()
            .iter()
            .find(|mount| mount.definition.name == EXPLORE_NOTE_TOOL)
            .expect("专用工具在表内")
            .exec,
        ToolExec::ExploreNote
    );
    for name in [ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL] {
        assert_eq!(
            registry()
                .iter()
                .find(|mount| mount.definition.name == name)
                .expect("archi 工具在表内")
                .exec,
            ToolExec::Archi
        );
    }
    assert!(
        !registry()
            .iter()
            .any(|mount| matches!(mount.exec, ToolExec::FinishStep(_))),
        "静态表内零 FinishStep（收口工具按注入面动态铸造）"
    );
}

#[test]
fn registry定义面完整十项定义形状与非空() {
    for mount in registry() {
        let definition = &mount.definition;
        assert!(
            !definition.description.trim().is_empty(),
            "工具 {} 定义描述非空",
            definition.name
        );
        assert_eq!(
            definition.parameters["type"], "object",
            "工具 {} 参数 schema 为对象",
            definition.name
        );
    }
}

#[test]
fn registry惰性单点_两次调用同一指针且内容相等() {
    let first = registry();
    let second = registry();
    assert!(
        std::ptr::eq(first, second),
        "`OnceLock` 单点语义：两次调用返回同一指针"
    );
    assert_eq!(
        names(first).len(),
        names(second).len(),
        "同一次 get_or_init 不重建"
    );
}

// ---------------------------------------------------------------------------
// 正向：definitions 名字查表
// ---------------------------------------------------------------------------

#[test]
fn definitions内建七件_tools为none即七内建且零finish_step() {
    let mounts = definitions(&SessionInjections::default(), "").expect("缺省注入面合法");
    assert_eq!(mounts.len(), 7, "`tools: None` = 内建七件");
    assert_eq!(names(&mounts), TOOL_NAMES.to_vec(), "名字序 == TOOL_NAMES");
    assert!(
        !names(&mounts).contains(&"finish_step".to_owned()),
        "finish 位缺席时零 finish_step"
    );
}

#[test]
fn definitions名字清单收窄且按registry序() {
    // 入参序与 registry 序相反 → 输出按 registry 序
    let mounts = definitions(&injected(&["archi_module", "read"]), "").expect("合法名");
    assert_eq!(
        names(&mounts),
        vec!["read".to_owned(), "archi_module".to_owned()],
        "按 registry 序过滤（不按入参序）"
    );
    let mounts = definitions(&injected(&["bash", "read"]), "").expect("合法名");
    assert_eq!(
        names(&mounts),
        vec!["read".to_owned(), "bash".to_owned()],
        "read 先于 bash（registry 序）"
    );

    // 三个专用工具以名字进清单后定义由 registry 提供
    for special in [EXPLORE_NOTE_TOOL, ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL] {
        let mounts = definitions(&injected(&[special]), "").expect("专用名合法");
        assert_eq!(
            names(&mounts),
            vec![special.to_owned()],
            "专用名 {special} 可下发"
        );
    }
}

#[test]
fn definitions重复名与空清单与空串元素三形态() {
    // 重复名 → 一份（不重复下发）
    let mounts = definitions(&injected(&["read", "read"]), "").expect("重复名合法");
    assert_eq!(names(&mounts), vec!["read".to_owned()], "重复名去重下发");

    // 空清单 → 合法空面（与 None = 七件语义可辨）
    let mounts = definitions(&injected(&[]), "").expect("空清单合法");
    assert!(mounts.is_empty(), "`Some([])` = 零下发");

    // 空串元素 → 非法名
    let error = definitions(&injected(&["read", ""]), "").expect_err("空串是非法名");
    assert!(
        error.contains("注入面非法工具名"),
        "空串 Err 文案携前缀: {error}"
    );
}

#[test]
fn definitions非法名err且不返回部分清单() {
    let error =
        definitions(&injected(&["read", "no_such_tool"]), "").expect_err("清单外名字应 Err");
    assert!(
        error.contains("no_such_tool"),
        "文案携非法名（可归因）: {error}"
    );

    let error = definitions(&injected(&["README"]), "").expect_err("大小写漂移应 Err");
    assert!(error.contains("README"), "大小写漂移同样 Err: {error}");

    // 对照臂：合法单名 Ok（Err 不是「滤过后的子集」，与改造前静默滤过显式不同）
    assert!(
        definitions(&injected(&["read"]), "").is_ok(),
        "对照：合法单名 Ok"
    );
}

// ---------------------------------------------------------------------------
// 正向：finish 位铸造
// ---------------------------------------------------------------------------

#[test]
fn definitions的finish位缺席时零追加() {
    for tools in [None, Some(vec!["read".to_owned()]), Some(vec![])] {
        let injections = SessionInjections {
            preamble: None,
            tools,
            finish: None,
            ask: None,
        };
        let mounts = definitions(&injections, "## 静态清单\n| C1 | 项 | 据 |\n").expect("合法");
        assert!(
            !names(&mounts).contains(&"finish_step".to_owned()),
            "finish 位缺席零追加，实际: {:?}",
            names(&mounts)
        );
    }
}

#[test]
fn definitions的finish位置位时末尾追加finish_step() {
    let prompt = prompt_of(&["C1", "C2"]);
    let mounts = definitions(
        &injected_finish(&[], Some(FinishSpec::EvaluatorChecklist)),
        &prompt,
    )
    .expect("finish 位合法");

    assert_eq!(mounts.len(), 1, "空名字清单 + finish 位 = 恰一项");
    let finish = mounts.last().expect("末项为收口工具");
    assert_eq!(finish.definition.name, "finish_step", "工具名");
    assert_eq!(finish.tier, ToolTier::Protocol, "收口档位为 Protocol");
    assert_eq!(
        finish.exec,
        ToolExec::FinishStep(FinishSpec::EvaluatorChecklist),
        "执行体携带收口档位"
    );
    assert_eq!(
        finish.definition.description,
        orchestration::finish::description(FinishSpec::EvaluatorChecklist),
        "描述取 description 单点"
    );

    // 追加在末尾：前七项逐名不变（`tools: None` 的正向面）
    let mounts = definitions(
        &injected_finish(&[], Some(FinishSpec::EvaluatorChecklist)),
        &prompt,
    )
    .expect("finish 位合法");
    let _ = mounts;
    let injections = SessionInjections {
        preamble: None,
        tools: None,
        finish: Some(FinishSpec::EvaluatorChecklist),
        ask: None,
    };
    let mounts = definitions(&injections, &prompt).expect("finish 位置位合法");
    assert_eq!(mounts.len(), 8, "七内建 + finish_step");
    assert_eq!(
        mounts[..7]
            .iter()
            .map(|mount| mount.definition.name.as_str())
            .collect::<Vec<_>>(),
        TOOL_NAMES.to_vec(),
        "前七项逐名不变（RunStarted.tools 其余名单不变）"
    );
    assert_eq!(mounts[7].definition.name, "finish_step", "末项为收口工具");
}

#[test]
fn definitions动态enum逐值等于prompt清单id序() {
    let cases: [(&str, Vec<&str>); 3] = [
        ("C1", vec!["C1", "C2", "C3"]),
        ("A1", vec!["A1"]),
        ("R1", vec!["R1", "R2", "R3", "R4"]),
    ];
    for (_, expected_ids) in cases {
        let prompt = prompt_of(&expected_ids);
        let expected: Vec<String> = expected_ids.into_iter().map(str::to_owned).collect();
        let mounts = definitions(
            &injected_finish(&[], Some(FinishSpec::EvaluatorChecklist)),
            &prompt,
        )
        .expect("合法");
        let enum_values = mounts.last().expect("末项").definition.parameters["properties"]["items"]
            ["items"]["properties"]["id"]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("enum 数组在场，实际 prompt: {prompt}"))
            .iter()
            .map(|value| value.as_str().expect("枚举元素为字符串").to_owned())
            .collect::<Vec<_>>();
        assert_eq!(enum_values, expected, "enum 逐值有序等于 prompt ID 序");
    }

    // 退化面：prompt 为空 → enum 键缺失且其余结构在
    let mounts = definitions(
        &injected_finish(&[], Some(FinishSpec::EvaluatorChecklist)),
        "",
    )
    .expect("合法");
    let id_schema = &mounts.last().expect("末项").definition.parameters["properties"]["items"]
        ["items"]["properties"]["id"];
    assert!(id_schema.get("enum").is_none(), "空 ID 集省略 enum");
    assert_eq!(id_schema["type"], "string", "其余结构在（退化面不 panic）");
}

#[test]
fn definitions的decision形态与prompt无关() {
    let decision = |prompt: &str| {
        definitions(&injected_finish(&[], Some(FinishSpec::Decision)), prompt)
            .expect("合法")
            .last()
            .expect("末项")
            .clone()
    };
    let first = decision("");
    let second = decision(&prompt_of(&["C1"]));

    assert_eq!(first.definition.name, "finish_step");
    assert_eq!(
        first.definition.description,
        orchestration::finish::description(FinishSpec::Decision),
        "描述取 decision 形态单点"
    );
    assert_ne!(
        first.definition.description,
        orchestration::finish::description(FinishSpec::EvaluatorChecklist),
        "两形态描述不相等"
    );
    let actions: Vec<&str> = first.definition.parameters["properties"]["action"]["enum"]
        .as_array()
        .expect("action 枚举")
        .iter()
        .map(|value| value.as_str().expect("字符串"))
        .collect();
    assert_eq!(
        actions,
        vec!["backtrack", "stop"],
        "decision 形态两动作（ask 已迁出为独立阻塞工具）"
    );
    assert_eq!(
        first.definition.parameters, second.definition.parameters,
        "decision 形态参数与 prompt 无关"
    );
}

#[test]
fn definitions非法名与finish并存时err优先() {
    let error = definitions(
        &injected_finish(&["no_such_tool"], Some(FinishSpec::Decision)),
        "",
    )
    .expect_err("非法名先于铸造 Err");
    assert!(
        error.contains("no_such_tool"),
        "不静默降级为「只下发 finish_step」: {error}"
    );
}

// ---------------------------------------------------------------------------
// 边界：lookup
// ---------------------------------------------------------------------------

#[test]
fn lookup十项命中四类未命中() {
    for name in EXPECTED_NAMES {
        let definition = lookup(name).unwrap_or_else(|| panic!("{name} 应命中"));
        assert_eq!(definition.name, name, "命中项名与入参相等");
    }
    for miss in ["", "Read", "read ", "no_such_tool"] {
        assert!(
            lookup(miss).is_none(),
            "未命中形态 {miss:?} 应 None（精确匹配、零归一化）"
        );
    }
}

#[test]
fn lookup与registry同源_定义逐字段相等() {
    for mount in registry() {
        let looked = lookup(&mount.definition.name).expect("挂载清单内名可查");
        assert_eq!(
            looked.name, mount.definition.name,
            "lookup 不改名（克隆出口）"
        );
        assert_eq!(
            looked.description, mount.definition.description,
            "lookup 不产第二份定义（描述同源）"
        );
        assert_eq!(
            looked.parameters, mount.definition.parameters,
            "lookup 不产第二份定义（参数同源）"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：测试夹具形态
// ---------------------------------------------------------------------------

#[test]
fn 夹具形态_injected与injected_finish四断言() {
    let injections = injected(&["read", "archi_module"]);
    assert_eq!(
        injections.tools,
        Some(vec!["read".to_owned(), "archi_module".to_owned()]),
        "名字清单透传"
    );
    assert_eq!(injections.preamble, None, "preamble 缺省 None");
    assert_eq!(injections.finish, None, "finish 缺省 None");

    assert_eq!(
        injected(&[]).tools,
        Some(Vec::new()),
        "空清单表达「显式空」而非 None"
    );
    assert_eq!(
        injected_finish(&["read"], Some(FinishSpec::Decision)).finish,
        Some(FinishSpec::Decision),
        "finish 位透传"
    );
    assert_eq!(
        injected(&["read"]),
        injected_finish(&["read"], None),
        "两夹具产出可等值比较（Eq 链）"
    );
}

// ---------------------------------------------------------------------------
// 边界：类型派生面
// ---------------------------------------------------------------------------

#[test]
fn 类型派生面_tier与exec可复制且mount可克隆() {
    // Copy：赋值后原值仍可用
    let tier = ToolTier::Read;
    let copied = tier;
    assert_eq!(tier, copied, "ToolTier Copy + Eq");

    let exec = ToolExec::FinishStep(FinishSpec::Decision);
    let copied_exec = exec;
    assert_eq!(exec, copied_exec, "ToolExec Copy + Eq");

    for tier in [
        ToolTier::Read,
        ToolTier::Write,
        ToolTier::Execute,
        ToolTier::Protocol,
    ] {
        assert_eq!(tier, tier, "四 tier 全值域可判等");
    }

    // ToolMount：Debug + Clone
    let mount = registry().first().expect("非空").clone();
    assert!(!format!("{mount:?}").is_empty(), "ToolMount Debug 非空");
    assert_eq!(
        mount.clone().definition.name,
        mount.definition.name,
        "clone 后定义名相等"
    );

    // 源码自证（D1 显式留白）：`ToolMount` 的 derive 行不含 `PartialEq` / `Eq`
    // ——内含 `serde_json::Value` 无 `Eq`，本类型驻 infra 不承担 `Eq` 链
    let own = source_of("src/sdk/mount.rs");
    let struct_at = own
        .find("pub(crate) struct ToolMount")
        .expect("ToolMount 定义在场");
    let head = &own[..struct_at];
    let derive_at = head.rfind("#[derive(").expect("derive 行在场");
    let derive_line = head[derive_at..]
        .split(')')
        .next()
        .expect("derive 行以 `)` 收尾");
    let derived: Vec<&str> = derive_line
        .trim_start_matches("#[derive(")
        .split(',')
        .map(str::trim)
        .collect();
    assert!(
        derived.contains(&"Debug") && derived.contains(&"Clone"),
        "ToolMount 派生 Debug + Clone，实际: {derived:?}"
    );
    assert!(
        !derived.contains(&"PartialEq") && !derived.contains(&"Eq"),
        "ToolMount derive 行不含 PartialEq / Eq（D1 显式留白），实际: {derived:?}"
    );
}

// ---------------------------------------------------------------------------
// 异常：工具名作者单点（源码自证）
// ---------------------------------------------------------------------------

#[test]
fn 工具名作者单点与字面量归属() {
    let own = source_of("src/sdk/mount.rs");
    for constant in [
        "pub(crate) const TOOL_NAMES",
        "pub(crate) const EXPLORE_NOTE_TOOL",
        "pub(crate) const ARCHI_MODULE_TOOL",
        "pub(crate) const ARCHI_SEARCH_TOOL",
    ] {
        assert!(
            own.contains(constant),
            "常量定义只在 mount.rs 一处: {constant}"
        );
    }
    assert!(
        !own.contains("\"finish_step\""),
        "mount.rs 内 `finish_step` 字面量零命中（经 orchestration::finish::TOOL_NAME 引用）"
    );

    // `TOOL_NAMES` 不参与任何判定：policy / loop 内零命中
    for relative in ["src/sdk/policy.rs", "src/sdk/loop.rs"] {
        let text = source_of(relative);
        assert!(
            !text.contains("TOOL_NAMES"),
            "{relative} 内 TOOL_NAMES 零命中（准入与成员校验全查挂载清单）"
        );
    }
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`ask` 位铸造（挂载项 / 四组合独立 / 追加序 / 定义单源）
// ---------------------------------------------------------------------------

/// ask 位夹具：名字清单 + ask 档位 + 收口档位（三面正交的构造面）。
fn injected_ask(
    names: &[&str],
    ask: Option<AskMode>,
    finish: Option<FinishSpec>,
) -> SessionInjections {
    SessionInjections {
        preamble: None,
        tools: Some(names.iter().map(|name| (*name).to_owned()).collect()),
        finish,
        ask,
    }
}

/// `Some(Generic)` 追加（正向，AC-1 / AC-6）：挂载项数 = 七 + 1；末项名称 /
/// tier / exec；`description` / `parameters` 与 `orchestration::ask::{description,
/// parameters}` **逐字节相等**（同名覆盖的定义单源判据）。
#[test]
fn definitions的ask位generic档追加且定义与ask模块逐字节相等() {
    let mounts = definitions(
        &injected_ask(&EXPECTED_NAMES[..7], Some(AskMode::Generic), None),
        "",
    )
    .expect("ask 位置位合法");

    assert_eq!(mounts.len(), 8, "七内建 + AskUserQuestion");
    let ask = mounts.last().expect("末项为 ask 工具");
    assert_eq!(ask.definition.name, "AskUserQuestion", "工具名取 TOOL_NAME");
    assert_eq!(ask.tier, ToolTier::Protocol, "ask 档位为 Protocol");
    assert_eq!(ask.exec, ToolExec::Ask(AskMode::Generic), "执行体携档位");
    assert_eq!(
        ask.definition.description,
        orchestration::ask::description(),
        "描述取 ask 模块单点（逐字节相等）"
    );
    assert_eq!(
        ask.definition.parameters,
        orchestration::ask::parameters(),
        "参数取 ask 模块单点（逐字节相等）"
    );
}

/// `Some(Flow)` 档位可辨（正向，AC-2）：名称 / 描述 / schema 三项与 `Generic`
/// 形态逐字相同（模型视角零差异），唯一差异是 `exec`。
#[test]
fn definitions的ask位两档位模型视角零差异仅执行体不同() {
    let generic = definitions(&injected_ask(&[], Some(AskMode::Generic), None), "").expect("合法");
    let flow = definitions(&injected_ask(&[], Some(AskMode::Flow), None), "").expect("合法");

    assert_eq!(generic.len(), 1, "空名字清单 + ask 位 = 恰一项");
    assert_eq!(flow.len(), 1);
    assert_eq!(
        generic[0].definition.name, flow[0].definition.name,
        "名称逐字相同"
    );
    assert_eq!(
        generic[0].definition.description, flow[0].definition.description,
        "描述逐字相同"
    );
    assert_eq!(
        generic[0].definition.parameters, flow[0].definition.parameters,
        "schema 逐字相同"
    );
    assert_eq!(generic[0].tier, flow[0].tier, "档位相同");
    assert_eq!(generic[0].exec, ToolExec::Ask(AskMode::Generic));
    assert_eq!(flow[0].exec, ToolExec::Ask(AskMode::Flow));
    assert_ne!(generic[0].exec, flow[0].exec, "唯一差异为执行体");
}

/// 追加序（正向，DD14）：`ask: Some` + `finish: Some(Decision)` → 名单尾两项
/// 依次 `AskUserQuestion`、`finish_step`（ask 先、finish 保持末位）。
#[test]
fn definitions的ask位追加在finish之前() {
    let mounts = definitions(
        &injected_ask(&[], Some(AskMode::Generic), Some(FinishSpec::Decision)),
        "",
    )
    .expect("两位并存合法");

    assert_eq!(mounts.len(), 2, "空名字清单 + 两位 = 两项");
    let tail: Vec<&str> = mounts
        .iter()
        .map(|mount| mount.definition.name.as_str())
        .collect();
    assert_eq!(
        tail,
        vec!["AskUserQuestion", "finish_step"],
        "追加序为 ask 先、finish 后（finish 保持末位）"
    );
}

/// 名字清单不收窄 ask（正向）：`tools: Some([六件名])`（explore 面）+ ask 位
/// → ask 仍追加（按位追加的基线工具，不占名字清单、不被清单过滤碰到）。
#[test]
fn definitions的ask位不被名字清单过滤() {
    let explore_names = [
        "read",
        "grep",
        "glob",
        "ls",
        "write",
        "edit",
        "create_explore_note",
    ];
    let mounts = definitions(
        &injected_ask(&explore_names, Some(AskMode::Generic), None),
        "",
    )
    .expect("explore 面 + ask 合法");

    let projected = names(&mounts);
    assert_eq!(projected.len(), explore_names.len() + 1, "清单 + ask 一项");
    assert_eq!(
        projected.last().map(String::as_str),
        Some("AskUserQuestion")
    );
    assert!(
        !explore_names.contains(&"AskUserQuestion"),
        "ask 不占名字清单（清单本身不含该名）"
    );
}

/// 四组合独立（异常，AC-3 / AC-4，核心判据）：`(ask, finish)` 四组合 → 名单
/// 投影逐一断言，证两分支无合取判据。
#[test]
fn definitions的ask与finish四组合互相独立() {
    let combinations: [(Option<AskMode>, Option<FinishSpec>, [bool; 2]); 4] = [
        (
            Some(AskMode::Generic),
            Some(FinishSpec::Decision),
            [true, true],
        ),
        (Some(AskMode::Generic), None, [true, false]),
        (None, Some(FinishSpec::Decision), [false, true]),
        (None, None, [false, false]),
    ];
    for (ask, finish, [expect_ask, expect_finish]) in combinations {
        let mounts = definitions(&injected_ask(&[], ask, finish), "").expect("四组合均合法");
        let projected = names(&mounts);
        assert_eq!(
            projected.contains(&"AskUserQuestion".to_owned()),
            expect_ask,
            "(ask={ask:?}, finish={finish:?}) 的 ask 项在场性"
        );
        assert_eq!(
            projected.contains(&"finish_step".to_owned()),
            expect_finish,
            "(ask={ask:?}, finish={finish:?}) 的 finish 项在场性"
        );
    }
}

/// `ask: None` 零追加（异常，AC-3）：七件名清单 + `ask: None` → 挂载项数恰 7、
/// 名单内零 `AskUserQuestion`。
#[test]
fn definitions的ask位缺席时零追加() {
    for tools in [
        Some(
            EXPECTED_NAMES[..7]
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>(),
        ),
        None,
    ] {
        let injections = SessionInjections {
            preamble: None,
            tools,
            finish: None,
            ask: None,
        };
        let mounts = definitions(&injections, "").expect("合法");
        assert_eq!(mounts.len(), 7, "AskUserQuestion 不追加到内建七件");
        assert!(
            !names(&mounts).contains(&"AskUserQuestion".to_owned()),
            "ask = None 时名单零该名（RunStarted.tools 随名单派生）"
        );
    }
}

/// `tools` 三形态 × ask 两位（边界，组合）：`None`（内建七件）/ `Some(vec![])`
/// / `Some(含 archi 名)` 配 `ask: None` / `Some(Generic)` → 七件底座形态零改动；
/// ask 追加与 `tools` 形态无关（唯一依赖 = ask 位）。
#[test]
fn definitions的tools三形态与ask两位组合() {
    let archi_names = ["read", "archi_module", "archi_search"];
    let tool_shapes: [Option<Vec<String>>; 3] = [
        None,
        Some(Vec::new()),
        Some(archi_names.iter().map(|name| (*name).to_owned()).collect()),
    ];
    for tools in &tool_shapes {
        for ask in [None, Some(AskMode::Generic)] {
            let injections = SessionInjections {
                preamble: None,
                tools: tools.clone(),
                finish: None,
                ask,
            };
            let mounts = definitions(&injections, "").expect("组合合法");
            let projected = names(&mounts);
            let expect_ask = ask.is_some();
            assert_eq!(
                projected.contains(&"AskUserQuestion".to_owned()),
                expect_ask,
                "tools={tools:?} 与 ask={ask:?} 组合下 ask 追加只依赖 ask 位"
            );
            if tools.is_none() {
                assert_eq!(
                    projected.len(),
                    if expect_ask { 8 } else { 7 },
                    "内建七件底座 + ask 一位"
                );
            }
        }
    }
}

/// `prompt` 无关性（边界，参数化 2 形态）：ask 位置位时 `parameters()` 不依赖
/// prompt 文本（`prompt: ""` 与长清单产出逐字节相等）；对照臂 =
/// `finish: Some(EvaluatorChecklist)` 的 schema 随 prompt 变化（证明本行非空转）。
#[test]
fn definitions的ask位参数与prompt无关而对照片随prompt变化() {
    let long_prompt = prompt_of(&["C1", "C2", "C3", "C4", "C5"]);
    let ask_empty = definitions(&injected_ask(&[], Some(AskMode::Generic), None), "")
        .expect("合法")
        .pop()
        .expect("末项")
        .definition
        .parameters;
    let ask_long = definitions(
        &injected_ask(&[], Some(AskMode::Generic), None),
        &long_prompt,
    )
    .expect("合法")
    .pop()
    .expect("末项")
    .definition
    .parameters;
    assert_eq!(ask_empty, ask_long, "ask schema 与 prompt 无关");

    let finish_empty = definitions(
        &injected_ask(&[], None, Some(FinishSpec::EvaluatorChecklist)),
        "",
    )
    .expect("合法")
    .pop()
    .expect("末项")
    .definition
    .parameters;
    let finish_long = definitions(
        &injected_ask(&[], None, Some(FinishSpec::EvaluatorChecklist)),
        &long_prompt,
    )
    .expect("合法")
    .pop()
    .expect("末项")
    .definition
    .parameters;
    assert_ne!(
        finish_empty, finish_long,
        "对照臂：evaluator schema 随 prompt 变化（本行非空转）"
    );
}

/// 五臂可辨（边界，参数化 5 臂）：`ToolExec` 五值两两 `!=`；
/// `Ask(Generic) != Ask(Flow)`（档位参与 `Eq`）。
#[test]
fn tool_exec五臂两两可辨且ask档位参与相等语义() {
    let arms = [
        ToolExec::Pure,
        ToolExec::ExploreNote,
        ToolExec::Archi,
        ToolExec::FinishStep(FinishSpec::Decision),
        ToolExec::Ask(AskMode::Generic),
    ];
    for outer in 0..arms.len() {
        for inner in (outer + 1)..arms.len() {
            assert_ne!(arms[outer], arms[inner], "五臂 {outer} 与 {inner} 两两可辨");
        }
    }
    assert_ne!(
        ToolExec::Ask(AskMode::Generic),
        ToolExec::Ask(AskMode::Flow),
        "档位参与 Eq（执行体可辨）"
    );
    assert_eq!(
        ToolExec::Ask(AskMode::Generic),
        ToolExec::Ask(AskMode::Generic)
    );
}

/// 无合取自证（异常，源码自证，AC-4）：`mount.rs` 代码行内零合取形态；两处
/// `if` 分别只读 `injections.ask` 与 `injections.finish`。
#[test]
fn mount_rs的ask与finish两分支无合取判据() {
    let own = source_of("src/sdk/mount.rs");
    let code_only = own
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("///") || trimmed.starts_with("//"))
        })
        .collect::<Vec<_>>()
        .join("\n");

    for forbidden in [
        "ask.is_some() &&",
        "&& injections.ask",
        "&& injections.finish",
    ] {
        assert!(
            !code_only.contains(forbidden),
            "mount.rs 代码行内零合取形态 {forbidden}（两分支独立）"
        );
    }
    assert!(
        code_only.contains("if let Some(mode) = injections.ask"),
        "ask 分支只读 injections.ask"
    );
    assert!(
        code_only.contains("if let Some(spec) = injections.finish"),
        "finish 分支只读 injections.finish"
    );
}

/// 工具名字面量单点（异常，源码自证，AC-4 延伸）：`mount.rs` 全文零
/// `"AskUserQuestion"` 字面量（经 `orchestration::ask::TOOL_NAME` 引用）；
/// `TOOL_NAMES` 常量内亦不含该名（它不是七件之一）。
#[test]
fn mount_rs零ask工具名字面量且不在静态清单() {
    let own = source_of("src/sdk/mount.rs");
    assert!(
        !own.contains("\"AskUserQuestion\""),
        "mount.rs 零工具名字面量（经 TOOL_NAME 单点引用）"
    );
    assert!(
        own.contains("orchestration::ask::TOOL_NAME"),
        "工具名经 ask 模块单点引用"
    );
    assert!(
        !TOOL_NAMES.contains(&"AskUserQuestion"),
        "AskUserQuestion 不在十项静态清单（按位追加的基线工具）"
    );
}
