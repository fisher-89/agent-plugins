# 测试设计: desktop-drawer-session-column

> **日期**: 2026-10-04

---

## 验收范围

<!-- 逐条映射 proposal.md 验收标准；`被测文件或模块` = 承载用例的测试文件（单值）。
     跨模块组合用例挂靠链路入口模块 per-file 章节（2.10.44 模板无集成测试层）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 打开任意节点/列头抽屉：左列为会话信息区（约 60% 宽，元信息 + 转录拉满滚动），右列为【本站文档 / 评估记录 / 文件清单】三分节列内滚动；抽屉宽约 960px 且 `max-w-[85vw]`；列头与 ToolStep/Gate 选中左列呈空态占位，双列结构不跳变（detail-drawer.test.tsx 覆盖） | `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` |
| AC-1 | （同上——左列恒渲染与空态占位渲染面半边：空 roleRefs → `drawer-session-empty`、转录容器填充宿主列高度） | `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx` |
| AC-2 | 点击 active 节点：呈 executor / evaluator / decision 三转录 tab，sessionId=null、sourceRef=`<change>/<phase>/<role>/<attempt>` 反查；已开跑角色重放已落库转录（含进行中会话已流出部分），未开跑角色呈「（暂无该会话转录）」空态；全程不发起实时事件流订阅（detail-drawer.test.tsx 覆盖） | `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` |
| AC-3 | `useSessionTranscript` 暴露 `SessionSummary`（row + stats + turns）；左列呈现 session id 与运行状态徽章等元信息（具体字段清单 design 定夺）（session-transcript-panel.test.tsx 覆盖） | `packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts` |
| AC-3 | （同上——`SessionMeta` 元信息呈现半边：id 截断 + title 全量、运行徽章、轮数、token 合计、summary null 不渲染） | `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx` |
| AC-4 | WorkerAgent 节点（运行中或已收口）上无「查看会话」按钮，`onOpenSession` prop 与接线删除；点击节点本体即打开抽屉（change-flow-graph.test.tsx / run-step-node.test.tsx 覆盖） | `packages/desktop/src/views/changes/flow/run-step-node.test.tsx` |
| AC-4 | （同上——`onOpenSession` 图层注入删半边：`toChartNodes` runtime 分支 data 收敛 `{ node }`、节点点击上抛不变） | `packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx` |
| AC-5 | `FlowNodeKind = 'eval' \| 'active'`；`collectInterrupted` / `InterruptedBody` / dashed 分支删；`NODE_PRECEDENCE` 收敛两段；含 `interrupted[]` 留档的存量 change 详情图上无 interrupted 节点、无编译残留（graph.test.ts 等随动全绿） | `packages/desktop/src/views/changes/flow/graph.test.ts` |
| AC-5 | （同上——事件节点渲染面半边：`InterruptedBody` / dashed 回退分支删、`nodeClass` 收敛两分支） | `packages/desktop/src/views/changes/flow/flow-event-node.test.tsx` |
| AC-5 | （同上——素材挂载降级链半边：`NODE_PRECEDENCE` 收敛 `['eval', 'active']`） | `packages/desktop/src/views/changes/flow/attachments.test.ts` |
| AC-6 | `Workflow.interrupted`、`ChangeDetail.interrupted`、线面 `InterruptedEntry` 删除；含 `interrupted[]` 的存量 workflow.json 照常解析不报错（未知字段忽略）；`queries::detail` 与 parse 测试随动全绿 | `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file_test.rs` |
| AC-6 | （同上——detail 线面半边：wire 面 interrupted 键删除 + 存量文件聚合投影不回归） | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs` |
| AC-6 | （同上——orchestration 快照断言随动半边：跨 crate 消费面的 `detail.interrupted` 断言删） | `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs` |
| AC-6 | （同上——Rust cfg(test) 构造点随动半边：`Workflow` 字面量去 `interrupted` 字段的编译期适配，既有断言零变化） | `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/eval_checklist_test.rs` |
| AC-7 | `tests/golden/*.json` detail 线面按显式重写流程更新（v2-b 无 interrupted 字段）；`corpus_golden_test` 的 `interruptedCount` 投影删且 golden 再生；`bindings.ts` 再生成后 `ChangeDetail.interrupted` / `InterruptedEntry` 消失，tsc 全量类型检查拦截前端消费漂移 | `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` |
| AC-7 | （同上——bindings 产物断言半边：mod_test `DTO_TYPES` 清单移除 `"InterruptedEntry"`、重导幂等不破） | `packages/desktop/src-tauri/src/bindings/mod_test.rs` |
| AC-8 | `pnpm -C packages/desktop run client:check`（fmt / lint / knip）与 `pnpm -C packages/desktop run test` 全绿；`cargo test --workspace`（src-tauri 根）全绿；无新增豁免条目 | —（见不可测试项 1） |
| AC-9 | `packages/desktop/package.json` `version` 0.4.6 → 0.4.7；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 版本不随动 | —（见不可测试项 2） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test]、前端 vite-plus 纯函数/组件 mock 测试）。
     本变更以「删减」为主：interrupted 词汇清除的废弃用例与新增边界用例同表列示（迭代类型区分）；
     fixtures / golden / bindings.ts / 纯类型 / 未解析出既有测试文件的面统一落「不可测试项」声明（含路由去向）。
     跨模块组合用例挂靠链路入口模块（发起方 / 最上层调用方）的 `#### 用例` 表，无独立集成测试章节。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs -> packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file_test.rs

#### 待测功能

- parse_workflow_file(path: &Path) -> WorkflowFileParse: 签名不变；停提取 `interrupted`（键回落 serde 未知字段忽略），两段式宽松解析骨架、单条损坏条目容错、宽松时间戳与 camelCase alias 口径零改动（AC-6 / D7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parse_workflow_file 停提取 interrupted | 废弃 | 既有「含 interrupted 条目的 workflow.json 解析」用例中 `workflow.interrupted.len() == 1` / `[0].phase` / `[0].end_at` 三断言行废弃——`Workflow` 字段面收敛五字段后断言随删；fixture 的 `interrupted[]` 键**保留在场**（磁盘形状实证，proposal「不要修改」） | 废弃 |
| parse_workflow_file 停提取 interrupted | 正向 | 含 `interrupted[]` 的存量 workflow.json 照常解析（空数组与一条实数据两形态）：Ok 分支、`workflow_type` / `created` / `eval` / `file_log` / `active_phase` 五字段逐字段照常、不产生 interrupted 字段（未知字段忽略——AC-6 存量解析半边） | 新增 |
| parse_workflow_file 停提取 interrupted | 异常 | `interrupted` 键为非数组形态（如对象、字符串）：解析照常成功不报错（未知字段忽略对任意值形态成立，键不再有类型敏感的提取分支） | 新增 |
| parse_workflow_file 停提取 interrupted | 边界 | 既有宽松解析用例族全绿：单条损坏 eval / file_log 条目跳过该条、宽松时间戳、未知顶层键忽略、v1 / v0 代际降级（两段式骨架不变与容错措辞随动的回归锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workflow.json（数据文件） | 进程边界：TempWs tempdir 真盘 fixture 真实读写（既有装置沿用，零 mock）；存量形态 fixture 的 `interrupted[]` 键原样保留 | 全部行 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- change_detail(layout: &Layout, name: &str) -> Option&lt;ChangeDetail&gt;: 签名不变；`ChangeDetail` 线面减 `interrupted` 字段（线面 `InterruptedEntry` 与 `From` impl 随删）；其余线面口径（=null + ISO 串、纯 derive 零字段属性）不变（AC-6 / D7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_detail 线面 interrupted 键删除 | 废弃 | 既有用例 `detail.interrupted.len() == 1` / `detail.interrupted[0].phase` 两断言废弃——字段删除后随删（fixture `interrupted[]` 键保留在场） | 废弃 |
| change_detail 线面 interrupted 键删除 | 废弃 | 序列化 wire 断言 `value["interrupted"][0]["endAt"]` ISO 串废弃——键不再出线（AC-7 bindings 面「返回面去 interrupted 键」的 Rust 侧直接证据） | 废弃 |
| change_detail 线面 interrupted 键删除 | 正向 | 含 `interrupted[]` 存量文件 → `change_detail` 返回 `Some`：pipeline 9 站聚合 / active_phase / fileLog / artifacts 投影逐字段照常、序列化 wire 无 interrupted 键（AC-6 detail 半边 + AC-5「存量留档不出图」的上游保证——detail 不再携带则转换层无从收集） | 新增 |
| change_detail 线面 interrupted 键删除 | 异常 | 含 `interrupted[]` 且 eval 条目损坏的存量文件：宽松解析跳过损坏条目后 detail 照常聚合（停提取不改变容错语义——unparsable / 降级既有用例族全绿） | 新增 |
| change_detail 线面 interrupted 键删除 | 边界 | 既有 detail 用例族全绿：=null + ISO 串线面、同 phase 多 attempt 折叠升序、时间戳分量伪影不出现（纯 derive 零字段属性口径不变的回归锚——golden 冻结口径「字段演进之外形态仍冻结」） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workflow.json（数据文件） | 进程边界：TempWs tempdir 真盘 fixture 真实读写（既有装置沿用，零 mock） | 全部行 |

### packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未声明该文件公共 API（snapshot.rs / FsSnapshot 读路径零触达）；
     本章节仅承接 proposal 测试文件清单 snapshot_test.rs 的 `detail.interrupted` 断言随动
    （design 阶段七留痕：Rust cfg(test) 构造点适配由测试轨道承接）。 -->

- snapshot 读路径 detail 断言面（VALID_WORKFLOW fixture + 聚合字段断言）: `Workflow.interrupted` / `ChangeDetail.interrupted` 删除后的跨 crate 消费面随动（AC-6 半边）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| snapshot detail 断言随动 | 废弃 | `VALID_WORKFLOW` fixture 的 `interrupted[]` 键保留（磁盘形状实证）；「运行态：active_phase + interrupted 留档」用例中 `detail.interrupted.len() == 1` / `[0].phase == "test-gen"` 两断言随删 | 废弃 |
| snapshot detail 断言随动 | 正向 | 含 `interrupted[]` 的 fixture 上 snapshot 全链路（读 workflow.json → detail 聚合）端到端不炸：active_phase / pipeline 9 站 / file_log 条目字段面断言照常通过（跨 crate 消费面的 detail 线面零漂移证据） | 新增 |
| snapshot detail 断言随动 | 异常 | 停解析后读路径无新增失败形态：既有容错与降级断言族全绿（与 detail_test 存量用例同型，orchestration 侧横切回归锚） | 新增 |
| snapshot detail 断言随动 | 边界 | `VALID_WORKFLOW` fixture 的 `interrupted[]` 键保留在场（磁盘形状零改写——proposal「不要修改」；键由 serde 未知字段忽略承接，删除仅限断言面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workflow.json（数据文件） | 进程边界：真盘 fixture 真实读写（既有装置沿用，零 mock） | 全部行 |

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/eval_checklist.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/eval_checklist_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未声明该文件公共 API（artifacts/eval_checklist.rs 零触达）；
     本章节仅承接 design 阶段七留痕的 cfg(test) 构造点机械适配：
     `Workflow` 字面量随字段面收敛去 `interrupted` 字段（编译适配，非断言变化）。 -->

- Workflow 字面量构造点（cfg(test)）: 去 `interrupted` 字段——`Workflow` 字段面收敛 `workflow_type` / `created` / `eval` / `file_log` / `active_phase` 五字段后字面量随动（AC-6 编译适配半边）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| Workflow 字面量构造点适配 | 废弃 | `Workflow { ... interrupted: ... }` 字面量构造点删 `interrupted` 字段——既有用例断言零变化（非死断言，仅构造面收敛；cfg(test) 不在默认编译面，删除由 cargo test 编译期暴露） | 废弃 |
| Workflow 字面量构造点适配 | 正向 | 适配后既有 eval_checklist 用例族全绿（产物信封 / 清单投影断言零变化——artifacts 面零触达的横切回归锚） | 新增 |
| Workflow 字面量构造点适配 | 异常 | cfg(test) 字面量残留 `interrupted` 字段 → cargo test 编译失败（`Workflow` 无该字段——适配完整性由编译门禁保证，不静默带病通过） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workflow.json（数据文件） | 进程边界：TempWs tempdir 真盘 fixture（既有装置沿用，零 mock） | 全部行 |

### packages/desktop/src/types/generated/bindings.ts（生成物断言面） -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未声明该面公共 API（bindings.ts 为 specta 重导生成物，非手写）；
     待测面为 mod_test.rs 的覆盖清单随动（D8 ④）。清单维护口径：本变更零新命令，
     `COMMAND_NAMES` / `COMMAND_WRAPPERS` 零增删，仅 `DTO_TYPES` 移除消失的类型名。 -->

- DTO_TYPES 覆盖清单: 移除 `"InterruptedEntry"` 条目（与生成物强耦合——「产物缺出线类型」panic 门禁下删类型不随动必挂，D8 随动）；`COMMAND_NAMES` / `COMMAND_WRAPPERS` 零变化（本变更零新命令，不补录；含语义对历史滞后容忍的既有口径不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings 产物断言清单随动 | 正向 | `bindings:export` 再生后既有覆盖性用例绿：产物文本不含 `InterruptedEntry` 类型名、`getChangeDetail` 返回面无 `interrupted` 键、`dto.ts` 的 `export type *` re-export 自动跟随（`DTO_TYPES` 移除该条目后清单与产物集重新一致——AC-7） | 新增 |
| bindings 产物断言清单随动 | 边界 | 重导幂等既有守线用例族全绿：连续两次导出逐字节一致、篡改产物重导恢复、产物无 `Result` 包装（生成面零漂移——`get_change_detail` 契约除 interrupted 外零变化的横切锚） | 新增 |
| bindings 产物断言清单随动 | 异常 | 门禁语义回归：`DTO_TYPES` 与产物集强耦合断言（产物缺出线类型 panic）在移除 `InterruptedEntry` 后不再触发——清单不残留幽灵条目（删类型后不随动必挂的反向证明即本用例族恢复绿） | 新增 |
| bindings 产物断言清单随动 | 边界 | `COMMAND_NAMES` 零变化：清单逐项与变更前一致（含语义容忍历史滞后——本变更无新命令不补录，既有滞后债不在本变更范围） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| bindings.ts 产物文件 | 进程边界（文件系统，真实组合不 mock）：`export_bindings()` 真实重导 + 生成物文本真实读取（既有装置沿用） | 全部行 |

### packages/desktop/src-tauri/crates/core/workflow/tests/golden（13 份 change 语料快照对账物） -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未声明该面公共 API（fixtures / golden 为入仓只读语料与快照对账物）；
     待测面取自 design 修改文件行与 D8：投影删行 + 13 份 golden 显式重写对账
    （DESKTOP_GOLDEN_REWRITE 为运行环境重写开关，复核轮不带开关运行）。 -->

- project_change_fixture parse 摘要投影: 删 `"interruptedCount"` 行（golden 生成机械随动——非用例增删）
- 13 份 change 语料 golden（v0-a / v0-b / v1-a / v1-b / v1-c / v2-a / v2-b / v3-a / corrupt 五族）: detail 段 `interrupted` 键删除 + 含 parse 摘要段 10 份 `interruptedCount` 键删除（D8 预期 diff 清单）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 语料 golden 显式重写对账（AC-7） | 正向 | 语料完整性断言零变化绿：fixtures 目录集 == `CHANGE_FIXTURES` 清单集、golden 文件集 == 语料集 + `layout-*` + README（本变更零新增语料、零新增 golden 文件——清单三方一致口径不回归） | 新增 |
| 语料 golden 显式重写对账（AC-7） | 正向 | 显式重写后全 13 份 detail 段无 `interrupted` 键（v2-b 为实数据数组整键删、其余 12 份空数组键删）；含 parse 摘要段的 10 份无 `interruptedCount` 键（投影删除的直接投影面——AC-7「v2-b 无 interrupted 字段」） | 新增 |
| 语料 golden 显式重写对账（AC-7） | 边界 | fixtures 样本文件内容零改写：v2-b workflow.json 的 `interrupted[]` 原样保留（「存量文件照常解析」的实证语料路径——读模型停解析后投影不再携带该键）；`layout-*` 三份、`golden/README.md`、detect 段零变化 | 新增 |
| 语料 golden 显式重写对账（AC-7） | 边界 | 复核轮（不带重写开关）全 13 份投影与重写后 golden 等价全绿（重写幂等——非一次性 diff，后续变更重跑可复现） | 新增 |
| 语料 golden 显式重写对账（AC-7） | 异常 | corrupt 语料族（损坏 eval / file_log / 时间戳 / verdict / 整体坏 JSON 五份）重写后投影形态与既有容错语义一致：parse 摘要段其余键零变化（停提取 interrupted 不改变宽松解析降级——D7 骨架不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| fixtures / golden 语料文件 | 进程边界（数据文件，真实组合不 mock）：TempWs 拷贝真实语料 → 全链路投影 → 规范化 JSON 与入仓 golden 全量对比 | 全部行 |
| DESKTOP_GOLDEN_REWRITE 环境变量 | 运行环境开关（重写轮置位、复核轮不带）：置位时投影覆写入仓 golden 而非对比（既有机制沿用） | 重写轮全部行；复核轮不置位 |

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

#### 待测功能

- buildFlowGraph(detail: ChangeDetail, runNodes?: RuntimeFlowNode[]): FlowGraph: 签名不变；`collectInterrupted` 删除、`mergeByStartAt` 第二参收窄 `ActiveFlowNode[]`、`buildFlowGraph` 归并列表只余 `collectActive(detail)`（AC-5 / D6）；`PIPELINE_PHASES` 9 列布局、时间序边推导公式、attempt 缺号兜底、插入规则、run overlay 追加语义零改动
- FlowNode / FlowNodeKind（types.ts 随动消费面）: 收敛 `'eval' \| 'active'` 两分类事件节点 + runtime overlay 三词汇

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| buildFlowGraph interrupted 收集路径删除 | 废弃 | 「interrupted 非空 → 独立节点携带 startAt / endAt 与同号 eval 并存（v2-b 形态）」用例废弃——收集路径删除后节点集不可能含 interrupted 分类（`ChangeDetail` 类型面已无该字段，构造输入消失） | 废弃 |
| buildFlowGraph interrupted 收集路径删除 | 废弃 | 「interrupted 与 activePhase 共存 → 两类节点共存、各自恰一条入边」用例废弃——归并列表只余 `collectActive`（active 单节点归并语义保留） | 废弃 |
| buildFlowGraph interrupted 收集路径删除 | 废弃 | 「靠后列 interrupted → 回跳 backtrack 边」用例废弃——backtrack 边推导公式保留但源节点只可能是 eval（interrupted 臂删；eval `backtrackTo` 既有用例承载推导回归） | 废弃 |
| buildFlowGraph interrupted 收集路径删除 | 废弃 | 「interrupted / active 按 startAt 插入骨干（严格晚于 / 相等追加链尾）」用例改写为 active 单独形态——插入规则断言保留、interrupted 构造臂删 | 废弃 |
| buildFlowGraph interrupted 收集路径删除 | 正向 | active 单节点归并（同号 eval 缺席时独立成节点、恰一条入边）既有用例保留——归并链收敛后唯一事件分类的正向锚（AC-5「归并只剩 active 单节点」） | 新增 |
| buildFlowGraph interrupted 收集路径删除 | 边界 | 输出节点 kind 穷举断言：事件节点 kind ∈ {'eval', 'active'}、运行步节点 kind = 'runtime'（`FlowNodeKind` 收敛后的类型级穷举——AC-5 两分类收敛，无第三分支可走） | 新增 |
| buildFlowGraph interrupted 收集路径删除 | 边界 | 既有用例族 fixture 去 `interrupted` 字段后全绿：9 列布局 / 时间序边 / attempt 缺号兜底 0 兜底 id / run overlay 追加（图结构不变量零回归锚——proposal「不要修改」） | 新增 |
| buildFlowGraph interrupted 收集路径删除 | 异常 | attempt 缺号（null）兜底既有用例全绿：节点 id 以 0 兜底成 `eval:<phase>:0` 形态（转换层容错零改动——删减不变异既有分支） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无） | 纯函数真实组合：`buildFlowGraph` 直调，输入以 ChangeDetail fixture 直构（去 interrupted 字段后的线面形态） | 全部行 |

### packages/desktop/src/views/changes/flow/flow-event-node.tsx -> packages/desktop/src/views/changes/flow/flow-event-node.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无该文件行（FlowEventNode 签名零变化）；
     待测面取自 design 修改文件行与 D6。 -->

- FlowEventNode: `InterruptedBody` 组件与 `kind === 'interrupted'` 渲染分支删；`nodeClass` 收敛 eval / active 两分支（dashed 回退分支删）；Handle 四侧锚点、eval 三色 / stale 淡化 / active pulse 视觉零改动（AC-5 / D6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| FlowEventNode interrupted 渲染分支删除 | 废弃 | 「interrupted → dashed 灰显标记 + startAt / endAt 文案」用例废弃（组件无该分支与「中断留档」词汇） | 废弃 |
| FlowEventNode interrupted 渲染分支删除 | 废弃 | 「interrupted startAt / endAt 双 null → 两侧 — 占位」用例废弃（`interruptedNode` 装置随删） | 废弃 |
| FlowEventNode interrupted 渲染分支删除 | 废弃 | 「三种 kind 直渲染不抛错」用例收敛为 eval / active 两 kind——「中断留档」文案断言随删（最小 NodeProps 形态直渲染的回归语义保留） | 废弃 |
| FlowEventNode interrupted 渲染分支删除 | 正向 | eval / active 两 kind 渲染零回归：eval 三色 / verdict 徽标 / attempt 时间文案 / stale 淡化 / active「运行中」pulse / Handle 四侧锚点既有用例族全绿 | 新增 |
| FlowEventNode interrupted 渲染分支删除 | 异常 | 渲染输出恒不含 dashed 类名与「中断留档」词汇（eval / active 两 kind 穷举——「无编译残留」的运行时面：dashed 回退分支删除后无路径可产出） | 新增 |
| FlowEventNode interrupted 渲染分支删除 | 边界 | eval 无 record 字段退化形态（缺号 / 空 checklist）既有断言全绿（nodeClass 收敛不改 eval 分支内部——渲染面收敛仅删除第三分支） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| NodeProps 最小注入 | 入参例外（组件显式 data 载荷注入，既有装置沿用）：`data: { node }` 直构 fixture，其余 NodeProps 字段不注入 | 全部行 |
| ReactFlowProvider | 真实组合（运行环境——Handle 依赖 ReactFlow store context，沿既有先例外包 Provider，不 mock） | 全部行 |

### packages/desktop/src/views/changes/flow/attachments.ts -> packages/desktop/src/views/changes/flow/attachments.test.ts

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无该文件行（mountMaterials 签名零变化、NODE_PRECEDENCE 模块私有常量）；
     待测面取自 design 修改文件行与 D6。 -->

- mountMaterials（NODE_PRECEDENCE 收敛 `['eval', 'active']`）: checklist 定位降级链 eval → active 两段；文档挂列 / checklist 挂节点 / file_log 挂节点 / outsideFiles 兜底零改动（AC-5 / D6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| NODE_PRECEDENCE 两段收敛 | 废弃 | 「同号仅 active / 仅 interrupted 降级链 eval → active → interrupted」用例废弃——三段链收敛两段（interrupted 段无构造输入；「仅 active」降级臂保留） | 废弃 |
| NODE_PRECEDENCE 两段收敛 | 正向 | 仅 active 在场（eval 缺席）→ checklist 挂 `active:<phase>:<attempt>` 节点（降级链末段语义保留——D6 收敛后的唯一降级臂） | 新增 |
| NODE_PRECEDENCE 两段收敛 | 边界 | eval 与 active 并存 → checklist 恒挂 eval（优先级首位断言保留；「三类并存 eval 优先」用例去 interrupted 臂改写为两分类形态） | 新增 |
| NODE_PRECEDENCE 两段收敛 | 边界 | 既有用例族全绿：文档挂列（col 键）/ file_log 挂节点 / outsideFiles 兜底（挂载规则零改动回归锚——proposal「不要修改」） | 新增 |
| NODE_PRECEDENCE 两段收敛 | 异常 | 同号 eval / active 均缺席（仅他号条目）→ checklist 不误挂他号节点（定位 miss 兜底既有语义全绿——两段链不引入新误挂路径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无） | 纯函数真实组合：`buildFlowGraph` + `mountMaterials` 真实串联（挂载面以真实图输出为输入——内部模块不 mock） | 全部行 |

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

- RunStepNode(props: NodeProps&lt;RunStepFlowNode&gt;): data 面收敛 `{ node }`（`RunStepNodeData` 去 `onOpenSession` 键）；「查看会话」按钮块删除；分组徽章 / 步词汇 / 状态视觉 / Handle 零改动（AC-4）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunStepNodeData 收敛与按钮退役 | 废弃 | 「workerAgent + onOpenSession 注入 → view-session 按钮渲染 + stopPropagation 上抛」用例废弃（按钮与 data 键删除——AC-4） | 废弃 |
| RunStepNodeData 收敛与按钮退役 | 废弃 | 「onOpenSession 缺省 → 不渲染按钮」用例废弃（data 面无该键，用例前提消失；`renderNode` 装置去 onOpenSession 参） | 废弃 |
| RunStepNodeData 收敛与按钮退役 | 正向 | workerAgent / toolStep / gate 三组九步词汇渲染既有用例族全绿：分组徽章 / 步词汇 / run-step-detail 详情文案 / Handle 零回归（AC-4「ToolStep / Gate 呈现不变」） | 新增 |
| RunStepNodeData 收敛与按钮退役 | 边界 | data 载荷收敛 `{ node }` 后节点渲染不炸（类型级收敛的运行时锚：data 无多余键、渲染输出不含 button 元素——workerAgent 在场亦然，按钮块删除的组件级穷举） | 新增 |
| RunStepNodeData 收敛与按钮退役 | 异常 | status / detail 形态边界既有断言全绿（运行中 pulse、detail 文案直呈、null detail 缺省——删减不变异既有渲染分支） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| NodeProps 最小注入 | 入参例外（组件显式 data 载荷注入，既有装置沿用——装置随 `RunStepNodeData` 收敛同步去掉 onOpenSession 形参） | 全部行 |
| ReactFlowProvider | 真实组合（运行环境——Handle 依赖 ReactFlow store context，沿既有先例外包 Provider，不 mock） | 全部行 |

### packages/desktop/src/views/changes/flow/change-flow-graph.tsx -> packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无该文件行（toChartNodes 为模块内装配函数）；
     待测面取自 design 修改文件行与 AC-4。 -->

- toChartNodes: runtime 分支注入回 `data: { node }`（`onOpenSession` 注入与注释删）；`onNodeClick` 节点点击上抛 DrawerSelection 零改动（AC-4）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| toChartNodes onOpenSession 注入删除 | 废弃 | 「workerAgent 节点 view-session 按钮 + 点击 onSelect 同参」用例废弃（图层注入删除——AC-4） | 废弃 |
| toChartNodes onOpenSession 注入删除 | 废弃 | 「toolStep / gate 节点 data 无 onOpenSession 键」用例废弃（全部 runtime 节点 data 均无该键——用例前提收敛为恒真，随 detail fixture 去 interrupted 字段一并适配） | 废弃 |
| toChartNodes onOpenSession 注入删除 | 正向 | 节点点击（事件节点 / 运行步节点）→ `onSelect` 上抛 DrawerSelection 既有用例全绿（点击节点本体打开抽屉入口不变——AC-4「点击节点本体即打开抽屉」） | 新增 |
| toChartNodes onOpenSession 注入删除 | 边界 | 注入形态收敛后图结构零回归：9 列 + 事件节点 + 运行步节点数量断言、边集与重建图逐 id 一致（「注入不增删节点 / 不产边」断言保留——data 形态收敛不触图结构） | 新增 |
| toChartNodes onOpenSession 注入删除 | 异常 | 渲染输出恒不含 view-session testid（workerAgent 运行节点在场亦然——「图层仍注入 / 组件仍渲染」半边漂移的残留异常面检测） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ResizeObserver / DOMMatrixReadOnly / getBBox | 运行环境垫片（jsdom 缺口 stub 而非业务 mock，既有装置沿用）：节点度量真实发生 | 全部行 |
| onSelect spy | 入参例外（注入回调）：vi.fn 断言 DrawerSelection 载荷 | 「节点点击上抛」行 |
| buildFlowGraph / mountMaterials | 真实组合（不 mock）：图数据一律真实产出 | 全部行 |

### packages/desktop/src/views/changes/hooks/use-session-transcript.ts -> packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts

#### 待测功能

- useSessionTranscript(params: { root: string \| null; sourceRef: string \| null; sessionId: string \| null; liveEvents: AgentEvent[] }): UseSessionTranscriptResult——返回面增 `summary: SessionSummary \| null`（三件套 row + stats + turns，AC-3 / D4）；`loadBySessionId` 返回 sessionDetail 三件套、`loadBySourceRef` 返回反查 latest 的三件套、双 null 清空态置 null；直查优先 / 反查兜底 / seq 归并 / running 推导 / liveEvents 并入零改动（proposal「不要修改」）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useSessionTranscript summary 面 | 正向 | sessionId 直查命中 → `summary` = session_detail 应答三件套逐字段（row.id / provenance / stats / turns——AC-3 直查半边；与 messages 同一次直查取数，不重复发 invoke） | 新增 |
| useSessionTranscript summary 面 | 正向 | sourceRef 反查命中 → `summary` = 清单 latest 会话三件套（与 messages 同源同会话；「同 ref 多会话取末位」既有口径对 summary 同样成立） | 新增 |
| useSessionTranscript summary 面 | 正向 | sourceRef / sessionId 变化重查 → `summary` 随新会话更新（与 messages / running 同一 effect 重查链——active 节点切 tab 重挂即重查的 hook 底座） | 新增 |
| useSessionTranscript summary 面 | 异常 | session_detail / 反查 reject → `error` 呈现、`summary` 保持 null（错误态不虚构元信息——面板错误态下 meta 区同臂不渲染） | 新增 |
| useSessionTranscript summary 面 | 边界 | sessionDetail 应答 null（blank root 透传）→ `summary` null、不发起 transcript 重放（既有空态用例补 summary 断言） | 新增 |
| useSessionTranscript summary 面 | 边界 | sourceRef null / root null / 双 null 三空态 → `summary` null 零 invoke（`UseSessionTranscriptResult` 双 null 清空态置 null——D4） | 新增 |
| useSessionTranscript summary 面 | 边界 | 反查空清单（节点尚无会话）→ `summary` null、messages 空、不触发转录重放（既有合法空态语义，summary 与 messages 同空——未开跑角色空态的 hook 半边） | 新增 |
| useSessionTranscript summary 面 | 边界 | 既有用例族全绿：直查 Err 不回退反查、seq 去重乱序归并、竞态守卫（disposed 门禁）、running 推导、liveEvents 空数组不重装配（返回面扩展为纯增量——messages / running / error 面不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke） | 进程边界（IPC）：invokeMock 按命令名分发（session_detail / agent_sessions / agent_session_transcript，可切 resolve / reject / 挂起并记录入参——既有 mockIpc 装置沿用） | 全部行 |
| agent-adapter | 真实组合（不 mock，内部模块）：eventsToUIMessages 真实参与折叠装配 | 全部行 |

### packages/desktop/src/views/changes/flow/session-transcript-panel.tsx -> packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx

#### 待测功能

- SessionTranscriptPanel(props: SessionTranscriptPanelProps): props 签名不变；新增 `SessionMeta` 元信息区（`data-testid="session-meta"`：session id 等宽截断 + `title` 全量、运行状态徽章 running →「运行中」/ 否则「已收口」、`stats.turnCount`、`stats.inputTokens` / `stats.outputTokens` null →「—」）；`summary` null 时元信息区整体不渲染；空 roleRefs 返回 `drawer-session-empty` 空态占位（原 null）；根节改 `flex h-full min-h-0 flex-col`、时间线容器 `max-h-[480px]` → `min-h-0 flex-1`（AC-1 / AC-3 / D2 / D4 / D5）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| SessionTranscriptPanel 元信息区与恒渲染 | 正向 | summary 在场（轮行含 running）→ `session-meta` 渲染：session id、「运行中」徽章、轮数 `stats.turnCount`、token 合计（AC-3 元信息半边——字段清单按 D4） | 新增 |
| SessionTranscriptPanel 元信息区与恒渲染 | 正向 | summary 在场（全部终态轮行）→ 徽章「已收口」（与 timeline-running 消失同源推导——状态单一事实源为轮行清单） | 新增 |
| SessionTranscriptPanel 元信息区与恒渲染 | 边界 | `stats.inputTokens` / `stats.outputTokens` null → 「—」占位（不渲染字符串 "null"——既有占位纪律）；id 超长 → `title` 属性携全量 id（截断为 CSS truncate 面） | 新增 |
| SessionTranscriptPanel 元信息区与恒渲染 | 边界 | summary null（反查无会话 / 未开跑角色 / 直查 null 透传）→ `session-meta` 整体不渲染、transcript-empty 空态承担（D4——元信息不虚构会话） | 新增 |
| SessionTranscriptPanel 元信息区与恒渲染 | 边界 | 高度语义改填充宿主列：时间线容器类断言含 `min-h-0 flex-1 overflow-y-auto`、不含 `max-h-[480px]`；根节含 `h-full`（AC-1「转录拉满左列」半边——jsdom 无布局引擎，以类契约断言） | 新增 |
| SessionTranscriptPanel 元信息区与恒渲染 | 异常 | 直查 / 反查 reject → transcript-error 既有锚点全绿、`session-meta` 同样不渲染（错误态无元信息可呈） | 新增 |
| SessionTranscriptPanel 元信息区与恒渲染 | 废弃 | 「roleRefs 空：面板不渲染、零 invoke」用例废弃——改写为「面板渲染 `drawer-session-empty` 空态占位 + 零 invoke」（恒渲染结构，AC-1 空态半边 / D2） | 废弃 |
| SessionTranscriptPanel 元信息区与恒渲染 | 边界 | 三 role tab / 反查直查联动 / tab key 回退链 / refs 变化复位 / 空态错误态既有用例族全绿（tab 结构与 `ROLE_LABEL` 词汇不变——proposal「不要修改」；`AgentTimeline` 复用无第二套时间线） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke） | 进程边界（IPC）：invokeMock 按 sourceRef / sessionId 双寻址注册表（既有 mockIpc 装置沿用） | 全部行 |
| useSessionTranscript / AgentTimeline | 真实组合（不 mock，内部模块——最小 mock 原则）：fixture 经 mock IPC 流入真实 hook 与时间线，断言渲染输出与 invoke 入参而非 hook 内部 | 全部行 |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

#### 待测功能

- DetailDrawer(props: DetailDrawerProps): aside 双列布局——`w-[960px]` + `max-w-[85vw]`、去整列单滚、内容行 `flex min-h-0 flex-1`、左列 `w-[60%] min-w-0` 会话区 / 右列 `min-w-0 flex-1 overflow-y-auto` 三分节（AC-1 / D1）；`selectionRoleRefs` 补 `kind === 'active'` 分支（executor / evaluator / decision 三 ref，sessionId 恒 null、sourceRef 定式 `` `${change}/${node.phase}/${role}/${node.attempt}` ``——AC-2 / D3）；删 `roleRefs.length > 0` 条件渲染（左列恒渲染——D2）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| DetailDrawer 双列壳（D1） | 正向 | 打开抽屉（eval 节点选中）→ 左列会话区与右列三分节（drawer-docs / drawer-eval / drawer-files 三锚点）同时渲染；抽屉壳类断言含 `w-[960px]` 与 `max-w-[85vw]`；右列容器 overflow-y-auto、整列单滚移除（AC-1 布局半边——jsdom 无布局引擎，以类契约断言列结构） | 新增 |
| DetailDrawer 双列壳（D1） | 正向 | 右列三分节内容组装既有用例族全绿：ArtifactView / eval report + checklist 信封 / FileLogTable / 空态文案 / v1 代际降级（右列节内结构与 data-testid 锚点零改动锚——proposal「不要修改」） | 新增 |
| DetailDrawer 双列壳（D1） | 边界 | 列头 / ToolStep / role=null 运行步选中 → 左列 `drawer-session-empty` 空态占位、右列三分节保持、双列结构不跳变（既有「非会话选中」用例改写：active 移出无联动集合、interrupted 成员删除——AC-1） | 新增 |
| DetailDrawer 双列壳（D1） | 边界 | selection = null → 返回 null 不渲染（既有语义不回归）；遮罩关闭 / 关闭按钮既有用例全绿 | 新增 |
| DetailDrawer 双列壳（D1） | 异常 | 悬空 selection（节点 id 未命中 graph）→ 标题中性占位「事件节点」不崩、双列结构保持（既有用例改写：空态占位承接左列、三分节空态文案保留） | 新增 |
| DetailDrawer 双列壳（D1） | 废弃 | 「active / interrupted 节点选中 → eval 节空态」用例改写——interrupted 臂删（active 选中 eval 节空态断言保留并入 active 联动用例族）；detail() 装置 ChangeDetail 字面量 `interrupted: []` 字段删（detail 线面已无该字段——类型级构造点适配） | 废弃 |
| DetailDrawer 双列壳（D1） | 废弃 | 「非会话选中（active / interrupted / role=null 运行步 / 列头）→ 无转录联动区（面板不渲染）」用例废弃——active 已有转录联动、interrupted 不存在、面板恒渲染（面板锚点 toBeNull 断言改 `drawer-session-empty` 断言） | 废弃 |
| DetailDrawer 双列壳（D1） | 废弃 | 「eval 节点 attempt 为 null → ref 组为空、无转录区（面板不渲染）」用例改写——ref 组为空 → 面板呈 `drawer-session-empty` 空态占位（零 invoke 断言保留、attempt — 标题占位断言保留——D2 恒渲染的既有退化语义承接） | 废弃 |
| selectionRoleRefs active 三会话反查（D3） | 正向 | active 节点选中 → 三转录 tab（执行 / 评估 / 决策）；executor 反查 `agent_sessions` 携 `{ root, source: 'change', sourceRef: 'test-change/<phase>/executor/<attempt>' }`（sessionId 恒 null——session_detail 恒零调用，直查不发起——AC-2） | 新增 |
| selectionRoleRefs active 三会话反查（D3） | 正向 | 已开跑角色（反查命中，会话轮行含 running 的进行中形态）→ 已流出转录重放呈现在左列（建档即落库 + sealed append 的查询时快照面——AC-2「重放已落库转录」） | 新增 |
| selectionRoleRefs active 三会话反查（D3） | 正向 | 切 decision tab → decision 走反查（与 eval 节点 decision 槽位缺席双 null 空态语义不同——active 无槽位，反查是该 phase / attempt 下 decision 会话唯一寻址，D3） | 新增 |
| selectionRoleRefs active 三会话反查（D3） | 异常 | 未开跑角色（反查空清单）→ 切该 tab 呈 transcript-empty「（暂无该会话转录）」空态、零进一步查询（AC-2 未开跑半边） | 新增 |
| selectionRoleRefs active 三会话反查（D3） | 边界 | liveEvents 非空传入 → 面板转录不含任何实时事件（active 无 sessionId → liveEvents 按 sessionId 过滤恒空——「全程不发起实时事件流订阅」的结构保证，AC-2） | 新增 |
| selectionRoleRefs active 三会话反查（D3） | 边界 | sourceRef 定式与 active 节点 id 同源：`ActivePhase.attempt` 非空保证四段定式可组装（`active:<phase>:<attempt>` 与 `<change>/<phase>/<role>/<attempt>` 同参——组装定式断言） | 新增 |
| selectionRoleRefs active 三会话反查（D3） | 边界 | runtime WorkerAgent 直查 / 反查兜底 / liveEvents 过滤、eval 三 tab 直查 / 双 null 空态 / 悬空 selection 中性占位既有用例族全绿（既有会话联动语义零回归——proposal「不要修改」） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke） | 进程边界（IPC）：invokeMock 按 sourceRef / sessionId 双寻址 fixture 注册表（detailFixture / sessionsFixture / transcriptFixture，既有装置沿用） | 「双列壳」与「active 反查」全部行 |
| buildFlowGraph / mountMaterials / ArtifactView / FileLogTable | 真实组合（不 mock，内部模块）：图与素材面真实组装 | 全部行 |
| useSessionTranscript | 真实组合（不 mock——内部协作 hook 经 mock invoke 流入，抽屉断言渲染输出与 invoke 入参而非 hook 内部） | 转录区各行 |

---

## 不可测试项

- AC-8 全管线回归（`client:check` 的 fmt / lint / knip 零新增豁免 + 前端与 Rust 两套自动化套件全绿） — **原因**: 执行聚合面——由本文件各章节用例全集在两套套件运行时综合承载（test-execution 阶段承接）；「无编译残留 / tsc 全量类型检查拦截前端消费漂移」为静态检查面，非独立被测单元。
- AC-9 版本交付（归档轨道） — **原因**: 归档提交将 `packages/desktop/package.json` version 0.4.6 → 0.4.7 为人工归档动作（`tauri.conf.json` 经 `../package.json` 自动跟随、`src-tauri/Cargo.toml` 不随动），非可执行代码单元，无自动化断言面（session-visibility AC-11 同型先例）。
- `packages/desktop/src/types/generated/bindings.ts` 生成物 — **原因**: specta 重导生成物非手写可测单元，不建共置 `bindings.test.ts`（防空套件红灯，test_resolve_paths 解析路径 `bindings.test.ts` 不采纳）；出线断言由 `src/bindings/mod_test.rs` 承载（见「bindings.ts（生成物断言面）-> mod_test.rs」章节），字段级线面由 detail_test.rs 序列化断言与 golden 快照守卫。
- `packages/desktop/src/views/changes/flow/types.ts` 纯类型（`FlowNodeKind` 收敛 / `InterruptedFlowNode` 删除） — **原因**: 纯类型模块无独立运行时行为，不建 `types.test.ts`（test_resolve_paths 解析路径 `types.test.ts` 不采纳，文件不存在且本变更不建）；两分类语义由 graph / flow-event-node / attachments / detail-drawer 用例经真实组合覆盖，编译面由 tsc 全量拦截。
- `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` / `model/mod.rs`（`InterruptedEntry` + `Workflow.interrupted` 删除 / re-export 随动） — **原因**: test_resolve_paths 解析出 `model/workflow_test.rs` / `model/mod_test.rs`，二者不存在且本变更不建（proposal 测试文件清单未列）；停解析的行为证明由 workflow_file_test.rs 存量照常解析用例 + detail_test.rs 线面断言 + golden 语料对账三方承载，模型出口单点无独立运行时语义。
- 前端共置 fixture 字段适配（`app.test.tsx` / `change-detail-view.test.tsx` / `use-change-detail.test.ts` / `layout.test.ts` 的 `ChangeDetail` 字面量 `interrupted: []` 键删） — **原因**: 类型级机械适配非行为断言（design 阶段七指派守线阶段留痕处理），四文件的被测源模块零触达、无新增用例面，不设章节不设用例；适配正确性由其既有用例族全绿承载。
- golden 13 份「diff 人工逐份确认留痕」（AC-7 人工确认半边） — **原因**: 人工评审行为不可自动化；自动化半边（显式重写后复核全绿 + 预期 diff 清单范围断言）由 corpus_golden_test.rs 章节承载。
- 双列布局视觉观感（列宽 60/40 比例的渲染像素、85vw 截断时两列等比收窄、极窄窗口列内滚动可读性） — **原因**: jsdom 无布局引擎，渲染像素观感不在自动化面；类契约断言（`w-[960px]` / `max-w-[85vw]` / `w-[60%]` / `min-w-0` / overflow-y-auto）覆盖结构契约，视觉回归依赖人工或后续视觉快照轨道。


