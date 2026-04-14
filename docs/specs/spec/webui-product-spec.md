# WebUI Product Spec

## Goal
Deliver a virtualization management UI that is better than incumbent operator experiences by being:
- clearer
- more task-transparent
- more state-legible
- more consistent across cluster, node, and VM workflows

## Primary users
- infra/platform operators
- SRE / virtualization admins
- small private-cloud teams
- managed infrastructure operators

## MVP-1 user goals
1. see cluster and node health quickly
2. create and manage VMs reliably
3. inspect storage and network attachments
4. track every operation and error
5. put nodes into maintenance or drain states
6. understand why a placement or action failed

## Non-goals
- consumer-facing self-service portal
- advanced billing / quota product
- visual network topology engine
- polished multi-tenant self-service UX in MVP-1

## Core product surfaces
- fleet overview dashboard
- node list + node detail
- VM list + VM detail
- volume list + volume detail
- network list + network detail
- tasks center
- events and alerts center
- maintenance and upgrade center

## MVP-1 must-have actions
- create VM
- start VM
- stop VM
- reboot VM
- delete VM
- attach volume
- detach volume
- resize volume
- enter maintenance
- exit maintenance
- pause/resume scheduling
- drain node

## Product rules
- every mutation produces a task
- every task is linkable to its target resource
- every resource page exposes recent related tasks and events
- stale or degraded state must be obvious
