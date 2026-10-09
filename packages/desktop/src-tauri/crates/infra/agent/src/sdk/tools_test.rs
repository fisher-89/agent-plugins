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
// 七工具定义面（正向 + 封闭性边界）
// ---------------------------------------------------------------------------

#[test]
fn 定义清单恰为七工具且名称_描述_schema齐全() {
    let defs = definitions();
    assert_eq!(defs.len(), 7, "恰为七工具（bash 入册后的封闭清单）");
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
    // 关键入参形状抽查：glob 无 path 字段（pattern 驱动），其余六工具 path 必填
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
    // bash 条目形状：command 必填、timeout_ms 可选（缺省与钳位语义见描述）
    let bash = defs
        .iter()
        .find(|def| def.name == "bash")
        .expect("bash 定义在场");
    let bash_required = bash.parameters["required"]
        .as_array()
        .expect("bash required 数组");
    assert!(
        bash_required.iter().any(|value| value == "command"),
        "bash 的 command 必填"
    );
    assert!(
        bash_required.iter().all(|value| value != "timeout_ms"),
        "bash 的 timeout_ms 可选（不入 required）"
    );
    assert!(
        bash.parameters["properties"]["timeout_ms"].is_object(),
        "bash 声明 timeout_ms 属性"
    );
    assert!(
        bash.parameters["properties"]["command"]["type"] == json!("string"),
        "bash 的 command 为字符串"
    );

    // read schema：offset / limit 在场且描述承载截断语义
    let read = defs
        .iter()
        .find(|def| def.name == "read")
        .expect("read 定义");
    for field in ["offset", "limit"] {
        assert!(
            read.parameters["properties"][field].is_object(),
            "read 声明 {field} 属性"
        );
    }
    let read_desc = format!(
        "{} {}",
        read.description, read.parameters["properties"]["limit"]["description"]
    );
    assert!(
        read_desc.contains("2000") && read_desc.contains("截断") && read_desc.contains("offset"),
        "read 定义含 offset/limit 描述与截断语义（翻页口径）: {read_desc}"
    );

    // grep schema：context 参数在场、pattern 描述改正则口径
    let grep = defs
        .iter()
        .find(|def| def.name == "grep")
        .expect("grep 定义");
    assert!(
        grep.parameters["properties"]["context"].is_object(),
        "grep 声明 context 属性"
    );
    let grep_pattern_desc = grep.parameters["properties"]["pattern"]["description"]
        .as_str()
        .expect("pattern 描述");
    assert!(
        grep_pattern_desc.contains("正则"),
        "pattern 描述改正则口径（regex 语义）: {grep_pattern_desc}"
    );
    assert!(
        !TOOL_NAMES.contains(&"Bash"),
        "封闭清单精确匹配（大小写变体不在册）"
    );
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
// read：2000 行截断 + 翻页（AC-3 工具质量半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn read超2000行截断尾部留痕且offset翻页取回后续窗口() {
    let dir = tempdir("read-paginate");
    let root = root_of(&dir);
    // 短行 fixture：2000 行输出体积须低于 L1 单结果 30KB 上限（行截断半边
    // 独立可考，不被字节上限先行截断）
    let body: String = (1..=2001).map(|line| format!("L{line}\n")).collect();
    let path = write_rel(&root, "big.txt", &body);

    let output = execute(&root, "read", &json!({ "path": path }))
        .await
        .expect("读取成功");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(
        lines.len(),
        2001,
        "前 2000 行 + 1 行尾部留痕: {}",
        lines.len()
    );
    assert!(output.contains("2000\tL2000"), "第 2000 行在场: {output}");
    assert!(!output.contains("L2001"), "第 2001 行不在本段窗口");
    let tail = lines.last().expect("尾部留痕");
    assert!(
        tail.contains("已截断") && tail.contains("offset=2001"),
        "尾部留痕指明翻页 offset: {tail}"
    );

    // offset 翻页：取回第 2001 行窗口
    let paged = execute(&root, "read", &json!({ "path": path, "offset": 2001 }))
        .await
        .expect("翻页读取成功");
    assert!(
        paged.contains("2001\tL2001"),
        "offset 翻页取回后续窗口: {paged}"
    );
    assert!(!paged.contains("L2000"), "翻页窗口不含前段行: {paged}");
}

#[tokio::test]
async fn read恰2000行不截断无留痕() {
    let dir = tempdir("read-exact");
    let root = root_of(&dir);
    let body: String = (1..=2000).map(|line| format!("L{line}\n")).collect();
    let path = write_rel(&root, "exact.txt", &body);

    let output = execute(&root, "read", &json!({ "path": path }))
        .await
        .expect("读取成功");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2000, "恰阈值全量返回");
    assert!(
        output.contains("2000\tL2000"),
        "末行在场（恰阈值过）: {output}"
    );
    assert!(!output.contains("已截断"), "恰 2000 行无截断留痕");
}

#[tokio::test]
async fn readlimit缺省与显式2000行为一致() {
    let dir = tempdir("read-limit");
    let root = root_of(&dir);
    let body: String = (1..=2500).map(|line| format!("L{line}\n")).collect();
    let path = write_rel(&root, "long.txt", &body);

    let default_read = execute(&root, "read", &json!({ "path": path }))
        .await
        .expect("缺省读取成功");
    let explicit_read = execute(&root, "read", &json!({ "path": path, "limit": 2000 }))
        .await
        .expect("显式 limit 读取成功");

    assert_eq!(
        default_read, explicit_read,
        "缺省与显式 limit=2000 行为一致（缺省即上限口径）"
    );
    assert!(default_read.contains("已截断"), "超限文件两形态均留痕");
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
async fn grep无命中与空文件返回非错误占位() {
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

    // 空 pattern：显式拒绝（非静默全行匹配）
    assert!(
        execute(&root, "grep", &json!({ "path": path, "pattern": "" }))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn grep正则语义匹配替换子串字面_中文与emoji行内容保真() {
    let dir = tempdir("grep-regex");
    let root = root_of(&dir);
    let path = write_rel(
        &root,
        "code.rs",
        "fn main() {}\nfn helper() {}\n// 无关行\n包含 正则元字符 (.) 与 emoji 🎉 的行",
    );

    // 正则通配：`fn.m` 以 `.` 通配命中（字面子串 "fn.m" 不在文中——翻转旧
    // 「元字符按子串字面语义」断言，元字符按正则解释）
    let wildcard = execute(&root, "grep", &json!({ "path": path, "pattern": "fn.m" }))
        .await
        .expect("正则命中");
    assert!(wildcard.contains("1: fn main() {}"), "{wildcard}");
    assert!(
        !wildcard.contains("fn helper"),
        "通配窗仅覆盖通配符位（helper 行不含 fn.m 形态）: {wildcard}"
    );

    // 元字符 pattern 按正则解释命中（`.` 通配任意单字符；字面 "正则(.)字"
    // 不在文中——翻转旧「元字符按子串字面语义」断言）
    let metachar = execute(
        &root,
        "grep",
        &json!({ "path": path, "pattern": "正则(.)字" }),
    )
    .await
    .expect("正则语义命中");
    assert!(metachar.contains("4: "), "元字符行命中: {metachar}");

    // 中文与 emoji 行内容保真不变
    let emoji = execute(&root, "grep", &json!({ "path": path, "pattern": "🎉" }))
        .await
        .expect("emoji 命中");
    assert!(
        emoji.contains("包含 正则元字符 (.) 与 emoji 🎉 的行"),
        "命中行内容保真: {emoji}"
    );

    // 非法正则：显式 Err（不静默空匹配）
    let invalid = execute(&root, "grep", &json!({ "path": path, "pattern": "[" }))
        .await
        .expect_err("非法正则必须 Err");
    assert!(invalid.contains("非法正则"), "记因指明非法正则: {invalid}");
}

#[tokio::test]
async fn grep_context上下文行窗口合并去重且组间以分隔符隔离() {
    let dir = tempdir("grep-context");
    let root = root_of(&dir);
    // 命中位：第 2、3 行（相邻窗口合并）与第 7 行（独立组）
    let path = write_rel(&root, "lines.txt", "L1\nM1\nM2\nL4\nL5\nL6\nM3\n");

    // context=1：命中行 ± 1；相邻命中窗口合并去重（每行恰一次）；不连续组间 `--`
    let with_context = execute(
        &root,
        "grep",
        &json!({ "path": path, "pattern": "M\\d", "context": 1 }),
    )
    .await
    .expect("context 命中成功");
    let shown = path.to_string_lossy().into_owned();
    let expected: Vec<String> = vec![
        format!("{shown}:1: L1"),
        format!("{shown}:2: M1"),
        format!("{shown}:3: M2"),
        format!("{shown}:4: L4"),
        "--".to_owned(),
        format!("{shown}:6: L6"),
        format!("{shown}:7: M3"),
    ];
    assert_eq!(
        with_context.lines().collect::<Vec<_>>(),
        expected,
        "窗口合并去重 + 组间 -- 分隔: {with_context}"
    );
    assert_eq!(with_context.matches("--").count(), 1, "恰一组分隔（两组）");

    // context 缺省 0：维持逐行口径（无上下文行、无分隔符）
    let bare = execute(&root, "grep", &json!({ "path": path, "pattern": "M\\d" }))
        .await
        .expect("缺省 context 命中成功");
    let bare_expected: Vec<String> = vec![
        format!("{shown}:2: M1"),
        format!("{shown}:3: M2"),
        format!("{shown}:7: M3"),
    ];
    assert_eq!(
        bare.lines().collect::<Vec<_>>(),
        bare_expected,
        "context 缺省 0 维持逐行口径: {bare}"
    );
}

#[tokio::test]
async fn grep单文件context为0时窗口退化为裸命中行() {
    let dir = tempdir("grep-context-zero");
    let root = root_of(&dir);
    let path = write_rel(&root, "tiny.txt", "甲\n乙\n丙\n");
    let output = execute(
        &root,
        "grep",
        &json!({ "path": path, "pattern": "乙", "context": 0 }),
    )
    .await
    .expect("context=0 显式传参成功");
    let shown = path.to_string_lossy().into_owned();
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        vec![format!("{shown}:2: 乙")],
        "context=0 与缺省同口径"
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
async fn glob尾段裸双星归一化_直属与深层文件命中() {
    let dir = tempdir("glob-recursive");
    let root = root_of(&dir);
    write_rel(&root, "changes/x/explore.md", "goal");
    write_rel(&root, "changes/x/specs/a.md", "a");

    // 回归锚（archive-merge-first proposal 探测假阴性现场）：glob crate 尾段
    // 裸 `**` 原生只命中后代目录，直属文件被遍历层丢弃；归一化后两者入场
    let direct = execute(&root, "glob", &json!({ "pattern": "changes/x/**" }))
        .await
        .expect("尾段裸 ** glob 成功");
    assert!(
        direct.contains("explore.md") && direct.contains("specs/a.md"),
        "直属与深层文件均命中: {direct}"
    );

    // 裸 **（root 全域）同口径
    let bare = execute(&root, "glob", &json!({ "pattern": "**" }))
        .await
        .expect("裸 ** glob 成功");
    assert!(bare.contains("explore.md"), "裸 ** 含直属文件: {bare}");

    // 无匹配回显原始 pattern（非归一化形）
    let none = execute(&root, "glob", &json!({ "pattern": "no-such/**" }))
        .await
        .expect("无匹配非错误");
    assert!(
        none.contains("无匹配: no-such/**"),
        "回显原 pattern: {none}"
    );
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
// L1 单结果字节上限收口（AC-5：Ok / Err 双路截断留痕）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execute_l1字节上限ok路截断留痕() {
    let dir = tempdir("l1-ok");
    let root = root_of(&dir);
    // 单行超 30KB 的文件：read 输出越过单结果上限
    let path = write_rel(&root, "huge-line.txt", &"a".repeat(40_000));

    let output = execute(&root, "read", &json!({ "path": path }))
        .await
        .expect("读取成功（L1 收口不炸）");

    assert!(
        output.contains("已截断：单结果超 30000 字节上限"),
        "截断留痕: {output}"
    );
    assert!(
        output.len() < 30_000 + 200,
        "输出钳在 30KB 上限附近（含留痕）: {}",
        output.len()
    );
}

#[tokio::test]
async fn execute_l1字节上限err路截断留痕() {
    let dir = tempdir("l1-err");
    let root = root_of(&dir);
    // Err 内容超限载体：bash 臂产出 40KB stdout 后以非零码退出（错误串携带
    // 全部输出）；powershell 经 git-bash / cmd 两底座均在册可达
    let command = "powershell -NoProfile -Command \"Write-Output ('x' * 40000); exit 3\"";

    let error = execute(&root, "bash", &json!({ "command": command }))
        .await
        .expect_err("非零退出必须 Err（内容超限走同款收口）");

    assert!(
        error.contains("已截断：单结果超 30000 字节上限"),
        "Err 路同款截断留痕（Ok / Err 双路收口）: {}",
        &error[..error.len().min(400)]
    );
    assert!(
        error.len() < 30_000 + 200,
        "Err 内容钳在 30KB 上限附近: {}",
        error.len()
    );
}

// ---------------------------------------------------------------------------
// bash 臂分发接入（AC-4 / AC-10 翻转半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bash臂分发接入_缺command报必填而非未知工具() {
    let dir = tempdir("bash-dispatch");
    let root = root_of(&dir);

    let output = execute(&root, "bash", &json!({ "command": "echo bash-arm-ok" }))
        .await
        .expect("bash 臂真实执行成功");
    assert!(output.contains("bash-arm-ok"), "{output}");

    let missing = execute(&root, "bash", &json!({}))
        .await
        .expect_err("缺 command 必须 Err");
    assert!(
        missing.contains("command"),
        "记因指明 command 必填: {missing}"
    );
    assert!(
        !missing.contains("未知工具"),
        "bash 已入册：缺参错误非「未知工具」拒绝"
    );
}

// ---------------------------------------------------------------------------
// 未知工具（执行体分发兜底）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 未知工具名执行分发显式拒绝() {
    let dir = tempdir("unknown-tool");
    let root = root_of(&dir);
    // 反例改用真实清单外名（bash 已入册，不再充当清单外反例）
    let error = execute(&root, "rm_rf", &json!({}))
        .await
        .expect_err("清单外工具显式拒绝");
    assert!(error.contains("未知工具"), "实际: {error}");
    assert!(
        !TOOL_NAMES.contains(&"rm_rf"),
        "前置：反例名确在封闭清单之外"
    );
}
