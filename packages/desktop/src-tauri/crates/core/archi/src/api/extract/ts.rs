//! TS / JS 声明抽取（`oxc`，与 `scan/ts.rs` 同 AST 家族、零新依赖）。
//!
//! **取数面 = 导出面**（与 Rust 侧同一条纪律）：只有带 `export` 关键字的声明才是 API
//! 面。无 `export` 的顶层 `function` / `class` / `const` / `type` / `interface` / `enum`
//! 一律不产出符号——它们对元素外不可达，混进导出集会让判据（`apiMissing` /
//! `apiUndeclared`）把私有符号当成公开承诺。
//!
//! 关键实测结论决定了抽取面：`export const` 里**零个箭头函数**（0/132），故**只有
//! `export function` 是操作**——MUST NOT 为「从 const 初始化表达式推断函数」造启发式，
//! 箭头函数初始化式**不产出符号**。`export const = memo(...)` / `forwardRef(...)` /
//! `lazy(...)` 是组件（本期 `api = none` 留白）；对象字面量与 `new Proxy(...)` 是值、
//! 不是 API 面（排除）；`export type` / `export interface` / `export class` 是类型、
//! `export enum` 是枚举；`export {…}` / `export * from` 是透传（本期不产出操作）。
//!
//! `export default` 形态**本期不抽**：对外名字由导入方决定（`import X from` 可任意
//! 命名），契约的 op / 类型名须是稳定标识符——从文件名或表达式反推名字即造第二套命名
//! 规则。
//!
//! **`unused` 本期生产面不产出（未接线）**：降级接口 [`apply_knip_report`] 已实现但
//! **零产品调用者**（`archi-api-gen` 与 `api::bootstrap` 都不读 knip，见待决 5 / 12）——
//! 这是**显式留白**，不是疏漏：`ts_test` 的五条用例测的是接口正确性，不是接线状态。
//! 接线是独立变更。降级判据只此一条来源，MUST NOT 为 TS 造第二套死代码扫描。
//!
//! **`visibility` 的本期值域与留白**：本文件只产出 `External`（有 `export`）；
//! `Unused` 由 [`apply_knip_report`] 单一来源降级。`crateOnly` / `testOnly` 两值在 TS
//! 侧**本期留白**——两者的判据都需要跨文件 import 反向事实（「谁引用了这个符号」），而
//! 本取数器只吃单文件 AST（`extract_file(file, source)`），结构上拿不到。将来补齐所需
//! 的输入是 `scan` 的 `ImportFact`（specifier + `typeOnly`）与 knip 报告的 files 条目。
//! **文件名族不得作 proxy**：`.test.` / `.spec.` / `__tests__` / `fixtures` 与引用关系
//! 无必然联系（`foo.test.ts` 可以只被生产代码 import，`foo.ts` 也可只被测试 import），
//! 用文件名冒充 `testOnly` 是伪判据，不是近似。
//!
//! 边界：纯读源文本 + AST，零读盘、零写盘、零 spawn。

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Class, Declaration, Expression, Function, Statement, TSType, TSTypeAnnotation,
    TSTypeName, VariableDeclaration, VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;

use super::{ApiParam, ApiSymbol, ApiSymbolKind, ApiType, Visibility};

/// 单文件抽取（语法错 → 零符号；不猜测、不 panic）。
///
/// 只走 `export <声明>` 一条分支：非导出的顶层声明与 `export default` 均不产出符号。
pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type_of(file)).parse();
    if parsed.panicked {
        return Vec::new();
    }
    let mut symbols = Vec::new();
    for statement in &parsed.program.body {
        if let Some(declaration) = exported_declaration(statement) {
            collect_declaration(declaration, source, &mut symbols);
        }
    }
    symbols
}

/// `export <声明>` → 其声明（**取数面的唯一入口**）：非导出顶层声明与 `export default`
/// 一律返回 `None`，不产出符号。
fn exported_declaration<'a, 'b>(statement: &'b Statement<'a>) -> Option<&'b Declaration<'a>> {
    match statement {
        Statement::ExportDeclaration(export) => Some(&export.declaration),
        _ => None,
    }
}

/// 把 knip 报的未使用导出降级为 [`Visibility::Unused`]（`None` 即不降级——不造第二套
/// 死代码扫描）。
pub fn apply_knip_report(symbols: &mut [ApiSymbol], report: Option<&Value>) {
    let Some(report) = report else {
        return;
    };
    let unused = collect_unused(report);
    if unused.is_empty() {
        return;
    }
    for symbol in symbols.iter_mut() {
        if unused.iter().any(|name| name == &symbol.name) {
            symbol.visibility = Visibility::Unused;
        }
    }
}

/// knip JSON → 未使用导出名清单（`files.<path>.exports[].name` 形态；字段名兼容性留给
/// 首个 TS bootstrap 落地的变更确认）。
fn collect_unused(report: &Value) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let Some(files) = report.get("files").and_then(Value::as_object) else {
        return names;
    };
    for file in files.values() {
        let Some(exports) = file.get("exports").and_then(Value::as_array) else {
            continue;
        };
        for entry in exports {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| entry.as_str());
            if let Some(name) = name {
                if !names.iter().any(|existing| existing == name) {
                    names.push(name.to_owned());
                }
            }
        }
    }
    names
}

/// `export <声明>` 的内部形态分派（导出面的唯一收口处）。
fn collect_declaration(declaration: &Declaration<'_>, source: &str, out: &mut Vec<ApiSymbol>) {
    match declaration {
        Declaration::FunctionDeclaration(function) => push_function(function, None, source, out),
        Declaration::ClassDeclaration(class) => push_type(
            class_name(class),
            ApiSymbolKind::Type,
            class_line(class, source),
            out,
        ),
        Declaration::VariableDeclaration(variable) => extract_const(variable, source, out),
        Declaration::TSTypeAliasDeclaration(alias) => push_type(
            alias.id.name.to_string(),
            ApiSymbolKind::Type,
            line_of(source, alias.span.start as usize),
            out,
        ),
        Declaration::TSInterfaceDeclaration(interface) => push_type(
            interface.id.name.to_string(),
            ApiSymbolKind::Type,
            line_of(source, interface.span.start as usize),
            out,
        ),
        Declaration::TSEnumDeclaration(item) => push_type(
            item.id.name.to_string(),
            ApiSymbolKind::Enum,
            line_of(source, item.span.start as usize),
            out,
        ),
        _ => {}
    }
}

/// `export const` 按初始化表达式分类：`memo(...)` / `forwardRef(...)` / `lazy(...)` →
/// 组件留白（`Type` 形态，`api = none` 的元素不进契约）；对象字面量与 `new Proxy(...)`
/// → 排除（是值不是 API 面）；箭头函数 → **不产出符号**（不造「从初始化表达式推断函数」
/// 的启发式）。其余调用形态按未知形态的导出值入表，由契约侧决定是否承诺。
fn extract_const(declaration: &VariableDeclaration<'_>, source: &str, out: &mut Vec<ApiSymbol>) {
    if declaration.kind != VariableDeclarationKind::Const {
        return;
    }
    for declarator in &declaration.declarations {
        let Some(name) = binding_name(&declarator.id) else {
            continue;
        };
        let Some(init) = &declarator.init else {
            continue;
        };
        let line = line_of(source, declarator.span.start as usize);
        match init {
            Expression::CallExpression(call) => {
                let callee = match &call.callee {
                    Expression::Identifier(identifier) => Some(identifier.name.to_string()),
                    Expression::StaticMemberExpression(member) => {
                        Some(member.property.name.to_string())
                    }
                    _ => None,
                };
                let marker = match callee.as_deref() {
                    Some("memo" | "forwardRef" | "lazy") => "component",
                    _ => "call",
                };
                push_type(name, ApiSymbolKind::Type, line, out);
                if let Some(last) = out.last_mut() {
                    last.returns = ApiType::Unknown(marker.to_owned());
                }
            }
            // 对象字面量 / 构造调用：不是 API 面符号（排除）
            Expression::ObjectExpression(_) | Expression::NewExpression(_) => {}
            _ => {}
        }
    }
}

/// `export function` → 函数符号（**唯一操作判定**）。
fn push_function(
    function: &Function<'_>,
    owner: Option<&str>,
    source: &str,
    out: &mut Vec<ApiSymbol>,
) {
    let Some(identifier) = &function.id else {
        return;
    };
    let params = function
        .params
        .items
        .iter()
        .map(|item| ApiParam {
            name: binding_name(&item.pattern).unwrap_or_else(|| "arg".to_owned()),
            ty: item
                .type_annotation
                .as_deref()
                .map(convert_annotation)
                .unwrap_or_else(|| ApiType::Unknown(String::new())),
            optional: item.optional || item.initializer.is_some(),
        })
        .collect();
    out.push(ApiSymbol {
        name: identifier.name.to_string(),
        owner: owner.map(str::to_owned),
        kind: ApiSymbolKind::Function,
        params,
        returns: function
            .return_type
            .as_deref()
            .map(convert_annotation)
            .unwrap_or_else(|| ApiType::Unknown(String::new())),
        is_async: function.r#async,
        line: line_of(source, function.span.start as usize),
        visibility: Visibility::External,
    });
}

/// 类型类符号入表（零参数零返回，供 bootstrap 起草时区分类型与操作）。
fn push_type(name: String, kind: ApiSymbolKind, line: u32, out: &mut Vec<ApiSymbol>) {
    out.push(ApiSymbol {
        name,
        owner: None,
        kind,
        params: Vec::new(),
        returns: ApiType::Unknown(String::new()),
        is_async: false,
        line,
        visibility: Visibility::External,
    });
}

/// 类名（匿名类退化为空串——仍如实入表，不猜名字）。
fn class_name(class: &Class<'_>) -> String {
    class
        .id
        .as_ref()
        .map(|identifier| identifier.name.to_string())
        .unwrap_or_default()
}

/// 类声明行号。
fn class_line(class: &Class<'_>, source: &str) -> u32 {
    line_of(source, class.span.start as usize)
}

/// 绑定模式的标识符名（解构 / 复杂模式 → `None`，调用方给占位名）。
fn binding_name(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

/// TS 类型注解 → 逻辑类型。
fn convert_annotation(annotation: &TSTypeAnnotation<'_>) -> ApiType {
    convert_type(&annotation.type_annotation)
}

/// TS 类型 → 逻辑类型（无法归类即 `Unknown`）。
fn convert_type(ty: &TSType<'_>) -> ApiType {
    match ty {
        TSType::TSStringKeyword(_) => ApiType::Str,
        TSType::TSNumberKeyword(_) => ApiType::Float,
        TSType::TSBooleanKeyword(_) => ApiType::Bool,
        TSType::TSArrayType(array) => ApiType::List(Box::new(convert_type(&array.element_type))),
        TSType::TSTypeReference(reference) => {
            let name = type_name(&reference.type_name);
            match name.as_str() {
                "Array" | "ReadonlyArray" => {
                    ApiType::List(Box::new(generic_argument(reference, 0)))
                }
                "Record" => ApiType::Map(Box::new(generic_argument(reference, 1))),
                "Promise" => ApiType::Outcome(Box::new(generic_argument(reference, 0))),
                _ => ApiType::Named(name),
            }
        }
        TSType::TSUnionType(union) => {
            let has_null = union.types.iter().any(|member| {
                matches!(
                    member,
                    TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_)
                )
            });
            let inner = union
                .types
                .iter()
                .find(|member| {
                    !matches!(
                        member,
                        TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_)
                    )
                })
                .map(convert_type)
                .unwrap_or_else(|| ApiType::Unknown(String::new()));
            if has_null {
                ApiType::Optional(Box::new(inner))
            } else {
                inner
            }
        }
        _ => ApiType::Unknown(String::new()),
    }
}

/// `TSTypeName` → 类型名（限定名取末段）。
fn type_name(name: &TSTypeName<'_>) -> String {
    match name {
        TSTypeName::IdentifierReference(identifier) => identifier.name.to_string(),
        TSTypeName::QualifiedName(qualified) => qualified.right.name.to_string(),
        TSTypeName::ThisExpression(_) => "this".to_owned(),
    }
}

/// 泛型实参里第 `index` 个类型（不可判 → `Unknown`）。
fn generic_argument(reference: &oxc_ast::ast::TSTypeReference<'_>, index: usize) -> ApiType {
    reference
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.get(index))
        .map(convert_type)
        .unwrap_or_else(|| ApiType::Unknown(String::new()))
}

/// 源文件扩展名 → `SourceType`（`.tsx` / `.jsx` 开 JSX）。
fn source_type_of(file: &str) -> SourceType {
    let extension = file.rsplit_once('.').map(|(_, tail)| tail).unwrap_or("ts");
    match extension {
        "tsx" => SourceType::tsx(),
        "jsx" => SourceType::jsx(),
        "js" | "mjs" => SourceType::mjs(),
        _ => SourceType::ts(),
    }
}

/// 字节偏移 → 行号（1 起始；`char` 边界安全回退）。
fn line_of(source: &str, byte: usize) -> u32 {
    let mut end = byte.min(source.len());
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    1 + source[..end].matches('\n').count() as u32
}
