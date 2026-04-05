# Contributing to CHV

Thank you for your interest in contributing to CHV! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Commit Messages](#commit-messages)
- [Pull Request Process](#pull-request-process)

## Code of Conduct

This project adheres to a code of conduct that we expect all contributors to follow:

- Be respectful and inclusive in your communication
- Welcome newcomers and help them get started
- Focus on constructive feedback rather than criticism
- Respect different viewpoints and experiences

## Getting Started

### Prerequisites

- Go 1.22 or later
- Docker and Docker Compose
- PostgreSQL 16 (for local development)
- Linux host with KVM support (for testing VM operations)

### Setting Up Your Development Environment

1. **Fork and clone the repository:**
```bash
git clone https://github.com/yourusername/chv.git
cd chv
```

2. **Install dependencies:**
```bash
go mod download
```

3. **Set up the development environment:**
```bash
make docker-up
```

4. **Verify your setup:**
```bash
make test
```

## Development Workflow

### Branch Naming

- `feature/description` - New features
- `bugfix/description` - Bug fixes
- `docs/description` - Documentation updates
- `refactor/description` - Code refactoring
- `security/description` - Security fixes

Example: `feature/add-vm-console-access`

### Making Changes

1. Create a new branch from `master`:
```bash
git checkout -b feature/your-feature-name
```

2. Make your changes following our [coding standards](#coding-standards)

3. Write or update tests as needed

4. Run the test suite:
```bash
make test
make test-race  # Run with race detector
```

5. Update documentation if applicable

## Coding Standards

### Go Code Style

We follow standard Go conventions:

- Use `gofmt` to format your code
- Use `go vet` to check for issues
- Follow [Effective Go](https://golang.org/doc/effective_go) guidelines
- Use meaningful variable and function names
- Keep functions focused and concise

### Project-Specific Conventions

#### Error Handling

Use the `pkg/errorsx` package for structured errors:

```go
import "github.com/chv/chv/pkg/errorsx"

// Create a new error
err := errorsx.New(errorsx.ErrNotFound, "VM not found")

// Wrap an existing error
err := errorsx.Wrap(err, errorsx.ErrInternal, "failed to start VM")
```

#### Logging

Use the `pkg/logger` package for structured logging:

```go
import "github.com/chv/chv/pkg/logger"

log := logger.New(logger.InfoLevel)
log.Info("Starting VM", logger.Field{Key: "vm_id", Value: vmID})
```

#### Context Usage

Always accept `context.Context` as the first parameter for functions that:
- Make database queries
- Call external services
- Perform long-running operations

```go
func (s *Service) DoSomething(ctx context.Context, param string) error {
    ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
    defer cancel()
    // ...
}
```

### Testing Standards

#### Unit Tests

- Write table-driven tests where appropriate
- Use meaningful test names that describe the behavior
- Mock external dependencies
- Aim for >80% code coverage for new code

Example:
```go
func TestValidateSafeForPath(t *testing.T) {
    tests := []struct {
        name    string
        id      string
        wantErr bool
    }{
        {"valid UUID", "550e8400-e29b-41d4-a716-446655440000", false},
        {"path traversal", "../../../etc/passwd", true},
    }
    
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            err := ValidateSafeForPath(tt.id)
            if (err != nil) != tt.wantErr {
                t.Errorf("ValidateSafeForPath() error = %v, wantErr %v", err, tt.wantErr)
            }
        })
    }
}
```

#### Race Detection

Always run tests with the race detector:

```bash
go test -race ./...
```

## Documentation

### Code Documentation

- Document all exported functions, types, and constants
- Use complete sentences with proper punctuation
- Provide examples for complex functions

```go
// ValidateSafeForPath validates that a VM ID is safe to use in file paths.
// It checks that the ID:
//   - Is a valid UUID (36 chars, standard format)
//   - Does not contain path separators (/ or \)
//   - Does not contain path traversal sequences (..)
//
// This prevents path traversal attacks where malicious IDs like
// "../../../etc/passwd" could escape intended directories.
func ValidateSafeForPath(id string) error {
    // ...
}
```

### Architecture Documentation

For significant architectural changes:

1. Create an Architecture Decision Record (ADR) in `docs/adr/`
2. Update relevant documentation in `docs/`
3. Update the README if user-facing changes

## Commit Messages

Follow conventional commit format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, no logic changes)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Build process, dependencies, etc.
- `security`: Security-related changes

### Examples

```
feat(scheduler): add BestFit placement strategy

Implement the BestFit scheduling strategy that selects the node
with the least available capacity that can still fit the VM.

Closes #123
```

```
fix(launcher): prevent race condition in StopVM

Add sync.Once to VMInstance to ensure cleanup happens exactly once,
preventing race conditions between StopVM and waitForProcessExit.

Fixes #456
```

## Pull Request Process

### Before Submitting

1. **Run all tests:**
```bash
make test
make test-race
```

2. **Check code formatting:**
```bash
gofmt -d .
```

3. **Run linters:**
```bash
golangci-lint run
```

4. **Update documentation** if needed

### PR Description

Your PR description should include:

1. **What** - What changes are being made
2. **Why** - Why are these changes necessary
3. **How** - How were the changes implemented
4. **Testing** - How were the changes tested

Example template:
```markdown
## Description
Add path traversal protection for VM IDs to prevent security vulnerabilities.

## Changes
- Add `ValidateSafeForPath()` function in `pkg/uuidx`
- Add validation to API handlers and launcher
- Add comprehensive unit tests

## Testing
- Unit tests added with 24 test cases covering various attack vectors
- All tests pass with race detector
- Manual testing with malicious input confirmed protection works

## Security Impact
This prevents path traversal attacks where malicious VM IDs could
escape intended directories.
```

### Review Process

1. All PRs require at least one review from a maintainer
2. All CI checks must pass
3. Address review feedback promptly
4. Keep PRs focused and reasonably sized (< 500 lines preferred)

### After Merge

- Delete your feature branch
- Update related issues
- Monitor for any issues in production

## Release Process

Releases are managed by maintainers:

1. Version follows [Semantic Versioning](https://semver.org/)
2. Update `CHANGELOG.md` with release notes
3. Create a git tag: `git tag -a v0.1.0 -m "Release v0.1.0"`
4. Push the tag: `git push origin v0.1.0`

## Getting Help

- **Discord/Slack**: [Join our community chat]
- **Issues**: Create a GitHub issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions

## Recognition

Contributors will be recognized in our release notes and CONTRIBUTORS file.

Thank you for contributing to CHV!
