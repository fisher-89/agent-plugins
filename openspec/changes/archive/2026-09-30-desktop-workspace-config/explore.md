# desktop-workspace-config 探索笔记

日期：2026-09-29（探索轮，无 change）

## 背景与诉求

两个原始诉求，经探索合为一个 change：

1. desktop 工作区增加**配置页**：解析工作区内 `openspec/config.json` 并呈现。
2. 将 `openspec` 目录名设置为 **Rust 全局常量**，方便后续修改目录。

追加架构约束（用户裁决）：**读取并校验配置属于核心功能，未来其他模块仅允许从核心获取某项配置**——需要完整的 core/infra 归位，不落在 app 层。

## 现状盘点（关键发现）

- `foundation::layout::domain_dir_name()` 已是全包唯一 `openspec` 字面量触点；`layout_test.rs` 的**命名隔离扫描**（大小写敏感 `contains("openspec")`，产品源码 `.rs` 全扫，唯一例外 layout.rs）已是被测试强制的架构不变量。诉求 2 的目标"改名只改一处"**已达成**，真实差距是：fn → const 的形式、`resolve()` 体内 `changes`/`archive`/`explores` 子目录名仍是行内字面量、`config.json` 文件名将成为新触点。
- 命名隔离扫描**大小写敏感**：Rust 标识符 CamelCase 含 `OpenSpec` 不炸，snake_case 含 `openspec` 直接炸——DTO 命名须用 `WorkspaceConfig` 一类，禁 `openspec_config`。
- `embedded-cli` 能力显示 desktop 曾捆绑 CLI，2026-08-17 `retire-openspec-bundled` 已退役，workflow 域全部 native Rust 解析——排除"shell 出调 CLI 取配置"路线，与 B 决定一致。
- CLI 侧语义源：`plugins/dev-team/bin/src/schemas/config/config.schema.ts`（zod/v4，prefault 默认值 + 枚举 + 值域 + passthrough）；`lib/config.ts` 的 `readConfig` 对缺失/坏 JSON/校验失败一律静默回退默认值。
- zod 默认值：coverage lines 80 / branches 70 / functions 75、mutation score 70；framework 八值枚举；suite `root` 禁 glob 通配符（`*?{[`）。
- 配置页架构有现成模板轨道：`/info` 代码统计页（无状态解析 + 显式刷新 + specta 生成 TS bindings + inline 错误态）。
- crate 落位"宪法"：`specs/desktop-crate-layout/spec.md`——三层分组依赖规则、五类边界分类学、`core/archi` 预留行即新 core crate 入座先例。

## 决策记录

| 决策点 | 结论 |
|---|---|
| D1 解析保真度 | **B：core 全量复刻 CLI 校验语义**（zod 语义翻成 serde：枚举/值域/通配符禁令/默认值填充/passthrough） |
| D1 精化 | **复刻校验、不复刻吞错**：`ConfigReport { config, diagnostics }` 信封——未来模块永远拿到合法配置（CLI 兼容的默认值填充），配置页拿 diagnostics 呈现"哪里非法/哪些字段吃了默认" |
| D4 常量化范围 | **完整版**：layout.rs 顶部常量组——`DOMAIN_DIR_NAME` + `changes`/`archive`/`explores` 子目录名 + `config.json` 文件名，`resolve()` 全部引用常量 |
| crate 归位 | **新 `crates/core/config`**（裸名 `config`）：读文件 + serde 解析 + 校验 + 默认值，依赖 `foundation + serde + specta`；依赖规则 `desktop-app → config → foundation`，未来 `workflow / 其他模块 → config` |
| 职责切分 | foundation 管路径（在哪，唯一字面量触点，不变量不变）；config 管语义（是什么、合法吗、默认值）。"foundation 保持极小"不破 |
| 只准从核心取配置的执法 | ① 依赖规则写入 `desktop-crate-layout` spec 增量；② 命名隔离扫描扩展：产品源码含 `"config.json"` 字面量即 panic（唯一例外 layout.rs）——绕开 `config::load` 直接读文件者连文件名都拼不出 |
| 缺文件/坏 JSON | config.json 不存在 → 空态（多数 workspace 常态，非错误）；JSON 语法坏 → inline 错误态（沿 info 页先例）；未知字段 → passthrough 呈现 |
| 页面形态 | **只读**；`/config` 路由 + 侧栏"页面"组加 [配置]；显式刷新模型（复刻 use-code-stats），无轮询无缓存 |
| 漂移防线 | **不做**（用户裁决）：CLI 短期内会下线，Rust core/config 将成唯一实现，双实现长期维护不成立 |
| 范围 | **一个 change**：常量组 + config_path + core/config crate + 命令轨 + bindings + 配置页 + spec 修订三处 |
| 版本 | 配置页为用户可见变更，archive 时 desktop bump 0.3.8 → 0.3.9 |

## 架构图

```
crates/core/
├── foundation/    ← 完整版常量组 + resolve() + config_path()
│                    路径从哪来 —— 全包唯一字面量触点
├── workflow/      ← change 域纯读（不动）
├── agent/         ← agent 域契约（不动）
└── config/ (新)   ← 读 + serde 解析 + 校验 + 默认值
      API: load(root) → ConfigReport { config, diagnostics }

src/commands/config/   薄包装（stats 轨道模板：参数转换 → 调用 → 错误映射）
bindings.rs            collect_commands 注册 → export_bindings 生成 TS
前端 views/config/     view + use-workspace-config hook + components
routes.tsx / sidebar   /config + [配置]
```

## 改动面清单

- `foundation/src/layout.rs`：常量组 + `config_path()`（形态待 design：Layout 加第四字段 vs 独立函数——config.json 是文件不是树，独立函数更贴切，倾向后者）
- `crates/core/config/`：新 crate——DTO（serde + specta::Type）+ 解析 + 校验 + 默认值 + 单测
- `src/commands/config/mod.rs`：新命令轨 `workspace_config(root)`，`*_inner` 纯函数模式
- `bindings.rs`：命令注册；`export-bindings` 重导出
- 前端 `views/config/`：`config-view.tsx` + hooks + components（合法配置分区呈现 + diagnostics 警示区 + 未设阈值标注"未设（默认 N）"）
- `routes.tsx` / `app-sidebar.tsx`：路由与导航
- spec deltas ×3：`desktop-crate-layout`（依赖规则 + Module Contract + 扫描扩展）、`workspace-layout-resolution`（常量组 + config_path）、新能力 spec `desktop-workspace-config`
- `layout_test.rs`：隔离扫描扩展 `"config.json"` 禁令

## 留给 design 阶段的问题

- `config_path()` 形态（Layout 字段 vs 独立函数）
- DTO 字段形状与 diagnostics 粒度（逐字段 vs 逐条消息）
- 校验实现方式（serde `deserialize_with` 逐字段 vs 事后校验 pass）
- 配置页分区呈现的组件拆分
