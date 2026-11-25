// The pages of a document to print, chosen in the preview, as the server's `pages` takes them («1,3-4»).

/**
 * The checked pages (`checked[i]` — page i + 1) as `pages` of the server: numbers and ranges by commas.
 * Null when every page is checked: the whole document, no parameter. Empty when none is: nothing to print.
 */
export function pageSelection(checked: boolean[]): string | null {
	if (checked.every(Boolean)) return null;
	const parts: string[] = [];
	let start = 0;
	while (start < checked.length) {
		if (!checked[start]) {
			start++;
			continue;
		}
		let end = start;
		while (checked[end + 1]) end++;
		parts.push(end === start ? `${start + 1}` : `${start + 1}-${end + 1}`);
		start = end + 1;
	}
	return parts.join(',');
}
