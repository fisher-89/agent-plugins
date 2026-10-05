import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentProviderRecord } from '../../../types/generated/bindings';
import type { AgentProvidersState } from '../hooks/use-agent-providers';
import { ProviderPanel } from './provider-panel';

/** provider 记录 fixture（serde camelCase 线格式，可空 contextLength）。 */
function provider(
  id: number,
  name: string,
  apiKey = 'sk-live-1234567890',
  contextLength: number | null = null,
): AgentProviderRecord {
  return {
    id,
    name,
    baseUrl: 'https://api.example.com/v1',
    apiKey,
    models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
    contextLength,
  };
}

/** AgentProvidersState fixture（动作轨道 vi.fn，可按需覆写实现）。 */
function fakeState(overrides: Partial<AgentProvidersState> = {}): AgentProvidersState {
  return {
    providers: [],
    loading: false,
    error: null,
    save: vi.fn().mockResolvedValue(null),
    remove: vi.fn().mockResolvedValue(false),
    ...overrides,
  };
}

function mount(state: AgentProvidersState) {
  render(<ProviderPanel state={state} />);
  return state;
}

function typeInto(testId: string, value: string) {
  fireEvent.change(screen.getByTestId(testId), { target: { value } });
}

describe('ProviderPanel：清单呈现（AC-9 前端半）', () => {
  it('清单渲染 name / base_url / models 三档 / 遮蔽 api_key（MaskedApiKey 真实组合，sk-***abc 形态）', () => {
    mount(fakeState({ providers: [provider(1, '端点甲'), provider(2, '端点乙', 'sk-abc-def')] }));

    const items = screen.getAllByTestId('provider-item');
    expect(items).toHaveLength(2);
    expect(items[0].getAttribute('data-name')).toBe('端点甲');
    expect(items[0].textContent).toContain('端点甲');
    expect(items[0].textContent).toContain('https://api.example.com/v1');
    // 三档 model 呈现
    expect(items[0].textContent).toContain('m-high');
    expect(items[0].textContent).toContain('m-medium');
    expect(items[0].textContent).toContain('m-low');
    // 遮蔽 api_key（末 3 字符 + sk-*** 前缀），明文全文不在 DOM
    expect(screen.getAllByTestId('masked-api-key')[0].textContent).toBe('sk-***890');
    expect(screen.getAllByTestId('masked-api-key')[1].textContent).toBe('sk-***def');
    expect(screen.getByTestId('provider-panel').textContent).not.toContain('sk-live-1234567890');
  });
});

describe('ProviderPanel：表单与提交（AC-2 前端半）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('新建表单提交 → state.save 以 (null, name, base_url, api_key, models 三档, contextLength null) 恰调用一次', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', '新端点');
    typeInto('provider-base-url', 'https://new.example.com/v1');
    typeInto('provider-api-key', 'sk-new-key-999');
    typeInto('provider-model-high', 'high-1');
    typeInto('provider-model-medium', 'mid-1');
    typeInto('provider-model-low', 'low-1');
    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: '新端点',
      baseUrl: 'https://new.example.com/v1',
      apiKey: 'sk-new-key-999',
      models: { high: 'high-1', medium: 'mid-1', low: 'low-1' },
      contextLength: null,
    });
  });

  it('新建表单填窗长提交 → state.save 入参 contextLength: 200000', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', '窗长端点');
    typeInto('provider-base-url', 'https://new.example.com/v1');
    typeInto('provider-api-key', 'sk-new-key-999');
    typeInto('provider-context-length', '200000');
    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith(expect.objectContaining({ contextLength: 200000 }));
  });

  it('编辑态 api_key 输入框恒空 + 提交 api_key 空串（留空 = 保持原值，回填语义由命令层承载）', async () => {
    const state = mount(fakeState({ providers: [provider(7, '存量端点')] }));

    fireEvent.click(screen.getByTestId('provider-edit'));
    // 编辑表单回填 name / base_url / 三档 model，api_key 恒空
    expect(screen.getByTestId<HTMLInputElement>('provider-name').value).toBe('存量端点');
    expect(screen.getByTestId<HTMLInputElement>('provider-base-url').value).toBe(
      'https://api.example.com/v1',
    );
    expect(screen.getByTestId<HTMLInputElement>('provider-api-key').value).toBe('');
    expect(screen.getByTestId<HTMLInputElement>('provider-model-high').value).toBe('m-high');

    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: 7,
      name: '存量端点',
      baseUrl: 'https://api.example.com/v1',
      apiKey: '',
      models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
      contextLength: null,
    });
  });

  it('编辑态 contextLength 回填：null → 空串；200000 → "200000"（toFormState 映射）', async () => {
    const state = mount(
      fakeState({
        providers: [provider(7, '缺列存量'), provider(8, '窗长存量', 'sk-live-1234567890', 200000)],
      }),
    );

    // contextLength null → 输入框空串
    fireEvent.click(screen.getAllByTestId('provider-edit')[0]);
    expect(screen.getByTestId<HTMLInputElement>('provider-context-length').value).toBe('');
    fireEvent.click(screen.getByTestId('provider-cancel'));

    // contextLength 200000 → 输入框 "200000"，提交原样入参
    fireEvent.click(screen.getAllByTestId('provider-edit')[1]);
    expect(screen.getByTestId<HTMLInputElement>('provider-context-length').value).toBe('200000');
    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith(
      expect.objectContaining({ id: 8, contextLength: 200000 }),
    );
  });

  it('保存成功收起表单；保存失败（save resolve null）表单保持（清单不变）', async () => {
    const state = mount(
      fakeState({
        providers: [provider(7, '存量端点')],
        save: vi.fn().mockResolvedValueOnce(provider(7, '存量端点')).mockResolvedValueOnce(null),
      }),
    );

    // 成功半边：save resolve 落库记录 → onDone 收起表单
    fireEvent.click(screen.getByTestId('provider-edit'));
    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByTestId('provider-form')).toBeNull());

    // 失败半边：save resolve null（后端 Err 经动作轨 toast 呈现的 hook 半）→ 表单保持
    fireEvent.click(screen.getByTestId('provider-edit'));
    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByTestId('provider-form')).not.toBeNull());
  });
});

describe('ProviderPanel：删除与错误呈现（AC-5 前端半）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('删除被引用 provider：state.remove 以 id 恰调用一次且清单不变（Err 呈现经 hook 动作轨 toast，组件清单面不变）', async () => {
    const state = mount(fakeState({ providers: [provider(1, '被引用端点')] }));

    fireEvent.click(screen.getByTestId('provider-delete'));
    await waitFor(() => expect(state.remove).toHaveBeenCalledTimes(1));
    expect(state.remove).toHaveBeenCalledWith(1);

    // 清单不变（remove 由 fixture 承接，组件不自行移除行）
    expect(screen.getAllByTestId('provider-item')).toHaveLength(1);
    expect(screen.getByTestId('provider-panel').textContent).toContain('被引用端点');
  });

  it('删除未被引用 provider：state.remove 调用，清单呈现保持由刷新轨道承载', async () => {
    const state = mount(fakeState({ providers: [provider(1, '可删端点')] }));

    fireEvent.click(screen.getByTestId('provider-delete'));
    await waitFor(() => expect(state.remove).toHaveBeenCalledWith(1));
    expect(screen.getAllByTestId('provider-item')).toHaveLength(1);
  });
});

describe('ProviderPanel：表单校验（边界）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('name 空白提交被阻（保存按钮禁用且 state.save 不被调用）', () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-base-url', 'https://new.example.com/v1');
    typeInto('provider-api-key', 'sk-new-key-999');

    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(true);
    fireEvent.click(screen.getByTestId('provider-save'));
    expect(state.save).not.toHaveBeenCalled();

    // 纯空白同样被阻
    typeInto('provider-name', '   ');
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(true);
  });

  it('api_key 新建必填（编辑态留空可提交）：新建未填 key 禁保存', () => {
    mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', '无名分端点');
    typeInto('provider-base-url', 'https://new.example.com/v1');
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(true);

    typeInto('provider-api-key', 'sk-x');
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(false);
  });

  it('超长（>1000 字符）name / base_url 与特殊字符原样入参（前端不截断不改写；单行 input 按 HTML 语义不携换行）', async () => {
    const state = mount(fakeState());
    const longName = `端点 ${'长'.repeat(1000)} 🎉 "引号"`;
    const longUrl = `https://例子.测试/${'x'.repeat(1000)}/v1 🚀`;

    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', longName);
    typeInto('provider-base-url', longUrl);
    typeInto('provider-api-key', 'sk-new-key-999');
    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: longName,
      baseUrl: longUrl,
      apiKey: 'sk-new-key-999',
      models: { high: '', medium: '', low: '' },
      contextLength: null,
    });
  });

  it('models 三档全空可提交（模型层无校验，前端不抢做）', async () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', '全空档端点');
    typeInto('provider-base-url', 'https://new.example.com/v1');
    typeInto('provider-api-key', 'sk-new-key-999');
    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: '全空档端点',
      baseUrl: 'https://new.example.com/v1',
      apiKey: 'sk-new-key-999',
      models: { high: '', medium: '', low: '' },
      contextLength: null,
    });
  });

  it('context_length 非正整数（0 / -1 / 1.5 / 非数字）提交被拒：保存禁用且 state.save 不被调用', () => {
    const state = mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', '边界端点');
    typeInto('provider-base-url', 'https://new.example.com/v1');
    typeInto('provider-api-key', 'sk-new-key-999');

    for (const invalid of ['0', '-1', '1.5', 'abc']) {
      typeInto('provider-context-length', invalid);
      expect(
        screen.getByTestId<HTMLButtonElement>('provider-save').disabled,
        `非法形态 ${invalid} 应禁用保存`,
      ).toBe(true);
      fireEvent.click(screen.getByTestId('provider-save'));
      expect(state.save, `非法形态 ${invalid} 不得提交`).not.toHaveBeenCalled();
    }

    // 合法正整数恢复可提交
    typeInto('provider-context-length', '131072');
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(false);
  });

  it('context_length 字段在场且携「留空跟随缺省 128K」语义提示（PROVIDER_FIELDS 增行）', () => {
    mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));

    const field = screen.getByTestId<HTMLInputElement>('provider-context-length');
    expect(field).toBeDefined();
    expect(field.placeholder).toContain('留空');
    expect(field.placeholder).toContain('128K');
  });
});

describe('ProviderPanel：空态与加载态（边界）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('空清单空态渲染不崩（provider-empty 在场）', () => {
    mount(fakeState({ providers: [] }));

    expect(screen.getByTestId('provider-empty').textContent).toContain('暂无 provider');
    expect(screen.queryAllByTestId('provider-item')).toHaveLength(0);
  });

  it('loading 态呈现（清单区收口为加载行），不与清单并存', () => {
    mount(fakeState({ loading: true }));

    expect(screen.getByTestId('provider-loading').textContent).toContain('加载中');
    expect(screen.queryByTestId('provider-list')).toBeNull();
  });

  it('error 态呈现（查询轨 inline 加载失败，不变形）', () => {
    mount(fakeState({ error: 'IPC 断开' }));

    expect(screen.getByTestId('provider-error').textContent).toContain('加载失败');
    expect(screen.getByTestId('provider-error').textContent).toContain('IPC 断开');
  });
});
