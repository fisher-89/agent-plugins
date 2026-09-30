import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentInstanceRecord, AgentProviderRecord } from '../../types/generated/bindings';
import { AgentsView } from './agents-view';

// ---------------------------------------------------------------------------
// IPC 进程边界 mock：invoke 按命令名分发替身（list_agent_providers /
// list_agent_instances 固定 fixture；数组形态防 null 崩溃），记录调用序列断言
// 命令面。AgentsView 为无 props 组合根：两取数 hook 与两 panel 真实组合
// （hook 内部协作者不 mock，经 invoke 面驱动）；sonner 以 vi.fn 替身注入
// toast.error（hook 动作轨依赖，页面级用例不触发）。页面为全局语义，不依赖
// root、不触发任何 workspace 命令。
// ---------------------------------------------------------------------------

const { invokeMock, toastErrorMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('sonner', () => ({
  toast: { error: toastErrorMock },
}));

let providersFixture: AgentProviderRecord[];
let instancesFixture: AgentInstanceRecord[];

function mockIpc() {
  invokeMock.mockReset();
  toastErrorMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'list_agent_providers') return Promise.resolve(providersFixture);
    if (command === 'list_agent_instances') return Promise.resolve(instancesFixture);
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  providersFixture = [];
  instancesFixture = [];
  mockIpc();
});

describe('AgentsView 两栏骨架（AC-1）', () => {
  it('渲染 Providers / Agents 两栏；两 hook 挂载取数回填后清单呈现记录（真实组合）', async () => {
    providersFixture = [
      {
        id: 1,
        name: '端点甲',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
      },
    ];
    instancesFixture = [{ id: 2, name: '实例乙', engine: 'sdk', providerId: 1, isDefault: true }];

    render(<AgentsView />);

    // 两栏 panel 真实组合呈现
    expect(screen.getByTestId('agents-view') !== null).toBe(true);
    expect(screen.getByTestId('provider-panel') !== null).toBe(true);
    expect(screen.getByTestId('agent-panel') !== null).toBe(true);
    expect(screen.getByText('Providers') !== null).toBe(true);
    expect(screen.getByText('Agents') !== null).toBe(true);

    // 取数回填后清单呈现记录（panel 真实消费两 hook 状态）
    await waitFor(() => expect(screen.getAllByTestId('provider-item')).toHaveLength(1));
    await waitFor(() => expect(screen.getAllByTestId('agent-item')).toHaveLength(1));
    expect(screen.getByTestId('provider-panel').textContent).toContain('端点甲');
    expect(screen.getByTestId('agent-panel').textContent).toContain('实例乙');
  });

  it('两清单均空 → 两栏空态渲染不崩', async () => {
    render(<AgentsView />);

    expect(screen.getByTestId('provider-panel') !== null).toBe(true);
    expect(screen.getByTestId('agent-panel') !== null).toBe(true);
    // 取数链收敛后空态呈现（两清单均空的初始形态）
    await waitFor(() => expect(screen.getByTestId('provider-empty') !== null).toBe(true));
    expect(screen.getByTestId('agent-empty').textContent).toContain('暂无 agent');
  });

  it('页面不触发任何 workspace 命令：invoke 面仅 list_agent_providers / list_agent_instances 各恰一次', async () => {
    render(<AgentsView />);
    await waitFor(() => expect(screen.getByTestId('provider-empty') !== null).toBe(true));

    const commandNames = invokeMock.mock.calls.map(([name]) => name);
    expect([...commandNames].sort((a, b) => a.localeCompare(b))).toEqual([
      'list_agent_instances',
      'list_agent_providers',
    ]);
    // 全局语义：无 workspace 域命令（list_workspaces / agent_runs 等零触发）
    expect(commandNames).not.toContain('list_workspaces');
    expect(commandNames).not.toContain('agent_runs');
  });
});
