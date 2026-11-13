//! `policy` 的单元测试（AC-2）：tier 准入纯函数的三档 × 四 tier 矩阵逐格断言
//!（`Protocol` / `Read` 三档恒放行，`Write` / `Execute` 仅 Default 档拒绝）+
//! 与改造前三张按名常量表的逐工具等价对拍 + 拒绝流出形态合成形状。
//! 纯决策表无进程边界依赖，不需要 Mock。

use agent::{AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode};

use crate::sdk::mount::{self, ToolTier};
use crate::sdk::policy::allows;

/// 工具名 → 挂载 tier（成员判定单点 = 挂载清单；清单外名无 tier）。
fn tier(name: &str) -> ToolTier {
    let mount = mount::registry()
        .iter()
        .find(|mount| mount.definition.name == name)
        .expect("被测工具名在挂载清单内");
    mount.tier
}

/// 三档单点遍历。
const MODES: [AgentPermissionMode; 3] = [
    AgentPermissionMode::Default,
    AgentPermissionMode::AcceptEdits,
    AgentPermissionMode::BypassPermissions,
];

// ---------------------------------------------------------------------------
// 三档 × 四 tier 矩阵（正向逐格，12 格）
// ---------------------------------------------------------------------------

#[test]
fn 三档乘四tier矩阵逐格() {
    for mode in MODES {
        for (tier, expected) in [
            (ToolTier::Read, true),
            (ToolTier::Protocol, true),
            (ToolTier::Write, mode != AgentPermissionMode::Default),
            (ToolTier::Execute, mode != AgentPermissionMode::Default),
        ] {
            assert_eq!(
                allows(mode, tier),
                expected,
                "{mode:?} 档 × {tier:?} tier 决策漂移"
            );
        }
    }
}

#[test]
fn default档只放只读与协议两档() {
    assert!(allows(AgentPermissionMode::Default, ToolTier::Read));
    assert!(allows(AgentPermissionMode::Default, ToolTier::Protocol));
    assert!(!allows(AgentPermissionMode::Default, ToolTier::Write));
    assert!(!allows(AgentPermissionMode::Default, ToolTier::Execute));
}

/// 纯函数与幂等（边界）：同一 `(mode, tier)` 连续两次调用**同判**（零内部状态、
/// 零 IO、零缓存）；四 tier × 三档全 12 格各两次调用无 panic（覆盖 `ToolTier`
/// 全值域）。
#[test]
fn 同一mode与tier连续两次调用同判且12格全覆盖() {
    let tiers = [
        ToolTier::Read,
        ToolTier::Write,
        ToolTier::Execute,
        ToolTier::Protocol,
    ];
    for mode in MODES {
        for tier in tiers {
            let first = allows(mode, tier);
            let second = allows(mode, tier);
            assert_eq!(
                first, second,
                "{mode:?} 档 × {tier:?} tier 连续两次调用判定不一致（幂等面破裂）"
            );
            let third = allows(mode, tier);
            assert_eq!(first, third, "{mode:?} 档 × {tier:?} tier 第三次调用仍同判");
        }
    }
}

// ---------------------------------------------------------------------------
// 与改造前三张常量表逐格等价（对拍）
// ---------------------------------------------------------------------------

#[test]
fn 十一项挂载逐工具三档等价于原三张按名常量表() {
    for mount in mount::registry() {
        let name = mount.definition.name.as_str();
        let expected = match name {
            // 原 READONLY_TOOLS：三档全放（`archi_api` 属 Read 档，三档恒放）
            "read" | "grep" | "glob" | "ls" | "archi_module" | "archi_search" | "archi_api" => {
                [true, true, true]
            }
            // 原 WRITE_TOOLS（含 create_explore_note）
            "write" | "edit" | "create_explore_note" => [false, true, true],
            // 原 EXECUTE_TOOLS
            "bash" => [false, true, true],
            other => unreachable!("挂载清单封闭集外名 {other}"),
        };
        let actual = [
            allows(AgentPermissionMode::Default, mount.tier),
            allows(AgentPermissionMode::AcceptEdits, mount.tier),
            allows(AgentPermissionMode::BypassPermissions, mount.tier),
        ];
        assert_eq!(
            actual, expected,
            "工具 {name} 三档行为与改造前常量表漂移：期望 {expected:?} 实际 {actual:?}"
        );
    }
}

#[test]
fn 档位归属抽样_read与protocol恒放写与执行需非default() {
    assert_eq!(tier("read"), ToolTier::Read);
    assert_eq!(tier("archi_module"), ToolTier::Read);
    assert_eq!(tier("write"), ToolTier::Write);
    assert_eq!(tier("create_explore_note"), ToolTier::Write);
    assert_eq!(tier("bash"), ToolTier::Execute);
}

#[test]
fn accept_edits与bypass对同一工具首次同判_与default分化() {
    for name in ["write", "edit", "bash", "create_explore_note"] {
        let tier = tier(name);
        assert!(
            !allows(AgentPermissionMode::Default, tier),
            "default 拒 {name}"
        );
        assert!(
            allows(AgentPermissionMode::AcceptEdits, tier),
            "acceptEdits 放 {name}"
        );
        assert!(
            allows(AgentPermissionMode::BypassPermissions, tier),
            "bypassPermissions 放 {name}"
        );
    }
}

// ---------------------------------------------------------------------------
// 清单外工具名（成员判定归属挂载清单，policy 零按名清单）
// ---------------------------------------------------------------------------

#[test]
fn 清单外工具名不在挂载清单内() {
    for unknown in [
        "",
        "Write",
        "rm_rf",
        "web_search",
        "archi_write",
        "READ",
        "create_Explore_Note",
    ] {
        assert!(
            mount::lookup(unknown).is_none(),
            "未知名 {unknown:?} 必须不在挂载清单内（成员校验据此拒绝）"
        );
    }
}

#[test]
fn 挂载清单内十一项名字两两不同() {
    let names: Vec<&str> = mount::registry()
        .iter()
        .map(|mount| mount.definition.name.as_str())
        .collect();
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(names.len(), unique.len(), "挂载清单名字两两不同: {names:?}");
}

// ---------------------------------------------------------------------------
// 常量表退役（AC-2 / 源码自证）：四名零命中 + `fn allows` 签名轴换位
// ---------------------------------------------------------------------------

/// 常量表退役（异常，AC-2 源码自证）：`policy.rs` 代码行内 `READONLY_TOOLS` /
/// `WRITE_TOOLS` / `EXECUTE_TOOLS` / `listed` 四名零命中；`fn allows` 恰 1 处且
/// 签名轴为 `(AgentPermissionMode, ToolTier)`（工具名轴零残留）。
#[test]
fn 常量表退役四名零命中且allows签名轴换位() {
    // 剔除注释行：本文件的对拍基准注释里会出现「原 READONLY_TOOLS」等历史说明，
    // 自证只看产品源的代码行。
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sdk/policy.rs"),
    )
    .expect("读取 policy.rs");
    let code_lines = own
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for retired in ["READONLY_TOOLS", "WRITE_TOOLS", "EXECUTE_TOOLS", "listed"] {
        assert!(
            !code_lines.contains(retired),
            "policy.rs 代码行内 {retired} 零命中（按名常量表已退役）"
        );
    }

    assert_eq!(
        code_lines.matches("fn allows").count(),
        1,
        "fn allows 恰一处"
    );
    assert!(
        code_lines.contains("pub fn allows(mode: AgentPermissionMode, tier: ToolTier) -> bool"),
        "allows 首参 AgentPermissionMode、次参 ToolTier（工具名轴零残留）"
    );
    assert!(
        !code_lines.contains("tool: &str") && !code_lines.contains("tool_name"),
        "签名面零工具名参数（判定轴已换为 tier）"
    );
    // 本变更加载点：`TOOL_NAMES` 不参与任何判定——policy.rs 内零引用
    assert!(
        !code_lines.contains("TOOL_NAMES"),
        "policy.rs 内 TOOL_NAMES 零命中（常量不参与判定）"
    );
}

// ---------------------------------------------------------------------------
// 拒绝流出形态（异常）：SystemNotice{permission_denied} + is_error ToolResult
// ---------------------------------------------------------------------------

#[test]
fn 拒绝决策的合成产物形状为_system_notice_permission_denied_加同id的is_error_tool_result() {
    // loop 侧拒绝合成契约（run 不中断的流出形态）：决策表拒绝的工具（default
    // 档 write）→ SystemNotice{subtype:"permission_denied"} 信封 + 与 tool_use
    // 同 id 的 is_error ToolResult。
    let denied_tool = "write";
    assert!(
        !allows(AgentPermissionMode::Default, tier(denied_tool)),
        "前置：default 档 write 必须被决策表拒绝"
    );

    let tool_use_id = "tu_reject_1";
    let reason = format!("权限档位不允许该工具: {denied_tool}");
    let notice = AgentEventKind::SystemNotice {
        subtype: "permission_denied".to_owned(),
        payload: serde_json::json!({ "tool": denied_tool, "reason": reason }),
    };
    let result = AgentEventKind::Message {
        role: AgentMessageRole::Tool,
        blocks: vec![AgentBlock::ToolResult {
            id: tool_use_id.to_owned(),
            content: reason.clone(),
            is_error: true,
        }],
        parent_tool_use_id: None,
    };

    // 信封形状：subtype 逐字 permission_denied，payload 携带工具名与拒绝原因
    let AgentEventKind::SystemNotice { subtype, payload } = &notice else {
        panic!("合成产物必须是 SystemNotice")
    };
    assert_eq!(subtype.as_str(), "permission_denied");
    assert_eq!(payload["tool"], serde_json::json!("write"));
    assert_eq!(payload["reason"], serde_json::json!(reason));

    // ToolResult 形状：与 tool_use 同 id、is_error=true（run 不中断的成对回灌）
    let AgentEventKind::Message { role, blocks, .. } = &result else {
        panic!("合成产物必须携带 ToolResult 消息")
    };
    assert_eq!(
        *role,
        AgentMessageRole::Tool,
        "工具结果管道以 tool role 密封"
    );
    let Some(AgentBlock::ToolResult {
        id,
        content,
        is_error,
    }) = blocks.first()
    else {
        panic!("blocks 首块必须是 ToolResult")
    };
    assert_eq!(id.as_str(), tool_use_id, "与 tool_use 同 id 成对");
    assert!(*is_error, "拒绝结果必须 is_error");
    assert_eq!(content.as_str(), reason);
}
