//! `finish_step` 工具协议单点：工具名 / 两形态 description 与 parameters /
//! 校验入口。纯函数（零 IO、零运行时状态）——infra 只调本模块铸造工具定义并
//! 执行校验，不认识 verdict / 动作语义。
//!
//! evaluator 形态的 `items[].id.enum` 由**本轮 prompt 文本**（`## 静态清单`
//! 段落）解析派生，使「校验的 ID 集 == 模型眼前那份 prompt 的 ID 集」天然自一致
//!（换 prompt 变体、加 ID 自动跟随，零 md 读取依赖）。

use agent::FinishSpec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use workflow::model::checklist_entries;

use crate::decision::{validate_decision, DecisionAction};
use crate::verdict::{validate_checklist, EvaluatorChecklist};

/// 工具名单点（infra 引它铸造挂载项与 notice subtype——`"finish_step"` 全仓只有
/// 这一处字面量）。
pub const TOOL_NAME: &str = "finish_step";

/// 收口载荷（`SystemNotice{finish_step}.payload` 的线形态）：外部 tag，线值为
/// `{"evaluator": {...}}` / `{"decision": {...}}`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinishPayload {
    /// evaluator 步：逐项 checklist（verdict 由 checklist 推导）
    Evaluator(EvaluatorChecklist),
    /// 决策步：两动作封闭集之一
    Decision(DecisionAction),
}

/// 工具描述（按收口档位两形态，均 <500 字）。
pub fn description(spec: FinishSpec) -> &'static str {
    match spec {
        FinishSpec::EvaluatorChecklist => {
            "本步收尾。按下列清单逐项给出结果；report ≤2000 字符。\
             items[].id 取自本 prompt 的静态清单（枚举已给出全集），逐项给 pass 与 evidence\
             （evidence 须为事实依据）。verdict 由清单推导，无需提交；不得调用 phase_log / \
             phase_next / phase_start / backtrack（落账由桌面编排代写）"
        }
        FinishSpec::Decision => {
            "提交本步的路由决策（两动作封闭集之一）：backtrack（to 必须取 prompt 白名单内\
             相位 + reason ≤500 字符）/ stop（reason ≤500 字符）。无法裁决时调用 \
             AskUserQuestion 工具（问题 + 非空选项）。越权目标与超长 reason 会被拒绝"
        }
    }
}

/// 工具参数 schema（evaluator 形态的 `id` enum 由本轮 prompt 静态清单派生；
/// ID 集为空时省略 `enum` 键——空数组是非法 JSON Schema 形态）。
pub fn parameters(spec: FinishSpec, prompt: &str) -> Value {
    match spec {
        FinishSpec::EvaluatorChecklist => {
            let mut id_schema = json!({
                "type": "string",
                "description": "静态清单项 ID（取本 prompt「静态清单」表首列）",
            });
            let ids: Vec<String> = checklist_entries(prompt)
                .into_iter()
                .map(|entry| entry.id)
                .collect();
            if !ids.is_empty() {
                id_schema["enum"] = json!(ids);
            }
            json!({
                "type": "object",
                "properties": {
                    "report": {
                        "type": "string",
                        "description": "评估报告（≤2000 字符）",
                    },
                    "items": {
                        "type": "array",
                        "minItems": 1,
                        "description": "静态清单逐项结果（ID 需覆盖 prompt 清单全集）",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": id_schema,
                                "pass": {
                                    "type": "boolean",
                                    "description": "该项是否通过",
                                },
                                "evidence": {
                                    "type": "string",
                                    "description": "判断依据（事实 / file:line）",
                                },
                            },
                            "required": ["id", "pass", "evidence"],
                        },
                    },
                },
                "required": ["report", "items"],
            })
        }
        FinishSpec::Decision => json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["backtrack", "stop"],
                    "description": "两动作封闭集之一",
                },
                "to": {
                    "type": "string",
                    "description": "backtrack 目标相位（必须取 prompt 白名单内相位）",
                },
                "reason": {
                    "type": "string",
                    "description": "backtrack / stop 的决策理由（≤500 字符）",
                },
            },
            "required": ["action"],
        }),
    }
}

/// 校验入口（纯函数）：按收口档位分派到 checklist / 决策校验器，并包成载荷。
/// Err 文案自足（字段名 + 期望 + 当前值 / 超出量）——工具 Err 是唯一反馈面。
pub fn validate(spec: FinishSpec, args: &Value, prompt: &str) -> Result<FinishPayload, String> {
    match spec {
        FinishSpec::EvaluatorChecklist => {
            validate_checklist(args, prompt).map(FinishPayload::Evaluator)
        }
        FinishSpec::Decision => validate_decision(args).map(FinishPayload::Decision),
    }
}
