import { McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';
import { type Transport } from '@modelcontextprotocol/sdk/shared/transport';
import { type z, type ZodType } from 'zod/v4';

import { runBacktrack } from './commands/backtrack';
import { runChangeList } from './commands/change-list';
import { runConfigGet } from './commands/config-get';
import { runPhaseLog } from './commands/phase-log';
import { runPhaseNext } from './commands/phase-next';
import { runTestDetectFrameworks } from './commands/test-detect-frameworks';
import { runTestResolvePaths } from './commands/test-resolve-paths';
import {
  collectProjectRootCandidates,
  getProjectRootCandidates,
  withResolvedProjectRoot,
} from './lib/project-root';
import {
  phaseLogInputSchema,
  phaseLogOutputSchema,
  backtrackInputSchema,
  backtrackOutputSchema,
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

function jsonContent<S extends ZodType>(_outputSchema: S, data: z.output<S>) {
  return {
    content: [{ type: 'text' as const, text: JSON.stringify(data) }],
    structuredContent: data,
  };
}

type McpOutput<Output extends ZodType> = {
  content: { type: 'text'; text: string }[];
  structuredContent?: z.output<Output>;
  isError?: boolean;
};

// Tool name/description StringLiterals and top-level handler ArrowFunctions are load-time
// (perTest selection only runs listTools). Identity is asserted in mcp.test.ts listTools suite.
// Stryker disable StringLiteral,ArrowFunction
const MCP_TOOLS = [
  {
    name: 'phase_log',
    description: 'Append an evaluation result entry to eval.json for a given workflow phase. ',
    inputSchema: phaseLogInputSchema,
    outputSchema: phaseLogOutputSchema,
    handler: async (
      args: z.input<typeof phaseLogInputSchema>,
    ): Promise<McpOutput<typeof phaseLogOutputSchema>> =>
      withResolvedProjectRoot('phase_log', args as Record<string, unknown>, async () => {
        const result = runPhaseLog(args);
        return jsonContent(phaseLogOutputSchema, result);
      }),
  },
  {
    name: 'archi_query',
    description:
      'Query C4 architecture model elements and relationships. Optionally filter by element fully-qualified name.',
    inputSchema: archiQueryInputSchema,
    outputSchema: archiQueryOutputSchema,
    handler: async (
      args: z.input<typeof archiQueryInputSchema>,
    ): Promise<McpOutput<typeof archiQueryOutputSchema>> =>
      withResolvedProjectRoot(
        'archi_query',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const { queryModel } = await import('./lib/archi-query');
          const result = await queryModel(projectRoot, args.element);
          return jsonContent(archiQueryOutputSchema, result);
        },
      ),
  },
  {
    name: 'archi_validate',
    description:
      'Validate C4 architecture DSL syntax. Validates the current model or a provided DSL text string.',
    inputSchema: archiValidateInputSchema,
    outputSchema: archiValidateOutputSchema,
    handler: async (
      args: z.input<typeof archiValidateInputSchema>,
    ): Promise<McpOutput<typeof archiValidateOutputSchema>> =>
      withResolvedProjectRoot(
        'archi_validate',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const { validateDsl } = await import('./lib/archi-validate');
          const result = await validateDsl(projectRoot, args.source);
          return jsonContent(archiValidateOutputSchema, result);
        },
      ),
  },
  {
    name: 'archi_write',
    description:
      'Validate and write a C4 architecture model file to the models/ directory. Validates DSL before writing.',
    inputSchema: archiWriteInputSchema,
    outputSchema: archiWriteOutputSchema,
    handler: async (
      args: z.input<typeof archiWriteInputSchema>,
    ): Promise<McpOutput<typeof archiWriteOutputSchema>> =>
      withResolvedProjectRoot(
        'archi_write',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const { writeDsl } = await import('./lib/archi-write');
          const result = await writeDsl(projectRoot, args.source, args.path);
          return jsonContent(archiWriteOutputSchema, result);
        },
      ),
  },
  {
    name: 'archi_check',
    description:
      'Cross-reference validation: check code imports against the C4 architecture model. Detects unmodeled dependencies and unused relationships in changed files.',
    inputSchema: archiCheckInputSchema,
    outputSchema: archiCheckOutputSchema,
    handler: async (
      args: z.input<typeof archiCheckInputSchema>,
    ): Promise<McpOutput<typeof archiCheckOutputSchema>> =>
      withResolvedProjectRoot(
        'archi_check',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const { runCrossRefCheck } = await import('./lib/c4-cross-ref');
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
      ),
  },
  {
    name: 'phase_next',
    description:
      'Return the next phase to execute in a PGE workflow. Handles gate check, skip passed phases, retry, backtrack, round limit, and mid-phase interruption. Returns the phase identifier and planner/evaluator agent config for the skill to execute.',
    inputSchema: phaseNextInputSchema,
    outputSchema: phaseNextOutputSchema,
    handler: async (
      args: z.input<typeof phaseNextInputSchema>,
    ): Promise<McpOutput<typeof phaseNextOutputSchema>> =>
      withResolvedProjectRoot('phase_next', args as Record<string, unknown>, async () => {
        const result = runPhaseNext({
          project_root: args.project_root,
          change: args.change,
          run_id: args.run_id,
        });
        return jsonContent(phaseNextOutputSchema, result);
      }),
  },
  {
    name: 'config_get',
    description:
      'Read a value from openspec/config.json by dot-separated key path. Returns the value and whether the key exists. When the key does not exist, exists is false.',
    inputSchema: configGetInputSchema,
    outputSchema: configGetOutputSchema,
    handler: async (
      args: z.input<typeof configGetInputSchema>,
    ): Promise<McpOutput<typeof configGetOutputSchema>> =>
      withResolvedProjectRoot(
        'config_get',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const result = runConfigGet({
            key: args.key,
            projectRoot,
          });
          return jsonContent(configGetOutputSchema, result);
        },
      ),
  },
  {
    name: 'test_detect_frameworks',
    description:
      'Detect test framework(s) for given files based on config.json tests suite mappings. When files is omitted, auto-scan the project for files in suite scope. Returns per-file framework detection and a plan built from tests[].',
    inputSchema: testDetectFrameworksInputSchema,
    outputSchema: testDetectFrameworksOutputSchema,
    handler: async (
      args: z.input<typeof testDetectFrameworksInputSchema>,
    ): Promise<McpOutput<typeof testDetectFrameworksOutputSchema>> =>
      withResolvedProjectRoot(
        'test_detect_frameworks',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const result = runTestDetectFrameworks({
            files: args.files,
            projectRoot,
          });
          return jsonContent(testDetectFrameworksOutputSchema, result);
        },
      ),
  },
  {
    name: 'test_resolve_paths',
    description:
      'Derive unit test file paths from a module list (files or directories). Three modes: (1) modules is an empty array — directories are auto-detected from config.json test configuration; (2) modules is a non-empty array — paths are filtered by test config scope before resolving; (3) modules is "git-change" — reads git diff HEAD --name-only to discover changed files, then resolves test paths filtered by test config. Returns colocated unit test paths per source file.',
    inputSchema: testResolvePathsInputSchema,
    outputSchema: testResolvePathsOutputSchema,
    handler: async (
      args: z.input<typeof testResolvePathsInputSchema>,
    ): Promise<McpOutput<typeof testResolvePathsOutputSchema>> =>
      withResolvedProjectRoot(
        'test_resolve_paths',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const result = runTestResolvePaths({ ...args, project_root: projectRoot });
          return jsonContent(testResolvePathsOutputSchema, result);
        },
      ),
  },
  {
    name: 'change_list',
    description:
      'List all active (non-archived) changes under openspec/changes/. Returns each change with its artifacts, task progress, and latest eval phase.',
    inputSchema: changeListInputSchema,
    outputSchema: changeListOutputSchema,
    handler: async (
      args: z.input<typeof changeListInputSchema>,
    ): Promise<McpOutput<typeof changeListOutputSchema>> =>
      withResolvedProjectRoot(
        'change_list',
        args as Record<string, unknown>,
        async (projectRoot) => {
          const result = runChangeList(projectRoot);
          return jsonContent(changeListOutputSchema, result);
        },
      ),
  },
  {
    name: 'backtrack',
    description:
      'Set backtrack target and reason for a phase entry in eval.json. This is the only way to modify backtrack state.',
    inputSchema: backtrackInputSchema,
    outputSchema: backtrackOutputSchema,
    handler: async (
      args: z.input<typeof backtrackInputSchema>,
    ): Promise<McpOutput<typeof backtrackOutputSchema>> =>
      withResolvedProjectRoot('backtrack', args as Record<string, unknown>, async () => {
        const result = runBacktrack(args);
        return jsonContent(backtrackOutputSchema, result);
      }),
  },
];
// Stryker restore StringLiteral,ArrowFunction

export async function connectToServer(transport: Transport): Promise<McpServer> {
  const server = new McpServer({ name: 'dev-team', version: '2.8.11' });

  for (const mcpToolConfig of MCP_TOOLS) {
    server.registerTool(
      mcpToolConfig.name,
      {
        description: mcpToolConfig.description,
        inputSchema: mcpToolConfig.inputSchema,
        outputSchema: mcpToolConfig.outputSchema,
      },
      mcpToolConfig.handler,
    );
  }

  await server.connect(transport);
  await collectProjectRootCandidates(server.server);
  return server;
}

// CLI stdio bootstrap — not exercised by unit tests (InMemoryTransport covers connectToServer).
// Stryker disable all
if (require.main === module) {
  async function main(): Promise<void> {
    const transport = new StdioServerTransport();
    const server = await connectToServer(transport);

    const candidates = getProjectRootCandidates();
    void server.sendLoggingMessage({
      level: 'info',
      data:
        candidates.length > 0
          ? `MCP server started; project root candidates: ${candidates.join(', ')}`
          : 'MCP server started; project root candidates: (none)',
    });
  }

  main().catch((e) => {
    process.stderr.write(`Fatal: ${e.message}\n`);
    process.exit(1);
  });
}
// Stryker restore all
