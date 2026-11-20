//! `mount.rs`（工具挂载单源）的单元测试：十六项静态清单 / 名字查表 / 非法名
//! `Err` / `finish` 位置位铸造 / `lookup` 单点 / 夹具形态 / 类型派生面。
//!
//! 不 mock：`registry()` / `definitions` / `lookup` 全为进程内纯函数
//!（`OnceLock` 一次性静态表，无 IO / 无 spawn / 无网络）；`prompt` 用内存合成
//! 静态清单字符串；`orchestration::finish::{description, parameters}` 真实调用
//!（铸造内容即被测面，替身会掩盖 enum 对拍）。

use agent::{AskMode, FinishSpec, SessionInjections};

use crate::sdk::mount::{
    builtin, definitions, injected, injected_finish, lookup, registry, ToolExec, ToolMount,
    ToolTier, ARCHI_API_TOOL, ARCHI_DRIFT_TOOL, ARCHI_MODULE_TOOL, ARCHI_SCAFFOLD_TOOL,
    ARCHI_SEARCH_TOOL, DB_QUERY_TOOL, DB_TABLES_TOOL, EXPLORE_NOTE_TOOL, TOOL_NAMES,
};

/// 十六项名字的期望序（前七恰七内建，五 archi 只读居中，末三项为两只只读 DB 工具 + 提案工具）。
const EXPECTED_NAMES: [&str; 16] = [
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
    "archi_api",
    "archi_drift",
    "archi_scaffold",
    "db_tables",
    "db_query",
    "propose_config",
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
// 正向：registry 十六项
// ---------------------------------------------------------------------------

#[test]
fn registry十六项名字唯一且有序且前七恰内建() {
    let names = names(registry());
    assert_eq!(names, EXPECTED_NAMES.to_vec(), "十六项名字逐值有序");
    assert_eq!(names.len(), 16, "十六项");

    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 16, "无重复名: {names:?}");

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
    let expected: [(&str, ToolTier); 16] = [
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
        ("archi_api", ToolTier::Read),
        ("archi_drift", ToolTier::Read),
        ("archi_scaffold", ToolTier::Read),
        ("db_tables", ToolTier::Read),
        ("db_query", ToolTier::Read),
        ("propose_config", ToolTier::Read),
    ];
    for (name, tier) in expected {
        let mount = registry()
            .iter()
            .find(|mount| mount.definition.name == name)
            .unwrap_or_else(|| panic!("挂载清单应含 {name}"));
        assert_eq!(mount.tier, tier, "工具 {name} 的 tier 归属");
    }
    // 计数面：Read 十 / Write 三 / Execute 一 / Protocol 零
    let count = |tier: ToolTier| registry().iter().filter(|mount| mount.tier == tier).count();
    assert_eq!(
        count(ToolTier::Read),
        12,
        "Read 十二项（五 archi + 两 DB + 四内建 + propose_config）"
    );
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
    for name in [ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL, ARCHI_API_TOOL] {
        assert_eq!(
            registry()
                .iter()
                .find(|mount| mount.definition.name == name)
                .expect("archi 工具在表内")
                .exec,
            ToolExec::Archi
        );
    }
    // 两 DB 工具共用一个执行体（`ToolExec::Db`），名字由 loop 侧传入
    for name in [DB_TABLES_TOOL, DB_QUERY_TOOL] {
        assert_eq!(
            registry()
                .iter()
                .find(|mount| mount.definition.name == name)
                .expect("DB 工具在表内")
                .exec,
            ToolExec::Db
        );
    }
    assert_eq!(
        registry()
            .iter()
            .filter(|mount| mount.exec == ToolExec::Db)
            .map(|mount| mount.definition.name.as_str())
            .collect::<Vec<_>>(),
        vec![DB_TABLES_TOOL, DB_QUERY_TOOL],
        "`ToolExec::Db` 的成员恰两工具（序即静态表序）"
    );
    assert!(
        !registry()
            .iter()
            .any(|mount| matches!(mount.exec, ToolExec::FinishStep(_))),
        "静态表内零 FinishStep（收口工具按注入面动态铸造）"
    );
}

#[test]
fn registry定义面完整十六项定义形状与非空() {
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
// bash 定义面：`intent` 进必填 + 工具级追加强制句（长度不升为约束）
// ---------------------------------------------------------------------------

/// `intent` 字段描述必须承载的四个取值要素（分工单点：只在这一处出现）。
const INTENT_DESCRIPTION_PIECES: [&str; 4] = ["一句话", "折叠状态", "30 字", "复述命令"];

#[test]
fn bash定义required逐值为command与intent且timeout_ms仍可选() {
    let definition = lookup("bash").expect("bash 定义在案");
    let required = definition.parameters["required"]
        .as_array()
        .expect("required 数组");
    let values: Vec<&str> = required
        .iter()
        .map(|value| value.as_str().expect("字符串"))
        .collect();

    assert_eq!(
        values,
        vec!["command", "intent"],
        "逐值有序：command 先、intent 后"
    );
    assert_eq!(required.len(), 2, "恰两值（不多不少）");
    assert!(
        !values.contains(&"timeout_ms"),
        "timeout_ms 仍可选（不入 required）: {values:?}"
    );
}

#[test]
fn bash定义intent属性为字符串且长度类约束键缺席() {
    let definition = lookup("bash").expect("bash 定义在案");
    let intent = definition.parameters["properties"]["intent"]
        .as_object()
        .unwrap_or_else(|| panic!("intent 属性对象在场: {}", definition.parameters));

    assert_eq!(
        intent.get("type"),
        Some(&serde_json::json!("string")),
        "intent 为字符串"
    );
    let mut keys: Vec<&str> = intent.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["description", "type"],
        "键集恰为 type + description"
    );
    for banned in ["maxLength", "minLength", "pattern", "enum"] {
        assert!(
            intent.get(banned).is_none(),
            "intent 不得带 {banned}（长度只软提示，不升为协议约束）"
        );
    }
}

#[test]
fn bash定义intent描述四要素齐全() {
    let definition = lookup("bash").expect("bash 定义在案");
    let description = definition.parameters["properties"]["intent"]["description"]
        .as_str()
        .expect("intent 描述为字符串");

    for piece in INTENT_DESCRIPTION_PIECES {
        assert!(
            description.contains(piece),
            "字段描述缺要素 `{piece}`: {description}"
        );
    }
}

#[test]
fn bash工具级描述前缀逐字不动且末尾追加强制句() {
    let definition = lookup("bash").expect("bash 定义在案");

    assert!(
        definition
            .description
            .starts_with("执行 shell 命令并返回合并输出"),
        "既有三段描述前缀零改（追加句只能落末尾）: {}",
        definition.description
    );
    assert!(
        definition
            .description
            .ends_with("；每次调用必须给 intent（缺失或空白即拒绝执行）"),
        "末尾追加强制与后果句: {}",
        definition.description
    );
    for piece in ["30 字", "复述命令"] {
        assert!(
            !definition.description.contains(piece),
            "工具级只讲强制与后果，取值纪律 `{piece}` 只在字段描述出现: {}",
            definition.description
        );
    }
}

#[test]
fn bash定义属性键集恰为command与intent与timeout_ms() {
    let definition = lookup("bash").expect("bash 定义在案");
    let properties = definition.parameters["properties"]
        .as_object()
        .expect("properties 对象");

    let mut keys: Vec<&str> = properties.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["command", "intent", "timeout_ms"],
        "无缺失、无多余属性"
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
            protect: None,
            before_done: None,
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
        protect: None,
        before_done: None,
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
fn lookup十六项命中四类未命中() {
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
        protect: None,
        before_done: None,
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
            protect: None,
            before_done: None,
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
    let archi_names = ["read", "archi_module", "archi_search", "archi_api"];
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
                protect: None,
                before_done: None,
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
        "AskUserQuestion 不在十六项静态清单（按位追加的基线工具）"
    );
}

// ---------------------------------------------------------------------------
// 本变更：`archi_api` 只读工具的档位 / 路由 / 入参 schema
// ---------------------------------------------------------------------------

/// 正向（AC-15 / D14 / spec「只读档位归属」）：`lookup("archi_api")` 命中且 `tier == Read`
/// / `exec == Archi`；执行体 `ToolExec::Archi` 的成员恰三工具，分发臂在 loop 侧单处。
#[test]
fn archi_api的档位与执行路由() {
    let mount = registry()
        .iter()
        .find(|mount| mount.definition.name == ARCHI_API_TOOL)
        .expect("archi_api 在静态表内");
    assert_eq!(mount.tier, ToolTier::Read, "只读档位");
    assert_eq!(mount.exec, ToolExec::Archi, "执行路由 Archi");
    assert_eq!(ARCHI_API_TOOL, "archi_api", "常量值");

    // 执行路由面：恰五工具走 `ToolExec::Archi`（名字清单即五常量），
    // 分发臂在 loop 侧按执行体路由（零按名分支）
    let archi_mounts: Vec<&str> = registry()
        .iter()
        .filter(|mount| mount.exec == ToolExec::Archi)
        .map(|mount| mount.definition.name.as_str())
        .collect();
    assert_eq!(
        archi_mounts,
        vec![
            ARCHI_MODULE_TOOL,
            ARCHI_SEARCH_TOOL,
            ARCHI_API_TOOL,
            ARCHI_DRIFT_TOOL,
            ARCHI_SCAFFOLD_TOOL,
        ],
        "`ToolExec::Archi` 的成员恰五工具（序即静态表序）"
    );

    let loop_source = source_of("src/sdk/loop.rs");
    assert_eq!(
        loop_source.matches("ToolExec::Archi =>").count(),
        1,
        "分发臂恰一处（零第二份按名分支）"
    );
    assert!(
        loop_source.contains("ctx.execute(&name, &input)"),
        "分发臂按执行体路由（零按工具名 match）"
    );
}

/// 正向（AC-15）：`policy::allows` 对 `Read` 档在 `AgentPermissionMode` 全部三档恒放行
/// （`archi_api` 属 Read 档，故零放行名单变化）。
#[test]
fn archi_api三档恒放行() {
    use agent::AgentPermissionMode;

    use crate::sdk::policy;

    // `AgentPermissionMode` 封闭三值：Default 拒绝写 / 执行，其余两档全放行；Read 三档恒放行
    for mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        assert!(
            policy::allows(mode, ToolTier::Read),
            "{mode:?} 档对 Read 恒放行"
        );
    }
}

/// 正向（AC-15 / D13）：`archi_api` 的入参 schema 形态——`action` 为封闭二值
/// （`check` / `bootstrap`）、`element` 可选字符串、`required == ["action"]`，描述里含
/// 「生成物过期用既有命令重生成、MUST NOT 手改生成物」要点。
#[test]
fn archi_api入参schema形态() {
    let definition = lookup(ARCHI_API_TOOL).expect("定义在案");
    let parameters = &definition.parameters;
    assert_eq!(parameters["type"], "object", "对象形态");
    assert_eq!(
        parameters["properties"]["action"]["enum"],
        serde_json::json!(["check", "bootstrap"]),
        "action 为封闭二值"
    );
    assert_eq!(
        parameters["properties"]["element"]["type"], "string",
        "element 为字符串（可选）"
    );
    assert_eq!(
        parameters["required"],
        serde_json::json!(["action"]),
        "仅 action 必填"
    );
    for piece in ["MUST NOT", "手改生成物", "重生成"] {
        assert!(
            definition.description.contains(piece),
            "描述含要点 `{piece}`：{}",
            definition.description
        );
    }
}

/// 异常（AC-15 / 回归登记）：未知名 `"archi_write"` / 尾空格 / 空串 → `lookup` 均 `None`
/// （成员校验 fail-closed）；既有十项全 `Some`。
#[test]
fn archi_api非法名与既有查表语义() {
    for miss in ["archi_write", "archi_api ", "", "ARCHI_API"] {
        assert!(lookup(miss).is_none(), "非法名 {miss:?} → None");
    }
    for name in [
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
        ARCHI_API_TOOL,
    ] {
        assert!(lookup(name).is_some(), "在册名 {name} 应 Some");
    }
}

// ---------------------------------------------------------------------------
// 本变更：两 DB 只读工具的挂载面（定义形状 / tier / 下发面 / 词表同源）
// ---------------------------------------------------------------------------

/// 正向（AC-9 / AC-1）：两 DB 工具在静态表末两位且 `tier == Read`、
/// `exec == ToolExec::Db`；名字常量取线值。
#[test]
fn db两工具在静态表末两位且档位与路由() {
    let tail: Vec<&str> = registry()
        .iter()
        .rev()
        .take(3)
        .map(|mount| mount.definition.name.as_str())
        .collect();
    assert_eq!(
        tail,
        vec![
            crate::sdk::config_propose::PROPOSE_CONFIG_TOOL,
            DB_QUERY_TOOL,
            DB_TABLES_TOOL
        ],
        "末三位为两 DB 工具 + 提案工具（序 = 登记序 db_tables → db_query → propose_config）"
    );
    for name in [DB_TABLES_TOOL, DB_QUERY_TOOL] {
        let mount = registry()
            .iter()
            .find(|mount| mount.definition.name == name)
            .expect("DB 工具在表内");
        assert_eq!(mount.tier, ToolTier::Read, "{name} 为只读档位");
        assert_eq!(mount.exec, ToolExec::Db, "{name} 执行路由 Db");
    }
    assert_eq!(DB_TABLES_TOOL, "db_tables", "常量线值");
    assert_eq!(DB_QUERY_TOOL, "db_query", "常量线值");

    // 分发臂在 loop 侧单处（零按工具名分支）
    let loop_source = source_of("src/sdk/loop.rs");
    assert_eq!(
        loop_source.matches("ToolExec::Db =>").count(),
        1,
        "Db 分发臂恰一处"
    );
    assert!(
        loop_source.contains("专用工具执行上下文未装配"),
        "上下文缺席以 is_error 回灌（与 explore / archi 同文案）"
    );
}

/// 正向（AC-1）：`db_tables` 定义描述覆盖两形态语义 + scope 缺省口径；
/// 参数 shape 为两属性 + `required == []`。
#[test]
fn db_tables定义描述与参数形状() {
    let definition = lookup(DB_TABLES_TOOL).expect("定义在案");
    assert_eq!(definition.name, DB_TABLES_TOOL);
    let parameters = &definition.parameters;
    assert_eq!(parameters["type"], "object", "对象形态");
    assert_eq!(
        parameters["required"],
        serde_json::json!([]),
        "无必填参数（两形态皆可省略 table）"
    );
    assert_eq!(
        parameters["properties"]["scope"]["enum"],
        serde_json::json!(["user", "workspace"]),
        "scope 为两值 enum"
    );
    assert_eq!(
        parameters["properties"]["table"]["type"], "string",
        "table 为字符串"
    );
    for piece in [
        "table",
        "summary",
        "dimension",
        "count",
        "scope",
        "user",
        "workspace",
        "db_query",
        "落库线名",
    ] {
        assert!(
            definition.description.contains(piece),
            "描述含要点 `{piece}`：{}",
            definition.description
        );
    }
    // scope 缺省口径两处都得写（省略 = 两库全列；描述与参数 schema 同源）
    for text in [
        definition.description.as_str(),
        parameters["properties"]["scope"]["description"]
            .as_str()
            .expect("scope 参数说明为字符串"),
    ] {
        assert!(
            text.contains("缺省"),
            "scope 缺省口径（省略 = 两库全列）不可缺: {text}"
        );
    }
}

/// 正向（AC-4 / AC-6 / AC-7）：`db_query` 定义描述与 `QUERY_OPS` 同源（10 词逐
/// 词）、覆盖 scope / limit 缺省与上限 / 预算实际生效 / 自读警示；参数七属性 +
/// `required == ["table"]`。
#[test]
fn db_query定义描述与参数形状() {
    let definition = lookup(DB_QUERY_TOOL).expect("定义在案");
    assert_eq!(definition.name, DB_QUERY_TOOL);
    let parameters = &definition.parameters;
    assert_eq!(parameters["type"], "object", "对象形态");
    assert_eq!(
        parameters["required"],
        serde_json::json!(["table"]),
        "仅 table 必填"
    );
    for property in [
        "table", "scope", "where", "order", "select", "offset", "limit",
    ] {
        assert!(
            parameters["properties"].get(property).is_some(),
            "参数含 {property}：{parameters}"
        );
    }
    assert_eq!(
        parameters["properties"]["scope"]["enum"],
        serde_json::json!(["user", "workspace"]),
        "scope 为两值 enum"
    );
    assert_eq!(
        parameters["properties"]["where"]["items"]["properties"]["op"]["enum"],
        serde_json::json!(store::QUERY_OPS),
        "op 词表与 `QUERY_OPS` 同一份清单"
    );

    for word in store::QUERY_OPS {
        assert!(
            definition.description.contains(word),
            "描述含 op 词表逐词 `{word}`：{}",
            definition.description
        );
    }
    for piece in [
        "user",
        "workspace",
        "limit 缺省 20",
        "上限 1000",
        "40 KB",
        "实际生效值",
        "db_tables",
        "select",
        "offset",
        "session_event",
        "本会话自身转录",
    ] {
        assert!(
            definition.description.contains(piece),
            "描述含要点 `{piece}`：{}",
            definition.description
        );
    }
    // scope 缺省口径（缺省 workspace）——描述与参数 schema 两处都得写
    assert!(
        definition.description.contains("缺省"),
        "描述含 scope 缺省语义：{}",
        definition.description
    );
    let scope_doc = parameters["properties"]["scope"]["description"]
        .as_str()
        .expect("scope 参数说明为字符串");
    assert!(
        scope_doc.contains("缺省 workspace"),
        "scope 参数说明写明缺省 workspace：{scope_doc}"
    );
}

/// 正向（AC-8）：两 DB 工具 `tier = Read` → 三档全放行（无单独放行名单）。
#[test]
fn db两工具三档恒放行() {
    use agent::AgentPermissionMode;

    use crate::sdk::policy;

    for name in [DB_TABLES_TOOL, DB_QUERY_TOOL] {
        let tier = registry()
            .iter()
            .find(|mount| mount.definition.name == name)
            .expect("DB 工具在表内")
            .tier;
        for mode in [
            AgentPermissionMode::Default,
            AgentPermissionMode::AcceptEdits,
            AgentPermissionMode::BypassPermissions,
        ] {
            assert!(
                policy::allows(mode, tier),
                "{name} 在 {mode:?} 档放行（Read 恒放行）"
            );
        }
    }
}

/// 正向（AC-9）：两 DB 名进清单即下发，序按 registry（清单序不决定下发序）；
/// `tools = None` 仍恰前七项（不含两 DB 名）。
#[test]
fn definitions_db名下发按registry序且tools_none零db名() {
    let mounts = definitions(&injected(&["db_tables", "read"]), "").expect("合法名");
    assert_eq!(
        names(&mounts),
        vec!["read", DB_TABLES_TOOL],
        "清单序不决定下发序（`read` 在前）"
    );
    let reversed = definitions(&injected(&["db_query", "db_tables"]), "").expect("合法名");
    assert_eq!(
        names(&reversed),
        vec![DB_TABLES_TOOL, DB_QUERY_TOOL],
        "两 DB 名同下发时按 registry 序"
    );

    let base = definitions(&SessionInjections::default(), "").expect("缺省注入面合法");
    for name in [DB_TABLES_TOOL, DB_QUERY_TOOL] {
        assert!(
            !names(&base).contains(&name.to_owned()),
            "`tools: None` 不含 {name}（内建七件语义零改动）"
        );
    }
    // 两 DB 名 + finish 位：收口工具仍尾追加（两分支互不依赖）
    let with_finish = definitions(
        &injected_finish(
            &["db_tables", "db_query"],
            Some(agent::FinishSpec::Decision),
        ),
        "## 静态清单\n",
    )
    .expect("合法名 + 收口位");
    assert_eq!(
        names(&with_finish),
        vec![
            DB_TABLES_TOOL,
            DB_QUERY_TOOL,
            orchestration::finish::TOOL_NAME
        ],
        "收口工具尾追加"
    );
}

/// 异常（AC-11）：未知名仍 `Err` 携非法名，不返回部分清单。
#[test]
fn definitions_db近似名仍err() {
    for miss in ["db_query2", "DB_QUERY", "db tables"] {
        let error = definitions(&injected(&["read", miss]), "").expect_err("未知名应 Err");
        assert!(error.contains(miss), "文案携非法名 {miss:?}: {error}");
    }
}

// ===========================================================================
// `archi_module` 描述的 `doc` 纪律（源码自证：旧关联面词汇零命中）
// ===========================================================================

/// 异常：`mount.rs` 内旧关联面词汇零命中——「关联文档」「四字段条目」「`docNotFound` 警告」
/// 与 `WarningKind` 引用一律退场（`doc` 纪律两半由 `tools_test.rs` 的行为面承载）。
#[test]
fn mount描述零旧关联面词汇() {
    let source = include_str!("mount.rs");
    for stale in [
        "关联文档",
        "四字段条目",
        "docNotFound",
        "WarningKind",
        "docs",
    ] {
        assert!(
            !source.contains(stale),
            "`mount.rs` 内旧关联面词汇零命中：{stale}"
        );
    }
}

/// 正向：`archi_module` 描述仍点名入口文档单值 `doc` 的两半（留白可写 / `declared` 走
/// `api.contractPath`），且与 `archi_search` 的「入口文档路径」表述同源。
#[test]
fn mount描述含doc纪律两半() {
    let source = include_str!("mount.rs");
    for piece in ["单值 doc", "api = none", "api.contractPath", "入口文档路径"] {
        assert!(
            source.contains(piece),
            "`mount.rs` 内 doc 纪律片段在案：{piece}"
        );
    }
}

/// 边界：三工具的 `enum` 词面零 `docs`（`search` 的 scope 三值单数形态），零
/// `findings` 槽位词进描述。
#[test]
fn mount描述enum词面零旧值() {
    let source = include_str!("mount.rs");
    assert!(
        source.contains("\"elements\", \"doc\", \"all\""),
        "scope 三值单数形态：{source}"
    );
    assert!(!source.contains("\"docs\""), "零旧词 \"docs\"");
}

// ---------------------------------------------------------------------------
// 本变更：`archi_drift` / `archi_scaffold` 定义面与 `archi_module` 描述改述
// ---------------------------------------------------------------------------

/// 正向：`archi_drift` 描述含三要素——漂移读数语义、**「`gate` 全 `off` 时
/// `unmatchedFiles` 是常态不是错误」**、`files` 只在排查归属问题时用。
#[test]
fn archi_drift定义描述三要素() {
    let definition = lookup(ARCHI_DRIFT_TOOL).expect("定义在案");
    assert_eq!(ARCHI_DRIFT_TOOL, "archi_drift", "常量值");
    let description = &definition.description;
    for piece in ["漂移", "全量", "零写盘"] {
        assert!(
            description.contains(piece),
            "描述含读数语义 `{piece}`：{description}"
        );
    }
    assert!(
        description.contains("gate 全 off 时 unmatchedFiles"),
        "描述点名常态量：{description}"
    );
    assert!(
        description.contains("常态不是错误"),
        "描述把常态与错误分开：{description}"
    );
    assert!(
        description.contains("files 只在排查归属问题时用"),
        "描述限定 files 的使用时机：{description}"
    );
}

/// 边界：`archi_drift` 的 `scope` enum 逐值等于 `DRIFT_SCOPES`、`required` 为空数组、
/// 零 `mode` / 零 `root` 参数（恒全量的协议面）。
#[test]
fn archi_drift定义scope词表单点且零多余参数() {
    let definition = lookup(ARCHI_DRIFT_TOOL).expect("定义在案");
    let parameters = &definition.parameters;
    let scope = &parameters["properties"]["scope"];
    let enum_values: Vec<&str> = scope["enum"]
        .as_array()
        .expect("enum 为数组")
        .iter()
        .map(|value| value.as_str().expect("enum 元素为字符串"))
        .collect();
    assert_eq!(
        enum_values,
        crate::sdk::archi_drift_view::DRIFT_SCOPES.to_vec(),
        "enum 与 `DRIFT_SCOPES` 逐值相等（词表单源）"
    );
    assert_eq!(scope["type"], "string");
    assert_eq!(
        parameters["required"],
        serde_json::json!([]),
        "required 为空数组（零必填参数）"
    );
    let keys: Vec<&str> = parameters["properties"]
        .as_object()
        .expect("properties 为对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["scope"], "零 mode / 零 root 参数（恒全量）");
}

/// 正向：`archi_scaffold` 描述含「字段序 / 内联 `metadata` / `path` 必填提示 / 关系模板」
/// 「`kind` 省略取 `domain`」「零写盘、写面仍是 `write` / `edit`」。
#[test]
fn archi_scaffold定义描述要素() {
    let definition = lookup(ARCHI_SCAFFOLD_TOOL).expect("定义在案");
    assert_eq!(ARCHI_SCAFFOLD_TOOL, "archi_scaffold", "常量值");
    let description = &definition.description;
    for piece in [
        "字段序",
        "内联 metadata",
        "path 必填提示",
        "关系模板",
        "kind 省略取 domain",
        "只读零写盘",
        "write / edit",
    ] {
        assert!(
            description.contains(piece),
            "描述含要素 `{piece}`：{description}"
        );
    }
}

// ---------------------------------------------------------------------------
// 本变更：`propose_config` 挂载面（定义单源 / tier 与执行体 / 源码结构锚）
// ---------------------------------------------------------------------------

/// 正向：末项 `tier == Read` / `exec == ToolExec::Config`，定义与执行体模块
/// 同源（名字在挂载清单、描述在执行体模块的两处分叉由本断言堵住）。
#[test]
fn 提案工具在末项且档位与路由与定义同源() {
    let mount = registry()
        .iter()
        .find(|mount| mount.definition.name == crate::sdk::config_propose::PROPOSE_CONFIG_TOOL)
        .expect("提案工具在表内");
    assert_eq!(mount.tier, ToolTier::Read, "只读档位（三档恒放行）");
    assert_eq!(mount.exec, ToolExec::Config, "执行路由 Config");
    let expected = crate::sdk::config_propose::definition();
    assert_eq!(mount.definition.name, expected.name, "定义名同源");
    assert_eq!(
        mount.definition.parameters, expected.parameters,
        "参数面同源（定义单源在 config_propose）"
    );
    assert_eq!(
        mount.definition.description, expected.description,
        "描述同源（无「名字在清单、描述在执行体」分叉）"
    );
}

/// 边界：源码结构锚——`Vec::with_capacity(16)` 恰一处、两段 doc 的「十六项」在场。
#[test]
fn 提案工具挂载面的源码结构锚() {
    let source = source_of("src/sdk/mount.rs");
    assert_eq!(
        source.matches("Vec::with_capacity(16)").count(),
        1,
        "构造容量随末项随动（恰一处）"
    );
    assert_eq!(
        source.matches("十六项静态挂载清单").count(),
        1,
        "registry 段 doc 的「十六项静态挂载清单」在场"
    );
    assert_eq!(
        source.matches("十六个字面构造项按序").count(),
        1,
        "build_registry 段 doc 的项数随动（十六个字面构造项）"
    );
    assert_eq!(
        source.matches("PROPOSE_CONFIG_TOOL").count(),
        1,
        "再导出恰一处（常数本体驻执行体模块）"
    );
}

/// 边界：`registry()` 两次调用字符串序相同（`OnceLock` 单次构建，构造函数零随机序）。
#[test]
fn registry两次调用名字序相同() {
    let first = names(registry());
    let second = names(registry());
    assert_eq!(first, second, "同一份静态表（OnceLock 单次构建）");
    assert!(
        std::ptr::eq(registry(), registry()),
        "两次调用返回同一份切片内存"
    );
}

/// 正向：`definitions(tools: Some(六名))` → 恰 6 项，序与清单序一致、逐项路由对位。
#[test]
fn 提案工具在六名注入面下按清单序下发() {
    let injections = SessionInjections {
        preamble: None,
        tools: Some(
            [
                "read",
                "grep",
                "glob",
                "ls",
                "bash",
                crate::sdk::config_propose::PROPOSE_CONFIG_TOOL,
            ]
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        ),
        finish: None,
        ask: None,
        protect: None,
        before_done: None,
    };
    let mounts = definitions(&injections, "").expect("config 六名装配应成功");
    assert_eq!(names(&mounts).len(), 6, "恰六项");
    let names = names(&mounts);
    assert_eq!(
        names,
        vec!["read", "grep", "glob", "ls", "bash", "propose_config"],
        "序与挂载清单序一致（read → grep → glob → ls → bash → propose_config）"
    );
    let last = mounts.last().expect("末项在场");
    assert_eq!(last.tier, ToolTier::Read);
    assert_eq!(last.exec, ToolExec::Config);
    for mount in &mounts[..5] {
        assert_eq!(
            mount.exec,
            ToolExec::Pure,
            "{} 走纯函数执行体",
            mount.definition.name
        );
    }
}

/// 边界：`tools: None` 仍只取前七内建（`propose_config` 不进 `tools: None` 语义）；
/// `ask = Generic` 末尾追加 ask 工具、`finish = None` 不追加 `finish_step`。
#[test]
fn 提案工具不进tools_none语义且ask位独立追加() {
    let injections = SessionInjections {
        preamble: None,
        tools: None,
        finish: None,
        ask: Some(AskMode::Generic),
        protect: None,
        before_done: None,
    };
    let mounts = definitions(&injections, "").expect("内建名单装配应成功");
    let mut expected: Vec<String> = TOOL_NAMES.iter().map(|name| (*name).to_owned()).collect();
    expected.push("AskUserQuestion".to_owned());
    assert_eq!(names(&mounts), expected, "七内建 + ask 工具（末位）");
    assert!(
        !names(&mounts).iter().any(|name| name == "propose_config"),
        "`tools: None` 语义载体仍是七内建（提案工具不入）"
    );
    assert!(
        !names(&mounts).iter().any(|name| name == "finish_step"),
        "finish = None → 不追加收口工具"
    );
}

/// 正向：`lookup(\"propose_config\")` 命中且定义名逐字相同。
#[test]
fn lookup命中提案工具名() {
    let definition =
        lookup(crate::sdk::config_propose::PROPOSE_CONFIG_TOOL).expect("在册名应 Some");
    assert_eq!(
        definition.name,
        crate::sdk::config_propose::PROPOSE_CONFIG_TOOL
    );
    assert_eq!(
        definition.parameters,
        crate::sdk::config_propose::definition().parameters
    );
}

/// 异常（矩阵 4 形态）：近似名全 `None`（精确名判据，非前缀 / 大小写 / 空白嗅探）。
#[test]
fn lookup提案工具近似名全none() {
    for name in [
        "propose-config",
        "ProposeConfig",
        "propose_config ",
        " propose_config",
        "propose_configx",
    ] {
        assert!(
            lookup(name).is_none(),
            "近似名 {name:?} 必须 miss（精确名判据）"
        );
    }
}

/// 边界：`archi_scaffold` 的 `kind` enum 与 `ElementKind::ALL.map(as_str)` 逐值相等
/// （长度 4、序一致，零第二份字面量）。
#[test]
fn archi_scaffold定义kind词表派生() {
    let definition = lookup(ARCHI_SCAFFOLD_TOOL).expect("定义在案");
    let enum_values: Vec<&str> = definition.parameters["properties"]["kind"]["enum"]
        .as_array()
        .expect("enum 为数组")
        .iter()
        .map(|value| value.as_str().expect("enum 元素为字符串"))
        .collect();
    let expected = archi::dto::ElementKind::ALL
        .map(archi::dto::ElementKind::as_str)
        .to_vec();
    assert_eq!(enum_values, expected, "kind enum 由词表派生");
    assert_eq!(enum_values.len(), 4, "四值封闭");
    assert_eq!(
        definition.parameters["required"],
        serde_json::json!([]),
        "kind 可选（省略取 domain）"
    );
}

/// 正向（表驱动）：`archi_module` 描述改述后的三句——`hasPath` 判缺 `path`、入口解析
/// 结果随响应给出（显式 / 约定默认 / 不可用成因）、新元素字段形态用 `archi_scaffold`。
#[test]
fn archi_module描述补三句指路() {
    let definition = lookup(ARCHI_MODULE_TOOL).expect("定义在案");
    let description = &definition.description;
    for piece in ["hasPath", "resolvedEntry", "archi_scaffold"] {
        assert!(
            description.contains(piece),
            "描述含新指路 `{piece}`：{description}"
        );
    }
    assert!(
        description.contains("入口解析结果") || description.contains("显式命中"),
        "入口解析三态有交代：{description}"
    );
}

/// 边界：`archi_module` 描述删除字段形态教学——零代码围栏、零 `entry` 三档表；
/// `modelFile` + `modelLine` 可直接 `edit` 的句子保留。
#[test]
fn archi_module描述零字段形态教学() {
    let definition = lookup(ARCHI_MODULE_TOOL).expect("定义在案");
    let description = &definition.description;
    assert!(
        !description.contains("```"),
        "零代码围栏（示例 TOML 归 archi_scaffold）：{description}"
    );
    assert!(
        !description.contains("entry 三档") && !description.contains("三档"),
        "零 entry 三档表：{description}"
    );
    assert!(
        description.contains("modelFile") && description.contains("modelLine"),
        "声明位置可直接 edit 的句子保留：{description}"
    );
    assert!(description.contains("edit"), "edit 指路在场");
}

/// 边界：`injected_finish` 夹具的 `SessionInjections` 含 `protect: None`（第五位机械
/// 闭合，编译期 + 断言双守）。
#[test]
fn 夹具injected_finish第五位protect为none() {
    let injections = injected_finish(&["read"], Some(agent::FinishSpec::EvaluatorChecklist));
    assert!(
        injections.protect.is_none(),
        "夹具不引入会话规则（第五位显式 None）"
    );
}

/// 异常：清单含未知名 → `Err` 文案前缀逐字不变（末项不改变该判据）。
#[test]
fn 清单含未知名err文案前缀不变() {
    let injections = SessionInjections {
        preamble: None,
        tools: Some(vec!["propose_config".to_owned(), "config_write".to_owned()]),
        finish: None,
        ask: None,
        protect: None,
        before_done: None,
    };
    let error = definitions(&injections, "").expect_err("未知名必须 Err");
    assert!(
        error.starts_with("注入面非法工具名"),
        "文案前缀逐字不变，实际 {error}"
    );
    assert!(error.contains("config_write"), "文案携非法名，实际 {error}");
}

/// 边界：`TOOL_NAMES` 仍七件且逐字不变（`tools: None` 面契约零位移）。
#[test]
fn 内建七名逐字不变() {
    assert_eq!(
        TOOL_NAMES.to_vec(),
        vec!["read", "grep", "glob", "ls", "write", "edit", "bash"]
    );
    assert_eq!(builtin(), TOOL_NAMES.as_slice());
    assert_eq!(builtin().len(), 7, "内建封闭七件");
}
