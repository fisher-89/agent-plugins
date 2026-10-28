use std::path::Path;
use std::sync::Arc;

use foundation::layout::{resolve, Layout};
use workflow::model::Verdict;
use workflow::queries::locate_change;
use workflow::state::{
    ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseStateRecord, StepCommand, StepKind,
};
use workflow::write::{phase_table, SessionAnchors};

use crate::port::{
    ArchiCheckRunner, ArchiveVcsPort, BoxToolFuture, MutationRunner, StaticCheckRunner,
    TestExecutionRunner, ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest,
};
use crate::state::ArchivePreflight;

/// 产物三件核对清单（requirement 工作流定式；markdown_doc 注册表现役名单同源）。
const ARTIFACT_FILES: [&str; 3] = ["proposal.md", "design.md", "tasks.md"];

/// 进程内工具步骤
pub struct LocalToolSteps {
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
    test_execution: Arc<dyn TestExecutionRunner>,
    /// 突变执行缝（checks 边界 infra 实现；清单 / 补丁经本消费点 vcs 缝取数注入）
    mutation: Arc<dyn MutationRunner>,
    /// 架构漂移门禁缝（`archi-runtime` 实现；变更集经本消费点 vcs 缝取数注入）
    archi: Arc<dyn ArchiCheckRunner>,
    /// 归档 git 步执行缝（Preflight 的 vcs 探测同走本缝；进程 spawn 驻
    /// `crates/infra/vcs`）
    vcs: Arc<dyn ArchiveVcsPort>,
    store: Arc<dyn ChangeStateStore>,
    run_id: String,
}

impl LocalToolSteps {
    /// 组合根装配（run 作用域一次：锚点随 run 铸新、runner / vcs 随进程复用）。
    /// 形参为组合根装配契约（四检查缝 + vcs + store + run_id），拆结构体只增间接。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        anchors: Arc<SessionAnchors>,
        static_check: Arc<dyn StaticCheckRunner>,
        test_execution: Arc<dyn TestExecutionRunner>,
        mutation: Arc<dyn MutationRunner>,
        archi: Arc<dyn ArchiCheckRunner>,
        vcs: Arc<dyn ArchiveVcsPort>,
        store: Arc<dyn ChangeStateStore>,
        run_id: String,
    ) -> Self {
        Self {
            anchors,
            static_check,
            test_execution,
            mutation,
            archi,
            vcs,
            store,
            run_id,
        }
    }
}

impl ToolStepPort for LocalToolSteps {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture {
        let anchors = Arc::clone(&self.anchors);
        let static_check = Arc::clone(&self.static_check);
        let test_execution = Arc::clone(&self.test_execution);
        let mutation = Arc::clone(&self.mutation);
        let archi = Arc::clone(&self.archi);
        let vcs = Arc::clone(&self.vcs);
        let store = Arc::clone(&self.store);
        let run_id = self.run_id.clone();
        Box::pin(async move {
            execute(
                anchors,
                static_check,
                test_execution,
                mutation,
                archi,
                vcs,
                store,
                run_id,
                step,
            )
            .await
        })
    }
}

/// 执行一次工具步骤（四检查缝 + vcs + store 的组合根入参面——参数数随
/// archi 缝扩至 9，沿 `walk_run` 先例显式 allow）
#[allow(clippy::too_many_arguments)]
async fn execute(
    anchors: Arc<SessionAnchors>,
    static_check: Arc<dyn StaticCheckRunner>,
    test_execution: Arc<dyn TestExecutionRunner>,
    mutation: Arc<dyn MutationRunner>,
    archi: Arc<dyn ArchiCheckRunner>,
    vcs: Arc<dyn ArchiveVcsPort>,
    store: Arc<dyn ChangeStateStore>,
    run_id: String,
    step: ToolStepRequest,
) -> Result<ToolStepOutput, String> {
    // 取消信号仅作字段透传（不做决策）：三分支互斥消费，无需 clone
    let cancel = step.cancel;
    match step.command {
        ToolCommand::PhaseNext { change_id, run_id } => {
            let outcome =
                workflow::write::phase_next(store.as_ref(), &change_id, &run_id, &anchors);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::PhaseNext,
                outcome.as_ref().map(|value| {
                    (
                        format!(
                            "phase_next → {}（round {}）",
                            value.next_phase.as_deref().unwrap_or("done"),
                            value.round
                        ),
                        None,
                    )
                }),
            );
            Ok(ToolStepOutput::PhaseNext(Box::new(outcome?)))
        }
        ToolCommand::PhaseStart { change_id, phase } => {
            let outcome = workflow::write::phase_start(store.as_ref(), &change_id, &phase);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::PhaseStart,
                outcome.as_ref().map(|value| {
                    (
                        format!("phase_start {} attempt={}", value.phase, value.attempt),
                        None,
                    )
                }),
            );
            Ok(ToolStepOutput::PhaseStart(outcome?))
        }
        ToolCommand::PhaseLog {
            change_id, input, ..
        } => {
            let outcome = workflow::write::phase_log(store.as_ref(), &change_id, &input);
            let reference = input
                .evaluator_session_id
                .clone()
                .or_else(|| input.executor_session_id.clone());
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::PhaseLog,
                outcome.as_ref().map(|value| {
                    (
                        format!("phase_log {} attempt={}", value.phase, value.attempt),
                        reference,
                    )
                }),
            );
            Ok(ToolStepOutput::PhaseLog(outcome?))
        }
        ToolCommand::Backtrack {
            change_id, input, ..
        } => {
            let outcome = workflow::write::backtrack(store.as_ref(), &change_id, &input);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::Backtrack,
                outcome.as_ref().map(|value| {
                    (
                        format!(
                            "backtrack {} → {}（{}）",
                            value.phase, value.target, input.reason
                        ),
                        None,
                    )
                }),
            );
            Ok(ToolStepOutput::Backtrack(outcome?))
        }
        ToolCommand::DecisionLog {
            change_id,
            phase,
            session_id,
        } => {
            let outcome =
                workflow::write::decision_log(store.as_ref(), &change_id, &phase, &session_id);
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::DecisionLog,
                outcome
                    .as_ref()
                    .map(|_| (format!("decision_log {phase}"), Some(session_id))),
            );
            Ok(ToolStepOutput::DecisionLog(outcome?))
        }
        ToolCommand::StaticCheck { change_id, slot } => {
            // 审计行 change_id 归键（命令载荷携 id——空串占位退役，逐条
            // change id 可枚举），run_id 仍串链本 run 步骤序列
            // 磁盘面 port 收 name（输出记录目录派生为 name 化、报告槽位随命令
            // 载荷透传）：消费点经 id → 记录 → name 解析（未建档显式 Err，与
            // TestExecution 分支同式）
            let name = store
                .get_change(&change_id)
                .map_err(|error| error.to_string())?
                .map(|record| record.name)
                .ok_or_else(|| {
                    format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从执行静态检查")
                })?;
            let output = static_check.run(&step.root, &name, slot, cancel).await;
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::StaticCheck,
                match &output {
                    Ok(ToolStepOutput::StaticCheck(outcome)) => {
                        Ok((format!("static_check passed={}", outcome.passed), None))
                    }
                    Ok(_) => Err("static_check 输出漂移：产出类型不匹配".to_owned()),
                    Err(error) => Err(error.clone()),
                },
            );
            output
        }
        ToolCommand::TestExecution { change_id, slot } => {
            // 磁盘面 port 收 name（报告目录派生为 name 化、报告槽位随命令载荷
            // 透传）：消费点经 id → 记录 → name 解析（未建档显式 Err）
            let name = store
                .get_change(&change_id)
                .map_err(|error| error.to_string())?
                .map(|record| record.name)
                .ok_or_else(|| {
                    format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从执行测试门禁")
                })?;
            let output = test_execution.run(&step.root, &name, slot, cancel).await;
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::TestExecution,
                match &output {
                    Ok(ToolStepOutput::TestExecution(outcome)) => Ok((
                        format!(
                            "test_execution conclusion={} total={} passed={} failed={} skipped={}",
                            outcome.conclusion.as_str(),
                            outcome.total,
                            outcome.passed,
                            outcome.failed,
                            outcome.skipped
                        ),
                        Some(outcome.report_dir.clone()),
                    )),
                    Ok(_) => Err("test_execution 输出漂移：产出类型不匹配".to_owned()),
                    Err(error) => Err(error.clone()),
                },
            );
            output
        }
        ToolCommand::MutationTest { change_id } => {
            // 磁盘面 port 收 name（报告目录派生为 name 化）：消费点经
            // id → 记录 → name 解析（未建档显式 Err，与 TestExecution 分支同式）
            let name = store
                .get_change(&change_id)
                .map_err(|error| error.to_string())?
                .map(|record| record.name)
                .ok_or_else(|| {
                    format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从执行突变门禁")
                })?;
            // 突变范围与补丁唯一入口（vcs 缝取数注入 runner——checks 侧零 git；
            // 取数失败显式传播，MUST NOT 降级空清单 / 空补丁）
            let files = vcs.change_edit_set(Path::new(&step.root), "HEAD")?;
            let patch = vcs.change_edit_patch(Path::new(&step.root), "HEAD")?;
            let output = mutation
                .run(&step.root, &name, &files, &patch, cancel)
                .await;
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::MutationTest,
                match &output {
                    Ok(ToolStepOutput::MutationTest(outcome)) => Ok((
                        format!(
                            "mutation_test conclusion={} score={} threshold={}",
                            outcome.conclusion.as_str(),
                            outcome.score,
                            outcome.threshold
                        ),
                        None,
                    )),
                    Ok(_) => Err("mutation_test 输出漂移：产出类型不匹配".to_owned()),
                    Err(error) => Err(error.clone()),
                },
            );
            output
        }
        ToolCommand::ArchiCheck { change_id } => {
            // 磁盘面 port 收 name（报告目录派生为 name 化）：消费点经 id → 记录 →
            // name 解析（未建档显式 Err，与既有三 runner 同式）
            let name = store
                .get_change(&change_id)
                .map_err(|error| error.to_string())?
                .map(|record| record.name)
                .ok_or_else(|| {
                    format!(
                        "change \"{change_id}\" 未建档（无 ChangeRecord），无从执行架构漂移门禁"
                    )
                })?;
            // 变更集唯一入口（vcs 缝取数注入 runner——infra 侧零 git；取数失败显式
            // 传播为步失败，MUST NOT 降级空清单——空清单会静默变成 noChanges
            // = 假通过）
            let changed = vcs.change_edit_set(Path::new(&step.root), "HEAD")?;
            let output = archi.run(&step.root, &name, changed, cancel).await;
            audit_outcome(
                store.as_ref(),
                &run_id,
                &change_id,
                StepKind::ArchiCheck,
                match &output {
                    Ok(ToolStepOutput::ArchiCheck(outcome)) => Ok((
                        format!("archi_check passed={}", outcome.passed),
                        Some(outcome.report_dir.clone()),
                    )),
                    Ok(_) => Err("archi_check 输出漂移：产出类型不匹配".to_owned()),
                    Err(error) => Err(error.clone()),
                },
            );
            output
        }
        // ── 归档内联两步（不写 StepRecord 审计行：归档步为执行图节点，非相位机
        //    步骤——图节点由 walker 经 RunUpdate 单点落库）──
        ToolCommand::ArchivePreflight { change_id } => {
            let payload = run_archive_preflight(
                Path::new(&step.root),
                store.as_ref(),
                &change_id,
                vcs.as_ref(),
            )
            .ok_or_else(|| {
                format!("change \"{change_id}\" 不可归档（未建档 / 已归档），无从核对归档前置")
            })?;
            Ok(ToolStepOutput::ArchivePreflight(payload))
        }
        ToolCommand::ArchiveSeal { change_id } => {
            let layout = resolve(Path::new(&step.root));
            let outcome = workflow::write::archive(&layout, store.as_ref(), &change_id)?;
            Ok(ToolStepOutput::ArchiveSeal(outcome))
        }
    }
}

/// 审计落行出口：成功行携摘要 + 引用（status `ok`），失败行携错误串（status
/// `error`）；best-effort——审计失败不阻断步本身（审计 only，不做恢复依据）。
fn audit_outcome<E: std::fmt::Display>(
    store: &dyn ChangeStateStore,
    run_id: &str,
    change_id: &str,
    step_kind: StepKind,
    outcome: Result<(String, Option<String>), E>,
) {
    let (status, summary, reference) = match outcome {
        Ok((summary, reference)) => ("ok", summary, reference),
        Err(error) => ("error", error.to_string(), None),
    };
    let _ = store.append_step(&StepCommand {
        run_id: run_id.to_owned(),
        change_id: change_id.to_owned(),
        step_kind,
        status: status.to_owned(),
        summary: clip_summary(summary),
        reference,
        timestamp: now_millis(),
    });
}

/// 摘要有界截断（design D10）：≤500 字符（`chars().count()` 口径）原样透传；
/// 超出截断前 500 字符并追加 `…（截断，共 N 字符）` 留痕（N 为原文全长）。
fn clip_summary(summary: String) -> String {
    const MAX_CHARS: usize = 500;
    let total = summary.chars().count();
    if total <= MAX_CHARS {
        return summary;
    }
    format!(
        "{}…（截断，共 {total} 字符）",
        summary.chars().take(MAX_CHARS).collect::<String>()
    )
}

/// 当前 UTC unix 毫秒（时钟早于 epoch 取 0，不 panic）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 归档前置只读载荷（store 读 + vcs 探测聚合；归档前停等面与归档相位 Preflight
// 步共用单点——自 archive_flow.rs 搬移，语义零改动）
// ---------------------------------------------------------------------------

/// 归档前置只读载荷单点：建档在案且 status=active 才 `Some`（`None` = 不可归档
/// ——未建档 / 已归档 / db 读失败的读面兜底）。`main_root` 为 layout 解析锚（主
/// 仓布局；worktree 记录在场才探测合入目标分支）。
pub(crate) fn run_archive_preflight(
    main_root: &Path,
    store: &dyn ChangeStateStore,
    id: &str,
    vcs: &dyn ArchiveVcsPort,
) -> Option<ArchivePreflight> {
    let record = store.get_change(id).ok().flatten()?;
    if record.status != ChangeStatus::Active {
        return None;
    }
    let layout = resolve(main_root);
    // workflow_type 不受支持 → completed=false + 空 incomplete（读面只出布尔结论）
    let (completed, incomplete_phases) =
        completion_of(&record, store).unwrap_or((false, Vec::new()));
    let worktree = record.worktree.as_deref();
    let name = record.name.as_str();
    let branch = worktree.map(|_| change_branch(name));
    let merge_target = match worktree {
        Some(_) => vcs.current_branch(main_root).ok(),
        None => None,
    };
    Some(ArchivePreflight {
        name: name.to_owned(),
        completed,
        incomplete_phases,
        missing_artifacts: missing_artifacts(&layout, worktree, name),
        delta_specs: detect_delta_specs(&layout, worktree, name),
        worktree: worktree.map(str::to_owned),
        branch,
        merge_target,
    })
}

/// 归档分支名单点（`branch = change/<name>`——name 恒自记录供给）。
pub(crate) fn change_branch(name: &str) -> String {
    format!("change/{name}")
}

/// 相位是否已过（非 stale 的 pass / skipped 条目在位）——`phase_next::
/// has_phase_passed` 同语义谓词的私有镜像（谓词不导出，相位机零改动红线）。
fn phase_passed(entries: &[PhaseStateRecord], phase_id: &str) -> bool {
    entries.iter().any(|entry| {
        entry.phase == phase_id && (entry.verdict == Verdict::Pass || entry.skipped) && !entry.stale
    })
}

/// 完成度核算（db PhaseRecord eval 序列即 `workflow_done` 的等价单源）：
/// `Some((completed, incomplete_phases))`；workflow_type 不受支持（相位表
/// None）→ `None`（不可核算——专项警告词汇承载，MUST NOT 读 workflow.json 或调
/// MCP `change_list`）。
fn completion_of(
    record: &ChangeStateRecord,
    store: &dyn ChangeStateStore,
) -> Option<(bool, Vec<String>)> {
    let table = phase_table(&record.workflow_type)?;
    let entries = store.list_phase_records(&record.id).unwrap_or_default();
    let incomplete: Vec<String> = table
        .iter()
        .filter(|definition| !phase_passed(&entries, definition.id))
        .map(|definition| definition.id.to_owned())
        .collect();
    Some((incomplete.is_empty(), incomplete))
}

/// 产物三件核对：定位目录内 `proposal.md` / `design.md` / `tasks.md` 在场情况
/// （缺席清单；定位失败 = 全缺）。
fn missing_artifacts(layout: &Layout, worktree: Option<&str>, name: &str) -> Vec<String> {
    let location = locate_change(layout, worktree, name);
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

/// delta specs 探测（**主仓优先、worktree 回退**）：capability 目录清单（含
/// `spec.md` 者，字典序）。
pub(crate) fn detect_delta_specs(
    layout: &Layout,
    worktree: Option<&str>,
    name: &str,
) -> Vec<String> {
    let location =
        locate_change(layout, None, name).or_else(|| locate_change(layout, worktree, name));
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

// ---------------------------------------------------------------------------
// 归档半程挡回（rebase 中间态检出）：探测在本文件、`Terminal` 收口在 walker 的
// Preflight 步——`run_archive_preflight` 被归档前停等面与 Preflight 步共用且无
// 错误通道，故挡回不能落在其中。
// ---------------------------------------------------------------------------

/// 文案字符上限（与 node detail 的 200 字符截断同宽——`run-error` 全文即节点
/// detail，无需第二个取数通道）。
pub(crate) const ARCHIVE_REASON_MAX_CHARS: usize = 200;

/// rebase 半程检出：`worktree_snapshot` 的 `rebase_head` 在场即「上一次归档失败
/// 留下的 rebase 中间态」，`Err` 挡回重跑。探测自身失败亦 `Err`（fail-closed：
/// 无法区分「半程」与「干净」时，误放行的代价是 `git add -A && git commit` 在
/// rebase 中间态产生污染）。零新 git 能力、零 spawn。
///
/// **前置**：调用方须先确认 `worktree` 目录在场（`Path::is_dir`）——目录缺失的
/// 处置归 Preflight 的既有目录存在性分支（分支已合入即照常放行，design D13），
/// 故 fail-closed 只覆盖「目录在场而探测读不出」这一种情形，文案里的两条命令也
/// 都落在确实存在的目录内。
pub(crate) fn check_rebase_halfway(vcs: &dyn ArchiveVcsPort, worktree: &str) -> Result<(), String> {
    match vcs.worktree_snapshot(Path::new(worktree)) {
        Ok(snapshot) if snapshot.rebase_head.is_some() => {
            Err(rebase_halfway_message(worktree, None))
        }
        Ok(_) => Ok(()),
        Err(error) => Err(rebase_halfway_message(worktree, Some(&error))),
    }
}

/// 半程挡回文案（三要素并列：worktree 绝对路径 / `git rebase --continue` 后重跑
/// / `git rebase --abort` 后重跑；含探测失败因时另行前置）。总长 ≤
/// [`ARCHIVE_REASON_MAX_CHARS`] 字符。
///
/// 探测失败形态不假定可进入该目录（探测失败的原因可能正是目录不可读）：先要求
/// 核对目录在场与 git 状态，再给两条出路——文案不把「进目录执行 rebase」当唯一
/// 动作。
pub(crate) fn rebase_halfway_message(worktree: &str, error: Option<&str>) -> String {
    let message = match error {
        Some(error) => format!(
            "rebase 半程探测失败（{error}），为防污染拒绝继续：worktree {worktree}；\
             请核对目录在场且 git 可读后重跑归档；若确有半程：解完模型冲突后 \
             `git rebase --continue` 再重跑归档，或 `git rebase --abort` 后重跑归档"
        ),
        None => format!(
            "上一次归档失败留下的 rebase 半程未处置：worktree {worktree}；\
             ① 在其中解完模型冲突后 `git rebase --continue` 再重跑归档；\
             ② 或 `git rebase --abort` 后重跑归档（会重新 rebase 一次）"
        ),
    };
    clip_reason(message)
}

/// 文案按字符上限截断（超限即截至上限，尾部以 `…` 留痕）。
pub(crate) fn clip_reason(message: String) -> String {
    if message.chars().count() <= ARCHIVE_REASON_MAX_CHARS {
        return message;
    }
    let end = ARCHIVE_REASON_MAX_CHARS.saturating_sub(1);
    format!("{}…", message.chars().take(end).collect::<String>())
}
