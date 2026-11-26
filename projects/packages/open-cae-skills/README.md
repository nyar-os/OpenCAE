# @wae/open-cae-skills

Agent instructions for OpenCAE: the command surface, the preflight steps and the
evidence rules an assistant needs to build and solve a model without inventing a
capability.

## Install

```bash
pnpm add @wae/open-cae-skills
```

Point your agent at the skill file:

```text
node_modules/@wae/open-cae-skills/skills/open-cae/SKILL.md
```

## Example prompt

> Build a 1000 mm span two-bar truss with a 500 mm rise and −1000 N at the apex.
> Solve it, report the axial force in each bar and the apex displacement, then
> export the deformed shape as an SVG drawing.

The skill tells the agent to run `open-cae capabilities` first, to state which
document it wrote, and to report the solver result instead of estimating one.

## Links

- [OpenCAE skill](./skills/open-cae/SKILL.md)
- [OpenCAE README](../../../README.md)
- [CLI reference](../../../documentation/cli.md)
- [Capabilities](../../../documentation/capabilities.md)
