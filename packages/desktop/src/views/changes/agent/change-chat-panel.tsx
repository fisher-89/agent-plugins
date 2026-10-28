import { SessionPanel } from '@/components/agent';

import { ChangeChatComposer } from './change-chat-composer';
import { useChangeChat } from './hooks/use-change-chat';

export interface ChangeChatPanelProps {
  /** workspace root（清单与发起的 store 寻址半边） */
  root: string | null;
  /** change id（会话归属 `source_ref`；面板态身份源） */
  changeId: string | null;
  /** 执行根来源（`detail.worktree`；null = legacy 记录回落 root） */
  cwd: string | null;
  /** 归档 change：只读（全禁发、历史仍可浏览） */
  readOnly: boolean;
  /** 在飞 change flow run：禁发判据 */
  runActive: boolean;
}

/**
 * change 详情左栏通用 Agent 面板（骨架消费者）：判据矩阵与状态行文案由
 * `useChangeChat` 单源供给，composer 经插槽注入（`running` 取插槽动作面，
 * `disabled` / `disabledReason` 取面板判据）。
 */
export function ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element {
  const { root, changeId, cwd, readOnly, runActive } = props;
  const state = useChangeChat({
    root,
    changeId,
    worktree: cwd,
    runActive,
    archived: readOnly,
  });
  return (
    <SessionPanel
      state={state}
      title="通用 Agent"
      testIdPrefix="change-chat"
      composer={({ running }) => (
        <ChangeChatComposer
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
