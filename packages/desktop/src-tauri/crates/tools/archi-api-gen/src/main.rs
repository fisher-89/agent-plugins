use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use archi::api::paths;
use archi::api::{self, GeneratedFile};

/// CLI 子命令（封闭四值；未知参数归 `Usage`）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    /// 写四类生成物（幂等）
    Generate,
    /// 生成到内存后与磁盘比对，零写入
    Check,
    /// 输出候选草稿到 stdout
    Bootstrap(String),
    /// 打印用法并退出 2
    Usage,
}

/// 进程退出码（`generate` / `--check` 无差异 → 0；有差异或失败 → 1；用法错 → 2）。
///
/// 零子进程：本文件只读 argv 并调库，MUST NOT 派生任何外部进程（无 `Command`、无 shell）。
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repository_root();
    match run(parse_args(&args), &root) {
        Ok(code) => ExitCode::from(code as u8),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// argv → 子命令（`cargo run -p archi-api-gen -- --check` 的第一个 `--` 由 cargo 剥掉）。
fn parse_args(args: &[String]) -> Command {
    match args {
        [first] if first == "generate" => Command::Generate,
        [first] if first == "--check" || first == "check" => Command::Check,
        [first, element] if first == "bootstrap" => Command::Bootstrap(element.clone()),
        _ => Command::Usage,
    }
}

/// 子命令实现体（离 `main` 便于 tempdir 驱动；返回退出码）。
pub(crate) fn run(command: Command, root: &Path) -> Result<i32, String> {
    match command {
        Command::Generate => {
            let files = collect(root)?;
            for file in &files {
                write(root, file)?;
            }
            println!("{} 份生成物已写入", files.len());
            Ok(0)
        }
        Command::Check => {
            let expected = collect(root)?;
            match differences(root, &expected) {
                Report {
                    mismatched,
                    orphaned,
                    missing,
                } if mismatched.is_empty() && orphaned.is_empty() && missing.is_empty() => Ok(0),
                report => {
                    println!("{}", report.to_json());
                    Ok(1)
                }
            }
        }
        Command::Bootstrap(element) => {
            let text = api::bootstrap(root, &element).map_err(|error| error.message())?;
            println!("{text}");
            Ok(0)
        }
        Command::Usage => {
            println!(
                "用法：archi-api-gen <子命令>\n\
                 \x20 generate            写四类生成物（schema.json / INDEX.toml / .i.rs / .i.ts）\n\
                 \x20 --check             读盘比对，零写入；有差异即非零退出并列清单\n\
                 \x20 bootstrap <元素>    输出契约草稿到 stdout（不写盘）"
            );
            Ok(2)
        }
    }
}

/// 观察根锚定：从当前工作目录向上取**最近的、含架构域的祖先**（契约与模型都住在
/// 架构域里，故这是唯一有意义的锚）；一个都不命中 → 退化为当前目录。
///
/// 不做行内路径字面量：判据本身经 [`foundation::layout`] 派生（脚本可从
/// `packages/desktop` 起手调用，无需在脚本里写 `cd ../..`）。
fn repository_root() -> PathBuf {
    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut candidate = current.as_path();
    loop {
        if foundation::layout::archi_dir(candidate).is_dir() {
            return candidate.to_path_buf();
        }
        match candidate.parent() {
            Some(parent) => candidate = parent,
            None => return current,
        }
    }
}

/// 应生成物清单（元素表来自当前 root 的模型）。
fn collect(root: &Path) -> Result<Vec<GeneratedFile>, String> {
    let elements = model_elements(root)?;
    let snapshot = api::load_contracts(root, &elements).map_err(|error| error.message())?;
    api::render_all(root, &snapshot).map_err(|error| error.message())
}

/// 单份生成物写盘（父目录经 [`foundation::layout`] 派生的契约目录，或按目标路径自身
/// 的父目录创建——两者都已在 `archi` 侧算好，本层零拼装）。
///
/// **第二层拦截（限根）**：`archi::api::paths` 已做形态校验（D33），本层再以
/// `starts_with(root)` 复核目标确实落在观察根之下——写盘是本工具唯一的危险动作
/// （建目录 + 写文件可触达任意路径），两层判据的交叉验证是 `docs::resolved_within`
/// 的既有先例（判据 + join 后复核，任一层失效另一层仍拦得住）。
fn write(root: &Path, file: &GeneratedFile) -> Result<(), String> {
    if !file.path.starts_with(root) {
        return Err(format!(
            "生成物目标越出观察根（{} 不在 {} 之下）——拒绝写盘",
            file.path.display(),
            root.display()
        ));
    }
    if let Some(parent) = file.path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("目录创建失败（{}）：{error}", parent.display()))?;
    }
    fs::write(&file.path, &file.content)
        .map_err(|error| format!("写入失败（{}）：{error}", file.path.display()))
}

/// 磁盘 vs 应生成的三类偏差。
struct Report {
    /// 内容不符（文件在盘但字节不同）
    mismatched: Vec<String>,
    /// 应删未删（盘上存在但不在应生成集，且文件名匹配生成物命名规则）
    orphaned: Vec<String>,
    /// 应生成未生成
    missing: Vec<String>,
}

impl Report {
    /// `--check` 的 JSON 差异清单（stdout 单行，便于脚本解析）。
    fn to_json(&self) -> String {
        serde_json::json!({
            "mismatched": self.mismatched,
            "orphaned": self.orphaned,
            "missing": self.missing,
        })
        .to_string()
    }
}

/// 比对（零写入）。
fn differences(root: &Path, expected: &[GeneratedFile]) -> Report {
    let mut mismatched: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut expected_paths: BTreeSet<PathBuf> = BTreeSet::new();
    for file in expected {
        expected_paths.insert(file.path.clone());
        match fs::read_to_string(&file.path) {
            Ok(actual) => {
                if actual != file.content {
                    mismatched.push(archi::display(root, &file.path));
                }
            }
            Err(_) => missing.push(archi::display(root, &file.path)),
        }
    }
    let mut orphaned: Vec<String> = Vec::new();
    for path in generated_files_on_disk(root) {
        if expected_paths.contains(&path) {
            continue;
        }
        orphaned.push(archi::display(root, &path));
    }
    Report {
        mismatched,
        orphaned,
        missing,
    }
}

/// 盘上现有的三类生成物（D21 的「应删未删」取自这里）：
///
/// 1. 契约目录内 `*.schema.json` 与 `INDEX.toml`（递归）；
/// 2. **元素根同级**的 `*.i.rs` / `*.i.ts`——元素根可在仓库任意深度
///    （`crates/infra/archi` → `crates/infra/archi.i.rs`、`views/welcome-view.tsx` →
///    `views/welcome-view.tsx.i.ts`），故 MUST 全树递归找，只看仓库顶层会漏掉
///    `crates/**`、`src/**` 这些真实落点；
/// 3. 与 (2) 同一条命名判据，出现在契约目录之外的任何位置也算（生成物的**命名**
///    即身份，与落点无关）。
///
/// 递归跳过构建产物目录（`target` / `node_modules` / `dist` / `build` / `.git`）：
/// 它们不是生成物的合法落点，遍历它们只增加成本且会误报无关同名文件。
fn generated_files_on_disk(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let api_dir = paths::directory(root);
    let index = paths::index_path(root);
    walk(&api_dir, &mut |path| {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        if name.ends_with(".schema.json") || path == index {
            found.push(path.to_path_buf());
        }
        false
    });
    walk_modules(root, Path::new(""), 0, &mut found);
    found.sort();
    found.dedup();
    found
}

/// 生成物命名判据（`*.i.rs` / `*.i.ts`）：命名即身份，与落点深度无关。
fn is_generated_module_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".i.rs") || name.ends_with(".i.ts"))
}

/// 全树找元素根同级的生成物（递归、符号链接不跟随、构建产物目录剪枝、深度上限防环）。
fn walk_modules(dir: &Path, relative: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_MODULE_WALK_DEPTH {
        return;
    }
    let Ok(reader) = fs::read_dir(dir) else {
        return;
    };
    for entry in reader.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_symlink() {
            continue;
        }
        let child = relative.join(entry.file_name());
        if metadata.is_dir() {
            let name = child
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if SKIPPED_DIRECTORIES.contains(&name) {
                continue;
            }
            walk_modules(&path, &child, depth + 1, found);
            continue;
        }
        if is_generated_module_file(&path) {
            found.push(path);
        }
    }
}

/// 目录递归（符号链接不跟随；回调返回 `true` 即提前结束）。
fn walk(directory: &Path, visit: &mut impl FnMut(&Path) -> bool) {
    let Ok(reader) = fs::read_dir(directory) else {
        return;
    };
    for entry in reader.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            walk(&path, visit);
            continue;
        }
        if visit(&path) {
            return;
        }
    }
}

/// 递归遍历的跳过目录（构建产物 / 依赖 / 版本库元数据——生成物不落其中）。
const SKIPPED_DIRECTORIES: [&str; 5] = ["target", "node_modules", "dist", "build", ".git"];

/// 递归深度上限（防环；仓库内元素根最深也在十级以内）。
const MAX_MODULE_WALK_DEPTH: usize = 24;

/// 当前 root 的模型元素表（生成器的输入面；模型目录缺席 → 空表）。
fn model_elements(root: &Path) -> Result<Vec<archi::dto::ArchiElement>, String> {
    archi::dto::graph_view(root)
        .map(|graph| graph.elements)
        .map_err(|error| {
            format!("模型读取失败（生成器的输入面）：{error}；先修好模型再重跑 generate")
        })
}

#[cfg(test)]
mod main_test;
