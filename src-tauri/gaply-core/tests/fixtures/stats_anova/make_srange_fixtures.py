"""The studentized range's gold reference (Research OS, Phase 8G, decisions
314 to 316): P(Q > q; k, nu) evaluated in mpmath from the distribution's
definition, for tests/studentized_range_reference.rs.

    P(W > w)       = k * int phi(z) [Phi(z)^(k-1) - (Phi(z) - Phi(z - w))^(k-1)] dz
    P(Q > q; k, nu) = int P(W > q s) f_nu(s) ds,  f_nu the density of chi_nu / sqrt(nu)

Written from that definition and nothing else: the bracket is the plain
difference of powers, evaluated at WORKING_DPS digits (the cancellation it
suffers is at most about 15 digits at the p this grid reaches), and the outer
integral is in s itself, not in ln s as gaply_core's is, on breakpoints of
its own. Each integral is a composite Gauss-Legendre rule (mpmath's nodes)
on fixed panels: the inner on width-3 panels over z in [-13, 13] (its
integrand is at most k phi(z), below 1e-36 outside), the outer as upper()
places them. Every row's value is at DEGREE; every CHECK_EVERY-th row, and
every row below df 2, is evaluated again at DEGREE + 1 (twice the nodes per
panel), and the relative difference is written beside the value as the
reference's own error estimate ("NA" on the rows not checked).

q comes from srange_placement.tsv (make_srange_placement.R: R's qtukey at a
target p for df >= 2, a fixed q grid below df 2, where R returns NaN). Where
R could not place a target at df 2 to 5, q is extrapolated from the last
placed pair by the tail's power law, q * (p_last / p)^(1/df). The placement
need not be exact: each row records mpmath's p at the q it holds.

Run with: python3 -I make_srange_fixtures.py > srange_mpmath.tsv
(stderr carries progress). Rows are computed in 8 processes.
"""
import sys
from multiprocessing import Pool

import mpmath as mp

WORKING_DPS = 30
OUT_DIGITS = 30
DEGREE = 4  # 3 * 2^(DEGREE - 1) = 24 nodes per panel; DEGREE + 1 has 48

PS = [0.5, 0.1, 0.05, 0.01, 1e-3, 1e-5, 1e-8, 1e-10]

_nodes = {}


def nodes(degree):
    if degree not in _nodes:
        gl = mp.calculus.quadrature.GaussLegendre(mp.mp)
        _nodes[degree] = gl.calc_nodes(degree, mp.mp.prec)
    return _nodes[degree]


def composite(f, edges, degree):
    total = mp.mpf(0)
    for a, b in zip(edges, edges[1:]):
        mid, half = (a + b) / 2, (b - a) / 2
        total += half * mp.fsum(w * f(mid + half * x) for x, w in nodes(degree))
    return total


_tables = {}


def table(k, degree):
    """The inner rule's fixed nodes in z (panels of width 3 on [-13, 13]),
    with weight * phi(z) and Phi(z) at each, cached per (k, degree)."""
    if (k, degree) not in _tables:
        rows = []
        for a in range(-13, 13, 3):
            a, b = mp.mpf(a), mp.mpf(min(a + 3, 13))
            mid, half = (a + b) / 2, (b - a) / 2
            for x, w in nodes(degree):
                z = mid + half * x
                rows.append((z, half * w * mp.npdf(z), mp.ncdf(z)))
        _tables[(k, degree)] = rows
    return _tables[(k, degree)]


def range_upper(w, k, degree):
    """P(W > w), from the definition (the plain difference of powers)."""
    if w <= 0:
        return mp.mpf(1)
    if w > 40:
        return mp.mpf(0)
    total = mp.fsum(wt * (pz ** (k - 1) - (pz - mp.ncdf(z - w)) ** (k - 1)) for z, wt, pz in table(k, degree))
    return k * total


def chi_density(s, nu):
    """The density of chi_nu / sqrt(nu) at s > 0."""
    return mp.exp(
        (nu / 2) * mp.log(nu)
        - mp.loggamma(nu / 2)
        - (nu / 2 - 1) * mp.log(2)
        + (nu - 1) * mp.log(s)
        - nu * s * s / 2
    )


def upper(q, k, nu, degree):
    q = mp.mpf(q)
    if nu == "inf":
        return range_upper(q, k, degree)
    nu = mp.mpf(nu)
    sd = 1 / mp.sqrt(2 * nu)
    # The density's support, to where it is below 1e-40 of its peak: a
    # crude bound found by stepping, never by the gaply_core formula.
    peak = chi_density(mp.mpf(1), nu)
    hi = mp.mpf(1)
    while chi_density(hi, nu) > peak * mp.mpf("1e-45"):
        hi += max(sd, mp.mpf("0.01"))
    # Below lo the density's mass is about f(lo) lo / nu (it rises as
    # s^(nu - 1)): stop when that is below 1e-42 of the peak.
    lo = mp.mpf(1)
    while lo > mp.mpf("1e-300") and chi_density(lo, nu) * lo > peak * mp.mpf("1e-42"):
        lo = lo - sd if lo - sd > lo / 2 else lo / 16
    hi = min(hi, mp.mpf(40) / q)
    if hi <= lo:
        return mp.mpf(0)
    # Then panels of 2 sd
    # width to hi; panels of width at most sd / 2 near the peak.
    # Below 1 - 20 sd, or below 0.5 at small nu, the panels are geometric
    # (ratio 4), so no panel touches 0, where the density's s^(nu - 1) is a
    # fractional power at nu 1.25 and 1.5 (a first panel [0, 1.26] there left
    # the rule 1.4e-5 from itself at the next degree).
    edges = [lo]
    near = max(lo, 1 - 20 * sd if 1 - 20 * sd > 0.5 else mp.mpf("0.5"))
    while edges[-1] * 4 < near:
        edges.append(edges[-1] * 4)
    if near > edges[-1]:
        edges.append(near)
    step = min(2 * sd, (hi - edges[-1]) / 8) if hi > edges[-1] else sd
    while edges[-1] + step < hi:
        nxt = edges[-1] + step
        edges.append(nxt)
        if nxt > 1 + 20 * sd:
            step = step * 2
    edges.append(hi)
    # Where q s crosses the range distribution's transition, add panels.
    extra = [x / q for x in (mp.mpf("0.5"), mp.mpf(1), mp.mpf(2), mp.mpf(4), mp.mpf(8), mp.mpf(16))]
    edges = sorted(set(edges + [e for e in extra if lo < e < hi]))
    return composite(lambda s: range_upper(q * s, k, degree) * chi_density(s, nu), edges, degree)


def placements():
    rows = []
    by = {}
    for line in open("srange_placement.tsv"):
        if line.startswith("#") or line.startswith("q\t"):
            continue
        q, k, d, rp, pt = line.rstrip("\n").split("\t")
        if float(q) <= 0:
            # R's qtukey gave 0 for this target: no point to place.
            continue
        rows.append((float(q), int(k), d, rp, pt))
        if pt != "NA":
            by.setdefault((int(k), d), []).append((float(pt), float(q)))
    for (k, d), placed in by.items():
        nu = float(d) if d != "inf" else None
        if nu is None or nu >= 10:
            continue
        have = {p for p, _ in placed}
        p_last, q_last = min(placed)
        for p in PS:
            if p not in have and p < p_last:
                q = q_last * (p_last / p) ** (1 / nu)
                rows.append((float(f"{q:.12g}"), k, d, "NA", repr(p)))
    return rows


CHECK_EVERY = 5


def row(args):
    index, (q, k, nu, rp, pt) = args
    mp.mp.dps = WORKING_DPS
    a = upper(q, k, nu, DEGREE)
    small = nu != "inf" and float(nu) < 2
    if index % CHECK_EVERY == 0 or small:
        b = upper(q, k, nu, DEGREE + 1)
        err = mp.nstr(abs(a - b) / abs(b), 3) if b != 0 else "0"
    else:
        err = "NA"
    print(f"done k={k} nu={nu} q={q}", file=sys.stderr, flush=True)
    return f"{q!r}\t{k}\t{nu}\t{mp.nstr(a, OUT_DIGITS)}\t{err}\t{rp}\t{pt}"


if __name__ == "__main__":
    import platform

    mp.mp.dps = WORKING_DPS
    grid = placements()
    if len(sys.argv) > 1 and sys.argv[1] == "--sample":
        grid = [g for g in grid if (g[1], g[2]) in ((3, "10"), (5, "1e5"), (50, "1"), (2, "inf"))][::9]
    print(f"# written by: python3 -I make_srange_fixtures.py ({platform.python_implementation()} {platform.python_version()}, mpmath {mp.__version__})")
    print(f"# WORKING_DPS {WORKING_DPS}, DEGREE {DEGREE}, checked at {DEGREE + 1} every {CHECK_EVERY}th row and below df 2, {len(grid)} rows; r_ptukey_upper and p_target from srange_placement.tsv")
    print("q\tk\tdf\tp_upper\tself_error\tr_ptukey_upper\tp_target")
    done = {}
    if len(sys.argv) > 2 and sys.argv[1] == "--resume":
        # Rows an interrupted run of this same script already wrote, reused
        # as they are.
        for line in open(sys.argv[2]):
            if not line.startswith("#") and not line.startswith("q\t"):
                f = line.split("\t")
                done[(float(f[0]), int(f[1]), f[2])] = line.rstrip("\n")
    todo = [(i, g) for i, g in enumerate(grid) if (g[0], g[1], g[2]) not in done]
    with Pool(8) as pool:
        computed = dict(zip([i for i, _ in todo], pool.imap(row, todo)))
    for i, g in enumerate(grid):
        print(done.get((g[0], g[1], g[2])) or computed[i], flush=True)
