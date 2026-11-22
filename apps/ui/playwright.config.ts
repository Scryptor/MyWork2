import { defineConfig, devices } from '@playwright/test';
import { FRESH_PORTS } from './e2e/support/fresh';

// Real mw-server instances serve the built UI and the API from one origin each:
// 4173 runs without sign-in (open mode), 4174 has an administrator; both are set up already.
// The fresh ones (e2e/support/fresh.ts) start with a new base each time: the first run and the import.
const server = (port: number, extra = '') => ({
	command: `node e2e/support/server.mjs --port ${port}${extra}`,
	url: `http://127.0.0.1:${port}/api/v1/server-info`,
	reuseExistingServer: !process.env.CI,
	// The first start includes a cold `cargo build`.
	timeout: 300_000,
	// SIGTERM lets server.mjs stop mw-server and remove its temporary data folder.
	gracefulShutdown: { signal: 'SIGTERM' as const, timeout: 10_000 },
	stdout: 'ignore' as const,
	stderr: 'pipe' as const
});

const ports = [4173, 4174, ...Object.values(FRESH_PORTS).flatMap((ports) => Object.values(ports))];

// Printing after an intake and an issue is on by default; in a browser it opens the preview, which would
// cover the screen in every test that accepts or issues an order. Off here, in every origin; the tests of
// printing (e2e/print.spec.ts) start with a clean browser and so with the defaults.
const printOff = JSON.stringify({
	v: 1,
	onIntake: { act: false, sticker: false },
	onIssue: { act: false }
});
const storageState = {
	cookies: [],
	origins: ports.map((port) => ({
		origin: `http://127.0.0.1:${port}`,
		localStorage: [{ name: 'mw.print', value: printOff }]
	}))
};

export default defineConfig({
	testDir: 'e2e',
	// Builds `make_fixture` once, before the tests (the import tests run it).
	globalSetup: './e2e/support/global-setup.ts',
	use: { baseURL: 'http://127.0.0.1:4173', storageState },
	webServer: [
		server(4173),
		server(4174, ' --admin'),
		// Never one left from an earlier run: its base would be set up already.
		...ports.slice(2).map((port) => ({ ...server(port, ' --fresh'), reuseExistingServer: false }))
	],
	projects: [
		{
			name: 'chromium',
			use: {
				...devices['Desktop Chrome'],
				launchOptions: {
					args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream']
				}
			}
		},
		{ name: 'webkit', use: { ...devices['Desktop Safari'] } }
	]
});
