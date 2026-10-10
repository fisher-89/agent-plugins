/**
 * 运行控制面板：发起 / 停止 / phase 间确认（继续 / 终止）/ ask 中断卡片
 *（问题 + 选项 + 自由文本应答）。状态面 = activeRun 活面（六值状态机 /
 * 停等卡片自统一查询 activeRun）∪ lastRun runs 尾行（收口后终态徽章与收口
 * 记因——unify-run-state-persistence D9：头部「上次运行」信息面经 runs 尾
 * 行可达，activeRun 退场后呈现）；生命周期状态与可用操作对齐——运行中主操
 * 作是停止，终局后主操作回到发起。
 */
import { useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type {
  ActiveRunView,
  AskPayload,
  ChangeRunEntry,
  ChangeRunStatus,
  RunStatus,
} from '../../../types/dto';
import type { UseChangeFlowRunResult } from '../hooks/use-change-flow-run';

/** 活面状态徽章文案（六档状态机镜像）。 */
const ACTIVE_STATUS_LABEL: Record<ChangeRunStatus, string> = {
  running: '运行中',
  waitingConfirm: '等待确认',
  waitingAsk: '等待应答',
  completed: '已完成',
  stopped: '已停止',
  failed: '失败',
};

/** 库读史终态徽章文案（RunStatus 五值；interrupted 仅启动标定产生）。 */
const LAST_RUN_STATUS_LABEL: Record<RunStatus, string> = {
  running: '运行中',
  completed: '已完成',
  stopped: '已停止',
  failed: '失败',
  interrupted: '已中断',
};

/** 动作面（useChangeFlowRun 缩位返回型直用）。 */
type RunActions = Pick<UseChangeFlowRunResult, 'start' | 'stop' | 'confirm' | 'answer' | 'error'>;

interface RunControlPanelProps {
  /** change id（命令面透传——动作经 useChangeFlowRun 的 id 链路） */
  changeId: string;
  /** change 名（展示面：aria-label / 文案取 `detail.name`） */
  name: string;
  /** 在飞 run 活面（统一视图 activeRun；终态即除名 → null） */
  activeRun: ActiveRunView | null;
  /** runs 尾行（库读史最近一次 run——收口后终态徽章与收口记因） */
  lastRun: ChangeRunEntry | null;
  actions: RunActions;
}

/** 发起 / 停止主操作行 */
function RunActions({
  actions,
  active,
}: {
  actions: RunActions;
  active: boolean;
}): React.JSX.Element {
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
          <Button onClick={() => void actions.stop()} data-testid="run-stop">
            停止
          </Button>
        ) : (
          <Button onClick={() => void actions.start(autoNextPhase)} data-testid="run-start">
            发起运行
          </Button>
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
  actions,
  phase,
}: {
  actions: RunActions;
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
        <Button onClick={() => void actions.confirm(true)} data-testid="run-confirm-proceed">
          继续
        </Button>
        <Button onClick={() => void actions.confirm(false)} data-testid="run-confirm-stop">
          终止运行
        </Button>
      </div>
    </div>
  );
}

/** ask 中断卡片：问题 + 选项按钮 + 自由文本应答（应答后 Continue 决策会话）。 */
function AskCard({ actions, ask }: { actions: RunActions; ask: AskPayload }): React.JSX.Element {
  const [text, setText] = useState('');
  const submit = (): void => {
    const trimmed = text.trim();
    if (trimmed === '') return;
    setText('');
    void actions.answer(trimmed);
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
              onClick={() => void actions.answer(option)}
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

/** 运行控制面板：状态行 + 停等卡片 + 终态记因。状态徽章自活面（运行中六
 * 值状态机）；activeRun 退场后经 runs 尾行呈现收口终态。 */
/** 状态徽章行：活面六值状态机优先；activeRun 退场后经 runs 尾行呈现收口
 * 终态（unify-run-state-persistence D9）。 */
function RunStatusBadge({
  activeRun,
  lastRun,
}: {
  activeRun: ActiveRunView | null;
  lastRun: ChangeRunEntry | null;
}): React.JSX.Element | null {
  if (activeRun !== null) {
    return (
      <Badge variant={activeRun.status === 'failed' ? 'fail' : 'default'} data-testid="run-status">
        {ACTIVE_STATUS_LABEL[activeRun.status]}
      </Badge>
    );
  }
  if (lastRun !== null) {
    return (
      <Badge variant={lastRun.status === 'failed' ? 'fail' : 'default'} data-testid="run-status">
        {LAST_RUN_STATUS_LABEL[lastRun.status]}
      </Badge>
    );
  }
  return null;
}

export function RunControlPanel({
  // 命令面身份入参保契约面（动作链路已由父层 useChangeFlowRun 按 id 绑定；
  // 面板显名收敛纪律：id 与 name 不静默互换）
  changeId: _changeId,
  name,
  activeRun,
  lastRun,
  actions,
}: RunControlPanelProps): React.JSX.Element {
  const active = activeRun !== null;
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="run-control-panel"
      aria-label={`change ${name} 运行控制`}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">运行控制</h2>
        <RunActions actions={actions} active={active} />
      </div>
      <div className="flex items-center gap-2">
        <RunStatusBadge activeRun={activeRun} lastRun={lastRun} />
      </div>
      {actions.error !== null && (
        <div
          className="mt-2 break-all rounded-md bg-fail-bg px-2.5 py-1.5 text-[13px] text-fail"
          data-testid="run-error"
        >
          {actions.error}
        </div>
      )}
      {activeRun !== null && activeRun.status === 'waitingConfirm' && activeRun.phase !== null && (
        <ConfirmCard actions={actions} phase={activeRun.phase} />
      )}
      {activeRun !== null && activeRun.ask !== null && (
        <AskCard actions={actions} ask={activeRun.ask} />
      )}
      {activeRun === null && lastRun !== null && lastRun.reason !== null && (
        <div
          className="mt-2 break-words text-[13px] text-muted-foreground"
          data-testid="run-finished-reason"
        >
          收口：{lastRun.reason}
        </div>
      )}
    </section>
  );
}
