//! 通用本地文件读取轨道：`read_file` 是前端唯一的内容读取入口。
//!
//! 替换面（本变更删三个旧命令）：`archi_read_doc`（架构文档关联集内的受限读取）
//! 与 `read_explore`（explore 笔记专用读取）退场，改为「先 `explore_doc_path` /
//! 直接拼绝对路径，再 `read_file`」；`read_exec_output` 为零消费者的死命令，删除。
//!
//! 三件事纪律沿 `commands/config` 模板：参数转换 → 调用 → 错误映射。命令层零
//! 领域规则——不认识 change / explore / 架构模型，只认路径与字节窗口。
//!
//! **范围**：不限制路径（对齐产品口径「整个文件系统」）。故本命令 MUST NOT 承担
//! 越界校验；它的约束只有三条：字节窗口、UTF-8 边界对齐、单次大小上限。
//!
//! 能力 spec：`specs/desktop-file-read/spec.md`。

use std::fs;
use std::io::{Read, Seek, SeekFrom};

/// 单次读取缺省窗口（字节）。
const DEFAULT_LIMIT: u64 = 1024 * 1024;

/// 单次读取硬顶（字节）：上限是稳定性面也是安全面——防一次把 GB 级文件读进
/// WebView 内存。
const MAX_LIMIT: u64 = 8 * 1024 * 1024;

/// 文件 stat：与内容同一次取数一起出线（调用方无需二次往返）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    /// 全文件字节数
    pub size: u64,
    /// 修改时间（Unix 毫秒；平台不支持时为 `None`）
    pub modified_ms: Option<u64>,
    pub is_file: bool,
    pub is_dir: bool,
}

/// 内容切片：**字节**窗口（不是行窗口）——`read_exec_output_delta` 的游标就是
/// UTF-8 字节偏移，行分页无法表达无稳定行界的流式文本。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileSlice {
    /// UTF-8 文本切片；`encoding == "binary"` 时为空串
    pub content: String,
    pub stat: FileStat,
    /// 实际起点（已前向对齐到字符边界）
    pub offset: u64,
    /// 下次调用应传的游标；唯一产出方在后端
    pub next_offset: u64,
    /// 后面还有未读字节
    pub truncated: bool,
    /// `"utf8"` | `"binary"`（开放词表：后续可加 `"base64"` 而不破契约）
    pub encoding: String,
}

/// 窗口首部前向对齐：跳过续接字节（`10xxxxxx`，最多 3 个）落在字符首字节。
/// 入参 `offset` 落在多字节中间时据此前移，保证「连续分页拼接 == 全文」。
fn align_forward(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take(3)
        .take_while(|byte| (**byte & 0xC0) == 0x80)
        .count()
}

/// 完整 UTF-8 前缀长度：尾部残缺序列整段丢弃（不产出替换符）。
fn utf8_prefix_len(bytes: &[u8]) -> usize {
    match std::str::from_utf8(bytes) {
        Ok(_) => bytes.len(),
        Err(error) => error.valid_up_to(),
    }
}

fn modified_millis(metadata: &fs::Metadata) -> Option<u64> {
    let modified = metadata.modified().ok()?;
    let elapsed = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_millis()).ok()
}

/// [`read_file`] 的领域组装纯函数（离 Tauri 运行时可测）。
///
/// `Ok(None)` 的语义收敛为「无内容可读」：路径不存在、目标是目录、内容为二进制。
/// 其余 I/O 故障（权限、设备错误）显式 `Err`——与旧 `read_explore`「一切失败都
/// 塌成 None」相比，这里把真故障与空态分开，属严格改进而非回归。
pub fn read_file_inner(
    path: &str,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<Option<FileSlice>, String> {
    if path.trim().is_empty() {
        return Err("path 不得为空白".to_owned());
    }
    let target = std::path::Path::new(path);
    let metadata = match fs::metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取失败（{path}）：{error}")),
    };
    let stat = FileStat {
        size: metadata.len(),
        modified_ms: modified_millis(&metadata),
        is_file: metadata.is_file(),
        is_dir: metadata.is_dir(),
    };
    if !metadata.is_file() {
        return Ok(Some(FileSlice {
            content: String::new(),
            stat,
            offset: 0,
            next_offset: 0,
            truncated: false,
            encoding: "utf8".to_owned(),
        }));
    }

    let size = metadata.len();
    let offset = offset.unwrap_or(0).min(size);
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    // 多读 3 字节：既供尾部残字判定，也供起点对齐后仍能取满一个完整窗口
    let want = limit.saturating_add(3).min(size - offset);
    let mut window = Vec::with_capacity(usize::try_from(want).unwrap_or(0));
    let mut file =
        fs::File::open(target).map_err(|error| format!("读取失败（{path}）：{error}"))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("读取失败（{path}）：{error}"))?;
    file.take(want)
        .read_to_end(&mut window)
        .map_err(|error| format!("读取失败（{path}）：{error}"))?;

    let skip = align_forward(&window);
    let body = &window[skip..];
    // 窗口封顶在 `limit` 字节内再判 UTF-8：多读的 3 字节只用于「尾部残字」判定，
    // 不进内容，否则 `content.len()` 会越过上限
    let window_limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let capped = &body[..body.len().min(window_limit)];
    let prefix = utf8_prefix_len(capped);
    // 二进制判定：有 NUL 字节，或整个窗口即非法 UTF-8 起始（窗口 ≤ 3 字节时前缀
    // 零合法只可能是文件尾截断的半字符，不算二进制）
    let binary = capped.contains(&0) || (prefix == 0 && capped.len() >= 4);
    let (content, consumed, encoding): (String, usize, &str) = if binary {
        (String::new(), skip + capped.len(), "binary")
    } else {
        (
            String::from_utf8_lossy(&capped[..prefix]).into_owned(),
            skip + prefix,
            "utf8",
        )
    };
    let next_offset = offset.saturating_add(u64::try_from(consumed).unwrap_or(limit));
    Ok(Some(FileSlice {
        content,
        stat,
        offset: offset.saturating_add(u64::try_from(skip).unwrap_or(0)),
        next_offset,
        truncated: next_offset < size,
        encoding: encoding.to_owned(),
    }))
}

/// 通用文件读取（唯一内容出口）：字节分页 + 同批 stat。
#[tauri::command]
#[specta::specta]
pub fn read_file(
    path: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<Option<FileSlice>, String> {
    read_file_inner(&path, offset, limit)
}

#[cfg(test)]
mod mod_test;
