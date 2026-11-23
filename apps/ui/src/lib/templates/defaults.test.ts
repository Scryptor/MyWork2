import { describe, expect, it } from 'vitest';
import { BLOCK_TYPES, newBlock } from './defaults.js';

describe('newBlock', () => {
	it('the menu has the types in this order and no calibration marks', () => {
		expect([...BLOCK_TYPES]).toEqual([
			'text',
			'fields',
			'barcode',
			'logo',
			'signatures',
			'separator',
			'spacer',
			'columns',
			'copy_label'
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
			signatures: { type: 'signatures', left: 'Подпись сервиса', right: 'Подпись клиента' },
			separator: { type: 'separator' },
			spacer: { type: 'spacer', mm: 4 },
			columns: { type: 'columns', columns: [[], []], gap_mm: 6 },
			copy_label: { type: 'copy_label', align: 'right' }
		} as const;
		for (const type of BLOCK_TYPES) {
			expect(newBlock(type)).toEqual({ copies: 'both', body: bodies[type] });
		}
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
