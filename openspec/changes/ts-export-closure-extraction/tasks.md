# 任务: ts-export-closure-extraction

> 覆盖 proposal「变更范围 - 实现文件」**5 行全量**（`api/extract/{mod,ts,rust}.rs`、`api/mod.rs`、`packages/desktop/package.json`）+ design「新增文件」的一份留痕（`reports/api/closure-evidence.md`）。**测试文件面归测试相位**（五份 `*_test.rs` 的夹具补 `file`、`extract_element` 调用点适配、闭包语料、闸 2 改写）——本相位不写用例、不跑测试门禁；逐项移交清单见文末。
>
> 依赖序：**阶段零**（改动前基线）必须先于任何源码改动——`bootstrap` 的「改动前零符号」一旦被代码覆盖就永久失去对照面。**阶段一**（`ApiSymbol.file` 端到端）解锁阶段二 / 四（同一个 `file` 事实的两端）。**阶段二**（`ts.rs` 三面 + 三态单点）解锁阶段三（闭包消费 `ExportSurface`）。**阶段三**（闭包 + 签名）与**阶段四**（注记）合起来才让 AC-1 / AC-5 可实测。**阶段五**收官（版本 + 证据留痕 + 零扩散核对）。
>
> 四条不可动摇的机械锚点：① `api/extract/ts.rs` 内 `match statement` 恰 **1** 处、`Statement::` 恰 **3** 处（等于臂数）、`Statement::ExportNamedDeclaration` 与 `Statement::ExportDefaultDeclaration` **零命中**——**doc 注释里 MUST NOT 写出这三类字面量**（注释会被同一份文本断言计入）；② `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {` 的首行逐字不变，且对 `export { a, b } from` / `export * from` / `export type { T } from` / `export default` 仍零符号；③ `extract(element, file, source)`（D52）产品侧零调用者、`paths::ts_import_specifier` 未接线——出现调用即误动；④ `scan/**` / `api/{check,paths,diff,schema,contract,emit}/**` / `.dev-team/architecture/**` / `packages/desktop/src/types/generated/**` / `openspec/specs/**` **零 diff**。
>
> 自查 cwd：**仓库根**（`git` 面 / `openspec` / 留痕）与 `packages/desktop`（`package.json` / `pnpm` 脚本）；`archi-api-gen` 命令按 proposal 的形式在仓库根执行（根锚由 `repository_root()` 向上找 `archi_dir`）。静态门禁（`server:check` / `bindings:export` / `client:check`）与 `apiFindings` 分桶实测由桌面端 static-check / test-execution 承接，本相位不重复执行机械步骤（design D9）。

## 阶段零：改动前基线（必须先于任何源码改动）

- [x] 采集 `cargo run -q --manifest-path packages/desktop/src-tauri/Cargo.toml -p archi-api-gen -- bootstrap App.Frontend.Components.Agent` 的 **stdout 原文**（元素级口径：单元素 `bootstrap`，不是全量检查），预期为「零符号骨架」——H1 + 空散文段 + 零 `###` 操作段 + `"api-types": {}`；原文落 `reports/api/closure-evidence.md` §1「改动前」
- [x] 在同一份留痕 §0 写下口径声明：① 条目用**元素级**口径（`archi-api-gen bootstrap <FQN>`，零契约资产即可复算）；② `apiFindings` 用**全量检查**口径（分母 = `path` 非空元素 44）；③ 两者读数不可互相换算
- [x] 记录改动前 `pnpm -C packages/desktop api:check` 的读数（预期零输出、退出 0，三列 `missing` / `mismatched` / `orphaned` 皆空）作为阶段五的对照基线
- [x] 记录改动前 `grep -n '"version"' packages/desktop/package.json` = `0.6.48` 与 `git status --porcelain` 的改动集（应只有本 change 目录）作为零扩散基线

## 阶段一：`ApiSymbol.file`（定义文件事实端到端）

- [x] 修改 `crates/core/archi/src/api/extract/mod.rs`：`ApiSymbol` 在 `line` 之后新增 `pub file: String`，doc 写明「定义所在文件，repo 相对 POSIX；TS 取抽取面形参，Rust 取所在模块文件（内联 `mod` 与父同文件）；MUST NOT 填入口串」
- [x] 修改 `crates/core/archi/src/api/extract/ts.rs`：`collect_declaration` / `extract_const` / `push_function` / `push_type` 各增 `file: &str` 形参，四个 `ApiSymbol` 构造点（`push_function` / `push_type` 各一）把 `file` 落到符号上；`extract_file` 把入参 `file` 传下去（未用到的形参不留 `_` 占位说明）
- [x] 修改 `crates/core/archi/src/api/extract/rust.rs`：`walk_items` / `push_type` / `function_symbol` 各增 `file: &str` 形参并落 `ApiSymbol.file`；`extract_file` 传自己的 `file`（`let _ = (file, crate_name)` 改为 `let _ = crate_name`）；`extract_crate` 以 `entry` 作根文件；`walk_modules` 对文件型 `mod` 传 `&file`、对内联 `mod` 沿用父文件（**不变**：抽取面 / `visibility` 四值 / 签名归一 / `#[path]` 限根三步）
- [x] 修改 `rust.rs` 模块 doc：补一句 `file` 语义（子模块符号指向子模块文件、内联 `mod` 与父同文件），并保持「边界」段（读入口与 `mod` 定位文件、零写盘、零 spawn）逐字不变
- [x] 自查 阶段一（构造点齐备）：`grep -n "ApiSymbol {" crates/core/archi/src/api/extract/*.rs` ⇒ 产品侧恰 **4** 处（`ts::push_function` / `ts::push_type` / `rust::push_type` / `rust::function_symbol`），四处都带 `file:`；`grep -rn "file:" crates/core/archi/src/api/extract/*.rs` 与四处逐一对上

## 阶段二：`ts.rs` 导出形态单一分派点扩成三态 + `extract_surface`

- [x] 在 `ts.rs` 新增类型：`pub struct NamedExport { pub local: String, pub exported: String, pub specifier: String }`、`pub struct ExportSurface { pub symbols: Vec<ApiSymbol>, pub named: Vec<NamedExport>, pub star: Vec<String> }`（各 `#[derive(Debug, Clone)]`，每个 pub 字段带 doc——沿本 crate 既有风格），以及**私有** `enum ExportedItem<'a, 'b> { Declaration(&'b Declaration<'a>), Named(Vec<NamedExport>), Star(String), None }`（四变体封闭，MUST NOT 扩第五态）
- [x] 把 `exported_declaration` 改写为 `fn exported_item<'a, 'b>(statement: &'b Statement<'a>) -> ExportedItem<'a, 'b>`：**三臂**——`Statement::ExportDeclaration(export)` ⇒ `Declaration(&export.declaration)`；`Statement::ExportFromDeclaration(export)` ⇒ `Named(具名名单)`；`Statement::ExportAllDeclaration(export)` ⇒ `export.exported.is_some()` 时 `None`（`export * as ns` 留白）否则 `Star(export.source.value.to_string())`；尾部 `_ => ExportedItem::None`（覆盖 `export { localA }` 无 `from`、`export default`、其余语句）
- [x] 同文件新增 `pub fn extract_surface(file: &str, source: &str) -> ExportSurface`：一次 `parsed.program.body` 遍历，按 `exported_item` 的三态分别推入 `symbols`（经 `collect_declaration`）/ `named` / `star`；`parsed.panicked` ⇒ 三面全空（不 panic、不猜）；具名名单的 `local` / `exported` 从 `ExportSpecifier` 取（`ModuleExportName` 的标识符两形态取名字，字符串字面量形态跳过该条——名字不是合法契约标识符）
- [x] 把 `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol>` 的体改为 `extract_surface(file, source).symbols`，**首行签名逐字不变**（`pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {`）；doc 保留「只认 `export <声明>`、re-export / `export default` 零符号」的语义说明
- [x] 改写 `ts.rs` 模块 doc：把「`export {…}` / `export * from` 是透传（本期不产出操作）」改为「透传由 `extract_surface` 的 `named` / `star` 两面交给闭包（闭包在 `extract/mod.rs`，本文件零读盘），`extract_file` 对它们仍零符号」；登记四类留白（`export { localA }` 无 `from` / `export * as ns` / 跨元素与仓外 re-export / `export default`）；**MUST NOT** 在 doc 里写出 `Statement::ExportNamedDeclaration`、`Statement::ExportDefaultDeclaration` 或第三处 `Statement::`（注释会被断言文本计入）
- [x] 自查 阶段二（单点与留白锚点）：`grep -c "match statement" api/extract/ts.rs` ⇒ **1**；`grep -c "Statement::" api/extract/ts.rs` ⇒ **3**；`grep -c "Statement::ExportNamedDeclaration\|Statement::ExportDefaultDeclaration"` ⇒ **0**；`grep -n "ArrowFunctionExpression"` ⇒ **0**；`grep -n "pub fn extract_file"` 的首行逐字等于 `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {`（cwd = `packages/desktop/src-tauri`）

## 阶段三：`extract/mod.rs` 闭包（TS 分支由入口单文件扩为入口导出闭包）

- [x] 修改 `pub fn extract_element` 签名为 `pub fn extract_element(root: &Path, elements: &[ArchiElement], element: &ArchiElement, entry: &str) -> Vec<ApiSymbol>`；Rust 分支 `rust::extract_crate(root, entry)` **逐字不变**，`Language::Unknown` 分支逐字不变，TS 分支改为 `Closure::new(root, elements, element).collect(entry, None)`
- [x] 新增**私有** `struct Closure<'a> { root: &'a Path, owners: &'a OwnerIndex, element: &'a ArchiElement, resolver: TsResolver, visited: BTreeSet<(String, Option<Vec<String>>)> }` 与 `fn collect(&mut self, file: &str, names: Option<&[String]>) -> Vec<ApiSymbol>`，严格按 design「闭包数据流」的 8 步：visited 键（**名字集排序后**）先插后判 → 读盘失败返回空 → `ts::extract_surface(file, &source)` → 声明面按 `names` 过滤 → 具名面（`names` 允许 `exported` 才处理）→ 通配面（取全量面）→ `(name, kind)` 去重保留首次
- [x] 新增**私有**辅助：`fn reexport_ok(owners: &OwnerIndex, element: &ArchiElement, target: &str) -> bool`（`Owner::Element{fqn}` 且 `fqn == element.fqn` 或以 `format!("{}.", element.fqn)` 为前缀；`Ambiguous` / `None` ⇒ `false`）、`fn export_target(resolver: &mut TsResolver, file: &str, specifier: &str) -> Option<String>`（`resolve` 后按 `paths::language_of(target) == Language::TypeScript` 过滤，其余 `None`）、`fn visited_key`、`fn dedupe`、`fn keeps`（`names` 判据）
- [x] 别名改名落在具名面递归之后（`collect(target, Some(&[local]))` 的每个符号 `name := exported`），`file` / `line` / `kind` 不改——`ApiSymbol` 的「定义在哪」与「对外叫什么」两件事因此可以同时为真
- [x] 修改 `extract/mod.rs` 模块 doc「边界」句为「`extract` / `ts` / 归一纯函数零读盘；`extract_element`（与 `rust::extract_crate`）按入口与闭包内目标文件读盘，零写盘、零 spawn」，并在「两个取数入口」段补一句「TS 分支已走入口导出闭包（沿 `export … from` / `export * from` 递归），Rust 分支沿 `mod` 树」；`extract`（D52）与 `apply_knip_report` 的留白段落逐字保留
- [x] 修改 `crates/core/archi/src/api/mod.rs` 两处产品调用点补参数：`element_symbols`（`load_contracts` 内，`elements` / `element` 本已在手）与 `bootstrap`（同）——`extract::extract_element(root, elements, element, &entry)`
- [x] 自查 阶段三（零第二份实现）：`grep -rn "oxc_resolver\|Resolver::new\|TsconfigOptions" crates/core/archi/src/api` ⇒ **0**；`grep -rn "fn owner_of\|prefix_matches" crates/core/archi/src/api` ⇒ **0**；`grep -rn "unresolved" crates/core/archi/src/api` ⇒ 仅既有注释（本变更 MUST NOT 新增写入点）
- [x] 自查 阶段三（算法锚点）：`grep -n 'format!("{}."' crates/core/archi/src/api/extract/mod.rs` 恰 1 处（`reexport_ok` 的分段前缀，**不是**裸 `starts_with(element.fqn)`）；`grep -n "TsResolver::new" crates/core/archi/src/api/extract/mod.rs` ⇒ `Closure::new` 内恰 1 处（每元素一次）；`grep -n "BTreeSet<(String, Option<Vec<String>>)>"` 恰 1 处（`visited` 只此一份）

## 阶段四：`bootstrap` 注记改指向符号定义文件

- [x] 修改 `api/mod.rs` 的 `fn bootstrap_text(fqn: &str, symbols: &[ApiSymbol]) -> String`（**`entry` 形参退出**），注记行改为 `<!-- {visibility} @ {symbol.file}:{line}{async 后缀} -->`；`bootstrap` 内的调用同批去掉 `&entry`
- [x] 修改 `bootstrap_types`：`待剪枝` 注记里的字面量「入口」换成 `symbol.file`（形态保持 `待剪枝：{kind} @ {file}:{line} {visibility}`）
- [x] 修改三处 doc（`bootstrap` 的「每个符号附 visibility 与定义位置注释」、`bootstrap_text`、`bootstrap_types`）为「定义文件与行」，并说明 `entry` 已退出注记面（闭包一跳后行号属另一个文件）
- [x] 自查 阶段四（注记真值源）：`grep -n "@ {entry}\|@ 入口" crates/core/archi/src/api/mod.rs` ⇒ **0**；`grep -n "symbol.file" crates/core/archi/src/api/mod.rs` ⇒ op 注记与类型注记各 1 处（恰 2 处写出点）
- [x] 自查 阶段四（Rust 侧不再谎报）：对一个**有子模块**的 Rust 元素跑 `cargo run -q --manifest-path packages/desktop/src-tauri/Cargo.toml -p archi-api-gen -- bootstrap App.Backend.Infra.ArchiRuntime`，注记 `@` 后的路径 MUST 是该符号所在模块文件（不再是入口串）；原文落留痕 §1 附注

## 阶段五：版本纪律 + 证据留痕 + 零扩散收官

- [x] 修改 `packages/desktop/package.json`：`"version"` `0.6.48` → `0.6.49`（只 +1 一次、不追溯）；其余键与 `scripts` 段零改动
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍是 `"version": "../package.json"` 引用形态（**零内容改动**，`git diff` 为空）；`packages/desktop/src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变
- [x] 采集改动后 `archi-api-gen bootstrap App.Frontend.Components.Agent` 的 stdout 原文，落留痕 §1「改动后」，逐项核对：符号名集合恰 7 名（`AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState`，3 `Function` + 4 `Type`）；`AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中；注记 `@` 后逐符号指向 `agent-input.tsx:152` / `agent-messages.tsx:33` / `agent-session-view.tsx:{19,30,122}` / `session-panel.tsx:{12,230}`，**零** `@ packages/desktop/src/components/agent/index.tsx:<line>` 形态
- [x] 若改动后仍为零符号：按 design AC-1 行的排查序查（先 `entry_state` 是否 `Resolved`，再 `OwnerIndex::owner_of` 是否对四个子元素命中 `Owner::Element`），**MUST NOT** 放宽同元素约束或改成裸前缀比较凑数——不达即停手在留痕登记，不改判据（条件未触发：改动后 7 名已在案，零排查动作、零约束放宽）
- [x] 留痕 §3 采集 `pnpm -C packages/desktop api:check`（三列皆空、退出 0）与 `pnpm -C packages/desktop api:export` 二跑（预期「0 份写入」、`git diff .dev-team/architecture/api` 零输出），原文附上
- [x] 留痕 §2 写 `apiFindings` 分桶：期望 `apiEntryMissing` 12 / `apiMissing` 29 / 总 41 / 分母 44 / `status = clean` / `passed = true`，并注明**全量检查口径**与读数来源（门禁 runner 落的报告）；无报告时写「待 test-execution 回填」，**MUST NOT** 手写伪读数、MUST NOT 用元素级 `bootstrap` 读数顶替
- [x] 零扩散核对（AC-8 白名单）：`git status --porcelain` ⊆ {本 change 目录、`crates/core/archi/src/api/extract/{mod,ts,rust}.rs`、`crates/core/archi/src/api/mod.rs`、五份 `*_test.rs`（测试相位）、`packages/desktop/package.json`}
- [x] 零 diff 核对（「不要修改」面逐项）：`git diff --stat .dev-team/architecture packages/desktop/src/types/generated openspec/specs packages/desktop/src-tauri/crates/core/archi/src/scan packages/desktop/src-tauri/crates/core/archi/src/api/check.rs packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs packages/desktop/src-tauri/crates/core/archi/src/api/emit` **零输出**
- [x] 留白终检（D7 / 锚点③）：`grep -rn "extract::extract(" crates/core/archi/src crates/infra crates/tools | grep -v _test.rs` 零命中（D52 仍零产品调用者）；`grep -rn "ts_import_specifier" crates/core/archi/src/api/extract crates/core/archi/src/api/mod.rs` **零命中**（既有调用点只在 `api/emit/ts.rs:157` 与 `paths.rs` 定义处，本变更 MUST NOT 新增）；`grep -rn "\.i\.ts" crates/core/archi/src/api/extract` 零命中（无 `.i.ts` 试点）
- [x] 源码不变量终检（锚点①②，供测试相位对表）：`grep -c "match statement"` = 1、`grep -c "Statement::"` = 3、两类留白字面量 = 0、`extract_file` 首行签名逐字、`extract` 与 `apply_knip_report` 的留白段落仍在

**移交清单（不勾为任务；承接方＝测试相位 / 桌面端 static-check · test-execution）**

> 实现面不产出、不勾选：缺项是「移交未完成」，不是「实现未完成」。红线——MUST NOT 用删除断言换绿，MUST NOT 手写伪读数。
>
> **静态门禁反馈修复（第 1 次）**：`server:check` 跑 `cargo clippy --workspace --all-targets`，编译面含 `#[cfg(test)]`
> sidecar ⇒ T1 / T2 与闸 2、以及 `mod_test::未知语言落空集恰两处且无猜测回落` 的 `paths::language_of(` 计数（2 → 「
> `match paths::language_of(` == 2 + `paths::language_of(` == 3」，登记新增的 `export_target` 过滤不是分派）已在本相位
> 完成：只做**形状适配 + R-1 预定形态改写**，零新增用例、零删除断言。逐项留痕见
> `reports/api/closure-evidence.md` §5。

- **T1 夹具补 `file`**（✅ 已随静态门禁反馈修复完成：`--all-targets` 编译面要求 sidecar 立即可编译）：`crates/core/archi/src/api/extract/mod_test.rs` 的 `fn symbol(...)`、`crates/core/archi/src/api/check_test.rs` 的 `fn function(...)` / `fn type_symbol(...)` 三处 `ApiSymbol` 字面量各补 `file: ...`（`check_test` 的断言语义零变化）
- **T2 `extract_element` 调用点适配**（✅ 已随同一次反馈修复完成）：`api/extract/mod_test.rs` 内全部调用点补前两个实参（`elements` / `element`）：`extract_element整元素导出集含子模块op`（`:502`）、`extract_element的ts走单文件`（`:535` / `:544`）、`extract_element入口读不到返回空集`（`:563`）、`extract_element语言未知返回空集`（`:578`）、`extract_element与bootstrap同源`（`:604`）、`extract` 与 `extract_element` 同源一条（`:1436`）；`api/mod_test.rs` 的 `bootstrap` 系列随签名适配（`bootstrap` 公开签名未变，故只需内部断言适配）
- **T3 闭包语料（落 `extract/mod_test.rs`）**：精确名单（只 `export { A }` ⇒ 恰 1 个符号，`B` / `C` 零命中）/ 通配等价（三条逐名语句 vs 一条 `export * from './x'` ⇒ 逐字段相等，含顺序）/ 环（`a → b → a` 不 panic、不重复）/ 菱形（`entry` 星号到 `l` / `r`，两者再星号到 `leaf` ⇒ `leaf` 的符号按 `(name, kind)` 恰一条）/ owner 四值（子元素命中、另一元素、`Owner::None`、`Owner::Ambiguous` 三条全不入集；`{fqn}Foo` 形态不入集）/ 别名改名（`export { A as B } from './x'` ⇒ 入集名 `B`、`file` 仍指向 `./x`）/ 稳定性（同输入两次调用逐字相等）/ 同目标两条具名语句（`{AgentSessionView}` 与 `{AgentSessionActions, AgentSessionViewProps}` 形态）都不得互相吞掉
- **T4 `ts_test.rs` 三闸**（闸 2 已随同一次反馈修复按 R-1 预定形态改写完毕；闸 1 / 闸 3 零改动；`extract_surface` 三面断言仍待补）：闸 1（`extract_file` 签名逐字）与闸 3（透传零符号：`export { a, b } from` / `export * from` / `export type { T } from`）**保留不改**；闸 2 由「`Statement::` 恰 1 处」改写为「`match statement` 恰 1 处 + `Statement::` = 臂数 3 + `Statement::ExportNamedDeclaration` / `Statement::ExportDefaultDeclaration` 零命中」（MUST NOT 删除断言、MUST NOT 用注释绕过）；新增 `extract_surface` 三面断言（`symbols` / `named` / `star` 如实，留白形态三面皆空或 `named` / `star` 为空）
- **T5 Rust 侧与注记用例**：`rust_test.rs` 补「子模块 `pub fn` 的 `file` = 子模块文件、内联 `mod` 的 `file` = 父文件」；`api/mod_test.rs` 的「注记行取解析链入口」判据说明改为「注记取符号定义文件（定义在入口内时与入口同值）」、断言值不变，并新增「符号定义在子模块 ⇒ 注记指向子模块文件」；`bootstrap不产used_by` 的注记形态断言随 `file` 形态复核
- **T6 静态门禁与读数**：`pnpm -C packages/desktop check`（`server:check` → `bindings:export` → `api:check` → `client:check`）全绿；一次全量检查的 `apiFindings` 分桶（期望 12 / 29、总 41、分母 44、`status = clean`、`passed = true`）与 `bindings:export` 幂等输出回填留痕 §2 / §3，并注明元素级 / 全量口径
