# Changelog

All notable changes to this project will be documented in this file.

## [0.0.0.1] - 2026-04-10

### Changed
- Simplified docker-compose configurations by removing agent service (runs on bare-metal hosts)
- Changed controller port mapping from 8080:8080 to 8088:8080 to avoid conflicts
- Removed agent dependency from controller service
