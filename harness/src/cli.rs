//! `cargo ccc <command>`: the pipeline's commands.

use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use crate::config::Config;
use crate::io::{self, OutputStyle};
use crate::layout::{Workspace, file_name};
use crate::validate::{self, Verdict};
use crate::{Level, SolveFn, ingest, platform};

const HELP: &str = "\
cargo ccc — Cloudflight Coding Contest pipeline

  ingest [paths..] [--level N]   unpack a download (zip/dir/files; default: newest zip in Downloads)
  fetch N                        download level N from codingcontest.org and ingest it
  validate N                     run level N on the example(s), compare to the expected output
  all N                          validate, then (only on GO) write every real output to work/levelN/out
  run N <in> [out]               run level N on one file (prints to stdout without [out])
  bench N                        time level N on every input
  new-level N [--from M]         create solution/src/levels/levelN.rs (from the stub, or a copy of level M)
  status                         what's ingested, solved and produced
  progress                       your progress on codingcontest.org (also checks the credentials)
  submit N [--yes]               re-validate, check outputs are fresh, then hand them to the platform

Set CCC_DEBUG=1 to see harness::trace! output on stderr.";

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

    fn validate(&self, n: u32) -> Result<bool, String> {
        let solve = self.solver(n)?;
        let style = self.style(n)?;
        let mut checked = 0;
        let mut all_go = true;
        for example in self.ws.examples(n)? {
            let name = file_name(&example.input);
            let Some(expected_path) = example.expected else {
                println!("  (skipping {name}: no matching .out)");
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
                Verdict::NoGo(detail) => {
                    println!("NO-GO — level {n} does not match {name}.\n{detail}");
                    all_go = false;
                }
            }
        }
        if checked == 0 {
            println!("NO-GO — no example with an expected .out for level {n}, so nothing to validate against.");
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
        let mut produced = Vec::new();
        let mut ok = true;
        for input in self.ws.real_inputs(n)? {
            match solve_file(solve, &input) {
                Ok(lines) => {
                    let out = self.ws.output_for(n, &input);
                    io::write_lines(&out, &lines, style)?;
                    produced.push(format!("{} ({} lines)", file_name(&out), lines.len()));
                }
                Err(e) => {
                    println!("FAILED — {e}");
                    ok = false;
                }
            }
        }
        println!("Produced {} output file(s) [{}]: {}", produced.len(), style.describe(), produced.join(", "));
        if ok {
            println!("Next: review (/review {n}), then `cargo ccc submit {n}`");
        }
        Ok(ok)
    }

    fn run(&self, args: &[String]) -> Result<bool, String> {
        let [level, input, rest @ ..] = args else {
            return Err("usage: cargo ccc run N <in> [out]".into());
        };
        let n = parse_level(level)?;
        let lines = solve_file(self.solver(n)?, Path::new(input))?;
        match rest.first() {
            Some(out) => {
                let style = self.style(n).unwrap_or(OutputStyle::CRLF);
                io::write_lines(Path::new(out), &lines, style)?;
                eprintln!("Wrote {out} ({} lines)", lines.len());
            }
            None => println!("{}", lines.join("\n")),
        }
        Ok(true)
    }

    fn bench(&self, n: u32) -> Result<bool, String> {
        let solve = self.solver(n)?;
        for input in self.ws.inputs(n)? {
            let text = io::read_input(&input)?;
            let start = Instant::now();
            let result = catch(|| solve(&text));
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            match result {
                Ok(lines) => println!("{ms:>10.2} ms  {:>8} lines  {}", lines.len(), file_name(&input)),
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
            self.cfg.get("platform").unwrap_or("manual")
        );
        let solved: Vec<String> = self.levels.iter().map(|l| l.n.to_string()).collect();
        println!("Solvers compiled in: [{}]", solved.join(", "));
        for n in self.ws.ingested_levels() {
            let examples = self.ws.examples(n).map(|v| v.len()).unwrap_or(0);
            let real = self.ws.real_inputs(n).unwrap_or_default();
            let outputs = real.iter().filter(|i| self.ws.output_for(n, i).is_file()).count();
            let solver = if self.levels.iter().any(|l| l.n == n) { "solver" } else { "NO solver" };
            println!(
                "  level {n}: {examples} example(s), {} real input(s), {outputs}/{} outputs, {solver}",
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
        let source_time = fs::metadata(self.ws.level_source(n)).and_then(|m| m.modified()).ok();
        let mut outputs = Vec::new();
        for input in self.ws.real_inputs(n)? {
            let out = self.ws.output_for(n, &input);
            let out_time = fs::metadata(&out).and_then(|m| m.modified()).map_err(|_| {
                format!("missing {}; run `cargo ccc all {n}` first", file_name(&out))
            })?;
            if source_time.is_some_and(|src| src > out_time) {
                println!("Refusing to submit: {} is older than level{n}.rs. Run `cargo ccc all {n}`.", file_name(&out));
                return Ok(false);
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
