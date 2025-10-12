# Contributing to ryt

Thank you for your interest in contributing to ryt! This document provides guidelines and instructions for contributing.

## Code of Conduct

This project adheres to a [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## How Can I Contribute?

### Reporting Bugs

Before creating bug reports, please check existing issues to avoid duplicates. When creating a bug report, include:

- **Clear title and description**
- **Steps to reproduce** the issue
- **Expected behavior** vs **actual behavior**
- **Environment details** (OS, Rust version, etc.)
- **Relevant logs** or error messages

### Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. When creating an enhancement suggestion, include:

- **Clear title and description**
- **Use case** - why this enhancement would be useful
- **Possible implementation** - if you have ideas
- **Alternatives considered**

### Pull Requests

1. **Fork** the repository
2. **Create a branch** from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **Make your changes** following our coding standards
4. **Add tests** for new functionality
5. **Ensure all tests pass**:
   ```bash
   cargo test
   cargo clippy
   cargo fmt
   ```
6. **Commit your changes** following commit message guidelines
7. **Push to your fork** and submit a pull request

## Development Setup

### Prerequisites

- Rust 1.70 or higher
- Cargo (comes with Rust)

### Setup

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/ytget.git
cd ytget/ryt

# Build the project
cargo build

# Run tests
cargo test

# Run the application
cargo run -- "VIDEO_URL"
```

### Project Structure

```
ryt/
├── src/
│   ├── cli/         # CLI interface and argument parsing
│   ├── core/        # Core business logic
│   ├── platform/    # Platform API integration
│   ├── download/    # Download system
│   └── utils/       # Utility functions
├── tests/           # Integration tests
└── examples/        # Usage examples
```

## Coding Standards

### Rust Style Guide

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common mistakes
- Write idiomatic Rust code

### Code Quality

- **No panics** - Use `Result` for error handling
- **Avoid `any`/`interface{}`** equivalents
- **Minimize use of** `reflect`, `runtime`
- **Prefer explicit types** over generics where reasonable
- **Document public APIs** with doc comments

### Error Handling

- Use `thiserror` for custom error types
- Propagate errors to the caller when appropriate
- Use `anyhow` for application-level errors only
- Don't use `%w` format specifier (Go-style)

### String Concatenation

- Use `strings::Builder` pattern (or `String::push_str()`)
- For logging: use `format!()` for simple cases
- Avoid unnecessary string allocations

### Testing

- **Test-first approach** preferred
- Use table-driven tests where appropriate
- Mock external dependencies (database, network)
- Tests must be environment-independent
- All tests must pass before submitting PR

### Naming Conventions

- Types: `PascalCase`
- Functions/variables: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Request types: suffix with `Request` or `RQ`
- Response types: suffix with `Response` or `RS`

## Git Workflow

### Branch Naming

Format: `<type>/<short-description>`

Types:
- `feature/` - New features
- `fix/` - Bug fixes
- `refactor/` - Code refactoring
- `docs/` - Documentation changes
- `test/` - Test additions or changes
- `chore/` - Maintenance tasks

Example: `feature/add-playlist-support`

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code refactoring
- `docs` - Documentation
- `test` - Tests
- `chore` - Maintenance
- `style` - Code style changes

**Examples:**

```
feat(download): add rate limiting support

Implements configurable download rate limiting using the governor crate.
Adds --rate-limit CLI option.

Closes #42
```

```
fix(platform): handle missing thumbnail URLs

Some videos don't have all thumbnail sizes available.
Adds fallback to lower resolution thumbnails.
```

### Pull Request Process

1. **Update documentation** if needed
2. **Add tests** for new functionality
3. **Update CHANGELOG.md** with your changes
4. **Ensure CI passes** on your PR
5. **Request review** from maintainers
6. **Address feedback** promptly
7. **Squash commits** before merge if requested

## Testing Guidelines

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration

# Run with coverage
cargo tarpaulin --out Html
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Arrange
        let input = create_test_input();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn test_async_feature() {
        // Test async code
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

## Documentation

- Add doc comments to public APIs:
  ```rust
  /// Brief description
  ///
  /// # Arguments
  ///
  /// * `arg` - Argument description
  ///
  /// # Returns
  ///
  /// Return value description
  ///
  /// # Examples
  ///
  /// ```
  /// let result = function(arg);
  /// ```
  pub fn function(arg: Type) -> Result<ReturnType> {
      // ...
  }
  ```

- Update README.md for user-facing changes
- Add examples in `examples/` for new features

## Release Process

Maintainers will handle releases, but contributors should:

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Tag the release following semver

## Questions?

Feel free to:
- Open an issue for discussion
- Join our discussions
- Reach out to maintainers

Thank you for contributing to ryt! 🎉

