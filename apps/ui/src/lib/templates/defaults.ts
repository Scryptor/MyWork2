import type { Block } from '$api/Block';
import type { BlockBody } from '$api/BlockBody';
import { ru } from '#lib/i18n/ru.js';

/** The order of the «Add block» menu; calibration marks are not offered. */
export const BLOCK_TYPES = [
	'text',
	'fields',
	'barcode',
	'logo',
	'signatures',
	'separator',
	'spacer',
	'columns',
	'copy_label'
] as const satisfies readonly BlockBody['type'][];

function body(type: (typeof BLOCK_TYPES)[number]): BlockBody {
	const d = ru.templates.defaults;
	switch (type) {
		case 'text':
			return { type, text: d.text, size_pt: null, bold: false, align: 'left', one_line: false };
		case 'fields':
			return {
				type,
				rows: [{ label: d.fieldLabel, value: '', hide_if_empty: false, bold: false }]
			};
		case 'barcode':
			return { type, height_mm: 12, digits: true, align: 'center', fill: false, stretch: false };
		case 'logo':
			return { type, width_mm: 40, height_mm: 15, align: 'left' };
		case 'signatures':
			return { type, left: d.signatureLeft, right: d.signatureRight };
		case 'separator':
			return { type };
		case 'spacer':
			return { type, mm: 4 };
		case 'columns':
			return { type, columns: [[], []], gap_mm: 6 };
		case 'copy_label':
			return { type, align: 'right' };
	}
}

/** A new block of the type with its starting values, printed in both copies. */
export function newBlock(type: BlockBody['type']): Block {
	return { copies: 'both', body: body(type as (typeof BLOCK_TYPES)[number]) };
}
