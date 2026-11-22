/**
 * How long the address of the blob lives: the save starts after the click returns (WebKit asks where to
 * save first, the desktop program's handler copies the file then), so it is not revoked at once.
 */
export const BLOB_LIFETIME_MS = 60_000;

/** Gives the user a text as a file to keep (a report): a temporary `<a download>` of a blob, no request. */
export function downloadText(name: string, text: string): void {
	downloadBlob(name, new Blob([text], { type: 'text/plain;charset=utf-8' }));
}

/**
 * Gives the user a file already here (a document's PDF) under `name`: a temporary `<a download>`; the
 * desktop program saves it into the downloads folder (`on_download`).
 */
export function downloadBlob(name: string, blob: Blob): void {
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download = name;
	document.body.append(link);
	link.click();
	link.remove();
	setTimeout(() => URL.revokeObjectURL(url), BLOB_LIFETIME_MS);
}
