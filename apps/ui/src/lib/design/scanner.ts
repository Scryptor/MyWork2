import { isEditableTarget } from './hotkeys.svelte.js';

/** Recognises a barcode scanner: a fast burst of characters ended with Enter. */
export class ScanDetector {
	private readonly minLength: number;
	private readonly maxIntervalMs: number;
	private buffer = '';
	private last = 0;

	constructor(opts?: { minLength?: number; maxIntervalMs?: number }) {
		this.minLength = opts?.minLength ?? 6;
		this.maxIntervalMs = opts?.maxIntervalMs ?? 35;
	}

	feed(key: string, timeMs: number): string | null {
		if (key === 'Enter') {
			const code = this.buffer.length >= this.minLength ? this.buffer : null;
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

/** Up to 7 digits without a leading zero is an order number; 8 or more digits is a barcode. */
export function classifyScan(code: string): ScanResult {
	if (/^[1-9]\d{0,6}$/.test(code)) return { kind: 'order', id: Number(code) };
	if (/^\d{8,}$/.test(code)) return { kind: 'barcode', value: code };
	return { kind: 'text', value: code };
}

export function installScanner(onscan: (r: ScanResult) => void): () => void {
	const detector = new ScanDetector();
	const onKeydown = (e: KeyboardEvent) => {
		if (isEditableTarget(e.target) || e.ctrlKey || e.metaKey || e.altKey) return;
		const code = detector.feed(e.key, e.timeStamp);
		if (code !== null) {
			e.preventDefault();
			onscan(classifyScan(code));
		}
	};
	window.addEventListener('keydown', onKeydown, true);
	return () => window.removeEventListener('keydown', onKeydown, true);
}
