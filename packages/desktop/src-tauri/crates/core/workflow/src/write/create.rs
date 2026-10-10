//! change 创建写操作（建域四段组合，design D2/D3/D4/D5）：前置七道校验
//!（名称 kebab-case + 长度 → goal 非空白 → 主仓 active 目录已存在 → db 同名
//! active 建档 → git 探测三态（不可发现 / 非 git 仓 / 空仓，MUST NOT 静默回退
//! 主 root 创建）→ branch `change/<name>` 冲突 → worktree 目录冲突——全部在
//! 任何 IO 之前，拒绝面零目录零记录零 vcs 调用）→ 铸出 uuid v7 形态 id →
//! db 建档先行（携 id / `worktree` / `base_commit`）→ `git worktree add`
//!（HEAD 基线铸分支）→ worktree 内建
//! `openspec/changes/<n>/` 目录树与 explore.md → 脏仓警告 → 确定性 bootstrap
//!（lockfile 映射表首匹配；失败不回滚、警告立即呈现）。补偿链：add 失败 →
//! 删本次建档 + 尽力删分支；树写出失败 → remove_worktree → 删分支 → 删建档
//!（尽力链，任一失败 Err 呈现残留对象与 `git worktree list` 手动清理指引）；
//! bootstrap 段失败不回收。MUST NOT 产出 workflow.json（双向墙）。sync 零
//! Tauri 零 tokio；双根（主仓根 / worktree 落位根）注入，两棵 `Layout` 写面
//! 内经 `foundation::layout::resolve` 自铸；落库经 [`ChangeStateStore`] port
//! 缝、进程执行经 [`WorktreePort`] port 缝。
//! 能力 spec：`specs/desktop-change-create/spec.md`（路径相对域根）。

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;
use time::OffsetDateTime;

use foundation::layout::resolve;

use crate::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus};
use crate::write::worktree::WorktreePort;

/// 名称长度上限（与插件 `createChange` 同宽）。
const MAX_NAME_LENGTH: usize = 128;

/// worktree 建域分支前缀（branch = `change/<name>`）。
const CHANGE_BRANCH_PREFIX: &str = "change/";

/// bootstrap 映射表（驻 create 私有常量，首匹配序，worktree 根探测；
/// design D4）：已知 lockfile → 冻结锁面安装命令（零 diff 污染——
/// `--frozen-lockfile` / `npm ci` / `--locked` 均不落新锁文件）。
const BOOTSTRAP_LOCKFILE_COMMANDS: [(&str, &str); 4] = [
    ("pnpm-lock.yaml", "pnpm install --frozen-lockfile"),
    ("package-lock.json", "npm ci"),
    ("yarn.lock", "yarn install --frozen-lockfile"),
    ("Cargo.lock", "cargo fetch --locked"),
];

/// 脏仓警告文案（D5 词汇单点：基线语义纯净的显式声明——未提交内容不会
/// 混入本 change）。
const WARN_DIRTY_MAIN: &str =
    "主仓有未提交改动（worktree 基线仍取 HEAD，未提交内容不会进入本 change）：建议先提交再开新 change";

/// 未知依赖管理器注记文案（D5 词汇单点）。
const WARN_NO_KNOWN_MANAGER: &str = "未识别依赖管理器，跳过依赖引导";

/// Cargo 无 lock 专项注记文案（D5 词汇单点：无 `--locked` 的 fetch 会铸出
/// 未跟踪 Cargo.lock，污染本 change 的 diff 面）。
const WARN_CARGO_NO_LOCK: &str =
    "检测到 Cargo.toml 但无 Cargo.lock，跳过依赖引导（避免生成未跟踪 lockfile 混入变更上下文）";

/// 创建产出（IPC DTO）：本次铸出的 change id（身份锚——前端导航 / 一切后续
/// 寻址入参）、名称、创建日期、worktree 绝对路径（刻意出线的执行锚——review /
/// 手动 commit / merge 可达）与警告清单（脏仓 / bootstrap 注记，持久入 DTO
/// 抵达前端行内呈现）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateOutcome {
    /// 本次铸出的 change id（uuid v7 形态；库内记录同值逐字一致）
    pub id: String,
    pub name: String,
    /// UTC 日历日期 `YYYY-MM-DD`（取 db 建档 `created_at`，写面铸出后随 DTO
    /// 直达命令返回，无需回读）
    pub created: String,
    /// 本 change 分配的 worktree 绝对路径（执行锚；db 记录同源）
    pub worktree: String,
    /// 警告清单（脏仓引导 / 依赖引导注记与失败；干净仓且顺利 → 空清单）
    pub warnings: Vec<String>,
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

/// 创建 change（建域四段组合）：前置七道校验全 IO 前置（拒绝面零 worktree、
/// 零建档、零目录）→ db 建档先行（`worktree` / `base_commit` 入记录）→
/// worktree add（HEAD 基线铸 `change/<name>` 分支）→ worktree 内目录树 +
/// explore.md（goal 原文直写，UTF-8 零结构包装）→ 脏仓警告 → bootstrap。
/// 双根注入：`main_root` 主仓根（目录冲突检查 + git 命令 `-C` 锚）、
/// `worktree_root` 落位父锚（worktree 目录 = `worktree_root/<name>`）。
pub fn create(
    main_root: &Path,
    worktree_root: &Path,
    store: &dyn ChangeStateStore,
    vcs: &dyn WorktreePort,
    name: &str,
    goal: &str,
) -> Result<CreateOutcome, String> {
    // 前置①②：名称与 goal（纯内存校验）
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
    // 前置③：主仓 active 目录已存在（既有三道校验风格——冲突检查先于 git
    // 探测，vcs 零调用）
    let main_layout = resolve(main_root);
    let main_dir = main_layout.changes_root.join(name);
    if main_dir.exists() {
        return Err(format!("change \"{name}\" 已存在: {}", main_dir.display()));
    }
    // 前置④：db 同名 active 记录（name 无唯一约束，主键冲突面消失——写面单
    // 点 name 扫描查重，仅拒同名 **active**；归档同名共存合法化，不拒（D11））
    if store
        .list_change_records()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|record| record.name == name && record.status == ChangeStatus::Active)
    {
        return Err(format!(
            "change \"{name}\" 已存在同名建档记录（status: {}）",
            ChangeStatus::Active.as_str()
        ));
    }
    // 前置⑤：git 探测三态（git 不可发现 / 非 git 仓 / 空仓无 HEAD 显式 Err
    // 引导——MUST NOT 静默回退主 root 创建；probe 产出同时是基线与脏仓源）
    let probe = vcs.probe(main_root)?;
    // 前置⑥：branch `change/<name>` 已存在（下次同名创建会撞分支的既有残留）
    let branch = format!("{CHANGE_BRANCH_PREFIX}{name}");
    if vcs.branch_exists(main_root, &branch)? {
        return Err(format!(
            "branch \"{branch}\" 已存在（可能是遗留的半成品 change）：请先清理（git branch -D {branch}）再创建"
        ));
    }
    // 前置⑦：worktree 目标目录已存在（数据根下落位冲突）
    let worktree = worktree_root.join(name);
    if worktree.exists() {
        return Err(format!(
            "worktree 目录已存在: {}（请先清理或换名创建）",
            worktree.display()
        ));
    }

    // 执行段一：db 建档先行（fs / vcs 失败可补偿；反向则出现被禁破口
    // 「worktree 在而记录缺」）——携 id / worktree / base_commit 执行锚。
    // id 铸出点（D1）：前置七道全过后、建档之前直铸 uuid v7（与 created_at
    // 同段；纯 id 生成无 IO 无 vcs 调用，拒绝面零铸出）
    let id = uuid::Uuid::now_v7().to_string();
    let created_at = super::now_millis();
    let worktree_path = worktree.to_string_lossy().into_owned();
    store
        .create_change_record(ChangeStateRecord {
            id: id.clone(),
            name: name.to_owned(),
            // V1 唯一支持的工作流类型（与发起前置校验同口径）
            workflow_type: "requirement".to_owned(),
            created_at,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: Some(worktree_path.clone()),
            base_commit: Some(probe.head.clone()),
        })
        .map_err(|error| error.to_string())?;

    // 执行段二：worktree 建域（HEAD 基线铸分支）。失败 → 删本次建档 + 尽力
    // 删分支（git worktree add 可能已铸分支后才失败）
    if let Err(error) = vcs.add_worktree(main_root, &worktree, &branch) {
        return Err(compensate_add_failure(
            store, vcs, main_root, &id, &branch, &error,
        ));
    }

    // 执行段三：worktree 内目录树 + explore.md。失败 → remove_worktree →
    // 删分支 → 删建档（尽力链）
    let worktree_layout = resolve(&worktree);
    let change_dir = worktree_layout.changes_root.join(name);
    if let Err(error) = write_fs_half(&change_dir, goal) {
        return Err(compensate_tree_failure(
            store, vcs, main_root, &id, &branch, &worktree, &error,
        ));
    }

    // 执行段四：脏仓警告 + 确定性 bootstrap（警告收集，失败不回滚不阻断）
    let mut warnings = Vec::new();
    if probe.dirty {
        warnings.push(WARN_DIRTY_MAIN.to_owned());
    }
    bootstrap(&worktree, vcs, &mut warnings);

    Ok(CreateOutcome {
        id,
        name: name.to_owned(),
        created: utc_date(created_at),
        worktree: worktree_path,
        warnings,
    })
}

/// fs 半边：`create_dir_all` 建树 + explore.md（goal 原文直写）。
fn write_fs_half(dir: &Path, goal: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("创建 change 目录失败: {error}"))?;
    fs::write(dir.join("explore.md"), goal)
        .map_err(|error| format!("写入 explore.md 失败: {error}"))
}

/// 确定性 bootstrap（design D4）：worktree 根探测 lockfile → 映射表首匹配
/// 命中即 spawn 安装（cwd = worktree）；无已知 lockfile → 跳过 + 注记（先判
/// Cargo 无 lock 专项注记，后落「未识别管理器」）。安装失败不回滚不阻断
/// （后续确定性检查步大声失败兜底），警告立即呈现（D5 词汇单点）。
fn bootstrap(worktree: &Path, vcs: &dyn WorktreePort, warnings: &mut Vec<String>) {
    for (lockfile, command) in BOOTSTRAP_LOCKFILE_COMMANDS {
        if !worktree.join(lockfile).exists() {
            continue;
        }
        match vcs.run_install(worktree, command) {
            Ok(run) if run.success => {}
            Ok(run) => warnings.push(format!("依赖引导失败（{command}）: {}", run.summary)),
            Err(error) => warnings.push(format!("依赖引导未执行成功（{command}）: {error}")),
        }
        return; // 首匹配即收口（单安装，映射表序）
    }
    if worktree.join("Cargo.toml").exists() {
        warnings.push(WARN_CARGO_NO_LOCK.to_owned());
    } else {
        warnings.push(WARN_NO_KNOWN_MANAGER.to_owned());
    }
}

/// add_worktree 失败补偿（D3）：删本次建档（按 id）+ 尽力 `delete_branch`
///（add 半途可能已铸分支）。补偿再失败 → Err 呈现残留对象与手动清理指引
/// （不静默自愈）。
fn compensate_add_failure(
    store: &dyn ChangeStateStore,
    vcs: &dyn WorktreePort,
    main_root: &Path,
    id: &str,
    branch: &str,
    add_error: &str,
) -> String {
    let record = compensate_record_delete(store, id);
    let branch_delete = match vcs.delete_branch(main_root, branch) {
        Ok(()) => None,
        Err(error) => Some(format!("branch \"{branch}\": {error}")),
    };
    match (record, branch_delete) {
        (None, None) => {
            format!("创建 change 失败: worktree 建域失败: {add_error}（建档与分支已补偿回收）")
        }
        (record_residual, branch_residual) => {
            let residuals = residual_note(record_residual.into_iter().chain(branch_residual));
            format!(
                "创建 change 失败: worktree 建域失败: {add_error}；补偿回收未完成，残留: {residuals}。\
                 请以 git worktree list 对账并手动清理（git worktree remove --force <path> / \
                 git branch -D {branch}）"
            )
        }
    }
}

/// 目录树 / explore.md 写出失败补偿（D3 尽力链）：`remove_worktree --force`
/// → `delete_branch` → 删本次建档（按 id）。任一失败 → Err 呈现残留对象
///（worktree / branch / 记录 id）与 `git worktree list` 手动清理指引。
fn compensate_tree_failure(
    store: &dyn ChangeStateStore,
    vcs: &dyn WorktreePort,
    main_root: &Path,
    id: &str,
    branch: &str,
    worktree: &Path,
    fs_error: &str,
) -> String {
    let worktree_note = worktree.to_string_lossy().into_owned();
    let mut residuals = Vec::new();
    if let Err(error) = vcs.remove_worktree(main_root, worktree) {
        residuals.push(format!("worktree {worktree_note}: {error}"));
    }
    if let Err(error) = vcs.delete_branch(main_root, branch) {
        residuals.push(format!("branch \"{branch}\": {error}"));
    }
    if let Some(record_note) = compensate_record_delete(store, id) {
        residuals.push(record_note);
    }
    if residuals.is_empty() {
        return format!(
            "创建 change 失败: {fs_error}（worktree、分支 {branch} 与建档已补偿回收；\
             可以 git worktree list 对账确认）"
        );
    }
    format!(
        "创建 change 失败: {fs_error}；补偿回收未完成，残留: {}。\
         请以 git worktree list 对账并手动清理（git worktree remove --force <path> / \
         git branch -D {branch}）",
        residual_note(residuals.into_iter())
    )
}

/// 建档补偿删除（按 id）：成功 → `None`；失败 → 残留记因（残留行经同 id
/// 防御拒绝在下一次铸出撞号时显式暴露）。
fn compensate_record_delete(store: &dyn ChangeStateStore, id: &str) -> Option<String> {
    store
        .delete_change_record(id)
        .err()
        .map(|error| format!("建档记录 \"{id}\": {error}"))
}

/// 残留清单拼形（`; ` 连接）。
fn residual_note(residuals: impl Iterator<Item = String>) -> String {
    residuals.collect::<Vec<_>>().join("; ")
}
