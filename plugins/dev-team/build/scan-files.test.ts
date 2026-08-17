/**
 * 单元测试: scan-files.ts — 宽 include / 窄 exclude 文本枚举
 */

import { mkdirSync, mkdtempSync, rmSync, statSync, writeFileSync } from 'node:fs';
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

  it('排除 .map 与二进制扩展名，但 .js 作为文本文件被枚举', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'keep.md'), 'ok');
    writeFileSync(join(root, 'app.js'), 'bundle');
    writeFileSync(join(root, 'x.map'), 'map');
    writeFileSync(join(root, 'pic.png'), Buffer.from([1, 2, 3]));
    writeFileSync(join(root, 'pic.jpg'), Buffer.from([1, 2, 3]));

    const files = scanTextFiles(root);
    expect(files.sort()).toEqual(['app.js', 'keep.md'].sort());
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

  it('openspec-bundled.js 不再被排除，scanTextFiles 返回结果中包含该文件', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'openspec-bundled.js'), 'bundle');
    writeFileSync(join(root, 'keep.md'), 'ok');
    const files = scanTextFiles(root);
    expect(files).toContain('openspec-bundled.js');
    expect(files).toContain('keep.md');
  });

  it('EXCLUDE_BASENAMES 集合为空，不基于文件名排除任何文本文件', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'config.json'), '{}');
    writeFileSync(join(root, 'main.js'), 'x');
    const files = scanTextFiles(root);
    expect(files.sort()).toEqual(['config.json', 'main.js'].sort());
  });

  it('rootDir 指向文件而非目录时返回空数组（不抛错）', () => {
    const root = makeTempDir();
    const filePath = join(root, 'a.md');
    writeFileSync(filePath, '# a');
    expect(statSync(filePath).isDirectory()).toBe(false);
    expect(scanTextFiles(filePath)).toEqual([]);
  });

  it('rootDir 含 emoji 与 unicode 非 ASCII 字符时按 OS 路径规则处理', () => {
    const root = makeTempDir();
    const sub = join(root, '技能-🚀');
    mkdirSync(sub, { recursive: true });
    writeFileSync(join(sub, '指南.md'), '文档');
    const files = scanTextFiles(sub);
    expect(files).toEqual(['指南.md']);
    // 不存在的 unicode 路径按 statSync 语义抛 ENOENT
    expect(() => scanTextFiles(join(root, '不存在-💥'))).toThrow();
  });

  it('rootDir 超长路径（>260 字符）不吞没 statSync 异常', () => {
    const root = makeTempDir();
    const longPath = join(root, 'x'.repeat(300));
    const filePath = join(longPath, 'a.md');
    let threw = false;
    try {
      mkdirSync(longPath, { recursive: true });
      writeFileSync(filePath, 'x');
    } catch {
      threw = true;
    }
    // Windows MAX_PATH（260 字符）以上的绝对路径通常无法创建/访问
    if (threw) {
      expect(() => scanTextFiles(longPath)).toThrow();
      return;
    }
    // 长路径可用的平台（如 \?\ 前缀或非 Windows）仍能正常扫描
    expect(scanTextFiles(longPath)).toEqual(['a.md']);
  });
});
