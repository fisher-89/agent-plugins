# 提案: desktop-workspace-db-split

> **变更**: desktop-workspace-db-split
> **日期**: 2026-09-29
> **状态**: proposed

---

## 问题

packages/desktop 当前以**单一 db 文件**（`home_dir()/.dev-team/desktop-store.redb`）承载全部持久化数据：workspace 注册表（`WorkspaceRecord`）、agent 运行记录与事件流（`AgentRunRecord` / `AgentEventRecord`）、explore 清单（`ExploreRecord`）共四个模型同库。所有 workspace 域数据（运行历史、explore 绑定与会话链）与全局数据（注册表）物理混居一库，导致：

1. **无物理隔离**：任一 workspace 的数据损坏（文件级）波及全局注册表与全部 workspace 的历史；
2. **不可分域治理**：无法按 workspace 备份 / 搬迁 / 清理运行历史，db 查看器也只能整体浏览；
3. **维度语义悬空**：desktop-data-dimensions 已划分 user / workspace 两维度，但 workspace 维度至今无落盘载体，且其现行裁定（workspace 维度数据落 workspace repo 内）与用户最新要求（每 workspace 独立 db 文件、**依然放在全局目录下**）直接冲突。

用户需求：将 workspace 数据与全局数据分离，每个 workspace 独立 db 文件，仍置于全局数据目录下；**无需考虑兼容历史数据**（2026-09-29 补充裁定）。

---

## 提案

**双库布局**：按数据维度拆分落盘载体，以**全新文件组冷启动**——

- **全局库**：仅注册 `WorkspaceRecord`（跨 workspace 的注册表）及未来 user 维度租户（workflow 过程数据、设置、窗口状态——desktop-data-dimensions 既有裁定不变）；
- **workspace 库**（新）：`home_dir()/.dev-team/workspaces/` 子树下**每个 workspace 恰一个独立 db 文件**，注册 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三个 workspace 维度模型。文件路径由 canonical root 经单点纯函数确定性派生（哈希成分抗碰撞，文件名不含路径成分），同根恒同名、跨重启可复现。

**归属裁定**：每条 run 的 `cwd` 恒为当前 workspace root（前端固定传 root），`ExploreRecord` 自带 `root`——三者归 workspace 维度；注册表归 user 维度。**模型 shape 零变化**（native_model id / version / 字段面全部不动），本变更只平移落盘归属，非 shape 演进。

**历史数据**：不迁移、不搬移、不做格式探测——沿用 store 既有「不做旧库兼容」立场（redb 兼容层与版本 from-chain 均已退役的同一先例）。新布局以全新文件组落地（全局库启用新文件名，dev-design 定名），旧单库文件 `desktop-store.redb` 成为**惰性残留**：不读、不改名、不删除；首启后 workspace 清单从空开始（用户重新添加目录），存量运行历史与 explore 绑定随旧库废弃（用户已裁定接受）。

**store 层**：`Store` 类型与 `Store::open(path)` 注入式打开语义不变（仍是单文件句柄）；新增 workspace 库路径派生单点与 `WorkspaceStores` 注册表——全局操作走全局库，workspace 域操作按 root 解析对应库实例；同一 workspace db 文件进程内 MUST NOT 重复打开（redb/native_db 单文件写锁硬约束），句柄按 root 复用缓存。数据目录根仍由 desktop-app 注入，store 内零环境解析（既有约束不变）。

**命令面**：workspace 域命令统一 root 寻址——`agent_runs` / `agent_run_events` / `agent_run_chain` / `agent_stop` 新增（或等效使用）root 参数，路由至当前 workspace 库；运行清单随之收窄为**当前 workspace 的历史**（调试页本就 workspace 态可达，跨库聚合视图不再提供）。`agent_stop` 的 `RunStopRegistry` 寻址键随 id 域演进消解歧义（run id 改为 workspace 库域内自增，裸 id 跨库有歧义）。db 查看命令（`db_models` / `db_records`）增加 scope（全局库 / 当前 workspace 库），信封 API 零模型特定代码不变。explores 轨道命令签名不变（root 本就是入参），仅内部路由切换。

**生命周期**：`add_workspace` 注册成功后预开对应 workspace 库（坏文件注册时暴露，fail fast 口径同 setup）；`remove_workspace` 仅删注册记录，**workspace db 文件保留**（不自动清理）——重新添加同根目录时历史完整恢复；孤儿文件治理二期再议。

---

## 能力

### 新增能力

- 无——本变更全部为既有能力的修改与收敛，不引入新域能力。

### 修改的能力

- **desktop-workspace-store** — 双库布局与模型注册分组、workspace 库文件确定性寻址单点、`WorkspaceStores` 注册表与进程内单开、全新文件组冷启动（旧库惰性废弃，零迁移）、workspace 域命令 root 寻址解析入口；`ExploreRecord` 维度声明由 user 改 workspace。
- **desktop-data-dimensions** — workspace 维度落盘方案反转：由「workspace repo 内」改为「全局目录下 per-workspace db 文件」（用户裁定，2026-09-29）；维度代表清单重裁定（agent 运行历史、explore 清单归 workspace 维度）；「三笔账」中 gitignore 分型与克隆可重建两笔随「数据不进 repo」消灭，独占锁一笔由 store 侧承接。
- **desktop-agent-execution** — run 事件落库维度由 user 改 workspace（落所属 workspace 库）；`agent_runs` / `agent_run_events` / `agent_run_chain` / `agent_stop` root 寻址；run id 语义演进为 workspace 库域内。
- **desktop-db-inspector** — 查看面增加 scope 寻址（全局库 / 当前 workspace 库），两库清单与扫描互不混列；信封 API 与零模型特定代码约束不变。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — `models()` 拆全局 / workspace 两组静态注册；workspace 库路径派生单点；`WorkspaceStores`（全局库持有 + per-root 实例缓存、`global()` / `for_root(root)`）
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — 模型按维度分组注册（**模型 struct 字段面与 native_model id / version 不动**）
- `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` — 公共导出面（`WorkspaceStores` 等）
- `packages/desktop/src-tauri/src/main.rs` — setup 打开全局库 + 挂载 `WorkspaceStores`；db 落位常量演进（全局库新文件名 + `workspaces/` 子树语义）
- `packages/desktop/src-tauri/src/commands/workspaces/mod.rs` — `State` 切换至 `WorkspaceStores`（注册表操作走全局库；`add_workspace` 预开校验）
- `packages/desktop/src-tauri/src/commands/explores/mod.rs` — root → workspace 库路由（命令签名不变）
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` + `agent.rs` — `agent_start` / `agent_stop` / `agent_runs` / `agent_run_events` / `agent_run_chain` root 寻址；`RunStopRegistry` 寻址键演进
- `packages/desktop/src-tauri/src/commands/db/mod.rs` — `db_models` / `db_records` scope 寻址
- 前端接线：`src/hooks/use-agent-chat.ts`、`src/views/agent/hooks/use-agent-run-history.ts`、`src/views/agent/agent-debug-view.tsx`、`src/views/explores/hooks/use-explore-session.ts`、`src/views/db/hooks/use-db-inspector.ts` 及相关视图（root / scope 参数透传、db 页 scope 切换）
- `packages/desktop/src/types/generated/bindings.ts` — 随 build 再生成
- `packages/desktop/package.json` — version bump（用户可见行为变更：数据隔离 + 运行清单范围收窄 + 存量数据废弃）+ 产物 rebuild

### 测试文件

- `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` — 双库布局、路径派生确定性、注册表单开复用、旧库惰性废弃冷启动、`remove_workspace` 文件保留
- `packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs`、`explores/mod_test.rs`、`exec/mod_test.rs`、`db/mod_test.rs` — 路由正确性与 workspace 间隔离
- `packages/desktop/src/hooks/use-agent-chat.test.ts`、`src/views/agent/hooks/use-agent-run-history.test.ts`、`src/views/explores/hooks/use-explore-session.test.ts`、`src/views/db/hooks/` 下测试、`src/app.test.tsx` — invoke 入参面（root / scope）更新

### 删除文件

- 无（旧单库文件 `desktop-store.redb` 为用户磁盘上的惰性残留，运行时不删除；是否提供清理入口二期再议）

### 不要修改

- 四个模型 struct 的字段面与 native_model id / version（拆分是落盘归属平移，非 shape 演进；`AgentRunRecord.cwd` / `ExploreRecord.root` 原样兼任归属键）
- `crates/core/foundation/src/layout.rs`（`resolve` / `domain_dir_name`：openspec 域目录解析，与 db 落位无关）
- desktop-data-dimensions 的「workflow 过程数据归 user 维度」裁定（workflow 过程数据落全局库、二级索引检索、不依赖分库——与本变更不冲突：其检索约束约束的是 user 库内实现方式，非落盘维度）
- `tests/golden` 线面契约 fixture（DTO 纯 derive、detail 线面冻结口径不变）
- app data dir 与 home_dir 的全局目录口径差异（现状代码为 `home_dir()/.dev-team`，spec 行文有 `app data dir` 残留——口径统一另行裁定，不在本变更范围）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 双库布局与模型注册分组 | 全局库仅注册 `WorkspaceRecord`；workspace 库注册 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`；全局库与 workspace 库（`home_dir()/.dev-team/workspaces/` 子树）文件面分离，测试断言同 root 跨重开派生同一路径 |
| AC-2 | 数据分流写入 | 集成测试：`add_workspace` 后全局库含注册记录；对某 workspace 执行 `create_explore_record` 与 `begin_agent_run` 后，记录落在该 workspace 库，全局库无混入 |
| AC-3 | workspace 库进程内单开与复用 | 同 root 多次 `for_root` 复用同一打开实例（不触发 redb 文件锁冲突）；不同 root 各自独立实例；路径派生为单点纯函数，消费侧零派生逻辑 |
| AC-4 | 全新文件组冷启动（零迁移） | 预置旧四模型单库文件与旧路径：新布局启动照常成功、不读旧文件、不改名不删除；全局库与 workspace 库从空开始按新布局写入；无任何迁移 / 格式探测代码 |
| AC-5 | 命令面 root 寻址与 workspace 隔离 | `agent_runs` / `agent_run_events` / `agent_run_chain` / `agent_stop` 携 root 路由至对应 workspace 库；workspace A 与 B 各有 runs（含同 id 并行）时互不可见、`agent_stop` 不跨库误停；调试页运行清单仅呈现当前 workspace 历史 |
| AC-6 | db 查看双库 scope | `db_models` / `db_records` 按 scope 返回对应库的模型清单与记录，两库互不混列；信封 API 与查看器零模型特定代码（git diff 无 per-model 分支） |
| AC-7 | `remove_workspace` 文件保留 | remove 后注册记录消失、workspace db 文件仍在；重新 add 同 root 后 explore 清单与会话链历史完整可读 |
| AC-8 | 模型 shape 零变化 | 审查确认四模型 native_model id / version 与字段面不变；前端 DTO 除新增 root / scope 参数外不变；`tests/golden` 契约未触碰 |
| AC-9 | 维度 spec 自洽 | desktop-data-dimensions 拆分后语义自洽：workspace 维度落全局目录 per-workspace db、不进 repo；workflow 过程数据维持 user 维度裁定不变 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 存量 explore 绑定、会话链与运行历史随旧库废弃，注册清单清空需用户重新添加 | 一次性用户体验损失 | 已裁定接受（用户 2026-09-29 明示无需兼容历史数据） | 变更说明留痕；旧文件不删除，磁盘可考 |
| run id 改为 workspace 库域内自增，裸 id 跨库歧义（`RunStopRegistry`、前端 id 引用） | 停止错 run、链还原错库 | 中 | 命令面统一 root 寻址；registry 寻址键含 root；测试覆盖双 workspace 同 id 并行场景 |
| 双开同一 workspace 库文件（dev 与正式版并行） | 文件锁错误 | 中 | 既有 tauri-plugin-single-instance + 单进程约束留痕承接（现状继承，非新增敞口）；错误经 `Err(String)` 显式呈现 |
| workspace 库文件派生若含可读路径段，泄露目录结构于全局目录清单 | 低（本机数据目录，非共享面） | 低 | 哈希成分为主、可读段可选（dev-design 定）；文件名不含完整路径 |
| 旧版本应用遭遇新布局（更新回滚 / 降级） | 旧版读不到新库，表现为空数据 | 低 | 更新器单向前行，回滚非支持路径，风险留痕 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| workspace 数据范围界定（哪些模型分离） | `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 归 workspace 维度；`WorkspaceRecord` 与未来 workflow 过程数据、设置留全局库 | 每条 run 的 cwd 恒为当前 workspace root（前端固定传 root），explore 记录自带 root——天然 workspace 域；注册表跨 workspace；workflow 过程数据已有 user 维度裁定且无生产者 | 只移 explore 不移 runs（隔离不彻底）；全部不动（不满足需求） |
| workspace 库落盘位置 | 全局数据目录下 `workspaces/` 子树，每 workspace 恰一个 db 文件 | 用户明确要求「依然放在全局目录下」；数据不进 repo，使原「三笔账」中 gitignore 分型与克隆可重建两笔直接消灭 | workspace repo 内落盘（既有裁定，被本次用户需求反转，留档退役）；单库加 workspace 列（无物理隔离，不满足需求） |
| workspace 库文件命名 | canonical root 经单点纯函数确定性派生，含哈希成分抗碰撞 | 路径字符串直接作文件名有非法字符 / 长度 / Windows 大小写归一等坑；需跨重启稳定复现 | 原路径拼接（脆）；数据库内 root→文件映射表（鸡生蛋：映射自身落哪库） |
| 存量数据处置 | 零迁移：全新文件组冷启动，旧单库惰性废弃（不读 / 不改名 / 不删） | 用户裁定无需兼容历史数据；与 store 既有「不做旧库兼容」立场（redb 兼容层、v1/v2 from-chain 退役先例）完全同轨，零迁移代码零维护面 | 一次性同引擎搬移（否决：用户裁定无需兼容）；原路径子集模型打开（否决：依赖 native_db 未验证行为，为保一份可重建的注册清单引入 spike 不值） |
| 全局库是否沿用原文件路径 | 启用新文件名（dev-design 定名） | 旧文件为四模型旧布局，换名使新旧布局文件面彻底无歧义，免格式探测 | 沿用 `desktop-store.redb`（需验证 native_db 子集模型打开行为，引入 spike） |
| 运行清单范围 | `agent_runs` 收窄为当前 workspace 的历史 | workspace 分离语义的直接体现；调试页本就 workspace 态可达；跨库聚合需打开全量库，违背分离初衷 | 保留跨 workspace 聚合清单（需聚合扫描全量 workspace 库） |
| `agent_stop` 跨库歧义消解 | 命令携 root（或等价复合寻址），registry 键随 root 消歧 | run id 变 workspace 库域内，裸 id 双 workspace 同 id 并行必歧义 | 全局 id 分配器（引入跨库写耦合与全局单点，否决） |
| `remove_workspace` 是否删 workspace 库文件 | 保留文件（不自动清理） | 移除注册 ≠ 销毁历史；重加同根目录历史完整恢复；误删注册不致数据丢失 | 随注册删除（不可逆数据丢失）；改名归档（引入归档区概念，二期再议） |

### 待决问题

- 全局库新文件名定名与 workspace 库文件名组成（哈希算法、可读段取舍，dev-design 定）
- workspace 库句柄缓存策略：进程生命周期常开（当前倾向）vs LRU 上限
- `remove_workspace` 的彻底清理入口（含 workspace db 文件与旧全局库残留的删除，需确认交互）是否二期提供
- db 查看页 scope 切换的交互形态（双 tab vs 下拉），dev-design 随页面设计定

---
