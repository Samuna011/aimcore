# sense-maxer

Windows-native Bevy VALORANT Input Validation Lab (M1).

M1 proves the pipeline end-to-end:

```
RAW MOUSE INPUT → SENSITIVITY MODEL → CAMERA YAW → NATIVE 3D RENDER → TELEMETRY → SQLITE
```

See `docs/` for architecture, input model, telemetry schema, and experiment design.

## Build

```bash
cargo build
```

## Status

Workspace scaffold only. Feature implementation follows the M1 plan.
