# desktop-change-worktree Specification (Delta)

## MODIFIED Requirements

### Requirement: 确定性依赖 bootstrap

写面 `create` SHALL 在 worktree 就绪后执行确定性依赖引导，按 workspace 配置的 `projects` 分两种模式：

- **未配置项目根**（`projects` 缺失 / 空数组）：探测 **worktree 根**的 lockfile（已知管理器映射：pnpm-lock.yaml / package-lock.json / yarn.lock / Cargo.lock 等，映射表由 design 定稿）→ 首匹配命中则在 worktree 根 spawn 对应安装命令（经 vcs 边界 port）；未知管理器或无 lockfile → 跳过并在警告面注记。
- **配置了项目根**（`projects` 非空）：按 **`depends_on` 的拓扑序**逐个解析 `<worktree>/<root>`（`root` 为相对 worktree 根的项目目录，`.` 等价 worktree 根）——依赖先于依赖方安装，无依赖或同层者保持**配置声明序**（声明序是唯一 tie-break，MUST NOT 改按字典序）；每个项目独立探测同一张映射表，首匹配命中即在该项目目录 spawn 安装（cwd = 项目目录）；每个项目至多一次安装，多个项目可各自安装。**脏依赖数据 MUST NOT 阻断安装**（校验面不做引用校验，消费面天然免疫）：`depends_on` 指向未声明 root 的悬空项直接跳过、自引用与成环经 `visited` 集合收口、同名 `root` 重复项折叠为一次安装（以首次出现为准）——四种情形一律零 `Err`、零新增诊断，产出确定安装序。项目目录不存在 / 无已知 lockfile / 安装失败均在该项目的警告注记（文案含项目根）并继续后续项目；MUST NOT 回退扫描 worktree 根。
- 配置文件 SHALL 从**主 workspace root** 读取（create 时刻的当前配置，含未提交改动）；worktree 内配置副本仍停在 base commit。项目根 SHALL 承载扫描 / 安装锚点、编码语言标注与项目间依赖声明三项语义；**这三项之外 MUST NOT 承载语义**（MUST NOT 沦为通用元数据口袋；字段面与软规则边界见 desktop-workspace-config「项目协议语义边界与消费点纪律」）。

bootstrap 失败 SHALL NOT 回滚 create（建档与 worktree 保留），失败信息 SHALL 立即呈现于 create 产出警告清单（static-check / test-execution 为确定性步骤无 agent 救场，后续大声失败兜底）。

#### Scenario: lockfile 命中执行安装

- **WHEN** 未配置 `projects` 且 worktree 检出含 `pnpm-lock.yaml` 的仓库
- **THEN** create 在 worktree 根 spawn 对应安装命令（cwd = worktree 根），成功后无相关警告

#### Scenario: 未知管理器跳过注记

- **WHEN** 未配置 `projects` 且 worktree 根无任何已知管理器 lockfile
- **THEN** bootstrap 跳过 spawn，警告清单注记「未识别依赖管理器，跳过依赖引导」

#### Scenario: 失败不回滚

- **WHEN** 安装命令非零退出（任一模式）
- **THEN** create 以成功收口（建档与 worktree 保留），警告清单含失败摘要；后续 static-check / test-execution 步以自身失败面兜底

#### Scenario: 多项目按拓扑序安装

- **WHEN** `projects` 为 `[{root: "packages/desktop/src-tauri", depends_on: ["packages/desktop"]}, {root: "packages/desktop"}]`，前者含 `Cargo.lock`、后者含 `pnpm-lock.yaml`
- **THEN** create 执行两次安装，**顺序与声明序相反**：先 `<worktree>/packages/desktop`（`pnpm install --frozen-lockfile`）后 `<worktree>/packages/desktop/src-tauri`（`cargo fetch --locked`）；顺利时警告清单为空

#### Scenario: 无依赖保持声明序

- **WHEN** `projects` 声明三个项目（顺序 A、B、C）且均无 `depends_on`、均命中 lockfile
- **THEN** 安装序逐字等于声明序 A、B、C（退化情形：拓扑序 = 声明序，既有「按序安装」断言零改动）

#### Scenario: 悬空与自引用不阻断

- **WHEN** 某项目 `depends_on` 含未声明的 root（悬空），另一项目 `depends_on` 含自身 root（自引用）
- **THEN** 两项目照常安装、其余项目照常安装；create `Ok`、警告清单未因依赖数据新增条目、诊断面零条目（悬空项静默跳过）

#### Scenario: 成环依赖按声明序落定

- **WHEN** 两个项目互相 `depends_on`（A→B→A），均命中 lockfile
- **THEN** create `Ok`，两项目各安装恰一次，安装序为确定值（按声明序落定，零 `Err`、不死循环）

#### Scenario: 重复 root 折叠安装一次

- **WHEN** `projects` 含两条 `root` 相同的元素（后者另带不同 `depends_on`）
- **THEN** 该项目恰安装一次（以首次出现的元素为准），零诊断、零 `Err`

#### Scenario: 项目内首匹配单安装

- **WHEN** 某项目目录同时含 `pnpm-lock.yaml` 与 `package-lock.json`
- **THEN** 该项目恰执行一次安装（映射表首匹配 `pnpm install --frozen-lockfile`），后续项目继续扫描

#### Scenario: 配置根缺失与项目无 lockfile

- **WHEN** `projects` 含 worktree 内不存在的 root，或项目目录存在但无任何已知 lockfile
- **THEN** 该项目零安装，警告清单含「项目根 "<root>"：在 worktree 内不存在，跳过依赖引导」或「项目根 "<root>"：未识别依赖管理器，跳过依赖引导」；create 照常成功并继续后续项目

#### Scenario: 非空 projects 抑制根扫描

- **WHEN** `projects` 非空且 worktree 根另有 lockfile
- **THEN** worktree 根 lockfile 不参与扫描、零安装；仅按项目根执行

#### Scenario: 单项目失败不中断后续

- **WHEN** 第一个项目安装非零退出、第二个项目命中 lockfile
- **THEN** create 照常成功，第一个项目记失败警告、第二个项目仍执行安装，警告按**安装序**累积

## Module Contract

| 模块 | 类型 | 职责 / 契约 |
|------|------|-------------|
| `crates/core/workflow/src/write/create.rs` | Function | 新增 `pub struct BootstrapProject { pub root: String, pub depends_on: Vec<String> }`（`depends_on` 缺省空，工作流侧不区分「未设」与「空」）；`create` / `bootstrap` 的项目入参由 `project_roots: &[String]` 改为 `projects: &[BootstrapProject]` |
| `crates/core/workflow/src/write/create.rs`（`install_order`，新私有纯函数） | Function | `fn install_order(projects: &[BootstrapProject]) -> Vec<&str>`：后序 DFS（依赖先于依赖方）+ 声明序 tie-break + `visited` 去重/断环 + 悬空依赖跳过；纯内存、零 `Err`、零 IO |
| `crates/core/workflow/src/write/create.rs`（`bootstrap` / `bootstrap_project`，改） | Function | 遍历序改由 `install_order` 产出；`bootstrap_project` 的映射表探测、警告文案、「每项目至多一次安装」「失败继续」语义逐字不变（只换遍历顺序与折叠后的元素集） |
| `crates/core/workflow/src/write/promote.rs` | Function | `promote_explore` 项目入参随 `create` 同形态（纯透传，语义零变化） |
| `crates/core/workflow/src/write/mod.rs` | API | 导出 `BootstrapProject`（命令层构造入参） |
| `src-tauri/src/commands/changes/mod.rs` / `src-tauri/src/commands/explores/mod.rs` | API（命令层） | 配置读取映射改为 `config::ProjectConfig → BootstrapProject`（`root` 原样、`depends_on` 缺省空）；`language` 在安装路径**不消费** |
| `crates/core/workflow/src/write/create_test.rs` / `promote_test.rs` | 测试 | 夹具 `create_with_projects` 收 `&[BootstrapProject]`；新增拓扑序 / 无依赖退化 / 悬空与自引用 / 成环 / 重复 root 五例；既有九例断言结构不变 |
