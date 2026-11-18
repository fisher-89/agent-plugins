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
      placeholder="就此 change 提问、让它读代码或起草改动…"
      running={running}
      disabled={disabled}
      disabledReason={disabledReason ?? undefined}
      onSend={onSend}
      onStop={onStop}
    />
  );
}
