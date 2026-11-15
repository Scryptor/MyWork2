import { describe, expect, it, vi } from 'vitest';
import { classifyScan, installScanner, ScanDetector } from './scanner.js';

function type(d: ScanDetector, text: string, start: number, step: number): number {
	let t = start;
	for (const ch of text) {
		d.feed(ch, t);
		t += step;
	}
	return t;
}

describe('ScanDetector', () => {
	it('detects a fast burst ending with Enter', () => {
		const d = new ScanDetector();
		const t = type(d, '201234567893', 0, 10);
		expect(d.feed('Enter', t)).toBe('201234567893');
	});
	it('ignores slow typing', () => {
		const d = new ScanDetector();
		const t = type(d, '201234567893', 0, 100);
		expect(d.feed('Enter', t)).toBeNull();
	});
	it('ignores short input', () => {
		const d = new ScanDetector();
		const t = type(d, '12345', 0, 10);
		expect(d.feed('Enter', t)).toBeNull();
	});
	it('restarts after a pause', () => {
		const d = new ScanDetector();
		let t = type(d, '999', 0, 10);
		t += 200;
		t = type(d, '201234567893', t, 10);
		expect(d.feed('Enter', t)).toBe('201234567893');
	});
	it('ignores Shift inside a burst', () => {
		const d = new ScanDetector();
		let t = type(d, 'AB', 0, 10);
		d.feed('Shift', t);
		t += 10;
		t = type(d, 'C1234', t, 10);
		expect(d.feed('Enter', t)).toBe('ABC1234');
	});
});

describe('classifyScan', () => {
	it('classifies', () => {
		expect(classifyScan('201234567893')).toEqual({ kind: 'barcode', value: '201234567893' });
		expect(classifyScan('10481')).toEqual({ kind: 'order', id: 10481 });
		expect(classifyScan('0')).toEqual({ kind: 'text', value: '0' });
		expect(classifyScan('ABC-123')).toEqual({ kind: 'text', value: 'ABC-123' });
	});
});

describe('installScanner', () => {
	it('reports scans and stops after uninstall', () => {
		const onscan = vi.fn();
		const off = installScanner(onscan);
		const key = (k: string) =>
			window.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true }));
		for (const ch of '10481') key(ch);
		key('Enter');
		// five digits are below the minimum length
		expect(onscan).not.toHaveBeenCalled();
		for (const ch of '201234567893') key(ch);
		key('Enter');
		expect(onscan).toHaveBeenCalledWith({ kind: 'barcode', value: '201234567893' });
		off();
		onscan.mockClear();
		for (const ch of '201234567893') key(ch);
		key('Enter');
		expect(onscan).not.toHaveBeenCalled();
	});
});
