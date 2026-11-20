//! `commands::config` 的单元测试（AC-3 / AC-7）：`workspace_config` 无状态
//! 薄包装 + `workspace_config_inner` 领域组装纯函数。
//!
//! `#[tauri::command]` 保留原函数可直调，测试不启动 Tauri runtime（stats
//! mod_test 先例）。文件系统不 mock：真实 tempdir fixture（RAII 清理）承载
//! 有效 / 缺失 / 坏 JSON / 非目录 / 不可读（悬空 junction，cfg(windows)）root
//! 与配置文件改写序列；断言只收敛自研组装层（Err 通道分层、信封平移、无缓存
//! 行为化核对），不逐项断言 fs / serde 库自身语义。

use std::fs;
use std::path::{Path, PathBuf};

use super::{
    workspace_config, workspace_config_apply, workspace_config_apply_inner, workspace_config_inner,
};

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "desktop-app-config-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn ensure_dir(&self) {
        fs::create_dir_all(&self.0).expect("创建临时根目录失败");
    }

    /// 相对 root 落一份配置文件（自动建父目录 `.dev-team`）。
    fn write_config(&self, content: &str) {
        let path = self.0.join(".dev-team").join("config.json");
        fs::create_dir_all(path.parent().expect("配置路径应有父目录")).expect("建 .dev-team 失败");
        fs::write(path, content).expect("写配置文件失败");
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// workspace_config：薄包装结构证明（不启动 runtime 的可直调性）
// ---------------------------------------------------------------------------

#[test]
fn workspace_config命令与inner纯函数结果逐字段一致_命令层零加工() {
    let ws = TempWs::new("wrapper-parity");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "桌面端插件仓库", "tests": []}"#);
    let root = ws.root().to_string_lossy().into_owned();

    let via_command = workspace_config(root.clone()).expect("有效 root 应 Ok");
    let via_inner = workspace_config_inner(Path::new(&root)).expect("有效 root 应 Ok");

    // 经 serde 序列化对比：命令层除参数转换（String → &Path）与 Err 透传外零加工
    let a = serde_json::to_value(&via_command).expect("命令结果序列化失败");
    let b = serde_json::to_value(&via_inner).expect("inner 结果序列化失败");
    assert_eq!(a, b, "命令层与纯函数对同一 tempdir 根结果逐字段一致");
}

#[test]
fn workspace_config以string原签名直调_无state薄包装结构证明() {
    let ws = TempWs::new("wrapper-direct");
    ws.ensure_dir();
    ws.write_config(r#"{"schema": "spec-driven", "tests": []}"#);
    let root = ws.root().to_string_lossy().into_owned();

    // `#[tauri::command]` 保留 (String,) 原函数签名可直调、无 State 入参
    let report = workspace_config(root).expect("有效 root 应返回 Ok");
    assert_eq!(report.config.schema, "spec-driven");
    assert!(report.diagnostics.is_empty());

    // 无效 root 的 Err 经命令层透传且错误串与 inner 一致（错误映射之末环）
    let ghost = "肯定不存在的路径-直调".to_string();
    let via_command = workspace_config(ghost.clone());
    let via_inner = workspace_config_inner(Path::new(&ghost));
    assert_eq!(
        via_command.unwrap_err(),
        via_inner.unwrap_err(),
        "无效 root 的错误串应与 inner 一致（命令层透传）"
    );
}

// ---------------------------------------------------------------------------
// workspace_config_inner：root 有效性检查（Err 通道唯一来源）
// ---------------------------------------------------------------------------

#[test]
fn 不存在root路径返回err不panic() {
    let missing = std::env::temp_dir().join(format!(
        "desktop-app-config-test-{}-不存在",
        std::process::id()
    ));

    let err = workspace_config_inner(&missing).expect_err("缺失 root 应返回 Err");
    assert!(
        err.contains("root 无效"),
        "错误串应出自前置 metadata 检查，实际 {err:?}"
    );
}

#[test]
fn root指向普通文件返回err() {
    let ws = TempWs::new("file-root");
    ws.ensure_dir();
    fs::write(ws.0.join("plain.txt"), "普通文本文件\n").expect("写 fixture 文件失败");

    let err = workspace_config_inner(&ws.0.join("plain.txt")).expect_err("非目录 root 应返回 Err");
    assert!(err.contains("不是目录"), "错误串应指明非目录，实际 {err:?}");
}

#[test]
fn 空白root返回err() {
    // blank root 与无效 root 同走 Err（与 code_stats 裁定同式：blank 只能来自
    // 调用 bug，不静默空报告）
    assert!(
        workspace_config_inner(Path::new("")).is_err(),
        "空串 root 应返回 Err 而非空报告"
    );
}

#[test]
fn 超长不存在路径root返回err不panic() {
    let long = format!(
        "{}\\{}",
        std::env::temp_dir().display(),
        "very-long-segment-".repeat(80)
    );
    assert!(long.chars().count() > 1000, "前置：路径超长");
    assert!(!Path::new(&long).exists(), "前置：路径不存在");

    assert!(
        workspace_config_inner(Path::new(&long)).is_err(),
        "超长不存在路径应返回 Err 而非 panic"
    );
}

#[test]
#[cfg(windows)]
fn 不可读root返回err_windows下以悬空junction使metadata失败() {
    // stats mod_test 先例 fixture：提权环境下目录 ACL 收紧拦不住 fs::metadata，
    // 改以悬空 junction（指向不存在目标）使 metadata 必然失败——被测属性不变：
    // metadata 失败 → Err 通道，不 panic、不静默空报告。
    let ws = TempWs::new("unreadable-root");
    ws.ensure_dir();
    let link = ws.0.join("dangling");
    let mklink = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(ws.0.join("no-such-target"))
        .output()
        .expect("mklink 应可执行（cmd 内置）");
    assert!(mklink.status.success(), "创建悬空 junction 应成功");
    assert!(
        std::fs::metadata(&link).is_err(),
        "前置：悬空 junction 的 metadata 应失败"
    );

    let err = workspace_config_inner(&link).expect_err("不可达 root 应返回 Err");
    assert!(
        err.contains("root 无效"),
        "错误串应出自前置 metadata 检查，实际 {err:?}"
    );
}

// ---------------------------------------------------------------------------
// workspace_config_inner：文件态分层（Err 通道 vs 报告内标记）
// ---------------------------------------------------------------------------

#[test]
fn 有效root无配置文件返回ok加file_missing诊断与全默认config() {
    let ws = TempWs::new("missing-file");
    ws.ensure_dir();

    // 配置文件缺失不是 Err（多数 workspace 常态）：空态标记在报告内
    let report = workspace_config_inner(ws.root()).expect("配置文件缺失应返回 Ok");
    assert_eq!(report.diagnostics.len(), 1);
    assert!(
        matches!(
            report.diagnostics[0].kind,
            config::DiagnosticKind::FileMissing
        ),
        "缺失应以 fileMissing 诊断在案，实际 {:?}",
        report.diagnostics[0].kind
    );
    assert_eq!(report.diagnostics[0].path, "$");
    // 全默认 config（空态数据面）
    assert_eq!(report.config.schema, "spec-driven");
    assert!(report.config.tests.is_empty());
}

#[test]
fn 坏json返回ok加json_invalid诊断与默认config_两通道分层() {
    let ws = TempWs::new("bad-json");
    ws.ensure_dir();
    ws.write_config("{ 这不是合法 JSON ]");

    // 报告内 fatal（JsonInvalid）走 Ok：仅无效 root 走 Err——两通道分层裁定
    let report = workspace_config_inner(ws.root()).expect("报告内 fatal 应返回 Ok");
    assert!(matches!(
        report.diagnostics[0].kind,
        config::DiagnosticKind::JsonInvalid
    ));
    assert_eq!(
        report.config.schema, "spec-driven",
        "fatal 同样落默认值报告"
    );
}

#[test]
fn 合法配置文件返回ok_diagnostics为空且信封字段平移一致() {
    let ws = TempWs::new("legal-config");
    ws.ensure_dir();
    ws.write_config(r#"{"schema": "spec-driven", "tests": [], "context": "合法上下文", "rules": {"tasks": ["规则一"]}}"#);

    let report = workspace_config_inner(ws.root()).expect("合法配置应返回 Ok");
    assert!(
        report.diagnostics.is_empty(),
        "实际诊断 {:?}",
        report.diagnostics
    );
    assert_eq!(report.config.context.as_deref(), Some("合法上下文"));

    // ConfigReport → WorkspaceConfigReport 字段平移一致：与 config crate
    // load 的领域报告逐字段对照（命令层零加工）
    let loaded = config::load(ws.root());
    let inner_value = serde_json::to_value(&report.config).expect("命令线面 config 序列化失败");
    let load_value = serde_json::to_value(&loaded.config).expect("领域 config 序列化失败");
    assert_eq!(inner_value, load_value, "信封 config 平移一致");
}

#[test]
fn projects配置逐项透传且保序() {
    let ws = TempWs::new("projects-pass");
    ws.ensure_dir();
    ws.write_config(
        r#"{"schema": "spec-driven", "tests": [], "projects": [{"root": "packages/desktop"}, {"root": "packages/desktop/src-tauri"}]}"#,
    );

    let report = workspace_config_inner(ws.root()).expect("合法配置应返回 Ok");

    assert!(
        report.diagnostics.is_empty(),
        "实际诊断 {:?}",
        report.diagnostics
    );
    let roots: Vec<&str> = report
        .config
        .projects
        .iter()
        .map(|project| project.root.as_str())
        .collect();
    assert_eq!(roots, ["packages/desktop", "packages/desktop/src-tauri"]);
}

// ---------------------------------------------------------------------------
// workspace_config_inner：不落库无缓存（行为化核对）
// ---------------------------------------------------------------------------

#[test]
fn 连续两次调用各自完整读取_改写文件后结果随之变化() {
    let ws = TempWs::new("no-cache");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "第一版"}"#);

    let first = workspace_config_inner(ws.root()).expect("首次读取应 Ok");
    assert_eq!(first.config.context.as_deref(), Some("第一版"));

    // 同一进程内改写文件内容后再次调用：结果随文件变化（无缓存行为化核对）
    ws.write_config(r#"{"context": "第二版"}"#);
    let second = workspace_config_inner(ws.root()).expect("二次读取应 Ok");
    assert_eq!(
        second.config.context.as_deref(),
        Some("第二版"),
        "每次调用完整重读重校验（无缓存）"
    );
}

// ---------------------------------------------------------------------------
// 本变更：写面命令 `workspace_config_apply`（门 + 失败零写入 + 写后重读）
// ---------------------------------------------------------------------------

/// 配置文件路径（经 `foundation::layout` 派生：crate 内零路径字面量纪律的同一口径）。
fn config_path_of(ws: &TempWs) -> PathBuf {
    foundation::layout::config_path(ws.root())
}

/// 建配置域目录（生产 workspace 恒有 `.dev-team`；`write_atomic` 不建目录）。
fn ensure_config_dir(ws: &TempWs) {
    fs::create_dir_all(config_path_of(ws).parent().expect("配置路径应有父目录"))
        .expect("建配置域目录失败");
}

/// 目标文件的 (内容, mtime) 双证快照（不存在两形态同为 `None`）。
fn snapshot(path: &Path) -> (Option<String>, Option<std::time::SystemTime>) {
    match fs::metadata(path) {
        Ok(metadata) => (
            Some(fs::read_to_string(path).expect("读配置文件失败")),
            metadata.modified().ok(),
        ),
        Err(_) => (None, None),
    }
}

/// 目录内全部条目名（临时文件残留观测面）。
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|read| {
            read.flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// 满配候选（file face，snake_case）。
fn full_candidate() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://example.com/spec-driven.schema.json",
        "schema": "spec-driven",
        "context": "写回后的上下文",
        "static_analysis": "clippy",
        "projects": [{"root": "packages/desktop", "language": "typescript", "depends_on": ["packages/shared"]}],
        "rules": {"proposal": ["提案规则"], "tasks": ["任务规则"]},
        "tests": [{
            "root": "packages/desktop",
            "framework": "vite-plus",
            "cwd": "packages/desktop",
            "coverage": {"lines": 90, "branches": 88, "functions": 85},
            "mutation": {"cwd": "packages/desktop", "score": 75},
        }],
        "write_protection": {"files": [{"glob": "tests/golden/**", "reason": "金样冻结"}]},
    })
}

/// 正向：命令与 inner 结果逐字段一致（`String` 直调 + `Path` 直调，不启 runtime）。
#[test]
fn apply命令与inner结果逐字段一致_命令层零加工() {
    let ws = TempWs::new("apply-parity");
    ws.ensure_dir();
    ensure_config_dir(&ws);
    let root = ws.root().to_string_lossy().into_owned();

    let via_command = workspace_config_apply(root.clone(), full_candidate()).expect("命令应 Ok");
    let via_inner =
        workspace_config_apply_inner(Path::new(&root), &full_candidate()).expect("inner 应 Ok");

    assert_eq!(
        serde_json::to_value(&via_command).expect("命令结果序列化失败"),
        serde_json::to_value(&via_inner).expect("inner 结果序列化失败"),
        "命令层除参数转换与 Err 透传外零加工"
    );
}

/// 正向：合法候选 → 文件被整体替换为规范化内容（pretty + 尾换行、
/// 三处 snake_case），无临时残留。
#[test]
fn apply合法候选整体替换为规范化内容() {
    let ws = TempWs::new("apply-write");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "旧上下文很长，写回后应整体消失"}"#);
    let before_entries = entries(ws.root());

    workspace_config_apply_inner(ws.root(), &full_candidate()).expect("写回应 Ok");

    let written = fs::read_to_string(config_path_of(&ws)).expect("读回失败");
    assert!(written.ends_with('\n'), "尾换行在场");
    assert!(written.contains("\n  "), "pretty 分隔符在场");
    for key in ["static_analysis", "write_protection", "depends_on"] {
        assert!(written.contains(key), "键形 {key} 在盘面");
    }
    for camel in ["staticAnalysis", "writeProtection", "dependsOn"] {
        assert!(!written.contains(camel), "盘面零 camelCase 残留 {camel}");
    }
    assert!(!written.contains("旧上下文"), "整体替换（无旧字节残留）");
    let after_entries = entries(ws.root());
    assert!(
        after_entries.iter().all(|name| !name.contains("archi-tmp")),
        "无临时文件残留: {after_entries:?}"
    );
    assert_eq!(
        after_entries.len(),
        before_entries.len(),
        "目录条目零新增（临时文件已 rename 掉）"
    );
}

/// 正向：写后重读证明——写前盘面为空，候选设 context / tests → 报告为新值、
/// 无 `fileMissing`（不是写前快照）。
#[test]
fn apply返回写后重读的报告() {
    let ws = TempWs::new("apply-reread");
    ws.ensure_dir();
    ensure_config_dir(&ws);
    let before = workspace_config_inner(ws.root()).expect("空盘面应 Ok");
    assert!(
        before
            .diagnostics
            .iter()
            .any(|d| matches!(d.kind, config::DiagnosticKind::FileMissing)),
        "前置：写前文件缺失"
    );

    let report = workspace_config_apply_inner(ws.root(), &full_candidate()).expect("写回应 Ok");

    assert_eq!(
        report.config.context.as_deref(),
        Some("写回后的上下文"),
        "报告为新值（写后重读，非写前快照）"
    );
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| matches!(d.kind, config::DiagnosticKind::FileMissing)),
        "写后无 fileMissing 诊断"
    );
    assert_eq!(report.config.tests.len(), 1, "suite 逐条重读在案");
}

/// 异常（矩阵 3 形态）：脏候选 → `Err` 且文案逐条含 `path`；文件内容 + mtime 双证不变。
#[test]
fn apply脏候选err且文件零变化() {
    let cases = [
        serde_json::json!({"tests": [{"root": "pkg", "framework": "jest", "coverage": {"lines": 101}}]}),
        serde_json::json!({"tests": [{"root": "pkg", "framework": "jest2"}]}),
        serde_json::json!({"tests": [{"root": "src/*", "framework": "rust"}]}),
    ];
    for (index, candidate) in cases.iter().enumerate() {
        let ws = TempWs::new(&format!("apply-dirty-{index}"));
        ws.ensure_dir();
        ws.write_config(r#"{"context": "原封不动"}"#);
        let path = config_path_of(&ws);
        let before = snapshot(&path);

        let error = workspace_config_apply_inner(ws.root(), candidate).expect_err("脏候选必须 Err");

        assert!(error.contains("tests"), "文案逐条含 path，实际 {error}");
        assert_eq!(
            snapshot(&path),
            before,
            "文件内容与 mtime 双证不变（零写入）"
        );
    }
}

/// 异常（矩阵 4 形态）：root 形态非法 → `Err` 不 panic、任何文件零改动。
#[test]
fn apply_root四形态err不panic() {
    let ws = TempWs::new("apply-root-shapes");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "不改"}"#);
    let path = config_path_of(&ws);
    let before = snapshot(&path);

    let regular_file = ws.root().join("not-a-dir.txt");
    fs::write(&regular_file, "x").expect("落普通文件失败");

    let cases: [(&str, PathBuf); 4] = [
        ("空白", PathBuf::from("")),
        ("不存在", ws.root().join("absent-root")),
        ("普通文件", regular_file),
        ("超长不存在", ws.root().join("x".repeat(600))),
    ];
    for (tag, root) in cases {
        let result = workspace_config_apply_inner(&root, &full_candidate());
        assert!(result.is_err(), "{tag}: root 非法必须 Err");
    }
    assert_eq!(snapshot(&path), before, "root 非法时零文件改动");
}

/// 边界：候选 `{}` → `Ok`（`defaultApplied` 不拦写回），文件内容为 `{}` + 尾换行。
#[test]
fn apply空候选写回空对象形态() {
    let ws = TempWs::new("apply-empty");
    ws.ensure_dir();
    ensure_config_dir(&ws);

    workspace_config_apply_inner(ws.root(), &serde_json::json!({})).expect("空候选应 Ok");

    assert_eq!(
        fs::read_to_string(config_path_of(&ws)).expect("读回失败"),
        "{}\n",
        "空对象 pretty 形态 + 尾换行"
    );
}

/// 边界：幂等——候选 == 盘面现值 → 连续两次 apply 内容逐字相同。
#[test]
fn apply幂等两次内容逐字相同() {
    let ws = TempWs::new("apply-idempotent");
    ws.ensure_dir();
    ensure_config_dir(&ws);

    workspace_config_apply_inner(ws.root(), &full_candidate()).expect("首次写回应 Ok");
    let first = fs::read_to_string(config_path_of(&ws)).expect("读回失败");
    workspace_config_apply_inner(ws.root(), &full_candidate()).expect("二次写回应 Ok");
    let second = fs::read_to_string(config_path_of(&ws)).expect("读回失败");

    assert_eq!(first, second, "重复 apply 不改变内容（规范化幂等）");
}

/// 边界：候选含未知顶层键 → `Ok` 且该键保留在文件顶层，重读报告 `extra` 含该条目。
#[test]
fn apply未知顶层键保留在文件顶层() {
    let ws = TempWs::new("apply-extra");
    ws.ensure_dir();
    ensure_config_dir(&ws);
    let mut candidate = full_candidate();
    candidate["customFlag"] = serde_json::json!({"nested": true});

    let report = workspace_config_apply_inner(ws.root(), &candidate).expect("写回应 Ok");

    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_path_of(&ws)).expect("读回失败"))
            .expect("盘面为合法 JSON");
    assert_eq!(
        written["customFlag"],
        serde_json::json!({"nested": true}),
        "未知顶层键在文件顶层（extra 展开）"
    );
    let extra = report
        .config
        .extra
        .iter()
        .find(|entry| entry.key == "customFlag")
        .expect("重读报告的 extra 含该条目");
    assert_eq!(extra.value, serde_json::json!({"nested": true}));
}

/// 正向（签名面结构锚）：两函数第二参为 file-face `Value`，源码零 typed
/// `WorkspaceConfig` 入参；既有读命令面逐字未动。
#[test]
fn apply签名面为零值面且读命令零改动() {
    let source = include_str!("mod.rs").replace("\r\n", "\n");
    assert!(
        source.contains("candidate: serde_json::Value"),
        "命令签名第二参为 serde_json::Value（file face）"
    );
    assert!(
        source.contains("candidate: &serde_json::Value"),
        "inner 签名第二参为 &serde_json::Value"
    );
    assert!(
        !source.contains("candidate: WorkspaceConfig"),
        "源码零 typed `WorkspaceConfig` 入参（两面不同名，typed 入参即要求反向拼装）"
    );
    assert!(
        source.contains(
            "pub fn workspace_config(root: String) -> Result<WorkspaceConfigReport, String>"
        ),
        "既有读命令签名逐字未动"
    );
    assert!(
        source.contains("pub fn workspace_config_inner(root: &Path)"),
        "既有读 inner 签名逐字未动"
    );
    assert!(
        source.contains("config::plan_apply_config(root, candidate)?"),
        "写面门 = `plan_apply_config`（纯函数零 IO）"
    );
    assert!(
        source.contains("crate::commands::atomic::write_atomic(&plan.file, &plan.content)?"),
        "落盘点 = 命令层共用原子写（搬移后单点）"
    );
}

/// 边界（写面门结构）：失败路径上 `write_atomic` 不在调用链上——脏候选后文件零变
/// 即结构性证明（不靠一条 `if`）。
#[test]
fn apply失败路径无写盘点参与() {
    let ws = TempWs::new("apply-gate");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "不动"}"#);
    let path = config_path_of(&ws);
    let before = snapshot(&path);
    let before_entries = entries(ws.root());

    let _ = workspace_config_apply_inner(
        ws.root(),
        &serde_json::json!({"tests": [{"root": "pkg", "framework": "jest2"}]}),
    )
    .expect_err("违例必须 Err");

    assert_eq!(snapshot(&path), before, "失败零写入（内容 + mtime）");
    assert_eq!(entries(ws.root()), before_entries, "无临时文件被创建");
}

/// 异常：目标目录只读（`cfg(unix)` 0o555）→ `Err` 或可写穿时零临时残留 +
/// 目标可读（沿 `write_atomic` 兜底口径），不 panic。
#[cfg(unix)]
#[test]
fn apply只读目录err或可写穿零残留() {
    use std::os::unix::fs::PermissionsExt;

    let ws = TempWs::new("apply-readonly");
    ws.ensure_dir();
    ws.write_config(r#"{"context": "只读前"}"#);
    let domain = config_path_of(&ws)
        .parent()
        .expect("配置路径应有父目录")
        .to_path_buf();
    fs::set_permissions(&domain, fs::Permissions::from_mode(0o555)).expect("置只读");

    let result = workspace_config_apply_inner(ws.root(), &full_candidate());
    fs::set_permissions(&domain, fs::Permissions::from_mode(0o755)).expect("恢复权限");

    let leftovers: Vec<String> = fs::read_dir(&domain)
        .expect("读域目录失败")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("archi-tmp"))
        .collect();
    assert!(leftovers.is_empty(), "零临时文件残留: {leftovers:?}");
    assert!(fs::read(config_path_of(&ws)).is_ok(), "目标文件仍可读");
    // 以 root 运行时可写穿只读目录：不伪造失败（只保留上面的零残留判据）
    let _ = result;
}
