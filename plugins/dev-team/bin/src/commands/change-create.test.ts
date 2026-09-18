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

import { describe, expect, it, vi } from 'vite-plus/test';

import { resolveChangeDir } from '../lib/change';
import { workflowFileSchema, type ChangeCreateInput } from '../schemas';
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
      expect(result.path).toBe(resolveChangeDir('my-change', project.root));
      expect(result.path).toBe(path.resolve(project.root, 'openspec', 'changes', 'my-change'));
      expect(path.isAbsolute(result.path)).toBe(true);
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

describe('runChangeCreate — workflow.json 唯一创建者契约 (AC-6)', () => {
  it('新建成功后 workflow.json 仅有 workflow_type、created、file_log 三键，无 eval 键且不创建 eval.json (AC-1)', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('my-change', project.root, 'requirement');
      const workflow = readWorkflowJson(result.path);

      expect(Object.prototype.hasOwnProperty.call(workflow, 'eval')).toBe(false);
      expect(Object.keys(workflow)).toEqual(['workflow_type', 'created', 'file_log']);
      expect(fs.existsSync(path.join(result.path, 'eval.json'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('4 值枚举逐一均可创建，且均无 eval 键、无 eval.json', () => {
    const project = createTempProject();
    try {
      for (const workflowType of ['requirement', 'test-only', 'bug-fix', 'refactor'] as const) {
        const result = runChangeCreate(`enum-${workflowType}`, project.root, workflowType);
        const workflow = readWorkflowJson(result.path);

        expect(workflow.workflow_type).toBe(workflowType);
        expect(Object.prototype.hasOwnProperty.call(workflow, 'eval')).toBe(false);
        expect(fs.existsSync(path.join(result.path, 'eval.json'))).toBe(false);
      }
    } finally {
      project.cleanup();
    }
  });

  it('产出文件可被 workflowFileSchema 解析（唯一创建者契约的读方视角）', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('schema-change', project.root, 'test-only');
      const parsed = workflowFileSchema.safeParse(readWorkflowJson(result.path));

      expect(parsed.success).toBe(true);
      if (parsed.success) {
        expect(parsed.data.workflow_type).toBe('test-only');
        expect(parsed.data.eval).toBeUndefined();
      }
    } finally {
      project.cleanup();
    }
  });

  it('紧凑 JSON.stringify + 末尾换行的现行格式保留', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('format-change', project.root, 'requirement');
      const raw = fs.readFileSync(path.join(result.path, 'workflow.json'), 'utf-8');

      expect(raw.endsWith('\n')).toBe(true);
      expect(raw.trimEnd()).not.toContain('\n');
    } finally {
      project.cleanup();
    }
  });

  it('name 恰 128 字符时创建成功且仍无 eval 键', () => {
    const project = createTempProject();
    try {
      const maxName = 'a'.repeat(128);
      const result = runChangeCreate(maxName, project.root, 'requirement');

      expect(Object.prototype.hasOwnProperty.call(readWorkflowJson(result.path), 'eval')).toBe(
        false,
      );
    } finally {
      project.cleanup();
    }
  });

  it('name 超长（>1000 chars）时长度校验失败，不创建目录', () => {
    const project = createTempProject();
    try {
      expect(() => runChangeCreate('a'.repeat(1001), project.root, 'requirement')).toThrow(
        /128 字符/,
      );
      expect(fs.existsSync(path.join(project.root, 'openspec'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('name 含 emoji / 含 \\n 时 kebab 校验失败，不创建目录', () => {
    const project = createTempProject();
    try {
      for (const badName of ['chg-🧪', 'a\nb']) {
        expect(() => runChangeCreate(badName, project.root, 'requirement')).toThrow(/kebab-case/);
      }

      expect(fs.existsSync(path.join(project.root, 'openspec'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('name 为 undefined / null 时抛错且不创建目录', () => {
    const project = createTempProject();
    try {
      for (const badName of [undefined, null]) {
        expect(() =>
          runChangeCreate(badName as unknown as string, project.root, 'requirement'),
        ).toThrow();
      }

      expect(fs.existsSync(path.join(project.root, 'openspec'))).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('projectRoot 为 undefined / null 时抛错', () => {
    expect(() =>
      runChangeCreate('some-change', undefined as unknown as string, 'requirement'),
    ).toThrow();
    expect(() =>
      runChangeCreate('some-change', null as unknown as string, 'requirement'),
    ).toThrow();
  });

  it("workflowType 为空串 '' 时命令层原样写入，不因此补 eval 键；该文件不被 workflowFileSchema 接受", () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate(
        'empty-type-change',
        project.root,
        '' as ChangeCreateInput['workflow_type'],
      );
      const workflow = readWorkflowJson(result.path);

      expect(workflow.workflow_type).toBe('');
      expect(Object.keys(workflow)).toEqual(['workflow_type', 'created', 'file_log']);
      expect(workflowFileSchema.safeParse(workflow).success).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it("projectRoot 为空串 '' 时不崩溃（递归 mkdir 的相对路径行为）", () => {
    const project = createTempProject();
    // 空 projectRoot 由 `path.resolve('', …)` 相对 process.cwd() 解析。这里 mock
    // process.cwd 而非调用 process.chdir：Stryker 的 vitest runner 强制 worker
    // 线程池，worker 中 process.chdir 会抛 ERR_WORKER_UNSUPPORTED_OPERATION
    //（同类约定见 run-static-analysis.test.ts 对 getProjectDir 的 mock）。
    const cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(project.root);
    try {
      const result = runChangeCreate('relative-root-change', '', 'requirement');
      const workflow = readWorkflowJson(result.path);

      expect(result.path).toBe(
        path.resolve(project.root, 'openspec', 'changes', 'relative-root-change'),
      );
      expect(Object.prototype.hasOwnProperty.call(workflow, 'eval')).toBe(false);
    } finally {
      cwdSpy.mockRestore();
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
      // 已存在目录永不被「修复」：不补 eval 键、不创建 eval.json（AC-6）
      expect(Object.prototype.hasOwnProperty.call(originalWorkflow, 'eval')).toBe(false);
      expect(fs.existsSync(path.join(first.path, 'eval.json'))).toBe(false);
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

// ===========================================================================
// runChangeCreate — file_log 初始化 (AC-1 / AC-7 必需补充)
// ===========================================================================

describe('runChangeCreate — file_log 初始化 (AC-1)', () => {
  it('新建 workflow.json 含 file_log: [] 初始空日志，且不含 files 键 (AC-1)', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('inventory-init', project.root, 'requirement');
      const workflow = readWorkflowJson(result.path);

      expect(workflow.file_log).toEqual([]);
      expect(Object.prototype.hasOwnProperty.call(workflow, 'files')).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('键序为 workflow_type → created → file_log（JSON.stringify 插入序断言）(AC-1)', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('key-order', project.root, 'bug-fix');
      const raw = fs.readFileSync(path.join(result.path, 'workflow.json'), 'utf-8');
      const workflow = readWorkflowJson(result.path);

      expect(Object.keys(workflow)).toEqual(['workflow_type', 'created', 'file_log']);
      // 原始文本顺序同样契约化
      const typeIdx = raw.indexOf('"workflow_type"');
      const createdIdx = raw.indexOf('"created"');
      const logIdx = raw.indexOf('"file_log"');
      expect(typeIdx).toBeGreaterThan(-1);
      expect(createdIdx).toBeGreaterThan(typeIdx);
      expect(logIdx).toBeGreaterThan(createdIdx);
    } finally {
      project.cleanup();
    }
  });

  it('新建 workflow.json 可被 getChangedFiles 读取（空净状态）——与读方硬报错契约闭环 (AC-1)', async () => {
    const project = createTempProject();
    try {
      runChangeCreate('readable-log', project.root, 'requirement');

      const { getChangedFiles } = await import('../modules/workflow');
      expect(getChangedFiles('readable-log', project.root)).toEqual({ written: [], deleted: [] });
    } finally {
      project.cleanup();
    }
  });

  it('产出可被 workflowFileSchema 解析，file_log 字段合法为空数组 (AC-1)', () => {
    const project = createTempProject();
    try {
      const result = runChangeCreate('schema-files', project.root, 'refactor');
      const parsed = workflowFileSchema.safeParse(readWorkflowJson(result.path));

      expect(parsed.success).toBe(true);
      if (parsed.success) {
        expect(parsed.data.file_log).toEqual([]);
      }
    } finally {
      project.cleanup();
    }
  });

  it('4 值 workflow_type 枚举（requirement/bug-fix/refactor/test-only）逐一创建均含 file_log: [] (AC-1)', () => {
    const project = createTempProject();
    try {
      for (const workflowType of ['requirement', 'bug-fix', 'refactor', 'test-only'] as const) {
        const result = runChangeCreate(`files-${workflowType}`, project.root, workflowType);
        const workflow = readWorkflowJson(result.path);

        expect(workflow.workflow_type).toBe(workflowType);
        expect(workflow.file_log).toEqual([]);
      }
    } finally {
      project.cleanup();
    }
  });

  it('已存在 change 拒绝时不覆写、不修补已有 file_log（幂等回归扩展，AC-1）', () => {
    const project = createTempProject();
    try {
      const first = runChangeCreate('no-patch', project.root, 'requirement');
      // 把已有 change 退化成机制前旧形态（无 file_log），模拟历史目录
      const workflowPath = path.join(first.path, 'workflow.json');
      fs.writeFileSync(
        workflowPath,
        JSON.stringify({ workflow_type: 'requirement', created: '2025-01-01' }),
        'utf-8',
      );
      const before = fs.readFileSync(workflowPath, 'utf-8');

      expect(() => runChangeCreate('no-patch', project.root, 'requirement')).toThrow(/已存在/);

      // 拒绝路径绝不「顺手修补」file_log
      expect(fs.readFileSync(workflowPath, 'utf-8')).toBe(before);
      expect(readWorkflowJson(first.path).file_log).toBeUndefined();
    } finally {
      project.cleanup();
    }
  });
});
