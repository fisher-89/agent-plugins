# 提案: move-queries-command-to-change

> **变更**: move-queries-command-to-change
> **日期**: 2026-10-03（基于 2026-10-02 草案复核合并：版本现状事实随 agent-turn-eof 归档更新）
> **状态**: 草案

---

## 问题

desktop 桌面端的 change 域命令被拆在两个命令组里，域边界与命令组边界错位：

1. `commands/queries/` 现存的三个命令（`list_changes` / `get_change_detail` / `read_artifact`）**全部**是 change 域读面命令——"queries 轨道" 名义上是通用查询轨道，实际已退化为 change 域读面的孤儿落位（explore 查询早已独立成 `commands/explores/` 组，workspace 查询在 `commands/workspaces/`，db 查询在 `commands/db/`，通用查询轨道名存实亡）；
2. `commands/changes/mod.rs` 的模块 doc 声明 change 域三分职责：`queries`（纯读）/ 本组（记录面写）/ `change_flow`（run 编排控制）——读面与写面分属两组，但 explores 组已有「读 + 记录面」同组先例（`read_explore` / `scan_explores` 与记录 CRUD 共组），change 域反而比 explore 域拆得更碎，无真实理由；
3. 命令组是 IPC 命令的唯一落位叙事（desktop-app-shell「Tauri command 轨道组织」），"queries 轨道只剩 change 读命令" 使该叙事持续失真，后续特性落位（如 change 域新查询）需要在错误的轨道叙事下重新论证归属。

---

## 提案

将 `commands/queries/` 的三个 change 域读命令**原样搬迁**至既有 change 域命令组 `commands/changes/`，`queries` 轨道随之消解（目录删除，不留空壳——沿 exec 轨道「MUST NOT 出现空壳」同款纪律）：

1. **命令原样搬迁**：`list_changes` / `get_change_detail` / `read_artifact` 三命令与 `is_blank_root` 助手平移进 `commands/changes/mod.rs`，函数名、签名、命令体逻辑零改动；`all_commands!` 清单中三命令的登记路径由 `$crate::commands::queries::*` 改为 `$crate::commands::changes::*`；
2. **IPC 面零变化**：命令名与类型零改动 → specta 出线零变化 → 重导出后 `src/types/generated/bindings.ts` 零 diff（`bindings:check` 的 `git diff --exit-code` 守卫即为验收面）；前端按函数名从生成 bindings 导入，零改动；
3. **组内两种 blank root 口径并存且不互换**：读命令保持「blank root → 空结果语义」（`is_blank_root` 早退），`create_change` 保持「blank root → 显式 `Err`」（写无空结果语义）——合并后同组两口径是既有语义，MUST NOT 互相污染；
4. **测试合并**：`queries/mod_test.rs` 的 7 个测试并入 `changes/mod_test.rs`（两组各有 TempWs 装置，统一为单一装置、统一临时目录前缀），测试内容除模块路径外零改动；
5. **叙事收口**：`changes/mod.rs` 模块 doc 由「三分职责」更新为「二分」——本组（读 + 记录面，沿 explores 组先例）/ `change_flow`（run 编排控制）；spec 基线同步 desktop-app-shell 轨道组织与 desktop-change-create 落位表述。

**版本裁决**：纯内部重构，无用户可见变化，`packages/desktop` 版本不 bump（沿「仅用户可见变更才提升」纪律；现状 0.4.2 系 agent-turn-eof 缺陷修复归档所 bump，本变更在其上零提升）。

---

## 能力

### 新增能力

- 无。

### 修改的能力

- **desktop-app-shell** — 「Tauri command 轨道组织」requirement 的 queries 轨道条款改为 `commands/changes/` 组（读 + 记录面同组、queries 轨道消解不留空壳）；「workspace 注册命令轨道」「command body 纪律与 app 层微形态」两 requirement 中的轨道枚举随改。
- **desktop-change-create** — 「create_change IPC 命令面」requirement 的落位表述由「`commands/changes/` 新命令组」更新为该组同组承载 change 域读命令（读 + 记录面同组沿 explores 先例）；Module Contract 的 `commands/changes/mod.rs` 行随归档同步。

### 沿用（语义不变，仅被引用）

- desktop-change-queries（workflow crate 核心查询层，本次不动——命令层搬迁不触及 `crates/core/workflow/src/queries/`）
- desktop-ipc-type-bindings（bindings 管线不变，重导出幂等性即验收面）
- desktop-page-routing / desktop-change-flow-view（按命令名引用读命令，零影响）

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — 接收三读命令与 `is_blank_root` 助手；模块 doc 更新（三分职责 → 读 + 记录面二分，沿 explores 组先例）
- `packages/desktop/src-tauri/src/commands/mod.rs` — 移除 `pub mod queries;`；`all_commands!` 三命令登记路径 re-path（单一登记面，`main` 的 `generate_handler` 与 specta builder 同源自动跟随）
- `packages/desktop/src-tauri/src/commands/explores/mod.rs` — 仅 doc 注释：`is_blank_root` 的「同 `commands::queries` 口径」引用改为指向 `commands::changes`（注释级改动，无逻辑）
- `packages/desktop/src/types/generated/bindings.ts` — 重导出验证零 diff（不改内容；`bindings:check` 守卫）

### 测试文件

- `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` — 并入 queries 侧 7 个测试（纯透传等值 / 详情 DTO / 未知 change → None / 敌意 source 拒绝 / blank root 空结果语义等），TempWs 装置统一为单一实现

### 删除文件

- `packages/desktop/src-tauri/src/commands/queries/mod.rs`
- `packages/desktop/src-tauri/src/commands/queries/mod_test.rs`

### 不要修改

- 三命令的函数名 / 签名 / 命令体逻辑与 DTO（IPC 兼容红线——bindings 零 diff 是验收面）
- `crates/core/workflow/**` — 核心查询层与写面零改动（`crate::queries::list_changes` 等核心层引用与命令组搬迁无关）
- 前端 `src/**`（hooks / views 按函数名导入生成 bindings，不受 Rust 模块路径影响）
- `commands/change_flow` / `commands/explores` / `commands/agents` / `commands/workspaces` 等其他命令组的逻辑
- `plugins/dev-team/**`
- `openspec/specs/**` 既有基线（增量以本变更 specs/ delta 表达，归档时同步）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 命令落位与轨道消解 | `list_changes` / `get_change_detail` / `read_artifact` 三命令实现位于 `commands/changes/mod.rs`；`commands/queries/` 目录不存在（无空目录 / 空模块残留）；`src/commands/mod.rs` 无 `pub mod queries;`；`cargo check` 通过 |
| AC-2 | IPC 面零变化 | `all_commands!` 仍登记同名三命令（路径改经 `changes::`）；`pnpm -C packages/desktop run bindings:check` 通过（重导出后 `src/types/generated` 零 diff）；前端源码零改动 |
| AC-3 | 语义零变化 | 合并后 `changes/mod_test.rs` 覆盖原 queries 侧全部 7 个测试与原 create_change 侧 4 个测试，`cargo test` 全绿；blank root 双口径并存不互换——读命令返回空结果 / `None`，`create_change` 返回显式 `Err` |
| AC-4 | 叙事与 spec 基线 | `changes/mod.rs` 模块 doc 声明「读 + 记录面」二分组织（沿 explores 先例），不再引用 `queries` 独立轨道；specs/ delta 覆盖 desktop-app-shell 三处 requirement 与 desktop-change-create 落位表述 |
| AC-5 | 版本不 bump | `packages/desktop/package.json` version 保持 0.4.2 不变（0.4.2 为 agent-turn-eof 归档 bump 后的现状值，本变更加其上零提升）；`plugins/dev-team` 无任何改动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 搬迁中意外改动命令签名 / DTO 导致 bindings.ts 出 diff | 前端类型面漂移，破坏「IPC 零变化」承诺 | 低 | 命令体逐字平移（仅模块路径变）；`bindings:check` 的 `git diff --exit-code -- src/types/generated` 作硬守卫，AC-2 直接拦截 |
| 测试合并时装置冲突（两组各有 TempWs，临时目录前缀不同） | 合并后编译失败或测试互踩临时目录 | 低 | 统一为单一 TempWs 装置与统一前缀（进程 id + tag 隔离先例已在用）；AC-3 全绿验收 |
| blank root 双口径在同组内被误统一（读改 Err 或写改空结果） | 前端空态 / 错误态语义漂移（空 workspace 误报错误，或写失败静默） | 低 | 两种口径的既有测试各自在合并后保留并全绿（`root为空字符串…空列表不panic` vs `blank_root显式err不进写面链路`）；AC-3 明示「并存不互换」 |
| explore.md 目标「commands/change」与既有 `commands/changes`（复数）的命名歧义 | 若新建单数 `commands/change` 组会与既有复数组并存，同域两组荒谬 | — | 已裁决并入既有 `commands/changes`（见决策表），proposal 显式留痕 |
| spec 基线同步遗漏（desktop-app-shell 三处 requirement + Module Contract 行） | 基线叙事与代码错位，后续落位论证引用失真 spec | 低 | specs/ delta 已逐条列出；归档 sync 步骤按 delta 意图合并（Module Contract 行随归档同步，proposal 留痕） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 目标模块 | 并入既有 `commands/changes/`（复数） | explore.md「commands/change」中文不标复数；change 域命令组已存在且模块 doc 已自认 change 域职责；三读命令与 `create_change` 同域合组即完成「读 + 记录面」收口 | 新建单数 `commands/change` 组（与既有复数组并存，同域两组，否决）；并入 `change_flow`（该组语义为 run 编排控制，混入查询 / 建档语义，desktop-change-create 决策已否决过同类） |
| queries 轨道处置 | 消解（目录删除，不留空壳） | 三命令全部是 change 域读面，搬空后轨道无成员；exec 轨道纪律已确立「MUST NOT 空壳」同款先例；保留空模块只会延续失真叙事 | 保留空 queries 组等未来通用查询（投机性预留，违「无投机特性」纪律，否决） |
| 命令名与签名 | 逐字平移，零改动 | IPC 兼容优先：命令名是前端 invoke 与生成 bindings 的键，改名即破坏前端；搬迁的价值在组织叙事，不在重命名 | 顺手重命名（如 `list_changes` → `change_list`）（无收益纯风险，否决） |
| 版本 | 不 bump | 纯内部重构，无用户可见变化（沿「仅用户可见变更才提升」既有裁决） | bump 0.4.3（无可见变化不支持，否决；0.4.2 已被 agent-turn-eof 缺陷修复消费） |

### 待决问题

- 无。范围闭合：代码搬迁 + 测试合并 + doc 叙事 + spec 基线增量，无开放设计点（design 相位预计仅定测试合并的装置统一细节）。

---
