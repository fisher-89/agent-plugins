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
// Temporary config generation
// ---------------------------------------------------------------------------

/** Normalize source paths to forward-slash absolute paths (resolved against rootPath). */
function normalizeSourceFilesForStryker(rootPath: string, sourceFiles: string[]): string[] {
  const absRoot = path.resolve(rootPath);
  return sourceFiles.map((f) => {
    if (f === '') {
      return '';
    }
    const abs =
      path.isAbsolute(f) || /^[A-Za-z]:/.test(f) ? path.resolve(f) : path.resolve(absRoot, f);
    return abs.replace(/\\/g, '/');
  });
}

/**
 * Build jest/vitest runner overlay with absolute configFile when suite.config is set.
 * Relative paths resolve against rootPath (mutation sandbox root).
 */
function buildRunnerConfigOverlay(
  rootPath: string,
  testRunner: string,
  frameworkConfigPath: string | null | undefined,
): Record<string, { configFile: string }> {
  if (!frameworkConfigPath) {
    return {};
  }
  const absConfig = (
    path.isAbsolute(frameworkConfigPath) || /^[A-Za-z]:/.test(frameworkConfigPath)
      ? path.resolve(frameworkConfigPath)
      : path.resolve(rootPath, frameworkConfigPath)
  ).replace(/\\/g, '/');
  return {
    [testRunner]: { configFile: absConfig },
  };
}

function generateTempConfig(
  rootPath: string,
  sourceFiles: string[],
  testRunner: string,
  reportDir: string,
  frameworkConfigPath?: string | null,
): { configPath: string; tempDirPath: string } {
  const randomSuffix = crypto.randomBytes(4).toString('hex');
  const configPath = path.resolve(rootPath, `stryker.config.${randomSuffix}.json`);
  const tempDirPath = path.resolve(rootPath, '.stryker-tmp');
  const normalizedSources = normalizeSourceFilesForStryker(rootPath, sourceFiles);
  const mutationFileAbs = path.resolve(reportDir, 'mutation.json').replace(/\\/g, '/');

  const config = {
    $schema: 'node_modules/@stryker-mutator/core/schema/stryker-schema.json',
    mutate: normalizedSources,
    testRunner,
    plugins: [resolvePluginPackage(testRunner)],
    ...buildRunnerConfigOverlay(rootPath, testRunner, frameworkConfigPath),
    ignoreStatic: true,
    reporters: ['json', 'html'],
    jsonReporter: { fileName: mutationFileAbs },
    timeoutMS: 10000,
  };

  fs.writeFileSync(configPath, JSON.stringify(config, null, 2), 'utf-8');
  return { configPath, tempDirPath };
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Resolve the StrykerJS configuration for a project.
 *
 * Always generates a temporary configuration in the mutation sandbox root
 * (`rootPath`, typically suite root — not suite cwd) with
 * `jsonReporter.fileName` as an absolute path to `{reportDir}/mutation.json`.
 * The temp config is deleted by the caller after the run; user long-lived
 * Stryker configs are never modified.
 *
 * @param rootPath - Absolute mutation sandbox root (suite root when cwd is under root)
 * @param sourceFiles - Array of source file paths (relative to rootPath, or absolute)
 * @param framework  - The test framework name ("jest", "vitest", or "vite-plus")
 * @param reportDir  - Absolute plan report directory (mutation.json lands here)
 * @param frameworkConfigPath - Absolute (or resolvable) path to suite `tests[].config`, if any
 * @throws {Error} If the framework is not supported by StrykerJS
 */
export function resolveStrykerConfig(
  rootPath: string,
  sourceFiles: string[],
  framework: string,
  reportDir: string,
  frameworkConfigPath?: string | null,
): { configPath: string; tempDirPath: string } {
  const testRunner = resolveTestRunner(framework);
  const { configPath, tempDirPath } = generateTempConfig(
    rootPath,
    sourceFiles,
    testRunner,
    reportDir,
    frameworkConfigPath,
  );
  return { configPath, tempDirPath };
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
      return 'jest';
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
    jest: '@stryker-mutator/jest-runner',
    vitest: '@stryker-mutator/vitest-runner',
  };
  return pluginMap[testRunner] ?? `@stryker-mutator/${testRunner}`;
}
