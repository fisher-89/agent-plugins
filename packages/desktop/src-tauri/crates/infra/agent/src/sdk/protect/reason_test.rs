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
    let path = "/tmp/ws/openspec/config.json";
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
        "/ws/openspec/config.json",
        "bash",
    );
    let second = reason::deny_reason(
        &ReasonKind::BuiltinConfig,
        "/ws/openspec/config.json",
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
