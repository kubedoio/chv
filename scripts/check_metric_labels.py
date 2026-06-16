#!/usr/bin/env python3
"""Detect Prometheus alert/emitter label drift in the CHV repo.

See scripts/check-metric-labels.sh for the operator-facing rationale and
limitations. This is the parser that does the actual work.

Algorithm
---------
1. Walk crates/**/*.rs, collect every `metrics::counter!`, `metrics::histogram!`,
   `metrics::gauge!` invocation. For each invocation extract:
     - metric name (string literal, OR `CHV_*` const resolved against
       chv-observability/src/lib.rs)
     - the set of label keys passed (e.g. "op", "result", "status")
   Result: dict[metric_name -> set[label_key]]

2. Walk monitoring/rules/*.yml, extract every `<metric_name>{<label>="<value>"}`
   pattern and every `<metric_name>` bare reference. For PromQL aggregations
   the matrix selector form `metric{...}[5m]` is supported. The histogram
   suffixes `_bucket`, `_count`, `_sum` are stripped before lookup.

3. For every (metric_name, label_key) pair in the rule set, verify the
   metric is in the emitter set AND the label_key is in that metric's set.
   Allow-list known-external metrics (`up`, `http_requests_total`, etc.).

4. Print a structured drift report; exit 1 if any drift, 0 otherwise.

Output format
-------------
Drift entries are printed in the form:
  monitoring/rules/<file>.yml:<line>: alert "<name>" references
    metric "<metric>" with unknown label "<label_key>"
    (emitter labels: {<known_keys>})

Designed to be easy to read in CI logs and to grep.
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CRATES_DIR = REPO_ROOT / "crates"
RULES_DIR = REPO_ROOT / "monitoring" / "rules"
OBSERVABILITY_LIB = REPO_ROOT / "crates" / "chv-observability" / "src" / "lib.rs"

# Histogram exporter auto-emits these suffixes; strip before lookup.
HISTOGRAM_SUFFIXES = ("_bucket", "_count", "_sum")

# Metrics produced outside the Rust workspace or by infrastructure exporters.
# Adding a name here is a deliberate decision: it means "this gate cannot
# verify the emitter, trust the rule author".
EXTERNAL_METRIC_ALLOWLIST: set[str] = {
    # Prometheus self-monitoring + kube-state-metrics standard surface.
    "up",
    # node_exporter / kube-state metrics referenced indirectly.
    # (None used in chv.yml today; kept here as the documented extension point.)
}

# Labels that Prometheus / metrics-exporter-prometheus inject automatically
# and that emitter source code never sets explicitly. Treat them as always
# present for every metric.
IMPLICIT_LABELS: set[str] = {
    "le",  # histogram bucket boundary
    "quantile",  # summary quantile
    "job",  # scrape config
    "instance",  # scrape config
}

# ---------------------------------------------------------------------------
# Phase 1: parse emitter sources
# ---------------------------------------------------------------------------

# Match metric macro invocations. Captures everything between the opening
# paren and the matching close paren (we balance manually because labels
# can contain nested parens via .to_string() etc.).
METRIC_MACRO_RE = re.compile(
    r"metrics::(counter|histogram|gauge)!\s*\("
)

# String-literal metric name: "chv_foo_total"
NAME_LITERAL_RE = re.compile(r'"([A-Za-z_][A-Za-z0-9_]*)"')

# Label key=>value pairs: "label_key" => something
LABEL_KEY_RE = re.compile(r'"([A-Za-z_][A-Za-z0-9_]*)"\s*=>')


def _split_first_top_comma(s: str) -> tuple[str, str]:
    """Split `s` at the first comma that is not nested in (), [], {}, or "..".

    Used to isolate the first positional macro argument (the metric name)
    from the rest of the macro arguments. Returns (head, rest) where rest
    excludes the comma. If no top-level comma exists, returns (s, "").
    """
    depth = 0
    in_str = False
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if in_str:
            if c == "\\" and i + 1 < n:
                i += 2
                continue
            if c == '"':
                in_str = False
        else:
            if c == '"':
                in_str = True
            elif c in "([{":
                depth += 1
            elif c in ")]}":
                depth -= 1
            elif c == "," and depth == 0:
                return s[:i], s[i + 1 :]
        i += 1
    return s, ""


def load_observability_consts() -> dict[str, str]:
    """Resolve `pub const CHV_FOO: &str = "chv_foo";` definitions.

    Crates reference metric names via these consts (e.g.
    `chv_observability::CHV_VM_OPS_TOTAL`); we need to map them to the
    underlying string to compare against rule files.
    """
    consts: dict[str, str] = {}
    if not OBSERVABILITY_LIB.exists():
        return consts
    pattern = re.compile(
        r'pub\s+const\s+([A-Z][A-Z0-9_]+)\s*:\s*&str\s*=\s*"([^"]+)"\s*;'
    )
    for m in pattern.finditer(OBSERVABILITY_LIB.read_text()):
        consts[m.group(1)] = m.group(2)
    return consts


def find_balanced_paren(text: str, open_pos: int) -> int:
    """Return the index of the `)` matching the `(` at `open_pos`.

    Handles string literals correctly so quoted parens don't throw off the
    balance count. Returns -1 if unmatched (truncated source).
    """
    depth = 0
    i = open_pos
    in_str = False
    n = len(text)
    while i < n:
        c = text[i]
        if in_str:
            if c == "\\" and i + 1 < n:
                i += 2
                continue
            if c == '"':
                in_str = False
        else:
            if c == '"':
                in_str = True
            elif c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
                if depth == 0:
                    return i
        i += 1
    return -1


@dataclass
class EmitterSet:
    """Maps metric_name -> set of label keys observed in emitter code."""

    by_metric: dict[str, set[str]]
    # Track files where each metric was emitted, for diagnostics.
    sources: dict[str, list[str]]


def parse_emitters(consts: dict[str, str]) -> EmitterSet:
    by_metric: dict[str, set[str]] = defaultdict(set)
    sources: dict[str, list[str]] = defaultdict(list)

    for path in CRATES_DIR.rglob("*.rs"):
        # Skip auto-generated proto code and test fixtures that intentionally
        # emit shape-mismatched metrics.
        if "/gen/" in str(path) or path.name.endswith(".pb.rs"):
            continue
        text = path.read_text(errors="replace")
        for m in METRIC_MACRO_RE.finditer(text):
            open_paren = text.index("(", m.end() - 1)
            close_paren = find_balanced_paren(text, open_paren)
            if close_paren < 0:
                continue
            args = text[open_paren + 1 : close_paren]

            # The metric name is the first positional argument — everything
            # before the first comma at depth 0 (commas inside `Foo::Bar(x)`
            # do not count). Splitting on the first top-level comma isolates
            # `chv_observability::CHV_VM_OPS_TOTAL` from `"op" => self.op`.
            head, rest = _split_first_top_comma(args)
            head_clean = head.strip()
            name: str | None = None
            lit_head = NAME_LITERAL_RE.fullmatch(head_clean)
            if lit_head:
                name = lit_head.group(1)
            else:
                # Resolve const by short name. The full path may be qualified
                # (e.g. `chv_observability::CHV_VM_OPS_TOTAL`); take the last
                # `::` segment.
                short = head_clean.rsplit("::", 1)[-1]
                if short in consts:
                    name = consts[short]
                # Allow a literal that is concatenated/computed at compile
                # time but starts with a string — last fallback.
                elif (lit2 := NAME_LITERAL_RE.search(head_clean)) is not None:
                    name = lit2.group(1)
            if not name:
                # Dynamic name (e.g. `metrics::counter!(name)`); skip.
                continue

            labels = set(LABEL_KEY_RE.findall(rest))
            by_metric[name] |= labels
            rel = path.relative_to(REPO_ROOT)
            sources[name].append(str(rel))

    return EmitterSet(by_metric=dict(by_metric), sources=dict(sources))


# ---------------------------------------------------------------------------
# Phase 2: parse rule files
# ---------------------------------------------------------------------------

# Capture metric{label="value", other="x"} or metric{label!~"..."} forms.
RULE_REF_RE = re.compile(
    r'\b(chv_[A-Za-z0-9_]+)\s*\{([^}]*)\}'
)
# Inside the braces, find label keys (with =, !=, =~, !~ operators).
LABEL_OP_RE = re.compile(r'([A-Za-z_][A-Za-z0-9_]*)\s*(?:=~|!~|=|!=)')


@dataclass
class RuleRef:
    file: str
    line: int
    alert: str
    metric: str
    label: str


def parse_rules(rules_dir: Path) -> list[RuleRef]:
    refs: list[RuleRef] = []
    if not rules_dir.exists():
        return refs

    for path in rules_dir.rglob("*.yml"):
        text = path.read_text()
        # Track current alert name by scanning line-by-line.
        current_alert = "<unknown>"
        for lineno, line in enumerate(text.splitlines(), start=1):
            stripped = line.strip()
            if stripped.startswith("- alert:"):
                current_alert = stripped[len("- alert:") :].strip().strip('"')
            for m in RULE_REF_RE.finditer(line):
                metric = m.group(1)
                inner = m.group(2)
                for lab in LABEL_OP_RE.finditer(inner):
                    refs.append(
                        RuleRef(
                            file=str(path.relative_to(REPO_ROOT)),
                            line=lineno,
                            alert=current_alert,
                            metric=metric,
                            label=lab.group(1),
                        )
                    )
    return refs


# ---------------------------------------------------------------------------
# Phase 3: compare
# ---------------------------------------------------------------------------


def strip_histogram_suffix(name: str) -> str:
    for suf in HISTOGRAM_SUFFIXES:
        if name.endswith(suf):
            return name[: -len(suf)]
    return name


@dataclass
class Drift:
    ref: RuleRef
    reason: str
    known_labels: set[str]


def detect_drift(emitters: EmitterSet, refs: list[RuleRef]) -> list[Drift]:
    drifts: list[Drift] = []
    for ref in refs:
        base = strip_histogram_suffix(ref.metric)
        if base in EXTERNAL_METRIC_ALLOWLIST:
            continue

        # Find the emitter entry. A histogram emitted as `chv_foo_seconds`
        # produces `_bucket`/`_count`/`_sum` derived series; the labels on
        # the derived series mirror the histogram emitter's labels.
        emitter_labels: set[str] | None = None
        if base in emitters.by_metric:
            emitter_labels = emitters.by_metric[base]
        elif ref.metric in emitters.by_metric:
            emitter_labels = emitters.by_metric[ref.metric]

        if emitter_labels is None:
            drifts.append(
                Drift(
                    ref=ref,
                    reason=f"metric '{ref.metric}' has no emitter in crates/",
                    known_labels=set(),
                )
            )
            continue

        if ref.label in IMPLICIT_LABELS:
            continue
        if ref.label not in emitter_labels:
            drifts.append(
                Drift(
                    ref=ref,
                    reason=(
                        f"metric '{ref.metric}' is emitted but never with "
                        f"label key '{ref.label}'"
                    ),
                    known_labels=emitter_labels,
                )
            )
    return drifts


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    consts = load_observability_consts()
    emitters = parse_emitters(consts)
    refs = parse_rules(RULES_DIR)
    drifts = detect_drift(emitters, refs)

    if not drifts:
        sys.stdout.write(
            f"check-metric-labels: OK - {len(refs)} rule label-refs across "
            f"{len(emitters.by_metric)} emitted metrics align.\n"
        )
        return 0

    sys.stderr.write(
        f"check-metric-labels: DRIFT - {len(drifts)} alert label reference"
        f"{'s' if len(drifts) != 1 else ''} do not match any emitter.\n\n"
    )
    for d in drifts:
        known = (
            ", ".join(sorted(d.known_labels)) if d.known_labels else "<no emitter>"
        )
        sys.stderr.write(
            f"  {d.ref.file}:{d.ref.line}: alert \"{d.ref.alert}\"\n"
            f"    metric:  {d.ref.metric}\n"
            f"    label:   {d.ref.label}\n"
            f"    reason:  {d.reason}\n"
            f"    emitter: {known}\n\n"
        )
    sys.stderr.write(
        "Fix: either rename the alert label to match the emitter, or extend\n"
        "the emitter to set the label. Update docs/OPERATIONS.md examples in\n"
        "lockstep so the operator-facing surface stays consistent.\n"
    )
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
