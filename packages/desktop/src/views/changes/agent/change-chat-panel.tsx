/**
 * change 详情左栏通用 Agent 面板：状态 / 清单 / 判据全在 `useChangeChat`（面板态
 * 挂 change id，切换即复位）；渲染体是共享骨架 `SessionPanel`（`testIdPrefix =
 * "change-chat"`）——头区动作 / 状态行 / 历史清单 / 折叠窄条由骨架按前缀生成，
 * `change-chat-*` 查询挂钩与文案逐字不变；本文件只剩归属参数与 composer 插槽。
 */
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
  /** 左栏是否处于折叠窄条形态（真相取分栏 `onResize`，视图持有） */
  collapsed: boolean;
  /** 折叠 / 展开切换（沿分栏 `panelRef` 命令面） */
  onToggleCollapsed: () => void;
}

/**
 * change 详情左栏通用 Agent 面板（骨架消费者）：判据矩阵与状态行文案由
 * `useChangeChat` 单源供给，composer 经插槽注入（`running` 取插槽动作面，
 * `disabled` / `disabledReason` 取面板判据）。
 */
export function ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element {
  const { root, changeId, cwd, readOnly, runActive, collapsed, onToggleCollapsed } = props;
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
      collapsed={collapsed}
      onToggleCollapsed={onToggleCollapsed}
    />
  );
}
