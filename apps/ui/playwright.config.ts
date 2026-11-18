import { defineConfig, devices } from '@playwright/test';

// Two real mw-server instances serve the built UI and the API from one origin each:
// 4173 runs without sign-in (open mode), 4174 has an administrator.
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
	use: { baseURL: 'http://127.0.0.1:4173' },
	webServer: [server(4173), server(4174, ' --admin')],
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
