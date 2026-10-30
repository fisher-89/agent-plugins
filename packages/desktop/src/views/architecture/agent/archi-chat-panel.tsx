import { SessionPanel } from '@/components/agent';

import { ArchiChatComposer } from './archi-chat-composer';
import { useArchiChat } from './hooks/use-archi-chat';

export interface ArchiChatPanelProps {
  /** workspace root（清单与发起的 store 寻址半边；null = 零 invoke） */
  root: string | null;
}

export function ArchiChatPanel(props: ArchiChatPanelProps): React.JSX.Element {
  const { root } = props;
  const state = useArchiChat({ root });
  return (
    <SessionPanel
      state={state}
      title="架构 Agent"
      testIdPrefix="archi-chat"
      composer={({ running }) => (
        <ArchiChatComposer
          running={running}
          disabled={!state.canSend}
          disabledReason={state.disabledReason}
          onSend={state.send}
          onStop={state.stop}
        />
      )}
    />
  );
}
