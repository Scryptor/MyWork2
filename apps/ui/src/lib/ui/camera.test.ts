import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cameraErrorText, cameraFileName } from './camera.js';

const err = (name: string) => new DOMException('x', name);

describe('cameraErrorText', () => {
	beforeEach(() => vi.stubGlobal('navigator', { mediaDevices: {} }));
	afterEach(() => vi.unstubAllGlobals());
	it('says what to do for a denied camera', () => {
		expect(cameraErrorText(err('NotAllowedError'))).toBe(
			'Нет доступа к камере. Разрешите его в настройках системы или добавьте фото файлами'
		);
	});
	it('says the camera is unavailable when there is none', () => {
		const text = 'Камера недоступна — добавьте фото файлами';
		expect(cameraErrorText(err('NotFoundError'))).toBe(text);
		expect(cameraErrorText(err('OverconstrainedError'))).toBe(text);
		vi.stubGlobal('navigator', {});
		expect(cameraErrorText(new Error('boom'))).toBe(text);
	});
	it('has a general text for anything else', () => {
		expect(cameraErrorText(err('AbortError'))).toBe(
			'Камера не запустилась — добавьте фото файлами'
		);
		expect(cameraErrorText('x')).toBe('Камера не запустилась — добавьте фото файлами');
	});
});

describe('cameraFileName', () => {
	it('is camera-YYYYMMDD-HHMMSS.jpg in local time', () => {
		expect(cameraFileName({ year: 2026, month: 3, day: 7, hour: 9, minute: 5, second: 1 })).toBe(
			'camera-20260307-090501.jpg'
		);
	});
});
