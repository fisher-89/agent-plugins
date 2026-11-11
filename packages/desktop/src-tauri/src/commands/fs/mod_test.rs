//! `read_file` / `read_file_inner` 单测：字节分页与字符边界、binary 探测、
//! 目录与缺失、大小上限、stat 面。零 mock——真实临时目录落盘。

use super::{read_file, read_file_inner, FileSlice, DEFAULT_LIMIT, MAX_LIMIT};
use std::fs;
use tempfile::TempDir;

/// 落一个文件并返回其绝对路径字符串。
fn write(dir: &TempDir, name: &str, bytes: impl AsRef<[u8]>) -> String {
    let path = dir.path().join(name);
    fs::write(&path, bytes).expect("落盘应成功");
    path.to_string_lossy().into_owned()
}

fn read(path: &str, offset: Option<u64>, limit: Option<u64>) -> FileSlice {
    read_file_inner(path, offset, limit)
        .expect("读取应成功")
        .expect("应命中非空切片")
}

#[test]
fn read_file整读返回全文与stat() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "a.md", "# 标题\n正文\n");

    let slice = read(&path, None, None);

    assert_eq!(slice.content, "# 标题\n正文\n");
    assert_eq!(slice.offset, 0);
    assert_eq!(slice.next_offset, slice.stat.size);
    assert!(!slice.truncated);
    assert_eq!(slice.encoding, "utf8");
    assert!(slice.stat.is_file);
    assert!(!slice.stat.is_dir);
    assert_eq!(slice.stat.size, "# 标题\n正文\n".len() as u64);
    assert!(slice.stat.modified_ms.is_some(), "修改时间应可取");
}

#[test]
fn read_file命令面与纯函数一致() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "a.txt", "hello");

    let via_command = read_file(path.clone(), None, None).expect("Ok");
    let via_inner = read_file_inner(&path, None, None).expect("Ok");

    assert_eq!(via_command, via_inner, "薄包装零加工");
}

#[test]
fn read_file分页拼接等于全文() {
    let dir = TempDir::new().expect("tempdir");
    // 多字节字符跨窗口边界：每字符 3 字节，窗口 4 字节必切断
    let text = "甲乙丙丁戊己庚辛壬癸";
    let path = write(&dir, "zh.txt", text.as_bytes());

    let mut collected = String::new();
    let mut offset = 0u64;
    let mut rounds = 0;
    loop {
        let slice = read(&path, Some(offset), Some(4));
        assert_eq!(slice.encoding, "utf8");
        collected.push_str(&slice.content);
        rounds += 1;
        assert!(rounds <= text.len(), "分页应终止");
        if !slice.truncated {
            break;
        }
        assert!(slice.next_offset > offset, "游标必须前进");
        offset = slice.next_offset;
    }

    assert_eq!(collected, text, "连续分页拼接 == 全文（无重无漏）");
}

#[test]
fn read_file起点落在多字节中间时前向对齐() {
    let dir = TempDir::new().expect("tempdir");
    let text = "甲b";
    let path = write(&dir, "align.txt", text.as_bytes());

    // 偏移 1 / 2 都落在「甲」（3 字节）内部 → 对齐到字符 1（即 'b'）
    for offset in [1u64, 2] {
        let slice = read(&path, Some(offset), Some(8));
        assert_eq!(slice.content, "b", "偏移 {offset} 应前向对齐到字符边界");
        assert_eq!(slice.offset, 3);
    }
}

#[test]
fn read_file尾部残字被丢弃不产出替换符() {
    let dir = TempDir::new().expect("tempdir");
    // 5 字节：完整「甲」+「乙」的前两字节
    let mut bytes = "甲".as_bytes().to_vec();
    bytes.extend_from_slice(&"乙".as_bytes()[..2]);
    let path = write(&dir, "cut.txt", &bytes);

    let slice = read(&path, None, Some(100));

    assert_eq!(slice.content, "甲", "残缺尾序列整段丢弃");
    assert!(!slice.content.contains('\u{FFFD}'), "不产出替换符");
    assert!(slice.truncated, "剩余字节仍待读");
    assert!(slice.next_offset < slice.stat.size);
}

#[test]
fn read_file_binary_includes_nul() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "blob.bin", b"abc\0def");

    let slice = read(&path, None, None);

    assert_eq!(slice.encoding, "binary");
    assert_eq!(slice.content, "");
    assert!(!slice.truncated, "二进制不重复上报截断");
    assert_eq!(slice.stat.size, 7, "stat 仍给真实大小");
}

#[test]
fn read_file_binary_includes_invalid_utf8() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "blob2.bin", [0xFF, 0xFE, 0xFD, 0xFC, 0xFB]);

    let slice = read(&path, None, None);

    assert_eq!(slice.encoding, "binary");
    assert_eq!(slice.content, "");
}

#[test]
fn read_file空文件返回空内容非空切片() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "empty.txt", b"");

    let slice = read(&path, None, None);

    assert_eq!(slice.content, "");
    assert_eq!(slice.stat.size, 0);
    assert!(!slice.truncated);
    assert_eq!(slice.encoding, "utf8");
}

#[test]
fn read_file截断标记与next_offset() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "long.txt", b"x".repeat(100));

    let slice = read(&path, None, Some(10));

    assert_eq!(slice.content.len(), 10);
    assert!(slice.truncated);
    assert_eq!(slice.next_offset, 10);
    assert_eq!(slice.stat.size, 100);

    let tail = read(&path, Some(slice.next_offset), Some(200));
    assert_eq!(tail.content.len(), 90);
    assert!(!tail.truncated, "窗口覆盖剩余全部");
}

#[test]
fn read_file_limit夹到硬顶() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "big.txt", b"y".repeat(MAX_LIMIT as usize + 64));

    let slice = read(&path, None, Some(u64::MAX));

    assert_eq!(slice.content.len() as u64, MAX_LIMIT, "硬顶生效");
    assert!(slice.truncated);
}

#[test]
fn read_file缺省窗口为默认上限() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(
        &dir,
        "default.txt",
        b"z".repeat(DEFAULT_LIMIT as usize + 32),
    );

    let slice = read(&path, None, None);

    assert_eq!(slice.content.len() as u64, DEFAULT_LIMIT);
    assert!(slice.truncated);
}

#[test]
fn read_file目录返回stat非err() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_string_lossy().into_owned();

    let slice = read(&path, None, None);

    assert_eq!(slice.content, "");
    assert_eq!(slice.next_offset, 0);
    assert!(slice.stat.is_dir);
    assert!(!slice.stat.is_file);
}

#[test]
fn read_file缺失路径返回none() {
    let dir = TempDir::new().expect("tempdir");
    let missing = dir.path().join("nope.md").to_string_lossy().into_owned();

    assert_eq!(
        read_file_inner(&missing, None, None),
        Ok(None),
        "缺失 → None"
    );
}

#[test]
fn read_file空白路径为err() {
    for blank in ["", "   ", "\t"] {
        assert!(
            read_file_inner(blank, None, None).is_err(),
            "空白 path 应显式 Err: {blank:?}"
        );
    }
}

#[test]
fn read_file_offset越界夹到文件尾() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "short.txt", b"abc");

    let slice = read(&path, Some(999), None);

    assert_eq!(slice.content, "");
    assert_eq!(slice.offset, 3);
    assert!(!slice.truncated);
}

#[test]
fn read_file零limit夹到1() {
    let dir = TempDir::new().expect("tempdir");
    let path = write(&dir, "one.txt", b"abcdef");

    let slice = read(&path, None, Some(0));

    assert_eq!(slice.content, "a", "limit 下界为 1");
    assert!(slice.truncated);
}

#[test]
fn read_file预算小于首字符宽度时前向取整() {
    let dir = TempDir::new().expect("tempdir");
    // 「甲」3 字节 + 'b'：limit = 1 装不下一个字符，须前向取整到字符边界
    let path = write(&dir, "narrow.txt", "甲b");

    let slice = read(&path, None, Some(1));

    assert_eq!(slice.content, "甲", "前向取整至完整字符");
    assert_eq!(slice.next_offset, 3, "游标落在字符边界且必须前进");
    assert!(slice.truncated, "后面仍有 'b'");
}

#[test]
fn read_file中段非法字节以替换符呈现不误判二进制() {
    let dir = TempDir::new().expect("tempdir");
    // 文本中段混入一个非法字节：文件级编码仍是 utf8，该字节以替换符消费
    let mut bytes = b"abc".to_vec();
    bytes.push(0xFF);
    bytes.extend_from_slice(b"def");
    let path = write(&dir, "mixed.log", &bytes);

    let slice = read(&path, None, None);

    assert_eq!(slice.encoding, "utf8", "中段非法字节不判二进制");
    assert_eq!(slice.content, "abc\u{FFFD}def");
    assert_eq!(slice.next_offset, slice.stat.size, "游标越过非法字节");
    assert!(!slice.truncated);
}

#[test]
fn read_file编码判定与偏移无关() {
    let dir = TempDir::new().expect("tempdir");
    // 开头即非法（0xC3 后不跟续接字节）——文件级二进制，任一 offset 同结论
    let path = write(&dir, "head.bin", [0xC3, 0x28, b'a', b'b']);

    for offset in [0u64, 2] {
        let slice = read(&path, Some(offset), None);
        assert_eq!(slice.encoding, "binary", "offset {offset} 仍判二进制");
        assert_eq!(slice.next_offset, slice.stat.size, "游标一次到文件尾");
        assert!(!slice.truncated, "binary 恒不报截断");
    }
}

#[test]
fn read_file文件尾半字符零推进不报截断() {
    let dir = TempDir::new().expect("tempdir");
    // 完整「甲」+「乙」的首字节：末字符永远补不齐
    let mut bytes = "甲".as_bytes().to_vec();
    bytes.push("乙".as_bytes()[0]);
    let path = write(&dir, "tail.txt", &bytes);

    let head_slice = read(&path, None, None);
    assert_eq!(head_slice.content, "甲");
    assert_eq!(head_slice.next_offset, 3);
    assert!(head_slice.truncated, "半字符之前仍有内容");

    let tail_slice = read(&path, Some(3), None);
    assert_eq!(tail_slice.content, "", "半字符不产出内容");
    assert_eq!(tail_slice.next_offset, 3, "零推进");
    assert!(
        !tail_slice.truncated,
        "零推进时不得报截断（否则调用方死转）"
    );
}

#[test]
fn read_file大二进制文件游标到尾部且不报截断() {
    let dir = TempDir::new().expect("tempdir");
    let mut bytes = vec![0u8];
    bytes.extend(std::iter::repeat(b'x').take(MAX_LIMIT as usize + 64));
    let path = write(&dir, "big.bin", &bytes);

    let slice = read(&path, None, None);

    assert_eq!(slice.encoding, "binary");
    assert_eq!(slice.content, "");
    assert_eq!(slice.next_offset, slice.stat.size, "游标一次到文件尾");
    assert!(!slice.truncated, "binary 是终局判定，不报截断");
}
