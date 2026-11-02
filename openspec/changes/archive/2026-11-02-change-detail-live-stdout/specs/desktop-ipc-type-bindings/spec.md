# desktop-ipc-type-bindings Specification（delta: change-detail-live-stdout）

## ADDED Requirements

### Requirement: 实时输出三命令与出线类型入册

脚本步运行中 stdout 的取数面 SHALL 以三条新命令入 `all_commands!` 单一登记面（与 builder 注册同源），使 `bindings.ts` 与 `bindings.zod.ts` 双产物自动生成 typed 包装与 schema（前端 `ipc` 代理层零改动即覆盖）：

| 命令 | 签名 | 出线类型 |
|---|---|---|
| `exec_output_path` | `(root: String, change_name: String, exec_id: String) -> Option<String>` | — |
| `read_exec_command` | `(root: String, change_name: String, exec_id: String) -> Option<String>` | — |
| `read_exec_output_delta` | `(root: String, change_name: String, exec_id: String, from_offset: u64) -> Option<ExecOutputChunk>` | `ExecOutputChunk` |

- **出线 DTO 落位**：`ExecOutputChunk` SHALL 定义在壳层命令模块（`src/commands/changes/`），携 `serde`（`rename_all = "camelCase"`）+ `specta::Type`，字段为 `text: String` / `next_offset: u64` → TS `{ text: string; nextOffset: number }`。它 MUST NOT 定义在 `crates/infra/exec`（该 crate 的依赖面 MUST NOT 因出线需要而扩 serde / specta；exec 侧返回的 `ExecOutputDelta` 为**零 derive 纯结构**，命令层做 1:1 映射）。
- **`u64` 出线口径**：`from_offset` 与 `nextOffset` 沿用 workspace 既有 bigint 配置（`dangerously_cast_bigints_to_number`）出线为 `number`，MUST NOT 单独引入 `BigInt` 面（与 `tokenInput` / `tokenOutput` 同口径；文件长度上界受 2^53 约束，实际日志远小于此）。
- **miss 语义与既有读命令一致**：三命令在 blank root、`exec_id` 非法 / 未知、或两槽位均无该记录时 SHALL 返回 `None`（miss 非错误、非 `Err`），MUST NOT 借故障通道表达「暂时还没输出」——运行中零字节由 `Some { text: "", nextOffset: 0 }` 表达（与「无记录」可分辨）。
- **零新事件面**：三命令均为同步返回，MUST NOT 引入 `emit` / `listen` 事件或新 `Channel` 参数。
- **既有命令零变**：`read_exec_output(root, change_name, exec_id) -> Option<String>` 的签名、参数键与返回类型 MUST NOT 改动（既有 spec 的「零变」承诺与既有前端 / 测试调用面因此不破）。
- **重导出**：三命令与 `ExecOutputChunk` 出线后 MUST 经导出入口再生成并入库（`bindings:export` 即把盘上产物修至最新）；入库物是否过期由提交前 `git status` / `git diff` 人工核对，MUST NOT 依赖自动化 diff 门禁（零 diff 守卫已退役）。

#### Scenario: 运行中零字节与无记录可分辨

- **WHEN** 前端对「刚 begin 尚未产出字节」与「两槽位均无该 exec_id」两种情形各调一次 `read_exec_output_delta(…, 0)`
- **THEN** 前者返回 `{ text: "", nextOffset: 0 }`（非 `null`、非 `Err`）；后者返回 `null`；两者前端可分辨（「运行中，等待输出…」vs「（无输出记录）」）

#### Scenario: 三命令 typed 包装在案

- **WHEN** 重导出 bindings 后审查生成物
- **THEN** `commands` 含 `execOutputPath` / `readExecCommand` / `readExecOutputDelta` 三个 typed 包装（参数键为 lowerCamelCase：`root` / `changeName` / `execId` / `fromOffset`），`bindings.zod.ts` 的 `argsSchemas` / `resultSchemas` 各含对应表项

#### Scenario: ExecOutputChunk 出线形态

- **WHEN** 审查生成物中 `readExecOutputDelta` 的返回类型
- **THEN** 为 `Promise<ExecOutputChunk | null>`，`ExecOutputChunk` 形如 `{ text: string; nextOffset: number }`（camelCase、`u64` → `number`），无 `bigint` 面

#### Scenario: 既有整读命令零 diff

- **WHEN** 导出后审查 `readExecOutput` 的生成签名与 args schema
- **THEN** 与本变更前逐字一致（参数键与返回类型零变化）；既有前端调用点与测试零改动

#### Scenario: 重导出刷新过期生成物

- **WHEN** 新增三命令后只改 Rust 源码、未重导生成物即执行 `pnpm check`
- **THEN** 前置 `bindings:export` 把 `src/types/generated/` 刷新为含三命令与 `ExecOutputChunk` 的最新产物；该改动在 `git status` 中可见，由提交前人工核对入库

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/mod.rs` | 命令登记面 | `all_commands!` 增 `commands::changes::exec_output_path` / `read_exec_command` / `read_exec_output_delta` 三行（builder 注册与 `generate_handler!` 同源；命令条数不硬编码） |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 三薄命令与出线 DTO | `ExecOutputChunk { text: String, next_offset: u64 }`（`rename_all = "camelCase"` + `specta::Type`）；三命令做 blank root 早退（读命令空结果语义 `None`）+ `LiveExec`/exec 三件回查的 1:1 映射（`ExecOutputDelta` → `ExecOutputChunk`） |
| `packages/desktop/src/types/generated/bindings.ts`（生成，入库） | 前端唯一 IPC 类型与调用面 | 三命令 typed 包装 + `ExecOutputChunk` 类型；随前置重导出更新（过期由提交前人工核对） |
| `packages/desktop/src/types/generated/bindings.zod.ts`（生成，入库） | 第二产物 | 三命令 `argsSchemas` / `resultSchemas` 表项 + `ExecOutputChunk` 具名 schema（纯接受面，零修饰） |
| `packages/desktop/src/lib/ipc.ts` | 消费面（零改动） | 键遍历自动覆盖三命令，参数面 strict / 结果面 soft-report 语义不变 |
