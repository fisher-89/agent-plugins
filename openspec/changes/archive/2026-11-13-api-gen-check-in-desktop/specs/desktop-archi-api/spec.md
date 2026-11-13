# desktop-archi-api Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 只改两条 requirement，二者都是**追加**性质：①「生成物入仓与零 diff 门挂载」补一条「应用内两条 IPC 与 CLI 两驱动面消费同一实现」；②「仓库根文档的契约纪律节」的要点 ⑤ 随 `apiFindings` 纳管口径改写（`status` 会随契约在案变化、`passed` 仍不变）。契约格式、关键字集、生成物落点、`verify` 闭环与语言纪律四条 requirement 逐字不变。

## MODIFIED Requirements

### Requirement: 生成物入仓与零 diff 门挂载

四类生成物 SHALL 全部入仓并进零 diff 门，形态沿既有 `bindings.ts` 先例（MUST NOT 另造第二套守卫机制）：

| 生成物 | 落点 | 入库 | 门禁 |
|---|---|---|---|
| `<FQN>.schema.json` | `<archi_api_dir>/<Domain>/` | 是 | `api:check` 零 diff |
| `INDEX.toml` | `<archi_api_dir>/` 根（全局唯一一份） | 是 | `api:check` 零 diff |
| `<root>.i.rs` | 元素根同级 | 是 | `api:check` 零 diff + `cargo check` |
| `<root>.i.ts` | 元素根同级 | 是 | `api:check` 零 diff + `tsc --noEmit` |

- **脚本面封闭两条**：`api:export`（`cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`，写盘）与 `api:check`（同一调用加 `--check`；生成到内存后比对、**零写入**，差异即非零退出；比对由工具内完成，**不依赖任何 `git diff` 段**）。`check` 链 SHALL 由三段扩为四段：`server:check && bindings:export && api:check && client:check` —— 该扩展只加一段（`api:check` 插在第二段之后）；`bindings:export` 与 `client:check` 两段命令形态零变化，`server:check` 的 fmt 段 SHALL 由写模式 `cargo fmt --all` 改为校验模式 `cargo fmt --all --check`（写模式门使工作树无法停在不动点，故本变更 MUST 改它；clippy 段逐字不动）。**第二段 SHALL 是 `bindings:export`**（`check` 的生成物零 diff 守卫已退役，见 `desktop-ipc-type-bindings`——`check` 只前置重导出、不比对）；规范内 MUST NOT 引用任何已退役的守卫脚本名；段名 SHALL 以 `package.json` 的 `scripts` 实存键为准（`grep -c '"bindings:export"' package.json` ≥ 1 即证该键在案）。
- **应用内两驱动面与脚本面同源**：生成物的渲染与三类差异比对 SHALL 各有唯一实现（`core/archi` 的渲染 / 比对纯函数），写盘 SHALL 有唯一实现（`infra/archi` 的写盘单点）；应用内两条 IPC（检查 / 生成，见 `desktop-archi-contract-panel`）与 CLI 两子命令 MUST NOT 各写一份渲染、比对或写盘——**四条消费路径共用同一实现**是「零 diff 门」与「应用内检查」结论一致的前提。脚本面两条命令形态本文逐字不变（仅写盘实现的落点变化，命令串不变）。
- **`knip` 边界**：`knip.json` 的 `ignore` SHALL 增 `src/**/*.i.ts`（生成物非手写代码；实测未被 import 的 `.i.ts` 会被报为 unused file，因 `project` glob 覆盖 `src/**/*.ts`）；MUST NOT 新增 `ignoreExportsUsedInFile` 或 `ignoreDependencies` 条目（那是关判据而非满足判据）。
- **类型绑定再生成**：`ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` 与报告字段 `apiFindings` SHALL 经 `bindings:export` 进 `bindings.ts` 与 `bindings.zod.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役）；本能力 MUST NOT 手写前端类型镜像。
- **TS 侧形状校验的机制前提**：实现侧 SHALL `import type` 引用生成的 `.i.ts` 并标注返回类型（否则校验不发生）；`isolatedModules` 下重导出须写 `export type { … }`。
- **版本纪律**：本变更改 `packages/desktop` 源码 ⇒ `package.json` 的 `version` SHALL 相对**落地时 `HEAD` 的现值 +1 patch**（相对递增，规范内 MUST NOT 写死具体版本号——现值随每次源码改动滚动，写死即失效）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动。

#### Scenario: 四类生成物齐入仓且零 diff

- **WHEN** 执行 `api:export` 后再执行 `api:check`
- **THEN** 四类生成物均在盘且已在 git 内，第二次检查零差异、退出码 0；`git status` 干净

#### Scenario: 手改任一生成物即被拦

- **WHEN** 手改已提交的 `<FQN>.schema.json`、`INDEX.toml`、`.i.rs` 或 `.i.ts` 任一后执行 `api:check`
- **THEN** 检查以非零退出并列出被改文件（`--check` 阶段无写盘）；回退后恢复通过

#### Scenario: check 链扩一段且 fmt 段改校验模式

- **WHEN** 审查 `package.json` 的 `check` 脚本与既有 `server:check` / `bindings:export` / `client:check` 三段
- **THEN** `check` 为四段，逐字为 `pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check`（新增 `api:check` 位于 `bindings:export` 与 `client:check` 之间）；`bindings:export` 与 `client:check` 两段命令原文逐字不变；`server:check` 的 fmt 段为 `cargo fmt --all --check` 且其 clippy 段逐字不变；`client:check`（含 knip）在 `src/**/*.i.ts` 进 ignore 后仍零 dead code 报告

#### Scenario: 应用内检查与脚本面结论一致

- **WHEN** 在同一观察根分别调用 `archi_api_check`（IPC）与 `api:check`（CLI）
- **THEN** 两者报出的三类差异文件集合逐条相等；随后用 `archi_api_generate`（IPC）生成，再跑一次 `api:check` 得零差异——四条消费路径（两 IPC / 两 CLI 子命令）零第二份渲染、比对或写盘实现

### Requirement: 仓库根文档的契约纪律节

仓库根 `AGENT.md` SHALL 承载一节 `## 模块 API 契约` 作为 agent 侧的操作纪律入口，内容 SHALL 限于**指针与最简规则**，MUST NOT 复制契约正文、判据长表或 fenced json 示例：

- 该节 SHALL 恰含五条要点：① 手写真相位置 `<archi_api_dir>/<Domain>/<FQN>.md` 与三类 fence 的名字（`api-op` / `api-types` / `api-reexport`）；② 四类生成物勿手改（`.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）与「已提交手改即零 diff 门红、未提交即被静默覆盖」；③ `api:export` / `api:check` 两脚本与 `check` 链的段序；④ `api = none` 是显式留白而非「还没写」；⑤ **契约在案元素**的判据进 `status`（可把状态词抬到 `warnings`）、不进 `passed`，索引与计数经 `INDEX.toml` 查看，两轴经架构页「契约」视图一眼看全。
- 该节 MUST NOT 含契约示例正文、`ArchiApiFindingKind` 的十值长表或判据定义（定义归本能力与 `desktop-archi-api-check`）——文档与 spec 双写必然漂移（`AGENT.md` / `CLAUDE.md` 同源复制即既有反例）。
- 该节 MUST NOT 出现 `openspec/specs/**` 路径（spec 语料非 agent 的契约操作面），且 MUST NOT 把契约路径写进模型元素 `docs`（见 `desktop-archi-module-docs`）。
- `CLAUDE.md` SHALL 同步同一节且与 `AGENT.md` 保持既有同源纪律：两文件该节正文逐字相同，全文件差异 SHALL 限于节外既有差异（标题行、首行说明行、文件末尾换行）；**该节自身的两份副本若分化即违反本条**。

#### Scenario: 纪律节只指路不带正文

- **WHEN** 审查 `AGENT.md` 的 `## 模块 API 契约` 一节
- **THEN** 该节恰有 5 条要点、`grep -c '^## 模块 API 契约' = 1`；`grep 'apiEntryMissing'` 在该节零命中（判据长表未复制）；节内零 `openspec/specs/` 路径；节内无 fenced json 代码块

#### Scenario: 两份副本不分化

- **WHEN** 对 `AGENT.md` 与 `CLAUDE.md` 做 `diff`
- **THEN** 差异恰 2 处且全在 `## 模块 API 契约` 节外（`:1` 标题、`:3` 首行说明）；该节在两文件各恰一处且节内正文零 diff 行（逐字相同）

#### Scenario: 纪律节第 ⑤ 条与纳管口径一致

- **WHEN** 对照该节第 ⑤ 条与 `desktop-archi-api-check`「报告并列槽位 apiFindings」的纳管条款
- **THEN** 两处都表达「契约在案元素的判据影响 `status`、不影响 `passed`」，MUST NOT 残留「不进 `status`」一类旧口径

## Module Contract

### API: 生成物驱动面（零命令形态变化，实现落点变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `packages/desktop/package.json` | 零改动 | `api:export` / `api:check` / `check` 三条命令串逐字不变；`version` 相对 `HEAD` +1 patch |
| `crates/core/archi/src/api/**` | 追加 | 渲染 / 比对纯函数（`workspace_render` / `compare`）为四条消费路径的共同实现；契约格式与判据面零改动 |
| `crates/infra/archi/src/api_artifacts.rs` | 新增 | 写盘单点（`write_artifacts`）；CLI 与 IPC 共用 |

### Component: 仓库根文档

| 项 | 变化 | 描述 |
|---|---|---|
| `AGENT.md` / `CLAUDE.md` | 改 | `## 模块 API 契约` 节第 ⑤ 条改为纳管口径（契约在案元素的判据进 `status`、不进 `passed`）；两份副本该节正文逐字相同 |
