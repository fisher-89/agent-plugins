# TS 导出闭包 · 实现面实测留痕

> 变更: `ts-export-closure-extraction` · 采集日期: 2026-11-17 · 采集 cwd: 仓库根
> （`archi-api-gen` 的根锚由 `repository_root()` 向上找 `archi_dir`）

## §0 口径声明

| 条目 | 口径 | 命令 | 可复算性 |
|---|---|---|---|
| §1 符号名集合与注记行 | **元素级**（单元素 `bootstrap`，不是全量检查） | `cargo run -q --manifest-path packages/desktop/src-tauri/Cargo.toml -p archi-api-gen -- bootstrap App.Frontend.Components.Agent` | 零契约资产即可复算 |
| §1 附注 Rust 侧注记 | **元素级** | 同上，元素换 `App.Backend.Infra.ArchiRuntime` | 同上 |
| §2 `apiFindings` 分桶 | **全量检查**（分母 = `path` 非空元素 44） | 门禁 runner / IPC 落的全量漂移报告 | 取数通道不属实现面（design D9） |
| §3 `api:check` 三列 / `api:export` 幂等 | **全仓生成物对盘** | `pnpm -C packages/desktop api:check` / `api:export` | 同上 |

**三者读数不可互相换算**：元素级 `bootstrap` 是单元素的取数面草稿，全量检查是 44 个元素的判据汇总；
后人的读数差异不得据元素级读数解释，反之亦然。

## §1 元素级：`bootstrap App.Frontend.Components.Agent` 前后对照

### 改动前（零符号骨架；任何源码改动之前采集）

```text
# App.Frontend.Components.Agent

## 职能

（待补：一句职能）

## 不变量

（待补）

## 操作

## 类型

```json api-types
{}
```
```

符号数 = 0（`## 操作` 段零 `###` 条目、`api-types` 为 `{}`）——与 proposal「病灶一 / 病灶二」逐字一致：
barrel 的内容全是 re-export，入口单文件取数面对此零产出。

### 改动后（符号数 = 7：3 `Function` + 4 `Type`）

```text
# App.Frontend.Components.Agent

## 职能

（待补：一句职能）

## 不变量

（待补）

## 操作

### AgentInput

<!-- external @ packages/desktop/src/components/agent/agent-input.tsx:152 -->

```json api-op
{
  "params": [{"name": "arg", "schema": {"$ref": "#/$defs/AgentInputProps"}}],
  "returns": {"$ref": "#/$defs/Element"}
}
```

### AgentSessionView

<!-- external @ packages/desktop/src/components/agent/agent-session-view.tsx:122 -->

```json api-op
{
  "params": [{"name": "props", "schema": {"$ref": "#/$defs/AgentSessionViewProps"}}],
  "returns": {"$ref": "#/$defs/Element"}
}
```

### SessionPanel

<!-- external @ packages/desktop/src/components/agent/session-panel.tsx:230 -->

```json api-op
{
  "params": [{"name": "props", "schema": {"$ref": "#/$defs/SessionPanelProps"}}],
  "returns": {"$ref": "#/$defs/Element"}
}
```

## 类型

```json api-types
{
  "AskAnswerActions": {"type": "object", "x-common": {"note": "待剪枝：类型 @ packages/desktop/src/components/agent/agent-messages.tsx:33 external"}},
  "AgentSessionActions": {"type": "object", "x-common": {"note": "待剪枝：类型 @ packages/desktop/src/components/agent/agent-session-view.tsx:19 external"}},
  "AgentSessionViewProps": {"type": "object", "x-common": {"note": "待剪枝：类型 @ packages/desktop/src/components/agent/agent-session-view.tsx:30 external"}},
  "SessionPanelState": {"type": "object", "x-common": {"note": "待剪枝：类型 @ packages/desktop/src/components/agent/session-panel.tsx:12 external"}}
}
```
```

逐项对表（AC-1 / AC-5）：

| 判据 | 实测 |
|---|---|
| 名字集合逐字等于 `AgentInput` / `AskAnswerActions` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` / `SessionPanel` / `SessionPanelState` | ✅ 7 名，且顺序 = barrel 语句序（1 `AgentInput` → 2 `AskAnswerActions` → 3 `AgentSessionView` → 4 `AgentSessionActions` / `AgentSessionViewProps` → 5 `SessionPanel` → 6 `SessionPanelState`） |
| 3 个 `Function` + 4 个 `Type` | ✅ 三个 `### <op>` 段 + `api-types` 四条 |
| `AgentInputProps` / `AgentMessagesProps` / `isToolPart` 零命中 | ✅ 三者均不出现（具名名单纪律：未被 barrel 导出即不入集） |
| 注记 `@` 后指向真实定义文件 | ✅ `agent-input.tsx:152` / `agent-session-view.tsx:122` / `session-panel.tsx:230` / `agent-messages.tsx:33` / `agent-session-view.tsx:19` / `agent-session-view.tsx:30` / `session-panel.tsx:12` |
| 零 `@ packages/desktop/src/components/agent/index.tsx:<line>` 形态 | ✅ 零命中（入口串退出注记面） |
| `R-3`：同一目标的两条具名语句互不吞名 | ✅ `agent-session-view` 被两条语句指向（`{AgentSessionView}` 与 `{AgentSessionActions, AgentSessionViewProps}`），三个名字都在案——若 `visited` 键只含文件，此处只会有 5 名且无任何报警 |
| 稳定性（AC-3 的同一输入两次调用） | ✅ 同一命令连跑两次 `diff` 零差异 |

> 语料纪律：以上 7 名是**仓库实测留痕**，不进 crate 语料（`desktop-archi-api-check` 既有条款）。

### 附注 · Rust 侧注记真值源（`App.Backend.Infra.ArchiRuntime`，元素级口径）

改动前这四条的 `@` 后全是入口 `…/src/lib.rs:<line>`（行号属另一个文件）；改动后逐条指向**所在模块文件**：

```text
<!-- crateOnly @ packages/desktop/src-tauri/crates/infra/archi/src/report.rs:16 -->
<!-- external @ packages/desktop/src-tauri/crates/infra/archi/src/runner.rs:32 -->
<!-- external @ packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts.rs:15 -->
"LocalArchiCheck": {"type": "object", "x-common": {"note": "待剪枝：类型 @ packages/desktop/src-tauri/crates/infra/archi/src/lib.i.rs:6 external"}}
```

Rust 分支的抽取面 / `visibility` 四值 / 签名归一逐字未变（`extract_element` 的 Rust 分支仍直接
`rust::extract_crate`，`file` 由同一趟 `walk_items` / `walk_modules` 给出 ⇒ 与 `extract_crate` 逐字段相等，AC-6）。

## §2 全量检查的 `apiFindings` 分桶

**已实测（取数通道 = 契约面自检 `archi_api check`，全量口径）**：

```text
total   = 41
byCategory = { contract: 29, entry: 12 }      ← 期望 12 / 29（总 41）逐项相等
artifacts = { missing: [], mismatched: [], orphaned: [] }
```

分母 44（`path` 非空元素）与派生的 `status = clean` / `passed = true` 属漂移报告字段，本工具面不输出——
**待 test-execution 的全量检查报告回填**（design D9 / 待决 2）。MUST NOT 用元素级 `bootstrap` 读数顶替本段。

语义根据（为什么变了取数面而读数不变）：闭包只可能让导出集**变宽** ⇒ `apiMissing` 只减不增；本仓唯一持
barrel 的 `App.Frontend.Components.Agent` 尚无契约文件（`apiMissing` 报的是「declared 但契约缺失」，与导出集
宽度无关）⇒ 41 条逐条不变。`apiUndeclared` 仍 `off` ⇒ 零新增条目。

## §3 生成物对盘（实现面）

### `pnpm -C packages/desktop api:check`

```text
$ cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- --check
（零输出，退出码 0 ⇒ missing / mismatched / orphaned 三列皆空）
```

### `pnpm -C packages/desktop api:export` 二跑（幂等）

```text
$ cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate
0 份生成物已写入（3 份未变）
$ cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate
0 份生成物已写入（3 份未变）
```

两次二跑后 `git status --porcelain .dev-team/architecture` **零输出**（`.dev-team/architecture/**` 零 diff）。

### 版本与零扩散

```text
packages/desktop/package.json               "version": "0.6.49"          （0.6.48 → +1 patch，只加一次）
packages/desktop/src-tauri/tauri.conf.json  "version": "../package.json"  （引用形态，零内容改动）
packages/desktop/src-tauri/Cargo.toml       version = "0.1.0"             （crate 版本线独立，不随动）

git status --porcelain 的改动集：
 M packages/desktop/package.json
 M packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs
 M packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs
 M packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs
 M packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs
?? openspec/changes/ts-export-closure-extraction/
```

`bindings:export` 幂等：`ApiSymbol` 不进 `dto` / 不进 specta ⇒ `packages/desktop/src/types/generated/**` 零 diff
（`git diff --stat` 零输出即等价证据；最终幂等输出由 static-check 相位回填）。

## §4 源码不变量（机械锚点，供测试相位对表）

| 锚点 | 命令 | 实测 |
|---|---|---|
| 单点分派 | `grep -c "match statement" api/extract/ts.rs` | **1** |
| 臂数 | `grep -c "Statement::" api/extract/ts.rs` | **3**（= 三臂） |
| 两类留白零命中 | `grep -c "Statement::ExportNamedDeclaration\|Statement::ExportDefaultDeclaration"` | **0** |
| 签名逐字 | `grep -n "pub fn extract_file" api/extract/ts.rs` | `pub fn extract_file(file: &str, source: &str) -> Vec<ApiSymbol> {` |
| 别名零启发式 | `grep -c "ArrowFunctionExpression" api/extract/ts.rs` | **0** |
| 零第二份解析 / 归属 | `grep -rn "oxc_resolver\|TsconfigOptions\|fn owner_of\|prefix_matches" api/` | **0**（`Resolver::new` 一条 = `TsResolver::new`，即被复用的单点本身；见下注） |
| 闭包只建一次 | `grep -c "TsResolver::new" api/extract/mod.rs` | **1**（`Closure::new` 内） |
| 环防键唯一 | `grep -rn "BTreeSet<(String, Option<Vec<String>>)>" api/` | **1** |
| 分段前缀不被裸 `starts_with` 顶替 | `grep -c 'format!("{}."' api/extract/mod.rs` | **1** |
| `ApiSymbol` 构造点齐备 | `grep -c "ApiSymbol {"`（产品侧） | **4**（`ts::push_function` / `ts::push_type` / `rust::push_type` / `rust::function_symbol`），四处均带 `file:` |
| 注记真值源 | `grep -c "symbol.file" api/mod.rs` | **2**（op 注记 + 类型注记，恰两个写出点） |
| `@ {entry}` / `@ 入口` 残留 | `grep -c "@ {entry}\|@ 入口" api/mod.rs` | **0** |
| D52 留白 | `grep -rn "extract::extract(" crates/**`（去 `_test.rs`） | **0** |
| `ts_import_specifier` 未接线 | `grep -rn "ts_import_specifier" api/extract api/mod.rs` | **0** |
| 无 `.i.ts` 试点 | `grep -rn "\.i\.ts" api/extract` | **0** |
| 语言分派仍两处 | `grep -c "match paths::language_of(" api/extract/mod.rs` | **2**（`extract` / `extract_element` 各一） |
| `language_of` 第三处是过滤不是分派 | `grep -c "paths::language_of(" api/extract/mod.rs` | **3**（两处分派 + `export_target` 的 TS 面过滤，D5） |

> 注（`Resolver::new` 一条的口径）：任务书的两条自查里，「`Resolver::new` ⇒ 0」与「`TsResolver::new` 恰 1 处」
> 字面上互斥（前者是后者的子串）。取舍：**后者才是被钉的不变量**（每元素只建一次解析器、且只在 `Closure::new`
> 内），故实现保留唯一一处 `TsResolver::new`；「零第二份解析」由 `oxc_resolver` / `TsconfigOptions` 零命中承担
> （`api/` 下不出现 resolver 的构造面，只用 `scan` 的既有单点）。MUST NOT 为凑「0」把 `TsResolver::new` 藏进别名
> ——那会让这条不变量失去唯一报警点。

## §5 测试面（本相位已完成 / 仍移交）

**静态门禁的一次反馈修复**（`server:check` 跑 `cargo clippy --workspace --all-targets`，编译面含 `#[cfg(test)]`
sidecar）把两批「不修就编译不过 / 恒红」的项从移交清单拉回本相位——只做**形状适配**与**按 R-1 预定形态改写**，
零新增用例、零删除断言：

| 项 | 内容 | 判据 |
|---|---|---|
| T1 ✅ | `api/extract/mod_test.rs` 的 `fn symbol(…)`、`api/check_test.rs` 的 `fn function(…)` / `fn type_symbol(…)` 三处 `ApiSymbol` 字面量补 `file: "src/lib.rs"` | `cargo clippy --all-targets` 编译面 |
| T2 ✅ | `extract_element` 全部调用点补 `elements` / `element`（`api/extract/mod_test.rs` 六处、`api/mod_test.rs` 一处）：Rust 元素用其 `path` 造元素表，TS 元素用 `views/` + `tsx/` 的父元素，未知语言 / 读不到两例用与其入口同形的元素 | 同上 |
| T4 ✅（闸 2） | `ts_test::ts取数面为导出面且非导出分支零命中` 由「`Statement::` 恰 1 处」改写为「`match statement` 恰 1 处 + `Statement::` == 3（= 臂数）+ 两臂在位 + 两类留白字面量零命中」——即 **R-1 预定的形态**，断言未删、判据更严 | `cargo test -p archi`（1037 全绿） |
| T2′ ✅ | `mod_test::未知语言落空集恰两处且无猜测回落` 的 `paths::language_of(` 计数由 2 → 「`match paths::language_of(` == 2 + `paths::language_of(` == 3」，把新增的第三方调用（`export_target` 的 TS 面过滤，D5）显式登记为**过滤而非分派** | 同上 |
| T3 / T5 移交 | 闭包语料（精确名单 / 通配等价 / 环 / 菱形 / owner 四值 / 别名 / 稳定性）与 `extract_surface` 三面断言、`rust_test` 的 `file` 用例、`api/mod_test` 的「子模块符号注记指向子模块」用例 | 测试相位 |
| T6 移交 | 全量检查报告的 `status` / `passed` / 分母 44（§2 已给出 `total = 41` 与 `contract 29 / entry 12` 实测分桶） | 门禁 runner / test-execution |

门禁读数（本相位实测）：`cargo fmt --all --check` 干净、`cargo clippy --workspace --all-targets` 零错误（唯一
warning 是 `workflow::write::create::compensate_tree_failure` 的 8 参告警，本变更未触碰该文件，属既有）、
`cargo test -p archi` **1037 passed / 0 failed**、`server:check` / `bindings:export` / `api:check` / `client:check`
四段全 0。
