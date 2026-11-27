# desktop-change-worktree Specification (Delta)

## ADDED Requirements

### Requirement: 子目录 workspace 的执行根对齐（exec root = host/rel）

change 的执行根 SHALL 由 `ChangeRecord` 两列派生：`worktree`（git 落位目录 host）+ `exec_root`（执行根绝对路径；`None` = 与 `worktree` 同值，字段语义见 desktop-change-state-store）。workspace root 为 git 仓库子目录时，`rel` SHALL 为该子目录相对 git 根的偏移（`/` 分隔、根 workspace 为 `""`），且：

- **落位不纳入 `rel`**：worktree 落位 SHALL 仍恒为 `<git 根父目录>/.worktrees/<仓库目录名>/<name>`——MUST NOT 叠加 `rel`、MUST NOT 以 workspace root 为锚、MUST NOT 进入 branch 名。
- **执行根 = `host/<rel>`**：`rel` 非空时执行根 SHALL 为落位目录下的对应子目录（`worktree.join(rel)`）。
- **`rel` 的射程**：`rel` SHALL 只参与 ① 执行根派生；② 准入判据（见下）。MUST NOT 进入 store 身份派生（`for_root` 恒 workspace root）。
- **偏移来源**：`rel` SHALL 经 `git rev-parse --show-prefix` 原样取得（根 workspace 为空串、尾带 `/`），MUST NOT 由 canonicalize 路径 `strip_prefix` 重建（`\\?\` 前缀 / 真实大小写 / 分隔符三个坑一次绕开）。

核心不变量（本能力与全部消费点的唯一判据）：`exec root ≡ workspace root 在 worktree 内的对应物` ⇒ 任何「相对 workspace root」的声明在主仓与 worktree 两侧同解。仓内四个既有声明因此自动正确，MUST NOT 为它们逐个加特判：`projects[].root`（bootstrap 安装锚 = 执行根）、`tests[].root`（run 的执行根）、`static_analysis` 相对路径命令（检查器 spawn cwd）、`write_protection.files[].glob`（装配根 = 主仓 workspace root）。

**准入两层（MUST NOT 回退到 host 执行）**：

1. **前置判据**：`HEAD:<rel>` 不在案（`git rev-parse --verify --quiet` 非零）⇒ 显式 `Err`；判定位置 MUST 在 `git worktree add` 与建档之前（拒绝面零 worktree、零 branch、零建档、零目录）。`rel = ""` SHALL 恒命中且 MUST NOT 新增 git spawn。`HEAD` 内该路径缺失的三形态（子目录整体未跟踪 / 在 `HEAD` 已删除 / 被 gitignore）SHALL 共用同一拒绝面，文案 SHALL 含 `rel`、该形态的成因与「把该子目录纳入 git 跟踪并提交，或改用仓库根作为 workspace root」引导。
2. **后置断言**：`add_worktree` 成功后 `exec_root.is_dir()` 为假（竞态 / 磁盘异常）⇒ 走既有补偿链（回收 host + branch + 建档）后 `Err`，文案 SHALL 与前置拒绝可辨（指明「已回收」）。

**同仓同名拒绝**：落位与 branch 名都是全仓命名空间 ⇒ 同仓两个子目录 workspace 各自 create 同名 change SHALL 以既有冲突前置（worktree 目标目录已存在 / branch `change/<name>` 已存在）显式拒绝，零副作用；跨 workspace 命名唯一即可规避。本能力 MUST NOT 引入同名隔离机制（`rel` 纳入落位是独立变更）。

**根 workspace 零差异**：`rel = ""` 时 SHALL 不写 `exec_root` 列（`None`），全部路径、spawn 序列与行为 MUST 与拆分前逐字相同。

#### Scenario: 根 workspace 零差异

- **WHEN** 对 git 根 workspace（`rel = ""`）创建 change 并随即发起 run
- **THEN** 建档记录 `exec_root` 为 `None`（落库形态与本变更前逐字相同）、`worktree` 为落位绝对路径；create 的 spawn 序列与本变更前逐字相同（`tracked_at_head` 零调用）；会话 cwd / Layout / 检查器目录 = 该 worktree 路径

#### Scenario: 子目录 workspace 建域四件套

- **WHEN** workspace root 为 git 仓库子目录（如 `<repo>/packages/desktop`）时调用写面 `create`
- **THEN** `ChangeRecord.worktree` = `<repo 父>/.worktrees/<repo>/<name>`、`ChangeRecord.exec_root` = `<repo 父>/.worktrees/<repo>/<name>/packages/desktop`；`git worktree add` 的目标仍是 host；`openspec/changes/<name>/explore.md` 落在执行根下（`host/openspec/` 与主仓 `<repo>/openspec/` 两侧均无该目录）

#### Scenario: 未跟踪与失效子目录前置拒绝

- **WHEN** workspace root 为以下三种形态之一：本地新建且从未 `git add` 的子目录、`HEAD` 中已删除但工作区仍有残留的子目录、被 gitignore 的子目录
- **THEN** create 返回显式 `Err`（文案含该相对偏移、成因与引导），零建档、零目录、零 worktree、零 branch；主仓 `git status` 与 `git worktree list` 零变化

#### Scenario: 跟踪但仅含被忽略文件仍可建域

- **WHEN** 子目录在 `HEAD` 中仅含被 gitignore 的文件（目录本身被跟踪）时调用写面 `create`
- **THEN** 前置判据命中、建档成功（`exec_root` 在案），执行根内可建 change 目录；其内部被忽略的资产不出现在 worktree 内属既定边界（V1 边界第 4 条）

#### Scenario: 前置命中后置缺席即补偿回收

- **WHEN** 前置判据命中而 `add_worktree` 后执行根目录仍不在场（竞态 / 磁盘异常夹具）
- **THEN** create 返回 `Err` 且完成既有补偿（`git worktree list` 无该 worktree、branch `change/<name>` 已删、db 无建档记录），文案指明「已回收」并给双源残留指引

#### Scenario: 同仓同名 change 被显式拒绝

- **WHEN** 同仓子目录 workspace A（`rel = packages/desktop`）已建 change `x`，子目录 workspace B（`rel = apps/web`）以同名 `x` 调用 `create`
- **THEN** 第二次返回既有冲突 `Err`（worktree 目录已存在或 branch 已存在，文案给清理指引），零建档、零目录、零 worktree；断言落位路径与 `rel` 无关（两 workspace 的同名落位指向同一 `…/<repo>/<name>`）

#### Scenario: 相对 workspace root 的四个声明两侧同解

- **WHEN** 审查 `projects[].root` / `tests[].root` / `static_analysis` / `write_protection.files[].glob` 四个消费点在子目录 workspace 的 worktree change 上的运行结果
- **THEN** 前三者以执行根为基准解析（bootstrap cwd、测试 runner 的项目目录、检查器 spawn cwd 均落在 `host/<rel>` 内），第四者仍以主仓 workspace root 为装配根；四处均无「按 `rel` 拼路径」的特判代码

## MODIFIED Requirements

### Requirement: create 分配 worktree 与分支基线

写面 `create` 对每次成功创建 SHALL 分配同名 worktree：以主仓 HEAD 为基线 `git worktree add` 落位于 git 根同级 `.worktrees/<仓库目录名>/`，并建 branch `change/<name>`——主仓未提交与 staged 改动 MUST NOT 进入 worktree（基线恒 HEAD）。`openspec/changes/<name>/` 目录树与 `explore.md` SHALL 建于**执行根**内（`rel = ""` 时即 worktree 根；主仓 active 树在 merge 前 MUST NOT 含该目录）。建档 SHALL 记录 worktree 绝对路径（host）、执行根（`ChangeRecord.exec_root`——`rel` 非空时 = `host/<rel>`，根 workspace MUST NOT 写该列）与创建基线（`base_commit`，字段语义见 desktop-change-state-store）。前置校验 SHALL 在任何写 IO 之前覆盖：git 可用性（git 不可发现或主仓非 git 仓 → 显式 `Err` 引导，MUST NOT 静默回退主 root 创建）、branch `change/<name>` 已存在、worktree 目标目录已存在、子目录 workspace 的相对偏移在 `HEAD` 中在案（`rel = ""` 恒命中且零新增 spawn，见「子目录 workspace 的执行根对齐」）——拒绝面零 worktree、零建档、零目录。`git worktree add` 成功后 SHALL 断言执行根目录在场，缺席即走既有补偿链并 `Err`（后置断言，理由同上）。进程 spawn（git / bootstrap）SHALL 经 core/workflow 定义、vcs 边界（`crates/infra/vcs`，裸名 `vcs-runtime`）实现的 port 缝承载，core/workflow 零 infra 依赖、零进程 spawn 红线不变。

#### Scenario: 成功建域四件套

- **WHEN** 对干净 git 仓 workspace 以合法名称与 goal 调用写面 `create`
- **THEN** worktree 目录存在于 `<git 根父目录>/.worktrees/<git 根目录名>/<name>`、branch `change/<name>` 在案且指向主仓 HEAD、执行根内（根 workspace 下即 worktree 根）`openspec/changes/<name>/explore.md` 为 goal 原文、db `ChangeRecord` 携 worktree 路径（host）与 base_commit；主仓 active 树无该目录

#### Scenario: git 不可用拒绝

- **WHEN** git 不可发现或主仓不是 git 仓库时调用写面 `create`
- **THEN** 返回显式 `Err`（含引导文案），不产生任何 worktree / branch / 目录 / db 记录，MUST NOT 静默回退主 root 创建

#### Scenario: branch 与目录冲突拒绝

- **WHEN** branch `change/<name>` 已存在或 worktree 目标目录已存在时调用写面 `create`
- **THEN** 返回显式 `Err`（含冲突对象），零 worktree、零建档、零目录改动

#### Scenario: 基线恒 HEAD

- **WHEN** 主仓存在未提交改动（工作区或 staged）时创建 worktree 成功
- **THEN** worktree 内容为主仓 HEAD 检出，不含任何未提交改动；base_commit 记录该 HEAD

### Requirement: 双 root 执行语义与 db 身份不变量

change run SHALL 以双 root 组合执行：

- **workspace root**（用户注册的 canonical root）是唯一 store 身份锚：`for_root` 派生 workspace 库、provider / 会话 / change 建档 / 相位状态全部钉在其上；worktree 路径 MUST NOT 进入 `for_root`（直接喂入会铸出空库）。
- **exec root**（`ChangeRecord.exec_root` 在案 → 该绝对路径；`exec_root=None` 且 `worktree` 在案 → 该 worktree 绝对路径；两者皆 `None`（legacy 记录）→ 主 workspace root）承担：会话 cwd（经既有 `turn.root → SessionCtx.workspace_root` 通道，CLI `current_dir` 与 SDK `LoopTurn.cwd` 同锚）、工件根（`Layout::resolve`）、git diff 与检查器（static-check / test-execution）执行目录、bootstrap 扫描锚、archi 改动侧。
- **host**（git 落位目录，`ChangeRecord.worktree`）承担全部 git 步与回收：`add_worktree` / `remove_worktree`、在场判定、`restore_baseline`、`worktree_snapshot`、`dirty` / `commit_all` / `rebase` / `ff_merge`、branch 探测。MUST NOT 把 exec root 交给 git 步——`reset --hard` 覆盖全树而 `clean -fd` 与 `ls-files` 只认 cwd 子树，锚 exec root 会得「半域恢复 / 索引缩域」的静默失真（workspace root 为仓库子目录时 exec root = `host/<rel>`，见「子目录 workspace 的执行根对齐」）。

`compose_turn` 的 store 半边 SHALL 由命令层预解析的 workspace root 库实例注入；相位状态机零 fs 保持不动；会话 resume 的 cwd 由当次 turn 的 root 重推导（`SessionRecord` 不存 cwd），resume 链零改动。SDK 路径沙箱前缀校验以会话 cwd（即 exec root）为锚（既有机制零改动），worktree change 的文件编辑 SHALL 因此囚于执行根内（根 workspace 下即整个 worktree）；worktree 内 `git diff HEAD` SHALL 恰为本 change 编辑集（不含主仓无关改动）。会话转录 / 三槽位 / StepRecord SHALL 照旧落 workspace root 库——db 不受 worktree 存在或删除影响是本能力核心不变量。

#### Scenario: worktree run 的 db 身份不变

- **WHEN** worktree change 的 run 走完一相位（executor / evaluator 会话 + phase_log 落账）
- **THEN** 会话记录、槽位与 StepRecord 落在 workspace root 对应的 workspace 库（provider 可解析、与主 root 会话同库可查），compose 全程未以 worktree 路径进 `for_root`

#### Scenario: 编辑囚于 worktree

- **WHEN** worktree change 的 executor 会话（SDK 引擎）尝试编辑 worktree 目录外文件
- **THEN** 路径沙箱前缀校验拒绝（既有机制，锚 = exec root），编辑不落主仓

#### Scenario: diff 面恰为本 change 编辑集

- **WHEN** 主仓存在无关未提交改动、worktree 内 executor 已产生本 change 编辑后组装 executor prompt
- **THEN** git diff 上下文取自 worktree，仅含本 change 编辑集，不含主仓无关改动

#### Scenario: 子目录 workspace 的 run 全链落执行根

- **WHEN** workspace root 为 git 仓库子目录（`rel = packages/desktop`）的 worktree change 发起 run 并走完一相位
- **THEN** 会话 cwd / `Layout::resolve` 的工件根 / static-check 与 test-execution 的 spawn cwd / archi 改动侧全部为 `host/packages/desktop`；归档相位与 initialize 的 git 步（`restore_baseline` / `worktree_snapshot` / `commit_all`）仍以 host 为锚（覆盖全树、索引不缩域）

#### Scenario: legacy 主 root 执行

- **WHEN** 对 `worktree=None` 的存量建档 change 发起 run
- **THEN** exec root 解析为主 workspace root，既有全链语义（cwd / Layout / diff / 检查器 / 归档）零变化

### Requirement: 确定性依赖 bootstrap

写面 `create` SHALL 在 worktree 就绪后执行确定性依赖引导，按 workspace 配置的 `projects` 分两种模式。**扫描与安装锚恒为执行根**（`rel = ""` 时即 worktree 根；下文「执行根」即此义）：

- **未配置项目根**（`projects` 缺失 / 空数组）：探测 **执行根**的 lockfile（已知管理器映射：pnpm-lock.yaml / package-lock.json / yarn.lock / Cargo.lock 等，映射表由 design 定稿）→ 首匹配命中则在执行根 spawn 对应安装命令（经 vcs 边界 port）；未知管理器或无 lockfile → 跳过并在警告面注记。
- **配置了项目根**（`projects` 非空）：按 **`depends_on` 的拓扑序**逐个解析 `<执行根>/<root>`（`root` 为相对执行根的项目目录，`.` 等价执行根）——依赖先于依赖方安装，无依赖或同层者保持**配置声明序**（声明序是唯一 tie-break，MUST NOT 改按字典序）；每个项目独立探测同一张映射表，首匹配命中即在该项目目录 spawn 安装（cwd = 项目目录）；每个项目至多一次安装，多个项目可各自安装。**脏依赖数据 MUST NOT 阻断安装**（校验面不做引用校验，消费面天然免疫）：`depends_on` 指向未声明 root 的悬空项直接跳过、自引用与成环经 `visited` 集合收口、同名 `root` 重复项折叠为一次安装（以首次出现为准）——四种情形一律零 `Err`、零新增诊断，产出确定安装序。项目目录不存在 / 无已知 lockfile / 安装失败均在该项目的警告注记（文案含项目根）并继续后续项目；MUST NOT 回退扫描执行根。
- 配置文件 SHALL 从**主 workspace root** 读取（create 时刻的当前配置，含未提交改动）；worktree 内配置副本仍停在 base commit。项目根 SHALL 承载扫描 / 安装锚点、编码语言标注与项目间依赖声明三项语义；**这三项之外 MUST NOT 承载语义**（MUST NOT 沦为通用元数据口袋；字段面与软规则边界见 desktop-workspace-config「项目协议语义边界与消费点纪律」）。

bootstrap 失败 SHALL NOT 回滚 create（建档与 worktree 保留），失败信息 SHALL 立即呈现于 create 产出警告清单（static-check / test-execution 为确定性步骤无 agent 救场，后续大声失败兜底）。

#### Scenario: lockfile 命中执行安装

- **WHEN** 未配置 `projects` 且 worktree 检出含 `pnpm-lock.yaml` 的仓库
- **THEN** create 在执行根 spawn 对应安装命令（cwd = 执行根），成功后无相关警告

#### Scenario: 未知管理器跳过注记

- **WHEN** 未配置 `projects` 且执行根无任何已知管理器 lockfile
- **THEN** bootstrap 跳过 spawn，警告清单注记「未识别依赖管理器，跳过依赖引导」

#### Scenario: 失败不回滚

- **WHEN** 安装命令非零退出（任一模式）
- **THEN** create 以成功收口（建档与 worktree 保留），警告清单含失败摘要；后续 static-check / test-execution 步以自身失败面兜底

#### Scenario: 多项目按拓扑序安装

- **WHEN** `projects` 为 `[{root: "packages/desktop/src-tauri", depends_on: ["packages/desktop"]}, {root: "packages/desktop"}]`，前者含 `Cargo.lock`、后者含 `pnpm-lock.yaml`
- **THEN** create 执行两次安装，**顺序与声明序相反**：先 `<执行根>/packages/desktop`（`pnpm install --frozen-lockfile`）后 `<执行根>/packages/desktop/src-tauri`（`cargo fetch --locked`）；顺利时警告清单为空

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

- **WHEN** `projects` 含执行根内不存在的 root，或项目目录存在但无任何已知 lockfile
- **THEN** 该项目零安装，警告清单含「项目根 "<root>"：在执行根内不存在，跳过依赖引导」或「项目根 "<root>"：未识别依赖管理器，跳过依赖引导」；create 照常成功并继续后续项目

#### Scenario: 非空 projects 抑制根扫描

- **WHEN** `projects` 非空且执行根另有 lockfile
- **THEN** 执行根 lockfile 不参与扫描、零安装；仅按项目根执行

#### Scenario: 单项目失败不中断后续

- **WHEN** 第一个项目安装非零退出、第二个项目命中 lockfile
- **THEN** create 照常成功，第一个项目记失败警告、第二个项目仍执行安装，警告按**安装序**累积

### Requirement: worktree 路径 UI 可见性

create 产出与 change 详情 SHALL 暴露 worktree 绝对路径（记录带 worktree 时），供用户 review / 手动 commit / merge 可达（worktree 藏于 home 数据目录下的可见性补偿）；legacy 记录 SHALL 出线 null（前端不呈现该信息面）。**执行根在场时 SHALL 另出线 `execRoot` 并以两行呈现**——`worktree：<host>`（git 落位目录，对得上 `git worktree list`、供开编辑器看整棵检出）+ `执行根：<execRoot>`（本 change 的工作面，用户实际编辑的地方）；`execRoot = null`（legacy / 根 workspace）时 MUST NOT 渲染第二行、MUST NOT 留空行。两列 MUST NOT 互相顶替（把执行根当落位目录给用户，会让用户 `cd` 到既不是自己编辑处、也不是 `git worktree list` 所里的位置）。create 对话框 SHALL 行内呈现警告清单（脏仓 / bootstrap）。详情出线经 detail DTO 字段演进（wire contract 冻结约束下走 golden 显式重写流程）。

#### Scenario: create 成功呈现路径与警告

- **WHEN** create 成功返回（含或不含警告）
- **THEN** 对话框成功面呈现 worktree 绝对路径（`execRoot` 非空时另起一行呈现执行根）与警告清单（有则行内列出）

#### Scenario: 详情信息行可达

- **WHEN** 打开带 worktree 记录的 change 详情
- **THEN** 详情信息面呈现 worktree 路径；legacy change 详情该面为 null 且不渲染

#### Scenario: 子目录 workspace 两行并存

- **WHEN** 打开子目录 workspace 的 worktree change 详情（记录 `worktree` = host、`exec_root` = `host/<rel>`）
- **THEN** 信息面呈现两行（落位目录 + 执行根），两行文本各自含自身绝对路径且不相等；根 workspace change（`exec_root = null`）只呈现落位目录一行、无占位空行

#### Scenario: 两列不互相顶替

- **WHEN** 审查 create 成功面与详情信息行的取值来源
- **THEN** 落位行恒取 `worktree`、执行根行恒取 `execRoot`；两处均无「以执行根填充落位行」或反向的表达式；面板内的执行根文案行保持退役态（不复制第三份）

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **branch 清理手动（删除面已偿还）**：worktree 由 archive 相位 Cleanup 步自动移除（两段式回收尽力步，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」与本 spec「worktree 回收两段式」）；归档路径下 branch `change/<name>` 仍保留（历史可溯）。需要彻底清理时走清单删除入口并勾选「同时清空 worktree」——该路径同时删除 worktree 目录与 branch（见 desktop-change-delete）。
2. **legacy 主 root 路径**：`worktree=None` 存量记录照旧主 root 执行，零迁移；非 git 仓 workspace 的 create 显式拒绝（不提供 legacy 创建逃生口，真实诉求另立变更）。
3. **`agent_start` 与探索笔记留主仓 root**：exec 轨道手动会话与 `.dev-team/explores/` 笔记不混入 change worktree。
4. **未跟踪资产不随 worktree 复制（含 config / AGENT.md 冻结）**：worktree 内配置停在创建时刻（base commit 检出），主仓中途改动不传导，文档化；子目录 workspace 的前置判据（`HEAD:<rel>`）只保证该子目录被跟踪——其内被 gitignore 的资产（`config.json` / `AGENT.md` / `.dev-team/` 架构资产 / `openspec/` 语料树）MUST NOT 出现在 worktree 内（本仓 `.dev-team/` 已跟踪，不受影响）。唯一例外是 create 在创建时读取**主 workspace root** 的当前配置以选择 `projects[].root`（依赖引导锚点），worktree 内配置副本仍冻结。
5. **构建成本接受**：fresh worktree 依赖 / 构建产物全量重建；共享 target 目录类逃生口（`CARGO_TARGET_DIR` 等）仅文档化，不内建机制。
6. **孤儿 worktree 对账（两源）**：create 中途崩溃遗留的 worktree / branch、以及删除失败留下的已注销孤儿，SHALL 以**两源**对账——① `git worktree list`；② 直接检查 worktree 落位根 `<git 根父目录>/.worktrees/<仓库目录名>/` 下的子目录。已注销孤儿不在 git 登记面（`list` 不显示、`prune` 无输出、`remove` 报 `not a working tree`），单靠 git 对账无法定位，故面向用户的残留指引 MUST NOT 只给 git 一侧。有建档记录的残留经清单删除入口勾选「同时清空 worktree」清理（desktop-change-delete——删除面按记录定位，故只覆盖有建档记录的残留），且回收单点自带 OS 层兜底、一次失败不再等于终态（见「worktree 回收两段式」）；无记录的孤儿仍须手动删除其目录。create 前置校验保证下次创建显式报错而非误用；create 前置 worktree 磁盘保底门槛为失控防线（见 desktop-change-create）。
7. **report_dir 绝对路径接受失效**：phase_log 报告文本内嵌的 worktree 绝对路径在 worktree 删除后失效（转录历史，仅展示面），不做相对化；归档 Cleanup 步使删除成为缺省路径，本边界维持。
8. **并行 merge 冲突处置分层**：两 change 改同一文件各自绿、merge 撞时——经 archive 相位自动合入的冲突由 Rebase / ResolveConflicts 步的解冲突 agent 裁决（agent 无法裁决时 lean 档停链转人工，见 desktop-change-archive）；链外用户手动 merge 的冲突走 git 原生冲突处理。
9. **上游锁面不可表达**：`projects[].root` 的校验拒绝 `..` 段与绝对路径（该路径会成为安装命令的 cwd，逃逸即拆 cwd 逃逸护栏）⇒ 子目录 workspace 无法声明仓库根的 lockfile（monorepo 根 `pnpm-lock.yaml` 一类）；legacy 扫描也只看执行根 ⇒ 恒落「未识别依赖管理器，跳过依赖引导」。逃生口是 `projects: [{ root: "." }]`（仅当依赖清单就在执行根时有效），MUST NOT 为「看到仓库根」放开路径逃逸。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 九条边界均可考，后续变更无需重新论证是否知情

#### Scenario: 残留指引双源

- **WHEN** 审查三处面向用户的 worktree 残留指引（`remove_worktree` 的 `Err`、create 前置⑨ 门槛文案、create 补偿残留文案）
- **THEN** 三者均同时给出「`git worktree list` 对账」与「直接检查 `.worktrees/<仓库目录名>/` 下的子目录」两源，且均不含「执行 `git worktree remove --force <path>` 清理」单源指引

## Module Contract

### 模块增量：子目录执行根对齐（worktree-subdir-exec-root）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/workflow/src/write/worktree.rs` | 写面 → vcs 缝（端口形态改） | `fn placement(&self, main_root: &Path) -> Result<WorktreePlacement, String>` 取代 `worktree_root`；`fn tracked_at_head(&self, main_root: &Path, rel: &str) -> Result<bool, String>`（新增；`rel = ""` 直返 `true`）；`pub struct WorktreePlacement { pub root: PathBuf, pub rel: String }`（`root` = `<.worktrees>/<repo>`，`rel` = `/` 分隔、根为 `""`） |
| `crates/infra/vcs/src/lib.rs`（`ProcessWorktree`） | 缝实现 | `placement` / `tracked_at_head` 委托 `git::placement` / `git::tracked_at_head`；`worktree_root(workspace_root)` 公共单点退役（`worktree_root_from_toplevel` 纯派生保留供单测） |
| `crates/infra/vcs/src/git.rs` | git 工作面进程执行 | `placement(root)`：一次 `rev-parse --show-toplevel --show-prefix` 解析两行（根 workspace 第二行为空）；`tracked_at_head(root, rel)`：`rev-parse --verify --quiet HEAD:<rel>`（空 `rel` 零 spawn）；`restore_baseline` / `worktree_snapshot` / `remove_worktree` / `commit_all` / `dirty` / `rebase` / `ff_merge` 命令体逐字不变 |
| `crates/core/workflow/src/write/create.rs` | create 建域组合 | 用 `placement` 取 host 与 `rel`；前置新增 `tracked_at_head` 判据（先于建档与 `add_worktree`）；建档双写 `worktree` / `exec_root`；`add_worktree` 目标恒 host；后置 `exec_root.is_dir()` 断言（miss ⇒ 既有 `compensate_tree_failure` + `Err`）；`resolve(&exec_root)` 与 `bootstrap(&exec_root, …)`；`bootstrap_worktree_root` → `bootstrap_exec_root`；`BootstrapProject.root` 文档与警告文案改「相对执行根」；`CreateOutcome` 增 `execRoot: Option<String>` |
| `crates/core/workflow/src/write/promote.rs` | promote 透传 | 逐字不变（项目入参与 exec root 由 `create` 承担） |
| `crates/core/workflow/src/write/delete.rs` | 删除执行序 | 逐字不变；语义随动：「同时清空 worktree」的回收锚 = 记录 `worktree`（host），MUST NOT 用执行根（传执行根会只剩「删子目录 + 留登记」） |
| `crates/core/workflow/src/state.rs`（`ChangeStateRecord`） | 记录中性快照 | 增 `exec_root: Option<String>`；`fn exec_root(&self) -> Option<&str>` 访问器单点（`None` 回落 `worktree`；非 host 真后代 ⇒ `eprintln!("[workflow] …")` 留痕 + 回落 `worktree`）；`create_change_record` 写入面同时携两列 |
| `crates/core/workflow/src/queries/detail.rs` | 详情读面 | `locate_change` 回退参改 `record.exec_root()`；`ChangeDetail` 增 `execRoot: Option<String>`（`worktree` 字段原样保留） |
| `crates/infra/store/src/model.rs`（`ChangeRecord`） | 模型版本 | v4 → v5：现役形态加 `#[serde(default)] pub exec_root: Option<String>`，历史形态抄为 `ChangeRecordV4`（`from = ChangeRecordV3`）；`ChangeRecord::new` 增 `exec_root` 入参；`WORKSPACE_STORE_FORMAT_VERSION` 零改动 |
| `crates/infra/store/src/store.rs` | 映射单点 | `change_state()` 与 `create_change_record` 补新列；其余写路径（读记录 → 改字段 → upsert）零改 |
| `src/commands/changes/mod.rs` | create / detail / artifact 命令面 | `read_artifact` 的 `locate_change` 回退参改执行根；`change_dirs` 候选改执行根（`push_worktree_candidate` 复用、探测序不变）；`CreateOutcome` 透传 `execRoot` |
| `src/commands/archi/mod.rs` | archi 改动侧解析 | `archi_change_summary` 的改动侧改 `record.exec_root()`（archived 仍恒 `None`，与归档 change 会话 cwd 回落同口径） |
| `src/commands/change_flow/mod.rs` | exec root 解析 | 前置校验 3 改 `record.exec_root()`（`is_dir` 校验与文案沿用）；`RunRequest.worktree`（host）透传 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 前端可见性 | 信息面两行（`worktree` + 条件 `执行根`）；画布 root 与 `ChangeChatPanel.cwd` 取 `execRoot ?? worktree` |
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | 会话 cwd 派生 | `cwd = archived ? null : (execRoot ?? worktree)` |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 新图预览根 | `preflight.execRoot ?? preflight.worktree ?? root` |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | create 成功面 | 落位行 + 条件执行根行（`execRoot = null` 时零占位） |
| `packages/desktop/package.json` | 版本交付 | 落地时 `HEAD` 现值 + 1 patch；`src-tauri/Cargo.toml` 不随动 |
