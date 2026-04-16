# VeriPB Migration Guide From Version 2.0 to Version 3.0 of the Proof Format

Below are two lists that give an overview of the [breaking changes](#breaking-changes) from version 2.0 to version 3.0 of the proof format and the [new features](#new-features) in the 3.0 proof format. More details about the current grammar, including new features, can be found in the [latest grammar document](https://gitlab.com/api/v4/projects/70013030/jobs/artifacts/main/raw/docs/grammar.pdf?job=build-grammar-doc&search_recent_successful_pipelines=true).

For examples of version 3.0 proofs, see the [latest grammar document](https://gitlab.com/api/v4/projects/70013030/jobs/artifacts/main/raw/docs/grammar.pdf?job=build-grammar-doc&search_recent_successful_pipelines=true) or the [test instances](tests/instances/correct) in this repository.

## Breaking Changes
The following things have been changed in version 3.0 of the proof format:
- Every proof line (rule or definition) now ends by semicolon (`;`) instead of newline (`\n`, `\r` or `\r\n`). Newlines are allowed after the semicolons and are treated as whitespace. **NOTE**: Starting a subcontext, for instance with `subproof` (previously `begin`) or `proofgoal`, is not considered a proof step on its own (see later examples in the migration guide).

**VeriPB 2.0**:
```
pol 1 2 +
```
**VeriPB 3.0**:
```
pol 1 2 +;
```

- Constraint IDs may no longer be prefixed with a plus sign `+`.

**VeriPB 2.0**:
```
pol +1 +2 +
```
**VeriPB 3.0**:
```
pol 1 2 +;
```

- Comments now start with percentage sign (`%`) instead of asterisk (`*`) which means they now work like \LaTeX{} comments (and no longer as OPB comments).

**VeriPB 2.0**:
```
* This is no longer a comment.
```
**VeriPB 3.0**:
```
% This is now a comment.
```

- Optional arguments in rules are now separated by colon (`:`) instead of semicolons (`;`).

**VeriPB 2.0**:
```
red +1 ~r1 +1 x1 >= 1 ; r1 -> 0
e +1 ~r1 +1 x1 >= 1 ; -1
```
**VeriPB 3.0**:
```
red +1 ~r1 +1 x1 >= 1 : r1 -> 0;
e +1 ~r1 +1 x1 >= 1 : -1;
```

- Witnesses/Substitutions (in strengthening rules) may no longer contain optional commas (`,`) to separate assignments. Furthermore, whitespace is now required around the optional arrows (`->`).

**VeriPB 2.0**:
```
aa -> bb, cc dd, ee -> ff
aa ->bb cc ->dd ee ->ff
```
**VeriPB 3.0**:
```
aa -> bb cc dd ee -> ff
aa -> bb cc -> dd ee -> ff
```

- Subproofs for strengthening rules, the objective update rule (`obju`), and checked deletion now start with `subproof` instead of `begin`. Subproofs, proofs (in order definitions) and proof goals now always ends with `qed` instead of `end`.

**VeriPB 2.0**:
```
red +1 ~r1 +1 x1 >= 1 ; r1 -> 0 ;
begin
	proofgoal #1
		...
	end
end
```
**VeriPB 3.0**:
```
red +1 ~r1 +1 x1 >= 1 : r1 -> 0 : subproof
	proofgoal #1
		...
	qed;
qed;
```

- It is no longer allowed to use the redundance-based strengthening rule with an empty witness. The new proof by contradiction should be used instead.

**VeriPB 2.0**:
```
red +1 x1 >= 0 ; ; begin
	proofgoal #1
		...
	end
end
```
**VeriPB 3.0**:
```
pbc +1 x1 >= 0 : subproof

	...

qed;
```

- The hint (for the contradicting constraint) that are allowed at the end of subproofs and proofgoals must now be preceded by a colon (`:`).

**VeriPB 2.0**:
```
red +1 ~r1 +1 x1 >= 1 ; r1 -> 0 ; begin
	proofgoal #1
		...
	end -1
end -1
```
**VeriPB 3.0**:
```
red +1 ~r1 +1 x1 >= 1 : r1 -> 0 : subproof
	proofgoal #1
		...
	end : -1;
end : -1;
```

- The equals and add rule (`ea`-rule) have been removed in favor of the equals rule (`e`-rule) with a label, since the `e`-rule now return the ID of a matching constraint such that a label can be assigned that ID.

**VeriPB 2.0**:
```
* The line below adds a new ID!
@label ea +1 x1 >= 1
```
**VeriPB 3.0**:
```
% The line below does NOT add a new ID!
@label e +1 x1 >= 1;
```

- Multiplication by zero is not longer allowed in the `pol`-rule. Furthermore, it is no longer allowed to prefix integers with leading zeros.

**VeriPB 2.0**:
```
* Multiply constraint with ID 42 by int.
pol 42 0 *
pol 42 002 *
```
**VeriPB 3.0**:
```
% Multiply constraint with ID 42 by int.
pol 42;
pol 42 2 *;
```

- All short/alternative names for rules and definitions with multiple names have been removed.

**VeriPB 2.0**:
```
p 1 2 +
u +1 x1 >= 0;
d 42
v ~x1 x2 x3
o ~x1 x2 x3
pre_order
	...
end
```
**VeriPB 3.0**:
```
pol 1 2 +;
rup +1 x1 >= 0;
del 42;
solx ~x1 x2 x3;
soli ~x1 x2 x3;
def_order
	...
end;
```

- Rename the set level rule from `#` to `setlvl` and the wipe level rule from `w` to `wiplvl`.

**VeriPB 2.0**:
```
# 1
w 3
```
**VeriPB 3.0**:
```
setlvl 1;
wiplvl 3;
```

- The output type `CONSTRAINT` has been removed (it was never fully supported). Please write the contents to a file, specify that filename on the command line and use the output type `FILE`.

**VeriPB 2.0**:
```
output EQUISATISFIABLE CONSTRAINT ...
```
**VeriPB 3.0**:
```
output EQUISATISFIABLE FILE;
```

## New Features
The following new features have been added in version 3.0:

- Comments and whitespace (even newlines) may appear (almost) everywhere. Comments and whitespace will separate tokens but otherwise have no effect.

- Added a new rule: Proof by contradiction rule (`pbc`-rule) which is allowed in all contexts as replacement for using the redundance-based strengthening rule (`red`-rule) with an empty witness.

- Every rule using a subproof and every proof goal may now optionally end by (repeating) the name of the rule or proof goal being proved before the optional hint, if any. Note that the optional hint must now be preceded by a colon (`:`). Examples of what is allowed now:

```
proofgoal #1 ... qed #1 : -1;
proofgoal #1 ... qed #1;
proofgoal #1 ... qed : -1;
proofgoal #1 ... qed;
```
```
pbc ... : subproof ... qed pbc : -1;
pbc ... : subproof ... qed pbc;
pbc ... : subproof ... qed : -1;
pbc ... : subproof ... qed;
```
- Similarly, every definition ending with `end` can now optionally be followed by the name of the definition i.e.`reflexivity ... end;` can now also be `reflexivity ... end reflexivity;`.

- The equals rule (`e`-rule) and the implied rule (`i`-rule) now return the ID of a matching constraint such that a label can be assigned that ID, e.g., write `@label e <constraint>;` to assign `@label` to the ID of a constraint equal to/implying `<constraint>`.

- Constraints specified in rules (like `rup`, `red`, `del spec`, etc.) no longer end with a semicolon (`;`) - since the rule describes where to put (semi)colons and not the constraints (arguments).

- Extended the order definition (`def_order`) to also contain a specification (a derivation from the empty set) block and two fresh sets of auxiliary variables for the transitivity proof. This is to allow orders with auxiliary variables. For further details on the syntax and semantics of orders with auxiliary variables, have a look at [the paper introducing this feature](https://arxiv.org/abs/2511.16637).

- Auxiliary variables used in orders must start with a dollar sign (`$`) followed by any allowed character for variable names. (The grammar does not restrict the usage of auxiliary variables in places where normal variables are used. However, their usage will be restricted at the semantic level.)

- Added `scope`'s to subproofs for strengthening rules (`dom` and `red`). Scopes work like a subproof inside the subproof and are used to introduce the additional specification constraints used in orders with auxiliary variables.

- Added a new rule: Equal order rule (`eord_def`-rule).

- Added a new rule: Equal loaded order rule (`eord_loaded`-rule).

- Added a new rule: Objective improvement rule (`obji`-rule) that allows to log an objective value using `obji <objective value>;` without specifying a solution and add the corresponding objective improving constraint. However, a valid solution with the same or a better objective value must be specified later in the proof. The `soli`-rule and `sol`-rule now allow to optionally specify an objective value using `(sol|soli) <solution> [: <objective value>];`.

- Constraints may now also be written as a reification in one direction where the right implication is a (non-empty) conjunction of literals (`z1 z2 ~z3 ==> +1 x1 +2 x2 +3 x3 +4 x4 >= 4`) and the left implication is a single literal (`z1 <== +1 x1 +2 x2 +3 x3 +4 x4 >= 4`).

- The reverse polish notation rule (`pol`) for cutting plane derivations have been updated with new derivation rules: Variable normal form division (`c`), variable normal form mixed integer rounding (MIR) cut (`m`), literal normal form MIR cut (`n`) and decrementing the right-hand side (`-`). The difference between the variable normal form operations (`c` and `m`) and their literal normal form (normalized form) counterparts (`d` and `n`) is which representation of the constraint that division and MIR cut, respectively, is applied to. All of these rules expect two elements on the `pol` stack: an positive integer at the top and a constraint below it. E.g., `pol 42 1 - 2 d 3 c 4 n 5 m;` would decrease the right-hand side of the constraint with ID 42 by 1, then divide the result in normalized form by 2, then divide that result in variable normal form by 3, then apply MIR with divisor 4 on this result in normalized form and finally apply MIR with divisor 5 on the previous result in variable normal form.
