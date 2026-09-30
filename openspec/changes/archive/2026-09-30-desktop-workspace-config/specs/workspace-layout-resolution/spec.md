# workspace-layout-resolution Specification

## MODIFIED Requirements

### Requirement: resolve 函数返回 Layout 结构

foundation crate SHALL 提供纯函数 `resolve(root: &Path) -> Layout`，将用户选定的 workspace 根目录解析为布局结构：

```
Layout {
  changes_root:  PathBuf   # 进行中 change 目录树
  archive_root:  PathBuf   # 已归档 change 目录树
  explores_root: PathBuf   # 探索笔记目录树
}
```

layout.rs SHALL 在模块顶部设**目录名常量组**：域目录名（`DOMAIN_DIR_NAME`）、`changes` / `archive` / `explores` 子目录名、`config.json` 文件名。`resolve` SHALL 全量引用常量组装路径（MUST NOT 行内字面量）；`domain_dir_name` 的 fn 形态 SHALL 收敛为常量（公开形态 design 定夺：导出 `pub const` 或保留 fn 包装），既有消费点（stats 轨道的 tokei `ignored_directories` 通道）随之更新、语义零变化。

`resolve` 与常量组 SHALL 是整个 desktop 包内唯一知晓磁盘目录名、子目录名与 config.json 文件名的位置（字面量隔离执法见 desktop-crate-layout：`openspec` 与 `config.json` 双禁令）；目录或子目录未来改名 SHALL 只需修改 layout.rs 常量组。`resolve` MUST NOT 访问文件系统（纯路径推导），对不存在的 root 亦 SHALL 正常返回 Layout（存在性检查交由上层查询处理）。

#### Scenario: 从任意根解析布局

- **WHEN** 以某个包含 change 数据的项目根目录调用 `resolve(root)`
- **THEN** 返回的 `Layout` 三字段分别指向该根下进行中、归档、探索笔记三个目录树

#### Scenario: 改名只动常量组

- **WHEN** 假设磁盘目录 `openspec/` 或任一子目录 / config.json 文件名更名
- **THEN** 仅需修改 layout.rs 常量组，model / parse / queries / config / desktop-app 与前端零改动

#### Scenario: resolve 全量引用常量

- **WHEN** 审查 `resolve` 实现与 `layout_test` 断言
- **THEN** 路径组装全部经常量引用，无常量组之外的行内目录名 / 文件名字面量；常量组覆盖域目录名、三个子目录名与 config.json 文件名

#### Scenario: 对不存在的根仍可解析

- **WHEN** 以一个不存在的路径调用 `resolve`
- **THEN** 不报错，正常返回 Layout 结构；目录缺失由查询层按空结果处理

### Requirement: workspace 语义通用化

App SHALL 支持读取任意用户选定的项目根目录下的 change 记录，MUST NOT 将路径绑定到本仓库。

#### Scenario: 切换 workspace 根

- **WHEN** 用户通过文件夹选择器选定另一个项目根目录
- **THEN** 列表与详情取数均以新根为基准重新扫描

## ADDED Requirements

### Requirement: config_path 提供 config.json 路径解析

foundation crate SHALL 提供 config.json 路径解析能力：给定 workspace 根返回 `<root>/<域目录名>/config.json` 路径（倾向形态为独立函数 `config_path(root: &Path) -> PathBuf`——config.json 是文件而非目录树，`Layout` 加第四字段不贴切；最终形态 design 定夺并记录理由）。路径 SHALL 全量引用目录名常量组，MUST NOT 行内字面量；解析 SHALL 为纯路径推导、无任何文件系统访问（文件存在性交由 `config` crate 读取层处理）；该函数 SHALL 是 `config` crate 获取配置文件路径的唯一通道。

#### Scenario: 路径解析正确

- **WHEN** 以某 workspace 根调用 config.json 路径解析
- **THEN** 返回路径为根目录拼接域目录名常量再拼接 config.json 文件名常量的结果，与 `resolve` 的域目录推导同源

#### Scenario: 纯推导无 IO

- **WHEN** 以不存在的 root 调用 config.json 路径解析
- **THEN** 正常返回拼接路径，无文件系统访问、不报错

#### Scenario: 唯一通道

- **WHEN** 审查 `config` crate 的取路径方式
- **THEN** config.json 路径仅来自 foundation 的该解析能力，crate 内无第二处路径拼接、无文件名字面量

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `foundation::layout` | 磁盘布局解析 + 目录名常量组 | `resolve(root: &Path) -> Layout`；常量组覆盖域目录名 / `changes` / `archive` / `explores` / `config.json`；`config_path`（形态 design 定夺）同源引用常量；纯路径推导、无 IO；全包唯一字面量触点（`openspec` + `config.json` 双禁令） |
| `domain_dir_name`（fn → 常量形态） | 域目录名裸名消费通道 | tokei `ignored_directories` 等裸名语义消费方经此取字面量；公开形态（`pub const` vs fn 包装）design 定夺，消费点语义零变化 |
