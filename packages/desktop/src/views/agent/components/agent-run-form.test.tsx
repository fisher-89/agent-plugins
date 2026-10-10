import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { AgentInstanceRecord, AgentPermissionMode } from '../../../types/dto';
import { AgentRunForm } from './agent-run-form';

// AgentRunForm 为纯回调组件（onStart 以 vi.fn() 注入），无进程边界，不需要 Mock。
// AgentStartInput 形状由 onStart 入参类型承载（'./agent-run-form' 同源导出）。

type AgentStartInput = Parameters<React.ComponentProps<typeof AgentRunForm>['onStart']>[0];

/** agent 实例 fixture（serde camelCase 线格式；providerId 对表单面无关可空） */
function agentInstance(
  id: number,
  name: string,
  engine: AgentInstanceRecord['engine'],
  isDefault = false,
): AgentInstanceRecord {
  return { id, name, engine, providerId: null, isDefault };
}

function mount(disabled = false, agents: AgentInstanceRecord[] = []) {
  const onStart = vi.fn();
  render(<AgentRunForm agents={agents} disabled={disabled} onStart={onStart} />);
  return { onStart };
}

function typePrompt(value: string) {
  fireEvent.change(screen.getByTestId('agent-prompt'), { target: { value } });
}

function startButton(): HTMLButtonElement {
  return screen.getByTestId('agent-start');
}

function selectOf(testId: string): HTMLSelectElement {
  return screen.getByTestId(testId);
}

/** agent-select 为 StandardSelect（base-ui Select）：testid 落在 SelectValue
 * span（data-value 承载选中 id），trigger 为其外层 button（role=combobox）。 */
function agentValue(): HTMLElement {
  return screen.getByTestId('agent-select');
}

function agentTrigger(): HTMLElement {
  const trigger = agentValue().closest('button');
  if (!trigger) throw new Error('agent-select 不在 trigger button 内');
  return trigger;
}

/** 打开 agent 选择器：base-ui Select 以 mousedown 打开、开启动作为 frame
 * 异步执行，以 listbox 出现收敛最终态。 */
async function openAgentSelect(): Promise<void> {
  fireEvent.mouseDown(agentTrigger());
  await screen.findByRole('listbox');
}

/** 打开选择器并点选指定 label 项：pointerDown 先行满足 base-ui 鼠标选择
 * 守卫（allowMouseSelectionRef），click 提交选中。option 查询收敛在 listbox
 * 内（native permission-mode select 的 option 同样呈 role=option）。 */
async function pickAgent(label: string): Promise<void> {
  await openAgentSelect();
  const option = within(screen.getByRole('listbox')).getByRole('option', { name: label });
  fireEvent.pointerDown(option);
  fireEvent.click(option);
}

describe('AgentRunForm：参数面默认值与 prompt 必填（AC-5）', () => {
  it('初始 permission-mode=bypassPermissions（默认档断言）', () => {
    mount();

    expect(selectOf('agent-permission-mode').value).toBe('bypassPermissions');
    // bare 认证前提提示初始不在场
    expect(screen.queryByTestId('bare-auth-note')).toBeNull();
  });

  it('prompt 为空串时启动按钮禁用；输入任意非空后启用', () => {
    mount();

    expect(startButton().disabled).toBe(true);
    // 纯空白仍视为空
    typePrompt('   ');
    expect(startButton().disabled).toBe(true);

    typePrompt('你好');
    expect(startButton().disabled).toBe(false);
  });
});

describe('AgentRunForm：onStart 回调与档位切换', () => {
  it('填写后点击启动 → onStart 以 { prompt, permissionMode, agent } 恰调用一次（空清单 agent=null 缺省）', () => {
    const { onStart } = mount();

    typePrompt('帮我跑一轮');
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onStart).toHaveBeenCalledWith({
      prompt: '帮我跑一轮',
      permissionMode: 'bypassPermissions',
      agent: null,
    } satisfies AgentStartInput);
  });

  it('permission-mode 三档下拉含 default / acceptEdits / bypassPermissions 且均可选回填', () => {
    mount();

    const select = selectOf('agent-permission-mode');
    const values = Array.from(select.options).map((option) => option.value);
    expect(values).toEqual(['bypassPermissions', 'acceptEdits', 'default']);

    for (const value of values as AgentPermissionMode[]) {
      fireEvent.change(select, { target: { value } });
      expect(select.value).toBe(value);
    }
  });
});

describe('AgentRunForm：agent 选择器（调试页 agent 选择，默认选中默认 agent）', () => {
  it('空清单：trigger 呈缺省占位（请选择）、data-value 缺席、弹层无 option（发起走后端缺省解析）', async () => {
    mount();

    expect(agentValue().getAttribute('data-value')).toBeNull();
    expect(agentValue().textContent).toBe('请选择');

    await openAgentSelect();
    expect(within(screen.getByRole('listbox')).queryAllByRole('option')).toHaveLength(0);
  });

  it('清单含默认 agent：初始即选中默认 agent（isDefault 记录 id，trigger 呈其 label）', async () => {
    mount(false, [agentInstance(3, 'cli-a', 'cli'), agentInstance(7, 'sdk-b', 'sdk', true)]);

    expect(agentValue().getAttribute('data-value')).toBe('7');
    expect(agentValue().textContent).toBe('sdk-b（sdk）');

    await openAgentSelect();
    const options = within(screen.getByRole('listbox')).getAllByRole('option');
    expect(options.map((option) => option.textContent)).toEqual(['cli-a（cli）', 'sdk-b（sdk）']);
  });

  it('切至显式 agent 后启动 → onStart 以 { prompt, permissionMode, agent: 3 } 恰调用一次', async () => {
    const { onStart } = mount(false, [
      agentInstance(3, 'cli-a', 'cli'),
      agentInstance(7, 'sdk-b', 'sdk', true),
    ]);

    await pickAgent('cli-a（cli）');
    expect(agentValue().getAttribute('data-value')).toBe('3');
    typePrompt('显式 agent 调试轮');
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onStart).toHaveBeenCalledWith({
      prompt: '显式 agent 调试轮',
      permissionMode: 'bypassPermissions',
      agent: 3,
    } satisfies AgentStartInput);
  });

  it('不显式选择直接发起 → onStart 携带默认 agent（7）（自定义选择器无清单外注入面，默认落位即兜底）', () => {
    const { onStart } = mount(false, [agentInstance(7, 'sdk-b', 'sdk', true)]);

    typePrompt('清单外 agent 轮');
    fireEvent.click(startButton());
    expect(onStart).toHaveBeenCalledWith({
      prompt: '清单外 agent 轮',
      permissionMode: 'bypassPermissions',
      agent: 7,
    } satisfies AgentStartInput);
  });
});

describe('AgentRunForm：禁用与输入保真（边界）', () => {
  it('disabled=true 时启动入口禁用且点击不触发 onStart', () => {
    const { onStart } = mount(true);

    typePrompt('你好');
    expect(startButton().disabled).toBe(true);
    fireEvent.click(startButton());

    expect(onStart).not.toHaveBeenCalled();
  });

  it('prompt 含换行 / emoji / 超长（>1000 字符）时 onStart 原样上抛', () => {
    const { onStart } = mount();
    const longPrompt = `多行 🎉\n"quoted" ${'长'.repeat(1000)}`;

    typePrompt(longPrompt);
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledWith({
      prompt: longPrompt,
      permissionMode: 'bypassPermissions',
      agent: null,
    } satisfies AgentStartInput);
  });

  it('无 cwd 输入、无 model 选择（AC-6 缺席断言，D9：cwd 由 invoke 隐含传 root）', () => {
    mount();

    expect(screen.queryByLabelText(/cwd/i)).toBeNull();
    expect(screen.queryByLabelText(/model/i)).toBeNull();
    expect(screen.queryByTestId('agent-cwd')).toBeNull();
    expect(screen.queryByTestId('agent-model')).toBeNull();
  });
});
