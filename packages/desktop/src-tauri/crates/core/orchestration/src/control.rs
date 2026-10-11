//! run 控制注册表（进程内，键 = `(workspace root, change id)` 复合——design
//! D10 修掉两 workspace 改名 / 同名前缀假冲突先例 bug，worktree 隔离解锁同
//! workspace 多 change 真并行）：并行冲突检测、cancel watch、当前会话 id 槽、ask /
//! 确认单次应答通道、`RunNotice` broadcast、重挂快照。run 生命周期由本注册
//! 表承载**在飞 run 与停等面**，节点 + 边**双累积器**驻本表条目（emit 序追
//! 加，全词汇 10 类；边四 kind 显式落边）；run 执行图落库 RunRecord /
//! RunNodeRecord / RunEdgeRecord 三表（unify-run-state-persistence 决策翻案 +
//! run-workflow-by-step 决策③：原「不建 flow_runs 表；run 终态即除名，桌面重
//! 启后 run 消失」决策随翻案显式立项修订——本注册表保持进程内零 IO 不变
//! 量，落库写缝收 walker 侧 `RunHistoryPort`，终态即除名、重启后库史可达、残
//! 留 running 经启动标定 interrupted）。发布单点：[`Self::publish`] 同步快照
//! 面并广播——walker 经 [`RunGuard::emit`] 间达，WorkerAgent 会话事件经命令
//! 层 sink 桥直发；广播侧仅发 kind-only [`RunNotice`]（载荷剥离单点，
//! `From<&RunUpdate>`），`RunUpdate` 为进程内 seam 类型。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, oneshot, watch};

use crate::state::{
    AskPayload, ChangeRunSnapshot, ChangeRunStatus, ChangeStepEdge, ChangeStepKind,
    ChangeStepState, RunNotice, RunUpdate,
};

/// broadcast 通道容量（步状态 + 会话事件窗口；溢出即滞后，由订阅侧重挂
/// 快照兜底）。
const UPDATE_CAPACITY: usize = 512;

/// 注册表条目（per-run 运行态）。
struct RunEntry {
    run_id: String,
    status: ChangeRunStatus,
    /// 当前相位（停等定位）
    phase: Option<String>,
    /// 当前 attempt
    attempt: Option<u32>,
    /// waitingAsk 载荷
    ask: Option<AskPayload>,
    /// 发起时刻（UTC unix 毫秒；`begin_run` 入参，与 RunRequest.started_at
    /// 同值——统一视图活面 startedAt 投影源）
    started_at: i64,
    /// 节点累积器（首次出现按 emit 序追加，同稳定身份键的后续 emit 就地合并
    /// ——终态 / 会话 / 输出引用覆盖 running 行；全词汇 10 类零过滤）
    nodes: Vec<ChangeStepState>,
    /// 边累积器（emit 序追加；四 kind 显式落边）
    edges: Vec<ChangeStepEdge>,
    /// 步实例号分配表（`(phase, attempt, step)` → 已分配上界；feedback 重跑为
    /// 递增新实例——节点稳定身份键的分配单点）
    instances: HashMap<(String, u32, ChangeStepKind), u32>,
    cancel: watch::Sender<bool>,
    /// 当前 WorkerAgent 会话 id 槽（停止寻址经既有 StopRegistry）
    session: Mutex<Option<String>>,
    confirm_tx: Mutex<Option<oneshot::Sender<bool>>>,
    answer_tx: Mutex<Option<oneshot::Sender<String>>>,
    updates: broadcast::Sender<RunNotice>,
}

/// run 控制注册表：命令层读写、walker 持 [`RunGuard`] 写。
#[derive(Default)]
pub struct ChangeFlowControl {
    runs: Mutex<HashMap<(String, String), RunEntry>>,
}

impl ChangeFlowControl {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 发起登记：同 `(root, change_id)` 已有 run → `Err`（并行冲突；异
    /// workspace / 异 id 不误拒）；否则登记 cancel watch 与 broadcast，返回 walker
    /// 控制柄（guard 内持注册表 Arc——命令层以 `Arc<ChangeFlowControl>` 托
    /// 管，与 StopRegistry 同型）。`started_at` 为发起时刻（命令层铸造，与
    /// `RunRequest.started_at` 同值——corpus 确定性由命令携带时间戳保证）。
    pub fn begin_run(
        self: &Arc<Self>,
        root: &str,
        change_id: &str,
        run_id: String,
        started_at: i64,
    ) -> Result<RunGuard, String> {
        let key = (root.to_owned(), change_id.to_owned());
        let mut runs = self.runs.lock().expect("run 注册表锁不可中毒");
        if runs.contains_key(&key) {
            return Err(format!(
                "change \"{change_id}\" 已有运行中的 run，不可并行发起"
            ));
        }
        let (updates, _) = broadcast::channel(UPDATE_CAPACITY);
        let (cancel, _) = watch::channel(false);
        runs.insert(
            key.clone(),
            RunEntry {
                run_id,
                status: ChangeRunStatus::Running,
                phase: None,
                attempt: None,
                ask: None,
                started_at,
                nodes: Vec::new(),
                edges: Vec::new(),
                instances: HashMap::new(),
                cancel,
                session: Mutex::new(None),
                confirm_tx: Mutex::new(None),
                answer_tx: Mutex::new(None),
                updates,
            },
        );
        drop(runs);
        Ok(RunGuard {
            key,
            control: Arc::clone(self),
        })
    }

    /// 订阅 run 状态流（`change_flow_start` / `change_flow_watch` 共用入口）；
    /// 无运行 run → `None`。
    pub fn subscribe(&self, root: &str, change_id: &str) -> Option<broadcast::Receiver<RunNotice>> {
        self.runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&(root.to_owned(), change_id.to_owned()))
            .map(|entry| entry.updates.subscribe())
    }

    /// 置 cancel 标志（停止不必先应答停等）；miss（无运行 run）幂等返回
    /// false。`send_replace` 直写 watch 槽位——无订阅者时置位不丢（`send`
    /// 在无 receiver 时是 no-op，迟滞订阅会读不到停止信号）。
    pub fn request_stop(&self, root: &str, change_id: &str) -> bool {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        match runs.get(&(root.to_owned(), change_id.to_owned())) {
            Some(entry) => {
                entry.cancel.send_replace(true);
                true
            }
            None => false,
        }
    }

    /// 当前 WorkerAgent 会话 id 槽读取（命令层停止寻址：经既有 StopRegistry
    /// 请求终止）；无槽位 → `None`。
    pub fn current_session(&self, root: &str, change_id: &str) -> Option<String> {
        self.runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&(root.to_owned(), change_id.to_owned()))
            .and_then(|entry| entry.session.lock().expect("会话槽锁不可中毒").clone())
    }

    /// 发布一条 run 状态更新（快照面同步 + kind-only Notice 广播）。发布单
    /// 点：步推进 / 边落表 / 停等态迁移 / 终态收口与 WorkerAgent 会话事件透传
    /// 同由此进。快照面突变照旧（`RunUpdate` 进程内 seam 载荷消费：节点累积
    ///（同稳定身份键就地合并）与 phase/attempt 同步、边累积、停等态迁移、终态
    /// 落值）；广播侧只发 kind-only 的 [`RunNotice`]（`From<&RunUpdate>` 投影，
    /// 载荷剥离单点，D3）。
    pub fn publish(&self, root: &str, change_id: &str, update: RunUpdate) {
        let notice = RunNotice::from(&update);
        {
            let mut runs = self.runs.lock().expect("run 注册表锁不可中毒");
            if let Some(entry) = runs.get_mut(&(root.to_owned(), change_id.to_owned())) {
                match &update {
                    RunUpdate::Step { step } => {
                        entry.phase = Some(step.phase.clone());
                        entry.attempt = Some(step.attempt);
                        accumulate_node(entry, step.clone());
                    }
                    RunUpdate::Edge { edge } => entry.edges.push(edge.clone()),
                    RunUpdate::Ask { question, options } => {
                        entry.status = ChangeRunStatus::WaitingAsk;
                        entry.ask = Some(AskPayload {
                            question: question.clone(),
                            options: options.clone(),
                        });
                    }
                    RunUpdate::ConfirmWait { phase } => {
                        entry.phase = Some(phase.clone());
                        entry.status = ChangeRunStatus::WaitingConfirm;
                    }
                    RunUpdate::Finished { status, .. } => entry.status = *status,
                    RunUpdate::SessionEvent { .. } => {}
                }
            }
        }
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        if let Some(entry) = runs.get(&(root.to_owned(), change_id.to_owned())) {
            let _ = entry.updates.send(notice);
        }
    }

    /// 当前 WorkerAgent 会话 id 槽写入（命令层 sink 桥在首个会话事件到达时
    /// 同步——adapter 事件早于 turn 收口，停止寻址由此先行可见）。
    pub fn set_session(&self, root: &str, change_id: &str, session_id: Option<String>) {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        if let Some(entry) = runs.get(&(root.to_owned(), change_id.to_owned())) {
            *entry.session.lock().expect("会话槽锁不可中毒") = session_id;
        }
    }

    /// ask 应答回流：walker 以应答文本 Continue 决策会话重出封闭集；
    /// 无运行 run / 无等待方 → `Err`。
    pub fn answer(&self, root: &str, change_id: &str, text: String) -> Result<(), String> {
        let sender = self
            .take_pending(root, change_id, |entry| &entry.answer_tx)
            .ok_or_else(|| format!("change \"{change_id}\" 无运行中的 run"))?;
        sender
            .ok_or_else(|| "当前无等待中的 ask".to_owned())?
            .send(text)
            .map_err(|_| "ask 应答通道已关闭".to_owned())
    }

    /// phase 间停等确认：proceed=false → walker 受控终态 stopped；
    /// 无运行 run / 无等待方 → `Err`。
    pub fn confirm(&self, root: &str, change_id: &str, proceed: bool) -> Result<(), String> {
        let sender = self
            .take_pending(root, change_id, |entry| &entry.confirm_tx)
            .ok_or_else(|| format!("change \"{change_id}\" 无运行中的 run"))?;
        sender
            .ok_or_else(|| "当前无等待中的 phase 确认".to_owned())?
            .send(proceed)
            .map_err(|_| "确认通道已关闭".to_owned())
    }

    /// 重挂快照查询（进程内；run 终态后除名 → `None`）。快照含发起时刻与节点 /
    /// 边累积器克隆（全词汇 emit 序 + 四 kind——重挂恢复节点与边不再恒空）。
    pub fn snapshot(&self, root: &str, change_id: &str) -> Option<ChangeRunSnapshot> {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        runs.get(&(root.to_owned(), change_id.to_owned()))
            .map(|entry| ChangeRunSnapshot {
                run_id: entry.run_id.clone(),
                status: entry.status,
                phase: entry.phase.clone(),
                attempt: entry.attempt,
                ask: entry.ask.clone(),
                started_at: entry.started_at,
                nodes: entry.nodes.clone(),
                edges: entry.edges.clone(),
            })
    }

    /// 单次应答通道取走（take 语义：单次应答，重复应答落空）；无运行 run
    /// 返回 `None`（与通道空置 `Some(None)` 可辨）。
    fn take_pending<T>(
        &self,
        root: &str,
        change_id: &str,
        slot: impl Fn(&RunEntry) -> &Mutex<Option<oneshot::Sender<T>>>,
    ) -> Option<Option<oneshot::Sender<T>>> {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        runs.get(&(root.to_owned(), change_id.to_owned()))
            .map(|entry| slot(entry).lock().expect("应答通道锁不可中毒").take())
    }
}

/// walker 持有的单 run 控制柄：emit / 当前会话槽 / 停等 / 取消观测。终态
/// 收口在 walker 主入口单点 [`RunGuard::finish`]（消费 self——终态出口唯一）。
pub struct RunGuard {
    /// 复合键（workspace root, change id）
    key: (String, String),
    control: Arc<ChangeFlowControl>,
}

impl RunGuard {
    /// 广播一条 run 状态更新（无订阅者时静默——broadcast 语义），快照面
    /// 随 [`ChangeFlowControl::publish`] 同步。
    pub fn emit(&self, update: RunUpdate) {
        self.control.publish(&self.key.0, &self.key.1, update);
    }

    /// 步实例号分配（节点稳定身份键单点）：同 `(phase, attempt, step)` 每次执行
    /// 递增——feedback 重跑 = `instance_no` 递增的独立新节点。返回本实例号。
    pub fn begin_instance(&self, phase: &str, attempt: u32, step: ChangeStepKind) -> u32 {
        let mut runs = self.control.runs.lock().expect("run 注册表锁不可中毒");
        let Some(entry) = runs.get_mut(&self.key) else {
            return 0; // 已收口（除名）：无累积面，实例号退回 0（写面不再有落点）
        };
        let counter = entry
            .instances
            .entry((phase.to_owned(), attempt, step))
            .or_insert(0);
        let instance_no = *counter;
        *counter += 1;
        instance_no
    }

    /// 节点累积器快照读取（全词汇 emit 序克隆，稳定身份键；walker 终态出口组
    /// 装 run 整包消费——本面零过滤）。
    pub fn nodes(&self) -> Vec<ChangeStepState> {
        self.control
            .runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&self.key)
            .map(|entry| entry.nodes.clone())
            .unwrap_or_default()
    }

    /// 边累积器快照读取（四 kind emit 序克隆；walker 终态出口组装 run 整包消
    /// 费）。
    pub fn edges(&self) -> Vec<ChangeStepEdge> {
        self.control
            .runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&self.key)
            .map(|entry| entry.edges.clone())
            .unwrap_or_default()
    }

    /// 当前 WorkerAgent 会话 id 槽写入。
    pub fn set_session(&self, session_id: Option<String>) {
        self.control
            .set_session(&self.key.0, &self.key.1, session_id);
    }

    /// phase 间停等：挂起至 `change_flow_confirm` 应答；等待期间取消信号
    /// 置位即以 proceed=false 收敛（停止不必先应答）。
    pub async fn wait_confirm(&self) -> bool {
        let (tx, rx) = oneshot::channel::<bool>();
        if !self.register_pending(|entry| &entry.confirm_tx, tx) {
            return false;
        }
        tokio::select! {
            proceed = rx => proceed.unwrap_or(false),
            _ = self.cancelled_wait() => false,
        }
    }

    /// ask 停等：挂起至 `change_flow_answer` 应答回流；取消信号置位 →
    /// `None`（walker 收敛 stopped）。
    pub async fn wait_answer(&self) -> Option<String> {
        let (tx, rx) = oneshot::channel::<String>();
        if !self.register_pending(|entry| &entry.answer_tx, tx) {
            return None;
        }
        tokio::select! {
            answer = rx => answer.ok(),
            _ = self.cancelled_wait() => None,
        }
    }

    /// 取消观测（同步）。
    pub fn cancelled(&self) -> bool {
        self.cancel_rx()
            .map(|mut rx| *rx.borrow_and_update())
            .unwrap_or(false)
    }

    /// 终态收口：`Finished` 广播（快照面随 publish 落终值）+ 除名。消费
    /// self——终态出口唯一（walker 主入口单点调用）。
    pub fn finish(self, status: ChangeRunStatus, reason: Option<String>) {
        self.emit(RunUpdate::Finished { status, reason });
        self.control
            .runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .remove(&self.key);
    }

    fn cancel_rx(&self) -> Option<watch::Receiver<bool>> {
        self.control
            .runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&self.key)
            .map(|entry| entry.cancel.subscribe())
    }

    /// 取消信号等待半边（watch 异步等待；除名后的迟滞等待即刻返回——调用方
    /// 随后的注册表访问自回落空路径）。
    async fn cancelled_wait(&self) {
        let Some(mut rx) = self.cancel_rx() else {
            return;
        };
        loop {
            if *rx.borrow_and_update() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// 登记单次应答发送端（注册表条目在位为前提；run 已收口 → false）。
    fn register_pending<T>(
        &self,
        slot: impl Fn(&RunEntry) -> &Mutex<Option<oneshot::Sender<T>>>,
        tx: oneshot::Sender<T>,
    ) -> bool {
        let runs = self.control.runs.lock().expect("run 注册表锁不可中毒");
        let Some(entry) = runs.get(&self.key) else {
            return false;
        };
        *slot(entry).lock().expect("应答通道锁不可中毒") = Some(tx);
        true
    }
}

/// 节点累积单点：同稳定身份键 `(phase, attempt, step, instance_no)` 的后续 emit
/// 就地合并（running → 终态 / 会话 id / 详情 / 输出引用覆写，位置保持首次 emit
/// 序），否则按 emit 序追加——同键多实例不串扰，槽位配对语义随身份键上收后端。
fn accumulate_node(entry: &mut RunEntry, step: ChangeStepState) {
    let hit = entry.nodes.iter_mut().find(|node| {
        node.phase == step.phase
            && node.attempt == step.attempt
            && node.step == step.step
            && node.instance_no == step.instance_no
    });
    match hit {
        Some(node) => {
            node.status = step.status;
            if step.session_id.is_some() {
                node.session_id = step.session_id;
            }
            if step.detail.is_some() {
                node.detail = step.detail;
            }
            if step.output_ref.is_some() {
                node.output_ref = step.output_ref;
            }
        }
        None => entry.nodes.push(step),
    }
}
