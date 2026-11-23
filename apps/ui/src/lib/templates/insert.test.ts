import { describe, expect, it } from 'vitest';
import { inTypstCode, insertAtCaret } from './insert.js';

describe('insertAtCaret', () => {
	it('puts the text in the middle', () => {
		expect(insertAtCaret('abcd', { start: 2, end: 2 }, '{x}')).toEqual({
			value: 'ab{x}cd',
			caret: 5
		});
	});
	it('puts the text at the end and at the start', () => {
		expect(insertAtCaret('ab', { start: 2, end: 2 }, '{x}')).toEqual({ value: 'ab{x}', caret: 5 });
		expect(insertAtCaret('ab', { start: 0, end: 0 }, '{x}')).toEqual({ value: '{x}ab', caret: 3 });
	});
	it('replaces the selection', () => {
		expect(insertAtCaret('abcd', { start: 1, end: 3 }, '{x}')).toEqual({
			value: 'a{x}d',
			caret: 4
		});
	});
	it('cuts a selection out of the text to the text', () => {
		expect(insertAtCaret('ab', { start: 9, end: 12 }, '!')).toEqual({ value: 'ab!', caret: 3 });
	});
});

describe('inTypstCode', () => {
	it('is markup at the top of the code, after a closed block and inside content', () => {
		expect(inTypstCode('')).toBe(false);
		expect(inTypstCode('Номер: ')).toBe(false);
		expect(inTypstCode('#{\n  v("a")\n}\n')).toBe(false);
		expect(inTypstCode('#{\n  text(size: 8pt)[Номер ')).toBe(false);
		expect(inTypstCode('#v("order.number") (скобка) ')).toBe(false);
	});
	it('is code inside an unclosed { or ( opened in code', () => {
		expect(inTypstCode('#{\n')).toBe(true);
		expect(inTypstCode('#if copy == "service" {\n')).toBe(true);
		expect(inTypstCode('#{\n  text(size: 8pt, ')).toBe(true);
		expect(inTypstCode('#{\n  text[a] + ')).toBe(true);
	});
	it('does not take a bracket of the text for code', () => {
		expect(inTypstCode('Телефон (рабочий) ')).toBe(false);
		expect(inTypstCode('Цена (руб.: ')).toBe(false);
		expect(inTypstCode('#text(size: 8pt)[ примечание (мелко ')).toBe(false);
		expect(inTypstCode('Скобка ) лишняя #v(')).toBe(true);
		expect(inTypstCode('#calc.min(')).toBe(true);
		expect(inTypstCode('#if data.blank { ')).toBe(true);
		expect(inTypstCode('#{ let a = (1, ')).toBe(true);
	});
	it('is code in every branch of per-copy code, as «Свой Typst-код» writes it', () => {
		// `block_code` of crates/mw-docs/src/typst_gen/code.rs.
		const service = '#if copy == "service" {\n  text[a]\n';
		const client = `${service}} else if copy == "client" {\n  text[b]\n`;
		const single = `${client}} else {\n  text[c]\n`;
		expect(inTypstCode(service)).toBe(true);
		expect(inTypstCode(client)).toBe(true);
		expect(inTypstCode(single)).toBe(true);
		expect(inTypstCode(`${single}}\n`)).toBe(false);
		expect(inTypstCode('#if a [\n  x\n] else {\n')).toBe(true);
	});
	it('is code after #context and #return, and after a name of any letters', () => {
		expect(inTypstCode('#context {\n')).toBe(true);
		expect(inTypstCode('#context (')).toBe(true);
		expect(inTypstCode('#return (')).toBe(true);
		expect(inTypstCode('#моя(')).toBe(true);
		expect(inTypstCode('#моя.функция_2(')).toBe(true);
	});
	it('takes a bracket after a closed #set rule for text', () => {
		expect(inTypstCode('#set text(size: 8pt)\nЦена (руб.: ')).toBe(false);
		expect(inTypstCode('#set text(size: 8pt) Цена (руб.: ')).toBe(false);
		expect(inTypstCode('#set text(')).toBe(true);
		expect(inTypstCode('#set text(fill: rgb(')).toBe(true);
		expect(inTypstCode('#set par(justify: true) if data.blank {')).toBe(true);
		expect(inTypstCode('Цена } else ( ')).toBe(false);
	});
	it('does not count brackets in strings and comments', () => {
		expect(inTypstCode('#v("{(") ')).toBe(false);
		expect(inTypstCode('#{ "}" + ')).toBe(true);
		expect(inTypstCode('#{ "a\\"}" + ')).toBe(true);
		expect(inTypstCode('// Свой Typst-код: v( — значение поля {\n')).toBe(false);
		expect(inTypstCode('#{ /* } */ ')).toBe(true);
	});
});
