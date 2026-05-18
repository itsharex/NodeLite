# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- New features and improvements

### Changed
- Changes to existing functionality

### Deprecated
- Soon-to-be removed features

### Removed
- Now removed features

### Fixed
- Bug fixes

### Security
- Vulnerability fixes

## [v2.1.0] - 2026-05-18

### Added
- Added Argon2id hash/verify helpers for node session tokens.
- Added `token_hash` and `token_generation` fields to registered nodes while keeping legacy `token` reads for upgrade compatibility.
- Added hash-at-rest coverage, including a migration test for legacy plaintext registry tokens.
- Added graceful WebSocket shutdown handling so active sessions receive a Close frame during server shutdown.

### Changed
- Node authorization now verifies plaintext tokens only during handshake, then tracks `token_generation` for the WebSocket hot path.
- Token refresh now rotates the token hash and increments generation, allowing existing sessions to detect registry-side token changes without repeated hash verification.
- Install sessions now temporarily hold the plaintext node session token only for the bootstrap flow; generated agent config receives that plaintext explicitly.
- History writes now use a bounded channel and batched writer task to avoid blocking the realtime WebSocket path.
- Server tuning constants for timeouts, ping limits, SQLite busy timeout, sanitization limits, and warning intervals are now configurable.

### Fixed
- Legacy registry files with plaintext node `token` values are automatically migrated on load: the token is hashed into `token_hash`, the plaintext field is cleared, `token_generation` is initialized, and the upgraded registry is persisted back to disk.
- WebSocket handling now keeps token refresh generation in sync after manual and pre-expiry refreshes.

### Security
- Node session tokens are no longer persisted in plaintext for new or migrated registry entries.
- Readonly Basic Auth comparison uses constant-time matching.
- Systemd service hardening now includes a tighter syscall filter.

## [v2.0.7] - 2026-05-18

### Added
- Prometheus metrics endpoint (`/metrics`) for monitoring integration
- Comprehensive test coverage for token refresh concurrency and expiry boundaries
- Property-based testing for sanitize and registry modules

### Fixed
- Registry file lock cleanup panics are now properly handled
- Server update temp script path hardened for security

## [v2.0.6] - 2026-05-17

### Changed
- Code formatting improvements with rustfmt

## [v2.0.5] - 2026-05-16

### Added
- API response caching for dashboard payloads
- Dashboard brand logo asset optimization

### Fixed
- Default node token lifetime shortened for security
- Upstream refresh errors hidden from clients
- Valid 2FA sessions preserved after mutex poison
- POSIX shell compatibility for scripts
- Dashboard first paint performance improved

## [v2.0.4] - 2026-05-15

### Added
- CONTRIBUTING.md guide for contributors
- CLAUDE.md for AI-assisted development
- GitHub issue templates
- Root MIT license file

### Changed
- Module comments converted to rustdoc
- Code formatting improvements

### Security
- Password validation requirements strengthened

## [2.0.3] - 2026-05-14

### Added
- Initial stable release features

## [2.0.2] - 2026-05-13

### Added
- Bug fixes and improvements

## [2.0.1] - 2026-05-12

### Added
- Initial open source release
- Server-Agent architecture
- WebSocket real-time communication
- Token authentication with TOTP 2FA
- SQLite historical data storage
- Lightweight monitoring (server: 4-10MB, agent: 800KB)

### Changed
- Main entry point refactored to thin entrypoint
- Settings module split for better organization

## [2.0.0] - 2026-05-11

### Added
- Complete rewrite with modern architecture
- New configuration format
- Enhanced security features

## [1.2.27] - 2026-05-10

### Fixed
- Various bug fixes and improvements

## [1.2.26] - 2026-05-09

### Fixed
- Stability improvements

## [1.2.25] - 2026-05-08

### Fixed
- Performance optimizations

## [1.2.24] - 2026-05-07

### Fixed
- Security enhancements

## [1.2.23] - 2026-05-06

### Fixed
- UI improvements

## [1.2.22] - 2026-05-05

### Fixed
- Configuration handling improvements

## [1.2.21] - 2026-05-04

### Fixed
- WebSocket connection stability

## [1.2.20] - 2026-05-03

### Fixed
- Authentication flow improvements

## [1.2.19] - 2026-05-02

### Fixed
- Data collection reliability

## [1.2.18] - 2026-05-01

### Fixed
- Memory usage optimizations

## [1.2.17] - 2026-04-30

### Fixed
- Logging improvements

## [1.2.16] - 2026-04-29

### Fixed
- Initial stable release

## [1.1.0] - 2026-04-28

### Added
- Multi-node support
- Historical data storage

## [1.0.8] - 2026-04-27

### Fixed
- Bug fixes and stability improvements

## [1.0.7] - 2026-04-26

### Fixed
- Performance optimizations

## [1.0.6] - 2026-04-25

### Fixed
- Security enhancements

## [1.0.5] - 2026-04-24

### Fixed
- UI improvements

## [1.0.4] - 2026-04-23

### Fixed
- Configuration handling

## [1.0.3] - 2026-04-22

### Fixed
- WebSocket stability

## [1.0.2] - 2026-04-21

### Fixed
- Authentication improvements

## [1.0.1] - 2026-04-20

### Fixed
- Initial bug fixes

## [1.0.0] - 2026-04-19

### Added
- Initial release
- Basic monitoring functionality
- Agent-server architecture
- Real-time data visualization
