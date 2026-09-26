mod analysis;
mod profile;

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Mutex;

use analysis::{
    activity_heatmap, call_tree, callers_callees, detect_runs, find_gaps, identify_phases,
    sample_at, search_stacks, thread_activity, thread_sample_gaps, thread_timeline, top_functions,
};
use profile::Profile;
use serde_json::{Value, json};

// ── Global profile state ────────────────────────────────────────────

static PROFILE: Mutex<Option<Profile>> = Mutex::new(None);

fn with_profile<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&Profile) -> T,
{
    let guard = PROFILE.lock().map_err(|e| format!("lock poisoned: {e}"))?;
    match guard.as_ref() {
        Some(p) => Ok(f(p)),
        None => Err("No profile loaded. Use load_profile first.".into()),
    }
}

fn with_profile_result<F>(f: F) -> Result<String, String>
where
    F: FnOnce(&Profile) -> Result<String, String>,
{
    with_profile(f)?
}

// ── Tool definitions ────────────────────────────────────────────────

fn tool_definitions() -> Value {
    json!([
        {
            "name": "load_profile",
            "description": "Load a samply Firefox Profiler JSON profile (plain or gzipped). Automatically loads companion .syms.json if present. Must be called before any analysis tools.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to profile.json or profile.json.gz"
                    },
                    "syms_path": {
                        "type": "string",
                        "description": "Optional path to .syms.json symbol file. If omitted, auto-detects companion file next to profile."
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "profile_summary",
            "description": "High-level overview: thread counts, main thread, detected runs, and top threads by sample count. Call this first after loading.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "top_functions",
            "description": "Flat profile: rank functions by self-time (leaf sample count). The primary tool for finding hot spots.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "thread": { "type": "string", "description": "Filter to threads whose name contains this string" },
                    "limit": { "type": "integer", "description": "Max functions to return (default: 30)" }
                }
            }
        },
        {
            "name": "callers_callees",
            "description": "Butterfly/sandwich view: for a given function, show who calls it (callers) and what it calls (callees) with sample counts. Use after top_functions to understand calling context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "function": { "type": "string", "description": "Function name (or substring) to analyze" },
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "thread": { "type": "string", "description": "Filter to threads whose name contains this string" }
                },
                "required": ["function"]
            }
        },
        {
            "name": "call_tree",
            "description": "Text flamegraph: hierarchical call tree with inclusive time percentages. Shows the full call hierarchy from root to leaf functions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "thread": { "type": "string", "description": "Filter to threads whose name contains this string" },
                    "max_depth": { "type": "integer", "description": "Maximum stack depth (default: 20)" },
                    "min_pct": { "type": "number", "description": "Minimum percentage to show a node (default: 1.0)" }
                }
            }
        },
        {
            "name": "search_stacks",
            "description": "Grep for a function name across all sampled stacks. Returns matching stack traces with timestamps. Use to find where/when a specific function appears.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Function name or substring to search for" },
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "thread": { "type": "string", "description": "Filter to threads whose name contains this string" },
                    "max_results": { "type": "integer", "description": "Maximum matches to return (default: 20)" }
                },
                "required": ["pattern"]
            }
        },
        {
            "name": "activity_heatmap",
            "description": "ASCII heatmap of CPU core utilization over time. Shows how many CPU cores are active in each time bin.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "bin_ms": { "type": "number", "description": "Time bin size in ms (default: 5)" },
                    "compact": { "type": "boolean", "description": "Show every 5th bin only (default: true)" }
                }
            }
        },
        {
            "name": "find_gaps",
            "description": "Find idle periods. Without 'thread': CPU utilization gaps (all cores idle). With 'thread': sample-stream gaps for matching threads (thread was unsampled/blocked).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "thread": { "type": "string", "description": "If set, find sample-stream gaps for this thread instead of CPU utilization gaps" },
                    "bin_ms": { "type": "number", "description": "Time bin size for CPU mode (default: 5)" },
                    "idle_threshold": { "type": "number", "description": "CPU utilization fraction for CPU mode (default: 0.2)" },
                    "min_gap_ms": { "type": "number", "description": "Minimum gap duration in ms (default: 10 for CPU mode, 5 for thread mode)" }
                }
            }
        },
        {
            "name": "identify_phases",
            "description": "Classify time ranges into parallel/sequential/idle phases based on CPU core utilization patterns.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "bin_ms": { "type": "number", "description": "Time bin size in ms (default: 2)" }
                }
            }
        },
        {
            "name": "thread_timeline",
            "description": "Show sampled call stacks over time for one or more threads. Use to see what threads are doing at specific times.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" },
                    "thread": { "type": "string", "description": "Filter to threads whose name contains this string (default: all process threads)" },
                    "sample_every": { "type": "integer", "description": "Show every Nth sample (default: 32)" },
                    "stack_depth": { "type": "integer", "description": "Stack depth to show (default: 4)" }
                }
            }
        },
        {
            "name": "thread_activity",
            "description": "Per-thread activity breakdown: sample counts, estimated active time, first/last sample within a range.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run": { "type": "integer", "description": "Run index (0-based, default: 0)" },
                    "start_ms": { "type": "number", "description": "Start time override" },
                    "end_ms": { "type": "number", "description": "End time override" }
                }
            }
        },
        {
            "name": "sample_at",
            "description": "Snapshot: show what every thread is doing at a specific timestamp. Finds the nearest sample within tolerance for each active thread.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "time_ms": { "type": "number", "description": "Timestamp in ms to snapshot" },
                    "tolerance_ms": { "type": "number", "description": "Max distance from time_ms to match a sample (default: 1.0)" },
                    "stack_depth": { "type": "integer", "description": "Stack depth to show (default: 6)" }
                },
                "required": ["time_ms"]
            }
        }
    ])
}

// ── Time range resolution ───────────────────────────────────────────

fn resolve_time_range(profile: &Profile, args: &Value) -> Result<(f64, f64), String> {
    // Explicit start/end override
    if let (Some(s), Some(e)) = (
        args.get("start_ms").and_then(|v| v.as_f64()),
        args.get("end_ms").and_then(|v| v.as_f64()),
    ) {
        return Ok((s, e));
    }

    // By run index
    let runs = detect_runs(profile, 50.0);
    if runs.is_empty() {
        return Err("No runs detected in profile.".into());
    }

    let run_idx = args.get("run").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if run_idx >= runs.len() {
        return Err(format!(
            "Run index {run_idx} out of range (0..{})",
            runs.len()
        ));
    }

    let run = &runs[run_idx];
    Ok((run.start_ms, run.end_ms))
}

// ── Tool dispatch ───────────────────────────────────────────────────

fn handle_tool_call(name: &str, args: &Value) -> Value {
    match name {
        "load_profile" => handle_load_profile(args),
        "profile_summary" => handle_profile_summary(),
        "top_functions" => handle_top_functions(args),
        "callers_callees" => handle_callers_callees(args),
        "call_tree" => handle_call_tree(args),
        "search_stacks" => handle_search_stacks(args),
        "activity_heatmap" => handle_activity_heatmap(args),
        "find_gaps" => handle_find_gaps(args),
        "identify_phases" => handle_identify_phases(args),
        "thread_timeline" => handle_thread_timeline(args),
        "thread_activity" => handle_thread_activity(args),
        "sample_at" => handle_sample_at(args),
        _ => error_result(&format!("Unknown tool: {name}")),
    }
}

fn text_result(text: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    })
}

fn error_result(text: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": true
    })
}

// ── Tool handlers ───────────────────────────────────────────────────

fn handle_load_profile(args: &Value) -> Value {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return error_result("Missing required parameter: path"),
    };
    let syms_path = args.get("syms_path").and_then(|v| v.as_str());

    let p = Path::new(path);
    if !p.exists() {
        return error_result(&format!("File not found: {path}"));
    }

    let syms_p = syms_path.map(Path::new);

    match Profile::load(p, syms_p) {
        Ok(profile) => {
            let summary = format!(
                "Loaded profile: {} threads ({} CPU, {} process)\nSymbols: {}\nMain thread: {}",
                profile.thread_count(),
                profile.cpu_count(),
                profile.process_count(),
                if profile.syms_loaded {
                    "loaded from .syms.json"
                } else {
                    "none (use --unstable-presymbolicate or provide .syms.json)"
                },
                profile
                    .main_thread()
                    .map(|t| format!(
                        "{} (tid={}, {} samples)",
                        t.name,
                        t.tid,
                        t.sample_times.len()
                    ))
                    .unwrap_or_else(|| "none detected".into()),
            );
            let mut guard = PROFILE.lock().unwrap();
            *guard = Some(profile);
            text_result(&summary)
        }
        Err(e) => error_result(&format!("Failed to load profile: {e}")),
    }
}

fn handle_profile_summary() -> Value {
    match with_profile(|p| {
        let runs = detect_runs(p, 50.0);
        let threads = p.list_threads();
        let main_info = p.main_thread().map(|t| {
            format!(
                "  Name: {}\n  TID: {}\n  Samples: {}",
                t.name,
                t.tid,
                t.sample_times.len()
            )
        });

        let mut out = String::new();
        out.push_str(&format!(
            "Profile: {} threads ({} CPU, {} process)\nSymbols: {}\n\n",
            p.thread_count(),
            p.cpu_count(),
            p.process_count(),
            if p.syms_loaded {
                "yes (.syms.json)"
            } else {
                "no"
            }
        ));

        out.push_str("Main thread:\n");
        out.push_str(&main_info.unwrap_or_else(|| "  not detected".into()));
        out.push_str("\n\n");

        out.push_str(&format!("Detected runs: {}\n", runs.len()));
        for r in &runs {
            out.push_str(&format!(
                "  Run {}: {:.1} - {:.1}ms ({:.1}ms)\n",
                r.index, r.start_ms, r.end_ms, r.duration_ms
            ));
        }

        out.push_str(&format!(
            "\nTop threads by samples ({} total):\n",
            threads.len()
        ));
        for t in threads.iter().take(10) {
            out.push_str(&format!(
                "  {:40} {:>8} samples{}\n",
                t.name,
                t.sample_count,
                if t.is_main { " (main)" } else { "" }
            ));
        }

        out
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_activity_heatmap(args: &Value) -> Value {
    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let bin_ms = args.get("bin_ms").and_then(|v| v.as_f64()).unwrap_or(5.0);
        let compact = args
            .get("compact")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let heatmap = activity_heatmap(p, start, end, bin_ms);

        let bar_width = 60;
        let mut out = format!(
            "Activity heatmap ({:.1} - {:.1}ms, bin={:.0}ms, {} CPUs):\n\n",
            start,
            end,
            bin_ms,
            p.cpu_count()
        );

        let step = if compact {
            (bin_ms * 5.0).max(1.0) as usize
        } else {
            1
        };

        for bin in &heatmap {
            let offset = bin.bin_start_ms - start;
            let offset_int = offset as usize;
            if compact && step > 1 && offset_int % step != 0 {
                continue;
            }
            let total = bin.total_cpus.max(1);
            let filled = bin.active_cpus * bar_width / total;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
            out.push_str(&format!(
                "  +{:6.0}ms |{}| {:2}/{}\n",
                offset, bar, bin.active_cpus, bin.total_cpus
            ));
        }
        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_find_gaps(args: &Value) -> Value {
    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let thread_filter = args.get("thread").and_then(|v| v.as_str());

        if let Some(tf) = thread_filter {
            // Thread sample-stream gap mode
            let min_gap_ms = args
                .get("min_gap_ms")
                .and_then(|v| v.as_f64())
                .unwrap_or(5.0);
            let gaps = thread_sample_gaps(p, Some(tf), start, end, min_gap_ms);

            let mut out = format!(
                "Sample-stream gaps for '{}' (>{:.0}ms) in {:.1}-{:.1}ms:\n\n",
                tf, min_gap_ms, start, end
            );

            if gaps.is_empty() {
                out.push_str("  No gaps found.\n");
            } else {
                out.push_str(&format!(
                    "{:>10}  {:>8}  {}\n",
                    "Offset", "Duration", "Context"
                ));
                out.push_str(&"-".repeat(60));
                out.push('\n');
                for g in &gaps {
                    let offset = g.start_ms - start;
                    out.push_str(&format!(
                        "  +{:7.1}ms  {:7.1}ms  {} → {}\n",
                        offset, g.duration_ms, g.before_func, g.after_func
                    ));
                }
            }
            Ok(out)
        } else {
            // CPU utilization gap mode
            let bin_ms = args.get("bin_ms").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let idle_threshold = args
                .get("idle_threshold")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.2);
            let min_gap_ms = args
                .get("min_gap_ms")
                .and_then(|v| v.as_f64())
                .unwrap_or(10.0);

            let gaps = find_gaps(p, start, end, bin_ms, idle_threshold, min_gap_ms);

            let mut out = format!(
                "CPU idle gaps (<{:.0}% utilization, >{:.0}ms) in {:.1}-{:.1}ms:\n\n",
                idle_threshold * 100.0,
                min_gap_ms,
                start,
                end
            );

            if gaps.is_empty() {
                out.push_str("  No gaps found.\n");
            } else {
                out.push_str(&format!(
                    "{:>10}  {:>8}  {}\n",
                    "Offset", "Duration", "Context"
                ));
                out.push_str(&"-".repeat(60));
                out.push('\n');
                for g in &gaps {
                    let offset = g.start_ms - start;
                    out.push_str(&format!(
                        "  +{:7.1}ms  {:7.1}ms  {} → {}\n",
                        offset, g.duration_ms, g.before_func, g.after_func
                    ));
                }
            }
            Ok(out)
        }
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_identify_phases(args: &Value) -> Value {
    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let bin_ms = args.get("bin_ms").and_then(|v| v.as_f64()).unwrap_or(2.0);

        let phases = identify_phases(p, start, end, bin_ms);

        let mut out = format!("Phases in {:.1}-{:.1}ms:\n\n", start, end);
        out.push_str(&format!(
            "{:>10}  {:>12}  {:>8}  {:>10}\n",
            "Offset", "Type", "Duration", "Avg CPUs"
        ));
        out.push_str(&"-".repeat(50));
        out.push('\n');

        for ph in &phases {
            let offset = ph.start_ms - start;
            out.push_str(&format!(
                "  +{:7.1}ms  {:12}  {:7.1}ms  {:5.1} CPUs\n",
                offset,
                ph.phase_type.as_str(),
                ph.duration_ms,
                ph.avg_cpus
            ));
        }
        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_thread_timeline(args: &Value) -> Value {
    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let thread_filter = args.get("thread").and_then(|v| v.as_str());
        let sample_every = args
            .get("sample_every")
            .and_then(|v| v.as_u64())
            .unwrap_or(32) as usize;
        let stack_depth = args
            .get("stack_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(4) as usize;

        let timeline = thread_timeline(p, thread_filter, start, end, sample_every, stack_depth);

        let base = timeline.first().map(|e| e.time_ms).unwrap_or(start);
        let filter_desc = thread_filter
            .map(|f| format!(", thread: '{f}'"))
            .unwrap_or_else(|| ", all threads".into());
        let mut out = format!(
            "Thread timeline ({:.1}-{:.1}ms, every {}th sample, depth {}{}):\n\n",
            start, end, sample_every, stack_depth, filter_desc
        );

        for entry in &timeline {
            let offset = entry.time_ms - base;
            out.push_str(&format!(
                "+{:7.1}ms [{}]: {}\n",
                offset,
                entry.thread_name,
                entry.frames.join(" | ")
            ));
        }
        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_top_functions(args: &Value) -> Value {
    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let thread_filter = args.get("thread").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

        let funcs = top_functions(p, thread_filter, start, end, limit);

        let mut out = format!(
            "Top functions by self-time ({:.1}-{:.1}ms{}):\n\n",
            start,
            end,
            thread_filter
                .map(|f| format!(", thread: '{f}'"))
                .unwrap_or_default()
        );

        out.push_str(&format!("{:>6}  {:>6}  {}\n", "Count", "%", "Function"));
        out.push_str(&"-".repeat(60));
        out.push('\n');

        for f in &funcs {
            out.push_str(&format!(
                "{:>6}  {:>5.1}%  {}\n",
                f.count, f.percentage, f.name
            ));
        }
        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_callers_callees(args: &Value) -> Value {
    let function = match args.get("function").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return error_result("Missing required parameter: function"),
    };

    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let thread_filter = args.get("thread").and_then(|v| v.as_str());

        let result = callers_callees(p, function, thread_filter, start, end);

        let mut out = format!(
            "Callers/callees for '{}' ({:.1}-{:.1}ms):\n",
            result.function, start, end
        );
        out.push_str(&format!(
            "  Total: {} samples, Self: {} samples\n\n",
            result.total_count, result.self_count
        ));

        out.push_str("Callers (who calls this function):\n");
        if result.callers.is_empty() {
            out.push_str("  (none — this is a root function)\n");
        } else {
            for c in &result.callers {
                out.push_str(&format!(
                    "  {:>5.1}% ({:>5})  {}\n",
                    c.percentage, c.count, c.name
                ));
            }
        }

        out.push_str("\nCallees (what this function calls):\n");
        if result.callees.is_empty() {
            out.push_str("  (none — this is a leaf function)\n");
        } else {
            for c in &result.callees {
                out.push_str(&format!(
                    "  {:>5.1}% ({:>5})  {}\n",
                    c.percentage, c.count, c.name
                ));
            }
        }

        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_call_tree(args: &Value) -> Value {
    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let thread_filter = args.get("thread").and_then(|v| v.as_str());
        let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        let min_pct = args.get("min_pct").and_then(|v| v.as_f64()).unwrap_or(1.0);

        let tree = call_tree(p, thread_filter, start, end, max_depth, min_pct);

        let mut out = format!(
            "Call tree ({:.1}-{:.1}ms, depth≤{}, ≥{:.1}%{}):\n\n",
            start,
            end,
            max_depth,
            min_pct,
            thread_filter
                .map(|f| format!(", thread: '{f}'"))
                .unwrap_or_default()
        );
        out.push_str(&tree);
        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_search_stacks(args: &Value) -> Value {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return error_result("Missing required parameter: pattern"),
    };

    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let thread_filter = args.get("thread").and_then(|v| v.as_str());
        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        let matches = search_stacks(p, pattern, thread_filter, start, end, max_results);

        let mut out = format!(
            "Stack search for '{}' ({:.1}-{:.1}ms): {} matches\n\n",
            pattern,
            start,
            end,
            matches.len()
        );

        for (i, m) in matches.iter().enumerate() {
            out.push_str(&format!(
                "#{} +{:.1}ms [{}]:\n",
                i + 1,
                m.time_ms - start,
                m.thread_name
            ));
            for (depth, frame) in m.frames.iter().enumerate() {
                out.push_str(&format!("  {:>2}: {}\n", depth, frame));
            }
            out.push('\n');
        }

        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_thread_activity(args: &Value) -> Value {
    match with_profile_result(|p| {
        let (start, end) = resolve_time_range(p, args)?;
        let activity = thread_activity(p, start, end);

        let mut out = format!("Thread activity ({:.1}-{:.1}ms):\n\n", start, end);
        out.push_str(&format!(
            "{:<40} {:>8} {:>10} {:>10} {:>10}\n",
            "Thread", "Samples", "Active ms", "First ms", "Last ms"
        ));
        out.push_str(&"-".repeat(80));
        out.push('\n');

        for ta in &activity {
            out.push_str(&format!(
                "{:<40} {:>8} {:>10.2} {:>10.1} {:>10.1}\n",
                ta.name,
                ta.sample_count,
                ta.active_ms,
                ta.first_ms - start,
                ta.last_ms - start
            ));
        }
        Ok(out)
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

fn handle_sample_at(args: &Value) -> Value {
    let time_ms = match args.get("time_ms").and_then(|v| v.as_f64()) {
        Some(t) => t,
        None => return error_result("Missing required parameter: time_ms"),
    };
    let tolerance_ms = args
        .get("tolerance_ms")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let stack_depth = args
        .get("stack_depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(6) as usize;

    match with_profile(|p| {
        let snapshots = sample_at(p, time_ms, tolerance_ms, stack_depth);

        let mut out = format!(
            "Thread snapshot at {:.1}ms (±{:.1}ms): {} threads active\n\n",
            time_ms,
            tolerance_ms,
            snapshots.len()
        );

        for s in &snapshots {
            out.push_str(&format!(
                "[{}] at {:.1}ms:\n",
                s.thread_name, s.sample_time_ms
            ));
            for (depth, frame) in s.frames.iter().enumerate() {
                out.push_str(&format!("  {:>2}: {}\n", depth, frame));
            }
            out.push('\n');
        }

        out
    }) {
        Ok(text) => text_result(&text),
        Err(e) => error_result(&e),
    }
}

// ── MCP stdio protocol ──────────────────────────────────────────────

fn write_response(msg: &Value) {
    let mut stdout = io::stdout().lock();
    let _ = serde_json::to_writer(&mut stdout, msg);
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
}

fn handle_initialize(req_id: &Value) {
    write_response(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "samply-mcp", "version": "0.1.0" },
            "instructions": concat!(
                "MCP server for analyzing samply Firefox Profiler JSON profiles. ",
                "Start by calling load_profile, then profile_summary for orientation. ",
                "Use top_functions to find hot spots, callers_callees for calling context, ",
                "call_tree for a text flamegraph. search_stacks greps for functions across stacks. ",
                "activity_heatmap shows CPU utilization over time, find_gaps detects idle periods ",
                "(CPU-wide or per-thread), identify_phases classifies parallel/sequential/idle. ",
                "thread_timeline shows call stacks over time, sample_at snapshots all threads at a point."
            )
        }
    }));
}

fn handle_tools_list(req_id: &Value) {
    write_response(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": { "tools": tool_definitions() }
    }));
}

fn handle_tools_call(req_id: &Value, params: &Value) {
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = handle_tool_call(name, &args);

    write_response(&json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "result": result
    }));
}

fn main() {
    let stdin = io::stdin().lock();

    for line in stdin.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let req_id = msg.get("id").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => handle_initialize(&req_id),
            "notifications/initialized" => {}
            "tools/list" => handle_tools_list(&req_id),
            "tools/call" => {
                let params = msg.get("params").cloned().unwrap_or(json!({}));
                handle_tools_call(&req_id, &params);
            }
            _ => {
                if !req_id.is_null() {
                    write_response(&json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "error": {
                            "code": -32601,
                            "message": format!("Method not found: {method}")
                        }
                    }));
                }
            }
        }
    }
}
