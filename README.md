# Vega-Lite (Rust port)

Port of [Vega-Lite](https://github.com/vega/vega-lite) to Rust, tracking upstream
`v6.5.0` (commit `4a29260c`).

The goal is to keep module boundaries and behaviour aligned with the upstream
TypeScript sources, and to translate the upstream test-suite module by module.

## Vega renderer (`vega-svg`)

The `vega-svg` crate renders Vega specifications (and Vega-Lite specs directly)
to SVG. It follows the upstream Vega package structure (`vega-expression`,
`vega-transforms`, `vega-scale`, `vega-scenegraph`, `vega-parser`/`vega-view`).


The Rust API exposes `vega_to_svg`, `vega_lite_to_svg`, `*_with_options`, and
`compiled_to_svg` (for an existing `CompileResult`). See
[`vega-svg/README.md`](vega-svg/README.md) for the supported subset.
`scripts/compare_svg.mjs` compares the renderer against upstream Vega on the
compiled example corpus.
