// Before the tests (once, like server.mjs builds the server): `make_fixture`, which writes the synthetic
// MyWork 2 folder of the import tests (`legacy.ts`). The tests only run it; its path reaches them through
// MW_E2E_FIXTURE_BIN, which CI sets to the binary it built with the server.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const exe = (name: string) => (process.platform === 'win32' ? `${name}.exe` : name);

export default function globalSetup(): void {
	const ready = process.env.MW_E2E_FIXTURE_BIN;
	if (ready) {
		const binary = path.resolve(ready);
		if (!fs.existsSync(binary))
			throw new Error(`MW_E2E_FIXTURE_BIN points to a missing file: ${binary}`);
		process.env.MW_E2E_FIXTURE_BIN = binary;
		return;
	}
	const build = spawnSync(
		'cargo',
		['build', '-p', 'mw-legacy', '--features', 'fixtures', '--example', 'make_fixture'],
		{ cwd: repoRoot, stdio: 'inherit', shell: process.platform === 'win32' }
	);
	if (build.status !== 0) throw new Error('make_fixture did not build');
	const target = process.env.CARGO_TARGET_DIR
		? path.resolve(process.env.CARGO_TARGET_DIR)
		: path.join(repoRoot, 'target');
	// Workers see what global setup puts into the environment.
	process.env.MW_E2E_FIXTURE_BIN = path.join(target, 'debug', 'examples', exe('make_fixture'));
}
