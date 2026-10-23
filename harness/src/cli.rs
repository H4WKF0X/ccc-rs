//! `cargo ccc <command>`: the pipeline's commands.

use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use crate::best::{self, Scored};
use crate::cache::{self, Record};
use crate::checker::{self, Judges};
use crate::config::Config;
use crate::io::{self, OutputStyle};
use crate::layout::{Workspace, file_name, is_example};
use crate::validate::{self, Verdict};
use crate::{Level, ScoreFn, SolveFn, ingest, par, platform};

const HELP: &str = "\
cargo ccc — Cloudflight Coding Contest pipeline

  login                          LOGIN OK / LOGIN FAILED: the codingcontest.org login (from Firefox) and the contest
  mode [online|manual]           show/switch the mode for you; manual = zips and uploads via ~/Downloads
  progress                       your progress on codingcontest.org
  fetch N                        download level N from codingcontest.org and ingest it
  ingest [paths..] [--level N]   unpack a download (zip/dir/files; default: newest zip in Downloads)
  guard N                        check level N's statement PDF for hidden (prompt-injection) text; fetch/ingest do this
  guard-html N                   (humans only) check level N's sandbox HTML for text aimed at an AI; prints findings
  validate N                     run level N on the example(s), compare to the expected output
  all N                          validate, then solve every real input (in parallel) and check the outputs;
                                 outputs already made by this exact build are reused
  run N <in> [out]               run level N on one file (prints to stdout without [out])
  bench N                        time level N on every input (also saves the real outputs)
  best N [files/dirs..]          optimization levels: merge candidate outputs per case into the best
                                 (also reads work/levelN/candidates/); `all` does this for every run
  new-level N [--from M]         create solution/src/levels/levelN.rs (from the stub, or a copy of level M)
  status                         what's ingested, solved and produced
  submit N [--yes]               re-validate, require current + checked outputs, then upload

--budget S (all/run/bench): seconds per input file for searches that check harness::budget::expired().
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
    // --budget S: seconds per input file for searches that check harness::budget::expired().
    let mut args: Vec<String> = args.to_vec();
    if let Some(i) = args.iter().position(|a| a == "--budget") {
        let secs: f64 = args.get(i + 1).and_then(|v| v.parse().ok()).ok_or("--budget needs a number of seconds")?;
        crate::budget::set(std::time::Duration::from_secs_f64(secs));
        args.drain(i..=i + 1);
    }
    let args = &args[..];
    let Some(cmd) = args.first() else {
        println!("{HELP}");
        return Ok(false);
    };
    let ws = Workspace::find()?;
    let cfg = Config::load(&ws.root)?;
    let ctx = Ctx { ws, cfg, levels };
    let rest = &args[1..];
    match cmd.as_str() {
        "ingest" => ingest::run(&ctx.ws, &ctx.cfg, rest),
        "fetch" => platform::fetch(&ctx.ws, &ctx.cfg, level_arg(rest)?),
        "progress" => platform::progress(&ctx.cfg),
        "login" => platform::login(&ctx.cfg),
        "mode" => platform::mode(&ctx.ws, &ctx.cfg, rest.first().map(String::as_str)),
        "guard-html" => crate::guard::html(&ctx.ws, &ctx.cfg, level_arg(rest)?),
        "guard" => {
            crate::guard::run(&ctx.ws, &ctx.cfg, level_arg(rest)?);
            Ok(true)
        }
        "validate" => ctx.validate(level_arg(rest)?),
        "all" => ctx.all(level_arg(rest)?),
        "run" => ctx.run(rest),
        "bench" => ctx.bench(level_arg(rest)?),
        "best" => ctx.best(rest),
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
    fn scorer(&self, n: u32) -> Option<ScoreFn> {
        self.levels.iter().find(|l| l.n == n).and_then(|l| l.score)
    }

    /// Merge a fresh output and/or extra candidates into `work/level<N>/best/` per case, and return
    /// the best lines plus a one-line note. Invalid candidates are skipped (and named in the note).
    fn keep_best(
        &self,
        n: u32,
        score: ScoreFn,
        input: &Path,
        fresh: Option<Vec<String>>,
        extra: Vec<(String, Vec<String>)>,
    ) -> Result<(Vec<String>, String), String> {
        let text = io::read_input(input)?;
        let best_path = self.ws.level_dir(n).join("best").join(file_name(&self.ws.output_for(n, input)));
        let previous = io::read_input(&best_path).ok().map(|t| io::lines(&t));
        let mut candidates: Vec<Scored> = Vec::new();
        let mut rejected = Vec::new();
        let sources = previous
            .map(|l| ("best so far".to_string(), l))
            .into_iter()
            .chain(fresh.map(|l| ("this run".to_string(), l)))
            .chain(extra);
        for (source, lines) in sources {
            match best::evaluate(score, &text, lines, &source) {
                Ok(scored) => candidates.push(scored),
                Err(why) => rejected.push(format!("{source}: {why}")),
            }
        }
        let before = candidates.iter().find(|c| c.source == "best so far").map(Scored::total);
        let this_run = candidates.iter().find(|c| c.source == "this run").map(Scored::total);
        let Some(merged) = best::merge(&candidates, |lines| best::evaluate(score, &text, lines, "merged").ok()) else {
            return Err(format!("no valid output for {}: {}", file_name(input), rejected.join("; ")));
        };
        let total = merged.best.total();
        io::write_lines(&best_path, &merged.best.lines, OutputStyle::LF)?;
        let mut note = format!("score {total}");
        let mut parts = Vec::new();
        if let Some(t) = this_run {
            parts.push(format!("this run {t}"));
        }
        if let Some(b) = before {
            parts.push(if total < b { format!("was {b}, -{}", b - total) } else { format!("was {b}") });
        }
        let mixed = merged.picks.iter().collect::<std::collections::BTreeSet<_>>().len();
        if mixed > 1 {
            parts.push(format!("best per case from {mixed} sources"));
        }
        if !parts.is_empty() {
            note.push_str(&format!(" ({})", parts.join(", ")));
        }
        if !rejected.is_empty() {
            let first = rejected[0].rsplit('/').next().unwrap_or(&rejected[0]);
            note.push_str(&format!("; {} invalid candidate(s) ignored (e.g. {})", rejected.len(), clip_note(first)));
        }
        Ok((merged.best.lines, note))
    }

    /// `cargo ccc best N [files/dirs...]`: merge candidate outputs (teammates', other sessions',
    /// `work/levelN/candidates/`) per case into the best outputs, then check them.
    fn best(&self, args: &[String]) -> Result<bool, String> {
        let n = level_arg(args)?;
        let score = self.scorer(n).ok_or_else(|| {
            format!("level {n} has no `pub fn score` in level{n}.rs: it's needed to compare candidates")
        })?;
        let mut files = Vec::new();
        let mut roots: Vec<PathBuf> = args.iter().skip(1).map(PathBuf::from).collect();
        roots.push(self.ws.level_dir(n).join("candidates"));
        for root in roots {
            collect_files(&root, &mut files);
        }
        let style = self.style(n)?;
        let judges = self.judges(n);
        let mut ok = true;
        for input in self.ws.real_inputs(n)? {
            let id = crate::layout::file_id(&file_name(&input)).unwrap_or_else(|| file_name(&input));
            let extra: Vec<(String, Vec<String>)> = files
                .iter()
                .filter(|f| is_candidate_file(f) && mentions_id(&file_name(f), &id))
                .filter_map(|f| Some((f.display().to_string(), io::lines(&io::read_input(f).ok()?))))
                .collect();
            let out = self.ws.output_for(n, &input);
            let current = io::read_input(&out).ok().map(|t| io::lines(&t));
            let found = extra.len();
            let (lines, note) = self.keep_best(n, score, &input, current, extra)?;
            io::write_lines(&out, &lines, style)?;
            let mut record = Record::new(&input, &out).ok_or("could not hash the output")?;
            let verdict = match &judges {
                Some(j) => {
                    record.checker = verdict_text(j.judge(&self.ws.root, &input, &out));
                    ok &= !record.checker.starts_with("rejected");
                    format!("   checker {}", describe_verdict(&record.checker, false))
                }
                None => String::new(),
            };
            cache::save(&out, &record)?;
            println!("  {:<28} {found} candidate(s), {note}{verdict}", file_name(&out));
        }
        println!("Next: `cargo ccc submit {n}` (only files whose score changed since the last upload are sent)");
        Ok(ok)
    }

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

    /// The usable checkers for level `n`: its own `check` and/or the configured external one, each
    /// only if it accepts the official example answer (otherwise it is ignored with a warning).
    fn judges(&self, n: u32) -> Option<Judges> {
        let check = self.levels.iter().find(|l| l.n == n).and_then(|l| l.check);
        let (judges, warnings) = Judges::for_level(&self.ws, &self.cfg, n, check);
        for warning in warnings {
            println!("  warning: {warning}");
        }
        (!judges.is_empty()).then_some(judges)
    }

    fn validate(&self, n: u32) -> Result<bool, String> {
        let solve = self.solver(n)?;
        let style = self.style(n)?;
        let mut judges: Option<Option<Judges>> = None; // looked up only if needed
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
            let produced = match solve_example(solve, &example.input) {
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
                Verdict::NoGo(detail) => match judges.get_or_insert_with(|| self.judges(n)) {
                    Some(judges) => {
                        let out = self.ws.level_dir(n).join("check").join(file_name(&expected_path));
                        io::write_lines(&out, &produced, style)?;
                        match judges.judge(&self.ws.root, &example.input, &out) {
                            checker::Verdict::Accepted => println!(
                                "GO (checker) — level {n}, {name}: differs from the example, but {} accepts it.",
                                judges.describe()
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
                        println!(
                            "    If level {n} accepts several valid answers, add to level{n}.rs\n    \
                             `pub fn check(input: &str, output: &[String]) -> Result<(), String>` (the statement's\n    \
                             validity rules, independent of solve; it must accept the official example answer)."
                        );
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
                // An explicit --budget asks for a (longer or shorter) search: always run it. On an
                // optimization level the per-case merge keeps the best, so nothing is lost.
                let current = if crate::budget::per_file().is_some() { None } else { cache::current(&input, &out) };
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
                    // Optimization level: never get worse; merge per case with the best so far.
                    let (lines, score_note) = match self.scorer(n) {
                        Some(score) => {
                            let (merged, note) = self.keep_best(n, score, input, Some(lines), Vec::new())?;
                            (merged, format!("   {note}"))
                        }
                        None => (lines, String::new()),
                    };
                    io::write_lines(out, &lines, style)?;
                    let record = Record::new(input, out);
                    if let Some(r) = &record {
                        cache::save(out, r)?;
                    }
                    records.push(record);
                    notes.push(format!("{} lines, solved in {secs:.1} s{score_note}", lines.len()));
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
        let checker = self.judges(n);
        let verdicts: Vec<Option<String>> = match &checker {
            None => vec![None; jobs.len()],
            Some(checker) => std::thread::scope(|s| {
                let handles: Vec<_> = jobs
                    .iter()
                    .zip(&records)
                    .map(|((input, out, _), record)| {
                        let pending = record.as_ref().is_some_and(|r| !r.checker_ok());
                        s.spawn(move || pending.then(|| verdict_text(checker.judge(&self.ws.root, input, out))))
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
            crate::budget::start();
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
        println!("Contest: {}   mode: {}", self.cfg.get("name").unwrap_or("(unnamed)"), self.cfg.mode());
        if self.cfg.mode() == "manual" {
            println!("Next level (manual mode): {}", platform::next_manual_level(&self.ws));
        }
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
        let checker = self.judges(n);
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
                record.checker = verdict_text(checker.judge(&self.ws.root, &input, &out));
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
        // Optimization levels: only upload what changed since the last (online) upload.
        let track = self.scorer(n).is_some() && self.cfg.mode() == "online";
        let uploaded = |out: &Path| self.ws.level_dir(n).join(".uploaded").join(file_name(out));
        if track {
            let (same, changed): (Vec<PathBuf>, Vec<PathBuf>) = outputs
                .into_iter()
                .partition(|o| fs::read_to_string(uploaded(o)).ok() == cache::hash_file(o));
            for o in &same {
                println!("  {} unchanged since the last upload: not sent again", file_name(o));
            }
            if changed.is_empty() {
                println!("Nothing changed since the last upload of level {n}.");
                return Ok(true);
            }
            outputs = changed;
        }
        let result = platform::submit(&self.ws, &self.cfg, n, &outputs, yes)?;
        if track {
            for o in &outputs {
                if let Some(hash) = cache::hash_file(o) {
                    let marker = uploaded(o);
                    let _ = fs::create_dir_all(marker.parent().unwrap());
                    let _ = fs::write(marker, hash);
                }
            }
        }
        Ok(result)
    }
}

/// Output lines and seconds taken, or why solving failed.
type Solved = Result<(Vec<String>, f64), String>;

/// Every file under `root` (recursively); nothing if it doesn't exist.
fn collect_files(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        out.push(root.to_path_buf());
    } else if let Ok(entries) = fs::read_dir(root) {
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| Some(e.ok()?.path())).collect();
        paths.sort();
        for p in paths {
            collect_files(&p, out);
        }
    }
}

/// Could this be someone's output? Not inputs, harness records, logs or sources.
fn is_candidate_file(path: &Path) -> bool {
    let name = file_name(path).to_lowercase();
    let skip_dir = path.components().any(|c| matches!(c.as_os_str().to_str(), Some(".records" | ".git" | "target" | "in")));
    let skip_ext = [".log", ".rs", ".py", ".js", ".sh", ".md", ".toml", ".json", ".pdf", ".html", ".zip"].iter().any(|e| name.ends_with(e));
    !skip_dir && !skip_ext && !name.starts_with("in_") && !name.contains("example")
}

fn clip_note(s: &str) -> String {
    match s.char_indices().nth(100) {
        Some((cut, _)) => format!("{}…", &s[..cut]),
        None => s.to_owned(),
    }
}

/// Does a candidate's file name refer to input file `id` (`1-small`)? `x_1-small.txt`, `1-small.out`
/// yes; `11-small.txt` no.
fn mentions_id(name: &str, id: &str) -> bool {
    name.match_indices(id).any(|(i, _)| !name[..i].ends_with(|c: char| c.is_ascii_digit()))
}

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

/// Like [`solve_file`], for an example: only a short slice of any `--budget`.
fn solve_example(solve: SolveFn, input: &Path) -> Result<Vec<String>, String> {
    let text = io::read_input(input)?;
    crate::budget::start_example();
    catch(|| solve(&text)).map_err(|msg| format!("solver panicked on {}: {msg}", file_name(input)))
}

/// Read an input file and solve it; a panic in the solver becomes an error naming the file.
fn solve_file(solve: SolveFn, input: &Path) -> Result<Vec<String>, String> {
    let text = io::read_input(input)?;
    crate::budget::start();
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
