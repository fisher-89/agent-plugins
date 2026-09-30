// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { WriteProtection } from '../../../types/generated/bindings';
import { WriteProtectionSection } from './write-protection-section';

// ---------------------------------------------------------------------------
// WriteProtectionSection 单测（AC-5）：纯组件 fixture 直传入参渲染断言，无 mock。
// ---------------------------------------------------------------------------

describe('WriteProtectionSection：write_protection 面板与空态占位（AC-5）', () => {
  it('files 逐条 glob + reason 呈现（config-write-protection 在场）', () => {
    const protection: WriteProtection = {
      files: [
        { glob: 'tests/golden/**', reason: '金样冻结' },
        { glob: '*.lock', reason: null },
      ],
    };

    render(<WriteProtectionSection protection={protection} />);

    expect(screen.getByTestId('config-write-protection') !== null).toBe(true);
    const rules = screen.getAllByTestId('config-write-protection-rule');
    expect(rules).toHaveLength(2);
    expect(rules[0].textContent).toContain('tests/golden/**');
    expect(rules[0].textContent).toContain('金样冻结');
    expect(rules[1].textContent).toContain('*.lock');
  });

  it('protection 为 null：弱化「未配置」占位、不崩', () => {
    render(<WriteProtectionSection protection={null} />);

    expect(screen.getByTestId('config-write-protection') !== null).toBe(true);
    expect(screen.getByText('未配置') !== null).toBe(true);
    expect(screen.queryAllByTestId('config-write-protection-rule')).toHaveLength(0);
  });

  it('protection 在而 files 为 null / 空数组：弱化空态呈现不崩', () => {
    const nullFiles = render(<WriteProtectionSection protection={{ files: null }} />);
    expect(screen.getByTestId('config-write-protection') !== null).toBe(true);
    expect(screen.getByText('未配置') !== null).toBe(true);
    nullFiles.unmount();

    const emptyFiles = render(<WriteProtectionSection protection={{ files: [] }} />);
    expect(screen.getByTestId('config-write-protection') !== null).toBe(true);
    expect(screen.getByText('未配置') !== null).toBe(true);
    expect(screen.queryAllByTestId('config-write-protection-rule')).toHaveLength(0);
    emptyFiles.unmount();
  });

  it('reason 为 null：该条 reason 位占位不出文案；多条规则（含 glob null 形态）全量渲染', () => {
    const protection: WriteProtection = {
      files: [
        { glob: 'a/**', reason: null },
        { glob: 'b/**', reason: '理由' },
        { glob: null, reason: null },
      ],
    };

    render(<WriteProtectionSection protection={protection} />);

    const rules = screen.getAllByTestId('config-write-protection-rule');
    expect(rules).toHaveLength(3);
    expect(rules[0].textContent).toContain('a/**');
    expect(rules[0].textContent).not.toContain('理由');
    expect(rules[1].textContent).toContain('b/**');
    expect(rules[1].textContent).toContain('理由');
    // glob 为 null 的条目以「—」占位呈现（RuleRow glob ?? '—' 防御）
    expect(rules[2].textContent).toContain('—');
  });
});
