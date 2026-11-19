// A folder of MyWork 2 for the import tests. Only synthetic data: the folder is written by `make_fixture` of
// `mw-legacy` (`mw_legacy::fixture`), never copied from a real base. The binary is built before the tests by
// `global-setup.ts` (or given by CI): MW_E2E_FIXTURE_BIN.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

/** The administrator of the fixture's base (fictional; its password is stored as MyWork 2 does). */
export const MW2_ADMIN_LOGIN = 'Ирина';
export const MW2_ADMIN_PASSWORD = '1234';

function fixtureBinary(): string {
	const ready = process.env.MW_E2E_FIXTURE_BIN;
	if (!ready)
		throw new Error(
			'MW_E2E_FIXTURE_BIN is not set: e2e/support/global-setup.ts builds make_fixture'
		);
	return ready;
}

export interface LegacyFolder {
	/** The full path of the folder (where `baza.sqlite` is). */
	path: string;
	/** Deletes the folder and the temporary folder around it. */
	remove(): Promise<void>;
}

/** Writes a synthetic MyWork 2 folder (11 orders, 6 files, users) into a new temporary folder. */
export async function legacyFolder(): Promise<LegacyFolder> {
	const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mw-e2e-mw2-'));
	const dir = path.join(root, 'MyWork2');
	const made = spawnSync(fixtureBinary(), [dir], { encoding: 'utf8' });
	if (made.status !== 0) {
		await fs.rm(root, { recursive: true, force: true });
		throw new Error(`make_fixture failed: ${made.stderr || made.error?.message}`);
	}
	return { path: dir, remove: () => fs.rm(root, { recursive: true, force: true }) };
}

/** An empty temporary folder (not a MyWork 2 folder); `remove` deletes it. */
export async function emptyFolder(): Promise<LegacyFolder> {
	const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mw-e2e-empty-'));
	return { path: root, remove: () => fs.rm(root, { recursive: true, force: true }) };
}
