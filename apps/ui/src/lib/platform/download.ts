/** Gives the user a text as a file to keep (a report): a temporary `<a download>` of a blob, no request. */
export function downloadText(name: string, text: string): void {
	const url = URL.createObjectURL(new Blob([text], { type: 'text/plain;charset=utf-8' }));
	const link = document.createElement('a');
	link.href = url;
	link.download = name;
	document.body.append(link);
	link.click();
	link.remove();
	setTimeout(() => URL.revokeObjectURL(url), 0);
}
