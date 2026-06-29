#!/usr/bin/env node
// static-check.mjs — SubagentStop hook: run static analysis before generator ends

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const FOLLOWUP_PREFIX = "静态检查未通过，请修复以下错误后重新提交：\n\n";

export function resolveCliPath() {
  return fileURLToPath(import.meta.resolve("../../bin/dev-team-cli.cjs"));
}

function mergeCliOutput(stdout, stderr) {
  const parts = [stdout, stderr].filter((s) => s != null && s !== "");
  return parts.join("\n");
}

function buildFollowupMessage(cliOutput) {
  return FOLLOWUP_PREFIX + (cliOutput ?? "");
}

export function formatOutput({ status, stdout = "", stderr = "" }) {
  if (status === 0) return {};
  const combined = mergeCliOutput(stdout, stderr);
  return { decision: "block", reason: buildFollowupMessage(combined) };
}

export function handleMissingCli(cliPath) {
  return {
    decision: "block",
    reason: `dev-team CLI not found at ${cliPath ?? ""}`,
  };
}

export function parseWorkspaceRoot(stdinRaw) {
  try {
    const event = JSON.parse(stdinRaw);
    const roots = event?.workspace_roots;
    if (Array.isArray(roots) && roots.length > 0 && typeof roots[0] === "string") {
      return path.posix.resolve(roots[0]);
    }
  } catch {
    // ignore JSON parse errors
  }
  return null;
}

function main() {
  const stdinRaw = readFileSync(0, "utf-8");
  const workspaceRoot = parseWorkspaceRoot(stdinRaw);
  const cliPath = resolveCliPath();

  if (!existsSync(cliPath)) {
    process.stdout.write(`${JSON.stringify(handleMissingCli(cliPath))}\n`);
    return;
  }

  const args = [cliPath, "run_static_analysis"];
  if (workspaceRoot) {
    args.push("--project-root", workspaceRoot);
  }

  const result = spawnSync(process.execPath, args, { encoding: "utf-8" });

  const stdout = (result.stdout || "").trim();
  const stderr = (result.stderr || "").trim();
  const output = formatOutput({
    status: result.status ?? 1,
    stdout,
    stderr,
  });

  process.stdout.write(`${JSON.stringify(output)}\n`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main();
}
