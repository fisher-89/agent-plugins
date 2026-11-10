import { act, fireEvent, render, screen } from '@testing-library/react';
import { cn } from 'cn';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { buttonVariants } from '@/components/ui/button';
import { CodeBlockCopyButton } from '@/components/ui/code-block';

import type { ChangeStepStatus } from '../../../types/dto';
import { StdoutPanel } from './stdout-panel';

// ---------------------------------------------------------------------------
// StdoutPanel 单测（change-detail-live-stdout / AC-8 / AC-9）：三段结构（命令头行
// 在最上方 + 状态提示 + 文本区）、三态按 `status` 与空文本分派、粘底跟随、
// 渲染前 `\r` 归一。
//
// 进程边界 Mock（Mock策略表）：`invokeMock` 按命令名分发三命令；`navigator.clipboard`
// 与滚动三指标为**运行环境能力替身**（jsdom 无 layout 引擎）；`use-exec-output.ts`
// / `stdout-text.ts` / `CodeBlockCopyButton` 真实组合。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    public onmessage: ((message: unknown) => void) | null = null;
    public static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
    public deliver(): void {
      this.onmessage?.({});
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

const ROOT = 'C:\\demo\\flow';
const CHANGE_NAME = 'add-feature';
const EXEC_ID = 'exec-1727000000000-3';

/** 增量读应答（`{ text, nextOffset }` = 命中、`null` = miss、`Error` = reject）。 */
type DeltaAnswer = { text: string; nextOffset: number } | null | Error;

let deltaAnswer: DeltaAnswer = { text: 'output\n', nextOffset: 7 };
let commandAnswer: string | null | Error = 'pnpm run check 2>&1';
let pathAnswer: string | null = `${ROOT}\\reports\\test\\exec-outputs\\${EXEC_ID}.log`;
let clipboardMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  deltaAnswer = { text: 'output\n', nextOffset: 7 };
  commandAnswer = 'pnpm run check 2>&1';
  pathAnswer = `${ROOT}\\reports\\test\\exec-outputs\\${EXEC_ID}.log`;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'read_exec_command') {
      return commandAnswer instanceof Error
        ? Promise.reject(commandAnswer.message)
        : Promise.resolve(commandAnswer);
    }
    if (command === 'read_exec_output_delta') {
      return deltaAnswer instanceof Error
        ? Promise.reject(deltaAnswer.message)
        : Promise.resolve(deltaAnswer);
    }
    if (command === 'exec_output_path') return Promise.resolve(pathAnswer);
    if (command === 'watch_subscribe') return Promise.resolve(1);
    if (command === 'watch_unsubscribe') return Promise.resolve(true);
    return Promise.resolve(null);
  });
  clipboardMock = vi.fn(() => Promise.resolve());
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: clipboardMock },
  });
});

afterEach(() => {
  // 计时器复位（Mock策略表「计时器」行）：本文件六处 `vi.useFakeTimers()`（粘底
  // 三处 + 复制按钮复位三处）各自开启假计时器，若不在此复位则泄漏到后续用例
  // （既有的 500 ms 防抖推进与 2 s 复位互相干扰），故与 `use-exec-output.test.ts`
  // 的 afterEach 同口径先 `useRealTimers()`。
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function flush(): Promise<void> {
  await act(async () => {});
}

function renderPanel(
  options: {
    root?: string | null;
    status?: ChangeStepStatus | null;
  } = {},
) {
  const { root = ROOT, status = null } = options;
  return render(
    <StdoutPanel root={root} changeName={CHANGE_NAME} execId={EXEC_ID} status={status} />,
  );
}

/** 面板三态 testid 的在场性快照。 */
function stateIds(): { text: boolean; empty: boolean; running: boolean; error: boolean } {
  return {
    text: screen.queryByTestId('stdout-text') !== null,
    empty: screen.queryByTestId('stdout-empty') !== null,
    running: screen.queryByTestId('stdout-running') !== null,
    error: screen.queryByTestId('stdout-error') !== null,
  };
}

describe('StdoutPanel：命令头行', () => {
  it('正向：两段命令 → stdout-command 为面板最上方内容，文本逐字含两段且保留换行', async () => {
    const twoSegments = '\n=== plan a · rust ===\ncargo test\n=== plan b · vp ===\npnpm check\n';
    commandAnswer = twoSegments;
    renderPanel({ status: 'passed' });
    await flush();

    const command = screen.getByTestId('stdout-command');
    expect(command.tagName).toBe('PRE');
    expect(command.textContent).toBe(twoSegments);
    expect(command.className).toContain('whitespace-pre-wrap');
    // 位于面板内容最上方：其父级是面板的首个子块
    const panel = screen.getByTestId('stdout-panel');
    expect(panel.contains(command)).toBe(true);
    expect(panel.firstElementChild?.nextElementSibling).toBe(command.parentElement);
  });

  it('正向：复制按钮在位，点击后 clipboard 收到命令全文（多行原样）', async () => {
    const multiline = 'line-1\nline-2 🚀';
    commandAnswer = multiline;
    renderPanel({ status: 'passed' });
    await flush();

    const copy = screen.getByTestId('stdout-command-copy');
    fireEvent.click(copy);
    await flush();
    expect(clipboardMock).toHaveBeenCalledWith(multiline);
  });

  it('异常：read_exec_command 返回 null（老记录）→ 呈「（命令未记录）」、零复制按钮、内容面照常', async () => {
    commandAnswer = null;
    deltaAnswer = { text: 'still rendered\n', nextOffset: 15 };
    renderPanel({ status: 'passed' });
    await flush();

    expect(screen.getByTestId('stdout-command').textContent).toBe('（命令未记录）');
    expect(screen.queryByTestId('stdout-command-copy')).toBeNull();
    expect(screen.getByTestId('stdout-text').textContent).toBe('still rendered\n');
  });

  it('异常：复制按钮点击但 clipboard reject → 不 throw、不炸图、内容面不受影响', async () => {
    clipboardMock = vi.fn(() => Promise.reject(new Error('剪贴板不可用')));
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardMock },
    });
    deltaAnswer = { text: 'content\n', nextOffset: 8 };
    renderPanel({ status: 'passed' });
    await flush();

    fireEvent.click(screen.getByTestId('stdout-command-copy'));
    await flush();
    expect(screen.getByTestId('stdout-text').textContent).toBe('content\n');
    expect(screen.queryByTestId('stdout-error')).toBeNull();
  });

  it('边界：命令为空串 → 命令头行渲染空 pre、复制按钮仍在（null 与 "" 可分辨）', async () => {
    commandAnswer = '';
    renderPanel({ status: 'passed' });
    await flush();

    expect(screen.getByTestId('stdout-command').textContent).toBe('');
    expect(screen.queryByTestId('stdout-command-copy')).not.toBeNull();
  });
});

describe('StdoutPanel：三态分派', () => {
  it('正向：status=running 且空增量 → 「运行中，等待输出…」，零 text / empty', async () => {
    deltaAnswer = { text: '', nextOffset: 0 };
    renderPanel({ status: 'running' });
    await flush();

    expect(screen.getByTestId('stdout-running').textContent).toBe('运行中，等待输出…');
    expect(stateIds()).toEqual({ text: false, empty: false, running: true, error: false });
  });

  it('正向：status=passed 且 miss（null）→ 「（无输出记录）」，零 running（两文案不混同）', async () => {
    deltaAnswer = null;
    renderPanel({ status: 'passed' });
    await flush();

    expect(screen.getByTestId('stdout-empty').textContent).toBe('（无输出记录）');
    expect(stateIds()).toEqual({ text: false, empty: true, running: false, error: false });
  });

  it('异常：增量 reject → stdout-error 呈拒绝串，面板骨架与命令头行仍在', async () => {
    deltaAnswer = new Error('读输出记录失败: 磁盘不可读');
    renderPanel({ status: 'running' });
    await flush();

    expect(screen.getByTestId('stdout-error').textContent).toBe('读输出记录失败: 磁盘不可读');
    expect(screen.getByTestId('stdout-panel')).not.toBeNull();
    expect(screen.getByTestId('stdout-command').textContent).toBe('pnpm run check 2>&1');
    expect(screen.queryByTestId('stdout-text')).toBeNull();
  });

  it('异常：错误态优先于空态 —— 文本面为空时 reject 呈 stdout-error 而非「（无输出记录）」', async () => {
    deltaAnswer = new Error('读输出记录失败');
    renderPanel({ status: 'passed' });
    await flush();

    expect(screen.getByTestId('stdout-error').textContent).toBe('读输出记录失败');
    expect(screen.queryByTestId('stdout-empty')).toBeNull();
    expect(screen.queryByTestId('stdout-running')).toBeNull();
    expect(screen.queryByTestId('stdout-text')).toBeNull();
  });

  // 三态 × 文本面正交矩阵（4 × 4 = 16 组合）：分派规则单点——`status === 'running'`
  // 且空白文本 → running 提示；非 running 且空白文本 → empty 提示；其余 → text。
  // 「空白」取 `(text ?? '').trim() === ''`，故 null 与 "" 同归空白面。
  const MATRIX_STATUSES: ReadonlyArray<ChangeStepStatus | null> = [
    'running',
    'passed',
    'failed',
    null,
  ];
  const LONG_TEXT = `${'x'.repeat(1200)}
🚀`;

  /** 增量四形态（`null` miss / 空串 / 单字符 / 超长）。 */
  const MATRIX_ANSWERS: ReadonlyArray<[string, DeltaAnswer]> = [
    ['miss(null)', null],
    ['空串', { text: '', nextOffset: 0 }],
    ['单字符', { text: 'x', nextOffset: 1 }],
    ['超长', { text: LONG_TEXT, nextOffset: LONG_TEXT.length }],
  ];

  /** 期望分支（与组件三段 JSX 的分派同源，独立重述一遍以免断言自证）。 */
  function expectedBranch(status: ChangeStepStatus | null, answer: DeltaAnswer): string {
    const rawText = answer instanceof Error || answer === null ? null : answer.text;
    const empty = (rawText ?? '').trim() === '';
    if (status === 'running') return empty ? 'running' : 'text';
    return empty ? 'empty' : 'text';
  }

  const matrix: Array<[string, ChangeStepStatus | null, DeltaAnswer, string]> = [];
  for (const status of MATRIX_STATUSES) {
    for (const [answerLabel, answer] of MATRIX_ANSWERS) {
      matrix.push([
        `status=${status ?? 'null'} × 增量=${answerLabel}`,
        status,
        answer,
        expectedBranch(status, answer),
      ]);
    }
  }

  it.each(matrix)(
    '边界：%s → %s 分支（恰一个状态面在场）',
    async (_label, status, answer, expected) => {
      deltaAnswer = answer;
      renderPanel({ status });
      await flush();

      const ids = stateIds();
      expect(ids.error).toBe(false);
      expect({ text: ids.text, empty: ids.empty, running: ids.running }).toEqual({
        text: expected === 'text',
        empty: expected === 'empty',
        running: expected === 'running',
      });
      // 文本面在场时内容逐字等于入参（文本内无 CR，归一不改写）
      if (expected === 'text') {
        expect(screen.getByTestId('stdout-text').textContent).toBe(
          (answer as { text: string }).text,
        );
      }
    },
  );

  it('边界：16 组合覆盖完整（4 status × 4 增量，无重复无遗漏）', () => {
    const labels = matrix.map(([label]) => label);
    expect(labels).toHaveLength(16);
    expect(new Set(labels).size).toBe(16);
    for (const status of MATRIX_STATUSES) {
      for (const [answerLabel] of MATRIX_ANSWERS) {
        expect(labels).toContain(`status=${status ?? 'null'} × 增量=${answerLabel}`);
      }
    }
  });

  it('边界：root === null（非运行态）→ 零 invoke、呈「（无输出记录）」空态', async () => {
    // 非运行态判别：`running` 分支只由 `status` 决定，故 root 缺席 + 非运行态落空态
    renderPanel({ root: null, status: null });
    await flush();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByTestId('stdout-empty').textContent).toBe('（无输出记录）');
  });
});

describe('StdoutPanel：\\r 归一（渲染前收敛）', () => {
  it('正向：cargo 进度序列只留最后一段、其后行原样', async () => {
    deltaAnswer = {
      text: 'Compiling a\rCompiling b\rCompiling c\nnext line',
      nextOffset: 44,
    };
    renderPanel({ status: 'passed' });
    await flush();

    expect(screen.getByTestId('stdout-text').textContent).toBe('Compiling c\nnext line');
  });

  it('异常：归一不触碰其它字符 —— 含替换符 / \\0 / 制表符 / emoji 且无 CR 的文本逐字等于入参', async () => {
    // 异常面：非 UTF-8 字节被解码成替换符、NUL 与制表符都是控制字符——归一只按
    // `\r` 分段，MUST NOT 改写 / 转义 / 吞掉任何一个其它字节（AC-8 的「不堆残行」
    // 不授权做字符替换）
    const controls = 'a\tb\u0000c \uFFFDdone 🚀\nnext\tline\u0000';
    deltaAnswer = { text: controls, nextOffset: controls.length };
    renderPanel({ status: 'passed' });
    await flush();

    const rendered = screen.getByTestId('stdout-text').textContent;
    expect(rendered).toBe(controls);
    expect(rendered).toHaveLength(controls.length);
    expect(rendered?.includes('\u0000')).toBe(true);
    expect(rendered?.includes('\t')).toBe(true);
  });

  it.each([
    ['行内单 \\r', 'a\rb', 'b'],
    ['无 CR 多行', 'a\nb', 'a\nb'],
  ])('边界：%s → 归一后为该值', async (_label, input, expected) => {
    deltaAnswer = { text: input, nextOffset: input.length };
    renderPanel({ status: 'passed' });
    await flush();
    expect(screen.getByTestId('stdout-text').textContent).toBe(expected);
  });

  it.each([
    ['仅 CR', '\r'],
    ['空串', ''],
  ])('边界：%s → 面板落空态（空白文本按三态语义分派，非空 <pre>）', async (_label, input) => {
    deltaAnswer = { text: input, nextOffset: input.length };
    renderPanel({ status: 'passed' });
    await flush();
    expect(screen.getByTestId('stdout-empty')).not.toBeNull();
    expect(screen.queryByTestId('stdout-text')).toBeNull();
  });
});

describe('StdoutPanel：粘底跟随', () => {
  /** 在元素上打桩滚动三指标（jsdom 无布局引擎）+ `scrollTop` 可观测 setter。 */
  function stubScrollMetrics(
    element: HTMLElement,
    metrics: { scrollHeight: number; clientHeight: number },
    initialTop = 0,
  ): { writes: number[]; moveTo: (value: number) => void } {
    let scrollTop = initialTop;
    const writes: number[] = [];
    Object.defineProperty(element, 'scrollHeight', {
      configurable: true,
      get: () => metrics.scrollHeight,
    });
    Object.defineProperty(element, 'clientHeight', {
      configurable: true,
      get: () => metrics.clientHeight,
    });
    Object.defineProperty(element, 'scrollTop', {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => {
        scrollTop = value;
        writes.push(value);
      },
    });
    return {
      writes,
      /** 用户滚动位置变化（不经 effect setter，故不计入写入次数）。 */
      moveTo: (value: number) => {
        scrollTop = value;
      },
    };
  }

  /** 推一次 watch 信号过防抖窗口（文本面因此增长）。 */
  async function pushSignal(): Promise<void> {
    const channel = ChannelMock.instances[ChannelMock.instances.length - 1];
    expect(channel).toBeDefined();
    await act(async () => {
      channel.deliver();
    });
    await act(async () => {
      vi.advanceTimersByTime(500);
    });
    await flush();
  }

  it('正向：初始跟随（stickRef true）→ 文本变化把 scrollTop 置为 scrollHeight（写入 1 次）', async () => {
    vi.useFakeTimers();
    deltaAnswer = { text: 'line-1\n', nextOffset: 7 };
    renderPanel({ status: 'running' });
    await flush();

    const pre = screen.getByTestId('stdout-text');
    const probe = stubScrollMetrics(pre, { scrollHeight: 1200, clientHeight: 300 });
    expect(probe.writes).toEqual([]);

    await pushSignal();

    expect(screen.getByTestId('stdout-text').textContent).toBe('line-1\nline-1\n');
    expect(probe.writes).toEqual([1200]);
  });

  it('正向：用户上滑后文本增长不再写 scrollTop；回底后恢复跟随（各写入 0 / 1 次）', async () => {
    vi.useFakeTimers();
    deltaAnswer = { text: 'x\n', nextOffset: 2 };
    renderPanel({ status: 'running' });
    await flush();

    const pre = screen.getByTestId('stdout-text');
    const probe = stubScrollMetrics(pre, { scrollHeight: 1200, clientHeight: 300 });

    // 上滑：差值 1200 - 100 - 300 = 800 > 1 px 容差 → stickRef false
    probe.moveTo(100);
    act(() => {
      fireEvent.scroll(pre);
    });
    await pushSignal();
    expect(probe.writes).toEqual([]);

    // 回底：差值 0 → stickRef 恢复 true，下一次文本变化重新写入 scrollHeight
    probe.moveTo(900);
    act(() => {
      fireEvent.scroll(pre);
    });
    await pushSignal();
    expect(probe.writes).toEqual([1200]);
  });

  it('异常：容器 ref 为 null（文本区未渲染）→ effect 早返、零异常、零 scrollTop 写入', async () => {
    deltaAnswer = { text: '', nextOffset: 0 };
    renderPanel({ status: 'passed' });
    await flush();

    expect(screen.queryByTestId('stdout-text')).toBeNull();
    expect(screen.getByTestId('stdout-empty')).not.toBeNull();
    expect(screen.getByTestId('stdout-panel')).not.toBeNull();
  });

  it.each([
    ['差值 1 px（跟随）', 899, [1200]],
    ['差值 2 px（不跟随）', 898, []],
  ] as const)(
    '边界：%s → 文本变化后 scrollTop 写入值为 %j',
    async (_label, scrollTop, expected) => {
      vi.useFakeTimers();
      deltaAnswer = { text: 'content\n', nextOffset: 8 };
      renderPanel({ status: 'running' });
      await flush();

      const pre = screen.getByTestId('stdout-text');
      const probe = stubScrollMetrics(pre, { scrollHeight: 1200, clientHeight: 300 });
      probe.moveTo(scrollTop);
      act(() => {
        fireEvent.scroll(pre);
      });

      await pushSignal();

      expect(probe.writes).toEqual([...expected]);
    },
  );
});

// ---------------------------------------------------------------------------
// `components/ui/code-block-copy-button.tsx` 的复制面（change-detail-live-stdout /
// AC-9）：该窄模块是 `CodeBlockCopyButton` 从 `components/ui/code-block.tsx` 的**逐字
// 搬迁**（零行为变更），唯一生产消费方即本面板的命令头行 ⇒ 其测试路径就是本章节
//（**不新建测试文件**，见 test-design「测试布局决策」第 7 条）。经面板真实渲染路径
// 驱动：`navigator.clipboard` 打桩为运行环境能力替身（jsdom 无 Clipboard API）。
// ---------------------------------------------------------------------------
describe('CodeBlockCopyButton（经命令头行真实渲染路径驱动）', () => {
  /** 面板命令头行复制按钮（窄模块的唯一生产挂载点）。 */
  function copyButton(): HTMLElement {
    return screen.getByTestId('stdout-command-copy');
  }

  /** `sr-only` 播报行文本（`aria-live="polite"` 的单一文本节点，随按钮同挂）。 */
  function announcement(): string {
    const live = document.querySelector('[aria-live="polite"]');
    return live?.textContent ?? '';
  }

  it('正向：多行命令（含 \\n 与 emoji）→ 点击后 clipboard 收到全文逐字（零截断零转义）', async () => {
    const multiline = 'line-1 🚀\nline-2\nline-3';
    commandAnswer = multiline;
    renderPanel({ status: 'passed' });
    await flush();

    fireEvent.click(copyButton());
    await flush();

    expect(clipboardMock).toHaveBeenCalledTimes(1);
    expect(clipboardMock).toHaveBeenCalledWith(multiline);
  });

  it('正向：点击成功 → data-copied 在位、aria-label 由 Copy 转 Copied、播报文本为 Copied', async () => {
    renderPanel({ status: 'passed' });
    await flush();

    expect(copyButton().getAttribute('aria-label')).toBe('Copy');
    expect(copyButton().getAttribute('data-copied')).toBeNull();
    expect(announcement()).toBe('');

    fireEvent.click(copyButton());
    await flush();

    expect(copyButton().getAttribute('data-copied')).toBe('true');
    expect(copyButton().getAttribute('aria-label')).toBe('Copied');
    expect(announcement()).toBe('Copied');
  });

  it('异常：clipboard.writeText reject → 静默（零抛错、零 stdout-error、data-copied 不置位、内容面照常）', async () => {
    clipboardMock = vi.fn(() => Promise.reject(new Error('剪贴板不可用')));
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardMock },
    });
    deltaAnswer = { text: 'content\n', nextOffset: 8 };
    renderPanel({ status: 'passed' });
    await flush();

    fireEvent.click(copyButton());
    await flush();

    expect(clipboardMock).toHaveBeenCalledTimes(1);
    expect(copyButton().getAttribute('data-copied')).toBeNull();
    expect(copyButton().getAttribute('aria-label')).toBe('Copy');
    expect(announcement()).toBe('');
    expect(screen.queryByTestId('stdout-error')).toBeNull();
    expect(screen.getByTestId('stdout-text').textContent).toBe('content\n');
  });

  it('异常：2 s 窗口内连续两次点击 → writeText 调用 2 次，且复位计时不被重置', async () => {
    vi.useFakeTimers();
    renderPanel({ status: 'passed' });
    await flush();

    fireEvent.click(copyButton());
    await flush();
    expect(copyButton().getAttribute('data-copied')).toBe('true');

    // 第二次点击（仍在首次成功的 2 s 窗口内）：每次都发起写入，不因已在 Copied 态而短路
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    fireEvent.click(copyButton());
    await flush();
    expect(clipboardMock).toHaveBeenCalledTimes(2);

    // `copied` 由 true 再置 true ⇒ React 不重跑 effect ⇒ 计时以首次成功为零点
    await act(async () => {
      vi.advanceTimersByTime(999);
    });
    expect(copyButton().getAttribute('data-copied')).toBe('true');
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(copyButton().getAttribute('data-copied')).toBeNull();
    expect(copyButton().getAttribute('aria-label')).toBe('Copy');
  });

  it.each([
    ['空串', ''],
    ['超长（>1000 chars）', 'x'.repeat(1200)],
    ['含 \\n 与 \\0 与 emoji', 'a\nb\u0000c 🚀'],
  ])('边界：value 取%s → 原样写入（空串亦发起一次 writeText）', async (_label, value) => {
    commandAnswer = value;
    renderPanel({ status: 'passed' });
    await flush();

    fireEvent.click(copyButton());
    await flush();

    expect(clipboardMock).toHaveBeenCalledTimes(1);
    expect(clipboardMock).toHaveBeenCalledWith(value);
  });

  it('边界：command === null（老记录）→ 复制按钮零渲染（判据在面板侧，窄模块零内部判空）', async () => {
    commandAnswer = null;
    renderPanel({ status: 'passed' });
    await flush();

    expect(screen.queryByTestId('stdout-command-copy')).toBeNull();
    expect(screen.getByTestId('stdout-command').textContent).toBe('（命令未记录）');
  });

  it.each([
    ['1999 ms 仍在 Copied', 1999, 'true'],
    ['2000 ms 复位为 Copy', 2000, null],
  ] as const)('边界：点击后推进 %s', async (_label, elapsed, expected) => {
    vi.useFakeTimers();
    renderPanel({ status: 'passed' });
    await flush();

    fireEvent.click(copyButton());
    await flush();
    expect(copyButton().getAttribute('data-copied')).toBe('true');

    await act(async () => {
      vi.advanceTimersByTime(elapsed);
    });

    expect(copyButton().getAttribute('data-copied')).toBe(expected);
    expect(copyButton().getAttribute('aria-label')).toBe(expected === null ? 'Copy' : 'Copied');
    expect(announcement()).toBe(expected === null ? '' : 'Copied');
  });

  it('边界：复位前卸载 → 计时被 cleanup 清除，零「卸载后 setState」告警', async () => {
    vi.useFakeTimers();
    const warn = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { unmount } = renderPanel({ status: 'passed' });
    await flush();

    fireEvent.click(copyButton());
    await flush();
    unmount();
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(warn).not.toHaveBeenCalled();
  });

  it('正向：透传面 —— data-testid 落在 Button 上、data-slot 定式在位', async () => {
    renderPanel({ status: 'passed' });
    await flush();

    const button = copyButton();
    expect(button.tagName).toBe('BUTTON');
    expect(button.getAttribute('data-slot')).toBe('code-block-copy-button');
    expect(button.getAttribute('aria-label')).toBe('Copy');
  });

  // props 相位（test-design row「透传面」）：缺省 `variant` / `size` 与显式 `onClick`
  // 的先后序只能在 props 面观测（面板不传这两项）——故在本文件内直渲窄模块本身
  //（真实组件、真实 `Button` / `cn` / `lucide-react`，零替身；本文件即该模块的
  // 指定测试路径，见「测试布局决策」第 7 条）。
  it('正向：未显式传 variant / size 时取缺省 ghost / icon-xs', async () => {
    const { container } = render(<CodeBlockCopyButton value="cmd" />);

    const button = container.querySelector('[data-slot="code-block-copy-button"]');
    expect(button).not.toBeNull();
    // 与 `buttonVariants` 的缺省组合逐字相等（不把类名钉成字面量副本）
    expect(button?.className).toBe(
      cn(buttonVariants({ variant: 'ghost', size: 'icon-xs', className: '' })),
    );
    // 直渲窄模块：面板不参与（本章其余用例走面板真实渲染路径）
    expect(screen.queryByTestId('stdout-panel')).toBeNull();
  });

  it('正向：显式 onClick 与内部剪贴板写入两次都发生（内部先调 onClick）', async () => {
    const order: string[] = [];
    const onClick = vi.fn(() => order.push('props-onClick'));
    clipboardMock = vi.fn(() => {
      order.push('clipboard');
      return Promise.resolve();
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardMock },
    });

    const { container } = render(
      <CodeBlockCopyButton value="cmd-value" onClick={onClick} data-testid="direct-copy" />,
    );
    fireEvent.click(screen.getByTestId('direct-copy'));
    await flush();

    expect(onClick).toHaveBeenCalledTimes(1);
    expect(clipboardMock).toHaveBeenCalledTimes(1);
    expect(clipboardMock).toHaveBeenCalledWith('cmd-value');
    expect(order).toEqual(['props-onClick', 'clipboard']);
    expect(container.querySelector('[data-copied="true"]')).not.toBeNull();
  });
});

describe('StdoutPanel：既有回归（超长 / 特殊字符 / 空串三态化）', () => {
  it('正向：超长输出（>1000 chars）完整渲染不截断，含 emoji 与替换符逐字呈现', async () => {
    const long = `${'x'.repeat(1200)}\n🚀 \uFFFD 尾行标记`;
    deltaAnswer = { text: long, nextOffset: long.length };
    renderPanel({ status: 'passed' });
    await flush();

    const pre = screen.getByTestId('stdout-text');
    expect(pre.textContent).toBe(long);
    expect(pre.textContent?.length).toBe(long.length);
  });

  it('边界：空串按三态语义分派（running → stdout-running、passed → stdout-empty）', async () => {
    deltaAnswer = { text: '', nextOffset: 0 };
    renderPanel({ status: 'running' });
    await flush();
    expect(screen.getByTestId('stdout-running')).not.toBeNull();
    expect(screen.queryByTestId('stdout-empty')).toBeNull();
  });
});
