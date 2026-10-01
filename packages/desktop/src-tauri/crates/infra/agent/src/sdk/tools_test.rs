//! sdk 六工具面（read / grep / glob / ls / write / edit）的定义与执行体测试
//! （AC-5 / AC-6 的工具半边）：定义清单封闭性、逐工具执行体正反例与边界。
//! tempfile tempdir 真实目录驱动，glob 库以真实实现参与（workspace 直连依赖，
//! 不 mock）；执行体信任 loop 沙箱改写后的路径（不内嵌检查），沙箱链拒绝
//! 半边经 input_path → check 组合断言与 runner_test 假流缝全链承载。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::sdk::sandbox::check;
use crate::sdk::tools::{definitions, execute, input_path, TOOL_NAMES};

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-tools-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

fn root_of(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().to_path_buf()
}

/// 以 root 内相对路径落盘（返回绝对路径供执行体消费）。
fn write_rel(root: &Path, rel: &str, content: &str) -> PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("相对路径应有父目录")).expect("创建父目录失败");
    std::fs::write(&path, content).expect("写文件失败");
    path
}

// ---------------------------------------------------------------------------
// 六工具定义面（正向 + 封闭性边界）
// ---------------------------------------------------------------------------

#[test]
fn 定义清单恰为六工具且名称_描述_schema齐全() {
    let defs = definitions();
    assert_eq!(defs.len(), 6, "恰为六工具");
    let names: Vec<&str> = defs.iter().map(|def| def.name.as_str()).collect();
    assert_eq!(names, TOOL_NAMES.to_vec(), "定义面与 TOOL_NAMES 同源同序");
    for def in &defs {
        assert!(!def.description.is_empty(), "{} 描述非空", def.name);
        assert_eq!(
            def.parameters["type"],
            json!("object"),
            "{} 入参为 JSON schema object",
            def.name
        );
        assert!(
            def.parameters["properties"].is_object(),
            "{} 声明入参属性",
            def.name
        );
        assert!(
            def.parameters["required"]
                .as_array()
                .expect("required 数组")
                .iter()
                .all(Value::is_string),
            "{} required 为字符串数组",
            def.name
        );
    }
    // 关键入参形状抽查：glob 无 path 字段（pattern 驱动），其余五工具 path 必填
    let glob = defs
        .iter()
        .find(|def| def.name == "glob")
        .expect("glob 定义");
    assert!(glob.parameters["properties"]["pattern"].is_object());
    assert!(
        glob.parameters["properties"].get("path").is_none(),
        "glob 无 path 字段"
    );
    for name in ["read", "grep", "ls", "write", "edit"] {
        let def = defs.iter().find(|def| def.name == name).expect("定义在场");
        assert!(
            def.parameters["required"]
                .as_array()
                .expect("required 数组")
                .iter()
                .any(|value| value == "path"),
            "{name} 的 path 必填"
        );
    }
}

#[test]
fn bash不在工具面且清单外无多余条目() {
    // bash MUST NOT 进 MVP 工具面（缺席断言）
    assert!(!TOOL_NAMES.contains(&"bash"), "bash 不得进 MVP 工具面");
    assert!(
        definitions().iter().all(|def| def.name != "bash"),
        "定义面无 bash 条目"
    );
    // 封闭清单：无清单外多余条目
    let allowed = ["read", "grep", "glob", "ls", "write", "edit"];
    for name in TOOL_NAMES {
        assert!(allowed.contains(&name), "清单外条目 {name}");
    }
}

// ---------------------------------------------------------------------------
// read 执行体
// ---------------------------------------------------------------------------

#[tokio::test]
async fn read返回行号前缀内容且中文与emoji保真() {
    let dir = tempdir("read-ok");
    let root = root_of(&dir);
    let path = write_rel(&root, "note.md", "第一行 中文\nsecond line 🎉\n第三行");

    let output = execute(&root, "read", &json!({ "path": path }))
        .await
        .expect("读取成功");
    let mut lines = output.lines();
    let first = lines.next().expect("首行");
    assert!(first.contains("第一行 中文"), "内容保真: {first}");
    assert!(first.contains('\t'), "行号前缀以制表符分隔: {first}");
    for line in lines {
        assert!(
            line.contains("second line 🎉") || line.contains("第三行"),
            "{line}"
        );
    }
}

#[tokio::test]
async fn read_offset与limit截取行区间() {
    let dir = tempdir("read-window");
    let root = root_of(&dir);
    let path = write_rel(&root, "rows.txt", "一\n二\n三\n四\n五");

    // offset 1 起始：从第 2 行起取 2 行
    let output = execute(
        &root,
        "read",
        &json!({ "path": path, "offset": 2, "limit": 2 }),
    )
    .await
    .expect("读取成功");
    assert!(
        output.contains("二") && output.contains("三"),
        "实际: {output}"
    );
    assert!(
        !output.contains("一") && !output.contains("四"),
        "窗口外行不出现: {output}"
    );

    // 越界区间：不 panic，返回空区间占位
    let empty = execute(&root, "read", &json!({ "path": path, "offset": 99 })).await;
    assert!(empty.is_ok(), "越界区间显式占位不炸: {empty:?}");
}

#[tokio::test]
async fn read文件不存在返回is_error内容_root外路径经沙箱链拒绝() {
    let dir = tempdir("read-err");
    let root = root_of(&dir);
    let outside = tempdir("read-err-outside");
    let outside_file = outside.path().join("secret.txt");
    std::fs::write(&outside_file, "外部").expect("写外部文件");

    // 文件不存在 → Err(String)（is_error ToolResult 的内容来源）
    let missing = execute(&root, "read", &json!({ "path": root.join("no-such.txt") }))
        .await
        .expect_err("缺失文件必须 Err");
    assert!(missing.contains("读取失败"), "实际: {missing}");

    // root 外路径 → 沙箱链拒绝（loop 以 input_path 提取 + check 校验的同一
    // 组合顺序；执行体本身不内嵌检查，拦截半边归沙箱链）
    let input = json!({ "path": outside_file });
    let raw = input_path("read", &input).expect("path 字段在场");
    let denied = check(&root, raw).expect_err("root 外路径必须被沙箱链拦截");
    assert!(denied.contains("越出 workspace root"), "实际: {denied}");
}

// ---------------------------------------------------------------------------
// grep 执行体
// ---------------------------------------------------------------------------

#[tokio::test]
async fn grep多行多命中逐行返回且携路径行号() {
    let dir = tempdir("grep-ok");
    let root = root_of(&dir);
    let path = write_rel(
        &root,
        "code.rs",
        "fn main() {}\nlet 线索 = 1;\nfn helper() {}\n// 无关行\nlet 线索二 = 2;",
    );

    let output = execute(&root, "grep", &json!({ "path": path, "pattern": "fn " }))
        .await
        .expect("grep 成功");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2, "两处命中逐行返回: {output}");
    let first = lines[0];
    let second = lines[1];
    assert!(
        first.contains("1: fn main() {}"),
        "携路径:行号: 内容 — {first}"
    );
    assert!(second.contains("3: fn helper() {}"), "{second}");
}

#[tokio::test]
async fn grep无命中与空文件返回非错误_匹配串按子串字面语义() {
    let dir = tempdir("grep-edge");
    let root = root_of(&dir);
    let path = write_rel(
        &root,
        "text.txt",
        "包含 正则元字符 (.) 与 emoji 🎉 的行\n普通行",
    );
    let empty_path = write_rel(&root, "empty.txt", "");

    // 无命中：空清单语义（非错误的占位文案）
    let none = execute(
        &root,
        "grep",
        &json!({ "path": path, "pattern": "不存在的串" }),
    )
    .await
    .expect("无命中非错误");
    assert!(none.contains("无匹配行"), "实际: {none}");

    // 空文件：零命中非错误
    let on_empty = execute(
        &root,
        "grep",
        &json!({ "path": empty_path, "pattern": "任意" }),
    )
    .await
    .expect("空文件非错误");
    assert!(on_empty.contains("无匹配行"));

    // 中文 / emoji / 正则元字符按子串字面语义（非正则）
    for pattern in ["元字符 (.) 与", "🎉"] {
        let hit = execute(&root, "grep", &json!({ "path": path, "pattern": pattern }))
            .await
            .expect("子串命中");
        assert!(hit.contains("1: "), "命中首行: {hit}");
    }

    // 空 pattern：显式拒绝（非静默全行匹配）
    assert!(
        execute(&root, "grep", &json!({ "path": path, "pattern": "" }))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn grep目录路径递归匹配跨层级文件() {
    let dir = tempdir("grep-dir");
    let root = root_of(&dir);
    let top = write_rel(&root, "top.txt", "无关\n线索 here");
    let inner = write_rel(&root, "sub/inner.md", "线索 one\n无关");
    let leaf = write_rel(&root, "sub/deep/leaf.rs", "fn 线索() {}");

    let output = execute(&root, "grep", &json!({ "path": root, "pattern": "线索" }))
        .await
        .expect("目录递归非错误");
    // 目录分支输出路径统一 `/` 分隔（与 glob 工具同口径），期望值同步归一
    let flat = |path: &PathBuf| path.to_string_lossy().replace('\\', "/");
    assert!(
        output.contains(&format!("{}:2: 线索 here", flat(&top))),
        "顶层文件命中: {output}"
    );
    assert!(
        output.contains(&format!("{}:1: 线索 one", flat(&inner))),
        "子目录命中: {output}"
    );
    assert!(
        output.contains(&format!("{}:1: fn 线索() {{}}", flat(&leaf))),
        "深层目录命中: {output}"
    );
}

#[tokio::test]
async fn grep目录递归跳过二进制且命中上限截断留痕() {
    let dir = tempdir("grep-dir-edge");
    let root = root_of(&dir);
    // 205 行全命中的单文件：命中数越过上限，截断到 200 并留痕
    write_rel(&root, "bulk.txt", &"命中行\n".repeat(205));
    // 非 UTF-8 二进制：静默跳过不中断（glob 字典序下 blob.bin 先于 bulk.txt）
    let binary = root.join("blob.bin");
    std::fs::write(&binary, [0xFF, 0xFE, 0x00, 0x80]).expect("写二进制失败");

    let output = execute(&root, "grep", &json!({ "path": root, "pattern": "命中行" }))
        .await
        .expect("二进制在场不中断");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 201, "200 命中 + 1 截断留痕: {output}");
    assert!(lines.last().expect("末行").contains("已截断"), "{output}");
}

#[tokio::test]
async fn grep目录无命中返回占位_缺失路径回落文件读取错误() {
    let dir = tempdir("grep-dir-none");
    let root = root_of(&dir);
    write_rel(&root, "plain.txt", "无关内容");

    let none = execute(
        &root,
        "grep",
        &json!({ "path": root, "pattern": "不存在串" }),
    )
    .await
    .expect("目录无命中非错误");
    assert!(none.contains("无匹配行"), "实际: {none}");

    // metadata 失败（缺失路径）回落单文件分支：显式读取错误语义保留
    let missing = execute(
        &root,
        "grep",
        &json!({ "path": root.join("no-such"), "pattern": "x" }),
    )
    .await
    .expect_err("缺失路径必须 Err");
    assert!(missing.contains("读取失败"), "实际: {missing}");
}

// ---------------------------------------------------------------------------
// glob 执行体
// ---------------------------------------------------------------------------

#[tokio::test]
async fn glob命中单层与跨目录模式且路径字典序() {
    let dir = tempdir("glob-ok");
    let root = root_of(&dir);
    write_rel(&root, "a.rs", "a");
    write_rel(&root, "sub/b.rs", "b");
    write_rel(&root, "src/app.ts", "app");
    write_rel(&root, "src/lib/util.ts", "util");

    // 单层模式
    let flat = execute(&root, "glob", &json!({ "pattern": "*.rs" }))
        .await
        .expect("glob 成功");
    assert!(
        flat.contains("a.rs") && !flat.contains("b.rs"),
        "单层不跨目录: {flat}"
    );

    // 跨目录模式（** 形态）
    let deep = execute(&root, "glob", &json!({ "pattern": "src/**/*.ts" }))
        .await
        .expect("跨目录 glob 成功");
    assert!(
        deep.contains("app.ts") && deep.contains("util.ts"),
        "{deep}"
    );

    // 字典序
    let all = execute(&root, "glob", &json!({ "pattern": "**/*.rs" }))
        .await
        .expect("全量 glob 成功");
    let a_index = all.find("a.rs").expect("a.rs 在场");
    let b_index = all.find("b.rs").expect("b.rs 在场");
    assert!(a_index < b_index, "路径字典序输出");
}

#[tokio::test]
async fn glob无匹配返回非错误_空模式行为锁定不炸() {
    let dir = tempdir("glob-edge");
    let root = root_of(&dir);
    write_rel(&root, "only.txt", "x");

    let none = execute(&root, "glob", &json!({ "pattern": "*.rs" }))
        .await
        .expect("无匹配非错误");
    assert!(none.contains("无匹配"), "实际: {none}");

    // 空模式：不炸（行为锁定为非错误返回）
    let empty = execute(&root, "glob", &json!({ "pattern": "" })).await;
    assert!(empty.is_ok(), "空模式行为锁定不炸: {empty:?}");

    // 非法模式（非法字符序列）：显式 Err
    assert!(
        execute(&root, "glob", &json!({ "pattern": "[" }))
            .await
            .is_err(),
        "非法 glob 模式显式失败"
    );
}

// ---------------------------------------------------------------------------
// ls 执行体
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ls列一级条目字母序且目录带斜杠后缀() {
    let dir = tempdir("ls-ok");
    let root = root_of(&dir);
    write_rel(&root, "zeta.txt", "z");
    write_rel(&root, "alpha.txt", "a");
    write_rel(&root, "dir/inner.txt", "i");

    let output = execute(&root, "ls", &json!({ "path": root }))
        .await
        .expect("列目录成功");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 3, "一级条目: {output}");
    assert_eq!(lines.first().copied(), Some("alpha.txt"));
    assert_eq!(lines.get(1).copied(), Some("dir/"), "目录条目带 / 后缀");
    assert_eq!(lines.get(2).copied(), Some("zeta.txt"));
}

#[tokio::test]
async fn ls空目录返回占位_目标非目录返回错误() {
    let dir = tempdir("ls-edge");
    let root = root_of(&dir);
    let empty_dir = root.join("empty-dir");
    std::fs::create_dir_all(&empty_dir).expect("创建空目录失败");
    let file = write_rel(&root, "plain.txt", "f");

    let empty = execute(&root, "ls", &json!({ "path": empty_dir }))
        .await
        .expect("空目录非错误");
    assert_eq!(empty, "(空目录)");

    let not_dir = execute(&root, "ls", &json!({ "path": file }))
        .await
        .expect_err("目标非目录必须 Err");
    assert!(not_dir.contains("读目录失败"), "实际: {not_dir}");
}

// ---------------------------------------------------------------------------
// write 执行体
// ---------------------------------------------------------------------------

#[tokio::test]
async fn write新建文件含缺失父目录自动创建_已有文件覆写替换() {
    let dir = tempdir("write-ok");
    let root = root_of(&dir);

    // 新文件 + 缺失父目录自动创建
    let output = execute(
        &root,
        "write",
        &json!({ "path": root.join("new/deep/file.txt"), "content": "内容 🎉" }),
    )
    .await
    .expect("写入成功");
    assert!(output.contains("已写入"), "实际: {output}");
    let written = std::fs::read_to_string(root.join("new/deep/file.txt")).expect("落盘读回");
    assert_eq!(written, "内容 🎉", "内容保真落盘");

    // 已有文件覆写替换
    execute(
        &root,
        "write",
        &json!({ "path": root.join("new/deep/file.txt"), "content": "替换后" }),
    )
    .await
    .expect("覆写成功");
    let overwritten = std::fs::read_to_string(root.join("new/deep/file.txt")).expect("读回");
    assert_eq!(overwritten, "替换后", "全量覆盖");
}

#[tokio::test]
async fn write_root外路径沙箱链拒绝_bypass下唯一护栏不旁路() {
    let dir = tempdir("write-deny");
    let root = root_of(&dir);
    let outside = tempdir("write-deny-outside");

    // BypassPermissions 默认档下 write 的唯一护栏是沙箱链：root 外写入在
    // 进入执行体前被拦截（loop 组合顺序：input_path 提取 → check 拒绝）
    let input = json!({
        "path": outside.path().join("escape.txt"),
        "content": "越界内容",
    });
    let raw = input_path("write", &input).expect("path 字段在场");
    let denied = check(&root, raw).expect_err("root 外写入必须被沙箱链拦截");
    assert!(denied.contains("越出 workspace root"), "实际: {denied}");
    assert!(
        !outside.path().join("escape.txt").exists(),
        "拦截后文件不得落盘"
    );
}

// ---------------------------------------------------------------------------
// edit 执行体
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edit精确串替换成功含中文与emoji目标串且替换后落盘() {
    let dir = tempdir("edit-ok");
    let root = root_of(&dir);
    let path = write_rel(&root, "doc.md", "第一段 中文内容 🎉\n第二段");

    let output = execute(
        &root,
        "edit",
        &json!({
            "path": path,
            "old_string": "中文内容 🎉",
            "new_string": "替换后内容 🚀",
        }),
    )
    .await
    .expect("替换成功");
    assert!(output.contains("已替换 1 处"), "实际: {output}");
    let updated = std::fs::read_to_string(&path).expect("读回");
    assert!(updated.contains("替换后内容 🚀"), "替换后落盘: {updated}");
    assert!(!updated.contains("中文内容"), "原串移除");
}

#[tokio::test]
async fn edit目标串不存在返回错误且不落盘() {
    let dir = tempdir("edit-miss");
    let root = root_of(&dir);
    let path = write_rel(&root, "doc.md", "原文未动");

    let error = execute(
        &root,
        "edit",
        &json!({ "path": path, "old_string": "不存在的串", "new_string": "x" }),
    )
    .await
    .expect_err("未命中必须 Err");
    assert!(error.contains("未命中"), "实际: {error}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("读回"),
        "原文未动",
        "失败不落盘"
    );
}

#[tokio::test]
async fn edit旧串多处出现默认显式失败_replace_all全替换语义锁定() {
    let dir = tempdir("edit-multi");
    let root = root_of(&dir);
    let path = write_rel(&root, "doc.md", "重复-重复-重复");

    // 实现定稿锁定：多处出现且未传 replace_all → 显式失败（不静默首处替换）
    let multi = execute(
        &root,
        "edit",
        &json!({ "path": path, "old_string": "重复", "new_string": "新" }),
    )
    .await
    .expect_err("多处命中必须显式失败");
    assert!(multi.contains("3 处"), "报错指明命中数: {multi}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("读回"),
        "重复-重复-重复",
        "失败不落盘"
    );

    // replace_all=true → 全部替换
    let all = execute(
        &root,
        "edit",
        &json!({ "path": path, "old_string": "重复", "new_string": "新", "replace_all": true }),
    )
    .await
    .expect("全替换成功");
    assert!(all.contains("已替换 3 处"), "实际: {all}");
    assert_eq!(std::fs::read_to_string(&path).expect("读回"), "新-新-新");

    // 唯一命中 + replace_all 缺省：首处（唯一处）替换
    let single = execute(
        &root,
        "edit",
        &json!({ "path": path, "old_string": "新-新-新", "new_string": "终" }),
    )
    .await
    .expect("唯一命中替换成功");
    assert!(single.contains("已替换 1 处"));
}

// ---------------------------------------------------------------------------
// 未知工具（执行体分发兜底）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 未知工具名执行分发显式拒绝() {
    let dir = tempdir("unknown-tool");
    let root = root_of(&dir);
    let error = execute(&root, "bash", &json!({}))
        .await
        .expect_err("清单外工具显式拒绝");
    assert!(error.contains("未知工具"), "实际: {error}");
}
