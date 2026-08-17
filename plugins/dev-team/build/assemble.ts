import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';

import { applyEnvTokens } from './apply-env-tokens';
import { assertNoNameTokens } from './assert-no-tokens';
import { PRODUCT_ENV_KEYS, getEnv, type ProductEnv, type ProductEnvKey } from './env';
import { expandIncludes } from './expand-includes';
import { buildHooksFile } from './hooks-profile';
import { scanTextFiles } from './scan-files';

const STAGING_BIN = '.pack-staging/bin';

function sourcePath(...segments: string[]): string {
  return join(process.cwd(), ...segments);
}

interface HomeManifest {
  version: string;
  namePrefix: string;
  managedPaths: string[];
  mcpFragment: string;
  hooksFile: string;
}

function rmOutDir(outDir: string): void {
  if (existsSync(outDir)) {
    // Clear children instead of removing the directory: on Windows an IDE/file
    // watcher may hold a handle that makes rmSync(dir) fail with EPERM.
    for (const entry of readdirSync(outDir)) {
      rmSync(join(outDir, entry), { recursive: true, force: true });
    }
  } else {
    mkdirSync(outDir, { recursive: true });
  }
}

function writeText(filePath: string, content: string): void {
  mkdirSync(dirname(filePath), { recursive: true });
  writeFileSync(filePath, content, 'utf-8');
}

function copyStaticAssets(env: ProductEnv): void {
  cpSync(sourcePath('templates'), join(env.outDir, 'templates'), { recursive: true });
}
function copySkills(env: ProductEnv): string[] {
  const managed: string[] = [];
  const skillsRoot = 'skills';
  if (!existsSync(sourcePath(skillsRoot))) return managed;
  mkdirSync(join(env.outDir, 'skills'), { recursive: true });
  for (const logicalId of readdirSync(sourcePath(skillsRoot))) {
    const src = sourcePath(skillsRoot, logicalId);
    const destName = `${env.namePrefix}${logicalId}`;
    cpSync(src, join(env.outDir, 'skills', destName), { recursive: true });
    managed.push(`skills/${destName}`);
  }
  return managed;
}

function copyAgents(env: ProductEnv): string[] {
  const managed: string[] = [];
  const agentsRoot = 'agents';
  if (!existsSync(sourcePath(agentsRoot))) return managed;
  mkdirSync(join(env.outDir, 'agents'), { recursive: true });
  for (const file of readdirSync(sourcePath(agentsRoot))) {
    if (!file.endsWith('.md')) continue;
    const logicalId = file.slice(0, -3);
    const destName = `${env.namePrefix}${logicalId}.md`;
    cpSync(sourcePath(agentsRoot, file), join(env.outDir, 'agents', destName));
    managed.push(`agents/${destName}`);
  }
  return managed;
}

function copyStagingBins(env: ProductEnv): string[] {
  const managed: string[] = [];
  mkdirSync(join(env.outDir, 'bin'), { recursive: true });
  for (const file of readdirSync(sourcePath(STAGING_BIN))) {
    const src = sourcePath(STAGING_BIN, file);
    const destName = `${env.namePrefix}${file}`;
    cpSync(src, join(env.outDir, 'bin', destName));
    managed.push(`bin/${destName}`);
  }
  return managed;
}

function applyTokensInTree(env: ProductEnv): void {
  for (const rel of scanTextFiles(env.outDir)) {
    const filePath = join(env.outDir, rel);
    const before = readFileSync(filePath, 'utf-8');
    const expanded = expandIncludes(before, env);
    const after = applyEnvTokens(expanded, env);
    if (after !== before) writeFileSync(filePath, after, 'utf-8');
  }
}

function readCanonicalHooks(): object {
  return JSON.parse(readFileSync(sourcePath('hooks/hooks.canonical.json'), 'utf-8'));
}

function writeHooks(env: ProductEnv): void {
  const content = buildHooksFile(readCanonicalHooks(), env);
  writeText(join(env.outDir, env.hooksFilePath), content);
}

function writeMcp(env: ProductEnv): void {
  const serverKey = env.layout === 'home-image' ? 'dev-team_mcp' : 'dev-team';
  const binName = applyEnvTokens('__BIN:mcp__', env);
  const argsPath = applyEnvTokens(`__DEV_TEAM_ROOT__/bin/${binName}`, env).replace(/\\/g, '/');
  const doc = {
    mcpServers: {
      [serverKey]: {
        type: 'stdio',
        command: 'node',
        args: [argsPath],
      },
    },
  };
  writeText(join(env.outDir, env.mcpFilePath), `${JSON.stringify(doc, null, 2)}\n`);
}

async function writePluginManifest(env: ProductEnv): Promise<void> {
  if (env.layout !== 'plugin') return;
  const { version } = await import('../package.json');
  const rel = env.agent === 'claude' ? '.claude-plugin/plugin.json' : '.cursor-plugin/plugin.json';
  const doc = {
    name: 'dev-team',
    description: 'A plugin for enhancing development workflow with OpenSpec integration',
    version,
    bin: './bin',
    openspecVersion: '1.2.0',
  };
  writeText(join(env.outDir, rel), `${JSON.stringify(doc, null, 2)}\n`);
}

async function writeHomeExtras(env: ProductEnv, managedPaths: string[]): Promise<void> {
  if (env.layout !== 'home-image') return;
  const { version } = await import('../package.json');
  cpSync(sourcePath('.pack-staging/install.mjs'), join(env.outDir, 'install.mjs'));
  const extraManaged = [...managedPaths, 'hooks.json', 'mcp.json', 'install.mjs', 'manifest.json'];
  const templates = join(env.outDir, 'templates');
  if (existsSync(templates)) {
    for (const rel of scanTextFiles(templates)) {
      extraManaged.push(`templates/${rel}`);
    }
  }
  const manifest: HomeManifest = {
    version,
    namePrefix: env.namePrefix,
    managedPaths: [...new Set(extraManaged)].sort(),
    mcpFragment: 'mcp.json',
    hooksFile: 'hooks.json',
  };
  writeText(join(env.outDir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
}

async function assemble(key: ProductEnvKey): Promise<void> {
  const env = getEnv(key);
  rmOutDir(env.outDir);
  copyStaticAssets(env);
  const managed = [...copySkills(env), ...copyAgents(env), ...copyStagingBins(env)];
  writeHooks(env);
  writeMcp(env);
  await writePluginManifest(env);
  await writeHomeExtras(env, managed);
  applyTokensInTree(env);
  assertNoNameTokens(env.outDir, env);
}

export async function assembleAll(): Promise<void> {
  for (const key of PRODUCT_ENV_KEYS) {
    await assemble(key);
  }
}
