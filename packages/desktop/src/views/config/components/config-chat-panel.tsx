import { AgentInput, ConfigApplyScope, SessionPanel } from '@/components/agent';
import type { AgentPermissionMode } from '@/types/dto';

import { useConfigChat } from '../hooks/use-config-chat';

export interface ConfigChatPanelProps {
  /** workspace root（清单与发起的 store 寻址半边；null = 零 invoke） */
  root: string | null;
  /** 写回成功后的页面刷新钩子（透传给提案卡） */
  onApplied: () => void;
}

/** composer 插槽（同文件私有）：运行中禁发 + 理由文案由 hook 单源给出 */
function ConfigComposer({
  running,
  disabled,
  disabledReason,
  onSend,
  onStop,
}: {
  running: boolean;
  disabled: boolean;
  disabledReason: string | null;
  onSend: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  onStop: () => void;
}): React.JSX.Element {
  return (
    <AgentInput
      idPrefix="config-chat"
      placeholder="问这个 workspace 的配置，让它检测项目 / 框架并给出候选…"
      running={running}
      disabled={disabled}
      disabledReason={disabledReason ?? undefined}
      onSend={onSend}
      onStop={onStop}
    />
  );
}

/**
 * 配置页常驻 Agent 面板：状态面全在 `useConfigChat`，骨架复用共享 `SessionPanel`
 * （第三处消费者），写回动作面经 `ConfigApplyScope` 交给提案卡（`root` 缺席即不
 * 提供 provider——卡片呈只读态）。
 */
export function ConfigChatPanel(props: ConfigChatPanelProps): React.JSX.Element {
  const { root, onApplied } = props;
  const state = useConfigChat({ root });
  const panel = (
    <SessionPanel
      state={state}
      title="配置 Agent"
      testIdPrefix="config-chat"
      composer={({ running }) => (
        <ConfigComposer
          running={running}
          disabled={!state.canSend}
          disabledReason={state.disabledReason}
          onSend={state.send}
          onStop={state.stop}
        />
      )}
    />
  );
  if (root === null) return panel;
  return <ConfigApplyScope value={{ root, onApplied }}>{panel}</ConfigApplyScope>;
}
