# desktop-crate-layout Specification

## ADDED Requirements

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

Rust 侧 SHALL 按 cargo workspace 组织为三层分组：

```
crates/
├── core/                      # 引擎侧命名空间（纯目录，不是 crate）
│   ├── foundation/            # crate: layout 解析（刻意极小）
│   └── workflow/              # crate: change 域（model / parse / queries / artifacts）
└── desktop-app/               # crate: Tauri 壳
```

依赖规则 SHALL 为：`desktop-app → workflow → foundation`。`foundation` MUST NOT 依赖任何其他 crate；`workflow` MUST NOT 依赖 `desktop-app`。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖。

`core/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `workflow` / `archi` 裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查三个 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow` 与 `foundation`，`workflow` 依赖 `foundation`
- **AND** `foundation` 无 workspace 内依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力，不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具

### Requirement: 代码命名隔离 openspec 字样

`packages/desktop` 内的 crate 名、模块名、类型名、函数名与 DTO 字段 SHALL NOT 含 `openspec` 字样（该名称未来要改，代码概念上此目录树是 change 域而非 openspec 域）。磁盘真实路径（当前为 `openspec/changes/`）SHALL 全部收进 foundation 的 layout 解析器，除 `resolve` 一处外，desktop 源码 MUST NOT 出现硬编码的该路径字符串。

#### Scenario: 命名扫描通过

- **WHEN** 对 `packages/desktop` 源码做标识符扫描（crate 名、模块名、类型名）
- **THEN** 无标识符含 `openspec` 子串

#### Scenario: 路径硬编码唯一触点

- **WHEN** 在 `packages/desktop` 源码中搜索当前磁盘目录名字符串
- **THEN** 仅 foundation 的 `resolve` 实现处出现，model / parse / queries / 前端均经 `Layout` 结构取路径

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop`（包） | 独立桌面应用 | 自带 lockfile / 构建脚本；不进根 workspace、不进 `dist/` |
| `crates/core/foundation` | 共享地基（今天仅 layout） | 无 workspace 内依赖；第二消费者出现才下沉新能力 |
| `crates/core/workflow` | change 域纯读库 | 依赖 foundation；零 Tauri 依赖；无指令概念 |
| `crates/desktop-app` | Tauri 壳 | 依赖 workflow + foundation；command 薄包装 |
