# AGBot Plugin SDK

The `plugin_sdk` crate exposes the host-side contracts for sandboxed plugin
extension points. Plugins declare a manifest, capabilities, and an entrypoint;
the host validates the manifest, checks capabilities, runs the sandbox plan, and
records plugin identity on produced outputs.

## Scaffold

Use `scaffold_plugin` with a `PluginScaffoldRequest` to generate a minimal
plugin skeleton:

- `Cargo.toml`
- `src/lib.rs`
- `agbot-plugin.toml`
- `README.md`

The scaffold uses the shared extension-point taxonomy, so an invalid kind is
rejected before any files are emitted. Generated manifests can be validated with
`validate_manifest` and registered through `PluginHost::register_plugin`.

## Examples

The SDK includes two executable example definitions:

- `example_custom_vegetation_index_manifest` and
  `example_custom_vegetation_index_spec`
- `example_report_template_manifest` and `example_report_template_request`

Both examples are covered by `plugin_sdk` tests and execute through
`PluginHost` under sandbox capability checks.
