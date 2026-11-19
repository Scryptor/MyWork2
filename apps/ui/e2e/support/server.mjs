// Starts a real mw-server for the e2e tests: it serves the built UI and the API from one origin.
// Usage: node e2e/support/server.mjs --port 4173 [--admin]
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Test value only; the same constant is exported from api.ts.
const ADMIN_LOGIN = 'e2e-admin';
const ADMIN_PASSWORD = 'e2e-test-password';

const args = process.argv.slice(2);
const portIndex = args.indexOf('--port');
const port = portIndex >= 0 ? Number(args[portIndex + 1]) : NaN;
if (!Number.isInteger(port) || port <= 0) {
	console.error('usage: server.mjs --port <port> [--admin]');
	process.exit(2);
}
const withAdmin = args.includes('--admin');

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const uiDir = path.join(repoRoot, 'apps/ui/build');
// MW_E2E_SERVER_BIN: a ready binary (CI builds it once for both browsers); without it the server is built here.
let binary = process.env.MW_E2E_SERVER_BIN ? path.resolve(process.env.MW_E2E_SERVER_BIN) : '';
if (binary) {
	try {
		await fs.access(binary);
	} catch {
		console.error(`MW_E2E_SERVER_BIN points to a missing file: ${binary}`);
		process.exit(2);
	}
} else {
	const targetDir = process.env.CARGO_TARGET_DIR
		? path.resolve(process.env.CARGO_TARGET_DIR)
		: path.join(repoRoot, 'target');
	binary = path.join(
		targetDir,
		'debug',
		process.platform === 'win32' ? 'mw-server.exe' : 'mw-server'
	);
	const build = spawnSync('cargo', ['build', '-p', 'mw-server'], {
		cwd: repoRoot,
		stdio: 'inherit',
		shell: process.platform === 'win32'
	});
	if (build.status !== 0) process.exit(build.status ?? 1);
}

const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mw-e2e-'));
// A fixed enterprise time zone keeps dates deterministic.
const common = ['--data-dir', dataDir, '--timezone', 'UTC'];

let cleaned = false;
async function cleanup() {
	if (cleaned) return;
	cleaned = true;
	await fs.rm(dataDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}

// The child that is running right now (create-admin first, then the server). Signal handlers are
// installed before the first child starts, so a signal during create-admin still cleans up.
/** @type {import('node:child_process').ChildProcess | null} */
let current = null;
let stopping = false;
/** @param {NodeJS.Signals} signal */
function stop(signal) {
	stopping = true;
	if (current) current.kill(signal);
	else cleanup().finally(() => process.exit(1));
}
process.on('SIGINT', () => stop('SIGINT'));
process.on('SIGTERM', () => stop('SIGTERM'));

// Children get a stdin pipe and run with --exit-on-stdin-close: when this launcher dies (even by
// SIGKILL) the pipe closes and mw-server exits instead of staying behind as an orphan.
/**
 * @param {string[]} argv
 * @param {string} [input]
 * @returns {Promise<{ code: number | null, signal: NodeJS.Signals | null }>}
 */
function run(argv, input) {
	return new Promise((resolve) => {
		const proc = spawn(binary, argv, { stdio: ['pipe', 'inherit', 'inherit'] });
		current = proc;
		proc.on('error', (error) => {
			console.error(error);
			resolve({ code: 1, signal: null });
		});
		proc.on('exit', (code, signal) => resolve({ code, signal }));
		// An EPIPE here only means the child is already gone.
		proc.stdin.on('error', () => {});
		if (input !== undefined) proc.stdin.end(input);
	});
}

/** @param {number} code */
async function finish(code) {
	await cleanup();
	process.exit(code);
}

if (withAdmin) {
	const created = await run(
		[...common, 'create-admin', '--login', ADMIN_LOGIN, '--name', 'Администратор'],
		`${ADMIN_PASSWORD}\n`
	);
	if (stopping) await finish(1);
	if (created.code !== 0) await finish(created.code ?? 1);
}

const served = await run([
	'--listen',
	`127.0.0.1:${port}`,
	...common,
	'--ui-dir',
	uiDir,
	'--exit-on-stdin-close',
	'serve'
]);
await finish(served.code ?? (served.signal ? 1 : 0));
