import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeDetail, ChangeList } from './types/dto';

const { invokeMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

import App from './App';

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

const fakeDetail: ChangeDetail = {
  name: 'add-feature',
  source: 'active',
  inventory: 'v2',
  created: null,
  unparsable: false,
  pipeline: [
    {
      phase: 'proposal',
      attempts: [
        {
          attempt: 1,
          verdict: 'pass',
          report: 'OK',
          checklist: [],
          skipped: false,
          stale: false,
          startAt: null,
          timestamp: null,
          backtrackTo: null,
          backtrackReason: null,
        },
      ],
    },
  ],
  activePhase: null,
  interrupted: [],
  fileLog: [],
  artifacts: [],
};

function mockIpc() {
  invokeMock.mockImplementation((command: string, params: { kind?: string; source?: string }) => {
    if (command === 'list_changes') {
      return Promise.resolve(fakeList);
    }
    if (command === 'get_change_detail') {
      return Promise.resolve(fakeDetail);
    }
    if (command === 'read_artifact') {
      return Promise.resolve({
        kind: params?.kind,
        version: 1,
        title: params?.source,
        payload: null,
        fallbackText: '产物保底文本',
      });
    }
    return Promise.resolve(null);
  });
}

async function selectWorkspace(path = '/repo') {
  openMock.mockResolvedValue(path);
  fireEvent.click(screen.getByText('选择 workspace 文件夹'));
  await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: path }));
}

describe('App：应用壳的 workspace 选择、视图切换与刷新下发', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    mockIpc();
  });

  it('未选 workspace 时显示选择入口；选定后以该 root 进入列表视图', async () => {
    render(<App />);
    expect(screen.getByText('选择 workspace 文件夹') !== null).toBe(true);
    expect(invokeMock).not.toHaveBeenCalled();

    openMock.mockResolvedValue('/repo');
    fireEvent.click(screen.getByText('选择 workspace 文件夹'));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: '/repo' }));
    // 列表视图渲染（fixture 中的 change 名出现）
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
  });

  it('dialog 取消（返回 null）时保持未选 workspace 状态，不发起取数', async () => {
    render(<App />);
    openMock.mockResolvedValue(null);
    fireEvent.click(screen.getByText('选择 workspace 文件夹'));

    await waitFor(() => expect(openMock).toHaveBeenCalled());
    // 对话框 promise 已 settle 之后仍无取数
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByText('选择 workspace 文件夹') !== null).toBe(true);
  });

  it('dialog 调用 reject 时界面保持可选状态不白屏', async () => {
    render(<App />);
    openMock.mockRejectedValue(new Error('对话框崩溃'));
    fireEvent.click(screen.getByText('选择 workspace 文件夹'));

    await waitFor(() => expect(openMock).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByText('选择 workspace 文件夹') !== null).toBe(true);
  });

  it('列表项点击后切换到详情视图，root 与 change 名下发正确', async () => {
    render(<App />);
    await selectWorkspace('/repo');

    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: '/repo',
        change: 'add-feature',
      }),
    );
    // 详情视图渲染：标题为 change 名
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );
  });

  it('刷新动作触发 useChangeList 的 refresh 回调（再次下发 list_changes）', async () => {
    render(<App />);
    await selectWorkspace('/repo');
    const callsBefore = invokeMock.mock.calls.filter(([name]) => name === 'list_changes').length;

    fireEvent.click(screen.getByText('刷新列表'));
    await waitFor(() => {
      const callsAfter = invokeMock.mock.calls.filter(([name]) => name === 'list_changes').length;
      expect(callsAfter).toBe(callsBefore + 1);
    });
  });
});
