/**
 * `insert` put into `value` over the selection (a caret is an empty one); `caret` is the place right
 * after it. A selection out of the text is cut to the text.
 */
export function insertAtCaret(
	value: string,
	selection: { start: number; end: number },
	insert: string
): { value: string; caret: number } {
	const start = Math.max(0, Math.min(selection.start, value.length));
	const end = Math.max(start, Math.min(selection.end, value.length));
	return { value: value.slice(0, start) + insert + value.slice(end), caret: start + insert.length };
}
