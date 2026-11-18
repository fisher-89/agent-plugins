import { AgentInput } from '@/components/agent';
import type { AgentPermissionMode } from '@/types/dto';

export interface ArchiChatComposerProps {
  /** 运行中：禁发（本面板唯一判据，与 `disabled` 同源） */
  running: boolean;
  /** 整面板禁用（运行中）：输入框与发送同时禁用 */
  disabled: boolean;
  /** 禁用理由文案（`disabled` 时呈于工具行上方） */
  disabledReason: string | null;
  onSend: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  onStop: () => void;
}

export function ArchiChatComposer({
  running,
  disabled,
  disabledReason,
  onSend,
  onStop,
}: ArchiChatComposerProps): React.JSX.Element {
  return (
    <AgentInput
      idPrefix="archi-chat"
      placeholder="问这个 workspace 的架构、让它补模型或修文档…"
      running={running}
      disabled={disabled}
      disabledReason={disabledReason ?? undefined}
      onSend={onSend}
      onStop={onStop}
    />
  );
}
