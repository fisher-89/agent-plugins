//! `relation`（单条关系的按需事实取样）的单元测试：模型面（声明 / 声明行 / 标记 /
//! 理由）→ 事实面（只扫 `from` 元素 `path` 下的源文件）→ 两桶（调 `drift::api_buckets`
//! 同一纯函数）→ 命中点，以及四条纪律（零写盘 / 零缓存 / 零报告 / 单一求差）。
//!
//! 零 mock：真实 std 自建 `TempDir`（`graph/model/*.toml` + 两族元素源文件 + 架构域
//! `config.json`）+ 真实 `dto::load` / `scan::scan` / `drift::api_buckets`；`CancelFlag`
//! 用真实 `never()`。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::cancel::CancelFlag;
use foundation::layout::archi_config_path;

use crate::drift::report::ArchiDriftOccurrence;
use crate::drift::ArchiDriftMode;
use crate::dto::EdgeMark;

use super::relation_detail;

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-relation-test-{}-{}-{}",
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

    fn write_model(&self, content: &str) {
        self.write(".dev-team/architecture/graph/model/01-test.toml", content);
    }

    fn write_config(&self, content: &str) {
        let path = archi_config_path(&self.0);
        fs::create_dir_all(path.parent().expect("父目录")).expect("创建架构域目录失败");
        fs::write(&path, content).expect("写配置失败");
    }

    /// 全量字节快照（相对路径 → 字节；用于「零写盘」对拍）。
    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        let mut frontier: Vec<PathBuf> = vec![self.0.clone()];
        while let Some(dir) = frontier.pop() {
            let Ok(read) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    frontier.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(&self.0)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                entries.push((relative, fs::read(&path).expect("读文件失败")));
            }
        }
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
// 夹具
// ---------------------------------------------------------------------------

/// 单关系模型（`Billing → Auth`，`apis` 由入参给出；两元素各持源目录）。
fn model(apis: &str) -> String {
    format!(
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Auth\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/auth/\"]\n\n",
            "[element.\"App.Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/billing/\"]\n\n",
            "[[relation]]\nfrom = \"App.Billing\"\nto = \"App.Auth\"\n{apis}",
        ),
        apis = apis
    )
}

/// 健康盘：`apis = ["alpha", "beta"]`，from 侧引入 `alpha` / `beta` / `gamma`。
fn healthy(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model(&model("apis = [\"alpha\", \"beta\"]\n"));
    temp.write(
        "src/auth/token.ts",
        "export const alpha = 1;\nexport const beta = 2;\nexport const gamma = 3;\n",
    );
    temp.write(
        "src/billing/x.ts",
        "import { alpha, beta, gamma } from '../auth/token';\n",
    );
    temp
}

/// 命中点三元组（`(file, line, specifier)`，便于逐条对拍）。
fn occurrences(entries: &[ArchiDriftOccurrence]) -> Vec<(String, u32, Option<String>)> {
    entries
        .iter()
        .map(|entry| (entry.file.clone(), entry.line, entry.specifier.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// 模型面
// ---------------------------------------------------------------------------

/// 正向：健康盘 → 声明清单按原文、事实名并集去重、两桶按 `api_buckets` 求差。
#[test]
fn 健康盘模型面与两桶() {
    let temp = healthy("healthy");
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert_eq!(
        detail.declared,
        vec!["alpha".to_owned(), "beta".to_owned()],
        "声明清单按模型原文"
    );
    assert_eq!(
        detail.used,
        vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()],
        "事实名并集去重 + 字典序"
    );
    assert_eq!(
        detail.used_not_declared,
        vec!["gamma".to_owned()],
        "用了没声明"
    );
    assert!(detail.declared_not_used.is_empty(), "无声明没用项");
}

/// 正向：`modelFile` / `modelLine` 指向该关系声明所在文件与 `from` 键行（可点开定位）。
#[test]
fn 模型行号指向关系声明() {
    let temp = healthy("model-line");
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert_eq!(
        detail.model_file, ".dev-team/architecture/graph/model/01-test.toml",
        "modelFile 为仓相对声明文件"
    );
    let source = model("apis = [\"alpha\", \"beta\"]\n");
    let expected = source
        .lines()
        .position(|line| line.contains("from = \"App.Billing\""))
        .expect("源码含 from 键") as u32
        + 1;
    assert_eq!(detail.model_line, expected, "modelLine = from 键行");
}

/// 正向：`mark = none`、`reason = null`（本变更不做边失配徽标）。
#[test]
fn 标记与理由恒空() {
    let temp = healthy("mark");
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert_eq!(detail.mark, EdgeMark::None, "本期恒 none");
    assert_eq!(detail.reason, None, "豁免理由恒 null");
}

/// 正向：`description` 透传——有 `desc` → 原文；无 `desc` → `null`。
#[test]
fn 说明透传两态() {
    for (tail, expected) in [
        ("desc = \"计费调用认证\"\n", Some("计费调用认证")),
        ("", None),
    ] {
        let temp = TempDir::new("description");
        temp.write_model(&format!(
            concat!(
                "[element.\"App\"]\nkind = \"package\"\n\n",
                "[element.\"App.Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n\n",
                "[element.\"App.Billing\"]\nkind = \"module\"\npath = [\"./src/billing/\"]\n\n",
                "[[relation]]\nfrom = \"App.Billing\"\nto = \"App.Auth\"\n{}",
            ),
            tail
        ));
        temp.write("src/billing/x.ts", "export const local = 1;\n");
        let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
        assert_eq!(detail.description.as_deref(), expected, "{tail:?} 透传两态");
    }
}

/// 边界：`apis` 省略 → `declared = []`、`declaredNotUsed = []`，事实面照出 `used`。
#[test]
fn 省略apis仍出事实() {
    let temp = TempDir::new("apis-absent");
    temp.write_model(&model(""));
    temp.write("src/auth/token.ts", "export const alpha = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { alpha } from '../auth/token';\n",
    );
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert!(detail.declared.is_empty(), "未声明 API 粒度");
    assert_eq!(detail.used, vec!["alpha".to_owned()], "事实面照出 used");
    assert_eq!(
        detail.used_not_declared,
        vec!["alpha".to_owned()],
        "两桶仍走同一纯函数（声明面为空 ⇒ 全部事实名落在 used ∖ declared）"
    );
    assert!(
        detail.declared_not_used.is_empty(),
        "声明面为空 ⇒ 声明没用桶恒空"
    );
}

/// 边界：`occurrences` 逐条含 `file` / `line` / `specifier` / `typeOnly`，按 file / line
/// 排序，同文件多行不聚合。
#[test]
fn 命中点逐条排序() {
    let temp = TempDir::new("occurrences");
    temp.write_model(&model("apis = [\"alpha\"]\n"));
    temp.write("src/auth/token.ts", "export const alpha = 1;\n");
    temp.write(
        "src/billing/b.ts",
        "import { alpha } from '../auth/token';\n",
    );
    temp.write(
        "src/billing/a.ts",
        "export const local = 1;\nimport { alpha } from '../auth/token';\n",
    );
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert_eq!(
        occurrences(&detail.occurrences),
        vec![
            (
                "src/billing/a.ts".to_owned(),
                2,
                Some("../auth/token".to_owned())
            ),
            (
                "src/billing/b.ts".to_owned(),
                1,
                Some("../auth/token".to_owned())
            ),
        ],
        "按 file 再 line 排序、同文件多行不聚合"
    );
}

/// 边界：`apis` 含乱序多名 → `declared` 按模型原文序出线（求差在纯函数内做归一）。
#[test]
fn 声明清单原文序() {
    let temp = TempDir::new("declared-order");
    temp.write_model(&model("apis = [\"Session\", \"AgentInput\"]\n"));
    temp.write(
        "src/auth/token.ts",
        "export const AgentInput = 1;\nexport const Session = 2;\n",
    );
    temp.write(
        "src/billing/x.ts",
        "import { AgentInput, Session } from '../auth/token';\n",
    );
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert_eq!(
        detail.declared,
        vec!["Session".to_owned(), "AgentInput".to_owned()],
        "declared 保模型原文序"
    );
    assert_eq!(
        detail.used,
        vec!["AgentInput".to_owned(), "Session".to_owned()],
        "used 恒字典序"
    );
    assert!(detail.used_not_declared.is_empty(), "同一集合 ⇒ 空桶");
    assert!(detail.declared_not_used.is_empty(), "同一集合 ⇒ 空桶");
}

/// 边界：模型值面违例（`apis = [""]` / 重复名）→ `Err`（`invalidValue` 文案，模型整体
/// 停摆）。
#[test]
fn 模型值面违例即err() {
    for apis in ["apis = [\"\"]\n", "apis = [\"A\", \"A\"]\n"] {
        let temp = TempDir::new("invalid-apis");
        temp.write_model(&model(apis));
        temp.write("src/billing/x.ts", "export const local = 1;\n");
        let error =
            relation_detail(temp.root(), "App.Billing", "App.Auth").expect_err("值面违例必须 Err");
        assert!(error.contains("模型结构错"), "{apis:?} 走诊断文案：{error}");
        assert!(error.contains("invalidValue"), "诊断种类在文案内：{error}");
    }
}

/// 异常：关系未声明（元素对缺席）→ `Err` 文案点名该元素对与关系总数。
#[test]
fn 关系缺席即err并点名() {
    let temp = healthy("relation-absent");
    let error =
        relation_detail(temp.root(), "App.Auth", "App.Billing").expect_err("反向关系未声明 ⇒ Err");
    assert!(error.contains("关系不存在"), "文案点名关系不存在：{error}");
    assert!(
        error.contains("App.Auth") && error.contains("App.Billing"),
        "文案含元素对：{error}"
    );
    assert!(error.contains("1 条关系"), "文案含关系总数：{error}");
}

/// 异常：`from` / `to` 为空串或空白串 → `Err`（元素对不存在），零 panic。
#[test]
fn 空与空白元素对即err() {
    let temp = healthy("empty-pair");
    for (from, to) in [
        ("", "App.Auth"),
        ("   ", "App.Auth"),
        ("App.Billing", ""),
        ("App.Billing", "  "),
    ] {
        let result = relation_detail(temp.root(), from, to);
        assert!(result.is_err(), "{from:?} → {to:?} 必须 Err");
        assert!(!result.unwrap_err().is_empty(), "错误文案非空");
    }
}

/// 异常：结构诊断非空（坏模型）→ `Err` 走 `diagnostics_error` 同一文案（零部分详情）。
#[test]
fn 坏模型走诊断文案() {
    let temp = TempDir::new("broken-model");
    temp.write_model(concat!(
        "[element.\"App.Auth\"]\nkind = \"module\"\npath = [\"./src/auth/\"]\n\n",
        "[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Legacy\"\napis = [\"A\"]\n",
    ));
    let error = relation_detail(temp.root(), "App.Auth", "App.Legacy").expect_err("坏模型必须 Err");
    assert_eq!(
        error,
        crate::dto::diagnostics_error(&crate::dto::load(temp.root()).diagnostics),
        "与诊断文案单点逐字相同"
    );
}

/// 异常：模型目录缺席（零元素零关系）→ `Err`（计数 0 的文案）。
#[test]
fn 模型目录缺席即err() {
    let temp = TempDir::new("no-model");
    temp.write("src/a.ts", "export const a = 1;\n");
    let error =
        relation_detail(temp.root(), "App.Auth", "App.Billing").expect_err("无模型必须 Err");
    assert!(error.contains("0 个元素"), "文案含元素计数：{error}");
}

// ---------------------------------------------------------------------------
// 事实面
// ---------------------------------------------------------------------------

/// 正向：定向扫描只覆盖 `from` 元素 `path` 下文件——`to` 侧文件的反向 import 不计入
/// `used`。
#[test]
fn 只扫from侧文件() {
    let temp = healthy("from-side-only");
    temp.write(
        "src/auth/token.ts",
        "export const alpha = 1;\nexport const beta = 2;\nexport const gamma = 3;\n",
    );
    temp.write(
        "src/auth/reverse.ts",
        "import { zeta } from '../billing/x';\nexport const zeta = 4;\n",
    );
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert!(
        !detail.used.iter().any(|name| name == "zeta"),
        "to 侧的反向 import 不计入 used：{:?}",
        detail.used
    );
    assert!(
        detail
            .occurrences
            .iter()
            .all(|entry| entry.file.starts_with("src/billing/")),
        "命中点只在 from 侧文件：{:?}",
        occurrences(&detail.occurrences)
    );
}

/// 边界：`from` 侧零源文件（`path` 指向空目录）→ `used = []`、`occurrences = []`、
/// `declaredNotUsed` = 全部声明名。
#[test]
fn from侧零源文件() {
    let temp = TempDir::new("from-side-empty");
    temp.write_model(&model("apis = [\"alpha\", \"beta\"]\n"));
    temp.write("src/billing/.keep", "占位\n");
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert!(detail.used.is_empty(), "零源文件 ⇒ 零事实名");
    assert!(detail.occurrences.is_empty(), "零命中点");
    assert_eq!(
        detail.declared_not_used,
        vec!["alpha".to_owned(), "beta".to_owned()],
        "声明全未出现（字典序）"
    );
}

/// 异常：`from` 元素 `path` 指向**不存在**的目录 → 空白名单（零 panic）、`used = []`、
/// `occurrences = []`、`declaredNotUsed` = 全部声明名（`path` 缺席不是错误）。
#[test]
fn from侧path指向不存在目录() {
    let temp = TempDir::new("from-side-missing-path");
    temp.write_model(concat!(
        "[element.\"App\"]\nkind = \"package\"\n\n",
        "[element.\"App.Auth\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/auth/\"]\n\n",
        "[element.\"App.Billing\"]\nkind = \"module\"\ngate = \"warn\"\npath = [\"./src/absent/\"]\n\n",
        "[[relation]]\nfrom = \"App.Billing\"\nto = \"App.Auth\"\napis = [\"alpha\", \"beta\"]\n",
    ));
    temp.write("src/auth/token.ts", "export const alpha = 1;\n");
    let detail =
        relation_detail(temp.root(), "App.Billing", "App.Auth").expect("路径不存在不构成错误");
    assert!(detail.used.is_empty(), "空白名单 ⇒ 零事实名");
    assert!(detail.occurrences.is_empty(), "零命中点");
    assert_eq!(
        detail.declared_not_used,
        vec!["alpha".to_owned(), "beta".to_owned()],
        "声明全未出现（字典序）"
    );
}

/// 边界：名字取不出的形态（`export *` / `import default`）→ 不产假名（`used` 少该名，
/// `declaredNotUsed` 侧如实呈现）。
#[test]
fn 名字取不出不产假名() {
    let temp = TempDir::new("nameless");
    temp.write_model(&model("apis = [\"alpha\"]\n"));
    temp.write("src/auth/token.ts", "export const alpha = 1;\n");
    temp.write("src/billing/x.ts", "export * from '../auth/token';\n");
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert!(detail.used.is_empty(), "glob 形态取不到名字");
    assert_eq!(detail.occurrences.len(), 1, "命中点仍在案");
    assert_eq!(
        detail.declared_not_used,
        vec!["alpha".to_owned()],
        "如实呈现声明没用"
    );
}

/// 异常：扫描失败（非法 `exclude` 规则）→ `Err` 经 `CheckError::message()` 透传（含
/// 成因）。
#[test]
fn 扫描失败透传成因() {
    let temp = healthy("scan-failure");
    temp.write_config("{ \"exclude\": [\"src/[legacy\"] }");
    let error =
        relation_detail(temp.root(), "App.Billing", "App.Auth").expect_err("非法 glob 必须 Err");
    assert!(error.contains("src/[legacy"), "文案含非法条目原文：{error}");
}

/// 异常：架构域配置非法（未知键 / 缺 `reason`）→ `Err`（`load_config` 文案直传），零
/// panic。
#[test]
fn 配置非法透传文案() {
    for config in [
        "{ \"rules\": [] }",
        "{ \"ignore\": [{ \"from\": \"App.Billing\", \"to\": \"App.Auth\" }] }",
    ] {
        let temp = healthy("bad-config");
        temp.write_config(config);
        let error =
            relation_detail(temp.root(), "App.Billing", "App.Auth").expect_err("配置非法必须 Err");
        assert!(!error.is_empty(), "零 panic 且文案非空");
    }
}

/// 边界：零取消语义——`CancelFlag::never()` 下单次调用恒 `Ok`。
#[test]
fn 零取消语义() {
    let temp = healthy("no-cancel");
    assert!(!CancelFlag::never().is_set(), "真实 never 标志不置位");
    let outcome = relation_detail(temp.root(), "App.Billing", "App.Auth");
    assert!(outcome.is_ok(), "单次调用恒 Ok：{:?}", outcome.err());
}

/// 正常路径的扫描入参形态自证：`from_side_sources` 只把归属 `from` 的源文件放进白名单
/// （兄弟元素文件与非源文件排除；被 `exclude` 覆盖的源文件仍入——排除在扫描层判）。
#[test]
fn from侧白名单只含归属from的源文件() {
    let temp = healthy("whitelist");
    temp.write("src/auth/sibling.ts", "export const sibling = 1;\n");
    temp.write("src/billing/README.md", "# 说明\n");
    temp.write("src/billing/data.json", "{}\n");
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    let files: Vec<&str> = detail
        .occurrences
        .iter()
        .map(|entry| entry.file.as_str())
        .collect();
    assert_eq!(
        files,
        vec!["src/billing/x.ts"],
        "兄弟元素文件与非源文件不入扫描面"
    );
}

// ---------------------------------------------------------------------------
// 单点纪律
// ---------------------------------------------------------------------------

/// 组合（源码复核）：`api_buckets(` 在产品代码（非 `*_test.rs`）恰两处调用（`drift/mod.rs`
/// 与 `relation.rs`），`relation.rs` 零第二套集合差。
#[test]
fn 求差单点源码复核() {
    assert_eq!(
        include_str!("drift/mod.rs").matches("api_buckets(").count(),
        2,
        "drift 侧定义 + 装配各一处"
    );
    let relation_source = include_str!("relation.rs");
    assert_eq!(
        relation_source.matches("api_buckets(").count(),
        1,
        "relation 只转调同一纯函数"
    );
    assert!(
        !relation_source.contains("difference(") && !relation_source.contains("BTreeSet(&"),
        "relation 零第二套集合差"
    );
}

/// 组合：零写盘（调用前后盘面字节快照逐一相同）+ 零缓存（两次调用之间改盘面 → 第二次
/// 读到新状态）。
#[test]
fn 零写盘与零缓存() {
    let temp = healthy("no-write");
    let before = temp.snapshot();
    let first = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    let after = temp.snapshot();
    assert_eq!(before, after, "按需取数零写盘");

    temp.write(
        "src/billing/y.ts",
        "import { beta } from '../auth/token';\n",
    );
    let second = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    assert_eq!(
        first.occurrences.len(),
        1,
        "首轮命中点一条：{:?}",
        occurrences(&first.occurrences)
    );
    assert_eq!(
        second.occurrences.len(),
        2,
        "改盘面后第二轮读到新状态（零缓存）：{:?}",
        occurrences(&second.occurrences)
    );
}

/// 组合：不组装报告——返回字段集恰十二个（无 `status` / `passed` / `errors` /
/// `warnings`，嵌套只复用 `ArchiDriftOccurrence`）。
#[test]
fn 不组装报告() {
    let temp = healthy("no-report");
    let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
    let json = serde_json::to_value(&detail).expect("序列化成功");
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("详情为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "declared",
            "declaredNotUsed",
            "description",
            "from",
            "mark",
            "modelFile",
            "modelLine",
            "occurrences",
            "reason",
            "to",
            "used",
            "usedNotDeclared",
        ],
        "恰十二字段（不含报告结论面）"
    );
    let occurrence = json["occurrences"][0]
        .as_object()
        .expect("命中点为对象")
        .keys()
        .map(String::as_str)
        .collect::<Vec<&str>>();
    assert_eq!(
        occurrence.len(),
        4,
        "命中点复用 ArchiDriftOccurrence 四字段：{occurrence:?}"
    );
}
/// 组合：与 `drift::check` 对同一 `(from, to)`（`apis` 非空）→ `usedNotDeclared` /
/// `declaredNotUsed` 逐字相等（同一 `api_buckets`；AC-5）。
#[test]
fn 与drift判定逐字相等() {
    // 三形态：全匹配 / 漏项（声明多一个） / 多项（事实多一个）
    for (apis, import) in [
        ("apis = [\"alpha\", \"beta\"]\n", "alpha, beta"),
        ("apis = [\"alpha\", \"beta\"]\n", "alpha"),
        ("apis = [\"alpha\"]\n", "alpha, gamma"),
    ] {
        let temp = healthy("cross-check");
        temp.write_model(&model(apis));
        temp.write(
            "src/billing/x.ts",
            &format!("import {{ {import} }} from '../auth/token';\n"),
        );
        let detail = relation_detail(temp.root(), "App.Billing", "App.Auth").expect("应成功");
        let report = crate::drift::check(crate::drift::CheckInput {
            root: temp.root(),
            mode: ArchiDriftMode::Full,
            changed: &[],
            change: None,
            cancel: CancelFlag::never(),
        })
        .expect("判定应成功");

        let drift_used_not_declared = report
            .errors
            .iter()
            .chain(report.warnings.iter())
            .find(|entry| {
                entry.from.as_deref() == Some("App.Billing")
                    && entry.to.as_deref() == Some("App.Auth")
            })
            .map(|entry| entry.kind);
        if detail.used_not_declared.is_empty() {
            assert_eq!(
                drift_used_not_declared, None,
                "{import:?} 两桶空时 drift 侧零条目"
            );
        } else {
            assert_eq!(
                drift_used_not_declared,
                Some(crate::drift::ArchiDriftKind::UndeclaredApi),
                "{import:?} 两面对同一对给出同一结论"
            );
        }

        let drift_declared_not_used = report.observations.iter().any(|entry| {
            entry.kind == crate::drift::ArchiObservationKind::UnusedDeclaration
                && entry.from == "App.Billing"
                && entry.to == "App.Auth"
        });
        assert_eq!(
            drift_declared_not_used,
            !detail.declared_not_used.is_empty(),
            "{import:?} 声明没用两侧口径一致"
        );

        // 事实面同源：drift 侧条目（error / warning）的命中点与该详情逐条同形
        if let Some(entry) = report.errors.iter().find(|entry| {
            entry.from.as_deref() == Some("App.Billing") && entry.to.as_deref() == Some("App.Auth")
        }) {
            assert_eq!(
                occurrences(&entry.occurrences),
                occurrences(&detail.occurrences),
                "{import:?} 命中点逐条同形（同一聚合口径）"
            );
        }
    }
}
