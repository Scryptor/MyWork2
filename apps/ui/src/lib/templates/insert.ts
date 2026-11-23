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

/**
 * Whether the caret after `before` is in code mode of Typst (inside `#{…}`, `#(…)`, `#if … {…}`), where
 * a field is `v("…")` and `#` is an error; in markup (the top of the code, inside `[…]`) it is
 * `#v("…")`. A guess by brackets: the last unclosed of `{`, `(`, `[` decides; brackets in strings (in
 * code) and in comments do not count.
 */
export function inTypstCode(before: string): boolean {
	const open: string[] = [];
	const code = () => open.at(-1) === '{' || open.at(-1) === '(';
	for (let i = 0; i < before.length; i++) {
		const c = before[i];
		const rest = before.slice(i, i + 2);
		if (rest === '//' || rest === '/*') {
			const end = before.indexOf(rest === '//' ? '\n' : '*/', i + 2);
			if (end < 0) break;
			i = rest === '//' ? end : end + 1;
		} else if (c === '\\') {
			i++;
		} else if (c === '"' && code()) {
			i++;
			while (i < before.length && before[i] !== '"') i += before[i] === '\\' ? 2 : 1;
		} else if (c === '{' || c === '(' || c === '[') {
			open.push(c);
		} else if (c === '}' || c === ')' || c === ']') {
			open.pop();
		}
	}
	return code();
}
