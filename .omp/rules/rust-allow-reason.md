---
description: Lint suppressions state why, and never hide a panic or safety lint
condition: '#!?\[(?:allow|expect)\((?![^\]]*\breason\s*=)'
scope: "tool:edit(*.rs), tool:write(*.rs)"
interruptMode: tool-only
---
Suppressions carry a reason (`Cargo.toml` denies `clippy::allow_attributes_without_reason`): `#[expect(clippy::too_many_arguments, reason = "mirrors easytier-core's flag set one to one")]`. Prefer `#[expect(…)]` over `#[allow(…)]` so the suppression fails once it is no longer needed, scope it to the smallest item, and say why the lint is wrong here.

Never suppress a lint to make a check pass when the lint is about panics, safety or dropped failures (`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `string_slice`, `arithmetic_side_effects`, `unwrap_in_result`, `let_underscore_must_use`, `unused_must_use`, `print_stdout`, `print_stderr`, `unsafe_code`) outside test code: fix the code instead. In test helpers (`#[cfg(test)]` modules) `arithmetic_side_effects` and `string_slice` still fire; a scoped `#[expect(…, reason = "test arithmetic on fixed values")]` is fine there.
