# Changelog

All notable changes to this project will be documented in this file.

This changelog contains the most important changes to VeriPB from version to version. Due to being a prototype implementation and the active development of the proof system, there will be breaking changes. If there are breaking changes, they will be mentioned at the top for of each version.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [3.0.2](https://gitlab.com/MIAOresearch/software/veripb/compare/3.0.1...3.0.2) - 2026-03-20

### New features
- New cutting planes rules
  - `-` rule to lower the RHS of a constraint by a positive integer (e.g., `pol 42 3 -;` lowers RHS of constraint `42` by 3)
  - `c` rule for division in variable form, i.e., all literals are positive and negative coefficients are allowed (similar to Chvátal-Gomory cut)
  - `n` rule for mixed integer rounding (MIR) cut on constraints in normalized form (e.g., `pol 42 3 n;` for MIR cut of constraint `42` with divisor 3)
  - `m` rule for mixed integer rounding (MIR) cut on constraints in variable form
- Tracking of basic running time statistic through `--stats`
- Migration guide from version 2 to version 3 of the proof format
- VeriPB version 3 grammar documentation
- Faster cutting planes checking by merging weakening steps and additions of literal axioms
- Autoproving using syntactic implication using constraints substituted with the assignment propagated at level zero

### Changes
- Allow hyphens in variable names
- Seed hash maps and hash sets for reproducibility
- Disallow `conclusion UNSAT` if solution is logged

### Commits

- Change database proofgoal elaboration wrt original formula
- Change to seed HashSet for unique hashed constraints
- Fix appending IDs for duplicate constraint IDs
- Fix autoproving heuristic to do top-level RUP check
- Add threshold to elaboration of database proofgoals
- Fix right implication is conjunction of literals
- Add more links to migration guide.
- Streamline grammar document and migration guide.
- Change to use top level heuristic if witness is empty for checked
- Fix disallowing proofgoals for checked deletion under
- Fix elaboration of strengthening-to-core checked deletion
- Fix emptying derived set when enabling strengthening to core
- Change solution logging to no require all variables assigned
- Change parser to output correct error message for pol integer 0
- Make clippy happy.
- Change to use seconds for time and print decimals
- Add a very basic time stat.
- Change general PB constraints watches to HashSet
- Update CHANGELOG.md with new cutting planes rules
- Disable Windows and MacOS CI testing
- Add variable for MIR cut rule
- Add normalized form MIR cut rule
- Add variable form cutting planes division
- Add version 3 migration guide
- Add rule to lower RHS of constraint
- Add elaboration for autoproving using substituted implication
- Add autoproving using substituted syntactic implication
- Change to determine unique touched constraints before substitution
- Change to determine touched constraints before substitution
- Remove elaboration of trivial and negated constraint implied proofgoals
- Add back integration tests using CakePB
- Remove elaboartion of implication to cutting planes
- Fix seed for hash set used to detect unique constraints
- Fix formatting
- Fix to disallow conclusion UNSAT if solution logged
- Fix resetting elaborator ID after failed checked deletion
- Fix name of function checking assignment independent propagation
- Fix lazily adding constraint to propagator and trail saving
- Fix typo in error message for wrong conclusion
- Change autoproving propagator to propagate in first position
- Fix checking upper bound value conclusion with respect to original
- Change .gitlab-ci.yml to only run MacOS tests after merge
- Add documentation and comments to the verifier functions
- first check for contradiction for efficiency reasons
- optimize consecutive weakenings
- add documentation for cutting planes optimization
- make clippy and format happy
- merge together literal axiom additions
- Allow hyphens in variable names and require space around '->' in witnesses.
- Refactor move constraint to core propagation engine.
- Fix indexing error when a constraint was in the derived propagation set, then rup'ed (to make a copy) and the copy was moved to core which did not move the propagation constraint to the core propagator.

## [3.0.1](https://gitlab.com/MIAOresearch/software/veripb/compare/3.0.0...3.0.1) - 2025-11-02

### New Features
- Coloured and highlighted error messages
- Tracing for failed annotated RUP
- Tracing for proof by contradiction
- Optional objective value for sanity check with solution logging
- Check that canonical paths of derivation file and elaboration file are different

### Changes
- Improve trail saving and propagation engine

### Bug Fixes
- Syntactic implication check and elaboration
- Printing version number
- Parsing of OPB implications
- Disabling autoproving after failed checked deletion
- Elaboration of redundance-based strengthening with empty witness to proof by contradiction correctly

### Commits

- Fix resetting trail to respect core only reset
- Change to give useful errors when formula or derivation does not exist
- Fix canonicalization check if output proof is a new file
- Rename implies to implies_weak and implies_strong to implies
- Check canonical file paths are not the same.
- Add optional objective value to sol function
- Fix elaboration of redundance with empty witness to correct pbc rule
- Fix disabling autoproving propagation set after failed checked deletion
- Change constraint to remembers if it is in a saved reason
- Add tracing failed RUP for proof by contradiction
- Add tracing failed annotated RUP steps
- Fix parsing of OPB implications
- Add colouring of error messages
- Fix version number printing


## 3.0.0
This is the first release of VeriPB rewritten in Rust. This version should be feature-equal to version 2.3.0, except for labels.

### New
- Proof format version 3.0 parsing. New features are only supported in version 3.0.
- `obji` rule to introduce objective improving constraint from objective value.
- `soli` admits an optional objective value hint, which is used as a sanity check that the solution logged has the same objective value as the hint.
- Orders are now allowed have a specification section and can use auxiliary variables.
  - The specification is defined by a subproof that can use redundance-based strengthening by witnessing over auxiliary variables.
  - The specification can be used in redundance- and dominance-based strengthening subproofs by using the `scope` command.

### Dropped
- Parsing support for version 1.x files.
- `f` rule without a number of constraints.

---
***The version from here on are the old version of VeriPB written in Python and C++. The old version is available in the branch `version2`.***

## Version 2.3.0
Commit hash: 9dbb658ffb8d88815c3f3d8ff7fcefd6c6fc43db
### New
- Added optional constraint labels to refer to constraints in the proof instead of constraint IDs. Labels have to start with the `@` character and can be assigned to any constraint in the input OPB file or the proof.
- Added printing constraint label definition to trace.
### Changes
- Changed autoproving trace to mention that the database has been substituted for the implication check.
- Removed checked deletion flag check for checking the upper bound at the end of the proof.
- Improved performance for checking full solutions
### Fixes
- Fixed annotated RUP for using relative constraint IDs as hints.
- Fixed using relative constraint IDs for `deld`.
- Fixed bug not deleting duplicate constraints when using `deld`.
- Fixed elaboration of syntactic implication to derive the desired constraint
## Version 2.2.2 (PB competition 2024)
Commit hash: 765fa91e60f120d51c079a5fa83f664ff69d7d17
### New
- Added user-defined timers for custom statistics on tracking the time requires checking certain proof sections.
- Added elaboration for substituted database implication autoproving check.
### Bug Fixes
- Fixed circular import bug and provide instructions to avoid this bug.
- Fixed elaboration of autoproving by implication.
## Version 2.2.1
Commit hash: 62f78fd47b41ad28fd47df626a14491a43b14cc1
### New
- Added printing conclusion and output verification statements at the end of checking.
### Changes
- Changed to implicitly add negated constraints to annotated RUP hints.
- Removed legacy code setting unassigned variables after propagation to true.
- Improved `f` rule error message.
- Improved error messages for lower bound check.
- Changed to print `Verification failed` error message for more errors.
### Bug Fixes
- Fixed parsing bug when there is no space between semicolon and last witness mapping.
- Fixed erroneous on/off in README.md.
- Fixed elaboration of checked deletion using RUP with duplicate constraints.
- Fixed elaboration of RUP autoproving when using negated constraint.
- Fixed elaboration bug reusing constraint ID of negated constraint.
- Fixed elaboration bug when unloading order while no order is loaded.
- Fixed elaboration of implication to exactly derived the desired constraint.
- Fixed to check solution only for variables still occurring in the database.
- Fixed `conclusion BOUNDS` check when claiming solution and infeasible at the same time.
- Fixed elaboration for checked deletion RUP check.
- Fixed default VeriPB version when compiling outside repository.
## Version 2.2.0 (SAT competition 2024)
Commit hash: aa8fe7380af9e2ec7776b707f708db048e8fb3dd
### Breaking Changes
- The rules `e`, `ea`, `i`, and `ia` have a changed syntax. E.g., for the `e` rule the new syntax is
```
e <constraint in OPB syntax> ; [<constraint ID>]
```

### New
- Annotated RUP: The `rup` rule can now take a list of constraints as hints. The propagation is then only performed on that list. The negated constraint is denoted by `~`. The syntax for this rule is
```
rup <constraint in OPB syntax> ; [<list of IDs of propagated constraint>]
```
