# API 契约生成与检查接进桌面端

workflow_type: requirement

## 一句话现状

「生成」与「零 diff 检查」两条能力现在**只在 CLI 侧存在**（`crates/tools/archi-api-gen`，
由 `pnpm api:export` / `api:check` 驱动）；桌面端里只有「十条判据」这一半（`apiFindings`
随 `archi_drift` 报告出线），而它**前端零渲染、不进 `status` / `passed`、报告之外无人消费**。

注意一个易误判的事实：`api:check` 的零 diff 判定**已经间接进了门禁**——它住在
`static_analysis`（`pnpm run -C ./packages/desktop check` 的四段链）里，随 static-check 步
`passed=false` 硬拦，只是输出混在 lint 噪音中、无法与代码失败区分（详见「硬线一」）。

```
手写真相  .dev-team/architecture/api/<Domain>/<FQN>.md
   │ core/archi::api::contract 解析
   ▼
内存四类生成物  render_all() -> Vec<GeneratedFile>   ← 纯读盘、零写盘
   │                                  │
   │                       ┌──────────┴───────────┐
   ▼                       ▼                      ▼
CLI generate           CLI --check            api::check()（十条判据）
(写盘)                 (differences 在         → drift::check 报告 apiFindings
 tools/archi-api-gen    tools/src/main.rs)      → archi_drift 命令 + LocalArchiCheck 门禁
 = 唯一写盘面                                   → 前端：无人消费
```

冷启动护栏：`<archi_api_dir>` 缺席或零份 `*.md` ⇒ 判据全空。本仓现有 1 份试点契约
（`Infra/App.Backend.Infra.ArchiRuntime.md`）+ 1 份 schema + 1 份 `archi.i.rs`；
基线 28 `apiEntryMissing` / 13 `apiMissing`。

## 三段可拆的接缝

1. **判定面**（纯）：契约解析 / 渲染 / 十条判据 —— 已在 `core/archi`，可直接复用。
2. **比对面**（纯读盘）：`mismatched / orphaned / missing` 三类差异 + 全树找 `*.i.*` 的
   walker —— **在 tools 的 `main.rs` 里**，不在 core。任何第二消费面出现前必须上移，
   否则「判定单源」纪律当场破（本仓硬纪律：命令面与门禁面 MUST 同源）。
3. **写盘面**（危险动作）：目录创建 + 覆盖写 + 越根复核 —— tools 的 `write()`。
   spec（`desktop-crate-layout`）明写：`archi-api-gen` 是唯一写盘面，**MUST NOT 被
   app / core / infra 依赖**（避免 `tauri build` 拖带工具 bin；`export-bindings`
   为此付过 RT_MANIFEST 补丁的代价）。

→ 想让 app 自己「生成」，第 3 段必须换家；想让 app 自己「检查」，第 2 段必须上移。
这两步是「接进系统」的真实改动量所在，不是加两个 IPC 命令。

## 消费面候选（可叠加）

| 消费面 | 形态 | 现状 | 接进后 |
|---|---|---|---|
| `archi_drift` 报告 | `apiFindings` 并列槽位 | 已产出，无人看 | 前端渲染 + 可选进 `status` |
| IPC 命令 | `archi_api_check` / `archi_api_generate` | 无 | 架构页「契约」面：一键检查 / 生成、三类差异清单、orphan 清理 |
| 工作流门禁步 | `StepNode::ApiCheck` 或并入 `ArchiCheck` 步 | 无 api 步（implement 相位只有 StaticCheck → ArchiCheck → Evaluator） | executor 改完代码即被「生成物过期」打回，feedback loop 计数独立 |
| agent 工具 | `archi_api` 专用工具 | 只有 `archi_module` / `archi_search` 两个只读工具 | **本轮入范围（D 面）**：会话里 check 两轴 + bootstrap 契约草稿；生成经 bash 既有命令 |
| 写保护 | 内置锁定 config.json 一条 | 生成物**可被 agent 手改**（只能靠零 diff 门兜） | **本轮不锁**（裁决见「硬线二」）；命名判据 P1 后上移 core，为将来内置化留门 |

## 四条路线（取舍不同，不互斥）

**A. 只把判据接进 UI**（最小）
- 前端渲染 `apiFindings`（按 `category` 分组：entry / contract / signature / reexport / nesting / constructor），元素抽屉加「契约」区（`api.status` / `contractPath` / `schemaPath` / `generatedPath` 已在 `ArchiApiRef` 里躺着）。
- 零 Rust 改动。生成仍需 `pnpm api:export`。

**B. 生成 + 检查都进 IPC**（能力面完整）
- 比对上移 `core/archi::api`（新增 `diff(root) -> DiffReport`，纯读盘）；
- 写盘面挪到 app 可达层（三条备选已细化为 P1 / P2 / P3，见「B 的结构抉择」）；
- 前端架构页新增契约面板（检查 / 生成 / 差异清单）。

**C. 进工作流门禁**（把债变成机器判的）
- 零 diff 检查并入 `ArchiCheck` 步（报告新增槽位）或新 `StepNode::ApiCheck`（implement / test-gen 相位，独立 feedback 计数）。
- 门禁里**只 check 不 generate**（自动写盘 = 会话之外静默改工作树）。
- 成本待测：全量 `render_all` + 全树 walk（`ignore` 已依赖，可换 gitignore 感知遍历）。

**D. 给 agent 工具面**（本轮入范围）
- 新增 `archi_api` 专用只读工具（check / bootstrap 两动作），注入 archi 会话与 implement/executor；
- 与 B 共用 core 的判定面；写盘（生成）仍经 bash 既有命令——详见「D 面」段。

## 已定方向：A + B + D（UI 渲染 + IPC 生成/检查 + `archi_api` 只读工具）

### 两个正交的「检查」概念 —— 面板必须分开呈现

| 轴 | 事实源 | 谁修 | 数据来源 |
|---|---|---|---|
| **契约 vs 代码** | 十条判据（`apiFindings`） | 人或 agent 改代码 / 改契约 | `archi_drift` 报告（已有，架构页已取数） |
| **生成物新鲜度** | 四类生成物字节 vs 契约渲染结果 | 机器可一键修（生成） | 新 IPC（比对上移 core 后） |

`apiFindings` 的分类词已是六值（entry / contract / signature / reexport / nesting / constructor），
渲染直接按 `category` 分组即可，零新词汇。

### A 面（UI）的具体落点

- **元素抽屉**（`element-drawer.tsx` 现序：概览 → 文档 → 豁免 → 改档）插一段「契约」：
  `ArchiModuleDetail.api`（`ArchiApiRef`：`status` + `contractPath` / `schemaPath` /
  `generatedPath`）**已经躺在返回里**，前端一次都没读过。三个路径行照 `DocRow` 的形态
  开 `FileViewer` 即可（`read_file` 已通用）。
- **页级清单**：`apiFindings` 按元素过滤后进抽屉（本元素的判据）、页级列全量（可折叠），
  `apiEntryMissing` 28 条 / `apiMissing` 13 条基线就是这个面板的初始画像。
- 画布**不画契约标记**：`desktop-archi-canvas` 明写「节点标记不呈现」，
  `drift-marks.ts` 只产边标记——契约面走抽屉与清单，MUST NOT 破这条。

### B 面（IPC）的两个命令

```
archi_api_check(root)    -> { enabled, meta, mismatched[], orphaned[], missing[] }   零写入
archi_api_generate(root) -> { written[], unchanged[] }                              写盘
```

- **check 即生成预览**：`--check` 的三类差异（内容不符 / 应删未删 / 应生成未生成）
  正好是「点生成会写什么」的完整清单。故 UI 次序 = 先 check 看清单 → 再点生成。
- **generate 从不删文件**（已核实：`Command::Generate` 只写不删，orphan 仅报告）——
  写盘危险等级比预想低一档：`create_dir_all` + 覆盖写 + 越根复核，无删除分支。
- 停摆安全：模型坏时 `graph_view().elements` 为空 ⇒ 应生成集为空 ⇒ 生成零写、orphan 全量报告，
  不会误删（因为根本不删）。

### B 的结构抉择：写盘面搬去哪（真正需要裁决的只有这一条）

| | 形态 | 得 | 失 |
|---|---|---|---|
| **P1** | 写盘单点搬 `infra/archi`（`report.rs` 报告落盘的同型先例）；CLI 加 infra 依赖改调它 | 写盘函数单点；两驱动面（CLI / IPC）同源 | 破 `archi-api-gen` 的「零 infra 依赖」面 + crate-layout spec 两处措辞要改；tools 二进制拖进 orchestration |
| **P2** | 壳层写（`commands/archi` 的 `write_atomic` 同型）；CLI 保留自己的 `write()` | 依赖图零变更；壳已依赖 archi | 两份「15 行 dumb IO + `starts_with(root)` 越根复核」；「写盘单点」纪律破（判定/路径集仍单源 core） |
| **P3** | 写盘留 CLI，app 只做 check + 预览 + 「复制 `pnpm api:export`」 | 零写盘风险，改动最小 | 用户在 app 内仍要跳出去执行命令，B 只兑现一半 |

P3 保留价值的部分：**实施分两段**——先落 check 面（比对上移 + IPC + 面板，纯读零副作用），
再落 generate 面（P1 的写盘面搬家 + spec 措辞改造）。同一 change 内两段，第一段末已可交付价值。

### 上移比对面的细节

- `differences` + `generated_files_on_disk` 从 tools 的 `main.rs` 移入 `core/archi::api`
  （纯读盘，破不了零写盘纪律）；`GeneratedFile` 已是 core 类型，`archi::display` 已是现成
  的 repo 相对显示出口。
- 现 walker 是手写递归 + 跳过 `target / node_modules / dist / build / .git`；`archi` 已直接
  依赖 `ignore`（漂移扫描用的就是 gitignore 感知遍历）→ 上移时换 `ignore` 顺带消一套手写遍历
  （行为差异：`.gitignore` 里的路径不再被扫，属收紧，需在报告/测试里留痕）。

**裁决：P1**（写盘单点搬 `infra/archi`，CLI 改调它，两驱动面同源）。P2 出局理由：写盘是最
危险的动作（越根复核 + 批量覆盖），两份实现意味着两份安全判据靠人肉同步。

## P1 执行方案（逐文件）

```
core/archi/src/api/
  ├─ render.rs  (新) workspace_render(root) -> Result<Vec<GeneratedFile>, ApiError>
  │                   收编 CLI 的 collect + model_elements（graph_view 读元素 → load_contracts → render_all）
  └─ diff.rs    (新) compare(root) -> Result<ArtifactDiff, ApiError>
                      收编 CLI 的 differences + generated_files_on_disk
                      ArtifactDiff { mismatched[], orphaned[], missing[] }，串经 archi::display（repo 相对）

infra/archi/src/
  └─ api_artifacts.rs (新) write_artifacts(root, &[GeneratedFile]) -> Result<WriteOutcome, String>
                      收编 CLI 的 write()：starts_with(root) 越根复核（第二层）+ create_dir_all + fs::write
                      WriteOutcome { written[], unchanged[] }（先读盘比对再写——UI 要显示「写了几份」）

tools/archi-api-gen/src/main.rs
  → 只剩 argv 解析 + 退出码 + 三子命令编排；deps 加 archi-runtime（带用途注释）
  → 现有纪律测试翻转：main.rs 的 `fs::write` / `create_dir_all` 从「恰一处」变「零处」，
    单点断言迁到 infra 侧

src-tauri/src/commands/archi/mod.rs
  → 新增 archi_api_check / archi_api_generate + 两个 *_inner（check_root → 调 core / infra）
src-tauri/src/commands/mod.rs
  → all_commands! archi 段 6 → 8 条

packages/desktop/src/views/architecture/
  → hooks/use-archi-api-check.ts（挂根 + 手动刷新，零轮询）
  → components/contract-panel.tsx（两轴分列 + 生成按钮 + 契约资产 meta）
  → components/element-drawer.tsx 插「契约」段（ArchiApiRef 三路径 + 本元素 findings）
```

### 连带必须同批改的登记面（不全是代码）

| 面 | 现状措辞 | 改后 |
|---|---|---|
| `desktop-crate-layout`「工具 crate archi-api-gen 落位」 | 「新增入边：无」「允许写盘（工具边界）」 | 新增入边 `archi-runtime`（带用途注释）；写盘许可改述为「写盘函数单点驻 `infra/archi`，tools 只是驱动面」 |
| `desktop-crate-layout`「infra/archi 落地登记」 | 依赖恰为 archi / orchestration / foundation | 依赖不变，追加一句「本 crate 承载生成物的唯一写盘函数」 |
| `desktop-archi-model`「IPC 命令面契约（六命令）」 | 六命令、唯一写命令 `archi_set_gate` | 八命令、写命令两条（`archi_set_gate` / `archi_api_generate`），Purpose 标题同步 |
| `desktop-archi-api`「生成物入仓与零 diff 门挂载」 | 脚本面封闭两条 | 追加「应用内两条 IPC 消费同一实现」；脚本面两条原文不变 |
| `desktop-archi-api-check`「报告并列槽位 apiFindings」 | 「首期不进 status / passed」 | 本轮不改；追加「apiFindings 经架构页面板呈现」 |
| `archi` crate doc / workspace `Cargo.toml` 注释 | 「唯一写盘面是 tools」 | 「唯一写盘函数在 infra/archi，tools 与 IPC 两个驱动面」 |
| `bindings/mod_test.rs` 清单守卫 | 命令名 / DTO 清单计数、archi 族两组清单 | 计数随两命令 + 两个 DTO 顺延；尾部序断言插在 `archi_file_tree` 之后 |
| `package.json` | `version` | patch 顺延（桌面端源码变更纪律） |

### 风险与待测项

1. **比对面语义漂移**：walker 换 `ignore`（gitignore 感知）后 `.gitignore` 里的路径不再被扫
   ——属收紧。`diff.rs` 用例须锚定「无关同名文件 / 构建产物目录不误报」与「`.gitignore` 命中不扫」
   两条，并留痕说明这次语义变化。
2. **契约面未启用时的 INDEX**：现行为是「无契约面 → 应生成集只含 `INDEX.toml`」，`--check`
   首轮必报 missing。app 面板要能呈现这种冷启动态，MUST NOT 渲染成空面板了事。
3. **路径口径**：`archi::display` 只 strip 前缀不做规范化（反斜杠归一为 `/`）；UI 用这些串开
   `FileViewer`（root + 相对路径），需确认 `read_file` 接受同形态。
4. **成本**：`workspace_render` 是全量（模型元素 × 契约 × 声明抽取），MUST NOT 与画布取数绑死
   ——面板按需触发（进面板 / 点检查），不随 refresh 自动跑。
5. **bindings 与 zod 再生成**：新 DTO 进 `bindings.ts` / `bindings.zod.ts`，
   `lib/ipc-contract.test.ts` 的契约等价面随动。

## 硬线一：`apiFindings` 要不要进 `status` / `passed`

### 现状的准确形状

```
derive_status(structural, scanned, errors, warnings) -> ArchiCheckStatus{clean,errors,warnings,noChanges,skipped}
                                                            ↑
                                         api_findings 不在这四个入参里（装配在 derive 之后）

LocalArchiCheck.passed = report.errors.is_empty() && report.model.structural_errors.is_empty()
                                                            ↑
                                                    warnings 档不拦（passed 只看 errors / structural）
```

**两个槽位是两回事**：`status` 是报告词汇（人读），`passed` 是门禁布尔（机器判）。
所以存在一个被忽略的中间档：**进 `status` 但不进 `passed`** —— 让 `apiFindings` 把 status
拉到 `warnings`，门禁依旧绿，因为 `passed` 根本不看 warnings。

### 更关键的发现：生成物零 diff **已经在门禁里了**

```
config.static_analysis = "pnpm run -C ./packages/desktop check"
                              └─ server:check → bindings:export → api:check → client:check
crates/infra/checks/src/static_check.rs → spawn `{command} 2>&1`
```

也就是说 `api:check` 的零 diff 判定**已经随 static-check 步硬拦**（`passed=false` + 反馈边，
implement / test-gen 两个相位都有这一步）。集成到 app 的价值不在"补一道门"，而在**分轴与可读**：
现在 agent 收到的是「静态检查失败」+ 一段混在 lint 噪音里的 CLI 单行 JSON，
无法区分「代码 lint 红了」还是「生成物过期了」。两根轴并成一根是当前的病灶。

### 三个口径与后果

| 口径 | 实现 | 本仓后果 | 问题 |
|---|---|---|---|
| **① 现状**：完全不参与 | — | 41 条判据纯装饰，机器零拦 | 债不可见（除人开报告） |
| **② severity 折算进 status** | `api_findings` 里 `severity()==Warn` 的非空 ⇒ status 至少 `warnings` | 28 + 13 = 41 条 ⇒ **status 恒 `warnings`**；15 份金样 status 全改；warnings 语义被常规化稀释 | 契约面一启用就永久黄灯，噪音无梯度 |
| **③ 契约在案即纳管**（推荐） | 只取「`api.status=declared` **且契约文件在案**」元素的 findings 参与 status | 本仓仅 1 个元素（试点），findings 0 条 ⇒ **status / 金样零改动** | 需在报告侧多一步「契约在案集合」筛选 |

③ 的口径理由：**契约即承诺**。没写契约的元素，其 `apiEntryMissing` 是迁移 backlog（存量债，
spec 已明写"不得为消债而重构"）；写了契约的元素，判据就是"自己刚承诺的东西没兑现"——这才该进状态词。
它天然自激活：契约逐份落地，纳管面自动扩大，**无需白名单、无需开关、无需配置键**。

### 建议的落地形态

- `derive_status` 增一个入参（或 `drift::check` 内先算 `api_findings_gating: &[ArchiApiFinding]`）；
  报告 `apiFindings` 字段形状零改动（仍是全量），只是 status 推导多读一个子集。
- `passed` **本轮不动**：`apiFindings` 永不产生 `errors` 槽 ⇒ 门禁恒绿。收紧路径留给独立变更
  （把某类判据 `severity()` 改成 error + 改 passed 判据，两处同批）。
- 报告版本：`REPORT_VERSION` 是否需要 +1 —— 若只改 `status` 的推导输入而字段零改，
  版本语义变化仍是"消费方按版本分支"的边界，需在 design 里定（我倾向 +1 并在金样里留痕）。

## 硬线二：生成物要不要进写保护

### 现状的准确形状

- 内置锁定**只有一条**：`**/` + `layout::config_path` 的配置文件路径，`ReasonKind::BuiltinConfig`，
  **fail-closed**（任何配置状态都不影响）。
- 用户规则：`.dev-team/config.json` 的 `write_protection.files[]`（本仓目前只有 `packages/desktop/knip.json`）。
- 射程：`write` / `edit`（按 path）+ `bash`（路径 token / 重定向 / 内联脚本 / 批量还原）。
  **子进程写盘不追** —— `pnpm api:export` / `cargo run -p archi-api-gen` 一律通过。

### 关键判断

1. **能 derive 吗**：`rules.rs` 纪律要求内置 glob 一律经 `foundation::layout` 派生（源码零域目录名字面量）。
   四类生成物里 `<archi_api_dir>/**/*.schema.json` 与 `<archi_api_dir>/INDEX.toml` 可派生；
   `*.i.rs` / `*.i.ts` 的命名判据属 `archi::api`（P1 后正好上移 core）——`agent-runtime` 已依赖 `archi`，
   可直接复用，不写第二份命名判据。
2. **内置 vs 用户规则**：内置 = 产品语义（任意有契约面的工作区都锁）；用户规则 = 本仓一行配置。
   内置要新增 `ReasonKind` 变体 + 一条新文案（文案禁用域目录名，得说"生成物"而非文件名）。
3. **诚实边界**：保护面只挡"手工改生成物的常见路径"；`pnpm api:export` 这类子进程写盘天然穿过。
   所以它是**事前护栏**（挡住 `edit .i.rs`），零 diff 门仍是**事后兜底**——两者不可互相替代。
4. **反作用**：`*.i.rs` 全局锁定会挡住在无 cargo 环境下"手写临时代替"的应急路径——但那本来
   就是零 diff 门要红的形态，锁它是对齐的。

### 裁决：本轮不锁（保持现状）

**决定**：生成物不进写保护面（既不加用户规则、也不内置化）。手改仍由零 diff 门事后兜。

**理由（留痕）**：

1. 零 diff 门已经随 `static_analysis`（`pnpm check` 四段链）在 static-check 步硬拦，
   "手改生成物"当前**不会静默腐烂**——事后兜底在位，事前护栏的边际收益下降。
2. 集成后 UI 面板会直接呈现三类差异，人看得到"生成物过期"；再叠一层事前拦截，
   等于同一事实三处提示（面板 / 门禁 / 拒绝文案），诊断面反而变吵。
3. 保护面只挡常见路径（`write` / `edit` / shell 里的路径 token），子进程写盘天然穿过；
   在"能绕过"的前提下先上产品级内置规则，收益不足以抵消两处 spec 改写 + 新文案的代价。

**将来重开这条的条件（任一条成立即重新评估）**：

- 契约面从"本仓试点"扩到多工作区复用 ⇒ 收益变成产品级；
- 零 diff 门从 static-check 链路里被摘掉（届时无事前无事后的双重保障）；
- 实际观察到 agent 手改生成物的案例（面板里反复出现"内容不符"且非契约改动引起）。

**P1 顺带留的门**：`.i.rs` / `.i.ts` 命名判据上移 `core/archi` 后，将来自内置化只需在
`protect::rules` 侧派生（`agent-runtime` 已依赖 `archi`，`layout` 已供目录），不必再造第三份判据。

## 已裁决汇总（本次探索的结论面）

| # | 议题 | 裁决 |
|---|---|---|
| 1 | 集成范围 | **A + B + D**：UI 渲染 + IPC 生成/检查 + `archi_api` 只读专用工具（C 门禁步本轮不做） |
| 2 | 写盘面归属 | **P1**：写盘单点搬 `infra/archi`，`archi-api-gen` CLI 改调它（两驱动面同源） |
| 3 | `apiFindings` 进 status | **③ 契约在案即纳管**：只取契约文件在案元素的判据；`passed` 本轮不动 |
| 4 | 生成物写保护 | **本轮不锁**：保持现状，零 diff 门 + 新 UI 面板两面兜底（重开条件见上） |
| 5 | 面板落点 | 左栏分段控件第三模式「契约」（文件 / 元素 / **契约**） |
| 6 | 生成权限 | **U1 + D**：画布生成仅在 change 详情（`incremental`）；架构页保持只读，写盘走页内 agent 会话（新 `archi_api` 只读工具 + 既有 bash 生成命令） |

## UI 面板形态（细画）

### 落点：左栏分段控件加第三模式「契约」

左栏现在已是「一个分栏三种视角」的范式（`TreeModeSwitch`：文件 / 元素，`TreeMode` 两值枚举，
选项表驱动），契约面加第三值 `contract` 的改造成本 ≈ 一张选项表 + 一个分支：

```
ArchitectureView（标题 + 刷新）
└─ ResizablePanelGroup 横分
   ├─ ArchiChatPanel（左栏，默认收起）
   └─ ArchitectureCanvas (mode = full | incremental)
        └─ CanvasBody
             ├─ TreeColumn  ← 分段控件在这里：文件 | 元素 | 契约
             ├─ 画布（ReactFlow）
             └─ ElementDrawer（右抽屉）
```

不选浮层 / 独立页的理由：架构页已有右抽屉放元素详情，再开 Sheet 会与抽屉抢同一纵向空间；
而「契约」与「文件 / 元素」同属**左栏视角**（都是"从哪个维度看这个仓库"），同构。

### 面板形态

```
┌ 左栏 · 契约 ────────────────────────────────────────────┐
│ 契约面 0.1.0 · 映射 sha256:1f8f0ec1        [检查] [生成] │
│                                                          │
│ ── 生成物差异（3） ───────────────────────────────────── │
│   缺失  .dev-team/architecture/api/INDEX.toml    [打开]  │
│   不符  Infra/App.Backend.Infra.ArchiRuntime.schema.json │
│   孤儿  crates/core/legacy.i.rs                  [打开]  │
│                                                          │
│ ── 判据（41） ────────────────────────────────────────── │
│  [entry 28] [contract 13] [signature 0] [reexport 0] …   │
│   · apiEntryMissing  App.Backend.Shell                   │
│   · apiMissing       App.Backend.Infra.Store（契约缺失） │
│   · apiSignatureMismatch  App.Auth → parse（两侧归一签名）│
└──────────────────────────────────────────────────────────┘
      点判据条目 → 画布聚焦该元素 + 右抽屉滚到该元素契约段
```

要点：
- **两轴分组天然并列**：上半是「生成物新鲜度」（可机器修），下半是「契约 vs 代码」（要人或
  agent 修）。两个分区各自有计数，MUST NOT 混成一个"问题总数"。
- **`category` 六值做筛选 chip**（entry / contract / signature / reexport / nesting / constructor），
  词汇已在报告里，前端零新词表；chip 计数来自同一份 `apiFindings`。
- **条目点击 = 聚焦**（复用既有 `onFocus` 语义：元素树 / 文件树都这么干），零新交互范式。
- **「打开」走 `FileViewer`**（契约 md / schema / 生成物都能开；`read_file` 已通用）。

### 右抽屉「契约」段（插在概览与文档之间）

```
概览（fqn / kind / gate / path / modelFile:line）
契约                                     ← 新增
  状态 declared
  契约  Infra/App.Backend.Infra.ArchiRuntime.md        [查看]
  schema Infra/App.Backend.Infra.ArchiRuntime.schema.json [查看]
  生成物 crates/infra/archi.i.rs                        [查看]
  判据  0 条 · 本元素无漂移        ← 有则逐条列出（kind + message）
文档 / 豁免 / 改档（既有）
```

数据来源零新增：`ArchiModuleDetail.api`（`ArchiApiRef`）**早就在返回里**；本元素判据从
`apiFindings` 按 `element === fqn` 过滤（父级已取过同一份报告，不重复 invoke）。

### 数据流与取数纪律

```
useArchiGraph(root)          → graph（既有）
useArchiDrift(root, mode)    → report.apiFindings（既有，架构页已取）→ 传给 CanvasBody
useArchiApiCheck(root)       → { enabled, meta, mismatched, orphaned, missing }（新）
                                触发：切到「契约」模式时一次 + 手动「检查」按钮；零轮询
archi_api_generate(root)     → { written, unchanged }（新，点按钮触发，成功后自动重跑 check）
```

- **不随画布 refresh 自动跑**：`useArchiApiCheck` 只在契约模式挂载时 invoke（模式未选 = 零 invoke），
  与既有 `useArchiFileTree`「切到文件模式才取数」同款。
- `root === null` → 全部不 invoke（既有约定）。
- 生成成功后重跑 check（否则面板显示的还是旧差异）——用 hook 自身的 `refresh`，不重挂整页。

### 状态矩阵（面板必须把每种态都画出来）

| 态 | 呈现 |
|---|---|
| `loading` | 「解析中…」（沿既有 loading 文案） |
| `enabled = false`（无 `api` 目录 / 零份 md） | 「契约面未启用」+ 冷启动说明（此时应生成集只含 `INDEX.toml`，missing 里就那一条） |
| 零差异 + 零判据 | 「生成物最新，判据 0 条」 |
| invoke reject（模型坏 / 契约资产非法） | 行内错误串（照 `archi-file-tree-error` 形态），图上仍可操作 |
| 生成失败（越根 / 无权限） | 错误回显在面板顶部，零写盘（写盘是逐份的，失败点之前的已写——需在文案里如实说） |

### 生成交互

```
[生成] → StandardDialog 二次确认
   「将写入 3 份（不符 2 + 缺失 1），并保留 1 份孤儿生成物不做处理」
   [取消] [写入]
        → archi_api_generate → toast「写入 3 份 / 未变 2 份」→ 自动重跑 check
```

三个要点：
- **必须二次确认**：写盘 = 工作树改动（git 状态会脏）。轻量 toast 不够。
- **孤儿如实说明**：生成不删文件（已核实），dialog 必须写"保留 N 份孤儿不做处理"，
  否则用户会以为点了生成就干净了。
- **写盘非原子**：逐份 `fs::write`，中途失败 = 部分写入。dialog / 错误文案不得声称"全或无"。

### 一处与既有纪律的冲突（裁决：U1 + D，spec 零反转）

现有纪律不是随手一句，是**带理由的硬 requirement**（`desktop-archi-canvas`「能力差异（全量只读 /
增量可改档）」），四处文本互相咬合：

| 位置 | 原文要点 |
|---|---|
| 能力差异表 | `full` 改档 ❌「只读视图，呈现『改档需要开一个 change』提示」/ `incremental` ✅ |
| 只读约束段 | 「只读约束的对象 = **画布的人类可提交入口**」…「MUST NOT 以『会话能写』为理由为画布新增任何写入口或写命令，亦 MUST NOT 以『画布只读』为理由禁止该会话写」 |
| 场景三条 | 「全量模式零写入口」「增量模式改档落 worktree」「画布只读与页内会话写权并存」 |
| `desktop-archi-model` | 「`archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径」 |

| | 形态 | 得 | 失 |
|---|---|---|---|
| **U1（裁决）** | 生成按钮仅在 `incremental`（change 详情）可点；`full` 只显示差异 + 「生成需开 change，或让左侧会话代劳」 | 与既有"写要开 change"纪律同构，**四处 spec 文本原样在案** | 架构页仍是观察窗 |
| U2 | 两处都可生成 | 顺手 | 开先例：架构页从只读视图变成会改工作树的入口 |
| U3 | U2 + 同批放宽 `archi_set_gate` | 口径一致 | 反转四处带论证的 spec 文本 + 三条场景 + 一个 testid |

**裁决：U1 + D**。架构页的写盘走**页内 agent 会话**——这正是「只读约束」段原本给出的路径
（会话在主仓根直改，画布保持只读）；D 面把这条路做得更好用一点，而不是给画布开写入口。

## D 面：`archi_api` 专用工具（随 U1 一并入范围）

### 一个先撞上的硬约束

`desktop-crate-layout` 明写：「**`archi-runtime` MUST NOT 被 `workflow` / `archi` /
`agent-runtime` / `checks-runtime` 依赖**」。P1 把写盘函数放进 `archi-runtime` ⇒ **agent-runtime
拿不到写盘面**。所以 D 面的工具只能是**只读档**：

```
archi_api(action = "check")                          → 两轴结构化结果（生成物三类差异 + 十条判据摘要）
archi_api(action = "bootstrap", element = "<FQN>")   → 契约草稿（只出 stdout，零写盘）
```

生成仍由会话经 `bash` 跑既有命令（`pnpm api:export` / `cargo run -p archi-api-gen generate`）——
这条路今天就通，且零依赖变更。**「生成按钮」留给画布（且只在 change 详情）**。

### 接入点（全部是既有范式，零新 exec 变体）

| 面 | 改动 |
|---|---|
| `sdk/mount.rs` 静态清单 | 10 项 → 11 项：新增 `ARCHI_API_TOOL`，`ToolTier::Read`，`ToolExec::Archi`（沿用既有 `ArchiContext` 路由，不新增变体） |
| `sdk/archi_tool.rs` | `ArchiContext::execute` 的 match 加一臂（check / bootstrap 两动作），输出 `serde_json` 后过 `tools::cap_result`（L1 上限单点） |
| `runner.rs::mint_archi_context` | 「名字清单含任一 archi 工具」的 present 判据加新名 |
| `compose.rs` | `archi` 来源工具清单 +1；`archi_matrix` 给 implement/executor 加列（executor 改完代码自查生成物新鲜度） |
| `archi-knowledge.md` | 「自检纪律」段补一条：可调 `archi_api` 查两轴；生成物过期用 `pnpm api:export` 重生成，MUST NOT 手改生成物 |
| 相位 prompt（仅 implement/executor） | 一行工具说明（沿既有"额外可用只读工具"写法） |

### 若坚持「会话内直接生成」的写工具

唯一合法路径是放宽 `agent-runtime → archi-runtime` 那条 MUST NOT（一行 spec + 新 `ToolExec` 变体 +
`Write` 档挂载）。代价：写档工具意味着写盘**绕过转录里可见的命令行**（只留一个工具调用），
而生成是幂等且既有命令就能完成的动作——**不推荐**，多一个写面不换来能力。

### 生成按钮（U1 口径）

- `incremental`：`[检查] [生成]` 双按钮；生成走二次确认（见上条）。
- `full`：只有 `[检查]`；原生成位置显示一行提示「生成需开 change（或让左侧会话执行
  `pnpm api:export`）」，形态沿 `archi-gate-readonly` 既有提示。

### 前端工程注意点

- **knip**：新 hook 必须被组件真实消费（不得只被测试引用）；`ArchiApiCheckView` 类型经
  `bindings.ts` 出线即可用。
- **测试**：面板用例沿用既有形态（`architecture-canvas.test.tsx` 里已有 `archiDrift` 桩），
  需新增 `archiApiCheck` / `archiApiGenerate` 两个桩位；生成链路要用「确认 → 写 → toast →
  重跑 check」全链断言。
- **`FileViewer` 路径口径**：`archi::display` 输出 repo 相对 POSIX 串，`read_file` 需接受同形态
  （P1 风险项 3）。

## 仍开着的裁决点（本轮不做，但会影响将来形态）

1. **范围**：A+B+D 已定；C（门禁步，把「生成物过期」从 static-check 里拆成独立步）本轮不做，
   但 B/D 的执行体应为 C 将来能直接复用的形态（check 判定纯函数，而不是命令内联）。
2. **`passed` 收紧**：将来某类判据改 error 档时，passed 判据要与 severity 同批改（独立变更）。
