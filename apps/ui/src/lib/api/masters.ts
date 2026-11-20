import type { MasterUser } from '$api/MasterUser';
import { request } from './client.js';

/** The masters of orders: users marked «Мастер», the active ones first (disabled ones stay in old orders). */
export const listMasters = () => request<MasterUser[]>('GET', '/masters');
