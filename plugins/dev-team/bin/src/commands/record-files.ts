// record-files.ts — PostToolUse hook protocol adapter: parse the hook event,
// resolve the session-bound change and delegate attribution to the workflow
// module's recording pipeline (the sole owner of the inventory write path).

import { readFileSync } from 'node:fs';

import { resolveChangeDir } from '../lib/change';
import { getProjectDir } from '../lib/project-root';
import { bindSession, lookupChange } from '../lib/session-registry';
import { extractFileOps } from '../lib/shell-file-ops';
import { type FileOp, recordFileOps } from '../modules/workflow';
import { isPlainObject } from '../utils';

// ---------------------------------------------------------------------------
// record-files subcommand (PostToolUse file inventory recorder)
// ---------------------------------------------------------------------------

/** True for the MCP phase_next call event (matcher MUST use the full MCP tool name). */
function isPhaseNextCall(toolName: string): boolean {
  return toolName.startsWith('mcp__') && toolName.endsWith('phase_next');
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
 * Bind `session_id → change` from a `phase_next` MCP call event. The binding
 * refreshes on every agent turn's `phase_next` call, so a lost registry
 * self-heals at the cost of one turn's recording window.
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
 * Attribute one PostToolUse event to the session-bound change.
 *
 * Flow: a `phase_next` MCP call event binds `session_id → change` (from
 * `tool_input.change`) and records nothing; every other event looks up the
 * binding, extracts raw ops and hands them to the module recording pipeline
 * (normalization, exclusions, filtering, folding and persistence live there).
 */
function recordFilesEvent(stdinRaw: string): void {
  if (!stdinRaw || !stdinRaw.trim()) return;

  let parsed: unknown;
  try {
    parsed = JSON.parse(stdinRaw);
  } catch {
    return;
  }
  if (!isPlainObject(parsed)) return;

  const toolName = parsed.tool_name;
  if (typeof toolName !== 'string' || !toolName) return;
  const toolInput = isPlainObject(parsed.tool_input) ? parsed.tool_input : undefined;
  const sessionId = typeof parsed.session_id === 'string' ? parsed.session_id : undefined;
  const agentType = typeof parsed.agent_type === 'string' ? parsed.agent_type : undefined;

  const projectRoot = getProjectDir();

  if (isPhaseNextCall(toolName)) {
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
  // A legacy change (no `files`) throws inside the pipeline — swallowed by
  // runRecordFiles' catch-all (stderr diagnostic, exit 0); the change must be
  // recreated.
  recordFileOps(changeDir, ops, { projectRoot, agentType });
}

/**
 * record-files subcommand entry point (PostToolUse recorder).
 *
 * Any error — unbound session, unreadable stdin, legacy change without
 * `files` — is written to stderr only and the process exits 0: recording
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
