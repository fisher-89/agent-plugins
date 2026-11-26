# Desktop.Components.Agent.AgentInput

## 职能

会话输入框与提交交互：prompt 文本域 + permission-mode 档位 + 发送 / 停止两个动作位，另承载
两种「发不出去」的可见态（整面板禁用理由、在途排队中）。

**边界**：只上抛（`onSend` / `onStop`），零会话取数、零 IPC、零流订阅——链参数、stance 组装
与是否真的能发由上游判。档位态本地持有，不回传也不落库。

## 不变量

- **发送即清空**：点击发送与快捷键发送走同一条 `send(text)`，提交后本地清空 prompt；`text`
  由调用方显式给出，MUST NOT 由 `send` 内部回读 state。
- **快捷键取原生值**：`Ctrl+Enter` 发送读 `event.currentTarget.value`——受控组件的 `value` 是
  上一次渲染的快照，同一事件回调内 `setPrompt` 尚未生效，读它只会拿到旧值。
- **`disabled` 与 `pending` 语义正交**：`disabled` 是业务判据（归档只读 / 流程运行中），
  textarea 与发送同禁并呈 `disabledReason`；`pending` 是在途提交态，只禁发并呈「排队中…」
  ——两者都不影响停止入口。
- **`running` 只禁发、只呈停止**：运行中发送禁用、停止入口出现；发送可用性合成三轴
  `!disabled && !pending && prompt非空`。
- **`hidePermissionMode` 缺省 `true`**：工具行不渲档位，但 `onSend` 的 `permissionMode`
  参数位保留——下游 MUST NOT 因隐藏而消费它。
- **`idPrefix` 是 id / `data-testid` 的单一前缀**：`<prefix>-prompt` / `-send` / `-stop` /
  `-permission-mode` / `-composer` 全部由它派生，MUST NOT 在组件内另写第二种命名。
- **档位值域封闭**：`bypassPermissions` / `acceptEdits` / `default` 三值（缺省
  `bypassPermissions`），下拉只接受清单内的值，MUST NOT 透传自由文本。

## 契约面留白

- **组件不进契约**：形态是 `(props: <具名 props>) => React.JSX.Element`——入参具名类型、返回位
  是 JSX 元素。契约侧 `$ref` 归一为 `?<名>`、代码侧是具名类型，两侧恒不等；同类实测差异见
  `Desktop.Components.Agent.AgentError` 的留白段。
- **入参形状不进契约**：三个字段位引用外部类型——权限档位词表（`types/dto`）与两个内联函数形状
  （发送动作的对象参）。契约词汇没有跨模块具名类型位，把外部类型在本元素重抄一份即造第二个真相
  源（`types/dto` 改名后契约静默失真），故不生成本元素的 `$defs`。

## 类型

本元素自有 `$defs` 为空（入参形状全在留白里，见上）：契约侧不新建类型。
