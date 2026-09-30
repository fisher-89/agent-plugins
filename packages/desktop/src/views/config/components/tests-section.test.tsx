// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { TestSuite } from '../../../types/generated/bindings';
import { TestsSection } from './tests-section';

// ---------------------------------------------------------------------------
// TestsSection 单测（AC-5）：纯组件 fixture 直传入参渲染断言，无 mock。阈值
// 「未设（默认 N）」标注以 defaultedPaths（path 形如 tests[i].coverage.lines）
// 对位驱动，卡片以 data-testid="config-suite" + data-suite-root 定位。
// ---------------------------------------------------------------------------

function makeSuite(overrides: Partial<TestSuite> = {}, index = 0): TestSuite {
  return {
    root: `pkg-${index}`,
    framework: 'vite-plus',
    cwd: `pkg-${index}`,
    config: null,
    includes: null,
    excludes: null,
    coverage: { lines: 80, branches: 70, functions: 75 },
    mutation: { cwd: null, score: 70 },
    ...overrides,
  };
}

describe('TestsSection：tests 面板逐 suite 卡片与阈值标注对位（AC-5）', () => {
  it('多 suite 逐卡片呈现全字段（config-suite 计数与 fixture 一致，字段值逐字承载）', () => {
    const suites = [
      makeSuite(
        {
          root: 'packages/desktop',
          framework: 'vite-plus',
          cwd: 'packages/desktop',
          includes: ['src/**/*.test.tsx'],
        },
        0,
      ),
      makeSuite(
        {
          root: 'src-tauri',
          framework: 'rust',
          config: 'Cargo.toml',
          excludes: ['target/**'],
          coverage: { lines: 90, branches: 88, functions: 85 },
          mutation: { cwd: 'src-tauri', score: 75 },
        },
        1,
      ),
    ];

    render(<TestsSection suites={suites} defaultedPaths={new Set()} />);

    expect(screen.getByTestId('config-tests') !== null).toBe(true);
    const cards = screen.getAllByTestId('config-suite');
    expect(cards).toHaveLength(2);
    expect(cards[0].getAttribute('data-suite-root')).toBe('packages/desktop');
    expect(cards[1].getAttribute('data-suite-root')).toBe('src-tauri');
    expect(screen.getByText('vite-plus') !== null).toBe(true);
    expect(screen.getByText('rust') !== null).toBe(true);
    expect(within(cards[0]).getByText('src/**/*.test.tsx') !== null).toBe(true);
    expect(within(cards[1]).getByText('Cargo.toml') !== null).toBe(true);
    expect(within(cards[1]).getByText('target/**') !== null).toBe(true);
    // 三阈值 + mutation score 逐字承载（「label 值」行形态）
    expect(cards[0].textContent).toContain('lines 80');
    expect(cards[0].textContent).toContain('branches 70');
    expect(cards[0].textContent).toContain('functions 75');
    expect(cards[0].textContent).toContain('score 70');
    expect(cards[1].textContent).toContain('lines 90');
    expect(cards[1].textContent).toContain('branches 88');
    expect(cards[1].textContent).toContain('functions 85');
    expect(cards[1].textContent).toContain('score 75');
    expect(cards[1].textContent).toContain('cwd src-tauri');
  });

  it('阈值标注索引对位：tests[0].coverage.lines 标注落在第一卡片，tests[1] 同字段不受波及', () => {
    const suites = [makeSuite({ root: 'alpha' }, 0), makeSuite({ root: 'beta' }, 1)];
    const defaulted = new Set(['tests[0].coverage.lines', 'tests[0].mutation.score']);

    render(<TestsSection suites={suites} defaultedPaths={defaulted} />);

    const cards = screen.getAllByTestId('config-suite');
    expect(within(cards[0]).getAllByText('未设（默认 80）')).toHaveLength(1);
    expect(within(cards[0]).getByText('未设（默认 70）') !== null).toBe(true);
    expect(within(cards[1]).queryByText(/未设（默认/)).toBeNull();
  });

  it('suites 空数组：config-tests 在场呈弱化空行、无 config-suite', () => {
    render(<TestsSection suites={[]} defaultedPaths={new Set()} />);

    expect(screen.getByTestId('config-tests') !== null).toBe(true);
    expect(screen.getByText('当前工作区未配置测试 suite。') !== null).toBe(true);
    expect(screen.queryAllByTestId('config-suite')).toHaveLength(0);
  });

  it('includes / excludes / config / mutation.cwd 为 None：对位「未配置」占位不崩', () => {
    render(<TestsSection suites={[makeSuite({ root: 'nulls' })]} defaultedPaths={new Set()} />);

    expect(screen.getAllByTestId('config-suite')).toHaveLength(1);
    // config 行 + includes 行 + excludes 行 + mutation.cwd 内联各一处占位
    expect(screen.getAllByText('未配置')).toHaveLength(4);
  });

  it('阈值 number | null 出线口径的 null 值：?? 防御呈现不崩', () => {
    render(
      <TestsSection
        suites={[
          makeSuite({
            root: 'null-thresholds',
            coverage: { lines: null, branches: null, functions: null },
          }),
        ]}
        defaultedPaths={new Set()}
      />,
    );

    const card = screen.getByTestId('config-suite');
    expect(card.textContent).toContain('lines —');
    expect(card.textContent).toContain('branches —');
    expect(card.textContent).toContain('functions —');
  });

  it('30 suite 全量渲染无丢失、config-suite 计数一致', () => {
    const suites = Array.from({ length: 30 }, (_, index) => makeSuite({}, index));

    render(<TestsSection suites={suites} defaultedPaths={new Set()} />);

    expect(screen.getAllByTestId('config-suite')).toHaveLength(30);
  });
});
