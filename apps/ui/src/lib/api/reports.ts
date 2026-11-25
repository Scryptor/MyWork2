import type { OrderReport } from '$api/OrderReport';
import type { OrderReportQuery } from '$api/OrderReportQuery';
import { downloadBlob } from '#lib/platform/download.js';
import { request, requestFile } from './client.js';

/** The name of the file when the server gave none. */
const UNNAMED_REPORT = 'report.xlsx';

/** The report of the issued orders; `signal` calls a request off when the view has changed. */
export const orderReport = (query: OrderReportQuery, signal?: AbortSignal) =>
	request<OrderReport>('GET', '/reports/orders', { query, signal });

/** The same report as a file of Excel, given to the user under the name the server chose. */
export async function downloadOrderReport(query: OrderReportQuery): Promise<void> {
	const file = await requestFile('/reports/orders/xlsx', { query });
	downloadBlob(file.filename ?? UNNAMED_REPORT, file.blob);
}
