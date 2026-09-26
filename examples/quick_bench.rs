//! Lightweight perf sanity check: turbocow vs ecow (+ std reference) on the
//! core ops. NOT a rigorous benchmark — a quick ns/op read to spot gross
//! regressions before running the full criterion suite.
//!
//! Run: `cargo run --profile bench --example quick_bench`
//! (uses dev-dependency `ecow`; reports min-of-rounds ns/op + turbocow/ecow ratio)

use std::hint::black_box;
use std::time::Instant;

/// Time `f`: warm up, auto-scale iteration count to ~5ms/round, return the
/// minimum ns/op across rounds (min is the most stable estimator for fast ops).
fn bench(mut f: impl FnMut()) -> f64 {
    for _ in 0..200 {
        f();
    }
    let mut iters: u64 = 64;
    loop {
        let t = Instant::now();
        for _ in 0..iters {
            f();
        }
        if t.elapsed().as_secs_f64() > 0.005 || iters >= (1 << 30) {
            break;
        }
        iters = iters.saturating_mul(2);
    }
    let mut best = f64::MAX;
    for _ in 0..9 {
        let t = Instant::now();
        for _ in 0..iters {
            f();
        }
        let ns = t.elapsed().as_nanos() as f64 / iters as f64;
        if ns < best {
            best = ns;
        }
    }
    best
}

fn row3(name: &str, tc: f64, ec: f64, std: f64) {
    let ratio = tc / ec;
    let flag = if ratio <= 1.05 {
        "" // within noise / faster
    } else if ratio <= 1.25 {
        "  ~"
    } else {
        "  <-- slower"
    };
    println!(
        "{name:<22} {tc:>9.2} {ec:>9.2} {std:>9.2}   {ratio:>5.2}x{flag}",
        name = name,
        tc = tc,
        ec = ec,
        std = std,
        ratio = ratio
    );
}

fn row2(name: &str, tc: f64, ec: f64) {
    let ratio = tc / ec;
    let flag = if ratio <= 1.05 {
        ""
    } else if ratio <= 1.25 {
        "  ~"
    } else {
        "  <-- slower"
    };
    println!(
        "{name:<22} {tc:>9.2} {ec:>9.2} {dash:>9}   {ratio:>5.2}x{flag}",
        name = name,
        tc = tc,
        ec = ec,
        dash = "-",
        ratio = ratio
    );
}

fn main() {
    let long = "the quick brown fox jumps over the lazy dog, twice over now."; // 59 bytes (heap)
    let short = "hello"; // inline
    let data: Vec<u64> = (0..256).collect();
    let slice16 = &data[..16];

    println!("turbocow quick perf check — min ns/op (lower is better)\n");
    println!(
        "{:<22} {:>9} {:>9} {:>9}   {:>6}",
        "op", "turbocow", "ecow", "std", "tc/ec"
    );
    println!("{}", "-".repeat(64));

    // ── Strings ───────────────────────────────────────────────────────────
    {
        use ecow::EcoString as EcEs;
        use turbocow::EcoString as TcEs;

        row3(
            "String::from inline",
            bench(|| {
                black_box(TcEs::from(black_box(short)));
            }),
            bench(|| {
                black_box(EcEs::from(black_box(short)));
            }),
            bench(|| {
                black_box(String::from(black_box(short)));
            }),
        );
        row3(
            "String::from heap",
            bench(|| {
                black_box(TcEs::from(black_box(long)));
            }),
            bench(|| {
                black_box(EcEs::from(black_box(long)));
            }),
            bench(|| {
                black_box(String::from(black_box(long)));
            }),
        );

        let tc_in = TcEs::from(short);
        let ec_in = EcEs::from(short);
        let std_in = String::from(short);
        row3(
            "String::clone inline",
            bench(|| {
                black_box(black_box(&tc_in).clone());
            }),
            bench(|| {
                black_box(black_box(&ec_in).clone());
            }),
            bench(|| {
                black_box(black_box(&std_in).clone());
            }),
        );

        let tc_h = TcEs::from(long);
        let ec_h = EcEs::from(long);
        let std_h = String::from(long);
        row3(
            "String::clone heap",
            bench(|| {
                black_box(black_box(&tc_h).clone());
            }),
            bench(|| {
                black_box(black_box(&ec_h).clone());
            }),
            bench(|| {
                black_box(black_box(&std_h).clone());
            }),
        );

        row3(
            "String::push_str x8",
            bench(|| {
                let mut s = TcEs::new();
                for _ in 0..8 {
                    s.push_str(black_box("abcd"));
                }
                black_box(&s);
            }),
            bench(|| {
                let mut s = EcEs::new();
                for _ in 0..8 {
                    s.push_str(black_box("abcd"));
                }
                black_box(&s);
            }),
            bench(|| {
                let mut s = String::new();
                for _ in 0..8 {
                    s.push_str(black_box("abcd"));
                }
                black_box(&s);
            }),
        );

        let (tc_a, tc_b) = (TcEs::from(long), TcEs::from(long));
        let (ec_a, ec_b) = (EcEs::from(long), EcEs::from(long));
        let (std_a, std_b) = (String::from(long), String::from(long));
        row3(
            "String::eq (distinct)",
            bench(|| {
                black_box(black_box(&tc_a) == black_box(&tc_b));
            }),
            bench(|| {
                black_box(black_box(&ec_a) == black_box(&ec_b));
            }),
            bench(|| {
                black_box(black_box(&std_a) == black_box(&std_b));
            }),
        );
    }

    println!();

    // ── Vectors ───────────────────────────────────────────────────────────
    {
        use ecow::EcoVec as EcEv;
        use turbocow::EcoVec as TcEv;

        row3(
            "Vec::from &[u64;16]",
            bench(|| {
                black_box(TcEv::from(black_box(slice16)));
            }),
            bench(|| {
                black_box(EcEv::from(black_box(slice16)));
            }),
            bench(|| {
                black_box(Vec::from(black_box(slice16)));
            }),
        );

        row3(
            "Vec::push 64",
            bench(|| {
                let mut v = TcEv::new();
                for i in 0..64u64 {
                    v.push(black_box(i));
                }
                black_box(&v);
            }),
            bench(|| {
                let mut v = EcEv::new();
                for i in 0..64u64 {
                    v.push(black_box(i));
                }
                black_box(&v);
            }),
            bench(|| {
                let mut v = Vec::new();
                for i in 0..64u64 {
                    v.push(black_box(i));
                }
                black_box(&v);
            }),
        );

        let tc_v = TcEv::from(&data[..]);
        let ec_v = EcEv::from(&data[..]);
        let std_v = data.clone();
        row3(
            "Vec::clone (shared)",
            bench(|| {
                black_box(black_box(&tc_v).clone());
            }),
            bench(|| {
                black_box(black_box(&ec_v).clone());
            }),
            bench(|| {
                black_box(black_box(&std_v).clone());
            }),
        );

        row3(
            "Vec::iter sum 256",
            bench(|| {
                let mut s = 0u64;
                for &x in black_box(&tc_v).iter() {
                    s = s.wrapping_add(x);
                }
                black_box(s);
            }),
            bench(|| {
                let mut s = 0u64;
                for &x in black_box(&ec_v).iter() {
                    s = s.wrapping_add(x);
                }
                black_box(s);
            }),
            bench(|| {
                let mut s = 0u64;
                for &x in black_box(&std_v).iter() {
                    s = s.wrapping_add(x);
                }
                black_box(s);
            }),
        );

        // turbocow-only: SmallVec inline vs the same as a Vec push baseline.
        use turbocow::SmallVec;
        row2(
            "SmallVec<8>::push 8",
            bench(|| {
                let mut v: SmallVec<'_, u64, 8> = SmallVec::new();
                for i in 0..8u64 {
                    v.push(black_box(i));
                }
                black_box(&v);
            }),
            bench(|| {
                let mut v = TcEv::new();
                for i in 0..8u64 {
                    v.push(black_box(i));
                }
                black_box(&v);
            }),
        );
    }

    println!(
        "\n(min ns/op; tc/ec ratio: <=1.05 parity/faster, <=1.25 ~, else flagged.\n SmallVec row compares vs turbocow EcoVec, not ecow.)"
    );
}
