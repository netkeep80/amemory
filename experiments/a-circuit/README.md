# A-Circuit benchmark research

Issue: #61

This module develops a meaningful standard digital-circuit benchmark for A-memory
incrementally. It is deliberately a semantic research ladder before it becomes a
performance benchmark.

## Rule

Do not scale a circuit level until the previous level is semantically understood.

If a circuit needs an operation that is not derivable from the pinned A-memory
execution semantics, stop and classify the gap. Do not hide the missing operation
in host code.

## C0 — unary NOT

The current grounded reaction has the form:

```text
K⟼A + (A⟼B) => K⟼B
```

C0 uses two structural boolean states and a stable context/token `K`:

```text
K⟼0 + (0⟼1) => K⟼1
K⟼1 + (1⟼0) => K⟼0
```

The host constructs the fixture and checks the output, but it does not evaluate
NOT. The transition itself is executed by `OptimizedReactionEngine`.

Status:

```text
C0_NOT = expected executable
```

This is only a single-premise gate/state-transition witness. It is not yet a
multi-wire circuit.

## C1 — binary AND as a theory probe

A real binary AND requires joint evidence:

```text
A=0, B=0 -> OUT=0
A=0, B=1 -> OUT=0
A=1, B=0 -> OUT=0
A=1, B=1 -> OUT=1
```

The critical property is not the truth table itself. It is the need for a result
whose authority depends on **co-presence of two independent premises**.

For a fixed Theory, the currently implemented grounded reaction evaluates every
current member independently and normalizes the union of their images.

Abstractly:

```text
F(S) = normalize( union_{x in S} image_T(x) )
```

with the existing unmatched-member preservation rule.

Therefore:

```text
F({A,B}) = normalize(F({A}) union F({B}))
```

for independent current members under the same fixed Theory.

A desired AND result `OUT` would require:

```text
OUT notin F({A})
OUT notin F({B})
OUT in    F({A,B})
```

which contradicts pointwise additivity.

The executable C1 witness does two things:

1. exhausts all `2^9 = 512` fixed unary Theory subsets over a closed
   three-state universe `{A,B,OUT}`;
2. checks both the additive law and the absence of a one-step true-AND realization.

The finite exhaustive witness is not by itself a new MTS theorem. It is evidence
about the currently pinned execution mechanism. The general additivity statement
should be reviewed as a theory/profile question before C2 Full Adder work.

## Why this matters

This is exactly why #61 is iterative.

A large multiplier requires real fan-in. If A-memory cannot derive a multi-premise
join without:

- prejoining the inputs in host code;
- enumerating the full global circuit state;
- depending on processing order;
- changing Theory externally between premise arrivals;
- adding a special gate opcode;

then the benchmark has found a theoretical boundary before performance numbers
could hide it.

## Next decision after C0/C1

If C1 confirms the boundary, do **not** implement Half Adder by host pairing.

Instead classify what MTS/A-memory needs for a native conjunction/join, for example
a structurally authorized multi-premise condition or an equivalent construction
derivable from Links alone.

Only after that semantics is explicit should the ladder continue:

```text
C2 Half Adder
C3 Full Adder
C4 Ripple-Carry Adder
C5 Carry-Lookahead / prefix adder
C6 Array Multiplier
C7 fixed c6288 checkpoint
C8+ 32/64/128/... bit multiplier
```

Performance comparison starts only after the circuit is actually executed by
A-memory rather than by the host.


## S1/P1 — binary AND in two multi-argument encodings

Status:

```text
SEQUENTIAL_AND = GREEN
PARALLEL_AND   = GREEN
```

Sequential encoding is partial application:

```text
AND + a  -> AND_a
AND_a + b -> value
```

The partial function `AND_a` is a real portable Link.

Parallel encoding takes one complete ROOT-originating MTS ExactSequence:

```text
args = ExactSequence_R([a,b])
AND + args -> value
```

Important negative witness retained from the earlier C1 probe:

```text
independent Scope members {a,b}
!=
multi-argument function invocation
```

## C2a — Half Adder as a multi-argument / multi-result function

C2a deliberately tests the function boundary before testing actual gate composition.

Truth table:

```text
a b | Sum Carry
0 0 |  0    0
0 1 |  1    0
1 0 |  1    0
1 1 |  0    1
```

The result is one canonical ROOT-originating ExactSequence:

```text
Half(a,b) -> ExactSequence_R([Sum,Carry])
```

not an unordered set and not an ordinary pair.

Sequential form:

```text
HALF + a
  -> HALF_a

HALF_a + b
  -> ExactSequence_R([Sum,Carry])
```

Parallel form:

```text
HALF + ExactSequence_R([a,b])
  -> ExactSequence_R([Sum,Carry])
```

Required C2a evidence:

- all four rows;
- one fixed structural Theory per encoding;
- reference CPU / optimized CPU portable differential;
- exact first-stage partial function for sequential form;
- exact ordered two-position result sequence;
- `ExactSequence_R([Sum,Carry]) != PAIR(Sum,Carry)`;
- host checks expected arithmetic truth table but does not evaluate the function transition.

C2a is still not yet a composed circuit.

## C2b — actual Half Adder circuit

Next required stage:

```text
Sum   = XOR(a,b)
Carry = AND(a,b)
```

XOR and AND must execute as distinct function/gate structures.

The key research obligation is to produce the final:

```text
ExactSequence_R([Sum,Carry])
```

from those independently produced gate outputs **inside A-memory**.

Forbidden:

- host reading XOR/AND outputs and assembling the final sequence;
- host evaluating either gate;
- special HalfAdder opcode;
- replacing the actual gate network with one direct truth-table lookup and calling it a circuit.

C2b is the first real gate-composition test. Full Adder remains blocked until C2b is GREEN.
