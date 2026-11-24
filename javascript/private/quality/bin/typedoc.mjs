import * as td from "typedoc";

const ExitCodes = {
  Ok: 0,
  OptionError: 1,
  CompileError: 3,
  ValidationError: 4,
  OutputError: 5,
  ExceptionThrown: 6,
};

function filteredContainer(container, logger) {
  const dropped = new Set();
  const proxy = new Proxy(container, {
    get(target, prop) {
      if (prop === "setValue") {
        return (key, value, configPath) => {
          if (configPath === undefined) {
            return target.setValue(key, value);
          }
          dropped.add(key);
        };
      }
      const value = Reflect.get(target, prop, target);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  return { proxy, dropped };
}

class FilteredTSConfigReader {
  name = "filtered-tsconfig";
  order = 200;
  supportsPackages = false;
  #inner = new td.TSConfigReader();

  read(container, logger, cwd, usedFile) {
    const { proxy, dropped } = filteredContainer(container, logger);
    const result = this.#inner.read(proxy, logger, cwd, usedFile);
    if (dropped.size > 0) {
      logger.info(
        `Ignoring typedocOptions from tsconfig (${[...dropped].join(", ")}); ` +
          "set TypeDoc options through the rule's options attribute.",
      );
    }
    return result;
  }
}

async function run(app) {
  const opt = (name) => app.options.getValue(name);
  if (opt("watch")) {
    app.logger.error("--watch is not supported by the isolated TypeDoc launcher.");
    return ExitCodes.OptionError;
  }
  if (opt("plugin").length > 0) {
    app.logger.error("Plugins are not supported by the isolated TypeDoc launcher.");
    return ExitCodes.OptionError;
  }
  if (app.logger.hasErrors()) {
    return ExitCodes.OptionError;
  }
  if (opt("treatWarningsAsErrors") && app.logger.hasWarnings()) {
    return ExitCodes.OptionError;
  }
  const project = await app.convert();
  if (!project) {
    return ExitCodes.CompileError;
  }
  if (opt("treatWarningsAsErrors") && app.logger.hasWarnings()) {
    return ExitCodes.CompileError;
  }
  const preValidationWarnCount = app.logger.warningCount;
  app.validate(project);
  const hadValidationWarnings =
    app.logger.warningCount !== preValidationWarnCount || app.logger.validationWarningCount != 0;
  if (app.logger.hasErrors()) {
    return ExitCodes.ValidationError;
  }
  if (hadValidationWarnings && (opt("treatWarningsAsErrors") || opt("treatValidationWarningsAsErrors"))) {
    return ExitCodes.ValidationError;
  }
  if (opt("emit") !== "none") {
    await app.generateOutputs(project);
    if (app.logger.hasErrors()) {
      return ExitCodes.OutputError;
    }
    if (opt("treatWarningsAsErrors") && app.logger.hasWarnings()) {
      return ExitCodes.OutputError;
    }
  }
  return ExitCodes.Ok;
}

async function main() {
  let app;
  try {
    app = await td.Application.bootstrap({}, [
      new td.ArgumentsReader(0),
      new FilteredTSConfigReader(),
      new td.ArgumentsReader(300).ignoreErrors(),
    ]);
    const exitCode = await run(app);
    const { errorCount, warningCount } = app.logger;
    if (errorCount) {
      app.logger.error(td.i18n.found_0_errors_and_1_warnings(errorCount.toString(), warningCount.toString()));
    } else if (warningCount) {
      app.logger.warn(td.i18n.found_0_errors_and_1_warnings(errorCount.toString(), warningCount.toString()));
    }
    process.exit(exitCode);
  } catch (error) {
    console.error("TypeDoc exiting with unexpected error:");
    console.error(error);
    process.exit(ExitCodes.ExceptionThrown);
  }
}

void main();
