//! 两 DB 只读工具的执行体：`db_tables`（表清单 / 单表字段说明）与 `db_query`
//! （条件取数）。
//!
//! 本模块只做四件事：scope 解析、形状解析、敏感字段守卫与打码、文字预算闸与
//! 文案装配。**过滤 / 排序 / 投影 / 分页语义单源在 store**（[`store::QuerySpec`]
//! 与 `Store::query`）——此处 MUST NOT 出现第二份实现，否则两套语义必然漂移。
//!
//! 打码与预算属 **agent 面政策**（`store` 是纯数据层，DB 查看页同用），故住在
//! 执行体；错误文案即自纠通道（候选表名 / 候选线名 / op 词表 / 字段类型 / 下标
//! 指引全部来自 store 侧校验链，此处只做 scope 与形状两处本地文案）。

use std::sync::Arc;

use serde_json::Value;
use store::{DbDimension, QuerySpec, Store};

use crate::sdk::{mount, tools};

/// 结果体的字节预算（40 KB，低于 [`tools::MAX_RESULT_BYTES`] 的 50 KB 硬墙）：
/// 行数是名义上限，本预算是实际生效值——累加到「加入该条即超预算」时停止。
pub(crate) const RESULT_BUDGET_BYTES: usize = 40 * 1024;

/// 信封预留（envelope 字段与 hint 文案的体积余量，从预算中先扣除）。
pub(crate) const ENVELOPE_RESERVE_BYTES: usize = 1024;

/// 敏感字段的打码值。
pub(crate) const MASKED_VALUE: &str = "***";

/// 敏感字段线名（`AgentProviderRecord.api_key`）：输出恒打码，且不可用于过滤 /
/// 排序（只打码输出仍留前缀探测 oracle：`LIKE 'sk-a%'` 可逐字符重建明文）。
const SENSITIVE_FIELD: &str = "apiKey";

/// 取数落哪一库（缺省 [`Scope::Both`] 只对表清单有意义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// 全局库（user 维度）
    User,
    /// 会话 workspace 库
    Workspace,
    /// 两库（仅表清单形态；缺省值）
    Both,
}

/// 两工具的 per-turn 执行上下文：两把库句柄（皆来自组合根注入的既有实例，
/// MUST NOT 二次 open——redb 单进程单句柄）。
#[derive(Clone)]
pub(crate) struct DbContext {
    global: Arc<Store>,
    workspace: Arc<Store>,
}

impl DbContext {
    /// 上下文构造（泵起播按名字清单含任一 DB 工具名铸造）。
    pub(crate) fn new(global: Arc<Store>, workspace: Arc<Store>) -> Self {
        Self { global, workspace }
    }

    /// 执行体分派：`db_tables` / `db_query` 两臂，未知名 `Err`；结果与错误都过
    /// L1 单结果收口（预算闸使其对本工具永不实际触发）。
    pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let outcome = match name {
            mount::DB_TABLES_TOOL => self.tables(input),
            mount::DB_QUERY_TOOL => self.query(input),
            other => Err(format!("未知 DB 工具: {other}")),
        };
        match outcome {
            Ok(text) => Ok(tools::cap_result(text)),
            Err(error) => Err(tools::cap_result(error)),
        }
    }

    /// `db_tables`：无 `table` → 表清单数组（`dimension` / `table` / `count` /
    /// `summary`）；有 `table` → 该表字段对象（`name` / `type` / `doc`）。两形态
    /// 字段集互不重叠（数组 / 对象本身即形态判别信号）。
    fn tables(&self, input: &Value) -> Result<String, String> {
        let scope = scope_of(input)?;
        let Some(table) = table_of(input)? else {
            let docs = match scope {
                Scope::Both => self.global.describe().and_then(|mut docs| {
                    docs.extend(self.workspace.describe()?);
                    Ok(docs)
                }),
                single => self.store_for(single).describe(),
            }
            .map_err(|error| error.to_string())?;
            let rows: Vec<Value> = docs
                .into_iter()
                .map(|doc| {
                    serde_json::json!({
                        "dimension": dimension_scope(doc.dimension),
                        "table": doc.table,
                        "count": doc.count,
                        "summary": doc.summary,
                    })
                })
                .collect();
            return serde_json::to_string(&rows)
                .map_err(|error| format!("表清单序列化失败: {error}"));
        };
        let dimension = store::dimension_of(table).ok_or_else(|| {
            format!(
                "未知表: {table}（候选: {}）",
                self.table_candidates().unwrap_or_default()
            )
        })?;
        if let Some(selected) = scope.dimension() {
            if selected != dimension {
                return Err(format!(
                    "表 {table} 属 {} 维度，与 scope: \"{}\" 不符",
                    dimension_scope(dimension),
                    dimension_scope(selected)
                ));
            }
        }
        let doc = self
            .store_of(dimension)
            .describe_table(table)
            .map_err(|error| error.to_string())?;
        let fields: Vec<Value> = doc
            .fields
            .into_iter()
            .map(|field| {
                serde_json::json!({ "name": field.name, "type": field.ty, "doc": field.doc })
            })
            .collect();
        let body = serde_json::json!({
            "table": doc.table,
            "dimension": dimension_scope(doc.dimension),
            "summary": doc.summary,
            "fields": fields,
        });
        serde_json::to_string(&body).map_err(|error| format!("字段说明序列化失败: {error}"))
    }

    /// `db_query`：scope 解析 → 形状解析（`scope` 不参与 store 反序列化）→ 敏感
    /// 字段守卫 → store 查询 → 打码 → 预算闸 → 结果成形。
    fn query(&self, input: &Value) -> Result<String, String> {
        let scope = scope_of(input)?;
        let mut payload = input.clone();
        payload
            .as_object_mut()
            .ok_or_else(|| {
                "入参形状不符：期望 JSON 对象 {table, where:[{field,op,value}], order:{field,desc}, select:[…], offset, limit}"
                    .to_owned()
            })?
            .remove("scope");
        let spec: QuerySpec = serde_json::from_value(payload).map_err(|error| {
            format!(
                "入参形状不符（期望 {{table, where:[{{field,op,value}}], order:{{field,desc}}, select:[…], offset, limit}}）: {error}"
            )
        })?;
        guard_sensitive(&spec)?;
        let dimension = scope.dimension().unwrap_or(DbDimension::Workspace);
        if let Some(actual) = store::dimension_of(&spec.table) {
            if actual != dimension {
                return Err(format!(
                    "表 {} 属 {} 维度，请加 scope: \"{}\"",
                    spec.table,
                    dimension_scope(actual),
                    dimension_scope(actual)
                ));
            }
        }
        let page = self
            .store_of(dimension)
            .query(&spec)
            .map_err(|error| error.to_string())?;
        let budget = RESULT_BUDGET_BYTES.saturating_sub(ENVELOPE_RESERVE_BYTES);
        let mut used = 0usize;
        let mut bytes_stopped = false;
        let mut records: Vec<Value> = Vec::new();
        for record in page.records {
            let mut value = record.value;
            mask_api_key(&mut value);
            let entry = serde_json::json!({ "key": record.key, "value": value });
            let length = serde_json::to_string(&entry)
                .map_err(|error| format!("记录序列化失败: {error}"))?
                .len()
                + 1;
            if used + length > budget {
                // 整条原子：单条记录要么整条在结果里、要么不在——MUST NOT 出现被
                // 字节切开的半截 JSON
                bytes_stopped = true;
                break;
            }
            used += length;
            records.push(entry);
        }
        let returned = records.len();
        let truncated_by = if bytes_stopped {
            Some("bytes")
        } else if returned as u64 == u64::from(spec.limit) && page.matched > returned as u64 {
            Some("limit")
        } else {
            None
        };
        let hint = truncated_by.map_or_else(
            || {
                if page.matched == 0 {
                    nested_path_hint(&spec)
                } else {
                    None
                }
            },
            |by| Some(truncation_hint(by, returned, page.matched)),
        );
        let mut body = serde_json::json!({
            "table": spec.table,
            "total": page.total,
            "matched": page.matched,
            "returned": returned,
            "truncated": truncated_by.is_some(),
            "truncatedBy": truncated_by,
            "records": records,
        });
        if let Some(hint) = hint {
            body["hint"] = Value::String(hint);
        }
        let mut text =
            serde_json::to_string(&body).map_err(|error| format!("查询结果序列化失败: {error}"))?;
        // 防御性裁剪（双保险）：envelope 字段与 hint 的体积不可精确预算，成文后
        // 仍可能越 L1 硬墙——自尾部丢记录重序列化，确保 `cap_result` 永不实际触发
        while text.len() > tools::MAX_RESULT_BYTES {
            let returned = {
                let Some(records) = body.get_mut("records").and_then(Value::as_array_mut) else {
                    break;
                };
                if records.pop().is_none() {
                    break;
                }
                records.len()
            };
            if let Some(object) = body.as_object_mut() {
                object.insert("returned".to_owned(), Value::from(returned));
                object.insert("truncated".to_owned(), Value::Bool(true));
                object.insert("truncatedBy".to_owned(), Value::String("bytes".to_owned()));
                object.insert(
                    "hint".to_owned(),
                    Value::String(truncation_hint("bytes", returned, page.matched)),
                );
            }
            text = serde_json::to_string(&body)
                .map_err(|error| format!("查询结果序列化失败: {error}"))?;
        }
        Ok(text)
    }

    /// scope → 库句柄（`Both` 只对表清单形态有意义，落到 workspace 半边）。
    fn store_for(&self, scope: Scope) -> &Arc<Store> {
        match scope {
            Scope::User => &self.global,
            Scope::Workspace | Scope::Both => &self.workspace,
        }
    }

    /// 维度 → 库句柄（下钻派库与 scope 校验共用单点）。
    fn store_of(&self, dimension: DbDimension) -> &Arc<Store> {
        match dimension {
            DbDimension::User => &self.global,
            DbDimension::Workspace => &self.workspace,
        }
    }

    /// 全量候选表名（未知表名文案用；经两库 describe 面拼接，不另立清单）。
    fn table_candidates(&self) -> Result<String, String> {
        let mut names: Vec<String> = Vec::new();
        for store in [&self.global, &self.workspace] {
            for doc in store.describe().map_err(|error| error.to_string())? {
                names.push(doc.table);
            }
        }
        Ok(names.join(" / "))
    }
}

impl Scope {
    /// scope 对应的维度（`Both` = 缺省，未收窄到单一库）。
    fn dimension(self) -> Option<DbDimension> {
        match self {
            Self::User => Some(DbDimension::User),
            Self::Workspace => Some(DbDimension::Workspace),
            Self::Both => None,
        }
    }
}

/// 维度 → `scope` 线值（与 `DbDimension` 的 serde 线值同字面量）。
fn dimension_scope(dimension: DbDimension) -> &'static str {
    match dimension {
        DbDimension::User => "user",
        DbDimension::Workspace => "workspace",
    }
}

/// `scope` 入参解析（三态：`"user"` / `"workspace"` / 缺省 = [`Scope::Both`]）；
/// 其他值 `Err` 列两值（错误文案即自纠通道）。
fn scope_of(input: &Value) -> Result<Scope, String> {
    match input.get("scope") {
        None | Some(Value::Null) => Ok(Scope::Both),
        Some(Value::String(value)) if value == "user" => Ok(Scope::User),
        Some(Value::String(value)) if value == "workspace" => Ok(Scope::Workspace),
        Some(other) => Err(format!(
            "scope 须为 \"user\" 或 \"workspace\"（收到 {other}；缺省读 workspace）"
        )),
    }
}

/// `table` 入参解析（三态：缺省 / `null` = 表清单形态；非空白字符串 = 字段下钻
/// 形态；其余一律 `Err` 携期望形状）。
///
/// 非对象入参、非字符串 `table`、纯空白 `table` **不得**静默退化成清单：清单是
/// 14 行「看起来成功了」的输出，参数写错却拿到它，模型会把「我传错了」读成
/// 「查询成功」，与「错误文案即自纠通道」正好相反。
fn table_of(input: &Value) -> Result<Option<&str>, String> {
    let object = input.as_object().ok_or_else(|| {
        format!(
            "入参形状不符：期望 JSON 对象 {{table, scope}}（收到 {input}；省略参数即表清单，给 table 即字段下钻）"
        )
    })?;
    match object.get("table") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(table)) => {
            let table = table.trim();
            if table.is_empty() {
                return Err("table 不可为空白（省略该键 = 表清单，填表名 = 字段下钻）".to_owned());
            }
            Ok(Some(table))
        }
        Some(other) => Err(format!(
            "table 须为字符串（收到 {other}；省略该键 = 表清单，填表名 = 字段下钻）"
        )),
    }
}

/// 敏感字段不可作比较面（`where` / `order` 引用即 `Err`）：只打码输出仍留前缀
/// 探测通道，且守卫必须与打码同址（否则 oracle 仍在）；`select` 允许（投影不
/// 构成比较通道，值与不查询等价）。
fn guard_sensitive(spec: &QuerySpec) -> Result<(), String> {
    let paths = spec
        .conds
        .iter()
        .map(|cond| cond.field.as_str())
        .chain(spec.order.as_ref().map(|order| order.field.as_str()));
    for path in paths {
        if path.split('.').any(|segment| segment == SENSITIVE_FIELD) {
            return Err(format!(
                "{SENSITIVE_FIELD} 不可用于过滤 / 排序（该字段输出恒为 \"{MASKED_VALUE}\"，比较通道会泄明文）"
            ));
        }
    }
    Ok(())
}

/// 敏感字段递归打码（对象键 == `apiKey` 且值为字符串 → `"***"`；数组逐元素
/// 递归）：覆盖「整条 / 投影 / 嵌套」三形态的单点。
fn mask_api_key(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                if key == SENSITIVE_FIELD && entry.is_string() {
                    *entry = Value::String(MASKED_VALUE.to_owned());
                    continue;
                }
                mask_api_key(entry);
            }
        }
        Value::Array(items) => {
            for item in items {
                mask_api_key(item);
            }
        }
        _ => {}
    }
}

/// 截断提示（两档同文案，如实回报三元口径——「本页 N 行 / 命中 M 行」是模型
/// 判断「要不要翻页 / 收窄」的唯一信号）。
fn truncation_hint(truncated_by: &str, returned: usize, matched: u64) -> String {
    let cause = if truncated_by == "bytes" {
        "字节预算 40 KB 先到"
    } else {
        "到达行数上限"
    };
    format!(
        "已截断（{cause}）：本页返回 {returned} 行 / 命中 {matched} 行。收缩方式：用 select 收窄字段 / 加 where 条件 / 翻 offset（深翻在大表上较慢）"
    )
}

/// 嵌套路径提示（`where` 含点号路径且零命中时）：嵌套段不做静态字段校验，故
/// 「查了但没中」需要一次显式自纠指引（零额外扫描成本的补偿）。
fn nested_path_hint(spec: &QuerySpec) -> Option<String> {
    spec.conds
        .iter()
        .any(|cond| cond.field.contains('.'))
        .then(|| {
            "嵌套路径（x.y）不做静态字段校验：先确认顶层字段名（db_tables 下钻），点号段须逐级命中对象字段"
                .to_owned()
        })
}
