import preact from "@astrojs/preact";
import { defineConfig, passthroughImageService } from "astro/config";

export default defineConfig({
	image: { service: passthroughImageService() },
	integrations: [preact()],
});
