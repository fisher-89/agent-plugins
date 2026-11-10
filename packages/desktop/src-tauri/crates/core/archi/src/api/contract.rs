//! `<FQN>.md` 解析与双向 gate：手写契约是**唯一真相**，本模块是它的唯一读面。
//!
//! 章节位置语义封闭为五类（零 markdown 解析依赖——「扫行 + fence 状态机」即可
//! 机械判定）：
//!
//! | 位置 | 承载 |
//! |---|---|
//! | H1 标题 | `<FQN>`（与文件名一致，作锚点校验） |
//! | `## <二标题>` | 散文区（人审主体：职能 / 边界 / 不变量 / 承载理由） |
//! | `### <op-id>` + 紧随一个 `api-op` fenced json | 操作锚点与签名（块内 MUST NOT 自包含 op 名） |
//! | `api-types` fenced json | 本模块自有 `$defs` 定义（同文件唯一） |
//! | `api-reexport` fenced json | 透传清单（键 = 导出名，值 = 直接子元素 FQN；同文件唯一） |
//!
//! **载体为 md 内含 fenced json**：MUST NOT 用 JSON 内嵌 `description`（散文变整行
//! 超长字符串），MUST NOT 用 YAML block scalar（YAML 1.1 把 `off` / `on` / `yes` /
//! `no` 解析为布尔，与「语言无关」正面对撞）。
//!
//! **fence 内的重复键在文本层判**（D37）：`serde_json` 解析时把重复键折叠为「后者胜、
//! 前者消失」，故 `&Value` 面结构上不可达「同名两处定义」这一形态——判据只能落在
//! [`parse`] 交给 `serde_json` 之前的同一份文本上（[`duplicate_keys`]）。`$defs` 的
//! 键名即身份，同名即定义两次。
//!
//! **[`verify`] 是装载期的校验收口**：三条双向 gate + 闸一（七族名字 MUST 是 ASCII
//! 标识符，D45）+ 落点闸（`x-function` 的合法落点封闭二处，D47）。三者同函数、同趟
//! 收口——它们都是「契约资产的错」，须在 `load_contracts` 中止，MUST NOT 留到写盘面。
//! 渲染期因此不做二次校验（名字与位置都已保证），`emit` 内零标识符判据、两侧
//! `value_type` 零 `x-function` 分支。
//!
//! 错误一律带 `file:line` 与原文片段——契约坏了要能直接改到那一行。

use serde_json::{Map, Value};

use super::{kind_of, ApiError};

/// 操作 fence 的语言标。
const FENCE_OP: &str = "api-op";

/// 自有类型 fence 的语言标。
const FENCE_TYPES: &str = "api-types";

/// 透传清单 fence 的语言标。
const FENCE_REEXPORT: &str = "api-reexport";

/// 函数标记层关键字（D47 落点闸判的对象：合法落点封闭二处）。
const X_FUNCTION: &str = "x-function";

/// 构造器名的精确匹配集（契约 MUST NOT 声明构造器——构造器由生成器从 `$defs` 条目
/// 自动产出）。
const CONSTRUCTOR_NAMES: [&str; 6] = ["new", "default", "build", "empty", "never", "open"];

/// 构造器名的前缀集（`from_*` / `with_*`）。
const CONSTRUCTOR_PREFIXES: [&str; 2] = ["from_", "with_"];

/// 一份已解析的契约（手写真相的内存形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    /// 元素 FQN（= H1 标题，与文件名同源）
    pub fqn: String,
    /// H1 标题原文
    pub title: String,
    /// 契约文件（repo 相对 POSIX，错误文案与报告用）
    pub file: String,
    /// 操作清单（声明序）
    pub ops: Vec<ContractOp>,
    /// 本模块自有 `$defs` 定义（`api-types` fence 原文，声明序）
    pub types: Map<String, Value>,
    /// 透传清单（`api-reexport` fence 原文：键 = 导出名、值 = 直接子元素 FQN）
    pub reexports: Map<String, Value>,
    /// 散文区里出现的透传导出名（双向 gate 的另一侧）
    pub reexport_prose: Vec<String>,
    /// 散文区（`##` 段；供 gate 与索引摘要）
    pub sections: Vec<ContractSection>,
}

/// 一个操作锚点 + 其签名（`api-op` fence 原文，块内不自包含 op 名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractOp {
    /// 操作 id（`### <op-id>` 的原文）
    pub id: String,
    /// heading 所在行（1 起始）
    pub line: u32,
    /// `api-op` fence 原文（原生 JSON Schema 子集，生成器只装配不翻译）
    pub signature: Value,
}

/// 一段散文区（`## <heading>` 及其正文，fence 内容不入 body）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractSection {
    /// 二标题原文
    pub heading: String,
    /// heading 所在行（1 起始）
    pub line: u32,
    /// 正文（不含任何 fenced code block 的内容行）
    pub body: String,
}

/// 解析一份契约：H1 锚点 → 散文区 → `### <op-id>` 分段 → 三类 fence。
///
/// 结构性违反（H1 与文件名不一致 / op heading 无 fence / op fence 无 heading / 同一
/// 文件出现第二个 `api-types` 或 `api-reexport` / fence 内容非 JSON 对象）一律
/// `ApiError::Contract`，文案带 `file:line` 与原文——MUST NOT 静默跳过。
pub fn parse(source: &str, fqn: &str, file: &str) -> Result<Contract, ApiError> {
    let expected = file_title(file, fqn);
    let mut title: Option<String> = None;
    let mut ops: Vec<ContractOp> = Vec::new();
    let mut types: Option<Map<String, Value>> = None;
    let mut reexports: Option<Map<String, Value>> = None;
    let mut sections: Vec<ContractSection> = Vec::new();
    let mut section: Option<ContractSection> = None;
    let mut pending_op: Option<(String, u32)> = None;
    let mut fence: Option<(Vec<String>, u32, String)> = None;

    for (index, raw) in source.lines().enumerate() {
        let line = index as u32 + 1;
        let trimmed = raw.trim_end();
        let trimmed_start = trimmed.trim_start();

        if let Some((collected, start, info)) = fence.as_mut() {
            if trimmed_start.starts_with("```") {
                let body = collected.join("\n");
                let start_line = *start;
                let info = info.clone();
                fence = None;
                // 语言标可为单 token（`api-op`）或 `json api-op` 形态——两者同判
                let tokens = info_tokens(&info);
                if tokens.contains(&FENCE_OP) {
                    let Some((id, heading_line)) = pending_op.take() else {
                        return Err(ApiError::Contract(format!(
                            "{file}:{start_line}: {FENCE_OP} 代码块不属于任何 `### <op-id>` heading\
                             （op 块 MUST NOT 自包含 op 名）"
                        )));
                    };
                    let signature = parse_object_value(&body, file, start_line, FENCE_OP)?;
                    ops.push(ContractOp {
                        id,
                        line: heading_line,
                        signature,
                    });
                    continue;
                }
                if tokens.contains(&FENCE_TYPES) {
                    if types.is_some() {
                        return Err(ApiError::Contract(format!(
                            "{file}:{start_line}: {FENCE_TYPES} 只允许出现一次（第二处即重复定义）"
                        )));
                    }
                    types = Some(parse_object(&body, file, start_line, FENCE_TYPES)?);
                    continue;
                }
                if tokens.contains(&FENCE_REEXPORT) {
                    if reexports.is_some() {
                        return Err(ApiError::Contract(format!(
                            "{file}:{start_line}: {FENCE_REEXPORT} 只允许出现一次"
                        )));
                    }
                    reexports = Some(parse_object(&body, file, start_line, FENCE_REEXPORT)?);
                    continue;
                }
                // 其余 fence（示例代码等）不参与契约，内容不入散文区
                continue;
            }
            collected.push(trimmed.to_owned());
            continue;
        }

        if trimmed_start.starts_with("```") {
            let info = trimmed_start.trim_start_matches("```").trim().to_owned();
            fence = Some((Vec::new(), line, info));
            continue;
        }

        if let Some(heading) = trimmed_start.strip_prefix("### ") {
            if let Some((id, heading_line)) = pending_op.take() {
                return Err(ApiError::Contract(format!(
                    "{file}:{heading_line}: op `{id}` 缺 {FENCE_OP} 代码块（heading 之后必须紧随一个）"
                )));
            }
            finish_section(&mut section, &mut sections);
            pending_op = Some((heading.trim().to_owned(), line));
            continue;
        }

        if let Some(heading) = trimmed_start.strip_prefix("## ") {
            if let Some((id, heading_line)) = pending_op.take() {
                return Err(ApiError::Contract(format!(
                    "{file}:{heading_line}: op `{id}` 缺 {FENCE_OP} 代码块（heading 之后必须紧随一个）"
                )));
            }
            finish_section(&mut section, &mut sections);
            section = Some(ContractSection {
                heading: heading.trim().to_owned(),
                line,
                body: String::new(),
            });
            continue;
        }

        if let Some(heading) = trimmed_start.strip_prefix("# ") {
            if title.is_some() {
                return Err(ApiError::Contract(format!(
                    "{file}:{line}: 出现第二个 H1 标题（`{}`），契约 MUST NOT 多 H1",
                    heading.trim()
                )));
            }
            finish_section(&mut section, &mut sections);
            title = Some(heading.trim().to_owned());
            continue;
        }

        if let Some(current) = section.as_mut() {
            current.body.push_str(trimmed);
            current.body.push('\n');
        }
    }

    if let Some((id, heading_line)) = pending_op {
        return Err(ApiError::Contract(format!(
            "{file}:{heading_line}: op `{id}` 缺 {FENCE_OP} 代码块（heading 之后必须紧随一个）"
        )));
    }
    if fence.is_some() {
        return Err(ApiError::Contract(format!(
            "{file}: 文件结束时 fenced code block 未闭合"
        )));
    }
    finish_section(&mut section, &mut sections);

    let Some(title) = title else {
        return Err(ApiError::Contract(format!(
            "{file}: 缺 H1 标题（契约 MUST 以 `# {expected}` 起手）"
        )));
    };
    if title != expected {
        return Err(ApiError::Contract(format!(
            "{file}: H1 标题 `{title}` 与文件名 `{expected}` 不一致（H1 是契约锚点，必须逐字相同）"
        )));
    }
    let mut seen: Vec<&str> = Vec::new();
    for op in &ops {
        if seen.contains(&op.id.as_str()) {
            return Err(ApiError::Contract(format!(
                "{file}:{}: 重复的 op id `{}`（一 op 一段，MUST NOT 重复声明）",
                op.line, op.id
            )));
        }
        seen.push(&op.id);
    }

    let reexport_prose = collect_reexport_prose(&sections);
    Ok(Contract {
        fqn: fqn.to_owned(),
        title,
        file: file.to_owned(),
        ops,
        types: types.unwrap_or_default(),
        reexports: reexports.unwrap_or_default(),
        reexport_prose,
        sections,
    })
}

/// 双向 gate（三条，机械）：`### <op-id>` heading 集合 ≡ `api-op` fence 归属集合
/// （解析期已逐条绑定，此处只防重复）；`api-types` 键集 ⊆ `schema` 的 `$defs` 键集；
/// `api-reexport` 键集 ≡ 散文透传列表。任一不成立即契约错误并**点名差集**。
///
/// 三条之后是**闸一**（D45，语言无关）：七族名字 MUST 是 ASCII 标识符——落本函数而非
/// `emit`（写盘面报错太晚）或 `schema::verify_defs`（只判 `$defs` 内部结构，拿不到 op id、
/// `owner` 与参数名）。装载期是本仓唯一的中止点：[`super::load_contracts`] 失败即不产出半真报告。
///
/// 闸一之后是**落点闸**（D47）：`x-function` 的合法落点**封闭二处**——`api-op` fence 的顶层
/// 签名与 `$defs` 条目第一层 `properties` 的**值**；其余位置一律契约错误并点名 JSON pointer
/// （判的是「键在哪儿」而非「名字长什么样」，故它不是闸一的第八族，而是 ④ 族射程的前置）。
pub fn verify(contract: &Contract, schema: &Value) -> Result<(), ApiError> {
    let mut seen: Vec<&str> = Vec::new();
    for op in &contract.ops {
        if seen.contains(&op.id.as_str()) {
            return Err(ApiError::Contract(format!(
                "{}:{}: 重复的 op id `{}`（heading 集合与 {FENCE_OP} fence 归属集合必须一一对应）",
                contract.file, op.line, op.id
            )));
        }
        seen.push(&op.id);
    }

    let defined: Vec<String> = schema
        .get("$defs")
        .and_then(Value::as_object)
        .map(|defs| defs.keys().cloned().collect())
        .unwrap_or_default();
    let mut missing: Vec<&str> = Vec::new();
    for key in contract.types.keys() {
        if !defined.iter().any(|name| name == key) {
            missing.push(key);
        }
    }
    if !missing.is_empty() {
        return Err(ApiError::Contract(format!(
            "{}: {FENCE_TYPES} 的键在生成的 `$defs` 里缺失：{}",
            contract.file,
            missing.join(" / ")
        )));
    }

    let mut fence_keys: Vec<&str> = contract.reexports.keys().map(String::as_str).collect();
    fence_keys.sort_unstable();
    let mut prose: Vec<&str> = contract.reexport_prose.iter().map(String::as_str).collect();
    prose.sort_unstable();
    if fence_keys != prose {
        let only_fence: Vec<&str> = fence_keys
            .iter()
            .copied()
            .filter(|key| !prose.contains(key))
            .collect();
        let only_prose: Vec<&str> = prose
            .iter()
            .copied()
            .filter(|key| !fence_keys.contains(key))
            .collect();
        return Err(ApiError::Contract(format!(
            "{}: {FENCE_REEXPORT} 键集与散文透传列表不等（仅 fence：{}；仅散文：{}）",
            contract.file,
            if only_fence.is_empty() {
                "无".to_owned()
            } else {
                only_fence.join(" / ")
            },
            if only_prose.is_empty() {
                "无".to_owned()
            } else {
                only_prose.join(" / ")
            }
        )));
    }
    // 闸一（D45，语言无关，装载期）：**七族**名字 MUST 是 ASCII 标识符
    // `[A-Za-z_][A-Za-z0-9_]*` —— ① `### <op-id>` 的 id（含 `api-types` 里
    // `x-function` / `x-class` 的方法名）；② `api-types` 的 `$defs` 键名；③
    // `api-reexport` 的键名；④ 每个 `x-function` 的 `params[].name`（`api-op` 顶层签名、
    // `$defs` 的 `x-class` 方法与其函数指针字段同判）；⑤ `$defs` 条目的 `properties` 键
    // （数据字段名，渲染器直插为 `{field}`）；⑥ `api-op` fence 的 `owner` 键
    // （`op_witness` 直插为 `super::{owner}::{op.id}`）；⑦ 所有 `$ref` 的末段名
    // （两侧 `value_type` 经 [`super::ref_name`] 直插为类型位置）。
    //
    // 名字类的错是**契约资产的错**，须在装载期中止（写盘面报错太晚，且 `--check` 的另一侧
    // 已生成文本）；渲染期 MUST NOT 重复校验同一事实（`emit/{rust,ts}.rs` 里零
    // `is_identifier` 调用），`params[].name` 的「缺名落 `arg`」兜底保留（缺名与非法名是
    // 两件事）。七族与渲染期插值点的对照见 D45 的「插值点 ↔ 来源族」枚举。
    for op in &contract.ops {
        if !is_identifier(&op.id) {
            return Err(identifier_error(
                contract, "op id", &op.id, FENCE_OP, op.line,
            ));
        }
        for name in function_param_names(&op.signature) {
            if !is_identifier(&name) {
                return Err(identifier_error(
                    contract,
                    "参数名",
                    &name,
                    FENCE_OP,
                    op.line,
                ));
            }
        }
        if let Some(owner) = op.signature.get("owner").and_then(Value::as_str) {
            if !is_identifier(owner) {
                return Err(identifier_error(
                    contract,
                    "owner 键",
                    owner,
                    FENCE_OP,
                    op.line,
                ));
            }
        }
        for name in reference_names(&op.signature) {
            if !is_identifier(&name) {
                return Err(identifier_error(
                    contract,
                    "$ref 末段名",
                    &name,
                    FENCE_OP,
                    op.line,
                ));
            }
        }
    }
    for name in contract.types.keys() {
        if !is_identifier(name) {
            return Err(identifier_error(
                contract,
                "$defs 键名",
                name,
                FENCE_TYPES,
                0,
            ));
        }
    }
    for definition in schema
        .get("$defs")
        .and_then(Value::as_object)
        .map(|defs| defs.values().collect::<Vec<&Value>>())
        .unwrap_or_default()
    {
        for field in property_names(definition) {
            if !is_identifier(&field) {
                return Err(identifier_error(
                    contract,
                    "properties 数据字段键",
                    &field,
                    FENCE_TYPES,
                    0,
                ));
            }
        }
        for (method, function) in methods_of(definition) {
            if !is_identifier(&method) {
                return Err(identifier_error(
                    contract,
                    "方法名",
                    &method,
                    FENCE_TYPES,
                    0,
                ));
            }
            for name in function_param_names(function) {
                if !is_identifier(&name) {
                    return Err(identifier_error(contract, "参数名", &name, FENCE_TYPES, 0));
                }
            }
        }
        for name in reference_names(definition) {
            if !is_identifier(&name) {
                return Err(identifier_error(
                    contract,
                    "$ref 末段名",
                    &name,
                    FENCE_TYPES,
                    0,
                ));
            }
        }
    }
    for name in contract.reexports.keys() {
        if !is_identifier(name) {
            return Err(identifier_error(
                contract,
                "透传导出名",
                name,
                FENCE_REEXPORT,
                0,
            ));
        }
    }
    // 落点闸（D47）：与七族闸一**同函数、同趟收口**（MUST NOT 拆第二个公开入口）。
    // 位置错了就不该走到渲染期——否则深层 `x-function` 的 `params[].name` 会沿
    // `value_type` → `function_type` / `method_signature` 裸插，而 ④ 族从未判过它。
    verify_x_function_positions(contract, schema)?;
    Ok(())
}

/// 落点闸（D47）：`x-function` 的合法落点**封闭二处**——① `api-op` fence 的顶层签名
/// （裸形态与显式 `{"x-function": …}` 包裹形态等价，装配期由
/// [`super::schema::function_schema`] 归一为单层标记）；② `$defs` 条目**第一层**
/// `properties` 的**值**（方法 / 函数指针字段）。
///
/// 判的是「键在哪儿」而非「名字长什么样」，故它不是闸一的第八族；它的作用是让闸一 ④
/// 族（`params[].name`）的有限收集面成为**真全覆盖**（不存在「深层 `x-function` 从未
/// 进判」的路径）——这是 D47 的直接溯源。
///
/// **MUST NOT** 落 [`super::schema::verify_defs`]：合法落点跨两处来源（`api-op` 顶层
/// 签名只有本函数看得到，`$defs` 在装配后才有），只有 [`verify`] 同时持有 `Contract`
/// 与已装配的 schema。
fn verify_x_function_positions(contract: &Contract, schema: &Value) -> Result<(), ApiError> {
    for op in &contract.ops {
        scan_x_function(
            &op.signature,
            &format!("{FENCE_OP}.{}", op.id),
            true,
            contract,
        )?;
    }
    for (name, definition) in schema
        .get("$defs")
        .and_then(Value::as_object)
        .map(|defs| defs.iter().collect::<Vec<(&String, &Value)>>())
        .unwrap_or_default()
    {
        let pointer = format!("$defs.{name}");
        let Some(object) = definition.as_object() else {
            continue;
        };
        // `$defs` 条目**自身顶层**的 `x-function`（`{"x-function": …}` 作为整个条目）非法：
        // 函数签名无从「按名见证」，且它会让类型面凭空多出一个无名字的条目。
        if object.contains_key("x-function") {
            return Err(x_function_position_error(contract, &pointer));
        }
        for (key, value) in object {
            if key == "properties" {
                if let Some(properties) = value.as_object() {
                    for (field, property) in properties {
                        scan_x_function(
                            property,
                            &format!("{pointer}.properties.{field}"),
                            true,
                            contract,
                        )?;
                    }
                    continue;
                }
            }
            scan_x_function(value, &format!("{pointer}.{key}"), false, contract)?;
        }
    }
    for (name, target) in &contract.reexports {
        scan_x_function(target, &format!("{FENCE_REEXPORT}.{name}"), false, contract)?;
    }
    Ok(())
}

/// 落点闸的递归主体（逐层携带 pointer 前缀）：`allowed` 为真表示**本节点上**的
/// `x-function` 键合法（合法二处的落点），其子树一律转假——合法的 `x-function` 值内部
/// 不得再有 `x-function`（那又是一层「函数作为值」，同样表达不了）。
fn scan_x_function(
    value: &Value,
    pointer: &str,
    allowed: bool,
    contract: &Contract,
) -> Result<(), ApiError> {
    match value {
        Value::Object(object) => {
            for (key, inner) in object {
                if key == X_FUNCTION {
                    if !allowed {
                        return Err(x_function_position_error(contract, pointer));
                    }
                    scan_x_function(inner, &format!("{pointer}.{X_FUNCTION}"), false, contract)?;
                    continue;
                }
                scan_x_function(inner, &format!("{pointer}.{key}"), false, contract)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                scan_x_function(item, &format!("{pointer}[{index}]"), false, contract)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// 落点闸的错误面：**定形族别串** `x-function 落点`（判据锚它，MUST NOT 靠散文近似
/// 措辞）+ 该 `x-function` 所在的 **JSON pointer**。
fn x_function_position_error(contract: &Contract, pointer: &str) -> ApiError {
    ApiError::Contract(format!(
        "{}: x-function 落点非法（JSON pointer `{pointer}`）——`{X_FUNCTION}` 只许出现在 \
         `{FENCE_OP}` fence 的顶层签名与 `$defs` 条目第一层 `properties` 的值二处；\
         其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 的元素内、\
         `properties` 的第二层及更深、`$defs` 条目自身顶层、`{FENCE_REEXPORT}` 的任何位置）\
         都表达不了「函数作为具名值」：请去掉标记层，或把该形状提升为具名 `$defs` 条目",
        contract.file
    ))
}

/// 闸一的错误面（族别 + 名字 + fence + 行；行号未知时退化为 `file` 级前缀）。
fn identifier_error(
    contract: &Contract,
    family: &str,
    name: &str,
    fence: &str,
    line: u32,
) -> ApiError {
    let where_ = if line == 0 {
        contract.file.clone()
    } else {
        format!("{}:{line}", contract.file)
    };
    ApiError::Contract(format!(
        "{where_}: {fence} 的 {family} `{name}` 不是 ASCII 标识符（`[A-Za-z_][A-Za-z0-9_]*`）——\
         契约里的名字会直接成为目标语言源码的标识符：MUST NOT 含空格 / 连字符 / 点 / 首字符数字；\
         需要人类可读的措辞请写在散文区"
    ))
}

/// 一段 `x-function` 形态值里的 `params[].name`（裸形态与 `{"x-function": …}` 包裹形态同判；
/// 缺名不产出条目——兜底 `arg` 属渲染期，不在本闸判）。
fn function_param_names(signature: &Value) -> Vec<String> {
    let function = signature.get("x-function").unwrap_or(signature);
    function
        .get("params")
        .and_then(Value::as_array)
        .map(|params| {
            params
                .iter()
                .filter_map(|param| param.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// `$defs` 条目里带 `x-function` 的 `properties` 项（`x-class` 的 trait 方法与函数指针
/// 字段同形：都是 `properties` 的值；返回（方法名，该 `x-function` 值））。
fn methods_of(definition: &Value) -> Vec<(String, &Value)> {
    definition
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(method, property)| {
                    property
                        .get("x-function")
                        .map(|function| (method.clone(), function))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `$defs` 条目的 **`properties` 数据字段**键（⑤ 族：`struct_block` / `interface_block` 把
/// `{field}` 直插为结构体字段 / interface 属性）。
///
/// 带 `x-function` 的项**不在此列**：那是方法名 / 函数指针字段名，归 ①「方法名」族
/// （[`methods_of`] 判）——两族各报各的，同名事实不得由两个族各判一遍（先判者胜会让
/// 错误面的族别词随遍历顺序漂移）。
fn property_names(definition: &Value) -> Vec<String> {
    definition
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter(|(_, property)| property.get("x-function").is_none())
                .map(|(field, _)| field.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// 一棵 JSON 子树内所有 `$ref` 的**末段名**（⑦ 族；末段切分走 [`super::ref_name`]，
/// MUST NOT 自写第二个切分——`check` 与两渲染器共用同一单点）。
fn reference_names(value: &Value) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    collect_reference_names(value, &mut names);
    names
}

/// [`reference_names`] 的递归主体（对象逐值、数组逐项）。
fn collect_reference_names(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, inner) in object {
                if key == "$ref" {
                    if let Some(reference) = inner.as_str() {
                        names.push(super::ref_name(reference));
                        continue;
                    }
                }
                collect_reference_names(inner, names);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_reference_names(item, names);
            }
        }
        _ => {}
    }
}

/// 构造器名封闭集判据（精确 `new` / `default` / `build` / `empty` / `never` / `open`，
/// 前缀 `from_` / `with_`）。契约校验与 `apiConstructorDeclared` 判据共用同一函数，
/// MUST NOT 各写一份。
pub fn is_constructor(op_id: &str) -> bool {
    CONSTRUCTOR_NAMES.contains(&op_id)
        || CONSTRUCTOR_PREFIXES
            .iter()
            .any(|prefix| op_id.starts_with(prefix))
}

/// 契约锚点期望值：文件名去 `.md`（无 `.md` 后缀时退回 FQN 入参）。
fn file_title(file: &str, fqn: &str) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    match name.strip_suffix(".md") {
        Some(stem) if !stem.is_empty() => stem.to_owned(),
        _ => fqn.to_owned(),
    }
}

/// fence info string → token 列表（空格 / 逗号分隔；`json api-op` 与 `api-op` 同判）。
fn info_tokens(info: &str) -> Vec<&str> {
    info.split(|character: char| character.is_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .collect()
}

/// 对象层级键唯一性判据（D37）：返回首个重复键与其在文本内的行偏移（0 起始）。
///
/// **这是「同名两处定义即契约错误」的唯一可判处**：`serde_json::from_str` 在解析阶段
/// 把重复键**折叠**（后者胜、前者消失），故 `schema::verify_defs` 的 `&Value` 入参
/// 结构上不可达该形态。判据因此必须落在文本层，且 MUST 与后续解析用**同一份文本**
/// （先 `from_str` 再比对已无意义）。
///
/// 扫描只认**对象**的键（数组元素无身份）；键按 JSON 字符串语义解码后比较，故
/// `"\u0041"` 与 `"A"` 是同一个键。
fn duplicate_keys(text: &str) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut frames: Vec<Frame> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                frames.push(Frame {
                    keys: Some(Vec::new()),
                    expect_key: true,
                });
                index += 1;
            }
            b'[' => {
                frames.push(Frame {
                    keys: None,
                    expect_key: false,
                });
                index += 1;
            }
            b'}' | b']' => {
                frames.pop();
                index += 1;
            }
            b',' => {
                if let Some(frame) = frames.last_mut().filter(|frame| frame.keys.is_some()) {
                    frame.expect_key = true;
                }
                index += 1;
            }
            b'"' => {
                let start = index;
                // 未闭合的字符串在本层不判（结构错误由后续 `serde_json` 报），直接收工
                let end = string_end(bytes, start)?;
                if let Some(frame) = frames.last_mut().filter(|frame| frame.expect_key) {
                    frame.expect_key = false;
                    let key = decode_key(&text[start + 1..end - 1]);
                    if let Some(keys) = frame.keys.as_mut() {
                        if keys.iter().any(|seen| seen == &key) {
                            return Some((key, text[..start].matches('\n').count()));
                        }
                        keys.push(key);
                    }
                }
                index = end;
            }
            _ => index += 1,
        }
    }
    None
}

/// 键唯一性扫描的层级帧（`keys` 为 `None` 即数组帧——数组元素无身份）。
struct Frame {
    /// 对象帧携带已见键；数组帧为 `None`
    keys: Option<Vec<String>>,
    /// 下一个字符串 token 是否是键（`{` / `,` 之后为真，读入即转假）
    expect_key: bool,
}

/// 字符串 token 的结束位置（`start` 处是开引号；返回闭引号之后的下标；未闭合 → `None`）。
///
/// 未闭合不是本层的判定对象：那属结构错误，由 `serde_json` 报并自带行号——本函数因此
/// MUST NOT 猜测边界（猜错即让键比较越界切片）。
fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index + 1),
            _ => index += 1,
        }
    }
    None
}

/// JSON 字符串字面量（已去引号）→ 解码后的键（只处理键比较所需的转义面）。
fn decode_key(raw: &str) -> String {
    if !raw.contains('\\') {
        return raw.to_owned();
    }
    let mut decoded = String::new();
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => decoded.push('\n'),
            Some('t') => decoded.push('\t'),
            Some('r') => decoded.push('\r'),
            Some('u') => {
                let hex: String = characters.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(unit) => decoded.push(unit),
                    None => {
                        decoded.push('\\');
                        decoded.push('u');
                        decoded.push_str(&hex);
                    }
                }
            }
            Some(other) => decoded.push(other),
            None => decoded.push('\\'),
        }
    }
    decoded
}

/// 重复键 → 契约错误（`file:line` 与既有解析错误同款前缀；`offset` 是键在 fence 内的
/// 行偏移，故行号 = fence 起始行 + 偏移 + 1）。
fn duplicate_key_error(file: &str, line: u32, fence: &str, key: &str, offset: usize) -> ApiError {
    ApiError::Contract(format!(
        "{file}:{}: {fence} 内键 `{key}` 重复声明（$defs 的键名即身份，同名即定义两次；MUST NOT 依赖 serde_json 的后者胜折叠）",
        line.saturating_add(offset as u32).saturating_add(1)
    ))
}

/// fence 内容 → JSON 对象（`api-types` / `api-reexport` 的形态要求）。
fn parse_object(
    body: &str,
    file: &str,
    line: u32,
    fence: &str,
) -> Result<Map<String, Value>, ApiError> {
    if let Some((key, offset)) = duplicate_keys(body) {
        return Err(duplicate_key_error(file, line, fence, &key, offset));
    }
    let value: Value = serde_json::from_str(body).map_err(|error| {
        ApiError::Contract(format!("{file}:{line}: {fence} 内容不是合法 JSON：{error}"))
    })?;
    match value {
        Value::Object(map) => Ok(map),
        other => Err(ApiError::Contract(format!(
            "{file}:{line}: {fence} 内容必须是 JSON 对象（键 = 名字，值 = 定义），实际为 {}",
            kind_of(&other)
        ))),
    }
}

/// fence 内容 → JSON 值（`api-op` 允许任意原生 JSON Schema 形态，但必须是对象或
/// 布尔 schema；非 JSON 即错误）。
fn parse_object_value(body: &str, file: &str, line: u32, fence: &str) -> Result<Value, ApiError> {
    if let Some((key, offset)) = duplicate_keys(body) {
        return Err(duplicate_key_error(file, line, fence, &key, offset));
    }
    let value: Value = serde_json::from_str(body).map_err(|error| {
        ApiError::Contract(format!("{file}:{line}: {fence} 内容不是合法 JSON：{error}"))
    })?;
    match value {
        Value::Object(_) | Value::Bool(_) => Ok(value),
        other => Err(ApiError::Contract(format!(
            "{file}:{line}: {fence} 内容必须是 JSON 对象，实际为 {}",
            kind_of(&other)
        ))),
    }
}

/// 散文区的透传列表：含 CJK 的行不算（那是解释句），列表项的**首个 ASCII 标识符**
/// 算一个导出名（可带反引号）。零 `api-reexport` 且零透传段时两侧皆空。
fn collect_reexport_prose(sections: &[ContractSection]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for section in sections {
        for line in section.body.lines() {
            let item = line
                .trim_start()
                .strip_prefix("- ")
                .or_else(|| line.trim_start().strip_prefix("* "));
            let Some(item) = item else {
                continue;
            };
            if item.chars().any(is_cjk) {
                continue;
            }
            let token = item
                .trim_start_matches('`')
                .split(|character: char| {
                    character.is_whitespace()
                        || character == '`'
                        || character == ':'
                        || character == '：'
                        || character == ','
                        || character == '，'
                })
                .find(|token| !token.is_empty())
                .unwrap_or("");
            if is_identifier(token) && !names.iter().any(|name| name == token) {
                names.push(token.to_owned());
            }
        }
    }
    names
}

/// ASCII 标识符判据（`[A-Za-z_][A-Za-z0-9_]*`）。
fn is_identifier(token: &str) -> bool {
    let mut characters = token.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// CJK 字符判据（散文解释句与导出清单的机械分界）。
fn is_cjk(character: char) -> bool {
    matches!(character, '\u{3000}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{FF00}'..='\u{FFEF}')
}

/// 收束当前散文段（后续 heading 或文件结束）：空段（无正文）不入 `sections`。
fn finish_section(section: &mut Option<ContractSection>, sections: &mut Vec<ContractSection>) {
    if let Some(closed) = section.take() {
        if !closed.body.trim().is_empty() {
            sections.push(closed);
        }
    }
}
