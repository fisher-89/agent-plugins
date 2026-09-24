// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { AttemptRecord, ChangeDetail } from '../types/dto';

// ---------------------------------------------------------------------------
// 集成：useChangeDetail 显式 refresh → ChangeDetailView 图区重算（AC-7 不变式）。
// 详情更新仅由显式 refresh 触发：挂载恰一次 get_change_detail、静置不产生新调用
// （排除轮询 / watch / 事件订阅回归）、刷新恰 +1 且新 DTO 穿透转换层到图区、
// 抽屉与列头 / 节点点击等交互全程零 invoke。
//
// 进程边界 Mock：invoke 按命令名分发并记录调用 + 可控 DTO（沿 ipc_pipeline /
// explore_watch_refresh 既有模式）；定时器以 vi.useFakeTimers 推进静置期；
// ResizeObserver / getBBox 为 jsdom 环境垫片。
// ---------------------------------------------------------------------------

const { checkMock, getVersionMock, invokeMock, openMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  getVersionMock: vi.fn(),
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: checkMock }));

class ResizeObserverStub {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}

function attempt(overrides: Partial<AttemptRecord> = {}): AttemptRecord {
  return {
    attempt: 1,
    verdict: 'pass',
    report: '评估记录',
    checklist: [],
    skipped: false,
    stale: false,
    startAt: null,
    timestamp: null,
    backtrackTo: null,
    backtrackReason: null,
    ...overrides,
  };
}

function stationsOf(attemptsByPhase: Record<string, AttemptRecord[]>): ChangeDetail['pipeline'] {
  return [
    'proposal',
    'dev-design',
    'test-design',
    'implement',
    'test-gen',
    'test-execution',
    'code-review',
    'acceptance',
    'code-analyze',
  ].map((phase) => ({ phase, attempts: attemptsByPhase[phase] ?? [] }));
}

/** 刷新前 DTO：仅 proposal 一条 eval。 */
function detailBefore(): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
    pipeline: stationsOf({
      proposal: [attempt({ report: '提案通过', startAt: '2026-09-01T10:00:00Z' })],
    }),
    activePhase: null,
    interrupted: [],
    fileLog: [],
    artifacts: [],
  };
}

/** 刷新后 DTO：test-design 站新增 eval（节点集合变化可观测）。 */
function detailAfter(): ChangeDetail {
  return {
    ...detailBefore(),
    pipeline: stationsOf({
      proposal: [attempt({ report: '提案通过', startAt: '2026-09-01T10:00:00Z' })],
      'test-design': [attempt({ report: '测试设计通过', startAt: '2026-09-02T10:00:00Z' })],
    }),
  };
}

let detailCallCount = 0;

function mockIpc() {
  invokeMock.mockImplementation((command: string) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([{ root: '/repo', name: 'repo', addedAt: 1 }]);
    }
    if (command === 'list_changes') {
      return Promise.resolve({
        active: [
          {
            name: 'add-feature',
            source: 'active',
            inventory: 'v2',
            created: null,
            unparsable: false,
          },
        ],
        archiveGroups: [],
      });
    }
    if (command === 'get_change_detail') {
      detailCallCount += 1;
      return Promise.resolve(detailCallCount === 1 ? detailBefore() : detailAfter());
    }
    return Promise.resolve(null);
  });
}

function detailCalls(): number {
  return invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail').length;
}

/** 冲刷微任务（fake timers 下 waitFor 不可用的替代）。 */
async function flush() {
  await act(async () => {});
  await act(async () => {});
}

/** 轮询冲刷直到条件成立（fake timers 下以微任务轮次代替 waitFor 定时器）。 */
async function settleUntil(check: () => boolean, maxRounds = 100) {
  for (let round = 0; round < maxRounds; round += 1) {
    if (check()) return;
    await act(async () => {});
  }
  if (!check()) throw new Error('settleUntil 条件未达成');
}

/** 启动恢复 → 进入 change 详情，等待图区挂载稳定。 */
async function onDetail() {
  render(<App />);
  await settleUntil(() => screen.queryByText('add-feature') !== null);
  fireEvent.click(screen.getByText('add-feature'));
  await settleUntil(() => screen.queryByTestId('flow-graph') !== null);
  await flush();
}

beforeEach(() => {
  window.location.hash = '';
  getVersionMock.mockReset();
  invokeMock.mockReset();
  openMock.mockReset();
  checkMock.mockReset();
  checkMock.mockResolvedValue(null);
  getVersionMock.mockResolvedValue('0.3.3');
  toast.dismiss();
  detailCallCount = 0;
  mockIpc();
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  const svgPrototype = globalThis.SVGElement?.prototype as unknown as
    | Record<string, unknown>
    | undefined;
  if (svgPrototype !== undefined && typeof svgPrototype.getBBox !== 'function') {
    svgPrototype.getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 });
  }
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('change_flow_refresh：调用计数不变式与图区穿透（AC-7）', () => {
  it('挂载恰一次 get_change_detail；静置多个周期计数不增长；点击刷新恰 +1 且图区随新 DTO 重算', async () => {
    await onDetail();
    expect(detailCalls()).toBe(1);
    expect(screen.queryByTestId('rf__node-eval:proposal:1') !== null).toBe(true);
    expect(screen.queryByTestId('rf__node-eval:test-design:1')).toBeNull();

    // 静置推进 10 分钟（远超任何轮询周期）：无 watch / 轮询 / 事件订阅回归
    await act(async () => {
      vi.advanceTimersByTime(10 * 60 * 1000);
    });
    await flush();
    expect(detailCalls()).toBe(1);

    // 显式 refresh：恰 +1 次，且新 DTO 穿透转换层 → 图区节点集合变化
    fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    await settleUntil(() => screen.queryByTestId('rf__node-eval:test-design:1') !== null);
    expect(detailCalls()).toBe(2);
    // 旧集合中不存在、新集合新增的节点如实上墙
    expect(screen.queryByTestId('rf__node-eval:proposal:1') !== null).toBe(true);
  });

  it('抽屉打开 / 关闭、列头与节点点击等交互不触发任何 invoke', async () => {
    await onDetail();
    const before = invokeMock.mock.calls.length;
    expect(before).toBeGreaterThan(0);

    // 节点点击 → 抽屉打开 → 列头点击换选 → 关闭：全程零新调用
    fireEvent.click(screen.getByTestId('rf__node-eval:proposal:1'));
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    const columnWrapper = screen.getByTestId('rf__node-col:dev-design');
    fireEvent.click(within(columnWrapper).getByTestId('flow-column'));
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.queryByTestId('detail-drawer')).toBeNull();
    await flush();

    expect(invokeMock.mock.calls.length).toBe(before);
    expect(detailCalls()).toBe(1);
  });

  it('抽屉开启状态下静置推进虚拟计时，invoke 调用计数不增长（无抽屉驱动的隐式取数）', async () => {
    await onDetail();
    fireEvent.click(screen.getByTestId('rf__node-eval:proposal:1'));
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    const before = invokeMock.mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(10 * 60 * 1000);
    });
    await flush();
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    expect(invokeMock.mock.calls.length).toBe(before);
  });
});
