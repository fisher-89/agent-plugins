//! `api::emit::ts`（`.i.ts` 渲染）的单元测试：七种协议映射（`object` →
//! `export interface`、`enum` → `as const` + 同名 `type`、`x-class` → `interface` +
//! 降级注释、标量 → 别名（不记 degraded）、`oneOf` / `anyOf` → union、`x-function` →
//! `(…) => R` / `Promise<R>`、非 `required` → `T | null`）、透传 `export type { … } from`、
//! 头注释、ambient-free 与无品牌类型、幂等与格式化器不动点（一律单引号）。
//!
//! 零 mock：真实 `contract::parse` + `schema::build_schema`；`paths::ts_import_specifier`
//! 真实调用；本文件零 `eprintln!`（警告单点在 `emit/mod.rs`）——以源码自证表达。

use std::collections::BTreeMap;

use serde_json::Value;

use crate::api::contract::{parse, Contract};

use super::ts::{render, ts_comment_text, ts_string_literal};
use super::{EmitTarget, ModuleRef};
use crate::api::paths::{self, EntryForm};
use crate::api::schema::build_schema;
use crate::api::ApiMeta;

/// 契约文件（repo 相对 POSIX）。
const FILE: &str = ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md";

/// 元素 FQN。
const FQN: &str = "App.Backend.Infra.ArchiRuntime";

/// 载元（真实值；`tool_version` 取编译期常量）。
fn meta() -> ApiMeta {
    crate::api::meta()
}

/// 解析 + 装配（失败即断言失败）。
fn prepare(source: &str) -> (Contract, Value) {
    let contract = parse(source, FQN, FILE).expect("契约应解析成功");
    let schema = build_schema(&contract, &meta()).expect("schema 应装配成功");
    (contract, schema)
}

/// 渲染（目录形态的 root；失败即断言失败）。
///
/// 「空 scope」由**入参**表达（D46）：空查表 + 目录形态的 `EmitTarget`。
fn render_source(source: &str) -> String {
    let (contract, schema) = prepare(source);
    render(&contract, &schema, &meta(), &empty_scope())
}

/// 空 scope 的 `EmitTarget`（空查表 + 目录形态；D46 的退化形态由入参表达）。
fn empty_scope() -> EmitTarget {
    EmitTarget {
        kind: crate::api::paths::GeneratedKind::TypeScript,
        form: EntryForm::SubdirModule,
        modules: BTreeMap::new(),
    }
}

/// 渲染装置（机械随动，D40 / D41）：把「元素根路径 + 目标 FQN 末段」装成
/// [`EmitTarget`]，形态与查表在此**显式**给出——渲染器零形态推导、零 FQN 派生，故两者
/// 只能由调用方（本装置即调用方姿态）提供。
///
/// 这是**测试姿态**：产品侧的两值由 `render_all` 经 `paths::{entry_form, module_name,
/// rust_module_path}` 算齐；用例的判据形态（查表 vs 派生）由测试相位按 D40 裁定。
fn render_root(contract: &Contract, schema: &Value, meta: &ApiMeta, element_root: &str) -> String {
    render(
        contract,
        schema,
        meta,
        &render_root_target_for(contract, element_root),
    )
}

/// 渲染装置的目标构造面（同一查表口径；供「入参表达空 scope」的用例直接取 `EmitTarget`）。
fn render_root_target_for(contract: &Contract, element_root: &str) -> EmitTarget {
    let form = if paths::module_name(element_root).contains('.') {
        EntryForm::SingleFile
    } else {
        EntryForm::SubdirModule
    };
    // 目标 FQN → 目标元素根：产品侧由 `render_all` 从元素表取得，本装置用同形表复刻
    let targets = [
        ("App.Frontend.Components.Agent.Input", "views/agent/Input"),
        (
            "App.Frontend.Components.Agent.AgentBlocks",
            "views/agent/AgentBlocks",
        ),
    ];
    let mut modules = BTreeMap::new();
    for target in contract.reexports.values() {
        let Some(fqn) = target.as_str() else {
            continue;
        };
        let Some((_, element_root)) = targets
            .iter()
            .find(|(candidate, _)| *candidate == fqn)
            .copied()
        else {
            continue;
        };
        modules.insert(
            fqn.to_owned(),
            ModuleRef {
                name: paths::module_name(element_root),
                module_path: paths::rust_module_path(element_root),
            },
        );
    }
    EmitTarget {
        kind: crate::api::paths::GeneratedKind::TypeScript,
        form,
        modules,
    }
}

/// 只含 `api-types` fence 的源文渲染。
fn render_types(types: &str) -> String {
    render_source(&format!("# {FQN}\n\n```json api-types\n{types}\n```\n"))
}

// ---------------------------------------------------------------------------
// object / 非 required 字段
// ---------------------------------------------------------------------------

#[test]
fn render的object为interface() {
    let text = render_types(
        r#"{"Item": {"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]}}"#,
    );
    assert!(
        text.contains("export interface Item {"),
        "object → export interface：\n{text}"
    );
    assert!(
        text.lines().any(|line| line.trim() == "a: string;"),
        "字段逐项（required → 裸类型）：\n{text}"
    );

    let zero_field = render_types(r#"{"Empty": {"type": "object", "properties": {}}}"#);
    assert!(
        zero_field.contains("export interface Empty {}"),
        "零字段 object → 空体形态：\n{zero_field}"
    );
}

/// 正向（AC-5）：非 `required` 字段 → `T | null`；`required = []` → 全 `| null`。
#[test]
fn render的非required字段为可空() {
    let partial = render_types(
        r#"{
  "Item": {
    "type": "object",
    "properties": {"a": {"type": "string"}, "b": {"type": "integer"}},
    "required": ["a"]
  }
}"#,
    );
    assert!(
        partial.lines().any(|line| line.trim() == "a: string;"),
        "required 字段为裸类型：\n{partial}"
    );
    assert!(
        partial
            .lines()
            .any(|line| line.trim() == "b: number | null;"),
        "未列字段加 `| null`：\n{partial}"
    );

    let none_required = render_types(
        r#"{"Item": {"type": "object", "properties": {"a": {"type": "string"}}, "required": []}}"#,
    );
    assert!(
        none_required
            .lines()
            .any(|line| line.trim() == "a: string | null;"),
        "required = [] → 全可空：\n{none_required}"
    );
}

// ---------------------------------------------------------------------------
// enum / x-class / 标量 / oneOf
// ---------------------------------------------------------------------------

/// 正向（AC-5 / AC-6）：`enum` → `as const` 数组 + 同名 `type`（值 / 类型共存），数组元素
/// 一律单引号；零值 enum 形态可判。
#[test]
fn render的enum为as_const与同名type() {
    let text = render_types(r#"{"Mode": {"enum": ["a", "b"]}}"#);
    assert!(
        text.contains("export const Mode = ['a', 'b'] as const;"),
        "as const 数组（单引号）：\n{text}"
    );
    assert!(
        text.contains("export type Mode = (typeof Mode)[number];"),
        "同名类型别名（同名值 / 类型共存）：\n{text}"
    );
    assert!(
        !text.contains("[\"a\", \"b\"]"),
        "数组字面量 MUST NOT 用双引号（prettier 不动点）：\n{text}"
    );

    let empty = render_types(r#"{"None": {"enum": []}}"#);
    assert!(
        empty.contains("export const None = [] as const;"),
        "零值 enum 形态可判：\n{empty}"
    );
}

/// 正向（AC-5 / R-5）：`x-class: true` → `export interface` + `degraded` 注释（结构化
/// 类型无法强制实现，MUST NOT 静默弱一档）；方法成员逐项。
#[test]
fn render的x_class为interface并记degraded() {
    let text = render_types(
        r#"{
  "Runner": {
    "x-class": true,
    "properties": {"run": {"x-function": {"params": [{"name": "input", "schema": {"type": "string"}}], "returns": {"type": "boolean"}}}}
  }
}"#,
    );
    assert!(
        text.contains("export interface Runner {"),
        "x-class → interface：\n{text}"
    );
    assert!(
        text.contains("x-common.degraded"),
        "降级留痕注释在案：\n{text}"
    );
    assert!(
        text.lines()
            .any(|line| line.trim() == "run: (input: string) => boolean;"),
        "方法成员逐项：\n{text}"
    );
    assert!(
        !text.lines().any(|line| line.contains(": number;")),
        "MUST NOT 有属性成员（值域判据在上游）：\n{text}"
    );
}

/// 正向：标量 → 类型别名（按设计退化、不记 `degraded`）。
#[test]
fn render的标量为别名() {
    let cases = [
        ("integer", "export type Count = number;"),
        ("string", "export type Label = string;"),
        ("boolean", "export type Flag = boolean;"),
    ];
    for (ty, expected) in cases {
        let text = render_types(&format!(
            r#"{{"{name}": {{"type": "{ty}"}}}}"#,
            name = match ty {
                "integer" => "Count",
                "string" => "Label",
                _ => "Flag",
            }
        ));
        assert!(
            text.contains(expected),
            "{ty} → 别名（{expected}）：\n{text}"
        );
        assert!(
            !text.contains("x-common.degraded"),
            "标量别名是按设计退化，不记 degraded：\n{text}"
        );
    }
}

/// 正向（AC-5）：`oneOf` / `anyOf` → union；单元素无悬垂 `|`。
#[test]
fn render的oneof为union() {
    for key in ["oneOf", "anyOf"] {
        let text = render_types(&format!(
            r##"{{
  "Choice": {{"{key}": [{{"$ref": "#/$defs/A"}}, {{"$ref": "#/$defs/B"}}]}},
  "A": {{"type": "object", "properties": {{}}}},
  "B": {{"type": "object", "properties": {{}}}}
}}"##
        ));
        assert!(
            text.contains("export type Choice = A | B;"),
            "{key} → union：\n{text}"
        );
    }

    let single = render_types(
        r##"{
  "Only": {"oneOf": [{"$ref": "#/$defs/A"}]},
  "A": {"type": "object", "properties": {}}
}"##,
    );
    assert!(
        single.contains("export type Only = A;"),
        "单元素 union 无悬垂 `|`：\n{single}"
    );
}

// ---------------------------------------------------------------------------
// x-function 同步 / 异步
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`x-function` 同步 → `(a: string) => string`；`async = true` → 返回位
/// `Promise<R>`（错误通道走 reject，记 degraded）。
///
/// **合法落点②**（D47）：`$defs` 条目第一层 `properties` 的**值**是 `x-function` 的
/// 第二个合法位置（落点①是 `api-op` 顶层签名）——两者都产出函数类型，故两种来源
/// 各一断言。
#[test]
fn render的x_function同步与异步() {
    // 落点①：`api-op` 顶层签名（`function_of` 读包裹形态）
    let sync_op = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [{{\"name\": \"a\", \"schema\": \
         {{\"type\": \"string\"}}}}], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
    ));
    assert!(
        sync_op.contains("export type load = (a: string) => string;"),
        "同步函数别名：\n{sync_op}"
    );

    let asynchronous = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"type\": \
         \"string\"}}, \"async\": true}}\n```\n"
    ));
    assert!(
        asynchronous.contains("export type load = () => Promise<string>;"),
        "异步函数 → Promise<R>：\n{asynchronous}"
    );

    // 落点②：`$defs` 条目第一层 `properties` 的值（`interface_block` 的方法分流）
    let field_function = render_types(
        r#"{"Hooks": {"type": "object", "properties": {"call": {"x-function": {"params": [{"name": "a", "schema": {"type": "string"}}], "returns": {"type": "string"}}}}}}"#,
    );
    assert!(
        field_function.contains("export interface Hooks {"),
        "带 `x-function` 字段的对象仍落 interface：\n{field_function}"
    );
    assert!(
        field_function
            .lines()
            .any(|line| line.trim() == "call: (a: string) => string;"),
        "`properties` 值的 `x-function` → 函数类型成员（合法落点②）：\n{field_function}"
    );
}

/// 正向（AC-4 / AC-11）：`render` 的目录形态与单文件形态 specifier 均**逐字
/// 等于** `paths::ts_import_specifier` 的返回值（零手算 `../` 层数）。
#[test]
fn render目录形态specifier() {
    use crate::api::paths::{ts_import_specifier, EntryForm};

    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    ));

    let directory = render_root(&contract, &schema, &meta(), "crates/core/archi");
    let directory_specifier = ts_import_specifier(EntryForm::SubdirModule, "Input");
    assert_eq!(
        directory_specifier, "../Input.i",
        "目录形态单点值（`../<name>.i`）"
    );
    assert!(
        directory.contains(&format!("from '{directory_specifier}'")),
        "目录形态 specifier 逐字取自单点函数：\n{directory}"
    );

    let single = render_root(&contract, &schema, &meta(), "views/architecture.tsx");
    let single_specifier = ts_import_specifier(EntryForm::SingleFile, "Input");
    assert_eq!(
        single_specifier, "./Input.i",
        "单文件形态单点值（`./<name>.i`）"
    );
    assert!(
        single.contains(&format!("from '{single_specifier}'")),
        "单文件形态 specifier 逐字取自单点函数：\n{single}"
    );
    assert_ne!(
        directory_specifier, single_specifier,
        "两形态互不相等（手算层数即错位）"
    );
}

/// 边界（D41 / D46）：「空 scope」由**入参**表达——空查表 + 目录形态的 `EmitTarget`
/// 传进四参 `render` 后与「有查表但零透传条目」形态**逐字相等**（MUST NOT 两套模板、
/// MUST NOT 第二个函数入口）；有透传条目而查表为空时只出注释、零 specifier。
#[test]
fn render的空scope由入参表达() {
    // ① 有透传条目 + 空查表 → 只出注释、不产 specifier（不 panic、不退化到 FQN 末段）
    let with_reexport = format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n\n\
         ```json api-types\n{{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    );
    let (contract, schema) = prepare(&with_reexport);
    let by_empty_table = render(&contract, &schema, &meta(), &empty_scope());
    assert!(
        by_empty_table.contains("// 透传自 App.Frontend.Components.Agent.Input"),
        "查不到目标只留注释：\n{by_empty_table}"
    );
    assert!(
        !by_empty_table.contains(" from '"),
        "查表为空即不产 specifier：\n{by_empty_table}"
    );

    // ② 零透传条目：空查表与「有查表」两形态逐字相等（无透传即不参与）
    let without_reexport = format!(
        "# {FQN}\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    );
    let (plain_contract, plain_schema) = prepare(&without_reexport);
    let mut filled = empty_scope();
    filled.modules.insert(
        "App.Frontend.Components.Agent.Input".to_owned(),
        ModuleRef {
            name: "Input".to_owned(),
            module_path: "crate::input".to_owned(),
        },
    );
    assert_eq!(
        render(&plain_contract, &plain_schema, &meta(), &empty_scope()),
        render(&plain_contract, &plain_schema, &meta(), &filled),
        "无透传条目时查表不参与（空 scope 由入参表达、同一实现）"
    );
    assert!(
        !render(&plain_contract, &plain_schema, &meta(), &filled).contains(" from '"),
        "无透传条目零 import 语句"
    );
}

/// 异常（源码自证 + 行为对拍）：形态判定不含字面量层数——来源文件内 `render`
/// 体内零 `"../"` / `"./"` 字面量（形态走 `EntryForm`、值走 `paths::ts_import_specifier`）；
/// 把单文件元素改判为目录形态串 → 输出随之改变且仍与单点函数一致（零硬编码）。
#[test]
fn render形态判定不含字面量层数() {
    use crate::api::paths::{ts_import_specifier, EntryForm};

    let source = include_str!("ts.rs");
    assert!(
        !source.contains("\"../") && !source.contains("\"./"),
        "来源文件零 `../` / `./` 字面量（层数不手算）"
    );
    assert!(
        !source.contains("entry_form_of"),
        "零形态二次派生（D41：形态由 EmitTarget.form 传入，私有 entry_form_of 已删）"
    );
    assert!(
        !source.contains("rsplit('.')"),
        "零 FQN 派生（D40：`<name>` 只来自调用方查表）"
    );
    assert!(
        source.contains("target.form") && source.contains("ts_import_specifier"),
        "形态取 EmitTarget.form、specifier 值走 paths::ts_import_specifier"
    );

    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    ));
    let as_single = render_root(&contract, &schema, &meta(), "views/architecture.tsx");
    let as_directory = render_root(&contract, &schema, &meta(), "views/architecture");
    assert_ne!(
        as_single, as_directory,
        "形态串不同 ⇒ 输出随之改变（无硬编码层数）"
    );
    assert!(
        as_single.contains(&format!(
            "from '{}'",
            ts_import_specifier(EntryForm::SingleFile, "Input")
        )),
        "单文件形态仍与单点函数一致：\n{as_single}"
    );
    assert!(
        as_directory.contains(&format!(
            "from '{}'",
            ts_import_specifier(EntryForm::SubdirModule, "Input")
        )),
        "目录形态仍与单点函数一致：\n{as_directory}"
    );
}

/// 边界：`element_root` 含空格 / 非 ASCII / emoji / 超长（>1000 chars）→ 不 panic，
/// specifier 仍是可判字符串；带尾斜杠 / 前导 `./` → 经 `module_name` 归一后与无前缀
/// 形态等价。
#[test]
fn render特殊字符element_root() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    ));

    let long = format!("views/{}", "a".repeat(1200));
    let cases = [
        "views/with space/architecture",
        "views/模块/architecture",
        "views/🎯/architecture",
        long.as_str(),
    ];
    for element_root in cases {
        let text = render_root(&contract, &schema, &meta(), element_root);
        assert!(
            text.contains("from '../Input.i'"),
            "目录形态 specifier 仍可判（不断言 TS 语义，只断言形态契约）：{element_root:?}\n{text}"
        );
    }

    let plain = render_root(&contract, &schema, &meta(), "views/architecture");
    for equivalent in ["./views/architecture", "views/architecture/"] {
        assert_eq!(
            render_root(&contract, &schema, &meta(), equivalent),
            plain,
            "尾斜杠 / 前导 `./` 经 module_name 归一后等价：{equivalent:?}"
        );
    }

    // 空串仍可判（退化形态，不 panic）
    let empty = render_root(&contract, &schema, &meta(), "");
    assert!(
        empty.contains("from '../Input.i'"),
        "空 root 走目录形态：\n{empty}"
    );
}

/// 正向（AC-5 / D9 位置分派）：容器类型在两侧逐位对等——`array` → `T[]`、`object` +
/// `additionalProperties` → `Record<string, V>`；缺 `additionalProperties` 与不可归类的
/// `type` 均退 `unknown`（零品牌类型，也不静默收窄成 `any`）。
#[test]
fn render的容器类型映射() {
    let array = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [{{\"name\": \"items\", \"schema\": \
         {{\"type\": \"array\", \"items\": {{\"type\": \"string\"}}}}}}], \"returns\": {{\"type\": \
         \"array\", \"items\": {{\"type\": \"number\"}}}}}}\n```\n"
    ));
    assert!(
        array.contains("export type load = (items: string[]) => number[];"),
        "array → T[]（两侧同位）：\n{array}"
    );

    let map = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [{{\"name\": \"m\", \"schema\": \
         {{\"type\": \"object\", \"additionalProperties\": {{\"type\": \"integer\"}}}}}}], \
         \"returns\": {{\"type\": \"object\"}}}}\n```\n"
    ));
    assert!(
        map.contains("export type load = (m: Record<string, number>) => Record<string, unknown>;"),
        "map → Record<string, V>、无 additionalProperties → Record<string, unknown>：\n{map}"
    );

    // 字段位 array 与操作位同形（同一 `value_type` 单点）
    let field = render_types(
        r#"{"Holder": {"type": "object", "properties": {"items": {"type": "array", "items": {"type": "boolean"}}}, "required": ["items"]}}"#,
    );
    assert!(
        field.contains("items: boolean[];"),
        "字段位 array → T[]：\n{field}"
    );
}

#[test]
fn render的可选参数与outcome信封() {
    let optional = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [{{\"name\": \"a\", \"schema\": \
         {{\"type\": \"string\"}}, \"optional\": true}}], \"returns\": {{\"type\": \"string\"}}}}\n```\n"
    ));
    assert!(
        optional.contains("export type load = (a: string | null) => string;"),
        "可选参数标 `| null`：\n{optional}"
    );

    let outcome = render_source(&format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"outcome\": \
         {{\"type\": \"string\"}}}}}}\n```\n"
    ));
    assert!(
        outcome.contains("export type load = () => { ok: string } | { err: string };"),
        "Outcome 信封展开：\n{outcome}"
    );
}

// ---------------------------------------------------------------------------
// 透传 / 头注释
// ---------------------------------------------------------------------------

/// 正向（AC-11 / AC-6）：纯 `$ref` alias 两项 → `export type { X } from '<specifier>'`
/// （`isolatedModules` 下用 `export type`），specifier 取自单点函数（零手算 `../`）。
#[test]
fn render的透传export_type() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\", \
         \"AgentBlocks\": \"App.Frontend.Components.Agent.AgentBlocks\"}}\n```\n\n## 透传\n\n\
         - AgentInput → App.Frontend.Components.Agent.Input\n\
         - AgentBlocks → App.Frontend.Components.Agent.AgentBlocks\n"
    ));

    let directory = render_root(&contract, &schema, &meta(), "views/architecture");
    assert!(
        directory.contains("export type { AgentInput } from '../Input.i';"),
        "目录形态 specifier `../<name>.i`：\n{directory}"
    );
    assert!(
        directory.contains("export type { AgentBlocks } from '../AgentBlocks.i';"),
        "第二项透传在案：\n{directory}"
    );

    let single = render_root(&contract, &schema, &meta(), "views/architecture.tsx");
    assert!(
        single.contains("export type { AgentInput } from './Input.i';"),
        "单文件形态 specifier `./<name>.i`：\n{single}"
    );
}

/// 正向（D13）：文本首部为 `//` 注释块，含四句 + 工具版本 + `common.json` 摘要前 12 位；
/// 注释块之外是可编译的 TS 语句形态。
#[test]
fn render的头注释() {
    let text = render_source(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \"properties\": \
         {{\"a\": {{\"type\": \"string\"}}}}}}}}\n```\n"
    ));
    let header: Vec<&str> = text
        .lines()
        .take_while(|line| line.starts_with("//"))
        .collect();
    assert!(header.len() >= 4, "头注释至少四行：{header:?}");
    let joined = header.join("\n");
    for phrase in ["勿手改", "下次 generate", "api:check", "impl 块"] {
        assert!(joined.contains(phrase), "头注释含 `{phrase}`：\n{joined}");
    }
    assert!(
        joined.contains(&format!("archi-api {}", env!("CARGO_PKG_VERSION"))),
        "头注释含工具版本：\n{joined}"
    );
    let digest = meta().common_digest;
    let (algorithm, hex) = digest.split_once(':').expect("摘要含算法前缀");
    assert!(
        joined.contains(&format!("{algorithm}:{}", &hex[..12])),
        "摘要取前 12 位：\n{joined}"
    );

    let body: Vec<&str> = text
        .lines()
        .skip_while(|line| line.starts_with("//"))
        .collect();
    assert!(
        body.iter()
            .any(|line| line.starts_with("export interface Item")),
        "注释块之外是正常 TS 语句：\n{text}"
    );
}

// ---------------------------------------------------------------------------
// 反例：ambient-free / 无品牌类型 / 零 eprintln!
// ---------------------------------------------------------------------------

/// 异常（AC-6 反例在案）：渲染结果 ambient-free——不含 `declare` / `declare module` /
/// `.d.ts` 形态；来源文件双自查零命中。
#[test]
fn render的ambient_free() {
    let text = render_source(&format!(
        "# {FQN}

```json api-types
{{\"Item\": {{\"type\": \"object\", \"properties\": {{}}}}}}
```
"
    ));
    assert!(
        !text.contains("declare module") && !text.contains("declare const"),
        "产物 MUST NOT 含 ambient 声明：
{text}"
    );
    assert!(
        !text.contains(".d.ts"),
        "产物 MUST NOT 是 .d.ts 形态：
{text}"
    );
    // 来源文件自证：零 ambient 声明语句（文档注释里的禁令措辞不算）
    let source = include_str!("ts.rs");
    assert!(
        !source.contains("declare module") && !source.contains("declare const"),
        "来源文件零 ambient 声明语句"
    );
    assert!(
        source.contains("ambient-free"),
        "来源文件显式声明 ambient-free 纪律"
    );
}

/// 边界（proposal §5）：标量别名不产品牌类型（`number & { __brand: … }` 会让 cast
/// 传染）——文本零 `__brand` 字面量。
#[test]
fn render无品牌类型() {
    let text = render_types(r#"{"Count": {"type": "integer"}}"#);
    assert!(
        text.contains("export type Count = number;"),
        "标量别名是裸基础类型：
{text}"
    );
    assert!(
        !text.contains("__brand"),
        "MUST NOT 产品牌类型：
{text}"
    );
    assert!(
        !text.contains(" & {"),
        "MUST NOT 产交叉类型品牌形态：
{text}"
    );
}

/// 边界（D27）：本渲染器零 `eprintln!`（`x-class` 警告单点在 `emit/mod.rs`），故同一
/// 契约不会「rust 侧警告 + ts 侧再警告」双报；`x-class` 的文本只含方法成员。
#[test]
fn render零eprintln且x_class只含方法() {
    assert!(
        !include_str!("ts.rs").contains("eprintln!("),
        "本渲染器零 eprintln!（警告单点在 emit/mod.rs）"
    );

    let text = render_types(
        r#"{
  "Runner": {
    "x-class": true,
    "properties": {"run": {"x-function": {"params": [], "returns": {"type": "boolean"}}}}
  }
}"#,
    );
    let members: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("  ") && line.contains(':'))
        .collect();
    assert!(!members.is_empty(), "有方法成员：\n{text}");
    for member in members {
        assert!(
            member.contains("=>"),
            "x-class 的 interface 成员必须是方法（含 `=>`）：{member}"
        );
    }
}

// ---------------------------------------------------------------------------
// 泛型 / 幂等 / 空契约 / 规模
// ---------------------------------------------------------------------------

/// 异常（AC-5 反例）：本渲染器不处理泛型——`$ref` 名字原样落位、MUST NOT 自创
/// `Array<T>` / `<T>` 形态（拦截在 `emit::render_target` 的 `reject_generics`）。
#[test]
fn render不猜泛型语法() {
    let text = render_types(
        r##"{"Wrapper": {"type": "object", "properties": {"inner": {"$ref": "#/$defs/T"}}}}"##,
    );
    assert!(
        text.contains("inner: T | null;"),
        "`$ref` 名字原样进字段类型：\n{text}"
    );
    assert!(
        !text.contains("Array<"),
        "MUST NOT 自创 `Array<T>` 形态：\n{text}"
    );
    assert!(!text.contains("<T>"), "MUST NOT 产 `<T>` 形态：\n{text}");
}

/// 边界：同输入两次渲染逐字相等；类型段按 `$defs` 声明序（不排序、不合并）。
#[test]
fn render幂等与键序() {
    let source = format!(
        "# {FQN}\n\n```json api-types\n{{\"Zeta\": {{\"type\": \"object\", \"properties\": {{}}}}, \
         \"Alpha\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    );
    let first = render_source(&source);
    let second = render_source(&source);
    assert_eq!(first, second, "两次渲染逐字相等");
    let zeta = first.find("export interface Zeta").expect("Zeta 在案");
    let alpha = first.find("export interface Alpha").expect("Alpha 在案");
    assert!(zeta < alpha, "类型段按声明序：\n{first}");
}

/// 边界（AC-12 / D29）：输出为 `vp fmt` 语义的不动点——零尾随空白、无 tab 缩进、数组
/// 字面量一律单引号、文件以换行收束；渲染路径零子进程（不 spawn prettier / vp）。
#[test]
fn render形态为格式化器不动点() {
    let text = render_source(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Mode\": {{\"enum\": [\"a\", \"b\"]}}, \"Item\": {{\"type\": \"object\", \"properties\": {{\"a\": {{\"type\": \"string\"}}}}}}}}\n```\n"
    ));
    for line in text.lines() {
        assert_eq!(line, line.trim_end(), "零尾随空白：{line:?}");
        assert!(!line.contains('\t'), "零 tab 缩进：{line:?}");
    }
    assert!(text.ends_with('\n'), "末尾以换行收束");
    assert!(
        !text.contains("[\"a\", \"b\"]"),
        "数组字面量一律单引号：\n{text}"
    );
    assert!(
        !include_str!("ts.rs").contains("std::process"),
        "渲染路径零子进程（格式化器不 spawn）"
    );
}

/// 边界：零类型零 op → 仅头注释或空串，不含 `export`。
#[test]
fn render空契约() {
    let text = render_source(&format!("# {FQN}\n\n## 职能\n\n正文。\n"));
    assert!(!text.contains("export "), "零内容不出 export：\n{text}");
    assert!(
        text.lines()
            .all(|line| line.is_empty() || line.starts_with("//")),
        "只余头注释：\n{text}"
    );
}

/// 边界：1000 类型 → 不 panic、条数相等；类型名含 emoji / 超长 `description`
/// （>1000 chars）不破坏 TS 形态（不含裸 NUL）。
#[test]
fn render特殊字符与规模() {
    let long = "描".repeat(1200);
    let mut types = String::from("{");
    for index in 0..1000 {
        if index > 0 {
            types.push(',');
        }
        types.push_str(&format!(
            "\"T{index}\": {{\"type\": \"object\", \"properties\": {{}}}}"
        ));
    }
    types.push_str(&format!(
        ",\"长🎯\": {{\"type\": \"object\", \"properties\": {{}}, \"description\": \"{long}\"}}}}"
    ));

    let text = render_types(&types);
    assert_eq!(
        text.matches("export interface T").count(),
        1000,
        "1000 类型逐条渲染"
    );
    assert!(
        text.contains("export interface 长🎯"),
        "非 ASCII 类型名原样：\n{}",
        &text[..200.min(text.len())]
    );
    assert!(!text.contains('\0'), "MUST NOT 含裸 NUL");
}

/// 异常（D40 反例在案）：目标元素 `App.Backend.Infra.ArchiRuntime`（FQN 末段
/// `ArchiRuntime`，元素根末段 `archi`）→ 文本 specifier 含 `archi` 而**不含**
/// `ArchiRuntime`；`emit/ts.rs` 内零 `rsplit('.')`；`modules` 查不到该 FQN → 只出注释、
/// 不产 specifier（不退化到 FQN 末段）。
#[test]
fn render的name取路径末段而非fqn末段() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"Archived\": \"{FQN}\"}}\n```\n\n\
         ## 透传\n\n- Archived → {FQN}\n"
    ));
    let mut modules = BTreeMap::new();
    modules.insert(
        FQN.to_owned(),
        ModuleRef {
            name: paths::module_name("crates/infra/archi"),
            module_path: paths::rust_module_path("crates/infra/archi"),
        },
    );
    let target = EmitTarget {
        kind: paths::GeneratedKind::TypeScript,
        form: EntryForm::SubdirModule,
        modules,
    };
    let text = render(&contract, &schema, &meta(), &target);
    assert!(
        text.contains("from '../archi.i'"),
        "specifier 的 `<name>` 取路径末段：\n{text}"
    );
    assert!(
        !text.contains("ArchiRuntime.i"),
        "MUST NOT 取目标 FQN 末段：\n{text}"
    );

    let source = include_str!("ts.rs");
    assert!(
        !source.contains("rsplit('.')"),
        "`emit/ts.rs` 内零 `rsplit('.')`（在案反例的形态）"
    );

    // 查不到该 FQN → 只出注释、不产 specifier
    let mut empty = BTreeMap::new();
    let target_without = EmitTarget {
        kind: paths::GeneratedKind::TypeScript,
        form: EntryForm::SubdirModule,
        modules: std::mem::take(&mut empty),
    };
    let degraded = render(&contract, &schema, &meta(), &target_without);
    assert!(
        degraded.contains(&format!("// 透传自 {FQN}")),
        "查不到目标即只出注释：\n{degraded}"
    );
    assert!(
        !degraded.contains("from '"),
        "不产 specifier（不退化到 FQN 末段）：\n{degraded}"
    );
}

/// 边界（D40 边界）：`ModuleRef.name` 含空格 / 非 ASCII / emoji / 超长（>1000 chars）→
/// 不 panic，生成的 specifier 仍是可判字符串（不断言 TS 语义，只断言形态契约不破）；
/// 归一责任在 `paths::module_name`（渲染器零归一）。
#[test]
fn render的查表值特殊字符() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"Archived\": \"{FQN}\"}}\n```\n\n\
         ## 透传\n\n- Archived → {FQN}\n"
    ));
    let long = "a".repeat(1200);
    let cases = ["with space", "模块", "🎯", long.as_str()];
    for name in cases {
        let mut modules = BTreeMap::new();
        modules.insert(
            FQN.to_owned(),
            ModuleRef {
                name: name.to_owned(),
                module_path: format!("crate::{name}"),
            },
        );
        let target = EmitTarget {
            kind: paths::GeneratedKind::TypeScript,
            form: EntryForm::SubdirModule,
            modules,
        };
        let text = render(&contract, &schema, &meta(), &target);
        let expected = paths::ts_import_specifier(EntryForm::SubdirModule, name);
        assert!(
            text.contains(&format!("from '{expected}'")),
            "specifier 仍逐字取自单点函数（name = {name:?}）：\n{text}"
        );
    }
}

// ---------------------------------------------------------------------------
// 闸二：TS 字符串字面量与注释拼接的单点（D45 / AC-23 ②③④）
// ---------------------------------------------------------------------------

/// 正向（AC-23 ② / D45 闸二）：`ts_string_literal` 产出**单引号**串并转义七个面。
#[test]
fn ts_string_literal单引号与转义() {
    assert_eq!(ts_string_literal("a"), "'a'", "单引号串（不是双引号）");
    assert_eq!(ts_string_literal("it's"), "'it\\'s'", "`'` 转义");
    assert_eq!(ts_string_literal("a\\b"), "'a\\\\b'", "`\\` → `\\\\`");
    assert_eq!(ts_string_literal("a\nb"), "'a\\nb'", "换行 → 转义序列");
    assert_eq!(ts_string_literal("a\rb"), "'a\\rb'", "回车 → 转义序列");
    assert_eq!(ts_string_literal("a\tb"), "'a\\tb'", "制表 → 转义序列");
    assert_eq!(
        ts_string_literal("a\u{2028}b"),
        "'a\\u2028b'",
        "U+2028 → `\\u2028`（不产裸行分隔符）"
    );
    assert_eq!(
        ts_string_literal("a\u{2029}b"),
        "'a\\u2029b'",
        "U+2029 → `\\u2029`"
    );

    // 每个产出都以单引号起止（合法 TS 字符串字面量形态）
    for text in ["a", "it's", "a\\b", "a\nb", "", "🚀"] {
        let literal = ts_string_literal(text);
        assert!(literal.starts_with('\''), "首字符是单引号：{literal:?}");
        assert!(
            literal.ends_with('\'') && literal.len() >= 2,
            "末字符是单引号：{literal:?}"
        );
        assert!(
            !literal.starts_with('"'),
            "MUST NOT 双引号形态：{literal:?}"
        );
    }
}

/// 边界（D45）：空串 → `''`；emoji 原样；超长不 panic；`\0` 与连续 `'''` 不破串。
#[test]
fn ts_string_literal边界() {
    assert_eq!(ts_string_literal(""), "''", "空串合法");
    assert_eq!(ts_string_literal("🚀"), "'🚀'", "emoji 非转义目标（原样）");

    let long = "a".repeat(1200);
    let literal = ts_string_literal(&long);
    assert_eq!(literal.len(), long.len() + 2, "超长不 panic 且长度可判");
    assert!(literal.starts_with('\'') && literal.ends_with('\''));

    // NUL 不在转义目标集内（单引号串内的裸 NUL 仍是合法 TS 字面量）——断言形态不破
    let nul = ts_string_literal("a\u{0}b");
    assert!(
        nul.starts_with('\'') && nul.ends_with('\''),
        "含 NUL 仍是单引号包裹的合法字面量形态：{nul:?}"
    );

    assert_eq!(
        ts_string_literal("'''"),
        "'\\'\\'\\''",
        "每个 `'` 各自转义（不断串）"
    );

    // 双引号无需转义（单引号串内原样保留）
    assert_eq!(
        ts_string_literal("it's a \"quote\""),
        "'it\\'s a \"quote\"'",
        "双引号值原样保留"
    );
}

/// 异常（AC-23 ③ 关键行为 / D45）：`enum` 值经 `ts_string_literal` 后产出**合法 TS**；
/// 源码自证零 `format!("'{text}'")` 形态的裸插值。
#[test]
fn render的enum值经ts_string_literal() {
    let text = render_types(r#"{"Mode": {"enum": ["it's a \"quote\"", "a\nb"]}}"#);
    assert!(
        text.contains(r#"export const Mode = ['it\'s a "quote"', 'a\nb'] as const;"#),
        "两成员各自经转义单点（合法 TS）：\n{text}"
    );
    assert!(
        text.contains("export type Mode = (typeof Mode)[number];"),
        "同名类型别名并存：\n{text}"
    );

    // 反例：源码内零裸插值形态、`ts_string_literal(` 调用在案
    let source = include_str!("ts.rs");
    assert!(
        !source.contains("format!(\"'{"),
        "零 `format!(\"'{{...}}'\")` 形态的裸插值"
    );
    assert!(
        source.matches("ts_string_literal(").count() >= 2,
        "`ts_string_literal(` 至少两处（定义 + 消费）"
    );

    // 文本内零双引号数组字面量（除契约原文在单引号串内的 `\"`）
    assert!(
        !text.contains("[\""),
        "数组字面量 MUST NOT 用双引号（D29 不动点）：\n{text}"
    );
}

/// 正向（AC-23 ② / D45）：`ts_comment_text` 断行防护。
#[test]
fn ts_comment_text断行防护() {
    assert_eq!(ts_comment_text("a\nb"), "a b", "换行 → 空格");
    assert_eq!(ts_comment_text("a\rb"), "a b", "回车 → 空格");
    assert_eq!(ts_comment_text("a\u{2028}b"), "a b", "U+2028 → 空格");
    assert_eq!(ts_comment_text("a\u{2029}b"), "a b", "U+2029 → 空格");
    assert_eq!(ts_comment_text("a'b"), "a'b", "注释内 `'` 无需转义");
    assert_eq!(ts_comment_text(""), "", "空串不 panic");

    let output = ts_comment_text("x\ny\rz\u{2028}w\u{2029}v");
    for banned in ['\n', '\r', '\u{2028}', '\u{2029}'] {
        assert!(
            !output.contains(banned),
            "输出内零断行字符 {banned:?}：{output:?}"
        );
    }
}

/// 边界（D45）：`modules` 查不到目标时产出的 `// 透传自 <FQN>` 注释经 `ts_comment_text`
/// ——单行、零新增语句；零 `eprintln!`。
#[test]
fn render的透传注释经ts_comment_text() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-reexport\n{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}\n```\n\n\
         ## 透传\n\n- AgentInput → App.Frontend.Components.Agent.Input\n"
    ));
    let target = EmitTarget {
        kind: crate::api::paths::GeneratedKind::TypeScript,
        form: EntryForm::SingleFile,
        modules: BTreeMap::new(),
    };
    let text = render(&contract, &schema, &meta(), &target);
    let comments: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("// 透传自 "))
        .collect();
    assert_eq!(comments.len(), 1, "透传注释恰一行：\n{text}");
    assert!(
        !comments[0].contains("from '"),
        "查不到目标不产 specifier：{}",
        comments[0]
    );
    assert!(
        !text.contains(&format!("from '{}", "./Agent.Input.i")),
        "MUST NOT 退化到 FQN 末段：\n{text}"
    );

    let source = include_str!("ts.rs");
    assert!(
        !source.contains("eprintln!("),
        "本渲染器零 `eprintln!`（警告单点在 emit/mod.rs；doc 注释里的词面不算）"
    );
    assert!(
        source.matches("ts_comment_text(").count() >= 3,
        "注释拼接均经该单点（头注释循环 + 透传两分支）；实际 {} 处",
        source.matches("ts_comment_text(").count()
    );
}

/// 边界（AC-23 ④ / D45）：`emit/` 内零 `serde_json::to_string` 生成字面量（双引号形态
/// 会破 D29 的 `vp fmt` 不动点）。
#[test]
fn 零serde_json_to_string生成字面量() {
    for (path, source) in [
        ("emit/mod.rs", include_str!("mod.rs")),
        ("emit/rust.rs", include_str!("rust.rs")),
        ("emit/ts.rs", include_str!("ts.rs")),
    ] {
        assert!(
            !source.contains("serde_json::to_string"),
            "`{path}` 内零 `serde_json::to_string`（字面量一律经三个转义单点）"
        );
    }
    // 三个转义单点各恰 1 处 `pub fn`
    let rust_source = include_str!("rust.rs");
    assert_eq!(
        rust_source.matches("pub fn rust_comment_text").count(),
        1,
        "`rust_comment_text` 恰 1 处"
    );
    let ts_source = include_str!("ts.rs");
    assert_eq!(
        ts_source.matches("pub fn ts_string_literal").count(),
        1,
        "`ts_string_literal` 恰 1 处"
    );
    assert_eq!(
        ts_source.matches("pub fn ts_comment_text").count(),
        1,
        "`ts_comment_text` 恰 1 处"
    );
}

// ---------------------------------------------------------------------------
// 出口单形（D46 / AC-24）：四参带签名恰一、零异名孪生、render_scope 私有
// ---------------------------------------------------------------------------

/// 取源码内顶层函数体（自 `signature` 起的花括号配对区段；未命中即断言失败）。
fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("源码内应有 `{signature}`"));
    let open = start + source[start..].find('{').expect("函数体起手花括号");
    let mut depth = 0usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("`{signature}` 的函数体未闭合");
}

/// 递归收集产品 `.rs`（排除 `*_test.rs` 与构建产物目录）的（路径，行）二元组。
fn scan_product_rs(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .map(|name| name == "target" || name == "node_modules")
                .unwrap_or(false);
            if !skip {
                scan_product_rs(&path, out);
            }
            continue;
        }
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if !name.ends_with(".rs") || name.ends_with("_test.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let display = path.to_string_lossy().replace('\\', "/");
        for line in text.lines() {
            out.push((display.clone(), line.to_owned()));
        }
    }
}

/// 正向（AC-24 ① / D46）：`emit/ts.rs` 内**带完整四参签名**的 `pub fn render` **恰 1**
/// 命中（同时钉住「恰一个导出」「四参」「`target` 末位」「返回 `String`」四事实——
/// MUST NOT 只数 `^pub fn render\(`，那形态在三参孪生与四参单形下同为 1）；行为面：
/// 同一契约 + 同一 `EmitTarget` 经 `render(..)` 与经 `emit::render_target(..)` 分派产出
/// 逐字相等（分派零额外语义）。
#[test]
fn render出口单形四参签名恰一() {
    let source = include_str!("ts.rs");
    const SIGNATURE: &str =
        "pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String";
    assert_eq!(
        source.matches(SIGNATURE).count(),
        1,
        "四参单形（`target` 末位、返回 `String`）恰 1 处（D46）"
    );
    // 返回 `String`（纯函数、零错误面）：本渲染器不构造 `ApiError`
    assert!(
        !source.contains("ApiError::Contract"),
        "本渲染器不构造契约错误（拦截在上游）"
    );

    // 行为面：直接调用与经 `emit::render_target` 分派逐字相等
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Item\": {{\"type\": \"object\", \
         \"properties\": {{\"a\": {{\"type\": \"string\"}}}}}}}}\n```\n"
    ));
    let target = empty_scope();
    let direct = render(&contract, &schema, &meta(), &target);
    let dispatched =
        crate::api::emit::render_target(&target, &contract, &schema, &meta()).expect("分派应成功");
    assert_eq!(direct, dispatched, "分派零额外语义（两路逐字相等）");
}

/// 异常（AC-24 ② / D46）：逐产品文件（`rust.rs` / `ts.rs` / `mod.rs` 各一条命令，
/// **MUST NOT 用目录级判据**——目录级会把本文件的用例名与调用面命中算进来）——
/// `fn render_with_root` / `fn render_with_target` **各 0 命中**；「空 scope」由
/// `EmitTarget { modules: BTreeMap::new(), .. }` 表达，MUST NOT 靠第二个函数表达。
#[test]
fn render零异名孪生() {
    for (path, source) in [
        ("emit/rust.rs", include_str!("rust.rs")),
        ("emit/ts.rs", include_str!("ts.rs")),
        ("emit/mod.rs", include_str!("mod.rs")),
    ] {
        for twin in ["fn render_with_root", "fn render_with_target"] {
            assert_eq!(
                source.matches(twin).count(),
                0,
                "`{path}` 内零 `{twin}`（两个异名孪生一并删除，D46）"
            );
        }
    }
}

/// 边界（AC-24 ③ / D46）：`^fn render_scope` 在 `emit/ts.rs` **恰 1** 命中、
/// `pub fn render_scope` **零命中**；唯一消费方是同文件的 `render`（升格即「仅测试
/// 可见的导出」，与 D43 对 `extract::render` 的裁定同款）。
#[test]
fn render_scope保持私有() {
    let source = include_str!("ts.rs");
    assert_eq!(
        source.matches("\nfn render_scope(").count(),
        1,
        "`fn render_scope` 恰 1 处定义"
    );
    assert_eq!(
        source.matches("pub fn render_scope").count(),
        0,
        "MUST NOT 为可测性升格 `render_scope` 的可见性"
    );

    // 唯一消费方是同文件的 `render`：`render_scope(` 的调用点只出现一次且在 `render` 体内
    let render_body = function_body(source, "pub fn render(");
    assert_eq!(
        render_body.matches("render_scope(").count(),
        1,
        "`render` 体内恰一处 `render_scope(` 调用（唯一消费方）"
    );
    let total_calls = source.matches("render_scope(").count();
    assert_eq!(
        total_calls, 2,
        "全文件 `render_scope(` 共 2 处：定义行 + `render` 体内唯一调用"
    );
}

/// 边界（AC-24 ④ / D46）：产品 `.rs`（滤 `*_test.rs`）内 `ts::render(` 的直连面**恰为
/// 分派器一处**（「没有第二个入口」的正面证据；`MUST NOT 用「零命中」作判据`——分派器
/// 本身必然调该函数）。
#[test]
fn 产品侧零直连ts渲染器() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    let direct: Vec<&(String, String)> = hits
        .iter()
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains("ts::render("))
        .collect();
    assert_eq!(
        direct.len(),
        1,
        "产品面 `ts::render(` 恰 1 处直连（分派器该分支）：{direct:?}"
    );
    assert!(
        direct[0].0.ends_with("api/emit/mod.rs"),
        "唯一调用点在分派器（实际：{}）",
        direct[0].0
    );

    // 分派面恰两路（`Rust` / `TypeScript` 各一），无第三条分派路径
    let dispatch = include_str!("mod.rs");
    assert_eq!(
        dispatch.matches("rust::render(").count(),
        1,
        "`Rust` 分支恰一路"
    );
    assert_eq!(
        dispatch.matches("ts::render(").count(),
        1,
        "`TypeScript` 分支恰一路"
    );
}

// ---------------------------------------------------------------------------
// value_type 零 x-function 分支（AC-25 ② / D47）
// ---------------------------------------------------------------------------

/// 异常（**AC-25 ②** / D47）：**逐函数体、锚函数**判（**MUST NOT 用文件级 `grep -c`**——
/// `ts.rs` 删前 **3**、删后 **2**（剩 `interface_block` 与 `function_of`），而 `rust.rs`
/// 恒 **3**，两侧合法命中点个数本就不同，任何「相等 / 恰 N」的文件级断言都无区分能力）：
/// `value_type` 体内 `"x-function"` 字面量 **恰 0**（删前 1）；行为面：
/// `{"type":"array","items":{"x-function":…}}` 形态**不再**产出 `((…) => R)[]`——该形态
/// 在 D47 之后装载期即中止、渲染器结构上收不到；`interface_block` 与 `function_of` 两处
/// `x-function` 读取点**仍在**（删的是 `value_type` 体内那一处，不是全部）。
#[test]
fn value_type零x_function分支() {
    let source = include_str!("ts.rs");
    let value_type_body = function_body(source, "fn value_type(");
    assert_eq!(
        value_type_body.matches("\"x-function\"").count(),
        0,
        "`value_type` 体内零 `\"x-function\"` 分支（D47 ②；删前 1）"
    );
    // 两处合法读取点仍在：`properties` 值（interface）与 `api-op` 包裹形态（function_of）
    let interface_body = function_body(source, "fn interface_block(");
    assert!(
        interface_body.contains("\"x-function\""),
        "`interface_block` 的 `x-function` 读取点在案（properties 值分流）"
    );
    let function_of_body = function_body(source, "fn function_of(");
    assert!(
        function_of_body.contains("\"x-function\""),
        "`function_of` 的 `x-function` 读取点在案（`api-op` 包裹形态）"
    );
    // 文件级命中点恰 2（不是判据，只作留痕：两侧个数本就不同 ⇒ MUST NOT 与 rust.rs 比）
    assert_eq!(
        source.matches("get(\"x-function\")").count(),
        2,
        "`ts.rs` 的合法读取点恰 2 处（`interface_block` / `function_of`）"
    );

    // 行为面：字段位 `items` 内的 `x-function` 不产出函数类型
    let text = render_types(
        r#"{"Holder": {"type": "object", "properties": {"boxed": {"type": "array", "items": {"x-function": {"params": [], "returns": {"type": "string"}}}}}}}"#,
    );
    assert!(
        !text.contains("=>"),
        "深层 `x-function` 不给兜底形态（装载期即中止，MUST NOT 出 `((…) => R)[]`）：\n{text}"
    );
    assert!(text.contains("unknown[]"), "`items` 走常规值映射：\n{text}");
}

// ---------------------------------------------------------------------------
// TS 侧 specifier 转义零筛选（D48 / AC-27）
// ---------------------------------------------------------------------------

/// 边界（**D48 / AC-27**）：`ModuleRef.name` 为**带扩展名末段**（`architecture.tsx`）⇒
/// specifier `'./architecture.tsx.i'` 经 `ts_string_literal` 引用——前端单文件元素的
/// **合法且必需**引用形式，TS 侧**零筛选**（MUST NOT 用 `is_module_segment` 判据筛 TS：
/// 那会误杀本形态）；`name` 含 `'` / `\` / 换行 → 转义后可判且不破坏 TS 形态。
#[test]
fn ts侧specifier转义零筛选() {
    // ① 带扩展名末段：单文件形态 specifier 合法且必需
    let dotted = "architecture.tsx";
    let dotted_target = render(
        &prepare(&format!(
            "# {FQN}\n\n```json api-reexport\n{{\"Archived\": \"{FQN}\"}}\n```\n\n\
             ## 透传\n\n- Archived → {FQN}\n"
        ))
        .0,
        &prepare(&format!(
            "# {FQN}\n\n```json api-reexport\n{{\"Archived\": \"{FQN}\"}}\n```\n\n\
             ## 透传\n\n- Archived → {FQN}\n"
        ))
        .1,
        &meta(),
        &EmitTarget {
            kind: paths::GeneratedKind::TypeScript,
            form: EntryForm::SingleFile,
            modules: BTreeMap::from([(
                FQN.to_owned(),
                ModuleRef {
                    name: dotted.to_owned(),
                    module_path: format!("crate::{}", paths::module_name(dotted)),
                },
            )]),
        },
    );
    let expected = paths::ts_import_specifier(EntryForm::SingleFile, dotted);
    assert_eq!(
        expected, "./architecture.tsx.i",
        "带扩展名的 specifier 形态"
    );
    assert!(
        dotted_target.contains(&format!("from '{expected}'")),
        "TS 侧零筛选：带扩展名末段照常入表并按单点产出 specifier：\n{dotted_target}"
    );

    // ② 含特殊字符的 name：转义后可判、不破 TS 形态（零筛选的必然推论）
    for name in ["it's.tsx", "a\\b.tsx", "a\nb.tsx"] {
        let (contract, schema) = prepare(&format!(
            "# {FQN}\n\n```json api-reexport\n{{\"Archived\": \"{FQN}\"}}\n```\n\n\
             ## 透传\n\n- Archived → {FQN}\n"
        ));
        let target = EmitTarget {
            kind: paths::GeneratedKind::TypeScript,
            form: EntryForm::SubdirModule,
            modules: BTreeMap::from([(
                FQN.to_owned(),
                ModuleRef {
                    name: name.to_owned(),
                    module_path: format!("crate::{}", paths::module_name(name)),
                },
            )]),
        };
        let text = render(&contract, &schema, &meta(), &target);
        let specifier = paths::ts_import_specifier(EntryForm::SubdirModule, name);
        let literal = ts_string_literal(&specifier);
        assert!(
            text.contains(&format!("from {literal}")),
            "特殊字符经 `ts_string_literal` 转义（name = {name:?}）：\n{text}"
        );
        assert!(
            literal.starts_with('\'') && literal.ends_with('\''),
            "字面量首末单引号：{literal:?}"
        );
    }

    // ③ 源码自证：本文件零 `is_module_segment`（Rust 判据 MUST NOT 筛 TS）
    let source = include_str!("ts.rs");
    assert!(
        !source.contains("is_module_segment"),
        "`emit/ts.rs` 内零 `is_module_segment`（TS 侧字面量可转义，零筛选）"
    );
}

// ---------------------------------------------------------------------------
// 同一 schema 两侧形态收敛（D47 行为面）
// ---------------------------------------------------------------------------

/// 边界（**D47** 行为面）：含 `x-function` 的合法二处（`api-op` 顶层签名 / `$defs` 条目
/// 第一层 `properties` 值）经两侧渲染：TS 出函数类型 `(…) => R`、Rust 出 `pub fn`
/// ——两侧**都**按各自语言的函数形态产出，零「一侧出函数、另一侧出 `serde_json::Value`」
/// 的静默分叉；越界位置（`items` / `additionalProperties` / `oneOf` 分支）在装载期即中
/// 止（本文件结构上收不到）。
#[test]
fn 同一schema两侧形态收敛() {
    let (contract, schema) = prepare(&format!(
        "# {FQN}\n\n```json api-types\n{{\"Runner\": {{\"x-class\": true, \"properties\": \
         {{\"run\": {{\"x-function\": {{\"params\": [{{\"name\": \"input\", \"schema\": \
         {{\"type\": \"string\"}}}}], \"returns\": {{\"type\": \"boolean\"}}}}}}}}}}}}\n```\n"
    ));
    let target = EmitTarget {
        kind: paths::GeneratedKind::TypeScript,
        form: EntryForm::SingleFile,
        modules: BTreeMap::new(),
    };
    let ts_text = render(&contract, &schema, &meta(), &target);
    assert!(
        ts_text.contains("run: (input: string) => boolean")
            || ts_text.contains("run(input: string)"),
        "TS 侧按函数形态产出（`x-function` → `(…) => R`）：\n{ts_text}"
    );

    // Rust 侧同源契约：出 `pub fn`（两侧都按函数形态，零静默分叉）
    let rust_target = EmitTarget {
        kind: paths::GeneratedKind::Rust,
        form: EntryForm::Crate,
        modules: BTreeMap::new(),
    };
    let rust_text = crate::api::emit::rust::render(&contract, &schema, &meta(), &rust_target);
    assert!(
        rust_text.contains("fn run(input: &str) -> bool"),
        "Rust 侧同一 schema 出函数签名（`x-class` 内为 trait 方法，零「一侧出函数、另一侧出 Value」）：\n{rust_text}"
    );

    // 越界位置在装载期即中止：本渲染器无位置判据（结构上收不到该输入）
    let source = include_str!("ts.rs");
    for marker in ["x-function 落点", "JSON pointer"] {
        assert!(
            !source.contains(marker),
            "`emit/ts.rs` 内零 `{marker}`（落点闸在装载期）"
        );
    }
}

// ---------------------------------------------------------------------------
// 映射规则查表（D54 ③ / AC-32）：产物文本由资产驱动，非渲染器硬编码
// ---------------------------------------------------------------------------

/// 查表命中（TS 槽位）：自造规则表后产物随之变化 ⇒ 证明文本来自表。
#[test]
fn common_type按表展开且随表变化() {
    use std::collections::BTreeMap;

    use crate::api::CommonRule;

    let source = format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"outcome\": \
         {{\"type\": \"string\"}}}}}}\n```\n"
    );
    let (contract, schema) = prepare(&source);

    let mut custom: BTreeMap<String, CommonRule> = BTreeMap::new();
    custom.insert(
        "Outcome".to_owned(),
        CommonRule {
            rust: "Mapped<{}>".to_owned(),
            typescript: "Mapped<{}>".to_owned(),
            degraded: None,
            note: None,
        },
    );
    let mut meta = meta();
    meta.common = custom;
    let text = render(&contract, &schema, &meta, &empty_scope());
    assert!(text.contains("Mapped<string>"), "产物经表展开：\n{text}");
    assert!(
        !text.contains("ok:"),
        "资产被替换后 MUST NOT 残留原文本形：\n{text}"
    );
}

/// 查表未命中：空表 ⇒ 回落 `Record<string, unknown>` 一类普通分派（不 panic）。
#[test]
fn common_type空表回落普通分派() {
    use std::collections::BTreeMap;

    let source = format!(
        "# {FQN}\n\n### load\n\n```json api-op\n{{\"params\": [], \"returns\": {{\"outcome\": \
         {{\"type\": \"string\"}}}}}}\n```\n"
    );
    let (contract, schema) = prepare(&source);
    let mut meta = meta();
    meta.common = BTreeMap::new();
    let text = render(&contract, &schema, &meta, &empty_scope());
    assert!(!text.contains("ok:"), "空表不产出资产文本形：\n{text}");
}

/// 边界（**D58 / AC-34 ①**）：「三字段只读两项」——`ts.rs` 内 `target.path` /
/// `target.element_root` 各 0 命中（两字段已删）；`target.form` 与 `target.modules` 各
/// ≥ 1（specifier 经 `paths::ts_import_specifier(form, module_ref.name)` 单点产出 + 透传
/// 查表）；把 `form` 从 `SubdirModule` 改成 `SingleFile` → specifier 从 `'../<name>.i'`
/// 变为 `'./<name>.i'`，且不存在第三条输入能影响它。
#[test]
fn 三字段只读两项() {
    let source = include_str!("ts.rs");
    for removed in ["target.path", "target.element_root"] {
        assert_eq!(
            source.matches(removed).count(),
            0,
            "`{removed}` 零命中（D58 已删该字段）"
        );
    }
    for kept in ["target.form", "target.modules"] {
        assert!(
            source.matches(kept).count() >= 1,
            "`{kept}` 是本层读取面（≥ 1）"
        );
    }
    assert!(
        source.contains("ts_import_specifier(form,"),
        "specifier 经单点函数产出（形态入参即 `form`）"
    );

    // 行为面：同一契约只改 `form` ⇒ specifier 随变
    let (contract, schema) = prepare(&format!(
        "# {FQN}

```json api-reexport
{{\"AgentInput\": \"App.Frontend.Components.Agent.Input\"}}
```

         ## 透传

- AgentInput → App.Frontend.Components.Agent.Input
"
    ));
    let directory = render_root(&contract, &schema, &meta(), "views/architecture");
    let single = render_root(&contract, &schema, &meta(), "views/architecture.tsx");
    assert!(
        directory.contains("from '../Input.i'"),
        "目录形态：
{directory}"
    );
    assert!(
        single.contains("from './Input.i'"),
        "单文件形态：
{single}"
    );
    assert_ne!(directory, single, "`form` 是 specifier 的唯一形态输入");
}

/// 渲染器产品面零硬编码：资产里的 TS 文本形只许在 `common.json`。
#[test]
fn 渲染器产品面零硬编码映射串() {
    let source = include_str!("ts.rs");
    assert_eq!(
        source.matches("{ ok: {} } | { err: string }").count(),
        0,
        "映射文本形只许在 `common.json`，渲染器零字面量"
    );
    assert!(source.contains("fn common_type"), "查表单点在案（D54 ③）");
}
