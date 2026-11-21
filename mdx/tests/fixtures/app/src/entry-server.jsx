import { renderToString } from "preact-render-to-string";
import Page, { meta } from "./pages/Page.mdx";

export { meta };

export function render() {
	return renderToString(<Page />);
}
