# 测试设计: desktop-step-running-snapshot

> **变更**: desktop-step-running-snapshot
> **日期**: 2026-10-17
> **输入**: proposal.md（AC-1–AC-11）、design.md（D1–D12 定稿、「公共函数 / API」与「变更清单」子表；公共 API 子表未列行的源文件按「类型定义」表 / 修改文件表 / D 决策声明的行为面映射条目，各节 HTML 注释留痕来源）
> **测试框架与布局判定**: Rust 侧沿 crate 内联 `#[cfg(test)]` 模块 + `<模块名>_test.rs` 同目录姊妹文件（`lib.rs` 挂 `mod`；agent / orchestration / workflow / infra-store 四 crate 同式），断言用 std `assert_eq!` / `assert!` + `serde_json` 形态断言；workflow 另有 `tests/corpus_golden_test.rs` 集成 golden 面（本节自映射：测试文件即被测文件）。前端沿 vitest（`vite-plus/test`）+ @testing-library/react，测试文件同目录 `*.test.ts(x)`；走秒用例沿 `vi.useFakeTimers()` / `vi.advanceTimersByTime()` 既有先例（use-change-detail.test.ts 等）。既有测试构造点（`ChangeStepState` 字面量约 20 处，含 change-detail-view.test.tsx `stepRow()`）的机械补字段由编译器驱动随实现清单随动，本设计不逐处展开。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 契约四字段全链在场：六载体齐备（i64 / Option 口径一致）、bindings 再生守卫、活面自动跟随、命令面签名零改动 | `core/orchestration/src/state.rs`、`core/workflow/src/state.rs`、`core/orchestration/src/run_history.rs`、`infra/store/src/model.rs`、`core/workflow/src/queries/detail.rs`、`flow/types.ts`、`types/generated/bindings.ts`（specta 再生；一致性守卫归桌面端静态管线） |
| AC-2 | 时间铸造语义：running emit 铸 started 且 ended=None、终态 emit 铸 ended、started 合并保首见不被终态顶替、瞬步单 emit started==ended、stopped 收口同有 ended | `core/orchestration/src/walker.rs`、`core/orchestration/src/control.rs` |
| AC-3 | token 本步增量：run_worker 终态单次落值 = outcome.transcript 的 TurnDone.usage 求和、usage 缺席 None、非 WorkerAgent 步恒 None、无二次刷新通道 | `core/orchestration/src/walker.rs`、`core/agent/src/usage.rs` |
| AC-4 | 合并语义：accumulate_node started 无条件保首见、ended / token_input / token_output some 才覆写、同键多 emit 后快照与合并规则一致 | `core/orchestration/src/control.rs` |
| AC-5 | 落库与旧行兼容：新行四列与累积器逐字段一致、旧库行解码缺省 `0` / `None` ×3 零报错零迁移、timestamp 列语义保留、写路径重复调用无副作用累积 | `infra/store/src/model.rs`、`infra/store/src/store.rs` |
| AC-6 | 出线口径：`ChangeRunNodeRecord` 毫秒数纯 derive、旧行 `0` / `null` 原样出线零回填、runs 级 ISO 口径不变两口径并存 | `core/workflow/src/queries/detail.rs` |
| AC-7 | 节点呈现：标题下常驻行、已收口显静态耗时、token 段仅 workerAgent、缺席与旧数据 "-"（耗时）/ "—"（token）、0ms 瞬步省显 | `flow/metrics.ts`、`flow/run-step-node.tsx` |
| AC-8 | running 走秒：仅活面节点、每秒 tick 局部重渲染、翻态 / 卸载即清 timer 回落静态、库读史 status=running 残留不走秒 | `flow/run-state.ts`（live 源标注）、`flow/run-step-node.tsx` |
| AC-9 | corpus 与 golden：种子定值注入零 wall-clock、时间 / token 有无两态样本、detail nodes 投影 golden 显式重写人工确认留痕 | `core/workflow/tests/corpus_golden_test.rs`、`core/workflow/tests/fixtures/README.md`、`core/workflow/tests/golden/` |
| AC-10 | 求和上移：唯一实现居 core/agent、orchestration / infra-store 两消费方同源、store 会话统计输出与上移前逐字段一致、orchestration 零 infra 新增 | `core/agent/src/usage.rs`、`core/agent/src/lib.rs`、`infra/store/src/store.rs` |
| AC-11 | 版本与管线：desktop 0.5.11、plugins/dev-team 2.10.44 零改动、全绿管线（静态面归桌面端，无单元测试面） | `packages/desktop/package.json` |

> 路径前缀约定：Rust 侧相对 `packages/desktop/src-tauri/crates/`，前端相对 `packages/desktop/src/`。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/agent/src/usage.rs -> packages/desktop/src-tauri/crates/core/agent/src/usage_test.rs

#### 待测功能

- sum_usage_tokens(): 转录 `TurnDone.usage` 按鸭子类型 key 求和单源（全缺席 None、部分在场 saturating 求和；函数体自 infra/store 上移逐行一致）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| sum_usage_tokens | 正向 | 多个 TurnDone 事件均携指定 key（`input_tokens` / `output_tokens` 两键各测一组）→ Some(逐值之和) | 多事件遍历 |
| sum_usage_tokens | 正向 | 单个 TurnDone 事件携 key → Some(该值) | 单事件 |
| sum_usage_tokens | 边界 | 空转录 `&[]` → None（零事件边界） | 空集 |
| sum_usage_tokens | 边界 | usage 在场但 key 缺席（`{}` / `Value::Null`）→ None | 缺键 |
| sum_usage_tokens | 边界 | 部分事件携 key、部分缺席 → Some(在场值之和)（any-then-some 语义锚） | 混合事件 |
| sum_usage_tokens | 边界 | 求和溢出 u64（`u64::MAX` 邻近两值）→ saturating_add 饱和不 panic | 溢出边界 |
| sum_usage_tokens | 边界 | 超大转录（数千事件）求和结果与逐值 fold 一致（遍历不截断） | 超大列表 |
| sum_usage_tokens | 异常 | key 对应值非 u64（字符串 / 浮点）→ 该事件不计入、不 panic（`as_u64` 判定锚） | 类型漂移 |
| sum_usage_tokens | 异常 | 非 TurnDone 事件（RunStarted / Message / MessageDelta / SystemNotice / Raw）混入 → 全部跳过，仅 TurnDone 参与求和 | 干扰事件 |
| sum_usage_tokens | 组合 | describe「sum_usage_tokens 上移同源（AC-10）」：同一转录分别求 in / out 两键，与 store 会话统计（SessionStats.input/output_tokens）现算口径一致——两消费方同源锚（store 侧半边见 store_test.rs 节） | 两键对照 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——纯函数直测；`AgentEvent` 字面量直构（`AgentEvent::stamp(seq, AgentEventKind::TurnDone { usage: json!({...}), .. })`，非 TurnDone 变体同式构造） | 全部用例（无进程边界、无注入依赖） |

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

#### 待测功能

- finish_command(): run 收口写命令组装——节点映射增四字段直复制进 `RunNodeEntry`（零过滤零改写；签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| finish_command | 正向 | 节点四字段全值（started_at=1_727_000_000_000 / ended_at=Some(+200_000) / token_input=Some(12_000) / token_output=Some(3_400)）→ `RunNodeEntry` 逐字段等值直复制 | 单节点 |
| finish_command | 正向 | 多节点混排（WorkerAgent 步 / 工具步 / Gate / Git 步）四字段各自直复制、整包序 = 入参序 | 多节点 |
| finish_command | 边界 | token 两值与 ended_at 全 None（工具步 / Gate 形态）→ None 直透（不铸造不默认） | None 态 |
| finish_command | 边界 | started_at = 0（缺席语义值）→ 原样复制零改写 | 0 值 |
| finish_command | 边界 | 空节点整包 → nodes 空 vec（零步收口合法，既有回归持衡） | 空集 |
| finish_command | 异常 | running 残留步入包（status=Running、ended_at=None）→ None 直透不补值不报错（收口在途步合法形态） | 在途步 |
| finish_command | 异常 | 瞬步 started_at == ended_at 同刻值 → 两字段独立直复制（互不推导不合并） | 同刻值 |
| finish_command | 组合 | describe「ChangeStepState → RunNodeEntry → RunNodeRecord 透传链」：四字段经 finish_command 组装后喂 `RunNodeRecord::new`，与源逐字段一致（透传链上半程锚，AC-1；下半程见 model_test.rs 节） | 单链路 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——纯组装函数直测；`ChangeStepState` / `RunRequest` 字面量直构（含四字段两态） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- RunNodeRecord::new(): 直复制 `entry` 四字段进新列（`timestamp` 参数语义不变；签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| RunNodeRecord::new | 正向 | entry 四字段全值 → record 四列逐字段等值；timestamp 参数独立赋值（与 started_at 不混淆——timestamp 语义保留锚） | 单行 |
| RunNodeRecord::new | 正向 | native_model 编码 → decode 回环四列逐字段一致（加列后 id=16 version=1 编解码回归） | 编解码往返 |
| RunNodeRecord::new | 边界 | entry token 两值 / ended_at None → 列 None 直透 | None 态 |
| RunNodeRecord::new | 边界 | 旧 JSON 形态（无四新键、仅既有键）反序列化 → started_at=0 / ended_at=None / token_input=None / token_output=None，零报错零迁移（`#[serde(default)]` 锚，AC-5） | 旧行解码 |
| RunNodeRecord::new | 边界 | 同 run 多行 node_seq 递增 → node_key 打包不撞键；node_id() / node_seq() 身份面不因新列漂移 | 多行 |
| RunNodeRecord::new | 边界 | 序列化 wire 面：camelCase 四新键 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` 在案 | 序列化形态 |
| RunNodeRecord::new | 异常 | entry 载荷防御形态值（started_at 负值 / token 接近 u64::MAX）→ 直复制不 panic 不钳制 | 防御值 |
| RunNodeRecord::new | 组合 | describe「RunNodeEntry → RunNodeRecord → list_run_nodes 读回」：真件 Store 经 finish_change_run 单事务落库后 `list_run_nodes` 读回四列逐字段一致（写读同字段集，AC-5；读面半边见 store_test.rs 节） | 真件回环 |
| RunNodeRecord::new | 边界 | 写幂等锚一：同一 entry 连续两次 `new` → 两产物 `PartialEq` 全等（纯构造可重复调用、零副作用累积——AC-5 加列后构造面稳定性） | 重复调用 |
| RunNodeRecord::new | 边界 | 写幂等锚二：native_model 编码 → 解码 → 再编码，两次编码字节形态一致（version=1 additive 加列不引入重编码漂移，重复落读不累积改写） | 二次编码 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| native_db 工作区库 | 真件 Store（tempdir 临时库）沿 model_test 既有写读回环先例；无外部进程边界 mock | 编解码往返 / 真件回环用例；纯构造用例零装置 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- Store::list_run_nodes(): 四列逐字段透传 `RunNodeStateRecord`（node_seq 升序 = emit 序口径不变；签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| Store::list_run_nodes | 正向 | finish_change_run 落含四字段多节点整包 → 读回 `RunNodeStateRecord` 四字段逐字段等值 | 多行 |
| Store::list_run_nodes | 正向 | 多 run 并存各携节点 → 按 run_id 圈定 + node_seq 升序（emit 序口径不因新列漂移） | 多 run |
| Store::list_run_nodes | 边界 | 落库行 token 两值 / ended_at None → 读回 None 原样 | None 态 |
| Store::list_run_nodes | 边界 | 未知 run_id / 空库 → 空数组（既有回归持衡） | 空集 |
| Store::list_run_nodes | 异常 | 旧行（无新列编码）与新行混存同 run → 混合解码各自形态（旧行缺省 0/None、新行原值）零报错 | 混合旧行 |
| Store::list_run_nodes | 组合 | describe「sum_usage_tokens 上移同源（AC-10）」：经 store 既有会话 / 轮 / 事件操作面植入含 TurnDone.usage 转录的会话 → 会话清单与单查统计（SessionStats.input/output_tokens）输出与上移前口径逐字段一致——store 改引 `agent::sum_usage_tokens` 后聚合行为零变化 | 同源锚 |
| Store::list_run_nodes | 边界 | 读幂等：落库后连续两次 `list_run_nodes` → 两次返回逐字段全等（读面纯投影、重复调用零漂移零累积） | 重复读 |
| Store::list_run_nodes | 异常 | 写幂等守卫：同一 RunFinishCommand 收口成功后二次 `finish_change_run` → `StoreError::Conflict`（run 已终态拒重复收口）、`list_run_nodes` 行集不变（node_key 主键防重——无副作用累积，AC-5 单事务半边） | 二次收口 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| native_db 工作区库 | 真件 Store（tempdir 临时库）+ store 既有操作面植入语料（run / 会话 / 事件写路径），零 mock | 全部用例（私有 sum_usage_tokens 删除后 grep 单实现归桌面端静态检查） |

### packages/desktop/src/views/changes/flow/metrics.ts -> packages/desktop/src/views/changes/flow/metrics.test.ts

#### 待测功能

- formatTokens(): token 缩写（null → "—" / 小于 1000 原样 / k / M 一位小数四舍五入去尾零）
- settledDurationLabel(): 已收口步耗时段（缺席 → "-"、0ms → null 省显、正值 → 分级串；缺席判定收本模块单点）
- runningDurationLabel(): running 走秒段（startedAt <= 0 → "-"、否则 now - startedAt 分级串；now 参数注入纯函数可测）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| formatTokens | 正向 | 999 → "999"（< 1000 原样十进制）；1_000 → "1k"；12_000 → "12k" | 阶下与 k 阶 |
| formatTokens | 正向 | 12_300 → "12.3k"；12_360 → "12.4k"（一位小数四舍五入）；去尾零：12_000 → "12k" 非 "12.0k" | 去尾零 |
| formatTokens | 正向 | 1_000_000 → "1M"；1_500_000 → "1.5M"；2_300_000 → "2.3M" | M 阶 |
| formatTokens | 边界 | null → "—"（em dash，与耗时缺席 "-" 词面分立锚） | 缺席 |
| formatTokens | 边界 | 0 → "0" | 最小值 |
| formatTokens | 边界 | 999_999 → "1000k"（k 阶封顶不跨界 M，D8 边界留痕） | 阶界 |
| formatTokens | 边界 | 超大值（9_999_999_999 级）→ M 阶输出不炸不跨界（D8 无 B 阶） | 超大值 |
| formatTokens | 异常 | NaN → 不抛错、返回可读串（无 "NaNk" 类碎片拼接异常） | 非法数值 |
| settledDurationLabel | 正向 | (1_000_000, 1_200_000) 差 200_000ms → "3m20s"（分级互斥：分秒阶不落秒小数） | 分秒级 |
| settledDurationLabel | 正向 | (1_000_000, 1_001_234) → "1.2s"；(1_000_000, 1_059_900) → "59.9s" | 秒级 |
| settledDurationLabel | 正向 | 差 3_600_000 → "1h00m"；差 3_900_000 → "1h05m"（两位补零分）；差 7_200_000 → "2h00m" | 时级 |
| settledDurationLabel | 边界 | (1_000_000, 1_000_999) 差 999ms → "1.0s"（999ms → 1.0s 边界留痕，D8） | 秒阶界 |
| settledDurationLabel | 边界 | ended == started（0ms 瞬步）→ null（省显——调用侧不渲染该段） | 0ms |
| settledDurationLabel | 边界 | ended < started（逆序 / 容差）→ null（`<=` 判定省显） | 逆序 |
| settledDurationLabel | 边界 | startedAt = 0（旧库行缺省）→ "-"；startedAt < 0 → "-"（缺席判定单点，无 0 魔数散落，R4） | 缺席 |
| settledDurationLabel | 边界 | endedAt = null（running / 库读史残留）→ "-" | null 态 |
| settledDurationLabel | 异常 | startedAt / endedAt 含 NaN → 不抛错（缺席或原样分级，无碎片串） | 非法数值 |
| runningDurationLabel | 正向 | (1_000_000, 1_001_234) → "1.2s"（now 参数注入，零 Date.now() 依赖） | 秒级 |
| runningDurationLabel | 正向 | (1, 3_600_000) 差 3_599_999 → "59m59s"（时阶界下沿）；(1, 3_600_001) → "1h00m" | 阶界 |
| runningDurationLabel | 正向 | 差 7_200_000 → "2h00m"（长跑形态） | 时级 |
| runningDurationLabel | 边界 | startedAt = 0 → "-"；startedAt < 0 → "-" | 缺席 |
| runningDurationLabel | 边界 | now == startedAt（刚起步 0ms）→ "0.0s"（running 恒显、无省显路径） | 0ms |
| runningDurationLabel | 异常 | now < startedAt（时钟回拨形态）→ 不抛错、非空串输出（负差防御） | 逆序 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——纯函数直测；`formatDurationMs` 模块私有不导出（knip test-only export 纪律，D8），分级断言全部经两个导出标签函数覆盖 | 全部用例 |

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

- unifiedRunNodes(): 双源合成返回步行增 `live` 源标注（persisted false / live true；签名不变）
- runStepNodes(): 节点透传 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` / `live` 五字段（签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| unifiedRunNodes | 正向 | 库读史 runs[].nodes 携四字段 → 步行四字段透传 + live=false（`stepFromRecord` 纯复制半程锚） | 库读史源 |
| unifiedRunNodes | 正向 | liveRun.nodes 携四字段 → 透传 + live=true（AC-8 活面标注半边） | 活面源 |
| unifiedRunNodes | 边界 | liveRun=null → 仅库读史步、全部 live=false（既有回归持衡） | null 源 |
| unifiedRunNodes | 边界 | 双源同 `(phase, attempt, step, instanceNo)`（跨 run 同名身份键）→ 两行并存、live 标注各随其源不串扰 | 双源并存 |
| unifiedRunNodes | 边界 | 库读史旧行形态（startedAt=0 / endedAt=null / token null）→ 原样透传零改写 | 旧数据 |
| unifiedRunNodes | 异常 | runs 空数组 + liveRun=null → 空数组（不产生幽灵步） | 空集 |
| runStepNodes | 正向 | UnifiedRunStep 携四字段 + live → RuntimeFlowNode 五字段逐字段等值 | 单节点 |
| runStepNodes | 正向 | 多步混排（workerAgent / toolStep / gate）→ 各节点五字段各自透传、输出序保持输入序 | 多节点 |
| runStepNodes | 边界 | 四字段 0 / null 边界值形态 → 原样进节点零转换零默认 | 边界值 |
| runStepNodes | 边界 | phaseOrder 外相位步 → 不入图（坑位校验回归，四字段不改变出局判定） | 坑位 |
| runStepNodes | 异常 | 载荷含非法 status cast（沿既有 weirdStatus 先例）→ 透传不炸 | 非法载荷 |
| runStepNodes | 组合 | describe「stepFromRecord → unifiedRunNodes → runStepNodes 透传链」：库读史 ChangeRunNodeRecord 四字段 → 最终 RuntimeFlowNode 逐字段一致（前端透传链全程锚，AC-1 / AC-8） | 全链透传 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——纯函数直测；`ChangeRunNodeRecord` / `ChangeStepState` / `LiveRunFace` fixture 字面量直构（含四字段与 live 两态） | 全部用例 |

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

- RunStepNode(): 常驻行（静态耗时 + workerAgent token 段）+ 走秒呈现（组件内 interval、仅活面 running 挂载；props / 节点类型注册零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| RunStepNode | 正向 | 已收口 workerAgent 步（startedAt>0、endedAt>startedAt、token 两值 12_000/3_400）→ `run-step-metrics` 行在场：`run-step-duration` 含 "⏱ " 前缀 + 分级串、`run-step-tokens` 为 "tokens 12k/3.4k"、两段以 " ▸ " 分隔（D10 词面） | 静态双段 |
| RunStepNode | 正向 | 已收口非 workerAgent 步（toolStep / gate / git）→ token 段不渲染、仅耗时段（无 " ▸ " 分隔符） | 分组门控 |
| RunStepNode | 正向 | workerAgent 步 token 缺席（null）→ token 段 "tokens —/—" 渲染在场（em dash） | token 缺席 |
| RunStepNode | 边界 | 0ms 瞬步且非 workerAgent（startedAt==endedAt、无 token 段）→ 整行 `run-step-metrics` 省显（行条件 ≥ 1 段在场） | 整行省显 |
| RunStepNode | 边界 | 0ms 瞬步 workerAgent → 行在场、仅 token 段（耗时段 null 省显） | 瞬步 token |
| RunStepNode | 边界 | 旧数据（startedAt=0 / endedAt=null、非活面 running）→ 耗时段 "-"（hyphen，AC-7 旧库读史形态） | 旧数据 |
| RunStepNode | 边界 | 库读史 status=running 残留（live=false）→ 不挂 timer、耗时段静态 "-"（AC-8 库读史半边） | 库读史 running |
| RunStepNode | 正向 | 活面走秒：live=true && status=running，fake timers 下 advanceTimersByTime(1000) 逐秒推进 → `run-step-duration` 文本随 Date.now()-startedAt 更新（每秒 tick 局部重渲染） | 走秒迭代 |
| RunStepNode | 边界 | 走秒收口回落：running → passed 翻态 rerender 后 timer 卸载、耗时段回落静态分级串（再推进 fake timers 无变化，AC-8 / R1） | 收口回落 |
| RunStepNode | 边界 | 走秒卸载清理：组件 unmount 后推进 fake timers 无状态更新告警 / 泄漏（timer 生命周期绑定组件，R1） | 卸载清理 |
| RunStepNode | 边界 | live=false && status=running → 不挂 interval（推进 fake timers 文本不变） | 不挂载 |
| RunStepNode | 异常 | 异常载荷与常驻行并存（超长 detail / 非法 status cast）→ 渲染不炸、既有锚（run-step-node / run-step-kind / run-step-group / run-step-detail）与常驻行同屏可辨 | 异常载荷 |
| RunStepNode | 组合 | describe「RuntimeFlowNode(+4+live) → 常驻行」：既有头行 / detail HoverCard / 布局锚零回归——常驻行插于标题头行与 detail 行之间（D10 排布） | 排布回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 时钟 / interval | `vi.useFakeTimers()`（含 Date 伪造，沿 use-change-detail.test.ts 既有先例）+ `vi.advanceTimersByTime(1000)` 精确推进 | 走秒迭代 / 收口回落 / 卸载清理 / 不挂载用例 |
| xyflow 上下文 | `ReactFlowProvider` 真件包裹沿既有 renderNode 装置（非 mock） | 全部渲染用例 |
| metrics.ts | 不 mock——真件组合（纯函数依赖最小 mock 原则不适用） | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 子表未列该文件行（ChangeStepState 四字段为类型定义面，见 design「类型定义」表）；以下条目映射自该表声明的类型面变更，断言沿 state_test 既有线面式样扩键，非 grep 源码导出虚构。 -->

- ChangeStepState（类型面）: 四字段 wire 出线——camelCase `startedAt` / `endedAt` / `tokenInput` / `tokenOutput`、Option 缺席出线 `null` 不省键（serde 属性随 struct 既有，AC-1）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ChangeStepState | 正向 | 四字段全值步 `serde_json::to_value` → 线面四 camelCase 键逐键等值（数值口径，AC-1） | 全值线面 |
| ChangeStepState | 正向 | `RunUpdate::Step { step }` 变体出线 → `step["step"]["startedAt"]` 等四键在场（既有 run_update 线面用例随动扩断言——载荷自动跟随） | 变体线面 |
| ChangeStepState | 边界 | running 形态（ended_at / token 两值全 None）→ 线面三键 `null` 不省键（与既有 sessionId / detail 同式） | None 态 |
| ChangeStepState | 边界 | started_at = 0（缺省语义值）→ 线面 `0` 原样（i64 非可选零改写） | 0 值 |
| ChangeStepState | 边界 | token u64::MAX 邻近值 → 线面数值保真（JSON number 面不截断不 panic） | 极值 |
| ChangeStepState | 异常 | 线面 JSON 回读（`from_value`）→ 四字段逐字段还原（Option `null` → None 还原无损） | 往返 |
| ChangeStepState | 异常 | 缺四键旧形态 JSON 反序列化 → Err（活面契约无 `#[serde(default)]`——旧行兼容仅落库 `RunNodeRecord` 面承载，两载体语义分立锚，AC-5） | 旧形态拒绝 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——`ChangeStepState` / `RunUpdate` 字面量直构 + `serde_json` 直测（沿 state_test 既有线面断言式样） | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 子表未列该文件行（emit_node / StepSettle / run_worker 均模块私有，表尾注明示）；以下条目映射自 design「修改文件」表与 D1 / D2 决策声明的行为面，非 grep 源码导出虚构。 -->

- emit_node(): 铸造单点——running emit 铸 `started_at` 且 `ended_at=None`、终态 emit（passed / failed / stopped）铸 `ended_at`、瞬步单 emit started==ended（D1，AC-2）
- run_worker(): Ok 路径终态 emit 前对 `outcome.transcript` 的 TurnDone.usage 求 in / out 单次落值（无论 outcome.status）；Err 路径恒 None（D2，AC-3）
- StepSettle.with_tokens(): token 两值随行终态 emit 的 builder 面（D1；非 run_worker 路径不调用即中性 None）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| emit_node | 正向 | FakeWorker 正常收口的 executor 双 emit 步 → 终态节点 `started_at > 0` 且 `ended_at >= started_at`（wall-clock 关系式断言，绝对值不入断言；AC-2） | 双 emit |
| emit_node | 正向 | `phaseStart` 瞬步单 emit passed → 节点 `started_at == ended_at` 同刻（AC-2 瞬步半边） | 单 emit |
| emit_node | 正向 | 用户停止路径 stopped 收口 → 终态节点 `ended_at` 在场 `> 0`（AC-2 stopped 半边） | stopped 路径 |
| emit_node | 正向 | 工具步 / Gate / Git 步终态 emit → token 两值 None（`with_tokens` 仅 run_worker 调用、构造器中性继承，AC-3 非工步半边） | 非工步遍历 |
| emit_node | 边界 | EvaluatorGate 既有挂起装置下中途 snapshot → running 在途节点 `started_at > 0`、`ended_at = None`（running 恒 None） | 在途捕获 |
| emit_node | 边界 | StepSettle 构造器中性预填（started_at=0 / ended_at=None）经 emit_node 铸造覆写 → 出面无中性值残留（D1 单点覆写锚） | 中性残留 |
| emit_node | 异常 | 调用方预填冲突值（running 态载荷预携 ended_at=Some）→ 铸造按 status 覆写、线面以铸造值为准（铸造不信任调用方预填值——防御锚） | 预填冲突 |
| emit_node | 组合 | describe「emit 铸造 → accumulate → snapshot / 落包出口」：同 run 多步双 emit 序列后，control.snapshot().nodes 与终态出口 nodes（finish_command 输入）同键节点四字段一致（AC-2 / AC-4 交界，walker 为链路发起方） | 全链读取 |
| run_worker | 正向 | FakeWorker 转录注入多 TurnDone（usage 两键各值）→ 终态节点 token_input / token_output = 逐值之和（AC-3 主断言） | 多事件求和 |
| run_worker | 正向 | 会话失败收敛（Failed outcome 但 transcript 在手）→ token 仍按 transcript 求和落值（D2 无论 outcome.status） | 失败收敛 |
| run_worker | 边界 | 转录零 TurnDone / usage 缺席 → 终态节点 token 两值 None（AC-3 缺席半边） | 缺席 |
| run_worker | 边界 | `worker.run` 返回 Err（无 outcome / 无 transcript）→ failed emit 节点 token 恒 None（D2 Err 半边） | Err 路径 |
| run_worker | 异常 | 转录混入非 TurnDone 事件与 usage 值类型漂移（字符串 / 浮点）→ 求和仅计合法 TurnDone 数值、不 panic（`agent::sum_usage_tokens` 语义在消费面的表现，AC-3 / AC-10） | 干扰载荷 |
| StepSettle.with_tokens | 正向 | with_tokens(Some, Some) 随行终态 emit → 终态节点两值落位（builder 正向链路） | 双 Some |
| StepSettle.with_tokens | 边界 | with_tokens 单侧 Some（input 有 / output None）→ 终态节点单侧值、缺席侧 None | 单侧 |
| StepSettle.with_tokens | 异常 | 未调用 builder 的终态步 → token 两值 None（builder 缺席中性——44 调用点零触点锚） | 缺席 builder |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| WorkerAgentPort | FakeWorker 既有假件（预录密封产出 + 请求捕获）扩 TurnDone.usage 注入口——walker 的注入端口依赖，最小 mock 合规 | run_worker / with_tokens 全部 token 用例 |
| ToolsPort / VcsPort | FakeTools / FakeVcs 既有假件沿用（walker 注入端口依赖） | 非工步恒 None / 工具 / Git 步用例 |
| wall-clock（now_millis） | 不 mock 不注入——复用既有生产时钟零新缝（D1）；断言取关系式（started > 0、ended >= started、瞬步同刻、stopped ended 在场），wall-clock 绝对值不入断言 | 全部铸造用例 |
| EvaluatorGate | 既有 armed + Notify 挂起装置（同步原语真件，非 mock） | running 在途捕获用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 子表未列该文件行（accumulate_node 模块私有，表尾注明示）；条目映射自 design「修改文件」表与 D4 决策声明的合并语义，经既有 begin_run → emit → snapshot 累积器读面断言承载，非 grep 源码导出虚构。 -->

- accumulate_node(): 合并单点——`started_at` 无条件保首见（终态行同刻值被丢弃）、`ended_at` / `token_input` / `token_output` some 才覆写、None 臂首见落值（D4，AC-4）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| accumulate_node | 正向 | 同键 running → 终态双 emit（终态行携相异 started 值）→ snapshot 节点 started = running 首见值（终态同刻值被丢弃，AC-2 / AC-4） | 双 emit |
| accumulate_node | 正向 | 双 emit 后 snapshot：ended_at / token 两值 = 终态行值（some 才覆写正向半边） | some 覆写 |
| accumulate_node | 正向 | 瞬步单 emit（首见即终态行）→ 节点 started == ended 自然成立（无需区分首 emit 是否终态，D4） | 单 emit |
| accumulate_node | 正向 | 多键混排（phase / attempt / step / instanceNo 各异）→ 各键独立合并不串扰 | 多键 |
| accumulate_node | 边界 | 终态行 ended_at / token 全 None（running 残留收口形态）→ snapshot 保持首见 None（None 不覆写半边） | 全 None 覆写 |
| accumulate_node | 边界 | 终态行 token 单侧在场（input Some / output None）→ 在场侧覆写、缺席侧保首见 | 单侧 some |
| accumulate_node | 边界 | 同键三 emit 序列（running → running 重发 → 终态）→ started 仍首见、ended 取终态值（AC-4「同键多 emit 后快照与合并规则一致」） | 三 emit |
| accumulate_node | 异常 | 终态行携逆序时间（ended < started）→ 原样覆写零改写（合并不做语义校验——防御值直透） | 逆序时间 |
| accumulate_node | 异常 | 首见行即终态且 ended None → started 落值、ended None（合并层不铸造不补值——构造值直透） | 构造直透 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——真件 `ChangeFlowControl`（begin_run / guard.emit / snapshot 全真组合）；双 emit 序列 `ChangeStepState` 字面量直构（沿 sample_step 既有装置，含四字段两态） | 全部用例 |

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 子表未列该文件行（仅 `mod usage;` 声明 + `pub use` re-export，D3）；条目映射自 design「修改文件」表声明的 re-export 面，沿 lib_test 既有「crate 根导出面锚定」式样承载，非 grep 源码导出虚构。 -->

- crate 根 re-export 面: `agent::sum_usage_tokens` 根路径可达（`pub use usage::sum_usage_tokens`），调用语义与 `usage::sum_usage_tokens` 同一实现（AC-10 唯一实现锚的根面半边）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| crate 根 re-export 面 | 正向 | 经根路径 `agent::sum_usage_tokens(&transcript, key)` 持有并调用 → 编译可达即锚（沿 crate根导出面锚定 既有式样，AC-10） | 路径引用 |
| crate 根 re-export 面 | 正向 | 函数项签名锚（`fn(&[AgentEvent], &str) -> Option<u64>` 形态断言持有根路径函数）→ 签名面与 D3 声明一致 | 签名锚定 |
| crate 根 re-export 面 | 边界 | 空转录 `&[]` 经根路径调用 → None（根路径行为面轻量锚——语义全量归 usage_test.rs 节） | 空集 |
| crate 根 re-export 面 | 异常 | 缺席 key（`""` / 未知键）经根路径调用 → None；同一转录重复调用两次结果一致（re-export 零状态零副作用——防御锚） | 缺键 / 重复调用 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——纯路径 / 调用面直测；`AgentEvent` 字面量沿 event_test 既有装置 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 子表未列该文件行；条目映射自 design「类型定义」表的 RunNodeEntry / RunNodeStateRecord 四字段（类型定义面），断言沿 state_test 既有节点史构造 / 往返式样扩键，非 grep 源码导出虚构。 -->

- RunNodeStateRecord（类型面）: 四字段构造与 serde 线面（camelCase 四键、Option null 出线、往返无损，AC-1 / AC-5）
- RunNodeEntry（类型面）: finish 载荷内联四字段构造（写面与读面同一份字段集，AC-1 / AC-5）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| RunNodeStateRecord | 正向 | 四字段全值 `serde_json::to_string` → camelCase `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` 四键逐键等值（既有节点史线面用例随动扩断言，AC-1） | 全值线面 |
| RunNodeStateRecord | 正向 | to_string → from_str 往返 → 四字段逐字段还原无损（既有往返用例随动扩断言） | 往返 |
| RunNodeStateRecord | 边界 | token / ended None 形态 → 线面三键 `null` 不省键（沿 node_none 两态式样随动） | None 态 |
| RunNodeStateRecord | 边界 | started_at = 0 → 线面 `0` 原样（port 读面缺省语义值零改写） | 0 值 |
| RunNodeStateRecord | 异常 | 缺四键旧形态 JSON 反序列化 → Err（port 读面无 `#[serde(default)]`——旧行兼容仅落库 `RunNodeRecord` 面承载，两载体语义分立锚，AC-5） | 旧形态拒绝 |
| RunNodeEntry | 正向 | 四字段构造 → 与 RunNodeStateRecord 同名字段集逐字段可对齐（写面 / 读面同一份字段集锚，AC-1） | 字段集对齐 |
| RunNodeEntry | 边界 | 四字段 0 / None 边界形态构造 → 直存零转换零默认（payload 面零逻辑分支） | 边界值 |
| RunNodeEntry | 异常 | 防御值构造（started_at 负值 / token u64::MAX）→ 直存零钳制零 panic（载荷面零校验——校验归铸造侧锚） | 防御值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无外部依赖） | 零 mock——类型字面量直构 + `serde_json` 直测（沿 state_test 既有式样） | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 子表未列该文件行；条目映射自 design「修改文件」表声明的「ChangeRunNodeRecord 增四字段（毫秒数口径）+ `From<&RunNodeStateRecord>` 直读透出」（D6，AC-6），非 grep 源码导出虚构。 -->

- ChangeRunNodeRecord（From 投影）: 四字段毫秒数纯 derive 出线（`From<&RunNodeStateRecord>` 直读、零回填零改写）；runs 级 ISO 口径并存不变（D6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ChangeRunNodeRecord | 正向 | DetailStore 种子节点行四字段全值 → detail.runs[0].nodes[0] 四字段毫秒数逐字段等值（AC-6 主断言） | 单节点投影 |
| ChangeRunNodeRecord | 正向 | 多节点 emit 序种子 → nodes 投影保序（库读序 = node_seq 序口径不变） | 多节点 |
| ChangeRunNodeRecord | 正向 | 同一 run 行 startedAt / finishedAt 仍 ISO 串（RFC3339，`iso_from_millis` 单点）与节点级毫秒数并存断言（两口径并存留痕，AC-6 / D6） | 两口径并存 |
| ChangeRunNodeRecord | 边界 | 旧行形态种子（started_at=0 / ended None / token None）→ 出线 `0` / `null` 原样零回填（AC-6 旧数据半边） | 旧行形态 |
| ChangeRunNodeRecord | 边界 | token 单侧在场（input Some / output None）→ 线面单键数值单键 null | 单侧 |
| ChangeRunNodeRecord | 边界 | 空节点 run（nodes 空 vec 种子）→ runs[].nodes 空数组（零节点 run 合法形态） | 空集 |
| ChangeRunNodeRecord | 异常 | arm_nodes_fault 注入（list_run_nodes Err）→ runs 降级空数组零 panic（沿既有 fault 注入装置；四字段不改变降级路径） | 读面失败 |
| ChangeRunNodeRecord | 组合 | describe「RunNodeStateRecord → ChangeRunNodeRecord → wire 线面」：投影结果 `serde_json::to_value` camelCase 四键与活面 ChangeStepState 线面同形（UnifiedRunStep 单一形状锚，AC-1 / AC-6） | 投影线面 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| store 读端口 | DetailStore 既有 in-test 假件（可编程 runs / run_nodes / run_edges 序列 + arm_* fault 注入）——`change_detail` 的注入端口依赖，最小 mock 合规；查询投影逻辑真件 | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

#### 待测功能

<!-- 本节自我映射（测试文件即被测文件）。design.md 公共函数 / API 子表未列该文件行（step / step_at 为测试内私有 fixture 构造器）；条目映射自 design「修改文件」表与 D11 决策声明的种子面与 golden 面（AC-9），非 grep 源码导出虚构。 -->

- step / step_at（fixture 定值注入）: 四字段定值注入（默认形态 + 两态 overrides）、零 wall-clock 进 corpus（D11）
- golden 断言面: detail 全量序列化（runs[].nodes 在内）含四字段 camelCase 键；时间 / token 有无两态样本在案（AC-9）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| step / step_at | 正向 | 定值注入（overrides 显式四值）→ 经 finish_command 落库读回逐字段等值、project_detail 序列化含对应键值（定值直达 golden，AC-9） | 定值注入 |
| step / step_at | 边界 | 默认形态（不传 overrides）→ token None / ended 按步状态派生定值（fixture 缺省确定性——两次构造逐字段一致） | 缺省派生 |
| step / step_at | 异常 | overrides 混防御值（started_at 负 / 0、token 单侧）→ 直达 golden 零改写零 panic（种子面零逻辑分支） | 防御值 |
| golden 断言面 | 正向 | 全量语料 project_detail 序列化与 golden 文件逐值一致；同机二次运行输出稳定（零 wall-clock——确定性锚，AC-9） | 全量比对 |
| golden 断言面 | 正向 | 时间两态样本各 ≥ 1：正常起止双值步 / 瞬步 started==ended / running 收口在途 ended=None → golden 各形态在案 | 时间两态 |
| golden 断言面 | 正向 | token 两态样本各 ≥ 1：WorkerAgent 步带值 / 工具步 None → golden nodes 行 tokenInput / tokenOutput 两态在案（AC-9） | token 两态 |
| golden 断言面 | 边界 | 瞬步样本定值 started == ended → golden 节点行两键同值（0ms 形态入 golden）；在途样本 `endedAt: null` 出线不省键 | 同刻 / null |
| golden 断言面 | 异常 | 漂移守卫：断言深查 runs[].nodes[] 四 camelCase 键在场（旧 golden 未重写时显式红、非静默通过）；既有规模语料（超长 detail / 多节点）+ 四字段注入 → 全量序列化零 panic | 漂移 / 规模 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| native_db 工作区库 | 真件 Store（tempdir 临时库）+ store run 域操作面植入语料（种子经 finish_command 落库，禁裸表插桩——D11 条款）零 mock | 全部用例 |
| wall-clock | 不入语料——种子直构 ChangeStepState 定值、零 emit_node / now_millis 触点（D11 确定性口径）；golden 重写流程（DESKTOP_GOLDEN_REWRITE=1）归桌面端执行、不入用例 | 全部用例 |

---

## 不可测试项

| 模块 | 原因 |
|------|------|
| `packages/desktop/src/views/changes/flow/types.ts` | 纯类型声明扩展（`RuntimeFlowNode` interface 增五字段），无独立可执行面；行为覆盖由消费侧承载——`run-state.test.ts`（runStepNodes 透传）与 `run-step-node.test.tsx`（渲染消费），类型正确性由 TypeScript 编译面保证 |
| `packages/desktop/src/types/generated/bindings.ts` | specta 生成物零手改；`ChangeStepState` / `ChangeRunNodeRecord` TS 镜像一致性由 bindings 守卫（桌面端静态管线）拦截漂移，无单元测试路径 |
| `packages/desktop/package.json` | 版本号交付（0.5.10 → 0.5.11，D12），无被测逻辑；归桌面端静态管线检查域 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/README.md` | 覆盖面矩阵文档随动（AC-9「步级时间与 token」维度行说明面），非可执行代码 |
