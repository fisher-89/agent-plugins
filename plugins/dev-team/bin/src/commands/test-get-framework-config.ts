// ---------------------------------------------------------------------------
// Framework Command Registry
//
// Hardcoded mapping of known test frameworks to their test and coverage
// commands.  Framework commands are implementation details managed by the
// plugin maintainer, NOT by project config.json (see design decision D1).
// ---------------------------------------------------------------------------

interface FrameworkConfig {
  framework: string;
  test_cmd: string;
  coverage_cmd: string;
  coverage_format: 'istanbul' | 'llvm-cov';
  coverage_output: string;
  coverage_artifacts: string[];
  coverage_cleanup: string[];
}

const FRAMEWORK_REGISTRY: Record<string, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    test_cmd: 'npx jest --verbose',
    coverage_cmd: 'npx jest --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage', '.nyc_output'],
  },
  vitest: {
    framework: 'vitest',
    test_cmd: 'npx vitest run --reporter=verbose',
    coverage_cmd: 'npx vitest run --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage', '.nyc_output'],
  },
  'vite-plus': {
    framework: 'vite-plus',
    test_cmd: 'vp test',
    coverage_cmd: 'vp test --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage', '.nyc_output'],
  },
  bun: {
    framework: 'bun',
    test_cmd: 'bun test',
    coverage_cmd: 'bun test --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage'],
  },
  rust: {
    framework: 'rust',
    test_cmd: 'cargo test',
    coverage_cmd: 'cargo llvm-cov --all --coverage',
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**', 'target/llvm-cov/**'],
    coverage_cleanup: ['coverage', 'target/llvm-cov'],
  },
};

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

  if (!framework || typeof framework !== 'string') {
    throw new Error('Framework name is required and must be a non-empty string');
  }

  const trimmed = framework.trim();
  if (!trimmed) {
    throw new Error('Framework name must not be empty');
  }

  const entry = FRAMEWORK_REGISTRY[trimmed];
  if (!entry) {
    throw new Error(
      `Unknown framework "${trimmed}". Supported frameworks: ${Object.keys(FRAMEWORK_REGISTRY).join(', ')}`,
    );
  }

  // Return a shallow copy to prevent mutation of the registry
  return { ...entry };
}

/**
 * Return the list of all supported framework names.
 */
export function getSupportedFrameworks(): string[] {
  return Object.keys(FRAMEWORK_REGISTRY);
}

/**
 * Return the default glob pattern for a given framework.
 *
 * Used when `test.frameworks` is configured as a string shorthand (e.g.
 * `"vitest"`) and needs to be normalised to a `{glob, framework}` entry.
 */
export function getDefaultGlobForFramework(framework: string): string {
  switch (framework) {
    case 'jest':
    case 'vitest':
    case 'vite-plus':
    case 'bun':
      return '**/*.{test,spec}.{js,ts,jsx,tsx}';
    case 'rust':
      return '**/tests/**/*.rs';
    default:
      throw new Error(`No default glob pattern for unknown framework "${framework}"`);
  }
}
