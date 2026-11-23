# Backend.Core.Foundation

## 职能

工作区在磁盘上的目录布局只在这里算一次：把「根目录 + 各层目录名 + 报告槽位」拼成具体
路径。另有三项判断跟路径同源，也放在这里——一个字符串是不是仓库内的相对路径、哪些目录
属于依赖产物、Windows 上哪些名字不能拿来当路径。

五块公开面各占一个子模块：

- `layout`：目录布局——进行中 / 已归档 / 探索笔记三棵目录树，`reports/<槽位>` 对应的三个
  子目录名，架构域的五层目录（域根 / 契约资产 / 架构域配置 / 契约索引 / 模型声明），另有
  四条相对仓库根的路径串、工作区配置文件的位置、域根目录名。
- `identity`：workspace 身份串 `{可读部分}-{hash}`——按工作区分库、分目录时的唯一命名来源。
- `prune`：依赖与工具产物的目录名单，外加「以 `.` 开头就跳过」那条判断（探测时不进清单、
  清理时不删依赖，两处用同一份名单）。
- `windows_name`：Git for Windows 的保留设备名清单，以及按路径逐层判断的检查。
- `cancel`：取消的两半——`CancelFlag`（读端，克隆后共享同一个标志）与 `CancelHandle`
  （写端）。

**边界**：不起子进程、不碰版本控制、不持数据库句柄、不读工作区配置、不带异步运行时。
依赖只有 SHA-256 hash实现（只给 `identity` 用），不依赖工作区内任何其它模块。

## 承载理由

- **不依赖工作区内任何模块，这是唯一的准入门槛**：它存在的意义是给依赖图留一个汇点——谁都
  可以依赖它，它不依赖谁，因此不可能绕出环来。凡是需要引第三方库才能做的事，不管有几个
  调用方，都不进这里：早先想把通配符匹配搬进来，就因为要引一个匹配库而被拒；外部依赖也只
  保留了一个哈希实现。「有人用第二次才搬下来」只回答「什么时候搬」，不回答「能不能进来」
  ——后者全看这一条。
- **目录名与文件名只写一次**：工作区布局用到的全部名字——`openspec` / `changes` / `archive`
  / `explores` / `reports` / `test` / `baseline` / `archi` / `.dev-team` / `architecture` /
  `models` / `graph` / `model` / `config.json`——各自只有一处定义。上层一律经本模块取路径，
  MUST NOT 自己拼：自己拼一处，目录改名就从「改一个常量」变成「全仓搜索」。`.dev-team` 在
  应用侧还有另一个同名不同义的定义（本机数据根，与工作区布局无关），两者 MUST NOT 混用。
- **有人用第二次才搬下来**（只管时机，不管准入）：身份串原本在持久层、产物目录名单原本在
  检查域、取消两半原本分散在编排与执行体、架构域那几个路径原本各自在调用方手上——都是等到
  第二个调用方出现才搬到这里。没人用第二次的通用文件助手、通用错误类型刻意不铺，本模块因此
  保持很小。
- **取消只依赖标准库，这是门槛的后果而非风格偏好**：拿不到异步运行时的通知 / 订阅通道，
  取消只能写成「共享一个原子标志 + 需要时按固定间隔来查」。异步的等待（停等、应答）仍走
  编排侧原有通道，两条路并行，谁也替代不了谁。

## 不变量

- 拼路径的函数**只做字符串运算**：不读盘、不建目录、不起子进程。某个目录在不在，由调用方
  自己判断，所以「根目录还不存在」也能正常拿到路径。
- 上层 MUST NOT 自己拼目录名和文件名：配置文件名、报告槽位的三个后缀、架构域的五层目录，
  都只在这里出现一次。
- 报告槽位只有三个取值（`Test` / `Baseline` / `Archi`），只在本进程内用，不做序列化、不进
  进程间调用；`checks` 侧的两个执行体 MUST NOT 接受 `Archi` 槽位（漂移报告与检查报告分开
  存放）。
- `CancelFlag` 的相等看的是「是不是同一个标志」（共享同一个底层原子才相等），不看「当前有
  没有被置上」；置上可以重复，重复置没有副作用。
- `windows_name` 逐层判断、ASCII 大小写不敏感：每一层里第一个 `.` 或 `:` 之前的部分命中
  就算（扩展名、`:` 之后的流名都不改变结论），行尾空格先按 Win32 的归一规则剥掉。

## 操作

契约词汇（JSON Schema + `x-function`）只能表达值的形态：没有「路径」这个位置，也没有
「引用一个已经存在的类型」的位置——`$ref` 只能指向本模块自己的 `$defs`，生成物是照着它
**新建**类型。本模块 25 个非构造器公开操作里，9 个只涉及标量，逐条列在下面；其余 16 个
算「契约面留白」（构造器 `CancelFlag::never` 按纪律不进契约，不计入这 25 个）。

`owner` 键填的是「这个方法挂在哪个类型上」（`CancelFlag`）或「这个函数在哪个子模块」
（`layout` / `identity` / `prune` / `windows_name`）——它唯一的用处，是在生成物里做编译期
签名核对时拼出 `<owner>::<op-id>` 这个限定写法。所以填类型名或子模块名，不填 FQN。

### is_archi_rel_path

一个相对仓库根的路径是否落在架构域之内（`/` 与 `\` 都先归一；判据：剥掉前导 `./` 与末尾
分隔符后，前两层依次是架构域顶层目录名与架构目录名）。归档遇到冲突时，判断「这批冲突里有
没有模型文件」只有这一条依据——调用方 MUST NOT 自己去比较目录名的字面量。

```json api-op
{
  "owner": "layout",
  "params": [{ "name": "rel", "schema": { "type": "string" } }],
  "returns": { "type": "boolean" }
}
```

### archi_rel_dir

架构域根相对仓库根的路径（`<顶层>/architecture`）。`/` 分隔、没有前导 `./`——用到它的地方
都拿它做字符串前缀比对，不落到磁盘路径，所以不接根目录入参。它自己没有直接调用方：只是
契约资产 / 契约索引 / 模型声明三条路径的共同前缀，架构域那两个目录名因此只写一次。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### archi_models_rel_dir

契约资产目录相对仓库根的路径（`<archi_rel_dir>/models`）。三处在用：契约索引 `INDEX.toml`
说明段里的「契约面：目录」（MUST NOT 写成索引自己所在的 `graph` 目录）、架构会话的知识
文本、写保护面判断「这次写是不是落在架构域里」。整个串从 `archi_rel_dir` 接出来，架构域的
两个目录名不再重复。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### archi_graph_rel_dir

契约索引目录相对仓库根的路径（`<archi_rel_dir>/graph`）；与契约资产目录同级相邻，MUST NOT
混用。用在写保护面判断「这次写是不是落在架构域里」；模型声明的路径从它接出来（`graph`
这个名字不再重复）。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### archi_graph_model_rel_dir

模型声明目录相对仓库根的路径（`<archi_graph_rel_dir>/model`）；架构会话知识文本里的「模型
目录」和扫描排除都用它，调用方 MUST NOT 自己拼 `model`。

```json api-op
{
  "owner": "layout",
  "params": [],
  "returns": { "type": "string" }
}
```

### workspace_identity_segment

workspace 的身份串：`{可读部分}-{hash}`，可读部分清洗完为空时只剩hash。可读部分取根目录
最后一层的名字（非法字符换成 `_`、按字符截到 24 个、去掉末尾的 `.` 与空格）；hash取根目录
字符串（调用方保证已规范化）的 SHA-256 前 16 字节，32 位小写十六进制，128 位长度，不同根
不会撞名。

```json api-op
{
  "owner": "identity",
  "params": [{ "name": "canonical_root", "schema": { "type": "string" } }],
  "returns": { "type": "string" }
}
```

### is_pruned_dir_name

一个目录名是否属于「依赖与工具产物」（以 `.` 开头，或命中那份名单）。探测时跳过、清理时
排除，用的是同一条判断——名单改一处，两个地方同时生效。

```json api-op
{
  "owner": "prune",
  "params": [{ "name": "name", "schema": { "type": "string" } }],
  "returns": { "type": "boolean" }
}
```

### is_reserved_path

路径里是否含有 Windows 保留设备名的某一层（任意一层，ASCII 大小写不敏感）。写入面拒收时
只有这一条依据；`COM0` 不算保留（git 允许）、`LPT0` 算。

```json api-op
{
  "owner": "windows_name",
  "params": [{ "name": "path", "schema": { "type": "string" } }],
  "returns": { "type": "boolean" }
}
```

### is_set

取消标志有没有被置上（宽松内存序读：只保证「置上之后再读能看见」，不承担其它同步作用）。

```json api-op
{
  "owner": "CancelFlag",
  "params": [],
  "returns": { "type": "boolean" }
}
```

## 契约面留白

下面 16 个操作**代码里已经有了，只是没进 `api-op`**：契约词汇只表达值的形态——没有「路径」
这个位置，也没有「引用已有类型」的位置，硬写进去两边的签名一定对不上
（`apiSignatureMismatch` 永远是红的），所以只在这里登记、不进契约面。要补齐得先扩契约词汇
（加路径位、加已有类型的引用位），而不是把判据放松。

- 入参 `&Path` / 返回 `PathBuf` 的 13 个拼路径函数：`resolve`、`config_path`、
  `change_reports`、`change_reports_in`、`change_test_reports`、`change_test_reports_in`、
  `change_baseline_reports`、`change_baseline_reports_in`、`archi_dir`、`archi_models_dir`、
  `archi_config_path`、`archi_graph_dir`、`archi_graph_model_dir`。
- 返回借用字符串的 `domain_dir_name`（返回位只能表达自己拥有的 `String`）。
- 返回具名类型的 `CancelFlag::handle`（`CancelHandle`）。
- 零返回的 `CancelHandle::set`（代码这边零返回记成 `?()`，契约那边是 `()`，对不上）。

## 类型

四个只在本进程内传递的类型（`Layout` / `ReportSlot` / `CancelFlag` / `CancelHandle`）**不进
`api-types`**：它们不做序列化、不进进程间调用，也不出现在任何已声明的操作签名里——按「第一
层 `properties` 加经 `$ref` 能到达的那些类型」这个公开面范围，本模块的 `$defs` 是空的。它们
是路径结果与取消信号的容身之处（只在进程内传，不出去），形状由代码负责，契约不管。

因此本模块的生成物只有签名核对、没有类型定义；入口目前也没接生成物，两者之间没有编译期
绑定。要接的前提是这些类型在入口处可见（`is_set` 的核对是按入口路径去找的），本模块不会
为了「让生成物能编译」多开导出面。
