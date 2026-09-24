# desktop-file-watch Specification

## Purpose

定义通用「watch 文件变化」推送通道契约：notify 单文件订阅、失效信号语义（通知不含内容）、壳层 Channel 桥接与生命周期边界。该通道是与执行流通道（`agent_start` Channel）并列的第二个被认可的推送语义，是「显式取数、无 watch、无轮询」既有取数纪律的唯一显式例外。

## Requirements

### Requirement: 单文件 watch 订阅通道

desktop SHALL 新增通用「watch 文件变化」推送通道：订阅目标是**单个文件**（当前消费者：explore 详情页的当前 `explore.md`）。实现 SHALL 分层落位——notify 单文件订阅与信号流归 `crates/infra/watch/` 新 crate（零 Tauri，产出「文件被修改」信号流）；Tauri Channel 包装 SHALL 留在壳层命令（core 禁 Tauri 纪律不变）。命令面 SHALL 为 `watch_subscribe` / `watch_unsubscribe` 薄命令（新 `commands/watch/` 轨道）：subscribe 建立订阅并绑定 Channel，unsubscribe 解除订阅并停止信号流；对同一文件的重复订阅 SHALL 幂等（不产生重复信号）。订阅目标可能尚未存在或已被删除：subscribe SHALL NOT 因目标缺失报错（信号流就绪，目标（重）出现后事件生效）。

#### Scenario: 修改信号到达

- **WHEN** 订阅 `openspec/explores/foo.md` 后该文件被外部进程修改保存
- **THEN** 前端经 Channel 收到该文件的修改信号（编辑器 rename-replace 原子保存形态下不丢事件）

#### Scenario: 目标缺失不报错

- **WHEN** 对尚未落盘的 `explore.md` 发起 `watch_subscribe`
- **THEN** 订阅成功无错误；文件随后被创建时修改信号正常到达

### Requirement: 失效信号语义（非数据通道）

watch 通知 SHALL 只携带「目标文件被修改」信号与订阅标识，MUST NOT 携带任何文件内容——通道是**失效信号通道而非数据通道**。数据面 SHALL 维持既有取数纪律：前端收到信号后防抖，再经既有取数命令（如 `read_explore`）显式拉取刷新；后端 SHALL 只透传修改事件、MUST NOT 下沉去抖逻辑（防抖窗口由前端持有）。MUST NOT 将该通道扩展为内容推送、目录树事件广播或任意轮询替代品。

#### Scenario: 通知无内容且触发显式拉取

- **WHEN** 一次修改信号到达前端
- **THEN** 通知体不含文件内容字节；前端防抖后恰发起一次 `read_explore` 刷新（防抖窗口内多次信号合并为一次拉取）

### Requirement: 生命周期与平台边界

订阅生命周期 SHALL 即消费页面生命周期：详情页卸载（含 workspace 切换 replace 回清单）即 `watch_unsubscribe`，退订后 MUST NOT 再有信号到达前端。平台边界 SHALL 显式留痕：本应用目标平台为 Windows（notify 底层经父目录实现，单文件 watch 在编辑器 rename-replace 原子保存下不丢事件）；类 Linux/inotify 平台单文件 watch 存在 rename 失效陷阱，跨平台支持时再议（本变更 MUST NOT 为此预建抽象）。

#### Scenario: 卸载退订

- **WHEN** 用户从 explore 详情页切换 workspace（replace 回 `/explores`）或关闭页面
- **THEN** `watch_unsubscribe` 被调用；此后修改该文件前端不再收到信号

#### Scenario: 平台边界留痕可考

- **WHEN** 查阅 watch 实现的代码注释或 crate 文档
- **THEN** Windows 目标平台语义与 Linux inotify 陷阱均有显式说明，无跨平台兼容层代码

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/watch/`（新 crate，零 Tauri） | notify 单文件订阅 | 订阅/退订；「文件被修改」信号流（std mpsc）；目标缺失不报错；零内容、零去抖 |
| `src-tauri/src/commands/watch/`（新轨道） | Channel 桥接薄命令 | `watch_subscribe`（绑定 Tauri Channel）/ `watch_unsubscribe`；同文件重复订阅幂等；`Result<T, String>` 错误模板 |
| `packages/desktop/src-tauri/Cargo.toml` | workspace 注册 | `crates/infra/watch` 进 members；notify 依赖收敛于该 crate |
| 前端 watch hook（`views/explores/hooks/`） | 消费侧 | 防抖窗口持有方；信号 → 显式 `read_explore` 拉取；页面卸载退订 |
