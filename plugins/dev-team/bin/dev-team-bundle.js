#!/usr/bin/env node
"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// src/index.ts
var src_exports = {};
__export(src_exports, {
  main: () => main
});
module.exports = __toCommonJS(src_exports);

// node_modules/.pnpm/cac@6.7.14/node_modules/cac/dist/index.mjs
var import_events = require("events");
function toArr(any) {
  return any == null ? [] : Array.isArray(any) ? any : [any];
}
function toVal(out, key, val, opts) {
  var x, old = out[key], nxt = !!~opts.string.indexOf(key) ? val == null || val === true ? "" : String(val) : typeof val === "boolean" ? val : !!~opts.boolean.indexOf(key) ? val === "false" ? false : val === "true" || (out._.push((x = +val, x * 0 === 0) ? x : val), !!val) : (x = +val, x * 0 === 0) ? x : val;
  out[key] = old == null ? nxt : Array.isArray(old) ? old.concat(nxt) : [old, nxt];
}
function mri2(args, opts) {
  args = args || [];
  opts = opts || {};
  var k, arr, arg, name, val, out = { _: [] };
  var i = 0, j = 0, idx = 0, len = args.length;
  const alibi = opts.alias !== void 0;
  const strict = opts.unknown !== void 0;
  const defaults = opts.default !== void 0;
  opts.alias = opts.alias || {};
  opts.string = toArr(opts.string);
  opts.boolean = toArr(opts.boolean);
  if (alibi) {
    for (k in opts.alias) {
      arr = opts.alias[k] = toArr(opts.alias[k]);
      for (i = 0; i < arr.length; i++) {
        (opts.alias[arr[i]] = arr.concat(k)).splice(i, 1);
      }
    }
  }
  for (i = opts.boolean.length; i-- > 0; ) {
    arr = opts.alias[opts.boolean[i]] || [];
    for (j = arr.length; j-- > 0; )
      opts.boolean.push(arr[j]);
  }
  for (i = opts.string.length; i-- > 0; ) {
    arr = opts.alias[opts.string[i]] || [];
    for (j = arr.length; j-- > 0; )
      opts.string.push(arr[j]);
  }
  if (defaults) {
    for (k in opts.default) {
      name = typeof opts.default[k];
      arr = opts.alias[k] = opts.alias[k] || [];
      if (opts[name] !== void 0) {
        opts[name].push(k);
        for (i = 0; i < arr.length; i++) {
          opts[name].push(arr[i]);
        }
      }
    }
  }
  const keys = strict ? Object.keys(opts.alias) : [];
  for (i = 0; i < len; i++) {
    arg = args[i];
    if (arg === "--") {
      out._ = out._.concat(args.slice(++i));
      break;
    }
    for (j = 0; j < arg.length; j++) {
      if (arg.charCodeAt(j) !== 45)
        break;
    }
    if (j === 0) {
      out._.push(arg);
    } else if (arg.substring(j, j + 3) === "no-") {
      name = arg.substring(j + 3);
      if (strict && !~keys.indexOf(name)) {
        return opts.unknown(arg);
      }
      out[name] = false;
    } else {
      for (idx = j + 1; idx < arg.length; idx++) {
        if (arg.charCodeAt(idx) === 61)
          break;
      }
      name = arg.substring(j, idx);
      val = arg.substring(++idx) || (i + 1 === len || ("" + args[i + 1]).charCodeAt(0) === 45 || args[++i]);
      arr = j === 2 ? [name] : name;
      for (idx = 0; idx < arr.length; idx++) {
        name = arr[idx];
        if (strict && !~keys.indexOf(name))
          return opts.unknown("-".repeat(j) + name);
        toVal(out, name, idx + 1 < arr.length || val, opts);
      }
    }
  }
  if (defaults) {
    for (k in opts.default) {
      if (out[k] === void 0) {
        out[k] = opts.default[k];
      }
    }
  }
  if (alibi) {
    for (k in out) {
      arr = opts.alias[k] || [];
      while (arr.length > 0) {
        out[arr.shift()] = out[k];
      }
    }
  }
  return out;
}
var removeBrackets = (v) => v.replace(/[<[].+/, "").trim();
var findAllBrackets = (v) => {
  const ANGLED_BRACKET_RE_GLOBAL = /<([^>]+)>/g;
  const SQUARE_BRACKET_RE_GLOBAL = /\[([^\]]+)\]/g;
  const res = [];
  const parse = (match) => {
    let variadic = false;
    let value = match[1];
    if (value.startsWith("...")) {
      value = value.slice(3);
      variadic = true;
    }
    return {
      required: match[0].startsWith("<"),
      value,
      variadic
    };
  };
  let angledMatch;
  while (angledMatch = ANGLED_BRACKET_RE_GLOBAL.exec(v)) {
    res.push(parse(angledMatch));
  }
  let squareMatch;
  while (squareMatch = SQUARE_BRACKET_RE_GLOBAL.exec(v)) {
    res.push(parse(squareMatch));
  }
  return res;
};
var getMriOptions = (options) => {
  const result = { alias: {}, boolean: [] };
  for (const [index, option] of options.entries()) {
    if (option.names.length > 1) {
      result.alias[option.names[0]] = option.names.slice(1);
    }
    if (option.isBoolean) {
      if (option.negated) {
        const hasStringTypeOption = options.some((o, i) => {
          return i !== index && o.names.some((name) => option.names.includes(name)) && typeof o.required === "boolean";
        });
        if (!hasStringTypeOption) {
          result.boolean.push(option.names[0]);
        }
      } else {
        result.boolean.push(option.names[0]);
      }
    }
  }
  return result;
};
var findLongest = (arr) => {
  return arr.sort((a, b) => {
    return a.length > b.length ? -1 : 1;
  })[0];
};
var padRight = (str, length) => {
  return str.length >= length ? str : `${str}${" ".repeat(length - str.length)}`;
};
var camelcase = (input) => {
  return input.replace(/([a-z])-([a-z])/g, (_, p1, p2) => {
    return p1 + p2.toUpperCase();
  });
};
var setDotProp = (obj, keys, val) => {
  let i = 0;
  let length = keys.length;
  let t = obj;
  let x;
  for (; i < length; ++i) {
    x = t[keys[i]];
    t = t[keys[i]] = i === length - 1 ? val : x != null ? x : !!~keys[i + 1].indexOf(".") || !(+keys[i + 1] > -1) ? {} : [];
  }
};
var setByType = (obj, transforms) => {
  for (const key of Object.keys(transforms)) {
    const transform = transforms[key];
    if (transform.shouldTransform) {
      obj[key] = Array.prototype.concat.call([], obj[key]);
      if (typeof transform.transformFunction === "function") {
        obj[key] = obj[key].map(transform.transformFunction);
      }
    }
  }
};
var getFileName = (input) => {
  const m = /([^\\\/]+)$/.exec(input);
  return m ? m[1] : "";
};
var camelcaseOptionName = (name) => {
  return name.split(".").map((v, i) => {
    return i === 0 ? camelcase(v) : v;
  }).join(".");
};
var CACError = class extends Error {
  constructor(message) {
    super(message);
    this.name = this.constructor.name;
    if (typeof Error.captureStackTrace === "function") {
      Error.captureStackTrace(this, this.constructor);
    } else {
      this.stack = new Error(message).stack;
    }
  }
};
var Option = class {
  constructor(rawName, description, config) {
    this.rawName = rawName;
    this.description = description;
    this.config = Object.assign({}, config);
    rawName = rawName.replace(/\.\*/g, "");
    this.negated = false;
    this.names = removeBrackets(rawName).split(",").map((v) => {
      let name = v.trim().replace(/^-{1,2}/, "");
      if (name.startsWith("no-")) {
        this.negated = true;
        name = name.replace(/^no-/, "");
      }
      return camelcaseOptionName(name);
    }).sort((a, b) => a.length > b.length ? 1 : -1);
    this.name = this.names[this.names.length - 1];
    if (this.negated && this.config.default == null) {
      this.config.default = true;
    }
    if (rawName.includes("<")) {
      this.required = true;
    } else if (rawName.includes("[")) {
      this.required = false;
    } else {
      this.isBoolean = true;
    }
  }
};
var processArgs = process.argv;
var platformInfo = `${process.platform}-${process.arch} node-${process.version}`;
var Command = class {
  constructor(rawName, description, config = {}, cli) {
    this.rawName = rawName;
    this.description = description;
    this.config = config;
    this.cli = cli;
    this.options = [];
    this.aliasNames = [];
    this.name = removeBrackets(rawName);
    this.args = findAllBrackets(rawName);
    this.examples = [];
  }
  usage(text) {
    this.usageText = text;
    return this;
  }
  allowUnknownOptions() {
    this.config.allowUnknownOptions = true;
    return this;
  }
  ignoreOptionDefaultValue() {
    this.config.ignoreOptionDefaultValue = true;
    return this;
  }
  version(version, customFlags = "-v, --version") {
    this.versionNumber = version;
    this.option(customFlags, "Display version number");
    return this;
  }
  example(example) {
    this.examples.push(example);
    return this;
  }
  option(rawName, description, config) {
    const option = new Option(rawName, description, config);
    this.options.push(option);
    return this;
  }
  alias(name) {
    this.aliasNames.push(name);
    return this;
  }
  action(callback) {
    this.commandAction = callback;
    return this;
  }
  isMatched(name) {
    return this.name === name || this.aliasNames.includes(name);
  }
  get isDefaultCommand() {
    return this.name === "" || this.aliasNames.includes("!");
  }
  get isGlobalCommand() {
    return this instanceof GlobalCommand;
  }
  hasOption(name) {
    name = name.split(".")[0];
    return this.options.find((option) => {
      return option.names.includes(name);
    });
  }
  outputHelp() {
    const { name, commands } = this.cli;
    const {
      versionNumber,
      options: globalOptions,
      helpCallback
    } = this.cli.globalCommand;
    let sections = [
      {
        body: `${name}${versionNumber ? `/${versionNumber}` : ""}`
      }
    ];
    sections.push({
      title: "Usage",
      body: `  $ ${name} ${this.usageText || this.rawName}`
    });
    const showCommands = (this.isGlobalCommand || this.isDefaultCommand) && commands.length > 0;
    if (showCommands) {
      const longestCommandName = findLongest(commands.map((command) => command.rawName));
      sections.push({
        title: "Commands",
        body: commands.map((command) => {
          return `  ${padRight(command.rawName, longestCommandName.length)}  ${command.description}`;
        }).join("\n")
      });
      sections.push({
        title: `For more info, run any command with the \`--help\` flag`,
        body: commands.map((command) => `  $ ${name}${command.name === "" ? "" : ` ${command.name}`} --help`).join("\n")
      });
    }
    let options = this.isGlobalCommand ? globalOptions : [...this.options, ...globalOptions || []];
    if (!this.isGlobalCommand && !this.isDefaultCommand) {
      options = options.filter((option) => option.name !== "version");
    }
    if (options.length > 0) {
      const longestOptionName = findLongest(options.map((option) => option.rawName));
      sections.push({
        title: "Options",
        body: options.map((option) => {
          return `  ${padRight(option.rawName, longestOptionName.length)}  ${option.description} ${option.config.default === void 0 ? "" : `(default: ${option.config.default})`}`;
        }).join("\n")
      });
    }
    if (this.examples.length > 0) {
      sections.push({
        title: "Examples",
        body: this.examples.map((example) => {
          if (typeof example === "function") {
            return example(name);
          }
          return example;
        }).join("\n")
      });
    }
    if (helpCallback) {
      sections = helpCallback(sections) || sections;
    }
    console.log(sections.map((section) => {
      return section.title ? `${section.title}:
${section.body}` : section.body;
    }).join("\n\n"));
  }
  outputVersion() {
    const { name } = this.cli;
    const { versionNumber } = this.cli.globalCommand;
    if (versionNumber) {
      console.log(`${name}/${versionNumber} ${platformInfo}`);
    }
  }
  checkRequiredArgs() {
    const minimalArgsCount = this.args.filter((arg) => arg.required).length;
    if (this.cli.args.length < minimalArgsCount) {
      throw new CACError(`missing required args for command \`${this.rawName}\``);
    }
  }
  checkUnknownOptions() {
    const { options, globalCommand } = this.cli;
    if (!this.config.allowUnknownOptions) {
      for (const name of Object.keys(options)) {
        if (name !== "--" && !this.hasOption(name) && !globalCommand.hasOption(name)) {
          throw new CACError(`Unknown option \`${name.length > 1 ? `--${name}` : `-${name}`}\``);
        }
      }
    }
  }
  checkOptionValue() {
    const { options: parsedOptions, globalCommand } = this.cli;
    const options = [...globalCommand.options, ...this.options];
    for (const option of options) {
      const value = parsedOptions[option.name.split(".")[0]];
      if (option.required) {
        const hasNegated = options.some((o) => o.negated && o.names.includes(option.name));
        if (value === true || value === false && !hasNegated) {
          throw new CACError(`option \`${option.rawName}\` value is missing`);
        }
      }
    }
  }
};
var GlobalCommand = class extends Command {
  constructor(cli) {
    super("@@global@@", "", {}, cli);
  }
};
var __assign = Object.assign;
var CAC = class extends import_events.EventEmitter {
  constructor(name = "") {
    super();
    this.name = name;
    this.commands = [];
    this.rawArgs = [];
    this.args = [];
    this.options = {};
    this.globalCommand = new GlobalCommand(this);
    this.globalCommand.usage("<command> [options]");
  }
  usage(text) {
    this.globalCommand.usage(text);
    return this;
  }
  command(rawName, description, config) {
    const command = new Command(rawName, description || "", config, this);
    command.globalCommand = this.globalCommand;
    this.commands.push(command);
    return command;
  }
  option(rawName, description, config) {
    this.globalCommand.option(rawName, description, config);
    return this;
  }
  help(callback) {
    this.globalCommand.option("-h, --help", "Display this message");
    this.globalCommand.helpCallback = callback;
    this.showHelpOnExit = true;
    return this;
  }
  version(version, customFlags = "-v, --version") {
    this.globalCommand.version(version, customFlags);
    this.showVersionOnExit = true;
    return this;
  }
  example(example) {
    this.globalCommand.example(example);
    return this;
  }
  outputHelp() {
    if (this.matchedCommand) {
      this.matchedCommand.outputHelp();
    } else {
      this.globalCommand.outputHelp();
    }
  }
  outputVersion() {
    this.globalCommand.outputVersion();
  }
  setParsedInfo({ args, options }, matchedCommand, matchedCommandName) {
    this.args = args;
    this.options = options;
    if (matchedCommand) {
      this.matchedCommand = matchedCommand;
    }
    if (matchedCommandName) {
      this.matchedCommandName = matchedCommandName;
    }
    return this;
  }
  unsetMatchedCommand() {
    this.matchedCommand = void 0;
    this.matchedCommandName = void 0;
  }
  parse(argv = processArgs, {
    run = true
  } = {}) {
    this.rawArgs = argv;
    if (!this.name) {
      this.name = argv[1] ? getFileName(argv[1]) : "cli";
    }
    let shouldParse = true;
    for (const command of this.commands) {
      const parsed = this.mri(argv.slice(2), command);
      const commandName = parsed.args[0];
      if (command.isMatched(commandName)) {
        shouldParse = false;
        const parsedInfo = __assign(__assign({}, parsed), {
          args: parsed.args.slice(1)
        });
        this.setParsedInfo(parsedInfo, command, commandName);
        this.emit(`command:${commandName}`, command);
      }
    }
    if (shouldParse) {
      for (const command of this.commands) {
        if (command.name === "") {
          shouldParse = false;
          const parsed = this.mri(argv.slice(2), command);
          this.setParsedInfo(parsed, command);
          this.emit(`command:!`, command);
        }
      }
    }
    if (shouldParse) {
      const parsed = this.mri(argv.slice(2));
      this.setParsedInfo(parsed);
    }
    if (this.options.help && this.showHelpOnExit) {
      this.outputHelp();
      run = false;
      this.unsetMatchedCommand();
    }
    if (this.options.version && this.showVersionOnExit && this.matchedCommandName == null) {
      this.outputVersion();
      run = false;
      this.unsetMatchedCommand();
    }
    const parsedArgv = { args: this.args, options: this.options };
    if (run) {
      this.runMatchedCommand();
    }
    if (!this.matchedCommand && this.args[0]) {
      this.emit("command:*");
    }
    return parsedArgv;
  }
  mri(argv, command) {
    const cliOptions = [
      ...this.globalCommand.options,
      ...command ? command.options : []
    ];
    const mriOptions = getMriOptions(cliOptions);
    let argsAfterDoubleDashes = [];
    const doubleDashesIndex = argv.indexOf("--");
    if (doubleDashesIndex > -1) {
      argsAfterDoubleDashes = argv.slice(doubleDashesIndex + 1);
      argv = argv.slice(0, doubleDashesIndex);
    }
    let parsed = mri2(argv, mriOptions);
    parsed = Object.keys(parsed).reduce((res, name) => {
      return __assign(__assign({}, res), {
        [camelcaseOptionName(name)]: parsed[name]
      });
    }, { _: [] });
    const args = parsed._;
    const options = {
      "--": argsAfterDoubleDashes
    };
    const ignoreDefault = command && command.config.ignoreOptionDefaultValue ? command.config.ignoreOptionDefaultValue : this.globalCommand.config.ignoreOptionDefaultValue;
    let transforms = /* @__PURE__ */ Object.create(null);
    for (const cliOption of cliOptions) {
      if (!ignoreDefault && cliOption.config.default !== void 0) {
        for (const name of cliOption.names) {
          options[name] = cliOption.config.default;
        }
      }
      if (Array.isArray(cliOption.config.type)) {
        if (transforms[cliOption.name] === void 0) {
          transforms[cliOption.name] = /* @__PURE__ */ Object.create(null);
          transforms[cliOption.name]["shouldTransform"] = true;
          transforms[cliOption.name]["transformFunction"] = cliOption.config.type[0];
        }
      }
    }
    for (const key of Object.keys(parsed)) {
      if (key !== "_") {
        const keys = key.split(".");
        setDotProp(options, keys, parsed[key]);
        setByType(options, transforms);
      }
    }
    return {
      args,
      options
    };
  }
  runMatchedCommand() {
    const { args, options, matchedCommand: command } = this;
    if (!command || !command.commandAction)
      return;
    command.checkUnknownOptions();
    command.checkOptionValue();
    command.checkRequiredArgs();
    const actionArgs = [];
    command.args.forEach((arg, index) => {
      if (arg.variadic) {
        actionArgs.push(args.slice(index));
      } else {
        actionArgs.push(args[index]);
      }
    });
    actionArgs.push(options);
    return command.commandAction.apply(this, actionArgs);
  }
};
var cac = (name = "") => new CAC(name);

// src/lib/eval-json.ts
var fs = __toESM(require("fs"));
var path = __toESM(require("path"));
var EVAL_JSON_FILE = "eval.json";
var SCHEMA_VERSION = "1.0";
function readEvalJson(phasesDir) {
  const filePath = path.join(phasesDir, EVAL_JSON_FILE);
  if (!fs.existsSync(filePath)) {
    return [];
  }
  const raw = fs.readFileSync(filePath, "utf-8");
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      throw new Error(`eval.json \u6839\u5143\u7D20\u5FC5\u987B\u662F\u6570\u7EC4\uFF0C\u4F46\u5B9E\u9645\u7C7B\u578B\u4E3A ${typeof parsed}`);
    }
    return parsed;
  } catch (e) {
    if (e instanceof SyntaxError) {
      throw new Error(`eval.json \u89E3\u6790\u5931\u8D25: ${e.message}`);
    }
    throw e;
  }
}
function validateVerdict(verdict) {
  if (verdict !== "pass" && verdict !== "fail") {
    throw new Error(`verdict \u5FC5\u987B\u4E3A "pass" \u6216 "fail"\uFF0C\u4F46\u6536\u5230: ${JSON.stringify(verdict)}`);
  }
}
function validateReportLength(report) {
  if (report.length > 500) {
    throw new Error(
      `\u62A5\u544A\u957F\u5EA6\u8D85\u8FC7 500 \u5B57\u7B26\u9650\u5236\uFF08\u5F53\u524D ${report.length} \u5B57\u7B26\uFF09\u3002\u8BF7\u7CBE\u7B80\u62A5\u544A\u5185\u5BB9\u3002`
    );
  }
}
function validateItemsJson(itemsStr) {
  let parsed;
  try {
    parsed = JSON.parse(itemsStr);
  } catch (e) {
    throw new Error(
      `items \u53C2\u6570\u4E0D\u662F\u6709\u6548\u7684 JSON \u6570\u7EC4\u3002\u8BF7\u786E\u4FDD\u4F7F\u7528\u5355\u5F15\u53F7\u5305\u88F9 JSON \u5B57\u7B26\u4E32\uFF0C\u4F8B\u5982: --items '[{"item_id":"R1","pass":true}]'\u3002\u89E3\u6790\u9519\u8BEF: ${e.message}`
    );
  }
  if (!Array.isArray(parsed)) {
    throw new Error(`items \u53C2\u6570\u5FC5\u987B\u662F\u4E00\u4E2A JSON \u6570\u7EC4\uFF0C\u4F46\u6536\u5230: ${typeof parsed}`);
  }
  return parsed;
}
function buildEntry(params) {
  const entry = {
    phase: params.phase,
    timestamp: (/* @__PURE__ */ new Date()).toISOString(),
    attempt: params.attempt,
    verdict: params.verdict,
    report: params.report,
    items: params.items,
    backtrack_to: params.backtrack_to !== void 0 ? params.backtrack_to : null,
    schema_version: SCHEMA_VERSION
  };
  return entry;
}
function computeAttempt(entries, phase, explicitAttempt) {
  if (explicitAttempt !== void 0) {
    if (!Number.isInteger(explicitAttempt) || explicitAttempt < 1) {
      throw new Error(`attempt \u5FC5\u987B\u4E3A\u6B63\u6574\u6570\uFF0C\u4F46\u6536\u5230: ${explicitAttempt}`);
    }
    return explicitAttempt;
  }
  const phaseEntries = entries.filter((e) => e.phase === phase);
  return phaseEntries.length + 1;
}
function checkGate(entries, priorPhases) {
  const missing = [];
  for (const phase of priorPhases) {
    const hasPass = entries.some(
      (e) => e.phase === phase && e.verdict === "pass"
    );
    if (!hasPass) {
      missing.push(phase);
    }
  }
  return { passed: missing.length === 0, missing };
}
function appendEntry(phasesDir, entry) {
  fs.mkdirSync(phasesDir, { recursive: true });
  const filePath = path.join(phasesDir, EVAL_JSON_FILE);
  let data;
  if (fs.existsSync(filePath)) {
    data = readEvalJson(phasesDir);
  } else {
    data = [];
  }
  data.push(entry);
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n", "utf-8");
}

// src/lib/workflow.ts
var PHASES = [
  "01-requirements",
  "02-test-design",
  "03-dev-proposal",
  "04-test-gen",
  "05-implementation",
  "06-code-review",
  "07-acceptance"
];
function getPhaseIndex(phase) {
  return PHASES.indexOf(phase);
}
function getPriorPhases(phase) {
  const idx = getPhaseIndex(phase);
  if (idx <= 0)
    return [];
  return PHASES.slice(0, idx);
}

// src/lib/change.ts
var path2 = __toESM(require("path"));
function resolveChangeDir(changeName) {
  const binDir = path2.resolve(__dirname, "..");
  const pluginDir = path2.resolve(binDir, "..");
  const projectRoot = path2.resolve(pluginDir, "..");
  return path2.resolve(projectRoot, "openspec", "changes", changeName);
}
function getPhasesDir(changeName) {
  return path2.resolve(resolveChangeDir(changeName), "phases");
}

// src/commands/eval-log.ts
function registerEvalLogCommand(cli) {
  cli.command("eval-log", "Append evaluation result to eval.json for a given phase").option("--change <name>", "Change name (corresponds to openspec/changes/<name>)").option("--phase <phase>", "Phase identifier (e.g. 01-requirements)").option("--verdict <verdict>", "Evaluation verdict: pass or fail").option("--report <text>", "Evaluation report text (max 500 chars)").option("--items <json>", "Checklist items as JSON array string").option("--attempt <n>", "Attempt number (auto-calculated from existing entries if omitted)").option("--backtrack-to <phase>", "Backtrack target phase identifier").action((options) => {
    const REQUIRED_ARGS = ["change", "phase", "verdict", "report", "items"];
    const missing = REQUIRED_ARGS.filter(
      (r) => !options[r] || options[r] === ""
    );
    if (missing.length > 0) {
      console.error(`\u9519\u8BEF: \u7F3A\u5C11\u5FC5\u586B\u53C2\u6570: --${missing.join(", --")}`);
      process.exit(1);
    }
    try {
      validateVerdict(options.verdict);
    } catch (e) {
      console.error(`\u9519\u8BEF: ${e.message}`);
      process.exit(1);
    }
    try {
      validateReportLength(options.report);
    } catch (e) {
      console.error(`\u9519\u8BEF: ${e.message}`);
      process.exit(1);
    }
    let items;
    try {
      items = validateItemsJson(options.items);
    } catch (e) {
      console.error(`\u9519\u8BEF: ${e.message}`);
      process.exit(1);
    }
    const phasesDir = getPhasesDir(options.change);
    let entries;
    try {
      entries = readEvalJson(phasesDir);
    } catch (e) {
      console.error(`\u9519\u8BEF: \u8BFB\u53D6 eval.json \u5931\u8D25: ${e.message}`);
      process.exit(1);
    }
    const priorPhases = getPriorPhases(options.phase);
    if (priorPhases.length > 0) {
      const gate = checkGate(entries, priorPhases);
      if (!gate.passed) {
        console.error(
          `\u9519\u8BEF: \u95E8\u63A7\u68C0\u67E5\u672A\u901A\u8FC7 - \u4EE5\u4E0B\u524D\u7F6E\u9636\u6BB5\u7F3A\u5C11 pass \u8BB0\u5F55: ${gate.missing.join(", ")}`
        );
        process.exit(1);
      }
    }
    const explicitAttempt = options.attempt ? parseInt(options.attempt, 10) : void 0;
    let attempt;
    try {
      attempt = computeAttempt(entries, options.phase, explicitAttempt);
    } catch (e) {
      console.error(`\u9519\u8BEF: ${e.message}`);
      process.exit(1);
    }
    const entry = buildEntry({
      phase: options.phase,
      verdict: options.verdict,
      report: options.report,
      items,
      attempt,
      backtrack_to: options.backtrackTo || null
    });
    try {
      appendEntry(phasesDir, entry);
    } catch (e) {
      console.error(`\u9519\u8BEF: \u5199\u5165 eval.json \u5931\u8D25: ${e.message}`);
      process.exit(1);
    }
    console.log(JSON.stringify({ written: true, phase: options.phase, attempt }));
  });
}

// src/index.ts
function main(argv = process.argv) {
  const cli = cac("dev-team");
  cli.usage("[command] [options]");
  cli.help();
  registerEvalLogCommand(cli);
  if (argv.length <= 2) {
    cli.parse([...argv, "--help"]);
    return;
  }
  cli.parse(argv);
}
if (require.main === module) {
  main();
}
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  main
});
