#!/usr/bin/env node

import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

//#region home-install.ts
/**
* Install cursor-home-image/dev-team into a Cursor user home root (default ~/.cursor).
*
* Usage: node install.mjs [--root <path>]
*/
const IMAGE_ROOT = path.dirname(fileURLToPath(import.meta.url));
const STATE_FILE = "dev-team-install.json";
function isRecord(value) {
	return value !== null && typeof value === "object";
}
function cloneRecord(value) {
	return isRecord(value) ? structuredClone(value) : {};
}
function parseArgs(argv) {
	let root = path.resolve(os.homedir(), ".cursor");
	for (let i = 0; i < argv.length; i += 1) if (argv[i] === "--root") {
		const value = argv[i + 1];
		if (!value) throw new Error("--root requires a path");
		root = path.resolve(value);
		i += 1;
	}
	return { root };
}
function offsetToLineCol(text, offset) {
	let line = 1;
	let column = 1;
	const end = Math.min(Math.max(offset, 0), text.length);
	for (let i = 0; i < end; i += 1) if (text[i] === "\n") {
		line += 1;
		column = 1;
	} else column += 1;
	return {
		line,
		column
	};
}
function formatJsonParseError(filePath, raw, error) {
	const reason = error instanceof Error ? error.message : String(error);
	const offsetText = /position\s+(\d+)/i.exec(reason)?.[1];
	if (offsetText === void 0) return /* @__PURE__ */ new Error(`Invalid JSON in ${filePath}: ${reason}`);
	const { line, column } = offsetToLineCol(raw, Number(offsetText));
	return /* @__PURE__ */ new Error(`Invalid JSON in ${filePath}:${line}:${column}: ${reason}`);
}
function readJson(filePath) {
	let raw;
	try {
		raw = readFileSync(filePath, "utf-8");
	} catch (error) {
		const reason = error instanceof Error ? error.message : String(error);
		throw new Error(`Failed to read JSON file ${filePath}: ${reason}`);
	}
	if (raw.charCodeAt(0) === 65279) raw = raw.slice(1);
	try {
		return JSON.parse(raw);
	} catch (error) {
		throw formatJsonParseError(filePath, raw, error);
	}
}
function writeJson(filePath, value) {
	mkdirSync(path.dirname(filePath), { recursive: true });
	writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf-8");
}
/** Expand path tokens; normalize Windows separators in the install root only. */
function expandHomePathTokens(text, absoluteRoot) {
	if (text == null) throw new Error("text is required");
	if (typeof text !== "string") throw new Error("text must be a string");
	if (absoluteRoot == null || absoluteRoot === "") throw new Error("absoluteRoot is required");
	if (typeof absoluteRoot !== "string") throw new Error("absoluteRoot must be a string");
	return expandPathTokens(text, absoluteRoot);
}
function expandPathTokens(text, absoluteRoot) {
	const normalized = absoluteRoot.replace(/\\/g, "/");
	return text.replaceAll("__DEV_TEAM_ROOT__", normalized).replaceAll("__DEV_TEAM_RUNTIME_ROOT__", normalized);
}
function writeExpanded(srcFile, destFile, absoluteRoot) {
	mkdirSync(path.dirname(destFile), { recursive: true });
	const base = path.basename(srcFile);
	if (base === "openspec-bundled.js" || base.endsWith(".map")) {
		writeFileSync(destFile, readFileSync(srcFile));
		return;
	}
	writeFileSync(destFile, expandPathTokens(readFileSync(srcFile, "utf-8"), absoluteRoot), "utf-8");
}
function listFilesRecursive(dir, base = dir) {
	const out = [];
	if (!existsSync(dir)) return out;
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const full = path.join(dir, entry.name);
		if (entry.isDirectory()) out.push(...listFilesRecursive(full, base));
		else if (entry.isFile()) out.push(path.relative(base, full).replace(/\\/g, "/"));
	}
	return out;
}
function requireNamePrefix(namePrefix) {
	if (typeof namePrefix !== "string" || namePrefix === "") throw new Error("manifest.namePrefix is required");
	return namePrefix;
}
function hooksMap(doc) {
	return isRecord(doc.hooks) ? doc.hooks : {};
}
function mcpServersMap(doc) {
	return isRecord(doc.mcpServers) ? doc.mcpServers : {};
}
/** Paths that are merged or installer metadata — not blind-copied into the install root. */
function syncSkipSet(manifest) {
	const hooksFile = typeof manifest.hooksFile === "string" ? manifest.hooksFile : "hooks.json";
	const mcpFragment = typeof manifest.mcpFragment === "string" ? manifest.mcpFragment : "mcp.json";
	return new Set([
		hooksFile,
		mcpFragment,
		"install.mjs",
		"manifest.json"
	]);
}
/**
* Sync one manifest.managedPaths entry from the image into the install root.
* Directories are replaced wholesale; files are written with path-token expansion.
*/
function syncManagedPath(rel, absoluteRoot) {
	const src = path.join(IMAGE_ROOT, rel);
	const dest = path.join(absoluteRoot, rel);
	if (!existsSync(src)) throw new Error(`managed path missing from image: ${rel}`);
	if (statSync(src).isDirectory()) {
		if (existsSync(dest)) rmSync(dest, {
			recursive: true,
			force: true
		});
		mkdirSync(dest, { recursive: true });
		for (const child of listFilesRecursive(src)) writeExpanded(path.join(src, child), path.join(dest, child), absoluteRoot);
	} else writeExpanded(src, dest, absoluteRoot);
}
function isManagedHookEntry(entry, namePrefix) {
	if (!isRecord(entry)) return false;
	const command = entry.command;
	return typeof command === "string" && command.includes(namePrefix);
}
/**
* Upsert/remove managed hook entries (command contains namePrefix); keep user entries.
*/
function mergeManagedHooks(existing, incoming, namePrefix) {
	requireNamePrefix(namePrefix);
	const result = isRecord(existing) ? structuredClone(existing) : {
		version: 1,
		hooks: {}
	};
	if (!isRecord(result.hooks)) result.hooks = {};
	const hooks = hooksMap(result);
	const imageHooks = hooksMap(cloneRecord(incoming));
	for (const [eventName, imageEntries] of Object.entries(imageHooks)) {
		if (!Array.isArray(imageEntries)) continue;
		const kept = (Array.isArray(hooks[eventName]) ? hooks[eventName] : []).filter((entry) => !isManagedHookEntry(entry, namePrefix));
		const next = imageEntries.map((entry) => structuredClone(entry));
		hooks[eventName] = [...kept, ...next];
	}
	for (const eventName of Object.keys(hooks)) {
		if (Object.prototype.hasOwnProperty.call(imageHooks, eventName)) continue;
		const current = hooks[eventName];
		if (!Array.isArray(current)) continue;
		hooks[eventName] = current.filter((entry) => !isManagedHookEntry(entry, namePrefix));
	}
	result.hooks = hooks;
	return result;
}
/**
* Upsert/remove mcpServers keys starting with namePrefix; keep other servers.
*/
function mergeManagedMcp(existing, fragment, namePrefix) {
	requireNamePrefix(namePrefix);
	const result = isRecord(existing) ? structuredClone(existing) : { mcpServers: {} };
	if (!isRecord(result.mcpServers)) result.mcpServers = {};
	const servers = mcpServersMap(result);
	for (const key of Object.keys(servers)) if (key.startsWith(namePrefix)) delete servers[key];
	const imageServers = mcpServersMap(cloneRecord(fragment));
	for (const [key, server] of Object.entries(imageServers)) {
		if (!key.startsWith(namePrefix)) continue;
		servers[key] = structuredClone(server);
	}
	result.mcpServers = servers;
	return result;
}
function mergeHooks(existing, imageHooks, absoluteRoot, namePrefix) {
	const expanded = cloneRecord(imageHooks);
	for (const entries of Object.values(hooksMap(expanded))) {
		if (!Array.isArray(entries)) continue;
		for (const entry of entries) {
			if (!isRecord(entry) || typeof entry.command !== "string") continue;
			entry.command = expandPathTokens(entry.command, absoluteRoot);
		}
	}
	return mergeManagedHooks(existing, expanded, namePrefix);
}
function mergeMcp(existing, imageMcp, absoluteRoot, namePrefix) {
	const expanded = cloneRecord(imageMcp);
	for (const server of Object.values(mcpServersMap(expanded))) {
		if (!isRecord(server) || !Array.isArray(server.args)) continue;
		server.args = server.args.map((arg) => typeof arg === "string" ? expandPathTokens(arg, absoluteRoot) : arg);
	}
	return mergeManagedMcp(existing, expanded, namePrefix);
}
function readManifest() {
	const raw = readJson(path.join(IMAGE_ROOT, "manifest.json"));
	if (!isRecord(raw)) throw new Error("invalid manifest.json");
	return raw;
}
function main() {
	const { root } = parseArgs(process.argv.slice(2));
	const absoluteRoot = path.resolve(root);
	const manifest = readManifest();
	const namePrefix = requireNamePrefix(manifest.namePrefix);
	if (!Array.isArray(manifest.managedPaths)) throw new Error("manifest.managedPaths must be an array");
	const skip = syncSkipSet(manifest);
	const managedPaths = [];
	mkdirSync(absoluteRoot, { recursive: true });
	for (const rel of manifest.managedPaths) {
		if (typeof rel !== "string" || rel === "" || skip.has(rel)) continue;
		syncManagedPath(rel, absoluteRoot);
		managedPaths.push(rel.replace(/\\/g, "/"));
	}
	writeMergedConfigs(absoluteRoot, manifest, namePrefix);
	writeJson(path.join(absoluteRoot, STATE_FILE), {
		version: manifest.version,
		root: absoluteRoot,
		installedAt: (/* @__PURE__ */ new Date()).toISOString(),
		managedPaths: [...new Set(managedPaths)].sort()
	});
	process.stdout.write(`dev-team home image v${String(manifest.version)} installed to ${absoluteRoot}\nPlease run "Developer: Reload Window" in Cursor to pick up skills/agents/hooks/mcp.
`);
}
function writeMergedConfigs(absoluteRoot, manifest, namePrefix) {
	const hooksFile = typeof manifest.hooksFile === "string" ? manifest.hooksFile : "hooks.json";
	const hooksPath = path.join(absoluteRoot, "hooks.json");
	writeJson(hooksPath, mergeHooks(existsSync(hooksPath) ? readJson(hooksPath) : null, readJson(path.join(IMAGE_ROOT, hooksFile)), absoluteRoot, namePrefix));
	const mcpPath = path.join(absoluteRoot, "mcp.json");
	const mcpFragment = typeof manifest.mcpFragment === "string" ? manifest.mcpFragment : "mcp.json";
	writeJson(mcpPath, mergeMcp(existsSync(mcpPath) ? readJson(mcpPath) : null, readJson(path.join(IMAGE_ROOT, mcpFragment)), absoluteRoot, namePrefix));
}
if (process.argv[1] != null && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) try {
	main();
} catch (error) {
	process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
	process.exit(1);
}

//#endregion
export { expandHomePathTokens, mergeManagedHooks, mergeManagedMcp };