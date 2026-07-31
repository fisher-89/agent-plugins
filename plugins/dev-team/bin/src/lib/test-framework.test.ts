/**
 * Tests for lib/test-framework — registry placeholders, coverage_output, bun lcov.
 */

import { describe, expect, it } from 'vite-plus/test';

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

describe('getFrameworkConfig -- known frameworks', () => {
  it('istanbul 族框架 coverage_format 为 istanbul', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus'] as const) {
      expect(getFrameworkConfig(fw).coverage_format).toBe('istanbul');
    }
  });

  it('bun coverage_format 为 lcov', () => {
    expect(getFrameworkConfig('bun').coverage_format).toBe('lcov');
  });

  it('应返回所有框架的 coverage_output', () => {
    for (const fw of ALL_EIGHT) {
      expect(getFrameworkConfig(fw).coverage_output).toBeTruthy();
    }
  });
});

describe('getFrameworkConfig -- 未知框架', () => {
  it('未知框架名抛错且错误信息列出八框架', () => {
    expect(() => getFrameworkConfig('unknown')).toThrow(/Unknown framework/);
    try {
      getFrameworkConfig('unknown');
    } catch (e) {
      const msg = (e as Error).message;
      for (const fw of ALL_EIGHT) {
        expect(msg).toContain(fw);
      }
    }
  });
});

describe('getFrameworkConfig -- 模板占位符与 coverage 约定', () => {
  it('jest 模板含 --json --outputFile={results_file} 与 --coverageDirectory={report_dir}', () => {
    const cfg = getFrameworkConfig('jest');
    const cmd = te(cfg.shell.test_execution);
    expect(cmd).toContain('--json');
    expect(cmd).toContain('--outputFile={results_file}');
    expect(cmd).toContain('--coverageDirectory={report_dir}');
    expect(cfg.coverage_output).toBe('coverage-summary.json');
  });

  it('vitest/vite-plus 模板含 {results_file} / {report_dir}', () => {
    for (const fw of ['vitest', 'vite-plus'] as const) {
      const cmd = te(getFrameworkConfig(fw).shell.test_execution);
      expect(cmd).toContain('{results_file}');
      expect(cmd).toContain('{report_dir}');
      expect(getFrameworkConfig(fw).coverage_output).toBe('coverage-summary.json');
    }
  });

  it("bun：coverage_format='lcov'；coverage_output='lcov.info'；config_flag='--config'", () => {
    const cfg = getFrameworkConfig('bun');
    expect(cfg.coverage_format).toBe('lcov');
    expect(cfg.coverage_output).toBe('lcov.info');
    expect(cfg.config_flag).toBe('--config');
  });

  it('go：coverage_output 为相对 reportDir 的 func-summary.txt；模板含 coverprofile 占位符', () => {
    const cfg = getFrameworkConfig('go');
    expect(cfg.coverage_output).toBe('func-summary.txt');
    const cmd = te(cfg.shell.test_execution);
    expect(cmd).toMatch(/\{coverprofile_file\}|\{coverage_file\}/);
  });

  it('rust：llvm-cov --output-path={coverage_file}', () => {
    const cmd = te(getFrameworkConfig('rust').shell.test_execution);
    expect(cmd).toContain('--output-path');
    expect(cmd).toContain('{coverage_file}');
    expect(getFrameworkConfig('rust').coverage_output).toBe('coverage-summary.json');
  });

  it("pytest：--cov-report=json:{coverage_file}；coverage_output='coverage.json'", () => {
    const cfg = getFrameworkConfig('pytest');
    expect(te(cfg.shell.test_execution)).toContain('--cov-report=json:{coverage_file}');
    expect(cfg.coverage_output).toBe('coverage.json');
  });

  it('node-test：模板支持 {files}；coverage_output 为 results.txt', () => {
    const cfg = getFrameworkConfig('node-test');
    expect(te(cfg.shell.test_execution)).toContain('{files}');
    expect(cfg.coverage_output).toBe('results.txt');
  });

  it('coverage_cleanup 字段已删除且不再驱动 suite cwd 清理', () => {
    for (const fw of ALL_EIGHT) {
      const cfg = getFrameworkConfig(fw) as unknown as Record<string, unknown>;
      expect(cfg).not.toHaveProperty('coverage_cleanup');
      expect(cfg.shell as object).not.toHaveProperty('coverage_cleanup');
      expect(cfg.cmd as object).not.toHaveProperty('coverage_cleanup');
    }
  });

  it('八框架 shell/cmd.test_execution 均含报告相关占位符或可由 runner 追加 >', () => {
    const reportPlaceholders = [
      '{results_file}',
      '{coverage_file}',
      '{report_dir}',
      '{coverprofile_file}',
      '{config_args}',
    ];
    for (const fw of ALL_EIGHT) {
      const shell = te(getFrameworkConfig(fw).shell.test_execution);
      const cmd = te(getFrameworkConfig(fw).cmd.test_execution);
      const hasPlaceholder = reportPlaceholders.some((p) => shell.includes(p) || cmd.includes(p));
      expect(hasPlaceholder || shell.includes('{files}') || shell.includes('{config_args}')).toBe(
        true,
      );
    }
  });

  it('bun 模板不含 post-copy / 不写死用户 bunfig 路径', () => {
    const shell = te(getFrameworkConfig('bun').shell.test_execution);
    expect(shell).not.toMatch(/bunfig\.toml/);
    expect(shell).not.toMatch(/post-?copy/i);
    expect(shell).not.toContain('cp ');
  });

  it('coverage_output 均不以 suite cwd 旧路径 coverage/ 为前缀', () => {
    for (const fw of ALL_EIGHT) {
      expect(getFrameworkConfig(fw).coverage_output.startsWith('coverage/')).toBe(false);
    }
  });
});
