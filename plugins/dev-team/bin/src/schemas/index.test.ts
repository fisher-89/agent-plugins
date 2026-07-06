/**
 * 单元测试: schemas/index.ts — 已删除 schema 导出验证
 *
 * @see openspec/changes/write-protection-config/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// exports — 已删除 schema 不再导出 (AC-7)
// ---------------------------------------------------------------------------

describe('exports — 已删除 schema 不再导出 (AC-7)', () => {
  it('configSetInputSchema 不再从 index.ts 导出', async () => {
    await expect(import('./index')).resolves.not.toHaveProperty('configSetInputSchema');
  });

  it('configSetOutputSchema 不再从 index.ts 导出', async () => {
    await expect(import('./index')).resolves.not.toHaveProperty('configSetOutputSchema');
  });

  it('configUnsetInputSchema 不再从 index.ts 导出', async () => {
    await expect(import('./index')).resolves.not.toHaveProperty('configUnsetInputSchema');
  });

  it('configUnsetOutputSchema 不再从 index.ts 导出', async () => {
    await expect(import('./index')).resolves.not.toHaveProperty('configUnsetOutputSchema');
  });

  it('configContextInputSchema 不再从 index.ts 导出', async () => {
    await expect(import('./index')).resolves.not.toHaveProperty('configContextInputSchema');
  });

  it('configContextOutputSchema 不再从 index.ts 导出', async () => {
    await expect(import('./index')).resolves.not.toHaveProperty('configContextOutputSchema');
  });
});

// ---------------------------------------------------------------------------
// exports — config_get 相关导出仍保留 (AC-6)
// ---------------------------------------------------------------------------

describe('exports — configGetInputSchema / configGetOutputSchema 仍保留导出 (AC-6)', () => {
  it('configGetInputSchema 仍从 index.ts 导出', async () => {
    const mod = await import('./index');
    expect(mod).toHaveProperty('configGetInputSchema');
  });

  it('configGetOutputSchema 仍从 index.ts 导出', async () => {
    const mod = await import('./index');
    expect(mod).toHaveProperty('configGetOutputSchema');
  });
});

// ---------------------------------------------------------------------------
// exports — 核心导出仍保留
// ---------------------------------------------------------------------------

describe('exports — 核心导出仍保留', () => {
  it('phaseLogInputSchema / phaseLogOutputSchema 仍导出', async () => {
    const mod = await import('./index');
    expect(mod).toHaveProperty('phaseLogInputSchema');
    expect(mod).toHaveProperty('phaseLogOutputSchema');
  });

  it('configSchema 仍导出', async () => {
    const mod = await import('./index');
    expect(mod).toHaveProperty('configSchema');
  });

  it('changeListInputSchema / changeListOutputSchema 仍导出', async () => {
    const mod = await import('./index');
    expect(mod).toHaveProperty('changeListInputSchema');
    expect(mod).toHaveProperty('changeListOutputSchema');
  });
});
