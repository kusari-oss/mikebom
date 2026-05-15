cmake_minimum_required(VERSION 3.20)
project(test_cmake_project)

include(FetchContent)
include(ExternalProject)

FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG release-1.14.0
)

ExternalProject_Add(
    zlib
    URL https://zlib.net/zlib-1.3.1.tar.gz
    URL_HASH SHA256=9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23
)

find_package(OpenSSL REQUIRED)

include(cmake/third_party.cmake)

add_subdirectory(third_party/foo)
add_subdirectory(src)
