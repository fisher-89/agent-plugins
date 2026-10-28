use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;
use time::OffsetDateTime;

use foundation::layout::resolve;
use foundation::windows_name::is_reserved_path;

use crate::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus};
use crate::write::phase_table::phase_table;
use crate::write::worktree::WorktreePort;

/// 名称长度上限（与插件 `createChange` 同宽）。
const MAX_NAME_LENGTH: usize = 128;

/// worktree 建域分支前缀（branch = `change/<name>`）。
const CHANGE_BRANCH_PREFIX: &str = "change/";

/// active change 建档数上限（前置⑧；源码常量不进 `config.json`，沿
/// `FEEDBACK_LOOP_LIMIT` 先例——配置面先例是用户域配置非流程护栏）。
const ACTIVE_CHANGE_LIMIT: usize = 5;

/// worktree 磁盘保底门槛冗余量（前置⑨门槛 = [`ACTIVE_CHANGE_LIMIT`] + 本值
/// 派生：db 与磁盘漂移防线——孤儿 worktree / 手动残留不占 active 额度但占磁
/// 盘，+2 容忍既有在途漂移）。
const WORKTREE_DIR_SLACK: usize = 2;

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

/// 创建 change
#[allow(clippy::too_many_arguments)]
pub fn create(
    main_root: &Path,
    store: &dyn ChangeStateStore,
    vcs: &dyn WorktreePort,
    project_roots: &[String],
    name: &str,
    goal: &str,
    title: &str,
    workflow_type: &str,
) -> Result<CreateOutcome, String> {
    // 前置①②：名称与 goal（纯内存校验）
    if !is_kebab_case(name) {
        return Err(format!(
            "name 必须为 kebab-case（小写字母/数字，可用 `-` 连接），收到: {name:?}"
        ));
    }
    // Windows 保留设备名（nul/con/prn/aux/com1-9/lpt0-9）会令 worktree 目录
    // 创建失败（`worktree_root.join(name)` 落成设备 / 非法目录）——按可移植性
    // 规则无条件拒绝，前端同口径本地拦截。
    if is_reserved_path(name) {
        return Err(format!(
            "name {name:?} 是 Windows 保留设备名（无法作为 worktree 目录名），请换一个名称"
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
    // 前置②补：标题回退单点（title 恒非空不变量——空白标题回退 name，前端
    // 零空态回退分支）
    let title = if title.trim().is_empty() { name } else { title };
    // 前置②补：workflow_type 合法值校验（`phase_table` 命中为唯一权威；落
    // canonical 小写值——未知类型显式拒绝，零退化 requirement）
    let workflow_type = workflow_type.trim().to_ascii_lowercase();
    if phase_table(&workflow_type).is_none() {
        return Err(format!(
            "workflow_type \"{workflow_type}\" 不受支持（支持 requirement / test-only）"
        ));
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
    // 前置⑧：active 建档数上限（worktree 生命周期护栏；archived 记录不计入。
    // 全 IO 前置——位于 git 探测之前，拒绝面零 vcs 调用零建档零目录零 worktree）
    let active_count = store
        .list_change_records()
        .map_err(|error| error.to_string())?
        .iter()
        .filter(|record| record.status == ChangeStatus::Active)
        .count();
    if active_count >= ACTIVE_CHANGE_LIMIT {
        return Err(format!(
            "active change 数已达上限（当前 {active_count}，上限 {ACTIVE_CHANGE_LIMIT}）：\
             请先归档或手动清理后再创建"
        ));
    }
    let worktree_root = vcs.worktree_root(main_root)?;
    // 前置⑨：worktree 磁盘保底门槛（db 与磁盘漂移防线：孤儿 worktree / 手动
    // 残留不占 active 额度但占磁盘。`worktree_root` 即 `.worktrees/<仓库目录
    // 名>/` 父锚，直属子目录逐目录计数；目录不存在（尚无 worktree 面）计 0，
    // 其余读错误显式 Err——误扫成 0 会绕过门槛）
    let worktree_dirs = match fs::read_dir(&worktree_root) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(format!("worktree 目录扫描失败: {error}")),
    };
    let worktree_threshold = ACTIVE_CHANGE_LIMIT + WORKTREE_DIR_SLACK;
    if worktree_dirs > worktree_threshold {
        return Err(format!(
            "worktree 目录数已达保底门槛（当前 {worktree_dirs}，门槛 {worktree_threshold}）：\
             请先以 git worktree list 对账并手动清理后再创建"
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
    // id 铸出点（D1）：前置九道全过后、建档之前直铸 uuid v7（与 created_at
    // 同段；纯 id 生成无 IO 无 vcs 调用，拒绝面零铸出）
    let id = uuid::Uuid::now_v7().to_string();
    let created_at = super::now_millis();
    let worktree_path = worktree.to_string_lossy().into_owned();
    store
        .create_change_record(ChangeStateRecord {
            id: id.clone(),
            name: name.to_owned(),
            title: title.to_owned(),
            // 工作流类型随入参落库（前置校验已收口 canonical 小写值）
            workflow_type: workflow_type.clone(),
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
    bootstrap(&worktree, project_roots, vcs, &mut warnings);

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

/// 确定性 bootstrap 分派：未配置项目根 → worktree 根单扫描（legacy，行为与
/// 文案逐字不变）；配置了项目根 → 按配置序逐个项目根扫描，命中即在该项目
/// 目录执行安装（多安装，每项目至多一次）。失败 / 未命中均只记警告，不回滚
/// create（后续确定性检查步大声失败兜底；D5 词汇单点）。
fn bootstrap(
    worktree: &Path,
    project_roots: &[String],
    vcs: &dyn WorktreePort,
    warnings: &mut Vec<String>,
) {
    if project_roots.is_empty() {
        bootstrap_worktree_root(worktree, vcs, warnings);
        return;
    }
    for root in project_roots {
        bootstrap_project(worktree, root, vcs, warnings);
    }
}

/// legacy（未配置项目根）：worktree 根探测 lockfile → 映射表首匹配命中即
/// spawn 安装（cwd = worktree 根）；无已知 lockfile → 跳过 + 注记（先判
/// Cargo 无 lock 专项注记，后落「未识别管理器」）。
fn bootstrap_worktree_root(worktree: &Path, vcs: &dyn WorktreePort, warnings: &mut Vec<String>) {
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

/// 项目模式（配置了 `projects[].root`）：worktree 内项目目录不存在 → 警告
/// 跳过；lockfile 命中 → 在该项目目录执行安装（cwd = `<worktree>/<root>`）；
/// 未命中 → 专项 / 通用注记。每项目至多一条 bootstrap 警告，失败不中断
/// 后续项目。
fn bootstrap_project(
    worktree: &Path,
    root: &str,
    vcs: &dyn WorktreePort,
    warnings: &mut Vec<String>,
) {
    let target = worktree.join(root);
    if !target.is_dir() {
        warnings.push(format!(
            "项目根 \"{root}\"：在 worktree 内不存在，跳过依赖引导"
        ));
        return;
    }
    for (lockfile, command) in BOOTSTRAP_LOCKFILE_COMMANDS {
        if !target.join(lockfile).exists() {
            continue;
        }
        match vcs.run_install(&target, command) {
            Ok(run) if run.success => {}
            Ok(run) => warnings.push(format!(
                "项目根 \"{root}\"：依赖引导失败（{command}）: {}",
                run.summary
            )),
            Err(error) => warnings.push(format!(
                "项目根 \"{root}\"：依赖引导未执行成功（{command}）: {error}"
            )),
        }
        return; // 首匹配即收口（每项目单安装）
    }
    if target.join("Cargo.toml").exists() {
        warnings.push(format!(
            "项目根 \"{root}\"：检测到 Cargo.toml 但无 Cargo.lock，跳过依赖引导（避免生成未跟踪 lockfile 混入变更上下文）"
        ));
    } else {
        warnings.push(format!("项目根 \"{root}\"：未识别依赖管理器，跳过依赖引导"));
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
