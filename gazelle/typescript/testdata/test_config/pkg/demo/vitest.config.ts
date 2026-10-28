const extra: string[] = [];

export default {
  test: {
    setupFiles: ["./setup.ts", "@testing-library/jest-dom", ...extra],
    globalSetup: "./global_setup.ts",
  },
};
