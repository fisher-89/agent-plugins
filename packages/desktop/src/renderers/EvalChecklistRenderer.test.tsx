import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ArtifactEnvelope } from '../types/dto';
import { EvalChecklistRenderer } from './EvalChecklistRenderer';

function envelope(payload: unknown, fallbackText: string | null = null): ArtifactEnvelope {
  return { kind: 'eval-checklist', version: 1, title: '评估清单', payload, fallbackText };
}

const fullPayload = {
  phase: 'proposal',
  attempt: 2,
  verdict: 'pass',
  items: [
    { item: '问题描述清晰', pass: true, evidence: 'L11-23 含痛点与动机' },
    { item: '范围明确', pass: false, evidence: '缺删除清单' },
  ],
};

const validItem = { item: '问题描述清晰', pass: true, evidence: 'L11-23 含痛点与动机' };

/** 断言 payload 被拒收：以保底 <pre> 呈现（fallback-text 挂钩）且未渲染清单区块。 */
function expectFallback(text: string): void {
  expect(screen.getByTestId('fallback-text').textContent).toBe(text);
  expect(screen.queryByTestId('checklist')).toBeNull();
}

describe('EvalChecklistRenderer：items（item / pass / evidence）清单渲染', () => {
  it('items 渲染为清单，每条含 item 名与 evidence', () => {
    render(<EvalChecklistRenderer envelope={envelope(fullPayload)} />);
    expect(screen.getByText('问题描述清晰') !== null).toBe(true);
    expect(screen.getByText('L11-23 含痛点与动机') !== null).toBe(true);
    expect(screen.getByText('范围明确') !== null).toBe(true);
    expect(screen.getByText('缺删除清单') !== null).toBe(true);
  });

  it('pass true / false 条目有可区分的标识文案', () => {
    render(<EvalChecklistRenderer envelope={envelope(fullPayload)} />);
    // 在 checklist 域内收集 checklist-verdict 徽标文本（verdict 徽标在 attempt-meta 域，不入此列）
    const badgeTexts = within(screen.getByTestId('checklist'))
      .getAllByTestId('checklist-verdict')
      .map((badge) => badge.textContent ?? '');
    expect(badgeTexts).toContain('pass');
    expect(badgeTexts).toContain('fail');
  });

  it('items 为空数组时渲染空清单不崩（回退保底文本）', () => {
    render(
      <EvalChecklistRenderer
        envelope={envelope({ ...fullPayload, items: [] }, '- [x] 保底清单文本')}
      />,
    );
    expect(screen.getByText('- [x] 保底清单文本') !== null).toBe(true);
  });

  it('attempt 为 null 时不出现 NaN / undefined 字样', () => {
    const { container } = render(
      <EvalChecklistRenderer envelope={envelope({ ...fullPayload, attempt: null })} />,
    );
    expect(container.textContent).not.toContain('NaN');
    expect(container.textContent).not.toContain('undefined');
    expect(container.textContent).toContain('phase: proposal');
  });

  it('payload 缺 items 字段时不抛错（回退空态）', () => {
    expect(() =>
      render(<EvalChecklistRenderer envelope={envelope({ phase: 'x' })} />),
    ).not.toThrow();
    expect(screen.getByText('（清单为空）') !== null).toBe(true);
  });
});

describe('EvalChecklistRenderer：payload 收窄、统计与降级分支', () => {
  it('通过计数按 pass 条目数统计而非条目总数', () => {
    const { container } = render(<EvalChecklistRenderer envelope={envelope(fullPayload)} />);
    expect(container.textContent).toContain('1/2 通过');
    expect(container.textContent).not.toContain('2/2 通过');
    expect(container.textContent).not.toContain('0/2 通过');
  });

  it('attempt 有值时展示编号，为 null 时整段隐藏', () => {
    const withAttempt = render(<EvalChecklistRenderer envelope={envelope(fullPayload)} />);
    expect(withAttempt.container.textContent).toContain('attempt: 2');

    const without = render(
      <EvalChecklistRenderer envelope={envelope({ ...fullPayload, attempt: null })} />,
    );
    expect(without.container.textContent).not.toContain('attempt:');
  });

  it('verdict 徽标三态：pass / fail / null 不渲染徽标', () => {
    const pass = render(<EvalChecklistRenderer envelope={envelope(fullPayload)} />);
    expect(within(pass.container).getByTestId('attempt-verdict').textContent).toBe('pass');
    pass.unmount();

    const fail = render(
      <EvalChecklistRenderer envelope={envelope({ ...fullPayload, verdict: 'fail' })} />,
    );
    expect(within(fail.container).getByTestId('attempt-verdict').textContent).toBe('fail');
    fail.unmount();

    const none = render(
      <EvalChecklistRenderer envelope={envelope({ ...fullPayload, verdict: null })} />,
    );
    expect(within(none.container).queryByTestId('attempt-verdict')).toBeNull();
  });

  it('条目徽标按 pass / fail 计数且归属域正确', () => {
    const { container } = render(<EvalChecklistRenderer envelope={envelope(fullPayload)} />);
    // verdict(pass) + 第 1 条 item(pass) → 2 个 pass 徽标；第 2 条 item(fail) → 1 个 fail 徽标
    const verdictTexts = [
      ...within(container).getAllByTestId('attempt-verdict'),
      ...within(container).getAllByTestId('checklist-verdict'),
    ].map((badge) => badge.textContent ?? '');
    expect(verdictTexts.filter((text) => text === 'pass')).toHaveLength(2);
    expect(verdictTexts.filter((text) => text === 'fail')).toHaveLength(1);
    // checklist 域内的条目徽标依次为 pass / fail
    const checklistTexts = within(container)
      .getAllByTestId('checklist-verdict')
      .map((badge) => badge.textContent ?? '');
    expect(checklistTexts).toEqual(['pass', 'fail']);
  });

  it('items 为空数组且无保底文本时渲染（清单为空）占位', () => {
    render(<EvalChecklistRenderer envelope={envelope({ ...fullPayload, items: [] })} />);
    expectFallback('（清单为空）');
  });

  it('payload 非对象（null / 原始值）时回退保底形态且不崩溃', () => {
    for (const bad of [null, 'text', 42, true]) {
      const { unmount } = render(<EvalChecklistRenderer envelope={envelope(bad)} />);
      expectFallback('（清单为空）');
      unmount();
    }
  });

  it('phase / attempt / verdict 类型漂移时拒收 payload', () => {
    const cases: unknown[] = [
      { ...fullPayload, phase: 42 },
      { ...fullPayload, attempt: '2' },
      { ...fullPayload, attempt: undefined },
      { ...fullPayload, verdict: 42 },
    ];
    for (const bad of cases) {
      const { unmount } = render(<EvalChecklistRenderer envelope={envelope(bad)} />);
      expectFallback('（清单为空）');
      unmount();
    }
  });

  it('缺失 phase 字段时拒收 payload', () => {
    const noPhase = { attempt: 2, verdict: 'pass', items: fullPayload.items };
    render(<EvalChecklistRenderer envelope={envelope(noPhase)} />);
    expectFallback('（清单为空）');
  });

  it('verdict 与 attempt 为 null 属合法 payload 并正常渲染清单', () => {
    const { container } = render(
      <EvalChecklistRenderer
        envelope={envelope({
          phase: 'implement',
          attempt: null,
          verdict: null,
          items: fullPayload.items,
        })}
      />,
    );
    expect(within(container).getByTestId('checklist') !== null).toBe(true);
    expect(container.textContent).toContain('phase: implement');
    expect(container.textContent).toContain('1/2 通过');
  });

  it('items 非数组时拒收且不崩溃', () => {
    for (const bad of [{}, 'items', 3]) {
      const payload = { phase: 'proposal', attempt: 1, verdict: 'pass', items: bad };
      const { unmount } = render(<EvalChecklistRenderer envelope={envelope(payload)} />);
      expectFallback('（清单为空）');
      unmount();
    }
  });

  it('items 含非法条目（item / pass / evidence 类型漂移或非对象）时整批拒收', () => {
    const invalidEntries: unknown[] = [
      { item: 42, pass: true, evidence: 'ok' },
      { item: 'x', pass: 'yes', evidence: 'ok' },
      { item: 'x', pass: true, evidence: 42 },
      { item: 'x', pass: true },
      null,
      'entry',
    ];
    for (const bad of invalidEntries) {
      const payload = { phase: 'proposal', attempt: 1, verdict: 'pass', items: [validItem, bad] };
      const { unmount } = render(<EvalChecklistRenderer envelope={envelope(payload)} />);
      expectFallback('（清单为空）');
      unmount();
    }
  });
});
