import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeRunStatus } from '../../../types/dto';
import type { UseChangeFlowRunResult } from '../hooks/use-change-flow-run';
import { RunControlPanel } from './run-control-panel';
import type { ChangeFlowRunState } from './run-state';

// ---------------------------------------------------------------------------
// RunControlPanel 单测：hook 边界以 UseChangeFlowRunResult 替身直传（可编程
// state 六态 fixture 与 start/stop/confirm/answer spy），无 invoke 参与——
// hook 契约在 use-change-flow-run.test.ts 锁定。生命周期状态与可用操作对齐
// 矩阵 / 确认卡片 / ask 卡片 / error 与终态记因（AC-5 控制入口 + AC-2 ask 半边）。
// ---------------------------------------------------------------------------

/** 宽松默认值构造 run 视图模型（run-state.ts ChangeFlowRunState 同形）。 */
function runState(overrides: Partial<ChangeFlowRunState> = {}): ChangeFlowRunState {
  return {
    runId: 'run-1727',
    status: 'running',
    phase: 'implement',
    attempt: 2,
    ask: null,
    confirmPhase: null,
    steps: [],
    finishedReason: null,
    liveEvents: {},
    ...overrides,
  };
}

/** UseChangeFlowRunResult 替身：state 可编程、四个操作为 spy（入参例外）。 */
function runStub(overrides: Partial<UseChangeFlowRunResult> = {}): UseChangeFlowRunResult {
  return {
    state: null,
    start: vi.fn(async () => {}),
    stop: vi.fn(async () => {}),
    confirm: vi.fn(async () => {}),
    answer: vi.fn(async () => {}),
    error: null,
    ...overrides,
  };
}

function renderPanel(run: UseChangeFlowRunResult, change = 'add-feature') {
  return render(<RunControlPanel change={change} run={run} />);
}

describe('RunControlPanel：主操作与生命周期对齐', () => {
  it('state null（空闲）：发起为主操作且点击触发 start、停止不呈现、状态徽章缺席', () => {
    const start = vi.fn(async () => {});
    renderPanel(runStub({ start }));

    expect(screen.getByTestId('run-start') !== null).toBe(true);
    expect(screen.queryByTestId('run-stop')).toBeNull();
    expect(screen.queryByTestId('run-status')).toBeNull();

    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenCalledTimes(1);
  });

  it('running：停止为主操作且点击触发 stop、发起入口退位、状态徽章呈运行中', () => {
    const stop = vi.fn(async () => {});
    renderPanel(runStub({ state: runState({ status: 'running' }), stop }));

    expect(screen.getByTestId('run-stop') !== null).toBe(true);
    expect(screen.queryByTestId('run-start')).toBeNull();
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');

    fireEvent.click(screen.getByTestId('run-stop'));
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it('六态操作矩阵：运行期三态呈现停止、终局三态回到发起；状态徽章文案逐态对齐不回归', () => {
    const matrix: Array<{ status: ChangeRunStatus; badge: string; terminal: boolean }> = [
      { status: 'running', badge: '运行中', terminal: false },
      { status: 'waitingConfirm', badge: '等待确认', terminal: false },
      { status: 'waitingAsk', badge: '等待应答', terminal: false },
      { status: 'completed', badge: '已完成', terminal: true },
      { status: 'stopped', badge: '已停止', terminal: true },
      { status: 'failed', badge: '失败', terminal: true },
    ];
    for (const cell of matrix) {
      const rendered = renderPanel(runStub({ state: runState({ status: cell.status }) }));
      expect(screen.getByTestId('run-status').textContent).toBe(cell.badge);
      if (cell.terminal) {
        // 收口对齐：停止不再为主操作
        expect(screen.queryByTestId('run-stop')).toBeNull();
        expect(screen.getByTestId('run-start') !== null).toBe(true);
      } else {
        expect(screen.queryByTestId('run-start')).toBeNull();
        expect(screen.getByTestId('run-stop') !== null).toBe(true);
      }
      rendered.unmount();
    }
  });

  it('failed 态徽章走 fail 变体（bg-fail-bg），运行期徽章走 active 变体（bg-primary）', () => {
    const failed = renderPanel(runStub({ state: runState({ status: 'failed' }) }));
    expect(screen.getByTestId('run-status').className).toContain('bg-fail-bg');
    failed.unmount();

    renderPanel(runStub({ state: runState({ status: 'running' }) }));
    expect(screen.getByTestId('run-status').className).toContain('bg-primary');
  });

  it('aria-label 携 change 名（面板寻址可辨）', () => {
    renderPanel(runStub(), 'my-change');
    expect(screen.getByTestId('run-control-panel').getAttribute('aria-label')).toBe(
      'change my-change 运行控制',
    );
  });
});

describe('RunControlPanel：waitingConfirm 卡片（phase 间拍板点）', () => {
  it('confirmPhase 非空：确认卡片呈现相位名；继续 / 终止分别触发 confirm(true / false)', () => {
    const confirm = vi.fn(async () => {});
    renderPanel(
      runStub({
        state: runState({
          status: 'waitingConfirm',
          confirmPhase: 'dev-design',
          phase: 'dev-design',
        }),
        confirm,
      }),
    );

    const card = screen.getByTestId('run-confirm-card');
    expect(card.textContent).toContain('dev-design');

    fireEvent.click(screen.getByTestId('run-confirm-proceed'));
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(confirm).toHaveBeenLastCalledWith(true);

    fireEvent.click(screen.getByTestId('run-confirm-stop'));
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(confirm).toHaveBeenLastCalledWith(false);
  });

  it('confirmPhase 为 null（running）：无确认卡片', () => {
    renderPanel(runStub({ state: runState({ status: 'running' }) }));
    expect(screen.queryByTestId('run-confirm-card')).toBeNull();
  });
});

describe('RunControlPanel：waitingAsk 卡片（AC-2 ask 卡片 UI 半边）', () => {
  it('问题与选项呈现：点击选项 → answer(option) 原样回流', () => {
    const answer = vi.fn(async () => {});
    renderPanel(
      runStub({
        state: runState({
          status: 'waitingAsk',
          ask: { question: 'backtrack 到哪个相位？', options: ['dev-design', 'test-design'] },
        }),
        answer,
      }),
    );

    expect(screen.getByTestId('run-ask-question').textContent).toBe('backtrack 到哪个相位？');
    const options = screen.getAllByTestId('run-ask-option');
    expect(options.map((option) => option.textContent)).toEqual(['dev-design', 'test-design']);

    fireEvent.click(options[1]);
    expect(answer).toHaveBeenCalledTimes(1);
    expect(answer).toHaveBeenLastCalledWith('test-design');
  });

  it('自由文本应答：非空提交触发 answer(trimmed) 且输入清空；空白提交不触发', () => {
    const answer = vi.fn(async () => {});
    renderPanel(
      runStub({
        state: runState({ status: 'waitingAsk', ask: { question: '继续还是停止？', options: [] } }),
        answer,
      }),
    );
    const input = screen.getByTestId('run-ask-input');
    const submit = screen.getByTestId('run-ask-submit');

    fireEvent.change(input, { target: { value: '  追加约束  ' } });
    fireEvent.click(submit);
    expect(answer).toHaveBeenCalledTimes(1);
    expect(answer).toHaveBeenLastCalledWith('追加约束');
    expect((input as HTMLInputElement).value).toBe('');

    fireEvent.change(input, { target: { value: '   ' } });
    fireEvent.click(submit);
    expect(answer).toHaveBeenCalledTimes(1);
  });

  it('options 空数组：无选项按钮、自由应答通道仍可用', () => {
    const answer = vi.fn(async () => {});
    renderPanel(
      runStub({
        state: runState({ status: 'waitingAsk', ask: { question: '是否继续？', options: [] } }),
        answer,
      }),
    );

    expect(screen.queryAllByTestId('run-ask-option')).toHaveLength(0);
    fireEvent.change(screen.getByTestId('run-ask-input'), { target: { value: '继续' } });
    fireEvent.click(screen.getByTestId('run-ask-submit'));
    expect(answer).toHaveBeenCalledTimes(1);
    expect(answer).toHaveBeenLastCalledWith('继续');
  });

  it('ask 为 null（running）：无 ask 卡片', () => {
    renderPanel(runStub({ state: runState({ status: 'running' }) }));
    expect(screen.queryByTestId('run-ask-card')).toBeNull();
  });
});

describe('RunControlPanel：error 呈现与终态记因', () => {
  it('error 非空：错误条呈现原文（发起失败可见可重试——发起入口仍在）', () => {
    renderPanel(runStub({ error: '同 change 已有并行 run' }));
    expect(screen.getByTestId('run-error').textContent).toBe('同 change 已有并行 run');
    expect(screen.getByTestId('run-start') !== null).toBe(true);
  });

  it('finishedReason 非空：收口记因呈现（前缀「收口：」）', () => {
    renderPanel(runStub({ state: runState({ status: 'failed', finishedReason: 'CLI 漂移' }) }));
    expect(screen.getByTestId('run-finished-reason').textContent).toBe('收口：CLI 漂移');
  });

  it('error 与 finishedReason 双 null：两者均不渲染（常态无占位）', () => {
    renderPanel(runStub());
    expect(screen.queryByTestId('run-error')).toBeNull();
    expect(screen.queryByTestId('run-finished-reason')).toBeNull();
  });
});
