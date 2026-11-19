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

export default defineConfig({
	testDir: 'e2e',
	// Builds `make_fixture` once, before the tests (the import tests run it).
	globalSetup: './e2e/support/global-setup.ts',
	use: { baseURL: 'http://127.0.0.1:4173' },
	webServer: [
		server(4173),
		server(4174, ' --admin'),
		// Never one left from an earlier run: its base would be set up already.
		...Object.values(FRESH_PORTS)
			.flatMap((ports) => Object.values(ports))
			.map((port) => ({ ...server(port, ' --fresh'), reuseExistingServer: false }))
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
