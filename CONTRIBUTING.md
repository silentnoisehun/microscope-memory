# Contributing to Microscope Memory

Thank you for your interest in contributing to Microscope Memory! We welcome contributions from everyone.

## How to Contribute

### Reporting Issues

- Check if the issue already exists
- Include system information (OS, Rust version)
- Provide minimal reproduction steps
- Include relevant logs or error messages

### Submitting Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass (`cargo test`)
6. Run clippy (`cargo clippy -- -D warnings`)
7. Format your code (`cargo fmt`)
8. Commit with descriptive messages
9. Push to your fork
10. Open a Pull Request

### Code Style

- Follow Rust naming conventions
- Use meaningful variable names
- Add comments for complex logic
- Keep functions small and focused
- Document public APIs

### Testing

- Write unit tests for new functions
- Add integration tests for new features
- Ensure benchmarks still run
- Test on multiple platforms if possible

### Areas of Interest

We're particularly interested in contributions for:

- **Performance optimizations**: SIMD, parallel processing
- **New distance metrics**: Beyond L2 (cosine, Manhattan)
- **Compression**: Block compression algorithms
- **Visualization**: Tools to visualize the 3D memory space
- **Language bindings**: Python, JavaScript, Go
- **Real-time updates**: Incremental index updates
- **Distributed mode**: Sharding across machines

## Development Setup

```bash
# Clone your fork
git clone https://github.com/yourusername/microscope-memory.git
cd microscope-memory

# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench
```

## Questions?

Feel free to open an issue for any questions about contributing!