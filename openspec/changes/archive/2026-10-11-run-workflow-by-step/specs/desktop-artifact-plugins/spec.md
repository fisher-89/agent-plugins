# desktop-artifact-plugins Specification (Delta)

## MODIFIED Requirements

### Requirement: Rust 侧 matcher/parser 静态注册表

workflow crate 的 `artifacts/` 模块 SHALL 提供 matcher + parser 的 trait 与编译期静态注册表，MUST NOT 使用 dylib 动态加载。每类产物为一个自包含模块：matcher 判定命中、parser 产出 `ArtifactEnvelope`，并自注册进注册表；扫描循环 / DTO / 组件路由对产物类型 MUST NOT 硬编码分支。候选来源 SHALL 由 **phase 定义清单驱动**（desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）：`discover_artifacts` 按清单相对路径枚举 change 目录命中候选，`enumerate_candidates` 全树扫描与文件树遍历 SHALL 退役；matcher SHALL 仅在清单命中的候选项内判定 kind（相对路径模式 vs 扩展名）。原 Docs（markdown 产物）探测逻辑 SHALL 折叠进 `markdown-doc` 插件的 matcher（按代际做文件名匹配 + 清单归属），MUST NOT 保留并行的独立探测机制。

#### Scenario: 加产物不改核心

- **WHEN** 新增一类产物类型
- **THEN** 仅新增一个 Rust 模块（matcher + parser 自注册）与一个 React 模块（组件 + 注册），扫描循环 / DTO / 路由代码零改动

#### Scenario: Docs 探测即插件且清单驱动

- **WHEN** phase 清单声明 proposal / design / tasks / specs / explore 等 markdown 产物
- **THEN** 候选由清单相对路径枚举产生，`markdown-doc` 插件的 matcher 命中并产出信封；不存在清单之外的文档探测路径、无 `enumerate_candidates` 全树扫描

### Requirement: 第一波三个插件实例

MVP SHALL 将服务第一刀视图的产物实现为第一波插件实例，使抽象从第一天被真实实例校准：

| kind | 内容 |
|------|------|
| `markdown-doc` | markdown 产物文档（proposal / design / tasks / specs / explore，按代际匹配） |
| `eval-checklist` | eval 条目的 checklist 展开（item / pass / evidence） |
| `tasks-progress` | tasks.md 勾选进度（统计 `- [ ]` / `- [x]`） |

每个 kind SHALL 同时具备 Rust 侧 matcher + parser 与 React 侧 renderer 注册。候选来源 SHALL 为 phase 清单驱动：`markdown-doc` / `tasks-progress` 按清单相对路径枚举命中（title / rank 名单逻辑保留），matches 判定改「清单类型 == markdown-doc / tasks-progress」而非「扩展名 == md」；`eval-checklist` 为 PhaseRecord 投影（payload 自带 `phase` + `attempt`），SHALL 保持 `(phase, attempt)` 关联分支，不走相对路径清单（与其余两插件来源不同，统一表达方式由 design 定稿）。`file-log`、`test-report-summary`、`html-report-ref` 不在 MVP 范围，SHALL 随第二刀对应视图（file_log 时间线 / reports 渲染 / mutation 内嵌）以插件加法方式落地。

#### Scenario: 三个 kind 全部可产出

- **WHEN** 对含完整 v2 数据的 fixture change 发起产物发现
- **THEN** 三个 kind 均可被 matcher 命中并解析出信封（就各自清单声明的源文件 / PhaseRecord 投影）

#### Scenario: markdown-doc 清单来源

- **WHEN** change 目录含 `proposal.md` 与清单未声明的 `notes.md`
- **THEN** `proposal.md` 经清单相对路径枚举命中并产出 markdown-doc 信封；`notes.md` 不被枚举（零候选、零信封）

#### Scenario: tasks 进度统计

- **WHEN** tasks.md 含若干 `- [ ]` 与 `- [x]` 条目
- **THEN** `tasks-progress` 的 payload 含未完成与已完成计数，renderer 呈现进度

## ADDED Requirements

### Requirement: 清单驱动发现与全树扫描退役

产物发现 SHALL 以 phase 定义清单为唯一候选来源：`discover_artifacts(change_dir, phases)` 签名 SHALL 变为「读 phase 定义清单 → 按相对路径枚举匹配 change 目录 → 插件 parse」（`enumerate_candidates` 文件树遍历 + eval 序号候选整个退役）；`ArtifactDescriptor.source` SHALL 继续以相对 POSIX 路径（`encode_file_source`）作关联键，phase 归属 SHALL 由清单声明供给（不再由前端 `docColumn` 派生）。`eval-checklist` SHALL 为 PhaseRecord 投影分支（`(phase, attempt)` 关联），不参与相对路径枚举；与其余清单驱动插件的统一表达方式由 design 定稿。清单未命中的文件 MUST NOT 进入候选集（白名单/清单驱动取代全量扫描），`registry.rs` 的 `collect_files` 全树遍历 SHALL 退役。

#### Scenario: 白名单清单驱动

- **WHEN** change 目录含清单声明与未声明文档混合
- **THEN** 仅清单声明的相对路径进入候选并可能产出信封；未声明文件零候选、零信封，无全树扫描残留

#### Scenario: 相对路径关联键不变

- **WHEN** 某清单命中的 markdown 产物产出 `ArtifactDescriptor`
- **THEN** `source` 为该文件相对 POSIX 路径（既有 `encode_file_source` 口径），phase 归属自清单声明，前端可直接按 source 挂列（无 docColumn 映射）

#### Scenario: eval-checklist 投影分支保留

- **WHEN** 某 change 的 PhaseRecord 含带 checklist 的 attempt
- **THEN** `eval-checklist` 仍经 PhaseRecord 投影产出（`(phase, attempt)` 关联），不依赖相对路径清单、不参与文件枚举

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `workflow::artifacts` | 信封 + trait + 静态注册表 | `ArtifactEnvelope { kind, version, title, payload, fallback_text }`；matcher + parser 自注册；无 dylib；候选来源 = phase 清单 |
| `crates/core/workflow/src/artifacts/registry.rs` | 清单驱动发现 | `discover_artifacts(change_dir, phases)` 读清单枚举；`enumerate_candidates` / `collect_files` 全树扫描退役；`encode_file_source` 相对路径口径不变 |
| `workflow::artifacts` 第一波实例 | 三类产物解析 | markdown-doc / tasks-progress 改清单驱动；eval-checklist 保留 PhaseRecord 投影分支；其余 kind 随第二刀以插件加法落地 |
| 前端 `views/changes/renderers/` | renderer 注册表 + Fallback | 按 kind 路由；未注册 → Fallback（fallback_text + kind 徽标） |
