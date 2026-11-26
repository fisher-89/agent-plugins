//! 模块资产与生成物路径单点：`<root>` 归一、资产落点（`<叶段>.md` 文档 /
//! `<叶段>.schema.json` 接口，文件名只取 FQN 叶段）、生成物落点、入口解析链与形态判定、
//! TS import specifier、跨域 `$ref` 相对前缀。
//!
//! **几乎全是路径运算，零 IO**：[`entry_state`] 的存在性探测是唯一例外——它必须看盘
//! 才能回答「入口在哪」；[`entry_candidates`] 的候选派生**纯文本、零 IO**。资产目录
//! 经 [`foundation::layout::archi_models_dir`]、契约索引经
//! [`foundation::layout::archi_graph_dir`] 派生：本文件 MUST NOT 行内拼 `models` /
//! `graph` 段，MUST NOT 手算 `../` 层数（值形态由本文件单点产出，消费方零字面量）。
//!
//! **生成物落点 = 实现入口同级**（`<入口目录>/<入口主干>.i.<ext>`）：元素根路径
//! （`path.first()`）只回答「根在哪」，MUST NOT 参与落点派生；`#[path]` 的值因此恒为
//! 文件名（零 `../`）。
//!
//! **路径形态校验是四类派生的共同前置**：[`contract_path`] / [`schema_path`] /
//! [`generated_path`] / [`entry_state`] 一律**可失败**（非法形态 → `None` /
//! `EntryState::Invalid`）。理由是
//! `Path::join` 的语义：base 被绝对路径 / 盘符**替换**、`..` 段会**上溯**——放行即
//! 让「证据在盘」的探测越出仓库（任意文件读），也让工具 crate 的写盘面落成任意文件
//! 写。判据与 `scan::rust::normalize_rel` 同一条纪律（repo
//! 相对：`/` 分隔、无绝对前缀、无盘符、无 `.` / `..` / 空段），MUST NOT 靠调用方自觉。

use std::path::{Component, Path, PathBuf};

use foundation::layout::{archi_graph_dir, archi_models_dir};

use super::SCHEMA_FILE_SUFFIX;
use crate::dto::ArchiElement;

/// 索引文件名（`<archi_graph_dir>/INDEX.toml`，全局唯一一份；写盘目标与孤儿扫描
/// 收面共用同一持有者）。
pub(crate) const INDEX_FILE_NAME: &str = "INDEX.toml";

/// 入口形态（封闭三值）：由「入口与元素根的关系」派生（[`form_of`]），供
/// `EmitTarget.form`（TS 侧形态位；`ts_import_specifier` 待接线）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryForm {
    /// `<dir>/src/lib.rs`（crate 形态）
    Crate,
    /// `<dir>/mod.rs` / `<dir>/index.ts(x)`（目录形态）
    SubdirModule,
    /// 单文件形态（元素根本身即模块文件）
    SingleFile,
}

/// 入口不可用的理由（封闭四值）：与 `apiEntryMissing` 的四条理由一一对应（形态 /
/// 落位 / 存在 / 跨元素唯一）。
///
/// 每个变体自带载荷（`entry` 是归一后的声明值、`other_fqn` 是冲突方），判据结论因此
/// 自带措辞素材——`check` 不再回读模型字段拼文案，也不必为拿冲突方再跑一遍比对。
/// **MUST NOT 进结构化诊断**（九类诊断会让整图停摆），只经 `apiFindings` 出线。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryProblem {
    /// 声明的 `entry` 不是 repo 相对 POSIX 形态（基准判据）
    Malformed {
        /// 归一后的声明值
        entry: String,
    },
    /// 声明的 `entry` 落在本元素全部 `path` 之外（落位判据）
    OutsidePath {
        /// 归一后的声明值
        entry: String,
    },
    /// 声明或默认命中的入口不在盘上（存在判据）
    NotOnDisk {
        /// 归一后的声明值
        entry: String,
    },
    /// 与另一元素解析到同一入口文件（跨元素唯一判据）
    Duplicated {
        /// 归一后的声明值
        entry: String,
        /// 冲突方元素 FQN
        other_fqn: String,
    },
}

/// 入口解析链的结论（封闭三态）。
///
/// `Resolved` 的 `form` 只供 `EmitTarget.form`（TS 侧形态位，本期零 `.i.ts` 实例），
/// `entry` 是落点派生与取数的唯一输入；`Absent` 是**合法终态**（该元素不参与契约面），
/// MUST NOT 用 `api = none` 冒充——「没有门」与「明说无公共 API」是两个事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryState {
    /// 可用入口：形态位 + repo 相对入口路径
    Resolved {
        /// 入口形态（由入口与根的关系派生）
        form: EntryForm,
        /// 入口的 repo 相对 POSIX 路径
        entry: String,
    },
    /// 有入口事实但被判不可用（四理由各带载荷）
    Invalid(EntryProblem),
    /// 无入口事实（未声明且无约定默认，或约定默认全不落盘）
    Absent,
}

/// 生成物种类（封闭二值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedKind {
    /// `<入口主干>.i.rs`
    Rust,
    /// `<入口主干>.i.ts`
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
/// - **生成物面**（[`generated_kind`]）：`Unknown` → [`GeneratedKind::Rust`]（入口不带
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
/// **MUST NOT 去文件扩展名**——`<root>` 是模块身份，文件型根的扩展名正是「根即入口」
/// 的判据（[`entry_candidates`] 的 ②），剥掉它就会把单文件根降级成目录型根。
pub fn normalize_root(raw: &str) -> String {
    let unified = raw.replace('\\', "/");
    let mut trimmed = unified.as_str();
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest;
    }
    trimmed.trim_end_matches('/').to_owned()
}

/// `<root>` 末段（`crates/infra/archi` → `archi`、`views/welcome-view.tsx` →
/// `welcome-view.tsx`）——**只服务 Rust / TS 透传命名**（D40），MUST NOT 参与生成物
/// 文件名或 `#[path]` 值（后者是入口主干的函数，见 [`generated_path`]），MUST NOT 取
/// FQN 末段。
///
/// **纯派生、零形态判据**：透传名字对任意末段都成立，故本函数不校验字符集、零改名。**末段进目标
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
/// 套命名（D40 明禁），且会制造「静默错配」；产出 `r#<段>` 同样被否——[`module_name`] 的
/// 消费面是 **Rust 透传模块路径**与 **TS 透传 `<name>`**，两处都只接受原样末段（清洗或加
/// 前缀会与目标元素的真实模块名错配）。故正解是拒绝：不合法时装配点
/// （`api::module_refs` 的 Rust 分支）令该目标不入透传查表，渲染器只留
/// `// 透传自 <FQN>` 注释（与「模型里没有该元素」同一条降级通路）。
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

/// FQN 的**分层目录段**（FQN 段链去掉叶段）：`Backend.Core.Foundation` →
/// `["Backend", "Core"]`、`Desktop.Components.Agent` → `["Desktop", "Components"]`；
/// 单段 FQN（包根，如 `Backend`）→ `[]`（落契约目录根）；空串 → `None`。
///
/// 根段就是包级元素本身（`Backend` / `Desktop`），它参与目录分层（`Backend/Core/`、
/// `Desktop/Components/Agent/`）——不再被剥掉。目录与元素身份**解耦**：父子层级由段链
/// 镜像，元素身份仍是 FQN（文件名只取叶段，祖先链由目录承载），同名段
/// （`Backend.Core.Agent` 与 `Desktop.Components.Agent`）因而落在不同层次、互不混装。
pub fn dir_segments(fqn: &str) -> Option<Vec<String>> {
    let segments: Vec<&str> = fqn.split('.').collect();
    if fqn.is_empty() {
        return None;
    }
    Some(
        segments[..segments.len() - 1]
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect(),
    )
}

/// 契约目录本身（== `<archi_models_dir>`：分层目录段与 `<叶段>.md` / `.schema.json` 的
/// base）——本文件是它的唯一派生点。
pub fn directory(root: &Path) -> PathBuf {
    archi_models_dir(root)
}

/// 索引文件路径（`<archi_graph_dir>/INDEX.toml`，全局唯一一份）。
///
/// 索引与契约目录首次分家：本函数 MUST NOT 由 `directory(root).join(…)` 派生。
pub fn index_path(root: &Path) -> PathBuf {
    archi_graph_dir(root).join(INDEX_FILE_NAME)
}

/// 语言 → 生成物种类（分派单点；MUST NOT 让调用方各自 match 语言再拼
/// [`GeneratedKind`]）。
///
/// **输入是入口的扩展名**（入口恒带扩展名：目录型根的四个约定候选全是 `*.rs` /
/// `*.ts(x)`）。`Unknown` 归 `Rust` **不是猜测**：显式 `entry` 若指向 `.md` 一类非源
/// 文件，取数面本就零符号，生成物面按 Rust 形态渲染是唯一能承接的分支；真正不可判的
/// 语言（`.py` / `.go` 等）不存在于入口里，即便出现也只有 Rust 一侧的渲染器能承接——
/// 把它静默丢弃会让 `render_all` 少出一份生成物，那是比「按 Rust 形态渲染」更坏的静默
/// 失败。
pub fn generated_kind(language: Language) -> GeneratedKind {
    match language {
        Language::TypeScript => GeneratedKind::TypeScript,
        Language::Rust | Language::Unknown => GeneratedKind::Rust,
    }
}

/// FQN 叶段（最后一个点分段）——契约资产文件名的主干。
///
/// 祖先链已由目录镜像（[`dir_segments`]），文件名再带全 FQN 会把同一段前缀在路径里
/// 累计两次（`Desktop/Components/Agent/Desktop.Components.Agent.AgentInput.md`），深层
/// FQN 很快顶到 Windows 的 `MAX_PATH`。目录 + 叶段仍唯一：同目录同叶段 ⟺ 同 FQN。
pub fn leaf_name(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

/// 文档（资产①）路径：`<archi_models_dir>/<分层目录段…>/<叶段>.md`。
///
/// **落点恒由 FQN 派生，与 `api` 状态、模型 `doc` 字段都无关**——`declared` 与 `none`
/// 元素共用这一条派生（`doc` 是零判据读者的呈现事实，不进派生面）。
///
/// FQN 形态非法（空 / 含路径分隔符 / `.` 或 `..` 段 / 绝对前缀）→ `None`：FQN 同时
/// 充当**分层目录段与文件名锚**，放行即让下层的 `is_file()` 探测与工具 crate 的写盘
/// 越出契约目录。
pub fn contract_path(root: &Path, fqn: &str) -> Option<PathBuf> {
    if !is_fqn(fqn) {
        return None;
    }
    Some(dir_of(root, fqn).join(format!("{}.md", leaf_name(fqn))))
}

/// 接口（资产②）路径：与文档同目录的 `<叶段>.schema.json`（形态判据同
/// [`contract_path`]）。**它是手写真相，不是生成物**。
pub fn schema_path(root: &Path, fqn: &str) -> Option<PathBuf> {
    if !is_fqn(fqn) {
        return None;
    }
    Some(dir_of(root, fqn).join(format!("{}{SCHEMA_FILE_SUFFIX}", leaf_name(fqn))))
}

/// 接口定义文件的 repo 相对路径（`<archi_models_rel_dir>/<分层目录段…>/<叶段>.schema.json`）
/// ——错误文案与报告里的「文件」口径（`validate_schema` 无 root 入参，用它代替）。
pub fn schema_rel(fqn: &str) -> Option<String> {
    if !is_fqn(fqn) {
        return None;
    }
    let mut segments = dir_segments(fqn).unwrap_or_default();
    segments.push(format!("{}{SCHEMA_FILE_SUFFIX}", leaf_name(fqn)));
    Some(format!(
        "{}/{}",
        foundation::layout::archi_models_rel_dir(),
        segments.join("/")
    ))
}

/// 接口文件名主干（剥 `.schema.json` 的**唯一**后缀切分点；非该后缀 → `None`）。
fn schema_stem(file_name: &str) -> Option<&str> {
    file_name.strip_suffix(SCHEMA_FILE_SUFFIX)
}

/// 接口文件相对路径（**相对 `<archi_models_dir>`**）→ 元素 FQN 的**唯一**反查：
/// 分层目录段 + 文件主干（叶段）以 `.` 连接。
///
/// 反查必须看**整条相对路径**——文件名只剩叶段，单看文件名分不清 `Backend.Core.Agent`
/// 与 `Desktop.Components.Agent`。主干带点（旧式 `<FQN>.schema.json`）、缺后缀、空段、
/// `..` / 绝对前缀一律 `None`：那不是派生落点，拒绝即让写闸不把非规范位置的文件当接口。
///
/// 消费面两处：写闸（`write` / `edit` 落盘前从路径反查 FQN）与
/// [`super::reexport_target`]（跨元素 `$ref` 的文件部分反查目标元素）。MUST NOT 出现
/// 第二份后缀切分。
pub fn schema_fqn(relative: &Path) -> Option<String> {
    let mut segments: Vec<&str> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => segments.push(segment.to_str()?),
            _ => return None,
        }
    }
    let file = segments.pop()?;
    let stem = schema_stem(file)?;
    if stem.contains('.') {
        return None;
    }
    segments.push(stem);
    let fqn = segments.join(".");
    is_fqn(&fqn).then_some(fqn)
}

/// 元素生成物路径：`<入口目录>/<入口主干>.i.rs` / `.i.ts`（与**入口**同级）。
///
/// 入参是**入口**而非元素根：落点是入口的单值函数（无入口即无路径可派生），元素根
/// 只回答「根在哪」。`<入口主干>` = 文件名最后一次 `.` 之前的部分（无 `.` 或主干为空
/// → 整个文件名，MUST NOT 丢文件名）。`#[path]` 的值即本函数结果的文件名（零 `../`）。
///
/// 形态非法 → `None`（同 [`checked_root`] 的判据，见模块 doc 的安全边界）。
pub fn generated_path(root: &Path, entry: &str, kind: GeneratedKind) -> Option<PathBuf> {
    let entry = checked_root(entry)?;
    let (directory, file) = match entry.rsplit_once('/') {
        Some((directory, file)) => (Some(directory), file),
        None => (None, entry.as_str()),
    };
    let stem = match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file,
    };
    let name = format!("{stem}.i.{}", kind.extension());
    Some(match directory {
        Some(directory) => root.join(directory).join(name),
        None => root.join(name),
    })
}

/// 入口候选（**纯文本、零 IO、按优先序**）：三态解析链的唯一产出点。
///
/// ```text
/// ① 显式 entry（归一后）                        → [entry]        唯一候选
/// ② 文件型根（末段扩展名 ∈ {rs,ts,tsx,js,jsx}） → [<root>]       根文件自身即模块
/// ③ 目录型根（语言无关，按序取首个落盘者）
///    → [<root>/src/lib.rs, <root>/mod.rs, <root>/index.ts, <root>/index.tsx]
/// ④ 其余（`path` 空 / `<root>` 形态非法）        → []
/// ```
///
/// **不做校验**：形态非法的声明原样返回，由 [`entry_state`] 判
/// [`EntryProblem::Malformed`]——这里滤空就分不清「声明非法」与「没有声明」。
///
/// ③ **MUST NOT 做语言判定**：目录型根没有扩展名（`language_of` 对它是 `Unknown`），
/// 按语言分派会让前端目录根的 `index.ts(x)` 候选失效。四种形态互不相交，按序取首个
/// 落盘者即够。`<dir>/lib.rs` 不在列（crate 的 `lib.rs` 恒在 `src/` 下），
/// `<dir>.rs` / `<dir>.ts` / `<dir>.tsx` 也不在列（对目录型根只能指到 `path` 之外的
/// 兄弟文件，必违落位判据）。
pub fn entry_candidates(element: &ArchiElement) -> Vec<String> {
    if let Some(declared) = &element.entry {
        return vec![normalize_root(declared)];
    }
    let Some(root) = element.path.first().and_then(|raw| checked_root(raw)) else {
        return Vec::new();
    };
    if crate::scan::is_source_file(&root) {
        return vec![root];
    }
    vec![
        format!("{root}/src/lib.rs"),
        format!("{root}/mod.rs"),
        format!("{root}/index.ts"),
        format!("{root}/index.tsx"),
    ]
}

/// 入口解析链的判定（三态）与四条判据的唯一判定点：**读盘只在此**（`is_file`）。
///
/// 显式声明走 ①（基准 → 落位 → 存在 → 唯一，判序固定、MUST NOT 增分支）；否则由
/// [`entry_candidates`] 按序取**首个**落盘者，再对命中者补跑落位与唯一两条判据
/// （约定默认不会「越界」之外还有别的形态问题：候选由本文件拼出，形态必然合法）。
pub fn entry_state(root: &Path, elements: &[ArchiElement], element: &ArchiElement) -> EntryState {
    if let Some(declared) = &element.entry {
        let declared = normalize_root(declared);
        if !is_repo_relative(&declared) {
            return EntryState::Invalid(EntryProblem::Malformed { entry: declared });
        }
        if !within_path(element, &declared) {
            return EntryState::Invalid(EntryProblem::OutsidePath { entry: declared });
        }
        if !root.join(&declared).is_file() {
            return EntryState::Invalid(EntryProblem::NotOnDisk { entry: declared });
        }
        if let Some(other_fqn) = duplicate_of(elements, element, &declared) {
            return EntryState::Invalid(EntryProblem::Duplicated {
                entry: declared,
                other_fqn: other_fqn.to_owned(),
            });
        }
        return EntryState::Resolved {
            form: form_of(&declared, element),
            entry: declared,
        };
    }
    let Some(found) = entry_candidates(element)
        .into_iter()
        .find(|candidate| root.join(candidate).is_file())
    else {
        return EntryState::Absent;
    };
    if !within_path(element, &found) {
        return EntryState::Invalid(EntryProblem::OutsidePath { entry: found });
    }
    if let Some(other_fqn) = duplicate_of(elements, element, &found) {
        return EntryState::Invalid(EntryProblem::Duplicated {
            entry: found,
            other_fqn: other_fqn.to_owned(),
        });
    }
    EntryState::Resolved {
        form: form_of(&found, element),
        entry: found,
    }
}

/// 落位判据：入口 MUST 等于某条 `path` 条目，或落在某条**目录型** `path` 之内。
///
/// 目录分支只对非源文件形态的 `path` 条目生效（判据 = [`crate::scan::is_source_file`]
/// 的扩展名白名单，与枚举面同源），故「文件型根声明同目录的另一个文件」自然落
/// `OutsidePath`，也不会出现「`views/welcome-view.tsx/xx.rs` 被前缀匹配放行」的形态。
/// `path` 条目形态非法时该条不成立。
fn within_path(element: &ArchiElement, entry: &str) -> bool {
    element.path.iter().any(|path| {
        let normalized = normalize_root(path);
        if !is_repo_relative(&normalized) {
            return false;
        }
        if entry == normalized {
            return true;
        }
        !crate::scan::is_source_file(&normalized) && entry.starts_with(&format!("{normalized}/"))
    })
}

/// 跨元素唯一判据 = **候选交集**：其它元素（`fqn` 不同）中首个「候选表含同一入口文本」
/// 者即冲突方。
///
/// 候选派生零 IO，故不必为另一个元素跑完整解析（那要递归 + 读盘）。代价是一处保守
/// 近似：同一目录型根下 `src/lib.rs` 与 `mod.rs` **同时**在盘时，另一元素显式声明后者
/// 会被判冲突（其实前者优先命中）——只多报不瞒报。
fn duplicate_of<'a>(
    elements: &'a [ArchiElement],
    element: &ArchiElement,
    entry: &str,
) -> Option<&'a str> {
    elements
        .iter()
        .filter(|candidate| candidate.fqn != element.fqn)
        .find(|candidate| {
            entry_candidates(candidate)
                .iter()
                .any(|candidate_entry| candidate_entry == entry)
        })
        .map(|candidate| candidate.fqn.as_str())
}

/// 形态位派生：由**入口与根的关系**回答，MUST NOT 反推入口。
///
/// 入口 == 根 → `SingleFile`；入口以 `src/lib.rs` 收尾 → `Crate`；其余 → `SubdirModule`。
fn form_of(entry: &str, element: &ArchiElement) -> EntryForm {
    let root = element.path.first().and_then(|raw| checked_root(raw));
    if root.as_deref() == Some(entry) {
        return EntryForm::SingleFile;
    }
    if entry == "src/lib.rs" || entry.ends_with("/src/lib.rs") {
        return EntryForm::Crate;
    }
    EntryForm::SubdirModule
}

/// TS 侧 `import type` 路径的单点产出（目录形态 `../<name>.i`、单文件形态
/// `./<name>.i`）。
///
/// **待接线留白**：入口同级规则（同目录 `./<目标入口主干>.i`、跨目录由入口到入口派生）
/// 归前端契约变更；当前输出 MUST NOT 被当作既有真值——本仓 `.i.ts` 生成物为零，该规则
/// 此刻无实例可验。
pub fn ts_import_specifier(form: EntryForm, name: &str) -> String {
    match form {
        EntryForm::Crate | EntryForm::SubdirModule => format!("../{name}.i"),
        EntryForm::SingleFile => format!("./{name}.i"),
    }
}

/// 跨文件 `$ref` 的相对前缀：由两侧**分层目录段**逐段求相对路径（同目录 → `./`、
/// 向上一层 → `../`、向下一层 → `./<段>/`），返回串以 `/` 收尾（`$ref` 的值即
/// `<前缀><目标叶段>.schema.json#/$defs/<name>`）。MUST NOT 手算层数。
pub fn ref_relative(from_fqn: &str, to_fqn: &str) -> String {
    let from = dir_segments(from_fqn).unwrap_or_default();
    let to = dir_segments(to_fqn).unwrap_or_default();
    let common = from
        .iter()
        .zip(to.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let up = "../".repeat(from.len() - common);
    let down: String = to[common..]
        .iter()
        .map(|segment| format!("{segment}/"))
        .collect();
    match (up.is_empty(), down.is_empty()) {
        (true, true) => "./".to_owned(),
        (true, false) => format!("./{down}"),
        (false, _) => format!("{up}{down}"),
    }
}

/// 跨文件 `$ref` 的文件部分 → 目标元素 FQN：以本元素的**分层目录**为起点解相对路径
/// （`.` 原地、`..` 上溯一层、其余段下沉），文件主干即目标叶段。
///
/// 反查必须带本元素 FQN——文件名只剩叶段，单看串已不知道它属于哪条祖先链。解析结果
/// 仍过 [`is_fqn`]：越出契约根（`..` 多于层数）、空段、带点的段（旧式 `<FQN>` 文件名或
/// 任意前缀残片）一律 `None`。前缀**是否逐字等于** [`ref_relative`] 的产物由校验侧比对
/// （本函数只解路径，不做规范形判定）。
pub fn ref_target(from_fqn: &str, file: &str) -> Option<String> {
    let stem = schema_stem(file)?;
    let mut segments = dir_segments(from_fqn)?;
    let mut parts = stem.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if part.is_empty() || part.contains('.') {
                return None;
            }
            segments.push(part.to_owned());
        } else {
            match part {
                "" => return None,
                "." => {}
                ".." => {
                    segments.pop()?;
                }
                _ if part.contains('.') => return None,
                _ => segments.push(part.to_owned()),
            }
        }
    }
    let fqn = segments.join(".");
    is_fqn(&fqn).then_some(fqn)
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

/// 分层目录绝对路径（[`dir_segments`] 逐段 `push` 到契约目录；单段 FQN 即契约目录本身
/// ——契约目录的唯一引用点）。
fn dir_of(root: &Path, fqn: &str) -> PathBuf {
    let mut dir = directory(root);
    for segment in dir_segments(fqn).unwrap_or_default() {
        dir.push(segment);
    }
    dir
}

/// 路径形态判据 + 归一（[`entry_candidates`] / [`generated_path`] / [`entry_state`]
/// 的共同前置）：
/// [`normalize_root`] 后为空或形态非法 → `None`。
pub(crate) fn checked_root(raw: &str) -> Option<String> {
    let root = normalize_root(raw);
    if !is_repo_relative(&root) {
        return None;
    }
    Some(root)
}

/// repo 相对路径判据的**唯一实现**（文档关联能力已退役）：`/` 分隔、无绝对前缀、
/// 无盘符、无 `~`、无 `.` / `..` / 空段、非空。
///
/// **这是路径派生的安全边界**：`root.join(绝对路径)` 会替换 base、`..` 段会越出
/// 观察根——放行即让 `is_file()` 探测变成任意文件读、让工具 crate 的写盘变成任意
/// 文件写。
pub(crate) fn is_repo_relative(relative: &str) -> bool {
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

/// FQN 形态判据：FQN 同时充当资产文件名与分层目录段的取段源，故 MUST 是「点分段
/// 标识符」——非空、不含路径分隔符（`/` 与反斜杠）、不含盘符前缀、无空段
/// （`Backend..X` / `Backend.` 一类会让目录段退化成空串或 `.`，即目录穿越面）。
pub fn is_fqn(fqn: &str) -> bool {
    if fqn.is_empty() || fqn.contains('/') || fqn.contains('\\') {
        return false;
    }
    let bytes = fqn.as_bytes();
    if bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !fqn.split('.').any(str::is_empty)
}
