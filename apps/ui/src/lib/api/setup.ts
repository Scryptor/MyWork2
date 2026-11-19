import type { SetupRequest } from '$api/SetupRequest';
import { request } from './client.js';

/** The first run with an empty base: the name, the first point, the masters (once: then 409). */
export const runSetup = (body: SetupRequest) => request<void>('POST', '/setup', { body });
