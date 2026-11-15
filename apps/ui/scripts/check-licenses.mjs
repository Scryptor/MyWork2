// Проверка лицензий дерева dependencies (без devDependencies).
import { execSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const ALLOWED = new Set([
	'MIT',
	'MIT-0',
	'Apache-2.0',
	'BSD-2-Clause',
	'BSD-3-Clause',
	'ISC',
	'0BSD',
	'Zlib',
	'Unicode-3.0',
	'Unicode-DFS-2016'
]);

// В package.json нет поля license, но в пакете лежит файл LICENSE с текстом MIT.
const LICENSE_OVERRIDES = new Map([['svelte-toolbelt', 'MIT']]);

// Инструмент сборки Vite (минификатор CSS, MPL-2.0): попадает в дерево как зависимость vite,
// но в собранный интерфейс не входит и не распространяется.
function isBuildOnly(/** @type {string} */ packageName) {
	return packageName === 'lightningcss' || packageName.startsWith('lightningcss-');
}

/** @param {string} packageName */
function isFontPackage(packageName) {
	return packageName.startsWith('@fontsource/') || packageName.startsWith('@fontsource-variable/');
}

/**
 * @param {string} part
 * @param {string} packageName
 */
function partAllowed(part, packageName) {
	const id = part.trim();
	if (id === 'OFL-1.1') return isFontPackage(packageName);
	return ALLOWED.has(id);
}

/**
 * SPDX-выражение: `OR` проходит по любой ветке, ветка с `AND` — только если разрешены все части.
 * @param {string} license
 * @param {string} packageName
 * @returns {boolean}
 */
export function isAllowed(license, packageName) {
	const expression = String(license ?? '')
		.replace(/[()]/g, ' ')
		.trim();
	if (!expression) return false;
	return expression
		.split(/\s+OR\s+/)
		.some((branch) => branch.split(/\s+AND\s+/).every((part) => partAllowed(part, packageName)));
}

/** @param {{ license?: string | { type?: string }, licenses?: Array<string | { type?: string }> }} manifest */
function licenseOf(manifest) {
	if (typeof manifest.license === 'string') return manifest.license;
	if (typeof manifest.license === 'object' && typeof manifest.license.type === 'string')
		return manifest.license.type;
	if (Array.isArray(manifest.licenses)) {
		return manifest.licenses.map((l) => (typeof l === 'string' ? l : (l.type ?? ''))).join(' OR ');
	}
	return '';
}

function main() {
	const paths = execSync('npm ls --omit=dev --all --parseable', {
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024
	})
		.split('\n')
		.map((line) => line.trim())
		.filter(Boolean);
	const root = process.cwd();
	const seen = new Set();
	const violations = [];
	for (const path of paths) {
		if (path === root || seen.has(path)) continue;
		// `$app` создаёт SvelteKit при синхронизации: это не пакет.
		if (!existsSync(join(path, 'package.json'))) continue;
		seen.add(path);
		const manifest = JSON.parse(readFileSync(join(path, 'package.json'), 'utf8'));
		if (isBuildOnly(manifest.name)) continue;
		const license = licenseOf(manifest) || LICENSE_OVERRIDES.get(manifest.name) || '';
		if (!isAllowed(license, manifest.name)) {
			violations.push(`${manifest.name}@${manifest.version}: ${license || 'нет лицензии'}`);
		}
	}
	if (violations.length > 0) {
		console.error(violations.join('\n'));
		process.exit(1);
	}
	console.log(`Лицензии в порядке: ${seen.size} пакетов`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
