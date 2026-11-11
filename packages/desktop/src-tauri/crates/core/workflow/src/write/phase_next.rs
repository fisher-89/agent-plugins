use std::collections::HashMap;
use std::sync::Mutex;

use agent::ModelLevel;

use crate::model::{ChecklistItem, Verdict};
use crate::state::{ArtifactSpec, ChangeStateStore, OnFail, PhaseStateRecord, StepDefinition};
use crate::write::phase_table::{
    backtrack_options, phase_table, BacktrackOption, PhaseAgentSpec, PhaseDefinition,
    MAX_BACKTRACK_WINDOWS, MAX_RETRY_TIMES, MAX_ROUNDS,
};

/// 进程内会话锚点：(change_id, run_id) → 首见时 PhaseRecord 行数基线。每 run
/// 一个实例（组合根创建后注入工具步缝；run_id 键隔离，跨 run / 跨实例不共享，
/// 无进程级全局可变状态）。
#[derive(Default)]
pub struct SessionAnchors {
    anchors: Mutex<HashMap<(String, String), usize>>,
}

impl SessionAnchors {
    /// 空锚点表（每 run 组合根创建一个）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 首见登记基线、复见返回既有锚点（与插件 `getOrCreateAnchor` 同语义；
    /// 复合键身份段 = change id）。
    fn get_or_create(&self, change_id: &str, run_id: &str, entries_len: usize) -> usize {
        let mut anchors = self.anchors.lock().expect("会话锚点锁不可中毒");
        *anchors
            .entry((change_id.to_owned(), run_id.to_owned()))
            .or_insert(entries_len)
    }
}

/// 路由错误封闭集：verdict fail（未决）与重试上限标注是决策 agent 的两个
/// 唤起触发点（其余失败路径一律 `Err` 串走显式失败）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseNextError {
    /// 本相位已连续失败 N 次（窗口 fail 数达 `MAX_RETRY_TIMES`）——决策分叉
    /// 标注变体之一（与 `VerdictFail` 同一路径，仅标注不同）
    MaxRetriesExceeded {
        phase: String,
        round: u32,
    },
    VerdictFail {
        phase: String,
        round: u32,
    },
}

/// 最近一次评估条目快照（决策输入面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastResult {
    pub phase: String,
    pub verdict: Verdict,
    pub report: String,
    /// 落账时刻（UTC unix 毫秒；条目恒在位，位保留消费方可空语义）
    pub timestamp: Option<i64>,
}

/// 路由产出的已组装角色会话规格：静态角色知识主体 + 上下文头
/// `change:` + 记录 `name`（append）+ 失败反馈段 / 回溯重做窗口段（append）；
/// 跨 crate 消费方（orchestration）只消费本类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPhaseSpec {
    pub prompt: String,
    pub model_level: ModelLevel,
}

/// 一次未闭环回溯 = 一个窗口。结构面服务判据 / 堆叠 / 裁剪 / 测试断言，
/// `from_phase` / `to_phase` / 两个 index 一律不进 prompt（相位标识是流程
/// 机制的一部分，不是 agent 的领域知识）；prompt 只渲染 `reason`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacktrackWindowEntry {
    /// 发起相位（泳道位置靠后）—— 区间上界；不进 prompt
    pub from_phase: String,
    /// 发起相位的相位表下标（与 `detail.pipeline` 下标同源）；不进 prompt
    pub from_index: usize,
    /// 目标相位（泳道位置靠前）—— 区间下界；不进 prompt
    pub to_phase: String,
    /// 目标相位的相位表下标；不进 prompt
    pub to_index: usize,
    /// 标记行 `backtrack_reason` 原文逐字（不 trim / 不截断 / 不压平）；
    /// **prompt 唯一渲染字段**
    pub reason: String,
}

/// 路由产出（executor / evaluator prompt 已组装；白名单与 next 相位计划步序列
/// / 产物清单随行下发）。**路由权威不变**：本响应只为消费方供给「本相位该走哪
/// 些 step」与「本相位声明哪些产物」，walker 不自持步序（过渡仍问本函数）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseNextOutcome {
    /// 全相位 pass 走完（run 收口，不触发归档）
    pub done: bool,
    pub next_phase: Option<String>,
    /// 会话窗口轮次（窗口条目数 + 1）
    pub round: u32,
    /// next 相位的计划步序列（有序克隆；`done` / `error` 响应为空）
    pub steps: Vec<StepDefinition>,
    /// next 相位的产物清单（`done` / `error` 响应为空）
    pub artifact_manifest: Vec<ArtifactSpec>,
    pub executor: Option<ResolvedPhaseSpec>,
    pub evaluator: Option<ResolvedPhaseSpec>,
    /// 覆盖本响应对应相位的开放回溯窗口（创建序升序，最早在前）：正常响应
    /// 对 `next_phase`、error 响应对失败相位（决策输入取此处）。`Vec` 非
    /// `Option`——空集即「无窗口」，`Default` 语义与「整段省略」同义。
    pub backtrack_window: Vec<BacktrackWindowEntry>,
    /// 回溯白名单（相位 id + 表内特性描述；决策 prompt 逐条渲染供判断）
    pub allowed_backtrack_phases: Vec<BacktrackOption>,
    pub last_result: Option<LastResult>,
    pub error: Option<PhaseNextError>,
}

/// 只读路由状态机（按 change **id** 寻址）：不改状态库。路由权威唯一——
/// walker 每步过渡都问本函数，白名单随响应下发；上下文头的 change 段由记录
/// `name` 供给（id → 记录 → name 分辨率单点）。
pub fn phase_next(
    store: &dyn ChangeStateStore,
    change_id: &str,
    run_id: &str,
    anchors: &SessionAnchors,
) -> Result<PhaseNextOutcome, String> {
    if run_id.trim().is_empty() {
        return Err("missing_run_id: 缺少必需参数 run_id".to_owned());
    }
    let record = store
        .get_change(change_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("change \"{change_id}\" 未建档（无 ChangeRecord），无从路由"))?;
    let table = phase_table(&record.workflow_type).ok_or_else(|| {
        format!(
            "workflow_type \"{}\" 不受支持（支持 requirement / test-only 工作流）",
            record.workflow_type
        )
    })?;
    let entries = store
        .list_phase_records(change_id)
        .map_err(|error| error.to_string())?;
    let anchor = anchors
        .get_or_create(change_id, run_id, entries.len())
        .min(entries.len());
    let round = (entries.len() - anchor) as u32 + 1;
    let last_result = latest_result(&entries);

    if round > MAX_ROUNDS {
        return Err(format!(
            "round_limit_exceeded: 超过 {MAX_ROUNDS} 轮限制，可能存在循环回溯。请检查 \
             回溯记录，或手动清理后重试。"
        ));
    }

    // backtrack 检测：最新条目的 backtrack_to 在位 → 路由到最早有效目标
    //（旧 phase_log 机制遗留的 backtrack 条目兼容面，与插件 handleBacktrack 同语义）
    if let Some(entry) = latest_entry(&entries) {
        if let Some(target) = entry
            .backtrack_to
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
        {
            let Some(idx) = table.iter().position(|def| def.id == target) else {
                return Err(format!(
                    "invalid_backtrack_target: 回溯目标 \"{target}\" 不包含有效的 phase 标识符"
                ));
            };
            return Ok(build_phase_response(
                table,
                &table[idx],
                round,
                &record.name,
                &entries,
                last_result,
                None,
            ));
        }
    }

    // 默认路由：全 pass → done；首个未过相位 → 重试上限判定 → 正常下发。
    // archive 为表内第 9 相位（archive 列空 checklist 落账即 pass），本函数零
    // archive 特判——8 相位 pass 后下一相位即 archive（用户确认进入），archive
    // pass 后全表 `all_passed` 即 done。
    let all_passed = table.iter().all(|def| has_phase_passed(&entries, def.id));
    if all_passed {
        return Ok(PhaseNextOutcome {
            done: true,
            next_phase: None,
            round,
            steps: Vec::new(),
            artifact_manifest: Vec::new(),
            executor: None,
            evaluator: None,
            backtrack_window: Vec::new(),
            allowed_backtrack_phases: Vec::new(),
            last_result,
            error: None,
        });
    }
    let next = table
        .iter()
        .find(|def| !has_phase_passed(&entries, def.id))
        .expect("存在未过相位（all_passed 已排除空判）");
    let window_fails = entries[anchor..]
        .iter()
        .filter(|entry| entry.phase == next.id && entry.verdict == Verdict::Fail)
        .count() as u32;
    // 决策已决防重触发：该 fail 条目已进入过决策（`decision_session_id` 在案）
    // → 同一条目不再重复分叉。唯一合法重入路径为决策 backtrack 改写
    // `backtrack_to`，目标相位真执行后新落账的 fail 条目照常分叉。删掉本幂等闸
    // 后任何 fail 过的相位都无法再被执行（重做 pass 后最新 fail 条目仍会再次
    // 触发决策），直烧到 `MAX_ROUNDS` 硬失败。
    let latest_fail =
        latest_phase_entry(&entries, next.id).filter(|entry| entry.verdict == Verdict::Fail);
    let fail_decided = latest_fail.is_some_and(|entry| entry.decision_session_id.is_some());
    if window_fails >= MAX_RETRY_TIMES && !fail_decided {
        return Ok(error_outcome(
            &entries,
            table,
            round,
            last_result,
            PhaseNextError::MaxRetriesExceeded {
                phase: next.id.to_owned(),
                round,
            },
        ));
    }
    // verdict fail 立即决策分叉（带评估步或升格门禁步的相位）：决策一次的
    // 消耗低于盲目同相位重跑（executor / evaluator 会话重复执行），重试修复
    // 或回退由决策 agent 裁决。判据取步声明而非角色 spec——test-execution 为
    // 确定性相位（零角色 spec），其门禁步 `OnFail::Escalate` 即「首败交决策」
    // 的自描述。archive（无评估步、无升格步）不触发——重跑确定性步序列有效。
    // 已决条目（fail_decided）放行一次重跑；决策 backtrack 重入为唯一合法重入
    // 路径。
    let escalates = next
        .steps
        .iter()
        .any(|step| step.on_fail == Some(OnFail::Escalate));
    if (next.evaluator.is_some() || escalates) && latest_fail.is_some() && !fail_decided {
        return Ok(error_outcome(
            &entries,
            table,
            round,
            last_result,
            PhaseNextError::VerdictFail {
                phase: next.id.to_owned(),
                round,
            },
        ));
    }
    let retry_feedback =
        latest_phase_entry(&entries, next.id).filter(|entry| entry.verdict == Verdict::Fail);
    Ok(build_phase_response(
        table,
        next,
        round,
        &record.name,
        &entries,
        last_result,
        retry_feedback,
    ))
}

/// 决策分叉错误响应（两触发点共用形：不下发相位 / 步序 / 清单 / 角色 spec）。
/// 失败相位自 `PhaseNextError` 单点取出，`allowed_backtrack_phases` 为该相位的
/// `backtrack_options`（与正常路由同一推导单点——决策输入不得依赖 walker 的
/// 上一站缓存，run 首轮即分叉时无「上一站」可缓存）、`backtrack_window` 填覆盖
/// 该失败相位的开放窗口（决策输入取此处）。
fn error_outcome(
    entries: &[PhaseStateRecord],
    table: &'static [PhaseDefinition],
    round: u32,
    last_result: Option<LastResult>,
    error: PhaseNextError,
) -> PhaseNextOutcome {
    let phase = match &error {
        PhaseNextError::MaxRetriesExceeded { phase, .. }
        | PhaseNextError::VerdictFail { phase, .. } => phase,
    };
    PhaseNextOutcome {
        done: false,
        next_phase: None,
        round,
        steps: Vec::new(),
        artifact_manifest: Vec::new(),
        executor: None,
        evaluator: None,
        backtrack_window: open_backtrack_windows(entries, table, phase),
        allowed_backtrack_phases: backtrack_options(table, phase),
        last_result,
        error: Some(error),
    }
}

/// 相位是否已过（非 stale 的 pass / skipped 条目在位；与插件 `hasPhasePassed`
/// 同语义）。
fn has_phase_passed(entries: &[PhaseStateRecord], phase_id: &str) -> bool {
    entries.iter().any(|entry| {
        entry.phase == phase_id && (entry.verdict == Verdict::Pass || entry.skipped) && !entry.stale
    })
}

/// 最新条目（timestamp 降序取首；同时间戳保序取先——按落行序扫描，严格大于
/// 才替换）。
fn latest_entry(entries: &[PhaseStateRecord]) -> Option<&PhaseStateRecord> {
    let mut best: Option<&PhaseStateRecord> = None;
    for entry in entries {
        let replace = match best {
            None => true,
            Some(current) => entry.timestamp > current.timestamp,
        };
        if replace {
            best = Some(entry);
        }
    }
    best
}

/// 指定相位的最新条目（timestamp 降序取首，与 [`latest_entry`] 同判序；相位
/// 过滤前置——失败反馈只认所路由相位自身的落账，不借全局最新条目）。
fn latest_phase_entry<'a>(
    entries: &'a [PhaseStateRecord],
    phase: &str,
) -> Option<&'a PhaseStateRecord> {
    let mut best: Option<&PhaseStateRecord> = None;
    for entry in entries.iter().filter(|entry| entry.phase == phase) {
        let replace = match best {
            None => true,
            Some(current) => entry.timestamp > current.timestamp,
        };
        if replace {
            best = Some(entry);
        }
    }
    best
}

/// 最新条目快照（决策输入面）。
fn latest_result(entries: &[PhaseStateRecord]) -> Option<LastResult> {
    latest_entry(entries).map(|entry| LastResult {
        phase: entry.phase.clone(),
        verdict: entry.verdict,
        report: entry.report.clone(),
        timestamp: Some(entry.timestamp),
    })
}

/// 覆盖指定相位的开放回溯窗口（创建序升序；空集 = 无窗口）。
///
/// 三条判据（`col` = 该相位在相位表中的下标，与 `detail.pipeline` 下标、前端
/// `colIndex` 同源，MUST NOT 另引第二份相位序）：
///
/// 1. 拼接 `to_index ≤ col(P) ≤ from_index`（含源头相位 `P = from`；自环
///    `to == from` 自然退化为单点，零特判）；
/// 2. 未闭环 = 发起相位不存在 `id` 大于标记行 `id` 的条目（窗口关闭靠本判据，
///    标记行永久留在库中）；
/// 3. 堆叠 = 按标记行 `id`（落账序）升序 → 取末位 `MAX_BACKTRACK_WINDOWS` 层。
///
/// 口径分层：本函数按 **db 事件序全量条目** 计算（背景层，跨 run 直到闭环），
/// 与按 run 窗口 `entries[anchor..]` 计算的预算层（`round` / 窗口 fail 计数）
/// MUST NOT 混用——入参亦不含 `SessionAnchors` / `run_id`。
pub(crate) fn open_backtrack_windows(
    entries: &[PhaseStateRecord],
    table: &[PhaseDefinition],
    phase_id: &str,
) -> Vec<BacktrackWindowEntry> {
    let Some(col) = table.iter().position(|def| def.id == phase_id) else {
        return Vec::new();
    };
    // 中间元组：`order` 只是排序键（落账序），投影时不落结构体
    let mut hits: Vec<(i64, BacktrackWindowEntry)> = Vec::new();
    for entry in entries {
        let Some(to) = entry
            .backtrack_to
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
        else {
            continue;
        };
        let Some(from_index) = table.iter().position(|def| def.id == entry.phase) else {
            continue;
        };
        let Some(to_index) = table.iter().position(|def| def.id == to) else {
            continue;
        };
        let open = !entries
            .iter()
            .any(|later| later.phase == entry.phase && later.id > entry.id);
        if !open || col < to_index || col > from_index {
            continue;
        }
        hits.push((
            entry.id,
            BacktrackWindowEntry {
                from_phase: entry.phase.clone(),
                from_index,
                to_phase: to.to_owned(),
                to_index,
                reason: entry.backtrack_reason.clone().unwrap_or_default(),
            },
        ));
    }
    hits.sort_by_key(|(order, _)| *order);
    let dropped = hits.len().saturating_sub(MAX_BACKTRACK_WINDOWS);
    hits.into_iter()
        .skip(dropped)
        .map(|(_, window)| window)
        .collect()
}

/// 回溯重做窗口段（文案与渲染单点；经 `write/mod.rs` 再导出供 orchestration
/// 复用，零第二份副本）：空集返回空串；非空时以说明段起首 + 逐条
/// `<backtrack reason>` 定界的原因原文。前导 `\n\n` 归段内、尾随零换行（两处
/// 消费点各只需一次插值，空集分支与未引入本段前的 prompt 逐字节相同）。
///
/// 只取 `reason` 一个字段：标签行零属性、零相位 id；原因原文不截断、不压平、
/// 不 trim。
pub fn backtrack_window_section(windows: &[BacktrackWindowEntry]) -> String {
    if windows.is_empty() {
        return String::new();
    }
    let mut section = String::from(
        "\n\n## 回溯重做窗口（背景信息）\n\n\
         本 change 发生过下列相位回退，且均尚未回到闭环。这些是此前回溯留下的、\
         需要解决的问题——原因原文按发生先后排列（最上面最早），保留原始换行与\
         标点，未做截断或改写。它们是背景，不构成额外验收条件。",
    );
    for window in windows {
        section.push_str("\n\n<backtrack reason>\n");
        section.push_str(&window.reason);
        section.push_str("\n</backtrack>");
    }
    section
}

/// 失败反馈段：fail 重试时随 prompt 下发的检查结果——失败项优先（item +
/// evidence）；全 pass 清单配 fail verdict 的漂移形态全量随行不虚减（与决策
/// 输入 `build_decision_input` 同判）；清单空退回 report 全文。
fn fail_feedback_suffix(record: &PhaseStateRecord) -> String {
    let failing: Vec<&ChecklistItem> = record.checklist.iter().filter(|item| !item.pass).collect();
    let source: Vec<&ChecklistItem> = if failing.is_empty() {
        record.checklist.iter().collect()
    } else {
        failing
    };
    let items: Vec<String> = source
        .iter()
        .map(|item| format!("- {}：{}", item.item, item.evidence))
        .collect();
    let body = if items.is_empty() {
        record.report.clone()
    } else {
        items.join("\n")
    };
    format!(
        "\n\n⚠️ 上次评估未通过（attempt {}），请针对以下失败项修复：\n\n{body}",
        record.attempt
    )
}

/// 正常路由响应：上下文头（`change:` + 记录 `name`）+ 静态角色知识主体 +
/// 回溯重做窗口段（静态主体之后、失败反馈段之前）+ 失败反馈段（fail 重试时
/// 在场）+ 白名单随行（append 组装，零模板替换）。
fn build_phase_response(
    table: &'static [PhaseDefinition],
    def: &PhaseDefinition,
    round: u32,
    name: &str,
    entries: &[PhaseStateRecord],
    last_result: Option<LastResult>,
    retry_feedback: Option<&PhaseStateRecord>,
) -> PhaseNextOutcome {
    let context_header = format!("change: {name}\n\n");
    let open_windows = open_backtrack_windows(entries, table, def.id);
    let window_section = backtrack_window_section(&open_windows);
    let feedback_suffix = retry_feedback.map(fail_feedback_suffix).unwrap_or_default();
    let resolve = |spec: &PhaseAgentSpec| ResolvedPhaseSpec {
        prompt: format!(
            "{context_header}{}{window_section}{feedback_suffix}",
            spec.prompt
        ),
        model_level: spec.model_level,
    };
    PhaseNextOutcome {
        done: false,
        next_phase: Some(def.id.to_owned()),
        round,
        // next 相位计划步序列 + 产物清单随行下发（路由权威不变：walker 不自持
        // 步序 / 归属表，仅消费本响应）
        steps: def.steps.to_vec(),
        artifact_manifest: def.artifact_manifest.to_vec(),
        executor: def.executor.as_ref().map(resolve),
        evaluator: def.evaluator.as_ref().map(resolve),
        backtrack_window: open_windows,
        allowed_backtrack_phases: backtrack_options(table, def.id),
        last_result,
        error: None,
    }
}
