//! Rust 声明抽取（`syn`，AST 归一——MUST NOT 用正则抽签名：多行签名占 `pub fn` 的
//! 30–62%，单行 `grep` 抽取会截半）。
//!
//! **单文件入口 [`extract_file`]**：只抽一个文件里的 `pub` 项（bootstrap 的逐文件面）。
//! **整包入口 [`extract_crate`]**：从实现入口出发沿 `mod` 声明递归走模块树（这是判据
//! 「代码导出集」的唯一取数面——一个 crate 的公开面散在 `lib.rs` 与它声明的各模块
//! 文件里，只看入口文件会把大部分操作判成缺失）。
//!
//! 抽取面：`pub struct` → 类型、`pub trait` → trait、`pub enum` → 枚举、自由 `pub fn` 与
//! `impl` 内 `pub fn` → 函数（`owner` = 接收类型名）。**构造器如实抽名、不过滤**（过滤
//! 只在判据与渲染侧经 `is_constructor`）。
//!
//! `visibility` 四值判据：`pub mod` 链可达（或名字出现在 crate 根的 `pub use` 白名单里）
//! → `External`；`pub(crate)` / 私有 `mod` 内的 `pub` → `CrateOnly`；`#[cfg(test)]` 子树 →
//! `TestOnly`；`Unused` 由调用方按引用事实降级（本模块不造第二套死代码扫描）。
//!
//! **inherent 方法的可达性按接收类型判**（本仓真实形态：`impl LocalArchiCheck` 写在私有
//! 模块里，类型本身经 `pub use` 出线）：类型 `External` 时其 `pub` 方法同为 `External`。
//!
//! **两处取数面的结构前提**（D44，缺任一条则自有生成物的元素上恒假阳）：① `mod` 带
//! `#[path = "…"]` 时按**属性值相对当前模块目录**定位目标文件（生成物落元素根同级即靠它
//! 进符号集）；② crate 根的 `pub use <模>::*` 的通配前缀参与升格（`walk_modules` 走到同名
//! 模块即视其内声明为出线）。
//!
//! **`#[path]` 读盘面与写盘面同一条限根纪律**（D49）：定位走三步——① **反斜杠归一在折叠
//! 之前**（`split('/')` 切不开 `\`，整串会被当单段名原样入栈 ⇒ `Path::join` 的纯拼接不折叠
//! `..` ⇒ Windows 上逐级上溯到观察根之外）；② 分段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即
//! 拒）；③ **`join` 后复核**目标仍在观察根之下（`starts_with(root)`）。任一步不成立即**不
//! 命中该子树**（零 `ApiError` 变体、零 panic）。这与 `scan/rust.rs` 的 `normalize_rel` /
//! `docs.rs` 的 `resolved_within` 是同一条纪律，本处是**同一形态的第三处**——MUST NOT 抽跨
//! crate 公共归一函数（三处入参面不同：本处还要回带「该文件的父目录」作下一层模块目录）。
//!
//! **已知近似**：通配升格只认 crate 根一层（`use a::b::*` 取末段 `b`、只对 crate 直接
//! 子模块命中）；`#[path]` 值越出观察根（绝对路径 / 盘符 / `..` 上溯到根之上）一律不命中；
//! 宏生成的 `mod` / `include!` 仍在射程外。二者都不冒充精确结果——模型里没有的元素即无
//! 路径可派生。
//!
//! 边界：读入口文件与它声明的模块文件（`mod` 定位失败即跳过该子树）；零写盘、零 spawn、
//! 零宏展开。

use std::fs;
use std::path::Path;

use proc_macro2::Span;
use syn::spanned::Spanned;

use super::{ApiParam, ApiSymbol, ApiSymbolKind, ApiType, Visibility};
use crate::api::paths;

/// 单文件抽取（`file` 只用于诊断定位，`crate_name` 保留以便未来区分同名 crate 符号）。
pub fn extract_file(file: &str, source: &str, crate_name: &str) -> Vec<ApiSymbol> {
    let Ok(parsed) = syn::parse_file(source) else {
        // 语法错即零符号（bootstrap 起草的近似误差如实留痕，MUST NOT 猜）
        return Vec::new();
    };
    let mut symbols = Vec::new();
    walk_items(&parsed.items, true, false, &mut symbols);
    let _ = (file, crate_name);
    symbols
}

/// 整包抽取：从实现入口出发走模块树（`root` = 观察根，`entry` = 入口 repo 相对路径）。
///
/// 入口读不到 → 空 `Vec`（缺口由 `apiEntryMissing` 报）；模块定位失败 → 跳过该子树
/// （宏生成 `mod` 的已知近似）。
pub fn extract_crate(root: &Path, entry: &str) -> Vec<ApiSymbol> {
    let entry_path = root.join(entry);
    let Ok(source) = fs::read_to_string(&entry_path) else {
        return Vec::new();
    };
    let Ok(parsed) = syn::parse_file(&source) else {
        return Vec::new();
    };
    // crate 根的 `pub use` 白名单：私有模块内声明的名字经它出线后仍是公开面
    // （具名 `pub use a::B` 与通配 `pub use a::*` 分两条登记——后者只能按模块名升格，
    // 无法枚举出线名字）
    let mut reexported: Vec<String> = Vec::new();
    let mut wildcards: Vec<String> = Vec::new();
    collect_reexports(&parsed.items, &mut reexported, &mut wildcards);

    let mut collected = Collected::default();
    walk_items(&parsed.items, true, false, &mut collected.symbols);
    // 模块树：`mod` 声明（带 `;` 无 body）按 `mod.rs` / `<名>.rs` / `#[path]` 定位后递归
    let module_dir = module_dir(entry);
    walk_modules(
        &parsed.items,
        root,
        &module_dir,
        true,
        false,
        &wildcards,
        &mut collected,
    );

    // 出线名单 = 具名 `pub use` 的导入名 + 通配模块里声明的类型名：升格本身只走
    // `upgrade_reexported` 一处（MUST NOT 在模块递归里另立一套升格）。
    for name in collected.glob_types {
        if !reexported.iter().any(|existing| existing == &name) {
            reexported.push(name);
        }
    }
    upgrade_reexported(&mut collected.symbols, &reexported);
    collected.symbols
}

/// 整包抽取的累加面（符号集 + 通配出线的类型名；打包成一项以免递归参数继续膨胀）。
#[derive(Default)]
struct Collected {
    /// 已抽到的符号（声明序）
    symbols: Vec<ApiSymbol>,
    /// 通配出线的模块内声明的类型名（`pub use <模>::*` 的名字面）
    glob_types: Vec<String>,
}

/// 模块树递归（`mod x;` → `<dir>/x.rs` 或 `<dir>/x/mod.rs`；带 `#[path = "…"]` 时按
/// 属性值定位，D44）。
///
/// `wildcards` 只承载**crate 根**的 `pub use <模>::*` 前缀（递归进下一层即清空——通配升格
/// 只对 crate 直接子模块生效，见「已知近似」段）。命中即把该模块内声明的符号视为出线
/// （`External`）：`pub use x::*` 让 `x` 的公开面整体可达，与 `mod x` 自身的可见性无关。
fn walk_modules(
    items: &[syn::Item],
    root: &Path,
    directory: &str,
    reachable: bool,
    in_test: bool,
    wildcards: &[String],
    collected: &mut Collected,
) {
    for item in items {
        let syn::Item::Mod(item_mod) = item else {
            continue;
        };
        let is_test = in_test || has_cfg_test(&item_mod.attrs);
        let name = item_mod.ident.to_string();
        let glob_exported = wildcards.iter().any(|wildcard| wildcard == &name);
        if let Some((_, nested)) = &item_mod.content {
            // 内联模块：body 就地递归（模块目录仍是当前目录）
            let inner_reachable = glob_exported || (reachable && is_pub(&item_mod.vis));
            let before = collected.symbols.len();
            walk_items(nested, inner_reachable, is_test, &mut collected.symbols);
            collect_glob_types(
                glob_exported,
                &collected.symbols[before..],
                &mut collected.glob_types,
            );
            walk_modules(
                nested,
                root,
                directory,
                inner_reachable,
                is_test,
                &[],
                collected,
            );
            continue;
        }
        let located = match path_attribute(&item_mod.attrs) {
            // `#[path]` 存在但文件读不到 ⇒ 跳过该子树（与既有「模块定位失败即跳过」同口径）
            Some(value) => locate_declared_module(root, directory, &value, &name),
            None => locate_module(root, directory, &name),
        };
        let Some((file, next_dir)) = located else {
            continue;
        };
        let Ok(source) = fs::read_to_string(root.join(&file)) else {
            continue;
        };
        let Ok(parsed) = syn::parse_file(&source) else {
            continue;
        };
        let inner_reachable = glob_exported || (reachable && is_pub(&item_mod.vis));
        let before = collected.symbols.len();
        walk_items(
            &parsed.items,
            inner_reachable,
            is_test,
            &mut collected.symbols,
        );
        collect_glob_types(
            glob_exported,
            &collected.symbols[before..],
            &mut collected.glob_types,
        );
        walk_modules(
            &parsed.items,
            root,
            &next_dir,
            inner_reachable,
            is_test,
            &[],
            collected,
        );
    }
}

/// 通配出线的模块里声明的类型名入出线名单（`pub use <模>::*` 无法枚举导出名，故取
/// 「该模块声明的类型」这一最小形态；只对 `glob_exported` 的模块生效）。
fn collect_glob_types(glob_exported: bool, declared: &[ApiSymbol], glob_types: &mut Vec<String>) {
    if !glob_exported {
        return;
    }
    for symbol in declared {
        if symbol.kind == ApiSymbolKind::Function {
            continue;
        }
        if !glob_types.iter().any(|name| name == &symbol.name) {
            glob_types.push(symbol.name.clone());
        }
    }
}

/// `#[path = "…"]` 的属性值（`Meta::NameValue` 的字符串字面量；缺省或非字符串 → `None`）。
///
/// 属性值本身是**字面量**（Rust 属性值不能求值），故本函数只读取、不推导：值的形态由
/// 写入口行的人负责，与入口形态的等值性由 `paths::mod_path_attr` 与 `paths_test` 双守。
fn path_attribute(attributes: &[syn::Attribute]) -> Option<String> {
    for attribute in attributes {
        if !attribute.path().is_ident("path") {
            continue;
        }
        let syn::Meta::NameValue(named) = &attribute.meta else {
            return None;
        };
        let syn::Expr::Lit(literal) = &named.value else {
            return None;
        };
        let syn::Lit::Str(text) = &literal.lit else {
            return None;
        };
        return Some(text.value());
    }
    None
}

/// `#[path]` 声明模块的定位：属性值**相对当前模块目录**解析（`../../archi.i.rs` 形态即
/// 生成物落在元素根同级）；命中即取该文件、模块目录为「该文件的父目录」。
///
/// **三步限根**（D49，与写盘面同一条纪律，见模块 doc）：① 反斜杠归一见先于折叠展开；
/// ② 分段折叠；③ `root.join(candidate)` 必须 `starts_with(root)`。任一步不成立 → `None`
/// （不命中该子树）。绝对路径 / `~` / 盘符一并拒收——它们会让 `Path::join` 丢弃 base。
fn locate_declared_module(
    root: &Path,
    directory: &str,
    value: &str,
    name: &str,
) -> Option<(String, String)> {
    // ① 反斜杠归一**在折叠之前**：`split('/')` 切不开 `\`，不归一则 `..\..\x.rs` 整串被当
    // 单段名入栈，`Path::join` 的纯拼接把 `..` 原样带到 Windows 的路径解析里
    let unified = value.replace('\\', "/");
    if unified.starts_with('/') || unified.starts_with('~') || unified.contains(':') {
        return None;
    }
    let mut stack: Vec<&str> = directory.split('/').filter(|seg| !seg.is_empty()).collect();
    // ② 分段折叠（前向 `/` 形态的深度已由 `stack` 初值即上界自限：`pop()?` 在栈空即返回，
    // 故任意多个 `..` 只能弹到「至少保留 1 段」——MUST NOT 加严成「限制 `..` 个数」）
    for segment in unified.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            other => stack.push(other),
        }
    }
    if stack.is_empty() {
        return None;
    }
    let file = stack.join("/");
    // ③ `join` 后复核：`starts_with` 按分量比较，等价于 `strip_prefix(..).is_ok()`（`join`
    // 遇绝对路径 / 盘符会丢弃 base，此处复核它没有发生）
    if !root.join(&file).starts_with(root) {
        return None;
    }
    // 子模块目录 = 该文件的父目录 + 模块名（多文件模块形态；与 `locate_module` 的
    // `<dir>/<name>/mod.rs` 一支同口径）
    let parent = file.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let next_dir = if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    };
    Some((file, next_dir))
}

/// `mod <name>;` 的文件定位：`<dir>/<name>.rs` 或 `<dir>/<name>/mod.rs`；返回（repo 相对
/// 文件路径，该模块的子模块目录）。
fn locate_module(root: &Path, directory: &str, name: &str) -> Option<(String, String)> {
    let flat = format!("{directory}/{name}.rs");
    if root.join(&flat).is_file() {
        return Some((flat, format!("{directory}/{name}")));
    }
    let nested = format!("{directory}/{name}/mod.rs");
    if root.join(&nested).is_file() {
        return Some((nested, format!("{directory}/{name}")));
    }
    None
}

/// 入口文件的模块目录（`<dir>/src/lib.rs` → `<dir>/src`、`<dir>/mod.rs` → `<dir>`、
/// `<dir>.rs` → `<dir>` 的父目录）。
fn module_dir(entry: &str) -> String {
    let normalized = paths::normalize_root(entry);
    match normalized.rsplit_once('/') {
        Some((parent, "lib.rs" | "mod.rs")) => parent.to_owned(),
        Some((parent, _)) => parent.to_owned(),
        None => String::new(),
    }
}

/// crate 根的 `pub use` 白名单：具名导出名进 `names`，通配（`pub use <模>::*`）的模块
/// 末段进 `wildcards`（两条出线形态分别登记——通配无法枚举出线名字，只能按模块升格）。
fn collect_reexports(items: &[syn::Item], names: &mut Vec<String>, wildcards: &mut Vec<String>) {
    for item in items {
        let syn::Item::Use(item_use) = item else {
            continue;
        };
        if !is_pub(&item_use.vis) {
            continue;
        }
        let mut collected: Vec<String> = Vec::new();
        flatten_use_tree(&item_use.tree, &mut Vec::new(), &mut collected, wildcards);
        for name in collected {
            if !names.iter().any(|existing| existing == &name) {
                names.push(name);
            }
        }
    }
}

/// use 树展开（`pub use a::b::{C, D}` → `C` / `D`）。
///
/// `UseTree::Glob` **不再直接丢弃**（D44 缺口②）：`pub use <模>::*` 把该模块的公开面整体
/// 出线，而通配无法枚举名字 ⇒ 只登记该 use 树的**模块路径末段**（`use a::b::*` → `b`，
/// 等价的最小形态），由 [`walk_modules`] 在推进模块树时按「通配前缀命中即升格」处理。
/// **只对 `Glob` 产生的通配生效**——具名 `pub use` 的精确语义零变化。
fn flatten_use_tree(
    tree: &syn::UseTree,
    stack: &mut Vec<String>,
    out: &mut Vec<String>,
    wildcards: &mut Vec<String>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            stack.push(path.ident.to_string());
            flatten_use_tree(&path.tree, stack, out, wildcards);
            stack.pop();
        }
        syn::UseTree::Name(name) => out.push(name.ident.to_string()),
        syn::UseTree::Rename(rename) => out.push(rename.rename.to_string()),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(item, stack, out, wildcards);
            }
        }
        syn::UseTree::Glob(_) => {
            if let Some(prefix) = stack.last() {
                if !wildcards.iter().any(|existing| existing == prefix) {
                    wildcards.push(prefix.clone());
                }
            }
        }
    }
}

/// 白名单名字的可见性升级：`pub use` 出的类型与其 `pub` 方法升为 `External`（inherent
/// 方法的可达性按接收类型判）。
///
/// 通配（`pub use <模>::*`）产生的出线**不在此处理**：它无法枚举名字，改由
/// [`walk_modules`] 在声明处以「通配前缀命中即可达」生效（同一事实的另一种最小形态，
/// MUST NOT 两处各写一套升格）。
fn upgrade_reexported(symbols: &mut [ApiSymbol], reexported: &[String]) {
    if reexported.is_empty() {
        return;
    }
    let external_types: Vec<String> = symbols
        .iter()
        .filter(|symbol| symbol.kind != ApiSymbolKind::Function)
        .filter(|symbol| reexported.iter().any(|name| name == &symbol.name))
        .map(|symbol| symbol.name.clone())
        .collect();
    for symbol in symbols.iter_mut() {
        if reexported.iter().any(|name| name == &symbol.name) {
            symbol.visibility = Visibility::External;
        }
        if let Some(owner) = &symbol.owner {
            if external_types.iter().any(|name| name == owner) {
                symbol.visibility = Visibility::External;
            }
        }
    }
}

/// 递归遍历项（`reachable` = `pub mod` 链可达；`in_test` = `#[cfg(test)]` 子树内）。
fn walk_items(items: &[syn::Item], reachable: bool, in_test: bool, out: &mut Vec<ApiSymbol>) {
    for item in items {
        match item {
            syn::Item::Struct(item_struct) => {
                push_type(
                    &item_struct.vis,
                    &item_struct.ident,
                    item_struct.span(),
                    ApiSymbolKind::Type,
                    reachable,
                    in_test,
                    out,
                );
            }
            syn::Item::Trait(item_trait) => {
                push_type(
                    &item_trait.vis,
                    &item_trait.ident,
                    item_trait.span(),
                    ApiSymbolKind::Trait,
                    reachable,
                    in_test,
                    out,
                );
            }
            syn::Item::Enum(item_enum) => {
                push_type(
                    &item_enum.vis,
                    &item_enum.ident,
                    item_enum.span(),
                    ApiSymbolKind::Enum,
                    reachable,
                    in_test,
                    out,
                );
            }
            syn::Item::Fn(item_fn) => {
                if let Some(symbol) = function_symbol(
                    &item_fn.vis,
                    &item_fn.sig,
                    None,
                    item_fn.span(),
                    reachable,
                    in_test,
                ) {
                    out.push(symbol);
                }
            }
            syn::Item::Impl(item_impl) => {
                let owner = type_name(&item_impl.self_ty);
                for impl_item in &item_impl.items {
                    let syn::ImplItem::Fn(method) = impl_item else {
                        continue;
                    };
                    if let Some(symbol) = function_symbol(
                        &method.vis,
                        &method.sig,
                        owner.as_deref(),
                        method.span(),
                        reachable,
                        in_test,
                    ) {
                        out.push(symbol);
                    }
                }
            }
            syn::Item::Mod(item_mod) => {
                let Some((_, nested)) = &item_mod.content else {
                    continue;
                };
                let is_test = in_test || has_cfg_test(&item_mod.attrs);
                let inner_reachable = reachable && is_pub(&item_mod.vis);
                walk_items(nested, inner_reachable, is_test, out);
            }
            _ => {}
        }
    }
}

/// 类型类符号入表（`pub` 才有资格；`pub(crate)` / 私有 `mod` 内的 `pub` 归 `CrateOnly`）。
fn push_type(
    vis: &syn::Visibility,
    ident: &syn::Ident,
    span: Span,
    kind: ApiSymbolKind,
    reachable: bool,
    in_test: bool,
    out: &mut Vec<ApiSymbol>,
) {
    let Some(visibility) = visibility_of(vis, reachable, in_test) else {
        return;
    };
    out.push(ApiSymbol {
        name: ident.to_string(),
        owner: None,
        kind,
        params: Vec::new(),
        returns: ApiType::Unknown("()".to_owned()),
        is_async: false,
        line: line_of(span),
        visibility,
    });
}

/// 函数符号（`pub` / `pub(crate)` 才有资格；私有函数不进 API 面）。
fn function_symbol(
    vis: &syn::Visibility,
    sig: &syn::Signature,
    owner: Option<&str>,
    span: Span,
    reachable: bool,
    in_test: bool,
) -> Option<ApiSymbol> {
    let visibility = visibility_of(vis, reachable, in_test)?;
    let params = sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(typed) => Some(ApiParam {
                name: param_name(typed),
                ty: convert_type(&typed.ty),
                optional: matches!(convert_type(&typed.ty), ApiType::Optional(_)),
            }),
            // `self` 接收器不参与签名比对（方法归属由 `owner` 表达）
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    Some(ApiSymbol {
        name: sig.ident.to_string(),
        owner: owner.map(str::to_owned),
        kind: ApiSymbolKind::Function,
        params,
        returns: match &sig.output {
            syn::ReturnType::Default => ApiType::Unknown("()".to_owned()),
            syn::ReturnType::Type(_, ty) => convert_type(ty),
        },
        is_async: sig.asyncness.is_some(),
        line: line_of(span),
        visibility,
    })
}

/// `pub` 判据（`pub mod` 链可达性的唯一判词）。
fn is_pub(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// 可见性判据（`pub` 且链可达 → `External`；`pub` 但链断 → `CrateOnly`；其余 `None`）。
fn visibility_of(vis: &syn::Visibility, reachable: bool, in_test: bool) -> Option<Visibility> {
    if in_test {
        return matches!(vis, syn::Visibility::Public(_)).then_some(Visibility::TestOnly);
    }
    match vis {
        syn::Visibility::Public(_) => Some(if reachable {
            Visibility::External
        } else {
            Visibility::CrateOnly
        }),
        syn::Visibility::Restricted(restricted) => restricted
            .path
            .is_ident("crate")
            .then_some(Visibility::CrateOnly),
        syn::Visibility::Inherited => None,
    }
}

/// 参数名（模式为 `ident` 时取名字，其余退化为稳定占位）。
fn param_name(typed: &syn::PatType) -> String {
    match typed.pat.as_ref() {
        syn::Pat::Ident(ident) => ident.ident.to_string(),
        _ => "arg".to_owned(),
    }
}

/// `#[cfg(test)]` 属性判据（内联测试子树的唯一标记；`cfg` 列表里出现 `test`
/// 标识符即命中）。
fn has_cfg_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        let Ok(list) = attribute.meta.require_list() else {
            return false;
        };
        if !list.path.is_ident("cfg") {
            return false;
        }
        list.tokens
            .clone()
            .into_iter()
            .any(|token| matches!(token, proc_macro2::TokenTree::Ident(ident) if ident == "test"))
    })
}

/// 类型路径的末段名（`impl` 接收类型 / `$defs` 名字的来源）。
fn type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => type_name(&reference.elem),
        _ => None,
    }
}

/// `syn::Type` → 逻辑类型（AST 归一；无法归类即 `Unknown(原文)`）。
fn convert_type(ty: &syn::Type) -> ApiType {
    match ty {
        syn::Type::Reference(reference) => match reference.elem.as_ref() {
            syn::Type::Slice(slice) => ApiType::List(Box::new(convert_type(&slice.elem))),
            inner => {
                // `&str` 的 AST 形态是 `Type::Path(str)`，由下面的 Path 分支判成 `Str`
                match convert_type(inner) {
                    ApiType::Named(name) if name == "str" => ApiType::Str,
                    other => other,
                }
            }
        },
        syn::Type::Slice(slice) => ApiType::List(Box::new(convert_type(&slice.elem))),
        syn::Type::Tuple(tuple) if tuple.elems.is_empty() => ApiType::Unknown("()".to_owned()),
        syn::Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return ApiType::Unknown(quote_like(ty));
            };
            let name = segment.ident.to_string();
            match name.as_str() {
                "String" => ApiType::OwnedString,
                "str" => ApiType::Str,
                "bool" => ApiType::Bool,
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
                | "u128" | "usize" => ApiType::Int,
                "f32" | "f64" => ApiType::Float,
                "Vec" => ApiType::List(Box::new(first_argument(segment))),
                "BTreeMap" | "HashMap" => ApiType::Map(Box::new(second_argument(segment))),
                "Option" => ApiType::Optional(Box::new(first_argument(segment))),
                "Result" => ApiType::Outcome(Box::new(first_argument(segment))),
                _ => ApiType::Named(name),
            }
        }
        _ => ApiType::Unknown(quote_like(ty)),
    }
}

/// 泛型参数里第 1 个类型（不可判 → `Unknown`）。
fn first_argument(segment: &syn::PathSegment) -> ApiType {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return ApiType::Unknown(segment.ident.to_string());
    };
    arguments
        .args
        .iter()
        .find_map(|argument| match argument {
            syn::GenericArgument::Type(ty) => Some(convert_type(ty)),
            _ => None,
        })
        .unwrap_or_else(|| ApiType::Unknown(segment.ident.to_string()))
}

/// 泛型参数里第 2 个类型（`Map` 的值位；不可判 → `Unknown`）。
fn second_argument(segment: &syn::PathSegment) -> ApiType {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return ApiType::Unknown(segment.ident.to_string());
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(ty) => Some(convert_type(ty)),
            _ => None,
        })
        .nth(1)
        .unwrap_or_else(|| ApiType::Unknown(segment.ident.to_string()))
}

/// 类型原文的近似串（`Unknown` 留痕用；不含换行与多余空白）。
///
/// 只做可读留痕，MUST NOT 被当作签名解析面（归一比对走 [`ApiType`]）。
fn quote_like(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<String>>()
            .join("::"),
        syn::Type::Reference(reference) => format!("&{}", quote_like(&reference.elem)),
        syn::Type::Tuple(tuple) => {
            let inner = tuple
                .elems
                .iter()
                .map(quote_like)
                .collect::<Vec<String>>()
                .join(", ");
            format!("({inner})")
        }
        syn::Type::Slice(slice) => format!("[{}]", quote_like(&slice.elem)),
        _ => "?".to_owned(),
    }
}

/// 行号（1 起始；`proc-macro2` 的 `span-locations` 特性在 workspace 统一开启）。
fn line_of(span: Span) -> u32 {
    span.start().line as u32
}
