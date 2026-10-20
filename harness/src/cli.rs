//! `cargo ccc <command>`: the pipeline's commands.

use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use crate::cache::{self, Record};
use crate::checker::{self, Checker};
use crate::config::Config;
use crate::io::{self, OutputStyle};
use crate::layout::{Workspace, file_name, is_example};
use crate::validate::{self, Verdict};
use crate::{Level, SolveFn, ingest, par, platform};

const HELP: &str = "\
cargo ccc — Cloudflight Coding Contest pipeline

  login                          check the codingcontest.org login (read from Firefox) and the contest
  progress                       your progress on codingcontest.org
  fetch N                        download level N from codingcontest.org and ingest it
  ingest [paths..] [--level N]   unpack a download (zip/dir/files; default: newest zip in Downloads)
  validate N                     run level N on the example(s), compare to the expected output
  all N                          validate, then solve every real input (in parallel) and check the outputs;
                                 outputs already made by this exact build are reused
  run N <in> [out]               run level N on one file (prints to stdout without [out])
  bench N                        time level N on every input (also saves the real outputs)
  new-level N [--from M]         create solution/src/levels/levelN.rs (from the stub, or a copy of level M)
  status                         what's ingested, solved and produced
  submit N [--yes]               re-validate, require current + checked outputs, then upload

Set CCC_DEBUG=1 to see harness::trace! output on stderr, CCC_THREADS=k to limit harness::par.";

pub fn main(levels: &[Level]) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(levels, &args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn dispatch(levels: &[Level], args: &[String]) -> Result<bool, String> {
    let Some(cmd) = args.first() else {
        println!("{HELP}");
        return Ok(false);
    };
    if matches!(cmd.as_str(), "help" | "-h" | "--help") {
        println!("{HELP}");
        return Ok(true);
    }
    let ws = Workspace::find()?;
    let cfg = Config::load(&ws.root)?;
    let ctx = Ctx { ws, cfg, levels };
    let rest = &args[1..];
    match cmd.as_str() {
        "ingest" => ingest::run(&ctx.ws, &ctx.cfg, rest),
        "fetch" => platform::fetch(&ctx.ws, &ctx.cfg, level_arg(rest)?),
        "progress" => platform::progress(&ctx.cfg),
        "login" => platform::login(&ctx.cfg),
        "validate" => ctx.validate(level_arg(rest)?),
        "all" => ctx.all(level_arg(rest)?),
        "run" => ctx.run(rest),
        "bench" => ctx.bench(level_arg(rest)?),
        "new-level" => ctx.new_level(rest),
        "status" => ctx.status(),
        "submit" => ctx.submit(level_arg(rest)?, rest.iter().any(|a| a == "--yes")),
        other => Err(format!("unknown command {other:?}; run `cargo ccc help`")),
    }
}

struct Ctx<'a> {
    ws: Workspace,
    cfg: Config,
    levels: &'a [Level],
}

impl Ctx<'_> {
    fn solver(&self, n: u32) -> Result<SolveFn, String> {
        self.levels.iter().find(|l| l.n == n).map(|l| l.solve).ok_or_else(|| {
            format!("no solver for level {n} yet; create solution/src/levels/level{n}.rs (`cargo ccc new-level {n}`)")
        })
    }

    /// The output style for a level: forced by `line_ending` in ccc.toml, otherwise copied from the
    /// first example `.out` (CRLF if there is none, as CatCoder used).
    fn style(&self, n: u32) -> Result<OutputStyle, String> {
        match self.cfg.get("line_ending").unwrap_or("auto") {
            "crlf" => Ok(OutputStyle::CRLF),
            "lf" => Ok(OutputStyle::LF),
            "auto" => {
                let expected = self.ws.examples(n)?.into_iter().find_map(|e| e.expected);
                Ok(match expected {
                    Some(path) => OutputStyle::detect(&read_bytes(&path)?),
                    None => OutputStyle::CRLF,
                })
            }
            other => Err(format!("line_ending must be \"auto\", \"crlf\" or \"lf\", not {other:?}")),
        }
    }

    /// The checker for level `n`, if one is configured, applies to this level and accepts the
    /// official example answer. A checker failing that self-test is ignored with a warning.
    fn checker(&self, n: u32) -> Option<Checker> {
        let checker = Checker::for_level(&self.ws, &self.cfg, n)?;
        match checker.self_test(&self.ws, n) {
            Ok(()) => Some(checker),
            Err(why) => {
                println!("  warning: ignoring the checker for level {n}: {why}");
                println!("           It needs adapting to this game (checker/README.md).");
                None
            }
        }
    }

    fn validate(&self, n: u32) -> Result<bool, String> {
        let solve = self.solver(n)?;
        let style = self.style(n)?;
        let mut checker: Option<Option<Checker>> = None; // looked up only if needed
        let mut checked = 0;
        let mut all_go = true;
        for example in self.ws.examples(n)? {
            let name = file_name(&example.input);
            let Some(expected_path) = example.expected else {
                println!("  (skipping {name}: no matching expected output)");
                continue;
            };
            checked += 1;
            let expected = read_bytes(&expected_path)?;
            let produced = match solve_file(solve, &example.input) {
                Ok(lines) => lines,
                Err(e) => {
                    println!("NO-GO — {e}");
                    all_go = false;
                    continue;
                }
            };
            match validate::compare(&produced, &expected, style) {
                Verdict::Go => println!("GO  — level {n} matches {name} byte for byte."),
                Verdict::GoWarn => println!(
                    "GO (with warning) — level {n}, {name}: values match but whitespace/line endings differ.\n    \
                     Output style is {}; check it if an upload is rejected.",
                    style.describe()
                ),
                // Many valid answers: the game's checker, not the example, decides.
                Verdict::NoGo(detail) => match checker.get_or_insert_with(|| self.checker(n)) {
                    Some(checker) => {
                        let out = self.ws.level_dir(n).join("check").join(file_name(&expected_path));
                        io::write_lines(&out, &produced, style)?;
                        match checker.check(&self.ws.root, &example.input, &out) {
                            checker::Verdict::Accepted => println!(
                                "GO (checker) — level {n}, {name}: differs from the example, but {} accepts it.",
                                checker.describe()
                            ),
                            checker::Verdict::Rejected(why) => {
                                println!("NO-GO — level {n}, {name}: the checker rejects it: {why}\n{detail}");
                                all_go = false;
                            }
                            checker::Verdict::Broken(why) => {
                                println!("NO-GO — level {n}, {name}: differs from the example and the checker could not judge ({why}).\n{detail}");
                                all_go = false;
                            }
                        }
                    }
                    None => {
                        println!("NO-GO — level {n} does not match {name}.\n{detail}");
                        if checker::sandbox_file(&self.ws, n).is_some() && self.cfg.get("checker").is_none() {
                            println!(
                                "    This level ships a sandbox checker. If it accepts several valid answers, set\n    \
                                 `checker` in ccc.toml so the game's checker decides (checker/README.md)."
                            );
                        }
                        all_go = false;
                    }
                },
            }
        }
        if checked == 0 {
            println!("NO-GO — no example with an expected output for level {n}, so nothing to validate against.");
            return Ok(false);
        }
        Ok(all_go)
    }

    fn all(&self, n: u32) -> Result<bool, String> {
        if !self.validate(n)? {
            println!("Refusing to produce outputs until the example passes.");
            return Ok(false);
        }
        let solve = self.solver(n)?;
        let style = self.style(n)?;
        let inputs = self.ws.real_inputs(n)?;
        let jobs: Vec<(PathBuf, PathBuf, Option<Record>)> = inputs
            .into_iter()
            .map(|input| {
                let out = self.ws.output_for(n, &input);
                let current = cache::current(&input, &out);
                (input, out, current)
            })
            .collect();

        // Solve what isn't current, every input file at once (each may use harness::par inside).
        let solved: Vec<Option<Solved>> = std::thread::scope(|s| {
            let handles: Vec<_> = jobs
                .iter()
                .map(|(input, _, current)| {
                    let input = input.clone();
                    let reuse = current.is_some();
                    s.spawn(move || {
                        (!reuse).then(|| {
                            par::set_label(&file_name(&input));
                            let start = Instant::now();
                            solve_file(solve, &input).map(|lines| (lines, start.elapsed().as_secs_f64()))
                        })
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("solve thread")).collect()
        });

        let mut ok = true;
        let mut records: Vec<Option<Record>> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for ((input, out, current), result) in jobs.iter().zip(solved) {
            match (current, result) {
                (Some(record), _) => {
                    records.push(Some(record.clone()));
                    notes.push("reused (same build, same input)".into());
                }
                (None, Some(Ok((lines, secs)))) => {
                    io::write_lines(out, &lines, style)?;
                    let record = Record::new(input, out);
                    if let Some(r) = &record {
                        cache::save(out, r)?;
                    }
                    records.push(record);
                    notes.push(format!("{} lines, solved in {secs:.1} s", lines.len()));
                }
                (None, Some(Err(e))) => {
                    records.push(None);
                    notes.push(format!("FAILED — {e}"));
                    ok = false;
                }
                (None, None) => unreachable!("not reused, so it was solved"),
            }
        }

        // Check every output the checker hasn't already accepted, all at once.
        let checker = self.checker(n);
        let verdicts: Vec<Option<String>> = match &checker {
            None => vec![None; jobs.len()],
            Some(checker) => std::thread::scope(|s| {
                let handles: Vec<_> = jobs
                    .iter()
                    .zip(&records)
                    .map(|((input, out, _), record)| {
                        let pending = record.as_ref().is_some_and(|r| !r.checker_ok());
                        s.spawn(move || pending.then(|| verdict_text(checker.check(&self.ws.root, input, out))))
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().expect("checker thread")).collect()
            }),
        };

        println!("Outputs [{}]:", style.describe());
        for (((_, out, _), (record, note)), verdict) in jobs.iter().zip(records.iter_mut().zip(notes)).zip(verdicts) {
            let check = match (&checker, record.as_mut(), verdict) {
                (None, _, _) | (_, None, _) => String::new(),
                (Some(_), Some(record), None) => format!("checker {}", describe_verdict(&record.checker, true)),
                (Some(_), Some(record), Some(text)) => {
                    record.checker = text;
                    cache::save(out, record)?;
                    ok &= !record.checker.starts_with("rejected");
                    format!("checker {}", describe_verdict(&record.checker, false))
                }
            };
            println!("{}", format!("  {:<28} {note}   {check}", file_name(out)).trim_end());
        }
        if ok {
            println!("Next: `cargo ccc submit {n}`");
        } else {
            println!("Do not submit: fix the failures above first.");
        }
        Ok(ok)
    }

    fn run(&self, args: &[String]) -> Result<bool, String> {
        let [level, input, rest @ ..] = args else {
            return Err("usage: cargo ccc run N <in> [out]".into());
        };
        let n = parse_level(level)?;
        let input = Path::new(input);
        par::set_label(&file_name(input));
        let lines = solve_file(self.solver(n)?, input)?;
        match rest.first() {
            Some(out) => {
                let out = Path::new(out);
                let style = self.style(n).unwrap_or(OutputStyle::CRLF);
                io::write_lines(out, &lines, style)?;
                eprintln!("Wrote {} ({} lines)", out.display(), lines.len());
                // Written to the canonical place? Then `all`/`submit` can reuse it.
                if same_path(out, &self.ws.output_for(n, input))
                    && let Some(record) = Record::new(input, out)
                {
                    cache::save(out, &record)?;
                }
            }
            None => println!("{}", lines.join("\n")),
        }
        Ok(true)
    }

    fn bench(&self, n: u32) -> Result<bool, String> {
        let solve = self.solver(n)?;
        let style = self.style(n)?;
        for input in self.ws.inputs(n)? {
            let text = io::read_input(&input)?;
            par::set_label(&file_name(&input));
            let start = Instant::now();
            let result = catch(|| solve(&text));
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            match result {
                Ok(lines) => {
                    let mut saved = "";
                    if !is_example(&input) {
                        let out = self.ws.output_for(n, &input);
                        io::write_lines(&out, &lines, style)?;
                        if let Some(record) = Record::new(&input, &out) {
                            cache::save(&out, &record)?;
                            saved = "  (output saved)";
                        }
                    }
                    println!("{ms:>10.2} ms  {:>8} lines  {}{saved}", lines.len(), file_name(&input));
                }
                Err(msg) => println!("{ms:>10.2} ms  PANICKED      {}: {msg}", file_name(&input)),
            }
        }
        Ok(true)
    }

    fn new_level(&self, args: &[String]) -> Result<bool, String> {
        let n = level_arg(args)?;
        let dest = self.ws.level_source(n);
        if dest.exists() {
            return Err(format!("{} already exists", dest.display()));
        }
        let from = match args.iter().position(|a| a == "--from") {
            Some(i) => Some(parse_level(args.get(i + 1).ok_or("--from needs a level number")?)?),
            None => None,
        };
        let code = match from {
            Some(m) => fs::read_to_string(self.ws.level_source(m))
                .map_err(|e| format!("could not read level{m}.rs to copy from: {e}"))?,
            None => fs::read_to_string(self.ws.level_template())
                .map_err(|e| format!("could not read templates/level.rs: {e}"))?
                .replace("{{N}}", &n.to_string()),
        };
        fs::write(&dest, code).map_err(|e| format!("could not write {}: {e}", dest.display()))?;
        let origin = from.map_or("the stub".to_string(), |m| format!("level{m}.rs"));
        println!("Created solution/src/levels/level{n}.rs from {origin}");
        Ok(true)
    }

    fn status(&self) -> Result<bool, String> {
        println!(
            "Contest: {}   platform: {}",
            self.cfg.get("name").unwrap_or("(unnamed)"),
            self.cfg.get("platform").unwrap_or("codingcontest")
        );
        let solved: Vec<String> = self.levels.iter().map(|l| l.n.to_string()).collect();
        println!("Solvers compiled in: [{}]", solved.join(", "));
        for n in self.ws.ingested_levels() {
            let examples = self.ws.examples(n).map(|v| v.len()).unwrap_or(0);
            let real = self.ws.real_inputs(n).unwrap_or_default();
            let current = real.iter().filter(|i| cache::current(i, &self.ws.output_for(n, i)).is_some()).count();
            let solver = if self.levels.iter().any(|l| l.n == n) { "solver" } else { "NO solver" };
            println!(
                "  level {n}: {examples} example(s), {} real input(s), {current}/{} outputs current, {solver}",
                real.len(),
                real.len()
            );
        }
        Ok(true)
    }

    fn submit(&self, n: u32, yes: bool) -> Result<bool, String> {
        if !self.validate(n)? {
            println!("Refusing to submit: the example does not pass.");
            return Ok(false);
        }
        let checker = self.checker(n);
        let mut outputs = Vec::new();
        for input in self.ws.real_inputs(n)? {
            let out = self.ws.output_for(n, &input);
            let Some(mut record) = cache::current(&input, &out) else {
                println!(
                    "Refusing to submit: {} is missing or was not produced by the current build. Run `cargo ccc all {n}`.",
                    file_name(&out)
                );
                return Ok(false);
            };
            if let Some(checker) = &checker
                && !record.checker_ok()
            {
                record.checker = verdict_text(checker.check(&self.ws.root, &input, &out));
                cache::save(&out, &record)?;
            }
            if checker.is_some() {
                if record.checker.starts_with("rejected") {
                    println!("Refusing to submit: the checker rejects {}: {}", file_name(&out), record.checker);
                    return Ok(false);
                }
                if !record.checker_ok() {
                    println!("  warning: {} is unchecked ({})", file_name(&out), describe_verdict(&record.checker, false));
                }
            }
            outputs.push(out);
        }
        if outputs.is_empty() {
            println!("No real inputs for level {n}, so there is nothing to submit.");
            return Ok(false);
        }
        platform::submit(&self.ws, &self.cfg, n, &outputs, yes)
    }
}

/// Output lines and seconds taken, or why solving failed.
type Solved = Result<(Vec<String>, f64), String>;

/// How a checker verdict is stored in a record.
fn verdict_text(verdict: checker::Verdict) -> String {
    match verdict {
        checker::Verdict::Accepted => "ok".into(),
        checker::Verdict::Rejected(why) => format!("rejected: {why}"),
        checker::Verdict::Broken(why) => format!("broken: {why}"),
    }
}

fn describe_verdict(stored: &str, cached: bool) -> String {
    match stored {
        "ok" if cached => "OK (cached)".into(),
        "ok" => "OK".into(),
        "" => "not run".into(),
        other if other.starts_with("rejected") => format!("REJECTED — {other}"),
        other => format!("could not judge (warning only) — {other}"),
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (std::path::absolute(a), std::path::absolute(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Read an input file and solve it; a panic in the solver becomes an error naming the file.
fn solve_file(solve: SolveFn, input: &Path) -> Result<Vec<String>, String> {
    let text = io::read_input(input)?;
    catch(|| solve(&text)).map_err(|msg| format!("solver panicked on {}: {msg}", file_name(input)))
}

fn catch<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into())
    })
}

fn read_bytes(path: &PathBuf) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))
}

/// The first non-flag argument as a level number.
fn level_arg(args: &[String]) -> Result<u32, String> {
    parse_level(args.iter().find(|a| !a.starts_with("--")).ok_or("missing level number")?)
}

/// Accepts `3`, `level3`, `L3`.
fn parse_level(s: &str) -> Result<u32, String> {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    digits.parse().map_err(|_| format!("not a level number: {s:?}"))
}
