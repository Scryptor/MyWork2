import { describe, expect, it } from 'vitest';
import { acceptsFile } from './accept.js';

const png = { name: 'photo.PNG', type: 'image/png' };
const jpg = { name: 'IMG_0001.jpg', type: 'image/jpeg' };
const pdf = { name: 'act.pdf', type: 'application/pdf' };
const unknown = { name: 'scan.heic', type: '' };

describe('acceptsFile', () => {
	it('a wildcard type takes its whole family', () => {
		expect(acceptsFile('image/*', png)).toBe(true);
		expect(acceptsFile('image/*', jpg)).toBe(true);
		expect(acceptsFile('image/*', pdf)).toBe(false);
		expect(acceptsFile('image/*', unknown)).toBe(false);
	});

	it('exact types and extensions, in a list, without case', () => {
		expect(acceptsFile('application/pdf', pdf)).toBe(true);
		expect(acceptsFile('.png, .pdf', png)).toBe(true);
		expect(acceptsFile('.png, .pdf', pdf)).toBe(true);
		expect(acceptsFile('.png, .pdf', jpg)).toBe(false);
		expect(acceptsFile('IMAGE/JPEG', jpg)).toBe(true);
		expect(acceptsFile('.heic', unknown)).toBe(true);
	});

	it('an empty accept takes every file', () => {
		expect(acceptsFile('', pdf)).toBe(true);
		expect(acceptsFile(' ', unknown)).toBe(true);
	});
});
