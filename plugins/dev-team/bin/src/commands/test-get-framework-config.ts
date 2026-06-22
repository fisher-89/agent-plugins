// ---------------------------------------------------------------------------
// Framework Command Registry
//
// Hardcoded mapping of known test frameworks to their test and coverage
// commands.  Framework commands are implementation details managed by the
// plugin maintainer, NOT by project config.json (see design decision D1).
// ---------------------------------------------------------------------------

import { type TestFrameworks } from '../schemas';

interface FrameworkConfig {
  framework: TestFrameworks;
  test_cmd: string;
  coverage_cmd: string;
  coverage_format: 'istanbul' | 'llvm-cov';
  coverage_output: string;
  coverage_artifacts: string[];
  coverage_cleanup: string[];
  default_glob: string;
}

const FRAMEWORK_REGISTRY: Record<TestFrameworks, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    test_cmd: 'npx jest --verbose',
    coverage_cmd: 'npx jest --coverage --coverageReporters=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  vitest: {
    framework: 'vitest',
    test_cmd: 'npx vitest run --reporter=verbose',
    coverage_cmd: 'npx vitest run --coverage --coverage.reporter=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  'vite-plus': {
    framework: 'vite-plus',
    test_cmd: 'vp test',
    coverage_cmd: 'vp test --coverage --coverage.reporter=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  bun: {
    framework: 'bun',
    test_cmd: 'bun test',
    coverage_cmd: 'bun test --coverage --coverageReporters=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  rust: {
    framework: 'rust',
    test_cmd: 'cargo test',
    coverage_cmd: 'cargo llvm-cov --json',
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', 'target/llvm-cov'],
    default_glob: '**/tests/**/*.rs',
  },
};

function isTestFramework(framework: string): framework is TestFrameworks {
  return framework in FRAMEWORK_REGISTRY;
}

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

export interface TestGetFrameworkConfigOptions {
  framework: string;
}

/**
 * Look up a framework's test and coverage command configuration from the
 * hardcoded registry.
 *
 * @returns The framework configuration object.
 * @throws {Error} If the framework name is not in the registry.
 */
export function runTestGetFrameworkConfig(options: TestGetFrameworkConfigOptions): FrameworkConfig {
  const { framework } = options;

  if (!isTestFramework(framework)) {
    throw new Error(
      `Unknown framework "${framework}". Supported frameworks: ${Object.keys(FRAMEWORK_REGISTRY).join(', ')}`,
    );
  }

  const entry = FRAMEWORK_REGISTRY[framework];
  return { ...entry };
}

/**
 * Return the default glob pattern for a given framework.
 *
 * Used to expand a `test.framework` enum value into a `{glob, framework}` mapping
 * for the detection engine.
 */
export function getDefaultGlobForFramework(framework: string): string {
  if (!isTestFramework(framework)) {
    throw new Error(
      `Unknown framework "${framework}". Supported frameworks: ${Object.keys(FRAMEWORK_REGISTRY).join(', ')}`,
    );
  }
  return FRAMEWORK_REGISTRY[framework].default_glob;
}
