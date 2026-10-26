use super::windows_name::{is_reserved_path, RESERVED_DEVICE_NAMES};

/// 全集：25 个设备名逐字命中（大小写不敏感的基线锚）。
#[test]
fn 保留名全集命中() {
    for name in RESERVED_DEVICE_NAMES {
        assert!(is_reserved_path(name), "保留名 {name} 应命中");
    }
}

/// 形态族：扩展名 / ADS / 尾随空格 / 大小写 / 任意路径段均不改变保留性。
#[test]
fn 扩展名与ads与大小写形态命中() {
    for path in [
        "nul.txt",
        "NUL.TXT",
        "NuL",
        "nul.tar.gz",
        "nul.",
        "nul ",
        "nul:stream",
        "dir/aux.log",
        "dir\\aux.log",
        "a/nul/b.txt",
        "conin$",
        "CONOUT$.log",
        "com1.rs",
        "lpt0.log",
    ] {
        assert!(is_reserved_path(path), "{path:?} 应命中");
    }
}

/// 近邻反例：非保留名、非保留族、空段与点段不误伤。
#[test]
fn 近邻名字不误伤() {
    for path in [
        "",
        ".",
        "..",
        "nullable.txt",
        "console.txt",
        "auxiliary.log",
        "nulish",
        "nulx/b.txt",
        "com0",
        "com10",
        "lpt10",
        "conin",
        "conout",
    ] {
        assert!(!is_reserved_path(path), "{path:?} 不应命中");
    }
}
