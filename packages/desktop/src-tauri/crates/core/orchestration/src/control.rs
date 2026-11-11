use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foundation::cancel::CancelFlag;
use tokio::sync::{broadcast, oneshot, watch};

use crate::state::{
    ArchivePreflight, AskPayload, ChangeRunSnapshot, ChangeRunStatus, ChangeStepEdge,
    ChangeStepKind, ChangeStepState, ChangeStepStatus, RunNotice, RunUpdate,
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
    /// waitingConfirm 且停等点为 archive 前时的归档前置载荷（其余停等点 /
    /// 非停等态恒 None）
    archive: Option<ArchivePreflight>,
    /// 发起时刻（UTC unix 毫秒；`begin_run` 入参，与 RunRequest.started_at
    /// 同值——统一视图活面 startedAt 投影源）
    started_at: i64,
    /// 节点累积器（首次出现按 emit 序追加，同稳定身份键的后续 emit 就地合并
    /// ——终态 / 会话 / 输出引用覆盖 running 行；全词汇 18 类零过滤）
    nodes: Vec<ChangeStepState>,
    /// 边累积器（emit 序追加；四 kind 显式落边）
    edges: Vec<ChangeStepEdge>,
    /// 步实例号分配表（`(phase, attempt, step)` → 已分配上界；feedback 重跑为
    /// 递增新实例——节点稳定身份键的分配单点）
    instances: HashMap<(String, u32, ChangeStepKind), u32>,
    cancel: watch::Sender<bool>,
    /// 进程面取消标志（与 `cancel` watch 同道：watch 供停等 / 应答等待的异步观测，
    /// 本标志供执行体 `exec::run_shell` 轮询；无订阅者也不丢位）
    cancel_flag: CancelFlag,
    /// 当前 WorkerAgent 会话 id 槽（停止寻址经既有 StopRegistry）
    session: Mutex<Option<String>>,
    confirm_tx: Mutex<Option<oneshot::Sender<(bool, bool)>>>,
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
                archive: None,
                started_at,
                nodes: Vec::new(),
                edges: Vec::new(),
                instances: HashMap::new(),
                cancel,
                // 进程面取消标志（恒 false 起步；`request_stop` 置位，run 除名后
                // 标志随 Arc 存活至最后一个持有者释放——在跑的 runner 仍能读到）
                cancel_flag: CancelFlag::never(),
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
    /// false。**双写**：
    /// - `send_replace` 直写 watch 槽位——无订阅者时置位不丢（`send` 在无
    ///   receiver 时是 no-op，迟滞订阅会读不到停止信号）；停等 / 应答等待的
    ///   异步观测面语义不变；
    /// - `cancel_flag.handle().set()` 置进程面标志（执行体 200 ms 轮询观测，
    ///   使运行中的检查域 spawn 可被杀树收敛）。
    ///
    /// 两次写均幂等（同刻连续停止不报错、不丢位）。
    pub fn request_stop(&self, root: &str, change_id: &str) -> bool {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        match runs.get(&(root.to_owned(), change_id.to_owned())) {
            Some(entry) => {
                entry.cancel.send_replace(true);
                entry.cancel_flag.handle().set();
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
    ///
    /// 会话事件首拍另将 session id 提前挂到当前运行中的会话型步节点并补发一
    /// 拍 `Step` 通知——running emit 不携 id、终态 emit 才回写，提前挂载后运行
    /// 中节点即可按 id 直查实时转录（desktop-change-flow-view「运行中节点实时
    /// 转录经转录库」）。
    pub fn publish(&self, root: &str, change_id: &str, update: RunUpdate) {
        let notice = RunNotice::from(&update);
        let mut attached = false;
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
                    RunUpdate::ConfirmWait { phase, archive } => {
                        entry.phase = Some(phase.clone());
                        entry.status = ChangeRunStatus::WaitingConfirm;
                        entry.archive = archive.clone();
                    }
                    RunUpdate::Finished { status, .. } => entry.status = *status,
                    RunUpdate::SessionEvent { session_id, .. } => {
                        attached = attach_session(entry, session_id);
                    }
                }
            }
        }
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        if let Some(entry) = runs.get(&(root.to_owned(), change_id.to_owned())) {
            // 首挂补发 Step（统一视图重查触发——前端拿到 session id）；后续
            // 会话事件仅按原 kind（SessionEvent）广播
            if attached {
                let _ = entry.updates.send(RunNotice::Step);
            }
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

    /// ask 应答回流：walker 以应答文本 Continue 决策会话重出封闭集；无运行
    /// run / 无等待方 → `Err`。通道 take 成功即视为「应答受理」——停等态解除
    /// 先于 send（`Finished` 可能被唤醒后抢先 emit，清理若落在其后会把终态倒
    /// 灌回运行中），send 失败（walker 已退出）仍按受理返回 Ok 并广播失效信号。
    ///
    /// 失效信号复用 `RunNotice::Step`（既有「状态已变、请重查」通用 kind）：
    /// `Ask` / `ConfirmWait` 在通知面被读作「新停等已开始」（前端不分流，但
    /// 测试与外部订阅以此 kind 驱动应答回路——重放同 kind 会让订阅方对已解除
    /// 的等待再答一次）。
    pub fn answer(&self, root: &str, change_id: &str, text: String) -> Result<(), String> {
        let (sender, updates) = self.accept_pending(
            root,
            change_id,
            |entry| &entry.answer_tx,
            ChangeRunStatus::WaitingAsk,
            "当前无等待中的 ask",
        )?;
        let _ = sender.send(text);
        let _ = updates.send(RunNotice::Step);
        Ok(())
    }

    /// phase 间停等确认：proceed=false → walker 受控终态 stopped；`sync_specs`
    /// 为归档前停等点的「是否执行 delta specs 同步」选择回流（phase 间停等点
    /// 置位值不影响；archive 前停等点消费）；无运行 run / 无等待方 → `Err`。
    /// 受理语义与 [`Self::answer`] 同形（先清停等态、后 send、失败仍算受理）。
    pub fn confirm(
        &self,
        root: &str,
        change_id: &str,
        proceed: bool,
        sync_specs: bool,
    ) -> Result<(), String> {
        let (sender, updates) = self.accept_pending(
            root,
            change_id,
            |entry| &entry.confirm_tx,
            ChangeRunStatus::WaitingConfirm,
            "当前无等待中的 phase 确认",
        )?;
        let _ = sender.send((proceed, sync_specs));
        let _ = updates.send(RunNotice::Step);
        Ok(())
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
                archive: entry.archive.clone(),
                started_at: entry.started_at,
                nodes: entry.nodes.clone(),
                edges: entry.edges.clone(),
            })
    }

    /// 单次应答受理（take 语义：单次应答，重复应答落空）：同一把 `runs` 锁内
    /// 完成「取通道 + 解除停等态」，锁外由调用方 send 与广播。无运行 run →
    /// `Err`（与通道空置可辨）。
    ///
    /// 解除条件是「当前 status 恰为该停等态」——`Finished` 已先到时条件不成立，
    /// 终态不被倒灌回 `Running`。`ask` / `archive` 槽与 `status` 是三个独立出线
    /// 判据（徽章读 status、卡片读载荷槽），三处同刻清空，留任一即残留。
    fn accept_pending<T>(
        &self,
        root: &str,
        change_id: &str,
        slot: impl Fn(&RunEntry) -> &Mutex<Option<oneshot::Sender<T>>>,
        waiting: ChangeRunStatus,
        missing: &str,
    ) -> Result<(oneshot::Sender<T>, broadcast::Sender<RunNotice>), String> {
        let mut runs = self.runs.lock().expect("run 注册表锁不可中毒");
        let entry = runs
            .get_mut(&(root.to_owned(), change_id.to_owned()))
            .ok_or_else(|| format!("change \"{change_id}\" 无运行中的 run"))?;
        let sender = slot(entry)
            .lock()
            .expect("应答通道锁不可中毒")
            .take()
            .ok_or_else(|| missing.to_owned())?;
        if entry.status == waiting {
            entry.status = ChangeRunStatus::Running;
            entry.ask = None;
            entry.archive = None;
        }
        Ok((sender, entry.updates.clone()))
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

    /// phase 间停等：挂起至 `change_flow_confirm` 应答；等待期间取消信号置位
    /// 即以 `(false, false)` 收敛（停止不必先应答）。返回 `(proceed, sync_specs)`
    /// ——`sync_specs` 仅归档前停等点消费（是否执行 delta specs 同步）。
    pub async fn wait_confirm(&self) -> (bool, bool) {
        let (tx, rx) = oneshot::channel::<(bool, bool)>();
        if !self.register_pending(|entry| &entry.confirm_tx, tx) {
            return (false, false);
        }
        tokio::select! {
            answer = rx => answer.unwrap_or((false, false)),
            _ = self.cancelled_wait() => (false, false),
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

    /// 进程面取消标志读取（克隆共享同一底层原子——供 `run_tool` 单点填充
    /// `ToolStepRequest.cancel`，执行体以 200 ms 轮询观测）。run 已除名（终态
    /// 收口后）回落 [`CancelFlag::never`]——与 `nodes()` / `edges()` /
    /// `cancelled()` 的「除名即空路径」纪律同型，不 panic 不 `Err`。
    pub fn cancel_flag(&self) -> CancelFlag {
        self.control
            .runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&self.key)
            .map(|entry| entry.cancel_flag.clone())
            .unwrap_or_else(CancelFlag::never)
    }

    /// `'static` 步状态发射器（持 `(root, change_id)` 复合键 + `Arc<ChangeFlowControl>`）：
    /// 供跨 port 边界（`ToolStepRequest.on_output_ref`）的回调持有——回调不能借用
    /// `&RunGuard`，故以本方法重建同身份的发射器。
    pub fn emitter(&self) -> StepEmitter {
        StepEmitter {
            key: self.key.clone(),
            control: Arc::clone(&self.control),
        }
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

/// 步状态行落值（emit 载荷收窄：状态 + 三可选面 + 步级时间与 token 载荷镜像面）。
/// token 两值为数据随行面（`run_worker` 经 [`StepSettle::with_tokens`] 落值、其余
/// 步不设即 None）；`started_at` / `ended_at` 两时间字段由 [`emit_node`] 单点铸造
/// 覆写——构造器恒中性值（0 / None），MUST NOT 由调用方携带时间。
pub(crate) struct StepSettle {
    status: ChangeStepStatus,
    session_id: Option<String>,
    detail: Option<String>,
    output_ref: Option<String>,
    started_at: i64,
    ended_at: Option<i64>,
    token_input: Option<u64>,
    token_output: Option<u64>,
}

impl StepSettle {
    /// 步进行中（无会话 / 详情 / 输出引用；时间与 token 恒中性——时间由
    /// `emit_node` 铸造覆写）。
    pub(crate) fn running() -> Self {
        Self {
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: 0,
            ended_at: None,
            token_input: None,
            token_output: None,
        }
    }

    /// 步通过（携人读摘要）。
    pub(crate) fn passed(detail: Option<String>) -> Self {
        Self {
            status: ChangeStepStatus::Passed,
            detail,
            ..Self::running()
        }
    }

    /// 步失败（携失败记因）。
    pub(crate) fn failed(detail: Option<String>) -> Self {
        Self {
            status: ChangeStepStatus::Failed,
            detail,
            ..Self::running()
        }
    }

    /// 步因停止 / 终止收敛。
    pub(crate) fn stopped(detail: Option<String>) -> Self {
        Self {
            status: ChangeStepStatus::Stopped,
            detail,
            ..Self::running()
        }
    }

    /// 随行会话 id（WorkerAgent 步）。
    pub(crate) fn with_session(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// 随行全量输出引用（`ExecOutput::Recorded { exec_id }`）。
    pub(crate) fn with_output_ref(mut self, output_ref: Option<String>) -> Self {
        self.output_ref = output_ref;
        self
    }

    /// 随行本步 token 增量（in / out 两值；仅 WorkerAgent 步终态落值，缺席
    /// None）——数据随行面，时间字段不在本 builder 语义内。
    pub(crate) fn with_tokens(
        mut self,
        token_input: Option<u64>,
        token_output: Option<u64>,
    ) -> Self {
        self.token_input = token_input;
        self.token_output = token_output;
        self
    }
}

/// 步状态行 emit（节点累积 + Notice 广播；同稳定身份键的后续 emit 就地合并）。
/// 步级时间铸造收本单点：全部调用点零触点覆盖——running emit 铸
/// `(now, None)`、终态 emit 铸 `(now, Some(now))`（`phaseStart` 等单 emit 瞬步
/// 自然 `started_at == ended_at` 同刻）；该 started 同刻值是否被采信由
/// [`accumulate_node`] 合并侧裁定（started 保首见）。复用 [`now_millis`] 零新时钟缝
/// ——收口时刻铸造只在生产路径消费，corpus 确定性由测试直构 `ChangeStepState` 注入
/// 定值承载。
pub(crate) fn emit_node(
    guard: &RunGuard,
    phase: &str,
    attempt: u32,
    step: ChangeStepKind,
    instance_no: u32,
    mut settle: StepSettle,
) {
    let now = now_millis();
    settle.started_at = now;
    settle.ended_at = (settle.status != ChangeStepStatus::Running).then_some(now);
    guard.emit(RunUpdate::Step {
        step: ChangeStepState {
            phase: phase.to_owned(),
            attempt,
            step,
            instance_no,
            status: settle.status,
            session_id: settle.session_id,
            detail: settle.detail,
            output_ref: settle.output_ref,
            started_at: settle.started_at,
            ended_at: settle.ended_at,
            token_input: settle.token_input,
            token_output: settle.token_output,
        },
    });
}

/// 当前 UTC unix 毫秒（时钟早于 epoch 取 0，不 panic）——run 收口时刻铸造点
///（finish 整包同刻，corpus 确定性由测试注入 `started_at` / 假 history 承载，本
/// 时钟只在生产路径消费）。
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// `'static` 步状态发射器：`(root, change_id)` 复合键 + 注册表 `Arc`，唯一构造面
/// 是 [`RunGuard::emitter`]。
///
/// `'static` 约束是硬性的——上抛回调跨 port 边界（`ToolStepRequest.on_output_ref`），
/// 不能借用 `&RunGuard`。
#[derive(Clone)]
pub struct StepEmitter {
    key: (String, String),
    control: Arc<ChangeFlowControl>,
}

impl StepEmitter {
    /// 补发一拍 `running + output_ref` 的步状态：经同一铸造单点 [`emit_node`] 落值
    ///（不新增第二条构造 `ChangeStepState` 的路径）。`accumulate_node` 的「some 才
    /// 覆写」直接吃掉该拍——不新增节点、不动 `started_at`。
    pub fn attach_output_ref(
        &self,
        phase: &str,
        attempt: u32,
        step: ChangeStepKind,
        instance_no: u32,
        exec_id: &str,
    ) {
        let guard = RunGuard {
            key: self.key.clone(),
            control: Arc::clone(&self.control),
        };
        emit_node(
            &guard,
            phase,
            attempt,
            step,
            instance_no,
            StepSettle::running().with_output_ref(Some(exec_id.to_owned())),
        );
    }
}

/// 会话事件首拍把 session id 提前挂到「最近一个运行中的会话型步节点」：
/// running emit 不携会话 id（worker 收口后终态 emit 才回写），不提前挂载则
/// 运行中节点无法按 id 直查实时转录。返回是否首次挂载（调用方据此补发一拍
/// `Step` 通知触发统一视图重查；后续会话事件不重复挂载 / 不重复通知）。
fn attach_session(entry: &mut RunEntry, session_id: &str) -> bool {
    let target = entry.nodes.iter_mut().rev().find(|node| {
        node.status == ChangeStepStatus::Running
            && node.session_id.is_none()
            && is_session_step(node.step)
    });
    let Some(node) = target else {
        return false;
    };
    node.session_id = Some(session_id.to_owned());
    true
}

/// 会话型步词汇判定（WorkerAgent 五类，与 `roleOf` / `worker` 通道同集）。
fn is_session_step(step: ChangeStepKind) -> bool {
    matches!(
        step,
        ChangeStepKind::Executor
            | ChangeStepKind::Evaluator
            | ChangeStepKind::Decision
            | ChangeStepKind::SpecSync
            | ChangeStepKind::ResolveConflicts
    )
}

/// 节点累积单点：同稳定身份键 `(phase, attempt, step, instance_no)` 的后续 emit
/// 就地合并（running → 终态 / 会话 id / 详情 / 输出引用覆写；起止与 token——
/// `started_at` 保首见、`ended_at` / token some 才覆写，位置保持首次 emit 序），
/// 否则按 emit 序追加——同键多实例不串扰，槽位配对语义随身份键上收后端。
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
            // `started_at` 零赋值 = 无条件保首见：running emit 落值后不被终态
            // emit 顶替（终态行携带的同刻 started 值被丢弃）；瞬步单 emit 首见
            // 即真值（started == ended 自然成立）
            if step.session_id.is_some() {
                node.session_id = step.session_id;
            }
            if step.detail.is_some() {
                node.detail = step.detail;
            }
            if step.output_ref.is_some() {
                node.output_ref = step.output_ref;
            }
            if step.ended_at.is_some() {
                node.ended_at = step.ended_at;
            }
            if step.token_input.is_some() {
                node.token_input = step.token_input;
            }
            if step.token_output.is_some() {
                node.token_output = step.token_output;
            }
        }
        None => entry.nodes.push(step),
    }
}
