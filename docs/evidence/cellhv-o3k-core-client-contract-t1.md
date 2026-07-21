# O3K Core Client T1 Contract Registration Evidence

Status: Registered; not run

The T1-only O3K client profile was registered against
`https://github.com/kubedoio/o3k` revision
`53fd2cb36ee79f42da49c8181d6ceed12b41b3aa`. The source audit and passing O3K
unit/source-test record remain T0 evidence; see
`docs/evidence/20260721-o3k-source-tests.md`.

No O3K-to-CellHV Core integration was executed. There is currently no O3K-owned
Core client or injectable compute boundary at the pinned revision, so
`OCORE-002` through `OCORE-005` remain prerequisites-bound and not run. The
registration does not provide T5 evidence or any OpenStack compatibility claim.

Machine enforcement is provided by
`scripts/check-cellhv-core-architecture.py`, which fixes the source revision,
requires the executed-scenario list to remain empty, and rejects any O3K Core
scenario or profile labeled above T1.
