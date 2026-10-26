/// Git for Windows `compat/mingw.c::is_valid_win32_path` 的保留设备名集合：
/// 经典设备名 + Win32 控制台设备 `CONIN$` / `CONOUT$`。`COM0` 不保留（git
/// 允许）；`LPT0` 保留（git 的 LPT 判据为单个十进制数字 0-9）。
pub const RESERVED_DEVICE_NAMES: [&str; 25] = [
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM1", "COM2", "COM3", "COM4", "COM5",
    "COM6", "COM7", "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
    "LPT8", "LPT9",
];

/// 路径是否含 Windows 保留设备名段：任意段、ASCII 大小写不敏感；段内首个
/// `.` / `:` 前的词干命中即真（`NUL.txt` / `dir/aux.log` / `nul:stream` 均
/// 命中——扩展名与 ADS 后缀不改变保留性）；尾随空格按 Win32 归一语义剥除。
pub fn is_reserved_path(path: &str) -> bool {
    path.replace('\\', "/").split('/').any(is_reserved_segment)
}

fn is_reserved_segment(segment: &str) -> bool {
    let stem = segment
        .split(['.', ':'])
        .next()
        .unwrap_or("")
        .trim_end_matches(' ');
    !stem.is_empty()
        && RESERVED_DEVICE_NAMES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(stem))
}
