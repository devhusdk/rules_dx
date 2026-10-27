import { hydrate } from "preact";
import Page from "./pages/Page.mdx";

hydrate(<Page />, document.getElementById("app"));
