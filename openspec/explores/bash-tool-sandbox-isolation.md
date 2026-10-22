# Bash 工具沙箱隔离：可行性复核与代码落点

workflow_type: requirement

讨论稿的方向（Windows 降 IL + macOS sandbox-exec）在形状上是对的，但**关键那一条 Windows 机制经本机实测是空转的**，且两个平台的真实卡点不在"怎么降权"，而在"降权后工具链还能不能跑"。以下按 实测 → 原理 → 落点 → 分支 组织。

## 一、三个实测结论（会改写方案）

| 讨论稿断言 | 本机实测（Windows 11，普通非提权账户 `wps`） | 影响 |
|---|---|---|
| `runas /trustlevel:0x20000` = Low Integrity | 子进程 **仍是 Medium**（`S-1-16-8192`）；组清单 / SID 与父进程逐行一致；仅特权被裁到只剩 `SeChangeNotifyPrivilege` | Windows 臂**完全没降完整性级别**，写保护为零 |
| 降权后 stdout "依然可以被完美捕获" | `runas` 连自己的 usage 都不进重定向句柄（重定向文件 0 字节）；子进程输出只能靠子进程自己写文件 | 与 bash 工具的"双管道实时排水"根本冲突 |
| `icacls <workspace> /setintegritylevel Low` 即锁死读写 | Low 标签施加成功（无需提权）；但 Medium 进程**照旧能写** Low 目录 | 这一半是对的（write-down 允许），但反方向才是被拦的那半 |

第三张表要单独看：这不是"配置少了一步"，是**方向搞反了**。

复现方式（本次探索的探针，已清理）：

```bash
# 1) runas 到底给了什么令牌：子进程自己写文件回报
runas /trustlevel:0x20000 probe.bat      # bat 内跑 whoami /groups /fo csv + whoami /priv
#   → "Mandatory Label\Medium Mandatory Level","Label","S-1-16-8192"
#   → 特权只剩 SeChangeNotifyPrivilege

# 2) stdout 透传
cmd /c "runas /showtrustlevels > out.txt" # out.txt 存在但 0 字节

# 3) 标签方向
icacls <dir> /setintegritylevel (OI)(CI)Low   # Successfully processed
echo x > <dir>/a.txt                           # 成功（Medium 写 Low = 允许）
icacls <dir> /setintegritylevel (OI)(CI)High   # 失败：需 SeRelabelPrivilege
```

顺带一个方法论缺口：`High` 标签施加被拒（需 `SeRelabelPrivilege`），所以"Low 进程写 Medium 对象被拒"这半边**没法用 Medium 自查**——要验真必须手上有一个真 Low 令牌。方案落地前这是必过的一关。

## 二、完整性级别拦的是"写"，不是"读"

```
                  读                     写
Low 进程 ──→ Medium 对象   ✓ 允许        ✗ 拒绝（NO_WRITE_UP）
Medium 进程 ─→ Low 对象    ✓ 允许        ✓ 允许（write-down）
```

Windows 内核**只实施 NO_WRITE_UP**；`NO_READ_UP` 在文件面不实施（SACL 里写了也不生效）。所以：

- `%USERPROFILE%\.ssh\id_rsa`、`AppData\Local\...\Cookies`、`.aws\credentials`、各种 `.env`——**Low IL 全都能读**。
- 讨论稿明确"不需要限制联网"。
- 读敏感文件 + 能联网 = prompt injection 外泄链**完整保留**。

也就是说：用户自述的痛点"防止 Agent **读写**系统敏感文件"，Low IL 只兑现了半个"写"，恰好漏掉了风险更高的"读"。真要拦读，Windows 原生只有 **AppContainer**（或给一个专用 SID 做 ACL deny）这一类机制。

## 三、现在的 bash 工具长什么样（落点）

```
model tool_call(bash)
  └─ sdk/loop.rs: 成员校验 → policy::allows → sandbox_rewrite → tools::execute
       └─ sdk/bash.rs::execute
            ├─ exec::resolve_shell()            // git-bash / cmd /C / sh -c 探测
            ├─ exec::SystemCommand::script_with().into_tokio()
            └─ .current_dir(root).kill_on_drop(true).spawn()
                 └─ tokio::process::Command     // ← 只暴露 creation_flags
```

- `crates/infra/exec` 是**全仓唯一 spawn 配置点**（`SystemCommand` + `run_shell`），也是当前唯一的 `CREATE_NO_WINDOW` 施加点——沙箱要落地，天然属于这里。
- 但 `tokio::process::Command` / `std::process::Command` **无法指定访问令牌**（`creation_flags` 只能加 flags）。想真降权只有两条路：进程外蹦床，或在 exec 内自建 Win32 spawn。这决定了工作量的量级（见第五节）。
- 同域还有一批 spawn 不由 bash 工具走：`checks` 域的 static_check / testexec / mutation 直接 `run_shell(cargo|vitest|stryker|cargo-mutants)`。它们是"固定命令"，风险低于模型自由拼命令，但**如果统一给 exec 挂钩子，它们会被一起沙箱化**，而它们恰恰最依赖用户级缓存。

现状的知情边界在 `sdk/policy.rs` 模块文档与 spec 里都写明过：

- `openspec/specs/desktop-agent-execution/spec.md`「SDK 引擎 bash 工具」→「知情边界：授权/沙箱/安全机制不进本期」
- 同文件「SDK 租户 MVP 边界与已知限制」#4「bash 无沙箱」
- 「静态权限与路径沙箱」：明写"命令级白/黑名单 MUST NOT 作为安全手段（假安全），真沙箱（job object / 容器）与交互审批留后续 change"

本次就是那条"留后续 change"的兑现。附带校正一处 spec 表述：**job object 不解决文件隔离**，它解决的是进程树/资源限额（正好对应 MVP 限制 #1 的孤儿进程）。文件隔离与生命周期治理是两件事，别混在一条 change 里。

## 四、Windows 臂的三处硬伤（换真 Low 令牌也躲不掉）

### 1. 输出透传

`runas` 是控制台程序 + `CreateProcessWithLogonW/SAFER`，stdout 不可靠透传（实测 0 字节）。真 Low 令牌走 `CreateProcessAsUser` 时，**我们自己**创建管道就行——所以这条只在"用 runas 抄近路"时是死路，不是机制本身的问题。

### 2. 工具链缓存与临时目录（最大的现实卡点）

Low 进程写不了任何 Medium 对象，而工具链全写在这些地方：

| 工具 | 必写路径 | Low 下的结局 |
|---|---|---|
| cargo / rustc | `~/.cargo/registry`、`~/.rustup`、`target/` | 下载解包失败 |
| npm / node | `~/.npm`、`%LOCALAPPDATA%\npm-cache` | install 失败 |
| pip / python | `%LOCALAPPDATA%\pip\Cache`、`__pycache__` | 缓存写失败 |
| 几乎所有 | `%TEMP%`（`AppData\Local\Temp`） | 大量工具在此 panic |

三条出路，各有代价：

- (a) 重定向进 workspace：`TMP/TEMP/CARGO_HOME/npm_config_cache` 全指到 `<root>/.tmp` 一类位置——仓库被污染、每个 repo 一份独立缓存（首次 cargo build 全量重来）、还要管清理。
- (b) 把用户缓存目录也递归打 Low（`icacls /T`）——让 agent 能写 `~/.cargo/registry` 的源码包，**供应链面直接打开**；且污染用户级目录。
- (c) 只读挂载式缓和：让沙箱读缓存、写缓存指到 workspace 内副本——需要每个工具各自支持（`CARGO_HOME` 会把 registry 一起换掉，只能整份复制）。

### 3. workspace 打 Low 标签是持久副作用

IL 标签存在对象 SACL 里，随 `(OI)(CI)` 继承整棵树。跑完不复位，用户的 repo 长期处于 Low 标签；`git`、编辑器、其它 Medium 工具照旧能写（write-down），所以"不会坏"，但语义脏且难解释。用完要 `/T` 递归复位——又是个失败即残留的写副作用（本仓对这类"先改外部状态"的操作一贯谨慎）。

## 五、三条实现路线（真正的工作量分野）

```
路线 A：蹦床 sidecar                      路线 B：exec 内自建 spawn
─────────────                            ─────────────
app ──Stdio(pipe)──▶ launcher.exe        exec ──CreatePipe──▶ CreateProcessAsUser(token)
                        │                                        └─ 包成 Stdio::from(handle)
                        └─CreateProcessAsUser(Low token)──▶ bash -c cmd
                                                                 
unsafe 关在一个小 crate，stdio 天然透传      exec 里 ~300-400 行 unsafe；
退出码代理，taskkill /T 仍收得到树          与 run_shell 的取消轮询/双管道排水全部重接
（Medium 杀 Low = write-down，允许）        两条 spawn 路径语义对齐成本高
```

**路线 C：AppContainer**（唯一能拦读的原生机制）

```
icacls <root> /grant "*S-1-15-2-<appcontainer-sid>":(OI)(CI)M
子进程默认对用户目录零访问 → 读也被封（成就了"读写敏感文件"这条原始诉求）
代价：capability（internetClient）、node/cargo 在 AppContainer 下的实测面、
      每仓 ACL 授予与撤销、以及失败时的诊断噪音
```

能力矩阵（凭实测定性，非宣言）：

| 机制 | 拦读 | 拦写 | 保 stdio | 保工具链 | 工作量 |
|---|---|---|---|---|---|
| 现状（Medium） | ✗ | ✗ | ✓ | ✓ | 0 |
| `runas /trustlevel` | ✗ | ✗ | ✗ | ✓ | 极小（但无效） |
| 真 Low IL 令牌 | ✗ | 部分 | 需自建 | ✗（缓存） | 中～大 |
| AppContainer | ✓ | ✓ | 需自建 | ?（待实测） | 大 |
| job object | ✗ | ✗ | ✓ | ✓ | 小（只治生命周期） |
| macOS sandbox-exec | ✗ | ✓（除白名单） | ✓ | 需白名单 | 极小 |

## 六、macOS 臂：形状对，四处注意

- `sandbox-exec -p '<profile>' bash -c ...` 保留 stdio，确实是几行的事；SBPL 语义"后匹配胜"，`(allow default)` → `(deny file-write*)` → `(allow file-write* (subpath ...))` 的顺序正确。
- sandbox **单调收窄**：子进程自己 exec `sandbox-exec -p '(allow default)'` 也拿不回权限，逃逸不成立。
- deprecated 但仍在（Chrome 旧沙箱同源），Apple 不保证未来版本。
- 同样的缓存问题（`~/.npm`、`~/.cargo`、`~/Library/Caches/pip`、`$TMPDIR`），而且还要额外放行 `/dev/null`——profile 里漏了它，个别工具会以很难归因的方式失败。
- AV 侧的假设要单独校验：macOS 是 XProtect/YARA，「用 sandbox-exec 就不报」没有依据——`sandbox-exec` 恰恰是恶意软件常用技俩之一。

## 七、杀毒议题：机制和症状对得上吗？

需要先对齐因果，否则容易做出"加了沙箱但误报照旧"的工程：

1. Defender 的 on-access 扫描**不因目标是 Low 标签而跳过**；ASR / 行为监控看的是行为序列与命令行特征，不是令牌完整性级别。
2. 降 IL 不会让"agent 刚写出来的脚本"变得不像脚本；`powershell -enc`、`curl | sh` 这类字符面特征跟令牌无关。
3. 真正降误报的杠杆通常在别处：代码签名（本仓 `RELEASE.md` 只签 updater 工件，**没有代码签名证书/公证**的记录）+ 微软提交白名单 + 不现场生成可执行脚本。
4. 因此建议先取证据再定方案：被隔离的**具体文件路径**、Defender 的检测名（`Trojan:Script/...` vs `Behavior:...`）与事件 ID。这三样决定的是完全不同的修法。

## 八、开放问题

1. **目标是"拦写"还是"拦读+拦写"？** 这一条直接决定 Low IL 还是 AppContainer。按用户原话（"防止读写敏感文件"）应该是后者，而讨论稿的方案只覆盖前者。
2. bash 之外的 spawn 要不要一并进沙箱？`checks` 域的 cargo/vitest/stryker 一进沙箱就要面对缓存重定向问题。
3. 沙箱是**恒开**还是**配置位**？现在 `permission_mode` 在 `walker.rs` 与 store 缺省处硬编码 `BypassPermissions`，需要一个权威配置点（`workspace config` 已有 `write_protection` 先例可循）。
4. **失败面怎么回灌模型**：内核拒绝访问时，现在只有"命令退出码 + stderr 噪音"或工具链 panic 两条路，模型很难自愈。要不要一个专门的 `sandbox_denied` 风格留痕（`SystemNotice` 词典是开放的）。
5. 兼容基线：Windows 家庭版无 Hyper-V 时 AppContainer 仍可用；是否需要 WSL2 备选路径（重量级，但与"免 Docker"目标不冲突）。

## 九、下一步分支（挑一条继续挖）

- **A. 先做证据链**：拿到 Defender 的实际检测名与被隔离文件，判定"沙箱"是否真是对症药。
- **B. macOS 先落地 + Windows 只做能力探测**：探测"这台机器上能否真拿到 Low 令牌 / AppContainer"，拿不到就不发车（避免上一版那种"看起来降权了其实没有"）。
- **C. 蹦床 sidecar（路线 A）**：最小可用的真 Low IL，unsafe 隔离在独立小 crate，stdio 天然透传。
- **D. AppContainer（路线 C）**：直接对齐"读写都拦"的原始诉求，先做一次 node/cargo 在 AppContainer 下的实跑验证再定。
- **E. 只做 job object**：把 MVP 限制 #1 的孤儿进程还掉，安全面明确留痕不碰——最小风险、不解决安全诉求。

## 十、spec 影响面（供转 change 时参考）

| 落点 | 改动性质 |
|---|---|
| `desktop-agent-execution`「SDK 引擎 bash 工具」知情边界段 | 从"不进本期"改为"已偿还"+新边界 |
| `desktop-agent-execution`「SDK 租户 MVP 边界与已知限制」#4 | 同上（边界条目改写，不删除留痕纪律） |
| `desktop-agent-execution`「静态权限与路径沙箱」 | "job object = 真沙箱"的表述需校正为生命周期治理 |
| `sdk/policy.rs` 模块文档 | "bash 无沙箱知情边界"段落重写 |
| `crates/infra/exec` | 新增沙箱能力（新增 `windows-sys` 依赖或 sidecar 装配）——注意"唯一 spawn 配置点"的契约要写明沙箱臂 |
| 新 ability 建议 | `desktop-bash-sandbox`（或并入 `desktop-exec-runtime`），承载能力矩阵与降级语义 |
