/**
 * 集成测试: create → templates/adr.md → decisions 落盘
 *
 * @see openspec/changes/migrate-archi-decide-to-mcp/test-design.md
 */

import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual };
});

import * as fs from 'node:fs';

import { createAdr } from '../../src/lib/archi-decide';

function setupProjectRoot(): { dir: string; cleanup: () => void } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'archi-template-it-'));
  return {
    dir,
    cleanup: () => fs.rmSync(dir, { recursive: true, force: true }),
  };
}

function decisionsDir(projectRoot: string): string {
  return path.join(projectRoot, 'openspec', 'architecture', 'decisions');
}

describe('create → 真实模板渲染', () => {
  let project: ReturnType<typeof setupProjectRoot>;

  beforeEach(() => {
    project = setupProjectRoot();
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-08-12T09:00:00'));
  });

  afterEach(() => {
    vi.useRealTimers();
    project.cleanup();
  });

  it('真实模板渲染后含全部章节标题与填入的 background/decision', () => {
    const result = createAdr(project.dir, {
      title: 'Use Event Sourcing',
      background: 'We need auditability.',
      decision: 'Adopt event sourcing for core domains.',
    });
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toContain('# ADR: Use Event Sourcing');
    expect(content).toContain('We need auditability.');
    expect(content).toContain('Adopt event sourcing for core domains.');
    for (const heading of [
      '## 背景',
      '## 决策',
      '## 后果',
      '### 正面后果',
      '### 负面后果',
      '## 备选方案',
      '## 影响范围',
    ]) {
      expect(content).toContain(heading);
    }
    expect(result.filename).toBe('2026-08-12-use-event-sourcing.md');
  });

  it('未传 status 时状态为 proposed', () => {
    const result = createAdr(project.dir, {
      title: 'Default Status ADR',
      background: 'bg',
      decision: 'dec',
    });
    expect(result.success).toBe(true);
    expect(fs.readFileSync(result.path!, 'utf-8')).toMatch(/\*\*状态\*\*:\s*proposed/);
  });

  it('人为切断模板解析路径时失败且 decisions 下无新文件', () => {
    const origExists = fs.existsSync.bind(fs);
    const existsSpy = vi.spyOn(fs, 'existsSync').mockImplementation((p) => {
      const normalized = String(p).replace(/\\/g, '/');
      if (normalized.includes('templates/adr.md')) {
        return false;
      }
      return origExists(p);
    });
    try {
      const result = createAdr(project.dir, {
        title: 'No Template',
        background: 'bg',
        decision: 'dec',
      });
      expect(result.success).toBe(false);
      expect(result.error).toContain('ADR template not found');
      expect(fs.existsSync(decisionsDir(project.dir))).toBe(false);
    } finally {
      existsSpy.mockRestore();
    }
  });

  it('scope 为空且 alternatives 为空时仍保留备选方案节与 (none) 影响范围', () => {
    const result = createAdr(project.dir, {
      title: 'Empty Sections',
      background: 'bg',
      decision: 'dec',
      scope: [],
      alternatives: [],
    });
    expect(result.success).toBe(true);
    const content = fs.readFileSync(result.path!, 'utf-8');
    expect(content).toContain('## 备选方案');
    expect(content).toContain('- (none)');
  });
});
