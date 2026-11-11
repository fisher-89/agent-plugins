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
    /// 后面还有未读字节（不变量：置真时 `nextOffset > offset`）
    pub truncated: bool,
    /// `"utf8"` | `"binary"`（开放词表：后续可加 `"base64"` 而不破契约）
    pub encoding: String,
}

/// 编码探测窗口（字节）：只看文件头，故编码判定与 `offset` 无关。
const PROBE_LIMIT: u64 = 4 * 1024;

/// 窗口首部前向对齐：跳过续接字节（`10xxxxxx`，最多 3 个）落在字符首字节。
/// 入参 `offset` 落在多字节中间时据此前移，保证「连续分页拼接 == 全文」。
fn align_forward(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take(3)
        .take_while(|byte| (**byte & 0xC0) == 0x80)
        .count()
}

/// UTF-8 首字节所需的完整序列长度；`0xC0` / `0xC1`（过长编码）与
/// `0xF5..=0xFF`（超出 U+10FFFF）不是合法首字节 ⇒ `None`。
fn char_width(byte: u8) -> Option<usize> {
    match byte {
        0x00..=0x7F => Some(1),
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
}

/// 编码判定（文件级，只看头部）：含 NUL 字节，或文件**开头**即真非法序列。
///
/// `error_len` 为 `None` 只说明探测窗截断了合法前缀，不算二进制；中段非法字节
/// 也不判二进制——那是文本里混入的代码页字节，按替换符消费。否则同一文件按
/// `offset` 读出两种结论，且整窗内容被静默跳过。
fn is_binary(head: &[u8]) -> bool {
    head.contains(&0)
        || matches!(
            std::str::from_utf8(head),
            Err(error) if error.valid_up_to() == 0 && error.error_len().is_some()
        )
}

/// 窗口终点：预算内最大的字符边界。末字符在预算处被截断且**窗口内后续字节能
/// 补齐**时前向取整（至多溢出 3 字节）；补不齐（文件尾半字符）则退位到该字符
/// 起点。非法首字节按预算消费（lossy 替换），保证游标越过。
fn window_end(body: &[u8], budget: usize) -> usize {
    let budget = budget.min(body.len());
    for back in 1..=budget.min(3) {
        let index = budget - back;
        let byte = body[index];
        if byte & 0b1100_0000 == 0b1000_0000 {
            continue; // 续接字节：继续前看首字节
        }
        let Some(need) = char_width(byte) else {
            return budget;
        };
        return if index + need <= budget {
            budget
        } else if index + need <= body.len() {
            index + need
        } else {
            index
        };
    }
    budget // 回看窗内全是续接字节（非法流）：按预算消费
}

fn modified_millis(metadata: &fs::Metadata) -> Option<u64> {
    let modified = metadata.modified().ok()?;
    let elapsed = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_millis()).ok()
}

/// [`read_file`] 的领域组装纯函数（离 Tauri 运行时可测）。
///
/// `Ok(None)` 只表示「路径不存在」；目录与二进制返回带 `stat` 的空内容切片（空态
/// 与缺失可分辨）。其余 I/O 故障（权限、设备错误）显式 `Err`——与旧 `read_explore`
/// 「一切失败都塌成 None」相比，这里把真故障与空态分开，属严格改进而非回归。
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
    let mut file =
        fs::File::open(target).map_err(|error| format!("读取失败（{path}）：{error}"))?;

    // 编码是文件级属性（只看头部）：先探头再取窗口，避免「同一文件按 offset 读出
    // 两种编码结论」。二进制是终局判定——这类内容永远读不出来，游标一次性到文件尾。
    let mut head = Vec::with_capacity(usize::try_from(PROBE_LIMIT.min(size)).unwrap_or(0));
    (&file)
        .take(PROBE_LIMIT.min(size))
        .read_to_end(&mut head)
        .map_err(|error| format!("读取失败（{path}）：{error}"))?;
    if is_binary(&head) {
        return Ok(Some(FileSlice {
            content: String::new(),
            stat,
            offset,
            next_offset: size,
            truncated: false,
            encoding: "binary".to_owned(),
        }));
    }

    // 多读 3 字节：起点对齐与尾部补齐各需至多 3 字节，不额外多读
    let want = limit.saturating_add(3).min(size - offset);
    let mut window = Vec::with_capacity(usize::try_from(want).unwrap_or(0));
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("读取失败（{path}）：{error}"))?;
    file.take(want)
        .read_to_end(&mut window)
        .map_err(|error| format!("读取失败（{path}）：{error}"))?;

    let skip = align_forward(&window);
    let body = &window[skip..];
    let budget = usize::try_from(limit).unwrap_or(usize::MAX);
    let end = window_end(body, budget);
    let next_offset = offset.saturating_add(u64::try_from(skip + end).unwrap_or(limit));
    Ok(Some(FileSlice {
        content: String::from_utf8_lossy(&body[..end]).into_owned(),
        stat,
        offset: offset.saturating_add(u64::try_from(skip).unwrap_or(0)),
        // 零推进只出现在文件尾半字符：此时「后面还有字节」不成立（残留不足一个
        // 字符），报截断会让调用方按 `nextOffset` 死转
        truncated: next_offset > offset && next_offset < size,
        next_offset,
        encoding: "utf8".to_owned(),
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
