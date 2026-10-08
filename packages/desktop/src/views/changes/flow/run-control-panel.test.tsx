import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeRunStatus } from '../../../types/dto';
import type { UseChangeFlowRunResult } from '../hooks/use-change-flow-run';
import { RunControlPanel } from './run-control-panel';
import type { ChangeFlowRunState } from './run-state';

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
    expect(start).toHaveBeenLastCalledWith(false);
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

  it('failed 态徽章走 fail 变体（bg-destructive/10），运行期徽章走 default 变体（bg-primary）', () => {
    const failed = renderPanel(runStub({ state: runState({ status: 'failed' }) }));
    expect(screen.getByTestId('run-status').className).toContain('bg-destructive/10');
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

describe('RunControlPanel：发起区自动确认开关（AC-4）', () => {
  it('默认关：复选框未选中；未勾选直接点击发起 → start 携 false 调用恰一次（data-testid 查询）', () => {
    const start = vi.fn(async () => {});
    renderPanel(runStub({ start }));

    const checkbox = screen.getByTestId('run-auto-next-phase');
    expect((checkbox as HTMLInputElement).checked).toBe(false);

    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenLastCalledWith(false);
  });

  it('勾选开关后点击发起 → start 携 true 调用恰一次（开启后 start 携 autoNextPhase=true）', () => {
    const start = vi.fn(async () => {});
    renderPanel(runStub({ start }));

    const checkbox = screen.getByTestId('run-auto-next-phase');
    fireEvent.click(checkbox);
    expect((checkbox as HTMLInputElement).checked).toBe(true);

    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenLastCalledWith(true);
  });

  it('常驻轻提示文案：默认态与开启态均渲染同一行 muted 说明，不随开关状态条件渲染（D5）', () => {
    renderPanel(runStub());

    const hint = screen.getByText(/仅跳过相位间停等/);
    expect(hint.textContent).toContain('仅跳过相位间停等');
    expect(hint.textContent).toContain('ask 中断');
    expect(hint.textContent).toContain('不自动归档');
    expect(hint.className).toContain('text-muted-foreground');

    // 开启态：同一行文案仍在场（同一 DOM 节点——非条件渲染重建）
    fireEvent.click(screen.getByTestId('run-auto-next-phase'));
    expect(screen.getByText(/仅跳过相位间停等/)).toBe(hint);
  });

  it('运行期三态开关 disabled 定格：当前取值保持可见非隐藏、开关冻结不可切（jsdom 的 fireEvent 不复现浏览器对 disabled 控件的事件抑制——「切档无效」以 disabled 门为断言面）', () => {
    // 三运行态：开关在场（可见非隐藏）、disabled 定格、取值保留可见
    for (const status of ['running', 'waitingConfirm', 'waitingAsk'] as const) {
      const rendered = renderPanel(runStub({ state: runState({ status }) }));
      const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
      expect(checkbox.disabled).toBe(true);
      expect(checkbox.checked).toBe(false);
      rendered.unmount();
    }

    // 定格前已勾选的取值同样保留可见（定格呈现，非隐藏归零——真实浏览器
    // 中 disabled 控件不派发事件，切档无从发生）
    const first = renderPanel(runStub());
    fireEvent.click(screen.getByTestId('run-auto-next-phase'));
    first.rerender(
      <RunControlPanel
        change="add-feature"
        run={runStub({ state: runState({ status: 'running' }) })}
      />,
    );
    const running = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
    expect(running.disabled).toBe(true);
    expect(running.checked).toBe(true);
    first.unmount();
  });

  it('终局三态恢复可编辑且保留上次取值：勾选过的开关终局后仍选中（发起以点击时刻值为准）', () => {
    const start = vi.fn(async () => {});
    const first = renderPanel(runStub({ start }));
    fireEvent.click(screen.getByTestId('run-auto-next-phase'));

    for (const status of ['completed', 'stopped', 'failed'] as const) {
      first.rerender(
        <RunControlPanel
          change="add-feature"
          run={runStub({ start, state: runState({ status }) })}
        />,
      );
      const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
      expect(checkbox.disabled).toBe(false);
      expect(checkbox.checked).toBe(true);

      // 发起以点击时刻开关值为实参：终局后可直接携 true 重发
      fireEvent.click(screen.getByTestId('run-start'));
      expect(start).toHaveBeenLastCalledWith(true);
    }
    first.unmount();
  });

  it('发起失败（error 非空、state 保持 null）时开关可编辑且取值保留（未进入运行态——可改档后重试发起）', () => {
    const start = vi.fn(async () => {});
    renderPanel(runStub({ error: '同 change 已有并行 run', start }));

    const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
    expect(checkbox.disabled).toBe(false);
    expect(checkbox.checked).toBe(false);

    fireEvent.click(checkbox);
    expect(checkbox.checked).toBe(true);
    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenLastCalledWith(true);
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
    // 选项容器在场：卡片为「问题行 + 选项容器 + 应答行」三个直接子节点（守卫 true 半边）
    expect(screen.getByTestId('run-ask-card').children).toHaveLength(3);

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
    // 选项容器整体缺席（非「空容器在场」）：守卫为 false 时卡片只余问题行 + 应答行
    // 两个直接子节点——空壳包裹层不渲染（ask.options.length > 0 守卫可辨面）
    expect(screen.getByTestId('run-ask-card').children).toHaveLength(2);
    fireEvent.change(screen.getByTestId('run-ask-input'), { target: { value: '继续' } });
    fireEvent.click(screen.getByTestId('run-ask-submit'));
    expect(answer).toHaveBeenCalledTimes(1);
    expect(answer).toHaveBeenLastCalledWith('继续');
  });

  it("ask 输入初值：挂载即空串（未输入前不携脏值——初值 useState('') 可辨）", () => {
    renderPanel(
      runStub({
        state: runState({ status: 'waitingAsk', ask: { question: '继续还是停止？', options: [] } }),
      }),
    );
    expect(screen.getByTestId<HTMLInputElement>('run-ask-input').value).toBe('');
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

  it('state 非空且 finishedReason 为 null（running 运行中）：无收口记因（记因守卫不被 state 短路掩盖）', () => {
    renderPanel(runStub({ state: runState({ status: 'running', finishedReason: null }) }));
    expect(screen.queryByTestId('run-finished-reason')).toBeNull();
  });

  it('error 与 finishedReason 双 null：两者均不渲染（常态无占位）', () => {
    renderPanel(runStub());
    expect(screen.queryByTestId('run-error')).toBeNull();
    expect(screen.queryByTestId('run-finished-reason')).toBeNull();
  });
});
