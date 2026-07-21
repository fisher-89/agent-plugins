// launcher.js
const { spawn } = require('child_process');
const os = require('os');

// 获取命令行传进来的第一个参数（即包名）
const packageName = process.argv[2];

if (!packageName) {
  console.error('Error: Please specify an MCP package name.');
  process.exit(1);
}

const isWin = os.platform() === 'win32';
const command = isWin ? 'cmd.exe' : 'npx';

// 动态将包名拼接进参数数组中
const args = isWin 
  ? ['/c', 'npx', '-y', packageName] 
  : ['-y', packageName];

const server = spawn(command, args, { stdio: ['pipe', 'pipe', 'inherit'] });

process.stdin.pipe(server.stdin);
server.stdout.pipe(process.stdout);

// 确保子进程退出时，主进程也正确退出
server.on('exit', (code) => process.exit(code || 0));