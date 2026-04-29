# Bloch Sphere

An interactive 3D visualization of single-qubit states on the Bloch
sphere. Click the link below to open it in a new tab — it uses
[three.js](https://threejs.org/) loaded from a CDN to render the sphere
and animate gate operations.

[**Open the interactive Bloch sphere →**](./bloch.html)

## What it shows

- The unit sphere with $|0\rangle$ at the north pole and $|1\rangle$ at
  the south pole.
- The state vector for a single qubit, with controls to apply $X$, $Y$,
  $Z$, $H$, $S$, and $T$ gates and watch the vector rotate.
- The mapping between gate operations and rotations of the sphere (see
  [Gate Physics](./gate-physics.md) for the underlying math).

## Source

The source is a single self-contained HTML file at
[`docs/theory/bloch.html`](./bloch.html) in the repository.
