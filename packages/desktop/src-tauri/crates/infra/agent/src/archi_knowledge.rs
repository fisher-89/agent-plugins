//! 架构会话知识拼装（`preamble` 的生产者，两处消费：`source = "archi"` 会话与
//! `source = "change"` 的写权相位）：静态骨架 md（`include_str!` 编译期内嵌）+ 两处目录
//! 路径拼成单条文本。骨架的「写两件」是**模型**写面（`archi_element` / `archi_relation`）
//! ——archi 工具面共三写工具（两模型写 + 一生成物写）；第三个写工具 `archi_generate` 只写
//! 三类生成物（不写模型、不写契约），在骨架里以「生成物过期调它重生成」一句点名，不占写面
//! 名单。
//!
//! 纪律：
//! 1. **目录路径只从代码派生**：模型声明目录与资产目录（文档 `<叶段>.md` + 接口
//!    `<叶段>.schema.json`）取自 `foundation::layout` 的相对路径派生族，作为唯一程序追加
//!    段——骨架 MUST NOT 手写目录字面量；
//! 2. **尽力增强、不炸 run**：拼装失败（骨架为空 / 含占位符）降级 [`knowledge`]
//!    为 `None`，装配层只丢 `preamble` 位，工具面与 run 不受影响。

/// 静态骨架（三资产分工 / 设计顺序 / 三条不可越界 / 工具面 / 留给判断五段；派生路径段
/// 由 [`build`] 追加）：只留不可
/// 脚本化的散文与工具指路，字段形态 / 键形 / 值域 / 祖先 / 计数全部归工具面
/// （`archi_scaffold` 词典与两模型写工具的参数 `description`）。
const SKELETON: &str = include_str!("archi-knowledge.md");

/// 架构会话知识（唯一入口）：`build(SKELETON).ok()`——`None` = 降级（装配层只丢
/// `preamble` 位）。纯函数零 IO、零 panic 路径。
pub(crate) fn knowledge() -> Option<String> {
    build(SKELETON).ok()
}

/// 骨架 + 目录路径段拼装：骨架 trim 后为空 / 含 `<>` 占位符即 `Err`（触发点写进
/// 文案），否则以 `skeleton.trim_end()` 起手追加两处 `foundation::layout` 派生路径
/// （模型声明目录 / 资产目录及其两种文件：文档 `<叶段>.md`、接口 `<叶段>.schema.json`）。
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
         契约资产：{}/<分层目录…>/<叶段>.md（理由）+ <叶段>.schema.json（接口，手写真相）",
        skeleton.trim_end(),
        foundation::layout::archi_graph_model_rel_dir(),
        foundation::layout::archi_models_rel_dir(),
    ))
}
