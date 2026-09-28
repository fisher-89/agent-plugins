import { useState } from 'react';

import { Button } from '@/components/ui/button';

import { AgentTimeline } from '../../components/agent';
import { useAgentChat } from '../../hooks/use-agent-chat';
import { AgentRawStream } from './components/agent-raw-stream';
import { AgentRunForm, type AgentStartInput } from './components/agent-run-form';
import { AgentRunHistory } from './components/agent-run-history';
import { useAgentRunHistory } from './hooks/use-agent-run-history';

export interface AgentDebugViewProps {
  /** 当前 workspace root（cwd 隐含来源）；null 即未选定 workspace */
  root: string | null;
}

/** 视图切换行（页面级 chrome）：时间线 / 原始 JSONL 切换 + 运行中停止入口 */
function StreamToggle({
  showRaw,
  onToggle,
  running,
  onStop,
}: {
  showRaw: boolean;
  onToggle: (showRaw: boolean) => void;
  running: boolean;
  onStop: () => void;
}): React.JSX.Element {
  return (
    <div className="mb-3 flex items-center gap-2" data-testid="stream-toggle">
      <span className="text-xs text-muted-foreground">实时流视图</span>
      <Button
        aria-pressed={!showRaw}
        className={showRaw ? 'bg-muted text-muted-foreground' : undefined}
        data-testid="toggle-timeline"
        onClick={() => onToggle(false)}
      >
        时间线
      </Button>
      <Button
        aria-pressed={showRaw}
        className={showRaw ? undefined : 'bg-muted text-muted-foreground'}
        data-testid="toggle-raw"
        onClick={() => onToggle(true)}
      >
        原始 JSONL
      </Button>
      {running && (
        <Button className="ml-auto" data-testid="run-stop" onClick={onStop}>
          停止运行
        </Button>
      )}
    </div>
  );
}

/** 错误横幅：启动阶段失败（CLI 缺失 / spawn 失败 / store 失败）直出 */
function RunErrorBanner({ error }: { error: string }): React.JSX.Element {
  return (
    <div
      className="mb-4 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
      data-testid="run-error"
    >
      运行发起失败：{error}
    </div>
  );
}

/**
 * Agent 调试页：参数面 + 实时时间线（原始 JSONL 切换）+ 运行中停止入口 +
 * 历史运行区。页面状态经统一会话基建 use-agent-chat 承载（source="debug"、
 * sourceRef=null 不重放装载，每次发起先 reset 防串场）；时间线经 AgentTimeline
 * 保真透镜、原始流消费 chat 的 events 镜像；run 表单 / 历史列表 / JSONL 开关
 * 为页面级 chrome，包在共享核心外圈。布局接入壳层 flex 链：根 flex-1，流视图
 * 区（时间线 / 原始 JSONL 二选一）填充剩余区域并内部滚动，表单 / 横幅 / 切换
 * 行 / 历史区自然高。历史列表与重放经 useAgentRunHistory 的
 * 显式 invoke 查询；实时流属执行流通道（agent_start 命令作用域），不属
 * 「刷新取数模型」禁止的轮询取数。无路由（页面切换由 App 层顶层 state 承担）。
 */
export function AgentDebugView({ root }: AgentDebugViewProps): React.JSX.Element {
  const history = useAgentRunHistory();
  const [showRaw, setShowRaw] = useState(false);
  const session = useAgentChat({ source: 'debug', sourceRef: null, root });

  // 每跑重置：上一跑 messages / events / chain 清空，不串场
  const start = (input: AgentStartInput) => {
    session.reset();
    session.sendMessage(input);
  };

  if (root === null) {
    return (
      <div className="text-muted-foreground" data-testid="agent-no-root">
        请先在侧栏选择一个 workspace。
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <AgentRunForm disabled={session.running} onStart={start} />
      {session.error !== null && <RunErrorBanner error={session.error} />}
      <StreamToggle
        showRaw={showRaw}
        onToggle={setShowRaw}
        running={session.running}
        onStop={session.stop}
      />
      {showRaw ? (
        <AgentRawStream events={session.events} />
      ) : (
        <AgentTimeline messages={session.messages} running={session.running} />
      )}
      <AgentRunHistory state={history} />
    </div>
  );
}
