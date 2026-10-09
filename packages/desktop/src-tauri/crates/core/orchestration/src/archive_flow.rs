use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agent::{AgentEvent, AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use foundation::layout::{domain_dir_name, resolve, Layout};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{broadcast, watch};
use workflow::model::Verdict;
use workflow::queries::locate_change;
use workflow::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseStateRecord};
use workflow::write::phase_table;

use crate::port::{
    ArchiveVcsPort, RebaseOutcome, WorkerAgentPort, WorkerRole, WorkerTurnRequest, WorktreeSnapshot,
};

/// 归档链 broadcast 通道容量（阶段状态 + 会话事件窗口；溢出即滞后，由订阅
/// 侧重挂快照兜底）。
const UPDATE_CAPACITY: usize = 512;

/// 会话 provenance 来源（sourceRef = `<change>/archive/spec-sync`）。
const SOURCE_CHANGE: &str = "change";

/// 产物三件核对清单（requirement 工作流定式；markdown_doc 注册表现役名单
/// 同源）。
const ARTIFACT_FILES: [&str; 3] = ["proposal.md", "design.md", "tasks.md"];

/// 完成度警告前缀（后接「：<未过相位清单>」）。
const TXT_WARN_INCOMPLETE: &str = "工作流未全部通过";
/// 产物缺失警告前缀（后接「：<三件子集>」）。
const TXT_WARN_MISSING_ARTIFACTS: &str = "缺少产物文档";
/// 停止收敛词汇（取消旗 / 会话被终止的统一收敛面；冲突解算被停止的特例见
/// `lean_converge`——lean 串承载冲突摘要，不用本词收敛）。
const TXT_STOPPED: &str = "归档链已停止";
/// 提交段跳过因：worktree 干净（无未提交改动）。
const TXT_SKIP_CLEAN: &str = "干净";
/// 合入段跳过因：分支已合入主仓（提交段目录缺失形态同词——内容已在主仓，
/// 提交无从发生也无需发生）。
const TXT_SKIP_MERGED: &str = "已合入";
/// 同步段跳过因：无 delta specs。
const TXT_SKIP_NO_DELTA: &str = "无 delta specs";
/// 同步段跳过因：用户选择跳过同步。
const TXT_SKIP_BY_USER: &str = "用户选择";
/// 提交 / 合入段跳过因：legacy 记录（worktree=None，主仓提交面为用户手动）。
const TXT_SKIP_LEGACY: &str = "legacy 无 worktree";
/// 落盘段跳过因：归档改名已落盘（脏探测为假）。
const TXT_SKIP_FINALIZED: &str = "已落盘";
/// Merge 段 running detail 提示（冲突解算中——同段后写覆盖呈现当前会话转录）。
const TXT_MERGE_RESOLVING: &str = "合入冲突，解冲突 agent 裁决中";
/// 冲突解通过 detail（链代收口后的合入段通过摘要）。
const TXT_MERGE_RESOLVED_PREFIX: &str = "已解冲突 ";
/// lean 串原因词族（D7 六面——解冲突无法裁决的收敛原因，词汇单点驻本文件）。
const LEAN_AGENT_FAILED: &str = "agent 会话失败";
const LEAN_AGENT_STOPPED: &str = "agent 会话被停止";
const LEAN_RESIDUAL: &str = "残留冲突未解";
const LEAN_OUTSIDE_CHANGES: &str = "清单外新改动";
const LEAN_PROBE_FAILED: &str = "后验探测失败";
const LEAN_REBASE_CLOSED: &str = "agent 违约自行收口 rebase";
/// lean 串 abort 附注（收敛序中 abort 自身失败的显式面——不掩盖原记因）。
const TXT_LEAN_ABORT_FAILED: &str = "abort 未成功，请手动核验 worktree git 状态";
/// 被停止注记（冲突 agent 被 stop 的 lean 串尾注——D7 特例，不用 TXT_STOPPED
/// 收敛词，冲突摘要与手动裁决引导照常呈现）。
const TXT_LEAN_STOPPED_NOTE: &str = "（归档链已停止）";
/// `rebase_continue` 失败的收口引导（冲突解已验通过、rebase 态完整——人工
/// 续走或回退二选一，重试前半截态须人工清，链不自动 abort 已验的解算成果）。
const TXT_REBASE_CONTINUE_GUIDE: &str = "worktree 处于 rebase 态：可手动 git rebase --continue\
     续走收口（完成后主仓手动 git merge --ff-only 快进），或 git rebase --abort 回退后重试归档\
     （重试前半截态须人工清）";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveStage {
    /// 前置重校验（建档在案 + status=active + worktree 在场性）
    Preflight,
    /// worktree 全域提交（worktree 记录在场才执行；干净探测唯一跳过依据）
    Commit,
    /// 主仓合入（worktree 内 rebase 重放 branch `change/<name>` 至主仓当前分
    /// 支 → 主仓 ff-only 快进；冲突解 agent 分支内嵌，冲突态囚于 worktree）
    Merge,
    /// delta specs 同步 agent 会话（缺席 / 用户跳过则 skipped）
    SpecSync,
    /// 写面 `archive` 双写收口（改名 + db 翻转）
    Seal,
    /// 归档落盘 pathspec 提交（脏探测跳过幂等面；扩围含 delta capability 主
    /// specs 子树）
    Finalize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveStageStatus {
    /// 阶段进行中
    Running,
    /// 阶段通过
    Passed,
    /// 阶段跳过（detail 携带跳过因）
    Skipped,
    /// 阶段失败（停在该阶段）
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStageState {
    pub stage: ArchiveStage,
    pub status: ArchiveStageStatus,
    /// 跳过因（干净 / 已合入 / 无 delta specs / 用户选择 / legacy 无 worktree /
    /// 已落盘）或失败记因
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveSpecsStatus {
    /// 已同步 delta specs
    Synced,
    /// 跳过 spec 同步（用户选择）
    Skipped,
    /// 无 delta specs（摘要行照常呈现）
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSummary {
    pub name: String,
    /// 归档位置（archive 树目录名，`YYYY-MM-DD-<name>` 日期前缀形态）
    pub archived_dir: String,
    pub specs: ArchiveSpecsStatus,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "ipc", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ArchiveUpdate {
    /// 阶段状态变更（每段 running → 终态成对；同段后写覆盖）
    Stage { stage: ArchiveStageState },
    /// 归档 agent 会话事件透传（转录面板实时流；命令层 ArchiveSink 转译面）
    SessionEvent {
        session_id: String,
        event: AgentEvent,
    },
    /// 终态收口（summary 与 error 互斥——成功 / 失败停止两态）
    Finished {
        summary: Option<ArchiveSummary>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSnapshot {
    pub stages: Vec<ArchiveStageState>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePreflight {
    pub name: String,
    /// 完成度结论（workflow 相位表全相位 non-stale pass/skipped；无相位表
    /// 恒 false——警告词汇单点承载）
    pub completed: bool,
    /// 未过相位清单（workflow_type 不受支持时为空——不可核算）
    pub incomplete_phases: Vec<String>,
    /// 缺席产物文档（proposal.md / design.md / tasks.md 子集）
    pub missing_artifacts: Vec<String>,
    /// delta specs capability 清单（定位目录 specs/ 子树非空者）
    pub delta_specs: Vec<String>,
    /// worktree 记录（None = legacy 主 root change）
    pub worktree: Option<String>,
    /// change 分支名（worktree 记录在场才在案：`change/<name>`）
    pub branch: Option<String>,
    /// 主仓当前分支名（合入目标；worktree 记录在场才探测）
    pub merge_target: Option<String>,
    /// 该 change 存在运行中 run（对话呈现拒绝因）
    pub run_active: bool,
}

struct ArchiveEntry {
    /// 阶段状态累积（同段后写覆盖单槽终值）
    stages: Vec<ArchiveStageState>,
    /// 取消旗（archive_flow_stop 置位；阶段间检查点观测）
    cancel: watch::Sender<bool>,
    /// 当前归档 agent 会话 id 槽（停止寻址经既有 StopRegistry）
    session: Mutex<Option<String>>,
    updates: broadcast::Sender<ArchiveUpdate>,
}

#[derive(Default)]
pub struct ArchiveControl {
    flows: Mutex<HashMap<(String, String), ArchiveEntry>>,
}

impl ArchiveControl {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 发起登记：同 `(root, change)` 已有归档链 → `Err`（重入防护——重复点击
    /// 的幂等拒绝）；否则登记 cancel watch 与 broadcast，返回链控制柄。
    pub fn begin(self: &Arc<Self>, root: &str, change: &str) -> Result<ArchiveGuard, String> {
        let key = (root.to_owned(), change.to_owned());
        let mut flows = self.flows.lock().expect("归档注册表锁不可中毒");
        if flows.contains_key(&key) {
            return Err(format!(
                "change \"{change}\" 的归档链进行中，不可重复发起（请等待收口或先停止）"
            ));
        }
        let (updates, _) = broadcast::channel(UPDATE_CAPACITY);
        let (cancel, _) = watch::channel(false);
        flows.insert(
            key.clone(),
            ArchiveEntry {
                stages: Vec::new(),
                cancel,
                session: Mutex::new(None),
                updates,
            },
        );
        drop(flows);
        Ok(ArchiveGuard {
            key,
            control: Arc::clone(self),
        })
    }

    /// 归档链进行中判别（反向互斥面：`change_flow_start` 前置校验消费）。
    pub fn is_active(&self, root: &str, change: &str) -> bool {
        self.flows
            .lock()
            .expect("归档注册表锁不可中毒")
            .contains_key(&(root.to_owned(), change.to_owned()))
    }

    /// 订阅归档链状态流（`archive_flow_start` / `archive_flow_watch` 共用入口）；
    /// 无在案链 → `None`。
    pub fn subscribe(
        &self,
        root: &str,
        change: &str,
    ) -> Option<broadcast::Receiver<ArchiveUpdate>> {
        self.flows
            .lock()
            .expect("归档注册表锁不可中毒")
            .get(&(root.to_owned(), change.to_owned()))
            .map(|entry| entry.updates.subscribe())
    }

    /// 置 cancel 标志（停止不必等待 agent 收口）；miss（无在案链）幂等返回
    /// false（`send_replace` 直写 watch 槽位——迟滞订阅不丢停止信号）。
    pub fn request_stop(&self, root: &str, change: &str) -> bool {
        let flows = self.flows.lock().expect("归档注册表锁不可中毒");
        match flows.get(&(root.to_owned(), change.to_owned())) {
            Some(entry) => {
                entry.cancel.send_replace(true);
                true
            }
            None => false,
        }
    }

    /// 当前归档 agent 会话 id 槽读取（命令层停止寻址：经既有 StopRegistry
    /// 请求终止）；无槽位 → `None`。
    pub fn current_session(&self, root: &str, change: &str) -> Option<String> {
        self.flows
            .lock()
            .expect("归档注册表锁不可中毒")
            .get(&(root.to_owned(), change.to_owned()))
            .and_then(|entry| entry.session.lock().expect("会话槽锁不可中毒").clone())
    }

    /// 当前归档 agent 会话 id 槽写入（命令层 sink 桥在首个会话事件到达时
    /// 同步——停止寻址由此先行可见）。
    pub fn set_session(&self, root: &str, change: &str, session_id: Option<String>) {
        let flows = self.flows.lock().expect("归档注册表锁不可中毒");
        if let Some(entry) = flows.get(&(root.to_owned(), change.to_owned())) {
            *entry.session.lock().expect("会话槽锁不可中毒") = session_id;
        }
    }

    /// 发布一条归档链状态更新（快照面同步 + broadcast）：`Stage` 同段后写
    /// 覆盖（running → 终态成对，单槽终值）；`SessionEvent` / `Finished` 只
    /// 广播不动阶段面。
    pub fn publish(&self, root: &str, change: &str, update: ArchiveUpdate) {
        let mut flows = self.flows.lock().expect("归档注册表锁不可中毒");
        if let Some(entry) = flows.get_mut(&(root.to_owned(), change.to_owned())) {
            if let ArchiveUpdate::Stage { stage } = &update {
                match entry.stages.iter_mut().find(|row| row.stage == stage.stage) {
                    Some(slot) => *slot = stage.clone(),
                    None => entry.stages.push(stage.clone()),
                }
            }
            let _ = entry.updates.send(update);
        }
    }

    /// 重挂快照查询（进程内；链终态后除名 → `None`）。
    pub fn snapshot(&self, root: &str, change: &str) -> Option<ArchiveSnapshot> {
        let flows = self.flows.lock().expect("归档注册表锁不可中毒");
        flows
            .get(&(root.to_owned(), change.to_owned()))
            .map(|entry| ArchiveSnapshot {
                stages: entry.stages.clone(),
                session_id: entry.session.lock().expect("会话槽锁不可中毒").clone(),
            })
    }
}

/// 归档链持有的单链控制柄：emit / 会话槽 / 取消观测。终态收口在链主入口
/// 单点 [`ArchiveGuard::finish`]（消费 self——终态出口唯一）。
pub struct ArchiveGuard {
    /// 复合键（workspace root, change）
    key: (String, String),
    control: Arc<ArchiveControl>,
}

impl ArchiveGuard {
    /// 广播一条归档链状态更新（无订阅者时静默——broadcast 语义），快照面
    /// 随 [`ArchiveControl::publish`] 同步。
    pub fn emit(&self, update: ArchiveUpdate) {
        self.control.publish(&self.key.0, &self.key.1, update);
    }

    /// 当前归档 agent 会话 id 槽写入。
    pub fn set_session(&self, session_id: Option<String>) {
        self.control
            .set_session(&self.key.0, &self.key.1, session_id);
    }

    /// 取消观测（同步）。
    pub fn cancelled(&self) -> bool {
        self.control
            .flows
            .lock()
            .expect("归档注册表锁不可中毒")
            .get(&self.key)
            .map(|entry| *entry.cancel.borrow())
            .unwrap_or(false)
    }

    /// 终态收口：`Finished` 广播（summary 成功 / error 失败停止两态）+ 除名。
    /// 消费 self——终态出口唯一（链主入口单点调用）。
    pub fn finish(self, summary: Option<ArchiveSummary>, error: Option<String>) {
        self.emit(ArchiveUpdate::Finished { summary, error });
        self.control
            .flows
            .lock()
            .expect("归档注册表锁不可中毒")
            .remove(&self.key);
    }
}

// ---------------------------------------------------------------------------
// 读面聚合与私有镜像谓词（design D6/D7）
// ---------------------------------------------------------------------------

/// 相位是否已过（非 stale 的 pass / skipped 条目在位）——`phase_next::
/// has_phase_passed` 同语义谓词的私有镜像：相位机零改动红线（谓词不导出），
/// 等价性经 archive_flow_test 以 `phase_next` 公开 `done` 旗对拍佐证。
fn phase_passed(entries: &[PhaseStateRecord], phase_id: &str) -> bool {
    entries.iter().any(|entry| {
        entry.phase == phase_id && (entry.verdict == Verdict::Pass || entry.skipped) && !entry.stale
    })
}

/// 完成度核算（db PhaseRecord eval 序列即 `workflow_done` 的等价单源）：
/// `Some((completed, incomplete_phases))`；workflow_type 不受支持（相位表
/// None）→ `None`（不可核算——专项警告词汇承载，MUST NOT 读 workflow.json
/// 或调 MCP `change_list`）。
fn completion_of(
    record: &ChangeStateRecord,
    store: &dyn ChangeStateStore,
) -> Option<(bool, Vec<String>)> {
    let table = phase_table(&record.workflow_type)?;
    let entries = store.list_phase_records(&record.name).unwrap_or_default();
    let incomplete: Vec<String> = table
        .iter()
        .filter(|definition| !phase_passed(&entries, definition.id))
        .map(|definition| definition.id.to_owned())
        .collect();
    Some((incomplete.is_empty(), incomplete))
}

/// 产物三件核对：定位目录内 `proposal.md` / `design.md` / `tasks.md` 在场
/// 情况（缺席清单；定位失败 = 全缺）。
fn missing_artifacts(layout: &Layout, worktree: Option<&str>, change: &str) -> Vec<String> {
    let location = locate_change(layout, worktree, change);
    ARTIFACT_FILES
        .iter()
        .filter(|file| {
            !location
                .as_ref()
                .is_some_and(|loc| loc.dir.join(file).is_file())
        })
        .map(|file| (*file).to_owned())
        .collect()
}

/// delta specs 探测：**主仓优先、worktree回退**
pub(crate) fn detect_delta_specs(
    layout: &Layout,
    worktree: Option<&str>,
    change: &str,
) -> Vec<String> {
    let location =
        locate_change(layout, None, change).or_else(|| locate_change(layout, worktree, change));
    let Some(location) = location else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(location.dir.join("specs")) else {
        return Vec::new();
    };
    let mut capabilities: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter(|entry| entry.path().join("spec.md").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    capabilities.sort();
    capabilities
}

/// 归档前置读面聚合（确认对话数据面，design D6）：建档在案 + status=active
/// 门（`None` = 不可归档——未建档 / 已归档 / db 读失败的读面兜底）、完成度
/// 核算、产物三件 fs 核对、delta specs capability 清单、合入目标分支
///（worktree 记录在场才探测）。`main_root` 即 layout 解析锚（`resolve` 无
/// IO，进程内单点派生）。
pub fn preflight(
    main_root: &Path,
    store: &dyn ChangeStateStore,
    worktree: Option<&str>,
    change: &str,
    vcs: &dyn ArchiveVcsPort,
    run_active: bool,
) -> Option<ArchivePreflight> {
    let record = store.get_change(change).ok().flatten()?;
    if record.status != ChangeStatus::Active {
        return None;
    }
    let layout = resolve(main_root);
    // workflow_type 不受支持 → completed=false + 空 incomplete（专项警告由
    // 链收口 summary 承载，读面只出布尔结论）
    let (completed, incomplete_phases) =
        completion_of(&record, store).unwrap_or((false, Vec::new()));
    let branch = worktree.map(|_| format!("change/{change}"));
    let merge_target = match worktree {
        Some(_) => vcs.current_branch(main_root).ok(),
        None => None,
    };
    Some(ArchivePreflight {
        name: change.to_owned(),
        completed,
        incomplete_phases,
        missing_artifacts: missing_artifacts(&layout, worktree, change),
        delta_specs: detect_delta_specs(&layout, worktree, change),
        worktree: worktree.map(str::to_owned),
        branch,
        merge_target,
        run_active,
    })
}

// ---------------------------------------------------------------------------
// 归档 agent prompt 单点（design 数据模型逐字——spec 同步与解冲突一族两模板，
// 增量合并语义与解算纪律单源 = desktop-change-archive spec requirement 自述，
// 零 CLI skill 溯源；零 MCP / __TOOL_ASK_USER__ / workflow.json 依赖；git 面
// 收缩由模板明令——归档编排代执行收口）
// ---------------------------------------------------------------------------

/// spec 同步 prompt（D8 溯源摘除版）：增量合并语义自持（ADDED / MODIFIED /
/// REMOVED / RENAMED、保留未提及内容、幂等、capability 缺席创建）。
pub(crate) fn spec_sync_prompt(change: &str) -> String {
    format!(
        "你执行 change「{change}」归档链的 delta specs 同步段（完成度核对与归档确认已由桌面完成，无需重复）。

对每个 `openspec/changes/{change}/specs/` 下的 `<capability>/spec.md`：
1. 读 delta 与主基线 `openspec/specs/<capability>/spec.md`（主基线可能不存在）。
2. 按增量语义合并——delta 表达意图而非整体替换，保留 delta 未提及的主 spec 内容：
   - `## ADDED Requirements`：requirement 缺席则追加；已存在则更新为与 delta 一致。
   - `## MODIFIED Requirements`：只应用增量——新增 scenario、修改列中的 scenario、修订描述；不复制既有 scenario。
   - `## REMOVED Requirements`：整块移除该 requirement。
   - `## RENAMED Requirements`：把 FROM: requirement 改名为 TO:。
3. capability 主 spec 缺席时创建：简短 `## Purpose`（TBD 可）+ ADDED requirements。
4. 合并幂等：对已同步的主基线重跑本段应零变化。

约束（必须遵守）：
- 只编辑 `openspec/specs/**`；不修改 `openspec/changes/{change}/`（delta 原件由归档收口整体迁移）。
- 禁止调用 MCP 工具（change_list 等）、禁止 __TOOL_ASK_USER__、不读写 workflow.json。
- 不执行任何 git 命令——提交与合入由桌面编排代执行。
- 完成后最终消息简述各 capability 的合并动作（added / modified / removed / renamed）。"
    )
}

pub(crate) fn merge_conflict_prompt(change: &str, conflicts: &[String]) -> String {
    let list = conflicts.join("\n");
    format!(
        "你执行 change「{change}」归档链的合入冲突解算段。worktree 内重放分支 change/{change} 至主仓当前分支时以下文件冲突（冲突态已保留，rebase 进行中）：

{list}

对每个冲突文件：
1. 读文件内的冲突标记区块（<<<<<<< / ======= / >>>>>>>），结合两侧语义裁决出正确的合并结果。
2. 以裁决结果编辑该文件，移除全部冲突标记。
3. 完成后对该文件执行 `git add <该文件路径>`（逐文件精确 add）。

约束（必须遵守）：
- 只编辑上面清单内的文件；清单外任何文件一律不动（含未提交 / staged 的无关内容——它们属于用户）。
- 禁止 `git add -A` / `git add .`；禁止 `git commit` / `git merge` / `git rebase`（含 `git rebase --continue` / `git rebase --abort`）/ `git reset` / `git stash` 等收口或改写历史的命令——rebase 续走由桌面编排执行。
- 禁止调用 MCP 工具、禁止 __TOOL_ASK_USER__。
- 全部冲突文件解算并 add 后，最终消息简述各文件的裁决要点。"
    )
}

// ---------------------------------------------------------------------------
// 阶段机（design「阶段机」定形；D15：git 同步子命令与写面 archive 毫秒级
// 内联调用，分钟级面只有 agent turn）
// ---------------------------------------------------------------------------

/// 归档链发起入参。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRequest {
    /// 主 workspace root（layout 解析 / 主仓 git 命令锚）
    pub root: String,
    pub change: String,
    /// 是否执行 delta specs 同步段（确认面「跳过同步，直接归档」对译）
    pub sync_specs: bool,
}

/// 内部终态：失败 / 停止的携带物（guard 未动，finish 由主入口单点）。
enum Terminal {
    /// 失败停等（错误呈现，停在该阶段——已成功阶段零回滚）
    Failed(String),
    /// 用户停止（取消旗 / 会话被终止 → 已停止词汇收敛）
    Stopped,
}

/// 归档链主入口：走完返回终态（成功 summary / 失败停止 error）；终态收口
///（[`ArchiveGuard::finish`]）在本函数单点。`control` 随行持有（注册表生命
/// 周期由命令层 Arc 托管，此参面保设计签名对称；链全部控制面经 guard）。
pub async fn run_archive_flow(
    worker: Arc<dyn WorkerAgentPort>,
    vcs: Arc<dyn ArchiveVcsPort>,
    store: Arc<dyn ChangeStateStore>,
    control: Arc<ArchiveControl>,
    guard: ArchiveGuard,
    request: ArchiveRequest,
) {
    let (summary, error) = match drive(&worker, &vcs, &store, &guard, &request).await {
        Ok(summary) => (Some(summary), None),
        Err(Terminal::Failed(error)) => (None, Some(error)),
        Err(Terminal::Stopped) => (None, Some(TXT_STOPPED.to_owned())),
    };
    drop(control);
    guard.finish(summary, error);
}

/// 阶段机主体：线性六段（校验 → worktree 提交 → 主仓合入（含冲突 agent 分支）
/// → spec 同步 → 双写收口 → 落盘提交），任一失败停在该阶段（failed 信封 +
/// Terminal），阶段间取消旗检查点。
async fn drive(
    worker: &Arc<dyn WorkerAgentPort>,
    vcs: &Arc<dyn ArchiveVcsPort>,
    store: &Arc<dyn ChangeStateStore>,
    guard: &ArchiveGuard,
    request: &ArchiveRequest,
) -> Result<ArchiveSummary, Terminal> {
    let main_root = PathBuf::from(&request.root);
    let layout = resolve(&main_root);
    let branch = format!("change/{}", request.change);

    // ── Preflight（重校验）：建档在案 + status=active + worktree 在场性 ──
    stage_running(guard, ArchiveStage::Preflight);
    let record = store
        .get_change(&request.change)
        .map_err(|error| {
            fail_stage(
                guard,
                ArchiveStage::Preflight,
                format!("读取 change 记录失败: {error}"),
            )
        })?
        .ok_or_else(|| {
            fail_stage(
                guard,
                ArchiveStage::Preflight,
                format!(
                    "change \"{}\" 未建档（无 ChangeRecord），不可归档",
                    request.change
                ),
            )
        })?;
    if record.status != ChangeStatus::Active {
        return Err(fail_stage(
            guard,
            ArchiveStage::Preflight,
            format!(
                "change \"{}\" 已归档（status=archived），不可重复归档",
                request.change
            ),
        ));
    }
    let worktree = record.worktree.as_deref();
    // worktree 记录在场且目录缺失且分支未合入 → Err 引导（恢复目录或手动处置）
    if let Some(worktree_path) = worktree {
        if !Path::new(worktree_path).is_dir() {
            let merged = vcs.branch_merged(&main_root, &branch).map_err(|error| {
                fail_stage(
                    guard,
                    ArchiveStage::Preflight,
                    format!("分支合入判定失败: {error}"),
                )
            })?;
            if !merged {
                return Err(fail_stage(
                    guard,
                    ArchiveStage::Preflight,
                    format!(
                        "worktree 目录不存在（可能已被手动删除）: {worktree_path}；\
                         请恢复目录，或手动将分支 {branch} 合入主仓后重试归档"
                    ),
                ));
            }
        }
    }
    // 过程警告清单（摘要随行——D12 词汇单点；警告不阻断链发起）
    let mut warnings = Vec::new();
    match completion_of(&record, store.as_ref()) {
        Some((true, _)) => {}
        Some((false, incomplete)) => {
            warnings.push(format!("{TXT_WARN_INCOMPLETE}：{}", incomplete.join("、")))
        }
        None => warnings.push(format!(
            "工作流类型 \"{}\" 无相位表，完成度不可核算",
            record.workflow_type
        )),
    }
    let missing = missing_artifacts(&layout, worktree, &request.change);
    if !missing.is_empty() {
        warnings.push(format!(
            "{TXT_WARN_MISSING_ARTIFACTS}：{}",
            missing.join("、")
        ));
    }
    stage_passed(guard, ArchiveStage::Preflight, None);
    check_stop(guard)?;

    // ── Commit：干净探测唯一跳过依据（merged 短路砍除——D2：零提交分支
    // tip = 创建基线，主仓前进即令基线可达 HEAD，可达性跳过正是死法根源；
    // 桌面执行相位从不提交 → 脏即提交）──
    stage_running(guard, ArchiveStage::Commit);
    match worktree {
        None => stage_skipped(
            guard,
            ArchiveStage::Commit,
            Some(TXT_SKIP_LEGACY.to_owned()),
        ),
        Some(worktree_path) => {
            if !Path::new(worktree_path).is_dir() {
                // 目录缺失 = 内容已在主仓（Preflight 已拦「缺失且未合入」形态，
                // 此处不设第二次 merged 查询；Preflight 与本段间微秒级 TOCTOU
                //（用户恰在此窗口删目录）退化为干净跳过，非破坏性，留痕）
                stage_skipped(
                    guard,
                    ArchiveStage::Commit,
                    Some(TXT_SKIP_MERGED.to_owned()),
                );
            } else if vcs.dirty(Path::new(worktree_path), &[]) {
                vcs.commit_all(
                    Path::new(worktree_path),
                    &format!("archive: {}", request.change),
                )
                .map_err(|error| {
                    fail_stage(
                        guard,
                        ArchiveStage::Commit,
                        format!("worktree 提交失败: {error}"),
                    )
                })?;
                stage_passed(guard, ArchiveStage::Commit, None);
            } else {
                stage_skipped(guard, ArchiveStage::Commit, Some(TXT_SKIP_CLEAN.to_owned()));
            }
        }
    }
    check_stop(guard)?;

    stage_running(guard, ArchiveStage::Merge);
    match worktree {
        None => stage_skipped(guard, ArchiveStage::Merge, Some(TXT_SKIP_LEGACY.to_owned())),
        Some(worktree_path) => {
            let merged = vcs.branch_merged(&main_root, &branch).map_err(|error| {
                fail_stage(
                    guard,
                    ArchiveStage::Merge,
                    format!("分支合入判定失败: {error}"),
                )
            })?;
            if merged {
                stage_skipped(guard, ArchiveStage::Merge, Some(TXT_SKIP_MERGED.to_owned()));
            } else {
                // 重放目标 = 主仓当前分支（与快进目标同锚——detached 探测由
                // current_branch Err 面承载）
                let onto = vcs.current_branch(&main_root).map_err(|error| {
                    fail_stage(
                        guard,
                        ArchiveStage::Merge,
                        format!("合入目标分支探测失败: {error}"),
                    )
                })?;
                match vcs.rebase_branch(Path::new(worktree_path), &onto) {
                    Ok(RebaseOutcome::Rebased) => {
                        ff_advance(vcs, guard, &main_root, &branch)?;
                        stage_passed(guard, ArchiveStage::Merge, None);
                    }
                    // 非冲突失败（worktree 状态不允许等）：串即 port Err——零
                    // 加工（git 语境 + 手动处置引导已在 port 侧铸好）
                    Err(error) => return Err(fail_stage(guard, ArchiveStage::Merge, error)),
                    Ok(RebaseOutcome::Conflicted(conflicts)) => {
                        let mut resolved_total = conflicts.len();
                        let mut pending = conflicts;
                        loop {
                            match resolve_conflicts(
                                worker,
                                vcs,
                                guard,
                                request,
                                worktree_path,
                                &branch,
                                &pending,
                            )
                            .await?
                            {
                                // 重放续走又停在下一个提交（多提交分支形态）——
                                // 新清单续解
                                ResolutionOutcome::Conflicted(next) => {
                                    resolved_total += next.len();
                                    pending = next;
                                }
                                ResolutionOutcome::Rebased => break,
                            }
                        }
                        ff_advance(vcs, guard, &main_root, &branch)?;
                        stage_passed(
                            guard,
                            ArchiveStage::Merge,
                            Some(format!("{TXT_MERGE_RESOLVED_PREFIX}{resolved_total} 文件")),
                        );
                    }
                }
            }
        }
    }
    check_stop(guard)?;

    // ── SpecSync：段位后移至合入后（D8——worktree change 的 delta specs 已
    // 随合入进入主仓 active 树、legacy 本就在主仓，`locate_change` 主仓优先
    // 解析链此刻必主仓命中）：cwd 恒主 root，同步合并基线恒 = 主仓当前 specs；
    // 死法 B（sync 产物搁浅 worktree）结构性消灭──
    let delta_specs = detect_delta_specs(&layout, worktree, &request.change);
    let specs = if delta_specs.is_empty() {
        stage_running(guard, ArchiveStage::SpecSync);
        stage_skipped(
            guard,
            ArchiveStage::SpecSync,
            Some(TXT_SKIP_NO_DELTA.to_owned()),
        );
        ArchiveSpecsStatus::None
    } else if !request.sync_specs {
        stage_running(guard, ArchiveStage::SpecSync);
        stage_skipped(
            guard,
            ArchiveStage::SpecSync,
            Some(TXT_SKIP_BY_USER.to_owned()),
        );
        ArchiveSpecsStatus::Skipped
    } else {
        stage_running(guard, ArchiveStage::SpecSync);
        // 会话 cwd 恒 = 主 workspace root（worktree-cwd 分支砍除——D8）
        let turn = WorkerTurnRequest {
            root: request.root.clone(),
            prompt: spec_sync_prompt(&request.change),
            provenance: SessionProvenance {
                source: SOURCE_CHANGE.to_owned(),
                source_ref: Some(format!("{}/archive/spec-sync", request.change)),
            },
            permission: AgentPermissionMode::BypassPermissions,
            model_level: ModelLevel::High,
            continue_session: None,
            agent: None,
            role: WorkerRole::Executor,
        };
        let outcome = worker.run(turn).await.map_err(|error| {
            fail_stage(
                guard,
                ArchiveStage::SpecSync,
                format!("spec 同步会话执行失败: {error}"),
            )
        })?;
        match outcome.status {
            AgentRunStatus::Completed => {}
            // 会话被终止（archive_flow_stop 经 StopRegistry 寻址）→ 已停止收敛
            AgentRunStatus::Stopped => return Err(Terminal::Stopped),
            AgentRunStatus::Failed | AgentRunStatus::Running => {
                return Err(fail_stage(
                    guard,
                    ArchiveStage::SpecSync,
                    "spec 同步会话失败收敛（delta specs 未同步，链停同步段）".to_owned(),
                ))
            }
        }
        stage_passed(
            guard,
            ArchiveStage::SpecSync,
            Some(outcome.session_id.clone()),
        );
        ArchiveSpecsStatus::Synced
    };
    check_stop(guard)?;

    // ── Seal：写面 archive 双写单点直调（半完成重试走既有续半边）──
    stage_running(guard, ArchiveStage::Seal);
    let sealed = workflow::write::archive(&layout, store.as_ref(), &request.change)
        .map_err(|error| fail_stage(guard, ArchiveStage::Seal, error))?;
    stage_passed(
        guard,
        ArchiveStage::Seal,
        Some(sealed.archived_date.clone()),
    );

    // ── Finalize：归档落盘 pathspec 提交（脏探测跳过幂等面；扩围 D9）──
    stage_running(guard, ArchiveStage::Finalize);
    let domain = domain_dir_name();
    let active_pathspec = format!("{domain}/changes/{}", request.change);
    let archived_dir = format!("{}-{}", sealed.archived_date, request.change);
    let archived_pathspec = format!("{domain}/changes/archive/{archived_dir}");
    // pathspec 集 = 归档改名两路径 + delta capability 主 specs 子树各一段
    //（D9——与同步段探测同源零二次探测；用户跳过同步时同样扩围；全树
    // `{domain}/specs/**` 通配为禁区——无关 specs 的用户未提交编辑 MUST NOT
    // 被吞并）。子树缺席（新 capability 且跳过同步等形态）经 fs 门剔除：缺席
    // 段本就无事可做，且 git add / commit 对 unmatched pathspec fatal——缺席
    // 段留在集合内会误停落盘段。
    let mut pathspecs = vec![active_pathspec, archived_pathspec.clone()];
    for capability in &delta_specs {
        let spec_path = format!("{domain}/specs/{capability}");
        if main_root.join(&spec_path).is_dir() {
            pathspecs.push(spec_path);
        }
    }
    let pathspec_refs: Vec<&str> = pathspecs.iter().map(String::as_str).collect();
    if vcs.dirty(&main_root, &pathspec_refs) {
        vcs.commit_paths(
            &main_root,
            &pathspec_refs,
            &format!("archive: move {} to archive", request.change),
        )
        .map_err(|error| fail_stage(guard, ArchiveStage::Finalize, error))?;
        stage_passed(guard, ArchiveStage::Finalize, Some(archived_pathspec));
    } else {
        stage_skipped(
            guard,
            ArchiveStage::Finalize,
            Some(TXT_SKIP_FINALIZED.to_owned()),
        );
    }

    Ok(ArchiveSummary {
        name: request.change.clone(),
        archived_dir,
        specs,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// 合入冲突分支（design D3–D7 + D10 lean 收敛）：冲突态保留 → A 快照 → 解冲突
// agent 会话 → 后验（标记扫描 + B 快照 + 状态面对比）→ 链代收口；无法裁决
// 一律 lean 收敛（abort 尽力 + 停链咨询串）。
// ---------------------------------------------------------------------------

/// 合入冲突分支主体
enum ResolutionOutcome {
    Rebased,
    Conflicted(Vec<String>),
}

/// 主仓快进 + Merge 段通过信封（重放完成后的后半段单点——直入与冲突解算
/// 两路径共收）。
fn ff_advance(
    vcs: &Arc<dyn ArchiveVcsPort>,
    guard: &ArchiveGuard,
    main_root: &Path,
    branch: &str,
) -> Result<(), Terminal> {
    vcs.ff_merge(main_root, branch)
        .map_err(|error| fail_stage(guard, ArchiveStage::Merge, error))
}

async fn resolve_conflicts(
    worker: &Arc<dyn WorkerAgentPort>,
    vcs: &Arc<dyn ArchiveVcsPort>,
    guard: &ArchiveGuard,
    request: &ArchiveRequest,
    worktree_path: &str,
    branch: &str,
    conflicts: &[String],
) -> Result<ResolutionOutcome, Terminal> {
    let worktree = Path::new(worktree_path);
    let baseline = match vcs.worktree_snapshot(worktree) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return Err(lean_converge(
                guard,
                vcs,
                worktree,
                branch,
                conflicts,
                LEAN_PROBE_FAILED,
                false,
            ))
        }
    };

    guard.emit(ArchiveUpdate::Stage {
        stage: ArchiveStageState {
            stage: ArchiveStage::Merge,
            status: ArchiveStageStatus::Running,
            detail: Some(TXT_MERGE_RESOLVING.to_owned()),
        },
    });

    let turn = WorkerTurnRequest {
        root: worktree_path.to_owned(),
        prompt: merge_conflict_prompt(&request.change, conflicts),
        provenance: SessionProvenance {
            source: SOURCE_CHANGE.to_owned(),
            source_ref: Some(format!("{}/archive/merge-conflict", request.change)),
        },
        permission: AgentPermissionMode::BypassPermissions,
        model_level: ModelLevel::High,
        continue_session: None,
        agent: None,
        role: WorkerRole::Executor,
    };
    let outcome = match worker.run(turn).await {
        Ok(outcome) => outcome,
        Err(_) => {
            return Err(lean_converge(
                guard,
                vcs,
                worktree,
                branch,
                conflicts,
                LEAN_AGENT_FAILED,
                false,
            ))
        }
    };
    match outcome.status {
        AgentRunStatus::Completed => {}
        // 被停止特例：不用 TXT_STOPPED 收敛词——lean 串附「（归档链已停止）」
        // 注记，冲突摘要与手动裁决引导照常呈现
        AgentRunStatus::Stopped => {
            return Err(lean_converge(
                guard,
                vcs,
                worktree,
                branch,
                conflicts,
                LEAN_AGENT_STOPPED,
                true,
            ))
        }
        AgentRunStatus::Failed | AgentRunStatus::Running => {
            return Err(lean_converge(
                guard,
                vcs,
                worktree,
                branch,
                conflicts,
                LEAN_AGENT_FAILED,
                false,
            ))
        }
    }
    // ④ 后验序：残留标记 fs 扫描（locale 免疫）→ B 快照 → 状态面对比
    if !residual_markers(worktree, conflicts).is_empty() {
        return Err(lean_converge(
            guard,
            vcs,
            worktree,
            branch,
            conflicts,
            LEAN_RESIDUAL,
            false,
        ));
    }
    let after = match vcs.worktree_snapshot(worktree) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return Err(lean_converge(
                guard,
                vcs,
                worktree,
                branch,
                conflicts,
                LEAN_PROBE_FAILED,
                false,
            ))
        }
    };
    if let Err(reason) = verify_resolution(&baseline, &after, conflicts) {
        return Err(lean_converge(
            guard, vcs, worktree, branch, conflicts, &reason, false,
        ));
    }
    // ⑤ 链代续走（`rebase --continue` 沿用既定提交信息）。失败不走 lean
    // abort——冲突解已验通过、rebase 态完整，自动回退会丢弃解算成果（串附
    // 人工续走 / 回退二选一引导，半截态重试前须人工清）
    match vcs.rebase_continue(worktree) {
        Ok(RebaseOutcome::Rebased) => Ok(ResolutionOutcome::Rebased),
        Ok(RebaseOutcome::Conflicted(next)) => Ok(ResolutionOutcome::Conflicted(next)),
        Err(error) => Err(fail_stage(
            guard,
            ArchiveStage::Merge,
            format!("{error}；冲突解算已验通过但 rebase 续走失败：{TXT_REBASE_CONTINUE_GUIDE}"),
        )),
    }
}

/// lean 档无法裁决收敛（单点）：`rebase_abort` 尽力执行（worktree 恢复重放
/// 前干净态；自身 Err 附注呈现，不掩盖原记因）+ Merge 段 failed（lean 串 =
/// 原因词 + 冲突逐行清单 + abort 告知 + 手动裁决引导 + 重试幂等说明）。
fn lean_converge(
    guard: &ArchiveGuard,
    vcs: &Arc<dyn ArchiveVcsPort>,
    worktree: &Path,
    branch: &str,
    conflicts: &[String],
    reason: &str,
    stopped_note: bool,
) -> Terminal {
    let abort_note = match vcs.rebase_abort(worktree) {
        Ok(()) => String::new(),
        Err(_) => format!("（{TXT_LEAN_ABORT_FAILED}）"),
    };
    let mut message = format!(
        "合入冲突无法自动裁决（{reason}）。冲突文件 {} 个：{}\n已执行 git rebase --abort \
         恢复 worktree 干净态{abort_note}。请手动将分支 {branch} 合入主仓（在 worktree 内自行\
         rebase 解冲突，或主仓手动 merge）后重试归档——重试将识别已合入并续走收口。",
        conflicts.len(),
        conflicts.join("\n"),
    );
    if stopped_note {
        message.push_str(TXT_LEAN_STOPPED_NOTE);
    }
    fail_stage(guard, ArchiveStage::Merge, message)
}

/// 后验状态面对比（纯函数）
pub(crate) fn verify_resolution(
    baseline: &WorktreeSnapshot,
    after: &WorktreeSnapshot,
    conflicts: &[String],
) -> Result<(), String> {
    // ① REBASE_HEAD 在场性（rebase 半程判别——agent 违约收口 / 中止的捕获面）
    if after.rebase_head.is_none() {
        let baseline_head = &baseline.head;
        return Err(format!(
            "{LEAN_REBASE_CLOSED}——REBASE_HEAD 缺席（rebase 已被收口或中止），\
             请手动核验 worktree git 状态（基线 HEAD {baseline_head}）后重试"
        ));
    }
    let conflict_set: std::collections::HashSet<&str> =
        conflicts.iter().map(String::as_str).collect();
    // ②③ 冲突路径：无 stage>0 残留条目；worktree 干净（y=' '）且索引单条
    // stage-0（或已删除缺席）——全量 staged 才可收口
    for path in conflicts {
        let unmerged = after
            .index
            .iter()
            .any(|entry| entry.path == *path && entry.stage > 0);
        let worktree_dirty = after
            .status
            .iter()
            .any(|entry| entry.path == *path && entry.y != ' ');
        let stage_zero = after
            .index
            .iter()
            .filter(|entry| entry.path == *path && entry.stage == 0)
            .count();
        if unmerged || worktree_dirty || stage_zero > 1 {
            return Err(LEAN_RESIDUAL.to_owned());
        }
    }
    // ④ 非 C 路径逐字一致（porcelain XY 与索引 mode/hash/stage 双面对拍）
    let mut status_a: Vec<(&str, char, char)> = baseline
        .status
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| (entry.path.as_str(), entry.x, entry.y))
        .collect();
    status_a.sort_unstable();
    let mut status_b: Vec<(&str, char, char)> = after
        .status
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| (entry.path.as_str(), entry.x, entry.y))
        .collect();
    status_b.sort_unstable();
    let mut index_a: Vec<(&str, &str, &str, u8)> = baseline
        .index
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| {
            (
                entry.path.as_str(),
                entry.mode.as_str(),
                entry.hash.as_str(),
                entry.stage,
            )
        })
        .collect();
    index_a.sort_unstable();
    let mut index_b: Vec<(&str, &str, &str, u8)> = after
        .index
        .iter()
        .filter(|entry| !conflict_set.contains(entry.path.as_str()))
        .map(|entry| {
            (
                entry.path.as_str(),
                entry.mode.as_str(),
                entry.hash.as_str(),
                entry.stage,
            )
        })
        .collect();
    index_b.sort_unstable();
    if status_a != status_b || index_a != index_b {
        return Err(LEAN_OUTSIDE_CHANGES.to_owned());
    }
    // ④ 续：B 无 A 缺席的新路径（agent 新建 / 新 add 即违约）；消失路径 ⊆
    // 冲突清单（冲突路径解算后收敛至与 HEAD 一致而离开 status 是合法消失）
    let paths_a: std::collections::HashSet<&str> = baseline
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    if let Some(new_path) = after
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .find(|path| !paths_a.contains(path))
    {
        return Err(format!("{LEAN_OUTSIDE_CHANGES}: {new_path}"));
    }
    let paths_b: std::collections::HashSet<&str> = after
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    if let Some(gone_path) = baseline
        .status
        .iter()
        .map(|entry| entry.path.as_str())
        .find(|path| !paths_b.contains(path) && !conflict_set.contains(path))
    {
        return Err(format!("{LEAN_OUTSIDE_CHANGES}: {gone_path}"));
    }
    Ok(())
}

/// 冲突标记 fs 扫描（D4，locale 免疫）：逐清单文件逐行扫描行首 `<<<<<<< ` /
/// `>>>>>>> ` 形态，返回 `文件:行` 命中清单；读取失败 / 文件缺席按无标记处理
///（fs 缺席形态由后验状态面对比承载）。
pub(crate) fn residual_markers(root: &Path, conflicts: &[String]) -> Vec<String> {
    let mut hits = Vec::new();
    for path in conflicts {
        let Ok(content) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        for (offset, line) in content.lines().enumerate() {
            if line.starts_with("<<<<<<< ") || line.starts_with(">>>>>>> ") {
                hits.push(format!("{path}:{}", offset + 1));
            }
        }
    }
    hits
}

/// 阶段 running 信封发布。
fn stage_running(guard: &ArchiveGuard, stage: ArchiveStage) {
    guard.emit(ArchiveUpdate::Stage {
        stage: ArchiveStageState {
            stage,
            status: ArchiveStageStatus::Running,
            detail: None,
        },
    });
}

/// 阶段 passed 信封发布（detail 携带通过摘要）。
fn stage_passed(guard: &ArchiveGuard, stage: ArchiveStage, detail: Option<String>) {
    guard.emit(ArchiveUpdate::Stage {
        stage: ArchiveStageState {
            stage,
            status: ArchiveStageStatus::Passed,
            detail,
        },
    });
}

/// 阶段 skipped 信封发布（detail 携带跳过因——词汇单点）。
fn stage_skipped(guard: &ArchiveGuard, stage: ArchiveStage, detail: Option<String>) {
    guard.emit(ArchiveUpdate::Stage {
        stage: ArchiveStageState {
            stage,
            status: ArchiveStageStatus::Skipped,
            detail,
        },
    });
}

/// 阶段失败：failed 信封（detail 携错误）+ 终止携带物（停在该阶段）。
fn fail_stage(guard: &ArchiveGuard, stage: ArchiveStage, error: String) -> Terminal {
    guard.emit(ArchiveUpdate::Stage {
        stage: ArchiveStageState {
            stage,
            status: ArchiveStageStatus::Failed,
            detail: Some(error.clone()),
        },
    });
    Terminal::Failed(error)
}

/// 阶段间取消检查点：取消旗置位 → 已停止词汇收敛。
fn check_stop(guard: &ArchiveGuard) -> Result<(), Terminal> {
    if guard.cancelled() {
        Err(Terminal::Stopped)
    } else {
        Ok(())
    }
}
