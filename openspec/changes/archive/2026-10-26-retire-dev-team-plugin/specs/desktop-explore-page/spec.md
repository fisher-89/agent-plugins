# desktop-explore-page Specification (Delta)

## MODIFIED Requirements

### Requirement: 会话链与 stance 前导

每个 explore SHALL 对应一条会话链（同一 session 的轮序列即链，按 `started_at` 有序）：打开详情页 SHALL 以记录字段 `ExploreRecord.session_id`（归属 agent 会话 id，建档同事务预建空会话并回写；存量记录经 `ensure_explore_session` 归位补齐）**直查** `session_detail(root, sessionId)` 并重放落库事件还原全部历史；会话尚无轮（建档预建的空会话）则下次发送即其首轮（Continue 开轮，不新铸会话）。按 `source="explore"` + `source_ref` 反查定位链头并沿 `parent_run_id` 回溯的路径 SHALL 退役（前端不再经来源反查会话）。MVP MUST NOT 提供「新会话」动作。发送时 SHALL 将 desktop 内置的精简 explore stance 前导模板拼接进首轮 prompt 头部（模板归属 desktop 包，注释自持单源——原双源对端的插件 `skills/openspec-explore/SKILL.md` 已随插件退役删除）。stance 前导 SHALL 按记录定名状态分两形（判别 = `name` 是否 `draft-` 前缀，前端侧）：

- **命名指令形（占位记录）**：指引 agent 生成 kebab 最终名并经 `create_explore_note(name, content)` 原子定名落盘（引擎侧契约见 desktop-agent-execution「会话级工具面注入与 explore 笔记工具」）；MUST NOT 插值笔记文件路径（路径知识归工具描述与返回值）；含「优先用专用工具而非 bash 造文件」软约束；
- **笔记定位形（已定名记录）**：插值当前 `record.name` 与笔记路径，指引用 `edit` 修改笔记、文件缺失时以 `create_explore_note` 同名补建。

两形 SHALL 共同明示「笔记首行写 `# <标题>`」与「推算变更类型并写约定标记行」两条约定（标记行与首行标题同构，具体格式由 design 定稿）。**auto-send**：建档导航携带的 `state.autoPrompt` SHALL 在「重放装载完成（`!loading`）&& `chain.length === 0` && `state.autoPrompt` 在场」三条件齐备时自动发起首轮发送（stance 前导 + 需求，配一次性消费 ref）；HashRouter 下 back/forward 复现 state 时 `chain.length === 0` 闸门拒发（发出即有轮行），首轮发送失败 `chain` 仍 0 时复现构成一次免费重试（留痕接受）。续话 SHALL 以记录回写的 `session_id` 作为会话引用（`Continue`）发起。双恢复 SHALL 为：文档从磁盘读 + 对话重放该会话落库事件，两者互不依赖对方存在。

#### Scenario: stance 模板单源自持

- **WHEN** 审查 `src/lib/explore-stance.ts` 的注释头与 stance 前导文本来源
- **THEN** 注释只声明 desktop 包自持单源，MUST NOT 声明与插件 `skills/openspec-explore/SKILL.md` 双源互链或要求「插件源变更走版本 bump + 重建」
- **AND** stance 两形文本与既有注入行为零变化（只改溯源表述）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/lib/explore-stance.ts` | stance 前导模板 | 两形（占位命名指令形 / 已定名笔记定位形）拼接进 explore run prompt 头部；注释**单源自持**（双源互链表述随插件退役删除） |
| 插件 `skills/openspec-explore/SKILL.md` | **（删除）** | 笔记落盘约定文档随插件整目录退役；desktop 侧不再有第二源，`create_explore_note` 定名 + Edit 维护的约定由 stance 文本与工具描述自持 |
