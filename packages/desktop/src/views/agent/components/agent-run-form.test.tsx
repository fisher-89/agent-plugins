import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { AgentPermissionMode } from '../../../types/dto';
import { AgentRunForm } from './agent-run-form';

// AgentRunForm 为纯回调组件（onStart 以 vi.fn() 注入），无进程边界，不需要 Mock。
// AgentStartInput 形状由 onStart 入参类型承载（'./agent-run-form' 同源导出）。

type AgentStartInput = Parameters<React.ComponentProps<typeof AgentRunForm>['onStart']>[0];

function mount(disabled = false) {
  const onStart = vi.fn();
  render(<AgentRunForm disabled={disabled} onStart={onStart} />);
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
  it('填写后点击启动 → onStart 以 { prompt, permissionMode, engine } 恰调用一次（初始 engine=sdk）', () => {
    const { onStart } = mount();

    typePrompt('帮我跑一轮');
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onStart).toHaveBeenCalledWith({
      prompt: '帮我跑一轮',
      permissionMode: 'bypassPermissions',
      engine: 'sdk',
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

describe('AgentRunForm：引擎二值下拉（调试页引擎选择，初始 sdk 与后端默认一致）', () => {
  it('初始 agent-engine 下拉值为 sdk（对齐 permission-mode 默认档断言形态，与后端 DEFAULT_ENGINE 一致）', () => {
    mount();

    expect(selectOf('agent-engine').value).toBe('sdk');
    // 可选面：cli / sdk 二值
    const values = Array.from(selectOf('agent-engine').options).map((option) => option.value);
    expect(values).toEqual(['cli', 'sdk']);
  });

  it('切至 cli 后启动 → onStart 以 { prompt, permissionMode, engine: "cli" } 恰调用一次', () => {
    const { onStart } = mount();

    fireEvent.change(selectOf('agent-engine'), { target: { value: 'cli' } });
    expect(selectOf('agent-engine').value).toBe('cli');
    typePrompt('显式 cli 调试轮');
    fireEvent.click(startButton());

    expect(onStart).toHaveBeenCalledTimes(1);
    expect(onStart).toHaveBeenCalledWith({
      prompt: '显式 cli 调试轮',
      permissionMode: 'bypassPermissions',
      engine: 'cli',
    } satisfies AgentStartInput);
  });

  it('注入清单外 option 值（yolo）→ 不回填不触发 onChange，onStart 不携带非法 engine', () => {
    const { onStart } = mount();

    // ModeSelect 清单守卫：select 值被程序性改为清单外值时 onChange 不回填
    // （fireEvent.change 携带 ENGINE_OPTIONS 之外的值，options.find 不命中）
    fireEvent.change(selectOf('agent-engine'), { target: { value: 'yolo' } });
    expect(selectOf('agent-engine').value).toBe('sdk');

    typePrompt('清单外引擎轮');
    fireEvent.click(startButton());
    expect(onStart).toHaveBeenCalledWith({
      prompt: '清单外引擎轮',
      permissionMode: 'bypassPermissions',
      engine: 'sdk',
    } satisfies AgentStartInput);
    expect(onStart.mock.calls[0]?.[0]).not.toHaveProperty('engine', 'yolo');
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
      engine: 'sdk',
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
