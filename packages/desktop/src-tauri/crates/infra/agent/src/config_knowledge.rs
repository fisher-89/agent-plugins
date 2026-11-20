//! 配置协议知识拼装（`source = "config"` 会话的 `preamble` 唯一生产者）：静态骨架
//! md（`include_str!` 编译期内嵌）+ 代码派生词表拼成单条文本。
//!
//! 三条纪律：
//! 1. **词表只从代码派生**：顶层已知键取自 [`config::known_keys`]，框架枚举取自
//!    [`config::TestFramework::ALL`]——静态骨架 MUST NOT 手抄一份词表（新增键 / 新增
//!    变体时知识自动跟进）；
//! 2. **骨架自证**：骨架内零 `<>` 占位符（占位符即「词表未派生」的形态，[`build`]
//!    显式拒绝）；
//! 3. **尽力增强、不炸 run**：拼装失败（骨架为空 / 含占位符）降级 [`knowledge`] 为
//!    `None`，装配层只丢 `preamble` 位，工具面与 run 不受影响。
//!
//! 盘面真值**不进**本知识：取数时机由 `propose_config` 只读模式承担（避免
//! 「preamble 拍 T0 / propose 拍 T1 / apply 拍 T2」的双真值时刻）。

/// 静态骨架（写权 / 顶层字段语义 / 值域与阈值 / 路径规则 / 软规则 / 默认值口径 /
/// 提案工具使用指引）。
const SKELETON: &str = include_str!("config-knowledge.md");

/// 配置协议知识（唯一入口）：`build(SKELETON).ok()`——`None` = 降级（装配层只丢
/// `preamble` 位）。纯函数零 IO、零 panic 路径。
pub(crate) fn knowledge() -> Option<String> {
    build(SKELETON).ok()
}

/// 骨架 + 派生段拼装：骨架 trim 后为空 / 含 `<>` 占位符即 `Err`（触发点写进文案），
/// 否则以 `skeleton.trim_end()` 起手追加派生段（顶层已知键八项 / 框架枚举八值）。
pub(crate) fn build(skeleton: &str) -> Result<String, String> {
    if skeleton.trim().is_empty() {
        return Err("知识骨架为空".to_owned());
    }
    if skeleton.contains("<>") {
        return Err("知识骨架含 <> 占位符（词表须由派生注入）".to_owned());
    }
    let keys = join(config::known_keys().iter().copied());
    let frameworks = join(
        config::TestFramework::ALL
            .iter()
            .map(|framework| framework.as_str()),
    );
    Ok(format!(
        "{}\n\n工作区配置顶层已知键：{keys}\ntests[].framework 枚举：{frameworks}",
        skeleton.trim_end(),
    ))
}

/// 词表连接形态单点：已知键与框架枚举同用 `" / "` 分隔。
fn join<'a>(words: impl Iterator<Item = &'a str>) -> String {
    words.collect::<Vec<&str>>().join(" / ")
}
