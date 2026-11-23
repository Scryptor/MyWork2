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

/** `#`, or `#` with a name or a path, right before the bracket: `#{`, `#v(`, `#calc.min(`, `#моя(`. */
const CALL = /#[\p{L}\p{N}_.-]*$/u;
/** A keyword whose expression goes on to the end of the line: `#if a {`, `#show: it => {`, `#context {`. */
const KEYWORD = /#(?:if|for|while|let|show|context|return)\b/;
/** Of a `#set` rule only its arguments are code (`#set text(`), and its condition (`… if a {`). */
const SET = /#set\s+[\p{L}\p{N}_.-]+\s*$|#set\b.*\bif\b/u;
/** A line that goes on with `else` after a closed branch: `} else {`, `} else if copy == "client" {`. */
const ELSE = /^\s*[\]}]\s*else\b/;

/** Whether a `(` or `{` in markup after `line` (the markup line before it) opens code. */
function opensCode(line: string): boolean {
	return CALL.test(line) || KEYWORD.test(line) || SET.test(line) || ELSE.test(line);
}

/**
 * Whether the caret after `before` is in code mode of Typst (inside `#{…}`, `#(…)`, `#if … {…}`), where
 * a field is `v("…")` and `#` is an error; in markup (the top of the code, inside `[…]`) it is
 * `#v("…")`. A guess by brackets: the last unclosed of `{`, `(`, `[` decides; brackets in strings (in
 * code) and in comments do not count. In markup `(` and `{` open code only right after `#`, after
 * `#` with a name or a path (`#v(`, `#calc.min(`), after `#if`/`#for`/`#while`/`#let`/`#show`/
 * `#context`/`#return` on the same line, as the arguments or the condition of `#set`, or on a line
 * that goes on with `else` (`} else {`); any other one is text, and `)`/`}` in markup close nothing.
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
		} else if (c === '[') {
			open.push(c);
		} else if (c === '{' || c === '(') {
			if (code() || opensCode(before.slice(before.lastIndexOf('\n', i - 1) + 1, i))) open.push(c);
		} else if (c === ']') {
			open.pop();
		} else if ((c === '}' || c === ')') && code()) {
			open.pop();
		}
	}
	return code();
}
