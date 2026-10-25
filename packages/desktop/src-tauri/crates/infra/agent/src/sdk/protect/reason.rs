// ---------------------------------------------------------------------------
// 拒绝指令文案（面向 Agent，首要读者是模型；用户为次读者）
//
// 机制面 `loop::deny` **不中断 run**：被拒的模型会自然换路重试（write →
// 重定向 → `cp` → `node -e`）。因此「不要改用其他命令或工具尝试绕过」与
// 「立即停止并通知用户」两句是**功能句**，是掐断换路链的唯一手段，四条文案
// 一律携带（MUST NOT 省略）。
//
// 「中断」语义边界：文案里的「立即停止」指模型结束本轮（正常收敛）；引擎不
// 为此新增机制。本模块只产字符串——零事件流出（notice 载荷与 ToolResult 由
// loop 侧合成，单串双面）。
//
// 文案禁用域目录名 / 配置文件名字面量（命名隔离纪律）：内置规则的身份由
// `ReasonKind` 表达，不靠文案复述文件名。
// ---------------------------------------------------------------------------

use crate::sdk::protect::rules::ReasonKind;

/// 内置工作区配置规则串（五要素：受保护 + 路径 / 配置权威、Agent 不得改写 /
/// 「改哪个字段、改成什么、为什么」由用户自行操作 / 无法规避立即停止并通知
/// 用户 / 不得绕过）。
const BUILTIN_TEMPLATE: &str = "该文件受写入保护：%s（经 %t 工具）。它是本工作区的配置权威，\
Agent 不得改写。若确需调整，请在回复中说明「改哪个字段、改成什么、为什么」，由用户自行操作。\
若任务无法在不改该文件的前提下完成，请立即停止并通知用户。\
不要改用其他命令或工具尝试绕过（所有写入路径均被拦截）。";

/// 用户规则无 `reason` 的通用回落串。
const FALLBACK_TEMPLATE: &str = "该文件受写入保护：%s（经 %t 工具）不可写。\
请调整方案，避免修改该文件；如无法规避，请立即停止并通知用户。\
不要改用其他命令或工具尝试绕过（所有写入路径均被拦截）。";

/// 批量还原 / 清理专用串：`%s` 落中间事实串（含目标路径），`%t` 落工具名。
/// 在规格「事实串 + 不可见 + 逐路径引导」下界之上补两句功能句。
const BULK_TEMPLATE: &str = "该命令被拦截（经 %t 工具）：%s。\
批量还原 / 清理会一次性改写工作区，且对按路径判定的写入保护不可见；\
请改用逐路径 `git restore <path>`（可被记录并折叠为净 untouched）。\
若无法规避，请立即停止并通知用户。不要改用其他命令或工具尝试绕过。";

/// 三态选串：内置 → 内置模板；用户带 reason → 原样透传；用户无 reason →
/// 回落模板。两种用户形态均做 `%s` / `%t` 全量替换。
pub(super) fn deny_reason(kind: &ReasonKind, path: &str, tool: &str) -> String {
    match kind {
        ReasonKind::BuiltinConfig => substitute(BUILTIN_TEMPLATE, path, tool),
        ReasonKind::User(Some(template)) => substitute(template, path, tool),
        ReasonKind::User(None) => substitute(FALLBACK_TEMPLATE, path, tool),
    }
}

/// 占位符替换：`%s` → 路径、`%t` → 工具名，**全量**替换所有出现。
pub(super) fn substitute(template: &str, path: &str, tool: &str) -> String {
    template.replace("%s", path).replace("%t", tool)
}

/// 批量还原专用文案：`detail` 为中间事实串（如「git stash 会批量改写工作
/// 区」），由本函数包成完整指令。
pub(super) fn bulk_revert_reason(detail: &str, tool: &str) -> String {
    substitute(BULK_TEMPLATE, detail, tool)
}
