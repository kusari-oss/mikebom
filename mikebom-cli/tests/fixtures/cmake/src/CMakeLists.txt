# placeholder for first-party src sub-module test
# the parent CMakeLists.txt has `add_subdirectory(src)` which the
# CMake reader sees but MUST NOT emit as a vendored dep — the
# `third_party/` / `vendor/` path prefix is the gate.
