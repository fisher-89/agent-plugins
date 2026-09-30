#[derive(Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// 认证凭据（bearer token）
    pub api_key: String,
    /// openai 兼容端点 base_url（含版本段，如 `https://…/v1`）
    pub base_url: String,
    /// 模型标识（管理数据解析取 provider 三档 model 的 high 档）
    pub model: String,
}

impl EngineConfig {
    pub fn empty() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
        }
    }

    /// 三字段齐备检查（runner 启动校验用）：任一为空即配置缺失。
    pub fn is_complete(&self) -> bool {
        !self.api_key.is_empty() && !self.base_url.is_empty() && !self.model.is_empty()
    }
}
