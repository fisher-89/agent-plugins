import { startServer, sendResponse, sendError, ERROR_CODES } from "./lib/mcp-jsonrpc";
import type { JsonRpcRequest } from "./lib/mcp-jsonrpc";
import { runEvalLog } from "./commands/eval-log";
import { runEvalCheck } from "./commands/eval-check";
import { queryModel } from "./lib/archi-query";
import { validateDsl } from "./lib/archi-validate";
import { writeDsl } from "./lib/archi-write";
import { runCrossRefCheck } from "./lib/c4-cross-ref";

const SERVER_NAME = "dev-team";
const SERVER_VERSION = "2.4.6";

interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: {
    type: "object";
    properties: Record<string, unknown>;
    required?: string[];
  };
}

const TOOLS: ToolDefinition[] = [
  {
    name: "eval_log",
    description:
      "Append an evaluation result entry to eval.json for a given workflow phase. " +
      "Records the verdict (pass/fail), checklist items, and optional backtrack/findings for a change.",
    inputSchema: {
      type: "object",
      properties: {
        change: { type: "string", description: "Change name (corresponds to openspec/changes/<name>)" },
        phase: { type: "string", description: "Phase identifier (e.g. 01-requirements)" },
        verdict: { type: "string", enum: ["pass", "fail"], description: "Evaluation verdict" },
        report: { type: "string", maxLength: 500, description: "Evaluation report text (max 500 chars)" },
        items: {
          type: "string",
          description: 'Checklist evaluation items as JSON array string. Each item: {"item":"...","pass":true|false,"evidence":"...","notes":"..."}',
        },
        attempt: { type: "integer", description: "Attempt number (auto-calculated if omitted)" },
        backtrack_to: { type: "string", description: "Backtrack target phase identifier" },
        skipped: { type: "boolean", description: "Mark entry as skipped (no-op phase, requires verdict pass)" },
        findings: { type: "string", description: "Diagnostic findings text from decision tree analysis" },
      },
      required: ["change", "phase", "verdict", "report", "items"],
    },
  },
  {
    name: "eval_check",
    description:
      "Check if all prior workflow phases have passed evaluation for a given phase. " +
      "Runs gate check, timestamp order check, and backtrack check. Returns structured result.",
    inputSchema: {
      type: "object",
      properties: {
        change: { type: "string", description: "Change name (corresponds to openspec/changes/<name>)" },
        phase: { type: "string", description: "Phase identifier (e.g. 03-dev-proposal)" },
      },
      required: ["change", "phase"],
    },
  },
  {
    name: "archi_query",
    description:
      "Query C4 architecture model elements and relationships. Optionally filter by element fully-qualified name.",
    inputSchema: {
      type: "object",
      properties: {
        element: { type: "string", description: "Filter by element FQN (optional)" },
        project_root: { type: "string", description: "Project root directory (defaults to cwd)" },
      },
    },
  },
  {
    name: "archi_validate",
    description:
      "Validate C4 architecture DSL syntax. Validates the current model or a provided DSL text string.",
    inputSchema: {
      type: "object",
      properties: {
        source: { type: "string", description: "DSL text to validate (omit to validate current model files)" },
        project_root: { type: "string", description: "Project root directory (defaults to cwd)" },
      },
    },
  },
  {
    name: "archi_write",
    description:
      "Validate and write a C4 architecture model file to the models/ directory. Validates DSL before writing.",
    inputSchema: {
      type: "object",
      properties: {
        path: { type: "string", description: "Target file path within models/ directory (required)" },
        source: { type: "string", description: "DSL text to write (required)" },
        project_root: { type: "string", description: "Project root directory (defaults to cwd)" },
      },
      required: ["path", "source"],
    },
  },
  {
    name: "archi_check",
    description:
      "Cross-reference validation: check code imports against the C4 architecture model. " +
      "Detects unmodeled dependencies and unused relationships in changed files.",
    inputSchema: {
      type: "object",
      properties: {
        staged: { type: "boolean", description: "Check git staged files" },
        files: { type: "string", description: "Comma-separated file list to check" },
        project_root: { type: "string", description: "Project root directory (defaults to cwd)" },
      },
    },
  },
];

function resolveProjectRoot(cwd?: string): string {
  return cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd();
}

async function handleToolCall(name: string, args: Record<string, unknown>): Promise<string> {
  switch (name) {
    case "eval_log": {
      const result = runEvalLog({
        change: args.change as string,
        phase: args.phase as string,
        verdict: args.verdict as string,
        report: args.report as string,
        items: args.items as string,
        attempt: args.attempt != null ? String(args.attempt) : undefined,
        backtrackTo: args.backtrack_to as string | undefined,
        skipped: args.skipped as boolean | undefined,
        findings: args.findings as string | undefined,
      });
      return JSON.stringify(result);
    }

    case "eval_check": {
      const result = runEvalCheck({
        change: args.change as string,
        phase: args.phase as string,
      });
      return JSON.stringify(result);
    }

    case "archi_query": {
      const projectRoot = resolveProjectRoot(args.project_root as string | undefined);
      const result = await queryModel(projectRoot, args.element as string | undefined);
      return JSON.stringify(result, null, 2);
    }

    case "archi_validate": {
      const projectRoot = resolveProjectRoot(args.project_root as string | undefined);
      const result = await validateDsl(projectRoot, args.source as string | undefined);
      return JSON.stringify(result, null, 2);
    }

    case "archi_write": {
      const projectRoot = resolveProjectRoot(args.project_root as string | undefined);
      const result = await writeDsl(
        projectRoot,
        args.source as string,
        args.path as string,
      );
      return JSON.stringify(result, null, 2);
    }

    case "archi_check": {
      const projectRoot = resolveProjectRoot(args.project_root as string | undefined);
      const files = args.files
        ? (args.files as string).split(",").map((f: string) => f.trim()).filter(Boolean)
        : undefined;
      const result = await runCrossRefCheck(projectRoot, {
        staged: !!args.staged,
        files,
      });
      return JSON.stringify(result, null, 2);
    }

    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

function main(): void {
  startServer(async (req: JsonRpcRequest) => {
    switch (req.method) {
      case "initialize": {
        sendResponse(req.id!, {
          protocolVersion: "2024-11-05",
          capabilities: { tools: {} },
          serverInfo: { name: SERVER_NAME, version: SERVER_VERSION },
        });
        break;
      }

      case "tools/list": {
        sendResponse(req.id!, { tools: TOOLS });
        break;
      }

      case "tools/call": {
        const params = req.params as { name?: string; arguments?: Record<string, unknown> };
        if (!params?.name) {
          sendError(req.id!, ERROR_CODES.INVALID_PARAMS, "Missing tool name");
          return;
        }
        try {
          const text = await handleToolCall(params.name, params.arguments ?? {});
          sendResponse(req.id!, {
            content: [{ type: "text", text }],
          });
        } catch (e: any) {
          sendResponse(req.id!, {
            content: [{ type: "text", text: e.message }],
            isError: true,
          });
        }
        break;
      }

      default:
        sendError(req.id!, ERROR_CODES.METHOD_NOT_FOUND, `Unknown method: ${req.method}`);
    }
  });
}

main();
