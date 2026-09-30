# desktop-workspace-store 变更（desktop-workspace-db-split）

## ADDED Requirements

### Requirement: 全局库与 workspace 库双库布局

store SHALL 按数据维度拆分落盘载体为两组 db 文件：**全局库**仅注册 `WorkspaceRecord`（user 维度注册表，含未来 user 维度租户）；**workspace 库**按 workspace 分立、每 workspace 恰一个独立 db 文件，注册 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三个 workspace 维度模型。全局库 SHALL 位于全局数据目录（沿用 `home_dir()/.dev-team/` 根）并启用与旧单库不同的新文件名；workspace 库 SHALL 位于全局数据目录下 `workspaces/` 子树。任一库的模型归属 MUST NOT 混注：workspace 域模型 MUST NOT 出现在全局库注册清单，user 维度模型 MUST NOT 出现在 workspace 库注册清单（维度语义由 db 文件归属承载，见 desktop-data-dimensions）。数据目录根 SHALL 由 desktop-app 注入，store 内零环境解析（既有 `Store::open` 注入式打开约束不变）；静态模型注册 SHALL 拆为与两库一一对应的两组 `Models`。

#### Scenario: 模型注册分组

- **WHEN** 审查 store 的静态模型注册与两组 `Models` 的消费方
- **THEN** 全局库组仅定义 `WorkspaceRecord`，workspace 库组仅定义 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`，无交叉注册

#### Scenario: 数据分流写入

- **WHEN** `add_workspace` 注册目录后，对该 workspace 执行 `create_explore_record` 与 `begin_agent_run`
- **THEN** 注册记录落在全局库，explore 记录与 run 记录落在该 workspace 库，全局库无 workspace 域记录混入

#### Scenario: 注入式打开不变

- **WHEN** 审查 store 公共 API 与打开流程
- **THEN** 两库路径最终均来自调用方注入（数据目录根由 desktop-app 解析），store 源码无 home_dir / app data 目录解析逻辑

### Requirement: workspace 库文件确定性寻址

workspace 库文件路径 SHALL 由 canonical root 经**单点纯函数**确定性派生：同根恒同名（跨重启、跨进程可复现），异根必不同名（哈希成分抗碰撞），文件名 MUST NOT 含路径分隔符或 OS 非法字符。派生函数 SHALL 是 workspace 库文件名的唯一组装点，消费侧（desktop-app 命令层、前端）MUST NOT 出现派生逻辑或路径字符串拼接。哈希算法与文件名组成（可读目录段 + 哈希长度）由 dev-design 定夺；派生入参 SHALL 为已 canonical 化的 root 与注入的数据目录根，复用 store 既有 canonical 口径（UNC 前缀、大小写归一策略同源）。

#### Scenario: 同根跨重开同路径

- **WHEN** 对同一 canonical root 先后两次（重启之间）调用路径派生
- **THEN** 两次返回完全相同的文件路径；对两个仅大小写或尾部分隔符不同的等价路径派生，结果亦一致（canonical 口径归一在前）

#### Scenario: 异根不同名

- **WHEN** 对两个不同 workspace root 调用路径派生
- **THEN** 得到不同文件路径，不存在碰撞覆盖

### Requirement: workspace store 注册表与进程内单开

store SHALL 提供两级解析入口：全局注册操作（`add_workspace` / `list_workspaces` / `remove_workspace` 等）走全局库实例；workspace 域操作（run / 事件 / explore 记录面）经 `for_root(root)` 解析对应 workspace 库实例。同一 workspace db 文件在进程内 MUST NOT 重复打开（redb/native_db 单文件写锁硬约束）：注册表 SHALL 按 root 缓存并复用已打开实例；不同 root 各自独立实例。`Store` 类型与 `Store::open(path)` 单文件句柄语义不变；注册表类型（挂 Tauri State）SHALL 由 store crate 提供、desktop-app 挂载。`add_workspace` SHALL 在注册成功后预开对应 workspace 库（坏文件注册时即报错，fail fast 口径同 setup，不留给首个业务命令）。`remove_workspace` SHALL 仅删注册记录，workspace db 文件 MUST NOT 随之删除或改名（移除注册 ≠ 销毁历史；重新添加同 root 时清单与会话链历史完整恢复）；孤儿文件治理留待二期。缓存淘汰策略（进程生命周期常开 vs LRU）由 dev-design 定夺，默认进程生命周期常开。

#### Scenario: 同 root 实例复用

- **WHEN** 对同一 workspace root 连续多次发起 workspace 域命令
- **THEN** 均复用同一打开实例，不产生对同一 db 文件的第二次打开（无文件锁冲突）

#### Scenario: 异 root 独立实例

- **WHEN** 依次对 workspace A 与 B 发起 workspace 域命令
- **THEN** 两库各自独立打开、数据互不可见（A 的 runs / explore 清单不出现在 B 的任何查询结果中）

#### Scenario: 注册时预开校验

- **WHEN** `add_workspace` 选定一个 workspace 库文件已损坏（不可打开）的目录
- **THEN** 注册返回 `Err`，损坏文件在注册时暴露；健康目录注册成功且库文件就绪

#### Scenario: remove 后文件保留

- **WHEN** `remove_workspace` 移除某注册项后重新 `add_workspace` 同一根目录
- **THEN** remove 后 workspace db 文件仍存在于原位；重新添加后 explore 清单与会话链历史完整可读

### Requirement: 全新文件组冷启动与旧库惰性废弃

存量单库（四模型混居的旧 `desktop-store.redb`）SHALL **零迁移**处置（用户裁定，2026-09-29：无需考虑兼容历史数据）：新布局以全新文件组冷启动，应用启动与运行 MUST NOT 读取旧单库文件，MUST NOT 对其改名、删除或做任何格式探测；store MUST NOT 引入拆分搬移、格式探测或迁移层代码（与既有「不做旧库兼容」立场同轨）。旧文件作为磁盘惰性残留保持原样；新布局首启后全局库与 workspace 库均从空开始写入（workspace 清单从空开始，由用户重新添加目录重建）。

#### Scenario: 旧库存在时冷启动

- **WHEN** 全局数据目录下存在旧四模型单库文件，以新布局启动应用
- **THEN** 启动照常成功，旧文件字节原样未动（未读、未改名、未删除），新库文件从空开始按新布局写入

#### Scenario: 零迁移代码

- **WHEN** 审查 store 源码
- **THEN** 无拆分搬移、格式探测、迁移模块或旧格式读取代码路径

### Requirement: workspace 域命令 root 寻址解析入口

workspace 域命令（run / 事件 / explore 记录面）SHALL 携 root 寻址：命令层以 root 调 `for_root` 解析所属 workspace 库后调用 store 操作，store 操作面签名不变（实例即上下文）。blank root 纪律不变（空/空白 root 不进入解析与 store 链路，直接空结果语义）。全局轨命令（workspace 注册三命令）SHALL 经全局库实例执行，MUST NOT 误路由至 workspace 库。db 查看命令的 scope 寻址见 desktop-db-inspector 能力。

#### Scenario: 命令路由正确

- **WHEN** 分别以 workspace A 与 B 的 root 调用 `list_explore_records` 与 `agent_runs`
- **THEN** 各自返回对应 workspace 库的数据，路由无串库

#### Scenario: blank root 直达空结果

- **WHEN** 以空白 root 调用任一 workspace 域命令
- **THEN** 不触发库解析与打开，直接返回空结果语义（既有口径不变）

## MODIFIED Requirements

### Requirement: ExploreRecord 模型与 explore 清单操作面

store SHALL 以 `ExploreRecord` 模型（native_model id / version 不变）承载 explore 清单条目与 agent 会话链绑定。字段面 SHALL 至少含：独立主键 `id`（写事务内 max+1 分配，与 `AgentRunRecord` 同语义——身份与文件名解耦，文件改名不破绑定）、`root`（workspace 归属，canonical 口径与 `WorkspaceRecord` 一致）、展示名 `name`、`created_at` / `updated_at`（UTC unix 毫秒，同既有时间戳口径）；链锚与磁盘路径关联字段（`head_run_id` 双写 vs `(source, source_ref)` 派生；路径字段 vs 名字派生）由 dev-design 定夺。维度声明：`ExploreRecord` 归 **workspace 维度**（workspace 数据分离裁定，落所属 workspace 的独立 db 文件，见 desktop-data-dimensions），内容唯一真源在磁盘 `explore.md`（workspace repo），数据三分——记录（workspace 库）/ 内容（磁盘）/ 对话（同 workspace 库的 `AgentRunRecord` + 事件）。

store SHALL 提供 explore 清单操作面：`list_explore_records(root)`（按 root 过滤）、`create_explore_record`、`delete_explore_record`（删记录 MUST NOT 触碰磁盘文件；记录名下的会话 runs 及其事件 SHALL 随记录**同事务级联删除**——按 `(source="explore", source_ref=记录id)` 圈定。id 为幸存行上 max+1 的可复用计数，级联清理是悬空 `source_ref` 不被复用 id 错挂的引用完整性保证；MUST NOT 波及 debug 来源或其他 `source_ref` 的 runs）。孤儿语义 SHALL 为：磁盘文件被删记录保留（不自动清理，预览空态、下次落盘重建——文件是记录的可丢弃投影）；新模型 SHALL 随既有记录信封 API（`list_models` / `scan`）零改动可浏览。级联删除与链还原 SHALL 天然收敛于同一 workspace 库实例内（记录与名下 runs / 事件同库，无跨库引用）。

#### Scenario: 清单按 root 过滤

- **WHEN** store 中存在归属 workspace A 与 B 的 `ExploreRecord`，以 A 的 root 调用 `list_explore_records`
- **THEN** 仅返回归属 A 的记录，两次调用顺序稳定

#### Scenario: 身份与文件名解耦

- **WHEN** 某 `ExploreRecord` 绑定的文件被改名为 `new-name.md`
- **THEN** 记录主键不变、会话链绑定不破；记录更新文件关联后重新可读

#### Scenario: 孤儿保留

- **WHEN** 某 `ExploreRecord` 绑定的磁盘文件被外部删除
- **THEN** 记录仍在清单中；经 `delete_explore_record` 删除记录后磁盘不受影响（文件本已不存在则无操作）

#### Scenario: 删除级联清对话且 id 复用不错链

- **WHEN** 某 `ExploreRecord`（id=N）名下已有 `source="explore"`、`source_ref="N"` 的 runs 及事件，执行 `delete_explore_record` 后再新建记录（幸存行 max+1 复用 id=N）
- **THEN** 被删记录名下 runs 与事件已同事务消失（`(source, source_ref)` 链还原为空），新记录链还原为空不捞旧聊天；debug 来源及指向其他记录 id 的 runs 及其事件原样存活

#### Scenario: 信封 API 零改动覆盖

- **WHEN** 新增 `ExploreRecord` 注册后调用 `list_models` / `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与 DB 查看器代码无改动

#### Scenario: workspace 库落位隔离

- **WHEN** 审查 explore 记录的落盘位置
- **THEN** 记录与名下会话 runs / 事件位于所属 workspace 的独立 db 文件，全局库与 workspace repo 内无 explore 记录数据

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store`（crate 名 `store`） | native_db 本地库，db 边界第一成员 | 双库布局：全局库（`WorkspaceRecord`）+ `workspaces/` 子树 per-workspace 库（`AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`）；`Store::open(path)` 注入式打开不变；数据目录根注入、零环境解析；零迁移（旧单库惰性废弃） |
| 静态模型注册（拆两组） | 两库各自的 `Models` | 全局组仅 `WorkspaceRecord`；workspace 组仅 run / 事件 / explore 三模型；模型 id / version / 字段面全部不变 |
| workspace 库路径派生（新，单点） | canonical root → db 文件路径 | 纯函数、确定性、抗碰撞、无路径成分；store 内唯一组装点，消费侧零派生逻辑 |
| `WorkspaceStores`（新） | 两级解析与实例缓存 | `global()` → 全局库；`for_root(root)` → workspace 库（per-root 缓存复用，进程内单开）；挂 Tauri State；`add_workspace` 预开校验；`remove_workspace` 文件保留 |
| `WorkspaceRecord`（模型） | workspace 注册记录（user 维度） | PK = canonical root；root / name / added_at；落全局库 |
| `ExploreRecord`（模型） | explore 清单与绑定记录（workspace 维度） | 落所属 workspace 库；PK 独立自增 id；级联删除与链还原同库收敛；孤儿保留；其余语义不变 |
| `AgentRunRecord` / `AgentEventRecord`（模型） | agent 运行历史（workspace 维度） | 落所属 workspace 库；run id 为库域内自增；字段面与 id / version 不变 |
| 记录信封 API | 模型层通用读面 | `list_models()` / `scan(model, offset, limit)` 按实例生效（每库各自清单与计数）；只读；新模型注册即覆盖 |
