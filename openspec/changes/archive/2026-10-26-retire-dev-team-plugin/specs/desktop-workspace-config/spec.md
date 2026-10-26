# desktop-workspace-config Specification (Delta)

## MODIFIED Requirements

### Requirement: 配置解析 core crate

Rust 侧 SHALL 新增 crate `crates/core/config`（裸名 `config`），承载工作区 `config.json` 的读取、解析、校验与默认值填充：

- 依赖 SHALL 仅 `foundation`（workspace 内，取 config.json 路径）+ `serde` / `serde_json` / `specta`；MUST NOT 依赖 `workflow` / `agent` / 任何 infra crate，MUST NOT 依赖 Tauri；
- 入口 API SHALL 为 `load(root) -> ConfigReport`（函数签名形态 design 定夺），`ConfigReport` SHALL 为 `{ config, diagnostics }` 信封：`config` 为**永远合法**的完整配置（违例字段以默认值填充），`diagnostics` 记录违例细节（哪项非法、哪些字段吃了默认、文件缺失等）；
- 校验规则 SHALL 以 `core/config` **现行实现为权威**（`KNOWN_KEYS` 七项键集、`TestFramework` 八值枚举、数值阈值 0–100 值域、`tests[].root` 非空且 MUST NOT 含 glob 通配符（`*` `?` `{` `[`）、`schema` 字段字面量 `spec-driven`、顶层未知字段 passthrough 保留）：原对齐源（CLI zod `config.schema.ts`）已随插件退役删除，MUST NOT 再作为权威或对齐断言的目标；机器可读镜像见「`config.schema.json` 静态镜像与实现一致性」requirement；
- 默认值 SHALL 为：coverage lines 80 / branches 70 / functions 75、mutation score 70、suite `cwd` `.`、`tests` `[]`、`schema` `spec-driven`——数值以本 spec 与 `core/config` 实现为权威，原对齐源（CLI `defaults.ts`）已随插件退役删除，MUST NOT 再出现「对齐 `defaults.ts`」类断言或注释；
- CLI `readConfig` 的「非法即静默整体回默认」吞错语义 SHALL NOT 复刻：任意输入（缺失文件、坏 JSON、字段非法）MUST NOT 使 `load` 失败收场——产出合法 config + diagnostics；非法输入产出的 config SHALL 能通过同一套校验（自洽不变量）；
- config.json 路径 SHALL 经 `foundation::layout` 的路径解析取得，crate 内 MUST NOT 出现 `config.json` 文件名字面量（隔离执法见 desktop-crate-layout 增量）。

#### Scenario: 校验权威单源

- **WHEN** 审查 `core/config` 的校验规则来源（实现常量、注释、测试断言）
- **THEN** 权威只指向本 spec 与 `core/config` 实现（含 `config.schema.json` 镜像的双断言）
- **AND** 无「逐条复刻 CLI zod schema」「默认值对齐 CLI `defaults.ts`」类断言或注释残留（参照物已删除）

## ADDED Requirements

### Requirement: `config.schema.json` 静态镜像与实现一致性

`crates/core/config` SHALL 在 crate 内自持机器可读 schema 镜像，作为编辑器 `$schema` 补全的唯一来源：

- 落位 SHALL 为 `crates/core/config/config.schema.json`，与校验实现（`KNOWN_KEYS` / `TestFramework`）物理相邻；内容 SHALL 为迁入前的逐字副本（迁出后**手工维护**，不再由任何代码生成）；
- 镜像 SHALL 由同 crate 的一致性断言守护（两断言恒在，缺一即视为漂移无人拦）：`KNOWN_KEYS` 七项键集 == `schema.properties` 键集；`TestFramework` 八值枚举 == `tests.items.properties.framework.enum`。断言 SHALL 经 `include_str!` 或等价 crate 内读取路径取得镜像，MUST NOT 为此在实现侧新增导出；
- 镜像 MUST NOT 参与 tauri bundle 打包（编辑器消费走 GitHub raw URL），MUST NOT 改变包体积；
- 镜像 MUST NOT 破坏 `desktop-crate-layout` 的隔离禁令（产品 `.rs` 的 `config.json` 字面量禁令与 `openspec` 目录名禁令照旧；镜像只由 `*_test.rs` 读取）；
- 引用方 `$schema` SHALL 指向仓库内该路径的 raw URL（`…/master/packages/desktop/src-tauri/crates/core/config/config.schema.json`），MUST NOT 保留指向已删插件 `bin/` 目录的旧 URL；唯一引用方 `openspec/config.json` 的值 SHALL 按逐值清单（位置 / 旧值 / 新值，见该 change 的 design 与 tasks）以值级替换回填，回填后 MUST NOT 含旧资产路径（不变量见 `repo-surface-scope`）——本 crate 侧的可执行职责是「镜像在案且逐字一致」与「目标 URL 尾路径与仓库内现存路径同形」。

#### Scenario: 双断言拦住漂移

- **WHEN** 增删 `KNOWN_KEYS` 一项或增删 `TestFramework` 一个变体而未同步 `config.schema.json`
- **THEN** 同 crate 的一致性断言失败并指认漂移方向（键集差异 / 枚举差异）

#### Scenario: 镜像与实现现状对齐

- **WHEN** 现状下运行 `core/config` 测试
- **THEN** 两断言通过（七项键集逐字相同、八值枚举逐字相同），镜像内容与迁入前逐字一致

#### Scenario: 编辑器消费 URL 的落点与回填

- **WHEN** 读取 `openspec/config.json` 的 `$schema`，或核对该 change 的指针目标值
- **THEN** 目标 URL 尾路径为 `packages/desktop/src-tauri/crates/core/config/config.schema.json`，与仓库内现存镜像路径逐字同形
- **AND** 无指向插件 `bin/` 目录的 URL 在案；该文件内的两处指针 SHALL 由值级替换回填（改动限值级、行尾与其余字段字节零改），MUST NOT 为回填改动写保护内置规则语义

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/config/config.schema.json`（新） | 机器可读 schema 镜像 | 迁入前逐字副本；手工维护；不参与 tauri bundle；只由 `*_test.rs` 读取；顶层 `properties` 键集与 `tests.items.properties.framework.enum` 为双断言锚点 |
| `crates/core/config/src/lib.rs` | 校验权威实现 | `KNOWN_KEYS[7]` 与 `TestFramework` 八值为权威源；常量注释去掉「CLI zod / `defaults.ts` 对齐」措辞（语义零改） |
| `crates/core/config/src/lib_test.rs` | 一致性守护 | 新增两断言（键集 / 枚举）读 crate 内镜像；不新增生产导出 |
| `openspec/config.json` | 仓库面配置（写保护内置锁定面） | `$schema` 改指 `packages/desktop/src-tauri/crates/core/config/config.schema.json` 的 raw URL；回填 SHALL 为值级替换（逐值清单 + 复核命令，见 `repo-surface-scope`），MUST NOT 为回填改动内置锁定语义；`write_protection` 见 desktop-write-protection |
| 插件 `bin/dev-team-config.schema.json` | **（删除）** | 随插件整目录退役；生成源 `build/build-config-schema.ts`（`configSchema.toJSONSchema()`）同步消失，schema 自此转手工维护 |
