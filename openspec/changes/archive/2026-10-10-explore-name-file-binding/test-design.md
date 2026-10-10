# 测试设计: explore-name-file-binding

> **变更**: explore-name-file-binding
> **日期**: 2026-10-10
> **依据**: proposal.md（AC-1..AC-10）+ design.md（D1–D9 决策编号）；tasks.md 六阶段内的「编译面机械随动」只保编译不写断言，本文件展开全部新增 / 重写 / 适配锚（test-gen / test-execution 阶段承接）

---

## 验收范围

<!-- 逐条映射 proposal.md 的 10 条 AC。被测文件或模块 = design.md「变更清单 - 实现文件」中承载该 AC
     断言的源文件（被测模块，非测试文件）；源文件到测试文件的映射见「单元测试」per-file 章节。
     纯静态 / 流程性半边（工具链门 / 版本声明 / 生成物 / 纯 re-export / 零触点红线）落「—（见不可测试项 N）」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | ExploreRecord v2 升级：含 v1 记录的存量库 additive 打开，旧记录读出 `title = name`、`promoted_to = None`（decode-only 零手工迁移）；新记录 `title` / `promoted_to` 回环逐字段一致；`list_models` / `scan` 信封 API 零改动覆盖 | packages/desktop/src-tauri/crates/infra/store/src/model.rs（v1→v2 decode-only 与回环）；packages/desktop/src-tauri/crates/infra/store/src/store.rs（建档 / 回环 / 信封零改动） |
| AC-2 | ChangeRecord v4 升级：含 v3 记录的存量库 additive 打开，旧记录读出 `title = name`；`change_state` 映射与建档回环一致；change 清单 / 详情出线 title | packages/desktop/src-tauri/crates/infra/store/src/model.rs（v3→v4 decode-only）；packages/desktop/src-tauri/crates/infra/store/src/store.rs（change_state / create_change_record 映射）；packages/desktop/src-tauri/crates/core/workflow/src/state.rs（ChangeStateRecord 字段面）；packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs（create title 入参与建档载荷）；packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs（ChangeSummary.title）；packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs（ChangeDetail.title） |
| AC-3 | explore 名称 kebab 口径：`create_explore_record` / `rename_explore_record` 拒绝大写 / 下划线 / 空格 / 前导数字等非法 kebab；导入扫描不列非 kebab stem；存量非 kebab 记录仍可读、promote 显式拒绝 | packages/desktop/src-tauri/crates/infra/store/src/store.rs（kebab 校验与存量可读）；packages/desktop/src-tauri/src/commands/explores/mod.rs（scan 过滤与 promote 拒绝）；packages/desktop/src/views/explores/components/explore-create-dialog.tsx（本地 kebab 校验） |
| AC-4 | stance 注入 name：任一 explore run 的 prompt 原文含当前记录 name（落盘文件名 MUST 等于 name）与「首行写 `# 标题`」约定；调试页 run prompt 不含该模板 | packages/desktop/src/lib/explore-stance.ts（模板注入）；packages/desktop/src/views/explores/hooks/use-explore-session.ts（send 组装）；调试页 run 零触点 → 不可测试项 6 |
| AC-5 | title 回填：笔记首行 `# 标题` 与 record.title 不同时，watch 重读后经 `update_explore_title` 回填；纯读路径（read_explore）零写入；回填成功后清单与详情头 title 一致 | packages/desktop/src/lib/explore-title.ts（标题解析）；packages/desktop/src/views/explores/explore-detail-view.tsx（回填 effect）；packages/desktop/src-tauri/src/commands/explores/mod.rs（update_explore_title 命令）；packages/desktop/src-tauri/crates/infra/store/src/store.rs（set_explore_title）；read_explore 纯读零写入 → 不可测试项 6 |
| AC-6 | promote move 语义：合法 kebab explore（笔记已落盘且非空白）promote 成功——worktree 内 `changes/<name>/explore.md` 为笔记全文、ChangeRecord.title = explore.title、主仓 `explores/<name>.md` 已删、ExploreRecord.promoted_to = change.id、名下会话链保留（记录不删） | packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs（写面 move 半边）；packages/desktop/src-tauri/src/commands/explores/mod.rs（命令层四步与打标）；packages/desktop/src-tauri/crates/infra/store/src/store.rs（mark_explore_promoted） |
| AC-7 | promote 拒绝面：笔记未落盘 / 内容空白 / 非 kebab name / 已 promoted / 同名 active change 冲突 / 分支或 worktree 目录冲突——各返回显式 `Err`，explore 记录与笔记零改动 | packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs（create 失败 / 删笔记失败）；packages/desktop/src-tauri/src/commands/explores/mod.rs（前置拒绝）；packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs（同名 active 冲突既有） |
| AC-8 | promoted 详情态：promote 后探索详情显示「已转变更」+ 跳转（路由 `/changes/<change_id>`），预览不再读空；手动新建 change 后 change 详情头 / 清单条目显示 title（默认 = name） | packages/desktop/src/views/explores/explore-detail-view.tsx（promoted 态）；packages/desktop/src/views/explores/explore-view.tsx（清单 title）；packages/desktop/src/views/explores/hooks/use-explore-list.ts（promote / updateTitle 动作）；packages/desktop/src/views/changes/change-list-view.tsx；packages/desktop/src/views/changes/change-detail-view.tsx；packages/desktop/src-tauri/src/commands/changes/mod.rs（手动新建 title 默认） |
| AC-9 | change title 出线与 golden：`ChangeSummary` / `ChangeDetail` 增 title 字段经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕；bindings 一致性守卫绿 | packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs（golden 半边）；packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs（golden 半边）；golden diff 人工确认与 bindings 守线 → 不可测试项 3 |
| AC-10 | 版本交付：`packages/desktop` version 0.4.33；`plugins/dev-team` 零改动；`vp test` / `client:check` / knip 全绿 | —（见不可测试项 1 / 2） |

---

## 单元测试

<!-- 测试框架与测试区域识别：Rust 面为 src-tauri cargo workspace 套件（`cargo test`），测试文件为共置
     `*_test.rs` 模块 + `tests/corpus_golden_test.rs` + `tests/fixtures/` 语料 golden；前端面为
     packages/desktop vite-plus 套件（`vp test`，vitest describe/it/expect + @testing-library/react），
     测试文件为共置 `.test.ts(x)`。既有测试布局决定本变更全部为共置测试（源文件旁 `_test.rs` / `.test.ts(x)`），
     新增后端 promote_test.rs 与前端 explore-title / explore-view / explore-detail-view / explore-create-dialog 4 个测试文件。

     进程边界策略（最小 mock）：db 为进程边界——store 面真实 tempfile redb workspace db（旧形态库经裸 redb
     直写字节预置，沿用 inject_* 先例）；core 写面 / 查询面以进程内假件实现 `ChangeStateStore` / `WorktreePort`
     （注入依赖入参）+ 真实 tempdir fs 组合；命令层沿既有 MockRuntime 泛型缝 + tempfile store；前端仅 mock IPC
     进程边界（`@tauri-apps/api/core` 的 invoke / Channel），被测页面 / 面板 import 的内部 hooks 真实组合，
     fixture 经 mock IPC 流入后断言渲染输出。

     覆盖所有可在进程内验证的场景（Rust #[test] / #[tokio::test]、前端 vite-plus 纯函数 / 组件 mock 测试）。
     每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节；跨模块组合用例（promote move 全链、
     title 回填链路、命令回环）colocate 到链路入口模块（promote_test / explore-detail-view.test /
     explores mod_test）的 `#### 用例` 表，describe 标题写链路方向。纯 re-export / 生成物 / 版本声明
     （write/mod.rs、bindings.ts、package.json）不建独立测试文件，统一落「不可测试项」声明。

     迭代类型词表：**新增**（新用例）/ **重写**（既有用例语义演进改写——本变更主形态：name 展示键断言改
     title、宽松单分量名校验改 kebab）/ **适配**（机械传参或 fixture 补齐 title / promotedTo，既有断言面随动）/
     **持衡（沿用）**（零触点断言面）。golden 面：`DESKTOP_GOLDEN_REWRITE=1` 显式重写流程启用
     （`ChangeSummary.title` / `ChangeDetail.title` 出线 golden——AC-9），diff 人工确认留痕为流程性动作（见不可测试项）。
     title / kebab 口径纪律：`title` 恒非空（创建与升级默认 = name，promote 显式继承 explore.title，回填经
     `set_explore_title` 拒绝空白）；explore 名称口径升级为 kebab-case（等价 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`
     + 长度 ≤128），与 change 名称对齐。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- 挂 AC-1（ExploreRecord v1→v2 decode-only 与回环）、AC-2（ChangeRecord v3→v4 decode-only）。
     design.md 公共函数 / API 表列出 ExploreRecord.new / ChangeRecord.new；被测面另含类型定义表行
     （ExploreRecord v2 / ExploreRecordV1 / ChangeRecord v4 / ChangeRecordV3）。native_model 编解码
     自身语义不逐项验证（库语义），只测自研字段面 / 注册面 / 版本段。 -->

#### 待测功能

- `ExploreRecord.new(root, name, now)`: 签名不变；新增默认 `title = name`、`promoted_to = None`（id 置 0 由写事务 max+1 覆盖的既有语义不变）
- `ChangeRecord.new(id, name, workflow_type, created_at, worktree, base_commit)`: 签名不变；新增默认 `title = name`
- `ExploreRecord` v2: `name` 退化为笔记文件 stem 寻址键（kebab-case 口径）；新增 `title: String`（恒非空）与 `promoted_to: Option<String>`（指向 change id 身份锚）
- `ExploreRecordV1`（pub(crate)，4:v1）与双向 `From`: 升级半边补 `title = name` / `promoted_to = None`，降级半边丢弃新字段（仅升级链解码用，不注册）
- `ChangeRecord` v4: 新增 `title: String`（恒非空）；id 主键 / name 裸名属性及其余字段面逐字不变
- `ChangeRecordV3`（pub(crate)，9:v3）与双向 `From`: 升级半边补 `title = name`，降级半边丢弃 `title`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| model_test · ExploreRecord v2 回环 | 正向 | title / promoted_to 三态（None / Some(change id)）构造 → native_model encode/decode 往返逐字段相等（版本头 = 2） | 新增 |
| model_test · ExploreRecord V1→V2 decode-only 升级 | 正向 | V1 形态记录 encode → decode as V2 → `title == name`、`promoted_to == None`、id/root/name/created_at/updated_at 逐字一致（零 migrate 调用——AC-1） | 新增 |
| model_test · ExploreRecord V2→V1 降级半边 | 边界 | V2 → V1 降级丢弃 title / promoted_to，剩余字段逐字一致（运行时无降级读取路径，仅 `from` 双向语义） | 新增 |
| model_test · ExploreRecord.new 默认 title=name | 正向 | `new(root, name, now)` → title == name、promoted_to == None、id == 0、created_at == updated_at == now | 新增 |
| model_test · ChangeRecord v4 回环 | 正向 | title 显式值构造 → encode/decode 往返逐字段相等（版本头 = 4；v3 回环行重写） | 重写 |
| model_test · ChangeRecord V3→V4 decode-only 升级 | 正向 | V3 形态记录 encode → decode as V4 → `title == name`、其余字段逐字一致（AC-2） | 新增 |
| model_test · ChangeRecord V3→V4 降级半边 | 边界 | V4 → V3 降级丢弃 title，剩余字段逐字一致 | 新增 |
| model_test · ChangeRecord.new 默认 title=name | 正向 | `new(id, name, workflow_type, created_at, worktree, base_commit)` → title == name；status 恒 active 起步 / archived_at / active_phase 空起步持衡 | 重写 |
| model_test · 表名版本段 | 边界 | native_model id / version 断言：ExploreRecord 4:v2、ChangeRecord 9:v4（`{id}_{version}_{key}` 表名公式锚——与 store_test 常量同源；旧表 `4_1_*` / `9_3_*` 对新读面不可见） | 新增 |
| model_test · title 非空不变量 | 边界 | 构造与升级两路径下 title 恒非空（默认 = name 兜底）；promoted_to 默认 None | 新增 |
| model_test · 身份锚不变 | 边界 | ExploreRecord.id 主键不变（i64 max+1 分配）、ChangeRecord.id 主键不变（UUID 形态）；name 均降为可变属性（无唯一约束）——零换锚 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯内存构造 + native_model 编解码往返（零 IO 零进程边界） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- 挂 AC-1（ExploreRecord 回环 + 信封零改动）、AC-2（change_state / create_change_record title 映射）、
     AC-3（kebab 校验 + 存量可读）、AC-5（set_explore_title）、AC-6（mark_explore_promoted）。
     既有 Env / StoresEnv tempfile 装置沿用；V1 / V3 旧形态库经裸 redb 直写字节预置（inject_* 先例同式）。
     本地 `is_kebab_case` 为私有判定，行为面经公共操作面（建档 / 改名）断言。 -->

#### 待测功能

- `Store.create_explore_record(root, name)`: 签名不变；校验由单分量名升级为 kebab-case + 长度 ≤128；title 默认 = name、promoted_to = None
- `Store.rename_explore_record(root, name, new_name)`: 签名不变；目标名校验升级为 kebab-case + 长度 ≤128（in-place 保主键保链语义不变）
- `Store.set_explore_title(root, name, title)`: in-place 写 `title` + 刷新 `updated_at`；`title` 空白 → `Err`；miss → `Err`
- `Store.mark_explore_promoted(root, name, change_id)`: in-place 写 `promoted_to = Some(change_id)` + 刷新 `updated_at`；已 promoted / miss → `Err`
- `Store.create_change_record(record: ChangeStateRecord)`: 签名不变；映射 `title`（ChangeStateRecord → ChangeRecord → `change_state` 回环一致）
- `change_state()`: 记录 ↔ 中性类型映射单点新增 `title` 投影

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| store_test · create_explore_record kebab 正向 | 正向 | 合法 kebab（`api-retry` / `a-b2-c`）建档成功；title == name、promoted_to == None、id 为 max+1 | 重写 |
| store_test · create_explore_record 非法 kebab 拒绝 | 异常 | 大写 / 下划线 / 空格 / 前导数字 / 连号连字符 / 尾连字符 / 空段 / 超 128 字符 → `Err` 且零写入（拒绝面零残留——AC-3） | 重写 |
| store_test · rename_explore_record 非法 kebab 拒绝 | 异常 | 目标名非 kebab → `Err` 且原记录 name 零改动（in-place 拒绝不破链） | 重写 |
| store_test · rename_explore_record kebab 正向 | 正向 | 合法 kebab 改名 → 主键 id 不变（保会话链）、updated_at 刷新、title 随记录保留 | 重写 |
| store_test · 存量非 kebab 记录仍可读 | 边界 | 预置旧非 kebab 记录 → list / find 照常读出（list/find 零 name 合法性校验）；promote 前置拒绝归命令层 | 新增 |
| store_test · set_explore_title 正向 | 正向 | 写 title + 刷新 updated_at，返回记录 title 更新、id / name / promoted_to 不变 | 新增 |
| store_test · set_explore_title 空白拒绝 | 异常 | `title` 空白 → `Err`（title 恒非空单点，store 不回退） | 新增 |
| store_test · set_explore_title miss | 异常 | 记录不存在 → `Err`（NotFound 语义） | 新增 |
| store_test · mark_explore_promoted 正向 | 正向 | `promoted_to = Some(change_id)` + 刷新 updated_at，返回记录 promoted_to 就位 | 新增 |
| store_test · mark_explore_promoted 已 promoted 拒绝 | 异常 | 二次打标 → `Err`（不静默覆写） | 新增 |
| store_test · mark_explore_promoted miss | 异常 | 记录不存在 → `Err` | 新增 |
| store_test · create_change_record title 回环 | 正向 | `ChangeStateRecord{ title }` 落库 → `change_state` / find / list 读出 title 逐字一致（建档回环一致——AC-2） | 重写 |
| store_test · change_state title 映射 | 正向 | 建档记录 title 投影进中性快照；其余字段面逐字不变 | 新增 |
| store_test · create_change_record 同 id 防御拒绝 | 异常 | 同 id 再建档 → `Conflict`（既有语义零改动，仅 title 载荷随动） | 适配 |
| store_test · list_explore_records / find_explore_record 信封零改动 | 边界 | 清单 / 寻址 API 签名与返回面零改动（title / promotedTo 随 DTO 出线不新增信封维度） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile 真实 redb workspace db（既有 Env / StoresEnv 装置）；V1 / V3 旧形态库经裸 redb `TableDefinition` 直写字节预置，不经 mock | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

<!-- 挂 AC-2（ChangeStateRecord 字段面半边）。design.md 公共函数 / API 表未声明该文件具名导出
     （纯类型定义 + trait）；被测面为类型定义表行（ChangeStateRecord 增 title）。 -->

#### 待测功能

<!-- design.md 未声明该文件具名导出；被测面为类型定义表行。 -->

- `ChangeStateRecord`: 新增 `title: String`（`name` 之后；人类可读标题，恒非空）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| state_test · ChangeStateRecord title 字段面 | 正向 | title 独立字段构造 → PartialEq 逐字段可辨（title 与 name 双字段可辨） | 新增 |
| state_test · title 与 name 可辨 | 边界 | 同 name 不同 title 两记录不等价；title 恒非空（默认 = name 由构造侧单点保证，中性类型不校验） | 新增 |
| state_test · 既有字段面持衡 | 边界 | id / name / workflow_type / status / archived_at / active_phase / worktree / base_commit 字段面零改动 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯内存构造与等值面（零 IO 零进程边界） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

<!-- 挂 AC-2（create 建档 title 载荷半边）、AC-7（同名 active 冲突既有半边）。既有 create_test 扩展节：
     create 增 title 入参（空/空白回退 name 单点）；ChangeStateRecord 建档载荷含 title；既有前置七道与
     补偿链语义零改动（title 为纯投影零行为）。既有用例主体为形参适配（六参 → 七参）。 -->

#### 待测功能

- `create(main_root, worktree_root, store, vcs, name, goal, title)`: 新增 `title` 入参；空/空白回退 `name`（title 恒非空单点）；`ChangeStateRecord` 建档载荷含 title；`CreateOutcome` 字段面不变
- `CreateOutcome`: 字段面不变（id / name / created / worktree / warnings）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| create_test · create 显式 title | 正向 | 显式 title 传入 → 假 store 捕获建档载荷 `title == 显式值`；worktree 内 explore.md = goal 原文不变 | 新增 |
| create_test · create 空 title 回退 name | 边界 | title 空串 / 纯空白 → 建档载荷 `title == name`（回退单点，恒非空） | 新增 |
| create_test · create 建档载荷 title | 正向 | 建档 ChangeStateRecord 含 title 字段（假 store 捕获载荷逐字段断言） | 新增 |
| create_test · 既有七前置持衡 | 边界 | name kebab / goal 空白 / 主仓目录冲突 / 同名 active 冲突 / git 探测 / 分支冲突 / worktree 目录冲突——七前置语义零改动（仅签名加 title） | 适配 |
| create_test · 同名 active 冲突既有 | 异常 | 同名 active 记录存在 → `Err`（AC-7 同名 active 冲突半边）；归档同名不拒语义持衡 | 适配 |
| create_test · 补偿链零改动 | 异常 | add 失败 / 树写出失败 / bootstrap 段失败的按 id 补偿删除语义零改动（title 纯投影零行为） | 适配 |
| create_test · 成功后清单立即可见 | 正向 | create 成功后 list_changes 出线条目（title 随 ChangeSummary 投影见 list 章节） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ChangeStateStore`（进程内假件） | 进程内假件实现 trait：捕获建档载荷（title / name / goal）+ 可编程补偿故障 | 本节全部用例 |
| `WorktreePort`（进程内假件） | 进程内脚本化假件：probe / branch_exists / add_worktree 可编程注入 Err + 调用与参数捕获 | 本节全部用例 |
| 文件系统 | 真实 tempdir 双根（主仓根 + worktree 落位父锚）+ 预置路径分量文件占位注入写失败（不经 mock） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/promote_test.rs

<!-- 挂 AC-6（写面 move 半边）、AC-7（create 失败 / 删笔记失败半边）。新增测试文件：promote_explore
     复用 create（goal=note 全文、title 显式继承）成功后删主仓 explores/<name>.md（move 半边）；
     PromoteOutcome{change_id, change_name}。跨模块组合用例（move 全链）colocate 本入口。 -->

#### 待测功能

- `promote_explore(main_root, worktree_root, store, vcs, name, note, title)`: 复用 `create`（goal=note 全文、title 显式继承）成功后删主仓 `explores/<name>.md`（move 半边）；sync 零 Tauri
- `PromoteOutcome`: `change_id: String` / `change_name: String`（serde camelCase；specta 出线）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| promote_test · promote 成功 move 全链 | 正向 | 预置主仓 `explores/<name>.md` → promote 成功：假 store 捕获 create 建档载荷（name=explore.name、goal=note、title）；worktree 内 `explores.md` 内容 == note 全文；主仓原笔记已删；PromoteOutcome 返回 change_id / change_name（AC-6 写面半边） | 新增 |
| promote_test · note 全文含首行标题 | 边界 | note 含首行 `# 标题` → 原样写入 worktree explore.md（零内容转换，标题行保留） | 新增 |
| promote_test · title 显式继承 | 正向 | promote 传入 title == explore.title → create 建档载荷 title 逐字一致（不做回退） | 新增 |
| promote_test · create 失败零副作用 | 异常 | create 前置拒绝（假 store / vcs 注入 Err）→ promote 返回 `Err`，主仓笔记零删除、零变化 | 新增 |
| promote_test · 删笔记失败显式 Err | 异常 | create 成功后预置路径分量使 `fs::remove_file` 失败 → `Err` 携 change id + 「笔记全文已在 change explore.md 留底」回写指引（R1 残留呈现） | 新增 |
| promote_test · PromoteOutcome 线形 | 边界 | 返回 DTO 两字段（change_id / change_name）serde camelCase 出线（`changeId` / `changeName`） | 新增 |
| promote_test · sync 零 Tauri 纪律 | 边界 | `promote_explore` 为 sync 函数（无 tokio 上下文依赖——经 spawn_blocking 由命令层驱动） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ChangeStateStore`（进程内假件） | 进程内假件实现 trait：捕获 create 建档载荷（name / goal / title）+ 可编程失败 | 本节全部用例 |
| `WorktreePort`（进程内假件） | 进程内脚本化假件：probe / branch_exists / add_worktree 可编程注入 Err | 本节全部用例 |
| 文件系统 | 真实 tempdir 双根；删笔记失败经预置同名目录占位注入（`remove_file` 对目录失败，不经 mock） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs

<!-- 挂 AC-2（ChangeSummary title 投影）、AC-9（ChangeSummary.title golden 半边）。design.md 公共函数 /
     API 表未声明该文件具名导出（`list_changes` 签名不变）；被测面为类型定义表行（ChangeSummary 增 title）
     与 `db_entry` 映射。corpus_golden_test.rs 的 golden 断言半边 colocate 本入口（describe 标题写 golden）。 -->

#### 待测功能

<!-- design.md 未声明该文件具名导出签名变更；被测面为类型定义表行与映射单点。 -->

- `ChangeSummary`: 新增 `title: String`（`name` 之后；纯投影）
- `db_entry(record)`: 映射 `record.title`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| list_test · db_entry title 投影 | 正向 | 假 store 注入 `ChangeStateRecord{ title }` → `list_changes` 输出 `ChangeSummary.title == record.title`（active / archive 两归组同式） | 新增 |
| list_test · title 与 name 可辨 | 边界 | title 显式 ≠ name → 清单条目出线 title 独立字段（name 恒裸名零污染） | 新增 |
| list_test · 归组 / 排序零改动 | 边界 | active 按 name 排序 / archive 月分组 / 未知时间置尾——title 为纯投影零派生改写 | 持衡 |
| corpus_golden · ChangeSummary.title golden | 正向 | 语料种子含 title 两态（title=name 缺省样本与显式标题样本）→ list golden 逐字节重写后复核轮绿（`DESKTOP_GOLDEN_REWRITE=1`） | 新增 |
| corpus_golden · title 线形契约 | 边界 | `ChangeSummary` JSON 出线含 `title` 字段（位置 name 之后；serde camelCase 不变量） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ChangeStateStore`（进程内假件） | 进程内假件实现 trait：注入 `ChangeStateRecord{ title }` 记录 | 单元用例 |
| 无 mock（golden 半边） | 真实语料种子 + corpus_golden 重写流程（db 单源零磁盘触点不变） | golden 用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

<!-- 挂 AC-2（ChangeDetail title 直读）、AC-9（ChangeDetail.title golden 半边）。design.md 公共函数 /
     API 表未声明该文件具名导出（`change_detail` 签名不变）；被测面为类型定义表行（ChangeDetail 增 title）
     与聚合映射。corpus_golden_test.rs 的 golden 断言半边 colocate 本入口。 -->

#### 待测功能

<!-- design.md 未声明该文件具名导出签名变更；被测面为类型定义表行与聚合映射。 -->

- `ChangeDetail`: 新增 `title: String`（`name` 之后；自记录直读）
- `change_detail(...)`: 聚合映射 `record.title`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail_test · change_detail title 直读 | 正向 | 假 store 注入建档记录 → 详情 `ChangeDetail.title == record.title` | 新增 |
| detail_test · 未知 id None 降级 | 边界 | db 无该 id → 详情恒 None（零磁盘回退零文档形态——既有语义零改动） | 持衡 |
| detail_test · 既有聚合面持衡 | 边界 | pipeline / active_phase / runs / artifacts / worktree 聚合面零改动（title 纯投影） | 适配 |
| corpus_golden · ChangeDetail.title golden | 正向 | 语料种子 title 两态 → detail golden 逐字节重写后复核轮绿（`DESKTOP_GOLDEN_REWRITE=1`） | 新增 |
| corpus_golden · title 线形契约 | 边界 | `ChangeDetail` JSON 出线含 `title` 字段（位置 name 之后） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ChangeStateStore`（进程内假件） | 进程内假件实现 trait：注入建档记录 + 相位 / run 历史 | 单元用例 |
| 无 mock（golden 半边） | 真实语料种子 + corpus_golden 重写流程 | golden 用例 |

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

<!-- 挂 AC-2（create 建档 title 默认半边）。既有 mod_test 扩展节：create_change IPC 保持三参、
     内部 title=name；create_change_with 增 title 参数并透传 write::create。 -->

#### 待测功能

- `create_change(app, root, name, goal)`: IPC 签名不变；内部以 `title = name` 调 `create_change_with`
- `create_change_with<R>(app, root, name, goal, title)`: 增 `title` 参数透传 `write::create`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| changes/mod_test · create_change 内部 title=name | 正向 | 捕获 write::create 载荷：`title == name`（IPC 三参面不变，title 默认 = name——AC-8 手动新建默认） | 新增 |
| changes/mod_test · create_change_with title 透传 | 正向 | 显式 title 传入 → write::create 载荷 `title == 显式值` | 新增 |
| changes/mod_test · blank root 显式 Err | 异常 | root 空白 → `Err`（既有语义零改动） | 持衡 |
| changes/mod_test · 既有 create 三参签面 | 边界 | IPC 签名与返回面零改动（title 不扩 IPC 入参面） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ChangeStateStore` / `WorktreePort`（进程边界） | 既有 MockRuntime 泛型缝 + tempfile store；捕获 write::create 建档载荷（title 透传） | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/explores/mod.rs -> packages/desktop/src-tauri/src/commands/explores/mod_test.rs

<!-- 挂 AC-3（scan 过滤 + promote 拒绝）、AC-5（update_explore_title 命令）、AC-6（命令层四步打标）、
     AC-7（promote 前置拒绝）。既有 mod_test 扩展节：新增 update_explore_title / promote_explore 命令；
     scan_explores 命令层增非 kebab stem 过滤。跨模块组合用例（promote 命令四步）colocate 本入口。 -->

#### 待测功能

- `update_explore_title(stores, root, name, title)`: Tauri 命令；blank root / 空白 title → 显式 `Err`；经 `set_explore_title`
- `promote_explore(app, root, name)`: Tauri 命令；blank root / 非 kebab name → 显式 `Err`；读笔记 → 读记录 → 写面 → 打标四步
- `scan_explores(root, stores)`: 签名不变；命令层在未绑定过滤之外增非 kebab stem 过滤

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| explores/mod_test · update_explore_title 正向 | 正向 | 经 store.set_explore_title 回填 title，返回更新后记录 | 新增 |
| explores/mod_test · update_explore_title 空白 title 拒绝 | 异常 | blank root / 空白 title → 显式 `Err`（title 恒非空） | 新增 |
| explores/mod_test · promote_explore 四步成功 | 正向 | 预置笔记与记录 → 读笔记命中 → 读记录未 promoted → 写面 move → `mark_explore_promoted(change_id)`；主仓笔记已删、记录保留、promoted_to == change.id（AC-6 命令层半边） | 新增 |
| explores/mod_test · promote_explore blank root / 非 kebab 拒绝 | 异常 | blank root 或非 kebab name → 显式 `Err`（零读写） | 新增 |
| explores/mod_test · promote_explore 笔记未落盘 / 空白拒绝 | 异常 | read_explore 返回 None 或空白 → 显式 `Err`，记录零改动 | 新增 |
| explores/mod_test · promote_explore 已 promoted 拒绝 | 异常 | 记录 promoted_to 非空 → 显式 `Err` | 新增 |
| explores/mod_test · promote_explore 打标失败 | 异常 | 写面成功后 `mark_explore_promoted` 注入 Err → `Err` 携 change id + 手动恢复指引（记录零改动，笔记已在 change explore.md 留底） | 新增 |
| explores/mod_test · scan_explores 过滤非 kebab | 边界 | 笔记目录含非 kebab stem（大写 / 下划线 / 前导数字）→ 不列入可绑定清单；已绑定过滤照常 | 新增 |
| explores/mod_test · scan_explores blank root | 边界 | blank root → 空结果（既有语义零改动） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ChangeStateStore` / `WorktreePort`（进程边界） | 既有 MockRuntime 泛型缝 + tempfile store + 进程内假 vcs；写面 move 半边可编程失败 | 本节全部用例 |
| 文件系统 | 真实 tempdir 笔记夹具（预置 explores/<name>.md） | promote / scan 用例 |

### packages/desktop/src/lib/explore-stance.ts -> packages/desktop/src/lib/explore-stance.test.ts

<!-- 挂 AC-4（模板注入半边）。既有 explore-stance.test.ts 扩展节：buildExplorePrompt 改两参
     （userInput, topic），注入 record.name 与「笔记首行写 # <标题>」约定。 -->

#### 待测功能

- `buildExplorePrompt(userInput, topic)`: 新增 `topic` 入参；模板注入当前记录 name（落盘文件名 MUST 等于 name）与「笔记首行写 `# <标题>`」约定

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| explore-stance_test · 注入 name 与首行标题约定 | 正向 | 输出含 topic（文件名 MUST 等于 topic 指令）与「笔记首行写 `# <标题>`」约定；前导开篇句 / 用户输入原文结尾语义不变 | 重写 |
| explore-stance_test · 两参拼接触发 | 边界 | `buildExplorePrompt(userInput, topic)` 输出 = 前导 + name 注入段 + 分隔线 + 用户输入原文（拼接非替换） | 重写 |
| explore-stance_test · topic 特殊字符 | 边界 | topic 含空格 / 中文 / 超长 / emoji → 原样注入不截断、不抛错 | 新增 |
| explore-stance_test · 空 topic | 边界 | topic 空串 → 不抛错，约定段照常输出（title 兜底由 agent 未写标题行时保持 name 承接） | 新增 |
| explore-stance_test · 前导恒等 / 恰一次 | 边界 | 前导半边对所有输入恒等；前导在输出中恰出现一次（既有断言面随两参签名适配） | 适配 |
| explore-stance_test · 调试页不含模板 | 边界 | 调试页不经 buildExplorePrompt（零触点——静态守线，见不可测试项 6；本节不设行为面） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数零依赖（模块私有常量以结构性事实断言） | 本节全部用例 |

### packages/desktop/src/lib/explore-title.ts -> packages/desktop/src/lib/explore-title.test.ts

<!-- 挂 AC-5（标题解析半边）。新增测试文件：extractMarkdownTitle 纯函数（首行 `# 标题` 解析）。 -->

#### 待测功能

- `extractMarkdownTitle(content)`: 首行 `# 标题` 解析；空白 / 无标题行 → `null`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| explore-title_test · 首行 # 标题解析 | 正向 | `# 标题\n正文` → 返回 `标题`（去前导 # 与空白） | 新增 |
| explore-title_test · 标题含空格 / 中文 / emoji / 特殊字符 | 边界 | 标题内特殊字符原样返回（零改写） | 新增 |
| explore-title_test · 无标题行 | 边界 | 正文起首（非 `# `）→ `null`（D5 无标题行不更新） | 新增 |
| explore-title_test · 空白内容 | 边界 | 空串 / 纯空白 / 纯换行 → `null` | 新增 |
| explore-title_test · 首行非一级标题 | 边界 | 首行 `## 二级` 或 `### 三级` → `null`（仅一级标题被解析） | 新增 |
| explore-title_test · 首行 # 后空标题 | 边界 | `# `（# 后仅空白）→ `null`（空标题不产出） | 新增 |
| explore-title_test · 多行 / 超长标题 | 边界 | 只解析首行；超长标题（>1000 字符）不截断 | 新增 |
| explore-title_test · CRLF 行尾 | 边界 | `\r\n` 行尾首行解析兼容（`# 标题\r\n正文`） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数零依赖 | 本节全部用例 |

### packages/desktop/src/views/explores/explore-view.tsx -> packages/desktop/src/views/explores/explore-view.test.tsx

<!-- 挂 AC-8（清单 title 半边）。新增测试文件：ExploreListItem 渲染 record.title（挂 data-testid）；
     ExploreDetailView 透传 list。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该视图导出函数（React 组件）；被测面为组件渲染行为与取数参数链。 -->

- `ExploreListItem`: 渲染 `record.title`（挂 `data-testid`；name 不再作标题展示）
- `ExploreView`: 向 `ExploreDetailView` 透传 `list`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| explore-view_test · 清单条目渲染 title | 正向 | fixture record.title 显示为标题、data-testid 在位；title 与 name 不同时显示 title 而非 name | 新增 |
| explore-view_test · 清单条目 title 两态 | 边界 | title=name 缺省样本与显式标题样本各渲染正确（恒非空不变量零回退分支） | 新增 |
| explore-view_test · 详情透传 list | 正向 | 选中记录时 ExploreDetailView 收到 list（refresh / updateTitle / promote 动作可达） | 新增 |
| explore-view_test · 未选中 / 缺失记录 | 边界 | 清单就绪前 record=null → 详情缺失态（既有语义零改动） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` invoke mock；内部 useExploreList 真实组合，fixture 经 mock IPC 流入后断言渲染输出 | 本节全部用例 |

### packages/desktop/src/views/explores/explore-detail-view.tsx -> packages/desktop/src/views/explores/explore-detail-view.test.tsx

<!-- 挂 AC-5（title 回填 effect 半边）、AC-8（promoted 详情态半边）。新增测试文件：详情头渲染 title；
     promoted 态（「已转变更」徽标 + 跳转）；「启动变更」入口（调 list.promote）；title 回填 effect；
     promoted 时停读 explores/<name>.md。 -->

#### 待测功能

- `ExploreDetailView({ root, record, list })`: props 新增 `list: ExploreListState`（供 promote / updateTitle / refresh）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| explore-detail-view_test · 详情头渲染 title | 正向 | 详情头显示 record.title（data-testid 在位） | 新增 |
| explore-detail-view_test · promoted 态徽标 + 跳转 | 正向 | record.promotedTo 非空 → 渲染「已转变更」徽标 + 「查看变更」按钮；点击 `navigate(/changes/<promotedTo>)` | 新增 |
| explore-detail-view_test · 草稿态「启动变更」入口 | 正向 | promotedTo 为 null → 渲染「启动变更」入口；点击调 list.promote 恰一次 | 新增 |
| explore-detail-view_test · promote 行内错误块 | 异常 | list.promote reject → 行内错误块呈现错误文本、页面不崩 | 新增 |
| explore-detail-view_test · title 回填 effect | 正向 | doc.doc 首行 `# 标题` 与 record.title 不同 → `list.updateTitle` 恰一次并 refresh（AC-5 链路） | 新增 |
| explore-detail-view_test · title 回填无更新分支 | 边界 | 首行标题与 record.title 相同 / 无标题行（extractMarkdownTitle 返回 null）→ 零 invoke 零报错 | 新增 |
| explore-detail-view_test · promoted 时停读原笔记 | 边界 | promotedTo 非空 → name 传 null（停 read/watch），渲染 promoted 提示面板而非继续读 explores/<name>.md | 新增 |
| explore-detail-view_test · 未知记录 null 态 | 边界 | record=null → 缺失态占位（既有语义零改动） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')`（invoke + Channel）；useExploreDoc / useExploreSession 内部 hook 真实组合，fixture 经 mock IPC 流入 | 本节全部用例 |
| `list`（注入依赖） | props 注入 `ExploreListState` 假件（updateTitle / promote / refresh 可编程 resolve / reject——入参例外） | 本节全部用例 |
| 路由（进程边界） | MemoryRouter + LocationProbe 装置（观察 navigate 落点） | promoted 跳转用例 |

### packages/desktop/src/views/explores/components/explore-create-dialog.tsx -> packages/desktop/src/views/explores/components/explore-create-dialog.test.tsx

<!-- 挂 AC-3（本地 kebab 校验半边）。新增测试文件：新话题本地 kebab-case 校验（非法禁提交）；
     导入列表消费命令层已过滤结果。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该视图导出函数（React 组件）；被测面为本地校验与提交行为。 -->

- `TopicEntry` 本地 kebab-case 校验（复用本地 `isKebabCase`，非法禁提交）
- `ImportList` 消费命令层已过滤结果（非 kebab 不出现在导入清单）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| explore-create-dialog_test · 合法 kebab 提交 | 正向 | 合法 kebab → 建档按钮可点，invoke createExploreRecord 恰一次 | 新增 |
| explore-create-dialog_test · 非法 kebab 禁提交 | 边界 | 大写 / 下划线 / 空格 / 前导数字 → 提交禁用 + 本地错误提示（零 invoke） | 新增 |
| explore-create-dialog_test · 本地错误呈现 | 边界 | 非法输入错误块呈现、合法化后错误清除 | 新增 |
| explore-create-dialog_test · 导入列表命令层已过滤 | 边界 | scanExplores 返回非 kebab 不渲染（命令层已过滤 + 本地零重复校验） | 持衡 |
| explore-create-dialog_test · 既有建档 / 导入成功回调 | 边界 | 成功建档后 onCreated(name) 恰一次（父层 refresh + 导航语义不变） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` invoke mock（scanExplores / createExploreRecord 可编程 resolve / reject） | 本节全部用例 |

### packages/desktop/src/views/explores/hooks/use-explore-list.ts -> packages/desktop/src/views/explores/hooks/use-explore-list.test.ts

<!-- 挂 AC-5（updateTitle 动作半边）、AC-8（promote 动作半边）。既有 use-explore-list.test.ts 扩展节：
     ExploreListState / ExploreActions 新增 updateTitle(name, title) / promote(name)（invoke 后 refresh；
     返回 Promise<void> 供详情页行内错误捕获）。 -->

#### 待测功能

- `useExploreList(root)`: 返回面新增 `updateTitle: (name, title) => Promise<void>` 与 `promote: (name) => Promise<void>`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-explore-list_test · updateTitle 动作 | 正向 | invoke("update_explore_title", { root, name, title }) → 成功后以当前 root 重新取数 | 新增 |
| use-explore-list_test · promote 动作 | 正向 | invoke("promote_explore", { root, name }) → 成功后 refresh（promoted 态随清单取数回流） | 新增 |
| use-explore-list_test · updateTitle / promote reject | 异常 | 动作 reject → error 置位（清单保持原值，Promise 拒绝供详情页行内捕获） | 新增 |
| use-explore-list_test · 无 root 守卫 | 边界 | root 为 null → updateTitle / promote 均 no-op（零 IPC） | 新增 |
| use-explore-list_test · fixture 增 title / promotedTo | 边界 | ExploreRecord fixture 补 title / promotedTo 字段，既有取数 / 竞态 / 动作断言面零改动 | 适配 |
| use-explore-list_test · 既有 create / rename / remove 持衡 | 边界 | 三动作 invoke 后 refresh 语义零改动（kebab 校验归命令 / store 层） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` invoke mock（既有 vi.hoisted 装置） | 本节全部用例 |

### packages/desktop/src/views/explores/hooks/use-explore-session.ts -> packages/desktop/src/views/explores/hooks/use-explore-session.test.ts

<!-- 挂 AC-4（send 组装半边）。既有 use-explore-session.test.ts 扩展节：send 组装
     buildExplorePrompt(input.prompt, record.name)；buildExplorePrompt 真实实现参与拼接语义断言。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件具名导出签名变更；被测面为 send 组装参数链。 -->

- `useExploreSession.send`: 组装 `buildExplorePrompt(input.prompt, record.name)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-explore-session_test · send 注入 record.name | 正向 | record 非空 → session.sendMessage prompt == buildExplorePrompt(prompt, record.name)（record.name 注入 stance） | 重写 |
| use-explore-session_test · record null 不发送 | 边界 | record=null → send 为 no-op（零 IPC，既有语义） | 持衡 |
| use-explore-session_test · 会话还原 / 续话持衡 | 边界 | sourceRef / 续话 / 轮镜像语义零改动（record.name 仅 stance 入参） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | invoke + Channel mock（既有 vi.hoisted 装置）；buildExplorePrompt 不 mock（真实实现参与拼接语义断言） | 本节全部用例 |

### packages/desktop/src/views/changes/change-list-view.tsx -> packages/desktop/src/views/changes/change-list-view.test.tsx

<!-- 挂 AC-8（change 清单 title 半边）。既有 change-list-view.test.tsx 扩展节：ChangeRow 渲染
     summary.title 为标题（summary.name 不再作标题展示；行键 / 导航恒 id 不变）。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该视图导出函数（React 组件）；被测面为组件渲染行为。 -->

- `ChangeRow`: 渲染 `summary.title`（`summary.name` 不再作标题展示；行键 / 导航恒 id）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change-list-view_test · 条目渲染 title | 正向 | fixture summary.title 显示为标题（title 与 name 不同时显示 title） | 重写 |
| change-list-view_test · name 不再作标题 | 边界 | summary.name 不渲染为标题位（仅作展示辅助 / 零标题语义） | 重写 |
| change-list-view_test · 行键 / 导航恒 id | 边界 | 行键 / navigate 以 id（既有语义零改动） | 持衡 |
| change-list-view_test · fixture 增 title | 边界 | ChangeList fixture 条目补 title 字段，既有空态 / 归档分组 / 月分组断言零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` invoke mock（既有 vi.hoisted 装置） | 本节全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

<!-- 挂 AC-8（change 详情头 title 半边）。既有 change-detail-view.test.tsx 扩展节：DetailHeader 渲染
     detail.title 为标题（detail.name 保留供归档 / run 控制面板）。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该视图导出函数（React 组件）；被测面为组件渲染行为。 -->

- `DetailHeader`: 渲染 `detail.title`（`detail.name` 保留供归档 / run 控制面板）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change-detail-view_test · 详情头渲染 title | 正向 | fixture detail.title 显示为标题（title 与 name 不同时显示 title） | 重写 |
| change-detail-view_test · name 保留供面板 | 边界 | 归档 / run 控制面板仍以 detail.name 供给（展示面与命令面可辨） | 适配 |
| change-detail-view_test · fixture 增 title | 边界 | ChangeDetail fixture 补 title 字段，既有取数 / 面板 / run 控制 / 未知 id 降级断言零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')`（invoke + Channel mock——既有 vi.hoisted 装置）；内部 hooks 真实组合，fixture 经 mock IPC 流入 | 本节全部用例 |

---

## 不可测试项

1. **AC-10 工具链门**：`vp test` / `client:check`（含 knip 零新增豁免）/ `server:check` / `bindings:check` 全绿，零新增豁免条目 — **原因**: 工具链级套件门，非单一测试文件可承载，归 test-execution 阶段承接验证（design AC-10 对齐行同口径）。
2. **AC-10 版本交付**：`packages/desktop/package.json` version 0.4.32 → 0.4.33（`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动）+ `plugins/dev-team` 零改动 — **原因**: 静态版本声明与只读红线，无行为面可自动化；由 tasks 阶段六对账与红线自查 grep 承载。
3. **AC-9 流程半边**：`ChangeSummary` / `ChangeDetail` golden 重写 diff 的人工确认与留痕 + `bindings:check` 守线 — **原因**: 流程性动作（人工审阅 diff 范围并留痕）不可自动化；其测试半边（title 两态 golden 断言 + 线形契约）已落 list_test / detail_test 的 corpus_golden 用例。
4. **`packages/desktop/src/types/generated/bindings.ts`**：specta 生成物零手改，新增 DTO 字段（`ExploreRecord.title` / `promotedTo`、`ChangeSummary.title` / `ChangeDetail.title`、`promote_explore` / `update_explore_title` 命令）经 `bindings:export` 再生成 — **原因**: 生成物无自有行为，空框架章节自相矛盾且诱发空套件；签面一致性由 `bindings:check` 守卫承载，消费面断言归各前端章节。
5. **`packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`**：`mod promote;` + `pub use promote::{promote_explore, PromoteOutcome};` 纯 re-export 胶水 — **原因**: 无自有行为；导出正确性由 promote_test 与 commands/explores 各节编译引用承载。
6. **AC-4 调试页 run prompt 不含模板 + AC-5 read_explore 纯读零写入**：调试页不经 `buildExplorePrompt`、`read_explore` 纯读零写入（title 回填走显式写命令）— **原因**: 「不作为」/零触点静态约束，无进程内行为断言面；由 crate 编译（引用残留即编译失败）与守线 grep 承载；其行为半边（explore run prompt 含 name / 回填走 `update_explore_title`）已落 explore-stance / use-explore-session / explore-detail-view 各节。
7. **存量非 kebab explore 的导流改名 UI（R3 留痕）**：本变更仅后端拒绝（kebab 校验）+ promote 错误引导，UI 层面的批量改名入口留后续 — **原因**: 后续变更边界，本变更零 UI 行为面；promote 拒绝断言已落 commands/explores/mod_test.rs（非 kebab name → 显式 `Err`）。
