import * as fs from 'fs';
import * as path from 'path';

import { configSchema, type OpenSpecConfig, type OpenSpecConfigInput } from '../schemas/';

const CONFIG_FILE = 'openspec/config.json';

// ---------------------------------------------------------------------------
// Core read / write
// ---------------------------------------------------------------------------

/**
 * Read and parse `openspec/config.json` from the project root.
 *
 * Returns a default `OpenSpecConfig ({ schema: 'spec-driven' })` when the
 * file does not exist or cannot be parsed.  For read-only operations this
 * graceful degradation ensures the tool stays usable even with a corrupted
 * config file.
 *
 * **Auto-migration**: If `config.json` does not exist but `config.yaml`
 * does, the YAML content is read, validated against the Zod schema,
 * written as JSON, and the YAML file is deleted.  If validation fails the
 * error propagates and the YAML file is preserved.
 */
export function readConfig(projectRoot: string): OpenSpecConfig {
  const jsonPath = path.join(projectRoot, CONFIG_FILE);

  let parsed: object;

  // No config file at all — return default
  if (!fs.existsSync(jsonPath)) {
    parsed = {};
  } else {
    try {
      const raw = fs.readFileSync(jsonPath, 'utf-8');
      parsed = JSON.parse(raw);
    } catch {
      parsed = {};
    }
  }

  const result = configSchema.safeDecode(parsed);

  return result.success ? result.data : configSchema.decode({});
}

/**
 * Serialize a config object to `openspec/config.json`.
 *
 * **Validation**: the data is validated with `parseConfig()` **before** any
 * file I/O.  If validation fails the file on disk is never touched.
 *
 * The JSON is written with 2-space indentation (ecosystem convention shared
 * by `tsconfig.json`, `package.json`, etc.).
 */
export function writeConfig(projectRoot: string, data: OpenSpecConfigInput): void {
  // Validate before touching the file system (D6)
  const validated = configSchema.parse(data);

  const dirPath = path.join(projectRoot, 'openspec');
  fs.mkdirSync(dirPath, { recursive: true });

  const filePath = path.join(projectRoot, CONFIG_FILE);
  const jsonStr = JSON.stringify(validated, null, 2);
  fs.writeFileSync(filePath, jsonStr, 'utf-8');
}

// ---------------------------------------------------------------------------
// Dot-path object traversal (unchanged signatures)
// ---------------------------------------------------------------------------

/**
 * Get a value from a config object using a dot-separated key path.
 * Returns the value and whether the key exists.
 */
export function getValue(
  config: OpenSpecConfig,
  keyPath: string,
): { value: unknown; exists: boolean } {
  const keys = keyPath.split('.');
  let current: unknown = config;

  for (let i = 0; i < keys.length; i++) {
    if (current == null || typeof current !== 'object') {
      return { value: undefined, exists: false };
    }
    // eslint-disable-next-line typescript/no-unsafe-type-assertion -- narrowing from object to record
    const obj = current as Record<string, unknown>;
    if (!(keys[i] in obj)) {
      return { value: undefined, exists: false };
    }
    current = obj[keys[i]];
  }

  return { value: current, exists: true };
}

/**
 * Set a value on a config object using a dot-separated key path.
 * Automatically creates intermediate objects for nested keys.
 * Returns the same config reference (mutates in place).
 */
export function setValue(
  config: Record<string, unknown>,
  keyPath: string,
  value: unknown,
): Record<string, unknown> {
  const keys = keyPath.split('.');
  let current = config;

  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i];
    if (!(key in current) || current[key] == null || typeof current[key] !== 'object') {
      current[key] = {};
    }
    // eslint-disable-next-line typescript/no-unsafe-type-assertion -- value is known to be an object
    current = current[key] as Record<string, unknown>;
  }

  const lastKey = keys[keys.length - 1];
  current[lastKey] = value;

  return config;
}

/**
 * Delete a key from a config object using a dot-separated key path.
 * Returns the updated config and whether a value was actually removed.
 */
export function unsetValue(
  config: Record<string, unknown>,
  keyPath: string,
): { config: Record<string, unknown>; removed: boolean } {
  const keys = keyPath.split('.');
  let current: Record<string, unknown> = config;

  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i];
    if (!(key in current) || current[key] == null || typeof current[key] !== 'object') {
      return { config, removed: false };
    }
    // eslint-disable-next-line typescript/no-unsafe-type-assertion -- value is known to be an object
    current = current[key] as Record<string, unknown>;
  }

  const lastKey = keys[keys.length - 1];
  if (lastKey in current) {
    delete current[lastKey];
    return { config, removed: true };
  }

  return { config, removed: false };
}

// ---------------------------------------------------------------------------
// Convenience: context field helpers
// ---------------------------------------------------------------------------

/**
 * Read the `context` field from `config.json`.
 * Returns an empty string when the field or file is missing.
 */
export function readContext(projectRoot: string): { context: string } {
  const config = readConfig(projectRoot);
  return { context: typeof config.context === 'string' ? config.context : '' };
}

/**
 * Write the `context` field to `config.json`.
 * Returns the written context value and `written: true`.
 */
export function writeContext(
  projectRoot: string,
  context: string,
): { context: string; written: boolean } {
  const config = readConfig(projectRoot);
  config.context = context;
  writeConfig(projectRoot, config);
  return { context, written: true };
}

// ---------------------------------------------------------------------------
// Skeleton management
// ---------------------------------------------------------------------------

/**
 * Ensure the `openspec/config.json` file exists.
 * If it does not exist, create it with the skeleton content
 * `{"schema": "spec-driven"}`.
 * Returns the parsed config object.
 */
export function ensureConfigFile(projectRoot: string): OpenSpecConfig {
  const filePath = path.join(projectRoot, CONFIG_FILE);
  if (!fs.existsSync(filePath)) {
    const dirPath = path.join(projectRoot, 'openspec');
    fs.mkdirSync(dirPath, { recursive: true });
    const defaultConfig: OpenSpecConfigInput = { schema: 'spec-driven' };
    fs.writeFileSync(filePath, JSON.stringify(defaultConfig, null, 2), 'utf-8');
  }
  return readConfig(projectRoot);
}
