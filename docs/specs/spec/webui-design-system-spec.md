# WebUI Design System Spec

## Design goals
- fast scanning
- low ambiguity
- serious operational tone
- excellent table/detail workflows
- clear action states and warnings

## Visual principles
- light mode first
- off-white background preferred over pure white
- restrained accent color usage
- sharp border hierarchy
- low visual clutter
- no decorative glow-heavy admin style

## Core components
- app shell
- left navigation
- top command bar
- section header
- summary cards
- metric tiles
- data tables
- filter bars
- side panels / drawers
- modal confirmation dialogs
- task timeline
- event timeline
- status badges
- inline banners
- empty states
- skeleton loading states

## Status color semantics
- healthy
- warning
- degraded
- failed
- unknown

## Important interaction rules
- destructive actions need explicit confirmation
- long-running actions open task references immediately
- filters persist per page where sensible
- bulk actions are gated carefully
- no silent success on asynchronous actions
