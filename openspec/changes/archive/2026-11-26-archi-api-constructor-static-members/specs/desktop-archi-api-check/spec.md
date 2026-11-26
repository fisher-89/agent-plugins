# desktop-archi-api-check Specification（delta: archi-api-constructor-static-members）

## Purpose

本 delta 只改**一个判据值的语义口径**，不动词汇表：

- `apiConstructorDeclared` 由「接口的 `properties` 声明了构造器」收窄为「**模块级** `properties` 声明了构造器名」——类型级构造入口从本期起有正规形态（`$defs.<T>.x-constructor`，见 `desktop-archi-api`「类型的构造入口与静态方法」），它 MUST NOT 触发本条判据。
- 词汇面零变化：`ArchiApiFindingKind` 仍**十值**、`ALL` 长度仍 10、`category` 仍七值（`constructor` 组仍是 `apiConstructorDeclared` 的归属）、`severity()` 与 `REPORT_VERSION` 逐字不变。
- 求值序零变化：`api = declared` 支的 ⑤⑥ 两步、首错短路与 pair 锚点序逐字沿用；新增的 `$defs` 成员比对与「至少一条构造通路」判定**不在本期**（后续 change）。
- 零 fixture 迁移：在案 11 份 schema 零 `x-class`、零 `x-constructor`/`x-static`，故 `apiFindings` 的条数与构成不因本变更变化。

## MODIFIED Requirements

### Requirement: 契约判据词汇封闭十值与开放分类

契约判据 SHALL 归入**新平行封闭枚举** `ArchiApiFindingKind`（**十值**，线格式 camelCase），MUST NOT 并入既有 `ArchiDriftKind`（四值）：

| 枚举值 | 线格式词 | 语义 |
|---|---|---|
| `ApiEntryMissing` | `apiEntryMissing` | **该元素没有可用入口**：无候选（未声明 `entry` 且无语言约定默认）**或**候选被判不可用（形态非法 / 落在元素 `path` 之外 / 文件不在盘上 / 与另一元素同一入口）。解析链与四条判据见 `desktop-archi-api`「生成物落点与入口查找」 |
| `ApiMissing` | `apiMissing` | **有约定无兑现**：接口定义的 `properties` / `$defs` 声明的 op / 类型在代码里不存在 |
| `ApiUndeclared` | `apiUndeclared` | 代码有 `external` 面但接口未声明（**唯一完备性判据**，档位 `off`） |
| `ApiSignatureMismatch` | `apiSignatureMismatch` | 接口签名与代码签名（归一后）不一致 |
| `ApiUnexpected` | `apiUnexpected` | 留白被违反（**单形态**）：`api = none` 而 `<叶段>.schema.json` 在盘 |
| `ApiShadowing` | `apiShadowing` | 父 `$defs` 与子元素 `$defs` 类型名交集非空 |
| `ApiRefMissing` | `apiRefMissing` | 跨模块 `$ref` 目标缺失 / 目标未 `declared` / 非直接子元素 |
| `ApiNestedCrossCrate` | `apiNestedCrossCrate` | 嵌套元素父子分属不同 crate |
| `ApiConstructorDeclared` | `apiConstructorDeclared` | **模块级**接口的 `properties` 声明了构造器名（构造器由生成器产出：零字段 `object` 自动 `new()`、类型级走 `$defs.<T>.x-constructor`；`$defs` 内的构造入口 MUST NOT 触发本值） |
| `DocMissing` | `docMissing` | **文档未就位**（单成因）：派生的 `<archi_models_dir>/<分层目录…>/<叶段>.md` 不在盘，**与 `api` 状态、`doc` 字段都无关** |

- **三项删除的事实源口径（必须逐条说准，MUST NOT 读成「放宽」）**：

  | 删除值 | 它的事实源 | 为何消失 |
  |---|---|---|
  | `apiReexportDrift` | md 散文透传列表 vs `api-reexport` fence 键集 | md 无结构、fence 不再被读取 ⇒ 两侧事实都不存在 |
  | `apiDocMissing` | 接口 md 在盘但正文无 api fence | 接口真相换到 `.schema.json`，md 不再承载接口 ⇒ 「有文档无约定」不再是缺口形态 |
  | `docNotFound` | `doc` 字段声明指不到盘 | `doc` 字段惰性化（零读者、零判据、零纳管） ⇒ 无对象 |

  三项都是**词汇缩小**（事实源消失），不是标准降低；补位判据（`schemaMissing` / `schemaInvalid`）明属后续 change，MUST NOT 在本期以别名预铺。
- **`docMissing` 收成单成因**：旧的双成因（`declared` 缺 md / `none` 未声明 `doc`）与三值分工（`docMissing` = 还没写 / `docNotFound` = 声明写错 / `apiDocMissing` = 承诺未展开）随之作废——本值只判「派生 md 不在盘」。
- **`apiUnexpected` 收成单形态**：旧形态②（`api = declared` 而写了 `doc`）**删除**；`doc` 字段零读者，写了不报。
- **`.i` 生成物的存在 MUST NOT 作为入口的替代证据**（生成物路径是入口的函数）。
- **`api = none` 元素退出入口判据**：其求值链只剩 `apiUnexpected` → `docMissing` 两步，MUST NOT 报 `apiEntryMissing`。
- **`apiEntryMissing` 的四种不可用理由挤进同一个值**（形态 / 落位 / 存在 / 跨元素唯一），文案点名具体理由；MUST NOT 新增第 11 个值。
- **`apiConstructorDeclared` 的射程收窄到模块级 `properties`**：`$defs` 条目的 `x-constructor`（以及 `x-static`）是**正规形态**，MUST NOT 被判；`$defs` 条目的 `properties.new` 由装载期 `x-class` 预留名判据拦（`ApiError`，不是本判据）——两面分工：模块面容错面产 finding、条目面在写闸即死。
- **`ALL` 与 `as_str` / `severity` 三方对拍**：`ArchiApiFindingKind::ALL: [Self; 10]` SHALL 与 `as_str` 逐值对拍、与 `severity()` 的取值面逐值对拍；新增变体未补入 `ALL`（长度断言）即测试红。
- **`category` SHALL 仍为开放值集字符串（MUST NOT 设枚举、MUST NOT 设 `ALL`）**，取值集**仍是七值**——三处删除只收缩成员、不清空任何分组：`entry`（`apiEntryMissing` / `apiUnexpected`）/ `contract`（`apiMissing` / `apiUndeclared`）/ `signature`（`apiSignatureMismatch`）/ `reexport`（`apiShadowing` / `apiRefMissing`）/ `nesting`（`apiNestedCrossCrate`）/ `constructor`（`apiConstructorDeclared`）/ **`doc`（只剩 `docMissing`）**。MUST NOT 为「只剩一个成员的分组」做合并或改名（chip 由数据聚合，空组自然不渲）。
- **条目形状**：`ArchiApiFinding { kind, category, element, target, message, occurrences }` 逐字不变（`occurrences` 复用 `ArchiDriftOccurrence`）。

#### Scenario: 十值词汇三方对拍

- **WHEN** 遍历 `ArchiApiFindingKind::ALL`、`as_str` 与 `severity()`
- **THEN** 十项一一对应、线格式词唯一且与语料词一致；从 `ALL` 删掉一项、新增变体未补 `ALL`、或 `severity()` 漏一个变体时测试红；`ALL` 长度恰 10

#### Scenario: 三值离场且线面同步

- **WHEN** 审查 `bindings.ts` / `bindings.zod.ts` 的 `ArchiApiFindingKind` 联合
- **THEN** `"apiReexportDrift"` / `"docNotFound"` / `"apiDocMissing"` 三值零命中；其余十值逐字在场；两份产物的联合成员集逐字相同

#### Scenario: category 七值无空组

- **WHEN** 遍历十个判据值并取 `category_of`
- **THEN** 七组各自至少一个成员（`doc` 只剩 `docMissing`）；无第八值、无空串、无空组；`apiUnexpected` 仍落 `entry`；`apiConstructorDeclared` 仍落 `constructor`

#### Scenario: 与既有漂移词汇互不混用

- **WHEN** 审查 `ArchiApiFindingKind` 与 `ArchiDriftKind` / `ArchiCheckStatus` / `ArchiObservationKind`
- **THEN** 四组词汇各自单点定义、互不复用；`ArchiDriftKind::ALL` 仍四值、`match` 臂零改动

#### Scenario: none 元素不再报入口缺失

- **WHEN** 构造一个 `api = none` 且无可用入口（无 `entry`、目录型根无 `index.ts(x)`）的元素
- **THEN** 该元素零 `apiEntryMissing`；若其派生 md 不在盘则恰一条 `docMissing`

#### Scenario: 类型级构造器不产该判据

- **WHEN** 某 `api = declared` 元素的 `$defs.Store` 带 `"type": "object"` + `"x-class": true` + `"x-constructor"`（`params` 非空）与一条返回本类型的 `x-static`，而模块级 `properties` 无构造器名
- **THEN** 该元素零 `apiConstructorDeclared`、零 `apiMissing`（`$defs` 条目不是模块导出比对的对象）；`ArchiApiFindingKind::ALL` 仍十值、`category` 仍七值

#### Scenario: 模块级构造器名仍判

- **WHEN** 同一元素在模块级 `properties` 写 `new`（带 `x-function`），同时在 `$defs.Store` 写 `x-constructor`
- **THEN** 恰一条 `apiConstructorDeclared` 点名模块级 `new`（`$defs` 的构造入口不在射程）；文案给出路「类型级构造用 `$defs.<T>.x-constructor`」

## Module Contract

### Function: `crates/core/archi/src/api/check.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `constructor_declared` | 改（文案 / 口径） | 仍从 `contract.ops`（模块级 `properties` 投影）里找首个构造器名；文案由「构造器由生成器的 `$defs` 条目自动产出」改为「零字段 `object` 自动产 `new()`、类型级构造走 `$defs.<T>.x-constructor`」；零新增参数、零新增调用点 |
| `check` 两支求值序 / `category_of` / 其余九条判据 | 零改动 | `api = none` 支两步、`api = declared` 支四步、pair 锚点序、`category` 七值逐字不变 |

### API: 判据词汇与报告面

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchiApiFindingKind` | 零改动 | 仍十值、`ALL: [Self; 10]`、`as_str` / `severity()` 逐值不变；`category` 分组不变（`constructor` 仍归 `apiConstructorDeclared`） |
| `ArchiApiFinding` / `ArchiDriftReport.api_findings` / `REPORT_VERSION` | 零改动 | 条目形状与报告槽位逐字不变（本变更只改一个值的**语义口径**，不改形状与桶） |
| 纳管口径（`gating_findings` / `derive_status` / `passed`） | 零改动 | `docMissing` 恒 `warn` 且不纳管；`apiConstructorDeclared` 的档位与纳管面逐字不变 |
| `bindings.ts` / `bindings.zod.ts` | 零改动 | 零枚举成员变化 ⇒ `bindings:export` 零 diff |

### CLI / Component: 零改动面

| 项 | 变化 | 描述 |
|---|---|---|
| `crates/core/archi/src/drift/**` | 零改动 | 报告装配与状态推导逐字不变 |
| 契约面板 / 画布（`desktop-archi-contract-panel` / `desktop-archi-canvas`） | 零改动 | chip 按数据聚合，`constructor` 组仍非空；零 UI 变化 |
| `tests/corpus/drift/**` 金样 | 零改动 | 在案语料零 `x-class` / 零新键 ⇒ `apiFindings` 条数与构成不变（落地时以实测复核） |
