//! Rust 依赖提取：workspace 清单 → crate 清单 → 逐 crate 模块树 → `use` 树最长
//! 前缀匹配落到文件。
//!
//! **零子进程**（MUST NOT 调 `cargo metadata`）：只读 `Cargo.toml`（workspace
//! `members` 含 glob，经 `globset` 展开）+ `syn` 解析源文件的 `mod` / `use` /
//! `#[path]` 属性。
//!
//! 不可判定面一律留痕到 `unresolved`（宏生成的 `mod` / `include!` / 孤儿 `.rs` /
//! `mod` 声明定位不到文件 / `use` 解析失败）；**外部 crate 静默丢弃**（`std` /
//! 第三方无元素覆盖，记进报告只是噪声）。不做类型检查、不展开宏、不做完整 name
//! resolution；`pub use` 只算「声明了依赖」，不穿透追源。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use globset::{Glob, GlobSetBuilder};
use syn::spanned::Spanned;

use crate::drift::report::ArchiUnresolvedFile;

use super::ImportFact;

/// 目录递归扫描深度上限（workspace member 目录发现；`crates/a/b/c` 级足够）。
const MEMBER_WALK_DEPTH: usize = 4;

/// Rust specifier 解析器（crate 清单 + 模块路径 → 文件表 + 建树期 `unresolved`）。
pub struct RustResolver {
    /// crate 名 → 模块表
    crates: BTreeMap<String, CrateModules>,
    /// 建树期不可判定项（宏 `mod` / `include!` / 孤儿 `.rs` / `mod` 定位失败）
    unresolved: Vec<ArchiUnresolvedFile>,
}

/// 单 crate 的模块表。
struct CrateModules {
    /// 模块路径（`""` = crate 根；其余形如 `a::b`）→ 文件（repo 相对 POSIX）
    modules: BTreeMap<String, String>,
    /// 文件（repo 相对 POSIX）→ 模块路径
    files: BTreeMap<String, String>,
}

/// 建树累加器（读盘 + 留痕；模块表在 `RustResolver` 内）。
struct TreeBuilder<'a> {
    root: &'a Path,
    modules: CrateModules,
    unresolved: Vec<ArchiUnresolvedFile>,
    visited: BTreeSet<String>,
}

impl RustResolver {
    /// 解析 workspace 清单 + member glob → crate 清单 → 逐 crate 模块树。
    ///
    /// 观察根无 `Cargo.toml` → 零 crate（纯 TS 工程合法）；member glob 命中的目录
    /// 无 `Cargo.toml` → 跳过（glob 过近似的常态）；清单 / member 存在但不可解析、
    /// 缺 crate 名或无入口 → `Err`（配置面坏了不该静默变成「零命中」）。
    ///
    /// crate 表按**源码标识符形态**登记（清单名 `archi-runtime` → 键 `archi_runtime`）：
    /// `use` 路径里写的是标识符，用连字符名查表必然全部落空（声明边会被误报成
    /// 「声明未兑现」）。
    pub fn new(root: &Path) -> Result<RustResolver, String> {
        let mut crates: BTreeMap<String, CrateModules> = BTreeMap::new();
        let mut unresolved: Vec<ArchiUnresolvedFile> = Vec::new();
        for spec in discover_crates(root)? {
            let mut builder = TreeBuilder {
                root,
                modules: CrateModules {
                    modules: BTreeMap::new(),
                    files: BTreeMap::new(),
                },
                unresolved: Vec::new(),
                visited: BTreeSet::new(),
            };
            let module_dir = module_dir_of(&spec.entry);
            builder.build_file(&spec.entry, "", &module_dir);
            builder.collect_orphans(&spec.entry);
            crates.insert(identifier_of(&spec.name), builder.modules);
            unresolved.append(&mut builder.unresolved);
        }
        Ok(RustResolver { crates, unresolved })
    }

    /// 建树期不可判定项（宏 `mod` / `include!` / 孤儿 `.rs` / `mod` 定位失败）。
    pub fn unresolved(&self) -> &[ArchiUnresolvedFile] {
        &self.unresolved
    }

    /// 单文件 `use` 树抽取：产出规范化后的 use 路径字符串（`crate::a::b` /
    /// `self::a` / `super::a` / `<首段>::a::b` 形态）与行号。
    ///
    /// 解析失败（`syn` 报错）→ `unresolved`（MUST NOT 冒充违例）。
    pub fn extract(
        &self,
        file_rel: &str,
        source: &str,
    ) -> (Vec<ImportFact>, Vec<ArchiUnresolvedFile>) {
        let mut facts = Vec::new();
        let mut unresolved = Vec::new();
        let parsed = match syn::parse_file(source) {
            Ok(parsed) => parsed,
            Err(error) => {
                unresolved.push(ArchiUnresolvedFile {
                    file: file_rel.to_owned(),
                    specifier: None,
                    message: format!("Rust 源文件解析失败：{error}"),
                });
                return (facts, unresolved);
            }
        };
        for item in &parsed.items {
            let syn::Item::Use(item_use) = item else {
                continue;
            };
            let line = item_use.span().start().line.max(1) as u32;
            let mut paths: Vec<String> = Vec::new();
            flatten_use_tree(&item_use.tree, &mut Vec::new(), &mut paths);
            for path in paths {
                facts.push(ImportFact {
                    specifier: path,
                    line,
                    // `use` 声明本身无 type-only 面（Rust 的 `use` 不区分值 / 类型）
                    type_only: false,
                });
            }
        }
        (facts, unresolved)
    }

    /// 模块路径最长前缀匹配落到文件；首段是外部 crate → `None`（静默丢弃）。
    pub fn resolve(&self, file_rel: &str, specifier: &str) -> Option<String> {
        let (crate_name, module_segments) = self.locate(file_rel)?;
        let segments: Vec<&str> = specifier
            .split("::")
            .filter(|segment| !segment.is_empty())
            .collect();
        let (target_crate, tail) = match *segments.first()? {
            "crate" => (
                crate_name.clone(),
                segments[1..]
                    .iter()
                    .map(|segment| (*segment).to_owned())
                    .collect(),
            ),
            "self" => {
                let mut full = module_segments.clone();
                full.extend(segments[1..].iter().map(|segment| (*segment).to_owned()));
                (crate_name.clone(), full)
            }
            "super" => {
                let mut rest = segments.clone();
                let mut base = module_segments.clone();
                while rest.first() == Some(&"super") {
                    base.pop();
                    rest.remove(0);
                }
                base.extend(rest.into_iter().map(str::to_owned));
                (crate_name.clone(), base)
            }
            first => {
                if self.crates.contains_key(first) {
                    (
                        first.to_owned(),
                        segments[1..]
                            .iter()
                            .map(|segment| (*segment).to_owned())
                            .collect(),
                    )
                } else if self
                    .crates
                    .get(&crate_name)
                    .is_some_and(|modules| modules.modules.contains_key(first))
                {
                    // 当前 crate 根下的模块（`use foo::Bar` 的 2015 形态）
                    (
                        crate_name.clone(),
                        segments
                            .iter()
                            .map(|segment| (*segment).to_owned())
                            .collect(),
                    )
                } else {
                    // 外部 crate（std / 第三方）：无元素覆盖，静默丢弃
                    return None;
                }
            }
        };
        let modules = self.crates.get(&target_crate)?;
        for length in (0..=tail.len()).rev() {
            let key = tail[..length].join("::");
            if let Some(file) = modules.modules.get(&key) {
                return Some(file.clone());
            }
        }
        None
    }

    /// 文件 → (所属 crate, 模块路径段)。
    fn locate(&self, file_rel: &str) -> Option<(String, Vec<String>)> {
        for (name, modules) in &self.crates {
            if let Some(module) = modules.files.get(file_rel) {
                let segments = if module.is_empty() {
                    Vec::new()
                } else {
                    module.split("::").map(str::to_owned).collect()
                };
                return Some((name.clone(), segments));
            }
        }
        None
    }
}

impl TreeBuilder<'_> {
    /// 单文件建树：解析 → 登记 → 递归 `mod` 项。
    fn build_file(&mut self, file_rel: &str, module_path: &str, dir: &str) {
        if !self.visited.insert(file_rel.to_owned()) {
            return;
        }
        let full = self.root.join(file_rel);
        let source = match std::fs::read_to_string(&full) {
            Ok(source) => source,
            Err(error) => {
                self.unresolved.push(ArchiUnresolvedFile {
                    file: file_rel.to_owned(),
                    specifier: None,
                    message: format!("Rust 源文件读取失败：{error}"),
                });
                return;
            }
        };
        if source.contains("include!") {
            self.unresolved.push(ArchiUnresolvedFile {
                file: file_rel.to_owned(),
                specifier: None,
                message: "文件含 include!，模块树不完整".to_owned(),
            });
        }
        let parsed = match syn::parse_file(&source) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.unresolved.push(ArchiUnresolvedFile {
                    file: file_rel.to_owned(),
                    specifier: None,
                    message: format!("Rust 源文件解析失败：{error}"),
                });
                return;
            }
        };
        self.modules
            .modules
            .insert(module_path.to_owned(), file_rel.to_owned());
        self.modules
            .files
            .insert(file_rel.to_owned(), module_path.to_owned());
        self.walk_items(&parsed.items, file_rel, module_path, dir);
    }

    /// 项级遍历：`mod` 四种形态（外部文件 / 内联 / `#[path]` / `cfg` 两臂同路径）
    /// 与宏 / `include!` 留痕。
    fn walk_items(&mut self, items: &[syn::Item], file_rel: &str, module_path: &str, dir: &str) {
        for item in items {
            match item {
                syn::Item::Mod(item_mod) => {
                    let name = item_mod.ident.to_string();
                    let child = join_module(module_path, &name);
                    match &item_mod.content {
                        // 内联 `mod x { … }`：登记路径（映射到本文件），子项在
                        // `<dir>/x/` 下解析
                        Some((_, inner_items)) => {
                            self.modules
                                .modules
                                .insert(child.clone(), file_rel.to_owned());
                            let inner_dir = format!("{dir}/{name}");
                            self.walk_items(inner_items, file_rel, &child, &inner_dir);
                        }
                        None => {
                            let target = self.locate_mod(item_mod, dir, &name);
                            match target {
                                Some(target) => {
                                    let target_dir = module_dir_of(&target);
                                    self.build_file(&target, &child, &target_dir);
                                }
                                None => self.unresolved.push(ArchiUnresolvedFile {
                                    file: file_rel.to_owned(),
                                    specifier: None,
                                    message: format!(
                                        "mod {name} 定位不到文件（{} 下既无 {name}/mod.rs 也无 \
                                         {name}.rs，且无 #[path]）",
                                        if dir.is_empty() { "." } else { dir }
                                    ),
                                }),
                            }
                        }
                    }
                }
                syn::Item::Macro(item_macro) => {
                    let message = if item_macro.ident.is_some() {
                        format!(
                            "macro_rules! {} 可能生成 mod，模块树不完整",
                            item_macro
                                .ident
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_default()
                        )
                    } else if item_macro.mac.path.is_ident("include") {
                        "include! 展开的文件不在模块树内".to_owned()
                    } else {
                        "宏调用可能生成 mod，模块树不完整".to_owned()
                    };
                    self.unresolved.push(ArchiUnresolvedFile {
                        file: file_rel.to_owned(),
                        specifier: None,
                        message,
                    });
                }
                _ => {}
            }
        }
    }

    /// `mod x;` 的文件定位：`#[path = "…"]` 优先，其次目录形态 `x/mod.rs` 优先、
    /// 再退 `x.rs`（两形态同存时按目录形态取用，MUST NOT 报错）。
    ///
    /// `#[path]` 取值先归一（`\` → `/`、折叠 `.` / `..`）并**限根**：绝对路径或
    /// 上溯越过观察根的形态一律不采信（模块树 MUST NOT 被带出仓外），留痕走
    /// 「`mod` 定位不到文件」。
    fn locate_mod(&self, item_mod: &syn::ItemMod, dir: &str, name: &str) -> Option<String> {
        if let Some(path) = path_attribute(&item_mod.attrs) {
            let candidate = normalize_rel(&join_rel(dir, &path))?;
            return self.exists(&candidate).then_some(candidate);
        }
        let directory_form = format!("{dir}/{name}/mod.rs");
        if self.exists(&directory_form) {
            return Some(directory_form);
        }
        let file_form = format!("{dir}/{name}.rs");
        self.exists(&file_form).then_some(file_form)
    }

    /// 孤儿 `.rs` 检出：crate `src/` 下既不在模块树、又非入口的文件（无 `mod`
    /// 声明纳入）→ `unresolved`。
    fn collect_orphans(&mut self, entry: &str) {
        let src_dir = match entry.rsplit_once('/') {
            Some((dir, _)) => dir.to_owned(),
            None => String::new(),
        };
        let scan_root = if src_dir.is_empty() {
            self.root.to_path_buf()
        } else {
            self.root.join(&src_dir)
        };
        for file in walk_rs_files(&scan_root) {
            let rel = match file.strip_prefix(self.root) {
                Ok(rel) => super::posix(&rel.to_string_lossy()),
                Err(_) => continue,
            };
            if rel == entry || self.modules.files.contains_key(&rel) {
                continue;
            }
            self.unresolved.push(ArchiUnresolvedFile {
                file: rel,
                specifier: None,
                message: "孤儿 .rs（未被任何 mod 声明纳入模块树）".to_owned(),
            });
        }
    }

    /// 相对路径在场判定（`root` 锚）。
    fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).is_file()
    }
}

/// crate 发现结果（crate 名 + 入口文件 repo 相对路径）。
struct CrateSpec {
    name: String,
    entry: String,
}

/// workspace 清单 → crate 清单（member glob 展开 + 逐 member 读 `[package] name`
/// 与入口；**零子进程**）。
fn discover_crates(root: &Path) -> Result<Vec<CrateSpec>, String> {
    let manifest = root.join("Cargo.toml");
    if !manifest.is_file() {
        // 纯 TS 工程（无 Rust crate）：零 crate 是合法形态，不报错
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&manifest)
        .map_err(|error| format!("workspace 清单读取失败（{}）：{error}", manifest.display()))?;
    let value: toml::Value = toml::from_str(&text)
        .map_err(|error| format!("workspace 清单解析失败（{}）：{error}", manifest.display()))?;
    let members: Vec<String> = value
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    let mut spec_dirs: Vec<String> = Vec::new();
    if members.is_empty() {
        spec_dirs.push(String::new());
    } else {
        let mut builder = GlobSetBuilder::new();
        for pattern in &members {
            let glob = Glob::new(pattern)
                .map_err(|error| format!("workspace members 条目非法：\"{pattern}\"（{error}）"))?;
            builder.add(glob);
        }
        let set = builder
            .build()
            .map_err(|error| format!("workspace members 编译失败：{error}"))?;
        let mut candidates: BTreeSet<String> = BTreeSet::new();
        for dir in walk_dirs(root) {
            let rel = super::posix(&dir.to_string_lossy());
            if set.is_match(&rel) {
                candidates.insert(rel);
            }
        }
        spec_dirs.extend(candidates);
    }

    let mut specs: Vec<CrateSpec> = Vec::new();
    for dir in spec_dirs {
        let crate_dir = if dir.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&dir)
        };
        // member glob 过近似命中的非 crate 目录 → 跳过；清单存在但不可解析 / 缺
        // `[package] name` / 无入口 → 显式 `Err`（配置面坏了 MUST NOT 静默变成
        // 「零 crate = 零命中」）
        match read_crate_spec(&crate_dir, &dir)? {
            Some(spec) => specs.push(spec),
            None => continue,
        }
    }
    specs.sort_by(|left, right| left.name.cmp(&right.name));
    specs.dedup_by(|left, right| left.name == right.name);
    Ok(specs)
}

/// 单 member 的 crate 名与入口（`[lib].path` → `src/lib.rs` → `src/main.rs`）。
///
/// `Ok(None)` = 该目录无 `Cargo.toml`（member glob 过近似命中，不是 crate）；
/// 有清单但读 / 解析失败、缺 `[package] name`、无入口文件 → `Err`（MUST NOT 静默
/// 吞成「少一个 crate」，那会让该 crate 的跨 crate 依赖整体消失）。
fn read_crate_spec(crate_dir: &Path, dir: &str) -> Result<Option<CrateSpec>, String> {
    let manifest = crate_dir.join("Cargo.toml");
    let text = match std::fs::read_to_string(&manifest) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "member 清单读取失败（{}）：{error}",
                manifest.display()
            ))
        }
    };
    let value: toml::Value = toml::from_str(&text)
        .map_err(|error| format!("member 清单解析失败（{}）：{error}", manifest.display()))?;
    let name = value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("member 缺 [package] name（{}）", manifest.display()))?
        .to_owned();
    let lib_path = value
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str);
    let mut candidates: Vec<&str> = Vec::new();
    if let Some(path) = lib_path {
        candidates.push(path);
    }
    candidates.push("src/lib.rs");
    candidates.push("src/main.rs");
    for candidate in candidates {
        if crate_dir.join(candidate).is_file() {
            return Ok(Some(CrateSpec {
                name,
                entry: join_rel(dir, candidate),
            }));
        }
    }
    Err(format!("member 无入口文件（{}）", manifest.display()))
}

/// 拼接 repo 相对路径（`dir` 为空即文件名本身）。
fn join_rel(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        rel.to_owned()
    } else {
        format!("{dir}/{rel}")
    }
}

/// 清单名 → 源码标识符形态（`-` → `_`；`use` 路径里写的是标识符）。
fn identifier_of(name: &str) -> String {
    name.replace('-', "_")
}

/// 相对路径归一（`\` → `/`、折叠 `.` 与 `..`、去空段）。
///
/// **限根**：绝对路径（前导 `/` 或含盘符的 `:`）与上溯越过观察根的形态（`..` 把
/// 栈弹空）一律 `None`——`#[path = "../../out/x.rs"]` MUST NOT 把模块树带出仓外；
/// 归一结果为空（如 `#[path = "."]`）同判 `None`。
fn normalize_rel(path: &str) -> Option<String> {
    let unified = path.replace('\\', "/");
    if unified.starts_with('/') || unified.contains(':') {
        return None;
    }
    let mut segments: Vec<&str> = Vec::new();
    for segment in unified.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(segments.join("/"))
}

/// 模块路径拼接（crate 根模块路径为空串）。
fn join_module(module_path: &str, name: &str) -> String {
    if module_path.is_empty() {
        name.to_owned()
    } else {
        format!("{module_path}::{name}")
    }
}

/// 文件所属模块的子模块目录（`src/lib.rs` / `src/main.rs` → `src`；
/// `src/foo.rs` → `src/foo`；`src/foo/mod.rs` → `src/foo`）。
///
/// 三个 crate 入口文件与 `mod.rs` 同判：其子模块落在**文件所在目录**下；其余
/// `foo.rs` 的子模块落在 `foo/` 目录下（rustc 的 file-module 规则）。
fn module_dir_of(file_rel: &str) -> String {
    let base = file_rel.rsplit('/').next().unwrap_or(file_rel);
    if base == "mod.rs" || base == "lib.rs" || base == "main.rs" {
        return parent_dir(file_rel);
    }
    match file_rel.strip_suffix(".rs") {
        Some(stripped) => stripped.to_owned(),
        None => parent_dir(file_rel),
    }
}

/// 父目录（repo 相对 POSIX；顶层文件 → 空串）。
fn parent_dir(file_rel: &str) -> String {
    file_rel
        .rsplit_once('/')
        .map_or(String::new(), |(dir, _)| dir.to_owned())
}

/// `#[path = "…"]` 属性值（唯一覆盖默认推导的形态）。
fn path_attribute(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("path") {
            continue;
        }
        if let syn::Meta::NameValue(meta) = &attr.meta {
            if let syn::Expr::Lit(literal) = &meta.value {
                if let syn::Lit::Str(value) = &literal.lit {
                    return Some(value.value());
                }
            }
        }
    }
    None
}

/// `use` 树展开为路径字符串清单（`use a::{b, c}` → `a::b` / `a::c`；`as` 别名
/// 丢弃；`*` glob 记模块路径）。
fn flatten_use_tree(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<String>) {
    match tree {
        syn::UseTree::Path(node) => {
            prefix.push(node.ident.to_string());
            flatten_use_tree(&node.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(node) => {
            let mut segments = prefix.clone();
            segments.push(node.ident.to_string());
            out.push(segments.join("::"));
        }
        syn::UseTree::Rename(node) => {
            let mut segments = prefix.clone();
            segments.push(node.ident.to_string());
            out.push(segments.join("::"));
        }
        syn::UseTree::Glob(_) => {
            if !prefix.is_empty() {
                out.push(prefix.join("::"));
            }
        }
        syn::UseTree::Group(node) => {
            for item in &node.items {
                flatten_use_tree(item, prefix, out);
            }
        }
    }
}

/// 目录枚举（member glob 匹配用；跳过构建 / 依赖产物树与隐藏目录，深度受限）。
fn walk_dirs(root: &Path) -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    let mut frontier: Vec<(std::path::PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = frontier.pop() {
        if depth >= MEMBER_WALK_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.')
                || matches!(name.as_str(), "target" | "node_modules" | "dist" | "build")
            {
                continue;
            }
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let child = entry.path();
            dirs.push(child.clone());
            frontier.push((child, depth + 1));
        }
    }
    dirs
}

/// crate `src/` 下的 `.rs` 文件清单（孤儿检出用；跳过隐藏与产物树）。
fn walk_rs_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut frontier: Vec<std::path::PathBuf> = vec![dir.to_path_buf()];
    while let Some(current) = frontier.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || matches!(name.as_str(), "target" | "node_modules") {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                frontier.push(entry.path());
            } else if kind.is_file() && name.ends_with(".rs") {
                files.push(entry.path());
            }
        }
    }
    files
}
