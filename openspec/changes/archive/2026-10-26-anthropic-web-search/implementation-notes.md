# 实现备注 · anthropic-web-search

> **定位**：实现相位的结论与留痕记录（不含实现代码，不重复 design 的论证）。供归档、**归档后人工验收**与后续变更引用。
>
> **口径依据**：`design.md`「验收面口径」/ `D17` / 待决问题 5；`proposal.md`「回溯修订（第二轮）」/ 风险 R5；`specs/desktop-agent-execution/spec.md` 边界条 16。
>
> **与 `tasks.md` 的关系**：本文件是 `tasks.md` 阶段 12「留痕确认（非实现任务）」的落点。`tasks.md` 不持有任何真机核验门项或阻塞项（不变量 4）；真机项在此留痕，**不构成实现期门**。

---

## 1. 任务完成状态

- `tasks.md` 阶段 1 – 阶段 14 全部标记 `[x]`，无遗留 `[ ]` 项。
- 阶段编号沿用原稿（阶段 1 起；原「阶段 0 · 前置核验（真机门）」已按 proposal 回溯修订第二轮整段删除、不重排）。
- 阶段 13 为**回溯修订**（`design.md` D1 第四稿）：删 `AgentBlock` 的 5 处 `skip_serializing_if`、保留 `#[serde(default)]`，线格式改为「键恒在（`null` / `[]`）」，以换 bindings 统一形态；同步 `proposal.md` / `specs/desktop-agent-execution/spec.md` 文案。**前端零 diff**、D2–D19 零改、`src/bindings/mod_test.rs` 零改。
- 阶段 14 为**回溯修订**（`project_choice` 倒序配对）：把「已消费」标记从「遍历到 use 时」前移到预索引期（详见 §3.8）；单一函数改动，类型面 / 契约面 / 前端零 diff。
- 测试编写 / 单元测试 / 集成测试由测试相位承接（见 `test-design.md`）；`static-check` / `test-execution` 等机械脚本由桌面端执行，不属本相位。

## 2. 真机核验项：`pause_turn` 的相邻 assistant 消息

| 项 | 结论 / 记载 |
|----|------|
| 核验动作 | **实现期未执行**。成因：本期无官方 Anthropic 端点凭据，该核验无法在实现期履行；把无法履行的核验写成实现期门只会堵塞实现（`proposal.md` 风险 R5） |
| 归类 | **已知限制**（`specs/desktop-agent-execution/spec.md` 边界条 16），非缺陷、非阻塞项、非待决阻塞项 |
| 留痕位置 | 边界条 16 + `design.md` `D17` / 待决问题 5 + 本文件 §2 |
| 实现形态事实（供核验者对照） | `sdk/resume.rs::rebuild` 对两条 assistant 事件照常各产一条 `Message::Assistant`，rig 编码侧**不做**同角色合并；`sdk/loop.rs` 的 `pause_turn` 续跑在**同一轮内**追加 assistant 消息，不改变该形态 |
| 确认动作（归档后人工验收） | 以官方端点 + 凭据跑一次真实触发 `pause_turn` 的 `web_search` 会话，并以 `Continue` 续轮，确认 Anthropic 服务端**接受相邻同角色 assistant 消息**；结论写入验收记录（本文件 §2「核验结论」栏与验收记录同步） |
| 核验结论 | **待归档后人工验收回填**（本文件不预置结论，不代测试 / 验收相位下判断） |
| 降级预案 | 若人工验收确认被拒：回放时按 `pause_turn` 边界**合并相邻 assistant 消息**（合并对象 = 该轮重建产出的连续 assistant `AssistantContent` 序列，合为一条消息的序列）。**本期不预建、无实现分支**，由后续变更据边界条 16 偿还（`D17`） |

## 3. 实现期已完成的自动化面自查（≠ 真机核验）

以下为源码级 / 构建级自查证据，仅覆盖 AC-1..AC-13 的自动化面，**不能**替代 §2 的真机确认：

1. **单字段模式残留扫描（阶段 4 末尾 + 阶段 5 编译门互为复核）**：生产源无 `AgentBlock::Text { text }` 单字段形态残留——`crates/core/orchestration/src/transcript.rs`（`Text { text, .. }`）、`sdk/loop.rs`（用户提示词字面量已补 `citations: None`；内联块转换已随 `web_search::project_choice` 删除）、`sdk/resume.rs`（`Text { text, .. }` 两处）均已补位。
2. **编译门（阶段 5 末尾唯一单点 + 阶段 11 末尾复核）**：`cargo check --manifest-path packages/desktop/src-tauri/Cargo.toml`（默认目标，不含 `--all-targets`）；复核点覆盖 `tauri-plugin-opener` 落地与 `main.rs` 插件注册——该阶段为最后一处 Rust 改动点。
3. **开关链路闭合**：store v4（`AgentProviderRecord.web_search`，`#[serde(default)]`）→ `save_agent_provider` 第九位置参 → `compose`（`provider.web_search && matches!(protocol, Protocol::Anthropic)`，移动前算出）→ `EngineFacade::with_web_search` → `SdkRunner::with_web_search` → `PumpPayload.web_search` → `LoopTurn.web_search`；门面与 runner 侧零协议判定。
4. **bindings 再生成**：`pnpm -C packages/desktop run bindings:export` 产物已含 `AgentBlock` 五变体 + `citations`、`AgentCitation` / `WebSearchSource`、`AgentProviderRecord.webSearch?: boolean`、`saveAgentProvider` 第九位置参 `webSearch: boolean`。阶段 13 之后的产物形态：**统一形态**（零相位伴生类型；`AgentEvent` 为对象 + `} & AgentEventKind;`；`agentStart` 首参 `Channel<AgentRunMessage>`），成员可选位不变（`citations?: AgentCitation[] | null` / `results?: WebSearchSource[]` / `error?: string | null` / `rawUse?`·`rawResult?: unknown | null`）；连续两次导出逐字节一致、`git diff --exit-code -- packages/desktop/src/types/generated` 为零（等价 `bindings:check`）。
   - 附带约束（实现事实，防回归）：`event.rs` 的 doc 文案**不得**出现 `_Serialize` / `_Deserialize` 字面量——doc 会进入生成物，命中 `src/bindings/mod_test.rs:464` 的 `!content.contains("_Deserialize")` 断言。
5. **前端依赖与壳层权限面**：`@tauri-apps/plugin-opener@^2.0.0` 与 `pnpm-lock.yaml` 同步（`node_modules/@tauri-apps/plugin-opener` 可解析，`openUrl` 为唯一 import 面）；Rust 侧 `tauri-plugin-opener = "2"` 于 `main.rs` 注册；`capabilities/default.json` 仅 `opener:allow-open-url` + `http://*` / `https://*` 两条 allow，无 `opener:default` / `mailto:` / `tel:` / `file:`。
6. **版本抬升**：`packages/desktop/package.json` `0.5.28` → `0.5.29`（依赖已在阶段 7 落地，此处只抬版本）。
7. **回溯修正（客户端测试首轮的源码侧根因，两处；均在既有实现文件内对齐已定稿决策，不改类型面与契约面）**：
   - `src/lib/agent-adapter.ts::citationMetadata` 的 citations 载荷形状改为 `{ agent: { citations } }`（对齐 spec「数据来源 SHALL 为 `providerMetadata.agent.citations`」与 `D12`）——原单层 `{ agent: citations }` 会让 `agent-messages.tsx::citationsOfPart` 恒读到 `undefined`，引用列表恒空；`plansToParts` 与 `textChunks`（`text-start` / `text-delta` 同一份对象）经同一 helper 自动同形。
   - `src/components/agent/agent-blocks.tsx::WebSearchCard` 的 `output.search` 读取改为「加位缺席与显式 `null` 同义」（`output?.search ?? null`），对齐 `test-design.md`「载荷形态异常不炸卡」条目与决策 ③「卡面无状态降级」——原实现只判 `undefined`，`search: null` 时读 `.results` 抛错。
8. **回溯修正（`sdk/web_search.rs::project_choice` 倒序配对，源码侧根因一处）**：「已消费」标记改为**预索引期落定**——先收集 `web_search` 的 `server_tool_use` id 集合、再按 `tool_use_id` 索引**首个** result 下标，随后仅为「有配对 use 的首个 result 下标」预置 `consumed`。修正前：choice 倒序（result 先于 use）时 result 先以孤儿成块、use 侧随后又配出第二块，同一 `WebSearch.id` 重复（违背 D2「预索引期即记已消费」与 `test-design.md`「`blocks` 恰一块、result 不另产块」）。修正后：无配对 use 的 result 不预置（孤儿规则原样）、同 id 的第二条 result 亦不预置（照常成块）；`project_choice` 签名、`loop` 调用点、`resume` / `context` 的 raw 处理与前端两态判别零 diff。

## 4. 其余已知限制留痕对照（同属知情边界，均无实现期门）

1. **边界条 15 · 升级后不可回退**：旧客户端读到含 `WebSearch` 块的事件会在整条事件解码时失败（非少显示一张卡）；仓库不提供降级分支。
2. **边界条 12 · 搜索过程不可见**：无 `AgentDelta::Search`，`normalize.rs` / `agent-transport.ts` 零改动；续跑期唯一过程观测点为 `SystemNotice{pause_turn_continued}`。
3. **边界条 13 · 转录体积不设上限**：`raw_result` 含加密索引（一次搜索数 KB），压缩会破坏 raw 逐字节保真，故明确不做。
4. **边界条 14 · `web_search` 服务端工具不可配置**：`max_uses = 5` 锁为常量，配置化需 store v5 迁移；openai 协议恒不生效（门控在解析期）。

## 5. 归档后人工验收待办（非本变更门项，不阻塞归档）

1. 执行 §2「确认动作」，把「相邻 assistant 消息是否被服务端接受」的结论回填本文件 §2 与验收记录。
2. 若结论为「被拒」，按 §2 降级预案开后续变更（合并相邻 assistant 消息），本期不做任何代码改动。
3. 其余手工 E2E（管理页勾选项生效、搜索卡与引用呈现、外链打开行为）按 `explore.md`「验证」节的手工清单执行。
