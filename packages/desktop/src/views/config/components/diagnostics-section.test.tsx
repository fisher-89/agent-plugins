// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ConfigDiagnostic } from '../../../types/generated/bindings';
import { DiagnosticsSection } from './diagnostics-section';

// ---------------------------------------------------------------------------
// DiagnosticsSection 单测（AC-5）：纯组件直传入参渲染断言，无进程边界、无 mock。
// kind 视觉分级以 className 拓扑断言（invalidValue 强调 text-fail /
// defaultApplied 弱化 text-muted-foreground），条目以 testid 定位。
// ---------------------------------------------------------------------------

function diag(kind: ConfigDiagnostic['kind'], path: string, message: string): ConfigDiagnostic {
  return { kind, path, message };
}

function itemByPath(path: string): HTMLElement {
  const hit = screen
    .getAllByTestId('config-diagnostic-item')
    .find((item) => item.textContent?.includes(path));
  if (!hit) throw new Error(`path 为 ${path} 的诊断条目不存在`);
  return hit;
}

describe('DiagnosticsSection：diagnostics 警示区单列与 kind 视觉分级（AC-5）', () => {
  it('invalidValue 条目呈现 path 与 message 且强调色分级', () => {
    render(
      <DiagnosticsSection
        diagnostics={[
          diag(
            'invalidValue',
            'tests[0].root',
            'suite root 必须为非空字符串且不含 glob 通配符（原值 "bad*"），该 suite 已被剔除。',
          ),
        ]}
      />,
    );

    expect(screen.getByTestId('config-diagnostics') !== null).toBe(true);
    const item = screen.getByTestId('config-diagnostic-item');
    expect(item.getAttribute('data-diagnostic-kind')).toBe('invalidValue');
    expect(item.className).toContain('text-fail');
    expect(item.className).not.toContain('text-muted-foreground');
    expect(item.textContent).toContain('tests[0].root');
    expect(item.textContent).toContain('suite root 必须为非空字符串且不含 glob 通配符');
  });

  it('defaultApplied 条目弱化呈现，与 invalidValue 视觉可分，path 与中文文案逐字承载', () => {
    render(
      <DiagnosticsSection
        diagnostics={[
          diag(
            'defaultApplied',
            'tests[0].coverage.lines',
            '覆盖率阈值 lines 未设置，使用默认值 80。',
          ),
          diag(
            'invalidValue',
            'schema',
            'schema 字段必须为字面量 "spec-driven"（原值 "x"），使用默认值。',
          ),
        ]}
      />,
    );

    const defaulted = itemByPath('tests[0].coverage.lines');
    expect(defaulted.getAttribute('data-diagnostic-kind')).toBe('defaultApplied');
    expect(defaulted.className).toContain('text-muted-foreground');
    expect(defaulted.className).not.toContain('text-fail');
    expect(defaulted.textContent).toContain('覆盖率阈值 lines 未设置，使用默认值 80。');

    // 同区两种 kind 视觉可分：class 互异
    const invalid = itemByPath('schema');
    expect(invalid.className).not.toBe(defaulted.className);
  });

  it('空数组：整区不渲染（config-diagnostics 不在场）', () => {
    const { container } = render(<DiagnosticsSection diagnostics={[]} />);

    expect(screen.queryByTestId('config-diagnostics')).toBeNull();
    expect(screen.queryAllByTestId('config-diagnostic-item')).toHaveLength(0);
    expect(container.textContent).toBe('');
  });

  it('混合 kind 列表逐条渲染条数一致，20+ 条全量渲染无丢失', () => {
    const many = Array.from({ length: 24 }, (_, index) =>
      diag(
        index % 2 === 0 ? 'invalidValue' : 'defaultApplied',
        `tests[${index}].root`,
        `第 ${index} 条诊断`,
      ),
    );

    render(<DiagnosticsSection diagnostics={many} />);

    expect(screen.getAllByTestId('config-diagnostics')).toHaveLength(1);
    const items = screen.getAllByTestId('config-diagnostic-item');
    expect(items).toHaveLength(24);
    for (const [index, item] of items.entries()) {
      expect(item.getAttribute('data-diagnostic-kind')).toBe(
        index % 2 === 0 ? 'invalidValue' : 'defaultApplied',
      );
      expect(item.textContent).toContain(`tests[${index}].root`);
    }
  });

  it('path 两形态原样呈现：文件级 $ 与点路径 tests[0].coverage.lines', () => {
    render(
      <DiagnosticsSection
        diagnostics={[
          diag('invalidValue', '$', '工作区配置顶层必须为对象（原值 数组），呈现全部默认值。'),
          diag(
            'defaultApplied',
            'tests[0].coverage.lines',
            '覆盖率阈值 lines 未设置，使用默认值 80。',
          ),
        ]}
      />,
    );

    const fileLevel = itemByPath('呈现全部默认值');
    expect(within(fileLevel).getByText('$') !== null).toBe(true);

    const dotPath = itemByPath('tests[0].coverage.lines');
    expect(within(dotPath).getByText('tests[0].coverage.lines') !== null).toBe(true);
  });
});
