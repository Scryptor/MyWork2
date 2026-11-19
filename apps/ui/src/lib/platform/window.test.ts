import { afterEach, expect, it, vi } from 'vitest';
import { fakeDesktop, leaveDesktop } from './fake-desktop.js';
import { watchWindow } from './window.js';

afterEach(leaveDesktop);

it('in a browser there are no window events: nothing to follow or to stop', () => {
	const onchange = vi.fn();
	const stop = watchWindow(onchange);
	stop();
	expect(onchange).not.toHaveBeenCalled();
});

it('in the desktop program a move or a new monitor scale calls back; stop() unsubscribes both', async () => {
	const desktop = fakeDesktop();
	const onchange = vi.fn();
	const stop = watchWindow(onchange);
	await vi.waitFor(() =>
		expect(desktop.listening()).toEqual(['tauri://move', 'tauri://scale-change'])
	);

	desktop.fire('tauri://move', { x: 1920, y: 0 });
	expect(onchange).toHaveBeenCalledTimes(1);
	desktop.fire('tauri://scale-change', { scaleFactor: 2, size: { width: 2560, height: 1440 } });
	expect(onchange).toHaveBeenCalledTimes(2);

	stop();
	await vi.waitFor(() => expect(desktop.listening()).toEqual([]));
	desktop.fire('tauri://move', { x: 0, y: 0 });
	expect(onchange).toHaveBeenCalledTimes(2);
});

it('stop() while a subscription is on its way takes it back once it is made, and asks for no more', async () => {
	const desktop = fakeDesktop({ hold: true });
	const onchange = vi.fn();
	const stop = watchWindow(onchange);
	await vi.waitFor(() => expect(desktop.asked()).toEqual(['tauri://move']));

	stop();
	desktop.release();
	await vi.waitFor(() => expect(desktop.listening()).toEqual([]));
	expect(desktop.asked()).toEqual(['tauri://move']);
	desktop.fire('tauri://move', { x: 0, y: 0 });
	expect(onchange).not.toHaveBeenCalled();
});
