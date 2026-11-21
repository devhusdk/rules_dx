import { render as renderApp } from "svelte/server";
import App from "./App.svelte";

export function render(): string {
	return renderApp(App).body;
}
