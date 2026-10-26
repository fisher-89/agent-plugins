# repo-surface-scope Specification (Delta)

## ADDED Requirements

### Requirement: 语料只含存活能力

`openspec/specs/` SHALL 只含描述**当前存活代码**的 capability：

- 每个 capability 目录 SHALL 恰有一份非空 `spec.md`；MUST NOT 存在 0 字节 `spec.md`、缺失 `spec.md` 的 capability 目录或空目录；
- 资产（源码 / 命令 / 工具 / 交付产物）整体退役时，描述该资产的 capability SHALL 在**同一变更内整块下线**（目录删除），MUST NOT 以「保留历史留痕」为由留下空壳占位——历史由 `openspec/changes/archive/**` 承载，不由主基线承载；
- capability 去留判定 SHALL 以**代码落点**为准，MUST NOT 以命名前缀一刀切：命名未带产品前缀但描述存活实现的 capability SHALL 视为存活（`workspace-layout-resolution` 是既有先例——它是 foundation crate 的 Layout / resolve 活 spec）；
- 新增 capability SHALL 对应一份可指认的存活实现或一条可验证的仓库面不变量；MUST NOT 为「可能要做」立空 spec。

#### Scenario: 资产退役后无残留 capability

- **WHEN** 某资产的源码与交付产物被整块删除
- **THEN** `openspec/specs/` 下不再存在描述该资产的 capability 目录（含只讲其文档约定、只讲其打包链路的 capability）
- **AND** 该资产的退役理由与删除清单在对应 change 的 `proposal.md` 在案可考

#### Scenario: 无空壳

- **WHEN** 执行 `find openspec/specs -type d -empty` 与 `find openspec/specs -name spec.md -size 0`
- **THEN** 两条命令均无输出；每个 capability 目录的 `spec.md` 至少含一条 requirement

#### Scenario: 前缀不是去留判据

- **WHEN** 判定命名未带 `desktop-` 前缀的 capability 去留
- **THEN** 以代码落点为准（实现存活则留、资产已删则删），MUST NOT 因「无前缀」直接判为退役

### Requirement: 仓库根文档与对外通道只描述存活资产

仓库根文档 SHALL 以**当前存活产品**为主叙事：`README.md` / `AGENT.md` / `CLAUDE.md` MUST NOT 以已下线资产的安装手册或架构叙事为主体；对外分发通道（marketplace 清单文件）SHALL 只登记存活产物。

- 资产下线时 SHALL 同批重写或删除其安装 / 使用 / 构建段落，MUST NOT 留指向已删路径的安装命令、仓库 URL 或产物名；
- marketplace 清单 SHALL 与产物目录同步：产物整块删除时登记项 SHALL 同批移除；无任何可分发产物时 SHALL 整文件删除，MUST NOT 留 `plugins: []` 空壳（空市场即坏市场）；
- 剩余文档 SHALL 能让新协作者在不读历史 change 的前提下完成「安装 / 开发 / 构建 / 发版」四件事。

#### Scenario: 文档面无已下线资产主体叙事

- **WHEN** 读取 `README.md` / `AGENT.md` / `CLAUDE.md`
- **THEN** 主叙事为当前存活产品（桌面端），含 dev / build / 发版入口
- **AND** 无已下线资产的安装步骤、插件市场安装命令或「以插件架构为主」的仓库定位叙述

#### Scenario: 市场登记项与产物同步

- **WHEN** 仓库已无任何可分发产物
- **THEN** marketplace 清单文件不在案（整文件删除），MUST NOT 留登记项指向已删的相对路径

### Requirement: 活跃面无指向已删资产的路径引用

仓库「活跃面」SHALL NOT 含指向已删资产的路径引用。活跃面含：产品源码（含注释）、CI 工作流、`openspec/config.json`、主 spec（`openspec/specs/**`）、仓库根文档与 `.gitignore`。

- 改写通道 SHALL 明记：`openspec/config.json` 位于写保护的内置锁定面（`**/openspec/config.json` 由 `rules::builtin_rules` 派生、恒在规则首位、fail-closed、不可配置不可移除），故其指针改写 SHALL 限**值级替换**（改动恰 2 行，行尾与其余字段字节零改）；MUST NOT 为实现「零命中」观感给内置规则加 override 或改动 `protect/**` 任何代码——保护面 SHALL 零代码改动；
- 指针改写 SHALL 有**逐值清单**在案：给出「位置 / 旧值 / 新值」与最小改动改法（值级替换、行尾与其余字段字节零改），清单 MUST 在该 change 目录内可考；MUST NOT 以「把该文件移出核对面」代替清单；
- 改写 SHALL 以双段判据复核：**改写后**判「旧资产路径零命中 + 指针可达」；**改写前**（指针仍为旧值时）判「命中数恰等于清单行数且逐行逐值对应」。MUST NOT 把该文件排除出「活跃面零悬空」判据，也 MUST NOT 为实现相位「全绿」观感而改动产品保护语义（如给内置规则加 override）；
- 指针 SHALL 恒可达：`$schema` 值 SHALL 以指向仓库内现存镜像路径的 raw URL 结尾（尾路径与仓库路径逐字同形），`write_protection.files[].glob` SHALL 命中现存文件且未被忽略——两者悬空即静默失效（`$schema` 悬空静默失去编辑器补全，glob 悬空静默失去用户规则）；
- `openspec/changes/archive/**` 与 `openspec/explores/**` SHALL 原样保留（历史事实，含已删资产路径引用属考古内容），MUST NOT 为「grep 干净」改写历史；
- 测试 fixture 中作为**数据样本**出现的旧路径（语料 JSON、glob 匹配样本）SHALL 豁免——它们不构成依赖。

#### Scenario: 全量 grep 无悬空引用（活跃面）

- **WHEN** 对活跃面 grep 已下线资产的源码路径前缀（排除 `openspec/changes/archive`、`openspec/explores`、进行中 change 目录、`openspec/specs`、语料 fixture 与 `golden`、以及以旧路径为样本的 glob 测试文件；`openspec/config.json` **不入排除面**）
- **THEN** 命中数为零（源码注释、CI 配置、根文档、`.gitignore`、`openspec/config.json` 与 desktop 产品面同属检索面）
- **AND** 每个排除项都有在案授权：历史事实 / 过程产物 / 归档相位判据 / 数据样本

#### Scenario: 配置指针零命中与可达

- **WHEN** 对 `openspec/config.json` grep 已下线资产的路径前缀
- **THEN** 命中数为零，两处指针（`$schema` / `write_protection.files[0].glob`）指向仓库内现存目标
- **AND** 改动面恰为这两行值（其余字段与行尾字节零改），`protect/**` 零代码改动
- **AND** 若某个副本内指针仍为旧值，同一命令的命中数应恰等于逐值清单行数且逐行逐值对应（改写前后的双段判据）

#### Scenario: 配置指针可达

- **WHEN** 读取 `openspec/config.json` 的指针
- **THEN** `$schema` 指向的 schema 镜像在仓库内现存
- **AND** `write_protection.files[].glob` 逐条命中现存文件

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `openspec/specs/` | capability 主基线 | 只含存活能力；每目录恰一份非空 `spec.md`；退役能力整目录删除，MUST NOT 留空壳或 `TBD` 占位 |
| `openspec/changes/archive/**` · `openspec/explores/**` | 历史事实 | 原样保留，MUST NOT 为引用清理而改写 |
| `README.md` · `AGENT.md` · `CLAUDE.md` | 仓库根文档 | 以存活产品为主叙事；含安装 / dev / build / 发版四段最小集 |
| `.claude-plugin/marketplace.json` · `.cursor-plugin/marketplace.json` | 对外分发登记 | 只登记存活产物；无可分发产物时整文件删除，MUST NOT 留空壳 |
| `openspec/config.json` | 仓库面配置（写保护内置锁定面） | `$schema` 与 `write_protection.files[].glob` 恒指向现存文件；改写限值级替换（恰 2 行、其余字节零改），保护面代码零改动 |
| change 目录的「配置指针」表 | 指针改写与复核 | 逐值给出旧值 → 新值 + 最小改动改法（值级替换、其余字节零改）+ 复核命令（`grep` 零命中、`git diff` 恰 2 行）；MUST NOT 省略复核 |
| `.gitignore` | 忽略规则 | MUST NOT 保留指向已删产物目录的忽略条目（死条目） |
