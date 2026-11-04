import { render } from "svelte/server";
import Thrower from "./Thrower.svelte";

export function renderApp() {
	return render(Thrower).body;
}
