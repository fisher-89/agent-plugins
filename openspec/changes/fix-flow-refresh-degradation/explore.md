# 流程图刷新降级修复探索(fix-flow-refresh-degradation)

2026-10-15 探索定稿。背景:桌面端发起 run 后流程图"不自动刷新"问题的真机取证与修复方案收敛。

## 一、证据链(真机取证,2026-10-15)

手法:`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port` 起 `tauri dev`,CDP 驱动真实 UI 发起 run,逐秒采样 DOM + 并行直接 invoke 后端对照(memory: project-cdp-repro-desktop)。

三因素分解:

1. **长工具步静默是设计使然**——test-execution(1765 测试≈5min)、mutation-test、agent 会话期间零通知,节点仅在步 starting 时上图一次。发起后 ~1s 首刷新链路证实是活的(节点出现、徽章翻"运行中")。
2. **缺陷(复现一次)**:步迁移写入突发(append_step×2 + record upsert)与通知触发的重查同刻竞争,该轮 `get_change_detail` 失败 → 前端 `apply({detail:null})` 整页塌兜底(nodes=0、无徽章、无停止按钮),到下一条通知才自愈。迁移间的更新被吃掉。
3. **吞错面放大**:`queries/detail.rs` `store.get_change(id).ok().flatten()?` 把读 Err 降级成 None → 前端呈"未找到该 change"(运行中!);`loadUnifiedView` 对 `!unified.detail` 清空整页,且 `activeRun:null` 连锁把 `activeRunPresent` 翻 false → `change_flow_watch` 订阅被 effect cleanup 拆除。

用户现场(库取证):09:24 发起 run-1792026966457,test-execution attempt4 通过、mutation-test 通过、phaselog 落账,停在相位间确认;8 分钟静默观感后用户点停止。

## 二、修复方案(定稿)

### 层 1:失败轮降级契约(核心)

- **保 last-good**:重查失败(Err / unified 空)且 detail 在手时,不清 detail/activeRun/artifacts,不塌整页;内联"刷新失败"提示(非整页 fallback)。首载失败(无可保)维持兜底页不变。
- **有界重试(已定)**:3 次,退避 1s / 2s / 4s;穷尽后静默回落到"靠下一条通知兜底"。
  - 理由(终态竞态洞):Finished 是 run 最后一条通知;walker 写序 `run_finished(整包落库) → guard.finish(发通知)`(walker.rs 已核验),300ms 去抖后的终态重查若失败且只靠通知恢复 = 永无下一条 → 必须自带重试。
  - 取消语义(已定):重试计时器与去抖**同生命周期**——任何 tick 变化(新通知、手动刷新、换 root/id、卸载)即作废挂起的重试。重试是"自动补发的刷新意图",与通知/手动刷新共用同一条取消通道,任一时刻最多一个待生效的刷新意图;新意图天然取代旧重试,不产生交错重复查询。现有 `useDebouncedBump` 卸载清计时器已覆盖"离开页面"半边,补"新意图取代旧重试"半边。重试期间面板动作(停止/确认/应答)为独立 invoke,不受取消语义影响。
- 保 last-good 期间面板操作(停止/确认/应答)照常——动作 invoke 不依赖前端态,无风险。

### 层 2:后端错误语义分流(观测性)

`get_change_detail` 出线区分:

- **None = 真"未找到"**(记录缺席)→ 兜底页(现语义不变);
- **Err = 读失败**(store 错误串)→ 前端走层 1 保 last-good + 内联提示。

消掉 `change_detail` 内 `get_change` 的 `.ok()` 吞错。效果:同类竞态下次发生时错误串自现,根因(redb 读事务 vs 写并发的瞬时错?)不阻塞本方案、由观测面浮出。

### 层 3:订阅粘性(随层 1 自然解决)

`activeRunPresent` 挂在单轮查询结果上;层 1 保 last-good 后自然粘住不闪,仅补测试验证。不做"mount 即订阅"等独立改造。

### 层 4(已定:拆出,不在本 change)

长步活性反馈:节点携 started_at + 前端渲染"已运行 XmYs"计时。动 `ChangeStepState` 活面时间戳 + snapshot 投影 + DTO(**change 域 golden 显式重写**)+ 节点渲染,纯 UX 增强,独立 change 承载。

## 三、specs 触面

- `desktop-change-queries`:None/Err 语义分流条款;「详情标示『运行中』SHALL 与实际在飞 run 一致 / MUST NOT 假象」条款**保持原强度**,仅追加一行指向 `desktop-change-flow-view` 降级例外的引用。
- `desktop-change-flow-view`:「通知仅失效信号、查询结果权威」维持;追加通知重查的失败降级条款(保 last-good + 有界重试 + 终态重试兜底),陈旧容忍措辞挂这里(已定,**信号界限为主、时间界限为辅**):
  - 主条款(信号界限):失败轮保 last-good 期间以最后一次成功查询为权威;陈旧窗口内 MUST 呈现"刷新失败"内联提示,MUST NOT 伪造新鲜态;
  - 辅条款(时间界限):陈旧窗口有界——≤3 次退避(1s/2s/4s)内自愈,或下一条通知到达即恢复,最终一致。
  - 立法辨析:原「MUST NOT 假象」杀的是**无界、静默**的假态(archive 后挂"运行中"的旧 bug);本例外豁免的是**有界、带失败提示、收敛保证**的过渡态——两者性质不同,措辞上不稀释原条款。

## 四、测试策略

- jsdom 注入"第 N 轮 invoke 失败 / 返回空"验证降级契约、重试节奏与取消语义(单测层稳定复现);
- 真机竞态不可稳定复现,**不作为 AC**;修复后用 CDP 手法做一次真机走查(发起→长步→迁移→收口全程图面不塌、终态不悬)。

## 五、遗留开放问题(进 proposal)

1. store 读错根因——层 2 落地后靠错误串观测,不在本 change 钉死。

(原开放问题 1 重试取消语义、2 陈旧容忍措辞已于 2026-10-15 定稿,分别并入层 1 与 specs 触面。)
