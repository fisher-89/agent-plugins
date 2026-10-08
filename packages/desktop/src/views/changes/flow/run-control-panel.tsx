/**
 * 运行控制面板：发起 / 停止 / phase 间确认（继续 / 终止）/ ask 中断卡片
 *（问题 + 选项 + 自由文本应答）。生命周期状态与可用操作对齐——运行中主
 * 操作是停止，终局后主操作回到发起（收口后停止不再呈现）。
 */
import { useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { ChangeRunStatus } from '../../../types/dto';
import type { UseChangeFlowRunResult } from '../hooks/use-change-flow-run';

/** 状态徽章文案（六档状态机镜像）。 */
const STATUS_LABEL: Record<ChangeRunStatus, string> = {
  running: '运行中',
  waitingConfirm: '等待确认',
  waitingAsk: '等待应答',
  completed: '已完成',
  stopped: '已停止',
  failed: '失败',
};

function isTerminal(status: ChangeRunStatus): boolean {
  return status === 'completed' || status === 'stopped' || status === 'failed';
}

interface RunControlPanelProps {
  change: string;
  run: UseChangeFlowRunResult;
}

/** 发起 / 停止主操作行 */
function RunActions({ run }: { run: UseChangeFlowRunResult }): React.JSX.Element {
  const active = run.state !== null && !isTerminal(run.state.status);
  const [autoNextPhase, setAutoNextPhase] = useState(false);
  return (
    <div className="flex flex-col items-end gap-1">
      <div className="flex items-center gap-2">
        <label className="flex items-center gap-1.5 text-[13px]">
          <input
            type="checkbox"
            checked={autoNextPhase}
            disabled={active}
            onChange={(event) => setAutoNextPhase(event.target.checked)}
            data-testid="run-auto-next-phase"
          />
          自动确认步骤
        </label>
        {active ? (
          <Button onClick={() => void run.stop()} data-testid="run-stop">
            停止
          </Button>
        ) : (
          <Button onClick={() => void run.start(autoNextPhase)} data-testid="run-start">
            发起运行
          </Button>
        )}
        {run.state !== null && (
          <Badge
            variant={run.state.status === 'failed' ? 'fail' : 'default'}
            data-testid="run-status"
          >
            {STATUS_LABEL[run.state.status]}
          </Badge>
        )}
      </div>
      <span className="text-xs text-muted-foreground">
        自动确认：仅跳过相位间停等；ask 中断、失败与终态收口仍停下等待，不自动归档。
      </span>
    </div>
  );
}

/** phase 间停等确认卡片（proceed=false → 受控终态 stopped）。 */
function ConfirmCard({
  run,
  phase,
}: {
  run: UseChangeFlowRunResult;
  phase: string;
}): React.JSX.Element {
  return (
    <div
      className="mt-2 rounded-md border border-border bg-card px-3 py-2"
      data-testid="run-confirm-card"
    >
      <div className="text-[13px]">
        相位 <span className="text-muted-foreground">{phase}</span> 已收口，是否继续推进下一相位？
      </div>
      <div className="mt-2 flex gap-2">
        <Button onClick={() => void run.confirm(true)} data-testid="run-confirm-proceed">
          继续
        </Button>
        <Button onClick={() => void run.confirm(false)} data-testid="run-confirm-stop">
          终止运行
        </Button>
      </div>
    </div>
  );
}

/** ask 中断卡片：问题 + 选项按钮 + 自由文本应答（应答后 Continue 决策会话）。 */
function AskCard({
  run,
  ask,
}: {
  run: UseChangeFlowRunResult;
  ask: { question: string; options: string[] };
}): React.JSX.Element {
  const [text, setText] = useState('');
  const submit = (): void => {
    const trimmed = text.trim();
    if (trimmed === '') return;
    setText('');
    void run.answer(trimmed);
  };
  return (
    <div
      className="mt-2 rounded-md border border-warn bg-warn-bg px-3 py-2"
      data-testid="run-ask-card"
    >
      <div className="break-words text-[13px]" data-testid="run-ask-question">
        {ask.question}
      </div>
      {ask.options.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1.5">
          {ask.options.map((option) => (
            <Button
              key={option}
              onClick={() => void run.answer(option)}
              data-testid="run-ask-option"
            >
              {option}
            </Button>
          ))}
        </div>
      )}
      <div className="mt-2 flex gap-2">
        <input
          className="w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none"
          placeholder="自由应答…"
          value={text}
          onChange={(event) => setText(event.target.value)}
          data-testid="run-ask-input"
        />
        <Button onClick={submit} data-testid="run-ask-submit">
          应答
        </Button>
      </div>
    </div>
  );
}

/** 运行控制面板：状态行 + 停等卡片 + 终态记因。 */
export function RunControlPanel({ change, run }: RunControlPanelProps): React.JSX.Element {
  const state = run.state;
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="run-control-panel"
      aria-label={`change ${change} 运行控制`}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">运行控制</h2>
        <RunActions run={run} />
      </div>
      {run.error !== null && (
        <div
          className="mt-2 break-all rounded-md bg-fail-bg px-2.5 py-1.5 text-[13px] text-fail"
          data-testid="run-error"
        >
          {run.error}
        </div>
      )}
      {state !== null && state.confirmPhase !== null && (
        <ConfirmCard run={run} phase={state.confirmPhase} />
      )}
      {state !== null && state.ask !== null && <AskCard run={run} ask={state.ask} />}
      {state !== null && state.finishedReason !== null && (
        <div
          className="mt-2 break-words text-[13px] text-muted-foreground"
          data-testid="run-finished-reason"
        >
          收口：{state.finishedReason}
        </div>
      )}
    </section>
  );
}
