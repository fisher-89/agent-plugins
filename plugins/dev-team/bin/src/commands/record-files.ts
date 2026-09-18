// record-files.ts — PostToolUse hook protocol adapter: parse the hook event,
// resolve the session-bound change, gate the event on the change's running
// phase state and delegate attribution to the workflow module's recording
// pipeline (the sole owner of the file-log write path).

import { readFileSync } from 'node:fs';

import { resolveChangeDir } from '../lib/change';
import { getWorkflowType } from '../lib/change-config';
import { getProjectDir } from '../lib/project-root';
import { bindSession, lookupChange } from '../lib/session-registry';
import { extractFileOps } from '../lib/shell-file-ops';
import { getPhaseTable, matchesExecutorAgent } from '../lib/workflow';
import {
  type ActivePhase,
  type FileOp,
  type RecordScope,
  readActivePhase,
  recordFileOps,
} from '../modules/workflow';
import { isPlainObject } from '../utils';

// ---------------------------------------------------------------------------
// record-files subcommand (PostToolUse file inventory recorder)
// ---------------------------------------------------------------------------

/** True for the MCP phase_next call event (matcher MUST use the full MCP tool name). */
function isPhaseNextCall(toolName: string): boolean {
  return toolName.startsWith('mcp__') && toolName.endsWith('phase_next');
}

/** True for the MCP phase_start call event (matcher MUST use the full MCP tool name). */
function isPhaseStartCall(toolName: string): boolean {
  return toolName.startsWith('mcp__') && toolName.endsWith('phase_start');
}

/**
 * Extract raw file ops from a PostToolUse event's tool input:
 * Write/Edit/StrReplace (Cursor's Edit) → `file_path`,
 * NotebookEdit → `notebook_path`,
 * Bash/PowerShell/Shell (Cursor's shell) → `extractFileOps(command)`.
 * Unknown tools yield no ops.
 */
function extractOpsFromToolInput(
  toolName: string,
  toolInput: Record<string, unknown> | undefined,
): FileOp[] {
  if (!toolInput) return [];

  if (toolName === 'Write' || toolName === 'Edit' || toolName === 'StrReplace') {
    const filePath = toolInput.file_path;
    return typeof filePath === 'string' && filePath ? [{ op: 'write', path: filePath }] : [];
  }

  if (toolName === 'NotebookEdit') {
    const notebookPath = toolInput.notebook_path;
    return typeof notebookPath === 'string' && notebookPath
      ? [{ op: 'write', path: notebookPath }]
      : [];
  }

  if (toolName === 'Bash' || toolName === 'PowerShell' || toolName === 'Shell') {
    const command = toolInput.command;
    if (typeof command !== 'string') return [];
    return extractFileOps(command.replace(/\\/g, '/'));
  }

  return [];
}

/**
 * Bind `session_id → change` from a `phase_next` / `phase_start` MCP call
 * event. The binding refreshes on every agent turn's lifecycle call, so a
 * lost registry self-heals at the cost of one turn's recording window.
 */
function bindFromPhaseNextCall(
  projectRoot: string,
  sessionId: string | undefined,
  toolInput: Record<string, unknown> | undefined,
): void {
  const change = toolInput?.change;
  if (sessionId && typeof change === 'string' && change) {
    bindSession(projectRoot, sessionId, change);
  }
}

/**
 * Resolve the recording scope for a write event against the change's running
 * state (gate 3): when the event's `agent_type` belongs to the running
 * phase's executor, the record is phase-scoped (with the running attempt);
 * otherwise (main agent / non-executor subagent / executor-less phase) it is
 * recorded at workflow scope — never dropped.
 */
function resolveRecordScope(
  change: string,
  active: ActivePhase,
  eventAgentType: string | undefined,
): RecordScope {
  const phaseDef = getPhaseTable(getWorkflowType(change)).find((p) => p.id === active.phase);
  if (matchesExecutorAgent(eventAgentType, phaseDef?.executor?.agent_type ?? null)) {
    return { kind: 'phase', phase: active.phase, attempt: active.attempt };
  }
  return { kind: 'workflow' };
}

/** Parsed PostToolUse event fields the recorder consumes. */
interface RecordFilesEvent {
  toolName: string;
  toolInput: Record<string, unknown> | undefined;
  sessionId: string | undefined;
  agentType: string | undefined;
}

/** Parse and shape-check the raw hook event; null when unusable. */
function parseRecordFilesEvent(stdinRaw: string): RecordFilesEvent | null {
  if (!stdinRaw || !stdinRaw.trim()) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(stdinRaw);
  } catch {
    return null;
  }
  if (!isPlainObject(parsed)) return null;

  const toolName = parsed.tool_name;
  if (typeof toolName !== 'string' || !toolName) return null;

  return {
    toolName,
    toolInput: isPlainObject(parsed.tool_input) ? parsed.tool_input : undefined,
    sessionId: typeof parsed.session_id === 'string' ? parsed.session_id : undefined,
    agentType: typeof parsed.agent_type === 'string' ? parsed.agent_type : undefined,
  };
}

/**
 * Attribute one PostToolUse event to the session-bound change.
 *
 * Flow: a `phase_next` / `phase_start` MCP call event binds
 * `session_id → change` (from `tool_input.change`) and records nothing;
 * every other event runs the ordered gate —
 * 1. session binding (unbound → silently dropped);
 * 2. running state (no `active_phase` → dropped with a stderr diagnostic —
 *    this is the gate that keeps post-completion / mid-turn out-of-workflow
 *    writes out of the inventory);
 * 3. agent match against the running phase's executor (mismatch → workflow
 *    scope, never dropped) — then hands the ops plus the resolved scope to
 *    the module recording pipeline (normalization, exclusions, filtering,
 *    log keying and persistence live there).
 */
function recordFilesEvent(stdinRaw: string): void {
  const event = parseRecordFilesEvent(stdinRaw);
  if (!event) return;

  const { toolName, toolInput, sessionId, agentType } = event;
  const projectRoot = getProjectDir();

  if (isPhaseNextCall(toolName) || isPhaseStartCall(toolName)) {
    bindFromPhaseNextCall(projectRoot, sessionId, toolInput);
    return;
  }

  if (!sessionId) return;
  const change = lookupChange(projectRoot, sessionId);
  if (!change) {
    process.stderr.write(
      `record-files: session ${sessionId} 未绑定 change，丢弃 ${toolName} 事件\n`,
    );
    return;
  }

  const ops = extractOpsFromToolInput(toolName, toolInput);
  if (ops.length === 0) return;

  const changeDir = resolveChangeDir(change, projectRoot);

  // Gate 2: recording is only open while a phase is running.
  const active = readActivePhase(changeDir);
  if (!active) {
    process.stderr.write(
      `record-files: change "${change}" 无 active_phase 运行态，丢弃 ${toolName} 事件\n`,
    );
    return;
  }

  // A legacy change (no `file_log`) or a malformed `workflow.json` throws
  // inside the pipeline — swallowed by runRecordFiles' catch-all (stderr
  // diagnostic, exit 0); the change must be recreated.
  recordFileOps(changeDir, ops, {
    projectRoot,
    scope: resolveRecordScope(change, active, agentType),
  });
}

/**
 * record-files subcommand entry point (PostToolUse recorder).
 *
 * Any error — unbound session, unreadable stdin, legacy change without
 * `file_log` — is written to stderr only and the process exits 0: recording
 * MUST NOT block the observed tool call.
 */
export function runRecordFiles(): void {
  let stdinRaw: string;
  try {
    stdinRaw = readFileSync(0, 'utf-8');
  } catch {
    return;
  }
  try {
    recordFilesEvent(stdinRaw);
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e);
    process.stderr.write(`record-files: ${message}\n`);
  }
}
