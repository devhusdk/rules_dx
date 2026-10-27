// Test plugin: marks the project name so the isolation test can see it ran.
export function load(app) {
  app.converter.on("resolveEnd", (context) => {
    context.project.name += "DX_SENTINEL_PACKAGE";
  });
}
