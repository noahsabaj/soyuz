# Signed Distance Fields

The mathematical foundation of Soyuz.

## What Is An SDF?

A Signed Distance Field is a function that takes a point in 3D space and returns the distance to the nearest surface:

- Positive values are outside the shape.
- Negative values are inside the shape.
- Zero is exactly on the surface.

## Why SDFs?

- Boolean operations become compact math.
- Smooth blending is natural.
- The model is resolution independent until mesh export.
- GPU raymarching can preview the field efficiently.

## From SDF To Mesh

SDFs are useful for modeling and previewing. Games and 3D software usually need triangle meshes, so Soyuz uses Marching Cubes to convert SDFs to exportable meshes.
