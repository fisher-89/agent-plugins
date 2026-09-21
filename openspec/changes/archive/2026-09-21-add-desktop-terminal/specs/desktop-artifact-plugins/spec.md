# desktop-artifact-plugins Specification

## ADDED Requirements

### Requirement: ArtifactEnvelope 稳定信封契约

中间产物 SHALL 以 `ArtifactEnvelope` 信封在 Rust 解析侧与 React 渲染侧之间传递，信封是两侧唯一共享知识：

```
ArtifactEnvelope {
  kind:          String   # 契约 ID（如 "eval-checklist"），不含 openspec 字样
  version:       u32      # 产物格式版本
  title:         String
  payload:       JSON     # kind 自描述结构化负载
  fallback_text: Option<String>  # 渲染器缺席时的保底文本
}
```

解析 SHALL 留在 Rust 侧（React 只负责渲染）；renderer SHALL 与解析器同版本发布，MUST NOT 期望独立于 Rust 侧演进。

#### Scenario: 产物以信封传递

- **WHEN** 前端请求任一中间产物
- **THEN** 收到完整 `ArtifactEnvelope`（kind / version / title / payload / fallback_text），且 kind 命名不含 `openspec` 字样

#### Scenario: 解析职责边界

- **WHEN** 新增一类产物的展示
- **THEN** 数据结构化解析发生在 Rust matcher/parser，React 组件仅消费 payload 渲染

### Requirement: Rust 侧 matcher/parser 静态注册表

workflow crate 的 `artifacts/` 模块 SHALL 提供 matcher + parser 的 trait 与编译期静态注册表，MUST NOT 使用 dylib 动态加载。每类产物为一个自包含模块：matcher 判定命中、parser 产出 `ArtifactEnvelope`，并自注册进注册表；扫描循环 / DTO / 组件路由对产物类型 MUST NOT 硬编码分支。原 Docs（markdown 产物）探测逻辑 SHALL 折叠进 `markdown-doc` 插件的 matcher（按代际做文件名匹配），MUST NOT 保留并行的独立探测机制。

#### Scenario: 加产物不改核心

- **WHEN** 新增一类产物类型
- **THEN** 仅新增一个 Rust 模块（matcher + parser 自注册）与一个 React 模块（组件 + 注册），扫描循环 / DTO / 路由代码零改动

#### Scenario: Docs 探测即插件

- **WHEN** 扫描发现 proposal / design / tasks / specs / explore 等 markdown 文档
- **THEN** 由 `markdown-doc` 插件的 matcher 命中并产出信封，不存在注册表之外的文档探测路径

### Requirement: 第一波三个插件实例

MVP SHALL 将服务第一刀视图的产物实现为第一波插件实例，使抽象从第一天被真实实例校准：

| kind | 内容 |
|------|------|
| `markdown-doc` | markdown 产物文档（proposal / design / tasks / specs / explore，按代际匹配） |
| `eval-checklist` | eval 条目的 checklist 展开（item / pass / evidence） |
| `tasks-progress` | tasks.md 勾选进度（统计 `- [ ]` / `- [x]`） |

每个 kind SHALL 同时具备 Rust 侧 matcher + parser 与 React 侧 renderer 注册。`file-log`、`test-report-summary`、`html-report-ref` 不在 MVP 范围，SHALL 随第二刀对应视图（file_log 时间线 / reports 渲染 / mutation 内嵌）以插件加法方式落地。

#### Scenario: 三个 kind 全部可产出

- **WHEN** 对含完整 v2 数据的 fixture change 发起产物发现
- **THEN** 三个 kind 均可被 matcher 命中并解析出信封（就各自存在的源文件）

#### Scenario: tasks 进度统计

- **WHEN** tasks.md 含若干 `- [ ]` 与 `- [x]` 条目
- **THEN** `tasks-progress` 的 payload 含未完成与已完成计数，renderer 呈现进度

### Requirement: 未注册 kind 的 Fallback 硬要求

前端 SHALL 维护 renderer 注册表并按信封 kind 路由；未注册的 kind SHALL 路由到 Fallback 组件，以 `fallback_text` 渲染并附 kind 徽标。任何产物在任何代际数据下 SHALL 永不白屏——这与 v0 / v1 / v2 降级展示同哲学。

#### Scenario: 未注册 kind 走 Fallback

- **WHEN** 收到 kind 无对应 renderer 的信封
- **THEN** Fallback 组件以 fallback_text 渲染并显示 kind 徽标，不报错、不白屏

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `workflow::artifacts` | 信封 + trait + 静态注册表 | `ArtifactEnvelope { kind, version, title, payload, fallback_text }`；matcher + parser 自注册；无 dylib |
| `workflow::artifacts` 第一波实例 | 三类产物解析 | markdown-doc / eval-checklist / tasks-progress；其余 kind 随第二刀以插件加法落地 |
| 前端 `renderers/` | renderer 注册表 + Fallback | 按 kind 路由；未注册 → Fallback（fallback_text + kind 徽标） |
