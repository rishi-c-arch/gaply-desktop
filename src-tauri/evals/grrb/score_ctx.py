"""Score the with-context run against §11 D204, on the rows both could answer.

Usage: python3 score_ctx.py <ctx-raw.tsv>

Every number is printed beside the row count that produced it, and the D204
comparison is restricted to the SAME rows — the with-context payload is
refused on 4 rows D204 answered, so an unrestricted comparison would put the
two runs on different denominators.
"""
import json, sys, collections

SRC = "/Users/rishi/dev/gaply-react-frontend/src-tauri/evals/grrb/"
REP = "/Users/rishi/dev/gaply-react-frontend/src-tauri/evals/reports/"

cases = {json.loads(l)["id"]: json.loads(l)
         for l in open(SRC + "scientific_extraction_ctx.jsonl") if l.strip()}

def load(path):
    out = {}
    for line in open(path):
        p = line.rstrip("\n").split("\t")
        if len(p) < 4 or p[0] == "variant":
            continue
        out.setdefault(p[0], {})[p[1]] = p[3]
    return out

def metrics(ans, ids):
    tp = sum(1 for i in ids if ans.get(i) == "yes" and cases[i]["expected"]["finding"])
    fp = sum(1 for i in ids if ans.get(i) == "yes" and not cases[i]["expected"]["finding"])
    pos = sum(1 for i in ids if cases[i]["expected"]["finding"])
    fn = pos - tp
    return dict(n=len(ids), yes=tp + fp, tp=tp, fp=fp, fn=fn, pos=pos,
                precision=100 * tp / (tp + fp) if tp + fp else 0.0,
                recall=100 * tp / pos if pos else 0.0)

ctx = load(sys.argv[1])
old = {"A": load(REP + "grrb-cloud-gpt4o-A-raw.tsv").get("A", {}),
       "B": load(REP + "grrb-cloud-gpt4o-B-raw.tsv").get("B", {})}

print("id counts per variant:",
      {v: len(a) for v, a in ctx.items()},
      "| answered:", {v: sum(1 for x in a.values() if x in ("yes", "no")) for v, a in ctx.items()})
for v, a in ctx.items():
    print(f"  variant {v} outcome mix:", dict(collections.Counter(a.values())))

for v in sorted(ctx):
    a = ctx[v]
    scored = {i for i, x in a.items() if x in ("yes", "no")}
    common = sorted(scored & {i for i, x in old[v].items() if x in ("yes", "no")})
    print(f"\n=== variant {v} ===  answered {len(scored)}   common with D204 {len(common)}")
    for name, answers, ids in (
        ("WITH CONTEXT   (this run)", a, common),
        ("D204 no context, same rows", old[v], common),
    ):
        m = metrics(answers, ids)
        print(f"  {name:28s} n={m['n']:3d} pos={m['pos']:2d} yes={m['yes']:3d} "
              f"tp={m['tp']:2d} fp={m['fp']:2d} fn={m['fn']:2d}  "
              f"precision {m['precision']:5.1f}%  recall {m['recall']:5.1f}%")
    # the falsifiable sub-prediction: does the yes-rate become position-aware?
    for name, answers in (("with context", a), ("D204", old[v])):
        inm = [i for i in common if cases[i]["context"]["kind"] == "Methods"]
        out = [i for i in common if cases[i]["context"]["kind"] != "Methods"]
        yi = sum(1 for i in inm if answers.get(i) == "yes")
        yo = sum(1 for i in out if answers.get(i) == "yes")
        print(f"    {name:13s} yes-rate  in Methods {yi:2d}/{len(inm):2d} ({100*yi/len(inm):4.0f}%)"
              f"   outside {yo:2d}/{len(out):2d} ({100*yo/len(out):4.0f}%)")
