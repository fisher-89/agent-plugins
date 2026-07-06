// ---------------------------------------------------------------------------
// StrykerJS Configuration Resolver
//
// Detects whether a project already has a stryker.config.* file.  If one
// exists, returns its path with cleanup=false.  Otherwise generates a
// temporary configuration file based on built-in template parameters.
// ---------------------------------------------------------------------------

import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface StrykerConfigResult {
  /** Absolute path to the StrykerJS configuration file */
  configPath: string;
  /** Whether the config file was generated and should be cleaned up after execution */
  cleanup: boolean;
}

// ---------------------------------------------------------------------------
// Existing config detection
// ---------------------------------------------------------------------------

const STRYKER_CONFIG_FILES = [
  'stryker.config.json',
  'stryker.config.mjs',
  'stryker.config.cjs',
  'stryker.config.js',
];

/**
 * Check if a StrykerJS configuration file already exists in the project root.
 *
 * Searches for stryker.config.json, stryker.config.mjs, stryker.config.cjs,
 * and stryker.config.js. Returns the first match found, or null.
 */
function findExistingConfig(projectRoot: string): string | null {
  for (const configFile of STRYKER_CONFIG_FILES) {
    const fullPath = path.resolve(projectRoot, configFile);
    if (fs.existsSync(fullPath)) {
      return fullPath;
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Temporary config generation
// ---------------------------------------------------------------------------
function generateTempConfig(
  projectRoot: string,
  sourceFiles: string[],
  testRunner: string,
): string {
  const randomSuffix = crypto.randomBytes(4).toString('hex');
  const configFileName = `stryker.config.${randomSuffix}.json`;
  const configPath = path.resolve(projectRoot, configFileName);

  // Normalize source file paths to forward-slash, relative to project root
  const normalizedSources = sourceFiles.map((f) =>
    f.replace(/\\/g, '/').replace(path.resolve(projectRoot).replace(/\\/g, '/') + '/', ''),
  );

  const config = {
    $schema:
      'https://raw.githubusercontent.com/stryker-mutator/stryker-js/main/packages/core/schema/stryker-schema.json',
    mutate: normalizedSources,
    testRunner,
    plugins: [resolvePluginPackage(testRunner)],
    reporters: ['json'],
    json: {
      file: 'reports/mutation/mutation.json',
    },
    thresholds: {
      high: 80,
      low: 60,
      break: null,
    },
    timeoutMS: 60000,
    maxTestRunnerReuse: 10,
    concurrency: 4,
    tempDirName: `stryker-tmp-${randomSuffix}`,
  };

  fs.writeFileSync(configPath, JSON.stringify(config, null, 2), 'utf-8');
  return configPath;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Resolve the StrykerJS configuration for a project.
 *
 * If the project already has a stryker.config.* file, returns its path with
 * cleanup=false.  Otherwise generates a temporary configuration file and
 * returns its path with cleanup=true.
 *
 * @param projectRoot - Absolute path to the project root
 * @param sourceFiles - Array of source file paths (relative to projectRoot)
 * @param testFiles  - Array of test file paths (relative to projectRoot)
 * @param framework  - The test framework name ("jest", "vitest", or "vite-plus")
 * @returns StrykerConfigResult with configPath and cleanup flag
 * @throws {Error} If the framework is not supported by StrykerJS
 */
export function resolveStrykerConfig(
  projectRoot: string,
  sourceFiles: string[],
  testFiles: string[],
  framework: string,
): StrykerConfigResult {
  // Check for existing config first
  const existingConfig = findExistingConfig(projectRoot);
  if (existingConfig) {
    return { configPath: existingConfig, cleanup: false };
  }

  // Determine the test runner plugin
  const testRunner = resolveTestRunner(framework);

  // Merge sourceFiles and derived source paths from testFiles
  const allSources = [...new Set([...sourceFiles, ...deriveSourcesFromTestFiles(testFiles)])];

  const configPath = generateTempConfig(projectRoot, allSources, testRunner);
  return { configPath, cleanup: true };
}

/**
 * Resolve the StrykerJS test runner plugin name from a framework identifier.
 *
 * @param framework - The test framework name
 * @returns The StrykerJS runner plugin name
 * @throws {Error} If the framework is not supported
 */
function resolveTestRunner(framework: string): string {
  switch (framework) {
    case 'jest':
      return 'jest-runner';
    case 'vitest':
    case 'vite-plus':
      return 'vitest';
    default:
      throw new Error(
        `Unsupported mutation testing framework "${framework}". Supported: jest, vitest, vite-plus`,
      );
  }
}

/**
 * Resolve the npm plugin package name from the test runner name.
 *
 * StrykerJS v9 renamed the vitest runner plugin from "vitest-runner" to
 * "vitest" at the config level, but the npm package remains
 * `@stryker-mutator/vitest-runner`.  This function maps the simplified
 * runner name back to the correct package name.
 */
function resolvePluginPackage(testRunner: string): string {
  // In StrykerJS v9, the vitest runner registers as "vitest" but the
  // npm package is still "@stryker-mutator/vitest-runner".
  const pluginMap: Record<string, string> = {
    'jest-runner': '@stryker-mutator/jest-runner',
    vitest: '@stryker-mutator/vitest-runner',
  };
  return pluginMap[testRunner] ?? `@stryker-mutator/${testRunner}`;
}

/**
 * Derive source file paths from test file paths.
 *
 * Conventions:
 *   - .test.ts -> .ts
 *   - .spec.ts -> .ts
 *   - etc.
 */
function deriveSourcesFromTestFiles(testFiles: string[]): string[] {
  const sources = new Set<string>();
  for (const tf of testFiles) {
    const posix = tf.replace(/\\/g, '/');
    const src = posix.replace(/\.test\./g, '.').replace(/\.spec\./g, '.');
    if (src !== posix) {
      sources.add(src);
    }
  }
  return Array.from(sources);
}
