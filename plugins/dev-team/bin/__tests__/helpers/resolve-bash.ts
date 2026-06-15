import * as fs from 'node:fs';

/** Resolve bash executable; on Windows prefers Git Bash when `bash` is not on PATH. */
export function resolveBash(): string {
  if (process.platform === 'win32') {
    const candidates = [
      'C:\\Program Files\\Git\\bin\\bash.exe',
      'C:\\Program Files (x86)\\Git\\bin\\bash.exe',
    ];
    for (const candidate of candidates) {
      if (fs.existsSync(candidate)) {
        return candidate;
      }
    }
  }
  return 'bash';
}
