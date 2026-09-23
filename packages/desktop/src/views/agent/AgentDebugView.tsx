import { useState } from 'react';

import { Button } from '@/components/ui/button';

import { AgentEventTimeline } from './components/AgentEventTimeline';
import { AgentRawStream } from './components/AgentRawStream';
import { AgentRunForm } from './components/AgentRunForm';
import { AgentRunHistory } from './components/AgentRunHistory';
import { useAgentRun } from './hooks/useAgentRun';
import { useAgentRunHistory } from './hooks/useAgentRunHistory';

export interface AgentDebugViewProps {
  /** 当前 workspace root（cwd 隐含来源）；null 即未选定 workspace */
  root: string | null;
}

/**
 * Agent 调试页：参数面 + 实时时间线（原始 JSONL 切换）+ 历史运行区。
 * 实时流经 useAgentRun 的 Channel 订阅（执行流通道例外）；历史列表与重放
 * 经 useAgentRunHistory 的显式 invoke 查询；无路由（页面切换由 App 层
 * 顶层 state 承担）。
 */
export function AgentDebugView({ root }: AgentDebugViewProps): React.JSX.Element {
  const run = useAgentRun(root);
  const history = useAgentRunHistory();
  const [showRaw, setShowRaw] = useState(false);

  if (root === null) {
    return (
      <div className="text-muted-foreground" data-testid="agent-no-root">
        请先在侧栏选择一个 workspace。
      </div>
    );
  }

  return (
    <div>
      <AgentRunForm disabled={run.running} onStart={run.start} />
      {run.error !== null && (
        <div
          className="mb-4 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="run-error"
        >
          运行发起失败：{run.error}
        </div>
      )}
      <div className="mb-3 flex items-center gap-2" data-testid="stream-toggle">
        <span className="text-xs text-muted-foreground">实时流视图</span>
        <Button
          aria-pressed={!showRaw}
          className={showRaw ? 'bg-muted text-muted-foreground' : undefined}
          data-testid="toggle-timeline"
          onClick={() => setShowRaw(false)}
        >
          时间线
        </Button>
        <Button
          aria-pressed={showRaw}
          className={showRaw ? undefined : 'bg-muted text-muted-foreground'}
          data-testid="toggle-raw"
          onClick={() => setShowRaw(true)}
        >
          原始 JSONL
        </Button>
      </div>
      {showRaw ? (
        <AgentRawStream events={run.events} />
      ) : (
        <AgentEventTimeline events={run.events} running={run.running} />
      )}
      <AgentRunHistory state={history} />
    </div>
  );
}
