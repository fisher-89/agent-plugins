//! 架构会话知识拼装（`source = "archi"` 会话的 `preamble` 唯一生产者）：静态骨架
//! md（`include_str!` 编译期内嵌）+ 代码派生词表 / 目录路径拼成单条文本。
//!
//! 三条纪律：
//! 1. **词表只从代码派生**：元素 kind / gate 档 / 结构诊断词三面词表取自
//!    `archi::dto` 的全量常量（`ElementKind::ALL` / `GateTier::ALL` /
//!    `DiagnosticKind::ALL`），目录路径取自 `foundation::layout` 的相对路径派生
//!    族——产品源码与静态骨架 MUST NOT 手抄一份词表或目录字面量；
//! 2. **骨架自证**：骨架内零 `<>` 占位符（占位符即「路径未派生」的形态，[`build`]
//!    显式拒绝），示例 TOML 由测试侧解析面自证可解析；
//! 3. **尽力增强、不炸 run**：拼装失败（骨架为空 / 含占位符）降级 [`knowledge`]
//!    为 `None`，装配层只丢 `preamble` 位，工具面与 run 不受影响。

use archi::dto::{DiagnosticKind, ElementKind, GateTier};

/// 静态骨架（键形 / 字段面 / 层级推导 / 边界 / 自检 / 写面与可见性 / 历史会话
/// 续接 + 一段可解析示例 TOML）。
const SKELETON: &str = include_str!("archi-knowledge.md");

/// 架构会话知识（唯一入口）：`build(SKELETON).ok()`——`None` = 降级（装配层只丢
/// `preamble` 位）。纯函数零 IO、零 panic 路径。
pub(crate) fn knowledge() -> Option<String> {
    build(SKELETON).ok()
}

/// 骨架 + 派生段拼装：骨架 trim 后为空 / 含 `<>` 占位符即 `Err`（触发点写进文案），
/// 否则以 `skeleton.trim_end()` 起手追加派生段（kind 四值 / gate 三值 + 缺省档 /
/// 结构诊断九词 / 模型与决策记录目录相对路径）。
pub(crate) fn build(skeleton: &str) -> Result<String, String> {
    if skeleton.trim().is_empty() {
        return Err("知识骨架为空".to_owned());
    }
    if skeleton.contains("<>") {
        return Err("知识骨架含 <> 占位符（路径须由派生注入）".to_owned());
    }
    let kinds = join(ElementKind::ALL.iter().map(|kind| kind.as_str()));
    let gates = join(GateTier::ALL.iter().map(|gate| gate.as_str()));
    let diagnostics = join(DiagnosticKind::ALL.iter().map(|kind| kind.as_str()));
    Ok(format!(
        "{}\n\n\
         元素 kind：{kinds}\n\
         gate 档：{gates}（新元素缺省 {}）\n\
         结构诊断：{diagnostics}\n\
         模型目录：{}（按文件名字典序聚合）；决策记录目录：{}",
        skeleton.trim_end(),
        GateTier::default().as_str(),
        foundation::layout::archi_models_rel_dir(),
        foundation::layout::archi_decisions_rel_dir(),
    ))
}

/// 词表连接形态单点：`kind` / `gate` / 结构诊断三面同用 `" / "` 分隔。
fn join<'a>(words: impl Iterator<Item = &'a str>) -> String {
    words.collect::<Vec<&str>>().join(" / ")
}
