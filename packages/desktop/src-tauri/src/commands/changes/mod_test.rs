//! `commands::changes::create_change` 的单元测试（test-design
//! 「commands/changes/mod.rs -> mod_test.rs」节）：三件事薄包装直测——blank
//! root 显式 Err（命令层拦截，不进入写面链路）→ foundation `layout::resolve`
//! → 写面 create → `Result<CreateOutcome, String>` 错误映射；返回 DTO 仅
//! `name` 与 `created` 两字段；跨模块组合用例（链路最上层调用方 = 命令层）：
//! `create_change 命令 → layout::resolve → write::create → 磁盘产物`。
//!
//! `#[tauri::command]` 保留原函数可直调；sync 纯函数命令（无 State /
//! AppHandle / Channel——D5），不启动 Tauri runtime，直调即测。
//!
//! Mock策略：无进程边界 mock（fs 真实组合）——tempdir workspace root 真盘，
//! 薄包装等值以同形态独立 tempdir 直调写面比对。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;
use workflow::write;

use super::create_change;

/// 临时 workspace 根 RAII（沿 commands/queries mod_test TempWs 先例）：测试
/// 结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "desktop-changes-create-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn root_string(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    fn layout(&self) -> foundation::layout::Layout {
        resolve(&self.0)
    }

    fn change_dir(&self, name: &str) -> PathBuf {
        self.layout().changes_root.join(name)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 正向：命令 → resolve → 写面 → 磁盘产物（薄包装不加工）
// ---------------------------------------------------------------------------

/// `create_change 命令 → layout::resolve → write::create → 磁盘产物`：Ok
/// 与直调写面 serde 等值（薄包装不加工）；workflow.json + explore.md 真盘
/// 落位（AC-4 薄包装半边）。
#[test]
fn create_change命令到磁盘产物_与直调写面serde等值() {
    let via_command = TempWs::new("cmd");
    let via_core = TempWs::new("core");

    let command_outcome = create_change(
        via_command.root_string(),
        "fix-bug".to_owned(),
        "修复登录重试的竞态问题".to_owned(),
    )
    .expect("合法输入经命令层应 Ok");
    let core_outcome = write::create(&via_core.layout(), "fix-bug", "修复登录重试的竞态问题")
        .expect("直调写面应 Ok");

    assert_eq!(
        serde_json::to_value(&command_outcome).expect("命令结果序列化失败"),
        serde_json::to_value(&core_outcome).expect("写面结果序列化失败"),
        "薄包装不加工"
    );

    // 真盘落位
    assert!(
        via_command
            .change_dir("fix-bug")
            .join("workflow.json")
            .is_file(),
        "workflow.json 真盘落位"
    );
    assert!(
        via_command
            .change_dir("fix-bug")
            .join("explore.md")
            .is_file(),
        "explore.md 真盘落位"
    );
}

// ---------------------------------------------------------------------------
// 边界：返回 DTO 仅两字段
// ---------------------------------------------------------------------------

/// 返回 DTO 仅两字段：serde_json 序列化恰 `{"name":…,"created":…}` 两键、
/// 零磁盘路径字段（AC-4 DTO 纪律）。
#[test]
fn 返回dto恰name_created两键零磁盘路径字段() {
    let ws = TempWs::new("dto-shape");

    let outcome = create_change(ws.root_string(), "fix-bug".to_owned(), "goal".to_owned())
        .expect("合法输入应 Ok");

    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("序列化为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["created", "name"], "恰两键且零磁盘路径字段");
}

// ---------------------------------------------------------------------------
// 异常：blank root 命令层拦截 / 写面拒绝透传
// ---------------------------------------------------------------------------

/// blank root 显式 Err 不进写面：root "" / "   " 各 Err（命令层拦截文案，
/// 写面错误面无此分支）且 changes_root 下零新目录（AC-4；沿 explores 组写
/// 命令 blank root 纪律口径）。
#[test]
fn blank_root显式err不进写面链路() {
    let ws = TempWs::new("blank-root");
    for root in ["", "   "] {
        let error = create_change(root.to_owned(), "fix-bug".to_owned(), "goal".to_owned())
            .expect_err("blank root 应 Err");
        assert_eq!(
            error, "非法 root: 不得为空白",
            "命令层拦截（不进入写面链路），实际: {error}"
        );
    }

    assert!(
        !ws.layout().changes_root.exists(),
        "changes_root 下零新目录"
    );
}

/// 写面拒绝透传：非法名 / 空白 goal 经命令层 Err（错误映射不吞不加工，写面
/// 原文透传）且零产生（AC-3 命令入口同口径）。
#[test]
fn 写面拒绝透传_非法名与空白goal经命令层err且零产生() {
    let ws = TempWs::new("passthrough-err");

    let name_error = create_change(ws.root_string(), "Fix_Bug".to_owned(), "goal".to_owned())
        .expect_err("非法名应 Err");
    assert!(
        name_error.contains("kebab-case"),
        "错误映射不吞不加工（写面原文透传），实际: {name_error}"
    );

    let goal_error = create_change(ws.root_string(), "fix-bug".to_owned(), "   ".to_owned())
        .expect_err("空白 goal 应 Err");
    assert!(
        goal_error.contains("goal"),
        "空白 goal 错误透传，实际: {goal_error}"
    );

    assert!(
        !ws.layout().changes_root.exists(),
        "零产生（拒绝面无目录无文件）"
    );
}
