//! `preclose`（收口前架构判据自检）的单元测试：`baseline` 的会话判定与起点哈希、
//! `touched` 的写面三支判据，以及 `run` 的 T1 / T2 触发、notice 文案面与 fail-open
//! 三面。
//!
//! Mock 只发生在进程边界：`Sender` 用真实 `tokio::sync::mpsc::channel`（「发了什么、
//! 发了几条」即被测面）；`spawn_blocking` / `assets_hash` / `drift::check` /
//! `protect::written_paths` 全部真实——替身会掩盖真实报告形态与真实写面提取面。

use std::path::{Path, PathBuf};

use agent::{AgentEventKind, AgentPermissionMode};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::sdk::archi_drift_view::drift_summary;
use crate::sdk::preclose;
use crate::sdk::protect::ProtectRules;
use crate::sdk::r#loop::{LoopTurn, StreamLiveness, StreamRetry};

// ---------------------------------------------------------------------------
// 装置：真实 tempdir 盘面 + 真实 LoopTurn
// ---------------------------------------------------------------------------

/// 落一个 repo 相对文件（自动建父目录）。
fn write_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建父目录失败");
    }
    std::fs::write(path, content).expect("写文件失败");
}

/// 落模型声明（`<archi_graph_model_dir>/01-test.toml`）。
fn write_model(root: &Path, content: &str) {
    let dir = foundation::layout::archi_graph_model_dir(root);
    std::fs::create_dir_all(&dir).expect("创建模型声明目录失败");
    std::fs::write(dir.join("01-test.toml"), content).expect("写模型文件失败");
}

/// 健康盘面：一元素认领 `src/auth/`，另有一处无主源文件。
fn healthy_root(tag: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("sdk-preclose-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败");
    write_model(
        dir.path(),
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
    write_file(dir.path(), "src/loose/orphan.ts", "export const x = 1;\n");
    dir
}

/// 轮参数：`preamble` 位即「archi 会话」判据（`None` = 非 archi 会话）。
fn loop_turn(cwd: &Path, preamble: Option<&str>) -> LoopTurn {
    LoopTurn {
        question: "把模型补一下".to_owned(),
        cwd: cwd.to_path_buf(),
        permission_mode: AgentPermissionMode::Default,
        model_name: "rig-test-model".to_owned(),
        session_id: "sdk-preclose-0".to_owned(),
        defense: crate::sdk::context::ContextDefense::resolve(None),
        liveness: StreamLiveness::default(),
        retry: StreamRetry::default(),
        injections: agent::SessionInjections {
            preamble: preamble.map(str::to_owned),
            ..agent::SessionInjections::default()
        },
        explore_note: None,
        archi: None,
        db: None,
        ask: None,
        protect: ProtectRules::load(None),
        web_search: false,
        effort_params: None,
    }
}

/// 跑一次自检并把事件流取空（收端在用例侧 drain——「发了什么」即被测面）。
async fn collect_events(
    turn: &LoopTurn,
    touched: bool,
    baseline: Option<&str>,
) -> Vec<AgentEventKind> {
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(32);
    preclose::run(&sender, turn, touched, baseline).await;
    drop(sender);
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    events
}

/// 事件流里的 notice 清单（`(subtype, payload)`）。
fn notices(events: &[AgentEventKind]) -> Vec<(&str, &Value)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEventKind::SystemNotice { subtype, payload } => Some((subtype.as_str(), payload)),
            _ => None,
        })
        .collect()
}

/// 模型声明目录内某 `.toml` 的绝对路径。
fn model_file(root: &Path) -> PathBuf {
    foundation::layout::archi_graph_model_dir(root).join("01-test.toml")
}

/// 契约资产目录内某 `.md` 的绝对路径。
fn contract_file(root: &Path) -> PathBuf {
    foundation::layout::archi_models_dir(root).join("App/Auth.md")
}

// ---------------------------------------------------------------------------
// `baseline`
// ---------------------------------------------------------------------------

/// 正向：`preamble` 在场 → `Some(hash)` 且与直调 `archi::drift::assets_hash(cwd)` 逐字
/// 相等（起点值单点，零第二份哈希）。
#[tokio::test]
async fn baseline取起点资产哈希() {
    let root = healthy_root("baseline-hit");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let value = preclose::baseline(&turn)
        .await
        .expect("archi 会话应有起点值");
    assert_eq!(
        value,
        archi::drift::assets_hash(root.path()),
        "起点值 = assets_hash 直调值"
    );
    assert!(value.starts_with("sha256:"), "哈希形态 sha256 前缀");
    assert_eq!(value.len(), "sha256:".len() + 64, "64 位十六进制");
}

/// 边界（表驱动）：`preamble = None`（非 archi 会话）→ `None`；`cwd` 指向不存在目录
/// 亦 `None`（零 IO，零 panic）。
#[tokio::test]
async fn baseline非archi会话恒none() {
    let root = healthy_root("baseline-miss");
    let missing = root.path().join("does-not-exist");
    for cwd in [root.path(), missing.as_path()] {
        let turn = loop_turn(cwd, None);
        assert!(
            preclose::baseline(&turn).await.is_none(),
            "preamble 缺席即 None（零 IO）：{cwd:?}"
        );
    }
}

/// 边界：`preamble = Some("")`（空串）→ 仍 `Some`——判据是 `is_some()`，不按内容判
/// 会话（空 preamble 也是 archi 臂的产物形态）。
#[tokio::test]
async fn baseline空preamble仍判archi会话() {
    let root = healthy_root("baseline-empty");
    let turn = loop_turn(root.path(), Some(""));
    assert!(
        preclose::baseline(&turn).await.is_some(),
        "空串 preamble 仍判为 archi 会话"
    );
}

/// 异常：三处资产根皆不存在 → 返回全空盘稳定值（不抛错、两次相等）。
#[tokio::test]
async fn baseline空盘稳定值不报错() {
    let root = tempfile::Builder::new()
        .prefix("sdk-preclose-baseline-empty-")
        .tempdir()
        .expect("创建合成目录失败");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let first = preclose::baseline(&turn).await.expect("仍应给出起点值");
    let second = preclose::baseline(&turn).await.expect("仍应给出起点值");
    assert_eq!(first, second, "同一盘面稳定");
}

// ---------------------------------------------------------------------------
// `touched`
// ---------------------------------------------------------------------------

/// 正向（表驱动）：`write` / `edit` 的 `path` 落在两个架构子目录内 → `true`。
#[test]
fn touched_write与edit命中两目录() {
    let root = healthy_root("touched-write");
    let model = model_file(root.path());
    let contract = contract_file(root.path());
    for path in [model, contract] {
        let input = serde_json::json!({ "path": path.to_string_lossy() });
        assert!(
            preclose::touched("write", &input, root.path()),
            "write 命中 {path:?}"
        );
        assert!(
            preclose::touched("edit", &input, root.path()),
            "edit 命中 {path:?}"
        );
    }
}

/// 边界（表驱动）：`path` 恰为架构域根 / `graph` / `models` 目录本身（不带尾随文件
/// 名）→ `false`（判据是目录前缀）。
#[test]
fn touched目录本身不算命中() {
    let root = healthy_root("touched-dir");
    let archi = foundation::layout::archi_dir(root.path());
    let graph = foundation::layout::archi_graph_dir(root.path());
    let models = foundation::layout::archi_models_dir(root.path());
    for path in [archi, graph, models] {
        let input = serde_json::json!({ "path": path.to_string_lossy() });
        assert!(
            !preclose::touched("write", &input, root.path()),
            "目录本身不命中：{path:?}"
        );
    }
}

/// 边界（表驱动）：`path` 缺失 / 非字符串（`null` / 数字）→ `false`，零 panic。
#[test]
fn touched非法path一律false() {
    let root = healthy_root("touched-invalid");
    for input in [
        serde_json::json!({}),
        serde_json::json!({ "path": null }),
        serde_json::json!({ "path": 42 }),
    ] {
        assert!(
            !preclose::touched("write", &input, root.path()),
            "非法入参 fail-open：{input}"
        );
        assert!(
            !preclose::touched("edit", &input, root.path()),
            "非法入参 fail-open（edit）：{input}"
        );
    }
}

/// 边界（表驱动）：`path` 落在项目其他目录（`src/` / `openspec/`）→ `false`。
#[test]
fn touched其他目录不命中() {
    let root = healthy_root("touched-other");
    for relative in ["src/auth/lib.rs", "openspec/specs/x/spec.md"] {
        let path = root.path().join(relative);
        let input = serde_json::json!({ "path": path.to_string_lossy() });
        assert!(
            !preclose::touched("write", &input, root.path()),
            "非架构域路径不命中：{relative}"
        );
    }
}

/// 正向（表驱动）：`bash` 写面命令（重定向 / `tee` / 追加）指向模型路径 → `true`
/// （与保护闸同一写面提取面）。
#[test]
fn touched_bash写面命中() {
    let root = healthy_root("touched-bash-write");
    let model = model_file(root.path()).to_string_lossy().into_owned();
    for command in [
        format!("cat > {model}"),
        format!("tee {model}"),
        format!("echo 'kind = \"module\"' >> {model}"),
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            preclose::touched("bash", &input, root.path()),
            "写面命令应命中：{command}"
        );
    }
}

/// 边界（表驱动）：只读命令（`ls` / `cat x` / `grep -n x y`）→ `false`。
#[test]
fn touched_bash只读命令不命中() {
    let root = healthy_root("touched-bash-read");
    for command in [
        "ls src",
        "cat src/auth/lib.rs",
        "grep -n login src/auth/lib.rs",
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            !preclose::touched("bash", &input, root.path()),
            "只读命令不命中：{command}"
        );
    }
}

/// 边界（表驱动）：空命令串 / 空白串 → `false`（`written_paths` 空数组）。
#[test]
fn touched_bash空命令不命中() {
    let root = healthy_root("touched-bash-empty");
    for command in ["", "   \t\n"] {
        let input = serde_json::json!({ "command": command });
        assert!(
            !preclose::touched("bash", &input, root.path()),
            "空命令不命中：{command:?}"
        );
    }
}

/// 边界（表驱动）：命名工具非 `write` / `edit` / `bash` → 恒 `false`，即便入参带
/// `path`（射程由名字白名单表达）。
#[test]
fn touched其他工具名恒false() {
    let root = healthy_root("touched-names");
    let input = serde_json::json!({ "path": model_file(root.path()).to_string_lossy() });
    for name in ["read", "archi_module", "finish_step"] {
        assert!(
            !preclose::touched(name, &input, root.path()),
            "工具 {name} 不在写面射程内"
        );
    }
}

/// 异常（表驱动）：大小写 / 盘符与 `cwd` 前缀失配的绝对路径 → `false`（假阴由 T2 兜底，
/// 不为它扩提取面、不加路径折叠）。
#[test]
fn touched前缀失配假阴不命中() {
    let root = healthy_root("touched-case");
    let relative = foundation::layout::archi_graph_model_rel_dir();
    let upper = root
        .path()
        .join(relative.to_uppercase())
        .join("x.toml")
        .to_string_lossy()
        .into_owned();
    let other_drive = format!("Z:/project/{relative}/x.toml");
    for path in [upper, other_drive] {
        let input = serde_json::json!({ "path": path });
        assert!(
            !preclose::touched("write", &input, root.path()),
            "前缀失配即假阴：{path}"
        );
    }
}

/// 异常：`bash` 写目标的不可解析形态（含 `~` 变量 / 空目标）→ 该 op 跳过、其余照判，
/// 零 panic。
#[test]
fn touched不可解析写目标跳过() {
    let root = healthy_root("touched-unparsable");
    for command in [
        "cat > ~/archi-graph-model.toml".to_owned(),
        "cat > $TARGET".to_owned(),
        "tee ''".to_owned(),
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            !preclose::touched("bash", &input, root.path()),
            "不可解析目标不 panic 且不命中：{command}"
        );
    }
}

// ---------------------------------------------------------------------------
// `run`：触发面
// ---------------------------------------------------------------------------

/// 正向：T1 命中（`touched = true`）且盘面未变 → 发一条 `archi_preclose`，payload 含
/// `status` / `passed` / `counts` / `durationMs` / `changedAssets = false`。
#[tokio::test]
async fn run_t1命中发notice() {
    let root = healthy_root("run-t1");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;

    let notices = notices(&events);
    assert_eq!(notices.len(), 1, "恰一条 notice");
    let (subtype, payload) = notices[0];
    assert_eq!(subtype, "archi_preclose");
    assert!(payload["status"].is_string(), "payload 含 status");
    assert!(payload["passed"].is_boolean(), "payload 含 passed");
    assert!(payload["counts"].is_object(), "payload 含 counts");
    assert!(payload["durationMs"].is_u64(), "payload 含 durationMs");
    assert_eq!(
        payload["changedAssets"],
        Value::Bool(false),
        "盘面未变即 changedAssets = false"
    );
}

/// 正向：T2 命中（`touched = false`、盘面在两测点之间被改）→ 同样发 notice 且
/// `changedAssets = true`。
#[tokio::test]
async fn run_t2命中发notice() {
    let root = healthy_root("run-t2");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    write_model(
        root.path(),
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/auth/\"]\n",
            "summary = \"认证模块\"\n",
        ),
    );
    let events = collect_events(&turn, false, Some(&baseline)).await;

    let notices = notices(&events);
    assert_eq!(notices.len(), 1, "恰一条 notice");
    assert_eq!(notices[0].0, "archi_preclose");
    assert_eq!(
        notices[0].1["changedAssets"],
        Value::Bool(true),
        "盘面变化即 changedAssets = true"
    );
}

/// 边界：T1、T2 皆假 → 接收端零事件（纯问答不产生转录噪声，只留引擎日志）。
#[tokio::test]
async fn run双假零notice() {
    let root = healthy_root("run-skip");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, false, Some(&baseline)).await;
    assert!(events.is_empty(), "零事件，实际: {events:?}");
}

/// 边界：`baseline = None`（非 archi 会话）→ 早退：零 notice、零事件。
#[tokio::test]
async fn run非archi会话早退() {
    let root = healthy_root("run-not-archi");
    let turn = loop_turn(root.path(), None);
    let events = collect_events(&turn, true, None).await;
    assert!(events.is_empty(), "非 archi 会话零事件，实际: {events:?}");
}

/// 异常：`drift::check` 返回 `CheckError`（架构域配置非法）→ 发
/// `archi_preclose_skipped` 且 `reason` 含成因，函数正常返回（不抛错、不阻断收口）。
#[tokio::test]
async fn run配置非法fail_open() {
    let root = healthy_root("run-config-error");
    let config = foundation::layout::archi_config_path(root.path());
    std::fs::create_dir_all(config.parent().expect("父目录")).expect("建架构域目录");
    std::fs::write(&config, "{ \"unknownKey\": [] }").expect("写配置");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;

    let notices = notices(&events);
    assert_eq!(notices.len(), 1, "fail-open 仍恰一条 notice");
    let (subtype, payload) = notices[0];
    assert_eq!(subtype, "archi_preclose_skipped");
    assert!(
        payload["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("unknownKey")),
        "reason 携成因：{payload}"
    );
}

/// 边界：零请求增量 / 零历史追加——函数发出的事件种类集合 ⊆ `{SystemNotice}`（不回灌
/// 模型、不催轮次）。
#[tokio::test]
async fn run事件种类只有notice() {
    let root = healthy_root("run-only-notice");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;
    assert!(!events.is_empty(), "触发面应有事件");
    for event in &events {
        assert!(
            matches!(event, AgentEventKind::SystemNotice { .. }),
            "只发 SystemNotice，实际: {event:?}"
        );
    }
}

/// 边界：空架构域（无模型）→ notice `status = "skipped"` 照常发出，`passed` 与
/// `report.passed()` 一致。
#[tokio::test]
async fn run空架构域仍发notice() {
    let root = tempfile::Builder::new()
        .prefix("sdk-preclose-run-empty-")
        .tempdir()
        .expect("创建合成目录失败");
    write_file(root.path(), "src/loose/orphan.ts", "export const x = 1;\n");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;
    let notices = notices(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].1["status"], "skipped", "无模型即 skipped");
    assert_eq!(
        notices[0].1["passed"],
        Value::Bool(true),
        "skipped 报告零 errors 即 passed"
    );
}

/// 边界：常态大报告（数百条 unmatched）→ payload 只在计数字段出现该数量，JSON 文本零
/// unmatched 路径串。
#[tokio::test]
async fn run常态大报告只出计数() {
    let root = tempfile::Builder::new()
        .prefix("sdk-preclose-run-scale-")
        .tempdir()
        .expect("创建合成目录失败");
    write_model(
        root.path(),
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/auth/\"]\n",
        ),
    );
    write_file(root.path(), "src/auth/lib.rs", "pub fn login() {}\n");
    let mut orphans: Vec<String> = Vec::new();
    for index in 0..302 {
        let relative = format!("src/loose/orphan{index}.ts");
        write_file(root.path(), &relative, "export const x = 1;\n");
        orphans.push(relative);
    }
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;
    let notices = notices(&events);
    assert_eq!(notices.len(), 1);
    let payload = notices[0].1;
    assert_eq!(
        payload["counts"]["scan"]["unmatched"],
        Value::from(orphans.len() + 1),
        "计数位等于清单长度（含 off 档元素覆盖文件）"
    );
    let text = payload.to_string();
    for orphan in &orphans {
        assert!(
            !text.contains(orphan.as_str()),
            "notice payload MUST NOT 出现 unmatched 路径串 {orphan}"
        );
    }
}

/// 组合：notice payload = `drift_summary(report, duration_ms)` 逐字段 + 追加
/// `changedAssets`（投影单点，零第二份装配）。
#[tokio::test]
async fn run_payload与drift_summary同源() {
    let root = healthy_root("run-same-source");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;
    let payload = notices(&events)[0].1;

    let report = archi::drift::check(archi::drift::CheckInput {
        root: root.path(),
        mode: archi::drift::ArchiDriftMode::Full,
        changed: &[],
        change: None,
        cancel: foundation::cancel::CancelFlag::never(),
    })
    .expect("同一盘面应判定成功");
    let duration_ms = payload["durationMs"].as_u64().expect("durationMs 为整数");
    let summary = drift_summary(&report, duration_ms);
    let mut expected = summary.as_object().expect("summary 为对象").clone();
    expected.insert("changedAssets".to_owned(), Value::Bool(false));
    assert_eq!(
        payload.as_object().expect("payload 为对象"),
        &expected,
        "payload 与 drift_summary 逐字段相等（+ changedAssets）"
    );
}

/// 异常（AC-4 / D8）：坏模型（`dependsOnAncestor` 结构错）→ `drift::check` 仍
/// 产报告（结构错属 `model.structuralErrors`，不是 `CheckError`）→ notice 照常为
/// `archi_preclose`，`status = "errors"` 且结构错明细在场（fail-open 不 panic）。
#[tokio::test]
async fn run坏模型态出结构错报告而非跳过() {
    let root = tempfile::Builder::new()
        .prefix("sdk-preclose-bad-model-")
        .tempdir()
        .expect("创建合成目录失败");
    write_model(
        root.path(),
        concat!(
            "[element.\"Root\"]\n",
            "kind = \"package\"\n",
            "path = [\"./src/\"]\n",
            "\n",
            "[element.\"Root.Child\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/auth/\"]\n",
            "\n",
            "[[relation]]\n",
            "from = \"Root.Child\"\n",
            "to = \"Root\"\n",
        ),
    );
    write_file(
        root.path(),
        "src/auth/token.ts",
        "export const token = 1;\n",
    );
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;

    let notices = notices(&events);
    assert_eq!(notices.len(), 1, "坏模型也恰一条 notice");
    let (subtype, payload) = notices[0];
    assert_eq!(
        subtype, "archi_preclose",
        "结构错走报告面（非 skipped，fail-open 不 panic）：{payload}"
    );
    assert_eq!(payload["status"], "errors", "坏模型即 errors");
    assert!(
        payload["model"]["structuralErrors"]
            .as_array()
            .is_some_and(|list| !list.is_empty()),
        "结构错明细在场：{payload}"
    );
    assert!(
        payload["model"]["structuralErrorCount"]
            .as_u64()
            .is_some_and(|count| count >= 1),
        "结构错计数在场：{payload}"
    );
    assert_eq!(
        payload["errors"].as_array().map(Vec::len),
        Some(0),
        "结构错不进 errors[]：{payload}"
    );
}

/// 异常（AC-4 / D8 第二行）：`drift::check` 返回 `CheckError::Scan`（看护配置
/// `exclude` 含语法非法 glob）→ 发 `archi_preclose_skipped`，`reason` 含非法条目原文，
/// 函数正常返回（不阻断收口）。
#[tokio::test]
async fn run扫描面检查错误跳过并携成因() {
    let root = healthy_root("run-scan-error");
    let config = foundation::layout::archi_config_path(root.path());
    std::fs::create_dir_all(config.parent().expect("父目录")).expect("建架构域目录");
    std::fs::write(&config, "{ \"exclude\": [\"src/[legacy\"] }").expect("写配置");
    let turn = loop_turn(root.path(), Some("架构知识"));
    let baseline = preclose::baseline(&turn).await.expect("起点值");
    let events = collect_events(&turn, true, Some(&baseline)).await;

    let notices = notices(&events);
    assert_eq!(notices.len(), 1, "fail-open 仍恰一条 notice");
    let (subtype, payload) = notices[0];
    assert_eq!(subtype, "archi_preclose_skipped");
    assert!(
        payload["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("src/[legacy")),
        "reason 携扫描面成因原文：{payload}"
    );
}

/// 异常（AC-4 / D8 第三面）：`spawn_blocking` 任务失败（`JoinError`）分支发
/// `archi_preclose_skipped` 且 `reason` 含「自检任务失败」。
///
/// 该分支是阻塞池 panic / 取消的 fail-open 兜底：真实 `drift::check` 全程无 panic
/// 点，无法从公共入参确定性地注入一次 `JoinError`，故以源码锚守住分支文案与子类
/// （与 `run` 内三臂一一对应）。
#[test]
fn 自检任务失败分支带成因文案() {
    let source = include_str!("preclose.rs");
    assert!(
        source.contains("Err(join_error) =>"),
        "JoinError 分支在场（spawn_blocking 失败面）"
    );
    assert!(
        source.contains("自检任务失败: {join_error}"),
        "reason 文案含「自检任务失败」并携 JoinError"
    );
    assert!(
        source.contains("ARCHI_PRECLOSE_SKIPPED"),
        "失败面走 archi_preclose_skipped 子类"
    );
}
