//! `write::create` 的单元测试（test-design「create.rs -> create_test.rs」节）：
//! 三道前置校验（kebab-case 字符级判定 + ≤128 → goal 非空白 → 已存在拒绝，
//! 全 IO 前置零产生）→ `create_dir_all` 建树 → workflow.json 键序定形写出
//!（2 空格 pretty + 尾换行、无 `eval` 键）→ explore.md 落 goal 原文；
//! CreateOutcome serde 线形状；跨模块组合用例（链路发起方 = 写面 create）：
//! 创建 → 既有清单读面 / flow 前置校验输入面 / 详情读面（含探索条目）/
//! 插件 createChange 形态对照。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——tempdir 空白 workspace 根 RAII
//! 装置（沿 phase_next_test TempWs 先例）；插件 createChange 形态以紧凑单行
//! fixture 字符串锚定（进程内字符串常量，非 mock）；「零产生 / 零改动」以
//! 目录枚举与调用前后字节比对断言。

use std::fs;
use std::path::PathBuf;

use foundation::layout::{resolve, Layout};
use time::OffsetDateTime;

use super::persist::load_doc;
use super::{create, phase_table, CreateOutcome};

/// 临时 workspace 根 RAII（沿 phase_next_test TempWs 先例）：测试结束自动
/// 清理；`new` 不建目录——「空白树」用例依赖根全链不存在的前置。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-create-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn layout(&self) -> Layout {
        resolve(&self.0)
    }

    fn create(&self, name: &str, goal: &str) -> Result<CreateOutcome, String> {
        create(&self.layout(), name, goal)
    }

    fn change_dir(&self, name: &str) -> PathBuf {
        self.layout().changes_root.join(name)
    }

    fn workflow_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(self.change_dir(name).join("workflow.json")).expect("读 workflow.json 失败")
    }

    fn explore_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(self.change_dir(name).join("explore.md")).expect("读 explore.md 失败")
    }

    /// changes_root 下现存目录名（根不存在即空集）——「零产生」观察面。
    fn active_dir_names(&self) -> Vec<String> {
        let root = self.layout().changes_root;
        let Ok(entries) = fs::read_dir(&root) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()
    }

    /// 预置既有 change（同名拒绝用例的对照物）。
    fn seed_existing(&self, name: &str, workflow_json: &str, explore: &str) {
        let dir = self.change_dir(name);
        fs::create_dir_all(&dir).expect("预置 change 目录失败");
        fs::write(dir.join("workflow.json"), workflow_json).expect("预置 workflow.json 失败");
        fs::write(dir.join("explore.md"), explore).expect("预置 explore.md 失败");
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 当前 UTC 日历日期 `YYYY-MM-DD`（与写面同式）。
fn utc_date_today() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

/// created 落在调用前后的 UTC 日期并集内（防御测试恰跨 UTC 午夜）。
fn assert_today(created: &str, before: &str, after: &str) {
    assert!(
        created == before || created == after,
        "created 应为 UTC 当日日期（YYYY-MM-DD），实际: {created}"
    );
}

// ---------------------------------------------------------------------------
// 正向：合法输入建域 / explore.md 落 goal 原文
// ---------------------------------------------------------------------------

/// create 合法输入建域：Ok(CreateOutcome{name, created=当日 UTC 日期})，
/// workflow.json 字节恰为 2 空格 pretty 三键文档 + 尾换行（键序
/// workflow_type → created → file_log、无 eval 键——AC-1/D2 字节级契约）。
#[test]
fn 合法输入建域_ok返回名称与当日且workflow_json字节定形() {
    let ws = TempWs::new("happy");
    let before = utc_date_today();
    let outcome = ws
        .create("fix-bug", "修复登录重试的竞态问题")
        .expect("合法输入应 Ok");
    let after = utc_date_today();

    assert_eq!(outcome.name, "fix-bug");
    assert_today(&outcome.created, &before, &after);

    let bytes = String::from_utf8(ws.workflow_bytes("fix-bug")).expect("UTF-8");
    let expected = format!(
        "{{\n  \"workflow_type\": \"requirement\",\n  \"created\": \"{}\",\n  \"file_log\": []\n}}\n",
        outcome.created
    );
    assert_eq!(
        bytes, expected,
        "2 空格 pretty 三键文档 + 尾换行（键序定形、无 eval 键）"
    );
}

/// explore.md 落 goal 原文：字节恰为 goal 原文（UTF-8 零标题前缀零包装——
/// AC-1/AC-5/D7）。
#[test]
fn explore_md落goal原文零包装() {
    let ws = TempWs::new("explore-raw");
    let goal = "探索目标正文：把清单页新建入口收进对话框";
    ws.create("raw-goal", goal).expect("合法输入应 Ok");

    assert_eq!(ws.explore_bytes("raw-goal"), goal.as_bytes());
}

// ---------------------------------------------------------------------------
// 边界：空白树深层建树 / CreateOutcome serde 线形状 / 名称宽度与正则正样本 /
// goal 保真（首尾空白 / 多行 + emoji + 超长）/ 校验顺序
// ---------------------------------------------------------------------------

/// 空白树深层建树：调用前 changes_root 全链不存在，create_dir_all 建全树后
/// 目录链完整、两文件落位（AC-1「空白树」字面）。
#[test]
fn 空白树深层建树_全链不存在时建全树且两文件落位() {
    let ws = TempWs::new("blank-tree");
    assert!(
        !ws.layout().changes_root.exists(),
        "前置：changes_root 全链不存在"
    );

    ws.create("deep-root", "空白树首个 change")
        .expect("空白树应建树成功");

    assert!(
        ws.change_dir("deep-root").join("workflow.json").is_file(),
        "workflow.json 落位"
    );
    assert!(
        ws.change_dir("deep-root").join("explore.md").is_file(),
        "explore.md 落位"
    );
}

/// CreateOutcome serde 线形状：序列化恰 `{"name":…,"created":…}` 两键
///（camelCase、零磁盘路径字段——AC-4 DTO 纪律的类型面）。
#[test]
fn create_outcome_serde线形状恰两键零磁盘路径字段() {
    let outcome = CreateOutcome {
        name: "fix-bug".to_owned(),
        created: "2026-10-02".to_owned(),
    };

    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let object = value.as_object().expect("序列化为对象");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["created", "name"], "恰两键且零磁盘路径字段");
    assert_eq!(object["name"], "fix-bug");
    assert_eq!(object["created"], "2026-10-02");
}

/// 恰 128 字符放行（≤128 含端点——D3 宽度上界）。
#[test]
fn 恰128字符合法名放行() {
    let ws = TempWs::new("width-128");
    let name = format!("a{}", "b".repeat(127));
    assert_eq!(name.len(), 128, "前置：恰 128 字节");

    let outcome = ws.create(&name, "宽度上界 goal").expect("≤128 含端点应 Ok");
    assert_eq!(outcome.name, name);
}

/// 单字符与数字段放行（正则正样本）。
#[test]
fn 单字符与数字段名放行() {
    let ws = TempWs::new("positive-samples");
    for name in ["a", "fix-bug-2"] {
        let outcome = ws
            .create(name, "正样本 goal")
            .unwrap_or_else(|error| panic!("合法名 {name:?} 应 Ok，实际: {error}"));
        assert_eq!(outcome.name, name);
    }
}

/// goal 首尾空白保真：explore.md 原样保留（写面零 trim——trim 为前端提交前
/// 职责 D6）。
#[test]
fn goal首尾空白保真_写面零trim() {
    let ws = TempWs::new("goal-pad");
    let goal = "  带首尾空白的 goal  ";
    ws.create("padded-goal", goal).expect("合法输入应 Ok");

    assert_eq!(ws.explore_bytes("padded-goal"), goal.as_bytes());
}

/// goal 多行 + emoji + 超 1000 字符：explore.md 字节保真（UTF-8 free-form——
/// AC-5；str 边界映射空 / 空白 / 特殊字符 / 超长全覆盖）。
#[test]
fn goal多行emoji与超长字符保真() {
    let ws = TempWs::new("goal-rich");
    let long_tail = "长".repeat(1000);
    let goal = format!("第一行\n第二行\t制表符 🚀 emoji\n{long_tail}");
    assert!(goal.chars().count() > 1000, "前置：超 1000 字符");

    ws.create("rich-goal", &goal).expect("合法输入应 Ok");

    assert_eq!(ws.explore_bytes("rich-goal"), goal.as_bytes());
}

/// 校验顺序名称优先：非法名 + 空白 goal 同投，Err 归因名称校验（D4 顺序
/// 第一锚：名称 → goal → 已存在）。
#[test]
fn 校验顺序_非法名与空白goal同投时归因名称() {
    let ws = TempWs::new("order-name-first");

    let error = ws.create("Bad_Name", "   ").expect_err("非法名应 Err");
    assert!(
        error.contains("kebab-case"),
        "归因名称校验（D4 顺序第一锚），实际: {error}"
    );
    assert!(
        !error.contains("goal"),
        "goal 校验未参与（名称先决），实际: {error}"
    );
}

/// 校验顺序 goal 先于已存在：合法名 + 空白 goal + 同名已预置，Err 归因
/// goal 空白（D4 顺序第二锚）。
#[test]
fn 校验顺序_合法名空白goal且同名已存在时归因goal() {
    let ws = TempWs::new("order-goal-second");
    ws.seed_existing(
        "seeded",
        r#"{ "workflow_type": "requirement", "file_log": [] }"#,
        "既有探索正文",
    );

    let error = ws.create("seeded", "\n\t ").expect_err("空白 goal 应 Err");
    assert!(
        error.contains("goal"),
        "归因 goal 空白（D4 顺序第二锚），实际: {error}"
    );
    assert!(
        !error.contains("已存在"),
        "已存在校验未参与（goal 先决），实际: {error}"
    );
}

// ---------------------------------------------------------------------------
// 异常：非法 kebab-case 全族 / 超 128 字符 / goal 空白 / 已存在同名
// ---------------------------------------------------------------------------

/// 非法 kebab-case 全族拒绝：大写 / 下划线 / 空格 / 前导数字 / 尾连字符 /
/// 连号连字符 / 前导连字符 / 空串 / 穿越分量——各 Err 且 changes_root 下
/// 零目录零文件（AC-3/D3 spec 正则语义）。
#[test]
fn 非法kebab全族拒绝且零产生() {
    let ws = TempWs::new("bad-names");
    let invalid_names = [
        "Fix-Bug",  // 大写
        "fix_bug",  // 下划线
        "fix bug",  // 空格
        "1fix",     // 前导数字
        "fix-",     // 尾连字符
        "-fix",     // 前导连字符
        "fix--bug", // 连号连字符
        "",         // 空串
        "a/b",      // 穿越分量
        "../x",     // 穿越分量
    ];

    for name in invalid_names {
        let result = ws.create(name, "拒绝面 goal");
        assert!(result.is_err(), "非法名 {name:?} 应 Err");
    }

    assert!(
        ws.active_dir_names().is_empty(),
        "changes_root 下零目录零文件（全 IO 前置拒绝），实际: {:?}",
        ws.active_dir_names()
    );
}

/// 超 128 字符拒绝：129 字符合法字符集名 Err 且零产生（AC-3）。
#[test]
fn 超128字符拒绝且零产生() {
    let ws = TempWs::new("width-129");
    let name = format!("a{}", "b".repeat(128));
    assert_eq!(name.len(), 129, "前置：129 字节");

    let error = ws.create(&name, "越界 goal").expect_err("超 128 应 Err");
    assert!(error.contains("128"), "错误归因长度限制，实际: {error}");
    assert!(ws.active_dir_names().is_empty(), "零产生（校验全 IO 前置）");
}

/// goal 空白拒绝："" / "   " / "\n\t" 各 Err 且目标目录与文件零产生（AC-3）。
#[test]
fn goal空白拒绝且目标目录与文件零产生() {
    let ws = TempWs::new("blank-goal");
    for goal in ["", "   ", "\n\t"] {
        let result = ws.create("fix-bug", goal);
        assert!(result.is_err(), "空白 goal {goal:?} 应 Err");
    }

    assert!(ws.active_dir_names().is_empty(), "目标目录与文件零产生");
}

/// 已存在同名拒绝：预置既有 change 再 create 同名，Err（错误串含目录路径）
/// 且既有 workflow.json / explore.md 字节零变更（AC-3）。
#[test]
fn 已存在同名拒绝且既有产物字节零变更() {
    let ws = TempWs::new("name-taken");
    let existing_workflow = r#"{ "workflow_type": "requirement", "file_log": [] }"#;
    let existing_explore = "既有探索正文";
    ws.seed_existing("taken", existing_workflow, existing_explore);
    let workflow_before = ws.workflow_bytes("taken");
    let explore_before = ws.explore_bytes("taken");

    let error = ws.create("taken", "新建 goal").expect_err("同名应 Err");
    assert!(error.contains("已存在"), "错误归因已存在，实际: {error}");
    let dir_string = ws.change_dir("taken").to_string_lossy().into_owned();
    assert!(
        error.contains(dir_string.as_str()),
        "错误串含目录路径，实际: {error}"
    );

    assert_eq!(
        ws.workflow_bytes("taken"),
        workflow_before,
        "既有 workflow.json 字节零变更"
    );
    assert_eq!(
        ws.explore_bytes("taken"),
        explore_before,
        "既有 explore.md 字节零变更"
    );
}

// ---------------------------------------------------------------------------
// 组合（链路：创建 → 既有读面 / flow 前置校验 / 详情读面 / 插件形态对照）
// ---------------------------------------------------------------------------

/// `创建 → 既有清单读面`：create 后真实组合 queries::list_changes——active
/// 恰一条、inventory=v2、source=active、created=当日、unparsable=false
///（AC-2 v2 识别半边——file_log 键在位判 v2）。
#[test]
fn 创建后既有清单读面识别为v2单条active() {
    let ws = TempWs::new("combo-list");
    let before = utc_date_today();
    ws.create("combo-list", "组合用例 goal")
        .expect("create 应 Ok");
    let after = utc_date_today();

    let list = crate::queries::list_changes(&ws.layout());

    assert_eq!(list.active.len(), 1, "active 恰一条");
    let summary = &list.active[0];
    assert_eq!(summary.name, "combo-list");
    assert_eq!(summary.source, crate::queries::ChangeSource::Active);
    assert_eq!(
        summary.inventory,
        crate::model::Inventory::V2,
        "file_log 键在位判 v2"
    );
    assert!(
        summary.created.as_deref() == Some(before.as_str())
            || summary.created.as_deref() == Some(after.as_str()),
        "created 透传当日日期，实际: {:?}",
        summary.created
    );
    assert!(!summary.unparsable, "新文档可解析");
}

/// `创建 → flow 前置校验输入面`：create 后 persist::load_doc Ok 且
/// workflow_type="requirement"、phase_table("requirement") 为 Some——
/// change_flow_start 三项前置（存在 + 可解析 + 相位表在位）对新文档天然
/// 通过（AC-2 发起半边）。
#[test]
fn 创建后flow前置三项_存在可解析相位表在位() {
    let ws = TempWs::new("combo-flow");
    ws.create("combo-flow", "发起前置输入面 goal")
        .expect("create 应 Ok");

    // 前置 2/3：可解析 + workflow_type=requirement（前置 1「存在」由 Ok 联合
    // 承载——文件缺失时 load_doc 即 Err）
    let doc = load_doc(&ws.layout(), "combo-flow").expect("load_doc 应 Ok");
    assert_eq!(doc.typed.workflow_type, "requirement");
    assert!(
        phase_table(&doc.typed.workflow_type).is_some(),
        "requirement 相位表在位"
    );
}

/// `创建 → 详情读面（含探索条目）`：create 后真实组合 queries::change_detail
/// ——Some 且产物清单含 explore.md→「探索」条目（markdown_doc 既有收录规则
/// 零改动沿用的组合证据——AC-2 detail 可达半边 + AC-5 详情条目的数据面）。
#[test]
fn 创建后详情读面可达且产物清单含探索条目() {
    let ws = TempWs::new("combo-detail");
    ws.create("combo-detail", "详情组合 goal")
        .expect("create 应 Ok");

    let detail = crate::queries::change_detail(&ws.layout(), "combo-detail").expect("详情应可达");
    assert_eq!(detail.name, "combo-detail");
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "markdown-doc"
                && artifact.source == "explore.md"
                && artifact.title == "探索"),
        "产物清单含 explore.md→「探索」条目，实际: {:?}",
        detail.artifacts
    );
}

/// `创建 → 插件 create_change 形态对照`：磁盘 workflow.json 解析 Value 与
/// 插件 createChange 紧凑单行 fixture 解析 Value 全等（字段集 / 值一致；
/// 序列化差异仅空白布局，键序已由字节级行锚定——AC-2 zod 可解析半边 /
/// 风险表「形状漂移」缓解锚）。
#[test]
fn 创建产物与插件create_change紧凑单行形状全等() {
    let ws = TempWs::new("combo-shape");
    ws.create("combo-shape", "形状对照 goal")
        .expect("create 应 Ok");

    let disk_text = fs::read_to_string(ws.change_dir("combo-shape").join("workflow.json"))
        .expect("读 workflow.json 失败");
    let disk: serde_json::Value = serde_json::from_str(&disk_text).expect("磁盘文档可解析");

    // 插件紧凑单行 fixture（created 与磁盘文档同日；序列化差异仅空白布局）
    let created = disk["created"].as_str().expect("created 为字符串");
    let plugin_text =
        format!(r#"{{"workflow_type":"requirement","created":"{created}","file_log":[]}}"#);
    let plugin: serde_json::Value =
        serde_json::from_str(&plugin_text).expect("插件 fixture 可解析");

    assert_eq!(disk, plugin, "字段集 / 值一致");
}
