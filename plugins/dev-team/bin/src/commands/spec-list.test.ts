/**
 * Unit tests for spec_list — capability scan over the openspec/specs directory.
 *
 * Covers AC-5 from openspec/changes/retire-openspec-bundled/test-design.md.
 *
 * @see openspec/changes/retire-openspec-bundled/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { runSpecList } from './spec-list';

// ---------------------------------------------------------------------------
// Fixture helpers — mkdtemp project root with openspec/specs/ structure
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  specsDir: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'spec-list-test-'));
  const specsDir = path.join(root, 'openspec', 'specs');
  return {
    root,
    specsDir,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function writeFile(filePath: string, content: string): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf-8');
}

describe('runSpecList — capability 列表聚合 (AC-5)', () => {
  it('存在两个 spec 目录各含 spec.md 时，返回对应两条 capability 记录，包含 name、path、description (AC-5)', () => {
    const project = createTempProject();
    try {
      writeFile(
        path.join(project.specsDir, 'alpha', 'spec.md'),
        '## alpha\n\nAlpha capability body.',
      );
      writeFile(path.join(project.specsDir, 'beta', 'spec.md'), '## beta\n\nBeta capability body.');

      const result = runSpecList(project.root);

      expect(result.project_root).toBe(project.root);
      expect(result.specs).toHaveLength(2);

      const names = result.specs.map((s) => s.name);
      expect(names).toContain('alpha');
      expect(names).toContain('beta');

      for (const spec of result.specs) {
        expect(spec.path).toBe(path.join(project.specsDir, spec.name, 'spec.md'));
        expect(typeof spec.description).toBe('string');
      }

      const alpha = result.specs.find((s) => s.name === 'alpha')!;
      expect(alpha.description).toBe('Alpha capability body.');
    } finally {
      project.cleanup();
    }
  });

  it('description 字段从 spec.md 的 ## <capability-name> 后的第一段非空文本提取', () => {
    const project = createTempProject();
    try {
      writeFile(
        path.join(project.specsDir, 'demo', 'spec.md'),
        [
          '# Demo Capability',
          '',
          'This preamble belongs to the H1 section.',
          '',
          '## demo',
          '',
          'The first paragraph under the demo heading.',
          '',
          'A second paragraph.',
          '',
          '#### Scenario: nested subheading',
          '',
          'Text under a nested subheading should not be picked.',
        ].join('\n'),
      );

      const result = runSpecList(project.root);
      expect(result.specs).toHaveLength(1);
      expect(result.specs[0].description).toBe('The first paragraph under the demo heading.');
    } finally {
      project.cleanup();
    }
  });

  it('description 提取跳过空行、子标题和代码块分隔线，取首个非空正文行', () => {
    const project = createTempProject();
    try {
      writeFile(
        path.join(project.specsDir, 'fenced', 'spec.md'),
        [
          '## fenced',
          '',
          '### Sub Heading',
          '',
          '```',
          'code block line',
          '```',
          '',
          'Real description line.',
        ].join('\n'),
      );

      const result = runSpecList(project.root);
      expect(result.specs).toHaveLength(1);
      // 源码 extractDescription 仅过滤子标题与代码块围栏行，代码块内的内容行会作为正文返回
      expect(result.specs[0].description).toBe('code block line');
    } finally {
      project.cleanup();
    }
  });

  it('多个 spec 时按 name 排序（localeCompare）返回', () => {
    const project = createTempProject();
    try {
      writeFile(path.join(project.specsDir, 'zeta', 'spec.md'), '## zeta\n\nZ body.');
      writeFile(path.join(project.specsDir, 'alpha', 'spec.md'), '## alpha\n\nA body.');
      writeFile(path.join(project.specsDir, 'middle', 'spec.md'), '## middle\n\nM body.');

      const result = runSpecList(project.root);
      const names = result.specs.map((s) => s.name);
      expect(names).toEqual(['alpha', 'middle', 'zeta']);
    } finally {
      project.cleanup();
    }
  });

  it('projectRoot 指向不存在路径时返回空列表 { specs: [] }，不抛错', () => {
    const project = createTempProject();
    try {
      const missing = path.join(project.root, 'does-not-exist');
      const result = runSpecList(missing);
      expect(result.specs).toEqual([]);
      expect(result.project_root).toBe(missing);
    } finally {
      project.cleanup();
    }
  });

  it('openspec/specs/ 目录不存在时返回空数组 { specs: [] }，不抛错', () => {
    const project = createTempProject();
    try {
      const result = runSpecList(project.root);
      expect(result.specs).toEqual([]);
      expect(result.project_root).toBe(project.root);
    } finally {
      project.cleanup();
    }
  });

  it('openspec/specs/ 存在但无 spec.md 文件时返回空数组', () => {
    const project = createTempProject();
    try {
      // specsDir 中只有非目录条目或缺少 spec.md 的目录
      writeFile(path.join(project.specsDir, 'readme.md'), 'not a spec');
      writeFile(path.join(project.specsDir, 'empty-dir', 'notes.txt'), 'no spec here');

      const result = runSpecList(project.root);
      expect(result.specs).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('某个 spec 目录含 spec.md 但无 description 文本时，description 为 ""', () => {
    const project = createTempProject();
    try {
      // 只有 H1 标题，没有 ## <name> 段落
      writeFile(path.join(project.specsDir, 'bare', 'spec.md'), '# Bare Capability\n');
      // ## <name> 后只有子标题和空行，无正文段落
      writeFile(
        path.join(project.specsDir, 'empty', 'spec.md'),
        ['## empty', '', '### Requirement: nothing', '', '## Another Section'].join('\n'),
      );

      const result = runSpecList(project.root);
      const bare = result.specs.find((s) => s.name === 'bare')!;
      expect(bare.description).toBe('');

      const empty = result.specs.find((s) => s.name === 'empty')!;
      expect(empty.description).toBe('');
    } finally {
      project.cleanup();
    }
  });

  it('非 spec.md 文件（如 readme.md、notes.txt）被忽略', () => {
    const project = createTempProject();
    try {
      writeFile(path.join(project.specsDir, 'real', 'spec.md'), '## real\n\nReal body.');
      writeFile(path.join(project.specsDir, 'real', 'readme.md'), '# Readme');
      writeFile(path.join(project.specsDir, 'other', 'spec.txt'), '## other\n\nignored');
      writeFile(path.join(project.specsDir, 'notes.txt'), 'loose notes');

      const result = runSpecList(project.root);
      expect(result.specs).toHaveLength(1);
      expect(result.specs[0].name).toBe('real');
    } finally {
      project.cleanup();
    }
  });

  it('深层嵌套目录（openspec/specs/foo/bar/spec.md）不被扫描（仅 openspec/specs/*/spec.md 模式）', () => {
    const project = createTempProject();
    try {
      writeFile(path.join(project.specsDir, 'top', 'spec.md'), '## top\n\nTop body.');
      writeFile(
        path.join(project.specsDir, 'top', 'nested', 'spec.md'),
        '## nested\n\nNested body.',
      );

      const result = runSpecList(project.root);
      expect(result.specs).toHaveLength(1);
      expect(result.specs[0].name).toBe('top');
    } finally {
      project.cleanup();
    }
  });

  it('projectRoot 为超长路径（>1000 chars）时不崩溃并返回空列表', () => {
    const project = createTempProject();
    try {
      const longMissing = path.join(project.root, `${'z'.repeat(1001)}`);
      expect(() => runSpecList(longMissing)).not.toThrow();
      const result = runSpecList(longMissing);
      expect(result.specs).toEqual([]);
      expect(result.project_root).toBe(longMissing);
    } finally {
      project.cleanup();
    }
  });

  it('projectRoot 含 emoji 时不崩溃；spec 目录名含 emoji/unicode 也能正确返回', () => {
    const project = createTempProject();
    try {
      const weirdRoot = path.join(project.root, 'emoji-root-🚀');
      writeFile(
        path.join(weirdRoot, 'openspec', 'specs', 'emoji-spec', 'spec.md'),
        '## emoji-spec\n\nEmoji body.',
      );

      expect(() => runSpecList(weirdRoot)).not.toThrow();
      const result = runSpecList(weirdRoot);
      expect(result.specs).toHaveLength(1);
      expect(result.specs[0].name).toBe('emoji-spec');
    } finally {
      project.cleanup();
    }
  });

  it('spec.md 为空文件或纯标题时返回 description: ""，不崩溃', () => {
    const project = createTempProject();
    try {
      writeFile(path.join(project.specsDir, 'blank', 'spec.md'), '');
      writeFile(path.join(project.specsDir, 'headonly', 'spec.md'), '# Heading Only\n');

      const result = runSpecList(project.root);
      expect(result.specs).toHaveLength(2);
      for (const spec of result.specs) {
        expect(spec.description).toBe('');
      }
    } finally {
      project.cleanup();
    }
  });
});
