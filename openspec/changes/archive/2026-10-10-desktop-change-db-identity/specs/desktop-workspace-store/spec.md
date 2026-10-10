# desktop-workspace-store Specification (Delta)

## ADDED Requirements

### Requirement: workspace 库格式版本与旧库作废重建

workspace 库 SHALL 承载 store 格式版本（change 身份锚形态版本——本变更随 name 主键 → id 主键 + name 属性换锚 bump）。库打开 SHALL 探测该版本：**版本缺失（旧形态库——含 name 主键 `ChangeRecord` 的存量库）或低于当前版本 → 旧库整体作废并重建全新空库**；版本一致 → 照常打开。作废 SHALL 丢弃旧库全部数据（存量 change / 相位 / run / 会话转录 / explore 记录的损失为既定接受面——用户拍板），MUST NOT 读取、解码、搬移、迁移或以任何方式消费旧形态行，MUST NOT 引入 name → id 迁移链或迁移层。作废与探测的实现形态（库级版本标记 + 文件删除重建 vs 旧文件换名留档后新建）由 design 定稿；无论形态，作废后读面 MUST 为**全新空库**（旧数据零可达），探测与作废 MUST 幂等（同库重复打开不作废已就绪的新库）；打开路径 MUST NOT 因旧形态库报错。全局库（user 维度：`WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord`）SHALL NOT 受本作废影响——workspace 注册表保留，壳态照常从注册表恢复；`remove_workspace` 不留库文件与重新添加恢复历史的既有语义在新格式下照旧。

#### Scenario: 旧形态库打开即作废重建

- **WHEN** 打开含 name 主键形态 `ChangeRecord` 行的存量 workspace 库（旧格式 / 版本缺失）
- **THEN** 打开成功且不报错：旧数据零可达（新库为空，change / run / 会话转录零残留）、零迁移代码路径参与；store 源码无 name → id 迁移 / 解码链

#### Scenario: 新库版本标记幂等

- **WHEN** 新建 workspace 库并写入数据后重复打开同一文件
- **THEN** 版本标记就位，第二次打开非作废路径（数据原样可读），作废探测幂等

#### Scenario: 全局库不受波及

- **WHEN** 任一 workspace 库经作废重建
- **THEN** 全局库 workspace 注册记录原样可读，壳态恢复当前根照常；工作区注册（add / remove）语义零变化

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/store.rs`（open 路径） | 格式版本探测与作废重建 | 版本缺失 / 低于当前 → 旧库整体作废并重建空库（实现形态 design 定稿）；幂等、零报错、零迁移层；`open_with` 的建 / 开分支语义与 `for_root` 缓存复用不变 |
| `crates/infra/store/src/model.rs` + `lib.rs`（版本载体） | 格式版本标记 | store 格式版本常量与载体（库级标记记录形态 design 定稿）单点定义；与 native_model 模型版本段换锚（表名含版本与主键段，旧表对新读面不可见）两层衔接 |
| 全局库路径（不改） | 零触点 | 全局库打开 / 模型组 / workspace 注册命令零改动 |
