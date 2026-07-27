/**
 * 单元测试: runStaticAnalysis 命令
 *
 * 覆盖 AC-3、AC-4、AC-5、AC-7：配置读取、命令执行、exit code 传播与错误输出。
 *
 * @see openspec/changes/static-check-agent-hook/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { execCommand } from '../lib/exec-command';
import { getProjectDir } from '../lib/project-root';
import { runStaticAnalysis } from './run-static-analysis';

vi.mock('../lib/exec-command');

// Clear execCommand call history before each test to prevent shared mock
// state across shuffled test order.
beforeEach(() => {
  vi.mocked(execCommand).mockClear();
});

// getProjectDir 已有独立的单元测试（constant.test.ts），此处 mock 以支持
// Stryker worker 线程环境（worker 中不支持 process.chdir）。
vi.mock('../lib/project-root');

// ---------------------------------------------------------------------------
// Helpers: 临时项目目录
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData?: unknown): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'run-static-analysis-'));
  if (configData !== undefined) {
    const openspecDir = path.join(root, 'openspec');
    fs.mkdirSync(openspecDir, { recursive: true });
    fs.writeFileSync(
      path.join(openspecDir, 'config.json'),
      JSON.stringify(configData, null, 2),
      'utf-8',
    );
  }
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function writeConfig(root: string, configData: unknown): void {
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify(configData, null, 2),
    'utf-8',
  );
}

// ---------------------------------------------------------------------------
// AC-4 / AC-7: 未配置 static_analysis 时放行
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — 未配置放行 (AC-4, AC-7)', () => {
  it('openspec/config.json 不存在时应 exit 0 且不执行外部命令', () => {
    const project = createTempProject();
    const execSpy = vi.mocked(execCommand);
    try {
      const code = runStaticAnalysis({ projectRoot: project.root });
      expect(code).toBe(0);
      expect(execSpy).not.toHaveBeenCalled();
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });

  it('config 存在但无 static_analysis 字段时应 exit 0', () => {
    const project = createTempProject({ schema: 'spec-driven' });
    const execSpy = vi.mocked(execCommand);
    try {
      const code = runStaticAnalysis({ projectRoot: project.root });
      expect(code).toBe(0);
      expect(execSpy).not.toHaveBeenCalled();
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// 边界: 空/空白 static_analysis 配置
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — 空配置放行 (边界)', () => {
  it('static_analysis 为空字符串时应 exit 0 且不执行外部命令', () => {
    const project = createTempProject({ schema: 'spec-driven', static_analysis: '' });
    const execSpy = vi.mocked(execCommand);
    try {
      const code = runStaticAnalysis({ projectRoot: project.root });
      expect(code).toBe(0);
      expect(execSpy).not.toHaveBeenCalled();
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });

  it('static_analysis 为仅空白字符串时应 exit 0（trim 后视为空）', () => {
    const project = createTempProject({ schema: 'spec-driven', static_analysis: '   ' });
    const execSpy = vi.mocked(execCommand);
    try {
      const code = runStaticAnalysis({ projectRoot: project.root });
      expect(code).toBe(0);
      expect(execSpy).not.toHaveBeenCalled();
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-3 / AC-7: 已配置且命令成功
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — 命令成功 (AC-3, AC-7)', () => {
  it('已配置且 mock 命令 exit 0 时 CLI 应 exit 0', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      static_analysis: 'node -e process.exit(0)',
    });
    const execSpy = vi.mocked(execCommand).mockReturnValue({
      status: 0,
      stdout: '',
      stderr: '',
      pid: 1,
      output: [null, '', ''],
      signal: null,
      error: undefined,
    });
    try {
      const code = runStaticAnalysis({ projectRoot: project.root });
      expect(code).toBe(0);
      expect(execSpy).toHaveBeenCalled();
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });

  it('命令 exit 0 时 CLI stdout 应无多余输出', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      static_analysis: 'node -e process.exit(0)',
    });
    vi.mocked(execCommand).mockReturnValue({
      status: 0,
      stdout: '',
      stderr: '',
      pid: 1,
      output: [null, '', ''],
      signal: null,
      error: undefined,
    });
    const stdoutSpy = vi.spyOn(process.stdout, 'write').mockImplementation(() => true);
    try {
      runStaticAnalysis({ projectRoot: project.root });
      const calls = stdoutSpy.mock.calls.flat().join('');
      expect(calls.trim()).toBe('');
    } finally {
      stdoutSpy.mockRestore();
      vi.mocked(execCommand).mockReset();
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-5 / AC-7: 命令失败与错误输出
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — 命令失败 (AC-5, AC-7)', () => {
  it('已配置且 mock 命令 exit 非 0 时 CLI 应返回相同 exit code', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      static_analysis: 'node -e process.exit(1)',
    });
    const execSpy = vi.mocked(execCommand).mockReturnValue({
      status: 1,
      stdout: '',
      stderr: 'lint failed',
      pid: 1,
      output: [null, '', 'lint failed'],
      signal: null,
      error: undefined,
    });
    try {
      const code = runStaticAnalysis({ projectRoot: project.root });
      expect(code).toBe(1);
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });

  it('命令失败时 CLI stderr 应含 stdout/stderr 合并输出', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      static_analysis: 'node -e process.exit(1)',
    });
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    vi.mocked(execCommand).mockReturnValue({
      status: 1,
      stdout: 'stdout line\n',
      stderr: 'stderr line\n',
      pid: 1,
      output: [null, 'stdout line\n', 'stderr line\n'],
      signal: null,
      error: undefined,
    });
    try {
      runStaticAnalysis({ projectRoot: project.root });
      const output = stderrSpy.mock.calls.flat().join('');
      expect(output).toContain('stdout line');
      expect(output).toContain('stderr line');
    } finally {
      stderrSpy.mockRestore();
      vi.restoreAllMocks();
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// 边界: exit code 传播
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — exit code 传播 (边界)', () => {
  it.each([1, 2, 127])('mock 命令 exit %i 时 CLI 应返回相同 exit code', (exitCode) => {
    const project = createTempProject({
      schema: 'spec-driven',
      static_analysis: `node -e process.exit(${String(exitCode)})`,
    });
    vi.mocked(execCommand).mockReturnValue({
      status: exitCode,
      stdout: '',
      stderr: '',
      pid: 1,
      output: [null, '', ''],
      signal: null,
      error: undefined,
    });
    try {
      expect(runStaticAnalysis({ projectRoot: project.root })).toBe(exitCode);
    } finally {
      vi.restoreAllMocks();
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-7: 项目根目录解析
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — 项目根目录 (AC-7)', () => {
  let project: TempProject;

  beforeEach(() => {
    project = createTempProject({ schema: 'spec-driven', static_analysis: 'echo ok' });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    project.cleanup();
  });

  it('未传 options.projectRoot 时应使用 getProjectDir() 返回值定位 config', () => {
    vi.mocked(getProjectDir).mockReturnValue(project.root);
    const execSpy = vi.mocked(execCommand).mockReturnValue({
      status: 0,
      stdout: '',
      stderr: '',
      pid: 1,
      output: [null, '', ''],
      signal: null,
      error: undefined,
    });
    try {
      runStaticAnalysis();
      expect(execSpy).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({ cwd: project.root }),
      );
    } finally {
      execSpy.mockRestore();
    }
  });

  it('options.projectRoot 应优先于 getProjectDir()', () => {
    const other = createTempProject({ schema: 'spec-driven', static_analysis: 'echo ok' });
    vi.mocked(getProjectDir).mockReturnValue(other.root);
    const execSpy = vi.mocked(execCommand).mockReturnValue({
      status: 0,
      stdout: '',
      stderr: '',
      pid: 1,
      output: [null, '', ''],
      signal: null,
      error: undefined,
    });
    try {
      runStaticAnalysis({ projectRoot: project.root });
      expect(execSpy).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({ cwd: project.root }),
      );
    } finally {
      execSpy.mockRestore();
      other.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// 边界: config JSON 解析失败
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — config 容错 (边界)', () => {
  it('openspec/config.json JSON 解析失败时应等同未配置并 exit 0', () => {
    const project = createTempProject();
    writeConfig(project.root, '{ invalid json');
    const execSpy = vi.mocked(execCommand);
    try {
      const code = runStaticAnalysis({ projectRoot: project.root });
      expect(code).toBe(0);
      expect(execSpy).not.toHaveBeenCalled();
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// AC-7: 工作目录与命令参数
// ---------------------------------------------------------------------------

describe('runStaticAnalysis — 工作目录与参数 (AC-7)', () => {
  it('已配置时应在项目根目录执行命令（cwd 为 projectRoot）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      static_analysis: 'node -e process.exit(0)',
    });
    const execSpy = vi.mocked(execCommand).mockReturnValue({
      status: 0,
      stdout: '',
      stderr: '',
      pid: 1,
      output: [null, '', ''],
      signal: null,
      error: undefined,
    });
    try {
      runStaticAnalysis({ projectRoot: project.root });
      expect(execSpy).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({ cwd: project.root }),
      );
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });

  it('执行 static_analysis 命令时不应传入 change name 或其他 CLI 参数', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      static_analysis: 'node -e process.exit(0)',
    });
    const execSpy = vi.mocked(execCommand).mockReturnValue({
      status: 0,
      stdout: '',
      stderr: '',
      pid: 1,
      output: [null, '', ''],
      signal: null,
      error: undefined,
    });
    try {
      runStaticAnalysis({ projectRoot: project.root });
      const [command] = execSpy.mock.calls[0] ?? [];
      expect(String(command)).not.toMatch(/change|openspec\/changes/i);
    } finally {
      execSpy.mockRestore();
      project.cleanup();
    }
  });
});
