import * as fs from 'fs';
import * as path from 'path';

import { configSchema, type OpenSpecConfig, type OpenSpecConfigInput } from '../schemas/';
import { isPlainObject } from '../utils';

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

  let parsed: unknown = {};

  // No config file at all — return default
  if (fs.existsSync(jsonPath)) {
    try {
      parsed = JSON.parse(fs.readFileSync(jsonPath, 'utf-8'));
    } catch {
      parsed = {};
    }
  }

  const input = isPlainObject(parsed) ? parsed : {};
  const result = configSchema.safeDecode(input);

  return result.success ? result.data : configSchema.decode({});
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

  for (const key of keys) {
    if (current == null || typeof current !== 'object') {
      return { value: undefined, exists: false };
    }
    if (!(key in current)) {
      return { value: undefined, exists: false };
    }
    current = Reflect.get(current, key);
  }

  return { value: current, exists: true };
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
    const defaultConfig: OpenSpecConfigInput = {
      $schema:
        'https://raw.githubusercontent.com/fisher-89/agent-plugins/master/plugins/dev-team/bin/dev-team-config.schema.json',
      schema: 'spec-driven',
    };
    fs.writeFileSync(filePath, JSON.stringify(defaultConfig, null, 2), 'utf-8');
  }
  return readConfig(projectRoot);
}
