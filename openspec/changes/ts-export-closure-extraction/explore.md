# 变更 C · TS 取数面沿 `export … from` 走（导出闭包）

workflow_type: requirement

> 主线：`openspec/explores/module-entry-declaration.md`。
> 本单元 = 把「元素导出集」从「入口单文件」扩成「入口的导出闭包」。
> 它是变更 B 能否成立的前提：barrel 全是 re-export，读不出来就等于前端契约一写就红。

## C.0 scope 与依赖

| 项 | 内容 |
|---|---|
| 前置依赖 | 变更 A（入口字段提供「元素根」用于同元素约束） |
| 被依赖 | 变更 B（barrel）、变更 D 的演练（前端契约成型度） |
| 独立可归档 | 是（判据落在 `extract` 面与既有契约语料上） |
| 边界 | 不新增 specifier 解析实现；不抽 `export default`；不跟进跨元素 re-export |

## C.1 硬阻塞：TS 取数面读不出 barrel（必须先修）

```text
extract_element  (api/extract/mod.rs:207)
  └─ TypeScript → fs::read_to_string(root.join(entry)) → ts::extract_file(entry, source)
        ts::extract_file 只认 Declaration::{Function,Class,Variable,TSTypeAlias,TSInterface,TSEnum}
        （api/extract/ts.rs:119-139）—— **零 ExportNamedDeclaration / ExportAllDeclaration 分支**
```

后果链：

1. barrel 的内容天然全是 `export { X } from './x'` / `export * from './y'`；
2. 取数器对此**零产出** → 元素代码导出集恒为空；
3. 于是任何前端契约只要声明一条 op/类型，`apiMissing`（warn）就恒报「代码导出集里没有实现（代码导出集：空）」；
4. `bootstrap`（草稿生成）同样经 `extract_element` → 前端草稿恒为空骨架，无人可用。

**修法只有一条路**：让 TS 取数沿 `export … from` 走（一跳或递归 + 环防），把目标文件的导出并入本元素导出集；顺带 `visibility` 需要新事实（re-export 出来的符号仍是 `External`）。否则「统一 i」+ 前端契约 = 一写就红，实际等于前端继续 `api = none`，白补 barrel。

这也解释了为什么 schema 侧的「透传」（`api-reexport` / `$ref` alias）在契约模型里已有位置、而代码侧的 re-export 取数缺席——两者是同一件事实的两半，前者做了、后者没做。

## C.2 取数面设计（原主线 §11）

### C.2.1 要解决的五个问题（缺一个就退化成近似）

| # | 问题 | 现状 | 需要的事实 |
|---|---|---|---|
| 1 | **精确名单** | — | `export { A, B } from './x'` 只能把 A/B 计入本元素公共面；把 x 的全部导出都拉进来会虚增（今天的 `apiUndeclared` severity=off 让虚增不报，但那是撞运气） |
| 2 | **通配** | — | `export * from './x'` = x 的全部导出面 |
| 3 | **环与深度** | — | `a → b → a`（前端 helpers 互引很常见）必须有 visited 集；深度上限只是第二道保险 |
| 4 | **同元素约束** | Rust 侧天然满足（`mod` 树只能指子模块） | TS 的 specifier 可指任意路径（含 `@/lib/**`）——MUST 要求解析结果落在本元素某条 `path` 前缀内，否则**跳过**（跨元素 re-export 不是本元素公共面） |
| 5 | **定义位置** | `ApiSymbol.line` 只有行号，无文件 | 跳转后行号属于另一个文件，`bootstrap` 的 `<!-- pub fn x @ <entry>:<line> -->` 会**指错文件** |

### C.2.2 算法草案（自顶向下的导出闭包）

```text
extract_module(root, entry, element_paths):
  surface(file, names: Option<Set<str>>, visited) -> Vec<ApiSymbol>:
      if visited.contains((file, names)) → 返回空          // 环防
      parsed = parse(file)                                  // oxc，与 extract_file 同一 AST 家族
      decls = extracted_exported_declarations(parsed)        // 只认 export <声明>（既有纪律不变）
      named_exports  = { name → specifier }                 // export { A, B } from '<spec>'
      star_exports   = [ specifier ]                        // export * from '<spec>'
      out = decls 过滤（names 为 None ⇒ 全部；否则只留 names 内的名字）
      for (name, spec) in named_exports where names 允许该名字:
          target = resolve(file, spec)                      // ← 见 11.3
          if target 不在 element_paths 前缀内: continue      // 同元素约束
          out += surface(target, Some({name}), visited)
      for spec in star_exports:
          target = resolve(file, spec)
          if target 不在 element_paths 前缀内: continue
          out += surface(target, None, visited)             // 全部导出面
      按 (name, kind) 去重（保留首次出现，声明序稳定）
  return surface(entry, None, ∅)
```

- 入口 = 唯一的 `None` 起点：**只有从入口可达的导出才进公共面**，这正是「入口 = 公共面」的机械定义；
- 去重键 `(name, kind)`：同一类型经两条路径到达（菱形）是常态；
- 符号 `visibility` 一律 `External`（可达即公开），与 Rust 侧 `pub use` 升格后的语义对齐。

### C.2.3 specifier 解析：复用 `scan/ts.rs` 的 `TsResolver`，不造第二份

```text
scan::ts::TsResolver::new(root)
  ├─ extract(file_rel, source) -> Vec<ImportFact>        ← 依赖边用
  └─ resolve(file_rel, specifier) -> Option<String>      ← **取数面正需要的那个**
        （tsconfig paths / 别名 / 扩展名 / index 文件 / 相对路径折叠）
```

`api/extract` 今天不依赖 `scan`（同 crate，兄弟模块）。取数面若要自己再写一份 specifier 解析，就与 `scan` 的「零第二份解析」纪律正面冲突——**这里应当新增一条 `api::extract → scan::ts` 的内部依赖**，并在 doc 里写明理由（与既有「`display` 跨 crate 单点」同族）。注意 `resolve` 需要 `&mut self`（缓存 tsconfig 编译），闭包里要带一个 resolver 实例。

### C.2.4 签名要扩：`extract_element` 必须知道元素根

```rust
// 今天：只够 Rust（mod 树自锚）
pub fn extract_element(root: &Path, entry: &str) -> Vec<ApiSymbol>
// 需要：至少要能判「目标是否落在本元素 path 内」
pub fn extract_element(root: &Path, element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>
```

`extract/mod.rs` 头部已经 `use crate::dto::ArchiElement`，所以元素本体随手可得。调用点只有两处产品侧（`api/mod.rs:335` 的 `element_symbols` 与 `api/mod.rs:547` 的 `bootstrap`），都拿得到元素；测试面（`extract/{mod,rust,ts}_test`、`check_test` 的构造夹具）要跟着改。

### C.2.5 `ApiSymbol` 加 `file`（否则 bootstrap 注释说谎）

`ApiSymbol` 是**进程内类型**（不在 `dto` / 不进 specta / 不进 bindings），所以加一个 `pub file: String` 的线面零成本；受影响的是三处构造点（`rust.rs` 的三处 push / `ts.rs` 的 push / 测试夹具）。收益：`bootstrap` 的 `@ <file>:<line>` 恢复真实，且将来 `apiSignatureMismatch` 的 occurrence 也能指向真正定义处（今天是契约文件行号）。

### C.2.6 已知近似与留白（要写进 spec，不许口头约定）

| 形态 | 处置 |
|---|---|
| `export { localA }`（无 `from`，导出本文件的非导出声明） | 本期留白（取数器只认 `export <声明>`）：需要「按名字取回本文件声明」的第二遍，与既有纪律冲突，登记为缺口 |
| `export * as ns from './x'` | 产出命名空间对象而非符号 → 跳过（不猜） |
| 跨元素 re-export（`export … from '@/lib/**'` 或另一个元素的文件） | 跳过（同元素约束），零 finding（取数器非权威，缺口由契约面判据显示） |
| `export default` | 既有纪律：不抽（对外名字由导入方决定）——barrel 里 MUST NOT 依赖 default 导出 |

### C.2.7 修好之后的现实预期（别高估）

把 12 个 barrel 的目标文件扫一遍，前端导出面的构成是：

- **组件函数**：`export function ChangeDetailView({ root }: {...})` → `returns: React.JSX.Element` → 归一后落 `ApiType::Unknown`；
- **类型 / 接口**：`RendererComponent`、`DrawerSelection`、`FlowGraph`… → 类型面可映射；
- **registry 分派函数**：`resolveRenderer(kind: string): RendererComponent` → 唯一「像 op」的东西。

也就是说：**前端契约的真实形态是「类型面 + 模块人口」，行为契约基本写不出来**（组件 props 是内联类型字面量，抽取器不产出字段）。这不否定修取数面——它是「入口唯一」能被验证的前提；但要把预期落成 spec 的一句话：「前端 op 面本期留白，`api-types` 面可用」。

---

## C.3 验收判据（机械可复算）

1. 精确名单：`components/agent/index.tsx` 的导出集恰 7 个名字——`AgentInputProps` / `AgentMessagesProps` / `isToolPart` **不出现**；
2. 通配：`export * from './x'` 与逐名 re-export 在闭包内等价（同一簇文件两种写法产出同集）；
3. 环：`a → b → a` 不 panic、不重复、结果稳定（两次调用逐字相等）；
4. 同元素约束：指向 `@/lib/**` 或另一元素的 re-export **不入集**；
5. `bootstrap` 对前端元素产出非空骨架，且 `@ file:line` 指向真实定义处；
6. 既有 Rust 取数面零变化（`extract_crate` 的语料逐字不动）。

## C.4 未决（本单元内）

- `export { localA }`（无 `from`）是否本期补齐（需要「按名字取回本文件声明」的第二遍）；
- 跨元素 re-export 是否要一条 finding 提示（本期建议静默，避免与 `apiRefMissing` 语义打架）；
- `TsResolver` 复用是否会与 `scan` 的 `unresolved` 记账冲突（需在实现时定：取数面复用 resolver 但不写 `unresolved`）。

---

## C.5 启动前置对账（A 已合入后 · 实测）

### C.5.1 A 的遗留事项：哪些是真的账、哪些本就在 C 的范围内

| # | 遗留项 | A 的出处 | 实盘状态（本次核） | 归属 |
|---|---|---|---|---|
| 1 | `models/04` 壳 `entry = .../src-tauri/src/lib.rs` | §六 后续变更表 / R-10 | `grep -c '^entry = ' models/*.toml` = **0**（`05` 只有两行 `api = "none"`） | 数据回填（新，极小的 change） |
| 2 | `models/05` 两工具 crate `entry = .../src/main.rs` | 同上 | 同上 | 同上（与 1 同一变更） |
| 3 | 基线 `apiFindings` 分桶 15/26 → **12/27**（总 39） | AC-4 降级段 | 当前仍是 15/26 态（模型数据未动） | 同上（与 1、2 同批，判据是同一份报告） |
| 4 | 主线 `module-entry-declaration.md` 状态表仍写「A … **未合入**」 | 主线文档维护 | 过期（A 已合入归档 `2026-11-16-module-entry-field-spec`） | 随 1–3 顺手改，或 C 的 change 提笔时改 |
| 5 | `ts_import_specifier` 真相对路径接线 | **R-7：变更 C 必须接线** | `paths.rs:452` 仍是旧形态（`../<name>.i`），doc 标「待接线」 | **C 自身范围**，不是遗留 |
| 6 | `owner.rs` 三候选首个**真实实例** | AC-5 尾句「本仓零实例，MUST NOT 记为已接线」 | 本仓 `.i.ts` = **0**，`INDEX.toml` 42 条里 `generated` 非空的只有 `ArchiRuntime` | C 或其后的首个前端契约试点 |

**关键分辨**：1–4 是「等宿主重建」的时序账；5–6 是 A 主动交给 C 的接线账。**只有 1–4 需要宿主版本条件**，5–6 不影响 C 能否启动。

### C.5.2 实测事实（A 在 master 上确实生效）

```text
api/paths.rs     entry_candidates:324 / entry_state:347      ✔
                 entry_form / mod_path_attr                  ✘ 零命中（已删）
                 generated_path(entry) → <入口目录>/<主干>.i.<ext>  ✔ 入口同级
infra/archi      src/lib.i.rs 在盘 / archi.i.rs 已删 / #[path = "lib.i.rs"]  ✔
api/INDEX.toml   ArchiRuntime.generated = …/infra/archi/src/lib.i.rs   ✔（42 条条目不变）
scan/owner.rs    StrippedCandidate::{Prefix,Stem} 三候选         ✔
models/*.toml    ^entry 零命中（15/26 基线态）                    ✔
发布时序         desktop-v0.6.44 → 5d069bd（A 之前）
                 desktop-v0.6.45 → 1faeb4b（A 归档 commit 之后）⇒ **0.6.45 是首个含 A 的发行版**
```

⇒ A 的**代码**前置对 C 已满足；A 的**数据**前置（1–3）与 C 无依赖。

### C.5.3 C 的硬阻塞与配套，逐条状态

| C 的前提 | 状态 |
|---|---|
| A 的落点（入口同级）与归属三候选 | ✔ 已在 master（C 产出 `.i.ts` 才有意义） |
| `entry_state` 单点给出入口（C 不必自造探测） | ✔ — C.2.4 的签名扩展可简化为「多带元素」而非「重算入口」 |
| `scan::ts::TsResolver::resolve(&mut self, …)` 可复用 | ✔ 且 **`resolve` 无 `unresolved` 副作用**（它只读缓存、只返回 `Option<String>`；`unresolved` 由依赖边抽取的返回值承载）⇒ **C.4 第三条可关闭** |
| `api → scan` 依赖方向 | ✔ 已有（`api/diff.rs` 用 `walk_source_files`、`paths.rs` 用 `is_source_file`）；`scan::ts` 是首次但方向不新 |
| `ts.rs` 认 `export … from` | ✘ **仍是硬阻塞**：`exported_declaration` 只有 `Declaration::{Function,Class,Variable,TSTypeAlias,TSInterface,TSEnum}` 六分支，模块 doc 明写「`export {…}` / `export * from` 是透传（本期不产出操作）」 |
| C.3 判据 1 的语料 | ✔ `components/agent/index.tsx` 实测 7 个导出（`AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`） |

### C.5.4 C.4 之外新增的三条待裁

1. **D52 留白接口**：`extract::extract(element, file, source)`（`mod.rs:199`）零产品调用者、退出条件是「首个按文件起草的 bootstrap」。C 走元素级递归，**不应触发它接线** ⇒ 建议 C 明写「保持留白」。
2. **源码断言会红**：`ts_test.rs:377–381` 用字面串钉住 `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {` 首行——C 增分支若动签名/结构，该断言须同批改，否则独立红。
3. **`.i.ts` 仍零实例 ⇒ R-7 接线无观测点**：`INDEX.toml` 42 条里只有 `ArchiRuntime` 一条 `generated` 非空，前端契约资产为零。C 落地后若不做试点，`ts_import_specifier` 的接线与 `owner` 第三候选都仍只有合成断言。是否需要 C 内做 `Components.Agent` 的 `api-types` 试点（`components/agent/index.i.ts`，正好落在「文件型 path 主干对齐」第三候选上，是 AC-5 的首个真实实例），是 C 起草时必须裁的一件事。

### C.5.5 顺序建议

```text
A（已合入 + 0.6.45 已发布）
   ├─ 探针（可选，1 小时级）：models/04 + 05 三处 entry + 基线 12/27 + 主线状态表
   │      价值：验证「新宿主能解析 entry」这件事真的成立（R-10 的唯一机械证据）
   ▼
C（本探索）── 依赖只有 A，不依赖探针
   ▼
B（12 个 barrel）
```

**结论：可以启动。** C 的前置（A 的代码面）已在 master 且已随 0.6.45 发行；A 的三处数据遗留与 C 零依赖。唯一需要用户确认的是**本机运行的桌面端是否已升到 ≥ 0.6.45**——若仍是 0.6.44 及更早，C 的 change 在宿主内测得的 archi 报告会按旧落点判定（不影响 `cargo test` / `api:check`，只影响漂移与 `apiFindings` 观测口径）。

---

## C.6 数据回填落地后的复核（第二轮）与一条**必须在 C 起草前裁掉**的硬冲突

### C.6.1 数据回填已兑现（实测）

```text
commit 71d318a  archive: module-entry-data-backfill
  models/04  entry = "./packages/desktop/src-tauri/src/lib.rs"        ✔
  models/05  entry = "./…/tools/export-bindings/src/main.rs"          ✔
  models/05  entry = "./…/tools/archi-api-gen/src/main.rs"            ✔
  reports/archi/20261116T1024554217206Z.json
     apiFindings = 39   = apiEntryMissing 12 + apiMissing 27          ✔（预测值逐条落）
     structuralErrors 0 / errors 0 / warnings 0                       ✔
     model 51 元素 / 113 关系                                          ✔
  package.json 0.6.47（远端 tag desktop-v0.6.47 → f179580）           ✔
```

⇒ **A 侧的全部账目清空**。C 的前置在 master 上全绿（`ts.rs` 的 `export … from` 仍是唯一硬阻塞，而那是 C 自己的工作）。

主线 `module-entry-declaration.md` 状态表仍有四处过期（A-遗留仍写「候选，待排期」、结论 5「三处 `entry` 仍空」等）——纯文档，随 C 提笔顺手改即可。

### C.6.2 硬冲突：同元素约束 ⊥ 判据 1 的语料

C.2.1 问题 4 与 C.2.2 草案把「同元素约束」写成 **目标文件必须落在本元素某条 `path` 前缀内**（与 `paths::within_path` 同口径）。但判据 1（C.3）指定的语料 `components/agent/index.tsx` 的元素 `App.Frontend.Components.Agent` 是：

```text
path = ["./packages/desktop/src/components/agent/index.tsx"]   ← 文件型、单值
其 7 个导出全部定义在兄弟文件：
  ./agent-input        → components/agent/agent-input.tsx
  ./agent-messages     → components/agent/agent-messages.tsx
  ./agent-session-view → components/agent/agent-session-view.tsx
  ./session-panel      → components/agent/session-panel.tsx
```

`within_path` 对**文件型** `path` 条目只认精确相等（`api/paths.rs:397`），目录分支只对非源文件条目生效 ⇒ **四个 re-export 目标全部越界 ⇒ 闭包恒为空 ⇒ 判据 1 恒失败**。这不是实现细节，是 C.2.2 与 C.3 互相矛盾。

12 个目录型根不受此影响（`Views.*` 的 `path` 是 `…/views/agent/` 形态，`normalize_root` 去尾斜杠后 `starts_with("{dir}/")` 成立 ✔），所以 C 若把判据 1 语料换成 B 落地后的 barrel 就不会撞；但 C 是 B 的前置（C.0），**C 内必然先跑在现态的 `Components.Agent` 上**。

### C.6.3 三种口径（须择一写进 C 的 spec）

| 口径 | 判据 | 对 `Components.Agent` | 代价 |
|---|---|---|---|
| ① 严格 `path` 前缀（C 原稿） | 同 `within_path` | 闭包恒空 ✘ | 判据 1 必须换语料（改合成夹具，等 B 落地才有真语料） |
| ② **owner 子树**（推荐） | 目标文件经 `scan::OwnerIndex::owner_of` 归属的 FQN **以本元素 FQN 为前缀**（本元素或其子孙） | `agent-input.tsx` → `…Agent.AgentInput`，前缀匹配 ✔ | `api` 层新增对 `scan::OwnerIndex` 的使用（`scan` 依赖已有先例）；须在 spec 写明「父元素的公共面 ⊇ 子元素可达面」是 barrel 语义，MUST NOT 与「虚增」混为一谈 |
| ③ 入口所在目录 | 目标落在 `<入口目录>/**` 内 | ✔ | 对 `App.Frontend.Shell`（4 个散落文件）会放开到 `src/` 整棵树 ⇒ 虚增面失控 ✘ |

② 的额外好处：它天然允许 `Views.Changes` 的 barrel 聚合 `flow/`、`renderers/` 两个子域（与目录型 `path` 前缀在结论上一致，无第二条规则分叉），也天然拒绝 `@/lib/**`（归属 lib 元素，不在子树内）。

② 需要一并确认的边界：**具名 re-export 仍走精确名单**，故 `AgentInputProps` / `AgentMessagesProps` / `isToolPart`（未被 barrel 导出）不入集——判据 1 不变；只有 `export *` 才取全量面（C.2.6 既有裁）。

### C.6.4 第二轮复核后 C 的启动判定

| 前置 | 状态 |
|---|---|
| A 代码面（落点 / 归属三候选 / `entry_state` 单点） | ✔ master |
| A 数据面（三处 `entry` + 12/27 基线） | ✔ 已归档 |
| 宿主版本 | ✔ 0.6.45 起含 `entry` 字段面，当前 0.6.47 |
| `TsResolver::resolve` 可复用且无 `unresolved` 副作用 | ✔（C.4 第三条可关闭） |
| `extract_element(root, entry)` 只差「带元素」 | ✔ 两处调用点（`api/mod.rs:420`、`:651`）都已持有 `element` |
| `ts.rs` 认 `export … from` | ✘ 待 C 实现（唯一实质工作） |
| 判据 1 语料与同元素约束 | ✘ **须先裁 C.6.3** |

⇒ **可以启动**，但 C 的 proposal 第一件事是裁 C.6.3 的口径；不裁则实现到判据 1 必卡。

### C.6.5 裁定：口径 ②「owner 子树」（用户选定）

```text
re-export 目标文件 reexport_ok(element, target):
    match OwnerIndex::build(elements).owner_of(target):
        Owner::Element { fqn, .. } => fqn == element.fqn || fqn.starts_with(&format!("{}.", element.fqn))
        Owner::Ambiguous(_)        => false        // MUST NOT 任选（与既有口径一致）
        Owner::None                => false        // 无元素认领 ⇒ 不是本元素公共面
```

- FQN 按 `.` 分段比较（`starts_with("{fqn}.")`），MUST NOT 裸 `starts_with(fqn)`——否则 `…Components.Agent` 会误认 `…Components.AgentFoo`；
- 与目录型 `path` 前缀口径**结论一致**（`Views.Changes` 的目录前缀本就放行 `flow/`、`renderers/`），故不产生第二条规则分叉；
- `@/lib/**`、`@/components/ui/**` 等归属其它元素的路径一律拒绝；
- 具名 re-export 仍走精确名单 ⇒ `AgentInputProps` / `AgentMessagesProps` / `isToolPart` 不入集（判据 1 的后半段不变）。

实现面：`extract_element` 需带 `element` + `elements`（构建一次 `OwnerIndex`），与 C.2.4 的签名扩展合并为一次改动；`api → scan` 依赖新增一条，与 `walk_source_files` / `is_source_file` 同族。

### C.6.6 ② 的实测副作用：判据 1 的期望值从 7 变成 **5**

`components/agent/` 下**没有**为 `session-panel.tsx` 建元素（7 个子元素覆盖 `index.tsx` / `agent-blocks.tsx` / `agent-error.ts` / `agent-input.tsx` / `agent-messages.tsx` / `agent-session-view.tsx` / `tool-cards.tsx`）。于是：

```text
export { AgentInput }             from './agent-input'         → 归属 …Agent.AgentInput          ✔ 入集
export type { AskAnswerActions }  from './agent-messages'      → 归属 …Agent.AgentMessages       ✔ 入集
export { AgentSessionView }       from './agent-session-view'  → 归属 …Agent.AgentSessionView    ✔ 入集
export type { AgentSessionActions, AgentSessionViewProps }     → 同上                           ✔ 入集（2 个）
export { SessionPanel }           from './session-panel'       → Owner::None                     ✘ 拒绝
export type { SessionPanelState } → 同上                                                          ✘ 拒绝
                                                              闭包 = 5 个名字（3 类型 + 2 组件函数）
```

两条处置：

| 处置 | 判据 1 期望 | 评价 |
|---|---|---|
| （i）**接受 5 + 具名登记缺口**（推荐） | 现态 **5**；B 的容器翻转（B.1.6 把 `Components.Agent` 的 `path` 改成目录）后 **7** | 缺口成因是**模型没给 `session-panel.tsx` 建元素**，不是取数器的近似；缺口以 `apiMissing` 可见（「契约写了 SessionPanel，代码导出集没有」会报），倒逼模型补齐——正是架构模型驱动设计的正确反馈。B.1.6 已在 B 范围内，7 必然到账 |
| （ii）② 再加一条「`Owner::None` 且落在入口同一目录 ⇒ 放行」 | 现态 **7** | 立刻对齐直觉，但给「模型漏建元素」开了静默口子，且 `apiUndeclared` 现为 `off` ⇒ 虚增无从暴露，与「入口 = 公共面」的机械性相悖 |

若取（i），C 的 spec 须同时写两段值（`5`（现态，附缺口成因）/ `7`（B 翻转后）），MUST NOT 只写 7 而不写语料条件——否则 C 落地即红。

### C.6.7 追查：`session-panel.tsx` 该不该建元素 —— **该，是数据漏了**

判据真值在 `.dev-team/architecture/decisions/0001-model-granularity.md` §2：

> 前端到组件：`components/agent/*.{ts,tsx}` 与 `components/standard/*.tsx` **每文件一元素**（排除测试与 barrel `index.tsx`）

`session-panel.tsx` 完全落在该判据内，`models/06` 却没给它建元素。顺手把前端覆盖度整体对拍了一遍（排除 `ui/` 收敛层、`hooks/`、`lib/`、`types/` 与测试）：

| 未覆盖文件 | ADR 判据 | 性质 | 牵连 |
|---|---|---|---|
| `components/agent/session-panel.tsx` | §2「agent/*.{ts,tsx} 每文件一元素」 | **漂移**（数据漏项） | 被 barrel 导出（`SessionPanel` / `SessionPanelState`）⇒ 正是 C 判据 1 少 2 个名字的成因 |
| `components/standard/tooltip.tsx` | §2「standard/*.tsx 每文件一元素」 | **漂移**（数据漏项） | 被 `Views.Changes`（`change-list-view.tsx`）与 `Views.Explores`（`explore-list-view.tsx`）import ⇒ 补元素同时补 2 条入边 |
| `components/file/file-viewer.tsx` | **ADR 无该档**（三层表只列 agent / standard / ui） | **判据缺口** | 被 `Views.Architecture` 的 `element-drawer.tsx` / `contract-panel.tsx` import ⇒ 补它必须先改 ADR（改能力），或登记为已知缺口 |
| `components/standard/hooks/use-open.ts` | §3 反例明写「hooks 目录不入模型」 | 合法排除 ✔ | — |

⇒ **`session-panel` 不是「不该建」，而是「当年漏建」**。ADR §8 的漂移纪律要求「新增共享组件时同一 change 内同步模型」，这一条被漏了；`unmatchedFiles` 只在**全量**扫描时列出（当前报告是 `incremental`，`scannedFiles 0`，所以一直没显形），于是静默了。

### C.6.8 结论：先补模型，再启动 C（② 因此不需要任何例外分支）

补 `session-panel` 元素后，`owner_of('components/agent/session-panel.tsx')` = `Owner::Element { fqn: "App.Frontend.Components.Agent.SessionPanel" }`，FQN 前缀命中 `Components.Agent` ⇒ **判据 1 在现态就是 7**，且 B 的容器翻转前后口径一致。于是：

- ② 保持纯粹（无 `Owner::None` 放行分支）；
- 不需要 C.6.6 的 (i)/(ii) 二选一——两个选项都是在给模型漂移打补丁；
- 顺带把 `tooltip` 一起补（同一条判据、同一个文件面），`file-viewer` 单独裁（要动 ADR = 改能力）。

**补模型的连锁（估算，须在 change 内逐条按判据复核）**：

```text
只补 session-panel + tooltip：
  models/06        28 元素 / 64 关系  →  30 / 70        （+2 元素 / +6 边）
     session-panel 出边：→ Components.Agent.AgentSessionView（值 import）
                        → App.Backend.Shell（经 bindings 映射，与 AgentSessionView 同口径）
                        → Components.Ui（ui 原语去重 1 条）
     barrel 出边     2 → 3（多一条重导出 SessionPanel；ADR §9 的「6 条端点边」→ 7）
     tooltip       入边：Views.Changes →、Views.Explores →（2 条）；出边：→ Components.Ui（1 条）
  全仓             51 / 113 → 53 / 119
  apiFindings      12 / 27（总 39）→ 12 / 29（总 41）   ← 两个新元素有入口、无契约 ⇒ 各 +1 apiMissing
  牵动 spec        repo-archi-model（计数与构成）/ desktop-archi-api-check（基线桶 12/29）/
                   desktop-archi-drift（若 file-viewer 一并纳入）/ ADR 0001 §2 §3 §9
```

⚠️ 特别注意：`00` 系列的数字（51 / 113、28 / 64、15+26、12+27、6 条 barrel 端点边）散落在 **4 份存活 spec + ADR + 模型头注**，改动面类似 `module-entry-data-backfill`（纯数据但要同步 spec 数字），不是「改一行 toml」。

**排期三选**：

| | 序列 | 代价 |
|---|---|---|
| X（推荐） | 微型 change「前端组件元素覆盖补齐」→ C → B | 多一个纯数据 change；C 全套判据在现态即可成立，② 无例外分支；两处漂移一次清 |
| Y | C（判据 1 = 5 + 缺口登记）→ B | C 落地即带一条技术债；`tooltip` 漂移继续静默 |
| Z | 只补 `session-panel` 一项（不碰 tooltip）→ C | 最小干预、判据 1 现态 7；但 `tooltip` 仍漏，且下次全量扫描会再撞到同一类问题；`06` 的 28/64 / 总数 51/113 / ADR §2 的「10 个共享组件文件」仍需改 |

### C.6.9 裁定：路线 X（先补 `session-panel` + `tooltip`，再启动 C）

#### 判据强度分层（为什么这两处是「补」而不是「加」）

| 证据 | `session-panel.tsx` | `tooltip.tsx` |
|---|---|---|
| ADR 0001 §2 粒度判据 | `components/agent/*.{ts,tsx}` 每文件一元素 ✘ 缺 | `components/standard/*.tsx` 每文件一元素 ✘ 缺 |
| 存活 spec 是否已在案 | ✔ `desktop-change-chat`：「共享组件 `SessionPanel`（`components/agent/session-panel.tsx`，经 `components/agent` 桶导出）」 | ✘ spec 零提及（只是 15 行的 `TextTooltip` wrapper） |
| 是否在公共面 | ✔ 被 barrel 导出（`SessionPanel` / `SessionPanelState`） | ✘ 仅被两个视图直接 import |

⇒ `session-panel` 是**双面漂移**（ADR 判据 + spec 都在案，唯模型缺）；`tooltip` 是**单面漂移**（判据在案，spec 未提）。两者都属「数据追上判据」，MUST NOT 被读成新增架构设计。

#### 微型 change 的产物清单（草案，供起草）

```text
models/06-frontend.toml
  [element."App.Frontend.Components.Agent.SessionPanel"]
  kind = "component"; gate = "off"
  path = ["./packages/desktop/src/components/agent/session-panel.tsx"]
  summary = "会话面板骨架（头区动作 / 状态行 / 历史清单 / 折叠窄条）"

  [element."App.Frontend.Components.Standard.Tooltip"]
  kind = "component"; gate = "off"
  path = ["./packages/desktop/src/components/standard/tooltip.tsx"]
  summary = "文本气泡（原语层 Tooltip 的业务封装）"

关系边（按 ADR §2「目标按最具体已声明元素归属」逐条取证；数字须在 change 内复核）
  SessionPanel → AgentSessionView     值 import（`./agent-session-view`）        +1 出
  SessionPanel → Components.Ui        `@/components/ui/*` 去重                  +1 出
  SessionPanel → App.Backend.Shell    `@/types/generated/bindings` 映射（ADR §7）+1 出
  Components.Agent（barrel）→ SessionPanel  重导出                              +1 出
  Standard.Tooltip → Components.Ui    `@/components/ui/tooltip`                  +1 出
  Views.Changes → Standard.Tooltip    `change-list-view.tsx`                     +1 入
  Views.Explores → Standard.Tooltip   `explore-list-view.tsx`                    +1 入
                                                                   合计 +7 条边

计数（三处同值：模型表头 / repo-archi-model 计数表 / ADR §9）
  06-frontend      28 / 64   →  30 / 71     （叶边 58→64，barrel 端点边 6→7）
  全仓             51 / 113  →  53 / 120
  前端恒等式        71 = 64（叶→叶 / 叶→容器）+ 7（barrel 端点边）
  apiFindings      12 / 27（总 39） → 12 / 29（总 41）
                   两个新元素是文件型根 ⇒ 有入口、无契约 ⇒ 各 +1 `apiMissing`
```

#### 牵动面（起草时须逐行核，别漏）

| 文件 | 要改什么 |
|---|---|
| `.dev-team/architecture/models/06-frontend.toml` | 2 元素 + 7 边 + 表头「28 元素 / 64 关系」与「共享组件文件 10」两个数 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | §2 反例行、§3（barrel 端点边 2 出 → 3 出）、§9 复算表；若顺带裁 `components/file/` 则 §2 需新增一档 |
| `openspec/specs/repo-archi-model/spec.md` | `:42`（表头同值纪律）、`:49`（51/113）、`:205`（`64 = 58 + 6` 恒等式 → `71 = 64 + 7`）、`:209`、`:301`、`:347`（06 的构成表）、`:351` |
| `openspec/specs/desktop-archi-api-check/spec.md` | `:171`（39 条 → 41）、`:187`（基线表 12 / 27 → 12 / 29）、`:188`（三段读数链末段）、`:189`（基线单段）、`:209`（scenario 逐条点名） |
| `openspec/specs/desktop-archi-model/spec.md` | 若含前端元素构成描述需同步（grep `28` / `64`） |
| 版本纪律 | `packages/desktop/package.json` +1 patch（纯数据变更是否 bump 按 AGENT.md「desktop source change」口径——本变更零源码，须在 change 内明写判据） |

#### 顺带处置 `components/file/file-viewer.tsx`

它不属「数据追上判据」（ADR 三层粒度表没有 `components/<其它目录>` 档），所以**本 change 不补**，而是在 ADR §8 的「已知缺口」段登记一条（与其他缺口同形），要补须另开 change 改能力（新增粒度档 + 连带 1 元素 / 4 边）。

#### 对 C 的净效果

```text
补完之后：owner_of("components/agent/session-panel.tsx") = Owner::Element{ …Agent.SessionPanel }
         FQN 前缀命中 Components.Agent ⇒ C 判据 1 现态 = 7 个名字
         C.6.3 的口径 ② 不需要任何 Owner::None 放行分支
         B 的容器翻转（path → 目录）之后仍 = 7，口径不分叉
```

---

## C.7 第三轮状态复核（微型 change 已归档后）

### C.7.1 实测兑现：预估逐项命中

```text
commit 57a6c470  archive: frontend-component-element-coverage（package.json 0.6.47 → 0.6.48）
   models/06            30 元素 / 71 关系   （= C.6.9 预估）
   全仓                 53 / 120            （= 预估；含 §4.2 ③ 那条 bindings type-only 边）
   apiFindings          41 = apiEntryMissing 12 + apiMissing 29   （= 预估）
   api/INDEX.toml       44 条               （42 + 2，与分母同值）
   structuralErrors 0 / errors 0 / warnings 0
   新增 2 条 apiMissing 逐条点名：…Agent.SessionPanel、…Standard.Tooltip
```

**R-1 已解**：`@/types/generated/bindings` 的 **type-only import 确实建边**——报告写明「`observations` 相对基线恰 +1 条，新增项逐字为 `App.Frontend.Components.Agent.SessionPanel → App.Backend.Shell`（`unfulfilledDeclaration`）」，指向壳的前端声明边 11 → 12。故边数 **120（不是 119）**，恒等式 `71 = 64 + 7` 成立。

**R-4 已裁**：纯数据变更**也 bump patch**（0.6.47 → 0.6.48），判据落在 change 内。

### C.7.2 事实校准：本文 C.6.9 / 独立 explore 的 AC-4 措辞需要修正

我原写「补元素后 `unmatchedFiles` 不再含这两个文件」——**错**。该 change 的报告（`reports/archi/model-sync-check.md` §1.1）给出的事实：

```text
scan/mod.rs 的 Owner::Element { gate: Off } 分支 ⇒ gate 全 off 的元素覆盖的文件
照常进 unmatchedFiles（只有非 off 元素才进 matched）
⇒ 加元素后两文件**仍在** unmatchedFiles 里，这是 off 语义，不是覆盖失败
⇒ 覆盖的正确机械证据是 archi_file_tree.owners：
     …/components/agent/session-panel.tsx → mark = "elementRoot", owners = ["…Agent.SessionPanel"]
     …/components/standard/tooltip.tsx    → mark = "elementRoot", owners = ["…Standard.Tooltip"]
```

对 C 的启示：**「归属」与「导出集」是两条不同的判据通道**——C 的 owner 子树口径吃的是 `OwnerIndex::owner_of`（静态、与 gate 无关），不会因 `gate = off` 失效；但任何以 `unmatchedFiles` / `matched` 当证据的验收措辞都必须先分清 gate 语义。

### C.7.3 C 的全部前置：现状清单

| # | 前置 | 状态 |
|---|---|---|
| 1 | A 代码面（落点入口同级 / 归属三候选 / `entry_state` 单点） | ✔ 归档 `2026-11-16-module-entry-field-spec` |
| 2 | A 数据面（三处 `entry`、基线 12/27） | ✔ 归档 `2026-11-16-module-entry-data-backfill` |
| 3 | 前端组件元素覆盖（判据 1 语料） | ✔ 归档 `2026-11-17-frontend-component-element-coverage`（0.6.48） |
| 4 | 判据 1 的 7 个名字现态成立 | ✔ `…Agent.SessionPanel` 元素在案，FQN 前缀落在 `Components.Agent` 子树 |
| 5 | `TsResolver::resolve(&mut self, …)` 可复用、无 `unresolved` 副作用 | ✔ `scan/ts.rs:156` |
| 6 | `extract_element(root, entry)` 只差「带元素 + 元素表」 | ✔ `api/extract/mod.rs:208` 仍是两参形态，两处调用点都持有 `element` |
| 7 | `ts.rs` 认 `export … from` | ✘ **仍是唯一硬阻塞**：`grep -c "ExportNamedDeclaration\|ExportAllDeclaration"` = **0** |

### C.7.4 仍未裁的三条（C 起草时必须落纸）

| # | 待裁 | 建议 |
|---|---|---|
| 1 | **同元素约束口径** | ② owner 子树（用户已选，C.6.5）；缺口（`Owner::None`）不再需要放行分支 |
| 2 | **D52 留白接口** `extract::extract(element, file, source)` | 保持留白（零产品调用者，退出条件是「按文件起草的 bootstrap」）；C 走元素级递归 MUST NOT 触发它 |
| 3 | **`ts.rs` 的三道闸**（详见 C.9） | 见 C.9：选方案 α（闭包在外层）则三道全绿；不要盲改断言 |
| 4 | **`.i.ts` 零实例 ⇒ 是否做试点** | `INDEX.toml` 44 条里 `generated` 非空仍只有 `ArchiRuntime`；建议做 `Components.Agent` 的 `api-types` 试点，它同时是 owner 第三候选（文件型根主干对齐）的首个真实实例 |

### C.7.5 文档账（不属 C 的实现面，随 C 提笔顺手改）

`openspec/explores/module-entry-declaration.md` 的状态表三处过期：

- `A` 行仍写「代码已落地，**三处 `entry` 数据未写**」（已回填）；
- `A-遗留` 行仍写「**候选，待排期**」（已归档 `2026-11-16-module-entry-data-backfill`），且仍是相对链接而非归档路径；
- 表内**无** `frontend-component-element-coverage` 条目（已归档 `2026-11-17-…`），而它现在是 C 的语料前置；
- 结论 5 仍写「三处 `entry` 数据仍空」。

### C.7.6 结论

**C 可以启动，且没有未清的前置。** 唯一实质工作是 `ts.rs` 的 `export … from` 闭包（C.2.2 草案 + C.6.5 的 owner 子树口径），其余全是已裁口径或文档同步。C 的 proposal 应先落三件事：口径 ② 的形式化定义（含 `Ambiguous` / `None` 双拒）、`extract_element` 的签名扩展（带 `element` + `elements`）、以及判据 1 = 7 个名字的逐名清单（`AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`，且 `AgentInputProps` / `AgentMessagesProps` / `isToolPart` 不出现）。

---

## C.8 遗留前清单与清理方案

### C.8.1 先划清除：**spec / ADR / 模型已全部同步，零遗留**

| 面 | 实测 | 结论 |
|---|---|---|
| `models/06-frontend.toml` | 表头 `30 元素 / 71 关系`、`共享组件文件 12` | ✔ 同步 |
| `models/*` 全仓 | `53` 元素 / `120` 关系 | ✔ 同步 |
| `repo-archi-model` spec | `:23` 53/120 + `71 = 64 + 7` + 12 共享组件；`:41` 旧值点名「仍写 51/113 即漂移」；`:63` / `:202` / `:206` / `:296` / `:341` / `:345` | ✔ 同步 |
| `desktop-archi-api-check` spec | `:185` 分母 44；`:186` **四段**读数链（28+13 → 15+26 → 12+27 → **12+29**）；`:187` 基线单段 12/29 | ✔ 同步 |
| ADR 0001 | `:9` 53/120；`:62` `file-viewer` 缺口登记；`:71–74` §9 复算表 | ✔ 同步 |
| `api/INDEX.toml` | 44 条（= `path` 非空元素数，与分母交叉校验） | ✔ 同步 |

⇒ **全部遗留都只在 `openspec/explores/` 的文档层**，零代码、零 spec、零数据。

### C.8.2 三层分级（关键：不是所有过期都要回改）

| 层 | 内容 | 处置 | 理由 |
|---|---|---|---|
| **L1 当前状态** | 主线 `module-entry-declaration.md` §7 状态表 | **直接改** | 状态表 = 现状；写「候选，待排期」而实际已归档，是纯误导 |
| **L2 结论** | 主线 §8 结论 5 / 6 | **直接改** | 结论是给读者的当前判断；基线已从三段变四段 |
| **L3 历史快照** | 主线 §0 行 2、§1 `entry_form` 伪代码、§2 `mod_path_attr`、§4/§5/§6 的旧符号引用 | **不回改，加顶部校准块** | 探索文档的价值在推理链（「当时为什么这么判」）；逐条回改会抹掉它，且 `api/paths.rs:242` 这类行号引用是 infinite 工程 |

### C.8.3 L1 精确改动点（主线 §7）

```text
:174  依赖图节点「A-遗留（数据回填，候选）：建议先于 B」
      → 「A-遗留（数据回填，已归档）→ 补建元素（已归档）→ C → B」（含新的补建元素节点）

:179  A 行「代码已落地，三处 entry 数据未写（05 已落两行 api = none）」
      → 「代码已落地，三处 entry 数据已由 A-遗留 回填」
      「三处 entry、基线 12/27（总 39）… → 候选 [module-entry-data-backfill](…)」
      → 「三处 entry、基线 12/27 → 已由 A-遗留 兑现」

:180  A-遗留 行 [module-entry-data-backfill](module-entry-data-backfill.md)（候选，待排期）
      → 已归档 `2026-11-16-module-entry-data-backfill`（删死链）
      「分桶 15/26 → 12/27（总 39）」→ 补「后续由补建元素换段为 12/29（总 41）」

新增行 | 补建元素 | 已归档 `2026-11-17-frontend-component-element-coverage`
       | session-panel.tsx / tooltip.tsx 两元素 + 7 边（模型追上 ADR 判据）
       | A-遗留 | 53/120、30/71、apiFindings 12/29；C 判据 1 语料到位
```

**死链事实**：主线里的 `[...](module-entry-data-backfill.md)` 指向 `openspec/explores/` 下**不存在**的文件（explore 文档随 change 开立被移入归档目录；`openspec/explores/frontend-component-element-coverage.md` 现已不存在）。故链接 MUST 改成归档目录路径，或直接写裸目录名（与 `D` 行「已归档 `2026-11-15-…`」同形）。

### C.8.4 L2 精确改动点（主线 §8）

```text
:192  结论 5「A 因此分两阶段：代码面随 A 归档落地，三处 entry 数据仍空 … 已由候选补位」
      → 「A 分两阶段（代码先行 / 数据回填），两阶段均已归档；该约束仍是后续任何新字段的通用纪律」

:193  结论 6「基线是三段而不是两段：28/13 → 15/26 → 12/27」
      → 「基线是四段：28/13（总 41）→ 15/26（总 41）→ 12/27（总 39）
         → 12/29（总 41，分母 42→44）；第四段成因 = 补建 session-panel / tooltip 两元素
         （有入口、无契约 ⇒ apiMissing +2）」
      真值锚点改为 `openspec/specs/desktop-archi-api-check/spec.md:186`（存活 spec 比归档报告更权威）
```

### C.8.5 L3 校准块（插在主线顶部导语之后，一次性声明）

```markdown
> **事实校准（截至 0.6.48，2026-11-17）**：§0–§6 是 2026-11-16 的现状快照与裁决推演，
> 其中出现的 `paths::entry_form` / `mod_path_attr` / `strip_candidates` 双候选 / 行号引用
> 均已随三次变更失效或改写：A（`2026-11-16-module-entry-field-spec`）删除前两者并把落点
> 移到入口同级；A-遗留（`2026-11-16-module-entry-data-backfill`）回填三处显式 `entry`；
> 补建元素（`2026-11-17-frontend-component-element-coverage`）把 `strip_candidates` 变为
> 三候选并补两元素 / 七边。**当前真值只看 §7 状态表与 §8 结论**；快照段保留原样以存推理链，
> MUST NOT 据此判断「代码里还有 `entry_form`」。
```

### C.8.6 顺手校准（B 的 explore，2 处）

| 位置 | 现值 | 问题 |
|---|---|---|
| `frontend-barrel-entry-migration.md:76` | 「前端 28 元素构成（4 容器 + 24 叶）→ 3 容器 + 25 叶（总数不变）」；「6 条端点边逐条不变」 | 现值 30 元素（4 容器 + 26 叶）⇒ 翻转后应为 **3 容器 + 27 叶**；端点边已 **7** 条 |
| `:177` | 「master 的 `paths::entry_form` 探测就会命中 `index.ts`」；「基线写死 28/13 → 15/26 → 12/27」 | `entry_form` 已删（改为 `entry_candidates` / `entry_state`）；基线已四段，末段 12/29 |

B 未开 change，改动风险极低；但若要把 B 的文档整体刷新，属 B 起草时的事，MUST NOT 在本轮顺手改掉 B 的验收判据。

### C.8.7 交接包：C.7.4 四条待裁不是「脏账」，是 C 的 proposal 内容

| # | 待裁 | 落点 |
|---|---|---|
| 1 | 同元素约束 = ② owner 子树（`Element` 前缀命中 / `Ambiguous` 拒 / `None` 拒） | proposal「口径」节 + delta spec |
| 2 | D52 留白接口 `extract::extract(element, file, source)` 保持不接线 | proposal「明确不做」表 |
| 3 | `ts_test.rs:377-381` 源码断言同批改 | tasks 的「表外同批适配面」 |
| 4 | `.i.ts` 零实例 ⇒ 是否做 `Components.Agent` 的 `api-types` 试点 | proposal「验收判据」+ 风险表 |

⇒ 这四条**不需要"清理"**，只需在 C 的 proposal 落纸即自动消解。真正的机械清理量 = 主线 L1（4 处）+ L2（2 处）+ 校准块（1 处）+ B 文档（2 处），共 **9 处文本**。

### C.8.8 清干净的机械判据（实测已全绿）

```bash
F=openspec/explores/module-entry-declaration.md

# ① 过期事实零命中（口径：不收宽泛的「候选，」，否则误命中 §4 的历史推演「两个候选，因为…」）
grep -n "候选，待排期\|数据未写\|数据仍空" $F                       # → 0 行   ✔ 实测 0

# ② §7 状态表的四个已归档项（A / A-遗留 / 补建元素 / D）
awk -F'|' '/^\| (A|A-遗留|补建元素|D) \|/ {print $2 $3}' $F         # → 4 行且各含「已归档」 ✔ 实测 4

# ③ 链接可达（explore 文档随 change 开立移入归档，explores/ 下会消失）
LINKS=$(grep -o '([a-z0-9-]*\.md)' $F | sed 's/[()]//g' | sort -u)
for f in $LINKS; do test -f "openspec/explores/$f" || echo "断链: $f"; done   # → 0 行 ✔ 实测 0（原 3 处 data-backfill 死链已随行改写消解）

# ④ 基线数字与存活 spec 同值（现值第四段）
grep -n "12 / 29\|四段" $F                                          # → 命中且写明四段 ✔

# ⑤ 校准块在场（L3 快照防误读）
grep -c "事实校准（截至" $F                                          # → 1 ✔
```

**执行记录（2026-11-17）**：① 0 / ② 4 / ③ 0 / ④ 命中 / ⑤ 1，五项全绿。改动落点——

| 文件 | 改动 |
|---|---|
| `openspec/explores/module-entry-declaration.md` | 顶部校准块（1 段）；§7 依赖图重写（4 已归档节点串成 A → A-遗留 → 补建元素 → C → B）；§7 表 A / A-遗留 两行改写 + 新增「补建元素」行 + C 行依赖补「补建元素」；§8 结论 5 / 6 改写（三段 → 四段，真值锚点指向存活 spec） |
| `openspec/explores/frontend-barrel-entry-migration.md` | `:76` 元素构成 28 → 30、端点边 6 → 7（附「现值更正」引注）；`:177` `entry_form` → `entry_candidates` / `entry_state`、基线三段 → 四段（附引注，副作用性质不变） |

---

## C.9 展开：`ts.rs` 的三道闸（C.7.4 第 3 条的实际内容）

C.7.4 第 3 条原写「`ts_test.rs:377-381` 的源码断言」——实测**不止一条，而是三道**，且最硬的不是签名断言：

| # | 闸 | 位置 | 类型 | 触发条件 |
|---|---|---|---|---|
| 1 | **签名逐字** | `ts_test.rs:377-381` | 源码断言 | `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {` 首行逐字相等 ⇒ **只有改签名才红** |
| 2 | **`Statement::` 恰 1 处** | `ts_test.rs:668-680` | 源码断言 | `ts.rs` 内 `source.matches("Statement::").count() == 1`，且必为 `Statement::ExportDeclaration` ⇒ **新开一处 `match statement` 就红** |
| 3 | **透传零符号** | `ts_test.rs:182-194` `extract_file的透传不产符号` | **行为断言** | `export { a, b } from './x'` / `export * from './y'` / `export type { T } from './z'` 三种源文 ⇒ `extract_file` 必须返回**空** |

### C.9.1 闸 3 是架构分水岭，不是绊脚石

它其实是**守门员**，把 C 的实现形态逼向唯一正确的落点：

| 方案 | `extract_file` 语义 | 闸 1 | 闸 2 | 闸 3 | 边界纪律 |
|---|---|---|---|---|---|
| **α（正确）** 闭包在外层：`extract_element` 递归 + `scan::ts::TsResolver`，`ts.rs` 只**多产出一份「re-export 名单」**（不并入 `extract_file` 的符号集） | 单文件抽取面不变（仍「只认 `export <声明>`」） | ✔ 绿 | 需按 C.9.2 处理 | ✔ 绿 | `ts.rs` 保持「纯读源文本 + AST，零读盘」 |
| **β（错）** 让 `extract_file` 自己认 `export … from` | 单文件抽取器必须读别的文件 | 可能不动 | 可能红 | ✘ **必红** | `ts.rs` 破「零读盘」边界，且递归/环防挤进纯函数 |

β 不是「改断言就行」——`ts.rs` 模块 doc 明写边界「**纯读源文本 + AST，零读盘、零写盘、零 spawn**」，而 re-export 闭包必须读盘 + resolve + 环防，天然属于 `extract_element` 那一层。闸 3 的存在正是把这条边界变成机械可验的。

### C.9.2 闸 2 的隐藏设计约束（最容易踩）

`ts.rs:66-71` 今天只有一处 `Statement::` 匹配：

```rust
fn exported_declaration<'a, 'b>(statement: &'b Statement<'a>) -> Option<&'b Declaration<'a>> {
    match statement {
        Statement::ExportDeclaration(export) => Some(&export.declaration),
        _ => None,
    }
}
```

C 要拿到 re-export 名单，**直觉写法是在新函数里再开一个 `match statement { Statement::ExportNamedDeclaration … }`**——那会让 `Statement::` 计数变 3+，闸 2 立刻红。

正确写法是**把这一处扩成三态枚举**，仍保持「`Statement::` 恰 1 处」：

```rust
enum ExportedItem<'a, 'b> {
    Declaration(&'b Declaration<'a>),          // export function / class / const / type / interface / enum
    Named(Vec<(String, String)>),              // export { A, B } from './x'  → (本地名, specifier)
    Star(String),                              // export * from './y'
    None,
}
```

⇒ **闸 2 事实上在要求 C 做「导出形态的单一收口」**，这与仓库既有的「零第二份」纪律同族。C 的 proposal 应把这条写成显式设计决策，而不是「撞了再改断言」。

### C.9.3 处置结论

| # | 闸 | 处置 |
|---|---|---|
| 1 | 签名 | **不改**。闭包在外层 ⇒ `extract_file` 签名一字不动，断言零改动 |
| 2 | `Statement::` 恰 1 处 | **扩同一处为三态**，仍 1 处 ⇒ 断言零改动；若实现者坚持新开 match，则该断言必须同批改成「恰 1 处 `match statement` 分派点」之类的新不变量，MUST NOT 直接删 |
| 3 | 透传零符号 | **保留**。它守的正是 α 方案的边界；若哪天 C 之后的变更让 `extract_file` 认透传，这条断言是唯一会立刻报警的地方 |

⇒ C.7.4 第 3 条**不需要「裁」**，需要的是在 proposal 里落一句实现形态约束：**闭包在 `extract_element` 层，`ts.rs` 只扩「导出形态」的单点分派，`extract_file` 的签名与产出语义零变化，三条闸全绿**。
