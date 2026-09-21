# Desktop 终端探索笔记（packages/desktop）

> 三轮探索收敛记录。拟议 change 名候选：`add-desktop-terminal`。

## 目标

Tauri 2 桌面应用，读取任意 workspace 下的 changes 记录，展示任务进程与中间产物。
源代码位于 `packages/desktop`。

**战略定位**：未来将转型多 agent 架构、抛弃 plugin，写入最终也用 Rust —— 本应用是
未来 Rust 运行时的第一块砖，不只是看板。

## 数据面考古结论

`openspec/changes/`（含 archive，74 个历史 change）存在三代结构：

| 代际 | 特征 | 判定 |
|---|---|---|
| v2（现在） | workflow.json 含 `file_log[]`、`active_phase`、`interrupted[]` | 有 workflow.json 且有 file_log |
| v1（中期） | workflow.json 只有 `eval[]` + 旧 `files{}` 桶；有 `phases/` 快照、`.openspec.yaml` | 有 workflow.json 无 file_log |
| v0（早期） | 仅 markdown 产物（proposal/design/tasks/specs） | 无 workflow.json |

关键数据结构（v2 workflow.json）：

- `eval[]`：每 phase 尝试史 —— `phase / attempt / verdict / report(中文评审报告) /
  checklist[{item, pass, evidence}] / timestamp`。含 fail→retry、backtrack 轨迹。
- `active_phase`：正在运行的 phase（phase_start 写入，phase_log 清场置 null）。
- `file_log[]`：日志式文件清单，每条 `{op: write|delete|revert, scope, attempt, path, at}`。
- `reports/`：`results.json`、`coverage-summary.json`、`mutation.json/html`。

Phase 流水线（9 站）：
proposal → dev-design → test-design → implement → test-gen → test-execution →
code-review → acceptance（每站可 fail→retry，可 backtrack 回跳）。

其他事实：

- V0 archive 目录名自带日期前缀（`2026-05-18-xxx`），无 workflow.json 也能排时间线。
- schema 真理源目前在 `plugins/dev-team/bin/src/schemas/workflow.schema.ts`（zod）。
- 74 个 archive 横跨三代结构 = 现成的离线解析测试语料（快照回归）。
- 仓库根目录无 package.json / pnpm-workspace.yaml；`plugins/dev-team` 自成 workspace。

## 已定决策

| # | 岔路 | 决策 | 理由 |
|---|---|---|---|
| 1 | 解析层 | **Rust serde 解析**（不复用 TS zod） | 未来写入也在 Rust；桌面端是第二实现，schema 漂移靠宽松解析 + 语料回归兜底 |
| 2 | 实时性 | **不做 watch，刷新按钮** | 砍掉 notify/事件/去抖；取数收在 hook 后，未来可换推送 |
| 3 | 老数据 | **三代降级展示**（v2 完整 / v1 部分 / v0 仅文档） | Inventory enum 收编代际差异 |
| 4 | 落位 | **完全独立包** | `packages/desktop` 自己的 lockfile，不引入根 workspace，不进 dist/ 分发链 |
| 5 | 前端栈 | **React + TS** | markdown 密集型查看器，生态零件最全 |
| 6 | workspace 语义 | **通用** | 用户挑任意项目根目录，App 读其 `openspec/changes/`；附最近打开列表 |
| 7 | 命名 | **代码中不出现 openspec** | 该名称未来要改；含磁盘路径隔离（见下） |
| 8 | 执行能力 | **预留指令抽象** | 今天只读；写入/驱动 agent 走预留缝 |

## 命名与路径隔离（决策 #7 的两层含义）

1. **代码命名**：crate 名、模块名、类型名不含 `openspec`。core crate 候选：
   `workflow-core`（倾向，领域概念最稳）/ `changes-core` / `engine-core`。
   留待 proposal 阶段定。
2. **磁盘路径**：`openspec/` 目录本身未来也要改名。扫描层不写死路径，收进
   layout 解析器：`core::layout::resolve(root) → Layout { changes_root, archive_root,
   explores_root }`。目录改名只动这一个函数。

## 指令抽象的预留形态（决策 #8）

预留的是缝，不是空壳 trait（与 CLAUDE.md "no speculative" 的张力由用户显式决策覆盖）：

```
Tauri command 层双轨:
├── commands/queries/   list_changes / get_change_detail / read_artifact  ← MVP 实现
└── commands/exec/      exec_*                                           ← 预留空轨道

纪律:
  - 所有 workspace 状态只经 core 函数访问
  - Tauri command 是无状态薄包装（参数 → core → DTO）
  - DTO 区分 Query Result / Command Result
  - trait 定形等第一条真实命令落地（届时才知道同步/异步、进度上报形态）
```

## 架构

```
packages/desktop (独立包)
├── package.json / src/            # React + TS 前端
│   └── hooks/ useChangeList / useChangeDetail   # 刷新按钮触发；未来可换推送
└── src-tauri/crates/
    ├── workflow-core/             # 纯 Rust 库，零 Tauri 依赖（未来引擎种子）
    │   ├── layout/                # 路径解析（openspec/ 改名唯一触点）
    │   ├── model/                 # Change / Workflow / Inventory(v2|v1|v0)
    │   ├── parse/                 # serde 宽松解析 + 代际探测
    │   └── queries/               # 扫描/聚合（流水线视图、耗时、清单折叠）
    └── desktop-app/               # Tauri 壳
        ├── commands/queries/      # 已实现
        └── commands/exec/         # 预留
```

数据流（无 watch）：

```
[🔄 刷新] → invoke("list_changes") → core 扫描 → serde DTO → 前端渲染
```

## MVP 切分

第一刀（MVP）：

- workspace 选择（文件夹 + 最近列表）
- change 列表（active + archive 按代际标注、按月分组）
- phase 流水线（attempt / verdict）
- eval checklist 展开（item / pass / evidence）
- markdown 产物渲染（proposal / design / tasks / specs / explore）
- tasks.md 勾选进度（数 `- [ ]` / `- [x]`，成本近零体感收益大）
- 刷新按钮

第二刀：

- reports 渲染（覆盖率 / 变异分数）
- file_log 时间线视图
- 阶段耗时统计（active_phase.start_at → eval timestamp）
- mutation.html 内嵌
- 多 workspace 常驻
- （远）操作能力：exec 轨道接入

## 测试策略

- fixture 语料：从 74 个 archive 挑三代代表性样本进 fixtures。
- serde 宽松：未知字段忽略、单字段损坏降级不炸整条记录。
- 快照回归：全量解析 → golden 对比，防 TS 侧 schema 演进导致 Rust 解析静默劣化。

## 与项目规则的边界

- CLAUDE.md「改插件源码后 bump 版本 + rebuild 产物」针对 `plugins/`；desktop 是独立
  app，不进 dist/ 分发链，不受该规则约束 —— proposal 中需显式声明，避免 evaluator 误判。

## 第四轮追加：crate 分层（2026-09-20）

背景：未来 desktop 端还要做架构守护等功能。现有插件 archi 工具组
（`archi_query/validate/write/check`、`archi_decide`）读 `openspec/architecture/decisions/`
（ADR）与 `models/`（C4 DSL）—— 与 workflow 域共享同一个要改名的 `openspec/` 根。
layout 解析器因此是共享地基，放任何单域里都会在第二个域出现时搬家。

**决策 #9**：crate 组织采用三层分组（**修订**上文架构图中 `workflow-core` 单 crate 的画法）：

```
crates/
├── core/                      # 引擎侧命名空间（纯目录，不是 crate）
│   ├── foundation/            # crate: layout 解析（今天只装这个，刻意极小）
│   ├── workflow/              # crate: change 域（model / parse / queries）
│   └── archi/                 # crate（未来落地时才创建，不预留空壳）
└── desktop-app/               # crate: Tauri 壳

依赖规则：desktop-app → workflow → foundation；desktop-app → archi → foundation（未来）
foundation 不依赖任何人；workflow ↔ archi 互不依赖。
```

理由：未来多 agent 运行时按域整块抽取，crate 缝即抽取缝；编译器依赖图强制边界，
优于 mod 可见性。弃选：单 crate + 模块（边界弱、抽取要补刀）；扁平多 crate（无分组，
信息量少一格）。

三条注意：

1. crate 名不得为 `core`（与 Rust 内置 core 撞名）；`core/` 仅作目录名，内部 crate 用
   `foundation` / `workflow` / `archi` 裸名（workspace 内唯一即可）。
2. foundation 今天只装 layout；fs 助手 / 错误类型等第二个消费者出现再下沉，不预铺。
3. queries/exec 双轨随之变形：各域自暴露 queries，desktop-app 负责组合；exec 预留轨道
   在 app 层（未来 agent 驱动是 app 层的事），域 crate 保持纯读、不出现指令概念。

分组目录命名：`core/` 优于 `engine/`（中性稳妥；目录改名远比 crate 改名便宜）。

## 第五轮追加：中间产物插件化（2026-09-20）

背景：中间产物是第一变化轴（v1 `phases/` 快照消失、`files{}` → `file_log[]`、reports 格式
演进；多 agent 未来只会更快）。流水线结构反而稳定。因此把"产物类型"隔离为插件，
前后端各一个注册表，中间靠稳定信封契约缝合。

**决策 #10**：中间产物采用"抽象 + 插件"组织：

```
Rust 侧（发现 + 解析）             React 侧（渲染）
matcher 链（注册表）                renderer 注册表
  ├─ 命中 → parser → Envelope ───▶  "kind" → 专属组件
  └─ 未命中 → "raw" 兜底             未注册 → Fallback 组件
```

信封（两侧唯一共享知识）：

```
ArtifactEnvelope {
  kind:    str        ← 契约 ID（如 "eval-checklist"），不含 openspec 字样
  version: u32        ← 产物格式版本
  title:    str
  payload:  JSON      ← kind 自描述结构化负载
  fallback_text: str? ← 渲染器缺席时的保底文本
}
```

新增产物 = 两侧各加一个自包含模块（Rust: matcher+parser 自注册；
React: 组件+注册），核心（扫描循环/DTO/组件路由）零改动 —— 加法而非改法。

要点：

1. Rust 侧插件 = trait + 编译期静态注册表，**不做 dylib 动态加载**（ABI 生态痛，
   诉求仅为"加产物不动核心"，静态注册即满足；第三方动态加载属遥远需求）。
2. 兜底是硬要求：未注册 kind → Fallback（fallback_text + kind 徽标），与 v0/v1/v2
   降级展示同哲学 —— 永不白屏。
3. 原 Docs 探测逻辑折叠进 matcher：`markdown-doc` 插件自带文件名匹配（按代际），
   发现机制即插件机制，删除一套并行抽象。
4. 放置：抽象先住 `workflow` crate（`artifacts/` 模块），不进 foundation ——
   沿用第四轮纪律"第二个消费者出现再下沉"；archi 域落地时提升至 foundation。
5. MVP 把已知产物全部实现为第一波插件实例：`markdown-doc`、`eval-checklist`、
   `tasks-progress`、`file-log`、`test-report-summary`、`html-report-ref` ——
   抽象从第一天被真实实例校准，满足 CLAUDE.md 无投机抽象守则。
6. 渲染器不追求独立于 Rust 更新（解析留在 Rust，React 只管画）—— 与"未来读写都在
   Rust"战略一致；代价是渲染器与解析器同版本发。（默认采用推荐，proposal 阶段可复议。）

## 第六轮追加：MVP 收敛复核（2026-09-20）

proposal 评审后用户裁决两刀，修订第五轮 #10.5 与第一轮 MVP 列表，**与上文冲突处以本节为准**：

1. **第一波插件 6 → 3**：MVP 只做 `markdown-doc` / `eval-checklist` / `tasks-progress`
   （正对应第一刀全部视图）。`file-log` / `test-report-summary` / `html-report-ref`
   随第二刀对应视图（file_log 时间线 / reports 渲染 / mutation 内嵌）落地——依据正是
   插件化"后加便宜"的设计卖点；3 个真实实例 + Fallback 已满足抽象校准诉求。
2. **最近打开列表延后**：MVP 仅文件夹选择器；recent list（需持久化）第二刀加回。
