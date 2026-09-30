import { fireEvent, render, screen } from '@testing-library/react';
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
  it('空清单：agent-select 仅存缺省项且值为缺省（发起走后端缺省解析）', () => {
    mount();

    expect(selectOf('agent-select').value).toBe('');
    const values = Array.from(selectOf('agent-select').options).map((option) => option.value);
    expect(values).toEqual(['']);
  });

  it('清单含默认 agent：初始即选中默认 agent（isDefault 记录 id）', () => {
    mount(false, [agentInstance(3, 'cli-a', 'cli'), agentInstance(7, 'sdk-b', 'sdk', true)]);

    expect(selectOf('agent-select').value).toBe('7');
    const values = Array.from(selectOf('agent-select').options).map((option) => option.value);
    expect(values).toEqual(['', '3', '7']);
  });

  it('切至显式 agent 后启动 → onStart 以 { prompt, permissionMode, agent: 3 } 恰调用一次', () => {
    const { onStart } = mount(false, [
      agentInstance(3, 'cli-a', 'cli'),
      agentInstance(7, 'sdk-b', 'sdk', true),
    ]);

    fireEvent.change(selectOf('agent-select'), { target: { value: '3' } });
    expect(selectOf('agent-select').value).toBe('3');
    typePrompt('显式 agent 调试轮');
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onStart).toHaveBeenCalledWith({
      prompt: '显式 agent 调试轮',
      permissionMode: 'bypassPermissions',
      agent: 3,
    } satisfies AgentStartInput);
  });

  it('程序性注入清单外 option 值（yolo）→ 不炸、发起不携带非法 agent（jsdom 归一缺省项）', () => {
    const { onStart } = mount(false, [agentInstance(7, 'sdk-b', 'sdk', true)]);

    // 浏览器 select 不可能产生清单外选中值（守卫为 defense-in-depth）；jsdom
    // 对无匹配 option 的 select.value 归一为 ''（缺省项），组件按缺省语义承接
    fireEvent.change(selectOf('agent-select'), { target: { value: 'yolo' } });

    typePrompt('清单外 agent 轮');
    fireEvent.click(startButton());
    expect(onStart).toHaveBeenCalledWith({
      prompt: '清单外 agent 轮',
      permissionMode: 'bypassPermissions',
      agent: null,
    } satisfies AgentStartInput);
    expect(onStart.mock.calls[0]?.[0]).not.toHaveProperty('agent', 'yolo');
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
