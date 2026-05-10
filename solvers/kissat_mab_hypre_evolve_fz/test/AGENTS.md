# test/ — Test Suite (tissat)

**47 test files + test data subdirs.**

## WHERE TO LOOK

| Component | Files | Purpose |
|-----------|-------|---------|
| **Test runner** | `test.c` | Main tissat executable |
| **Test framework** | `test.h` | Macros: `tissat_assert`, `tissat_assume`, `FATAL` |
| **Scheduler** | `testscheduler.h` | Parallel job execution |
| **Unit tests** | `testheap.c`, `testvector.c`, `testparse.c` | Component tests |
| **Data dirs** | `cnf/`, `parse/`, `cover/`, `file/` | Test CNF instances |

## CONVENTIONS

- Test functions: `test_<component>_<name>()`
- Macro: `DECLARE_AND_INIT_SOLVER(SOLVER)` — creates test instance
- Run: `./build/tissat` or `make test`
- Options: `-v` (verbose), `-s` (sequential), `-j[N]` (parallel jobs)

## BUILD TEST

```bash
./configure --test && make test
./build/tissat -s -v           # Verbose sequential tests
./build/tissat heap            # Run specific test
```