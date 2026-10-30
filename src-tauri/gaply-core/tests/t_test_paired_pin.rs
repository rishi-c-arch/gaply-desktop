//! **`stats_verify::t_test_paired`'s outputs, pinned bit for bit (Research
//! OS, Phase 8D-1, decision 254).** Level 1's paired t-test computes through
//! it; 8D-1 makes it share its arithmetic with the one-sample t-test. This
//! test was written and green on `81aed16`'s code, before that change, so the
//! change is shown to leave every bit and every refusal as it was.
//!
//! The inputs: Level 1's paired datasets (`data_paired.csv`,
//! `data_paired_small.csv`, the precision sweep's `c06_paired_2dec.csv`, each
//! with its incomplete rows left out), 200 pairs of vectors from a seeded
//! xorshift generator (ties, a 1e9 offset, negative values, n = 2), and one
//! input for each refusal: unequal lengths, one pair, constant differences,
//! cells near 1e200 and cells near 1e-200.
//!
//! Two SHA-256 digests over one line per input:
//! * `CORE` covers t, df, se, mean_a and mean_b as their IEEE 754 bits, the
//!   method word, and each refusal's `Debug` text (variant and message;
//!   `GaplyError` has no `PartialEq`). Checked on every platform: these are
//!   sums, products, quotients and square roots, which IEEE 754 fixes.
//! * `P` covers `p_two_sided`, which goes through the platform's maths
//!   library (statrs' Student t). Captured on macOS aarch64 and, since
//!   Research OS Phase 8E (decision 277), checked on every platform: the
//!   reviewer got the same digest on Linux x86_64 at `3d1253e`; CI's Linux
//!   job checks it on every push, the Windows job on a pull request.

use gaply_core::contracts::ContentHash;
use gaply_core::stats_verify::t_test_paired;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_l1");

/// Captured on `81aed16`'s code, macOS aarch64, 26 Oct 2026.
const CORE: &str = "3e308b9efcbabd68826975c0ef32dd57a79205fe6132ca01c722ae1e25c01616";
const P: &str = "6297bc48f0f0af13970b11fc4bfeb8afc926ae4bf6465d5bbd3df702ff22df62";

/// xorshift64*, seeded: the same stream on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1), from the top 53 bits.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// `v` rounded to `places` decimals, as a data file would hold it.
fn rounded(v: f64, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (v * k).round() / k
}

/// The complete rows of a two-column CSV below its '#' header, in order.
fn pairs(path: &str) -> (Vec<f64>, Vec<f64>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    lines.next().expect("a header");
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        // A missing cell is empty or `NA` (Level 1's fixtures write `NA`).
        let present = |s: &str| !s.is_empty() && s != "NA";
        if f.len() == 2 && present(f[0]) && present(f[1]) {
            a.push(f[0].parse::<f64>().expect("a number"));
            b.push(f[1].parse::<f64>().expect("a number"));
        }
    }
    (a, b)
}

/// One random pair of vectors, of one of four shapes.
fn random_pair(rng: &mut Rng, k: usize) -> (Vec<f64>, Vec<f64>) {
    let n = if k % 25 == 0 {
        2
    } else {
        2 + rng.below(39) as usize
    };
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let (x, y) = match k % 4 {
            // Ties: integers 1 to 5.
            0 => ((1 + rng.below(5)) as f64, (1 + rng.below(5)) as f64),
            // A 1e9 offset with a spread of about 10.
            1 => (
                1e9 + rounded((rng.unit() - 0.5) * 20.0, 3),
                1e9 + rounded((rng.unit() - 0.5) * 20.0 + 1.0, 3),
            ),
            // Negative values.
            2 => (
                -500.0 + rounded((rng.unit() - 0.5) * 60.0, 2),
                -480.0 + rounded((rng.unit() - 0.5) * 50.0, 2),
            ),
            // Related columns: y near x.
            _ => {
                let x = rounded(rng.unit() * 100.0, 2);
                (x, rounded(x + (rng.unit() - 0.3) * 8.0, 2))
            }
        };
        a.push(x);
        b.push(y);
    }
    (a, b)
}

fn inputs() -> Vec<(String, Vec<f64>, Vec<f64>)> {
    let mut out = Vec::new();
    for name in [
        "data_paired.csv",
        "data_paired_small.csv",
        "precision_sweep/c06_paired_2dec.csv",
    ] {
        let (a, b) = pairs(&format!("{DIR}/{name}"));
        assert!(a.len() >= 2, "{name}: {} complete rows", a.len());
        out.push((name.to_string(), a, b));
    }
    let mut rng = Rng(0x8D1_2026_1026);
    for k in 0..200 {
        let (a, b) = random_pair(&mut rng, k);
        out.push((format!("random {k}"), a, b));
    }
    out.push((
        "unequal lengths".into(),
        vec![1.0, 2.0, 3.0],
        vec![1.0, 2.0],
    ));
    out.push(("one pair".into(), vec![1.0], vec![2.0]));
    out.push((
        "constant differences".into(),
        vec![1.0, 2.0, 3.0],
        vec![0.0, 1.0, 2.0],
    ));
    out.push((
        "near 1e200".into(),
        vec![1e200, -1e200, 1.5e200],
        vec![-1e200, 1e200, -1.5e200],
    ));
    out.push((
        "near 1e-200".into(),
        vec![1e-200, 2e-200, 3e-200],
        vec![0.0, 0.0, 0.0],
    ));
    out
}

#[test]
fn t_test_paired_gives_the_same_bits_and_refusals_as_before_8d1() {
    let mut core = String::new();
    let mut p = String::new();
    let (mut computed, mut refused) = (0, 0);
    for (name, a, b) in inputs() {
        match t_test_paired(&a, &b) {
            Ok(r) => {
                core.push_str(&format!(
                    "{name}: {:016x} {:016x} {:016x} {:016x} {:016x} {}\n",
                    r.t.to_bits(),
                    r.df.to_bits(),
                    r.se.to_bits(),
                    r.mean_a.to_bits(),
                    r.mean_b.to_bits(),
                    r.method
                ));
                p.push_str(&format!("{name}: {:016x}\n", r.p_two_sided.to_bits()));
                computed += 1;
            }
            Err(e) => {
                core.push_str(&format!("{name}: {e:?}\n"));
                refused += 1;
            }
        }
    }
    let core_hash = ContentHash::of_bytes(core.as_bytes());
    let p_hash = ContentHash::of_bytes(p.as_bytes());
    println!(
        "{computed} computed, {refused} refused; CORE {} P {}",
        core_hash.as_hex(),
        p_hash.as_hex()
    );
    // Each refusal reached its own branch: 5 inputs refuse, every other computes.
    assert_eq!((computed, refused), (203, 5));
    assert_eq!(
        core_hash.as_hex(),
        CORE,
        "t, df, se, the means, the method or a refusal changed:\n{core}"
    );
    assert_eq!(
        p_hash.as_hex(),
        P,
        "p changed (captured on macOS aarch64):\n{p}"
    );
}
