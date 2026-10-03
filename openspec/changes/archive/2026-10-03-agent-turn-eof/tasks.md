# 任务: agent-turn-eof

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖 design.md 变更清单的实现文件。顺序按依赖排列：协议面注释口径 → CLI 组装 → CLI 泵 → SDK 泵 → 守线。版本提升（0.4.1 → 0.4.2）按 proposal AC-6 于归档时执行，不在实现任务列表（design D7）。

## 阶段一：core/agent 注释口径（逻辑零改动）

- [x] 修改 `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs`（纯注释）：`RunningTurn.question` 字段注释（现「轮提问（ask 语义的驱动载荷）」）与 `drive` doc 注释（现「泵驱动（ask 语义）：…」）改为一轮一命口径（送达一问 → 观测循环至引擎侧单边 EOF → converge）；`converge` doc 的「ask 失败」臂措辞保留（该臂即泵返回后第二问送达的失败路径）；`drive` 观测循环（`while let Some(kind) = session.observations.recv().await`）、`converge`、`begin_turn`、`StopRegistry` 逻辑零触碰（不增状态机 break / stop select / 第二收口路径）
- [x] 修改 `packages/desktop/src-tauri/crates/core/agent/src/runner.rs`（纯注释）：`AgentSession.questions` 字段注释（现「轮驱动：逐轮送达提问（引擎按自身进程模型兑现）」）与 `AgentRunner::open_session` doc（现「返回的会话句柄随后经 questions 逐轮驱动（ask 语义）」）改单问驱动（一轮一命）口径；类型 / trait / 字段签名零改动

## 阶段二：CLI 组装 headless 禁 ask

- [x] 修改 `packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs`：`build_args` 在 `--resume` 段（`resume_handle` 为 `Some` 时的两枚）之后追加 `"--disallowedTools"` 与 `"AskUserQuestion"` 两枚 arg——恒为参数序列最末两枚（variadic `<tools...>` flag 置中间会吞并后续参数）；三档 permission（Default / AcceptEdits / BypassPermissions）× New / Continue（`resume_handle` Some/None）全组合位置一致；工具名字面量为 camelCase `AskUserQuestion`；`TurnParams` 形状、`permission_mode_flag_value` 与既有 flag 面零改动

## 阶段三：CLI 泵一轮一命

- [x] 修改 `packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs`：`session_pump` 去除 `while let Some(question) = questions.recv().await` 多问循环，改单问服务——`recv` 取一问（`None` 即组合根半边先关，直接 return）→ `spawn_turn` 成功则 `process.run(observations.clone()).await` 完成后无条件 return（`stopped` 返回值不再消费：停止路径在 `pump_lines` 内已单轮终止，泵不回到提问接收）；spawn 失败臂既有合成收敛（`TurnDone` failed 记因 `error_cli_missing` / `error_spawn_failed`）+ return 原样；`QUESTION_CHANNEL_CAPACITY` 注释「逐轮送达」改单问送达口径；`session_pump` doc 注释改一轮一命（服务恰一问 → 泵返回 → observations 发送端释放 → 内核 EOF 收口；spawn 失败合成收敛后同样终止），spec 指针沿用相对域根定式、不含 layout 命名隔离双禁令字面量；`open_session` / `TurnProcess` / `pump_lines` / `spawn_turn` / `kill_process_tree` / `build_command` 逻辑零改动

## 阶段四：SDK 泵一轮一命与 history_slot 移除

- [x] 修改 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`：`session_pump` 去除多问循环与 `history_slot`（`Option<Vec<Message>>` 的 `take()` / 回灌）整体——`history` 参数直传单轮 `r#loop::run`，其返回的累积史（跨轮回灌）在一轮一命下弃用（`sdk/loop.rs` 签名与实现零改动）；停止臂既有 `return` 与 biased 臂序语义原样（先导事件恒达论证注释保留、续轮回灌措辞移除）；`session_pump` doc 注释与 `QUESTION_CHANNEL_CAPACITY` 注释改一轮一命口径；`SdkRunner` / `resolve_resume` / `continue_session_id` / `build_model` / `next_session_id` 零改动（会话史唯一来源仍为 store 全史转录重建）

## 阶段五：守线（静态，不含测试执行）

- [x] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy）与 `pnpm -C packages/desktop run client:check`（前端零触点，作零 diff 边界确认，无新增豁免条目）；自动化套件的执行验证由 test-execution 阶段承接
- [x] 静态自查：`history_slot` 标识符在 `crates/infra/agent/src/sdk/runner.rs` 零命中（死代码移除收口，`sdk/loop.rs` 不受影响）
- [x] 静态自查：禁改面零 diff——`kernel.rs` 除两处注释位点外零 diff（EOF 唯一收口缝逻辑保持）、`tests/golden/**`、前端 `src/**`、`crates/core/orchestration/**`、store schema 与落库形态、`sdk/policy.rs` / `sdk/sandbox.rs`（git diff 范围核对）
- [x] 静态自查：新增/修改的产品 `.rs` 源码（含注释与 doc comment）不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名，豁免面以 `crates/core/foundation/src/layout/mod.rs` 为准），spec 指针用相对域根定式
- [x] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏；实现清单零 `packages/desktop/package.json` 触点）
