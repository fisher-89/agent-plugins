import cac from 'cac';

import { runStaticAnalysis } from './commands/run-static-analysis';
import { runUnitTest } from './commands/unit-test';

const cli = cac('dev-team');

cli
  .command('run_static_analysis', 'Run static analysis command from openspec/config.json')
  .option('--project-root <path>', 'Override project root directory')
  .action((options: { projectRoot?: string }) => {
    const exitCode = runStaticAnalysis({ projectRoot: options.projectRoot });
    process.exit(exitCode);
  });

cli
  .command('unit-test', 'Run unit tests with coverage and generate execution report')
  .option('--change <name>', 'Change name (reports written to openspec/changes/<name>/reports/)')
  .option('--project-root <path>', 'Override project root directory')
  .option('--files <files>', 'Comma-separated list of test files to run')
  .option('--framework <name>', 'Only run tests for the specified framework')
  .action(
    (options: { change?: string; projectRoot?: string; files?: string; framework?: string }) => {
      const files = options.files
        ? options.files
            .split(',')
            .map((f: string) => f.trim())
            .filter(Boolean)
        : undefined;
      const exitCode = runUnitTest({
        change: options.change,
        projectRoot: options.projectRoot,
        files,
        framework: options.framework,
      });
      process.exit(exitCode);
    },
  );

cli.help();
cli.parse();
