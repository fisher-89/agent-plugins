//! `protect` 模块入口的单元测试（AC-1 / AC-2 / AC-3 / AC-5 / AC-6 / AC-7 /
//! AC-9 / AC-10）：`ProtectRules::load`（唯一 IO 点 / 规则合并）与
//! `ProtectRules::evaluate`（工具名分派 + bash 三路判定 + 首个命中即止）。
//!
//! 断言口径：
//! - `evaluate` 用例以 `ProtectRules { rules: vec![…] }` 直构（被测 API 的显式
//!   入参）+ `rules::load(None)` 产出的内置快照驱动，纯内存零 IO；
//! - `load` 用例以 `tempfile::TempDir` 真实目录 + `foundation::layout` 派生的
//!   真实配置文件驱动，`config::load` 真实读取（不 mock）；
//! - 误拦 / 漏拦双向均断言（AC-5），拒绝串首条即止不拼接（AC-8）。

use std::path::{Path, PathBuf};

use foundation::layout;

use super::rules::{self, ReasonKind, Rule};
use super::ProtectRules;

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-protect-eval-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

/// 按 `foundation::layout::config_path(root)` 落盘配置文件。
fn write_config(root: &Path, config: serde_json::Value) {
    let path = layout::config_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建配置目录失败");
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&config).expect("配置序列化"),
    )
    .expect("写配置文件失败");
}

/// 原样落盘配置文本（坏 JSON / 非对象顶层等畸形形态）。
fn write_config_text(root: &Path, text: &str) {
    let path = layout::config_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建配置目录失败");
    }
    std::fs::write(&path, text).expect("写配置文件失败");
}

fn normalized(path: &Path) -> String {
    rules::normalize(&path.to_string_lossy())
}

/// 配置文件相对路径（域目录 + 文件名全部由 layout 派生）。
fn config_relative() -> String {
    rules::normalize(&layout::config_path(Path::new("")).to_string_lossy())
}

fn builtin_rule() -> Rule {
    rules::builtin_rules()
        .into_iter()
        .next()
        .expect("内置规则恒在")
}

fn user_rule(glob: &str, reason: Option<&str>) -> Rule {
    Rule {
        glob: glob.to_owned(),
        reason: ReasonKind::User(reason.map(str::to_owned)),
    }
}

/// 直构规则集（字段私有但子模块可见；生产只由 `load` 构造）。
fn with_rules(mut rules: Vec<Rule>) -> ProtectRules {
    rules.insert(0, builtin_rule());
    ProtectRules { rules }
}

/// 目录树递归快照（路径 + 字节数），用于「判定零写入」断言。
fn snapshot(root: &Path) -> Vec<(PathBuf, u64)> {
    let mut entries = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                stack.push(path.clone());
            }
            entries.push((path, metadata.len()));
        }
    }
    entries.sort();
    entries
}

fn protected_path(root: &Path) -> String {
    format!("{}/{}", normalized(root), config_relative())
}

// ---------------------------------------------------------------------------
// load（唯一 IO 点 / 规则合并）
// ---------------------------------------------------------------------------

#[test]
fn 未注入root时仅内置且命中照常() {
    let dir = tempdir("load-none");
    let loaded = ProtectRules::load(None);
    assert_eq!(
        loaded.rules.len(),
        rules::builtin_rules().len(),
        "None → 只装载内置规则（不产出「保护关闭」空集）"
    );

    let reason = loaded
        .evaluate(
            "write",
            &serde_json::json!({ "path": protected_path(dir.path()), "content": "x" }),
            dir.path(),
        )
        .expect("内置规则对任意前缀的配置文件名命中");
    assert!(
        reason.contains("配置权威"),
        "拒绝串为内置模板，实际: {reason}"
    );
}

#[test]
fn 注入root时合并用户规则且模板已替换() {
    let dir = tempdir("load-merge");
    let absolute = format!("{}/absolute.md", normalized(dir.path()));
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "docs/**", "reason": "文档冻结：%s（%t）" },
                { "glob": absolute }
            ] }
        }),
    );

    let loaded = ProtectRules::load(Some(dir.path()));
    assert_eq!(
        loaded.rules.len(),
        rules::builtin_rules().len() + 2,
        "内置数 + 两条用户规则"
    );
    assert!(
        matches!(loaded.rules[0].reason, ReasonKind::BuiltinConfig),
        "内置在前（位次即优先级）"
    );

    let target = format!("{}/docs/a.md", normalized(dir.path()));
    let reason = loaded
        .evaluate("edit", &serde_json::json!({ "path": target }), dir.path())
        .expect("用户规则命中");
    assert_eq!(
        reason,
        format!("文档冻结：{target}（edit）"),
        "用户模板 %s / %t 全量替换"
    );
}

#[test]
fn 配置失败面三形态只减用户面() {
    let missing = tempdir("load-fail-missing");
    let bad_json = tempdir("load-fail-json");
    write_config_text(bad_json.path(), "{");
    let top_array = tempdir("load-fail-array");
    write_config_text(top_array.path(), "[]");

    for dir in [&missing, &bad_json, &top_array] {
        let loaded = ProtectRules::load(Some(dir.path()));
        assert_eq!(
            loaded.rules.len(),
            rules::builtin_rules().len(),
            "配置失败面只剩内置规则（不 panic），目录: {:?}",
            dir.path()
        );
        assert!(
            loaded
                .evaluate(
                    "write",
                    &serde_json::json!({ "path": protected_path(dir.path()) }),
                    dir.path(),
                )
                .is_some(),
            "内置命中面在配置失败时照常生效"
        );
    }
}

#[test]
fn root边界形态不panic且内置恒在() {
    let long_root = PathBuf::from(format!("{}/中文🚀/{}", "a".repeat(600), "b".repeat(600)));
    for root in [
        PathBuf::new(),
        PathBuf::from("D:/__not_exists__/sdk-protect-root"),
        long_root,
    ] {
        let loaded = ProtectRules::load(Some(&root));
        assert!(
            !loaded.rules.is_empty(),
            "内置规则恒在（不因 root 形态失效），root: {root:?}"
        );
        let reason = loaded.evaluate(
            "write",
            &serde_json::json!({ "path": "/任意前缀/openspec/config.json" }),
            &root,
        );
        assert!(reason.is_some(), "内置命中面恒在，root: {root:?}");
    }
}

#[test]
fn 快照语义_run内改配置不改本轮判定() {
    let dir = tempdir("snapshot");
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [ { "glob": "docs/**", "reason": "文档冻结" } ] }
        }),
    );
    let snapshot_a = ProtectRules::load(Some(dir.path()));
    let second_target = format!("{}/src/x.rs", normalized(dir.path()));
    assert!(
        snapshot_a
            .evaluate(
                "write",
                &serde_json::json!({ "path": second_target }),
                dir.path()
            )
            .is_none(),
        "快照 A 不含后加规则"
    );

    // 改配置新增第二条规则（本轮快照不随之变化）
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "docs/**", "reason": "文档冻结" },
                { "glob": "src/**", "reason": "源码冻结" }
            ] }
        }),
    );
    assert!(
        snapshot_a
            .evaluate(
                "write",
                &serde_json::json!({ "path": second_target }),
                dir.path()
            )
            .is_none(),
        "旧快照判定不随配置文件改动变化（每 run 一份纯数据）"
    );

    let snapshot_b = ProtectRules::load(Some(dir.path()));
    assert!(
        snapshot_b
            .evaluate(
                "write",
                &serde_json::json!({ "path": second_target }),
                dir.path()
            )
            .is_some(),
        "下一 run 重新装载即见新规则"
    );
}

#[test]
fn 主仓口径双根_规则取自主仓而worktree副本照拦() {
    let main_root = tempdir("main-root");
    let worktree = tempdir("worktree-root");
    write_config(
        main_root.path(),
        serde_json::json!({
            "write_protection": { "files": [ { "glob": "docs/**", "reason": "主仓文档冻结" } ] }
        }),
    );
    // worktree 内的配置副本（内置 glob 不锚定 root，照拦）
    write_config(
        worktree.path(),
        serde_json::json!({ "schema": "spec-driven" }),
    );

    let loaded = ProtectRules::load(Some(main_root.path()));
    let worktree_protected = worktree.path().join(config_relative());
    assert!(worktree_protected.is_file(), "worktree 配置副本真实落盘");

    let worktree_copy = format!("{}/{}", normalized(worktree.path()), config_relative());
    assert!(
        loaded
            .evaluate(
                "write",
                &serde_json::json!({ "path": worktree_copy }),
                worktree.path()
            )
            .is_some(),
        "worktree 内配置副本仍被内置规则拦（glob 不锚定 root）"
    );

    for (label, target) in [
        (
            "主仓用户规则命中",
            format!("{}/docs/a.md", normalized(main_root.path())),
        ),
        (
            "worktree 同形路径跨前缀命中",
            format!("{}/docs/a.md", normalized(worktree.path())),
        ),
    ] {
        let reason = loaded
            .evaluate(
                "write",
                &serde_json::json!({ "path": target }),
                worktree.path(),
            )
            .unwrap_or_else(|| panic!("{label} 应命中: {target}"));
        assert!(reason.contains("主仓文档冻结"), "{label} 取自主仓规则");
    }

    assert!(
        loaded
            .evaluate(
                "write",
                &serde_json::json!({ "path": format!("{}/src/x.rs", normalized(worktree.path())) }),
                worktree.path(),
            )
            .is_none(),
        "非受保护源码路径放行"
    );
}

// ---------------------------------------------------------------------------
// evaluate：write / edit 路径面
// ---------------------------------------------------------------------------

#[test]
fn write命中内置返回单条内置拒绝串() {
    let dir = tempdir("eval-write");
    let loaded = ProtectRules::load(None);
    let path = protected_path(dir.path());
    let reason = loaded
        .evaluate(
            "write",
            &serde_json::json!({ "path": path, "content": "x" }),
            dir.path(),
        )
        .expect("内置规则命中");

    assert!(reason.contains(&path), "拒绝串含归一后的路径原文: {reason}");
    assert!(
        reason.contains("配置权威"),
        "内置拒绝串五要素在场: {reason}"
    );
    assert!(
        reason.contains("不要改用其他命令或工具尝试绕过"),
        "禁绕过功能句在场: {reason}"
    );
}

#[test]
fn edit命中用户规则且占位符已替换() {
    let dir = tempdir("eval-edit");
    let loaded = with_rules(vec![user_rule("**/docs/**", Some("受保护：%s（%t）"))]);
    let target = format!("{}/docs/a.md", normalized(dir.path()));
    let reason = loaded
        .evaluate("edit", &serde_json::json!({ "path": target }), dir.path())
        .expect("用户规则命中");
    assert_eq!(reason, format!("受保护：{target}（edit）"));
}

#[test]
fn 判定零io零副作用且不查盘() {
    let dir = tempdir("eval-zero-io");
    // 受保护文件与普通文件真实落盘（判定前后字节 / 存在性比对）
    write_config(dir.path(), serde_json::json!({ "schema": "spec-driven" }));
    let notes = dir.path().join("notes.txt");
    std::fs::write(&notes, "原始内容").expect("落盘普通文件");

    let before = snapshot(dir.path());
    let loaded = ProtectRules::load(None);

    // 磁盘上不存在的受保护路径同样命中（判定不 canonicalize、不查盘）
    let phantom = dir.path().join("phantom").join(config_relative());
    assert!(
        loaded
            .evaluate(
                "write",
                &serde_json::json!({ "path": phantom.to_string_lossy() }),
                dir.path(),
            )
            .is_some(),
        "不存在的路径照判（零 IO）"
    );
    for command in [
        "echo x > openspec/config.json",
        "rm openspec/config.json",
        "git stash",
    ] {
        let _ = loaded.evaluate(
            "bash",
            &serde_json::json!({ "command": command }),
            dir.path(),
        );
    }

    assert_eq!(before, snapshot(dir.path()), "判定前后目录快照完全一致");
    assert_eq!(
        std::fs::read_to_string(&notes).expect("普通文件可读"),
        "原始内容",
        "判定不改动任何文件字节"
    );
}

#[test]
fn 射程遍历仅三工具可判() {
    let dir = tempdir("eval-scope");
    let loaded = ProtectRules::load(None);
    let path = protected_path(dir.path());

    for tool in [
        "read",
        "grep",
        "glob",
        "ls",
        "create_explore_note",
        "write_many",
    ] {
        assert!(
            loaded
                .evaluate(tool, &serde_json::json!({ "path": path }), dir.path())
                .is_none(),
            "只读四工具 / 专用工具 / 未知名恒放行: {tool}"
        );
    }
    for tool in ["write", "edit"] {
        assert!(
            loaded
                .evaluate(tool, &serde_json::json!({ "path": path }), dir.path())
                .is_some(),
            "路径面工具命中: {tool}"
        );
    }
    assert!(
        loaded
            .evaluate(
                "bash",
                &serde_json::json!({ "command": format!("rm {}", config_relative()) }),
                dir.path(),
            )
            .is_some(),
        "bash 臂命中"
    );
}

#[test]
fn write与edit入参异常fail_open而多余字段不影响命中() {
    let dir = tempdir("eval-input-anomaly");
    let loaded = ProtectRules::load(None);
    for input in [
        serde_json::json!({}),
        serde_json::json!({ "path": null }),
        serde_json::json!({ "path": 5 }),
        serde_json::json!({ "path": [] }),
        serde_json::json!({ "path": {} }),
        serde_json::json!({ "path": true }),
    ] {
        for tool in ["write", "edit"] {
            assert!(
                loaded.evaluate(tool, &input, dir.path()).is_none(),
                "path 缺失 / 非字符串一律放行（错误归属执行体）: {tool} {input}"
            );
        }
    }

    let with_extra = serde_json::json!({
        "path": protected_path(dir.path()),
        "content": "x",
        "extra": 1
    });
    assert!(
        loaded.evaluate("write", &with_extra, dir.path()).is_some(),
        "多余字段不影响命中（只取 path）"
    );
}

#[test]
fn bash入参异常fail_open() {
    let dir = tempdir("eval-bash-anomaly");
    let loaded = ProtectRules::load(None);
    for input in [
        serde_json::json!({}),
        serde_json::json!({ "command": null }),
        serde_json::json!({ "command": 5 }),
        serde_json::json!({ "command": "" }),
        serde_json::json!({ "command": "   \n\t" }),
    ] {
        assert!(
            loaded.evaluate("bash", &input, dir.path()).is_none(),
            "command 缺失 / 非字符串 / 空白一律放行: {input}"
        );
    }
}

#[test]
fn windows前缀与反斜杠归一后仍命中() {
    let dir = tempdir("eval-windows");
    let loaded = ProtectRules::load(None);
    let cases = [
        (r"\\?\D:\ws\openspec\config.json", true),
        (r"D:\ws\openspec\config.json", true),
        (r"D:\ws\openspec\config.jsonx", false),
    ];
    for (path, expected) in cases {
        let hit = loaded
            .evaluate("write", &serde_json::json!({ "path": path }), dir.path())
            .is_some();
        assert_eq!(hit, expected, "归一后命中面，路径: {path}");
    }
}

#[test]
fn 双点段保留而点段消解后均命中() {
    let dir = tempdir("eval-dot-segments");
    let anchor = normalized(dir.path());
    let loaded = ProtectRules::load(None);
    for path in [
        format!("{anchor}/a/../{}", config_relative()),
        format!("{anchor}/./{}", config_relative()),
    ] {
        assert!(
            loaded
                .evaluate("write", &serde_json::json!({ "path": path }), dir.path())
                .is_some(),
            "`..` 段保留 / `.` 段消解后仍命中，路径: {path}"
        );
    }
}

// ---------------------------------------------------------------------------
// evaluate：bash 命令面
// ---------------------------------------------------------------------------

#[test]
fn bash相对路径按cwd解析() {
    let dir = tempdir("eval-bash-cwd");
    let nested = dir.path().join("nested");
    let loaded = ProtectRules::load(None);
    let command = serde_json::json!({ "command": format!("echo {{}} > {}", config_relative()) });

    let at_root = loaded
        .evaluate("bash", &command, dir.path())
        .expect("cwd 为 root 时命中");
    assert!(
        at_root.contains(&format!("{}/{}", normalized(dir.path()), config_relative())),
        "拒绝串携带以 cwd 解析出的绝对路径，实际: {at_root}"
    );

    let at_nested = loaded
        .evaluate("bash", &command, &nested)
        .expect("相对路径按各自 cwd 解析（`**/` 规则不锚定具体 root）");
    assert!(
        at_nested.contains(&format!("{}/{}", normalized(&nested), config_relative())),
        "落点随 cwd 变化，实际: {at_nested}"
    );
    assert_ne!(at_root, at_nested, "两 cwd 的拒绝串不同");

    assert!(
        loaded
            .evaluate(
                "bash",
                &serde_json::json!({ "command": "rm notes.txt" }),
                dir.path(),
            )
            .is_none(),
        "非受保护目标放行"
    );
}

#[test]
fn bash反斜杠命令文本先归一() {
    let dir = tempdir("eval-bash-slash");
    let loaded = ProtectRules::load(None);
    let command = format!("echo {{}} > {}", config_relative().replace('/', "\\"));
    assert!(
        loaded
            .evaluate(
                "bash",
                &serde_json::json!({ "command": command }),
                dir.path()
            )
            .is_some(),
        "整串命令先 `\\`→`/` 归一后命中"
    );
}

#[test]
fn bash三路组合锚_五写法与批量还原分别命中() {
    let dir = tempdir("eval-bash-three-routes");
    let loaded = ProtectRules::load(None);
    let config = config_relative();
    let commands = [
        format!("echo {{}} > {config}"),
        format!("rm {config}"),
        format!("cp /tmp/x {config}"),
        format!("node -e \"require('fs').writeFileSync('{config}','{{}}')\""),
        format!("sed -i s/a/b/ {config}"),
        "git stash".to_owned(),
    ];
    for command in commands {
        assert!(
            loaded
                .evaluate(
                    "bash",
                    &serde_json::json!({ "command": command }),
                    dir.path()
                )
                .is_some(),
            "等效写法各自命中（重定向 / 删除 / cp / inline / sed -i / bulk），命令: {command}"
        );
    }
}

#[test]
fn bash三路判定次序_文件操作优先于内联优先于批量还原() {
    let dir = tempdir("eval-order");
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [ { "glob": "docs/**", "reason": "文档冻结：%s" } ] }
        }),
    );
    let loaded = ProtectRules::load(Some(dir.path()));

    let inline_first = loaded
        .evaluate(
            "bash",
            &serde_json::json!({
                "command": "node -e \"fs.writeFileSync('docs/a.md','')\" && git stash"
            }),
            dir.path(),
        )
        .expect("inline 路命中");
    assert!(
        inline_first.contains("文档冻结"),
        "内联路串（用户模板替换后），实际: {inline_first}"
    );
    assert!(
        !inline_first.contains("stash"),
        "MUST NOT 落到批量还原串（次序：inline 先于 bulk），实际: {inline_first}"
    );

    let file_first = loaded
        .evaluate(
            "bash",
            &serde_json::json!({ "command": "echo {} > docs/a.md && git stash" }),
            dir.path(),
        )
        .expect("文件操作路命中");
    assert!(
        file_first.contains("文档冻结"),
        "文件操作路串，实际: {file_first}"
    );
    assert!(
        !file_first.contains("批量还原"),
        "文件操作用例不落到批量还原串，实际: {file_first}"
    );
}

#[test]
fn bash树落点按子树相交判定() {
    let dir = tempdir("eval-tree");
    let loaded = ProtectRules::load(None);
    let cases = [
        ("find . -delete", true),
        ("find src -delete", false),
        ("find . -print", false),
    ];
    for (command, expected) in cases {
        let hit = loaded
            .evaluate(
                "bash",
                &serde_json::json!({ "command": command }),
                dir.path(),
            )
            .is_some();
        assert_eq!(hit, expected, "树落点判定，命令: {command}");
    }
}

#[test]
fn 精度双向_普通目标不误拦而同形受保护目标全拦() {
    let dir = tempdir("eval-precision");
    let loaded = ProtectRules::load(None);
    let config = config_relative();

    let allowed = [
        "echo foo > notes.txt".to_owned(),
        "cat <<EOF > notes.txt\nrm -rf /\nEOF".to_owned(),
        "echo \"rm -rf /\" > notes.txt".to_owned(),
        "echo a->b".to_owned(),
        "ls 2>&1".to_owned(),
    ];
    for command in &allowed {
        assert!(
            loaded
                .evaluate(
                    "bash",
                    &serde_json::json!({ "command": command }),
                    dir.path()
                )
                .is_none(),
            "普通目标不误拦，命令: {command}"
        );
    }

    let blocked = [
        format!("echo foo > {config}"),
        format!("cat <<EOF > {config}\nrm -rf /\nEOF"),
        format!("echo \"rm -rf /\" > {config}"),
    ];
    for command in &blocked {
        assert!(
            loaded
                .evaluate(
                    "bash",
                    &serde_json::json!({ "command": command }),
                    dir.path()
                )
                .is_some(),
            "同一装置换成受保护目标即拦，命令: {command}"
        );
    }
}

#[test]
fn 首个命中即止且不拼接多条串() {
    let dir = tempdir("eval-first-hit");
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "docs/**", "reason": "首条模板：%s" },
                { "glob": "docs/secret/**", "reason": "次条模板：%s" }
            ] }
        }),
    );
    let loaded = ProtectRules::load(Some(dir.path()));
    let target = format!("{}/docs/secret/x.md", normalized(dir.path()));
    let reason = loaded
        .evaluate("write", &serde_json::json!({ "path": target }), dir.path())
        .expect("首条规则命中");
    assert_eq!(
        reason,
        format!("首条模板：{target}"),
        "返回首条模板全文（MUST NOT 拼接第二条）"
    );
    assert!(!reason.contains("次条模板"), "不含第二条模板片段");

    // 内置与用户规则同时命中 → 返回内置串（内置在首位）
    let both = tempdir("eval-first-hit-builtin");
    write_config(
        both.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": config_relative(), "reason": "用户模板：%s" }
            ] }
        }),
    );
    let loaded = ProtectRules::load(Some(both.path()));
    let reason = loaded
        .evaluate(
            "write",
            &serde_json::json!({ "path": protected_path(both.path()) }),
            both.path(),
        )
        .expect("两条规则同时命中");
    assert!(reason.contains("配置权威"), "内置串优先，实际: {reason}");
    assert!(
        !reason.contains("用户模板"),
        "不返回用户模板，实际: {reason}"
    );
}

#[test]
fn 畸形与超长输入不panic且不挂起() {
    let dir = tempdir("eval-malformed");
    let loaded = ProtectRules::load(None);
    let config = config_relative();

    let long_command = format!("echo {} > {config}", "y".repeat(100_000));
    assert!(
        loaded
            .evaluate(
                "bash",
                &serde_json::json!({ "command": long_command }),
                dir.path(),
            )
            .is_some(),
        "100 KB 命令内 1 处受保护写入仍命中（不挂起）"
    );

    for command in [
        format!("echo 中文🚀\u{0} > {config}"),
        format!("echo 🚀 > {config}"),
        format!("sed -i s/中文/🚀/ {config}"),
    ] {
        assert!(
            loaded
                .evaluate(
                    "bash",
                    &serde_json::json!({ "command": command }),
                    dir.path()
                )
                .is_some(),
            "emoji / 中文 / NUL 命令文本不 panic，命令: {command:?}"
        );
    }

    let long_path = format!("/ws/{}", "长".repeat(1_200));
    assert!(
        loaded
            .evaluate(
                "write",
                &serde_json::json!({ "path": long_path }),
                dir.path()
            )
            .is_none(),
        "1000+ 字符路径不 panic（不命中）"
    );
}

#[test]
fn 入参面无档位量_判定确定性() {
    let dir = tempdir("eval-no-mode");
    let loaded = ProtectRules::load(None);
    let input = serde_json::json!({ "path": protected_path(dir.path()) });
    // 签名 `evaluate(&self, tool, input, cwd)` 三参不含 permission-mode：同一规则
    // 集与入参下判定结果与档位无关（档位面的闸门行为由 `loop_test` 承接）
    let first = loaded.evaluate("write", &input, dir.path());
    let second = loaded.evaluate("write", &input, dir.path());
    assert_eq!(first, second, "纯函数确定性（入参面零档位量）");
    assert!(first.is_some(), "内置命中不受调用序影响");
}

// ---------------------------------------------------------------------------
// 类型面（ProtectRules）
// ---------------------------------------------------------------------------

#[test]
fn 类型面_clone判定等价与空规则集恒放行() {
    let dir = tempdir("eval-type");
    let loaded = with_rules(vec![user_rule("**/docs/**", Some("受保护：%s"))]);
    let cloned = loaded.clone();
    let input = serde_json::json!({ "path": format!("{}/docs/a.md", normalized(dir.path())) });
    assert_eq!(
        loaded.evaluate("write", &input, dir.path()),
        cloned.evaluate("write", &input, dir.path()),
        "clone 后判定等价（每 run 一份纯数据快照）"
    );
    assert!(
        format!("{loaded:?}").contains("rules"),
        "Debug 渲染含规则容器，实际: {loaded:?}"
    );

    // 空规则集：防御性形态（生产不产生），判定恒 None
    let empty = ProtectRules { rules: Vec::new() };
    for (tool, input) in [
        (
            "write",
            serde_json::json!({ "path": "/ws/openspec/config.json" }),
        ),
        (
            "edit",
            serde_json::json!({ "path": "/ws/openspec/config.json" }),
        ),
        (
            "bash",
            serde_json::json!({ "command": "rm openspec/config.json" }),
        ),
    ] {
        assert!(
            empty.evaluate(tool, &input, dir.path()).is_none(),
            "空规则集恒放行: {tool}"
        );
    }
}
