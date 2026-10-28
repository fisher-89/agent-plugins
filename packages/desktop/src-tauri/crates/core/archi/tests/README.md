# `crates/core/archi` 成本实测表（AC-18）

本文件是架构漂移判定域的**依赖成本与运行期预算**登记面（`desktop-archi-drift`「运行期
预算与语料」）。三节：阈值（软线 / 硬线）、回退序、实测表。

> 口径：五項**依赖成本**（NSIS 安装包 / 主二进制 / `Cargo.lock` 净新增包数 / CI 冷构建
> 单腿 / 本地增量）由四段式实测（基线 → 变体 → 差分 → 判读，**同一台机器**）产出；运行
> 期**预算**（单次检查 p95）的装置是 `tests/perf.rs`，判据口径为 release
> （`cargo test -p archi --release --test perf -- --nocapture`）；数字已回填（见「三、实测表 ·
> 运行期预算」）。
>
> 分工：五项中**仅 `Cargo.lock` 净新增包数**可由实现相位自证（数锁文件，零构建、零测试
> 命令，已回填）；其余四项依赖成本**显式移交发布实测**（具 release 打包环境的相位或人工
> 实测——CI 冷构建单腿无法本地产生），p95 在 acceptance 前按 release 口径补跑回填（test-execution
> 门禁实跑为 debug 口径，不作判据）。实现相位只登记版本
> 钉定事实与未测移交标注，MUST NOT 执行测试门禁、MUST NOT 以本地临时跑值充数。
> 本文件表格与 `openspec/changes/archi-check-and-gate/design.md` §6 可自证项 / 未测文本
> **逐字一致**（含未测文本一致），改任一处必须同批改另一处。

## 一、阈值

| 指标 | 软线 | 硬线 | 备注 |
|------|------|------|------|
| NSIS 安装包 | +0.8 MB | +1.5 MB（≤ 8.2 MB） | 越硬线触发回退序 |
| 主二进制 | +3 MB | +5 MB（≤ 31.6 MB） | 同上 |
| `Cargo.lock` 净新增包数 | ≤ 25 | ≤ 40 | oxc 家族是唯一真新增面 |
| CI 冷构建（单腿） | +45 s | +90 s | 共享 runner 抖动会淹没差值——不常态化跑 |
| 本地增量（改 `archi` 一文件） | +8 s | +15 s | 同上 |
| 单次检查（300 元素 / 1000 源文件） | 1.5 s | 3 s（p95，release） | `std::time::Instant` 计时，零统计框架 |

## 二、回退序（代价递增；越硬线即按序执行并留痕）

1. 收 `oxc_parser` 特性（只 parse，不引 semantic / codegen）。
2. 自算路径替代 `oxc_resolver`（约 150 行；`exports` / `imports` 退化）。
3. 正则 import 替代 `oxc_parser`（`typeOnly` 与行号退化，且实质推翻「type-only 算
   架构依赖」的裁决——**必须回到设计讨论**，MUST NOT 实现层自定）。

每次回退后重测，并把两轮数据一并留在下表。

## 三、实测表

### 版本钉定（本相位可确证的部分）

| 依赖 | 钉定值 | 判据 |
|------|--------|------|
| `oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax` | `=0.146.0` | 家族同版发布面中**本工具链可用的最新同版**（`rustc 1.95.0`；0.147.0 起 `rust-version = 1.96.0`、0.153.0 起 1.97.0——超出本仓工具链）。`oxc_ast` / `oxc_span` / `oxc_syntax` 是 `oxc_parser` 的同版伴随件（`ParserReturn.program` / `SourceType` 需直接引用其类型），声明为直接依赖**零新增锁包** |
| `oxc_parser` 特性面 | `default-features = false` | 只开 parser 所需（不引 `regular_expression` / semantic / codegen）——回退序 ① 已在钉版时执行 |
| `oxc_resolver` | `=11.24.3` | 独立版本线（11.x，与 oxc 家族不同版），取本工具链（`rust-version = 1.95.0`）可用的最新值 |
| `syn` | `2`（`features = ["full"]`） | 锁内既有 2.0.119，零新增包 |
| `proc-macro2` | `1`（`features = ["span-locations"]`） | 锁内既有；`syn` 的 `use` 项行号依赖该特性（fallback span 行列记录） |
| `globset` | `0.4` | 锁内既有 0.4.20（由传递依赖提为直接） |
| `sha2` / `time` | 既有收敛项 | `modelHash` / `timestamp`；锁内已有消费者 |

> **件数（实测 `crates/core/archi/Cargo.toml` 与 `[workspace.dependencies]` 逐项对齐）**：oxc 家族直接依赖共 **6 件**（上表第一行为 `oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax` 五件同版 + `oxc_resolver` 一件独立版本线），另加 **`proc-macro2`**（`syn` 的 span 行号伴随件，锁内既有、随 `span-locations` 特性提为直接）——「三件」只是「真新增锁包的三件」（parser / allocator / resolver），直接依赖面 MUST NOT 按三件计。

### 依赖成本（四段式实测）

| 指标 | 基线（实测） | 变体（实测） | 差分 | 判读 |
|------|--------------|--------------|------|------|
| NSIS 安装包 | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 本仓无 release 打包环境；MUST NOT 以临时跑值充数 |
| 主二进制 | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 同上 |
| `Cargo.lock` 净新增包数 | 998 包（HEAD） | 1038 包（含本变更） | **+40**（34 个新包名 + 版本分裂条目；其中 `archi-runtime` 为本仓新 crate ⇒ 外部包 39） | 越软线（≤ 25）、**正好落在硬线（≤ 40）内**；回退序 ① 已在钉版时执行（`oxc_parser` `default-features = false`），再往下即回退序 ② |
| CI 冷构建（单腿） | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 同上 |
| 本地增量（改 `archi` 一文件） | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 同上 |

> `oxc_*` / `syn` 的引入是本变更唯一真新增依赖面；若锁包数越硬线（≤ 40），按回退序 ①
> 先收紧特性再复测。

### 运行期预算

| 指标 | 软线 | 硬线 | 实测 p95 | 数据来源 |
|------|------|------|----------|----------|
| 单次检查（300 元素 / 1000 源文件，release） | 1.5 s | 3 s | **0.62 s**（617.4 ms，release，n = 10；p50 596.3 ms） | `tests/perf.rs`（10 次取 p95） |

> 判读：`cargo test -p archi --release --test perf -- --nocapture`（cargo 1.95.0，本机）实测
> 617.4 ms，**落在软线（1.5 s）内**，距硬线（3 s）余量约 4.9 倍；不触发回退序任一步。样本
> （ms）：`601.5 / 605.0 / 592.8 / 588.4 / 583.5 / 617.4 / 593.7 / 613.5 / 596.3 / 612.5`。
> 回填时点：acceptance 前（test-execution 门禁只跑到 debug 口径，其连通性样本不作判据）。
