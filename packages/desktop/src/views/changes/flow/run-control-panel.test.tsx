import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { ActiveRunView, ChangeRunEntry, ChangeRunStatus } from '../../../types/dto';
import type { UseChangeFlowRunResult } from '../hooks/use-change-flow-run';
import { RunControlPanel } from './run-control-panel';

/** 固定 change id 字面量（命令面 prop——面板自身不寻址，展示面断言须与 id
 * 相异方可辨「id / name 不静默互换」）。 */
const CHANGE_ID = '0198f7a0-0000-7000-8000-000000000000';

/** 宽松默认值构造统一视图活面（ActiveRunView 同形）。 */
function activeRun(overrides: Partial<ActiveRunView> = {}): ActiveRunView {
  return {
    runId: 'run-1727',
    status: 'running',
    phase: 'implement',
    attempt: 2,
    ask: null,
    startedAt: '2026-10-01T08:00:00.000Z',
    steps: [],
    ...overrides,
  };
}

/** 宽松默认值构造 runs 尾行（ChangeRunEntry 同形）。 */
function lastRun(overrides: Partial<ChangeRunEntry> = {}): ChangeRunEntry {
  return {
    runId: 'run-1726',
    status: 'completed',
    reason: null,
    startedAt: '2026-10-01T07:00:00.000Z',
    finishedAt: '2026-10-01T07:30:00.000Z',
    steps: [],
    ...overrides,
  };
}

/** UseChangeFlowRunResult 动作面替身：四个操作为 spy（入参例外）。 */
function runStub(overrides: Partial<UseChangeFlowRunResult> = {}): UseChangeFlowRunResult {
  return {
    start: vi.fn(async () => {}),
    stop: vi.fn(async () => {}),
    confirm: vi.fn(async () => {}),
    answer: vi.fn(async () => {}),
    error: null,
    ...overrides,
  };
}

/** 面板 props 装配：命令面 changeId 与展示面 name 分离注入（缺省 id 取固定
 * 字面量、name 取可辨展示值——显名收敛纪律的断言面）。 */
function renderPanel(
  actions: UseChangeFlowRunResult,
  face: { activeRun: ActiveRunView | null; lastRun: ChangeRunEntry | null } = {
    activeRun: null,
    lastRun: null,
  },
  identity: { changeId?: string; name?: string } = {},
) {
  return render(
    <RunControlPanel
      changeId={identity.changeId ?? CHANGE_ID}
      name={identity.name ?? 'add-feature'}
      activeRun={face.activeRun}
      lastRun={face.lastRun}
      actions={actions}
    />,
  );
}

describe('RunControlPanel：主操作与生命周期对齐', () => {
  it('activeRun null（空闲）：发起为主操作且点击触发 start、停止不呈现、状态徽章缺席', () => {
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
    renderPanel(runStub({ stop }), { activeRun: activeRun(), lastRun: null });

    expect(screen.getByTestId('run-stop') !== null).toBe(true);
    expect(screen.queryByTestId('run-start')).toBeNull();
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');

    fireEvent.click(screen.getByTestId('run-stop'));
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it('活面六态操作矩阵：运行期三态呈现停止、状态徽章文案逐态对齐不回归', () => {
    const matrix: Array<{ status: ChangeRunStatus; badge: string }> = [
      { status: 'running', badge: '运行中' },
      { status: 'waitingConfirm', badge: '等待确认' },
      { status: 'waitingAsk', badge: '等待应答' },
    ];
    for (const cell of matrix) {
      const rendered = renderPanel(runStub(), {
        activeRun: activeRun({ status: cell.status }),
        lastRun: null,
      });
      expect(screen.getByTestId('run-status').textContent).toBe(cell.badge);
      expect(screen.queryByTestId('run-start')).toBeNull();
      expect(screen.getByTestId('run-stop') !== null).toBe(true);
      rendered.unmount();
    }
  });

  it('failed 态徽章走 fail 变体（bg-destructive/10），运行期徽章走 default 变体（bg-primary）', () => {
    const failed = renderPanel(runStub(), {
      activeRun: null,
      lastRun: lastRun({ status: 'failed', reason: 'CLI 漂移' }),
    });
    expect(screen.getByTestId('run-status').className).toContain('bg-destructive/10');
    failed.unmount();

    renderPanel(runStub(), { activeRun: activeRun(), lastRun: null });
    expect(screen.getByTestId('run-status').className).toContain('bg-primary');
  });

  it('props 收敛面：aria-label 取展示面 name（changeId 命令面不入展示——id / name 不静默互换）', () => {
    renderPanel(
      runStub(),
      { activeRun: null, lastRun: null },
      { changeId: CHANGE_ID, name: 'my-change' },
    );

    const panel = screen.getByTestId('run-control-panel');
    expect(panel.getAttribute('aria-label')).toBe('change my-change 运行控制');
    // 展示面零 id 残留（显名收敛：面板文案不泄漏身份键）
    expect(panel.textContent).not.toContain(CHANGE_ID);
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

  it('运行期开关 disabled 定格：activeRun 在场即冻结（活面判别非状态值枚举）', () => {
    const rendered = renderPanel(runStub(), { activeRun: activeRun(), lastRun: null });
    const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
    expect(checkbox.disabled).toBe(true);
    expect(checkbox.checked).toBe(false);
    rendered.unmount();
  });

  it('终局恢复可编辑且保留上次取值：activeRun 退场后勾选过的开关仍选中（发起以点击时刻值为准）', () => {
    const start = vi.fn(async () => {});
    const first = renderPanel(runStub({ start }));
    fireEvent.click(screen.getByTestId('run-auto-next-phase'));

    first.rerender(
      <RunControlPanel
        changeId={CHANGE_ID}
        name="add-feature"
        activeRun={null}
        lastRun={lastRun({ status: 'failed', reason: 'CLI 漂移' })}
        actions={runStub({ start })}
      />,
    );
    const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
    expect(checkbox.disabled).toBe(false);
    expect(checkbox.checked).toBe(true);

    // 发起以点击时刻开关值为实参：终局后可直接携 true 重发
    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenLastCalledWith(true);
    first.unmount();
  });

  it('发起失败（error 非空、activeRun null）时开关可编辑且取值保留（未进入运行态——可改档后重试发起）', () => {
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
  it('activeRun waitingConfirm：确认卡片呈现相位名；继续 / 终止分别触发 confirm(true / false)', () => {
    const confirm = vi.fn(async () => {});
    renderPanel(runStub({ confirm }), {
      activeRun: activeRun({
        status: 'waitingConfirm',
        phase: 'dev-design',
        attempt: 1,
      }),
      lastRun: null,
    });

    const card = screen.getByTestId('run-confirm-card');
    expect(card.textContent).toContain('dev-design');

    fireEvent.click(screen.getByTestId('run-confirm-proceed'));
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(confirm).toHaveBeenLastCalledWith(true);

    fireEvent.click(screen.getByTestId('run-confirm-stop'));
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(confirm).toHaveBeenLastCalledWith(false);
  });

  it('activeRun running（phase 非停等态）：无确认卡片', () => {
    renderPanel(runStub(), { activeRun: activeRun(), lastRun: null });
    expect(screen.queryByTestId('run-confirm-card')).toBeNull();
  });
});

describe('RunControlPanel：waitingAsk 卡片（AC-2 ask 卡片 UI 半边）', () => {
  it('问题与选项呈现：点击选项 → answer(option) 原样回流', () => {
    const answer = vi.fn(async () => {});
    renderPanel(
      runStub({
        answer,
      }),
      {
        activeRun: activeRun({
          status: 'waitingAsk',
          ask: { question: 'backtrack 到哪个相位？', options: ['dev-design', 'test-design'] },
        }),
        lastRun: null,
      },
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
    renderPanel(runStub({ answer }), {
      activeRun: activeRun({
        status: 'waitingAsk',
        ask: { question: '继续还是停止？', options: [] },
      }),
      lastRun: null,
    });
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
    renderPanel(runStub({ answer }), {
      activeRun: activeRun({
        status: 'waitingAsk',
        ask: { question: '是否继续？', options: [] },
      }),
      lastRun: null,
    });

    expect(screen.queryAllByTestId('run-ask-option')).toHaveLength(0);
    fireEvent.change(screen.getByTestId('run-ask-input'), { target: { value: '继续' } });
    fireEvent.click(screen.getByTestId('run-ask-submit'));
    expect(answer).toHaveBeenCalledTimes(1);
    expect(answer).toHaveBeenLastCalledWith('继续');
  });

  it('ask 输入初值：挂载即空串（未输入前不携脏值——初值 useState 可辨）', () => {
    renderPanel(runStub(), {
      activeRun: activeRun({
        status: 'waitingAsk',
        ask: { question: '继续还是停止？', options: [] },
      }),
      lastRun: null,
    });
    expect(screen.getByTestId<HTMLInputElement>('run-ask-input').value).toBe('');
  });

  it('activeRun.ask 为 null（running）：无 ask 卡片', () => {
    renderPanel(runStub(), { activeRun: activeRun(), lastRun: null });
    expect(screen.queryByTestId('run-ask-card')).toBeNull();
  });
});

describe('RunControlPanel：error 呈现与终态记因（runs 尾行，D9）', () => {
  it('error 非空：错误条呈现原文（发起失败可见可重试——发起入口仍在）', () => {
    renderPanel(runStub({ error: '同 change 已有并行 run' }));
    expect(screen.getByTestId('run-error').textContent).toBe('同 change 已有并行 run');
    expect(screen.getByTestId('run-start') !== null).toBe(true);
  });

  it('lastRun.reason 非空：收口记因呈现（前缀「收口：」——activeRun 退场后 runs 尾行供能）', () => {
    renderPanel(runStub(), {
      activeRun: null,
      lastRun: lastRun({ status: 'failed', reason: 'CLI 漂移' }),
    });
    expect(screen.getByTestId('run-finished-reason').textContent).toBe('收口：CLI 漂移');
  });

  it('lastRun.reason null：无收口记因（记因守卫不被尾行短路掩盖）', () => {
    renderPanel(runStub(), {
      activeRun: null,
      lastRun: lastRun({ status: 'completed', reason: null }),
    });
    expect(screen.queryByTestId('run-finished-reason')).toBeNull();
  });

  it('error 与尾行记因双 null：两者均不渲染（常态无占位）', () => {
    renderPanel(runStub());
    expect(screen.queryByTestId('run-error')).toBeNull();
    expect(screen.queryByTestId('run-finished-reason')).toBeNull();
  });

  it('runs 尾行终态徽章逐档对齐（interrupted 标定值可辨——RunStatus 五值封闭集）', () => {
    const matrix: Array<{ status: ChangeRunEntry['status']; badge: string }> = [
      { status: 'running', badge: '运行中' },
      { status: 'completed', badge: '已完成' },
      { status: 'stopped', badge: '已停止' },
      { status: 'failed', badge: '失败' },
      { status: 'interrupted', badge: '已中断' },
    ];
    for (const cell of matrix) {
      const rendered = renderPanel(runStub(), {
        activeRun: null,
        lastRun: lastRun({ status: cell.status }),
      });
      expect(screen.getByTestId('run-status').textContent).toBe(cell.badge);
      rendered.unmount();
    }
  });
});
