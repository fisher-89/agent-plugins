# 测试设计: change-detail-agent-panel

> **变更**: change-detail-agent-panel
> **日期**: 2026-10-20
> **基于**: proposal.md（验收标准 AC-1 ~ AC-8）、design.md（新增 / 修改文件表、公共函数 / API 表、类型定义、配置表、D1 ~ D20）、specs 四份 delta（`desktop-change-chat` ADDED ×4、`desktop-agent-execution` MODIFIED ×2、`desktop-agent-chat-infra` MODIFIED ×2、`desktop-change-flow-view` MODIFIED ×1）

## 测试框架与布局（既有模式盘点）

- **前端（`pnpm -C packages/desktop run client:test`）**：`vite.config.ts` 的 `test` 节——`include: ['src/**/*.test.ts?(x)']`、`environment: 'jsdom'`、`globals: true`、`setupFiles: ['src/test-setup.ts']`（`ResizeObserver` 空实现 / Web Animations 兜底 / `asyncUtilTimeout` 5s）；`testTimeout` / `hookTimeout` 20s。测试文件与源文件 **colocated 同目录**（`<name>.test.ts` / `<name>.test.tsx`），新增目录沿同惯例（`views/changes/agent/hooks/use-change-chat.test.ts`）。
- **断言与装置**：`import { describe, expect, it, vi } from 'vite-plus/test'`（vitest 风格 `expect`）；组件 / hook 用 `@testing-library/react`（`render` / `renderHook` / `fireEvent` / `act` / `waitFor` / `screen` / `within`）。进程边界 mock 统一 `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` 的 `invokeMock`（按命令名分派对答）+ `ChannelMock`（可编程类，`instances` 捕获 `onmessage` 供测试直投信封）。生成绑定（`src/types/generated/bindings.ts`）底层即 `invoke`，故按命令名断言 `('agent_start' | 'list_change_chat_sessions' | …)` 与参数对象键即等价于绑定位置参数面。
- **Rust（`pnpm -C packages/desktop run server:test`）**：`cargo test --workspace`；命令层测试与源文件同目录 `mod_test.rs`（`#[cfg(test)] mod mod_test;`），中文 `fn` 名即用例名；装置为 `tempfile` 真库（`Env { data_dir, ws_root }`）+ `tauri::test::mock_app` 托管真实 `WorkspaceStores` / `StopRegistry` / `SessionStreamRegistry`；`Channel::new` 真实构造（`discarding_channel` / `capturing_channel`）；写形状守卫在 `src/bindings/mod_test.rs`（重导出生成物后读字节做 `contains` 逐字断言）。
- **本变更测试面**：新增 2 个前端测试文件（`views/changes/agent/change-chat-panel.test.tsx` 承载面板 + composer 断言、`views/changes/agent/hooks/use-change-chat.test.ts`；design D19 定稿 testid 挂钩），扩写 6 个既有前端测试文件（`agent-transport` / `use-agent-session` / `agent-input` / `change-detail-view` / `standard/drawer` / `flow/detail-drawer`），扩写 1 个既有 Rust 测试文件（`commands/exec/mod_test.rs`）；`src/bindings/mod_test.rs` 属**既有形状守卫**（design 明文「本设计不新增测试章节」，断言随动由测试相位承接）。零测试路径的交付面（生成物 / 宏登记面 / 版本号 / 单行 re-export）登记见「不可测试项」；零改动面（`agent-session-view.tsx`）仅留章节框架作回归锚。

---

## 验收范围

<!-- 逐条映射 proposal.md「验收标准」AC-1 ~ AC-8：
  - `AC ID` / `验收条件`：proposal 原文摘录（每条 AC 至少一行）
  - `被测文件或模块`：承载该 AC 的源文件（相对项目根）。源文件 → 测试文件的映射见下方「单元测试」各 `### <源文件> -> <测试文件>` heading；零测试路径面见「不可测试项」 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 环境落到 worktree：非归档带 worktree 的 change，面板会话以 `detail.worktree` 为 cwd，而会话行落 workspace 库（同 root 的 `agentSessions` 可查、workspace 下无第二个 `workspaces/*.redb` 新生） | `packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src/views/changes/change-detail-view.tsx`、`packages/desktop/src/views/changes/agent/change-chat-panel.tsx` |
| AC-2 | 双 root 不变量：`cwd` 传非存在目录 → 发起点显式 `Err`（文案含路径），不进引擎、不产生会话行；`cwd` 省略（调试页 / explore 链）→ 行为与变更前逐字一致 | `packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/hooks/use-agent-session.ts` |
| AC-3 | 归属与清单：库内 `source='change'`、`source_ref=<change id>`；`list_change_chat_sessions` 只返回该 change 的 chat 会话（流程四段 `source_ref` 一条不漏地不出现）；调试页 `change` 筛选照常可见 | `packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/hooks/use-agent-session.ts`、`packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` |
| AC-4 | 新会话 / 历史会话：打开即空态（zero 会话行、zero 取数）且可发送——首发 invoke 的 `sessionId` 为 null（`New` 语义），会话行此刻铸造并带 change 归属，随后呈现且出现在历史清单；「新会话」`reset()` 回空态且立即可再发、旧会话仍可重开；历史点选经 `sessionDetail` + 转录**直查**（零 `source` / `source_ref` 反查） | `packages/desktop/src/views/changes/agent/change-chat-panel.tsx`、`packages/desktop/src/views/changes/agent/change-chat-composer.tsx`、`packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts`、`packages/desktop/src/hooks/use-agent-session.ts` |
| AC-5 | 不自动恢复 + 无重放风暴：页面刷新回空态（不自动选最近一条）；一次 flow run 全程面板的 `session_detail` / 转录 invoke 次数不随 `refreshTick` 增长 | `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts`、`packages/desktop/src/views/changes/change-detail-view.tsx` |
| AC-6 | 禁发与只读：矩阵四行逐行可验（run 期间 / 归档禁用且呈理由、「新会话」禁用、历史仍可切；无 run 的 active 正常收发、空态亦照常可发）；`sessionId === null` 在任何行都不单独构成禁用理由、也不豁免禁发行 | `packages/desktop/src/components/agent/agent-input.tsx`、`packages/desktop/src/views/changes/agent/change-chat-composer.tsx`、`packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts`、`packages/desktop/src/views/changes/agent/change-chat-panel.tsx`、`packages/desktop/src/views/changes/change-detail-view.tsx`、`packages/desktop/src/components/agent/agent-session-view.tsx`（零契约改动，既有注入形态回归锚） |
| AC-7 | 并读成立：抽屉为非模态（无遮罩节点、`data-modal="false"`），左侧 chat 输入与发送全程可交互；抽屉仍是唯一交互入口（内容分派不变），关闭经页脚「关闭」或 Esc | `packages/desktop/src/components/standard/drawer.tsx`、`packages/desktop/src/views/changes/flow/detail-drawer.tsx`、`packages/desktop/src/views/changes/change-detail-view.tsx`、`packages/desktop/src/components/ui/resizable.tsx`（单行 re-export，经折叠用例间接覆盖，见「不可测试项」） |
| AC-8 | 全管线绿：`pnpm -C packages/desktop run check`（含 `bindings:check` 守卫）与 `run test` 全绿；bindings 已重导入库；desktop 版本 0.5.20 | `packages/desktop/src-tauri/src/bindings/mod_test.rs`（既有形状守卫断言随动）、`packages/desktop/src-tauri/src/commands/mod.rs`、`packages/desktop/src-tauri/src/types/generated/bindings.ts`、`packages/desktop/package.json`（后三者无单元测试路径，见「不可测试项」） |

---

## 单元测试

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

<!-- 既有 Rust 命令层测试文件扩写（`#[cfg(test)] mod mod_test;` 与源文件同目录；既有装置 `Env` / `app_with_stores` / `discarding_channel` / `capturing_channel` / `seed_provider` / `seed_sdk_default` 复用，不新建装置）。
  既有列表 / 分页 / 转录 / watch 用例原样保留，仅新增 cwd 面与清单命令面。
  `Env::ws(name)` 既在 `ws_root` 下真建目录（cwd 存在性 fixture 直接复用）；「cwd 不派生新库」断言读 `env.data_dir.path().join("workspaces")` 的文件计数（`crates/infra/store` 的 `workspaces/` 子树派生点，`store.rs:383`）。 -->

#### 待测功能

- `agent_start()`: 命令薄入口尾部 `cwd`（位置索引 8，`Option<String>`）原样透传 `agent_start_with`；参数面与生成绑定同序（零解析残留，形状半边由 `src/bindings/mod_test.rs` 断言）
- `agent_start_with()`: `is_blank_root` 之后插入执行根解析——`cwd` 为 `None` 或 trim 后空串 → 回落 `root`；`Some(p)` 且 `Path::new(p).is_dir()` → `p`，否则 `Err("worktree 目录不存在（可能已被手动删除）: {p}；请恢复目录或手动清理后重建 change")`；`SessionCtx.workspace_root` 取解析值，`stores.for_root(&root)` 逐字不动（双 root 不变量）
- `list_change_chat_sessions()`: `blank root` → 空页信封 `{ sessions: [], total: 0 }`；否则 `for_root(&root)` → `session_query(...).list_sessions(Some("change"), Some(id), offset, limit)`（`id` 原样作 `source_ref`，不 trim / 不校验建档 / 不规范化；`limit` 越界由 store 侧 `MAX_SESSION_PAGE_LIMIT` 截断）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `agent_start` 薄入口参数面形状（`cwd` 末位） | 边界 | 命令体零解析残留：`agent_start(app, on_event, root, prompt, permission_mode, session_id, source, source_ref, agent, cwd)` 以 10 个位置实参同序转调 `agent_start_with`——本文件以该调用点编译通过为形状锚（错位即编译失败），行为断言归下方 `agent_start_with` 用例；生成绑定签名逐字断言归 `src/bindings/mod_test.rs`（双侧锁形，proposal 风险 9） | 单次 |
| `agent_start_with` 执行根进 `SessionCtx`（链路：命令 → 组合根 → 内核） | 正向 | `cwd = Some(env.ws("wt") 真目录)` + 默认 sdk 实例（非法 base_url 使泵即时收敛）→ 提前 resolve 的 running 轮行在场、`session_id` 以 `ses-` 开头；`for_root(root).list_sessions(None, None)` 恰 1 条且 id 与轮行一致 | 单次 |
| `agent_start_with` 双 root 不变量（store 半边恒 root） | 正向 | 同上 fixture 且 `cwd` 指向 **`ws_root` 之外的目录** → `env.data_dir/workspaces` 下 db 文件数在发起前后不变（= 发起前计数，通常 1）；`for_root(&cwd)` 未被调用过（以文件计数**而非**调用 `for_root(&cwd)` 观测，避免断言自身铸库） | 单次 |
| `agent_start_with` `cwd = None` 回归锚 | 正向 | `cwd = None` → 行为与变更前逐字一致：零 `Err`、会话行落 root 库、轮行 running、Channel 收 `record` 收尾（既有用例不改即证，本行补一次显式 `None` 断言） | 单次 |
| `agent_start_with` 空白 `cwd` 回落 root | 边界 | `cwd = Some("")` / `Some("   ")` / `Some("\t\n ")` 三形态 → 零 `Err`、会话行落 root 库（空白按缺省处理，不当「不存在的目录」） | 参数化迭代 |
| `agent_start_with` 归属写侧（`source` / `source_ref`） | 正向 | `cwd = Some(真目录)` + `source = Some("change")` + `source_ref = Some("<change id>")` → 库内会话行 `provenance.source == "change"` 且 `provenance.source_ref == Some("<change id>")`（AC-3 写侧前置） | 单次 |
| `agent_start_with` `cwd` 非存在路径显式 `Err` | 异常 | `cwd = Some("<不存在路径>")` → `Err` 文案含「worktree 目录不存在（可能已被手动删除）」与路径原文；库内零会话行、零轮行、`capturing_channel` 零信封（未进引擎、不推流，AC-2） | 单次 |
| `agent_start_with` `cwd` 指向文件而非目录 | 异常 | `cwd = Some(<临时文件路径>)` → 同款 `Err`（判据是 `is_dir` 而非 `exists`），零落库零推流 | 单次 |
| `agent_start_with` 守卫序：blank root 先行 | 异常 | `root = ""`（或纯空白）+ `cwd = Some(不存在路径)` → `Err("非法 root: 不得为空白（无 cwd 无从发起）")`（root 校验先于 cwd 校验，错误面不串味，D2） | 单次 |
| `agent_start_with` `cwd` 超长字符串 | 边界 | `cwd = Some("a".repeat(1200) 组成的非存在路径)` → `Err` 且文案含完整 1200 字符路径（不截断、不深埋进引擎错误） | 单次 |
| `agent_start_with` `cwd` 特殊字符 | 边界 | `cwd = Some("wt\nwith\nnewline")` / `Some("wt\0null")` / `Some("工作树-🚀")` 三形态（均非目录）→ 各自 `Err` 不 panic；含 emoji 的串若为真目录则接受并落 root 库 | 参数化迭代 |
| `agent_start_with` `cwd` 等于 `root` | 边界 | `cwd = Some(root 本体)` → 与 `None` 等价（`workspace_root = root`），会话行落同一库、零 `Err` | 单次 |
| `agent_start_with` `cwd` 合法而 root 不可寻址 | 异常 | `cwd = Some(真目录)` + `root = <含非法字符的不可 canonical 路径>` → cwd 校验通过后 `for_root(&root)` 的 `Err` 透传（store 半边不以 cwd 兜底寻址，双 root 不变量反向锚） | 单次 |
| `agent_start_with` 相对路径 `cwd` | 边界 | `cwd = Some(".")` / `Some("..")`（相对进程 cwd）→ 按 `is_dir` 实际结果执行（为目录即接受、会话行仍落 root 库），不 panic、不把相对路径改写为绝对路径 | 参数化迭代 |
| `list_change_chat_sessions` chat 归属命中 | 正向 | 同一 root 库内：2 条 chat 会话（`source="change"`, `source_ref=Some("<id>")`）+ 1 条流程会话（`source="change"`, `source_ref=Some("<id>/implement/worker/1")`）+ 1 条他 change 的 chat 会话 → 返回恰 2 条 chat、`total == 2`（AC-3） | 单次 |
| `list_change_chat_sessions` 薄包装等价性 | 正向 | 与 `agent_sessions(state, root, Some("change"), Some("<id>"), 0, 20)` 同参数 → 两个 `SessionPage` 逐字段全等（`sessions` 序、`total`、轮行、`stats.turnCount`） | 单次 |
| `list_change_chat_sessions` 分页信封 | 正向 | 3 条 chat 会话（`updated_at` 递增注入）→ `(offset 0, limit 1)` 取 `updated_at` 最大者且 `total == 3`；`(offset 1, limit 1)` 取第二条；序为 `updated_at` 降序（并列按 id 降序） | 单次 |
| `list_change_chat_sessions` 流程四段零误入 | 异常 | 库内仅流程四段会话（`<id>/<phase>/<role>/<attempt>`）×2 与 `<id>x` 前缀混淆项 ×1 → 返回空 `sessions`、`total == 0`（精确相等而非前缀匹配，AC-3 / proposal 风险 8） | 单次 |
| `list_change_chat_sessions` blank root | 边界 | `root = ""` / `"   "` → `Ok(SessionPage { sessions: [], total: 0 })`，且 `env.data_dir/workspaces` 文件计数不变（守卫先行，不寻址库） | 参数化迭代 |
| `list_change_chat_sessions` 未知 change id | 边界 | `id = "<未建档 id>"` → 空页信封（探测语义选择空页而非 `Err`，D5） | 单次 |
| `list_change_chat_sessions` `id` 空串 | 边界 | `id = ""` → 仅命中 `source_ref == Some("")` 的会话（若 fixture 无则为空页）；不 panic、不把空串当「全量」（≠ 传 `None`） | 单次 |
| `list_change_chat_sessions` `id` 特殊 / 超长 | 边界 | `id = "<id>…".repeat(120).slice(0,1200)`（超长）/ `"chg-🚀"` / `"chg\nid"` 三形态 → 原样作 `source_ref` 精确匹配，零命中即空页，无 panic | 参数化迭代 |
| `list_change_chat_sessions` `offset` 越界 | 边界 | `offset = total + 1` → `sessions == []` 且 `total` 仍为过滤集全量（越界空页语义） | 单次 |
| `list_change_chat_sessions` `offset` 极值 | 边界 | `offset = u32::MAX` → 空页 + `total` 全量（`start >= len` 分支无溢出 panic） | 单次 |
| `list_change_chat_sessions` `limit = 0` | 边界 | `limit = 0` → `sessions == []`、`total` 全量 | 单次 |
| `list_change_chat_sessions` `limit` 超大与截断 | 边界 | 501 条 chat 会话 fixture + `limit = u32::MAX` → 返回 500 条（`MAX_SESSION_PAGE_LIMIT` 截断）、`total == 501`、零 panic（超大列表边界） | 单次 |
| `list_change_chat_sessions` 跨 root 隔离 | 边界 | 会话落 A root 库，以 B root 调清单 → 空页；A root 调用仍命中（库位 = root 寻址） | 单次 |
| `list_change_chat_sessions` root 不可寻址 | 异常 | `root = <含非法字符的不可 canonical 路径>` → `for_root` 的 `Err` 透传不 panic（与既有 `root不可寻址_三查询命令err透传不panic` 同口径） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据存储（`WorkspaceStores` / redb 文件） | 不 mock：`tempfile` 真库（复用既有 `Env` 装置，`WorkspaceStores::open(env.data_dir.path())`），断言直落 `for_root` 读面；「不派生新库」读 `data_dir/workspaces` 文件计数 | 全部用例 |
| 引擎边界（内核 + 组合根 + 工具面） | 不 mock：经既有装置让后台泵即时收敛——`seed_provider` 注入非法 base_url（sdk 路径 `api_error` 同步收敛）或沿用 `TEST_PATH_LOCK` 的 PATH 隔离（cli 路径不可达），不落网络、不触真实 CLI | `agent_start_with` 全部用例 |
| `Channel<AgentRunMessage>` | 不 mock：真实 `Channel::new`（`discarding_channel` / `capturing_channel`，后者逐信封收 `serde_json::Value`） | 全部用途（观测零推流 / 终态收尾） |
| 文件系统目录（cwd 存在性） | 不 mock：`Env::ws(name)` 真建目录、`tempfile` 临时文件作「非目录」fixture；不存在路径为拼接的临时根下未创建名字 | `cwd` 校验与执行根用例 |
| Tauri 运行句柄 | 不 mock 行为：`tauri::test::mock_app()`（`MockRuntime`）经既有 `app_with_stores` 托管真实 `WorkspaceStores` / `StopRegistry` / `SessionStreamRegistry` | 全部用例 |
| 时钟 | 不 mock：`updated_at` / `created_at` 由 fixture 显式给定数值 | 清单排序与分页用例 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 宏登记面（`all_commands!` 纯路径注入、零函数体）：映射到的测试文件是**既有**生成物形状守卫 `src-tauri/src/bindings/mod_test.rs`，不新建测试文件；本节亦在「不可测试项」登记零独立测试路径（无 per-file 行为用例），此处三段只登记既有守卫的断言随动面。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`all_commands!` 宏登记面无导出函数行）：不虚构条目。`list_change_chat_sessions` 的登记效果完全由既有形状守卫的 `COMMAND_WRAPPERS` / `COMMAND_NAMES` 清单断言覆盖。 -->

#### 用例

<!-- 无新增用例：沿用既有守卫（`src-tauri/src/bindings/mod_test.rs` 既有 `#[test]`），本变更只做清单补录与条目数锚随动（36 → 37）。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 既有 `导出产物包含全部32条命令包装名与invoke命令名及出线dto类型名`（`mod_test.rs:241`） | 正向 | `COMMAND_WRAPPERS` 补录 `listChangeChatSessions`、`COMMAND_NAMES` 补录 `list_change_chat_sessions` 后重跑 → 产物 `export const commands = {` 段含该包装名、并含该 invoke 命令名（宏登记在册，零新增用例） | 单次 |
| 既有 `command清单补录_archive_change后36条且既有锚点与在册命令保持`（`mod_test.rs:854`） | 异常 | 缺口检出面：条目数锚 36 → 37（`COMMAND_NAMES.len()` / `COMMAND_WRAPPERS.len()`），首尾锚（`list_changes` / `workspace_config`）与在册命令保持；宏漏登记则上一条 `contains` 判据转红、忘改条目数锚则本条转红（异常面即既有判据自身，不新增负向用例） | 单次 |
| 宏登记面的断言边界 | 边界 | 宏只注入路径、不携参数语义：`list_change_chat_sessions` 的 `root` / `id` / `offset` / `limit` 边界（blank / 空串 / 越界 / 截断）不在本文件断言，归 `exec/mod_test.rs` 命令层用例；本节仅锁「在册与否」（零行为面） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 不 mock：既有守卫经真实 `export_bindings()` 重导出生成物，再以 `fs::read` 读字节做 `contains` 断言（`file_lock()` 串行、`RestoreOnDrop` 兜底恢复）；无进程边界 mock | 全部用例 |

### packages/desktop/src-tauri/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 生成物（禁手改）：`agentStart` 尾部 `cwd: string | null` 与 `listChangeChatSessions` 由 `bindings:export` 铸定，映射到的 `src-tauri/src/bindings/mod_test.rs` 即既有形状守卫；本文件亦在「不可测试项」登记（无独立单元测试路径）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（生成物无手写导出面，`bindings:export` 铸定）：形状断言属既有守卫文件自身（design 明文「本设计不新增测试章节」），不虚构条目。 -->

#### 用例

<!-- 无新增用例：沿用既有守卫的逐字 `contains` 断言，本变更只改锚串（`agentStart` 末位 `cwd`）与清单补录（`listChangeChatSessions`）。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 既有 `agent_start绑定签名含尾部agent入参且invoke参数对象恒含agent键`（`mod_test.rs:307`） | 正向 | `wrapper_head` 逐字串随动改为以 `cwd`（nullable string）收尾，且 invoke 参数对象字面量补 `cwd` 键 → 生成物 `agentStart` 签名与 `agent_start` 参数面同步（10 个位置参数、`cwd` 末位） | 单次 |
| 既有 `导出产物包含全部32条命令包装名与invoke命令名及出线dto类型名` + `session_page出线为sessions与total两字段且agent_sessions绑定返回分页信封`（`mod_test.rs:241` / `:964`） | 正向 | 产物含 `listChangeChatSessions: (root: string, id: string, offset: number, limit: number) => __TAURI_INVOKE<SessionPage>("list_change_chat_sessions", { root, id, offset, limit })` 逐字行（包装名 / invoke 名 / 返回 `SessionPage` 三面） | 单次 |
| 既有 `预先篡改产物文件后重导出恢复权威内容` + `同输入连续两次导出产物逐字节一致且无机器路径嵌入`（`mod_test.rs:493` / `:473`） | 异常 | 手改生成物（如把 `cwd` 删掉改回 8 参）→ 权威快照重导出覆盖、逐字断言转红 → 「禁手改」不被绕过（既有判据承接，不新增用例） | 单次 |
| 生成物的断言边界 | 边界 | 只锁形状不锁语义：`cwd` 的 `null` / 空串 / 超长 / 特殊字符行为不做字节级断言（`contains` 无法表达），语义归 `exec/mod_test.rs`（后端）与 `agent-transport.test.ts`（前端翻译层） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 不 mock：真实 `export_bindings()` + 真实文件字节（`fs::read`）+ `file_lock()` 串行 + `RestoreOnDrop` 恢复；`CARGO_MANIFEST_DIR` 为编译期常量（非 mock 点） | 全部用例 |

### packages/desktop/src-tauri/src/bindings/mod_test.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 自映射：本文件即生成物形状守卫（非行为测试、非被测对象），亦在「不可测试项」登记。段内三段只登记**既有断言改写面**（design 明文该改写由测试相位直接承接），不另立新用例语义。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（守卫脚本无导出面）：不虚构条目。其断言改写面 = `agentStart` 逐字签名以 `cwd: string | null` 收尾 + invoke 参数对象补 `cwd` + 命令清单增 `listChangeChatSessions`。 -->

#### 用例

<!-- 无新增用例：改写既有 `#[test]` 断言的字面锚与清单常量，判据形态（逐字 `contains` / 条目数锚）保持。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 既有 `agent_start绑定签名含尾部agent入参且invoke参数对象恒含agent键`（`:307`）断言改写 | 正向 | `wrapper_head` 末位由 `agent`（nullable number）改为 `agent` 后接 `cwd`（nullable string）（同序 10 参）、invoke 参数对象字面量补 `cwd` 键 → 改写后仍绿且锁住新参数面 | 单次 |
| 既有 `COMMAND_WRAPPERS` / `COMMAND_NAMES` 常量与 `导出产物包含…`（`:241`）、`command清单补录…`（`:854`）断言改写 | 正向 | 两常量各补 `listChangeChatSessions` / `list_change_chat_sessions`（紧随 `sessionDetail` / `session_detail` 之后，与 `all_commands!` 登记序对位）、条目数锚 36 → 37 → 断言仍绿 | 单次 |
| 既有 `预先篡改产物文件后重导出恢复权威内容`（`:493`）的检出能力 | 异常 | 改写只动字面锚与常量、不删断言：产物被篡改（删 `cwd` / 删新命令包装）时 `RestoreOnDrop` 恢复 + `contains` 转红 → 「改写不削弱检出性」 | 单次 |
| 既有 `file_lock` 串行面 | 边界 | 改写后的用例仍持 `lock()` 读 `authoritative_snapshot()`（单一共享产物串行访问），并行 `cargo test` 下产物互斥不被削弱 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 不 mock：真实 `export_bindings()` / `fs::read` / `file_lock()` / `RestoreOnDrop`；产物路径由 `CARGO_MANIFEST_DIR` 编译期常量派生（非 mock 点） | 全部用例 |

### packages/desktop/src/views/changes/agent/change-chat-panel.tsx -> packages/desktop/src/views/changes/agent/change-chat-panel.test.tsx

<!-- 新增源文件 + 新建 colocated 测试文件（`views/**` 既有先例：`change-detail-view.test.tsx` 形态——进程边界仅 mock `@tauri-apps/api/core`，内部组件与 hook 真实组合）。
  design D19 固化的 testid 挂钩：`change-chat-panel` / `change-chat-new` / `change-chat-history` / `change-chat-note` / `change-chat-cwd` / `change-chat-expand` / `change-chat-history-item`；composer 经 `idPrefix="change-chat"` 产出 `change-chat-prompt` / `change-chat-permission-mode` / `change-chat-send` / `change-chat-stop` / `change-chat-disabled-reason`。
  组合用例入口 = 本组件（链路最上层调用方：面板 → `useChangeChat` → `useAgentSession` → transport → `agent_start`）。 -->

#### 待测功能

- `ChangeChatPanel(props)`: 头区动作面（「新会话」/「历史 ▾」/ 状态提示行 / 执行根文案）+ `AgentSessionView` 注入形态对话体（`session` prop）+ `ChangeChatComposer` 插槽 + 折叠窄条；不含自有转录查询或消息拼装
- `ChangeChatComposer(props)`: `AgentInput` 薄适配（`idPrefix="change-chat"`，透传 `running` / `disabled` / `disabledReason` / `onSend` / `onStop`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChangeChatPanel` 打开即空态且可发送（链路：面板 → hook → `agent_start`） | 正向 | 挂载 `{ root, changeId: 'chg-1', cwd: 'C:\\demo\\wt', readOnly: false, runActive: false, collapsed: false }` → `change-chat-panel` 在场；`change-chat-prompt` 可编辑、输入后 `change-chat-send` 可点；挂载期 `session_detail` / `agent_session_transcript` / `list_change_chat_sessions` invoke **零次**（AC-4 / AC-5 打开即空态） | 单次 |
| `ChangeChatPanel` 空态首发即 `New` | 正向 | 输入「首轮提问」→ 点发送 → `agent_start` 参数 `{ root, prompt: '首轮提问', sessionId: null, source: 'change', sourceRef: 'chg-1', cwd: 'C:\\demo\\wt' }`（AC-4 / AC-1） | 单次 |
| `ChangeChatPanel` 首发后会话呈现（`currentSessionId` 真相源） | 正向 | 发送后经 `ChannelMock` 投递 early-resolve 轮行（`record.sessionId = 'ses-1'`）+ `session_detail` 应答该会话 → 投递后 `session-meta-id` 呈 `ses-1`（不以本地 `sessionId=null` 判空态，D10） | 单次 |
| `ChangeChatPanel` 历史清单展开取数 | 正向 | 点击 `change-chat-history` → `list_change_chat_sessions` 恰一次、参数 `{ root, id: 'chg-1', offset: 0, limit: 20 }`；应答 2 条 → `change-chat-history-item` 节点数 2 | 单次 |
| `ChangeChatPanel` 历史点选直查重放 | 正向 | 清单应答 `[ses-1, ses-2]` → 点第二个 `change-chat-history-item` → `session_detail` 与 `agent_session_transcript` 各一次且参数恰 `{ root, sessionId: 'ses-2' }`（**零** `source` / `source_ref` 键，AC-4） | 单次 |
| `ChangeChatPanel` 新会话重置可再发 | 正向 | 已选 `ses-2`（对话体有内容）→ 点 `change-chat-new` → 对话回空、抛出的下一次发送 `sessionId == null`；`change-chat-history` 仍在场且条目仍为 2 条（旧会话可重开，AC-4） | 单次 |
| `ChangeChatPanel` 执行根提示行 | 正向 | `readOnly: false` + `runActive: false` + `cwd: 'C:\\demo\\wt'` → `change-chat-note` 在场且 `textContent` 含 worktree 路径（`change-chat-cwd` 同源，D19）；`cwd: null` 时呈主工作区口径文本且不抛错 | 参数化迭代 |
| `ChangeChatPanel` `runActive` 禁发行（矩阵 row1） | 异常 | `runActive: true` + `readOnly: false` → `change-chat-disabled-reason` 文本「流程运行中」、`change-chat-prompt` 与 `change-chat-send` 均 `disabled`、`change-chat-new` 禁用；点击 `change-chat-history` 仍展开并 invoke 清单（历史可切，AC-6） | 单次 |
| `ChangeChatPanel` 归档只读行（矩阵 row4） | 异常 | `readOnly: true` + `cwd: 'C:\\demo\\wt'`（悬空 worktree 形态）→ 理由「已归档 change 只读」、输入 / 发送 / 新会话全禁用、历史可切；点发送 → `agent_start` 零 invoke（AC-6） | 单次 |
| `ChangeChatPanel` 空态不豁免禁发 | 边界 | 空态（未发生过发送）+ `runActive: true` → 禁用与理由在场；空态 + `readOnly: true` → 同上（`sessionId === null` 不作豁免理由，AC-6） | 参数化迭代 |
| `ChangeChatPanel` 本会话 running 行（矩阵 row2） | 异常 | `runActive: false` + `session_detail` 应答含 running 轮行（库态 `running: true`）→ `change-chat-send` 禁用 + 停止入口 `change-chat-stop` 在场、`change-chat-disabled-reason` 不在场（`disabled` 不顶替 `running`）、`change-chat-new` 禁用（AC-6 row2 / D13） | 单次 |
| `ChangeChatPanel` 折叠窄条与展开入口 | 正向 | `collapsed: true` → `change-chat-expand` 在场、`change-chat-panel` 主体（`change-chat-prompt`）不在场；点击窄条展开入口 → `onToggleCollapsed` 回调恰一次 | 单次 |
| `ChangeChatPanel` 展开态完整骨架 | 正向 | `collapsed: false` → `change-chat-panel` / `change-chat-new` / `change-chat-history` / `change-chat-prompt` 同时在场，`change-chat-expand` 不在场 | 单次 |
| `ChangeChatPanel` 清单空页 | 边界 | `list_change_chat_sessions` 应答 `{ sessions: [], total: 0 }` → 展开后零 `change-chat-history-item`、空占位文案在场、面板不崩 | 单次 |
| `ChangeChatPanel` 清单分页累加 | 边界 | 25 条 fixture（`total: 25`）：首次 `limit 20` → 20 条且「加载更多」入口在场；点击 → 第二次 invoke `offset: 20` → 累加 25 条且入口消失（AC-4 分页信封） | 单次 |
| `ChangeChatPanel` 清单取数失败 | 异常 | `list_change_chat_sessions` reject `'清单取数失败'` → 错误文案原样呈现、面板与历史入口仍在、输入与发送不受影响（降级不阻断会话面） | 单次 |
| `ChangeChatPanel` change id 变化重置 | 边界 | `chg-1` → rerender `changeId: 'chg-2'` → 回空态（`change-chat-panel` 空对话、`currentSessionId` 为 null）、历史清单收起且旧条目不在场；再展开 → invoke 以 `id: 'chg-2'` 取数 | 单次 |
| `ChangeChatPanel` `root = null` | 边界 | `root: null` → 面板在场；输入后点发送 → `agent_start` 零 invoke（`useAgentSession` root 空守，不崩） | 单次 |
| `ChangeChatPanel` 发起失败降级 | 异常 | `agent_start` reject `'worktree 目录不存在（可能已被手动删除）: C:\\gone'` → `agent-session-error` 呈原始错误串、消息区不出半截气泡、输入仍可用（下次可重试） | 单次 |
| `ChangeChatPanel` 首发后清单重取 | 正向 | 空态发送 → 轮行确立 `currentSessionId` 非 null → `list_change_chat_sessions` 调用数 +1（新铸会话进「历史 ▾」）；随后轮收口（`running` true→false）再 +1（D11 三处取数之二三，AC-4） | 单次 |
| `ChangeChatComposer` 前缀与透传 | 正向 | 面板挂载后：`change-chat-prompt` / `change-chat-permission-mode` / `change-chat-send` 均带 `change-chat` 前缀；`runActive: true` 时理由行 `change-chat-disabled-reason` 文本 = 上层传入理由（`disabledReason` 透传无改写） | 单次 |
| `ChangeChatComposer` 档位与发送回调 | 正向 | 切 `permission-mode` 至 `default` 后发送 → `agent_start` 参数 `permissionMode: 'default'`；`onSend` 路径不额外改写 `text` | 单次 |
| `ChangeChatComposer` 无理由态零理由行 | 边界 | `disabled: false` + `disabledReason: null` → `change-chat-disabled-reason` 不在场（不渲染空节点） | 单次 |
| `ChangeChatComposer` 停止入口 | 正向 | `running: true` + `disabled: false` → `change-chat-stop` 在场；点击 → `agent_stop` invoke 携 `{ root, sessionId }` | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` / `Channel`（进程边界 IPC） | `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` 的 `invokeMock` / `ChannelMock`；`invokeMock.mockImplementation` 按命令名分派：`agent_start`（可切 resolve / reject）、`session_detail`、`agent_session_transcript`、`agent_session_watch`、`list_change_chat_sessions`（可队列式应答）、`agent_stop`；`ChannelMock.instances` 捕获 `onmessage` 供测试直投 `{ ipc: 'record' }` early-resolve 轮行 | 全部用例 |
| `useChangeChat` / `useAgentSession` / `AgentSessionView` / `ChangeChatComposer` / `AgentInput` | 真实组合，不 mock：面板布局、判据透传、注入形态与 composer 插槽即被测行为（组合用例入口即本组件） | 全部用例 |
| `list_change_chat_sessions` 应答序列 | IPC 应答注入（队列式 / 按 `offset` 分派）：首发后重取、展开重取、分页第二页各出一次应答；用例内按需重置 | 分页累加、首发后重取、change id 变化重取 |
| 时间显示（`new Date(updatedAt).toLocaleString()`） | 不 mock 且不入断言：清单条目断言取 `change-chat-history-item` 节点数与「含会话 id 前缀」，不取本地化时间文本 | 历史清单全部用例 |
| jsdom 缺件（`ResizeObserver` / Web Animations） | 沿用既有 `src/test-setup.ts` 兜底，不新增 mock | 全部用例 |

### packages/desktop/src/views/changes/agent/change-chat-composer.tsx -> packages/desktop/src/views/changes/agent/change-chat-panel.test.tsx

<!-- design.md 定稿：composer 断言并入 `change-chat-panel.test.tsx`（面板插槽即唯一挂载面，不新建 `change-chat-composer.test.tsx`）。上方 panel 章节内 `ChangeChatComposer` 四行为本文件的映射对应用例；Mock 策略同上表。 -->

#### 待测功能

- `ChangeChatComposer(props)`: `AgentInput` 薄适配（`idPrefix="change-chat"`，`disabled` / `disabledReason` / `running` / `onSend` / `onStop` 全量透传）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChangeChatComposer` 前缀落地与理由透传 | 正向 | 面板挂载（`runActive: true`）→ `change-chat-prompt` / `change-chat-send` / `change-chat-permission-mode` / `change-chat-disabled-reason` 全部在场且前缀一致 | 单次 |
| `ChangeChatComposer` `running` 与 `disabled` 正交 | 异常 | `running: true` + `disabled: false` → `change-chat-stop` 在场、发送禁用；`running: true` + `disabled: true` → `change-chat-stop` 不在场、理由行在场 | 参数化迭代 |
| `ChangeChatComposer` 零理由态 | 边界 | `disabled: false` + `disabledReason: null` → 理由行不在场；`disabledReason: ''` → 不出现字面 `undefined` | 参数化迭代 |

#### Mock策略

同 `change-chat-panel.test.tsx`（同一测试文件与同一装置）。

### packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts -> packages/desktop/src/views/changes/agent/hooks/use-change-chat.test.ts

<!-- 新增源文件 + 新建 colocated 测试文件（`hooks/use-agent-session.test.ts` 形态：`renderHook` + 进程边界 invoke mock，`useAgentSession` 与 `@ai-sdk/react` 真实组合）。
  design D11 三处取数、D13 判据单源、D3 cwd 派生、D8 缺省语义为本文件断言主体；清单命令参数键以 `list_change_chat_sessions` 的 invoke 参数对象观测。 -->

#### 待测功能

- `useChangeChat(params)`: 当前 `sessionId` 本地态（`null` = 草案态，不入禁发判据）+ 注入形态 `useAgentSession({ source: 'change', sourceRef, cwd })` + 清单取数 / 分页 / 三处重取 + 新会话 / 历史点选 + change id 重置 + 禁发判据单源（`disabledReason` / `canSend` / `canStart`）+ `cwd` 派生（`archived ? null : worktree`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useChangeChat` 挂载零取数 | 正向 | `{ root: ROOT, changeId: 'chg-1', worktree: 'C:\\demo\\wt', runActive: false, archived: false }` 挂载 → `sessionId === null`、`sessions == []`、`total === 0`、`listOpen === false`；`list_change_chat_sessions` / `session_detail` / `agent_session_transcript` 均零 invoke（AC-4 / AC-5） | 单次 |
| `useChangeChat` 归属与执行根注入 | 正向 | 同上挂载 → 调 `send({ text: 'hi', permissionMode: 'default' })` → `agent_start` 参数 `{ source: 'change', sourceRef: 'chg-1', cwd: 'C:\\demo\\wt', sessionId: null }`（AC-1 / AC-3 / AC-4） | 单次 |
| `useChangeChat` 判据 row3（可发） | 正向 | `runActive: false` + `archived: false` + 会话未运行 → `disabledReason === null`、`canSend === true`、`canStart === true`；空态（`sessionId === null`）同判（AC-6 row3） | 单次 |
| `useChangeChat` 判据 row1（run 期） | 异常 | `runActive: true` → `disabledReason === '流程运行中'`、`canSend === false`、`canStart === false`；`send()` 后续零 `agent_start` invoke（AC-6 row1） | 单次 |
| `useChangeChat` 判据 row4（归档只读） | 异常 | `archived: true` + `worktree: 'C:\\demo\\wt'` → `disabledReason === '已归档 change 只读'`、`canSend === false`、`canStart === false`、`cwd === null`（归档不传悬空路径，D3 / AC-6 row4） | 单次 |
| `useChangeChat` 判据 row2（本会话 running） | 异常 | `runActive: false` + `session_detail` 应答含 running 轮行 → `disabledReason === null`（running 不进理由）、`canSend === true`、`canStart === false`（AC-6 row2 / D13） | 单次 |
| `useChangeChat` 判据优先级 | 边界 | `archived: true` + `runActive: true` → `disabledReason === '已归档 change 只读'`（归档理由优先于 run 理由，D13 三元序）；两行 `canSend` / `canStart` 均 false | 单次 |
| `useChangeChat` 展开清单取数 | 正向 | `toggleList()` → `list_change_chat_sessions` 恰一次、参数 `{ root, id: 'chg-1', offset: 0, limit: 20 }`；resolve `{ sessions: [s1, s2], total: 2 }` → `sessions.length === 2`、`total === 2`、`listOpen === true`、`listLoading === false`、`listError === null` | 单次 |
| `useChangeChat` 每次展开都重取 | 正向 | `toggleList()` 展开→收起→再展开 → `list_change_chat_sessions` 累计 2 次（首屏重取，非缓存，D11） | 单次 |
| `useChangeChat` 加载更多累加 | 正向 | 首屏应答 20 条 / `total: 25` → `loadMore()` → 第二次 invoke `{ offset: 20, limit: 20 }`；累加后 `sessions.length === 25`；再 `loadMore()` → 零 invoke（`items.length < total` 判据） | 单次 |
| `useChangeChat` 首发后重取（会话 id 确立） | 正向 | 空态 `send()` → 投递 early-resolve 轮行 `sessionId: 'ses-1'` → `currentSessionId === 'ses-1'` 且 `list_change_chat_sessions` 调用数 +1（D11 ②） | 单次 |
| `useChangeChat` 收口重取 | 正向 | 库态 running（running 轮行）→ 重放/收口后 `session_detail` 应答转 completed → `running` true→false → `list_change_chat_sessions` 调用数 +1（D11 ③） | 单次 |
| `useChangeChat` 新会话语义 | 正向 | 已 `selectSession('ses-2')` → `newSession()` → `sessionId === null`、`canSend === true`（回同一可发送空态）、`sessions` 条目保留（旧会话仍可重开）；随后发送 `sessionId == null`（AC-4） | 单次 |
| `useChangeChat` 历史点选直查 | 正向 | `selectSession('ses-2')` → `sessionId === 'ses-2'`、`session_detail` + `agent_session_transcript` 参数恰 `{ root, sessionId: 'ses-2' }`（零 `source` / `source_ref` 反查，AC-4） | 单次 |
| `useChangeChat` change id 变化重置 | 边界 | `chg-1`（已选会话、清单展开）→ rerender `changeId: 'chg-2'` → `sessionId === null`、`listOpen === false`、`sessions == []`；随后 `toggleList()` 以 `id: 'chg-2'` 取数 | 单次 |
| `useChangeChat` `root = null` 守卫 | 边界 | `root: null` → `toggleList()` 零 invoke、`send()` 零 `agent_start` invoke、`sessions == []` 不崩 | 单次 |
| `useChangeChat` `changeId = null` 守卫 | 边界 | `changeId: null` → `toggleList()` 零 invoke（不拼 `source_ref`）、`canSend` 仍按矩阵判定（root 层面允许发送但 body `sourceRef: null`） | 单次 |
| `useChangeChat` 清单取数失败 | 异常 | `list_change_chat_sessions` reject `'清单取数失败'` → `listError === '清单取数失败'`、`listLoading === false`、`sessions` 保持前值（不清空）、面板判据不受影响 | 单次 |
| `useChangeChat` 空页与零增量 | 边界 | 首次应答 `{ sessions: [], total: 0 }` → `sessions == []`、`loadMore()` 零 invoke；第二次应答仍 `total: 0` → 状态稳定不抖动 | 单次 |
| `useChangeChat` `worktree = null`（legacy） | 边界 | `worktree: null` + `archived: false` → `cwd === null`（后端回落 root，D3）；`send()` 的 `agent_start` 参数 `cwd: null` | 单次 |
| `useChangeChat` 分页常量单源 | 边界 | 任一取数 → `limit` 恒为 20（`CHANGE_CHAT_PAGE_SIZE`）；超长清单（`total: 120`）连续 `loadMore()` → offset 序列 `0/20/40/60/80/100`，无跳页无重复 | 单次 |
| `useChangeChat` 并发 `loadMore` | 异常 | `listLoading === true`（首屏未决）期间再 `loadMore()` → 零额外 invoke（在飞守卫） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` / `Channel` | `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` mock；`invokeMock.mockImplementation` 按命令名分派（`agent_start` / `agent_stop` / `session_detail` / `agent_session_transcript` / `agent_session_watch` / `list_change_chat_sessions`），`ChannelMock` 投递 early-resolve 轮行与终态 `record` | 全部用例 |
| `list_change_chat_sessions` 应答 | IPC 应答注入：按调用序 / 按 `offset` 分派，可切 reject；用例内 `mockReset` 后重建 | 清单取数、分页、三处重取、失败降级 |
| `useAgentSession` / `@ai-sdk/react` / `lib/agent-transport` | 真实组合，不 mock：`sessionId` / `running` / `currentSessionId` 派生即被测链路（hook 即链路入口） | 全部用例 |
| `lib/agent-transport` 读参校验 | 不 mock：非法值拒绝行为在 `agent-transport.test.ts` 直测，本文件仅以合法 body 观测透传结果 | 发送类用例 |
| 时钟 | 不 mock：`updatedAt` / `createdAt` 由 fixture 显式给定 | 清单类用例 |

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

<!-- 既有测试文件扩写：新增 `cwd` / `sourceRef` 读参与具名解构后的位置参数序断言；既有「body 会话参数原样进入 agent_start」与「生成绑定调用面 7 键」两条断言随参数面扩至 8 键（设计 D6 / D7）。 -->

#### 待测功能

- `TauriAgentTransport.sendMessages()`: `readSessionParams` 增读 `sourceRef` / `cwd`（`readNullableString`，`string | null` 之外一律 `Err` 且不改写 / 不 trim），`invokeStart` 改「具名解构 → 按生成绑定位置签名传参」（`cwd` 位置 8）
- `TauriAgentTransport.reconnectToStream()`: 订阅 / 重放面不变——唯 `sessionId` + `root`，不携 `cwd` / `sourceRef`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `sendMessages` 两字段透传 | 正向 | body 携 `sourceRef: 'chg-1'` + `cwd: 'C:\\demo\\wt'` → `agent_start` invoke 参数对象恰 9 键（`onEvent` + 会话参数 8）且 `sourceRef` / `cwd` 逐字到达 | 单次 |
| `sendMessages` 具名解构后位置参数序 | 正向 | 同 body（8 个会话参数各取互异的哨兵值）→ invoke 对象逐键等于 body（`root` / `prompt` / `permissionMode` / `sessionId` / `source` / `sourceRef` / `agent` / `cwd`），任一错位即红（proposal 风险 9） | 单次 |
| `sendMessages` 缺省不漂移 | 正向 | body 不含 `cwd` / `sourceRef` → 两键均在且值为 `null`（`agent_start` 参数对象共 9 键，键集合稳定） | 单次 |
| `reconnectToStream` 订阅面回归 | 正向 | `reconnectToStream({ chatId, body })` → 仅 `agent_session_watch` 类 invoke，参数键无 `cwd` / `sourceRef`（订阅面不携执行根，design「不变」行） | 单次 |
| `sendMessages` 非法类型拒绝 | 异常 | `cwd = 42` / `cwd = true` / `sourceRef = 42` / `sourceRef = {}` 四形态 → 分别以 `非法 cwd: …` / `非法 sourceRef: …` 拒绝、`agent_start` 零 invoke、流以错误 reject 且零 chunk（D7 同源校验） | 参数化迭代 |
| `sendMessages` 空串原样透传 | 边界 | `cwd = ''` / `sourceRef = ''` → invoke 收到空串（transport 不 trim、不改写为 `null`） | 参数化迭代 |
| `sendMessages` 超长与特殊字符 | 边界 | `cwd = 'C:\\' + 'a'.repeat(1200)` + `sourceRef = 'chg-🚀\nid'` → 逐字透传（不截断、不转义改写），参数对象值全等 | 单次 |
| `sendMessages` 显式 `null` 与缺席同值 | 边界 | `cwd: null` / `sourceRef: null` 显式给值 → 与缺席形态的 invoke 参数对象全等 | 参数化迭代 |
| `sendMessages` 既有多余字段零影响 | 边界 | body 携未声明的额外键（`foo: 'bar'`）→ invoke 参数对象不含该键（transport 只取声明字段，不整体摊开 body） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` / `Channel` | 沿用既有 `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` mock；`invokeMock` 记录 `agent_start` 入参并可按命令名分派 | 全部用例 |
| `ReadableStream` / `agent-adapter`（`eventToChunk`） | 真实实现，不 mock：流行为与 chunk 序即既有断言面 | 全部用例 |
| 生成绑定（`commands.agentStart`） | 真实生成物，不 mock：`cwd` 末位签名与 invoke 键即被测翻译层（形状另由 `src/bindings/mod_test.rs` 守卫） | 全部用例 |

### packages/desktop/src/hooks/use-agent-session.ts -> packages/desktop/src/hooks/use-agent-session.test.ts

<!-- 既有测试文件扩写：`UseAgentSessionParams` 增 `sourceRef?` / `cwd?`，`sendMessage` body 的 `sourceRef: null` 常量退役改 `?? null`（D8）。既有断言中以 `sourceRef: null` 逐字比较 args 的用例按新参数面扩为 9 键（`cwd: null` 同时在场）。 -->

#### 待测功能

- `useAgentSession(params)`: 参数面增 `sourceRef?` / `cwd?` 并以 `?? null` 透传进发送 body；未提供时行为逐字不变（重放 / 订阅 / 停止语义零漂移）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAgentSession` 两字段进发送 body | 正向 | `{ root, sessionId, source: 'change', sourceRef: 'chg-1', cwd: 'C:\\demo\\wt' }` + `sendMessage` → `agent_start` args `{ …, source: 'change', sourceRef: 'chg-1', cwd: 'C:\\demo\\wt' }`（原 `sourceRef: null` 常量断言改写为实值，AC-3 / AC-4） | 单次 |
| `useAgentSession` 未提供即 null（零漂移） | 正向 | 不传两字段（调试页 / explore 形态）→ args `sourceRef: null` 且 `cwd: null`（键恒在，值缺省；既有 8 键断言扩为 9 键） | 单次 |
| `useAgentSession` rerender 换 cwd 即生效 | 正向 | `cwd: 'C:\\a'` 发送一次 → rerender `cwd: 'C:\\b'` → 第二次 `sendMessage` 的 args `cwd === 'C:\\b'`（依赖数组随动，无陈旧闭包） | 单次 |
| `useAgentSession` 空态首发携 cwd | 正向 | `sessionId: null` + `cwd` 非空 → `sendMessage` args `sessionId: null` 且 `cwd` 在场（`New` 语义与执行根并存，D9） | 单次 |
| `useAgentSession` 重放面不受 cwd 影响 | 正向 | `sessionId` 非空 + `cwd` 非空 → 重放 effect 的 `session_detail` / `agent_session_transcript` 参数仍恰 `{ root, sessionId }`（cwd 不参与寻址 / 重放，AC-4 直查语义） | 单次 |
| `useAgentSession` 非法 cwd 类型（类型断言越过签名） | 异常 | `cwd: 42 as unknown as string` + `sendMessage` → 发起前以 `非法 cwd: 42` 拒绝、`agent_start` 零 invoke、`running` 不被脏值置位 | 单次 |
| `useAgentSession` 显式 `null` 与缺席同值 | 边界 | `{ sourceRef: null, cwd: null }` vs 全缺省 → 两次 `agent_start` args 全等 | 参数化迭代 |
| `useAgentSession` 空串原样透传 | 边界 | `cwd: ''` → args `cwd === ''`（hook 不 trim、不改写为 null，后端按空白回落 root） | 单次 |
| `useAgentSession` 停止寻址不含两字段 | 边界 | `cwd` / `sourceRef` 在场时 `stop()` → `agent_stop` 参数恰 `{ root, sessionId }`（停止面不携执行根） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` / `Channel` | 沿用既有 `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` mock（`session_detail` / `agent_session_transcript` / `agent_session_watch` / `agent_start` / `agent_stop` 按命令名分派，`ChannelMock` 投递信封） | 全部用例 |
| `@ai-sdk/react`（`useChat`）、`lib/agent-transport`、`lib/agent-adapter` | 真实组合，不 mock：hook 即链路入口，发送组装与流折叠即被测行为 | 全部用例 |
| 时钟 | 不 mock：`startedAt` / `updatedAt` 由 fixture 显式给定 | 全部用例 |

### packages/desktop/src/components/agent/agent-input.tsx -> packages/desktop/src/components/agent/agent-input.test.tsx

<!-- 既有测试文件扩写：`AgentInputProps` 增 `disabled?` / `disabledReason?`（D14）。既有 `running` 三例原样保留作回归锚，新增禁用态 describe。 -->

#### 待测功能

- `AgentInput(props)`: 增 `disabled`（textarea 与发送同禁、禁用态**不呈**停止入口、理由文案呈于工具行上方 `data-testid={idPrefix}-disabled-reason`）与 `disabledReason`；`disabled === false` 时 `running` 语义逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentInput` 禁用态全量呈现 | 正向 | `disabled: true` + `disabledReason: '流程运行中'` → `explore-prompt` `disabled`、`explore-send` `disabled`、理由行 `explore-disabled-reason` 文本 = 「流程运行中」 | 单次 |
| `AgentInput` 禁用态不顶替 running 呈现 | 正向 | `disabled: true` + `running: true` → 无停止按钮（`explore-stop` 不在场）、理由行在场（两标志语义正交，D14） | 单次 |
| `AgentInput` `running` 语义回归 | 正向 | `disabled: false` + `running: true` → 停止入口在场、发送禁用、textarea 可编辑、零理由行；`disabled: false` + `running: false` → 无停止入口、输入后可发送 | 参数化迭代 |
| `AgentInput` 禁用态零回调 | 异常 | `disabled: true` + 输入已有文本 → 点击 `explore-send`、`fireEvent.submit` → `onSend` 零调用（禁用不止视觉） | 单次 |
| `AgentInput` 禁用态无理由文案 | 边界 | `disabled: true` + `disabledReason` 缺席 → textarea / 发送仍禁用、`explore-disabled-reason` 不在场（不渲染空占位） | 单次 |
| `AgentInput` 空理由串 | 边界 | `disabled: true` + `disabledReason: ''` → 理由行文本不含字面 `undefined`、不抛错 | 单次 |
| `AgentInput` 超长 / 特殊字符理由 | 边界 | `disabledReason = '原因'.repeat(400)`（>1000 chars）/ `"流程运行中\n🚀"` → 原样呈现、不截断、按文本节点转义（不注入 HTML） | 参数化迭代 |
| `AgentInput` 档位与既有文本保持 | 边界 | `disabled: true` + 先输入文本再 rerender 至禁用 → 文本保留、`explore-permission-mode` 仍呈当前值（不因禁用清空） | 单次 |
| `AgentInput` 既有发送守卫回归 | 边界 | `disabled: false` + 纯空白文本 → 发送禁用；输入非空 → 发送可用（既有空文本边界不变） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯渲染组件：`onSend` / `onStop` 经 props 注入 `vi.fn()` 观察；不 mock 被测组件与 `components/ui/**` 底座 | 全部用例 |
| 时间 / 网络 / 存储 | 不涉及（组件零 IO） | 全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

<!-- 既有测试文件扩写：`DetailLoaded` 纵向单列 → `ResizablePanelGroup orientation="horizontal"`（左 agent 面板 collapsible + 窄条、右 stale note → 动作面板 → 流程图），`DetailHeader` 增面板显隐开关（D15 / D16）。
  既有「列头 / 运行步节点点击 → detail-drawer 挂载 / 卸载」「刷新失败内联横幅位置」「既有 invoke 总账」等断言在分栏改造后须保持绿；两侧 Panel 内的元素查询由 RTL 默认整树查询承担（分栏不改可查询性）。
  组合用例入口 = 本视图（链路最上层：视图 → `ChangeChatPanel` → `useChangeChat` → `useAgentSession` → transport → `agent_start`）。 -->

#### 待测功能

- `ChangeDetailView({ root })`: 分栏宿主（左 `ChangeChatPanel` collapsible + 窄条，右 stale note → 动作面板 → 流程图；header 整宽留在分栏之上）+ `DetailHeader` 面板显隐开关 + change id 变化的折叠复位（签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChangeDetailView` 分栏骨架 | 正向 | 建档 detail fixture（`source: 'active'`、`worktree: 'C:\\demo\\wt'`）→ `resizable-panel-group` 在场且 `aria-orientation="horizontal"`；左 `resizable-panel` 内 `change-chat-panel` 在场、右 `resizable-panel` 内 `flow-graph` 在场；`detail-header` 位于 `resizable-panel-group` 之前（整宽，D16） | 单次 |
| `ChangeDetailView` 右栏内部次序 | 正向 | `refreshFailed` 非空（刷新失败轮）+ 动作面板在场 → 右栏内节点序为 `refresh-stale-note` → 动作面板容器 → `flow-graph`（横幅收进右栏后左栏不位移，D16 / AC-7） | 单次 |
| `ChangeDetailView` 面板显隐开关 | 正向 | 初始展开 → `detail-chat-toggle` 文本「隐藏通用 Agent」；点击 → 文本翻转「显示通用 Agent」且 `change-chat-panel` 主体（`change-chat-prompt`）不在场、窄条 `change-chat-expand` 在场 | 单次 |
| `ChangeDetailView` 开关往返 | 正向 | 再点 `detail-chat-toggle`（或窄条 `change-chat-expand`）→ 文本回「隐藏通用 Agent」、`change-chat-prompt` 复现（两入口共用 `toggle()`，D15） | 单次 |
| `ChangeDetailView` 初拍折叠真相 | 边界 | 挂载即渲染 → `chatCollapsed === false`（`onResize` 首拍无前值不置 collapsed），开关文案为展开态文案、面板完整渲染（D15） | 单次 |
| `ChangeDetailView` change id 变化折叠复位 | 边界 | 收起面板（`isCollapsed()` 真）后 rerender 至 `/changes/<另一 id>` → 折叠复位（开关文案回「隐藏通用 Agent」、面板主体在场，`expand()` 生效） | 单次 |
| `ChangeDetailView` 面板传参（组合） | 正向 | `detail.worktree = 'C:\\demo\\wt'` → 在面板输入并发送 → `agent_start` 参数 `{ cwd: 'C:\\demo\\wt', source: 'change', sourceRef: <URL id>, sessionId: null }`（changeId 取 URL id 而非 `detail.name`，AC-1） | 单次 |
| `ChangeDetailView` 打开即零会话取数（组合） | 正向 | 页面挂载完成（detail 已落位）→ `session_detail` / `agent_session_transcript` / `list_change_chat_sessions` 均零 invoke（AC-4 / AC-5 打开即空态；面板不自建转录查询） | 单次 |
| `ChangeDetailView` 无重放风暴（组合） | 正向 | activeRun 运行中，注入 N（≥10）条密封事件驱动 `refreshTick` 递增 → 面板的 `session_detail` / 转录 invoke 次数恒为 0（无会话）或恒为 1（已选会话），不随 `refreshTick` 增长（AC-5 / proposal 风险 2） | 单次 |
| `ChangeDetailView` 归档 change 只读（组合） | 异常 | `detail.source = 'archive'` + `worktree` 悬空路径 → 面板理由行「已归档 change 只读」、输入与发送禁用、`change-chat-new` 禁用；点历史仍展开并 invoke 清单（AC-6 row4） | 单次 |
| `ChangeDetailView` run 运行中禁发（组合） | 异常 | `activeRun` 非 null（统一查询应答在场）→ 面板理由行「流程运行中」、发送零 `agent_start` invoke、历史可用（AC-6 row1） | 单次 |
| `ChangeDetailView` 无 run 的 active 正常收发（组合） | 正向 | `activeRun: null` + `source: 'active'` → 输入后可发送、`agent_start` 恰一次且 args `sessionId: null`（首发即 `New`，AC-6 row3） | 单次 |
| `ChangeDetailView` legacy `worktree = null`（组合） | 边界 | `detail.worktree = null` → 面板 `cwd` 传 `null`、发送 `agent_start` 参数 `cwd: null`（后端回落 root），图区与动作面板照常（AC-1 边界 / proposal 风险 5） | 单次 |
| `ChangeDetailView` 抽屉挂载 / 卸载回归 | 边界 | 点列头 / 运行步节点 → `detail-drawer` 挂载（`selection` 状态在本组件）；关闭 → 卸载；抽屉 `data-modal="false"` 且无遮罩节点（AC-7 既有断言 + 非模态新断言） | 单次 |
| `ChangeDetailView` 首载失败降级 | 异常 | `get_change_detail` reject → 整页降级页（`detail-note` / `error-note` 既有挂钩）、`change-chat-panel` 零挂载、零 agent 类 invoke（面板不抢跑） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` / `Channel` | 沿用既有 `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` mock（`get_change_detail` / `read_artifact` / `change_flow_*` / `read_exec_output` 既有分派 + 新增 `agent_start` / `session_detail` / `agent_session_transcript` / `agent_session_watch` / `list_change_chat_sessions` / `agent_stop`） | 全部用例 |
| `./flow/attachments` 的 `mountMaterials` | 沿用既有 `vi.mock(import('./flow/attachments'), { spy: true })`（保留原实现、仅观测调用次数，AC-4 素材 memo 断言不失效） | 全部用例 |
| `react-router` | 不 mock：真实 `MemoryRouter` + `Routes`（`/changes/:id`）+ 位置探针（既有装置） | change id 变化复位、深链取数 |
| `react-resizable-panels`（`components/ui/resizable`） | 不 mock：真实组件 + `src/test-setup.ts` 的 `ResizeObserver` 空实现；折叠断言以**命令面结果**为准（`panelRef.isCollapsed()` 表现出的开关文案 / 窄条 testid），不以 `onResize` 像素值断言（jsdom 无布局，ResizeObserver stub 零回调） | 分栏与折叠用例 |
| `views/changes/agent/**`（面板 / hook / composer） | 真实组合，不 mock：判据透传、执行根派生、零取数与零重放即被测行为（组合用例入口 = 本视图） | 全部组合用例 |
| 时间显示 | 不 mock 且不入断言：`created` / `updatedAt` 断言取原始串，不取本地化格式 | 全部用例 |

### packages/desktop/src/components/standard/drawer.tsx -> packages/desktop/src/components/standard/drawer.test.tsx

<!-- 既有测试文件扩写：`StandardDrawerProps` 增 `modal?: boolean = true`，解构后传入 `<Drawer modal={modal}>`（**不**落入 `{...rest}`，D17）。既有 trigger 开合 / footer 单按钮断言原样保留。 -->

#### 待测功能

- `StandardDrawer(props)`: 增 `modal`（缺省 `true`，透传底层 `Drawer`；既有调用方零回归）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `StandardDrawer` 缺省模态 | 正向 | 不传 `modal`（既有调用形态）→ 打开后内容容器 `data-modal="true"`、遮罩节点在场（`[data-slot="drawer-overlay"]`）——既有调用零回归 | 单次 |
| `StandardDrawer` 非模态透传 | 正向 | `modal={false}` → 内容容器 `data-modal="false"`、遮罩节点不在场、title / content / 页脚「关闭」照常呈现 | 单次 |
| `StandardDrawer` 显式 `modal={true}` | 边界 | `modal={true}` 显式给值 → 与缺省形态 DOM 全等（`data-modal="true"` + 遮罩在场） | 单次 |
| `StandardDrawer` `modal` 不落 `DrawerContent` 的 DOM 属性面 | 异常 | `modal={false}` → 内容壳 `hasAttribute('modal') === false`（仅 `data-modal` 由底层给出），`console.error` 无未知属性 / 未知 prop 告警 | 单次 |
| `StandardDrawer` 受控档开合 | 边界 | `open` + `onOpenChange` 外部驱动（不传 `trigger`）+ `modal={false}` → 开合与回调照常；`onOpenChange` 收到 `false` 一次（关闭经页脚按钮 / dismiss） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（`@base-ui/react` Drawer 底座） | 不 mock：真实渲染；`onOpenChange` 以 `vi.fn()` 注入观察面；`containerClassName` 仅作被检属性（既有约定：不作查询挂钩） | 全部用例 |
| jsdom 缺件 | 沿用 `src/test-setup.ts`（`getAnimations` 空实现 + `BASE_UI_ANIMATIONS_DISABLED`），不新增 mock | 全部用例 |
| `console.error` 观测 | `vi.spyOn(console, 'error')`（仅断言无 React 未知属性告警，`afterEach` 复原） | DOM 属性面用例 |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

<!-- 既有测试文件扩写：`<StandardDrawer … modal={false}>`（D17）；内容分派 / 宽度 / 选中驱动逐字不变。文件头注释由「遮罩点击或关闭按钮」改为「页脚关闭按钮 / Esc」（D18 兜底若需）。 -->

#### 待测功能

- `DetailDrawer(props)`: 内部传 `modal={false}`（`data-modal="false"`、无遮罩节点），签名不变；选中驱动 / 内容分派 / `containerClassName` 不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DetailDrawer` 非模态形态 | 正向 | `selection = { scope: 'column', phase: 'implement' }` → 抽屉内容容器 `data-modal="false"`、遮罩节点不在场（AC-7） | 单次 |
| `DetailDrawer` 并读成立（无遮罩拦截） | 正向 | 抽屉打开时，页面上的兄弟按钮（`render` 时与抽屉同树）点击 → 其 `onClick` 回调触发（Viewport 不吃指针事件）；抽屉内容仍呈选中相位素材（AC-7） | 单次 |
| `DetailDrawer` 页脚关闭 | 正向 | 点击页脚「关闭」→ `onClose` 恰一次、内容卸载（非模态主路径） | 单次 |
| `DetailDrawer` Esc 关闭 | 边界 | 抽屉打开时 `fireEvent.keyDown(document, { key: 'Escape' })` → `onClose` 恰一次（库自身 dismiss 或 D18 兜底 effect 承接，**不得双触发**） | 单次 |
| `DetailDrawer` `selection = null` | 边界 | `selection: null` → 组件返回 `null`、零 `data-modal` 节点（既有语义不变） | 单次 |
| `DetailDrawer` 内容分派回归 | 正向 | 列头 → `ArtifactTabs`（本站文档）；WorkerAgent 步（`role !== null`）→ 会话转录（`session_detail` 直查 + `sessionRefreshKey` 触发重查）；`outputRef` 步 → stdout 面板（既有断言原样保留，AC-7「唯一交互入口 / 内容分派不变」） | 单次 |
| `DetailDrawer` 选中面取数失败降级 | 异常 | `session_detail` reject `'查无此 id'` / `read_exec_output` reject → 抽屉内错误态呈现且不崩、页脚「关闭」仍可用、`data-modal="false"` 与「无遮罩」形态不变（非模态不因失败退化） | 参数化迭代 |
| `DetailDrawer` 宽度与容器类回归 | 边界 | 任意 selection → 内容容器 `containerClassName` 仍为 `w-[960px] max-w-[85vw]`（不以渲染结构断言，改取 class 属性） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | 沿用既有 `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` mock（`session_detail` / `agent_session_transcript` / `read_exec_output` 按命令名分派） | 内容分派与转录用例 |
| `SessionTranscriptPanel` / `ArtifactTabs` / `StdoutPanel`（内部模块） | 真实组合，不 mock（内容分派即被测行为） | 全部用例 |
| `StandardDrawer`（`components/standard/drawer.tsx`） | 真实组合，不 mock：`modal={false}` 透传链即被测面（`data-modal` 由 `components/ui/drawer` 底层给出） | 全部用例 |
| 键盘事件（Esc） | 不 mock：`fireEvent.keyDown` 真实派发；不以 mock 替代 dismiss 语义 | Esc 用例 |

### packages/desktop/src/components/agent/agent-session-view.tsx -> packages/desktop/src/components/agent/agent-session-view.test.tsx

<!-- 零契约改动面（design 定稿「无改动」：`disabled` 由面板侧直接传给 `ChangeChatComposer`，`AgentSessionActions` 与组件契约零变化）：映射到的既有测试文件原样重跑作回归锚。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（design 定稿「无改动」）：不虚构条目。既有注入形态用例（`session` prop 即注入、`SessionAbsent` 仅非注入态渲染、composer 插槽仅需 `running`）即本文件的回归判据——若面板改造误触该组件契约即转红。 -->

#### 用例

<!-- 无新增用例：沿用既有 `agent-session-view.test.tsx` 的六个 describe（不新建文件、不改写断言），本变更只重跑作回归锚。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 既有 `AgentSessionView（注入形态）` describe（`agent-session-view.test.tsx:314` / `:329`） | 正向 | `session` prop 注入 → 零额外会话查询、零第二条流，渲染体与自持形态一致；注入空 session（`sessionId: null`）仍呈现（不落自持空态分支） | 参数化迭代 |
| 既有 `AgentSessionView：composer 插槽与只读形态` describe（`:344` / `:377`） | 正向 | render prop 内容渲染于消息区下方、`running` / `loading` 取自会话状态；不传 composer（change 面板历史重放形态）→ 无发送输入、无停止按钮、零动作面接线 | 参数化迭代 |
| 既有 `AgentSessionView：空态与错误面` 的 `会话查询 reject → 错误提示呈现（不抛错、不渲染伪气泡）`（`:400`） | 异常 | `session_detail` reject → 错误提示呈现；面板若把禁发态塞进 `session` prop 或新增强制查询，本条与上两条转红（契约误触即红，构成回归的异常检出锚） | 单次 |
| 契约边界（禁发面挂口） | 边界 | `disabled` / `disabledReason` **不经** `AgentSessionActions` 下传（design 定稿：面板 → `ChangeChatComposer` → `AgentInput`），故 `running` 仍是插槽唯一动作态、`disabled` 三态（true / false / undefined）不在本文件断言——边界归 `agent-input.test.tsx` 与 `change-chat-panel.test.tsx` | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` / `Channel` | 同既有 `agent-session-view.test.tsx` 装置（`vi.mock('@tauri-apps/api/core')` + `vi.hoisted` 的 `invokeMock` / `ChannelMock`），本变更不新增 mock 条目 | 全部用例 |
| `useAgentSession` / `AgentMessages` / `agent-blocks` | 真实组合，不 mock（既有约定：组件即链路入口，零内部 mock） | 全部用例 |


## 不可测试项

| 模块 | 原因 |
|---|---|
| `packages/desktop/src/components/ui/resizable.tsx` | 单行 `export { usePanelRef } from 'react-resizable-panels'`：`usePanelRef` 无自有行为（不做包装、不改写返回值），独立测试只能断言「re-export 与上游同一函数引用」，属同义反复；且 `components/ui/**` 全目录无测试先例并在 `vite.config.ts` 的 `NO_STATIC_CHECK` 忽略面内。其消费行为（`collapse()` / `expand()` / `isCollapsed()`）由 `change-detail-view.test.tsx` 的折叠 / 复位用例经命令面结果端到端覆盖（D15） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` 宏登记面（纯路径注入、零函数体）：登记正确性由既有生成物形状守卫 `src/bindings/mod_test.rs`（命令清单含 `listChangeChatSessions`、`bindings:export` 幂等重导出）与 `pnpm run bindings:check`（git diff 守卫）承接；不为其设 per-file 行为用例（design 明文该守卫断言由测试相位改写，不新增测试章节） |
| `packages/desktop/src-tauri/src/types/generated/bindings.ts` | 生成物（禁手改）：`agentStart` 尾部 `cwd: string \| null` 与 `listChangeChatSessions(root, id, offset, limit)` 由 `bindings:export` 铸定，正确性由 `src/bindings/mod_test.rs` 逐字 `contains` 断言 + `bindings:check` 守卫承接；生成物本身无独立单元测试路径 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 本文件即生成物形状守卫（非被测对象）：其断言改写（`agentStart` 签名以 `cwd: string \| null` 收尾、invoke 参数对象含 `cwd` 键、命令清单增 `listChangeChatSessions`）由测试相位直接改写既有断言，不为其再设计用例（design「修改文件」表说明） |
| `packages/desktop/package.json` | 版本 bump（0.5.19 → 0.5.20）：无单元测试路径；由桌面端静态检查 / 打包流程（`run check` / `bindings:check` / 发布打包读 `version`）承接 |
| `crates/infra/store`（`query_sessions` 的 `source_ref` 精确匹配直测） | proposal 将「两段 vs 四段不撞」列为**若补**的固化断言，但 design 把 `crates/infra/store/**` 明列为只读红线（零 schema / 零查询语义改动、零 delta）——故不在 store crate 新增测试；不撞性改由 `list_change_chat_sessions` 命令层用例（流程四段 / `<id>x` 前缀混淆项零命中）固化，等价锁定同一语义（proposal 风险 8） |
| 面板视觉细节（头区文案像素、窄条宽度 `3rem`、历史清单虚拟滚动形态、折叠动画） | design D19 / 待决 3 明示不固化像素，行为面（可见性、禁用理由文本、testid 挂钩、折叠开关文案）已在 per-file 章节以断言锁定；样式类与排版由 `vp check` 静态检查承接，不设单测 |
| 真实引擎侧可观测面（`SessionCtx.workspace_root` 落到工具沙箱根 / bash `current_dir` / 每轮 `AGENT.md` 读取点均在 worktree 内） | `crates/core/agent` / `crates/infra/agent` 在本变更只读红线内、且真实工具执行需 CLI / 网络（本仓库测试装置以非法 base_url / PATH 隔离让泵即时收敛，不跑真实工具）。执行根接线的可测代理面已覆盖：`cwd` 非目录即 `Err`（校验在引擎之前）、会话行落 root 库而 cwd 不派生新库（双 root 不变量）、`cwd` 逐字进入 `agent_start` 参数面；「沙箱内可验」由桌面端静态 / 手工验收承接（AC-1 文档面） |
