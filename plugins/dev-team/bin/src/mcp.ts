import { McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';

import pluginConfig from '../../.claude-plugin/plugin.json';
import { runPhaseCheck } from './commands/phase-check';
import { runPhaseLog } from './commands/phase-log';
import { runPhaseNext } from './commands/phase-next';
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
} from './schemas';

const { name: SERVER_NAME, version: SERVER_VERSION } = pluginConfig;

function resolveProjectRoot(cwd?: string): string {
  return cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd();
}

function jsonContent(data: unknown) {
  return {
    content: [{ type: 'text' as const, text: JSON.stringify(data) }],
    structuredContent: data as Record<string, unknown>,
  };
}

async function main(): Promise<void> {
  const server = new McpServer(
    { name: SERVER_NAME, version: SERVER_VERSION },
    { capabilities: {} },
  );

  server.registerTool(
    'phase/log',
    {
      description:
        'Append an evaluation result entry to eval.json for a given workflow phase. ' +
        'Records the verdict (pass/fail), checklist items, and optional backtrack/findings for a change.',
      inputSchema: phaseLogInputSchema,
      outputSchema: phaseLogOutputSchema,
    },
    async (args) => {
      const result = runPhaseLog({
        change: args.change,
        phase: args.phase,
        verdict: args.verdict,
        report: args.report,
        items: args.items,
        attempt: args.attempt != null ? String(args.attempt) : undefined,
        backtrackTo: args.backtrack_to,
        skipped: args.skipped,
        findings: args.findings,
      });
      return jsonContent(result);
    },
  );

  server.registerTool(
    'phase/check',
    {
      description:
        'Check if all prior workflow phases have passed evaluation for a given phase. ' +
        'Runs gate check, timestamp order check, and backtrack check. Returns structured result.',
      inputSchema: phaseCheckInputSchema,
      outputSchema: phaseCheckOutputSchema,
    },
    async (args) => {
      const result = runPhaseCheck({
        change: args.change,
        phase: args.phase,
      });
      return jsonContent(result);
    },
  );

  server.registerTool(
    'archi/query',
    {
      description:
        'Query C4 architecture model elements and relationships. Optionally filter by element fully-qualified name.',
      inputSchema: archiQueryInputSchema,
      outputSchema: archiQueryOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = await queryModel(projectRoot, args.element);
      return jsonContent(result);
    },
  );

  server.registerTool(
    'archi/validate',
    {
      description:
        'Validate C4 architecture DSL syntax. Validates the current model or a provided DSL text string.',
      inputSchema: archiValidateInputSchema,
      outputSchema: archiValidateOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = await validateDsl(projectRoot, args.source);
      return jsonContent(result);
    },
  );

  server.registerTool(
    'archi/write',
    {
      description:
        'Validate and write a C4 architecture model file to the models/ directory. Validates DSL before writing.',
      inputSchema: archiWriteInputSchema,
      outputSchema: archiWriteOutputSchema,
    },
    async (args) => {
      const projectRoot = resolveProjectRoot(args.project_root);
      const result = await writeDsl(projectRoot, args.source, args.path);
      return jsonContent(result);
    },
  );

  server.registerTool(
    'archi/check',
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
      return jsonContent(result);
    },
  );

  server.registerTool(
    'phase/next',
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
      return jsonContent(result);
    },
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((e) => {
  process.stderr.write(`Fatal: ${e.message}\n`);
  process.exit(1);
});
