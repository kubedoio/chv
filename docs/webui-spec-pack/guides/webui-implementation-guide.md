# WebUI Implementation Guide

## Goal
Implement a CHV WebUI that feels better than incumbent virtualization UIs by being cleaner, more task-transparent, and more state-legible.

## Recommended order
1. read ADRs
2. implement BFF/API contracts
3. build app shell and navigation
4. implement overview and task center
5. implement nodes and VMs
6. implement mutation flows
7. add volumes, networks, and maintenance

## LLM workflow
For each implementation chat:
- paste relevant ADRs
- paste the specific spec file
- paste the relevant proto
- make the model do analysis first
- approve file map and phases
- then let it implement one vertical slice only

## First slices
- app shell + navigation
- overview page with mock/stubbed BFF
- tasks center
- nodes list/detail
- VMs list/detail
