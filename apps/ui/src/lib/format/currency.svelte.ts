// The currency symbol of the program (the «Валюта» of «Общие»). A module of its own, so that `money.ts`
// can read it while the app context, which writes it, imports nothing from `money.ts`.

/** What is shown until the settings are loaded, and when the settings have no symbol. */
export const DEFAULT_CURRENCY = '₽';

let symbol = $state(DEFAULT_CURRENCY);

/** The symbol amounts are shown with; reactive. */
export function currencySymbol(): string {
	return symbol;
}

/** `null` or a blank symbol is the default one. */
export function setCurrencySymbol(next: string | null | undefined): void {
	symbol = next?.trim() ? next.trim() : DEFAULT_CURRENCY;
}
