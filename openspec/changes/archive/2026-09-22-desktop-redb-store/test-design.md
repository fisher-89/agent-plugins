# 测试设计: desktop-redb-store

> **日期**: 2026-09-21

---

## 验收范围

<!-- 测试框架口径（由 test_detect_frameworks 识别）：
  - Rust 侧：src-tauri 根 workspace 套件，colocated `*_test.rs` + `#[test]`（沿用 queries/mod_test.rs、list_test.rs 既有模式），新 crate 经 `[workspace].members` 自动收编（AC-10）
  - 前端侧：vite-plus（`vite-plus/test` 的 vitest 接口 + testing-library），沿用 useChangeList.test.ts / App.test.tsx 既有 mock IPC 模式
  本变更无既有 test-design.md，全部用例为新增；唯一被修改的既有测试文件是 App.test.tsx，其受 UI 流改造影响的用例以「废弃」标记。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 新增 `crates/infra/store` crate：落位并加入 `[workspace].members`；`Cargo.toml` 无 Tauri 依赖、无 core crate 依赖 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`（crate 随 workspace 测试套编译并运行即 members 收编证据；依赖约束为静态清单，见不可测试项） |
| AC-2 | `Store::open(path)` + add/list/remove/touch 四操作；tempdir 集成测试覆盖全链路，重开同一 db 文件后记录仍在 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` |
| AC-3 | canonical path 作 key 的 upsert 去重：等价路径（大小写/尾分隔符等书写差异）重复 add 仅一条记录且 `last_opened_at` 刷新；等价路径 touch/remove 命中同一条 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` |
| AC-4 | `list_workspaces` 按 `last_opened_at` 降序，第一名即最近打开；`name` 为目录名最后一段 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` |
| AC-5 | META 表 schema_version：新开 db 即写入；业务表命名含 user 维度语义 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` |
| AC-6 | store 公共 API、desktop-app 命令签名、前端 DTO 中均无 redb 类型 | 单元测试 | `packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs`（命令返回值 serde 形状对比锁定序列化面）；签名字面约束由 D6 可见性编译期保证（见不可测试项） |
| AC-7 | workspace 四命令注册于 invoke_handler，经 `State<Store>` 访问 store；失败路径返回 `Err(String)` 并在前端 error 态呈现 | 单元测试 | `packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs`；前端 error 态呈现半边由集成测试 `packages/desktop/src/__tests__/workspace_restore.test.tsx` 承接 |
| AC-8 | db 文件打不开时启动失败并报错，无静默空清单降级 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`（`Store::open` 失败路径）；启动装配链见不可测试项 |
| AC-9 | 有记录时启动自动恢复 `last_opened_at` 第一名；无记录停欢迎屏；下拉切换清空 change 选中并按新根取数；移除当前打开项切换到剩余第一名（清单空时回欢迎屏） | 集成测试 | `packages/desktop/src/__tests__/workspace_restore.test.tsx`（壳 × hook × 命令面时序契约）；视图状态断言由 `packages/desktop/src/App.test.tsx` 单测承接 |
| AC-10 | 不修改 `openspec/config.json` 套件注册，新 crate 测试被 `cargo test --workspace` 覆盖 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`（store crate 单元测试随 `cargo test --workspace` 编译并运行，即 members 收编证据——与 AC-1 同一覆盖路径；不挂集成关系：既有两关系分属已收编的 desktop-app crate 与前端 mock 组合，均非 AC-10 实体；config.json 零改动由实现自检承接，见不可测试项） |

---

## 单元测试

<!-- Rust 侧为 colocated `#[cfg(test)]` 模块文件（`*_test.rs`），前端侧为 vitest 纯函数/组件 mock 测试。
  每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节。
  store crate 为 D6 私有模块结构（公共面仅 `Store` / `StoreError` / `WorkspaceRecord` 三名字），
  其内部协作（model 编解码、canonical 口径）一律经 `Store` 公共 API 覆盖，不为其单独开测试导出（符合仓库「不为测试增加导出」规则）。 -->

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- store crate 核心被测面：四操作 + open + StoreError，tempdir 真开 redb 文件。 -->

#### 待测功能

<!-- 来源：design.md「公共函数 / API」表（所在文件 = store.rs 的行） -->

- Store::open(): 打开（不存在则创建）db：`create_dir_all` 父目录 + `Database::create` + META 写入/校验 `schema_version`；打不开即 Err
- Store::add_workspace(): canonicalize + upsert + touch，返回落库后的 `WorkspaceRecord`
- Store::list_workspaces(): `last_opened_at` 降序（并列按 root 字典序升序）返回清单
- Store::remove_workspace(): 按 canonical key 删除（含消失目录词法回退匹配），miss 幂等 `Ok(false)`
- Store::touch_workspace(): 刷新 `last_opened_at`，miss 幂等 `Ok(false)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| Store::open | 正向 | 对不存在的 tempdir 子路径 open：返回 Ok，db 文件与父目录被创建（AC-2/AC-5） | 新增 |
| Store::open | 正向 | 已有 db 文件再次 open：返回 Ok，且此前经公共 API 写入的 workspace 记录完整读回（重开持久化，AC-2） | 新增 |
| Store::open | 异常 | 父路径被同名普通文件占据（create_dir_all 无法建目录）：返回 `Err(StoreError::Db)` 且错误串以 `db:` 前缀，不 panic（AC-8） | 新增 |
| Store::open | 异常 | 目标路径为写入任意字节的损坏文件（非合法 redb 文件）：open 返回 Err 而非静默降级为空库（AC-8） | 新增 |
| Store::open | 边界 | db 内 `schema_version` 等于当前 `SCHEMA_VERSION`：open 幂等成功（AC-5） | 新增 |
| Store::open | 边界 | db 内 `schema_version` 大于当前支持版本：open 返回 Err（版本校验，为将来迁移留位，AC-5） | 新增 |
| Store::open | 边界 | 全新库 open 后经 redb 只读句柄读 `user_meta` 表：`schema_version` 恰为当前版本值（AC-5） | 新增 |
| Store::add_workspace | 正向 | 对新目录 add：返回记录 `root` 为 canonical 完整路径、`name` 为目录名最后一段、`added_at` 与 `last_opened_at` 同值（AC-4/D8） | 新增 |
| Store::add_workspace | 正向 | 以大小写不同的等价路径二次 add 同一目录：清单仅一条记录，`last_opened_at` 刷新且 `added_at` 保留首添值（AC-3/D8） | 新增 |
| Store::add_workspace | 边界 | 尾分隔符、正反斜杠混写等书写差异路径 add：与已存 canonical key 去重为一条（AC-3） | 新增 |
| Store::add_workspace | 边界 | 目录名含空格、中文、emoji 的路径 add：`name` 提取正确且记录 JSON 编解码往返无损（字符串特殊字符边界） | 新增 |
| Store::add_workspace | 异常 | 不存在的目录路径 add：`Err(StoreError::Canonicalize)` 且错误串以 `canonicalize:` 前缀（D1 失败语义） | 新增 |
| Store::add_workspace | 异常 | 空路径 add：`Err(StoreError::Canonicalize)`，不 panic（字符串空值边界） | 新增 |
| Store::list_workspaces | 正向 | 依次 add/touch 多个 workspace 后 list：顺序与 `last_opened_at` 降序一致，第一名即最近 touch 的（AC-4） | 新增 |
| Store::list_workspaces | 边界 | `last_opened_at` 并列（同毫秒连续操作）：并列项按 `root` 字典序升序，结果顺序确定可复现（D8 tie-break） | 新增 |
| Store::list_workspaces | 边界 | 空库 list：返回空向量不报错（容器空边界；AC-9「无记录停欢迎屏」的上游语义） | 新增 |
| Store::touch_workspace | 正向 | 对已存在 key touch：`Ok(true)` 且 `last_opened_at` 不减（AC-3） | 新增 |
| Store::touch_workspace | 边界 | 未注册路径 touch：`Ok(false)` 幂等 miss，不产生新记录（D8） | 新增 |
| Store::touch_workspace | 边界 | 目录已从磁盘删除后以原路径 touch：canonicalize 失败走词法归一化回退，命中存量 key 并刷新（D1 回退口径） | 新增 |
| Store::touch_workspace | 异常 | 空路径 touch：回退匹配不命中，`Ok(false)` 幂等（空串边界穿透回退路径） | 新增 |
| Store::remove_workspace | 正向 | 对已存在 key remove：`Ok(true)`，list 不再含该项（AC-3） | 新增 |
| Store::remove_workspace | 边界 | 未注册路径 remove：`Ok(false)`，库内容不变（D8） | 新增 |
| Store::remove_workspace | 边界 | 以大小写不同的等价路径 remove：命中并删除同一条，无孤儿条目（AC-3 全链路同源） | 新增 |
| Store::remove_workspace | 边界 | 目录消失后 remove：回退匹配命中删除，残留清单项可被清理（D1 用户救济路径） | 新增 |
| StoreError | 边界 | `Db` / `Canonicalize` 两变体的 Display 输出分别携带 `db:` / `canonicalize:` 前缀，错误可排查（D5） | 新增 |
| store 全链路（tempdir 重开持久化） | 正向 | 单库依次 add/list/touch/remove 后 drop 并重开同一 db 文件再 list：清单状态与各操作返回一致（AC-2 提案「tempdir 全链路」场景） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件系统目录 | dev-dependency `tempfile` 提供真实 tempdir 与 db 文件路径，测试结束自动清理（沿用既有 TempWs RAII 装置模式）；存储层不 mock，真开 redb 文件 | 全部用例 |
| redb 只读检查 | 测试内以 `redb::Database::open_read_only` 直读 `user_meta` 断言 `schema_version`——仅作为检查手段，redb 自身事务/持久化语义不在断言范围 | Store::open 的 schema_version 三例 |
| 系统时钟 | 不 mock：设计未注入时钟（D4 真实毫秒时间戳），为测试注入时钟属投机抽象；同毫秒并列场景以 tie-break 与「不减」断言表述 | list_workspaces 排序两例、add_workspace 刷新断言 |

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/lib_test.rs

<!-- design.md 未声明该文件的公共 API 变更：lib.rs 仅为 crate 入口（私有模块声明 + D6 的 `pub use` 三名字收敛），
  无独立运行时行为；公共面结构约束（AC-6）由可见性设计在编译期机械保证，行为覆盖经 Store 公共 API 在 store_test.rs 承接。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- design.md 未声明该文件的公共 API 变更（D6：model 为私有模块，`WorkspaceRecord::from_root` 等内部机制不升公共 API）。
  name 提取与 JSON 编解码行为经 Store 公共 API 在 store_test.rs 覆盖（AC-4 的 name 断言、特殊字符往返），
  serde camelCase 序列化形状在命令层 serde 对比中锁定（AC-6）。本章节不设用例，符合「不为测试增加导出」的仓库规则。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src-tauri/crates/infra/store/src/canonical.rs -> packages/desktop/src-tauri/crates/infra/store/src/canonical_test.rs

<!-- design.md 未声明该文件的公共 API 变更（D1/D6：`canonical_key` 为内部机制，不进公共面）。
  canonical 口径（dunce 主口径、大小写不敏感、尾分隔符归一、消失目录词法回退）作为 store 四操作的输入路径变体
  在 store_test.rs 中覆盖（AC-3 全部用例）。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/commands/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更：仅增 `pub mod workspaces;` 挂载声明，queries/exec 双轨语义不变；
  三轨挂载由 workspaces 轨道用例的编译与运行隐式覆盖。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src-tauri/src/commands/workspaces/mod.rs -> packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs

<!-- 命令轨道为薄包装：State 取 store + String→Path 参数转换 + StoreError→String 错误映射。
  `#[tauri::command]` 保留原函数可直调，测试不启动 Tauri runtime（沿用 queries/mod_test.rs 模式）。
  本文件同时承载「workspace命令面 → Store持久化」集成关系（见集成测试章节，binary crate 无库目标，集成用例与此共置）。 -->

#### 待测功能

<!-- 来源：design.md「公共函数 / API」表（所在文件 = src/commands/workspaces/mod.rs 的行）；私有 `*_inner` 不列入 -->

- list_workspaces(): `State<'_, Store>` 取 store 透传降序清单，`Result<Vec<WorkspaceRecord>, String>`
- add_workspace(): String root 转 Path 调 store，返回 canonical 记录
- remove_workspace(): 转发删除，返回是否命中 bool；失败路径 `Err(String)`
- touch_workspace(): 转发刷新，返回是否命中 bool；失败路径 `Err(String)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| workspace 命令轨道 | 正向 | tempdir 真开 Store 后四命令结果与直连 store 公共 API 的结果经 serde 值对比一致：薄包装不加工 DTO（AC-7） | 新增 |
| workspace 命令轨道 | 正向 | add_workspace 传入书写不等价的 String root：返回记录的 root 与库内 canonical key 同源（AC-3/AC-7） | 新增 |
| workspace 命令轨道 | 边界 | 未注册 root 的 remove/touch：`Ok(false)` 幂等语义穿透命令面（D8） | 新增 |
| workspace 命令轨道 | 异常 | add_workspace 传入不存在的目录：`Err(String)` 且含 `canonicalize:` 前缀——错误约定模板的首个实例（AC-7） | 新增 |
| workspace 命令轨道 | 异常 | 空字符串 root：add 为 `Err`，touch/remove 为 `Ok(false)`——三命令对同一非法入参语义一致（字符串空值边界） | 新增 |
| workspace 命令轨道 | 边界 | add_workspace 返回值经 serde_json 序列化：顶层键恰为 `root` / `name` / `addedAt` / `lastOpenedAt`（camelCase），无 redb 概念泄漏到序列化面（AC-6） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | 不启动 Tauri runtime：以 `State::from(&store)` 从真实 Store 引用直接构造 State（运行环境装置，非被测对象 mock） | 全部用例 |
| 临时文件系统目录 | dev-dependency `tempfile` 提供 tempdir 与 db 路径（desktop-app 新增 dev-dependencies 的用途） | 全部用例 |

---

### packages/desktop/src-tauri/src/main.rs -> packages/desktop/src-tauri/src/main_test.rs

<!-- design.md 未声明该文件的公共 API 变更：main.rs 为启动装配层（home_dir 解析 + Store::open + .manage() + 命令注册），
  无可单测的导出函数。其可自动化的上游失败路径（Store::open 打不开返回 Err、不静默空库）由 store_test.rs 承接（AC-8）；
  完整启动 fail-fast 链见「不可测试项」。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src/App.tsx -> packages/desktop/src/App.test.tsx

<!-- design.md 未声明 App.tsx 的公共 API 变更（壳为组合层，D7）。
  本章节承载两层内容：(1) 既有 App.test.tsx 五例受「选择与恢复」UI 流改造影响，以「废弃」标记；
  (2) AC-9 的视图状态断言（proposal 指定 App.test.tsx 为落点）以「新增」标记——IPC 全 mock 的壳状态机测试。
  invoke 时序与次数契约由集成测试 workspace_restore.test.tsx 承接，两文件断言焦点不同、不重复。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App 壳：选择与恢复 | 正向 | 既有用例「未选 workspace 时显示选择入口；选定后以该 root 进入列表视图」：选择入口由欢迎屏清单 + 添加流取代，断言前提失效 | 废弃 |
| App 壳：选择与恢复 | 边界 | 既有用例「dialog 取消（返回 null）时保持未选 workspace 状态，不发起取数」：启动自动 load 使「invoke 零调用」前提失效（list_workspaces 挂载即调用） | 废弃 |
| App 壳：选择与恢复 | 异常 | 既有用例「dialog 调用 reject 时界面保持可选状态不白屏」：同上前提失效，语义由「添加 reject 置 error 态」新例承接 | 废弃 |
| App 壳：选择与恢复 | 正向 | 既有用例「列表项点击后切换到详情视图，root 与 change 名下发正确」：setup 流变更（选择入口 → 恢复/清单），断言语义由新例承接 | 废弃 |
| App 壳：选择与恢复 | 正向 | 既有用例「刷新动作触发 useChangeList 的 refresh 回调」：setup 流变更，断言语义由新例承接 | 废弃 |
| App 壳：选择与恢复 | 边界 | `list_workspaces` 返回空清单启动：呈现欢迎屏空态与「添加新文件夹」入口，不进入列表视图（AC-9） | 新增 |
| App 壳：选择与恢复 | 正向 | 有记录启动：自动恢复 `last_opened_at` 第一名为当前根并进入列表视图（AC-9） | 新增 |
| App 壳：选择与恢复 | 正向 | Header 下拉切换：change 选中清空、以新根重取列表、清单项悬停 title 含完整 path（AC-9） | 新增 |
| App 壳：选择与恢复 | 正向 | 移除当前打开的清单项：切换到剩余第一名，被移除项从下拉消失（AC-9） | 新增 |
| App 壳：选择与恢复 | 边界 | 添加对话框取消（返回 null）：不调用 `add_workspace`、停留欢迎屏（重写自废弃例 2） | 新增 |
| App 壳：选择与恢复 | 异常 | 添加对话框 reject 或 `add_workspace` reject：error-note 呈现、停留欢迎屏、可重试（重写自废弃例 3） | 新增 |
| App 壳：选择与恢复 | 正向 | 恢复进入列表后：列表项点击进入详情视图、刷新按钮重发 `list_changes`（既有语义在恢复流下的承接，重写自废弃例 4/5） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 接口 | `vi.mock('@tauri-apps/api/core')` 以 invokeMock 按命令名分发（`list_workspaces` / `list_changes` / `get_change_detail` / `read_artifact` / `add_workspace` / `remove_workspace` / `touch_workspace`），沿用既有 mockIpc 模式扩展 workspace 四命令 | 全部用例 |
| 文件夹选择对话框 | `vi.mock('@tauri-apps/plugin-dialog')` 以 openMock 按用例 resolve 路径 / null / reject | 添加流用例 |

---

### packages/desktop/src/hooks/useWorkspaces.ts -> packages/desktop/src/hooks/useWorkspaces.test.ts

#### 待测功能

<!-- 来源：design.md「公共函数 / API」表 + 「类型定义」表 WorkspaceState 契约 -->

- useWorkspaces(): 清单取数收口：挂载自动 load 一次（唯一自动取数例外）；add/remove/touch 成功后内部刷新清单；返回 `WorkspaceState`
- WorkspaceState.add(root): 成功返回 `WorkspaceRecord`，失败返回 null 并置 error 态
- WorkspaceState.remove(root): 成功/miss 返回 boolean，失败返回 false 并置 error 态
- WorkspaceState.touch(root): 同 remove 语义

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useWorkspaces：清单取数收口 | 正向 | 挂载即自动 invoke `list_workspaces` 恰一次，resolve 后 `workspaces` 更新、`loading` 复位（AC-9 前置） | 新增 |
| useWorkspaces：清单取数收口 | 异常 | `list_workspaces` reject：error 置位含错误串、`workspaces` 保持空数组、不抛未捕获异常（AC-7 前端半边） | 新增 |
| useWorkspaces：清单取数收口 | 正向 | add 成功：invoke `add_workspace` 返回记录后内部刷新清单（`list_workspaces` 再次调用）（design hook 契约） | 新增 |
| useWorkspaces：清单取数收口 | 异常 | add reject：返回 null 且 error 置位（WorkspaceState 契约） | 新增 |
| useWorkspaces：清单取数收口 | 边界 | add 传入空字符串 root：参数原样 invoke、失败置 error 不崩溃（字符串空值边界） | 新增 |
| useWorkspaces：清单取数收口 | 正向 | remove 成功：返回 true 且清单内部刷新 | 新增 |
| useWorkspaces：清单取数收口 | 边界 | remove resolve(false)（store miss）：返回 false 且不置 error——幂等 miss 非错误的语义穿透前端（D8） | 新增 |
| useWorkspaces：清单取数收口 | 异常 | remove reject：返回 false 且 error 置位 | 新增 |
| useWorkspaces：清单取数收口 | 正向 | touch 成功：返回 true 且清单内部刷新（design hook 契约） | 新增 |
| useWorkspaces：清单取数收口 | 边界 | touch reject：返回 false 且 error 置位，不阻塞后续 add/remove 动作 | 新增 |
| useWorkspaces：清单取数收口 | 边界 | 无轮询无 watch：fake timers 推进数分钟后 invoke 次数不增长（显式刷新纪律，spec「无 watch 依赖」场景；沿用 useChangeList.test.ts 的虚拟计时模式） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 接口 | `vi.mock('@tauri-apps/api/core')` 以 invokeMock 替身，按命令名 mockResolvedValue / mockRejectedValue（沿用 useChangeList.test.ts 的 vi.hoisted 模式） | 全部用例 |

---

### packages/desktop/src/types/dto.ts -> packages/desktop/src/types/dto.test.ts

<!-- design.md 未声明该文件的公共 API 变更：新增纯类型 interface（运行时无行为，编译期即保证），
  不设运行时用例；DTO 字段与 Rust 模型的对应性由命令层 serde 形状对比（workspaces/mod_test.rs，AC-6）
  与前端用例的 mock 数据形状间接锁定。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src/views/WelcomeView.tsx -> packages/desktop/src/views/WelcomeView.test.tsx

<!-- design.md 未声明该文件的公共 API 变更（D7：视图组件，与 ChangeListView 同型，无独立导出函数）。
  欢迎屏清单渲染、添加入口、移除按钮与 error-note 行为经 App 壳测试（App.test.tsx 与集成测试
  workspace_restore.test.tsx）以真实组合方式覆盖，避免为组件单测另设 hook mock 缝。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释 -->

#### Mock策略

<!-- 无 -->

---

## 集成测试

<!-- 集成测试验证跨模块交互，聚焦模块组合时才暴露的行为。
  Rust 侧注意：desktop-app 为纯 binary crate（无 lib 目标），`tests/` 目录无法链接命令模块，
  故「workspace命令面 → Store持久化」关系的集成用例按仓库既有模式与命令单测共置于 commands/workspaces/mod_test.rs；
  前端侧集成文件按仓库既有约定置于 `src/__tests__/`（同 ipc_pipeline.test.tsx）。 -->

### workspace命令面 → Store持久化 → `packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/src/commands/workspaces/mod.rs` | 触发方（IPC 命令薄包装：String→Path 参数转换 + StoreError→Err(String) 映射） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 持久化方（redb 读写、upsert/排序/幂等语义） |
| `packages/desktop/src-tauri/crates/infra/store/src/canonical.rs` | 中间件（key 口径：dunce canonical 主口径 + 消失目录词法回退） |

**关联AC**: AC-2, AC-3, AC-7

**关系描述**:

命令轨道是 store 公共 API 与前端 IPC 之间唯一的组装点：前端回传的 root 是 list 结果中的 canonical root 字符串，经「命令 String→Path 转换 → store 再 canonicalize/回退匹配 → 表 key」三层后必须仍是同一个 key。任何一层口径不一致（命令转换丢信息、store 二次 canonicalize 出不同形态）都表现为 touch/remove 静默 miss 或重复记录，且单看任一模块的单测都发现不了——这正是 canonical 口径必须在组合层端到端锁定的原因（proposal 风险「Windows canonicalization 口径」的测试落点）。此外，`Err(String)` 错误约定在跨层传递时的前缀保真（`db:` / `canonicalize:`）也只有在 store 真实失败、命令层映射后才能观察。此关系以 tempdir 真开 redb 文件跑全链路，被替换的只有 Tauri runtime 本身。

#### 场景: tempdir 全链路经命令面（add → list → touch → remove → 重开）

前置：tempdir 打开 Store 并以 `State::from(&store)` 包装。验证命令层透传不加工、持久化跨 Store 实例存活。输入为命令面 String root，预期输出与直连 store 公共 API 的结果 serde 值一致。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 经命令面依次 add → touch → list：顺序与 `last_opened_at` 降序一致，记录字段与直连 store 的返回一致（AC-2/AC-7） | 新增 |
| 正向 | remove 命中 true 后 list 不含该项；drop Store 并重开同一 db 文件再经命令面 list：结果与删除后状态一致（AC-2 提案「重开持久化」场景） | 新增 |
| 边界 | 空库经命令面 list 返回空数组（AC-2；AC-9「无记录停欢迎屏」的上游） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri 运行环境 | 不启动 Tauri runtime：`State::from(&store)` 从真实 Store 引用直接构造 State（`#[tauri::command]` 保留原函数可直调） | 本关系全部场景 |
| 临时文件系统目录 | dev-dependency `tempfile` 提供 tempdir 与 db 文件路径 | 本关系全部场景 |

#### 场景: 等价路径去重与命中经命令面

前置：tempdir Store 已含一个 canonical root 记录。以书写不等价的 String root（大小写差异、尾分隔符）走命令面，验证三层口径合成后仍是同一 key。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 以大小写不同的 String root 二次 add_workspace：list 仅一条记录且 `last_opened_at` 刷新（AC-3） | 新增 |
| 边界 | 以等价路径 touch/remove：均命中同一条，无孤儿条目（AC-3；模拟前端回传 canonical root 的真实形态） | 新增 |

##### Mock策略

<!-- 共用场景 1 的 Mock 装置，无额外 Mock -->

#### 场景: 错误约定跨层穿透

前置：tempdir Store。以 store 必然失败的入参驱动，验证 StoreError 经命令层映射为 `Err(String)` 且前缀保真。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | add_workspace 传入不存在的目录：`Err(String)` 且含 `canonicalize:` 前缀（AC-7 错误约定模板） | 新增 |
| 异常 | 空字符串 root：add 为 `Err`、touch/remove 为 `Ok(false)`——三命令对同一非法入参语义一致（AC-7，字符串空值边界） | 新增 |

##### Mock策略

<!-- 共用场景 1 的 Mock 装置，无额外 Mock -->

---

### useWorkspaces启动取数 → App自动恢复 → workspace命令面 → `packages/desktop/src/__tests__/workspace_restore.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/hooks/useWorkspaces.ts` | 触发方（清单取数收口：挂载自动 load + 恢复 touch 一次守卫 + add/remove/touch 动作与内部刷新） |
| `packages/desktop/src/App.tsx` | 消费方/中间件（下拉切换、移除切换剩余第一名的壳状态机） |
| `packages/desktop/src-tauri/src/commands/workspaces/mod.rs`（经 invoke IPC 边界） | 读取方/写入方（list/add/touch/remove 四命令，测试中以 mock 边界替身） |
| `packages/desktop/src/views/WelcomeView.tsx` | 呈现方（欢迎屏空态、添加入口、error-note） |

**关联AC**: AC-9, AC-7

**关系描述**:

自动恢复是本变更唯一一处跨多个 render 周期与多个模块的时序行为：hook 挂载自动 load、取得首个非空清单即 fire-and-forget touch 第一名（先于 setRoot 派发）、随后既有 useChangeList/useChangeDetail 按新根取数——三段时序只有在组合时才可观察。最易出错的组合点是恢复守卫：touch 触发的内部刷新会再次取得非空清单，若无一次守卫（restoredRef）将形成取数-恢复循环（design「前端交互时序」第 2 点）；hook 单测各自 mock 后只能验证局部状态，无法暴露守卫失效。此关系以 mock IPC 边界驱动真实 App 组合，断言 invoke 的时序、次数与参数契约；视图层的界面状态断言归 App.test.tsx 单测，两者不重复。

#### 场景: 启动恢复链

前置：mock `list_workspaces` 返回按降序排列的两条记录，render 真实 App。验证恢复链的触发顺序与参数：恰一次 list → touch（fire-and-forget，不阻塞后续取数）→ `list_changes` 以恢复根触发。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 有记录启动：自动恢复 `last_opened_at` 第一名为当前根，`list_changes` 收到 `{ root: 第一名.root }`（AC-9） | 新增 |
| 正向 | 恢复触发恰一次 `touch_workspace(first.root)`，且 touch 失败（reject）不阻断 `list_changes` 取数（D3 fire-and-forget） | 新增 |
| 异常 | `list_workspaces` reject：无 touch、无 list_changes 取数，error 呈现且欢迎屏手动添加路径仍可用（design 时序 3「恢复失败不阻断手动添加」） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| IPC 进程边界（invoke） | `vi.mock('@tauri-apps/api/core')`：invokeMock 按命令名分发固定 fixture（`list_workspaces` 返回降序清单、`add_workspace` 返回 canonical 记录），并按用例切换 reject | 本关系全部场景 |
| 文件夹选择对话框 | `vi.mock('@tauri-apps/plugin-dialog')`：open 按用例 resolve 路径或 null | 「添加与下拉切换链」场景 |

#### 场景: 无记录停欢迎屏

前置：mock `list_workspaces` 返回空数组。验证无记录时零后续取数。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | 空清单启动：不触发 touch 与 `list_changes`，呈现欢迎屏与「添加新文件夹」入口（AC-9） | 新增 |

##### Mock策略

<!-- 共用「启动恢复链」场景的 Mock 装置，无额外 Mock -->

#### 场景: 移除当前项（切换剩余第一名且不二次恢复）

前置：App 已自动恢复第一名。经 Header 移除当前项，mock 的 `list_workspaces` 在 hook 内部刷新后再次返回剩余清单。验证守卫 ref 使恢复 touch 恰好发生一次。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 移除当前打开项：`remove_workspace` 被调用后切换到剩余第一名并以其为根重取列表（AC-9） | 新增 |
| 边界 | 移除后 hook 内部刷新取得剩余清单：不发生第二次自动恢复 touch，`list_changes` 恰以剩余第一名追加一次（一次守卫，design 时序 2） | 新增 |

##### Mock策略

<!-- 共用「启动恢复链」场景的 Mock 装置，无额外 Mock -->

#### 场景: 添加与下拉切换链

前置：欢迎屏或已恢复状态。验证添加流以 canonical root 打开、切换流清空 change 选中并以新根取数。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 欢迎屏添加：dialog resolve 路径 → `add_workspace` → 以返回记录的 canonical root 触发 `list_changes`（design 时序 6：Header 展示与库内 key 同源） | 新增 |
| 正向 | 下拉切换另一项：`touch_workspace(新根)` + change 选中清空 + `list_changes` 以新根重取（AC-9/design 时序 4） | 新增 |
| 边界 | dialog 取消（返回 null）：不调用 `add_workspace`、停留欢迎屏（design 时序 3 的添加分支） | 新增 |

##### Mock策略

<!-- 共用「启动恢复链」场景的 Mock 装置（含 dialog open mock），无额外 Mock -->

---

## 不可测试项

<!-- 列出 proposal 范围内但无法通过自动化测试验证的条目，每项说明原因。 -->

- main.rs 启动 fail-fast 全链（setup 返回 Err → run 返回 Err → 进程报错退出，AC-8 的装配半边） — **原因**: 需真实 Tauri 运行时与应用窗口生命周期，rust 测试进程无法启动桌面应用；其可自动化的上游失败路径（`Store::open` 打不开返回 `Err`、不静默降级为空库）已在 store_test.rs 锁定，装配链本身以实现评审承接。
- store `Cargo.toml` 零 Tauri、零 core crate 依赖（AC-1 的依赖约束子句） — **原因**: 依赖清单为静态结构约束，无运行时断言形态；由 crate 图与实现审查承接（store 随 workspace 测试套编译运行是其「成员收编」的自动化证据）。
- store 公共签名与前端 DTO 字面的「无 redb 类型」（AC-6 的签名字面子句） — **原因**: 类型签名是编译期结构约束，无运行时断言形态；D6 可见性收敛（私有模块 + 三名字 `pub use`）使其由编译机械保证，其可观测残余（命令返回值 serde 序列化形状恰为四字段 camelCase）已由 workspaces/mod_test.rs 用例锁定。
- `user_workspaces` / `user_meta` 表命名字符串与 lib.rs 单进程约束文档声明（AC-5 命名子句、spec「约束声明可见」场景） — **原因**: 命名约定与文档声明无可断言的运行时行为；表存在性经 schema_version 只读检查与四操作成功间接证实，双开风险声明以 crate 文档评审承接。
- redb 库自带语义（事务原子性、多表隔离、崩溃恢复、文件格式持久化保证） — **原因**: 按既定测试口径仅测自研层（record 编解码、canonical 去重、排序组装、命令映射），库自身语义不逐项验证；tempdir 真开 redb 文件已覆盖自研组装正确性。
- 双开数据竞争（dev 与正式版指向同一 home_dir db 文件） — **原因**: 需两个真实 OS 进程并发写同一文件，超出 rust/前端测试套能力；redb 单进程模型为已知约束，以 lib.rs 文档显式声明，进程单实例治理（tauri-plugin-single-instance）不进本变更。
- `openspec/config.json` 零改动（AC-10 的一半） — **原因**: 静态文件不变式无测试断言形态，由实现任务显式自检承接；另一半（新 crate 测试被 workspace 套件覆盖）由 rust 套件运行自动证明，无需独立用例。
