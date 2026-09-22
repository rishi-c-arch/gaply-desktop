# Evidence Ledger — citation verification

**Scope.** Every citation attributed in the solutions document and the original
architecture document, checked against the actual paper record: ACL Anthology,
AAAI proceedings, the arXiv API, Crossref, or the publisher's own documentation.
**Search-result snippets were not accepted as evidence** — each classification
below rests on a fetched authoritative record.

**Nothing in the application was changed.** This file is the only output.

**A note on reading depth.** Where a paper is paywalled I read the official
abstract/record and say so; those rows are not classified above PARTIALLY
SUPPORTED on the unread part. Where a record states a number, that number is
carried; **no number appears here that its own source does not state.**

---

## 1. The table

| # | Citation | Exists | Venue / Year | Exact claim made | Actual evidence | Domain | Dataset | Classification | Gaply applicability | Implementation consequence |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | **ReviewGrounder**, Zhuofeng Li et al., `2026.acl-long.1477` | ✅ | ACL 2026 (main) | draft → targeted grounding → verification | Two staged phases: draft, then "targeted evidence consolidation" via tool-integrated agents; beats larger baselines on ReviewBench across 8 rubric dimensions | AI-conference peer review | ReviewBench | **VERIFIED — CLAIM SUPPORTED** *(established, preserved)* | Pipeline shape transfers; the evaluation does not | KEEP. Limitation stands: AI-conference reviews, not journal peer review; measures review quality, **not** acceptance or editor utility |
| 2 | **Applicability Condition Extraction**, Luo, Nishida, Matsumoto, Arase, `2026.findings-acl.154` | ✅ | Findings of ACL 2026 | conditions govern when a relation applies | First dataset of drug–disease–condition triples; **1,119 drug–disease pairs** on biomedical abstracts; LoRA-based method beats baselines | Biomedical literature | Own annotated set, biomedical **abstracts** | **VERIFIED — PARTIALLY SUPPORTED** *(established, preserved)* | Conceptual only | KEEP BUT MEASURE. Explicitly **no** evaluation on journal rules or checklist applicability; transfer to A3 is a hypothesis Gaply must measure |
| 3 | **Talaria**, Chung-ju Huang et al., `2026.acl-long.4` | ✅ | ACL 2026 (main) | confidential inference for cloud LLMs | Partitions the pipeline between a **client-verified Confidential VM** and the cloud; requires host-side partitioning; frames itself as "the first work that ensures clients' prompts and responses remain inaccessible to the cloud" | Confidential computing | — | **VERIFIED — PARTIALLY SUPPORTED** *(established, preserved)* | As a concept only | RE-JUSTIFY. **Not** an implementation basis for the OpenAI path: needs a client-controlled CVM and model-host cooperation. The paper's own "first work" framing refutes any claim that a commercial API already provides this |
| 4 | **DAPR**, Wang, Reimers, Gurevych, `2024.acl-long.236` | ✅ | ACL 2024 (main) | hybrid BM25 + dense solves document context | **53.5%** of major retriever errors come from missing document context; hybrid is strongest on mixed queries but "completely fails on the hard queries that require document-context understanding" | Wikipedia + research papers | DAPR benchmark | **CONTRADICTS** *(established, preserved)* | Directly applicable | REMOVE AS JUSTIFICATION for hybrid retrieval solving document context. Consistent with Gaply's own §11 D207 |
| 5 | **SciERC / SciIE**, Luan, He, Ostendorf, Hajishirzi, `D18-1360` | ✅ | EMNLP 2018 | joint entities + relations + coreference; multi-task reduces cascading errors | Unified framework with shared span representations; joint IE with coreference propagation | AI research text | **500 abstracts**, 12 AI conference/workshop proceedings — **abstracts, not full text** | **VERIFIED — CLAIM SUPPORTED** | Method transfers; domain does not | KEEP BUT MEASURE. Nothing evaluated on journal instructions or manuscript requirements |
| 6 | **SciER**, Zhang, Chen, Pan, Caragea, Latecki, Dragut, `2024.emnlp-main.726` | ✅ | EMNLP 2024 (main) | full-text extraction of datasets/methods/tasks; 106 papers, >24k entities, >12k relations | Record confirms **106 full-text publications, >24,000 entities, >12,000 relations**, plus an out-of-distribution test set | Scientific (dataset/method/task entities) | SciER | **VERIFIED — CLAIM SUPPORTED** | Full-text IE is the right granularity | KEEP BUT MEASURE. Entity types are datasets/methods/tasks — **not** manuscript requirements or compliance items |
| 7 | **S2ORC**, Lo, Wang, Neumann, Kinney, Weld, `2020.acl-main.447` | ✅ | ACL 2020 (main) | structured full text with citations, figures, tables | **81.1M papers, 8.1M full texts**; full text "annotated with automatically-detected inline mentions of citations, figures, and tables" | Multi-domain academic | S2ORC | **VERIFIED — CLAIM SUPPORTED** | Corpus resource | KEEP as a corpus citation only; it is not evidence for any reasoning capability |
| 8 | **RAID**, Dugan et al., `2024.acl-long.674` | ✅ | ACL 2024 (main) | >6M generations, 11 models, 8 domains, attacks + decoding; detectors are not robust | Confirmed: **>6M generations, 11 models, 8 domains, 11 adversarial attacks, 4 decoding strategies, 12 detectors**; detectors "easily fooled by adversarial attacks, variations in sampling strategies, repetition penalties, and unseen generative models" | Machine-generated text detection | RAID | **VERIFIED — CLAIM SUPPORTED** | Strongly applicable to A6 | KEEP. The 8 domains are **not enumerated in the abstract** and full text was not read, so whether scientific manuscripts are covered is unconfirmed |
| 9 | **Attribute or Abstain**, Buchmann, Liu, Gurevych, `2024.emnlp-main.463` | ✅ | EMNLP 2024 (main) | evidence attribution for long-document assistants | Introduces **LAB**: 6 long-document tasks, 5 LLMs | Long documents | LAB | **VERIFIED — CLAIM SUPPORTED** | Applicable to the evidence gate | KEEP |
| 10 | **CoSAEmb**, Shruti Singh, Mayank Singh, `2024.sdp-1.27` | ✅ | **SDP 2024 workshop** (not main conference) | section-aware representations improve full-text retrieval | Trained on **97,402 S2ORC papers**; improves full-text IR "in comparison to models trained only on paper titles and abstracts"; **comparable** to SOTA on SciRepEval and CSFCube | Scientific articles | S2ORC subset | **VERIFIED — CLAIM SUPPORTED** | Applicable | KEEP BUT MEASURE. Venue is a workshop; the gain is over title/abstract-only baselines, not a general SOTA claim |
| 11 | **LAB** (cited as a separate work) | ⚠️ | — | attribution as its own problem; increases verifiability; complex claims hard | **LAB is the benchmark introduced by #9**, not a separate paper. Findings: long-document attribution evaluation "is missing" from prior RAG-only setups ✅; "models struggle with providing evidence for complex claims" ✅; the *verifiability* benefit is motivation, not a measured result in the abstract | Long documents | LAB | **VERIFIED — PARTIALLY SUPPORTED** | Applicable | **CITATION ERROR — merge into #9.** Keep the "complex claims are hard" finding; drop "increases verifiability" as an empirical claim |
| 12 | **FRONT**, Lei Huang et al., `2024.findings-acl.838` | ✅ | Findings of ACL 2024 | grounds claims in fine-grained quotes, not whole-document citations | "By initially grounding fine-grained supporting quotes, which then guide the generation process…" | General QA attribution | **ALCE** | **VERIFIED — CLAIM SUPPORTED** | Mechanism transfers | KEEP BUT MEASURE. No evaluation on scientific manuscripts or peer review |
| 13 | **Can AI Be a Good Peer Reviewer?**, Sihong Wu et al., arXiv `2604.27924` | ✅ | ACL 2026 (accepted; survey) | review architectures and evaluation methodology | Covers fine-tuning, agent-based and RL methods, rebuttals/meta-reviews, and "human-centered, reference-based, LLM-based and aspect-oriented" evaluation; discusses "limitations, ethical concerns, and future directions" | Peer review | survey | **VERIFIED — CLAIM SUPPORTED** | Directly relevant | KEEP as a survey citation; a survey is not evidence that any component works |
| 14 | **LLMs for automated scholarly paper review**, Zhuang, Chen, Xu, Jiang, Lin | ✅ | Information Fusion **124**, 103332, 2025; DOI `10.1016/j.inffus.2025.103332` | survey including limitations | Record confirms survey scope and that it "summarize[s] the performance and issues of LLMs in ASPR" and publisher/academic attitudes | Peer review | survey | **VERIFIED — CLAIM SUPPORTED** | Relevant | KEEP. **Full text not read** (ScienceDirect/ACM 403); verified from the arXiv record carrying the journal ref and DOI |
| 15 | **Fine-Grained Distillation for Long Document Retrieval**, Yucheng Zhou et al. | ✅ | **AAAI 2024**, proceedings article 29947; arXiv `2212.10423` | multi-granularity retrieval/distillation | "global-consistent representations crossing different fine granularity"; multi-granular aligned distillation at training time only; two long-document retrieval benchmarks | Long-document IR | two LDR benchmarks (unnamed in record) | **VERIFIED — CLAIM SUPPORTED** | Retrieval only | KEEP BUT MEASURE. arXiv v1 (2022) states **no venue**; AAAI 2024 confirmed separately from the proceedings. Published author list adds Jianbing Shen |
| 16 | **Neural NLP for Long Texts**, Tsirmpas, Gkionis, Papadopoulos, Mademlis | ✅ | EAAI **133**, 108231, 2024; DOI `10.1016/j.engappai.2024.108231` | survey of long-document architectures | Confirmed survey with "an original overarching taxonomy of common deep neural methods for long document analysis" | Long text (web, legal, medical, financial) | survey | **VERIFIED — CLAIM SUPPORTED** | Background only | KEEP as background. Explicitly **does not** address scientific manuscripts or peer review |
| 17 | Unnamed ScienceDirect paper on topic-graph retrieval | ❌ | — | preserves related evidence while reducing redundancy | No title, author or DOI supplied; ScienceDirect returns **HTTP 403** to the fetcher | — | — | **NOT VERIFIED** | Unknown | REMOVE AS JUSTIFICATION until identified by DOI |
| 18 | Unnamed ScienceDirect source on AI peer-review limitations | ❌ | — | validation, scientific content, trustworthy deployment | No identifying metadata supplied; not resolvable | — | — | **NOT VERIFIED** | Unknown | REMOVE AS JUSTIFICATION until identified by DOI |
| 19 | **SciPy `loadmat` documentation** | ✅ | SciPy official docs | v4/v6/v7–7.2 supported; v7.3 needs HDF5 | Verbatim: "v4 (Level 1.0), v6 and v7 to 7.2 matfiles are supported" and "You will need an HDF5 Python library to read MATLAB 7.3 format mat files" | Software documentation | — | **VERIFIED — CLAIM SUPPORTED** | Directly applicable to C6 | KEEP. Independently corroborates Gaply's MATLAB 7.3 refusal boundary |

### Lower-priority sources (existence + one-line relevance only)

| Identifier | Exists | Record | Topic | Relevance to Gaply's review claims |
|---|---|---|---|---|
| arXiv `2308.00352` | ✅ | MetaGPT, Sirui Hong, 2023 | SOPs encoded into multi-agent prompts | Agent framework — not peer review evidence |
| arXiv `2308.08155` | ✅ | AutoGen, Qingyun Wu, 2023 | conversational multi-agent framework | Agent framework — not peer review evidence |
| arXiv `2307.07924` | ✅ | ChatDev, Chen Qian, 2023 | agents for software development | Agent framework — **software**, not review |
| arXiv `2604.20801` | ✅ | Synthesizing Multi-Agent Harnesses for Vulnerability Discovery, Hanzhi Liu, 2026 | security vulnerability discovery | Unrelated domain |
| arXiv `2603.09619` | ✅ | Context Engineering: From Prompts to Corporate Multi-Agent Architecture, Vishnyakova, 2026 | enterprise agent architecture | Not peer review |
| arXiv `2510.04618` | ✅ | Agentic Context Engineering, Qizheng Zhang, 2025 | evolving contexts for self-improvement | Not peer review |
| arXiv `2507.13334` | ✅ | A Survey of Context Engineering for LLMs, Lingrui Mei, 2025 | context engineering survey | Background only |
| arXiv `2603.07670` | ✅ | Memory for Autonomous LLM Agents, Pengfei Du, 2026 | agent memory survey | Background only |
| `2025.acl-long.131` | ✅ | **MAIN-RAG**, Chia-Yuan Chang et al., ACL 2025 | multi-agent filtering for RAG | Retrieval filtering — not review |
| DOI `10.1016/j.neucom.2026.134438` | ✅ | **MeMAT**, Gege Sun et al., Neurocomputing 699, 2026 | multi-agent transformer with memory | Multi-agent RL — unrelated |
| IOS Press `FAIA251160` | ✅ | **Mem0**, Chhikara et al., ECAI 2025 | long-term memory for agents | Agent memory — not review |
| IOS Press `FAIA251440` | ✅ | **Multi-Agentic Workflows and Long-Term Memory Use Cases in AI-Builder**, Murthy et al., ECAI 2025 | agentic workflow prototypes | Not review |
| ScienceDirect `S0925231225011427` | ❌ | ScienceDirect returns **HTTP 403**; no DOI supplied | — | **NOT VERIFIED** |

---

## 2. Grouped by classification

**VERIFIED — CLAIM SUPPORTED (13):** ReviewGrounder (#1), SciERC (#5), SciER (#6),
S2ORC (#7), RAID (#8), Attribute or Abstain (#9), CoSAEmb (#10), FRONT (#12),
Can AI Be a Good Peer Reviewer? (#13), Zhuang survey (#14), FGD (#15),
Neural NLP for Long Texts (#16), SciPy docs (#19).

**VERIFIED — PARTIALLY SUPPORTED (3):** Applicability Condition Extraction (#2),
Talaria (#3), LAB (#11).

**CONTRADICTS (1):** DAPR (#4).

**NOT VERIFIED (2 of the 19, plus 1 lower-priority):** unnamed topic-graph
retrieval (#17), unnamed AI peer-review limitations (#18); and ScienceDirect
`S0925231225011427` from the lower-priority list.

**Reconciliation of the 19 numbered citations: 13 supported + 3 partially
supported + 1 contradicts + 2 not verified = 19.**

**VERIFIED — CLAIM NOT SUPPORTED (0).** No citation was found to be irrelevant
to its stated topic; the failures are of *transfer*, not of *topic*.

---

## 3. Implementation implications

Stated as dispositions only. **Nothing here is implemented.**

| Component | Disposition | Why |
|---|---|---|
| **Hybrid retrieval (BM25 + dense)** | **REMOVE AS JUSTIFICATION** | DAPR measures it *failing* on exactly the hard document-context queries it was cited to solve. Keep hybrid retrieval if it earns its place on Gaply's own numbers; do not cite DAPR for it |
| **Document-aware retrieval** | **KEEP BUT MEASURE** | DAPR establishes the *problem* (53.5% of errors) without providing a solution. The need is evidenced; the remedy is not |
| **Draft → ground → verify reviewer** | **KEEP** | ReviewGrounder supports the architecture. Its evaluation is AI-conference reviews on ReviewBench — **not** journal peer review, acceptance, or editor usefulness |
| **Evidence / quote gate** | **KEEP** | FRONT and Attribute-or-Abstain both support quote-level grounding. Neither was evaluated on manuscripts; LAB additionally shows complex claims remain hard to attribute |
| **Study Graph with coreference** | **KEEP BUT MEASURE** | SciERC and SciER support joint IE over scientific text — 500 AI *abstracts* and 106 full texts of *datasets/methods/tasks* respectively. Neither touches manuscript requirements. Gaply's §11 D207 already measured the harder version of this task at below no-skill |
| **Applicability extraction** | **KEEP BUT MEASURE** | Luo et al. is the closest published analogue and is biomedical drug–disease conditions on abstracts. Transfer to journal checklist applicability (dossier A3) is unevaluated anywhere |
| **Span-level AI detection** | **RE-JUSTIFY** | RAID supports the *robustness problem*, not any particular detector. It is evidence **against** confidence in detection, which is consistent with Gaply's own A6 finding that the verdict was driven by the bibliography |
| **Confidential inference** | **RE-JUSTIFY** | Talaria is a design requiring a client-controlled CVM and host cooperation. It cannot be cited as something the OpenAI path provides — the paper says it is the *first* to achieve this |
| **Section-aware embeddings** | **KEEP BUT MEASURE** | CoSAEmb supports the idea on full-text IR, from a workshop paper, with gains over title/abstract-only baselines and *comparable* (not superior) benchmark performance |

---

## 4. What the literature does not settle

Three experiments follow from the gaps above, and none is answered by any paper
in this ledger:

1. **Does document context help Gaply's task?** DAPR says hybrid retrieval does
   not solve it; Gaply's §11 D207 measured context arriving and changing nothing
   (+0.3 / −1.7). No cited paper measures the positive case on manuscripts.
2. **Does applicability transfer from biomedical relations to journal rules?**
   Luo et al. is the only evidence and it is a different domain, a different
   unit, and abstracts rather than requirement pages.
3. **Is any AI-writing detector reliable on scientific manuscripts?** RAID
   establishes that detectors are not robust in general and does not enumerate
   its domains in the abstract; Gaply's own measurement found the signal driven
   by non-prose.
