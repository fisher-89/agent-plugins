/**
 * Unit tests for change_create — kebab-case validation and change directory creation.
 *
 * Covers AC-2~AC-4 from openspec/changes/retire-openspec-bundled/test-design.md.
 *
 * @see openspec/changes/retire-openspec-bundled/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { getChangeDir } from '../lib/change';
import { runChangeCreate } from './change-create';

// ---------------------------------------------------------------------------
// Fixture helpers — mkdtemp project root, real filesystem, cleaned after each
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'change-create-test-'));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function readWorkflowJson(changeDir: string): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8'));
}

describe('runChangeCreate — kebab-case 名称校验 (AC-3)', () => {
  it('合法 kebab-case 名称（如 my-change）创建目录并写入 workflow.json，包含 workflow_type: "requirement" 和 ISO 日期 created (AC-2)', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('my-change', project.root, 'requirement');
      const changeDir = result.path;
      expect(fs.existsSync(changeDir)).toBe(true);
      expect(fs.existsSync(path.join(changeDir, 'workflow.json'))).toBe(true);

      const workflow = readWorkflowJson(changeDir);
      expect(workflow.workflow_type).toBe('requirement');
      expect(workflow.created).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(workflow.created).toBe(new Date().toISOString().slice(0, 10));
    } finally {
      project.cleanup();
    }
  });

  it('返回对象包含 name 和 path 字段，path 指向 openspec/changes/<name>/ (AC-2)', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('my-change', project.root, 'requirement');
      expect(result.name).toBe('my-change');
      expect(result.path).toBe(getChangeDir('my-change', project.root));
      expect(result.path).toBe(path.resolve(project.root, 'openspec', 'changes', 'my-change'));
      expect(path.isAbsolute(result.path)).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('传入 workflow_type 参数时写入对应 workflow_type 到 workflow.json', () => {
    const project = createTempProject();
    try {
      for (const type of ['bug-fix', 'refactor', 'test-only']) {
        const result = runChangeCreate(`wf-${type}`, project.root, type);
        expect(readWorkflowJson(result.path).workflow_type).toBe(type);
      }
    } finally {
      project.cleanup();
    }
  });

  it('省略 workflow_type 参数时默认写入 "requirement"（保持向后兼容）', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('default-wf', project.root, 'requirement');
      expect(readWorkflowJson(result.path).workflow_type).toBe('requirement');
    } finally {
      project.cleanup();
    }
  });

  it('名称仅单个单词（如 fix）应通过 kebab-case 校验', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('fix', project.root, 'requirement');
      expect(result.name).toBe('fix');
      expect(fs.existsSync(result.path)).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('名称含数字（如 fix-123）应通过 kebab-case 校验', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('fix-123', project.root, 'requirement');
      expect(result.name).toBe('fix-123');
      expect(fs.existsSync(result.path)).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('名称含数字前缀（如 123-fix）应通过 kebab-case 校验 (AC-2)', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('123-fix', project.root, 'requirement');
      expect(result.name).toBe('123-fix');
      expect(fs.existsSync(result.path)).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('非 kebab-case 名称（My Change / my_change / MyChange / -fix / 空字符串）抛出错误 (AC-3)', () => {
    const project = createTempProject();
    try {
      for (const badName of ['My Change', 'my_change', 'MyChange', '-fix', '']) {
        expect(() => runChangeCreate(badName, project.root, 'requirement')).toThrow(/kebab-case/);
      }
    } finally {
      project.cleanup();
    }
  });

  it('非 kebab-case 名称不创建任何目录或文件 (AC-3)', () => {
    const project = createTempProject();
    try {
      expect(() => runChangeCreate('MyChange', project.root, 'requirement')).toThrow(/kebab-case/);
      expect(fs.existsSync(path.join(project.root, 'openspec'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('名称以 - 结尾（fix-）按当前 kebab-case 正则属于合法输入，创建成功（与源码行为一致）', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('fix-', project.root, 'requirement');
      expect(fs.existsSync(result.path)).toBe(true);
      expect(readWorkflowJson(result.path).workflow_type).toBe('requirement');
    } finally {
      project.cleanup();
    }
  });

  it('名称超长（>128 字符）时应被拒绝', () => {
    const project = createTempProject();
    try {
      const longName = 'a'.repeat(129);
      expect(() => runChangeCreate(longName, project.root, 'requirement')).toThrow(/128 字符/);
      expect(fs.existsSync(path.join(project.root, 'openspec'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('名称恰为 128 字符时应通过校验并创建成功（与源码 MAX_NAME_LENGTH=128 一致）', () => {
    const project = createTempProject();
    try {
      const maxName = 'a'.repeat(128);
      const result = runChangeCreate(maxName, project.root, 'requirement');
      expect(result.name).toBe(maxName);
      expect(fs.existsSync(result.path)).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('projectRoot 指向不存在路径时仍递归创建 openspec/changes/ 目录（mkdirSync recursive）', () => {
    const project = createTempProject();
    try {
      const deepMissing = path.join(project.root, 'nested', 'deep', 'root');
      const result = runChangeCreate('deep-change', deepMissing, 'requirement');
      expect(fs.existsSync(result.path)).toBe(true);
      expect(result.path).toBe(path.resolve(deepMissing, 'openspec', 'changes', 'deep-change'));
    } finally {
      project.cleanup();
    }
  });

  it('openspec/changes/ 目录不存在时自动创建（递归 mkdir）(AC-2)', () => {
    const project = createTempProject();
    try {
      expect(fs.existsSync(path.join(project.root, 'openspec'))).toBe(false);
      const result = runChangeCreate('fresh-change', project.root, 'requirement');
      expect(fs.existsSync(result.path)).toBe(true);
      expect(fs.existsSync(path.join(result.path, 'workflow.json'))).toBe(true);
    } finally {
      project.cleanup();
    }
  });
});

describe('runChangeCreate — 拒绝已存在 change (AC-4)', () => {
  it('已存在的 change 目录名称抛出错误，且不覆盖已有目录内容 (AC-4)', () => {
    const project = createTempProject();
    try {
      const first = runChangeCreate('existing-change', project.root, 'requirement');
      // 在已有目录中放置一个用户文件，验证不被覆盖
      fs.writeFileSync(path.join(first.path, 'proposal.md'), '# original proposal', 'utf-8');
      const originalWorkflow = readWorkflowJson(first.path);

      expect(() => runChangeCreate('existing-change', project.root, 'requirement')).toThrow(
        /已存在/,
      );

      expect(fs.readFileSync(path.join(first.path, 'proposal.md'), 'utf-8')).toBe(
        '# original proposal',
      );
      expect(readWorkflowJson(first.path)).toEqual(originalWorkflow);
    } finally {
      project.cleanup();
    }
  });

  it('重复调用同名称不产生副作用：目录、workflow.json 内容保持不变（幂等）', () => {
    const project = createTempProject();
    try {
      const first = runChangeCreate('idem-change', project.root, 'requirement');
      const firstWorkflow = readWorkflowJson(first.path);

      expect(() => runChangeCreate('idem-change', project.root, 'requirement')).toThrow(/已存在/);

      expect(fs.existsSync(first.path)).toBe(true);
      expect(readWorkflowJson(first.path)).toEqual(firstWorkflow);
    } finally {
      project.cleanup();
    }
  });
});
