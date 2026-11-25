//! `Contract` 投影与复用三闸：接口真相是**手写的 `<FQN>.schema.json`**，`Contract` 只是它的进程内
//! 投影（[`Contract::from_schema`] 解包 `x-function`、`$defs` 二分；无失败面——调用它时已跑过
//! ③–⑧ 闸）。**[`verify`] 是写闸与装载期共用的复用面**（七族名字 / 落点 / 自洽三闸），文本层重复键
//! 另由 [`duplicate_keys`] 判。三闸同趟收口 ⇒ 渲染期零标识符判据；错误带 `file` 前缀与（可能的）
//! 行号——接口文件坏了要能直接改到那一处。

use serde_json::{Map, Value};

use super::{kind_of, ApiError};

/// 函数标记层关键字（D47 落点闸判的对象；`schema` 的形态闸与 `dto` 的键名摘要同调本常量）。
pub(crate) const X_FUNCTION: &str = "x-function";

/// 构造器名的精确匹配集与前缀集（契约 MUST NOT 声明构造器——构造器由生成器自动产出）。
const CONSTRUCTOR_NAMES: [&str; 6] = ["new", "default", "build", "empty", "never", "open"];
const CONSTRUCTOR_PREFIXES: [&str; 2] = ["from_", "with_"];

/// 一份接口定义的内存投影（手写真相的读面形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    /// 元素 FQN（= `title`，与文件名同源）
    pub fqn: String,
    /// `title` 原文
    pub title: String,
    /// 接口文件（repo 相对 POSIX，错误文案与报告用）
    pub file: String,
    /// 操作清单（`properties` 声明序）
    pub ops: Vec<ContractOp>,
    /// 本模块自有 `$defs` 定义（非纯 `$ref` alias，声明序）
    pub types: Map<String, Value>,
    /// 透传清单（纯 `$ref` alias 且目标可反查：键 = 导出名、值 = 直接子元素 FQN）
    pub reexports: Map<String, Value>,
}

/// 一个操作 + 其签名（`properties.<op>.x-function` 解包后的值）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractOp {
    /// 操作 id（`properties` 的键）
    pub id: String,
    /// 声明行（schema 无行号 ⇒ 恒 `0`，即「位置未知」口径）
    pub line: u32,
    /// `x-function` 解包后的原文（不含 `description`）
    pub signature: Value,
}

impl Contract {
    /// schema → `Contract` 投影（零失败面：调用方已跑过 ③–⑧ 闸）：`types` = `$defs` 里非纯
    /// alias 的条目；`reexports` = 纯 alias 且文件主干可经 [`super::reexport_target`] 反查出目标
    /// FQN 的条目（**本地 alias 不入 `reexports`**——它不是透传）。`description` 不进投影。
    pub fn from_schema(fqn: &str, file: &str, schema: &Value) -> Contract {
        let title_text = schema.get("title").and_then(Value::as_str);
        let title = title_text.unwrap_or(fqn).to_owned();
        let mut ops: Vec<ContractOp> = Vec::new();
        let declared = schema.get("properties").and_then(Value::as_object);
        for (id, property) in declared.into_iter().flatten() {
            let Some(function) = property.get(X_FUNCTION) else {
                continue;
            };
            let mut signature = function.clone();
            if let Some(object) = signature.as_object_mut() {
                object.remove("description");
            }
            ops.push(ContractOp {
                id: id.clone(),
                line: 0,
                signature,
            });
        }
        let mut types: Map<String, Value> = Map::new();
        let mut reexports: Map<String, Value> = Map::new();
        for (name, definition) in defs(schema) {
            if super::check::is_pure_ref_alias(definition) {
                let target = definition
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(super::reexport_target);
                if let Some(target) = target {
                    reexports.insert(name.clone(), Value::String(target));
                }
                continue;
            }
            types.insert(name.clone(), definition.clone());
        }
        Contract {
            fqn: fqn.to_owned(),
            title,
            file: file.to_owned(),
            ops,
            types,
            reexports,
        }
    }
}

/// `schema` 的 `$defs` 表（缺省 / 非对象 ⇒ 空迭代）。
fn defs(schema: &Value) -> impl Iterator<Item = (&String, &Value)> {
    let defs = schema.get("$defs").and_then(Value::as_object);
    defs.into_iter().flatten()
}

/// 校验的复用面（写闸与装载期同趟调用，见 [`super::schema::validate_schema`]）。
///
/// **七族名字闸**（D45，语言无关）：名字 MUST 是 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*` ——
/// ① `properties` 的 op 键；② `$defs` 键名；③ `x-function` 的 `params[].name`（模块操作与
/// `$defs` 的方法同判）；④ `$defs` 条目的 `properties` 键；⑤ `owner` 键；⑥ 所有 `$ref` 的
/// 末段名；⑦ 透传导出名的键（由 ② 覆盖）。名字类的错是**接口文件的错**，须在写盘前 / 装载期
/// 中止——渲染期 MUST NOT 重复校验同一事实（「缺名落 `arg`」的兜底属渲染期）。
///
/// 同趟续判**落点闸**（D47）与**自洽闸**（`required` ⊆ `properties`），二者判集合关系与键位置、
/// 不占族别编号。**各族分趟扫、MUST NOT 合并成单趟**——族别与遍历序即错误面。
pub fn verify(contract: &Contract, schema: &Value) -> Result<(), ApiError> {
    for op in &contract.ops {
        if !is_identifier(&op.id) {
            return Err(identifier_error(contract, "op id", &op.id, None));
        }
        for name in function_param_names(&op.signature) {
            if !is_identifier(&name) {
                return Err(identifier_error(contract, "参数名", &name, None));
            }
        }
        if let Some(owner) = op.signature.get("owner").and_then(Value::as_str) {
            if !is_identifier(owner) {
                return Err(identifier_error(contract, "owner 键", owner, None));
            }
        }
        for name in reference_names(&op.signature) {
            if !is_identifier(&name) {
                return Err(identifier_error(contract, "$ref 末段名", &name, None));
            }
        }
    }
    for (name, definition) in defs(schema) {
        if !is_identifier(name) {
            return Err(identifier_error(contract, "$defs 键名", name, None));
        }
        let declared = definition.get("properties").and_then(Value::as_object);
        let fields: Vec<(&String, &Value)> = declared.into_iter().flatten().collect();
        // ④ 族：数据字段键（带 `x-function` 的项是方法名，归 ③ 族，下一趟判）
        for (field, property) in &fields {
            if property.get(X_FUNCTION).is_none() && !is_identifier(field) {
                let pointer = format!("$defs.{name}.properties.{field}");
                return Err(identifier_error(
                    contract,
                    "properties 数据字段键",
                    field,
                    Some(&pointer),
                ));
            }
        }
        // 自洽闸：`required` 元素 MUST 在同条目 `properties` 里（非字符串元素也点名形态）
        let required = definition.get("required").and_then(Value::as_array);
        for (index, element) in required.into_iter().flatten().enumerate() {
            let present = element
                .as_str()
                .is_some_and(|key| fields.iter().any(|(field, _)| field.as_str() == key));
            if !present {
                return Err(required_dangling_error(contract, name, index, element));
            }
        }
        // ③ 族：方法名（`x-class` 的 trait 方法与函数指针字段同形）+ 其 `params` / `owner`
        for (method, function) in methods_of(definition) {
            if !is_identifier(&method) {
                return Err(identifier_error(contract, "方法名", &method, None));
            }
            for name in function_param_names(function) {
                if !is_identifier(&name) {
                    return Err(identifier_error(contract, "参数名", &name, None));
                }
            }
            if let Some(owner) = function.get("owner").and_then(Value::as_str) {
                if !is_identifier(owner) {
                    return Err(identifier_error(contract, "owner 键", owner, None));
                }
            }
        }
        for name in reference_names(definition) {
            if !is_identifier(&name) {
                return Err(identifier_error(contract, "$ref 末段名", &name, None));
            }
        }
    }
    for name in contract.reexports.keys() {
        if !is_identifier(name) {
            return Err(identifier_error(contract, "透传导出名", name, None));
        }
    }
    verify_x_function_positions(contract, schema)
}

/// `$defs` 条目里带 `x-function` 的 `properties` 项（trait 方法与函数指针字段同形；返回
/// （方法名，该 `x-function` 值））——③ 族的收集面唯一入口。
fn methods_of(definition: &Value) -> Vec<(String, &Value)> {
    let fields = definition.get("properties").and_then(Value::as_object);
    fields
        .into_iter()
        .flatten()
        .filter_map(|(method, property)| {
            let function = property.get(X_FUNCTION)?;
            Some((method.clone(), function))
        })
        .collect()
}

/// 落点闸（D47）：`x-function` 的合法落点**封闭二处**——① `properties.<k>` 的值（模块操作）；
/// ② `$defs` 条目**第一层** `properties` 的**值**（方法 / 函数指针字段）。判「键在哪儿」而非
/// 「名字长什么样」，故不是名字闸的第八族；只有本函数同时持有 `Contract` 与 schema 的整体视角，
/// **MUST NOT** 落 [`super::schema::verify_defs`]。
fn verify_x_function_positions(contract: &Contract, schema: &Value) -> Result<(), ApiError> {
    for op in &contract.ops {
        // 投影已解包 ⇒ 这里从 `x-function` 的**内层**起扫：本节点上的 `x-function`
        // 已不合法（函数值内部不得再有函数作为值）。
        let pointer = format!("properties.{}", op.id);
        scan_x_function(&op.signature, &pointer, false, contract)?;
    }
    for (name, definition) in defs(schema) {
        let pointer = format!("$defs.{name}");
        let Some(object) = definition.as_object() else {
            continue;
        };
        // `$defs` 条目**自身顶层**的 `x-function` 非法：函数签名无从「按名见证」，且会让类型面
        // 凭空多出一个无名字的条目。
        if object.contains_key(X_FUNCTION) {
            return Err(x_function_position_error(contract, &pointer));
        }
        for (key, value) in object {
            if key == "properties" {
                if let Some(fields) = value.as_object() {
                    for (field, property) in fields {
                        let pointer = format!("{pointer}.properties.{field}");
                        scan_x_function(property, &pointer, true, contract)?;
                    }
                    continue;
                }
            }
            scan_x_function(value, &format!("{pointer}.{key}"), false, contract)?;
        }
    }
    Ok(())
}

/// 落点闸的递归主体（逐层携带 pointer 前缀）：`allowed` 为真表示本节点上的 `x-function` 键合法，
/// 其子树一律转假。
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
                    let nested = format!("{pointer}.{X_FUNCTION}");
                    scan_x_function(inner, &nested, false, contract)?;
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

/// 落点闸的错误面：**定形族别串** `x-function 落点`（判据锚它，MUST NOT 靠散文近似措辞）+ **JSON pointer**。
fn x_function_position_error(contract: &Contract, pointer: &str) -> ApiError {
    ApiError::Contract(format!(
        "{}: x-function 落点非法（JSON pointer `{pointer}`）——`{X_FUNCTION}` 只许出现在 \
         `properties.<k>` 的值与 `$defs` 条目第一层 `properties` 的值二处；\
         其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 的元素内、\
         `properties` 的第二层及更深、`$defs` 条目自身顶层、`x-function` 自身的子树）\
         都表达不了「函数作为具名值」：请去掉标记层，或把该形状提升为具名 `$defs` 条目",
        contract.file
    ))
}

/// 名字闸的错误面（族别 + 名字 + 可执行出路）。`pointer` 只在「properties 数据字段键」族传入
/// （唯一深处的族），其余族 MUST NOT 传 `Some`。
fn identifier_error(
    contract: &Contract,
    family: &str,
    name: &str,
    pointer: Option<&str>,
) -> ApiError {
    let detail = match pointer {
        Some(pointer) => format!(
            "JSON pointer `{pointer}`——属性键必须是标识符；`data-testid` 一类键不进接口定义\
             （前端契约需避开 DOM 测试属性）；人类可读的措辞请写在 `description`"
        ),
        None => "接口定义里的名字会直接成为目标语言源码的标识符：MUST NOT 含空格 / 连字符 / \
                 点 / 首字符数字；需要人类可读的措辞请写在 `description`"
            .to_owned(),
    };
    ApiError::Contract(format!(
        "{}: {family} `{name}` 不是 ASCII 标识符（`[A-Za-z_][A-Za-z0-9_]*`）；{detail}",
        contract.file
    ))
}

/// 一段 `x-function` 形态值里的 `params[].name`（包裹形态与解包形态同判；缺名不产出条目）。
fn function_param_names(signature: &Value) -> Vec<String> {
    let function = signature.get(X_FUNCTION).unwrap_or(signature);
    let params = function.get("params").and_then(Value::as_array);
    params
        .into_iter()
        .flatten()
        .filter_map(|param| param.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// 自洽闸的错误面：点名 JSON pointer、该元素（非字符串经 [`kind_of`] 报形态）与可执行出路。
fn required_dangling_error(
    contract: &Contract,
    definition: &str,
    index: usize,
    element: &Value,
) -> ApiError {
    let element = match element.as_str() {
        Some(key) => format!("`{key}`"),
        None => format!("（{}）", kind_of(element)),
    };
    ApiError::Contract(format!(
        "{}: required 元素在同条目 `properties` 里没有对应键——该元素 = {element}\
         （JSON pointer `$defs.{definition}.required[{index}]`）；`required` 与 `properties` \
         必须闭合：改成 `properties` 里已有的键，或补上该字段的定义\
         （悬垂会让两侧产物把该字段静默渲染成可选）",
        contract.file
    ))
}

/// 一棵 JSON 子树内所有 `$ref` 的**末段名**（⑥ 族；切分走 [`super::ref_name`]，MUST NOT 自写第二份）。
fn reference_names(value: &Value) -> Vec<String> {
    fn walk(value: &Value, names: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                for (key, inner) in object {
                    if key == "$ref" {
                        if let Some(reference) = inner.as_str() {
                            names.push(super::ref_name(reference));
                            continue;
                        }
                    }
                    walk(inner, names);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, names);
                }
            }
            _ => {}
        }
    }
    let mut names: Vec<String> = Vec::new();
    walk(value, &mut names);
    names
}

/// 构造器名封闭集判据（接口校验与 `apiConstructorDeclared` 判据共用同一函数，MUST NOT 各写一份）。
pub fn is_constructor(op_id: &str) -> bool {
    CONSTRUCTOR_NAMES.contains(&op_id)
        || CONSTRUCTOR_PREFIXES
            .iter()
            .any(|prefix| op_id.starts_with(prefix))
}

/// 对象层级键唯一性判据（D37）：返回首个重复键与其在文本内的行偏移（0 起始）。
/// **这是「同名两处定义即接口错误」的唯一可判处**：`serde_json::from_str` 把重复键**折叠**
/// （后者胜），故 `&Value` 面不可达该形态——判据须落在文本层且与解析用**同一份文本**；扫描只认
/// **对象**的键，键按 JSON 字符串语义解码后比较。
pub(crate) fn duplicate_keys(text: &str) -> Option<(String, usize)> {
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
    /// 下一个字符串 token 是否是键（`{` / `,` 之后为真）
    expect_key: bool,
}

/// 字符串 token 的结束位置（`start` 处是开引号；未闭合 → `None`——那属结构错误、由 `serde_json` 报）。
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
                    None => decoded.push_str(&format!("\\u{hex}")),
                }
            }
            Some(other) => decoded.push(other),
            None => decoded.push('\\'),
        }
    }
    decoded
}

/// 重复键 → 接口错误（`file:line` 与既有错误同款前缀；`offset` 是键在文本内的行偏移）。
pub(crate) fn duplicate_key_error(file: &str, key: &str, offset: usize) -> ApiError {
    ApiError::Contract(format!(
        "{file}:{}: 键 `{key}` 重复声明（`$defs` 的键名即身份，同名即定义两次；\
         MUST NOT 依赖 serde_json 的后者胜折叠）——删掉其中一处",
        offset + 1
    ))
}

/// ASCII 标识符判据（`[A-Za-z_][A-Za-z0-9_]*`）：**七族名字闸的唯一判据**，也是 `archi_api` 的 `link`
/// 动作对导出名的校验单点（MUST NOT 各写一份）。
pub fn is_identifier(token: &str) -> bool {
    let mut characters = token.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}
