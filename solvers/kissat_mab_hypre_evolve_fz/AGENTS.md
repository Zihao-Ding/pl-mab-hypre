# PROJECT KNOWLEDGE BASE

**Generated:** 2026-03-12
**Branch:** main

## OVERVIEW

Kissat SAT Solver — compact, competitive Boolean SAT solver in pure C (C99).

## STRUCTURE

```
./
├── src/           # Main solver (~207 .c/.h files)
├── test/          # Test suite (tissat framework)
├── build/         # Generated build artifacts
├── scripts/       # Build utility scripts
├── configure      # Custom configuration script
├── makefile       # Build wrapper (delegates to build/)
├── verify.c       # Standalone proof checker
└── drat-trim      # External proof trimming tool
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Main entry | `src/main.c` | `main()` → `kissat_application()` |
| Core solver | `src/internal.h` | Aggregates all solver components |
| Configuration | `./configure` | Custom shell script (not autotools) |
| Tests | `test/test.c` | tissat test runner |

## CONVENTIONS

- **C Standard**: C99 (strict in pedantic mode)
- **Formatting**: `clang-format` — run `make format` before commit
- **Build**: `./configure && make` — out-of-tree in `build/`
- **Configure flags**: `-g` (debug), `-c` (assertions), `-O[0-3]`, `--competition`, `--test`

## ANTI-PATTERNS (THIS PROJECT)

- No .clang-format config — uses clang-format directly
- No CI/CD pipeline (no GitHub Actions)
- Root makefile has hardcoded absolute paths

## UNIQUE STYLES

- Custom `ATTRIBUTE_ALWAYS_INLINE` compiler hints
- Test macros: `tissat_assert`, `tissat_assume`, `FATAL`
- Two TODO items: `src/vivify.c:731`, `src/application.c:872`

## COMMANDS

```bash
./configure --competition --test && make  # Build
make test                                   # Run tests
make format                                 # Format code
./build/kissat instance.cnf                # Solve SAT
./build/tissat -s -v                        # Verbose tests
```

## NOTES

- `src/makefile` and root `makefile` are symlinks to `build/makefile`
- Duplicate configure scripts at `./`, `./src/`, `./test/`
- Build products: `build/kissat`, `build/tissat`, `build/kitten`, `build/libkissat_mab_hypre_evolve_fz.a`