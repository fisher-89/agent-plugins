use serde::{Deserialize, Serialize};
use serde_json::Value;
use workflow::model::{checklist_entries, derive_verdict, ChecklistItem, Verdict};

/// report 长度上限（与插件 `phaseLogSchema.report` max 2000 同源，超长在
/// 桌面侧先行拒绝——写面落账前最后一道）。
pub const MAX_REPORT_CHARS: usize = 2000;

/// verdict 封闭结构（收口载荷的 evaluator 臂）：`verdict` 恒由
/// [`derive_verdict`] 从 `checklist` 推导（模型不可提交），`report` 与
/// `checklist` 来自 `finish_step` 参数；phase / attempt / skipped 由 walker 按
/// provenance 盖戳落账（evaluator 不回声，故本类型不含三字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorChecklist {
    pub verdict: Verdict,
    pub report: String,
    pub checklist: Vec<ChecklistItem>,
}

/// `finish_step` 的 evaluator 形态参数校验：形状 → ID 三分类比对 → `id → item`
/// 映射（md 静态清单「检查项」列权威正文）→ report 上限 → verdict 推导。
///
/// 入口形状 `{ report, items: [{ id, pass, evidence }] }` ≠ 载荷形状
/// `{ verdict, report, checklist: [{ item, pass, evidence }] }`——映射在工具边界
/// 完成，落账面因此零变更。模型传入的 `verdict` 字段完全忽略（不读不校验）。
pub fn validate_checklist(args: &Value, prompt: &str) -> Result<EvaluatorChecklist, String> {
    let object = args
        .as_object()
        .ok_or_else(|| "参数形状非法：期望对象 { report, items }，当前非对象".to_owned())?;
    let report = object
        .get("report")
        .and_then(Value::as_str)
        .ok_or_else(|| "字段 report 缺失或非字符串：期望字符串（≤2000 字符）".to_owned())?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| "字段 items 缺失或非数组：期望 [{ id, pass, evidence }]".to_owned())?;
    if items.is_empty() {
        return Err("字段 items 为空数组：期望至少一项（按本 prompt 静态清单逐项）".to_owned());
    }
    let entries = checklist_entries(prompt);
    if entries.is_empty() {
        return Err(
            "本 prompt 无静态清单 ID（期望非空）：`## 静态清单` 段落未解析出任何 `| <ID> |` 行"
                .to_owned(),
        );
    }
    let expected: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();

    // 逐项形态 + ID 收集（未知 ID = 元素形态层：缺失 / 非字符串 / 空串）
    let mut received: Vec<String> = Vec::with_capacity(items.len());
    let mut passing: Vec<(String, bool, String)> = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(record) = item.as_object() else {
            return Err(format!(
                "字段 items[{index}] 非对象：期望 {{ id, pass, evidence }}，当前 {item}"
            ));
        };
        let id = match record.get("id").and_then(Value::as_str).map(str::trim) {
            Some(id) if !id.is_empty() => id.to_owned(),
            _ => {
                return Err(format!(
                    "字段 items[{index}].id 未知：期望本 prompt 静态清单 ID 集 {:?}，当前缺失 / 非字符串 / 空串",
                    expected
                ))
            }
        };
        let pass = record.get("pass").and_then(Value::as_bool).ok_or_else(|| {
            format!(
                "字段 items[{index}].pass 缺失或非布尔：期望 true / false，当前 {}",
                record.get("pass").unwrap_or(&Value::Null)
            )
        })?;
        let evidence = record
            .get("evidence")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("字段 items[{index}].evidence 缺失或非字符串：期望事实依据（file:line）")
            })?;
        received.push(id.clone());
        passing.push((id, pass, evidence.to_owned()));
    }

    // ID 集合比对（三类文案分列：缺项 / 清单外 ID / 未知 ID 已在上段）
    let missing: Vec<&str> = expected
        .iter()
        .copied()
        .filter(|id| !received.iter().any(|got| got == id))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "字段 items[].id 缺项 {:?}：期望本 prompt 静态清单 ID 集 {:?}，实际收到 {:?}",
            missing, expected, received
        ));
    }
    let unexpected: Vec<&str> = received
        .iter()
        .map(String::as_str)
        .filter(|id| !expected.contains(id))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "字段 items[].id 含清单外 ID {:?}：期望本 prompt 静态清单 ID 集 {:?}，实际收到 {:?}",
            unexpected, expected, received
        ));
    }

    let report_chars = report.chars().count();
    if report_chars > MAX_REPORT_CHARS {
        return Err(format!(
            "字段 report 超长：上限 {MAX_REPORT_CHARS} 字符，当前 {report_chars} 字符，超出 {}",
            report_chars - MAX_REPORT_CHARS
        ));
    }

    let checklist: Vec<ChecklistItem> = passing
        .into_iter()
        .map(|(id, pass, evidence)| ChecklistItem {
            item: entries
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.item.clone())
                .unwrap_or_default(),
            pass,
            evidence,
        })
        .collect();
    let verdict = derive_verdict(&checklist);
    Ok(EvaluatorChecklist {
        verdict,
        report: report.to_owned(),
        checklist,
    })
}
