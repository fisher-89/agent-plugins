//! `mount.rs`（工具挂载单源）的单元测试：十项静态清单 / 名字查表 / 非法名
//! `Err` / `finish` 位置位铸造 / `lookup` 单点 / 夹具形态 / 类型派生面。
//!
//! 不 mock：`registry()` / `definitions` / `lookup` 全为进程内纯函数
//!（`OnceLock` 一次性静态表，无 IO / 无 spawn / 无网络）；`prompt` 用内存合成
//! 静态清单字符串；`orchestration::finish::{description, parameters}` 真实调用
//!（铸造内容即被测面，替身会掩盖 enum 对拍）。

use agent::{FinishSpec, SessionInjections};

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
    assert_eq!(actions, vec!["backtrack", "stop", "ask"]);
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
