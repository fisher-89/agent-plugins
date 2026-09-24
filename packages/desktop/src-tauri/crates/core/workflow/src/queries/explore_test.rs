//! `queries::explore` 的单元测试：`read_explore` 单文件读（单分量校验 +
//! UTF-8 归一语义，AC-1）与 `scan_explores` 导入扫描（顶层 `*.md` 收集 +
//! 排序 + 空目录纪律，AC-2 纯扫描半）。真实文件系统装置（hand-rolled
//! TempWs，沿 list_test.rs 惯例，本 crate 零新增 dev-dependency）；全程
//! 不 mock fs——不测 std::fs / 目录遍历库自身语义，只测本模块的校验、
//! 收集与排序组装。纯读模块：全部用例不得产生写入副作用。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::{read_explore, scan_explores};
use foundation::layout::resolve;

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-explore-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 笔记目录（`<root>/openspec/explores`），按需创建。
    fn explores_dir(&self) -> PathBuf {
        let dir = resolve(&self.0).explores_root;
        fs::create_dir_all(&dir).expect("创建 explores 目录失败");
        dir
    }

    /// 写一篇笔记（`explores_root/<name>.md`）。
    fn note(&self, name: &str, content: &str) {
        fs::write(self.explores_dir().join(format!("{name}.md")), content).expect("写笔记文件失败");
    }

    fn layout(&self) -> foundation::layout::Layout {
        resolve(&self.0)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 文件修改时间（UTC unix 毫秒）：与被测模块 `modified_millis` 同口径独立推导。
fn modified_millis_of(path: &Path) -> i64 {
    fs::metadata(path)
        .expect("读文件 metadata 失败")
        .modified()
        .expect("读修改时间失败")
        .duration_since(UNIX_EPOCH)
        .expect("修改时间早于 epoch")
        .as_millis() as i64
}

// ---------------------------------------------------------------------------
// read_explore：单分量校验 + 文本读出（AC-1）
// ---------------------------------------------------------------------------

#[test]
fn 含foo_md的workspace读取返回名称与全文() {
    let ws = TempWs::new("read-hit");
    ws.note("foo", "# 探索笔记\n第一行\n- 线索 A");

    let doc = read_explore(&ws.layout(), "foo").expect("foo.md 应可读");

    assert_eq!(doc.name, "foo", "name 即请求的 stem");
    assert_eq!(
        doc.content, "# 探索笔记\n第一行\n- 线索 A",
        "content 为文件全文"
    );
}

#[test]
fn 未知stem返回none而非错误() {
    let ws = TempWs::new("read-miss");
    ws.note("foo", "有内容");

    assert_eq!(read_explore(&ws.layout(), "bar"), None, "缺失归一为 None");
}

#[test]
fn 含分隔符或穿越分量的名一律拒绝返回none() {
    let ws = TempWs::new("read-traversal");
    ws.note("x", "可读内容");

    for name in ["a/b", "../x", "..", "..\\x", "a\\b", "a:b", "C:\\evil"] {
        assert_eq!(
            read_explore(&ws.layout(), name),
            None,
            "敌意名 {name:?} 应被单分量校验拒绝"
        );
    }
    // 拒绝即不触盘：穿越分量指向的子路径不得被读取副作用创建（纯读零写入）
    assert!(
        !ws.explores_dir().join("a").exists(),
        "拒绝路径不得在笔记目录下产生任何新条目"
    );
}

#[test]
fn 空串名返回none() {
    let ws = TempWs::new("read-empty");
    ws.note("foo", "可读内容");

    assert_eq!(read_explore(&ws.layout(), ""), None, "空分量被校验拒绝");
}

#[test]
fn 超长名返回none不panic() {
    let ws = TempWs::new("read-long");
    ws.note("foo", "可读内容");
    let long = format!("a{}", "长".repeat(1000));

    assert_eq!(
        read_explore(&ws.layout(), &long),
        None,
        "超长名直接拒绝，不触盘不 panic"
    );
}

#[test]
fn 非utf8内容读失败归一为none() {
    let ws = TempWs::new("read-binary");
    let dir = ws.explores_dir();
    fs::write(dir.join("binary.md"), [0xFF, 0xFE, 0x00, 0xD8]).expect("写字节失败");

    assert_eq!(
        read_explore(&ws.layout(), "binary"),
        None,
        "非 UTF-8 读失败归一为缺失语义，不报错不 panic"
    );
}

#[test]
fn 同名md位置实为目录时返回none() {
    let ws = TempWs::new("read-dir");
    fs::create_dir_all(ws.explores_dir().join("dirmode.md")).expect("建同名目录失败");

    assert_eq!(
        read_explore(&ws.layout(), "dirmode"),
        None,
        "fs 读目录失败不逃逸，归一为 None"
    );
}

// ---------------------------------------------------------------------------
// scan_explores：顶层 *.md 收集与排序（AC-2 纯扫描半）
// ---------------------------------------------------------------------------

#[test]
fn 三个md返回stem升序且modified_at为文件修改时间() {
    let ws = TempWs::new("scan-hit");
    // 刻意乱序创建：顺序由 stem 排序决定，与创建顺序无关
    ws.note("c", "# c");
    ws.note("a", "# a");
    ws.note("b", "# b");

    let scanned = scan_explores(&ws.layout());

    let names: Vec<&str> = scanned.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b", "c"], "stem 升序，与创建顺序无关");
    for entry in &scanned {
        let expected = modified_millis_of(&ws.explores_dir().join(format!("{}.md", entry.name)));
        assert_eq!(
            entry.modified_at,
            Some(expected),
            "modifiedAt 为该文件修改时间的 UTC unix 毫秒值"
        );
    }
}

#[test]
fn 目录缺失返回空vec不报错() {
    // 新 workspace：未创建 explores 目录（resolve 纯推导不落盘）
    let ws = TempWs::new("scan-missing");

    let scanned = scan_explores(&ws.layout());

    assert!(scanned.is_empty(), "目录缺失返回空结果而非报错");
}

#[test]
fn 目录存在但为空返回空vec() {
    let ws = TempWs::new("scan-empty");
    ws.explores_dir();

    assert!(scan_explores(&ws.layout()).is_empty(), "空目录返回空 Vec");
}

#[test]
fn 只列顶层md不递归不收其他扩展名() {
    let ws = TempWs::new("scan-filter");
    ws.note("keep", "# 顶层笔记");
    fs::write(ws.explores_dir().join("notes.txt"), "非 markdown").expect("写 txt 失败");
    // 子目录内含 .md：不递归
    let sub = ws.explores_dir().join("sub");
    fs::create_dir_all(&sub).expect("建子目录失败");
    fs::write(sub.join("inner.md"), "子目录内笔记").expect("写子目录文件失败");

    let scanned = scan_explores(&ws.layout());

    let names: Vec<&str> = scanned.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["keep"],
        "只列顶层 .md，不递归、不收录其他扩展名"
    );
}
