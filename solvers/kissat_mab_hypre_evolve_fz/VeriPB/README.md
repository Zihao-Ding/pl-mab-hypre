# VeriPB - Verifier for Pseudo-Boolean Proofs

VeriPB is a tool for verifying pseudo-Boolean certificates of satisfiability, unsatisfiability, and optimality bounds written in Rust. For the old version of VeriPB, check out the branch `version2`.

A quick overview of the proof file format can be found in [proof_format_overview.md](proof_format_overview.md).

A detailed documentation of the proof checker VeriPB and the formally verified proof checker CakePB as submitted to the SAT competition 2025 can be found on the [SAT competition 2025 website](https://satcompetition.github.io/2025/downloads/checkers/veripb.pdf).

**NOTE:** If you installed the old version of VeriPB before, make sure to uninstall the old version, since the binary is named the same.


## Installation
### Requirements
To install VeriPB, you need to have Rust installed. It is recommended to install Rust via the tool `rustup`, as [described on the official website](https://www.rust-lang.org/tools/install).

Alternatively, the package manager of your choice (e.g., `apt`, `dnf`, ...) might also distribute the Rust compiler `rustc` and the default build tool `cargo`.

### Installing VeriPB
<!-- We should also make VeriPB available on crates.io as soon as we have a first release version. -->
VeriPB can be installed by cloning this repository and using the default Rust build tool Cargo. The same command can also be used to update the installation.
```bash
cargo install --path .
```

## Usage
A formula `instance.opb` and a derivation `proof.pbp`, you can call
```bash
veripb instance.opb proof.pbp
```

For further options use:
```bash
veripb --help
```

## Development Build
For development and quick testing it might make sense to run VeriPB without installing it and with debug symbols enabled. To run VeriPB directly after compilation with the formula `instance.opb` and derivation `proof.pbp`, use
```bash
cargo r -- instance.opb proof.pbp
```
where `--` separates the options for Cargo and VeriPB.

### Tests
To run all test cases (including unit and integration tests), use
```bash
cargo t --workspace
```

To have a nicer formatting of the test results, it is recommended to run the tests with the Cargo plugin `nextest`, where [installation instruction can be found here](https://nexte.st/docs/installation/pre-built-binaries/). To run all tests cases with `nextest`, use
```bash
cargo nextest run --workspace
```

## License
This version of VeriPB is distributed under the terms of both the [Apache License (Version 2.0)](https://www.apache.org/licenses/LICENSE-2.0) and the [MIT License](https://opensource.org/license/MIT), at your option.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

## How to Cite VeriPB
Please cite up to three of the following references in this order of priority. You can click on the references to get their BibTeX entry:

<p>
<details>
<summary>
Bart Bogaerts, Stephan Gocht, Ciaran McCreesh, and Jakob Nordström.
Certified Dominance and Symmetry Breaking for Combinatorial Optimisation.
Journal of Artificial Intelligence Research, 2023.
</summary>

```
@article{BGMN23Dominance,
  author    = {Bart Bogaerts and Stephan Gocht and Ciaran McCreesh
               and Jakob Nordström},
  title     = {Certified Dominance and Symmetry Breaking for
               Combinatorial Optimisation},
  year      = {2023},
  month     = aug,
  journal   = {Journal of Artificial Intelligence Research},
  volume    = {77},
  pages     = {1539\nobreakdash--1589},
  note      = {Preliminary version in \emph{AAAI~'22}},
}
```

</details>
</p>

<P>
<details>
<summary>
Stephan Gocht, and Jakob Nordström.
Certifying Parity Reasoning Efficiently Using Pseudo-Boolean Proofs.
Proceedings of the 35th AAAI Conference on Artificial Intelligence (AAAI '21), 2021.
</summary>

```
@inproceedings{GN21CertifyingParity,
  author    = {Stephan Gocht and Jakob Nordström},
  title     = {Certifying Parity Reasoning Efficiently Using
               Pseudo-{B}oolean Proofs},
  year      = {2021},
  month     = feb,
  booktitle = {Proceedings of the 35th {AAAI} Conference on
               Artificial Intelligence ({AAAI}~'21)},
  pages     = {3768\nobreakdash--3777}
}
```

</details>
</p>

<p>
<details>
<summary>
Stephan Gocht.
Certifying Correctness for Combinatorial Algorithms by Using Pseudo-Boolean Reasoning.
Lund University, Lund, Sweden, 2022.
</summary>

```
@phdthesis{Gocht22Thesis,
  author  = {Stephan Gocht},
  title   = {Certifying Correctness for Combinatorial Algorithms
             by Using Pseudo-{B}oolean Reasoning},
  school  = {Lund University},
  address = {Lund, Sweden},
  year    = {2022},
  month   = jun,
  note    = {Available at
             \url{https://portal.research.lu.se/en/publications/certifying-correctness-for-combinatorial-algorithms-by-using-pseu}},
}
```

</details>
</p>


## Applications

VeriPB has already been used for various applications including proof logging of

- subgraph isomorphism [[GMN20](#references)],
- clique and maximum common (connected) subgraph [[GMMNPT20](#references)],
- constraint programming with all different constraints [[EGMN20](#references)],
- parity reasoning in the context of CDCL SAT solvers [[GN21](#references)],
- dominance and symmetry breaking [[BGMN23](#references)],
- pseudo-Boolean to CNF encodings [[GMNO22](#references)],
- core-guided MaxSAT [[BBNOV23](#references)],
- linear search SAT-UNSAT MaxSAT [[VDB22, BBNOPV24](#references)],
- MaxSAT preprocessing [[IOTBJMN24]($references)],
- 0-1 ILP presolving [[HOGN24](#references)], and
- reasoning about states and transitions (as in dynamic programming) [[DMMNOS24](#references)].

## References

[DMMNOS24]: Emir Demirović, Ciaran McCreesh, Matthew J. McIlree, Jakob Nordström, Andy Oertel, Konstantin Sidorov. Pseudo-Boolean Reasoning About States and Transitions to Certify Dynamic Programming and Decision Diagram Algorithms. In Proceedings of the 30th International Conference on Principles and Practice of Constraint Programming (CP 2024), 2024.

[BBNOPV24]: Jeremias Berg, Bart Bogaerts, Jakob Nordström, Andy Oertel, Tobias Paxian, and Dieter Vandesande. Certifying Without Loss of Generality Reasoning in Solution-Improving Maximum Satisfiability. In Proceedings of the 30th International Conference on Principles and Practice of Constraint Programming (CP 2024), 2024.

[IOTBJMN24]: Hannes Ihalainen, Andy Oertel, Yong Kiam Tan, Jeremias Berg, Matti Järvisalo, Magnus O. Myreen, and Jakob Nordström. Certified MaxSAT Preprocessing. In Proceedings of the International Joint Conference on Automated Reasoning (IJCAR 2024), 2024.

[HOGN24]: Alexander Hoen, Andy Oertel, Ambros Gleixner, and Jakob Nordström.
Certifying MIP-based Presolve Reductions for 0–1 Integer Linear Programs.
In Proceedings of the 21st International Conference on the Integration of Constraint Programming, Artificial Intelligence, and Operations Research (CPAIOR 2024), 2024.

[BGMN23]: Bart Bogaerts, Stephan Gocht, Ciaran McCreesh, and Jakob Nordström.
Certified Dominance and Symmetry Breaking for Combinatorial Optimisation.
Journal of Artificial Intelligence Research, 2023.

[BBNOV23]: Jeremias Berg, Bart Bogaerts, Jakob Nordström, Andy Oertel, and Dieter Vandesande.
Certified Core-Guided MaxSAT Solving.
In Proceedings of the 29th International Conference on Automated Deduction (CADE-29), 2023.

[VDB22]: Dieter Vandesande, Wolf De Wulf, and Bart Bogaerts .
QMaxSATpb: A Certified MaxSAT Solver.
In Proceedings of the 16th International Conference on Logic Programming and Non-monotonic Reasoning, 2022.

[GMNO22]: Stephan Gocht, Jakob Nordström Ruben Martins and Andy Oertel.
Certified CNF Translations for Pseudo-Boolean Solving.
In Proceedings of the 25nd International Conference on Theory and Applications of Satisfiability Testing (SAT '22), 2022.

[GN21]: Stephan Gocht, and Jakob Nordström.
Certifying Parity Reasoning Efficiently Using Pseudo-BooleanProofs.
Proceedings of the AAAI Conference on Artificial Intelligence, 2021, 35, 3768-3777.

[GMN21]: Stephan Gocht, Ciaran McCreesh, and Jakob Nordström.
VeriPB: The Easy Way to Make Your Combinatorial Search Algorithm Trustworthy.
From Constraint Programming to Trustworthy AI, workshop at the 26th International Conference on Principles and Practice of Constraint Programming (CP '20), September 2020.
[PDF](http://www.jakobnordstrom.se/docs/publications/VeriPB_CPTAI2020.pdf>) [VIDEO](https://www.youtube.com/watch?v=SQ1-lF9clHQ>)


[GMMNPT20]: Stephan Gocht, Ross McBride, Ciaran McCreesh, Jakob Nordström, Patrick Prosser, and James Trimble.
Certifying Solvers for Clique and Maximum Common (Connected) Subgraph Problems.
In Proceedings of the 26th International Conference on Principles and Practice of Constraint Programming (CP '20), Lecture Notes in Computer Science, volume 12333, pages 338-357, September 2020.


[GMN20]: Stephan Gocht, Ciaran McCreesh, and Jakob Nordström.
Subgraph Isomorphism Meets Cutting Planes: Solving with Certified Solutions.
In Proceedings of the 29th International Joint Conference on Artificial Intelligence (IJCAI '20), pages 1134-1140, July 2020.


[EGMN20]: Jan Elffers, Stephan Gocht, Ciaran McCreesh, and Jakob Nordström.
Justifying All Differences Using Pseudo-Boolean Reasoning.
In Proceedings of the 34th AAAI Conference on Artificial Intelligence (AAAI '20), pages 1486-1494, February 2020.
