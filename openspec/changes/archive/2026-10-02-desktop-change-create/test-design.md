# 测试设计: desktop-change-create

> **日期**: 2026-10-02

---

## 验收范围

<!-- 每行 `被测文件或模块` 为该 AC 用例的主要承载测试文件（单值），半边路由如下：
     AC-2 的读面识别 / flow 前置 / 插件形态对照均以写面 create 为链路发起方，组合用例挂靠 create_test.rs（真实组合 queries::list_changes / persist::load_doc / phase_table）。
     AC-4 的 all_commands! 注册 / specta 出线 / bindings 重导出 git diff 干净静态半边见不可测试项 2。
     AC-5 的交接行既有文本 / 相位表与 schema 零改动 / 详情页「探索」条目半边见不可测试项 3；explore.md 即 goal 原文半边落 create_test.rs。
     AC-6 的清单页挂载与 onCreated 流转半边落 change-list-view.test.tsx（proposal 测试文件清单第 4 行「增补」承载行）。
     AC-7 见不可测试项 1。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 对空白树 workspace 以合法名称 + goal 调用后：`changes_root/<name>/workflow.json` 存在且内容恰为 `{"workflow_type":"requirement","created":"<UTC YYYY-MM-DD>","file_log":[]}`（2 空格 pretty + 尾换行、无 `eval` 键）；`changes_root/<name>/explore.md` 存在且内容为 goal 原文（UTF-8） | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs |
| AC-2 | 创建后既有 `list_changes` 以 v2 代际列出该 change；`get_change_detail` 可达；`change_flow_start` 前置校验（存在 + 可解析 + requirement 相位表在位）通过；创建产物可被插件 zod schema 形状解析（key 序 / 字段集一致） | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs |
| AC-3 | 非法 kebab-case（大写 / 下划线 / 空格）、超 128 字符、goal 空白、已存在同名 active change——各返回显式 `Err`，目标目录与文件零产生、既有 change 零改动 | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs |
| AC-4 | `create_change` 经 `all_commands!` 注册、specta 出线、bindings 重导出后 `git diff` 干净；blank root 显式 `Err`；返回 DTO 仅含 `name` 与 `created`（磁盘路径知识不下沉前端） | packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-5 | 相位表 proposal 交接行（既有文本）指引读取 `changes/<change>/explore.md`；新建 change 的 explore.md 即 goal 原文；详情页产物清单含「探索」（explore.md）条目；相位表与 workflow.json schema 零改动 | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs |
| AC-6 | 清单页有新建 toggle；展开后名称 / goal 均必填，名称本地 kebab-case 校验不合法时禁提交且不发起 invoke；成功后清单刷新并导航 `/changes/<name>`；后端错误行内呈现；挂钩均为 data-testid | packages/desktop/src/views/changes/components/change-create-dialog.test.tsx |
| AC-7 | `packages/desktop/package.json` version 为 0.4.1；`plugins/dev-team` 无任何改动 | —（见不可测试项 1） |

---

## 单元测试

<!-- Rust 侧套件为 src-tauri 工作区根注册的 cargo 测试（`*_test.rs` 共置模块，#[test]；write 子模块测试经 `#[cfg(test)] mod create_test;` 挂载）；前端为 packages/desktop 的 vite-plus 测试运行器（`*.test.tsx` 共置）。
     主装置：tempdir 真实 workspace 根 RAII（沿 phase_next_test TempWs / explores mod_test Env 先例）——fs 为进程边界、真实组合零进程边界 mock；「零产生 / 零改动」以目录枚举与调用前后字节比对断言。
     跨模块组合用例挂靠链路入口模块：`创建 → 既有读面 / flow 前置 / 插件形态对照` 挂 create_test（链路发起方 = 写面 create），`create_change 命令 → layout::resolve → write::create → 磁盘产物` 挂 commands/changes/mod_test（最上层调用方 = 命令层），`清单挂载 → 对话框提交 → refresh + navigate` 挂 change-list-view.test（前端链路入口 = 清单页）；describe 标题写链路方向；不设独立集成测试章节、不设 `__tests__/` 组合测试区。
     最小 mock 原则：仅进程边界（`@tauri-apps/api/core` invoke）允许 mock；Rust 侧内部模块（queries / parse / write、Layout）与前端内部模块（useChangeList、ChangeCreateDialog）一律真实组合——change-list-view.test 既有 useChangeList 受控替身不沿用（内部协作 hook 不 mock），状态回灌改经 mock IPC fixture。
     test_resolve_paths 对 `write/mod.rs` / `write/persist.rs` / `commands/mod.rs` / `types/generated/bindings.ts` 解析出的测试文件均不存在且不应创建（声明 / 注释 / 生成物面，防空套件红灯），章节保留空框架、统一落不可测试项 4；`packages/desktop/package.json` 解析报「Not a testable source file」，落不可测试项 1。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

#### 待测功能

- create(): 三道前置校验（kebab-case `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` 语义字符级判定 + ≤128 → goal 非空白 `trim().is_empty()` → 已存在拒绝，全 IO 前置零目录零文件）→ `create_dir_all` 建树 → workflow.json 键序定形写出（`workflow_type → created → file_log`、恒 `"requirement"`、无 `eval` 键、2 空格 pretty + 尾换行、`created` 取 UTC 日期）→ explore.md 写 goal 原文；sync 零 Tauri
- CreateOutcome(): IPC 返回 DTO（`name` + `created` 两字段；`Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type` + camelCase——D1；磁盘路径知识不下沉）

<!-- 链路入口：`创建 → 既有读面 / flow 前置校验 / 插件 createChange 形态对照` 组合用例挂靠本节（链路发起方 = 写面 create），真实组合 crate 内 queries / parse / persist 与 phase_table；describe 标题写链路方向。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| create 合法输入建域 | 正向 | 空白树 tempdir workspace + "fix-bug" + goal 文本：Ok(CreateOutcome{name:"fix-bug", created=当日 UTC YYYY-MM-DD})；workflow.json 字节恰为 2 空格 pretty 三键文档 + 尾换行（键序 workflow_type → created → file_log、无 eval 键——AC-1/D2 字节级契约） | 新增 |
| explore.md 落 goal 原文 | 正向 | 同产物：explore.md 存在且字节恰为 goal 原文（UTF-8、零标题前缀零包装——AC-1/AC-5/D7） | 新增 |
| 空白树深层建树 | 边界 | 调用前 changes_root 全链不存在（tempdir 空根）：create_dir_all 建全树后目录链完整、两文件落位（AC-1「空白树」字面） | 新增 |
| CreateOutcome serde 线形状 | 边界 | serde_json 序列化恰 `{"name":…,"created":…}` 两键（camelCase、零磁盘路径字段——AC-4 DTO 纪律的类型面） | 新增 |
| `创建 → 既有清单读面` | 正向 | create 后真实组合 queries::list_changes：active 恰一条、inventory=v2、source=active、created=当日、unparsable=false（AC-2 v2 识别半边——file_log 键在位判 v2） | 新增 |
| `创建 → flow 前置校验输入面` | 正向 | create 后 persist::load_doc Ok 且 workflow_type="requirement"、phase_table("requirement") 为 Some：change_flow_start 三项前置（存在 + 可解析 + 相位表在位）对新文档天然通过（AC-2 发起半边） | 新增 |
| `创建 → 详情读面（含探索条目）` | 正向 | create 后真实组合 queries::change_detail：Some 且产物清单含 explore.md→「探索」条目（markdown_doc 既有收录规则零改动沿用的组合证据——AC-2 detail 可达半边 + AC-5 详情条目的数据面） | 新增 |
| `创建 → 插件 createChange 形态对照` | 边界 | 磁盘 workflow.json 解析 Value 与插件紧凑单行 fixture `{"workflow_type":"requirement","created":"<当日>","file_log":[]}` 解析 Value 全等（字段集 / 值一致；序列化差异仅空白布局，键序已由字节级行锚定——AC-2 zod 可解析半边 / 风险表「形状漂移」缓解锚） | 新增 |
| 非法 kebab-case 全族拒绝 | 异常 | 大写 / 下划线 / 空格 / 前导数字 / 尾连字符 / 连号连字符 / 前导连字符 / 空串 / 穿越分量（"a/b"、"../x"）：各 Err 且 changes_root 下零目录零文件（AC-3/D3 spec 正则语义） | 新增 |
| 超 128 字符拒绝 | 异常 | 129 字符合法字符集名：Err 且零产生（AC-3） | 新增 |
| goal 空白拒绝 | 异常 | goal "" / "   " / "\n\t"：Err 且目标目录与文件零产生（AC-3） | 新增 |
| 已存在同名拒绝 | 异常 | 预置既有 change（workflow.json 带既有字节）再 create 同名：Err（错误串含目录路径）且既有 workflow.json / explore.md 字节零变更（AC-3） | 新增 |
| 校验顺序名称优先 | 边界 | 非法名 + 空白 goal 同投：Err 归因名称校验（错误信息可辨——D4 顺序锚：名称 → goal → 已存在） | 新增 |
| 校验顺序 goal 先于已存在 | 边界 | 合法名 + 空白 goal + 同名已预置：Err 归因 goal 空白（D4 顺序第二锚） | 新增 |
| 恰 128 字符放行 | 边界 | 128 字符合法 kebab 名：Ok（≤128 含端点——D3 宽度上界） | 新增 |
| 单字符与数字段放行 | 边界 | "a"（单字符）、"fix-bug-2"（数字段）：Ok（正则正样本） | 新增 |
| goal 首尾空白保真 | 边界 | goal 带首尾空白：explore.md 原样保留（写面零 trim——trim 为前端提交前职责 D6） | 新增 |
| goal 多行与特殊字符保真 | 边界 | 多行 + emoji + >1000 字符 goal：explore.md 字节保真（UTF-8 free-form——AC-5；str 边界映射空 / 空白 / 特殊字符 / 超长全覆盖） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无进程边界 mock（fs 真实组合） | tempdir 空白 workspace 根 RAII 装置（沿 phase_next_test TempWs 先例）；插件 createChange 形态以紧凑单行 fixture 字符串锚定（进程内字符串常量，非 mock）；「零产生 / 零改动」以目录枚举与调用前后字节比对断言 | 全部 describe |

### packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/mod_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表明示 mod.rs 为声明与再导出面、不设独立条目（修改行：`mod create;` 声明 + `pub use create::{create, CreateOutcome};` + `#[cfg(test)] mod create_test;` 挂载位，既有四操作导出零改动）。不建 mod_test.rs（防空套件红灯），见不可测试项 4。 -->

（design.md 未声明该文件的公共 API 变更——不建测试文件，可达性经 create_test 以 write:: / super:: 路径导入编译期承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 4） | — | 导出面无独立行为断言：create_test 以 write::create / write::CreateOutcome 路径导入，任一破坏即编译失败 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/persist_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：load_doc doc 注释「change_create 是唯一创建者，写面从不创建文件」换血为「插件 MCP change_create 与桌面写面 create 并存」双创建者声明——零逻辑改动）。不建 persist_test.rs（防空套件红灯），见不可测试项 4。 -->

（design.md 未声明该文件的公共 API 变更——注释换血零逻辑改动，无行为断言落点。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——注释换血零逻辑改动，见不可测试项 4） | — | load_doc 行为零变更由写面既有四操作 *_test 与 create_test 的读写回路回归承载 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

- create_change(): 三件事薄包装：blank root 显式 `Err`（不进入写面链路）→ foundation `layout::resolve` → 写面 create → `Result<CreateOutcome, String>` 错误映射；`#[tauri::command]` + `#[specta::specta]` 出线；sync 纯函数无缝直测（D5——无 State / AppHandle / Channel，无需 mock_app 装置）

<!-- 链路入口：`create_change 命令 → layout::resolve → write::create → workflow.json/explore.md 磁盘产物` 组合用例挂靠本节（链路最上层调用方 = 命令层），describe 标题写链路方向。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `create_change 命令 → layout::resolve → write::create → 磁盘产物` | 正向 | tempdir root + "fix-bug" + goal：Ok(CreateOutcome) 与直调 write::create serde 等值（薄包装不加工）；workflow.json + explore.md 真盘落位（AC-4 薄包装半边） | 新增 |
| 返回 DTO 仅两字段 | 边界 | serde_json::to_value(返回值) 恰 `{"name":…,"created":…}` 两键、零磁盘路径字段（AC-4 DTO 纪律） | 新增 |
| blank root 显式 Err 不进写面 | 异常 | root "" / "   "：Err 且 changes_root 下零新目录（命令层拦截，不进入写面链路——AC-4；沿 explores 组写命令 blank root 纪律口径） | 新增 |
| 写面拒绝透传 | 异常 | 非法名 / 空白 goal 经命令层：Err（错误映射不吞不加工）且零产生（AC-3 命令入口同口径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无进程边界 mock（fs 真实组合） | tempdir workspace root 真盘；create_change 为 sync 纯函数命令（无 State / AppHandle / Channel——D5），无需 tauri mock_app 装置，直调即测 | 全部 describe |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/commands/mod_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表明示 commands/mod.rs 为声明与登记面、不设独立条目（修改行：`pub mod changes;` + `all_commands!` 宏清单追加 create_change——单一登记面）。不建 mod_test.rs（防空套件红灯），见不可测试项 2 / 4。 -->

（design.md 未声明该文件的公共 API 变更——登记面无进程内断言落点。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 2、4） | — | all_commands! 登记由编译期 specta builder 与 bindings 零 diff 自检静态核查（AC-4 静态半边） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src/views/changes/components/change-create-dialog.tsx -> packages/desktop/src/views/changes/components/change-create-dialog.test.tsx

#### 待测功能

- ChangeCreateDialog(): toggle 展开；名称（kebab-case 提示）+ goal 多行输入均必填；提交前 trim + 本地同口径校验（`^[a-z][a-z0-9]*(-[a-z0-9]+)*$` 语义 + ≤128），不合法禁提交且不发起 invoke；`commands.createChange(root, name, goal)`；后端错误 break-all 行内块；成功回调 onCreated(name)；data-testid 挂钩（change-create-dialog / change-create-toggle / change-create-name / change-create-goal / change-create-submit / change-create-error）
- ChangeCreateDialogProps(): props 契约 `{ root: string; onCreated: (name: string) => void }`（非 null——D6 沿 ExploreCreateDialogProps 先例；纯类型，由用例构造编译期锚定）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| toggle 展开 / 收起 | 正向 | 点击 change-create-toggle：名称 / goal 输入与提交按钮呈现；再点收起、输入区不残留（沿 explore-create-toggle 先例） | 新增 |
| 合法输入提交成功流转 | 正向 | 名称 "fix-bug" + goal 文本：invoke("create_change") 恰一次、载荷 {root, name, goal}；resolve CreateOutcome {name, created} → onCreated("fix-bug") 恰一次（AC-6 成功回调半边） | 新增 |
| 提交前 trim | 边界 | 名称与 goal 带首尾空白：invoke 载荷为 trim 后值（沿 explore TopicEntry trim 先例——D6） | 新增 |
| 名称 / goal 空缺禁提交 | 异常 | 名称空或 goal 空白（"" / "   "）：提交按钮 disabled、点击零 invoke（必填拦截——AC-6） | 新增 |
| 非法 kebab-case 禁提交不 invoke | 异常 | 大写 / 下划线 / 空格 / 前导数字 / 尾连字符 / 连号连字符 / 超 128 字符：禁提交且零 invoke（本地与写面同口径校验——AC-6；「本地放行 ⇒ 后端必过」D3） | 新增 |
| 后端错误行内呈现 | 异常 | invoke reject：change-create-error 行内块呈现错误文本、onCreated 零调用（AC-6 错误呈现半边） | 新增 |
| 恰 128 字符名称可提交 | 边界 | 128 字符合法名：可提交且 invoke 发起（≤128 含端点，与写面上界同口径） | 新增 |
| goal 多行与特殊字符透传 | 边界 | 多行 + emoji + >1000 字符 goal：载荷原样透传不损（free-form UTF-8 前端半边——D7） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（@tauri-apps/api/core，进程边界） | invoke 按命令名分发：create_change 可切换 resolve（CreateOutcome fixture {name, created}）/ reject（错误串），记录入参断言载荷与调用次数 | 全部 describe |
| 内部模块真实组合（不 mock） | ChangeCreateDialog 直渲染（onCreated 以 vi.fn spy 经 props 注入——入参例外）；输入 / 校验 / 流转逻辑真实参与，不 mock | 全部 describe |

### packages/desktop/src/views/changes/change-list-view.tsx -> packages/desktop/src/views/changes/change-list-view.test.tsx

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的导出函数（ChangeListView 为既有导出；修改行：root 非空时于头部行下挂载 ChangeCreateDialog + 增 onCreated 回调（state.refresh() + navigate）——design D6 与 AC-6 流转半边；proposal 测试文件清单第 4 行「增补」承载行）。本节承载挂载与流转半边；既有清单渲染 / 分组 / 徽标 / 空态语义用例沿用，装置改锚见 Mock策略与废弃行。 -->

- ChangeListView(): 既有清单视图 + root 非空时挂载 ChangeCreateDialog；onCreated = state.refresh() + navigate('/changes/<name>')（D6；创建后不自动发起 run）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| root 非空挂载新建入口 | 正向 | root="/repo" 且清单载入：change-create-toggle 在场（头部行下挂载——AC-6 挂载半边 / D6） | 新增 |
| root 为 null 不挂载 | 边界 | root=null：toggle 不在场、create_change 零 invoke（无 workspace 无建档语义——D6） | 新增 |
| `清单挂载 → 对话框提交 → refresh + navigate` | 正向 | 展开 toggle、填合法名称与 goal、提交：invoke("create_change") 恰一次 → 清单刷新（invoke("list_changes") 调用数递增）+ pathname 落 /changes/<name>（LocationProbe 观测）；change_flow_start 零调用（不自动发起 run——proposal 拍板） | 新增 |
| 创建失败不停流转 | 异常 | invoke("create_change") reject：行内错误呈现、pathname 不变、清单不刷新（refresh 未触发） | 新增 |
| useChangeList hook 替身入参断言 | 废弃 | 既有「清单页自取数布线：useChangeList 以 root 入参调用」的内部 hook 受控替身与入参观察断言随真实组合改锚出局；「挂载即以 root 取数」语义由 invoke("list_changes", {root}) 进程边界断言承接（mock 纪律：内部协作 hook 不 mock） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（@tauri-apps/api/core，进程边界） | invoke 按命令名分发：list_changes 以 mockResolvedValue / mockRejectedValue / pending promise 回灌 ChangeList fixture，驱动真实 useChangeList 的 data / loading / error / 空态四形态；create_change 可切换 resolve / reject | 全部 describe |
| 内部模块真实组合（不 mock） | useChangeList / ChangeCreateDialog / react-router（MemoryRouter + Routes + LocationProbe 装置沿既有）真实组合；既有 `vi.mock('./hooks/use-change-list')` 文件级替身不沿用、随改锚移除，既有清单渲染 describe 的状态回灌改经 mock IPC fixture（断言语义不变） | 全部 describe |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/types/generated/bindings.test.ts

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（specta 生成物：经 bindings:export 再生成、非手改——createChange typed 包装 + CreateOutcome 类型出线）。纯生成物不建测试文件（防空套件红灯），见不可测试项 4。 -->

（生成物无手写运行时行为——出线正确性由 bindings 零 diff 守卫与 Rust 侧 serde/specta 断言承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 4） | — | 生成物零手写行为：createChange / CreateOutcome 出线正确性由 bindings 零 diff 自检与 create_change / CreateOutcome 的 Rust 侧 serde/specta 断言承载 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

---

## 不可测试项

- AC-7 版本交付半边：`packages/desktop/package.json` version 0.4.0 → 0.4.1（`tauri.conf.json` 经 `../package.json` 自动跟随、`src-tauri/Cargo.toml` 不随动）、`plugins/dev-team` 全树零改动（`change-create.ts` / MCP 工具面 / 版本 2.10.44 / 三类交付产物） — **原因**: 配置值与仓库冻结面核查，无进程内可断言行为，由静态检查与评审阶段核查（design「验收标准对齐」AC-7 行同款口径）；test_resolve_paths 对 `packages/desktop/package.json` 返回「Not a testable source file」，无对应测试文件。
- AC-4 静态半边：create_change 经 `all_commands!` 注册、`#[specta::specta]` 出线、bindings 重导出后 `git diff` 干净 — **原因**: 构建管线静态核查（编译期 specta builder + bindings 零 diff 自检守卫），非进程内可断言行为；命令本体的可自动化半边（薄包装 serde 等值 / blank root Err / DTO 仅两字段）由 commands/changes/mod_test.rs 承载。
- AC-5 静态半边：phase_table.rs proposal 交接行既有文本零改动、相位表与 workflow.json schema 零新字段（变更清单零 `model/**` 与 `phase_table.rs` 条目）、详情页「探索」条目的 UI 渲染呈现 — **原因**: 零改动承诺与既有能力沿用，无新增可断言行为；「探索」条目的数据面已由 create_test「创建 → 详情读面（含探索条目）」组合行承载，UI 渲染由 markdown-doc 渲染既有场景回归（套件全绿口径）；explore.md 即 goal 原文半边由 create_test.rs 承载；proposal-planner 运行期实际 Read explore.md 为 agent 行为，不在进程内单测射程。
- 声明 / 注释 / 生成物组：`write/mod.rs`（导出面）、`write/persist.rs`（doc 注释换血零逻辑）、`commands/mod.rs`（all_commands! 登记面）、`types/generated/bindings.ts`（specta 生成物） — **原因**: 纯导出 / 注释 / 清单 / 生成物无可执行行为，不建对应测试文件（防空套件红灯——vitest 零用例文件直接报错）；可达性由 create_test / mod_test 的 use 导入编译期承载；bindings 出线正确性由零 diff 自检与 Rust 侧 serde/specta 断言承载。
- IO 中途失败残留（workflow.json 已写而 explore.md 写失败的目录半成品） — **原因**: design D4 与待决问题 V1 显式接受非事务形态（与插件 createChange「mkdir 后 writeFileSync」同型、无补偿、显式 Err 抵达调用方），无补偿行为可断言；实测出现后另行评估残留目录清理。
