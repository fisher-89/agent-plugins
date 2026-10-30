---
name: redb
description: 读取和分析 redb（Rust 嵌入式数据库）文件。当用户要求查看、检查、导出、分析 .redb 数据库内容（包括 WPS AI 桌面端的 desktop-global.redb / workspaces/*.redb），或排查 redb 表结构、行数、存储占用、会话与转录数据时使用。触发词：redb、desktop-global.redb、desktop-store.redb、数据库内容、表结构、dump 数据库。
---

# redb

`redb-peek` 是一个只读的 redb 检视工具，专门处理 `native_db` 建出来的库
（WPS AI 桌面端用的就是这个）。它做三件事：枚举表结构与行数、按表导出内容、
在没有 schema 的情况下从二进制载荷里榨出可读字符串。

所有路径都相对于**本 skill 目录**（脚本用 `BASH_SOURCE` / `$PSScriptRoot`
自己定位，不要写死绝对路径）。二进制名在 Windows 上是 `redb-peek.exe`，
macOS / Linux 上是 `redb-peek`。

## 构建

macOS / Linux：

```bash
cd <本 skill 目录>
./build.sh              # 等价于 cargo build --release
```

Windows：

```powershell
Set-Location <本 skill 目录>
cargo build --release
```

产物：`target/release/redb-peek`（Windows 为 `target\release\redb-peek.exe`）。
冷构建约 14 秒。依赖只有 redb 与 serde_json，首次构建会从 crates.io 拉取。

## 核心用法

三个命令，全部只读。

### 1. 看有哪些表

```bash
./target/release/redb-peek <db.redb> tables
```

输出每张表的：表名、类型（`table` / `multimap` 二级索引）、行数、
键值类型、以及从表名解析出的 native_model 身份（`id` / `version` / 键字段）。

`--jsonl` 输出每表一行 JSON，便于脚本处理。

### 2. 看存储占用

```bash
./target/release/redb-peek <db.redb> stats
```

每表行数、叶子页数、存储字节、碎片字节。

### 3. 导出表内容

```bash
./target/release/redb-peek <db.redb> dump <table> [选项]
```

| 选项 | 作用 |
|---|---|
| `--limit N` | 最多输出 N 行（默认全部） |
| `--offset N` | 跳过前 N 行（在 grep 过滤之后计） |
| `--hex` | 键/值一律十六进制（`--grep` 仍按 UTF-8 匹配） |
| `--strings` | 从二进制载荷里榨出全部字符串 |
| `--min-str N` | `--strings` 的最短长度（默认 4） |
| `--max-bytes N` | 单值预览上限（默认 512） |
| `--grep TEXT` | 只保留包含该文本的行（**区分大小写**） |
| `--jsonl` | 每行一个 JSON 对象 |

工具有三种解码策略，按顺序自动尝试：

1. **JSON 载荷** —— native_db 里用 serde_json 编码的表（WPS 的转录事件表
   `8_1_event_key` 就是这种），直接输出 JSON。
2. **bincode 字符串提取（默认）** —— 没有 schema 也解不出字段的 bincode 表，
   按 bincode 的 `u64 LE 长度 + UTF-8` 规则把字符串字段全挖出来，输出成
   JSON 字符串数组。**这是分析 bincode 表的主要手段。** 例如会话表
   `7_2_id` 会给出 `["ses-0-…","sdk-0-…","glm-5.3","change","<阶段路径>"]`
   —— id、引擎会话、模型、来源、阶段全在里面。
3. **十六进制** —— 以上都不适用时。`--hex` 强制走这条，看原始字节。

转录表查询示例（`toolUse` 是 JSON 里实际的大小写，`--grep` 区分大小写）：

```bash
./target/release/redb-peek <ws.redb> dump 8_1_event_key --grep toolUse --limit 2 --max-bytes 200
# → 0x00 05 a7 0c 25 61 ed 35 ... 0d  =>  {"event":{"blocks":[{"kind":"thinking",...
#      {"id":"call_00_...","input":{"command":"ls openspec/changes/..."},
#       "kind":"toolUse","name":"bash"},...}...  (… bytes total)
# -- 2 shown / 41713 matched / 97145 scanned (table rows = 97145)
```

`matched` / `scanned` 这一行能直接当分析指标用：本机那个库 97,145 条事件里
有 41,713 条含工具调用，整表 grep 约 1.4 秒。

注意 JSONL 模式**不截断**（保证可解析），`--max-bytes` 只作用于人读的表格
输出。真实转录事件单条可达数万字符，看预览时记得配 `--max-bytes`。

## 分析真实数据：批量扫描

macOS / Linux：

```bash
./scan.sh ~/.dev-team
./scan.sh --details ~/.dev-team     # 逐库列出每张表
```

Windows：

```powershell
pwsh -NoProfile -File ./scan.ps1 -Root "$env:USERPROFILE\.dev-team"
pwsh -NoProfile -File ./scan.ps1 -Root <数据目录> -Details   # 逐库列出每张表
```

递归找出目录下所有 `.redb`，逐库汇总。读不了的库（旧格式、被占用）会打印
原因并跳过，不会中断整轮扫描。本机 prod 根实测：7 个库、90 张表、248,627 行，
1.3 秒。

`scan.sh` 依赖 `python3` 解析 `tables --jsonl`（macOS 装了 Xcode 命令行工具就有）；
二进制不存在时会自动跑 `build.sh` 构建。

## 常见分析套路

**看某工作区有哪些模型、各阶段用了什么**
```bash
./target/release/redb-peek <ws.redb> dump 7_2_id --max-bytes 300
# 输出 JSON 字符串数组：["ses-…","sdk-…","glm-5.3","change","<uuid>/proposal/executor/1"]
```

**统计工具调用分布 / 定位某类事件**
```bash
./target/release/redb-peek <ws.redb> dump 8_1_event_key --grep toolUse --jsonl \
  | head -2000 > tools.jsonl
```
`matched / scanned` 行直接给出占比（例：97145 条里 41713 条含工具调用）。

**找某个会话的全部转录**（走二级索引 `8_1_session_id`）
```bash
./target/release/redb-peek <ws.redb> dump 8_1_session_id --grep "ses-9-179274"
# → ses-9-179274…  =>  0x00 05 a7 0c 25 61 ed 35 00 00 00 00 00 00 00 00
```
主键就是 `8_1_event_key` 的键，拿它回查事件表。

**轮统计里只有会话 id，没有状态字面量**
```bash
./target/release/redb-peek <ws.redb> dump 2_4_id --limit 5
# → 0x00 00 00 00 00 00 00 04  =>  [2:4] ["ses-1-1791716952501"]
```
轮统计的状态 / 成本 / 时长是 bincode 的**枚举序号和 f64**，没有字面量可
grep——要判断失败轮，得 `--hex` 看原始字节再对 schema。字符串提取只对
真正含文本的字段（如 `error`）有输出。

## WPS AI 桌面端的数据在哪

数据根由 `packages/desktop/src-tauri/src/env.rs` 决定，**不是** tauri identifier
对应的 `%APPDATA%` 目录：

| 构建 | 数据根 |
|---|---|
| prod（release） | `~/.dev-team` |
| dev（debug） | `~/.dev-team/dev`（可用 `DEV_TEAM_DATA_ROOT` 覆盖） |

`~` 是用户主目录，Windows / macOS 同形。

布局分两层：

```
<数据根>/desktop-global.redb          ← 全局库：workspace 注册表 + agent 实例
<数据根>/workspaces/<name>-<hash>.redb ← 每 workspace 一个库：会话 / 转录 / change / run
```

直接扫 prod 根：

```bash
./scan.sh ~/.dev-team                          # macOS / Linux
pwsh -NoProfile -File ./scan.ps1 -Root "$env:USERPROFILE\.dev-team"   # Windows
```

本机实测（2026-10-30）`~/.dev-team/workspaces/wps-claude-plugin-*.redb`
有 97,145 条转录事件、1,273 条轮统计、724 个会话——扫描 22.8 万行约几秒。

workspace 库里的业务表（按 native_model id 认）：

| 表 | 内容 |
|---|---|
| `7_2_id` | 会话（SessionRecord）：id、引擎会话、模型、来源、阶段 |
| `8_1_event_key` | 转录事件（**JSON 载荷，可直接读**） |
| `2_4_id` | 轮统计（AgentRunRecord）：状态、成本、时长 |
| `9_4_id` | change 记录 |
| `15_1_key` | 库格式版本标记 |
| `8_1_session_id` 等 | 二级索引表（multimap），`change_id` / `run_id` 反查用 |

全局库按 model version 多头并存（`5_1_id` … `5_4_id` 是同一 model 的历史
版本表），不是重复数据——`tables` 会把每张表的 `id`/`v` 都标出来。

## 坑（都是实测踩到的）

- **应用运行时根本读不了。** redb 在 Windows 上用 `LockFile` 锁住整个文件
  范围，这是**强制锁**：应用运行时别的进程连打开文件头、连 `copy` 都被拒
  （`os error 33`）。工具的"拷贝快照"回退在这个平台上救不了你——唯一解法
  是**先完全退出应用**。工具会明确告诉你这一点，不要误以为是自己命令写错了。
  macOS 上 redb 走 `flock`（劝告锁），读不受影响，被占用时会自动拷贝快照再读。
- **叫 `desktop-store.redb` 不代表读不了——看头部，不看名字。** 本机有两个
  同名文件：`~/.dev-team/desktop-store.redb` 是正常的 redb 2.x 库（20,108
  行，含旧的 run/event 表，可直接读）；而
  `%APPDATA%\dev.wps.desktop-terminal\desktop-store.redb` 是 redb 1.x 布局
  （文件头 `region_header_pages = 0`，文件大小 167 KB 的残骸），redb 2.x
  读它在 layout 断言里直接 **panic**（不是返回错误）。工具按头部判据挡掉
  后者并给退出码 4。**判据是 `region_header_pages`，不是文件名。**
- **键类型名是 `DatabaseInnerKeyValue`，值是 `&[u8]`。** redb 打开表时会
  逐字比对类型名，`native_db` 用的是自定义名，所以通用工具不能直接拿
  `&[u8]` 当键——必须复刻这个名字（见 `src/main.rs` 的 `NativeKey`）。
  写成 `&[u8]` 会报 `TableTypeMismatch`。
- **键"看起来是十六进制"不代表是数字。** `native_db` 的整数主键是
  big-endian，`2_4_id` 的键就是 `0x00…01`；但字符串键直接是 UTF-8。
  工具对非 UTF-8 键自动打 `0x` 前缀并十六进制展开。
- **bincode 载荷长度前缀是 `u64` 小端**，不是 `u32`。按 `u32` 解会把
  中文串整个漏掉（夹具测试就是为了钉住这条）。

## 验证工具本身

```bash
cd <本 skill 目录>
cargo test --test fixture -- --nocapture
```

`tests/fixture.rs` 会造一个 native_db 布局的 fixture 库（JSON 表、
bincode 表、multimap 二级索引表、metadata 表各一），并打印路径。
拿它跑 `tables` / `dump` / `dump --strings` 可以确认三条解码路径都正常，
不需要真实数据。

## 退出码

| 码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | `--help` / 参数用法错误 |
| 2 | 文件不存在 / 不是 redb / 太短 / 命令或参数错误 |
| 3 | 文件被别的进程独占且无法快照（Windows 上 = 应用在运行，先退出应用） |
| 4 | redb 1.x 遗留格式或 redb 内部 panic（格式过旧 / 损坏） |
