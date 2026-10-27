/**
 * 架构页左栏通用 Agent 面板（`desktop-archi-chat`）：归属参数（`source = "archi"` /
 * `sourceRef = null` / `cwd = null` → 后端回落主仓根）与判据全在 `useArchiChat`；
 * 渲染体是共享骨架 `SessionPanel`（`testIdPrefix = "archi-chat"`），本文件只剩
 * 归属装配与 composer 插槽。
 */
import { SessionPanel } from '@/components/agent';

import { ArchiChatComposer } from './archi-chat-composer';
import { useArchiChat } from './hooks/use-archi-chat';

export interface ArchiChatPanelProps {
  /** workspace root（清单与发起的 store 寻址半边；null = 零 invoke） */
  root: string | null;
  /** 左栏是否处于折叠窄条形态（真相取页面开关 state，非分栏像素派生） */
  collapsed: boolean;
  /** 折叠 / 展开切换（页面头开关与窄条展开入口共用同一入口） */
  onToggleCollapsed: () => void;
}

export function ArchiChatPanel(props: ArchiChatPanelProps): React.JSX.Element {
  const { root, collapsed, onToggleCollapsed } = props;
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
      collapsed={collapsed}
      onToggleCollapsed={onToggleCollapsed}
    />
  );
}
