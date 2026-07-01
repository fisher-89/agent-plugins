// ---------------------------------------------------------------------------
// Test Framework Registry
//
// Hardcoded mapping of known test frameworks to their test and coverage
// commands.  Framework commands are implementation details managed by the
// plugin maintainer, NOT by project config.json (see design decision D1).
// ---------------------------------------------------------------------------

import { type TestFrameworks } from '../schemas';

export interface FrameworkConfig {
  framework: TestFrameworks;
  test_cmd: string;
  coverage_cmd: string;
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  coverage_artifacts: string[];
  coverage_cleanup: string[];
  default_glob: string;
}

const FRAMEWORK_REGISTRY: Record<TestFrameworks, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    test_cmd: 'npx jest --verbose --json --coverage --coverageReporters=json-summary {files}',
    coverage_cmd: 'npx jest --coverage --coverageReporters=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  vitest: {
    framework: 'vitest',
    test_cmd: 'npx vitest run --reporter=json --coverage --coverage.reporter=json-summary {files}',
    coverage_cmd: 'npx vitest run --coverage --coverage.reporter=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  'vite-plus': {
    framework: 'vite-plus',
    test_cmd: 'vp test --coverage --coverage.reporter=json-summary {files}',
    coverage_cmd: 'vp test --coverage --coverage.reporter=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  bun: {
    framework: 'bun',
    test_cmd: 'bun test --coverage --coverageReporters=json-summary {files}',
    coverage_cmd: 'bun test --coverage --coverageReporters=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  rust: {
    framework: 'rust',
    test_cmd:
      'cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X',
    coverage_cmd: 'cargo llvm-cov --json',
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', 'target/llvm-cov'],
    default_glob: '**/tests/**/*.rs',
  },
  'node-test': {
    framework: 'node-test',
    test_cmd: 'node --test --experimental-test-coverage {files}',
    coverage_cmd: 'node --test --experimental-test-coverage',
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    coverage_artifacts: ['coverage/node-test-output.txt'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.test.{mjs,js,cjs}',
  },
  go: {
    framework: 'go',
    test_cmd: 'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
    coverage_cmd:
      'go test -coverprofile=coverage.out -covermode=atomic ./... && mkdir -p coverage && go tool cover -func=coverage.out > coverage/func-summary.txt',
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    coverage_artifacts: ['coverage/func-summary.txt'],
    coverage_cleanup: ['coverage', 'coverage.out'],
    default_glob: '**/*_test.go',
  },
  pytest: {
    framework: 'pytest',
    test_cmd:
      'pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X',
    coverage_cmd: 'pytest --cov=. --cov-report=json --cov-branch -q',
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    coverage_artifacts: ['coverage.json'],
    coverage_cleanup: ['.coverage', 'htmlcov'],
    default_glob: '**/test_*.py',
  },
};

function isTestFramework(framework: string): framework is TestFrameworks {
  return framework in FRAMEWORK_REGISTRY;
}

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/**
 * Look up a framework's test and coverage command configuration from the
 * hardcoded registry.
 *
 * @param framework - The framework name to look up.
 * @returns The framework configuration object.
 * @throws {Error} If the framework name is not in the registry.
 */
export function getFrameworkConfig(framework: string): FrameworkConfig {
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
 *
 * @param framework - The framework name.
 * @returns The default glob pattern.
 * @throws {Error} If the framework name is not in the registry.
 */
export function getDefaultGlobForFramework(framework: string): string {
  if (!isTestFramework(framework)) {
    throw new Error(
      `Unknown framework "${framework}". Supported frameworks: ${Object.keys(FRAMEWORK_REGISTRY).join(', ')}`,
    );
  }
  return FRAMEWORK_REGISTRY[framework].default_glob;
}
