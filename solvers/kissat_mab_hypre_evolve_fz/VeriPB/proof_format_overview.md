# VeriPB Proof Format Overview

This document provides a brief overview of the formula file formats supported by VeriPB and the VeriPB proof format.


## Useful Examples

A good way to getting started is probably to have a look at the examples under `tests/instances/correct` and to run VeriPB with the `--trace` option, which will output the derived proof.

For example:
```bash
cd tests/integration_tests/correct
veripb --trace version3/all_diff.opb version3/all_diff.pbp
```

## Supported Formula File Formats
### OPB

The formula is provided in [OPB](http://www.cril.univ-artois.fr/PB12/format.pdf) format. A short overview can be found [here](https://gitlab.com/MIAOresearch/roundingsat/-/blob/master/InputFormats.md).

The verifier also supports an extension to OPB, which allows arbitrary variable names instead of `x1`, `x2`, ... Variable names must follow the following properties:

- start with a letter in `A-Z, a-z`
- are at least two characters long
- may not contain space
- variables introduced by VeriPB start with `_`

The following characters are guaranteed to be supported: `a-z, A-Z, 0-9, []{}_^-`. Support of further characters is implementation specific and produces an error if unsupported characters are used.


### DIMACS CNF

The formula can be provided in the [DIMACS CNF format](https://web.archive.org/web/20190325181937/https://www.satcompetition.org/2009/format-benchmarks2009.html). This format is then internally viewed as a OPB formula.

#### Variables

The variable `i` in the DIMACS CNF format is represented by `x<i>` and the literal `-i` is `~x<i>`.

#### Clauses

The semantics of DIMACS CNF are followed with respect to duplicate literals in clauses. Hence, the clause `1 -2 1 0` becomes the constraint `1 x1 1 ~x2 >= 1 ;`.

### MaxSAT

The formula can also be provided in [MaxSAT (WCNF)](https://maxsat-evaluations.github.io/2022/rules.html#input) format (both old and new format are supported). This format is then internally viewed as a OPB formula.

#### Variables

The variable `i` in the WCNF format input file is represented by `x<i>`.

#### Hard Clauses

Hard clauses are viewed as OPB constraints, where all coefficients are `1` and the right-hand side is `1`.

#### Soft Clauses

Soft clauses containing one literal are added directly to the objective without adding a constraint to the database. This is done by adding the negated literal and the weight of the soft clause as the coefficient to the objective.

Soft clauses with more than one literal are reformulated using a blocking literal `_b<i>`, where `i` is the index of the soft clause in the WCNF input file. Then the soft clauses with the literal `~_b<i>` is added to the OPB formula as a constraint and the literal `~_b<i>` with the weight of the soft clause as coefficient is added to the objective.


#### Example

| WCNF        | OPB                              |
| ----------- | -------------------------------- |
|             | `min: 1 ~x1 1 ~_b3 2 ~x2 2 ~_b5` |
| `1 1 0`     |                                  |
| `h 1 2 3 0` | `1 x1 1 x2 1 x3 >= 1`            |
| `1 2 3 0`   | `1 x2 1 x3 1 ~_b3 >= 1`          |
| `2 2 0`     |                                  |
| `2 1 2 0`   | `1 x1 1 x2 1 ~_b5 >= 1`          |

## Basic Proof Format
### TLDR;

```
pseudo-Boolean proof version 2.0
* compute constraint in polish notation
pol <sequence of operations in reverse polish notation>
* introduce constraint that is verified by reverse unit propagation
rup  <OPB style constraint> ;
* delete constraints
del id <constraintID1> <constraintID2> <constraintID3> ...
* objective update
obju <OPB style objective> ;
* add constraint by redundance based strengthening
red <OPB style constraint> ; <substitution>
* add constraint by dominance based strengthening
dom <OPB style constraint> ; <substitution>
```

### Introduction

There are multiple rules, which are described in more detail below. Every rule has to be written on one line and no line may contain more than one rule. Each rule can create an arbitrary number of constraints (including none). The verifier keeps a database of constraints and each constraint is assigned an index, called ConstraintID, starting from 1 and increasing by one for every added constraint. Rules can reference other constraints by their ConstraintID.

The constraints from the formula file are loaded before any rule is executed and get the first ConstraintIDs.

In what follows we will use IDmax to refer to the largest used ID before a rule is executed.

#### Constraint Labels
Additionally to the ConstraintID, a label for a constraint can be specified. The ConstraintID and the label can be used interchangeably in proofs. Labels have to start with the character `@`. To define a label for a constraint in the OPB file, prepend the constraint with the label, e.g.,
```
@label_name 1 x1 1 x2 1 x3 >= 1
```
To define a label for a constraint introduced by a rule, start the line of the rule with the label, e.g.,
```
@label_name pol 1 2 + 3 d
```
If a constraint label is defined that has already been defined earlier, then the label will be overwritten with the new ConstraintID.

To refer to a constraint with the label `@label_name`, just use the label instead of the ConstraintID, e.g.,
```
pol 1 @label_name + 3 d
```
Hence, in the following, whenever a ConstraintID is used as an argument for a proof rule, a label can be used instead of the ConstraintID.

### (pol) Reverse Polish Notation

```
pol <sequence in reverse polish notation>
```

Add a new constraint with ConstraintID := IDmax + 1. How to derive the constraint is describe by a 0 terminated sequence of arithmetic operations over the constraints. These are written down in reverse polish notation. We will use `[constraint]` to indicate either a ConstraintID or a subsequence in reverse polish notation. Available operations are:

#### Addition
```
<constraint> <constraint> +
```

#### Scalar Multiplication
```
<constraint> <factor> *
```
The factor is a strictly positive integer and needs to be the second
operand.

#### Boolean Division
```
<constraint> <divisor> d
```
The divisor is a strictly positive integer and needs to be the second
operand.


#### Boolean Saturation
```
<constraint> s
```

#### Literal Axioms
```
<literal>
x1
~x1
```
Where ``<literal>`` is a variable name or its negation (``~``) and generates the constraint that the literal is greater equal zero. For example for ``~x1`` this generates the constraint `~x1 >= 0`.

#### Weakening
```
<constraint> <variable> w
```
Where ``<variable>`` is a variable name and may not contain negation. This step adds literal axioms such that ``<variable>`` disappears from the constraint, i.e., its coefficient becomes zero.

#### Conclusion

This set of instructions allows writing down any treelike refutation with a single rule.

For example
```
pol 42 3 * 43 + s 2 d
```

Creates a new constraint by taking 3 times the constraint with index 42, then adds constraint 43, followed by a saturation step and a division by 2.

### (rup) Reverse Unit Propagation

```
rup <OPB style constraint> ;
rup <OPB style constraint> ; <ID1> <ID2> ...
```

Use reverse unit propagation to check if the constraint is implied, i.e., it temporarily adds the negation of the constraint and performs unit propagation, including all other (non deleted) constraints in the database. If this unit propagation yields contradiction then we know that the constraint is implied and the check passes.

Optionally, the rule can be annotated by a list of constraint IDs. If this list is given, then VeriPB will only perform unit propagation on these constraints. The reserved symbol `~` is used to specify the negation of the constraint that we want to derive. VeriPB will first perform the unit propagation in the order of the list. Hence, if the propagation order is known, then the constraint IDs should be printed in order.

If the reverse unit propagation check passes then the constraint is added with ConstraintID := IDmax + 1. Otherwise, verification fails.


### (del) Delete Constraint

```
del id <constraintID1> <constraintID2> <constraintID3> ...
del spec <OPB style constraint> ;
del range <constraintIDStart> <constraintIDEnd>
```

Delete constraints with given constrain IDs, specification or in the range from `constraintIDStart` to `constraintIDEnd`, including `constraintIDStart` but not `constraintIDEnd`. If a constraint is deleted that propagated under the empty assignment (e.g., a unit clause), then the propagations from this constraint are also deleted from the trail, which is different to DRAT.

#### Deletion from the Core Set

A constraint can only be deleted from the core set after a deletion check has been performed. The deletion check comes in two flavours. By default, VeriPB runs the [checked deletion checks](#checked-deletion), as this check guarantees that the new core set and the input formula are equienumerable/equioptimal/equisatisfiable. If the checked deletion check fails for any deletion from the core, these guarantees are lost and VeriPB only performs [unchecked deletion checks](#unchecked-deletion) for the rest of the proof, as they are computationally less expensive (and never fails).

##### Unchecked Deletion

Unchecked deletion performs the following checks:
1. If **no** order is loaded, accept deletion.
2. Otherwise, if the derived set is empty, accept deletion.
3. Otherwise, move all constraints from the derived set to the core set and accept deletion.

So unchecked deletion will never fail as it can automatically change the database to satisfy the second check.

##### Checked Deletion

The idea of checked deletion is that we can rederive the deleted constraint from the remaining constraints in the core by [redundance-based strengthening](#red-redundance-based-strengthening).

The deletion checks of multiple constraints will be done in the order in which the constraints are given. For instance, if we delete $C$ and $D$ and have the set of core constraints $\mathcal{C}$, then it is first checked that $C$ can be derived from $\mathcal{C} \setminus \{ C \}$ and then that $D$ can be derived from $\mathcal{C} \setminus \{ C, D \}$.

The syntax for a deletion check is very similar to [redundance-based strengthening](#red-redundance-based-strengthening). Checked deletion will create the same proofgoals as redundance-based strengthening and a substitution can be supplied if required to prove the proofgoals.

The following syntax is used for checked deletion with a witness:
```
<deletion rule> <deletion parameters> ; <substitution>
```
The syntax of `<substitution>` is described in the [substitution section](#substitution).

The proofgoals of checked deletion can be manually proven using the [subproof](#subproofs) syntax, or they are [autoproven](#autoproving) by VeriPB if they are trivial enough.

### (delc) Delete Core Constraint

```
delc <constraintID1> <constraintID2> <constraintID3> ...
```

This rule is identical to [`del id`](#delete-constraint) except that it checks if all `constraintIDs` are from the core set. So the rule will fail if at least one `constraintID` is from the derived set.

### (deld) Delete Derived Constraint

```
deld <constraintID1> <constraintID2> <constraintID3> ...
```

This rule is identical to [`del id`](#delete-constraint) except that it checks if all `constraintIDs` are from the derived set. So the rule will fail if at least one `constraintID` is from the core set.

### (obju) Objective Update

```
* objective update to new objective
obju new <new objective f_new in OPB format> ;
* objective update by difference
obju diff <f_new - f_old in OPB format> ;
* or with explicit subproof
obju new <new objective f_new in OPB format> ; begin
    proofgoal #1
        * proof f_new >= f_current
        <subproof>
    end -1
    proofgoal #2
        * proof f_current >= f_new
        <subproof>
    end -1
end
```

The version `obju new` of the rule updates the objective to the specified objective.

The version `obju diff` updates the objective by adding the specified difference between old and new objective to the old objective. Subtracting the old objective from the new objective results in an affine function, like all objective functions. Hence, the same syntax is used for stating a difference or an objective.

The new objective will be the only valid objective after the update.

To update the objective, it has to be shown that the previous objective ($f_{current}$) is equal to the new objective ($f_{new}$). This is done by showing that the constraints $f_{new} \geq f_{current}$ and $f_{current} \geq f_{new}$ can be derived from the formula. If these two constraints can be trivially proven by [autoproving](#autoproving), then no subproofs have to be specified to derive these two constraints. Otherwise, subproofs have to be specified for the constraints. The proofgoal ID for the constraint $f_{new} \geq f_{current}$ is ``#1`` and for the constraint $f_{current} \geq f_{new}$ the proofgoal ID is ``#2``.

**Attention:** To maintain soundness, [autoproving](#autoproving) and subproofs can only use constraints from the core set. Technically, this condition is not necessary for deriving $f_{current} \geq f_{new}$ (proofgoal ``#2``), but for simplicity, this condition is required for the derivation of both constraints.

## Strengthening Rules
### Substitution

A substitution ``<substitution>`` is a space separated sequence of multiple mappings from a variable to a constant or a literal.

```
<variable> -> 0
<variable> -> 1
<variable> -> <literal>
```

Using ``->`` is optional and can improve readability.

For example
```
x1 -> 0 x2 -> ~x3
x1 0 x2 ~x3
```



### (red) Redundance-Based Strengthening

```
red <OPB style constraint> ; <substitution>
```

Adding the constraint is successful if it passes the map e check via unit propagation or syntactic checks, i.e., if it can be shown that every assignment satisfying the constraints in the database $F$ but falsifying the to-be-added constraint $C$ can be transformed into an assignment satisfying both by using the assignment (or witness) $\omega$ provided by the list of literals. More formally it is checked that,

$$
F \land \neg C \models (F \land C)\upharpoonright\omega .
$$
For details, please refer to [[GN21](#references)].

If the redundance rule is used in the context of optimization and / or dominance breaking, additional conditions are checked. For details, please refer to [[BGMN23](#references)].

### Subproofs

For both strengthening rules it is possible to provide an explicit subproof. A subproof starts by ending the strengthening step with ``; begin`` and is concluded by ``end``. Within a subproof it is possible to specify proof goals using ``proofgoal <goalID>``, which are in turn terminated by ``end``. Each proofgoal needs to derive contradiction using the provided constraints.

Example
```
red 1 x1 >= 1 ; x1 -> 1 ; begin
    proofgoal #1
        pol -1 -2 +
    end -1

    proofgoal 1
        rup >= 1 ;
    end -1
end
```

The ``<goalID>`` are as follows: If a goal originates from a constraint in the database the ``<goalID>`` is identical to the constraintID of the constraint in the database. Otherwise, the goalID starts with a ``#`` followed by a number which is increased for each goal in the following order (if applicable): the constraint to be derived (only redundance), one goal per constraint in the order, one goal for the negated order (only dominance), objective condition (only for optimization problems). Tip: Use ``--trace`` option to display required goals.

#### Autoproving

For a subproof or a single proofgoal VeriPB will try out some techniques to automatically prove (*autoprove*) them. If VeriPB is able to do this, then it is not required to present an explicit proof for the whole subproof or the single proofgoal.

A subproof can be autoproven if unit propagation derives contradiction with respect to the database and the additional premises added at the start of the subproof (e.g., the negated constraint for [redundance-based strengthening](#red-redundance-based-strengthening)).

A proofgoal can be autoproven if the goal constraint is trivial (degree of falsity is zero), implied by [reverse unit propagation (RUP)](#rup-reverse-unit-propagation), or [syntactically implied](#i-implies) by any constraint in the database or the additional premise, where all variables that get assigned by unit propagation are substituted with their value in the premise and conclusion constraint of the implication.

We recommend that you look at the trace (using the `--trace` option) of VeriPB to see what autoproving is done by VeriPB and it can make sense to compare the performance of autoproving and explicit proofs for your use case.

### (dom) Dominance Based Strengthening

```
dom <OPB style constraint> ; <substitution>
```

For details, please refer to [[BGMN23](#references)]. For syntax have a look at the example under ``tests/integration_tests/correct/dominance/example.pbp`` .

Example proof:
```
def_order simple
    * specify variables
    vars
        left u1
        right v1
    end

    * define the order
    def
        -1 u1 1 v1 >= 0 ;
    end

    * proof goal: transitivity
    transitivity
        vars
            fresh_right w1
        end
        proof
            proofgoal #1
                p 1 2 + 3 +
            qed -1
        qed
    qed
end

load_order simple x1
dom 1 ~x1 >= 1 ; x1 0
```

#### Order Definition

```
def_order <order name>
    vars
        left <list of variables>
        right <list of variables>
        aux <list of variables>
    end

    def
        <constraints defining the order>
    end

    transitivity
        vars
            fresh_right <list of variables>
        end
        proof
            <subproofs>
        qed
    end

    reflexivity
        proof
            <subproofs>
        qed
    end
end
```

A new order ${\cal O}_\preceq(\vec{u}, \vec{v})$ (i.e., $\vec{u} \preceq \vec{v}$) can be defined using the above syntax. The order is a preorder, thus the defined order need to be reflexive and transitive.

The first `vars` defines the variables used in the definition of the order. The variables after `left` are the variables in $\vec{u}$ and the variables after `right` are the variables in $\vec{v}$. The number of variables in $\vec{u}$ must be the same as in $\vec{v}$. The variables after `aux` are additional variables that can be used to defined the order.

The constraints in `def` define the order. Only variables in `left`, `right` and `aux` can be used.

The `transitivity` proof established that the order is transitive, i.e., if ${\cal O}_\preceq(\vec{u}, \vec{v})$ and ${\cal O}_\preceq(\vec{v}, \vec{w})$, then ${\cal O}_\preceq(\vec{u}, \vec{w})$. The variables after `fresh_right` are the variables in $\vec{w}$ and the number of variables in $\vec{w}$ has to be the same as in $\vec{u}$ (and $\vec{v}$). In the `proof` it has to be proven that each constraint in ${\cal O}_\preceq(\vec{u}, \vec{w})$ can be derived from the constraints in ${\cal O}_\preceq(\vec{u}, \vec{v})$ and ${\cal O}_\preceq(\vec{v}, \vec{w})$.

The `reflexivity` proof establishes that the order is reflexive, i.e., ${\cal O}_\preceq(\vec{u}, \vec{u})$. The `reflexivity` proof is optional if the reflexivity of the order is trivial (negated constraints in ${\cal O}_\preceq(\vec{u}, \vec{u})$ are contradiction). In the `proof` it has to be proven that each constraint in ${\cal O}_\preceq(\vec{u}, \vec{u})$ can be derived from an empty formula.

The transitivity proof has to come before the reflexivity proof (if an explicit reflexivity proof is given).

### Moving Constraints to Core

```
core id <constraintID1> <constraintID2> ...
core range <constraintIDStart> <constraintIDEnd>
```

## Output and Conclusion Section

### TLDR;

```
* output section
output <output guarantee> <output type>
* conclusion section
conclusion <conclusion type> [<conclusion parameters>]
* end of proof
end pseudo-Boolean proof
```

Every proof has to end with the output and conclusion section. This section must contain in the following order:

1. the output section
2. the conclusion section
3. end of proof

### Output Section

```
output <output guarantee> <output type>
```

For the moment, the output guarantees `NONE`, `DERIVABLE`, `EQUISATISFIABLE`, and `EQUIOPTIMAL` and output types `IMPLICIT`, and `FILE` are implemented.

#### Output Guarantees

The following table details the output guarantees and what is required for the guarantees. We refer to *input* as the input problem that the proof starts with and *output* as the output problem to check against.

| Identifier                             | Guarantee                                                      | Conditions                                                                     |
| -------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `NONE`                                 | no guarantee                                                   | output type is empty (just `output NONE`)                                      |
| `DERIVABLE`                            | *output* derivable from *input*                                | no conditions                                                                  |
| `EQUISATISFIABLE`                      | *output* is equisatisfiable to *input*                         | always checked deletion used, *input* does not have objective                  |
| `EQUIOPTIMAL`                          | *output* has same optimal value as *input*                     | always checked deletion used, *input* has objective                            |
| `EQUIENUMERABLE` (**not implemented**) | *output* has the same number of (optimal) solutions as *input* | always checked deletion used, no preserved variable in the domain of a witness |


#### Output Types

The following table details the output types and how the output problem should be given.

| Identifier                          | How to give output?                                                                                                           |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `FILE`                              | external file in same format as input file gives as third positional argument (e.g., `veripb input.opb proof.pbp output.opb`) |
| `CONSTRAINT` (**not implemented**)  | `output` is followed by a list of constraints (and objective) as in OPB format                                                |
| `IMPLICIT`                          | output is implicitly the current core (and objective)                                                                         |
| `PERMUTATION` (**not implemented**) | constraints are permuted as given by a list of constraint IDs and current objective output                                    |


### Conclusion Section

```
conclusion NONE

conclusion SAT [: <literal> <literal> ...]
conclusion UNSAT [: <constraintID>]

conclusion BOUNDS <lower bound> [: <constraintID>] <upper bound> [: <literal> <literal> ...]
```

#### Conclusion ``NONE``

The conclusion ``NONE`` states that the proof concludes without any conclusion. This conclusion is always valid, but no guarantees on the proof are enforced.


#### Conclusion ``SAT``

The conclusion ``SAT`` states that the formula is satisfiable. If this conclusion is used, then the proof has to show that there exists at least one solution. To show this, a list of literals can be specified after the conclusion, which must be a solution. If no solution is specified after the conclusion, then at least one solution has to be logged using log (sol)ution.


#### Conclusion ``UNSAT``

The conclusion ``UNSAT`` states that the formula is unsatisfiable. If the proof claims this conclusion then it has to show that contradiction can be derived. This can be done by explicitly deriving contradiction and pointing to it as the optional hint after the conclusion. If no hint is given, then there must be a constraint in the database that syntactically implies contradiction.


#### Conclusion ``BOUNDS``

This conclusion can only be used for optimization problems. The conclusion ``BOUNDS`` states that the optimal value is between ``<lower bound>`` and ``<upper bound>``. If the bounds are equal, this means that the optimal value has been found.

To show the correctness of the ``<lower bound>`` a constraint $C$ that shows that the objective is at least the ``<lower bound>`` has to be derived. This has to be done by explicitly deriving a constraint that syntactically implies $C$ (which might already be derived in the proof). The ID of the constraint that syntactically implies $C$ can optionally be given as a hint for the lower bound or VeriPB will search through the database for this constraint.

To show the correctness of the ``<upper bound>``, there must be a solution that has an objective value that is at least as good as the ``<upper bound>``. The solution can be given as a hint or otherwise must have been logged before in the proof using the log (sol)ution rule.

For optimization problems there are the following special cases:
**Infeasible:** Use the lower bound and upper bound to `INF` (infinity) to denote an infeasible instance. The hint for the lower bound should be a contradicting constraint and no hint is required for the upper bound.
**Unbounded:** This case does not really exist for PB instances, so you would give the smallest possible value as lower bound and upper bound. No hint is required for the lower bound and the hint for the upper bound is an assignment that sets all literals in the objective to 0.
**Only lower bound:** The upper bound should be set to `INF`. No hint is required for the upper bound.

### End of Proof

```
end pseudo-Boolean proof
```

The proof has to end with this line. Everything after this line is not part of this proof. It is possible to start a new proof after this.


## Convenience Rules and Rules for Sanity Checks
### TLDR;

```
* check number of constraints in formula
f <nProblemConstraints>
* check equality
e <OPB style constraint> ; [<ConstraintID>]
* add constraint if equal
ea <OPB style constraint> ; [<ConstraintID>]
* Check equality objective
eobj <OPB style objective> ;
* check implication
i <OPB style constraint> ; [<ConstraintID>]
* add constraint if implied
ia <OPB style constraint> ; [<ConstraintID>]
* set level (for easier deletion)
#   <level>
* wipe out level (for easier deletion)
w   <level>
* strengthening to core mode
strengthening_to_core on|off
```

### (f) Formula Check

```
f <nProblemConstraints>
```

This rule can be used to check that the correct number of constraints have been loaded by VeriPB and to check that the proof logger starts with the correct constraint ID.

The value of `<nProblemConstraints>` is the number of constraints counting equalities twice. This is because equalities in the input formula are replaced by two inequalities, where the first inequality is `>=` and the second `<=`. Afterwards, the `i`-th inequality in the input formula gets `ID := IDmax + i`.

If the constraint count does not match, then the verification fails. If the constraint count is missing, then the check is ignored.


For example if we have the OPB file
```
* #variable= 3 #constraint= 1
1 x1 2 x2 >= 1 ;
1 x3 1 x4  = 1 ;
```

then VeriPB will load the constraints
```
1: 1 x1 2 x2 >= 1 ;
2: 1 x3 1 x4 >= 1 ;
3: -1 x3 -1 x4 >= -1 ;
```

so the following formula check will succeed
```
pseudo-Boolean proof version 2.0
f 3
```

In the past, this rule was used to load the formula into VeriPB. However, VeriPB loads the full formula right from the start now. So it is only used for checking that the right number of constraints have been loaded.

### (e) Equals

```
e <OPB style constraint D> ; [<ConstraintID for C>]
```

Verify that C is the same constraint as D, i.e., has the same degree and contains the same terms (order of terms does not matter). If the optional constraint ID of C is not specified, then this rule will check if there exists the same constraint as D in the database.


### (ea) Equals and Add

```
ea <OPB style constraint D> ; [<ConstraintID for C>]
```

Identical to [equals](#e-equals) but also adds the constraint `D` to the database with `ConstraintID := IDmax + 1`.


### (eobj) Equal Objective
```
eobj <OPB style objective> ;
```

This rule checks if the current objective is equal to the objective given in the rule. The given objective will be normalized before performing the comparison with the normalized current objective function. If the check fails, the proof checking fails.


### (i) Implies

```
i <OPB style constraint D> ; [<ConstraintID for C>]
```

Verify that C syntactically implies D. I.e., it is possible to derive D from C by adding literal axioms followed by one saturation step and finally adding literal axioms for the coefficients in D that are larger than the degree of D. If the optional constraint ID of C is not specified, then this rule will check if there exists any constraint in the database that syntactically implies D.


### (ia) Implies and Add

```
ia <OPB style constraint D> ; [<ConstraintID for C>]
```

Identical to [implies](#i-implies) but also adds the constraint that is implied to the database with `ConstraintID := IDmax + 1`.

### (#) Set Level

```
# <level>
```

This rule does mark all following constraints, up to the next invocation of this rule, with ``<level>``. ``<level>`` is a non-negative integer. Constraints which are generated before the first occurrence of this rule are not marked with any level.

### (w) Wipe out Level

```
w <level>
```

Delete all constraints (see deletion command) that are marked with ``<level>`` or a greater number. Constraints that are not marked with a level can not be removed with this command.

### Strengthening to Core Mode

```
strengthening_to_core on|off
```

This rule enables (`strengthening_to_core on`) and disables (`strengthening_to_core off`) the strengthening to core mode. When enabling the strengthening to core mode, all constraints are moved from the set of derived constraints to the set of core constraints.

When the strengthening to core mode is active, then all constraints introduced by strengthening rules are added to the set of core constraints instead of the set of derived constraints. This has the advantage that redundance-based strengthening only has constraints from the core as proofgoals from the formula.

## Beyond Refutations
### TLDR;

```
  * log solution
  sol  <literal> <literal> ...
  * log solution and add objective-improving constraint
  soli <literal> <literal> ...
  * log solution and add solution-excluding constraint
  solx <literal> <literal> ...
```

### (sol) Log Solution

```
sol <literal> <literal> ...
sol x1 ~x2
```

Given a partial assignment in form of a list of ``<literal>``, i.e., variable names with ``~`` as prefix to indicate negation, check that:

- after unit propagation we are left with a full assignment to the current database, i.e., an assignment that assigns all variables that are mentioned in a constraint in the formula or the proof

- the full assignment does not violate any constraint in the current database

### (soli) Log Solution and Add Objective-Improving Constraint

```
soli <literal> <literal> ...
soli x1 ~x2
```

This rule can only be used if the OPB file specifies an objective function $f(x)$, i.e., it contains a line of the form
```
min: <coefficient> <literal> <coefficient> <literal> ...
```

This rule performs the same checks as the log (sol)ution rule.

If the check is successful then the constraint $f(x) \leq f(\rho) - 1$ is added with `ConstraintID := IDmax + `1. If the check is not successful then verification fails.


### (solx) Log Solution and Add Solution-Excluding Constraint

```
solx <literal> <literal> ...
solx x1 ~x2
```

This rule performs the same checks as the log (sol)ution rule.

If the check is successful then the clause consisting of the negation of all literals is added with `ConstraintID := IDmax + 1`. If the check is not successful then verification fails.


## Debugging and for Development Only
### TLDR;

```
* add constraint as unchecked assumption
a <OPB style constraint> ;
* track the time of a section
start_time <name>
end_time <name>
* check if constraint is not in database
is_deleted <OPB style constraint> ;
```

### (a) Unchecked Assumption

```
a <OPB style constraint> ;
```

Adds the given constraint without any checks. The constraint gets `ConstraintID := IDmax + 1`. Proofs that contain this rule are not valid, because it allows adding any constraint. For example one could simply add contradiction directly.

This rule is intended to be used during solver development, when not all aspects of the solver have implemented proof logging, yet. For example, imagine that the solver knows by some fancy algorithm that it is OK to add a constraint C, however proof logging for the derivation of C is not implemented yet. Using this rule we can simply add C without providing a derivation and check with VeriPB that all other derivations that are already implemented are correct.

### Tracking Time to Check Sections of Proof
The following 2 rules can be used to track the time of names sections in the proof. If there are multiple sections with the same name, then the times are added up to a total time. The total time is displayed at the end of the checking when the option `--stats` is used. The `<name>` of a section can be any string that does not contain a whitespace.

#### (start_time) Start Cutom Timer

```
start_time <name>
```

Start the timer with the name `<name>`.

**Note:** If the timer `<name>` is already running, then the second start will be ignored and a warning is printed.

#### (end_time) End Custum Timer

```
end_time <name>
```

Stops the timer with the name `<name>` and adds the time that has been elapsed since the start of the timer to the total time for the timer `<name>`.

**Note:** If a timer is ended that is not running, then the end is ignored and a waring is printed.

### (is_deleted) Check If Constraint is Deleted

```
is_deleted <OPB style constraint> ;
```

This rule checks if the given constraint exists in the database. If the constraint is in the database, the proof will fail. The proof continues normally if this constraint does not exist in the database.

This rule can be used to double-check that a constraint is truly deleted from the database maintained by the checker.


### (fail) Fail Proof

```
fail
```

This rule immediately fails the proof checking. This rule can be used to fail proof checking at a certain point if the proof should only be checked until this point and not further.
