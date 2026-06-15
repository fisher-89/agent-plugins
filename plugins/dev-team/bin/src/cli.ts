import cac from 'cac';

import { runStaticAnalysis } from './commands/run-static-analysis';

const cli = cac('dev-team');

cli
  .command('run_static_analysis', 'Run static analysis command from openspec/config.json')
  .action(() => {
    const exitCode = runStaticAnalysis();
    process.exit(exitCode);
  });

cli.help();
cli.parse();
