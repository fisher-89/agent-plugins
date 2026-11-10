// 生成于 archi-api 0.1.0 · common.json@sha256:1f8f0ec14abb —— 勿手改
// 手改将在下次 generate 时被覆盖，且 api:check 零 diff 门会失败。
// 类型定义与自动构造器由契约派发；impl 块写在本模块任意手写文件。
// 本文件由元素的契约 markdown 生成，勿手改：手改将在下次 generate 时被覆盖，且 api:check 零 diff 门会失败

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalArchiCheck;

impl LocalArchiCheck {
    /// 构造器（自动产出；契约 MUST NOT 声明构造器）
    pub fn new() -> Self {
        Self
    }
}

// 签名见证：契约与实现的编译期绑定（参数数或类型不符即 E0308）
const _: fn(&str) -> String = super::LocalArchiCheck::file_stem;
const _: fn() -> super::LocalArchiCheck = super::LocalArchiCheck::new;
