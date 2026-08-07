/**
 * 单元测试: scan-files.ts — 宽 include / 窄 exclude 文本枚举
 */

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, describe, expect, it } from 'vite-plus/test';

import { scanTextFiles } from './scan-files';

const tempDirs: string[] = [];

function makeTempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), 'scan-files-'));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
});

describe('scanTextFiles', () => {
  it('临时树含 skills / agents / bin 文本时均被枚举', () => {
    const root = makeTempDir();
    mkdirSync(join(root, 'skills', 'a'), { recursive: true });
    mkdirSync(join(root, 'agents'), { recursive: true });
    mkdirSync(join(root, 'bin'), { recursive: true });
    writeFileSync(join(root, 'skills', 'a', 'SKILL.md'), '# skill');
    writeFileSync(join(root, 'agents', 'b.md'), '# agent');
    writeFileSync(join(root, 'bin', 'mcp.cjs'), 'module.exports = {}');

    const files = scanTextFiles(root).sort();
    expect(files).toEqual(['agents/b.md', 'bin/mcp.cjs', 'skills/a/SKILL.md'].sort());
  });

  it('templates / utils 下文本被枚举', () => {
    const root = makeTempDir();
    mkdirSync(join(root, 'templates'), { recursive: true });
    mkdirSync(join(root, 'utils'), { recursive: true });
    writeFileSync(join(root, 'templates', 'adr.md'), 'tpl');
    writeFileSync(join(root, 'utils', 'tool.py'), 'print(1)');

    const files = scanTextFiles(root);
    expect(files).toContain('templates/adr.md');
    expect(files).toContain('utils/tool.py');
  });

  it('rootDir 不存在时抛错', () => {
    expect(() => scanTextFiles(join(tmpdir(), 'missing-scan-' + Date.now()))).toThrow();
  });

  it('rootDir 为 null / undefined 时抛错', () => {
    expect(() => scanTextFiles(null as unknown as string)).toThrow();
    expect(() => scanTextFiles(undefined as unknown as string)).toThrow();
  });

  it('rootDir 为空字符串时抛错或返回空列表', () => {
    let threw = false;
    let result: string[] | undefined;
    try {
      result = scanTextFiles('');
    } catch {
      threw = true;
    }
    expect(threw || result?.length === 0).toBe(true);
  });

  it('排除 openspec-bundled.js、.map 与二进制扩展名', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'keep.md'), 'ok');
    writeFileSync(join(root, 'openspec-bundled.js'), 'bundle');
    writeFileSync(join(root, 'x.map'), 'map');
    writeFileSync(join(root, 'pic.png'), Buffer.from([1, 2, 3]));
    writeFileSync(join(root, 'pic.jpg'), Buffer.from([1, 2, 3]));

    const files = scanTextFiles(root);
    expect(files).toEqual(['keep.md']);
  });

  it('空目录返回空列表', () => {
    const root = makeTempDir();
    expect(scanTextFiles(root)).toEqual([]);
  });

  it('超大文件列表仍完整返回且不崩溃', () => {
    const root = makeTempDir();
    mkdirSync(join(root, 'many'), { recursive: true });
    for (let i = 0; i < 120; i += 1) {
      writeFileSync(join(root, 'many', `f${i}.md`), `file ${i}`);
    }
    const files = scanTextFiles(root);
    expect(files).toHaveLength(120);
  });
});
