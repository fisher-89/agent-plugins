import { McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';
import { type z, type ZodType } from 'zod/v4';

import pluginConfig from '../../.claude-plugin/plugin.json';
import { runChangeList } from './commands/change-list';
import { runConfigContext } from './commands/config-context';
import { runConfigGet } from './commands/config-get';
import { runConfigSet } from './commands/config-set';
import { runConfigUnset } from './commands/config-unset';
import { runPhaseCheck } from './commands/phase-check';
import { runPhaseLog } from './commands/phase-log';
import { runPhaseNext } from './commands/phase-next';
import { runTestDetectFrameworks } from './commands/test-detect-frameworks';
import { runTestGetFrameworkConfig } from './commands/test-get-framework-config';
import { runTestResolvePaths } from './commands/test-resolve-paths';
import { queryModel } from './lib/archi-query';
import { validateDsl } from './lib/archi-validate';
import { writeDsl } from './lib/archi-write';
import { runCrossRefCheck } from './lib/c4-cross-ref';
import {
  phaseLogInputSchema,
  phaseLogOutputSchema,
  phaseCheckInputSchema,
  phaseCheckOutputSchema,
  archiQueryInputSchema,
  archiQueryOutputSchema,
  archiValidateInputSchema,
  archiValidateOutputSchema,
  archiWriteInputSchema,
  archiWriteOutputSchema,
  archiCheckInputSchema,
  archiCheckOutputSchema,
  phaseNextInputSchema,
  phaseNextOutputSchema,
  configGetInputSchema,
  configGetOutputSchema,
  configSetInputSchema,
  configSetOutputSchema,
  configUnsetInputSchema,
  configUnsetOutputSchema,
  configContextInputSchema,
  configContextOutputSchema,
  testDetectFrameworksInputSchema,
  testDetectFrameworksOutputSchema,
  testGetFrameworkConfigInputSchema,
  testGetFrameworkConfigOutputSchema,
  testResolvePathsInputSchema,
  testResolvePathsOutputSchema,
  changeListInputSchema,
  changeListOutputSchema,
} from './schemas';

const { name: SERVER_NAME, version: SERVER_VERSION } = pluginConfig;

function resolveProjectRoot(cwd?: string | null): string {
  return cwd || process.env.PROJECT_DIR || process.cwd();
}

function jsonContent<S extends ZodType>(_outputSchema: S, data: z.output<S>) {
  return {
    content: [{ type: 'text' as const, text: JSON.stringify(data) }],
    structuredContent: data,
  };
}

async function main(): Promise<void> {
  const server = new McpServer(
    { name: SERVER_NAME, version: SERVER_VERSION },
    { capabilities: {} },
  );

  server.registerTool(
    'phase_log',
    {
      description:
        'Append an evaluation result entry to eval.json for a given workflow phase. ' +
        'Records the verdict (pass/fail), checklist items, and optional backtrack_to for a change.',
      inputSchema: phaseLogInputSchema,
      outputSchema: phaseLogOutputSchema,
    },
    async (args) => {
      const result = runPhaseLog(args);
      return jsonContent(phaseLogOutputSchema, result);
    },
  );

  server.registerTool(
    'phase_check',
    {
      description:
        '[DEPRECATED] Check if all prior workflow phases have passed evaluation for a given phase. ' +
        'Retained for debugging only — the workflow loop uses phase_next as the single decision point. ' +
        'Runs prerequisite gate check and returns structured result.',
      inputSchema: phaseCheckInputSchema,
      outputSchema: phaseCheckOutputSchema,
    },
    async (args) => {
      const result = runPhaseCheck(args);
      return jsonContent(phaseCheckOutputSchema, result);
    },
  );

  server.registerTool(
    'archi_query',
    {
      description:
        'Query C4 architecture model elements and relationships. Optionally filter by element fully-qualified name.',
      inputSchema: archiQueryInputSchema,
      outputSchema: archiQueryOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = await queryModel(projectRoot, args.element);
      return jsonContent(archiQueryOutputSchema, result);
    },
  );

  server.registerTool(
    'archi_validate',
    {
      description:
        'Validate C4 architecture DSL syntax. Validates the current model or a provided DSL text string.',
      inputSchema: archiValidateInputSchema,
      outputSchema: archiValidateOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = await validateDsl(projectRoot, args.source);
      return jsonContent(archiValidateOutputSchema, result);
    },
  );

  server.registerTool(
    'archi_write',
    {
      description:
        'Validate and write a C4 architecture model file to the models/ directory. Validates DSL before writing.',
      inputSchema: archiWriteInputSchema,
      outputSchema: archiWriteOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = await writeDsl(projectRoot, args.source, args.path);
      return jsonContent(archiWriteOutputSchema, result);
    },
  );

  server.registerTool(
    'archi_check',
    {
      description:
        'Cross-reference validation: check code imports against the C4 architecture model. ' +
        'Detects unmodeled dependencies and unused relationships in changed files.',
      inputSchema: archiCheckInputSchema,
      outputSchema: archiCheckOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const files = args.files
        ? args.files
            .split(',')
            .map((f: string) => f.trim())
            .filter(Boolean)
        : undefined;
      const result = await runCrossRefCheck(projectRoot, {
        staged: !!args.staged,
        files,
      });
      return jsonContent(archiCheckOutputSchema, result);
    },
  );

  server.registerTool(
    'phase_next',
    {
      description:
        'Return the next phase to execute in a PGE workflow. ' +
        'Handles gate check, skip passed phases, retry, backtrack, round limit, and mid-phase interruption. ' +
        'Returns the phase identifier, planner/evaluator agent config, and auto_steps for the skill to execute.',
      inputSchema: phaseNextInputSchema,
      outputSchema: phaseNextOutputSchema,
    },
    async (args) => {
      const result = runPhaseNext({
        change: args.change,
        workflow_type: args.workflow_type,
      });
      return jsonContent(phaseNextOutputSchema, result);
    },
  );

  server.registerTool(
    'config_get',
    {
      description:
        'Read a value from openspec/config.json by dot-separated key path. ' +
        'Returns the value and whether the key exists. When the key does not exist, exists is false.',
      inputSchema: configGetInputSchema,
      outputSchema: configGetOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = runConfigGet({
        key: args.key,
        projectRoot,
      });
      return jsonContent(configGetOutputSchema, result);
    },
  );

  server.registerTool(
    'config_set',
    {
      description:
        'Write a value to openspec/config.json by dot-separated key path. ' +
        'Supports nested key paths (e.g. "test_scripts.unit"). ' +
        'When the file does not exist, creates a skeleton file with schema: spec-driven.',
      inputSchema: configSetInputSchema,
      outputSchema: configSetOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = runConfigSet({
        key: args.key,
        value: args.value,
        projectRoot,
      });
      return jsonContent(configSetOutputSchema, result);
    },
  );

  server.registerTool(
    'config_unset',
    {
      description:
        'Delete a key from openspec/config.json by dot-separated key path. ' +
        'Returns removed: false if the key did not exist.',
      inputSchema: configUnsetInputSchema,
      outputSchema: configUnsetOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = runConfigUnset({
        key: args.key,
        projectRoot,
      });
      return jsonContent(configUnsetOutputSchema, result);
    },
  );

  server.registerTool(
    'config_context',
    {
      description:
        'Read or write the context field in openspec/config.json. ' +
        'Without the context parameter, reads and returns the current context value. ' +
        'With the context parameter, writes the new context value.',
      inputSchema: configContextInputSchema,
      outputSchema: configContextOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = runConfigContext({
        context: args.context,
        projectRoot,
      });
      return jsonContent(configContextOutputSchema, result);
    },
  );

  server.registerTool(
    'test_detect_frameworks',
    {
      description:
        'Detect test framework(s) for given files based on config.json ' +
        'test.framework glob mappings. When files is omitted, auto-scan ' +
        'the project for matching test files. Returns per-file framework ' +
        'detection and a deduplicated framework list.',
      inputSchema: testDetectFrameworksInputSchema,
      outputSchema: testDetectFrameworksOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = runTestDetectFrameworks({
        files: args.files,
        projectRoot,
      });
      return jsonContent(testDetectFrameworksOutputSchema, result);
    },
  );

  server.registerTool(
    'test_get_framework_config',
    {
      description:
        'Get the test and coverage command configuration for a known ' +
        'test framework (jest, vitest, vite-plus, bun, rust). Returns ' +
        'test_cmd, coverage_cmd, coverage_format, and coverage_output.',
      inputSchema: testGetFrameworkConfigInputSchema,
      outputSchema: testGetFrameworkConfigOutputSchema,
    },
    async (args) => {
      const result = runTestGetFrameworkConfig({
        framework: args.framework,
      });
      return jsonContent(testGetFrameworkConfigOutputSchema, result);
    },
  );

  server.registerTool(
    'test_resolve_paths',
    {
      description:
        'Derive unit and integration test file paths from a module list ' +
        '(files or directories). Returns colocated unit test paths per source ' +
        'file and __tests__/<scenario>/ integration test paths.',
      inputSchema: testResolvePathsInputSchema,
      outputSchema: testResolvePathsOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = runTestResolvePaths({ ...args, project_root: projectRoot });
      return jsonContent(testResolvePathsOutputSchema, result);
    },
  );

  server.registerTool(
    'change_list',
    {
      description:
        'List all active (non-archived) changes under openspec/changes/. ' +
        'Returns each change with its artifacts, task progress, and latest eval phase.',
      inputSchema: changeListInputSchema,
      outputSchema: changeListOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = runChangeList({ project_root: projectRoot });
      return jsonContent(changeListOutputSchema, result);
    },
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((e) => {
  process.stderr.write(`Fatal: ${e.message}\n`);
  process.exit(1);
});
