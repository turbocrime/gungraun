#!/usr/bin/env python3
"""Generate a culled .exp_stacks fixture from a complete (min_width=0) one.

Usage: python scripts/cull_stacks.py <input.exp_stacks> <threshold_percent>

The threshold is a percentage of total root inclusive cost. Child edges whose
inclusive cost falls below the threshold are replaced with a single
[below threshold] tombstone per parent, summing all culled children.

Uses a single-pass stack: scan entries forward, push each onto a stack. When
depth decreases (subtree finished), pop entries and decide keep vs cull.
Inclusive costs propagate up naturally — no tree construction needed.
"""

import sys


def main():
    if len(sys.argv) < 3:
        print(
            f"Usage: {sys.argv[0]} <input.exp_stacks> <threshold_percent>",
            file=sys.stderr,
        )
        sys.exit(1)

    input_path = sys.argv[1]
    threshold_pct = float(sys.argv[2])

    with open(input_path) as f:
        lines = f.read().splitlines()

    entries = []
    for line in lines:
        line = line.strip()
        if not line:
            continue
        last_space = line.rfind(" ")
        entries.append((line[:last_space], int(line[last_space + 1 :])))

    total_cost = sum(cost for _, cost in entries)
    min_cost = int(total_cost * threshold_pct / 100.0)

    result = cull(entries, min_cost)

    for path, cost in result:
        print(f"{path} {cost}")


def cull(entries, min_cost):
    # Stack entries: [path, self_cost, children_output, culled_cost]
    # children_output: flat list of (path, cost) lines to emit after self
    stack = []
    result = []

    def pop_one():
        path, self_cost, children_out, culled = stack.pop()
        inclusive = self_cost + sum(c for _, c in children_out) + culled

        # Build this node's output: self, tombstone (if any), then children
        emit = [(path, self_cost)]
        if culled > 0:
            emit.append((f"{path};[below threshold]", culled))
        emit.extend(children_out)

        if stack:
            if inclusive < min_cost:
                stack[-1][3] += inclusive
            else:
                stack[-1][2].extend(emit)
        else:
            # Root — always keep
            result.extend(emit)

    for path, cost in entries:
        depth = path.count(";")
        while stack and stack[-1][0].count(";") >= depth:
            pop_one()
        stack.append([path, cost, [], 0])

    while stack:
        pop_one()

    return result


if __name__ == "__main__":
    main()
