//! change 创建写操作：三道前置校验（名称 kebab-case + 长度 → goal 非空白 →
//! 冲突双检查（目录已存在或 db 已有同名 active 记录），全部在任何 IO 之前，
//! 拒绝面零目录零记录）→ db 建档先行 → `create_dir_all` 建树 + explore.md
//! 落最初 goal → fs 失败补偿删除本次建档（design D5：建档先行使其仅在补偿
//! 再失败的双故障角落可能出现「目录在而记录缺」，且显式报错不静默自愈）。
//! MUST NOT 产出 workflow.json（双向墙）。sync 零 Tauri；磁盘路径全部经
//! [`Layout`](foundation::layout::Layout) 取得；落库经
//! [`ChangeStateStore`] port 缝。
//! 能力 spec：`specs/desktop-change-create/spec.md`（路径相对域根）。

use std::fs;

use serde::{Deserialize, Serialize};
use specta::Type;
use time::OffsetDateTime;

use foundation::layout::Layout;

use crate::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus};

/// 名称长度上限（与插件 `createChange` 同宽）。
const MAX_NAME_LENGTH: usize = 128;

/// 创建产出（IPC DTO）：仅名称与创建日期，磁盘路径知识不下沉前端。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateOutcome {
    pub name: String,
    /// UTC 日历日期 `YYYY-MM-DD`（取 db 建档 `created_at`，写面铸出后随 DTO
    /// 直达命令返回，无需回读）
    pub created: String,
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

/// UTC unix 毫秒 → UTC 日历日期 `YYYY-MM-DD`（时钟早于 epoch 取 epoch 日，
/// 不 panic）。
pub(crate) fn utc_date(millis: i64) -> String {
    let secs = if millis < 0 { 0 } else { millis / 1000 };
    let now = OffsetDateTime::from_unix_timestamp(secs).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

/// 创建 change：名称（kebab-case + ≤128）→ goal 非空白 → 冲突双检查三道
/// 校验全 IO 前置（拒绝面目标目录与既有记录零产生）；通过后 db 建档先行，
/// 再 `create_dir_all` 建树与 explore.md（goal 原文直写，UTF-8 零结构包装）；
/// fs 半边失败补偿删除本次建档（独立写面调用，只删本次自插行）。
pub fn create(
    layout: &Layout,
    store: &dyn ChangeStateStore,
    name: &str,
    goal: &str,
) -> Result<CreateOutcome, String> {
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
    // 冲突双检查 db 半边（同名 active 记录显式拒绝，db 零建档）
    if let Some(record) = store
        .get_change(name)
        .map_err(|error| error.to_string())?
        .filter(|record| record.status == ChangeStatus::Active)
    {
        return Err(format!(
            "change \"{name}\" 已存在同名建档记录（status: {}）",
            record.status.as_str()
        ));
    }

    // db 建档先行（fs 失败可补偿；反向则出现被禁破口「目录在而记录缺」）
    let created_at = super::now_millis();
    store
        .create_change_record(ChangeStateRecord {
            name: name.to_owned(),
            // V1 唯一支持的工作流类型（与发起前置校验同口径）
            workflow_type: "requirement".to_owned(),
            created_at,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
        })
        .map_err(|error| error.to_string())?;

    // fs 半边：建树 + explore.md；任一失败补偿删除本次建档
    if let Err(error) = write_fs_half(&dir, goal) {
        return Err(compensate(store, name, &error));
    }

    Ok(CreateOutcome {
        name: name.to_owned(),
        created: utc_date(created_at),
    })
}

/// fs 半边：`create_dir_all` 建树 + explore.md（goal 原文直写）。
fn write_fs_half(dir: &std::path::Path, goal: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("创建 change 目录失败: {error}"))?;
    fs::write(dir.join("explore.md"), goal)
        .map_err(|error| format!("写入 explore.md 失败: {error}"))
}

/// fs 半边失败补偿：删除本次建档；补偿亦失败则 Err 呈现残留记录名（不静默
/// 自愈——残留记录随下一次同名建档的冲突检查显式暴露）。
fn compensate(store: &dyn ChangeStateStore, name: &str, fs_error: &str) -> String {
    match store.delete_change_record(name) {
        Ok(_) => format!("创建 change 失败: {fs_error}（建档已补偿回滚）"),
        Err(delete_error) => format!(
            "创建 change 失败: {fs_error}；且建档补偿删除失败，残留记录 \"{name}\" 需手工处理: {delete_error}"
        ),
    }
}
