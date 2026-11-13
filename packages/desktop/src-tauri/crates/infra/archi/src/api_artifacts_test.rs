//! `api_artifacts`（生成物写盘单点）的单元测试：首轮全 `written` / 次轮全 `unchanged`
//! 且不重写、内容变化只重写变化份、父目录自动创建、越根即 `Err`（该份零写盘）、单份失败
//! 不回滚、从不删文件、两列串与 `archi::display` 同源、空列表、`root` 非法与超大列表、
//! 写盘实现单点（源码自证）。
//!
//! 零 mock：真实 `tempfile::TempDir`（本 crate dev-dep 已声明）+ 真实 `fs` 读写（写盘语义
//! 即被测对象）；失败面以「父路径被普通文件占位」注入，不改权限、不依赖平台。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use archi::dto::ArchiApiWriteOutcome;
use archi::GeneratedFile;
use tempfile::TempDir;

use crate::api_artifacts::write_artifacts;

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

/// 一份应生成物（落点 = `root` 之下的绝对路径）。
fn file(root: &Path, relative: &str, content: &str) -> GeneratedFile {
    GeneratedFile {
        path: root.join(relative),
        content: content.to_owned(),
    }
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path)
        .expect("读元数据失败")
        .modified()
        .expect("取 mtime 失败")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("读文件失败")
}

/// 全量字节 + mtime 快照（零写盘 / 零删除对拍）。
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>, Option<SystemTime>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>, Option<SystemTime>)>) {
        let Ok(reader) = fs::read_dir(dir) else {
            return;
        };
        for entry in reader.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
                continue;
            }
            let relative = path
                .strip_prefix(base)
                .expect("相对路径")
                .to_string_lossy()
                .replace('\\', "/");
            let stamp = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
            out.push((relative, fs::read(&path).expect("读文件失败"), stamp));
        }
    }
    let mut entries = Vec::new();
    walk(root, root, &mut entries);
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn new_temp() -> TempDir {
    TempDir::new().expect("建临时目录失败")
}

/// 三份跨层语料（含需要新建的中间目录）。
fn three_files(root: &Path) -> Vec<GeneratedFile> {
    vec![
        file(root, "crates/core/archi.i.rs", "// 生成物 A\n"),
        file(
            root,
            ".dev-team/architecture/api/Core/App.md.schema.json",
            "{}\n",
        ),
        file(root, "src/types/generated/bindings.i.ts", "export {};\n"),
    ]
}

// ---------------------------------------------------------------------------
// 正向：written / unchanged / 只重写变化份 / 建目录
// ---------------------------------------------------------------------------

/// 正向（AC-2 / AC-5 / spec「生成命令契约」）：空盘 + 三份应生成物（含跨层新目录）→ `Ok`；
/// `written.len() == 3` 且逐串 repo 相对；`unchanged` 空；三份内容逐字节相同。
#[test]
fn write_artifacts首轮全written() {
    let temp = new_temp();
    let files = three_files(temp.path());

    let outcome = write_artifacts(temp.path(), &files).expect("写盘应成功");
    let ArchiApiWriteOutcome { written, unchanged } = outcome;
    assert_eq!(written.len(), 3, "三份全部写入：{written:?}");
    assert!(unchanged.is_empty(), "首轮零 unchanged：{unchanged:?}");
    for relative in &written {
        assert!(
            !relative.starts_with('/'),
            "repo 相对串无前导 /：{relative}"
        );
        assert!(
            !relative.contains('\\'),
            "repo 相对串分隔符恒 /：{relative}"
        );
    }
    for item in &files {
        assert_eq!(read(&item.path), item.content, "内容逐字节相同");
    }
}

/// 正向（AC-2 / D10 / spec「幂等且不动 mtime」）：同一批再调 → `unchanged.len() == 3` /
/// `written` 空；三份 mtime **不变**（字节相同即不重写）；内容零变化。
#[test]
fn write_artifacts次轮全unchanged且不重写() {
    let temp = new_temp();
    let files = three_files(temp.path());
    write_artifacts(temp.path(), &files).expect("首轮写盘应成功");
    let stamps: Vec<SystemTime> = files.iter().map(|item| mtime(&item.path)).collect();

    let outcome = write_artifacts(temp.path(), &files).expect("次轮应成功");
    assert!(
        outcome.written.is_empty(),
        "次轮零写入：{:?}",
        outcome.written
    );
    assert_eq!(outcome.unchanged.len(), 3, "三份全部计入 unchanged");
    for (index, item) in files.iter().enumerate() {
        assert_eq!(mtime(&item.path), stamps[index], "未重写 ⇒ mtime 逐字不变");
        assert_eq!(read(&item.path), item.content, "内容零变化");
    }
}

/// 正向（AC-2 / spec「只增改」）：三份中的一份入参内容改一字 → `written` 恰该一份、
/// `unchanged` 含另两份；盘上该份内容更新、另两份 mtime 不动。
#[test]
fn write_artifacts内容变化只重写变化份() {
    let temp = new_temp();
    let files = three_files(temp.path());
    write_artifacts(temp.path(), &files).expect("首轮写盘应成功");

    let changed_path = files[1].path.clone();
    let mut changed = three_files(temp.path());
    changed[1].content = "{}\n// 改一字\n".to_owned();
    let before = mtime(&changed_path);
    let untouched: Vec<SystemTime> = [0usize, 2usize]
        .iter()
        .map(|index| mtime(&files[*index].path))
        .collect();

    let outcome = write_artifacts(temp.path(), &changed).expect("次轮应成功");
    assert_eq!(
        outcome.written,
        vec![archi::display(temp.path(), &changed_path)],
        "written 恰含变化的一份"
    );
    assert_eq!(outcome.unchanged.len(), 2, "另两份计入 unchanged");
    assert_eq!(read(&changed_path), changed[1].content, "盘上内容已更新");
    assert!(
        mtime(&changed_path) >= before,
        "被重写的一份 mtime 不倒退（真实重写）"
    );
    for (slot, index) in [0usize, 2usize].iter().enumerate() {
        assert_eq!(
            mtime(&files[*index].path),
            untouched[slot],
            "未变化的两份 mtime 不动"
        );
    }
}

/// 正向（AC-2 / D10）：目标 `<root>/a/b/c/x.schema.json`（三层目录全不存在）→ `Ok`、
/// 文件在盘、内容正确、`written` 含该串。
#[test]
fn write_artifacts父目录自动创建() {
    let temp = new_temp();
    let deep = file(temp.path(), "a/b/c/x.schema.json", "{\"$defs\": {}}\n");
    let outcome = write_artifacts(temp.path(), std::slice::from_ref(&deep)).expect("写盘应成功");

    assert_eq!(
        outcome.written,
        vec!["a/b/c/x.schema.json".to_owned()],
        "三层新目录的落点在 written 内"
    );
    assert!(deep.path.is_file(), "文件真实在盘");
    assert_eq!(read(&deep.path), deep.content, "内容正确");
    for dir in ["a", "a/b", "a/b/c"] {
        assert!(temp.path().join(dir).is_dir(), "中间目录逐层建出：{dir}");
    }
}

// ---------------------------------------------------------------------------
// 异常：越根 / 单份失败不回滚 / 从不删
// ---------------------------------------------------------------------------

/// 异常（AC-5 / D10 / spec「越根复核」）：入参含 `root` 之外的绝对目标 → `Err`，文案含
/// 「越出观察根」与两侧路径；该目标未创建、父目录未被建。两种次序的实际行为在用例内钉死
/// （越根在前 ⇒ 整批未写；越根在后 ⇒ 前序已写）——不声称全或无。
#[test]
fn write_artifacts越根目标即err() {
    let outside_root = new_temp();
    let outside = outside_root.path().join("evil.i.ts");

    // 次序 A：越根份在首位 ⇒ 整批未写
    let temp_a = new_temp();
    let mut leading = vec![GeneratedFile {
        path: outside.clone(),
        content: "// 越根\n".to_owned(),
    }];
    leading.extend(three_files(temp_a.path()));
    let error = write_artifacts(temp_a.path(), &leading).expect_err("越根 ⇒ Err");
    assert!(error.contains("越出观察根"), "文案点名越根：{error}");
    assert!(
        error.contains(&outside.display().to_string()),
        "文案含越根一侧路径：{error}"
    );
    assert!(
        error.contains(&temp_a.path().display().to_string()),
        "文案含观察根一侧路径：{error}"
    );
    assert!(!outside.exists(), "越根目标未创建");
    assert!(
        snapshot(temp_a.path()).is_empty(),
        "越根在首位 ⇒ 整批未写：{:?}",
        snapshot(temp_a.path())
    );

    // 次序 B：越根份在末位 ⇒ 前序三份已写（非原子、不回滚）
    let temp_b = new_temp();
    let mut trailing = three_files(temp_b.path());
    trailing.push(GeneratedFile {
        path: outside.clone(),
        content: "// 越根\n".to_owned(),
    });
    let error = write_artifacts(temp_b.path(), &trailing).expect_err("越根 ⇒ Err");
    assert!(error.contains("越出观察根"), "文案点名越根：{error}");
    assert_eq!(snapshot(temp_b.path()).len(), 3, "前序三份保留在盘");
    assert!(!outside.exists(), "越根目标仍未创建");
    assert!(
        !outside.parent().expect("父路径").join("evil.i.ts").exists(),
        "越根目标的父目录未被该份创建"
    );
}

/// 异常（AC-5 / D10 / spec「非原子如实」）：目标父路径被**普通文件**占位 → `Err`
/// （`create_dir_all` 失败），文案不含「全或无」字样；前序已写的份**保留在盘**。
#[test]
fn write_artifacts单份失败不回滚() {
    let temp = new_temp();
    // `<root>/a` 是普通文件 ⇒ `<root>/a/b/x.i.rs` 的建目录必失败
    fs::write(temp.path().join("a"), "占位文件\n").expect("写占位文件失败");

    let files = vec![
        file(temp.path(), "ok/first.i.rs", "// 先写\n"),
        file(temp.path(), "a/b/x.i.rs", "// 建目录失败\n"),
    ];
    let error = write_artifacts(temp.path(), &files).expect_err("建目录失败 ⇒ Err");
    assert!(
        error.contains("目录创建失败"),
        "文案点名失败原因（逐份 IO 错误）：{error}"
    );
    assert!(!error.contains("全或无"), "MUST NOT 声称全或无：{error}");
    assert!(
        temp.path().join("ok/first.i.rs").is_file(),
        "前序已写的一份保留在盘（不回滚）"
    );
    assert!(!temp.path().join("a/b").exists(), "失败份的目录未创建");
}

/// 异常（AC-2 / AC-5 / 硬线二）：盘上预置孤儿（不在 `files` 内）→ 调用后字节与 mtime 均
/// 不变、仍在盘；`api_artifacts.rs` 零 `remove_file` / `remove_dir` / `remove_dir_all`。
#[test]
fn write_artifacts从不删文件() {
    let temp = new_temp();
    let orphan = temp.path().join("crates/legacy/old.i.rs");
    fs::create_dir_all(orphan.parent().expect("父路径")).expect("建目录失败");
    fs::write(&orphan, "// 历史孤儿\n").expect("写孤儿失败");
    let before_bytes = read(&orphan);
    let before_mtime = mtime(&orphan);

    let files = three_files(temp.path());
    let outcome = write_artifacts(temp.path(), &files).expect("写盘应成功");
    assert_eq!(outcome.written.len(), 3, "孤儿不入两列");

    assert!(orphan.is_file(), "孤儿仍在盘");
    assert_eq!(read(&orphan), before_bytes, "孤儿字节不变");
    assert_eq!(mtime(&orphan), before_mtime, "孤儿 mtime 不变");

    let source = include_str!("api_artifacts.rs");
    for forbidden in ["remove_file", "remove_dir", "remove_dir_all"] {
        assert!(
            !source.contains(forbidden),
            "`api_artifacts.rs` 零 `{forbidden}`（零删除分支）"
        );
    }
}

// ---------------------------------------------------------------------------
// 正向：两列串与 archi::display 同源
// ---------------------------------------------------------------------------

/// 正向（AC-3 / AC-5）：两列串均无 `\`、无前导 `./`、无前导 `/`、不含 `root` 前缀；含中文 /
/// 空格 / emoji 的目录名同样归一；与 `archi::display(root, &path)` 逐字相等。
///
/// `root` 取绝对路径与 `<root>/.`（非归一的等价形态）两种——**不改进程 cwd**：并行测试下
/// `set_current_dir` 会让同进程其它用例的相对根失真，非归一形态已足以验证 `strip_prefix`
/// 单点（`display` 是唯一归一落点）。
#[test]
fn write_artifacts两列路径串repo相对posix() {
    let exotic = [
        "src/中文目录/a.i.ts",
        "src/with space/b.i.rs",
        "src/emoji🎯/c.i.ts",
        "src/plain/d.i.rs",
    ];

    for tag in ["绝对根", "root/. 形态"] {
        let temp = new_temp();
        let root: PathBuf = if tag == "绝对根" {
            temp.path().to_path_buf()
        } else {
            temp.path().join(".")
        };
        let files: Vec<GeneratedFile> = exotic
            .iter()
            .map(|relative| file(&root, relative, "// 生成物\n"))
            .collect();

        let outcome = write_artifacts(&root, &files).expect("写盘应成功");
        assert_eq!(outcome.written.len(), exotic.len(), "{tag}：全部写入");
        for (index, relative) in exotic.iter().enumerate() {
            let entry = &outcome.written[index];
            assert_eq!(
                entry,
                &archi::display(temp.path(), &temp.path().join(relative)),
                "{tag}：串逐字等于 archi::display"
            );
            assert_eq!(entry, relative, "{tag}：repo 相对形态逐字保留");
            assert!(!entry.starts_with("./"), "{tag}：无前导 ./：{entry}");
            assert!(!entry.starts_with('/'), "{tag}：无前导 /：{entry}");
            assert!(!entry.contains('\\'), "{tag}：分隔符恒 /：{entry}");
            assert!(
                !entry.contains(&temp.path().display().to_string()),
                "{tag}：不含 root 前缀：{entry}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 边界：空列表 / root 非法 / 超大列表
// ---------------------------------------------------------------------------

/// 边界（边界 / list 映射）：`files = []` → `Ok` 且两列皆空；零目录创建、零写盘。
#[test]
fn write_artifacts空列表() {
    let temp = new_temp();
    let before = snapshot(temp.path());
    let outcome = write_artifacts(temp.path(), &[]).expect("空列表应 Ok");
    assert!(outcome.written.is_empty(), "written 空");
    assert!(outcome.unchanged.is_empty(), "unchanged 空");
    assert_eq!(before, snapshot(temp.path()), "零目录创建、零写盘");
    assert!(fs::read_dir(temp.path()).expect("读目录").next().is_none());
}

/// 边界（边界）：`root` 为空 `Path` / 不存在目录 / 指向文件 → 落在 `Result` 内不 panic
/// （空 `Path` 下 `starts_with` 恒真 ⇒ 写入照常发生，用例内钉死该实际行为）；1000 份应生成物
/// → `written.len() == 1000`、不抛错、两列不交叠。
#[test]
fn write_artifacts_root非法与超大列表() {
    // 空 Path：`starts_with("")` 恒真，逐份照常写（无越根拒绝）
    let empty_root = new_temp();
    let target = file(empty_root.path(), "src/a.i.rs", "// 空根\n");
    let outcome = write_artifacts(Path::new(""), std::slice::from_ref(&target))
        .expect("空 Path 下 starts_with 恒真 ⇒ Ok");
    assert_eq!(outcome.written.len(), 1, "空 Path 形态：该份照常写入");
    assert_eq!(outcome.unchanged.len(), 0, "首轮无 unchanged");
    assert!(target.path.is_file(), "文件真实在盘");

    // 不存在目录：目标相对该目录 ⇒ 建目录后写入
    let missing_root = std::env::temp_dir().join("archi-write-no-such-root");
    let _ = fs::remove_dir_all(&missing_root);
    let nested = file(&missing_root, "a/b.i.ts", "// 新根\n");
    let outcome = write_artifacts(&missing_root, std::slice::from_ref(&nested))
        .expect("不存在的 root 照常建目录写入");
    assert_eq!(outcome.written.len(), 1, "不存在 root：建目录后写入");
    assert!(nested.path.is_file(), "文件真实在盘");
    let _ = fs::remove_dir_all(&missing_root);

    // 指向文件的 root：目标 `<file>/...` 的建目录必失败 ⇒ Err
    let file_root = new_temp();
    fs::write(file_root.path().join("plain.txt"), "not a dir\n").expect("写占位文件失败");
    let blocked = GeneratedFile {
        path: file_root.path().join("plain.txt").join("x.i.rs"),
        content: "// 建目录失败\n".to_owned(),
    };
    let error = write_artifacts(file_root.path().join("plain.txt").as_path(), &[blocked])
        .expect_err("root 为文件 ⇒ 建目录失败");
    assert!(error.contains("目录创建失败"), "文案点名失败原因：{error}");

    // 超大列表：1000 份全部写入、两列不交叠
    let large = new_temp();
    let files: Vec<GeneratedFile> = (0..1000)
        .map(|index| file(large.path(), &format!("gen/x{index:04}.i.rs"), "// 批次\n"))
        .collect();
    let outcome = write_artifacts(large.path(), &files).expect("大列表应 Ok");
    assert_eq!(outcome.written.len(), 1000, "1000 份全部写入");
    assert!(outcome.unchanged.is_empty(), "首轮零 unchanged");
    let mut union: Vec<String> = outcome
        .written
        .iter()
        .chain(outcome.unchanged.iter())
        .cloned()
        .collect();
    let total = union.len();
    union.sort();
    union.dedup();
    assert_eq!(union.len(), total, "两列不交叠");
}

// ---------------------------------------------------------------------------
// 纪律：写盘实现单点（源码自证）
// ---------------------------------------------------------------------------

/// 异常（AC-5 / 纪律判据）：`include_str!("api_artifacts.rs")` 自证 `fs::write` 恰 1、
/// `create_dir_all` 恰 1、`starts_with(root)` 恰 1、`remove_*` 零命中、`pub fn` 计数恰 1、
/// 零 `WalkBuilder` / 零 `std::process` / 零 `tokio::process`（dumb IO，零判定零渲染）。
#[test]
fn write_artifacts写盘实现单点() {
    let source = include_str!("api_artifacts.rs");
    assert_eq!(
        source.matches("std::fs::write").count(),
        1,
        "fs::write 恰一处"
    );
    assert_eq!(
        source.matches("create_dir_all").count(),
        1,
        "create_dir_all 恰一处"
    );
    assert_eq!(
        source.matches("starts_with(root)").count(),
        1,
        "越根复核恰一处"
    );
    for forbidden in ["remove_", "WalkBuilder", "std::process", "tokio::process"] {
        assert!(
            !source.contains(forbidden),
            "`api_artifacts.rs` 零 `{forbidden}`（dumb IO、零删除、零 spawn）"
        );
    }
    assert_eq!(
        source.matches("pub fn ").count(),
        1,
        "公共面恰一个函数（write_artifacts）"
    );
}
