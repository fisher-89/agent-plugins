//! `rules` 的单元测试（AC-1 / AC-2 / AC-6）：规则装配（内置恒在 + 用户规则
//! 合并）、glob 派生、路径归一 / 解析与子树相交判定。
//!
//! 断言口径：
//! - 命中判定一律经 `checks::globmatch::match_glob`（D7 glob 方言单点，测试侧
//!   不自造第二份匹配实现）；
//! - 派生断言经 `foundation::layout` 复算（源码零目录名 / 文件名字面量）；
//! - 配置文件面不 mock：`tempfile::TempDir` 真实目录 + `layout::config_path`
//!   派生的真实 JSON 文件，`config::load` 真实读取。

use std::path::Path;

use checks::globmatch::match_glob;
use foundation::layout;

use super::rules::{self, BulkGlobs, ReasonKind, Rule};

// ---------------------------------------------------------------------------
// 装置：临时目录 + 真实配置文件（内容以 serde_json 组装，免手写转义）
// ---------------------------------------------------------------------------

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-protect-rules-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

/// 按 `foundation::layout::config_path(root)` 落盘配置文件（目录自动创建）。
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

/// 原样落盘配置文件文本（坏 JSON / 非对象顶层等畸形形态）。
fn write_config_text(root: &Path, text: &str) {
    let path = layout::config_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建配置目录失败");
    }
    std::fs::write(&path, text).expect("写配置文件失败");
}

/// 内置 glob（由 `layout` 派生的唯一内置规则模式）。
fn builtin_glob() -> String {
    rules::builtin_rules()
        .into_iter()
        .next()
        .expect("内置规则恒在")
        .glob
}

/// 配置文件名字面量（首段域目录名 + 文件名，全部由 layout 派生）。
fn config_relative() -> String {
    rules::normalize(&layout::config_path(Path::new("")).to_string_lossy())
}

/// 用户规则载体（`reason` 三态直构，不走 IO）。
fn user_rule(glob: &str, reason: Option<&str>) -> Rule {
    Rule {
        glob: glob.to_owned(),
        reason: ReasonKind::User(reason.map(str::to_owned)),
    }
}

// ---------------------------------------------------------------------------
// builtin_rules（内置锁定）
// ---------------------------------------------------------------------------

#[test]
fn 内置规则恰一条且glob与reason由layout派生() {
    let builtin = rules::builtin_rules();
    assert_eq!(builtin.len(), 1, "内置面恒一条（不可配置、不可移除）");
    assert_eq!(
        builtin[0].glob,
        format!("**/{}", config_relative()),
        "内置 glob = `**/` + layout 派生的配置文件名路径"
    );
    assert!(
        matches!(builtin[0].reason, ReasonKind::BuiltinConfig),
        "内置规则 reason 为 BuiltinConfig 变体（非文案比对认内置）"
    );
}

#[test]
fn 内置glob对任意前缀与任意深度均命中() {
    let glob = builtin_glob();
    let relative = config_relative();
    let cases = [
        format!("/{}", relative),
        format!("D:/ws/{}", relative),
        relative.clone(),
        format!("/a/b/c/{}", relative),
    ];
    for path in cases {
        assert!(
            match_glob(&path, &glob),
            "内置 glob 覆盖任意前缀（含零段 `**/`），路径: {path}，glob: {glob}"
        );
    }
}

#[test]
fn 内置glob对近似形态不命中() {
    let glob = builtin_glob();
    let relative = config_relative();
    let cases = [
        format!("/ws/{}.bak", relative),
        format!("/ws/{}/x", relative),
        "/ws/explores/config.json".to_owned(),
        "/ws/config.json".to_owned(),
        format!("/ws/{}", relative.to_uppercase()),
    ];
    for path in cases {
        assert!(
            !match_glob(&path, &glob),
            "近似形态不命中（大小写敏感），路径: {path}"
        );
    }
}

// ---------------------------------------------------------------------------
// load（规则合并）
// ---------------------------------------------------------------------------

#[test]
fn load未注入root时仅内置且非空() {
    let loaded = rules::load(None);
    assert_eq!(
        loaded.len(),
        rules::builtin_rules().len(),
        "None → 只装载内置规则"
    );
    assert!(
        matches!(loaded[0].reason, ReasonKind::BuiltinConfig),
        "首位恒为内置规则（不产出「保护关闭」空集）"
    );
}

#[test]
fn load注入root时内置在前且用户规则按配置序追加() {
    let dir = tempdir("merge-order");
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "docs/**", "reason": "文档冻结" },
                { "glob": "src/gen/*.rs" }
            ] }
        }),
    );

    let loaded = rules::load(Some(dir.path()));
    assert_eq!(loaded.len(), 3, "内置 + 两条用户规则");
    assert!(
        matches!(loaded[0].reason, ReasonKind::BuiltinConfig),
        "内置在前（位次即优先级）"
    );
    assert_eq!(loaded[1].glob, "**/docs/**", "相对 glob 前缀 `**/`");
    assert_eq!(loaded[2].glob, "**/src/gen/*.rs", "第二条按配置序追加");
}

#[test]
fn load配置失败面五形态仅减用户面且内置恒在() {
    // ① 无配置文件
    let missing = tempdir("load-missing");
    // ② 坏 JSON ③ 顶层非对象 ④ write_protection 非对象 ⑤ files 非数组
    let bad_json = tempdir("load-bad-json");
    write_config_text(bad_json.path(), "{");
    let top_array = tempdir("load-top-array");
    write_config_text(top_array.path(), "[]");
    let protection_str = tempdir("load-protection-str");
    write_config_text(protection_str.path(), r#"{"write_protection": "x"}"#);
    let files_object = tempdir("load-files-object");
    write_config_text(
        files_object.path(),
        r#"{"write_protection": {"files": {}}}"#,
    );

    for dir in [
        &missing,
        &bad_json,
        &top_array,
        &protection_str,
        &files_object,
    ] {
        let loaded = rules::load(Some(dir.path()));
        assert_eq!(
            loaded.len(),
            rules::builtin_rules().len(),
            "配置失败面只减用户规则、不 panic，目录: {:?}",
            dir.path()
        );
        assert!(
            matches!(loaded[0].reason, ReasonKind::BuiltinConfig),
            "内置规则独立于配置解析结果（fail-closed）"
        );
        assert!(
            match_glob(&format!("/任意前缀/{}", config_relative()), &loaded[0].glob),
            "内置命中面在配置失败时照常生效"
        );
    }
}

#[test]
fn load_root异常形态不panic且内置恒在() {
    let long_root = format!("{}/中文🚀/{}", "a".repeat(600), "b".repeat(600));
    let roots = [
        std::path::PathBuf::new(),
        std::path::PathBuf::from("D:/__not_exists__/sdk-protect"),
        std::path::PathBuf::from(long_root),
    ];
    for root in roots {
        let loaded = rules::load(Some(&root));
        assert!(!loaded.is_empty(), "内置规则恒在，root: {root:?}");
        assert!(
            matches!(loaded[0].reason, ReasonKind::BuiltinConfig),
            "首位恒内置，root: {root:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// user_rules（用户规则装配）
// ---------------------------------------------------------------------------

#[test]
fn 用户规则相对glob加双星前缀且跨前缀命中() {
    let dir = tempdir("user-relative");
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "plugins/dev-team/bin/knip.json" }
            ] }
        }),
    );

    let produced = rules::user_rules(dir.path());
    assert_eq!(produced.len(), 1);
    assert_eq!(
        produced[0].glob, "**/plugins/dev-team/bin/knip.json",
        "相对 glob 加 `**/` 前缀（本仓现行真实规则形态）"
    );
    assert!(
        match_glob(
            &format!(
                "{}/plugins/dev-team/bin/knip.json",
                dir.path().to_string_lossy()
            ),
            &produced[0].glob
        ),
        "root 内命中"
    );
    assert!(
        match_glob(
            "/另一前缀/plugins/dev-team/bin/knip.json",
            &produced[0].glob
        ),
        "`**/` 前缀使规则跨前缀命中"
    );
    assert!(
        !match_glob(
            "/另一前缀/plugins/dev-team/bin/knip.json5",
            &produced[0].glob
        ),
        "扩展名不同不命中"
    );
}

#[test]
fn 用户规则绝对glob原样且不跨根命中() {
    let dir = tempdir("user-absolute");
    let other = tempdir("user-absolute-other");
    let raw_glob = format!("{}/secret.md", dir.path().to_string_lossy());
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [ { "glob": raw_glob } ] }
        }),
    );

    let produced = rules::user_rules(dir.path());
    assert_eq!(produced.len(), 1);
    assert_eq!(
        produced[0].glob,
        raw_glob.replace('\\', "/"),
        "绝对 glob 原样（不加 `**/`，仅分隔符归一）"
    );
    assert!(
        match_glob(&raw_glob, &produced[0].glob),
        "命中该绝对路径本身"
    );
    assert!(
        !match_glob(
            &format!("{}/secret.md", other.path().to_string_lossy()),
            &produced[0].glob
        ),
        "不命中另一前缀同名文件"
    );
}

#[test]
fn 用户规则reason三态_缺席落none_在场原样() {
    let without = tempdir("user-reason-none");
    write_config(
        without.path(),
        serde_json::json!({
            "write_protection": { "files": [ { "glob": "plugins/dev-team/bin/knip.json" } ] }
        }),
    );
    let produced = rules::user_rules(without.path());
    assert!(
        matches!(&produced[0].reason, ReasonKind::User(None)),
        "无 reason → User(None)（回落串面）"
    );

    let with = tempdir("user-reason-some");
    write_config(
        with.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "plugins/dev-team/bin/knip.json", "reason": "金样冻结" }
            ] }
        }),
    );
    let produced = rules::user_rules(with.path());
    match &produced[0].reason {
        ReasonKind::User(Some(template)) => assert_eq!(template, "金样冻结", "reason 原样保留"),
        other => panic!("应为 User(Some)，实际: {other:?}"),
    }
}

#[test]
fn 用户规则glob缺省条目被跳过不panic() {
    let dir = tempdir("user-glob-none");
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "reason": "只给 reason" },
                { "glob": "docs/**", "reason": "文档冻结" }
            ] }
        }),
    );

    let produced = rules::user_rules(dir.path());
    assert_eq!(produced.len(), 1, "glob 缺省条目跳过（只余一条）");
    assert_eq!(produced[0].glob, "**/docs/**");
}

#[test]
fn 用户规则空串与非法glob由组装层剔除_非法模式fail_open() {
    let dir = tempdir("user-glob-invalid");
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "" },
                { "glob": 5 },
                { "glob": "docs/**" }
            ] }
        }),
    );

    let produced = rules::user_rules(dir.path());
    assert_eq!(
        produced.len(),
        1,
        "空串 / 非字符串 glob 由配置组装层剔除，仅留合法条"
    );
    assert_eq!(produced[0].glob, "**/docs/**");

    // 直构非法模式驱动匹配面：不命中、不 panic（非法模式 fail-open）
    let broken = user_rule("[", None);
    assert!(
        !match_glob("/ws/docs/a.md", &broken.glob),
        "非法 glob 按不命中收敛（fail-open）"
    );
}

#[test]
fn 用户规则保序且重复条不折叠不去重() {
    let dir = tempdir("user-order");
    let absolute = format!("{}/absolute.md", dir.path().to_string_lossy());
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "docs/**" },
                { "glob": absolute },
                { "glob": "docs/**" },
                { "glob": "src/**", "reason": "源码冻结" }
            ] }
        }),
    );

    let produced = rules::user_rules(dir.path());
    assert_eq!(produced.len(), 4, "四条原样产出（重复条不去重）");
    assert_eq!(produced[0].glob, "**/docs/**");
    assert_eq!(produced[1].glob, absolute.replace('\\', "/"));
    assert_eq!(produced[2].glob, "**/docs/**", "重复条保留且位次不变");
    assert_eq!(produced[3].glob, "**/src/**");
}

#[test]
fn 用户规则glob边界字符不panic且不误命中() {
    let dir = tempdir("user-glob-edge");
    let long_glob = "a".repeat(1_200);
    write_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "   " },
                { "glob": "文档/🚀.md" },
                { "glob": long_glob },
                { "glob": "$OUT/**" }
            ] }
        }),
    );

    let produced = rules::user_rules(dir.path());
    assert_eq!(produced.len(), 4, "四条均产出（装配面不做语义校验）");
    assert!(
        !match_glob("/ws/a/b.txt", &produced[0].glob),
        "全空白 glob 不命中常规路径"
    );
    assert!(
        match_glob("/ws/文档/🚀.md", &produced[1].glob),
        "中文与 emoji 模式正常命中"
    );
    assert!(
        !match_glob("/ws/a/b.txt", &produced[2].glob),
        "1000+ 字符模式不误命中常规路径"
    );
    assert!(
        !match_glob("/ws/x.txt", &produced[3].glob),
        "含 `$` 模式按字面（不误命中）"
    );
}

// ---------------------------------------------------------------------------
// bulk_globs（批量还原的结构 glob）
// ---------------------------------------------------------------------------

#[test]
fn 批量还原两条结构glob由layout派生() {
    let globs: BulkGlobs = rules::bulk_globs();
    assert_eq!(
        globs.artifacts,
        format!("**/{}/**", layout::domain_dir_name()),
        "域目录树 glob 由 domain_dir_name 派生"
    );
    assert_eq!(
        globs.changes,
        format!(
            "**/{}/**",
            rules::normalize(
                &layout::resolve(Path::new(""))
                    .changes_root
                    .to_string_lossy()
            )
        ),
        "changes 子树 glob 由 layout::resolve 派生"
    );
}

#[test]
fn 批量还原glob命中面与窄面呈子集关系() {
    let globs = rules::bulk_globs();
    let domain = layout::domain_dir_name();
    let changes = rules::normalize(
        &layout::resolve(Path::new(""))
            .changes_root
            .to_string_lossy(),
    );
    let in_changes = format!("/root/{}/x/proposal.md", changes);
    let in_explores = format!("/root/{}/explores/a.md", domain);

    assert!(
        match_glob(&in_changes, &globs.changes),
        "changes glob 命中 change 产物"
    );
    assert!(
        !match_glob(&in_explores, &globs.changes),
        "changes glob 不命中探索笔记"
    );
    assert!(
        match_glob(&in_changes, &globs.artifacts),
        "域树命中 changes"
    );
    assert!(
        match_glob(&in_explores, &globs.artifacts),
        "域树命中 explores（changes 命中集 ⊆ artifacts 命中集）"
    );
}

// ---------------------------------------------------------------------------
// glob_dir_hit（子树相交判定）
// ---------------------------------------------------------------------------

#[test]
fn 子树判定_点搜索根落在anchor锚定面时命中() {
    let dir = tempdir("dir-hit-dot");
    let anchor = dir.path();
    assert!(
        rules::glob_dir_hit(".", &builtin_glob(), anchor),
        "`find . -delete` 的搜索根经 anchor 归一后覆盖受保护目标"
    );
}

#[test]
fn 子树判定_源码与相似名目录不越拦() {
    let dir = tempdir("dir-hit-source");
    let anchor = rules::normalize(&dir.path().to_string_lossy());
    let glob = builtin_glob();
    assert!(
        !rules::glob_dir_hit(&format!("{anchor}/src"), &glob, dir.path()),
        "源码子树不在受保护前缀内（不误拦）"
    );
    assert!(
        !rules::glob_dir_hit(
            &format!("{anchor}/{domain}x", domain = layout::domain_dir_name()),
            &glob,
            dir.path()
        ),
        "段边界：相似名目录不以受保护前缀起手（不误拦）"
    );
}

#[test]
fn 子树判定_祖先相等后代三面均命中() {
    let dir = tempdir("dir-hit-three-faces");
    let anchor = dir.path();
    let anchor_text = rules::normalize(&anchor.to_string_lossy());
    let glob = builtin_glob();
    let domain = layout::domain_dir_name();

    assert!(
        rules::glob_dir_hit(&anchor_text, &glob, anchor),
        "祖先面：删整棵 anchor 必然波及受保护目标"
    );
    assert!(
        rules::glob_dir_hit(&format!("{anchor_text}/{domain}"), &glob, anchor),
        "前缀面：dir 为受保护前缀的祖先段"
    );
    assert!(
        rules::glob_dir_hit(
            &format!("{anchor_text}/{}", config_relative()),
            &glob,
            anchor
        ),
        "相等面：搜索根即受保护目标本身"
    );
}

#[test]
fn 子树判定_绝对glob与anchor无关且反斜杠归一() {
    let anchor = Path::new("/unused/anchor");
    let glob = "D:/ws/docs/**";
    assert!(rules::glob_dir_hit("D:/ws", glob, anchor), "祖先面命中");
    assert!(
        !rules::glob_dir_hit("D:/other", glob, anchor),
        "无关目录不命中"
    );
    assert!(
        rules::glob_dir_hit(r"D:\ws", glob, anchor),
        "反斜杠形态归一后同样命中"
    );
}

#[test]
fn 子树判定_无静态段与非法glob不命中() {
    let dir = tempdir("dir-hit-degenerate");
    let anchor = dir.path();
    let anchor_text = rules::normalize(&anchor.to_string_lossy());

    assert!(
        rules::glob_dir_hit(&format!("{anchor_text}/any/sub"), "**/*", anchor),
        "`**/*` 退化为 anchor 基座：anchor 下任意子目录命中"
    );
    assert!(
        !rules::glob_dir_hit("/outside/dir", "**/*", anchor),
        "anchor 外目录不命中"
    );
    for glob in ["", "**/", "["] {
        assert!(
            !rules::glob_dir_hit(&anchor_text, glob, anchor),
            "纯 `**` / 空 / 非法 glob 不构成可命中规则: {glob:?}"
        );
    }
}

#[test]
fn 子树判定_目录参数边界不panic且不误判() {
    let dir = tempdir("dir-hit-edge");
    let anchor = dir.path();
    let glob = builtin_glob();

    for empty in ["", "   "] {
        assert!(
            !rules::glob_dir_hit(empty, &glob, anchor),
            "空 / 全空白目录不误判为命中: {empty:?}"
        );
    }
    assert!(
        !rules::glob_dir_hit(&"长".repeat(1_200), &glob, anchor),
        "1000+ 字符目录不 panic 且不命中"
    );
    assert!(
        !rules::glob_dir_hit("/ws/emoji🚀/目录", &glob, anchor),
        "emoji 目录不 panic 且不命中"
    );
}

// ---------------------------------------------------------------------------
// normalize（文本归一单点）
// ---------------------------------------------------------------------------

#[test]
fn 归一_剥windows前缀且分隔符转正斜杠() {
    assert_eq!(
        rules::normalize(r"\\?\D:\ws\openspec\config.json"),
        "D:/ws/openspec/config.json",
        r"剥 `\\?\` 前缀 + 反斜杠转 `/`"
    );
    assert_eq!(rules::normalize(r"a\b\c"), "a/b/c");
    assert_eq!(rules::normalize("a//b///c"), "a/b/c", "空段丢弃");
}

#[test]
fn 归一_点段消解且双点段保留() {
    assert_eq!(rules::normalize("a/./b"), "a/b", "`.` 段消解");
    assert_eq!(rules::normalize("."), "");
    assert_eq!(rules::normalize("./a"), "a");
    assert_eq!(
        rules::normalize("a/../b"),
        "a/../b",
        "`..` 段保留（不猜测语义）"
    );
    assert_eq!(rules::normalize("a/.."), "a/..");
}

#[test]
fn 归一_前导斜杠与尾部分隔符与根形态() {
    assert_eq!(rules::normalize("/a/b"), "/a/b", "前导 `/` 保留");
    assert_eq!(rules::normalize("a/b/"), "a/b", "尾部分隔符丢弃");
    assert_eq!(rules::normalize("/"), "/", "根形态");
    assert_eq!(rules::normalize("///"), "/");
    assert_eq!(
        rules::normalize("D:/"),
        "D:",
        "盘符形态去尾分隔符（相对形态无前导 `/`）"
    );
}

#[test]
fn 归一_空串特殊字符与幂等() {
    assert_eq!(rules::normalize(""), "");
    for raw in ["中文/路径🚀/a.md", "a b/c\td.txt", "  /a/b  "] {
        let once = rules::normalize(raw);
        assert!(
            once.contains("中文") || once.contains("🚀") || once.contains("a"),
            "特殊字符逐字符保留，输入: {raw:?}，产物: {once:?}"
        );
        assert_eq!(
            rules::normalize(&once),
            once,
            "幂等：normalize(normalize(x)) == normalize(x)，输入: {raw:?}"
        );
    }
    let long = format!("/{}/a.md", "长".repeat(1_200));
    assert_eq!(
        rules::normalize(&long).chars().count(),
        long.chars().count(),
        "1000+ 字符输入不 panic 且不丢字符"
    );
}

// ---------------------------------------------------------------------------
// resolve（提取路径解析）
// ---------------------------------------------------------------------------

#[test]
fn 解析_绝对路径原样归一() {
    assert_eq!(
        rules::resolve(Path::new("/anchor"), r"D:\ws\a.md"),
        Some("D:/ws/a.md".to_owned()),
        "盘符形态按绝对处理（不拼 anchor）"
    );
    assert_eq!(
        rules::resolve(Path::new("/anchor"), "/x/y.md"),
        Some("/x/y.md".to_owned())
    );
}

#[test]
fn 解析_相对路径拼anchor后归一() {
    let dir = tempdir("resolve-relative");
    let anchor = dir.path();
    let anchor_text = rules::normalize(&anchor.to_string_lossy());

    assert_eq!(
        rules::resolve(anchor, &config_relative()),
        Some(format!("{anchor_text}/{}", config_relative())),
        "相对路径拼 anchor 后归一"
    );
    assert_eq!(
        rules::resolve(anchor, "./a/./b.txt"),
        Some(format!("{anchor_text}/a/b.txt")),
        "`.` 段消解"
    );
    assert_eq!(
        rules::resolve(anchor, "../x.txt"),
        Some(format!("{anchor_text}/../x.txt")),
        "`..` 段保留后拼（不消解）"
    );
}

#[test]
fn 解析_空白与变量形态放行() {
    let dir = tempdir("resolve-drop");
    let anchor = dir.path();
    assert_eq!(rules::resolve(anchor, ""), None, "空串 → None");
    assert_eq!(rules::resolve(anchor, "   "), None, "全空白 → None");
    assert_eq!(rules::resolve(anchor, "$HOME/x"), None, "`$` 形态 → None");
    assert_eq!(rules::resolve(anchor, "${P}/x"), None);

    let literal = rules::resolve(anchor, r"%TEMP%\x").expect("非 `$` 形态按字面解析");
    assert!(
        literal.ends_with("/%TEMP%/x"),
        "百分号变量按字面参与归一，实际: {literal}"
    );
}

#[test]
fn 解析_边界字符不panic() {
    let dir = tempdir("resolve-edge");
    let anchor = dir.path();
    assert!(
        rules::resolve(anchor, &"长".repeat(1_200)).is_some(),
        "1000+ 字符相对路径不 panic"
    );
    assert!(
        rules::resolve(anchor, "目录/🚀.md").is_some(),
        "emoji 路径不 panic"
    );
    let quoted = rules::resolve(anchor, "'a b.txt'").expect("包裹引号按原样归一");
    assert!(
        quoted.ends_with("'a b.txt'"),
        "包裹引号原样参与归一，实际: {quoted}"
    );
}

// ---------------------------------------------------------------------------
// Rule::concrete_prefix（glob → concrete 静态前缀）
// ---------------------------------------------------------------------------

#[test]
fn 静态前缀_双星斜杠起手锚定anchor() {
    let dir = tempdir("prefix-builtin");
    let anchor = dir.path();
    let anchor_text = rules::normalize(&anchor.to_string_lossy());
    let rule = user_rule(&format!("**/{}", config_relative()), None);

    assert_eq!(
        rule.concrete_prefix(anchor),
        format!("{anchor_text}/{}", config_relative()),
        "`**/` 段被 anchor 置换"
    );
}

#[test]
fn 静态前缀_绝对glob与无通配形态() {
    let dir = tempdir("prefix-absolute");
    let anchor = dir.path();
    let anchor_text = rules::normalize(&anchor.to_string_lossy());

    let absolute = user_rule("D:/ws/secret.md", None);
    assert_eq!(
        absolute.concrete_prefix(anchor),
        "D:/ws/secret.md",
        "绝对 glob 即自身静态段（与 anchor 无关）"
    );

    let relative = user_rule("**/plugins/x/knip.json", None);
    assert_eq!(
        relative.concrete_prefix(anchor),
        format!("{anchor_text}/plugins/x/knip.json"),
        "无通配段保留全段"
    );
}

#[test]
fn 静态前缀_首个通配段截断与退化形态() {
    let dir = tempdir("prefix-truncate");
    let anchor = dir.path();
    let anchor_text = rules::normalize(&anchor.to_string_lossy());

    assert_eq!(
        user_rule("**/a/**/*.md", None).concrete_prefix(anchor),
        format!("{anchor_text}/a"),
        "首个通配段之前截断"
    );
    for glob in ["**/*", "**/"] {
        assert_eq!(
            user_rule(glob, None).concrete_prefix(anchor),
            anchor_text,
            "零静态段退化为 anchor 归一形态: {glob}"
        );
    }
    assert_eq!(
        user_rule("", None).concrete_prefix(anchor),
        "",
        "空 glob 不 panic（空前缀）"
    );
}

// ---------------------------------------------------------------------------
// 类型面（Rule / ReasonKind / BulkGlobs）
// ---------------------------------------------------------------------------

#[test]
fn 类型面_rule_clone等价且无reason变体可辨() {
    let rule = user_rule("**/docs/**", Some("文档冻结"));
    let cloned = rule.clone();
    assert_eq!(cloned.glob, rule.glob, "clone 后 glob 逐字段相等");
    assert!(
        format!("{rule:?}").contains("User"),
        "Debug 渲染含变体名，实际: {rule:?}"
    );

    // ReasonKind 三形态（封闭集）逐个构造与判别
    let kinds = [
        ReasonKind::BuiltinConfig,
        ReasonKind::User(Some("自有文案".to_owned())),
        ReasonKind::User(None),
    ];
    assert!(matches!(kinds[0], ReasonKind::BuiltinConfig));
    assert!(matches!(&kinds[1], ReasonKind::User(Some(t)) if t == "自有文案"));
    assert!(matches!(&kinds[2], ReasonKind::User(None)));

    // BulkGlobs 两字段可读且 clone 等价
    let globs = rules::bulk_globs();
    let cloned = globs.clone();
    assert_eq!(cloned.artifacts, globs.artifacts);
    assert_eq!(cloned.changes, globs.changes);
    assert!(
        format!("{globs:?}").contains("artifacts"),
        "Debug 渲染含字段名，实际: {globs:?}"
    );
}
