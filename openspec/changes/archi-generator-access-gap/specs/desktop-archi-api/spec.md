# desktop-archi-api Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 只动一处：把「谁可以驱动生成」从**四个消费路径**扩为**五个**（加 agent 工具 `archi_generate`），并把「渲染单点 / 比对单点 / 写盘单点」三条唯一实现逐字钉住——生成物的**来源**不变，只是多了一个调用方。

## MODIFIED Requirements

### Requirement: 生成物入仓与零 diff 门挂载

**三类**生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `INDEX.toml` | `<archi_graph_dir>/`（全局唯一一份） | 是 | `api:check` 零 diff |
| `<入口主干>.i.rs` | **元素实现入口同级**（`crates/infra/archi/src/lib.i.rs`） | 是 | `api:check` 零 diff + `cargo check` |
| `<入口主干>.i.ts` | **元素实现入口同级**（`views/welcome-view.i.ts`） | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **`<叶段>.schema.json` 不再是生成物**：它是手写真相（第二份资产），MUST NOT 进 `api:check` 的应生成集、MUST NOT 进 `orphaned` 列、MUST NOT 由任何生成器写出。它同样入库（git 基线是人工 review 与审计的入口）。
- **`INDEX.toml` 的 `generated` 列取入口同级路径**：无入口（`Absent` / `Invalid`）的元素该列留空——生成物路径是入口的函数，无入口即无路径可写。
- **孤儿扫描 SHALL 只剩一个根**：`<archi_graph_dir>/INDEX.toml`（全局唯一）。`*.schema.json` MUST NOT 进孤儿枚举——不去掉就是「11 份手写文件全进 `orphaned`」，症状是「全红」而不是「少一条」。深度上限防环、符号链接不跟随、`read_dir` 只出现在有界枚举里、repo 相对路径串由同一 `display` 单点产出，四条纪律逐字沿用。
- **脚本面封闭两条**：`api:export`（写盘）与 `api:check`（生成到内存后比对、**零写入**，差异即非零退出；比对由工具内完成，不依赖任何 `git diff` 段）。`check` 链 SHALL 为四段：`server:check && bindings:export && api:check && client:check`——段序与段数零变化，`bindings:export` 与 `client:check` 两段命令形态逐字不变。
- **单一实现、多驱动面（本变更由四改五）**：渲染与三类差异比对各有唯一实现（`core/archi` 纯函数），写盘有唯一实现（`infra/archi` 的 `write_artifacts`）；消费路径 SHALL 恰为**五条**——两 IPC（检查 / 生成）、两 CLI 子命令（`export` / `check`）与 **agent 工具 `archi_generate`**（见 `desktop-archi-agent`「archi_generate 生成物工具」）。五者 MUST NOT 各写一份渲染、比对或写盘；agent 工具 SHALL 经 `core/archi` 的 `ArtifactWriter` 端口把写盘委托给同一 `write_artifacts`（端口与实现落位见 `desktop-crate-layout`「core/archi crate 落地登记」）。MUST NOT 为 agent 面新增任何写盘实现、MUST NOT 让 `core/archi` 出现写盘。
- **`knip` 边界**：`knip.json` 的 `ignore` SHALL 含 `src/**/*.i.ts`；MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies`。
- **类型绑定再生成**：`ArchiApiFindingKind`（十值）/ `ArchiApiFinding` / 报告字段 `apiFindings` / `ArchiModuleDetail` 的新字段 SHALL 经 `bindings:export` 进 `bindings.ts` 与 `bindings.zod.ts`（再生成后入库，过期由提交前人工核对）；本能力 MUST NOT 手写前端类型镜像。**工具响应层的字段不走该面**：`archi_generate` 的 `{ written, unchanged, conflicts }` 与 `archi_api(check)` 的 `remedy` 是字符串响应，MUST NOT 进 DTO 或 bindings。
- **TS 侧形状校验的机制前提**：实现侧 SHALL `import type` 引用生成的 `.i.ts` 并标注返回类型；`isolatedModules` 下重导出须写 `export type { … }`。
- **版本纪律**：本变更改 `packages/desktop` 源码 ⇒ `package.json` 的 `version` SHALL 相对**落地时 `HEAD` 的现值 +1 patch**（规范内 MUST NOT 写死具体版本号）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

#### Scenario: 三类生成物齐入仓且零 diff

- **WHEN** 执行 `api:export` 后再执行 `api:check`
- **THEN** 三类生成物均在盘且已在 git 内，第二次检查零差异、退出码 0；`git status` 干净

#### Scenario: 手改 `.schema.json` 不被 `api:check` 判红

- **WHEN** 手改一份已提交的 `<叶段>.schema.json`（合法改动）后执行 `api:check`
- **THEN** 差异面只报 `.i.rs` / `.i.ts` / `INDEX.toml` 的应随动项（若有），**不报该 schema 为 `orphaned` / `mismatched`**——它不是生成物；回退 .i.* 后检查恢复通过

#### Scenario: 手改真生成物即被拦

- **WHEN** 手改已提交的 `INDEX.toml`、`.i.rs` 或 `.i.ts` 任一后执行 `api:check`
- **THEN** 检查以非零退出并列出被改文件（`--check` 阶段无写盘）；回退后恢复通过

#### Scenario: 孤儿生成物在任意深度均被检出

- **WHEN** 删除元素或改其根路径后，在任意深度留下 `*.i.rs` / `*.i.ts` 残留，随后执行 `api:check`
- **THEN** 「应删未删」逐条列出这些文件并以非零退出；`target` / `node_modules` / `dist` / `build` / `.git` 里的同名文件不参与；普通源文件（`*.ts` / `*.rs` / `*.md` / `*.schema.json`）零误伤

#### Scenario: check 链四段且 fmt 段为校验模式

- **WHEN** 审查 `package.json` 的 `check` 脚本与四段
- **THEN** `check` 逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`；`api:check` 的调用形态逐字不变；`server:check` 的 fmt 段为 `cargo fmt --all --check`

#### Scenario: 五条驱动面同源可对拍（本变更新增）

- **WHEN** 在同一临时根上分别经 IPC `archi_api_generate`、CLI `archi-api-gen generate` 与 agent 工具 `archi_generate` 各生成一次
- **THEN** 三者写出的文件集与字节内容逐字相同、`written` / `unchanged` 的划分相同；`grep` 对「第二份 `write_artifacts` 等价实现」零命中；`core/archi` 仍零 `fs::write`（写盘只在 `infra/archi` 一处）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### API: 生成物驱动面

| 项 | 变化 | 描述 |
|---|---|---|
| IPC `archi_api_generate` | 零改动 | 签名与 `ArchiApiWriteOutcome` 出参逐字不变 |
| CLI `archi-api-gen` | 零改动 | `export` / `check` 两子命令逐字不变 |
| agent 工具 `archi_generate` | 新增驱动面 | 经 `core/archi::ArtifactWriter` 端口委托 `write_artifacts`；响应在工具层拼 `{ written, unchanged, conflicts }`，**不进 DTO / bindings** |

### Component: 数据与门禁

| 项 | 描述 |
|---|---|
| `.dev-team/architecture/graph/INDEX.toml` 与 `.i.*` | 生成规则、落点、孤儿口径、零 diff 门逐字不变；本变更零生成物字节变化 |
| `api:check` 四段链 | 段序与命令形态逐字不变 |
