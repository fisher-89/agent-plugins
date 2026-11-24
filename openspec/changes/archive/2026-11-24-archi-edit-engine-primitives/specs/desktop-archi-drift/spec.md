# desktop-archi-drift Specification（delta: archi-edit-engine-primitives）

## Purpose

本 delta 只给判定域加**两个单文件粒度的哈希薄封装**（`file_hash` / `file_hash_of`），服务写面乐观锁——`plan_*` 的 `expect` 校验与 `ModelWrite.hash` 的内容哈希。既有两份哈希（`modelHash` 报告字段 / `assets_hash`）的覆盖面、语义与报告形状逐字不变：

- **零新实现**：两者都经既有私有 `hash_files` 累加 helper（文件名 + 字节 + 空字节分隔），MUST NOT 另写一份 Sha256 拼装，`hash_files` 的可见性不放宽。
- **粒度与可比性**：单文件时文件名分量恒定 ⇒ 值随内容变且同路径稳定；文件名参与累加 ⇒ 与 `modelHash` / `assets_hash` **数值不可比**，唯一可比对象是同一路径的前后取值。
- **写后等价锚**：`file_hash_of(file, content)` 逐字等于「把 `content` 落盘后 `file_hash(file)`」——写面据此把「写后新值」直接交给调用方作下一次 `expect`。
- 判定面零改动：`check` / 报告结构 / `status` 五值 / `modelHash` 口径 / 词汇集全部不动。

## ADDED Requirements

### Requirement: 单文件哈希（写面乐观锁粒度）

判定域 SHALL 另提供两个**单文件粒度**哈希函数（`archi::drift` 公开面）：`file_hash(path) -> String`（读盘后累加）与 `file_hash_of(path, content) -> String`（字节由调用方给出）。用途单一是写面乐观锁——`plan_*` 以 `file_hash` 校验 `expect`、以 `file_hash_of` 填 `ModelWrite.hash`（写面语义归 `desktop-archi-model`）：

- **零第二份实现**：两者 SHALL 经既有私有 `hash_files` 累加 helper（按给定序逐文件累加「相对路径 + 空字节 + 字节 + 空字节」，输出 `sha256:` 前缀的十六进制摘要）；MUST NOT 另写一份 Sha256 拼装，`hash_files` 的**可见性不放宽**（仍是私有 helper）。
- **覆盖面封闭**：本组只覆盖**单个文件**；报告字段 `modelHash` SHALL 继续只覆盖 `graph/model/*.toml`，`assets_hash` SHALL 继续覆盖四类架构资产，两者口径逐字不变。两个新函数 MUST NOT 被写进报告、MUST NOT 取代 `modelHash` / `assets_hash`、MUST NOT 被用作缓存失效依据（本期零缓存）。
- **可比性边界**：文件名参与累加 ⇒ 单文件值**与目录粒度值（`modelHash` / `assets_hash`）数值不可比**（异名同内容两文件得不同值，同内容不同路径亦然）。唯一可比对象是「同一路径的前后取值」；消费面 MUST NOT 断言本组值与 `modelHash` / `assets_hash` 相等，文档与测试口径都按此写明。
- **写后等价锚**：`file_hash_of(file, content)` SHALL 逐字等于「把 `content` 落盘到 `file` 后 `file_hash(file)`」的返回值——写面把该值作为 `ModelWrite.hash` 交付，使调用方能把它直接作为下一次 `expect`（TOCTOU 闭环的前提）。
- **稳定与容错（沿用资产哈希条）**：同一份字节必得同一值，与文件系统时间戳无关；单文件读取失败 SHALL 以空字节参与累加（宁可哈希变化，MUST NOT 让消费侧因 IO 抖动失败）。纯读盘、零写盘、零 spawn。

#### Scenario: 形态与同盘稳定

- **WHEN** 对同一模型文件连续两次调用 `file_hash`，随后改动该文件一个字节再调一次
- **THEN** 前两次返回值逐字相等且形态为 `sha256:` + 64 位小写十六进制；第三次取值变化——同一路径上「值 != 上次值」即「文件已变」的判据成立

#### Scenario: 写后等价锚成立

- **WHEN** 取任一 `plan_*` 产出的 `content` 与 `hash`，用 `file_hash_of(file, content)` 独立重算，再把 `content` 落盘到 `file` 后调 `file_hash(file)`
- **THEN** 三次取值逐字相等（`hash` == `file_hash_of` == 落盘后的 `file_hash`）；把该值作 `expect` 再调一次 `plan_*` 返回 `Ok`——「写后新值 → 下一次乐观锁」闭环成立

#### Scenario: 不可与目录粒度哈希互比

- **WHEN** 对同一文件分别取 `file_hash`、含它的 `assets_hash(root)` 与报告 `modelHash`；另对两个异名同内容的文件各取 `file_hash`
- **THEN** 三者只共享 `sha256:` 前缀，数值 MUST NOT 被判等；两个异名同内容文件的 `file_hash` 亦不相等（文件名参与累加）——可比性边界由该条钉住

#### Scenario: 零独立拼装

- **WHEN** 审查 `crates/core/archi/src/drift/mod.rs` 与写面消费者 `crates/core/archi/src/edit/`
- **THEN** 全仓只有一处累加实现（`drift::hash_files`，仍为私有）；`edit/` 内零 `Sha256` / `sha2` 字面；`model_hash` / `assets_hash` / `check` 的实现逐字不变

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/mod.rs` | 单文件哈希（本变更 +2 个公开薄封装，既有面零改动） | `pub fn file_hash(path: &Path) -> String` / `pub fn file_hash_of(path: &Path, content: &str) -> String`，两者都经**私有** `hash_files`（可见性零放宽、零独立 Sha256 拼装）；`model_hash` / `assets_hash` / `check` / `CheckInput` / `CheckError` 逐字不变 |
| `crates/core/archi/src/drift/mod_test.rs` | 断言面 | 新增四条（形态与同盘稳定 / 写后等价锚 / 不可与目录粒度互比 / 零独立拼装）；既有 `assets_hash` 与 `model_hash` 用例逐字不动 |
| `crates/core/archi/src/edit/`（消费者，口径归 `desktop-archi-model`） | 写面乐观锁 | `plan_*` 以 `file_hash` 校验 `expect`、以 `file_hash_of` 填 `ModelWrite.hash`；本能力只登记哈希口径，不定义写面的落点 / 三态 / 自证语义 |

### API: 对外面

| 项 | 变化 | 描述 |
|---|---|---|
| `file_hash` / `file_hash_of` | 新增（Rust 面） | 单文件内容哈希；`sha256:` + 64 位小写 hex；非 IPC 命令、不带 `specta::Type`、**不出线**（`bindings.ts` 零 diff） |
| 报告 `modelHash` | 零改动 | 仍只覆盖 `graph/model/*.toml`；口径逐字不变 |
| `assets_hash` | 零改动 | 四类资产覆盖面与累加口径逐字不变；与新增两函数共用同一 helper，覆盖面不同 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| `archi_drift` 命令与报告结构 | 零改动：`modelHash` / `errors` / `warnings` / `apiFindings` / `status` 与报告版本逐字不变 |
| 画布 / 门禁 / 契约面板 | 零感知：本组哈希不入报告、不入 DTO、不入知识文本 |
| `.dev-team/architecture/**` 与四类生成物 | 零字节变化：`api:check` 零差异、`api:export` 零 diff |
