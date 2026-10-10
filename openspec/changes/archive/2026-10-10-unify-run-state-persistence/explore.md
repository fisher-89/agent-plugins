# 统一 run 状态落库探索（useChangeDetail / useChangeFlowRun 口径一致）

日期:2026-10-09 · 状态:范围已决(见范围决议),待开 change

## 动机

详情页两套流程数据各持一个真相源:`useChangeDetail`(库派生面)与
`useChangeFlowRun`(内存注册表面)。运行中页面实时显示的内容——run 状态机、
终态记因、步节点史——完成后查询(显式 refresh / 重挂)一概不可见;且库侧
`active_phase` 在 run 中途死亡时永久悬挂撒谎。诉求:**运行中返回的状态一并
落库,保证页面实时更新和完成后查询内容一致**。

## 现状:一份数据,两个真相源

```
              ┌─────────────────── change-detail-view ───────────────────┐
              │   useChangeDetail                useChangeFlowRun         │
              │   拉取面:显式 refresh           快照恢复 + Channel 实时流  │
              └────────┬──────────────────────────────┬──────────────────┘
                       ▼                              ▼
              get_change_detail            change_flow_state / watch
                       ▼                              ▼
           queries::change_detail          ChangeFlowControl(进程内注册表)
                       ▼                              ▲
              redb store(落库面)                walker ─ publish(RunUpdate)
        ChangeRecord.status      ◀─写面─ phase_start / phase_log
        ChangeRecord.active_phase         │
        PhaseRecord[](attempt 评估史)     └─ steps / gates / 决策 → 只 emit,零落库
```

- 库面读链:`change-detail-view.tsx` → `useChangeDetail` →
  `commands/changes/mod.rs:get_change_detail` → `workflow/queries/detail.rs:change_detail`
  (record + phase_records 聚合,`active_phase` 直读 `ChangeRecord`)。
- 内存面读链:`useChangeFlowRun` → `commands/change_flow/mod.rs` →
  `orchestration/control.rs:ChangeFlowControl`(键 = `(root, change)`,快照
  `snapshot()` 只含 run_id/status/phase/attempt/ask,**不含 steps**)。
- 前端两处拼缝:图 = `buildFlowGraph(detail, runNodes)` 双源合成;终态迁移时
  `useRunViewEffects` 触发一次显式 refresh,图「回落 ChangeDetail 派生规则」
  (run-state.ts 头注释明说不持久化)。

## 口径差清单

| 数据 | 运行中页面显示 | 完成后查询 | 一致? |
|------|--------------|-----------|------|
| run 状态机(running / waitingAsk / …) | ✓ 注册表 | ✗ 库里无此概念(只有 active/archived) | ✗ |
| run 终态 + reason | ✓ 冻结的 run.state | ✗ 广播完即除名 | ✗ |
| 步节点史(executor / 决策 / gate / 工具步) | ✓ 前端 reducer 内存累积 | ✗ 零落库,只剩 attempt 级 verdict | ✗ 粒度断崖 |
| phase / attempt 相位 | Step 事件跟随 | `record.active_phase` | ⚠ 见悬挂问题 |
| 会话事件 | liveEvents 内存缓存 | 会话转录库(attempt 槽位可达) | 半一致(形态不同) |
| attempt / verdict / checklist | 实时面不显示 | ✓ PhaseRecord | ✓(完成后才有) |

附:重挂恢复(同进程)`initialRunState` steps 恒 `[]`——步史在重挂后同样
丢失,只有后续 update 会进表;`RunUpdate::Step` 的信息量在内存面也是
易逝品。

## 两个关键认识

**① active_phase 悬挂(最尖锐的不一致)。** `phase_start` 写入、`phase_log`
清位(`workflow/src/state.rs:88` 注释「开相在位、落账清位」)——但 run
中途死亡(会话失败、verdict 解析失败、用户停止)时 `phase_log` 永不执行,
`active_phase` 停在 `Some`。此后任何时刻查详情,头部都渲染「运行中 ·
phase · attempt」,而实际没有 run 在跑。库在撒谎且永远撒下去。

**② 这是写死的决策,不是实现遗漏。** `control.rs` 头注释明说:「run 生命
周期由本注册表承载(**不建 flow_runs 表**);run 终态即除名,桌面重启后
run 消失(重新发起自 active_phase 续走)」。本诉求等于翻掉这条决策,应
作为决策翻案显式立项,而非当作补丁。

## 方案空间(落库时机光谱)

```
  ┌──────────────┬──────────────────────┬──────────────────────┐
  │ A. 终态一次性 │ B. publish 写透       │ C. 只并读面不落库     │
  │ finish 时整包 │ 每条 RunUpdate 随发随写│ detail 合并内存快照   │
  ├──────────────┼──────────────────────┼──────────────────────┤
  │ 库增 last_run│ 库增 runs + run_steps │ 库零改动              │
  │ + steps 整包 │ 两张表                │                      │
  │ 崩溃丢过程史 │ 崩溃也留半程史         │ 终态即除名 → 完成后   │
  │(崩溃=重启    │ 写放大:每步一事务?    │ 仍不一致;重启丢 run  │
  │ = run 已死,  │ 需批量/合并策略       │ 基本不满足诉求        │
  │ 可接受?)     │                       │(列出仅作对照)        │
  └──────────────┴──────────────────────┴──────────────────────┘
```

当前倾向:**A 的骨架 + B 的终态语义**——

- walker 在 `finish` 时一次性落整包:它自己 emit 过全部步,收齐在 walker /
  RunGuard 零成本;单事务写 run 记录(run_id / status / reason / 起止时刻)
  + steps 全列,避免每步写放大。
- `get_change_detail` 聚合最近 run(ChangeDetail 增字段)——注意 golden
  线面契约:字段演进允许但走 golden 显式重写流程,形态(null + ISO 串、
  纯 derive)仍冻结;时间戳按既有口径 i64 毫秒落库、ISO 串出线。
- 前端 `runStepNodes` 改可从库中 steps 派生 → 实时面与完成后面走**同一条
  派生路径**,而非结果碰巧一样。
- 终态时处置 active_phase:清位或转「上次运行停在 X」静态记录,杀掉悬挂。
- 启动标定:开库/启动发现库中 run 状态非终态 → 标 interrupted(重启后
  run 客观已死,库不能继续说 running)。

### 写缝位置(candidate seam)

`ChangeFlowControl` 刻意「进程内无 IO」。终态一次性写可完全留在 walker 侧
(finish 调 store port),control 保持纯洁——倾向此路;若走 B 写透,得经
命令层 `ChangeFlowSink`(已持 store 句柄)或给 control 注 port,crate 边界
改动更大。

## 开放问题(决定方案形状)

1. **一致到什么粒度?** 只到「run 终态 + reason」(最小面,图完成后仍回落
   attempt 级),还是到「步节点史」(图完成后仍显示 executor/gate 节点,
   换终态样式)?后者才是页面上肉眼可见的一致。
2. **completed 之后 steps 要不要常驻渲染?** 「完成后图回落」也可以是刻意的
   产品语义(终局视角只看评估史)。若是,「一致」只指 header 状态面,范围
   小很多。
3. **多次 run 历史都留还是只留 last_run?** `run-<millis>` 每次发起一个;
   全留 = 运行史审计面(哪个 attempt 属于哪次 run),只留 last_run 轻得多。
4. **中途停等态(waitingAsk / waitingConfirm)落不落?** 落了重启后也无人
   应答(应答通道在内存);不落则库里 run 状态只有 running + 三终态。

## 范围决议(2026-10-09,用户拍板)

原开放问题四问全数拍板:

1. **步史落库口径 = 有内容的步**:只落 agent 阶段(Executor / Evaluator /
   Decision)与脚本阶段(StaticCheck / TestExecution)——封闭集 10 变体留
   5;流程面步骤(PhaseStart / PhaseLog **及推断纳入的三门**
   VerdictGate / RetryGate / WhitelistGate)不落。
2. **常驻渲染**:完成后图不回落——步节点从库派生常驻,与实时面同一条
   派生路径,「图回落 ChangeDetail 派生规则」语义退场。
3. **全史保留**:不截 last_run,runs + steps 全量留存(运行史审计面)。
   attempt 天然归属唯一 run——phase_start 的 attempt=max+1 分配保证跨 run
   不撞号,全史叠加在同一列面即可分层。
4. **停等态不落**:库中 run 状态词汇 = running + 三终态(completed /
   stopped / failed);waitingAsk / waitingConfirm 仅存内存——应答通道
   (oneshot)本就活在进程内,落了重启也无人接。

### 决议牵出的形状

- 库面:`runs`(run_id / change / status / reason / 起止时刻)+
  `run_steps`(决议 1 过滤后的步子集,含 sessionId / detail)两张表;run
  行 **start 时建**(status=running,支撑启动标定)、finish 时终态更新 +
  步整包落(每 run 两写,零每步写放大)。启动 sweep:残留 running 标
  interrupted。
- 出线:ChangeDetail 增 runs + steps 字段(golden 契约字段演进,显式重写
  流程;时间戳 i64 毫秒落库、ISO 串出线不变);前端 `runStepNodes` 数据源
  统一为「库读史 ∪ 实时流」,键 `phase:attempt:step` 天然不撞。
- 图常驻渲染取**全史叠加**(非仅最近 run):与决议 3「多次尝试历史保留」
  同构;在飞 run 的活步继续叠加其上。

### 残余微问题(留 design 相位,不阻塞 proposal)

- 三门(VerdictGate / RetryGate / WhitelistGate)归「流程面忽略集」是
  探索推断,proposal 时确认。
- 步过滤位置:walker emit 时分流(emit 两面:广播全量 / 落库过滤)vs
  finish 整包时过滤——影响 control/walker 接口形状。
- run_steps 行主键与序(库内自增 id 按 finish 整包序,还是 seq 随 emit
  序)——重挂恢复时 live 面与库面的拼接依赖稳定序。

## 残余问题原则(2026-10-09,用户拍板)

> 落户与回显口径一致;最好接口层查询也能合并;运行时仅做通知刷新,由
> 客户端自行请求。

一条总原则收掉残余微问题,同时定下读路架构(推拉反转):

```
  现状(推送载荷)                      目标(通知 + 拉取)
  ─────────────────                    ─────────────────
  RunUpdate 携带步/状态/ask 载荷        RunUpdate 降位为变更通知
  前端 reducer 累积(状态机镜像)        客户端收通知 → 自行重查
  两钩拼接(detail + flow state)        接口层合并查询一次出统一视图
```

- **口径一致(微问题 1+2 落定)**:落户子集 = 回显子集,同一份过滤词汇
  (5 留 5 忽略)单点定义;不存在「广播全量 / 落库过滤」的两面 emit——
  实时面与库面共用一个过滤位(候选:read 层单点过滤,walker / control
  零过滤逻辑)。
- **接口层合并(读路统一)**:合并查询落接口层——get_change_detail(或
  其扩展)一次返回「库读史 ∪ 内存在飞 run」统一视图,客户端不再双命令
  拼接。写侧仍是每 run 两写(start 建 running 行 / finish 终态 + 步整包),
  合并发生在**读时**——零每步写放大的前提不变。
- **运行时仅通知(推拉反转)**:
  - 累积器移驻服务端:步表从前端 reducer 移入 `RunEntry`(注册表条目
    自持步列表,通知触发重读);ask / 停等态同走注册表内存面(决议 4
    不落库,但合并查询可见)。
  - 前端 `applyRunUpdate` 状态机镜像 / liveEvents 累积 / seq 去重大面积
    解散,客户端态退化为「查询缓存 + 失效重取」;两钩并一
    (useChangeFlowRun 缩为控制动作 + 通知订阅)。
  - 抽屉实时转录面:liveEvents 缓存可顺势退役——转录库本就流式落库,
    通知 → 重查转录(use-session-transcript 已有查询面),口径进一步
    统一到转录库。
  - stop / confirm / answer 控制面不变(本就是请求-应答,非推送)。
- **微问题 3 随读路统一收口**:合并查询按 (run 起始, emit 序) 稳定排序;
  库面 seq 取 emit 序、finish 整包保序落。
- **新增设计考量**:通知高频面(每步 / 每会话事件)需客户端合并去抖,
  或通知侧粗粒度 coalesce——留 design 定节流策略。

## 关键文件锚点

- 前端拼缝:`packages/desktop/src/views/changes/change-detail-view.tsx`
  (`useRunViewEffects` 终态 refresh 回落)
- run 视图模型:`packages/desktop/src/views/changes/flow/run-state.ts`
  (steps / liveEvents 内存累积,头注释声明不持久化)
- 内存注册表:`packages/desktop/src-tauri/crates/core/orchestration/src/control.rs`
  (不建 flow_runs 表决策原文;`snapshot()` 不含 steps)
- walker 终态单点:`.../orchestration/src/walker.rs:walk_run` → `guard.finish`
  (终态出口唯一,落包缝)
- 库面聚合:`.../workflow/src/queries/detail.rs:change_detail`
  (active_phase 直读;字段演进走 golden 显式重写)
- 状态词汇:`.../workflow/src/state.rs`(ChangeStatus 二值 vs
  ChangeRunStatus 六值,维度不同)