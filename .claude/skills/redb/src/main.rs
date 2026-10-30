//! redb-peek — 只读检视 redb 数据库（native_db 感知）。
//!
//! redb 没有自描述 schema：表名与键/值类型名写在 B 树元数据里，值是不透明字节。
//! 因此本工具做三件事：
//!   1. 枚举表（普通表 / multimap 二级索引表）与行数、存储统计；
//!   2. 用「类型探针」反查每张表声明的类型，据此决定怎么迭代；
//!   3. 对 native_db 落库行：解 native_model 头（id / version），载荷能当 JSON
//!      读就美化输出，否则给 UTF-8 探测 + 十六进制。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use redb::{
    AccessGuard, Database, MultimapTableDefinition, MultimapTableHandle, ReadableMultimapTable,
    ReadableTable, ReadableTableMetadata, TableDefinition, TableHandle, TypeName,
};

// ---------------------------------------------------------------------------
// 键类型：复刻 native_db::db_type::Key 的类型身份
// ---------------------------------------------------------------------------

/// native_db 的主键 / 二级索引键都是裸字节，但 redb 打开表时会比对类型名。
/// native_db 注册为自定义类型 `DatabaseInnerKeyValue`，这里必须逐字复刻，
/// 否则 `open_table` 报 `TableTypeMismatch`。
#[derive(Debug)]
struct NativeKey;

impl redb::Value for NativeKey {
    type SelfType<'a> = &'a [u8];
    type AsBytes<'a> = &'a [u8];

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> &'a [u8]
    where
        Self: 'a,
    {
        data
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8]
    where
        Self: 'b,
    {
        value
    }

    fn type_name() -> TypeName {
        TypeName::new("DatabaseInnerKeyValue")
    }
}

impl redb::Key for NativeKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}

// ---------------------------------------------------------------------------
// 表编码探测
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Encoding {
    /// native_db 主表：Key = DatabaseInnerKeyValue，Value = &'static [u8]
    NativeDb,
    /// native_db 的 metadata 表：(&str, &str)
    StrStr,
    /// 其它自定义类型，只知道声明的类型名
    Unknown,
}

struct TableInfo {
    name: String,
    is_multimap: bool,
    rows: u64,
    encoding: Encoding,
    key_type: String,
    value_type: String,
}

fn describe_table(db: &Database, name: &str, is_multimap: bool) -> TableInfo {
    let txn = match db.begin_read() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("redb-peek: 开启读事务失败: {e}");
            std::process::exit(2);
        }
    };

    // 行数走 untyped 通道：不校验类型，任何表都能开。
    // 句柄只能从 list_* 拿到（Sealed 未公开，外部无法自造），故在入口处先取。
    let rows = if is_multimap {
        txn.list_multimap_tables()
            .ok()
            .and_then(|mut it| it.find(|h| h.name() == name))
            .and_then(|h| txn.open_untyped_multimap_table(h).ok())
            .map(|t| t.len().unwrap_or(0))
            .unwrap_or(0)
    } else {
        txn.list_tables()
            .ok()
            .and_then(|mut it| it.find(|h| h.name() == name))
            .and_then(|h| txn.open_untyped_table(h).ok())
            .map(|t| t.len().unwrap_or(0))
            .unwrap_or(0)
    };

    // 类型探测：逐个试已知编码，失败时用错误文本反查真实类型名。
    let mut encoding = Encoding::Unknown;
    let mut key_type = String::from("?");
    let mut value_type = String::from("?");
    let mut last_err = None;

    if is_multimap {
        if txn
            .open_multimap_table(MultimapTableDefinition::<NativeKey, NativeKey>::new(name))
            .is_ok()
        {
            encoding = Encoding::NativeDb;
            key_type = "DatabaseInnerKeyValue".into();
            value_type = "DatabaseInnerKeyValue".into();
        }
    } else {
        if txn
            .open_table(TableDefinition::<NativeKey, &'static [u8]>::new(name))
            .is_ok()
        {
            encoding = Encoding::NativeDb;
            key_type = "DatabaseInnerKeyValue".into();
            value_type = "&[u8]".into();
        } else if txn
            .open_table(TableDefinition::<&str, &str>::new(name))
            .is_ok()
        {
            encoding = Encoding::StrStr;
            key_type = "&str".into();
            value_type = "&str".into();
        } else if let Err(e) = txn.open_table(TableDefinition::<u64, u64>::new(name)) {
            // 该探针必然失配（除非真是 u64 表），错误文本形如
            // `<name> is of type Table<&str, &str>`——直接拿来报告真实类型。
            last_err = Some(e.to_string());
        }
    }

    if encoding == Encoding::Unknown {
        if let Some(msg) = last_err {
            if let Some(rest) = msg.split("is of type Table<").nth(1) {
                if let Some(inner) = rest.strip_suffix('>') {
                    if let Some((k, v)) = inner.split_once(", ") {
                        key_type = k.to_owned();
                        value_type = v.to_owned();
                    }
                }
            }
        }
    }

    TableInfo {
        name: name.to_owned(),
        is_multimap,
        rows,
        encoding,
        key_type,
        value_type,
    }
}

// ---------------------------------------------------------------------------
// 载荷解码
// ---------------------------------------------------------------------------

/// native_db 落库行 = native_model 头（u32 LE id + u32 LE version）+ 载荷。
/// 头是可以无条件信任的：native_db 写行时总会带上。
fn split_native_model_header(bytes: &[u8]) -> Option<(u32, u32, &[u8])> {
    if bytes.len() < 8 {
        return None;
    }
    let id = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    Some((id, version, &bytes[8..]))
}

fn looks_like_json(bytes: &[u8]) -> bool {
    matches!(bytes.first(), Some(b'{') | Some(b'['))
}

fn utf8_preview(bytes: &[u8], max: usize) -> Option<String> {
    let slice = &bytes[..bytes.len().min(max)];
    let text = std::str::from_utf8(slice).ok()?;
    if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return None;
    }
    Some(if bytes.len() > max {
        format!("{text}…")
    } else {
        text.to_owned()
    })
}

fn hex_preview(bytes: &[u8], max: usize) -> String {
    let slice = &bytes[..bytes.len().min(max)];
    let mut out = String::with_capacity(slice.len() * 3);
    for (i, b) in slice.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
    if bytes.len() > max {
        out.push_str(" …");
    }
    out
}

/// 从 bincode 载荷里榨出可读字符串。
///
/// bincode 1.3 把 `String` / `&str` 编码为 `u64 LE 长度 + UTF-8 字节`。没有
/// schema 也能靠这个找到全部字符串字段——这是「不镜像结构也能分析数据」的关键。
///
/// **两段式，优先级严格**：只要长度前缀扫描命中了任何串，就**只**返回它们。
/// 否则再兜底扫连续可打印 ASCII。这个顺序很要紧——f64 / u64 数值字段的字节
/// 常常能凑出 `"NCq@"` 这类假串，兜底扫描必须在无真串时才启用，否则噪声会
/// 淹没结果（`2_4_id` 的成本 / 时长字段就是这么暴露出来的）。
fn extract_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // ① 长度前缀扫描（精确）
    for i in 0..bytes.len().saturating_sub(8) {
        let len = u64::from_le_bytes(match bytes[i..i + 8].try_into() {
            Ok(a) => a,
            Err(_) => continue,
        });
        if len < min_len as u64 || len > 8192 {
            continue;
        }
        let start = i + 8;
        let end = start + len as usize;
        if end > bytes.len() {
            continue;
        }
        if let Ok(text) = std::str::from_utf8(&bytes[start..end]) {
            if is_printable(text) && seen.insert(text.to_owned()) {
                out.push(text.to_owned());
            }
        }
    }
    if !out.is_empty() {
        return out;
    }

    // ② 兜底：连续可打印串（仅当 ① 一无所获）
    let mut run = Vec::new();
    for &b in bytes {
        if b.is_ascii_graphic() || b == b' ' {
            run.push(b);
        } else {
            flush_run(&mut run, &mut out, &mut seen, min_len);
        }
    }
    flush_run(&mut run, &mut out, &mut seen, min_len);

    out
}

fn flush_run(
    run: &mut Vec<u8>,
    out: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
    min_len: usize,
) {
    if run.len() >= min_len.max(4) {
        if let Ok(text) = std::str::from_utf8(run) {
            let trimmed = text.trim();
            if trimmed.len() >= min_len && seen.insert(trimmed.to_owned()) {
                out.push(trimmed.to_owned());
            }
        }
    }
    run.clear();
}

fn is_printable(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| !c.is_control())
}

/// 把 JSON 压成一行字符串；超过 `max` 字节时截断（保留前缀，标明省略）。
/// 真实转录事件单条可达数万字符，不截断会把终端刷爆。
fn json_to_line(v: &serde_json::Value, max: usize) -> String {
    let s = serde_json::to_string(v).unwrap_or_else(|_| "<json>".into());
    if s.len() <= max {
        return s;
    }
    let mut cut = max;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}… ({} bytes total)", &s[..cut], s.len())
}

/// 把一行值渲染成人能读的样子。
fn render_value(bytes: &[u8], opts: &DumpOpts) -> String {
    if opts.hex {
        return hex_preview(bytes, opts.max_bytes);
    }
    if let Some((id, version, payload)) = split_native_model_header(bytes) {
        if looks_like_json(payload) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(payload) {
                return json_to_line(&v, opts.max_bytes);
            }
        }
        // bincode 载荷：能榨出真串就给串（分析数据时最有价值），榨不出才退十六进制。
        if !opts.hex {
            let strs = extract_strings(payload, opts.min_str);
            if !strs.is_empty() {
                let joined = serde_json::to_string(&strs).unwrap_or_else(|_| "[]".into());
                return format!("[{id}:{version}] {joined}");
            }
        }
        if let Some(text) = utf8_preview(payload, opts.max_bytes) {
            return format!("[{id}:{version}] {text}");
        }
        return format!(
            "[{id}:{version}] {{bincode}} {}",
            hex_preview(payload, opts.max_bytes)
        );
    }
    if looks_like_json(bytes) {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
            return json_to_line(&v, opts.max_bytes);
        }
    }
    let strs = extract_strings(bytes, opts.min_str);
    if !strs.is_empty() {
        return serde_json::to_string(&strs).unwrap_or_else(|_| "[]".into());
    }
    if let Some(text) = utf8_preview(bytes, opts.max_bytes) {
        return text;
    }
    hex_preview(bytes, opts.max_bytes)
}

fn render_key(bytes: &[u8], opts: &DumpOpts) -> String {
    if opts.hex {
        return hex_preview(bytes, opts.max_bytes);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) if !s.chars().any(|c| c.is_control()) => s.to_owned(),
        _ => format!("0x{}", hex_preview(bytes, opts.max_bytes)),
    }
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

struct DumpOpts {
    limit: usize,
    offset: usize,
    hex: bool,
    strings: bool,
    min_str: usize,
    max_bytes: usize,
    grep: Option<String>,
    jsonl: bool,
}

impl Default for DumpOpts {
    fn default() -> Self {
        Self {
            limit: usize::MAX,
            offset: 0,
            hex: false,
            strings: false,
            min_str: 4,
            max_bytes: 512,
            grep: None,
            jsonl: false,
        }
    }
}

fn list_tables(db: &Database) -> Vec<TableInfo> {
    let txn = match db.begin_read() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("redb-peek: 开启读事务失败: {e}");
            std::process::exit(2);
        }
    };
    let mut out = Vec::new();
    let mut normal: Vec<String> = txn
        .list_tables()
        .map(|it| it.map(|h| h.name().to_string()).collect())
        .unwrap_or_default();
    let mut multi: Vec<String> = txn
        .list_multimap_tables()
        .map(|it| it.map(|h| h.name().to_string()).collect())
        .unwrap_or_default();
    normal.sort();
    multi.sort();
    drop(txn);

    for name in normal {
        out.push(describe_table(db, &name, false));
    }
    for name in multi {
        out.push(describe_table(db, &name, true));
    }
    out
}

/// native_db 表名 `{model_id}_{model_version}_{key_field}`；非该形态返回 None。
fn parse_table_name(name: &str) -> Option<(u32, u32, &str)> {
    let mut parts = name.splitn(3, '_');
    let id = parts.next()?.parse::<u32>().ok()?;
    let version = parts.next()?.parse::<u32>().ok()?;
    let key = parts.next()?;
    Some((id, version, key))
}

fn cmd_tables(db: &Database, jsonl: bool) {
    let tables = list_tables(db);
    if jsonl {
        for t in &tables {
            let row = serde_json::json!({
                "table": t.name,
                "kind": if t.is_multimap { "multimap" } else { "table" },
                "rows": t.rows,
                "encoding": match t.encoding {
                    Encoding::NativeDb => "native_db",
                    Encoding::StrStr => "str_str",
                    Encoding::Unknown => "unknown",
                },
                "key_type": t.key_type,
                "value_type": t.value_type,
                "model_id": parse_table_name(&t.name).map(|p| p.0),
                "model_version": parse_table_name(&t.name).map(|p| p.1),
            });
            println!("{row}");
        }
        return;
    }

    println!(
        "{:<34} {:<9} {:>8}  {:<46} {}",
        "TABLE", "KIND", "ROWS", "KEY -> VALUE", "NATIVE MODEL"
    );
    for t in &tables {
        let model = match parse_table_name(&t.name) {
            Some((id, ver, key)) => format!("id={id} v{ver} key={key}"),
            None => "-".into(),
        };
        println!(
            "{:<34} {:<9} {:>8}  {:<46} {}",
            t.name,
            if t.is_multimap { "multimap" } else { "table" },
            t.rows,
            format!("{} -> {}", t.key_type, t.value_type),
            model
        );
    }
    let total: u64 = tables.iter().map(|t| t.rows).sum();
    println!("\n{} tables, {} rows total", tables.len(), total);
}

fn cmd_stats(db: &Database) {
    let tables = list_tables(db);
    println!(
        "{:<34} {:>8} {:>9} {:>12} {:>9}",
        "TABLE", "ROWS", "LEAF PGS", "STORED BYTES", "FRAG"
    );
    let txn = db.begin_read().ok();
    for t in &tables {
        let stat = match &txn {
            Some(txn) if t.is_multimap => txn
                .open_multimap_table(MultimapTableDefinition::<NativeKey, NativeKey>::new(
                    t.name.as_str(),
                ))
                .ok()
                .and_then(|tb| tb.stats().ok()),
            Some(txn) => match t.encoding {
                Encoding::NativeDb => txn
                    .open_table(TableDefinition::<NativeKey, &'static [u8]>::new(
                        t.name.as_str(),
                    ))
                    .ok()
                    .and_then(|tb| tb.stats().ok()),
                Encoding::StrStr => txn
                    .open_table(TableDefinition::<&str, &str>::new(t.name.as_str()))
                    .ok()
                    .and_then(|tb| tb.stats().ok()),
                Encoding::Unknown => None,
            },
            None => None,
        };
        let (leaf, stored, frag) = stat
            .map(|s| (s.leaf_pages(), s.stored_bytes(), s.fragmented_bytes()))
            .unwrap_or((0, 0, 0));
        println!(
            "{:<34} {:>8} {:>9} {:>12} {:>9}",
            t.name, t.rows, leaf, stored, frag
        );
    }
}

fn cmd_dump(db: &Database, table_name: &str, opts: &DumpOpts) {
    let info = list_tables(db)
        .into_iter()
        .find(|t| t.name == table_name)
        .unwrap_or_else(|| {
            eprintln!("redb-peek: 表不存在: {table_name}（先跑 `tables`）");
            std::process::exit(2);
        });

    let txn = db.begin_read().expect("读事务");
    let mut shown = 0usize;
    let mut seen = 0usize;
    let mut matched = 0usize;

    match (info.is_multimap, info.encoding) {
        // ---- native_db 主表 ----
        (false, Encoding::NativeDb) => {
            let table = txn
                .open_table(TableDefinition::<NativeKey, &'static [u8]>::new(table_name))
                .expect("native_db 主表打开失败");
            for item in table.iter().expect("迭代失败") {
                // offset/limit 在**未过滤**时可以在扫描阶段直接剪枝；
                // 带 --grep 时必须读完（命中数事先不可知）。
                if opts.grep.is_none() {
                    if seen >= opts.offset.saturating_add(opts.limit) {
                        break;
                    }
                }
                let (k, v): (AccessGuard<NativeKey>, AccessGuard<&'static [u8]>) =
                    item.expect("读行失败");
                seen += 1;
                let key = k.value().to_vec();
                let val = v.value().to_vec();
                if let Some(pat) = &opts.grep {
                    if !String::from_utf8_lossy(&val).contains(pat.as_str()) {
                        continue;
                    }
                }
                matched += 1;
                if seen <= opts.offset || shown >= opts.limit {
                    continue;
                }
                shown += 1;
                emit_row(&key, &val, opts);
            }
        }
        // ---- native_db 二级索引表（multimap）----
        (true, Encoding::NativeDb) => {
            let table = txn
                .open_multimap_table(MultimapTableDefinition::<NativeKey, NativeKey>::new(
                    table_name,
                ))
                .expect("multimap 打开失败");
            for item in table.iter().expect("迭代失败") {
                if opts.grep.is_none()
                    && seen >= opts.offset.saturating_add(opts.limit)
                {
                    break;
                }
                let (sk, values) = item.expect("读行失败");
                let sk = sk.value().to_vec();
                let sk_text = render_key(&sk, opts);
                for v in values {
                    let pk = v.expect("读值失败").value().to_vec();
                    let pk_text = render_key(&pk, opts);
                    seen += 1;
                    // 二级索引表两侧都该搜：按会话 id 查转录索引是主用法，
                    // 而按主键查（找某条记录的所有索引键）同样自然。
                    if let Some(pat) = &opts.grep {
                        let hit = sk_text.contains(pat.as_str())
                            || pk_text.contains(pat.as_str())
                            || String::from_utf8_lossy(&sk).contains(pat.as_str())
                            || String::from_utf8_lossy(&pk).contains(pat.as_str());
                        if !hit {
                            continue;
                        }
                    }
                    matched += 1;
                    if seen <= opts.offset || shown >= opts.limit {
                        continue;
                    }
                    shown += 1;
                    if opts.jsonl {
                        println!(
                            "{}",
                            serde_json::json!({
                                "secondary_key": sk_text,
                                "primary_key": pk_text,
                            })
                        );
                    } else {
                        println!("{sk_text}  =>  {pk_text}");
                    }
                }
            }
        }
        // ---- (&str, &str)：native_db 自己的 metadata 表 ----
        (false, Encoding::StrStr) => {
            let table = txn
                .open_table(TableDefinition::<&str, &str>::new(table_name))
                .expect("字符串表打开失败");
            for item in table.iter().expect("迭代失败") {
                let (k, v) = item.expect("读行失败");
                let key = k.value().to_owned();
                let val = v.value().to_owned();
                seen += 1;
                if let Some(pat) = &opts.grep {
                    if !val.contains(pat.as_str()) {
                        continue;
                    }
                }
                matched += 1;
                if seen <= opts.offset || shown >= opts.limit {
                    continue;
                }
                shown += 1;
                if opts.jsonl {
                    println!("{}", serde_json::json!({ "key": key, "value": val }));
                } else {
                    println!("{key} = {val}");
                }
            }
        }
        (is_mm, enc) => {
            eprintln!(
                "redb-peek: 表 {table_name} 的编码 {}（{}）暂不支持按行导出；\
                 仅支持 native_db 主表 / multimap 与 (&str,&str) 表。",
                if is_mm { "multimap" } else { "table" },
                match enc {
                    Encoding::Unknown => "未知类型",
                    _ => "未覆盖",
                }
            );
            std::process::exit(3);
        }
    }

    if !opts.jsonl {
        // 提前剪枝时 `scanned` 只是下界，如实标注，不冒充全表扫描。
        let pruned = opts.grep.is_none() && shown >= opts.limit && opts.limit != usize::MAX;
        eprintln!(
            "-- {shown} shown / {matched} matched / {seen} scanned{}{} (table rows = {})",
            if pruned { " (已按 --limit 提前停止，" } else { "" },
            if pruned { "matched 为下界)" } else { "" },
            info.rows
        );
    }
}

fn emit_row(key: &[u8], val: &[u8], opts: &DumpOpts) {
    if opts.jsonl {
        let row = if let Some((id, version, body)) = split_native_model_header(val) {
            let as_json = if looks_like_json(body) {
                serde_json::from_slice::<serde_json::Value>(body).ok()
            } else {
                None
            };
            match as_json {
                // JSONL 是给程序消费的：不截断，保证可解析。需要限流时用
                // --limit 或 --grep，不要指望 --max-bytes（那只管人读的预览）。
                Some(j) => serde_json::json!({
                    "key": render_key(key, opts),
                    "model_id": id,
                    "model_version": version,
                    "value": j,
                }),
                None => serde_json::json!({
                    "key": render_key(key, opts),
                    "model_id": id,
                    "model_version": version,
                    "payload_len": body.len(),
                    "strings": extract_strings(body, opts.min_str),
                }),
            }
        } else {
            serde_json::json!({
                "key": render_key(key, opts),
                "value_len": val.len(),
                "value": render_value(val, opts),
            })
        };
        println!("{row}");
    } else {
        println!("{}  =>  {}", render_key(key, opts), render_value(val, opts));
    }
}

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

/// redb 文件头速览。2.x 的 layout 断言会在 1.x 文件上 **panic**（不是返回
/// `UpgradeRequired`），所以既做前置判别给出可读原因，也保留 catch_unwind 兜底。
struct HeaderInfo {
    region_header_pages: u32,
}

enum SniffError {
    Io(std::io::Error),
    NotRedb,
    TooShort,
}

fn sniff_header(path: &Path) -> Result<HeaderInfo, SniffError> {
    use std::io::Read;
    const MAGIC: [u8; 9] = [b'r', b'e', b'd', b'b', 0x1A, 0x0A, 0xA9, 0x0D, 0x0A];
    let mut f = std::fs::File::open(path).map_err(SniffError::Io)?;
    let mut head = [0u8; 32];
    let n = f.read(&mut head).map_err(SniffError::Io)?;
    if n < 32 {
        return Err(SniffError::TooShort);
    }
    if head[..9] != MAGIC {
        return Err(SniffError::NotRedb);
    }
    Ok(HeaderInfo { region_header_pages: u32::from_le_bytes(head[16..20].try_into().unwrap()) })
}

/// redb 在损坏 / 旧格式文件上 panic。把 panic 收敛成 Err，并静默默认的
/// panic 打印，改由我们输出一条可操作的说明。
fn guarded_open(path: &Path) -> Result<Database, String> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Database::open(path)
    }));
    std::panic::set_hook(prev);

    match result {
        Ok(Ok(db)) => Ok(db),
        Ok(Err(redb::DatabaseError::DatabaseAlreadyOpen)) => Err("ALREADY_OPEN".into()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("PANIC".into()),
    }
}

/// 文件被别的进程独占时 Windows 报 ERROR_LOCK_VIOLATION(33) /
/// ERROR_SHARING_VIOLATION(32)，Unix 报 EBUSY(16) / EACCES(13)。
fn is_locked_io_error(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(33) | Some(32) | Some(16) | Some(13))
}

/// 尝试把被锁的库拷成快照再读。
///
/// **Windows 上这条路径通常不通**：redb 用 `LockFile` 锁住整个文件范围，
/// 这是强制锁，别的进程连读都读不了（`std::fs::copy` 直接失败）。此时唯一
/// 正确的解法是让用户关掉应用。Unix 上 redb 用 `flock`（劝告锁），读不受
/// 影响，快照路径可用。
fn try_snapshot(path: &Path) -> Option<(Database, PathBuf)> {
    let tmp = std::env::temp_dir().join(format!(
        "redb-peek-{}-{}.redb",
        std::process::id(),
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("db")
    ));
    if let Err(e) = std::fs::copy(path, &tmp) {
        eprintln!(
            "redb-peek: {} 被其它进程独占，且无法拷贝快照（{e}）。\n\
             这不是工具的问题：redb 在 Windows 上用强制字节区间锁，应用运行时\n\
             别的进程既不能读也不能复制。请先完全退出应用再重试。",
            path.display()
        );
        return None;
    }
    eprintln!(
        "redb-peek: {} 被占用，已读拷贝快照 {}",
        path.display(),
        tmp.display()
    );
    match guarded_open(&tmp) {
        Ok(db) => Some((db, tmp)),
        Err(e) => {
            eprintln!("redb-peek: 快照也打不开: {e}（应用正在写入？）");
            let _ = std::fs::remove_file(&tmp);
            None
        }
    }
}

fn open_db(path: &Path) -> (Database, Option<PathBuf>) {
    // ① 头部读不到：要么被锁（Windows 强制锁连读都拒），要么真坏了。
    let header = match sniff_header(path) {
        Ok(h) => h,
        Err(SniffError::Io(e)) if is_locked_io_error(&e) => {
            match try_snapshot(path) {
                Some((db, tmp)) => return (db, Some(tmp)),
                None => std::process::exit(3),
            }
        }
        Err(SniffError::Io(e)) => {
            eprintln!("redb-peek: 读取 {} 失败: {e}", path.display());
            std::process::exit(2);
        }
        Err(SniffError::NotRedb) => {
            eprintln!(
                "redb-peek: {} 不是 redb 文件（magic 不匹配）。",
                path.display()
            );
            std::process::exit(2);
        }
        Err(SniffError::TooShort) => {
            eprintln!(
                "redb-peek: {} 太短，不是有效库（空文件 / 截断）。",
                path.display()
            );
            std::process::exit(2);
        }
    };

    // ② redb 1.x 布局：region_header_pages = 0。2.x 的 layout 断言会 panic。
    if header.region_header_pages == 0 {
        eprintln!(
            "redb-peek: {} 是 redb 1.x 遗留格式（头部 region_header_pages = 0；\n\
             redb 2.x 读它会在 layout 断言里 panic）。这类文件来自旧版应用布局\n\
             （如 `desktop-store.redb`），当前版本已作废重建。请改为读取新布局的\n\
             `desktop-global.redb` 与 `workspaces/*.redb`。",
            path.display()
        );
        std::process::exit(4);
    }

    // ③ 正常打开；仍被锁则走快照。
    match guarded_open(path) {
        Ok(db) => (db, None),
        Err(e) if e == "ALREADY_OPEN" => match try_snapshot(path) {
            Some((db, tmp)) => (db, Some(tmp)),
            None => std::process::exit(3),
        },
        Err(e) if e == "PANIC" => {
            eprintln!(
                "redb-peek: 打开 {} 时 redb 内部 panic——文件格式过旧或已损坏。",
                path.display()
            );
            std::process::exit(4);
        }
        Err(e) => {
            eprintln!("redb-peek: 打开 {} 失败: {e}", path.display());
            std::process::exit(2);
        }
    }
}

fn usage() -> ! {
    eprintln!(
        r#"redb-peek — 只读检视 redb 数据库（native_db 感知）

用法:
  redb-peek <db.redb> tables [--jsonl]
  redb-peek <db.redb> stats
  redb-peek <db.redb> dump <table> [选项]

dump 选项:
  --limit N        最多输出 N 行（默认全部）
  --offset N       跳过前 N 行（在 grep 过滤后计）
  --hex            键/值一律十六进制
  --strings        载荷里榨出全部字符串（bincode 字段无 schema 也能看）
  --min-str N      --strings 的最短长度（默认 4）
  --max-bytes N    单值预览上限（默认 512）
  --grep TEXT      只保留值中包含该文本的行
  --jsonl          每行一个 JSON 对象（便于管道处理）

例:
  redb-peek desktop-global.redb tables
  redb-peek desktop-global.redb dump 1_1_root
  redb-peek ws.redb dump 7_2_id --strings
  redb-peek ws.redb dump 8_1_event_key --grep ToolUse --limit 5 --jsonl
"#
    );
    std::process::exit(1);
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }

    let mut idx = 0;
    // 允许 `--help` 出现在首位
    if args[0] == "--help" || args[0] == "-h" {
        usage();
    }
    let db_path = PathBuf::from(&args[idx]);
    idx += 1;
    let cmd = args.get(idx).map(|s| s.as_str()).unwrap_or("tables");
    idx += 1;

    let mut opts = DumpOpts::default();
    let mut table_name: Option<String> = None;
    let mut jsonl = false;

    while idx < args.len() {
        match args[idx].as_str() {
            "--limit" => {
                idx += 1;
                opts.limit = args.get(idx).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
            }
            "--offset" => {
                idx += 1;
                opts.offset = args.get(idx).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "--max-bytes" => {
                idx += 1;
                opts.max_bytes = args.get(idx).and_then(|s| s.parse().ok()).unwrap_or(512);
            }
            "--min-str" => {
                idx += 1;
                opts.min_str = args.get(idx).and_then(|s| s.parse().ok()).unwrap_or(4);
            }
            "--grep" => {
                idx += 1;
                opts.grep = args.get(idx).cloned();
            }
            "--hex" => opts.hex = true,
            "--strings" => opts.strings = true,
            "--jsonl" => {
                opts.jsonl = true;
                jsonl = true;
            }
            other if other.starts_with('-') => {
                eprintln!("redb-peek: 未知选项 {other}");
                usage();
            }
            other => table_name = Some(other.to_owned()),
        }
        idx += 1;
    }

    if !db_path.exists() {
        eprintln!("redb-peek: 文件不存在: {}", db_path.display());
        return ExitCode::from(2);
    }

    let (db, tmp) = open_db(&db_path);
    match cmd {
        "tables" | "list" => cmd_tables(&db, jsonl),
        "stats" => cmd_stats(&db),
        "dump" => {
            let name = table_name.unwrap_or_else(|| {
                eprintln!("redb-peek: dump 需要表名");
                usage();
            });
            cmd_dump(&db, &name, &opts);
        }
        other => {
            eprintln!("redb-peek: 未知命令 {other}");
            usage();
        }
    }

    drop(db);
    if let Some(tmp) = tmp {
        let _ = std::fs::remove_file(tmp);
    }
    ExitCode::SUCCESS
}
