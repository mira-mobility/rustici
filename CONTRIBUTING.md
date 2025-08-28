# Contributing to rustici

Thank you for your interest in contributing to **rustici**! 🎉  
We welcome all forms of contributions, whether it is a bug report, documentation improvement, feature request, or code.  
This guide outlines the process to help you get started.

---

## 📜 Code of Conduct

We are committed to fostering a welcoming, inclusive, and respectful community. Please:

- Be respectful and considerate in all interactions
- Provide constructive criticism and helpful feedback
- Respect differing viewpoints and experiences

---

## 🚀 How Can I Contribute?

### Reporting Issues

Before opening a new issue:

1. Search existing issues to avoid duplicates
2. Use the provided templates when possible
3. Include **relevant details**:
   - Rust version (`rustc --version`)
   - Operating system and environment
   - Steps to reproduce
   - Expected vs. actual behavior
   - Error messages/logs

### Suggesting Enhancements

- Use the **feature request template**
- Clearly explain your use case and motivation
- Ensure it aligns with the project’s **minimal and focused approach**

### Contributing Code

We welcome contributions that:

- Fix bugs 🐛
- Add or improve tests 🧪
- Improve documentation 📖
- Introduce new features aligned with project goals ✨
- Enhance performance or code quality ⚡

---

## 🛠 Development Setup

```bash
# Clone the repository
git clone https://github.com/mira-mobility/rustici.git
cd rustici

# Build the project
cargo build

# Run tests
cargo test
cargo test --all-features

# Check formatting
cargo fmt --all -- --check

# Run lints
cargo clippy --all-targets --all-features -- -D warnings

# Build documentation
cargo doc --open
```

---

## 📏 Development Guidelines

### Code Standards

- **No unsafe code** → enforced with `#![forbid(unsafe_code)]`
- **Documentation required** → enforced with `#![deny(missing_docs)]`
- **Style** → Run `cargo fmt` before committing
- **Lint clean** → Pass `cargo clippy` with zero warnings
- **Test coverage** → Add tests for all new functionality

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): short description

Longer explanation if needed.

Fixes #123
```

**Types:** `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`, `ci`

### Testing

- **Unit tests** → co-located in the same file under `#[cfg(test)]`
- **Integration tests** → in `tests/`
- **Examples** → in `examples/` (runnable via `cargo run --example <name>`)
- Ensure tests pass on **stable**, **beta**, and **nightly**

### Documentation

- All public items must have rustdoc comments
- Add runnable examples where helpful (` /// ```rust `)
- Update **README.md** for significant changes
- Keep comments concise but informative

---

## 🔀 Pull Request Process

1. **Fork and branch** → branch from `main`
2. **Implement changes** → follow guidelines above
3. **Test thoroughly** → run full test suite
4. **Update docs** → including README if needed
5. **Open PR** → use the PR template
6. **CI checks must pass** ✅
7. **Code review** → address feedback constructively
8. **Squash if requested** → keep history clean

### ✅ PR Checklist

- [ ] Code formatted & linted (`cargo fmt`, `cargo clippy`)
- [ ] Tests added/updated and passing
- [ ] Documentation updated
- [ ] No new warnings introduced
- [ ] Clear commit messages (conventional commits)
- [ ] PR description explains the change

---

## 🏗 Project Architecture

### Key Components

- `src/wire.rs` → Message encoding/decoding (sections, lists, key-values)
- `src/packet.rs` → Packet layer definitions
- `src/client.rs` → Synchronous UNIX socket client
- `src/error.rs` → Error types

### Design Principles

1. **Minimal dependencies** → Avoid external dependencies
2. **Pure Rust** → no FFI
3. **Blocking I/O** → synchronous by design
4. **UNIX-only** → targets UNIX domain sockets
5. **Protocol-focused** → implements wire protocol, not abstractions

---

## ⚖️ Legal

### License

This project is licensed under **LGPL-2.1+**.  
By contributing, you agree that:

- Your contributions are licensed under the same terms
- You have the right to submit the work
- You understand the implications of LGPL licensing

### Sign-off (Optional)

You may sign off commits with:

```
Signed-off-by: Your Name <your.email@example.com>
```

This indicates agreement with the [Developer Certificate of Origin](https://developercertificate.org/).

---

## 🙋 Need Help?

- Review existing documentation and examples
- Check closed issues for similar discussions
- Open a **discussion issue** if you’re unsure about an approach
- Contact maintainers (see README)

---

## 🌟 Recognition

- Contributors are acknowledged in **release notes**
- Significant contributions may be highlighted in the **README**

Thank you for helping make **rustici** better!
