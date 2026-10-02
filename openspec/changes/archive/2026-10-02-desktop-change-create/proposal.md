# 提案: desktop-change-create

> **变更**: desktop-change-create
> **日期**: 2026-10-02
> **状态**: 草案

---

## 问题

desktop 桌面端目前只能**浏览**既有 change（清单 / 详情 / 产物渲染 / change-flow run 编排），无法**创建** change：`openspec/changes/` 下的新变更只能经插件 skill 路径（CLI 会话内由 agent 调 `change_create` MCP 工具）产生。桌面端创建入口缺失导致：

1. 用户必须在 CLI 会话里才能开一个变更，桌面端 change-flow 工作流没有独立入口；
2. 变更的**最初目标**（goal / 需求描述）在 CLI 流程里是随口话入对话的，桌面发起 run 时没有持久的目标载体——`core/workflow` 写面相位表（`phase_table.rs`）的 proposal 相位 executor prompt 只携带 `<change>` 占位与 explore 交接行，无 goal 通道，walker 组装 prompt 时 goal 无处安放（`orchestration/prompt.rs` 仅拼角色要点 + 相位 prompt + git diff 段）；
3. `core/workflow` 写面 `load_doc`（`persist.rs`）现状注释明确「`change_create` 是唯一创建者，写面从不创建文件」——Rust 侧无创建路径，桌面被硬性挡在门外。

---

## 提案

桌面端补齐「新建变更」能力，作为 change-flow 工作流的第一入口。用户在变更清单页打开新建对话框，输入 **change 名称（kebab-case）** 与 **最初目标（goal，必填非空白）**，一次提交完成三件事：

1. **建域**：`core/workflow` 写面新增 `create` 操作（进程内、sync、零 Tauri），创建 `changes_root/<name>/` 目录并写出 `workflow.json`——形状与插件 `createChange`（`plugins/dev-team/bin/src/commands/change-create.ts`）逐字段一致：`{ workflow_type: "requirement", created: <UTC 日期 YYYY-MM-DD>, file_log: [] }`（无 `eval` 键、2 空格 pretty + 尾换行），使新 change 立即被既有读面识别为 v2 且可直达 `change_flow_start` 前置校验；
2. **落 goal**：goal 原文写入 `changes_root/<name>/explore.md`（UTF-8、free-form、不加结构假设）。这是 goal 进工作流的既有通道：proposal 相位 executor prompt 内建的交接行已指引 proposal-planner「若 explore.md 存在则 Read 为 free-form explore context」，goal 无需改任何 prompt 模板、无需给 workflow.json 加任何字段即抵达提案阶段；且 explore.md 经 markdown-doc 产物插件收录，在详情页即见「探索」条目；
3. **入列**：IPC 命令 `create_change`（薄包装三件事纪律）→ 前端对话框成功回调 `refresh` 清单并导航 `/changes/<name>`，用户在详情 / flow 视图显式发起 run（不自动发起）。

关键裁决：goal 载体取 explore.md 而非 workflow.json 新字段（写面「schema 零新字段」纪律）或 run 发起参数（重启丢失、prompt 无通道）；创建主体取 Rust 写面而非 CLI 子进程（desktop-change-orchestration 零 CLI 写通道红线）。插件侧零改动。

---

## 能力

### 新增能力

- **desktop-change-create** — 桌面端新建变更：写面 `create` 操作（建目录 + workflow.json + explore.md 落 goal）、`create_change` IPC 命令、清单页新建对话框、名称/goal 校验面、V1 边界留痕。

### 修改的能力

- 无。既有能力仅被引用沿用、语义不变：desktop-workflow-write-face（sync / 零 Tauri / serde schema 权威纪律）、desktop-change-queries（新建后经既有 list/detail 可见）、desktop-page-routing（`/changes/:name` 导航）、desktop-artifact-plugins（explore.md 既有收录）、desktop-ipc-type-bindings（新命令经既有 specta 管线出线）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`（新）— `create` 操作：kebab-case / 长度 / goal 非空白校验、已存在拒绝、`create_dir_all` 建树、workflow.json + explore.md 写出
- `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` — 导出 `create`（签名与 DTO 形状 design 定稿）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs` — 「唯一创建者」注释更新为「插件 MCP 与桌面 create 并存」声明
- `packages/desktop/src-tauri/src/commands/changes/mod.rs`（新组）— `create_change` 命令：blank root 检查、`Result<T, String>` 错误映射、`#[specta::specta]`
- `packages/desktop/src-tauri/src/commands/mod.rs` — `all_commands!` 登记
- `packages/desktop/src/views/changes/components/change-create-dialog.tsx`（新）— 新建对话框（沿 `explore-create-dialog` 先例：toggle 展开、名称 / goal 输入、行内错误、本地 kebab-case 校验）
- `packages/desktop/src/views/changes/change-list-view.tsx` — 挂载新建入口 + `onCreated` 回调（refresh + navigate）
- `packages/desktop/src/types/generated/bindings.ts` — 重导出生成物（`create_change` + 返回 DTO）
- `packages/desktop/package.json` — `version` 0.4.0 → 0.4.1

### 测试文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs`（新）— 创建形状 / 各拒绝面 / 保形与兼容断言
- `packages/desktop/src-tauri/src/commands/changes/mod_test.rs`（新）— 命令薄包装与 blank root / 错误映射
- `packages/desktop/src/views/changes/components/change-create-dialog.test.tsx`（新）— 对话框交互 / 校验拦截 / 成功流转
- `packages/desktop/src/views/changes/change-list-view.test.tsx` — 增补新建入口挂载与 onCreated 流转断言

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team/**` — 插件面冻结：`change-create.ts`、MCP 工具面、版本 2.10.44、三类交付产物均不动
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — 相位表与 prompt 模板（含 proposal explore 交接行）不动
- `packages/desktop/src-tauri/crates/core/workflow/src/model/**` — workflow.json 领域模型零新字段
- 既有写面四操作（`phase_next` / `phase_start` / `phase_log` / `backtrack`）与编排 / 查询面语义
- `openspec/specs/**` 既有基线 spec

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 写面 `create` 操作 | 对空白树 workspace 以合法名称 + goal 调用后：`changes_root/<name>/workflow.json` 存在且内容恰为 `{"workflow_type":"requirement","created":"<UTC YYYY-MM-DD>","file_log":[]}`（2 空格 pretty + 尾换行、无 `eval` 键）；`changes_root/<name>/explore.md` 存在且内容为 goal 原文（UTF-8） |
| AC-2 | 新建 change 可发现可发起 | 创建后既有 `list_changes` 以 v2 代际列出该 change；`get_change_detail` 可达；`change_flow_start` 前置校验（存在 + 可解析 + requirement 相位表在位）通过；创建产物可被插件 zod schema 形状解析（key 序 / 字段集一致） |
| AC-3 | 校验拒绝面 | 非法 kebab-case（大写 / 下划线 / 空格）、超 128 字符、goal 空白、已存在同名 active change——各返回显式 `Err`，目标目录与文件零产生、既有 change 零改动 |
| AC-4 | IPC 命令面 | `create_change` 经 `all_commands!` 注册、specta 出线、bindings 重导出后 `git diff` 干净；blank root 显式 `Err`；返回 DTO 仅含 `name` 与 `created`（磁盘路径知识不下沉前端） |
| AC-5 | goal 进入工作流 | 相位表 proposal 交接行（既有文本）指引读取 `changes/<change>/explore.md`；新建 change 的 explore.md 即 goal 原文；详情页产物清单含「探索」（explore.md）条目；相位表与 workflow.json schema 零改动 |
| AC-6 | 清单页新建入口 | 清单页有新建 toggle；展开后名称 / goal 均必填，名称本地 kebab-case 校验不合法时禁提交且不发起 invoke；成功后清单刷新并导航 `/changes/<name>`；后端错误行内呈现；挂钩均为 data-testid |
| AC-7 | 版本交付 | `packages/desktop/package.json` version 为 0.4.1；`plugins/dev-team` 无任何改动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 双创建者并存（插件 MCP `change_create` 与桌面写面 `create`）形状漂移 | 一侧产物不被另一侧消费面识别（zod / serde） | 中 | 桌面 create 产出与插件 `createChange` 逐字段对照（key 序、尾换行、`file_log: []`、无 `eval`），create_test 以插件输出形态 fixture 锚定；schema 权威已由 desktop-workflow-write-face 声明移交 serde |
| proposal-planner 忽略 explore.md 导致 goal 未进提案 | 提案阶段缺目标输入，run 白跑首相位 | 低 | 交接行是相位表既有文本（不新增不改动），语义为 MUST 级指引；AC-5 静态核对通道在位；风险留痕备后续补偿（如详情页 goal 显著化） |
| 与 workspace 级「写文件」租户边界混淆（desktop-crate-layout 归位表：workspace 写文件 → exec 轨道 + write protection） | 写通道被泛化，绕过边界纪律 | 低 | 写触点收口为 change 域目录内两文件、经 layout 取路径、名称穿越校验；spec 边界条款明示 MUST NOT 泛化为任意 workspace 写通道 |
| 归档树存在同名 change 造成清单歧义 | 用户混淆新旧变更 | 低 | V1 仅拒 active 冲突（与插件 parity，决策留痕）；清单以 v2 徽标 + 创建时间区分 |
| 创建后 goal 无桌面编辑入口（详情只读渲染） | goal 写错只能改盘上文件 | 中 | V1 接受（边界留痕），goal 修正可走既有 explore.md 手改 / 后续变更补编辑能力 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| goal 持久化载体 | 写入 `changes/<name>/explore.md` | proposal 相位交接行既有 MUST 指引读取该文件，goal 零成本抵达提案阶段；重启安全、详情可见（markdown-doc 插件）；不动相位表、不动 schema | workflow.json 加 `goal` 字段（违反写面零新字段纪律，且 prompt 无占位符）；run 发起参数透传（重启丢失、无 prompt 通道） |
| 创建主体 | `core/workflow` 写面新增 `create` 操作 | 零 CLI 子进程写通道红线；写面即 workflow.json 域 Rust 单权威（读 + 写），create 是其自然补全；sync / 零 Tauri 沿写面纪律 | 前端拼路径写文件（违命名隔离 + 单权威）；CLI 子进程调 MCP 工具（红线排除） |
| `workflow_type` 取值 | 恒写 `"requirement"`，不入参 | V1 相位表仅支持 requirement（`phase_table` 其余返回 None，`change_flow_start` 显式拒绝同口径）；暴露枚举入参只会制造必然失败的选择 | 入参枚举透传（4 值）——留给后续 bug-fix / test-only 相位表变更一起开 |
| 名称来源 | 用户输入 kebab-case，本地 + 写面双端校验 | 沿 explore-create-dialog「主题名（kebab-case）」先例；同步、零成本、可预期 | agent 由 goal 派生名称（需先起会话、异步、成本高，留后续增强） |
| 命令落位 | 新组 `commands/changes/` | change 域记录面写命令与 `queries`（纯读）/ `change_flow`（run 控制面）职责分离；沿 explores 组「读 + 记录面」先例；单命令组有 stats / config 先例 | 并入 `change_flow` 组（该组语义为 run 编排控制，混入建档语义） |
| 创建后是否自动发起 run | 不自动发起 | 新建是入口不是开关；run 消耗 agent 会话与预算，应由用户在详情 / flow 视图显式触发 | 创建即发起（隐式消耗预算，误触成本高） |
| 名称冲突判定 | 仅拒 active 同名（插件 parity） | 与 `createChange` 行为一一对应，避免双实现行为漂移 | 同时拒归档同名（更严格但偏离插件基准） |

### 待决问题

- goal 编辑能力（详情页对 explore.md 的编辑器）是否立项独立变更——V1 留痕不阻塞；
- explore.md 初始内容是否需要轻量包装（如一行标题前缀）——当前决策为 goal 原文直写，design 可复核措辞但不得引入结构假设。

---
