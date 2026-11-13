//! 应生成物的渲染单点：元素表 → 装载契约 → 装配四类生成物。
//!
//! **纯读盘、零写盘、零 spawn**：CLI（`archi-api-gen`）、两条 IPC（检查 / 生成）与
//! agent 工具（`archi_api`）共用本函数，MUST NOT 出现第二份 `collect`。
//!
//! fail-closed 来自 [`super::elements_of`]：模型含任一结构诊断即 `ApiError::Element`
//! ⇒ 坏模型不会被固化成入库生成物（与 [`super::bootstrap`] 同口径）。

use std::path::Path;

use super::{elements_of, load_contracts, render_all, ApiError, GeneratedFile};

/// 观察根下的应生成集（契约 schema → `.i.rs` / `.i.ts` → `INDEX.toml` 末位）。
pub fn workspace_render(root: &Path) -> Result<Vec<GeneratedFile>, ApiError> {
    let elements = elements_of(root)?;
    let snapshot = load_contracts(root, &elements)?;
    render_all(root, &snapshot)
}
