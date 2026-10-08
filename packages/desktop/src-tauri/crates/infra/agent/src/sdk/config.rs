#[derive(Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// 认证凭据（bearer token）
    pub api_key: String,
    /// openai 兼容端点 base_url（含版本段，如 `https://…/v1`）
    pub base_url: String,
    /// 模型标识（管理数据解析取 provider 三档 model 的 high/low 档）
    pub model_high: String,
    pub model_low: String,
}

impl EngineConfig {
    pub fn empty() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
            model_high: String::new(),
            model_low: String::new(),
        }
    }

    /// 四字段齐备检查（runner 启动校验用）
    pub fn is_complete(&self) -> bool {
        !self.api_key.is_empty()
            && !self.base_url.is_empty()
            && !self.model_high.is_empty()
            && !self.model_low.is_empty()
    }
}
