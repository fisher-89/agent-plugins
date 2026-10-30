import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentProviderRecord } from '../../../types/generated/bindings';
import type { AgentProvidersState } from '../hooks/use-agent-providers';
import { ProviderPanel } from './provider-panel';

/** provider 记录 fixture（serde camelCase 线格式，协议 + 可空 contextLength /
 * maxTokens）。 */
function provider(
  id: number,
  name: string,
  apiKey = 'sk-live-1234567890',
  contextLength: number | null = null,
): AgentProviderRecord {
  return {
    id,
    name,
    protocol: 'openai',
    baseUrl: 'https://api.example.com/v1',
    apiKey,
    models: { high: 'm-high', medium: 'm-medium', low: 'm-low' },
    contextLength,
    maxTokens: null,
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
      protocol: 'openai',
      contextLength: null,
      maxTokens: null,
      webSearch: false,
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
      protocol: 'openai',
      contextLength: null,
      maxTokens: null,
      webSearch: false,
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
      protocol: 'openai',
      contextLength: null,
      maxTokens: null,
      webSearch: false,
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
      protocol: 'openai',
      contextLength: null,
      maxTokens: null,
      webSearch: false,
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

  it('context_length 字段在场且携「默认 128K」语义提示（PROVIDER_FIELDS 增行）', () => {
    mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));

    const field = screen.getByTestId<HTMLInputElement>('provider-context-length');
    expect(field).toBeDefined();
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

// ---------------------------------------------------------------------------
// 协议选择器 / base_url 提示联动（DD19 / AC-11）：新建缺省 openai、切换即换
// placeholder 与提示文案（受控表单双向）
// ---------------------------------------------------------------------------

/** 协议选择器的 SelectValue（data-testid 落点，data-value 承载选中线值）。 */
function protocolValueEl(): HTMLElement {
  return screen.getByTestId('provider-protocol');
}

/** 协议选择器触发器（SelectValue 外层 button）。 */
function protocolTrigger(): HTMLButtonElement {
  const button = protocolValueEl().closest('button');
  if (!button) throw new Error('SelectValue 不在触发器按钮内');
  return button;
}

/** 打开协议弹层：base-ui Select 以 mousedown 打开、frame 异步执行。 */
async function openProtocolSelect(): Promise<void> {
  fireEvent.mouseDown(protocolTrigger());
  await screen.findByRole('listbox');
}

/** 选中协议项（pointerDown 满足鼠标选择守卫 + click 提交）。 */
async function pickProtocol(label: string): Promise<void> {
  await openProtocolSelect();
  const option = screen.getByRole('option', { name: label });
  fireEvent.pointerDown(option);
  fireEvent.click(option);
}

describe('ProviderPanel：协议选择器与 base_url 提示联动（AC-11）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('新建缺省协议 openai：选择器现值 openai 且 base_url placeholder 为 openai 形态', () => {
    mount(fakeState());

    fireEvent.click(screen.getByTestId('provider-new'));

    expect(protocolValueEl().getAttribute('data-value')).toBe('openai');
    expect(screen.getByTestId<HTMLInputElement>('provider-base-url').placeholder).toBe(
      'https://…/v1',
    );
    expect(screen.getByTestId('provider-base-url-hint').textContent).toContain('含版本段');
  });

  it('切 anthropic → placeholder 与提示文案同步；切回 openai 复原（受控双向）', async () => {
    mount(fakeState());
    fireEvent.click(screen.getByTestId('provider-new'));

    await pickProtocol('anthropic');

    expect(protocolValueEl().getAttribute('data-value')).toBe('anthropic');
    expect(screen.getByTestId<HTMLInputElement>('provider-base-url').placeholder).toBe(
      'https://api.anthropic.com',
    );
    const anthropicHint = screen.getByTestId('provider-base-url-hint').textContent ?? '';
    expect(anthropicHint).toContain('/v1 等后缀由引擎自动剥离后拼 /v1/messages');

    await pickProtocol('openai');

    expect(protocolValueEl().getAttribute('data-value')).toBe('openai');
    expect(screen.getByTestId<HTMLInputElement>('provider-base-url').placeholder).toBe(
      'https://…/v1',
    );
    expect(screen.getByTestId('provider-base-url-hint').textContent).toContain('含版本段');
  });

  it('协议选择器弹层关闭不置空：Escape 后现值保持且表单不崩', async () => {
    mount(fakeState());
    fireEvent.click(screen.getByTestId('provider-new'));

    await pickProtocol('anthropic');
    await openProtocolSelect();
    fireEvent.keyDown(screen.getByRole('listbox'), { key: 'Escape' });

    // 清空 / 失焦形态不置空（DD19 显式约束）
    expect(protocolValueEl().getAttribute('data-value')).toBe('anthropic');
    expect(screen.queryByTestId('provider-form')).not.toBeNull();
  });

  it('缺协议列的存量记录：徽标与编辑表单回退 openai 档（不渲染空徽标）', () => {
    const legacy = {
      ...provider(1, '存量端点'),
      protocol: undefined as unknown as AgentProviderRecord['protocol'],
    };
    mount(fakeState({ providers: [legacy] }));

    expect(screen.getByTestId('provider-protocol-badge').textContent).toBe('openai');

    fireEvent.click(screen.getByTestId('provider-edit'));
    expect(protocolValueEl().getAttribute('data-value')).toBe('openai');
  });
});

// ---------------------------------------------------------------------------
// 清单协议徽标（AC-11）：每行徽标与记录 protocol 一致（不渲染空徽标 / 不串行）
// ---------------------------------------------------------------------------

describe('ProviderPanel：清单协议徽标（AC-11）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('openai / anthropic 两条 → 每行徽标文案与记录协议一致', () => {
    const anthropic = {
      ...provider(2, '协议端点乙'),
      protocol: 'anthropic' as const,
      baseUrl: 'https://api.anthropic.com',
    };
    mount(fakeState({ providers: [provider(1, '协议端点甲'), anthropic] }));

    const badges = screen.getAllByTestId('provider-protocol-badge');
    expect(badges).toHaveLength(2);
    expect(badges[0].textContent).toBe('openai');
    expect(badges[1].textContent).toBe('anthropic');
    // 不串行：徽标按记录序与对应行同源
    const items = screen.getAllByTestId('provider-item');
    expect(items[0].getAttribute('data-name')).toBe('协议端点甲');
    expect(items[1].getAttribute('data-name')).toBe('协议端点乙');
  });
});

// ---------------------------------------------------------------------------
// 模型名后缀提示（DD19）：带 [1m] 且 context_length 非空时声明「已被覆盖」
// ---------------------------------------------------------------------------

describe('ProviderPanel：模型名后缀提示（DD19）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('带后缀且 context_length 非空 → 逐档列出「窗长已被模型名覆盖」', () => {
    mount(fakeState());
    fireEvent.click(screen.getByTestId('provider-new'));

    typeInto('provider-model-high', 'claude-x[1m]');
    typeInto('provider-context-length', '200000');

    const hint = screen.getByTestId('provider-suffix-hint').textContent ?? '';
    expect(hint).toContain('high 档窗长已被模型名覆盖');
    expect(hint).toContain('后缀优先于 context_length');
  });

  it('多档带后缀 → 逐档列出；无后缀 → 不呈现覆盖提示（[1m] 基础说明仅 anthropic 档）', async () => {
    mount(fakeState());
    fireEvent.click(screen.getByTestId('provider-new'));
    // 后缀语义归 anthropic 协议：[1m] 基础说明仅在 anthropic 档呈现
    await pickProtocol('anthropic');
    typeInto('provider-context-length', '200000');

    typeInto('provider-model-high', 'claude-x[1m]');
    typeInto('provider-model-low', 'claude-y[1M]');
    const multi = screen.getByTestId('provider-suffix-hint').textContent ?? '';
    expect(multi).toContain('high / low 档窗长已被模型名覆盖');

    // 无后缀档位 → 该提示不呈现（仅基础说明）
    typeInto('provider-model-high', 'claude-x');
    typeInto('provider-model-low', 'claude-y');
    const plain = screen.getByTestId('provider-suffix-hint').textContent ?? '';
    expect(plain).not.toContain('已被模型名覆盖');
    expect(plain).toContain('[1m]');
  });

  it('context_length 留空 → 不呈现覆盖提示（无叠加歧义）', () => {
    mount(fakeState());
    fireEvent.click(screen.getByTestId('provider-new'));

    typeInto('provider-model-high', 'claude-x[1m]');
    typeInto('provider-context-length', '');

    expect(screen.getByTestId('provider-suffix-hint').textContent).not.toContain('已被模型名覆盖');
  });
});

// ---------------------------------------------------------------------------
// max_tokens 字段（DD19 / AC-11）：输入 / 下限 / 非法形态 / 字符串边界 /
// 编辑态回填
// ---------------------------------------------------------------------------

describe('ProviderPanel：max_tokens 字段（AC-11）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /** 填完必填项（新建形态），返回 state 方便后续断言。 */
  function fillRequired(state: AgentProvidersState): AgentProvidersState {
    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', '上限端点');
    typeInto('provider-base-url', 'https://api.anthropic.com');
    typeInto('provider-api-key', 'sk-ant-x');
    return state;
  }

  it('max_tokens 留空 → 提交 maxTokens: null（留空 = 未配置）', async () => {
    const state = fillRequired(mount(fakeState()));

    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith(
      expect.objectContaining({ contextLength: null, maxTokens: null }),
    );
  });

  it('max_tokens 填 8192 + 协议 anthropic → 提交入参逐键对齐（含新字段）', async () => {
    const state = fillRequired(mount(fakeState()));

    await pickProtocol('anthropic');
    typeInto('provider-model-high', 'claude-sonnet-4-5[1m]');
    typeInto('provider-model-medium', 'claude-m');
    typeInto('provider-model-low', 'claude-haiku-4-5');
    typeInto('provider-context-length', '200000');
    typeInto('provider-max-tokens', '8192');
    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith({
      id: null,
      name: '上限端点',
      protocol: 'anthropic',
      baseUrl: 'https://api.anthropic.com',
      apiKey: 'sk-ant-x',
      models: { high: 'claude-sonnet-4-5[1m]', medium: 'claude-m', low: 'claude-haiku-4-5' },
      contextLength: 200000,
      maxTokens: 8192,
      webSearch: false,
    });
  });

  it('低于下限（0 / 4095 / -1）→ 保存禁用且不提交；恰 4096 放行（与后端 MIN_OUTPUT_TOKENS 对齐）', () => {
    const state = fillRequired(mount(fakeState()));

    for (const invalid of ['0', '4095', '-1']) {
      typeInto('provider-max-tokens', invalid);
      expect(
        screen.getByTestId<HTMLButtonElement>('provider-save').disabled,
        `低于下限 ${invalid} 应禁用保存`,
      ).toBe(true);
      fireEvent.click(screen.getByTestId('provider-save'));
      expect(state.save, `低于下限 ${invalid} 不得提交`).not.toHaveBeenCalled();
    }

    typeInto('provider-max-tokens', '4096');
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(false);
  });

  it('非法形态（abc / 1.5 / 8e3 / 8192abc）→ 保存禁用且不提交', () => {
    const state = fillRequired(mount(fakeState()));

    for (const invalid of ['abc', '1.5', '8e3', '8192abc']) {
      typeInto('provider-max-tokens', invalid);
      expect(
        screen.getByTestId<HTMLButtonElement>('provider-save').disabled,
        `非法形态 ${invalid} 应禁用保存（非十进制整数一律非法）`,
      ).toBe(true);
      fireEvent.click(screen.getByTestId('provider-save'));
      expect(state.save, `非法形态 ${invalid} 不得提交`).not.toHaveBeenCalled();
    }
  });

  it('字符串边界：前后空白裁剪、前导零按数值语义、精度外超长串拒提交', async () => {
    const state = fillRequired(mount(fakeState()));

    // 前后空白 → 裁剪后 8192
    typeInto('provider-max-tokens', ' 8192 ');
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(false);

    // 前导零 → 数值语义 4096 合法
    typeInto('provider-max-tokens', '04096');
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(false);

    // 精度外（>20 位）→ 非法拒提交（不静默截断）
    typeInto('provider-max-tokens', '1'.repeat(25));
    expect(screen.getByTestId<HTMLButtonElement>('provider-save').disabled).toBe(true);
    fireEvent.click(screen.getByTestId('provider-save'));
    expect(state.save).not.toHaveBeenCalled();

    // 回到合法值提交：空白被裁剪后的数值入参
    typeInto('provider-max-tokens', ' 8192 ');
    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith(expect.objectContaining({ maxTokens: 8192 }));
  });

  it('编辑态回填：protocol 原值 + maxTokens 数值回填；null → 空串（不渲染 "null"）', async () => {
    const anthropic = {
      ...provider(8, '上限存量'),
      protocol: 'anthropic' as const,
      contextLength: 200000,
      maxTokens: 8192,
    };
    const state = mount(fakeState({ providers: [provider(7, '缺列存量'), anthropic] }));

    // maxTokens null → 空串「未配置」语义
    fireEvent.click(screen.getAllByTestId('provider-edit')[0]);
    expect(screen.getByTestId<HTMLInputElement>('provider-max-tokens').value).toBe('');
    expect(screen.getByTestId<HTMLInputElement>('provider-max-tokens').value).not.toBe('null');
    fireEvent.click(screen.getByTestId('provider-cancel'));

    // 编辑 anthropic 记录：协议与上限原值回填，提交原样回传
    fireEvent.click(screen.getAllByTestId('provider-edit')[1]);
    expect(protocolValueEl().getAttribute('data-value')).toBe('anthropic');
    expect(screen.getByTestId<HTMLInputElement>('provider-max-tokens').value).toBe('8192');

    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith(
      expect.objectContaining({ id: 8, protocol: 'anthropic', maxTokens: 8192 }),
    );
  });

  it('max_tokens 提示锚在场（锚名 + 语义文案）', () => {
    mount(fakeState());
    fireEvent.click(screen.getByTestId('provider-new'));

    const hint = screen.getByTestId('provider-max-tokens-hint').textContent ?? '';
    expect(hint).toContain('留空 = 按协议缺省');
    expect(hint).toContain('仅 anthropic 协议生效');
    expect(hint).toContain('4096');
    expect(screen.getByTestId<HTMLInputElement>('provider-max-tokens').placeholder).toBe(
      '留空 = 按协议缺省',
    );
  });
});

// ---------------------------------------------------------------------------
// web_search 勾选项（AC-11）：仅 anthropic 渲染、切协议清零、载荷恒携
// ---------------------------------------------------------------------------

/** 携协议 + 开关的记录 fixture（他端写入的存量态可构造）。 */
function providerWith(
  id: number,
  name: string,
  protocol: AgentProviderRecord['protocol'],
  webSearch: boolean | undefined,
): AgentProviderRecord {
  return {
    ...provider(id, name),
    protocol,
    webSearch,
  };
}

/** 打开编辑表单（唯一 provider 行）。 */
function openEdit(state: AgentProvidersState): void {
  render(<ProviderPanel state={state} />);
  fireEvent.click(screen.getByTestId('provider-edit'));
}

/** 勾选 / 取消 web_search。 */
function toggleWebSearch(): void {
  fireEvent.click(screen.getByTestId('provider-web-search'));
}

describe('ProviderPanel：web_search 仅 anthropic 可见（AC-11）', () => {
  it('openai 记录 → 勾选项不在文档中；切到 anthropic → checkbox 出现、未勾、计费提示在场', async () => {
    openEdit(fakeState({ providers: [providerWith(41, 'openai 端点', 'openai', false)] }));

    expect(screen.queryByTestId('provider-web-search')).toBeNull();

    await pickProtocol('anthropic');

    const checkbox = screen.getByTestId('provider-web-search');
    expect(checkbox).toBeTruthy();
    expect(checkbox.getAttribute('aria-checked')).toBe('false');
    expect(screen.getByTestId('provider-form').textContent).toContain(
      'Anthropic 服务端网页搜索（按次计费）',
    );
  });

  it('存量 openai + true：openai 下不渲染也不就地改值；切到 anthropic 后按记录回显为已勾', async () => {
    openEdit(fakeState({ providers: [providerWith(42, '存量开关端点', 'openai', true)] }));

    expect(screen.queryByTestId('provider-web-search')).toBeNull();

    await pickProtocol('anthropic');

    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('true');
  });

  it('编辑态回显：anthropic + true → 勾选态；webSearch 键缺席 / null（旧记录）→ 未勾', () => {
    const on = render(
      <ProviderPanel
        state={fakeState({ providers: [providerWith(43, '开关端点', 'anthropic', true)] })}
      />,
    );
    fireEvent.click(screen.getByTestId('provider-edit'));
    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('true');
    on.unmount();

    const absent = render(
      <ProviderPanel
        state={fakeState({ providers: [providerWith(44, '旧记录端点', 'anthropic', undefined)] })}
      />,
    );
    fireEvent.click(screen.getByTestId('provider-edit'));
    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('false');
    absent.unmount();

    // null（旧 JSON 读出形态）同按「未勾」处理（`?? false`，不抛错）
    const legacyNull = {
      ...provider(45, 'null 记录端点'),
      protocol: 'anthropic' as const,
      webSearch: null,
    } as unknown as AgentProviderRecord;
    render(<ProviderPanel state={fakeState({ providers: [legacyNull] })} />);
    fireEvent.click(screen.getByTestId('provider-edit'));
    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('false');
  });
});

describe('ProviderPanel：web_search 切协议清零与载荷恒携（AC-11）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('anthropic 下勾选 → 切 openai → 保存 → webSearch 为 false；切回 anthropic 仍未勾', async () => {
    const state = mount(
      fakeState({ providers: [providerWith(45, '切换端点', 'anthropic', false)] }),
    );

    fireEvent.click(screen.getByTestId('provider-edit'));
    toggleWebSearch();
    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('true');

    await pickProtocol('openai');
    expect(screen.queryByTestId('provider-web-search')).toBeNull();
    fireEvent.click(screen.getByTestId('provider-save'));

    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    expect(state.save).toHaveBeenCalledWith(expect.objectContaining({ webSearch: false }));

    // 切回 anthropic：不自动恢复（需用户重勾）
    await pickProtocol('anthropic');
    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('false');
  });

  it('提交载荷恒携 webSearch：openai 下为 false（键在场），anthropic + 勾选为 true', async () => {
    const state = mount(fakeState());
    fireEvent.click(screen.getByTestId('provider-new'));
    typeInto('provider-name', '恒携端点');
    typeInto('provider-base-url', 'https://new.example.com/v1');
    typeInto('provider-api-key', 'sk-new-key-999');

    // 新建缺省 openai：未勾 → webSearch: false 键在场
    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(1));
    const firstPayload = vi.mocked(state.save).mock.calls[0][0];
    expect(Object.keys(firstPayload)).toContain('webSearch');
    expect(firstPayload.webSearch).toBe(false);

    // anthropic + 勾选 → true
    await pickProtocol('anthropic');
    toggleWebSearch();
    fireEvent.click(screen.getByTestId('provider-save'));
    await waitFor(() => expect(state.save).toHaveBeenCalledTimes(2));
    expect(vi.mocked(state.save).mock.calls[1][0].webSearch).toBe(true);
  });

  it('布尔控件与文本字段分离：勾选项不是文本输入，连续点击两次状态正确切换', async () => {
    mount(fakeState({ providers: [providerWith(46, '控件端点', 'anthropic', false)] }));
    fireEvent.click(screen.getByTestId('provider-edit'));

    const checkbox = screen.getByTestId('provider-web-search');
    expect(checkbox.tagName).not.toBe('INPUT');
    expect(checkbox.getAttribute('type')).not.toBe('text');

    toggleWebSearch();
    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('true');
    toggleWebSearch();
    expect(screen.getByTestId('provider-web-search').getAttribute('aria-checked')).toBe('false');
  });
});
