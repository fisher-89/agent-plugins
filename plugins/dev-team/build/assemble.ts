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
import { buildHooksFile, hooksCanonicalSchema, type HooksCanonical } from './hooks-profile';
import { scanTextFiles } from './scan-files';

const STAGING_BIN = '.pack-staging/bin';
const STATIC_BIN_FILES = ['bin/openspec', 'bin/openspec-bundled.js', 'bin/openspec.cmd'];

export interface HomeManifest {
  version: string;
  namePrefix: string;
  managedPaths: string[];
  mcpFragment: string;
  hooksFile: string;
}

function rmOutDir(outDir: string): void {
  if (existsSync(outDir)) rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
}

function writeText(filePath: string, content: string): void {
  mkdirSync(dirname(filePath), { recursive: true });
  writeFileSync(filePath, content, 'utf-8');
}

function copyStaticAssets(env: ProductEnv): void {
  for (const dir of ['templates', 'utils']) {
    cpSync(dir, join(env.outDir, dir), { recursive: true });
  }
  for (const file of STATIC_BIN_FILES) {
    if (!existsSync(file)) continue;
    cpSync(file, join(env.outDir, file));
  }
}

function copySkills(env: ProductEnv): string[] {
  const managed: string[] = [];
  const skillsRoot = 'skills';
  if (!existsSync(skillsRoot)) return managed;
  mkdirSync(join(env.outDir, 'skills'), { recursive: true });
  for (const logicalId of readdirSync(skillsRoot)) {
    const src = join(skillsRoot, logicalId);
    const destName = `${env.namePrefix}${logicalId}`;
    cpSync(src, join(env.outDir, 'skills', destName), { recursive: true });
    managed.push(`skills/${destName}`);
  }
  return managed;
}

function copyAgents(env: ProductEnv): string[] {
  const managed: string[] = [];
  const agentsRoot = 'agents';
  if (!existsSync(agentsRoot)) return managed;
  mkdirSync(join(env.outDir, 'agents'), { recursive: true });
  for (const file of readdirSync(agentsRoot)) {
    if (!file.endsWith('.md')) continue;
    const logicalId = file.slice(0, -3);
    const destName = `${env.namePrefix}${logicalId}.md`;
    cpSync(join(agentsRoot, file), join(env.outDir, 'agents', destName));
    managed.push(`agents/${destName}`);
  }
  return managed;
}

function copyStagingBins(env: ProductEnv): string[] {
  const managed: string[] = [];
  mkdirSync(join(env.outDir, 'bin'), { recursive: true });
  for (const file of readdirSync(STAGING_BIN)) {
    const src = join(STAGING_BIN, file);
    const destName = `${env.namePrefix}${file}`;
    cpSync(src, join(env.outDir, 'bin', destName));
    managed.push(`bin/${destName}`);
    const mapSrc = `${src}.map`;
    if (existsSync(mapSrc)) cpSync(mapSrc, join(env.outDir, 'bin', `${destName}.map`));
  }
  return managed;
}

function applyTokensInTree(env: ProductEnv): void {
  const pathTokens = env.pathReplacePhase === 'build';
  for (const rel of scanTextFiles(env.outDir)) {
    const filePath = join(env.outDir, rel);
    const before = readFileSync(filePath, 'utf-8');
    const after = applyEnvTokens(before, env, { pathTokens });
    if (after !== before) writeFileSync(filePath, after, 'utf-8');
  }
}

function readCanonicalHooks(): HooksCanonical {
  return hooksCanonicalSchema.parse(
    JSON.parse(readFileSync('hooks/hooks.canonical.json', 'utf-8')),
  );
}

function writeHooks(env: ProductEnv): void {
  const content = buildHooksFile(readCanonicalHooks(), env);
  const rel = env.hooksProfile === 'claudeNested' ? 'hooks/hooks.json' : 'hooks.json';
  writeText(join(env.outDir, rel), content);
}

function writeMcp(env: ProductEnv): void {
  const serverKey = env.layout === 'home-image' ? 'dev-team_mcp' : 'dev-team';
  const binName = applyEnvTokens('__BIN:mcp__', env, { pathTokens: false });
  const argsPath = applyEnvTokens(`__DEV_TEAM_RUNTIME_ROOT__/bin/${binName}`, env, {
    pathTokens: env.pathReplacePhase === 'build',
  }).replace(/\\/g, '/');
  const doc = {
    mcpServers: {
      [serverKey]: {
        type: 'stdio',
        command: 'node',
        args: [argsPath],
      },
    },
  };
  writeText(join(env.outDir, env.mcpOut), `${JSON.stringify(doc, null, 2)}\n`);
}

async function writePluginManifest(env: ProductEnv): Promise<void> {
  if (env.layout !== 'plugin') return;
  const { version } = await import('../package.json');
  const rel = env.key === 'claude' ? '.claude-plugin/plugin.json' : '.cursor-plugin/plugin.json';
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
  cpSync('build/home-install.mjs', join(env.outDir, 'install.mjs'));
  const extraManaged = [
    ...managedPaths,
    'bin/openspec',
    'bin/openspec-bundled.js',
    'bin/openspec.cmd',
    'hooks.json',
    'mcp.dev-team.json',
    'install.mjs',
    'manifest.json',
  ];
  for (const dir of ['templates', 'utils']) {
    const root = join(env.outDir, dir);
    if (!existsSync(root)) continue;
    for (const rel of scanTextFiles(root)) {
      extraManaged.push(`${dir}/${rel}`);
    }
  }
  const manifest: HomeManifest = {
    version,
    namePrefix: env.namePrefix,
    managedPaths: [...new Set(extraManaged)].sort(),
    mcpFragment: 'mcp.dev-team.json',
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
