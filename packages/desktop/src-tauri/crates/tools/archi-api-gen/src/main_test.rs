use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{parse_args, run, Command};

// ---------------------------------------------------------------------------
// 真实临时目录（无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-api-gen-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    fn read(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.0.join(relative)).ok()
    }

    fn exists(&self, relative: &str) -> bool {
        self.0.join(relative).exists()
    }

    /// 全量字节快照（零写入对拍）。
    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            let Ok(read) = fs::read_dir(dir) else {
                return;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, out);
                    continue;
                }
                let relative = path
                    .strip_prefix(base)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, fs::read(&path).expect("读文件失败")));
            }
        }
        let mut entries = Vec::new();
        walk(&self.0, &self.0, &mut entries);
        entries.sort();
        entries
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 夹具：模型 + 入口 + 契约
// ---------------------------------------------------------------------------

/// 后端 crate 形态元素的 FQN（试点）。
const CRATE_FQN: &str = "App.Backend.Core.Archi";

/// 前端单文件形态元素的 FQN。
const TS_FQN: &str = "App.Frontend.Views.Welcome";

/// 契约 md 内容（1 op + 1 类型；H1 = `fqn`）。
fn contract_markdown(fqn: &str) -> String {
    format!(
        "# {fqn}\n\n## 职能\n\n正文。\n\n## 操作\n\n### file_stem\n\n```json api-op\n\
         {{\"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
         \"returns\": {{\"type\": \"string\"}}}}\n```\n\n## 类型\n\n```json api-types\n\
         {{\"LocalArchiCheck\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
    )
}

/// 落一套可生成的后端 crate 元素（模型 + `src/lib.rs` + 契约）。
fn stage_crate(temp: &TempDir) {
    temp.write(
        ".dev-team/architecture/models/01-test.toml",
        "[element.\"App\"]\nkind = \"package\"\ngate = \"off\"\npath = []\n\n\
         [element.\"App.Backend\"]\nkind = \"domain\"\ngate = \"off\"\npath = []\n\n\
         [element.\"App.Backend.Core\"]\nkind = \"domain\"\ngate = \"off\"\npath = []\n\n\
         [element.\"App.Backend.Core.Archi\"]\nkind = \"module\"\ngate = \"off\"\n\
         path = [\"crates/core/archi/\"]\n",
    );
    temp.write(
        "crates/core/archi/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    temp.write(
        ".dev-team/architecture/api/Core/App.Backend.Core.Archi.md",
        &contract_markdown(CRATE_FQN),
    );
}

/// 落一套前端元素（模型 + 入口 + 契约）。
///
/// 入口文件落在 `<root>/index.ts`（目录形态）而非 `<root>` 本身：`paths::entry_form` 的
/// 四分支按「盘上命中哪个入口文件」判形态，前端元素的 `<root>` 若自带 `.tsx` 后缀，四
/// 分支的候选串（`<root>.ts` 等）结构上不可能命中——D41 因此规定「拿不到入口 ⇒ 该契约
/// 不产 `.i.*`」。本装置取可命中的目录形态，使 `.i.ts` 落点仍为
/// `views/welcome-view.tsx.i.ts`（`<root>` 同级、`<root>` 逐字不变）。
fn stage_ts(temp: &TempDir) {
    temp.write(
        ".dev-team/architecture/models/02-test.toml",
        "[element.\"App\"]\nkind = \"package\"\ngate = \"off\"\npath = []\n\n\
         [element.\"App.Frontend\"]\nkind = \"domain\"\ngate = \"off\"\npath = []\n\n\
         [element.\"App.Frontend.Views\"]\nkind = \"domain\"\ngate = \"off\"\npath = []\n\n\
         [element.\"App.Frontend.Views.Welcome\"]\nkind = \"module\"\ngate = \"off\"\n\
         path = [\"views/welcome-view.tsx\"]\n",
    );
    temp.write(
        "views/welcome-view.tsx/index.ts",
        "export function view(): string { return ''; }\n",
    );
    temp.write(
        ".dev-team/architecture/api/Views/App.Frontend.Views.Welcome.md",
        &contract_markdown(TS_FQN),
    );
}

// ---------------------------------------------------------------------------
// argv → Command
// ---------------------------------------------------------------------------

/// 正向：`generate` → `Generate`；`--check` / `check` → `Check`；
/// `bootstrap <元素>` → `Bootstrap`。
#[test]
fn argv解析三子命令() {
    assert_eq!(parse_args(&["generate".to_owned()]), Command::Generate);
    assert_eq!(parse_args(&["--check".to_owned()]), Command::Check);
    assert_eq!(
        parse_args(&["check".to_owned()]),
        Command::Check,
        "两种词面都接（D21 固定 `--check`，兼容 `check`）"
    );
    assert_eq!(
        parse_args(&["bootstrap".to_owned(), CRATE_FQN.to_owned()]),
        Command::Bootstrap(CRATE_FQN.to_owned())
    );
}

/// 边界：空 argv / 未知子命令 / `bootstrap` 缺元素 / 多余参数 / 大小写变体 →
/// 全 `Usage`（退出码 2 的入口）。
#[test]
fn argv解析异常面归usage() {
    let cases: [&[&str]; 7] = [
        &[],
        &["unknown"],
        &["bootstrap"],
        &["generate", "extra"],
        &["--check", "extra"],
        &["GENERATE"],
        &[""],
    ];
    for argv in cases {
        let owned: Vec<String> = argv.iter().map(|item| (*item).to_owned()).collect();
        assert_eq!(
            parse_args(&owned),
            Command::Usage,
            "argv {argv:?} 应归 Usage"
        );
    }
}

/// 正向：`Usage` 分支返回退出码 2（不 panic、零写盘）。
#[test]
fn usage分支退出码二() {
    let temp = TempDir::new("usage");
    let before = temp.snapshot();
    assert_eq!(
        run(Command::Usage, temp.root()).expect("Usage 不报 Err"),
        2,
        "用法错退出码 2"
    );
    assert_eq!(temp.snapshot(), before, "用法错零写盘");
}

// ---------------------------------------------------------------------------
// Generate
// ---------------------------------------------------------------------------

/// 正向（AC-12）：后端 crate 元素 → 磁盘出现 `<FQN>.schema.json` / `INDEX.toml` /
/// `<root>.i.rs` 三件；内容含契约类型名；返回 0。
#[test]
fn generate落四类生成物() {
    let temp = TempDir::new("generate-crate");
    stage_crate(&temp);
    assert_eq!(
        run(Command::Generate, temp.root()).expect("生成应成功"),
        0,
        "成功退出码 0"
    );

    let schema = temp
        .read(".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json")
        .expect("schema 生成物在盘");
    assert!(schema.contains("\"LocalArchiCheck\""), "schema 含类型定义");
    assert!(schema.contains("\"file_stem\""), "schema 含 op 键");

    let index = temp
        .read(".dev-team/architecture/api/INDEX.toml")
        .expect("INDEX 生成物在盘");
    assert!(index.contains("archi-api"), "INDEX 含生成器标识");
    assert!(
        index.contains(&format!("fqn = \"{CRATE_FQN}\"")),
        "INDEX 含元素 entry"
    );

    let module = temp
        .read("crates/core/archi.i.rs")
        .expect("`<root>.i.rs` 生成物在盘");
    assert!(
        module.contains("pub struct LocalArchiCheck;"),
        "生成物落元素根同级且含类型定义"
    );
    assert!(
        !temp.exists("crates/core/archi.i.ts"),
        "后端元素不产 `.i.ts`"
    );
}

/// 正向（AC-12 / D41）：前端元素 → 磁盘出现 `<root>.i.ts`（`views/welcome-view.tsx.i.ts`），
/// 含 `export interface`。形态由 `paths::entry_form` 判（本装置命中 `<root>/index.ts`，
/// 故为目录形态），落点仍是 `<root>` 同级。
#[test]
fn generate前端元素产i_ts() {
    let temp = TempDir::new("generate-ts");
    stage_ts(&temp);
    assert_eq!(run(Command::Generate, temp.root()).expect("生成应成功"), 0);
    let generated = temp
        .read("views/welcome-view.tsx.i.ts")
        .expect("`<root>.i.ts` 生成物在盘");
    assert!(
        generated.contains("export interface LocalArchiCheck"),
        "TS 生成物含 interface：\n{generated}"
    );
    assert!(
        !temp.exists("views/welcome-view.tsx.i.rs"),
        "前端元素不产 `.i.rs`"
    );
}

/// 边界（AC-2 / AC-12）：连跑两次 `Generate` → 第二次仍返回 0 且全量字节快照与第一次
/// 逐字节相同；随后 `Check` 返回 0。
#[test]
fn generate幂等且check随后通过() {
    let temp = TempDir::new("generate-idempotent");
    stage_crate(&temp);

    assert_eq!(run(Command::Generate, temp.root()).expect("首次生成"), 0);
    let first = temp.snapshot();
    assert_eq!(run(Command::Generate, temp.root()).expect("二次生成"), 0);
    assert_eq!(temp.snapshot(), first, "第二次字节逐字节相同");

    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不报错"),
        0,
        "生成后 --check 无差异即 0"
    );
    assert_eq!(temp.snapshot(), first, "--check 零写入");
}

/// 边界（AC-12 / AC-17）：目录创建经 `foundation::layout` 派生——初始无
/// `<archi_api_dir>` 时仍自动建成；来源文件内零路径字面量。
#[test]
fn generate目录创建经layout派生() {
    let temp = TempDir::new("generate-dirs");
    stage_crate(&temp);
    let api_dir = foundation::layout::archi_api_dir(temp.root());
    assert!(api_dir.exists(), "夹具已在契约目录下落地 md");
    // 清空契约目录（md 一并移除）→ 元素转 absent，INDEX 目录须由 layout 派生重建
    fs::remove_dir_all(&api_dir).expect("清空契约目录");
    assert!(!api_dir.exists(), "清空后目录不存在");

    assert_eq!(run(Command::Generate, temp.root()).expect("生成应成功"), 0);
    assert!(
        temp.exists(".dev-team/architecture/api/INDEX.toml"),
        "INDEX 落盘"
    );

    let source = include_str!("main.rs");
    for literal in [".dev-team", "architecture", "\"api\""] {
        assert!(
            !source.contains(literal),
            "来源文件零路径字面量 `{literal}`（目录全经 layout 派生）"
        );
    }
    assert!(
        source.contains("foundation::layout::archi_dir")
            || source.contains("foundation::layout::archi_api_dir"),
        "目录派生经 foundation::layout"
    );
}

/// 异常（D20）：契约 H1 与文件名不一致 → `Err(String)`、**零写入**（失败前不留半成品）。
#[test]
fn generate契约错误零写入() {
    let temp = TempDir::new("generate-bad-contract");
    stage_crate(&temp);
    temp.write(
        ".dev-team/architecture/api/Core/App.Backend.Core.Archi.md",
        "# App.Backend.Core.Other\n\n## 职能\n\n正文。\n",
    );
    let before = temp.snapshot();
    let error = run(Command::Generate, temp.root()).expect_err("坏契约应 Err");
    assert!(
        error.contains(CRATE_FQN) || error.contains("App.Backend.Core.Other"),
        "文案含契约定位：{error}"
    );
    assert_eq!(temp.snapshot(), before, "失败前零写入（不留半成品）");
}

/// 异常（D33 第二层拦截，写盘限根）：`write` 对落在观察根之外的目标 MUST 拒绝写盘
/// ——`archi::api::paths` 的形态校验是第一层（D33），本函数的 `starts_with(root)` 是
/// 第二层；两层判据交叉验证（`docs::resolved_within` 的既有先例），任一层失效另一层
/// 仍拦得住「任意文件写」。
#[test]
fn write拒绝越出观察根的生成物() {
    let temp = TempDir::new("write-out-of-root");
    let outside = std::env::temp_dir().join(format!(
        "archi-api-gen-outside-{}-{}",
        std::process::id(),
        "guard"
    ));
    let _ = fs::remove_file(&outside);

    let file = archi::api::GeneratedFile {
        path: outside.clone(),
        content: "// 越界\n".to_owned(),
    };
    let error = super::write(temp.root(), &file).expect_err("越出观察根应拒绝");
    assert!(error.contains("越出观察根"), "文案点明限根判据：{error}");
    assert!(!outside.exists(), "拒绝后客观外面零文件创建（零副作用）");

    // 反向：落点观察根之内 → 正常写入（判据不误伤）
    let inside = temp.root().join("ok.i.rs");
    super::write(
        temp.root(),
        &archi::api::GeneratedFile {
            path: inside.clone(),
            content: "// ok\n".to_owned(),
        },
    )
    .expect("根内目标应写入");
    assert!(inside.is_file(), "根内目标在盘");
}

// ---------------------------------------------------------------------------
// Check
// ---------------------------------------------------------------------------

/// 正向（AC-12）：`Generate` 后 `Check` → 返回 0；连跑两次 `Check` 退出码相同。
#[test]
fn check无差异() {
    let temp = TempDir::new("check-clean");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");
    assert_eq!(run(Command::Check, temp.root()).expect("check 应成功"), 0);
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 应成功"),
        0,
        "同输入两次退出码相同"
    );
}

/// 异常（AC-2 / D21）：手改 schema 一处字节 → `Check` 返回 1 且磁盘内容未被修复。
#[test]
fn check内容不符() {
    let temp = TempDir::new("check-mismatched");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    let relative = ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json";
    let original = temp.read(relative).expect("schema 在盘");
    let broken = original.replace("\"title\"", "\"titleX\"");
    assert_ne!(broken, original, "夹具应确实改到一处");
    temp.write(relative, &broken);

    assert_eq!(run(Command::Check, temp.root()).expect("check 不报错"), 1);
    assert_eq!(
        temp.read(relative).as_deref(),
        Some(broken.as_str()),
        "改坏的字节未被修复（零写入）"
    );
}

/// 异常（D21）：磁盘多一个匹配命名规则但不在应生成集的文件 → `Check` 返回 1 且该文件
/// 未被删除（零写入）。
#[test]
fn check孤儿生成物() {
    let temp = TempDir::new("check-orphaned");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    // 落一个匹配生成物命名规则、位于 root 顶层（元素根同级）的多余文件
    temp.write("stray-module.i.ts", "// 孤儿生成物\n");
    let before = temp.snapshot();
    assert_eq!(run(Command::Check, temp.root()).expect("check 不报错"), 1);
    assert_eq!(temp.snapshot(), before, "孤儿文件未被删除（零写入）");
    assert!(temp.exists("stray-module.i.ts"), "孤儿文件仍在盘");
}

/// 异常（D21）：契约新增但未跑 `Generate` → `Check` 返回 1（`missing` 非空）。
#[test]
fn check应生成未生成() {
    let temp = TempDir::new("check-missing");
    stage_crate(&temp);
    // 不跑 Generate，直接 Check
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不报错"),
        1,
        "应生成未生成即非零退出"
    );
}

/// 边界（AC-12）：三种偏差形态各跑一次 `Check`，调用前后 tempdir 全量快照逐字节相等；
/// 来源文件内 `fs::write` / `create_dir_all` 只在 `Generate` 的 `write` 路径出现。
#[test]
fn check零写盘总断言() {
    let temp = TempDir::new("check-no-write");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    // 形态一：无差异
    let before = temp.snapshot();
    let _ = run(Command::Check, temp.root()).expect("check 不报错");
    assert_eq!(temp.snapshot(), before, "无差异形态零写入");

    // 形态二：内容不符
    let relative = ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json";
    temp.write(relative, "{}");
    let before = temp.snapshot();
    let _ = run(Command::Check, temp.root()).expect("check 不报错");
    assert_eq!(temp.snapshot(), before, "内容不符形态零写入");

    // 形态三：孤儿 + 缺失并存
    temp.write("orphan-module.i.rs", "// orphan");
    fs::remove_file(temp.root().join("crates/core/archi.i.rs")).expect("移除应生成物");
    let before = temp.snapshot();
    let _ = run(Command::Check, temp.root()).expect("check 不报错");
    assert_eq!(temp.snapshot(), before, "孤儿 + 缺失形态零写入");

    let source = include_str!("main.rs");
    assert_eq!(
        source.matches("fs::write").count(),
        1,
        "`fs::write` 只出现一处（Generate 的 write 函数）"
    );
    assert_eq!(
        source.matches("create_dir_all").count(),
        1,
        "`create_dir_all` 只出现一处（Generate 的 write 函数）"
    );
}

/// 边界（D17）：无契约面 → 应生成集只含 `INDEX.toml`；盘上无生成物 → 非零退出
/// （`missing` 含 INDEX）；不 panic 且零写入。
#[test]
fn check未启用契约面() {
    let temp = TempDir::new("check-no-contract-face");
    temp.write(
        ".dev-team/architecture/models/01-test.toml",
        "[element.\"App\"]\nkind = \"package\"\ngate = \"off\"\n\n\
         [element.\"App.Backend\"]\nkind = \"domain\"\ngate = \"off\"\n\n\
         [element.\"App.Backend.Core\"]\nkind = \"domain\"\ngate = \"off\"\n\n\
         [element.\"App.Backend.Core.Archi\"]\nkind = \"module\"\ngate = \"off\"\n\
         path = [\"crates/core/archi/\"]\n",
    );
    temp.write("crates/core/archi/src/lib.rs", "pub fn f() {}\n");

    let before = temp.snapshot();
    let code = run(Command::Check, temp.root()).expect("check 不 panic");
    assert_eq!(
        code, 1,
        "契约面未启用但 INDEX 属应生成物 → 首轮应报缺失（非零退出）"
    );
    assert_eq!(temp.snapshot(), before, "零写入");

    // 生成后即通过（应生成集收敛为 INDEX 一件）
    assert_eq!(run(Command::Generate, temp.root()).expect("生成应成功"), 0);
    assert!(
        temp.exists(".dev-team/architecture/api/INDEX.toml"),
        "INDEX 落盘"
    );
    assert_eq!(run(Command::Check, temp.root()).expect("check 应成功"), 0);
}

/// 异常（AC-2 / D26）：手改 INDEX.toml 的 `categories` 段 → `Check` 返回 1 且磁盘未被
/// 修复；重跑 `Generate` 后该段逐字节复原（纯派生）。
#[test]
fn check检出categories差异() {
    let temp = TempDir::new("check-categories");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    let relative = ".dev-team/architecture/api/INDEX.toml";
    let original = temp.read(relative).expect("INDEX 在盘");
    assert!(
        original.contains("[entry.categories]"),
        "试点契约应产出 categories 子表：\n{original}"
    );
    let broken = original.replace("object = 1", "object = 2");
    assert_ne!(broken, original, "夹具应确实改到一处");
    temp.write(relative, &broken);
    assert_eq!(run(Command::Check, temp.root()).expect("check 不报错"), 1);
    assert_eq!(
        temp.read(relative).as_deref(),
        Some(broken.as_str()),
        "改坏的内容未被修复（零写入）"
    );

    assert_eq!(run(Command::Generate, temp.root()).expect("生成应成功"), 0);
    assert_eq!(
        temp.read(relative).as_deref(),
        Some(original.as_str()),
        "categories 是纯派生：重跑 Generate 逐字节复原"
    );
}

/// 边界（D27）：契约含「`properties` 全为函数却未标 `x-class`」→ `Generate` / `Check`
/// 退出码仍 0；同输入两次 `Check` 退出码相同（警告只走 stderr）。
#[test]
fn check不受x_class警告影响() {
    let temp = TempDir::new("check-x-class-warning");
    stage_crate(&temp);
    temp.write(
        ".dev-team/architecture/api/Core/App.Backend.Core.Archi.md",
        &format!(
            "# {CRATE_FQN}\n\n```json api-types\n{{\"Hooks\": {{\"type\": \"object\", \
             \"properties\": {{\"run\": {{\"x-function\": {{\"params\": [], \"returns\": \
             {{\"type\": \"string\"}}}}}}}}}}}}\n```\n"
        ),
    );
    assert_eq!(
        run(Command::Generate, temp.root()).expect("生成应成功"),
        0,
        "警告不改生成退出码"
    );
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 应成功"),
        0,
        "警告不改 --check 退出码"
    );
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 应成功"),
        0,
        "同输入两次退出码相同"
    );
}

/// 边界（AC-12 / D29）：生成物是格式化器语义的不动点——`.i.rs` 零尾随空白 / 无 tab /
/// 末尾换行；本工具内零 `std::process`（格式化器不 spawn）。
#[test]
fn generate产物为格式化器不动点() {
    let temp = TempDir::new("generate-format");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    for relative in [
        "crates/core/archi.i.rs",
        ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json",
        ".dev-team/architecture/api/INDEX.toml",
    ] {
        let text = temp
            .read(relative)
            .unwrap_or_else(|| panic!("{relative} 应在盘"));
        for line in text.lines() {
            assert_eq!(line, line.trim_end(), "{relative} 零尾随空白：{line:?}");
            assert!(!line.contains('\t'), "{relative} 零 tab 缩进");
        }
        assert!(text.ends_with('\n'), "{relative} 末尾以换行收束");
    }

    let source = include_str!("main.rs");
    assert_eq!(
        source.matches("std::process").count(),
        1,
        "`std::process` 恰一处（`ExitCode` 导入；零 spawn）"
    );
    assert!(
        !source.contains("Command::new"),
        "零 `Command::new`（不派生子进程）"
    );
}

// ---------------------------------------------------------------------------
// repository_root：观察根锚定（D32）
// ---------------------------------------------------------------------------

/// 正向（D32 / AC-12）：直调 `super::repository_root()`（私有函数对子模块可见 ⇒ 无需
/// 放宽可见性）——返回值是 cwd 的祖先且 `current_dir()` 以它开头；「最近」判据：返回
/// 值再上一层使 `archi_dir(..).is_dir()` 为假（唯一命中，不是最外层祖先）；连续两次
/// 调用逐字相等（无状态、幂等）。
#[test]
fn repository_root向上锚定到最近架构域祖先() {
    let root = super::repository_root();
    let current = std::env::current_dir().expect("读取 cwd");

    assert!(
        current.starts_with(&root),
        "返回值是 cwd 的祖先（或相等）：root = {root:?} / cwd = {current:?}"
    );
    assert!(!root.as_os_str().is_empty(), "返回值非空（零退化到空路径）");

    // cwd 自身不是架构域时，root MUST 是真祖先——这一条即否证「root 取 current_dir()」
    if !foundation::layout::archi_dir(&current).is_dir() {
        assert_ne!(
            root, current,
            "cwd 自身无架构域 ⇒ root MUST 是更上层祖先（MUST NOT 直接取 cwd，D32）"
        );
        assert!(
            foundation::layout::archi_dir(&root).is_dir(),
            "命中的祖先自身必须有架构域：root = {root:?}"
        );
        let parent = root.parent().expect("命中分支的祖先必有父目录");
        assert!(
            !foundation::layout::archi_dir(parent).is_dir(),
            "「最近」判据：再上一层不命中：{parent:?}"
        );
    }

    assert_eq!(
        super::repository_root(),
        root,
        "无状态、幂等（两次调用逐字相等）"
    );
}

/// 边界（D32）：「全不命中」需 cwd 起于无架构域之树，本进程内不可构造（改 cwd 会污染
/// 同进程其它用例）⇒ 以**不变量**表达：返回值恒满足「`archi_dir(返回).is_dir()`」**或**
/// 「返回值 == `current_dir()`」二者之一（退化分支的唯一合法形态），且可作 `Path` 使用。
#[test]
fn repository_root退化为cwd的不变量() {
    let root = super::repository_root();
    let current = std::env::current_dir().expect("读取 cwd");

    let hit = foundation::layout::archi_dir(&root).is_dir();
    let degraded = root == current;
    assert!(
        hit || degraded,
        "恒为「命中祖先」或「退化为 cwd」之一：root = {root:?} / cwd = {current:?}"
    );
    assert!(!root.as_os_str().is_empty(), "MUST NOT 为空路径");
    assert!(
        root.parent().is_some() || root == Path::new("/"),
        "可作 Path 使用（父目录派生不 panic）：{root:?}"
    );
    // 派生不 panic（纯推导，零 IO 副作用）
    assert!(
        foundation::layout::archi_dir(&root).components().count() > 0,
        "archi_dir 派生不 panic：{root:?}"
    );
    assert!(
        foundation::layout::archi_api_dir(&root)
            .components()
            .count()
            > 0,
        "archi_api_dir 派生不 panic：{root:?}"
    );
}

/// 边界（D32 / 零行内字面量）：`include_str!("main.rs")` 的四条机械断言——
/// `fn repository_root` 恰 1 处；`archi_dir(candidate).is_dir()` 恰 1 命中（锚定判据经
/// `foundation::layout` 派生）；`current_dir` 恰 1 处且不出现在 `main` 的 root 取用处
/// （`main` 内只有 `let root = repository_root();` 形态）；零路径字面量。
#[test]
fn repository_root源码面机械断言() {
    let source = include_str!("main.rs");
    assert_eq!(
        source.matches("fn repository_root").count(),
        1,
        "`fn repository_root` 恰一处（单点锚定）"
    );
    assert_eq!(
        source.matches("archi_dir(candidate).is_dir()").count(),
        1,
        "锚定判据经 foundation::layout 派生且恰一处"
    );
    assert_eq!(
        source.matches("current_dir").count(),
        1,
        "`current_dir` 恰一处（只在 repository_root 内）"
    );
    assert!(
        source.contains("let root = repository_root();"),
        "main 内只有 `let root = repository_root();` 形态"
    );
    assert!(
        !source.contains("let root = std::env::current_dir"),
        "MUST NOT 把 current_dir() 直接当 root"
    );
    for literal in [".dev-team", "architecture", "\"api\""] {
        assert!(
            !source.contains(literal),
            "零路径字面量 `{literal}`（一律经 foundation::layout 派生）"
        );
    }
}

/// 异常（禁 test-only `pub`）：`repository_root` 无 `pub` / `pub(crate)` 前缀；`main.rs`
/// 的 `pub(crate)` 面**只有** `run`（`pub(crate) fn` 恰 1 处）——本用例存在即证明该纪律。
#[test]
fn repository_root保持私有() {
    let source = include_str!("main.rs");
    assert_eq!(
        source.matches("pub(crate) fn").count(),
        1,
        "`pub(crate) fn` 恰一处（只有 run）"
    );
    assert!(
        source.contains("pub(crate) fn run"),
        "唯一 pub(crate) 面是 run"
    );
    assert!(
        !source.contains("pub fn repository_root")
            && !source.contains("pub(crate) fn repository_root"),
        "MUST NOT 为可测性给 repository_root 加可见性"
    );
    assert!(
        source.contains("fn repository_root() -> PathBuf"),
        "私有形态在案（签名零形参）"
    );
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

/// 正向（AC-12）：`Bootstrap(<试点元素>)` → 返回 0、输出含 H1 与 `api-op` fence；
/// 调用前后 tempdir 快照相等（零写盘）。
#[test]
fn bootstrap只出stdout() {
    let temp = TempDir::new("bootstrap");
    stage_crate(&temp);
    let before = temp.snapshot();
    assert_eq!(
        run(Command::Bootstrap(CRATE_FQN.to_owned()), temp.root()).expect("起草应成功"),
        0
    );
    assert_eq!(temp.snapshot(), before, "Bootstrap 零写盘");

    // 文本内容面由 api::bootstrap 保证（此处仅断言命令不退化为写盘）
    let source = include_str!("main.rs");
    assert!(
        source.contains("println!(\"{text}\")"),
        "Bootstrap 只打 stdout"
    );
}

/// 边界（**D39**）：`main.rs` 内 `knip` **零命中**、零子进程（`Command::new` 零命中）；
/// 行为面：草稿文本内**零 `unused`** 词（`Visibility::Unused` 生产面不可达，`bootstrap`
/// 不产生该降级）；`--check` 的退出码不受任何 knip 相关分支影响（`Check` 的返回码与
/// 有无 knip 无关——本工具根本不读它）。
#[test]
fn bootstrap不读knip() {
    let source = include_str!("main.rs");
    assert_eq!(
        source.matches("knip").count(),
        0,
        "`main.rs` 内零 `knip`（D39：人工驱动，不由工具调）"
    );
    assert_eq!(
        source.matches("Command::new").count(),
        0,
        "零子进程（`api/**` 与工具 crate 均零 spawn）"
    );

    let temp = TempDir::new("bootstrap-no-knip");
    stage_crate(&temp);
    // 行为面：草稿文本零 `unused` 词（生产面不可达）
    let draft = archi::api::bootstrap(temp.root(), CRATE_FQN).expect("起草应成功");
    assert!(
        !draft.contains("unused"),
        "草稿内零 `unused` 词（Unused 生产面不可达）：
{draft}"
    );

    // `--check` 的退出码与 knip 无关：先 Generate 再 Check → 恒 0
    assert_eq!(run(Command::Generate, temp.root()).expect("生成应成功"), 0);
    assert_eq!(
        run(Command::Check, temp.root()).expect("无差异应 0"),
        0,
        "`--check` 退出码不受任何 knip 相关分支影响"
    );
}

/// 异常：`Bootstrap("App.Nope")` → `Err(String)` 且文案含该 FQN；零写盘。
#[test]
fn bootstrap元素不存在() {
    let temp = TempDir::new("bootstrap-missing");
    stage_crate(&temp);
    let before = temp.snapshot();
    let error =
        run(Command::Bootstrap("App.Nope".to_owned()), temp.root()).expect_err("元素不存在应 Err");
    assert!(error.contains("App.Nope"), "文案含 FQN：{error}");
    assert_eq!(temp.snapshot(), before, "失败时零写盘");
}

// ---------------------------------------------------------------------------
// 边界：空 root / 不存在 root
// ---------------------------------------------------------------------------

/// 边界：`root` 指向不存在路径 → `Generate` 不 panic（模型目录缺席即空元素表 → 只落
/// INDEX 或返回 Err，两者都合法）；不残留半写文件。
#[test]
fn 空root不panic() {
    let missing = std::env::temp_dir().join(format!(
        "archi-api-gen-missing-root-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&missing);

    let outcome = run(Command::Generate, &missing);
    match outcome {
        Ok(code) => {
            assert_eq!(code, 0, "成功路径退出码 0");
            // 只会落 INDEX.toml（若无元素则连它也不落）
            let index = missing
                .join(".dev-team")
                .join("architecture")
                .join("api")
                .join("INDEX.toml");
            if index.exists() {
                let text = fs::read_to_string(&index).expect("读 INDEX 失败");
                assert!(!text.contains("[[entry]]"), "空元素表零 entry");
            }
        }
        Err(message) => assert!(!message.is_empty(), "错误路径文案非空"),
    }
    let _ = fs::remove_dir_all(&missing);

    // 空 Path 不 panic
    let empty = Path::new("");
    let _ = run(Command::Check, empty);
}

// ---------------------------------------------------------------------------
// 退出码面三分 / 零 spawn（AC-12）
// ---------------------------------------------------------------------------

/// 边界（AC-12）：退出码面三分互不混用——`0`（成功 / 无差异）/ `1`（有差异）/
/// `2`（用法错）；同一输入两次运行退出码相同（确定性）。
#[test]
fn 退出码面三分互不混用() {
    // 2：用法错
    let usage = TempDir::new("exit-code-usage");
    assert_eq!(
        run(Command::Usage, usage.root()).expect("Usage 不 Err"),
        2,
        "用法错 → 2"
    );

    // 0 / 1：同一棵树上「有差异」→「无差异」
    let temp = TempDir::new("exit-code-diff");
    stage_crate(&temp);
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不 Err"),
        1,
        "应生成未生成 → 1"
    );
    let first = run(Command::Check, temp.root()).expect("check 不 Err");
    let second = run(Command::Check, temp.root()).expect("check 不 Err");
    assert_eq!(first, second, "同输入两次退出码相同（确定性）");

    assert_eq!(
        run(Command::Generate, temp.root()).expect("生成应成功"),
        0,
        "生成成功 → 0"
    );
    let clean_first = run(Command::Check, temp.root()).expect("check 不 Err");
    let clean_second = run(Command::Check, temp.root()).expect("check 不 Err");
    assert_eq!(clean_first, 0, "无差异 → 0");
    assert_eq!(clean_first, clean_second, "同输入两次退出码相同");

    assert_ne!(clean_first, first, "三类退出码互不混用（0 ≠ 1）");
    assert_ne!(clean_first, 2, "成功码不等于用法错码");
    assert_ne!(first, 2, "差异码不等于用法错码");
}

/// 边界（AC-12 红线）：零 spawn——`main.rs` 内 `Command::new` / `process::exit`
/// **双零命中**；`std::process` 只出现恰一处（`ExitCode` 导入，`main` 返回
/// `ExitCode` 所需），不是 spawn 判据。
#[test]
fn 零spawn双零命中() {
    let source = include_str!("main.rs");
    for forbidden in ["Command::new", "process::exit"] {
        assert_eq!(
            source.matches(forbidden).count(),
            0,
            "`main.rs` 内零 `{forbidden}`（不派生子进程、不 process::exit）"
        );
    }
    assert_eq!(
        source.matches("std::process").count(),
        1,
        "`std::process` 恰一处（`ExitCode` 导入；路径本身不作 spawn 判据）"
    );
    assert!(
        source.contains("use std::process::ExitCode;"),
        "唯一使用是 `ExitCode` 导入"
    );
    assert!(
        source.contains("fn main() -> ExitCode"),
        "`main` 返回 `ExitCode`（进程边界的表达）"
    );
}

/// 正向（AC-12 / D21）：`--check` 的差异清单形态——`Report` 的三类偏差各自独立成数组、
/// 序列化为单行 JSON 且键名固定（`mismatched` / `orphaned` / `missing`），三类并存时
/// 互不吞并。stdout **不捕获**（沿设计「不捕获真实 stdout」），故判据落在源码面 +
/// 行为面（三类各自单独可检出）两向。
#[test]
fn check差异清单三数组独立且键名固定() {
    let source = include_str!("main.rs");
    for key in ["mismatched", "orphaned", "missing"] {
        assert!(
            source.contains(&format!("\"{key}\": ")),
            "差异清单含键 `{key}`（单行 JSON）"
        );
    }
    assert!(
        source.contains("fn to_json"),
        "差异清单的序列化单点在 `Report::to_json`"
    );

    // 行为面：三类偏差各自单独可检出（互不吞并的最小形态）
    let mismatched = TempDir::new("diff-三数组-mismatched");
    stage_crate(&mismatched);
    run(Command::Generate, mismatched.root()).expect("生成应成功");
    let relative = ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json";
    let original = mismatched.read(relative).expect("schema 在盘");
    mismatched.write(relative, &original.replace("\"title\"", "\"titleX\""));
    assert_eq!(
        run(Command::Check, mismatched.root()).expect("check 不 Err"),
        1,
        "内容不符单独可检出"
    );

    let orphaned = TempDir::new("diff-三数组-orphaned");
    stage_crate(&orphaned);
    run(Command::Generate, orphaned.root()).expect("生成应成功");
    orphaned.write("stray.i.rs", "// 孤儿\n");
    assert_eq!(
        run(Command::Check, orphaned.root()).expect("check 不 Err"),
        1,
        "孤儿生成物单独可检出"
    );

    let missing = TempDir::new("diff-三数组-missing");
    stage_crate(&missing);
    assert_eq!(
        run(Command::Check, missing.root()).expect("check 不 Err"),
        1,
        "应生成未生成单独可检出"
    );

    // 三类偏差同时存在 → 三个数组各自非空（互不吞并）；判据经 run 返回码 + 落盘不变表达
    let all = TempDir::new("diff-三数组-并存");
    stage_crate(&all);
    run(Command::Generate, all.root()).expect("生成应成功");
    let schema_rel = ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json";
    let schema_text = all.read(schema_rel).expect("schema 在盘");
    all.write(schema_rel, &schema_text.replace("\"title\"", "\"titleX\""));
    all.write(
        "stray.i.rs",
        "// 孤儿
",
    );
    fs::remove_file(all.root().join("crates/core/archi.i.rs")).expect("移除一件应生成物");
    let before = all.snapshot();
    assert_eq!(
        run(Command::Check, all.root()).expect("check 不 Err"),
        1,
        "三类偏差并存仍统一报非零"
    );
    assert_eq!(all.snapshot(), before, "三类并存的判据零写入");

    // 源码自证：三类偏差各自独立成数组（三个 push 目标互不共用）
    for needle in ["mismatched.push(", "orphaned.push(", "missing.push("] {
        assert!(
            source.contains(needle),
            "三类偏差各有独立收集：`{needle}` 应在案"
        );
    }
}

/// 边界（AC-12 / D29）：生成物的不动点守卫有**两态**——生成后 `.i.rs` 满足格式化器
/// 形态（无尾随空白 / 无 tab / 以换行收束）；手工改坏一处（注入双空格与尾随空白）后
/// `--check` 报 `mismatched` 而**不是**静默通过。本工具内零 `std::process`（不 spawn
/// 格式化器，不动点靠模板 + 外部 `--check` 门兜住）。
#[test]
fn check不动点守卫两态() {
    let temp = TempDir::new("check-format-guard");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");
    let relative = "crates/core/archi.i.rs";
    let generated = temp.read(relative).expect("生成物在盘");
    for line in generated.lines() {
        assert_eq!(line, line.trim_end(), "不动点态零尾随空白：{line:?}");
        assert!(!line.contains('\t'), "不动点态零 tab 缩进");
    }
    assert!(generated.ends_with('\n'), "不动点态以换行收束");
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不 Err"),
        0,
        "不动点态零差异"
    );

    // 人为破坏：注入尾随空白（格式化器会改写这一行）
    temp.write(relative, &generated.replace("pub struct", "pub  struct"));
    let before = temp.read(relative).expect("破坏后在盘");
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不 Err"),
        1,
        "人为破坏即报差异（不静默通过）"
    );
    assert_eq!(
        temp.read(relative).as_deref(),
        Some(before.as_str()),
        "零写入：破坏后的字节未被修复"
    );

    // 零 spawn：格式化器不在本工具内运行
    let source = include_str!("main.rs");
    assert!(!source.contains("Command::new"), "零子进程");
    assert!(
        !source.contains("rustfmt") && !source.contains("prettier"),
        "不在渲染路径 spawn 格式化器（不动点靠模板）"
    );
}

// ---------------------------------------------------------------------------
// D21 三类偏差之「应删未删」：孤儿扫描的覆盖范围
// ---------------------------------------------------------------------------

/// 正向（D21）：元素根同级的生成物可在**仓库任意深度**（`crates/core/archi.i.rs`、
/// `views/welcome-view.tsx.i.ts`），故孤儿扫描 MUST 全树递归——只看仓库顶层会把
/// 这些真实落点上的孤儿判成「不存在」，`api:check` 因此对它们失明。
#[test]
fn check孤儿扫描覆盖任意深度的元素根同级() {
    let temp = TempDir::new("check-orphaned-deep");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    // 深度二：`crates/core/` 下的孤儿（与真实落点同层）
    temp.write("crates/core/stale.i.rs", "// 孤儿（深度二）\n");
    // 深度一：前端单文件元素的真实落点层
    temp.write("views/stale.tsx.i.ts", "// 孤儿（深度一）\n");
    // 深层嵌套：任意深度的元素根同级都算
    temp.write("a/b/c/d/stale.i.ts", "// 孤儿（深层）\n");

    let before = temp.snapshot();
    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不 Err"),
        1,
        "任意深度的孤儿生成物必须可检出"
    );
    assert_eq!(temp.snapshot(), before, "孤儿扫描零写入");
    for relative in [
        "crates/core/stale.i.rs",
        "views/stale.tsx.i.ts",
        "a/b/c/d/stale.i.ts",
    ] {
        assert!(temp.exists(relative), "孤儿文件未被删除：{relative}");
    }
}

/// 边界（D21）：孤儿扫描跳过构建产物目录（`target` / `node_modules` / `dist` /
/// `build` / `.git`）——它们不是生成物的合法落点，遍历它们既慢又会把无关同名文件
/// 误报成孤儿。
#[test]
fn check孤儿扫描跳过构建产物目录() {
    let temp = TempDir::new("check-orphaned-skipped");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    temp.write("target/debug/stale.i.rs", "// 构建产物里的同名文件\n");
    temp.write("node_modules/pkg/stale.i.ts", "// 依赖里的同名文件\n");

    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不 Err"),
        0,
        "构建产物 / 依赖目录里的同名文件不算孤儿"
    );
}

/// 异常（D21）：孤儿扫描只认 `*.i.rs` / `*.i.ts` 与契约目录内的 `*.schema.json` /
/// `INDEX.toml`——普通源文件与任意其他 `.ts` / `.rs` 不参与（命名即身份，零误伤）。
#[test]
fn check孤儿扫描零误伤普通源文件() {
    let temp = TempDir::new("check-orphaned-scope");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    temp.write("crates/core/archi/src/other.rs", "pub fn other() {}\n");
    temp.write("views/normal.tsx", "export const x = 1;\n");
    temp.write("crates/core/notes.md", "# 笔记\n");

    assert_eq!(
        run(Command::Check, temp.root()).expect("check 不 Err"),
        0,
        "非生成物命名的文件不参与孤儿判据"
    );
}

/// 边界（源码自证）：递归遍历的三条纪律在案——命名判据单点（`is_generated_module_file`）、
/// 跳过目录表单点（`SKIPPED_DIRECTORIES`）、深度上限（`MAX_MODULE_WALK_DEPTH`）。
#[test]
fn check孤儿扫描的三条纪律源码面() {
    let source = include_str!("main.rs");
    assert_eq!(
        source.matches("fn is_generated_module_file").count(),
        1,
        "命名判据单点"
    );
    assert!(
        source.contains("SKIPPED_DIRECTORIES") && source.contains("MAX_MODULE_WALK_DEPTH"),
        "跳过目录表与深度上限在案"
    );
    assert_eq!(
        source.matches("fn walk_modules").count(),
        1,
        "元素根同级的递归遍历单点"
    );
}

/// 正向（D21 差异清单内容面）：`--check` 的 JSON 清单**逐数组点名该文件**——三类偏差
/// 各只命中自己的数组，不互相串位（「应删未删」的孤儿必须出现在 `orphaned` 里而不是
/// 只靠退出码可判）。
///
/// `differences` 是私有函数、`main_test` 是本文件的 `#[cfg(test)]` 子模块，故经
/// `super::differences` 直调即可观测真实清单（MUST NOT 为测试放宽可见性）。
#[test]
fn check差异清单逐数组点名该文件() {
    let temp = TempDir::new("check-report-content");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");

    // 基线：无差异 → 三数组全空
    let clean = super::differences(temp.root(), &super::collect(temp.root()).expect("应生成集"));
    assert!(clean.mismatched.is_empty(), "无差异：mismatched 空");
    assert!(clean.orphaned.is_empty(), "无差异：orphaned 空");
    assert!(clean.missing.is_empty(), "无差异：missing 空");
    assert_eq!(
        clean.to_json(),
        "{\"mismatched\":[],\"orphaned\":[],\"missing\":[]}",
        "三键恒在场（零省略）"
    );

    // 内容不符：mismatched 点名该文件，另两数组空
    let mismatched = TempDir::new("check-report-mismatched");
    stage_crate(&mismatched);
    run(Command::Generate, mismatched.root()).expect("生成应成功");
    let schema_rel = ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json";
    let original = mismatched.read(schema_rel).expect("schema 在盘");
    mismatched.write(schema_rel, &original.replace("\"title\"", "\"titleX\""));
    let report = super::differences(
        mismatched.root(),
        &super::collect(mismatched.root()).expect("应生成集"),
    );
    assert_eq!(report.mismatched, vec![schema_rel.to_owned()], "点名该文件");
    assert!(report.orphaned.is_empty() && report.missing.is_empty());
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).expect("单行 JSON");
    assert_eq!(
        json["mismatched"],
        serde_json::json!([schema_rel]),
        "mismatched 数组含该文件（repo 相对 POSIX 串）"
    );

    // 应删未删：orphaned 点名该文件，另两数组空
    let orphaned = TempDir::new("check-report-orphaned");
    stage_crate(&orphaned);
    run(Command::Generate, orphaned.root()).expect("生成应成功");
    orphaned.write("crates/core/stale.i.rs", "// 孤儿\n");
    let report = super::differences(
        orphaned.root(),
        &super::collect(orphaned.root()).expect("应生成集"),
    );
    assert_eq!(
        report.orphaned,
        vec!["crates/core/stale.i.rs".to_owned()],
        "orphaned 点名该文件（不再只靠退出码可判）"
    );
    assert!(report.mismatched.is_empty() && report.missing.is_empty());

    // 应生成未生成：missing 点名该文件，另两数组空
    let missing = TempDir::new("check-report-missing");
    stage_crate(&missing);
    let report = super::differences(
        missing.root(),
        &super::collect(missing.root()).expect("应生成集"),
    );
    assert!(
        report.missing.iter().any(
            |path| path == ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json"
        ),
        "missing 点名未生成的 schema：{:?}",
        report.missing
    );
    assert!(report.mismatched.is_empty() && report.orphaned.is_empty());
}

// ---------------------------------------------------------------------------
// `display` 跨 crate 单点（D55 / AC-31 ⑤）
// ---------------------------------------------------------------------------

/// 工具侧 MUST NOT 自持一份路径归一实现：本地 `fn display` 已删，调用面是
/// `archi::display`（D55 的唯一单点）。
#[test]
fn 路径归一单点跨crate复用且本地零副本() {
    let source = include_str!("main.rs");
    assert_eq!(
        source
            .lines()
            .filter(|line| line.starts_with("fn display"))
            .count(),
        0,
        "本地自由函数 `fn display` 已删（D55）"
    );
    assert!(
        source.matches("archi::display").count() >= 1,
        "调用面改指跨 crate 单点"
    );
}

/// 行为面：差异清单的路径串与 `archi::display` 同源（剥 root、`\` → `/`）。
#[test]
fn 差异清单路径串与跨crate单点同源() {
    let temp = TempDir::new("display-single-point");
    stage_crate(&temp);
    run(Command::Generate, temp.root()).expect("生成应成功");
    let schema_rel = ".dev-team/architecture/api/Core/App.Backend.Core.Archi.schema.json";
    let absolute = temp.root().join(schema_rel);
    assert_eq!(
        archi::display(temp.root(), &absolute),
        schema_rel,
        "跨 crate 单点的输出即差异清单的串形态"
    );

    let original = temp.read(schema_rel).expect("schema 在盘");
    temp.write(schema_rel, &original.replace("\"title\"", "\"titleX\""));
    let report = super::differences(temp.root(), &super::collect(temp.root()).expect("应生成集"));
    assert_eq!(
        report.mismatched,
        vec![archi::display(temp.root(), &absolute)],
        "清单串与单点逐字相等（零第二份归一）"
    );
}
