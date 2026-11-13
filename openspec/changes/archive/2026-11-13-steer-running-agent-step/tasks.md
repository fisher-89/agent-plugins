# 任务: steer-running-agent-step

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 19 项（1 新增文件 + 18 修改面，含 1 个生成物）+ 机械改动（`SessionKernel::new` 57 处 / `AgentSession` 字面构造 7 处 / `loop::run` 21 处 / `compose_turn` 52 处，见 design「编译闭合批」与「事实校准 1–4」）。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：proposal「测试文件」表逐行的新增 / 改造用例归 test-design / test-gen / test-execution 相位；`cargo test` / `pnpm -C packages/desktop test` 等回归由桌面端 test-execution 相位承接。本相位对测试文件的唯一动作是**让工作区随签名变化编译通过**（`SessionKernel::new(…, steer_registry)` / `AgentSession { …, steers }` / `loop::run(…, steers, …)` / `compose_turn(…, steer_registry, …)`），MUST NOT 新增用例、MUST NOT 改断言口径。
>
> **依赖序（五阶段）**：
>
> - **阶段一**（core/agent 会话地基：`steer.rs` + `AskRegistry::is_pending` + `AgentSession` 第四件套 + 内核登记 / 除名 + 出口 + 生产 `AgentSession` 构造点随动）**破坏性**，编译期全暴露；它是全部后续阶段的底座（注册表类型 + 通道字段 + 五参内核三者同源）。
> - **阶段二**（infra/agent 通道接线 + 泵透传 + 请求边界 drain + 组合根注入）依赖阶段一（`SteerRegistry` / `SteerMessage` / `AgentSession.steers` / 五参 `SessionKernel::new`）。
> - **阶段三**（壳层托管 + `agent_steer` + 两处 `compose_turn` 实参 + 命令登记）依赖阶段二（`compose_turn` 新实参存在）。
> - **阶段四**（前端：生成物 + headless 动作面 + 适配层多 user 规则 + composer 两可选位 + 抽屉接线）依赖阶段三（命令登记驱动生成物），Rust 面零依赖，可先做组件再取生成物。
> - **阶段五**（版本 + 边界自查 + 交接备忘）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ⓪ **不合并 / 不排队纪律**：drain 必须用 `try_recv` **循环**一次边界取净，每条独立密封事件 + 独立 `Message::user` + 独立 `ack.send(())`（MUST NOT 合并成一条、MUST NOT 只 `recv` 一条让后几条滞留到下个边界）；MUST NOT 在 `execute_ask` 的 `select!` 加 steer 臂（等于把「不排队」改成「缓冲到工具结果后落下」）；MUST NOT 收口后补投（零 Continue、零第二轮）。
> ① **逐字不动面**（`git diff` 出现即越界）：`crates/core/agent/src/ask.rs` 的 `register` / `answer` / `remove` 三方法体；`crates/infra/agent/src/sdk/loop.rs` 的 `execute_ask`；`crates/core/agent/src/kernel.rs` 的 `stamp` 单点 / 状态机 / `drive` 的收口缝语义；`LoopTurn` / `PumpPayload` / `EngineFacade` 三处（AC-7 的机械判据）；`crates/core/orchestration/**`、`crates/infra/store/**`、`crates/core/workflow/**`、工具面与防线 / 沙箱 / 保护 / 压缩 / 重建全套。
> ② **锁纪律**（design DD9）：`agent_steer` 的三段各自「块内取放」，任两段 MUST NOT 嵌套（`is_pending` 取 ask 锁即释放、`request` 取 steer 锁即释放、`ack.await` 零锁）；与 `execute_ask` 的 `register`（引擎线程持 ask 锁）之间不得形成双向等待面。
> ③ **两个会话 id 不得混用**：注册表键 = core 铸 session id（`AgentSession` 由内核按该键登记）；`LoopTurn.session_id` 是引擎侧 `sdk-…` 标识，MUST NOT 作注册表键、MUST NOT 进 `SteerRegistry`。
> ④ **生成物 / 版本纪律**：`src/types/generated/**` 不手改（由 `bindings:export` 再生成，产物随变更提交）；`package.json` 版本按「落地时 `HEAD` +1 patch」相对递增（MUST NOT 写死字面号作判据）；`src-tauri/Cargo.toml` crate 版本行不跟。
> ⑤ **门禁分工**：整链 `pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**由桌面端 static-check / test-execution 相位执行，本相位不运行、不重复执行。据此自查项**只收源码级静态判据**——`cargo clippy`、`grep` 零命中、读码结构复核、`git diff` 边界比对、前端 `vp check` / `knip`；测试回归与整链门一律写成「交接 …」条目，MUST NOT 作为任何阶段自查项的完成条件。
> ⑥ **测试声明与 sidecar 文件同批**：本相位**不**在 `crates/core/agent/src/lib.rs` 加 `#[cfg(test)] mod steer_test;`（先加声明会让 `cargo check --all-targets` 因缺文件失败）；该声明由 test-gen 创建 `steer_test.rs` 时同批落地（位置：`mod state_test;` 与 `mod stream_test;` 之间，字母序）。

## 阶段一：core/agent 会话地基（`SteerRegistry` + `is_pending` + 通道字段 + 内核登记 / 除名）

> 本阶段**破坏性**：`SessionKernel::new` 加第 5 参、`AgentSession` 加第四字段，两处一起改才可编译（拆开即工作区不可编译）。编译闭合批随本阶段同批落地——**含生产侧两个构造点**（`compose.rs` 的 `SessionKernel::new` 调用、`sdk/runner.rs:248` 的 `Ok(AgentSession { … })` 全字段字面构造），后置到阶段二会让本阶段末的 `cargo clippy --workspace --all-targets` 因 `E0061` / `E0063` 必红。

- [x] 新建 `crates/core/agent/src/steer.rs`：
  - 顶部 `use std::collections::HashMap; use std::sync::Mutex; use tokio::sync::{mpsc, oneshot};`
  - 两个类型（doc 逐句写清「不合并」与「零编排域类型」的理由）：
    ```rust
    /// 补充说明载荷：`text` 为原样文本，`ack` 是「已入本轮史」的逐条确认。
    /// 每条消息自带一个 ack，是「不合并」在类型上的载体——三条在途 = 三个 ack。
    #[derive(Debug)]
    pub struct SteerMessage {
        pub text: String,
        pub ack: oneshot::Sender<()>,
    }

    /// steer 注入注册表（内存态，进程生命周期，应用重启即清空）：会话级单向
    /// 通道，键 = core 铸会话 id（与 [`crate::StopRegistry`] / [`crate::AskRegistry`]
    /// 同址同键空间）。**零编排域类型**：不认识 workspace root 与 change——
    /// 两段寻址由壳层翻译为会话 id。
    #[derive(Default)]
    pub struct SteerRegistry {
        slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>>,
    }
    ```
  - `pub fn new() -> Self`：`Self::default()`（与 `StopRegistry::new` 同形）
  - `pub fn register(&self, session_id: &str, tx: mpsc::UnboundedSender<SteerMessage>)`：`self.slots.lock().expect("steer 注册表锁不可中毒").insert(session_id.to_owned(), tx);`（doc：同键覆盖即上一轮的接收入端已 drop）
  - `pub fn request(&self, session_id: &str, msg: SteerMessage) -> Result<(), String>`：锁内 `self.slots.lock().expect("steer 注册表锁不可中毒").get(session_id)`，`None` → `Err("会话已收口，补充说明不可送达".to_owned())`，`Some(tx)` → `tx.send(msg).map_err(|_| "会话已收口，补充说明不可送达".to_owned())`（doc 按 design DD1 写明「miss 与 send 失败同归一个臂：两者都是『该会话当前没有在跑的轮』；**不是** `AskRegistry::answer` 的受理语义——steer 是送达语义，静默 `Ok` 会让用户以为消息进了史」）
  - `pub fn remove(&self, session_id: &str)`：`self.slots.lock().expect("steer 注册表锁不可中毒").remove(session_id);`（miss 幂等）
- [x] `crates/core/agent/src/ask.rs`：新增只读方法（**三方法体零改动**）
  ```rust
  /// 该会话此刻是否有等待中的 ask（ask 阻塞窗口的唯一真相）。`SteerRegistry`
  /// 的条目在 ask 阻塞期仍在场（会话没结束，只是 loop 卡在工具执行分支），
  /// 故「条目在场」不能证明可送达——本方法只读 `contains_key`，O(1)，取锁即释放。
  pub fn is_pending(&self, session_id: &str) -> bool {
      self.slots
          .lock()
          .expect("ask 注册表锁不可中毒")
          .contains_key(session_id)
  }
  ```
- [x] `crates/core/agent/src/runner.rs`：`AgentSession` 加第四字段（其余字段与 `RunHandle` / `AgentRunner` 逐字不动）
  ```rust
  /// 用户 → turn 的单向注入（补充说明 / steer）：发送端交内核登记，接收入端
  /// 由引擎侧建通道时交给泵并随本轮 loop 生命周期存亡（一轮一命不破）。
  pub steers: tokio::sync::mpsc::UnboundedSender<crate::steer::SteerMessage>,
  ```
- [x] `crates/core/agent/src/kernel.rs`：
  - 顶部 use 补 `use crate::steer::SteerRegistry;`
  - `SessionKernel` 加私有字段 `steer_registry: Arc<SteerRegistry>`；`new(sink, registry, streams, ask_registry, steer_registry: Arc<SteerRegistry>)` 五参，doc 补一句「两张会话级注册表（ask / steer）由组合根注入，MUST NOT 内部自铸——否则与引擎侧登记的不是同一张」
  - `RunningTurn` 加私有字段 `steer_registry: Arc<SteerRegistry>`；`begin_turn` 的 `Ok(RunningTurn { … })` 补 `steer_registry: Arc::clone(&self.steer_registry),`
  - `begin_turn` 在 `self.streams.begin(&session_id);` 之后加（与停止登记同址）
    ```rust
    // 补充说明登记（键 = core session id，与停止 / ask 同址）：sender 由本轮
    // 引擎会话携带，接收入端在循环内 drain
    self.steer_registry
        .register(&session_id, agent_session.steers.clone());
    ```
  - `drive` 的 `let RunningTurn { … } = self;` 解构补 `steer_registry,`；**3 处** `Self::converge(...)`（`:241` 提问送达失败 / `:298` 落库失败 / `:333` 终态出口）各补 `&steer_registry,`；`converge` 签名在 `ask_registry: &AskRegistry,` 之后加 `steer_registry: &SteerRegistry,`
  - `converge` 内紧随 `ask_registry.remove(session_id);` 加 `steer_registry.remove(session_id);`，doc 的收口清单补「→ steer 槽除名（残留补充说明随接收入端 drop 收 `Err`）」
- [x] `crates/core/agent/src/lib.rs`：`mod steer;` 置于 `mod state;` 与 `mod stream;` 之间（字母序）；出口 `pub use steer::{SteerMessage, SteerRegistry};` 置于 `pub use state::{…}` 与 `pub use stream::{…}` 之间；**不加** `#[cfg(test)] mod steer_test;`（约束 ⑥）
- [x] 编译闭合批 —— `SessionKernel::new` **57 处**补第 5 参 `Arc::new(SteerRegistry::default())`：`kernel_test.rs`(52) / `lib_test.rs`(1) / `compose.rs`(2：1 处生产 `compose_turn` + 1 处 `#[cfg(test)]` 夹具 `composed_with_runner`，先补同形实参保编译；阶段二把生产那处换成 `compose_turn` 新实参透传、夹具那处保留显式实例) / `compose_test.rs`(2)。**每用例一条显式实例，MUST NOT 引入全局静态**（避免测试间串表）
- [x] 编译闭合批 —— `AgentSession` 字面构造 **6 处测试夹具**补 `steers`：`kernel_test.rs`(2) / `runner_test.rs`(1) / `compose_test.rs`(2) / `worker_test.rs`(1) 就地 `let (steer_tx, _steer_rx) = tokio::sync::mpsc::unbounded_channel();` 后传 `steer_tx`（接收端就地 drop = 「无在途」，与生产语义一致）
- [x] 编译闭合批 —— `AgentSession` 字面构造第 **7 处**（生产 `crates/infra/agent/src/sdk/runner.rs:248`）**本阶段就地落地，MUST NOT 后置到阶段二**：
  - `open_session` 内（与 `question` / `observation` 通道同段）加 `let (steer_tx, _steer_rx) = mpsc::unbounded_channel::<agent::SteerMessage>();`（doc：每轮一条实例通道，无界——`request` 同步 `send`，无容量失败面）；接收端前缀 `_` 即免未用告警——真正的消费点（`try_recv`）要等阶段二的 `loop::run` 落位
  - `Ok(AgentSession { observations, questions, handle })` → `Ok(AgentSession { observations, questions, steers: steer_tx, handle })`
  - 本阶段 `_steer_rx` 随 `open_session` 作用域 drop 即可：此阶段若被投递，`request` 走 send 失败臂 → `Err`（行为面未闭合但工作区可编译、clippy 全绿）；阶段二把 `_steer_rx` 改名为 `steer_rx` 接进 `session_pump` → `loop::run`
- [x] 自查 阶段一（只列静态判据）：`grep -c "pub fn " crates/core/agent/src/steer.rs` 为 4；`grep -n "会话已收口，补充说明不可送达" crates/core/agent/src/steer.rs` **2 处**（miss 与 send 失败同臂）；`grep -c "fn is_pending" crates/core/agent/src/ask.rs` 为 1 且该文件 `register` / `answer` / `remove` 三方法体 `git diff` 零 hunk；`grep -n "steer_registry" crates/core/agent/src/kernel.rs` 覆盖字段 / 五参 / 登记 / 3 处 converge 实参 / `converge` 签名 / remove；读码复核 `begin_turn` 的登记位在 `streams.begin` 之后、`converge` 的 `remove` 与 `registry.remove` / `ask_registry.remove` 相邻；**生产侧闭合已同批落地**：`grep -n "steers: steer_tx" crates/infra/agent/src/sdk/runner.rs` 为 1、`grep -c "unbounded_channel::<agent::SteerMessage>" crates/infra/agent/src/sdk/runner.rs` 为 1（通道在本阶段建、阶段二只接线）；`cargo clippy --workspace --all-targets` 通过（阶段末工作区可编译是阶段划分的不变量，MUST NOT 靠收窄口径绕过）
- [x] 交接 阶段一（测试回归，本相位不执行）：`steer_test`（新，注册表四态：命中原文 / `remove` 后 miss `Err` / miss 幂等 / 接收端已 drop 时 `Err`）、`ask_test`（`is_pending` 四态：登记 → `true` / `answer` 后 `false` / `remove` 后 `false` / 未登记 `false`；既有用例零改动）、`kernel_test`（五参夹具 + `begin_turn` 后可查 / `converge` 后不可查，与 `StopRegistry` 同址断言）、`runner_test` / `compose_test` / `worker_test`（夹具补 `steers`）**由桌面端 test-design / test-gen / test-execution 相位承接**；`steer_test.rs` 的 `#[cfg(test)] mod steer_test;` 声明由 test-gen 与文件同批添加（约束 ⑥）。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段二：infra/agent 通道接线 + 泵透传 + 请求边界 drain + 组合根注入

> 依赖阶段一（`AgentSession.steers` 的通道在阶段一已建好并回填，本阶段只把接收端接进泵与 loop）。`LoopTurn` / `PumpPayload` / `EngineFacade` 三处**零改动**（design DD3）——接收端走 `loop::run` 位置参数。

- [x] `crates/infra/agent/src/sdk/runner.rs`（通道与 `AgentSession.steers` 回填已在阶段一落地，本阶段只接线）：
  - `_steer_rx` 去前缀为 `steer_rx`，`session_pump(...)` 实参在 `question_rx` 之后传 `steer_rx`
  - `session_pump` 形参在 `mut questions: mpsc::Receiver<TurnQuestion>,` 之后加 `mut steers: mpsc::UnboundedReceiver<agent::SteerMessage>,`（doc：本轮补充说明接收入端，随 loop 生命周期存亡）
  - 泵 `select!` 第一臂改 `_ = r#loop::run(&model, &turn, payload.history, steers, observations.clone(), pump_handle.clone()) => {`（其余臂与注释逐字不动）
- [x] `crates/infra/agent/src/sdk/loop.rs`：
  - `run` 签名在 `mut history: Vec<Message>,` 之后加 `mut steers: mpsc::UnboundedReceiver<agent::SteerMessage>,`；doc 补「接收端不进 `LoopTurn`：`try_recv` 需 `&mut`，而 `LoopTurn` 以 `&` 携带（design DD3）」
  - 新增私有 drain（design DD2 三步序，doc 写明「为什么是循环 / 为什么必须 push 新 `Message::user` / 为什么要逐条 ack」）：
    ```rust
    /// 请求边界 drain：把在途补充说明逐条落成「独立 user 密封事件 + 独立史消息 +
    /// 独立 ack」。返回 `false` = 事件通道已关闭（调用方直接 `return history`），
    /// 此时不补 ack——剩余项随接收端 drop 收 `Err`（drop-Err 是送达语义的诚实面）。
    ///
    /// 循环而非单次 `recv`：一次边界把全部在途项取走，后几条不滞留到下个边界。
    /// 必须 push **新的** `Message::user`（不并入 `tool_results` 那条）：rig 0.43.0
    /// 不合并相邻同角色消息，openai 侧展开会把 user 文本排到 tool 之前（provider 400）。
    async fn drain_steers(
        steers: &mut mpsc::UnboundedReceiver<agent::SteerMessage>,
        history: &mut Vec<Message>,
        sender: &mpsc::Sender<AgentEventKind>,
    ) -> bool {
        while let Ok(steer) = steers.try_recv() {
            let ok = sender
                .send(AgentEventKind::Message {
                    role: AgentMessageRole::User,
                    blocks: vec![AgentBlock::Text { text: steer.text.clone(), citations: None }],
                    parent_tool_use_id: None,
                })
                .await
                .is_ok();
            if !ok {
                return false;
            }
            history.push(Message::user(steer.text));
            let _ = steer.ack.send(());
        }
        true
    }
    ```
  - 主循环顶插入 drain（位置：停止快检的 `}` 之后、`// 续跑计数轴` 注释之前——即 `defend` 与发请求之前）：
    ```rust
    // 请求边界 drain：补充说明在下一个请求可见（位置在 defend 之前、尾部追加）
    if !drain_steers(&mut steers, &mut history, &sender).await {
        append_engine_log(&format!(
            "loop 事件通道关闭退出（补充说明 drain）session={} turn_index={turn_index}",
            turn.session_id
        ));
        return history;
    }
    ```
- [x] `crates/infra/agent/src/compose.rs`：`compose_turn` 形参在 `ask_registry: Arc<agent::AskRegistry>,` 之后加 `steer_registry: Arc<agent::SteerRegistry>,`（doc 补「两张会话级注册表成组注入内核；引擎门面不参与——登记发生在内核」）；`SessionKernel::new(Arc::new(StoreSink::new(store)), registry, streams, Arc::clone(&ask_registry), steer_registry)`；`composed_with_runner` 夹具补 `Arc::new(agent::SteerRegistry::default())`
- [x] 编译闭合批 —— `loop::run` **21 处**补第 4 实参：`sdk/loop_test.rs`(20) 既有用例传 `mpsc::unbounded_channel::<agent::SteerMessage>().1`（sender 就地 drop ⇒ `try_recv` 立即 `Disconnected` ⇒ 零副作用）；生产 `sdk/runner.rs`(1) 已在上条落地。**不引入共享静态通道**
- [x] 编译闭合批 —— `compose_turn` **52 处**补实参：`compose_test.rs`(41) / `worker_test.rs`(9) 传 `Arc::new(agent::SteerRegistry::default())`（每用例独立实例）；`change_flow/mod.rs`(1) / `exec/mod.rs`(1) 随阶段三填真值（本阶段先传 `Arc::new(agent::SteerRegistry::default())` 保编译，阶段三换成 `Arc::clone(app.state::<Arc<SteerRegistry>>().inner())`）
- [x] 自查 阶段二（静态判据）：
  - `git diff -- crates/infra/agent/src/sdk/loop.rs` **不含** `LoopTurn` 结构体块、`execute_ask`、`defend`、`finish_success` / `finish_error` / `finish`、`truncation_kind` / `abnormal_finish` 的 hunk（约束 ①）
  - `git diff -- crates/infra/agent/src/sdk/runner.rs` **不含** `struct PumpPayload` 块与 `LoopTurn { … }` 铸造块的 hunk（`AgentSession` 构造块与通道创建在阶段一已落，本阶段只把 `_steer_rx` 改名接线）；`grep -c "struct PumpPayload" …/runner.rs` 为 1 且其字段面零变化；`grep -c "unbounded_channel::<agent::SteerMessage>" …/runner.rs` 仍为 1（MUST NOT 二次铸通道）
  - `grep -c "EngineFacade" crates/infra/agent/src/lib.rs` 与 `git diff -- crates/infra/agent/src/lib.rs` **整文件零 diff**
  - `grep -c "drain_steers" crates/infra/agent/src/sdk/loop.rs` 为 **2**（定义 + 唯一调用点）；`grep -n "ack.send(())" …/loop.rs` 为 1 且在 `history.push(Message::user(...))` 之**后**
  - `grep -n "loop::run(" crates/infra/agent/src/sdk/loop_test.rs \| wc -l` 为 20 且每处多一个实参；`cargo clippy --workspace --all-targets` 通过
- [x] 交接 阶段二（测试回归，本相位不执行）：`sdk/loop_test`（drain 单条 / 多条不合并 / 无在途零副作用 / 停止截停 / 事件通道关闭一态）、`compose_test` / `worker_test`（夹具随动 + 注册表透传内核）**由桌面端 test-design / test-gen / test-execution 相位承接**。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段三：壳层托管 + `agent_steer` 命令 + 两处 `compose_turn` 实参 + 命令登记

- [x] `src-tauri/src/main.rs`：`use agent::{AskRegistry, SessionStreamRegistry, SteerRegistry, StopRegistry};`；`setup_hook` 在 `app.manage(Arc::new(AskRegistry::default()));` 之后加
  ```rust
  // agent 补充说明注册表（会话级，键 = core session id，与 StopRegistry /
  // AskRegistry 同型托管）：内核 begin_turn 登记 / converge 除名 /
  // agent_steer 投递，此处只挂空表
  app.manage(Arc::new(SteerRegistry::default()));
  ```
- [x] `src-tauri/src/commands/exec/mod.rs`：
  - `use ::agent::{…}` 清单补 `SteerMessage, SteerRegistry`（字母序落在 `StopRegistry` 邻近）
  - `agent_start_with` 内 `let ask_registry = Arc::clone(app.state::<Arc<AskRegistry>>().inner());` 之后加 `let steer_registry = Arc::clone(app.state::<Arc<SteerRegistry>>().inner());`，`compose_turn(...)` 的 `ask_registry` 实参之后传 `steer_registry`
  - 新增命令（薄包装四段，紧挨 `agent_answer` 之后落地）：
    ```rust
    /// 补充说明注入（薄包装四段：三参校验 → ask 槽判据 → 注册表投递 → ack 等待）：
    /// 零新开会话 / 轮、零状态机操作、零通知广播、零排队续跑。判据次序是硬要求
    /// （ask 阻塞期 `SteerRegistry` 条目仍在场，只看 miss 判不出来）；三条不可
    /// 送达文案各自独立（成因可辨，MUST NOT 归一成一句「发送失败」）。
    /// 两个 `State` 的 `Arc` 先取出再 await（不跨 await 持借用）。
    #[tauri::command]
    #[specta::specta]
    pub async fn agent_steer(
        steer: State<'_, Arc<SteerRegistry>>,
        ask: State<'_, Arc<AskRegistry>>,
        root: String,
        session_id: String,
        text: String,
    ) -> Result<(), String> {
        if is_blank_root(&root) {
            return Err("非法 root: 不得为空白".to_owned());
        }
        if session_id.trim().is_empty() {
            return Err("非法 session_id: 不得为空白".to_owned());
        }
        if text.trim().is_empty() {
            return Err("非法 text: 不得为空白".to_owned());
        }
        let steer_registry = Arc::clone(steer.inner());
        let ask_registry = Arc::clone(ask.inner());
        // ① ask 槽判据（取 ask 锁即释放）
        if ask_registry.is_pending(&session_id) {
            return Err("等待应答中，补充说明不可送达".to_owned());
        }
        // ② 注册表投递（取 steer 锁即释放；miss 与 send 失败同臂）
        let (ack, delivered) = tokio::sync::oneshot::channel::<()>();
        steer_registry.request(&session_id, SteerMessage { text, ack })?;
        // ③ ack 等待（零锁；对端 drop 即「生效前结束」）
        delivered
            .await
            .map_err(|_| "未送达（会话在补充说明生效前结束）".to_owned())
    }
    ```
- [x] `src-tauri/src/commands/change_flow/mod.rs`：`use ::agent::{…, SteerRegistry, …}`；`let ask_registry = Arc::clone(app.state::<Arc<AskRegistry>>().inner());` 之后加 `let steer_registry = Arc::clone(app.state::<Arc<SteerRegistry>>().inner());`，`compose_turn(...)` 的一处调用在 `ask_registry` 之后补 `steer_registry`（`ChangeFlowControl` / sink / walker 零改动）
- [x] `src-tauri/src/commands/mod.rs`：`all_commands!` 内 `$crate::commands::exec::agent_answer,` 之后加 `$crate::commands::exec::agent_steer,`
- [x] 自查 阶段三（静态判据）：`grep -c "agent_steer" src/commands/mod.rs` 为 1；`grep -c "fn agent_steer" src/commands/exec/mod.rs` 为 1 且命令体三段判据依次为 `is_pending` → `request` → `await`（读码复核次序不可换）；`grep -n "SteerRegistry" src/main.rs` 命中 manage 一处；`grep -c "steer_registry" src/commands/exec/mod.rs src/commands/change_flow/mod.rs` 各 ≥ 2；`grep -rn "Arc::new(agent::SteerRegistry::default())" src/` **零命中**（阶段二的临时实参已全部换成壳层托管的那一张）；命令体 `agent_steer` 段读码不含 `SessionRef` / `begin_turn` / `publish` / `control` 一类符号（零新开会话 / 零状态机 / 零广播）；`cargo clippy --workspace --all-targets` 通过
- [x] 交接 阶段三（测试回归 + 生成物驱动，本相位不执行）：`exec/mod_test`（`agent_steer` 六态：blank root / blank session_id / blank text / ask 登记在场 / 无在跑轮 / 命中后 ack 回流 `Ok` 且等待方收到原文；另加 ack 竞态一态）、`change_flow/mod_test`（`compose_turn` 夹具随动）、`src/bindings/mod_test.rs`（`COMMAND_NAMES` / `COMMAND_WRAPPERS` 各 50 → 51 且含 `agent_steer` / `agentSteer`；首尾锚不变——尾锚仍为 `read_file`）**由桌面端 test-design / test-gen / test-execution 相位承接**。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段四：前端（生成物 + headless 动作面 + 适配层多 user 规则 + composer 变体 + 抽屉接线）

> 生成物（`bindings.ts` / `bindings.zod.ts`）由阶段三的命令登记驱动：先触发再生成取回再继续（本相位只**触发再生成并提交产物**，MUST NOT 手改生成物）。

- [x] `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts` 与 `bindings.zod.ts`（期望：新增 `agentSteer: (root: string, sessionId: string, text: string) => Promise<null>` 与命令名登记，其余零 diff）；产物随变更提交
- [x] `packages/desktop/src/hooks/use-agent-session.ts`：
  - `UseAgentSessionState` 增两成员：
    ```ts
    /** 补充说明注入（运行中会话的 steer）：`agent_steer` 薄包装；`root` /
     * `sessionId` 为 null 时不 invoke、`Err` 进既有 error 面 */
    steer: (text: string) => void;
    /** 已提交、等 ack 回流的 pending 态（提交置 true，ack 成功或 Err 落地置 false） */
    steering: boolean;
    ```
  - `applyUserQuestion` → 更名 `applyUserMessage`，按 design DD4 分化（判据 = 该轮提问气泡是否已在场；doc 重写为「首条 = 提问归一 / 其余 = 补充说明以 `evt-<seq>` 追加，不吞」）：
    ```ts
    const questionId = questionIdOf(event);              // turn-<turnId>-user | evt-<seq>（旧转录）
    const isQuestion = event.turnId == null || !messages.some((m) => m.id === questionId);
    const targetId = isQuestion ? questionId : `evt-${event.seq}`;
    if (messages.some((message) => message.id === targetId)) return messages;
    // 归一路径仅对 isQuestion 成立（pendingId 非 null 时把本地副本原地换 id，消息数不增）
    // 锚点与既有相同：插到 turn-<turnId> 之前（提问先落位 → 补充说明自然紧随其后），找不到即追加
    ```
  - 新增 `useSteer`（与 `useAskAnswer` 同型，design DD8）：
    ```ts
    function useSteer(
      mirrors: SessionMirrors,
      sessionId: string | null | undefined,
      root: string | null | undefined,
    ): { steer: (text: string) => void; steering: boolean; actionError: string | null }
    ```
    函数体：`target = mirrors.currentSessionIdRef.current ?? sessionId ?? null`；`root` / `target` 任一为 null 即 `return`（零 invoke 零副作用）；否则 `setActionError(null)` → `setSteering(true)` → `void ipc.agentSteer(root, target, text).catch((cause: unknown) => setActionError(String(cause))).finally(() => setSteering(false))`
  - hook 返回面：`error` 扩四源保序 `replay.error ?? (error !== undefined ? error.message : null) ?? ask.actionError ?? steer.actionError`；返回对象补 `steer: steerAction.steer` 与 `steering: steerAction.steering`
  - 既有 `answer` / `stop` / `sendMessage` / 重放 / 订阅调度**逐字不动**
- [x] `packages/desktop/src/lib/agent-adapter.ts`：
  - 新增私有 `steerMessage`（design DD5 / DD6）：
    ```ts
    /** 该轮补充说明事件 → user 消息（id 锚在自身 seq；与提问共用同一条部件映射） */
    function steerMessage(event: SealedMessage, paired: PairedResults): AgentUIMessage {
      return {
        id: messageId(event.seq),
        role: 'user',
        metadata: { seq: event.seq, parentToolUseId: event.parentToolUseId },
        parts: plansToParts(planBlocks(event.blocks, paired)),
      };
    }
    ```
  - `entryToMessages` 的 user 分支改写（`find` → 首条 + 其余；`filter` 沿用既有类型谓词，不新增 helper）：
    ```ts
    const users = entry.group.events.filter(
      (event): event is SealedMessage => event.kind === 'message' && event.role === 'user',
    );
    const [question, ...steers] = users;
    if (question !== undefined) messages.push(questionMessage(question, paired));
    for (const steer of steers) messages.push(steerMessage(steer, paired));
    messages.push(assistantMessage(entry.group, entry.record, paired));
    ```
  - 模块 doc / `entryToMessages` / `foldSessionMessages` 的「提问（可缺席）+ 恰一条 assistant」口径补「+ 补充说明（组内其余 user 事件各一条，按 seq 序排在提问之后、assistant 之前，id `evt-<seq>`）」；`questionId` 的 doc 补「只服务组内首条」
- [x] `packages/desktop/src/components/agent/agent-session-view.tsx`：`AgentSessionActions` 增 `steer: (text: string) => void` 与 `steering: boolean`（doc：additive——未接字段的消费点零影响）；`actions` 组装补 `steer: state.steer,` / `steering: state.steering,`
- [x] `packages/desktop/src/components/agent/agent-input.tsx`：
  - `AgentInputProps` 增 `hidePermissionMode?: boolean`（doc：`true` 时不呈 permission-mode 档位行；`onSend` 的 `permissionMode` 参数位保留但下游 MUST NOT 消费）与 `pending?: boolean`（doc：`true` 时发送呈「排队中…」并禁用防连点；输入框与停止入口不受影响）；解构缺省 `hidePermissionMode = false, pending = false`
  - `canSend = useMemo(() => !disabled && !pending && prompt.length > 0, [disabled, pending, prompt])`
  - `ComposerToolbar` 增 `hidePermissionMode` 一参（true 时不渲 `ModeSelect`，其余排布零改动）；`pending` 的可见文案行由 `AgentInput` 在 `<ComposerToolbar>` **之前**渲染（与 `disabled-reason` 同区、同型的可见小字）：`<div className="mb-2 text-xs text-muted-foreground" data-testid={`${idPrefix}-pending`}>排队中…</div>`（`pending` 时）；发送禁用由 `canSend` 合成式承载，`ComposerToolbar` 无需第二套判据
  - 默认变体（两位缺省）渲染与交互**逐字不变**；`send` 的 `onSend({ text, permissionMode })` 形状零改动
- [x] `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`：
  - `viewProps` 增 `composer` 插槽（`AgentInput` 经 `@/components/agent` barrel 引入）：
    ```tsx
    composer: (actions) => (
      <AgentInput
        idPrefix="step-session"
        placeholder="给这一步补一句话（模型在下一个请求边界看到）"
        running={actions.running}
        hidePermissionMode
        pending={actions.steering}
        onSend={({ text }) => actions.steer(text)}
        onStop={actions.stop}
      />
    ),
    ```
  - 组件 doc 由「只读消费点：不接 composer 插槽、无发送 / 停止入口」改为「接 composer 插槽：呈补充说明输入 + 停止；不呈 permission-mode 档位与新会话入口；可用性不预判（三态由后端 `Err` 说话）」
  - `roleRef` 缺席 / `roleRef.sessionId === null` 两条空态**逐字不动**（不渲染 composer）；`askAnswer` 注入位照旧
- [x] 自查 阶段四（静态判据）：
  - `grep -c "evt-\${event.seq}" packages/desktop/src/hooks/use-agent-session.ts` ≥ 1 且补充说明分支在「提问已在场」判据之**后**（读码）；`grep -n "turn-\${event.turnId}-user" packages/desktop/src/hooks/use-agent-session.ts` 只在 `questionIdOf` 一处
  - `grep -c "steerMessage" packages/desktop/src/lib/agent-adapter.ts` 为 2（定义 + 调用）；`grep -n "role === 'user'" …/agent-adapter.ts` 的 `assistantMessage` 跳过逻辑零 diff
  - `grep -rn "ipc\.agentSteer" packages/desktop/src` 只出现在 `hooks/use-agent-session.ts`（组件族零 IPC）
  - `grep -c "pending" packages/desktop/src/components/agent/agent-input.tsx` ≥ 3（prop / canSend / 文案行）；`git diff -- packages/desktop/src/components/agent/agent-input.tsx` 不含默认路径（档位行 / 停止 / 发送形状）的 hunk
  - `git diff --stat -- packages/desktop/src` 的文件面 = design「修改文件」表所列 6 项前端实现文件 + `types/generated/**`；`archi-chat-composer.tsx` / `change-chat-composer.tsx` / `explore-detail-pane.tsx` / `agent-debug-view.tsx` / `agent-virtual.ts` / `agent-messages.tsx` / `session-panel.tsx` / `detail-drawer.tsx` / `change-detail-view.tsx` / `components/agent/index.tsx` **整文件零 diff**
  - `pnpm -C packages/desktop exec vp check` 通过；`knip` 无新增未用导出（`steer` / `steering` 被 `session-transcript-panel` 消费即为已用）
- [x] 交接 阶段四（测试回归，本相位不执行）：`use-agent-session.test`（`applyUserMessage` 分化两态 / `steer` 三参逐字 / null 守卫零 invoke / `steering` 置位与两收尾复位 / `Err` 进 error 面）、`agent-adapter.test`（折叠路两条 user 事件 → 提问 + 补充说明 + 恰一条 assistant，序与 id 两两不等）、`agent-input.test.tsx`（`hidePermissionMode` 无档位行 / `pending` 发送禁用 + 文案行 + 停止面照常 / 默认变体零回归）、`agent-session-view.test.tsx`（`steer` / `steering` 组装）、`session-transcript-panel.test.tsx`（composer 在场 + 两动作接线 + 空态不渲染 composer）**由桌面端 test-design / test-gen / test-execution 相位承接**。本相位不以任何用例结果为完成判据。**交付物 = 期望登记与移交**

## 阶段五：版本、边界自查与交接备忘

- [x] 修改 `packages/desktop/package.json` 的 `version`：按**落地时 `HEAD` 现值 +1 patch** 相对递增（当前盘面 `0.6.38` → `0.6.39`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行恰 `+1`，MUST NOT 写死字面号）；`dependencies` / `devDependencies` **零变化**
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零手改，跟随即生效）；`src-tauri/Cargo.toml` crate 版本行（`0.1.0`）**未被改动**
- [x] 自查 边界零 diff（逐项 `git diff --stat`）：`crates/infra/store/**`、`crates/core/workflow/**`（相位表 / 16 份 prompt md / `include_str!`）、`crates/core/orchestration/**`、`crates/infra/agent/src/sdk/{mount,tools,protect/**,policy,sandbox,context,compact,resume,normalize,web_search,bash,model_spec,explore_note,archi_tool}.rs`、`crates/infra/agent/src/lib.rs`、`.dev-team/architecture/{models,api}/**`、全部 `Cargo.toml`、`packages/desktop/src/{components/agent/{agent-messages,agent-blocks,tool-cards,session-panel,index}.tsx,lib/agent-virtual.ts,views/changes/{change-detail-view.tsx,flow/detail-drawer.tsx},views/changes/agent/**,views/changes/hooks/use-change-chat.ts,views/architecture/agent/**,views/explores/**,views/agent/**}` —— 以上**整文件零 diff**
- [x] 自查 关键判据集中复核（AC 逐条，均为 `grep` / 读码 / `git diff` 面，不含任何用例运行）：
  - AC-1 / AC-2：`drain_steers` 为 `try_recv` **循环**、三步序（事件 → 入史 → ack）且 `push(Message::user)` 独立；drain 位于停止快检之后、`defend` 之前
  - AC-3 / AC-5：`request` 的 miss 与 send 失败同臂同一文案；`converge` 内 `steer_registry.remove` 在场；命令体 ③ 段 `map_err` 文案独立；无超时代码
  - AC-4：命令体 ① 段 `is_pending` 在 ② 段 `request` 之前（次序不可换）；`ask.rs` 三方法体 `git diff` 零 hunk；`grep -c "is_pending" crates/core/agent/src/ask.rs` 为 1
  - AC-6：`begin_turn` 登记位与 `streams.begin` 相邻；`converge` 三处 `remove` 相邻；停止路径零新增代码（`StopRegistry::request_stop` / `RunHandle::{request_stop, wait_requested}` 零 diff）
  - AC-7：`LoopTurn` / `PumpPayload` / `EngineFacade` 三处 `git diff` 零 hunk；`grep -c "questions.recv" crates/infra/agent/src/sdk/runner.rs` 为 1
  - AC-8：`agent_steer` 在 `all_commands!` 内紧随 `agent_answer`；生成物 `agentSteer` 在场且 `git diff` 只多该命令块
  - AC-9 / AC-10：前端两路 id 规则逐条复核（`turn-<turnId>-user` 只属首条 / 补充说明恒 `evt-<seq>` / 两两不等）
  - AC-11 / AC-12：`useSteer` 的 null 守卫与 `steering` 两收尾复位；`AgentInput` 两可选位缺省路径零回归；抽屉插槽三动作接线
- [x] 登记计划外收敛（若有）：实现中若出现「任务未列、实现必需」的最小面，如实留痕于本节（说明为何必需、为何不改变 design 任何裁决的语义面），以免下轮重演
  - `crates/infra/agent/src/sdk/runner.rs` 的 `session_pump` 加 `#[allow(clippy::too_many_arguments)]`：新增 `steers` 形参后为 8 参（clippy 上限 7）。与 `converge` / `compose_turn` 同址同法（既有惯例），零语义面。
  - `crates/infra/agent/src/lib_test.rs` 的 `compose_turn` 签名锚定 `fn` 指针补 `Arc<agent::SteerRegistry>` 一位：该测试以 `fn` 指针形状锚定组合根签名，实参变化即类型不匹配（`E0308`）。属「字段面 / 签名收敛」的机械闭合，零断言语义改动。
  - 前端三处测试夹具补 `steer` / `steering`（`agent-session-view.test.tsx` / `index.test.tsx` / `session-panel.test.tsx`）：`UseAgentSessionState` 与 `AgentSessionActions` 的新成员是**必填**，夹具缺位即 `TS2739` / `TS2322`。属编译闭合（design「新增字段全部可选」只对 `AgentInputProps` 成立），零用例与断言改动。
  - `use-agent-session.ts` 抽出模块私有 `errorChain`（四源 `??` 保序合并）：`useAgentSession` 加两成员后超 `eslint(max-lines-per-function)` 上限（51/50）。等价内联、单点复用，零行为面。
- [x] 交接交付门（本相位不执行，只登记期望并向桌面端移交）：`bindings:export` / `api:check` / `client:check` 三段链（`pnpm -C packages/desktop run check`）与 `types/generated/**` 的再生成核实归桌面端 **static-check**（期望：`agent_steer` / `agentSteer` 出线，`SteerMessage` / `SteerRegistry` 不出线；`api:check` 的 `apiFindings` 若列出新增公共面属预期，不影响 `status` / `passed`）；**测试回归**（`cargo test` 全 workspace、`pnpm -C packages/desktop test`）归桌面端 **test-execution**。两者均不写入任何阶段自查项，也不作为本相位的完成判据。**交付物 = 期望登记与移交**
- [x] 交接备忘（发给 test-design / test-gen / test-execution，本相位不写用例；**交付物 = 备忘登记与移交**）：
  - `crates/core/agent/src/steer_test.rs`（新）＋ 同批加 `#[cfg(test)] mod steer_test;`：`register` 后 `request` 命中（等待方收原文）/ `remove` 后 miss `Err` / miss 幂等 / 接收端已 drop 时 `Err`（send 失败归同臂）
  - `crates/core/agent/src/ask_test.rs`：`is_pending` 四态（登记 `true` / `answer` 后 `false` / `remove` 后 `false` / 未登记 `false`）；既有 take 语义用例零改动
  - `crates/core/agent/src/kernel_test.rs`：五参夹具；`begin_turn` 后两表（`StopRegistry` / `SteerRegistry`）都能查到该键、`converge` 后都查不到且 `request` → `Err`
  - `crates/infra/agent/src/sdk/loop_test.rs`：drain 单条语义（一次边界两条在途 → 两条密封 user 事件 + 两条 `Message::user` + 两次 ack，`history` 尾部 `tool_results` 之后是新 user 消息）；无在途零副作用；停止截停（残留收 `Err`、转录零该文本事件）
  - `crates/infra/agent/src/compose_test.rs` / `worker_test.rs`：`compose_turn` 第 5 参夹具与注册表透传内核
  - `src-tauri/src/commands/exec/mod_test.rs`：`agent_steer` 六态（blank root / blank session_id / blank text / ask 在场 / 无在跑轮 / 命中 `Ok` 且原文送达）+ ack 竞态一态（投递后立刻 drop 接收端 → `Err`）
  - `src-tauri/src/bindings/mod_test.rs`：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 各 50 → 51，含 `agent_steer` / `agentSteer` 断言，首尾锚不变
  - `src-tauri/src/commands/change_flow/mod_test.rs`：`compose_turn` 夹具随动
  - 前端：`use-agent-session.test.ts`（`applyUserMessage` 首条归一 / 第二条 user（`seq = 9`）追加 `evt-9` 且不丢 / `steer` 三参逐字 / null 零 invoke / `steering` 两收尾复位 / `Err` 进 error 面）、`agent-adapter.test.ts`（折叠路两条 user → 提问 + 补充说明 + 恰一条 assistant，序与 id 两两不等）、`agent-input.test.tsx`、`agent-session-view.test.tsx`、`session-transcript-panel.test.tsx`
