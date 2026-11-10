//! `api::extract::ts`（TS / JS 取数器）的单元测试：`export function` 是唯一操作判定、
//! `export type` / `interface` / `class` 是类型、`export const` 三分类（`memo` 组件留白 /
//! 对象与 `Proxy` 排除 / 箭头函数**不作操作**）、非导出声明与 `export default` 不抽、
//! 透传零符号、参数与返回类型映射、`source_type` 分派、行号与规模、
//! `apply_knip_report` 的降级与「只降不升」。
//!
//! **取数面 = 导出面**（D19-a）：无 `export` 的顶层声明一律零符号。
//! **`visibility` 值域留白**（D19-b）：本文件只产 `External`，`Unused` 只由
//! `apply_knip_report` 降级；`CrateOnly` / `TestOnly` 本期不产出，且 `.test.` / `.spec.` /
//! `__tests__` / `fixtures` 文件名族**不得**充当 `testOnly` proxy（用例逐形态断言 `External`）。
//!
//! 零 mock：真实 `oxc` AST 解析（三分类判定即被测语义）；knip 报告以 `json!` 直传入参
//! （**不 spawn `knip`**——进程边界由 CLI 侧承担，`api/**` 零 spawn）。

use serde_json::json;

use super::ts::{apply_knip_report, extract_file};
use super::{ApiSymbol, ApiSymbolKind, ApiType, Visibility};

/// 单文件抽取（`.ts` 源型）。
fn extract(source: &str) -> Vec<ApiSymbol> {
    extract_file("src/views/x.ts", source)
}

/// 按名字找符号（缺失即断言失败）。
fn find<'a>(symbols: &'a [ApiSymbol], name: &str) -> &'a ApiSymbol {
    symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap_or_else(|| {
            panic!(
                "符号 `{name}` 未抽出（实际：{:?}）",
                symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
            )
        })
}

// ---------------------------------------------------------------------------
// export function：唯一操作判定
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`export function` → 一条 `Function`；`export async function` →
/// `is_async == true`；生成器函数不 panic。
#[test]
fn extract_file的export_function为操作() {
    let symbols = extract(
        r#"export function load(a: string): string { return a; }
"#,
    );
    assert_eq!(symbols.len(), 1, "一条 export function：{symbols:?}");
    assert_eq!(symbols[0].name, "load");
    assert_eq!(symbols[0].kind, ApiSymbolKind::Function);
    assert!(!symbols[0].is_async, "同步函数");

    let asynchronous = extract(
        r#"export async function f(): Promise<void> {}
"#,
    );
    assert!(
        find(&asynchronous, "f").is_async,
        "export async function → is_async"
    );

    let generator = extract(
        r#"export function* g() {}
"#,
    );
    assert!(
        generator.iter().all(|symbol| symbol.name != "*"),
        "生成器：`*` 不进名字（零 panic）：{generator:?}"
    );
}

// ---------------------------------------------------------------------------
// export type / interface / class：类型面
// ---------------------------------------------------------------------------

/// 正向（AC-5）：`export type` / `export interface` / `export class` → 三条全 `Type`。
#[test]
fn extract_file的type_interface_class为类型() {
    let source = r#"export type A = string;
export interface B { x: number }
export class C {}
"#;
    let symbols = extract(source);
    assert_eq!(symbols.len(), 3, "三条类型符号：{symbols:?}");
    for name in ["A", "B", "C"] {
        let symbol = find(&symbols, name);
        assert_eq!(symbol.kind, ApiSymbolKind::Type, "{name} 的形态");
        assert_eq!(symbol.name, name, "{name} 逐字");
    }
}

// ---------------------------------------------------------------------------
// export const 三分类
// ---------------------------------------------------------------------------

/// 边界：`memo(...)` / `forwardRef(...)` / `lazy(...)` 形态 → 不产生 `Function` 符号
/// （组件留白，本期 `api = none` 形态）；内层具名函数不改判。
#[test]
fn extract_file的memo组件不产操作() {
    for source in [
        "export const View = memo(() => null);\n",
        "export const View = memo(function V() { return null; });\n",
        "export const View = forwardRef((props, ref) => null);\n",
        "export const View = lazy(() => import('./x'));\n",
    ] {
        let symbols = extract(source);
        assert!(
            !symbols
                .iter()
                .any(|symbol| symbol.name == "View" && symbol.kind == ApiSymbolKind::Function),
            "组件留白 MUST NOT 产出操作：{source} → {symbols:?}"
        );
        let view = find(&symbols, "View");
        assert_eq!(view.kind, ApiSymbolKind::Type, "组件记为类型面");
        assert_eq!(
            view.returns,
            ApiType::Unknown("component".to_owned()),
            "组件标记形态可辨"
        );
    }
}

/// 边界：`export const` 对象字面量与 `new Proxy(...)` → 零函数符号（排除）。
#[test]
fn extract_file的对象与proxy排除() {
    let object = extract("export const OBJ = { a: 1 };\n");
    assert!(
        !object
            .iter()
            .any(|symbol| symbol.kind == ApiSymbolKind::Function),
        "对象字面量不产操作：{object:?}"
    );

    let proxy = extract("export const P = new Proxy({}, {});\n");
    assert!(
        !proxy
            .iter()
            .any(|symbol| symbol.kind == ApiSymbolKind::Function),
        "Proxy 构造不产操作：{proxy:?}"
    );
}

/// 异常（D19-a 裁定② / S-9）：`export const` 的初始化式是箭头函数 / 函数表达式时
/// **MUST NOT** 产出 `Function` 符号——「从初始化表达式推断函数」是 spec `:294` 明文禁的
/// 启发式（`export const` 里实测零箭头函数，0/132）；标量字面量初始化式同样不产符号。
///
/// 三形态返回集均为空（`f` / `g` / `h` 连类型面也不占位）：箭头分支已被删除，不是
/// 「抽出来再过滤」。
#[test]
fn extract_file的箭头函数不作操作() {
    for source in [
        "export const f = (a: string) => a;\n",
        "export const g = async (x: string): Promise<string> => x;\n",
        "export const h = function inner() {};\n",
    ] {
        let symbols = extract(source);
        assert!(
            !symbols
                .iter()
                .any(|symbol| symbol.kind == ApiSymbolKind::Function),
            "箭头 / 函数表达式初始化式 MUST NOT 产操作（{source}）：{symbols:?}"
        );
        for name in ["f", "g", "h"] {
            assert!(
                !symbols.iter().any(|symbol| symbol.name == name),
                "`{name}` 不作为任何符号出现（{source}）：{symbols:?}"
            );
        }
    }

    let literal = extract("export const x = 1;\n");
    assert!(
        !literal
            .iter()
            .any(|symbol| symbol.kind == ApiSymbolKind::Function),
        "字面量初始化式不产函数符号：{literal:?}"
    );
}

/// 边界：`export { … }` / `export * from` / `export type { … } from` → 零符号（本期透传
/// 不产出操作，与 `api-reexport` fence 的表达面无关）。
#[test]
fn extract_file的透传不产符号() {
    for source in [
        "export { a, b } from './x';\n",
        "export * from './y';\n",
        "export type { T } from './z';\n",
    ] {
        let symbols = extract(source);
        assert!(
            symbols.is_empty(),
            "透传形态零符号（{source}）：{symbols:?}"
        );
    }
}

/// 边界（D19-a）：非导出顶层声明**不抽**——`function f() {}` / `const x = 1` /
/// `interface I {}` / `class C {}` / `type T = string` / `enum E { A }`（全无 `export`）
/// → 零符号（取数面 = 导出面；混进私有符号会让 `apiMissing` / `apiUndeclared` 把私有
/// 符号当成公开承诺）。
#[test]
fn extract_file的非导出声明不抽() {
    let cases = [
        "function f() {}\n",
        "const x = 1;\n",
        "interface I {}\n",
        "class C {}\n",
        "type T = string;\n",
        "enum E { A }\n",
    ];
    for source in cases {
        let symbols = extract(source);
        assert!(
            symbols.is_empty(),
            "无 export 的顶层声明零符号（{source}）：{symbols:?}"
        );
    }

    // 混合源：只有 `export` 那条入表，非导出邻居不连带
    let mixed = extract(
        r#"function private_fn() {}
export function public_fn(): string { return ''; }
const private_const = 1;
export interface PublicIface {}
"#,
    );
    let names: Vec<&str> = mixed.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["public_fn", "PublicIface"],
        "只抽导出面（非导出邻居不连带）"
    );
}

/// 边界（D19-a）：`export default` 形态**本期不抽**——对外名字由导入方决定（`import X
/// from` 可任意命名），契约的 op / 类型名须是稳定标识符；反推名字即造第二套命名规则。
#[test]
fn extract_file的export_default不抽() {
    for source in [
        "export default function App() { return null; }\n",
        "export default () => null;\n",
        "export default class X {}\n",
        "export default 42;\n",
    ] {
        let symbols = extract(source);
        assert!(
            symbols.is_empty(),
            "export default 零符号（{source}）：{symbols:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Visibility
// ---------------------------------------------------------------------------

/// 正向（AC-5 / AC-9）：抽取出的符号默认 `External`；`unused` 只由 knip 降级（本取数器
/// 不造第二套死代码扫描）。
#[test]
fn extract_file的visibility默认为external() {
    let symbols = extract(
        r#"export function load(): string { return ''; }
"#,
    );
    assert_eq!(
        symbols[0].visibility,
        Visibility::External,
        "顶层 export 即 external（unused 由 knip 降级）"
    );
}

/// 正向（D19-b / D39 留白口径）：**生产面**值域 **⊆ `{External}`**——顶层 `export`
/// → `External`；同文件只有 `export` 无引用者 → **仍** `External`。产出集中
/// **不出现** `CrateOnly` / `TestOnly`（两值本期留白，判据需跨文件 import 反向事实）
/// 与 `Unused`（唯一产出点在被零调用的 `apply_knip_report` 内 ⇒ 生产面不可达）。
///
/// **接口面**（另一断言面）：把同一批符号喂给 `apply_knip_report` 后值域可扩到
/// `{External, Unused}`——两值域 MUST NOT 混谈：前者是「没跑过降级接口」的实况，
/// 后者是「接口正确」的证据（D39 显式留白）。
#[test]
fn extract_file的visibility值域仅两值() {
    let source_text = r#"export function load(): string { return ''; }
export interface Shape { x: number }
export const View = memo(() => null);
export enum Mode { A }
"#;
    let symbols = extract(source_text);
    assert_eq!(symbols.len(), 4, "四类导出形态各一条：{symbols:?}");

    // 生产面：值域 ⊆ {External}（未跑降级接口）
    for symbol in &symbols {
        assert_eq!(
            symbol.visibility,
            Visibility::External,
            "生产面值域 ⊆ {{External}}：{} 为 {:?}",
            symbol.name,
            symbol.visibility
        );
        for forbidden in [
            Visibility::CrateOnly,
            Visibility::TestOnly,
            Visibility::Unused,
        ] {
            assert_ne!(
                symbol.visibility, forbidden,
                "{} MUST NOT 产 {forbidden:?}（CrateOnly / TestOnly 本期留白、Unused 生产面不可达）",
                symbol.name
            );
        }
    }

    // 接口面：喂 knip 报告后值域扩到 {External, Unused}
    let mut degraded = symbols.clone();
    let report = json!({
        "files": { "src/x.ts": { "exports": [ { "name": "load" }, { "name": "Mode" } ] } }
    });
    apply_knip_report(&mut degraded, Some(&report));
    let degraded_range: Vec<Visibility> = degraded.iter().map(|symbol| symbol.visibility).collect();
    assert!(
        degraded_range.contains(&Visibility::Unused),
        "接口面可产 Unused（apply_knip_report 降级生效）: {degraded:?}"
    );
    assert!(
        degraded_range.contains(&Visibility::External),
        "未列出的符号保持 External（只降级不升级）: {degraded:?}"
    );
    for visibility in &degraded_range {
        assert!(
            matches!(visibility, Visibility::External | Visibility::Unused),
            "接口面值域 ⊆ {{External, Unused}}（实得 {visibility:?}）"
        );
    }
}

/// 异常（D19-b 反例在案）：`.test.` / `.spec.` / `__tests__` / `fixtures` 文件名族**不作**
/// `testOnly` proxy——文件名与引用关系无必然联系（`foo.test.ts` 可只被生产代码 import），
/// 用文件名冒充 `testOnly` 是伪判据。四形态同内容 `export function f() {}` → `visibility`
/// 全为 `External`。
///
/// 源码自证：`extract_file` 入参面恰 `(file, source)`，`ts.rs` 内零 `.test.` 判据
/// （零 `is_test_file` 形参、零 `contains(".test")`），故文件名不可能影响 `visibility`。
#[test]
#[allow(non_snake_case)]
fn extract_file的文件名不作testOnly的proxy() {
    for file in [
        "src/views/foo.test.ts",
        "src/views/foo.spec.ts",
        "src/views/__tests__/a.ts",
        "src/views/fixtures/b.ts",
    ] {
        let symbols = extract_file(file, "export function f(): void {}\n");
        assert_eq!(symbols.len(), 1, "{file} 下一条导出：{symbols:?}");
        assert_eq!(
            symbols[0].visibility,
            Visibility::External,
            "文件名 `{file}` 不作 testOnly proxy（名字与引用关系无必然联系）"
        );
    }

    let source = include_str!("ts.rs");
    assert!(
        !source.contains("is_test_file"),
        "入参面零 `is_test_file` 形参（签名恰 (file, source)）"
    );
    assert!(
        !source.contains("Visibility::TestOnly"),
        "本文件零 TestOnly 产出点（本期留白，D19-b）"
    );
    assert!(
        !source.contains("Visibility::CrateOnly"),
        "本文件零 CrateOnly 产出点（本期留白，D19-b）"
    );
    let signature = source
        .lines()
        .find(|line| line.contains("pub fn extract_file"))
        .unwrap_or_else(|| panic!("extract_file 签名必须可检出"));
    assert_eq!(
        signature.trim(),
        "pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {",
        "签名恰 (file, source)：无测试文件形参"
    );
}

/// 边界（D19-b）：四值**词表封闭**（这一面测词表面，不测产出面）。`Visibility::ALL` 仍是
/// 四值、`testOnly` 仍是词表成员（封闭不变）、`ts.rs` 内零 `TestOnly` 构造点——**MUST NOT**
/// 断言「取数器产得出 `TestOnly`」（本期不产出）。四值词表的逐值 `as_str()` 与去重断言在
/// `extract/mod.rs` 章节，本节只留这一条否定断言。
#[test]
#[allow(non_snake_case)]
fn extract_file不产testOnly的否定断言() {
    assert_eq!(
        Visibility::ALL.len(),
        4,
        "词表封闭不变（两语言共用同一枚举）"
    );

    let source = include_str!("ts.rs");
    assert!(
        !source.contains("Visibility::TestOnly"),
        "取数器 MUST NOT 产 TestOnly（本期留白，D19-b）"
    );
    // 词面仍在枚举上（四值封闭：TestOnly 是词表成员，只是本取数器不产）
    assert!(
        Visibility::ALL
            .iter()
            .any(|value| value.as_str() == "testOnly"),
        "TestOnly 仍是词表成员（封闭四值）"
    );
}

// ---------------------------------------------------------------------------
// apply_knip_report
// ---------------------------------------------------------------------------

/// 正向（AC-6 / R-10）：knip 报告列出 `f` 为未使用导出 → 该符号降为 `Unused`；
/// 未列出的符号保持原值（只降级不升级）。
#[test]
fn apply_knip_report降级未使用导出() {
    let mut symbols = extract(
        "export function f(): string { return ''; }\nexport function g(): string { return ''; }\n",
    );
    let report = json!({
        "files": {
            "src/views/x.ts": {
                "exports": [{"name": "f", "line": 1}]
            }
        }
    });
    apply_knip_report(&mut symbols, Some(&report));

    assert_eq!(
        find(&symbols, "f").visibility,
        Visibility::Unused,
        "knip 报的未使用导出降为 unused"
    );
    assert_eq!(
        find(&symbols, "g").visibility,
        Visibility::External,
        "未列出的符号保持原值（只降级不升级）"
    );
}

/// 边界：`None` / `Some({})` → 符号集与 `visibility` 逐字段不变。
#[test]
fn apply_knip_report无报告不降级() {
    let baseline = extract(
        r#"export function f(): string { return ''; }
"#,
    );

    let mut without = baseline.clone();
    apply_knip_report(&mut without, None);
    assert_eq!(without, baseline, "None → 逐字段不变");

    let mut empty = baseline.clone();
    apply_knip_report(&mut empty, Some(&json!({})));
    assert_eq!(empty, baseline, "空报告 → 逐字段不变");
}

/// 异常：报文形态异常（非对象 / 非 `null` 标量 / 缺 `files` / `exports` 非数组 / 名字
/// 缺失）→ 不 panic、零降级；`exports` 数组项为字符串时同判。
#[test]
fn apply_knip_report报文异常不panic() {
    let baseline = extract(
        r#"export function f(): string { return ''; }
"#,
    );

    let reports = [
        json!([1, 2, 3]),
        json!("just a string"),
        json!(null),
        json!({"files": {}}),
        json!({"files": {"src/views/x.ts": {"exports": "not an array"}}}),
        json!({"files": {"src/views/x.ts": {"exports": [{"no_name": 1}]}}}),
    ];
    for report in reports {
        let mut symbols = baseline.clone();
        apply_knip_report(&mut symbols, Some(&report));
        assert_eq!(symbols, baseline, "异常报文零降级（{report}）");
    }

    let mut as_strings = baseline.clone();
    apply_knip_report(
        &mut as_strings,
        Some(&json!({"files": {"src/views/x.ts": {"exports": ["f"]}}})),
    );
    assert_eq!(
        find(&as_strings, "f").visibility,
        Visibility::Unused,
        "数组项为字符串时同判"
    );
}

// ---------------------------------------------------------------------------
// 参数与返回类型
// ---------------------------------------------------------------------------

/// 正向（AC-5）：可选参数标 `optional`、类型逐项映射；`Promise<T>` 归 `Outcome`；异步
/// 组合可判。
#[test]
fn extract_file的参数与返回类型() {
    let symbols = extract("export function f(a: string, b?: number): boolean { return true; }\n");
    let symbol = find(&symbols, "f");
    assert_eq!(symbol.params.len(), 2);
    assert_eq!(symbol.params[0].ty, ApiType::Str, "string → Str");
    assert!(!symbol.params[0].optional, "必填参数");
    assert_eq!(symbol.params[1].ty, ApiType::Float, "number → Float");
    assert!(symbol.params[1].optional, "`b?:` → optional");

    let asynchronous = extract("export async function g(): Promise<string> { return ''; }\n");
    let symbol = find(&asynchronous, "g");
    assert!(symbol.is_async, "async 标记");
    assert_eq!(
        symbol.returns,
        ApiType::Outcome(Box::new(ApiType::Str)),
        "Promise<string> → Outcome(Str)"
    );

    // 与 `extract::schema_type` 同一逻辑词汇（D43）：TS 侧不产 Rust 侧专有形（`OwnedString`
    // / `Int` 只由 Rust 取数器产出）
    for produced in [&symbols, &asynchronous] {
        for item in produced {
            assert!(
                item.params
                    .iter()
                    .all(|param| !matches!(param.ty, ApiType::OwnedString | ApiType::Int)),
                "TS 取数器不产 `OwnedString` / `Int`（参数位）：{:?}",
                item.params
            );
            assert!(
                !matches!(item.returns, ApiType::OwnedString | ApiType::Int),
                "返回位同口径：{:?}",
                item.returns
            );
        }
    }
}

/// 边界：数组 / `Array<T>` / `Record<K, V>` / 联合 `| null` 的形态映射。
#[test]
fn extract_file的容器类型映射() {
    let source = r#"export function f_list(a: string[]): number[] { return []; }
export function f_generic(a: Array<string>): number { return 0; }
export function f_record(a: Record<string, number>): void {}
export function f_union(a: string | null): void {}
"#;
    let symbols = extract(source);
    assert_eq!(
        find(&symbols, "f_list").params[0].ty,
        ApiType::List(Box::new(ApiType::Str)),
        "string[] → List(Str)"
    );
    assert_eq!(
        find(&symbols, "f_list").returns,
        ApiType::List(Box::new(ApiType::Float)),
        "number[] → List(Float)"
    );
    assert_eq!(
        find(&symbols, "f_generic").params[0].ty,
        ApiType::List(Box::new(ApiType::Str)),
        "Array<T> → List"
    );
    assert_eq!(
        find(&symbols, "f_record").params[0].ty,
        ApiType::Map(Box::new(ApiType::Float)),
        "Record<K, V> → Map(V)"
    );
    assert_eq!(
        find(&symbols, "f_union").params[0].ty,
        ApiType::Optional(Box::new(ApiType::Str)),
        "`string | null` → Optional(Str)"
    );
}

// ---------------------------------------------------------------------------
// source_type 分派 / 异常 / 行号 / 规模
// ---------------------------------------------------------------------------

/// 边界：`.ts` / `.tsx` 接受 TS 语法面，`.js` / `.jsx` 走纯 JS 源型（TS 注解在纯 JS
/// 源上即语法错 → 零符号）；含 JSX 的 `.tsx` 源不 panic。
#[test]
fn extract_file的source_type分派() {
    let typed = "export function load(a: string): string { return a; }\n";
    for file in ["x.ts", "x.tsx"] {
        let symbols = extract_file(file, typed);
        assert_eq!(symbols.len(), 1, "{file} 下 TS 源可抽");
        assert_eq!(symbols[0].name, "load");
    }

    let plain = "export function load(a) { return a; }\n";
    for file in ["x.js", "x.jsx"] {
        let symbols = extract_file(file, plain);
        assert_eq!(symbols.len(), 1, "{file} 下纯 JS 源可抽");
        assert_eq!(symbols[0].name, "load");
    }
    assert!(
        extract_file("x.js", typed).is_empty(),
        "纯 JS 源型不接受 TS 注解（语法错即零符号）"
    );

    let jsx = "export function view(): JSX.Element { return <div />; }\n";
    let symbols = extract_file("src/views/x.tsx", jsx);
    assert_eq!(symbols.len(), 1, "含 JSX 的 .tsx 源不 panic");
    assert_eq!(symbols[0].name, "view");
}

/// 异常：空源 → 空 `Vec`；未闭合括号 / 非法 token → 空 `Vec`；100 KB 单文件不 panic。
#[test]
fn extract_file语法错源与规模() {
    assert!(extract("").is_empty(), "空源零符号");
    assert!(
        extract("export function f( {").is_empty(),
        "未闭合括号 → 解析失败即零符号"
    );
    assert!(
        extract("export function @@() {}").is_empty(),
        "非法 token → 零符号"
    );

    let mut big = String::new();
    for index in 0..3000 {
        big.push_str(&format!(
            "export function op{index}(a: string): string {{ return a; }}\n"
        ));
    }
    assert!(big.len() > 100_000, "夹具应超 100 KB");
    let symbols = extract(&big);
    assert_eq!(symbols.len(), 3000, "大文件不 panic、不截断");
}

/// 边界（int 边界）：首行 `export function` → `line == 1`；第 500 行 → `line == 500`；
/// 500 个 `export function` → 符号数 500、无重复。
#[test]
fn extract_file行号与规模() {
    let first_line = extract("export function f(): void {}\n");
    assert_eq!(first_line[0].line, 1, "首行声明 line == 1");

    let mut padded = String::new();
    for index in 0..499 {
        padded.push_str(&format!("// 填充 {index}\n"));
    }
    padded.push_str("export function f(): void {}\n");
    let symbols = extract(&padded);
    assert_eq!(find(&symbols, "f").line, 500, "第 500 行声明 line == 500");

    let mut many = String::new();
    for index in 0..500 {
        many.push_str(&format!("export function op{index}(): void {{}}\n"));
    }
    let symbols = extract(&many);
    assert_eq!(symbols.len(), 500, "500 条无截断");
    let mut names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 500, "500 条无重复");
}

// ---------------------------------------------------------------------------
// 源码自证：取数面 = 导出面、值域留白、文件名族不作 proxy（AC-20 ⑤ / ④ / ⑥）
// ---------------------------------------------------------------------------

/// 边界（AC-20 ⑤）：TS 取数面 = 导出面——`ts.rs` 内 `Statement::` 分支**恰 1 条**
/// （`ExportDeclaration`，内部交 `collect_declaration` 分类）；六个非导出顶层分支
/// 与 `ArrowFunctionExpression` / `ExportDefault` **全零命中**（本期留白）。
#[test]
fn ts取数面为导出面且非导出分支零命中() {
    let source = include_str!("ts.rs");
    assert_eq!(
        source.matches("Statement::").count(),
        1,
        "`Statement::` 分支恰一条（取数面 = 导出面）"
    );
    assert!(
        source.contains("Statement::ExportDeclaration"),
        "唯一分支是 `ExportDeclaration`"
    );

    for forbidden in [
        "Statement::FunctionDeclaration",
        "Statement::ClassDeclaration",
        "Statement::VariableDeclaration",
        "Statement::TSTypeAliasDeclaration",
        "Statement::TSInterfaceDeclaration",
        "Statement::TSEnumDeclaration",
        "ArrowFunctionExpression",
        "ExportDefault",
    ] {
        assert!(
            !source.contains(forbidden),
            "`ts.rs` MUST NOT 命中 `{forbidden}`（非导出顶层分支 / 箭头函数 / export default 全留白）"
        );
    }

    // 行为面：非导出顶层声明与 export default 均零符号（与本文件既有同名用例互证）
    for probe in [
        "function f() {}\n",
        "const x = 1;\n",
        "export default function App() { return null; }\n",
        "export const h = (a: string) => a;\n",
    ] {
        assert!(
            extract(probe).is_empty(),
            "非导出面 / export default / 箭头函数 const 零符号：{probe:?}"
        );
    }
}

/// 边界（AC-20 ④，TS 侧留白）：`ts.rs` 内 `CrateOnly` / `TestOnly` **零命中**（两值
/// 本期留白），`pub fn apply_knip_report` **恰 1 处**（`unused` 的唯一降级来源）。
#[test]
fn ts侧visibility留白两值且降级单点() {
    let source = include_str!("ts.rs");
    for (label, needle) in [
        ("CrateOnly", "Visibility::CrateOnly"),
        ("TestOnly", "Visibility::TestOnly"),
    ] {
        assert_eq!(
            source.matches(needle).count(),
            0,
            "`ts.rs` 零 `{label}` 产出点（本期留白，D19-b）"
        );
    }
    assert_eq!(
        source.matches("pub fn apply_knip_report").count(),
        1,
        "`apply_knip_report` 恰一处（unused 的唯一降级来源）"
    );
    let unused_code_lines = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("Visibility::Unused"))
        .count();
    assert_eq!(
        unused_code_lines, 1,
        "`Unused` 的产出点只在 apply_knip_report 内恰一处（注释不计）"
    );
    // `External` 的产出点即导出面过滤（本文件两处：函数 / 类型各自的符号构造）
    assert!(
        source.matches("Visibility::External").count() >= 1,
        "导出面过滤即 `External` 产出点"
    );
}

/// 异常（AC-20 ⑥ / D19-b 反例在案）：产品侧 `ts.rs` 内 `.test.` / `.spec.` /
/// `__tests__` / `fixtures` 四族**零非注释命中**；`used_by` 在产品 `.rs` 内
/// **零命中**（本期留白，需 import 反向图）。
#[test]
fn ts侧文件名族与used_by作零非注释命中() {
    let source = include_str!("ts.rs");
    for family in [".test.", ".spec.", "__tests__", "fixtures", "used_by"] {
        // 非注释命中计数（doc 注释里的禁令说明不算产出点）
        let hits = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .filter(|line| line.contains(family))
            .count();
        assert_eq!(
            hits, 0,
            "`ts.rs` 内 `{family}` 非注释命中须为 0（文件名族不作 proxy / used_by 留白）"
        );
    }

    // 全 `api/` 产品面同样零 `used_by`（留白是全局口径，不只 ts.rs）
    for (path, product) in [
        ("extract/mod.rs", include_str!("mod.rs")),
        ("extract/rust.rs", include_str!("rust.rs")),
        ("extract/ts.rs", source),
        ("check.rs", include_str!("../check.rs")),
        ("../mod.rs", include_str!("../mod.rs")),
    ] {
        assert!(
            !product.contains("used_by"),
            "产品 `.rs` 内零 `used_by`（本期留白）：{path}"
        );
    }
}

// ---------------------------------------------------------------------------
// D39 显式留白：`apply_knip_report` 零产品调用者
// ---------------------------------------------------------------------------

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

/// 边界（**D39** 显式留白）：源码自证——产品 `.rs`（滤 `*_test.rs`）内对
/// `apply_knip_report(` 的调用**零命中**（只有定义）；`api/mod.rs` 与
/// `crates/tools/archi-api-gen/src/main.rs` 内 `knip` 一词**各零命中**；
/// `Visibility::Unused` 的产出点唯一（本函数体内），故生产面不可达；两处 doc 各写明
/// 「本期生产面不产出 `Unused`（knip 未接线）+ 零调用者的预留接口」。
#[test]
fn apply_knip_report零产品调用者() {
    // ① 产品面零调用者（定义行不算）
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut src_tauri = manifest.clone();
    for _ in 0..3 {
        src_tauri.pop();
    }
    let mut hits: Vec<(String, String)> = Vec::new();
    scan_product_rs(&src_tauri.join("crates"), &mut hits);
    scan_product_rs(&src_tauri.join("src"), &mut hits);
    let callers: Vec<&(String, String)> = hits
        .iter()
        .filter(|(path, line)| {
            !path.ends_with("api/extract/ts.rs")
                && !line.trim_start().starts_with("//")
                && line.contains("apply_knip_report(")
        })
        .collect();
    assert!(
        callers.is_empty(),
        "`apply_knip_report` 零产品调用者（D39 显式留白，不是缺陷）：{callers:?}"
    );

    // ② `api/mod.rs` 与 CLI 内 `knip` 各零命中（bootstrap 不读 knip）
    assert_eq!(
        include_str!("../mod.rs").matches("knip").count(),
        0,
        "`api/mod.rs` 内零 `knip`（bootstrap 不读）"
    );
    let main_rs = src_tauri.join("crates/tools/archi-api-gen/src/main.rs");
    let main_source = std::fs::read_to_string(&main_rs).expect("CLI 源文在案");
    assert_eq!(
        main_source.matches("knip").count(),
        0,
        "CLI 内零 `knip`（D39：人工驱动，不由工具调）"
    );

    // ③ `Visibility::Unused` 的产出点唯一（本文件体内，注释不计）
    let source = include_str!("ts.rs");
    let production_lines = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("Visibility::Unused"))
        .count();
    assert_eq!(
        production_lines, 1,
        "`Unused` 的产出点唯一（`apply_knip_report` 体内），故生产面不可达"
    );

    // ④ 两处 doc 各写明留白（枚举 doc 与模块 doc）
    let extract_source = include_str!("mod.rs");
    for (label, text) in [
        ("extract/mod.rs", extract_source),
        ("extract/ts.rs", source),
    ] {
        assert!(
            text.contains("未接线") || text.contains("零产品调用者"),
            "`{label}` 的 doc 写明留白（MUST NOT 用「已实现」掩盖）：{label}"
        );
    }

    // ⑤ 行为对照：`bootstrap` 的草稿文本内零 `unused` 词
    let temp = std::env::temp_dir().join(format!(
        "archi-api-ts-knip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join(".dev-team/architecture/models")).expect("建模型目录");
    std::fs::write(
        temp.join(".dev-team/architecture/models/01-test.toml"),
        crate::api::mod_test::declare_ancestors(
            "[element.\"App.Core.Ts\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"src/views/x\"]\n",
        ),
    )
    .expect("写模型");
    std::fs::create_dir_all(temp.join("src/views/x")).expect("建源码目录");
    std::fs::write(
        temp.join("src/views/x/index.ts"),
        "export function load(): string { return ''; }\n",
    )
    .expect("写源码");
    let draft = crate::api::bootstrap(&temp, "App.Core.Ts").expect("起草应成功");
    assert!(
        !draft.contains("unused"),
        "`bootstrap` 草稿内零 `unused` 词（Unused 生产面不可达）：\n{draft}"
    );
    let _ = std::fs::remove_dir_all(&temp);
}
