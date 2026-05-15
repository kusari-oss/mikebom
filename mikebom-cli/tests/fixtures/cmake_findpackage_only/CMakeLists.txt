cmake_minimum_required(VERSION 3.20)
project(findpackage_only)

include(FetchContent)

# A FetchContent_Declare for googletest proves the CMake reader RAN.
FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG release-1.14.0
)

# `find_package(zlib REQUIRED)` is the negative-emission anchor —
# per FR-007 the cmake reader MUST NOT emit any pkg:*/zlib component
# attributed to this CMakeLists.txt.
find_package(zlib REQUIRED)
