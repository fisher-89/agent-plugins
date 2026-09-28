/**
 * explore composer：AgentInput 薄适配——渲染、档位逻辑、发送/停止收口在
 * 组件族（components/agent），本文件只保留探索页文案（label / placeholder）
 * 与 explore-* testid 前缀。stance 拼接在会话 hook（来源侧）。
 */

import { AgentInput } from '../../../components/agent';
import type { ExploreSendInput } from '../hooks/use-explore-session';

export interface ExploreComposerProps {
  /** 运行中禁用发送并呈现停止入口 */
  running: boolean;
  /** 发送上抛（stance 拼接与链续话由 session hook 承担） */
  onSend: (input: ExploreSendInput) => void;
  /** 停止上抛（触发 agent_stop） */
  onStop: () => void;
}

/**
 * composer：prompt 输入 + permission-mode 档位（默认 bypassPermissions）+
 * 发送（运行中禁发）+ 停止入口；id/testid 前缀 explore（explore-prompt /
 * explore-send / explore-stop / explore-permission-mode / explore-composer）。
 */
export function ExploreComposer({
  running,
  onSend,
  onStop,
}: ExploreComposerProps): React.JSX.Element {
  return (
    <AgentInput
      idPrefix="explore"
      label="探索输入"
      placeholder="想聊清楚什么？调研问题、思路、约束…"
      running={running}
      onSend={({ text, permissionMode }) => onSend({ prompt: text, permissionMode })}
      onStop={onStop}
    />
  );
}
