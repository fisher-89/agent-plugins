import cac from 'cac';

import { runStaticAnalysis } from './commands/run-static-analysis';
import { runTestExecution } from './commands/test-execution';

const cli = cac('dev-team');

cli
  .command('run_static_analysis', 'Run static analysis command from openspec/config.json')
  .option('--project-root <path>', 'Override project root directory')
  .action((options: { projectRoot?: string }) => {
    const exitCode = runStaticAnalysis({ projectRoot: options.projectRoot });
    process.exit(exitCode);
  });

cli
  .command(
    'test-execution',
    'Run all automated tests (unit + integration) with coverage and generate execution report',
  )
  .option('--change <name>', 'Change name (reports written to openspec/changes/<name>/reports/)')
  .option('--project-root <path>', 'Override project root directory')
  .option('--files <files>', 'Comma-separated list of test files to run')
  .option('--framework <name>', 'Only run tests for the specified framework')
  .option('--no-mutation', 'Skip mutation testing phase')
  .action(
    (options: {
      change?: string;
      projectRoot?: string;
      files?: string;
      framework?: string;
      noMutation?: boolean;
    }) => {
      const files = options.files
        ? options.files
            .split(',')
            .map((f: string) => f.trim())
            .filter(Boolean)
        : undefined;
      const exitCode = runTestExecution({
        change: options.change,
        projectRoot: options.projectRoot,
        files,
        framework: options.framework,
        noMutation: options.noMutation,
      });
      process.exit(exitCode);
    },
  );

cli.help();
cli.parse();
