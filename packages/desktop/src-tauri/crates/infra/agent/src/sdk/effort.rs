//! 思考强度（effort）的协议 shaping **唯一单点**：两臂 wire 形状与键名常量
//! 都收在本文件——`loop` / `runner` / 门面 / 组合根一律持「已 shaping 的
//! 载荷」，零键名字面量、零协议判定（与 `web_search` 的「已门控最终值」同构）。
//!
//! 留空返回 `None` 而非空对象：这是「留空零痕迹」的类型化表达——空载荷若成
//! 形为 `{}`，非 DeepSeek / 非官方 Anthropic 的兼容网关会被无谓的新增键污染，
//! 而存量 provider（两档留空）必须与变更前逐字等价。
//!
//! 零值域校验：自定义值原样透传（与保存期「只 `trim`」同一条纪律）——两
//! vendor 的 effort 值域不是同一集合，本模块只负责形状，不负责合法性。

use serde_json::{Map, Value};

use crate::sdk::config::Protocol;

/// openai 臂的 wire 键（`{"reasoning_effort": v}`，Chat Completions 顶层）。
pub(crate) const OPENAI_EFFORT_KEY: &str = "reasoning_effort";

/// anthropic 臂的外层 wire 键（`{"output_config": {"effort": v}}`）。
pub(crate) const ANTHROPIC_OUTPUT_CONFIG_KEY: &str = "output_config";

/// anthropic 臂的内层 wire 键（`output_config.effort`）。
pub(crate) const ANTHROPIC_EFFORT_KEY: &str = "effort";

/// 按协议把 effort 原文 shaping 成请求侧旁路载荷：裁剪后为空 ⇒ `None`
/// （留空 = 不发）；`Openai` ⇒ `{"reasoning_effort": v}`；`Anthropic` ⇒
/// `{"output_config": {"effort": v}}`。纯函数，零 IO、零值域判定。
pub(crate) fn params(protocol: Protocol, effort: &str) -> Option<Value> {
    let value = effort.trim();
    if value.is_empty() {
        return None;
    }

    let mut params = Map::new();
    match protocol {
        Protocol::Openai => {
            params.insert(
                OPENAI_EFFORT_KEY.to_owned(),
                Value::String(value.to_owned()),
            );
        }
        Protocol::Anthropic => {
            let mut output_config = Map::new();
            output_config.insert(
                ANTHROPIC_EFFORT_KEY.to_owned(),
                Value::String(value.to_owned()),
            );
            params.insert(
                ANTHROPIC_OUTPUT_CONFIG_KEY.to_owned(),
                Value::Object(output_config),
            );
        }
    }
    Some(Value::Object(params))
}
