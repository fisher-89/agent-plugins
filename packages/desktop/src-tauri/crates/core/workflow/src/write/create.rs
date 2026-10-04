//! change 创建写操作：三道前置校验（名称 kebab-case + 长度 → goal 非空白 →
//! 已存在拒绝，全部在任何 IO 之前，拒绝面零目录零文件）→ `create_dir_all`
//! 建树 → workflow.json 初始文档键序定形写出 → explore.md 落最初 goal。
//! sync 零 Tauri；磁盘路径全部经 [`Layout`](foundation::layout::Layout) 取得。
//! 初始文档与插件 `createChange` 字段集 / 键序 / 值三一致（差异仅序列化
//! 空白形态：插件紧凑单行、本面 2 空格 pretty），代际检测即判 v2。
//! 能力 spec：`specs/desktop-change-create/spec.md`（路径相对域根）。

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;
use time::OffsetDateTime;

use foundation::layout::Layout;

use crate::parse::WORKFLOW_FILE_NAME;

/// 名称长度上限（与插件 `createChange` 同宽）。
const MAX_NAME_LENGTH: usize = 128;

/// 创建产出（IPC DTO）：仅名称与创建日期，磁盘路径知识不下沉前端。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateOutcome {
    pub name: String,
    /// UTC 日历日期 `YYYY-MM-DD`（写面铸出后随 DTO 直达命令返回，无需回读）
    pub created: String,
}

/// workflow.json 初始文档（局部结构体按字段声明序序列化，键序契约落点：
/// `workflow_type` → `created` → `file_log`）。不经 `serde_json::Value`
/// 组装——本 workspace serde_json 未启用 `preserve_order`，Value 对象为
/// 字母序 Map，声明序键序无法经 Value 保证。无 `eval` 键。
#[derive(Serialize)]
struct InitialWorkflowDoc {
    /// 恒 `"requirement"`（V1 唯一支持的工作流类型，与发起前置校验同口径）
    workflow_type: &'static str,
    /// UTC 日历日期 `YYYY-MM-DD`（与插件 `toISOString().slice(0, 10)` 同式）
    created: String,
    /// 恒空数组起步（代际检测以本键判 v2，缺键即 legacy）
    file_log: [(); 0],
}

/// kebab-case 字符级判定：等价正则 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` 语义
///（不引 regex 依赖）。首段以小写字母开头、仅小写字母 / 数字；后续段以
/// `-` 起头且各至少一个字符（可数字开头）——即禁前导数字、连号连字符、
/// 尾连字符与空段。
fn is_kebab_case(name: &str) -> bool {
    let segment_ok = |first: bool, segment: &str| {
        let mut chars = segment.chars();
        match chars.next() {
            // 首段必须字母开头；后续段可数字开头
            Some(c) if c.is_ascii_lowercase() || (!first && c.is_ascii_digit()) => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    };
    let mut segments = name.split('-');
    match segments.next() {
        Some(first) => {
            if !segment_ok(true, first) {
                return false;
            }
        }
        // split 至少返回一段，防御式拒绝
        None => return false,
    }
    segments.all(|segment| segment_ok(false, segment))
}

/// 当前 UTC 日历日期 `YYYY-MM-DD`。
fn utc_date_today() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

/// 创建 change：名称（kebab-case + ≤128）→ goal 非空白 → 已存在拒绝三道
/// 校验全 IO 前置（拒绝面目标目录与文件零产生、既有 change 零改动）；通过后
/// `create_dir_all` 建树，写 workflow.json 初始文档（键序 `workflow_type` →
/// `created` → `file_log`、无 `eval` 键、2 空格 pretty + 尾换行）与
/// explore.md（goal 原文直写，UTF-8 零结构包装）。
pub fn create(layout: &Layout, name: &str, goal: &str) -> Result<CreateOutcome, String> {
    if !is_kebab_case(name) {
        return Err(format!(
            "name 必须为 kebab-case（小写字母/数字，可用 `-` 连接），收到: {name:?}"
        ));
    }
    if name.len() > MAX_NAME_LENGTH {
        return Err(format!(
            "name 长度超过 {MAX_NAME_LENGTH} 字符限制（当前 {} 字符）",
            name.chars().count()
        ));
    }
    if goal.trim().is_empty() {
        return Err("goal 不得为空白（须为非空的需求描述）".to_owned());
    }
    let dir = layout.changes_root.join(name);
    if dir.exists() {
        return Err(format!("change \"{name}\" 已存在: {}", dir.display()));
    }

    fs::create_dir_all(&dir).map_err(|error| format!("创建 change 目录失败: {error}"))?;

    let doc = InitialWorkflowDoc {
        workflow_type: "requirement",
        created: utc_date_today(),
        file_log: [],
    };
    let workflow_path = dir.join(WORKFLOW_FILE_NAME);
    let mut text = serde_json::to_string_pretty(&doc)
        .map_err(|error| format!("workflow.json 序列化失败: {error}"))?;
    text.push('\n');
    fs::write(&workflow_path, text).map_err(|error| format!("写入 workflow.json 失败: {error}"))?;

    let explore_path = dir.join("explore.md");
    fs::write(&explore_path, goal).map_err(|error| format!("写入 explore.md 失败: {error}"))?;

    Ok(CreateOutcome {
        name: name.to_owned(),
        created: doc.created,
    })
}
