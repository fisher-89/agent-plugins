import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiElement } from '../../types/generated/bindings';
import { ArchitectureView } from './architecture-view';

/**
 * `ArchitectureView` 单测（AC-18，挂载点 A）：页头 + 显式刷新 + 画布以 `mode =
 * "full"` 装配、全量只读（无改档入口）、`root = null` 不发 invoke、显式刷新重发
 * 恰一次、假计时器推进零取数（零 watch 零轮询）。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`；xyflow 度量垫片照抄既有先例。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

class ResizeObserverStub {
  constructor(_callback: ResizeObserverCallback) {}

  observe(): void {}

  unobserve(): void {}

  disconnect(): void {}
}

const nativeOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
const nativeOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');

function element(fqn: string): ArchiElement {
  return {
    fqn,
    kind: 'module',
    gate: 'off',
    path: [],
    summary: null,
    metadata: {},
    modelFile: 'openspec/architecture/models/01-core.toml',
    modelLine: 2,
    mark: 'none',
  };
}

function callsOf(command: string): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_graph') {
      return Promise.resolve({
        elements: [element('App'), element('App.Auth')],
        relations: [{ from: 'App.Auth', to: 'App', description: null, mark: 'none', reason: null }],
        diagnostics: [],
      });
    }
    if (command === 'archi_module') {
      const fqn = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve({
        element: element(fqn),
        dependsOn: [],
        dependedBy: [],
        children: [],
        docs: [],
        warnings: [],
      });
    }
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });

  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
    configurable: true,
    get: () => 100,
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
    configurable: true,
    get: () => 100,
  });
  vi.stubGlobal(
    'DOMMatrixReadOnly',
    class {
      m22 = 1;
    },
  );
  const svgPrototype = globalThis.SVGElement?.prototype as unknown as
    | Record<string, unknown>
    | undefined;
  if (svgPrototype !== undefined && typeof svgPrototype.getBBox !== 'function') {
    svgPrototype.getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 });
  }
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  if (nativeOffsetWidth !== undefined) {
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', nativeOffsetWidth);
  }
  if (nativeOffsetHeight !== undefined) {
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', nativeOffsetHeight);
  }
});

describe('挂载点 A', () => {
  it('应渲染页头 / 刷新按钮 / 画布并装配 full 只读模式', async () => {
    render(<ArchitectureView root={'C:\\demo\\alpha'} />);
    expect(screen.getByText('架构')).toBeTruthy();
    expect(screen.getByTestId('archi-refresh')).toBeTruthy();
    await waitFor(() => expect(screen.getByTestId('archi-canvas')).toBeTruthy());
    // full 模式：抽屉内无 gate 可提交控件
    await waitFor(() => expect(screen.getByTestId('archi-gate-readonly')).toBeTruthy());
    expect(screen.queryByTestId('archi-gate-select')).toBeNull();
    expect(screen.queryByTestId('archi-gate-submit')).toBeNull();
  });

  it('应在点击刷新时以同一 root 重发恰一次 archi_graph', async () => {
    render(<ArchitectureView root={'C:\\demo\\alpha'} />);
    await waitFor(() => expect(callsOf('archi_graph')).toHaveLength(1));

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-refresh'));
    });
    await waitFor(() => expect(callsOf('archi_graph')).toHaveLength(2));
    expect(callsOf('archi_graph')[1]).toEqual({ root: 'C:\\demo\\alpha' });
  });

  it('应在 root 为 null 时不发 invoke 并呈现空态', async () => {
    render(<ArchitectureView root={null} />);
    await waitFor(() => expect(screen.getByTestId('archi-canvas-empty')).toBeTruthy());
    expect(callsOf('archi_graph')).toHaveLength(0);
    expect(callsOf('archi_set_gate')).toHaveLength(0);
  });

  it('应无改档调用点（全页只读的行为证据）', async () => {
    render(<ArchitectureView root={'C:\\demo\\alpha'} />);
    await waitFor(() => expect(screen.getByTestId('archi-canvas')).toBeTruthy());
    expect(callsOf('archi_set_gate')).toHaveLength(0);
    expect(screen.getByTestId('archi-gate-readonly').textContent).toContain(
      '改档需要开一个 change',
    );
  });

  it('应在假计时器推进下零取数（三触发之外无动作）', async () => {
    vi.useFakeTimers();
    try {
      render(<ArchitectureView root={'C:\\demo\\alpha'} />);
      await act(async () => {
        await Promise.resolve();
      });
      const before = callsOf('archi_graph').length;
      await act(async () => {
        vi.advanceTimersByTime(120_000);
      });
      expect(callsOf('archi_graph')).toHaveLength(before);
    } finally {
      vi.useRealTimers();
    }
  });
});
