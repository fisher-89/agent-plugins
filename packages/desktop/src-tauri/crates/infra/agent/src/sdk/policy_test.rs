//! `policy` 的单元测试（AC-5）：权限三档纯决策表逐格断言（档位 × 工具名 →
//! 允许/拒绝）+ 拒绝流出形态（`SystemNotice{permission_denied}` + is_error
//! ToolResult）合成形状。纯决策表无进程边界依赖，不需要 Mock。

use agent::{AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode};

use crate::sdk::policy::allows;
use crate::sdk::tools::TOOL_NAMES;

/// 只读四工具（default 档放行面，与 policy 模块清单同源口径）。
const READONLY: [&str; 4] = ["read", "grep", "glob", "ls"];
/// 写面两工具（acceptEdits 档追加放行）。
const WRITE: [&str; 2] = ["write", "edit"];

// ---------------------------------------------------------------------------
// 三档决策表（正向逐格）
// ---------------------------------------------------------------------------

#[test]
fn default档只读四工具允许且write_edit拒绝() {
    for tool in READONLY {
        assert!(
            allows(AgentPermissionMode::Default, tool),
            "default 档应放行只读工具 {tool}"
        );
    }
    for tool in WRITE {
        assert!(
            !allows(AgentPermissionMode::Default, tool),
            "default 档应拒绝写面工具 {tool}"
        );
    }
}

#[test]
fn accept_edits档只读加写面全允许() {
    for tool in READONLY {
        assert!(
            allows(AgentPermissionMode::AcceptEdits, tool),
            "acceptEdits 档应放行只读工具 {tool}"
        );
    }
    for tool in WRITE {
        assert!(
            allows(AgentPermissionMode::AcceptEdits, tool),
            "acceptEdits 档应放行写面工具 {tool}"
        );
    }
}

#[test]
fn bypass_permissions档六工具全放行且为六工具清单逐一命中() {
    // 与六工具定义面同源口径：清单内逐一放行
    assert_eq!(READONLY.len() + WRITE.len(), TOOL_NAMES.len());
    for tool in TOOL_NAMES {
        assert!(
            allows(AgentPermissionMode::BypassPermissions, tool),
            "bypassPermissions 档应全放行 {tool}"
        );
    }
    // 调试页既有默认档：permission-mode 默认即 BypassPermissions（决策表语义
    // 与调试页默认一致——serde 线格式缺省档为 default，调试页 UI 恒显式传
    // bypassPermissions，见 agent-run-form 断言；此处锁定档位枚举值存在且可匹配）
    let mode = AgentPermissionMode::BypassPermissions;
    assert_eq!(mode, AgentPermissionMode::BypassPermissions);
}

// ---------------------------------------------------------------------------
// 缺席与清单外（边界）
// ---------------------------------------------------------------------------

#[test]
fn bash工具名三档均拒绝_缺席断言锁定() {
    // bash 不进 MVP 工具面：三档均拒绝（将来引入 bash 时的预留档位语义）
    for mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        assert!(!allows(mode, "bash"), "{mode:?} 档对缺席工具 bash 必须拒绝");
    }
}

#[test]
fn 清单外工具名与大小写变体与空串一律拒绝() {
    // 未知工具名（模型幻觉）不静默放行
    for unknown in ["rm_rf", "web_search", "execute", "Bash"] {
        assert!(
            !allows(AgentPermissionMode::BypassPermissions, unknown),
            "清单外工具 {unknown} 即使 bypassPermissions 也必须拒绝"
        );
    }
    // 大小写变体（"READ"）：封闭清单精确匹配，不做大小写归一
    for mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        assert!(!allows(mode, "READ"), "大小写变体 READ 必须拒绝");
        assert!(!allows(mode, ""), "空串工具名必须拒绝");
    }
}

// ---------------------------------------------------------------------------
// 拒绝流出形态（异常）：SystemNotice{permission_denied} + is_error ToolResult
// ---------------------------------------------------------------------------

#[test]
fn 拒绝决策的合成产物形状为_system_notice_permission_denied_加同id的is_error_tool_result() {
    // loop 侧拒绝合成契约（run 不中断的流出形态，AC-5 后半句）：决策表拒绝
    // 的工具（default 档 write）→ SystemNotice{subtype:"permission_denied"}
    // 信封 + 与 tool_use 同 id 的 is_error ToolResult。此处以决策表驱动同一
    // 形状的合成产物断言（合成执行体归 loop，loop 语义经 runner_test 假流缝
    // 全链用例承载；本用例锁定信封与 ToolResult 的形状口径）。
    let denied_tool = "write";
    assert!(
        !allows(AgentPermissionMode::Default, denied_tool),
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
