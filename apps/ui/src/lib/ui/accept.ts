/**
 * Whether a file fits an `accept` list as the file chooser reads it: `image/*`, `application/pdf`,
 * `.pdf`, comma-separated, without case. An empty list takes every file.
 */
export function acceptsFile(accept: string, file: { name: string; type: string }): boolean {
	const rules = accept
		.split(',')
		.map((r) => r.trim().toLowerCase())
		.filter((r) => r !== '');
	if (rules.length === 0) return true;
	const name = file.name.toLowerCase();
	const type = file.type.toLowerCase();
	return rules.some((rule) => {
		if (rule.startsWith('.')) return name.endsWith(rule);
		if (rule.endsWith('/*')) return type.startsWith(rule.slice(0, -1));
		return type === rule;
	});
}
