# desktop-ipc-type-bindings Specification

## Purpose

定义 desktop-app IPC 面的类型绑定契约：tauri-specta 命令注册与 TypeScript 生成物、生成物入库与重导出、前端 typed 调用面切换、dto.ts shim 过渡与判据驱动退役、依赖落位与 knip 边界。Rust 类型为单一事实源,前端调用面 MUST NOT 维护手写镜像。

## Requirements

### Requirement: tauri-specta 接入与命令绑定生成

desktop-app SHALL 以 tauri-specta builder 注册全部 IPC 命令（既有 `main.rs` `generate_handler!` 全量迁移；命令清单以 `all_commands!` 单一登记面为准——bin 组装与绑定生成同源，MUST NOT 在文档 / 测试中硬编码命令条数），并生成 TypeScript 类型与 typed invoke 绑定：命令名、参数 key 的 camelCase 形状、返回类型 SHALL 全部受编译期校验，前端 MUST NOT 再以裸 `invoke('command_name', ...)` 字符串调用命令（生成物内部与 `@tauri-apps/api` 底层调用除外）。流式命令 SHALL 一并覆盖：`agent_start`、`agent_session_watch` 与 `watch_subscribe` 的 `Channel<...>` 参数 SHALL 被 typed（specta 对 `tauri::ipc::Channel` 的支持面）；应用无 `emit` / `listen` 事件，specta events 面 MUST NOT 引入。Rust 出线类型（core/agent 的 `AgentEvent` / `AgentEventKind` / `AgentBlock`、core/workflow 的 `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry`、store 模型、app 层 `AgentRunMessage` / `FileWatchEvent`）SHALL 全部加 `specta::Type` derive 且类型语义 MUST NOT 因此改变。

- **清单守卫随单 change 随动**：`bindings/mod_test.rs` 的命令名清单、包装名清单、出线 DTO 清单与分族清单（如 archi 族的命令 / 出线类型两组）SHALL 在新增或删除命令 / 出线类型的**同一 change 内**随动更新；这些清单的长度断言 SHALL 只作「清单被同步过」的连带核对，MUST NOT 被当作语义判据（真实判据是清单条目名与生成物逐名命中）。尾部追加序断言 SHALL 同步保持（新条目按登记顺序插入，既有条目零位移）。本变更实测随动面 = 命令 51 → 53、出线 DTO 101 → 104、`ARCHI_COMMANDS` 6 → 8、`ARCHI_DTO_TYPES` 23 → 26，另有两处**非计数**断言随动：尾部追加序期望 vec 增加两条、`ARCHI_COMMANDS` 末条对照臂由 `archi_file_tree` 变 `archi_api_generate`。
- **分族清单 SHALL 覆盖全族**：某一命令族的分族清单 MUST NOT 漏登该族既有命令——漏登会让「新增一条」的 diff 看起来比实际小，从而把「补齐既有漏项 + 新增本次条目」两步混淆成一步（本次 archi 族即为此情形：现状 5 条命令而清单只列 4 条，缺 `archi_drift`）。

接入 SHALL 以 PoC 先行：实现第一步 MUST 验证 rc.25 对 `AgentEvent`（`tag="kind"` 内部标签 + `rename_all_fields = "camelCase"`）与 `Channel<AgentRunMessage>` 信封的实际出线形态，结论留档于 design；不符预期时 SHALL 回退备选方案（ts-rs 只生成类型 + 手写薄 invoke 层）并重新评审。

#### Scenario: builder 注册覆盖全部命令

- **WHEN** 审查 specta builder 组装点与生成的 bindings
- **THEN** `all_commands!` 登记面内全部命令（含本次新增 `archi_api_check` / `archi_api_generate`）均经 builder 注册且在 bindings 中有对应 typed 包装；命令清单与 `main.rs` 迁移前一致（新增项显式计入）

#### Scenario: 流式 Channel 被 typed

- **WHEN** 审查 `agent_start`、`agent_session_watch` 与 `watch_subscribe` 的生成绑定签名
- **THEN** `onEvent: Channel<AgentRunMessage>` / `Channel<FileWatchEvent>` 参数在 TS 侧为 typed Channel，消息信封（tag `ipc` / watch 载荷）类型来自生成物；订阅命令的 `sinceSeq` 为 typed 数值参数

#### Scenario: 类型漂移编译期拦截

- **WHEN** Rust 侧改名字段或改命令参数形状而不更新生成物
- **THEN** 重导出把生成物刷新为最新；若生成物自检失败（`bindings:zod-check` 非零）检查即失败，否则前端消费处的编译错误由 tsc 拦截，运行期不再首见漂移

#### Scenario: PoC 先行留档

- **WHEN** 审查 design 与实现顺序
- **THEN** `AgentEvent` 内部标签枚举与 `Channel<AgentRunMessage>` 的出线形态验证记录在案，且先于前端切换任务执行

#### Scenario: 新增命令与出线类型时清单同 change 随动

- **WHEN** 新增两条命令（`archi_api_check` / `archi_api_generate`）与三个出线类型（`ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome`）
- **THEN** 命令名清单与包装名清单条目数各增二（且两者持衡）、出线 DTO 清单条目数增三、两组 archi 分族清单同增；`bindings:export` 后生成物含两条命令的 typed 包装（参数键 lowerCamelCase 与 invoke payload 逐字一致）与三个 `export type`，`bindings.zod.ts` 的 `argsSchemas` / `resultSchemas` 各含对应表项；尾部追加序断言把新命令按登记序插在 `archi_file_tree` 之后、`read_file` 之前（`read_file` 仍为尾条）、既有条目零位移

#### Scenario: 分族清单漏登既有条目即补

- **WHEN** 审查任一命令族的分族清单与其 `all_commands!` 段
- **THEN** 两者条目名集合相等（无漏登、无多余）；若发现漏登（如 archi 族清单只有 4 条而该段实有 5 条），该 change SHALL 一并补齐并在同一条断言里体现「补齐 + 新增」两段变化，MUST NOT 只加新条目让清单继续缺项

### Requirement: 生成物入库与重导出

生成的 bindings 产物（`bindings.ts` 与 `bindings.zod.ts` 两份）SHALL 全部入库版本控制，同落位 `src/types/generated/`（既有实现已定，MUST NOT 迁址；第二产物 MUST 与类型产物同目录、同一次导出产出）。Rust 侧 SHALL 提供唯一导出入口执行两次 `builder.export`（同一 builder、同一 `all_commands!` 登记面：`Typescript` 一次 + zod exporter 一次；形态为搭 `server:test = cargo test --workspace` 顺风车的 `#[test]` 或专用 bin，design 定夺）。

`check` 与 `build` SHALL 前置重导出（npm `pre` hook 或显式脚本串接）——重导出本身即把盘上产物修至最新，故两步之后 MUST NOT 再以「工作区与 HEAD 的 diff」判失败：change 流程的 static-check 跑在 change 私有 worktree（天生有未提交改动），该判据恒假且不携带「产物是否过期」的信息，只会把每一轮改过命令 / DTO 的首轮检查打成假红。入库物是否过期 SHALL 由人工在提交前经 `git status` / `git diff` 核对生成目录确认，MUST NOT 依赖自动化 diff 门禁。导出 SHALL 幂等（同输入同输出，重导出无 diff；两份产物各自幂等，且两次导出之间 MUST NOT 相互影响）。

#### Scenario: 重导出幂等

- **WHEN** 连续两次执行导出入口而不改任何 Rust 类型
- **THEN** 第二次导出后 `git diff` 为空，无格式抖动

#### Scenario: 脏工作区内的检查不被生成物误伤

- **WHEN** 在 change worktree 内修改 Rust 类型（命令签名 / 出线 DTO）后执行静态检查
- **THEN** 前置重导出把 `src/types/generated/` 更新为最新，检查不再因「工作区相对 HEAD 有 diff」而失败

#### Scenario: 生成物过期由提交前人工核对

- **WHEN** 修改 Rust 类型后仅提交源码、未提交重导后的生成物
- **THEN** 该遗漏由提交前的 `git status` / `git diff` 人工核对发现；`check` MUST NOT 以自动 diff 守卫冒充这一判定

#### Scenario: 新命令绑定同步

- **WHEN** 新增会话订阅命令 `agent_session_watch` 后执行 `pnpm check`
- **THEN** 前置重导出使生成物含该命令的 typed 包装（`onEvent: Channel<AgentRunMessage>` + `sinceSeq`），生成目录的改动在 `git status` 中可见

#### Scenario: 第二产物同随重导出

- **WHEN** 修改 Rust 侧出线 DTO 后执行 `pnpm check`
- **THEN** 同一次导出同时更新 `bindings.ts` 与 `bindings.zod.ts`（覆盖整个生成目录，MUST NOT 只更新类型产物）

#### Scenario: 第二产物幂等

- **WHEN** 连续两次执行导出入口且不改任何 Rust 类型
- **THEN** 两份产物均零 diff；导出顺序（`bindings.ts` 先、`bindings.zod.ts` 后）MUST NOT 改变任一产物的字节

### Requirement: 产物互斥的跨进程锁

多个导出入口用例会独占改写同一份共享产物（`src/types/generated/`），测试框架为 nextest（每个用例一个**独立进程**）时进程内互斥完全失效，故互斥 SHALL 由 **OS 文件锁**（`std::fs::File::try_lock` 轮询 + 句柄关闭释放）实现：以读写方式打开常驻锁文件 `target/bindings-test.lock`、非阻塞重试至多 [`LOCK_WAIT_TIMEOUT`]，`Drop` 仅关闭句柄 MUST NOT 删除锁文件。

MUST NOT 再用「`create_new` 独占创建 + `Drop` 删文件 + mtime 老化夺取」形态：Windows 上文件删除后、句柄关闭前处于 delete-pending，另一进程 `create_new` 得到 `ERROR_ACCESS_DENIED(5)` 而非 `ERROR_FILE_EXISTS(80)`（现网实测 `创建产物文件锁失败（…bindings-test.lock）: 拒绝访问。 (os error 5)`）；且持锁期间 `remove_file` 在 `FILE_SHARE_DELETE` 下会成功，「陈旧锁夺取」可让两进程各持不同 inode 的锁而互不互斥地双写产物。OS 锁随句柄关闭自动释放，进程崩溃不留残留，无需陈旧判定与夺取。

#### Scenario: 持锁期间第二句柄被拒

- **WHEN** 一个用例持有产物锁，同进程第二个句柄（或另一 nextest 进程）尝试取锁
- **THEN** 得到 `TryLockError::WouldBlock` 并轮询等待，MUST NOT 以 `os error 5` 之类的 IO 错误直接 panic

#### Scenario: 释放后立即可重取且文件常驻

- **WHEN** 守卫 `Drop` 后同一进程再次取锁
- **THEN** 无需等待即可取到（锁随句柄关闭释放），且 `target/bindings-test.lock` 仍存在（MUST NOT 删除）

### Requirement: 前端一次性切换 typed bindings

前端 SHALL 在本变更内一次性完成切换：`src/` 全部 invoke 调用点（约 27 个文件）从裸字符串 invoke 改为 typed bindings，MUST NOT 分批（双轨共存期 MUST 为 0）。`agent-transport.ts` 手写的 `AgentRunMessage` / `AgentStartChainParams` 镜像 SHALL 收编进生成物并删除手写副本；`readChainParams` 等运行时校验 SHALL 保留（ai-sdk body 穿透所需，与静态类型互补，非重复防线）。切换的真实风险（生成物与手写镜像的字段级差异，如 null vs optional）由 tsc 全量类型检查拦截，替换本身为机械操作。

#### Scenario: 裸 invoke 清零

- **WHEN** 以命令名字符串模式扫描 `packages/desktop/src`（排除生成物文件与测试内对生成物的断言）
- **THEN** 无业务代码直接调用 `invoke('command_name', ...)`，全部经生成绑定；`agent-transport.ts` 内无手写 IPC 镜像类型

#### Scenario: 运行时校验保留

- **WHEN** 审查切换后的 `agent-transport.ts`
- **THEN** `readChainParams` / `readPermissionMode` 等运行时校验仍在，body 穿透路径的清单外值拒绝语义不变

#### Scenario: 一次性切换可整体回滚

- **WHEN** 切换后需要回退
- **THEN** 单 change 整体 revert 即回到裸 invoke + 手写镜像形态，无半 typed 半裸字符串的中间态残留

### Requirement: dto.ts shim 过渡与判据驱动退役

`dto.ts` SHALL 收敛为纯 re-export shim（`export type * from` 生成物；生成名与现名有出入时经逐名 re-export 做名称映射），使一次性切换的漏网旧 import 不炸编译/运行时，并由 knip unused 报告兜出漏网名单。shim MUST NOT 承载任何手写类型定义（dto.ts 全量为 Rust 镜像、无前端独有类型，终局必删）。退役判据 SHALL 为 knip 报告 shim 零引用即删除，MUST NOT 按时间过渡。

#### Scenario: shim 纯 re-export

- **WHEN** 审查 shim 化后的 `dto.ts`
- **THEN** 文件仅含 re-export 语句（含必要的逐名映射），无任何手写 interface / type 定义

#### Scenario: 漏网 import 不炸

- **WHEN** 切换后存在未被机械替换覆盖的旧 `from '../types/dto'` import
- **THEN** 编译经 shim re-export 通过，knip unused 报告列出该漏网引用供收口

#### Scenario: 零引用退役

- **WHEN** knip 报告 dto.ts（shim）零引用
- **THEN** 删除该文件且全量检查仍绿；退役不以日历时间为准

### Requirement: 依赖落位与 knip 边界

`tauri-specta` / `specta-typescript` / `specta` 三件套 SHALL 以 `=` 精确 pin 并 lockstep 升级（RC 间无 semver 兼容），workspace.dependencies 收敛；`specta-serde` SHALL 与 `specta-typescript` 同版本同 `=` pin（其 `PhasesFormat` 是 zod 侧复用的 serde 相位语义层，版本错配会让两份产物的相位规则分叉）。依赖落位 SHALL 为：`tauri-specta` 与 `specta-typescript` 仅落 desktop-app（Tauri 系依赖不出壳层）；`specta` derive SHALL 准入 core/infra（specta 为普通类型导出 derive crate，非 Tauri 系，与 serde 先例同列，「core/infra 禁 Tauri」约束不破，crate 依赖方向规则全部不变）；zod exporter SHALL 以 vendored path 依赖（`crates/tools/specta-zod-patched`）仅落 desktop-app，MUST NOT 出现在 core/infra。生成物目录 SHALL 配置 knip ignore（生成物非手写代码，未引用的命令包装 MUST NOT 报 dead code；第二产物同受该目录 ignore 覆盖，MUST NOT 为它另开 ignore 条目）。本条只约束 Rust 侧 specta 系与 vendored exporter 的落位；前端**运行期** `zod` 依赖的声明形态、精确 pin、单实例与 knip 可见性归 `desktop-ipc-runtime-guard` 的「前端运行期 zod 依赖」requirement（前端消费面），MUST NOT 在本条内二次约束（两处约束即两处漂移面）。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查各 crate 的 `Cargo.toml`
- **THEN** tauri-specta / specta-typescript 仅出现在 desktop-app 依赖；core/agent、core/workflow、infra/store 仅有 specta（非 Tauri 系）且依赖方向既有规则不变

#### Scenario: 三件套精确 pin

- **WHEN** 审查 workspace.dependencies 中三件套版本
- **THEN** `specta` / `specta-typescript` / `tauri-specta` 均为 `=` 精确 pin 且版本 lockstep

#### Scenario: 四件套与 vendored exporter 锁定同一窗口

- **WHEN** 审查 `specta` / `tauri-specta` / `specta-typescript` / `specta-serde` 的 pin 与 vendored `specta-zod` 的基线版本
- **THEN** 四者 `=` 精确 pin 且相互兼容（同一 specta RC 窗口），vendored exporter 的 `Cargo.toml` 内 pin 与该窗口一致；`cargo tree` 中不存在两个 specta RC 版本并存

#### Scenario: knip 不报生成物

- **WHEN** 执行 `client:check`（含 knip）
- **THEN** 生成目录在 knip.json ignore 内，生成物未引用导出不报 dead code，检查通过

### Requirement: 第二产物职责切分（纯 schema，零 `export type`）

生成目录 SHALL 承载两份职责互斥的产物：`bindings.ts` 为**唯一类型源**（typed invoke 包装 + 全部出线 DTO 类型 + JSDoc 说明，形态与既有导出零变更），`bindings.zod.ts` 为**纯 schema**（具名类型 schema + 逐命令 args / result 表 + channel 参数元数据）。第二产物 MUST NOT 输出任何 `export type` / 相位伴生别名，MUST NOT 承担命令包装渲染；前端 MUST NOT 从第二产物取类型（禁 `z.infer` 作业务类型），类型一律出自 `bindings.ts`。

理由（结构性，不是偏好）：zod exporter 只吃 `&Types`、看不到 `BuilderConfiguration`，typed invoke 包装 / `Channel<…>` 参数 / 错误面变换全部活在 `cfg.commands` 里，只有 tauri-specta 能渲染；反之 `z.infer` 在递归类型上会塌成 `any` 并静默传播。故两份产物不可互换、不可一份替代另一份。

#### Scenario: 第二产物零类型别名

- **WHEN** 对 `src/types/generated/bindings.zod.ts` grep `export type` 与 `z.infer`
- **THEN** 零命中（无类型别名、无相位伴生别名、无 `z.infer` 转发）

#### Scenario: 类型主权单向

- **WHEN** 审查前端业务代码的 IPC 类型来源
- **THEN** 全部类型 import 自 `src/types/generated/bindings.ts`（或其 shim），无一处以 `z.infer<…>` / 第二产物作为业务类型声明

#### Scenario: 命令包装只在类型产物

- **WHEN** 审查两份产物的内容面
- **THEN** typed invoke 包装（含 `Channel` 参数映射与错误面变换）只出现在 `bindings.ts`；第二产物只有 schema 与命令表

### Requirement: 生成期 tsc 自检与降级清单

导出链 SHALL 在产物落地后对「只 import `bindings.zod.ts` 的桩文件」执行一次 `tsc --noEmit` 自检（单文件、零运行期依赖），一条自检同时看住三件事：struct 面 strict 化、递归引用 getter 化（无 `z.lazy` 自引用初值）、第二产物无类型别名。自检失败 MUST 以非零退出阻断 `bindings:export`（产物不可用 MUST NOT 被当作「已导出」入库）。

导出期 SHALL 收集降级清单（哪些类型 / 参数位落到了 `degraded("原因")`）并在 stderr 输出；降级 MUST NOT 静默发生。

#### Scenario: 自检挡住不合法产物

- **WHEN** 产物出现 `z.lazy` 自引用（未走 getter）或 struct 面未 strict 化
- **THEN** 自检桩文件 `tsc --noEmit` 报错，`bindings:export` 以非零退出，产物不入库

#### Scenario: 降级清单可见

- **WHEN** 导出过程中有类型被降级为 `degraded("原因")`
- **THEN** stderr 列出该清单（类型名 / 命令参数位 + 原因），退出码仍为 0（降级是可接受收口，不是失败）

#### Scenario: 自检不引入运行期依赖

- **WHEN** 审查自检桩文件
- **THEN** 它只 import 生成产物、只做类型层使用（无 DOM、无 Tauri、无网络），可独立 `tsc` 执行

### Requirement: vendored exporter 落位与跟版

zod exporter SHALL 以 vendored crate 落位 `packages/desktop/src-tauri/crates/tools/specta-zod-patched`（workspace 成员，与 `export-bindings` 同层；MUST NOT 落 core/infra，MUST NOT 引入 Tauri 依赖到 core/infra）。每处本地改动 MUST 带可 grep 的标记（如 `PATCH(n)`）并在 crate README 记录：基线版本、每个补丁的位置与理由、跟版核对清单。上游合入对应能力后，MUST 删除 vendor 改回 registry 依赖（判据是上游版本覆盖补丁语义，MUST NOT 按时间过渡）。

#### Scenario: 补丁可机器清点

- **WHEN** 对 vendored crate 源码 grep 补丁标记
- **THEN** 命中数与 README 记录的补丁条数一致，每条标记行紧邻改动说明

#### Scenario: 落位与依赖方向

- **WHEN** 检查 workspace members 与各 crate 依赖
- **THEN** vendored crate 位于 `crates/tools/` 且仅被 desktop-app 依赖；core / infra 任一 crate 的依赖中不出现它

#### Scenario: 上游合入即删 vendor

- **WHEN** 上游发布覆盖现有补丁语义的版本
- **THEN** 改为 registry 依赖并删除 `crates/tools/specta-zod-patched`，产物零 diff（判据驱动，不以日历时间为准）

### Requirement: 实时输出三命令与出线类型入册

脚本步运行中 stdout 的取数面 SHALL 以三条新命令入 `all_commands!` 单一登记面（与 builder 注册同源），使 `bindings.ts` 与 `bindings.zod.ts` 双产物自动生成 typed 包装与 schema（前端 `ipc` 代理层零改动即覆盖）：

| 命令 | 签名 | 出线类型 |
|---|---|---|
| `exec_output_path` | `(root: String, change_name: String, exec_id: String) -> Option<String>` | — |
| `read_exec_command` | `(root: String, change_name: String, exec_id: String) -> Option<String>` | — |
| `read_exec_output_delta` | `(root: String, change_name: String, exec_id: String, from_offset: u64) -> Option<ExecOutputChunk>` | `ExecOutputChunk` |

- **出线 DTO 落位**：`ExecOutputChunk` SHALL 定义在壳层命令模块（`src/commands/changes/`），携 `serde`（`rename_all = "camelCase"`）+ `specta::Type`，字段为 `text: String` / `next_offset: u64` → TS `{ text: string; nextOffset: number }`。它 MUST NOT 定义在 `crates/infra/exec`（该 crate 的依赖面 MUST NOT 因出线需要而扩 serde / specta；exec 侧返回的 `ExecOutputDelta` 为**零 derive 纯结构**，命令层做 1:1 映射）。
- **`u64` 出线口径**：`from_offset` 与 `nextOffset` 沿用 workspace 既有 bigint 配置（`dangerously_cast_bigints_to_number`）出线为 `number`，MUST NOT 单独引入 `BigInt` 面（与 `tokenInput` / `tokenOutput` 同口径；文件长度上界受 2^53 约束，实际日志远小于此）。
- **miss 语义与既有读命令一致**：三命令在 blank root、`exec_id` 非法 / 未知、或两槽位均无该记录时 SHALL 返回 `None`（miss 非错误、非 `Err`），MUST NOT 借故障通道表达「暂时还没输出」——运行中零字节由 `Some { text: "", nextOffset: 0 }` 表达（与「无记录」可分辨）。
- **零新事件面**：三命令均为同步返回，MUST NOT 引入 `emit` / `listen` 事件或新 `Channel` 参数。
- **退役命令**：`read_exec_output` 已退役（零消费者死命令，整读改走 `read_file`），其生成绑定条目 MUST NOT 继续出现在 `bindings.ts` / `bindings.zod.ts`；其余既有读取命令（`read_exec_command` / `read_exec_output_delta` / `read_artifact`）签名零变。
- **重导出**：三命令与 `ExecOutputChunk` 出线后 MUST 经导出入口再生成并入库（`bindings:export` 即把盘上产物修至最新）；入库物是否过期由提交前 `git status` / `git diff` 人工核对，MUST NOT 依赖自动化 diff 门禁（零 diff 守卫已退役）。

#### Scenario: 运行中零字节与无记录可分辨

- **WHEN** 前端对「刚 begin 尚未产出字节」与「两槽位均无该 exec_id」两种情形各调一次 `read_exec_output_delta(…, 0)`
- **THEN** 前者返回 `{ text: "", nextOffset: 0 }`（非 `null`、非 `Err`）；后者返回 `null`；两者前端可分辨（「运行中，等待输出…」vs「（无输出记录）」）

#### Scenario: 三命令 typed 包装在案

- **WHEN** 重导出 bindings 后审查生成物
- **THEN** `commands` 含 `execOutputPath` / `readExecCommand` / `readExecOutputDelta` 三个 typed 包装（参数键为 lowerCamelCase：`root` / `changeName` / `execId` / `fromOffset`），`bindings.zod.ts` 的 `argsSchemas` / `resultSchemas` 各含对应表项

#### Scenario: ExecOutputChunk 出线形态

- **WHEN** 审查生成物中 `readExecOutputDelta` 的返回类型
- **THEN** 为 `Promise<ExecOutputChunk | null>`，`ExecOutputChunk` 形如 `{ text: string; nextOffset: number }`（camelCase、`u64` → `number`），无 `bigint` 面

#### Scenario: 既有整读命令零 diff

- **WHEN** 导出后审查 `readExecOutput` 的生成签名与 args schema
- **THEN** 与本变更前逐字一致（参数键与返回类型零变化）；既有前端调用点与测试零改动

#### Scenario: 重导出刷新过期生成物

- **WHEN** 新增三命令后只改 Rust 源码、未重导生成物即执行 `pnpm check`
- **THEN** 前置 `bindings:export` 把 `src/types/generated/` 刷新为含三命令与 `ExecOutputChunk` 的最新产物；该改动在 `git status` 中可见，由提交前人工核对入库

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/mod.rs` | 命令登记面 | `all_commands!` 增 `commands::changes::exec_output_path` / `read_exec_command` / `read_exec_output_delta` 三行（builder 注册与 `generate_handler!` 同源；命令条数不硬编码）；archi 段追加 `commands::archi::archi_file_tree`（5 → 6 条，位于 `archi_drift` 之后、`read_file` 之前） |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 三薄命令与出线 DTO | `ExecOutputChunk { text: String, next_offset: u64 }`（`rename_all = "camelCase"` + `specta::Type`）；三命令做 blank root 早退（读命令空结果语义 `None`）+ `LiveExec`/exec 三件回查的 1:1 映射（`ExecOutputDelta` → `ExecOutputChunk`） |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 前端唯一 IPC 类型与调用面 | 三命令 typed 包装 + `ExecOutputChunk` 类型；随前置重导出更新（过期由提交前人工核对） |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | 第二产物 | 三命令 `argsSchemas` / `resultSchemas` 表项 + `ExecOutputChunk` 具名 schema（纯接受面，零修饰） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 清单守卫 | `COMMAND_NAMES` / `COMMAND_WRAPPERS` 48 → 49（`archi_file_tree` / `archiFileTree` 插在 `archi_drift` 之后、`read_file` 之前）；`DTO_TYPES` 97 → 101（清单尾部两个段落本身是字典序：`ArchiFileEntry` / `ArchiFileTree` 落在 archi 段的 `ArchiExcludedFile` 与 `ArchiGateChange` 之间，`FileEntryKind` / `FileTreeMark` 落在末段的 `EdgeMark` 与 `GateTier` 之间；清单首锚仍 `ActivePhase`、尾锚仍 `WarningKind`）；`ARCHI_COMMANDS` 4 → 6（补齐 `archi_drift` + 新增 `archi_file_tree`）；`ARCHI_DTO_TYPES` 19 → 23（同样按字典序：`ArchiFileEntry` / `ArchiFileTree` 插在 `ArchiDependsOn` 与 `ArchiDiagnostic` 之间，`FileEntryKind` / `FileTreeMark` 插在 `EdgeMark` 与 `GateTier` 之间）；3 处 `COMMAND_NAMES.len()` 断言同改；既有尾部追加序断言的期望 vec 同步为 `… archi_set_gate, archi_drift, archi_file_tree, read_file`；取 `ARCHI_COMMANDS` 末条做「副本删一条」对照臂的用例（`清单遗漏即产物覆盖断言必红`）随之改判（该族末条由 `archi_set_gate` 变 `archi_file_tree`）；`archi四命令在册且既有条目零位移` 一条用例名改为符实（该文件内只有这一处「四命令」用例名；`ARCHI_COMMANDS` 注释「架构域四命令」同步改为「架构域命令族」） |
| `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts`（新增，见 `desktop-archi-file-tree`） | 前端消费面 | 经 `commands.archiFileTree(root)` 调用（生成物 typed 包装，零裸 invoke 字符串） |
| `packages/desktop/src/lib/ipc.ts` | 消费面（零改动） | 键遍历自动覆盖三命令，参数面 strict / 结果面 soft-report 语义不变 |
| `packages/desktop/src-tauri/src/bindings/mod.rs` | 单 builder 两次导出 | `export_bindings()` 一次 `builder()`、两次 `.export`（`Typescript` → `bindings.ts`、zod exporter → `bindings.zod.ts`）；`ErrorHandlingMode::Throw` 与既有 TS 侧配置零改动 |
| `packages/desktop/src-tauri/src/bindings/zod.rs` | zod 侧 wrapper | `ZodLanguage` newtype（本地实现 `LanguageExt`）+ `ZodFormat`（`PhasesFormat` + 大整数 → `z.number()` + `serde_json::Value` → `z.unknown()`）+ 逐命令表注入（`Zod::with_raw`）；参数键名 lowerCamelCase 与 invoke payload 逐字一致；结果取 `Result` 的 Ok 臂 |
| `packages/desktop/src-tauri/crates/tools/specta-zod-patched/**` | vendored exporter | 基线 0.0.4 + 补丁 ①struct strict ②删 `export type` ③递归 getter 化（延后项与兜底项见 README）；每处带 `PATCH(n)` 标记 |
| `packages/desktop/src-tauri/crates/tools/export-bindings/src/main.rs` | 导出驱动 | 仍只调 `dev_team::bindings::export_bindings()`（入口唯一，两次导出在其内部）；失败以非零退出 |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 前端唯一 IPC 类型与调用面 | 全部命令 typed 包装 + 全部出线 DTO 类型（含 `archiFileTree` 包装与 `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark`）；入库 + check/build 前置重导出（过期由提交前人工核对）；knip ignore；类型与 typed invoke 包装来源不变，`ipc` 与 Equal 断言只从此处取类型，MUST NOT 转发 zod 类型 |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | 第二产物 | 纯 schema：具名类型 schema + `argsSchemas` / `resultSchemas` / `channelArgs` + `degraded()` prelude（含 `archi_file_tree` 表项与四个具名 schema）；零 `export type`；`z.strictObject` 写死；零 `.default/.transform/.catch/.pipe/.refine` |
| `packages/desktop/src-tauri/crates/core/agent`（derive 增量） | 出线事件类型导出面 | `AgentEvent` / `AgentEventKind` / `AgentBlock` + `specta::Type`；枚举语义与 serde 线格式不变；禁 Tauri 不破 |
| `packages/desktop/src-tauri/crates/core/workflow`（derive 增量） | 出线查询类型导出面 | `ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry` + `specta::Type`；语义不动 |
| `packages/desktop/src-tauri/crates/infra/store`（derive 增量） | 出线模型导出面 | store 模型 + `specta::Type`；native_model 版本治理不变 |
| `packages/desktop/src-tauri/src/commands/`（`AgentRunMessage` / `FileWatchEvent`） | app 层 IPC 信封导出面 | tag `ipc` 信封与 watch 载荷 typed；`STATUS_*` 裸常量随枚举化退役 |
| `packages/desktop/src/lib/agent-transport.ts` | transport 适配层 | 手写镜像删除、接生成绑定；`readChainParams` 等运行时校验保留 |
| `packages/desktop/src/types/dto.ts`（shim → 删） | 过渡容错网 | 纯 re-export（含逐名映射）；knip 零引用即删，不按时间过渡 |
| `packages/desktop/package.json` / `knip.json` | 脚本与检查边界 | check/build 前置重导出（零 diff 守卫已退役，过期由提交前人工核对）；生成目录 ignore 覆盖两份产物（不另开条目）；`bindings:export` / `check` 命令形态不变、覆盖面随生成目录扩大；前端 `zod` 依赖的精确 pin / 单实例 / knip 可见性见 `desktop-ipc-runtime-guard`（不在本条重复） |

### API: `packages/desktop/src-tauri/src/commands/mod.rs`（单一登记面）

| 项 | 变化 | 描述 |
|------|------|------|
| `all_commands!` | 新增 `commands::exec::agent_session_watch` | builder 注册与 `generate_handler!` 同源；绑定生成随之一致 |
| `all_commands!` | 追加 `commands::archi::archi_file_tree` | 位于 `archi_drift` 之后、`read_file` 之前（archi 段 5 → 6 条；builder 注册与 `generate_handler!` 同源） |

### API: `packages/desktop/src-tauri/src/bindings/mod.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `export_bindings()` | 增第二次 `.export` | 返回值与错误映射不变；第二产物路径与类型产物同目录（`src/types/generated/bindings.zod.ts`），`create_dir_all` 单点保留 |
| `builder()` | 零改动 | 仍为唯一注册面（`all_commands!` + `ErrorHandlingMode::Throw` + `semantic_types` + bigint 配置） |

### CLI: `packages/desktop/package.json`（脚本）

| 脚本 | 命令 | 描述 |
|------|------|------|
| `check` | `pnpm run server:check && pnpm run bindings:export && pnpm run client:check` | 检查链直接前置重导出（零 diff 守卫已退役）；生成物过期由提交前人工核对 |
| `bindings:export` / `build` | 不变 | 导出免守卫（构建产物需最新生成物）；`bindings:export` 内部多产出一份 zod schema，自检失败以非零退出 |

### Component

无。本变更新增零组件、零路由、零可见 UI 面（前端只新增非渲染模块 `src/lib/ipc.ts` / `src/lib/ipc-drift.ts`）。

### Component: `packages/desktop/src-tauri/src/bindings/mod_test.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `COMMAND_NAMES` / `COMMAND_WRAPPERS` | 51 → 53 | 追加 `("archi_api_check", "archiApiCheck")` / `("archi_api_generate", "archiApiGenerate")`（登记序在 `archi_file_tree` 之后）；三处 `len()` 断言与尾部序期望 vec 同步 |
| `DTO_TYPES` | 101 → 104 | 追加 `ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome`（按字典序插入 archi 段） |
| `ARCHI_COMMANDS` | 6 → 8 | 族末条由 `archi_file_tree` 变 `archi_api_generate`；「取末条做副本删一条」对照臂同步 |
| `ARCHI_DTO_TYPES` | 23 → 26 | 三个新类型按字典序入册 |

### Component: 生成物（入库）

| 项 | 描述 |
|---|---|
| `packages/desktop/src/types/generated/bindings.ts` | 再生成：`archiApiCheck` / `archiApiGenerate` typed 包装 + 三个新 `export type` |
| `packages/desktop/src/types/generated/bindings.zod.ts` | 再生成：两条命令的 `argsSchemas` / `resultSchemas` 表项与三个具名 schema |
| `packages/desktop/src/lib/ipc-contract.test.ts` | 零改动：键集全量断言（`Object.keys(argsSchemas)` 与 `commands` 键集相等）自动覆盖新命令，核对随动即可 |
