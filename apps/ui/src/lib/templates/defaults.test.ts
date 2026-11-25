import { describe, expect, it } from 'vitest';
import { BLOCK_TYPES, CODE_MAX, newBlock, QR_LINK_MAX } from './defaults.js';

describe('newBlock', () => {
	it('the menu has the types in this order and no calibration marks', () => {
		expect([...BLOCK_TYPES]).toEqual([
			'text',
			'fields',
			'barcode',
			'logo',
			'qr',
			'signatures',
			'separator',
			'spacer',
			'columns',
			'copy_label',
			'typst_code'
		]);
		expect(BLOCK_TYPES).not.toContain('calibration_marks');
	});

	it('gives the exact starting value of each type, for both copies', () => {
		const bodies = {
			text: {
				type: 'text',
				text: 'Текст',
				size_pt: null,
				bold: false,
				align: 'left',
				one_line: false
			},
			fields: {
				type: 'fields',
				rows: [{ label: 'Название', value: '', hide_if_empty: false, bold: false }]
			},
			barcode: {
				type: 'barcode',
				height_mm: 12,
				digits: true,
				align: 'center',
				fill: false,
				stretch: false
			},
			logo: { type: 'logo', width_mm: 40, height_mm: 15, align: 'left' },
			qr: {
				type: 'qr',
				link: '{ссылка.telegram}',
				size_mm: 20,
				caption: 'Мы в Telegram',
				align: 'center'
			},
			signatures: { type: 'signatures', left: 'Подпись сервиса', right: 'Подпись клиента' },
			separator: { type: 'separator' },
			spacer: { type: 'spacer', mm: 4 },
			columns: { type: 'columns', columns: [[], []], gap_mm: 6 },
			copy_label: { type: 'copy_label', align: 'right' },
			typst_code: {
				type: 'typst_code',
				code: '// Свой Typst-код: v("ключ") — значение поля, copy — экземпляр\n#v("order.number")\n'
			}
		} as const;
		for (const type of BLOCK_TYPES) {
			expect(newBlock(type)).toEqual({ copies: 'both', body: bodies[type] });
		}
	});

	it('a code block starts with a comment on what the code sees and one field', () => {
		expect(newBlock('typst_code').body).toEqual({
			type: 'typst_code',
			code: '// Свой Typst-код: v("ключ") — значение поля, copy — экземпляр\n#v("order.number")\n'
		});
		// As `MAX_CODE_CHARS` of the model.
		expect(CODE_MAX).toBe(20_000);
	});

	it('a link of a QR code is at most as long as the model takes', () => {
		// As `MAX_QR_LINK_CHARS` of the model.
		expect(QR_LINK_MAX).toBe(500);
	});

	it('every call gives a fresh object', () => {
		expect(newBlock('columns')).not.toBe(newBlock('columns'));
	});

	it('does not make calibration marks: they belong to the calibration label only', () => {
		// @ts-expect-error -- the type refuses it; the check at run time is for what the types miss
		expect(() => newBlock('calibration_marks')).toThrow();
		expect(BLOCK_TYPES).not.toContain('calibration_marks');
	});
});
