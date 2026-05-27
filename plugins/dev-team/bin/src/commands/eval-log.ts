import { CAC } from "cac";
import {
  readEvalJson,
  validateVerdict,
  validateReportLength,
  validateItemsJson,
  buildEntry,
  computeAttempt,
  checkGate,
  appendEntry,
} from "../lib/eval-json";
import { getPriorPhases } from "../lib/workflow";
import { getChangeDir } from "../lib/change";

/**
 * Register the eval-log subcommand on the given cac CLI instance.
 * Uses cac's declarative API: .command().option().action()
 */
export function registerEvalLogCommand(cli: CAC): void {
  cli
    .command("eval-log", "Append evaluation result to eval.json for a given phase")
    .option("--change <name>", "Change name (corresponds to openspec/changes/<name>)")
    .option("--phase <phase>", "Phase identifier (e.g. 01-requirements)")
    .option("--verdict <verdict>", "Evaluation verdict: pass or fail")
    .option("--report <text>", "Evaluation report text (max 500 chars)")
    .option("--items <json>", "Checklist evaluation items as JSON array string. Each item: {\"item\":\"检查项\",\"pass\":true|false,\"evidence\":\"...\",\"notes\":\"...\"}")
    .option("--attempt <n>", "Attempt number (auto-calculated from existing entries if omitted)")
    .option("--backtrack-to <phase>", "Backtrack target phase identifier")
    .option("--skipped", "Mark entry as skipped (no-op phase, requires verdict pass)")
    .option("--findings <text>", "Diagnostic findings text from decision tree analysis")
    .action((options: Record<string, any>) => {
      // Note: cac converts --backtrack-to to camelCase, access via options.backtrackTo
      // Note: --attempt is passed as string, needs parseInt

      // 0) Validate required arguments (cac's { required: true } only catches
      //    flag-style missing values, not completely missing options)
      const REQUIRED_ARGS = ["change", "phase", "verdict", "report", "items"];
      const missing = REQUIRED_ARGS.filter((r) => !options[r] || options[r] === "");
      if (missing.length > 0) {
        console.error(`错误: 缺少必填参数: --${missing.join(", --")}`);
        process.exit(1);
      }
      try {
        validateVerdict(options.verdict, options.skipped === true);
      } catch (e: any) {
        console.error(`错误: ${e.message}`);
        process.exit(1);
      }

      // 2) Validate report length
      try {
        validateReportLength(options.report);
      } catch (e: any) {
        console.error(`错误: ${e.message}`);
        process.exit(1);
      }

      // 3) Parse and validate items JSON
      let items: any[];
      try {
        items = validateItemsJson(options.items);
      } catch (e: any) {
        console.error(`错误: ${e.message}`);
        process.exit(1);
      }

      // 4) Resolve change directory
      const changeDir = getChangeDir(options.change);

      // 5) Read existing eval entries
      let entries: any[];
      try {
        entries = readEvalJson(changeDir);
      } catch (e: any) {
        console.error(`错误: 读取 eval.json 失败: ${e.message}`);
        process.exit(1);
      }

      // 6) Gate check: verify prior phases have pass records
      const priorPhases = getPriorPhases(options.phase);
      if (priorPhases.length > 0) {
        const gate = checkGate(entries, priorPhases);
        if (!gate.passed) {
          console.error(
            `错误: 门控检查未通过 - 以下前置阶段缺少 pass 记录: ${gate.missing.join(", ")}`,
          );
          process.exit(1);
        }
      }

      // 7) Compute attempt
      const explicitAttempt = options.attempt ? parseInt(options.attempt, 10) : undefined;
      let attempt: number;
      try {
        attempt = computeAttempt(entries, options.phase, explicitAttempt);
      } catch (e: any) {
        console.error(`错误: ${e.message}`);
        process.exit(1);
      }

      // 8) Build entry
      const entry = buildEntry({
        phase: options.phase,
        verdict: options.verdict,
        report: options.report,
        items,
        attempt,
        backtrack_to: options.backtrackTo || null,
        skipped: options.skipped === true ? true : undefined,
        findings: options.findings || undefined,
      });

      // 9) Append entry
      try {
        appendEntry(changeDir, entry);
      } catch (e: any) {
        console.error(`错误: 写入 eval.json 失败: ${e.message}`);
        process.exit(1);
      }

      // 10) Output success
      console.log(JSON.stringify({ written: true, phase: options.phase, attempt }));
    });
}
