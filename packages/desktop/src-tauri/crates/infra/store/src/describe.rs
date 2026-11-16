//! 表 / 字段说明的机械派生：`specta` 运行时类型图 → 人读文本。
//!
//! 为什么说明只能来自 doc 注释：任何手写文案表都是第二份真相，字段一改必漂移；
//! `///` 已经是给人看的散文（`bindings.ts` 的 JSDoc 即同类产物），derive 时被
//! 搬进 `NamedDataType.docs` / `Field.docs`，故「表介绍 + 字段说明 + 类型 + 落库
//! 线名」四件全链派生、零手写。
//!
//! 为什么必须应用 [`specta_serde::Format`]：`serde(rename_all = "camelCase")` 在
//! 类型图里只是运行期属性，不应用即为源码名（`change_id`），模型据此查询恒零
//! 命中——「静默零命中」比报错危险一个量级，故线名与查询校验 MUST 同源（同一
//! 派生单点，见 [`render_type`] / [`field_kind`] 的同一棵 match）。
//!
//! 类型图按 [`crate::envelope`] 注册表过滤：注册外命名类型（内嵌 struct 如
//! `AgentModelTiers`）MUST NOT 出现在表清单里。

use specta::datatype::{
    DataType, Field, Fields, NamedDataType, NamedReference, NamedReferenceType, Primitive,
    Reference, Struct,
};
use specta::{Format, Type, Types};

use crate::envelope::{entries, entry};
use crate::query::QueryError;
use crate::store::{DbDimension, StoreError};

/// 表说明的兜底字数上限（无句读或首句过长时按此截断）。
const MAX_DOC_CHARS: usize = 120;

/// 表清单一行（`db_tables` 无参形态的元素）：表名 + 维度 + 记录计数 + 表介绍。
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDoc {
    /// 表名（`db_query` 的 `table` 入参取值）
    pub table: String,
    /// 所属数据维度（决定 `scope` 缺省以外该取哪一库）
    pub dimension: DbDimension,
    /// 该表记录计数（同一读事务快照）
    pub count: u64,
    /// 表介绍（struct doc，句读截断后）
    pub summary: String,
}

/// 单表字段说明（`db_tables` 下钻形态）。
#[derive(Debug, Clone, PartialEq)]
pub struct TableDoc {
    /// 表名
    pub table: String,
    /// 所属数据维度
    pub dimension: DbDimension,
    /// 表介绍（句读截断后）
    pub summary: String,
    /// 字段清单（源码顺序）
    pub fields: Vec<FieldDoc>,
}

/// 字段说明一行。
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDoc {
    /// 落库线名（camelCase——`db_query` 的 `field` 入参取值）
    pub name: String,
    /// 渲染后的类型文本（见 [`render_type`]）
    pub ty: String,
    /// 字段说明（doc 注释，句读截断后）
    pub doc: String,
}

/// 字段值的比较类（`query` 的值类型校验用；与 [`render_type`] 同一棵 match——
/// 「一处说 string、另一处按 number 校验」是最难查的一类漂移）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKind {
    /// 数值（含全部整型 / 浮点位宽）
    Number,
    /// 文本（`str` / `char` / 命名枚举——线值是字符串）
    Text,
    /// 布尔
    Bool,
    /// 对象 / 数组（不校验值形状）
    Object,
    /// 不知（opaque / 泛型 / 嵌套路径）
    Unknown,
}

/// 单表字段索引（`query` 的静态校验面）：线名 + 比较类 + 渲染文本三元组。
pub(crate) struct FieldIndex {
    fields: Vec<(String, FieldKind, String)>,
}

impl FieldIndex {
    /// 全部顶层线名（未知字段错误文案的候选集）。
    pub(crate) fn names(&self) -> Vec<&str> {
        self.fields
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect()
    }

    /// 路径 → 比较类：顶层可查，含点号的嵌套路径返回 [`FieldKind::Unknown`]
    /// （嵌套值形态多样，静态深校验会把「合法但空」误判成错，见 design DD11）。
    pub(crate) fn kind_of(&self, path: &str) -> FieldKind {
        if path.contains('.') {
            return FieldKind::Unknown;
        }
        self.fields
            .iter()
            .find(|(name, _, _)| name == path)
            .map_or(FieldKind::Unknown, |(_, kind, _)| *kind)
    }

    /// 路径 → 渲染类型文本（未知路径 `None`；错误文案携字段类型用）。
    pub(crate) fn render_of(&self, path: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _, _)| name == path)
            .map(|(_, _, render)| render.as_str())
    }
}

/// 类型图 + 各表的根 `DataType`（根是 `Reference::Named`，`Format` 应用后引用
/// 仍可解析——`map_types` 是 `types.clone()` + 原地改属性，命名 id 存活）。
struct Graph {
    types: Types,
    roots: Vec<(&'static str, DataType)>,
}

/// 注册表 `schema` 字段的 derive 型实现：把本模型（及其依赖命名类型）灌进
/// `Types` 并返回其根 `DataType`。
pub(crate) fn schema_of<T: Type>(types: &mut Types) -> DataType {
    T::definition(types)
}

/// `session_event` 的 schema 供给（本表不 derive `Type`：`event` 载荷是
/// `Box<RawValue>`，无类型图表达）。3 条字段说明在此单独供给，线名 / 键集由
/// ratchet 机械对齐（`eventKey` / `sessionId` / `event` 与真实 serde 键集一致）；
/// `event` 以 opaque 引用表达——渲染为 `object`（模型据此知道「可点号下钻」）。
pub(crate) fn schema_session_event(types: &mut Types) -> DataType {
    let datatype = Struct::named()
        .field(
            "eventKey",
            field(
                DataType::Primitive(Primitive::str),
                "打包主键 `(hash64(session_id) << 64 | seq)` 的十六进制串形态。",
            ),
        )
        .field(
            "sessionId",
            field(
                DataType::Primitive(Primitive::str),
                "所属会话 id（重放查询入口）。",
            ),
        )
        .field(
            "event",
            field(
                DataType::Reference(Reference::opaque(SESSION_EVENT_PAYLOAD)),
                "密封事件载荷原文（`kind` / `role` / `seq` / `blocks` 等字段直挂此层，可点号下钻）。",
            ),
        )
        .build();
    let named = NamedDataType::new("SessionEventRecord", types, |_, ndt| {
        ndt.docs = "agent 会话转录单表".into();
        ndt.ty = Some(datatype);
    });
    DataType::Reference(named.reference(Vec::new()))
}

/// `event` opaque 引用的状态标识（`Reference::opaque` 需 `Hash + Eq + Send + Sync`）。
const SESSION_EVENT_PAYLOAD: &str = "session_event_payload";

/// 字段夹具：类型 + doc（`Field` 的其余字段取缺省）。
fn field(ty: DataType, docs: &str) -> Field {
    let mut field = Field::new(ty);
    field.docs = docs.to_owned().into();
    field
}

/// 类型图派生单点（注册表逐行灌入 → 两侧口径同源的唯一入口；`Format` 的
/// 应用点全仓仅此一处）。
fn graph() -> Result<Graph, QueryError> {
    let mut types = Types::default();
    let mut roots = Vec::with_capacity(entries().len());
    for entry in entries() {
        roots.push((entry.name, (entry.schema)(&mut types)));
    }
    let formatted = specta_serde::Format
        .map_types(&types)
        .map_err(|error| QueryError::Invalid(format!("schema 派生失败: {error}")))?;
    Ok(Graph {
        types: formatted.into_owned(),
        roots,
    })
}

/// 取本表的命名类型（根必为 `Reference::Named`；非引用即内部不一致）。
fn named<'a>(graph: &'a Graph, table: &str) -> Result<&'a NamedDataType, QueryError> {
    let root = graph
        .roots
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, root)| root)
        .ok_or_else(|| QueryError::Invalid(format!("未知表: {table}")))?;
    let DataType::Reference(Reference::Named(reference)) = root else {
        return Err(QueryError::Invalid(format!(
            "schema 派生失败: {table} 的根类型不是命名引用"
        )));
    };
    graph
        .types
        .get(reference)
        .ok_or_else(|| QueryError::Invalid(format!("schema 派生失败: {table} 命名类型缺失")))
}

/// 命名类型的字段清单（`Fields::Unit` / `Unnamed` 视为空——本仓记录模型均为
/// 命名结构体）。
fn fields_of(named: &NamedDataType) -> &[(std::borrow::Cow<'static, str>, Field)] {
    match named.ty.as_ref() {
        Some(DataType::Struct(Struct {
            fields: Fields::Named(named_fields),
            ..
        })) => &named_fields.fields,
        _ => &[],
    }
}

/// 本维度表清单（**不展开字段**：清单约 1.4 KB、字段全量约 11 KB，每轮白烧）。
pub(crate) fn tables(
    db: &native_db::Database<'static>,
    dimension: DbDimension,
) -> Result<Vec<ModelDoc>, QueryError> {
    let graph = graph()?;
    entries()
        .iter()
        .filter(|entry| entry.dimension == dimension)
        .map(|entry| {
            let named = named(&graph, entry.name)?;
            Ok(ModelDoc {
                table: entry.name.to_owned(),
                dimension: entry.dimension,
                count: (entry.count)(db).map_err(store_err)?,
                summary: truncate_doc(&named.docs),
            })
        })
        .collect()
}

/// 单表全字段说明；未知表 `Err` 携**本维度**候选表名（跨维度同名不存在——
/// 表名跨维度唯一由注册表保证）。
pub(crate) fn table(dimension: DbDimension, name: &str) -> Result<TableDoc, QueryError> {
    let Some(entry) = entry(name) else {
        return Err(QueryError::Invalid(format!(
            "未知表: {name}（候选: {}）",
            candidates(dimension)
        )));
    };
    if entry.dimension != dimension {
        return Err(QueryError::Invalid(format!(
            "表 {name} 属 {} 维度，不在本库（候选: {}）",
            dimension_name(entry.dimension),
            candidates(dimension)
        )));
    }
    let graph = graph()?;
    let named = named(&graph, name)?;
    let fields = fields_of(named)
        .iter()
        .map(|(wire, field)| FieldDoc {
            name: wire.to_string(),
            ty: match field.ty.as_ref() {
                Some(ty) => render_type(ty, &graph.types),
                None => "unknown".to_owned(),
            },
            doc: truncate_doc(&field.docs),
        })
        .collect();
    Ok(TableDoc {
        table: name.to_owned(),
        dimension: entry.dimension,
        summary: truncate_doc(&named.docs),
        fields,
    })
}

/// 单表字段索引（`query` 静态校验面，与 `db_tables` 下钻输出同源）。
pub(crate) fn field_index(table: &str) -> Result<FieldIndex, QueryError> {
    let graph = graph()?;
    let named = named(&graph, table)?;
    Ok(FieldIndex {
        fields: fields_of(named)
            .iter()
            .map(|(wire, field)| {
                let (kind, render) = match field.ty.as_ref() {
                    Some(ty) => (field_kind(ty, &graph.types), render_type(ty, &graph.types)),
                    None => (FieldKind::Unknown, "unknown".to_owned()),
                };
                (wire.to_string(), kind, render)
            })
            .collect(),
    })
}

/// 本维度候选表名（`、` 分隔；错误文案即自纠通道）。
pub(crate) fn candidates(dimension: DbDimension) -> String {
    entries()
        .iter()
        .filter(|entry| entry.dimension == dimension)
        .map(|entry| entry.name)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 全量候选表名（未知表名文案用——表名跨维度唯一，故全列一遍）。
pub(crate) fn all_candidates() -> String {
    entries()
        .iter()
        .map(|entry| entry.name)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 维度的人读名（错误文案用）。
pub(crate) fn dimension_name(dimension: DbDimension) -> &'static str {
    match dimension {
        DbDimension::User => "user",
        DbDimension::Workspace => "workspace",
    }
}

/// store 错误 → 查询错误面（`StoreError::Db` 的 `db:` 前缀不二次叠加）。
pub(crate) fn store_err(error: StoreError) -> QueryError {
    match error {
        StoreError::Db(message) => QueryError::Db(message),
        other => QueryError::Db(other.to_string()),
    }
}

/// 引用处的内联具体类型：specta 把 `String` / `Vec<T>` 这类容器型表达为「具名
/// 类型 + 内联使用点」（具名侧 `ty = None`，具体形状只挂在引用处）。不取它，
/// `String` 字段会渲染成 `String`、比较类落 `Unknown`——查询的字符串值类型校验
/// 随之整体失效（值类型不符静默放行）。
fn inlined(reference: &NamedReference) -> Option<&DataType> {
    match &reference.inner {
        NamedReferenceType::Inline { dt, .. } => Some(dt.as_ref()),
        _ => None,
    }
}

/// 类型渲染（人读文本）：`Primitive` 按位宽收敛、`Reference(Named)` 取命名
/// 类型名（`NamedReference` 无 `Display`，MUST 经 `types.get` 取名）、
/// `Nullable` 渲染 `T | null`。
pub(crate) fn render_type(ty: &DataType, types: &Types) -> String {
    match ty {
        DataType::Primitive(primitive) => primitive_text(primitive).to_owned(),
        DataType::Nullable(inner) => format!("{} | null", render_type(inner, types)),
        DataType::List(list) => format!("{}[]", render_type(&list.ty, types)),
        DataType::Map(map) => format!(
            "{{ [key: {}]: {} }}",
            render_type(map.key_ty(), types),
            render_type(map.value_ty(), types)
        ),
        DataType::Reference(Reference::Named(reference)) => match inlined(reference) {
            Some(inner) => render_type(inner, types),
            None => types
                .get(reference)
                .map_or_else(|| "unknown".to_owned(), |named| named.name.to_string()),
        },
        DataType::Reference(Reference::Opaque(_)) => "object".to_owned(),
        DataType::Enum(_) => "string".to_owned(),
        DataType::Struct(_) | DataType::Tuple(_) | DataType::Intersection(_) => "object".to_owned(),
        DataType::Generic(_) => "object".to_owned(),
    }
}

/// 值比较类（与 [`render_type`] 同一分类器的另一半：展示文本与校验类 MUST
/// 同源，否则会出现「说 string、按 number 校验」的静默漂移）。
pub(crate) fn field_kind(ty: &DataType, types: &Types) -> FieldKind {
    match ty {
        DataType::Primitive(primitive) => primitive_kind(primitive),
        DataType::Nullable(inner) => field_kind(inner, types),
        DataType::Reference(Reference::Named(reference)) => match inlined(reference) {
            Some(inner) => field_kind(inner, types),
            None => match types.get(reference).and_then(|named| named.ty.as_ref()) {
                Some(DataType::Enum(_)) => FieldKind::Text,
                Some(inner) => field_kind(inner, types),
                None => FieldKind::Unknown,
            },
        },
        DataType::Reference(Reference::Opaque(_)) | DataType::Generic(_) => FieldKind::Unknown,
        DataType::Enum(_) => FieldKind::Text,
        DataType::List(_)
        | DataType::Map(_)
        | DataType::Struct(_)
        | DataType::Tuple(_)
        | DataType::Intersection(_) => FieldKind::Object,
    }
}

/// 原始类型的展示文本。
fn primitive_text(primitive: &Primitive) -> &'static str {
    match primitive {
        Primitive::bool => "boolean",
        Primitive::char | Primitive::str => "string",
        _ => "number",
    }
}

/// 原始类型的比较类（展示文本与校验类的同一分类点）。
fn primitive_kind(primitive: &Primitive) -> FieldKind {
    match primitive {
        Primitive::bool => FieldKind::Bool,
        Primitive::char | Primitive::str => FieldKind::Text,
        _ => FieldKind::Number,
    }
}

/// 括号平衡收口：截断点落在未闭合的 `（` / `(` 内时回退到该开括号之前——句读截
/// 断不得留下「表介绍（半句」这类残句（长 doc 的括号常跨行）。
fn balance(text: &str) -> &str {
    let mut depth = 0usize;
    let mut last_open: Option<usize> = None;
    for (index, char) in text.char_indices() {
        match char {
            '（' | '(' => {
                depth += 1;
                last_open = Some(index);
            }
            '）' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    match (depth, last_open) {
        (0, _) => text,
        (_, Some(index)) => &text[..index],
        (_, None) => text,
    }
}

/// 说明截断（表介绍与字段 doc 同规则，单点复用）：取首个 `。` / `；` / 换行
/// （含该字符）为止；无句读或该句读位置超 [`MAX_DOC_CHARS`] 字 → 截到该字数
/// （`char` 边界）并补省略号；尾部空白剥离。
pub(crate) fn truncate_doc(doc: &str) -> String {
    let text = doc.trim();
    let terminator = text
        .char_indices()
        .find(|(_, c)| matches!(c, '。' | '；' | '\n'))
        .map(|(index, c)| index + c.len_utf8());
    if let Some(end) = terminator {
        if text[..end].chars().count() <= MAX_DOC_CHARS {
            return balance(&text[..end]).trim_end().to_owned();
        }
    }
    match text.char_indices().nth(MAX_DOC_CHARS) {
        Some((end, _)) => format!("{}…", text[..end].trim_end()),
        None => text.to_owned(),
    }
}
