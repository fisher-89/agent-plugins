import cac from 'cac';

import { runStaticAnalysis } from './commands/run-static-analysis';

const cli = cac('dev-team');

cli
  .command('run_static_analysis', 'Run static analysis command from openspec/config.json')
  .option('--project-root <path>', 'Override project root directory')
  .action((options: { projectRoot?: string }) => {
    const exitCode = runStaticAnalysis({ projectRoot: options.projectRoot });
    process.exit(exitCode);
  });

cli.help();
cli.parse();
