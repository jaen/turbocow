use std::collections::HashMap;

use crate::profile::Profile;

// ── Data types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RunInfo {
    pub index: usize,
    pub start_ms: f64,
    pub end_ms: f64,
    pub duration_ms: f64,
}

#[derive(Debug, Clone)]
pub struct Gap {
    pub start_ms: f64,
    pub end_ms: f64,
    pub duration_ms: f64,
    pub before_func: String,
    pub after_func: String,
    pub activity_ratio: f64,
}

#[derive(Debug, Clone)]
pub struct HeatmapBin {
    pub bin_start_ms: f64,
    pub active_cpus: usize,
    pub total_cpus: usize,
}

#[derive(Debug, Clone)]
pub struct Phase {
    pub start_ms: f64,
    pub end_ms: f64,
    pub duration_ms: f64,
    pub avg_cpus: f64,
    pub phase_type: PhaseType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseType {
    Idle,
    Sequential,
    Parallel,
}

impl PhaseType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Sequential => "sequential",
            Self::Parallel => "parallel",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSample {
    pub name: String,
    pub count: usize,
    pub percentage: f64,
}

// ── Run detection ───────────────────────────────────────────────────

pub fn detect_runs(profile: &Profile, gap_threshold_ms: f64) -> Vec<RunInfo> {
    let main = match profile.main_thread() {
        Some(t) => t,
        None => return vec![],
    };
    let times = &main.sample_times;
    if times.is_empty() {
        return vec![];
    }

    let mut runs = Vec::new();
    let mut run_start = times[0];
    let mut prev_t = times[0];

    for &t in &times[1..] {
        if t - prev_t > gap_threshold_ms {
            runs.push(RunInfo {
                index: runs.len(),
                start_ms: run_start,
                end_ms: prev_t,
                duration_ms: prev_t - run_start,
            });
            run_start = t;
        }
        prev_t = t;
    }

    runs.push(RunInfo {
        index: runs.len(),
        start_ms: run_start,
        end_ms: prev_t,
        duration_ms: prev_t - run_start,
    });

    runs
}

// ── Activity heatmap ────────────────────────────────────────────────

pub fn activity_heatmap(
    profile: &Profile,
    start_ms: f64,
    end_ms: f64,
    bin_ms: f64,
) -> Vec<HeatmapBin> {
    let n_bins = ((end_ms - start_ms) / bin_ms) as usize + 1;
    let total_cpus = profile.cpu_count();
    let mut bins = vec![0usize; n_bins];

    for &ci in &profile.cpu_thread_indices {
        let times = &profile.threads[ci].sample_times;
        for &t in times {
            if t >= start_ms && t <= end_ms {
                let b = ((t - start_ms) / bin_ms) as usize;
                if b < n_bins {
                    bins[b] = (bins[b] + 1).min(total_cpus);
                }
            }
        }
    }

    bins.iter()
        .enumerate()
        .map(|(i, &count)| HeatmapBin {
            bin_start_ms: start_ms + i as f64 * bin_ms,
            active_cpus: count,
            total_cpus,
        })
        .collect()
}

// ── Gap detection ───────────────────────────────────────────────────

pub fn find_gaps(
    profile: &Profile,
    start_ms: f64,
    end_ms: f64,
    bin_ms: f64,
    idle_threshold: f64,
    min_gap_ms: f64,
) -> Vec<Gap> {
    let heatmap = activity_heatmap(profile, start_ms, end_ms, bin_ms);
    if heatmap.is_empty() {
        return vec![];
    }

    let mut gaps = Vec::new();
    let mut in_gap = false;
    let mut gap_start = 0.0;

    for bin in &heatmap {
        let ratio = bin.active_cpus as f64 / bin.total_cpus.max(1) as f64;
        if ratio < idle_threshold {
            if !in_gap {
                in_gap = true;
                gap_start = bin.bin_start_ms;
            }
        } else if in_gap {
            let gap_end = bin.bin_start_ms;
            let duration = gap_end - gap_start;
            if duration >= min_gap_ms {
                let (before, after) = gap_context(profile, gap_start, gap_end);
                gaps.push(Gap {
                    start_ms: gap_start,
                    end_ms: gap_end,
                    duration_ms: duration,
                    before_func: before,
                    after_func: after,
                    activity_ratio: 0.0,
                });
            }
            in_gap = false;
        }
    }

    // Close trailing gap
    if in_gap {
        let duration = end_ms - gap_start;
        if duration >= min_gap_ms {
            let (before, after) = gap_context(profile, gap_start, end_ms);
            gaps.push(Gap {
                start_ms: gap_start,
                end_ms,
                duration_ms: duration,
                before_func: before,
                after_func: after,
                activity_ratio: 0.0,
            });
        }
    }

    gaps
}

fn gap_context(profile: &Profile, gap_start: f64, gap_end: f64) -> (String, String) {
    let main = match profile.main_thread() {
        Some(t) => t,
        None => return ("?".into(), "?".into()),
    };

    let times = &main.sample_times;
    let stacks = &main.sample_stacks;
    let mut before = "?".to_string();
    let mut after = "?".to_string();

    for (i, &t) in times.iter().enumerate() {
        if t <= gap_start {
            before = main.top_func(stacks.get(i).copied().flatten());
        } else if t >= gap_end {
            after = main.top_func(stacks.get(i).copied().flatten());
            break;
        }
    }

    (before, after)
}

// ── Thread sample-stream gaps ────────────────────────────────────────

pub fn thread_sample_gaps(
    profile: &Profile,
    thread_filter: Option<&str>,
    start_ms: f64,
    end_ms: f64,
    min_gap_ms: f64,
) -> Vec<Gap> {
    let indices = match thread_filter {
        Some(f) => profile.resolve_thread_indices(Some(f)),
        None => profile
            .main_thread_index
            .map(|i| vec![i])
            .unwrap_or_default(),
    };

    let mut gaps = Vec::new();
    for &ti in &indices {
        let t = &profile.threads[ti];
        let times = &t.sample_times;
        let stacks = &t.sample_stacks;
        let mut prev_t: Option<f64> = None;
        let mut prev_i: usize = 0;

        for (i, &time) in times.iter().enumerate() {
            if time >= start_ms && time <= end_ms {
                if let Some(pt) = prev_t {
                    let delta = time - pt;
                    if delta >= min_gap_ms {
                        let before = t.top_func(stacks.get(prev_i).copied().flatten());
                        let after = t.top_func(stacks.get(i).copied().flatten());
                        gaps.push(Gap {
                            start_ms: pt,
                            end_ms: time,
                            duration_ms: delta,
                            before_func: before,
                            after_func: after,
                            activity_ratio: 0.0,
                        });
                    }
                }
                prev_t = Some(time);
                prev_i = i;
            }
        }
    }

    gaps.sort_by(|a, b| a.start_ms.partial_cmp(&b.start_ms).unwrap());
    gaps
}

// ── Phase identification ────────────────────────────────────────────

pub fn identify_phases(profile: &Profile, start_ms: f64, end_ms: f64, bin_ms: f64) -> Vec<Phase> {
    let heatmap = activity_heatmap(profile, start_ms, end_ms, bin_ms);
    if heatmap.is_empty() {
        return vec![];
    }

    let mut phases: Vec<Phase> = Vec::new();
    let mut current_type: Option<PhaseType> = None;
    let mut phase_start = start_ms;
    let mut cpu_sum = 0.0f64;
    let mut bin_count = 0usize;

    for bin in &heatmap {
        let ratio = bin.active_cpus as f64 / bin.total_cpus.max(1) as f64;
        let pt = if ratio < 0.1 {
            PhaseType::Idle
        } else if ratio < 0.3 {
            PhaseType::Sequential
        } else {
            PhaseType::Parallel
        };

        if let Some(ct) = current_type {
            if pt != ct {
                if bin_count > 0 {
                    phases.push(Phase {
                        start_ms: phase_start,
                        end_ms: bin.bin_start_ms,
                        duration_ms: bin.bin_start_ms - phase_start,
                        avg_cpus: cpu_sum / bin_count as f64,
                        phase_type: ct,
                    });
                }
                phase_start = bin.bin_start_ms;
                cpu_sum = 0.0;
                bin_count = 0;
            }
        }

        current_type = Some(pt);
        cpu_sum += bin.active_cpus as f64;
        bin_count += 1;
    }

    // Close last phase
    if let Some(ct) = current_type {
        if bin_count > 0 {
            phases.push(Phase {
                start_ms: phase_start,
                end_ms,
                duration_ms: end_ms - phase_start,
                avg_cpus: cpu_sum / bin_count as f64,
                phase_type: ct,
            });
        }
    }

    // Merge short phases (< 5ms) into neighbors
    let mut merged: Vec<Phase> = Vec::new();
    for p in phases {
        if let Some(last) = merged.last_mut() {
            if p.duration_ms < 5.0 {
                last.end_ms = p.end_ms;
                last.duration_ms = last.end_ms - last.start_ms;
                continue;
            }
        }
        merged.push(p);
    }

    merged
}

// ── Thread timeline (generalized) ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ThreadTimelineEntry {
    pub thread_name: String,
    pub time_ms: f64,
    pub frames: Vec<String>,
}

pub fn thread_timeline(
    profile: &Profile,
    thread_filter: Option<&str>,
    start_ms: f64,
    end_ms: f64,
    sample_every: usize,
    stack_depth: usize,
) -> Vec<ThreadTimelineEntry> {
    let indices = profile.resolve_thread_indices(thread_filter);
    let mut result = Vec::new();

    for &ti in &indices {
        let t = &profile.threads[ti];
        let times = &t.sample_times;
        let stacks = &t.sample_stacks;

        for (i, &time) in times.iter().enumerate() {
            if time >= start_ms && time <= end_ms && i % sample_every.max(1) == 0 {
                let stack_idx = stacks.get(i).copied().flatten();
                let frames = t.walk_stack(stack_idx, stack_depth);
                result.push(ThreadTimelineEntry {
                    thread_name: t.name.clone(),
                    time_ms: time,
                    frames,
                });
            }
        }
    }

    result.sort_by(|a, b| a.time_ms.partial_cmp(&b.time_ms).unwrap());
    result
}

// ── Top functions (self time by sample count) ───────────────────────

pub fn top_functions(
    profile: &Profile,
    thread_name: Option<&str>,
    start_ms: f64,
    end_ms: f64,
    limit: usize,
) -> Vec<FunctionSample> {
    let thread_indices: Vec<usize> = match thread_name {
        Some(name) => profile
            .threads
            .iter()
            .enumerate()
            .filter(|(_, t)| t.name.contains(name))
            .map(|(i, _)| i)
            .collect(),
        None => profile.process_thread_indices.clone(),
    };

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut total = 0usize;

    for &ti in &thread_indices {
        let t = &profile.threads[ti];
        for (i, &time) in t.sample_times.iter().enumerate() {
            if time >= start_ms && time <= end_ms {
                total += 1;
                // Leaf frame = self time
                let name = t.top_func(t.sample_stacks.get(i).copied().flatten());
                *counts.entry(name).or_default() += 1;
            }
        }
    }

    let mut funcs: Vec<FunctionSample> = counts
        .into_iter()
        .map(|(name, count)| FunctionSample {
            name,
            count,
            percentage: if total > 0 {
                count as f64 / total as f64 * 100.0
            } else {
                0.0
            },
        })
        .collect();

    funcs.sort_by(|a, b| b.count.cmp(&a.count));
    funcs.truncate(limit);
    funcs
}

// ── Per-thread activity summary ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ThreadActivity {
    pub name: String,
    pub tid: String,
    pub sample_count: usize,
    pub active_ms: f64,
    pub first_ms: f64,
    pub last_ms: f64,
}

pub fn thread_activity(profile: &Profile, start_ms: f64, end_ms: f64) -> Vec<ThreadActivity> {
    let mut result = Vec::new();

    for &ti in &profile.process_thread_indices {
        let t = &profile.threads[ti];
        let samples_in_range: Vec<f64> = t
            .sample_times
            .iter()
            .copied()
            .filter(|&time| time >= start_ms && time <= end_ms)
            .collect();

        if samples_in_range.is_empty() {
            continue;
        }

        let first = samples_in_range[0];
        let last = *samples_in_range.last().unwrap();

        // Estimate active time from sample intervals
        // (sample rate ≈ 1/16000s = 0.0625ms per sample at 16kHz)
        let sample_interval_ms = 1.0 / 16000.0 * 1000.0;
        let active_ms = samples_in_range.len() as f64 * sample_interval_ms;

        result.push(ThreadActivity {
            name: t.name.clone(),
            tid: t.tid.clone(),
            sample_count: samples_in_range.len(),
            active_ms,
            first_ms: first,
            last_ms: last,
        });
    }

    result.sort_by(|a, b| b.sample_count.cmp(&a.sample_count));
    result
}

// ── Callers/callees (butterfly view) ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct CallerCalleeEntry {
    pub name: String,
    pub count: usize,
    pub percentage: f64,
}

#[derive(Debug, Clone)]
pub struct CallerCalleeResult {
    pub function: String,
    pub self_count: usize,
    pub total_count: usize,
    pub callers: Vec<CallerCalleeEntry>,
    pub callees: Vec<CallerCalleeEntry>,
}

pub fn callers_callees(
    profile: &Profile,
    function: &str,
    thread_filter: Option<&str>,
    start_ms: f64,
    end_ms: f64,
) -> CallerCalleeResult {
    let indices = profile.resolve_thread_indices(thread_filter);
    let mut callers: HashMap<String, usize> = HashMap::new();
    let mut callees: HashMap<String, usize> = HashMap::new();
    let mut self_count = 0usize;
    let mut total_count = 0usize;

    for &ti in &indices {
        let t = &profile.threads[ti];
        for (i, &time) in t.sample_times.iter().enumerate() {
            if time < start_ms || time > end_ms {
                continue;
            }
            let stack_idx = t.sample_stacks.get(i).copied().flatten();
            // Walk full stack (leaf to root), resolve names
            let names = t.walk_stack(stack_idx, 256);
            // names[0] = leaf, names[last] = root
            for (pos, name) in names.iter().enumerate() {
                if !name.contains(function) {
                    continue;
                }
                total_count += 1;
                if pos == 0 {
                    self_count += 1;
                }
                // Caller is deeper in stack (pos+1, toward root)
                if let Some(caller) = names.get(pos + 1) {
                    *callers.entry(caller.clone()).or_default() += 1;
                }
                // Callee is shallower (pos-1, toward leaf)
                if pos > 0 {
                    if let Some(callee) = names.get(pos - 1) {
                        *callees.entry(callee.clone()).or_default() += 1;
                    }
                }
                break; // Only count first occurrence per stack
            }
        }
    }

    let make_entries = |map: HashMap<String, usize>| -> Vec<CallerCalleeEntry> {
        let total = total_count.max(1) as f64;
        let mut v: Vec<CallerCalleeEntry> = map
            .into_iter()
            .map(|(name, count)| CallerCalleeEntry {
                name,
                count,
                percentage: count as f64 / total * 100.0,
            })
            .collect();
        v.sort_by(|a, b| b.count.cmp(&a.count));
        v
    };

    CallerCalleeResult {
        function: function.to_string(),
        self_count,
        total_count,
        callers: make_entries(callers),
        callees: make_entries(callees),
    }
}

// ── Search stacks ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StackMatch {
    pub thread_name: String,
    pub time_ms: f64,
    pub frames: Vec<String>,
}

pub fn search_stacks(
    profile: &Profile,
    pattern: &str,
    thread_filter: Option<&str>,
    start_ms: f64,
    end_ms: f64,
    max_results: usize,
) -> Vec<StackMatch> {
    let indices = profile.resolve_thread_indices(thread_filter);
    let mut results = Vec::new();

    for &ti in &indices {
        let t = &profile.threads[ti];
        for (i, &time) in t.sample_times.iter().enumerate() {
            if time < start_ms || time > end_ms {
                continue;
            }
            let stack_idx = t.sample_stacks.get(i).copied().flatten();
            let frames = t.walk_stack(stack_idx, 64);
            if frames.iter().any(|f| f.contains(pattern)) {
                results.push(StackMatch {
                    thread_name: t.name.clone(),
                    time_ms: time,
                    frames,
                });
                if results.len() >= max_results {
                    return results;
                }
            }
        }
    }

    results
}

// ── Sample-at (thread snapshot) ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ThreadSnapshot {
    pub thread_name: String,
    pub sample_time_ms: f64,
    pub frames: Vec<String>,
}

pub fn sample_at(
    profile: &Profile,
    time_ms: f64,
    tolerance_ms: f64,
    stack_depth: usize,
) -> Vec<ThreadSnapshot> {
    let mut results = Vec::new();

    for &ti in &profile.process_thread_indices {
        let t = &profile.threads[ti];
        // Binary search for closest sample
        let times = &t.sample_times;
        let pos = times.partition_point(|&x| x < time_ms);

        let closest = [pos.checked_sub(1), Some(pos)]
            .into_iter()
            .flatten()
            .filter(|&i| i < times.len())
            .min_by(|&a, &b| {
                let da = (times[a] - time_ms).abs();
                let db = (times[b] - time_ms).abs();
                da.partial_cmp(&db).unwrap()
            });

        if let Some(ci) = closest {
            if (times[ci] - time_ms).abs() <= tolerance_ms {
                let stack_idx = t.sample_stacks.get(ci).copied().flatten();
                let frames = t.walk_stack(stack_idx, stack_depth);
                results.push(ThreadSnapshot {
                    thread_name: t.name.clone(),
                    sample_time_ms: times[ci],
                    frames,
                });
            }
        }
    }

    results.sort_by(|a, b| b.frames.len().cmp(&a.frames.len()));
    results
}

// ── Call tree (text flamegraph) ──────────────────────────────────────

#[derive(Debug)]
struct CallTreeNode {
    self_count: usize,
    total_count: usize,
    children: HashMap<String, CallTreeNode>,
}

impl CallTreeNode {
    fn new() -> Self {
        Self {
            self_count: 0,
            total_count: 0,
            children: HashMap::new(),
        }
    }

    fn insert(&mut self, frames: &[String]) {
        self.total_count += 1;
        if frames.is_empty() {
            self.self_count += 1;
            return;
        }
        let child = self
            .children
            .entry(frames[0].clone())
            .or_insert_with(CallTreeNode::new);
        child.insert(&frames[1..]);
    }

    fn render(
        &self,
        name: &str,
        total_samples: usize,
        indent: usize,
        min_pct: f64,
        out: &mut String,
    ) {
        let pct = self.total_count as f64 / total_samples.max(1) as f64 * 100.0;
        if pct < min_pct {
            return;
        }
        let self_pct = self.self_count as f64 / total_samples.max(1) as f64 * 100.0;
        let prefix = "  ".repeat(indent);
        if self.self_count > 0 {
            out.push_str(&format!(
                "{}{:5.1}% ({:>5})  {}  [self: {:.1}%]\n",
                prefix, pct, self.total_count, name, self_pct
            ));
        } else {
            out.push_str(&format!(
                "{}{:5.1}% ({:>5})  {}\n",
                prefix, pct, self.total_count, name
            ));
        }

        // Sort children by total_count descending
        let mut children: Vec<(&String, &CallTreeNode)> = self.children.iter().collect();
        children.sort_by(|a, b| b.1.total_count.cmp(&a.1.total_count));
        for (child_name, child_node) in children {
            child_node.render(child_name, total_samples, indent + 1, min_pct, out);
        }
    }
}

pub fn call_tree(
    profile: &Profile,
    thread_filter: Option<&str>,
    start_ms: f64,
    end_ms: f64,
    max_depth: usize,
    min_pct: f64,
) -> String {
    let indices = profile.resolve_thread_indices(thread_filter);
    let mut root = CallTreeNode::new();
    let mut total = 0usize;

    for &ti in &indices {
        let t = &profile.threads[ti];
        for (i, &time) in t.sample_times.iter().enumerate() {
            if time < start_ms || time > end_ms {
                continue;
            }
            total += 1;
            let stack_idx = t.sample_stacks.get(i).copied().flatten();
            // Walk leaf→root, then reverse to get root→leaf
            let mut frames = t.walk_stack(stack_idx, max_depth);
            frames.reverse();
            root.insert(&frames);
        }
    }

    let mut out = String::new();
    // Render children of root (root itself is just the container)
    let mut children: Vec<(&String, &CallTreeNode)> = root.children.iter().collect();
    children.sort_by(|a, b| b.1.total_count.cmp(&a.1.total_count));
    for (name, node) in children {
        node.render(name, total, 0, min_pct, &mut out);
    }
    out
}
