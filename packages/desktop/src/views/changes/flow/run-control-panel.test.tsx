import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type {
  ActiveRunView,
  ArchivePreflight,
  ChangeRunEntry,
  ChangeRunStatus,
} from '../../../types/dto';
import type { UseChangeFlowRunResult } from '../hooks/use-change-flow-run';
import { RunControlPanel } from './run-control-panel';

// ---------------------------------------------------------------------------
// 进程边界 mock（archi 章节）：`archi_change_summary` / `archi_graph` /
// `archi_module` 按命令名分发。既有用例以 root=null 挂载（不触发取数），
// 故该 mock 对既有断言零影响。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
  Channel: class {
    onmessage: ((message: unknown) => void) | null = null;
  },
}));

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
    archive: null,
    startedAt: '2026-10-01T08:00:00.000Z',
    nodes: [],
    edges: [],
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
    nodes: [],
    edges: [],
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
      root={null}
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
    expect(start).toHaveBeenLastCalledWith(true);
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
  it('默认开：复选框选中；未勾选直接点击发起 → start 携 true 调用恰一次（data-testid 查询）', () => {
    const start = vi.fn(async () => {});
    renderPanel(runStub({ start }));

    const checkbox = screen.getByTestId('run-auto-next-phase');
    expect((checkbox as HTMLInputElement).checked).toBe(true);

    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenLastCalledWith(true);
  });

  it('勾选开关后点击发起 → start 携 false 调用恰一次（开启后 start 携 autoNextPhase=true）', () => {
    const start = vi.fn(async () => {});
    renderPanel(runStub({ start }));

    const checkbox = screen.getByTestId('run-auto-next-phase');
    fireEvent.click(checkbox);
    expect((checkbox as HTMLInputElement).checked).toBe(false);

    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenLastCalledWith(false);
  });

  it('运行期开关 disabled 定格：activeRun 在场即冻结（活面判别非状态值枚举）', () => {
    const rendered = renderPanel(runStub(), { activeRun: activeRun(), lastRun: null });
    const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
    expect(checkbox.disabled).toBe(true);
    expect(checkbox.checked).toBe(true);
    rendered.unmount();
  });

  it('终局恢复可编辑且保留上次取值：activeRun 退场后反选过的开关仍保持（发起以点击时刻值为准）', () => {
    const start = vi.fn(async () => {});
    const first = renderPanel(runStub({ start }));
    fireEvent.click(screen.getByTestId('run-auto-next-phase'));

    first.rerender(
      <RunControlPanel
        changeId={CHANGE_ID}
        name="add-feature"
        root={null}
        activeRun={null}
        lastRun={lastRun({ status: 'failed', reason: 'CLI 漂移' })}
        actions={runStub({ start })}
      />,
    );
    const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
    expect(checkbox.disabled).toBe(false);
    expect(checkbox.checked).toBe(false);

    // 发起以点击时刻开关值为实参：终局后可直接携 true 重发
    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenLastCalledWith(false);
    first.unmount();
  });

  it('发起失败（error 非空、activeRun null）时开关可编辑且取值保留（未进入运行态——可改档后重试发起）', () => {
    const start = vi.fn(async () => {});
    renderPanel(runStub({ error: '同 change 已有并行 run', start }));

    const checkbox = screen.getByTestId<HTMLInputElement>('run-auto-next-phase');
    expect(checkbox.disabled).toBe(false);
    expect(checkbox.checked).toBe(true);

    fireEvent.click(checkbox);
    expect(checkbox.checked).toBe(false);
    fireEvent.click(screen.getByTestId('run-start'));
    expect(start).toHaveBeenLastCalledWith(false);
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
    // 相位间停等点 syncSpecs 不参与（仅归档前停等点消费），恒回传 true
    expect(confirm).toHaveBeenLastCalledWith(true, true);

    fireEvent.click(screen.getByTestId('run-confirm-stop'));
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(confirm).toHaveBeenLastCalledWith(false, true);
  });

  it('activeRun running（phase 非停等态）：无确认卡片', () => {
    renderPanel(runStub(), { activeRun: activeRun(), lastRun: null });
    expect(screen.queryByTestId('run-confirm-card')).toBeNull();
  });
});

describe('RunControlPanel：归档确认卡片（stop-before-archive 停等面，AC-7 / AC-9）', () => {
  /** 归档前置只读载荷 fixture（八字段形；逐用例按需覆盖）。 */
  function preflight(overrides: Partial<ArchivePreflight> = {}): ArchivePreflight {
    return {
      name: 'add-feature',
      completed: false,
      incompletePhases: ['implement'],
      missingArtifacts: ['design.md'],
      deltaSpecs: ['desktop-flow'],
      worktree: '/tmp/wt/add-feature',
      branch: 'change/add-feature',
      mergeTarget: 'main',
      ...overrides,
    };
  }

  it('phase=archive + waitingConfirm → 渲染归档确认卡片，呈现完成度警告 / 未过相位 / 缺席产物 / delta specs / 合入目标', () => {
    renderPanel(runStub(), {
      activeRun: activeRun({
        status: 'waitingConfirm',
        phase: 'archive',
        attempt: 1,
        archive: preflight(),
      }),
      lastRun: null,
    });

    const card = screen.getByTestId('run-archive-confirm-card');
    expect(card).toBeDefined();
    const warnings = screen.getAllByTestId('run-archive-warning').map((row) => row.textContent);
    expect(warnings).toEqual(['工作流未全部通过：implement', '缺少产物文档：design.md']);
    expect(screen.getByTestId('run-archive-delta-specs').textContent).toContain('desktop-flow');
    expect(screen.getByTestId('run-archive-merge-target').textContent).toContain('main');
    // 归档确认卡片与普通确认卡片互斥（同一停等点单面承载）
    expect(screen.queryByTestId('run-confirm-card')).toBeNull();
  });

  it('跳过同步勾选语义：不勾选 → confirm(true, true)（执行同步）；勾选跳过 → confirm(true, false)（syncSpecs=false）；终止 → confirm(false, true)', () => {
    const confirm = vi.fn(async () => {});
    renderPanel(runStub({ confirm }), {
      activeRun: activeRun({ status: 'waitingConfirm', phase: 'archive', archive: preflight() }),
      lastRun: null,
    });

    // 不勾选：syncSpecs = true（执行 delta specs 同步）
    fireEvent.click(screen.getByTestId('run-archive-confirm-proceed'));
    expect(confirm).toHaveBeenLastCalledWith(true, true);
    expect(confirm).toHaveBeenCalledTimes(1);

    // 勾选「跳过同步」：勾选态映射 syncSpecs=false 回传（执行同步的否定面）
    fireEvent.click(screen.getByTestId('run-archive-sync-skip'));
    fireEvent.click(screen.getByTestId('run-archive-confirm-proceed'));
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(confirm).toHaveBeenLastCalledWith(true, false);

    fireEvent.click(screen.getByTestId('run-archive-confirm-stop'));
    expect(confirm).toHaveBeenLastCalledWith(false, true);
  });

  it('普通相位确认回归：phase 非 archive → 既有普通确认卡片（文案与 confirm(proceed, true) 单参语义不变）', () => {
    renderPanel(runStub(), {
      activeRun: activeRun({ status: 'waitingConfirm', phase: 'acceptance', archive: null }),
      lastRun: null,
    });

    expect(screen.getByTestId('run-confirm-card').textContent).toContain('acceptance');
    expect(screen.queryByTestId('run-archive-confirm-card')).toBeNull();
  });

  it('archive 载荷缺省（archive === null）→ 归档卡片降级渲染：警告与 delta specs 行缺席、跳过同步勾选仍可用', () => {
    const confirm = vi.fn(async () => {});
    renderPanel(runStub({ confirm }), {
      activeRun: activeRun({ status: 'waitingConfirm', phase: 'archive', archive: null }),
      lastRun: null,
    });

    const card = screen.getByTestId('run-archive-confirm-card');
    expect(card.textContent).toContain('等待确认进入归档');
    expect(screen.queryAllByTestId('run-archive-warning')).toHaveLength(0);
    expect(screen.queryByTestId('run-archive-delta-specs')).toBeNull();
    expect(screen.queryByTestId('run-archive-merge-target')).toBeNull();
    // 降级不降能：跳过同步勾选仍可用且照常回传
    fireEvent.click(screen.getByTestId('run-archive-sync-skip'));
    fireEvent.click(screen.getByTestId('run-archive-confirm-proceed'));
    expect(confirm).toHaveBeenLastCalledWith(true, false);
  });

  it('legacy 主 root 形态（worktree === null）→ 不渲染合入告知行（无 worktree 即无合入目标面）', () => {
    renderPanel(runStub(), {
      activeRun: activeRun({
        status: 'waitingConfirm',
        phase: 'archive',
        archive: preflight({ worktree: null, branch: null, mergeTarget: null, completed: true }),
      }),
      lastRun: null,
    });

    expect(screen.getByTestId('run-archive-confirm-card')).toBeDefined();
    expect(screen.queryByTestId('run-archive-merge-target')).toBeNull();
  });

  it('confirm 后端拒绝（无运行 run / 无等待方）：卡片不崩溃、保持等待确认、错误态呈现不静默吞错', async () => {
    const confirm = vi.fn(async () => {
      throw new Error('当前无等待中的 phase 确认');
    });
    const actions = runStub({ confirm, error: '当前无等待中的 phase 确认' });
    renderPanel(actions, {
      activeRun: activeRun({ status: 'waitingConfirm', phase: 'archive', archive: preflight() }),
      lastRun: null,
    });

    fireEvent.click(screen.getByTestId('run-archive-confirm-proceed'));
    expect(confirm).toHaveBeenCalledTimes(1);
    // 卡片仍在（未静默切态）；错误原文经错误条呈现（run-error）
    expect(screen.getByTestId('run-archive-confirm-card')).toBeDefined();
    expect(screen.getByTestId('run-error').textContent).toContain('当前无等待中的 phase 确认');
  });

  it('归档确认卡片状态视觉：running 态无归档卡片（非停等点）', () => {
    renderPanel(runStub(), {
      activeRun: activeRun({ status: 'running', phase: 'archive', archive: null }),
      lastRun: null,
    });
    expect(screen.queryByTestId('run-archive-confirm-card')).toBeNull();
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

// ---------------------------------------------------------------------------
// ArchiveMergeNotice 清理告知（worktree-cleanup-and-active-cap / AC-8 / D8）：
// worktree 记录在场才渲染「提交 + 合入 + 收口后自动移除 worktree（分支保留）」
// 文案；legacy 零清理告知；inform + confirm 语义不变（警告不阻断）
// ---------------------------------------------------------------------------

describe('RunControlPanel：归档卡片清理告知（AC-8 / D8）', () => {
  /** 归档前置只读载荷 fixture（八字段形；逐用例按需覆盖）。 */
  function preflight(overrides: Partial<ArchivePreflight> = {}): ArchivePreflight {
    return {
      name: 'add-feature',
      completed: true,
      incompletePhases: [],
      missingArtifacts: [],
      deltaSpecs: [],
      worktree: '/tmp/wt/add-feature',
      branch: 'change/add-feature',
      mergeTarget: 'main',
      ...overrides,
    };
  }

  it('worktree 记录在场：告知文案含「将执行提交 + 合入主仓当前分支：{mergeTarget}」与「收口后自动移除 worktree（分支 {branch} 保留）」', () => {
    renderPanel(runStub(), {
      activeRun: activeRun({
        status: 'waitingConfirm',
        phase: 'archive',
        attempt: 1,
        archive: preflight(),
      }),
      lastRun: null,
    });

    const notice = screen.getByTestId('run-archive-merge-target');
    expect(notice.textContent).toContain('将执行提交 + 合入主仓当前分支：');
    expect(notice.textContent).toContain('main');
    expect(notice.textContent).toContain('收口后自动移除 worktree');
    expect(notice.textContent).toContain('分支');
    expect(notice.textContent).toContain('change/add-feature');
    expect(notice.textContent).toContain('保留');
  });

  it('legacy（worktree === null）：零清理告知行——卡片不含「自动移除」文案与 merge-target 行，其余面照常', () => {
    renderPanel(runStub(), {
      activeRun: activeRun({
        status: 'waitingConfirm',
        phase: 'archive',
        attempt: 1,
        archive: preflight({ worktree: null, branch: null, mergeTarget: null }),
      }),
      lastRun: null,
    });

    const card = screen.getByTestId('run-archive-confirm-card');
    expect(card).toBeDefined();
    expect(screen.queryByTestId('run-archive-merge-target')).toBeNull();
    expect(card.textContent).not.toContain('自动移除');
    // 归档卡片其余面照常：警告不阻断语义不变（proceed / stop 两动作在场）
    expect(screen.getByTestId('run-archive-confirm-proceed') !== null).toBe(true);
    expect(screen.getByTestId('run-archive-confirm-stop') !== null).toBe(true);
  });

  it('卡片头动作清单句含「自动移除 worktree」（worktree 记录在场才缀入），inform + confirm 载荷回归不变', () => {
    const confirm = vi.fn(async () => {});
    renderPanel(runStub({ confirm }), {
      activeRun: activeRun({
        status: 'waitingConfirm',
        phase: 'archive',
        attempt: 1,
        archive: preflight(),
      }),
      lastRun: null,
    });

    const card = screen.getByTestId('run-archive-confirm-card');
    expect(card.textContent).toContain('提交 worktree / 合入主仓 / 同步 delta specs /');
    expect(card.textContent).toContain('自动移除 worktree');
    expect(card.textContent).toContain('双写收口');

    // confirm 载荷回归不变：proceed 携 (true, true)、stop 携 (false, true)
    fireEvent.click(screen.getByTestId('run-archive-confirm-proceed'));
    expect(confirm).toHaveBeenLastCalledWith(true, true);
    fireEvent.click(screen.getByTestId('run-archive-confirm-stop'));
    expect(confirm).toHaveBeenLastCalledWith(false, true);
  });
});

// ---------------------------------------------------------------------------
// archi 章节：归档确认面「🏛 架构变更」区块（AC-15）
// ---------------------------------------------------------------------------

describe('RunControlPanel：架构变更区块（归档确认面）', () => {
  /** 归档前置载荷（八字段形；worktree 供预览画布 root）。 */
  function preflight(overrides: Partial<ArchivePreflight> = {}): ArchivePreflight {
    return {
      name: 'add-feature',
      completed: false,
      incompletePhases: [],
      missingArtifacts: [],
      deltaSpecs: [],
      worktree: '/tmp/wt/add-feature',
      branch: 'change/add-feature',
      mergeTarget: 'main',
      ...overrides,
    };
  }

  /** 架构变更摘要 fixture（元素 +2 / −1、关系 +3 / −1、gate 一条）。 */
  function summary(overrides: Record<string, unknown> = {}) {
    return {
      elementsAdded: ['App.Billing', 'App.Billing.Invoice'],
      elementsRemoved: ['App.Legacy'],
      relationsAdded: 3,
      relationsRemoved: 1,
      gateChanges: [{ fqn: 'App.Billing', from: 'off', to: 'strict' }],
      diagnostics: [],
      ...overrides,
    };
  }

  /** 面板挂载（root 非 null：触发 archi_change_summary 取数）。 */
  function renderArchivePanel(root = 'C:demoalpha') {
    const confirm = vi.fn(async () => {});
    render(
      <RunControlPanel
        changeId={CHANGE_ID}
        name="add-feature"
        root={root}
        activeRun={activeRun({
          status: 'waitingConfirm',
          phase: 'archive',
          archive: preflight(),
        })}
        lastRun={null}
        actions={runStub({ confirm })}
      />,
    );
    return { confirm };
  }

  it('应在有改动时呈现区块（计数行 + FQN 清单 + gate 行）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') return Promise.resolve(summary());
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderArchivePanel();

    await screen.findByTestId('run-archi-block');
    const block = screen.getByTestId('run-archi-block');
    expect(block.textContent).toContain('元素 +2 / −1');
    expect(block.textContent).toContain('关系 +3 / −1');
    expect(block.textContent).toContain('gate 变更 1 项');

    fireEvent.click(screen.getByTestId('run-archi-diff'));
    await screen.findByTestId('run-archi-diff-detail');
    const diff = screen.getByTestId('run-archi-diff-detail');
    expect(diff.textContent).toContain('App.Billing');
    expect(diff.textContent).toContain('App.Legacy');
    expect(diff.textContent).toContain('App.Billing：off → strict');
  });

  it('应在无改动（summary = null）时零占位渲染（无标题 / 无空框 / 无 0 计数行）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') return Promise.resolve(null);
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderArchivePanel();

    await waitFor(() => expect(screen.getByTestId('run-archive-confirm-card')).toBeDefined());
    expect(screen.queryByTestId('run-archi-block')).toBeNull();
    expect(screen.queryByTestId('run-archi-diff')).toBeNull();
    expect(screen.queryByTestId('run-archi-preview')).toBeNull();
  });

  it('应在结构错时逐条红字呈现且确认按钮仍可点击（不阻断）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') {
        return Promise.resolve(
          summary({
            elementsAdded: [],
            elementsRemoved: [],
            relationsAdded: 0,
            relationsRemoved: 0,
            gateChanges: [],
            diagnostics: [
              {
                kind: 'cycle',
                message: '依赖成环：App.A -> App.B -> App.A',
                file: 'openspec/architecture/models/01-core.toml',
                line: 12,
                column: 1,
              },
            ],
          }),
        );
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    const { confirm } = renderArchivePanel();

    await waitFor(() => expect(screen.getAllByTestId('run-archi-error').length).toBeGreaterThan(0));
    const rows = screen.getAllByTestId('run-archi-error');
    expect(rows[0]?.textContent).toContain('cycle');
    expect(rows[0]?.textContent).toContain('openspec/architecture/models/01-core.toml:12');
    // 引导行与诊断行同区块（同一 flex 容器的末行，非 `run-archi-error` 行）
    expect(screen.getByText(/修复后再归档/)).toBeDefined();
    expect(rows).toHaveLength(1);

    fireEvent.click(screen.getByTestId('run-archive-confirm-proceed'));
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(confirm).toHaveBeenLastCalledWith(true, true);
  });

  it('应在预览入口展开画布（mode=incremental、root = preflight.worktree）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') return Promise.resolve(summary());
      if (command === 'archi_graph') {
        return Promise.resolve({ elements: [], relations: [], diagnostics: [] });
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderArchivePanel();

    await screen.findByTestId('run-archi-block');
    fireEvent.click(screen.getByTestId('run-archi-preview'));
    await screen.findByTestId('run-archi-preview-canvas');
    await waitFor(() =>
      expect(
        invokeMock.mock.calls.some(
          ([name, args]) =>
            name === 'archi_graph' && (args as { root?: string }).root === '/tmp/wt/add-feature',
        ),
      ).toBe(true),
    );
  });

  it('应以 (root, changeId) 调一次 archi_change_summary', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') return Promise.resolve(summary());
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderArchivePanel('C:demoalpha');
    await screen.findByTestId('run-archi-block');

    const calls = invokeMock.mock.calls.filter(([name]) => name === 'archi_change_summary');
    expect(calls).toHaveLength(1);
    expect(calls[0]?.[1]).toEqual({ root: 'C:demoalpha', changeId: CHANGE_ID });
  });

  it('应在取数失败时行内呈现错误（不阻断确认）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') {
        return Promise.reject(new Error('change "add-feature" 未建档'));
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderArchivePanel();
    await waitFor(() =>
      expect(screen.getByTestId('run-archi-error').textContent).toContain('架构变更取数失败'),
    );
    expect(screen.getByTestId('run-archive-confirm-proceed')).toBeDefined();
  });

  it('应把区块排在 delta specs 之后、同步跳过之前（DOM 序断言）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') return Promise.resolve(summary());
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    render(
      <RunControlPanel
        changeId={CHANGE_ID}
        name="add-feature"
        root="C:\demo\alpha"
        activeRun={activeRun({
          status: 'waitingConfirm',
          phase: 'archive',
          archive: preflight({ deltaSpecs: ['desktop-flow'] }),
        })}
        lastRun={null}
        actions={runStub()}
      />,
    );
    await screen.findByTestId('run-archi-block');

    const card = screen.getByTestId('run-archive-confirm-card');
    const order = Array.from(
      card.querySelectorAll('[data-testid^="run-archive-"], [data-testid="run-archi-block"]'),
    ).map((node) => node.getAttribute('data-testid') ?? '');
    expect(order.indexOf('run-archive-delta-specs')).toBeLessThan(order.indexOf('run-archi-block'));
    expect(order.indexOf('run-archi-block')).toBeLessThan(order.indexOf('run-archive-sync-skip'));
  });

  it('应在区块内不出现本期射程外信息（跨语言依赖 / 符号级契约 / 相邻报告）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_change_summary') return Promise.resolve(summary());
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    renderArchivePanel();
    await screen.findByTestId('run-archi-block');
    const block = screen.getByTestId('run-archi-block').textContent ?? '';
    for (const forbidden of ['跨语言依赖', '符号级契约', '相邻报告']) {
      expect(block).not.toContain(forbidden);
    }
  });
});
// ---------------------------------------------------------------------------
// 归档前置文案面与架构变更区块边界面（AC-7 / AC-9 / AC-15 文案与取数纪律）
// ---------------------------------------------------------------------------

/** 归档前置载荷（八字段形）。 */
function archivePreflight(overrides: Partial<ArchivePreflight> = {}): ArchivePreflight {
  return {
    name: 'add-feature',
    completed: false,
    incompletePhases: [],
    missingArtifacts: [],
    deltaSpecs: [],
    worktree: '/tmp/wt/add-feature',
    branch: 'change/add-feature',
    mergeTarget: 'main',
    ...overrides,
  };
}

/** 架构变更摘要 fixture（元素 +2 / −1、关系 +3 / −1、gate 一条）。 */
function archiSummary(overrides: Record<string, unknown> = {}) {
  return {
    elementsAdded: ['App.Billing', 'App.Billing.Invoice'],
    elementsRemoved: ['App.Legacy'],
    relationsAdded: 3,
    relationsRemoved: 1,
    gateChanges: [{ fqn: 'App.Billing', from: 'off', to: 'strict' }],
    diagnostics: [],
    ...overrides,
  };
}

/** 归档停等卡片面板（root 由调用方给定；actions 为 spy 桩）。 */
function renderArchiveCard(root: string | null, archive: ArchivePreflight | null) {
  return render(
    <RunControlPanel
      changeId={CHANGE_ID}
      name="add-feature"
      root={root}
      activeRun={activeRun({ status: 'waitingConfirm', phase: 'archive', archive })}
      lastRun={null}
      actions={runStub()}
    />,
  );
}

describe('RunControlPanel：归档前置警告与告知行文案', () => {
  it('应在完成且无缺席产物时零警告行', () => {
    renderArchiveCard(null, archivePreflight({ completed: true }));
    expect(screen.queryAllByTestId('run-archive-warning')).toHaveLength(0);
  });

  it('应在未完成且无缺失相位时只给「完成度不可核算」一条警告', () => {
    renderArchiveCard(null, archivePreflight({ completed: false, incompletePhases: [] }));
    expect(screen.getAllByTestId('run-archive-warning').map((row) => row.textContent)).toEqual([
      '完成度不可核算（工作流类型无相位表）',
    ]);
  });

  it('应逐条呈现未通过的相位与缺席产物（顿号连缀、全量不截断）', () => {
    renderArchiveCard(
      null,
      archivePreflight({
        completed: false,
        incompletePhases: ['design', 'implement'],
        missingArtifacts: ['proposal.md', 'tasks.md'],
      }),
    );
    expect(screen.getAllByTestId('run-archive-warning').map((row) => row.textContent)).toEqual([
      '工作流未全部通过：design、implement',
      '缺少产物文档：proposal.md、tasks.md',
    ]);
  });

  it('应在合入目标 / 分支探测失败时呈现占位文案（提示仍可读）', () => {
    renderArchiveCard(null, archivePreflight({ mergeTarget: null, branch: null }));
    const notice = screen.getByTestId('run-archive-merge-target');
    expect(notice.textContent).toContain('（当前分支探测失败）');
    expect(notice.textContent).toContain('change/<name>');
    expect(notice.textContent).toContain('收口后自动移除 worktree');
    expect(screen.getByTestId('run-archive-confirm-card').textContent).toContain(
      '自动移除 worktree /',
    );
  });

  it('应在 worktree 缺席时不提示自动移除 worktree（告知行整体退位）', () => {
    renderArchiveCard(null, archivePreflight({ worktree: null, branch: null, mergeTarget: null }));
    expect(screen.queryByTestId('run-archive-merge-target')).toBeNull();
    expect(screen.getByTestId('run-archive-confirm-card').textContent).not.toContain(
      '自动移除 worktree',
    );
  });

  it('应在 waitingConfirm 但 phase 缺席时不渲染任何停等卡片', () => {
    render(
      <RunControlPanel
        changeId={CHANGE_ID}
        name="add-feature"
        root={null}
        activeRun={activeRun({ status: 'waitingConfirm', phase: null })}
        lastRun={null}
        actions={runStub()}
      />,
    );
    expect(screen.queryByTestId('run-confirm-card')).toBeNull();
    expect(screen.queryByTestId('run-archive-confirm-card')).toBeNull();
  });

  it('应在 runs 尾行非 failed 时走默认徽章变体，且 reason 缺席时零收口行', () => {
    renderPanel(runStub(), {
      activeRun: null,
      lastRun: lastRun({ status: 'completed', reason: null }),
    });
    expect(screen.getByTestId('run-status').className).toContain('bg-primary');
    expect(screen.getByTestId('run-status').textContent).toBe('已完成');
    expect(screen.queryByTestId('run-finished-reason')).toBeNull();
  });

  it('应在 runs 尾行 failed 时走 fail 变体并呈现收口记因', () => {
    renderPanel(runStub(), {
      activeRun: null,
      lastRun: lastRun({ status: 'failed', reason: 'CLI 漂移' }),
    });
    expect(screen.getByTestId('run-status').className).toContain('bg-destructive/10');
    expect(screen.getByTestId('run-finished-reason').textContent).toContain('CLI 漂移');
  });
});

describe('RunControlPanel：架构变更区块边界面', () => {
  it('应在 root 缺席时零 archi_change_summary invoke（不以空根取数）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(() => Promise.resolve(archiSummary()));
    renderArchiveCard(null, archivePreflight());
    await waitFor(() => expect(screen.getByTestId('run-archive-confirm-card')).toBeTruthy());
    await act(async () => {
      await Promise.resolve();
    });
    expect(invokeMock.mock.calls.filter(([name]) => name === 'archi_change_summary')).toHaveLength(
      0,
    );
    expect(screen.queryByTestId('run-archi-block')).toBeNull();
  });

  it('应在 diff 明细里逐项列出增删元素，空组零行', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(() => Promise.resolve(archiSummary()));
    const filled = renderArchiveCard('C:demoalpha', archivePreflight());
    await screen.findByTestId('run-archi-block');

    fireEvent.click(screen.getByTestId('run-archi-diff'));
    await screen.findByTestId('run-archi-diff-detail');
    const diff = screen.getByTestId('run-archi-diff-detail');
    expect(diff.textContent).toContain('新增元素：App.Billing、App.Billing.Invoice');
    expect(diff.textContent).toContain('删除元素：App.Legacy');
    expect(diff.textContent).toContain('App.Billing：off → strict');

    // 空组：零「新增元素」/「删除元素」行（不落空清单标题）
    filled.unmount();
    invokeMock.mockImplementation(() =>
      Promise.resolve(
        archiSummary({
          elementsAdded: [],
          elementsRemoved: [],
          gateChanges: [],
          relationsAdded: 0,
          relationsRemoved: 0,
        }),
      ),
    );
    renderArchiveCard('C:demoalpha', archivePreflight());
    await screen.findByTestId('run-archi-block');
    fireEvent.click(screen.getByTestId('run-archi-diff'));
    await screen.findByTestId('run-archi-diff-detail');
    const empty = screen.getByTestId('run-archi-diff-detail');
    expect(empty.textContent).not.toContain('新增元素');
    expect(empty.textContent).not.toContain('删除元素');
    expect(empty.textContent?.trim()).toBe('');
  });

  it('应把「修复后再归档」引导行限定在诊断在场时（零诊断零引导）', async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(() => Promise.resolve(archiSummary({ diagnostics: [] })));
    renderArchiveCard('C:demoalpha', archivePreflight());
    const block = await screen.findByTestId('run-archi-block');
    expect(block.textContent).not.toContain('修复后再归档');
    expect(screen.queryAllByTestId('run-archi-error')).toHaveLength(0);
  });

  it('应在 root 变化后以新根重新取数，且旧根应答不回写', async () => {
    const pending: ((value: unknown) => void)[] = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          pending.push(resolve);
        }),
    );
    const panel = (root: string) => (
      <RunControlPanel
        changeId={CHANGE_ID}
        name="add-feature"
        root={root}
        activeRun={activeRun({
          status: 'waitingConfirm',
          phase: 'archive',
          archive: archivePreflight(),
        })}
        lastRun={null}
        actions={runStub()}
      />
    );
    const { rerender } = render(panel('C:demoa'));

    rerender(panel('C:demob'));
    await waitFor(() =>
      expect(
        invokeMock.mock.calls.filter(([name]) => name === 'archi_change_summary'),
      ).toHaveLength(2),
    );
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'archi_change_summary')[1]?.[1],
    ).toEqual({ root: 'C:demob', changeId: CHANGE_ID });

    await act(async () => {
      pending[1]?.(archiSummary({ elementsAdded: ['App.X'], elementsRemoved: [] }));
    });
    const block = await screen.findByTestId('run-archi-block');
    expect(block.textContent).toContain('元素 +1 / −0');

    // 旧根迟到应答：不回写成陈旧计数
    await act(async () => {
      pending[0]?.(
        archiSummary({
          elementsAdded: ['App.A', 'App.B', 'App.C'],
          elementsRemoved: [],
        }),
      );
    });
    expect(screen.getByTestId('run-archi-block').textContent).toContain('元素 +1 / −0');
  });
});
