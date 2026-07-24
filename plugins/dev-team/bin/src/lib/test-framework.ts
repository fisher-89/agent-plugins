// ---------------------------------------------------------------------------
// Test Framework Registry
//
// Hardcoded mapping of known test frameworks to their test and coverage
// commands.  Framework commands are implementation details managed by the
// plugin maintainer, NOT by project config.json (see design decision D1).
// ---------------------------------------------------------------------------

import { type TestFramework } from '../schemas';

export interface FrameworkConfig {
  framework: TestFramework;
  shell: {
    test_execution: string;
    coverage_cleanup: string[];
    mutation_execution?: string;
  };
  cmd: {
    test_execution: string;
    coverage_cleanup: string[];
    mutation_execution?: string;
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  coverage_artifacts: string[];
  default_glob: string;
  mutation_framework: string | null;
  /** CLI flag for injecting a framework config file; null when unsupported. */
  config_flag: string | null;
}

const FRAMEWORK_REGISTRY: Record<TestFramework, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    shell: {
      test_execution:
        'npx jest --randomize --verbose --json --coverage --coverageReporters=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    cmd: {
      test_execution:
        'npx jest --randomize --verbose --json --coverage --coverageReporters=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
    config_flag: '--config',
  },
  vitest: {
    framework: 'vitest',
    shell: {
      test_execution:
        'npx vitest run --sequence.shuffle --reporter=json --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    cmd: {
      test_execution:
        'npx vitest run --sequence.shuffle --reporter=json --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
    config_flag: '--config',
  },
  'vite-plus': {
    framework: 'vite-plus',
    shell: {
      test_execution:
        'vp test --sequence.shuffle --reporter=json --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    cmd: {
      test_execution:
        'vp test --sequence.shuffle --reporter=json --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
    config_flag: '--config',
  },
  bun: {
    framework: 'bun',
    shell: {
      test_execution: 'bun test --coverage --coverageReporters=json-summary {files}',
      coverage_cleanup: ['coverage'],
    },
    cmd: {
      test_execution: 'bun test --coverage --coverageReporters=json-summary {files}',
      coverage_cleanup: ['coverage'],
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: null,
    config_flag: null,
  },
  rust: {
    framework: 'rust',
    shell: {
      test_execution:
        'cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X',
      coverage_cleanup: ['coverage', 'target/llvm-cov'],
    },
    cmd: {
      test_execution:
        'cargo test & if errorlevel 1 set _X=%errorlevel% & cargo llvm-cov --json --output-path coverage/coverage-summary.json & exit /b %_X%',
      coverage_cleanup: ['coverage', 'target/llvm-cov'],
    },
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    default_glob: '**/tests/**/*.rs',
    mutation_framework: null,
    config_flag: null,
  },
  'node-test': {
    framework: 'node-test',
    shell: {
      test_execution: 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    cmd: {
      test_execution: 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    coverage_artifacts: ['coverage/node-test-output.txt'],
    default_glob: '**/*.test.{mjs,js,cjs}',
    mutation_framework: null,
    config_flag: null,
  },
  go: {
    framework: 'go',
    shell: {
      test_execution: 'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    cmd: {
      test_execution: 'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    coverage_artifacts: ['coverage/func-summary.txt'],
    default_glob: '**/*_test.go',
    mutation_framework: null,
    config_flag: null,
  },
  pytest: {
    framework: 'pytest',
    shell: {
      test_execution: 'pytest -v {files}; pytest --cov=. --cov-report=json --cov-branch -q',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    cmd: {
      test_execution: 'pytest -v {files} && pytest --cov=. --cov-report=json --cov-branch -q',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    coverage_artifacts: ['coverage.json'],
    default_glob: '**/test_*.py',
    mutation_framework: null,
    config_flag: null,
  },
};

function isTestFramework(framework: string): framework is TestFramework {
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
