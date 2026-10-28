//! `drift::config` 的单元测试：`<archi_dir>/config.json` 缺失即空配置、未知键
//! 拒绝、`reason` 必填非空白、豁免粒度封闭为元素对（禁 glob / 禁不存在的 FQN）、
//! `exclude` 条目形态，以及「经 `archi_config_path` 唯一合法定位」的证伪面。
//!
//! 零 mock：真实临时目录 + 真实 `foundation::layout::archi_config_path` + 真实
//! `serde_json`（定位与解析即被测对象）。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::archi_config_path;

use super::config::{load_config, CheckConfig};

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-config-test-{}-{}-{}",
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

    /// 落一个 repo 相对文件（自动建父目录）。
    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }

    /// 落架构域看护配置（唯一合法位置）。
    fn write_config(&self, content: &str) {
        let path = archi_config_path(&self.0);
        fs::create_dir_all(path.parent().expect("父目录")).expect("创建架构域目录失败");
        fs::write(&path, content).expect("写配置失败");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 模型内元素 FQN 全集（`check` 侧由 `dto::graph_view` 提供，本测试直构）。
fn known() -> BTreeSet<String> {
    ["App", "App.Auth", "App.Billing"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// 读配置（`known` 固定）。
fn load(root: &Path) -> Result<CheckConfig, String> {
    load_config(root, &known())
}

// ---------------------------------------------------------------------------
// 边界：缺失 / 空对象
// ---------------------------------------------------------------------------

/// 边界（AC-9）：无 `<archi_dir>/config.json` → 空配置（两字段皆空，零报错）。
#[test]
fn 文件缺失即空配置() {
    let temp = TempDir::new("missing");
    let config = load(temp.root()).expect("缺失即空配置，MUST NOT 报错");
    assert!(config.ignore.is_empty(), "ignore 空");
    assert!(config.exclude.is_empty(), "exclude 空");
    assert_eq!(config, CheckConfig::default(), "等于 Default 形态");
}

/// 边界：空对象 `{}` → `Ok` 两字段皆空；空文件 `""` 不是合法 JSON → 显式 `Err`
/// （MUST NOT 静默当空配置）。
#[test]
fn 空对象与空文件() {
    let empty_object = TempDir::new("empty-object");
    empty_object.write_config("{}");
    let config = load(empty_object.root()).expect("空对象合法");
    assert_eq!(config, CheckConfig::default(), "空对象即空配置");

    let empty_file = TempDir::new("empty-file");
    empty_file.write_config("");
    let error = load(empty_file.root()).expect_err("空文件不是合法 JSON");
    assert!(!error.is_empty(), "错误文案非空：{error}");
}

// ---------------------------------------------------------------------------
// 正向：两键齐备 / 单键缺席
// ---------------------------------------------------------------------------

/// 正向：`ignore` + `exclude` 两键齐备 → 逐值解析（`reason` 逐字、`exclude` 保序）。
#[test]
fn 两键齐备逐值解析() {
    let temp = TempDir::new("both-keys");
    temp.write_config(
        "{\n  \"ignore\": [{ \"from\": \"App\", \"to\": \"App.Auth\", \"reason\": \"迁移期\" }],\n  \"exclude\": [\"src/legacy/**\", \"src/gen/**\"]\n}\n",
    );
    let config = load(temp.root()).expect("两键齐备合法");
    assert_eq!(config.ignore.len(), 1, "一条豁免");
    assert_eq!(config.ignore[0].from, "App", "from 逐字");
    assert_eq!(config.ignore[0].to, "App.Auth", "to 逐字");
    assert_eq!(config.ignore[0].reason, "迁移期", "reason 逐字");
    assert_eq!(
        config.exclude,
        vec!["src/legacy/**".to_owned(), "src/gen/**".to_owned()],
        "exclude 顺序保留"
    );
}

/// 边界：只有 `ignore` / 只有 `exclude` → 另一字段为空，`Ok`。
#[test]
fn 单键缺席() {
    let only_ignore = TempDir::new("only-ignore");
    only_ignore.write_config(
        "{\n  \"ignore\": [{ \"from\": \"App\", \"to\": \"App.Auth\", \"reason\": \"迁移期\" }]\n}\n",
    );
    let config = load(only_ignore.root()).expect("单键合法");
    assert_eq!(config.ignore.len(), 1, "ignore 解析");
    assert!(config.exclude.is_empty(), "exclude 缺省为空");

    let only_exclude = TempDir::new("only-exclude");
    only_exclude.write_config("{\n  \"exclude\": [\"src/legacy/**\"]\n}\n");
    let config = load(only_exclude.root()).expect("单键合法");
    assert!(config.ignore.is_empty(), "ignore 缺省为空");
    assert_eq!(config.exclude.len(), 1, "exclude 解析");
}

// ---------------------------------------------------------------------------
// 异常：未知键 / 必填面 / 粒度封闭
// ---------------------------------------------------------------------------

/// 异常（AC-9）：顶层多余键 → `Err`，文案含 serde 的 `unknown field` 字样。
#[test]
fn 未知键拒绝() {
    for content in ["{ \"rules\": [] }", "{ \"archi\": {} }"] {
        let temp = TempDir::new("unknown-key");
        temp.write_config(content);
        let error = load(temp.root()).unwrap_err();
        assert!(
            error.contains("unknown field"),
            "deny_unknown_fields 原文应在文案内：{error}"
        );
    }
}

/// 异常（AC-6）：`to` 缺失 → `Err`（元素对不完整，禁单边形态）。
#[test]
fn to缺失显式报错() {
    let temp = TempDir::new("missing-to");
    temp.write_config("{ \"ignore\": [{ \"from\": \"App\", \"reason\": \"迁移期\" }] }");
    let error = load(temp.root()).unwrap_err();
    assert!(error.contains('`'), "文案指明缺 `to` 条目：{error}");
    assert!(error.contains("to"), "文案含字段名：{error}");
}

/// 异常（AC-6）：`reason` 缺失或 `trim` 后为空 → `Err`（豁免必须带理由）。
#[test]
fn reason缺失或空白显式报错() {
    let missing = TempDir::new("missing-reason");
    missing.write_config("{ \"ignore\": [{ \"from\": \"App\", \"to\": \"App.Auth\" }] }");
    let error = load(missing.root()).unwrap_err();
    assert!(error.contains("reason"), "文案指明 reason：{error}");

    for reason in ["\"\"", "\"   \"", "\"\\n\\t\""] {
        let temp = TempDir::new("blank-reason");
        temp.write_config(&format!(
            "{{ \"ignore\": [{{ \"from\": \"App\", \"to\": \"App.Auth\", \"reason\": {reason} }}] }}"
        ));
        assert!(load(temp.root()).is_err(), "空白 reason {reason} 必须 Err");
    }
}

/// 异常（AC-6）：`from` / `to` 不在模型内（含 glob 形态）→ `Err`，文案指明该 FQN。
#[test]
#[allow(non_snake_case)]
fn 非模型FQN显式报错() {
    let deleted = TempDir::new("deleted-fqn");
    deleted.write_config(
        "{ \"ignore\": [{ \"from\": \"App.Deleted\", \"to\": \"App.Auth\", \"reason\": \"x\" }] }",
    );
    let error = load(deleted.root()).unwrap_err();
    assert!(error.contains("App.Deleted"), "文案指明该 FQN：{error}");

    for (from, to) in [("App", "src/**"), ("App.*", "App.Auth"), ("App", "**")] {
        let temp = TempDir::new("glob-fqn");
        temp.write_config(&format!(
            "{{ \"ignore\": [{{ \"from\": \"{from}\", \"to\": \"{to}\", \"reason\": \"x\" }}] }}"
        ));
        let error = load(temp.root()).unwrap_err();
        assert!(
            error.contains("元素对") || error.contains(from) || error.contains(to),
            "glob 形态 {from} → {to} 必须被拒：{error}"
        );
    }
}

/// 异常（AC-6）：`exclude` 条目无通配符 → `Err`（目录须写 `src/legacy/**`）。
#[test]
fn exclude无通配符显式报错() {
    for entry in ["src/legacy", "", "src/"] {
        let temp = TempDir::new("exclude-no-glob");
        temp.write_config(&format!("{{ \"exclude\": [\"{entry}\"] }}"));
        assert!(
            load(temp.root()).is_err(),
            "无通配符条目 {entry:?} 必须 Err"
        );
    }
}

/// 边界：`exclude` 语法非法条目**本层放行**（形态关只看空白 / 通配符），非法
/// glob 即 `Err` 的直证挂 `scan::exclude::ExcludeSet::build`；对照形态 `src/{a`
/// 不含 `*` / `?` / `[` → 按「无通配符」`Err`（同一节内两方向都钉住，防漏判）。
#[test]
fn exclude语法非法条目本层放行且形态拒对照() {
    // 三形态：2 放行（含 `[` 且非空白 → 过形态关）+ 1 形态拒（`{` 不算通配符）
    for entry in ["src/[legacy", "src/a[bc"] {
        let temp = TempDir::new("exclude-glob-form");
        temp.write_config(&format!("{{ \"exclude\": [\"{entry}\"] }}"));
        let config = load(temp.root())
            .unwrap_or_else(|error| panic!("{entry} 属 glob 形态（含 `[`），本层应放行：{error}"));
        assert_eq!(
            config.exclude,
            vec![entry.to_owned()],
            "条目原文逐字保序（本层不引 globset 编译，避免第二份合法性判定）"
        );
    }

    let brace = TempDir::new("exclude-brace-form");
    brace.write_config("{ \"exclude\": [\"src/{a\"] }");
    let error = load(brace.root()).expect_err("不含 * / ? / [ 的条目按无通配符拒");
    assert!(
        error.contains("不是文件级 glob") && error.contains("src/{a"),
        "文案指明条目原文：{error}"
    );

    // 合法条目零回归：真 glob 形态照常放行（保序、不去重）
    let legal = TempDir::new("exclude-legal-form");
    legal.write_config("{ \"exclude\": [\"src/legacy/**\", \"src/a?b\"] }");
    let config = load(legal.root()).expect("合法 glob 应放行");
    assert_eq!(
        config.exclude,
        vec!["src/legacy/**".to_owned(), "src/a?b".to_owned()],
        "顺序保留、零去重"
    );
}

// ---------------------------------------------------------------------------
// 异常：JSON 形态与条目形状
// ---------------------------------------------------------------------------

/// 异常：非法 JSON / 顶层数组 / 顶层字符串 / `null` → 全 `Err` 且不 panic。
#[test]
fn json语法与顶层形态报错() {
    for content in ["{ \"ignore\": []", "\"x\"", "null"] {
        let temp = TempDir::new("bad-json");
        temp.write_config(content);
        let error = load(temp.root())
            .err()
            .unwrap_or_else(|| panic!("非法形态 {content} 必须 Err"));
        assert!(!error.is_empty(), "错误文案非空：{content}");
    }

    // 空序列形态：serde 按字段序 + 两字段 `#[serde(default)]` 落为空配置（零报错），
    // 与 `{}` 同解——「顶层数组」不是可判错误面，故单列断言。
    let sequence = TempDir::new("sequence-top");
    sequence.write_config("[]");
    assert_eq!(
        load(sequence.root()).expect("空序列落默认两字段"),
        CheckConfig::default(),
        "空序列形态与空对象同解"
    );
}

/// 异常：条目形状违例（`ignore` 为对象 / `exclude` 为字符串 / `ignore[]` 元素为
/// 字符串）→ 全 `Err`（类型面封闭）。
#[test]
fn 条目形状违例报错() {
    for content in [
        "{ \"ignore\": {} }",
        "{ \"exclude\": \"src/legacy/**\" }",
        "{ \"ignore\": [\"App\"] }",
    ] {
        let temp = TempDir::new("bad-shape");
        temp.write_config(content);
        assert!(load(temp.root()).is_err(), "形状违例必须 Err：{content}");
    }
}

// ---------------------------------------------------------------------------
// 边界：空数组 / 超大列表 / 特殊字符
// ---------------------------------------------------------------------------

/// 边界：空数组两键 → `Ok`；`exclude: [""]` → `Err`。
#[test]
fn 空数组与空串条目() {
    let empty = TempDir::new("empty-arrays");
    empty.write_config("{ \"ignore\": [], \"exclude\": [] }");
    let config = load(empty.root()).expect("空数组合法");
    assert!(
        config.ignore.is_empty() && config.exclude.is_empty(),
        "两字段空"
    );

    let blank = TempDir::new("blank-entry");
    blank.write_config("{ \"exclude\": [\"\"] }");
    assert!(load(blank.root()).is_err(), "空串条目无通配符 ⇒ Err");
}

/// 边界：100 条合法 `ignore` + 100 条合法 `exclude` → `Ok` 且逐条保序（不截断、
/// 不去重）。
#[test]
fn 超大列表保序() {
    let temp = TempDir::new("large");
    let mut ignore_entries: Vec<String> = Vec::new();
    for index in 0..100 {
        ignore_entries.push(format!(
            "{{ \"from\": \"App\", \"to\": \"App.Auth\", \"reason\": \"豁免 {index}\" }}"
        ));
    }
    let mut exclude_entries: Vec<String> = Vec::new();
    for index in 0..100 {
        exclude_entries.push(format!("\"src/gen{index}/**\""));
    }
    temp.write_config(&format!(
        "{{ \"ignore\": [{}], \"exclude\": [{}] }}",
        ignore_entries.join(", "),
        exclude_entries.join(", ")
    ));
    let config = load(temp.root()).expect("100 + 100 条合法");
    assert_eq!(config.ignore.len(), 100, "ignore 不截断");
    assert_eq!(config.exclude.len(), 100, "exclude 不截断");
    assert_eq!(config.ignore[0].reason, "豁免 0", "首条保序");
    assert_eq!(config.ignore[99].reason, "豁免 99", "末条保序");
    assert_eq!(
        config.exclude[99], "src/gen99/**",
        "exclude 末条保序（无去重 / 重排）"
    );
}

/// 边界：`reason` 含 emoji / NUL / >1000 chars → `Ok` 且原文保留（不做长度上限）；
/// 含 emoji 的通配符条目 → `Ok`。
#[test]
fn 特殊字符原文保留() {
    let long_reason = format!("{}🎯", "长".repeat(1100));
    let temp = TempDir::new("special");
    temp.write_config(&format!(
        "{{ \"ignore\": [{{ \"from\": \"App\", \"to\": \"App.Auth\", \"reason\": \"{long_reason}\" }}], \"exclude\": [\"src/🎯/**\"] }}"
    ));
    let config = load(temp.root()).expect("特殊字符合法");
    assert_eq!(config.ignore[0].reason, long_reason, "超长 reason 原文保留");
    assert_eq!(config.exclude[0], "src/🎯/**", "含 emoji 的通配符条目");

    let nul = TempDir::new("nul-reason");
    nul.write_config(
        "{ \"ignore\": [{ \"from\": \"App\", \"to\": \"App.Auth\", \"reason\": \"a\\u0000b\" }] }",
    );
    let config = load(nul.root()).expect("NUL 转义合法");
    assert_eq!(config.ignore[0].reason, "a\u{0}b", "NUL 原样保留");
}

// ---------------------------------------------------------------------------
// 边界：定位单点（AC-9）
// ---------------------------------------------------------------------------

/// 边界（AC-9）：只有落在 `<archi_dir>/config.json` 的配置被读到——工作区配置
/// 位置 `<root>/openspec/config.json` 与模型目录下的同名文件均不被读取。
#[test]
fn 定位单点() {
    let temp = TempDir::new("location");
    temp.write_config("{ \"exclude\": [\"src/legacy/**\"] }");
    let config = load(temp.root()).expect("合法位置应被读到");
    assert_eq!(
        config.exclude,
        vec!["src/legacy/**".to_owned()],
        "架构域配置位置唯一合法"
    );

    let elsewhere = TempDir::new("location-elsewhere");
    elsewhere.write(
        "openspec/config.json",
        "{ \"exclude\": [\"src/workspace/**\"] }",
    );
    elsewhere.write(
        ".dev-team/architecture/models/config.json",
        "{ \"exclude\": [\"src/models/**\"] }",
    );
    let config = load(elsewhere.root()).expect("别处文件不影响");
    assert!(
        config.exclude.is_empty(),
        "工作区配置位置 / 模型目录下的同名文件 MUST NOT 被读取"
    );
}

/// 边界（AC-9）：`config.rs` 源码不含 `config.json` 连续字面量（文件名只在
/// `foundation::layout` 持有）。
#[test]
fn 产品源码零文件名字面量() {
    let source = include_str!("config.rs");
    assert!(
        !source.contains("config.json"),
        "架构域配置文件名字面量只允许出现在 foundation::layout"
    );
}

// ---------------------------------------------------------------------------
// 边界：确定性与根形态
// ---------------------------------------------------------------------------

/// 边界：同一文件连续两次读取结果逐字段相等（`exclude` / `ignore` 均保序）。
#[test]
fn 确定性() {
    let temp = TempDir::new("determinism");
    temp.write_config(
        "{\n  \"ignore\": [{ \"from\": \"App\", \"to\": \"App.Auth\", \"reason\": \"迁移期\" }],\n  \"exclude\": [\"src/legacy/**\", \"src/gen/**\"]\n}\n",
    );
    let first = load(temp.root()).expect("合法");
    let second = load(temp.root()).expect("合法");
    assert_eq!(first, second, "两次读取逐字段相等");
}

/// 边界：`root` 含空格 / 非 ASCII / 不存在 → 分别「能读到」与「视作缺失」，
/// 零 panic。
#[test]
fn 根路径边界() {
    let spaced = TempDir::new("根 含空格");
    spaced.write_config("{ \"exclude\": [\"src/legacy/**\"] }");
    let config = load(spaced.root()).expect("含空格与非 ASCII 的 root 仍可定位");
    assert_eq!(config.exclude.len(), 1, "定位成功");

    let deep = TempDir::new("deep");
    let nested = deep.root().join("a/b/c/d/e");
    fs::create_dir_all(&nested).expect("建深层目录失败");
    assert_eq!(
        load(&nested).expect("深层 root 合法"),
        CheckConfig::default(),
        "深层 root 无配置 ⇒ 空配置"
    );

    let missing = deep.root().join("no-such-root");
    assert_eq!(
        load(&missing).expect("不存在的 root 视为文件缺失"),
        CheckConfig::default(),
        "不存在的 root ⇒ 空配置（零报错）"
    );
}
