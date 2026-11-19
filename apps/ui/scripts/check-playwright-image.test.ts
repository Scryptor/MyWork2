import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

const script = resolve('scripts/check-playwright-image.mjs');
const ci = resolve('../../.github/workflows/ci.yml');
const lock = resolve('package-lock.json');

const dirs: string[] = [];
afterEach(() => {
	for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});

function run(ciPath: string, lockPath: string) {
	return spawnSync(process.execPath, [script, ciPath, lockPath], { encoding: 'utf8' });
}

/** A temporary copy of ci.yml with the given text replaced. */
function ciCopy(edit: (text: string) => string): string {
	const dir = mkdtempSync(join(tmpdir(), 'mw-pw-image-'));
	dirs.push(dir);
	const path = join(dir, 'ci.yml');
	writeFileSync(path, edit(readFileSync(ci, 'utf8')));
	return path;
}

describe('check-playwright-image', () => {
	it('passes for the real ci.yml and package-lock.json', () => {
		const r = run(ci, lock);
		expect(r.stderr).toBe('');
		expect(r.status).toBe(0);
	});

	it('fails when the image tag differs from the @playwright/test version', () => {
		const r = run(
			ciCopy((t) => t.replace(/playwright:v\d+\.\d+\.\d+-noble/, 'playwright:v1.0.0-noble')),
			lock
		);
		expect(r.status).not.toBe(0);
		expect(r.stderr).toContain('1.0.0');
	});

	it('fails when ci.yml has no Playwright image', () => {
		const r = run(
			ciCopy((t) => t.replace(/mcr\.microsoft\.com\/playwright:\S+/, 'node:22')),
			lock
		);
		expect(r.status).not.toBe(0);
	});
});
