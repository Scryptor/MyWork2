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

if (withAdmin) {
	const created = spawnSync(
		binary,
		[...common, 'create-admin', '--login', ADMIN_LOGIN, '--name', 'Администратор'],
		{ input: `${ADMIN_PASSWORD}\n`, stdio: ['pipe', 'inherit', 'inherit'] }
	);
	if (created.status !== 0) {
		await fs.rm(dataDir, { recursive: true, force: true });
		process.exit(created.status ?? 1);
	}
}

const child = spawn(
	binary,
	['--listen', `127.0.0.1:${port}`, ...common, '--ui-dir', uiDir, 'serve'],
	{ stdio: 'inherit' }
);

let cleaned = false;
async function cleanup() {
	if (cleaned) return;
	cleaned = true;
	await fs.rm(dataDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}

process.on('SIGINT', () => child.kill('SIGINT'));
process.on('SIGTERM', () => child.kill('SIGTERM'));
child.on('exit', async (code, signal) => {
	await cleanup();
	process.exit(code ?? (signal ? 1 : 0));
});
child.on('error', async (error) => {
	console.error(error);
	await cleanup();
	process.exit(1);
});
