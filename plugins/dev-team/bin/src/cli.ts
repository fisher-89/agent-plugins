import cac from 'cac';

import { runStaticAnalysis } from './commands/run-static-analysis';
import { runTestExecution } from './commands/test-execution';

export const cli = cac('dev-team');

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
  .option(
    '--change <name>',
    'Change name (reports written to openspec/changes/<name>/reports/; also selects the mutation scope from the change file inventory)',
  )
  .option('--project-root <path>', 'Override project root directory')
  .option('--files <files>', 'Comma-separated list of test files to run, relative to project root')
  .option('--framework <name>', 'Only run tests for the specified framework')
  .option('--skip-mutation', 'Skip mutation testing phase')
  .action(
    async (options: {
      change?: string;
      projectRoot?: string;
      files?: string;
      framework?: string;
      skipMutation?: boolean;
    }) => {
      const files = options.files
        ? options.files
            .split(',')
            .map((f: string) => f.trim())
            .filter(Boolean)
        : undefined;
      const exitCode = await runTestExecution({
        change: options.change,
        projectRoot: options.projectRoot,
        files,
        framework: options.framework,
        noMutation: options.skipMutation,
      });
      process.exit(exitCode);
    },
  );

cli.command('[*]').action((command) => {
  if (command) {
    process.stderr.write(`Unknown command: "${command}"`);
  } else {
    process.stderr.write('No command specified');
  }
  // 展示帮助信息
  cli.outputHelp();
  process.exit(1);
});

if (require.main === module) {
  cli.help();
  cli.parse();
}
