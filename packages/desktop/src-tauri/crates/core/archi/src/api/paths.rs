//! 契约与生成物路径单点：`<root>` 归一、四类路径派生、入口查找与形态判定、
//! `#[path]` 值与 TS import specifier、跨域 `$ref` 相对前缀。
//!
//! **全部路径运算，零 IO**（[`entry_form`] 的存在性探测是唯一例外——它必须看盘才能
//! 回答「入口在哪」）。契约目录一律经 [`foundation::layout::archi_api_dir`] 派生：
//! 本文件 MUST NOT 行内拼 `api` 段，MUST NOT 手算 `../` 层数（值形态由本文件单点
//! 产出，消费方零字面量）。
//!
//! **路径形态校验是四类派生的共同前置**：[`contract_path`] / [`schema_path`] /
//! [`generated_path`] / [`entry_form`] 一律**可失败**（非法形态 → `None`）。理由是
//! `Path::join` 的语义：base 被绝对路径 / 盘符**替换**、`..` 段会**上溯**——放行即
//! 让「证据在盘」的探测越出仓库（任意文件读），也让工具 crate 的写盘面落成任意文件
//! 写。判据与 `docs::normalize_rel` / `scan::rust::normalize_rel` 同一条纪律（repo
//! 相对：`/` 分隔、无绝对前缀、无盘符、无 `.` / `..` / 空段），MUST NOT 靠调用方自觉。

use std::path::{Path, PathBuf};

use foundation::layout::archi_api_dir;

use crate::dto::ArchiElement;

/// 元素根路径顶层分组名（`<Domain>` 缺省——单段 FQN 的契约落此目录）。
const ROOT_DOMAIN: &str = "Root";

/// 索引文件名（`<archi_api_dir>/INDEX.toml`，全局唯一一份）。
const INDEX_FILE_NAME: &str = "INDEX.toml";

/// 入口形态（封闭三值）：决定 `#[path]` 值与 TS import specifier 的形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryForm {
    /// `<dir>/src/lib.rs`（crate 形态）
    Crate,
    /// `<dir>/mod.rs` / `<dir>/index.ts(x)`（目录形态）
    SubdirModule,
    /// 单文件形态（元素根本身即模块文件）
    SingleFile,
}

/// 生成物种类（封闭二值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedKind {
    /// `<root>.i.rs`
    Rust,
    /// `<root>.i.ts`
    TypeScript,
}

impl GeneratedKind {
    /// 生成物文件扩展名。
    fn extension(self) -> &'static str {
        match self {
            GeneratedKind::Rust => "rs",
            GeneratedKind::TypeScript => "ts",
        }
    }
}

/// 源文件语言（封闭三值；按扩展名判定）。
///
/// 两个消费面对 `Unknown` 的处置不同，且都由本文件的单点函数回答：
/// - **取数面**（[`crate::api::extract`]）：`Unknown` → 零符号（不猜测语法）；
/// - **生成物面**（[`generated_kind`]）：`Unknown` → [`GeneratedKind::Rust`]（`<root>` 不带
///   前端扩展名即 Rust 形态，判据见该函数 doc）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// `.rs`
    Rust,
    /// `.ts` / `.tsx` / `.js` / `.jsx`
    TypeScript,
    /// 其余（取数面按「零符号」处置；生成物面按 [`generated_kind`] 的判据）
    Unknown,
}

/// 元素根路径归一：`\` → `/`、循环剥前导 `./`、去尾 `/`。
///
/// **MUST NOT 去文件扩展名**——`<root>` 是模块身份，前端单文件元素的 root 保留
/// `.tsx` 后入口四分支必然零命中（如实报 `apiEntryMissing`，不为凑绿去扩展名）。
pub fn normalize_root(raw: &str) -> String {
    let unified = raw.replace('\\', "/");
    let mut trimmed = unified.as_str();
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest;
    }
    trimmed.trim_end_matches('/').to_owned()
}

/// `<root>` 末段（`crates/infra/archi` → `archi`、`views/welcome-view.tsx` →
/// `welcome-view.tsx`）——生成物文件名与 `#[path]` 值都用它，MUST NOT 取 FQN 末段。
///
/// **纯派生、零形态判据**：文件名与 `#[path]` 值对任意末段都成立
/// （`welcome-view.tsx.i.ts` 是合法文件名），故本函数不校验字符集、零改名。**末段进目标
/// 语言源码时的形态闸按语言分列**：Rust 侧经 `is_module_segment` 拒绝（**形态且非
/// 关键字**，语法位置只收标识符）、TS 侧经 `emit::ts::ts_string_literal` 转义（字符串字面量
/// 任意文本都有安全形态、**零筛选**）。`checked_root` 只判整条 `<root>` 的段结构、**不判
/// 末段字符集**，故 MUST NOT 拿它当「末段可进源码」的形态依据。
pub fn module_name(element_root: &str) -> String {
    let normalized = normalize_root(element_root);
    let tail = normalized.rsplit('/').next().unwrap_or("");
    tail.to_owned()
}

/// Rust 关键字封闭集（**38 严格关键字 + 13 保留字 = 51**）：[`is_module_segment`] 的第二
/// 判据项。
///
/// 定值依据是**行为判据**——逐条以 `rustc --edition 2021` 试 `mod <词>;` 是否报错，因此
/// `async` / `await` / `dyn` / `try` 这类跨 edition 分档的条目也一并纳入（宁可多拒：被拒的
/// 合法形态降级为注释，而放行一个关键字会产出不可编译文本）。**MUST NOT 把本表散进
/// `matches!` 长臂**（那样无法逐值对拍），**MUST NOT 引入第二份关键字表**（`contract::verify`
/// 的闸一判契约文本名字、只管字符集，关键字不在其射程）。
///
/// 前 38 项是**严格关键字**（含 `async` / `await` / `dyn` 这三个 2018 edition 起升为关键字
/// 的条目），后 13 项是**保留字**。两段之间不插行注释——`rustfmt` 会把行尾注释挂到前一项
/// 上，使逐值对拍（`grep` 一段一项）失真。
const RUST_KEYWORDS: [&str; 51] = [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// `module_name` 的末段是否可作 **Rust 模块段**——「路径派生的模块名进 Rust 源码」的唯一
/// 形态闸（`pub use <模块路径>::<名>` 的模块段是**语法位置**）。
///
/// 判据是**合取二项**：① 形态 `[A-Za-z_][A-Za-z0-9_]*`（**空串不合法**）；② **不在
/// [`RUST_KEYWORDS`] 内**。只判字符集不够——`crate` / `self` / `super` / `fn` / `mod` /
/// `type` / `impl` / `use` / `loop` 一类全部通过正则，而 `pub use crate::loop::X;` **不可
/// 编译**（本仓真实形态：`crates/infra/agent/src/sdk/mod.rs` 的 `pub(crate) mod r#loop;`）。
///
/// **只有合法 / 拒绝两态，零归一化、零第二形态**：清洗字符集（`-` → `_`、剥扩展名）就是第二
/// 套命名（D40 明禁），且会制造「静默错配」；产出 `r#<段>` 同样被否——[`module_name`] 的另
/// 一消费面是**文件名**（`<name>.i.ts`）与 **`#[path]` 值**（`"../<name>.i.rs"`），`r#` 在
/// 那两处是错的（`welcome-r#loop.i.ts` 不是合法文件名、`#[path = "../r#loop.i.rs"]` 找不到
/// 文件）。故正解是拒绝：不合法时装配点（`api::module_refs` 的 Rust 分支）令该目标不入透传
/// 查表，渲染器只留 `// 透传自 <FQN>` 注释（与「模型里没有该元素」同一条降级通路）。
///
/// **为什么 TS 侧不用它**：TS 的 specifier 落在**字符串字面量**里（`from '<specifier>'`），
/// 任意文本都有安全形态——经 `emit::ts::ts_string_literal` 转义即可，且带扩展名的末段
/// （`views/architecture.tsx` → `./architecture.tsx.i`）本就是**合法且必需**的 TS 引用形式。
/// 拿本条去筛 TS 会误杀前端 barrel 对单文件元素的合法透传，故两语言的闸**刻意不对称**：
/// Rust 侧拒绝（语法位置）、TS 侧转义（字面量）。
///
/// **与 `contract::verify` 的 `is_identifier`（D45 闸一）事实源不同**：那条判**契约文本里
/// 写的名字**（装载期、`ApiError::Contract`、文案点名 fence 行、只管字符集，**关键字不在其
/// 射程**——契约名不是源码名），本条判**路径派生的模块段**（装配期、降级为注释）。
/// `paths` 是 `api` 的地基层（`contract` / `schema` / `check` / `emit` 全都依赖它），MUST NOT
/// 反向依赖 `contract` 去复用一个判据函数。
pub(crate) fn is_module_segment(token: &str) -> bool {
    let mut characters = token.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    if !characters.all(|character| character.is_ascii_alphanumeric() || character == '_') {
        return false;
    }
    !RUST_KEYWORDS.contains(&token)
}

/// Rust 透传模块路径（`crate::<末段>`）——`pub use` 的模块段单点（D40）。
///
/// `crates/infra/archi` → `crate::archi`、`crates/core/archi` → `crate::archi`。
/// **MUST NOT 由目标 FQN 派生**：FQN 的点分段不是 Rust 路径。跨 crate 的父子组合由
/// `apiNestedCrossCrate` 判为不可表达，故本函数零 crate 名推导。
pub fn rust_module_path(element_root: &str) -> String {
    format!("crate::{}", module_name(element_root))
}

/// FQN 的 `<Domain>`（父 FQN 末段）：`App.Backend.Infra.ArchiRuntime` → `Infra`；
/// 单段 FQN → `None`。
pub fn domain_of(fqn: &str) -> Option<String> {
    let mut segments: Vec<&str> = fqn.split('.').collect();
    segments.pop()?;
    segments.last().map(|segment| (*segment).to_owned())
}

/// 契约目录本身（`<archi_dir>/api`）——本文件是它的唯一派生点。
pub fn directory(root: &Path) -> PathBuf {
    archi_api_dir(root)
}

/// 索引文件路径（`<archi_api_dir>/INDEX.toml`，全局唯一一份）。
pub fn index_path(root: &Path) -> PathBuf {
    directory(root).join(INDEX_FILE_NAME)
}

/// 语言 → 生成物种类（分派单点；MUST NOT 让调用方各自 match 语言再拼
/// [`GeneratedKind`]）。
///
/// `Unknown` 归 `Rust` **不是猜测**：按 D2 的 `<root>` 归一纪律，Rust 元素的根是
/// **目录或无后缀文件**（`crates/infra/archi` → 无扩展名），前端元素根一律带
/// `.ts` / `.tsx` / `.js` / `.jsx`。故「无扩展名」在生成物面上唯一对应 Rust 形态；
/// 真正不可判的语言（`.py` / `.go` 等）不存在于模型元素 `path` 里，即便出现也只有
/// Rust 一侧的渲染器能承接——把它静默丢弃会让 `render_all` 少出一份生成物，
/// 那是比「按 Rust 形态渲染」更坏的静默失败。
pub fn generated_kind(language: Language) -> GeneratedKind {
    match language {
        Language::TypeScript => GeneratedKind::TypeScript,
        Language::Rust | Language::Unknown => GeneratedKind::Rust,
    }
}

/// 契约文件路径：`<archi_api_dir>/<Domain>/<FQN>.md`。
///
/// FQN 形态非法（空 / 含路径分隔符 / `.` 或 `..` 段 / 绝对前缀）→ `None`：FQN 同时
/// 充当**文件名**，放行即让下层的 `is_file()` 探测与工具 crate 的写盘越出契约目录。
pub fn contract_path(root: &Path, fqn: &str) -> Option<PathBuf> {
    if !is_fqn(fqn) {
        return None;
    }
    Some(domain_dir(root, fqn).join(format!("{fqn}.md")))
}

/// schema 生成物路径：与契约同目录的 `<FQN>.schema.json`（形态判据同
/// [`contract_path`]）。
pub fn schema_path(root: &Path, fqn: &str) -> Option<PathBuf> {
    if !is_fqn(fqn) {
        return None;
    }
    Some(domain_dir(root, fqn).join(format!("{fqn}.schema.json")))
}

/// 元素生成物路径：`<root>.i.rs` / `<root>.i.ts`（`<root>` = `path[0]` 归一）。
///
/// 入参只收 `<root>` 字符串（D42）：「容器元素天然无生成物」由调用方以
/// `element.path.first()?` 表达，本层只判形态——`<root>` 形态非法 → `None`
/// （[`checked_root`]）。MUST NOT 为凑参数构造占位元素（接口面大于需求面会让占位值
/// 被下游误用时静默出错）。
pub fn generated_path(root: &Path, element_root: &str, kind: GeneratedKind) -> Option<PathBuf> {
    let element_root = checked_root(element_root)?;
    Some(root.join(format!("{element_root}.i.{}", kind.extension())))
}

/// 入口查找（顺序封闭，MUST NOT 增分支）：Rust `<dir>/src/lib.rs` → `<dir>/mod.rs`
/// → `<dir>.rs`；前端 `<dir>/index.ts` → `<dir>/index.tsx` → `<dir>.ts` →
/// `<dir>.tsx`。
///
/// 返回形态与该入口的 repo 相对 POSIX 路径；`path` 为空或 `<root>` 形态非法 →
/// `None`（同 [`generated_path`] 的判据）；形态合法但无一候选命中 → `None`（即
/// `apiEntryMissing`）。`<dir>/lib.rs` MUST NOT 在列——crate 的 `lib.rs` 恒在
/// `src/` 下。
pub fn entry_form(root: &Path, element: &ArchiElement) -> Option<(EntryForm, String)> {
    let element_root = checked_root(element.path.first()?)?;
    let directory = root.join(&element_root);

    // Rust 三分支
    if directory.join("src").join("lib.rs").is_file() {
        return Some((EntryForm::Crate, format!("{element_root}/src/lib.rs")));
    }
    if directory.join("mod.rs").is_file() {
        return Some((EntryForm::SubdirModule, format!("{element_root}/mod.rs")));
    }
    if root.join(format!("{element_root}.rs")).is_file() {
        return Some((EntryForm::SingleFile, format!("{element_root}.rs")));
    }

    // 前端四分支
    for name in ["index.ts", "index.tsx"] {
        if directory.join(name).is_file() {
            return Some((EntryForm::SubdirModule, format!("{element_root}/{name}")));
        }
    }
    for extension in ["ts", "tsx"] {
        if root.join(format!("{element_root}.{extension}")).is_file() {
            return Some((EntryForm::SingleFile, format!("{element_root}.{extension}")));
        }
    }
    None
}

/// `#[path]` 值的单点产出（目标恒为**根同级**生成物 `<root>.i.rs`）：
///
/// | 入口形态 | 值 |
/// |---|---|
/// | `Crate` | `../../<name>.i.rs` |
/// | `SubdirModule` | `../<name>.i.rs` |
/// | `SingleFile` | `<name>.i.rs` |
///
/// 手写「入口同级」会错位到 `<dir>/<name>.i.rs`，故 MUST NOT 手算。
pub fn mod_path_attr(form: EntryForm, name: &str) -> String {
    match form {
        EntryForm::Crate => format!("../../{name}.i.rs"),
        EntryForm::SubdirModule => format!("../{name}.i.rs"),
        EntryForm::SingleFile => format!("{name}.i.rs"),
    }
}

/// TS 侧 `import type` 路径的单点产出（目录形态 `../<name>.i`、单文件形态
/// `./<name>.i`）。
pub fn ts_import_specifier(form: EntryForm, name: &str) -> String {
    match form {
        EntryForm::Crate | EntryForm::SubdirModule => format!("../{name}.i"),
        EntryForm::SingleFile => format!("./{name}.i"),
    }
}

/// 跨文件 `$ref` 的相对前缀：同 `<Domain>` → `./`；跨目录 → `../<Domain>/`；
/// 目标无 `<Domain>` → `../Root/`。MUST NOT 手算。
pub fn ref_relative(from_fqn: &str, to_fqn: &str) -> String {
    match domain_of(to_fqn) {
        Some(domain) if domain_of(from_fqn).as_deref() == Some(domain.as_str()) => "./".to_owned(),
        Some(domain) => format!("../{domain}/"),
        None => format!("../{ROOT_DOMAIN}/"),
    }
}

/// 按扩展名判语言（取数器分派的唯一判据；其余 `Unknown`——不猜测）。
pub fn language_of(file: &str) -> Language {
    let extension = file.rsplit_once('.').map(|(_, tail)| tail).unwrap_or("");
    match extension {
        "rs" => Language::Rust,
        "ts" | "tsx" | "js" | "jsx" => Language::TypeScript,
        _ => Language::Unknown,
    }
}

/// `<Domain>` 目录（FQN 缺父段时退化 [`ROOT_DOMAIN`]；契约目录的唯一引用点）。
fn domain_dir(root: &Path, fqn: &str) -> PathBuf {
    directory(root).join(domain_of(fqn).unwrap_or_else(|| ROOT_DOMAIN.to_owned()))
}

/// 元素根路径的形态判据 + 归一（[`entry_form`] / [`generated_path`] 的共同前置）：
/// [`normalize_root`] 后为空或形态非法 → `None`。
fn checked_root(raw: &str) -> Option<String> {
    let root = normalize_root(raw);
    if !is_repo_relative(&root) {
        return None;
    }
    Some(root)
}

/// repo 相对路径判据（与 `docs::normalize_rel` 同一条纪律）：`/` 分隔、无绝对前缀、
/// 无盘符、无 `.` / `..` / 空段、非空。
///
/// **这是路径派生的安全边界**：`root.join(绝对路径)` 会替换 base、`..` 段会越出
/// 观察根——放行即让 `is_file()` 探测变成任意文件读、让工具 crate 的写盘变成任意
/// 文件写。
fn is_repo_relative(relative: &str) -> bool {
    if relative.is_empty() || relative.starts_with('/') || relative.starts_with('~') {
        return false;
    }
    let bytes = relative.as_bytes();
    // 盘符前缀（`C:` / `c:`）——Windows 绝对路径在 `join` 时同样替换 base
    if bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !relative
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
}

/// FQN 形态判据：FQN 同时充当契约文件名与 `<Domain>` 的取段源，故 MUST 是「点分段
/// 标识符」——非空、不含路径分隔符（`/` 与反斜杠）、不含盘符前缀、无空段
/// （`App..X` / `App.` 一类会让 `<Domain>` 退化成空串或 `.`）。
fn is_fqn(fqn: &str) -> bool {
    if fqn.is_empty() || fqn.contains('/') || fqn.contains('\\') {
        return false;
    }
    let bytes = fqn.as_bytes();
    if bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !fqn.split('.').any(str::is_empty)
}
