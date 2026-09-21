# workspace-layout-resolution Specification

## ADDED Requirements

### Requirement: resolve 函数返回 Layout 结构

foundation crate SHALL 提供纯函数 `resolve(root: &Path) -> Layout`，将用户选定的 workspace 根目录解析为布局结构：

```
Layout {
  changes_root:  PathBuf   # 进行中 change 目录树
  archive_root:  PathBuf   # 已归档 change 目录树
  explores_root: PathBuf   # 探索笔记目录树
}
```

`resolve` SHALL 是整个 desktop 包内唯一知晓当前磁盘目录名（`openspec/`）的位置；目录未来改名 SHALL 只需修改此函数。`resolve` MUST NOT 访问文件系统（纯路径推导），对不存在的 root 亦 SHALL 正常返回 Layout（存在性检查交由上层查询处理）。

#### Scenario: 从任意根解析布局

- **WHEN** 以某个包含 change 数据的项目根目录调用 `resolve(root)`
- **THEN** 返回的 `Layout` 三字段分别指向该根下进行中、归档、探索笔记三个目录树

#### Scenario: 改名只动一处

- **WHEN** 假设磁盘目录 `openspec/` 更名为其他名称
- **THEN** 仅需修改 `resolve` 的实现，model / parse / queries / desktop-app 与前端零改动

#### Scenario: 对不存在的根仍可解析

- **WHEN** 以一个不存在的路径调用 `resolve`
- **THEN** 不报错，正常返回 Layout 结构；目录缺失由查询层按空结果处理

### Requirement: workspace 语义通用化

App SHALL 支持读取任意用户选定的项目根目录下的 change 记录，MUST NOT 将路径绑定到本仓库。

#### Scenario: 切换 workspace 根

- **WHEN** 用户通过文件夹选择器选定另一个项目根目录
- **THEN** 列表与详情取数均以新根为基准重新扫描

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `foundation::layout` | 磁盘布局解析 | `resolve(root: &Path) -> Layout`；纯路径推导、无 IO；目录名唯一触点 |
| `desktop-app`（workspace 状态） | 当前 workspace 持有 | 持有选定的根目录 |
