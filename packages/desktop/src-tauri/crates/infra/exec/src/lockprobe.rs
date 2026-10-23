use std::path::{Path, PathBuf};

use crate::output::is_source_excerpt_line;

/// 被锁文件路径的错误串三形态（大小写不敏感的行内包含匹配）。
const LOCKED_PATH_KEYWORDS: [&str; 3] = [
    "failed to rename archive file",
    "failed to write",
    "failed to remove",
];

/// 单次诊断解析出的被锁文件上限（防极端输出把归因文案撑爆）。
const LOCKED_PATHS_MAX: usize = 8;

/// 占用者清单项（`(进程名, pid)` 去重后的单条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockHolder {
    /// 进程名（Windows `RM_PROCESS_INFO.strAppName` / `lsof` 命令名）
    pub name: String,
    /// 进程 id
    pub pid: u32,
}

/// 诊断文本 → 被锁文件路径清单
pub fn locked_paths(diagnostics: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for line in diagnostics.lines() {
        if is_source_excerpt_line(line) {
            continue;
        }
        // ASCII 小写化只动 ASCII 字节，故关键词在原文中的字节位置仍然有效
        let lowered = line.to_ascii_lowercase();
        for keyword in LOCKED_PATH_KEYWORDS {
            let Some(index) = lowered.find(keyword) else {
                continue;
            };
            let Some(candidate) = clean_candidate(&line[index + keyword.len()..]) else {
                continue;
            };
            let path = PathBuf::from(candidate);
            if !paths.contains(&path) {
                paths.push(path);
            }
            if paths.len() >= LOCKED_PATHS_MAX {
                return paths;
            }
        }
    }
    paths
}

/// 路径尾随标点集合（剥句读 / 括号 / 包裹引号）。
const TRAILING_PUNCTUATION: [char; 11] = ['`', '"', '\'', '。', '，', '.', ',', ';', ':', ')', ']'];

/// 关键词之后的片段 → 裸路径（字符串层可判定的清洗，不做路径规范化——避免改写
/// 用户可见路径）。
fn clean_candidate(rest: &str) -> Option<String> {
    let trimmed = rest.trim_start();
    let trimmed = trimmed.trim_start_matches(':').trim_start();
    let truncated = match trimmed.find(": ") {
        Some(index) => &trimmed[..index],
        None => trimmed,
    };
    let stripped = strip_trailing_parenthetical(truncated).trim();
    let stripped = stripped.trim_start_matches(['`', '"', '\'']);
    let stripped = stripped.trim_end_matches(TRAILING_PUNCTUATION);
    let stripped = stripped.trim();
    (!stripped.is_empty()).then(|| stripped.to_owned())
}

/// 尾随括注剥离：错误串常把 OS 错误码括在路径之后
/// （`…libb.rmeta (os error 32)`）。只在括号内含 `error` 时剥离——普通路径里合法
/// 出现的 `(…)` 片段不被改写。
fn strip_trailing_parenthetical(text: &str) -> &str {
    let trimmed = text.trim_end();
    let Some(without_close) = trimmed.strip_suffix(')') else {
        return trimmed;
    };
    let Some(open) = without_close.rfind(" (") else {
        return trimmed;
    };
    let inner = &without_close[open + 1..];
    if inner.to_ascii_lowercase().contains("error") {
        &trimmed[..open]
    } else {
        trimmed
    }
}

/// 占用者点名（Windows Restart Manager / 非 Windows `lsof` 降级）：按
/// `(name, pid)` 去重；任一步失败、系统服务不可用或空清单均返回空 vec——**不
/// `Err`、不 panic、不阻断检查主流程**（只丧失归因文本）。
pub fn lock_holders(paths: &[PathBuf]) -> Vec<LockHolder> {
    if paths.is_empty() {
        return Vec::new();
    }
    #[cfg(windows)]
    {
        dedup_holders(restart_manager_holders(paths))
    }
    #[cfg(not(windows))]
    {
        dedup_holders(lsof_holders(paths))
    }
}

/// 归因原料单点
pub fn attribution_text(diagnostics: &str) -> Option<String> {
    let paths = locked_paths(diagnostics);
    if paths.is_empty() {
        return None;
    }
    let blocked: Vec<PathBuf> = paths
        .iter()
        .filter(|path| blocks_delete(path))
        .cloned()
        .collect();
    let named: &[PathBuf] = if blocked.is_empty() {
        paths.as_slice()
    } else {
        blocked.as_slice()
    };
    let holders = lock_holders(named);
    let holders_text = if holders.is_empty() {
        "未定位占用者（可能为内核过滤驱动（安全软件），无法归因到用户进程）".to_owned()
    } else {
        holders
            .iter()
            .map(|holder| format!("{} (pid {})", holder.name, holder.pid))
            .collect::<Vec<_>>()
            .join("、")
    };
    let paths_text = paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("、");
    Some(format!("占用进程：{holders_text}\n被锁文件：{paths_text}"))
}

/// 判据：以 `DELETE | FILE_READ_ATTRIBUTES` 访问权 + 全共享模式打开一次即关。
///
/// 打开成功 → 可 rename / delete（未阻塞）；`Err` 且 OS 错误码 `32`
///（`ERROR_SHARING_VIOLATION`）→ 存在不允许删除共享的持有者（与 rustc / cargo
/// 失败同条件）。其余错误（`NotFound` / `AccessDenied`）与打开成功一律判「未
/// 阻塞」——本判据不承担「文件不存在 / 权限不足」的归因。
///
/// 非 Windows 臂恒 `false`：POSIX 无强制共享语义，探针不适用（点名仍走 `lsof`）。
#[cfg(windows)]
fn blocks_delete(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;

    /// `DELETE` 标准权限（可否删 = 本判据要问的唯一问题）
    const DELETE: u32 = 0x0001_0000;
    /// `FILE_READ_ATTRIBUTES`（最小附加权限：零内容访问，仍能打开属性面）
    const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    /// `ERROR_SHARING_VIOLATION`
    const ERROR_SHARING_VIOLATION: i32 = 32;

    let opened = std::fs::OpenOptions::new()
        // 全共享（含 DELETE）：正常读句柄不会被误报为锁；探针自身也不阻挡他人
        .access_mode(DELETE | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path);
    match opened {
        // 打开成功即未阻塞；句柄立即关闭（零残留）
        Ok(handle) => {
            drop(handle);
            false
        }
        Err(error) => error.raw_os_error() == Some(ERROR_SHARING_VIOLATION),
    }
}

/// 非 Windows 臂：无强制共享语义可探，恒判「未阻塞」（点名仍走 `lsof`）。
#[cfg(not(windows))]
fn blocks_delete(_path: &Path) -> bool {
    false
}

/// Restart Manager 占用者点名（Windows）：单 session 注册全部点名路径 → `RmGetList`
/// 两次（首取所需条目数，`ERROR_MORE_DATA` 视为正常）→ 读 `strAppName` +
/// `dwProcessId`；session MUST 在返回前 `RmEndSession`。任一步失败 / 空清单 →
/// 空 vec。
#[cfg(windows)]
fn restart_manager_holders(paths: &[PathBuf]) -> Vec<LockHolder> {
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
        RM_PROCESS_INFO,
    };

    // 路径 → NUL 结尾 UTF-16（`wide` 持有缓冲区，`pointers` 只借其指针）
    let wide: Vec<Vec<u16>> = paths
        .iter()
        .map(|path| {
            path.to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect()
        })
        .collect();
    let files: Vec<*const u16> = wide.iter().map(|units| units.as_ptr()).collect();

    let mut handle: u32 = 0;
    let mut key: Vec<u16> = vec![0; CCH_RM_SESSION_KEY as usize + 1];
    let mut holders: Vec<LockHolder> = Vec::new();
    // SAFETY: 入参均为本函数持有的有效缓冲（session key / 路径宽串指针数组），
    // 句柄只在 `ERROR_SUCCESS` 时有效，且无论后续成功与否都在返回前 RmEndSession。
    unsafe {
        if RmStartSession(&mut handle, 0, key.as_mut_ptr()) != ERROR_SUCCESS {
            return Vec::new();
        }
        let registered = RmRegisterResources(
            handle,
            files.len() as u32,
            files.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        );
        if registered == ERROR_SUCCESS {
            let mut needed: u32 = 0;
            let mut count: u32 = 0;
            let mut reasons: u32 = 0;
            // 首取：缓冲区为空 → 返 `ERROR_MORE_DATA` 并置 `needed`（正常路径）
            let first = RmGetList(
                handle,
                &mut needed,
                &mut count,
                std::ptr::null_mut(),
                &mut reasons,
            );
            if first == ERROR_SUCCESS || first == ERROR_MORE_DATA {
                let mut infos: Vec<RM_PROCESS_INFO> =
                    vec![RM_PROCESS_INFO::default(); needed as usize];
                count = needed;
                let second = RmGetList(
                    handle,
                    &mut needed,
                    &mut count,
                    infos.as_mut_ptr(),
                    &mut reasons,
                );
                if second == ERROR_SUCCESS {
                    infos.truncate(count as usize);
                    holders = infos
                        .iter()
                        .map(|info| LockHolder {
                            name: app_name(info),
                            pid: info.Process.dwProcessId,
                        })
                        .collect();
                }
            }
        }
        let _ = RmEndSession(handle);
    }
    holders
}

/// `RM_PROCESS_INFO.strAppName`（定长 UTF-16，NUL 结尾）→ `String`。
#[cfg(windows)]
fn app_name(info: &windows_sys::Win32::System::RestartManager::RM_PROCESS_INFO) -> String {
    let units = &info.strAppName;
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// `lsof` 占用者点名（非 Windows 降级）：逐路径 `lsof -F pc -- <path>`，解析
/// `p<pid>` / `c<name>` 相邻行成对；`lsof` 不可用（不在 PATH）静默跳过——MUST NOT
/// 阻断主流程、MUST NOT 视为错误。
#[cfg(not(windows))]
fn lsof_holders(paths: &[PathBuf]) -> Vec<LockHolder> {
    let mut holders: Vec<LockHolder> = Vec::new();
    for path in paths {
        let Ok(output) = crate::SystemCommand::bare("lsof")
            .args(["-F", "pc", "--"])
            .arg(path)
            .into_std()
            .output()
        else {
            continue;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let mut pending_pid: Option<u32> = None;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix('p') {
                pending_pid = rest.trim().parse::<u32>().ok();
            } else if let Some(rest) = line.strip_prefix('c') {
                if let Some(pid) = pending_pid.take() {
                    holders.push(LockHolder {
                        name: rest.trim().to_owned(),
                        pid,
                    });
                }
            }
        }
    }
    holders
}

/// 按 `(name, pid)` 去重（保首次出现序）。
fn dedup_holders(holders: Vec<LockHolder>) -> Vec<LockHolder> {
    let mut unique: Vec<LockHolder> = Vec::new();
    for holder in holders {
        if !unique.contains(&holder) {
            unique.push(holder);
        }
    }
    unique
}

#[cfg(test)]
#[path = "lockprobe_test.rs"]
mod lockprobe_test;
