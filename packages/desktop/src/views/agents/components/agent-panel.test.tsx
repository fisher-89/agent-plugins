import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentInstanceRecord, AgentProviderRecord } from '../../../types/generated/bindings';
import type { AgentInstancesState } from '../hooks/use-agent-instances';
import { AgentPanel } from './agent-panel';

// ---------------------------------------------------------------------------
// state（AgentInstancesState）与 providers 均为显式入参注入（入参例外）：
// 动作 save / remove / setDefault 以 vi.fn() 可编程 resolve / reject；
// providers fixture 含命中 / 缺失引用两形态；组件不触 invoke（无 IPC），
// sdk 必填校验单点在 store（前端不抢做校验，仅呈现后端 Err）。
// ---------------------------------------------------------------------------

/** provider 记录 fixture。 */
function providerRecord(id: number, name: string): AgentProviderRecord {
  return {
    id,
    name,
    baseUrl: 'https://api.example.com/v1',
    apiKey: 'sk-live-1234567890',
    models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
  };
}

/** agent 实例 fixture（providerId 缺省 null，对齐 cli 形态直呼）。 */
function instance(
  id: number,
  name: string,
  engine: AgentInstanceRecord['engine'],
  providerId: number | null = null,
  isDefault = false,
): AgentInstanceRecord {
  return { id, name, engine, providerId, isDefault };
}

/** AgentInstancesState fixture（动作轨道 vi.fn，可按需覆写实现）。 */
function fakeState(overrides: Partial<AgentInstancesState> = {}): AgentInstancesState {
  return {
    instances: [],
    loading: false,
    error: null,
    save: vi.fn().mockResolvedValue(null),
    remove: vi.fn().mockResolvedValue(false),
    setDefault: vi.fn().mockResolvedValue(null),
    ...overrides,
  };
}

const PROVIDERS = [providerRecord(11, '端点甲'), providerRecord(22, '端点乙')];

function mount(state: AgentInstancesState, providers: AgentProviderRecord[] = PROVIDERS) {
  render(<AgentPanel providers={providers} state={state} />);
  return state;
}

function typeInto(testId: string, value: string) {
  fireEvent.change(screen.getByTestId(testId), { target: { value } });
}

describe('AgentPanel：清单呈现', () => {
  it('清单渲染 name / engine 二值 / 引用 provider 名（经 providers 清单按 id 解析）/ 默认标记', () => {
    mount(
      fakeState({
        instances: [instance(1, 'cli实例', 'cli', null, true), instance(2, 'sdk实例', 'sdk', 22)],
      }),
    );

    const items = screen.getAllByTestId('agent-item');
    expect(items).toHaveLength(2);
    expect(items[0].getAttribute('data-name')).toBe('cli实例');
    expect(items[0].textContent).toContain('cli');
    // cli 无引用 → 占位「—」；默认标记呈徽标
    expect(items[0].textContent).toContain('provider: —');
    expect(screen.getByTestId('agent-default-badge').textContent).toBe('默认');

    // sdk 行经 providers 按 id 解析引用 provider 名（悬空 id 兜底 #id 形态）
    expect(items[1].textContent).toContain('端点乙');
    expect(items[1].textContent).toContain('sdk');
    expect(screen.getAllByTestId('agent-set-default')).toHaveLength(1);
  });

  it('悬空引用 provider id（providers 清单缺失）→ 兜底 #id 形态呈现，不崩', () => {
    mount(fakeState({ instances: [instance(1, '悬空实例', 'sdk', 999)] }), []);

    expect(screen.getByTestId('agent-item').textContent).toContain('provider: #999');
  });
});

describe('AgentPanel：表单与提交（AC-3 前端半）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('cli agent provider 可空提交 → state.save 以 (null, name, "cli", null) 恰调用一次', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    typeInto('agent-name', 'cli新实例');
    fireEvent.change(screen.getByTestId('agent-engine-kind'), { target: { value: 'cli' } });
    fireEvent.click(screen.getByTestId('agent-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: 'cli新实例',
      engine: 'cli',
      providerId: null,
    });
  });

  it('sdk agent 选 provider 提交 → save 携 providerId', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    typeInto('agent-name', 'sdk新实例');
    // engine 初始 sdk：provider 选择启用
    fireEvent.change(screen.getByTestId('agent-provider-select'), {
      target: { value: '22' },
    });
    fireEvent.click(screen.getByTestId('agent-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: 'sdk新实例',
      engine: 'sdk',
      providerId: 22,
    });
  });

  it('engine 下拉仅 cli / sdk 二值；cli 档 provider 下拉禁用（可空语义）', () => {
    mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    const engineSelect = screen.getByTestId<HTMLSelectElement>('agent-engine-kind');
    expect(Array.from(engineSelect.options).map((option) => option.value)).toEqual(['sdk', 'cli']);

    fireEvent.change(engineSelect, { target: { value: 'cli' } });
    expect(screen.getByTestId<HTMLSelectElement>('agent-provider-select').disabled).toBe(true);
    fireEvent.change(engineSelect, { target: { value: 'sdk' } });
    expect(screen.getByTestId<HTMLSelectElement>('agent-provider-select').disabled).toBe(false);
  });
});

describe('AgentPanel：保存失败呈现（异常）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sdk 未选 provider：保存入口禁用、state.save 不被调用（校验单点在 store 的后端面由命令层测试承载）', () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    typeInto('agent-name', 'sdk无provider实例');
    // engine 初始 sdk、providerId 恒 null：canSave 前端门禁不发起提交
    expect(screen.getByTestId<HTMLButtonElement>('agent-save').disabled).toBe(true);
    fireEvent.click(screen.getByTestId('agent-save'));
    expect(state.save).not.toHaveBeenCalled();
  });

  it('后端 reject（save resolve null 模拟 Err 经动作轨 toast 呈现）：表单保持、清单不变', async () => {
    const state = mount(
      fakeState({
        instances: [instance(1, '存量实例', 'cli', null)],
        save: vi.fn().mockResolvedValue(null),
      }),
    );

    // 编辑存量 sdk 实例（引用 providerId=22）后提交：后端重名等 Err 以 null 返回
    fireEvent.click(screen.getByTestId('agent-edit'));
    typeInto('agent-name', '改名实例');
    fireEvent.click(screen.getByTestId('agent-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));

    // 失败呈现半边：表单不收起（保留用户输入），清单不被组件自行改动
    await waitFor(() => expect(screen.queryByTestId('agent-form')).not.toBeNull());
    expect(screen.getAllByTestId('agent-item')).toHaveLength(1);
    expect(screen.getAllByTestId('agent-item')[0].getAttribute('data-name')).toBe('存量实例');
  });
});

describe('AgentPanel：默认标记切换（AC-4 前端半）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('切换默认 → state.setDefault 以目标 id 恰调用一次', async () => {
    const state = mount(
      fakeState({
        instances: [instance(1, '甲', 'cli', null, true), instance(2, '乙', 'cli')],
      }),
    );

    fireEvent.click(screen.getByTestId('agent-set-default'));
    await waitFor(() => expect(state.setDefault).toHaveBeenCalledTimes(1));
    expect(state.setDefault).toHaveBeenCalledWith(2);
  });

  it('成功后清单刷新呈现唯一默认标记（hook 刷新 → props 更新的呈现面）', async () => {
    const instances = [instance(1, '甲', 'cli', null, true), instance(2, '乙', 'cli')];
    const state = fakeState({
      instances,
      setDefault: vi.fn().mockImplementation(async (id: number) => {
        // 模拟 hook 动作轨：标记即切换（替换清单内 is_default 布局）
        const next = instances.map((row) => ({ ...row, isDefault: row.id === id }));
        instances.splice(0, instances.length, ...next);
        return next.find((row) => row.id === id) ?? null;
      }),
    });
    const view = render(<AgentPanel providers={PROVIDERS} state={state} />);
    expect(screen.getAllByTestId('agent-item')[0].textContent).toContain('默认');

    fireEvent.click(screen.getByTestId('agent-set-default'));
    await waitFor(() => expect(state.setDefault).toHaveBeenCalledTimes(1));

    // 父级以刷新后的清单重渲染（真实链路为 hook setState → props 更新）
    await act(async () => {
      view.rerender(
        <AgentPanel providers={PROVIDERS} state={{ ...state, instances: [...instances] }} />,
      );
    });
    const items = screen.getAllByTestId('agent-item');
    // 唯一默认标记移至乙（行内「设为默认」按钮文案含「默认」二字，以徽标
    // testid 为准）
    expect(within(items[0]).queryByTestId('agent-default-badge')).toBeNull();
    expect(within(items[1]).getByTestId('agent-default-badge') !== null).toBe(true);
    expect(screen.getAllByTestId('agent-default-badge')).toHaveLength(1);
  });
});

describe('AgentPanel：删除（边界）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('删除 agent → state.remove 调用恰一次', async () => {
    const state = mount(fakeState({ instances: [instance(1, '甲', 'cli')] }));

    fireEvent.click(screen.getByTestId('agent-delete'));
    await waitFor(() => expect(state.remove).toHaveBeenCalledTimes(1));
    expect(state.remove).toHaveBeenCalledWith(1);
  });

  it('默认 agent 删除后清单刷新无默认标记（fixture 驱动，无顺延）', async () => {
    const instances = [instance(1, '甲', 'cli', null, true), instance(2, '乙', 'cli')];
    const state = fakeState({
      instances,
      remove: vi.fn().mockImplementation(async (id: number) => {
        // 模拟 hook 动作轨刷新：清单移除该行（默认标记不顺延，与 store 语义对齐）
        instances.splice(0, instances.length, ...instances.filter((row) => row.id !== id));
        return true;
      }),
    });
    const view = render(<AgentPanel providers={PROVIDERS} state={state} />);

    // 删默认 agent（清单首行）
    fireEvent.click(screen.getAllByTestId('agent-delete')[0]);
    await waitFor(() => expect(state.remove).toHaveBeenCalledWith(1));

    await act(async () => {
      view.rerender(
        <AgentPanel providers={PROVIDERS} state={{ ...state, instances: [...instances] }} />,
      );
    });
    expect(screen.getAllByTestId('agent-item')).toHaveLength(1);
    expect(screen.queryByTestId('agent-default-badge')).toBeNull();
    expect(screen.getAllByTestId('agent-item')[0].getAttribute('data-name')).toBe('乙');
  });
});

describe('AgentPanel：边界与状态面', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('providers 清单为空时 provider 下拉空态（仅「未选择」项）', () => {
    mount(fakeState(), []);

    fireEvent.click(screen.getByTestId('agent-new'));
    const providerSelect = screen.getByTestId<HTMLSelectElement>('agent-provider-select');
    expect(Array.from(providerSelect.options).map((option) => option.textContent)).toEqual([
      '（未选择）',
    ]);
  });

  it('空清单空态渲染不崩；loading / error 态呈现不变形', () => {
    mount(fakeState({ instances: [] }));
    expect(screen.getByTestId('agent-empty').textContent).toContain('暂无 agent');

    const { unmount } = render(
      <AgentPanel providers={PROVIDERS} state={fakeState({ loading: true })} />,
    );
    expect(screen.getAllByTestId('agent-loading')).toHaveLength(1);
    unmount();

    render(<AgentPanel providers={PROVIDERS} state={fakeState({ error: 'IPC 断开' })} />);
    expect(screen.getAllByTestId('agent-error')).toHaveLength(1);
    expect(screen.getAllByTestId('agent-error')[0].textContent).toContain('加载失败');
  });
});
