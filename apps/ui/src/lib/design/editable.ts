const NON_TEXT_INPUTS = new Set([
	'button',
	'checkbox',
	'color',
	'file',
	'image',
	'radio',
	'range',
	'reset',
	'submit'
]);

/** A field that takes typed text: hotkeys and the scanner leave its keys alone. */
export function isEditableTarget(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return false;
	if (target instanceof HTMLInputElement) return !NON_TEXT_INPUTS.has(target.type);
	if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
	const editable = target.closest('[contenteditable]');
	return editable !== null && editable.getAttribute('contenteditable') !== 'false';
}
