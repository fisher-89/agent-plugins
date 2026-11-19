# App.Backend.Core.Foundation

## 职能

工作区磁盘布局的派生单点：把「根目录 + 域段 + 报告槽位」翻译成具体路径，并承载与路径
同源的三项纯判定（repo 相对路径、依赖产物树剪枝名单、Windows 保留设备名）。

五块公开面各占一个子模块：

- `layout`：布局派生（进行中 / 已归档 / 探索笔记三棵目录树、`reports/<槽位>` 三后缀、
  架构域五级目录）与四条 repo 相对路径串。
- `identity`：workspace 身份段 `{可读段}-{hash}`——按工作区分库 / 分目录的唯一命名来源。
- `prune`：依赖与工具产物树名单 + dot 剪枝判定（探测不入清单、清理不删依赖共用同一集）。
- `windows_name`：Git for Windows 保留设备名集合与段级判定。
- `cancel`：取消原语两半——`CancelFlag`（读端，可克隆共享）与 `CancelHandle`（写端）。

**边界**：零子进程、零版本控制、零库句柄、零工作区配置读取、零 tokio。依赖面只有
`sha2`（唯一用途是 `identity` 段的哈希），无任何 workspace 内 crate。

## 承载理由

- **段名字面量单点**：`openspec` / `.dev-team` / `architecture` / `models` / `graph` /
  `model` / `reports` / `test` / `baseline` / `archi` / `config.json` 各只有一份持有者。
  消费方 MUST 经本模块派生、MUST NOT 行内拼段——行内拼一处，目录搬家就从「改一个常量」
  变成「全仓搜索」。
- **「第二个消费者到场才下沉」**：身份段自 store 下沉、产物树名单自 checks 下沉、取消
  原语自编排与 exec 两消费者下沉——三次都是正触发。未到第二个消费者的通用 fs 助手 /
  错误类型刻意不铺（本 crate 因此保持极小）。
- **取消原语 std-only**：无 tokio ⇒ 不能引 `Notify` / `watch`，故取消以 `Arc` +
  `AtomicBool` 表达、观测方按定间隔轮询；异步等待面（停等 / 应答）仍归编排侧既有通道，
  两道并行、互不替代。

## 不变量

- 派生函数**纯推导零 IO**：不读盘、不建目录、不 spawn。存在性判定一律归消费方
  （`is_file` / `create_dir_all` 不在本模块发生），故「不存在的 root」也能正常派生。
- 上层 MUST NOT 拼段名与文件名：配置文件名、报告槽位后缀、架构域五级段全部只在此处
  出现一次。
- 报告槽位是**进程内封闭三值**（`Test` / `Baseline` / `Archi`），零 serde、不进 IPC；
  `checks` 侧两个 runner MUST NOT 接受 `Archi` 槽位（漂移报告与检查报告分树隔离）。
- `CancelFlag` 的相等是**身份比较**（共享同一底层原子才相等），不是状态比较；置位幂等，
  重复置位无副作用。
- `windows_name` 的判据按**段**、ASCII 大小写不敏感，段内首个 `.` / `:` 前的词干命中即
  真（扩展名与 ADS 后缀不改变保留性），尾随空格按 Win32 归一语义剥除。

## 操作

契约词汇（JSON Schema + `x-function`）只表达值形态：没有 path 位、也没有具名进程内类型
位。本模块 25 个非构造器公开操作里 9 个是纯标量面，逐条列在下面；其余 16 个按「契约面
留白」登记（构造器 `CancelFlag::never` 按纪律不进契约，不计入这 25 个）。

`owner` 键取**方法归属类型**（`CancelFlag`）或**自由函数所在子模块段**（`layout` /
`identity` / `prune` / `windows_name`）——它唯一的消费面是生成物里的签名见证形
`super::<owner>::<op-id>`，故取的是「`super::` 之后的第一段」，不是 FQN 段。

### is_archi_rel_path

repo 相对路径是否落在架构域之内（`/` 与 `\` 分隔均归一后判定）。归档冲突分流的唯一
判据——消费方 MUST NOT 自行比较架构域段名字面量。

```json api-op
{
  "owner": "layout",
  "params": [{ "name": "rel", "schema": { "type": "string" } }],
  "returns": { "type": "boolean" }
}
```

### archi_rel_dir

架构域根的 repo 相对路径（`<顶层>/architecture`）。`/` 分隔、零前导 `./`——直接进模型
可见文本，故不接 root 入参。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### archi_models_rel_dir

契约资产目录的 repo 相对路径（`<archi_rel_dir>/models`）；全从 `archi_rel_dir` 派生，
架构域段名不二次出现。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### archi_graph_rel_dir

契约索引目录的 repo 相对路径（`<archi_rel_dir>/graph`）；`INDEX.toml` 说明段的
「契约面：目录」串取它，MUST NOT 取索引自身所在目录。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### archi_graph_model_rel_dir

模型声明目录的 repo 相对路径（`<archi_graph_rel_dir>/model`）；消费者限于架构会话知识
文本。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### workspace_identity_segment

workspace 身份段：`{可读段}-{hash}`，可读段清洗后为空则退化为纯 `{hash}`。哈希 =
canonical root 的 SHA-256 前 16 字节（32 位小写 hex），异根必不同名。

```json api-op
{
  "owner": "identity",
  "params": [{ "name": "canonical_root", "schema": { "type": "string" } }],
  "returns": { "type": "string" }
}
```

### is_pruned_dir_name

目录名是否属于「依赖与工具产物树」（dot 前缀或名单命中）。探测剪枝与 clean 排除共用的
同一判定——名单扩展一处修改双点生效。

```json api-op
{
  "owner": "prune",
  "params": [{ "name": "name", "schema": { "type": "string" } }],
  "returns": { "type": "boolean" }
}
```

### is_reserved_path

路径是否含 Windows 保留设备名段（任意段、ASCII 大小写不敏感）。写入面拒收的唯一判据；
`COM0` 不保留（git 允许）、`LPT0` 保留。

```json api-op
{
  "owner": "windows_name",
  "params": [{ "name": "path", "schema": { "type": "string" } }],
  "returns": { "type": "boolean" }
}
```

### is_set

取消标志是否已置位（`Relaxed` 读：仅「置位后可见」语义）。

```json api-op
{
  "owner": "CancelFlag",
  "params": [],
  "returns": { "type": "boolean" }
}
```

## 契约面留白

下列 16 个操作**已在代码里成立、但未进 `api-op`**：契约词汇没有 path 位与具名进程内类型
位，写出来两侧归一必然不相等（`apiSignatureMismatch` 恒红），故只在此登记、不进契约面。
补齐的出路是扩契约词汇（path 位 + `$ref` → 具名类型），不是放宽判据。

- 入参 `&Path` / 返回 `PathBuf` 的 13 个派生函数：`resolve`、`config_path`、
  `change_reports`、`change_reports_in`、`change_test_reports`、`change_test_reports_in`、
  `change_baseline_reports`、`change_baseline_reports_in`、`archi_dir`、`archi_models_dir`、
  `archi_config_path`、`archi_graph_dir`、`archi_graph_model_dir`。
- 返回借用字符串的 `domain_dir_name`（返回位只能表达自有 `String`）。
- 返回具名类型的 `CancelFlag::handle`（`CancelHandle`）。
- 零返回的 `CancelHandle::set`（代码侧零返回归一为 `?()`，与契约侧 `()` 不等）。

## 类型

四个进程内类型（`Layout` / `ReportSlot` / `CancelFlag` / `CancelHandle`）**不进
`api-types`**：它们零 serde、不进 IPC，且不出现在任何已声明操作的签名里——按「第一层
`properties` + 经 `$ref` 可达闭包」的公共面口径，本模块的 `$defs` 为空。它们是承载
路径结果与取消信号的**进程内载体**，其形状由代码而非契约负责。

## 生成物接线（尚未接）

`lib.i.rs` 已由契约派生到入口同级（与 `src/lib.rs` 同目录），但 `lib.rs` **未加**
`#[path = "lib.i.rs"] mod api_gen; pub use api_gen::*;`——故该文件当前是派生记录、零编译期
绑定。未接的原因是 `$defs` 为空（生成物只有 9 行签名见证、零类型定义），且
`is_set` 的见证形 `super::CancelFlag::is_set` 要求 crate 根再导出 `CancelFlag`，本模块不
为「让生成物可编译」而增开再导出面。接线触发条件：本模块开始消费生成类型（当前为零），
届时连同再导出一起做。
