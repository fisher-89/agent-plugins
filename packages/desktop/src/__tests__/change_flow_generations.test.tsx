// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { AttemptRecord, ChangeDetail } from '../types/dto';

// ---------------------------------------------------------------------------
// 集成：ChangeDetailView 页面组装 → 三代际降级分支（AC-6）。判定依据分散在
// DTO 三处（inventory 值、pipeline 空数组、fileLog null），落地横跨「挂不挂图、
// 显不显 workflow 面板、抽屉文件表节形态」的组件联动。
//
// 进程边界 Mock：invoke 按命令名分发并记录调用（沿 ipc_pipeline.test.tsx 既有
// 模式）；运行环境 ResizeObserver / SVGGraphicsElement.getBBox 为 jsdom 垫片。
// 链路内组件 / hooks / 转换层全部真实实现。
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

beforeEach(() => {
  window.location.hash = '';
  getVersionMock.mockReset();
  invokeMock.mockReset();
  openMock.mockReset();
  checkMock.mockReset();
  checkMock.mockResolvedValue(null);
  getVersionMock.mockResolvedValue('0.3.3');
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
      return Promise.resolve(currentDetail());
    }
    if (command === 'read_artifact') {
      return Promise.resolve({
        kind: 'markdown-doc',
        version: 1,
        title: '提案',
        payload: { markdown: '# 提案正文' },
        fallbackText: null,
      });
    }
    return Promise.resolve(null);
  });
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
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
});

// ---------------------------------------------------------------------------
// 同一 fixture 骨架分别裁出三态（v0 / v1 / v2 / unparsable 混合形态）
// ---------------------------------------------------------------------------

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

const PIPELINE: ChangeDetail['pipeline'] = [
  {
    phase: 'proposal',
    attempts: [attempt({ report: '提案通过', startAt: '2026-09-01T10:00:00Z' })],
  },
  {
    phase: 'dev-design',
    attempts: [attempt({ verdict: 'fail', report: '设计未过', startAt: '2026-09-02T10:00:00Z' })],
  },
  ...[
    'test-design',
    'implement',
    'test-gen',
    'test-execution',
    'code-review',
    'acceptance',
    'code-analyze',
  ].map((phase) => ({ phase, attempts: [] as AttemptRecord[] })),
];

const UNPARSABLE_PIPELINE: ChangeDetail['pipeline'] = PIPELINE.map((station) => ({
  phase: station.phase,
  attempts: [],
}));

const ARTIFACTS: ChangeDetail['artifacts'] = [
  { kind: 'markdown-doc', source: 'proposal.md', title: '提案' },
];

function baseDetail(): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
    pipeline: PIPELINE,
    activePhase: null,
    interrupted: [],
    fileLog: [{ op: 'write', scope: 'workflow', attempt: null, path: 'workflow.json', at: null }],
    artifacts: ARTIFACTS,
  };
}

let generation: 'v0' | 'v1' | 'v2' | 'unparsable' | 'v0-no-filelog' = 'v2';

function currentDetail(): ChangeDetail {
  switch (generation) {
    case 'v0':
      return { ...baseDetail(), inventory: 'v0', pipeline: [], fileLog: null };
    case 'v1':
      return { ...baseDetail(), inventory: 'v1', fileLog: null };
    case 'unparsable':
      return { ...baseDetail(), unparsable: true, pipeline: UNPARSABLE_PIPELINE };
    case 'v0-no-filelog':
      return { ...baseDetail(), inventory: 'v0', pipeline: [], fileLog: null };
    default:
      return baseDetail();
  }
}

/** 启动恢复 → 清单 → 进入详情，等待详情页稳定呈现。 */
async function onDetail() {
  render(<App />);
  await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
  fireEvent.click(screen.getByText('add-feature'));
}

describe('change_flow_generations：三代际降级与 unparsable 退化', () => {
  it('v2 完整呈现：图区 + workflow 面板 + 产物区并存', async () => {
    generation = 'v2';
    await onDetail();
    await waitFor(() => expect(screen.getByTestId('flow-graph') !== null).toBe(true));
    expect(screen.getAllByTestId('flow-column')).toHaveLength(9);
    expect(screen.getAllByTestId('flow-node')).toHaveLength(2);
    expect(screen.getByTestId('workflow-panel') !== null).toBe(true);
    expect(screen.getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('v1 图正常绘制、无 workflow 面板、抽屉文件表节呈 v1 降级文案', async () => {
    generation = 'v1';
    await onDetail();
    await waitFor(() => expect(screen.getByTestId('flow-graph') !== null).toBe(true));
    expect(screen.getAllByTestId('flow-column')).toHaveLength(9);
    expect(screen.queryByTestId('workflow-panel')).toBeNull();
    // v1 判定信号穿透页面到抽屉：文件表节呈降级文案而非空表格
    fireEvent.click(screen.getByTestId('rf__node-eval:proposal:1'));
    expect(screen.getByTestId('drawer-files-section').textContent).toContain(
      '（无 file_log 数据：v1 及更早代际无此字段）',
    );
  });

  it('v0 空图占位（flow-empty）且产物区不受图区缺位影响照常渲染', async () => {
    generation = 'v0';
    await onDetail();
    await waitFor(() => expect(screen.getByTestId('flow-empty') !== null).toBe(true));
    expect(screen.getByTestId('flow-empty').textContent).toContain(
      'v0 早期代际：无 workflow.json，仅文档形态',
    );
    expect(screen.queryByTestId('flow-graph')).toBeNull();
    await waitFor(() => expect(screen.getAllByTestId('artifact-card')).toHaveLength(1));
    expect(screen.queryByTestId('workflow-panel')).toBeNull();
  });

  it('unparsable 详情 → 9 空列流程图正常呈现（列容器 9、事件节点 0）不白屏', async () => {
    generation = 'unparsable';
    await onDetail();
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    expect(screen.queryAllByTestId('flow-node')).toHaveLength(0);
    expect(screen.getByTestId('warn-note').textContent).toContain('workflow.json 无法解析');
    // 产物区照常，页面未白屏
    expect(screen.getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('v0 与 fileLog null 叠加（更早代际混合形态）→ 空图占位 + 无面板双降级互不冲突', async () => {
    generation = 'v0-no-filelog';
    await onDetail();
    await waitFor(() => expect(screen.getByTestId('flow-empty') !== null).toBe(true));
    expect(screen.getByTestId('flow-empty').textContent).toContain(
      'v0 早期代际：无 workflow.json，仅文档形态',
    );
    expect(screen.queryByTestId('flow-graph')).toBeNull();
    expect(screen.queryByTestId('workflow-panel')).toBeNull();
    // 产物区照常兜底
    expect(screen.getAllByTestId('artifact-card')).toHaveLength(1);
  });
});
