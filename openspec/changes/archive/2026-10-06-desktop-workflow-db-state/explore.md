# 探索：desktop 流程状态去 workflow.json 化，搬 workspace db

- 日期：2026-10-06
- 状态：结论已拍板（见「拍板决定」），待 phase-proposal 收敛
- 拟议 change 名：`desktop-workflow-db-state`

## 背景与动机

desktop 当前以 `openspec/changes/<name>/workflow.json` 为 change 流程状态唯一载体
（workflow_type / eval[] / active_phase / file_log[]）。该文件是三方共写的共享形状：

- 插件 MCP 工具（CLI 工作流）：change_create / phase_next / phase_start / phase_log /
  backtrack / change_files / workflow_files
- 插件 hooks：PostToolUse record-files（file_log 归账）、sweep-phase、protect-files
- Desktop Rust：`core/workflow` 写面（create / phase_next / phase_start / phase_log /
  backtrack / decision_log），walker 经 LocalToolSteps 进程内直调

痛点：

1. **双实现锁死共享形状**——desktop 已有相位机 Rust 端口（evaluator prompt 明令禁止
   agent 调 MCP phase 工具，落账由桌面代写），两套写面靠 JSON 文件形状对齐；
   `persist.rs` 的 raw Value 保形改写（W2/W3）只为不丢插件未知字段。
2. **无事务**——JSON 读-改-写，多窗口 / 插件 hook 并发共写有竞态面。
3. **与插件版本耦合的 schema**——历史痛点：CLI 2.10.39 读不了 file_log 版
   workflow.json、report 500 字上限读挂（见 memory）。
4. **list 全量扫盘 + 逐个 parse**（含 archive），fs 即数据。
5. **run 状态只在内存**（ChangeFlowControl），重启即丢。

已确认的关键事实：

- desktop 门禁不依赖 file_log：walker 给 evaluator 的变更上下文走 DiffContextPort
  （git diff），file_log 在 desktop 侧只是详情抽屉展示（数据来自插件 hook 在 spawned
  agent 里写的）；`persist.rs` 明言「file_log 零触点」。
- workspace db（app-data 下 `workspaces/{name}-{hash}.redb`）已有成熟落点：
  SessionRecord / SessionEventRecord / AgentRunRecord / ExploreRecord，native_db +
  native_model 版本治理、i64 unix millis 时间戳、写事务内 max+1 主键。
- **先例**：ExploreRecord 即「记录在 db（身份/时间），内容在磁盘笔记」双载体——
  change 状态搬库与其同构（状态 db、产物磁盘）。

## 拍板决定（2026-10-06）

| # | 分叉 | 决定 |
|---|------|------|
| 1 | CLI 互操作 | **接受双向墙**：desktop 不再兼容 workflow.json（不读不写）；db-backed change 对 CLI 工作流不可见，反向亦然 |
| 2 | file_log 接替 | **无实质依赖，直接降级到 git diff，不建 FileOpRecord**；变更文件上下文继续走 DiffContextPort |
| 3 | step 落库范围 | **(a) 相位机步骤历史审计**（phase_next/start/log/backtrack、static-check、test-execution 落库可查）；**不做 (b) run 崩溃恢复**——run 状态仍驻内存 ChangeFlowControl |
| 4 | 双源读 | **不存在**：状态单源 db；磁盘只承载 markdown 产物。workflow.json parse 路径整体退役 |
| 5 | watch crate | **保留 crate**（将来其他监听场景）；对 workflow.json 的订阅消费点随写面退役 |
| 6 | 归档语义 | db status 位 + 目录改名（加日期前缀）双写；db 主键 name 不随目录改名变 |

## 目标形态

```
desktop change:
  磁盘: openspec/changes/<name>/        ← 只有 markdown 产物
        (proposal/design/tasks/specs/reports/explore)
  db (workspace 库, app-data):
    ChangeRecord      { name(PK), workflow_type, created_at, status(active|archived),
                        active_phase: Option<{phase, attempt, start_at}> }
    PhaseRecord[]       { id(PK, 写事务内 max+1), change, phase, attempt,
                          verdict, report, skipped, stale,
                          backtrack_to/backtrack_reason,
                          executor/evaluator/decision session 槽位,
                          start_at, timestamp }
    ChecklistItemRecord[] { 打包主键 (phase_id as u128) << 64 | item_index,
                            item, pass, evidence }
    StepRecord[]      { run_id, change, step_kind(phase_next|phase_start|phase_log|
                        backtrack|decision_log|static_check|test_execution),
                        status, 时间戳, 输出摘要 }   ← 审计用，不做恢复

  CLI change（有 workflow.json 的）: desktop 不再 parse；
    目录存在 → 产物以文档形态可见（同构现 v0 路径），无状态面
```

收益对号：

- persist.rs raw Value 保形杂技退役（db 自持形状，字段演进走 native_model 版本）
- redb 单事务原子落（eval 追加 + stale 翻转 + active_phase 更新一步完成）
- 与插件 schema 解耦，CLI 版本兼容类痛点消失
- PhaseEvalRecord session 槽位直接 join SessionRecord/转录单表，详情抽屉
  transcript 的 sourceRef 逆查变库内一等查询
- 步骤历史可查（新增能力）

### 数据模型细化（2026-10-06 补）：checklist 与 PhaseRecord 分离

checklist 视作**相位内产物**独立存储，不内嵌 PhaseRecord（拍板）：

- **先例同构**：`SessionRecord → SessionEventRecord` 的父子行 + 打包键模式原样
  复刻；`phase_id` 本身是 i64，打包键 `(phase_id as u128) << 64 | item_index`
  无需 hash64，同相位内 item 序即自然序（保 evaluator 输出顺序）。
- **状态/产物分面**：PhaseRecord 持相位状态（verdict/attempt/stale/回溯标记/
  session 槽位），checklist 子表持评估证据——与「产物独立于状态」原则对齐。
- **形状演进解耦**：checklist 条目（item/pass/evidence，evidence 可长文本）走
  自己的 native_model 版本链；PhaseRecord 保持紧凑平直字段。
- **写时机不变**：phase_log 仍在一个 redb 事务里落 PhaseRecord + 全部
  ChecklistItemRecord——拆存储形状，不拆写原子性。
- **report 留内联**：只分 checklist，report（≤2000 字）保持 PhaseRecord 内联；
  desktop 自有 schema 后与插件 500 字读侧上限彻底解耦。
- **(change, phase, attempt) 唯一性**：写事务内查重（store 惯例，
  同 AgentProviderRecord.name）。
- **线面不变**：detail DTO 的 checklist 仍按 eval 条目聚合出线，queries 层
  从子表重组——存储级拆分，线面形状不动。

## 范围边界

**做**：

- store crate 加 ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord
  （workspace 维度；checklist 为相位内产物独立子表，见「数据模型细化」）
- `core/workflow` 写面重定义到 db（phase_next gate 判定 / phase_start / phase_log /
  backtrack / decision_log / create）；parse 模块（detect / workflow_file）退役
- queries（list / detail）改 db 读；磁盘扫描保留为「产物发现」（目录存在即 change，
  db 缺记录 → 文档形态展示）
- orchestration：steps 直调改 db 写面；snapshot 改 db 读；file_log 相关 AC 删除
- watch 的 workflow.json 订阅点退役（crate 保留）
- 归档命令双写（db status + 目录改名）
- stats 命令读面改 db（现读 workflow.json 的维度逐一核对）
- 前端：inventory 代际徽章（V0/V1/V2）与「workflow.json 无法解析」警示退役；
  detail 的 fileLog 展示区移除；bindings 重生成
- golden：detail 线面字段演进（inventory / fileLog 等）走 golden 显式重写流程
  （wire contract 冻结记忆：形态仍冻结，字段演进显式重写）
- 测试面：golden corpus（v1-b/v1-c/v2-a fixtures）随 parse 退役收缩或转标 legacy

**不做**：

- 历史数据迁移（旧 change 保持磁盘原样，desktop 以文档形态展示其产物）
- FileOpRecord / 文件清单记录（git diff 承接）
- run 崩溃恢复（ChangeFlowControl 仍驻内存；重启后 active_phase 留 db，
  下次 run 由 phase_next 依 eval 历史重算）
- 插件 / openspec CLI 侧任何改动（CLI 工作流照旧走 workflow.json）
- watch crate 删除

## 影响面速查

- crates：core/workflow（parse 退役、write 重做、queries 重做）、infra/store（新表）、
  core/orchestration（steps/snapshot 接线）、src/commands（change_flow/changes/stats/
  watch）、前端 views/changes + bindings
- specs（约 10 个 desktop spec 受波及）：desktop-workflow-write-face（重定义）、
  desktop-change-parse（退役或收缩）、desktop-change-orchestration（写触点 / 节点
  状态派生源 / file_log AC 改写）、desktop-change-create、desktop-change-queries、
  desktop-change-flow-view、desktop-corpus-regression、desktop-workspace-store（新表）、
  desktop-data-dimensions（change 状态归 workspace 库）、desktop-file-watch
- 桌面版本：用户可见变更，需 bump

## 遗留小问题（proposal 时定）

1. **发现语义细则**：列表 = db 记录 ∪ 磁盘目录（推荐：磁盘目录存在即入列表，db 缺
   记录 → 文档形态；与「双向墙」不冲突——产物 markdown 本就是共享载体）。同名共存
   （db 记录 + 磁盘 workflow.json 并存）以 db 为准。
2. **StepRecord 摘要字段面**：审计视图需要多大信息量（输出全文 vs 摘要 vs 引用）。
3. **time 口径**：db i64 unix millis（store 惯例），出线保持 ISO 串 + null
   （detail 线面既有形态），转换收 queries 层单点。
4. **unparsable 路径**：随 parse 退役（db 行 native_model 解码失败另有 StoreError 面）。
