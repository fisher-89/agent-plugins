//! `parse`（聚合层）的单元测试：`parse_dir` 的文件名字典序聚合、`file` 回填、
//! 跨文件重复 FQN 检出、读盘失败诊断与目录缺席 / 空目录的冷启动合法起点；以及
//! `SourceParser::parse` 的单文件契约（`file` 留空、行号相对源文本）。
//!
//! 零 mock：真实临时目录 + 真实 `toml` 解析 + 真实 `foundation::layout` 派生
//! （回填、字典序、读盘失败均为真实文件系统语义）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_models_dir;

use crate::dto::DiagnosticKind;

use super::{parse_dir, SourceParser, TomlParser};

// ---------------------------------------------------------------------------
// 真实临时目录（RAII；本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-parse-test-{}-{}-{}",
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
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 在 `<root>/.dev-team/architecture/models/` 下落一个模型文件。
fn write_model(root: &Path, name: &str, content: &str) {
    let dir = archi_models_dir(root);
    fs::create_dir_all(&dir).expect("创建模型目录失败");
    fs::write(dir.join(name), content).expect("写模型文件失败");
}

/// 在 `<root>/.dev-team/architecture/models/` 下落一个非 UTF-8 文件（读盘失败面）。
fn write_broken_model(root: &Path, name: &str) {
    let dir = archi_models_dir(root);
    fs::create_dir_all(&dir).expect("创建模型目录失败");
    fs::write(dir.join(name), [0xff_u8, 0xfe, 0x00, 0x80]).expect("写坏文件失败");
}

/// 子串所在行号（1 起始），用于把「行号 = 表头行」写成可读断言。
fn line_at(source: &str, needle: &str) -> u32 {
    let index = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("源文本中找不到 {needle:?}"));
    index as u32 + 1
}

const CORE_SOURCE: &str = concat!(
    "# 核心包\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"warn\"\n",
);

const AUTH_SOURCE: &str = concat!(
    "[element.\"App.Auth.Token\"]\n",
    "kind = \"component\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth.Token\"\n",
    "to = \"App.Auth\"\n",
    "desc = \"令牌属于认证域\"\n",
);

// ---------------------------------------------------------------------------
// 正向：多文件聚合 + 回填
// ---------------------------------------------------------------------------

/// 正向：`01-core.toml` + `02-auth.toml` → 元素与关系齐全；合并序 = 文件名字典序
/// × 文件内 FQN 序（`01` 全在 `02` 前）；零诊断。
#[test]
fn 聚合多文件_元素关系齐全且按文件字典序合并() {
    let temp = TempDir::new("merge-two");
    write_model(temp.root(), "01-core.toml", CORE_SOURCE);
    write_model(temp.root(), "02-auth.toml", AUTH_SOURCE);

    let outcome = parse_dir(temp.root());
    assert!(
        outcome.diagnostics.is_empty(),
        "健康源零诊断: {:?}",
        outcome.diagnostics
    );
    let fqns: Vec<&str> = outcome
        .model
        .elements
        .iter()
        .map(|e| e.fqn.as_str())
        .collect();
    assert_eq!(
        fqns,
        vec!["App", "App.Auth", "App.Auth.Token"],
        "01 文件元素全在 02 文件之前（文件名字典序 × 文件内序）"
    );
    assert_eq!(outcome.model.relations.len(), 1);
    let relation = &outcome.model.relations[0];
    assert_eq!(relation.from, "App.Auth.Token");
    assert_eq!(relation.to, "App.Auth");
    assert_eq!(relation.description.as_deref(), Some("令牌属于认证域"));
}

/// 正向：`file` 回填为 repo 相对路径且 `/` 分隔；`line` 逐元素等于表头键行。
#[test]
fn 聚合回填文件相对路径与表头行号() {
    let temp = TempDir::new("backfill");
    write_model(temp.root(), "01-core.toml", CORE_SOURCE);
    write_model(temp.root(), "02-auth.toml", AUTH_SOURCE);

    let outcome = parse_dir(temp.root());
    let app = outcome.model.element("App").expect("App 在模型内");
    assert_eq!(
        app.file, ".dev-team/architecture/models/01-core.toml",
        "file 为 repo 相对路径且 / 分隔"
    );
    assert_eq!(
        app.line,
        line_at(CORE_SOURCE, "[element.\"App\"]"),
        "line 等于表头键行"
    );

    let token = outcome
        .model
        .element("App.Auth.Token")
        .expect("子元素在模型内");
    assert_eq!(
        token.file, ".dev-team/architecture/models/02-auth.toml",
        "跨文件回填指向各自文件"
    );
    assert_eq!(
        token.line,
        line_at(AUTH_SOURCE, "[element.\"App.Auth.Token\"]")
    );
    assert_eq!(
        outcome.model.relations[0].file, ".dev-team/architecture/models/02-auth.toml",
        "关系 file 亦回填"
    );
    assert_eq!(
        outcome.model.relations[0].line,
        line_at(AUTH_SOURCE, "from = \"App.Auth.Token\""),
        "关系 line 等于 from 键行"
    );
}

/// 正向：文件名非数字序（`a` / `B` / `10` / `2`）→ 聚合序按**字节**字典序
/// （`10` < `2` < `B` < `a`），且各文件 `file` 指向自身。
#[test]
fn 聚合序按字节字典序而非数字序() {
    let temp = TempDir::new("byte-order");
    for (name, fqn) in [
        ("a.toml", "App.Aaa"),
        ("B.toml", "App.Bbb"),
        ("10.toml", "App.Ten"),
        ("2.toml", "App.Two"),
    ] {
        write_model(
            temp.root(),
            name,
            &format!("[element.\"{fqn}\"]\nkind = \"module\"\n"),
        );
    }
    let outcome = parse_dir(temp.root());
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let fqns: Vec<&str> = outcome
        .model
        .elements
        .iter()
        .map(|e| e.fqn.as_str())
        .collect();
    assert_eq!(
        fqns,
        vec!["App.Ten", "App.Two", "App.Bbb", "App.Aaa"],
        "字节字典序：10 < 2 < B < a"
    );
    for name in ["a.toml", "B.toml", "10.toml", "2.toml"] {
        let element = outcome
            .model
            .elements
            .iter()
            .find(|element| element.file.ends_with(name))
            .unwrap_or_else(|| panic!("{name} 应有对应元素"));
        assert_eq!(
            element.file,
            format!(".dev-team/architecture/models/{name}"),
            "各文件 file 指向自身"
        );
    }
}

// ---------------------------------------------------------------------------
// 异常：跨文件重复 FQN + 读盘失败
// ---------------------------------------------------------------------------

/// 异常：跨文件重复 FQN → 一条 `duplicateElement`，`file` 指字典序靠后者、
/// `line` 为该文件声明行；元素数不因重复而增。
#[test]
fn 跨文件重复fqn诊断为一条且指向靠后者() {
    let temp = TempDir::new("dup-two");
    write_model(temp.root(), "01-core.toml", CORE_SOURCE);
    let billing = "[element.\"App.Auth\"]\nkind = \"module\"\n";
    write_model(temp.root(), "03-billing.toml", billing);

    let outcome = parse_dir(temp.root());
    assert_eq!(outcome.diagnostics.len(), 1, "恰一条重复诊断");
    let diagnostic = &outcome.diagnostics[0];
    assert_eq!(diagnostic.kind, DiagnosticKind::DuplicateElement);
    assert_eq!(
        diagnostic.file, ".dev-team/architecture/models/03-billing.toml",
        "file 指字典序靠后者"
    );
    assert_eq!(
        diagnostic.line,
        line_at(billing, "[element.\"App.Auth\"]"),
        "line 为该文件声明行"
    );
    assert!(
        diagnostic.message.contains("App.Auth"),
        "文案点名重复 FQN: {}",
        diagnostic.message
    );
    assert_eq!(
        outcome.model.elements.len(),
        2,
        "重复声明不使元素数增长（不静默保留两份）"
    );
}

/// 异常：跨文件同一 FQN 三处声明 → 诊断恰 2 条（后两者各一条），`file` 分别指向
/// 第二 / 第三个文件。
#[test]
fn 跨文件三处重复fqn产两条诊断() {
    let temp = TempDir::new("dup-three");
    for name in ["01-core.toml", "02-auth.toml", "03-billing.toml"] {
        write_model(
            temp.root(),
            name,
            "[element.\"App.Auth\"]\nkind = \"domain\"\n",
        );
    }
    let outcome = parse_dir(temp.root());
    assert_eq!(outcome.diagnostics.len(), 2, "后两者各一条，共 2 条");
    let files: Vec<&str> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.file.as_str())
        .collect();
    assert_eq!(
        files,
        vec![
            ".dev-team/architecture/models/02-auth.toml",
            ".dev-team/architecture/models/03-billing.toml"
        ],
        "file 分别指向第二 / 第三个文件（字典序）"
    );
    assert_eq!(outcome.model.elements.len(), 1, "元素只留一份");
}

/// 异常：单文件读盘失败（非 UTF-8 内容）→ 一条 `syntaxError`，`message` 前缀
/// 「模型文件读取失败」，`file` 回填；其余文件照常解析（单次列全、零 panic）。
#[test]
fn 读盘失败落_syntax_error_且其余文件照常解析() {
    let temp = TempDir::new("read-failure");
    write_model(temp.root(), "01-core.toml", CORE_SOURCE);
    write_broken_model(temp.root(), "02-auth.toml");

    let outcome = parse_dir(temp.root());
    assert_eq!(outcome.diagnostics.len(), 1, "恰一条读取失败诊断");
    let diagnostic = &outcome.diagnostics[0];
    assert_eq!(diagnostic.kind, DiagnosticKind::SyntaxError);
    assert!(
        diagnostic.message.starts_with("模型文件读取失败"),
        "message 前缀固定: {}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic.file, ".dev-team/architecture/models/02-auth.toml",
        "file 回填到失败文件"
    );
    assert_eq!(diagnostic.line, 0, "读取失败无位置（line 0）");
    assert!(diagnostic.column.is_none(), "读取失败无列");
    assert_eq!(
        outcome.model.elements.len(),
        2,
        "另一文件（01-core）照常解析，元素不丢"
    );
}

/// 异常：坏点一次列全——每个坏文件各一条诊断（单文件解析全或无，故诊断数 =
/// 坏文件数，不短路的机械判据）。
#[test]
fn 多个坏文件诊断数等于坏文件数() {
    let temp = TempDir::new("multi-bad");
    write_model(temp.root(), "01-core.toml", CORE_SOURCE);
    write_model(
        temp.root(),
        "02-missing-kind.toml",
        "[element.\"App.Auth\"]\ngate = \"strict\"\n",
    );
    write_model(
        temp.root(),
        "03-invalid-gate.toml",
        "[element.\"App.Billing\"]\nkind = \"module\"\ngate = \"stirct\"\n",
    );
    write_model(
        temp.root(),
        "04-syntax.toml",
        "[element.\"App.Legacy]\nkind = \"component\"\n",
    );

    let outcome = parse_dir(temp.root());
    assert_eq!(outcome.diagnostics.len(), 3, "三个坏文件各一条，不短路");
    let kinds: Vec<DiagnosticKind> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            DiagnosticKind::MissingField,
            DiagnosticKind::InvalidValue,
            DiagnosticKind::SyntaxError
        ],
        "逐文件分类在案（按文件字典序）"
    );
    for diagnostic in &outcome.diagnostics {
        assert!(
            !diagnostic.file.is_empty(),
            "每条诊断均带文件身份: {diagnostic:?}"
        );
    }
    assert_eq!(outcome.model.elements.len(), 2, "健康文件元素仍在");
}

// ---------------------------------------------------------------------------
// 边界：目录缺席 / 空目录 / 收面判据 / 规模 / 文件名
// ---------------------------------------------------------------------------

/// 边界：`models/` 目录不存在 → 空模型（零元素零关系零诊断），不 `Err`。
#[test]
fn 模型目录不存在返回空模型零诊断() {
    let temp = TempDir::new("no-dir");
    let outcome = parse_dir(temp.root());
    assert!(outcome.diagnostics.is_empty(), "冷启动不报错");
    assert!(outcome.model.elements.is_empty());
    assert!(outcome.model.relations.is_empty());
}

/// 边界：空目录 / 仅空文件 / 仅注释文件 → 空模型零诊断。
#[test]
fn 空目录与空文件与仅注释文件均为空模型() {
    let empty_dir = TempDir::new("empty-dir");
    fs::create_dir_all(archi_models_dir(empty_dir.root())).expect("建空目录");
    let outcome = parse_dir(empty_dir.root());
    assert!(outcome.diagnostics.is_empty());
    assert!(outcome.model.elements.is_empty());

    let blank = TempDir::new("blank-file");
    write_model(blank.root(), "01-core.toml", "");
    let outcome = parse_dir(blank.root());
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert!(outcome.model.elements.is_empty());

    let comments = TempDir::new("comment-file");
    write_model(comments.root(), "01-core.toml", "# 只有注释\n# 第二行\n");
    let outcome = parse_dir(comments.root());
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert!(outcome.model.elements.is_empty());
}

/// 边界：非 `*.toml` 不参与聚合——`README.md` / `notes.txt` / `x.toml.bak` /
/// 子目录内 `nested/y.toml` 均不计入。
#[test]
fn 非toml与子目录文件不参与聚合() {
    let temp = TempDir::new("suffix");
    write_model(temp.root(), "01-core.toml", CORE_SOURCE);
    write_model(
        temp.root(),
        "README.md",
        "[element.\"App.Md\"]\nkind = \"module\"\n",
    );
    write_model(
        temp.root(),
        "notes.txt",
        "[element.\"App.Txt\"]\nkind = \"module\"\n",
    );
    write_model(
        temp.root(),
        "x.toml.bak",
        "[element.\"App.Bak\"]\nkind = \"module\"\n",
    );
    let nested = archi_models_dir(temp.root()).join("nested");
    fs::create_dir_all(&nested).expect("建子目录");
    fs::write(
        nested.join("y.toml"),
        "[element.\"App.Nested\"]\nkind = \"module\"\n",
    )
    .expect("写子目录文件");

    let outcome = parse_dir(temp.root());
    assert!(outcome.diagnostics.is_empty());
    let fqns: Vec<&str> = outcome
        .model
        .elements
        .iter()
        .map(|e| e.fqn.as_str())
        .collect();
    assert_eq!(fqns, vec!["App", "App.Auth"], "只由顶层 *.toml 决定元素面");
}

/// 边界：规模面——100 个文件 × 各 5 元素 / 单文件 500 元素 / 单文件 500 关系 →
/// 计数正确、`line` 正确、零 panic。
#[test]
fn 规模面百文件与单文件五百元素五百关系() {
    let many_files = TempDir::new("many-files");
    for file in 0..100 {
        let mut source = String::new();
        for element in 0..5 {
            source.push_str(&format!(
                "[element.\"App.File{file:03}.Mod{element}\"]\nkind = \"module\"\n\n"
            ));
        }
        write_model(many_files.root(), &format!("{file:03}.toml"), &source);
    }
    let outcome = parse_dir(many_files.root());
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(outcome.model.elements.len(), 500, "100 × 5 = 500 元素");

    let single = TempDir::new("single-file");
    let mut source = String::new();
    for element in 0..500 {
        source.push_str(&format!(
            "[element.\"App.Mod{element:03}\"]\nkind = \"module\"\n\n"
        ));
    }
    for relation in 0..500 {
        source.push_str(&format!(
            "[[relation]]\nfrom = \"App.Mod{relation:03}\"\nto = \"App.Mod{:03}\"\n\n",
            (relation + 1) % 500
        ));
    }
    write_model(single.root(), "01-big.toml", &source);
    let outcome = parse_dir(single.root());
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(outcome.model.elements.len(), 500);
    assert_eq!(outcome.model.relations.len(), 500);
    let probe = outcome
        .model
        .element("App.Mod250")
        .expect("探针元素在模型内");
    assert_eq!(
        probe.line,
        line_at(&source, "[element.\"App.Mod250\"]"),
        "规模面行号不漂移"
    );
}

/// 边界：文件名含中文 / emoji / 空格 → 不 panic；`file` 逐字保留（含 UTF-8）。
#[test]
fn 特殊字符文件名逐字保留且不panic() {
    let temp = TempDir::new("exotic-names");
    for name in ["架构 模型.toml", "🚀模型.toml", "with space.toml"] {
        write_model(
            temp.root(),
            name,
            "[element.\"App.Module\"]\nkind = \"module\"\n",
        );
    }
    // 三个文件声明同一 FQN → 后两者各一条重复诊断；file 逐字保留
    let outcome = parse_dir(temp.root());
    assert_eq!(outcome.model.elements.len(), 1);
    assert_eq!(outcome.diagnostics.len(), 2);
    for diagnostic in &outcome.diagnostics {
        assert!(
            diagnostic.file.starts_with(".dev-team/architecture/models/"),
            "目录段由派生函数给出: {}",
            diagnostic.file
        );
        let name = diagnostic
            .file
            .trim_start_matches(".dev-team/architecture/models/");
        assert!(
            ["架构 模型.toml", "🚀模型.toml", "with space.toml"].contains(&name),
            "文件名逐字保留（含 UTF-8 / 空格）: {name}"
        );
    }
}

// ---------------------------------------------------------------------------
// `SourceParser::parse` 单文件契约
// ---------------------------------------------------------------------------

/// 正向：直接以单文件源文本调用（`TomlParser`）→ `ParseOutcome.file` 为空串
/// （聚合层回填前契约），`line` 相对该源文本。
#[test]
fn 单文件解析契约_file留空且行号相对源文本() {
    let outcome = TomlParser::parse(CORE_SOURCE);
    assert!(outcome.diagnostics.is_empty());
    for element in &outcome.model.elements {
        assert_eq!(element.file, "", "单文件契约：file 留空待回填");
    }
    let auth = outcome
        .model
        .element("App.Auth")
        .expect("App.Auth 在模型内");
    assert_eq!(
        auth.line,
        line_at(CORE_SOURCE, "[element.\"App.Auth\"]"),
        "行号相对该源文本"
    );
}

/// 边界：空字符串 → 空模型零诊断；超长单文件（>100 KB）→ 不 panic 且诊断数与
/// 坏点数一致。
#[test]
fn 单文件解析空串与超长源不panic() {
    let outcome = TomlParser::parse("");
    assert!(outcome.diagnostics.is_empty());
    assert!(outcome.model.elements.is_empty());

    // 超长健康源（>100 KB）：约 4000 个元素
    let mut source = String::new();
    for element in 0..4000 {
        source.push_str(&format!(
            "[element.\"App.Mod{element:04}\"]\nkind = \"module\"\nsummary = \"摘要{element}\"\n\n"
        ));
    }
    assert!(source.len() > 100_000, "构造面确为 >100 KB");
    let outcome = TomlParser::parse(&source);
    assert!(outcome.diagnostics.is_empty(), "超长健康源零诊断");
    assert_eq!(outcome.model.elements.len(), 4000);

    // 超长坏源：唯一的坏点仍只产一条诊断
    let mut broken = source.clone();
    broken.push_str("[element.\"App.Broken]\nkind = \"module\"\n");
    let outcome = TomlParser::parse(&broken);
    assert_eq!(outcome.diagnostics.len(), 1, "坏点数与诊断数一致");
    assert_eq!(outcome.diagnostics[0].kind, DiagnosticKind::SyntaxError);
}
