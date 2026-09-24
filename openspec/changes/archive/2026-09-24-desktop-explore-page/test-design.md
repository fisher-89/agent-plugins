# 测试设计: desktop-explore-page

> **日期**: 2026-09-24

---

## 验收范围

<!-- 测试类型路由口径：进程内可验证归「单元测试」；需两模块以上组合才暴露行为归「集成测试」；纯管线/装配约束归「不可测试」。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 对含 `openspec/explores/foo.md` 的 workspace 调用返回文件文本；未知 stem 返回空；`a/b`、`..` 等穿越分量被拒；blank root 返回空结果 | 单元测试 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/explore_test.rs`、`packages/desktop/src-tauri/src/commands/explores/mod_test.rs` |
| AC-2 | 扫描列目录内全部 `*.md`（stem 形式），目录缺失返回空；命令层以 store 清单滤除已绑定文件 | 集成测试 | `src-tauri/crates/core/workflow/src/queries/explore.rs` + `src-tauri/crates/infra/store/src/store.rs` + `src-tauri/src/commands/explores/mod.rs`（关系「workflow扫描 → store清单求差 → scan_explores命令」）；纯扫描半在 `explore_test.rs` 单元覆盖 |
| AC-3 | 建档 / 按 root 清单 / 删除可用；独立主键与文件名解耦（文件改名记录不破）；磁盘文件删除记录保留；DB 查看器信封 API 零改动覆盖新模型 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`（信封覆盖面经既有 `envelope_test.rs` 全模型断言自然扩展） |
| AC-4 | 含 v1 记录的存量库打开后既有 run 可重放且 `source` 缺省 `debug`；新写入 run 携带 `source` / `source_ref` / `parent_run_id` | 集成测试 | `src-tauri/src/commands/exec/mod.rs` + `src-tauri/src/commands/exec/agent.rs` + `src-tauri/crates/infra/store/src/model.rs` + `src-tauri/crates/infra/store/src/store.rs`（关系「agent_start参数 → RunProvenance编排 → AgentRunRecord v2落库」）；v1 fixture 半在 `store_test.rs` 单元覆盖 |
| AC-5 | 同一 explore 的 resume 链（≥2 条 run，`parent_run_id` 相连）可按发起顺序完整还原并重放事件 | 集成测试 | `src-tauri/crates/infra/store/src/store.rs` + `src-tauri/src/commands/exec/mod.rs`（关系「链式run落库 → restore_run_chain还原 → agent_run_chain命令」）；前端重放半在 `useExploreSession.test.ts` 与 explore_session_pipeline 集成面 |
| AC-6 | `resume_session_id` 非空时 CLI 参数含 `--resume <id>` 且其余 flag 不变；None 时无此 flag；调试页不传新参数行为与现状完全一致 | 集成测试 | `src-tauri/crates/infra/agent/src/flags.rs` + `src-tauri/src/commands/exec/mod.rs`（关系「agent_start参数 → RunProvenance编排 → AgentRunRecord v2落库」）；flag 组装半在 `flags_test.rs` 单元覆盖 |
| AC-7 | 修改被订阅文件 → 前端收到仅含修改信号的推送（防抖后）重读刷新；通知不含文件内容；详情页卸载后退订、无后续信号 | 集成测试 | `src-tauri/crates/infra/watch/src/lib.rs` + `src-tauri/src/commands/watch/mod.rs`（关系「notify单文件订阅 → WatchRegistry → Channel桥接」）+ `src/views/explores/hooks/useExploreDoc.ts`（关系「watch信号 → 防抖 → 显式重拉 → 卸载退订」） |
| AC-8 | `/explores` 清单来自 store 按当前 workspace 过滤；导入选中未绑定文件即建档；新话题建档后预览呈空态（应用不落盘 explore.md、不预建空档） | 集成测试 | `src/views/explores/ExploreView.tsx` + `src/views/explores/hooks/useExploreList.ts` + `src-tauri/src/commands/explores/mod.rs`（关系「清单页 → 新建流程 → explore记录CRUD」）；CRUD 语义半在 `store_test.rs` 单元覆盖 |
| AC-9 | 详情双栏可拖分界；气泡映射四变体（Text→markdown、Thinking→折叠、ToolUse→卡片、ToolResult 成对）；打开续最近链重放历史；发送拼 stance 且经 resume 续话；`AskUserQuestion` 呈现静态卡片 | 集成测试 | `src/views/explores/ExploreDetailView.tsx` + `src/views/explores/hooks/useExploreSession.ts` + `src/views/explores/hooks/useExploreDoc.ts` + `src-tauri/src/commands/exec/mod.rs`（关系「详情双恢复 → 会话链resume发送」）；四变体映射半在 `ExploreConversation.test.tsx`、链重放半在 `useExploreSession.test.ts` 单元覆盖 |
| AC-10 | `/explores`、`/explores/:name` 可达；`nav-explores` active 态由 URL 派生；详情页 workspace 切换 replace 回 `/explores`；未知路径仍回 `/changes` | 集成测试 | `src/routes.tsx` + `src/components/AppSidebar.tsx` + `src/views/explores/ExploreView.tsx`（关系「侧栏导航与路由 → /explores 路由面」） |
| AC-11 | `vp check` / knip / `vp test` 全绿；`cargo test --workspace` 全绿（含 store / agent / workflow / watch 新测试）；无 knip 豁免新增 | 不可测试 | —（管线门禁类 AC，见「不可测试项」） |
| — | 非 AC 绑定模块面：单分量校验单点化、runner 契约字段、模型定义、信封登记、编排填充、侧栏件、DTO 类型 | 单元测试 | `queries/mod_test.rs`、`crates/core/agent/src/runner_test.rs`、`model_test.rs`、`envelope_test.rs`、`src/commands/exec/agent_test.rs`、`AppSidebar.test.tsx`、`src/types/dto.test.ts`（框架章，行为经上表 AC 行间接覆盖） |
| — | 非 AC 绑定模块面：explore 页面组件本体（清单 / 详情 / 对话区 / composer / 预览 / 新建对话框） | 单元测试 | `src/views/explores/*.test.tsx`、`src/views/explores/components/*.test.tsx`（框架章，组件行为经 hooks 单元章与集成面覆盖） |
| — | 非 AC 绑定模块面：shadcn registry 件（`message-scroller` / `message` / `resizable`） | 不可测试 | —（第三方生成件，见「不可测试项」） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test] 经 cargo test --workspace，前端 vitest API 经 vp test）。Rust 测试文件为与源文件同目录的 `*_test.rs` 模块；前端为同目录 `*.test.ts(x)`。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/queries/explore.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/explore_test.rs

#### 待测功能

- read_explore(): stem 单分量校验（防穿越）后读 `explores_root/<name>.md` UTF-8 文本；缺失/非 UTF-8 → `None`；纯读零写入
- scan_explores(): 列 explores_root 顶层 `*.md`（stem 升序 + 修改时间）；目录缺失返回空；不做绑定过滤

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| read_explore | 正向 | 含 `foo.md` 的 workspace：返回 `ExploreDoc{ name: "foo", content: 文件全文 }`（AC-1） | 新增 |
| read_explore | 异常 | 未知 stem：返回 `None` 而非错误（AC-1） | 新增 |
| read_explore | 异常 | `a/b`、`../x`、`..`、`..\\x`、绝对路径等含分隔符/穿越分量：一律 `None`（AC-1） | 新增 |
| read_explore | 边界 | name 为空串：`None`（单分量校验拒绝空分量） | 新增 |
| read_explore | 边界 | 超长 name（>1000 字符）：`None`，不 panic 不触盘 | 新增 |
| read_explore | 边界 | 文件存在但内容非 UTF-8：`None`（读失败归一为缺失语义） | 新增 |
| read_explore | 边界 | `<name>.md` 位置实为目录：`None`（fs 读失败不逃逸） | 新增 |
| scan_explores | 正向 | 目录含 3 个 `.md`：返回 3 条 `ExploreScanEntry`，stem 升序、`modified_at` 为文件修改时间（AC-2） | 新增 |
| scan_explores | 边界 | 目录缺失（新 workspace 无 explores 目录）：返回空 `Vec` 不报错（AC-2） | 新增 |
| scan_explores | 边界 | 目录存在但为空：返回空 `Vec`（AC-2） | 新增 |
| scan_explores | 边界 | 目录混有非 `.md` 文件与子目录（子目录内含 `.md`）：只列顶层 `.md`，不递归、不收录其他扩展名 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 磁盘 workspace 文件 | tempfile 临时目录搭 `explores_root` 真实文件树（写 `*.md`、非 UTF-8 字节、同名目录）；全程真实文件系统，不 mock fs | 全部用例 |

<!-- 不测 std::fs / 目录遍历库自身语义，只测本模块的校验、收集与排序组装。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`is_change_name` → `is_single_component_name` 为 pub(crate) helper，不列）。其行为面经两条既有测试路径回归覆盖：`explore_test.rs`（`read_explore` 的 stem 校验即该 helper 的消费方）与既有 `list_test.rs` / `detail_test.rs`（改名后 `locate_change` 口径不得漂移）。本章不单独立用例。 -->

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`AgentRunParams` 增 `resume_session_id: Option<String>` 属类型定义变更，唯一消费方 `build_args` 的组装断言在 flags_test.rs 章，契约透传断言在 exec/mod_test.rs 集成关系 R2）。既有 runner_test.rs 作存量回归：`Eq` derive 在新字段下保持成立。本章不单独立用例。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/flags.rs -> packages/desktop/src-tauri/crates/infra/agent/src/flags_test.rs

#### 待测功能

- build_args(): flag 组装纯函数——`resume_session_id = Some(id)` 时尾部追加 `--resume <id>`，`None` 时与既有序列完全一致；其余 flag 组装不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| build_args | 正向 | `resume_session_id = Some("s-1")`：args 尾部恰为 `["--resume", "s-1"]`，且前缀与既有四要素序列逐项相等（AC-6） | 新增 |
| build_args | 边界 | `resume_session_id = None`：组装结果与既有基线逐项相等，无 `--resume`（AC-6） | 新增 |
| build_args | 边界 | `resume_session_id = Some("")`：仍组装 `--resume` + 空 arg 单参数（组装层不把关，与空 prompt 同哲学）（AC-6） | 新增 |
| build_args | 边界 | resume id 含空格/引号/换行/emoji/超长（>1000 字符）：原样保留为单个 arg 不拆分（AC-6） | 新增 |
| build_args | 边界 | env × permission-mode 九种档位组合 × `Some("s-1")`：`--resume` 恒在尾部、档位 flag 与其相对次序不变（resume 组装与档位正交） | 新增 |
| build_args | 异常 | 九种档位组合 × `None`：均不含 `--resume`、`--include-partial-messages`、`--model`（原「全组合不含禁用 flag」断言的改写版：`--resume` 解禁为条件 flag，其余禁用项维持） | 废弃 |
| build_args | 正向 | `None` 时与改动前基线（不含 resume 字段的旧 params 组装结果）序列完全一致——向后兼容回归（AC-6） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数无外部依赖（现有惯例：无 Mock） | 全部用例 |

<!-- 废弃行说明：既有用例「全组合九种档位组装均不含禁用flag」将 `--resume` 列入禁用清单，随本变更解除「无 resume」MVP 边界而废弃，由上表两行改写替代（禁用清单收窄 + resume 组装正交断言）。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- design.md 未声明该文件的公共函数/API 行（`ExploreRecord`、`AgentRunRecord` v2/v1 均为类型定义）。模型编解码行为经 `Store` 公共 API 观测，用例收口在 `store_test.rs` 章（落库往返保真 + v1 fixture 存量读取）；既有 model_test.rs 作结构性回归（native_model id/version 登记不破坏既有模型编解码）。本章不单独立用例。 -->

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

<!-- design.md 未声明该文件的公共 API 变更（信封 API 函数本体零改动，`MODEL_ENTRIES` 仅登记 `explore` 一行）。既有 envelope_test.rs 的全模型遍历断言（list_models 计数 / scan / key_of）在登记 `ExploreRecord` 后自动扩展覆盖 AC-3「信封 API 零改动覆盖新模型」，无需新增用例面。本章不单独立用例。 -->

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- Store.create_explore_record(): 写事务内 `max(id)+1` 分配；name 单分量校验；同 `(root, name)` 重复建档 → `Err`；只写 DB 不触磁盘
- Store.list_explore_records(): 按 root 过滤，主键 id 升序稳定序
- Store.find_explore_record(): 按 `(root, name)` 寻址，miss → `None`
- Store.rename_explore_record(): in-place 改 `name` 保主键（保 `source_ref` 链绑定）；目标名已存在 → `Err`
- Store.delete_explore_record(): 删记录不动磁盘；miss 幂等 `Ok(false)`
- Store.restore_run_chain(): 按 `(source, source_ref)` 过滤 → `(started_at, id)` 最新为链头 → 沿 `parent_run_id` 回溯（visited 防环）→ 反转为发起顺序；无链返回空 `Vec`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| create_explore_record | 正向 | 两次建档：id 递增（max+1）、`created_at = updated_at` 为入库时刻毫秒值（AC-3） | 新增 |
| create_explore_record | 异常 | 同 `(root, name)` 重复建档 → `Err`（AC-3） | 新增 |
| create_explore_record | 异常 | name 为 `a/b`、`..` 等非单分量 → `Err` | 新增 |
| create_explore_record | 边界 | 同名不同 root 各建一档：互不影响、各自可查（root 是归属键） | 新增 |
| list_explore_records | 正向 | 多 root 多记录：仅返回入参 root 的记录，id 升序（AC-3 / AC-8） | 新增 |
| list_explore_records | 边界 | 空 root 串：不匹配任何记录，返回空 `Vec`（blank root 纪律的 store 半） | 新增 |
| find_explore_record | 正向 | 命中 `(root, name)` → `Some(记录)` | 新增 |
| find_explore_record | 异常 | name 或 root 未命中 → `None` | 新增 |
| rename_explore_record | 正向 | in-place 改名：主键 id 不变、`updated_at` 刷新、原 `(root, old_name)` 不再命中、`(root, new_name)` 命中（AC-3，D2 保链前提） | 新增 |
| rename_explore_record | 异常 | 目标名 `(root, new_name)` 已存在 → `Err`，原记录不被破坏 | 新增 |
| rename_explore_record | 异常 | 被改记录 miss → `Err` | 新增 |
| delete_explore_record | 正向 | 删除后 `find` 返回 `None`、返回 `true`；workspace 磁盘上的同名 `.md` 文件原样保留（store 不触磁盘，AC-3 孤儿保留） | 新增 |
| delete_explore_record | 边界 | miss → `Ok(false)`，重复删除幂等（AC-3） | 新增 |
| restore_run_chain | 正向 | 同 `(source, source_ref)` 落 2 条 run（后者 `parent_run_id` 指向前者）：按发起顺序还原，链头在末位（AC-5） | 新增 |
| restore_run_chain | 边界 | `(source, source_ref)` 未命中任何 run：返回空 `Vec`（无链起链语义，AC-5） | 新增 |
| restore_run_chain | 边界 | 混入干扰记录（同 `source_ref` 不同 `source`、同 `source` 不同 `source_ref`）：均不入链（AC-5） | 新增 |
| restore_run_chain | 边界 | `parent_run_id` 构造环（A→B→A）：visited 集防环，返回不悬挂、成员完整 | 新增 |
| restore_run_chain | 边界 | 三链交叉（分叉后再汇聚）：还原为单链（链头唯一取 `(started_at, id)` 最新），不重复、不遗漏 | 新增 |
| v1→v2 演进 | 正向 | 以保留的 v1 结构（13 字段）编码落库后重开 db：记录可读、`source` 缺省 `"debug"`、`source_ref` / `parent_run_id` 为 `None`、既有 13 字段保真（AC-4） | 新增 |
| v1→v2 演进 | 正向 | v2 写入（三字段非缺省）→ 重开 db 读回：三字段往返保真（AC-4） | 新增 |
| v1→v2 演进 | 边界 | 混合库（v1 记录 + v2 记录并存）打开后 `list_agent_runs` 全量可读、两代记录按 `(started_at, id)` 统一排序（AC-4） | 新增 |
| 事件表零改动 | 边界 | v1/v2 run 的事件记录照常经 `append/list_agent_run_events` 重放（事件表全局 `run_id` 锚定不受模型演进影响，AC-4/AC-5） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| db 文件 | tempfile 真开 db 文件（现有惯例：存储层不 mock），重开场景复用固定文件名路径 | 全部用例 |
| v1 历史载荷 | 以保留的 v1 版本化结构（非 `#[native_db]` 注册）经 native_model 编码构造存量字节，模拟升级前旧库内容 | v1→v2 演进三行 |
| 系统时钟 | 不 mock：`created_at` / `started_at` 仅断言入库保真与相对次序，不断言绝对值（现有惯例） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/watch/src/lib.rs -> packages/desktop/src-tauri/crates/infra/watch/src/lib_test.rs

#### 待测功能

- watch::subscribe(): notify 订阅单文件，修改事件经 mpsc 发出 `FileWatchSignal`（仅 `path`，无内容）；目标不存在不报错；返回 `Watcher` 句柄，drop 即停流退订

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| subscribe | 正向 | 订阅已存在文件 → 追加写入（rename-replace 原子保存模拟）→ `mpsc::Receiver` 在超时窗口内收到 `FileWatchSignal { path }`，path 与被订阅路径一致（AC-7） | 新增 |
| subscribe | 正向 | 目标文件不存在时订阅：`Ok(Watcher)` 不报错；随后创建该文件并写入 → 信号生效送达（AC-7，D5 未落盘照常订阅） | 新增 |
| subscribe | 边界 | 信号结构仅含 `path` 字段：序列化输出无任何内容/字节类键（通知无内容，AC-7） | 新增 |
| subscribe | 边界 | `Watcher` drop 后再修改文件：超时窗口内 `recv` 收不到任何信号（drop 即退订语义，AC-7） | 新增 |
| subscribe | 边界 | 文件删除后重建再修改：父目录语义下信号仍送达（Windows 目标平台事件不丢，D5 留痕断言） | 新增 |
| subscribe | 边界 | 短窗口多次修改：信号流就绪不丢不挂（无后端去抖的契约面——合并责任在前端，本层只保证转发） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 被监视文件 | tempfile dev-dependency 建临时目录与真实文件，真实触发文件系统事件；mpsc `recv_timeout` 收信号 | 全部用例 |

<!-- 不测 notify 库自身的事件语义，只测本 crate 的订阅组装、信号形态与 Watcher 生命周期句柄。事件等待设超时上限，防止环境无事件时用例悬挂。 -->

### packages/desktop/src-tauri/src/commands/explores/mod.rs -> packages/desktop/src-tauri/src/commands/explores/mod_test.rs

#### 待测功能

- read_explore()（命令）: blank root → `None`；layout 派生 + workflow 查询薄包装
- scan_explores()（命令）: workflow 扫描后以 store 清单求差滤除已绑定 stem；blank root → `Ok(vec![])`
- explore_doc_path(): 布局派生 `explores_root/<name>.md` 完整路径，无 IO；stem 校验同口径
- list_explore_records()（命令）: store 透传；blank root → `Ok(vec![])`
- create_explore_record()（命令）: store 建档薄包装
- rename_explore_record()（命令）: store 改名薄包装
- delete_explore_record()（命令）: store 删除薄包装，miss 幂等

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| read_explore | 正向 | 有效 root + 已落盘 `foo.md`：返回 doc，与 workflow 直查结果 serde 等值（薄包装不加工，AC-1） | 新增 |
| read_explore | 边界 | blank root（空串）：`None`，不 panic（AC-1 blank root 纪律） | 新增 |
| read_explore | 异常 | `a/b`、`..` 穿越分量：`None`（命令边界拒绝，同 `commands/queries` 敌意 source 口径，AC-1） | 新增 |
| scan_explores | 边界 | blank root：`Ok(vec![])`（AC-2） | 新增 |
| scan_explores | 异常 | store 打开态下正常调用：与「workflow 扫描 − store 已绑定 stem」手工求差结果一致（纯扫描半在 explore_test.rs，求差在集成关系 R1） | 新增 |
| explore_doc_path | 正向 | 有效 root + 合法 name：返回路径与 `Layout::explores_root` 派生结果逐段相等（测试文件豁免命名隔离扫描，可直接对比布局派生值）（D6） | 新增 |
| explore_doc_path | 异常 | 穿越分量 name → `None`；blank root → `None`（D6 前端取路径入口的防穿越） | 新增 |
| list_explore_records | 正向 | 有 2 条记录的 root：返回 2 条 DTO，与 `store.list_explore_records` serde 等值（AC-8） | 新增 |
| list_explore_records | 边界 | blank root：`Ok(vec![])`（AC-8） | 新增 |
| create_explore_record | 正向 | 建档返回记录 DTO，`list` 可见（AC-8） | 新增 |
| create_explore_record | 异常 | 重复 `(root, name)` → `Err`（store 错误透传为命令 `Err(String)`，AC-8） | 新增 |
| rename_explore_record | 正向 | 改名后返回更新记录、新名可 `find`（AC-3/D2 入口） | 新增 |
| delete_explore_record | 正向 | 删除已绑定记录 → `Ok(true)`；再删 → `Ok(false)` 幂等（AC-3） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri `State<Store>` | `tauri::test::mock_app()`（MockRuntime）manage 真实 Store（tempfile 真库），经 `app.state::<Store>()` 取 State（沿 `commands/exec/mod_test.rs` 惯例） | 涉及 store 的命令用例 |
| 磁盘 workspace | tempfile 临时 root 搭 `explores_root` 真实文件 | read_explore / scan_explores / explore_doc_path 用例 |
| 系统 PATH | 不适用：本轨道命令零子进程（`#[tauri::command]` 保留原函数可直调，无状态薄包装本身即结构断言） | — |

### packages/desktop/src-tauri/src/commands/watch/mod.rs -> packages/desktop/src-tauri/src/commands/watch/mod_test.rs

#### 待测功能

- watch_subscribe(): 建订阅绑定 `Channel<FileWatchEvent>`；同一路径（canonical path 键）重复订阅幂等返回既有 subscription_id；目标缺失不报错
- watch_unsubscribe(): 移除并 drop `Watcher`（停流）；miss 幂等 `Ok(false)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| watch_subscribe | 正向 | 订阅返回 `subscription_id`；修改被订阅文件 → Channel 收到 `{ path }` 事件且无内容字段（端到端信号面在集成关系 R4 展开） | 新增 |
| watch_subscribe | 边界 | 同一路径重复订阅：返回同一 subscription_id，Channel 不产生重复信号（D5 幂等） | 新增 |
| watch_subscribe | 边界 | 目标文件不存在：`Ok(id)` 不报错（AC-7） | 新增 |
| watch_subscribe | 边界 | 不同路径分别订阅：各自独立 subscription_id、事件互不串扰 | 新增 |
| watch_unsubscribe | 正向 | 以有效 id 退订 → `Ok(true)`；此后修改文件 Channel 无后续事件（AC-7） | 新增 |
| watch_unsubscribe | 边界 | 未知 id / 重复退订：`Ok(false)` 幂等（AC-7） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri `State<WatchRegistry>` | `mock_app()` setup 内 `app.manage(WatchRegistry::default())` 后取 State | 全部用例 |
| Tauri `Channel` | 构造测试用 Channel 收集器（沿 exec/mod_test.rs 的 `InvokeResponseBody` 捕获惯例），断言载荷 JSON 形态 | 信号到达/退订断言 |
| 被监视文件 | tempfile 真实文件触发真实 notify 事件（存储与文件系统层不 mock） | 信号到达/退订断言 |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- agent_start(): 增四个可选参数（`resume_session_id` / `source` / `source_ref` / `parent_run_id`）组装 `AgentRunParams` + `RunProvenance`；`source` 缺省 `debug`
- agent_run_chain(): `store.restore_run_chain` 命令薄包装（通用面，非 explore 专属）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| agent_start | 正向 | 全新参省略（调试页 invoke 形态）：FakeRunner 收到的 `AgentRunParams.resume_session_id == None`，落库记录 `source == "debug"`、其余两字段 `None`，与现状行为完全一致（AC-6 回归） | 新增 |
| agent_start | 边界 | 仅传 `resume_session_id`（source 等仍缺省）：params 携带 resume、记录仍按缺省 `debug` 落库（四参独立可选，无隐藏耦合） | 新增 |
| agent_start | 边界 | 仅传 `source = "explore"` 不传 `source_ref`：照入参落库，无来源一致性强校验（设计未规定拒收，锁定现状契约） | 新增 |
| agent_run_chain | 正向 | store 内有链：返回 `restore_run_chain` 同序同值结果（serde 等值，AC-5） | 新增 |
| agent_run_chain | 边界 | 无链：`Ok(vec![])`（AC-5） | 新增 |
| agent_run_chain | 边界 | 未知 source/source_ref 组合：空 `Vec` 不报错（AC-5） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| agent CLI 子进程 | 既有 `FakeRunner` 测试缝（`run_agent_with` 注入）+ 隔离 PATH 触发 `CliMissing`（PATH 修改以共享互斥锁串行化，现有惯例） | agent_start 全部用例 |
| Tauri `State<Store>` | `mock_app()` manage 真实 Store（tempfile 真库） | 落库与链查询用例 |
| Tauri `Channel` | 测试用 Channel 收集器捕获 `AgentEvent` 流 | agent_start 事件面断言 |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

<!-- design.md 未声明该文件的公共函数/API 行（`run_agent` / `run_agent_with` 为既有内部编排，新增 `RunProvenance` 属 pub(crate) 类型定义）。provenance 填充行为经本文件既有测试装置（FakeRunner）在 exec/mod_test.rs 集成关系 R2 中观测；既有 agent_test.rs 作存量回归（假 runner 测试缝不变）。本章不单独立用例。 -->

### packages/desktop/src/lib/exploreStance.ts -> packages/desktop/src/lib/exploreStance.test.ts

#### 待测功能

- buildExplorePrompt(): 精简 explore stance 前导 + 用户输入的拼接（每条 explore run 的 prompt 均拼前导，含 resume 续话轮）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| buildExplorePrompt | 正向 | 常规输入：返回串以前导模板开头、以用户输入原文结尾，前导恰出现一次（AC-9） | 新增 |
| buildExplorePrompt | 边界 | 空串输入：前导 + 空尾，不抛错（AC-9） | 新增 |
| buildExplorePrompt | 边界 | 输入含换行/引号/反引号/`${}` 插值形貌/emoji：原样保留在尾部，不被模板截断或改写（AC-9） | 新增 |
| buildExplorePrompt | 边界 | 超长输入（>1000 字符）：完整保留，无截断（AC-9） | 新增 |
| buildExplorePrompt | 边界 | 输入本身含前导文本片段：不递归展开、输出前导仍只出现一次（拼接非替换） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | TS 纯函数，零依赖零 Mock | 全部用例 |

### packages/desktop/src/views/explores/hooks/useExploreList.ts -> packages/desktop/src/views/explores/hooks/useExploreList.test.ts

#### 待测功能

- useExploreList(): 返回 `records` / `loading` / `error` / `refresh` / `create` / `rename` / `remove`；root 变更与显式动作触发取数，无轮询

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useExploreList | 正向 | 有效 root 挂载：以 `{ root }` 发起 `list_explore_records` 一次，`records` 呈现返回序（AC-8） | 新增 |
| useExploreList | 正向 | `create`：发起 `create_explore_record`，成功后以当前 root 重新取数、新记录入列（AC-8） | 新增 |
| useExploreList | 正向 | `rename` / `remove`：分别发起对应命令，成功后列表刷新反映变更（AC-8） | 新增 |
| useExploreList | 边界 | root 为 `null`：不发起任何取数、`records` 为空态（AC-8） | 新增 |
| useExploreList | 边界 | root 变更（A→B）：以新 root 重取，旧 root 的在途结果不覆盖新态（AC-8，与 ChangeList 同款竞态语义） | 新增 |
| useExploreList | 异常 | `list_explore_records` reject：`error` 置位、`records` 保持空态不崩（AC-8） | 新增 |
| useExploreList | 边界 | `refresh` 显式调用恰发一次取数；无 root 时 `refresh` 为 no-op | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri IPC（`@tauri-apps/api/core` invoke） | `vi.mock` 按命令名分发：`list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record` 返回 fixture DTO；可切换 reject | 全部用例 |

### packages/desktop/src/views/explores/hooks/useExploreDoc.ts -> packages/desktop/src/views/explores/hooks/useExploreDoc.test.ts

#### 待测功能

- useExploreDoc(): 返回 `doc` / `loading` / `error` / `refresh`；内聚 watch 订阅（`explore_doc_path` 取路径 → `watch_subscribe`）与信号 500ms trailing 防抖显式重拉；卸载退订

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useExploreDoc | 正向 | 有效 root+name 挂载：`read_explore` 拉取一次、`doc` 呈现内容；`explore_doc_path` 返回路径后恰发起一次 `watch_subscribe`（AC-7） | 新增 |
| useExploreDoc | 正向 | Channel 推入 1 个信号：fake timers 推进 500ms 后恰重拉一次 `read_explore`（trailing 防抖，AC-7） | 新增 |
| useExploreDoc | 边界 | 500ms 窗口内推入 N 个信号：合并为恰一次重拉（AC-7） | 新增 |
| useExploreDoc | 边界 | 信号载荷仅 `{ path }`：无内容字段仍正常触发重拉（通道只承载失效语义，AC-7） | 新增 |
| useExploreDoc | 边界 | 卸载：以订阅时返回的 subscription_id 发起 `watch_unsubscribe`，此后信号不再引发重拉（AC-7） | 新增 |
| useExploreDoc | 边界 | `explore_doc_path` 返回 `null`（穿越名/blank root）：不发起订阅、`doc` 空态不崩 | 新增 |
| useExploreDoc | 边界 | root/name 为 `null`：不取数、不订阅（AC-8 新话题未选中的空态半） | 新增 |
| useExploreDoc | 异常 | `read_explore` reject：`error` 置位、不崩；watch 订阅仍建立（文档缺失与订阅生命周期解耦，D5） | 新增 |
| useExploreDoc | 异常 | `watch_subscribe` reject（IPC 失败）：页面不崩、`refresh` 显式拉取仍可用（推送失效不阻断取数纪律） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri IPC invoke | `vi.mock` 按命令名分发：`read_explore` / `explore_doc_path` / `watch_subscribe` / `watch_unsubscribe`；`watch_subscribe` 返回固定 id、可切换 reject | 全部用例 |
| 推送通道（Channel 回调） | 捕获 `watch_subscribe` 入参的回调/Channel 载荷形态，测试内手动注入 `FileWatchEvent` 信号模拟后端推送 | 防抖与退订用例 |
| 定时器 | `vi.useFakeTimers()` 控制 500ms 防抖窗口推进 | 防抖合并用例 |

### packages/desktop/src/views/explores/hooks/useExploreSession.ts -> packages/desktop/src/views/explores/hooks/useExploreSession.test.ts

#### 待测功能

- useExploreSession(): 返回 `chain`（发起序 run 记录）/ `events`（重放 + 实时累积）/ `running` / `error` / `send`；`send` 拼 stance、取链尾 `sessionId` 作 `resume_session_id`、携带来源三元组；run 终态定点重读文档

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useExploreSession | 正向 | record 有既有链：挂载发起 `agent_run_chain` 一次 + 逐 run `agent_run_events`，`events` 按发起序拼接全链事件（AC-9/AC-5） | 新增 |
| useExploreSession | 正向 | 有链时 `send`：`agent_start` 收到 `prompt` 以 `buildExplorePrompt` 前导开头、`resume_session_id` 为链尾 run 的 `sessionId`、`source = "explore"`、`source_ref` 为记录 id 十进制串、`parent_run_id` 为链尾 run id（AC-9，D4/D7） | 新增 |
| useExploreSession | 正向 | 无链时 `send`（首条起链）：`resume_session_id` 与 `parent_run_id` 不传、`source` 三元组仍携带（AC-9） | 新增 |
| useExploreSession | 正向 | run 终态后：`agent_run_chain` 重取、链增长，二次 `send` 的 `resume_session_id` 更新为新链尾 `sessionId`（AC-9 多轮续话） | 新增 |
| useExploreSession | 边界 | record 为 `null`：不还原链、`send` 为 no-op（AC-9） | 新增 |
| useExploreSession | 边界 | 链中某 run 无 `session_id`（异常终态）：`send` 不携带 `resume_session_id` 而非传空串 | 新增 |
| useExploreSession | 边界 | 运行中重复 `send`：被抑制（`running` 门闩），不并发发起第二条 run | 新增 |
| useExploreSession | 异常 | `agent_start` reject：`error` 置位、`running` 复位、既有事件不丢（AC-9） | 新增 |
| useExploreSession | 异常 | `agent_run_events` 对中途 run reject：已取得的事件保留，错误可见不崩 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri IPC invoke | `vi.mock` 按命令名分发：`agent_run_chain` / `agent_run_events` / `agent_start` 返回 fixture（多 run 链 + 四变体事件序列）；`agent_start` 记录入参供断言；可切换 reject | 全部用例 |
| stance 前导 | 不 mock：`buildExplorePrompt` 以真实实现参与断言（`send` 拼接语义的真实性） | `send` 用例 |

### packages/desktop/src/types/dto.ts -> packages/desktop/src/types/dto.test.ts

<!-- design.md 未声明该文件的公共函数/API 行（`ExploreRecord` / `ExploreDoc` / `ExploreScanEntry` / `FileWatchEvent` 与 `AgentRunRecord` 扩展均为纯 TS 类型/接口，无运行时行为）。类型正确性由消费方（hooks / 组件 / 集成面）的编译期检查与管线门禁承担；serde camelCase 对齐经集成面 IPC fixture 断言兜底。本章不单独立用例。 -->

### packages/desktop/src/views/explores/ExploreView.tsx -> packages/desktop/src/views/explores/ExploreView.test.tsx

<!-- design.md 未声明该文件的公共函数/API 行（路由双态容器 + 清单页本体为组件，未列入公共函数表）。其行为面（params 派生、切换 replace 过渡抑制、清单渲染、新建入口）由 useExploreList 单元章与集成关系 F1/F2 覆盖。本章不单独立用例。 -->

### packages/desktop/src/views/explores/ExploreDetailView.tsx -> packages/desktop/src/views/explores/ExploreDetailView.test.tsx

<!-- design.md 未声明该文件的公共函数/API 行（resizable 双栏装配方为组件本体）。双栏拖分界、双恢复装配方与 watch 生命周期宿主行为由集成关系 F3/F4 覆盖。本章不单独立用例。 -->

### packages/desktop/src/views/explores/components/ExploreConversation.tsx -> packages/desktop/src/views/explores/components/ExploreConversation.test.tsx

<!-- design.md 未声明该文件的公共函数/API 行（对话区为组件本体）。四变体映射（Text→markdown、Thinking→折叠、ToolUse→卡片、ToolResult 成对）与 `AskUserQuestion` 静态卡片是本变更的明确验收面（AC-9），本章保留最小直测面：以事件 fixture 直渲染组件断言四变体 DOM 形态——但映射逻辑若实现为可复用纯函数并列入 design 公共函数表，则测试随之收口到该函数；组件级断言仅保留渲染不崩与变体可辨。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ExploreConversation | 正向 | `AgentEvent.Message.blocks` 含 Text 块：以 markdown 形态渲染（AC-9） | 新增 |
| ExploreConversation | 正向 | Thinking 块：呈折叠形态、展开后内容可读（AC-9） | 新增 |
| ExploreConversation | 正向 | ToolUse（含 `AskUserQuestion`）块：呈卡片形态，`AskUserQuestion` 为静态可读卡片、无可交互作答控件（AC-9） | 新增 |
| ExploreConversation | 边界 | ToolUse 与 ToolResult 成对呈现：同工具卡片配对、孤立 ToolResult 不悬挂不崩（AC-9） | 新增 |
| ExploreConversation | 边界 | 空事件序列：空态呈现不崩 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无外部依赖 | 组件以事件 fixture 直渲染（jsdom），`MarkdownDocRenderer` 等子件用真实实现 | 全部用例 |

### packages/desktop/src/views/explores/components/ExploreComposer.tsx -> packages/desktop/src/views/explores/components/ExploreComposer.test.tsx

<!-- design.md 未声明该文件的公共函数/API 行（composer 为组件本体，env / permission-mode 档位沿用调试页语义）。输入与档位交互由集成关系 F3 的发送链路覆盖。本章不单独立用例。 -->

### packages/desktop/src/views/explores/components/ExplorePreview.tsx -> packages/desktop/src/views/explores/components/ExplorePreview.test.tsx

<!-- design.md 未声明该文件的公共函数/API 行（预览为组件本体，复用既有 `MarkdownDocRenderer`）。渲染与空态行为由集成关系 F3/F4 覆盖；`MarkdownDocRenderer` 本体语义既有测试已覆盖、不重测。本章不单独立用例。 -->

### packages/desktop/src/views/explores/components/ExploreCreateDialog.tsx -> packages/desktop/src/views/explores/components/ExploreCreateDialog.test.tsx

<!-- design.md 未声明该文件的公共函数/API 行（新建对话框为组件本体）。导入扫描选择建档与新话题建档两入口行为由集成关系 F2 覆盖。本章不单独立用例。 -->

### packages/desktop/src/components/AppSidebar.tsx -> packages/desktop/src/components/AppSidebar.test.tsx

<!-- design.md 未声明该文件的公共函数/API 行（侧栏为组件本体，`nav-explores` testid 与 URL 派生 active 语义由集成关系 F1 按既有 route_pages 断言惯例覆盖）。既有 AppSidebar.test.tsx 作回归：既有 testid 语义与 active 派生不变。本章不单独立用例。 -->

---

## 集成测试

<!-- Rust 侧集成关系收口在命令轨道的 `*_test.rs`（沿 `commands/exec/mod_test.rs` 的「单元 + 集成关系同文件」既有惯例，mock_app + 真实 Store + 真实文件系统）；前端集成关系收敛在 `src/__tests__/` 管线文件（沿 route_pages / agent_run_pipeline 惯例：render(<App/>) 或 render(<View/>) 驱动真实 Router，仅 IPC 边界 mock）。 -->

### workflow扫描 → store清单求差 → scan_explores命令 → `packages/desktop/src-tauri/src/commands/explores/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/explore.rs` | 扫描方（列目录顶层 `*.md`） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 清单数据源（已绑定 stem 集合） |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | 求差组装方（命令边界） |

**关联AC**: AC-2, AC-3

**关系描述**: workflow 查询层只认识磁盘目录（纯读，不认识 store），store 只认识绑定记录（不认识磁盘），「未绑定文件」这一导入语义只能在命令层组合两者求差得出。值得测的组合点：求差以记录 `name`（非磁盘现状）为准，因此孤儿记录（文件已删）仍参与滤除、未绑定的新文件全量入选；任何一侧为空时求差退化为另一侧全集。可能的出错方式：求差误用磁盘文件名比对 store 的展示名、目录大小写/路径形态不一致导致漏滤、blank root 纪律在组合层被绕过。

#### 场景: 导入扫描求差与孤儿滤除

前置：tempfile workspace 的 `explores_root` 落 3 个 `.md`（`a`、`b`、`c`）；store 内建档绑定 `b`（其磁盘文件随后删除，模拟孤儿）。输入：`scan_explores(root)`。预期：仅返回 `a`、`c`（孤儿 `b` 仍被滤除——绑定语义优先于磁盘现状）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 3 文件 + 1 绑定：命令返回未绑定的 2 个 stem（AC-2） | 新增 |
| 边界 | 全部文件已绑定：返回空数组（AC-2） | 新增 |
| 边界 | 目录缺失 + 无任何绑定：返回空数组（两空集求差，AC-2） | 新增 |
| 边界 | 绑定记录的文件已删（孤儿）：该 stem 仍被滤除、不回入候选（AC-2/AC-3 孤儿语义） | 新增 |
| 边界 | store 有记录但目录中有同名大小写差异文件：按记录 `name` 精确求差（不引入文件系统大小写折叠） | 新增 |

##### Mock策略

<!-- 内部模块间调用不 mock；无跨进程边界。store 为 tempfile 真库、目录为 tempfile 真文件，State 经 mock_app 注入。 -->

### agent_start参数 → RunProvenance编排 → AgentRunRecord v2落库 → `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 触发方（参数组装 + 缺省填充） |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | 编排填充方（`RunProvenance` → running 记录初值） |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 契约承载（`AgentRunParams.resume_session_id`） |
| `packages/desktop/src-tauri/crates/infra/agent/src/flags.rs` | 契约消费（`--resume` 组装） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 持久化载体（v2 模型三字段） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 落库方 |

**关联AC**: AC-4, AC-6

**关系描述**: 四个可选参数从 Tauri 命令面流经编排层落到 store 的 v2 记录，且 `resume_session_id` 单独流进 runner 契约改变 CLI 行为（来源三元组走 `RunProvenance` 旁路，D4）。值得测的组合点：调试页 invoke（全参缺省）必须与现状逐位一致（向后兼容）；explore 形态入参下 resume 进 params、三元组进记录、两路互不污染。可能的出错方式：缺省填充漏项导致 v2 记录写空串而非 `debug`/`None`、`RunProvenance` 与 params 字段串线（resume 误进 source）、Tauri `Option` 参数缺省语义与 Rust 侧不一致。

#### 场景: explore 形态启动一次续话 run

前置：mock_app 挂真实 Store + FakeRunner 测试缝。输入：`agent_start(resume_session_id=Some("s-parent"), source=Some("explore"), source_ref=Some("7"), parent_run_id=Some(42), ...)`。预期：FakeRunner 收到的 `AgentRunParams.resume_session_id == Some("s-parent")` 且其余 flag 面不变（flag 组装细节在 flags_test.rs 单元章）；落库记录 `source == "explore"`、`source_ref == "7"`、`parent_run_id == Some(42)`；事件经 Channel 正常流出。

#### 场景: 调试页形态零变化回归

前置：同上装置。输入：`agent_start` 不传四个新参（现状调用形态）。预期：params 的 `resume_session_id == None`；记录三字段为 `"debug"` / `None` / `None`；与改动前行为无任何可观测差异（AC-6 第三分句）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | explore 形态全参：resume 进 params、三元组进记录、事件流正常（AC-4/AC-6） | 新增 |
| 正向 | 全参缺省（调试页形态）：params 无 resume、记录 `debug` 缺省，与现状一致（AC-6） | 新增 |
| 边界 | 仅传 resume（source 等缺省）：两路各自缺省、无串线 | 新增 |
| 边界 | `parent_run_id` 指向不存在的 run：编排无回查（D4），照常落库 | 新增 |
| 异常 | CLI 缺失（隔离 PATH）：`Err` 返回、记录落为 error 终态且三字段保留（失败路径不丢来源） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| agent CLI 子进程 | 既有 `FakeRunner`（`run_agent_with` 注入）捕获 `AgentRunParams`；隔离 PATH + 互斥锁覆盖 CliMissing 分支 | 全部场景 |
| Tauri `State<Store>` / `Channel` | mock_app manage 真实 Store（tempfile 真库）；测试用 Channel 收集器 | 落库与事件断言 |

### 链式run落库 → restore_run_chain还原 → agent_run_chain命令 → `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | 写入方（链字段落库的编排源头） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 还原查询单点（`restore_run_chain`） |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 命令包装方（`agent_run_chain`） |

**关联AC**: AC-5, AC-4

**关系描述**: 会话链的写入（每次 run 带 `parent_run_id` 落库）与还原（按 `(source, source_ref)` 定链头、沿指针回溯）分处两个模块，链语义只在组合时成立。值得测的组合点：多轮 explore 交替进行时各链互不串扰；还原顺序与事件重放顺序（链序逐 run 取事件）拼合后与真实对话序一致。可能的出错方式：链头取次序键错误导致链断、`source_ref` 十进制串与记录 id 类型不一致引发错配、两 explore 并发落库时回溯串链。

#### 场景: 两条 explore 链交替落库后各自完整还原

前置：真实 Store，explore 记录 R7 与 R8 各自交替落 2 条链式 run（R7-1 → R7-2、R8-1 → R8-2），混入一条 debug run。输入：`agent_run_chain("explore", "7")` 与 `agent_run_chain("explore", "8")`。预期：各自按发起序还原恰 2 条，debug 与对方链不入列。

#### 场景: 链还原 → 逐 run 事件重放拼合

前置：同上装置，每条 run 落一组可区分事件。输入：`restore_run_chain` 结果逐 run `list_agent_run_events`。预期：事件流按发起序拼合无重叠无遗漏（前端重放的同一收口语义，AC-5「完整还原并重放事件」）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 两链交替 + 干扰 run：各自还原恰 2 条、发起序（AC-5） | 新增 |
| 正向 | 链还原 × 事件重放拼合：全链事件按发起序无缝拼接（AC-5） | 新增 |
| 边界 | 单条 run 的「链」（无 parent）：还原为含单元素序列（AC-5） | 新增 |
| 边界 | 无链 source_ref：空 `Vec`（AC-5） | 新增 |
| 异常 | store 不可用时命令返回 `Err`（错误模板透传） | 新增 |

##### Mock策略

<!-- 内部模块间不 mock；store 为 tempfile 真库。CLI 层不参与本关系（链还原纯 store 面）。 -->

### notify单文件订阅 → WatchRegistry → Channel桥接 → `packages/desktop/src-tauri/src/commands/watch/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/infra/watch/src/lib.rs` | 信号源（notify 订阅 + mpsc 流） |
| `packages/desktop/src-tauri/src/commands/watch/mod.rs` | 注册与桥接方（WatchRegistry + mpsc → Tauri Channel 线程） |

**关联AC**: AC-7

**关系描述**: watch crate 是零 Tauri 的信号流，命令轨道负责把 mpsc 信号转译为 Tauri `Channel` 推送并托管订阅生命周期。值得测的组合点：信号从文件系统事件到前端 Channel 的全程形态（仅 `path` 无内容）；订阅/退订与桥接线程生命周期的对齐（drop Watcher 后桥接线程随通道关闭退出、不泄漏、不误推）。可能的出错方式：桥接线程在退订后仍持有发送端导致退订不生效、Channel 载荷形态与 `FileWatchSignal` 漂移、重复订阅叠加双桥接线程产生重复推送。

#### 场景: 订阅-修改-推送-退订全程

前置：mock_app 挂 WatchRegistry，tempfile 真实文件。输入：`watch_subscribe(channel, path)` → 修改文件 → 断言 Channel 载荷 → `watch_unsubscribe(id)` → 再修改。预期：修改后 Channel 收到恰一帧 `{ path }`（无内容键）；退订后再修改无任何后续帧（AC-7）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 订阅 → 修改 → Channel 恰收一帧 `{ path }`、无内容字段（AC-7） | 新增 |
| 正向 | 退订 → 再修改：无后续帧（AC-7 卸载后退订语义的后端半） | 新增 |
| 边界 | 重复订阅同一路径：单帧不翻倍（幂等防叠加，D5） | 新增 |
| 边界 | 目标缺失时订阅、文件后出现再修改：信号正常送达（AC-7/D5） | 新增 |
| 边界 | 未落盘期间修改其他无关文件：不产生帧（订阅面单文件收敛） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri `Channel` | 测试用 Channel 收集器（`InvokeResponseBody` 捕获，沿 exec/mod_test.rs 惯例），断言帧数与 JSON 形态 | 全部用例 |
| 被监视文件 | tempfile 真实文件触发真实 notify 事件（不 mock） | 全部用例 |

### 侧栏导航与路由 → /explores 路由面 → `packages/desktop/src/__tests__/route_pages.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/routes.tsx` | 路由表（新增两路由） |
| `packages/desktop/src/components/AppSidebar.tsx` | 导航入口（`nav-explores` NavLink） |
| `packages/desktop/src/views/explores/ExploreView.tsx` | 路由双态容器（params 派生 + 切换 replace 过渡抑制） |

**关联AC**: AC-10

**关系描述**: `/explores` 与 `/explores/:name` 接入既有 HashRouter 后，active 派生、深链直达、workspace 切换 replace 回清单、未知路径兜底四条语义必须与 `/changes` 既有行为并排成立且互不干扰（`/changes` 回归是本关系的隐藏验收面）。沿用 route_pages.test.tsx 既有装置：render(<App />) 驱动真实 HashRouter，仅 invoke 按命令名 mock。可能的出错方式：`nav-explores` 前缀派生把 `/explores-xyz` 误判 active、详情态切 workspace 的过渡轮以「新 root + 旧 name」发详情取数、新路由挤占未知路径兜底。

#### 场景: nav-explores 点击与 active 派生

前置：restored 启动（changes 清单在列）。输入：点击 `nav-explores`。预期：hash 落 `#/explores`、`nav-explores data-active=true` 且 `nav-changes`/`nav-agent` 失活、`list_explore_records` 以当前 root 发起。

#### 场景: 深链与 workspace 切换 replace 回清单

前置：hash 预置 `#/explores/foo` 启动；随后点击另一 workspace 项。预期：启动直达详情态（`nav-explores` 仍 active——前缀派生）；切换后 hash replace 回 `#/explores`、全程不产生「新 root + 旧 name」的取数（`ChangeView` 过渡抑制模式平移断言）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 点击 `nav-explores` → `#/explores`、自身 active、他项失活、清单取数以当前 root 发起（AC-10） | 新增 |
| 正向 | 详情态（`#/explores/foo`）`nav-explores` 仍 active（前缀派生），点击回落 `#/explores`（AC-10） | 新增 |
| 正向 | 深链 `#/explores/foo` 启动直达详情态、无 NavLink 点击发生（AC-10） | 新增 |
| 正向 | 详情态点击另一 workspace 项 → hash replace 回 `#/explores`、无「新 root + 旧 name」取数（AC-10） | 新增 |
| 边界 | 未知路径 `#/totally-unknown` 仍兜底 `#/changes`（新路由不破坏兜底，AC-10） | 新增 |
| 边界 | `#/explores/a/b` 双段路径：不匹配详情路由，安全兜底不白屏（AC-10） | 新增 |
| 边界 | `changes` / `agent` 页往返后 `nav-explores` 状态始终与 URL 一致（既有 active 语义回归） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri IPC invoke | `vi.mock('@tauri-apps/api/core')` 按命令名分发（`list_workspaces` / `list_explore_records` / `read_explore` / `agent_run_chain` 等），并记录调用序列供次数与参数断言 | 全部用例 |
| 应用级插件 API | `getVersion` / `dialog.open` / updater `check` 固定 resolve，隔离无关分支（既有惯例） | 全部用例 |

### 清单页 → 新建流程 → explore记录CRUD → `packages/desktop/src/__tests__/explore_pages.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/explores/ExploreView.tsx` | 清单页宿主（渲染清单 + 新建入口） |
| `packages/desktop/src/views/explores/components/ExploreCreateDialog.tsx` | 新建流程两入口（导入扫描 / 新话题） |
| `packages/desktop/src/views/explores/hooks/useExploreList.ts` | 取数与动作封装 |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | IPC 对端（mock 边界） |

**关联AC**: AC-8, AC-2

**关系描述**: 清单数据来自 store（非目录扫描），「导入」才触达磁盘扫描，两数据源在页面层汇合。值得测的组合点：清单只呈绑定记录、导入对话框只呈未绑定文件（求差在前端视角的呈现）；新话题建档必须只写 DB——invoke 序列中不得出现任何落盘类命令（应用不落盘 explore.md 的核心验收点）；建档成功后预览/详情呈空态而非报错。可能的出错方式：导入候选误含已绑定文件、建档后列表不刷新、把「无文件」当错误渲染。

#### 场景: 导入扫描绑定

前置：mock invoke 中 `list_explore_records` 返回已绑定 `alpha`、`scan_explores` 返回磁盘上 `alpha`/`beta`/`gamma`。输入：打开新建对话框 → 呈候选 → 选中 `beta` 建档。预期：候选仅 `beta`、`gamma`（已绑定滤除）；选中后发起 `create_explore_record`、成功后清单刷新含 `beta`。

#### 场景: 新话题建档不落盘

前置：mock invoke 全部命令记录调用序列。输入：新话题入口输入主题名建档。预期：仅发起 `create_explore_record`；调用序列无 `read_explore` 写面/文件写入类命令（应用零落盘）；进入该记录详情时 `read_explore` 返回 `null` → 预览空态呈现。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 清单呈现 store 记录（按当前 workspace，`list_explore_records` 入参 root 断言）（AC-8） | 新增 |
| 正向 | 导入候选恰为未绑定差集、选中即建档并刷新（AC-8/AC-2） | 新增 |
| 正向 | 新话题建档：仅 DB 记录命令、无落盘命令出现在调用序列（AC-8） | 新增 |
| 正向 | 新建记录详情：`read_explore` 为空 → 预览空态（不预建空档，AC-8） | 新增 |
| 边界 | 磁盘目录为空（`scan_explores` 空数组）：导入入口空态、无候选可建（AC-2） | 新增 |
| 边界 | `create_explore_record` reject（重复名）：错误可见、对话框不崩、清单不被污染（AC-8） | 新增 |
| 边界 | 空清单 workspace：空态引导呈现（AC-8） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri IPC invoke | 按命令名分发 fixture（`list_explore_records` / `scan_explores` / `create_explore_record` 等），记录全部调用序列供「无落盘命令」序列断言 | 全部用例 |

### 详情双恢复 → 会话链resume发送 → `packages/desktop/src/__tests__/explore_session_pipeline.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/explores/ExploreDetailView.tsx` | 装配方（双栏 + 双恢复调度） |
| `packages/desktop/src/views/explores/hooks/useExploreDoc.ts` | 文档恢复路（磁盘读） |
| `packages/desktop/src/views/explores/hooks/useExploreSession.ts` | 会话恢复与发送路（链重放 + resume） |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | IPC 对端（agent 命令面，mock 边界） |

**关联AC**: AC-9, AC-5, AC-4

**关系描述**: 打开详情时两路恢复并行——文档从磁盘 `read_explore`、对话从 `agent_run_chain` + 逐 run 事件重放，二者互不依赖（数据三分的双恢复语义）；发送时 stance 拼接、链尾 resume、来源三元组三件事在同一 invoke 参数里会合。值得测的组合点：文档缺失（未落盘/已删）时对话仍完整重放；链事件（四变体）与实时流事件在同一气泡区的衔接。可能的出错方式：重放序错乱（按 run id 而非发起序取事件）、resume 取到非链尾 session、文档读取失败阻断会话恢复。

#### 场景: 打开有链记录的双恢复与历史重放

前置：mock `read_explore` 返回文档、`agent_run_chain` 返回 2 条链式 run、`agent_run_events` 返回四变体事件 fixture。输入：进入 `#/explores/foo` 详情。预期：预览呈文档内容、对话区按发起序重放全链气泡（Text/Thinking/ToolUse/ToolResult 四变体齐备）；两路取数各自独立发起。

#### 场景: 发送续话与文档定点重读

前置：同上（链尾 run 带 `session_id = "s-tail"`）。输入：composer 输入并发送。预期：`agent_start` 入参 `prompt` 以 stance 前导开头、`resumeSessionId = "s-tail"`、`source = "explore"`、`sourceRef` = 记录 id 串、`parentRunId` = 链尾 id；run 终态后 `read_explore` 被定点重拉（agent 落盘后预览刷新）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 双恢复并行：`read_explore` 与 `agent_run_chain` 各发一次、互为前置不成立（AC-9） | 新增 |
| 正向 | 全链重放按发起序、四变体气泡齐备（AC-9/AC-5） | 新增 |
| 正向 | 发送拼 stance + resume 链尾 + 三元组（AC-9） | 新增 |
| 正向 | 无链记录发送：不携带 resume，起链后可续话（AC-9） | 新增 |
| 正向 | run 终态定点重读文档：`read_explore` 重发一次（AC-9） | 新增 |
| 边界 | `read_explore` 返回 `null`（文件缺失）：预览空态、对话重放不受影响（双恢复独立性，AC-9） | 新增 |
| 边界 | `AskUserQuestion` ToolUse 出现在链事件中：静态卡片呈现、composer 可继续输入并以 resume 续话（AC-9，MVP 交互闭环） | 新增 |
| 边界 | 双栏拖分界交互可用（resizable 面板宽度变化不重置会话状态）（AC-9） | 新增 |
| 异常 | `agent_start` reject：错误呈现、`running` 复位、历史气泡保留（AC-9） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri IPC invoke | 按命令名分发：`read_explore` / `explore_doc_path` / `watch_subscribe` / `agent_run_chain` / `agent_run_events` / `agent_start`（记录入参 + 返回 run 记录）；事件 fixture 覆盖四变体与 `AskUserQuestion` | 全部用例 |
| 执行流通道（AgentEvent 流） | 捕获 `agent_start` 的 Channel 入参，测试内注入实时事件模拟流式阶段 | 发送与实时累积用例 |

### watch信号 → 防抖 → 显式重拉 → 卸载退订 → `packages/desktop/src/__tests__/explore_watch_refresh.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/explores/ExploreDetailView.tsx` | 生命周期宿主（挂载订阅 / 卸载退订） |
| `packages/desktop/src/views/explores/hooks/useExploreDoc.ts` | 订阅、防抖与显式重拉方 |
| `packages/desktop/src-tauri/src/commands/watch/mod.rs` | IPC 对端（订阅命令面，mock 边界） |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | IPC 对端（`explore_doc_path` / `read_explore`，mock 边界） |

**关联AC**: AC-7

**关系描述**: 第二个被认可的推送语义在此闭环：后端只推「文件被修改」信号（无内容），前端防抖 500ms 后显式 `read_explore` 拉取，订阅生命周期即详情页生命周期。值得测的组合点：信号 → 防抖 → 拉取的节流正确性（多信号合并为一拉）；卸载后退订且无残余拉取（数据面不被幽灵订阅驱动）。可能的出错方式：防抖实现成 leading 导致抖动期多次拉取、卸载漏退订产生跨页信号、`explore_doc_path` 为 `null` 时仍尝试订阅。

#### 场景: 信号合并拉取与卸载退订

前置：进入详情态、订阅已建立（`watch_subscribe` 返回 id=1）。输入：向 Channel 注入 3 帧 `{ path }` → 推进 500ms；随后卸载组件 → 再注入 2 帧。预期：3 帧合并为恰一次 `read_explore` 重拉；卸载后发起 `watch_unsubscribe(1)` 恰一次，后续帧不再引发任何拉取。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 挂载 → `explore_doc_path` → `watch_subscribe` 恰一次，入参路径与派生路径一致（AC-7） | 新增 |
| 正向 | 单帧信号 → 500ms 后恰一次 `read_explore` 重拉（AC-7） | 新增 |
| 边界 | 500ms 窗口内 3 帧 → 合并恰一次重拉（防抖 trailing，AC-7） | 新增 |
| 边界 | 卸载 → `watch_unsubscribe` 以订阅 id 恰发一次、后续帧零拉取（AC-7） | 新增 |
| 边界 | 信号载荷含内容字段（契约破坏注入）：前端仍只按信号处理、不读取内容（通道无内容纪律的防御面，AC-7） | 新增 |
| 边界 | `explore_doc_path` 返回 `null`：零订阅、页面不崩（AC-7） | 新增 |
| 异常 | `watch_subscribe` reject：不阻断文档初始拉取与显式 `refresh`（取数纪律优先于推送，AC-7） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri IPC invoke | 按命令名分发并记录调用：`explore_doc_path` / `watch_subscribe`（返回固定 id）/ `watch_unsubscribe` / `read_explore` | 全部用例 |
| 推送通道（Channel 回调） | 捕获订阅入参回调，测试内按需注入 `FileWatchEvent` 帧模拟后端推送时序 | 合并/退订/防御面用例 |
| 定时器 | `vi.useFakeTimers()` 精确推进 500ms 防抖窗口 | 合并拉取用例 |

---

## 不可测试项

- AC-11（`vp check` / knip / `vp test` / `cargo test --workspace` 全绿、无 knip 豁免新增） — **原因**: 管线门禁类约束，其「全绿」即全部测试的执行结果本身，不存在可独立设计的用例面；由验收阶段以 proposal AC-11 所列管线检查核验（本阶段文档不写入测试命令）。
- `packages/desktop/src/routes.tsx`（路由表增两路由） — **原因**: 单元测试路径解析失败（`test_resolve_paths` 返回 Not in test config scope，路由表文件不在测试配置范围内）；其行为面（可达性、active 派生、兜底）已由集成关系「侧栏导航与路由 → /explores 路由面」全量覆盖。
- shadcn registry 件（`src/components/ui/message-scroller.tsx` / `message.tsx` / `resizable.tsx`） — **原因**: 第三方生成件，不自建测试重复验证库自带滚动/分栏语义（既有纪律）；装配正确性由集成关系 F3/F4 的页面级断言间接覆盖。
- 壳层装配与依赖清单（`src-tauri/src/main.rs` 命令注册、`src/commands/mod.rs` 轨道导出、`src-tauri/Cargo.toml` workspace member 与依赖、`package.json` / `components.json`） — **原因**: 编译期/装配期改动，注册缺漏在编译与应用启动期即暴露，由管线门禁兜底；mock 边界的集成测试无法真实覆盖壳层注册面。
- CLI `--resume` 的 fork/延续语义 E2E 结论 — **原因**: 需真实 CLI 会话实测（proposal 风险表已列，design 待决问题收口），不在自动化用例面；链模型以 `parent_run_id` 显式指针免疫该歧义，链还原正确性由集成关系「链式run落库 → restore_run_chain还原」覆盖。
- stance 模板文案与 `SKILL.md` 的口径一致性 — **原因**: 双源漂移为提案已拍板接受的设计代价（模板归属 desktop 包），一致性属人工评审面而非可断言语义；`buildExplorePrompt` 的拼接结构行为已由其单元章覆盖。

