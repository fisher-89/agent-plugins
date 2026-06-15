import { writeConfig, ensureConfigFile } from '../lib/config';

export interface ConfigContextOptions {
  context?: string;
  projectRoot?: string;
}

export interface ConfigContextResult {
  context: string;
  written?: boolean;
}

/**
 * Run config_context: read or write the context field in openspec/config.json.
 * - Without `context` param: reads and returns the current context string.
 * - With `context` param: writes the new context value and returns it.
 */
export function runConfigContext(options: ConfigContextOptions): ConfigContextResult {
  const projectRoot = options.projectRoot || process.env.PROJECT_DIR || process.cwd();

  // Ensure the config file exists
  const config = ensureConfigFile(projectRoot);

  if (options.context !== undefined) {
    // Write mode
    config.context = options.context;
    writeConfig(projectRoot, config);

    return {
      context: options.context,
      written: true,
    };
  }

  // Read mode: return the current context (empty string if not set)
  const context = typeof config.context === 'string' ? config.context : '';

  return {
    context,
  };
}
