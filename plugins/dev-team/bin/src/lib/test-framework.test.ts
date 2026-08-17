/**
 * Tests for lib/test-framework — registry literals, version gating, detectFrameworkVersion.
 * Strengthened for mutation-score-below-60 (AC-1).
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import * as execCommandMod from './exec-command';
import * as testFramework from './test-framework';
import { getFrameworkConfig } from './test-framework';

type TestExecutionBuilder = ReturnType<typeof getFrameworkConfig>['shell']['test_execution'];
function te(builder: TestExecutionBuilder, version = '99.0.0'): string {
  return builder(version);
}

const ALL_EIGHT = [
  'jest',
  'vitest',
  'vite-plus',
  'bun',
  'rust',
  'node-test',
  'go',
  'pytest',
] as const;

const VERSION_COMMANDS: Record<(typeof ALL_EIGHT)[number], string> = {
  jest: 'npx jest --version',
  vitest: 'npx vitest --version',
  'vite-plus': 'vp --version',
  bun: 'bun --version',
  rust: 'cargo --version',
  'node-test': 'node --version',
  go: 'go version',
  pytest: 'pytest --version',
};

const COVERAGE_FORMATS: Record<(typeof ALL_EIGHT)[number], string> = {
  jest: 'istanbul',
  vitest: 'istanbul',
  'vite-plus': 'istanbul',
  bun: 'lcov',
  rust: 'llvm-cov',
  'node-test': 'node-test',
  go: 'go-cover',
  pytest: 'coverage-py',
};

const COVERAGE_OUTPUTS: Record<(typeof ALL_EIGHT)[number], string> = {
  jest: 'coverage-summary.json',
  vitest: 'coverage-summary.json',
  'vite-plus': 'coverage-summary.json',
  bun: 'lcov.info',
  rust: 'coverage-summary.json',
  'node-test': 'results.txt',
  go: 'func-summary.txt',
  pytest: 'coverage.json',
};

const DEFAULT_GLOBS: Record<(typeof ALL_EIGHT)[number], string> = {
  jest: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  vitest: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  'vite-plus': '**/*.{test,spec}.{js,ts,jsx,tsx}',
  bun: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  rust: '**/tests/**/*.rs',
  'node-test': '**/*.test.{mjs,js,cjs}',
  go: '**/*_test.go',
  pytest: '**/test_*.py',
};

const CONFIG_FLAGS: Record<(typeof ALL_EIGHT)[number], string | null> = {
  jest: '--config',
  vitest: '--config',
  'vite-plus': '--config',
  bun: '--config',
  rust: null,
  'node-test': null,
  go: null,
  pytest: null,
};

describe('getFrameworkConfig -- 字面量杀伤', () => {
  it('八框架 version_command 精确等于约定命令', () => {
    for (const fw of ALL_EIGHT) {
      expect(getFrameworkConfig(fw).version_command).toBe(VERSION_COMMANDS[fw]);
    }
  });

  it('八框架 coverage_format / coverage_output / default_glob / config_flag 精确断言', () => {
    for (const fw of ALL_EIGHT) {
      const cfg = getFrameworkConfig(fw);
      expect(cfg.coverage_format).toBe(COVERAGE_FORMATS[fw]);
      expect(cfg.coverage_output).toBe(COVERAGE_OUTPUTS[fw]);
      expect(cfg.default_glob).toBe(DEFAULT_GLOBS[fw]);
      expect(cfg.config_flag).toBe(CONFIG_FLAGS[fw]);
      expect(cfg.framework).toBe(fw);
    }
  });

  it('jest/vitest/vite-plus 的 shell 与 cmd mutation_execution 精确为 npx --prefix（无 -p 自动安装）', () => {
    const expected = 'npx --prefix "{prefix}" stryker run "{config}"';
    for (const fw of ['jest', 'vitest', 'vite-plus'] as const) {
      const cfg = getFrameworkConfig(fw);
      expect(cfg.shell.mutation_execution?.('99.0.0')).toBe(expected);
      expect(cfg.cmd.mutation_execution?.('99.0.0')).toBe(expected);
      expect(cfg.shell.mutation_execution?.('99.0.0')).not.toMatch(/(^|\s)-p(\s|$)/);
      expect(cfg.cmd.mutation_execution?.('99.0.0')).not.toMatch(/(^|\s)-p(\s|$)/);
      expect(cfg.shell.mutation_execution?.('99.0.0')).toContain('--prefix');
      expect(cfg.shell.mutation_execution?.('99.0.0')).toContain('{prefix}');
    }
  });

  it('bun/rust/go/pytest/node-test 无 mutation_execution', () => {
    for (const fw of ['bun', 'rust', 'go', 'pytest', 'node-test'] as const) {
      const cfg = getFrameworkConfig(fw);
      expect(cfg.shell.mutation_execution).toBeUndefined();
      expect(cfg.cmd.mutation_execution).toBeUndefined();
    }
  });
});

describe('getFrameworkConfig -- 模板字面量', () => {
  it('jest shell/cmd 在 version=29.5.0 时含完整片段', () => {
    const cfg = getFrameworkConfig('jest');
    for (const builder of [cfg.shell.test_execution, cfg.cmd.test_execution]) {
      const cmd = te(builder, '29.5.0');
      expect(cmd).toContain('--randomize');
      expect(cmd).toContain('--no-verbose');
      expect(cmd).toContain('--json');
      expect(cmd).toContain('--outputFile="{results_file}"');
      expect(cmd).toContain('--silent');
      expect(cmd).toContain('--coverage');
      expect(cmd).toContain('--coverageDirectory="{report_dir}"');
      expect(cmd).toContain('--coverageReporters=json-summary');
      expect(cmd).toContain('{config_args}');
      expect(cmd).toContain('{files}');
      expect(cmd.startsWith('npx jest')).toBe(true);
    }
  });

  it('vitest / vite-plus shell 与 cmd 精确含 shuffle / reporter / coverage 片段', () => {
    const vitestShell = te(getFrameworkConfig('vitest').shell.test_execution);
    const vitestCmd = te(getFrameworkConfig('vitest').cmd.test_execution);
    for (const cmd of [vitestShell, vitestCmd]) {
      expect(cmd).toContain('npx vitest run');
      expect(cmd).toContain('--sequence.shuffle');
      expect(cmd).toContain('--reporter=json');
      expect(cmd).toContain('--outputFile="{results_file}"');
      expect(cmd).toContain('--coverage.reportsDirectory="{report_dir}"');
      expect(cmd).toContain('--coverage.reporter=json-summary');
    }

    const vpShell = te(getFrameworkConfig('vite-plus').shell.test_execution);
    const vpCmd = te(getFrameworkConfig('vite-plus').cmd.test_execution);
    for (const cmd of [vpShell, vpCmd]) {
      expect(cmd).toContain('vp test');
      expect(cmd).toContain('--sequence.shuffle');
      expect(cmd).toContain('--reporter=json');
      expect(cmd).toContain('--outputFile="{results_file}"');
      expect(cmd).toContain('--coverage.reportsDirectory="{report_dir}"');
      expect(cmd).toContain('--coverage.reporter=json-summary');
    }
  });

  it('bun shell/cmd 精确为 bun {config_args} test --coverage {files}', () => {
    const cfg = getFrameworkConfig('bun');
    expect(te(cfg.shell.test_execution)).toBe('bun {config_args} test --coverage {files}');
    expect(te(cfg.cmd.test_execution)).toBe('bun {config_args} test --coverage {files}');
  });

  it('go shell 含 coverprofile 链；cmd 含 & / exit /b 链', () => {
    const cfg = getFrameworkConfig('go');
    const shell = te(cfg.shell.test_execution);
    expect(shell).toContain(
      'go test -json -coverprofile="{coverprofile_file}" -covermode=atomic {directory}',
    );
    expect(shell).toContain('go tool cover -func="{coverprofile_file}" > "{coverage_file}"');
    expect(shell).toContain('; _X=$?;');
    expect(shell).toContain('exit $_X');

    const cmd = te(cfg.cmd.test_execution);
    expect(cmd).toContain(
      'go test -json -coverprofile="{coverprofile_file}" -covermode=atomic {directory}',
    );
    expect(cmd).toContain('& if errorlevel 1 set _X=%errorlevel%');
    expect(cmd).toContain('go tool cover -func="{coverprofile_file}" > "{coverage_file}"');
    expect(cmd).toContain('exit /b %_X%');
  });

  it('rust shell 含 cargo test; 与 llvm-cov；pytest shell 用 ;、cmd 用 &&', () => {
    const rustShell = te(getFrameworkConfig('rust').shell.test_execution);
    expect(rustShell).toContain('cargo test;');
    expect(rustShell).toContain('cargo llvm-cov --json --output-path "{coverage_file}"');

    const rustCmd = te(getFrameworkConfig('rust').cmd.test_execution);
    expect(rustCmd).toContain('cargo test &');
    expect(rustCmd).toContain('cargo llvm-cov --json --output-path "{coverage_file}"');
    expect(rustCmd).toContain('exit /b %_X%');

    const pyShell = te(getFrameworkConfig('pytest').shell.test_execution);
    expect(pyShell).toBe(
      'pytest -v {files}; pytest --cov=. --cov-report="json:{coverage_file}" --cov-branch -q',
    );
    const pyCmd = te(getFrameworkConfig('pytest').cmd.test_execution);
    expect(pyCmd).toBe(
      'pytest -v {files} && pytest --cov=. --cov-report="json:{coverage_file}" --cov-branch -q',
    );
  });

  it('node-test shell/cmd 精确含 node --test --experimental-test-coverage {files}', () => {
    const cfg = getFrameworkConfig('node-test');
    expect(te(cfg.shell.test_execution)).toBe('node --test --experimental-test-coverage {files}');
    expect(te(cfg.cmd.test_execution)).toBe('node --test --experimental-test-coverage {files}');
  });
});

describe('getFrameworkConfig -- jest 版本门控', () => {
  it('version=29.5.0 → 含 --randomize；29.4.9 / 29.4.0 / 空 / 非法 / 仅 major → 不含', () => {
    const builder = getFrameworkConfig('jest').shell.test_execution;
    expect(te(builder, '29.5.0')).toContain('--randomize');
    for (const v of ['29.4.9', '29.4.0', '', 'not-a-version', '29'] as const) {
      expect(te(builder, v)).not.toContain('--randomize');
    }
  });

  it('version=30.0.0 / 29.5.1 → 含 --randomize；major 更大时仍注入', () => {
    const builder = getFrameworkConfig('jest').shell.test_execution;
    expect(te(builder, '30.0.0')).toContain('--randomize');
    expect(te(builder, '29.5.1')).toContain('--randomize');
    expect(te(builder, '100.0.0')).toContain('--randomize');
    // patch 边界：29.5.0 恰好含；同 major/minor 更小 patch 不含
    expect(te(builder, '29.5.0')).toContain('--randomize');
    expect(te(builder, '29.4.99')).not.toContain('--randomize');
  });

  it('cmd.test_execution 与 shell 对同一 version 的 --randomize 行为一致', () => {
    const cfg = getFrameworkConfig('jest');
    expect(te(cfg.cmd.test_execution, '29.5.0')).toContain('--randomize');
    expect(te(cfg.cmd.test_execution, '29.4.9')).not.toContain('--randomize');
  });
});

describe('getFrameworkConfig -- 未知框架与浅拷贝', () => {
  it("getFrameworkConfig('unknown') / '' / 'jest ' 抛错，message 含 Unknown framework 且完整列出八键名", () => {
    for (const bad of ['unknown', '', 'jest '] as const) {
      expect(() => getFrameworkConfig(bad)).toThrow(/Unknown framework/);
      try {
        getFrameworkConfig(bad);
      } catch (e) {
        const msg = (e as Error).message;
        expect(msg).toContain('Unknown framework');
        for (const fw of ALL_EIGHT) {
          expect(msg).toContain(fw);
        }
        expect(msg).toContain(', ');
      }
    }
  });

  it('修改返回对象的 coverage_output 不影响再次 getFrameworkConfig 的值', () => {
    const a = getFrameworkConfig('bun');
    a.coverage_output = 'mutated.info';
    expect(getFrameworkConfig('bun').coverage_output).toBe('lcov.info');
  });
});

describe('getFrameworkConfig -- mutation_execution 模板', () => {
  const MUTATION_TEMPLATE = 'npx --prefix "{prefix}" stryker run "{config}"';

  it('jest / vitest / vite-plus 的 shell 与 cmd mutation_execution 精确等于模板（AC-6）', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus'] as const) {
      const cfg = getFrameworkConfig(fw);
      expect(cfg.shell.mutation_execution?.('99.0.0')).toBe(MUTATION_TEMPLATE);
      expect(cfg.cmd.mutation_execution?.('99.0.0')).toBe(MUTATION_TEMPLATE);
    }
  });

  it('mutation 模板含 --prefix 不得被误判为 -p token（AC-6）', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus'] as const) {
      const cfg = getFrameworkConfig(fw);
      const shell = cfg.shell.mutation_execution?.('99.0.0') ?? '';
      const cmd = cfg.cmd.mutation_execution?.('99.0.0') ?? '';
      expect(shell).not.toMatch(/(^|\s)-p(\s|$)/);
      expect(cmd).not.toMatch(/(^|\s)-p(\s|$)/);
      expect(shell).toContain('--prefix');
    }
  });

  it('framework 为 undefined / null 强转调用时抛错', () => {
    for (const bad of [undefined, null] as const) {
      expect(() => getFrameworkConfig(bad as unknown as string)).toThrow(/Unknown framework/);
    }
  });

  it('framework 超长（>1000 chars）或含 \\n / emoji → 抛 Unknown framework', () => {
    const long = `jest${'x'.repeat(1001)}`;
    expect(() => getFrameworkConfig(long)).toThrow(/Unknown framework/);
    expect(() => getFrameworkConfig('jest\n')).toThrow(/Unknown framework/);
    expect(() => getFrameworkConfig('jest🧪')).toThrow(/Unknown framework/);
  });

  it('mutation_execution 传入 version 为空 / 0.0.0 / 99.0.0 → 返回值与 version 无关', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus'] as const) {
      const cfg = getFrameworkConfig(fw);
      const builder = cfg.shell.mutation_execution!;
      expect(builder('')).toBe(MUTATION_TEMPLATE);
      expect(builder('0.0.0')).toBe(MUTATION_TEMPLATE);
      expect(builder('99.0.0')).toBe(MUTATION_TEMPLATE);
    }
  });

  it('修改返回对象的 version_command 不影响再次 getFrameworkConfig 的 mutation 模板（浅拷贝）', () => {
    const a = getFrameworkConfig('vitest');
    a.version_command = 'mutated';
    expect(getFrameworkConfig('vitest').shell.mutation_execution?.('99.0.0')).toBe(
      MUTATION_TEMPLATE,
    );
    expect(getFrameworkConfig('vitest').version_command).toBe('npx vitest --version');
  });
});

describe('detectFrameworkVersion', () => {
  type ExecReturn = ReturnType<typeof execCommandMod.execCommand>;

  function stubExec(result: Partial<ExecReturn>): void {
    vi.spyOn(execCommandMod, 'execCommand').mockReturnValue({
      status: 0,
      stdout: '',
      stderr: '',
      pid: 1,
      output: ['', '', ''],
      signal: null,
      ...result,
    } as ExecReturn);
  }

  beforeEach(() => {
    // 解除全局 spy，直接测生产 detectFrameworkVersion
    vi.mocked(testFramework.detectFrameworkVersion).mockRestore();
  });

  afterEach(() => {
    vi.spyOn(execCommandMod, 'execCommand').mockRestore();
  });

  it('status=0、stdout=1.2.3 → 返回 1.2.3；调用含 version_command、cwd、timeout:30000', () => {
    stubExec({ status: 0, stdout: '1.2.3', stderr: '' });

    expect(testFramework.detectFrameworkVersion('jest', '/tmp/proj')).toBe('1.2.3');
    expect(execCommandMod.execCommand).toHaveBeenCalledWith('npx jest --version', {
      cwd: '/tmp/proj',
      timeout: 30_000,
    });
  });

  it('semver 在 stderr（stdout 空）→ 仍抽取；stdout/stderr 拼接后取首个 x.y.z', () => {
    stubExec({ status: 0, stdout: '', stderr: 'vitest/1.0.0 node' });
    expect(testFramework.detectFrameworkVersion('vitest', '/cwd')).toBe('1.0.0');

    stubExec({ status: 0, stdout: 'first 2.3.4 then', stderr: 'also 9.9.9' });
    expect(testFramework.detectFrameworkVersion('bun', '/cwd')).toBe('2.3.4');
  });

  it('status≠0 → 空串；status=0 但无 semver → 空串', () => {
    stubExec({ status: 1, stdout: '1.2.3', stderr: '' });
    expect(testFramework.detectFrameworkVersion('go', '/cwd')).toBe('');

    stubExec({ status: 0, stdout: 'no version here', stderr: 'still none' });
    expect(testFramework.detectFrameworkVersion('go', '/cwd')).toBe('');
  });

  it('stdout=v29.5.0-beta → 29.5.0；空输出 → 空串；超长噪声夹带 0.0.1 → 抽取', () => {
    stubExec({ status: 0, stdout: 'v29.5.0-beta', stderr: '' });
    expect(testFramework.detectFrameworkVersion('jest', '/cwd')).toBe('29.5.0');

    stubExec({ status: 0, stdout: '', stderr: '' });
    expect(testFramework.detectFrameworkVersion('jest', '/cwd')).toBe('');

    const noise = `${'x'.repeat(1200)}0.0.1${'y'.repeat(200)}`;
    stubExec({ status: 0, stdout: noise, stderr: '' });
    expect(testFramework.detectFrameworkVersion('pytest', '/cwd')).toBe('0.0.1');
  });

  it('未知 framework → 抛错且不调用 exec', () => {
    const spy = vi.spyOn(execCommandMod, 'execCommand');
    expect(() => testFramework.detectFrameworkVersion('unknown', '/cwd')).toThrow(
      /Unknown framework/,
    );
    expect(spy).not.toHaveBeenCalled();
  });

  it('stdout/stderr 为 null/undefined 时按空串合并不抛', () => {
    stubExec({
      status: 0,
      stdout: null as unknown as string,
      stderr: undefined as unknown as string,
    });
    expect(testFramework.detectFrameworkVersion('rust', '/cwd')).toBe('');
  });
});

/**
 * Static registry mutants are only attributed to importers at module-load time.
 * Re-import after resetModules so literal/object/arrow mutants are covered by these tests.
 */
describe('getFrameworkConfig -- resetModules 杀静态变异', () => {
  afterEach(() => {
    vi.resetModules();
    vi.restoreAllMocks();
  });

  async function loadGetConfig() {
    vi.resetModules();
    const mod = await import('./test-framework');
    return mod.getFrameworkConfig;
  }

  it('八框架 registry 字段与 shell/cmd 模板经动态 import 精确相等', async () => {
    const getFrameworkConfig = await loadGetConfig();

    const jestCfg = getFrameworkConfig('jest');
    expect(jestCfg).toMatchObject({
      framework: 'jest',
      version_command: 'npx jest --version',
      coverage_format: 'istanbul',
      coverage_output: 'coverage-summary.json',
      default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
      config_flag: '--config',
    });
    expect(jestCfg.shell.mutation_execution?.('99.0.0')).toBe(
      'npx --prefix "{prefix}" stryker run "{config}"',
    );
    expect(jestCfg.cmd.mutation_execution?.('99.0.0')).toBe(
      'npx --prefix "{prefix}" stryker run "{config}"',
    );
    expect(typeof jestCfg.shell.test_execution).toBe('function');
    expect(typeof jestCfg.cmd.test_execution).toBe('function');
    expect(typeof jestCfg.shell.mutation_execution).toBe('function');
    expect(typeof jestCfg.cmd.mutation_execution).toBe('function');
    expect(jestCfg.shell.test_execution('29.5.0')).toBe(
      'npx jest --randomize --no-verbose --json --outputFile="{results_file}" --silent --coverage --coverageDirectory="{report_dir}" --coverageReporters=json-summary {config_args} {files}',
    );
    expect(jestCfg.shell.test_execution('29.4.9')).toBe(
      'npx jest --no-verbose --json --outputFile="{results_file}" --silent --coverage --coverageDirectory="{report_dir}" --coverageReporters=json-summary {config_args} {files}',
    );
    expect(jestCfg.cmd.test_execution('29.5.0')).toBe(jestCfg.shell.test_execution('29.5.0'));
    expect(jestCfg.cmd.test_execution('29.4.9')).toBe(jestCfg.shell.test_execution('29.4.9'));

    const vitestCfg = getFrameworkConfig('vitest');
    expect(vitestCfg.shell.test_execution('1.0.0')).toBe(
      'npx vitest run --sequence.shuffle --reporter=json --outputFile="{results_file}" --silent --coverage --coverage.reportsDirectory="{report_dir}" --coverage.reporter=json-summary {config_args} {files}',
    );
    expect(vitestCfg.cmd.test_execution('1.0.0')).toBe(vitestCfg.shell.test_execution('1.0.0'));
    expect(vitestCfg.shell.mutation_execution?.('99.0.0')).toBe(
      'npx --prefix "{prefix}" stryker run "{config}"',
    );
    expect(vitestCfg.cmd.mutation_execution?.('99.0.0')).toBe(
      'npx --prefix "{prefix}" stryker run "{config}"',
    );
    expect(vitestCfg.shell.mutation_execution?.('99.0.0')).not.toMatch(/(^|\s)-p(\s|$)/);

    const vp = getFrameworkConfig('vite-plus');
    expect(vp.version_command).toBe('vp --version');
    expect(vp.shell.mutation_execution?.('99.0.0')).toBe(
      'npx --prefix "{prefix}" stryker run "{config}"',
    );
    expect(vp.shell.test_execution('1.0.0')).toBe(
      'vp test --sequence.shuffle --reporter=json --outputFile="{results_file}" --silent --coverage --coverage.reportsDirectory="{report_dir}" --coverage.reporter=json-summary {config_args} {files}',
    );
    expect(vp.cmd.test_execution('1.0.0')).toBe(vp.shell.test_execution('1.0.0'));

    const bun = getFrameworkConfig('bun');
    expect(bun.shell.test_execution('1.0.0')).toBe('bun {config_args} test --coverage {files}');
    expect(bun.cmd.test_execution('1.0.0')).toBe(bun.shell.test_execution('1.0.0'));
    expect(bun.shell.mutation_execution).toBeUndefined();

    const rust = getFrameworkConfig('rust');
    expect(rust.shell.test_execution('1.0.0')).toBe(
      'cargo test; _X=$?; cargo llvm-cov --json --output-path "{coverage_file}"; exit $_X',
    );
    expect(rust.cmd.test_execution('1.0.0')).toBe(
      'cargo test & if errorlevel 1 set _X=%errorlevel% & cargo llvm-cov --json --output-path "{coverage_file}" & exit /b %_X%',
    );

    const nodeTest = getFrameworkConfig('node-test');
    expect(nodeTest.shell.test_execution('1.0.0')).toBe(
      'node --test --experimental-test-coverage {files}',
    );
    expect(nodeTest.cmd.test_execution('1.0.0')).toBe(nodeTest.shell.test_execution('1.0.0'));

    const go = getFrameworkConfig('go');
    expect(go.shell.test_execution('1.0.0')).toBe(
      'go test -json -coverprofile="{coverprofile_file}" -covermode=atomic {directory}; _X=$?; go tool cover -func="{coverprofile_file}" > "{coverage_file}"; exit $_X',
    );
    expect(go.cmd.test_execution('1.0.0')).toBe(
      'go test -json -coverprofile="{coverprofile_file}" -covermode=atomic {directory} & if errorlevel 1 set _X=%errorlevel% & go tool cover -func="{coverprofile_file}" > "{coverage_file}" & exit /b %_X%',
    );

    const py = getFrameworkConfig('pytest');
    expect(py.shell.test_execution('1.0.0')).toBe(
      'pytest -v {files}; pytest --cov=. --cov-report="json:{coverage_file}" --cov-branch -q',
    );
    expect(py.cmd.test_execution('1.0.0')).toBe(
      'pytest -v {files} && pytest --cov=. --cov-report="json:{coverage_file}" --cov-branch -q',
    );
  });

  it('jest 版本门控：双位数 minor/patch 与 major/minor/patch 分支', async () => {
    const getFrameworkConfig = await loadGetConfig();
    const builder = getFrameworkConfig('jest').shell.test_execution;

    // 双位 minor：杀 /(\d+)\.(\d)\.(\d+)/ 只吃一位 minor（误解析成 29.1.0 会丢 --randomize）
    expect(builder('29.10.0')).toContain('--randomize');
    // 双位 patch：杀 /(\d+)\.(\d+)\.(\d)/ 只吃一位 patch
    expect(builder('29.5.10')).toContain('--randomize');

    // major 相等走 minor：29.6.0 ≥ 29.5.0；若 if(aMaj!==bMaj) 被逼成 true 会误判
    expect(builder('29.6.0')).toContain('--randomize');
    expect(builder('29.4.0')).not.toContain('--randomize');

    // minor 相等走 patch：29.5.1 ≥；29.5.0 恰等于；若跳过 minor 分支会用 patch 误判 29.6.0 vs 29.5.1
    expect(builder('29.5.1')).toContain('--randomize');
    expect(builder('29.5.0')).toContain('--randomize');
    // patch 不足：杀 return true
    expect(builder('29.4.99')).not.toContain('--randomize');
    // 同 major/minor、更小 patch（相对 29.5.0 门槛）
    expect(builder('29.5.0').includes('--randomize')).toBe(true);
    // 用 detect 路径验证 extractSemver 双位
    vi.resetModules();
    vi.doMock('./exec-command', () => ({
      execCommand: () => ({
        status: 0,
        stdout: '29.10.0',
        stderr: '',
        pid: 1,
        output: ['', '29.10.0', ''],
        signal: null,
      }),
    }));
    const mod = await import('./test-framework');
    expect(mod.detectFrameworkVersion('jest', '/cwd')).toBe('29.10.0');

    vi.resetModules();
    vi.doMock('./exec-command', () => ({
      execCommand: () => ({
        status: 0,
        stdout: '1.2.34-beta',
        stderr: '',
        pid: 1,
        output: ['', '1.2.34-beta', ''],
        signal: null,
      }),
    }));
    const mod2 = await import('./test-framework');
    expect(mod2.detectFrameworkVersion('vitest', '/cwd')).toBe('1.2.34');
  });
});
