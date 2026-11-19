// Проверка образа CI: тег mcr.microsoft.com/playwright:vX.Y.Z-noble в ci.yml равен версии @playwright/test
// в package-lock.json (в образе лежат браузеры именно этой версии). Только встроенные модули Node.
// Запуск: node scripts/check-playwright-image.mjs [ci.yml] [package-lock.json]
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));

/** @param {string} yaml @returns {string[]} */
export function imageVersions(yaml) {
	return [...yaml.matchAll(/mcr\.microsoft\.com\/playwright:v(\d+\.\d+\.\d+)-noble/g)].map(
		(m) => m[1] ?? ''
	);
}

/** @param {string} lockText @returns {string | undefined} */
export function lockedVersion(lockText) {
	return JSON.parse(lockText).packages?.['node_modules/@playwright/test']?.version;
}

/** @returns {string[]} the problems, empty when the image and the package agree */
export function problems(/** @type {string} */ yaml, /** @type {string} */ lockText) {
	const want = lockedVersion(lockText);
	if (!want) return ['в package-lock.json нет @playwright/test'];
	const found = imageVersions(yaml);
	if (found.length === 0) return ['в ci.yml нет образа mcr.microsoft.com/playwright:vX.Y.Z-noble'];
	return found
		.filter((v) => v !== want)
		.map((v) => `образ Playwright в ci.yml ${v}, а @playwright/test в package-lock.json ${want}`);
}

function main() {
	const ci = resolve(process.argv[2] ?? resolve(root, '../../.github/workflows/ci.yml'));
	const lock = resolve(process.argv[3] ?? resolve(root, 'package-lock.json'));
	const found = problems(readFileSync(ci, 'utf8'), readFileSync(lock, 'utf8'));
	if (found.length > 0) {
		console.error(found.join('\n'));
		process.exit(1);
	}
	console.log('Образ Playwright в CI совпадает с @playwright/test');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
