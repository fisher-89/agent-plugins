/**
 * archi.ts — dev-team archi command registration.
 *
 * Subcommands:
 *   query [--element <fqn>]        Query C4 model elements and relations
 *   validate [--source <dsl>]      Validate C4 DSL syntax
 *   write --path <f> --source <dsl> Validate and write C4 model file
 *   check [--staged | --files <list>] Import cross-reference validation
 */

import { CAC } from "cac";
import { queryModel } from "../lib/archi-query";
import { validateDsl } from "../lib/archi-validate";
import { writeDsl } from "../lib/archi-write";
import { runCrossRefCheck } from "../lib/c4-cross-ref";

/**
 * Resolve project root by walking up from cwd.
 */
function resolveProjectRoot(): string {
  const cwd = process.cwd();
  const parts = cwd.replace(/\\/g, "/").split("/");
  for (let i = parts.length; i > 0; i--) {
    const candidate = parts.slice(0, i).join("/");
    try {
      const fs = require("fs");
      if (
        fs.existsSync(`${candidate}/openspec/specs/architecture`) ||
        fs.existsSync(`${candidate}/.git`)
      ) {
        return candidate;
      }
    } catch {
      continue;
    }
  }
  return cwd;
}

/**
 * Register the archi command and its subcommands on the given cac CLI instance.
 */
export function registerArchiCommand(cli: CAC): void {
  cli
    .command("archi", "Architecture model operations (query, validate, write, check)")
    .action(() => {
      // If no subcommand provided, show help
      cli.parse([...process.argv, "--help"]);
    });

  // archi query [--element <fqn>]
  cli
    .command("archi query", "Query C4 model elements and relationships")
    .option("--element <fqn>", "Filter by element fully-qualified name (optional)")
    .action(async (options: Record<string, any>) => {
      const projectRoot = resolveProjectRoot();
      const result = await queryModel(projectRoot, options.element || undefined);
      console.log(JSON.stringify(result, null, 2));
      if (result.error) {
        process.exit(1);
      }
    });

  // archi validate [--source <dsl>]
  cli
    .command("archi validate", "Validate C4 DSL syntax")
    .option("--source <dsl>", "DSL text to validate (omit to validate current model files)")
    .action(async (options: Record<string, any>) => {
      const projectRoot = resolveProjectRoot();
      const result = await validateDsl(projectRoot, options.source || undefined);
      console.log(JSON.stringify(result, null, 2));
      if (!result.valid) {
        process.exit(1);
      }
    });

  // archi write --path <f> --source <dsl>
  cli
    .command("archi write", "Validate and write C4 model file")
    .option("--path <path>", "Target file path within models/ directory (required)")
    .option("--source <dsl>", "DSL text to write (required)")
    .action(async (options: Record<string, any>) => {
      if (!options.path || !options.source) {
        console.error("错误: --path 和 --source 为必填参数");
        process.exit(1);
      }
      const projectRoot = resolveProjectRoot();
      const result = await writeDsl(projectRoot, options.source, options.path);
      console.log(JSON.stringify(result, null, 2));
      if (!result.success) {
        process.exit(1);
      }
    });

  // archi check [--staged | --files <list>]
  cli
    .command("archi check", "Cross-reference validation: check imports against model")
    .option("--staged", "Check git staged files")
    .option("--files <list>", "Comma-separated file list to check")
    .action(async (options: Record<string, any>) => {
      const projectRoot = resolveProjectRoot();
      const files = options.files
        ? (options.files as string)
          .split(",")
          .map((f: string) => f.trim())
          .filter(Boolean)
        : undefined;

      const result = await runCrossRefCheck(projectRoot, {
        staged: !!options.staged,
        files,
      });
      console.log(JSON.stringify(result, null, 2));
      if (result.status === "violations_found") {
        process.exit(1);
      }
    });
}
