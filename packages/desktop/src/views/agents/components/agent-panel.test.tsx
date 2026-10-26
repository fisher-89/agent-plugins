import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentInstanceRecord, AgentProviderRecord } from '../../../types/generated/bindings';
import type { AgentInstancesState } from '../hooks/use-agent-instances';
import { AgentPanel } from './agent-panel';

// ---------------------------------------------------------------------------
// state（AgentInstancesState）与 providers 均为显式入参注入（入参数外）：
// 动作 save / remove / setDefault 以 vi.fn() 可编程 resolve / reject；
// providers fixture 含命中 / 缺失引用两形态；组件不触 invoke（无 IPC），
// sdk 必填与拒 Cli 校验单点在 store（前端不抢做校验，仅呈现后端 Err）。
// 存量 cli 行只读可删（DD4）：「编辑」「设为默认」禁用 + title 退役提示、
// 「删除」保留；engine 下拉收单值 sdk（AC-8）。
// ---------------------------------------------------------------------------

/** provider 记录 fixture。 */
function providerRecord(id: number, name: string): AgentProviderRecord {
  return {
    id,
    name,
    protocol: 'openai',
    baseUrl: 'https://api.example.com/v1',
    apiKey: 'sk-live-1234567890',
    models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
    maxTokens: null,
    webSearch: false,
  };
}

/** agent 实例 fixture（providerId 缺省 null，对齐存量 cli 行直呼）。 */
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

/** StandardSelect（base-ui Select）：testid 落在 SelectValue span（`data-value`
 * 承载选中值），trigger 为其外层 button（role=combobox）。 */
function selectValueEl(testId: string): HTMLElement {
  return screen.getByTestId(testId);
}

function selectTrigger(testId: string): HTMLButtonElement {
  const button = selectValueEl(testId).closest('button');
  if (!button) throw new Error(`SelectValue 不在触发器按钮内：${testId}`);
  return button;
}

/** 打开选择器：base-ui Select 以 mousedown 打开、frame 异步执行，以 listbox
 * 出现收敛。 */
async function openSelect(testId: string): Promise<void> {
  if (screen.queryByRole('listbox')) return;
  fireEvent.mouseDown(selectTrigger(testId));
  await screen.findByRole('listbox');
}

/** 打开选择器并点选指定 label 项：pointerDown 先行满足鼠标选择守卫，click
 * 提交选中。 */
async function pickOption(testId: string, label: string): Promise<void> {
  await openSelect(testId);
  const option = within(screen.getByRole('listbox')).getByRole('option', { name: label });
  fireEvent.pointerDown(option);
  fireEvent.click(option);
}

/** 当前弹层 option 文本清单（弹层已开；顺序即渲染序）。 */
function optionLabels(): string[] {
  return within(screen.getByRole('listbox'))
    .getAllByRole('option')
    .map((option) => option.textContent ?? '');
}

// ---------------------------------------------------------------------------
// 清单呈现：存量 cli 行只读可删（DD4 / AC-5 前端半边）
// ---------------------------------------------------------------------------

describe('AgentPanel：清单呈现（存量 cli 行历史可读）', () => {
  it('清单渲染 name / engine 文本 / 引用 provider 名（经 providers 清单按 id 解析）/ 默认标记', () => {
    mount(
      fakeState({
        instances: [instance(1, 'cli实例', 'cli', null, true), instance(2, 'sdk实例', 'sdk', 22)],
      }),
    );

    const items = screen.getAllByTestId('agent-item');
    expect(items).toHaveLength(2);
    expect(items[0].getAttribute('data-name')).toBe('cli实例');
    expect(items[0].textContent).toContain('cli');
    // cli 无引用 → 占位「—」；默认标记呈徽标（历史可读，R5 呈现兜底）
    expect(items[0].textContent).toContain('provider: —');
    expect(screen.getByTestId('agent-default-badge').textContent).toBe('默认');

    // sdk 行经 providers 按 id 解析引用 provider 名
    expect(items[1].textContent).toContain('端点乙');
    expect(items[1].textContent).toContain('sdk');
    // 切换入口逐行齐备：当前默认行禁用（自设默认无意义）
    const setDefaultEntries = screen.getAllByTestId<HTMLButtonElement>('agent-set-default');
    expect(setDefaultEntries).toHaveLength(2);
    expect(setDefaultEntries[0].disabled).toBe(true);
    expect(setDefaultEntries[1].disabled).toBe(false);
  });

  it('悬空引用 provider id（providers 清单缺失）→ 兜底 #id 形态呈现，不崩', () => {
    mount(fakeState({ instances: [instance(1, '悬空实例', 'sdk', 999)] }), []);

    expect(screen.getByTestId('agent-item').textContent).toContain('provider: #999');
  });

  it('cli 行「编辑」「设为默认」禁用且 title 提示退役事实；「删除」保留可用；sdk 行不受影响', () => {
    mount(
      fakeState({
        instances: [instance(1, 'cli实例', 'cli'), instance(2, 'sdk实例', 'sdk', 22)],
      }),
    );
    const items = screen.getAllByTestId('agent-item');

    const retiredTitle = 'CLI 租户已下线，实例只读';
    const cliEdit = within(items[0]).getByTestId<HTMLButtonElement>('agent-edit');
    expect(cliEdit.disabled).toBe(true);
    expect(cliEdit.getAttribute('title')).toBe(retiredTitle);
    const cliSetDefault = within(items[0]).getByTestId<HTMLButtonElement>('agent-set-default');
    expect(cliSetDefault.disabled).toBe(true);
    expect(cliSetDefault.getAttribute('title')).toBe(retiredTitle);
    expect(within(items[0]).getByTestId<HTMLButtonElement>('agent-delete').disabled).toBe(false);

    // sdk 行编辑 / 设默认不受影响（无退役 title）
    const sdkEdit = within(items[1]).getByTestId<HTMLButtonElement>('agent-edit');
    expect(sdkEdit.disabled).toBe(false);
    expect(sdkEdit.getAttribute('title')).toBeNull();
    expect(within(items[1]).getByTestId<HTMLButtonElement>('agent-set-default').disabled).toBe(
      false,
    );
  });

  it('cli 行「删除」保留可用 → state.remove 恰调用一次（只读可删的删半边）', async () => {
    const state = mount(fakeState({ instances: [instance(1, 'cli实例', 'cli')] }));

    fireEvent.click(screen.getByTestId('agent-delete'));
    await waitFor(() => expect(state.remove).toHaveBeenCalledTimes(1));
    expect(state.remove).toHaveBeenCalledWith(1);
  });
});

// ---------------------------------------------------------------------------
// engine 下拉收单值与 canSave 无 cli 分支（AC-8）
// ---------------------------------------------------------------------------

describe('AgentPanel：engine 下拉收单值与 canSave 无 cli 分支（AC-8）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('engine 下拉 options 仅 sdk（无 cli 档）且表单恒 sdk', async () => {
    mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    // 既有「engine 下拉仅 cli / sdk 二值」断言翻转为单值断言（无 'cli' 档）
    expect(selectValueEl('agent-engine-kind').getAttribute('data-value')).toBe('sdk');
    await openSelect('agent-engine-kind');
    expect(optionLabels()).toEqual(['（未选择）', 'sdk']);
  });

  it('sdk + 非空名 + provider 齐 → 保存启用且 state.save 恰调一次携全参', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    typeInto('agent-name', '新实例');
    await pickOption('agent-provider-select', '端点乙');
    expect(selectValueEl('agent-provider-select').getAttribute('data-value')).toBe('22');

    expect(screen.getByTestId<HTMLButtonElement>('agent-save').disabled).toBe(false);
    fireEvent.click(screen.getByTestId('agent-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: '新实例',
      engine: 'sdk',
      providerId: 22,
    });
  });

  it('空名 / 纯空白名 → 保存禁用（trim 门禁）；超长与特殊字符名照常启用且原样透传', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    await pickOption('agent-provider-select', '端点甲');
    const save = screen.getByTestId<HTMLButtonElement>('agent-save');

    typeInto('agent-name', '');
    expect(save.disabled).toBe(true);
    typeInto('agent-name', '   ');
    expect(save.disabled).toBe(true);

    // name str 边界：超长 >1000 / 特殊字符 emoji 引号 非空白即放行
    // （单行 input 按 HTML 语义不携换行，沿 provider-panel 同口径，特殊字符取
    // emoji + 引号形态）
    const specialName = '特殊 "名😀"';
    typeInto('agent-name', `超长名${'名'.repeat(1100)}`);
    expect(save.disabled).toBe(false);
    typeInto('agent-name', specialName);
    expect(save.disabled).toBe(false);

    // 特殊字符名原样透传（canSave 只看 trim 非空，不加工不拒绝）
    fireEvent.click(save);
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: specialName,
      engine: 'sdk',
      providerId: 11,
    });
  });

  it('providerId null 恒禁用保存（canSave 双条件：name 非空 && providerId 非空）', () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    typeInto('agent-name', '无provider实例');
    expect(screen.getByTestId<HTMLButtonElement>('agent-save').disabled).toBe(true);
    fireEvent.click(screen.getByTestId('agent-save'));
    expect(state.save).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// provider 选择器恒启用必选（AC-8）
// ---------------------------------------------------------------------------

describe('AgentPanel：provider 选择器恒启用必选（AC-8）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('无 cli 档禁用语义：选择器恒可交互（disabled 恒 false），选中回写随保存提交', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('agent-new'));
    typeInto('agent-name', '恒启用实例');
    expect(selectTrigger('agent-provider-select').disabled).toBe(false);

    await pickOption('agent-provider-select', '端点甲');
    expect(selectValueEl('agent-provider-select').getAttribute('data-value')).toBe('11');
    fireEvent.click(screen.getByTestId('agent-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: '恒启用实例',
      engine: 'sdk',
      providerId: 11,
    });
  });

  it('providers 空清单 → 选择器空态呈现、保存不可达（canSave false）不白屏', async () => {
    mount(fakeState(), []);

    fireEvent.click(screen.getByTestId('agent-new'));
    typeInto('agent-name', '空清单实例');
    expect(selectValueEl('agent-provider-select').textContent).toBe('请选择');
    await openSelect('agent-provider-select');
    expect(optionLabels()).toEqual(['（未选择）']);
    expect(screen.getByTestId<HTMLButtonElement>('agent-save').disabled).toBe(true);
  });

  it('providers 单元素清单选择器稳定呈现（未选择 + 单档两项）', async () => {
    mount(fakeState(), [providerRecord(31, '唯一端点')]);

    fireEvent.click(screen.getByTestId('agent-new'));
    await openSelect('agent-provider-select');
    expect(optionLabels()).toEqual(['（未选择）', '唯一端点']);
    expect(selectTrigger('agent-provider-select').disabled).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 保存失败呈现（双层兜底的后端层经动作轨透传，前端不吞）
// ---------------------------------------------------------------------------

describe('AgentPanel：保存失败呈现（异常）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('save 返回 null（模拟 store 拒收等 Err 经动作轨 toast 呈现）：表单保持、清单不变', async () => {
    const state = mount(
      fakeState({
        instances: [instance(1, '存量实例', 'sdk', 22)],
        save: vi.fn().mockResolvedValue(null),
      }),
    );

    // 编辑 sdk 实例后提交：后端 Err 以 null 返回（动作轨 toast 呈现不进本面）
    fireEvent.click(screen.getByTestId('agent-edit'));
    typeInto('agent-name', '改名实例');
    fireEvent.click(screen.getByTestId('agent-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));

    // 失败呈现半边：表单不收起（保留用户输入），清单不被组件自行改动
    await waitFor(() => expect(screen.queryByTestId('agent-form')).not.toBeNull());
    expect(screen.getAllByTestId('agent-item')).toHaveLength(1);
    expect(screen.getAllByTestId('agent-item')[0].getAttribute('data-name')).toBe('存量实例');
  });

  it('cli 行编辑入口禁用 → 表单不进 cli 编辑态（编辑禁用为主形态，DD4）', () => {
    mount(fakeState({ instances: [instance(1, 'cli存量', 'cli')] }));

    fireEvent.click(screen.getByTestId('agent-edit'));
    expect(screen.queryByTestId('agent-form')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 默认标记切换与删除（sdk 行；既有语义零改动回归锚）
// ---------------------------------------------------------------------------

describe('AgentPanel：默认标记切换', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('切换默认 → state.setDefault 以目标 id 恰调用一次', async () => {
    const state = mount(
      fakeState({
        instances: [instance(1, '甲', 'sdk', 11, true), instance(2, '乙', 'sdk', 11)],
      }),
    );

    // 点乙行的切换入口（甲行沿用默认标记、其入口禁用）
    fireEvent.click(screen.getAllByTestId('agent-set-default')[1]);
    await waitFor(() => expect(state.setDefault).toHaveBeenCalledTimes(1));
    expect(state.setDefault).toHaveBeenCalledWith(2);
  });

  it('成功后清单刷新呈现唯一默认标记（hook 刷新 → props 更新的呈现面）', async () => {
    const instances = [instance(1, '甲', 'sdk', 11, true), instance(2, '乙', 'sdk', 11)];
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

    // 点乙行的切换入口（甲行为当前默认——其入口禁用不可点）
    fireEvent.click(screen.getAllByTestId('agent-set-default')[1]);
    await waitFor(() => expect(state.setDefault).toHaveBeenCalledTimes(1));

    // 父级以刷新后的清单重渲染（真实链路为 hook setState → props 更新）
    await act(async () => {
      view.rerender(
        <AgentPanel providers={PROVIDERS} state={{ ...state, instances: [...instances] }} />,
      );
    });
    const items = screen.getAllByTestId('agent-item');
    expect(within(items[0]).queryByTestId('agent-default-badge')).toBeNull();
    expect(within(items[1]).getByTestId('agent-default-badge') !== null).toBe(true);
    expect(screen.getAllByTestId('agent-default-badge')).toHaveLength(1);
  });
});

describe('AgentPanel：删除（边界）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('删除 sdk agent → state.remove 调用恰一次', async () => {
    const state = mount(fakeState({ instances: [instance(1, '甲', 'sdk', 11)] }));

    fireEvent.click(screen.getByTestId('agent-delete'));
    await waitFor(() => expect(state.remove).toHaveBeenCalledTimes(1));
    expect(state.remove).toHaveBeenCalledWith(1);
  });

  it('默认 agent 删除后清单刷新无默认标记（fixture 驱动，无顺延）', async () => {
    const instances = [instance(1, '甲', 'sdk', 11, true), instance(2, '乙', 'sdk', 11)];
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

// ---------------------------------------------------------------------------
// 状态面（既有断言保绿）
// ---------------------------------------------------------------------------

describe('AgentPanel：状态面', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
