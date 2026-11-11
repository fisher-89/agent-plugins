# desktop-file-read Specification

## Purpose

定义前端唯一的本地文件内容读取命令：`read_file` 的字节窗口、UTF-8 边界对齐、binary 探测、单次大小上限与同批 `stat` 语义，以及它替代 `archi_read_doc` / `read_explore` / `read_exec_output` 的替换口径。路径布局知识的派生仍经各域既有命令（如 `explore_doc_path` / `exec_output_path`），本能力不承载目录名知识。

## Requirements

### Requirement: read_file 通用内容读取

命令层 SHALL 提供 `read_file(path, offset, limit) -> Result<Option<FileSlice>, String>`（落 `commands/fs/`，薄包装 + `read_file_inner` 纯函数）：

- `path` SHALL 为文件系统路径，**不限制范围**（不设工作区 root 前缀约束、不跟随关联集）；空白 `path` 显式 `Err`。
- 窗口 SHALL 为**字节**窗口（非行窗口）：`offset` 缺省 0（越界夹到文件尾），`limit` 缺省 `DEFAULT_LIMIT`（1 MiB）并夹到 `[1, MAX_LIMIT]`（8 MiB 硬顶）。
- `FileSlice` SHALL 恰含 `content` / `stat` / `offset` / `nextOffset` / `truncated` / `encoding` 六字段；`stat` SHALL 同批给出 `size` / `modifiedMs` / `isFile` / `isDir`。

#### Scenario: 整读与 stat 同批

- **WHEN** 对一个 UTF-8 文本文件调用 `read_file(path, None, None)`
- **THEN** `content` 逐字节等于文件内容，`stat.size` 等于文件字节数，`stat.isFile = true`，`truncated = false`，`nextOffset == stat.size`

#### Scenario: 分段拼接等于全文

- **WHEN** 以固定小 `limit` 逐次调用 `read_file`，每次以上次的 `nextOffset` 作 `offset`，直至 `truncated = false`
- **THEN** 各段 `content` 顺序拼接与整读结果逐字节相等（无重无漏）

### Requirement: UTF-8 边界对齐与二进制探测

`read_file` SHALL 保证窗口内容落在字符边界：入参 `offset` 落在多字节字符中间时 SHALL **前向对齐**（跳过续接字节），返回值 `offset` 为实际起点；窗口尾部残缺的多字节序列 SHALL 整段丢弃（不产出替换符），`nextOffset` 恒落在字符边界。

窗口内容 SHALL 判定 `encoding`：含 NUL 字节、或（窗口 ≥ 4 字节时）整段即非法 UTF-8 起始 ⇒ `encoding = "binary"`、`content` 为空串、`truncated = false`（`stat` 仍给真实大小）；否则 `encoding = "utf8"`。`encoding` 为开放词表（后续可加 `"base64"` 而不破契约）。

#### Scenario: 偏移落在多字节中间

- **WHEN** 文件以三字节字符开头，以 `offset = 1` 或 `2` 调用
- **THEN** 实际 `offset` 前向对齐到该字符之后，`content` 从下一字符起，不产出替换符

#### Scenario: 尾部残缺序列

- **WHEN** 窗口在某个多字节字符中途截断
- **THEN** 该残缺尾序列整体丢弃，`content` 不含替换符，`truncated = true`，`nextOffset` 落在完整字符边界

#### Scenario: 二进制文件

- **WHEN** 对含 NUL 字节或首段即非法 UTF-8 的文件调用
- **THEN** `encoding = "binary"`、`content = ""`、`truncated = false`；`stat.size` 仍为真实字节数

### Requirement: 目录与缺失语义

目标为目录时 SHALL 返回 `Ok(Some(slice))`，其中 `content` 为空串、`nextOffset = 0`、`stat.isDir = true`（不报错）。路径不存在 SHALL 返回 `Ok(None)`（「无内容可读」，非错误）；其余 I/O 故障（权限 / 设备错误）SHALL 显式 `Err`——真故障与空态可分辨。

#### Scenario: 目录与缺失

- **WHEN** 分别以目录路径与不存在的路径调用
- **THEN** 前者返回带 `isDir = true` 的空内容切片；后者返回 `Ok(None)`

### Requirement: 替换面（archi_read_doc / read_explore / read_exec_output）

`read_file` SHALL 成为前端唯一的内容读取命令；下列命令在同一变更内退役：

- `archi_read_doc`（关联集成员校验 + 读盘）→ 前端改「调用方拼绝对路径 → `read_file`」；
- `read_explore`（explore 笔记专用读取）→ 前端改「`explore_doc_path` 派生路径 → `read_file`」；
- `read_exec_output`（零消费者死命令）→ 直接删除（`exec` crate 的库函数 `read_exec_output` / `read_exec_output_in` 保留，仍服务 `checks` crate）。

保留不改的读取面：`read_artifact`（领域信封解析，非纯文件读）、`read_exec_command` / `read_exec_output_delta` / `exec_output_path`（exec 记录专用）、`watch_subscribe` / `watch_unsubscribe`（失效信号通道，非内容读取）、agent 的 `read` 工具（workspace 沙箱内，归 `desktop-agent-execution`）。

#### Scenario: 文档预览链路

- **WHEN** 架构抽屉 / explore 预览需要渲染文档正文
- **THEN** 均经 `read_file` 取字节，不再出现 `archi_read_doc` / `read_explore` 调用；`archi_module` 的文档关联集与 `explore_doc_path` 的路径派生语义零变化

#### Scenario: 越界不再被拒（按设计）

- **WHEN** 以关联集之外或工作区之外的路径调用 `read_file`
- **THEN** 命令按文件系统直读（不因路径不在某关联集而拒绝）——这是「不限制范围」的既定口径

### Requirement: 安全不变量

`read_file` 不限制路径，故其消费面 SHALL 守住两条不变量：

- markdown 渲染 MUST NOT 注入可执行脚本：前端 MUST NOT 引入 `rehype-raw` 一类 raw-HTML 插件、MUST NOT 对读取内容使用 `dangerouslySetInnerHTML`；
- 单次读取 SHALL 受 `MAX_LIMIT` 约束（8 MiB 硬顶），防止一次性把超大文件读入 WebView 内存。

#### Scenario: raw HTML 不执行

- **WHEN** 被读取的 markdown 文件含内联 `<script>`
- **THEN** 预览按纯文本 / 转义呈现（react-markdown 默认不渲染 raw HTML），不执行脚本

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src-tauri/src/commands/fs/mod.rs`（新） | 通用读命令 | `read_file` / `read_file_inner`；`FileSlice` / `FileStat` DTO；字节窗口 + 字符边界对齐 + binary 探测 + 上限夹取；零领域规则（不认识 change / explore / 架构模型） |
| `src-tauri/src/commands/mod.rs` | 命令登记 | `all_commands!` 增 `commands::fs::read_file`；删 `archi_read_doc` / `read_explore` / `read_exec_output` 三行 |
| `packages/desktop/src/components/file/file-viewer.tsx` | 文档预览消费方 | `archiReadDoc` → `readFile`（相对路径按 root 拼接）；binary / 缺失 / 失败三态呈现 |
| `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | explore 预览取数 | `explore_doc_path`（按臂单次解析、读取与 watch 共用）+ `read_file`；`ExploreDoc` 转本地类型 |
| `packages/desktop/src/views/architecture/components/element-drawer.tsx` | 架构文档条目 | `FileViewer` 承载 ADR 正文读取（经 `read_file`） |
