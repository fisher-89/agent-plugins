//! ask 工具协议单点：工具名 / description / parameters / 校验入口。纯函数
//! （零 IO、零运行时状态）——infra 只调本模块铸造工具定义并执行校验。
//!
//! 本模块是本工具定义与校验的**唯一**落点（工具名、description、parameters、
//! validate 四者只此一份），与并行的另一份收口工具定义面**互不引用**：两模块
//! 零代码交叉（机械判据见能力 spec 的解耦条款）。

use serde_json::{json, Value};

/// 工具名单点（infra 引它铸造挂载项——`"AskUserQuestion"` 全仓只有这一处
/// 字面量）。
pub const TOOL_NAME: &str = "AskUserQuestion";

/// ask 参数校验产物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskQuestion {
    /// 本问标识：应答 map 的键（`^[A-Za-z][A-Za-z0-9_-]{0,63}$`）
    pub name: String,
    /// 要问用户的问题（单问；多问经多次调用）
    pub question: String,
    /// 候选选项（非空；用户也可自由作答）
    pub options: Vec<String>,
}

/// `name` 的线格式（schema `pattern` 与 [`validate`] 共用同一字面量）：字母
/// 开头，随后字母数字下划线短横，总长 1..=64。前导下划线 / 空白 / 标点一律
/// 拒绝——它是模型可见的键位，须与 JSON key 的书写面一致。
pub const NAME_PATTERN: &str = "^[A-Za-z][A-Za-z0-9_-]{0,63}$";

/// 工具描述（<300 字：何时问 / 单问一次 / name 契约 / 非空选项 / 应答形状与
/// 阻塞）。
pub fn description() -> &'static str {
    "向用户提问并等待应答。需要用户裁决（歧义澄清 / 方案选择 / 取舍授权）且你无法\
     自行判断时使用；一次只问一个问题（多个问题请多次调用）。name 为本问标识（字母\
     开头、字母数字 _ -），options 必须非空——尽力给出可选项，用户也可自由作答。\
     调用后本 turn 阻塞至用户作答（必答），结果形如 \
     {\"status\":\"answered\",\"answers\":{\"<name>\":\"<应答原文>\"}}，你随后在同一轮内继续。"
}

/// 工具参数 schema（静态：无 prompt 依赖，故零参；`options` 至少一项）。
pub fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "pattern": NAME_PATTERN,
                "description": "本问标识（应答以它为键回灌）：字母开头，仅含字母 / 数字 / _ / -，最长 64",
            },
            "question": {
                "type": "string",
                "description": "要问用户的问题（单问；多个问题请多次调用）",
            },
            "options": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string" },
                "description": "候选选项（至少一个；用户也可自由作答）",
            },
        },
        "required": ["name", "question", "options"],
    })
}

/// `name` 是否符合 [`NAME_PATTERN`]（手写逐字符判定：一个字符类不引入正则依赖，
/// 与 schema `pattern` 逐字同义——字母开头，余位字母数字 `_` `-`，1..=64 字符）。
fn name_well_formed(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    let rest = chars.as_str();
    rest.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        && name.chars().count() <= 64
}

/// 参数校验（纯函数）：name 合法 / 问题非空 / options 为非空字符串数组。Err 文案
/// 自足（字段名 + 期望 + 当前值）——工具 Err 是唯一反馈面。
pub fn validate(args: &Value) -> Result<AskQuestion, String> {
    let object = args.as_object().ok_or_else(|| {
        "参数形状非法：期望对象 { name, question, options }，当前非对象".to_owned()
    })?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "字段 name 缺失或非字符串：期望匹配 {NAME_PATTERN}，当前 {}",
                object
                    .get("name")
                    .map_or_else(|| "（缺失）".to_owned(), Value::to_string)
            )
        })?;
    if !name_well_formed(&name) {
        return Err(format!(
            "字段 name 非法：期望匹配 {NAME_PATTERN}，当前 {name:?}"
        ));
    }
    let question = object
        .get("question")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "字段 question 缺失 / 非字符串 / 空白：期望非空字符串，当前 {}",
                object
                    .get("question")
                    .map_or_else(|| "（缺失）".to_owned(), Value::to_string)
            )
        })?;
    let options: Vec<String> = object
        .get("options")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "字段 options 缺失或非数组：期望非空字符串数组，当前 {}",
                object
                    .get("options")
                    .map_or_else(|| "（缺失）".to_owned(), Value::to_string)
            )
        })?
        .iter()
        .map(|option| option.as_str().map(str::to_owned))
        .collect::<Option<Vec<String>>>()
        .ok_or_else(|| "字段 options 含非字符串元素：期望字符串数组".to_owned())?;
    if options.is_empty() {
        return Err("字段 options 为空数组：期望至少一个选项".to_owned());
    }
    Ok(AskQuestion {
        name,
        question,
        options,
    })
}
