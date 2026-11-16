import { isEditableTarget } from './editable.js';

/** The longest pause between two characters of a scan, ms. Hotkeys of one key wait this long too. */
export const SCAN_GAP_MS = 35;

/** Recognises a barcode scanner: a fast burst of characters ended with Enter. */
export class ScanDetector {
	private readonly minLength: number;
	private readonly maxIntervalMs: number;
	private buffer = '';
	private last = 0;

	constructor(opts?: { minLength?: number; maxIntervalMs?: number }) {
		this.minLength = opts?.minLength ?? 6;
		this.maxIntervalMs = opts?.maxIntervalMs ?? SCAN_GAP_MS;
	}

	feed(key: string, timeMs: number): string | null {
		if (key === 'Enter') {
			// Enter is part of the burst: a late Enter is not a scan.
			const fast = timeMs - this.last <= this.maxIntervalMs;
			const code = fast && this.buffer.length >= this.minLength ? this.buffer : null;
			this.buffer = '';
			return code;
		}
		if (key.length !== 1) return null; // Shift, Tab, arrows and the like
		if (this.buffer !== '' && timeMs - this.last > this.maxIntervalMs) this.buffer = '';
		this.buffer += key;
		this.last = timeMs;
		return null;
	}
}

export type ScanResult =
	| { kind: 'barcode'; value: string }
	| { kind: 'order'; id: number }
	| { kind: 'text'; value: string };

/**
 * 1-7 digits without a leading zero: order number. Exactly 12 digits: barcode.
 * 13 digits starting with 0 (UPC-A sent as EAN-13): barcode of the last 12 digits.
 * Everything else is plain text.
 */
export function classifyScan(code: string): ScanResult {
	if (/^[1-9]\d{0,6}$/.test(code)) return { kind: 'order', id: Number(code) };
	if (/^\d{12}$/.test(code)) return { kind: 'barcode', value: code };
	if (/^0\d{12}$/.test(code)) return { kind: 'barcode', value: code.slice(1) };
	return { kind: 'text', value: code };
}

export function installScanner(
	onscan: (r: ScanResult) => void,
	clock: (e: KeyboardEvent) => number = (e) => e.timeStamp
): () => void {
	const detector = new ScanDetector();
	const onKeydown = (e: KeyboardEvent) => {
		if (e.repeat || isEditableTarget(e.target) || e.ctrlKey || e.metaKey || e.altKey) return;
		const code = detector.feed(e.key, clock(e));
		if (code !== null) {
			e.preventDefault();
			onscan(classifyScan(code));
		}
	};
	window.addEventListener('keydown', onKeydown, true);
	return () => window.removeEventListener('keydown', onKeydown, true);
}
