# Contributing to Wesichain

Thank you for your interest in contributing to Wesichain! This document provides guidelines for contributing to the project.

## Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code.

## How to Contribute

### Reporting Bugs

- Check if the issue already exists
- Provide a clear description
- Include steps to reproduce
- Specify your environment (Rust version, OS)

### Suggesting Features

- Open an issue with the `enhancement` label
- Describe the use case
- Explain why existing solutions don't work

### Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Run formatting (`cargo fmt`)
6. Run linting (`cargo clippy`)
7. Commit with conventional commit format
8. Push to your fork
9. Open a Pull Request

## Development Setup

```bash
git clone https://github.com/wesichain/wesichain.git
cd wesichain/wesichain
cargo build
cargo test
```

## Commit Convention

We follow Conventional Commits:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `style:` Code style (formatting)
- `refactor:` Code refactoring
- `test:` Adding tests
- `chore:` Maintenance tasks

Example: `feat(agent): add streaming support for tool calls`

## Testing

- Write unit tests for new functionality
- Ensure all tests pass before submitting PR
- Include integration tests where appropriate

## Questions?

Feel free to open an issue for questions or join discussions.
