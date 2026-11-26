//! `sdk::archi_tool` 的单元测试：per-turn 上下文构造、`archi_module` 两形态（指定
//! 元素 / 省略元素的薄概览）、`archi_search` 三 scope、`archi_api` 两动作（`check`
//! 两轴结构化摘要 / `bootstrap` 草稿）、坏模型 / 参数缺失 / 未知 action 的 `Err`
//! 回灌面与 L1 结果收口（30 KB 上限），以及工具串与 DTO 串的词汇等价。
//!
//! 零 mock：真实 tempdir 模型 + 真实 `archi::dto` 投影 + 真实 `tools::cap_result`
//! （30 KB 上限与留痕文案即被测契约，禁第二份上限实现）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};

use archi::dto::{ApiStatus, DiagnosticKind, ElementKind, GateTier};
use archi::parse::{SourceParser, TomlParser, ELEMENT_FIELD_NAMES};

use crate::sdk::tools::{
    ARCHI_API_TOOL, ARCHI_MODULE_TOOL, ARCHI_SCAFFOLD_TOOL, ARCHI_SEARCH_TOOL, MAX_RESULT_BYTES,
};

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

/// 落一个模型文件（`.dev-team/architecture/graph/model/`）。
fn write_model(root: &Path, name: &str, content: &str) {
    let dir = root.join(".dev-team/architecture/graph/model");
    fs::create_dir_all(&dir).expect("创建模型声明目录失败");
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
    "path = [\"./src/auth/\"]\n",
    "api = \"none\"\n",
    "doc = \"./src/auth/README.md\"\n",
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
    // 检索语料 = `path` 非空元素的在盘派生资产（两条固定规则），故落一份 `<叶段>.md`
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Auth.md",
        "# App.Auth\n\n## 职能\n\n认证域。\n",
    );
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

/// 旧立场的禁止短语（按片段拼接：产品源码面对这些整串零字面命中——AC-12a 含测试
/// 夹具面；判据仍在，禁的是「字面持有」）。
fn stale_stances() -> Vec<String> {
    vec![
        ["无专用", "写工具"].concat(),
        ["不设专用", "写工具"].concat(),
        ["写盘仍用", "既有"].concat(),
        ["写盘仍走", "既有"].concat(),
    ]
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
        json!(".dev-team/architecture/graph/model/01-core.toml")
    );
    let line = element["modelLine"].as_u64().expect("modelLine 为数字");
    assert!(line >= 1, "modelLine 可直接 edit 到声明行: {line}");

    assert_eq!(detail["dependsOn"][0]["target"], json!("App.Billing"));
    assert_eq!(detail["dependsOn"][0]["description"], json!("认证调用计费"));
    assert_eq!(detail["dependedBy"], json!([]));
    assert_eq!(detail["children"], json!([]));
    assert_eq!(detail["doc"], json!("./src/auth/README.md"));
    assert_eq!(detail["api"]["status"], json!("none"));
    assert!(
        detail.get("docs").is_none() && detail.get("warnings").is_none(),
        "旧 `docs` / `warnings` 槽位已退役：{detail}"
    );

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
            vec!["api", "fqn", "gate", "hasPath", "kind", "summary"],
            "薄概览四字段 + api + hasPath（同一模型字段的纯拷贝 + `path` 布尔投影）"
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
    assert!(!all["doc"].as_array().expect("文档数组").is_empty());

    let default_scope = execute_json(&context, ARCHI_SEARCH_TOOL, json!({ "query": "auth" })).await;
    assert_eq!(default_scope, all, "scope 缺省按 all");

    let elements = execute_json(
        &context,
        ARCHI_SEARCH_TOOL,
        json!({ "query": "auth", "scope": "elements" }),
    )
    .await;
    assert_eq!(elements["doc"], json!([]), "elements 档 doc 为空数组");

    let docs = execute_json(
        &context,
        ARCHI_SEARCH_TOOL,
        json!({ "query": "auth", "scope": "doc" }),
    )
    .await;
    assert_eq!(docs["elements"], json!([]), "doc 档 elements 为空数组");
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
    for legal in ["elements", "doc", "all"] {
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
            "[element.\"App.Orphan.Child\"]\nkind = \"module\"\n\n",
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
        ".dev-team/architecture/models/App/Api/Subject.md",
        "# App.Api.Subject\n\n## 职能\n\n工具语料。\n",
    );
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Api/Subject.schema.json",
        concat!(
            "{\n",
            "  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n",
            "  \"$id\": \"App.Api.Subject.schema.json\",\n",
            "  \"title\": \"App.Api.Subject\",\n",
            "  \"$defs\": {},\n",
            "  \"properties\": {\n",
            "    \"file_stem\": {\n",
            "      \"x-function\": {\n",
            "        \"owner\": \"LocalApi\",\n",
            "        \"params\": [{ \"name\": \"timestamp\", \"schema\": { \"type\": \"string\" } }],\n",
            "        \"returns\": { \"type\": \"integer\" }\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
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
    // 仅判 `ends_with("INDEX.toml")` 钉不住新索引家（旧 `api/` 落点同样通过），故以
    // 「父目录末段 = graph」收口；Windows 的 `\` 先归一为 `/` 再判，两平台同判。
    let index_root = |path: &str| path.replace('\\', "/").ends_with("graph/INDEX.toml");
    assert!(
        missing[0].as_str().is_some_and(index_root),
        "缺口是 graph 根下的索引（父目录末段 = graph）：{missing:?}"
    );
}

/// 正向（AC-15 / D13）：`bootstrap` 出**双草稿** `{ schema, doc }`，两段各自与
/// `archi::api::bootstrap` 直出逐字相等（工具出口与 CLI 出口同源）。
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
    let schema = payload["schema"].as_str().expect("schema 为字符串");
    let doc = payload["doc"].as_str().expect("doc 为字符串");
    assert!(!schema.trim().is_empty(), "接口草稿非空");
    assert!(!doc.trim().is_empty(), "文档草稿非空");
    let direct = archi::api::bootstrap(dir.path(), "App.Api.Subject").expect("直调应成功");
    assert_eq!(schema, direct.schema, "接口草稿与 core 单点逐字相等");
    assert_eq!(doc, direct.doc, "文档草稿与 core 单点逐字相等");
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
            error.contains("check") && error.contains("bootstrap") && error.contains("link"),
            "文案并列三个合法值：{error}"
        );
    }
}

/// 边界（AC-15 / D13）：输出经 `tools::cap_result` 收口——3000 条判据时长度 ≤ L1 上限、
/// 含截断留痕；`total` 与 `byCategory` 全量保留（摘要优先，截断不动计数）；不 panic。
#[tokio::test]
async fn archi_api输出过cap_result() {
    let dir = tempdir("api-cap");
    // 3000 个元素各带 `path` 但无载体 ⇒ 每元素一条 `docMissing`（另加一个载体元素）
    let mut model = String::from(
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.Carrier\"]\nkind = \"module\"\n\
         path = [\"guides/carrier/\"]\n\n",
    );
    for index in 0..3000 {
        model.push_str(&format!(
            "[element.\"App.E{index:04}\"]\nkind = \"module\"\ngate = \"warn\"\n\
             path = [\"gen/e{index:04}/\"]\n\n"
        ));
    }
    write_model(dir.path(), "01.toml", &model);
    // 接口面启用需要一份 declared 元素的 `<叶段>.schema.json` 在盘；其余元素的
    // `docMissing`（派生 md 不在盘）即规模面的 3000 条判据来源
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Carrier.schema.json",
        "{\"$id\": \"App.Carrier.schema.json\", \"title\": \"App.Carrier\", \"$defs\": {}, \"properties\": {}}\n",
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
        text.contains("\"total\":3001"),
        "摘要优先：total 全量保留（3000 元素 + 启用证据元素各一条 docMissing）（{}）",
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
    // 模块头的禁令句（「MUST NOT 出现第二份生成物 `fs::write`」）是文档、不是写盘，先滤
    // 注释行再判代码面（代码面仍须零命中）
    let code = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["fs::write", "create_dir", "create_dir_all"] {
        assert!(
            !code.contains(forbidden),
            "`archi_tool.rs` 代码面零 `{forbidden}`（零写盘）"
        );
    }
    assert!(
        source.contains("fs::write"),
        "禁令句仍以文档形态在案（判据是代码面零命中，不是删句）"
    );
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
            (Ok(text), Ok(drafts)) => {
                let payload: Value = serde_json::from_str(&text).expect("结果为 JSON 串");
                assert_eq!(
                    payload["schema"].as_str(),
                    Some(drafts.schema.as_str()),
                    "接口草稿逐字透传（零加工）：{element:?}"
                );
                assert_eq!(
                    payload["doc"].as_str(),
                    Some(drafts.doc.as_str()),
                    "文档草稿逐字透传（零加工）：{element:?}"
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

// ===========================================================================
// `doc` 单值响应形状（`archi_module` / `archi_search` 消费侧）
// ===========================================================================

/// 留白 + 声明的两元素夹具（`App.Auth` 写 `doc`，`App.Billing` 为 `declared` 却写了 `doc`）。
fn doc_shape_dir(tag: &str) -> tempfile::TempDir {
    let dir = tempdir(tag);
    write_model(
        dir.path(),
        "01-core.toml",
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"domain\"\n",
            "api = \"none\"\n",
            "path = [\"./src/auth/\"]\n",
            "doc = \"./src/auth/README.md\"\n",
            "\n",
            "[element.\"App.Billing\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/billing/\"]\n",
            "doc = \"./src/billing/README.md\"\n",
            "\n",
            "[[relation]]\n",
            "from = \"App.Auth\"\n",
            "to = \"App.Billing\"\n",
            "desc = \"认证调用计费\"\n",
        ),
    );
    write_file(dir.path(), "src/auth/README.md", "# 认证说明\n\n正文\n");
    write_file(dir.path(), "src/billing/README.md", "# 计费说明\n\n正文\n");
    // 检索语料 = `path` 非空元素的在盘派生文档（`<叶段>.md`）
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Auth.md",
        "# App.Auth\n\n## 职能\n\n认证域。\n",
    );
    dir
}

/// 正向：`api = none` 的元素出模型原文（`./` 不剥）且与元素投影同源；`declared` 元素出
/// `null` 而元素投影保留原文（判据 ⑥ 的取数路不清零）。
#[tokio::test]
async fn archi_module响应doc单值三组同源() {
    let dir = doc_shape_dir("doc-shape");
    let context = ArchiContext::new(dir.path().to_path_buf());

    let blank = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Auth" }),
    )
    .await;
    assert_eq!(blank["doc"], json!("./src/auth/README.md"), "留白出原文");
    assert_eq!(
        blank["doc"], blank["element"]["doc"],
        "段与元素投影同源（不漂移）"
    );

    let declared = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Billing" }),
    )
    .await;
    assert_eq!(
        declared["doc"],
        json!("./src/billing/README.md"),
        "`declared` 也照原文出（`doc` 是零判据读者）"
    );
    assert_eq!(
        declared["doc"], declared["element"]["doc"],
        "段与元素投影同源（零抹平）"
    );
}

/// 异常：响应零 `docs` / 零 `warnings` 键；`doc` 为 `null` 时键仍显式在场。
#[tokio::test]
async fn archi_module响应doc键面零旧槽位() {
    let dir = doc_shape_dir("doc-keys");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let declared = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Billing" }),
    )
    .await;
    let keys: Vec<&str> = declared
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert!(keys.contains(&"doc"), "`doc` 键显式在场：{keys:?}");
    assert!(!keys.contains(&"docs"), "零 `docs` 键：{keys:?}");
    assert!(!keys.contains(&"warnings"), "零 `warnings` 键：{keys:?}");

    let element_keys: Vec<&str> = declared["element"]
        .as_object()
        .expect("对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert!(
        element_keys.contains(&"doc"),
        "元素投影的 `doc` 键不省略：{element_keys:?}"
    );
}

/// 边界：省略 `element` 的薄概览零 `doc` 字段；`archi_search` 的 `doc` 条目为路径串而非
/// 四字段对象；`scope = "docs"` 非法且文案列三合法值；`scope = "doc"` 时元素侧空。
#[tokio::test]
async fn archi_tool_doc边界与scope异常() {
    let dir = doc_shape_dir("doc-bounds");
    let context = ArchiContext::new(dir.path().to_path_buf());

    let overview = execute_json(&context, ARCHI_MODULE_TOOL, json!({})).await;
    for brief in overview.as_array().expect("数组") {
        assert!(brief.get("doc").is_none(), "薄概览零 `doc` 字段：{brief}");
    }

    let hit = execute_json(&context, ARCHI_SEARCH_TOOL, json!({ "query": "auth" })).await;
    let docs = hit["doc"].as_array().expect("doc 数组");
    assert!(!docs.is_empty(), "语料非空：{hit}");
    for entry in docs {
        assert!(entry.is_string(), "条目为路径串而非对象：{entry}");
    }

    let doc_scope = execute_json(
        &context,
        ARCHI_SEARCH_TOOL,
        json!({ "query": "auth", "scope": "doc" }),
    )
    .await;
    assert_eq!(doc_scope["elements"], json!([]), "doc 档元素侧空");

    let error = context
        .execute(
            ARCHI_SEARCH_TOOL,
            &json!({ "query": "auth", "scope": "docs" }),
        )
        .await
        .expect_err("旧词 docs 应 Err");
    for legal in ["elements", "doc", "all"] {
        assert!(error.contains(legal), "文案列合法值 {legal}: {error}");
    }
}

// ===========================================================================
// `archi_scaffold`：新元素 TOML 草稿（只读零写盘）
// ===========================================================================

/// 正向：省略 `kind` → `{ "draft": <TOML> }`，草稿含 `kind = "domain"` 实值行、
/// 字段序注释（`gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`）、
/// `metadata` 子表与内联表等价的纪律行与 `path` 基数提示。
#[tokio::test]
async fn archi_scaffold省略kind取domain且字段序齐备() {
    let dir = healthy("scaffold-default");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("响应为 { draft: <TOML> }");
    assert!(
        draft.contains("kind = \"domain\""),
        "省略 kind 取 domain 实值行：{draft}"
    );
    for piece in [
        "# gate",
        "# api",
        "# entry",
        "# path",
        "# doc",
        "# summary",
        "# metadata",
        "子表与内联表等价",
        "module 必填",
    ] {
        assert!(draft.contains(piece), "草稿含字段序/纪律线索 `{piece}`");
    }
}

/// 正向（表驱动）：四种 `kind` 逐值（经 `ElementKind::ALL` 派生，不手抄字面量）→
/// 草稿实值行逐值对应且零 `__KIND__` 残留。
#[tokio::test]
async fn archi_scaffold四kind逐值() {
    let dir = healthy("scaffold-kinds");
    let context = ArchiContext::new(dir.path().to_path_buf());
    for kind in ElementKind::ALL {
        let value = execute_json(
            &context,
            crate::sdk::tools::ARCHI_SCAFFOLD_TOOL,
            json!({ "kind": kind.as_str() }),
        )
        .await;
        let draft = value["draft"].as_str().expect("草稿为字符串");
        assert!(
            draft.contains(&format!("kind = \"{}\"", kind.as_str())),
            "{} 实值行逐值对应：{draft}",
            kind.as_str()
        );
        assert!(
            !draft.contains("__KIND__"),
            "占位符已替换（零残留）：{draft}"
        );
    }
}

/// 正向（表驱动）：模板的 `kind` 行四值语义逐组在场——语义词随 `ElementKind::ALL`
/// 派生，新增变体未补语义即红。
#[tokio::test]
async fn archi_scaffold模板kind行四值语义() {
    let dir = healthy("scaffold-kind-semantics");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");
    for kind in ElementKind::ALL {
        let meaning = match kind {
            ElementKind::Package => "至多一条包根",
            ElementKind::Domain => "领域容器",
            ElementKind::Module => "模块",
            ElementKind::Composite => "纯分组",
        };
        assert!(
            draft.contains(meaning),
            "kind 语义 `{meaning}`（{}）在场：{draft}",
            kind.as_str()
        );
    }
    assert!(
        draft.contains("纯分层容器是标准形态"),
        "`composite` 的标准形态句在场"
    );
}

/// 边界：模板的 `path` 行给出两句出路——`module` 必填（可多条）与 `package` 纯容器可
/// 留空，且写明「≥2 条即模型停摆」，与 `TomlParser` 的基数判据口径一致。
#[tokio::test]
async fn archi_scaffold模板path行出路() {
    let dir = healthy("scaffold-path-cardinality");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");
    let line = draft
        .lines()
        .find(|line| line.starts_with("# path："))
        .unwrap_or_else(|| panic!("草稿缺 `path` 纪律行：{draft}"));
    for needle in ["module 必填", "可多条", "纯容器可留空", "≥2 条即模型停摆"] {
        assert!(line.contains(needle), "`path` 行含 `{needle}`：{line}");
    }
    assert!(
        !line.contains("容器（package / domain）恒空"),
        "旧的「容器恒空」口径 MUST NOT 残留：{line}"
    );
}

/// 异常（表驱动）：草稿零结构诊断词（与骨架 `md不列结构诊断全量词表` 同源纪律）——
/// `kind` / `path` / `metadata` 三行改写不得引入诊断词。
#[tokio::test]
async fn archi_scaffold草稿零结构诊断词() {
    let dir = healthy("scaffold-no-diagnostic-words");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");
    for word in [
        "invalidValue",
        "unknownField",
        "missingField",
        "syntaxError",
    ] {
        assert!(!draft.contains(word), "草稿零诊断词 `{word}`：{draft}");
    }
}

/// 边界：草稿经真实模型解析零结构诊断（单段占位 FQN、关系块全注释）——把三种 kind
/// 的草稿各落成盘上唯一模型文件后，概览应 `Ok` 且恰一元素、`path` 仍是注释。
#[tokio::test]
async fn archi_scaffold草稿零结构诊断() {
    let source = healthy("scaffold-parse");
    let context = ArchiContext::new(source.path().to_path_buf());
    for kind in [None, Some("package"), Some("composite")] {
        let input = match kind {
            Some(kind) => json!({ "kind": kind }),
            None => json!({}),
        };
        let value = execute_json(&context, crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, input).await;
        let draft = value["draft"].as_str().expect("草稿为字符串").to_owned();

        let target = tempdir(&format!(
            "scaffold-parse-target-{}",
            kind.unwrap_or("default")
        ));
        write_model(target.path(), "01-draft.toml", &draft);
        let briefs = archi::dto::module_overview(target.path())
            .unwrap_or_else(|error| panic!("{kind:?} 草稿应零结构诊断：{error}"));
        assert_eq!(briefs.len(), 1, "{kind:?} 草稿恰一声明（单段占位 FQN）");
        assert!(!briefs[0].has_path, "{kind:?} 草稿的 path 为注释（未声明）");
    }
}

/// 边界：关系部分全为注释行（零裸 `[[relation]]` 表，避免 `missingField`），四键齐备
/// ——`apis` 行与其选填纪律在场（形态教学单源：模型语料里零 `apis`，模板是唯一教学点）。
#[tokio::test]
async fn archi_scaffold关系块全注释() {
    let dir = healthy("scaffold-relation");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");
    assert!(draft.contains("# [[relation]]"), "关系模板以注释给出");
    for key in ["# from = ", "# to = ", "# desc = ", "# apis = "] {
        assert!(
            draft.contains(key),
            "关系四键之一以注释在场 `{key}`：{draft}"
        );
    }
    for needle in ["只写名字", "可留空", "生成物面留空", "archi_drift"] {
        assert!(draft.contains(needle), "`apis` 选填纪律 `{needle}` 在场");
    }
    assert!(
        !draft
            .lines()
            .any(|line| line.trim_start().starts_with("[[relation]]")),
        "零裸 relation 表：{draft}"
    );
}

/// 边界：零写盘——调用前后盘面字节快照逐一相同。
#[tokio::test]
async fn archi_scaffold零写盘() {
    let dir = healthy("scaffold-no-write");
    let before = snapshot(dir.path());
    let context = ArchiContext::new(dir.path().to_path_buf());
    execute_json(&context, crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, json!({})).await;
    assert_eq!(before, snapshot(dir.path()), "只读草稿零写盘");
}

/// 边界：多字节字符安全——草稿正文含 `→` / `≥` / `——`（字符数 < 字节数）时，行首定位
/// 与块取仍按字节安全落到同一行（零 panic），行数按 `\n` 计数不失真、逐行字节数不为
/// 多字节字符吞掉半截。
#[tokio::test]
async fn archi_scaffold草稿多字节边界安全() {
    let dir = healthy("scaffold-multibyte");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");

    for needle in ["→", "≥", "——"] {
        assert!(
            draft.contains(needle),
            "草稿含多字节字符 `{needle}`（字符数 < 字节数的前提在于场）：{draft}"
        );
    }
    let chars = draft.chars().count();
    assert!(
        chars < draft.len(),
        "草稿 {chars} 字符 / {} 字节——多字节确实在场，后续行首 / 块取走的是字节偏移",
        draft.len()
    );

    assert_eq!(
        draft.lines().count(),
        draft.matches('\n').count(),
        "行数按 `\\n` 计数一致（草稿以换行收尾，多字节不额外切行）"
    );
    let accounted: usize = draft.lines().map(str::len).sum::<usize>() + draft.matches('\n').count();
    assert_eq!(
        accounted,
        draft.len(),
        "逐行字节数 + 换行数 = 全文字节（多字节字符未被截半、行数未失真）"
    );
    let metadata_line =
        line_starting_with(draft, "# metadata：").expect("多字节行的行首定位不因 `→` / `——` 失效");
    assert!(
        metadata_line.contains('→') && metadata_line.contains("——"),
        "`→` / `——` 落在 `# metadata：` 行内：{metadata_line}"
    );
    let path_line = line_starting_with(draft, "# path：").expect("多字节行的行首定位不因 `≥` 失效");
    assert!(
        path_line.contains('≥'),
        "`≥` 落在 `# path：` 行内：{path_line}"
    );

    for line in [metadata_line, path_line] {
        assert!(
            line.len() > line.chars().count(),
            "该行字节数 > 字符数（`：` 与 `→` / `≥` / `——` 均多字节）：{line}"
        );
        assert!(
            line.starts_with("# "),
            "含多字节全角冒号的行首前缀仍可逐字匹配：{line}"
        );
    }

    for field in ["doc", "summary", "metadata"] {
        assert!(
            line_starting_with(draft, &format!("# {field}：")).is_some(),
            "多字节行仍在其前的字段 `{field}` 行首判据成立（字节偏移不漂移）"
        );
    }
    let metadata_block = block_starting_with(draft, "# metadata：");
    assert!(
        metadata_block.lines().count() >= 2,
        "块取跨多字节行不截断：{metadata_block}"
    );
    for needle in ["值 MUST 是字符串", "子表与内联表等价"] {
        assert!(
            metadata_block.contains(needle),
            "块内多字节行之后的内容仍在场 `{needle}`：{metadata_block}"
        );
    }
}

/// 异常：解析判据非空转——草稿里被解注释的样例行必须让真实 `TomlParser` 数出诊断
/// （只判「`unwrap` 成功」不数诊断的实现会漏判模型停摆）。
///
/// 两条 `# path = ` 样例行各是**单元素数组**：同时解注释即 TOML 同键重复，document 层
/// 即拒（`duplicate key` → `duplicateElement`），走不到 `path` 的基数判据；故基数面按
/// 「两份样例并成一条数组」构造——值逐字取自草稿（零手抄），即成 `package` 的两条包根
/// → 恰一条 `invalidValue`。
#[tokio::test]
async fn archi_scaffold草稿解注释path被真实解析拒() {
    let dir = healthy("scaffold-path-uncomment");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, ARCHI_SCAFFOLD_TOOL, json!({ "kind": "package" })).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");

    let clean = TomlParser::parse(draft);
    assert!(
        clean.diagnostics.is_empty(),
        "对照：`package` 草稿原样零诊断：{:?}",
        clean.diagnostics
    );

    let path_examples: Vec<&str> = draft
        .lines()
        .filter(|line| line.starts_with("# path = "))
        .collect();
    assert_eq!(
        path_examples.len(),
        2,
        "模板给两条 `# path = ` 样例行：{path_examples:?}"
    );

    let both = draft
        .lines()
        .map(|line| match line.strip_prefix("# path = ") {
            Some(rest) => format!("path = {rest}"),
            None => line.to_owned(),
        })
        .collect::<Vec<String>>()
        .join("\n");
    let outcome = TomlParser::parse(&both);
    assert_eq!(
        outcome.diagnostics.len(),
        1,
        "两条 `path` 同时解注释 → 恰一条诊断（`unwrap` 成功 ≠ 零诊断）：{:?}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.diagnostics[0].kind,
        DiagnosticKind::DuplicateElement,
        "同键重复在 document 层即被拒（走不到基数判据）：{:?}",
        outcome.diagnostics[0]
    );

    let values: Vec<String> = path_examples
        .iter()
        .map(|line| {
            line.trim_start_matches("# path = ")
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_owned()
        })
        .collect();
    let without_examples = draft
        .lines()
        .filter(|line| !line.starts_with("# path = "))
        .collect::<Vec<&str>>()
        .join("\n");
    let candidate = format!("{without_examples}\npath = [{}]\n", values.join(", "));
    let outcome = TomlParser::parse(&candidate);
    assert_eq!(
        outcome.diagnostics.len(),
        1,
        "`package` 两条包根 → 恰一条诊断（零铺开）：{:?}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.diagnostics[0].kind,
        DiagnosticKind::InvalidValue,
        "包根越界是值域违例：{:?}",
        outcome.diagnostics[0]
    );
    assert!(
        outcome.diagnostics[0].message.contains("至多一条包根"),
        "诊断落的是 `validate_path` 而非泛化解析失败：{}",
        outcome.diagnostics[0].message
    );
    assert_eq!(
        outcome.model.elements.len(),
        1,
        "元素照常入模型、诊断照常累积"
    );
    assert_eq!(
        outcome.model.elements[0].path.len(),
        2,
        "两条包根逐条入模型：{:?}",
        outcome.model.elements[0].path
    );
}

/// 异常（表驱动）：`kind` 非法**串**（`packagex` / 大小写变体 `Domain`）→ `Err` 文案
/// 点名四值（词表经 `ElementKind::ALL` 派生）；空串 / `null` / 非字符串按「省略」处置
/// （取 `domain`），零静默退化。
#[tokio::test]
async fn archi_scaffold非法kind报错点名四值() {
    let dir = healthy("scaffold-invalid-kind");
    let context = ArchiContext::new(dir.path().to_path_buf());
    for input in [json!({ "kind": "packagex" }), json!({ "kind": "Domain" })] {
        let error = context
            .execute(crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, &input)
            .await
            .expect_err("值集外应 Err");
        for kind in ElementKind::ALL {
            assert!(
                error.contains(kind.as_str()),
                "Err 文案点名 {}：{error}",
                kind.as_str()
            );
        }
    }
    // 空串 / null / 非字符串视为省略（缺省 domain），不报错
    for input in [
        json!({ "kind": "" }),
        json!({ "kind": null }),
        json!({ "kind": 42 }),
    ] {
        let text = context
            .execute(crate::sdk::tools::ARCHI_SCAFFOLD_TOOL, &input)
            .await
            .expect("省略形态应 Ok");
        assert!(text.contains("domain"), "省略取 domain：{text}");
    }
}

// ===========================================================================
// 本变更：`archi_scaffold` 草稿即字段词典（字段面全集锚 / 值域派生 / 值面纪律）
//
// 判据一律按**行首 / 块取**设计：`contains(字段名)` 在 `apis` 顶替 `api` 时假命中，故
// 字段面与行内限定的断言只用行首定位；词表从真实常量派生（`ELEMENT_FIELD_NAMES` /
// `GateTier::ALL` / `ApiStatus::ALL` / `default()`），测试内零手抄字段名与值。
// ===========================================================================

/// 行首定位：首个以 `prefix` 开头的行（零 `contains` 假命中）。
fn line_starting_with<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines().find(|line| line.starts_with(prefix))
}

/// 块取：从首个以 `prefix` 开头的行起，到其后首个空行止（右开边界）；无后继空行即取到
/// 文末（末行块不 panic、不截断）。
fn block_starting_with(text: &str, prefix: &str) -> String {
    let mut block: Vec<&str> = Vec::new();
    let mut started = false;
    for line in text.lines() {
        if !started {
            started = line.starts_with(prefix);
            if !started {
                continue;
            }
        } else if line.trim().is_empty() {
            break;
        }
        block.push(line);
    }
    assert!(started, "草稿缺以 `{prefix}` 开头的行");
    block.join("\n")
}

/// 行首载体计数（三形态：实值行 / 冒号纪律行 / 赋值示例行）。
fn carrier_count(text: &str, field: &str) -> usize {
    let forms = [
        format!("{field} = "),
        format!("# {field}："),
        format!("# {field} = "),
    ];
    text.lines()
        .filter(|line| forms.iter().any(|form| line.starts_with(form.as_str())))
        .count()
}

/// 未注释赋值行的键清单（非 `#` 起始且含 ` = `）。
fn uncommented_assignment_keys(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_once(" = "))
        .map(|(key, _)| key.trim().to_owned())
        .collect()
}

/// 正向（表驱动，AC-2）：草稿的字段面全集锚——遍历 `ELEMENT_FIELD_NAMES`（测试内零手抄
/// 八字面量），逐字段断言三形态并集 + `# {f}：` **行首**在场（第一反向锚）+ 唯一未注释
/// 赋值行的键属字段面（第二反向锚）+ 行首载体数；四 kind 逐值恒成立。
#[tokio::test]
async fn archi_scaffold草稿含字段面全集() {
    let dir = healthy("scaffold-field-names");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("响应为 { draft: <TOML> }");

    for field in ELEMENT_FIELD_NAMES {
        let forms = [
            format!("{field} = "),
            format!("# {field}："),
            format!("# {field} = "),
        ];
        assert!(
            forms.iter().any(|form| draft.contains(form.as_str())),
            "字段 `{field}` 以三形态之一在场：{draft}"
        );
        assert!(
            line_starting_with(draft, &format!("# {field}：")).is_some(),
            "字段 `{field}` 的纪律行以行首形态在场（`contains` 会假命中）：{draft}"
        );
    }

    let keys = uncommented_assignment_keys(draft);
    assert_eq!(keys.len(), 1, "草稿内未注释赋值行恰一条：{keys:?}");
    assert!(
        ELEMENT_FIELD_NAMES.contains(&keys[0].as_str()),
        "唯一实值行的键属字段面：{keys:?}"
    );

    for field in ELEMENT_FIELD_NAMES {
        let expected = if field == "path" { 3 } else { 2 };
        assert_eq!(
            carrier_count(draft, field),
            expected,
            "字段 `{field}` 的行首载体数（`path` 3 / `kind` 2 / 其余各 2）——判据按存在而非恰一"
        );
    }

    for kind in ElementKind::ALL {
        let value = execute_json(
            &context,
            ARCHI_SCAFFOLD_TOOL,
            json!({ "kind": kind.as_str() }),
        )
        .await;
        let draft = value["draft"].as_str().expect("草稿为字符串");
        for field in ELEMENT_FIELD_NAMES {
            assert!(
                line_starting_with(draft, &format!("# {field}：")).is_some(),
                "{} 草稿含 `{field}` 纪律行（词典正文与 kind 值无关）",
                kind.as_str()
            );
        }
        assert_eq!(
            uncommented_assignment_keys(draft).len(),
            1,
            "{} 草稿唯一实值行（`__KIND__` 单点替换不动纪律行）",
            kind.as_str()
        );
    }
}

/// 异常（AC-2 反向可判）：删 `# api：` 行 → 行首判据为假（`# apis：` 在场不顶替）；删
/// `kind = "__KIND__"` 行 → 唯一实值行判据为假（0 条 ≠ 1）。
#[tokio::test]
async fn archi_scaffold草稿字段面反向锚可判() {
    let dir = healthy("scaffold-field-negative");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");

    let without_api: String = draft
        .lines()
        .filter(|line| !line.starts_with("# api："))
        .collect::<Vec<&str>>()
        .join("\n");
    assert!(
        line_starting_with(draft, "# api：").is_some(),
        "对照：原草稿的 `api` 纪律行判据为真"
    );
    assert!(
        without_api.contains("# apis："),
        "前提：`apis` 纪律行仍在场"
    );
    assert!(
        line_starting_with(&without_api, "# api：").is_none(),
        "`# apis：` 不顶替 `# api：`（假命中反例）"
    );

    let without_kind: String = draft
        .lines()
        .filter(|line| !line.starts_with("kind = "))
        .collect::<Vec<&str>>()
        .join("\n");
    assert!(
        uncommented_assignment_keys(&without_kind).is_empty(),
        "实值行缺失 → 0 条（≠ 1，删整行形态亦判红）"
    );
}

/// 正向（AC-4）：值域派生——`# gate：` 行内包含 `GateTier::ALL` 三值与缺省档（由
/// `Default` 派生，零手抄 `off`），`# api：` 行内包含 `ApiStatus::ALL` 二值与缺省档；行内
/// 限定防假命中（`declared` 亦出现在 `# doc：` 行）；草稿零结构诊断词。
#[tokio::test]
async fn archi_scaffold模板值域逐值且零诊断词() {
    let dir = healthy("scaffold-value-domain");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");

    let gate_line = line_starting_with(draft, "# gate：").expect("`gate` 纪律行在场");
    for tier in GateTier::ALL {
        assert!(
            gate_line.contains(tier.as_str()),
            "`gate` 行含值 {}（三值全在场）：{gate_line}",
            tier.as_str()
        );
    }
    assert!(
        gate_line.contains(&format!("缺省 {}", GateTier::default().as_str())),
        "缺省档从 `Default` 派生：{gate_line}"
    );

    let api_line = line_starting_with(draft, "# api：").expect("`api` 纪律行在场");
    for status in ApiStatus::ALL {
        assert!(
            api_line.contains(status.as_str()),
            "`api` 行含值 {}（二值全在场）：{api_line}",
            status.as_str()
        );
    }
    assert!(
        api_line.contains(&format!("缺省 {}", ApiStatus::default().as_str())),
        "缺省档从 `Default` 派生：{api_line}"
    );

    let doc_line = line_starting_with(draft, "# doc：").expect("`doc` 纪律行在场");
    let false_hit_word = ApiStatus::None.as_str();
    assert!(
        doc_line.contains(false_hit_word),
        "前提：`api = none` 措辞亦出现在 `# doc：` 行（全串 contains 会假命中）：{doc_line}"
    );
    let fragment = "# doc：约定字段（零判据读者）：常规只有 api = none 的元素会写\n";
    assert!(
        fragment.contains(false_hit_word),
        "构造片段的全串 `contains` 会命中 `doc` 行"
    );
    assert!(
        !line_starting_with(fragment, "# api：").is_some_and(|line| line.contains(false_hit_word)),
        "行首定位不把 `# doc：` 行算作 `# api：` 行判据（行内限定防假命中）"
    );

    for word in [
        "invalidValue",
        "unknownField",
        "missingField",
        "syntaxError",
    ] {
        assert!(
            !draft.contains(word),
            "草稿零结构诊断词 `{word}`（新文本只写行为）"
        );
    }
}

/// 正向（AC-1）：`entry` 单值纪律、`metadata` 块三 needle、`apis` 块八 needle——块一律按
/// 「`# {f}：` 行到下一空行」取（续行扩写免疫，单行断法必假）；末行块取到文末零截断；
/// `entry` 行内限定反例。
#[tokio::test]
async fn archi_scaffold模板值面纪律四条() {
    let dir = healthy("scaffold-value-face");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, ARCHI_SCAFFOLD_TOOL, json!({})).await;
    let draft = value["draft"].as_str().expect("草稿为字符串");

    let entry_line = line_starting_with(draft, "# entry：").expect("`entry` 纪律行在场");
    assert!(
        entry_line.contains("单值"),
        "`entry` 的单值语义在场（`Option<String>`）：{entry_line}"
    );

    let metadata_block = block_starting_with(draft, "# metadata：");
    let metadata_head = line_starting_with(draft, "# metadata：").expect("`metadata` 首行");
    for needle in ["值 MUST 是字符串", "非字符串值被拒", "子表与内联表等价"] {
        assert!(
            metadata_block.contains(needle),
            "`metadata` 块含 `{needle}`：{metadata_block}"
        );
    }
    assert!(
        metadata_block.lines().count() >= 2,
        "`metadata` 块已扩为多行（事实校准）：{metadata_block}"
    );
    assert!(
        !metadata_head.contains("子表与内联表等价"),
        "该 needle 落在续行（单行断法必假）：{metadata_head}"
    );

    let apis_block = block_starting_with(draft, "# apis：");
    let apis_head = line_starting_with(draft, "# apis：").expect("`apis` 首行");
    for needle in [
        "只写名字",
        "可留空",
        "逐项非空",
        "同名不重复",
        "显式空数组被拒",
        "生成物面留空",
        "archi_drift",
        "`apis = []` 与省略同义",
    ] {
        assert!(
            apis_block.contains(needle),
            "`apis` 块含 `{needle}`：{apis_block}"
        );
    }
    for needle in [
        "逐项非空",
        "同名不重复",
        "显式空数组被拒",
        "生成物面留空",
        "archi_drift",
        "`apis = []` 与省略同义",
    ] {
        assert!(
            !apis_head.contains(needle),
            "`{needle}` 在续行（块取法扩张免疫）：{apis_head}"
        );
    }
    assert!(
        apis_block
            .lines()
            .last()
            .unwrap_or_default()
            .contains("archi_drift"),
        "`apis` 块取到文末（无后继空行、零截断）：{apis_block}"
    );
    let tail = block_starting_with("# apis：末行块\n# 续行无空行收尾", "# apis：");
    assert_eq!(tail.lines().count(), 2, "无后继空行即取到文末");

    let fragment = "# doc：单值指针\n";
    assert!(fragment.contains("单值"), "全串 `contains` 会命中 `doc` 行");
    assert!(
        !line_starting_with(fragment, "# entry：").is_some_and(|line| line.contains("单值")),
        "`# doc：单值指针` 不顶替 `entry`（R-3 反例）"
    );
}

/// 边界（AC-5）：响应体量面——`cap_result` 未截断、响应单键 `{ draft }`、草稿 < 8 000 B
/// 且 ≤ 45 行（R-5 体量护栏；精确读数属桌面端账目）、转义增长后仍 ≪ 上限。
#[tokio::test]
async fn archi_scaffold响应体量与未截断() {
    let dir = healthy("scaffold-size");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let text = context
        .execute(ARCHI_SCAFFOLD_TOOL, &json!({}))
        .await
        .expect("省略 kind 应成功");

    assert!(
        !text.contains("已截断"),
        "`cap_result` 未截断（截断即纪律行被切 → 词典静默残废）"
    );
    assert!(
        text.len() < MAX_RESULT_BYTES,
        "响应 {} B ≪ 上限 {MAX_RESULT_BYTES} B",
        text.len()
    );

    let value: Value = serde_json::from_str(&text).expect("响应为 JSON 串");
    let keys: Vec<&str> = value
        .as_object()
        .expect("响应为对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["draft"], "响应单键（零第二键）");

    let draft = value["draft"].as_str().expect("草稿为字符串");
    assert!(draft.len() < 8_000, "草稿 {} B < 8000 B", draft.len());
    assert!(
        draft.lines().count() <= 45,
        "草稿 {} 行 ≤ 45 行",
        draft.lines().count()
    );

    let escaped = serde_json::to_string(&value).expect("重序列化成功");
    assert!(
        escaped.len() < MAX_RESULT_BYTES,
        "转义增长后 {} B 仍 ≪ 上限（词典扩容不触顶）",
        escaped.len()
    );
}

/// 组合（AC-2 / AC-4 端到端）：词典链路——模板 → `scaffold_draft` → `scaffold_payload`
/// → `serde_json` → `cap_result`：响应文本对八字段逐项行首对齐（一次读全，零二次点查）；
/// 样例可直接抄改（`gate` 逐值解注释后经真实 `TomlParser` 零诊断且值入模型）；`apis`
/// 纪律与 `validate_apis` 判据同向。
#[tokio::test]
async fn archi_scaffold词典链路端到端() {
    let dir = healthy("scaffold-dictionary-chain");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let text = context
        .execute(ARCHI_SCAFFOLD_TOOL, &json!({}))
        .await
        .expect("省略 kind 应成功");
    let value: Value = serde_json::from_str(&text).expect("响应为 JSON 串");
    let draft = value["draft"].as_str().expect("响应为 { draft: <TOML> }");

    for field in ELEMENT_FIELD_NAMES {
        assert!(
            text.contains(&format!("# {field}：")),
            "全链路输出含 `{field}` 纪律行（词典正文未被序列化抹掉）"
        );
    }

    let gate_example = line_starting_with(draft, "# gate = ")
        .expect("`gate` 样例行在场")
        .to_owned();
    for tier in GateTier::ALL {
        let candidate = draft.replace(&gate_example, &format!("gate = \"{}\"", tier.as_str()));
        let outcome = TomlParser::parse(&candidate);
        assert!(
            outcome.diagnostics.is_empty(),
            "gate = {} 的草稿零诊断：{:?}",
            tier.as_str(),
            outcome.diagnostics
        );
        assert_eq!(
            outcome.model.elements.len(),
            1,
            "草稿恰一元素（单段占位 FQN）"
        );
        assert_eq!(
            outcome.model.elements[0].gate,
            tier,
            "词典给的值域样例可直用（{} 入模型）",
            tier.as_str()
        );
    }

    let apis_example = line_starting_with(draft, "# apis = ")
        .expect("`apis` 样例行在场")
        .trim_start_matches("# ")
        .to_owned();
    let source =
        format!("[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\n{apis_example}\n");
    let outcome = TomlParser::parse(&source);
    assert!(
        outcome.diagnostics.is_empty(),
        "解注释的 `apis` 样例逐项合法：{:?}",
        outcome.diagnostics
    );
    assert!(
        !outcome.model.relations[0].apis.is_empty(),
        "样例名字逐项入模型（非空口承诺）"
    );

    let empty =
        TomlParser::parse("[[relation]]\nfrom = \"App.Auth\"\nto = \"App.Billing\"\napis = []\n");
    assert_eq!(empty.diagnostics.len(), 1, "显式空数组恰一条诊断");
    assert_eq!(
        empty.diagnostics[0].kind,
        DiagnosticKind::InvalidValue,
        "词典「显式空数组被拒」与 `validate_apis` 判据同向"
    );
}

// ===========================================================================
// `archi_drift`：恒全量 + 三 scope 逐级追加
// ===========================================================================

/// 漂移盘面：一元素认领 `src/auth/`，另有多处无主源文件（常态 unmatched）。
fn drift_dir(tag: &str, orphans: usize) -> tempfile::TempDir {
    let dir = tempdir(tag);
    write_model(
        dir.path(),
        "01-drift.toml",
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/auth/\"]\n",
        ),
    );
    write_file(dir.path(), "src/auth/lib.rs", "pub fn login() {}\n");
    for index in 0..orphans {
        let relative = format!("src/loose/orphan{index}.ts");
        write_file(dir.path(), &relative, "export const x = 1;\n");
    }
    dir
}

/// 正向：省略 `scope` → `summary` 投影（含计数与 `durationMs`，无 `observations` /
/// 判据条目 / `files` 清单）。
#[tokio::test]
async fn archi_drift缺省scope为summary() {
    let dir = drift_dir("drift-default", 5);
    let context = ArchiContext::new(dir.path().to_path_buf());
    for input in [json!({}), json!({ "scope": "summary" })] {
        let value = execute_json(&context, crate::sdk::tools::ARCHI_DRIFT_TOOL, input).await;
        assert!(value["counts"].is_object(), "含计数面");
        assert!(value["durationMs"].is_u64(), "含 durationMs");
        assert!(
            value.get("observations").is_none(),
            "summary 不含 observations"
        );
        assert!(value.get("files").is_none(), "summary 不含 files 清单");
        assert!(
            value["apiFindings"].get("items").is_none(),
            "summary 不含判据条目明细（只有 byKind 计数）"
        );
    }
}

/// 正向：`scope = "entries"` → summary 键 + `observations` + 判据条目全量；
/// `scope = "files"` → entries 键 + `files` 四类清单；三 scope 逐级包含。
#[tokio::test]
async fn archi_drift三scope逐级包含() {
    let dir = drift_dir("drift-scopes", 6);
    let context = ArchiContext::new(dir.path().to_path_buf());
    let summary = execute_json(
        &context,
        crate::sdk::tools::ARCHI_DRIFT_TOOL,
        json!({ "scope": "summary" }),
    )
    .await;
    let entries = execute_json(
        &context,
        crate::sdk::tools::ARCHI_DRIFT_TOOL,
        json!({ "scope": "entries" }),
    )
    .await;
    let files = execute_json(
        &context,
        crate::sdk::tools::ARCHI_DRIFT_TOOL,
        json!({ "scope": "files" }),
    )
    .await;

    assert!(
        entries["observations"].is_array(),
        "entries 含 observations"
    );
    assert!(
        entries["apiFindings"]["items"].is_array(),
        "entries 含判据条目"
    );
    for key in summary.as_object().expect("对象").keys() {
        assert!(entries.get(key).is_some(), "entries 含 summary 键 {key}");
    }
    for key in ["unmatched", "matched", "excluded", "unresolved"] {
        assert!(files["files"][key].is_array(), "files.{key} 为清单");
    }
    for key in entries.as_object().expect("对象").keys() {
        assert!(files.get(key).is_some(), "files 含 entries 键 {key}");
    }
    assert_eq!(
        files["counts"]["scan"]["unmatched"], summary["counts"]["scan"]["unmatched"],
        "增补不重算（计数两级相等）"
    );
    assert!(
        files["files"]["unmatched"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "files scope 才出路径清单"
    );
}

/// 异常（表驱动）：`scope` 值集外（`full` / 空串 / `Summary` 大小写变体）→ `Err`
/// 点名合法值，零静默退化；`null` 与缺键同解（= 未给参数，取缺省 `summary`）。
#[tokio::test]
async fn archi_drift非法scope报错点名合法值() {
    let dir = drift_dir("drift-invalid-scope", 1);
    let context = ArchiContext::new(dir.path().to_path_buf());
    for input in [
        json!({ "scope": "full" }),
        json!({ "scope": "" }),
        json!({ "scope": "Summary" }),
        json!({ "scope": "FILES" }),
    ] {
        let error = context
            .execute(crate::sdk::tools::ARCHI_DRIFT_TOOL, &input)
            .await
            .expect_err("值集外应 Err");
        for value in ["summary", "entries", "files"] {
            assert!(error.contains(value), "Err 点名 {value}：{error}");
        }
    }
    // `null` = 未给参数 → 缺省 summary（零静默退化到其它 scope）
    let value = execute_json(
        &context,
        crate::sdk::tools::ARCHI_DRIFT_TOOL,
        json!({ "scope": null }),
    )
    .await;
    assert!(value["counts"].is_object(), "null 走缺省 summary 投影");
    assert!(value.get("files").is_none(), "缺省不是 files");
}

/// 正向：恒全量——入参仅 `scope`（零 `mode` / 零 `changed` / 零 `change`），报告
/// `mode = "full"`，payload 零 `change` / `base` 槽位（全量模式无 change 上下文）。
#[tokio::test]
async fn archi_drift恒全量零change上下文() {
    let dir = drift_dir("drift-full", 3);
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_DRIFT_TOOL, json!({})).await;
    assert_eq!(value["mode"], "full", "恒全量");
    assert!(value.get("change").is_none(), "payload 零 change 槽位");
    assert!(value.get("base").is_none(), "payload 零 base 槽位");
}

/// 异常（表驱动）：坏模型 → `Ok` 且 `status = "errors"` + `structuralErrors` 明细
/// （结构错是报告内槽位，不走 `is_error`）；架构域配置非法 → `Err` 文案含成因（经
/// `CheckError::message()` 回灌），零 panic。
#[tokio::test]
async fn archi_drift错误面回灌成因() {
    let broken = tempdir("drift-broken-model");
    write_model(
        broken.path(),
        "01-broken.toml",
        "[element.\"App]\nkind = \"package\"\n",
    );
    let broken_context = ArchiContext::new(broken.path().to_path_buf());
    let value = execute_json(
        &broken_context,
        crate::sdk::tools::ARCHI_DRIFT_TOOL,
        json!({}),
    )
    .await;
    assert_eq!(value["status"], "errors", "坏模型即 errors");
    assert_eq!(value["passed"], serde_json::Value::Bool(false));
    assert!(
        value["model"]["structuralErrorCount"]
            .as_u64()
            .is_some_and(|count| count >= 1),
        "结构错计数在场：{}",
        value["model"]["structuralErrorCount"]
    );

    let bad_config = drift_dir("drift-bad-config", 1);
    write_file(
        bad_config.path(),
        ".dev-team/architecture/config.json",
        "{ \"unknownKey\": [] }",
    );
    let config_context = ArchiContext::new(bad_config.path().to_path_buf());
    let error = config_context
        .execute(crate::sdk::tools::ARCHI_DRIFT_TOOL, &json!({}))
        .await
        .expect_err("配置非法应 Err");
    assert!(error.contains("unknownKey"), "文案含配置成因：{error}");
}

/// 边界：`gate` 全 `off` 常态盘面 → 返回文本只在计数位含 `unmatched` 数，零
/// unmatched 路径串。
#[tokio::test]
async fn archi_drift常态量只出计数() {
    let dir = drift_dir("drift-summary-noise", 12);
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_DRIFT_TOOL, json!({})).await;
    let text = value.to_string();
    for index in 0..12 {
        let orphan = format!("src/loose/orphan{index}.ts");
        assert!(!text.contains(&orphan), "summary 零路径串：{orphan}");
    }
    assert!(
        value["counts"]["scan"]["unmatched"]
            .as_u64()
            .is_some_and(|count| count >= 12),
        "计数位仍在：{}",
        value["counts"]["scan"]["unmatched"]
    );
}

/// 边界：`files` scope 超大清单 → 尾部截断留痕，计数位仍在（`cap_result` 共用）。
#[tokio::test]
async fn archi_drift_files截断留痕() {
    let dir = drift_dir("drift-files-truncate", 0);
    let filler = "x".repeat(150);
    for index in 0..400 {
        write_file(
            dir.path(),
            &format!("src/loose/{index}-{filler}.ts"),
            "export const x = 1;
",
        );
    }
    let context = ArchiContext::new(dir.path().to_path_buf());
    let text = context
        .execute(
            crate::sdk::tools::ARCHI_DRIFT_TOOL,
            &json!({ "scope": "files" }),
        )
        .await
        .expect("应 Ok");
    assert!(
        text.contains(&format!(
            "已截断：单结果超 {} 字节上限",
            crate::sdk::tools::MAX_RESULT_BYTES
        )),
        "截断文案与 `cap_result` 同形（共用上限单点）"
    );
    assert!(text.contains("截断"), "截断留痕在场（尾部）");
}

/// 边界：零跨调用缓存——连调两次各自 `durationMs` 在场；两次之间改盘面 → 第二次读到
/// 新状态（模型元素数变化）。
#[tokio::test]
async fn archi_drift零跨调用缓存() {
    let dir = drift_dir("drift-no-cache", 2);
    let context = ArchiContext::new(dir.path().to_path_buf());
    let first = execute_json(&context, crate::sdk::tools::ARCHI_DRIFT_TOOL, json!({})).await;
    assert!(first["durationMs"].is_u64(), "每次调用独立计时");

    write_model(
        dir.path(),
        "02-more.toml",
        concat!(
            "[element.\"App.Auth.Login\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/auth/login.rs\"]\n",
        ),
    );
    let second = execute_json(&context, crate::sdk::tools::ARCHI_DRIFT_TOOL, json!({})).await;
    assert!(
        second["model"]["elements"].as_u64() > first["model"]["elements"].as_u64(),
        "第二次读到新模型状态（零跨调用缓存）"
    );
    assert!(second["durationMs"].is_u64(), "第二次亦独立计时");
}

/// 边界：无模型仓库 → `status = "skipped"` 正常出（不报错）。
#[tokio::test]
async fn archi_drift空架构域仍出报告() {
    let dir = tempdir("drift-skipped");
    write_file(dir.path(), "src/loose/orphan.ts", "export const x = 1;\n");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let value = execute_json(&context, crate::sdk::tools::ARCHI_DRIFT_TOOL, json!({})).await;
    assert_eq!(value["status"], "skipped", "无模型即 skipped");
}

// ---------------------------------------------------------------------------
// 本变更：两写工具（`archi_element` / `archi_relation`）——响应面、E1 硬闸、
// E2 前置、保护闸、幂等与乐观锁闭环
// ---------------------------------------------------------------------------

use crate::sdk::mount::WRITE_LIMITS;
use crate::sdk::mount::{ARCHI_ELEMENT_TOOL, ARCHI_RELATION_TOOL};
use crate::sdk::protect::ProtectRules;

/// 带保护规则集的上下文（真实用户配置 + 真实 `ProtectRules::load`）。
fn with_protect(root: &Path) -> ArchiContext {
    ArchiContext::new(root.to_path_buf()).with_protect(ProtectRules::load(Some(root)))
}

/// 元素写的成功响应体（`archi_element` 的三通道面）。
async fn element_write(context: &ArchiContext, input: Value) -> Value {
    execute_json(context, ARCHI_ELEMENT_TOOL, input).await
}

/// 正向：upsert 新建 ⇒ 响应含 `warnings`（数组）/ `action = "upsert"` / `element` /
/// `file`（repo 相对 POSIX）/ `hash`（`sha256:`）/ `created = true` / `changed`（数组）/
/// `relations` 四计数 / `counts` / `model` / `reordered`（搬迁块）；盘上字节 ==
/// `plan_element_upsert` 的 `content`。
#[tokio::test]
async fn 元素写响应三通道齐备且落盘同文本() {
    let dir = healthy("element-write-ok");
    let context = with_protect(dir.path());
    // 期望文本先在原地计算（同一引擎随后被工具调用消费；顺序颠倒会让「重放」变成
    // 「在案元素给 kind」）
    let mut spec = archi::edit::ElementSpec {
        fqn: "App.Auth.Token".to_owned(),
        ..Default::default()
    };
    spec.kind = Some(ElementKind::Module);
    spec.path = Some(vec!["./src/auth/token/".to_owned()]);
    spec.summary = Some(Some("令牌".to_owned()));
    let expected =
        archi::edit::plan_element_upsert(dir.path(), &spec).expect("同一规格的计划应成功");

    let payload = element_write(
        &context,
        json!({
            "action": "upsert",
            "element": "App.Auth.Token",
            "kind": "module",
            "path": ["./src/auth/token/"],
            "summary": "令牌",
        }),
    )
    .await;

    assert_eq!(payload["action"], json!("upsert"), "动作回显");
    assert_eq!(payload["element"], json!("App.Auth.Token"), "元素回显");
    assert!(
        payload["file"]
            .as_str()
            .is_some_and(|file| file.ends_with("01-core.toml")),
        "file 为 repo 相对 POSIX：{}",
        payload["file"]
    );
    assert!(
        payload["hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")),
        "hash 形态 sha256:<hex>：{}",
        payload["hash"]
    );
    assert_eq!(payload["created"], json!(true), "首次新建");
    assert!(
        payload["warnings"].is_array(),
        "warnings 恒在场（空数组 = 无异常）"
    );
    assert_eq!(
        payload["relations"],
        json!({ "created": 0, "updated": 0, "deleted": 0, "unchanged": 0 }),
        "无内联边 ⇒ 四计数皆 0（不是省略）"
    );
    assert_eq!(
        payload["counts"],
        json!({ "elements": 4, "relations": 1 }),
        "本文件计数"
    );
    assert_eq!(
        payload["model"],
        json!({ "elements": 4, "relations": 1 }),
        "全模型计数"
    );
    let changed: Vec<&str> = payload["changed"]
        .as_array()
        .expect("changed 为数组")
        .iter()
        .map(|field| field.as_str().expect("字段名为串"))
        .collect();
    for field in &changed {
        assert!(
            ["kind", "gate", "api", "entry", "path", "doc", "summary", "metadata"].contains(field),
            "changed 是参数面字段名子集：{field}"
        );
    }

    // 落盘文本 == `plan_element_upsert` 的 content（同一引擎，零第二份渲染）
    assert_eq!(
        fs::read_to_string(&expected.write.file).expect("读目标失败"),
        expected.write.content,
        "盘上字节 == plan 的 content"
    );
}

/// 正向：`archi_relation` 薄形——`warnings` **恒空数组** / `action` / `from` / `to` /
/// `file` / `hash` / `created` / `changed` ⊆ {`desc`, `apis`} / `reordered`（搬迁块）；
/// 零 `counts` / `model` / `relations` 键。
#[tokio::test]
async fn 关系写薄形无计数() {
    let dir = healthy("relation-write-ok");
    let context = with_protect(dir.path());
    let payload = execute_json(
        &context,
        ARCHI_RELATION_TOOL,
        json!({ "action": "upsert", "from": "App.Billing", "to": "App.Auth", "desc": "计费回调认证" }),
    )
    .await;

    let mut keys: Vec<&str> = payload
        .as_object()
        .expect("薄形为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "action",
            "changed",
            "created",
            "file",
            "from",
            "hash",
            "reordered",
            "to",
            "warnings"
        ],
        "薄形键面封闭（九项；零 counts / model / relations）"
    );
    assert_eq!(payload["warnings"], json!([]), "关系写 warnings 恒空数组");
    assert_eq!(payload["created"], json!(true), "新对 ⇒ created");
    assert!(
        payload["reordered"]["moved"].is_u64() && payload["reordered"]["blocks"].is_array(),
        "reordered 恒在场（moved 真值 + blocks 有界）：{}",
        payload["reordered"]
    );
    assert_eq!(
        payload["changed"],
        json!(["desc"]),
        "changed ⊆ {{desc, apis}}"
    );
    assert!(
        payload["file"]
            .as_str()
            .is_some_and(|file| file.ends_with("01-core.toml")),
        "新增关系落 from 元素文件：{}",
        payload["file"]
    );
}

/// 异常（矩阵多形态）：值域 / 必填硬闸逐项拒绝并点名字段与合法值；多处违例
/// **一次报全**（`；` 分隔）。
#[tokio::test]
#[allow(non_snake_case)]
async fn E1值域一次报全() {
    let dir = healthy("element-e1-domain");
    let context = with_protect(dir.path());
    let long_fqn = "A".repeat(WRITE_LIMITS.fqn + 1);

    for (input, needle) in [
        (
            json!({ "action": "modify", "element": "App.Auth" }),
            "action",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "kind": "service" }),
            "kind",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "gate": "hard" }),
            "gate",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "api": "partial" }),
            "api",
        ),
        (
            json!({ "action": "upsert", "element": "   " }),
            "element 不得为空白",
        ),
        (
            json!({ "action": "upsert", "element": long_fqn }),
            "element 超上限",
        ),
        (
            json!({
                "action": "upsert",
                "element": "App.Auth",
                "relations": [{ "to": "App.Billing", "action": "replace" }],
            }),
            "relations[1].action 非法值",
        ),
    ] {
        let error = context
            .execute(ARCHI_ELEMENT_TOOL, &input)
            .await
            .expect_err("值域违例必须 Err");
        assert!(error.contains(needle), "Err 应点名 {needle}：{error}");
    }

    // 一次报全：两处违例同批出现且以 `；` 分隔
    let error = context
        .execute(
            ARCHI_ELEMENT_TOOL,
            &json!({ "action": "modify", "element": "App.Auth", "kind": "service" }),
        )
        .await
        .expect_err("多处违例必须 Err");
    assert!(
        error.contains('；') && error.contains("action") && error.contains("kind"),
        "一次报全（两处违例同批）：{error}"
    );
    assert_eq!(error.lines().count(), 1, "违例串单行");
}

/// 异常（穷举上限）：逐字段超限各 `Err`，文案点名字段与上限（数字取自
/// `WRITE_LIMITS`）。
#[tokio::test]
#[allow(non_snake_case)]
async fn E1上限逐字段拒() {
    let dir = healthy("element-e1-limits");
    let context = with_protect(dir.path());
    let big = |limit: usize| "x".repeat(limit + 1);
    let too_many = |limit: usize| {
        (0..=limit)
            .map(|index| format!("p{index}"))
            .collect::<Vec<String>>()
    };
    let metadata: serde_json::Map<String, Value> = (0..=WRITE_LIMITS.metadata_items)
        .map(|index| (format!("k{index}"), json!("v")))
        .collect();
    let relations: Vec<Value> = (0..=WRITE_LIMITS.relations)
        .map(|index| json!({ "to": format!("App.E{index}") }))
        .collect();
    let long_key = big(WRITE_LIMITS.metadata_key);

    let cases: Vec<(Value, String)> = vec![
        (
            json!({ "action": "upsert", "element": "App.Auth", "path": too_many(WRITE_LIMITS.path_items) }),
            "path 超上限".to_owned(),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "path": [big(WRITE_LIMITS.path_item)] }),
            "path[0] 超上限".to_owned(),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "metadata": Value::Object(metadata) }),
            "metadata 超上限".to_owned(),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "metadata": { long_key: "v" } }),
            "metadata 键".to_owned(),
        ),
        (
            json!({
                "action": "upsert",
                "element": "App.Auth",
                "metadata": { "k": big(WRITE_LIMITS.metadata_value) },
            }),
            "metadata[\"k\"] 超上限".to_owned(),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "relations": relations }),
            "relations 超上限".to_owned(),
        ),
        (
            json!({
                "action": "upsert",
                "element": "App.Auth",
                "relations": [{ "to": "App.Billing", "apis": too_many(WRITE_LIMITS.relation_apis) }],
            }),
            "relations[1].apis 超上限".to_owned(),
        ),
        (
            json!({
                "action": "upsert",
                "element": "App.Auth",
                "relations": [{ "to": "App.Billing", "apis": [big(WRITE_LIMITS.relation_api)] }],
            }),
            "relations[1].apis[0] 超上限".to_owned(),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "summary": big(WRITE_LIMITS.text) }),
            "summary 超上限".to_owned(),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "entry": big(WRITE_LIMITS.pointer) }),
            "entry 超上限".to_owned(),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "file": big(WRITE_LIMITS.file) }),
            "file 超上限".to_owned(),
        ),
    ];
    for (input, needle) in cases {
        let error = context
            .execute(ARCHI_ELEMENT_TOOL, &input)
            .await
            .expect_err("超限必须 Err");
        assert!(error.contains(&needle), "Err 应点名 `{needle}`：{error}");
    }
}

/// 异常（矩阵形态）：类型面与跨字段互斥——`path` 非数组 / `metadata` 非对象 /
/// `relations` 非对象数组 / 项缺 `to` / `cascade = true` 配 `upsert`。
#[tokio::test]
#[allow(non_snake_case)]
async fn E1类型与跨字段互斥() {
    let dir = healthy("element-e1-types");
    let context = with_protect(dir.path());
    for (input, needle) in [
        (
            json!({ "action": "upsert", "element": "App.Auth", "path": "src/auth" }),
            "path 必须是字符串数组或 null",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "metadata": ["a"] }),
            "metadata 必须是对象或 null",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "relations": ["App.Billing"] }),
            "relations[1] 必须是对象",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "relations": [{}] }),
            "relations[1].to 必填",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "cascade": true }),
            "cascade 仅 action = delete",
        ),
    ] {
        let error = context
            .execute(ARCHI_ELEMENT_TOOL, &input)
            .await
            .expect_err("类型 / 互斥违例必须 Err");
        assert!(error.contains(needle), "Err 应含 `{needle}`：{error}");
    }
}

/// 异常（矩阵 2 入口）：模型含结构诊断 ⇒ 两写臂各 `Err`，文案 ==
/// `dto::graph_diagnostics_error` 全量形（含种类与 `file:line`）、零落盘。
#[tokio::test]
async fn 结构诊断前置零落盘() {
    let dir = tempdir("write-diagnostics");
    // `package` 两条 `path` ⇒ parser 结构诊断（模型停摆形态）
    write_model(
        dir.path(),
        "01-core.toml",
        "# 坏模型\n[element.\"App\"]\nkind = \"package\"\npath = [\"a\", \"b\"]\n",
    );
    let context = with_protect(dir.path());
    let expected = archi::dto::graph_diagnostics_error(
        &archi::dto::graph_view(dir.path())
            .expect("模型可读")
            .diagnostics,
    );
    let before = snapshot(dir.path());

    for (tool, input) in [
        (
            ARCHI_ELEMENT_TOOL,
            json!({ "action": "upsert", "element": "App.Auth", "kind": "domain" }),
        ),
        (
            ARCHI_RELATION_TOOL,
            json!({ "action": "upsert", "from": "App", "to": "App.Auth" }),
        ),
    ] {
        let error = context
            .execute(tool, &input)
            .await
            .expect_err("结构诊断前置必须 Err");
        assert_eq!(
            error, expected,
            "{tool} 的文案 == graph_diagnostics_error 全量形"
        );
        assert!(error.contains("01-core.toml:"), "文案含 file:line：{error}");
    }
    assert_eq!(snapshot(dir.path()), before, "结构诊断前置零落盘");
}

/// 异常（矩阵形态）：`plan_*` 错误映射——元素缺席 / 落点非法 / `expect` 不匹配 /
/// 批量重复违例 ⇒ `Err` 以判据句起始，判据句与事实之间恰一个全角 `：`，首行自足。
#[tokio::test]
async fn 写面err即message且第一行自足() {
    let dir = healthy("element-plan-errors");
    let context = with_protect(dir.path());

    let cases = [
        (
            json!({ "action": "delete", "element": "App.Legacy" }),
            "元素不存在",
            "模型共 3 个元素",
        ),
        (
            json!({ "action": "upsert", "element": "Orphan", "kind": "domain" }),
            "落点非法",
            "新建必须给出 file",
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "summary": "x", "expect": "sha256:错值" }),
            "目标文件已变",
            "重读后再试",
        ),
        (
            json!({
                "action": "upsert",
                "element": "App.Auth",
                "relations": [{ "to": "App.Billing" }, { "to": "App.Billing" }],
            }),
            "同一批 relations 重复声明同一目标",
            "只出现一次",
        ),
    ];
    for (input, criterion, needle) in cases {
        let error = context
            .execute(ARCHI_ELEMENT_TOOL, &input)
            .await
            .expect_err("写面判据必须 Err");
        assert!(
            error.starts_with(criterion),
            "Err 以判据句起始（`{criterion}`）：{error}"
        );
        assert_eq!(
            error.matches('：').count(),
            1,
            "判据句与事实恰一个全角冒号分隔：{error}"
        );
        let first = error.lines().next().unwrap_or_default();
        assert_eq!(first, error, "判据串单行（第一行自足）");
        assert!(
            first.contains(needle),
            "首行自足（失败动作 / 出路 `{needle}`）：{first}"
        );
    }

    // 内联边已声明在别的文件 ⇒ 点名该文件并指向 `archi_relation`
    let dir = tempdir("element-owned-elsewhere");
    write_model(
        dir.path(),
        "01-app.toml",
        concat!(
            "# 01 · app
",
            "[element.\"App\"]
",
            "kind = \"package\"
",
            "
",
            "[element.\"App.Auth\"]
",
            "kind = \"domain\"
",
            "
",
            "[element.\"App.Billing\"]
",
            "kind = \"domain\"
",
        ),
    );
    write_model(
        dir.path(),
        "02-relations.toml",
        "# 02 · 关系
[[relation]]
from = \"App.Auth\"
to = \"App.Billing\"
",
    );
    let context = with_protect(dir.path());
    let error = context
        .execute(
            ARCHI_ELEMENT_TOOL,
            &json!({
                "action": "upsert",
                "element": "App.Auth",
                "summary": "认证",
                "relations": [{ "to": "App.Billing" }],
            }),
        )
        .await
        .expect_err("内联边在他处必须 Err");
    assert!(
        error.contains("内联边声明在别的模型文件")
            && error.contains("02-relations.toml")
            && error.contains("改用 archi_relation"),
        "点名文件并给出改用路径：{error}"
    );
}

/// 异常（矩阵 2 形态）：用户规则 glob 命中**解析后的目标文件** ⇒ `Err` 为既有族拒串
/// （`%s` / `%t` 已替换）、零落盘；未命中 ⇒ 正常写入。
#[tokio::test]
async fn 命中用户规则即拒写() {
    let dir = healthy("element-protect");
    // 用户规则用**绝对 glob**（相对 glob 会按 `**/` 前缀归一，跨平台匹配面不稳）
    let root_text = dir.path().to_string_lossy().replace('\\', "/");
    let model_glob = format!("{root_text}/.dev-team/architecture/graph/model/**");
    let config = json!({
        "write_protection": {
            "files": [{ "glob": model_glob, "reason": "架构模型只经写工具" }]
        }
    });
    let config_path = foundation::layout::config_path(dir.path());
    fs::create_dir_all(config_path.parent().expect("配置有父目录")).expect("建配置目录失败");
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).expect("配置序列化"),
    )
    .expect("写配置文件失败");
    let context = with_protect(dir.path());
    let before = snapshot(dir.path());

    let error = context
        .execute(
            ARCHI_ELEMENT_TOOL,
            &json!({ "action": "upsert", "element": "App.Auth", "summary": "被拒" }),
        )
        .await
        .expect_err("命中用户规则必须拒写");
    assert!(
        error.contains("架构模型只经写工具") && !error.contains("%s") && !error.contains("%t"),
        "既有族拒串且占位已替换：{error}"
    );
    assert_eq!(snapshot(dir.path()), before, "拒写零落盘");

    // 未命中：另一个根无用户规则 ⇒ 正常写入
    let open_dir = healthy("element-protect-pass");
    let context = with_protect(open_dir.path());
    element_write(
        &context,
        json!({ "action": "upsert", "element": "App.Auth", "summary": "放行" }),
    )
    .await;
}

/// 异常：`ArchiContext::new(root)` 直接调写臂 ⇒ `Err` 并声明「生产不可达」（不静默
/// 放行）。
#[tokio::test]
async fn 未注入规则集不静默放行() {
    let dir = healthy("element-no-protect");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let error = context
        .execute(
            ARCHI_ELEMENT_TOOL,
            &json!({ "action": "upsert", "element": "App.Auth", "summary": "x" }),
        )
        .await
        .expect_err("未注入规则集必须 Err");
    assert!(
        error.contains("写入保护规则未装配") && error.contains("生产不可达"),
        "显式拒绝并声明生产不可达：{error}"
    );
}

/// 边界（矩阵 2 形态）：干净写 ⇒ `warnings == []`（键在场、零省略）；同值重复写 ⇒
/// `changed == []`、`created == false`、盘上字节与首次逐字节相等、两次 `hash` 相同。
#[tokio::test]
async fn warnings恒在场且幂等零字节变化() {
    let dir = healthy("element-idempotent");
    let context = with_protect(dir.path());
    write_file(dir.path(), "src/billing/README.md", "# 计费\n");
    let input = json!({
        "action": "upsert",
        "element": "App.Billing",
        "api": "none",
        "doc": "./src/billing/README.md",
    });
    let file = dir
        .path()
        .join(".dev-team/architecture/graph/model/01-core.toml");

    let first = element_write(&context, input.clone()).await;
    assert_eq!(first["warnings"], json!([]), "干净写 ⇒ 空数组（键仍在）");
    assert_eq!(first["changed"], json!(["api", "doc"]), "首次改动字段");
    assert_eq!(first["created"], json!(false), "在案元素不新建");
    let bytes_after_first = fs::read(&file).expect("读模型失败");

    let second = element_write(&context, input).await;
    assert_eq!(
        second["changed"],
        json!([]),
        "同值重放 ⇒ changed 空（幂等）"
    );
    assert_eq!(second["created"], json!(false));
    assert_eq!(first["hash"], second["hash"], "两次 hash 相同");
    assert_eq!(
        fs::read(&file).expect("读模型失败"),
        bytes_after_first,
        "同值重放零字节变化"
    );
}

/// 边界：满字段面 + 多条 warning 的成功响应 < `MAX_RESULT_BYTES`（未触达截断）。
#[tokio::test]
async fn 成功响应永不触达截断() {
    let dir = healthy("element-response-size");
    let context = with_protect(dir.path());
    write_file(dir.path(), "src/new/README.md", "# 新域\n");
    let long_text = "说".repeat(WRITE_LIMITS.text);
    let metadata: serde_json::Map<String, Value> = (0..WRITE_LIMITS.metadata_items)
        .map(|index| (format!("k{index}"), json!("v")))
        .collect();
    let input = json!({
        "action": "upsert",
        "element": "App.Core",
        "kind": "module",
        "path": ["./src/new/"],
        "summary": long_text,
        "metadata": Value::Object(metadata),
        "relations": [{ "to": "App.Auth", "desc": long_text }],
    });
    let text = context
        .execute(ARCHI_ELEMENT_TOOL, &input)
        .await
        .expect("满字段面写应成功");
    assert!(
        text.len() < MAX_RESULT_BYTES,
        "成功响应体量 {} B 应小于上限 {MAX_RESULT_BYTES} B",
        text.len()
    );
}

/// 边界（矩阵形态）：脏入参不 panic——`null` / 数字 / 数组 / 嵌套对象 / 多余键 ⇒
/// `Err` 或成功，零 panic；`relations: null` 与省略同义。
#[tokio::test]
async fn 脏入参不panic() {
    let dir = healthy("element-dirty-input");
    let context = with_protect(dir.path());
    for input in [
        Value::Null,
        json!(7),
        json!(["a"]),
        json!({ "action": "upsert", "element": "App.Auth", "path": { "nested": [1, 2] } }),
        json!({ "action": "upsert", "element": "App.Auth", "summary": "x", "unknown_field": "y" }),
    ] {
        let result = context.execute(ARCHI_ELEMENT_TOOL, &input).await;
        assert!(
            result.is_ok() || result.is_err(),
            "脏入参零 panic（只判返回形态）"
        );
    }
    let omitted = element_write(
        &context,
        json!({ "action": "upsert", "element": "App.Auth", "summary": "无 relations 键" }),
    )
    .await;
    let null_relations = element_write(
        &context,
        json!({
            "action": "upsert",
            "element": "App.Auth",
            "summary": "无 relations 键",
            "relations": null,
        }),
    )
    .await;
    assert_eq!(
        omitted["hash"], null_relations["hash"],
        "relations: null 与省略同义（出边不动、文本同值）"
    );
}

/// 正向（组合：`archi_module` → `archi_element` → 响应 `hash`）：`archi_module(element)`
/// 取 `fileHash` → `archi_element { expect: fileHash }` 成功 → 响应 `hash` 作下一次
/// `expect` 再次成功；两次响应 `hash` 与盘面 `file_hash` 同值。
#[tokio::test]
#[allow(non_snake_case)]
async fn fileHash到expect到hash闭环() {
    let dir = healthy("element-expect-roundtrip");
    let context = with_protect(dir.path());
    let file = dir
        .path()
        .join(".dev-team/architecture/graph/model/01-core.toml");

    let detail = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Auth" }),
    )
    .await;
    let file_hash = detail["fileHash"]
        .as_str()
        .expect("module 出参含 fileHash")
        .to_owned();
    assert_eq!(
        file_hash,
        archi::drift::file_hash(&file),
        "fileHash 与 drift::file_hash(声明文件) 逐字相等"
    );

    let first = element_write(
        &context,
        json!({
            "action": "upsert",
            "element": "App.Auth",
            "summary": "认证域（改）",
            "expect": file_hash,
        }),
    )
    .await;
    let first_hash = first["hash"].as_str().expect("响应含 hash").to_owned();
    assert_eq!(
        first_hash,
        archi::drift::file_hash(&file),
        "响应 hash == 盘面 file_hash"
    );

    let second = element_write(
        &context,
        json!({
            "action": "upsert",
            "element": "App.Auth",
            "summary": "认证域（再改）",
            "expect": first_hash,
        }),
    )
    .await;
    assert_eq!(
        second["hash"],
        json!(archi::drift::file_hash(&file)),
        "上一次响应的 hash 可作下一次 expect"
    );
}

/// 异常（矩阵 2 形态）：先写一次再以**旧** `expect` 写 ⇒ `ExpectMismatch` + 盘上零
/// 变化；写臂在 `plan_*` 之后才判保护闸（非法落点的 `Err` 来自 `plan_*` 而非保护串）。
#[tokio::test]
async fn 旧expect拒且保护闸后置() {
    let dir = healthy("element-stale-expect");
    let context = with_protect(dir.path());
    let file = dir
        .path()
        .join(".dev-team/architecture/graph/model/01-core.toml");
    let stale = archi::drift::file_hash(&file);

    element_write(
        &context,
        json!({ "action": "upsert", "element": "App.Auth", "summary": "第一改" }),
    )
    .await;
    let after_first = fs::read(&file).expect("读模型失败");

    let error = context
        .execute(
            ARCHI_ELEMENT_TOOL,
            &json!({
                "action": "upsert",
                "element": "App.Auth",
                "summary": "第二改",
                "expect": stale,
            }),
        )
        .await
        .expect_err("旧 expect 必须 Err");
    assert!(
        error.starts_with("目标文件已变") && error.contains("重读后再试"),
        "文案点名判据与出路：{error}"
    );
    assert_eq!(
        fs::read(&file).expect("读模型失败"),
        after_first,
        "旧 expect 零落盘"
    );

    // 非法落点：Err 来自 plan（落点判据），不来自保护串
    let error = context
        .execute(
            ARCHI_ELEMENT_TOOL,
            &json!({ "action": "upsert", "element": "Orphan", "kind": "domain" }),
        )
        .await
        .expect_err("非法落点必须 Err");
    assert!(
        error.contains("新建必须给出 file"),
        "落点判据先于保护闸：{error}"
    );
    assert!(
        !error.contains("写入保护") && !error.contains("配置权威"),
        "非保护串：{error}"
    );
}

/// 异常（源码锚）：`SCAFFOLD_TEMPLATE` 首行指向两写工具（零旧立场句）。
#[test]
fn 草稿模板改指两写工具() {
    let source = include_str!("archi_tool.rs");
    let first_line = source
        .lines()
        .find(|line| line.contains("新元素模板"))
        .expect("模板首行在场");
    assert!(
        first_line.contains("archi_element") && first_line.contains("archi_relation"),
        "模板首行指向两写工具：{first_line}"
    );
    assert!(
        first_line.contains("工具表达不了的才手改"),
        "模板首行保留手改出口：{first_line}"
    );
    for stale in stale_stances() {
        assert!(
            !source.contains(stale.as_str()),
            "archi_tool.rs 零旧立场句 `{stale}`"
        );
    }
    assert_eq!(
        source.matches("fn element_payload").count(),
        1,
        "元素写执行体恰一处"
    );
    assert_eq!(
        source.matches("fn relation_payload").count(),
        1,
        "关系写执行体恰一处"
    );
}

// ---------------------------------------------------------------------------
// 本变更（D-19）：值域查表单源与写面口径单源
// ---------------------------------------------------------------------------

/// 值域判据句的期望形态：合法值清单由 `ALL` 现算（测试内零手抄词表与分隔符）。
fn expected_domain_error<T: Copy>(
    raw: &str,
    name: &str,
    all: &[T],
    as_str: impl Fn(T) -> &'static str,
) -> String {
    let vocabulary = all
        .iter()
        .copied()
        .map(as_str)
        .collect::<Vec<_>>()
        .join(" / ");
    format!("{name} 非法值 {raw:?}（合法值：{vocabulary}）")
}

/// 正向（矩阵 3 字段 × 2 入口）：同一非法 `kind` 值经 `archi_element`（E1）与
/// `archi_scaffold`（草稿）两入口 ⇒ `Err` 串**逐字相同**，且逐字等于单条判据句
/// 「`{name} 非法值 "{raw}"（合法值：…）`」（合法值序由 `ElementKind::ALL` 现算）；
/// `gate` / `api` 两字段在 E1 面同句式。
#[tokio::test]
async fn 值域判据句单条且两入口同文() {
    let dir = healthy("domain-value-single-sentence");
    let context = with_protect(dir.path());

    for raw in ["service", "Domain", "packagex"] {
        let expected = expected_domain_error(raw, "kind", &ElementKind::ALL, ElementKind::as_str);
        let scaffold_error = context
            .execute(ARCHI_SCAFFOLD_TOOL, &json!({ "kind": raw }))
            .await
            .expect_err("草稿侧值集外应 Err");
        let element_error = context
            .execute(
                ARCHI_ELEMENT_TOOL,
                &json!({ "action": "upsert", "element": "App.Auth", "kind": raw }),
            )
            .await
            .expect_err("E1 值集外应 Err");
        assert_eq!(
            scaffold_error, expected,
            "草稿侧判据句单条：{scaffold_error}"
        );
        assert_eq!(element_error, expected, "E1 侧判据句单条：{element_error}");
        assert_eq!(scaffold_error, element_error, "两入口同一判据句");
    }

    for (input, expected) in [
        (
            json!({ "action": "upsert", "element": "App.Auth", "gate": "hard" }),
            expected_domain_error("hard", "gate", &GateTier::ALL, GateTier::as_str),
        ),
        (
            json!({ "action": "upsert", "element": "App.Auth", "api": "partial" }),
            expected_domain_error("partial", "api", &ApiStatus::ALL, ApiStatus::as_str),
        ),
    ] {
        let error = context
            .execute(ARCHI_ELEMENT_TOOL, &input)
            .await
            .expect_err("值集外应 Err");
        assert_eq!(error, expected, "E1 同句式判据句：{error}");
    }
}

/// 异常（源码锚，D-19 ①）：值域查表唯一核——`fn domain_value` 恰一处、查表动作（`find`）
/// 只在该核内；三词表只以实参形态下发（`kind` 两处 = 草稿 + E1，`gate` / `api` 各一处），
/// 零第二份 `ElementKind::ALL` 查表；旧文案零残留。
#[test]
fn 值域查表一处() {
    let source = include_str!("archi_tool.rs");
    assert_eq!(
        source.matches("fn domain_value<").count(),
        1,
        "值域核恰一处"
    );
    assert_eq!(
        source.matches("find(|candidate|").count(),
        1,
        "查表动作只在该私有核内"
    );
    assert_eq!(
        source.matches("ElementKind::ALL").count(),
        2,
        "kind 词表只经两处实参下发（草稿 + E1）"
    );
    for vocabulary in ["GateTier::ALL", "ApiStatus::ALL"] {
        assert_eq!(
            source.matches(vocabulary).count(),
            1,
            "{vocabulary} 只经 E1 一处实参下发"
        );
    }
    let legacy = ["非法 ", "kind:"].concat();
    assert!(!source.contains(&legacy), "旧文案零残留（D-19 ①）");
}

/// `marker` 行正上方的连续行注释块（`///`）；遇到空行或非注释行即止。
fn doc_above(source: &str, marker: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let index = lines
        .iter()
        .position(|line| line.contains(marker))
        .expect("锚行在场");
    let mut doc = lines[..index]
        .iter()
        .rev()
        .map(|line| line.trim())
        .take_while(|line| line.starts_with("///"))
        .collect::<Vec<_>>();
    doc.reverse();
    doc.join("\n")
}

/// `SCAFFOLD_TEMPLATE` 的行内首行（`r#"` 之后、首行文本）。
fn template_first_line(source: &str) -> String {
    source
        .lines()
        .find(|line| line.contains("const SCAFFOLD_TEMPLATE"))
        .and_then(|line| line.split_once("r#\""))
        .map(|(_, head)| head.to_owned())
        .expect("SCAFFOLD_TEMPLATE 首行在场")
}

/// 异常（源码锚，D-19 ②）：模块头、`scaffold_payload` 函数 doc、`SCAFFOLD_TEMPLATE`
/// 首行**三处**各含两写工具名（落盘走两写工具是同一口径）；`write` / `edit` 这两名在
/// 三处只在「工具表达不了的才手改」句里出现（hand-edit 是唯一出口，不是主写面）；
/// 旧立场句全文零命中。
#[test]
fn 写面口径三处同源() {
    let source = include_str!("archi_tool.rs");
    let hand_edit = "工具表达不了的才手改";

    let module_head = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");
    let scaffold_doc = doc_above(source, "fn scaffold_payload");
    let template_head = template_first_line(source);

    // 草稿模板首行与 `scaffold_payload` doc 必须给出同一句手改出口（模板是工具输出，
    // doc 是维护面口径）；模块头只需点名两写工具，不强制复述该句。
    for (label, text) in [
        ("scaffold_payload doc", scaffold_doc.as_str()),
        ("SCAFFOLD_TEMPLATE 首行", template_head.as_str()),
    ] {
        assert!(text.contains(hand_edit), "{label} 含手改出口一句：{text}");
    }

    for (label, text) in [
        ("模块头", module_head.as_str()),
        ("scaffold_payload doc", scaffold_doc.as_str()),
        ("SCAFFOLD_TEMPLATE 首行", template_head.as_str()),
    ] {
        assert!(!text.trim().is_empty(), "{label} 非空");
        assert!(
            text.contains("archi_element"),
            "{label} 含 archi_element：{text}"
        );
        assert!(
            text.contains("archi_relation"),
            "{label} 含 archi_relation：{text}"
        );
        for sentence in text.split('。') {
            let mentions_hand_tools = sentence.contains("`write`") || sentence.contains("`edit`");
            if mentions_hand_tools {
                assert!(
                    sentence.contains(hand_edit),
                    "{label} 的手改工具名只出现在手改出口句：{sentence}"
                );
            }
        }
    }

    for stale in stale_stances() {
        assert!(
            !source.contains(stale.as_str()),
            "archi_tool.rs 零旧立场句 `{stale}`"
        );
    }
}

// ---------------------------------------------------------------------------
// 三资产（`assets`）与读法（`reading`）：`archi_module` 单元素响应面
// ---------------------------------------------------------------------------

/// 三资产夹具根：`App.Auth`（`path` 非空 + 入口在盘），两份资产由用例各自落盘。
fn assets_root(tag: &str) -> tempfile::TempDir {
    let dir = tempdir(tag);
    write_model(
        dir.path(),
        "01-assets.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Auth\"]\nkind = \"module\"\npath = [\"src/auth/\"]\n",
        ),
    );
    write_file(dir.path(), "src/auth/src/lib.rs", "pub fn f() {}\n");
    dir
}

/// 异常 / 边界（AC-6 · 三态一：两份资产都不在盘）：三段 `present` 全假、接口三数组空、
/// 文档 `headings` 空；`topology` 恒给（file / line 来自模型）；`reading` 非空。
#[tokio::test]
async fn archi_module三资产皆缺() {
    let dir = assets_root("assets-none");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let payload = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Auth" }),
    )
    .await;

    let assets = &payload["assets"];
    assert_eq!(assets["interface"]["present"], json!(false), "接口不在盘");
    assert_eq!(assets["interface"]["bytes"], json!(0), "零字节");
    assert_eq!(assets["interface"]["ops"], json!([]), "零 op 键");
    assert_eq!(assets["interface"]["types"], json!([]), "零类型键");
    assert_eq!(assets["interface"]["reexports"], json!([]), "零透传键");
    assert_eq!(assets["doc"]["present"], json!(false), "文档不在盘");
    assert_eq!(assets["doc"]["headings"], json!([]), "零段名");
    assert!(
        assets["topology"]["file"]
            .as_str()
            .is_some_and(|file| file.ends_with("01-assets.toml")),
        "拓扑取模型声明文件：{assets}"
    );
    assert_eq!(assets["topology"]["out"], json!(0));
    assert_eq!(assets["topology"]["in"], json!(0));
    assert!(
        payload["reading"]
            .as_str()
            .is_some_and(|text| !text.is_empty()),
        "读法非空：{}",
        payload["reading"]
    );
}

/// 正向（AC-6 · 三态二：只有接口）：接口在盘 ⇒ `present = true`、键名三数组落位、判读按
/// 六形态取；文档段仍 `present = false`（两段独立）。
#[tokio::test]
async fn archi_module只有接口文件() {
    let dir = assets_root("assets-interface");
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Auth.schema.json",
        concat!(
            "{\n",
            "  \"$id\": \"App.Auth.schema.json\",\n",
            "  \"title\": \"App.Auth\",\n",
            "  \"$defs\": { \"Local\": { \"type\": \"object\", \"properties\": {} } },\n",
            "  \"properties\": {\n",
            "    \"run\": { \"x-function\": { \"params\": [], \"returns\": { \"type\": \"string\" } } },\n",
            "    \"Local\": { \"$ref\": \"#/$defs/Local\" }\n",
            "  }\n",
            "}\n",
        ),
    );
    let context = ArchiContext::new(dir.path().to_path_buf());
    let payload = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Auth" }),
    )
    .await;

    let interface = &payload["assets"]["interface"];
    assert_eq!(interface["present"], json!(true), "接口在盘");
    assert_eq!(interface["ops"], json!(["run"]), "op 键落位");
    assert_eq!(interface["types"], json!(["Local"]), "类型键落位");
    assert!(
        interface["abstract"]
            .as_str()
            .is_some_and(|text| text.contains("既有操作也有类型")),
        "判读按六形态：{interface}"
    );
    assert_eq!(
        payload["assets"]["doc"]["present"],
        json!(false),
        "文档段仍不在盘（两段独立）"
    );
}

/// 正向（AC-6 · 三态三：只有文档）：文档在盘 ⇒ `present = true`、段名按文件序、行数照实；
/// 接口段仍 `present = false` 且判读仍是「接口未写」；零正文外溢（`headings` 只给段名）。
#[tokio::test]
async fn archi_module只有文档() {
    let dir = assets_root("assets-doc");
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Auth.md",
        "# App.Auth\n\n## 职能\n\n认证域。\n\n## 不变量\n\n令牌不过期。\n",
    );
    let context = ArchiContext::new(dir.path().to_path_buf());
    let payload = execute_json(
        &context,
        ARCHI_MODULE_TOOL,
        json!({ "element": "App.Auth" }),
    )
    .await;

    let doc = &payload["assets"]["doc"];
    assert_eq!(doc["present"], json!(true), "文档在盘");
    assert_eq!(doc["headings"], json!(["职能", "不变量"]), "段名按文件序");
    assert_eq!(doc["lines"], json!(9), "行数照实");
    assert!(
        doc.get("content").is_none(),
        "只给段名与体量，零正文：{doc}"
    );
    assert_eq!(
        payload["assets"]["interface"]["present"],
        json!(false),
        "接口段仍不在盘"
    );
    assert!(
        payload["assets"]["interface"]["abstract"]
            .as_str()
            .is_some_and(|text| text.contains("接口未写")),
        "接口判读不因文档在盘而变：{}",
        payload["assets"]["interface"]["abstract"]
    );
}

// ---------------------------------------------------------------------------
// `archi_api` 的 `link` 动作：算 `$ref` 串（纯计算，零读盘零写盘）
// ---------------------------------------------------------------------------

/// 正向（AC-7 / D-15）：两形态互斥且前缀单点——跨元素形态的串 = `ref_relative(from, to)` +
/// `<to 叶段>.schema.json#/$defs/<name>`（前缀**不比 spec 示例字面**，与 `paths::ref_relative` 的
/// 输出逐字对拍）；本模块形态 = `#/$defs/<name>`；同批给两形态 ⇒ `Err`。
#[tokio::test]
async fn archi_api_link两形态且前缀单点() {
    let dir = tempdir("api-link");
    let context = ArchiContext::new(dir.path().to_path_buf());

    let payload = execute_json(
        &context,
        ARCHI_API_TOOL,
        json!({
            "action": "link",
            "from": "App.Api.Subject",
            "to": "App.Api.Subject.Part",
            "name": "Piece",
        }),
    )
    .await;
    let reference = payload["ref"].as_str().expect("ref 为字符串");
    let expected = format!(
        "{}{}.schema.json#/$defs/Piece",
        archi::api::paths::ref_relative("App.Api.Subject", "App.Api.Subject.Part"),
        archi::api::paths::leaf_name("App.Api.Subject.Part")
    );
    assert_eq!(
        reference, expected,
        "跨元素串 = ref_relative 输出 + 目标叶段文件名 + 片段（层数由单点求出）"
    );
    assert!(
        reference.starts_with("./Subject/"),
        "同域子元素的前缀是 `./Subject/`：{reference}"
    );

    let same = execute_json(
        &context,
        ARCHI_API_TOOL,
        json!({ "action": "link", "element": "App.Api.Subject", "name": "Piece" }),
    )
    .await;
    assert_eq!(
        same["ref"],
        json!("#/$defs/Piece"),
        "本模块形态只含片段（零文件部分）"
    );

    let error = context
        .execute(
            ARCHI_API_TOOL,
            &json!({
                "action": "link",
                "from": "App.Api.Subject",
                "to": "App.Api.Subject.Part",
                "element": "App.Api.Subject",
                "name": "Piece",
            }),
        )
        .await
        .expect_err("两形态同批给 ⇒ Err");
    assert!(error.contains("互斥"), "文案点明两形态互斥：{error}");
}

/// 异常（AC-7）：`from` / `to` 非 FQN、`name` 非标识符、三参数缺任一一律 `Err` 并点名该
/// 参数——FQN 形态闸与标识符闸各复用 `paths` / `contract` 单点（零第二份判据）。
#[tokio::test]
async fn archi_api_link参数校验() {
    let dir = tempdir("api-link-bad");
    let context = ArchiContext::new(dir.path().to_path_buf());
    let long_name = format!("{}.", "a".repeat(1000));

    let cases: [(&str, Value, &str); 9] = [
        (
            "from 非 FQN",
            json!({ "action": "link", "from": "App/Api", "to": "App.Api.Part", "name": "Piece" }),
            "from",
        ),
        (
            "to 非 FQN",
            json!({ "action": "link", "from": "App.Api.Subject", "to": "App..Part", "name": "Piece" }),
            "to",
        ),
        (
            "name 中划线",
            json!({ "action": "link", "from": "App.Api.Subject", "to": "App.Api.Part", "name": "bad-name" }),
            "标识符",
        ),
        (
            "name 空串",
            json!({ "action": "link", "element": "App.Api.Subject", "name": "   " }),
            "name",
        ),
        (
            "name emoji",
            json!({ "action": "link", "element": "App.Api.Subject", "name": "🚀" }),
            "标识符",
        ),
        (
            "name 超长含点",
            json!({ "action": "link", "element": "App.Api.Subject", "name": long_name }),
            "标识符",
        ),
        (
            "缺 name",
            json!({ "action": "link", "from": "App.Api.Subject", "to": "App.Api.Part" }),
            "name",
        ),
        (
            "缺 from",
            json!({ "action": "link", "to": "App.Api.Part", "name": "Piece" }),
            "from",
        ),
        (
            "缺 element 与 from/to",
            json!({ "action": "link", "name": "Piece" }),
            "element",
        ),
    ];
    for (label, input, needle) in cases {
        let error = match context.execute(ARCHI_API_TOOL, &input).await {
            Ok(text) => panic!("{label} 必须 Err，实际通过: {text}"),
            Err(error) => error,
        };
        assert!(
            error.contains(needle),
            "{label} 文案点名 `{needle}`：{error}"
        );
    }

    // 超长但形态合法的标识符过闸（判据只看形态，不设长度上限）
    let long_identifier = format!("a{}", "b".repeat(1200));
    let payload = execute_json(
        &context,
        ARCHI_API_TOOL,
        json!({ "action": "link", "element": "App.Api.Subject", "name": long_identifier }),
    )
    .await;
    assert_eq!(
        payload["ref"],
        json!(format!("#/$defs/{long_identifier}")),
        "合法形态的超长标识符原样出线"
    );
}

/// 边界（AC-7）：`link` 是纯计算——元素不存在 / 无模型目录也照算，调用前后盘面逐字节相等。
#[tokio::test]
async fn archi_api_link纯计算零io() {
    let dir = api_root("api-link-pure");
    let root = dir.path().to_path_buf();
    let before = snapshot(&root);
    let context = ArchiContext::new(root.clone());

    let payload = execute_json(
        &context,
        ARCHI_API_TOOL,
        json!({
            "action": "link",
            "from": "Ghost.Left",
            "to": "Ghost.Right",
            "name": "Thing",
        }),
    )
    .await;
    assert!(
        payload["ref"]
            .as_str()
            .is_some_and(|reference| reference.ends_with("Right.schema.json#/$defs/Thing")),
        "模型里没有该元素也照算：{}",
        payload["ref"]
    );
    assert_eq!(
        snapshot(&root),
        before,
        "调用前后盘面逐字节相等（零读盘零写盘）"
    );
}

/// 组合（AC-7 尾段）：`link` 算出的跨元素串写进目标 schema 后过写闸（`validate_schema` 的
/// ⑦ 逐字对拍同一 `ref_relative`），且 `check` 判据面 `apiRefMissing` 零命中——「算出的串
/// 必可达」。
#[tokio::test]
async fn archi_api_link到write到check可达() {
    let dir = tempdir("api-link-reach");
    write_model(
        dir.path(),
        "01-link.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Api\"]\nkind = \"domain\"\n\n",
            "[element.\"App.Api.Subject\"]\nkind = \"module\"\n",
            "path = [\"./crates/api/subject/\"]\n\n",
            "[element.\"App.Api.Subject.Part\"]\nkind = \"module\"\n",
            "path = [\"./crates/api/subject/part/\"]\n",
        ),
    );
    // 两元素的派生 md 在盘（`apiRefMissing` 的第四种问题＝目标 md 不在盘 ⇒ 零命中）
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Api/Subject.md",
        "# App.Api.Subject\n\n## 职能\n\n引用方。\n",
    );
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Api/Subject/Part.md",
        "# App.Api.Subject.Part\n\n## 职能\n\n被引用方。\n",
    );
    let context = ArchiContext::new(dir.path().to_path_buf());

    let reference = execute_json(
        &context,
        ARCHI_API_TOOL,
        json!({
            "action": "link",
            "from": "App.Api.Subject",
            "to": "App.Api.Subject.Part",
            "name": "Piece",
        }),
    )
    .await["ref"]
        .as_str()
        .expect("ref 为字符串")
        .to_owned();

    let schema = format!(
        "{{\n  \"$id\": \"App.Api.Subject.schema.json\",\n  \"title\": \"App.Api.Subject\",\n  \
         \"$defs\": {{ \"Piece\": {{ \"$ref\": \"{reference}\" }} }},\n  \
         \"properties\": {{ \"Piece\": {{ \"$ref\": \"#/$defs/Piece\" }} }}\n}}\n"
    );
    let target = dir
        .path()
        .join(".dev-team/architecture/models/App/Api/Subject.schema.json");
    let written = crate::sdk::tools::execute(
        dir.path(),
        "write",
        &json!({ "path": target, "content": schema }),
    )
    .await
    .expect("link 出的串必过写闸");
    assert!(
        written.contains("接口已校验"),
        "写盘经同一校验单点：{written}"
    );

    let payload = execute_json(&context, ARCHI_API_TOOL, json!({ "action": "check" })).await;
    let items = payload["findings"]["items"].as_array().expect("items 数组");
    let kinds: Vec<&str> = items
        .iter()
        .filter_map(|item| item["kind"].as_str())
        .collect();
    assert!(
        !kinds.contains(&"apiRefMissing"),
        "link 算出的跨元素串必可达（apiRefMissing 零命中）：{kinds:?}"
    );
}

// ---------------------------------------------------------------------------
// 本变更：写入序规范化（两写工具响应面的 `reordered`）
// ---------------------------------------------------------------------------

/// 关系组分裂夹具（R1 违例）：`A` 组被 `B → Y` 切开，新增 `A → Z` 即并回。
const RELATIONS_DIRTY: &str = concat!(
    "[element.\"A\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"B\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"X\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Y\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[element.\"Z\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"A\"\n",
    "to = \"X\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"B\"\n",
    "to = \"Y\"\n",
);

/// 九块错位模型（子块在前、父块在后）：一次写即报九块（`blocks` 截到 8 项）。
fn nine_block_model() -> String {
    let mut source = String::new();
    for index in 0..9 {
        source.push_str(&format!("[element.\"P{index}.C\"]\nkind = \"module\"\n\n"));
    }
    for index in 0..9 {
        source.push_str(&format!("[element.\"P{index}\"]\nkind = \"module\"\n\n"));
    }
    source
}

/// 单模型文件的合成根（`01-core.toml`）。
fn model_root(tag: &str, source: &str) -> tempfile::TempDir {
    let dir = tempdir(tag);
    write_model(dir.path(), "01-core.toml", source);
    dir
}

/// 模型文件绝对路径（`01-core.toml`）。
fn core_model_path(root: &Path) -> PathBuf {
    root.join(".dev-team/architecture/graph/model/01-core.toml")
}

/// 正向（AC-8 / C4）：元素写成功响应键面不变 + `reordered` 恒在场且零值形态恰
/// `{"moved":0,"blocks":[]}`（MUST NOT 因零值省略）。
#[tokio::test]
async fn 元素写响应reordered零值形态() {
    let dir = healthy("reorder-element-zero");
    let context = with_protect(dir.path());
    let payload = element_write(
        &context,
        json!({ "action": "upsert", "element": "App.Auth", "gate": "strict" }),
    )
    .await;

    assert_eq!(
        payload["reordered"],
        json!({ "moved": 0, "blocks": [] }),
        "零值也写两字段（键恒在场）"
    );
    let mut keys: Vec<&str> = payload
        .as_object()
        .expect("响应为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "action",
            "changed",
            "counts",
            "created",
            "element",
            "file",
            "hash",
            "model",
            "relations",
            "reordered",
            "warnings",
        ],
        "元素面键面 10 → 11（既有十键逐字不变）"
    );
}

/// 正向（AC-1 / AC-8）：新建元素落在父子树中段 ⇒ `moved >= 1` 且 `blocks` 含该 FQN，
/// 盘上文本的子树连续与之一致。
#[tokio::test]
async fn 元素写响应reordered非零形态() {
    let dir = healthy("reorder-element-moved");
    let context = with_protect(dir.path());
    let payload = element_write(
        &context,
        json!({
            "action": "upsert",
            "element": "App.Auth.Token",
            "kind": "module",
            "path": ["./src/auth/token/"],
        }),
    )
    .await;

    assert_eq!(payload["created"], json!(true), "新建分支");
    assert_eq!(
        payload["reordered"],
        json!({ "moved": 1, "blocks": ["App.Auth.Token"] }),
        "新元素收进父的子树区间即一块"
    );
    let text = fs::read_to_string(core_model_path(dir.path())).expect("读模型失败");
    let token = text
        .find("[element.\"App.Auth.Token\"]")
        .expect("新元素在盘");
    let billing = text.find("[element.\"App.Billing\"]").expect("兄弟在盘");
    assert!(token < billing, "盘上文本子树连续：\n{text}");
}

/// 正向（AC-3）：对写后文件重放同值 upsert ⇒ `reordered.moved = 0` 且盘上字节零变化。
#[tokio::test]
async fn 元素写同值重放零搬迁() {
    let dir = healthy("reorder-element-replay");
    let context = with_protect(dir.path());
    let first = element_write(
        &context,
        json!({
            "action": "upsert",
            "element": "App.Auth.Token",
            "kind": "module",
            "path": ["./src/auth/token/"],
        }),
    )
    .await;
    assert_eq!(first["reordered"]["moved"], json!(1));
    let bytes = fs::read(core_model_path(dir.path())).expect("读模型失败");

    // 重放省去 kind（在案元素给 kind 即 Err），其余字段同值
    let second = element_write(
        &context,
        json!({
            "action": "upsert",
            "element": "App.Auth.Token",
            "path": ["./src/auth/token/"],
        }),
    )
    .await;
    assert_eq!(
        second["reordered"],
        json!({ "moved": 0, "blocks": [] }),
        "重放 ⇒ 零搬迁"
    );
    assert_eq!(second["changed"], json!([]), "重放 ⇒ 零字段变化");
    assert_eq!(
        fs::read(core_model_path(dir.path())).expect("读模型失败"),
        bytes,
        "重放零字节变化"
    );
}

/// 正向（AC-5 / AC-8）：关系写造成 `from` 组分裂并回 ⇒ `moved >= 1`、`blocks` 为该
/// `from`，组连续化后文本序与 `reordered` 一致。
#[tokio::test]
async fn 关系写reordered搬迁语义() {
    let dir = model_root("reorder-relation", RELATIONS_DIRTY);
    let context = with_protect(dir.path());
    let payload = execute_json(
        &context,
        ARCHI_RELATION_TOOL,
        json!({ "action": "upsert", "from": "A", "to": "Z" }),
    )
    .await;

    assert_eq!(
        payload["reordered"],
        json!({ "moved": 1, "blocks": ["A"] }),
        "组并回即一块"
    );
    let text = fs::read_to_string(core_model_path(dir.path())).expect("读模型失败");
    let z = text.find("to = \"Z\"").expect("新边在盘");
    let y = text.find("to = \"Y\"").expect("B 组在盘");
    assert!(z < y, "A 组连续（新边紧随 A 组组尾）：\n{text}");
}

/// 边界（AC-8 / D-3）：>8 块搬迁夹具 ⇒ `moved` 为真值 `> 8`、`blocks` 恰 8 项、成功响应
/// 未触截断且 < 3 KB。
#[tokio::test]
async fn reordered块数上限与响应体量未触截断() {
    let dir = model_root("reorder-cap", &nine_block_model());
    let context = with_protect(dir.path());
    let text = context
        .execute(
            ARCHI_ELEMENT_TOOL,
            &json!({ "action": "upsert", "element": "P0", "gate": "off" }),
        )
        .await
        .expect("写应成功");
    assert!(!text.contains("已截断"), "成功响应未触截断留痕：{text}");
    assert!(text.len() < 3 * 1024, "成功响应 < 3 KB：{} B", text.len());

    let payload: Value = serde_json::from_str(&text).expect("结果为 JSON 串");
    assert_eq!(
        payload["reordered"]["moved"],
        json!(9),
        "moved 为块数真值（不截断）"
    );
    let blocks = payload["reordered"]["blocks"]
        .as_array()
        .expect("blocks 为数组");
    assert_eq!(blocks.len(), 8, "blocks 恰 8 项");
    assert_eq!(blocks[0], json!("P0"), "按交付文本序");
    assert!(
        text.len() < MAX_RESULT_BYTES,
        "仍经 cap_result 收口（未触上限）"
    );
}

/// 异常（AC-8 异常面）：各 `Err` 响应（值域 / 必填 / 端点 / 落点 / 乐观锁 / 保护闸）不
/// 出现 `reordered` 键。
#[tokio::test]
async fn 失败路径零reordered键() {
    let dir = healthy("reorder-errors");
    let context = with_protect(dir.path());
    for (tool, input) in [
        (
            ARCHI_ELEMENT_TOOL,
            json!({ "action": "modify", "element": "App.Auth" }),
        ),
        (ARCHI_ELEMENT_TOOL, json!({ "action": "upsert" })),
        (
            ARCHI_ELEMENT_TOOL,
            json!({ "action": "upsert", "element": "Orphan", "kind": "domain" }),
        ),
        (
            ARCHI_ELEMENT_TOOL,
            json!({ "action": "upsert", "element": "App.Auth", "summary": "x", "expect": "sha256:错值" }),
        ),
        (
            ARCHI_RELATION_TOOL,
            json!({ "action": "upsert", "from": "App.Auth", "to": "App.Legacy" }),
        ),
        (
            ARCHI_RELATION_TOOL,
            json!({ "action": "delete", "from": "App.Billing", "to": "App.Auth" }),
        ),
    ] {
        let error = context
            .execute(tool, &input)
            .await
            .expect_err("失败路径必须 Err");
        assert!(
            !error.contains("reordered"),
            "失败响应零 reordered 键（{tool}）：{error}"
        );
    }
}

/// 异常（AC-6）：脏夹具搬迁（`moved > 0`）的 `warnings` 与搬前同形——关系面恒空数组，
/// 元素面非 `path` / `api` 诱发的改动也不加项。
#[tokio::test]
async fn 搬迁不新增warnings条目() {
    let dir = model_root("reorder-warnings", RELATIONS_DIRTY);
    let context = with_protect(dir.path());
    let payload = execute_json(
        &context,
        ARCHI_RELATION_TOOL,
        json!({ "action": "upsert", "from": "A", "to": "Z" }),
    )
    .await;
    assert_eq!(payload["reordered"]["moved"], json!(1), "确有搬迁");
    assert_eq!(payload["warnings"], json!([]), "关系面 warnings 恒空数组");

    let element = element_write(
        &context,
        json!({ "action": "upsert", "element": "A", "gate": "warn" }),
    )
    .await;
    assert_eq!(
        element["warnings"],
        json!([]),
        "只改 gate ⇒ 零 warning（搬迁不诱发判据）"
    );
    assert_eq!(
        element["reordered"],
        json!({ "moved": 0, "blocks": [] }),
        "搬前已合规 ⇒ 本次零搬迁"
    );
}

/// 正向（组合链路，入口 = 工具执行体）：`archi_relation` upsert → `plan_relation_upsert`
/// → `layout::normalize` → 保护闸 → 原子写；盘上字节 == 计划的 `content` == 规范化序，
/// 响应 `moved` 与计划同源；再重放一次 ⇒ `moved = 0` 且盘上字节零变化。
#[tokio::test]
async fn 组合链路盘上文本等于计划文本且重放零搬迁() {
    let dir = model_root("reorder-chain", RELATIONS_DIRTY);
    let context = with_protect(dir.path());
    let file = core_model_path(dir.path());
    let source_before = fs::read_to_string(&file).expect("读模型失败");

    let planned = archi::edit::plan_relation_upsert(dir.path(), "A", "Z", None, None, None)
        .expect("同一计划应成功");
    assert!(planned.reordered.moved >= 1, "计划面已报搬迁");

    let payload = execute_json(
        &context,
        ARCHI_RELATION_TOOL,
        json!({ "action": "upsert", "from": "A", "to": "Z" }),
    )
    .await;
    assert_eq!(
        payload["reordered"]["moved"],
        json!(planned.reordered.moved),
        "响应 moved 与计划同源"
    );
    let after = fs::read_to_string(&file).expect("读模型失败");
    assert_eq!(after, planned.write.content, "盘上字节 == 计划的 content");
    assert_ne!(after, source_before, "首次规范化确实改了文本");

    let replay = execute_json(
        &context,
        ARCHI_RELATION_TOOL,
        json!({ "action": "upsert", "from": "A", "to": "Z" }),
    )
    .await;
    assert_eq!(
        replay["reordered"],
        json!({ "moved": 0, "blocks": [] }),
        "重放零搬迁"
    );
    assert_eq!(
        fs::read_to_string(&file).expect("读模型失败"),
        after,
        "重放零字节变化"
    );
}

// ===========================================================================
// 本变更：`archi_generate` 执行链（渲染 → 逐份保护 → 端口写盘）
//
// 零框架级 mock：渲染（`archi::workspace_render`）、保护判定（真实 `ProtectRules`）与
// 模型 / 接口 / 入口源文全为真件；**唯一替身是端口 `archi::ArtifactWriter`**（显式注入
// 依赖，只记录入参与回传构造值，不复制渲染 / 比对 / 写盘）。
// ===========================================================================

/// 端口应答形态（`Ok` 两列按需构造 / `Err` 原文）。
enum WriterResponse {
    /// `written` = 全部应生成物
    WrittenAll,
    /// `unchanged` = 全部应生成物
    UnchangedAll,
    /// 预置结果（越根错 / 写盘失败等）
    Fixed(Result<archi::ArchiApiWriteOutcome, String>),
}

/// 记录型端口替身：`Mutex` 记录每次 `(root, files)`，按应答队列回传（队列空 = `WrittenAll`）。
struct RecordingWriter {
    calls: std::sync::Mutex<Vec<(PathBuf, Vec<archi::GeneratedFile>)>>,
    responses: std::sync::Mutex<std::collections::VecDeque<WriterResponse>>,
}

impl RecordingWriter {
    fn new() -> Self {
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
            responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
        }
    }

    fn with_responses(responses: Vec<WriterResponse>) -> Self {
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
            responses: std::sync::Mutex::new(responses.into()),
        }
    }

    fn calls(&self) -> Vec<(PathBuf, Vec<archi::GeneratedFile>)> {
        self.calls.lock().expect("替身锁未中毒").clone()
    }

    fn call_count(&self) -> usize {
        self.calls.lock().expect("替身锁未中毒").len()
    }

    /// 交出的应生成物（repo 相对 POSIX 串，序即端口收到序）。
    fn handed(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .flat_map(|(root, files)| {
                files
                    .into_iter()
                    .map(move |file| archi::display(&root, &file.path))
                    .collect::<Vec<String>>()
            })
            .collect()
    }
}

impl archi::ArtifactWriter for RecordingWriter {
    fn write(
        &self,
        root: &Path,
        files: &[archi::GeneratedFile],
    ) -> Result<archi::ArchiApiWriteOutcome, String> {
        self.calls
            .lock()
            .expect("替身锁未中毒")
            .push((root.to_path_buf(), files.to_vec()));
        let relative = |file: &archi::GeneratedFile| archi::display(root, &file.path);
        let response = self
            .responses
            .lock()
            .expect("替身锁未中毒")
            .pop_front()
            .unwrap_or(WriterResponse::WrittenAll);
        match response {
            WriterResponse::WrittenAll => Ok(archi::ArchiApiWriteOutcome {
                written: files.iter().map(relative).collect(),
                unchanged: Vec::new(),
            }),
            WriterResponse::UnchangedAll => Ok(archi::ArchiApiWriteOutcome {
                written: Vec::new(),
                unchanged: files.iter().map(relative).collect(),
            }),
            WriterResponse::Fixed(result) => result,
        }
    }
}

/// 接入替身端口的上下文（真实规则集 + 显式注入的端口）。
fn with_writer(root: &Path, writer: &Arc<RecordingWriter>) -> ArchiContext {
    with_protect(root).with_writer(Some(writer.clone() as Arc<dyn archi::ArtifactWriter>))
}

/// 接口定义语料（1 op `file_stem` + 1 类型 `LocalGate`，带自我镜像 `$ref`：可过九道闸）。
const GENERATE_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "App.Api.Subject.schema.json",
  "title": "App.Api.Subject",
  "$comment": "手写接口真相：本文件是接口的唯一凭据，不是生成物",
  "$defs": { "LocalGate": { "type": "object", "properties": {} } },
  "properties": {
    "file_stem": {
      "x-function": {
        "owner": "LocalGate",
        "params": [{ "name": "timestamp", "schema": { "type": "string" } }],
        "returns": { "type": "string" }
      }
    },
    "LocalGate": { "$ref": "#/$defs/LocalGate" }
  }
}
"##;

/// 接口面留白的接口定义（`$defs` 与 `properties` 皆空 ⇒ 零 `.i.*` 产出）。
const EMPTY_SCHEMA: &str = concat!(
    "{\n",
    "  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n",
    "  \"$id\": \"App.Api.Subject.schema.json\",\n",
    "  \"title\": \"App.Api.Subject\",\n",
    "  \"$defs\": {},\n",
    "  \"properties\": {}\n",
    "}\n",
);

/// 入口源文（`pub fn file_stem` 与接口的 op 同名 ⇒ 抽取面与接口面可对拍）。
const GENERATE_ENTRY: &str = "pub fn file_stem(timestamp: &str) -> String { String::new() }\n";

/// 健康生成根：模型（容器 + 域 + 叶元素带 `path`）+ 接口定义 + 入口源文。
fn generate_root(tag: &str) -> tempfile::TempDir {
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
    write_file(dir.path(), "crates/api/subject/src/lib.rs", GENERATE_ENTRY);
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Api/Subject.schema.json",
        GENERATE_SCHEMA,
    );
    dir
}

/// 落一份用户写保护规则（绝对 glob：相对 glob 会按 `**/` 前缀归一，跨平台匹配面不稳）。
fn write_user_rules(root: &Path, rules: &[(&str, &str)]) {
    let root_text = root.to_string_lossy().replace('\\', "/");
    let files: Vec<Value> = rules
        .iter()
        .map(|(relative, reason)| {
            json!({ "glob": format!("{root_text}/{relative}"), "reason": reason })
        })
        .collect();
    let config = json!({ "write_protection": { "files": files } });
    let config_path = foundation::layout::config_path(root);
    fs::create_dir_all(config_path.parent().expect("配置有父目录")).expect("建配置目录失败");
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).expect("配置序列化"),
    )
    .expect("写配置文件失败");
}

/// 生成物写工具名（本体驻挂载清单）。
const GENERATE_TOOL: &str = crate::sdk::mount::ARCHI_GENERATE_TOOL;

/// 正向（AC-3）：健康根 → 响应三键齐备；替身端口收到的 `root` 与 `files` 与
/// `archi::workspace_render(root)` 的产出**逐字相等**（执行体零重渲染 / 零改写 / 零重排），
/// `written` 与端口回传值相等、`conflicts` 空。
#[tokio::test]
async fn 生成物写执行链三键齐备且渲染同源() {
    let dir = generate_root("generate-ok");
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);
    let expected = archi::workspace_render(dir.path()).expect("渲染应成功");
    assert!(!expected.is_empty(), "健康根应有应生成物");

    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;

    for key in ["written", "unchanged", "conflicts"] {
        assert!(
            value.get(key).is_some(),
            "响应三键齐备（含 `{key}`）：{value}"
        );
    }
    assert_eq!(value["conflicts"], json!([]), "无规则命中 ⇒ 冲突空");

    let calls = writer.calls();
    assert_eq!(calls.len(), 1, "端口恰被调用一次");
    assert_eq!(
        calls[0].0,
        dir.path(),
        "端口收到的 root = 上下文 root（会话 cwd）"
    );
    assert_eq!(
        calls[0].1, expected,
        "交端口的应生成物与 workspace_render 逐项逐字相等"
    );

    let expected_written: Vec<String> = expected
        .iter()
        .map(|file| archi::display(dir.path(), &file.path))
        .collect();
    assert_eq!(
        value["written"],
        json!(expected_written),
        "written 与端口回传值相等（执行体零改写）"
    );
    assert_eq!(value["unchanged"], json!([]), "首轮零 unchanged");
}

/// 边界（AC-5 校准口径的形态面）：① 无 `.schema.json` 的模型根 ⇒ `files` 仅 `INDEX.toml`
/// 一份；② 三元素模型（一元素有 op / 一元素 `path` 空 / 一元素接口留白）⇒ `files` 恰
/// `1 + N` 份且 `path` 空的元素零条目；两例端口均被调用一次。
#[tokio::test]
async fn 生成物应生成集两形态() {
    // ① 无接口文件
    let bare = tempdir("generate-bare");
    write_model(
        bare.path(),
        "01-api.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Api\"]\nkind = \"domain\"\ngate = \"off\"\n\n",
            "[element.\"App.Api.Subject\"]\nkind = \"module\"\n",
            "path = [\"./crates/api/subject/\"]\n",
        ),
    );
    write_file(bare.path(), "crates/api/subject/src/lib.rs", GENERATE_ENTRY);
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(bare.path(), &writer);
    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;
    assert_eq!(writer.call_count(), 1, "端口被调用一次");
    let handed = writer.handed();
    assert_eq!(handed.len(), 1, "应生成集只一份：{handed:?}");
    assert!(handed[0].ends_with("INDEX.toml"), "该份是索引：{handed:?}");
    assert_eq!(value["conflicts"], json!([]), "无规则命中");

    // ② 三元素：有 op / path 空 / 接口留白
    let mixed = tempdir("generate-mixed");
    write_model(
        mixed.path(),
        "01-api.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.One\"]\nkind = \"module\"\npath = [\"./crates/api/one/\"]\n\n",
            "[element.\"App.Two\"]\nkind = \"module\"\npath = [\"./crates/api/two/\"]\n\n",
            "[element.\"App.Three\"]\nkind = \"module\"\npath = [\"./crates/api/three/\"]\n",
        ),
    );
    for name in ["one", "two", "three"] {
        write_file(
            mixed.path(),
            &format!("crates/api/{name}/src/lib.rs"),
            GENERATE_ENTRY,
        );
    }
    // One：有 op 的接口（产 `.i.rs`）；Two：接口留白（零 `.i.*`）；Three：无接口文件
    write_file(
        mixed.path(),
        ".dev-team/architecture/models/App/One.schema.json",
        &GENERATE_SCHEMA.replace("App.Api.Subject", "App.One"),
    );
    write_file(
        mixed.path(),
        ".dev-team/architecture/models/App/Two.schema.json",
        &EMPTY_SCHEMA.replace("App.Api.Subject", "App.Two"),
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(mixed.path(), &writer);
    let _ = context.execute(GENERATE_TOOL, &json!({})).await;
    let handed = writer.handed();
    assert_eq!(handed.len(), 2, "恰 1 份 `.i.rs` + 1 份索引：{handed:?}");
    assert_eq!(
        handed
            .iter()
            .filter(|path| path.contains("/src/lib.i.rs"))
            .count(),
        1,
        "只有有 op 的元素产 `.i.rs`：{handed:?}"
    );
    assert!(
        handed
            .iter()
            .any(|path| path.ends_with("crates/api/one/src/lib.i.rs")),
        "有 op 的元素产 `.i.rs`：{handed:?}"
    );
    assert!(
        !handed.iter().any(|path| path.contains("api/two/")),
        "接口留白的元素零条目：{handed:?}"
    );
    assert!(
        !handed.iter().any(|path| path.contains("api/three/")),
        "无接口文件的元素零条目：{handed:?}"
    );
}

/// 异常：装配缺失两态——① `with_writer(None)` ⇒ `Err` 且文案点名装配缺失；②
/// `protect == None` ⇒ `Err`（fail-closed：无规则集不等于无规则）；两例端口零调用、磁盘
/// 零变化。
#[tokio::test]
async fn 生成物写装配缺失两态() {
    let dir = generate_root("generate-unwired");
    let before = snapshot(dir.path());

    // ① 端口缺席（规则集仍在）
    let error = with_protect(dir.path())
        .execute(GENERATE_TOOL, &json!({}))
        .await
        .expect_err("端口未装配必须 Err");
    assert!(
        error.contains("写盘端口未装配") && error.contains(GENERATE_TOOL),
        "文案点名装配缺失与工具名：{error}"
    );

    // ② 规则集缺席（端口仍在）
    let writer = Arc::new(RecordingWriter::new());
    let unwired = ArchiContext::new(dir.path().to_path_buf())
        .with_writer(Some(writer.clone() as Arc<dyn archi::ArtifactWriter>));
    let error = unwired
        .execute(GENERATE_TOOL, &json!({}))
        .await
        .expect_err("规则集缺席必须 Err（fail-closed）");
    assert!(
        error.contains("写入保护规则未装配"),
        "文案点名规则集缺失：{error}"
    );
    assert_eq!(writer.call_count(), 0, "两例端口零调用");
    assert_eq!(snapshot(dir.path()), before, "两例磁盘零变化");
}

/// 正向（AC-15）：同一根连续两次 `execute`——两次交给端口的 `files` 逐项字节相等（执行体零
/// 缓存、零跨调用状态）；替身第一次回 `written = all`、第二次回 `unchanged = all`，两轮响应
/// 如实转出（幂等判据单点归写盘实现，执行体不做第二份字节比对）。
#[tokio::test]
async fn 生成物写两次调用字节相同且幂等如实转出() {
    let dir = generate_root("generate-idempotent");
    let writer = Arc::new(RecordingWriter::with_responses(vec![
        WriterResponse::WrittenAll,
        WriterResponse::UnchangedAll,
    ]));
    let context = with_writer(dir.path(), &writer);
    let expected = archi::workspace_render(dir.path()).expect("渲染应成功");

    let first = execute_json(&context, GENERATE_TOOL, json!({})).await;
    let second = execute_json(&context, GENERATE_TOOL, json!({})).await;

    let calls = writer.calls();
    assert_eq!(calls.len(), 2, "两次调用各交端口一次");
    assert_eq!(calls[0].1, calls[1].1, "两次交给端口的 files 逐项相等");
    assert_eq!(calls[0].1, expected, "执行体零重渲染（与渲染单点同源）");

    assert!(
        !first["written"].as_array().expect("数组").is_empty(),
        "第一轮如实转出 written：{first}"
    );
    assert_eq!(first["unchanged"], json!([]), "第一轮零 unchanged");
    assert_eq!(second["written"], json!([]), "第二轮零 written（如实转出）");
    assert!(
        !second["unchanged"].as_array().expect("数组").is_empty(),
        "第二轮全部 unchanged：{second}"
    );
}

/// 边界（AC-15）：生成物已被手改成旧内容（盘上字节 ≠ 应生成物）⇒ 执行体不预判「已新鲜」、
/// 仍整批交端口（比对权只在写盘单点）。
#[tokio::test]
async fn 生成物写不预判新鲜度仍整批交端口() {
    let dir = generate_root("generate-stale");
    let stale = "// 手改过的旧内容\n";
    write_file(dir.path(), "crates/api/subject/src/lib.i.rs", stale);
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);

    let expected = archi::workspace_render(dir.path()).expect("渲染应成功");
    let _ = execute_json(&context, GENERATE_TOOL, json!({})).await;

    let calls = writer.calls();
    assert_eq!(calls.len(), 1, "仍整批交端口一次");
    assert_eq!(calls[0].1, expected, "旧内容的文件亦在批内（零预判）");
    assert!(
        calls[0]
            .1
            .iter()
            .any(|file| file.path.ends_with("lib.i.rs")),
        "旧内容的该份在批内"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("crates/api/subject/src/lib.i.rs"))
            .expect("读生成物失败"),
        stale,
        "执行体不写盘（写盘归端口实现）"
    );
}

/// 异常（AC-15）：端口返回 `Err("写入失败（…）不回滚")` ⇒ 工具 `Err`（`is_error`）、响应不
/// 出现三键；端口恰一次调用（`Err` 前无第二次写盘尝试）。
#[tokio::test]
async fn 生成物写端口err原文透传且零重试() {
    let dir = generate_root("generate-port-err");
    let message = "写入失败（crates/api/subject/src/lib.i.rs）：父路径被同名文件占位，此前已写入 1 份，不回滚";
    let writer = Arc::new(RecordingWriter::with_responses(vec![
        WriterResponse::Fixed(Err(message.to_owned())),
    ]));
    let context = with_writer(dir.path(), &writer);

    let error = context
        .execute(GENERATE_TOOL, &json!({}))
        .await
        .expect_err("端口 Err ⇒ 工具 Err（不吞、不改写成 Ok 冲突）");
    assert_eq!(error, message, "Err 原文透传（执行体零包装）");
    assert!(
        !error.contains("conflicts"),
        "Err 路径不产出三键响应：{error}"
    );
    assert_eq!(writer.call_count(), 1, "Err 前无第二次写盘尝试");
}

// ---------------------------------------------------------------------------
// 逐份保护判定与整批拒绝
// ---------------------------------------------------------------------------

/// 正向：用户规则命中应生成的一份 `.i.rs` ⇒ `conflicts` 恰含该项（`path` 为 repo 相对
/// POSIX、`reason` 取用户规则文案且 `%t` = 工具名），`written` / `unchanged` 皆空数组、端口
/// 零调用、工具仍 `Ok`、磁盘零变化。
#[tokio::test]
async fn 生成物写命中保护即整批拒绝() {
    let dir = generate_root("generate-conflict");
    write_user_rules(
        dir.path(),
        &[("crates/api/subject/src/lib.i.rs", "生成物只经 %t 写（%s）")],
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);
    let before = snapshot(dir.path());

    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;

    let conflicts = value["conflicts"].as_array().expect("冲突为数组");
    assert_eq!(conflicts.len(), 1, "恰一条冲突：{value}");
    assert_eq!(
        conflicts[0]["path"],
        json!("crates/api/subject/src/lib.i.rs"),
        "路径为 repo 相对 POSIX"
    );
    let reason = conflicts[0]["reason"].as_str().expect("reason 为字符串");
    assert!(
        reason.contains("生成物只经") && reason.contains(GENERATE_TOOL),
        "拒串取用户文案且 %t 已替换为工具名：{reason}"
    );
    assert!(
        reason.contains("crates/api/subject/src/lib.i.rs"),
        "拒串含被拒路径（%s 已替换）：{reason}"
    );
    assert!(
        !reason.contains("%s") && !reason.contains("%t"),
        "零占位残留"
    );
    assert_eq!(value["written"], json!([]), "written 空数组");
    assert_eq!(value["unchanged"], json!([]), "unchanged 空数组");
    assert_eq!(writer.call_count(), 0, "整批拒绝 ⇒ 端口零调用");
    assert_eq!(snapshot(dir.path()), before, "磁盘零变化");
}

/// 边界：两份命中 ⇒ `conflicts` 两条且序与 `files` 序一致（不排序、不去重）；唯一一份
/// `INDEX.toml` 命中 ⇒ 同款整批拒绝（零 `.i.*` 写盘）；同 fixture 去规则 ⇒ 零 `conflicts`、
/// 端口被调用（射程不误伤）。
#[tokio::test]
async fn 生成物写冲突序与索引单份形态() {
    let dir = generate_root("generate-conflict-order");
    write_user_rules(
        dir.path(),
        &[
            ("crates/api/subject/src/lib.i.rs", "写保护甲"),
            (".dev-team/architecture/graph/INDEX.toml", "写保护乙"),
        ],
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);
    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;
    let paths: Vec<String> = value["conflicts"]
        .as_array()
        .expect("数组")
        .iter()
        .map(|entry| entry["path"].as_str().expect("path").to_owned())
        .collect();
    assert_eq!(
        paths,
        vec![
            "crates/api/subject/src/lib.i.rs".to_owned(),
            ".dev-team/architecture/graph/INDEX.toml".to_owned(),
        ],
        "冲突序与 files 序一致（不排序、不去重）"
    );
    assert_eq!(writer.call_count(), 0, "两份命中 ⇒ 整批零写盘");

    // 唯一一份 INDEX.toml 命中：同款整批拒绝
    let only_index = generate_root("generate-conflict-index");
    write_user_rules(
        only_index.path(),
        &[(".dev-team/architecture/graph/INDEX.toml", "索引只经工具写")],
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(only_index.path(), &writer);
    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;
    assert_eq!(
        value["conflicts"].as_array().expect("数组").len(),
        1,
        "索引单份命中即整批拒绝：{value}"
    );
    assert_eq!(writer.call_count(), 0, "零 `.i.*` 写盘（整批拒绝）");

    // 同 fixture 去规则：射程不误伤
    let clean = generate_root("generate-conflict-clean");
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(clean.path(), &writer);
    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;
    assert_eq!(value["conflicts"], json!([]), "无规则 ⇒ 零冲突");
    assert_eq!(writer.call_count(), 1, "无规则 ⇒ 端口被调用");
}

/// 异常（反向锚）：`archi_generate` 无 `path` 入参仍须在保护射程内——拒绝串含工具名与路径
/// （证明判定不靠入参 `path`）；保护判定零第二份规则集（源码锚：执行体内只出现
/// `evaluate_target`，零自建 glob / 零自拼 reason）。
#[tokio::test]
async fn 生成物写保护判定不靠入参且单点() {
    let dir = generate_root("generate-conflict-tool-name");
    write_user_rules(
        dir.path(),
        &[("crates/api/subject/src/lib.i.rs", "仅 %t 可写：%s")],
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);

    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;
    let reason = value["conflicts"][0]["reason"]
        .as_str()
        .expect("reason 为字符串")
        .to_owned();
    assert!(
        reason.contains(GENERATE_TOOL) && reason.contains("crates/api/subject/src/lib.i.rs"),
        "拒串含工具名与路径（判定不靠入参 path）：{reason}"
    );

    let source = include_str!("archi_tool.rs");
    let start = source.find("fn generate_payload(").expect("生成臂在案");
    let body = source[start..]
        .split("\n    /// ")
        .next()
        .expect("函数体收口");
    assert_eq!(
        body.matches("evaluate_target(").count(),
        1,
        "保护判定只经单点 evaluate_target"
    );
    for stray in ["globset", "Glob::new", "match_glob", "%s", "%t"] {
        assert!(
            !body.contains(stray),
            "零第二份规则集 / 零自拼 reason：`{stray}`"
        );
    }
}

// ---------------------------------------------------------------------------
// 零产出三成因（AC-5 校准口径）+ 响应形状 + `remedy` 三态 + 写作范围封闭
// ---------------------------------------------------------------------------

/// 判定「应生成集里零 `.i.*` 条目」（零产出的唯一可断言形态；`INDEX.toml` 恒产出）。
fn assert_no_incremental_generated(written: &Value, unchanged: &Value) {
    for column in [written, unchanged] {
        for entry in column.as_array().expect("两列恒为数组") {
            let path = entry.as_str().expect("条目为字符串");
            assert!(
                !path.contains(".i.rs") && !path.contains(".i.ts"),
                "零产出成因下 `.i.*` 零条目：{path}"
            );
        }
    }
}

/// 正向（AC-5 校准口径）：三例各自调用——① 未写 `.schema.json`；② 接口面留白
/// （`$defs {}` + `properties {}`）；③ 有 op 但入口不在盘：三例 `is_error` 均为假、两列零
/// `.i.*` 条目（`INDEX.toml` 按端口回传落位）、磁盘零变化（替身不触盘）。
#[tokio::test]
async fn 零产出三成因均为正常() {
    // ① 未写接口定义
    let no_schema = tempdir("generate-zero-no-schema");
    write_model(
        no_schema.path(),
        "01-api.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.Api\"]\nkind = \"domain\"\ngate = \"off\"\n\n\
         [element.\"App.Api.Subject\"]\nkind = \"module\"\npath = [\"./crates/api/subject/\"]\n",
    );
    write_file(
        no_schema.path(),
        "crates/api/subject/src/lib.rs",
        GENERATE_ENTRY,
    );

    // ② 接口面留白
    let blank = generate_root("generate-zero-blank");
    write_file(
        blank.path(),
        ".dev-team/architecture/models/App/Api/Subject.schema.json",
        EMPTY_SCHEMA,
    );

    // ③ 有 op 但入口不在盘
    let no_entry = tempdir("generate-zero-no-entry");
    write_model(
        no_entry.path(),
        "01-api.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.Api\"]\nkind = \"domain\"\ngate = \"off\"\n\n\
         [element.\"App.Api.Subject\"]\nkind = \"module\"\npath = [\"./crates/api/subject/\"]\n",
    );
    write_file(
        no_entry.path(),
        ".dev-team/architecture/models/App/Api/Subject.schema.json",
        GENERATE_SCHEMA,
    );

    for (label, dir) in [
        ("未写 .schema.json", no_schema),
        ("接口面留白", blank),
        ("入口不在盘", no_entry),
    ] {
        let writer = Arc::new(RecordingWriter::new());
        let context = with_writer(dir.path(), &writer);
        let before = snapshot(dir.path());
        let value = execute_json(&context, GENERATE_TOOL, json!({})).await;

        assert_eq!(
            value["conflicts"],
            json!([]),
            "{label}：零冲突（零产出不是冲突）"
        );
        assert_no_incremental_generated(&value["written"], &value["unchanged"]);
        assert_eq!(writer.call_count(), 1, "{label}：端口被调用一次");
        assert_eq!(
            snapshot(dir.path()),
            before,
            "{label}：磁盘零变化（替身不触盘）"
        );
        let handed = writer.handed();
        assert_eq!(handed.len(), 1, "{label}：应生成集只一份索引：{handed:?}");
        assert!(
            handed[0].ends_with("INDEX.toml"),
            "{label}：该份即索引：{handed:?}"
        );
    }
}

/// 边界：三类成因同株并存（三元素各占一因）⇒ 一份 `.i.*` 都不产生、零 `Err`、`conflicts`
/// 空；零元素模型根 ⇒ `files` 恰一份 `INDEX.toml`。
#[tokio::test]
async fn 零产出同株并存与零元素根() {
    let dir = tempdir("generate-zero-mixed");
    write_model(
        dir.path(),
        "01-api.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            // ① 未写接口定义
            "[element.\"App.One\"]\nkind = \"module\"\npath = [\"./crates/api/one/\"]\n\n",
            // ② 接口面留白
            "[element.\"App.Two\"]\nkind = \"module\"\npath = [\"./crates/api/two/\"]\n\n",
            // ③ 有 op 但入口不在盘
            "[element.\"App.Three\"]\nkind = \"module\"\npath = [\"./crates/api/three/\"]\n",
        ),
    );
    for name in ["one", "two"] {
        write_file(
            dir.path(),
            &format!("crates/api/{name}/src/lib.rs"),
            GENERATE_ENTRY,
        );
    }
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Two.schema.json",
        &EMPTY_SCHEMA.replace("App.Api.Subject", "App.Two"),
    );
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Three.schema.json",
        &GENERATE_SCHEMA.replace("App.Api.Subject", "App.Three"),
    );

    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);
    let value = execute_json(&context, GENERATE_TOOL, json!({})).await;
    assert_no_incremental_generated(&value["written"], &value["unchanged"]);
    assert_eq!(value["conflicts"], json!([]), "零冲突");
    let handed = writer.handed();
    assert_eq!(handed.len(), 1, "只一份索引：{handed:?}");

    // 零元素模型根
    let empty = tempdir("generate-zero-empty");
    write_model(
        empty.path(),
        "01-api.toml",
        "[element.\"App\"]\nkind = \"package\"\n",
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(empty.path(), &writer);
    let _ = execute_json(&context, GENERATE_TOOL, json!({})).await;
    let handed = writer.handed();
    assert_eq!(handed.len(), 1, "零元素根也只一份索引：{handed:?}");
    assert!(handed[0].ends_with("INDEX.toml"), "{handed:?}");
}

/// 异常：坏模型（TOML 语法错）⇒ `Err` 且诊断带 `file:line`（与「零产出」严格可分）、端口
/// 零调用、磁盘零变化。
#[tokio::test]
async fn 坏模型与零产出严格可分() {
    let dir = tempdir("generate-broken-model");
    write_model(
        dir.path(),
        "01-broken.toml",
        "[element.\"App]\nkind = \"package\"\n",
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);
    let before = snapshot(dir.path());

    let error = context
        .execute(GENERATE_TOOL, &json!({}))
        .await
        .expect_err("坏模型必须 Err（零产出才是正常）");
    assert!(
        error.contains("01-broken.toml:") || error.contains("01-broken.toml @ "),
        "诊断带 file:line：{error}"
    );
    assert_eq!(writer.call_count(), 0, "渲染失败 ⇒ 端口零调用");
    assert_eq!(snapshot(dir.path()), before, "磁盘零变化");
}

/// 正向：三键恒在场（`conflicts` 非空时 `written` / `unchanged` 仍在且为 `[]`）；
/// `written` / `unchanged` 串均 repo 相对 POSIX（无 root 前缀、无反斜杠、无 `./` 前缀）。
#[test]
fn 响应形状三字段且路径相对形态() {
    // 该用例的断言面纯字符串形态（不触盘）：以渲染单点的产出形态为准
    let dir = generate_root("generate-shape");
    let files = archi::workspace_render(dir.path()).expect("渲染应成功");
    assert!(!files.is_empty(), "夹具应有应生成物");
    for file in &files {
        let display = archi::display(dir.path(), &file.path);
        assert!(!display.contains('\\'), "无正斜杠归一后的反斜杠：{display}");
        assert!(!display.starts_with("./"), "无 `./` 前缀：{display}");
        let root_text = dir.path().to_string_lossy().replace('\\', "/");
        assert!(
            !display.starts_with(&root_text),
            "无 root 前缀（repo 相对）：{display}"
        );
        assert!(
            display.ends_with("INDEX.toml")
                || display.ends_with(".i.rs")
                || display.ends_with(".i.ts"),
            "条目恒为三类生成物之一：{display}"
        );
    }
}

/// 边界：空结果序列化为 `[]` 而非 `null`；`conflicts` 每项恒含 `path` + `reason` 两键（零
/// 第三键、零嵌套 DTO）。
#[tokio::test]
async fn 响应形状空数组与冲突两键() {
    let dir = generate_root("generate-shape-empty");
    write_user_rules(
        dir.path(),
        &[("crates/api/subject/src/lib.i.rs", "形状面拒绝串")],
    );
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);
    let text = context
        .execute(GENERATE_TOOL, &json!({}))
        .await
        .expect("冲突不是 Err");
    let value: Value = serde_json::from_str(&text).expect("JSON 串");
    assert!(
        text.contains("\"written\":[]") && text.contains("\"unchanged\":[]"),
        "空结果序列化为 `[]` 而非 `null`：{text}"
    );
    let conflict = value["conflicts"][0].as_object().expect("冲突为对象");
    let mut keys: Vec<&str> = conflict.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["path", "reason"], "冲突项恒两键（零第三键）");
    assert!(
        conflict.values().all(|value| !value.is_object()),
        "零嵌套 DTO：{conflict:?}"
    );
}

/// 异常：端口返回越根错（`Err`）⇒ 工具 `Err` 原文透传（执行体不吞错、不改写为 `Ok` 冲突）；
/// 输出仍过 `tools::cap_result` 单点（源码锚：零第二份截断实现）。
#[tokio::test]
async fn 响应形状越根错透传且过同一收口() {
    let dir = generate_root("generate-shape-escape");
    let message = "写入失败（../outside.i.rs）：目标越出观察根（/repo），零写盘";
    let writer = Arc::new(RecordingWriter::with_responses(vec![
        WriterResponse::Fixed(Err(message.to_owned())),
    ]));
    let context = with_writer(dir.path(), &writer);

    let error = context
        .execute(GENERATE_TOOL, &json!({}))
        .await
        .expect_err("越根错必须 Err");
    assert_eq!(error, message, "原文透传（零改写为 Ok 冲突）");
    assert!(!error.contains("conflicts"), "不被包装成三键响应：{error}");

    let source = include_str!("archi_tool.rs");
    assert_eq!(
        source.matches("tools::cap_result(").count(),
        1,
        "输出收口单点（零第二份截断实现）"
    );
}

// ---------------------------------------------------------------------------
// `archi_api(check)` 的 `remedy` 三态（AC-6）
// ---------------------------------------------------------------------------

/// 正向：三列全空（零差异根）⇒ 响应含顶层 `remedy` 键且值为 `null`；`artifacts` /
/// `findings` 两轴键与值逐字不受影响。
#[tokio::test]
async fn 复检零差异时remedy为null且两轴不变() {
    let dir = generate_root("generate-remedy-none");
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);
    // 先把应生成物落到盘上（测试侧直接写文件——本 crate 无写盘单点），再复检得零差异
    let files = archi::workspace_render(dir.path()).expect("渲染应成功");
    for file in &files {
        let target = dir.path().join(archi::display(dir.path(), &file.path));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("建生成物目录失败");
        }
        fs::write(&target, &file.content).expect("写生成物失败");
    }

    let value = execute_json(
        &context,
        crate::sdk::mount::ARCHI_API_TOOL,
        json!({ "action": "check" }),
    )
    .await;
    assert!(value.get("remedy").is_some(), "`remedy` 键恒在场：{value}");
    assert_eq!(value["remedy"], Value::Null, "三列全空 ⇒ null");
    assert_eq!(
        value["artifacts"]["mismatched"],
        json!([]),
        "artifacts 轴内容不受 remedy 影响：{value}"
    );
    assert!(
        value["findings"].get("total").is_some(),
        "findings 轴键齐备：{value}"
    );
}

/// 边界：有差异 ∧ `generate == true` ⇒ `remedy` 为短串，含 `archi_generate` 与「MUST NOT 手改
/// 生成物」；同一根换 `generate == false` ⇒ 短串指向架构页会话且含「MUST NOT 手改生成物」、
/// 不承诺调用（无「调 `archi_generate`」字样）；两例 `artifacts` 三列内容相同。
#[tokio::test]
async fn remedy两态按generate位切换() {
    let dir = generate_root("generate-remedy-two");
    let available = with_writer(dir.path(), &Arc::new(RecordingWriter::new())).with_generate(true);
    let unavailable =
        with_writer(dir.path(), &Arc::new(RecordingWriter::new())).with_generate(false);

    let first = execute_json(
        &available,
        crate::sdk::mount::ARCHI_API_TOOL,
        json!({ "action": "check" }),
    )
    .await;
    let second = execute_json(
        &unavailable,
        crate::sdk::mount::ARCHI_API_TOOL,
        json!({ "action": "check" }),
    )
    .await;

    let remedy = first["remedy"].as_str().expect("有差异 ⇒ 非 null");
    assert!(
        remedy.contains("archi_generate"),
        "第二态指向该工具：{remedy}"
    );
    assert!(
        remedy.contains("MUST NOT 手改生成物"),
        "第二态载手改禁令：{remedy}"
    );

    let remedy = second["remedy"].as_str().expect("有差异 ⇒ 非 null");
    assert!(
        remedy.contains("架构页会话"),
        "第三态指向架构页会话：{remedy}"
    );
    assert!(
        remedy.contains("MUST NOT 手改生成物"),
        "第三态载手改禁令：{remedy}"
    );
    assert!(
        !remedy.contains("调 `archi_generate`"),
        "第三态不承诺未下发工具：{remedy}"
    );

    assert_eq!(
        first["artifacts"], second["artifacts"],
        "两态 artifacts 三列内容相同（remedy 不改两轴）"
    );
    assert_eq!(first["findings"], second["findings"], "两态 findings 相同");
}

/// 异常：三态之外的输入面——`artifacts.enabled = false`（接口面未启用）时 `remedy` 仍按
/// 「三列是否全空」判定（不新造第四态）；响应体长度不因 `remedy` 触发截断（短串）。
#[tokio::test]
async fn remedy不新造第四态且不触发截断() {
    // 未启用接口面（无任何 `<叶段>.schema.json`）⇒ 差异只有索引缺失
    let dir = tempdir("generate-remedy-disabled");
    write_model(
        dir.path(),
        "01-api.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.Api\"]\nkind = \"domain\"\ngate = \"off\"\n\n\
         [element.\"App.Api.Subject\"]\nkind = \"module\"\npath = [\"./crates/api/subject/\"]\n",
    );
    write_file(dir.path(), "crates/api/subject/src/lib.rs", GENERATE_ENTRY);
    let context = with_writer(dir.path(), &Arc::new(RecordingWriter::new())).with_generate(true);
    let value = execute_json(
        &context,
        crate::sdk::mount::ARCHI_API_TOOL,
        json!({ "action": "check" }),
    )
    .await;
    assert_eq!(value["artifacts"]["enabled"], json!(false), "接口面未启用");
    assert!(
        value["remedy"].as_str().is_some(),
        "有差异 ⇒ 第二态（不新造第四态）：{value}"
    );

    let text = serde_json::to_string(&value).expect("序列化");
    assert!(
        text.len() < crate::sdk::tools::MAX_RESULT_BYTES,
        "响应体未被截断（remedy 是短串）"
    );
    assert!(!text.contains("已截断"), "零截断留痕");
}

// ---------------------------------------------------------------------------
// 写作范围封闭与零第二份实现（源码锚）
// ---------------------------------------------------------------------------

/// 正向：`archi_tool.rs` 内零 `fs::write` / 零 `create_dir_all` / 零 `std::process`；生成臂只
/// 串接 `archi::workspace_render` → `ProtectRules::evaluate_target` → `writer.write` 三个既有
/// 单点（零 TOML 拼接、零模型写回、零契约写入）。
#[test]
fn 生成臂写作范围封闭源码锚() {
    let source = include_str!("archi_tool.rs");
    // 文档注释里的禁令句（「MUST NOT 出现第二份生成物 `fs::write`」）不计入代码面判据
    let code = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "fs::write",
        "create_dir_all",
        "std::process",
        "std::fs::",
        "Command::new",
    ] {
        assert!(
            !code.contains(forbidden),
            "执行体零写盘 / 零 spawn：`{forbidden}`"
        );
    }
    let start = source.find("fn generate_payload(").expect("生成臂在案");
    let body = source[start..]
        .split("\n    /// ")
        .next()
        .expect("函数体收口");
    for single_point in [
        "archi::workspace_render(",
        ".evaluate_target(",
        "writer.write(",
    ] {
        assert_eq!(
            body.matches(single_point).count(),
            1,
            "三个既有单点各恰一处：`{single_point}`"
        );
    }
    for stray in [
        "plan_element_upsert",
        "plan_relation_upsert",
        "toml::",
        "validate_schema(",
    ] {
        assert!(
            !body.contains(stray),
            "零 TOML 拼接 / 零模型写回 / 零契约写入：`{stray}`"
        );
    }
}

/// 边界：模块头与 `execute` doc 的臂清单计数由「七支 / 五只读 + 两写」改「八支 / 五只读 +
/// 三写（两模型写 + 一生成物写）」，且与 `execute` match 的实际臂数一致（八臂 + `other` 兜底）。
#[test]
fn 生成臂计数与doc一致() {
    let source = include_str!("archi_tool.rs");
    assert!(
        source.contains("五只读 + 三写（两模型写 + 一生成物写）"),
        "模块头口径随动"
    );
    assert!(
        source.contains("八支共用同一 per-turn 上下文"),
        "模块头臂数随动"
    );
    assert!(
        !source.contains("五只读 + 两写"),
        "旧口径「五只读 + 两写」零命中"
    );

    let start = source
        .find("pub(crate) async fn execute(")
        .expect("execute 在案");
    let body = source[start..]
        .split("\n    /// ")
        .next()
        .expect("函数体收口");
    assert_eq!(
        body.matches("=> self.").count(),
        8,
        "match 实际八臂（五只读 + 三写）：{body}"
    );
    assert!(
        body.contains("other => return Err("),
        "未知名兜底臂在场：{body}"
    );
}

/// 异常：未知名仍 `Err`（第八臂不吞 `archi_generate_x` / 空串）；`archi_generate` 不写
/// `<叶段>.schema.json` / `<叶段>.md`（写前写后两文件字节与 `mtime` 逐字不变）。
#[tokio::test]
async fn 生成物写不动契约资产且未知名仍err() {
    let dir = generate_root("generate-scope");
    let writer = Arc::new(RecordingWriter::new());
    let context = with_writer(dir.path(), &writer);

    for unknown in ["archi_generate_x", "", "archi_generat"] {
        let error = context
            .execute(unknown, &json!({}))
            .await
            .expect_err("未知名必须 Err");
        assert!(
            error.contains("未知架构工具") && error.contains(unknown),
            "文案含未知名 `{unknown}`：{error}"
        );
    }
    assert_eq!(writer.call_count(), 0, "未知名零端口调用");

    let schema = dir
        .path()
        .join(".dev-team/architecture/models/App/Api/Subject.schema.json");
    let doc = dir
        .path()
        .join(".dev-team/architecture/models/App/Api/Subject.md");
    write_file(
        dir.path(),
        ".dev-team/architecture/models/App/Api/Subject.md",
        "# App.Api.Subject\n\n## 职能\n\n生成器械语料。\n",
    );
    let schema_before = fs::read(&schema).expect("读接口定义失败");
    let doc_before = fs::read(&doc).expect("读文档失败");
    let schema_mtime = fs::metadata(&schema)
        .expect("接口元数据")
        .modified()
        .expect("mtime");
    let doc_mtime = fs::metadata(&doc)
        .expect("文档元数据")
        .modified()
        .expect("mtime");

    let _ = context
        .execute(GENERATE_TOOL, &json!({}))
        .await
        .expect("生成应 Ok");

    assert_eq!(
        fs::read(&schema).expect("读接口定义失败"),
        schema_before,
        "接口字节不变"
    );
    assert_eq!(
        fs::read(&doc).expect("读文档失败"),
        doc_before,
        "文档字节不变"
    );
    assert_eq!(
        fs::metadata(&schema)
            .expect("接口元数据")
            .modified()
            .expect("mtime"),
        schema_mtime,
        "接口 mtime 不变"
    );
    assert_eq!(
        fs::metadata(&doc)
            .expect("文档元数据")
            .modified()
            .expect("mtime"),
        doc_mtime,
        "文档 mtime 不变"
    );
}
