# desktop-app-shell Specification (Delta)

## ADDED Requirements

### Requirement: 外链打开插件与 capabilities 最小权限面

壳层 SHALL 提供**受控的外链打开能力**（供 agent 搜索来源与引用列表点击使用），并把权限面收窄到最小：

- **插件注册**：`tauri-plugin-opener` SHALL 加入 workspace 依赖（`packages/desktop/src-tauri/Cargo.toml` 版本 `2`）与 `src-tauri` 根依赖，并 SHALL 在 app builder 链注册（`.plugin(tauri_plugin_opener::init())`，与既有 `dialog` 插件同位——位置与形态沿 `tauri-plugin-dialog` 先例，MUST NOT 自造第二种注册形态）。
- **权限面**：`capabilities/default.json` SHALL 增 opener 的**打开 URL** 权限项，且 scope MUST 只含 `http://*` / `https://*`；MUST NOT 引入任何连带放开 `mailto:` / `tel:` / `file:` 的粗粒度项（如仅提供「默认 URL 集」形态的权限项则 MUST NOT 采用，回落显式 `allow-open-url` + scope 形态）。`capabilities/default.json` 的 `description` SHALL 同步更新（不再声称「无其他权限」）。
- **调用点收敛**：前端外链打开 SHALL 只有一个调用点（`packages/desktop/src/components/agent/agent-blocks.tsx` 的 `ExternalLink`，见 desktop-agent-chat-infra「web 搜索活动与引用来源呈现」）；MUST NOT 在其他视图层散落 opener 调用或自行拼 shell 命令打开浏览器。
- **前端依赖**：`@tauri-apps/plugin-opener` SHALL 加入 `packages/desktop/package.json` 依赖（版本沿 `@tauri-apps/plugin-dialog` 的 `^2` 系形态）。
- **失败面**：打开失败（scope 拒绝 / 系统无默认浏览器）SHALL 不抛出到会话渲染层（不打断呈现、不炸消息面板），MUST NOT 静默改为其他打开通道（如 shell 命令）。

#### Scenario: 插件注册与 dialog 同位

- **WHEN** 审查 `packages/desktop/src-tauri/src/lib.rs` 的 builder 链与 `Cargo.toml` 依赖
- **THEN** `tauri_plugin_opener::init()` 与 `tauri_plugin_dialog` 在同一 builder 链注册，workspace 与 crate 两侧依赖均在场

#### Scenario: 权限只放 http(s)

- **WHEN** 逐项审阅 `capabilities/default.json` 的 `permissions`
- **THEN** opener 相关项只授予 http / https 目标的打开能力，无 `mailto:` / `tel:` / `file:` 或「默认 URL 集」类宽权限项；`description` 与权限实况一致

#### Scenario: 唯一调用点

- **WHEN** 检索 `@tauri-apps/plugin-opener` 的 import 来源与打开 URL 的调用
- **THEN** 仅出现在 `ExternalLink` 一处；agent 会话外的视图层零调用

#### Scenario: 打开失败不炸呈现

- **WHEN** 目标 url 因 scope 拒绝或系统无默认浏览器而打开失败
- **THEN** 消息面板照常渲染（搜索卡与引用列表在场），失败不升级为会话错误态、不改用其他打开通道

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/Cargo.toml`（workspace 与 crate 依赖） | 外链插件依赖 | workspace `tauri-plugin-opener = "2"`；`src-tauri` 根依赖引用；与 `tauri-plugin-dialog` 同形态 |
| `packages/desktop/src-tauri/src/lib.rs` | app builder 链 | `.plugin(tauri_plugin_opener::init())` 与 dialog 同位；无第二种注册形态 |
| `packages/desktop/src-tauri/capabilities/default.json` | 最小权限面 | 增 opener 打开 URL 权限项，scope 仅 `http://*` / `https://*`；`description` 同步；无宽权限项 |
| `packages/desktop/package.json` | 前端依赖 | 增 `@tauri-apps/plugin-opener`（`^2` 系）；版本号随用户可见功能变更上抬 |
| `packages/desktop/src/components/agent/agent-blocks.tsx` | 唯一外链调用点 | `ExternalLink` 经 opener 打开 http(s) 目标；非 http(s) 不渲染为链接；失败不上抛 |
