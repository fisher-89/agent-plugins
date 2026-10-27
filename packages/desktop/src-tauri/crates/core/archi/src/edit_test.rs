//! `edit` 的单元测试：`set_gate` 纯函数的改 AST → 渲染 → 自证三道，以及
//! `plan_set_gate` 的「定位 + 读源 + 零写盘」产物语义。
//!
//! 零 mock：真实 `toml_edit`（注释与排版保留即被测能力）与真实 `TomlParser`
//! 读回校验（自证判据与解析层同源）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_models_dir;

use crate::dto::GateTier;
use crate::parse::{SourceParser, TomlParser};

use crate::edit::{plan_set_gate, set_gate};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-edit-test-{}-{}-{}",
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

    /// 落一个模型文件并返回其绝对路径。
    fn write_model(&self, name: &str, content: &str) -> PathBuf {
        let dir = archi_models_dir(&self.0);
        fs::create_dir_all(&dir).expect("创建模型目录失败");
        let path = dir.join(name);
        fs::write(&path, content).expect("写模型文件失败");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 读回模型里某元素的档位（自证判据与解析层同源）。
fn gate_of(source: &str, fqn: &str) -> GateTier {
    let outcome = TomlParser::parse(source);
    assert!(
        outcome.diagnostics.is_empty(),
        "改档产物应可解析: {:?}",
        outcome.diagnostics
    );
    outcome
        .model
        .element(fqn)
        .unwrap_or_else(|| panic!("改档产物应含 {fqn}"))
        .gate
}

const SOURCE: &str = concat!(
    "# 核心包（注释须逐字保留）\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "# 计费域\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "gate = \"off\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"warn\"\n",
);

// ---------------------------------------------------------------------------
// `set_gate` 正向
// ---------------------------------------------------------------------------

/// 正向：含注释 / 空行 / 多元素的源文本改 `App.Billing` 的档位 → 目标行为
/// `gate = "strict"`，其余行逐字保留，读回档位为新值且零诊断。
#[test]
fn 改档只动目标行且其余逐字保留() {
    let rendered = set_gate(SOURCE, "App.Billing", GateTier::Strict).expect("改档应成功");

    let before: Vec<&str> = SOURCE.lines().collect();
    let after: Vec<&str> = rendered.lines().collect();
    assert_eq!(before.len(), after.len(), "行数不变（就地改值，不增删行）");
    let mut changed: Vec<usize> = Vec::new();
    for (index, (old, new)) in before.iter().zip(after.iter()).enumerate() {
        if old != new {
            changed.push(index);
        }
    }
    assert_eq!(changed.len(), 1, "只有一行不同（按行 diff）: {changed:?}");
    assert!(
        after[changed[0]].contains("gate = \"strict\""),
        "目标行为新档位: {}",
        after[changed[0]]
    );
    assert_eq!(
        gate_of(&rendered, "App.Billing"),
        GateTier::Strict,
        "读回档位为新值"
    );
    assert_eq!(
        gate_of(&rendered, "App.Auth"),
        GateTier::Warn,
        "其他元素档位不变"
    );
    assert!(
        rendered.contains("# 核心包（注释须逐字保留）") && rendered.contains("# 计费域"),
        "注释逐字保留"
    );
}

/// 正向：目标元素无显式 `gate` 行 → 插入该行（缺省显式化）；再次调用改回 `off` →
/// 行仍在且值为 `off`（幂等可逆）。
#[test]
fn 无显式gate行插入并可逆() {
    let source = concat!(
        "[element.\"App\"]\n",
        "kind = \"package\"\n",
        "\n",
        "[element.\"App.Billing\"]\n",
        "kind = \"module\"\n",
        "summary = \"计费\"\n",
    );
    let inserted = set_gate(source, "App.Billing", GateTier::Strict).expect("插入应成功");
    assert!(
        inserted.contains("gate = \"strict\""),
        "插入显式档位行: {inserted}"
    );
    assert_eq!(gate_of(&inserted, "App.Billing"), GateTier::Strict);
    assert!(inserted.contains("summary = \"计费\""), "其余字段逐字保留");

    let reverted = set_gate(&inserted, "App.Billing", GateTier::Off).expect("改回应成功");
    assert!(reverted.contains("gate = \"off\""), "行仍在且值回退");
    assert_eq!(gate_of(&reverted, "App.Billing"), GateTier::Off);
}

/// 正向：三档逐值往返（`GateTier` 全 3 值覆盖）。
#[test]
fn 三档逐值往返读回一致() {
    for gate in GateTier::ALL {
        let rendered = set_gate(SOURCE, "App.Billing", gate)
            .unwrap_or_else(|error| panic!("改到 {} 应成功: {error}", gate.as_str()));
        assert_eq!(
            gate_of(&rendered, "App.Billing"),
            gate,
            "读回档位为 {}",
            gate.as_str()
        );
        assert!(
            rendered.contains(&format!("gate = \"{}\"", gate.as_str())),
            "渲染文本含线值字面量"
        );
    }
}

// ---------------------------------------------------------------------------
// `set_gate` 异常：零文本产出
// ---------------------------------------------------------------------------

/// 异常：元素缺席 → `Err` 且消息附现有 FQN 清单（零文本产出）。
#[test]
fn 元素缺席返回err并附现有元素清单() {
    let error = set_gate(SOURCE, "App.Missing", GateTier::Strict).expect_err("元素缺席应 Err");
    assert!(error.contains("元素不存在"), "文案点名缺席: {error}");
    assert!(error.contains("App.Missing"), "文案含目标 FQN: {error}");
    for existing in ["App", "App.Billing", "App.Auth"] {
        assert!(
            error.contains(existing),
            "文案附现有元素 {existing}: {error}"
        );
    }
}

/// 异常：注入式 FQN（含 `"` / 换行 / `]`）→ 一律 `Err`，绝不产出待落盘文本
/// （自证纪律）。
#[test]
fn 注入式fqn一律err零文本产出() {
    for fqn in ["App\"]", "App\n.Auth", "App]", "App\"", ""] {
        let result = set_gate(SOURCE, fqn, GateTier::Strict);
        let error = result
            .err()
            .unwrap_or_else(|| panic!("注入形态 {fqn:?} 不得产出待落盘文本（自证纪律）"));
        assert!(!error.is_empty(), "错误文案非空: {fqn:?}");
    }
}

/// 异常：源文本本身不可解析 / 无 `element` 表 → `Err`（零文本产出）。
#[test]
fn 坏源与无element表均err() {
    let broken = set_gate(
        "[element.\"App]\nkind = \"package\"\n",
        "App",
        GateTier::Strict,
    );
    assert!(broken.is_err(), "不可解析的源应 Err");

    let no_table = set_gate("", "App", GateTier::Strict);
    assert!(no_table.is_err(), "无 element 表应 Err");
    assert!(
        no_table.unwrap_err().contains("element"),
        "文案点名元素表缺失"
    );

    let only_relation = set_gate(
        "[[relation]]\nfrom = \"A\"\nto = \"B\"\n",
        "A",
        GateTier::Warn,
    );
    assert!(only_relation.is_err(), "只有关系表时无从定位元素");
}

// ---------------------------------------------------------------------------
// `set_gate` 边界：排版 / str / 规模
// ---------------------------------------------------------------------------

/// 边界：排版保留面——元素间空行 / 缩进 / 行内注释 / 无尾换行 / CRLF 换行 → 改值
/// 后各自保留（CRLF 只断言可解析与值正确，不断言换行字节；跨平台差异不入判据）。
#[test]
fn 排版保留面五形态() {
    // 空行 + 缩进 + 其余行行内注释
    let source = concat!(
        "[element.\"App\"]\n",
        "  kind   =   \"package\"   # 缩进与行内注释\n",
        "\n",
        "\n",
        "[element.\"App.Billing\"]\n",
        "kind = \"module\"\n",
        "gate = \"off\"\n",
    );
    let rendered = set_gate(source, "App.Billing", GateTier::Strict).expect("改档成功");
    assert!(
        rendered.contains("  kind   =   \"package\"   # 缩进与行内注释"),
        "缩进与行内注释逐字保留: {rendered}"
    );
    assert_eq!(rendered.matches('\n').count(), source.matches('\n').count());
    assert_eq!(gate_of(&rendered, "App.Billing"), GateTier::Strict);

    // 无尾换行
    let no_trailing = "[element.\"App\"]\nkind = \"package\"";
    let rendered = set_gate(no_trailing, "App", GateTier::Warn).expect("改档成功");
    assert_eq!(gate_of(&rendered, "App"), GateTier::Warn);

    // CRLF
    let crlf = "[element.\"App\"]\r\nkind = \"package\"\r\ngate = \"off\"\r\n";
    let rendered = set_gate(crlf, "App", GateTier::Strict).expect("CRLF 源改档成功");
    assert_eq!(gate_of(&rendered, "App"), GateTier::Strict, "CRLF 下值正确");
}

/// 边界：`str` 面——`fqn` 为 `""` / 超长 / emoji 元素名：合法键形下正常改档，
/// 非法 / 缺席形态 `Err`，均不 panic。
#[test]
fn 特殊字符fqn改档与err两臂() {
    // 空串 → 缺席 Err
    assert!(
        set_gate(SOURCE, "", GateTier::Strict).is_err(),
        "空 FQN 应 Err"
    );

    // 超长 FQN（作为合法键存在）→ 正常改档
    let long = "X".repeat(1200);
    let source = format!("[element.\"{long}\"]\nkind = \"module\"\ngate = \"off\"\n");
    let rendered = set_gate(&source, &long, GateTier::Warn).expect("超长 FQN 改档成功");
    assert_eq!(gate_of(&rendered, &long), GateTier::Warn);

    // emoji 元素名（引号键内合法）→ 正常改档
    let source = "[element.\"App.🚀Auth\"]\nkind = \"module\"\ngate = \"off\"\n";
    let rendered = set_gate(source, "App.🚀Auth", GateTier::Strict).expect("emoji FQN 改档成功");
    assert_eq!(gate_of(&rendered, "App.🚀Auth"), GateTier::Strict);

    // 超长 FQN 但模型中不存在 → Err
    assert!(
        set_gate(SOURCE, &long, GateTier::Strict).is_err(),
        "不存在的超长 FQN 应 Err"
    );
}

/// 边界：规模面——500 元素文本仅目标行变化（其余行逐字相等）。
#[test]
fn 规模面五百元素仅目标行变化() {
    let mut source = String::new();
    for index in 0..500 {
        source.push_str(&format!(
            "[element.\"App.Mod{index:03}\"]\nkind = \"module\"\ngate = \"off\"\n\n"
        ));
    }
    let rendered = set_gate(&source, "App.Mod250", GateTier::Strict).expect("规模面改档成功");
    let before: Vec<&str> = source.lines().collect();
    let after: Vec<&str> = rendered.lines().collect();
    assert_eq!(before.len(), after.len(), "行数不变");
    let diffs: usize = before
        .iter()
        .zip(after.iter())
        .filter(|(old, new)| old != new)
        .count();
    assert_eq!(diffs, 1, "只有目标行变化");
    assert_eq!(gate_of(&rendered, "App.Mod250"), GateTier::Strict);
    assert_eq!(gate_of(&rendered, "App.Mod249"), GateTier::Off, "邻行不变");
}

// ---------------------------------------------------------------------------
// `plan_set_gate`
// ---------------------------------------------------------------------------

/// 正向：两文件模型（目标元素在 `02-auth.toml`）→ `GateWrite.file` 为该文件绝对
/// 路径、`content` 含新档位；调用后两文件字节全等原文（零写盘）。
#[test]
fn 定位目标文件并零写盘() {
    let temp = TempDir::new("plan");
    let core_path = temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let auth_path = temp.write_model(
        "02-auth.toml",
        "[element.\"App.Auth\"]\nkind = \"domain\"\ngate = \"off\"\n",
    );
    let core_before = fs::read(&core_path).expect("读 core");
    let auth_before = fs::read(&auth_path).expect("读 auth");

    let plan = plan_set_gate(temp.root(), "App.Auth", GateTier::Strict).expect("定位成功");
    assert_eq!(plan.file, auth_path, "定位到声明所在文件的绝对路径");
    assert!(
        plan.content.contains("gate = \"strict\""),
        "待落盘全文含新档位"
    );
    assert_eq!(
        fs::read(&core_path).expect("读 core"),
        core_before,
        "非目标文件零写入"
    );
    assert_eq!(
        fs::read(&auth_path).expect("读 auth"),
        auth_before,
        "目标文件零写入（plan 不落盘）"
    );
}

/// 异常：元素缺席 / `models/` 目录缺席 / 模型结构错 / 坏文件读取 → 一律 `Err`
/// 不 panic。
#[test]
fn plan异常三形态均err() {
    let temp = TempDir::new("plan-err");
    temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let absent = plan_set_gate(temp.root(), "App.Missing", GateTier::Strict);
    assert!(absent.is_err(), "元素缺席应 Err");
    assert!(absent.unwrap_err().contains("元素不存在"), "文案点名缺席");

    let empty_root = TempDir::new("plan-empty-root");
    assert!(
        plan_set_gate(empty_root.root(), "App", GateTier::Strict).is_err(),
        "模型目录缺席应 Err"
    );

    let broken = TempDir::new("plan-broken");
    broken.write_model("01-core.toml", "[element.\"App\"]\ngate = \"strict\"\n");
    let error = plan_set_gate(broken.root(), "App", GateTier::Warn).expect_err("结构错模型应 Err");
    assert!(
        error.contains("结构诊断") || error.contains("拒绝改档"),
        "文案说明模型结构坏: {error}"
    );

    // 读盘失败（非 UTF-8 文件）→ 结构诊断路径亦 Err
    let unreadable = TempDir::new("plan-unreadable");
    let dir = archi_models_dir(unreadable.root());
    fs::create_dir_all(&dir).expect("建模型目录");
    fs::write(dir.join("01-core.toml"), [0xff_u8, 0xfe, 0x80]).expect("写坏文件");
    assert!(
        plan_set_gate(unreadable.root(), "App", GateTier::Strict).is_err(),
        "读盘失败应 Err 不 panic"
    );
}

/// 边界：空白 `root` / 不存在的 `root` → `Err`；`fqn` 含 emoji → 合法键形成功 /
/// 否则 `Err`（自证）。
#[test]
fn plan边界_空白root与不存在root与emoji() {
    assert!(
        plan_set_gate(Path::new(""), "App", GateTier::Strict).is_err(),
        "空白 root 应 Err"
    );
    assert!(
        plan_set_gate(
            Path::new("/nonexistent/archi/root"),
            "App",
            GateTier::Strict
        )
        .is_err(),
        "不存在 root 应 Err"
    );

    let temp = TempDir::new("plan-emoji");
    temp.write_model(
        "01-core.toml",
        "[element.\"App.🚀Auth\"]\nkind = \"module\"\ngate = \"off\"\n",
    );
    let plan =
        plan_set_gate(temp.root(), "App.🚀Auth", GateTier::Warn).expect("emoji 元素改档成功");
    assert!(plan.content.contains("gate = \"warn\""));
    assert!(
        plan_set_gate(temp.root(), "App.不存在", GateTier::Warn).is_err(),
        "缺席元素仍 Err（不 panic）"
    );
}
