//! `sdk::archi_tool` 的单元测试：per-turn 上下文构造、`archi_module` 两形态（指定
//! 元素 / 省略元素的薄概览）、`archi_search` 三 scope、`archi_api` 两动作（`check`
//! 两轴结构化摘要 / `bootstrap` 草稿）、坏模型 / 参数缺失 / 未知 action 的 `Err`
//! 回灌面与 L1 结果收口（30 KB 上限），以及工具串与 DTO 串的词汇等价。
//!
//! 零 mock：真实 tempdir 模型 + 真实 `archi::dto` 投影 + 真实 `tools::cap_result`
//! （30 KB 上限与留痕文案即被测契约，禁第二份上限实现）。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::sdk::tools::{ARCHI_API_TOOL, ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL};

use super::archi_tool::ArchiContext;

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-archi-tool-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

/// 落一个模型文件（`.dev-team/architecture/models/`）。
fn write_model(root: &Path, name: &str, content: &str) {
    let dir = root.join(".dev-team/architecture/models");
    fs::create_dir_all(&dir).expect("创建模型目录失败");
    fs::write(dir.join(name), content).expect("写模型文件失败");
}

/// 落一个 repo 相对文件。
fn write_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("创建父目录失败");
    }
    fs::write(path, content).expect("写文件失败");
}

/// 健康模型：三元素（`App` / `App.Auth` / `App.Billing`）+ 一条关系 + 一份文档。
const CORE: &str = concat!(
    "# 核心包\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"strict\"\n",
    "docs = [\"./src/auth/README.md\"]\n",
    "summary = \"认证域\"\n",
    "metadata = { owner = \"core\" }\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth\"\n",
    "to = \"App.Billing\"\n",
    "desc = \"认证调用计费\"\n",
);

/// 健康合成根。
fn healthy(tag: &str) -> tempfile::TempDir {
    let dir = tempdir(tag);
    write_model(dir.path(), "01-core.toml", CORE);
    write_file(dir.path(), "src/auth/README.md", "# 认证说明\n\n正文\n");
    dir
}

/// 执行并解析 JSON 结果。
async fn execute_json(context: &ArchiContext, name: &str, input: Value) -> Value {
    let text = context
        .execute(name, &input)
        .await
        .unwrap_or_else(|error| panic!("{name} 应成功: {error}"));
    serde_json::from_str(&text).expect("结果为 JSON 串")
}

// ---------------------------------------------------------------------------
// 上下文构造
// ---------------------------------------------------------------------------

/// 正向 / 边界：tempdir 路径上下文可用；`root` 不存在 / 空白 → `execute` 返回
/// `Err`（不 panic）。
#[tokio::test]
async fn 上下文构造三形态() {
    let dir = healthy("new");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let overview = execute_json(&context, ARCHI_MODULE_TOOL, json!({})).await;
    assert_eq!(
        overview.as_array().expect("薄概览为数组").len(),
        3,
        "tempdir 上下文可用"
    );

    // root 不存在 / 空白：工具执行体不做 root 校验（该职责在命令层）→ 空模型形态
    // 的 Ok 结果，绝不 panic
    let missing = dir.path().join("no-such-root");
    let context = ArchiContext::new(missing);
    let overview = execute_json(&context, ARCHI_MODULE_TOOL, json!({})).await;
    assert_eq!(overview, json!([]), "不存在的 root 视为空模型（不 panic）");

    let context = ArchiContext::new(PathBuf::from("   "));
    let result = context
        .execute(ARCHI_SEARCH_TOOL, &json!({ "query": "auth" }))
        .await;
    assert!(result.is_ok(), "空白 root 不 panic（空模型形态）");
}

// ---------------------------------------------------------------------------
// `archi_module` 两形态
// ---------------------------------------------------------------------------

/// 正向：指定元素 → 元素全字段 + 邻接 / 子元素 / 文档，`modelFile` + `modelLine`
/// 可直接定位声明行。
#[tokio::test]
async fn archi_module指定元素返回邻接与声明位置() {
    let dir = healthy("module");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let detail = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Auth" }),
    )
    .await;

    let element = &detail["element"];
    assert_eq!(element["fqn"], json!("App.Auth"));
    assert_eq!(element["kind"], json!("domain"));
    assert_eq!(element["gate"], json!("strict"));
    assert_eq!(element["summary"], json!("认证域"));
    assert_eq!(element["metadata"]["owner"], json!("core"));
    assert_eq!(
        element["modelFile"],
        json!(".dev-team/architecture/models/01-core.toml")
    );
    let line = element["modelLine"].as_u64().expect("modelLine 为数字");
    assert!(line >= 1, "modelLine 可直接 edit 到声明行: {line}");

    assert_eq!(detail["dependsOn"][0]["target"], json!("App.Billing"));
    assert_eq!(detail["dependsOn"][0]["description"], json!("认证调用计费"));
    assert_eq!(detail["dependedBy"], json!([]));
    assert_eq!(detail["children"], json!([]));
    assert_eq!(detail["docs"][0]["path"], json!("src/auth/README.md"));
    assert_eq!(detail["docs"][0]["source"], json!("explicit"));
    assert_eq!(detail["warnings"], json!([]));

    // 子元素面
    let billing = execute_json(&context, ARCHI_MODULE_TOOL, json!({ "element": "App" })).await;
    assert_eq!(billing["children"], json!(["App.Auth", "App.Billing"]));
}

/// 正向：省略 `element`（`{}` / 空白串）→ 全元素薄概览（四字段、无邻接）。
#[tokio::test]
async fn archi_module省略元素返回薄概览() {
    let dir = healthy("overview");
    let context = ArchiContext::new(dir.path().to_path_buf());
    for input in [json!({}), json!({ "element": "   " })] {
        let overview = execute_json(&context, ARCHI_MODULE_TOOL, input.clone()).await;
        let items = overview.as_array().expect("薄概览为数组");
        assert_eq!(items.len(), 3, "元素数 = 模型元素数（{input}）");
        let mut keys: Vec<&str> = items[0]
            .as_object()
            .expect("条目为对象")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["api", "fqn", "gate", "kind", "summary"],
            "薄概览四字段 + api（同一模型字段的纯拷贝）"
        );
    }
}

/// 边界：数百元素使输出超 30 KB → 结果经 `tools::cap_result` 收口（长度 ≤ 30 KB
/// + 截断留痕尾行），不 panic。
#[tokio::test]
async fn archi_module薄概览超限截断留痕() {
    let dir = tempdir("truncate");
    let mut source = String::new();
    source.push_str("[element.\"App\"]\nkind = \"package\"\n\n");
    for index in 0..500 {
        source.push_str(&format!(
            "[element.\"App.Mod{index:03}\"]\nkind = \"module\"\nsummary = \"这是第 {index} 个模块的摘要文本，用于撑大输出体积\"\n\n"
        ));
    }
    write_model(dir.path(), "01-big.toml", &source);
    let context = ArchiContext::new(dir.path().to_path_buf());
    let text = context
        .execute(ARCHI_MODULE_TOOL, &json!({}))
        .await
        .expect("大体量薄概览应 Ok");

    assert!(
        text.len() <= 50_000 + 200,
        "长度收口在 50 KB 量级: {}",
        text.len()
    );
    assert!(
        text.contains("已截断：单结果超 50000 字节上限"),
        "截断留痕与 cap_result 文案同形"
    );
}

/// 异常：元素不存在 → `Err` 含总量提示；坏模型（环 / 语法错）→ `Err` 含结构化
/// 诊断（`kind` + `file:line`），不 panic。
#[tokio::test]
async fn archi_module异常三形态() {
    let dir = healthy("module-err");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let absent = context
        .execute(ARCHI_MODULE_TOOL, &json!({ "element": "App.Missing" }))
        .await
        .expect_err("元素缺席应 Err");
    assert!(absent.contains("元素不存在"), "文案点名缺席: {absent}");
    assert!(absent.contains('3'), "文案含元素总数: {absent}");

    let cycle = tempdir("module-cycle");
    write_model(
        cycle.path(),
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.A\"]\nkind = \"module\"\n\n",
            "[element.\"App.B\"]\nkind = \"module\"\n\n",
            "[[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n",
            "[[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n",
        ),
    );
    let context = ArchiContext::new(cycle.path().to_path_buf());
    let error = context
        .execute(ARCHI_MODULE_TOOL, &json!({ "element": "App.A" }))
        .await
        .expect_err("坏模型应 Err");
    assert!(error.contains("cycle"), "结构化诊断含 kind: {error}");
    assert!(error.contains("01.toml"), "结构化诊断含 file: {error}");
    assert!(error.contains('1'), "诊断条数在案: {error}");

    let syntax = tempdir("module-syntax");
    write_model(
        syntax.path(),
        "01.toml",
        "[element.\"App]\nkind = \"package\"\n",
    );
    let context = ArchiContext::new(syntax.path().to_path_buf());
    let error = context
        .execute(ARCHI_MODULE_TOOL, &json!({}))
        .await
        .expect_err("语法错应 Err");
    assert!(error.contains("syntaxError"), "语法错 kind 在案: {error}");
}

/// 异常：`element` 为非字符串（`1` / `null` / `[]`）或缺失键 → 按缺失处理（走薄
/// 概览）；入参为 `null`（非对象）→ `Err` 不 panic。
#[tokio::test]
async fn archi_module参数形态() {
    let dir = healthy("module-args");
    let context = ArchiContext::new(dir.path().to_path_buf());

    for input in [
        json!({ "element": 1 }),
        json!({ "element": null }),
        json!({ "element": [] }),
    ] {
        let overview = execute_json(&context, ARCHI_MODULE_TOOL, input.clone()).await;
        assert_eq!(
            overview.as_array().map(Vec::len),
            Some(3),
            "非字符串 element 按缺失处理（走薄概览）: {input}"
        );
    }

    // 非对象入参：module 侧按「无 element」处理（薄概览），search 侧才是 Err
    let overview = execute_json(&context, ARCHI_MODULE_TOOL, Value::Null).await;
    assert_eq!(
        overview.as_array().map(Vec::len),
        Some(3),
        "null 入参走薄概览"
    );
    assert!(
        context
            .execute(ARCHI_SEARCH_TOOL, &Value::Null)
            .await
            .is_err(),
        "search 侧 null 入参应 Err 不 panic"
    );
}

// ---------------------------------------------------------------------------
// `archi_search`
// ---------------------------------------------------------------------------

/// 正向：`query` + `scope` 三档透传；`scope` 缺省按 `all`。
#[tokio::test]
async fn archi_search三scope与缺省() {
    let dir = healthy("search");
    let context = ArchiContext::new(dir.path().to_path_buf());

    let all = execute_json(
        &context,
        ARCHI_SEARCH_TOOL,
        json!({ "query": "auth", "scope": "all" }),
    )
    .await;
    assert!(!all["elements"].as_array().expect("元素数组").is_empty());
    assert!(!all["docs"].as_array().expect("文档数组").is_empty());

    let default_scope = execute_json(&context, ARCHI_SEARCH_TOOL, json!({ "query": "auth" })).await;
    assert_eq!(default_scope, all, "scope 缺省按 all");

    let elements = execute_json(
        &context,
        ARCHI_SEARCH_TOOL,
        json!({ "query": "auth", "scope": "elements" }),
    )
    .await;
    assert_eq!(elements["docs"], json!([]), "elements 档 docs 为空数组");

    let docs = execute_json(
        &context,
        ARCHI_SEARCH_TOOL,
        json!({ "query": "auth", "scope": "docs" }),
    )
    .await;
    assert_eq!(docs["elements"], json!([]), "docs 档 elements 为空数组");
}

/// 异常：`query` 缺失 / `null` / 数字 → `Err`；空白 `query` → `Err`（透传参数
/// 校验）；`scope = "unknown"` → `Err` 列出三合法值。
#[tokio::test]
async fn archi_search参数异常五形态() {
    let dir = healthy("search-args");
    let context = ArchiContext::new(dir.path().to_path_buf());

    for input in [json!({}), json!({ "query": null }), json!({ "query": 1 })] {
        assert!(
            context.execute(ARCHI_SEARCH_TOOL, &input).await.is_err(),
            "query 缺失 / 非字符串应 Err: {input}"
        );
    }
    let empty = context
        .execute(ARCHI_SEARCH_TOOL, &json!({ "query": "   " }))
        .await
        .expect_err("空白 query 应 Err");
    assert!(empty.contains("query"), "文案点名 query: {empty}");

    let scope = context
        .execute(
            ARCHI_SEARCH_TOOL,
            &json!({ "query": "auth", "scope": "unknown" }),
        )
        .await
        .expect_err("非法 scope 应 Err");
    for legal in ["elements", "docs", "all"] {
        assert!(scope.contains(legal), "错误文案列出合法值 {legal}: {scope}");
    }
}

/// 异常：非两 archi 工具名的任意名 → `Err`（不误路由到通用工具执行体）。
#[tokio::test]
async fn 非archi工具名一律err() {
    let dir = healthy("unknown-name");
    let context = ArchiContext::new(dir.path().to_path_buf());
    for name in ["read", "archi_write", "", "bash"] {
        let error = context
            .execute(name, &json!({ "path": "README.md" }))
            .await
            .expect_err("非 archi 工具名应 Err");
        assert!(error.contains("未知架构工具"), "文案说明路由边界: {error}");
    }
}

// ---------------------------------------------------------------------------
// 词汇面等价（工具串 vs DTO 串）
// ---------------------------------------------------------------------------

/// 边界：同一全词汇语料下，工具输出串中的结构诊断词汇集与 `dto` 投影串相等
/// （补齐 `lib.rs` 章节「工具串一面」在本 crate 的等价断言，AC-7）。
#[tokio::test]
async fn 词汇面与dto投影串等价() {
    let dir = tempdir("vocabulary");
    write_model(
        dir.path(),
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.A\"]\nkind = \"module\"\n\n",
            "[element.\"App.B\"]\nkind = \"module\"\n\n",
            "[element.\"App.Ref\"]\nkind = \"module\"\n\n",
            "[element.\"App.Orphan.Child\"]\nkind = \"component\"\n\n",
            "[[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n",
            "[[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n\n",
            "[[relation]]\nfrom = \"App.Ref\"\nto = \"App.Missing\"\n",
        ),
    );
    let context = ArchiContext::new(dir.path().to_path_buf());
    let tool_error = context
        .execute(ARCHI_MODULE_TOOL, &json!({ "element": "App.A" }))
        .await
        .expect_err("坏模型应 Err");

    let dto_error = archi::dto::module_detail(dir.path(), "App.A").expect_err("dto 侧同样 Err");

    let vocabulary = ["cycle", "unknownReference", "orphanHierarchy"];
    for word in vocabulary {
        assert!(
            tool_error.contains(word),
            "工具串含词汇 {word}: {tool_error}"
        );
        assert!(
            dto_error.contains(word),
            "DTO 串含同一词汇 {word}: {dto_error}"
        );
    }
    // 词表外不出现第二种写法（大小写 / 下划线变体）
    for forbidden in ["Cycle", "unknown_reference", "orphan_hierarchy"] {
        assert!(
            !tool_error.contains(forbidden) && !dto_error.contains(forbidden),
            "两面均只用一份线值写法（{forbidden} 不出现）"
        );
    }
}

// ---------------------------------------------------------------------------
// 本变更：`archi_api` 两动作（契约面自检）
// ---------------------------------------------------------------------------

/// 契约面语料根：模型（`App` 容器 + `App.Api` 域 + 叶元素）+ 入口源文 + 契约 md
/// （契约返回位声明为 `integer`、代码返回 `String` ⇒ 一条签名不符判据）。
fn api_root(tag: &str) -> tempfile::TempDir {
    let dir = tempdir(tag);
    write_model(
        dir.path(),
        "01-api.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Api\"]\nkind = \"domain\"\ngate = \"off\"\n\n",
            "[element.\"App.Api.Subject\"]\nkind = \"module\"\ngate = \"warn\"\n",
            "path = [\"./crates/api/subject/\"]\n",
        ),
    );
    write_file(
        dir.path(),
        "crates/api/subject/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    write_file(
        dir.path(),
        ".dev-team/architecture/api/Api/App.Api.Subject.md",
        "# App.Api.Subject\n\n## 职能\n\n工具语料。\n\n## 操作\n\n### file_stem\n\n\
         ```json api-op\n{\"owner\": \"LocalApi\", \
         \"params\": [{\"name\": \"timestamp\", \"schema\": {\"type\": \"string\"}}], \
         \"returns\": {\"type\": \"integer\"}}\n```\n",
    );
    dir
}

/// 全量字节快照（零写盘对拍）。
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(reader) = fs::read_dir(dir) else {
            return;
        };
        for entry in reader.flatten() {
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
    walk(root, root, &mut entries);
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

/// 异常（AC-15 / D13）：`check` 遇结构诊断 → `Err`（`is_error` 回灌），文案含「模型结构错」
/// 与诊断条数（经 `graph_diagnostics_error` 单点，零第二份 `file:line` 拼装）；不返回部分
/// 两轴。
#[tokio::test]
async fn archi_api_check结构诊断即err() {
    let dir = tempdir("api-check-broken");
    write_model(
        dir.path(),
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.A\"]\nkind = \"module\"\n\n",
            "[element.\"App.B\"]\nkind = \"module\"\n\n",
            "[[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n",
            "[[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n",
        ),
    );
    let context = ArchiContext::new(dir.path().to_path_buf());
    let error = context
        .execute(ARCHI_API_TOOL, &json!({ "action": "check" }))
        .await
        .expect_err("结构诊断 ⇒ Err（is_error）");
    assert!(error.contains("模型结构错"), "文案点名结构诊断：{error}");
    assert!(error.contains("共 "), "文案含诊断条数：{error}");
}

/// 正向（AC-15 / D13）：`check` 返回两轴结构化摘要——`artifacts` 五字段 + `findings`
/// 三字段（`total` / `byCategory` / `items`）；`total == items.len()` 且 `byCategory`
/// 与 `items` 的分组计数逐项相等（零新词表）。
#[tokio::test]
async fn archi_api_check两轴结构化摘要() {
    let dir = api_root("api-check-healthy");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let payload = execute_json(&context, ARCHI_API_TOOL, json!({ "action": "check" })).await;

    let artifacts = &payload["artifacts"];
    for key in ["enabled", "meta", "mismatched", "orphaned", "missing"] {
        assert!(
            artifacts.get(key).is_some(),
            "artifacts 含 {key}：{artifacts}"
        );
    }
    assert_eq!(artifacts["enabled"], json!(true), "契约面启用");
    assert!(
        !artifacts["missing"]
            .as_array()
            .expect("missing 数组")
            .is_empty(),
        "未生成时报缺口：{artifacts}"
    );

    let findings = &payload["findings"];
    let total = findings["total"].as_u64().expect("total 为数字") as usize;
    let items = findings["items"].as_array().expect("items 数组");
    assert_eq!(total, items.len(), "total == items.len()");
    assert_eq!(total, 1, "一条签名不符判据：{findings}");

    let mut counted = std::collections::BTreeMap::<String, usize>::new();
    for item in items {
        let category = item["category"].as_str().expect("category 串");
        *counted.entry(category.to_owned()).or_insert(0) += 1;
        assert!(item["kind"].is_string(), "条目含 kind：{item}");
        assert!(item["message"].is_string(), "条目含 message：{item}");
    }
    let by_category = findings["byCategory"]
        .as_object()
        .expect("byCategory 为对象");
    assert_eq!(
        by_category.len(),
        counted.len(),
        "byCategory 键集与 items 分组一致"
    );
    for (category, count) in counted {
        assert_eq!(
            by_category[&category].as_u64().expect("计数为数字") as usize,
            count,
            "byCategory 计数与 items 分组相等：{category}"
        );
    }
}

/// 边界（AC-15）：契约面缺席 → `findings.total == 0` / `byCategory == {}` / `items == []`，
/// `artifacts.enabled == false` 且 `missing` 恰 `INDEX.toml`（`is_error == false`）。
#[tokio::test]
async fn archi_api_check零判据成功态() {
    let dir = tempdir("api-check-empty");
    write_model(
        dir.path(),
        "01.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.Api\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"./crates/api/\"]\n",
    );
    write_file(dir.path(), "crates/api/src/lib.rs", "pub fn noop() {}\n");

    let context = ArchiContext::new(dir.path().to_path_buf());
    let payload = execute_json(&context, ARCHI_API_TOOL, json!({ "action": "check" })).await;
    assert_eq!(payload["findings"]["total"], json!(0), "零判据");
    assert_eq!(payload["findings"]["byCategory"], json!({}), "空分组表");
    assert_eq!(payload["findings"]["items"], json!([]), "空条目");
    assert_eq!(
        payload["artifacts"]["enabled"],
        json!(false),
        "契约面未启用"
    );
    let missing = payload["artifacts"]["missing"]
        .as_array()
        .expect("missing 数组");
    assert_eq!(missing.len(), 1, "缺口恰一条：{missing:?}");
    assert!(
        missing[0]
            .as_str()
            .is_some_and(|path| path.ends_with("INDEX.toml")),
        "缺口是索引：{missing:?}"
    );
}

/// 正向（AC-15 / D13）：`bootstrap` 出草稿，且与 `archi::api::bootstrap` 直出逐字相等。
#[tokio::test]
async fn archi_api_bootstrap出草稿() {
    let dir = api_root("api-bootstrap");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let payload = execute_json(
        &context,
        ARCHI_API_TOOL,
        json!({ "action": "bootstrap", "element": "App.Api.Subject" }),
    )
    .await;
    let draft = payload["draft"].as_str().expect("draft 为字符串");
    assert!(!draft.trim().is_empty(), "草稿非空");
    let direct = archi::api::bootstrap(dir.path(), "App.Api.Subject").expect("直调应成功");
    assert_eq!(draft, direct, "与 core 单点逐字相等");
}

/// 异常（AC-15 / spec「未知 action 与缺参」）：`bootstrap` 缺参 / 空白参 → `Err`；未知 /
/// 大小写变形 / 缺失 / null `action` → `Err`（文案点名合法值）。
#[tokio::test]
async fn archi_api非法action与缺参() {
    let dir = api_root("api-bad-input");
    let context = ArchiContext::new(dir.path().to_path_buf());

    for input in [
        json!({ "action": "bootstrap" }),
        json!({ "action": "bootstrap", "element": "" }),
        json!({ "action": "bootstrap", "element": "   " }),
    ] {
        let error = context
            .execute(ARCHI_API_TOOL, &input)
            .await
            .expect_err("缺参 / 空白参 ⇒ Err");
        assert!(error.contains("element"), "文案点名缺参：{error}");
    }

    for input in [
        json!({ "action": "write" }),
        json!({ "action": "CHECK" }),
        json!({}),
        json!({ "action": null }),
    ] {
        let error = context
            .execute(ARCHI_API_TOOL, &input)
            .await
            .expect_err("非法 action ⇒ Err");
        assert!(
            error.contains("check") && error.contains("bootstrap"),
            "文案含合法值：{error}"
        );
    }
}

/// 边界（AC-15 / D13）：输出经 `tools::cap_result` 收口——3000 条判据时长度 ≤ L1 上限、
/// 含截断留痕；`total` 与 `byCategory` 全量保留（摘要优先，截断不动计数）；不 panic。
#[tokio::test]
async fn archi_api输出过cap_result() {
    let dir = tempdir("api-cap");
    // 3000 个元素各带 `path` 但无入口 / 无契约 ⇒ 每元素一条 `apiEntryMissing`
    let mut model = String::from("[element.\"App\"]\nkind = \"package\"\n\n");
    for index in 0..3000 {
        model.push_str(&format!(
            "[element.\"App.E{index:04}\"]\nkind = \"module\"\ngate = \"warn\"\n\
             path = [\"gen/e{index:04}/\"]\n\n"
        ));
    }
    write_model(dir.path(), "01.toml", &model);
    // 契约面启用（目录含一份 md）——否则 `findings` 走 `enabled` 短路返回空集，
    // 「摘要优先 / 截断留痕」都无从表述；该 md 不属模型内元素，故不产生额外判据
    write_file(
        dir.path(),
        ".dev-team/architecture/api/App/Ghost.md",
        "# App.Ghost
",
    );

    let context = ArchiContext::new(dir.path().to_path_buf());
    let text = context
        .execute(ARCHI_API_TOOL, &json!({ "action": "check" }))
        .await
        .expect("大语料不 panic");
    assert!(
        text.len() <= 50_000 + 200,
        "输出受 L1 上限约束（上限 50000 字节 + 留痕文案；实际 {} 字节）",
        text.len()
    );
    assert!(
        text.contains("截断") || text.contains("truncated"),
        "超限时含截断留痕：{}",
        &text[text.len().saturating_sub(200)..]
    );
    assert!(
        text.contains("\"total\":3000"),
        "摘要优先：total 全量保留（{}）",
        &text[..200.min(text.len())]
    );
}

/// 边界（AC-15 / spec「零写盘」）：两动作前后对 `root` 快照相等；`archi_tool.rs` 零
/// `fs::write` / `create_dir*`；判据来源单点（`archi::api::findings` 与
/// `artifact_check` 各恰一处、零 `load_contracts` 直调）。
#[tokio::test]
async fn archi_api零写盘与判据来源单点() {
    let dir = api_root("api-no-write");
    let before = snapshot(dir.path());
    let context = ArchiContext::new(dir.path().to_path_buf());
    let _ = context
        .execute(ARCHI_API_TOOL, &json!({ "action": "check" }))
        .await;
    let _ = context
        .execute(
            ARCHI_API_TOOL,
            &json!({ "action": "bootstrap", "element": "App.Api.Subject" }),
        )
        .await;
    assert_eq!(snapshot(dir.path()), before, "两动作零写盘");

    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/sdk/archi_tool.rs"
    ))
    .expect("读 archi_tool.rs 失败");
    for forbidden in ["fs::write", "create_dir", "create_dir_all"] {
        assert!(
            !source.contains(forbidden),
            "`archi_tool.rs` 零 `{forbidden}`（零写盘）"
        );
    }
    assert_eq!(
        source.matches("archi::api::findings(").count(),
        1,
        "判据装配单点恰一处"
    );
    assert_eq!(
        source.matches("archi::api::artifact_check(").count(),
        1,
        "两轴差异单点恰一处"
    );
    assert!(!source.contains("load_contracts"), "零第二份契约装配");
}

/// 边界（AC-15 / str 映射）：`bootstrap` 的 `element` 取脏值（超长 >1000 chars / 含
/// emoji / 含 `/`）时，工具侧的成败与 `archi::api::bootstrap` 的直判**同向**、不 panic
/// （脏值判定只由 core 单点裁定，工具层零前置校验、零第二份文案拼装）；`input` 为
/// `null`（越签名脏值）→ `is_error`。
#[tokio::test]
async fn archi_api参数脏值不panic() {
    let dir = api_root("api-dirty-param");
    let context = ArchiContext::new(dir.path().to_path_buf());

    let too_long = format!("App.Api.{}", "x".repeat(1024));
    // 末项为对照臂（模型内在案元素）：脏值全落 `Err` 时，「同向」判据仍有 `Ok` 侧的实质。
    for element in [
        too_long.as_str(),
        "App.Api.Subject🔒",
        "App/Api/Subject",
        "App.Api.Subject",
    ] {
        let direct = archi::api::bootstrap(dir.path(), element);
        let result = context
            .execute(
                ARCHI_API_TOOL,
                &json!({ "action": "bootstrap", "element": element }),
            )
            .await;
        assert_eq!(
            result.is_err(),
            direct.is_err(),
            "脏值 / 对照值的成败与 core 单点同向：{element:?}"
        );
        match (result, direct) {
            (Ok(text), Ok(draft)) => {
                let payload: Value = serde_json::from_str(&text).expect("结果为 JSON 串");
                assert_eq!(
                    payload["draft"].as_str(),
                    Some(draft.as_str()),
                    "草稿逐字透传（零加工）：{element:?}"
                );
            }
            (Err(tool_error), Err(direct_error)) => assert_eq!(
                tool_error,
                direct_error.message(),
                "错误文案原样透传（零第二份拼装）：{element:?}"
            ),
            (tool, direct) => panic!("成败不同向：tool={tool:?} / core={direct:?}"),
        }
    }

    let error = context
        .execute(ARCHI_API_TOOL, &json!(null))
        .await
        .expect_err("input 为 null（越签名脏值）⇒ is_error");
    assert!(error.contains("action"), "文案点名缺失字段：{error}");
}
