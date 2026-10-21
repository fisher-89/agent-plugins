/**
 * change 面板 composer：`AgentInput` 薄适配——渲染、档位逻辑、发送 / 停止收口在
 * 组件族（`components/agent`），本文件只保留面板文案（label / placeholder）与
 * `change-chat-*` testid 前缀（`change-chat-prompt` / `change-chat-send` /
 * `change-chat-stop` / `change-chat-permission-mode` / `change-chat-composer`）。
 *
 * `disabled` / `disabledReason` 由面板侧的判据单源（`useChangeChat`）经插槽
 * 外的 props 直接传入——`AgentSessionActions` 契约零变化；`running` 语义（禁发 +
 * 停止入口）由组件族保持。
 */
import { AgentInput } from '@/components/agent';
import type { AgentPermissionMode } from '@/types/dto';

export interface ChangeChatComposerProps {
  /** 运行中：禁发 + 呈停止入口（既有语义） */
  running: boolean;
  /** 整面板禁用（归档只读 / 流程运行中）：输入框与发送同时禁用 */
  disabled: boolean;
  /** 禁用理由文案（`disabled` 时呈于工具行上方） */
  disabledReason: string | null;
  onSend: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  onStop: () => void;
}

export function ChangeChatComposer({
  running,
  disabled,
  disabledReason,
  onSend,
  onStop,
}: ChangeChatComposerProps): React.JSX.Element {
  return (
    <AgentInput
      idPrefix="change-chat"
      label="通用 Agent"
      placeholder="就此 change 提问、让它读代码或起草改动…"
      running={running}
      disabled={disabled}
      disabledReason={disabledReason ?? undefined}
      onSend={onSend}
      onStop={onStop}
    />
  );
}
