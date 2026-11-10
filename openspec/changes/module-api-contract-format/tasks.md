# 任务: module-api-contract-format

> **本稿（第二稿 · 四条设计面裁定的承接稿）**：design 侧已按 **D58 / D59 / D60** 三条裁定收口（外加 D57 的文本口径同源项），本稿把三条裁定的**实现面交接**落成阶段三十，并把**测试面改动的归属**逐条写给测试相位（`test-design.md` 的三字段改写归测试相位，本相位不代写）。**封闭数逐值不变**：清单 **40 行 / 42 路径**、机械随动白名单 **18 路径**、实现相位封闭数 **60 路径**、测试相位 **12 组 / 44 文件**、交付态并集 **104 路径**——阶段三十只改四处**已在清单内**的产品文件（`api/emit/mod.rs` / `api/mod.rs` / `api/emit/rust.rs` 与被删函数的调用形态），**零新增文件**。
>
> **本稿（收敛稿）**：把「封闭数」从**三条互相打架的断言**收成**一条等式**，并补齐阶段二十九（D54 / D55 / D57）。实测重算后的现行口径只有一组：清单 **40 行 / 42 路径**（新增 18 + 修改 22，修改侧展开 24）+ 机械随动白名单 **18 路径** = 实现相位封闭数 **60 路径**；测试相位 **12 组 / 44 文件**；交付态并集 **104 路径**。旧稿的 41/48/66/13/47/113 一律作废（其中 113 是 `bash.rs` 双计导致的算错值）。**六份 `sdk/**` 完全出范围**（`HEAD` 无 rustfmt 债务，实测 6/6 干净）——不在修改表、不在白名单、F 判据零例外。`proposal.md` 不改。
>
> 覆盖 proposal「变更范围 - 实现文件」表 **27 行** + 设计补入 **5 项**（`crates/core/archi/src/lib.rs` 模块声明与出口、`crates/core/archi/src/api/emit/mod.rs` 子模块与载元、`crates/core/archi/src/api/common.json` 映射规则资产、`crates/infra/archi/src/report.rs`（试点 op 的实际来源）、`src-tauri/Cargo.lock`（workspace 成员自动随动））= **新增 18 行 / 18 路径 + 修改 22 行 / 24 路径 = 清单 40 行 / 42 路径** + **机械随动白名单 18 路径** = **实现相位封闭数 60 路径**；测试相位产出白名单 **12 组 / 44 文件**；交付态并集 **104 路径**（与 design「变更清单 - 核对口径」段逐值相等；本文件 MUST NOT 另立数值）。
>
  > **读法（本文件全局，先读这条）**：数值口径的唯一来源是 design「变更清单」的**核对口径**段与「spec 措辞同步项」的**射程判据**段。**现行值只有一组**——清单 **40 行 / 42 路径**（18 新增 + 22 修改，修改侧展开 24 路径）+ 机械随动白名单 **18 路径** = 实现相位封闭数 **60 路径**；测试相位产出白名单 **12 组 / 44 文件**；交付态并集 **104 路径**。本文件 MUST NOT 另立数值；凡历史段落里出现的其他数字（`41` / `48` / `66` / `47` / `113` / `46` / `64` / `65` / `115` / `49 条` / `58` / `68` / `103` / `107` / `108` / `43 文件` / `44 行` 等）一律按**该轮成文时点的历史快照**读，只作留痕、不作判据。**`packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` 已按 D56 回滚到 `HEAD`**（`git checkout HEAD --` 一次，产品面零改动），它**不在**实现相位封闭集合内、也**已移出**测试相位白名单（回滚后零 diff）——判据是 `git diff HEAD -- 同文件` 为空。
>
> **本相位不写测试、不执行测试**：`crates/core/archi/src/api/*_test.rs` / `extract/*_test.rs` / `emit/*_test.rs`、`crates/core/archi/src/drift/report_test.rs`、`crates/core/archi/src/scan/owner_test.rs`、`crates/core/archi/src/{parse/toml_test.rs,model_test.rs,dto_test.rs}`、`crates/core/archi/tests/{vocabulary_test.rs,golden_test.rs,pilot_test.rs,corpus/**}`、`crates/core/foundation/src/layout/mod_test.rs`、`crates/tools/archi-api-gen/src/main_test.rs`、`crates/infra/agent/src/archi_knowledge_test.rs`、`crates/infra/archi/src/{runner_test.rs,report_test.rs}` 的用例与装置全部归 test-design / test-gen / test-execution 相位。`vp test` / `cargo nextest` 一律不由本相位外跑。
>
> **例外（已有落地，逐条登记，不得据此扩用）**：D55 / D58 / D59 / D60 的用例面**已由测试相位落地在工作树**，故本文件对下列用例只作**验收登记、不作实现相位授权**——`api/mod_test.rs`（`display路径归一语义` / `display边界值` / `display跨crate单点面` / `render_all的emit_target三值构造` / `api出口面归零且判据可判红`）、`api/check_test.rs`（`category_of不经api出口面`）、`api/contract_test.rs`（`choice_payload词面零残留`）、`api/emit/mod_test.rs`（`render_target的emit_target三字段面` / `render_target删两字段零行为影响` / `render_target的emit_target三字段承载`）、`api/emit/rust_test.rs`（`render的oneof三类分支均带负载` / `render的oneof兜底分支仍落占位变体` / `choice_payload零残留且单通路`）、`api/emit/ts_test.rs`（`三字段只读两项`）、`crates/infra/agent/src/sdk/bash_test.rs`（`resolve_timeout_ms可见性提升且实现体零改动`）。这些文件与新用例均在测试相位 44 文件集内计数，**不增实现相位封闭数**（60 路径不变）；上文「不写测试」的射程是**本相位不得再新增用例**，不是否认这批已落地用例的存在。
>
> 本相位只为**可编译**承担**四类**机械随动（逐文件清单位于 design「机械随动白名单」，表体共 **18 个测试文件**；MUST NOT 把任何产品文件塞进这一面）：
> ① 既有后端测试文件里 `Element { … }` / `ArchiElement { … }` 字面量补 `api: ApiStatus::Declared`（8 个文件各一处工厂函数或字面量，**不新增任何 `#[test]`**）；
> ② `ArchiDriftReport { … }` 字面量补 `api_findings: Vec::new()`——**测试侧 2 个文件 3 处**（`drift/report_test.rs` 的 `full_report()` / `empty_report()` + `crates/infra/archi/src/report_test.rs` 的 `empty_report()`），**产品侧 1 个文件 2 处**（`drift/mod.rs` 的 `check()` 与 `skipped_report()`，属清单修改表）；同批补齐否则 E0063；
> ③ `bindings:export` 之后前端 `packages/desktop/src/views/architecture/**` **8 个** `*.test.ts(x)` 的既有字面量补字段（`ArchiElement.api` / `ArchiModuleDetail.api` / `ArchiDriftReport.apiFindings`），否则 `vp check` / `tsc` 全红（逐文件见阶段十五）；
> ④ **第四类 = 「被删符号的调用形态适配」**（design **D53**）：`emit` 出口收为四参单形后，既有 sidecar 用例的调用是硬编译错误（E0061 / E0433），而 `server:check`（`--all-targets`）要求工作树可编译 ⇒ 该适配是**编译前置**。**射程封闭为三种形态**：`use` 行改指 / 实参补齐（元数，新增实参**只能取既有装置** `empty_target()` / `empty_scope()`）/ 装置同名提取；**MUST NOT 含**断言值 / 断言意图 / 用例名 / 新增或删除用例 / 夹具语义。落点恰两个文件（`api/emit/rust_test.rs` / `api/emit/ts_test.rs`），**不增白名单项**（两路径已在测试相位 44 文件集内计数，收进即双计）。**归属按改动种类划、不按文件划**，逐 `hunk` 判。
> 另有一条非文件级约定：新模块的 `#[cfg(test)] mod *_test;` 声明**暂不写**（测试相位建文件时同批加声明，避免留下不可解析的 mod 声明）。
>
> **核对封闭数（收尾自查 F 按此执行）**：允许改动集合 = 变更清单 **42 路径** + 机械随动白名单 **18 路径** = **60 路径**；测试相位产出白名单 **44 文件**；交付态并集 **104 路径**（并集 = 60 ∪ 44，交集为空）。核对命令（cwd = 仓库根）：
>
> ```bash
> # ① actual：工作树相对 HEAD 的全部改动（排除本变更目录本身）
> git status --porcelain -uall | grep -v '^.. openspec/changes/module-api-contract-format/' | sed 's/^...//' | LC_ALL=C sort -u > /tmp/actual.txt
>
> # ② allowlist：清单 42 + 机械随动白名单 18 + 测试相位 44 = 104 行，逐行物理全路径（零省略、零 `…`）
> # 生成规则见 design「核对口径 - 路径归一口径」条；两侧 MUST 同为仓库根相对物理路径
> LC_ALL=C sort -u /tmp/allowlist.txt -o /tmp/allowlist.txt
>
> # ③ 主判据：双向差集，MUST 零行
> comm -3 /tmp/allowlist.txt /tmp/actual.txt
>
> # ④ 反向自检：MUST 零行（清单内每一项都应在工作树有改动）
> comm -23 /tmp/allowlist.txt /tmp/actual.txt   # MUST 零行.rs
> wc -l /tmp/allowlist.txt                      # 快速自检：104
> ```
>
> **判据方向**：`comm -3` 的两栏语义相反——**左栏（allowlist 有、actual 无）= 清单内未改**（MUST 零行）；**右栏（actual 有、allowlist 无）= 清单外改动**（MUST 零行）。
> **零例外**：旧稿因为「六份 `sdk/**` 留在修改表、但在 `HEAD` 已清白」而必须留「恰六行」的合法例外，那是把两条互斥断言拼在一起的产物。六份经实测零债务，**已完全出范围**（不入清单、不入白名单、不入测试相位）⇒ **两侧都必须是零行**，判据不再有任何例外分支。
>
> **路径形态铁律（判据共用，防同型伪判据）**：`/tmp/allowlist.txt` 内**零 `…` 缩写、零前缀省略**，行内容与 `git status --porcelain -uall` 的路径列**逐字可比**（Rust 侧带 `packages/desktop/src-tauri/` 前缀）。推论：**凡以某文件是否在集合内为断言的判据串，MUST 取物理全路径**——`src/…` / `crates/…` 省略形态在该集合里必然 0 命中，读到 0 时**不得**据此断言「不在集合内」（那是伪判据，不是证据）。
>
> **依赖序要点**：**阶段一**（foundation 路径派生）与**阶段二**（`ApiStatus` + `api` 字段 + 投影三处出线）无相互依赖，可并行，共同解锁阶段三；**阶段三**（路径单点）解锁阶段四~七与阶段九~十；**阶段四**（契约解析）解锁阶段五（`schema` 装配）与阶段六（渲染）；**阶段六**解锁阶段九（`render_all`）；**阶段七**（取数器）与阶段五共同解锁阶段九与阶段十（十判据）；**阶段八**（报告类型与槽位）是**阶段十与阶段十一的类型前置**；**阶段九**（`api/mod.rs` 的类型与出口）解锁阶段十与阶段十一；**阶段十二**（归属剥离）独立可并行；**阶段十三**（工具 crate + workspace 接线）需阶段六与阶段九落地才能编译；**阶段十四**（试点资产与代码形状）需阶段十三；**阶段十五**（脚本面 + 绑定再生成）需阶段二、八、十三；**阶段十六 / 十七**（模型数据资产 / 知识文本）分别依赖阶段十三与阶段二；**阶段十八**收尾。**阶段十九~二十二**为各轮回溯的收口阶段（不新增文件）；**阶段二十三~二十八**为「上游文本与实现不可调和」的集中收口（D43~D53），必须在阶段四~十落地之后开工。**阶段二十九**（**本稿新增**，D54 / D55 / D56 / D57）依赖阶段六（两侧渲染器在案）、阶段九（`ApiMeta` 在案）与阶段十三（工具 crate 在案），且必须在阶段二十四（出口单形）之后——它改的是渲染器的**映射取数面**与工具侧的**重复实现面**，与出口形态互不重叠。**阶段三十**（**本稿新增**，D58 / D59 / D60）依赖阶段六（两侧渲染器在案）与阶段九（`render_all` 与出口面在案），且与阶段二十四 / 二十九不重叠（后者收出口单形与映射查表面，前者收字段面、删第二份映射、删出口冗余）。**阶段一~三十之间无环依赖**；「阶段二十」的 spec 文本项（S-1…S-17）与实现相位无编译期依赖，但 MUST 在归档前完成 delta 十五条、并在归档 SpecSync 步完成主基线两条（S-4 / S-5）。
>
> 中间态约定：阶段二让五个类型各长一个字段，全部字面量构造点同阶段补齐（否则 `cargo check` 全红）；阶段四~十的每个新模块**落地即声明**；阶段八让 `ArchiDriftReport` 长一个字段，构造点同批补 `api_findings: Vec::new()`（只求可编译，真实装配留阶段十一）；阶段十五之前 `api:check` 脚本不存在，MUST NOT 在前端引用任何新生成的 TS 类型。
>
> 自查命令 cwd 除特别标注外均为 `packages/desktop/src-tauri`；前端自查 cwd 为 `packages/desktop`；git 自查 cwd 为仓库根。**`cargo run -q -p archi-api-gen -- …` 系列对 cwd 不敏感**：root 由工具侧 `repository_root()` 向上锚定（design D32），故从任一 cwd 起手都判同一个仓；反之 **MUST NOT** 为它加 `cd` 前缀或在源码里写路径字面量。
>
> 不预铺：无 db 迁移 / 新表、无 `config.schema.json` 与 `KNOWN_KEYS` 变更、无 `openspec/config.json` 新键、无 `tauri.conf.json` / `capabilities/default.json` 变更、无 `crates/core/workflow/src/prompts/**` 改动、**`crates/infra/agent/src/sdk/**` 零改动**（六份完全出范围；唯一例外是 `bash.rs` 的 `pub(crate)` 可见性提升，进清单修改表）、无新 IPC 命令、无新前端路由、无 `.dev-team/architecture/config.json`。

## 阶段零：把 fmt 门改为校验模式（消 I6 根因，最先执行）

> 旧稿把它写成「周期性回滚（`git checkout --`）+ 每次门后重做」，该动作在写模式门下**不可能收敛**。本阶段按 design D28 / D30 改成「改门为校验模式」，使树进入**不动点**。

- [x] 把 `packages/desktop/package.json` 的 `server:check` 改成校验模式：`"server:check": "cargo fmt --all --check --manifest-path src-tauri/Cargo.toml && cargo clippy --workspace --all-targets --manifest-path src-tauri/Cargo.toml"`（**只把 `cargo fmt --all` 换成 `cargo fmt --all --check`**，clippy 段逐字不动，D28）
- [x] 把 `packages/desktop/package.json` 的行尾归到 `vp fmt` 的不动点（LF）；**MUST NOT** 再手工回写 CRLF（D30）；索引态 `i/crlf` 保留不动
- [x] 自查 阶段零（**不动点判据**）：`cargo fmt --all --check --manifest-path src-tauri/Cargo.toml`（cwd `packages/desktop`）**exit 0**；`npx vp check`（cwd `packages/desktop`）**exit 0**；`git status --porcelain crates/infra/agent/src/sdk/{context,loop,loop_test,resume,runner_test,tools_test}.rs` **零输出**（六份零改动，是本阶段的不变量而非任务）；`git diff --numstat` 与 `--ignore-cr-at-eol` 对 `package.json` 两侧同值
- [x] 自查 阶段零（**收敛性，本阶段最重要的一条**）：连续执行两次 `pnpm run check` 后，`git status --porcelain -uall | grep -v '^.. openspec/changes/'` 的文件名集合两次**逐行相等**（`diff` 为空）。**行数不作判据**（观测值随 `HEAD` 状态滚动），判据只取文件名集合

## 阶段一：foundation 路径派生族

- [x] 修改 `crates/core/foundation/src/layout/mod.rs`：常量组追加 `const API_DIR_NAME: &str = "api";`（doc：模块 API 契约子目录名，相对架构域根锚定；契约资产与其生成物的路径段唯一持有者）
- [x] 同文件 `archi_config_path` 之后追加 `pub fn archi_api_dir(root: &Path) -> PathBuf`：`archi_dir(root).join(API_DIR_NAME)`（doc：契约资产的唯一合法定位函数，消费者 MUST NOT 行内拼 `api` 段；纯推导零 IO）
- [x] 同文件 `archi_decisions_rel_dir` 之后追加 `pub fn archi_api_rel_dir() -> String`：`format!("{}/{}", archi_rel_dir(), API_DIR_NAME)`（doc：契约目录的 repo 相对路径；`/` 分隔、零前导 `./`；消费者限于知识文本与 `INDEX.toml` 说明段）
- [x] 自查 阶段一（AC-19）：**用例面**指向 `openspec/changes/module-api-contract-format/specs/workspace-layout-resolution/spec.md` 的四条既有 Scenario——`archi_api_dir 与其他派生互不遮蔽`（:46）/ `archi_api_rel_dir 同源且前缀一致`（:82）/ `与绝对路径族尾部一致`（:72）/ `零 IO 与零字面量`（:77），其用例由测试相位落 `crates/core/foundation/src/layout/mod_test.rs`；**产品侧四条机械 `grep`**：`grep -c 'API_DIR_NAME' crates/core/foundation/src/layout/mod.rs` ⇒ **4**；`grep -rl 'API_DIR_NAME' --include=*.rs crates/ src/` 滤 `*_test.rs` 后 ⇒ **1 个文件**；`grep -rn '"api"' --include=*.rs crates/ src/` 滤 `*_test.rs` 后 ⇒ **1 处**；`grep -cE 'archi_api_dir|archi_api_rel_dir' crates/core/foundation/src/layout/mod.rs` ⇒ **2**（两函数签名行）

## 阶段二：`ApiStatus` + `api` 字段 + 投影三处出线

- [x] `crates/core/archi/src/dto.rs` 新增 `ApiStatus`（`Declared` 缺省 / `None`；`#[serde(rename_all="lowercase")]` + `specta::Type` + `Default` + `as_str` + `ALL: [Self; 2]`）与 `ArchiApiRef`（`status` / `contract_path` / `schema_path` / `generated_path` 四字段，camelCase）
- [x] 同文件 `ArchiElement` / `ArchiElementBrief` 各增 `pub api: ApiStatus`；`ArchiModuleDetail` 增 `pub api: ArchiApiRef`；`to_element` / `to_brief` 填纯拷贝，`module_detail` 填 `api::api_ref(root, fqn)`
- [x] `crates/core/archi/src/model.rs` 的 `Element` 增 `pub api: ApiStatus`（doc：契约状态指针，缺省 `declared`；MUST NOT 承载协议正文）
- [x] `crates/core/archi/src/parse/toml.rs` 的 `RawElement` 增 `#[serde(default)] api: ApiStatus`；`to_model` 拷贝该字段；`classify` 零改动（`deny_unknown_fields` 面自动生效 ⇒ 拼错报 `invalidValue`）
- [x] 同批补齐后端测试面八个文件的工厂 / 字面量（机械随动白名单 ①：`diff_test` / `docs_test` / `graph_test` / `model_test` / `search_test` / `dto_test` / `scan/mod_test` / `scan/owner_test`，**不新增任何 `#[test]`**）
- [x] 自查 阶段二：`grep -c "pub api: ApiStatus" crates/core/archi/src/{dto,model}.rs` ⇒ **3**（`dto` 两处 + `model` 一处）；`grep -c "pub api: ArchiApiRef" crates/core/archi/src/dto.rs` ⇒ **1**；`cargo check -p archi --all-targets` 零错误

## 阶段三：路径单点 `api/paths.rs`

- [x] 新增 `crates/core/archi/src/api/paths.rs`：`normalize_root` / `contract_path` / `schema_path` / `generated_path`（D42 收窄为 `<root>` 字符串）/ `generated_kind` / `entry_form` / `mod_path_attr` / `ts_import_specifier` / `ref_relative` / `domain_of` / `directory` / `index_path` / `module_name` / `rust_module_path` / `language_of` / `is_module_segment`（`pub(crate)`，D48 / D50）
- [x] 同文件两个私有形态判据 `is_repo_relative` / `is_fqn`（D33：绝对路径 / 盘符 / `..` 段 / 空段 / 空串一律 `None`）；`RUST_KEYWORDS` 常量（38 严格 + 13 保留）单点持有
- [x] `crates/core/archi/src/lib.rs` 增 `pub mod api;`（字母序在 `pub mod diff;` 之前）+ 阶段二的出口
- [x] 自查 阶段三：`grep -c "^pub fn " crates/core/archi/src/api/paths.rs` ⇒ **15**（**现算为准，MUST NOT 回写任何历史行数**；本文件在 design 公共函数表**不立行**——它是同 crate 面的路径派生内部件，见该小节纪律②）；`grep -cE "fn is_repo_relative|fn is_fqn" crates/core/archi/src/api/paths.rs` ⇒ **2**；`grep -c "const RUST_KEYWORDS" crates/core/archi/src/api/paths.rs` ⇒ **1**；文件内零 `"api"` 段字面量（`grep -cF '"api"' ⇒ 0`）

## 阶段四：契约 md 解析 `api/contract.rs`

- [x] 新增 `crates/core/archi/src/api/contract.rs`：`parse`（H1 锚点 + 散文区 + `### <op-id>` 分段 + 三类 fence；**重复键文本层扫描在 `from_str` 之前**，D37）/ `verify`（双向 gate 三条 + **七族名字闸** + **`x-function` 落点闸**，D45 / D47）/ `is_constructor`
- [x] 同文件 `fn is_identifier` 单点（七族共用）+ `fn function_param_names`（合法落点二处的收集面）
- [x] 自查 阶段四：`grep -c "fn is_identifier" crates/core/archi/src/api/contract.rs` ⇒ **1**；`grep -cF "fn duplicate_keys" crates/core/archi/src/api/contract.rs` ≥ **1**（两串分列计数，MUST NOT 用 BRE 交替）；`grep -c "fn function_param_names" crates/core/archi/src/api/contract.rs` ⇒ **1**

## 阶段五：`$defs` 规则与装配 `api/schema.rs`

- [x] 新增 `crates/core/archi/src/api/schema.rs`：`build_schema`（六字段；两处键空间唯一性**先判后写**，D35）/ `verify_defs`（五条规则 + 透传纯 `$ref` + `x-class` 带属性判错）/ `build_index`（FQN 字典序 + 说明段 + `[entry.categories]`）
- [x] 自查 阶段五：`grep -c "键空间唯一" crates/core/archi/src/api/schema.rs` ⇒ **2**（两处撞名错误文案各一处）；`grep -c "def_shape" crates/core/archi/src/api/schema.rs` ⇒ **0**（形状打标在 `api/mod.rs` 的 `def_shape`，D26）

## 阶段六：渲染器 `api/emit/**`

- [x] 新增 `crates/core/archi/src/api/emit/mod.rs`：`pub mod rust; pub mod ts;` + `EmitTarget` + `ModuleRef` + `render_target`（分派前发 `x-class` 警告，D27）
- [x] 新增 `crates/core/archi/src/api/emit/rust.rs`：`render`（类型 / 自动构造器 / 透传 `pub use` / 头注释 / 签名见证）、`rust_comment_text`、`variant_name`（**私有 `fn`**，D45 闸二的归一承接地，不入公共函数表）
- [x] 新增 `crates/core/archi/src/api/emit/ts.rs`：`render`（`interface` / `as const` + `type` / 别名 / 透传 `export type { } from` / 头注释）、`ts_string_literal`、`ts_comment_text`
- [x] 自查 阶段六：`grep -cF ".d.ts" -r crates/core/archi/src/api/` **零命中**（`-F` 锚固定串，MUST NOT 用反斜杠转义点号）；`grep -cF "eprintln!" -r crates/core/archi/src/api/emit/` **恰一处**（`emit/mod.rs` 的 `x-class` 警告）；`grep -cF '"' crates/core/archi/src/api/emit/ts.rs` 的产品面单引号形态符合 D29（**本阶段保持合法态**：`enum` 单引号裸插值恰 1 / `ts_string_literal(` 在 `enum` 分支为 0，判据由**阶段二十三 ⑦** 承接）

## 阶段七：取数器 `api/extract/**`

- [x] 新增 `crates/core/archi/src/api/extract/mod.rs`：`extract`（产品调用者为零，**显式留白**，D52）/ `extract_element`（判据唯一取数面）/ `normalize_signature` / **`Position` / `schema_type` / `signature_of`（唯一写出点，D43）** / `Visibility`（四值 + `ALL`，doc 写明留白）
- [x] 新增 `crates/core/archi/src/api/extract/rust.rs`：`extract_file`（三值判定）/ `extract_crate`（模块树 + `pub use` 白名单 + `#[path]` 三步限根与 `pub use <mod>::*` 通配升格）
- [x] 新增 `crates/core/archi/src/api/extract/ts.rs`：`extract_file`（**只抽导出面**：仅 `Statement::ExportDeclaration` 一条顶层分支；箭头函数不作操作；`export default` 留白）/ `apply_knip_report`（**本期未接线**，D39；模块 doc 写明三处留白缘由与将来所需输入）
- [x] 自查 阶段七（判据一律锚行首缩进，避免 doc 注释误计）：`grep -cE '^\s+Statement::' crates/core/archi/src/api/extract/ts.rs` ⇒ **1**；`grep -cE 'Expression::ArrowFunctionExpression'` ⇒ **0**；`grep -cE 'Visibility::(CrateOnly|TestOnly)' crates/core/archi/src/api/extract/ts.rs` ⇒ **0**；`grep -c '留白' crates/core/archi/src/api/extract/ts.rs` ≥ **1**；`grep -n 'proxy' crates/core/archi/src/api/extract/mod.rs` 命中禁令句；`grep -c "syn::parse_file" crates/core/archi/src/api/extract/rust.rs` ⇒ **3**；`grep -c "regex" crates/core/archi/src/api/extract/rust.rs` ⇒ **0**

## 阶段八：报告词汇与槽位

- [x] `crates/core/archi/src/drift/report.rs` 新增 `ArchiApiFindingKind`（十值 + `ALL: [Self; 10]` + `as_str` + `severity`）、`ApiFindingSeverity`（`Off` / `Warn` + `ALL` + `as_str`）、`ArchiApiFinding`（六字段）；`ArchiDriftReport` 末位追加 `api_findings`（既有 14 字段逐字不动）
- [x] `crates/core/archi/src/drift/mod.rs` 的 `REPORT_VERSION` 1 → 2；两处 `Report { … }` 字面量同批补 `api_findings: Vec::new()`（只求可编译）
- [x] 同批补齐测试侧三处字面量（机械随动白名单 ②：`drift/report_test.rs` 两处 + `crates/infra/archi/src/report_test.rs` 一处）
- [x] 自查 阶段八：`grep -c "ArchiApiFindingKind::" crates/core/archi/src/drift/report.rs` ≥ **10**；`grep -c "api_findings: Vec::new()" crates/core/archi/src/drift/mod.rs` ⇒ **2**；`grep -c "REPORT_VERSION: u32 = 2" crates/core/archi/src/drift/mod.rs` ⇒ **1**

## 阶段九：装配与出口 `api/mod.rs`

- [x] 新增 `crates/core/archi/src/api/mod.rs`：`ApiError`（三值 + `message`）/ `ApiSnapshot`（**零 `absent`**，D36）/ `GeneratedFile` / `ApiCheckInput`（六字段）/ `ApiMeta`（**含 `common`**）/ `CommonRule`
- [x] 同文件 `meta` / `enabled` / `load_contracts` / `render_all` / `bootstrap` / `api_ref` / `api_ref_of` 七个 `pub fn`（本阶段落的七个；`check` 在阶段十落下后**起点八行**齐备 ⇒ `grep -c '^pub fn '` **8**）→ 阶段二十九 ④ 升格后 `display` 成为**第九行**、D57 的 `protocol_key` 为 `pub(crate)` 不入导出面 ⇒ **终态 9 个 `pub fn`**（终态以 design「变更清单 - 新增文件」表 `api/mod.rs` 行的九行为唯一源；`grep -c '^pub fn '` 终态现算 ⇒ **9**、`grep -c '^pub(crate) fn '` 由 **5** 降为 **4**）+ `index_entries` / `categories_of` / `def_shape` / `module_refs` / `generated_path_of` / `parse_common` / `protocol_key` 私有工序（**私有工序一律不入 design 公共函数表**，二者同一定义面）。**计数口径留痕**：本行原写「⇒ **终态 8 个 `pub fn`**」，与 design 新增文件表的九行互斥；九行为终态、八行为起点，本行不得再自称终态值
- [x] 自查 阶段九：`grep -c "absent" crates/core/archi/src/api/mod.rs` **只命中 `ApiFormStatus::Absent` 与 doc 词面**（字段面与读取面零残留：`grep -cF "absent:"` 与 `grep -cF "input.absent"` / `grep -cF "snapshot.absent"` 各 ⇒ **0**，两串分列计数，MUST NOT 用 BRE 交替）；`grep -c "^pub fn " crates/core/archi/src/api/mod.rs` **起点 ⇒ 8、落地阶段二十九 ④ 后终态 ⇒ 9**（两值分属两个时点，MUST NOT 互相反推；终态与 design「变更清单 - 新增文件」表 `api/mod.rs` 行的九行逐名相等），且 `grep -c "^pub(crate) fn "` 起点 ⇒ **5**、终态 ⇒ **4**（`display` 出线）

## 阶段十：十判据 `api/check.rs`

- [x] 新增 `crates/core/archi/src/api/check.rs`：`check`（固定求值序与互斥，D16）+ `category_of`（六值单点打标）；① `apiEntryMissing` → ② `apiMissing`（**op 段与 `$defs` 类型段同口径**，D44；`occurrences.line` op 取 heading 行、类型取 `0`，D51）→ ③ `apiUndeclared`（`severity() == Off` ⇒ 跳过）→ ④ `apiSignatureMismatch`（经 `extract::signature_of` 与 `normalize_signature`，**零本地归一**，D43）→ ⑤ `apiUnexpected` → ⑥–⑩ 嵌套与透传五项
- [x] 自查 阶段十：`grep -c "ArchiApiFindingKind::" crates/core/archi/src/api/check.rs` 与十值核对（每个 kind 至少一处构造）；`grep -cE "enum Position|fn normalize_schema|fn contract_signature" crates/core/archi/src/api/check.rs` ⇒ **0**（`-E` 显式声明扩展正则，MUST NOT 依赖 shell 保留反斜杠）（D43）；`grep -c "normalize_signature(" crates/core/archi/src/api/check.rs` ⇒ **1**；`grep -cE "extract_crate|ts::extract_file" crates/core/archi/src/api/check.rs` ⇒ **0**

## 阶段十一：报告装配 `drift/mod.rs`

- [x] `check()` 在观察项之后、`status` 推导之前插 `let api_findings = api::check(ApiCheckInput { … });` 并写入 `Report { …, api_findings }`；`skipped_report()` 恒 `Vec::new()`；模块 doc 补「`api_findings` 不参与 `status` 推导」
- [x] 自查 阶段十一：`grep -cF "api::check(" crates/core/archi/src/drift/mod.rs` ⇒ **1**；`grep -cF "api_findings" crates/core/archi/src/drift/mod.rs` ≥ **3**（装配 / skipped / doc）；`derive_status` 未读该字段（源码审查：`api_findings` 在 `drift/mod.rs` 内只出现在装配与 `skipped_report` 两处，**MUST NOT** 用「两条 `grep` 以转义竖线串联」的管道形态当判据——该形态依赖 shell 保留反斜杠，跨解释器语义不同）

## 阶段十二：归属剥离 `scan/owner.rs`

- [x] `owner_of` 入参准备处增私有 `fn strip_generated(rel_path: &str) -> String`（`.i.<ext>` 五值 + 标记段 `GENERATED_MARKER`）；`normalize` / `prefix_matches` / `build` 零改动
- [x] 自查 阶段十二：`grep -cF "GENERATED_EXTENSIONS" crates/core/archi/src/scan/owner.rs` 与 `grep -cF "GENERATED_MARKER" crates/core/archi/src/scan/owner.rs` 各 ≥ **1**（常量各恰 1 处定义）；`grep -c "strip_generated" crates/core/archi/src/scan/owner.rs` ⇒ **2**（定义 + 调用）

## 阶段十三：工具 crate `archi-api-gen` + workspace 接线

- [x] 新增 `crates/tools/archi-api-gen/Cargo.toml`（裸名 `archi-api-gen`；workspace 内依赖恰 `archi` + `foundation`，外部 `serde_json`；注释写明六条零）
- [x] 新增 `crates/tools/archi-api-gen/src/main.rs`：`fn main() -> ExitCode`（argv → `parse_args` → `repository_root()` → `run`）、`pub(crate) fn run(command, root)`、`fn repository_root() -> PathBuf`（**向上取最近的含架构域祖先**，全不命中退化为 cwd，D32）；三子命令 `generate` / `--check`（零写入，D21 / D34）/ `bootstrap`（只出 stdout，不读 knip，D39）
- [x] 写盘面第二层限根：`fn write` 内 `file.path.starts_with(root)` 复核，越界即拒绝并零副作用（D33）
- [x] `src-tauri/Cargo.toml`：`[workspace] members` 增 `"crates/tools/archi-api-gen"`；`[workspace.dependencies]` 增 `archi-api-gen = { path = … }`（注释「CLI 工具，不被 app 依赖」）；壳 `[dependencies]` **零新增**；`src-tauri/Cargo.lock` 随动提交
- [x] 自查 阶段十三：`grep -cF "fn repository_root" crates/tools/archi-api-gen/src/main.rs` ⇒ **1**；`grep -cF "archi_dir(candidate).is_dir()" crates/tools/archi-api-gen/src/main.rs` ⇒ **1**；`grep -cF "api" crates/tools/archi-api-gen/src/main.rs` 的命中全为主体代码词面、**零路径段字面量**（零路径字面量判据改用三条 `-F` 分列：`grep -cF ".dev-team" 同文件` ⇒ **0**、`grep -cF "architecture" 同文件` ⇒ **0**、`grep -cF '"api"' 同文件` ⇒ **0**；MUST NOT 用 `-E` 的反斜杠转义引号形态）；`grep -cF "starts_with(root)" 同文件` ≥ **1**；在 `packages/desktop` 下跑 `cargo run -q -p archi-api-gen -- --check` 得 **0**（root 锚定可观测判据）

## 阶段十四：试点资产与代码形状（AC-13）

- [x] `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md`（H1 + `## 职能` / `## 不变量` + `### file_stem` + `api-op` + `api-types`（`LocalArchiCheck` 零字段 object））；**无 `### new`**；`compose_text` 不进契约（D23）
- [x] 跑 `cargo run -q -p archi-api-gen -- generate` 产出 `.schema.json` / `INDEX.toml` / `crates/infra/archi.i.rs` 三份生成物并入库
- [x] `crates/infra/archi/src/lib.rs` 加入口编排 `#[path = "../../archi.i.rs"] mod api_gen; pub use api_gen::*;`（**字面量 + 与 `paths::mod_path_attr` 输出逐字相等**，D38），删 `pub use runner::LocalArchiCheck;`，模块 doc 注释如实化
- [x] `crates/infra/archi/src/runner.rs` 删手写 `pub struct LocalArchiCheck;` 与 `impl` 内 `new()`（生成物接管；`impl Default` 保留并调 `Self::new()`）；`compose_text` 保持私有
- [x] `crates/infra/archi/src/report.rs` 删私有 `fn file_stem`（挪进 `impl LocalArchiCheck` 并提升为 `pub`，实现体逐字照搬）；`write_report` 调用点改 `LocalArchiCheck::file_stem(&report.timestamp)`
- [x] 自查 阶段十四：`rustfmt --edition 2021 --check crates/infra/archi.i.rs` **exit 0**（D29 不动点）；`grep -c "fn file_stem" crates/infra/archi/src/report.rs` ⇒ **0**；`grep -c "pub struct LocalArchiCheck" crates/infra/archi/src/runner.rs` ⇒ **0**；`grep -c "pub fn new" crates/infra/archi/src/runner.rs` ⇒ **0**；`cargo check -p archi-runtime` 零错误（E0255 未触发）

## 阶段十五：脚本面 + 绑定再生成 + 模型/知识文本数据资产

- [x] `packages/desktop/package.json`：增 `api:export` / `api:check`；`check` 三段 → 四段（第二段 `bindings:export`，事实校准 24）；`version` 取当前 `HEAD` 值 +1 patch（**相对递增，MUST NOT 写死字面号**）；行尾 LF
- [x] `packages/desktop/knip.json` 的 `ignore` 增 `"src/**/*.i.ts"`；`ignoreExportsUsedInFile` / `ignoreDependencies` 零新增
- [x] 跑 `pnpm run bindings:export` 再生成 `src/types/generated/bindings.ts` + `bindings.zod.ts` 并核对零 diff
- [x] 前端机械随动：白名单 ③ 的 8 个 `src/views/architecture/**/*.test.ts(x)` 字段补齐（零逻辑改动）
- [x] `.dev-team/architecture/models/05-backend-tools.toml`：表头 `3 元素 / 3 关系` + 新元素 `App.Backend.Tools.ArchiApiGen` + 2 条 `[[relation]]`
- [x] `.dev-team/architecture/decisions/0001-model-granularity.md`：`§1` / `§4` / `§6` / `§9` 四处计数同值 51 / 113（后端 49）
- [x] `crates/infra/agent/src/archi_knowledge.rs` + `archi-knowledge.md`：`api` 词汇与「契约权威」边界改写（D25）
- [x] 自查 阶段十五（cwd `packages/desktop`；**含固定串的判据一律用 `grep -cF`**，MUST NOT 依赖 shell 保留反斜杠的 BRE 交替形态）：`grep -cF '"api:check"' package.json` ⇒ **1**、`grep -cF '"api:export"' package.json` ⇒ **1**（两串分列计数，合计 **2**）；`grep -cF '"check": "pnpm run server:check && pnpm run bindings:export && pnpm run api:check && pnpm run client:check"' package.json` ⇒ **1**；`grep -cF 'cargo fmt --all --check' package.json` ⇒ **1** 且 `grep -cF 'cargo fmt --all --manifest-path' package.json` ⇒ **0**；`grep -cF 'src/**/*.i.ts' knip.json` ⇒ **1**；`grep -cF 'ArchiApiGen' .dev-team/architecture/models/05-backend-tools.toml` ≥ **2**

## 阶段十六：模型同步核对（AC-15）

- [x] `crates/{core,infra,tools}` 目录 ↔ 元素 `path` 双向差集（自研 crate 16 ↔ 16，`specta-zod-patched` 单列）；`05` 段 3 元素 3 关系；合计 51 / 113；三处计数同值；`archi_graph` 零诊断零告警
- [x] 核对结果落 `openspec/changes/module-api-contract-format/reports/`（MUST NOT 把本仓模型塞进 crate 语料）

## 阶段十七：根文档契约纪律节（AC-18）

- [x] `AGENT.md` + `CLAUDE.md` 各在「Architecture」节之后新增同一节 `## 模块 API 契约`（五条要点；MUST NOT 复制契约正文 / 判据长表 / fenced json 示例；节内零 `openspec/specs/**` 路径）
- [x] 自查 阶段十七（cwd 仓库根）：`grep -c '^## 模块 API 契约' AGENT.md CLAUDE.md` 各 **1**；`grep -c 'apiEntryMissing' AGENT.md CLAUDE.md` 各 **0**；`diff AGENT.md CLAUDE.md` 恰 **3** 处差异且全在节外

## 阶段十八：收尾自查（A–F）

- [x] **A（fmt 不动点）**：`cargo fmt --all --check --manifest-path src-tauri/Cargo.toml`（cwd `packages/desktop`）**exit 0**；两份渲染器源码自证 D29（单引号形态）
- [x] **B（门禁面）**：`pnpm run api:check` **exit 0** 且零写入（`git status` 前后同）；`npx vp check` / `knip` / `cargo clippy --workspace --all-targets --manifest-path src-tauri/Cargo.toml` 全绿
- [x] **C（判据基线）**：全量检查实跑，`apiEntryMissing` **28** / `apiMissing` **13** / 其余七条 **0** / `apiUndeclared` 不适用；报告数与 design 判据基线表**逐项相等**，结果留痕 `reports/`
- [x] **D（行尾面）**：`git diff HEAD --numstat` 与 `git diff HEAD --numstat --ignore-cr-at-eol` 实测**逐项相等**（零处行尾不等）——`package.json` 两侧同形 LF、`loop.rs` 零 diff；交付前出现的四处行尾翻面（`AGENT.md` / `CLAUDE.md` / `bash_test.rs` / `vocabulary_test.rs`）已按各自 `HEAD` 形态归位
- [x] **E（模型与文档）**：阶段十六 / 十七的四条判据复验；`openspec/specs/` 零改动（`git status --porcelain openspec/specs/` 零行）
- [x] **F（封闭数）**：按首部「核对封闭数」段执行——`comm -3 /tmp/allowlist.txt /tmp/actual.txt` **双向皆零行**（实际与允许集合逐路径相等）。实测：`wc -l /tmp/allowlist.txt` ⇒ **104**（清单 42 + 机械随动 18 + 测试相位 44），`git status --porcelain -uall`（剔除本变更目录）亦 **104** 行，F① 清单外改动 **0**、F② 清单内未改 **0**。**前提自检（首部「路径形态铁律」段）**：`/tmp/allowlist.txt` 内 `grep -cF '…'` ⇒ **0**（零缩写、零前缀省略）；**权威自检是 `comm -3` 本身**——只有两侧行形态与 `git status --porcelain -uall` 的路径列逐字同一时它才有意义。

## 阶段十九：三处语义裁定的收口核对（D26 / D27 / 事实校准 15–17）

- [x] 核对 `INDEX.categories` 取 `$defs` **形状**分布（非判据分类）：`grep -c "def_shape" crates/core/archi/src/api/mod.rs` ⇒ **2**（定义 + 调用）；`grep -c "category_of" crates/core/archi/src/api/mod.rs` ⇒ **0**
- [x] 核对 `x-class` 两判据落点分工：错误在 `schema::verify_defs`、警告在 `emit::render_target` 且每元素恰一次（`grep -rn "eprintln!" crates/core/archi/src/api/emit/` **恰 1** 命中）
- [x] 核对 `categories` 与 `types` 同源：`sum(categories) == types`（对照 `INDEX.toml` 与 `$defs` 条目数）

## 阶段二十：spec 措辞同步登记（S-1…S-17；**非实现交付物**）

- [x] **S-1 / S-2 / S-3 / S-6**（delta 甲类，owner = spec 相位）：`specs/desktop-archi-api/spec.md` 的「生成物入仓与零 diff 门挂载」Requirement 正文、其 Scenario 标题与 THEN、Module Contract 表 `package.json` 行——逐条替换为「fmt 段改校验模式 / 两段原文不变」
- [x] **S-7 / S-8 / S-9 / S-10**（delta 乙类）：`visibility` 四值 requirement 的 TS 分句（两处原文）、Scenario `visibility 四值判定` 的 THEN、Module Contract 表 `api/extract/ts.rs` 行的箭头函数分句、Scenario `bootstrap 只出 stdout` 的 THEN 与 `api/extract/mod.rs` 行——按 D19-a / D19-b / D39 收紧（**替换文本 MUST NOT 复含旧串**；S-8 用 `本期不接线`，S-12 用 `本期未接线`，两串不互含）
- [x] **S-11 / S-12**（delta 丙类）：`#[path]` 分句改为「字面量 + 与 `mod_path_attr` 输出逐字相等」；`api/extract/ts.rs` 行末句改为「降级接口已实现但本期未接线」
- [x] **S-13 / S-14 / S-15**（delta 丁类）：位置分派条补「位置词汇与签名短形 MUST 各只有一份」；`desktop-archi-api-check` 的判据表 `apiMissing` 行与「覆盖两类缺口」条钉死「同一条判据、同一次求值 + 三条排除面 + 两条结构前提」、其 Module Contract 表 `api/check.rs` 行改「声明集（op **与 `$defs` 类型**）vs 导出集」；`desktop-archi-api` 的「四条明确不做」之前新增两道人字闸条
- [x] **S-16 / S-17**（delta 戊类，纯新增）：`#[path]` 读盘面与写盘面同一条限根纪律；`api/paths.rs` 行补「末段进源码的形态闸按语言分列 + `checked_root` 只判整条不判末段字符集」
- [x] **S-4 / S-5**（主基线，owner = 归档 SpecSync 步）：本相位只登记与核对**零改动**（`git status --porcelain openspec/specs/` 零行），MUST NOT 在本 change 内替换
- [x] 自查 阶段二十：`grep -cF` 对每条**新原文串**在 `specs/**` 各 ≥ **1**（S-14 的两条钉死句各 2）；旧原文串按表在 `specs/**` **零命中**（保留型 S-13 与 S-14 的两处补句只判新串在场；纯新增 S-15 / S-16 / S-17 无旧串可数）；`grep -F` 片段 MUST 从 design「spec 措辞同步项」表机械抽取，MUST NOT 手抄

## 阶段二十一：D33–D36 收口（形态校验 / 孤儿扫描 / 键空间 / 死数据）

- [x] **① 四条路径派生可失败**（D33）：`contract_path` / `schema_path` → `Option<PathBuf>`，`generated_path` / `entry_form` 的 `None` 语义扩为「形态非法亦 `None`」；调用点按新签名机械随动
- [x] **② `--check` 孤儿扫描全树递归**（D34）：`*.i.rs` / `*.i.ts` 任意深度 + 契约目录内 `*.schema.json` / `INDEX.toml`；跳过 `target` / `node_modules` / `dist` / `build` / `.git`；深度上限 `MAX_MODULE_WALK_DEPTH`
- [x] **③ `build_schema` 两处先判后写**（D35）：`$defs` 内 `api-types` 与 `api-reexport` 同名、`properties` 内 op 与类型同名 ⇒ `ApiError::Contract`
- [x] **④ `absent` 死数据删除**（D36）：`ApiSnapshot` / `ApiCheckInput` 各零该字段；装载期不记清单；判据 ② 按 `element.api == Declared && contract.is_none()` 判
- [x] 自查 阶段二十一：`grep "absent:" crates/core/archi/src/api/**` 零命中、`grep -cF "input.absent"` 与 `grep -cF "snapshot.absent"` 各 ⇒ **0**；`grep -c "MAX_MODULE_WALK_DEPTH" crates/tools/archi-api-gen/src/main.rs` ≥ **2**；`grep -c "键空间唯一" crates/core/archi/src/api/schema.rs` ⇒ **2**（判据落**产品文件**，MUST NOT 用测试文件计数）

## 阶段二十二：D37–D42 收口（文本层重复键 / 字面量等值 / knip 不读 / 三处二次派生）

- [x] **① 重复键文本层判据**（D37）：`contract::parse` 在 `from_str` **之前**做对象层级键唯一性扫描；`schema_test` 那条反向断言的改判归**测试相位**（本相位只登记方向，MUST NOT 写测试内容判据）
- [x] **② `#[path]` 字面量 + 等值断言**（D38）：入口行写字面量；`crates/infra/archi/src/lib.rs` 注释如实化；`mod_path_attr` 保留为等值真值源（零产品调用者是正常形态）
- [x] **③ knip 本期不读**（D39）：`main.rs` 与 `api::bootstrap` 零 `knip` 词；`apply_knip_report` 保留、产品路径零调用；两处 doc 写明留白
- [x] **④ `<name>` 取路径末段 + Rust 模块路径取元素根**（D40）：`EmitTarget.modules: BTreeMap<String, ModuleRef>`；`paths::module_name` / `rust_module_path` 单点
- [x] **⑤ 形态由调用方传入**（D41）：删 `emit/ts.rs` 的 `entry_form_of` 与 `target.rsplit('.')`；`EmitTarget.form` 由 `render_all` 经 `paths::entry_form` 求
- [x] **⑥ `generated_path` 收窄入参**（D42）：删 `api/mod.rs` 的 dummy `ArchiElement`；「容器元素无生成物」由调用方 `path.first()?` 表达
- [x] 自查 阶段二十二：`grep -cF "entry_form_of" crates/core/archi/src/api/emit/ts.rs` ⇒ **0** 且 `grep -cF "rsplit('.')" crates/core/archi/src/api/emit/ts.rs` **⇒ 0**（两串分列计数，MUST NOT 用 BRE 交替）；`grep -c "ArchiElement {" crates/core/archi/src/api/mod.rs` 的产品面**零构造**（`generated_path_of` 只收 `<root>` 串）；`grep -c knip crates/tools/archi-api-gen/src/main.rs` ⇒ **0**；`grep -rn "apply_knip_report(" crates --include=*.rs` 滤测试与定义后**零命中**

## 阶段二十三：D43 / D44 / D45 收口（位置单点 / 类型面并入 / 两道人字闸）

- [x] **① 位置词汇与短形各收一处**（D43）：`api/extract/mod.rs` 新增 `Position` / `schema_type` / `signature_of` 三行；**`render_scope` 保持私有**（逐语言的片段工序，D46），**`render` 是 D46 的四参 `pub` 出口**（`pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String`，盘上 rust `:46` / ts `:96`）——**MUST NOT 把 `render` 降为私有**，否则 `emit::render_target` 的分派分支不可编译、AC-24 的「出口单形」判据失去对象
- [x] **② `check.rs` 删本地位置表与本地归一**（D43）：删私有 `enum Position` / `fn normalize_schema` / `fn contract_signature`；④ 步骤改经 `extract::signature_of`
- [x] **③ 两侧渲染器删本地 `Position`**（D43）：`emit/{rust,ts}.rs` 改 `use` `extract::Position`
- [x] **④ `apiMissing` 的类型面并入同一判据**（D44）：`missing_contracts` 内 op 段之后追加类型段，共用三条排除面
- [x] **⑤ 取数面补两处结构前提**（D44）：`extract/rust.rs` 增 `#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格
- [x] **⑥ 闸一：七族名字的 ASCII 标识符判据**（D45）：`contract::verify` 内单点 `is_identifier` + 七族逐族收集；并有「插值点枚举表 8 组 ↔ 承接地」的落点
- [x] **⑦ 闸二：两侧字符串字面量与注释的单点转义**（D45）：`ts_string_literal` / `ts_comment_text` / `rust_comment_text` 三处单点，`enum` 值经 `ts_string_literal`
- [x] 自查 阶段二十三：`grep -c "fn is_identifier" crates/core/archi/src/api/` 滤测试后 ⇒ **1**（`contract.rs`）且调用点在 `verify` 体内；`grep -c "is_identifier" crates/core/archi/src/api/emit/rust.rs crates/core/archi/src/api/emit/ts.rs` 各 ⇒ **0**；**`is_identifier` 只管七族名字**：`grep -cF "is_identifier(" crates/core/archi/src/api/paths.rs` ⇒ **0**（末段形态闸**只**由 `paths::is_module_segment` 承接，D48 / D50；`paths` 是 `api` 地基层，MUST NOT 反向依赖 `contract`；判据串锚**调用形态** `is_identifier(`，因为该文件 doc 有一处「与 `contract::verify` 的 `is_identifier` 事实源不同」的说明句，裸 `is_identifier` 会把它误计为 1）；`grep -c "function_param_names(" crates/core/archi/src/api/contract.rs` ⇒ **3**（定义 1 + 调用 2）；`grep -c "enum Position" crates/core/archi/src/api/{check.rs,emit/rust.rs,emit/ts.rs}` 各 ⇒ **0**
- [x] **测试相位留痕（用例面归测试相位）**：`check_test.rs` / `contract_test.rs` 的既有夹具与断言面归测试相位裁定；本相位只保证产品面形态正确。D55 / D58 / D59 / D60 的新增用例已由测试相位落地，清单见页头「例外」段（本相位不再新增用例）

## 阶段二十四：渲染器出口单形（D46）

- [x] **① 两侧渲染器收为四参单形**：`pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String`（`target` **末位**；**MUST NOT** 主张与 `emit::render_target` 同序）；删 `render_with_root` / `render_with_target` 两个异名孪生；`render_scope` 保持私有
- [x] **② `emit/mod.rs` 分派随动**：两个分支改为 `GeneratedKind::Rust => rust::render(contract, schema, meta, target)` / `GeneratedKind::TypeScript => ts::render(…)`
- [x] ③ 自查 阶段二十四：逐产品文件带完整签名 `grep -c "pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String"` 各 ⇒ **1**；`grep -cE "render_with_root|render_with_target" crates/core/archi/src/api/**` 滤测试后 ⇒ **0**；产品面 `rust::render(` / `ts::render(` 直连各 ⇒ **1**（分派器分支）
- [x] **⑤ 调用点机械随动的归属二分（D53）**：**（甲）编译必需 ⇒ 本相位做**（`api/emit/rust_test.rs` 两处三参 → 四参；`api/emit/ts_test.rs` 的 `use` 行 + 五处 `render_with_root(..)` → 四参 + 三处三参 → 四参 + 装置同名提取）；**（丙）纯归测试相位 ⇒ 从本文件删除授权**、只登记**接手红点**与**禁止项**：签名自证串 / 职责清单 / 两处 `pub use` 断言 / 六处用例名去 `_with_root`。**MUST NOT** 为让旧断言继续绿而放宽产品面形态；**MUST NOT** 留下不可编译形态
- [x] ④ 自查（甲）档：`cargo check -p archi --all-targets` 零错误（E0061 / E0433 零命中）；两测试文件内 `render_with_root(` 零命中

## 阶段二十五：`x-function` 落点封闭（D47）

- [x] **① 落点闸**：落 `contract::verify` 内（与七族闸一同函数、同趟遍历，MUST NOT 拆第二个公开入口）；`x-function` 只许 `api-op` 顶层签名与 `$defs` 条目第一层 `properties` 的值
- [x] **② 渲染器收敛**：`emit/ts.rs` 的 `value_type` 删 `x-function` 分支；`emit/rust.rs` 的 `value_type` 写成不变量（doc 说明「结构上收不到该形态」）
- [x] 自查 阶段二十五：`grep -c "x-function" crates/core/archi/src/api/emit/ts.rs` 只在 doc 与不变量注释命中（`value_type` 体内零分支）；`grep -c "fn function_param_names" crates/core/archi/src/api/contract.rs` ⇒ **1**；四种越界形态（`items` / `additionalProperties` / `oneOf` 分支 / 深层嵌套）在 `load_contracts` 期即中止（行为面）
- [x] **测试相位留痕**：`contract_test.rs` 两例的断言面不受影响，改判归测试相位

## 阶段二十六：⑦ 组的路径形态承接（D48）

- [x] **① 路径面形态判据单点**：`paths::is_module_segment`（`[A-Za-z_][A-Za-z0-9_]*` + 非空 + 非关键字）
- [x] **② 装配点按语言筛选**：`api/mod.rs` 的 `module_refs` 对 Rust 分支拒绝非法末段（不入查表 ⇒ 只出 `// 透传自 <FQN>` 注释）
- [x] **③ TS 侧 `from` 字面量转义**：`emit/ts.rs` 的 `render_scope` 透传段改为 `ts_string_literal(&paths::ts_import_specifier(..))`
- [x] 自查 阶段二十六：`grep -c "fn is_module_segment" crates/core/archi/src/api/paths.rs` ⇒ **1**；`grep -c "is_module_segment" crates/core/archi/src/api/mod.rs` ≥ **1**；`grep -c "ts_string_literal(&paths::ts_import_specifier" crates/core/archi/src/api/emit/ts.rs` ⇒ **1**；**TS 面零筛选**（`emit/ts.rs` 内对 `module_name` 零字符集判据）
- [x] **测试相位留痕**：`paths_test` / `mod_test` / `emit/{rust,ts}_test` 的新增用例归测试相位；产品面收紧后旧断言必红属机械派生物

## 阶段二十七：D49–D52 收口（限根 / 关键字 / 行号 / 零调用者）

- [x] **① `#[path]` 读盘面补两步限根**（D49）：折叠之前反斜杠归一 + `join` 后 `starts_with(root)` 复核；越根即不命中该子树
- [x] **② `is_module_segment` 加关键字面**（D50）：`const RUST_KEYWORDS`（38 + 13）封闭持有；判据收为合取二项；**MUST NOT** 输出 `r#<段>`
- [x] **③ `occurrences.line` 分列**（D51）：op 缺口取 `ContractOp.line`、类型缺口取 `0`
- [x] **④ `extract::extract` 显式留白**（D52）：接口保留 + 模块 doc 与函数 doc 各一段留白声明；**MUST NOT** 加产品调用者、MUST NOT 连带降级 `extract_file`
- [x] 自查 阶段二十七：`grep -cF "starts_with(root)" crates/core/archi/src/api/extract/rust.rs` ≥ **1**；`grep -c "const RUST_KEYWORDS" crates/core/archi/src/api/paths.rs` ⇒ **1**；`grep -rn "extract::extract(" crates --include=*.rs` 滤测试与定义后**零命中**；三个反例（越根 / 反斜杠 / 栈空）各不命中（行为面）
- [x] **测试相位留痕**：`rust_test` / `paths_test` / `check_test` 的相应用例归测试相位

## 阶段二十八：第四类机械随动的验收（D53 / AC-30）

- [x] **① 三种形态逐项在案**：`use` 行改指 / 实参补齐（只取 `empty_target()` / `empty_scope()`）/ 装置同名提取；两落点文件内零 `render_with_root(`
- [x] **② 断言面零改动**：`api/emit/{rust,ts}_test.rs` 的断言值 / 断言意图 / 用例名 / 用例增删 / 夹具语义**零改动**（`git diff` 逐 `hunk` 复核，只允许三种形态）
- [x] **③ 白名单不增项**：机械随动白名单仍 **18 路径**、测试相位仍 **13 组 / 47 文件**、并集仍 **113 路径**（收进即双计，MUST NOT）
- [x] **④ 否定判据**：本文件内不再出现「可做可不做 / 可选随动」作为**授权项**
- [x] 自查 阶段二十八：撤掉（甲）档 `hunk` 后 `cargo clippy --workspace --all-targets` 转非绿（「编译必需」自证）；`grep -c "render_with_root(" crates/core/archi/src/api/emit/{rust,ts}_test.rs` 各 ⇒ **0**

## 阶段二十九：映射规则消费与跨 crate 单点（D54 / D55 / D56 / D57；AC-31 / AC-32 / AC-33；**本稿新增**）

> **本阶段消三处「design 页头声明了但正文零落地」的缺口**：`common.json` 是一等映射规则资产却零条要求渲染器解析（资产与实现零连接）、`api::display` 是 `pub(crate)` 而工具 crate 物理不可复用（存在第二份逐字实现）、一处与本变更零关联的测试装置超时被越界放宽。三条都不是实现走偏，而是**裁定缺失的补登**。

- [x] **① `common.json` 资产形态归一（D54 ① / D57）**：`crates/core/archi/src/api/common.json` 的**键 = 协议名字**（首期唯一键 `Outcome`，与 `ApiType::Outcome` 变体名、`#/$defs/Outcome`、spec `:149` 同形；**MUST NOT 改成小写 `outcome`**）+ 每语言单槽位（`rust` / `typescript`）+ `{}` 占位符 + 可选 `degraded` / `note`；**槽位内 MUST 恰含 ≥1 个 `{}`**；文件内零语言专属控制流（它只描述文本形）
- [x] **② 载元携带映射表（D54 ②）**：`crates/core/archi/src/api/mod.rs` 新增 `pub struct CommonRule { rust: String, typescript: String, degraded: Option<String>, note: Option<String> }`；`ApiMeta` 增 `pub common: BTreeMap<String, CommonRule>`；新增 `pub(crate) fn parse_common(source: &str) -> Result<BTreeMap<String, CommonRule>, ApiError>` 作**唯一解析点**（缺语言槽位 / 零 `{}` / JSON 非法 ⇒ `ApiError::Contract` 并点名键与语言）；`api::meta()` 填 `common`，`load_contracts` 以同一函数复核；**同文件新增 `pub(crate) fn protocol_key(name: &str) -> String`**（D57 换算单点：协议名字 `Outcome` → 契约 / schema 承载键 `"outcome"`）
- [x] **③ 两侧渲染器收为查表单点（D54 ③④）**：`emit/rust.rs` 与 `emit/ts.rs` 各新增私有 `fn common_type(schema: &Value, position: Position, common: &BTreeMap<String, CommonRule>) -> Option<String>`——**承载键经 `api::protocol_key("Outcome")` 求出**、`schema.get(..)` 取值后按协议名字 `Outcome` 查表（D57）、取本语言槽位、把 `{}` 替换为 `value_type(内容位)`；`value_type` 的两处 `if let Some(outcome) = schema.get("outcome")` 硬编码分支**删除**，改调 `common_type`；`emit/ts.rs` 的 `function_type` async 分支经**同一出口**取语言侧包装。**渲染器零 IO、四参签名不扩**（数据已进载元，D46 不受影响）。**命令形态纪律**：本节一切含定形串的判据一律 `grep -cF` + **单引号包裹**（串内引号原样书写、**MUST NOT** 写成反斜杠转义形态），跨 Bash / PowerShell / `cmd.exe` 同语义
- [x] **④ `api::display` 收为跨 crate 单点（D55）**：`crates/core/archi/src/api/mod.rs` 的 `pub(crate) fn display` 升为 **`pub fn display`**；`crates/core/archi/src/lib.rs` 增 `pub use api::display;`；`crates/tools/archi-api-gen/src/main.rs` 的本地 `fn display` **整条删除**，改用 `archi::display`；`short_digest` / `is_product_rs` 仍留 `pub(crate)`（MUST NOT 顺手扩大出口面）
- [x] **⑤ 装置超时越界回滚（D56 / AC-33）—— 已在收敛稿执行**：`packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` 的 `spawn_command_confirmer` 已回到约定口径（确认窗 `Duration::from_secs(60)`、无 `register_window`，保留「见过 run 登记后转 `None`」的退出判据），复合用例的收敛等待已回到 `wait_for`（30s 约定上限）。**执行形态 = `git checkout HEAD -- <该文件>` 一次**（该文件相对 `HEAD` 的全部改动都属越界弱化，整文件回滚即最小闭合），**产品面零改动、用例面零改动**。**判据（已实测）**：`grep -cF 'from_secs(180)'` ⇒ **0**、`grep -cF 'register_window'` ⇒ **0**、`grep -c 'wait_for'` ≥ **1**、`git diff HEAD -- 同文件` 为空。该文件**不在**变更清单、机械随动白名单与实现相位封闭集合（60 路径）内
- [x] 自查 阶段二十九（**判据一律锚符号名与定形串，MUST NOT 写死行号**）：
  - ① **硬编码零残留**：`grep -cF 'Result<{}, String>' crates/core/archi/src/api/emit/rust.rs` 滤测试后 ⇒ **0**；`grep -cF '{ ok: {} } | { err: string }' crates/core/archi/src/api/emit/ts.rs` 滤测试后 ⇒ **0**（两串只允许出现在 `api/common.json` 与 `ApiMeta` 的解析测试装置里）
  - ② **查表单点**：`grep -c "fn common_type" crates/core/archi/src/api/emit/rust.rs` ⇒ **1**、`…/emit/ts.rs` ⇒ **1**；`grep -c "common_type(" …/emit/rust.rs` ≥ **2**（定义 + `value_type` 调用）、`…/emit/ts.rs` ≥ **3**（定义 + `value_type` + `function_type`）
  - ③ **资产与载元**：`grep -c "pub common: BTreeMap<String, CommonRule>" crates/core/archi/src/api/mod.rs` ⇒ **1**；`grep -c "fn parse_common" crates/core/archi/src/api/mod.rs` ⇒ **1**；`grep -c "fn protocol_key" crates/core/archi/src/api/mod.rs` ⇒ **1**（D57）；`grep -cF '"rust": "Result<{}, String>"' crates/core/archi/src/api/common.json` ⇒ **1**；`grep -cF '"typescript"' crates/core/archi/src/api/common.json` ⇒ **1**；`grep -cF '"degraded"' crates/core/archi/src/api/common.json` ⇒ **1**（三条一律 `-F` + 单引号、**串内引号原样书写**，MUST NOT 写成反斜杠转义形态——转义形态依赖双引号包裹 + shell 保留反斜杠，Bash 得 1 而 PowerShell / `cmd.exe` 语义不同）；另 `grep -cF 'Outcome' crates/core/archi/src/api/common.json` ⇒ **≥ 1**（资产键 = 协议名字，D57 ①；**MUST NOT** 用 `'outcome'` 作资产键判据）
  - ④ **改表即变产物**：改 `common.json` 的 `typescript` 槽位（临时）后 `cargo run -q -p archi-api-gen -- generate` 的两侧产物文本随变，随后 `git checkout --` 还原（临时验证，MUST NOT 留改动）；`cargo run -q -p archi-api-gen -- --check` 在还原后 **exit 0**
  - ⑤ **`display` 单点**（cwd `packages/desktop/src-tauri`，逐条独立可判）：a) `grep -c '^fn display' crates/tools/archi-api-gen/src/main.rs` ⇒ **0**（本地**自由函数**已删；`grep -cF "fn display"` 会把 `file.path.display()` 一类方法调用与 doc 词面误计，故判据串锚**行首定义形态**）；b) `grep -cF "archi::display" crates/tools/archi-api-gen/src/main.rs` ⇒ **≥ 1**（改用跨 crate 单点）；c) `grep -c '^pub fn display' crates/core/archi/src/api/mod.rs` ⇒ **1**；d) `grep -cF "pub use api::display" crates/core/archi/src/lib.rs` ⇒ **1**；e) 行为判据：`cargo run -q -p archi-api-gen -- --check` 的差异清单路径串与改动前逐字相同
  - ⑥ **回滚的核对面（已执行）**：a) 该文件**不在**实现相位封闭集合——判据串取**物理全路径** `grep -cF 'packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs' /tmp/allowlist.txt` ⇒ **0**（集合内全是物理全路径、零前缀省略，故命中 0 才**真的**等于「不在集合内」；**MUST NOT** 用 `src/commands/…` 省略形态当判据串——该形态在物理路径集合里恒 0 命中，读到 0 不构成证据）；b) 回滚已闭合：`git diff HEAD -- 同文件` **为空**，且该文件因此**移出**测试相位白名单（回滚后零 diff，留在表内会让并集多 1）。**三条落地判据（`from_secs(180)` / `register_window` 各 ⇒ 0、`wait_for` 在场）已在 ⑤ 实测通过**
  - ⑦ **封闭数现行值**：清单 **40 行 / 42 路径**、机械随动白名单 **18 路径**、实现相位封闭数 **60 路径**、测试相位 **12 组 / 44 文件**、并集 **104 路径**（本阶段三处收口**零新增文件**：`common.json` / `api/mod.rs` / `emit/{rust,ts}.rs` / `lib.rs` / `main.rs` 全在既有清单内；`change_flow/mod_test.rs` 回滚后零 diff、两集皆不含它）。**判据**：与 design「变更清单 - 核对口径」段逐值相等；本阶段 MUST NOT 引入任何清单外文件
  - ⑧ **不变量**：`cargo check -p archi -p archi-api-gen --all-targets` 零错误；`cargo run -q -p archi-api-gen -- --check` **exit 0**（映射改经查表后产物与重算一致）
  - ⑨ **`bash.rs` 归属单列（收敛稿改判）**：`grep -c "pub(crate) fn resolve_timeout_ms" crates/infra/agent/src/sdk/bash.rs` ⇒ **1**；`grep -c "resolve_timeout_ms" crates/infra/agent/src/sdk/bash_test.rs` ≥ **1**（兄弟测试模块直测在案）；`git diff HEAD --numstat -- crates/infra/agent/src/sdk/bash.rs` ⇒ 恰 `2 1`（**+2 / −1**：doc 行 + 签名行）。**它归清单修改表、MUST NOT 同时列入测试相位白名单**——旧稿两处都写，令 66 ∪ 47 的并集少 1（真值 112 ≠ 113），是「并集恒对不上」的唯一算术来源
- [x] **⑩ 协议键两层口径（D57）**：`api/mod.rs` 新增 `pub(crate) fn protocol_key(name: &str) -> String`（资产键 = 协议名字 `Outcome` → 承载键 `"outcome"`，首期实现即小写化）；两侧 `common_type` 的 `schema.get(..)` 取值键 MUST 经它；**`common.json` 的键 MUST NOT 改成小写**（改成小写即与 spec `:149` / `:158` / `:185`、`ApiType::Outcome`、`#/$defs/Outcome` 全面失配）；两侧渲染器 MUST NOT 各写一份键变换
- [x] **测试相位留痕**：① D54 / D57 的用例已在 `api/mod_test.rs`（`parse_common` 四类坏形态 / `protocol_key` / `ApiMeta.common` 同源）、`emit/{rust,ts}_test.rs`（`common_type` 按表展开 + 空表回落 + 产品面零硬编码串）与 `tools/archi-api-gen/src/main_test.rs`（跨 crate `display` 单点）落地，`test-design.md` 的 AC-31/32/33 映射行与三章用例行同步补齐；② **`change_flow/mod_test.rs` 的装置超时回滚（D56 / AC-33）已由 ⑤ 执行并闭合**（`git diff HEAD` 为空），其用例面零改动

## 阶段三十：三条收口裁定的实现面交接（D58 / D59 / D60；AC-34；**本稿新增**）

> **本阶段消三处「接口面大于需求面 / 同一映射两处实现 / 出口面多一个零消费者」的形态**：`EmitTarget` 的 `path` / `element_root` 产品面零读取、`emit/rust.rs` 的 `choice_payload` 自带未使用形参且与 `value_type` 重复、`api/mod.rs` 的 `pub use check::category_of;` 产品面零消费者。三条都是**设计面未收口的直接后果**，实现相位只做「按裁定删字段 / 删函数 / 删重导出 + 同批修构造形态」，**零新增逻辑、零行为变化**（生成物逐字节不变）。
>
> **依赖序**：阶段三十依赖阶段六（渲染器在案）与阶段九（`render_all` 与出口面在案），且**与阶段二十四 / 二十九不重叠**（那两阶段收的是出口单形与映射查表面，本阶段收字段面、删第二份映射、删出口冗余）。本阶段**零新增文件**（四个产品文件全在既有清单内），封闭数逐值不变（60 / 44 / 104）。
>
> **两段分界（按改动种类划，不按文件划）**：本阶段 ①②③④ 是**实现相位**——①②③ 是产品面（字段面 / 构造点 / 删函数与出口），④ 是「产品可编译」所必需的**构造形态适配**（D53 第四类机械随动，射程封闭为「删两字段的初始化行 + 随之无用的 `use`」）；⑤ 是**测试相位移交清单**，本相位**只登记、不执行**（`test-design.md` 的表述改判、用例名与断言意图的改判全归测试相位）。判据是 ⑥ 的自查行——它们只断言产品面事实，不对测试文件命名 `MUST`。

- [x] **① `EmitTarget` 收为三字段（D58 / AC-34 ①）**：`crates/core/archi/src/api/emit/mod.rs` 的 `pub struct EmitTarget` 删 `path: PathBuf` 与 `element_root: String` 两字段，定义体恰剩 `kind: GeneratedKind` / `form: EntryForm` / `modules: BTreeMap<String, ModuleRef>`；字段 doc 同步删掉两段（「落点路径供调用方写盘与头注释溯源」与「仅用于注释与诊断定位」——后者正是被本裁定否决的表述）；`use std::path::PathBuf;` 若无其他消费者同批删除（`cargo clippy -D warnings` 会报 unused import）
- [x] **② `render_all` 的构造点改三值（D58 / AC-34 ①）**：`crates/core/archi/src/api/mod.rs` 的 `render_all` 内 `EmitTarget { kind, form, modules }` 三值构造——`path` 与 `scope` 保留为**局部变量**并直接用于 `GeneratedFile { path, content }`；`scope` 只再喂给 `paths::{generated_kind, generated_path}` 与 `paths::entry_form`（即只作**派生入参**，不成为渲染器入参）
- [x] **③ 一处 `pub use` 与 `choice_payload` 删除（D59 / D60 / AC-34 ②③）**：`crates/core/archi/src/api/mod.rs` 删 `pub use check::category_of;` 整行（`check::category_of` 的 `pub` 与 `check.rs` 内调用点**不动**）；`crates/core/archi/src/api/emit/rust.rs` 删私有 `fn choice_payload(...)` 整条，`variants_block` 的负载判定改经 `value_type(choice, Position::Field, common)` 的返回值——**判定即「返回值是否等于兜底文本 `serde_json::Value`」**（MUST NOT 在 `variants_block` 内重写第二份标量表，MUST NOT 新增第三个判定函数）
- [x] **④ 编译必需的构造形态适配（D53 第四类机械随动，**本相位执行**）**：`crates/core/archi/src/api/emit/{mod,rust,ts}_test.rs` / `api/paths_test.rs` / `crates/core/archi/tests/pilot_test.rs` 内共 **19 处** `EmitTarget {` 构造字面量删 `path` / `element_root` 两字段（逐文件 `grep -cE '^\s*(let [a-z_]+ = |&)?EmitTarget \{$'` 现算：`emit/mod_test.rs` 2 + `emit/rust_test.rs` 5 + `emit/ts_test.rs` 10 + `api/paths_test.rs` 1 + `tests/pilot_test.rs` 1 = 19；两字段初始化行合计 **38 行**；`emit/mod.rs` 与 `api/mod.rs` 的定义与构造点归 ①②）。**判据串 MUST 锚构造形态**：`grep -c 'EmitTarget {'` 会把 `-> EmitTarget {` 返回类型行、doc 注释与 `.find("pub struct EmitTarget {")` 一并计入（删前 26 / 删后 28），拿它当「构造字面量数」是伪判据。**射程封闭**：只删两字段初始化行与随之无用的 `use`（`PathBuf` 若在该文件其他处零消费者）；**MUST NOT** 改用例名 / 断言值 / 断言意图 / 增删用例 / 夹具语义。这 5 个文件已在测试相位 44 文件集内计数，**不增机械随动白名单项**
- [x] **⑤ 测试相位移交清单（本相位只登记、不执行；executor = 测试相位）**：**`test-design.md` 侧（a、c 的文档面）与 `emit/rust_test.rs` 的三类正例（e）已由测试相位落地**，下列各项作为「应然形态」的登记保留：a) `test-design.md` 的三处改判（**按用例名 / 段名锚定，不写死行号**——行号随编辑漂移，与阶段三十 ⑥ 的同一纪律）——`emit/mod.rs` 说明段里「`element_root` 只影响 TS specifier 的形态语境与诊断定位」整句删掉（**specifier 只由 `form` 与 `ModuleRef.name` 决定**）、「单契约粒度」与「`EmitTarget` 三字段面 / 三字段承载」两处用例行的「五字段」改「三字段 `{ kind, form, modules }`」（含该节说明段的 `EmitTarget` 字段枚举同步）；b) `crates/core/archi/src/api/emit/mod_test.rs` 的 `render_target的emit_target五字段面` 用例改判三字段（用例名 / 断言值 / 断言意图归本相位执行者）；c) `contract_test.rs` 注释里的 `choice_payload` 词面改指 `value_type`（与产品侧 `contract.rs` 的同一处注释改判同源，`contract.rs` 侧归 ③ 本相位执行）；d) `api/mod_test.rs` 的「`mod.rs` 内零 `category_of` 调用」断言**保持逐字不变**（删出口只会让它更真，MUST NOT 顺带改断言）；e) `api/emit/rust_test.rs` 的 `render的oneof为带负载enum` 用例：`$ref` 分支断言（`AA(A)` / `BB(B)`）**逐字不变**，另需按 D59 ⑤ 的精确射程补三类正例——**协议键分支**（`{"outcome": {...}}`）与**`array` 分支**与**带 `additionalProperties` 的 `object` 分支**各产带负载变体（此前落 `Variant{index}`）、而「`object` 无 `additionalProperties`」与「无 `type` 非 `$ref`」仍落 `Variant{index}`（D59 的 AC-34 ② 行为面）
- [x] **⑥ 自查 阶段三十（判据一律锚符号名与定形串，MUST NOT 写死行号）**：
  - ① **三字段面**：`grep -cF 'pub element_root' crates/core/archi/src/api/emit/mod.rs` ⇒ **0**；`grep -cF 'pub path: PathBuf' crates/core/archi/src/api/emit/mod.rs` ⇒ **0**；`grep -cF 'use std::path::PathBuf' …/emit/mod.rs` ⇒ **0**（`use` 只服务被删字段）；`grep -cF 'pub kind: GeneratedKind' …/emit/mod.rs` ⇒ **1**、`grep -cF 'pub form: EntryForm'` ⇒ **1**、`grep -cF 'pub modules: BTreeMap<String, ModuleRef>'` ⇒ **1**
  - ② **产品面读取点恰三处**：`grep -rnF -e 'target.kind' -e 'target.form' -e 'target.modules' --include=*.rs crates/core/archi/src/api` 滤 `*_test.rs` 后**恰 3 行**（**串一律 `-F` 分列给出、MUST NOT 用 `\|` 交替**——该形态依赖 shell 保留反斜杠，跨 Bash / PowerShell / `cmd.exe` 语义不同）（`emit/mod.rs` 的 `.kind`、`emit/rust.rs` 的 `.modules`、`emit/ts.rs` 的 `.form` 与 `.modules` 同行——**判据取行数不取串数**，否则 `ts.rs` 一行两串会把 3 算成 4）；`grep -rnF -e 'target.path' -e 'target.element_root' --include=*.rs crates/core/archi/src/api` 滤 `*_test.rs` 后 ⇒ **0**
  - ③ **`choice_payload` 零残留**：`grep -cF 'fn choice_payload' crates/core/archi/src/api/emit/rust.rs` ⇒ **0**；`grep -cF 'choice_payload' crates/core/archi/src/api/contract.rs` ⇒ **0**（注释改指 `value_type`）；`grep -cF '"i64"' crates/core/archi/src/api/emit/rust.rs` ⇒ **2**（由删前的 **3** 降为 **2**；剩下两处分别在 `value_type` 与 `scalar_type`，与 `variants_block` 无关——MUST NOT 取 **1**，那会让 `scalar_type` 的合法映射被误判为残留）
  - ④ **`category_of` 出口面**：`grep -cF 'pub use check::category_of' crates/core/archi/src/api/mod.rs` ⇒ **0**；`grep -c 'fn category_of' crates/core/archi/src/api/check.rs` ⇒ **1**；`grep -cF 'use super::check::{category_of, check}' crates/core/archi/src/api/check_test.rs` ⇒ **1**
  - ⑤ **试点零影响（生成物逐字节不变）**：`cargo run -q -p archi-api-gen -- --check` **exit 0**——`EmitTarget` 删两字段零可观测影响；`variants_block` 改通路的行为扩展面（协议键 / `array` / `object(additionalProperties)`）**在试点契约上不触发**（试点零 `oneOf` / `anyOf`，见 D59 ⑤），故四类生成物**逐字节不变**；`.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json` / `.dev-team/architecture/api/INDEX.toml` / `crates/infra/archi.i.rs` 三份生成物的 `git diff` **为空**
  - ⑥ **编译与静态门**：`cargo check -p archi -p archi-api-gen --all-targets` 零错误；`cargo clippy --workspace --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings` 零告警（删字段后 `PathBuf` / 未用 `use` 若残留即在此报红）
  - ⑦ **封闭数现行值不变**：清单 **40 行 / 42 路径**、机械随动白名单 **18 路径**、实现相位封闭数 **60 路径**、测试相位 **12 组 / 44 文件**、并集 **104 路径**——本阶段三处收口**零新增文件**（`emit/mod.rs` / `api/mod.rs` / `emit/rust.rs` 全在既有新增表内）。**判据**：与 design「变更清单 - 核对口径」段逐值相等；本阶段 MUST NOT 引入任何清单外文件

- [x] **⑦ 回溯项的实现面闭合（两处清单外 / 表外形态的净清零；本相位执行）**：
  - a) **`declare_ancestors` 内联进 `api/mod_test.rs`**：删除 `crates/core/archi/src/api/test_support.rs`（47 行，`#[cfg(test)] pub(crate) mod` 挂在产品文件 `api/mod.rs` 上、且不在新增 18 / 机械随动 18 / 测试相位 44 三表任一并集内 ⇒ 交付态实测 105 ≠ 声明 104），装置整条迁入 `api/mod_test.rs` 作 `pub(crate) fn declare_ancestors`，`api/mod.rs` 的 `#[cfg(test)] pub(crate) mod test_support;` 声明撤回；四处引用改指 `crate::api::mod_test::declare_ancestors`（`api/mod_test.rs` 与 `api/paths_test.rs` 的 `use` 行、`api/extract/{mod,ts}_test.rs` 的全路径调用）。**用例集零变化**（测试相位仍 12 组 / 44 文件，两数不变）。
  - b) **`emit::rust::variant_name` 从公共函数表撤行**：盘上形态即私有 `fn`，按 design「公共函数 / API」小节纪律①「私有工序一律不入表」处理（`emit/rust.rs` 行的行数说明由「3 行」改「2 行」：`render` / `rust_comment_text`）；**实现形态逐字不动**（`fn variant_name` 仍私有，D45 闸二的 `grep -c "fn variant_name"` ⇒ 1 不受影响）。
  - c) **判据（现算，MUST NOT 写死行号）**：`grep -rnF 'test_support' --include=*.rs packages/desktop/src-tauri/crates packages/desktop/src-tauri/src` ⇒ **0**；`grep -cF 'fn declare_ancestors' packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs` ⇒ **1**；`grep -cF 'pub fn variant_name' packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs` ⇒ **0**、`grep -c 'fn variant_name'` 同文件 ⇒ **1**；`sed -n '/^### 公共函数 \/ API$/,/^### 类型定义$/p' design.md | grep -c '^| `\|^| \*\*`'` ⇒ **59**；`git status --porcelain -uall` 扣除本 change 目录后 **104 行**（= 清单 42 + 机械随动 18 + 测试相位 44，`comm -3` 双向零行）。
- [x] **⑧ 出口面按「零消费者即不入出口」收窄到底（D60 同口径的射程补全；本相位执行）**
  - a) **`api/mod.rs` 的 `pub use` 面归零**：`pub use contract::{Contract, ContractOp, ContractSection};` / `pub use emit::{EmitTarget, ModuleRef};` / `pub use extract::{schema_type, signature_of, ApiParam, ApiSymbolKind, ApiType, Position, Visibility};` / `pub use schema::{build_index, build_schema, verify_defs};` 四行**整组删除**，按本文件自用面改写为普通 `use`：`use contract::Contract;` / `use emit::{EmitTarget, ModuleRef};` / `use extract::{ApiSymbol, ApiSymbolKind, ApiType};` / `use schema::{build_index, ApiFormStatus, IndexEntry};`（原 `use extract::ApiSymbol;` 与 `use schema::{ApiFormStatus, IndexEntry};` 两行并入，不重复声明）。**保留面**只留本文件真消费者：`Contract`（`ApiSnapshot` / `ApiCheckInput` / `module_refs`）、`EmitTarget` + `ModuleRef`（`render_all` / `module_refs`）、`ApiSymbolKind` + `ApiType`（`bootstrap_text` / `json_type`）、`build_index`（`render_all`）。
  - b) **零消费者九名删除**（`grep -rn` 全仓实测无 `api::<名>` / `super::<名>` 形态消费者）：`ContractOp` / `ContractSection`（`contract`）、`schema_type` / `signature_of` / `ApiParam` / `Position` / `Visibility`（`extract`）、`build_schema` / `verify_defs`（`schema`）——消费方一律走 `contract::` / `extract::` / `emit::` / `schema::` 模块路径。**同口径复核**：`build_schema` / `verify_defs` 未被 `lib.rs` 出口（`pub use api::{…}` 名单内零命中），故删除不缩小 crate 外可用面。
  - c) **机械随动（D53 第四类，射程封闭为「被删重导出的调用形态适配」）**：`api/mod_test.rs` 的 `super::build_schema(` / `super::verify_defs(` 两处与 `api/schema_test.rs` 的 `use super::{build_index, build_schema, verify_defs, …}` 一行改走 `super::schema::` 模块路径（与 `check_test.rs` 的 `use super::check::{category_of, check}` 同型，D60 先例）；**用例名 / 断言值 / 断言意图零改动**，两文件已在测试相位 44 文件集内计数，不增白名单项。
  - d) **判据（现算，MUST NOT 写死行号）**：`grep -cE '^pub use ' packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` ⇒ **0**；九名在 `api/mod.rs` 的**行首 `use` 声明面** 0 命中（`schema::build_schema` / `schema::verify_defs` 的两处**限定路径调用点**照旧在场——它们不是 import 面，正是「走模块路径」的形态本身）；`grep -cF 'pub use api::display' packages/desktop/src-tauri/crates/core/archi/src/lib.rs` ⇒ **1**（crate 根出口不动）；`cargo check -p archi --all-targets` 零错误、`cargo clippy --workspace --all-targets -- -D warnings` 零告警、`cargo fmt --all --check` exit 0；`cargo test -p archi --lib` 与 `--test pilot_test` 全绿；`cargo run -q -p archi-api-gen -- --check` **exit 0**（纯出口面收窄，四类生成物**逐字节不变**，零行为变化）。
