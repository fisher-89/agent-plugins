import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  AgentInstanceRecord,
  AgentProviderRecord,
  ModelCatalog,
} from '../../types/generated/bindings';
import { AgentsView } from './agents-view';

// ---------------------------------------------------------------------------
// IPC 进程边界 mock：invoke 按命令名分发替身（list_agent_providers /
// list_agent_instances / list_model_catalog 固定 fixture；数组形态防 null
// 崩溃），记录调用序列断言命令面。AgentsView 为无 props 组合根：三取数 hook
// 与两 panel 真实组合（hook 内部协作者不 mock，经 invoke 面驱动）；sonner 以
// vi.fn 替身注入 toast.error（hook 动作轨依赖，页面级用例不触发）。页面为全局
// 语义，不依赖 root、不触发任何 workspace 命令。
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

/** 目录取数三态：成功 / 因进程边界失败降级 / 仍在途（loading 不阻断表单） */
type CatalogMode = 'ok' | 'reject' | 'pending';

let catalogMode: CatalogMode;

/** 目录 fixture（协议段 + 协议级 effort 建议 + 条目）。 */
const CATALOG: ModelCatalog = {
  protocols: [
    {
      protocol: 'openai',
      efforts: ['low', 'high', 'max'],
      models: [
        {
          value: 'deepseek-reasoner',
          contextLength: 131072,
          maxTokens: 65536,
          supportsThinking: true,
          efforts: null,
        },
        {
          value: 'deepseek-chat',
          contextLength: 131072,
          maxTokens: 8192,
          supportsThinking: false,
          efforts: [],
        },
      ],
    },
  ],
};

function mockIpc() {
  invokeMock.mockReset();
  toastErrorMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'list_agent_providers') return Promise.resolve(providersFixture);
    if (command === 'list_agent_instances') return Promise.resolve(instancesFixture);
    if (command === 'list_model_catalog') {
      if (catalogMode === 'reject') return Promise.reject(new Error('目录不可用'));
      // 在途：永不 settle（loading 态保持）
      if (catalogMode === 'pending') return new Promise(() => {});
      return Promise.resolve(CATALOG);
    }
    // 保存面返回落库记录（结果面形状合法，不造漂移噪声）
    if (command === 'save_agent_provider') {
      return Promise.resolve({
        id: 99,
        name: '保存端点',
        protocol: 'openai',
        baseUrl: 'https://new.example.com/v1',
        apiKey: 'sk-new-key-999',
        models: { high: '', medium: '', low: '' },
        contextLength: null,
        maxTokens: null,
        webSearch: false,
        efforts: { high: '', low: '' },
      });
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  providersFixture = [];
  instancesFixture = [];
  catalogMode = 'ok';
  mockIpc();
});

describe('AgentsView 两栏骨架（AC-1）', () => {
  it('渲染 Providers / Agents 两栏；两 hook 挂载取数回填后清单呈现记录（真实组合）', async () => {
    providersFixture = [
      {
        id: 1,
        name: '端点甲',
        protocol: 'openai',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
        maxTokens: null,
        webSearch: false,
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

  it('页面不触发任何 workspace 命令：invoke 面仅三条清单取数各恰一次', async () => {
    render(<AgentsView />);
    await waitFor(() => expect(screen.getByTestId('provider-empty') !== null).toBe(true));

    const commandNames = invokeMock.mock.calls.map(([name]) => name);
    expect([...commandNames].sort((a, b) => a.localeCompare(b))).toEqual([
      'list_agent_instances',
      'list_agent_providers',
      'list_model_catalog',
    ]);
    // 全局语义：无 workspace 域命令（list_workspaces / agent_runs 等零触发）
    expect(commandNames).not.toContain('list_workspaces');
    expect(commandNames).not.toContain('agent_runs');
  });
});

// ---------------------------------------------------------------------------
// 目录装配与降级（AC-12 前端半）：目录取数收口在视图根、以 catalog 下传面板；
// 面板保持无取数职责，取数失败只退化为「无建议」，不阻断表单。
// ---------------------------------------------------------------------------

describe('AgentsView：模型目录装配与降级（AC-12）', () => {
  it('目录成功 ⇒ 打开 provider 表单后模型组合框建议项在场（目录已从视图下传）', async () => {
    render(<AgentsView />);
    await waitFor(() => expect(screen.getByTestId('provider-empty') !== null).toBe(true));

    fireEvent.click(screen.getByTestId('provider-new'));
    fireEvent.mouseDown(screen.getByTestId('provider-model-high'));

    const listbox = await screen.findByRole('listbox');
    expect(
      within(listbox)
        .getAllByRole('option')
        .map((option) => option.textContent),
    ).toEqual(['deepseek-reasoner', 'deepseek-chat']);
  });

  it('目录取数 reject ⇒ 视图与清单照常、表单可提交（无建议）、无阻断 error、toast 零调用', async () => {
    catalogMode = 'reject';
    mockIpc();
    providersFixture = [
      {
        id: 1,
        name: '端点甲',
        protocol: 'openai',
        baseUrl: 'https://api.example.com/v1',
        apiKey: 'sk-live-1234567890',
        models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
        maxTokens: null,
        webSearch: false,
      },
    ];

    render(<AgentsView />);
    await waitFor(() => expect(screen.getAllByTestId('provider-item')).toHaveLength(1));
    expect(screen.getByTestId('agents-view') !== null).toBe(true);
    expect(screen.queryByTestId('provider-error')).toBeNull();

    fireEvent.click(screen.getByTestId('provider-new'));
    fireEvent.mouseDown(screen.getByTestId('provider-model-high'));
    await waitFor(() => expect(screen.queryByRole('listbox')).toBeNull());

    fireEvent.change(screen.getByTestId('provider-name'), { target: { value: '降级端点' } });
    fireEvent.change(screen.getByTestId('provider-base-url'), {
      target: { value: 'https://new.example.com/v1' },
    });
    fireEvent.change(screen.getByTestId('provider-api-key'), {
      target: { value: 'sk-new-key-999' },
    });
    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() =>
      expect(invokeMock.mock.calls.some(([name]) => name === 'save_agent_provider')).toBe(true),
    );
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it('目录取数在途（loading）⇒ 表单组合框已在场且可键入，不阻断录入', async () => {
    catalogMode = 'pending';
    mockIpc();

    render(<AgentsView />);
    await waitFor(() => expect(screen.getByTestId('provider-empty') !== null).toBe(true));

    fireEvent.click(screen.getByTestId('provider-new'));
    const input = screen.getByTestId<HTMLInputElement>('provider-model-high');
    expect(input.tagName).toBe('INPUT');

    fireEvent.change(input, { target: { value: '手填模型' } });
    expect(input.value).toBe('手填模型');

    fireEvent.mouseDown(input);
    await waitFor(() => expect(screen.queryByRole('listbox')).toBeNull());
  });
});
