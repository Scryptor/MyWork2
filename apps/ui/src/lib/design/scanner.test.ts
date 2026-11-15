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
	it('ignores a late Enter', () => {
		const d = new ScanDetector();
		const t = type(d, '123456', 0, 10);
		expect(d.feed('Enter', t + 500)).toBeNull();
		expect(d.feed('Enter', t + 510)).toBeNull();
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
		expect(classifyScan('0201234567893')).toEqual({
			kind: 'barcode',
			value: '201234567893'
		});
	});
	it('treats other lengths as text', () => {
		for (const v of ['12345678', '12345678901', '1234567890123', '123456789012345'])
			expect(classifyScan(v)).toEqual({ kind: 'text', value: v });
	});
});

describe('installScanner', () => {
	function setup() {
		const onscan = vi.fn();
		let t = 0;
		const off = installScanner(onscan, () => t);
		const key = (k: string, target: EventTarget = window, init: KeyboardEventInit = {}) => {
			t += 10;
			const e = new KeyboardEvent('keydown', {
				key: k,
				bubbles: true,
				cancelable: true,
				...init
			});
			target.dispatchEvent(e);
			return e;
		};
		const burst = (text: string, target?: EventTarget) => {
			for (const ch of text) key(ch, target);
			return key('Enter', target);
		};
		return { onscan, off, key, burst, advance: (ms: number) => (t += ms) };
	}

	it('reports scans and stops after uninstall', () => {
		const { onscan, off, burst } = setup();
		burst('10481');
		expect(onscan).not.toHaveBeenCalled();
		const e = burst('201234567893');
		expect(onscan).toHaveBeenCalledWith({ kind: 'barcode', value: '201234567893' });
		expect(e.defaultPrevented).toBe(true);
		off();
		onscan.mockClear();
		burst('201234567893');
		expect(onscan).not.toHaveBeenCalled();
	});

	it('leaves a burst typed in a field alone', () => {
		document.body.innerHTML = '<input id="i">';
		const input = document.getElementById('i')!;
		const { onscan, off, burst } = setup();
		const e = burst('201234567893', input);
		expect(onscan).not.toHaveBeenCalled();
		expect(e.defaultPrevented).toBe(false);
		off();
		document.body.innerHTML = '';
	});

	it('ignores auto-repeat and a late Enter', () => {
		const { onscan, off, key, advance } = setup();
		for (let i = 0; i < 8; i++) key('1', window, { repeat: true });
		const e = key('Enter');
		expect(onscan).not.toHaveBeenCalled();
		expect(e.defaultPrevented).toBe(false);
		for (const ch of '201234567893') key(ch);
		advance(500);
		key('Enter');
		expect(onscan).not.toHaveBeenCalled();
		off();
	});
});
