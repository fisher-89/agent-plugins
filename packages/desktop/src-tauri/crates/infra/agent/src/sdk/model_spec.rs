//! 模型名 `[1m]` 后缀解析纯函数单点（无 I/O、无网络、无 store 依赖）。
//!
//! 保存期（命令层 `save_agent_provider`）与启动期（`SdkRunner::open_session`）
//! 共用本模块——MUST NOT 造第二套规则。解析产物 `ModelSpec.length` 同时驱动
//! 两件事：该档上下文防线窗长候选与 anthropic 1M beta 判定（「后缀在场」的
//! 唯一可考表示）。

/// `1m` 后缀对应的上下文窗长：**1000 进制**（`1m = 1_000_000`），对齐
/// Anthropic 标称与 Claude Code。注意与 [`crate::sdk::context`] 的 128K 缺省
/// 启发式（`128 * 1024`，1024 基）是两处独立口径——本常量不改既有缺省。
pub(crate) const MILLION_CONTEXT_WINDOW: u64 = 1_000_000;

/// anthropic 1M 上下文 beta 头值（`anthropic-beta`），等价 Claude Code 的
/// 1M 开关。
pub(crate) const BETA_CONTEXT_1M: &str = "context-1m-2025-08-07";

/// `[1m]` 后缀字面量（ASCII 大小写不敏感比较）。
const SUFFIX_1M: &[u8] = b"[1m]";

/// 模型名解析产物：裸名（实际发给 provider 的 `model`）+ 窗长候选。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpec {
    /// 裸模型名（后缀已剥离、两端空白已裁剪）
    pub name: String,
    /// 窗长候选：`Some` = 该档窗长由模型名后缀定档（覆盖 `context_length`）；
    /// `None` = 走 provider `context_length` / 128K 缺省启发式
    pub length: Option<u64>,
}

/// 模型名后缀非法（保存期与启动期共用）：只承载模型名原文与原因——结构上
/// 不可能含 api_key（错误串的机密边界由类型保证，非约定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpecError {
    /// 原始模型名（用户填写的原文形态）
    pub raw: String,
    /// 非法原因（中文短语，供拼错误串）
    pub reason: String,
}

impl std::fmt::Display for ModelSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "模型名后缀非法: {}（{}）", self.raw, self.reason)
    }
}

/// 解析模型名原文：末尾 `[1m]`（ASCII 大小写不敏感、两端空白裁剪）→ 裸名 +
/// `1_000_000`；其余方括号形态（`[2m]` / `[8k]` / `[]` / 内空白 / 非末尾）
/// 一律 `Err`；无后缀 → 裸名 + `None`。
///
/// 空串 / 全空白 → `Ok(ModelSpec { name: "", length: None })`：provider 三档
/// 模型名允许留空保存（齐备性由启动期 `ConfigMissing` 兜底），保存期校验
/// MUST NOT 新增拒收面。`"[1m]"`（后缀在场但裸名为空）反而 `Err`——方括号
/// 语义悬空。
pub fn parse_model_spec(raw: &str) -> Result<ModelSpec, ModelSpecError> {
    let trimmed = raw.trim();
    let (stripped, length) = match strip_suffix_1m(trimmed) {
        Some(stripped) => (stripped, Some(MILLION_CONTEXT_WINDOW)),
        None => (trimmed, None),
    };
    let name = stripped.trim();
    if length.is_some() && name.is_empty() {
        return Err(ModelSpecError {
            raw: raw.to_owned(),
            reason: "[1m] 后缀缺模型名".to_owned(),
        });
    }
    // 尾缀剥离后裸名仍残留方括号 = 非末尾方括号 / 重复后缀（`x[1m][1m]`）——
    // 同一规则源一律拒收，不贪婪多剥、不静默降级为裸文发送
    if name.contains('[') || name.contains(']') {
        return Err(ModelSpecError {
            raw: raw.to_owned(),
            reason: "仅支持末尾 [1m] 后缀，其他方括号形态不支持".to_owned(),
        });
    }
    Ok(ModelSpec {
        name: name.to_owned(),
        length,
    })
}

/// 末尾 `[1m]` 剥离（ASCII 大小写不敏感）。`]` 为 ASCII 边界，字节切片不会
/// 切开 UTF-8 多字节序列。
fn strip_suffix_1m(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let at = bytes.len().checked_sub(SUFFIX_1M.len())?;
    if !bytes[at..].eq_ignore_ascii_case(SUFFIX_1M) {
        return None;
    }
    Some(&text[..at])
}
