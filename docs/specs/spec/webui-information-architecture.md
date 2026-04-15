# WebUI Information Architecture

## Top-level navigation

1. Overview
2. Datacenters / Clusters
3. Nodes
4. Virtual Machines
5. Volumes
6. Networks
7. Images / Templates
8. Tasks
9. Events / Alerts
10. Maintenance / Upgrades
11. Settings / Access

## Overview page
Shows:
- fleet health summary
- node readiness summary
- VM status summary
- capacity summary
- active alerts
- recent tasks
- recent failures
- maintenance windows in effect

## Nodes page
List columns:
- node name
- cluster
- state
- CPU usage
- memory usage
- storage summary
- network health
- version
- maintenance state

Node detail tabs:
- Summary
- Virtual Machines
- Volumes
- Networks
- Tasks
- Events
- Configuration

## Virtual Machines page
List columns:
- VM name
- node
- power state
- health
- CPU
- memory
- storage count
- network count
- tags
- last task

VM detail tabs:
- Summary
- Console / Access
- Volumes
- Networks
- Configuration
- Tasks
- Events

## Volumes page
List columns:
- volume name
- backend class
- attached VM
- health
- size
- policy
- node
- last task

## Networks page
List columns:
- network name
- scope
- health
- attached VMs
- public exposure
- last task

## Tasks page
Filters:
- resource type
- status
- time window
- operation type
- node
- actor

## Events / Alerts page
Filters:
- severity
- resource
- state
- acknowledged/unacknowledged
