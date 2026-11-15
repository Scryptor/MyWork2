/** Mirror of mw_domain::search::normalize_search: NFC, lowercase, ё→е, collapse whitespace, trim. */
export function normalizeSearch(input: string): string {
	return input
		.normalize('NFC')
		.toLowerCase()
		.replaceAll('ё', 'е')
		.split(/\s+/)
		.filter((w) => w !== '')
		.join(' ');
}

/** Every word of the query occurs in the normalized text. */
export function matchesQuery(text: string, query: string): boolean {
	const haystack = normalizeSearch(text);
	return normalizeSearch(query)
		.split(' ')
		.filter((w) => w !== '')
		.every((w) => haystack.includes(w));
}
