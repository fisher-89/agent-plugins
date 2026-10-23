//! `explore_note`（`create_explore_note` 专用工具执行体）的单元测试
//!（auto-named-explore AC-3 / AC-6）：占位记录两步原子定名、已定名补建与异
//! 名拦截、前置校验零残留、失败面恢复指引。真实组合不 mock——native_db
//! workspace 库 + 文件系统 + `foundation::layout` 探索目录派生均为 tempfile
//! RAII 临时环境；`ExploreNoteContext` 为被测 API 显式构造入参。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use store::{ExploreRecord, Store};

use crate::sdk::explore_note::ExploreNoteContext;

// ---------------------------------------------------------------------------
// 装置：tempfile 双环境（workspace 树 + 库文件）+ 占位 / 已定名记录底座
// ---------------------------------------------------------------------------

struct NoteEnv {
    /// workspace 文件树根（RAII 清理）
    _ws: tempfile::TempDir,
    /// 库文件目录（RAII 清理）
    _db: tempfile::TempDir,
    store: Arc<Store>,
    root: PathBuf,
}

impl NoteEnv {
    fn new(tag: &str) -> Self {
        let ws = tempfile::Builder::new()
            .prefix(&format!("explore-note-test-{tag}-ws-"))
            .tempdir()
            .expect("创建 workspace 临时目录失败");
        let db = tempfile::Builder::new()
            .prefix(&format!("explore-note-test-{tag}-db-"))
            .tempdir()
            .expect("创建库目录临时目录失败");
        let store = Arc::new(
            Store::open_workspace(&db.path().join("ws.redb"))
                .unwrap_or_else(|e| panic!("打开临时 workspace 库失败: {e}")),
        );
        let root = ws.path().to_path_buf();
        Self {
            _ws: ws,
            _db: db,
            store,
            root,
        }
    }

    fn root_str(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// 探索笔记目录树（layout 真实派生）。
    fn explores_root(&self) -> PathBuf {
        foundation::layout::resolve(&self.root).explores_root
    }

    /// 占位建档（`None`）+ 以其会话指针铸造执行上下文。
    fn draft_context(&self) -> (ExploreRecord, ExploreNoteContext) {
        let record = self
            .store
            .create_explore_record(&self.root_str(), None)
            .expect("占位建档应成功");
        let context = self.context_for(&record);
        (record, context)
    }

    /// 显式名建档（导入路径）+ 以其会话指针铸造执行上下文。
    fn named_context(&self, name: &str) -> (ExploreRecord, ExploreNoteContext) {
        let record = self
            .store
            .create_explore_record(&self.root_str(), Some(name))
            .unwrap_or_else(|e| panic!("显式名 {name} 建档应成功: {e}"));
        let context = self.context_for(&record);
        (record, context)
    }

    /// 以记录会话指针铸造上下文（被测 API 显式构造入参）。
    fn context_for(&self, record: &ExploreRecord) -> ExploreNoteContext {
        ExploreNoteContext::new(
            Arc::clone(&self.store),
            self.root.clone(),
            record.session_id.clone().expect("建档回写会话指针"),
        )
    }

    /// 断言记录名仍为 `expected_name`（拒绝面零残留的记录半边）。
    fn assert_record_unchanged(&self, record: &ExploreRecord) {
        let current = self
            .store
            .find_explore_record(&self.root_str(), &record.name)
            .expect("find 应成功");
        assert_eq!(
            current.as_ref(),
            Some(record),
            "记录零改动（name / title / updated_at 均未变）"
        );
    }
}

/// 恰 129 字符的合法 kebab 名（长度上限 128 的越界一字节形态）。
fn name_129() -> String {
    format!("a-{}", "b".repeat(127))
}

// ---------------------------------------------------------------------------
// 构造与占位两步定名（正向，AC-3）
// ---------------------------------------------------------------------------

/// `ExploreNoteContext::new` 构造即用：三元组构造后 `execute` 可调用（入参
/// 形态校验臂先于记录圈定——构造完备性的行为面证明）。
#[tokio::test]
async fn new三元组构造即用_入参校验臂可达() {
    let env = NoteEnv::new("ctor");
    let (_record, context) = env.draft_context();

    let error = context
        .execute(&json!({}))
        .await
        .expect_err("空入参应命中入参校验臂");
    assert!(
        error.contains("name"),
        "构造即用（错误面自入参校验臂发出），实际: {error}"
    );
}

/// 占位记录两步定名（正向，AC-3）：`draft-` 记录 + 合法 `name="api-retry"` →
/// ① `explores_root/api-retry.md` 落盘（content 原样）② 记录 in-place 改名
///（`id` / `session_id` 不变）、返回 repo 相对 `/` 分隔路径。
#[tokio::test]
async fn 占位记录两步定名_落盘与inplace改名_返回repo相对路径() {
    let env = NoteEnv::new("finalize");
    let (record, context) = env.draft_context();

    let returned = context
        .execute(&json!({ "name": "api-retry", "content": "# 探索笔记\n重试策略结论" }))
        .await
        .expect("定名应成功");

    // 返回 repo 相对路径（`/` 分隔，供 Edit 续用）
    assert_eq!(
        returned, "openspec/explores/api-retry.md",
        "返回 repo 相对 / 分隔路径，实际: {returned}"
    );
    // ① 落盘：content 原样
    let note = env.explores_root().join("api-retry.md");
    assert!(note.exists(), "笔记文件已落盘");
    assert_eq!(
        std::fs::read_to_string(&note).unwrap(),
        "# 探索笔记\n重试策略结论",
        "content 原样落盘不改写"
    );
    // ② 记录 in-place 改名：id / session_id 不变
    let renamed = env
        .store
        .find_explore_record(&env.root_str(), "api-retry")
        .expect("find 应成功")
        .expect("新名命中");
    assert_eq!(renamed.id, record.id, "in-place 改名保主键");
    assert_eq!(renamed.session_id, record.session_id, "会话指针不变");
    assert_eq!(renamed.root, record.root, "归属键不变");
    // 旧名 miss
    assert!(
        env.store
            .find_explore_record(&env.root_str(), &record.name)
            .expect("find 应成功")
            .is_none(),
        "占位名不再命中"
    );
}

/// 零占位名残留（正向）：定名成功后磁盘不存在任何 `draft-*` 笔记文件、
/// `find_explore_record` 旧名 miss（世上无占位名文件）。
#[tokio::test]
async fn 定名成功后零占位名残留() {
    let env = NoteEnv::new("no-leftover");
    let (record, context) = env.draft_context();

    context
        .execute(&json!({ "name": "clean-name", "content": "内容" }))
        .await
        .expect("定名应成功");

    let explores_root = env.explores_root();
    let leftovers: Vec<String> = match std::fs::read_dir(&explores_root) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("draft-"))
            .collect(),
        Err(_) => Vec::new(),
    };
    assert!(
        leftovers.is_empty(),
        "磁盘不存在任何 draft-* 笔记文件，实际: {leftovers:?}"
    );
    assert!(
        env.store
            .find_explore_record(&env.root_str(), &record.name)
            .expect("find 应成功")
            .is_none(),
        "旧占位名记录 miss"
    );
}

/// 路径分隔符归一（边界）：root 为反斜杠绝对路径（Windows 形态 tempdir）→
/// 返回值 `/` 分隔（`strip_prefix` + 归一，供 Edit 续用）。
#[tokio::test]
async fn 返回路径分隔符归一为正斜杠() {
    let env = NoteEnv::new("slash");
    let (_record, context) = env.draft_context();

    let returned = context
        .execute(&json!({ "name": "slash-name", "content": "x" }))
        .await
        .expect("定名应成功");

    assert!(
        !returned.contains('\\'),
        "返回值不含反斜杠（归一 `/` 分隔），实际: {returned}"
    );
    assert_eq!(
        returned, "openspec/explores/slash-name.md",
        "root 前缀已剥离（repo 相对）"
    );
}

/// 父目录缺失补建（边界）：`openspec/explores/` 目录不存在 → 补建父目录后
/// 落盘成功。
#[tokio::test]
async fn explores目录缺失时补建父目录落盘() {
    let env = NoteEnv::new("mkdir-parents");
    assert!(!env.explores_root().exists(), "前置：explores 目录不存在");
    let (_record, context) = env.draft_context();

    let returned = context
        .execute(&json!({ "name": "fresh-name", "content": "首篇" }))
        .await
        .expect("父目录缺失应补建后落盘");

    assert_eq!(returned, "openspec/explores/fresh-name.md");
    assert_eq!(
        std::fs::read_to_string(env.explores_root().join("fresh-name.md")).unwrap(),
        "首篇"
    );
}

/// content 边界落盘（参数化）：空串 / 超长（>1000 字符）/ 特殊字符（换行、
/// 引号、emoji、中文）→ 原样落盘不改写。
#[tokio::test]
async fn content边界形态原样落盘() {
    let cases: Vec<(&str, String)> = vec![
        ("空串", String::new()),
        ("超长", "长".repeat(1500)),
        (
            "特殊字符",
            "换行\n引号\"双'单`反引${插值}🎉中文收尾".to_owned(),
        ),
    ];
    for (label, content) in cases {
        let env = NoteEnv::new("content-edge");
        let (_record, context) = env.draft_context();

        let returned = context
            .execute(&json!({ "name": "edge-note", "content": content }))
            .await
            .unwrap_or_else(|e| panic!("[{label}] 定名应成功: {e}"));

        assert_eq!(returned, "openspec/explores/edge-note.md", "[{label}]");
        let on_disk = std::fs::read_to_string(env.explores_root().join("edge-note.md"))
            .unwrap_or_else(|e| panic!("[{label}] 读取落盘文件失败: {e}"));
        assert_eq!(on_disk, content, "[{label}] content 原样不改写");
    }
}

// ---------------------------------------------------------------------------
// 前置校验拒绝面（异常，AC-6：零半边残留）
// ---------------------------------------------------------------------------

/// 非法 kebab name（参数化）：大写 / 下划线 / 前导数字 / 空串 → 显式 `Err`
/// 零残留（文件与记录均无变化——前置校验先于落盘）。
#[tokio::test]
async fn 非法kebab名前置拒绝且零残留() {
    for name in ["Api-Retry", "api_retry", "1api", ""] {
        let env = NoteEnv::new("bad-kebab");
        let (record, context) = env.draft_context();

        let error = context
            .execute(&json!({ "name": name, "content": "x" }))
            .await;
        let error = match error {
            Err(error) => error,
            Ok(returned) => panic!("非法名 {name:?} 应 Err，实际 Ok({returned})"),
        };
        assert!(
            error.contains("非法笔记名") && error.contains("kebab"),
            "拒绝记因携 kebab 口径与恢复指引，实际: {error}"
        );
        // 零残留：目标名文件不存在（非法名本身不落盘）、记录名不变
        assert!(
            !env.explores_root().join("api-retry.md").exists(),
            "零残留：目标名文件不存在"
        );
        env.assert_record_unchanged(&record);
    }
}

/// name 超长（异常）：129 字符合法 kebab → `Err`（长度 ≤128 三处同口径），
/// 零残留。
#[tokio::test]
async fn name超长129字符拒绝且零残留() {
    let env = NoteEnv::new("name-too-long");
    let (record, context) = env.draft_context();
    let name = name_129();
    assert_eq!(name.len(), 129, "边界构造恰 129 字节");

    let error = context
        .execute(&json!({ "name": name, "content": "x" }))
        .await
        .expect_err("129 字符应 Err（上限 128）");
    assert!(
        error.contains("长度超过 128"),
        "拒绝记因携长度口径，实际: {error}"
    );
    assert!(
        !env.explores_root().exists()
            || env
                .explores_root()
                .read_dir()
                .map(|entries| entries.count() == 0)
                .unwrap_or(true),
        "零文件落盘"
    );
    env.assert_record_unchanged(&record);
}

/// 目标名已有记录（异常）：预置同名记录 → `Err` 零残留（两记录均不变）。
#[tokio::test]
async fn 目标名已有记录拒绝且两记录零残留() {
    let env = NoteEnv::new("name-taken");
    // 预置目标名记录（另一会话的既有探索）
    let (taken, _taken_context) = env.named_context("api-retry");
    let (record, context) = env.draft_context();

    let error = context
        .execute(&json!({ "name": "api-retry", "content": "x" }))
        .await
        .expect_err("目标名已被占用应 Err");
    assert!(
        error.contains("目标名已被其他探索记录占用"),
        "拒绝记因点名占用，实际: {error}"
    );
    assert!(
        !env.explores_root().join("api-retry.md").exists(),
        "零落盘（既有记录的文件不被新建）"
    );
    env.assert_record_unchanged(&taken);
    env.assert_record_unchanged(&record);
}

/// 目标文件已存在（异常）：预置 `explores_root/<name>.md`（会话外文件 / 残留
/// 态复现）→ `Err` 文案附路径（恢复通道）。
#[tokio::test]
async fn 目标文件已存在拒绝且错误附路径() {
    let env = NoteEnv::new("file-exists");
    let (record, context) = env.draft_context();
    // 残留态复现：目标文件已在盘
    let existing = env.explores_root().join("api-retry.md");
    std::fs::create_dir_all(env.explores_root()).expect("创建 explores 目录失败");
    std::fs::write(&existing, "会话外先在内容").expect("预置目标文件失败");

    let error = context
        .execute(&json!({ "name": "api-retry", "content": "新内容" }))
        .await
        .expect_err("目标文件已存在应 Err");
    assert!(
        error.contains("笔记文件已存在") && error.contains("openspec/explores/api-retry.md"),
        "错误文案附路径（恢复通道），实际: {error}"
    );
    // 文件内容不被覆盖、记录零改动
    assert_eq!(
        std::fs::read_to_string(&existing).unwrap(),
        "会话外先在内容",
        "既有文件不被改写"
    );
    env.assert_record_unchanged(&record);
}

// ---------------------------------------------------------------------------
// 已定名记录臂（同名补建 / 异名拦截）
// ---------------------------------------------------------------------------

/// 已定名同名补建（正向）：已定名记录 + `name == record.name` + 文件缺失
///（孤儿）→ 补建文件、记录不变。
#[tokio::test]
async fn 已定名同名孤儿补建_记录不变() {
    let env = NoteEnv::new("orphan-rebuild");
    let (record, context) = env.named_context("named-topic");
    assert!(
        !env.explores_root().join("named-topic.md").exists(),
        "前置：孤儿形态（文件缺失）"
    );

    let returned = context
        .execute(&json!({ "name": "named-topic", "content": "补建内容" }))
        .await
        .expect("同名补建应成功");

    assert_eq!(returned, "openspec/explores/named-topic.md");
    assert_eq!(
        std::fs::read_to_string(env.explores_root().join("named-topic.md")).unwrap(),
        "补建内容"
    );
    env.assert_record_unchanged(&record);
}

/// 已定名同名且文件在盘（异常半边）：→ `Err` 引导转用 edit（不重写不覆盖）。
#[tokio::test]
async fn 已定名同名且文件在盘引导转用edit() {
    let env = NoteEnv::new("named-on-disk");
    let (record, context) = env.named_context("named-topic");
    std::fs::create_dir_all(env.explores_root()).expect("创建 explores 目录失败");
    std::fs::write(env.explores_root().join("named-topic.md"), "在盘原文").expect("预置笔记失败");

    let error = context
        .execute(&json!({ "name": "named-topic", "content": "新内容" }))
        .await
        .expect_err("同名且在盘应 Err");
    assert!(
        error.contains("已定名且笔记在盘") && error.contains("edit"),
        "错误文案即恢复通道（转用 edit），实际: {error}"
    );
    assert_eq!(
        std::fs::read_to_string(env.explores_root().join("named-topic.md")).unwrap(),
        "在盘原文",
        "在盘内容不被覆盖"
    );
    env.assert_record_unchanged(&record);
}

/// 已定名异名拦截（异常）：已定名记录 + 异名 → `Err` 附现有名与笔记路径
///（错误文案即恢复通道，导入场景常态命中）。
#[tokio::test]
async fn 已定名异名拦截且错误附现有名与路径() {
    let env = NoteEnv::new("name-mismatch");
    let (record, context) = env.named_context("existing-name");

    let error = context
        .execute(&json!({ "name": "other-name", "content": "x" }))
        .await
        .expect_err("异名应被拦截");
    assert!(
        error.contains("existing-name")
            && error.contains("other-name")
            && error.contains("openspec/explores/existing-name.md"),
        "错误附现有名与笔记路径（恢复通道），实际: {error}"
    );
    env.assert_record_unchanged(&record);
}

/// 会话指针 miss（异常）：`session_id` 无对应 explore 记录 → 显式 `Err`
///（按调用时圈定失败）。
#[tokio::test]
async fn 会话指针miss显式拒绝() {
    let env = NoteEnv::new("session-miss");
    let context = ExploreNoteContext::new(
        Arc::clone(&env.store),
        env.root.clone(),
        "ses-no-such-pointer".to_owned(),
    );

    let error = context
        .execute(&json!({ "name": "any-name", "content": "x" }))
        .await
        .expect_err("无归属记录应显式 Err");
    assert!(
        error.contains("未找到归属当前会话的探索记录") && error.contains("ses-no-such-pointer"),
        "失败按调用时圈定（携会话 id 记因），实际: {error}"
    );
}

// ---------------------------------------------------------------------------
// 入参形态（test-design 两行逐行：拒绝面 / 多余字段宽容面）与目标名前缀边界
// ---------------------------------------------------------------------------

/// input 形态拒绝面（异常，参数化——test-design「input 形态拒绝面」行）：
/// 缺 `name` / 缺 `content` / 字段非字符串（`name` / `content` 为数字等）/
/// 非 object Value（string / array / null）→ 显式 `Err`（入参抽取失败文案）。
#[tokio::test]
async fn input形态拒绝面_缺字段非字符串与非object显式err() {
    // 缺 name
    let env = NoteEnv::new("input-missing-name");
    let (_record, context) = env.draft_context();
    let error = context
        .execute(&json!({ "content": "x" }))
        .await
        .expect_err("缺 name 应 Err");
    assert!(error.contains("name"), "实际: {error}");

    // 缺 content
    let env = NoteEnv::new("input-missing-content");
    let (_record, context) = env.draft_context();
    let error = context
        .execute(&json!({ "name": "some-name" }))
        .await
        .expect_err("缺 content 应 Err");
    assert!(error.contains("content"), "实际: {error}");

    // 字段非字符串：name 为数字
    let env = NoteEnv::new("input-non-string-name");
    let (_record, context) = env.draft_context();
    let error = context
        .execute(&json!({ "name": 7, "content": "x" }))
        .await
        .expect_err("name 非字符串应 Err");
    assert!(error.contains("name"), "实际: {error}");

    // 字段非字符串：content 为数字（同一拒绝臂的字段对偶形态）
    let env = NoteEnv::new("input-non-string-content");
    let (_record, context) = env.draft_context();
    let error = context
        .execute(&json!({ "name": "some-name", "content": 9 }))
        .await
        .expect_err("content 非字符串应 Err");
    assert!(error.contains("content"), "实际: {error}");

    // 非 object Value（string / array / null）
    let env = NoteEnv::new("input-non-object");
    let (_record, context) = env.draft_context();
    for input in [
        Value::String("不是对象".to_owned()),
        json!([1, 2]),
        Value::Null,
    ] {
        let error = context
            .execute(&input)
            .await
            .expect_err("非 object 入参应 Err");
        assert!(
            error.contains("name"),
            "非 object 形态 {input} 应自入参校验臂拒绝，实际: {error}"
        );
    }
}

/// input 多余字段宽容面（边界——test-design「input 多余字段宽容面」行）：
/// 入参携多余字段（幻觉 `path` 参数 + 嵌套对象字段）→ additive 宽容：仅取
/// `name` / `content` 两字段、不消费不报错、定名流程照常完成（与
/// `sdk/tools.rs` `string_field` 七通用工具同款 additive 口径，执行体为
/// `input.get` 式 Value 抽取——多余字段不进任何消费面）。
#[tokio::test]
async fn input多余字段宽容面_幻觉path与嵌套字段不消费照常定名() {
    let env = NoteEnv::new("input-extra-field");
    let (record, context) = env.draft_context();

    let returned = context
        .execute(&json!({
            "name": "tolerant-name",
            "content": "内容",
            "path": "幻觉路径参数不消费",
            "future_field": { "any": true }
        }))
        .await
        .expect("多余字段 additive 宽容不应 Err（仅取 name / content 两字段）");

    assert_eq!(returned, "openspec/explores/tolerant-name.md");
    assert!(
        !env.explores_root().join("幻觉路径参数不消费").exists(),
        "path 字段不消费（无路径参数面，幻觉路径零落盘）"
    );
    // 定名流程照常完成：文件落盘于最终名、记录 in-place 改名
    assert!(
        env.explores_root().join("tolerant-name.md").exists(),
        "定名流程照常完成（文件落盘于最终名）"
    );
    let renamed = env
        .store
        .find_explore_record(&env.root_str(), "tolerant-name")
        .expect("find 应成功")
        .expect("定名后新名可寻址");
    assert_eq!(renamed.id, record.id, "in-place 保主键");
    assert_eq!(renamed.session_id, record.session_id, "会话指针不变");
}

/// 目标名为 draft- 前缀（边界）：合法 kebab 且 `draft-` 前缀目标名 → 按普通
/// 定名处理（改名后仍占位前缀，promote 门禁延续拦截——风险 7 同形留痕）。
#[tokio::test]
async fn 目标名draft前缀按普通定名处理() {
    let env = NoteEnv::new("draft-prefix-target");
    let (record, context) = env.draft_context();

    let returned = context
        .execute(&json!({ "name": "draft-999", "content": "仍占位形态" }))
        .await
        .expect("draft- 前缀合法 kebab 应按普通定名成功");

    assert_eq!(returned, "openspec/explores/draft-999.md");
    assert!(
        env.explores_root().join("draft-999.md").exists(),
        "落盘于最终名"
    );
    let renamed = env
        .store
        .find_explore_record(&env.root_str(), "draft-999")
        .expect("find 应成功")
        .expect("改名后可寻址");
    assert_eq!(renamed.id, record.id, "in-place 保主键");
    assert!(
        renamed.name.starts_with("draft-"),
        "改名后仍占位前缀（promote 门禁延续拦截的同形留痕）"
    );
}
