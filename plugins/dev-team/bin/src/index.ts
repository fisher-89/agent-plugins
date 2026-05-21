import { cac } from "cac";
import { registerEvalLogCommand } from "./commands/eval-log";

/**
 * Main CLI entry point.
 * Creates a cac instance, registers subcommands, and parses argv.
 * NOTE: cac.parse() expects the full process.argv array (with binary and script path
 * at indices 0 and 1), not the sliced version. Default is process.argv.
 */
export function main(argv: string[] = process.argv): void {
  const cli = cac("dev-team");
  cli.usage("[command] [options]");
  cli.help();

  registerEvalLogCommand(cli);

  // If no arguments provided (only node binary and script path), show help
  if (argv.length <= 2) {
    cli.parse([...argv, "--help"]);
    return;
  }

  cli.parse(argv);
}

// Auto-execute when run directly
if (require.main === module) {
  main();
}
