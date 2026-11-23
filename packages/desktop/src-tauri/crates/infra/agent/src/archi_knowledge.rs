//! 架构会话知识拼装（`source = "archi"` 会话的 `preamble` 唯一生产者）：静态骨架
//! md（`include_str!` 编译期内嵌）+ 两处目录路径拼成单条文本。
//!
//! 纪律：
//! 1. **目录路径只从代码派生**：模型声明目录与契约资产目录取自
//!    `foundation::layout` 的相对路径派生族，作为唯一程序追加段——骨架 MUST NOT
//!    手写目录字面量；
//! 2. **尽力增强、不炸 run**：拼装失败（骨架为空 / 含占位符）降级 [`knowledge`]
//!    为 `None`，装配层只丢 `preamble` 位，工具面与 run 不受影响。

/// 静态骨架（位置与键形 / 字段纪律 / `kind` 语义 / 层级与读数 /
/// 边界与自检 / 写面）：只留不可脚本化的散文与一行工具指路，示例 TOML 与字段形态
/// 归 `archi_scaffold`。
const SKELETON: &str = include_str!("archi-knowledge.md");

/// 架构会话知识（唯一入口）：`build(SKELETON).ok()`——`None` = 降级（装配层只丢
/// `preamble` 位）。纯函数零 IO、零 panic 路径。
pub(crate) fn knowledge() -> Option<String> {
    build(SKELETON).ok()
}

/// 骨架 + 目录路径段拼装：骨架 trim 后为空 / 含 `<>` 占位符即 `Err`（触发点写进
/// 文案），否则以 `skeleton.trim_end()` 起手追加两处 `foundation::layout` 派生路径
/// （模型声明目录 / 契约资产目录）。
pub(crate) fn build(skeleton: &str) -> Result<String, String> {
    if skeleton.trim().is_empty() {
        return Err("知识骨架为空".to_owned());
    }
    if skeleton.contains("<>") {
        return Err("知识骨架含 <> 占位符（路径须由派生注入）".to_owned());
    }
    Ok(format!(
        "{}\n\n\
         模型目录：{}；\
         契约资产：{}/<分层目录…>/<FQN>.md（手写真相）",
        skeleton.trim_end(),
        foundation::layout::archi_graph_model_rel_dir(),
        foundation::layout::archi_models_rel_dir(),
    ))
}
