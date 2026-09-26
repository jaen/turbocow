use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ── Firefox Profiler JSON schema (subset) ───────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawProfile {
    pub threads: Vec<RawThread>,
    #[serde(default)]
    pub libs: Vec<RawLib>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawLib {
    #[serde(default)]
    pub debug_name: String,
    #[serde(default)]
    pub code_id: Option<String>,
    #[serde(default)]
    pub breakpad_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawThread {
    pub name: String,
    pub tid: serde_json::Value,
    #[serde(default)]
    pub is_main_thread: bool,
    #[serde(default)]
    pub samples: Option<RawSamples>,
    #[serde(default)]
    pub stack_table: Option<RawStackTable>,
    #[serde(default)]
    pub frame_table: Option<RawFrameTable>,
    #[serde(default)]
    pub func_table: Option<RawFuncTable>,
    #[serde(default, rename = "stringArray")]
    pub string_array: Vec<String>,
    #[serde(default)]
    pub resource_table: Option<RawResourceTable>,
}

#[derive(Debug, Deserialize)]
pub struct RawResourceTable {
    #[serde(default)]
    pub lib: Vec<Option<usize>>,
}

#[derive(Debug, Deserialize)]
pub struct RawSamples {
    #[serde(default)]
    pub time: Vec<f64>,
    #[serde(default, rename = "timeDeltas")]
    pub time_deltas: Vec<f64>,
    #[serde(default)]
    pub stack: Vec<Option<usize>>,
}

#[derive(Debug, Deserialize)]
pub struct RawStackTable {
    #[serde(default)]
    pub frame: Vec<usize>,
    #[serde(default)]
    pub prefix: Vec<Option<usize>>,
}

#[derive(Debug, Deserialize)]
pub struct RawFrameTable {
    #[serde(default)]
    pub func: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RawFuncTable {
    #[serde(default)]
    pub name: Vec<usize>,
    #[serde(default)]
    pub resource: Vec<i64>,
}

// ── Syms.json types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SymsFile {
    pub string_table: Vec<String>,
    pub data: Vec<SymsLibData>,
}

#[derive(Debug, Deserialize)]
pub struct SymsLibData {
    pub debug_name: String,
    #[serde(default)]
    pub debug_id: String,
    #[serde(default)]
    pub code_id: String,
    pub symbol_table: Vec<SymbolEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SymbolEntry {
    pub rva: u64,
    pub size: u64,
    pub symbol: usize,
}

// ── Processed profile ───────────────────────────────────────────────

#[derive(Debug)]
pub struct Profile {
    pub threads: Vec<Thread>,
    pub cpu_thread_indices: Vec<usize>,
    pub process_thread_indices: Vec<usize>,
    pub main_thread_index: Option<usize>,
    pub syms_loaded: bool,
}

#[derive(Debug)]
pub struct Thread {
    pub name: String,
    pub tid: String,
    pub is_main: bool,
    pub sample_times: Vec<f64>,
    pub sample_stacks: Vec<Option<usize>>,
    pub stack_frames: Vec<usize>,
    pub stack_prefixes: Vec<Option<usize>>,
    pub frame_funcs: Vec<usize>,
    pub func_names: Vec<usize>,
    pub strings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub name: String,
    pub tid: String,
    pub is_main: bool,
    pub sample_count: usize,
    pub first_sample_ms: Option<f64>,
    pub last_sample_ms: Option<f64>,
}

impl Profile {
    pub fn load(path: &Path, syms_path: Option<&Path>) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        let reader = BufReader::new(file);

        let raw: RawProfile = if path
            .to_str()
            .is_some_and(|s| s.ends_with(".gz") || s.ends_with(".json.gz"))
        {
            let decoder = flate2::read::GzDecoder::new(reader);
            serde_json::from_reader(decoder)
        } else {
            serde_json::from_reader(reader)
        }
        .map_err(|e| format!("JSON parse error: {e}"))?;

        // Load syms.json (explicit path > auto-detected companion file)
        let auto_syms = companion_syms_path(path);
        let effective_syms = syms_path.or(auto_syms.as_deref());
        let syms = effective_syms.and_then(|p| load_syms(p).ok());
        let syms_lookup = syms.as_ref().map(|s| build_syms_lookup(s));

        let mut threads = Vec::with_capacity(raw.threads.len());
        let mut cpu_indices = Vec::new();
        let mut process_indices = Vec::new();

        for (i, rt) in raw.threads.into_iter().enumerate() {
            let tid = match &rt.tid {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };

            let (times, stacks) = match rt.samples {
                Some(s) => {
                    let times = if !s.time.is_empty() {
                        s.time
                    } else if !s.time_deltas.is_empty() {
                        let mut acc = 0.0;
                        s.time_deltas
                            .iter()
                            .map(|d| {
                                acc += d;
                                acc
                            })
                            .collect()
                    } else {
                        vec![]
                    };
                    (times, s.stack)
                }
                None => (vec![], vec![]),
            };

            let (stack_frames, stack_prefixes) = match rt.stack_table {
                Some(st) => (st.frame, st.prefix),
                None => (vec![], vec![]),
            };

            let (func_names, func_resources) = match rt.func_table {
                Some(ft) => (ft.name, ft.resource),
                None => (vec![], vec![]),
            };
            let resource_libs = rt.resource_table.map(|rt| rt.lib).unwrap_or_default();
            let frame_funcs = rt.frame_table.map(|ft| ft.func).unwrap_or_default();

            let mut strings = rt.string_array;

            // Apply symbolication from syms.json
            if let (Some(syms), Some(lookup)) = (&syms, &syms_lookup) {
                symbolicate_strings(
                    &mut strings,
                    &func_names,
                    &func_resources,
                    &resource_libs,
                    &raw.libs,
                    syms,
                    lookup,
                );
            }

            let is_cpu = rt.name.starts_with("CPU");
            if is_cpu {
                cpu_indices.push(i);
            } else {
                process_indices.push(i);
            }

            threads.push(Thread {
                name: rt.name,
                tid,
                is_main: rt.is_main_thread,
                sample_times: times,
                sample_stacks: stacks,
                stack_frames,
                stack_prefixes,
                frame_funcs,
                func_names,
                strings,
            });
        }

        // Find main thread: isMainThread=true with most samples
        let main_idx = process_indices
            .iter()
            .copied()
            .filter(|&i| threads[i].is_main && threads[i].sample_times.len() > 100)
            .max_by_key(|&i| threads[i].sample_times.len());

        Ok(Profile {
            threads,
            cpu_thread_indices: cpu_indices,
            process_thread_indices: process_indices,
            main_thread_index: main_idx,
            syms_loaded: syms.is_some(),
        })
    }

    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    pub fn cpu_count(&self) -> usize {
        self.cpu_thread_indices.len()
    }

    pub fn process_count(&self) -> usize {
        self.process_thread_indices.len()
    }

    pub fn main_thread(&self) -> Option<&Thread> {
        self.main_thread_index.map(|i| &self.threads[i])
    }

    pub fn resolve_thread_indices(&self, filter: Option<&str>) -> Vec<usize> {
        match filter {
            Some(name) => self
                .process_thread_indices
                .iter()
                .copied()
                .filter(|&i| self.threads[i].name.contains(name))
                .collect(),
            None => self.process_thread_indices.clone(),
        }
    }

    pub fn list_threads(&self) -> Vec<ThreadInfo> {
        let mut infos: Vec<ThreadInfo> = self
            .process_thread_indices
            .iter()
            .map(|&i| {
                let t = &self.threads[i];
                ThreadInfo {
                    name: t.name.clone(),
                    tid: t.tid.clone(),
                    is_main: t.is_main,
                    sample_count: t.sample_times.len(),
                    first_sample_ms: t.sample_times.first().copied(),
                    last_sample_ms: t.sample_times.last().copied(),
                }
            })
            .collect();
        infos.sort_by(|a, b| b.sample_count.cmp(&a.sample_count));
        infos
    }
}

impl Thread {
    /// Walk the stack from leaf to root, returning function name string
    /// indices.
    pub fn walk_stack_indices(&self, stack_idx: Option<usize>, max_depth: usize) -> Vec<usize> {
        let mut result = Vec::with_capacity(max_depth);
        let mut idx = stack_idx;
        while let Some(si) = idx {
            if result.len() >= max_depth {
                break;
            }
            if si >= self.stack_frames.len() {
                break;
            }
            let frame_idx = self.stack_frames[si];
            if frame_idx < self.frame_funcs.len() {
                let func_idx = self.frame_funcs[frame_idx];
                if func_idx < self.func_names.len() {
                    result.push(self.func_names[func_idx]);
                }
            }
            idx = if si < self.stack_prefixes.len() {
                self.stack_prefixes[si]
            } else {
                None
            };
        }
        result
    }

    /// Resolve a string index to the actual string, with Rust symbol
    /// shortening.
    pub fn resolve_name(&self, str_idx: usize) -> &str {
        self.strings
            .get(str_idx)
            .map(|s| s.as_str())
            .unwrap_or("[unknown]")
    }

    /// Walk the stack and return resolved, shortened function names.
    pub fn walk_stack(&self, stack_idx: Option<usize>, max_depth: usize) -> Vec<String> {
        self.walk_stack_indices(stack_idx, max_depth)
            .into_iter()
            .map(|si| shorten_rust_name(self.resolve_name(si)))
            .collect()
    }

    /// Get the leaf (top-of-stack) function name.
    pub fn top_func(&self, stack_idx: Option<usize>) -> String {
        let frames = self.walk_stack(stack_idx, 1);
        frames
            .into_iter()
            .next()
            .unwrap_or_else(|| "[no stack]".to_string())
    }
}

/// Shorten a Rust function name by stripping generics and keeping last 3
/// components.
pub fn shorten_rust_name(name: &str) -> String {
    if name.starts_with("0x") {
        return name.to_string();
    }
    let parts: Vec<&str> = name.split("::").collect();
    let mut short_parts = Vec::new();
    for p in &parts {
        let mut depth = 0i32;
        let mut cleaned = String::new();
        for ch in p.chars() {
            match ch {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ if depth == 0 => cleaned.push(ch),
                _ => {}
            }
        }
        let trimmed = cleaned.trim();
        if !trimmed.is_empty() {
            short_parts.push(trimmed.to_string());
        }
    }
    if short_parts.is_empty() {
        return name.chars().take(60).collect();
    }
    let n = short_parts.len();
    let start = n.saturating_sub(3);
    short_parts[start..].join("::")
}

// ── Syms.json loading & symbolication ──────────────────────────────

fn companion_syms_path(profile_path: &Path) -> Option<PathBuf> {
    let s = profile_path.to_str()?;
    // profile.json.gz → profile.json.syms.json
    let base = s.strip_suffix(".gz").unwrap_or(s);
    let syms_path = PathBuf::from(format!("{base}.syms.json"));
    if syms_path.exists() {
        Some(syms_path)
    } else {
        None
    }
}

fn load_syms(path: &Path) -> Result<SymsFile, String> {
    let file = File::open(path).map_err(|e| format!("cannot open syms {}: {e}", path.display()))?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).map_err(|e| format!("syms JSON parse error: {e}"))
}

fn build_syms_lookup(syms: &SymsFile) -> HashMap<String, usize> {
    syms.data
        .iter()
        .enumerate()
        .map(|(i, entry)| (entry.debug_name.clone(), i))
        .collect()
}

fn symbolicate_strings(
    strings: &mut [String],
    func_names: &[usize],
    func_resources: &[i64],
    resource_libs: &[Option<usize>],
    libs: &[RawLib],
    syms: &SymsFile,
    syms_lookup: &HashMap<String, usize>,
) {
    for (&name_idx, &resource_idx) in func_names.iter().zip(func_resources.iter()) {
        // For functions with no known resource, try all libraries
        if resource_idx < 0 {
            let name = match strings.get(name_idx) {
                Some(n) => n,
                None => continue,
            };
            let rva = if let Some(hex) = name.strip_prefix("fun_") {
                u64::from_str_radix(hex, 16).ok()
            } else if let Some(hex) = name.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).ok()
            } else {
                None
            };
            if let Some(rva) = rva {
                // Brute-force: search all syms libraries
                for sl in &syms.data {
                    let pos = sl.symbol_table.partition_point(|s| s.rva <= rva);
                    if pos == 0 {
                        continue;
                    }
                    let entry = &sl.symbol_table[pos - 1];
                    if rva < entry.rva + entry.size {
                        if let Some(resolved) = syms.string_table.get(entry.symbol) {
                            if !resolved.starts_with("fun_") && resolved != "UNKNOWN" {
                                strings[name_idx] = resolved.clone();
                                break;
                            }
                        }
                    }
                }
            }
            continue;
        }
        let resource_idx = resource_idx as usize;
        let lib_idx = match resource_libs.get(resource_idx).and_then(|v| *v) {
            Some(idx) => idx,
            None => continue,
        };
        let lib = match libs.get(lib_idx) {
            Some(l) => l,
            None => continue,
        };

        let name = match strings.get(name_idx) {
            Some(n) => n,
            None => continue,
        };
        // Resolve samply stub names (fun_XXXX) and raw hex addresses (0xXXXX)
        let rva = if let Some(hex) = name.strip_prefix("fun_") {
            u64::from_str_radix(hex, 16).ok()
        } else if let Some(hex) = name.strip_prefix("0x") {
            u64::from_str_radix(hex, 16).ok()
        } else {
            None
        };
        let rva = match rva {
            Some(v) => v,
            None => continue,
        };

        // Find syms entry for this library
        let syms_idx = match syms_lookup.get(&lib.debug_name) {
            Some(&idx) => idx,
            None => continue,
        };
        let sl = &syms.data[syms_idx];

        // Binary search: find last entry with rva <= target
        let pos = sl.symbol_table.partition_point(|s| s.rva <= rva);
        if pos == 0 {
            continue;
        }
        let entry = &sl.symbol_table[pos - 1];
        if rva >= entry.rva + entry.size {
            continue;
        }
        if let Some(resolved) = syms.string_table.get(entry.symbol) {
            if !resolved.starts_with("fun_") && resolved != "UNKNOWN" {
                strings[name_idx] = resolved.clone();
            }
        }
    }
}
