//! 只读查询原语：过滤 → 排序 → 投影 → 分页（`db_query` 的语义**单源**）。
//!
//! 三条纪律：
//! - **只读**：全部走读事务与纯函数加工，MUST NOT 出现写事务或第二份 JSON 读取
//!   实现绕过注册表。
//! - **不复用查看页分页上限**：`limit` 上限是 agent 表达力（[`MAX_QUERY_LIMIT`]），
//!   查看页单页上限管 IPC 载重——两者口径独立，本模块 MUST NOT 引用后者。
//! - **精确计数**：`total`（表全量）/ `matched`（过 `where` 后的行数）恒精确，
//!   `total` / `matched` / `records` 取同一读事务快照（走注册表 `walk`，逐条回调
//!   而非整表物化，内存上界 = 保留窗口）。
//!
//! 错误面全部是**自纠文本**（未知字段 → 候选线名 / 未知 op → 词表 / 类型不符 →
//! 字段类型 / 下标路径 → 改正指引）：静默零命中比报错危险一个量级。

use std::cmp::Ordering;
use std::fmt;

use native_db::Database;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::describe::{self, FieldIndex, FieldKind};
use crate::envelope::{self, ModelEntry, RecordEnvelope};
use crate::store::DbDimension;

/// `op` 封闭词表（工具描述拼接与 [`Op::parse`] 的同一份清单——MUST NOT 复制）。
pub const QUERY_OPS: [&str; 10] = [
    "=",
    "!=",
    ">",
    ">=",
    "<",
    "<=",
    "LIKE",
    "IN",
    "IS NULL",
    "IS NOT NULL",
];

/// `limit` 缺省值（最坏表 `session_event` 单行约 1.8 KB，20 行落在 40 KB 预算内）。
pub const DEFAULT_QUERY_LIMIT: u32 = 20;

/// `limit` 上限（agent 表达力上限，**不是**查看页分页口径）。
pub const MAX_QUERY_LIMIT: u32 = 1000;

/// 一次查询的声明（工具入参形状；`scope` 由执行体取出，不参与本结构）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuerySpec {
    /// 表名（记录信封注册表 14 张之一）
    pub table: String,
    /// 条件数组（多项 **AND**；缺省 / `null` / `[]` 三者等价 = 全量）
    #[serde(rename = "where", default, deserialize_with = "conds_or_null")]
    pub conds: Vec<QueryCond>,
    /// 排序（缺省 = 主键自然序）
    #[serde(default)]
    pub order: Option<QueryOrder>,
    /// 投影路径集（`key` 恒在、不占名额）
    #[serde(default)]
    pub select: Option<Vec<String>>,
    /// 深翻偏移（缺省 0）
    #[serde(default)]
    pub offset: u32,
    /// 本页行数上限（缺省 20、上限 1000，越界显式 `Err`）
    #[serde(default = "default_limit")]
    pub limit: u32,
}

/// 单条条件（`field` 为落库线名，支持点号下钻）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryCond {
    /// 字段路径（顶层段静态校验；点号下钻不静态深校验）
    pub field: String,
    /// 比较符（封闭词表见 [`QUERY_OPS`]）
    pub op: String,
    /// 比较值（`IS NULL` / `IS NOT NULL` 不接；`IN` 须为数组）
    #[serde(default)]
    pub value: Option<Value>,
}

/// 排序声明。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryOrder {
    /// 排序字段路径
    pub field: String,
    /// 降序（缺省升序）；`null` 恒排末位
    #[serde(default)]
    pub desc: bool,
}

/// 查询结果页（`total` / `matched` / `records` 同一读事务快照）。
#[derive(Debug, Clone, PartialEq)]
pub struct QueryPage {
    /// 该表全量计数
    pub total: u64,
    /// 过 `where` 后的行数（精确）
    pub matched: u64,
    /// 本页记录（`key` 恒在；`value` 经投影）
    pub records: Vec<RecordEnvelope>,
}

/// 查询面错误：`Invalid` = 模型可自纠的入参问题（未知表 / 字段 / op / 类型不符 /
/// 越界），`Db` = 库层失败。与 [`crate::StoreError`] 分立——`StoreError` 的四个
/// 变体是 change 域故障分类面，查询失败塞进去只会给模型看到误导性前缀。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// 入参校验失败（文案即自纠通道）
    Invalid(String),
    /// 库读写失败
    Db(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid: {message}"),
            Self::Db(message) => write!(f, "db: {message}"),
        }
    }
}

impl std::error::Error for QueryError {}

/// 库层错误 → 查询错误面（错误串携带操作语境）。
pub(crate) fn db_err<E: fmt::Display>(context: &str) -> impl Fn(E) -> QueryError + '_ {
    move |error| QueryError::Db(format!("{context}: {error}"))
}

/// `limit` 的 serde 缺省值。
fn default_limit() -> u32 {
    DEFAULT_QUERY_LIMIT
}

/// `where` 的显式 `null` ≡ 缺省（三类可选入参同一口径：`order` / `select` 是
/// `Option`，`null` 天然落 `None`；`where` 用 `Vec` + 本函数补齐，否则同一个
/// 「没给」在不同键上会分叉成「全量」与「形状错误」两种结果）。
fn conds_or_null<'de, D>(deserializer: D) -> Result<Vec<QueryCond>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Vec<QueryCond>>::deserialize(deserializer)?.unwrap_or_default())
}

/// 比较符（封闭集；解析失败即列全词表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Like,
    In,
    IsNull,
    IsNotNull,
}

impl Op {
    /// 词表单点解析（大小写敏感——`LIKE` 只认大写，与描述里的词表逐字一致）。
    fn parse(op: &str) -> Result<Self, QueryError> {
        Ok(match op {
            "=" => Self::Eq,
            "!=" => Self::Ne,
            ">" => Self::Gt,
            ">=" => Self::Gte,
            "<" => Self::Lt,
            "<=" => Self::Lte,
            "LIKE" => Self::Like,
            "IN" => Self::In,
            "IS NULL" => Self::IsNull,
            "IS NOT NULL" => Self::IsNotNull,
            other => {
                return Err(QueryError::Invalid(format!(
                    "未知 op: {other}（词表: {}）",
                    QUERY_OPS.join(" / ")
                )));
            }
        })
    }
}

/// 校验后的条件（路径 + 比较符 + 比较值）。
struct Cond {
    path: String,
    op: Op,
    value: Value,
}

/// 有序窗口一行（排序键 + 记录信封两半）。
struct Row {
    order: Value,
    key: Value,
    value: Value,
}

/// 查询执行体（DD4 双路径）：无 `where` 且无 `order` 走注册表既有惰性分页扫描
/// （深翻成本 O(offset)）；其余全表 `walk`——`total` / `matched` 边扫边精确计数，
/// 保留窗恒 ≤ `offset + limit` 行（带 `order` 时是有序窗，否则是自然序窗）。
pub(crate) fn run(
    db: &Database<'static>,
    dimension: DbDimension,
    spec: &QuerySpec,
) -> Result<QueryPage, QueryError> {
    let limit = check_limit(spec.limit)?;
    let entry = resolve_entry(&spec.table, dimension)?;
    let index = describe::field_index(&spec.table)?;
    let conds = validate_conds(&index, &spec.conds)?;
    let order = validate_order(&index, spec.order.as_ref())?;
    let select = validate_select(&index, spec.select.as_deref())?;
    if conds.is_empty() && order.is_none() {
        let page =
            (entry.scan)(db, spec.offset, limit, entry.key_of).map_err(describe::store_err)?;
        let total = page.total;
        let records = page
            .records
            .into_iter()
            .map(|record| RecordEnvelope {
                key: record.key,
                value: project(&record.value, select.as_deref()),
            })
            .collect();
        return Ok(QueryPage {
            total,
            matched: total,
            records,
        });
    }
    // 窗口在扫掠中即维持目标次序（自然序顺序收集 / 排序序有序插入），切页只在
    // 此处做一次——扫掠已按 offset 语义留窗，MUST NOT 二次跳过 offset
    let scanned = scan(db, entry, &conds, order.as_ref(), spec.offset, limit)?;
    let records = scanned
        .rows
        .into_iter()
        .skip(spec.offset as usize)
        .take(limit as usize)
        .map(|row| RecordEnvelope {
            key: row.key,
            value: project(&row.value, select.as_deref()),
        })
        .collect();
    Ok(QueryPage {
        total: scanned.all,
        matched: scanned.matched,
        records,
    })
}

/// 扫掠产物：表全量计数 + 过 `where` 的行数 + 保留窗口（已按目标次序）。
struct Scan {
    all: u64,
    matched: u64,
    rows: Vec<Row>,
}

/// 全表扫掠（**一个读事务**内完成）：`total` / `matched` 精确计数，窗口按
/// `order` 是否在场分别以「有序保留窗」与「自然序保留窗」收集——两者容量同为
/// `offset + limit`、次序即目标次序，切页（`skip(offset).take(limit)`）由
/// [`run`] 单处完成（MUST NOT 在扫掠里再跳一次 offset）。
fn scan(
    db: &Database<'static>,
    entry: &'static ModelEntry,
    conds: &[Cond],
    order: Option<&QueryOrder>,
    offset: u32,
    limit: u32,
) -> Result<Scan, QueryError> {
    let capacity = offset as usize + limit as usize;
    let mut matched = 0u64;
    let mut rows: Vec<Row> = Vec::new();
    let all = (entry.walk)(db, entry.key_of, &mut |key, value| {
        if !conds_match(value, conds)? {
            return Ok(true);
        }
        matched += 1;
        match order {
            Some(order) => {
                let row = Row {
                    order: resolve_path(value, &order.field)?
                        .cloned()
                        .unwrap_or(Value::Null),
                    key: key.clone(),
                    value: value.clone(),
                };
                insert_row(&mut rows, row, order.desc, capacity);
            }
            None => {
                if rows.len() < capacity {
                    rows.push(Row {
                        order: Value::Null,
                        key: key.clone(),
                        value: value.clone(),
                    });
                }
            }
        }
        Ok(true)
    })?;
    Ok(Scan { all, matched, rows })
}

/// 有序插入（窗口容量 = `offset + limit`，超出即剔尾部）：维持目标次序的同时
/// 把内存上界锁在保留窗（同值后到者插在等值段之后——自然序稳定）。
fn insert_row(rows: &mut Vec<Row>, row: Row, desc: bool, capacity: usize) {
    let position = rows
        .partition_point(|other| order_cmp(&other.order, &row.order, desc) != Ordering::Greater);
    rows.insert(position, row);
    if rows.len() > capacity {
        rows.pop();
    }
}

/// 目标次序比较：先按 `is_null` 分组（`null` 恒末位，**与方向无关**——降序反转
/// 的只是非 null 段），非 null 段按 `desc` 反转 [`compare_values`] 的方向。
/// `null` 若参与秩比较，降序时会被整体反转翻到首位，故分组必须独立于秩。
fn order_cmp(left: &Value, right: &Value, desc: bool) -> Ordering {
    match (left.is_null(), right.is_null()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            let ordering = compare_values(left, right);
            if desc {
                ordering.reverse()
            } else {
                ordering
            }
        }
    }
}

/// `limit` 合法性（只钉上限：越界显式 `Err`，MUST NOT 静默钳位——钳位会让模型
/// 把「被压到上限」误读成「库里只有这么多」）。`0` 是合法请求（空页但三元照常
/// 精确），两条执行路径都能承载：`scan` 的 `take(0)` 留空、`walk` 的保留窗
/// 容量取 `offset + 0` 后由 `skip(offset).take(0)` 切空。
fn check_limit(limit: u32) -> Result<u32, QueryError> {
    if limit > MAX_QUERY_LIMIT {
        return Err(QueryError::Invalid(format!(
            "limit 上限 {MAX_QUERY_LIMIT}（收到 {limit}；越界不做静默钳位）"
        )));
    }
    Ok(limit)
}

/// 表名 → 注册表行（维度不符与未知表分别给候选：前者列本库候选，后者列全量）。
fn resolve_entry(table: &str, dimension: DbDimension) -> Result<&'static ModelEntry, QueryError> {
    match envelope::entry(table) {
        Some(entry) if entry.dimension == dimension => Ok(entry),
        Some(entry) => Err(QueryError::Invalid(format!(
            "表 {table} 属 {} 维度，不在本库（本库候选: {}）",
            describe::dimension_name(entry.dimension),
            describe::candidates(dimension)
        ))),
        None => Err(QueryError::Invalid(format!(
            "未知表: {table}（候选: {}）",
            describe::all_candidates()
        ))),
    }
}

/// 条件校验链（顺序即错误优先级：字段 → op → 值形状与类型）。
fn validate_conds(index: &FieldIndex, conds: &[QueryCond]) -> Result<Vec<Cond>, QueryError> {
    let mut validated = Vec::with_capacity(conds.len());
    for cond in conds {
        check_path(index, &cond.field)?;
        let op = Op::parse(&cond.op)?;
        match op {
            Op::IsNull | Op::IsNotNull => {
                if let Some(value) = &cond.value {
                    if !value.is_null() {
                        return Err(QueryError::Invalid(format!(
                            "op {} 不接 value（字段 {} 收到 {value}；判空用 IS NULL / IS NOT NULL）",
                            cond.op, cond.field
                        )));
                    }
                }
            }
            Op::In => {
                let items = cond
                    .value
                    .as_ref()
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        QueryError::Invalid(format!(
                            "op IN 的 value 必须是数组（字段 {} 收到 {}）",
                            cond.field,
                            cond.value
                                .as_ref()
                                .map_or_else(|| "缺省".to_owned(), Value::to_string)
                        ))
                    })?;
                for item in items {
                    check_scalar(index, &cond.field, item)?;
                }
            }
            _ => {
                if let Some(value) = &cond.value {
                    check_scalar(index, &cond.field, value)?;
                }
            }
        }
        validated.push(Cond {
            path: cond.field.clone(),
            op,
            value: cond.value.clone().unwrap_or(Value::Null),
        });
    }
    Ok(validated)
}

/// 排序字段校验（路径同条件字段口径）。
fn validate_order(
    index: &FieldIndex,
    order: Option<&QueryOrder>,
) -> Result<Option<QueryOrder>, QueryError> {
    match order {
        Some(order) => {
            check_path(index, &order.field)?;
            Ok(Some(order.clone()))
        }
        None => Ok(None),
    }
}

/// 投影路径校验（形状错误由 serde 的 `deny_unknown_fields` 与类型面拦下；此处
/// 只校验路径本身）。`None` = 不投影（全字段）。
fn validate_select(
    index: &FieldIndex,
    select: Option<&[String]>,
) -> Result<Option<Vec<String>>, QueryError> {
    let Some(paths) = select else {
        return Ok(None);
    };
    for path in paths {
        check_path(index, path)?;
    }
    Ok(Some(paths.to_vec()))
}

/// 字段路径校验：**顶层段静态校验**（必须命中落库线名，否则 `Err` + 候选线名
/// ——静默零命中的唯一防线）；第二段起不静态深校验（嵌套值形态多样）；任一段
/// 为纯数字即数组下标，显式拒绝。
fn check_path(index: &FieldIndex, path: &str) -> Result<(), QueryError> {
    let segments: Vec<&str> = path.split('.').collect();
    for segment in &segments {
        if is_index_segment(segment) {
            return Err(QueryError::Invalid(format!(
                "字段路径 {path} 含数组下标 {segment}：不支持下标，改看整条记录或用点号取对象字段"
            )));
        }
    }
    let names = index.names();
    if !names.contains(&segments[0]) {
        return Err(QueryError::Invalid(format!(
            "未知字段: {}（候选线名: {}）",
            segments[0],
            names.join(" / ")
        )));
    }
    Ok(())
}

/// 纯数字段（= 数组下标记法）。
fn is_index_segment(segment: &str) -> bool {
    !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit())
}

/// 值类型与字段类型一致性（静默 false 会让模型误以为查无数据；`null` 是「无
/// 值」哨兵，不参与类型校验——`= null` / `!= null` 由 null 比较语义处置）。
fn check_scalar(index: &FieldIndex, path: &str, value: &Value) -> Result<(), QueryError> {
    if value.is_null() {
        return Ok(());
    }
    let matched = match index.kind_of(path) {
        FieldKind::Number => value.is_number(),
        FieldKind::Text => value.is_string(),
        FieldKind::Bool => value.is_boolean(),
        FieldKind::Object | FieldKind::Unknown => true,
    };
    if matched {
        return Ok(());
    }
    Err(QueryError::Invalid(format!(
        "字段 {path} 的类型是 {}，与值 {value} 不符",
        index.render_of(path).unwrap_or("unknown")
    )))
}

/// 点号下钻取值：路径缺失（含中间段非对象）→ `None`（等价 `null`，`IS NULL`
/// 可命中）；任一段为纯数字 → `Err`（不支持数组下标）。
fn resolve_path<'a>(value: &'a Value, path: &str) -> Result<Option<&'a Value>, QueryError> {
    let mut current = value;
    for segment in path.split('.') {
        if is_index_segment(segment) {
            return Err(QueryError::Invalid(format!(
                "字段路径 {path} 含数组下标 {segment}：不支持下标，改看整条记录或用点号取对象字段"
            )));
        }
        match current.get(segment) {
            Some(next) => current = next,
            None => return Ok(None),
        }
    }
    Ok(Some(current))
}

/// 条件全真判定（AND）。
fn conds_match(value: &Value, conds: &[Cond]) -> Result<bool, QueryError> {
    for cond in conds {
        if !cond_match(value, cond)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 单条条件判定（`null` 语义：`=` false / `!=` true / 大小比较不匹配 /
/// `LIKE` 不匹配）。
fn cond_match(record: &Value, cond: &Cond) -> Result<bool, QueryError> {
    let field = resolve_path(record, &cond.path)?;
    Ok(match cond.op {
        Op::IsNull => field.is_none_or(Value::is_null),
        Op::IsNotNull => field.is_some_and(|value| !value.is_null()),
        Op::Eq => field.is_some_and(|value| values_equal(value, &cond.value)),
        Op::Ne => field.is_none_or(|value| !values_equal(value, &cond.value)),
        Op::Like => field.is_some_and(|value| match (value.as_str(), cond.value.as_str()) {
            (Some(text), Some(pattern)) => like_match(text, pattern),
            _ => false,
        }),
        Op::In => field.is_some_and(|value| {
            cond.value
                .as_array()
                .is_some_and(|items| items.iter().any(|item| values_equal(value, item)))
        }),
        Op::Gt | Op::Gte | Op::Lt | Op::Lte => {
            field.is_some_and(|value| ordered_match(value, &cond.value, cond.op))
        }
    })
}

/// 等值判定：任一侧 `null` → false（`null` 与任何值都不相等，`!=` 由其取反
/// 得到 true）。
fn values_equal(left: &Value, right: &Value) -> bool {
    if left.is_null() || right.is_null() {
        return false;
    }
    left == right
}

/// 大小比较判定：任一侧 `null` 不匹配；其余按 [`compare_values`] 的全序。
fn ordered_match(left: &Value, right: &Value, op: Op) -> bool {
    if left.is_null() || right.is_null() {
        return false;
    }
    let ordering = compare_values(left, right);
    match op {
        Op::Gt => ordering == Ordering::Greater,
        Op::Gte => ordering != Ordering::Less,
        Op::Lt => ordering == Ordering::Less,
        Op::Lte => ordering != Ordering::Greater,
        _ => false,
    }
}

/// 全序比较（排序的非 null 段与大小比较共用）：跨类型按
/// `number < string < bool < object < array < null`；同类型内按数值 / 码点 /
/// false < true，对象与数组保持稳定序（等值）。`null` 的落位由 [`order_cmp`]
/// 先分组决定（恒末位、不随方向反转），本函数的秩只是兜底的全序构成。
fn compare_values(left: &Value, right: &Value) -> Ordering {
    let (left_rank, right_rank) = (type_rank(left), type_rank(right));
    if left_rank != right_rank {
        return left_rank.cmp(&right_rank);
    }
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left
            .as_f64()
            .partial_cmp(&right.as_f64())
            .unwrap_or(Ordering::Equal),
        (Value::String(left), Value::String(right)) => left.cmp(right),
        (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
        _ => Ordering::Equal,
    }
}

/// 类型序（`null` 秩最大——排序的末位由 [`order_cmp`] 分组保证，与方向无关；
/// 对象 / 数组不可比较故同秩内视为相等）。
fn type_rank(value: &Value) -> u8 {
    match value {
        Value::Number(_) => 0,
        Value::String(_) => 1,
        Value::Bool(_) => 2,
        Value::Object(_) => 3,
        Value::Array(_) => 4,
        Value::Null => 5,
    }
}

/// `LIKE` 匹配：`%` 匹配任意长度（含空串），`\%` / `\_` 为字面 `%` / `_`，其余
/// 字符逐字比较（区分大小写）；无 `%` 时天然退化为等值（与 SQL 同）。`_` 单
/// 字符通配、`ESCAPE` 子句、正则 MUST NOT 支持（标识符类数据里 `_` 通配极易
/// 误伤）。
fn like_match(text: &str, pattern: &str) -> bool {
    let tokens = like_tokens(pattern);
    let haystack: Vec<char> = text.chars().collect();
    let (mut haystack_at, mut token_at) = (0usize, 0usize);
    let mut backtrack: Option<(usize, usize)> = None;
    while haystack_at < haystack.len() {
        if token_at < tokens.len() && tokens[token_at] == LikeToken::Char(haystack[haystack_at]) {
            haystack_at += 1;
            token_at += 1;
        } else if token_at < tokens.len() && tokens[token_at] == LikeToken::Any {
            backtrack = Some((token_at, haystack_at));
            token_at += 1;
        } else if let Some((star_at, star_haystack)) = backtrack {
            token_at = star_at + 1;
            haystack_at = star_haystack + 1;
            backtrack = Some((star_at, star_haystack + 1));
        } else {
            return false;
        }
    }
    while token_at < tokens.len() && tokens[token_at] == LikeToken::Any {
        token_at += 1;
    }
    token_at == tokens.len()
}

/// `LIKE` 模式词元。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LikeToken {
    /// `%`：任意长度（含空）
    Any,
    /// 字面字符（含转义后的 `%` / `_` / `\`）
    Char(char),
}

/// 模式 → 词元（`\` 只对 `%` / `_` / `\` 生效；其余位置的反斜杠按字面保留）。
fn like_tokens(pattern: &str) -> Vec<LikeToken> {
    let mut tokens = Vec::new();
    let mut chars = pattern.chars();
    while let Some(char) = chars.next() {
        match char {
            '%' => tokens.push(LikeToken::Any),
            '\\' => match chars.clone().next() {
                Some(next @ ('%' | '_' | '\\')) => {
                    tokens.push(LikeToken::Char(next));
                    chars.next();
                }
                _ => tokens.push(LikeToken::Char('\\')),
            },
            other => tokens.push(LikeToken::Char(other)),
        }
    }
    tokens
}

/// 投影：按路径重建嵌套对象（未命中路径跳过；`None` = 全字段不投影）。
fn project(value: &Value, select: Option<&[String]>) -> Value {
    let Some(paths) = select else {
        return value.clone();
    };
    let mut out = Map::new();
    for path in paths {
        let segments: Vec<&str> = path.split('.').collect();
        let mut source = value;
        let mut found = true;
        for segment in &segments {
            match source.get(*segment) {
                Some(next) => source = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found {
            insert_path(&mut out, &segments, source.clone());
        }
    }
    Value::Object(out)
}

/// 路径写回（中间段缺失即补对象；同名路径先到者细、后到者覆盖为整值）。
fn insert_path(out: &mut Map<String, Value>, segments: &[&str], value: Value) {
    let head = segments[0];
    if segments.len() == 1 {
        out.insert(head.to_owned(), value);
        return;
    }
    let slot = out
        .entry(head.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !slot.is_object() {
        *slot = Value::Object(Map::new());
    }
    if let Value::Object(map) = slot {
        insert_path(map, &segments[1..], value);
    }
}
