import { McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';
import { type z, type ZodType } from 'zod/v4';

import pluginConfig from '../../.claude-plugin/plugin.json';
import { runChangeList } from './commands/change-list';
import { runConfigGet } from './commands/config-get';
import { runPhaseLog } from './commands/phase-log';
import { runPhaseNext } from './commands/phase-next';
import { runTestDetectFrameworks } from './commands/test-detect-frameworks';
import { runTestResolvePaths } from './commands/test-resolve-paths';
import { queryModel } from './lib/archi-query';
import { validateDsl } from './lib/archi-validate';
import { writeDsl } from './lib/archi-write';
import { runCrossRefCheck } from './lib/c4-cross-ref';
import { initProjectRootFromMcp } from './lib/project-root';
import {
  phaseLogInputSchema,
  phaseLogOutputSchema,
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
  testDetectFrameworksInputSchema,
  testDetectFrameworksOutputSchema,
  testResolvePathsInputSchema,
  testResolvePathsOutputSchema,
  changeListInputSchema,
  changeListOutputSchema,
} from './schemas';
import { getProjectDir } from './utils';

const { name: SERVER_NAME, version: SERVER_VERSION } = pluginConfig;

function resolveProjectRoot(cwd?: string | null): string {
  return cwd || getProjectDir();
}

function jsonContent<S extends ZodType>(_outputSchema: S, data: z.output<S>) {
  return {
    content: [{ type: 'text' as const, text: JSON.stringify(data) }],
    structuredContent: data,
  };
}

function registerPhaseLogTool(server: McpServer): void {
  server.registerTool(
    'phase_log',
    {
      description: 'Append an evaluation result entry to eval.json for a given workflow phase. ',
      inputSchema: phaseLogInputSchema,
      outputSchema: phaseLogOutputSchema,
    },
    async (args) => {
      const result = runPhaseLog(args);
      return jsonContent(phaseLogOutputSchema, result);
    },
  );
}

function registerArchiQueryTool(server: McpServer): void {
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
}

function registerArchiValidateTool(server: McpServer): void {
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
}

function registerArchiWriteTool(server: McpServer): void {
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
}

function registerArchiCheckTool(server: McpServer): void {
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
}

function registerPhaseNextTool(server: McpServer): void {
  server.registerTool(
    'phase_next',
    {
      description:
        'Return the next phase to execute in a PGE workflow. ' +
        'Handles gate check, skip passed phases, retry, backtrack, round limit, and mid-phase interruption. ' +
        'Returns the phase identifier and planner/evaluator agent config for the skill to execute.',
      inputSchema: phaseNextInputSchema,
      outputSchema: phaseNextOutputSchema,
    },
    async (args) => {
      const result = runPhaseNext({
        change: args.change,
      });
      return jsonContent(phaseNextOutputSchema, result);
    },
  );
}

function registerConfigGetTool(server: McpServer): void {
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
}

function registerTestDetectFrameworksTool(server: McpServer): void {
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
}

function registerTestResolvePathsTool(server: McpServer): void {
  server.registerTool(
    'test_resolve_paths',
    {
      description:
        'Derive unit and integration test file paths from a module list ' +
        '(files or directories). Three modes: (1) modules is an empty array — ' +
        'directories are auto-detected from config.json test configuration; ' +
        '(2) modules is a non-empty array — paths are filtered by test config ' +
        'scope before resolving; (3) modules is "git-change" — reads git diff ' +
        'HEAD --name-only to discover changed files, then resolves test paths ' +
        'filtered by test config. Returns colocated unit test paths per source ' +
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
}

function registerChangeListTool(server: McpServer): void {
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
}

async function main(): Promise<void> {
  const server = new McpServer(
    { name: SERVER_NAME, version: SERVER_VERSION },
    { capabilities: {} },
  );

  registerPhaseLogTool(server);
  registerArchiQueryTool(server);
  registerArchiValidateTool(server);
  registerArchiWriteTool(server);
  registerArchiCheckTool(server);
  registerPhaseNextTool(server);
  registerConfigGetTool(server);
  registerTestDetectFrameworksTool(server);
  registerTestResolvePathsTool(server);
  registerChangeListTool(server);

  const transport = new StdioServerTransport();
  await server.connect(transport);
  await initProjectRootFromMcp(server.server);

  void server.sendLoggingMessage({
    level: 'info',
    data: `MCP server started in project: ${getProjectDir()}`,
  });
}

main().catch((e) => {
  process.stderr.write(`Fatal: ${e.message}\n`);
  process.exit(1);
});
