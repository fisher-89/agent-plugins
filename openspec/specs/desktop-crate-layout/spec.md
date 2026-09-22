# desktop-crate-layout Specification

## Purpose

约束 `packages/desktop` 桌面端包的落位方式与 Rust crate 分层组织：作为完全独立的包存在于仓库中，cargo workspace 按三层分组组织依赖方向，并在代码命名上隔离 `openspec` 字样、将磁盘路径收口到 foundation 的 layout 解析器。

## Requirements

### Requirement: packages/desktop 完全独立包落位

桌面端 SHALL 落位为 `packages/desktop`，作为完全独立的包存在：自带 lockfile 与构建脚本，MUST NOT 引入仓库根 `package.json` 或 `pnpm-workspace.yaml`，MUST NOT 进入 `dist/` 插件分发链。CLAUDE.md「改插件源码后 bump `plugins/<name>/package.json` version 并 rebuild 产物」规则 SHALL NOT 适用于 `packages/desktop`。

#### Scenario: 独立包不污染主仓

- **WHEN** 检查仓库根目录与 `dist/`
- **THEN** 根目录无新增 `package.json` / `pnpm-workspace.yaml`
- **AND** `dist/` 不含 desktop 产物

#### Scenario: 不受插件 bump 规则约束

- **WHEN** `packages/desktop` 源码变更
- **THEN** 无任何 `plugins/<name>/package.json` 版本 bump 或 rebuild 步骤被要求

### Requirement: crate 三层分组与依赖规则

Rust 侧 SHALL 按 cargo workspace 组织为三层分组（core 引擎侧 / infra 基础设施侧 / desktop-app 壳）：

```
crates/
├── core/                      # 引擎侧命名空间（纯目录，不是 crate）
│   ├── foundation/            # crate: layout 解析（刻意极小）
│   └── workflow/              # crate: change 域（model / parse / queries / artifacts）
├── infra/                     # 基础设施侧命名空间（纯目录，不是 crate）
│   └── store/                 # crate: redb 本地库（db 边界第一成员）
└── desktop-app/               # crate: Tauri 壳
```

依赖规则 SHALL 为：`desktop-app → workflow → foundation`，且 `desktop-app → store`。`foundation` MUST NOT 依赖任何其他 crate；`workflow` MUST NOT 依赖 `desktop-app`；`store` MUST NOT 依赖 core 任何 crate（自含模型），MUST NOT 依赖 Tauri（Tauri 只属于 desktop-app）；core 任何 crate MUST NOT 依赖 `store`（由 crate 图机械保证有状态持久化进不了纯读域）。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖；未来 db 边界新成员（如图数据库）SHALL 落位 `infra/` 下新 crate。

`core/` 与 `infra/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `workflow` / `archi` / `store` 等裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查各 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow`、`foundation` 与 `store`，`workflow` 依赖 `foundation`
- **AND** `foundation` 无 workspace 内依赖；`store` 无 workspace 内 crate 依赖、无 Tauri 依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力，不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具

#### Scenario: store 落位 infra 且 core 依赖不到 store

- **WHEN** 检查 `store` 与 core 两个 crate 的 `Cargo.toml` 及目录落位
- **THEN** `store` 落位 `crates/infra/store`，core 两个 crate 的依赖中无 `store`
- **AND** `store` 不出现 Tauri 依赖

### Requirement: 代码命名隔离 openspec 字样

`packages/desktop` 内的 crate 名、模块名、类型名、函数名与 DTO 字段 SHALL NOT 含 `openspec` 字样（该名称未来要改，代码概念上此目录树是 change 域而非 openspec 域）。磁盘真实路径（当前为 `openspec/changes/`）SHALL 全部收进 foundation 的 layout 解析器，除 `resolve` 一处外，desktop 源码 MUST NOT 出现硬编码的该路径字符串。

#### Scenario: 命名扫描通过

- **WHEN** 对 `packages/desktop` 源码做标识符扫描（crate 名、模块名、类型名）
- **THEN** 无标识符含 `openspec` 子串

#### Scenario: 路径硬编码唯一触点

- **WHEN** 在 `packages/desktop` 源码中搜索当前磁盘目录名字符串
- **THEN** 仅 foundation 的 `resolve` 实现处出现，model / parse / queries / 前端均经 `Layout` 结构取路径

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按四类边界组织，任何新特性落位时 SHALL 先声明所属边界：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app；
- **db 边界**：redb（现有 `crates/infra/store`）及未来数据库成员（落位 `infra/` 下新 crate）；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）。

领域核心（change 域读模型与解释规则）与应用编排（现为 command）位于四类边界之内：core 两 crate 纯依赖，infra 不依赖 core 也不依赖 Tauri，依赖方向由 crate 图机械保证（既有 requirement 不变）。`infra/` 目录表达 db / api 等边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api 四类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate 的依赖声明
- **THEN** 无任何 Tauri 系依赖（tauri、tauri-plugin-* 等），shell 依赖仅存在于 desktop-app

### Requirement: 未来租户归位规则

已规划特性 SHALL 按下表归位，本变更 MUST NOT 据此预建任何目录或 crate：

| 特性 | 边界 | 归位 | 规则 |
|---|---|---|---|
| workspace 写文件 | 文件 | `commands/exec/` 轨道 | trait 形状等第一条真实执行命令落地时定形（既有决策不变）；写用户 repo 属对 repo 的变更，落地时 SHALL 有 write protection / 路径包含性检查 / 审计 |
| workspace 读写 db | db | `infra/store` 的 workspace 维度 | 落盘策略见 desktop-data-dimensions 能力 |
| 图数据库 | db | `infra/graph`（暂不建） | 暂不考虑；关系查询真实痛点出现再启，届时按 db 边界新成员落位 |
| 后端 api | api | `infra/api`（不建） | 先占位留痕：用途未定，MUST NOT 建目录、MUST NOT 动代码 |

`infra/fs` SHALL 在文件边界长出独立适配（区别于 workflow 内纯读逻辑的、可复用的 fs 基础设施）时再立，不预建。

#### Scenario: 无预建目录

- **WHEN** 检查 `crates/infra/` 目录树
- **THEN** 仅 `store` 一个 crate 成员，不存在 `infra/graph` / `infra/api` / `infra/fs` 空目录

#### Scenario: 图数据库与 api 决策留痕

- **WHEN** 查阅本能力 spec
- **THEN** 图数据库"暂不考虑、痛点驱动再启"与 api "先占位留痕"两条裁决可考，后续变更无需重新辩论是否引入

#### Scenario: exec 轨道承接文件写

- **WHEN** 检查 `commands/exec/`
- **THEN** 仍为空轨道（无实现、无空壳 trait）；workspace 写文件将落此轨道而非 queries / workspaces 轨道

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop`（包） | 独立桌面应用 | 自带 lockfile / 构建脚本；不进根 workspace、不进 `dist/` |
| `crates/core/foundation` | 共享地基（今天仅 layout） | 无 workspace 内依赖；第二消费者出现才下沉新能力 |
| `crates/core/workflow` | change 域纯读库 | 依赖 foundation；零 Tauri 依赖；无指令概念；不依赖 store |
| `crates/infra/store` | redb 本地库（db 边界第一成员） | 自含模型不依赖 core；零 Tauri 依赖；db 路径由 desktop-app 注入 |
| `crates/desktop-app` | Tauri 壳 | 依赖 workflow + foundation + store；command 薄包装 |
| 四类边界分类学 | 落位先声明边界 | shell（仅 desktop-app）/ db（infra 成员）/ 文件（workspace 读写）/ api（占位留痕）；core 与 infra 禁 Tauri |
| 未来租户归位表 | 特性 → 归位映射 | 写文件 → exec 轨道；读写 db → store workspace 维度；图数据库 → 暂不考虑；api → 不建目录 |
| `crates/infra/` | 目录表达计划 | 仅 store 一个成员；graph / api / fs 均不预建，成员出现才立 |
