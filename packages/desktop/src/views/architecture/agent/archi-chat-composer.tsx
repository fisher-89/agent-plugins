/**
 * 架构页面板 composer：`AgentInput` 薄适配——渲染、档位逻辑（缺省
 * `bypassPermissions`）、发送 / 停止收口在组件族（`components/agent`），本文件只
 * 保留面板文案（label / placeholder）与 `archi-chat-*` testid 前缀。
 *
 * `disabled` / `disabledReason` 由面板侧的判据单源（`useArchiChat`）经插槽外的
 * props 直接传入——`AgentSessionActions` 契约零变化。
 */
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
      label="架构 Agent"
      placeholder="问这个 workspace 的架构、让它补模型或修文档…"
      running={running}
      disabled={disabled}
      disabledReason={disabledReason ?? undefined}
      onSend={onSend}
      onStop={onStop}
    />
  );
}
