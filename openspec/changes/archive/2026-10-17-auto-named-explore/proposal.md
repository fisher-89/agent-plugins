# 提案: auto-named-explore

> **变更**: auto-named-explore
> **日期**: 2026-10-17
> **状态**: draft

---

## 问题

桌面端新建探索强制用户先起 kebab-case 主题名，再进详情手敲首轮提问，命名被前置为使用门槛：

1. **命名时机错位**：用户在需求尚未展开时就要给出文件名；起错名后无改名通道——`rename_explore_record`（store + 命令 + bindings 全链在案）UI 零接线，处于「已建成未接线」的休眠态。
2. **首轮手敲**：建档同事务预建空会话（`session_id` 回写）已让「建档即可会话」只差一次自动发送，但现状用户仍要在 composer 重敲一遍需求（`buildExplorePrompt` 拼好 stance 后等待手动 send）。
3. **explore 会话写面仅靠文本纪律**：与调试 / change 会话共享七工具面（`sdk/tools.rs` `TOOL_NAMES` 全量组装），探索立场「不写应用代码」只靠 stance 前导软约束；write / bash 全量在场。且 **definitions 收窄 ≠ 硬禁用**：loop 分发链 `policy → sandbox → execute` 无「调用名 ∈ 本轮下发 definitions」成员校验——即便从 definitions 移除 write，幻觉 write 调用仍会直通执行。
4. **笔记路径知识双源**：stance 前导插值 `openspec/explores/<topic>.md` 文件名（`stancePreamble(topic)` 按 `record.name` 钉死），与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源同步；改名机制缺席使该插值成为唯一路径真源。

## 提案

探索笔记（`openspec/explores/auto-named-explore.md`，决策记录一至四已收敛）裁定形状：**占位名建档 + create 工具原子定名 + desktop 感知改名**。五个落点：

1. **免命名建档（`create_explore_record` 的 `name` 改 `Option<String>`）**：`Some` 走既有显式名建档（导入绑定路径，零变化）；`None` 时后端**同事务**生成占位名与临时标题——占位名 = `draft-<epoch-millis>`（kebab 合法：首段字母、次段数字），同毫秒撞名以写事务内**单调 +1** 兜底重试（不依赖事务外时钟推进）；临时标题由同一时间戳派生为 `<yyyy-MM-dd HH:mm:ss>-分析中`（纯展示装饰，后续被 title 回填覆盖）。前端两连 invoke（create + update_title）问题随之消失。占位前缀 `draft-` 是「未定名」判别口径（promote 门禁、前端 stance 分形共用），字面钉死共享。占位名**仅存在于 store 记录名，从未落盘**。
2. **首轮自动发送（autoPrompt 闸门）**：新建入口改需求输入（textarea），建档成功后 `navigate(/explores/<占位名>, state={ autoPrompt: 需求 })`；详情页在「重放装载完成（`!loading`）&& `chain.length === 0` && `state.autoPrompt` 在场」三条件齐备时自动发起首轮（stance 前导 + 需求），配一次性消费 ref。HashRouter 下 back/forward 会复现 location state：重放后 `chain > 0` 自然拒发；首轮发送失败 `chain` 仍 0 时复现恰好构成一次免费重试（可接受）。`use-explore-session` 的 send 已内建 loading / 未就绪守卫，auto-send 只是新增一个 effect 触发点。
3. **explore 会话工具面注入（`SessionInjections` 契约位激活）**：core 契约 `SessionInjections` 增受控枚举位 `toolset: Option<SessionToolset>`（首值 `Explore`）；既有 `tools: Option<Vec<String>>` 位激活为七工具子集收窄表达。compose 装配（生产路径首次非 default）：`begin` 以 `provenance.source == "explore"` 判别 → 注入 `tools = [read, grep, glob, ls, edit, bash]`（**write 移除**）+ `toolset = Explore`；debug / change 等其余来源恒 default 七工具，零变化。loop 侧：definitions 按注入面组装；`RunStarted.tools` 报**本轮实际下发面**；执行前**成员校验**——调用名 ∉ 本轮 definitions 即拒绝（`SystemNotice{subtype:"tool_unavailable"}` + is_error ToolResult 回灌，run 不中断），堵死「收窄后幻觉调用直通执行」。
4. **`create_explore_note(name, content)` 专用工具（决策四形状：create 时定名，原子）**：无路径参数；服务端执行体**按调用时**（非轮建立快照）以会话指针（记录 `session_id` == 会话 id）圈定当前 explore 记录：
   - 记录为占位名（`draft-` 前缀）→ 前置校验（kebab / 目标名无既有记录 / 目标文件不存在）→ ① 笔记落盘 `explores_root/<name>.md`（create-only）→ ② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔，可供 Edit 续用）。**文件与记录自诞生即最终名，agent 手里不存在旧路径**；
   - 记录已定名 → 仅当 `name` == 记录名且文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有名与笔记路径（错误文案即恢复通道：导入记录常态命中，agent 转用 Edit）；
   - 失败面（非法 kebab / 目标名已存在 / 文件已存在附路径 / 记录 miss）显式 `Err` 回灌，agent 换名重试自愈；①后②失败的残留窗口以 `Err` 附恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
   执行体装配缝：**per-turn 专用工具执行上下文经组合根（compose）铸造**（workspace store 句柄 + root），sdk loop 经 `LoopTurn` 携带的上下文路由专用工具；`tools::execute` 对七通用工具保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。policy 决策表扩展：`create_explore_note` 归写面档位（Default 拒绝、AcceptEdits / BypassPermissions 放行）。
5. **前端感知改名（id 桥）+ stance 分形 + promote 门禁**：
   - **id 桥**：run 终态（`running` true→false 既有 effect）→ `list.refresh()` → 按记录 **id**（in-place 改名不变）重寻记录 → 名变则 `navigate replace(/explores/<新名>)`；`useExploreDoc` 以 name 为依赖自动重订阅新路径（既有机制零改）。刷新与导航之间的旧名 miss 以既有记录快照兜底，MUST NOT 闪现 MissingRecord。中间窗口（create 已执行、run 未终态）页面停留占位名（预览空态、标题「分析中」），run 终态后一次收敛——与「分析中」同窗，已接受。
   - **stance 前导分两形**（判别 = `draft-` 前缀，前端侧）：占位记录 → 命名指令形（生成 kebab 最终名、经 `create_explore_note` 原子定名；笔记结构约定：首行 `# <标题>`、次行 `workflow_type:`；优先用专用工具而非 bash 造文件）；已定名记录 → 笔记定位形（插值 `record.name` 与路径，指引 Edit 修改、文件缺失时以 create 补建）。占位形 MUST NOT 插值文件路径（路径知识归工具描述与返回值）。与 SKILL.md 双源同步照旧。
   - **promote 门禁**：记录名为占位前缀时 `promote_explore` 显式 `Err`（先完成会话定名）；校验先于笔记读取（占位名永不落盘，「笔记未落盘」错误不足以引导）。

**随决策链退役的设计**（不得在后续相位复活）：`name:` 标记行反写与第三路回填（决策四退位——name 走 create 参数，笔记维持两行约定）；`finalize` 改名命令（文件 + 记录联动改名）；「Edit 陈旧路径自愈环 / 错误文案带路径作主恢复通道」（原子定名后旧路径不存在）；`rename_explore_record` 维持休眠（仅服务会话外手动改名边缘，仍不接线）。

## 能力

盘点 `openspec/specs/`，本变更**全部为修改既有能力**（无新增能力目录）：

| 能力 | 分类 | 变更 |
|------|------|------|
| `desktop-agent-execution` | 修改 | ADDED「会话级工具面注入与 explore 笔记工具」（枚举扩展 / compose 装配 / loop 组装与成员校验 / `create_explore_note` 契约 / policy 归属）；MODIFIED「SDK 引擎（rig 进程内租户）」「SDK 引擎 bash 工具」「SDK 引擎系统提示词（AGENT.md 注入）」（解除 `SessionInjections` 消费面「维持现状」与 `RunStarted.tools` 恒报七工具的过时留痕） |
| `desktop-workspace-store` | 修改 | MODIFIED「ExploreRecord 模型与 explore 清单操作面」（`create_explore_record` 增 `Option<name>` 占位建档语义：占位名生成、撞名单调兜底、临时标题） |
| `desktop-explore-page` | 修改 | MODIFIED「探索清单页与新建流程」（新话题入口改需求输入，kebab 校验仅存导入路径）；MODIFIED「会话链与 stance 前导」（前导分两形 + auto-send 闸门）；ADDED「免命名定名与改名感知」（占位生命周期、id 桥、promote 门禁） |
| `desktop-explore-queries` | 修改 | MODIFIED「promote_explore 命令与笔记搬运」（占位前缀门禁先于笔记读取） |

## 变更范围

### 实现文件

- **core 契约**：`packages/desktop/src-tauri/crates/core/agent/src/runner.rs`（`SessionInjections` 增 `toolset` 受控枚举位；serde 兼容缺省）
- **引擎装配与 loop**：`crates/infra/agent/src/compose.rs`（`begin` 按 `provenance.source` 装配注入 + per-turn 专用工具执行上下文铸造）、`crates/infra/agent/src/sdk/loop.rs`（definitions 按注入组装、成员校验、`RunStarted.tools` 口径、专用工具路由）、`crates/infra/agent/src/sdk/tools.rs`（定义面组装参数化 + `create_explore_note` 定义；`execute` 保持七通用工具纯函数）、`crates/infra/agent/src/sdk/policy.rs`（`create_explore_note` 归写面档位）
- **store**：`crates/infra/store/src/store.rs`（`create_explore_record` 的 `Option<name>` 分支：占位名 + 临时标题 + 撞名单调兜底，同事务）
- **命令层**：`src-tauri/src/commands/explores/mod.rs`（`create_explore_record` 命令 `name: Option<String>` 透传；`promote_explore_with` 占位前缀门禁）；bindings 再生成（`packages/desktop/src/types/generated/bindings.ts`）
- **前端**：`packages/desktop/src/views/explores/components/explore-create-dialog.tsx`（TopicEntry 改需求 textarea，isKebabCase 校验退役；ImportEntry 传 `Some`）、`views/explores/explore-view.tsx`（onCreated 携 `state.autoPrompt` 导航）、`views/explores/hooks/use-explore-session.ts`（auto-send 闸门；stance 分形调用）、`views/explores/explore-detail-view.tsx`（id 桥改名感知与导航协调）、`lib/explore-stance.ts`（前导两形改造）
- **双源同步**：`plugins/dev-team/skills/openspec-explore/SKILL.md`（笔记落盘约定随工具化改写）——属插件源变更，按项目规则 bump `plugins/dev-team/package.json` version（2.10.44 → 2.10.45）并 `pnpm -C plugins/dev-team run build` 重建三面产物
- **版本**：`packages/desktop` 0.5.13 → 0.5.14

### 测试文件

- Rust：`core/agent/src/runner_test.rs`（`toolset` 序列化 additive）、`infra/agent/src/sdk/`（注入面组装 / 成员校验拒绝 / `create_explore_note` 执行体各失败臂）、`infra/store/src/store_test.rs`（占位建档 / 撞名兜底 / `Some` 路径回归）、`src-tauri/src/commands/explores/mod_test.rs`（create `Option` / promote 占位门禁）
- 前端：`explore-create-dialog.test.tsx`（kebab 校验用例转向需求输入）、`use-explore-session.test.ts`（auto-send 闸门三条件 / stance 分形）、`explore-detail-view.test.tsx`（id 桥收敛 / 无 MissingRecord 闪现）、`lib/explore-stance.test.ts`（两形断言）、`explore-view.test.tsx`（autoPrompt 导航）

### 不要修改

- `rename_explore_record` 命令与 store 操作（维持休眠；原子定名复用 store 操作但命令面不接线）
- workflow queries 读面（`read_explore` / `scan_explores` 纯读纪律；`desktop-explore-queries`「查询层纪律与命令面」零 delta）
- `SessionInjections.preamble` 位（stance 仍走前端首条 prompt 拼接，系统提示词唯一来源 `AGENT.md`）
- 事件信封 schema（`RunStarted.tools` 为既有字段，仅值面随会话来源分化；`AgentEvent` golden wire contract 不动）
- `desktop-change-*` 全域、L1-L3 上下文防线、bash 执行体、「引擎泵一轮一命与 headless 无问收口」（封闭清单哲学延续：七通用工具 + 受控枚举专用工具，自由字符串工具名不进执行）
- `openspec/explores/auto-named-explore.md`（explore 笔记，只读）

## 验收标准

1. 新建探索只输需求：不出现 kebab 输名入口；建档后详情页自动发出首轮（prompt = stance 前导 + 需求），无手动发送动作。
2. 占位建档：记录 `name` 匹配 `draft-<毫秒>`、`title` 为 `<时间戳>-分析中`、`session_id` 预建回写；`openspec/explores/` 无占位名文件；同毫秒重复建档经单调兜底获得互异名。
3. 原子定名：agent 调 `create_explore_note(name="api-retry", …)` 成功后，记录 `name=api-retry`（`id` / `session_id` 不变）、磁盘出现 `openspec/explores/api-retry.md`、返回路径可供同会话 Edit 续用；世上不存在占位名文件。
4. 改名收敛：run 终态后 URL replace 至新名、清单刷新、预览读新名文件、title / workflow_type 回填照常触发；全程无 MissingRecord 闪现。
5. 工具面硬约束：explore 会话 definitions 不含 `write`；构造幻觉 `write` 调用被成员校验拒绝（`tool_unavailable` + is_error ToolResult，run 不中断）；`RunStarted.tools` 报实际下发面（六工具 + `create_explore_note`）。
6. `create_explore_note` 拒绝面：非法 kebab / 目标名已存在 / 文件已存在（错误附路径）/ 已定名记录异名（错误附现有名与路径）均显式 `Err` 回灌且零半边残留（前置校验拦截）；Default 档拒绝（policy 写面归属）。
7. 非 explore 来源零变化：debug / change 会话 definitions 仍为七工具、`RunStarted.tools` 报七工具。
8. 导入路径回归：`Some(name)` 建档行为与现状逐字段一致（title = name）；已定名记录续话 stance 为笔记定位形。
9. promote 门禁：对 `draft-` 前缀记录点击「启动变更」得显式 `Err`（引导先完成定名），行内呈现。
10. 静态检查与测试（`pnpm run -C packages/desktop check` + 三测试根）由桌面端执行通过；插件产物重建后 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` 同步。

## 风险

1. **fs + db 跨存储非真原子**：`create_explore_note` ①落盘 ②改名两步间失败会留残留（文件在而记录未改）。缓解：前置校验把窗口收窄到并发 DB 错误级；失败 `Err` 附残留与恢复指引（重跑 create 命中「文件已存在」附路径 → 引导对齐记录）。设计期可评估步序翻转（先改名后落盘的自愈性）。
2. **agent 定名质量**：撞名 / 非 kebab / 语义差的名字依赖错误回灌 + agent 换名重试（软闭环，可能浪费一轮）；create-only 语义下名字一次定死，会话内无改口机制（用户裁定接受，会话外 rename 休眠能力兜底）。
3. **已定名记录的路径发现成本**：agent 不再从 prompt 获得路径，需经 create 错误面获得（导入场景常态命中）；stance 笔记定位形缓解，极端情况多一轮试错。
4. **`RunStarted.tools` 值面分化**：事件 schema 不变但值随来源分化；排查既有测试 / 消费方是否存在硬编码七工具断言（转录渲染按名渲染无假设，测试断言需随迁）。
5. **中间窗口可见性**：定名发生到 run 终态之间页面停留占位名（预览空态、「分析中」标题）；watch 仍盯旧路径——run 终态 id 桥一次收敛（用户裁定可接受）。
6. **autoPrompt 状态复现**：back/forward 重现 location state；`chain.length === 0` 闸门拒发已发出者，首轮失败则复现即重试——若首轮「部分成功」（发出但失败）可能造成意外重发一次（与免费重试同形，接受并留痕）。
7. **占位前缀撞用户自建名**：用户刻意导入 `draft-20260101…` 文件会被 promote 门禁拦截（可接受，文档留痕）；占位生成侧撞名由单调兜底吸收。
8. **stance / SKILL.md 双源漂移**：既有风险面随两形改造扩大，同步纪律照旧（双侧注释互链）。
9. **explore 未决项收敛情况**：create 执行体的上下文缝（LoopTurn 闭包形态）、枚举命名、stance 两形的具体文案、占位名时间戳格式细节属 design 定稿面；本提案已定缝隙归属（组合根铸造、core 只承载词汇）与对外契约。
