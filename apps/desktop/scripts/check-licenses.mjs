// Лицензии того, что входит в программу для компьютера: обычные зависимости workspace `src-tauri` (без
// зависимостей сборки и процедурных макросов) на всех трёх ОС. Разрешены лицензии из AGENTS.md; SPDX-выражение
// проходит, если одна из его веток `OR` разрешена целиком (так проходят двойные лицензии с MIT или Apache).
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const TARGETS = [
	'x86_64-pc-windows-msvc',
	'aarch64-pc-windows-msvc',
	'aarch64-apple-darwin',
	'x86_64-apple-darwin',
	'x86_64-unknown-linux-gnu',
	'aarch64-unknown-linux-gnu'
];

const ALLOWED = new Set([
	'MIT',
	'MIT-0',
	'Apache-2.0',
	'Apache-2.0 WITH LLVM-exception',
	'BSD-2-Clause',
	'BSD-3-Clause',
	'ISC',
	'0BSD',
	'Zlib',
	'Unicode-3.0',
	'Unicode-DFS-2016'
]);

/**
 * @param {string} license SPDX-выражение (`/` старых манифестов — то же, что `OR`)
 * @returns {boolean}
 */
export function isAllowed(license) {
	const expression = String(license ?? '')
		.replace(/[()]/g, ' ')
		.replace(/\s*\/\s*/g, ' OR ')
		.replace(/\s+/g, ' ')
		.trim();
	if (!expression) return false;
	return expression
		.split(/ OR /)
		.some((branch) => branch.split(/ AND /).every((part) => ALLOWED.has(part.trim())));
}

/** Строки `имя версия|лицензия` обычных зависимостей для одной цели. */
function packagesFor(target, manifestDir) {
	const out = execFileSync(
		'cargo',
		[
			'tree',
			'--locked',
			'--target',
			target,
			'-e',
			'normal,no-proc-macro',
			'--prefix',
			'none',
			'--format',
			'{p}|{l}'
		],
		{ cwd: manifestDir, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }
	);
	return out
		.split('\n')
		.map((line) => line.replace(/ \(\*\)$/, '').trim())
		.filter(Boolean);
}

function main() {
	const manifestDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src-tauri');
	/** @type {Map<string, Set<string>>} пакет → цели, где он есть */
	const bad = new Map();
	const seen = new Set();
	for (const target of TARGETS) {
		for (const line of packagesFor(target, manifestDir)) {
			const cut = line.lastIndexOf('|');
			const pkg = line.slice(0, cut).replace(/ \(.*\)$/, '');
			const license = line.slice(cut + 1);
			seen.add(pkg);
			if (!isAllowed(license)) {
				const key = `${pkg}: ${license || 'нет лицензии'}`;
				if (!bad.has(key)) bad.set(key, new Set());
				bad.get(key).add(target);
			}
		}
	}
	if (bad.size > 0) {
		for (const [key, targets] of bad) console.error(`${key} (${[...targets].join(', ')})`);
		process.exit(1);
	}
	console.log(`Лицензии программы для компьютера в порядке: ${seen.size} пакетов`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
