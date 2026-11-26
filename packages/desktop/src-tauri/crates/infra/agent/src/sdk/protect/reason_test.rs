//! `reason` 的单元测试（AC-2 / AC-8）：拒绝指令文案三态选串（内置 / 用户模板 /
//! 通用回落）与批量还原专用串的逐字面断言。
//!
//! 断言口径：文案模板为编译期常量、函数纯字符串运算（零 IO 零依赖），故全部
//! 用例以字符串字面量入参直驱，不建目录、不读配置。
//!
//! D2 命名隔离纪律的行为面锚：模板内禁域目录名 / 配置文件名字面量——以中性
//! 路径调用时产物不含 `openspec` / `config.json`（路径原文除外）。

use super::reason;
use super::rules::ReasonKind;

/// 用户带 reason 的规则载体（三态之一，构造集中）。
fn user_with(template: &str) -> ReasonKind {
    ReasonKind::User(Some(template.to_owned()))
}

/// 两句功能句（掐断换路链的唯一手段，四条文案一律携带）。
const STOP_SENTENCE: &str = "请立即停止并通知用户";
const NO_BYPASS_SENTENCE: &str = "不要改用其他命令或工具尝试绕过";

// ---------------------------------------------------------------------------
// deny_reason（三态选串）
// ---------------------------------------------------------------------------

#[test]
fn 内置串含路径配置权威调整指引与两句功能句() {
    let path = "/tmp/ws/.dev-team/config.json";
    let reason = reason::deny_reason(&ReasonKind::BuiltinConfig, path, "write");

    assert!(reason.contains(path), "① 路径原文在串内，实际: {reason}");
    assert!(
        reason.contains("配置权威") && reason.contains("不得改写"),
        "② 工作区配置权威 / Agent 不得改写语义在场，实际: {reason}"
    );
    assert!(
        reason.contains("改哪个字段") && reason.contains("改成什么") && reason.contains("为什么"),
        "③ 「改哪个字段、改成什么、为什么」指引在场，实际: {reason}"
    );
    assert!(
        reason.contains(STOP_SENTENCE),
        "④ 无法规避立即停止并通知用户，实际: {reason}"
    );
    assert!(
        reason.contains(NO_BYPASS_SENTENCE),
        "⑤ 不得改用其他命令或工具绕过，实际: {reason}"
    );
    assert!(
        reason.contains("write"),
        "工具名 %t 已替换（模型可辨拒绝来源），实际: {reason}"
    );
}

#[test]
fn 用户模板全量替换百分号s与百分号t() {
    let reason = reason::deny_reason(
        &user_with("受保护：%s（%t）不可写"),
        "/root/docs/a.md",
        "bash",
    );
    assert_eq!(
        reason, "受保护：/root/docs/a.md（bash）不可写",
        "用户模板 %s → 路径、%t → 工具名"
    );

    // 多处占位符：3 处 %s + 2 处 %t 全量替换（零残留）
    let repeated = reason::deny_reason(&user_with("%s|%s|%s|%t|%t"), "/a/b", "edit");
    assert_eq!(repeated, "/a/b|/a/b|/a/b|edit|edit", "全量替换所有出现");
    assert!(
        !repeated.contains("%s") && !repeated.contains("%t"),
        "零残留占位符，实际: {repeated}"
    );
}

#[test]
fn 无reason落通用回落串含路径工具名与两句功能句() {
    let path = "/root/plugins/dev-team/bin/knip.json";
    let reason = reason::deny_reason(&ReasonKind::User(None), path, "edit");

    assert!(reason.contains(path), "路径原文在场，实际: {reason}");
    assert!(reason.contains("edit"), "工具名在场，实际: {reason}");
    assert!(
        reason.contains("请调整方案"),
        "回落串携带调整方案引导，实际: {reason}"
    );
    assert!(
        reason.contains(STOP_SENTENCE),
        "两句功能句之一，实际: {reason}"
    );
    assert!(
        reason.contains(NO_BYPASS_SENTENCE),
        "两句功能句之二，实际: {reason}"
    );
}

#[test]
fn 三态同一入参产物两两不等且均含两句功能句() {
    let path = "/root/docs/a.md";
    let tool = "edit";
    let builtin = reason::deny_reason(&ReasonKind::BuiltinConfig, path, tool);
    let custom = reason::deny_reason(&user_with("自有文案：%s"), path, tool);
    let fallback = reason::deny_reason(&ReasonKind::User(None), path, tool);
    let bulk = reason::bulk_revert_reason("git stash 会批量改写工作区", tool);

    let all = [&builtin, &custom, &fallback, &bulk];
    for (left_index, left) in all.iter().enumerate() {
        for right in all.iter().skip(left_index + 1) {
            assert_ne!(left, right, "四条模板两两不等（无粘连段落）: {left}");
        }
    }
    // 两句功能句在「引擎自产」三态（内置 / 回落 / 批量）必含；用户模板为原样
    // 透传（文案责任在配置侧），故不参与该断言
    for reason in [&builtin, &fallback, &bulk] {
        assert!(reason.contains(STOP_SENTENCE), "模板缺停止句: {reason}");
        assert!(
            reason.contains(NO_BYPASS_SENTENCE),
            "模板缺禁绕过句: {reason}"
        );
    }
}

#[test]
fn 用户模板无占位符原样返回_空路径不残留占位符() {
    assert_eq!(
        reason::deny_reason(&user_with("原文文案"), "/a/b", "write"),
        "原文文案",
        "无占位符即原文（不做多余包装）"
    );
    for path in ["", "   "] {
        let reason = reason::deny_reason(&user_with("保护 %s（%t）"), path, "bash");
        assert!(
            !reason.contains("%s") && !reason.contains("%t"),
            "空值 / 全空白路径替换为空值且零残留，实际: {reason:?}"
        );
    }
}

#[test]
fn 内置串不含目录名与文件名硬编码字面量() {
    // D2 命名隔离纪律的行为面：模板不得复述域目录名 / 配置文件名（身份由
    // ReasonKind 表达），产物中两字面量仅可能来自入参路径本身
    let reason = reason::deny_reason(&ReasonKind::BuiltinConfig, "/tmp/ws/x.md", "write");
    assert!(
        !reason.contains("openspec"),
        "内置模板零域目录名字面量，实际: {reason}"
    );
    assert!(
        !reason.contains("config.json"),
        "内置模板零配置文件名字面量，实际: {reason}"
    );
}

#[test]
fn 超长与特殊字符路径原样注入不截断() {
    let long_path = format!("/tmp/{}/a.md", "长".repeat(1_200));
    let reason = reason::deny_reason(&ReasonKind::BuiltinConfig, &long_path, "write");
    assert!(reason.contains(&long_path), "1000+ 字符路径原样注入不截断");

    for path in ["/ws/中文路径🚀/a.md", "/ws/a\nb\0c.md"] {
        let reason = reason::deny_reason(&ReasonKind::BuiltinConfig, path, "write");
        assert!(
            reason.contains(path),
            "emoji / 换行 / NUL 路径不 panic 且原样注入，实际: {reason:?}"
        );
    }

    // 超长模板替换后长度 ≈ 模板 + 路径（全量替换不丢字符）
    let long_template = "前缀".repeat(500) + "%s";
    let reason = reason::deny_reason(&user_with(&long_template), "/p", "write");
    assert_eq!(
        reason.chars().count(),
        long_template.chars().count() - 2 + 2,
        "占位符替换后长度守恒（%s → 两字符路径）"
    );
}

#[test]
fn 同入参两次调用逐字节相等() {
    let first = reason::deny_reason(
        &ReasonKind::BuiltinConfig,
        "/ws/.dev-team/config.json",
        "bash",
    );
    let second = reason::deny_reason(
        &ReasonKind::BuiltinConfig,
        "/ws/.dev-team/config.json",
        "bash",
    );
    assert_eq!(first, second, "纯函数无隐藏状态（确定性）");
}

// ---------------------------------------------------------------------------
// substitute（占位符全量替换）
// ---------------------------------------------------------------------------

#[test]
fn substitute全量替换所有占位符() {
    assert_eq!(reason::substitute("%s %s %s", "p", "t"), "p p p");
    assert_eq!(reason::substitute("%t|%t", "p", "bash"), "bash|bash");
    assert_eq!(reason::substitute("%s", "/a/b", "edit"), "/a/b");
}

#[test]
fn substitute无占位符与空串边界() {
    assert_eq!(
        reason::substitute("no placeholder", "p", "t"),
        "no placeholder",
        "无占位符原样"
    );
    assert_eq!(reason::substitute("", "p", "t"), "", "空模板空产物");
    assert_eq!(
        reason::substitute("%s", "", ""),
        "",
        "空值替换为空值且不 panic"
    );
}

#[test]
fn substitute非占位符百分号与大小写变体原样保留() {
    assert_eq!(
        reason::substitute("100% 完成", "p", "t"),
        "100% 完成",
        "非占位符百分号逐字保留（不吞字符、不 panic）"
    );
    assert_eq!(
        reason::substitute("%S", "p", "t"),
        "%S",
        "占位符大小写敏感（%S 不作替换）"
    );
}

// ---------------------------------------------------------------------------
// bulk_revert_reason（批量还原专用串）
// ---------------------------------------------------------------------------

#[test]
fn 批量还原串含事实串逐路径引导工具名与两句功能句() {
    let detail = "git stash 会批量改写工作区";
    let reason = reason::bulk_revert_reason(detail, "bash");

    assert!(
        reason.contains(detail),
        "中间事实串原样嵌入，实际: {reason}"
    );
    assert!(
        reason.contains("git restore <path>"),
        "逐路径还原引导在场，实际: {reason}"
    );
    assert!(reason.contains("bash"), "工具名在场，实际: {reason}");
    assert!(reason.contains(STOP_SENTENCE), "停止句在场，实际: {reason}");
    assert!(
        reason.contains(NO_BYPASS_SENTENCE),
        "禁绕过句在场，实际: {reason}"
    );
}

#[test]
fn 批量还原串detail边界与确定性() {
    for detail in ["", "中文事实串🚀", &"超长事实".repeat(300)] {
        let first = reason::bulk_revert_reason(detail, "bash");
        let second = reason::bulk_revert_reason(detail, "bash");
        assert_eq!(first, second, "同入参两次调用相等（确定性）");
        assert!(first.contains(detail), "detail 原样嵌入不截断: {first}");
    }
}

// ===========================================================================
// 本变更：会话级相位锁模板（`session_lock_template`）与 `ReasonKind::Session`
// ===========================================================================

/// 会话规则载体（`deny_reason` 的显式入参）。
fn session_reason() -> ReasonKind {
    ReasonKind::Session(reason::session_lock_template().to_owned())
}

/// 正向（表驱动）：模板四要素逐条在场——`%s`（路径位）/ `%t`（工具位）/ 写权相位
/// 政策 / 补救指路 / 不得绕过。
#[test]
fn session_lock_template四要素齐备() {
    let template = reason::session_lock_template();
    for piece in [
        "%s",
        "%t",
        "proposal / dev-design 相位的执行侧（以及架构页会话）",
        "design.md",
        "tasks.md",
        "dev-design 相位落地",
        "架构页会话",
        NO_BYPASS_SENTENCE,
    ] {
        assert!(template.contains(piece), "模板含要素 `{piece}`：{template}");
    }
    assert!(template.contains("相位锁"), "事实串点明相位锁：{template}");
}

/// 边界：模板零架构域路径字面量（路径只经 `%s` 携入）；同值两次调用返回同一
/// `&'static str`（单点出口）。
#[test]
fn session_lock_template零路径字面量与单点出口() {
    let template = reason::session_lock_template();
    for literal in [
        ".dev-team",
        "architecture",
        "graph/model",
        "config.json",
        "INDEX.toml",
    ] {
        assert!(
            !template.contains(literal),
            "模板零路径字面量 `{literal}`：{template}"
        );
    }
    assert!(
        std::ptr::eq(template, reason::session_lock_template()),
        "同值两次调用返回同一 `&'static str`"
    );
}

/// 正向（表驱动）：`deny_reason(Session)` 做 `%s` / `%t` 全量替换（零残留），输出含
/// 实际路径与实际工具名。
#[test]
fn deny_reason_session三工具全量替换() {
    let path = "/repo/.dev-team/architecture/graph/model/01-core.toml";
    for tool in ["write", "edit", "bash"] {
        let text = reason::deny_reason(&session_reason(), path, tool);
        assert!(text.contains(path), "含实际路径（{tool}）：{text}");
        assert!(text.contains(tool), "含实际工具名（{tool}）：{text}");
        assert!(
            !text.contains("%s") && !text.contains("%t"),
            "占位符零残留（{tool}）：{text}"
        );
    }
}

/// 边界（表驱动）：路径含特殊字符（空格 / `：` / emoji）→ 原样插入、不转义零 panic。
#[test]
fn deny_reason_session特殊字符原样插入() {
    for path in [
        "/repo/with space/model/01-core.toml",
        "/repo/中文：目录/model.toml",
        "/repo/🎉emoji/model.toml",
    ] {
        let text = reason::deny_reason(&session_reason(), path, "write");
        assert!(text.contains(path), "原样插入：{text}");
        assert!(!text.contains("\\u"), "不转义：{text}");
    }
}

/// 边界：空 path / 空 tool → 占位替换为空串，文案结构完整。
#[test]
fn deny_reason_session空入参结构完整() {
    let empty_path = reason::deny_reason(&session_reason(), "", "write");
    assert!(!empty_path.contains('%'), "占位符零残留：{empty_path}");
    assert!(
        empty_path.contains("相位锁"),
        "结构完整（事实串仍在）：{empty_path}"
    );

    let empty_tool = reason::deny_reason(&session_reason(), "/repo/x.toml", "");
    assert!(!empty_tool.contains('%'), "占位符零残留：{empty_tool}");
    assert!(
        empty_tool.contains("/repo/x.toml"),
        "路径仍插入：{empty_tool}"
    );
}

/// 异常：模板无占位符（构造 `ReasonKind::Session("固定串")`）→ 原样返回，不 panic。
#[test]
fn deny_reason_session无占位符原样返回() {
    let fixed = ReasonKind::Session("固定串".to_owned());
    assert_eq!(
        reason::deny_reason(&fixed, "/repo/x.toml", "write"),
        "固定串",
        "无占位符即原样返回"
    );
}

// ===========================================================================
// 本变更：契约资产相位锁串（`contract_lock_template`）—— 第二个会话类模板实例
// ===========================================================================

/// 契约规则载体（`deny_reason` 的显式入参）。
fn contract_reason() -> ReasonKind {
    ReasonKind::Session(reason::contract_lock_template().to_owned())
}

/// 中性入参（断言面只用这两值，模板零架构域路径字面量）。
const CONTRACT_PATH: &str = "/repo/archi/models/App/Auth.schema.json";
const CONTRACT_TOOL: &str = "edit";

/// 正向（AC-7）：以中性路径与工具名展开后四要素逐个在场——① 事实串（相位锁 + 路径原文 +
/// 工具名）；② 政策（接口定义与模块文档 + 只在哪两处写 + 实现相位只实现契约）；③ **三条
/// 出路**；④ 不得绕过。
#[test]
fn 契约锁模板四要素齐备() {
    let text = reason::deny_reason(&contract_reason(), CONTRACT_PATH, CONTRACT_TOOL);

    // ① 事实串
    assert!(text.contains("相位锁"), "事实串点明相位锁：{text}");
    assert!(text.contains(CONTRACT_PATH), "路径原文在场：{text}");
    assert!(text.contains(CONTRACT_TOOL), "工具名替换到位：{text}");

    // ② 政策
    for piece in [
        "接口定义（.schema.json）与模块文档（.md）",
        "proposal / dev-design 相位的执行侧",
        "（以及架构页会话）写入",
        "实现相位只实现契约、不改契约",
    ] {
        assert!(text.contains(piece), "政策句含 `{piece}`：{text}");
    }

    // ③ 三条出路
    for piece in [
        "契约对就改代码追上它",
        "停下来说明冲突，由用户决定走 backtrack 回到 dev-design 修订",
        "或请用户改用架构页会话直接维护",
    ] {
        assert!(text.contains(piece), "出路句含 `{piece}`：{text}");
    }

    // ④ 不得绕过
    assert!(text.contains(NO_BYPASS_SENTENCE), "禁绕过句在场：{text}");
}

/// 边界：`%s` / `%t` **全量**替换——模板内占位计数与展开后的替换次数一致；展开后零残留
/// 占位符；空路径 / 空工具名两边界仍产出完整四要素串（零 panic）。
#[test]
fn 契约锁模板占位全量替换四形态() {
    let template = reason::contract_lock_template();
    let path_slots = template.matches("%s").count();
    let tool_slots = template.matches("%t").count();
    assert_eq!(path_slots, 1, "模板内 `%s` 占位恰一处");
    assert_eq!(tool_slots, 1, "模板内 `%t` 占位恰一处");

    // 路径正文含 `%s` 字面：插入不重扫，故路径原样在场且恰出现一次（替换次数 = 模板占位
    // 数，不被路径内容放大）
    let tricky = "/repo/%s/a.schema.json";
    let text = reason::deny_reason(&contract_reason(), tricky, CONTRACT_TOOL);
    assert_eq!(
        text.matches(tricky).count(),
        1,
        "路径正文原样插入（`%s` 插入不重扫）：{text}"
    );
    assert_eq!(
        text.matches(CONTRACT_TOOL).count(),
        1,
        "`%t` 只替换模板占位一处：{text}"
    );
    assert!(
        text.contains("（经 edit 工具）"),
        "工具名落位形态不变：{text}"
    );

    for (label, path, tool) in [
        ("空路径", "", CONTRACT_TOOL),
        ("空工具名", CONTRACT_PATH, ""),
        ("两空", "", ""),
    ] {
        let text = reason::deny_reason(&contract_reason(), path, tool);
        assert!(
            !text.contains("%s") && !text.contains("%t"),
            "{label}：占位零残留：{text}"
        );
        assert!(
            text.contains("相位锁") && text.contains(NO_BYPASS_SENTENCE),
            "{label}：结构完整（事实串 + 禁绕过句仍在）：{text}"
        );
    }
}

/// 异常：与模型串逐字不同（`assert_ne!`）；契约串零「架构模型 / 元素与关系 / 写进 design.md
/// 或 tasks.md」措辞（复用了就是三重误导），模型串零「.schema.json / 契约」措辞。
#[test]
fn 契约锁模板与模型锁模板逐字不同且措辞不串味() {
    let contract = reason::contract_lock_template();
    let model = reason::session_lock_template();
    assert_ne!(contract, model, "两份模板逐字不同（不得复用一份）");

    for stray in ["架构模型", "元素与关系", "design.md", "tasks.md"] {
        assert!(
            !contract.contains(stray),
            "契约串零模型面措辞 `{stray}`：{contract}"
        );
    }
    for stray in [".schema.json", "契约"] {
        assert!(
            !model.contains(stray),
            "模型串零契约面措辞 `{stray}`：{model}"
        );
    }
}

/// 回归：内置串 / 会话模型串 / 用户带 reason / 用户无 reason 回落四态逐字不变；
/// `contract_lock_template()` 返回值与 `CONTRACT_LOCK_TEMPLATE` 常量为同一 `&'static str`
/// （单一出口，装配层不再抄一份文案）。
#[test]
fn 既有四串零改动且契约模板单点出口() {
    let builtin = reason::deny_reason(&ReasonKind::BuiltinConfig, "/repo/x.json", "write");
    assert!(builtin.contains("配置权威"), "内置串语义不变");
    let session = reason::deny_reason(&session_reason(), "/repo/m.toml", "edit");
    assert!(session.contains("架构模型"), "模型串语义不变");
    let custom = reason::deny_reason(&user_with("自有文案：%s"), "/repo/x.md", "write");
    assert_eq!(custom, "自有文案：/repo/x.md", "用户模板原样透传");
    let fallback = reason::deny_reason(&ReasonKind::User(None), "/repo/x.md", "write");
    assert!(fallback.contains("请调整方案"), "回落串语义不变");

    assert!(
        std::ptr::eq(
            reason::contract_lock_template(),
            reason::contract_lock_template()
        ),
        "同值两次调用返回同一 `&'static str`（单点出口）"
    );
}

/// 回归边界：`ReasonKind::Session(template)` 对两份模板同款处理（模型串与契约串各展开一次，
/// 行为同构）；`substitute` 对两份模板的替换语义一致。
#[test]
fn 两模板经同一条替换链() {
    for template in [
        reason::session_lock_template(),
        reason::contract_lock_template(),
    ] {
        let via_session = reason::deny_reason(
            &ReasonKind::Session(template.to_owned()),
            "/repo/a/b.md",
            "write",
        );
        let via_substitute = reason::substitute(template, "/repo/a/b.md", "write");
        assert_eq!(
            via_session, via_substitute,
            "Session 臂与 substitute 同一条替换链：{template}"
        );
        assert!(
            !via_session.contains("%s") && !via_session.contains("%t"),
            "全量替换零残留：{via_session}"
        );
    }
}

/// 异常（源码锚）：模块头「五类文案形态」计数句仍在（契约串是会话类模板的第二个实例，不
/// 新增 `ReasonKind` 变体）；`contract_lock_template` 函数体只返常量（零格式化、零参数）。
#[test]
fn 契约模板源码锚_五类计数不变且函数体只返常量() {
    let source = include_str!("reason.rs");
    assert!(source.contains("五类文案形态"), "模块头五类计数句仍在");
    assert!(
        source.contains("第二个实例"),
        "契约串被记为会话类的第二个实例（不新增变体）"
    );

    let start = source
        .find("pub(crate) fn contract_lock_template()")
        .expect("模板出口在案");
    let body = source[start..]
        .lines()
        .take(4)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("CONTRACT_LOCK_TEMPLATE"),
        "函数体只返常量：{body}"
    );
    assert!(
        !body.contains("format!") && !body.contains("(&"),
        "零格式化、零参数：{body}"
    );

    // 装配层不再抄一份文案的锚：模板字面量只在本文件出现（compose.rs 取出口函数）
    let compose = include_str!("../../compose.rs");
    assert!(
        !compose.contains("该路径受相位锁保护"),
        "compose.rs 零模板文案副本（只取出口函数）"
    );
}
